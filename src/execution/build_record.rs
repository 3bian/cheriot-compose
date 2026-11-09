// Copyright 3Bian Limited and Contributors
// SPDX-License-Identifier: MIT

use std::collections::BTreeMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

#[derive(Clone, Default, Serialize, Deserialize)]
pub(super) struct BuildRecord {
    pub(super) command: String,
    pub(super) inputs: BTreeMap<PathBuf, String>,
    pub(super) outputs: BTreeMap<PathBuf, String>,
    pub(super) probe: Option<bool>,
}
