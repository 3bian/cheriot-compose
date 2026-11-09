// Copyright 3Bian Limited and Contributors
// SPDX-License-Identifier: MIT

use super::command_name::CommandName;
use CommandName::*;

mod formats;
mod option_description;
mod output_format;

use cheriot_compose::diagnostic::{Diagnostic, Result};
pub(super) use formats::Formats;
pub(super) use option_description::OptionDescription;
pub(super) use output_format::OutputFormat;

pub(super) const ALL: &[CommandName] = &[
    Check,
    Configuration,
    Plan,
    CompareCompilationDatabase,
    Build,
    Run,
    Explain,
    Clean,
    AnalyzeSwitcher,
    Audit,
    Package,
    Size,
];
const PROJECT: &[CommandName] = &[
    Check,
    Configuration,
    Plan,
    CompareCompilationDatabase,
    Build,
    Run,
    Explain,
    Clean,
    Audit,
    Package,
    Size,
];
const PROJECT_OUTPUT: &[CommandName] = &[
    Check,
    Configuration,
    Plan,
    Build,
    Run,
    Explain,
    Clean,
    Audit,
    Package,
    Size,
];
const TEXT_OUTPUT: &[CommandName] = &[
    Check,
    Configuration,
    Plan,
    CompareCompilationDatabase,
    Build,
    Explain,
    Clean,
    AnalyzeSwitcher,
    Audit,
    Package,
    Size,
];
const BUILDING: &[CommandName] = &[Build, Run, Audit, Package, Size];
const FORMATTED: &[CommandName] = &[
    Configuration,
    Plan,
    CompareCompilationDatabase,
    Build,
    Package,
    Size,
];
const PACKAGING: &[CommandName] = &[Plan, Explain, Clean];
const PROBING: &[CommandName] = &[Plan, CompareCompilationDatabase];
const DATABASE: &[CommandName] = &[CompareCompilationDatabase];

const fn option(
    name: &'static str,
    value: Option<&'static str>,
    repeatable: bool,
    commands: &'static [CommandName],
    description: &'static str,
) -> OptionDescription {
    OptionDescription {
        name,
        aliases: &[],
        value,
        repeatable,
        commands,
        conflicts_with_imported_database: false,
        description,
    }
}
const fn project_option(
    name: &'static str,
    value: Option<&'static str>,
    repeatable: bool,
    commands: &'static [CommandName],
    description: &'static str,
) -> OptionDescription {
    OptionDescription {
        conflicts_with_imported_database: true,
        ..option(name, value, repeatable, commands, description)
    }
}

