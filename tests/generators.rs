// Copyright 3Bian Limited and Contributors
// SPDX-License-Identifier: MIT

#[cfg(all(unix, feature = "host-tool-verification"))]
use std::{fs, os::unix::fs::PermissionsExt, process::Command};

#[cfg(all(unix, feature = "host-tool-verification"))]
use cheriot_compose::{
    build_graph::BuildAction,
    execution::{self, ExecutionEvent},
    render,
};
use test_fixture::TestFixture;

#[path = "support/test_fixture.rs"]
mod test_fixture;

#[cfg(all(unix, feature = "host-tool-verification"))]
fn executable(fixture: &TestFixture, path: &str, contents: &str) {
    fixture.write_file(path, contents);
    fs::set_permissions(
        fixture.directory.join(path),
        fs::Permissions::from_mode(0o755),
    )
    .unwrap();
}

fn configure(fixture: &TestFixture, generators: &str) {
    fixture.replace_text(
        "cheriot-compose.toml",
        "include = [\"./sdk/cheriot-compose.toml\"]",
        "include = [\"./sdk/cheriot-compose.toml\", \"./tools/generators.toml\"]",
    );
    fixture.write_file("tools/generators.toml", generators);
}

#[cfg(all(unix, feature = "host-tool-verification"))]
fn toolchain(fixture: &TestFixture) {
    executable(
        fixture,
        "compiler",
        r##"#!/bin/sh
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
"##,
    );
    fixture.set_fake_llvm(&fixture.directory.join("compiler"));
}

