// Copyright 3Bian Limited and Contributors
// SPDX-License-Identifier: MIT

use std::path::PathBuf;

use super::linker_segment::LinkerSegment;
use crate::configuration::{LinkerFragmentSource, ResolvedBoard};

pub fn firmware_linker_segments(
    board: &ResolvedBoard,
    read_only: PathBuf,
    writable: PathBuf,
) -> Vec<LinkerSegment> {
    let mut segments = vec![
        LinkerSegment {
            start: board.instruction_memory.start,
            start_expression: None,
            program_header: "LOAD".into(),
            source: LinkerFragmentSource::File { path: read_only },
            origin: None,
        },
        LinkerSegment {
            start: board
                .data_memory
                .as_ref()
                .map(|memory| memory.start)
                .unwrap_or(board.instruction_memory.start.saturating_add(1)),
            start_expression: board.data_memory.is_none().then(|| ".".into()),
            program_header: "LOAD".into(),
            source: LinkerFragmentSource::File { path: writable },
            origin: None,
        },
    ];
    segments.extend(board.linker_fragments.iter().map(|fragment| LinkerSegment {
        start: fragment.start,
        start_expression: Some(fragment.start_expression.clone()),
        program_header: fragment.program_header.clone(),
        source: fragment.source.clone(),
        origin: Some(fragment.origin.clone()),
    }));
    segments.sort_by_key(|segment| segment.start);
    segments
}
