// Copyright 3Bian Limited and Contributors
// SPDX-License-Identifier: MIT

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use super::calculate_fingerprint;
use crate::configuration::ResolvedProject;

pub const MARKER_VERSION: &str = "cheriot-compose-output-v1";

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Publication {
    pub identity: String,
    pub owner: String,
    pub firmware_name: String,
    pub output_dir: PathBuf,
    pub elf: PathBuf,
    pub report: PathBuf,
    pub report_enabled: bool,
    pub marker: PathBuf,
    pub registry: PathBuf,
}

impl Publication {
    pub fn registry_content(&self) -> crate::diagnostic::Result<String> {
        Ok(serde_json::to_string(self)?)
    }

    pub fn registry_variants(&self) -> crate::diagnostic::Result<(String, String)> {
        let mut enabled = self.clone();
        enabled.report_enabled = true;
        let mut disabled = self.clone();
        disabled.report_enabled = false;
        Ok((enabled.registry_content()?, disabled.registry_content()?))
    }

    pub fn for_project(project: &ResolvedProject) -> Self {
        Self::new(
            &project.build_dir,
            &project.output_dir,
            &project.firmware.name,
            &project.publication_identity,
            &project.publication_owner,
            project.firmware.compartment_report,
        )
    }

    pub fn new(
        build_dir: &Path,
        output_dir: &Path,
        name: &str,
        identity: &str,
        owner: &str,
        report_enabled: bool,
    ) -> Self {
        let destination_id = calculate_fingerprint(output_dir.to_string_lossy().as_bytes());
        Self {
            identity: identity.into(),
            owner: owner.into(),
            firmware_name: name.into(),
            output_dir: output_dir.into(),
            elf: output_dir.join(format!("{name}.elf")),
            report: output_dir.join(format!("{name}.json")),
            report_enabled,
            marker: output_dir.join(format!(".{name}.cheriot-compose-owner")),
            registry: build_dir.join(format!(".publication-{identity}-{destination_id}.json")),
        }
    }

    pub fn outputs(&self) -> Vec<PathBuf> {
        let mut outputs = vec![self.elf.clone()];
        if self.report_enabled {
            outputs.push(self.report.clone());
        }
        outputs.extend([self.marker.clone(), self.registry.clone()]);
        outputs
    }

    pub fn lock_path(&self) -> PathBuf {
        self.output_dir
            .join(format!(".{}.cheriot-compose-lock", self.firmware_name))
    }
}
