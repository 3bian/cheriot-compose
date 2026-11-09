// Copyright 3Bian Limited and Contributors
// SPDX-License-Identifier: MIT

use serde::Serialize;

#[derive(Serialize)]
pub(super) struct OwnedRecordModel {
    pub(super) path: String,
    pub(super) content: String,
}
