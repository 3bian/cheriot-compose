// Copyright 3Bian Limited and Contributors
// SPDX-License-Identifier: MIT

use std::process::Command;

use test_fixture::TestFixture;

#[cfg(all(unix, feature = "host-tool-verification"))]
#[path = "cli/execution.rs"]
mod execution;

#[path = "support/test_fixture.rs"]
mod test_fixture;
fn run(fixture: &TestFixture, arguments: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_cheriot-compose"))
        .args(arguments)
        .arg("-f")
        .arg(fixture.project_file())
        .output()
        .unwrap()
}

#[test]
fn tool_free_check_and_exports() {
    let fixture = TestFixture::new();
    fixture.set_toolchain_directory(&fixture.directory.join("missing-tools"));
    assert!(run(&fixture, &["check"]).status.success());
    assert!(
        run(
            &fixture,
            &["configuration", "--resolved", "--format", "toml"]
        )
        .status
        .success()
    );
    fixture.build_plan();
    for format in ["json", "shell", "compilation-database"] {
        let result = run(&fixture, &["plan", "--format", format]);
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        if format != "shell" {
            let _: serde_json::Value = serde_json::from_slice(&result.stdout).unwrap();
        }
    }
    let result = run(&fixture, &["build", "--dry-run"]);
    assert!(result.status.success());
    assert!(!fixture.directory.join(".cheriot-compose").exists());
    let result = run(&fixture, &["plan", "--probe-toolchain"]);
    assert!(!result.status.success());
    assert!(String::from_utf8_lossy(&result.stderr).contains("missing or non-executable tool"));
}

#[test]
fn argument_errors_are_not_ignored() {
    let fixture = TestFixture::new();
    for args in [
        vec!["check", "--dry-run"],
        vec!["plan", "--wat"],
        vec!["check", "--against", "x"],
        vec!["build", "--format", "toml"],
        vec!["build", "--jobs", "0"],
        vec!["build", "--jobs", "257"],
        vec!["build", "--jobs", "many"],
        vec!["unknown"],
    ] {
        assert!(!run(&fixture, &args).status.success());
    }
    let overridden = Command::new(env!("CARGO_BIN_EXE_cheriot-compose"))
        .args(["build", "--dry-run", "--jobs", "2", "--file"])
        .arg(fixture.project_file())
        .env("CHERIOT_COMPOSE_JOBS", "invalid")
        .output()
        .unwrap();
    assert!(overridden.status.success());
    let invalid_environment = Command::new(env!("CARGO_BIN_EXE_cheriot-compose"))
        .args(["build", "--dry-run", "--file"])
        .arg(fixture.project_file())
        .env("CHERIOT_COMPOSE_JOBS", "invalid")
        .output()
        .unwrap();
    assert!(!invalid_environment.status.success());
    assert!(String::from_utf8_lossy(&invalid_environment.stderr).contains("jobs must be"));
}

#[test]
fn command_string_database_is_read_without_execution() {
    let fixture = TestFixture::new();
    let db =
        r#"[{"directory":"/tmp","file":"a.cc","command":"'cc path' -D'VALUE=a b' -o a.o a.cc"}]"#;
    fixture.write_file("old.json", db);
    fixture.write_file("new.json", db);
    fixture.write_file("cheriot-compose.toml", "this is not valid TOML =\n");
    let output = Command::new(env!("CARGO_BIN_EXE_cheriot-compose"))
        .current_dir(&fixture.directory)
        .env("CC", "")
        .args(["compare-compilation-database", "--against"])
        .arg(fixture.directory.join("old.json"))
        .arg("--generated")
        .arg(fixture.directory.join("new.json"))
        .output()
        .unwrap();
    assert!(output.status.success());
    assert!(String::from_utf8_lossy(&output.stdout).contains("different: 0"));
}

#[test]
fn firmware_selection_is_explicit_for_multi_image_projects() {
    let fixture = TestFixture::new();
    let mut document = std::fs::read_to_string(fixture.project_file()).unwrap();
    document.push_str("\n[[firmwares]]\nname = \"second\"\nservices = [\"producer\"]\nthreads = [{ name = \"run\", compartment = \"producer\", entry = \"run\", priority = 1, stack_size = 512, trusted_stack_frames = 1 }]\n");
    fixture.write_file("cheriot-compose.toml", &document);
    let missing = run(&fixture, &["check"]);
    assert!(!missing.status.success());
    assert!(String::from_utf8_lossy(&missing.stderr).contains("--firmware"));
    let selected = run(&fixture, &["check", "--firmware", "second"]);
    assert!(
        selected.status.success(),
        "{}",
        String::from_utf8_lossy(&selected.stderr)
    );
    assert!(String::from_utf8_lossy(&selected.stdout).contains("1 threads"));
}

