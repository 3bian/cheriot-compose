// Copyright 3Bian Limited and Contributors
// SPDX-License-Identifier: MIT

use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    io::Write,
    path::{Path, PathBuf},
    process::{Command as Process, Stdio},
};

use crate::{
    build_graph::{
        BuildAction, BuildCommand, BuildOperation, BuildPlan, ProbeResults, calculate_fingerprint,
    },
    diagnostic::{Diagnostic, Result},
};
use build_record::BuildRecord;
use build_state::BuildState;

mod action;
mod build_lock;
mod build_record;
mod build_state;
mod clean;
mod dependency_file;
mod execution_event;
mod execution_options;
mod file_output;
mod fresh_output;
mod publication;
mod runner_execution;
mod scheduler;
mod tool_stdout;

pub use clean::clean_build_plan;
pub use dependency_file::parse_dependency_file;
pub use execution_event::ExecutionEvent;
pub use execution_options::ExecutionOptions;
pub use runner_execution::{execute_runner, execute_runner_for_plan};
pub use tool_stdout::ToolStdout;

use build_lock::BuildLock;
pub use file_output::write_if_changed;

fn digest(path: &Path) -> Option<String> {
    fs::read(path)
        .ok()
        .map(|contents| calculate_fingerprint(&contents))
}

fn output_digests(operation: &BuildOperation) -> Vec<Option<String>> {
    operation
        .outputs
        .iter()
        .map(|output| digest(output))
        .collect()
}

fn load_state(plan: &BuildPlan) -> Result<Option<BuildState>> {
    let path = plan.state_path();
    let contents = match fs::read(&path) {
        Ok(contents) => contents,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(Diagnostic::io(&path, error)),
    };
    let state: BuildState = serde_json::from_slice(&contents).map_err(|error| {
        Diagnostic::new(format!("invalid build state {}: {error}", path.display()))
    })?;
    if state.owner != plan.state_owner {
        return Err(Diagnostic::new(format!(
            "build state collision at {}; choose a different intermediate directory",
            path.display()
        )));
    }
    Ok(Some(state))
}

fn check_build_identity(plan: &BuildPlan, create: bool) -> Result<()> {
    let (Some(path), Some(owner)) = (plan.identity_path(), &plan.state_owner) else {
        return Ok(());
    };
    guarded_output(&path, &plan.build_dir)?;
    match fs::read(&path) {
        Ok(contents) if contents == owner.as_bytes() => Ok(()),
        Ok(_) => Err(Diagnostic::new(format!(
            "intermediate directory identity collision at {}; choose a different --build-dir",
            path.display()
        ))),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound && !create => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            let mut file = fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&path)
                .map_err(|error| Diagnostic::io(&path, error))?;
            file.write_all(owner.as_bytes())
                .map_err(|error| Diagnostic::io(&path, error))
        }
        Err(error) => Err(Diagnostic::io(&path, error)),
    }
}

fn check_ownership_record(plan: &BuildPlan) -> Result<()> {
    let Some(operation) = plan.operations.iter().find(|operation| {
        operation.policy.output_handling == crate::build_graph::OutputHandling::OwnershipRecord
    }) else {
        return Ok(());
    };
    let BuildAction::Generate { content, .. } = &operation.action else {
        return Err(Diagnostic::new("invalid owned-output action"));
    };
    let path = &operation.outputs[0];
    guarded_output(path, &plan.build_dir)?;
    match fs::read(path) {
        Ok(existing) if existing == content.as_bytes() => Ok(()),
        Ok(_) => Err(Diagnostic::new(format!(
            "refusing to overwrite an unowned build record: {}",
            path.display()
        ))),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(Diagnostic::io(path, error)),
    }
}

