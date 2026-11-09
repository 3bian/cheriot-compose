// Copyright 3Bian Limited and Contributors
// SPDX-License-Identifier: MIT

use std::collections::{BTreeMap, BTreeSet};

pub struct TrackedRender {
    pub content: String,
    pub partials: BTreeMap<String, String>,
    pub variables: BTreeSet<String>,
}
