// Copyright 3Bian Limited and Contributors
// SPDX-License-Identifier: MIT

use std::{
    fs,
    path::Path,
    process::{Command, Output},
};

#[cfg(all(unix, feature = "host-tool-verification"))]
use cheriot_compose::execution::{self, ExecutionEvent};
use cheriot_compose::planning;
use test_fixture::TestFixture;

#[path = "support/test_fixture.rs"]
mod test_fixture;

fn command(fixture: &TestFixture, directory: &Path, arguments: &[&str]) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_cheriot-compose"));
    command
        .current_dir(directory)
        .args(arguments)
        .arg("--file")
        .arg(fixture.project_file());
    for variable in [
        "CHERIOT_COMPOSE_TOOLCHAIN_DIR",
        "CHERIOT_COMPOSE_FILE",
        "RUSTC",
    ] {
        command.env_remove(variable);
    }
    command
}

fn resolved(output: Output) -> serde_json::Value {
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).unwrap()
}

#[test]
fn default_toolchain_is_available_offline_without_a_settings_file() {
    let fixture = TestFixture::new();
    let project = fixture.resolved_project();
    assert_eq!(project.toolchain.directory, Path::new("/cheriot-tools/bin"));
    assert_eq!(project.toolchain.cpp_compiler, "/cheriot-tools/bin/clang++");
    assert!(project.toolchain.directory_origin.rule.contains("built-in"));
    assert!(
        !fixture
            .directory
            .join("cheriot-compose.toolchain.toml")
            .exists()
    );
    let plan = planning::create_build_plan(&project).unwrap();
    assert!(plan.unresolved_probes().values().all(Option::is_none));
    let json = serde_json::to_value(&project).unwrap();
    assert!(json["toolchain"].get("settings_file").is_none());
}

#[test]
fn directory_precedence_and_relative_paths_follow_their_source() {
    let fixture = TestFixture::new();
    fixture.replace_text(
        "cheriot-compose.toml",
        "mode = \"release\"",
        "mode = \"release\"\ntoolchain_dir = \"./toml-tools/\"",
    );
    let invocation = fixture.directory.join("invocation");
    fs::create_dir(&invocation).unwrap();
    let options = ["configuration", "--resolved", "--format", "json"];
    let toml = resolved(command(&fixture, &invocation, &options).output().unwrap());
    assert_eq!(
        toml["toolchain"]["directory"],
        fixture.directory.join("toml-tools").to_str().unwrap()
    );
    assert!(
        toml["toolchain"]["directory_origin"]["file"]
            .as_str()
            .unwrap()
            .ends_with("cheriot-compose.toml")
    );
    let environment = resolved(
        command(&fixture, &invocation, &options)
            .env("CHERIOT_COMPOSE_TOOLCHAIN_DIR", "./environment-tools/")
            .output()
            .unwrap(),
    );
    assert_eq!(
        environment["toolchain"]["directory"],
        invocation.join("environment-tools").to_str().unwrap()
    );
    assert!(
        environment["toolchain"]["directory_origin"]["rule"]
            .as_str()
            .unwrap()
            .contains("CHERIOT_COMPOSE_TOOLCHAIN_DIR")
    );
    let cli = resolved(
        command(
            &fixture,
            &invocation,
            &[
                "configuration",
                "--resolved",
                "--format",
                "json",
                "--toolchain-dir",
                "./cli-tools/",
            ],
        )
        .env("CHERIOT_COMPOSE_TOOLCHAIN_DIR", "./environment-tools")
        .output()
        .unwrap(),
    );
    assert_eq!(
        cli["toolchain"]["directory"],
        invocation.join("cli-tools").to_str().unwrap()
    );
    assert_eq!(
        cli["toolchain"]["cpp_compiler"],
        invocation.join("cli-tools/clang++").to_str().unwrap()
    );
    for field in ["root", "output_dir", "build_dir"] {
        assert_eq!(toml[field], cli[field]);
    }
    let empty_env = command(&fixture, &invocation, &["check"])
        .env("CHERIOT_COMPOSE_TOOLCHAIN_DIR", "")
        .output()
        .unwrap();
    assert!(!empty_env.status.success());
    let overridden = command(
        &fixture,
        &invocation,
        &["check", "--toolchain-dir", "tools"],
    )
    .env("CHERIOT_COMPOSE_TOOLCHAIN_DIR", "")
    .output()
    .unwrap();
    assert!(
        overridden.status.success(),
        "{}",
        String::from_utf8_lossy(&overridden.stderr)
    );
}

