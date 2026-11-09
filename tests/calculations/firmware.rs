// Copyright 3Bian Limited and Contributors
// SPDX-License-Identifier: MIT

use std::collections::BTreeMap;

use crate::calculations::{firmware, threads};
use crate::configuration::{BuildOptions, RawFirmware, RawThread};

fn thread(name: &str, priority: u32, stack_size: u32) -> RawThread {
    RawThread {
        name: name.into(),
        compartment: "worker".into(),
        entry: "run".into(),
        priority,
        stack_size,
        trusted_stack_frames: 3,
    }
}

fn firmware_with_threads(threads: Vec<RawThread>) -> RawFirmware {
    RawFirmware {
        name: "example".into(),
        services: vec!["worker".into()],
        libraries: Vec::new(),
        threads,
        shared_objects: BTreeMap::new(),
        compartment_report: true,
    }
}

#[test]
fn thread_priorities_are_dense_and_ids_follow_the_input_order() {
    let firmware = firmware_with_threads(vec![
        thread("first", 400, 0x500),
        thread("second", 2, 0x500),
        thread("third", 400, 65_280),
    ]);
    let (resolved, maximum) = threads::resolve_threads(&firmware, &BTreeMap::new()).unwrap();
    assert_eq!(maximum, 1);
    assert_eq!(
        resolved.iter().map(|thread| thread.id).collect::<Vec<_>>(),
        [1, 2, 3]
    );
    assert_eq!(
        resolved
            .iter()
            .map(|thread| thread.priority)
            .collect::<Vec<_>>(),
        [1, 0, 1]
    );
    assert_eq!(resolved[0].entry_symbol, "__export_worker__Z3runv");
    let objects = firmware::shared_objects(&firmware, &resolved, &BTreeMap::new()).unwrap();
    assert_eq!(objects["allocator_epoch"], 4);
    assert_eq!(objects["allocator_hazard_pointers"], 48);
}

#[test]
fn firmware_calculations_reject_invalid_values() {
    let firmware = firmware_with_threads(vec![thread("worker", 1, 65_281)]);
    assert!(
        threads::resolve_threads(&firmware, &BTreeMap::new())
            .unwrap_err()
            .to_string()
            .contains("65,280")
    );
    let options = BuildOptions {
        allocator: false,
        allocator_rendering: true,
        ..BuildOptions::default()
    };
    assert!(
        firmware::resolve_options(&firmware, &options, &BTreeMap::new())
            .unwrap_err()
            .to_string()
            .contains("allocator rendering requires")
    );
    let mut existing = BTreeMap::from([("shared".into(), 4)]);
    assert!(
        firmware::merge_shared_objects(&mut existing, &BTreeMap::from([("shared".into(), 8)]))
            .unwrap_err()
            .to_string()
            .contains("conflicting sizes")
    );
}
