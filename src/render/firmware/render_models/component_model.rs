// Copyright 3Bian Limited and Contributors
// SPDX-License-Identifier: MIT

use serde::Serialize;

#[derive(Clone, Serialize)]
pub(super) struct ComponentModel {
    pub(super) compartment: String,
    pub(super) obj: String,
}
