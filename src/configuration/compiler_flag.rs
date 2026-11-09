// Copyright 3Bian Limited and Contributors
// SPDX-License-Identifier: MIT

use serde::Serialize;

use crate::diagnostic::Origin;

#[derive(Clone, Debug, Serialize)]
pub struct CompilerFlag {
    pub value: String,
    pub origin: Origin,
}
