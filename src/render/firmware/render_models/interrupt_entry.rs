// Copyright 3Bian Limited and Contributors
// SPDX-License-Identifier: MIT

use serde::Serialize;

#[derive(Serialize)]
pub(super) struct InterruptEntry {
    pub(super) name: String,
    pub(super) number: u32,
    pub(super) separator: &'static str,
}
