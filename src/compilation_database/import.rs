// Copyright 3Bian Limited and Contributors
// SPDX-License-Identifier: MIT

use std::{fs, path::Path};

use super::CompilationEntry;
use crate::diagnostic::{Diagnostic, Result};

pub fn split_command_arguments(text: &str) -> Result<Vec<String>> {
    let mut result = Vec::new();
    let mut word = String::new();
    let mut started = false;
    let mut quote = None;
    let mut chars = text.chars().peekable();
    while let Some(character) = chars.next() {
        if quote == Some('\'') {
            if character == '\'' {
                quote = None
            } else {
                word.push(character)
            }
            continue;
        }
        if character == '\\' {
            let next_character = chars
                .next()
                .ok_or_else(|| Diagnostic::new("trailing backslash in command"))?;
            if next_character == '\n' {
                continue;
            }
            if quote == Some('"') && !['$', '`', '"', '\\'].contains(&next_character) {
                word.push('\\')
            }
            word.push(next_character);
            started = true;
            continue;
        }
        if quote == Some('"') {
            if character == '"' {
                quote = None
            } else if character == '$' || character == '`' {
                return Err(Diagnostic::new(
                    "shell expansion in command is unsupported; use arguments",
                ));
            } else {
                word.push(character)
            }
            continue;
        }
        if character == '\'' || character == '"' {
            quote = Some(character);
            started = true;
        } else if character.is_whitespace() {
            if started {
                result.push(std::mem::take(&mut word));
                started = false;
            }
        } else if "|;&<>`$()".contains(character) {
            return Err(Diagnostic::new(
                "shell operators/expansion are unsupported in command; use arguments",
            ));
        } else {
            word.push(character);
            started = true;
        }
    }
    if quote.is_some() {
        return Err(Diagnostic::new("unclosed quote in compilation command"));
    }
    if started {
        result.push(word)
    }
    if result.is_empty() || result[0].contains('=') {
        return Err(Diagnostic::new(
            "compilation command must begin with an executable",
        ));
    }
    Ok(result)
}

pub fn read_compilation_database(path: &Path) -> Result<Vec<CompilationEntry>> {
    let text = fs::read_to_string(path).map_err(|error| Diagnostic::io(path, error))?;
    let mut entries: Vec<CompilationEntry> = serde_json::from_str(&text)?;
    for entry in &mut entries {
        if entry.arguments.is_empty() {
            entry.arguments = split_command_arguments(
                entry
                    .command
                    .as_deref()
                    .ok_or_else(|| Diagnostic::new("compdb entry needs arguments or command"))?,
            )?;
        }
        if entry
            .arguments
            .iter()
            .any(|argument| argument.contains('\0'))
        {
            return Err(Diagnostic::new("NUL in compilation arguments"));
        }
        if entry.output.is_none() {
            entry.output = infer_output(&entry.arguments);
        }
    }
    Ok(entries)
}

fn infer_output(arguments: &[String]) -> Option<String> {
    arguments
        .windows(2)
        .find(|window| window[0] == "-o")
        .map(|window| window[1].clone())
        .or_else(|| {
            arguments.iter().find_map(|argument| {
                argument
                    .strip_prefix("-o")
                    .filter(|item| !item.is_empty())
                    .map(str::to_string)
            })
        })
}
