// Copyright 3Bian Limited and Contributors
// SPDX-License-Identifier: MIT

#![cfg(all(unix, feature = "host-tool-verification"))]

use std::{fs, os::unix::fs::PermissionsExt, path::Path, process::Command};

use cheriot_compose::{
    build_graph::BuildAction,
    configuration::ProjectSettings,
    diagnostic::Origin,
    execution::{self, ExecutionEvent},
    planning, render,
};
use test_fixture::TestFixture;

#[path = "support/test_fixture.rs"]
mod test_fixture;

fn setup() -> TestFixture {
    let fixture = TestFixture::new();
    fixture.build_plan();
    fixture.write_file(
        "fake-compiler",
        r#"#!/bin/sh
set -eu
printf '%s\n' called >> "$CALLS"
output=''; depfile=''; report=''
while [ "$#" -gt 0 ]; do
  case "$1" in
    -o) shift; output=$1 ;;
    -MF) shift; depfile=$1 ;;
    --compartment-report=*) report=${1#*=} ;;
  esac
  shift
done
if [ -n "$output" ]; then printf 'compiled artifact\n' > "$output"; fi
if [ -n "$depfile" ]; then printf 'dummy: producer.cc\n' > "$depfile"; fi
if [ -n "$report" ]; then printf '{"firmware":"fixture"}\n' > "$report"; fi
"#,
    );
    let compiler = fixture.directory.join("fake-compiler");
    fs::set_permissions(&compiler, fs::Permissions::from_mode(0o755)).unwrap();
    fixture.set_fake_llvm(&compiler);
    fixture
}

fn plan(
    fixture: &TestFixture,
    output: Option<&Path>,
    mode: Option<&str>,
) -> cheriot_compose::build_graph::BuildPlan {
    let settings = ProjectSettings {
        output_directory: output
            .map(|path| (path.to_path_buf(), Origin::derived("test destination"))),
        mode: mode.map(|mode| (mode.to_owned(), Origin::derived("test mode"))),
        ..ProjectSettings::default()
    };
    let project = cheriot_compose::project::load_project_with_settings(
        &fixture.project_file(),
        None,
        &settings,
    )
    .unwrap();
    let mut plan = planning::create_build_plan(&project).unwrap();
    for operation in &mut plan.operations {
        if let BuildAction::Command { command, .. } | BuildAction::Probe { command } =
            &mut operation.action
        {
            command.environment.insert(
                "CALLS".into(),
                fixture.directory.join("calls.log").display().to_string(),
            );
        }
        operation.update_fingerprint().unwrap();
    }
    plan
}

fn execute(plan: &cheriot_compose::build_graph::BuildPlan) -> Vec<ExecutionEvent> {
    let mut events = Vec::new();
    execution::execute_build_plan_with_jobs(plan, false, true, 3, |event| {
        events.push(event.clone())
    })
    .unwrap();
    events
}

fn started(events: &[ExecutionEvent], prefix: &str) -> bool {
    events.iter().any(|event| matches!(event, ExecutionEvent::Started { operation, .. } if operation.starts_with(prefix)))
}

#[test]
fn default_outputs_are_separate_and_intermediates_are_shallow() {
    let fixture = setup();
    let plan = plan(&fixture, None, None);
    assert_eq!(
        plan.build_dir,
        fixture
            .directory
            .join(".cheriot-compose/sail/cheriot/release")
    );
    let publication = plan.publication.as_ref().unwrap();
    assert_eq!(
        publication.elf,
        fixture.directory.join("producer-consumer.elf")
    );
    assert_eq!(
        publication.report,
        fixture.directory.join("producer-consumer.json")
    );
    for operation in &plan.operations {
        if matches!(operation.action, BuildAction::Publish) {
            continue;
        }
        for path in operation.outputs.iter().chain(operation.depfile.iter()) {
            let parent = path.parent().unwrap();
            if parent != plan.build_dir {
                assert_eq!(parent.parent(), Some(plan.build_dir.as_path()));
                assert!(
                    parent
                        .file_name()
                        .unwrap()
                        .to_str()
                        .unwrap()
                        .starts_with("loader-")
                );
                assert!(
                    path.file_name()
                        .unwrap()
                        .to_str()
                        .unwrap()
                        .starts_with("boot.")
                );
            }
        }
    }
    execute(&plan);
    assert_eq!(fs::read(&publication.elf).unwrap(), b"compiled artifact\n");
    assert_eq!(
        fs::read(&publication.report).unwrap(),
        b"{\"firmware\":\"fixture\"}\n"
    );
}

