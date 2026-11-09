// Copyright 3Bian Limited and Contributors
// SPDX-License-Identifier: MIT

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ComponentRole {
    Debug,
    Stdio,
    Softfloat,
    WallClock,
    #[serde(skip_deserializing)]
    Allocator,
    #[serde(skip_deserializing)]
    Loader,
    #[serde(skip_deserializing)]
    Scheduler,
    #[serde(skip_deserializing)]
    Switcher,
    #[serde(skip_deserializing)]
    TokenLibrary,
    #[serde(skip_deserializing)]
    SoftwareRevoker,
}