fn outputs_owned_by_other_builds(plan: &BuildPlan) -> Result<BTreeSet<PathBuf>> {
    let Some(owner) = &plan.state_owner else {
        return Ok(BTreeSet::new());
    };
    let mut shared = BTreeSet::new();
    for entry in
        fs::read_dir(&plan.build_dir).map_err(|error| Diagnostic::io(&plan.build_dir, error))?
    {
        let entry = entry.map_err(|error| Diagnostic::io(&plan.build_dir, error))?;
        let name = entry.file_name();
        let Some(name) = name.to_str() else {
            continue;
        };
        if !(name.starts_with("state-") || name.starts_with(".outputs-"))
            || !name.ends_with(".json")
        {
            continue;
        }
        let Ok(contents) = fs::read(entry.path()) else {
            continue;
        };
        if name.starts_with("state-") && name.ends_with(".json") {
            let Ok(state) = serde_json::from_slice::<BuildState>(&contents) else {
                continue;
            };
            let Some(other) = state.owner else {
                continue;
            };
            if &other != owner
                && name == format!("state-{}.json", calculate_fingerprint(other.as_bytes()))
            {
                shared.extend(state.owned_outputs);
            }
        } else if name.starts_with(".outputs-") && name.ends_with(".json") {
            let Ok(record) =
                serde_json::from_slice::<crate::build_graph::OwnershipRecord>(&contents)
            else {
                continue;
            };
            let expected = format!(
                ".outputs-{}-{}.json",
                calculate_fingerprint(record.owner.as_bytes()),
                calculate_fingerprint(&contents)
            );
            if &record.owner != owner && name == expected {
                shared.extend(record.outputs);
            }
        }
    }
    Ok(shared)
}

fn executable_path(command: &BuildCommand, executable: &str) -> Option<PathBuf> {
    let executable_path = Path::new(executable);
    let resolve = |candidate: PathBuf| {
        let path = if candidate.is_absolute() {
            candidate
        } else {
            command.cwd.join(candidate)
        };
        let metadata = fs::metadata(&path).ok()?;
        if !metadata.is_file() {
            return None;
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            if metadata.permissions().mode() & 0o111 == 0 {
                return None;
            }
        }
        fs::canonicalize(path).ok()
    };
    if executable.contains('/') {
        return resolve(executable_path.to_path_buf());
    }
    let path = command
        .environment
        .get("PATH")
        .map(std::ffi::OsString::from)
        .or_else(|| std::env::var_os("PATH"))?;
    std::env::split_paths(&path).find_map(|directory| resolve(directory.join(executable_path)))
}

fn validate_llvm_command(
    toolchain: &crate::configuration::Toolchain,
    command: &BuildCommand,
) -> Result<()> {
    if let Some(error) = toolchain.llvm_error(&command.executable)
        && executable_path(command, &command.executable).is_none()
    {
        return Err(error);
    }
    Ok(())
}

pub fn validate_llvm_executable(
    toolchain: &crate::configuration::Toolchain,
    executable: &str,
    working_directory: &Path,
) -> Result<()> {
    validate_llvm_command(
        toolchain,
        &BuildCommand {
            executable: executable.to_owned(),
            argv: Vec::new(),
            cwd: working_directory.to_path_buf(),
            environment: BTreeMap::new(),
            stdout: None,
            stderr: None,
        },
    )
}

fn validate_action_tools(
    plan: &BuildPlan,
    operation: &BuildOperation,
    command: &BuildCommand,
) -> Result<()> {
    for check in operation
        .policy
        .tool_checks(plan.toolchain.as_ref(), command)
    {
        let Some(executable) = check.executable else {
            return Err(check.error);
        };
        if executable_path(command, &executable).is_none() {
            return Err(check.error);
        }
    }
    Ok(())
}

fn process(command: &BuildCommand) -> Result<Process> {
    let mut child_process = Process::new(&command.executable);
    child_process
        .args(command.argv.iter().map(|argument| &argument.value))
        .current_dir(&command.cwd)
        .envs(&command.environment);
    if let Some(output) = &command.stdout {
        child_process
            .stdout(fs::File::create(output).map_err(|error| Diagnostic::io(output, error))?);
    }
    if let Some(output) = &command.stderr {
        child_process
            .stderr(fs::File::create(output).map_err(|error| Diagnostic::io(output, error))?);
    }
    Ok(child_process)
}

fn git_description_header(command: &BuildCommand) -> Result<Vec<u8>> {
    let mut child = process(command)?;
    child.stderr(Stdio::null());
    let description = match child.output() {
        Ok(output) if output.status.success() => output.stdout,
        _ => b"unknown".to_vec(),
    };
    let mut header = b"#define CHERIOT_RTOS_GIT_DESCRIPTION \"".to_vec();
    for byte in description {
        match byte {
            b'\r' | b'\n' => (),
            b'\\' | b'\"' => {
                header.push(b'\\');
                header.push(byte);
            }
            byte => header.push(byte),
        }
    }
    header.extend_from_slice(b"\"\n");
    Ok(header)
}

