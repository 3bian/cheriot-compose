// Copyright 3Bian Limited and Contributors
// SPDX-License-Identifier: MIT

use super::{ConfigurationValue, ConfigurationValueKind};
use crate::diagnostic::{Diagnostic, Result};

pub(super) fn apply_patch(
    root: &mut ConfigurationValue,
    board_patch: &ConfigurationValue,
) -> Result<()> {
    let patch_fields = board_patch.map()?;
    for key in patch_fields.keys() {
        if !["op", "path", "value"].contains(&key.as_str()) {
            return Err(Diagnostic::new(format!("unknown patch field {key}"))
                .at(board_patch.origin.clone()));
        }
    }
    let get_field = |key: &str| {
        patch_fields.get(key).ok_or_else(|| {
            Diagnostic::new(format!("patch needs {key}")).at(board_patch.origin.clone())
        })
    };
    let operation = get_field("op")?.string()?;
    let path = get_field("path")?.string()?;
    if !path.starts_with('/') || path == "/" {
        return Err(
            Diagnostic::new("patch path must be a nonempty JSON pointer")
                .at(board_patch.origin.clone()),
        );
    }
    let mut parts = Vec::new();
    for segment in path[1..].split('/') {
        let mut characters = segment.chars();
        let mut decoded_segment = String::new();
        while let Some(character) = characters.next() {
            if character == '~' {
                decoded_segment.push(match characters.next() {
                    Some('0') => '~',
                    Some('1') => '/',
                    _ => {
                        return Err(Diagnostic::new("invalid JSON pointer escape")
                            .at(board_patch.origin.clone()));
                    }
                })
            } else {
                decoded_segment.push(character)
            }
        }
        parts.push(decoded_segment);
    }
    let (last, parents) = parts
        .split_last()
        .ok_or_else(|| Diagnostic::new("empty patch"))?;
    let invalid_path = || {
        Diagnostic::new(format!("invalid {operation} patch path {path}"))
            .at(board_patch.origin.clone())
    };
    let mut node = root;
    for key in parents {
        node = match &mut node.kind {
            ConfigurationValueKind::Mapping(mapping) => {
                mapping.get_mut(key).ok_or_else(invalid_path)?
            }
            ConfigurationValueKind::Sequence(values) => values
                .get_mut(index(key).ok_or_else(invalid_path)?)
                .ok_or_else(invalid_path)?,
            _ => return Err(invalid_path()),
        };
    }
    let value = if operation == "add" || operation == "replace" {
        Some(get_field("value")?.clone())
    } else {
        None
    };
    match (&mut node.kind, operation.as_str()) {
        (ConfigurationValueKind::Mapping(mapping), "remove") => {
            mapping.remove(last).ok_or_else(invalid_path)?;
        }
        (ConfigurationValueKind::Mapping(mapping), "add" | "replace") => {
            if operation == "replace" && !mapping.contains_key(last) {
                return Err(invalid_path());
            }
            mapping.insert(last.clone(), value.ok_or_else(invalid_path)?);
        }
        (ConfigurationValueKind::Sequence(values), "add") => {
            let index = if last == "-" {
                values.len()
            } else {
                index(last).ok_or_else(invalid_path)?
            };
            if index > values.len() {
                return Err(invalid_path());
            }
            values.insert(index, value.ok_or_else(invalid_path)?);
        }
        (ConfigurationValueKind::Sequence(values), "remove" | "replace") => {
            let item_index = index(last).ok_or_else(invalid_path)?;
            if item_index >= values.len() {
                return Err(invalid_path());
            }
            if operation == "remove" {
                values.remove(item_index);
            } else {
                values[item_index] = value.ok_or_else(invalid_path)?;
            }
        }
        _ => return Err(invalid_path()),
    }
    Ok(())
}

fn index(text: &str) -> Option<usize> {
    if text.len() > 1 && text.starts_with('0') {
        None
    } else {
        text.parse().ok()
    }
}
