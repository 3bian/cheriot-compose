// Copyright 3Bian Limited and Contributors
// SPDX-License-Identifier: MIT

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum OriginExpansion {
    Environment { variable: String },
}