#[test]
fn missing_outputs_republish_and_changing_destination_reuses_compilation() {
    let fixture = setup();
    let first_dir = fixture.directory.join("first destination");
    let second_dir = fixture.directory.join("second destination");
    let first = plan(&fixture, Some(&first_dir), None);
    execute(&first);
    fs::remove_file(first_dir.join("producer-consumer.json")).unwrap();
    let events = execute(&first);
    assert!(started(&events, "publish.firmware"));
    assert!(!started(&events, "compile."));
    assert!(!started(&events, "link.firmware"));
    assert!(first_dir.join("producer-consumer.json").is_file());
    fs::remove_file(first_dir.join("producer-consumer.elf")).unwrap();
    let events = execute(&first);
    assert!(started(&events, "publish.firmware"));
    assert!(!started(&events, "compile."));
    assert!(!started(&events, "link.firmware"));
    let second = plan(&fixture, Some(&second_dir), None);
    assert_eq!(first.build_dir, second.build_dir);
    assert_eq!(first.state_key, second.state_key);
    let first_compiles: Vec<_> = first
        .operations
        .iter()
        .filter(|operation| operation.id.starts_with("compile."))
        .map(|operation| &operation.outputs)
        .collect();
    let second_compiles: Vec<_> = second
        .operations
        .iter()
        .filter(|operation| operation.id.starts_with("compile."))
        .map(|operation| &operation.outputs)
        .collect();
    assert_eq!(first_compiles, second_compiles);
    let events = execute(&second);
    assert!(started(&events, "publish.firmware"));
    assert!(!started(&events, "compile."));
    assert!(!started(&events, "link.firmware"));
    fixture.write_file("first destination/notes.txt", "keep first");
    fixture.write_file("second destination/notes.txt", "keep second");
    execution::clean_build_plan(&second).unwrap();
    for directory in [&first_dir, &second_dir] {
        assert!(!directory.join("producer-consumer.elf").exists());
        assert!(!directory.join("producer-consumer.json").exists());
        assert!(directory.join("notes.txt").is_file());
    }
}

#[test]
fn collisions_and_user_modified_outputs_are_rejected_before_work() {
    let fixture = setup();
    let out = fixture.directory.join("shared");
    fs::create_dir(&out).unwrap();
    let elf = out.join("producer-consumer.elf");
    fs::write(&elf, "user data").unwrap();
    let release = plan(&fixture, Some(&out), None);
    let mut events = Vec::new();
    let error = execution::execute_build_plan_with_jobs(&release, false, true, 2, |event| {
        events.push(event.clone())
    })
    .unwrap_err();
    assert!(error.to_string().contains("unowned firmware"));
    assert!(events.is_empty());
    assert_eq!(fs::read_to_string(&elf).unwrap(), "user data");
    fs::remove_file(&elf).unwrap();
    execute(&release);
    let debug = plan(&fixture, Some(&out), Some("debug"));
    events.clear();
    let error = execution::execute_build_plan_with_jobs(&debug, false, true, 2, |event| {
        events.push(event.clone())
    })
    .unwrap_err();
    assert!(error.to_string().contains("collision"));
    assert!(events.is_empty());
    fs::write(&elf, "changed by user").unwrap();
    assert!(
        execution::execute_build_plan(&release, false, |_| {})
            .unwrap_err()
            .to_string()
            .contains("modified output")
    );
    assert!(
        execution::clean_build_plan(&release)
            .unwrap_err()
            .to_string()
            .contains("modified output")
    );
    assert_eq!(fs::read_to_string(&elf).unwrap(), "changed by user");
}

