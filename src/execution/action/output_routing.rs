// Copyright 3Bian Limited and Contributors
// SPDX-License-Identifier: MIT

#[derive(Clone, Copy)]
pub(in crate::execution) enum OutputRouting {
    Stream { stdout_to_stderr: bool },
    Group { stdout_to_stderr: bool },
}
