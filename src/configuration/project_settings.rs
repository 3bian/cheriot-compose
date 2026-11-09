// Copyright 3Bian Limited and Contributors
// SPDX-License-Identifier: MIT

use std::path::PathBuf;

use crate::diagnostic::Origin;

#[derive(Clone, Debug, Default)]
pub struct ProjectSettings {
    pub overlays: Vec<PathBuf>,
    pub board: Option<(String, Origin)>,
    pub mode: Option<(String, Origin)>,
    pub toolchain_directory: Option<(PathBuf, Origin)>,
    pub build_directory: Option<(PathBuf, Origin)>,
    pub output_directory: Option<(PathBuf, Origin)>,
    pub rust_compiler: Option<(String, Origin)>,
    pub rust_flags: Option<(Vec<String>, Origin)>,
}