#[test]
fn includes_and_overlays_keep_the_declaring_file_as_the_path_base() {
    let fixture = TestFixture::new();
    fixture.write_file(
        "settings/included.toml",
        "toolchain_dir = \"${env:TEST_TOOLCHAIN_PATH}\"\n",
    );
    fixture.replace_text(
        "cheriot-compose.toml",
        "include = [\"./sdk/cheriot-compose.toml\"]",
        "include = [\"./sdk/cheriot-compose.toml\", \"./settings/included.toml\"]",
    );
    let options = ["configuration", "--resolved", "--format", "json"];
    let included = resolved(
        command(&fixture, &fixture.directory, &options)
            .env("TEST_TOOLCHAIN_PATH", "./bin")
            .output()
            .unwrap(),
    );
    assert_eq!(
        included["toolchain"]["directory"],
        fixture.directory.join("settings/bin").to_str().unwrap()
    );
    assert!(
        included["toolchain"]["directory_origin"]["file"]
            .as_str()
            .unwrap()
            .ends_with("settings/included.toml")
    );
    fixture.write_file("settings/overrides/change.toml", "patches = [{ op = \"replace\", path = \"/toolchain_dir\", value = \"${env:TEST_TOOLCHAIN_PATH}\" }]\n");
    let overlay = resolved(
        command(
            &fixture,
            &fixture.directory,
            &[
                "configuration",
                "--resolved",
                "--format",
                "json",
                "--overlay",
                "settings/overrides/change.toml",
            ],
        )
        .env("TEST_TOOLCHAIN_PATH", "./bin")
        .output()
        .unwrap(),
    );
    assert_eq!(
        overlay["toolchain"]["directory"],
        fixture
            .directory
            .join("settings/overrides/bin")
            .to_str()
            .unwrap()
    );
    let missing = command(&fixture, &fixture.directory, &["check"])
        .env_remove("TEST_TOOLCHAIN_PATH")
        .output()
        .unwrap();
    assert!(!missing.status.success());
    assert!(String::from_utf8_lossy(&missing.stderr).contains("TEST_TOOLCHAIN_PATH"));
    let overridden = command(
        &fixture,
        &fixture.directory,
        &["check", "--toolchain-dir", "./selected-tools"],
    )
    .env_remove("TEST_TOOLCHAIN_PATH")
    .output()
    .unwrap();
    assert!(
        overridden.status.success(),
        "{}",
        String::from_utf8_lossy(&overridden.stderr)
    );
}

#[test]
fn conflicting_included_directories_require_an_explicit_project_choice() {
    let fixture = TestFixture::new();
    fixture.write_file("settings/one.toml", "toolchain_dir = \"./first\"\n");
    fixture.write_file("settings/two.toml", "toolchain_dir = \"./second\"\n");
    fixture.replace_text(
        "cheriot-compose.toml",
        "include = [\"./sdk/cheriot-compose.toml\"]",
        "include = [\"./sdk/cheriot-compose.toml\", \"./settings/one.toml\", \"./settings/two.toml\"]",
    );
    let conflicting = command(&fixture, &fixture.directory, &["check"])
        .output()
        .unwrap();
    assert!(!conflicting.status.success());
    assert!(
        String::from_utf8_lossy(&conflicting.stderr).contains("conflicting included toolchain_dir")
    );
    fixture.replace_text(
        "cheriot-compose.toml",
        "mode = \"release\"",
        "mode = \"release\"\ntoolchain_dir = \"./chosen\"",
    );
    let project = resolved(
        command(
            &fixture,
            &fixture.directory,
            &["configuration", "--format", "json"],
        )
        .output()
        .unwrap(),
    );
    assert_eq!(
        project["toolchain"]["directory"],
        fixture.directory.join("chosen").to_str().unwrap()
    );
}

