// Copyright 3Bian Limited and Contributors
// SPDX-License-Identifier: MIT

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use super::{ConfigurationValue, ConfigurationValueKind, canonicalize_path, read_toml_document};
use crate::diagnostic::{Diagnostic, Origin, Result};

mod board_directories;
pub(super) use board_directories::BoardDirectories;

pub(super) struct BoardPaths<'a> {
    pub project: &'a Path,
    pub sdk: &'a Path,
    pub directories: &'a BoardDirectories,
}

impl BoardPaths<'_> {
    pub fn resolve(&self, value: &str, origin: &Origin) -> Result<PathBuf> {
        let value = value
            .replace("${board}", &self.directories.selected.to_string_lossy())
            .replace("${boardBase}", &self.directories.base.to_string_lossy());
        crate::configuration::resolution::resolve_path_value(&value, origin, self.project, self.sdk)
    }
}

pub(super) fn apply_board_overlays(
    board: &mut ConfigurationValue,
    overlays: &[String],
    origins: &BTreeMap<String, Origin>,
    files: &mut Vec<PathBuf>,
    paths: &BoardPaths<'_>,
) -> Result<()> {
    for (index, overlay) in overlays.iter().enumerate() {
        let source = origins
            .get(&format!("/board_overlays/{index}"))
            .cloned()
            .unwrap_or_else(|| Origin::derived("board_overlays"));
        let path = paths.resolve(overlay, &source)?;
        let path = canonicalize_path(&path).map_err(|error| error.at(source.clone()))?;
        let document = read_toml_document(&path)?;
        let fields = document.map()?;
        for (name, value) in fields {
            if name != "patches" {
                return Err(
                    Diagnostic::new(format!("unknown board overlay field {name}"))
                        .at(value.origin.clone()),
                );
            }
        }
        let patches = fields.get("patches").ok_or_else(|| {
            Diagnostic::new("board overlay requires patches").at(document.origin.clone())
        })?;
        if let ConfigurationValueKind::Sequence(values) = &patches.kind {
            for value in values {
                super::patches::apply_patch(board, value)?;
            }
        } else {
            return Err(Diagnostic::new("patches must be a sequence").at(patches.origin.clone()));
        }
        if !files.contains(&path) {
            files.push(path);
        }
    }
    Ok(())
}

fn merge(base: &mut ConfigurationValue, over: ConfigurationValue) {
    match (&mut base.kind, over.kind) {
        (
            ConfigurationValueKind::Mapping(base_fields),
            ConfigurationValueKind::Mapping(overlay_fields),
        ) => {
            for (key, value) in overlay_fields {
                if let Some(existing) = base_fields.get_mut(&key) {
                    merge(existing, value)
                } else {
                    base_fields.insert(key, value);
                }
            }
        }
        (_, kind) => {
            base.kind = kind;
            base.origin = over.origin;
        }
    }
}

pub(super) fn load_board(
    path: &Path,
    stack: &mut BTreeSet<PathBuf>,
    files: &mut Vec<PathBuf>,
    project: &Path,
    sdk: &Path,
    selected_directory: &Path,
) -> Result<(ConfigurationValue, PathBuf)> {
    let path = canonicalize_path(path)?;
    if !stack.insert(path.clone()) {
        return Err(Diagnostic::new(format!(
            "board inheritance cycle at {}",
            path.display()
        )));
    }
    files.push(path.clone());
    let mut node = read_toml_document(&path)?;
    let parent = node.map_mut()?.remove("extends");
    let patches = node.map_mut()?.remove("patches");
    let (mut base, base_directory) = if let Some(parent_board) = parent {
        let parent_value = parent_board
            .string()?
            .replace("${board}", &selected_directory.to_string_lossy());
        let base_path = crate::configuration::resolution::resolve_path_value(
            &parent_value,
            &parent_board.origin,
            project,
            sdk,
        )?;
        let (mut base_board, base_directory) =
            load_board(&base_path, stack, files, project, sdk, selected_directory).map_err(
                |error| {
                    if error.origin.is_none() {
                        error.at(parent_board.origin.clone())
                    } else {
                        error
                    }
                },
            )?;
        merge(&mut base_board, node);
        (base_board, base_directory)
    } else {
        let directory = path
            .parent()
            .ok_or_else(|| Diagnostic::new("board file needs a directory"))?
            .to_path_buf();
        (node, directory)
    };
    if let Some(patches) = patches {
        if let ConfigurationValueKind::Sequence(values) = &patches.kind {
            for patch_node in values {
                super::patches::apply_patch(&mut base, patch_node)?
            }
        } else {
            return Err(Diagnostic::new("patches must be a sequence").at(patches.origin));
        }
    }
    stack.remove(&path);
    Ok((base, base_directory))
}
