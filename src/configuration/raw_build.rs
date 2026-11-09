// Copyright 3Bian Limited and Contributors
// SPDX-License-Identifier: MIT

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use super::RawSource;

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RawBuild {
    pub sources: Vec<RawSource>,
    pub compiler: Option<String>,
    #[serde(default)]
    pub flags: Vec<String>,
    #[serde(default)]
    pub c_flags: Vec<String>,
    #[serde(default)]
    pub cpp_flags: Vec<String>,
    #[serde(default)]
    pub assembly_flags: Vec<String>,
    #[serde(default)]
    pub rust_flags: Vec<String>,
    #[serde(default)]
    pub cargo_flags: Vec<String>,
    #[serde(default)]
    pub defines: Vec<String>,
    #[serde(default)]
    pub includes: Vec<String>,
    #[serde(default)]
    pub force_includes: Vec<String>,
    #[serde(default)]
    pub shared_objects: BTreeMap<String, u32>,
    #[serde(default)]
    pub revocable_memory: bool,
}
