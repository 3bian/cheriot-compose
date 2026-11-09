// Copyright 3Bian Limited and Contributors
// SPDX-License-Identifier: MIT

use std::path::PathBuf;

use serde::Serialize;

use super::BuildCommand;

#[derive(Clone, Debug, Serialize)]
pub struct RunnerPlan {
    pub firmware: PathBuf,
    pub command: BuildCommand,
    pub failure_patterns: Vec<String>,
}
