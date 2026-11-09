// Copyright 3Bian Limited and Contributors
// SPDX-License-Identifier: MIT

use serde::Serialize;

use crate::configuration::MemoryExtent;

use super::hexadecimal;

#[derive(Serialize)]
pub(super) struct MemorySpan {
    name: String,
    start: String,
    end: String,
}
impl MemorySpan {
    pub(super) fn new(name: &str, extent: &MemoryExtent) -> Self {
        Self {
            name: name.into(),
            start: hexadecimal(extent.start),
            end: hexadecimal(extent.end),
        }
    }
}
