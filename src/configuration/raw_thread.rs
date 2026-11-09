// Copyright 3Bian Limited and Contributors
// SPDX-License-Identifier: MIT

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RawThread {
    pub name: String,
    pub compartment: String,
    pub entry: String,
    pub priority: u32,
    pub stack_size: u32,
    pub trusted_stack_frames: u32,
}
