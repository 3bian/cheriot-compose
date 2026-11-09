// Copyright 3Bian Limited and Contributors
// SPDX-License-Identifier: MIT

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BenchmarkMetric {
    #[default]
    Rdcycle,
    Rdinstret,
    RdLsuStalls,
    RdIfetchStalls,
}

impl BenchmarkMetric {
    pub fn name(self) -> &'static str {
        match self {
            Self::Rdcycle => "rdcycle",
            Self::Rdinstret => "rdinstret",
            Self::RdLsuStalls => "rd_lsu_stalls",
            Self::RdIfetchStalls => "rd_ifetch_stalls",
        }
    }
}
