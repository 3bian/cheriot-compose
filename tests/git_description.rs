// Copyright 3Bian Limited and Contributors
// SPDX-License-Identifier: MIT

#![cfg(all(unix, feature = "host-tool-verification"))]

use std::collections::{BTreeMap, BTreeSet};
use std::os::unix::fs::PermissionsExt;

use cheriot_compose::{
    build_graph::{BuildAction, BuildCommand, BuildOperation, BuildOperationKind, BuildPlan},
    diagnostic::Sourced,
    execution::{self, ExecutionEvent},
    render,
};
use test_fixture::TestFixture;

#[path = "support/test_fixture.rs"]
mod test_fixture;

fn command(executable: &str, cwd: &std::path::Path, arguments: &[String]) -> BuildCommand {
    BuildCommand {
        executable: executable.into(),
        argv: arguments
            .iter()
            .map(|value| Sourced::derived(value.clone(), "test"))
            .collect(),
        cwd: cwd.into(),
        environment: BTreeMap::new(),
        stdout: None,
        stderr: None,
    }
}

#[test]
fn git_description_is_equivalent_in_native_and_shell_builds_and_rebuilds_only_when_changed() {
    let fixture = TestFixture::new();
    fixture.write_file("fake-git", "#!/bin/sh\ncat \"$DESCRIPTION_PATH\"\n");
    fixture.write_file("fake-compiler", "#!/bin/sh\ncp \"$1\" \"$2\"\n");
    let description = "tag\"quoted\\value $(touch HACKED) `touch HACKED2`\n";
    fixture.write_file("description.txt", description);
    for name in ["fake-git", "fake-compiler"] {
        std::fs::set_permissions(
            fixture.directory.join(name),
            std::fs::Permissions::from_mode(0o755),
        )
        .unwrap();
    }
    let build_dir = fixture.directory.join("generated");
    let header = build_dir.join("version.h");
    let output_file = build_dir.join("compiled.o");
    let mut git = command(
        fixture.directory.join("fake-git").to_str().unwrap(),
        &fixture.directory,
        &[],
    );
    git.environment.insert(
        "DESCRIPTION_PATH".into(),
        fixture
            .directory
            .join("description.txt")
            .display()
            .to_string(),
    );
    let mut generate = BuildOperation {
        id: "generate.version".into(),
        kind: BuildOperationKind::Generate,
        policy: Default::default(),
        action: BuildAction::GitDescription { command: git },
        inputs: vec![],
        outputs: vec![header.clone()],
        dependencies: vec![],
        depfile: None,
        provenance: vec![],
        fingerprint: String::new(),
    };
    generate.policy =
        cheriot_compose::build_graph::OperationPolicy::for_action(&generate.kind, &generate.action);
    generate.update_fingerprint().unwrap();
    let mut compile = BuildOperation {
        id: "compile.version".into(),
        kind: BuildOperationKind::Compile,
        policy: Default::default(),
        action: BuildAction::Command {
            command: command(
                fixture.directory.join("fake-compiler").to_str().unwrap(),
                &fixture.directory,
                &[
                    header.display().to_string(),
                    output_file.display().to_string(),
                ],
            ),
            conditional: None,
        },
        inputs: vec![header.clone()],
        outputs: vec![output_file.clone()],
        dependencies: vec![generate.id.clone()],
        depfile: None,
        provenance: vec![],
        fingerprint: String::new(),
    };
    compile.update_fingerprint().unwrap();
    let plan = BuildPlan {
        schema: "test".into(),
        profile: "test".into(),
        notes: vec![],
        toolchain: None,
        build_dir,
        state_key: None,
        state_owner: None,
        publication: None,
        operations: vec![generate, compile],
        probes: BTreeSet::new(),
    };
    execution::execute_build_plan(&plan, false, |_| {}).unwrap();
    let expected = b"#define CHERIOT_RTOS_GIT_DESCRIPTION \"tag\\\"quoted\\\\value $(touch HACKED) `touch HACKED2`\"\n";
    assert_eq!(std::fs::read(&header).unwrap(), expected);
    let mut events = Vec::new();
    execution::execute_build_plan(&plan, false, |event| events.push(event.clone())).unwrap();
    assert!(events.iter().any(|event| matches!(event, ExecutionEvent::Skipped { operation, .. } if operation == "compile.version")));
    fixture.write_file("description.txt", "new-version\n");
    events.clear();
    execution::execute_build_plan(&plan, false, |event| events.push(event.clone())).unwrap();
    assert!(events.iter().any(|event| matches!(event, ExecutionEvent::Started { operation, .. } if operation == "compile.version")));
    assert_eq!(
        std::fs::read(&output_file).unwrap(),
        b"#define CHERIOT_RTOS_GIT_DESCRIPTION \"new-version\"\n"
    );

    fixture.write_file("description.txt", description);
    execution::clean_build_plan(&plan).unwrap();
    fixture.write_file("build.sh", &render::render_shell_script(&plan).unwrap());
    let status = std::process::Command::new("sh")
        .arg(fixture.directory.join("build.sh"))
        .status()
        .unwrap();
    assert!(status.success());
    assert_eq!(std::fs::read(&header).unwrap(), expected);
    assert_eq!(std::fs::read(&output_file).unwrap(), expected);
    assert!(!fixture.directory.join("HACKED").exists());
    assert!(!fixture.directory.join("HACKED2").exists());
    std::fs::remove_file(fixture.directory.join("description.txt")).unwrap();
    execution::execute_build_plan(&plan, false, |_| {}).unwrap();
    assert_eq!(
        std::fs::read(&header).unwrap(),
        b"#define CHERIOT_RTOS_GIT_DESCRIPTION \"unknown\"\n"
    );
    execution::clean_build_plan(&plan).unwrap();
    let status = std::process::Command::new("sh")
        .arg(fixture.directory.join("build.sh"))
        .status()
        .unwrap();
    assert!(status.success());
    assert_eq!(
        std::fs::read(&header).unwrap(),
        b"#define CHERIOT_RTOS_GIT_DESCRIPTION \"unknown\"\n"
    );
}
