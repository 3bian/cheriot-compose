// Copyright 3Bian Limited and Contributors
// SPDX-License-Identifier: MIT

use crate::diagnostic::Result;

use super::invalid;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum InterruptState {
    Any,
    Deferred,
    Enabled,
}

impl InterruptState {
    pub(super) fn parse(value: &str, line: usize) -> Result<Self> {
        match value {
            "any" => Ok(Self::Any),
            "deferred" => Ok(Self::Deferred),
            "enabled" => Ok(Self::Enabled),
            _ => Err(invalid(line, "IRQ state must be any, deferred or enabled")),
        }
    }

    pub(super) fn font(self) -> &'static str {
        match self {
            Self::Any => "Times",
            Self::Deferred => "Times-Bold",
            Self::Enabled => "Times-Italic",
        }
    }
}
