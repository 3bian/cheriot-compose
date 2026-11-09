// Copyright 3Bian Limited and Contributors
// SPDX-License-Identifier: MIT

use std::collections::BTreeMap;
use std::path::PathBuf;

use super::FirmwareComponent;
use crate::calculations::linker_segment::LinkerSegment;

pub struct FirmwareInputs {
    pub components: Vec<FirmwareComponent>,
    pub segments: Vec<LinkerSegment>,
    pub fragment_templates: BTreeMap<PathBuf, String>,
}
