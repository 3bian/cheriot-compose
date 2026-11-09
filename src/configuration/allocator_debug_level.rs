// Copyright 3Bian Limited and Contributors
// SPDX-License-Identifier: MIT

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AllocatorDebugLevel {
    #[default]
    None,
    Information,
    Warning,
    Error,
    Critical,
}

impl AllocatorDebugLevel {
    pub fn sdk_name(self) -> &'static str {
        match self {
            Self::None => "None",
            Self::Information => "Information",
            Self::Warning => "Warning",
            Self::Error => "Error",
            Self::Critical => "Critical",
        }
    }
}
