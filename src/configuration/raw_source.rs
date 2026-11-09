// Copyright 3Bian Limited and Contributors
// SPDX-License-Identifier: MIT

use serde::{Deserialize, Serialize};

use super::RawSourceSettings;

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(untagged)]
pub enum RawSource {
    Path(String),
    Detailed(Box<RawSourceSettings>),
}
