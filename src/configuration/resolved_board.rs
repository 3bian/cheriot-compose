// Copyright 3Bian Limited and Contributors
// SPDX-License-Identifier: MIT

use std::collections::BTreeMap;
use std::path::PathBuf;

use serde::Serialize;

use super::{
    CompilerFlag, HeapRegion, Interrupt, MemoryExtent, PackagingKind, ResolvedLinkerFragment,
    ResolvedRunner, RevokerKind,
};

#[derive(Clone, Debug, Serialize)]
pub struct ResolvedBoard {
    pub name: String,
    pub cpu: String,
    pub heap: HeapRegion,
    pub revoker: RevokerKind,
    pub interrupts: Vec<Interrupt>,
    pub defines: Vec<String>,
    pub cxflags: Vec<String>,
    pub rtos_defines: BTreeMap<String, Vec<String>>,
    pub timer_hz: u64,
    pub tickrate_hz: u64,
    pub simulation: bool,
    pub runner: Option<ResolvedRunner>,
    pub packaging: Option<PackagingKind>,
    pub linker_fragments: Vec<ResolvedLinkerFragment>,
    pub devices: BTreeMap<String, MemoryExtent>,
    pub instruction_memory: MemoryExtent,
    pub data_memory: Option<MemoryExtent>,
    pub mmio_start: u32,
    pub mmio_end: u32,
    pub revocable_start: u32,
    pub heap_start: String,
    pub flags: Vec<CompilerFlag>,
    pub includes: Vec<crate::diagnostic::Sourced<PathBuf>>,
}