fn memory_image(bytes: &[u8], word_bytes: u8) -> Result<String> {
    use std::fmt::Write as _;
    if ![4, 8].contains(&word_bytes) {
        return Err(Diagnostic::new(
            "memory image word size must be 4 or 8 bytes",
        ));
    }
    let mut text = String::new();
    for chunk in bytes.chunks(usize::from(word_bytes)) {
        let mut word = [0_u8; 8];
        word[..chunk.len()].copy_from_slice(chunk);
        let value = u64::from_le_bytes(word);
        writeln!(
            text,
            "{value:0width$X}",
            width = usize::from(word_bytes) * 2
        )
        .map_err(|error| Diagnostic::new(format!("cannot format memory image: {error}")))?;
    }
    text.push('\n');
    Ok(text)
}

fn guarded_output(path: &Path, build: &Path) -> Result<()> {
    if !path.is_absolute()
        || crate::configuration::resolution::normalize_path(path) != path
        || !path.starts_with(build)
        || path == build
    {
        return Err(Diagnostic::new(format!(
            "refusing output outside build directory: {}",
            path.display()
        )));
    }
    let mut current = Some(path);
    while let Some(path) = current {
        if fs::symlink_metadata(path).is_ok_and(|mapping| mapping.file_type().is_symlink()) {
            return Err(Diagnostic::new(format!(
                "refusing symlink in build output path: {}",
                path.display()
            )));
        }
        current = path.parent();
    }
    Ok(())
}

fn record(
    operation: &BuildOperation,
    probes: &BTreeMap<String, Option<bool>>,
    tools: &mut BTreeMap<PathBuf, String>,
) -> Result<BuildRecord> {
    let mut value = serde_json::to_vec(operation)?;
    let command = match &operation.action {
        BuildAction::Command { command, .. } => Some(
            operation
                .resolve_command(probes)?
                .unwrap_or_else(|| command.clone()),
        ),
        BuildAction::Probe { command } | BuildAction::GitDescription { command } => {
            Some(command.clone())
        }
        _ => None,
    };
    let mut inputs = operation.inputs.clone();
    if let Some(active_command) = &command {
        value.extend_from_slice(&serde_json::to_vec(active_command)?);
        let mut environment: BTreeMap<std::ffi::OsString, std::ffi::OsString> =
            std::env::vars_os().collect();
        for (name, setting) in &active_command.environment {
            environment.insert(name.into(), setting.into());
        }
        for (name, setting) in environment {
            let name = name.as_encoded_bytes();
            let setting = setting.as_encoded_bytes();
            value.extend_from_slice(&(name.len() as u64).to_le_bytes());
            value.extend_from_slice(name);
            value.extend_from_slice(&(setting.len() as u64).to_le_bytes());
            value.extend_from_slice(setting);
        }
        if let Some(tool_path) = executable_path(active_command, &active_command.executable) {
            value.extend_from_slice(tool_path.to_string_lossy().as_bytes());
            let tool_digest = tools
                .entry(tool_path.clone())
                .or_insert_with(|| digest(&tool_path).unwrap_or_else(|| "unavailable".into()));
            value.extend_from_slice(tool_digest.as_bytes());
        } else {
            value.extend_from_slice(b"unavailable");
        }
        if operation
            .policy
            .required_tools
            .contains(&crate::build_graph::RequiredTool::FirmwareRust)
            && let Some(rustc) = active_command.environment.get("RUSTC")
            && let Some(tool_path) = executable_path(active_command, rustc)
        {
            value.extend_from_slice(tool_path.to_string_lossy().as_bytes());
            let tool_digest = tools
                .entry(tool_path.clone())
                .or_insert_with(|| digest(&tool_path).unwrap_or_else(|| "unavailable".into()));
            value.extend_from_slice(tool_digest.as_bytes());
        }
        if let Some(dependency_file) = &operation.depfile
            && let Ok(dependency_contents) = fs::read_to_string(dependency_file)
        {
            inputs.extend(parse_dependency_file(
                &dependency_contents,
                &active_command.cwd,
            )?);
        }
    }
    Ok(BuildRecord {
        command: calculate_fingerprint(&value),
        inputs: inputs
            .iter()
            .map(|candidate_path| {
                (
                    candidate_path.clone(),
                    digest(candidate_path).unwrap_or_else(|| "missing".into()),
                )
            })
            .collect(),
        outputs: operation
            .outputs
            .iter()
            .chain(operation.depfile.iter())
            .map(|candidate_path| {
                (
                    candidate_path.clone(),
                    digest(candidate_path).unwrap_or_else(|| "missing".into()),
                )
            })
            .collect(),
        probe: probes.get(&operation.id).copied().flatten(),
    })
}

