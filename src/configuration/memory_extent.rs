// Copyright 3Bian Limited and Contributors
// SPDX-License-Identifier: MIT

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct MemoryExtent {
    pub start: u32,
    pub end: u32,
    pub length: u32,
}
