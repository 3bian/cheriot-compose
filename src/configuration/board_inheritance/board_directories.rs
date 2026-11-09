// Copyright 3Bian Limited and Contributors
// SPDX-License-Identifier: MIT

use std::path::PathBuf;

pub(in crate::configuration) struct BoardDirectories {
    pub selected_file: PathBuf,
    pub selected: PathBuf,
    pub base: PathBuf,
}
