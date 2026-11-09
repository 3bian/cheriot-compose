// Copyright 3Bian Limited and Contributors
// SPDX-License-Identifier: MIT

use std::path::{Path, PathBuf};

use super::{
    command_help,
    command_name::CommandName,
    general_help,
    option_catalog::{self, OutputFormat},
};
use cheriot_compose::compilation_database::PathMapping;
use cheriot_compose::configuration::ProjectSettings;
use cheriot_compose::diagnostic::{Diagnostic, Origin, Result};

pub(super) struct Options {
    pub(super) command_name: CommandName,
    pub(super) audit_modules: Vec<PathBuf>,
    pub(super) audit_query: Option<String>,
    pub(super) audit_tool: Option<String>,
    pub(super) configuration_file: PathBuf,
    pub(super) compartment_name: Option<String>,
    pub(super) settings: ProjectSettings,
    pub(super) dry_run: bool,
    pub(super) firmware_name: Option<String>,
    pub(super) generated_database: Option<PathBuf>,
    pub(super) input_file: Option<PathBuf>,
    pub(super) include_packaging: bool,
    pub(super) jobs: usize,
    pub(super) operation_identifier: Option<String>,
    pub(super) output_file: Option<PathBuf>,
    pub(super) output_format: OutputFormat,
    pub(super) path_mappings: Vec<PathMapping>,
    pub(super) probe_toolchain: bool,
    pub(super) project_name: Option<String>,
    pub(super) reference_database: Option<PathBuf>,
    pub(super) runner_arguments: Vec<String>,
    pub(super) vm_profile: Option<PathBuf>,
    pub(super) verbose: bool,
    pub(super) quiet: bool,
}

fn environment_setting(name: &str) -> Result<Option<String>> {
    cheriot_compose::configuration::read_environment(
        name,
        cheriot_compose::configuration::EmptyEnvironmentValue::Reject,
    )
}

fn selected_value(
    option: Option<String>,
    variable: &str,
    option_name: &str,
) -> Result<Option<(String, Origin)>> {
    let (value, origin) = match option {
        Some(value) => (value, Origin::command_line(option_name)),
        None => match environment_setting(variable)? {
            Some(value) => (value, Origin::environment(variable)),
            None => return Ok(None),
        },
    };
    if value.is_empty() {
        return Err(Diagnostic::new(format!(
            "{} must not be empty",
            origin.rule
        )));
    }
    Ok(Some((value, origin)))
}

fn absolute_path(value: &str) -> Result<PathBuf> {
    let path = Path::new(value);
    if path.is_absolute() {
        Ok(cheriot_compose::configuration::resolution::normalize_path(
            path,
        ))
    } else {
        let current_directory = std::env::current_dir()
            .map_err(|error| Diagnostic::new(format!("cannot read current directory: {error}")))?;
        Ok(cheriot_compose::configuration::resolution::normalize_path(
            &current_directory.join(path),
        ))
    }
}

