// Copyright 3Bian Limited and Contributors
// SPDX-License-Identifier: MIT

use serde::Serialize;

#[derive(Serialize)]
pub(super) struct TestCall {
    pub(super) name: String,
    pub(super) display_name: String,
    pub(super) separator: &'static str,
}
