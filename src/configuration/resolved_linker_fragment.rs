// Copyright 3Bian Limited and Contributors
// SPDX-License-Identifier: MIT

use serde::Serialize;

use crate::diagnostic::Origin;

mod linker_fragment_source;
pub use linker_fragment_source::LinkerFragmentSource;

#[derive(Clone, Debug, Serialize)]
pub struct ResolvedLinkerFragment {
    pub start: u32,
    pub start_expression: String,
    pub program_header: String,
    pub source: LinkerFragmentSource,
    pub origin: Origin,
}
