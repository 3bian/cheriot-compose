// Copyright 3Bian Limited and Contributors
// SPDX-License-Identifier: MIT

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use super::build_record::BuildRecord;

#[derive(Default, Serialize, Deserialize)]
pub(super) struct BuildState {
    #[serde(default)]
    pub(super) owner: Option<String>,
    pub(super) operations: BTreeMap<String, BuildRecord>,
    #[serde(default)]
    pub(super) owned_outputs: BTreeSet<PathBuf>,
}
