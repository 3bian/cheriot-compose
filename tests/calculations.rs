// Copyright 3Bian Limited and Contributors
// SPDX-License-Identifier: MIT

use cheriot_compose::calculations::board;
use cheriot_compose::configuration::RawMemoryExtent;

#[test]
fn memory_extents_detect_overflow_and_disagreement() {
    let resolved = board::resolve_memory_extent(&RawMemoryExtent {
        start: 0x1000,
        end: None,
        length: Some(0x200),
    })
    .unwrap();
    assert_eq!(
        (resolved.start, resolved.end, resolved.length),
        (0x1000, 0x1200, 0x200)
    );
    for extent in [
        RawMemoryExtent {
            start: u32::MAX,
            end: None,
            length: Some(2),
        },
        RawMemoryExtent {
            start: 0x1000,
            end: Some(0x1200),
            length: Some(0x100),
        },
    ] {
        assert!(board::resolve_memory_extent(&extent).is_err());
    }
}
