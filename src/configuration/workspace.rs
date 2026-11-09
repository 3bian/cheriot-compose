// Copyright 3Bian Limited and Contributors
// SPDX-License-Identifier: MIT

use std::collections::BTreeMap;
use std::path::{Component, Path, PathBuf};

use serde::Deserialize;

use super::{canonicalize_path, deserialize_configuration, read_toml_document};
use crate::diagnostic::{Diagnostic, Origin, Result};

pub struct Workspace {
    pub projects: BTreeMap<String, PathBuf>,
}

pub fn read_workspace(path: &Path) -> Result<Option<Workspace>> {
    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct WorkspaceDocument {
        workspace: WorkspaceProjects,
    }

    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct WorkspaceProjects {
        projects: BTreeMap<String, String>,
    }

    let path = canonicalize_path(path)?;
    let document = read_toml_document(&path)?;
    if !document.map()?.contains_key("workspace") {
        return Ok(None);
    }
    let raw: WorkspaceDocument = deserialize_configuration(&document)?;
    if raw.workspace.projects.is_empty() {
        return Err(Diagnostic::new("workspace must name at least one project").at(document.origin));
    }
    let root = path
        .parent()
        .ok_or_else(|| Diagnostic::new("workspace needs a directory"))?;
    let mut origins = BTreeMap::<String, Origin>::new();
    document.origins("", &mut origins);
    let mut projects = BTreeMap::new();
    for (name, value) in raw.workspace.projects {
        let origin = origins
            .get(&format!("/workspace/projects/{name}"))
            .cloned()
            .unwrap_or_else(|| document.origin.clone());
        let relative = Path::new(&value);
        if !super::resolution::is_target_name(&name)
            || relative.is_absolute()
            || relative.components().any(|part| {
                matches!(
                    part,
                    Component::ParentDir | Component::RootDir | Component::Prefix(_)
                )
            })
            || relative
                .extension()
                .and_then(|extension| extension.to_str())
                != Some("toml")
        {
            return Err(Diagnostic::new("workspace project names must be valid and paths must be relative TOML files within the workspace").at(origin));
        }
        let project = canonicalize_path(&root.join(relative)).map_err(|_| {
            Diagnostic::new(format!("workspace project {name} is missing: {value}; copy the complete RTOS overlay into the checkout"))
                .at(origin.clone())
        })?;
        if !project.starts_with(root) || project == path || !project.is_file() {
            return Err(Diagnostic::new(
                "workspace project must be a different file inside the workspace",
            )
            .at(origin));
        }
        projects.insert(name, project);
    }
    Ok(Some(Workspace { projects }))
}

pub fn select_project(path: &Path, name: Option<&str>) -> Result<PathBuf> {
    match (read_workspace(path)?, name) {
        (Some(workspace), Some(name)) => workspace.projects.get(name).cloned().ok_or_else(|| {
            Diagnostic::new(format!(
                "unknown workspace project {name}; available projects: {}",
                workspace
                    .projects
                    .keys()
                    .cloned()
                    .collect::<Vec<_>>()
                    .join(", ")
            ))
        }),
        (Some(workspace), None) => Err(Diagnostic::new(format!(
            "select a workspace project with --project; available projects: {}",
            workspace
                .projects
                .keys()
                .cloned()
                .collect::<Vec<_>>()
                .join(", ")
        ))),
        (None, Some(_)) => Err(Diagnostic::new(
            "--project requires a workspace configuration",
        )),
        (None, None) => Ok(path.to_path_buf()),
    }
}
