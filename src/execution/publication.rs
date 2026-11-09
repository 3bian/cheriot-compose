// Copyright 3Bian Limited and Contributors
// SPDX-License-Identifier: MIT

use std::{
    fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

use crate::{
    build_graph::{BuildPlan, MARKER_VERSION, Publication},
    diagnostic::{Diagnostic, Result},
};

use super::{guarded_output, write_if_changed};

mod marker;
use marker::Marker;

static NEXT_STAGE: AtomicU64 = AtomicU64::new(0);

fn checksum(bytes: &[u8]) -> String {
    fn step(mut crc: u32, byte: u8) -> u32 {
        crc ^= u32::from(byte) << 24;
        for _ in 0..8 {
            crc = if crc & 0x8000_0000 != 0 {
                (crc << 1) ^ 0x04c1_1db7
            } else {
                crc << 1
            };
        }
        crc
    }
    let mut crc = 0_u32;
    for byte in bytes {
        crc = step(crc, *byte);
    }
    let mut length = bytes.len();
    while length != 0 {
        crc = step(crc, (length & 0xff) as u8);
        length >>= 8;
    }
    format!("{} {}", !crc, bytes.len())
}

fn read_checksum(path: &Path) -> Result<String> {
    fs::read(path)
        .map(|bytes| checksum(&bytes))
        .map_err(|error| Diagnostic::io(path, error))
}

fn marker_text(identity: &str, elf: &str, report: Option<&str>) -> String {
    format!(
        "{MARKER_VERSION}\n{identity}\n{elf}\n{}\n",
        report.unwrap_or("-")
    )
}

fn read_marker(path: &Path) -> Result<Option<Marker>> {
    let text = match fs::read_to_string(path) {
        Ok(text) => text,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(Diagnostic::io(path, error)),
    };
    let lines: Vec<_> = text.lines().collect();
    let valid_checksum = |value: &str| {
        let mut words = value.split(' ');
        words
            .next()
            .is_some_and(|number| number.parse::<u32>().is_ok())
            && words
                .next()
                .is_some_and(|number| number.parse::<u64>().is_ok())
            && words.next().is_none()
    };
    if lines.len() != 4
        || lines[0] != MARKER_VERSION
        || !valid_checksum(lines[2])
        || !(lines[3] == "-" || valid_checksum(lines[3]))
    {
        return Err(Diagnostic::new(format!(
            "invalid output ownership record {}; do not overwrite these files",
            path.display()
        )));
    }
    Ok(Some(Marker {
        identity: lines[1].into(),
        elf: lines[2].into(),
        report: (lines[3] != "-").then(|| lines[3].into()),
    }))
}

fn no_symlinks(path: &Path) -> Result<()> {
    for ancestor in path.ancestors() {
        match fs::symlink_metadata(ancestor) {
            Ok(metadata) if metadata.file_type().is_symlink() => {
                return Err(Diagnostic::new(format!(
                    "refusing symlink in final output path: {}",
                    ancestor.display()
                )));
            }
            Ok(_) => (),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => (),
            Err(error) => return Err(Diagnostic::io(ancestor, error)),
        }
    }
    Ok(())
}

fn existing_file(path: &Path) -> Result<bool> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.is_file() => Ok(true),
        Ok(_) => Err(Diagnostic::new(format!(
            "final output is not a regular file: {}",
            path.display()
        ))),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(Diagnostic::io(path, error)),
    }
}

fn check_contents(path: &Path, expected: &str) -> Result<()> {
    if existing_file(path)? && read_checksum(path)? != expected {
        return Err(Diagnostic::new(format!(
            "refusing to replace or remove modified output {}; move it or restore the original before continuing",
            path.display()
        )));
    }
    Ok(())
}

