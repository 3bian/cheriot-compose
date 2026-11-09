// Copyright 3Bian Limited and Contributors
// SPDX-License-Identifier: MIT

use std::{
    fs,
    path::{Path, PathBuf},
};

use crate::diagnostic::{Diagnostic, Result};

use super::guarded_output;

pub(super) struct BuildLock(PathBuf);

impl BuildLock {
    pub(super) fn acquire(build_dir: &Path) -> Result<Self> {
        let path = build_dir.join(".cheriot-compose.lock");
        guarded_output(&path, build_dir)?;
        fs::create_dir_all(build_dir).map_err(|error| Diagnostic::io(build_dir, error))?;
        fs::create_dir(&path).map_err(|error| {
            if error.kind() == std::io::ErrorKind::AlreadyExists {
                Diagnostic::new(format!("build directory is locked: {}. Wait for the other build to finish; if it stopped unexpectedly, remove the stale lock directory.", path.display()))
            } else { Diagnostic::io(&path, error) }
        })?;
        Ok(Self(path))
    }
}

impl Drop for BuildLock {
    fn drop(&mut self) {
        let _ = fs::remove_dir(&self.0);
    }
}
