// Copyright 3Bian Limited and Contributors
// SPDX-License-Identifier: MIT

use serde::Serialize;

#[derive(Serialize)]
pub(super) struct ReportModel {
    pub(super) source_report: String,
}
