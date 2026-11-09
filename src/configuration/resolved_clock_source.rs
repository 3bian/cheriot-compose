// Copyright 3Bian Limited and Contributors
// SPDX-License-Identifier: MIT

use std::path::PathBuf;

use serde::Serialize;

use crate::diagnostic::Origin;

#[derive(Clone, Debug, Serialize)]
pub struct ResolvedClockSource {
    pub header: String,
    pub type_name: String,
    pub include_dir: PathBuf,
    pub origin: Origin,
}
