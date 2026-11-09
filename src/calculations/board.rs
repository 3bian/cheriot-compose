// Copyright 3Bian Limited and Contributors
// SPDX-License-Identifier: MIT

use std::collections::{BTreeMap, BTreeSet};

use crate::configuration::resolution::{BoardValues, is_identifier, is_target_name};
use crate::configuration::{CompilerFlag, MemoryExtent, RawBoard, RawMemoryExtent, RevokerKind};
use crate::diagnostic::{Diagnostic, Origin, Result};

pub fn resolve_memory_extent(raw_extent: &RawMemoryExtent) -> Result<MemoryExtent> {
    let end = match (raw_extent.end, raw_extent.length) {
        (Some(end), _) => end,
        (None, Some(length)) => raw_extent
            .start
            .checked_add(length)
            .ok_or_else(|| Diagnostic::new("memory extent overflows 32-bit address space"))?,
        _ => return Err(Diagnostic::new("extent needs end or length")),
    };
    let length = end
        .checked_sub(raw_extent.start)
        .filter(|number| *number > 0)
        .ok_or_else(|| Diagnostic::new("memory extent must have end greater than start"))?;
    if raw_extent.length.is_some_and(|number| number != length) {
        return Err(Diagnostic::new("extent end and length disagree"));
    }
    Ok(MemoryExtent {
        start: raw_extent.start,
        end,
        length,
    })
}

pub(crate) fn resolve_board(
    input_board: &RawBoard,
    origins: &BTreeMap<String, Origin>,
) -> Result<BoardValues> {
    let at = |field_path: &str| {
        origins
            .get(field_path)
            .cloned()
            .unwrap_or_else(|| Origin::derived(field_path))
    };
    let fail = |field_path: &str, message: &str| Diagnostic::new(message).at(at(field_path));
    if !is_target_name(&input_board.name) {
        return Err(fail("/board/name", "invalid board name"));
    }
    let instruction_memory = resolve_memory_extent(&input_board.instruction_memory)
        .map_err(|error| error.at(at("/board/instruction_memory")))?;
    let data_memory = input_board
        .data_memory
        .as_ref()
        .map(resolve_memory_extent)
        .transpose()
        .map_err(|error| error.at(at("/board/data_memory")))?;
    if let Some(data_region) = &data_memory
        && instruction_memory.start < data_region.end
        && data_region.start < instruction_memory.end
    {
        return Err(fail(
            "/board/data_memory",
            "data and instruction memory overlap; omit data_memory for a unified region",
        ));
    }
    if input_board
        .heap
        .start
        .is_some_and(|start| start >= input_board.heap.end)
    {
        return Err(fail("/board/heap", "heap end must exceed start"));
    }
    let heap_mem = data_memory.as_ref().unwrap_or(&instruction_memory);
    if input_board.heap.end > heap_mem.end
        || input_board.heap.end <= heap_mem.start
        || input_board
            .heap
            .start
            .is_some_and(|start| start < heap_mem.start)
    {
        return Err(fail(
            "/board/heap",
            "heap must fit within data memory (or instruction memory on unified boards)",
        ));
    }
    let mut devices = BTreeMap::new();
    for (device_name, raw_extent) in &input_board.devices {
        if !is_identifier(device_name) {
            return Err(fail(
                &format!("/board/devices/{device_name}"),
                "device name must be a C identifier",
            ));
        }
        devices.insert(
            device_name.clone(),
            resolve_memory_extent(raw_extent)
                .map_err(|error| error.at(at(&format!("/board/devices/{device_name}"))))?,
        );
    }
    let mmio_start = devices
        .values()
        .map(|device| device.start)
        .min()
        .ok_or_else(|| {
            fail(
                "/board/devices",
                "at least one MMIO device is required by this profile",
            )
        })?;
    let mmio_end = devices
        .values()
        .map(|device| device.end)
        .max()
        .ok_or_else(|| fail("/board/devices", "no MMIO end"))?;
    let revocable_start = input_board.revokable_memory_start.unwrap_or(heap_mem.start);
    if input_board.timer_hz == 0
        || input_board.tickrate_hz == 0
        || input_board.tickrate_hz > input_board.timer_hz
    {
        return Err(fail(
            "/board/tickrate_hz",
            "clock rates must be positive and tick rate must not exceed timer rate",
        ));
    }
    if input_board
        .cpu
        .as_ref()
        .is_some_and(|cpu| !is_target_name(cpu))
    {
        return Err(fail("/board/cpu", "invalid CPU name"));
    }
    let mut interrupt_numbers = BTreeSet::new();
    let mut names = BTreeSet::new();
    for (interrupt_index, interrupt) in input_board.interrupts.iter().enumerate() {
        if !is_identifier(&interrupt.name)
            || !names.insert(&interrupt.name)
            || !interrupt_numbers.insert(interrupt.number)
        {
            return Err(fail(
                &format!("/board/interrupts/{interrupt_index}"),
                "interrupt names and numbers must be unique; names must be C identifiers",
            ));
        }
    }
    for target_name in input_board.rtos_defines.keys() {
        if !["allocator", "loader", "scheduler"].contains(&target_name.as_str()) {
            return Err(fail(
                &format!("/board/rtos_defines/{target_name}"),
                "unsupported RTOS define target",
            ));
        }
    }
    let mut flags = Vec::new();
    let mut add = |value: String, field_path: &str| {
        flags.push(CompilerFlag {
            value,
            origin: at(field_path),
        })
    };
    if input_board.revoker != RevokerKind::None {
        add("-DTEMPORAL_SAFETY".into(), "/board/revoker");
        if input_board.revoker == RevokerKind::Software {
            add("-DSOFTWARE_REVOKER".into(), "/board/revoker");
        }
    }
    for (definition_index, definition) in input_board.defines.iter().enumerate() {
        add(
            format!("-D{definition}"),
            &format!("/board/defines/{definition_index}"),
        );
    }
    add(
        format!("-DCPU_TIMER_HZ={}", input_board.timer_hz),
        "/board/timer_hz",
    );
    add(
        format!("-DTICK_RATE_HZ={}", input_board.tickrate_hz),
        "/board/tickrate_hz",
    );
    if input_board.simulation {
        add("-DSIMULATION".into(), "/board/simulation")
    }
    for device_name in devices.keys() {
        add(
            format!("-DDEVICE_EXISTS_{device_name}"),
            &format!("/board/devices/{device_name}"),
        );
    }
    Ok(BoardValues {
        instruction_memory,
        data_memory,
        devices,
        mmio_start,
        mmio_end,
        revocable_start,
        flags,
    })
}
