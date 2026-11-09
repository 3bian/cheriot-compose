// Copyright 3Bian Limited and Contributors
// SPDX-License-Identifier: MIT

#![cfg(all(unix, feature = "host-tool-verification"))]

use std::{collections::BTreeMap, fs, os::unix::fs::PermissionsExt, process::Command};

use cheriot_compose::{
    build_graph::{BuildAction, BuildPlan},
    execution,
};
use test_fixture::TestFixture;

#[path = "support/test_fixture.rs"]
mod test_fixture;
fn run_plan(
    plan: &mut BuildPlan,
    dry_run: bool,
) -> cheriot_compose::diagnostic::Result<Vec<String>> {
    let mut operations = Vec::new();
    execution::execute_build_plan(plan, dry_run, |event| match event {
        execution::ExecutionEvent::Planned { operation, reason }
        | execution::ExecutionEvent::Started {
            operation, reason, ..
        } => {
            operations.push(format!("{operation}: {reason}"));
        }
        _ => {}
    })?;
    Ok(operations)
}

fn fake_plan(fixture: &TestFixture) -> BuildPlan {
    fixture.write_file(
        "fake compiler ' tool",
        r#"#!/bin/sh
set -eu
printf 'cwd=%s\000env=%s\000' "$PWD" "${MARKER-}" >> "$LOG"
printf '%s\000' "$@" >> "$LOG"
printf '\n' >> "$LOG"
output=''
depfile=''
source=''
report=''
probe=false
while test "$#" -gt 0; do
  case "$1" in
    -fsyntax-only) probe=true ;;
    -o) shift; output=$1 ;;
    -MF) shift; depfile=$1 ;;
    --compartment-report=*) report=${1#*=} ;;
    *) source=$1 ;;
  esac
  shift
done
if "$probe"; then exit "${PROBE_RESULT-0}"; fi
if test -n "$output"; then printf '%s' 'test artifact' > "$output"; fi
if test -n "$report"; then printf '%s' '{}' > "$report"; fi
if test -n "$depfile"; then printf 'dummy: extra.h\n' > "$depfile"; fi
if test -z "$output"; then printf '%s\n' 'test stdout'; fi
"#,
    );
    let tool = fixture.directory.join("fake compiler ' tool");
    fs::set_permissions(&tool, fs::Permissions::from_mode(0o755)).unwrap();
    fixture.write_file("extra.h", "first header\n");
    let mut plan = fixture.build_plan();
    for op in &mut plan.operations {
        if let BuildAction::Command { command, .. } | BuildAction::Probe { command } =
            &mut op.action
        {
            command.executable = tool.display().to_string();
            command.environment = BTreeMap::from([
                (
                    "LOG".into(),
                    fixture.directory.join("calls.log").display().to_string(),
                ),
                (
                    "MARKER".into(),
                    "spaces ' quotes $HOME $(touch SHOULD_NOT_EXIST)".into(),
                ),
            ]);
        }
        op.update_fingerprint().unwrap();
    }
    plan
}

#[test]
fn shell_and_native_execute_the_same_complete_graph() {
    let fixture = TestFixture::new();
    let mut plan = fake_plan(&fixture);
    let script = cheriot_compose::render::render_shell_script(&plan).unwrap();
    fixture.write_file("build.sh", &script);
    assert!(
        Command::new("sh")
            .arg("-n")
            .arg(fixture.directory.join("build.sh"))
            .status()
            .unwrap()
            .success()
    );
    assert_eq!(run_plan(&mut plan, false).unwrap().len(), 62);
    let native = fs::read(fixture.directory.join("calls.log")).unwrap();
    let outputs: BTreeMap<_, _> = plan
        .operations
        .iter()
        .flat_map(|operation_2| operation_2.outputs.iter())
        .map(|path_2| (path_2.clone(), fs::read(path_2).unwrap()))
        .collect();
    execution::clean_build_plan(&plan).unwrap();
    fs::remove_file(fixture.directory.join("calls.log")).unwrap();
    let result = Command::new("sh")
        .arg(fixture.directory.join("build.sh"))
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert_eq!(
        native,
        fs::read(fixture.directory.join("calls.log")).unwrap()
    );
    for (p, v) in outputs {
        assert_eq!(fs::read(&p).unwrap(), v, "{}", p.display());
    }
    assert!(!fixture.directory.join("SHOULD_NOT_EXIST").exists());
}

