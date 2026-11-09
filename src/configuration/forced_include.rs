// Copyright 3Bian Limited and Contributors
// SPDX-License-Identifier: MIT

use std::path::PathBuf;

use serde::Serialize;

use crate::diagnostic::Origin;

#[derive(Clone, Debug, Serialize)]
pub struct ForcedInclude {
    pub path: PathBuf,
    pub origin: Origin,
}
