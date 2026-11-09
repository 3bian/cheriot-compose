// Copyright 3Bian Limited and Contributors
// SPDX-License-Identifier: MIT

use std::collections::BTreeMap;

use crate::configuration::{CompilerFlag, MemoryExtent};

pub(crate) struct BoardValues {
    pub instruction_memory: MemoryExtent,
    pub data_memory: Option<MemoryExtent>,
    pub devices: BTreeMap<String, MemoryExtent>,
    pub mmio_start: u32,
    pub mmio_end: u32,
    pub revocable_start: u32,
    pub flags: Vec<CompilerFlag>,
}
