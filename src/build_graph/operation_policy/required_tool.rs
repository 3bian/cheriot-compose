// Copyright 3Bian Limited and Contributors
// SPDX-License-Identifier: MIT

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RequiredTool {
    SelectedLlvm,
    ProbeCompiler,
    Cargo,
    FirmwareRust,
    Uf2Converter,
}