#[test]
fn shell_publication_matches_native_and_clean_keeps_unrelated_files() {
    let fixture = setup();
    let destination = fixture.directory.join("shell ' space $(not executed)");
    let plan = plan(&fixture, Some(&destination), None);
    execute(&plan);
    let expected: Vec<_> = plan
        .publication
        .as_ref()
        .unwrap()
        .outputs()
        .iter()
        .map(|path| (path.clone(), fs::read(path).unwrap()))
        .collect();
    execution::clean_build_plan(&plan).unwrap();
    fixture.write_file("build.sh", &render::render_shell_script(&plan).unwrap());
    let output = Command::new("sh")
        .arg(fixture.directory.join("build.sh"))
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    for (path, contents) in expected {
        assert_eq!(fs::read(path).unwrap(), contents);
    }
    fixture.write_file("shell ' space $(not executed)/keep.txt", "untouched");
    execution::clean_build_plan(&plan).unwrap();
    assert_eq!(
        fs::read_to_string(destination.join("keep.txt")).unwrap(),
        "untouched"
    );
    assert!(!fixture.directory.join("not").exists());
}

#[test]
fn output_directory_precedence_and_relative_paths_follow_their_source() {
    let fixture = setup();
    fixture.replace_text(
        "cheriot-compose.toml",
        "mode = \"release\"",
        "output_dir = \"toml-products\"\nmode = \"release\"",
    );
    fs::create_dir(fixture.directory.join("invocation")).unwrap();
    let invocation = fixture.directory.join("invocation");
    let run = |extra: &[&str], environment: Option<&str>| {
        let mut command = Command::new(env!("CARGO_BIN_EXE_cheriot-compose"));
        command
            .current_dir(&invocation)
            .args(["configuration", "--format", "json", "--file"])
            .arg(fixture.project_file())
            .args(extra);
        if let Some(value) = environment {
            command.env("CHERIOT_COMPOSE_OUTPUT_DIR", value);
        } else {
            command.env_remove("CHERIOT_COMPOSE_OUTPUT_DIR");
        }
        let result = command.output().unwrap();
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        serde_json::from_slice::<serde_json::Value>(&result.stdout).unwrap()["output_dir"]
            .as_str()
            .unwrap()
            .to_owned()
    };
    assert_eq!(
        run(&[], None),
        fixture
            .directory
            .join("toml-products")
            .display()
            .to_string()
    );
    assert_eq!(
        run(&[], Some("env-products")),
        invocation.join("env-products").display().to_string()
    );
    assert_eq!(
        run(&["--output-dir", "cli-products"], Some("env-products")),
        invocation.join("cli-products").display().to_string()
    );
    let result = Command::new(env!("CARGO_BIN_EXE_cheriot-compose"))
        .current_dir(&invocation)
        .args(["configuration", "--format", "json", "--file"])
        .arg(fixture.project_file())
        .args(["--build-dir", "scratch", "--output-dir", "dist"])
        .output()
        .unwrap();
    assert!(result.status.success());
    let configuration: serde_json::Value = serde_json::from_slice(&result.stdout).unwrap();
    assert_eq!(
        configuration["build_dir"],
        invocation
            .join("scratch/sail/cheriot/release")
            .display()
            .to_string()
    );
    assert_eq!(
        configuration["output_dir"],
        invocation.join("dist").display().to_string()
    );
}

#[test]
fn different_projects_cannot_publish_the_same_names_into_one_directory() {
    let first_fixture = setup();
    let second_fixture = setup();
    let destination = first_fixture.directory.join("shared-output");
    let first = plan(&first_fixture, Some(&destination), None);
    let second = plan(&second_fixture, Some(&destination), None);
    execute(&first);
    let original = fs::read(destination.join("producer-consumer.elf")).unwrap();
    let mut events = Vec::new();
    let error = execution::execute_build_plan_with_jobs(&second, false, true, 2, |event| {
        events.push(event.clone())
    })
    .unwrap_err();
    assert!(error.to_string().contains("collision"));
    assert!(events.is_empty());
    assert_eq!(
        fs::read(destination.join("producer-consumer.elf")).unwrap(),
        original
    );
}

