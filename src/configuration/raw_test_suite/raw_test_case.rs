// Copyright 3Bian Limited and Contributors
// SPDX-License-Identifier: MIT

use serde::{Deserialize, Serialize};

use super::super::build_options::enabled_by_default;

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RawTestCase {
    pub name: String,
    pub target: String,
    pub display_name: Option<String>,
    pub enabled: Option<bool>,
    #[serde(default = "enabled_by_default")]
    pub include_in_most: bool,
    #[serde(default = "enabled_by_default")]
    pub compartment: bool,
    #[serde(default = "enabled_by_default")]
    pub prototype: bool,
    #[serde(default = "enabled_by_default")]
    pub run: bool,
    #[serde(default)]
    pub absorbed_sources: Vec<String>,
    #[serde(default)]
    pub conflicts: Vec<String>,
    pub firmware_rust_source: Option<String>,
}
