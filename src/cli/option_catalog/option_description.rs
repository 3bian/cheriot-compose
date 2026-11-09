// Copyright 3Bian Limited and Contributors
// SPDX-License-Identifier: MIT

use super::super::command_name::CommandName;

pub(in crate::cli) struct OptionDescription {
    pub name: &'static str,
    pub aliases: &'static [&'static str],
    pub value: Option<&'static str>,
    pub repeatable: bool,
    pub commands: &'static [CommandName],
    pub conflicts_with_imported_database: bool,
    pub description: &'static str,
}

impl OptionDescription {
    pub fn label(&self) -> String {
        let mut label = self
            .aliases
            .iter()
            .chain(std::iter::once(&self.name))
            .copied()
            .collect::<Vec<_>>()
            .join(", ");
        if let Some(value) = self.value {
            label.push(' ');
            label.push_str(value);
        }
        label
    }
}