#[test]
fn multiple_firmwares_share_the_variant_directory_without_sharing_outputs() {
    let fixture = setup();
    let mut document = fs::read_to_string(fixture.project_file()).unwrap();
    document.push_str("\n[[firmwares]]\nname = \"another\"\nservices = [\"producer\"]\nthreads = [{ name = \"main\", compartment = \"producer\", entry = \"run\", priority = 1, stack_size = 512, trusted_stack_frames = 1 }]\n");
    fixture.write_file("cheriot-compose.toml", &document);
    let destination = fixture.directory.join("multiple");
    let settings = ProjectSettings {
        output_directory: Some((destination.clone(), Origin::derived("test destination"))),
        ..ProjectSettings::default()
    };
    let make = |name: &str| {
        let project = cheriot_compose::project::load_project_with_settings(
            &fixture.project_file(),
            Some(name),
            &settings,
        )
        .unwrap();
        let mut plan = planning::create_build_plan(&project).unwrap();
        for operation in &mut plan.operations {
            if let BuildAction::Command { command, .. } | BuildAction::Probe { command } =
                &mut operation.action
            {
                command.environment.insert(
                    "CALLS".into(),
                    fixture.directory.join("calls.log").display().to_string(),
                );
            }
            operation.update_fingerprint().unwrap();
        }
        plan
    };
    let first = make("producer-consumer");
    let second = make("another");
    assert_eq!(first.build_dir, second.build_dir);
    assert_ne!(first.state_key, second.state_key);
    execute(&first);
    execute(&second);
    assert!(destination.join("producer-consumer.elf").is_file());
    assert!(destination.join("another.elf").is_file());
    execution::clean_build_plan(&first).unwrap();
    assert!(!destination.join("producer-consumer.elf").exists());
    assert!(destination.join("another.elf").is_file());
    assert!(second.build_dir.join("board-interrupts.h").is_file());
    let events = execute(&second);
    assert!(!started(&events, "compile."));
    assert!(destination.join("another.json").is_file());
}

#[test]
fn failed_link_never_replaces_the_previous_elf_or_report() {
    let fixture = setup();
    let destination = fixture.directory.join("results");
    let mut plan = plan(&fixture, Some(&destination), None);
    execute(&plan);
    let elf = destination.join("producer-consumer.elf");
    let report = destination.join("producer-consumer.json");
    let original_elf = fs::read(&elf).unwrap();
    let original_report = fs::read(&report).unwrap();
    let link = plan
        .operations
        .iter_mut()
        .find(|operation| operation.id == "link.firmware")
        .unwrap();
    if let BuildAction::Command { command, .. } = &mut link.action {
        command.executable = "/bin/false".into();
    }
    link.update_fingerprint().unwrap();
    assert!(execution::execute_build_plan_with_jobs(&plan, false, true, 3, |_| {}).is_err());
    assert_eq!(fs::read(elf).unwrap(), original_elf);
    assert_eq!(fs::read(report).unwrap(), original_report);
}

