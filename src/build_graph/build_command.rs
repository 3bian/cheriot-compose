// Copyright 3Bian Limited and Contributors
// SPDX-License-Identifier: MIT

use std::collections::BTreeMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::diagnostic::Sourced;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct BuildCommand {
    pub executable: String,
    pub argv: Vec<Sourced<String>>,
    pub cwd: PathBuf,
    pub environment: BTreeMap<String, String>,
    pub stdout: Option<PathBuf>,
    pub stderr: Option<PathBuf>,
}
