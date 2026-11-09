// Copyright 3Bian Limited and Contributors
// SPDX-License-Identifier: MIT

use std::{
    collections::{BTreeMap, BTreeSet},
    path::PathBuf,
    sync::mpsc,
};

use super::action::{self, ActionResult, OutputRouting, PreparedAction};
use super::{
    BuildAction, BuildOperation, BuildPlan, BuildState, Diagnostic, ExecutionEvent,
    ExecutionOptions, ProbeResults, Result, ToolStdout, guarded_output, load_state, output_digests,
    rebuild_reason, record, write_if_changed,
};

struct Coordinator<'a> {
    plan: &'a BuildPlan,
    state: BuildState,
    probes: ProbeResults,
    completed: BTreeSet<String>,
    changed: BTreeSet<String>,
    tools: BTreeMap<PathBuf, String>,
    executed: usize,
    skipped: usize,
}

impl<'a> Coordinator<'a> {
    fn new(plan: &'a BuildPlan) -> Result<Self> {
        let state = load_state(plan)?.unwrap_or_else(|| BuildState {
            owner: plan.state_owner.clone(),
            ..BuildState::default()
        });
        let mut probes = plan.unresolved_probes();
        for (id, value) in &mut probes {
            if let Some(saved) = state.operations.get(id) {
                *value = saved.probe;
            }
        }
        Ok(Self {
            plan,
            state,
            probes,
            completed: BTreeSet::new(),
            changed: BTreeSet::new(),
            tools: BTreeMap::new(),
            executed: 0,
            skipped: 0,
        })
    }

    fn prepare(
        &mut self,
        operation: &'a BuildOperation,
        dry_run: bool,
    ) -> Result<Option<PreparedAction<'a>>> {
        let reason = match record(operation, &self.probes, &mut self.tools) {
            Ok(current) => rebuild_reason(&current, self.state.operations.get(&operation.id)),
            Err(error) if dry_run => {
                Some(format!("cannot determine until prerequisite runs: {error}"))
            }
            Err(error) => return Err(error),
        }
        .or_else(|| {
            operation
                .dependencies
                .iter()
                .any(|id| self.changed.contains(id))
                .then(|| "prerequisite changed".into())
        })
        .or_else(|| operation.policy.always_rebuild_reason().map(str::to_owned));
        let Some(reason) = reason else {
            return Ok(None);
        };
        let previous_outputs = operation
            .policy
            .compare_outputs()
            .then(|| output_digests(operation));
        let command = if dry_run {
            None
        } else {
            match &operation.action {
                BuildAction::Command { .. } => operation.resolve_command(&self.probes)?,
                BuildAction::Probe { command } | BuildAction::GitDescription { command } => {
                    Some(command.clone())
                }
                _ => None,
            }
        };
        Ok(Some(PreparedAction {
            operation,
            command,
            reason,
            previous_outputs,
        }))
    }

    fn finish(&mut self, result: ActionResult<'a>) -> Result<bool> {
        result.show_output()?;
        let ActionResult { work, result, .. } = result;
        let operation = work.operation;
        let probe = result?;
        if matches!(operation.action, BuildAction::Probe { .. }) {
            self.probes.insert(operation.id.clone(), probe);
        }
        self.state.owned_outputs.extend(
            operation
                .outputs
                .iter()
                .chain(operation.depfile.iter())
                .filter(|path| {
                    path.starts_with(&self.plan.build_dir)
                        && self
                            .plan
                            .publication
                            .as_ref()
                            .is_none_or(|publication| **path != publication.registry)
                })
                .cloned(),
        );
        self.state.operations.insert(
            operation.id.clone(),
            record(operation, &self.probes, &mut self.tools)?,
        );
        let state_path = self.plan.state_path();
        guarded_output(&state_path, &self.plan.build_dir)?;
        write_if_changed(&state_path, &serde_json::to_vec_pretty(&self.state)?)?;
        let changed = work
            .previous_outputs
            .as_ref()
            .is_none_or(|previous| *previous != output_digests(operation));
        if changed {
            self.changed.insert(operation.id.clone());
        }
        self.completed.insert(operation.id.clone());
        Ok(changed)
    }
}

