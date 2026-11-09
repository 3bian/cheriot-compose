// Copyright 3Bian Limited and Contributors
// SPDX-License-Identifier: MIT

use serde::Serialize;

#[derive(Serialize)]
pub(super) struct SharedObjectModel {
    pub(super) global: String,
    pub(super) size: u32,
}
