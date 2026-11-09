// Copyright 3Bian Limited and Contributors
// SPDX-License-Identifier: MIT

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use super::resolve_path_value;
use crate::configuration::RawProject;
use crate::diagnostic::{Diagnostic, Origin, Result};

pub(crate) fn load_test_suite_sources(
    raw: &RawProject,
    root: &Path,
    sdk: &Path,
    origins: &BTreeMap<String, Origin>,
) -> Vec<Option<Result<PathBuf>>> {
    let Some(suite) = &raw.test_suite else {
        return Vec::new();
    };
    suite
        .cases
        .iter()
        .enumerate()
        .map(|(index, case)| {
            case.firmware_rust_source.as_ref().map(|value| {
                let field = format!("/test_suite/cases/{index}/firmware_rust_source");
                let origin = origins
                    .get(&field)
                    .cloned()
                    .unwrap_or_else(|| Origin::derived(&field));
                let source = resolve_path_value(value, &origin, root, sdk)?;
                if !source.is_file() {
                    return Err(Diagnostic::new(format!(
                        "Rust test source does not exist: {}",
                        source.display()
                    ))
                    .at(origin));
                }
                Ok(source)
            })
        })
        .collect()
}
