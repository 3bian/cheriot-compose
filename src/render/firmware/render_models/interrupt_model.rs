// Copyright 3Bian Limited and Contributors
// SPDX-License-Identifier: MIT

use serde::Serialize;

use crate::configuration::ResolvedProject;

use super::interrupt_entry::InterruptEntry;

#[derive(Serialize)]
pub(crate) struct InterruptModel {
    interrupts: Vec<InterruptEntry>,
}
impl InterruptModel {
    pub fn new(project: &ResolvedProject) -> Self {
        let mut entries: Vec<_> = project
            .board
            .interrupts
            .iter()
            .map(|item| InterruptEntry {
                name: item.name.clone(),
                number: item.number,
                separator: ",\n",
            })
            .collect();
        if entries.is_empty() {
            entries.push(InterruptEntry {
                name: "DummyInterrupt".into(),
                number: 0,
                separator: "",
            });
        }
        if let Some(last) = entries.last_mut() {
            last.separator = "";
        }
        Self {
            interrupts: entries,
        }
    }
}