fn validate_ownership(publication: &Publication) -> Result<Option<Marker>> {
    for path in [
        &publication.output_dir,
        &publication.elf,
        &publication.report,
        &publication.marker,
        &publication.lock_path(),
    ] {
        no_symlinks(path)?;
    }
    let marker = read_marker(&publication.marker)?;
    match &marker {
        Some(marker) => {
            if marker.identity != publication.owner {
                return Err(Diagnostic::new(format!(
                    "final output collision at {}: it belongs to a different project, firmware, board or mode; choose a different --output-dir or clean the original build",
                    publication.elf.display()
                )));
            }
            check_contents(&publication.elf, &marker.elf)?;
            match &marker.report {
                Some(expected) => check_contents(&publication.report, expected)?,
                None if publication.report_enabled && existing_file(&publication.report)? => {
                    return Err(Diagnostic::new(format!(
                        "refusing to overwrite unowned report {}",
                        publication.report.display()
                    )));
                }
                None => (),
            }
        }
        None => {
            if existing_file(&publication.elf)? {
                return Err(Diagnostic::new(format!(
                    "refusing to overwrite unowned firmware {}",
                    publication.elf.display()
                )));
            }
            if publication.report_enabled && existing_file(&publication.report)? {
                return Err(Diagnostic::new(format!(
                    "refusing to overwrite unowned report {}",
                    publication.report.display()
                )));
            }
        }
    }
    Ok(marker)
}

pub(super) fn preflight(plan: &BuildPlan, publication: &Publication) -> Result<()> {
    for path in [
        &publication.elf,
        &publication.report,
        &publication.marker,
        &publication.registry,
    ] {
        if plan.operations.iter().any(|operation| {
            operation.inputs.contains(path) && plan.artifact_producer(path).is_none()
        }) {
            return Err(Diagnostic::new(format!(
                "final output would overwrite an input: {}",
                path.display()
            )));
        }
    }
    validate_registry(plan, publication)?;
    validate_ownership(publication)?;
    Ok(())
}

fn validate_registry(plan: &BuildPlan, publication: &Publication) -> Result<()> {
    guarded_output(&publication.registry, &plan.build_dir)?;
    let contents = match fs::read(&publication.registry) {
        Ok(contents) => contents,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(Diagnostic::io(&publication.registry, error)),
    };
    let mut alternative = publication.clone();
    alternative.report_enabled = !publication.report_enabled;
    if contents != publication.registry_content()?.as_bytes()
        && contents != alternative.registry_content()?.as_bytes()
    {
        return Err(Diagnostic::new(
            "publication registry belongs to another destination",
        ));
    }
    Ok(())
}

pub(super) struct PublicationLock(PathBuf);
impl PublicationLock {
    pub(super) fn acquire(publication: &Publication) -> Result<Self> {
        no_symlinks(&publication.output_dir)?;
        fs::create_dir_all(&publication.output_dir)
            .map_err(|error| Diagnostic::io(&publication.output_dir, error))?;
        let path = publication.lock_path();
        fs::create_dir(&path).map_err(|error| {
            if error.kind() == std::io::ErrorKind::AlreadyExists {
                Diagnostic::new(format!("final output is locked: {}. Wait for the other build; remove the stale lock directory only if it stopped unexpectedly", path.display()))
            } else { Diagnostic::io(&path, error) }
        })?;
        Ok(Self(path))
    }
}
impl Drop for PublicationLock {
    fn drop(&mut self) {
        let _ = fs::remove_dir(&self.0);
    }
}

fn stage_dir(publication: &Publication) -> Result<PathBuf> {
    for _ in 0..64 {
        let serial = NEXT_STAGE.fetch_add(1, Ordering::Relaxed);
        let path = publication.output_dir.join(format!(
            ".{}.stage-{}-{serial}",
            publication.firmware_name,
            std::process::id()
        ));
        match fs::create_dir(&path) {
            Ok(()) => return Ok(path),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => (),
            Err(error) => return Err(Diagnostic::io(&path, error)),
        }
    }
    Err(Diagnostic::new(
        "cannot allocate a unique publication staging directory",
    ))
}

fn remove_stage(path: &Path) {
    for name in [
        "new.elf",
        "new.json",
        "new.marker",
        "old.elf",
        "old.json",
        "old.marker",
    ] {
        let _ = fs::remove_file(path.join(name));
    }
    let _ = fs::remove_dir(path);
}

