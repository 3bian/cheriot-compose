// Copyright 3Bian Limited and Contributors
// SPDX-License-Identifier: MIT

use std::{
    collections::BTreeMap,
    io::{Read, Write},
    process::Stdio,
};

use super::{
    BuildLock, BuildPlan, Diagnostic, Result, check_build_identity, load_state, process,
    publication, rebuild_reason, record,
};

pub fn execute_runner(runner: &crate::build_graph::RunnerPlan) -> Result<()> {
    if !runner.firmware.is_file() {
        return Err(Diagnostic::new(format!(
            "firmware does not exist: {}",
            runner.firmware.display()
        )));
    }
    let mut command = process(&runner.command)?;
    if runner.failure_patterns.is_empty() {
        let status = command.status().map_err(|error| {
            Diagnostic::new(format!("cannot run {}: {error}", runner.command.executable))
        })?;
        if !status.success() {
            return Err(Diagnostic::new(format!(
                "runner {} failed with {status}",
                runner.command.executable
            )));
        }
        return Ok(());
    }
    command.stdout(Stdio::piped()).stderr(Stdio::piped());
    let mut child = command.spawn().map_err(|error| {
        Diagnostic::new(format!("cannot run {}: {error}", runner.command.executable))
    })?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| Diagnostic::new("runner stdout was not captured"))?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| Diagnostic::new("runner stderr was not captured"))?;
    let stdout_patterns = runner.failure_patterns.clone();
    let stderr_patterns = runner.failure_patterns.clone();
    let stdout_worker = std::thread::spawn(move || {
        relay_runner_output(stdout, std::io::stdout(), &stdout_patterns)
    });
    let stderr_worker = std::thread::spawn(move || {
        relay_runner_output(stderr, std::io::stderr(), &stderr_patterns)
    });
    let status = child
        .wait()
        .map_err(|error| Diagnostic::new(format!("cannot wait for runner: {error}")))?;
    let stdout_failure = stdout_worker
        .join()
        .map_err(|_| Diagnostic::new("runner stdout monitor failed"))??;
    let stderr_failure = stderr_worker
        .join()
        .map_err(|_| Diagnostic::new("runner stderr monitor failed"))??;
    if !status.success() {
        return Err(Diagnostic::new(format!(
            "runner {} failed with {status}",
            runner.command.executable
        )));
    }
    if let Some(pattern) = stdout_failure.or(stderr_failure) {
        return Err(Diagnostic::new(format!(
            "runner output contained failure marker {pattern:?}"
        )));
    }
    Ok(())
}

pub fn execute_runner_for_plan(
    plan: &BuildPlan,
    runner: &crate::build_graph::RunnerPlan,
) -> Result<()> {
    plan.validate()?;
    let publication = plan
        .publication
        .as_ref()
        .ok_or_else(|| Diagnostic::new("runner plan has no final firmware destination"))?;
    if runner.firmware != publication.elf {
        return Err(Diagnostic::new(
            "runner firmware does not match the selected build",
        ));
    }
    let _build_lock = BuildLock::acquire(&plan.build_dir)?;
    check_build_identity(plan, false)?;
    let _publication_lock = publication::PublicationLock::acquire(publication)?;
    publication::preflight(plan, publication)?;
    let state = load_state(plan)?.ok_or_else(|| {
        Diagnostic::new("build state is missing; build the firmware again before running")
    })?;
    let mut probes = plan.unresolved_probes();
    for (id, value) in &mut probes {
        if let Some(record) = state.operations.get(id) {
            *value = record.probe;
        }
    }
    let mut tools = BTreeMap::new();
    for operation in &plan.operations {
        let current = record(operation, &probes, &mut tools)?;
        if rebuild_reason(&current, state.operations.get(&operation.id)).is_some() {
            return Err(Diagnostic::new(format!(
                "{} changed before the runner started; build again",
                operation.id
            )));
        }
    }
    execute_runner(runner)
}

fn relay_runner_output(
    mut input: impl Read,
    mut output: impl Write,
    patterns: &[String],
) -> Result<Option<String>> {
    let max_length = patterns.iter().map(String::len).max().unwrap_or(0);
    let mut previous = Vec::new();
    let mut found = None;
    let mut buffer = [0_u8; 4096];
    loop {
        let length = input
            .read(&mut buffer)
            .map_err(|error| Diagnostic::new(format!("cannot read runner output: {error}")))?;
        if length == 0 {
            break;
        }
        output
            .write_all(&buffer[..length])
            .map_err(|error| Diagnostic::new(format!("cannot write runner output: {error}")))?;
        output
            .flush()
            .map_err(|error| Diagnostic::new(format!("cannot flush runner output: {error}")))?;
        previous.extend_from_slice(&buffer[..length]);
        if found.is_none() {
            found = patterns
                .iter()
                .find(|pattern| {
                    previous
                        .windows(pattern.len())
                        .any(|window| window == pattern.as_bytes())
                })
                .cloned();
        }
        let keep = max_length.saturating_sub(1);
        if previous.len() > keep {
            previous.drain(..previous.len() - keep);
        }
    }
    Ok(found)
}
