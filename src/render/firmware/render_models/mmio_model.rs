// Copyright 3Bian Limited and Contributors
// SPDX-License-Identifier: MIT

use serde::Serialize;

use crate::configuration::ResolvedProject;

use super::{hexadecimal, memory_span::MemorySpan};

#[derive(Serialize)]
pub(crate) struct MmioModel {
    devices: Vec<MemorySpan>,
    spans: Vec<MemorySpan>,
    mmio_start: String,
    mmio_end: String,
    heap_end: String,
}
impl MmioModel {
    pub fn new(project: &ResolvedProject) -> Self {
        let mut spans = vec![MemorySpan::new(
            "instruction_memory",
            &project.board.instruction_memory,
        )];
        if let Some(data) = &project.board.data_memory {
            spans.push(MemorySpan::new("data_memory", data));
        }
        Self {
            devices: project
                .board
                .devices
                .iter()
                .map(|(name, extent)| MemorySpan::new(name, extent))
                .collect(),
            spans,
            mmio_start: hexadecimal(project.board.mmio_start),
            mmio_end: hexadecimal(project.board.mmio_end),
            heap_end: hexadecimal(project.board.heap.end),
        }
    }
}
