// Copyright 3Bian Limited and Contributors
// SPDX-License-Identifier: MIT

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

use crate::build_graph::{
    BuildAction, BuildCommand, BuildOperation, BuildOperationKind, BuildPlan, OperationPolicy,
    OutputHandling,
};
use crate::calculations::sdk;
use crate::configuration::ResolvedProject;
use crate::diagnostic::{Origin, Result, Sourced};

mod component_operations;
mod component_outputs;
pub(crate) use component_outputs::component_output;
mod firmware_operations;
mod generated_operations;
mod packaging_operations;
pub mod runner;
mod rust_operations;
mod user_generators;

fn arguments_from_strings(values: &[&str]) -> Vec<String> {
    values.iter().map(|value| value.to_string()).collect()
}

fn create_command(
    project: &ResolvedProject,
    executable: String,
    argv: Vec<String>,
) -> BuildCommand {
    create_sourced_command(
        project,
        executable,
        argv.into_iter()
            .map(|value| Sourced::derived(value, "SDK command construction"))
            .collect(),
    )
}

fn create_sourced_command(
    project: &ResolvedProject,
    executable: String,
    argv: Vec<Sourced<String>>,
) -> BuildCommand {
    BuildCommand {
        executable,
        argv,
        cwd: project.root.clone(),
        environment: BTreeMap::new(),
        stdout: None,
        stderr: None,
    }
}

fn create_operation(
    id: String,
    kind: BuildOperationKind,
    action: BuildAction,
    inputs: Vec<PathBuf>,
    outputs: Vec<PathBuf>,
    dependencies: Vec<String>,
    provenance: Vec<Origin>,
) -> Result<BuildOperation> {
    let policy = OperationPolicy::for_action(&kind, &action);
    Ok(BuildOperation {
        id,
        kind,
        action,
        policy,
        inputs,
        outputs,
        dependencies,
        depfile: None,
        provenance,
        fingerprint: String::new(),
    })
}

pub fn create_build_plan(project: &ResolvedProject) -> Result<BuildPlan> {
    create_plan(project, false)
}

pub fn create_packaging_plan(project: &ResolvedProject) -> Result<BuildPlan> {
    create_plan(project, true)
}

fn create_plan(project: &ResolvedProject, include_packaging: bool) -> Result<BuildPlan> {
    let recipes = &project.components;
    let profile = project
        .sdk_compatibility
        .as_ref()
        .map(|compatibility| {
            format!(
                "{} (required SDK capabilities verified)",
                compatibility.integration
            )
        })
        .unwrap_or_else(|| format!("CHERIoT SDK rules based on {}", sdk::SDK_REVISION));
    let notes = vec![
        "Scope: selected firmware closure; reference compdb may also contain unused SDK targets."
            .into(),
        "Compiler probes are unresolved until execution. Conditional flags are explicit; no compiler is run during planning."
            .into(),
    ];
    let mut plan = BuildPlan {
        schema: "cheriot-compose.plan/v1".into(),
        profile,
        notes,
        toolchain: Some(project.toolchain.clone()),
        build_dir: project.build_dir.clone(),
        state_key: Some(project.publication_identity.clone()),
        state_owner: Some(project.publication_owner.clone()),
        publication: Some(crate::build_graph::Publication::for_project(project)),
        operations: Vec::new(),
        probes: BTreeSet::new(),
    };
    let mut generated = user_generators::add_generators(project, &mut plan)?;
    generated.extend(generated_operations::add_generated_operations(
        project, recipes, &mut plan,
    )?);
    let component_outputs =
        component_operations::add_component_operations(project, recipes, &generated, &mut plan)?;
    firmware_operations::add_firmware_operations(
        project,
        recipes,
        generated,
        component_outputs,
        &mut plan,
    )?;
    if project.toolchain.rust_compiler.is_none()
        && plan.operations.iter().any(|operation| {
            matches!(
                operation.kind,
                BuildOperationKind::RustCompile | BuildOperationKind::CargoBuild
            )
        })
    {
        plan.notes.push("Firmware Rust compiler is unresolved; set toolchain.rust_compiler or RUSTC before execution.".into());
    }
    if include_packaging {
        packaging_operations::add_packaging(project, &mut plan)?;
    }
    finalize_plan(project, &mut plan)?;
    Ok(plan)
}

