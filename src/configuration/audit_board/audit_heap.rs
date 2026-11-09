// Copyright 3Bian Limited and Contributors
// SPDX-License-Identifier: MIT

use serde::Serialize;

#[derive(Clone, Debug, Serialize)]
pub(super) struct AuditHeap {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) start: Option<u32>,
    pub(super) end: u32,
}
