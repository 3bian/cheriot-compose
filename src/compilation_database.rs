// Copyright 3Bian Limited and Contributors
// SPDX-License-Identifier: MIT

use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};

use crate::{
    build_graph::{BuildOperationKind, BuildPlan, CommandResolution},
    configuration::resolution::normalize_path,
    diagnostic::{Diagnostic, Result},
};

mod comparison_report;
mod compilation_difference;
mod compilation_entry;
mod import;
mod path_mapping;

pub use comparison_report::ComparisonReport;
pub use compilation_difference::CompilationDifference;
pub use compilation_entry::CompilationEntry;
pub use import::{read_compilation_database, split_command_arguments};
pub use path_mapping::PathMapping;

pub fn export_compilation_database(
    plan: &BuildPlan,
    probes: &crate::build_graph::ProbeResults,
) -> Result<Vec<CompilationEntry>> {
    let mut result = Vec::new();
    for operation in &plan.operations {
        if !matches!(
            operation.kind,
            BuildOperationKind::Compile | BuildOperationKind::Assemble
        ) {
            continue;
        }
        if let Some(resolution) = operation.command_resolution(probes)? {
            let variants = match resolution {
                CommandResolution::Concrete(command) => vec![(command, None)],
                CommandResolution::Alternatives {
                    probe,
                    when_false,
                    when_true,
                } => vec![
                    (when_false, Some(format!("{probe}=false (unresolved)"))),
                    (when_true, Some(format!("{probe}=true (unresolved)"))),
                ],
            };
            for (command, condition) in variants {
                let mut arguments = vec![command.executable];
                arguments.extend(command.argv.into_iter().map(|argument| argument.value));
                result.push(CompilationEntry {
                    directory: command.cwd.display().to_string(),
                    file: operation
                        .inputs
                        .first()
                        .ok_or_else(|| Diagnostic::new("compile input missing"))?
                        .display()
                        .to_string(),
                    output: operation
                        .outputs
                        .first()
                        .map(|path| path.display().to_string()),
                    arguments,
                    command: None,
                    cheriot_compose_condition: condition,
                    cheriot_compose_target: operation
                        .id
                        .strip_prefix("compile.")
                        .and_then(|id| id.rsplit_once('.').map(|(target, _)| target.to_owned())),
                });
            }
        }
    }
    Ok(result)
}

fn mapped(text: &str, path_mappings: &[PathMapping]) -> String {
    let mut best = None;
    for mapping in path_mappings {
        if (text == mapping.old || text.starts_with(&(mapping.old.clone() + "/")))
            && best.is_none_or(|previous: &PathMapping| mapping.old.len() > previous.old.len())
        {
            best = Some(mapping)
        }
    }
    if let Some(mapping) = best {
        format!("{}{}", mapping.new, &text[mapping.old.len()..])
    } else {
        text.into()
    }
}

fn absolute(text: &str, directory: &str) -> String {
    normalize_path(&if Path::new(text).is_absolute() {
        PathBuf::from(text)
    } else {
        Path::new(directory).join(text)
    })
    .display()
    .to_string()
}

fn normalize(entry: &CompilationEntry, path_mappings: &[PathMapping]) -> CompilationEntry {
    let mut arguments = Vec::with_capacity(entry.arguments.len());
    let mut path_next = false;
    for (argument_index, argument) in entry.arguments.iter().enumerate() {
        if argument_index == 0 {
            arguments.push(mapped(argument, path_mappings));
            continue;
        }
        if path_next {
            arguments.push(mapped(&absolute(argument, &entry.directory), path_mappings));
            path_next = false;
            continue;
        }
        if [
            "-o",
            "-I",
            "-isystem",
            "-iquote",
            "-include",
            "-imacros",
            "-MF",
            "--sysroot",
        ]
        .contains(&argument.as_str())
        {
            path_next = true;
            arguments.push(argument.clone());
            continue;
        }
        if let Some(prefix) = ["--sysroot=", "-isystem", "-iquote", "-include", "-I"]
            .iter()
            .find(|path| argument.starts_with(**path) && argument.len() > path.len())
        {
            arguments.push(format!(
                "{prefix}{}",
                mapped(
                    &absolute(&argument[prefix.len()..], &entry.directory),
                    path_mappings
                )
            ));
        } else if !argument.starts_with('-') && (argument == &entry.file || argument.contains('/'))
        {
            arguments.push(mapped(&absolute(argument, &entry.directory), path_mappings))
        } else {
            arguments.push(argument.clone())
        }
    }
    CompilationEntry {
        directory: mapped(
            &normalize_path(Path::new(&entry.directory))
                .display()
                .to_string(),
            path_mappings,
        ),
        file: mapped(&absolute(&entry.file, &entry.directory), path_mappings),
        output: entry
            .output
            .as_ref()
            .map(|path| mapped(&absolute(path, &entry.directory), path_mappings)),
        arguments,
        command: None,
        cheriot_compose_condition: entry.cheriot_compose_condition.clone(),
        cheriot_compose_target: entry.cheriot_compose_target.clone(),
    }
}