fn finalize_plan(project: &ResolvedProject, plan: &mut BuildPlan) -> Result<()> {
    add_ownership_record(plan)?;
    protect_project_inputs(project, plan)?;
    for operation in &mut plan.operations {
        operation.update_fingerprint()?;
    }
    plan.validate()
}

fn protect_project_inputs(project: &ResolvedProject, plan: &BuildPlan) -> Result<()> {
    let generated: BTreeSet<_> = project
        .generators
        .iter()
        .flat_map(|generator| generator.outputs.iter().cloned())
        .collect();
    let mut protected: BTreeSet<PathBuf> = project.config_files.iter().cloned().collect();
    for recipe in &project.components {
        protected.insert(project.sdk.join(&recipe.script));
        for source in &recipe.sources {
            if !generated.contains(&source.path) {
                protected.insert(source.path.clone());
            }
            protected.extend(
                source
                    .force_includes
                    .iter()
                    .map(|include| include.path.clone())
                    .filter(|path| !generated.contains(path)),
            );
        }
        protected.extend(
            recipe
                .force_includes
                .iter()
                .map(|include| include.path.clone())
                .filter(|path| !generated.contains(path)),
        );
    }
    for generator in &project.generators {
        protected.extend(
            generator
                .inputs
                .iter()
                .filter(|path| !generated.contains(*path))
                .cloned(),
        );
    }
    for source in &project.clock_sources {
        protected.insert(source.include_dir.join(&source.header));
    }
    for fragment in &project.board.linker_fragments {
        if let crate::configuration::LinkerFragmentSource::File { path } = &fragment.source {
            protected.insert(path.clone());
        }
    }
    for output in plan
        .operations
        .iter()
        .flat_map(|operation| operation.outputs.iter().chain(operation.depfile.iter()))
    {
        if protected.contains(output) {
            return Err(crate::diagnostic::Diagnostic::new(format!(
                "build output would overwrite a source or configuration file: {}",
                output.display()
            )));
        }
    }
    Ok(())
}

fn add_ownership_record(plan: &mut BuildPlan) -> Result<()> {
    let publication = plan.publication.as_ref().ok_or_else(|| {
        crate::diagnostic::Diagnostic::new("firmware publication destination is missing")
    })?;
    let outputs: BTreeSet<_> = plan
        .operations
        .iter()
        .filter(|operation| !matches!(operation.action, BuildAction::Publish))
        .flat_map(|operation| {
            operation
                .outputs
                .iter()
                .chain(operation.depfile.iter())
                .cloned()
        })
        .filter(|path| path.starts_with(&plan.build_dir))
        .collect();
    let content = serde_json::to_string(&crate::build_graph::OwnershipRecord {
        owner: publication.owner.clone(),
        outputs,
    })?;
    let fingerprint = crate::build_graph::calculate_fingerprint(content.as_bytes());
    let path = plan.build_dir.join(format!(
        ".outputs-{}-{fingerprint}.json",
        publication.identity
    ));
    let dependencies = plan
        .operations
        .iter()
        .map(|operation| operation.id.clone())
        .collect();
    let artifact = crate::render::firmware::GeneratedArtifact::literal(
        "owned intermediate paths",
        path,
        content,
        vec![],
        vec![Origin::derived("owned intermediate artifacts")],
    );
    let mut operation =
        generated_operations::artifact_operation("record.outputs".into(), artifact, dependencies)?;
    operation.policy.output_handling = OutputHandling::OwnershipRecord;
    plan.operations.push(operation);
    Ok(())
}
