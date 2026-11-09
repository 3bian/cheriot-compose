// Copyright 3Bian Limited and Contributors
// SPDX-License-Identifier: MIT

use serde::Serialize;

use crate::diagnostic::Origin;

#[derive(Clone, Debug, Serialize)]
pub struct ResolvedTestCase {
    pub name: String,
    pub target: String,
    pub display_name: String,
    pub enabled: bool,
    pub compartment: bool,
    pub prototype: bool,
    pub run: bool,
    pub origin: Origin,
}
