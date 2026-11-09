// Copyright 3Bian Limited and Contributors
// SPDX-License-Identifier: MIT

use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct CompilationDifference {
    pub file: String,
    pub output: Option<String>,
    pub changes: Vec<String>,
}
