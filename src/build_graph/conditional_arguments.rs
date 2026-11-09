// Copyright 3Bian Limited and Contributors
// SPDX-License-Identifier: MIT

use serde::{Deserialize, Serialize};

use crate::diagnostic::Sourced;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ConditionalArguments {
    pub probe: String,
    pub insert_at: usize,
    pub arguments: Vec<Sourced<String>>,
}