fn deliver_result<'a>(
    coordinator: &mut Coordinator<'a>,
    result: ActionResult<'a>,
    deliver: &mut impl FnMut(&ExecutionEvent),
) -> Result<()> {
    let id = result.work.operation.id.clone();
    match coordinator.finish(result) {
        Ok(changed) => {
            deliver(&ExecutionEvent::Completed {
                operation: id,
                changed,
            });
            Ok(())
        }
        Err(error) => {
            deliver(&ExecutionEvent::Failed {
                operation: id,
                message: error.to_string(),
            });
            Err(error)
        }
    }
}

pub(super) fn execute(
    plan: &BuildPlan,
    options: ExecutionOptions,
    mut deliver: impl FnMut(&ExecutionEvent),
) -> Result<ProbeResults> {
    let mut coordinator = Coordinator::new(plan)?;
    let mut failure = None;
    let batch_size = if options.dry_run { 1 } else { options.jobs };
    let stdout_to_stderr = options.tool_stdout == ToolStdout::Stderr;
    while coordinator.completed.len() < plan.operations.len() && failure.is_none() {
        let ready: Vec<_> = plan
            .operations
            .iter()
            .filter(|operation| {
                !coordinator.completed.contains(&operation.id)
                    && operation
                        .dependencies
                        .iter()
                        .all(|id| coordinator.completed.contains(id))
            })
            .take(batch_size)
            .collect();
        if ready.is_empty() {
            failure = Some(Diagnostic::new(
                "no build operation is ready; check the dependency graph",
            ));
            break;
        }
        let mut work = Vec::new();
        for operation in ready {
            match coordinator.prepare(operation, options.dry_run) {
                Ok(Some(item)) if options.dry_run => {
                    coordinator.executed += 1;
                    coordinator.changed.insert(operation.id.clone());
                    coordinator.completed.insert(operation.id.clone());
                    if matches!(operation.action, BuildAction::Probe { .. }) {
                        coordinator.probes.insert(operation.id.clone(), None);
                    }
                    deliver(&ExecutionEvent::Planned {
                        operation: operation.id.clone(),
                        reason: item.reason,
                    });
                }
                Ok(Some(item)) => work.push(item),
                Ok(None) => {
                    coordinator.skipped += 1;
                    coordinator.completed.insert(operation.id.clone());
                    deliver(&ExecutionEvent::Skipped {
                        operation: operation.id.clone(),
                        reason: "up to date".into(),
                    });
                }
                Err(error) => {
                    deliver(&ExecutionEvent::Failed {
                        operation: operation.id.clone(),
                        message: error.to_string(),
                    });
                    failure = Some(error);
                    break;
                }
            }
        }
        if failure.is_some() || options.dry_run {
            continue;
        }
        for item in &work {
            coordinator.executed += 1;
            deliver(&ExecutionEvent::Started {
                operation: item.operation.id.clone(),
                reason: item.reason.clone(),
                command: item.command.clone(),
            });
        }
        if options.jobs == 1 {
            for item in work {
                let result =
                    action::perform(item, plan, OutputRouting::Stream { stdout_to_stderr });
                if let Err(error) = deliver_result(&mut coordinator, result, &mut deliver) {
                    failure = Some(error);
                    break;
                }
            }
        } else {
            std::thread::scope(|scope| {
                let (sender, receiver) = mpsc::channel();
                let mut handles = Vec::new();
                for item in work {
                    let sender = sender.clone();
                    handles.push(scope.spawn(move || {
                        let result =
                            action::perform(item, plan, OutputRouting::Group { stdout_to_stderr });
                        let _ = sender.send(result);
                    }));
                }
                drop(sender);
                for result in receiver {
                    if let Err(error) = deliver_result(&mut coordinator, result, &mut deliver)
                        && failure.is_none()
                    {
                        failure = Some(error);
                    }
                }
                for handle in handles {
                    if handle.join().is_err() && failure.is_none() {
                        failure = Some(Diagnostic::new(
                            "parallel build worker stopped unexpectedly",
                        ));
                    }
                }
            });
        }
    }
    deliver(&ExecutionEvent::BuildCompleted {
        executed: coordinator.executed,
        skipped: coordinator.skipped,
        dry_run: options.dry_run,
        success: failure.is_none(),
    });
    match failure {
        Some(error) => Err(error),
        None => Ok(coordinator.probes),
    }
}
