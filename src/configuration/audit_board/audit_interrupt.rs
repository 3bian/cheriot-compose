// Copyright 3Bian Limited and Contributors
// SPDX-License-Identifier: MIT

use serde::Serialize;

#[derive(Clone, Debug, Serialize)]
pub(super) struct AuditInterrupt {
    pub(super) name: String,
    pub(super) number: u32,
    pub(super) priority: u32,
    #[serde(skip_serializing_if = "is_false")]
    pub(super) edge_triggered: bool,
}

fn is_false(value: &bool) -> bool {
    !value
}
