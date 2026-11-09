// Copyright 3Bian Limited and Contributors
// SPDX-License-Identifier: MIT

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum InterruptSource {
    #[default]
    IbexRevoker,
    SunburstUart1,
}

impl InterruptSource {
    pub fn name(self) -> &'static str {
        match self {
            Self::IbexRevoker => "ibex_revoker",
            Self::SunburstUart1 => "sunburst_uart1",
        }
    }
}