#[test]
fn incremental_changes_headers_flags_outputs_and_tool_identity() {
    let fixture = TestFixture::new();
    let mut plan = fake_plan(&fixture);
    run_plan(&mut plan, false).unwrap();
    assert!(run_plan(&mut plan, true).unwrap().is_empty());
    assert!(run_plan(&mut plan, false).unwrap().is_empty());
    fixture.write_file("extra.h", "changed header\n");
    let dirty = run_plan(&mut plan, true).unwrap();
    assert!(dirty.iter().any(|text| text.starts_with("compile.")));
    run_plan(&mut plan, false).unwrap();
    let op = plan
        .operations
        .iter_mut()
        .find(|operation_2| operation_2.id == "compile.producer.0")
        .unwrap();
    if let BuildAction::Command { command, .. } = &mut op.action {
        command.argv.insert(
            1,
            cheriot_compose::diagnostic::Sourced::derived(
                "-DCHANGED".into(),
                "changed flag regression",
            ),
        );
    }
    op.update_fingerprint().unwrap();
    let dirty = run_plan(&mut plan, true).unwrap();
    assert!(dirty.iter().any(|text| text.contains("command, template")));
    run_plan(&mut plan, false).unwrap();
    let output = plan
        .operations
        .iter()
        .find(|operation_2| operation_2.id == "link.firmware")
        .unwrap()
        .outputs[0]
        .clone();
    fs::remove_file(output).unwrap();
    let dirty = run_plan(&mut plan, true).unwrap();
    assert!(
        dirty
            .iter()
            .any(|text| text.contains("link.firmware: output"))
    );
    run_plan(&mut plan, false).unwrap();
    let tool = fixture.directory.join("fake compiler ' tool");
    let mut source = fs::read_to_string(&tool).unwrap();
    source.push_str("# identity changed\n");
    fs::write(tool, source).unwrap();
    assert!(!run_plan(&mut plan, true).unwrap().is_empty());
}

#[test]
fn failure_stops_dependents_and_clean_preserves_user_files() {
    let fixture = TestFixture::new();
    let mut plan = fake_plan(&fixture);
    let op = plan
        .operations
        .iter_mut()
        .find(|operation_2| operation_2.id == "compile.producer.0")
        .unwrap();
    if let BuildAction::Command { command, .. } = &mut op.action {
        command.executable = "/bin/false".into();
    }
    op.update_fingerprint().unwrap();
    assert!(
        run_plan(&mut plan, false)
            .unwrap_err()
            .to_string()
            .contains("compile.producer.0 failed")
    );
    assert!(
        !plan
            .operations
            .iter()
            .find(|operation_2| operation_2.id == "link.firmware")
            .unwrap()
            .outputs[0]
            .exists()
    );
    fixture.write_file(".cheriot-compose/user-note.txt", "keep");
    execution::clean_build_plan(&plan).unwrap();
    assert_eq!(
        fs::read_to_string(fixture.directory.join(".cheriot-compose/user-note.txt")).unwrap(),
        "keep"
    );
}

#[test]
fn changed_file_writes_are_stable_and_symlinks_rejected() {
    let fixture = TestFixture::new();
    let path = fixture.directory.join("same");
    execution::write_if_changed(&path, b"hello").unwrap();
    let modified_time = fs::metadata(&path).unwrap().modified().unwrap();
    execution::write_if_changed(&path, b"hello").unwrap();
    assert_eq!(
        fs::metadata(&path).unwrap().modified().unwrap(),
        modified_time
    );
    let mut plan = fake_plan(&fixture);
    fs::create_dir_all(&plan.build_dir).unwrap();
    let output = plan.operations[0].outputs[0].clone();
    std::os::unix::fs::symlink(&path, &output).unwrap();
    assert!(
        run_plan(&mut plan, false)
            .unwrap_err()
            .to_string()
            .contains("symlink")
    );
    assert_eq!(fs::read(&path).unwrap(), b"hello");
}

#[test]
fn both_probe_branches_execute_equally() {
    let fixture = TestFixture::new();
    let mut plan = fake_plan(&fixture);
    for operation in &mut plan.operations {
        if let BuildAction::Probe { command } = &mut operation.action {
            command
                .environment
                .insert("PROBE_RESULT".into(), "1".into());
        }
        operation.update_fingerprint().unwrap();
    }
    fixture.write_file(
        "build.sh",
        &cheriot_compose::render::render_shell_script(&plan).unwrap(),
    );
    run_plan(&mut plan, false).unwrap();
    let probe_results = execution::probe_toolchain(&plan).unwrap();
    assert!(probe_results.values().all(|value| *value == Some(false)));
    let direct = fs::read(fixture.directory.join("calls.log")).unwrap();
    assert!(!String::from_utf8_lossy(&direct).contains("-cheri-bounds=subobject-safe"));
    execution::clean_build_plan(&plan).unwrap();
    fs::remove_file(fixture.directory.join("calls.log")).unwrap();
    let result = Command::new("sh")
        .arg(fixture.directory.join("build.sh"))
        .output()
        .unwrap();
    assert!(result.status.success());
    assert_eq!(
        direct,
        fs::read(fixture.directory.join("calls.log")).unwrap()
    );
}

