// Copyright 3Bian Limited and Contributors
// SPDX-License-Identifier: MIT

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RawGenerator {
    pub executable: String,
    #[serde(default)]
    pub arguments: Vec<String>,
    #[serde(default)]
    pub inputs: Vec<String>,
    pub outputs: Vec<String>,
    #[serde(default)]
    pub dependencies: Vec<String>,
    pub working_directory: Option<String>,
    #[serde(default)]
    pub environment: BTreeMap<String, String>,
    pub stdout: Option<String>,
    pub stderr: Option<String>,
}
