// Copyright 3Bian Limited and Contributors
// SPDX-License-Identifier: MIT

use std::path::PathBuf;

use super::component_operations::object_path;
use super::{create_operation, create_sourced_command};
use crate::build_graph::{
    BuildAction, BuildOperationKind, BuildPlan, RequiredTool, path_to_string,
};
use crate::configuration::ComponentRecipe;
use crate::configuration::{ResolvedProject, ResolvedSource};
use crate::diagnostic::{Diagnostic, Origin, Result, Sourced};

pub(super) fn add_rust_source(
    project: &ResolvedProject,
    recipe: &ComponentRecipe,
    source: &ResolvedSource,
    index: usize,
    generated: &[String],
    plan: &mut BuildPlan,
) -> Result<(PathBuf, String)> {
    if !source.flags.is_empty()
        || !source.c_flags.is_empty()
        || !source.cpp_flags.is_empty()
        || !source.cargo_flags.is_empty()
        || !source.includes.is_empty()
        || !source.force_includes.is_empty()
        || source.rtos_git_description.is_some()
    {
        return Err(Diagnostic::new(format!(
            "Rust source {} must use rust_flags rather than C or C++ source settings",
            source.path.display()
        )));
    }
    let archive = object_path(project, recipe, &source.path)?.with_extension("a");
    let depfile = archive.with_extension("a.d");
    let mut arguments: Vec<_> = [
        format!("-Ctarget-cpu={}", project.board.cpu),
        "-C".into(),
        "opt-level=3".into(),
        "--target=riscv32cheriot-unknown-cheriotrtos".into(),
        "-C".into(),
        "opt-level=3".into(),
        "--crate-type=staticlib".into(),
        format!("--emit=link,dep-info={}", path_to_string(&depfile)?),
    ]
    .into_iter()
    .map(|value| Sourced::derived(value, "RTOS standalone Rust rule"))
    .collect();
    arguments.extend(
        project
            .toolchain
            .rust_flags
            .iter()
            .enumerate()
            .map(|(index, value)| {
                let origin = project
                    .provenance
                    .get(&format!("/toolchain/rust_flags/{index}"))
                    .cloned()
                    .unwrap_or_else(|| Origin::derived("toolchain Rust flags"));
                Sourced::new(value.clone(), origin)
            }),
    );
    arguments.extend(
        recipe
            .rust_flags
            .iter()
            .chain(&source.rust_flags)
            .map(|flag| Sourced::new(flag.value.clone(), flag.origin.clone())),
    );
    arguments.extend([
        Sourced::derived("-o".into(), "Rust archive output"),
        Sourced::derived(path_to_string(&archive)?, "Rust archive output"),
        Sourced::derived(path_to_string(&source.path)?, "Rust source"),
    ]);
    let id = format!("compile.{}.{index}", recipe.name);
    let mut inputs = vec![source.path.clone()];
    inputs.extend(project.config_files.clone());
    let mut operation = create_operation(
        id.clone(),
        BuildOperationKind::RustCompile,
        BuildAction::Command {
            command: create_sourced_command(
                project,
                project
                    .toolchain
                    .rust_compiler
                    .clone()
                    .unwrap_or_else(|| "<unresolved firmware Rust compiler>".into()),
                arguments,
            ),
            conditional: None,
        },
        inputs,
        vec![archive.clone()],
        generated.to_vec(),
        vec![Origin::derived("RTOS cheriot.rust standalone rule")],
    )?;
    operation.depfile = Some(depfile);
    plan.operations.push(operation);
    Ok((archive, id))
}

