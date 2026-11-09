// Copyright 3Bian Limited and Contributors
// SPDX-License-Identifier: MIT

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Interrupt {
    pub name: String,
    pub number: u32,
    pub priority: u32,
    #[serde(default)]
    pub edge_triggered: bool,
}
