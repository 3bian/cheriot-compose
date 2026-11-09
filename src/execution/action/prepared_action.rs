// Copyright 3Bian Limited and Contributors
// SPDX-License-Identifier: MIT

use crate::build_graph::{BuildCommand, BuildOperation};

pub(in crate::execution) struct PreparedAction<'a> {
    pub operation: &'a BuildOperation,
    pub command: Option<BuildCommand>,
    pub reason: String,
    pub previous_outputs: Option<Vec<Option<String>>>,
}
