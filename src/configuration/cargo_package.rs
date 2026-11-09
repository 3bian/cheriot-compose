// Copyright 3Bian Limited and Contributors
// SPDX-License-Identifier: MIT

use std::path::Path;

use serde::Serialize;

use super::CompilerFlag;
use crate::diagnostic::{Diagnostic, Result};

#[derive(Clone, Debug, Serialize)]
pub struct CargoPackage {
    pub package_name: String,
    pub library_name: String,
}

pub(super) fn read_cargo_package(path: &Path) -> Result<CargoPackage> {
    let text = std::fs::read_to_string(path).map_err(|error| Diagnostic::io(path, error))?;
    let manifest: toml::Value = toml::from_str(&text).map_err(|error| {
        Diagnostic::new(format!(
            "{}: invalid Cargo manifest: {error}",
            path.display()
        ))
    })?;
    let package_name = manifest
        .get("package")
        .and_then(|package| package.get("name"))
        .and_then(toml::Value::as_str)
        .ok_or_else(|| {
            Diagnostic::new(format!(
                "{}: Cargo source needs a package name",
                path.display()
            ))
        })?;
    let library = manifest.get("lib").ok_or_else(|| {
        Diagnostic::new(format!(
            "{}: Cargo package needs a [lib] staticlib",
            path.display()
        ))
    })?;
    if !library
        .get("crate-type")
        .and_then(toml::Value::as_array)
        .is_some_and(|types| types.iter().any(|kind| kind.as_str() == Some("staticlib")))
    {
        return Err(Diagnostic::new(format!(
            "{}: [lib] crate-type must include staticlib",
            path.display()
        )));
    }
    let library_name = library
        .get("name")
        .and_then(toml::Value::as_str)
        .unwrap_or(package_name)
        .replace('-', "_");
    if !super::resolution::is_identifier(&library_name) {
        return Err(Diagnostic::new(format!(
            "{}: invalid Cargo library name",
            path.display()
        )));
    }
    Ok(CargoPackage {
        package_name: package_name.into(),
        library_name,
    })
}

pub(super) fn validate_cargo_flags(flags: &[CompilerFlag]) -> Result<()> {
    let mut expect_feature = false;
    for flag in flags {
        if expect_feature {
            if flag.value.is_empty() || flag.value.starts_with('-') {
                return Err(
                    Diagnostic::new("--features needs a feature list").at(flag.origin.clone())
                );
            }
            expect_feature = false;
        } else if flag.value == "--features" {
            expect_feature = true;
        } else if ![
            "--no-default-features",
            "--all-features",
            "--locked",
            "--offline",
            "--frozen",
        ]
        .contains(&flag.value.as_str())
            && !flag
                .value
                .strip_prefix("--features=")
                .is_some_and(|value| !value.is_empty())
        {
            return Err(Diagnostic::new(
                "unsupported Cargo option; use features, --locked, --offline or --frozen",
            )
            .at(flag.origin.clone()));
        }
    }
    if expect_feature {
        return Err(Diagnostic::new("--features needs a feature list"));
    }
    Ok(())
}
