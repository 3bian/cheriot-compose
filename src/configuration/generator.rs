// Copyright 3Bian Limited and Contributors
// SPDX-License-Identifier: MIT

use std::{
    collections::{BTreeMap, BTreeSet},
    path::{Path, PathBuf},
};

use super::resolution::{is_identifier, is_target_name, normalize_path, resolve_path_value};
use crate::diagnostic::{Diagnostic, Origin, Result, Sourced};

mod raw_generator;
mod resolved_generator;

pub use raw_generator::RawGenerator;
pub use resolved_generator::ResolvedGenerator;

fn expand(value: &str, root: &Path, sdk: &Path, build: &Path, origin: &Origin) -> Result<String> {
    let value = value
        .replace("${project}", &root.to_string_lossy())
        .replace("${sdk}", &sdk.to_string_lossy())
        .replace("${build}", &build.to_string_lossy());
    if value.contains("${") || value.contains('\0') {
        return Err(
            Diagnostic::new("unknown generator substitution or NUL character").at(origin.clone()),
        );
    }
    Ok(value)
}

pub(super) fn expand_build_path(value: &str, build: &Path) -> String {
    value.replace("${build}", &build.to_string_lossy())
}

pub(super) fn resolve_generators(
    raw: &BTreeMap<String, RawGenerator>,
    root: &Path,
    sdk: &Path,
    build: &Path,
    origins: &BTreeMap<String, Origin>,
) -> Result<Vec<ResolvedGenerator>> {
    let at = |field: &str| {
        origins
            .get(field)
            .cloned()
            .unwrap_or_else(|| Origin::derived(field))
    };
    let mut resolved = BTreeMap::new();
    let mut producers = BTreeMap::new();
    for (name, generator) in raw {
        let prefix = format!("/generators/{name}");
        let origin = at(&prefix);
        if !is_target_name(name) || name.starts_with("cheriot.") {
            return Err(Diagnostic::new("invalid or reserved generator name").at(origin));
        }
        let executable_origin = at(&format!("{prefix}/executable"));
        let executable = expand(&generator.executable, root, sdk, build, &executable_origin)?;
        if executable.is_empty() || executable.starts_with('-') || executable.contains(['\n', '\r'])
        {
            return Err(
                Diagnostic::new("generator executable must be a tool name or path")
                    .at(executable_origin),
            );
        }
        let executable = if executable.contains('/') {
            resolve_path_value(&executable, &executable_origin, root, sdk)?
                .display()
                .to_string()
        } else {
            executable
        };
        let arguments = generator
            .arguments
            .iter()
            .enumerate()
            .map(|(index, value)| {
                let origin = at(&format!("{prefix}/arguments/{index}"));
                Ok(Sourced::new(
                    expand(value, root, sdk, build, &origin)?,
                    origin,
                ))
            })
            .collect::<Result<Vec<_>>>()?;
        let working_origin = at(&format!("{prefix}/working_directory"));
        let working_directory = match &generator.working_directory {
            Some(value) => resolve_path_value(
                &expand(value, root, sdk, build, &working_origin)?,
                &working_origin,
                root,
                sdk,
            )?,
            None => Path::new(&origin.file)
                .parent()
                .unwrap_or(root)
                .to_path_buf(),
        };
        if !working_directory.is_dir() {
            return Err(Diagnostic::new(format!(
                "generator working directory does not exist: {}",
                working_directory.display()
            ))
            .at(working_origin));
        }
        for (key, value) in &generator.environment {
            if !is_identifier(key) || value.contains('\0') {
                return Err(
                    Diagnostic::new("invalid generator environment name or value")
                        .at(at(&format!("{prefix}/environment/{key}"))),
                );
            }
        }
        let output_path = |value: &str, field: &str| -> Result<PathBuf> {
            let origin = at(field);
            let value = expand(value, root, sdk, build, &origin)?;
            if value.is_empty() || value.contains(['*', '?', '[', '\n', '\r']) {
                return Err(
                    Diagnostic::new("generator output must be an explicit file path").at(origin),
                );
            }
            let path = Path::new(&value);
            let path = normalize_path(&if path.is_absolute() {
                path.to_path_buf()
            } else {
                build.join(path)
            });
            let reserved = path.parent() == Some(build)
                && path
                    .file_name()
                    .and_then(|name| name.to_str())
                    .is_some_and(|name| {
                        name.starts_with(".publication-")
                            || name.starts_with(".outputs-")
                            || name.starts_with(".identity-")
                            || name.starts_with("state-") && name.ends_with(".json")
                    });
            if !path.starts_with(build)
                || path == build
                || path == build.join("state.json")
                || path == build.join(".cheriot-compose.lock")
                || reserved
            {
                return Err(Diagnostic::new(
                    "generator outputs must be files inside the build directory",
                )
                .at(origin));
            }
            Ok(path)
        };
        if generator.outputs.is_empty() {
            return Err(
                Diagnostic::new("generator must declare at least one output")
                    .at(at(&format!("{prefix}/outputs"))),
            );
        }
        let mut outputs = Vec::new();
        for (index, value) in generator.outputs.iter().enumerate() {
            let field = format!("{prefix}/outputs/{index}");
            let output = output_path(value, &field)?;
            if let Some(previous) = producers.insert(output.clone(), name.clone()) {
                return Err(Diagnostic::new(format!(
                    "generator output is also produced by {previous}: {}",
                    output.display()
                ))
                .at(at(&field)));
            }
            outputs.push(output);
        }
        let redirect = |value: &Option<String>, field: &str| -> Result<Option<PathBuf>> {
            value
                .as_ref()
                .map(|value| {
                    let output = output_path(value, field)?;
                    if !outputs.contains(&output) {
                        return Err(Diagnostic::new(
                            "redirect must name a declared generator output",
                        )
                        .at(at(field)));
                    }
                    Ok(output)
                })
                .transpose()
        };
        let stdout = redirect(&generator.stdout, &format!("{prefix}/stdout"))?;
        let stderr = redirect(&generator.stderr, &format!("{prefix}/stderr"))?;
        if stdout.is_some() && stdout == stderr {
            return Err(
                Diagnostic::new("stdout and stderr must use separate output files").at(origin),
            );
        }
        let inputs = generator
            .inputs
            .iter()
            .enumerate()
            .map(|(index, value)| {
                let origin = at(&format!("{prefix}/inputs/{index}"));
                resolve_path_value(
                    &expand(value, root, sdk, build, &origin)?,
                    &origin,
                    root,
                    sdk,
                )
            })
            .collect::<Result<Vec<_>>>()?;
        resolved.insert(
            name.clone(),
            ResolvedGenerator {
                name: name.clone(),
                executable,
                arguments,
                inputs,
                outputs,
                dependencies: generator.dependencies.clone(),
                working_directory,
                environment: generator.environment.clone(),
                stdout,
                stderr,
                origin,
            },
        );
    }
    for (name, generator) in &mut resolved {
        let mut dependencies = BTreeSet::new();
        for dependency in &generator.dependencies {
            if !raw.contains_key(dependency) || dependency == name {
                return Err(Diagnostic::new(format!(
                    "unknown or self-referencing generator dependency {dependency}"
                ))
                .at(generator.origin.clone()));
            }
            dependencies.insert(dependency.clone());
        }
        for input in &generator.inputs {
            if let Some(producer) = producers.get(input) {
                if producer == name {
                    return Err(Diagnostic::new("a generator cannot consume its own output")
                        .at(generator.origin.clone()));
                }
                dependencies.insert(producer.clone());
            } else if input.starts_with(build) || !input.is_file() {
                return Err(Diagnostic::new(format!(
                    "generator input does not exist or has no producer: {}",
                    input.display()
                ))
                .at(generator.origin.clone()));
            }
        }
        generator.dependencies = dependencies.into_iter().collect();
    }
    let mut ordered = Vec::new();
    let mut done = BTreeSet::new();
    while !resolved.is_empty() {
        let Some(name) = resolved
            .iter()
            .find(|(_, generator)| generator.dependencies.iter().all(|dep| done.contains(dep)))
            .map(|(name, _)| name.clone())
        else {
            return Err(Diagnostic::new("generator dependencies contain a cycle"));
        };
        let generator = resolved
            .remove(&name)
            .ok_or_else(|| Diagnostic::new("generator disappeared during resolution"))?;
        done.insert(name);
        ordered.push(generator);
    }
    Ok(ordered)
}
