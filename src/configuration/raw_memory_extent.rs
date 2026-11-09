// Copyright 3Bian Limited and Contributors
// SPDX-License-Identifier: MIT

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RawMemoryExtent {
    pub start: u32,
    pub end: Option<u32>,
    pub length: Option<u32>,
}
