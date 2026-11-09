// Copyright 3Bian Limited and Contributors
// SPDX-License-Identifier: MIT

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PackagingKind {
    SonataLegacy,
    SonataV1,
    MicrosoftSafe,
}
