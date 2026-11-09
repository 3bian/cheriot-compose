// Copyright 3Bian Limited and Contributors
// SPDX-License-Identifier: MIT

use crate::configuration::LinkerFragmentSource;
use crate::diagnostic::Origin;

#[derive(Clone, Debug)]
pub struct LinkerSegment {
    pub start: u32,
    pub start_expression: Option<String>,
    pub program_header: String,
    pub source: LinkerFragmentSource,
    pub origin: Option<Origin>,
}
