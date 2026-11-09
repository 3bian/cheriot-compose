// Copyright 3Bian Limited and Contributors
// SPDX-License-Identifier: MIT

use serde::Serialize;

#[derive(Clone, Debug, Serialize)]
pub struct ResolvedFirmware {
    pub name: String,
    pub services: Vec<String>,
    pub libraries: Vec<String>,
    pub compartment_report: bool,
}
