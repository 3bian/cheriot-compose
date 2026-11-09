// Copyright 3Bian Limited and Contributors
// SPDX-License-Identifier: MIT

use super::{RawBoard, RawFirmware, RawProject, resolution::ResolutionContext};

pub(crate) struct ComposedProject {
    pub raw: RawProject,
    pub firmware: RawFirmware,
    pub board: RawBoard,
    pub context: ResolutionContext,
}