#[test]
fn build_json_reports_planned_actions_and_failures() {
    let fixture = TestFixture::new();
    fixture.set_toolchain_directory(&fixture.directory.join("missing-tools"));
    fixture.build_plan();
    let planned = run(&fixture, &["build", "--dry-run", "--format", "json"]);
    assert!(planned.status.success());
    let report: serde_json::Value = serde_json::from_slice(&planned.stdout).unwrap();
    let events = report["events"].as_array().unwrap();
    assert!(events.iter().any(|event| event["event"] == "planned"));
    assert_eq!(events.last().unwrap()["success"], true);
    let failed = run(&fixture, &["build", "--format", "json"]);
    assert!(!failed.status.success());
    let report: serde_json::Value = serde_json::from_slice(&failed.stdout).unwrap();
    assert!(
        report["error"]
            .as_str()
            .unwrap()
            .contains("missing or non-executable tool")
    );
    let events = report["events"].as_array().unwrap();
    assert!(events.iter().any(|event| event["event"] == "failed"));
    assert_eq!(events.last().unwrap()["success"], false);
}

#[test]
fn explain_shows_rebuild_reason_and_argument_sources_without_running_tools() {
    let fixture = TestFixture::new();
    fixture.build_plan();
    let result = run(&fixture, &["explain", "compile.producer.0"]);
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let explanation: serde_json::Value = serde_json::from_slice(&result.stdout).unwrap();
    assert_eq!(explanation["state"]["status"], "will run");
    assert!(
        explanation["state"]["reason"]
            .as_str()
            .unwrap()
            .contains("no saved successful build")
    );
    assert!(
        explanation["operation"]["action"]["command"]["argv"]
            .as_array()
            .unwrap()
            .iter()
            .all(|argument| argument["origins"].is_array())
    );
    assert!(!fixture.directory.join(".cheriot-compose").exists());
}

#[test]
fn command_must_come_first_and_help_is_command_specific() {
    let executable = env!("CARGO_BIN_EXE_cheriot-compose");
    let misplaced = Command::new(executable)
        .args(["--file", "project.toml", "build"])
        .output()
        .unwrap();
    assert!(!misplaced.status.success());
    assert!(String::from_utf8_lossy(&misplaced.stderr).contains("command first"));
    for option in ["--help", "-h"] {
        let help = Command::new(executable).arg(option).output().unwrap();
        assert!(help.status.success());
        let text = String::from_utf8_lossy(&help.stdout);
        assert!(text.contains("Usage: cheriot-compose"));
        for directory_option in ["--build-dir", "--output-dir", "--toolchain-dir"] {
            assert!(text.contains(directory_option), "{directory_option}");
        }
    }
    let help = Command::new(executable)
        .args(["help", "build"])
        .output()
        .unwrap();
    assert!(help.status.success());
    assert!(String::from_utf8_lossy(&help.stdout).contains("cheriot-compose build"));
    assert!(
        Command::new(executable)
            .arg("version")
            .status()
            .unwrap()
            .success()
    );
    let repeated = Command::new(executable)
        .args(["check", "--file", "one.toml", "--file", "two.toml"])
        .output()
        .unwrap();
    assert!(!repeated.status.success());
    assert!(String::from_utf8_lossy(&repeated.stderr).contains("--file once"));
    let repeated = Command::new(executable)
        .args(["build", "--toolchain-dir", "one", "--toolchain-dir", "two"])
        .output()
        .unwrap();
    assert!(!repeated.status.success());
    assert!(
        String::from_utf8_lossy(&repeated.stderr)
            .contains("--toolchain-dir may only be supplied once")
    );
}

