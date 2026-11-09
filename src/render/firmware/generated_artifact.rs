// Copyright 3Bian Limited and Contributors
// SPDX-License-Identifier: MIT

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

use serde::Serialize;

use crate::{
    diagnostic::{Origin, Result},
    render::template::{self, EscapeMode},
};

#[derive(Clone)]
pub struct GeneratedArtifact {
    pub name: String,
    pub path: PathBuf,
    pub content: String,
    pub inputs: Vec<PathBuf>,
    pub template_dependencies: BTreeMap<String, String>,
    pub provenance: Vec<Origin>,
}

impl GeneratedArtifact {
    pub fn literal(
        name: &str,
        path: PathBuf,
        content: String,
        inputs: Vec<PathBuf>,
        provenance: Vec<Origin>,
    ) -> Self {
        Self {
            name: name.into(),
            path,
            content,
            inputs,
            template_dependencies: BTreeMap::new(),
            provenance,
        }
    }

    pub fn render<T: Serialize>(
        name: &str,
        path: PathBuf,
        template: &str,
        model: &T,
        partials: &BTreeMap<String, String>,
        inputs: Vec<PathBuf>,
        provenance: Vec<Origin>,
    ) -> Result<(Self, BTreeSet<String>)> {
        let rendered = template::render_tracked(
            template,
            &serde_json::to_value(model)?,
            partials,
            EscapeMode::Text,
        )?;
        let mut template_dependencies = rendered.partials;
        template_dependencies.insert(format!("template:{name}"), template.into());
        Ok((
            Self {
                name: name.into(),
                path,
                content: rendered.content,
                inputs,
                template_dependencies,
                provenance,
            },
            rendered.variables,
        ))
    }

    pub fn template_fingerprint(&self) -> Result<String> {
        Ok(crate::build_graph::calculate_fingerprint(
            &serde_json::to_vec(&self.template_dependencies)?,
        ))
    }
}
