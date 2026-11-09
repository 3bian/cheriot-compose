// Copyright 3Bian Limited and Contributors
// SPDX-License-Identifier: MIT

use super::BuildCommand;

#[derive(Clone, Debug)]
pub enum CommandResolution {
    Concrete(BuildCommand),
    Alternatives {
        probe: String,
        when_false: BuildCommand,
        when_true: BuildCommand,
    },
}
