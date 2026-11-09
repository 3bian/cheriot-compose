// Copyright 3Bian Limited and Contributors
// SPDX-License-Identifier: MIT

use std::collections::{BTreeMap, BTreeSet};

use crate::configuration::resolution::is_identifier;
use crate::configuration::{RawFirmware, ResolvedThread};
use crate::diagnostic::{Diagnostic, Origin, Result};

pub(crate) fn resolve_threads(
    firmware: &RawFirmware,
    origins: &BTreeMap<String, Origin>,
) -> Result<(Vec<ResolvedThread>, u32)> {
    let at = |field_path: &str| {
        origins
            .get(field_path)
            .cloned()
            .unwrap_or_else(|| Origin::derived(field_path))
    };
    let fail = |field_path: &str, message: &str| Diagnostic::new(message).at(at(field_path));
    let distinct: BTreeSet<u32> = firmware
        .threads
        .iter()
        .map(|thread| thread.priority)
        .collect();
    let priorities: BTreeMap<u32, u32> = distinct
        .into_iter()
        .enumerate()
        .map(|(index, priority)| (priority, index as u32))
        .collect();
    let mut threads = Vec::new();
    let mut thread_names = BTreeSet::new();
    for (thread_index, thread) in firmware.threads.iter().enumerate() {
        let field_path = format!("/firmware/threads/{thread_index}");
        if !thread_names.insert(&thread.name) {
            return Err(fail(&field_path, "duplicate thread name"));
        }
        if !is_identifier(&thread.entry) || !is_identifier(&thread.compartment) {
            return Err(fail(
                &field_path,
                "thread entry and compartment must be simple ASCII C identifiers; arbitrary C++ mangling is unsupported",
            ));
        }
        if thread.stack_size == 0 || thread.stack_size > 65280 {
            return Err(fail(
                &format!("{field_path}/stack_size"),
                "C-stack size must be between 1 and 65,280 bytes",
            ));
        }
        let priority = *priorities
            .get(&thread.priority)
            .ok_or_else(|| fail(&field_path, "missing priority"))?;
        threads.push(ResolvedThread {
            requested: thread.clone(),
            id: (thread_index + 1)
                .try_into()
                .map_err(|_| fail(&field_path, "too many threads"))?,
            priority,
            entry_symbol: format!(
                "__export_{}__Z{}{}v",
                thread.compartment,
                thread.entry.len(),
                thread.entry
            ),
        });
    }
    if threads.len() > u16::MAX as usize {
        return Err(fail(
            "/firmware/threads",
            "thread count exceeds loader metadata",
        ));
    }
    let max_priority = threads
        .iter()
        .map(|thread| thread.priority)
        .max()
        .unwrap_or(0);
    Ok((threads, max_priority))
}
