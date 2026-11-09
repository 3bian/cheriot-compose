// Copyright 3Bian Limited and Contributors
// SPDX-License-Identifier: MIT

use std::collections::BTreeMap;
use std::path::PathBuf;

pub(super) struct ComponentOutputs {
    pub(super) objects: BTreeMap<String, Vec<PathBuf>>,
    pub(super) producers: BTreeMap<String, Vec<String>>,
}

pub(crate) fn component_output(
    project: &crate::configuration::ResolvedProject,
    recipe: &crate::configuration::ComponentRecipe,
) -> PathBuf {
    use crate::configuration::SdkComponentKind;
    project.build_dir.join(format!(
        "{}--{}.{}",
        recipe.name,
        project.configuration_identity,
        match recipe.kind {
            SdkComponentKind::Library | SdkComponentKind::PrivilegedLibrary => "library",
            SdkComponentKind::Loader => "loader.o",
            SdkComponentKind::Object => "o",
            _ => "compartment",
        }
    ))
}
