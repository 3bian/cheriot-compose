// Copyright 3Bian Limited and Contributors
// SPDX-License-Identifier: MIT

use super::interrupt_state::InterruptState;

#[derive(Default)]
pub(super) struct Label {
    pub(super) line: usize,
    pub(super) required: Option<InterruptState>,
    pub(super) assumed: Option<InterruptState>,
}
