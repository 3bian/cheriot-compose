// Copyright 3Bian Limited and Contributors
// SPDX-License-Identifier: MIT

use serde::{Deserialize, Serialize};

use super::{ComponentRole, RawBuild, TargetKind};

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RawTarget {
    pub kind: TargetKind,
    #[serde(default)]
    pub role: Option<ComponentRole>,
    pub build: RawBuild,
    #[serde(default)]
    pub dependencies: Vec<String>,
}
