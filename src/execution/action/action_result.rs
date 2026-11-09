// Copyright 3Bian Limited and Contributors
// SPDX-License-Identifier: MIT

use std::io::Write;

use crate::diagnostic::{Diagnostic, Result};

use super::{OutputRouting, PreparedAction};

pub(in crate::execution) struct ActionResult<'a> {
    pub work: PreparedAction<'a>,
    pub result: Result<Option<bool>>,
    pub(super) stdout: Vec<u8>,
    pub(super) stderr: Vec<u8>,
    pub(super) routing: OutputRouting,
}

impl ActionResult<'_> {
    pub fn show_output(&self) -> Result<()> {
        let OutputRouting::Group { stdout_to_stderr } = self.routing else {
            return Ok(());
        };
        if !self.stdout.is_empty() || !self.stderr.is_empty() {
            eprintln!("Tool output for {}:", self.work.operation.id);
        }
        if stdout_to_stderr {
            std::io::stderr().lock().write_all(&self.stdout)
        } else {
            std::io::stdout().lock().write_all(&self.stdout)
        }
        .and_then(|()| std::io::stderr().lock().write_all(&self.stderr))
        .map_err(|error| Diagnostic::new(format!("cannot show tool output: {error}")))
    }
}
