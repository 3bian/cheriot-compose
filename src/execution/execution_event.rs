// Copyright 3Bian Limited and Contributors
// SPDX-License-Identifier: MIT

use serde::Serialize;

use crate::build_graph::BuildCommand;

#[derive(Clone, Debug, Serialize)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum ExecutionEvent {
    Planned {
        operation: String,
        reason: String,
    },
    Started {
        operation: String,
        reason: String,
        command: Option<BuildCommand>,
    },
    Skipped {
        operation: String,
        reason: String,
    },
    Completed {
        operation: String,
        changed: bool,
    },
    Failed {
        operation: String,
        message: String,
    },
    BuildCompleted {
        executed: usize,
        skipped: usize,
        dry_run: bool,
        success: bool,
    },
}