#[test]
fn opt_in_probes_run_only_feature_checks() {
    let fixture = TestFixture::new();
    let plan = fake_plan(&fixture);
    let before = serde_json::to_string(&plan).unwrap();
    let probe_results = execution::probe_toolchain(&plan).unwrap();
    assert!(probe_results.values().all(|value| *value == Some(true)));
    assert_eq!(serde_json::to_string(&plan).unwrap(), before);
    let log = fs::read(fixture.directory.join("calls.log")).unwrap();
    let log = String::from_utf8(log).unwrap();
    assert_eq!(log.matches("-fsyntax-only").count(), 3);
    assert!(!log.contains("producer.cc"));
    assert!(
        !plan
            .operations
            .iter()
            .find(|operation_2| operation_2.id == "link.firmware")
            .unwrap()
            .outputs[0]
            .exists()
    );
}

#[test]
fn relative_tools_are_fingerprinted_using_each_command_directory_and_path() {
    use cheriot_compose::build_graph::{BuildCommand, BuildOperation, BuildOperationKind};

    let fixture = TestFixture::new();
    for directory in ["first", "second"] {
        fixture.write_file(
            &format!("{directory}/tool"),
            "#!/bin/sh\nprintf artifact > \"$1\"\n",
        );
        fs::set_permissions(
            fixture.directory.join(directory).join("tool"),
            fs::Permissions::from_mode(0o755),
        )
        .unwrap();
    }
    let build_dir = fixture.directory.join(".cheriot-compose");
    let mut operations = Vec::new();
    for (index, directory) in ["first", "second"].iter().enumerate() {
        let output = build_dir.join(format!("{directory}.out"));
        let command = BuildCommand {
            executable: if index == 0 {
                "./tool".into()
            } else {
                "tool".into()
            },
            argv: vec![cheriot_compose::diagnostic::Sourced::derived(
                output.display().to_string(),
                "test",
            )],
            cwd: fixture.directory.join(directory),
            environment: BTreeMap::from([("PATH".into(), ".".into())]),
            stdout: None,
            stderr: None,
        };
        let mut operation = BuildOperation {
            id: format!("test.{directory}"),
            kind: BuildOperationKind::Compile,
            policy: Default::default(),
            inputs: Vec::new(),
            outputs: vec![output],
            dependencies: Vec::new(),
            action: BuildAction::Command {
                command,
                conditional: None,
            },
            depfile: None,
            provenance: Vec::new(),
            fingerprint: String::new(),
        };
        operation.update_fingerprint().unwrap();
        operations.push(operation);
    }
    let mut plan = BuildPlan {
        schema: "test".into(),
        profile: "test".into(),
        notes: Vec::new(),
        toolchain: None,
        build_dir,
        state_key: None,
        state_owner: None,
        publication: None,
        operations,
        probes: std::collections::BTreeSet::new(),
    };
    run_plan(&mut plan, false).unwrap();
    assert!(run_plan(&mut plan, true).unwrap().is_empty());
    fixture.write_file("second/tool", "#!/bin/sh\nprintf changed > \"$1\"\n");
    let dirty = run_plan(&mut plan, true).unwrap();
    assert_eq!(dirty.len(), 1, "{dirty:?}");
    assert!(dirty[0].starts_with("test.second:"));
}

#[test]
fn successful_command_must_create_its_declared_output() {
    let fixture = TestFixture::new();
    let mut plan = fake_plan(&fixture);
    let operation = plan
        .operations
        .iter_mut()
        .find(|operation| operation.id == "compile.producer.0")
        .unwrap();
    if let BuildAction::Command { command, .. } = &mut operation.action {
        command.executable = "/bin/true".into();
    }
    operation.update_fingerprint().unwrap();
    let error = run_plan(&mut plan, false).unwrap_err();
    assert!(
        error.to_string().contains("succeeded without producing"),
        "{error}"
    );
    assert!(
        !plan
            .operations
            .iter()
            .find(|operation| operation.id == "link.firmware")
            .unwrap()
            .outputs[0]
            .exists()
    );
}

