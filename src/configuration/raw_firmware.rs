// Copyright 3Bian Limited and Contributors
// SPDX-License-Identifier: MIT

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use super::RawThread;
use super::build_options::enabled_by_default;

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RawFirmware {
    pub name: String,
    pub services: Vec<String>,
    #[serde(default)]
    pub libraries: Vec<String>,
    pub threads: Vec<RawThread>,
    #[serde(default)]
    pub shared_objects: BTreeMap<String, u32>,
    #[serde(default = "enabled_by_default")]
    pub compartment_report: bool,
}