#[test]
fn shell_and_native_builds_respect_the_same_lock_and_collision_checks() {
    use std::{
        process::Stdio,
        time::{Duration, Instant},
    };
    let fixture = setup();
    let compiler = fixture.directory.join("fake-compiler");
    let text = fs::read_to_string(&compiler).unwrap().replacen(
        "#!/bin/sh\n",
        "#!/bin/sh\nif [ ! -e \"$SLOW_MARKER\" ]; then : > \"$SLOW_MARKER\"; sleep 1; fi\n",
        1,
    );
    fs::write(&compiler, text).unwrap();
    let destination = fixture.directory.join("locked-output");
    let release = plan(&fixture, Some(&destination), None);
    fixture.write_file(
        "parallel-shell.sh",
        &render::render_shell_script(&release).unwrap(),
    );
    let shell = Command::new("sh")
        .arg(fixture.directory.join("parallel-shell.sh"))
        .env("SLOW_MARKER", fixture.directory.join("started"))
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(5);
    let lock = release.build_dir.join(".cheriot-compose.lock");
    while !lock.exists() && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(10));
    }
    assert!(lock.is_dir());
    let error = execution::execute_build_plan(&release, false, |_| {}).unwrap_err();
    assert!(error.to_string().contains("locked"));
    let output = shell.wait_with_output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(!lock.exists());
    let debug = plan(&fixture, Some(&destination), Some("debug"));
    fixture.write_file(
        "collision-shell.sh",
        &render::render_shell_script(&debug).unwrap(),
    );
    let calls = fs::read(fixture.directory.join("calls.log")).unwrap();
    let output = Command::new("sh")
        .arg(fixture.directory.join("collision-shell.sh"))
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("collision"));
    assert_eq!(
        fs::read(fixture.directory.join("calls.log")).unwrap(),
        calls
    );
}

#[test]
fn an_overlay_without_an_output_setting_does_not_change_the_default() {
    let fixture = setup();
    fixture.write_file(
        "other/overlay.toml",
        "patches = [{ op = \"replace\", path = \"/mode\", value = \"debug\" }]\n",
    );
    let settings = ProjectSettings {
        overlays: vec![fixture.directory.join("other/overlay.toml")],
        ..ProjectSettings::default()
    };
    let project = cheriot_compose::project::load_project_with_settings(
        &fixture.project_file(),
        None,
        &settings,
    )
    .unwrap();
    assert_eq!(project.output_dir, fixture.directory);
    fixture.write_file(
        "other/overlay.toml",
        "patches = [{ op = \"add\", path = \"/output_dir\", value = \"relative-products\" }]\n",
    );
    let project = cheriot_compose::project::load_project_with_settings(
        &fixture.project_file(),
        None,
        &settings,
    )
    .unwrap();
    assert_eq!(
        project.output_dir,
        fixture.directory.join("other/relative-products")
    );
}

#[test]
fn run_uses_the_relocated_elf_and_keeps_runner_preparation_in_the_variant_directory() {
    let fixture = setup();
    fixture.write_file(
        "runner",
        "#!/bin/sh\nprintf '%s\\n' \"$PWD\" \"$1\" >> \"$RUN_RECORD\"\nprintf 'guest output\\n'\n",
    );
    let runner = fixture.directory.join("runner");
    fs::set_permissions(&runner, fs::Permissions::from_mode(0o755)).unwrap();
    fixture.replace_text(
        "boards/sail.toml",
        "runner = { executable = \"cheriot_sim\", failure_patterns = [\"Assertion failure\"] }",
        &format!("runner = {{ executable = {:?} }}", runner.to_str().unwrap()),
    );
    let invocation = fixture.directory.join("invocation");
    fs::create_dir(&invocation).unwrap();
    for _ in 0..2 {
        let output = Command::new(env!("CARGO_BIN_EXE_cheriot-compose"))
            .current_dir(&invocation)
            .args(["run", "--file"])
            .arg(fixture.project_file())
            .args(["--output-dir", "dist", "--jobs", "2"])
            .env("CALLS", fixture.directory.join("calls.log"))
            .env("RUN_RECORD", fixture.directory.join("runs.log"))
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(String::from_utf8_lossy(&output.stdout), "guest output\n");
        let progress = String::from_utf8_lossy(&output.stderr);
        assert!(
            progress.contains(
                &invocation
                    .join("dist/producer-consumer.elf")
                    .display()
                    .to_string()
            )
        );
        assert!(
            progress.contains(
                &invocation
                    .join("dist/producer-consumer.json")
                    .display()
                    .to_string()
            )
        );
    }
    let record = fs::read_to_string(fixture.directory.join("runs.log")).unwrap();
    let lines: Vec<_> = record.lines().collect();
    let working = fixture
        .directory
        .join(".cheriot-compose/sail/cheriot/release");
    let firmware = invocation.join("dist/producer-consumer.elf");
    assert_eq!(
        lines,
        [
            working.to_str().unwrap(),
            firmware.to_str().unwrap(),
            working.to_str().unwrap(),
            firmware.to_str().unwrap()
        ]
    );
}