#[test]
fn files_overlays_and_environment_have_explicit_precedence() {
    let fixture = TestFixture::new();
    fixture.write_file("settings/base.toml", "patches = [{ op = \"replace\", path = \"/mode\", value = \"debug\" }, { op = \"add\", path = \"/services/producer/build/c_flags\", value = [\"-DFIRST\"] }]\n");
    fixture.write_file("settings/local.toml", "include = [\"base.toml\"]\npatches = [{ op = \"replace\", path = \"/mode\", value = \"release\" }, { op = \"add\", path = \"/services/producer/build/c_flags/-\", value = \"-DSECOND\" }]\n");
    let overlay = fixture.directory.join("settings/local.toml");
    let output = Command::new(env!("CARGO_BIN_EXE_cheriot-compose"))
        .args(["configuration", "--resolved", "--format", "json"])
        .arg("--overlay")
        .arg(&overlay)
        .env("CHERIOT_COMPOSE_FILE", fixture.project_file())
        .env("CHERIOT_COMPOSE_MODE", "debug")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let document: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(document["mode"], "debug");
    assert_eq!(
        document["provenance"]["/mode"]["rule"],
        "environment variable CHERIOT_COMPOSE_MODE"
    );
    let flags = document["targets"]["producer"]["c_flags"]
        .as_array()
        .unwrap();
    assert_eq!(
        flags
            .iter()
            .map(|flag| flag["value"].as_str().unwrap())
            .collect::<Vec<_>>(),
        ["-DFIRST", "-DSECOND"]
    );
    assert!(
        flags[1]["origin"]["file"]
            .as_str()
            .unwrap()
            .ends_with("settings/local.toml")
    );
    let cli = Command::new(env!("CARGO_BIN_EXE_cheriot-compose"))
        .args([
            "configuration",
            "--resolved",
            "--format",
            "json",
            "--mode",
            "release",
        ])
        .arg("--file")
        .arg(fixture.project_file())
        .env("CHERIOT_COMPOSE_FILE", "/does/not/exist.toml")
        .env("CHERIOT_COMPOSE_MODE", "debug")
        .output()
        .unwrap();
    assert!(
        cli.status.success(),
        "{}",
        String::from_utf8_lossy(&cli.stderr)
    );
    let document: serde_json::Value = serde_json::from_slice(&cli.stdout).unwrap();
    assert_eq!(document["mode"], "release");
    assert_eq!(
        document["provenance"]["/mode"]["rule"],
        "command-line option --mode"
    );
    fixture.write_file("settings/cycle.toml", "include = [\"cycle.toml\"]\n");
    let cycle = run(
        &fixture,
        &[
            "check",
            "--overlay",
            fixture
                .directory
                .join("settings/cycle.toml")
                .to_str()
                .unwrap(),
        ],
    );
    assert!(!cycle.status.success());
    assert!(String::from_utf8_lossy(&cycle.stderr).contains("overlay include cycle"));
}

#[test]
fn explicit_environment_references_are_checked_without_shell_expansion() {
    let fixture = TestFixture::new();
    fixture.replace_text(
        "cheriot-compose.toml",
        "mode = \"release\"",
        "mode = \"${env:TEST_CHERIOT_MODE}\"",
    );
    let output = Command::new(env!("CARGO_BIN_EXE_cheriot-compose"))
        .args(["configuration", "--resolved", "--format", "json", "--file"])
        .arg(fixture.project_file())
        .env("TEST_CHERIOT_MODE", "debug")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let document: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(document["mode"], "debug");
    assert_eq!(
        document["provenance"]["/mode"]["rule"],
        "environment variable TEST_CHERIOT_MODE"
    );
    let missing = Command::new(env!("CARGO_BIN_EXE_cheriot-compose"))
        .args(["check", "--file"])
        .arg(fixture.project_file())
        .env_remove("TEST_CHERIOT_MODE")
        .output()
        .unwrap();
    assert!(!missing.status.success());
    assert!(String::from_utf8_lossy(&missing.stderr).contains("TEST_CHERIOT_MODE"));
    fixture.replace_text(
        "cheriot-compose.toml",
        "name = \"producer-consumer\"",
        "name = \"${env:TEST_NAME}\"",
    );
    let disallowed = Command::new(env!("CARGO_BIN_EXE_cheriot-compose"))
        .args(["check", "--file"])
        .arg(fixture.project_file())
        .env("TEST_CHERIOT_MODE", "debug")
        .env("TEST_NAME", "anything")
        .output()
        .unwrap();
    assert!(!disallowed.status.success());
    assert!(String::from_utf8_lossy(&disallowed.stderr).contains("not supported in this field"));
}

