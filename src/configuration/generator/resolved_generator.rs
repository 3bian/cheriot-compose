// Copyright 3Bian Limited and Contributors
// SPDX-License-Identifier: MIT

use std::{collections::BTreeMap, path::PathBuf};

use serde::Serialize;

use crate::diagnostic::{Origin, Sourced};

#[derive(Clone, Debug, Serialize)]
pub struct ResolvedGenerator {
    pub name: String,
    pub executable: String,
    pub arguments: Vec<Sourced<String>>,
    pub inputs: Vec<PathBuf>,
    pub outputs: Vec<PathBuf>,
    pub dependencies: Vec<String>,
    pub working_directory: PathBuf,
    pub environment: BTreeMap<String, String>,
    pub stdout: Option<PathBuf>,
    pub stderr: Option<PathBuf>,
    pub origin: Origin,
}