#[test]
fn execution_delivers_structured_success_skip_and_failure_events() {
    use cheriot_compose::execution::ExecutionEvent;
    let fixture = TestFixture::new();
    let mut plan = fake_plan(&fixture);
    let mut events = Vec::new();
    execution::execute_build_plan(&plan, false, |event| events.push(event.clone())).unwrap();
    assert!(events.iter().any(|event| matches!(event, ExecutionEvent::Started { operation, command: Some(_), .. } if operation == "compile.producer.0")));
    assert!(events.iter().any(|event| matches!(event, ExecutionEvent::Completed { operation, .. } if operation == "compile.producer.0")));
    assert!(matches!(
        events.last(),
        Some(ExecutionEvent::BuildCompleted { success: true, .. })
    ));
    events.clear();
    execution::execute_build_plan(&plan, false, |event| events.push(event.clone())).unwrap();
    assert!(events.iter().any(|event| matches!(event, ExecutionEvent::Skipped { operation, .. } if operation == "compile.producer.0")));
    let operation = plan
        .operations
        .iter_mut()
        .find(|operation| operation.id == "compile.producer.0")
        .unwrap();
    if let BuildAction::Command { command, .. } = &mut operation.action {
        command.executable = "/bin/false".into();
    }
    operation.update_fingerprint().unwrap();
    events.clear();
    assert!(
        execution::execute_build_plan(&plan, false, |event| events.push(event.clone())).is_err()
    );
    assert!(events.iter().any(|event| matches!(event, ExecutionEvent::Failed { operation, .. } if operation == "compile.producer.0")));
    assert!(matches!(
        events.last(),
        Some(ExecutionEvent::BuildCompleted { success: false, .. })
    ));
}

#[test]
fn one_and_multiple_workers_share_action_results_and_state() {
    let fixture = TestFixture::new();
    let plan = fake_plan(&fixture);
    let run = |jobs| {
        let mut events = Vec::new();
        let probes = execution::execute_build_plan_with_jobs(&plan, false, true, jobs, |event| {
            events.push(event.clone())
        })
        .unwrap();
        let outputs: BTreeMap<_, _> = plan
            .operations
            .iter()
            .flat_map(|operation| operation.outputs.iter())
            .map(|path| (path.clone(), fs::read(path).unwrap()))
            .collect();
        (probes, outputs, events)
    };
    let (serial_probes, serial_outputs, _) = run(1);
    let publication = plan.publication.as_ref().unwrap();
    assert_eq!(fs::read(&publication.elf).unwrap(), b"test artifact");
    assert_eq!(fs::read(&publication.report).unwrap(), b"{}");
    execution::clean_build_plan(&plan).unwrap();
    let (parallel_probes, parallel_outputs, _) = run(4);
    assert_eq!(serial_probes, parallel_probes);
    assert_eq!(serial_outputs, parallel_outputs);
    let (_, _, unchanged) = run(4);
    assert!(
        unchanged
            .iter()
            .all(|event| !matches!(event, execution::ExecutionEvent::Started { .. }))
    );
}

#[test]
fn missing_outputs_and_probe_results_are_consistent_for_all_worker_counts() {
    for jobs in [1, 4] {
        let fixture = TestFixture::new();
        let mut plan = fake_plan(&fixture);
        let probe = plan
            .operations
            .iter_mut()
            .find(|operation| matches!(operation.action, BuildAction::Probe { .. }))
            .unwrap();
        if let BuildAction::Probe { command } = &mut probe.action {
            command
                .environment
                .insert("PROBE_RESULT".into(), "1".into());
        }
        probe.update_fingerprint().unwrap();
        let results =
            execution::execute_build_plan_with_jobs(&plan, false, true, jobs, |_| {}).unwrap();
        assert!(results.values().any(|value| *value == Some(false)));
        let compile = plan
            .operations
            .iter_mut()
            .find(|operation| operation.id == "compile.producer.0")
            .unwrap();
        if let BuildAction::Command { command, .. } = &mut compile.action {
            command.executable = "/bin/true".into();
        }
        let missing_output = compile.outputs[0].clone();
        compile.update_fingerprint().unwrap();
        fs::remove_file(missing_output).unwrap();
        let previous = plan
            .publication
            .as_ref()
            .map(|publication| fs::read(&publication.elf).unwrap());
        let error =
            execution::execute_build_plan_with_jobs(&plan, false, true, jobs, |_| {}).unwrap_err();
        assert!(
            error.to_string().contains("succeeded without producing")
                || error.to_string().contains("failed")
        );
        assert_eq!(
            plan.publication
                .as_ref()
                .map(|publication| fs::read(&publication.elf).unwrap()),
            previous
        );
    }
}
