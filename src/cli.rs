// Copyright 3Bian Limited and Contributors
// SPDX-License-Identifier: MIT

use std::fs;

use cheriot_compose::build_graph::{BuildAction, BuildPlan, ProbeResults};
use cheriot_compose::configuration::ResolvedProject;
use cheriot_compose::diagnostic::{Diagnostic, Result};
use cheriot_compose::{
    compilation_database, configuration, execution,
    planning::{self, runner},
    render,
};
use command_name::CommandName;
use help::{command_help, general_help};
use option_catalog::OutputFormat;
use options::Options;

mod audit;
mod command_name;
mod help;
mod option_catalog;
mod options;
mod progress;
mod size;

pub(crate) fn execute_cli() -> Result<i32> {
    let Some(arguments) = Options::parse()? else {
        return Ok(0);
    };

    let (text, exit_code) = if arguments.command_name == CommandName::AnalyzeSwitcher {
        let path = arguments
            .input_file
            .as_ref()
            .ok_or_else(|| Diagnostic::new("analyze-switcher needs --input FILE"))?;
        let source = fs::read_to_string(path).map_err(|error| Diagnostic::io(path, error))?;
        let graph = cheriot_compose::switcher_analysis::analyze(&source)
            .map_err(|error| Diagnostic::new(format!("{}: {error}", path.display())))?;
        (graph, 0)
    } else if arguments.command_name == CommandName::CompareCompilationDatabase {
        compare_compilation_databases(&arguments)?
    } else if matches!(
        arguments.command_name,
        CommandName::Build | CommandName::Package
    ) && arguments.output_format == OutputFormat::Json
    {
        build_as_json(&arguments)?
    } else {
        (execute_project_command(&arguments)?, 0)
    };

    if let Some(path) = &arguments.output_file {
        if let Some(parent) = path.parent().filter(|path| !path.as_os_str().is_empty()) {
            fs::create_dir_all(parent).map_err(|error| Diagnostic::io(parent, error))?;
        }
        fs::write(path, text).map_err(|error| Diagnostic::io(path, error))?;
    } else if !text.is_empty() {
        println!("{text}");
    }
    Ok(exit_code)
}

fn selected_project(arguments: &Options) -> Result<ResolvedProject> {
    let path = configuration::select_project(
        &arguments.configuration_file,
        arguments.project_name.as_deref(),
    )?;
    cheriot_compose::project::load_project_with_settings(
        &path,
        arguments.firmware_name.as_deref(),
        &arguments.settings,
    )
}

fn execution_options(
    arguments: &Options,
    tool_stdout: execution::ToolStdout,
) -> execution::ExecutionOptions {
    execution::ExecutionOptions {
        dry_run: arguments.dry_run,
        jobs: arguments.jobs,
        tool_stdout,
    }
}

fn execute_with_progress(
    arguments: &Options,
    project: &ResolvedProject,
    plan: &BuildPlan,
    tool_stdout: execution::ToolStdout,
) -> Result<ProbeResults> {
    let mut progress = progress::BuildProgress::new(
        project,
        plan,
        arguments.verbose,
        arguments.quiet,
        arguments.dry_run,
    );
    let result = execution::execute_build_plan_with_options(
        plan,
        execution_options(arguments, tool_stdout),
        |event| progress.event(event),
    );
    progress.finish(result.is_ok());
    result
}

fn build_as_json(arguments: &Options) -> Result<(String, i32)> {
    let project = selected_project(arguments)?;
    let plan = if arguments.command_name == CommandName::Package {
        planning::create_packaging_plan(&project)?
    } else {
        planning::create_build_plan(&project)?
    };
    let mut events = Vec::new();
    let result = execution::execute_build_plan_with_options(
        &plan,
        execution_options(arguments, execution::ToolStdout::Stderr),
        |event| {
            events.push(event.clone());
        },
    );
    let status = if result.is_ok() { 0 } else { 1 };
    let report = serde_json::json!({
        "events": events,
        "error": result.err().map(|error| error.to_string()),
    });
    Ok((serde_json::to_string_pretty(&report)?, status))
}

