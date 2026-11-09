// Copyright 3Bian Limited and Contributors
// SPDX-License-Identifier: MIT

use serde::Serialize;

#[derive(Serialize)]
pub(super) struct ClockRenderSource {
    pub(super) header: String,
    pub(super) type_name: String,
}
