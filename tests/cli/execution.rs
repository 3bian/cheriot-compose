// Copyright 3Bian Limited and Contributors
// SPDX-License-Identifier: MIT

use std::process::Command;

use super::{TestFixture, run};

fn install_fake_toolchain(fixture: &TestFixture) {
    use std::os::unix::fs::PermissionsExt;
    fixture.build_plan();
    fixture.write_file(
        "fake-tool",
        r#"#!/bin/sh
set -eu
if [ -n "${SLOW_MARKER-}" ] && [ ! -e "$SLOW_MARKER" ]; then : > "$SLOW_MARKER"; sleep 1; fi
printf '%s\n' 'tool standard output'
if [ -n "${FAIL_TOOL-}" ]; then printf '%s\n' 'compiler diagnostic' >&2; exit 7; fi
output=''
depfile=''
report=''
while [ "$#" -gt 0 ]; do
  case "$1" in
    -o) shift; output=$1 ;;
    -MF) shift; depfile=$1 ;;
    --compartment-report=*) report=${1#*=} ;;
  esac
  shift
done
if [ -n "$output" ]; then printf 'artifact\n' > "$output"; fi
if [ -n "$depfile" ]; then printf 'dummy: producer.cc\n' > "$depfile"; fi
if [ -n "$report" ]; then printf '{}\n' > "$report"; fi
"#,
    );
    let tool = fixture.directory.join("fake-tool");
    std::fs::set_permissions(&tool, std::fs::Permissions::from_mode(0o755)).unwrap();
    fixture.set_fake_llvm(&tool);
}

#[test]
fn build_progress_is_live_and_machine_output_stays_valid() {
    use std::io::{BufRead, BufReader, Read};
    use std::process::Stdio;
    let fixture = TestFixture::new();
    install_fake_toolchain(&fixture);
    let mut child = Command::new(env!("CARGO_BIN_EXE_cheriot-compose"))
        .args(["build", "--file"])
        .arg(fixture.project_file())
        .env("SLOW_MARKER", fixture.directory.join("slow-marker"))
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let mut stderr = BufReader::new(child.stderr.take().unwrap());
    let mut first_line = String::new();
    stderr.read_line(&mut first_line).unwrap();
    assert!(first_line.contains("Building producer-consumer"));
    assert!(
        child.try_wait().unwrap().is_none(),
        "progress should appear before the build completes"
    );
    let mut remainder = String::new();
    stderr.read_to_string(&mut remainder).unwrap();
    assert!(child.wait().unwrap().success(), "{remainder}");
    assert!(remainder.contains("Compiling sources"));
    assert!(remainder.contains("Build succeeded"));
    let no_op = run(&fixture, &["build"]);
    assert!(no_op.status.success());
    assert!(String::from_utf8_lossy(&no_op.stderr).contains("up to date"));
    let quiet = run(&fixture, &["build", "--quiet"]);
    assert!(quiet.status.success());
    assert!(quiet.stderr.is_empty());
    let verbose = run(
        &fixture,
        &[
            "build",
            "--verbose",
            "--mode",
            "debug",
            "--output-dir",
            fixture.directory.join("debug-output").to_str().unwrap(),
        ],
    );
    assert!(
        verbose.status.success(),
        "{}",
        String::from_utf8_lossy(&verbose.stderr)
    );
    assert!(String::from_utf8_lossy(&verbose.stderr).contains("env --"));
    let json = run(
        &fixture,
        &[
            "build",
            "--format",
            "json",
            "--build-dir",
            fixture.directory.join("json-build").to_str().unwrap(),
        ],
    );
    assert!(
        json.status.success(),
        "{}",
        String::from_utf8_lossy(&json.stderr)
    );
    let report: serde_json::Value = serde_json::from_slice(&json.stdout).unwrap();
    assert_eq!(
        report["events"].as_array().unwrap().last().unwrap()["success"],
        true
    );
    assert!(String::from_utf8_lossy(&json.stderr).contains("tool standard output"));
    let parallel_json = run(
        &fixture,
        &[
            "build",
            "--jobs",
            "3",
            "--format",
            "json",
            "--build-dir",
            fixture
                .directory
                .join("parallel-json-build")
                .to_str()
                .unwrap(),
        ],
    );
    assert!(
        parallel_json.status.success(),
        "{}",
        String::from_utf8_lossy(&parallel_json.stderr)
    );
    let report: serde_json::Value = serde_json::from_slice(&parallel_json.stdout).unwrap();
    assert_eq!(
        report["events"].as_array().unwrap().last().unwrap()["success"],
        true
    );
    assert!(String::from_utf8_lossy(&parallel_json.stderr).contains("tool standard output"));
    let failed = Command::new(env!("CARGO_BIN_EXE_cheriot-compose"))
        .args(["build", "--file"])
        .arg(fixture.project_file())
        .arg("--build-dir")
        .arg(fixture.directory.join("failing-build"))
        .env("FAIL_TOOL", "1")
        .output()
        .unwrap();
    assert!(!failed.status.success());
    let diagnostics = String::from_utf8_lossy(&failed.stderr);
    assert!(diagnostics.contains("compiler diagnostic"));
    assert!(diagnostics.contains("Build failed"));
}

