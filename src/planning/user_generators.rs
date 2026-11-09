// Copyright 3Bian Limited and Contributors
// SPDX-License-Identifier: MIT

use super::create_operation;
use crate::{
    build_graph::{BuildAction, BuildCommand, BuildOperationKind, BuildPlan},
    configuration::ResolvedProject,
    diagnostic::Result,
};

pub(super) fn add_generators(
    project: &ResolvedProject,
    plan: &mut BuildPlan,
) -> Result<Vec<String>> {
    let mut ids = Vec::new();
    for generator in &project.generators {
        let id = format!("generate.user.{}", generator.name);
        let command = BuildCommand {
            executable: generator.executable.clone(),
            argv: generator.arguments.clone(),
            cwd: generator.working_directory.clone(),
            environment: generator.environment.clone(),
            stdout: generator.stdout.clone(),
            stderr: generator.stderr.clone(),
        };
        plan.operations.push(create_operation(
            id.clone(),
            BuildOperationKind::Generate,
            BuildAction::Command {
                command,
                conditional: None,
            },
            generator.inputs.clone(),
            generator.outputs.clone(),
            generator
                .dependencies
                .iter()
                .map(|name| format!("generate.user.{name}"))
                .collect(),
            vec![generator.origin.clone()],
        )?);
        ids.push(id);
    }
    Ok(ids)
}
