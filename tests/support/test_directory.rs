// Copyright 3Bian Limited and Contributors
// SPDX-License-Identifier: MIT

use std::{
    fs,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};

static NEXT: AtomicU64 = AtomicU64::new(0);

pub struct TestDirectory {
    pub directory: PathBuf,
}

impl TestDirectory {
    pub fn new() -> Self {
        let directory = std::env::temp_dir().join(format!(
            "cheri test ' space-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&directory).unwrap();
        Self { directory }
    }

    pub fn write_file(&self, name: &str, contents: &str) {
        let path = self.directory.join(name);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, contents).unwrap();
    }
}

impl Drop for TestDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.directory);
    }
}
