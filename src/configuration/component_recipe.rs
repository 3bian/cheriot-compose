// Copyright 3Bian Limited and Contributors
// SPDX-License-Identifier: MIT

use std::path::PathBuf;

use serde::Serialize;

use super::{CompilerFlag, ComponentRole, ForcedInclude, ResolvedSource, SdkComponentKind};

#[derive(Clone, Debug, Serialize)]
pub struct ComponentRecipe {
    pub name: String,
    pub kind: SdkComponentKind,
    pub role: Option<ComponentRole>,
    pub sources: Vec<ResolvedSource>,
    pub compiler: Option<crate::diagnostic::Sourced<String>>,
    pub dependencies: Vec<String>,
    pub flags: Vec<CompilerFlag>,
    pub c_flags: Vec<CompilerFlag>,
    pub cpp_flags: Vec<CompilerFlag>,
    pub assembly_flags: Vec<CompilerFlag>,
    pub rust_flags: Vec<CompilerFlag>,
    pub cargo_flags: Vec<CompilerFlag>,
    pub includes: Vec<crate::diagnostic::Sourced<PathBuf>>,
    pub force_includes: Vec<ForcedInclude>,
    pub compartment: Option<String>,
    pub script: String,
}

impl ComponentRecipe {
    pub fn uses_bare_metal_abi(&self) -> bool {
        matches!(
            self.role,
            Some(ComponentRole::Loader | ComponentRole::Switcher)
        )
    }

    pub fn needs_subobject_bounds_probe(&self) -> bool {
        matches!(
            self.role,
            Some(ComponentRole::Allocator | ComponentRole::Loader | ComponentRole::Scheduler)
        )
    }
}
