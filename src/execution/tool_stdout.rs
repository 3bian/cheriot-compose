// Copyright 3Bian Limited and Contributors
// SPDX-License-Identifier: MIT

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ToolStdout {
    Inherit,
    Stderr,
}