#[test]
fn size_reports_sdk_sections_without_mixing_build_output_into_json() {
    let fixture = TestFixture::new();
    install_fake_toolchain(&fixture);
    let path = fixture.directory.join("fake-tool");
    let script = std::fs::read_to_string(&path).unwrap().replacen(
        "#!/bin/sh\n",
        "#!/bin/sh\nif [ \"${1-}\" = --headers ]; then\n  printf ' 1 .producer_code 00000020 80000000 TEXT\\n 2 .producer_globals 00000004 80000020 DATA\\n 3 __cap_relocs 00000008 80000024 DATA\\n'\n  exit 0\nfi\n",
        1,
    );
    std::fs::write(&path, script).unwrap();
    fixture.set_fake_llvm(&path);
    let compartment = run(
        &fixture,
        &[
            "size",
            "--compartment",
            "producer",
            "--format",
            "json",
            "--jobs",
            "2",
        ],
    );
    assert!(
        compartment.status.success(),
        "{}",
        String::from_utf8_lossy(&compartment.stderr)
    );
    let report: serde_json::Value = serde_json::from_slice(&compartment.stdout).unwrap();
    assert_eq!(report["bytes"], 36);
    assert_eq!(report["compartment"], "producer");
    let full = run(&fixture, &["size", "--format", "bytes"]);
    assert!(full.status.success());
    assert_eq!(String::from_utf8_lossy(&full.stdout).trim(), "8");
    assert!(String::from_utf8_lossy(&full.stderr).contains("not a full firmware size"));
}

#[test]
fn run_executes_every_time_and_keeps_program_stdout_separate() {
    use std::os::unix::fs::PermissionsExt;
    let fixture = TestFixture::new();
    install_fake_toolchain(&fixture);
    fixture.write_file(
        "fake-runner",
        "#!/bin/sh\nprintf '%s\\n' \"$@\" >> \"$RUN_RECORD\"\nprintf '%s\\n' 'firmware output'\n",
    );
    let runner = fixture.directory.join("fake-runner");
    std::fs::set_permissions(&runner, std::fs::Permissions::from_mode(0o755)).unwrap();
    fixture.replace_text(
        "boards/sail.toml",
        "runner = { executable = \"cheriot_sim\", failure_patterns = [\"Assertion failure\"] }",
        &format!(
            "runner = {{ executable = {:?}, arguments = [\"initial\"] }}",
            runner.to_str().unwrap()
        ),
    );
    for _ in 0..2 {
        let output = Command::new(env!("CARGO_BIN_EXE_cheriot-compose"))
            .args(["run", "--file"])
            .arg(fixture.project_file())
            .args(["--", "--extra", "value with spaces"])
            .env("RUN_RECORD", fixture.directory.join("runs.log"))
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(String::from_utf8_lossy(&output.stdout), "firmware output\n");
    }
    let record = std::fs::read_to_string(fixture.directory.join("runs.log")).unwrap();
    let arguments: Vec<_> = record.lines().collect();
    let firmware = fixture.directory.join("producer-consumer.elf");
    let firmware = firmware.to_str().unwrap();
    assert_eq!(
        arguments,
        [
            "initial",
            firmware,
            "--extra",
            "value with spaces",
            "initial",
            firmware,
            "--extra",
            "value with spaces"
        ]
    );
}

#[test]
fn audit_is_separate_from_build_and_passes_query_as_data() {
    use std::os::unix::fs::PermissionsExt;
    let fixture = TestFixture::new();
    install_fake_toolchain(&fixture);
    fixture.write_file("policy.rego", "package fixture\n");
    fixture.write_file("fake-audit", "#!/bin/sh\nprintf '%s\\n' \"$@\" > \"$AUDIT_ARGUMENTS\"\nprintf '%s\\n' '{\"allowed\":true}'\n");
    let audit = fixture.directory.join("fake-audit");
    std::fs::set_permissions(&audit, std::fs::Permissions::from_mode(0o755)).unwrap();
    let output = fixture.directory.join("audit-result.json");
    let query = "input.value;$(touch SHOULD_NOT_EXIST)";
    let result = Command::new(env!("CARGO_BIN_EXE_cheriot-compose"))
        .args(["audit", "--file"])
        .arg(fixture.project_file())
        .arg("--audit-tool")
        .arg(&audit)
        .arg("--module")
        .arg(fixture.directory.join("policy.rego"))
        .arg("--query")
        .arg(query)
        .arg("--output")
        .arg(&output)
        .env(
            "AUDIT_ARGUMENTS",
            fixture.directory.join("audit-arguments.txt"),
        )
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert!(result.stdout.is_empty());
    let report: serde_json::Value =
        serde_json::from_slice(&std::fs::read(output).unwrap()).unwrap();
    assert_eq!(report["allowed"], true);
    let recorded = std::fs::read_to_string(fixture.directory.join("audit-arguments.txt")).unwrap();
    assert!(recorded.lines().any(|value| value == query));
    assert!(recorded.contains("board.json"));
    assert!(!fixture.directory.join("SHOULD_NOT_EXIST").exists());
}