pub(super) fn add_cargo_source(
    project: &ResolvedProject,
    recipe: &ComponentRecipe,
    source: &ResolvedSource,
    index: usize,
    generated: &[String],
    plan: &mut BuildPlan,
) -> Result<(PathBuf, String)> {
    if !source.flags.is_empty()
        || !source.c_flags.is_empty()
        || !source.cpp_flags.is_empty()
        || !source.includes.is_empty()
        || !source.force_includes.is_empty()
        || source.rtos_git_description.is_some()
    {
        return Err(Diagnostic::new(format!(
            "Cargo manifest {} must use rust_flags or cargo_flags rather than C or C++ source settings",
            source.path.display()
        )));
    }
    let package = source
        .cargo_package
        .as_ref()
        .ok_or_else(|| Diagnostic::new("Cargo source was not resolved"))?;
    let flags: Vec<_> = recipe
        .cargo_flags
        .iter()
        .chain(&source.cargo_flags)
        .collect();
    let cargo_name = format!(
        "cargo-{}-{}-{index}",
        recipe.name, project.configuration_identity
    );
    let metadata = project
        .build_dir
        .join(format!("{cargo_name}.metadata.json"));
    let output_dir = project.build_dir.join(cargo_name);
    let archive = output_dir
        .join("riscv32cheriot-unknown-cheriotrtos")
        .join(
            if project.mode == crate::configuration::BuildMode::Release {
                "release"
            } else {
                "debug"
            },
        )
        .join(format!("lib{}.a", package.library_name));
    let mut rust_flags = vec![
        format!("-Ctarget-cpu={}", project.board.cpu),
        "-Copt-level=3".into(),
    ];
    rust_flags.extend(project.toolchain.rust_flags.iter().cloned());
    rust_flags.extend(
        recipe
            .rust_flags
            .iter()
            .chain(&source.rust_flags)
            .map(|flag| flag.value.clone()),
    );
    if rust_flags.iter().any(|flag| flag.contains('\u{1f}')) {
        return Err(Diagnostic::new(
            "Rust flags cannot contain the Cargo flag separator",
        ));
    }
    let rust_environment = std::collections::BTreeMap::from([
        (
            "RUSTC".into(),
            project
                .toolchain
                .rust_compiler
                .clone()
                .unwrap_or_else(|| "<unresolved firmware Rust compiler>".into()),
        ),
        ("CARGO_ENCODED_RUSTFLAGS".into(), rust_flags.join("\u{1f}")),
    ]);
    let manifest_path = path_to_string(&source.path)?;
    let mut metadata_command = create_sourced_command(
        project,
        project.toolchain.cargo.clone(),
        [
            "metadata".into(),
            "--no-deps".into(),
            "--format-version=1".into(),
            "--manifest-path".into(),
            manifest_path.clone(),
        ]
        .into_iter()
        .map(|value| Sourced::derived(value, "Cargo metadata"))
        .collect(),
    );
    metadata_command.argv.extend(
        flags
            .iter()
            .map(|flag| Sourced::new(flag.value.clone(), flag.origin.clone())),
    );
    metadata_command.environment = rust_environment.clone();
    metadata_command.stdout = Some(metadata.clone());
    let metadata_id = format!("metadata.{}.{index}", recipe.name);
    let mut inputs = vec![source.path.clone()];
    inputs.extend(project.config_files.clone());
    let mut metadata_operation = create_operation(
        metadata_id.clone(),
        BuildOperationKind::Generate,
        BuildAction::Command {
            command: metadata_command,
            conditional: None,
        },
        inputs.clone(),
        vec![metadata.clone()],
        generated.to_vec(),
        vec![Origin::derived("RTOS cheriot.rust.crate metadata")],
    )?;
    metadata_operation
        .policy
        .required_tools
        .extend([RequiredTool::Cargo, RequiredTool::FirmwareRust]);
    plan.operations.push(metadata_operation);
    let mut arguments: Vec<_> = [
        "build".into(),
        "--lib".into(),
        "--manifest-path".into(),
        manifest_path,
        "--target-dir".into(),
        path_to_string(&output_dir)?,
        "--target=riscv32cheriot-unknown-cheriotrtos".into(),
    ]
    .into_iter()
    .map(|value| Sourced::derived(value, "RTOS Cargo rule"))
    .collect();
    if project.mode == crate::configuration::BuildMode::Release {
        arguments.push(Sourced::derived("--release".into(), "release build mode"));
    }
    arguments.extend(
        flags
            .into_iter()
            .map(|flag| Sourced::new(flag.value.clone(), flag.origin.clone())),
    );
    let mut command = create_sourced_command(project, project.toolchain.cargo.clone(), arguments);
    command.environment = rust_environment;
    inputs.push(metadata);
    let id = format!("compile.{}.{index}", recipe.name);
    plan.operations.push(create_operation(
        id.clone(),
        BuildOperationKind::CargoBuild,
        BuildAction::Command {
            command,
            conditional: None,
        },
        inputs,
        vec![archive.clone()],
        vec![metadata_id],
        vec![Origin::derived("RTOS cheriot.rust.crate build")],
    )?);
    Ok((archive, id))
}
