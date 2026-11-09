// Copyright 3Bian Limited and Contributors
// SPDX-License-Identifier: MIT

use std::collections::BTreeMap;
use std::path::PathBuf;

use crate::configuration::board_inheritance::BoardDirectories;
use crate::configuration::{
    AuditBoard, RawFirmware, RawProject, ResolvedBoard, ResolvedBuildOptions, ResolvedClockSource,
    ResolvedTestSuite, SdkCompatibility, Toolchain,
};
use crate::diagnostic::Origin;

pub(crate) struct LoadedProject {
    pub raw: RawProject,
    pub firmware: RawFirmware,
    pub root: PathBuf,
    pub(super) board_directories: BoardDirectories,
    pub origins: BTreeMap<String, Origin>,
    pub initial_origins: BTreeMap<String, Origin>,
    pub files: Vec<PathBuf>,
    pub sdk: PathBuf,
    pub sdk_compatibility: Option<SdkCompatibility>,
    pub toolchain: Toolchain,
    pub options: ResolvedBuildOptions,
    pub board: ResolvedBoard,
    pub audit_board: AuditBoard,
    pub test_suite: Option<ResolvedTestSuite>,
    pub clock_sources: Vec<ResolvedClockSource>,
}
