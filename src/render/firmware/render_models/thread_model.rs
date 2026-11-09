// Copyright 3Bian Limited and Contributors
// SPDX-License-Identifier: MIT

use serde::Serialize;

#[derive(Clone, Serialize)]
pub(super) struct ThreadModel {
    pub(super) thread_id: u32,
    pub(super) stack_size: u32,
    pub(super) trusted_stack_frames: u32,
    pub(super) priority: u32,
    pub(super) mangled_entry_point: String,
}
