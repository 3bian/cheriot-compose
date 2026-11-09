// Copyright 3Bian Limited and Contributors
// SPDX-License-Identifier: MIT

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RawToolchain {
    pub rust_compiler: Option<String>,
    pub cargo: Option<String>,
    pub uf2_converter: Option<String>,
    #[serde(default)]
    pub rust_flags: Vec<String>,
}
