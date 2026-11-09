// Copyright 3Bian Limited and Contributors
// SPDX-License-Identifier: MIT

#![cfg(unix)]

#[cfg(feature = "host-tool-verification")]
use std::os::unix::fs::PermissionsExt;

#[cfg(feature = "host-tool-verification")]
use cheriot_compose::execution;
use cheriot_compose::planning::runner;
use test_fixture::TestFixture;

#[path = "support/test_fixture.rs"]
mod test_fixture;

#[test]
#[cfg(feature = "host-tool-verification")]
fn runner_preserves_arguments_environment_working_directory_and_failure() {
    let fixture = TestFixture::new();
    fixture.write_file(
        "runner script.sh",
        "#!/bin/sh\nprintf '%s\\n' \"$PWD\" \"$TEST_MODE\" \"$1\" \"$2\" > \"$RECORD_PATH\"\nexit \"$RUNNER_STATUS\"\n",
    );
    std::fs::set_permissions(
        fixture.directory.join("runner script.sh"),
        std::fs::Permissions::from_mode(0o755),
    )
    .unwrap();
    fixture.replace_text(
        "boards/sail.toml",
        "runner = { executable = \"cheriot_sim\", failure_patterns = [\"Assertion failure\"] }",
        &format!(
            "runner = {{ executable = \"../runner script.sh\", arguments = [\"ibex safe\"], environment = {{ TEST_MODE = \"chosen\", RUNNER_STATUS = \"0\", RECORD_PATH = \"{}\" }} }}",
            fixture.directory.join("record.txt").display()
        ),
    );
    let plan = fixture.build_plan();
    let project = fixture.resolved_project();
    let mut run = runner::create_runner_plan(&project, &plan, None).unwrap();
    assert_eq!(run.command.argv[0].value, "ibex safe");
    assert!(run.command.argv[0].origins[0].file.ends_with("sail.toml"));
    assert!(
        execution::execute_runner(&run)
            .unwrap_err()
            .to_string()
            .contains("firmware does not exist")
    );
    std::fs::create_dir_all(&run.command.cwd).unwrap();
    std::fs::write(&run.firmware, "fixture").unwrap();
    execution::execute_runner(&run).unwrap();
    let record = std::fs::read_to_string(fixture.directory.join("record.txt")).unwrap();
    assert_eq!(
        record.lines().collect::<Vec<_>>(),
        [
            run.command.cwd.to_str().unwrap(),
            "chosen",
            "ibex safe",
            run.firmware.to_str().unwrap(),
        ]
    );
    run.command
        .environment
        .insert("RUNNER_STATUS".into(), "7".into());
    assert!(
        execution::execute_runner(&run)
            .unwrap_err()
            .to_string()
            .contains("exit status: 7")
    );
}

#[test]
fn run_dry_run_describes_the_build_and_runner_without_running_them() {
    let fixture = TestFixture::new();
    fixture.build_plan();
    let toolchain_directory = fixture.directory.join("custom toolchain");
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_cheriot-compose"))
        .args(["run", "--dry-run", "--verbose", "--toolchain-dir"])
        .arg(&toolchain_directory)
        .arg("-f")
        .arg(fixture.project_file())
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stdout.is_empty());
    let text = String::from_utf8(output.stderr).unwrap();
    assert!(text.contains("link.firmware"));
    assert!(text.contains("Would run:"));
    assert!(text.contains("cheriot_sim"));
    assert!(text.contains(toolchain_directory.to_str().unwrap()));
    assert!(!fixture.directory.join(".cheriot-compose").exists());
}