#[test]
fn removed_configuration_and_options_are_rejected() {
    for field in [
        "toolchain_file = \"old.toml\"",
        "schema = \"cheriot.compose/preview1\"",
    ] {
        let fixture = TestFixture::new();
        fixture.replace_text(
            "cheriot-compose.toml",
            "mode = \"release\"",
            &format!("mode = \"release\"\n{field}"),
        );
        let output = command(
            &fixture,
            &fixture.directory,
            &["check", "--toolchain-dir", "tools"],
        )
        .output()
        .unwrap();
        assert!(!output.status.success());
        assert!(String::from_utf8_lossy(&output.stderr).contains("unknown field"));
    }
    let fixture = TestFixture::new();
    for option in ["--toolchain", "--toolchain-file", "--toolchain-path"] {
        assert!(
            !command(&fixture, &fixture.directory, &["check", option, "old"])
                .output()
                .unwrap()
                .status
                .success()
        );
    }
    fixture.replace_text(
        "cheriot-compose.toml",
        "mode = \"release\"",
        "mode = \"release\"\ntoolchain_dir = \"\"",
    );
    let empty = command(&fixture, &fixture.directory, &["check"])
        .output()
        .unwrap();
    assert!(!empty.status.success());
    assert!(String::from_utf8_lossy(&empty.stderr).contains("must not be empty"));
}

#[cfg(all(unix, feature = "host-tool-verification"))]
#[test]
fn generic_compiler_variables_do_not_replace_the_selected_llvm_tools() {
    use std::os::unix::fs::PermissionsExt;
    let fixture = TestFixture::new();
    fixture.write_file(
        "fake-compiler",
        r#"#!/bin/sh
set -eu
for value in "$CC" "$CXX" "$LD" "$OBJDUMP" "$OBJCOPY" "$STRIP"; do
  test "$value" = /missing/ambient-tool
done
output=''; depfile=''; report=''
while [ "$#" -gt 0 ]; do
  case "$1" in
    -o) shift; output=$1 ;;
    -MF) shift; depfile=$1 ;;
    --compartment-report=*) report=${1#*=} ;;
  esac
  shift
done
if [ -n "$output" ]; then printf artifact > "$output"; fi
if [ -n "$depfile" ]; then printf 'dummy: producer.cc\n' > "$depfile"; fi
if [ -n "$report" ]; then printf '{}' > "$report"; fi
"#,
    );
    let compiler = fixture.directory.join("fake-compiler");
    fs::set_permissions(&compiler, fs::Permissions::from_mode(0o755)).unwrap();
    fixture.set_fake_llvm(&compiler);
    let configure = |arguments: &[&str]| {
        let mut command = command(&fixture, &fixture.directory, arguments);
        for variable in ["CC", "CXX", "LD", "OBJDUMP", "OBJCOPY", "STRIP"] {
            command.env(variable, "/missing/ambient-tool");
        }
        command
    };
    let model = resolved(
        configure(&["configuration", "--resolved", "--format", "json"])
            .output()
            .unwrap(),
    );
    assert_eq!(
        model["toolchain"]["cpp_compiler"],
        fixture
            .directory
            .join("fake-llvm/clang++")
            .to_str()
            .unwrap()
    );
    let build = configure(&["build"]).output().unwrap();
    assert!(
        build.status.success(),
        "{}",
        String::from_utf8_lossy(&build.stderr)
    );
    let plan = configure(&["plan", "--format", "shell"]).output().unwrap();
    assert!(plan.status.success());
    fixture.write_file("build.sh", std::str::from_utf8(&plan.stdout).unwrap());
    let shell = Command::new("sh")
        .arg(fixture.directory.join("build.sh"))
        .envs(
            ["CC", "CXX", "LD", "OBJDUMP", "OBJCOPY", "STRIP"]
                .map(|variable| (variable, "/missing/ambient-tool")),
        )
        .output()
        .unwrap();
    assert!(
        shell.status.success(),
        "{}",
        String::from_utf8_lossy(&shell.stderr)
    );
}

