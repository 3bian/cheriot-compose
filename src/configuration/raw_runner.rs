// Copyright 3Bian Limited and Contributors
// SPDX-License-Identifier: MIT

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RawRunner {
    #[serde(default)]
    pub kind: super::RunnerKind,
    pub executable: String,
    #[serde(default)]
    pub arguments: Vec<String>,
    #[serde(default)]
    pub failure_patterns: Vec<String>,
    #[serde(default)]
    pub environment: BTreeMap<String, String>,
}