pub(super) fn publish(plan: &BuildPlan, inputs: &[PathBuf]) -> Result<()> {
    let publication = plan
        .publication
        .as_ref()
        .ok_or_else(|| Diagnostic::new("missing publication destination"))?;
    let old = validate_ownership(publication)?;
    let stage = stage_dir(publication)?;
    let result = (|| -> Result<()> {
        let new_elf = stage.join("new.elf");
        fs::copy(&inputs[0], &new_elf).map_err(|error| Diagnostic::io(&inputs[0], error))?;
        let elf_checksum = read_checksum(&new_elf)?;
        let report_checksum = if publication.report_enabled {
            let report = stage.join("new.json");
            fs::copy(&inputs[1], &report).map_err(|error| Diagnostic::io(&inputs[1], error))?;
            Some(read_checksum(&report)?)
        } else {
            None
        };
        fs::write(
            stage.join("new.marker"),
            marker_text(
                &publication.owner,
                &elf_checksum,
                report_checksum.as_deref(),
            ),
        )
        .map_err(|error| Diagnostic::io(&stage, error))?;
        validate_registry(plan, publication)?;
        write_if_changed(
            &publication.registry,
            publication.registry_content()?.as_bytes(),
        )?;
        let mut moved = Vec::<(PathBuf, PathBuf)>::new();
        let mut installed = Vec::<PathBuf>::new();
        let change = (|| -> Result<()> {
            for (destination, backup, should_move) in [
                (&publication.elf, stage.join("old.elf"), old.is_some()),
                (
                    &publication.report,
                    stage.join("old.json"),
                    old.as_ref().is_some_and(|marker| marker.report.is_some()),
                ),
                (&publication.marker, stage.join("old.marker"), old.is_some()),
            ] {
                if should_move && existing_file(destination)? {
                    fs::rename(destination, &backup)
                        .map_err(|error| Diagnostic::io(destination, error))?;
                    moved.push((destination.clone(), backup));
                }
            }
            let mut replacements = vec![(new_elf, publication.elf.clone())];
            if publication.report_enabled {
                replacements.push((stage.join("new.json"), publication.report.clone()));
            }
            replacements.push((stage.join("new.marker"), publication.marker.clone()));
            for (source, destination) in replacements {
                fs::rename(&source, &destination)
                    .map_err(|error| Diagnostic::io(&destination, error))?;
                installed.push(destination);
            }
            Ok(())
        })();
        if let Err(error) = change {
            let mut restored = true;
            for path in installed.iter().rev() {
                if fs::remove_file(path).is_err() {
                    restored = false;
                }
            }
            for (destination, backup) in moved.iter().rev() {
                if fs::rename(backup, destination).is_err() {
                    restored = false;
                }
            }
            if !restored {
                return Err(Diagnostic::new(format!(
                    "{error}; recovery files remain in {}",
                    stage.display()
                )));
            }
            return Err(error);
        }
        Ok(())
    })();
    if result
        .as_ref()
        .err()
        .is_none_or(|error| !error.message.contains("recovery files remain"))
    {
        remove_stage(&stage);
    }
    result
}

pub(super) fn clean(publication: &Publication) -> Result<usize> {
    let Some(marker) = validate_ownership(publication)? else {
        return Ok(0);
    };
    let mut removed = 0;
    for path in std::iter::once(&publication.elf)
        .chain(marker.report.as_ref().map(|_| &publication.report))
        .chain(std::iter::once(&publication.marker))
    {
        match fs::remove_file(path) {
            Ok(()) => removed += 1,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => (),
            Err(error) => return Err(Diagnostic::io(path, error)),
        }
    }
    Ok(removed)
}

#[cfg(test)]
mod tests {
    use super::checksum;
    #[test]
    fn matches_posix_cksum() {
        assert_eq!(checksum(b""), "4294967295 0");
        assert_eq!(checksum(b"hello\n"), "3015617425 6");
        for (length, expected) in [
            (256, "4215202376 256"),
            (65_537, "4209397455 65537"),
            (1_048_576, "3018728591 1048576"),
        ] {
            let data = vec![0; length];
            assert_eq!(checksum(&data), expected);
        }
    }
}