pub(super) const OPTIONS: &[OptionDescription] = &[
    option(
        "--against",
        Some("FILE"),
        false,
        DATABASE,
        "Reference compilation database.",
    ),
    option(
        "--audit-tool",
        Some("FILE"),
        false,
        &[Audit],
        "Audit executable.",
    ),
    project_option(
        "--board",
        Some("BOARD"),
        false,
        PROJECT,
        "Board name or TOML file.",
    ),
    project_option(
        "--build-dir",
        Some("DIR"),
        false,
        PROJECT,
        "Root for intermediate files.",
    ),
    option(
        "--compartment",
        Some("NAME"),
        false,
        &[Size],
        "Compartment to measure.",
    ),
    option(
        "--dry-run",
        None,
        false,
        BUILDING,
        "Describe work without executing it.",
    ),
    OptionDescription {
        aliases: &["-f"],
        ..project_option(
            "--file",
            Some("FILE"),
            false,
            PROJECT,
            "Project or workspace manifest.",
        )
    },
    project_option(
        "--firmware",
        Some("NAME"),
        false,
        PROJECT,
        "Select a firmware image.",
    ),
    option(
        "--format",
        Some("FORMAT"),
        false,
        FORMATTED,
        "Select the output format.",
    ),
    option(
        "--generated",
        Some("FILE"),
        false,
        DATABASE,
        "Compare an existing generated database.",
    ),
    OptionDescription {
        aliases: &["-h"],
        ..option("--help", None, false, ALL, "Show this help.")
    },
    option(
        "--input",
        Some("FILE"),
        false,
        &[AnalyzeSwitcher],
        "Input switcher assembly.",
    ),
    option(
        "--jobs",
        Some("NUMBER"),
        false,
        BUILDING,
        "Maximum parallel jobs.",
    ),
    option(
        "--map-path",
        Some("OLD=NEW"),
        true,
        DATABASE,
        "Map a reference path; repeatable.",
    ),
    project_option(
        "--mode",
        Some("debug|release"),
        false,
        PROJECT,
        "Select the build mode.",
    ),
    option(
        "--module",
        Some("FILE"),
        true,
        &[Audit],
        "Audit module; repeatable.",
    ),
    option(
        "--output",
        Some("FILE"),
        false,
        TEXT_OUTPUT,
        "Write the command's text or structured output.",
    ),
    project_option(
        "--output-dir",
        Some("DIR"),
        false,
        PROJECT_OUTPUT,
        "Destination for final firmware files.",
    ),
    project_option(
        "--overlay",
        Some("FILE"),
        true,
        PROJECT,
        "Apply an ordered project overlay; repeatable.",
    ),
    option(
        "--package",
        None,
        false,
        PACKAGING,
        "Include board packaging operations.",
    ),
    project_option(
        "--probe-toolchain",
        None,
        false,
        PROBING,
        "Run compiler feature probes.",
    ),
    project_option(
        "--project",
        Some("NAME"),
        false,
        PROJECT,
        "Select a workspace project.",
    ),
    option("--query", Some("QUERY"), false, &[Audit], "Audit query."),
    option(
        "--quiet",
        None,
        false,
        BUILDING,
        "Hide routine build progress.",
    ),
    option(
        "--resolved",
        None,
        false,
        &[Configuration],
        "Show resolved configuration.",
    ),
    project_option(
        "--toolchain-dir",
        Some("DIR"),
        false,
        PROJECT,
        "Directory containing LLVM executables.",
    ),
    option(
        "--verbose",
        None,
        false,
        BUILDING,
        "Show commands and rebuild reasons.",
    ),
    option(
        "--vm-profile",
        Some("FILE"),
        false,
        &[Run],
        "CHERIoT VM runtime profile.",
    ),
];

pub(super) fn find(spelling: &str) -> Option<&'static OptionDescription> {
    OPTIONS
        .iter()
        .find(|option| option.name == spelling || option.aliases.contains(&spelling))
}

pub(super) fn accepts(command: CommandName, spelling: &str) -> bool {
    find(spelling).is_some_and(|option| option.commands.contains(&command))
}

pub(super) fn formats(command: CommandName) -> Formats {
    use OutputFormat::*;
    match command {
        Configuration => Formats {
            default: Toml,
            supported: &[Toml, Json],
        },
        Plan => Formats {
            default: Text,
            supported: &[Text, Json, Shell, CompilationDatabase],
        },
        CompareCompilationDatabase => Formats {
            default: Text,
            supported: &[Text, Json],
        },
        Build | Package => Formats {
            default: Human,
            supported: &[Json],
        },
        Size => Formats {
            default: Text,
            supported: &[Text, Json, Bytes],
        },
        _ => Formats {
            default: Human,
            supported: &[],
        },
    }
}

pub(super) fn resolve_format(
    command: CommandName,
    requested: Option<&str>,
) -> Result<OutputFormat> {
    let available = formats(command);
    match requested {
        None => Ok(available.default),
        Some(value) => available
            .supported
            .iter()
            .copied()
            .find(|format| format.name() == value)
            .ok_or_else(|| {
                Diagnostic::new(format!("invalid format {value} for {}", command.name()))
            }),
    }
}