#[cfg(all(unix, feature = "host-tool-verification"))]
#[test]
fn symlinked_clang_plus_plus_keeps_its_invocation_name() {
    use std::os::unix::fs::{PermissionsExt, symlink};
    let fixture = TestFixture::new();
    fixture.write_file(
        "fake-compiler",
        r#"#!/bin/sh
set -eu
printf '%s\n' "$0" >> "$TOOL_LOG"
output=''; depfile=''; report=''
while [ "$#" -gt 0 ]; do
  case "$1" in
    -o) shift; output=$1 ;;
    -MF) shift; depfile=$1 ;;
    --compartment-report=*) report=${1#*=} ;;
  esac
  shift
done
if [ -n "$output" ]; then printf artifact > "$output"; fi
if [ -n "$depfile" ]; then printf 'dummy: producer.cc\n' > "$depfile"; fi
if [ -n "$report" ]; then printf '{}' > "$report"; fi
"#,
    );
    let compiler = fixture.directory.join("fake-compiler");
    fs::set_permissions(&compiler, fs::Permissions::from_mode(0o755)).unwrap();
    fixture.set_fake_llvm(&compiler);
    let cpp = fixture.directory.join("fake-llvm/clang++");
    fs::remove_file(&cpp).unwrap();
    symlink("clang", &cpp).unwrap();
    let log = fixture.directory.join("tool.log");
    let build = command(&fixture, &fixture.directory, &["build"])
        .env("TOOL_LOG", &log)
        .output()
        .unwrap();
    assert!(
        build.status.success(),
        "{}",
        String::from_utf8_lossy(&build.stderr)
    );
    assert!(
        fs::read_to_string(log)
            .unwrap()
            .lines()
            .any(|line| line.ends_with("/clang++"))
    );
}

