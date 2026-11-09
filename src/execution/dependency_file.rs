// Copyright 3Bian Limited and Contributors
// SPDX-License-Identifier: MIT

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use crate::diagnostic::{Diagnostic, Result};

pub fn parse_dependency_file(text: &str, cwd: &Path) -> Result<Vec<PathBuf>> {
    let joined = text.replace("\\\r\n", "").replace("\\\n", "");
    let mut dependencies = BTreeSet::new();
    for line in joined.lines() {
        let mut escaped = false;
        let mut colon = None;
        for (index, character) in line.char_indices() {
            if escaped {
                escaped = false;
                continue;
            }
            if character == '\\' {
                escaped = true;
            } else if character == ':' {
                colon = Some(index);
                break;
            }
        }
        let Some(colon_index) = colon else {
            if line.trim().is_empty() {
                continue;
            }
            return Err(Diagnostic::new("malformed depfile: missing target colon"));
        };
        let mut chars = line[colon_index + 1..].chars().peekable();
        let mut word = String::new();
        let mut words = Vec::new();
        while let Some(character) = chars.next() {
            match character {
                '\\' => word.push(
                    chars
                        .next()
                        .ok_or_else(|| Diagnostic::new("trailing depfile escape"))?,
                ),
                '#' => break,
                '$' if chars.peek() == Some(&'$') => {
                    chars.next();
                    word.push('$');
                }
                c if c.is_whitespace() => {
                    if !word.is_empty() {
                        words.push(std::mem::take(&mut word));
                    }
                }
                _ => word.push(character),
            }
        }
        if !word.is_empty() {
            words.push(word)
        }
        for word in words {
            dependencies.insert(crate::configuration::resolution::normalize_path(
                &cwd.join(word),
            ));
        }
    }
    Ok(dependencies.into_iter().collect())
}
