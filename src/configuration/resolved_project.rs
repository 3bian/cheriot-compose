// Copyright 3Bian Limited and Contributors
// SPDX-License-Identifier: MIT

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

use serde::Serialize;

use super::ComponentRecipe;
use super::{
    AuditBoard, BuildMode, ResolvedBoard, ResolvedBuildOptions, ResolvedClockSource,
    ResolvedFirmware, ResolvedGenerator, ResolvedTarget, ResolvedTestSuite, ResolvedThread,
    SdkCompatibility, Toolchain,
};
use crate::diagnostic::Origin;

#[derive(Clone, Debug, Serialize)]
pub struct ResolvedProject {
    pub name: String,
    pub mode: BuildMode,
    pub options: ResolvedBuildOptions,
    pub toolchain: Toolchain,
    pub sdk_component_names: BTreeSet<String>,
    pub firmware: ResolvedFirmware,
    pub test_suite: Option<ResolvedTestSuite>,
    pub clock_sources: Vec<ResolvedClockSource>,
    pub generators: Vec<ResolvedGenerator>,
    pub root: PathBuf,
    pub manifest: PathBuf,
    pub sdk: PathBuf,
    pub sdk_compatibility: Option<SdkCompatibility>,
    pub build_dir: PathBuf,
    pub output_dir: PathBuf,
    pub configuration_identity: String,
    pub publication_identity: String,
    pub publication_owner: String,
    pub board: ResolvedBoard,
    pub audit_board: AuditBoard,
    pub threads: Vec<ResolvedThread>,
    pub max_priority: u32,
    pub shared_objects: BTreeMap<String, u32>,
    pub targets: BTreeMap<String, ResolvedTarget>,
    pub components: Vec<ComponentRecipe>,
    pub provenance: BTreeMap<String, Origin>,
    pub config_files: Vec<PathBuf>,
}