fn compare_compilation_databases(arguments: &Options) -> Result<(String, i32)> {
    let reference_path = arguments
        .reference_database
        .as_ref()
        .ok_or_else(|| Diagnostic::new("compare-compilation-db needs --against FILE"))?;
    let reference = compilation_database::read_compilation_database(reference_path)?;
    let actual = if let Some(path) = &arguments.generated_database {
        compilation_database::read_compilation_database(path)?
    } else {
        let project = selected_project(arguments)?;
        let plan = planning::create_build_plan(&project)?;
        let probes = if arguments.probe_toolchain {
            execution::probe_toolchain(&plan)?
        } else {
            plan.unresolved_probes()
        };
        compilation_database::export_compilation_database(&plan, &probes)?
    };

    let report = compilation_database::compare_compilation_databases(
        &reference,
        &actual,
        &arguments.path_mappings,
    );
    let exit_code = if report.is_match() { 0 } else { 1 };
    let text = match arguments.output_format {
        OutputFormat::Json => serde_json::to_string_pretty(&report)?,
        OutputFormat::Text => {
            cheriot_compose::render::compilation_database::format_comparison_report(&report)
        }
        _ => return Err(Diagnostic::new("comparison format must be text or json")),
    };
    Ok((text, exit_code))
}

fn execute_project_command(arguments: &Options) -> Result<String> {
    if arguments.command_name == CommandName::Check
        && arguments.project_name.is_none()
        && let Some(workspace) = configuration::read_workspace(&arguments.configuration_file)?
    {
        if arguments.firmware_name.is_some() {
            return Err(Diagnostic::new(
                "select --project before using --firmware in a workspace",
            ));
        }
        for (name, path) in &workspace.projects {
            cheriot_compose::project::load_project_with_settings(path, None, &arguments.settings)
                .map_err(|mut error| {
                error.message = format!("workspace project {name}: {}", error.message);
                error
            })?;
        }
        return Ok(format!(
            "Configuration valid: {} workspace projects. No toolchain was run.\n",
            workspace.projects.len()
        ));
    }
    let project = selected_project(arguments)?;
    match arguments.command_name {
        CommandName::Check => Ok(format!(
            "Configuration valid: {} ({} threads). No toolchain was run.\n",
            project.name,
            project.threads.len()
        )),
        CommandName::Configuration => match arguments.output_format {
            OutputFormat::Toml => {
                toml::to_string_pretty(&project).map_err(|error| Diagnostic::new(error.to_string()))
            }
            OutputFormat::Json => Ok(serde_json::to_string_pretty(&project)?),
            _ => Err(Diagnostic::new("configuration format must be toml or json")),
        },
        CommandName::Audit => audit::execute(arguments, &project),
        CommandName::Size => size::execute(arguments, &project),
        CommandName::Run => {
            let plan = if project
                .board
                .runner
                .as_ref()
                .is_some_and(|runner| runner.kind == configuration::RunnerKind::MicrosoftSafe)
                || runner::uses_sonata_sdk_runner(&project)
            {
                planning::create_packaging_plan(&project)?
            } else {
                planning::create_build_plan(&project)?
            };
            let mut runner =
                runner::create_runner_plan(&project, &plan, arguments.vm_profile.as_deref())?;
            runner::append_arguments(&mut runner, &arguments.runner_arguments)?;
            execute_with_progress(arguments, &project, &plan, execution::ToolStdout::Stderr)?;
            if arguments.dry_run || arguments.verbose {
                let mut command = runner.command.clone();
                command.environment.clear();
                eprintln!(
                    "{} {}",
                    if arguments.dry_run {
                        "Would run:"
                    } else {
                        "Running:"
                    },
                    render::format_shell_command(&command)
                );
            } else if !arguments.quiet {
                eprintln!("Running {}.", project.firmware.name);
            }
            if !arguments.dry_run {
                execution::execute_runner_for_plan(&plan, &runner)?;
            }
            Ok(String::new())
        }
        _ => {
            let plan =
                if arguments.include_packaging || arguments.command_name == CommandName::Package {
                    planning::create_packaging_plan(&project)?
                } else {
                    planning::create_build_plan(&project)?
                };
            let probes = if arguments.probe_toolchain {
                execution::probe_toolchain(&plan)?
            } else {
                plan.unresolved_probes()
            };
            execute_plan_command(arguments, &project, &plan, &probes)
        }
    }
}

