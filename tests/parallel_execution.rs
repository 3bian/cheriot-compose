// Copyright 3Bian Limited and Contributors
// SPDX-License-Identifier: MIT

#![cfg(all(unix, feature = "host-tool-verification"))]

use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    os::unix::fs::PermissionsExt,
};

use cheriot_compose::{
    build_graph::{BuildAction, BuildCommand, BuildOperation, BuildOperationKind, BuildPlan},
    diagnostic::Sourced,
    execution::{self, ExecutionEvent},
};
use test_directory::TestDirectory;

#[path = "support/test_directory.rs"]
mod test_directory;

fn parallel_plan(fixture: &TestDirectory) -> BuildPlan {
    fixture.write_file(
        "parallel-tool",
        "#!/bin/sh\nset -eu\nprintf ready > \"$1\"\ni=0\nwhile test ! -f \"$2\"; do\n i=$((i + 1)); test \"$i\" -lt 200 || exit 1; sleep 0.01\ndone\n",
    );
    let tool = fixture.directory.join("parallel-tool");
    fs::set_permissions(&tool, fs::Permissions::from_mode(0o755)).unwrap();
    let mut plan = BuildPlan {
        schema: "cheriot-compose.plan/v1".into(),
        profile: "parallel execution test".into(),
        notes: Vec::new(),
        toolchain: None,
        build_dir: fixture.directory.join("build"),
        state_key: None,
        state_owner: None,
        publication: None,
        operations: Vec::new(),
        probes: BTreeSet::new(),
    };
    let first = plan.build_dir.join("first");
    let second = plan.build_dir.join("second");
    let command = |id: &str, output: &std::path::Path, other: &std::path::Path| BuildOperation {
        id: id.into(),
        kind: BuildOperationKind::Compile,
        policy: Default::default(),
        inputs: vec![],
        outputs: vec![output.to_path_buf()],
        dependencies: vec![],
        action: BuildAction::Command {
            command: BuildCommand {
                executable: tool.display().to_string(),
                argv: [output, other]
                    .iter()
                    .map(|p| Sourced::derived(p.display().to_string(), "parallel test"))
                    .collect(),
                cwd: fixture.directory.clone(),
                environment: BTreeMap::new(),
                stdout: None,
                stderr: None,
            },
            conditional: None,
        },
        depfile: None,
        provenance: vec![],
        fingerprint: String::new(),
    };
    plan.operations = vec![
        command("first", &first, &second),
        command("second", &second, &first),
        BuildOperation {
            id: "dependent".into(),
            kind: BuildOperationKind::Generate,
            policy: Default::default(),
            inputs: vec![first, second],
            outputs: vec![plan.build_dir.join("dependent")],
            dependencies: vec!["first".into(), "second".into()],
            action: BuildAction::Generate {
                content: "done".into(),
                template: "test".into(),
                template_fingerprint: "test".into(),
            },
            depfile: None,
            provenance: vec![],
            fingerprint: String::new(),
        },
    ];
    plan.validate().unwrap();
    plan
}

#[test]
fn independent_commands_overlap_and_dependents_wait_for_both() {
    let fixture = TestDirectory::new();
    let mut plan = parallel_plan(&fixture);
    let mut events = Vec::new();
    execution::execute_build_plan_with_jobs(&plan, false, true, 2, |event| {
        events.push(event.clone())
    })
    .unwrap();
    let started = |name: &str| {
        events.iter().position(|event| matches!(event, ExecutionEvent::Started { operation, .. } if operation == name)).unwrap()
    };
    let completed = |name: &str| {
        events.iter().position(|event| matches!(event, ExecutionEvent::Completed { operation, .. } if operation == name)).unwrap()
    };
    assert!(started("first") < completed("second"));
    assert!(started("second") < completed("first"));
    assert!(started("dependent") > completed("first"));
    assert!(started("dependent") > completed("second"));
    assert_eq!(fs::read(plan.build_dir.join("dependent")).unwrap(), b"done");
    events.clear();
    execution::execute_build_plan_with_jobs(&plan, false, true, 2, |event| {
        events.push(event.clone())
    })
    .unwrap();
    assert!(
        events
            .iter()
            .all(|event| !matches!(event, ExecutionEvent::Started { .. }))
    );
    fs::remove_file(plan.build_dir.join("first")).unwrap();
    events.clear();
    execution::execute_build_plan_with_jobs(&plan, false, true, 2, |event| {
        events.push(event.clone())
    })
    .unwrap();
    assert!(events.iter().any(|event| matches!(event, ExecutionEvent::Started { operation, .. } if operation == "dependent")));
    if let BuildAction::Command { command, .. } = &mut plan.operations[0].action {
        command.executable = "/bin/false".into();
    }
    fs::remove_file(plan.build_dir.join("dependent")).unwrap();
    events.clear();
    assert!(
        execution::execute_build_plan_with_jobs(&plan, false, true, 2, |event| events
            .push(event.clone()))
        .is_err()
    );
    assert!(events.iter().all(|event| !matches!(event, ExecutionEvent::Started { operation, .. } if operation == "dependent")));
    assert!(!plan.build_dir.join("dependent").exists());
    assert!(!plan.build_dir.join(".cheriot-compose.lock").exists());
}

