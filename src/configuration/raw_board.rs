// Copyright 3Bian Limited and Contributors
// SPDX-License-Identifier: MIT

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use super::{
    HeapRegion, Interrupt, PackagingKind, RawLinkerFragment, RawMemoryExtent, RawRunner,
    RevokerKind,
};

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RawBoard {
    pub name: String,
    pub devices: BTreeMap<String, RawMemoryExtent>,
    pub instruction_memory: RawMemoryExtent,
    pub data_memory: Option<RawMemoryExtent>,
    pub heap: HeapRegion,
    pub revokable_memory_start: Option<u32>,
    #[serde(default)]
    pub revoker: RevokerKind,
    #[serde(default)]
    pub interrupts: Vec<Interrupt>,
    #[serde(default)]
    pub defines: Vec<String>,
    #[serde(default)]
    pub cxflags: Vec<String>,
    #[serde(default)]
    pub driver_includes: Vec<String>,
    #[serde(default)]
    pub rtos_defines: BTreeMap<String, Vec<String>>,
    pub timer_hz: u64,
    pub tickrate_hz: u64,
    #[serde(default)]
    pub simulation: bool,
    pub cpu: Option<String>,
    pub runner: Option<RawRunner>,
    pub audit_run_command: Option<String>,
    pub packaging: Option<PackagingKind>,
    #[serde(default)]
    pub linker_fragments: Vec<RawLinkerFragment>,
}
