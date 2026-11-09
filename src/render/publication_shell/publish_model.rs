// Copyright 3Bian Limited and Contributors
// SPDX-License-Identifier: MIT

use serde::Serialize;

use super::{publication_render_model::PublicationRenderModel, report_model::ReportModel};

#[derive(Serialize)]
pub(super) struct PublishModel {
    #[serde(flatten)]
    pub(super) publication: PublicationRenderModel,
    pub(super) stage_template: String,
    pub(super) source_elf: String,
    pub(super) report: Option<ReportModel>,
}
