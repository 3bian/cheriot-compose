// Copyright 3Bian Limited and Contributors
// SPDX-License-Identifier: MIT

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use super::{is_target_name, resolve_path_value};
use crate::configuration::{
    CompilerFlag, ComponentRole, ForcedInclude, RawProject, RawSource, ResolvedBoard,
    ResolvedBuildOptions, ResolvedSource, ResolvedTarget, TargetKind,
};
use crate::diagnostic::{Diagnostic, Origin, Result};

pub(super) struct GeneratedPaths<'a> {
    pub(super) build: &'a Path,
    pub(super) outputs: &'a BTreeSet<PathBuf>,
}

impl GeneratedPaths<'_> {
    fn resolve(&self, value: &str, origin: &Origin, root: &Path, sdk: &Path) -> Result<PathBuf> {
        resolve_path_value(
            &crate::configuration::generator::expand_build_path(value, self.build),
            origin,
            root,
            sdk,
        )
    }

    fn file_exists(&self, path: &Path) -> bool {
        self.outputs.contains(path) || (!path.starts_with(self.build) && path.is_file())
    }

    fn directory_exists(&self, path: &Path) -> bool {
        path.is_dir() || self.outputs.iter().any(|output| output.starts_with(path))
    }
}

pub(crate) fn validate_reachable_inputs(
    project: &crate::configuration::ResolvedProject,
    recipes: &[crate::configuration::ComponentRecipe],
) -> Result<()> {
    let outputs = project
        .generators
        .iter()
        .flat_map(|generator| generator.outputs.iter().cloned())
        .collect();
    let generated = GeneratedPaths {
        build: &project.build_dir,
        outputs: &outputs,
    };
    for recipe in recipes {
        let group = if project.sdk_component_names.contains(&recipe.name) {
            "sdk_components"
        } else if project
            .targets
            .get(&recipe.name)
            .is_some_and(|target| target.kind == TargetKind::Compartment)
        {
            "services"
        } else {
            "libraries"
        };
        for (index, source) in recipe.sources.iter().enumerate() {
            if !generated.file_exists(&source.path) {
                let field = format!("/{group}/{}/build/sources/{index}", recipe.name);
                let origin = project
                    .provenance
                    .get(&format!("{field}/path"))
                    .or_else(|| project.provenance.get(&field))
                    .cloned()
                    .unwrap_or_else(|| Origin::derived(&format!("SDK component {}", recipe.name)));
                let advice = if source.path.starts_with(project.sdk.join("third_party")) {
                    "; initialize the required SDK submodule if your checkout uses submodules"
                } else {
                    ""
                };
                return Err(Diagnostic::new(format!(
                    "source file required by {} does not exist: {}{advice}",
                    recipe.name,
                    source.path.display()
                ))
                .at(origin));
            }
            for include in &source.includes {
                if !generated.directory_exists(&include.value) {
                    return Err(Diagnostic::new(format!(
                        "include directory required by {} does not exist: {}",
                        recipe.name,
                        include.value.display()
                    ))
                    .at(include
                        .origins
                        .first()
                        .cloned()
                        .unwrap_or_else(|| Origin::derived("include directory"))));
                }
            }
            for include in &source.force_includes {
                if !generated.file_exists(&include.path) {
                    return Err(Diagnostic::new(format!(
                        "forced include required by {} does not exist: {}",
                        recipe.name,
                        include.path.display()
                    ))
                    .at(include.origin.clone()));
                }
            }
        }
        for include in &recipe.includes {
            if !generated.directory_exists(&include.value) {
                return Err(Diagnostic::new(format!(
                    "include directory required by {} does not exist: {}",
                    recipe.name,
                    include.value.display()
                ))
                .at(include
                    .origins
                    .first()
                    .cloned()
                    .unwrap_or_else(|| Origin::derived("include directory"))));
            }
        }
        for include in &recipe.force_includes {
            if !generated.file_exists(&include.path) {
                return Err(Diagnostic::new(format!(
                    "forced include required by {} does not exist: {}",
                    recipe.name,
                    include.path.display()
                ))
                .at(include.origin.clone()));
            }
        }
        if !matches!(
            recipe.kind,
            crate::configuration::SdkComponentKind::Object
                | crate::configuration::SdkComponentKind::Group
        ) {
            let script = project.sdk.join(&recipe.script);
            if !script.is_file() {
                return Err(Diagnostic::new(format!(
                    "linker script required by {} does not exist: {}",
                    recipe.name,
                    script.display()
                )));
            }
        }
    }
    Ok(())
}