#[test]
fn disabling_the_report_removes_the_previous_owned_report() {
    let fixture = setup();
    let destination = fixture.directory.join("reports");
    let with_report = plan(&fixture, Some(&destination), None);
    execute(&with_report);
    assert!(destination.join("producer-consumer.json").is_file());
    fixture.replace_text(
        "cheriot-compose.toml",
        "[[firmwares]]\nname = \"producer-consumer\"",
        "[[firmwares]]\ncompartment_report = false\nname = \"producer-consumer\"",
    );
    let without_report = plan(&fixture, Some(&destination), None);
    execute(&without_report);
    assert!(destination.join("producer-consumer.elf").is_file());
    assert!(!destination.join("producer-consumer.json").exists());
}

#[test]
fn unowned_report_blocks_enabling_report_generation() {
    let fixture = setup();
    fixture.replace_text(
        "cheriot-compose.toml",
        "[[firmwares]]\nname = \"producer-consumer\"",
        "[[firmwares]]\ncompartment_report = false\nname = \"producer-consumer\"",
    );
    let destination = fixture.directory.join("reports");
    let without_report = plan(&fixture, Some(&destination), None);
    execute(&without_report);
    fs::write(destination.join("producer-consumer.json"), "user report").unwrap();
    fixture.replace_text("cheriot-compose.toml", "compartment_report = false\n", "");
    let with_report = plan(&fixture, Some(&destination), None);
    let error = execution::execute_build_plan(&with_report, false, |_| {}).unwrap_err();
    assert!(error.to_string().contains("unowned report"));
    assert_eq!(
        fs::read_to_string(destination.join("producer-consumer.json")).unwrap(),
        "user report"
    );
}

#[test]
fn a_final_artifact_cannot_overwrite_a_declared_input() {
    let fixture = setup();
    fixture.write_file("producer-consumer.elf", "project input");
    let mut document = fs::read_to_string(fixture.project_file()).unwrap();
    document.push_str("\n[generators.read_input]\nexecutable = \"tool\"\ninputs = [\"producer-consumer.elf\"]\noutputs = [\"generated.txt\"]\n");
    fixture.write_file("cheriot-compose.toml", &document);
    let project = fixture.resolved_project();
    let error = planning::create_build_plan(&project).unwrap_err();
    assert!(
        error
            .to_string()
            .contains("overwrite a source or configuration")
    );
    assert_eq!(
        fs::read_to_string(fixture.directory.join("producer-consumer.elf")).unwrap(),
        "project input"
    );
}

#[test]
fn clean_can_remove_owned_final_files_after_the_intermediate_directory_was_removed() {
    let fixture = setup();
    let destination = fixture.directory.join("products");
    let plan = plan(&fixture, Some(&destination), None);
    execute(&plan);
    fs::remove_dir_all(&plan.build_dir).unwrap();
    execution::clean_build_plan(&plan).unwrap();
    assert!(!destination.join("producer-consumer.elf").exists());
    assert!(!destination.join("producer-consumer.json").exists());
    assert!(destination.is_dir());
}