fn execute_plan_command(
    arguments: &Options,
    project: &ResolvedProject,
    plan: &BuildPlan,
    probes: &ProbeResults,
) -> Result<String> {
    match arguments.command_name {
        CommandName::Plan => match arguments.output_format {
            OutputFormat::Json => {
                let mut document = serde_json::to_value(plan)?;
                let object = document
                    .as_object_mut()
                    .ok_or_else(|| Diagnostic::new("invalid plan document"))?;
                object.insert("probe_results".into(), serde_json::to_value(probes)?);
                Ok(serde_json::to_string_pretty(&document)?)
            }
            OutputFormat::Shell => render::render_shell_script(plan),
            OutputFormat::CompilationDatabase => Ok(serde_json::to_string_pretty(
                &compilation_database::export_compilation_database(plan, probes)?,
            )?),
            OutputFormat::Text => Ok(describe_plan(plan, probes)),
            _ => Err(Diagnostic::new(
                "plan format must be text, json, shell or compilation-database",
            )),
        },
        CommandName::Build | CommandName::Package => {
            if arguments.output_format != OutputFormat::Human {
                return Err(Diagnostic::new("build format must be json"));
            }
            execute_with_progress(arguments, project, plan, execution::ToolStdout::Inherit)?;
            Ok(String::new())
        }
        CommandName::Explain => {
            let identifier = arguments
                .operation_identifier
                .as_ref()
                .ok_or_else(|| Diagnostic::new("explain needs an operation ID"))?;
            let operation = plan
                .operations
                .iter()
                .find(|operation| &operation.id == identifier)
                .ok_or_else(|| Diagnostic::new(format!("unknown operation {identifier}")))?;
            let mut state = None;
            execution::execute_build_plan(plan, true, |event| match event {
                execution::ExecutionEvent::Planned { operation, reason }
                    if operation == identifier =>
                {
                    state = Some(serde_json::json!({ "status": "will run", "reason": reason }));
                }
                execution::ExecutionEvent::Skipped { operation, reason }
                    if operation == identifier =>
                {
                    state = Some(serde_json::json!({ "status": "up to date", "reason": reason }));
                }
                _ => {}
            })?;
            Ok(serde_json::to_string_pretty(
                &serde_json::json!({ "operation": operation, "state": state, "configuration_origins": project.provenance }),
            )?)
        }
        CommandName::Clean => Ok(format!(
            "Removed {} known build files.\n",
            execution::clean_build_plan(plan)?
        )),
        _ => Err(Diagnostic::new("invalid command")),
    }
}

fn describe_plan(plan: &BuildPlan, probes: &ProbeResults) -> String {
    let mut text = plan.notes.join("\n") + "\n";
    for operation in &plan.operations {
        text += &format!("\n{} [{:?}]\n", operation.id, operation.kind);
        match &operation.action {
            BuildAction::Publish => {
                for (input, output) in operation.inputs.iter().zip(operation.outputs.iter()) {
                    text += &format!("  publish {} -> {}\n", input.display(), output.display());
                }
            }
            BuildAction::Copy { input } => {
                text += &format!(
                    "  copy {} -> {}\n",
                    input.display(),
                    operation.outputs[0].display()
                );
            }
            BuildAction::Generate { template, .. } => {
                text += &format!(
                    "  render {template} -> {}\n",
                    operation.outputs[0].display()
                );
            }
            BuildAction::MemoryImage { input, word_bytes } => {
                text += &format!(
                    "  convert {} to {}-byte memory words -> {}\n",
                    input.display(),
                    word_bytes,
                    operation.outputs[0].display()
                );
            }
            BuildAction::Command {
                command,
                conditional,
            } => {
                text += &format!("  {}\n", render::format_shell_command(command));
                if let Some(condition) = conditional {
                    let status = match probes.get(&condition.probe).copied().flatten() {
                        Some(true) => "true",
                        Some(false) => "false",
                        None => "unresolved",
                    };
                    text += &format!(
                        "  if {} succeeds: insert {:?} at argv[{}] (result: {status})\n",
                        condition.probe, condition.arguments, condition.insert_at
                    );
                }
            }
            BuildAction::Probe { command } => {
                let status = match probes.get(&operation.id).copied().flatten() {
                    Some(true) => "true",
                    Some(false) => "false",
                    None => "unresolved",
                };
                text += &format!("  {} [{status}]\n", render::format_shell_command(command));
            }
            BuildAction::GitDescription { command } => {
                text += &format!(
                    "  {} (Git description, or unknown if unavailable)\n",
                    render::format_shell_command(command)
                );
            }
        }
    }
    text
}