fn rebuild_reason(current: &BuildRecord, previous: Option<&BuildRecord>) -> Option<String> {
    let Some(previous) = previous else {
        return Some("no saved successful build".into());
    };
    if current.command != previous.command {
        return Some("command, template, configuration or compiler changed".into());
    }
    if current.inputs.values().any(|value| value == "missing") || current.inputs != previous.inputs
    {
        return Some("input or included header changed/missing".into());
    }
    if current.outputs.values().any(|value| value == "missing")
        || current.outputs != previous.outputs
    {
        return Some("output changed/missing".into());
    }
    None
}

pub fn execute_build_plan(
    plan: &BuildPlan,
    dry_run: bool,
    deliver_event: impl FnMut(&ExecutionEvent),
) -> Result<ProbeResults> {
    execute_build_plan_with_diagnostics(plan, dry_run, false, deliver_event)
}

pub fn execute_build_plan_with_diagnostics(
    plan: &BuildPlan,
    dry_run: bool,
    redirect_tool_stdout: bool,
    deliver_event: impl FnMut(&ExecutionEvent),
) -> Result<ProbeResults> {
    execute_build_plan_with_jobs(plan, dry_run, redirect_tool_stdout, 1, deliver_event)
}

pub fn execute_build_plan_with_jobs(
    plan: &BuildPlan,
    dry_run: bool,
    redirect_tool_stdout: bool,
    jobs: usize,
    deliver_event: impl FnMut(&ExecutionEvent),
) -> Result<ProbeResults> {
    execute_build_plan_with_options(
        plan,
        ExecutionOptions {
            dry_run,
            jobs,
            tool_stdout: if redirect_tool_stdout {
                ToolStdout::Stderr
            } else {
                ToolStdout::Inherit
            },
        },
        deliver_event,
    )
}

pub fn execute_build_plan_with_options(
    plan: &BuildPlan,
    options: ExecutionOptions,
    deliver_event: impl FnMut(&ExecutionEvent),
) -> Result<ProbeResults> {
    if !(1..=256).contains(&options.jobs) {
        return Err(Diagnostic::new("jobs must be between 1 and 256"));
    }
    plan.validate()?;
    let _lock = if options.dry_run {
        None
    } else {
        Some(BuildLock::acquire(&plan.build_dir)?)
    };
    check_build_identity(plan, !options.dry_run)?;
    check_ownership_record(plan)?;
    let _publication_lock = match (&plan.publication, options.dry_run) {
        (Some(publication), false) => Some(publication::PublicationLock::acquire(publication)?),
        _ => None,
    };
    if let Some(publication) = &plan.publication {
        publication::preflight(plan, publication)?;
    }
    scheduler::execute(plan, options, deliver_event)
}

pub fn probe_toolchain(plan: &BuildPlan) -> Result<ProbeResults> {
    let mut probes = plan.clone();
    probes.publication = None;
    let roots: Vec<String> = probes
        .operations
        .iter()
        .filter(|operation| matches!(operation.action, BuildAction::Probe { .. }))
        .map(|operation| operation.id.clone())
        .collect();
    let prerequisites = probes.prerequisite_closure(&roots)?;
    probes
        .operations
        .retain(|operation| prerequisites.contains(&operation.id));
    execute_build_plan(&probes, false, |_| {})
}

#[cfg(test)]
mod memory_image_tests {
    use super::memory_image;

    #[test]
    fn memory_image_uses_little_endian_words_and_pads_the_last_word() {
        assert_eq!(memory_image(b"abc", 4).unwrap(), "00636261\n\n");
        assert_eq!(memory_image(b"abc", 8).unwrap(), "0000000000636261\n\n");
        assert_eq!(
            memory_image(b"abcdefgh", 4).unwrap(),
            "64636261\n68676665\n\n"
        );
        assert!(memory_image(b"abc", 3).is_err());
    }
}