#[test]
fn help_matches_command_options_and_unrelated_options_are_rejected() {
    let executable = env!("CARGO_BIN_EXE_cheriot-compose");
    for (command, present, absent) in [
        (
            "check",
            vec!["--file", "--toolchain-dir"],
            vec!["--jobs", "--query", "--input"],
        ),
        (
            "plan",
            vec!["--format", "--probe-toolchain"],
            vec!["--jobs", "--query"],
        ),
        (
            "build",
            vec!["--jobs", "--dry-run", "--toolchain-dir"],
            vec!["--query", "--input"],
        ),
        (
            "run",
            vec!["--vm-profile", "--jobs"],
            vec!["--format", "--output FILE"],
        ),
        (
            "audit",
            vec!["--query", "--module"],
            vec!["--vm-profile", "--format"],
        ),
        (
            "analyze-switcher",
            vec!["--input", "--output"],
            vec!["--file", "--toolchain-dir"],
        ),
        (
            "compare-compilation-database",
            vec!["--against", "--generated", "--map-path"],
            vec!["--jobs", "--output-dir"],
        ),
    ] {
        let help = Command::new(executable)
            .args(["help", command])
            .output()
            .unwrap();
        let after_command = Command::new(executable)
            .args([command, "--help"])
            .output()
            .unwrap();
        assert!(help.status.success());
        assert_eq!(help.stdout, after_command.stdout);
        let text = String::from_utf8(help.stdout).unwrap();
        for option in present {
            assert!(text.contains(option), "{command} help lacks {option}");
        }
        for option in absent {
            assert!(!text.contains(option), "{command} help includes {option}");
        }
    }
    for args in [
        vec!["check", "--jobs", "2"],
        vec!["check", "--verbose"],
        vec!["plan", "--jobs", "2"],
        vec!["run", "--format", "json"],
        vec!["analyze-switcher", "--toolchain-dir", "/tools"],
        vec![
            "compare-compilation-database",
            "--generated",
            "new.json",
            "--file",
            "project.toml",
        ],
        vec!["build", "--format", "json", "--verbose"],
        vec!["build", "--format", "json", "--quiet"],
        vec!["build", "--output", "report.json"],
    ] {
        let result = Command::new(executable).args(&args).output().unwrap();
        assert!(!result.status.success(), "accepted {args:?}");
    }
}

#[test]
fn standalone_help_and_version_ignore_project_and_toolchain_environment() {
    let fixture = TestFixture::new();
    for args in [
        vec!["help"],
        vec!["help", "build"],
        vec!["build", "--help"],
        vec!["version"],
    ] {
        let output = Command::new(env!("CARGO_BIN_EXE_cheriot-compose"))
            .current_dir(&fixture.directory)
            .args(args)
            .env("CHERIOT_COMPOSE_FILE", "")
            .env("CHERIOT_COMPOSE_TOOLCHAIN_DIR", "")
            .env("CHERIOT_COMPOSE_JOBS", "invalid")
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    let check = Command::new(env!("CARGO_BIN_EXE_cheriot-compose"))
        .current_dir(&fixture.directory)
        .args(["check"])
        .env("CHERIOT_COMPOSE_JOBS", "invalid")
        .output()
        .unwrap();
    assert!(
        check.status.success(),
        "{}",
        String::from_utf8_lossy(&check.stderr)
    );
}

#[test]
fn default_project_lookup_does_not_search_parent_directories() {
    let fixture = TestFixture::new();
    let child = fixture.directory.join("child");
    std::fs::create_dir(&child).unwrap();
    let invoke = |arguments: &[&str]| {
        Command::new(env!("CARGO_BIN_EXE_cheriot-compose"))
            .current_dir(&child)
            .env_remove("CHERIOT_COMPOSE_FILE")
            .args(arguments)
            .output()
            .unwrap()
    };
    assert!(!invoke(&["check"]).status.success());
    let explicit = invoke(&["check", "--file", "../cheriot-compose.toml"]);
    assert!(
        explicit.status.success(),
        "{}",
        String::from_utf8_lossy(&explicit.stderr)
    );
}

#[test]
fn command_help_and_parser_agree_on_formats_and_defaults() {
    let fixture = TestFixture::new();
    let binary = env!("CARGO_BIN_EXE_cheriot-compose");
    for (command, accepted, default) in [
        ("configuration", vec!["toml", "json"], "toml"),
        (
            "plan",
            vec!["text", "json", "shell", "compilation-database"],
            "text",
        ),
        ("build", vec!["json"], "human"),
        ("package", vec!["json"], "human"),
        ("size", vec!["text", "json", "bytes"], "text"),
        ("compare-compilation-database", vec!["text", "json"], "text"),
    ] {
        let help = Command::new(binary)
            .args(["help", command])
            .output()
            .unwrap();
        assert!(help.status.success());
        let text = String::from_utf8(help.stdout).unwrap();
        assert!(
            text.contains(&format!("default: {default}")),
            "{command}: {text}"
        );
        for format in accepted {
            assert!(text.contains(format), "{command} help omitted {format}");
            let output = Command::new(binary)
                .args([command, "--format", format, "--file"])
                .arg(fixture.project_file())
                .output()
                .unwrap();
            assert!(
                !String::from_utf8_lossy(&output.stderr).contains("invalid format"),
                "{command} rejected {format}"
            );
        }
        let output = Command::new(binary)
            .args([command, "--format", "unsupported"])
            .output()
            .unwrap();
        assert!(!output.status.success());
        assert!(String::from_utf8_lossy(&output.stderr).contains("invalid format"));
    }
}
