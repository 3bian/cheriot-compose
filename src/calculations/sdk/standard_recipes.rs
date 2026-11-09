// Copyright 3Bian Limited and Contributors
// SPDX-License-Identifier: MIT

use std::collections::BTreeMap;

use super::{ComponentRecipe, SdkComponentKind};
use crate::configuration::{ComponentRole, ResolvedProject};

pub(super) fn standard_recipes(project: &ResolvedProject) -> BTreeMap<String, ComponentRecipe> {
    let mut recipes = BTreeMap::new();
    let mut add = |name: &str,
                   kind: SdkComponentKind,
                   directory: &str,
                   files: &[&str],
                   dependencies: &[&str],
                   compartment: Option<&str>,
                   role: ComponentRole| {
        let script = match kind {
            SdkComponentKind::Library => "library.ldscript",
            SdkComponentKind::PrivilegedCompartment | SdkComponentKind::PrivilegedLibrary => {
                "privileged-compartment.ldscript"
            }
            SdkComponentKind::Loader => "core/loader/loader.ldscript",
            _ => "compartment.ldscript",
        };
        recipes.insert(
            name.to_string(),
            ComponentRecipe {
                name: name.into(),
                kind,
                role: Some(role),
                sources: files
                    .iter()
                    .map(|file| {
                        crate::configuration::ResolvedSource::new(
                            project.sdk.join(directory).join(file),
                        )
                    })
                    .collect(),
                compiler: None,
                dependencies: dependencies
                    .iter()
                    .map(|dependency| dependency.to_string())
                    .collect(),
                flags: Vec::new(),
                c_flags: Vec::new(),
                cpp_flags: Vec::new(),
                assembly_flags: Vec::new(),
                rust_flags: Vec::new(),
                cargo_flags: Vec::new(),
                includes: Vec::new(),
                force_includes: Vec::new(),
                compartment: compartment.map(str::to_string),
                script: script.into(),
            },
        );
    };
    add(
        "cheriot.allocator",
        SdkComponentKind::PrivilegedCompartment,
        "core/allocator",
        &["main.cc"],
        &["locks", "compartment_helpers"],
        Some("allocator"),
        ComponentRole::Allocator,
    );
    add(
        "cheriot.token_library",
        SdkComponentKind::PrivilegedLibrary,
        "core/token_library",
        &["token_unseal.S"],
        &[],
        None,
        ComponentRole::TokenLibrary,
    );
    add(
        "cheriot.software_revoker",
        SdkComponentKind::PrivilegedCompartment,
        "core/software_revoker",
        &["revoker.cc"],
        &[],
        Some("software_revoker"),
        ComponentRole::SoftwareRevoker,
    );
    add(
        "cheriot.switcher",
        SdkComponentKind::Object,
        "core/switcher",
        &["entry.S"],
        &[],
        None,
        ComponentRole::Switcher,
    );
    add(
        "cheriot.loader",
        SdkComponentKind::Loader,
        "core/loader",
        &["boot.S", "boot.cc"],
        &[],
        None,
        ComponentRole::Loader,
    );
    add(
        &format!("{}.scheduler", project.firmware.name),
        SdkComponentKind::PrivilegedCompartment,
        "core/scheduler",
        &["main.cc"],
        &["locks", "crt", "atomic1", "compartment_helpers"],
        Some("scheduler"),
        ComponentRole::Scheduler,
    );
    recipes
}
