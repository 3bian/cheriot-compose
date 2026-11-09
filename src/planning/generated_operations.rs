// Copyright 3Bian Limited and Contributors
// SPDX-License-Identifier: MIT

use std::collections::BTreeMap;

use super::{create_command, create_operation};
use crate::build_graph::{BuildAction, BuildOperation, BuildOperationKind, BuildPlan};
use crate::calculations::{layout, sdk};
use crate::configuration::ComponentRecipe;
use crate::configuration::{ComponentRole, ResolvedProject};
use crate::diagnostic::{Diagnostic, Origin, Result};
use crate::render::firmware::{self, GeneratedArtifact};

pub(super) fn artifact_operation(
    id: String,
    artifact: GeneratedArtifact,
    dependencies: Vec<String>,
) -> Result<BuildOperation> {
    let template_fingerprint = artifact.template_fingerprint()?;
    create_operation(
        id,
        BuildOperationKind::Generate,
        BuildAction::Generate {
            content: artifact.content,
            template: artifact.name,
            template_fingerprint,
        },
        artifact.inputs,
        vec![artifact.path],
        dependencies,
        artifact.provenance,
    )
}

pub(super) fn register_artifact(
    plan: &mut BuildPlan,
    id: String,
    artifact: GeneratedArtifact,
    dependencies: Vec<String>,
) -> Result<()> {
    plan.operations
        .push(artifact_operation(id, artifact, dependencies)?);
    Ok(())
}

pub(super) fn add_generated_operations(
    project: &ResolvedProject,
    recipes: &[ComponentRecipe],
    plan: &mut BuildPlan,
) -> Result<Vec<String>> {
    let mut generated = Vec::new();
    let board = GeneratedArtifact::literal(
        "resolved board JSON",
        project.build_dir.join("board.json"),
        serde_json::to_string_pretty(&project.audit_board)?,
        project.config_files.clone(),
        vec![Origin::derived("RTOS cheriot.board.file")],
    );
    register_artifact(plan, "generate.board.json".into(), board, vec![])?;
    let segments = layout::firmware_linker_segments(
        &project.board,
        project.build_dir.join(format!(
            "{}-firmware.rocode.ldscript",
            project.firmware.name
        )),
        project.build_dir.join(format!(
            "{}-firmware.rwdata.ldscript",
            project.firmware.name
        )),
    );
    let mut fragment_templates = BTreeMap::new();
    for segment in &segments {
        if let crate::configuration::LinkerFragmentSource::Generated { template, .. } =
            &segment.source
            && !fragment_templates.contains_key(template)
        {
            let contents = std::fs::read_to_string(template).map_err(|error| {
                let diagnostic = Diagnostic::io(template, error);
                match &segment.origin {
                    Some(origin) => diagnostic.at(origin.clone()),
                    None => diagnostic,
                }
            })?;
            fragment_templates.insert(template.clone(), contents);
        }
    }
    let inputs = firmware::FirmwareInputs {
        components: sdk::link_input_order(recipes)
            .into_iter()
            .map(|recipe| firmware::FirmwareComponent {
                name: recipe.name.clone(),
                kind: recipe.kind,
                object: super::component_output(project, recipe),
            })
            .collect(),
        segments,
        fragment_templates,
    };
    for artifact in firmware::generate(project, &inputs)? {
        let id = format!("generate.{}", artifact.name);
        generated.push(id.clone());
        register_artifact(plan, id, artifact, vec![])?;
    }
    if recipes
        .iter()
        .any(|recipe| recipe.role == Some(ComponentRole::WallClock))
    {
        let model = crate::render::firmware::clock_render_model::ClockRenderModel::from_sources(
            &project.clock_sources,
        );
        let template = include_str!("../../templates/rtc_sources.hh.mustache");
        let mut inputs = project.config_files.clone();
        inputs.extend(
            project
                .clock_sources
                .iter()
                .map(|source| source.include_dir.join(&source.header)),
        );
        let (artifact, _) = GeneratedArtifact::render(
            "rtc_sources.hh",
            project.build_dir.join("rtc_sources.hh"),
            template,
            &model,
            &Default::default(),
            inputs,
            project
                .clock_sources
                .iter()
                .map(|source| source.origin.clone())
                .collect(),
        )?;
        let id = "generate.rtc_sources.hh".to_string();
        register_artifact(plan, id.clone(), artifact, vec![])?;
        generated.push(id);
    }
    if let Some(suite) = &project.test_suite {
        let model =
            crate::render::firmware::test_suite_render_model::TestSuiteRenderModel::new(suite)?;
        for (name, template) in [
            (
                "tests-prototypes.h",
                include_str!("../../templates/sdk/tests-prototypes.h.mustache"),
            ),
            (
                "tests-all.inc",
                include_str!("../../templates/sdk/tests-all.inc.mustache"),
            ),
        ] {
            let id = format!("generate.{name}");
            let (artifact, _) = GeneratedArtifact::render(
                name,
                project.build_dir.join(name),
                template,
                &model,
                &Default::default(),
                project.config_files.clone(),
                suite.cases.iter().map(|case| case.origin.clone()).collect(),
            )?;
            register_artifact(plan, id.clone(), artifact, vec![])?;
            generated.push(id);
        }
    }
    if recipes
        .iter()
        .flat_map(|recipe| &recipe.sources)
        .any(|source| source.rtos_git_description.is_some())
    {
        let arguments = vec![
            "-C".into(),
            crate::build_graph::path_to_string(&project.sdk)?,
            "describe".into(),
            "--always".into(),
            "--dirty".into(),
        ];
        plan.operations.push(create_operation(
            "generate.rtos-git-description.h".into(),
            BuildOperationKind::Generate,
            BuildAction::GitDescription {
                command: create_command(project, "git".into(), arguments),
            },
            vec![],
            vec![project.build_dir.join("rtos-git-description.h")],
            vec![],
            vec![Origin::derived("cheriot.define-rtos-git-description")],
        )?);
    }
    Ok(generated)
}