impl Options {
    pub(super) fn parse() -> Result<Option<Self>> {
        let mut arguments = std::env::args().skip(1);
        let Some(command_name) = arguments.next() else {
            print!("{}", general_help());
            return Ok(None);
        };
        if matches!(command_name.as_str(), "-h" | "--help") {
            print!("{}", general_help());
            return Ok(None);
        }
        if command_name.starts_with('-') {
            return Err(Diagnostic::new(
                "put the command first, for example: cheriot-compose build --toolchain-dir /cheriot-tools/bin --file project.toml",
            ));
        }
        if command_name == "help" {
            if let Some(command) = arguments.next() {
                if arguments.next().is_some() {
                    return Err(Diagnostic::new("help accepts at most one command"));
                }
                print!("{}", command_help(&command)?);
            } else {
                print!("{}", general_help());
            }
            return Ok(None);
        }
        if command_name == "version" {
            if arguments.next().is_some() {
                return Err(Diagnostic::new("version does not take options"));
            }
            println!("cheriot-compose {}", env!("CARGO_PKG_VERSION"));
            return Ok(None);
        }
        let parsed_command = CommandName::parse(&command_name).ok_or_else(|| {
            Diagnostic::new(format!(
                "unknown command {command_name}; use cheriot-compose help"
            ))
        })?;
        let mut file = None;
        let mut audit_modules = Vec::new();
        let mut audit_query = None;
        let mut audit_tool = None;
        let mut compartment_name = None;
        let mut board = None;
        let mut mode = None;
        let mut toolchain_directory = None;
        let mut build_directory = None;
        let mut output_directory = None;
        let mut jobs = None;
        let mut overlays = Vec::new();
        let mut dry_run = false;
        let mut firmware_name = None;
        let mut generated_database = None;
        let mut input_file = None;
        let mut include_packaging = false;
        let mut operation_identifier = None;
        let mut output_file = None;
        let mut output_format = None;
        let mut path_mappings = Vec::new();
        let mut probe_toolchain = false;
        let mut project_name = None;
        let mut reference_database = None;
        let mut runner_arguments = Vec::new();
        let mut vm_profile = None;
        let mut verbose = false;
        let mut quiet = false;
        let mut seen_options = std::collections::BTreeSet::new();
        while let Some(argument) = arguments.next() {
            if argument == "--" && parsed_command == CommandName::Run {
                runner_arguments.extend(arguments);
                break;
            }
            if !argument.starts_with('-')
                && parsed_command == CommandName::Explain
                && operation_identifier.is_none()
            {
                operation_identifier = Some(argument);
                continue;
            }
            let option = option_catalog::find(&argument).ok_or_else(|| {
                Diagnostic::new(format!(
                    "unexpected argument {argument}; use cheriot-compose help {command_name}"
                ))
            })?;
            if !option.commands.contains(&parsed_command) {
                return Err(Diagnostic::new(format!(
                    "{argument} is not valid for {command_name}; use cheriot-compose help {command_name}"
                )));
            }
            if option.name == "--help" {
                print!("{}", command_help(&command_name)?);
                return Ok(None);
            }
            if !seen_options.insert(option.name) && !option.repeatable {
                return Err(Diagnostic::new(if option.name == "--file" {
                    "use --file once; use --overlay for additional files".to_string()
                } else {
                    format!("{} may only be supplied once", option.name)
                }));
            }
            let value = if option.value.is_some() {
                let value = arguments
                    .next()
                    .ok_or_else(|| Diagnostic::new(format!("{argument} needs a value")))?;
                if value.is_empty() {
                    return Err(Diagnostic::new(format!("{argument} must not be empty")));
                }
                Some(value)
            } else {
                None
            };
            match (option.name, value) {
                ("--file", Some(value)) => file = Some(value),
                ("--firmware", Some(value)) => firmware_name = Some(value),
                ("--project", Some(value)) => project_name = Some(value),
                ("--format", Some(value)) => output_format = Some(value),
                ("--output", Some(value)) => output_file = Some(PathBuf::from(value)),
                ("--against", Some(value)) => reference_database = Some(PathBuf::from(value)),
                ("--generated", Some(value)) => generated_database = Some(PathBuf::from(value)),
                ("--input", Some(value)) => input_file = Some(PathBuf::from(value)),
                ("--vm-profile", Some(value)) => vm_profile = Some(absolute_path(&value)?),
                ("--module", Some(value)) => audit_modules.push(absolute_path(&value)?),
                ("--query", Some(value)) => audit_query = Some(value),
                ("--compartment", Some(value)) => compartment_name = Some(value),
                ("--audit-tool", Some(value)) => {
                    audit_tool = Some(if value.contains('/') {
                        absolute_path(&value)?.display().to_string()
                    } else {
                        value
                    })
                }
                ("--map-path", Some(value)) => path_mappings.push(PathMapping::parse(&value)?),
                ("--overlay", Some(value)) => overlays.push(absolute_path(&value)?),
                ("--board", Some(value)) => board = Some(value),
                ("--mode", Some(value)) => mode = Some(value),
                ("--toolchain-dir", Some(value)) => toolchain_directory = Some(value),
                ("--build-dir", Some(value)) => build_directory = Some(value),
                ("--output-dir", Some(value)) => output_directory = Some(value),
                ("--jobs", Some(value)) => jobs = Some(value),
                ("--dry-run", None) => dry_run = true,
                ("--verbose", None) => verbose = true,
                ("--quiet", None) => quiet = true,
                ("--resolved", None) => (),
                ("--probe-toolchain", None) => probe_toolchain = true,
                ("--package", None) => include_packaging = true,
                _ => {
                    return Err(Diagnostic::new(format!(
                        "missing parser for {}",
                        option.name
                    )));
                }
            }
        }
        let output_format =
            option_catalog::resolve_format(parsed_command, output_format.as_deref())?;
        if verbose && quiet {
            return Err(Diagnostic::new("--verbose and --quiet cannot be combined"));
        }
        if output_file.is_some()
            && matches!(parsed_command, CommandName::Build | CommandName::Package)
            && output_format != OutputFormat::Json
        {
            return Err(Diagnostic::new(
                "--output requires --format json for this command",
            ));
        }
        if output_file.is_some() && parsed_command == CommandName::Audit && dry_run {
            return Err(Diagnostic::new(
                "--output cannot be used with audit --dry-run",
            ));
        }
        if matches!(parsed_command, CommandName::Build | CommandName::Package)
            && output_format == OutputFormat::Json
            && (verbose || quiet)
        {
            return Err(Diagnostic::new(
                "--verbose and --quiet cannot be combined with --format json",
            ));
        }
        let imported_database = parsed_command == CommandName::CompareCompilationDatabase
            && generated_database.is_some();
        if imported_database {
            for option in option_catalog::OPTIONS
                .iter()
                .filter(|option| option.conflicts_with_imported_database)
            {
                if seen_options.contains(option.name) {
                    return Err(Diagnostic::new(format!(
                        "{} cannot be combined with --generated",
                        option.name
                    )));
                }
            }
        }
        if parsed_command == CommandName::AnalyzeSwitcher || imported_database {
            return Ok(Some(Self {
                command_name: parsed_command,
                audit_modules,
                audit_query,
                audit_tool,
                configuration_file: PathBuf::new(),
                compartment_name,
                settings: ProjectSettings::default(),
                dry_run,
                firmware_name,
                generated_database,
                input_file,
                include_packaging,
                jobs: 1,
                operation_identifier,
                output_file,
                output_format,
                path_mappings,
                probe_toolchain,
                project_name,
                reference_database,
                runner_arguments,
                vm_profile,
                verbose,
                quiet,
            }));
        }
        let configuration_file = match file {
            Some(value) => PathBuf::from(value),
            None => PathBuf::from(
                environment_setting("CHERIOT_COMPOSE_FILE")?
                    .unwrap_or_else(|| "cheriot-compose.toml".into()),
            ),
        };
        let board = selected_value(board, "CHERIOT_COMPOSE_BOARD", "--board")?
            .map(|(value, origin)| {
                let value = if value.contains('/') || value.contains('\\') {
                    absolute_path(&value)?.display().to_string()
                } else {
                    value
                };
                Ok::<_, Diagnostic>((value, origin))
            })
            .transpose()?;
        let mode = selected_value(mode, "CHERIOT_COMPOSE_MODE", "--mode")?;
        if mode
            .as_ref()
            .is_some_and(|(value, _)| !["debug", "release"].contains(&value.as_str()))
        {
            return Err(Diagnostic::new("build mode must be debug or release"));
        }
        let toolchain_directory = selected_value(
            toolchain_directory,
            "CHERIOT_COMPOSE_TOOLCHAIN_DIR",
            "--toolchain-dir",
        )?
        .map(|(value, origin)| Ok::<_, Diagnostic>((absolute_path(&value)?, origin)))
        .transpose()?;
        let build_directory =
            selected_value(build_directory, "CHERIOT_COMPOSE_BUILD_DIR", "--build-dir")?
                .map(|(value, origin)| Ok::<_, Diagnostic>((absolute_path(&value)?, origin)))
                .transpose()?;
        let output_directory = selected_value(
            output_directory,
            "CHERIOT_COMPOSE_OUTPUT_DIR",
            "--output-dir",
        )?
        .map(|(value, origin)| Ok::<_, Diagnostic>((absolute_path(&value)?, origin)))
        .transpose()?;
        let rust_compiler = selected_value(None, "RUSTC", "RUSTC")?
            .map(|(value, origin)| {
                let executable = if value.contains('/') {
                    absolute_path(&value)?.display().to_string()
                } else {
                    value
                };
                Ok::<_, Diagnostic>((executable, origin))
            })
            .transpose()?;
        let rust_flags = cheriot_compose::configuration::read_environment(
            "RUSTFLAGS",
            cheriot_compose::configuration::EmptyEnvironmentValue::Allow,
        )?
        .map(|value| {
            (
                value.split_whitespace().map(str::to_owned).collect(),
                Origin::environment("RUSTFLAGS"),
            )
        });
        let jobs = if super::option_catalog::accepts(parsed_command, "--jobs") {
            selected_value(jobs, "CHERIOT_COMPOSE_JOBS", "--jobs")?
                .map(|(value, _)| {
                    value
                        .parse::<usize>()
                        .ok()
                        .filter(|count| (1..=256).contains(count))
                        .ok_or_else(|| {
                            Diagnostic::new("jobs must be a whole number between 1 and 256")
                        })
                })
                .transpose()?
                .unwrap_or(1)
        } else {
            1
        };
        Ok(Some(Self {
            command_name: parsed_command,
            audit_modules,
            audit_query,
            audit_tool,
            configuration_file,
            compartment_name,
            settings: ProjectSettings {
                overlays,
                board,
                mode,
                toolchain_directory,
                build_directory,
                output_directory,
                rust_compiler,
                rust_flags,
            },
            dry_run,
            firmware_name,
            generated_database,
            input_file,
            include_packaging,
            jobs,
            operation_identifier,
            output_file,
            output_format,
            path_mappings,
            probe_toolchain,
            project_name,
            reference_database,
            runner_arguments,
            vm_profile,
            verbose,
            quiet,
        }))
    }
}
