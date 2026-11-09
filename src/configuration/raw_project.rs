// Copyright 3Bian Limited and Contributors
// SPDX-License-Identifier: MIT

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use super::{
    BuildMode, BuildOptions, RawClockSource, RawFirmware, RawGenerator, RawSdkCompatibility,
    RawTarget, RawTestSuite, RawToolchain,
};

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RawProject {
    pub name: String,
    pub sdk: String,
    pub build_dir: Option<String>,
    pub output_dir: Option<String>,
    pub toolchain_dir: Option<String>,
    pub sdk_compatibility: Option<RawSdkCompatibility>,
    pub board: String,
    #[serde(default)]
    pub board_overlays: Vec<String>,
    #[serde(default)]
    pub include: Vec<String>,
    #[serde(default)]
    pub services: BTreeMap<String, RawTarget>,
    #[serde(default)]
    pub libraries: BTreeMap<String, RawTarget>,
    #[serde(default)]
    pub sdk_components: BTreeMap<String, RawTarget>,
    #[serde(default)]
    pub generators: BTreeMap<String, RawGenerator>,
    pub firmwares: Vec<RawFirmware>,
    pub test_suite: Option<RawTestSuite>,
    #[serde(default)]
    pub clock_sources: Vec<RawClockSource>,
    #[serde(default)]
    pub options: BuildOptions,
    #[serde(default)]
    pub mode: BuildMode,
    #[serde(default)]
    pub toolchain: RawToolchain,
}
