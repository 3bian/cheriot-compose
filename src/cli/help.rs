// Copyright 3Bian Limited and Contributors
// SPDX-License-Identifier: MIT

use cheriot_compose::diagnostic::{Diagnostic, Result};

use super::command_name::CommandName;
use super::option_catalog::{self, OptionDescription};

const LINE_WIDTH: usize = 80;

pub(super) fn general_help() -> String {
    let mut text = String::from(
        "Usage: cheriot-compose COMMAND [OPTION]...\n\n\
         Inspect and build CHERIoT firmware from TOML configuration.\n\n\
         Commands:\n",
    );
    let mut commands: Vec<_> = option_catalog::ALL
        .iter()
        .map(|command| (command.name(), command.signature(), command.description()))
        .collect();
    commands.sort_unstable_by_key(|(name, _, _)| *name);
    for (index, (_, signature, description)) in commands.iter().enumerate() {
        if index > 0 {
            text.push('\n');
        }
        append_entry(&mut text, signature, description);
    }
    text.push_str(
        "\nOptions follow the command. Available options and formats vary by command.\n\
         Use cheriot-compose COMMAND --help for its complete usage and restrictions.\n\
         Compiler probes remain unresolved until a build or an explicit probe.\n\n\
         Common options:\n",
    );
    append_options(
        &mut text,
        option_catalog::OPTIONS
            .iter()
            .filter(|option| option.commands.len() >= 5 || option.name == "--help")
            .collect(),
    );
    text
}

pub(super) fn command_help(command: &str) -> Result<String> {
    let selected = CommandName::parse(command).ok_or_else(|| {
        Diagnostic::new(format!(
            "unknown command {command}; use cheriot-compose --help"
        ))
    })?;
    let trailing = if selected == CommandName::Run {
        " [-- RUNNER-ARGS...]"
    } else {
        ""
    };
    let mut text = format!(
        "Usage: cheriot-compose {} [OPTION]...{trailing}\n\n",
        selected.signature()
    );
    append_wrapped(&mut text, selected.description(), "");
    let formats = option_catalog::formats(selected);
    if !formats.supported.is_empty() {
        let supported = formats
            .supported
            .iter()
            .map(|format| format.name())
            .collect::<Vec<_>>()
            .join(", ");
        text.push_str("\nFormats:\n");
        append_wrapped(&mut text, &format!("Accepted: {supported}."), "      ");
        append_wrapped(
            &mut text,
            &format!("Default: {}.", formats.default.name()),
            "      ",
        );
    }
    let mut notes = Vec::new();
    if option_catalog::accepts(selected, "--verbose") {
        notes.push("--verbose and --quiet cannot be combined.");
    }
    if matches!(selected, CommandName::Build | CommandName::Package) {
        notes.push("--output requires --format json.");
        notes.push("JSON output cannot be combined with --verbose or --quiet.");
    }
    if selected == CommandName::Audit {
        notes.push("--output cannot be combined with --dry-run.");
    }
    if selected == CommandName::CompareCompilationDatabase {
        notes.push("--generated compares a file and cannot be combined with project options.");
    }
    if selected == CommandName::Run {
        notes.push("Arguments after -- are passed to the configured runner.");
    }
    if !notes.is_empty() {
        text.push_str("\nNotes:\n");
        for note in notes {
            append_wrapped(&mut text, note, "      ");
        }
    }
    text.push_str("\nOptions:\n");
    append_options(
        &mut text,
        option_catalog::OPTIONS
            .iter()
            .filter(|option| option_catalog::accepts(selected, option.name))
            .collect(),
    );
    Ok(text)
}

fn append_entry(text: &mut String, signature: &str, description: &str) {
    text.push_str("  ");
    text.push_str(signature);
    text.push('\n');
    append_wrapped(text, description, "    ");
}

fn append_options(text: &mut String, mut options: Vec<&OptionDescription>) {
    options.sort_unstable_by_key(|option| {
        (option.name == "--help", option.name.trim_start_matches('-'))
    });
    for (index, option) in options.iter().enumerate() {
        if index > 0 {
            text.push('\n');
        }
        append_entry(text, &option.label(), option.description);
    }
}

fn append_wrapped(text: &mut String, description: &str, indentation: &str) {
    text.push_str(indentation);
    let mut width = indentation.len();
    for word in description.split_whitespace() {
        if width > indentation.len() {
            if width + 1 + word.len() > LINE_WIDTH {
                text.push('\n');
                text.push_str(indentation);
                width = indentation.len();
            } else {
                text.push(' ');
                width += 1;
            }
        }
        text.push_str(word);
        width += word.len();
    }
    text.push('\n');
}
