// Copyright 3Bian Limited and Contributors
// SPDX-License-Identifier: MIT

use std::collections::BTreeMap;

use serde::Serialize;

use super::{MemoryExtent, RawBoard, ResolvedBoard, RevokerKind};

mod audit_heap;
mod audit_interrupt;

use audit_heap::AuditHeap;
use audit_interrupt::AuditInterrupt;

#[derive(Clone, Debug, Serialize)]
pub struct AuditBoard {
    devices: BTreeMap<String, MemoryExtent>,
    instruction_memory: MemoryExtent,
    #[serde(skip_serializing_if = "Option::is_none")]
    data_memory: Option<MemoryExtent>,
    heap: AuditHeap,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    interrupts: Vec<AuditInterrupt>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    defines: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    driver_includes: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    cxflags: Vec<String>,
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    rtos_defines: BTreeMap<String, Vec<String>>,
    timer_hz: u64,
    tickrate_hz: u64,
    revoker: RevokerKind,
    simulation: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    cpu: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    run_command: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    revokable_memory_start: Option<u32>,
}

impl AuditBoard {
    pub(super) fn new(raw: &RawBoard, board: &ResolvedBoard) -> Self {
        Self {
            devices: board.devices.clone(),
            instruction_memory: board.instruction_memory.clone(),
            data_memory: board.data_memory.clone(),
            heap: AuditHeap {
                start: raw.heap.start,
                end: raw.heap.end,
            },
            interrupts: raw
                .interrupts
                .iter()
                .map(|interrupt| AuditInterrupt {
                    name: interrupt.name.clone(),
                    number: interrupt.number,
                    priority: interrupt.priority,
                    edge_triggered: interrupt.edge_triggered,
                })
                .collect(),
            defines: raw.defines.clone(),
            driver_includes: raw.driver_includes.clone(),
            cxflags: raw.cxflags.clone(),
            rtos_defines: raw.rtos_defines.clone(),
            timer_hz: raw.timer_hz,
            tickrate_hz: raw.tickrate_hz,
            revoker: raw.revoker,
            simulation: raw.simulation,
            cpu: raw.cpu.clone(),
            run_command: raw.audit_run_command.clone().or_else(|| {
                raw.runner.as_ref().map(|runner| {
                    std::iter::once(runner.executable.as_str())
                        .chain(runner.arguments.iter().map(String::as_str))
                        .collect::<Vec<_>>()
                        .join(" ")
                })
            }),
            revokable_memory_start: raw.revokable_memory_start,
        }
    }
}