#[test]
fn a_failed_publication_keeps_the_previous_firmware_and_report_together() {
    use cheriot_compose::{
        build_graph::{BuildCommand, BuildOperation, BuildOperationKind},
        diagnostic::Sourced,
    };
    let fixture = setup();
    let destination = fixture.directory.join("atomic-publication");
    let mut plan = plan(&fixture, Some(&destination), None);
    execute(&plan);
    let final_elf = destination.join("producer-consumer.elf");
    let final_report = destination.join("producer-consumer.json");
    let old_elf = fs::read(&final_elf).unwrap();
    let old_report = fs::read(&final_report).unwrap();
    let position = plan
        .operations
        .iter()
        .position(|operation| operation.id == "publish.firmware")
        .unwrap();
    let report_input = plan.operations[position].inputs[1].clone();
    let mut remove = BuildOperation {
        id: "test.remove-report".into(),
        kind: BuildOperationKind::PostProcess,
        policy: Default::default(),
        inputs: vec![report_input.clone()],
        outputs: vec![],
        dependencies: vec!["dump.firmware".into()],
        action: BuildAction::Command {
            command: BuildCommand {
                executable: "/bin/rm".into(),
                argv: vec![Sourced::derived(
                    report_input.display().to_string(),
                    "failure test",
                )],
                cwd: fixture.directory.clone(),
                environment: Default::default(),
                stdout: None,
                stderr: None,
            },
            conditional: None,
        },
        depfile: None,
        provenance: vec![],
        fingerprint: String::new(),
    };
    remove.update_fingerprint().unwrap();
    plan.operations[position]
        .dependencies
        .push(remove.id.clone());
    plan.operations[position].update_fingerprint().unwrap();
    plan.operations.insert(position, remove);
    assert!(execution::execute_build_plan_with_jobs(&plan, false, true, 2, |_| {}).is_err());
    assert_eq!(fs::read(final_elf).unwrap(), old_elf);
    assert_eq!(fs::read(final_report).unwrap(), old_report);
}

#[test]
fn unowned_build_metadata_is_not_overwritten_by_native_or_shell_builds() {
    let fixture = setup();
    let mut plan = plan(&fixture, Some(&fixture.directory.join("dist")), None);
    let ownership = plan
        .operations
        .iter_mut()
        .find(|operation| operation.id == "record.outputs")
        .unwrap();
    ownership.id = "record.renamed".into();
    ownership.update_fingerprint().unwrap();
    let record = plan
        .operations
        .iter()
        .find(|operation| operation.id == "record.renamed")
        .unwrap()
        .outputs[0]
        .clone();
    fs::create_dir_all(&plan.build_dir).unwrap();
    fs::write(&record, "unrelated data").unwrap();
    let mut events = Vec::new();
    let error = execution::execute_build_plan_with_jobs(&plan, false, true, 2, |event| {
        events.push(event.clone())
    })
    .unwrap_err();
    assert!(error.to_string().contains("unowned build record"));
    assert!(events.is_empty());
    fixture.write_file(
        "metadata-collision.sh",
        &render::render_shell_script(&plan).unwrap(),
    );
    let shell = Command::new("sh")
        .arg(fixture.directory.join("metadata-collision.sh"))
        .output()
        .unwrap();
    assert!(!shell.status.success());
    assert!(String::from_utf8_lossy(&shell.stderr).contains("unowned build record"));
    assert!(!fixture.directory.join("calls.log").exists());
    assert_eq!(fs::read_to_string(record).unwrap(), "unrelated data");
}

#[test]
fn distinct_board_files_with_the_same_name_cannot_share_a_final_path() {
    let fixture = setup();
    let board = fs::read_to_string(fixture.directory.join("boards/sail.toml")).unwrap();
    fixture.write_file("boards/second.toml", &board);
    let destination = fixture.directory.join("same-named-boards");
    let first = plan(&fixture, Some(&destination), None);
    execute(&first);
    let settings = ProjectSettings {
        output_directory: Some((destination.clone(), Origin::derived("test destination"))),
        board: Some((
            fixture
                .directory
                .join("boards/second.toml")
                .display()
                .to_string(),
            Origin::derived("test board"),
        )),
        ..ProjectSettings::default()
    };
    let project = cheriot_compose::project::load_project_with_settings(
        &fixture.project_file(),
        None,
        &settings,
    )
    .unwrap();
    let second = planning::create_build_plan(&project).unwrap();
    assert_eq!(first.build_dir, second.build_dir);
    assert_ne!(first.state_key, second.state_key);
    let mut events = Vec::new();
    let error = execution::execute_build_plan_with_jobs(&second, false, true, 2, |event| {
        events.push(event.clone())
    })
    .unwrap_err();
    assert!(error.to_string().contains("collision"));
    assert!(events.is_empty());
}