#[test]
fn vm_runner_requires_an_explicit_profile_and_keeps_it_separate_from_the_board() {
    let fixture = TestFixture::new();
    fixture.replace_text(
        "boards/sail.toml",
        "runner = { executable = \"cheriot_sim\", failure_patterns = [\"Assertion failure\"] }",
        "runner = { kind = \"cheriot_vm\", executable = \"cheriot-vm\" }",
    );
    fixture.write_file("vm-profile.toml", "[machine]\nname = \"test\"\n");
    let plan = fixture.build_plan();
    let project = fixture.resolved_project();
    let error = runner::create_runner_plan(&project, &plan, None).unwrap_err();
    assert!(error.to_string().contains("--vm-profile"));
    let profile = fixture.directory.join("vm-profile.toml");
    let run = runner::create_runner_plan(&project, &plan, Some(&profile)).unwrap();
    let arguments: Vec<_> = run
        .command
        .argv
        .iter()
        .map(|argument| argument.value.as_str())
        .collect();
    assert_eq!(arguments[0], "--profile");
    assert_eq!(arguments[1], profile.to_str().unwrap());
    assert_eq!(arguments[2], "--elf");
    assert_eq!(arguments[3], run.firmware.to_str().unwrap());
}

#[test]
#[cfg(feature = "host-tool-verification")]
fn runner_reports_a_configured_failure_marker_even_when_process_exits_zero() {
    let fixture = TestFixture::new();
    fixture.write_file(
        "failure-runner",
        "#!/bin/sh\nprintf 'Assertion '\nsleep 0.1\nprintf 'failure\\n'\nexit 0\n",
    );
    let executable = fixture.directory.join("failure-runner");
    std::fs::set_permissions(&executable, std::fs::Permissions::from_mode(0o755)).unwrap();
    fixture.replace_text(
        "boards/sail.toml",
        "runner = { executable = \"cheriot_sim\", failure_patterns = [\"Assertion failure\"] }",
        &format!(
            "runner = {{ executable = {:?}, failure_patterns = [\"Assertion failure\"] }}",
            executable.to_str().unwrap()
        ),
    );
    let plan = fixture.build_plan();
    let project = fixture.resolved_project();
    let run = runner::create_runner_plan(&project, &plan, None).unwrap();
    std::fs::create_dir_all(&run.command.cwd).unwrap();
    std::fs::create_dir_all(run.firmware.parent().unwrap()).unwrap();
    std::fs::write(&run.firmware, "fixture").unwrap();
    let error = execution::execute_runner(&run).unwrap_err();
    assert!(error.to_string().contains("failure marker"));
}

#[test]
fn sonata_sdk_runner_keeps_its_packaging_files_in_the_intermediate_directory() {
    let fixture = TestFixture::new();
    fixture.write_file("run-sonata-1.0.sh", "#!/bin/sh\nexit 0\n");
    fixture.replace_text(
        "boards/sail.toml",
        "name = \"sail\"",
        "name = \"sail\"\npackaging = \"sonata_v1\"",
    );
    fixture.replace_text(
        "boards/sail.toml",
        "simulation = true",
        "simulation = false",
    );
    fixture.replace_text(
        "boards/sail.toml",
        "runner = { executable = \"cheriot_sim\", failure_patterns = [\"Assertion failure\"] }",
        "runner = { executable = \"../run-sonata-1.0.sh\" }",
    );
    fixture.build_plan();
    let project = fixture.resolved_project();
    let plan = cheriot_compose::planning::create_packaging_plan(&project).unwrap();
    let run = runner::create_runner_plan(&project, &plan, None).unwrap();
    assert_eq!(
        run.firmware,
        project.output_dir.join("producer-consumer.elf")
    );
    assert_eq!(run.command.cwd.parent(), Some(project.build_dir.as_path()));
    assert!(
        run.command
            .argv
            .last()
            .unwrap()
            .value
            .starts_with("../producer-consumer--")
    );
    let copy = plan
        .operations
        .iter()
        .find(|operation| operation.id == "package.runner-image")
        .unwrap();
    assert_eq!(copy.outputs[0], run.command.cwd.join("firmware.uf2"));
    assert!(copy.dependencies.contains(&"package.slot1".to_owned()));
}