fn resolve_compiler(
    value: &str,
    origin: Origin,
    root: &Path,
    sdk: &Path,
) -> Result<crate::diagnostic::Sourced<String>> {
    if value.is_empty() || value.starts_with('-') || value.contains(['\0', '\n', '\r']) {
        return Err(Diagnostic::new("compiler must be an executable name or path").at(origin));
    }
    let executable = if value.contains('/') || value.contains("${") {
        resolve_path_value(value, &origin, root, sdk)?
            .display()
            .to_string()
    } else {
        value.to_owned()
    };
    Ok(crate::diagnostic::Sourced::new(executable, origin))
}

fn resolve_definition(
    value: &str,
    name: &str,
    board: &ResolvedBoard,
    options: &ResolvedBuildOptions,
    mut origin: Origin,
    origins: &BTreeMap<String, Origin>,
) -> Result<CompilerFlag> {
    let debug = options.component_debug.get(name).copied().unwrap_or(false);
    let debug_text = if debug { "true" } else { "false" };
    let settings = [
        (
            "${board}",
            board.name.as_str(),
            "/board/selection".to_string(),
        ),
        (
            "${options.benchmark_metric}",
            options.benchmark_metric.name(),
            "/options/benchmark_metric".to_string(),
        ),
        (
            "${options.interrupt_source}",
            options.interrupt_source.name(),
            "/options/interrupt_source".to_string(),
        ),
        (
            "${options.component_debug}",
            debug_text,
            format!("/options/component_debug/{name}"),
        ),
    ];
    let mut result = value.to_string();
    for (token, replacement, field) in settings {
        if result.contains(token) {
            result = result.replace(token, replacement);
            let source = origins
                .get(&field)
                .cloned()
                .unwrap_or_else(|| Origin::derived(&format!("default {field}")));
            origin
                .rule
                .push_str(&format!("; {field} from {}:{}", source.file, source.line));
        }
    }
    if result.contains("${") || result.contains('\0') {
        return Err(Diagnostic::new("unknown define substitution or invalid NUL").at(origin));
    }
    Ok(CompilerFlag {
        value: format!("-D{result}"),
        origin,
    })
}