#[test]
#[cfg(all(unix, feature = "host-tool-verification"))]
fn included_generators_produce_compiled_sources_and_headers() {
    let fixture = TestFixture::new();
    toolchain(&fixture);
    executable(
        &fixture,
        "tools/make-source",
        "#!/bin/sh\nset -eu\ncat \"$1\" > \"$2\"\nprintf '%s' \"$GENERATOR_MESSAGE\"\n",
    );
    executable(
        &fixture,
        "tools/make-header",
        "#!/bin/sh\nset -eu\ncp \"$1\" \"$2\"\n",
    );
    fixture.write_file("tools/source.in", "first generated content\n");
    fixture.write_file("tools/metadata.txt", "first metadata\n");
    configure(
        &fixture,
        r#"
[generators.source]
executable = "./make-source"
arguments = ["source.in", "${build}/generated/producer.cc", "$(touch INJECTION)"]
inputs = ["source.in", "metadata.txt"]
outputs = ["generated/producer.cc", "generated/log.txt"]
stdout = "generated/log.txt"
environment = { GENERATOR_MESSAGE = "hello world" }

[generators.header]
executable = "./make-header"
arguments = ["${build}/generated/producer.cc", "${build}/generated/header.h"]
inputs = ["${build}/generated/producer.cc"]
outputs = ["generated/header.h"]
"#,
    );
    fixture.replace_text("cheriot-compose.toml", "build = { sources = [\"producer.cc\"] }", "build = { sources = [\"${build}/generated/producer.cc\"], includes = [\"${build}/generated\"], force_includes = [\"${build}/generated/header.h\"] }");
    let project = cheriot_compose::project::load_project(&fixture.project_file()).unwrap();
    assert_eq!(
        project
            .generators
            .iter()
            .map(|g| g.name.as_str())
            .collect::<Vec<_>>(),
        ["source", "header"]
    );
    assert_eq!(
        project.generators[0].working_directory,
        fixture.directory.join("tools")
    );
    assert_eq!(
        project.generators[0].inputs,
        [
            fixture.directory.join("tools/source.in"),
            fixture.directory.join("tools/metadata.txt"),
        ]
    );
    let plan = cheriot_compose::planning::create_build_plan(&project).unwrap();
    let producer = plan
        .operations
        .iter()
        .find(|op| op.id == "compile.producer.0")
        .unwrap();
    assert!(
        producer
            .dependencies
            .contains(&"generate.user.source".to_owned())
    );
    assert!(
        producer
            .dependencies
            .contains(&"generate.user.header".to_owned())
    );
    let header = plan
        .operations
        .iter()
        .find(|op| op.id == "generate.user.header")
        .unwrap();
    assert_eq!(header.dependencies, ["generate.user.source"]);
    let mut events = Vec::new();
    execution::execute_build_plan_with_jobs(&plan, false, true, 3, |event| {
        events.push(event.clone())
    })
    .unwrap();
    let source = project.build_dir.join("generated/producer.cc");
    let header = project.build_dir.join("generated/header.h");
    assert_eq!(
        fs::read_to_string(&source).unwrap(),
        "first generated content\n"
    );
    assert_eq!(
        fs::read_to_string(&header).unwrap(),
        "first generated content\n"
    );
    assert_eq!(
        fs::read_to_string(project.build_dir.join("generated/log.txt")).unwrap(),
        "hello world"
    );
    events.clear();
    execution::execute_build_plan_with_jobs(&plan, false, true, 3, |event| {
        events.push(event.clone())
    })
    .unwrap();
    assert!(
        !events
            .iter()
            .any(|event| matches!(event, ExecutionEvent::Started { .. }))
    );
    fixture.write_file("tools/metadata.txt", "changed metadata\n");
    events.clear();
    execution::execute_build_plan_with_jobs(&plan, false, true, 3, |event| {
        events.push(event.clone())
    })
    .unwrap();
    assert!(events.iter().any(|event| matches!(event, ExecutionEvent::Completed { operation, changed: false } if operation == "generate.user.source")));
    assert!(!events.iter().any(|event| matches!(event, ExecutionEvent::Started { operation, .. } if operation == "compile.producer.0")));
    fixture.write_file("tools/source.in", "changed generated content\n");
    events.clear();
    execution::execute_build_plan_with_jobs(&plan, false, true, 3, |event| {
        events.push(event.clone())
    })
    .unwrap();
    assert!(events.iter().any(|event| matches!(event, ExecutionEvent::Started { operation, .. } if operation == "compile.producer.0")));
    assert_eq!(
        fs::read_to_string(&header).unwrap(),
        "changed generated content\n"
    );
    let script = render::render_shell_script(&plan).unwrap();
    fixture.write_file("build.sh", &script);
    execution::clean_build_plan(&plan).unwrap();
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
        fs::read_to_string(&header).unwrap(),
        "changed generated content\n"
    );
    assert!(!fixture.directory.join("tools/INJECTION").exists());
    assert!(matches!(
        &plan.operations[0].action,
        BuildAction::Command { .. }
    ));
}

#[test]
fn generators_reject_unsafe_outputs_missing_inputs_and_cycles() {
    let fixture = TestFixture::new();
    configure(
        &fixture,
        r#"[generators.bad]
executable = "tool"
outputs = ["../outside"]
"#,
    );
    let load = || {
        cheriot_compose::project::load_project(&fixture.project_file())
            .unwrap_err()
            .to_string()
    };
    assert!(load().contains("inside the build directory"));
    fixture.write_file(
        "tools/generators.toml",
        r#"[generators.bad]
executable = "tool"
inputs = ["missing"]
outputs = ["output"]
"#,
    );
    assert!(load().contains("input does not exist"));
    fixture.write_file(
        "tools/generators.toml",
        r#"[generators.first]
executable = "tool"
inputs = ["${build}/second"]
outputs = ["first"]
[generators.second]
executable = "tool"
inputs = ["${build}/first"]
outputs = ["second"]
"#,
    );
    assert!(load().contains("cycle"));
    fixture.write_file(
        "tools/generators.toml",
        r#"[generators.bad]
executable = "tool"
outputs = ["output"]
stdout = "another"
"#,
    );
    assert!(load().contains("declared generator output"));
}
