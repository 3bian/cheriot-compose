// Copyright 3Bian Limited and Contributors
// SPDX-License-Identifier: MIT

use serde::Serialize;

use crate::configuration::ResolvedClockSource;

mod clock_render_source;
use clock_render_source::ClockRenderSource;

#[derive(Serialize)]
pub(crate) struct ClockRenderModel {
    sources: Vec<ClockRenderSource>,
}

impl ClockRenderModel {
    pub(crate) fn from_sources(sources: &[ResolvedClockSource]) -> Self {
        Self {
            sources: sources
                .iter()
                .map(|source| ClockRenderSource {
                    header: source.header.clone(),
                    type_name: source.type_name.clone(),
                })
                .collect(),
        }
    }
}
