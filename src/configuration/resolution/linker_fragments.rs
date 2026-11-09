// Copyright 3Bian Limited and Contributors
// SPDX-License-Identifier: MIT

use std::collections::{BTreeMap, BTreeSet};

use super::{is_identifier, is_target_name};
use crate::configuration::board_inheritance::BoardPaths;
use crate::configuration::{LinkerFragmentSource, RawLinkerFragment, ResolvedLinkerFragment};
use crate::diagnostic::{Diagnostic, Origin, Result};

pub(super) fn resolve_linker_fragments(
    fragments: Vec<RawLinkerFragment>,
    paths: &BoardPaths<'_>,
    origins: &BTreeMap<String, Origin>,
) -> Result<Vec<ResolvedLinkerFragment>> {
    let mut names = BTreeSet::new();
    fragments.into_iter().enumerate().map(|(index, fragment)| {
        let prefix = format!("/board/linker_fragments/{index}");
        let at = |field: &str| origins.get(&format!("{prefix}/{field}"))
            .or_else(|| origins.get(&prefix)).cloned().unwrap_or_else(|| Origin::derived(&prefix));
        let fail = |field: &str, message: &str| Diagnostic::new(message).at(at(field));
        let program_header = fragment.program_header.unwrap_or_else(|| "LOAD".into());
        if !is_identifier(&program_header) {
            return Err(fail("program_header", "program header must be an identifier"));
        }
        let start_expression = fragment.start_expression.unwrap_or_else(|| format!("0x{:x}", fragment.start));
        if start_expression.is_empty() || !start_expression.bytes().all(|byte| {
            byte.is_ascii_alphanumeric() || b"_ .()+-*/&|~<>".contains(&byte)
        }) {
            return Err(fail("start_expression", "invalid linker start expression"));
        }
        let source = match (fragment.body, fragment.path, fragment.generated_name) {
            (Some(body), None, None) if !body.contains('\0') => LinkerFragmentSource::Inline { body },
            (None, Some(path), generated_name) => {
                let path = paths.resolve(&path, &at("path"))?;
                if !path.is_file() {
                    return Err(fail("path", &format!("linker fragment does not exist: {}", path.display())));
                }
                if let Some(name) = generated_name {
                    if !is_target_name(&name) || !names.insert(name.clone()) {
                        return Err(fail("generated_name", "generated linker fragment name must be unique and contain only letters, numbers, dots, dashes or underscores"));
                    }
                    LinkerFragmentSource::Generated { template: path, name }
                } else {
                    LinkerFragmentSource::File { path }
                }
            }
            _ => return Err(fail("", "linker fragment needs either body or path; generated_name requires path")),
        };
        Ok(ResolvedLinkerFragment {
            start: fragment.start,
            start_expression,
            program_header,
            source,
            origin: at(""),
        })
    }).collect()
}
