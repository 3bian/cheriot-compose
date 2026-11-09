// Copyright 3Bian Limited and Contributors
// SPDX-License-Identifier: MIT

use std::{
    fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

use super::guarded_output;
use crate::{
    build_graph::BuildOperation,
    diagnostic::{Diagnostic, Result},
};

static NEXT_STAGE: AtomicU64 = AtomicU64::new(0);

pub(super) fn with_fresh_link_outputs(
    operation: &BuildOperation,
    build_dir: &Path,
    run: impl FnOnce() -> Result<()>,
) -> Result<()> {
    if operation.policy.output_handling != crate::build_graph::OutputHandling::Fresh {
        return run();
    }
    let stage = (0..64)
        .find_map(|_| {
            let serial = NEXT_STAGE.fetch_add(1, Ordering::Relaxed);
            let path = build_dir.join(format!(".link-stage-{}-{serial}", std::process::id()));
            match fs::create_dir(&path) {
                Ok(()) => Some(Ok(path)),
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => None,
                Err(error) => Some(Err(Diagnostic::io(&path, error))),
            }
        })
        .unwrap_or_else(|| Err(Diagnostic::new("cannot allocate linker staging directory")))?;
    let mut prepared: Vec<(PathBuf, Option<PathBuf>)> = Vec::new();
    let outcome = (|| -> Result<()> {
        for (index, output) in operation.outputs.iter().enumerate() {
            guarded_output(output, build_dir)?;
            match fs::symlink_metadata(output) {
                Ok(metadata) if metadata.is_file() => {
                    let backup = stage.join(format!("old-{index}"));
                    fs::rename(output, &backup).map_err(|error| Diagnostic::io(output, error))?;
                    prepared.push((output.clone(), Some(backup)));
                }
                Ok(_) => {
                    return Err(Diagnostic::new(format!(
                        "link output is not a regular file: {}",
                        output.display()
                    )));
                }
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                    prepared.push((output.clone(), None))
                }
                Err(error) => return Err(Diagnostic::io(output, error)),
            }
        }
        run()
    })();
    let mut recovered = true;
    if outcome.is_err() {
        for (output, backup) in prepared.iter().rev() {
            match fs::remove_file(output) {
                Ok(()) => (),
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => (),
                Err(_) => recovered = false,
            }
            if let Some(backup) = backup
                && fs::rename(backup, output).is_err()
            {
                recovered = false;
            }
        }
    } else {
        for (_, backup) in &prepared {
            if let Some(backup) = backup
                && fs::remove_file(backup).is_err()
            {
                recovered = false;
            }
        }
    }
    if recovered {
        let _ = fs::remove_dir(&stage);
        outcome
    } else {
        Err(Diagnostic::new(format!(
            "{}; linker recovery files remain in {}",
            outcome.err().map_or_else(
                || "cannot remove linker backups".to_owned(),
                |error| error.to_string()
            ),
            stage.display()
        )))
    }
}
