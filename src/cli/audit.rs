// Copyright 3Bian Limited and Contributors
// SPDX-License-Identifier: MIT

use std::io::Write;
use std::process::Command;

use cheriot_compose::configuration::ResolvedProject;
use cheriot_compose::diagnostic::{Diagnostic, Result};
use cheriot_compose::execution;
use cheriot_compose::planning;

use super::{execute_with_progress, options::Options};

pub(super) fn execute(arguments: &Options, project: &ResolvedProject) -> Result<String> {
    let query = arguments
        .audit_query
        .as_ref()
        .ok_or_else(|| Diagnostic::new("audit needs --query QUERY"))?;
    for module in &arguments.audit_modules {
        if !module.is_file() {
            return Err(Diagnostic::new(format!(
                "audit module does not exist: {}",
                module.display()
            )));
        }
    }
    if !project.firmware.compartment_report {
        return Err(Diagnostic::new(
            "audit requires the firmware compartment report; enable compartment_report",
        ));
    }
    let plan = planning::create_build_plan(project)?;
    let report = plan
        .publication
        .as_ref()
        .filter(|publication| publication.report_enabled)
        .map(|publication| &publication.report)
        .ok_or_else(|| Diagnostic::new("firmware plan does not produce an audit report"))?;
    let board = project.build_dir.join("board.json");
    execute_with_progress(arguments, project, &plan, execution::ToolStdout::Stderr)?;
    let tool = arguments.audit_tool.as_deref().unwrap_or("cheriot-audit");
    if arguments.dry_run {
        if !arguments.quiet {
            eprintln!("Would audit {} with {tool}.", project.firmware.name);
        }
        return Ok(String::new());
    }
    if !arguments.quiet {
        eprintln!("Auditing {}.", project.firmware.name);
    }
    let mut command = Command::new(tool);
    command
        .current_dir(&project.root)
        .arg("--board")
        .arg(board)
        .arg("--firmware-report")
        .arg(report);
    for module in &arguments.audit_modules {
        command.arg("--module").arg(module);
    }
    command.arg("--query").arg(query);
    let output = command
        .output()
        .map_err(|error| Diagnostic::new(format!("cannot run audit tool {tool}: {error}")))?;
    if !output.stderr.is_empty() {
        let _ = std::io::stderr().write_all(&output.stderr);
    }
    if !output.status.success() {
        if !output.stdout.is_empty() {
            let _ = std::io::stderr().write_all(&output.stdout);
        }
        return Err(Diagnostic::new(format!(
            "audit tool failed with {}",
            output.status
        )));
    }
    String::from_utf8(output.stdout).map_err(|_| Diagnostic::new("audit output is not valid UTF-8"))
}