#[cfg(all(unix, feature = "host-tool-verification"))]
#[test]
fn tools_are_checked_only_when_used_and_changes_invalidate_the_cache() {
    use std::os::unix::fs::PermissionsExt;
    let fixture = TestFixture::new();
    let script = r#"#!/bin/sh
set -eu
output=''; depfile=''; report=''
while [ "$#" -gt 0 ]; do
  case "$1" in
    -o) shift; output=$1 ;;
    -MF) shift; depfile=$1 ;;
    --compartment-report=*) report=${1#*=} ;;
  esac
  shift
done
if [ -n "$output" ]; then printf artifact > "$output"; fi
if [ -n "$depfile" ]; then printf 'dummy: producer.cc\n' > "$depfile"; fi
if [ -n "$report" ]; then printf '{}' > "$report"; fi
"#;
    fixture.write_file("fake-compiler", script);
    let source = fixture.directory.join("fake-compiler");
    fs::set_permissions(&source, fs::Permissions::from_mode(0o755)).unwrap();
    fixture.set_fake_llvm(&source);
    fs::remove_file(fixture.directory.join("fake-llvm/llvm-strip")).unwrap();
    fs::remove_file(fixture.directory.join("fake-llvm/llvm-objcopy")).unwrap();
    let plan = fixture.build_plan();
    execution::execute_build_plan(&plan, false, |_| {}).unwrap();
    let mut started = Vec::new();
    execution::execute_build_plan(&plan, false, |event| {
        if let ExecutionEvent::Started { operation, .. } = event {
            started.push(operation.clone());
        }
    })
    .unwrap();
    assert!(started.is_empty(), "unchanged build reran {started:?}");
    let compiler = fixture.directory.join("fake-llvm/clang++");
    fs::write(
        &compiler,
        format!("{script}\n# changed compiler contents\n"),
    )
    .unwrap();
    execution::execute_build_plan(&plan, false, |event| {
        if let ExecutionEvent::Started { operation, .. } = event {
            started.push(operation.clone());
        }
    })
    .unwrap();
    assert!(
        started.iter().any(|id| id.starts_with("compile.")),
        "{started:?}"
    );
    fs::remove_file(&compiler).unwrap();
    let error = execution::execute_build_plan(&plan, false, |_| {})
        .unwrap_err()
        .to_string();
    assert!(
        error.contains("clang++")
            && error.contains(compiler.to_str().unwrap())
            && error.contains("--toolchain-dir"),
        "{error}"
    );
    let script = cheriot_compose::render::render_shell_script(&plan).unwrap();
    fixture.write_file("missing-tool-build.sh", &script);
    let shell = Command::new("sh")
        .arg(fixture.directory.join("missing-tool-build.sh"))
        .output()
        .unwrap();
    assert!(!shell.status.success());
    let error = String::from_utf8_lossy(&shell.stderr);
    assert!(
        error.contains(compiler.to_str().unwrap()) && error.contains("--toolchain-dir"),
        "{error}"
    );
    let alternate = fixture.directory.join("alternate-llvm");
    fs::create_dir(&alternate).unwrap();
    for name in ["clang", "clang++", "ld.lld", "llvm-objdump"] {
        fs::copy(&source, alternate.join(name)).unwrap();
    }
    fixture.set_toolchain_directory(&alternate);
    let changed_plan = fixture.build_plan();
    assert_eq!(
        changed_plan.toolchain.as_ref().unwrap().directory,
        alternate
    );
    started.clear();
    execution::execute_build_plan(&changed_plan, false, |event| {
        if let ExecutionEvent::Started { operation, .. } = event {
            started.push(operation.clone());
        }
    })
    .unwrap();
    assert!(
        started.iter().any(|id| id.starts_with("compile.")),
        "{started:?}"
    );
}

#[cfg(all(unix, feature = "host-tool-verification"))]
#[test]
fn inherited_tool_environment_invalidates_cached_commands() {
    use std::os::unix::fs::PermissionsExt;
    let fixture = TestFixture::new();
    fixture.write_file(
        "fake-compiler",
        r#"#!/bin/sh
set -eu
output=''; depfile=''; report=''
while [ "$#" -gt 0 ]; do
  case "$1" in
    -o) shift; output=$1 ;;
    -MF) shift; depfile=$1 ;;
    --compartment-report=*) report=${1#*=} ;;
  esac
  shift
done
if [ -n "$output" ]; then printf artifact > "$output"; fi
if [ -n "$depfile" ]; then printf 'dummy: producer.cc\n' > "$depfile"; fi
if [ -n "$report" ]; then printf '{}' > "$report"; fi
"#,
    );
    let compiler = fixture.directory.join("fake-compiler");
    fs::set_permissions(&compiler, fs::Permissions::from_mode(0o755)).unwrap();
    fixture.set_fake_llvm(&compiler);
    let run = |value: &str| {
        let output = command(&fixture, &fixture.directory, &["build", "--format", "json"])
            .env("EXTRA_COMPILER_SETTING", value)
            .output()
            .unwrap();
        let report = resolved(output);
        report["events"].as_array().unwrap().iter().any(|event| {
            event["event"] == "started"
                && event["operation"]
                    .as_str()
                    .is_some_and(|name| name.starts_with("compile."))
        })
    };
    assert!(run("one"));
    assert!(!run("one"));
    assert!(run("two"));
    let state = fs::read_to_string(fixture.build_plan().state_path()).unwrap();
    assert!(!state.contains("EXTRA_COMPILER_SETTING"));
}

