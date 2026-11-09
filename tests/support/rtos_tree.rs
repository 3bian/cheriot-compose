// Copyright 3Bian Limited and Contributors
// SPDX-License-Identifier: MIT

#![allow(dead_code)]

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT: AtomicU64 = AtomicU64::new(0);

pub fn copy_tree(source: &Path, destination: &Path) {
    std::fs::create_dir_all(destination).unwrap();
    for entry in std::fs::read_dir(source).unwrap() {
        let entry = entry.unwrap();
        let name = entry.file_name();
        let kind = entry.file_type().unwrap();
        if kind.is_dir() {
            if [".git", ".cheriot-compose", "target", "build"]
                .iter()
                .any(|excluded| name == *excluded)
            {
                continue;
            }
            copy_tree(&entry.path(), &destination.join(name));
        } else if kind.is_file() {
            std::fs::copy(entry.path(), destination.join(name)).unwrap();
        } else if kind.is_symlink() {
            #[cfg(unix)]
            std::os::unix::fs::symlink(
                std::fs::read_link(entry.path()).unwrap(),
                destination.join(name),
            )
            .unwrap();
            #[cfg(not(unix))]
            if entry.path().is_dir() {
                copy_tree(&entry.path(), &destination.join(name));
            } else {
                std::fs::copy(entry.path(), destination.join(name)).unwrap();
            }
        } else {
            panic!("unexpected source-tree entry: {}", entry.path().display());
        }
    }
}

pub struct RtosTree {
    pub root: PathBuf,
}

impl RtosTree {
    pub fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "cheriot-rtos-overlay-test-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let repository = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
        copy_tree(&repository.join("cheriot-rtos"), &root);
        copy_tree(
            &repository.join("cheriot-compose/integrations/cheriot-rtos"),
            &root,
        );
        Self { root }
    }
}

impl Drop for RtosTree {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}
