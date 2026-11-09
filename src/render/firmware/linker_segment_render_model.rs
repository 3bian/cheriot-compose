// Copyright 3Bian Limited and Contributors
// SPDX-License-Identifier: MIT

use serde::Serialize;

#[derive(Serialize)]
pub(crate) struct LinkerSegmentRenderModel {
    pub ix: usize,
    pub after_first: bool,
    pub start: String,
    pub start_string: String,
    pub phtype: String,
    pub body: String,
}
