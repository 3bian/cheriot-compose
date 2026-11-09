// Copyright 3Bian Limited and Contributors
// SPDX-License-Identifier: MIT

use std::path::{Path, PathBuf};

use crate::build_graph::{BuildCommand, BuildPlan, RunnerPlan, path_to_string, validate_command};
use crate::configuration::ResolvedProject;
use crate::diagnostic::{Diagnostic, Result, Sourced};

pub fn uses_sonata_sdk_runner(project: &ResolvedProject) -> bool {
    if project.board.simulation {
        return false;
    }
    let expected = match project.board.packaging {
        Some(crate::configuration::PackagingKind::SonataLegacy) => "run-sonata.sh",
        Some(crate::configuration::PackagingKind::SonataV1) => "run-sonata-1.0.sh",
        _ => return false,
    };
    project
        .board
        .runner
        .as_ref()
        .and_then(|runner| Path::new(&runner.executable).file_name())
        .is_some_and(|name| name == expected)
}

pub fn sonata_runner_directory(project: &ResolvedProject) -> PathBuf {
    project
        .build_dir
        .join(format!("sonata-runner-{}", project.configuration_identity))
}

pub fn append_arguments(runner: &mut RunnerPlan, arguments: &[String]) -> Result<()> {
    runner.command.argv.extend(
        arguments
            .iter()
            .map(|value| Sourced::derived(value.clone(), "runner command-line argument")),
    );
    validate_command(&runner.command)
}

pub fn create_runner_plan(
    project: &ResolvedProject,
    plan: &BuildPlan,
    vm_profile: Option<&Path>,
) -> Result<RunnerPlan> {
    let runner = project
        .board
        .runner
        .as_ref()
        .ok_or_else(|| Diagnostic::new("the selected board does not define a runner"))?;
    let firmware = plan
        .publication
        .as_ref()
        .map(|publication| &publication.elf)
        .ok_or_else(|| Diagnostic::new("the plan has no firmware output"))?
        .clone();
    let sonata_sdk = uses_sonata_sdk_runner(project);
    let cwd = if sonata_sdk {
        sonata_runner_directory(project)
    } else {
        plan.build_dir.clone()
    };
    let mut argv = runner.arguments.clone();
    match (runner.kind, vm_profile) {
        (crate::configuration::RunnerKind::CheriotVm, Some(profile)) => {
            if !profile.is_file() {
                return Err(Diagnostic::new(format!(
                    "CHERIoT VM profile does not exist: {}",
                    profile.display()
                )));
            }
            argv.push(Sourced::derived("--profile".into(), "CHERIoT VM profile"));
            argv.push(Sourced::derived(
                path_to_string(profile)?,
                "CHERIoT VM profile",
            ));
            argv.push(Sourced::derived("--elf".into(), "CHERIoT VM firmware"));
        }
        (crate::configuration::RunnerKind::CheriotVm, None) => {
            return Err(Diagnostic::new(
                "this board uses cheriot-vm; supply its runtime profile with --vm-profile FILE",
            ));
        }
        (
            crate::configuration::RunnerKind::Executable
            | crate::configuration::RunnerKind::MicrosoftSafe,
            Some(_),
        ) => {
            return Err(Diagnostic::new(
                "--vm-profile requires a board configured for cheriot-vm",
            ));
        }
        (
            crate::configuration::RunnerKind::Executable
            | crate::configuration::RunnerKind::MicrosoftSafe,
            None,
        ) => {}
    }
    if runner.kind != crate::configuration::RunnerKind::MicrosoftSafe {
        let argument = if sonata_sdk {
            let image = sonata_runner_directory(project).join("firmware.uf2");
            if plan.artifact_producer(&image).is_none() {
                return Err(Diagnostic::new(
                    "Sonata runner requires the board packaging plan",
                ));
            }
            let internal = plan
                .operations
                .iter()
                .find(|operation| {
                    matches!(operation.action, crate::build_graph::BuildAction::Publish)
                })
                .and_then(|operation| operation.inputs.first())
                .and_then(|path| path.file_name())
                .ok_or_else(|| Diagnostic::new("internal firmware output is missing"))?;
            PathBuf::from("..").join(internal)
        } else {
            firmware.clone()
        };
        argv.push(Sourced::derived(
            path_to_string(&argument)?,
            "firmware output",
        ));
    }
    let executable = if runner.kind == crate::configuration::RunnerKind::MicrosoftSafe {
        std::env::var("CHERIOT_MSFT_SAFE_SIM")
            .ok()
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| runner.executable.clone())
    } else {
        runner.executable.clone()
    };
    let command = BuildCommand {
        executable,
        argv,
        cwd,
        environment: runner.environment.clone(),
        stdout: None,
        stderr: None,
    };
    validate_command(&command)?;
    Ok(RunnerPlan {
        firmware,
        command,
        failure_patterns: runner.failure_patterns.clone(),
    })
}
