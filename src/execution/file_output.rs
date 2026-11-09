// Copyright 3Bian Limited and Contributors
// SPDX-License-Identifier: MIT

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};

use crate::diagnostic::{Diagnostic, Result};

static TEMPORARY_FILE_COUNTER: AtomicU64 = AtomicU64::new(0);
pub fn write_if_changed(path: &Path, content: &[u8]) -> Result<()> {
    if fs::read(path).ok().as_deref() == Some(content) {
        return Ok(());
    }
    let parent = path
        .parent()
        .ok_or_else(|| Diagnostic::new("output has no parent"))?;
    fs::create_dir_all(parent).map_err(|error| Diagnostic::io(parent, error))?;
    let temporary_path = parent.join(format!(
        ".cheri-{}-{}.tmp",
        std::process::id(),
        TEMPORARY_FILE_COUNTER.fetch_add(1, Ordering::Relaxed)
    ));
    let result = (|| {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary_path)
            .map_err(|error| Diagnostic::io(&temporary_path, error))?;
        file.write_all(content)
            .map_err(|error| Diagnostic::io(&temporary_path, error))?;
        file.sync_all()
            .map_err(|error| Diagnostic::io(&temporary_path, error))?;
        fs::rename(&temporary_path, path).map_err(|error| Diagnostic::io(path, error))
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temporary_path);
    }
    result
}
