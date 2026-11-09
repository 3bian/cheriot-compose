// Copyright 3Bian Limited and Contributors
// SPDX-License-Identifier: MIT

use serde::Serialize;

use crate::{
    build_graph::{MARKER_VERSION, Publication},
    diagnostic::Result,
};

use super::{quote_shell_argument, quoted};

#[derive(Serialize)]
pub(super) struct PublicationRenderModel {
    output_dir: String,
    elf: String,
    report_path: String,
    marker: String,
    identity: String,
    report_enabled: &'static str,
    lock_path: String,
    registry: String,
    registry_content: String,
    registry_enabled: String,
    registry_disabled: String,
    marker_version: String,
}
impl PublicationRenderModel {
    pub(super) fn new(publication: &Publication) -> Result<Self> {
        let (enabled, disabled) = publication.registry_variants()?;
        Ok(Self {
            output_dir: quoted(&publication.output_dir),
            elf: quoted(&publication.elf),
            report_path: quoted(&publication.report),
            marker: quoted(&publication.marker),
            identity: quote_shell_argument(&publication.owner),
            report_enabled: if publication.report_enabled {
                "true"
            } else {
                "false"
            },
            lock_path: quoted(&publication.lock_path()),
            registry: quoted(&publication.registry),
            registry_content: quote_shell_argument(&publication.registry_content()?),
            registry_enabled: quote_shell_argument(&enabled),
            registry_disabled: quote_shell_argument(&disabled),
            marker_version: quote_shell_argument(MARKER_VERSION),
        })
    }
}
