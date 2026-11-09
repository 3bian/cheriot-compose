// Copyright 3Bian Limited and Contributors
// SPDX-License-Identifier: MIT

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RawLinkerFragment {
    pub start: u32,
    pub start_expression: Option<String>,
    pub program_header: Option<String>,
    pub path: Option<String>,
    pub generated_name: Option<String>,
    pub body: Option<String>,
}
