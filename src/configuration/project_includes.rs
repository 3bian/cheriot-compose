// Copyright 3Bian Limited and Contributors
// SPDX-License-Identifier: MIT

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use super::{ConfigurationValue, canonicalize_path, deserialize_configuration, read_toml_document};
use crate::diagnostic::{Diagnostic, Result};

pub(super) fn load_included_project(
    path: &Path,
    stack: &mut BTreeSet<PathBuf>,
    files: &mut Vec<PathBuf>,
) -> Result<ConfigurationValue> {
    let path = canonicalize_path(path)?;
    if !stack.insert(path.clone()) {
        return Err(Diagnostic::new(format!(
            "project include cycle at {}",
            path.display()
        )));
    }
    if !files.contains(&path) {
        files.push(path.clone());
    }
    let mut node = read_toml_document(&path)?;
    if let Some(includes) = node.map_mut()?.remove("include") {
        let paths: Vec<String> = deserialize_configuration(&includes)?;
        for included_path in paths {
            let included_project = load_included_project(
                &path.parent().unwrap_or(Path::new("/")).join(included_path),
                stack,
                files,
            )
            .map_err(|error| {
                if error.origin.is_none() {
                    error.at(includes.origin.clone())
                } else {
                    error
                }
            })?;
            for (key, value) in included_project.map()? {
                if ["toolchain_dir", "build_dir", "output_dir"].contains(&key.as_str()) {
                    let map = node.map_mut()?;
                    if let Some(previous) = map.get(key) {
                        if previous.origin.file != path.display().to_string()
                            && previous.origin.file != value.origin.file
                        {
                            return Err(Diagnostic::new(format!(
                                "conflicting included {key} declarations; select one in the project"
                            ))
                            .at(value.origin.clone()));
                        }
                    } else {
                        map.insert(key.clone(), value.clone());
                    }
                    continue;
                }
                if key == "sdk_compatibility" {
                    let map = node.map_mut()?;
                    if let Some(previous) = map.get(key) {
                        if previous.origin.file != value.origin.file
                            || previous.origin.field != value.origin.field
                        {
                            return Err(Diagnostic::new("duplicate SDK compatibility declaration")
                                .at(value.origin.clone()));
                        }
                    } else {
                        map.insert(key.clone(), value.clone());
                    }
                    continue;
                }
                if !["services", "libraries", "sdk_components", "generators"]
                    .contains(&key.as_str())
                {
                    return Err(Diagnostic::new(
                        "included files may define only includes, directory settings, SDK compatibility, components and generators",
                    )
                    .at(value.origin.clone()));
                }
                let map = node.map_mut()?;
                if let Some(existing) = map.get_mut(key) {
                    for (name, item) in value.map()? {
                        if let Some(previous) = existing.map()?.get(name) {
                            if previous.origin.file == item.origin.file
                                && previous.origin.field == item.origin.field
                            {
                                continue;
                            }
                            return Err(Diagnostic::new(format!(
                                "duplicate included definition {name}"
                            ))
                            .at(item.origin.clone()));
                        }
                        existing.map_mut()?.insert(name.clone(), item.clone());
                    }
                } else {
                    map.insert(key.clone(), value.clone());
                }
            }
        }
    }
    stack.remove(&path);
    Ok(node)
}