#[test]
fn a_build_lock_protects_state_and_cleaning() {
    let fixture = TestDirectory::new();
    let plan = parallel_plan(&fixture);
    fs::create_dir_all(&plan.build_dir).unwrap();
    let lock = plan.build_dir.join(".cheriot-compose.lock");
    fs::write(&lock, "another build").unwrap();
    let error = execution::execute_build_plan_with_jobs(&plan, false, true, 2, |_| {}).unwrap_err();
    assert!(error.to_string().contains("locked"));
    assert!(
        execution::clean_build_plan(&plan)
            .unwrap_err()
            .to_string()
            .contains("locked")
    );
    assert_eq!(fs::read(&lock).unwrap(), b"another build");
}

#[test]
fn running_workers_finish_and_successful_state_is_kept_after_a_failure() {
    let fixture = TestDirectory::new();
    let mut plan = parallel_plan(&fixture);
    fixture.write_file(
        "parallel-tool",
        r#"#!/bin/sh
set -eu
if [ "$1" = "$FAIL_OUTPUT" ]; then
  i=0
  while [ ! -f "$2" ]; do i=$((i + 1)); test "$i" -lt 200; sleep 0.01; done
  exit 7
fi
printf started > "$1"
sleep 0.2
printf finished > "$1"
"#,
    );
    let first = plan.build_dir.join("first");
    for operation in plan.operations.iter_mut().take(2) {
        if let BuildAction::Command { command, .. } = &mut operation.action {
            command
                .environment
                .insert("FAIL_OUTPUT".into(), first.display().to_string());
        }
        operation.update_fingerprint().unwrap();
    }
    let mut events = Vec::new();
    assert!(
        execution::execute_build_plan_with_jobs(&plan, false, true, 2, |event| events
            .push(event.clone()))
        .is_err()
    );
    assert_eq!(
        fs::read(plan.build_dir.join("second")).unwrap(),
        b"finished"
    );
    assert!(events.iter().any(
        |event| matches!(event, ExecutionEvent::Failed { operation, .. } if operation == "first")
    ));
    assert!(events.iter().any(|event| matches!(event, ExecutionEvent::Completed { operation, .. } if operation == "second")));
    assert!(!events.iter().any(|event| matches!(event, ExecutionEvent::Started { operation, .. } if operation == "dependent")));
    if let BuildAction::Command { command, .. } = &mut plan.operations[0].action {
        command.executable = "/bin/sh".into();
        command.argv = [
            "-c".to_string(),
            "printf fixed > \"$1\"".to_string(),
            "sh".to_string(),
            first.display().to_string(),
        ]
        .into_iter()
        .map(|value| Sourced::derived(value, "failure recovery test"))
        .collect();
    }
    plan.operations[0].update_fingerprint().unwrap();
    events.clear();
    execution::execute_build_plan_with_jobs(&plan, false, true, 2, |event| {
        events.push(event.clone())
    })
    .unwrap();
    assert!(events.iter().any(
        |event| matches!(event, ExecutionEvent::Skipped { operation, .. } if operation == "second")
    ));
    assert_eq!(fs::read(plan.build_dir.join("dependent")).unwrap(), b"done");
}
