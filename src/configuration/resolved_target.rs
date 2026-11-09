// Copyright 3Bian Limited and Contributors
// SPDX-License-Identifier: MIT

use std::collections::BTreeMap;
use std::path::PathBuf;

use serde::Serialize;

use super::{CompilerFlag, ComponentRole, ForcedInclude, ResolvedSource, TargetKind};

#[derive(Clone, Debug, Serialize)]
pub struct ResolvedTarget {
    pub kind: TargetKind,
    pub role: Option<ComponentRole>,
    pub sources: Vec<ResolvedSource>,
    pub compiler: Option<crate::diagnostic::Sourced<String>>,
    pub includes: Vec<crate::diagnostic::Sourced<PathBuf>>,
    pub force_includes: Vec<ForcedInclude>,
    pub flags: Vec<CompilerFlag>,
    pub c_flags: Vec<CompilerFlag>,
    pub cpp_flags: Vec<CompilerFlag>,
    pub assembly_flags: Vec<CompilerFlag>,
    pub rust_flags: Vec<CompilerFlag>,
    pub cargo_flags: Vec<CompilerFlag>,
    pub dependencies: Vec<String>,
    pub shared_objects: BTreeMap<String, u32>,
}
