// Copyright 3Bian Limited and Contributors
// SPDX-License-Identifier: MIT

use std::collections::BTreeMap;
use std::path::{Component, Path};

use crate::configuration::{RawSdkCompatibility, SdkCompatibility};
use crate::diagnostic::{Diagnostic, Origin, Result};

pub(super) fn validate_sdk_compatibility(
    input: Option<&RawSdkCompatibility>,
    sdk: &Path,
    origins: &BTreeMap<String, Origin>,
) -> Result<Option<SdkCompatibility>> {
    let Some(input) = input else { return Ok(None) };
    let origin_for = |field: &str| {
        origins
            .get(field)
            .cloned()
            .unwrap_or_else(|| Origin::derived(field))
    };
    if input.integration.is_empty() || input.required_files.is_empty() {
        return Err(Diagnostic::new(
            "SDK compatibility needs an integration name and required files",
        )
        .at(origin_for("/sdk_compatibility")));
    }
    for (index, directory) in input.required_directories.iter().enumerate() {
        let origin = origin_for(&format!("/sdk_compatibility/required_directories/{index}"));
        let relative = Path::new(directory);
        if directory.is_empty()
            || relative
                .components()
                .any(|component| !matches!(component, Component::Normal(_)))
        {
            return Err(Diagnostic::new(
                "required SDK directories must be relative paths inside the SDK",
            )
            .at(origin));
        }
        let path = sdk.join(relative);
        if !path.is_dir() {
            return Err(Diagnostic::new(format!("incompatible {} SDK: required directory {} is missing; check the SDK path and initialise submodules if the checkout uses them", input.integration, path.display())).at(origin));
        }
    }
    for (index, required) in input.required_files.iter().enumerate() {
        let origin = origin_for(&format!("/sdk_compatibility/required_files/{index}/path"));
        let relative = Path::new(&required.path);
        if required.path.is_empty()
            || relative
                .components()
                .any(|component| !matches!(component, Component::Normal(_)))
        {
            return Err(Diagnostic::new(
                "required SDK paths must be relative files inside the SDK",
            )
            .at(origin));
        }
        let file = sdk.join(relative);
        if !file.is_file() {
            return Err(Diagnostic::new(format!(
                "incompatible {} SDK: required file {} is missing; check the SDK path and initialise submodules if the checkout uses them",
                input.integration, file.display()
            )).at(origin));
        }
        if let Some(marker) = &required.contains {
            if marker.is_empty() {
                return Err(Diagnostic::new("required SDK capability must not be empty").at(origin));
            }
            let contents = std::fs::read_to_string(&file)
                .map_err(|error| Diagnostic::io(&file, error).at(origin.clone()))?;
            if !contents.contains(marker) {
                return Err(Diagnostic::new(format!(
                    "incompatible {} SDK: {} lacks required capability {marker:?}; use an SDK that provides this capability",
                    input.integration, file.display()
                )).at(origin));
            }
        }
    }
    Ok(Some(SdkCompatibility {
        integration: input.integration.clone(),
    }))
}
