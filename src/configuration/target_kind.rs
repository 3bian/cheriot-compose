// Copyright 3Bian Limited and Contributors
// SPDX-License-Identifier: MIT

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TargetKind {
    Compartment,
    Library,
    Object,
    Group,
}