fn target(entry: &CompilationEntry) -> Option<&str> {
    entry.cheriot_compose_target.as_deref().or_else(|| {
        entry
            .output
            .as_ref()?
            .split("/.objs/")
            .nth(1)?
            .split('/')
            .next()
    })
}

pub fn compare_compilation_databases(
    reference: &[CompilationEntry],
    actual: &[CompilationEntry],
    path_mappings: &[PathMapping],
) -> ComparisonReport {
    let old: Vec<_> = reference
        .iter()
        .map(|entry| normalize(entry, path_mappings))
        .collect();
    let new: Vec<_> = actual
        .iter()
        .map(|entry| normalize(entry, path_mappings))
        .collect();
    let mut entries_by_file: BTreeMap<&str, Vec<usize>> = BTreeMap::new();
    for (index, entry) in new.iter().enumerate() {
        entries_by_file.entry(&entry.file).or_default().push(index);
    }
    let mut report = ComparisonReport {
        reference_count: old.len(),
        generated_count: new.len(),
        missing: Vec::new(),
        extra: Vec::new(),
        different: Vec::new(),
        ambiguous: Vec::new(),
        conditional: Vec::new(),
    };
    let mut used = vec![false; new.len()];
    for entry in &new {
        if let Some(condition) = &entry.cheriot_compose_condition {
            report
                .conditional
                .push(format!("{}: {condition}", entry.file))
        }
    }
    for reference_entry in old {
        let same: Vec<usize> = entries_by_file
            .get(reference_entry.file.as_str())
            .into_iter()
            .flatten()
            .copied()
            .filter(|index| !used[*index])
            .collect();
        let exact: Vec<_> = same
            .iter()
            .copied()
            .filter(|index| new[*index].output == reference_entry.output)
            .collect();
        let candidates = if exact.is_empty() {
            let same_target: Vec<_> = same
                .iter()
                .copied()
                .filter(|index| target(&new[*index]) == target(&reference_entry))
                .collect();
            if same_target.is_empty() {
                same
            } else {
                same_target
            }
        } else {
            exact
        };
        let found = if candidates.len() == 1 {
            candidates.first().copied()
        } else if candidates.len() > 1 {
            let matching_arguments: Vec<_> = candidates
                .iter()
                .copied()
                .filter(|index| new[*index].arguments == reference_entry.arguments)
                .collect();
            if matching_arguments.len() == 1 {
                matching_arguments.first().copied()
            } else {
                report.ambiguous.push(format!(
                    "{} ({:?}): {} possible variants",
                    reference_entry.file,
                    reference_entry.output,
                    candidates.len()
                ));
                None
            }
        } else {
            None
        };
        let Some(index) = found else {
            report.missing.push(reference_entry);
            continue;
        };
        used[index] = true;
        let actual_entry = &new[index];
        let mut changes = Vec::new();
        if reference_entry.directory != actual_entry.directory {
            changes.push(format!(
                "working directory: {:?} -> {:?}",
                reference_entry.directory, actual_entry.directory
            ))
        }
        if reference_entry.output != actual_entry.output {
            changes.push(format!(
                "output path: {:?} -> {:?}",
                reference_entry.output, actual_entry.output
            ))
        }
        for flag in ["-target", "--target", "-mabi", "-mcpu"] {
            let value = |arguments: &[String]| -> Option<String> {
                arguments
                    .windows(2)
                    .find(|window| window[0] == flag)
                    .map(|window| window[1].clone())
                    .or_else(|| {
                        arguments.iter().find_map(|argument| {
                            argument
                                .strip_prefix(&format!("{flag}="))
                                .map(str::to_string)
                        })
                    })
            };
            if value(&reference_entry.arguments) != value(&actual_entry.arguments) {
                changes.push(format!(
                    "target/ABI/CPU {flag}: {:?} -> {:?}",
                    value(&reference_entry.arguments),
                    value(&actual_entry.arguments)
                ))
            }
        }
        for argument_index in 0..reference_entry
            .arguments
            .len()
            .max(actual_entry.arguments.len())
        {
            if reference_entry.arguments.get(argument_index)
                != actual_entry.arguments.get(argument_index)
            {
                changes.push(format!(
                    "argv[{argument_index}]: {:?} -> {:?}",
                    reference_entry.arguments.get(argument_index),
                    actual_entry.arguments.get(argument_index)
                ))
            }
        }
        if !changes.is_empty() {
            report.different.push(CompilationDifference {
                file: reference_entry.file,
                output: reference_entry.output,
                changes,
            });
        }
    }
    drop(entries_by_file);
    report.extra = new
        .into_iter()
        .enumerate()
        .filter(|(index, _)| !used[*index])
        .map(|(_, entry)| entry)
        .collect();
    report
}

pub fn count_entries_by_target(entries: &[CompilationEntry]) -> BTreeMap<String, usize> {
    let mut counts = BTreeMap::new();
    for entry in entries {
        *counts
            .entry(target(entry).unwrap_or("unknown").into())
            .or_insert(0) += 1;
    }
    counts
}
