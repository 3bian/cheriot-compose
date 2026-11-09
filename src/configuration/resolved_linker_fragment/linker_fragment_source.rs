// Copyright 3Bian Limited and Contributors
// SPDX-License-Identifier: MIT

use std::path::PathBuf;

use serde::Serialize;

#[derive(Clone, Debug, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum LinkerFragmentSource {
    Inline { body: String },
    File { path: PathBuf },
    Generated { template: PathBuf, name: String },
}
