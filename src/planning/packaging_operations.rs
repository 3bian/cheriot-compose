// Copyright 3Bian Limited and Contributors
// SPDX-License-Identifier: MIT

use std::path::PathBuf;

use super::{create_command, create_operation};
use crate::build_graph::{
    BuildAction, BuildCommand, BuildOperationKind, BuildPlan, RequiredTool, path_to_string,
};
use crate::configuration::{PackagingKind, ResolvedProject};
use crate::diagnostic::{Diagnostic, Origin, Result};

pub(super) fn add_packaging(project: &ResolvedProject, plan: &mut BuildPlan) -> Result<()> {
    let kind = project.board.packaging.ok_or_else(|| {
        Diagnostic::new("the selected board does not declare a supported packaging format")
    })?;
    let firmware = plan
        .publication
        .as_ref()
        .map(|publication| publication.elf.clone())
        .ok_or_else(|| Diagnostic::new("the firmware plan has no ELF output"))?;
    let suffix = |extension: &str| {
        project.build_dir.join(format!(
            "{}--{}.elf.{extension}",
            project.firmware.name, project.configuration_identity
        ))
    };
    match kind {
        PackagingKind::SonataLegacy => {
            let binary = suffix("bin");
            add_command(
                project,
                plan,
                "package.binary",
                create_command(
                    project,
                    project.toolchain.object_copy.clone(),
                    vec![
                        "-O".into(),
                        "binary".into(),
                        path_to_string(&firmware)?,
                        path_to_string(&binary)?,
                    ],
                ),
                firmware.clone(),
                binary.clone(),
                vec!["publish.firmware".into()],
            )?;
            let output = suffix("uf2");
            add_command(
                project,
                plan,
                "package.uf2",
                create_command(
                    project,
                    project.toolchain.uf2_converter.clone(),
                    vec![
                        path_to_string(&binary)?,
                        "-b0x00101000".into(),
                        "-co".into(),
                        path_to_string(&output)?,
                    ],
                ),
                binary,
                output,
                vec!["package.binary".into()],
            )?;
            if crate::planning::runner::uses_sonata_sdk_runner(project) {
                add_runner_image(project, plan, suffix("uf2"), "package.uf2")?;
            }
        }
        PackagingKind::SonataV1 => {
            let stripped = suffix("strip");
            add_command(
                project,
                plan,
                "package.strip",
                create_command(
                    project,
                    project.toolchain.strip.clone(),
                    vec![
                        path_to_string(&firmware)?,
                        "-o".into(),
                        path_to_string(&stripped)?,
                    ],
                ),
                firmware.clone(),
                stripped.clone(),
                vec!["publish.firmware".into()],
            )?;
            for (slot, address) in [(1, "0x00000000"), (2, "0x10000000"), (3, "0x20000000")] {
                let output = suffix(&format!("slot{slot}.uf2"));
                add_command(
                    project,
                    plan,
                    &format!("package.slot{slot}"),
                    create_command(
                        project,
                        project.toolchain.uf2_converter.clone(),
                        vec![
                            path_to_string(&stripped)?,
                            format!("-b{address}"),
                            "-f0x6CE29E60".into(),
                            "-co".into(),
                            path_to_string(&output)?,
                        ],
                    ),
                    stripped.clone(),
                    output,
                    vec!["package.strip".into()],
                )?;
            }
            if crate::planning::runner::uses_sonata_sdk_runner(project) {
                add_runner_image(project, plan, suffix("slot1.uf2"), "package.slot1")?;
            }
        }
        PackagingKind::MicrosoftSafe => {
            let directory = project.build_dir.join("firmware");
            let binary = directory.join("cpu0_iram.bin");
            add_command(
                project,
                plan,
                "package.binary",
                create_command(
                    project,
                    project.toolchain.object_copy.clone(),
                    vec![
                        "-O".into(),
                        "binary".into(),
                        path_to_string(&firmware)?,
                        path_to_string(&binary)?,
                    ],
                ),
                firmware.clone(),
                binary.clone(),
                vec!["publish.firmware".into()],
            )?;
            for (word_bytes, suffix) in [(4, ""), (8, "64")] {
                let boot = directory.join(format!("cpu0_irom{suffix}.vhx"));
                let zero = if word_bytes == 4 {
                    "00000000\n"
                } else {
                    "0000000000000000\n"
                };
                let jump = if word_bytes == 4 {
                    "7813f06f\n"
                } else {
                    "000000007813f06f\n"
                };
                let content = format!("{}{jump}", zero.repeat(128 / usize::from(word_bytes)));
                let artifact = crate::render::firmware::GeneratedArtifact::literal(
                    "Microsoft SAFE simulator boot ROM",
                    boot,
                    content,
                    project.config_files.clone(),
                    vec![Origin::derived(
                        "RTOS Microsoft SAFE simulator boot preparation",
                    )],
                );
                super::generated_operations::register_artifact(
                    plan,
                    format!("package.irom{word_bytes}"),
                    artifact,
                    vec![],
                )?;
                let output = directory.join(format!("cpu0_iram{suffix}.vhx"));
                plan.operations.push(create_operation(
                    format!("package.iram{word_bytes}"),
                    BuildOperationKind::Generate,
                    BuildAction::MemoryImage {
                        input: binary.clone(),
                        word_bytes,
                    },
                    vec![binary.clone()],
                    vec![output],
                    vec!["package.binary".into()],
                    vec![Origin::derived(
                        "RTOS Microsoft SAFE simulator memory image",
                    )],
                )?);
            }
        }
    }
    Ok(())
}

fn add_runner_image(
    project: &ResolvedProject,
    plan: &mut BuildPlan,
    input: PathBuf,
    dependency: &str,
) -> Result<()> {
    let output = crate::planning::runner::sonata_runner_directory(project).join("firmware.uf2");
    plan.operations.push(create_operation(
        "package.runner-image".into(),
        BuildOperationKind::PostProcess,
        BuildAction::Copy {
            input: input.clone(),
        },
        vec![input],
        vec![output],
        vec![dependency.into()],
        vec![Origin::derived("Sonata SDK runner fallback image")],
    )?);
    Ok(())
}

fn add_command(
    project: &ResolvedProject,
    plan: &mut BuildPlan,
    id: &str,
    command: BuildCommand,
    input: PathBuf,
    output: PathBuf,
    dependencies: Vec<String>,
) -> Result<()> {
    let mut inputs = vec![input];
    inputs.extend(project.config_files.clone());
    let uses_uf2 = command.executable == project.toolchain.uf2_converter;
    let mut operation = create_operation(
        id.into(),
        BuildOperationKind::PostProcess,
        BuildAction::Command {
            command,
            conditional: None,
        },
        inputs,
        vec![output],
        dependencies,
        vec![Origin::derived("RTOS board packaging rule")],
    )?;
    if uses_uf2 {
        operation
            .policy
            .required_tools
            .push(RequiredTool::Uf2Converter);
    }
    plan.operations.push(operation);
    Ok(())
}
