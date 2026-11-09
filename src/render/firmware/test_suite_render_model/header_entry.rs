// Copyright 3Bian Limited and Contributors
// SPDX-License-Identifier: MIT

use serde::Serialize;

#[derive(Serialize)]
pub(super) struct HeaderEntry {
    pub(super) name: String,
    pub(super) constant: bool,
    pub(super) prototype: bool,
    pub(super) enabled: u8,
    pub(super) compartment: String,
    pub(super) separator: &'static str,
}
