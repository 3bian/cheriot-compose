// Copyright 3Bian Limited and Contributors
// SPDX-License-Identifier: MIT

use serde::Serialize;

#[derive(Serialize)]
pub(crate) struct FirmwareLayoutModel<T: Serialize> {
    pub segments: Vec<T>,
}
