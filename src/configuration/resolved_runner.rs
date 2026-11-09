// Copyright 3Bian Limited and Contributors
// SPDX-License-Identifier: MIT

use std::collections::BTreeMap;

use serde::Serialize;

use crate::diagnostic::Sourced;

#[derive(Clone, Debug, Serialize)]
pub struct ResolvedRunner {
    pub kind: super::RunnerKind,
    pub executable: String,
    pub arguments: Vec<Sourced<String>>,
    pub failure_patterns: Vec<String>,
    pub environment: BTreeMap<String, String>,
}