#[cfg(all(unix, feature = "host-tool-verification"))]
#[test]
fn firmware_rust_is_explicit_and_independent_of_the_llvm_directory() {
    use std::os::unix::fs::PermissionsExt;
    let fixture = TestFixture::new();
    fixture.write_file(
        "fake-compiler",
        r#"#!/bin/sh
set -eu
output=''; depfile=''; report=''
while [ "$#" -gt 0 ]; do
  case "$1" in
    -o) shift; output=$1 ;;
    -MF) shift; depfile=$1 ;;
    --compartment-report=*) report=${1#*=} ;;
  esac
  shift
done
if [ -n "$output" ]; then printf artifact > "$output"; fi
if [ -n "$depfile" ]; then printf 'dummy: producer.rs\n' > "$depfile"; fi
if [ -n "$report" ]; then printf '{}' > "$report"; fi
"#,
    );
    let compiler = fixture.directory.join("fake-compiler");
    fs::set_permissions(&compiler, fs::Permissions::from_mode(0o755)).unwrap();
    fixture.set_fake_llvm(&compiler);
    fixture.write_file("producer.rs", "#![no_std]\n");
    fixture.replace_text("cheriot-compose.toml", "producer.cc", "producer.rs");
    let plan = fixture.build_plan();
    assert!(
        plan.notes
            .iter()
            .any(|note| note.contains("Rust compiler is unresolved"))
    );
    assert!(
        cheriot_compose::render::render_shell_script(&plan)
            .unwrap()
            .contains("firmware Rust compiler is unresolved")
    );
    let error = execution::execute_build_plan(&plan, false, |_| {})
        .unwrap_err()
        .to_string();
    assert!(
        error.contains("firmware Rust compiler is unresolved"),
        "{error}"
    );

    fixture.replace_text(
        "cheriot-compose.toml",
        "mode = \"release\"",
        "mode = \"release\"\ntoolchain = { rust_compiler = \"./missing/rustc\" }",
    );
    let plan = fixture.build_plan();
    let error = execution::execute_build_plan(&plan, false, |_| {})
        .unwrap_err()
        .to_string();
    assert!(
        error.contains("cheriot-compose.toml")
            && error.contains("missing/rustc")
            && error.contains("RUSTC"),
        "{error}"
    );
    let shell = cheriot_compose::render::render_shell_script(&plan).unwrap();
    fixture.write_file("missing-rust.sh", &shell);
    let result = Command::new("sh")
        .arg(fixture.directory.join("missing-rust.sh"))
        .output()
        .unwrap();
    let error = String::from_utf8_lossy(&result.stderr);
    assert!(
        !result.status.success() && error.contains("missing/rustc") && error.contains("RUSTC"),
        "{error}"
    );

    fixture.write_file("crate/Cargo.toml", "[package]\nname = \"firmware-test\"\nversion = \"0.1.0\"\nedition = \"2024\"\n[lib]\ncrate-type = [\"staticlib\"]\n");
    fixture.write_file("crate/src/lib.rs", "#![no_std]\n");
    fixture.replace_text("cheriot-compose.toml", "producer.rs", "crate/Cargo.toml");
    fixture.replace_text(
        "cheriot-compose.toml",
        "toolchain = { rust_compiler = \"./missing/rustc\" }",
        "toolchain = { rust_compiler = \"/bin/true\", cargo = \"./missing/cargo\" }",
    );
    let plan = fixture.build_plan();
    let error = execution::execute_build_plan(&plan, false, |_| {})
        .unwrap_err()
        .to_string();
    assert!(
        error.contains("cheriot-compose.toml")
            && error.contains("missing/cargo")
            && error.contains("toolchain.cargo"),
        "{error}"
    );

    let configuration = resolved(
        command(
            &fixture,
            &fixture.directory,
            &[
                "configuration",
                "--resolved",
                "--format",
                "json",
                "--toolchain-dir",
                "another-llvm",
            ],
        )
        .env("RUSTC", "/custom/cheriot-rustc")
        .output()
        .unwrap(),
    );
    assert_eq!(
        configuration["toolchain"]["rust_compiler"],
        "/custom/cheriot-rustc"
    );
    assert_eq!(
        configuration["toolchain"]["directory"],
        fixture.directory.join("another-llvm").to_str().unwrap()
    );
}
