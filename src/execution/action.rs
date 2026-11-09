// Copyright 3Bian Limited and Contributors
// SPDX-License-Identifier: MIT

use std::{fs, process::Stdio};

use super::{
    BuildAction, BuildPlan, Diagnostic, Result, git_description_header, guarded_output,
    memory_image, process, write_if_changed,
};

mod action_result;
mod output_routing;
mod prepared_action;

pub(super) use action_result::ActionResult;
pub(super) use output_routing::OutputRouting;
pub(super) use prepared_action::PreparedAction;

pub(super) fn perform<'a>(
    work: PreparedAction<'a>,
    plan: &BuildPlan,
    routing: OutputRouting,
) -> ActionResult<'a> {
    let operation = work.operation;
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let result = (|| -> Result<Option<bool>> {
        if let Some(command) = &work.command {
            super::validate_action_tools(plan, operation, command)?;
        }
        for output in operation
            .outputs
            .iter()
            .chain(operation.depfile.iter())
            .filter(|_| !matches!(operation.action, BuildAction::Publish))
        {
            guarded_output(output, &plan.build_dir)?;
            if let Some(directory) = output.parent() {
                fs::create_dir_all(directory).map_err(|error| Diagnostic::io(directory, error))?;
            }
        }
        match &operation.action {
            BuildAction::Publish => super::publication::publish(plan, &operation.inputs)?,
            BuildAction::Copy { input } => {
                let bytes = fs::read(input).map_err(|error| Diagnostic::io(input, error))?;
                write_if_changed(&operation.outputs[0], &bytes)?;
            }
            BuildAction::Generate { content, .. } => {
                write_if_changed(&operation.outputs[0], content.as_bytes())?;
            }
            BuildAction::MemoryImage { input, word_bytes } => {
                let bytes = fs::read(input).map_err(|error| Diagnostic::io(input, error))?;
                let content = memory_image(&bytes, *word_bytes)?;
                write_if_changed(&operation.outputs[0], content.as_bytes())?;
            }
            BuildAction::GitDescription { command } => {
                let header = git_description_header(command)?;
                write_if_changed(&operation.outputs[0], &header)?;
            }
            BuildAction::Probe { command } => {
                let mut child = process(command)?;
                child.stdout(Stdio::null()).stderr(Stdio::null());
                let status = child.status().map_err(|error| {
                    Diagnostic::new(format!(
                        "{}: cannot run {}: {error}",
                        operation.id, command.executable
                    ))
                })?;
                if status.code().is_none() {
                    return Err(Diagnostic::new(format!(
                        "{}: compiler probe terminated by signal",
                        operation.id
                    )));
                }
                return Ok(Some(status.success()));
            }
            BuildAction::Command { .. } => {
                let command = work
                    .command
                    .as_ref()
                    .ok_or_else(|| Diagnostic::new("missing command"))?;
                super::fresh_output::with_fresh_link_outputs(operation, &plan.build_dir, || {
                    let mut child = process(command)?;
                    let status = match routing {
                        OutputRouting::Group { .. } => {
                            let output = child.output().map_err(|error| {
                                Diagnostic::new(format!(
                                    "{}: cannot run {}: {error}",
                                    operation.id, command.executable
                                ))
                            })?;
                            stdout = output.stdout;
                            stderr = output.stderr;
                            output.status
                        }
                        OutputRouting::Stream {
                            stdout_to_stderr: true,
                        } if command.stdout.is_none() => {
                            child.stdout(Stdio::piped());
                            let mut child = child.spawn().map_err(|error| {
                                Diagnostic::new(format!(
                                    "{}: cannot run {}: {error}",
                                    operation.id, command.executable
                                ))
                            })?;
                            if let Some(mut output) = child.stdout.take() {
                                std::io::copy(&mut output, &mut std::io::stderr().lock()).map_err(
                                    |error| {
                                        Diagnostic::new(format!("cannot show tool output: {error}"))
                                    },
                                )?;
                            }
                            child.wait().map_err(|error| {
                                Diagnostic::new(format!(
                                    "{}: cannot wait for {}: {error}",
                                    operation.id, command.executable
                                ))
                            })?
                        }
                        OutputRouting::Stream { .. } => child.status().map_err(|error| {
                            Diagnostic::new(format!(
                                "{}: cannot run {}: {error}",
                                operation.id, command.executable
                            ))
                        })?,
                    };
                    if !status.success() {
                        return Err(Diagnostic::new(format!(
                            "{} failed with {status}: {}",
                            operation.id, command.executable
                        )));
                    }
                    for output in operation.outputs.iter().chain(operation.depfile.iter()) {
                        guarded_output(output, &plan.build_dir)?;
                        if !output.is_file() {
                            return Err(Diagnostic::new(format!(
                                "{} succeeded without producing {}",
                                operation.id,
                                output.display()
                            )));
                        }
                    }
                    Ok(())
                })?;
            }
        }
        Ok(None)
    })();
    ActionResult {
        work,
        result,
        stdout,
        stderr,
        routing,
    }
}
