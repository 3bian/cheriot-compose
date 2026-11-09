// Copyright 3Bian Limited and Contributors
// SPDX-License-Identifier: MIT

use super::ToolStdout;

#[derive(Clone, Copy, Debug)]
pub struct ExecutionOptions {
    pub dry_run: bool,
    pub jobs: usize,
    pub tool_stdout: ToolStdout,
}
