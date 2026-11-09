// Copyright 3Bian Limited and Contributors
// SPDX-License-Identifier: MIT

use std::path::Path;

use super::quote_shell_argument;
use crate::{
    build_graph::{BuildAction, BuildPlan, OutputHandling},
    diagnostic::{Diagnostic, Result},
    render::template,
};

mod owned_record_model;
mod prelude_model;
mod publication_render_model;
mod publish_model;
mod report_model;
mod state_identity_model;

use owned_record_model::OwnedRecordModel;
use prelude_model::PreludeModel;
use publication_render_model::PublicationRenderModel;
use publish_model::PublishModel;
use report_model::ReportModel;
use state_identity_model::StateIdentityModel;

fn quoted(path: &Path) -> String {
    quote_shell_argument(&path.to_string_lossy())
}
pub(super) fn prelude(plan: &BuildPlan) -> Result<String> {
    let publication = plan
        .publication
        .as_ref()
        .map(PublicationRenderModel::new)
        .transpose()?;
    let owned_record = plan
        .operations
        .iter()
        .find(|operation| operation.policy.output_handling == OutputHandling::OwnershipRecord)
        .and_then(|operation| {
            if let BuildAction::Generate { content, .. } = &operation.action {
                Some(OwnedRecordModel {
                    path: quoted(&operation.outputs[0]),
                    content: quote_shell_argument(content),
                })
            } else {
                None
            }
        });
    let model = PreludeModel {
        build_lock: quoted(&plan.build_dir.join(".cheriot-compose.lock")),
        build_dir: quoted(&plan.build_dir),
        publication,
        owned_record,
        state_identity: plan
            .identity_path()
            .zip(plan.state_owner.as_ref())
            .map(|(path, owner)| StateIdentityModel {
                identity_path: quoted(&path),
                owner: quote_shell_argument(owner),
            }),
    };
    template::render_text(
        include_str!("../../templates/publication-prelude.mustache"),
        &serde_json::to_value(model)?,
    )
}
pub(super) fn publish(plan: &BuildPlan, inputs: &[std::path::PathBuf]) -> Result<String> {
    let publication = plan
        .publication
        .as_ref()
        .ok_or_else(|| Diagnostic::new("missing publication destination"))?;
    let template = publication
        .output_dir
        .join(format!(".{}.stage.XXXXXX", publication.firmware_name));
    let model = PublishModel {
        publication: PublicationRenderModel::new(publication)?,
        stage_template: quoted(&template),
        source_elf: quoted(&inputs[0]),
        report: publication.report_enabled.then(|| ReportModel {
            source_report: quoted(&inputs[1]),
        }),
    };
    template::render_text(
        include_str!("../../templates/publication-action.mustache"),
        &serde_json::to_value(model)?,
    )
}