pub(super) fn resolve_targets(
    raw: &RawProject,
    board: &ResolvedBoard,
    options: &ResolvedBuildOptions,
    root: &Path,
    sdk: &Path,
    origins: &BTreeMap<String, Origin>,
    generated: &GeneratedPaths<'_>,
) -> Result<BTreeMap<String, ResolvedTarget>> {
    let at = |field_path: &str| {
        origins
            .get(field_path)
            .cloned()
            .unwrap_or_else(|| Origin::derived(field_path))
    };
    let fail = |field_path: &str, message: &str| Diagnostic::new(message).at(at(field_path));
    for name in options.component_debug.keys() {
        if !raw.services.contains_key(name)
            && !raw.libraries.contains_key(name)
            && !raw.sdk_components.contains_key(name)
        {
            return Err(fail(
                &format!("/options/component_debug/{name}"),
                "debug setting names an unknown component",
            ));
        }
    }
    let mut targets = BTreeMap::new();
    for (group, map) in [
        ("services", &raw.services),
        ("libraries", &raw.libraries),
        ("sdk_components", &raw.sdk_components),
    ] {
        for (name, target) in map {
            let prefix = format!("/{group}/{name}");
            if options.component_debug.contains_key(name) {
                let declared = target
                    .build
                    .defines
                    .iter()
                    .any(|value| value.contains("${options.component_debug}"))
                    || target.build.sources.iter().any(|source| match source {
                        RawSource::Detailed(source) => source
                            .defines
                            .iter()
                            .any(|value| value.contains("${options.component_debug}")),
                        RawSource::Path(_) => false,
                    });
                if !declared {
                    return Err(fail(
                        &format!("/options/component_debug/{name}"),
                        "component has no ${options.component_debug} define",
                    ));
                }
            }
            if !is_target_name(name) || name.starts_with("cheriot.") {
                return Err(fail(&prefix, "invalid or reserved target name"));
            }
            if group == "services" && target.kind != TargetKind::Compartment {
                return Err(fail(&prefix, "services must be compartments"));
            }
            if group == "libraries" && target.kind == TargetKind::Compartment {
                return Err(fail(&prefix, "put compartments under services"));
            }
            if target.role.is_some() && group != "sdk_components" {
                return Err(fail(
                    &prefix,
                    "component roles apply only to SDK components",
                ));
            }
            if let Some(role) = target.role {
                let expected = if role == ComponentRole::WallClock {
                    TargetKind::Compartment
                } else {
                    TargetKind::Library
                };
                if target.kind != expected {
                    return Err(fail(&prefix, "component kind does not match its SDK role"));
                }
            }
            if targets.contains_key(name) {
                return Err(fail(&prefix, "duplicate target definition"));
            }
            if target.build.sources.is_empty() && target.kind != TargetKind::Group {
                return Err(fail(&prefix, "build.sources must not be empty"));
            }
            if target.kind == TargetKind::Group && !target.build.sources.is_empty() {
                return Err(fail(&prefix, "a dependency group cannot have sources"));
            }
            if target.kind == TargetKind::Group
                && (!target.build.flags.is_empty()
                    || target.build.compiler.is_some()
                    || !target.build.c_flags.is_empty()
                    || !target.build.cpp_flags.is_empty()
                    || !target.build.assembly_flags.is_empty()
                    || !target.build.rust_flags.is_empty()
                    || !target.build.cargo_flags.is_empty()
                    || !target.build.defines.is_empty()
                    || !target.build.includes.is_empty()
                    || !target.build.force_includes.is_empty()
                    || !target.build.shared_objects.is_empty()
                    || target.build.revocable_memory)
            {
                return Err(fail(
                    &prefix,
                    "a dependency group cannot have build settings",
                ));
            }
            let mut sources = Vec::new();
            let mut seen = BTreeSet::new();
            for (source_index, source) in target.build.sources.iter().enumerate() {
                let source_prefix = format!("{prefix}/build/sources/{source_index}");
                let (source_path, settings, path_field) = match source {
                    RawSource::Path(path) => (path, None, source_prefix.clone()),
                    RawSource::Detailed(settings) => (
                        &settings.path,
                        Some(settings),
                        format!("{source_prefix}/path"),
                    ),
                };
                let origin = at(&path_field);
                let path = generated.resolve(source_path, &origin, root, sdk)?;
                if group != "sdk_components" && !generated.file_exists(&path) {
                    return Err(Diagnostic::new(format!(
                        "source file does not exist: {}",
                        path.display()
                    ))
                    .at(origin));
                }
                if !seen.insert(path.clone()) {
                    return Err(Diagnostic::new("duplicate source in target").at(origin));
                }
                let cargo_manifest = path.file_name().is_some_and(|name| name == "Cargo.toml");
                if !cargo_manifest
                    && !matches!(
                        path.extension().and_then(|text| text.to_str()),
                        Some("c" | "cc" | "cpp" | "cxx" | "S" | "s" | "rs")
                    )
                {
                    return Err(Diagnostic::new(
                        "source type unsupported; use C, C++, assembly, Rust, or Cargo.toml",
                    )
                    .at(origin));
                }
                let mut resolved = ResolvedSource::new(path);
                if cargo_manifest && generated.file_exists(&resolved.path) {
                    resolved.cargo_package = Some(
                        crate::configuration::cargo_package::read_cargo_package(&resolved.path)
                            .map_err(|error| error.at(origin.clone()))?,
                    );
                }
                if let Some(settings) = settings {
                    let rust_source = resolved
                        .path
                        .extension()
                        .is_some_and(|extension| extension == "rs");
                    if (cargo_manifest || rust_source)
                        && (!settings.flags.is_empty()
                            || settings.compiler.is_some()
                            || !settings.c_flags.is_empty()
                            || !settings.cpp_flags.is_empty()
                            || !settings.assembly_flags.is_empty()
                            || !settings.defines.is_empty()
                            || !settings.includes.is_empty()
                            || !settings.force_includes.is_empty())
                    {
                        return Err(Diagnostic::new("Rust sources use rust_flags and cargo_flags rather than C or C++ source settings").at(origin.clone()));
                    }
                    if !cargo_manifest && !settings.cargo_flags.is_empty() {
                        return Err(Diagnostic::new("cargo_flags require a Cargo.toml source")
                            .at(origin.clone()));
                    }
                    if !cargo_manifest && !rust_source && !settings.rust_flags.is_empty() {
                        return Err(Diagnostic::new("rust_flags require a Rust or Cargo source")
                            .at(origin.clone()));
                    }
                    if settings.rtos_git_description {
                        if !matches!(
                            resolved
                                .path
                                .extension()
                                .and_then(|extension| extension.to_str()),
                            Some("c" | "cc" | "cpp" | "cxx")
                        ) {
                            return Err(Diagnostic::new(
                                "RTOS Git description requires a C or C++ source",
                            )
                            .at(at(&format!("{source_prefix}/rtos_git_description"))));
                        }
                        resolved.rtos_git_description =
                            Some(at(&format!("{source_prefix}/rtos_git_description")));
                    }
                    let collect_flags = |field: &str,
                                         values: &[String],
                                         define: bool|
                     -> Result<Vec<CompilerFlag>> {
                        values
                            .iter()
                            .enumerate()
                            .map(|(index, value)| {
                                let origin = at(&format!("{source_prefix}/{field}/{index}"));
                                if value.contains('\0') {
                                    return Err(
                                        Diagnostic::new("flags cannot contain NUL").at(origin)
                                    );
                                }
                                if define {
                                    resolve_definition(value, name, board, options, origin, origins)
                                } else {
                                    Ok(CompilerFlag {
                                        value: value.clone(),
                                        origin,
                                    })
                                }
                            })
                            .collect()
                    };
                    resolved.flags = collect_flags("flags", &settings.flags, false)?;
                    resolved
                        .flags
                        .extend(collect_flags("defines", &settings.defines, true)?);
                    resolved.c_flags = collect_flags("c_flags", &settings.c_flags, false)?;
                    resolved.cpp_flags = collect_flags("cpp_flags", &settings.cpp_flags, false)?;
                    resolved.assembly_flags =
                        collect_flags("assembly_flags", &settings.assembly_flags, false)?;
                    resolved.compiler = settings
                        .compiler
                        .as_ref()
                        .map(|value| {
                            resolve_compiler(
                                value,
                                at(&format!("{source_prefix}/compiler")),
                                root,
                                sdk,
                            )
                        })
                        .transpose()?;
                    resolved.rust_flags = collect_flags("rust_flags", &settings.rust_flags, false)?;
                    resolved.cargo_flags =
                        collect_flags("cargo_flags", &settings.cargo_flags, false)?;
                    crate::configuration::cargo_package::validate_cargo_flags(
                        &resolved.cargo_flags,
                    )?;
                    for (index, include) in settings.includes.iter().enumerate() {
                        let origin = at(&format!("{source_prefix}/includes/{index}"));
                        let path = generated.resolve(include, &origin, root, sdk)?;
                        if group != "sdk_components" && !generated.directory_exists(&path) {
                            return Err(Diagnostic::new(format!(
                                "include directory does not exist: {}",
                                path.display()
                            ))
                            .at(origin));
                        }
                        resolved
                            .includes
                            .push(crate::diagnostic::Sourced::new(path, origin));
                    }
                    for (index, include) in settings.force_includes.iter().enumerate() {
                        let origin = at(&format!("{source_prefix}/force_includes/{index}"));
                        let path = generated.resolve(include, &origin, root, sdk)?;
                        if group != "sdk_components" && !generated.file_exists(&path) {
                            return Err(Diagnostic::new(format!(
                                "forced include does not exist: {}",
                                path.display()
                            ))
                            .at(origin));
                        }
                        resolved.force_includes.push(ForcedInclude { path, origin });
                    }
                }
                sources.push(resolved);
            }
            let includes = target
                .build
                .includes
                .iter()
                .enumerate()
                .map(|(index, include)| {
                    let origin = at(&format!("{prefix}/build/includes/{index}"));
                    let path = generated.resolve(include, &origin, root, sdk)?;
                    if group != "sdk_components" && !generated.directory_exists(&path) {
                        return Err(Diagnostic::new(format!(
                            "include directory does not exist: {}",
                            path.display()
                        ))
                        .at(origin));
                    }
                    Ok(crate::diagnostic::Sourced::new(path, origin))
                })
                .collect::<Result<Vec<_>>>()?;
            let mut flags = Vec::new();
            for (flag_index, flag) in target.build.flags.iter().enumerate() {
                flags.push(CompilerFlag {
                    value: flag.clone(),
                    origin: at(&format!("{prefix}/build/flags/{flag_index}")),
                });
            }
            for (definition_index, definition) in target.build.defines.iter().enumerate() {
                flags.push(resolve_definition(
                    definition,
                    name,
                    board,
                    options,
                    at(&format!("{prefix}/build/defines/{definition_index}")),
                    origins,
                )?);
            }
            let mut dependencies = target.dependencies.clone();
            let mut set_print_option = |enabled: bool, option: &str, define: &str, width: &str| {
                if enabled {
                    for operation in ["compare", "convert", "add", "sub", "mul"] {
                        dependencies.push(format!("softfloat{width}{operation}"));
                    }
                } else {
                    flags.push(CompilerFlag {
                        value: format!("-D{define}"),
                        origin: origins
                            .get(&format!("/options/{option}"))
                            .or_else(|| origins.get(&format!("{prefix}/role")))
                            .cloned()
                            .unwrap_or_else(|| Origin::derived(&format!("SDK option {option}"))),
                    });
                }
            };
            match target.role {
                Some(ComponentRole::Debug) => {
                    set_print_option(
                        options.print_floats,
                        "print_floats",
                        "CHERIOT_NO_FLOAT_PRINTING",
                        "32",
                    );
                    set_print_option(
                        options.print_doubles,
                        "print_doubles",
                        "CHERIOT_NO_DOUBLE_PRINTING",
                        "64",
                    );
                }
                Some(ComponentRole::Stdio) => {
                    set_print_option(
                        options.print_doubles,
                        "print_doubles",
                        "CHERIOT_NO_DOUBLE_PRINTING",
                        "64",
                    );
                }
                _ => {}
            }
            let mut force_includes = Vec::new();
            for (index, include) in target.build.force_includes.iter().enumerate() {
                let origin = at(&format!("{prefix}/build/force_includes/{index}"));
                let path = generated.resolve(include, &origin, root, sdk)?;
                if group != "sdk_components" && !generated.file_exists(&path) {
                    return Err(Diagnostic::new(format!(
                        "forced include does not exist: {}",
                        path.display()
                    ))
                    .at(origin));
                }
                force_includes.push(ForcedInclude { path, origin });
            }
            if target.build.revocable_memory {
                flags.push(CompilerFlag {
                    value: format!("-DREVOKABLE_MEMORY_START=0x{:x}", board.revocable_start),
                    origin: at(&format!("{prefix}/build/revocable_memory")),
                });
            }
            let language_flags = |name: &str, values: &[String]| {
                values
                    .iter()
                    .enumerate()
                    .map(|(index, value)| CompilerFlag {
                        value: value.clone(),
                        origin: at(&format!("{prefix}/build/{name}/{index}")),
                    })
                    .collect::<Vec<_>>()
            };
            let c_flags = language_flags("c_flags", &target.build.c_flags);
            let cpp_flags = language_flags("cpp_flags", &target.build.cpp_flags);
            let assembly_flags = language_flags("assembly_flags", &target.build.assembly_flags);
            let compiler = target
                .build
                .compiler
                .as_ref()
                .map(|value| {
                    resolve_compiler(value, at(&format!("{prefix}/build/compiler")), root, sdk)
                })
                .transpose()?;
            let rust_flags = language_flags("rust_flags", &target.build.rust_flags);
            let cargo_flags = language_flags("cargo_flags", &target.build.cargo_flags);
            crate::configuration::cargo_package::validate_cargo_flags(&cargo_flags)?;
            if flags
                .iter()
                .chain(&c_flags)
                .chain(&cpp_flags)
                .chain(&assembly_flags)
                .chain(&rust_flags)
                .chain(&cargo_flags)
                .any(|flag| flag.value.contains('\0'))
            {
                return Err(fail(&prefix, "flags cannot contain NUL"));
            }
            targets.insert(
                name.clone(),
                ResolvedTarget {
                    kind: target.kind,
                    role: target.role,
                    sources,
                    compiler,
                    includes,
                    force_includes,
                    flags,
                    c_flags,
                    cpp_flags,
                    assembly_flags,
                    rust_flags,
                    cargo_flags,
                    dependencies,
                    shared_objects: target.build.shared_objects.clone(),
                },
            );
        }
    }
    Ok(targets)
}