#[test]
fn a_running_firmware_holds_the_intermediate_and_output_locks() {
    use std::time::{Duration, Instant};
    let fixture = setup();
    fixture.write_file(
        "slow-runner",
        "#!/bin/sh\nprintf started > \"$RUN_STARTED\"\nsleep 1\n",
    );
    let script = fixture.directory.join("slow-runner");
    fs::set_permissions(&script, fs::Permissions::from_mode(0o755)).unwrap();
    fixture.replace_text(
        "boards/sail.toml",
        "runner = { executable = \"cheriot_sim\", failure_patterns = [\"Assertion failure\"] }",
        &format!(
            "runner = {{ executable = {:?}, environment = {{ RUN_STARTED = {:?} }} }}",
            script.to_str().unwrap(),
            fixture.directory.join("runner-started").to_str().unwrap()
        ),
    );
    let destination = fixture.directory.join("runner-output");
    let plan = plan(&fixture, Some(&destination), None);
    execute(&plan);
    let settings = ProjectSettings {
        output_directory: Some((destination.clone(), Origin::derived("test destination"))),
        ..ProjectSettings::default()
    };
    let project = cheriot_compose::project::load_project_with_settings(
        &fixture.project_file(),
        None,
        &settings,
    )
    .unwrap();
    let runner =
        cheriot_compose::planning::runner::create_runner_plan(&project, &plan, None).unwrap();
    let plan_for_runner = plan.clone();
    let worker =
        std::thread::spawn(move || execution::execute_runner_for_plan(&plan_for_runner, &runner));
    let deadline = Instant::now() + Duration::from_secs(5);
    while !fixture.directory.join("runner-started").exists() && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(10));
    }
    assert!(fixture.directory.join("runner-started").exists());
    assert!(
        execution::clean_build_plan(&plan)
            .unwrap_err()
            .to_string()
            .contains("locked")
    );
    assert!(destination.join("producer-consumer.elf").exists());
    worker.join().unwrap().unwrap();
    execution::clean_build_plan(&plan).unwrap();
    assert!(!destination.join("producer-consumer.elf").exists());
}

#[test]
fn stale_link_outputs_cannot_be_published_after_a_false_success() {
    let fixture = setup();
    let destination = fixture.directory.join("stale-link");
    let mut plan = plan(&fixture, Some(&destination), None);
    for operation in &mut plan.operations {
        if operation.id == "link.firmware" {
            operation.id = "renamed.firmware-link".into();
        }
        for dependency in &mut operation.dependencies {
            if dependency == "link.firmware" {
                *dependency = "renamed.firmware-link".into();
            }
        }
        operation.update_fingerprint().unwrap();
    }
    execute(&plan);
    let final_elf = destination.join("producer-consumer.elf");
    let final_report = destination.join("producer-consumer.json");
    let old_elf = fs::read(&final_elf).unwrap();
    let old_report = fs::read(&final_report).unwrap();
    let link = plan
        .operations
        .iter_mut()
        .find(|operation| operation.id == "renamed.firmware-link")
        .unwrap();
    let internal = link.outputs.clone();
    if let BuildAction::Command { command, .. } = &mut link.action {
        command.executable = "/bin/true".into();
    }
    link.update_fingerprint().unwrap();
    let error = execution::execute_build_plan_with_jobs(&plan, false, true, 3, |_| {}).unwrap_err();
    assert!(error.to_string().contains("succeeded without producing"));
    assert!(internal.iter().all(|path| path.is_file()));
    assert_eq!(fs::read(&final_elf).unwrap(), old_elf);
    assert_eq!(fs::read(&final_report).unwrap(), old_report);
    fixture.write_file(
        "false-link.sh",
        &render::render_shell_script(&plan).unwrap(),
    );
    let output = Command::new("sh")
        .arg(fixture.directory.join("false-link.sh"))
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("succeeded without producing"));
    assert!(internal.iter().all(|path| path.is_file()));
    assert_eq!(fs::read(final_elf).unwrap(), old_elf);
    assert_eq!(fs::read(final_report).unwrap(), old_report);
}
