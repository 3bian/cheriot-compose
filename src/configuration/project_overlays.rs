// Copyright 3Bian Limited and Contributors
// SPDX-License-Identifier: MIT

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use super::{
    ConfigurationValue, ConfigurationValueKind, canonicalize_path, deserialize_configuration,
    patches, read_toml_document,
};
use crate::diagnostic::{Diagnostic, Result};

pub(super) fn apply_overlays(
    project: &mut ConfigurationValue,
    overlays: &[PathBuf],
    files: &mut Vec<PathBuf>,
) -> Result<()> {
    for overlay in overlays {
        apply_overlay(project, overlay, &mut BTreeSet::new(), files)?;
    }
    Ok(())
}

fn apply_overlay(
    project: &mut ConfigurationValue,
    path: &Path,
    stack: &mut BTreeSet<PathBuf>,
    files: &mut Vec<PathBuf>,
) -> Result<()> {
    let path = canonicalize_path(path)?;
    if !stack.insert(path.clone()) {
        return Err(Diagnostic::new(format!(
            "project overlay include cycle at {}",
            path.display()
        )));
    }
    let document = read_toml_document(&path)?;
    let fields = document.map()?;
    for (name, value) in fields {
        if name != "include" && name != "patches" {
            return Err(
                Diagnostic::new(format!("unknown project overlay field {name}"))
                    .at(value.origin.clone()),
            );
        }
    }
    if let Some(includes) = fields.get("include") {
        let paths: Vec<String> = deserialize_configuration(includes)?;
        let directory = path
            .parent()
            .ok_or_else(|| Diagnostic::new("overlay needs a directory"))?;
        for included in paths {
            apply_overlay(project, &directory.join(included), stack, files).map_err(|error| {
                if error.origin.is_none() {
                    error.at(includes.origin.clone())
                } else {
                    error
                }
            })?;
        }
    }
    if let Some(patches) = fields.get("patches") {
        let ConfigurationValueKind::Sequence(patches) = &patches.kind else {
            return Err(
                Diagnostic::new("overlay patches must be an array").at(patches.origin.clone())
            );
        };
        for patch in patches {
            patches::apply_patch(project, patch)?;
        }
    }
    if !files.contains(&path) {
        files.push(path.clone());
    }
    stack.remove(&path);
    Ok(())
}
