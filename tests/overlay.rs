// Copyright 3Bian Limited and Contributors
// SPDX-License-Identifier: MIT

use std::path::Path;
#[cfg(feature = "rtos-verification")]
use std::{path::PathBuf, process::Command};

#[cfg(feature = "rtos-verification")]
use cheriot_compose::configuration;

#[cfg(feature = "rtos-verification")]
#[path = "support/rtos_tree.rs"]
mod rtos_tree;

#[test]
fn overlay_contains_only_directories_and_regular_toml_files() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("integrations/cheriot-rtos");
    let mut directories = vec![root];
    let mut files = 0;
    while let Some(directory) = directories.pop() {
        for entry in std::fs::read_dir(directory).unwrap() {
            let entry = entry.unwrap();
            let kind = entry.file_type().unwrap();
            if kind.is_dir() {
                directories.push(entry.path());
            } else {
                assert!(
                    kind.is_file(),
                    "overlay entry must be a regular file: {}",
                    entry.path().display()
                );
                assert_eq!(
                    entry
                        .path()
                        .extension()
                        .and_then(|extension| extension.to_str()),
                    Some("toml")
                );
                let text = std::fs::read_to_string(entry.path()).unwrap();
                assert!(
                    !text.contains(env!("CARGO_MANIFEST_DIR")),
                    "overlay contains a host checkout path"
                );
                files += 1;
            }
        }
    }
    assert!(files > 40);
}

#[cfg(feature = "rtos-verification")]
fn installed_binary(root: &Path) -> PathBuf {
    let binary = root.join("installed-cheriot-compose");
    std::fs::copy(env!("CARGO_BIN_EXE_cheriot-compose"), &binary).unwrap();
    binary
}

#[test]
#[cfg(feature = "rtos-verification")]
fn copied_overlay_supports_workspace_and_project_commands() {
    let tree = rtos_tree::RtosTree::new();
    let installed = installed_binary(&tree.root);
    let run = |arguments: &[&str]| {
        Command::new(&installed)
            .args(arguments)
            .current_dir(&tree.root)
            .output()
            .unwrap()
    };
    let checked = run(&["check"]);
    assert!(
        checked.status.success(),
        "{}",
        String::from_utf8_lossy(&checked.stderr)
    );
    assert!(String::from_utf8_lossy(&checked.stdout).contains("21 workspace projects"));
    let configuration = run(&[
        "configuration",
        "--project",
        "tests",
        "--resolved",
        "--format",
        "json",
    ]);
    assert!(
        configuration.status.success(),
        "{}",
        String::from_utf8_lossy(&configuration.stderr)
    );
    let plan = run(&["plan", "--project", "tests", "--format", "json"]);
    assert!(
        plan.status.success(),
        "{}",
        String::from_utf8_lossy(&plan.stderr)
    );
    let plan_text = String::from_utf8(plan.stdout).unwrap();
    assert!(!plan_text.contains(env!("CARGO_MANIFEST_DIR")));
    assert!(plan_text.contains("compile.test_runner.0"));
    assert!(
        run(&["build", "--project", "tests", "--dry-run"])
            .status
            .success()
    );
    assert!(
        run(&["run", "--project", "tests", "--dry-run"])
            .status
            .success()
    );
    let unknown = run(&["check", "--project", "missing"]);
    assert!(!unknown.status.success());
    assert!(String::from_utf8_lossy(&unknown.stderr).contains("unknown workspace project"));
    let unselected = run(&["plan"]);
    assert!(!unselected.status.success());
    assert!(String::from_utf8_lossy(&unselected.stderr).contains("--project"));
}

#[test]
#[cfg(feature = "rtos-verification")]
fn every_workspace_plan_resolves_local_inputs_or_declares_their_producer() {
    let tree = rtos_tree::RtosTree::new();
    let workspace = configuration::read_workspace(&tree.root.join("cheriot-compose.toml"))
        .unwrap()
        .unwrap();
    for (name, path) in &workspace.projects {
        let project = cheriot_compose::project::load_project(path)
            .unwrap_or_else(|error| panic!("{name}: {error}"));
        assert!(
            project.sdk.starts_with(&tree.root),
            "{name}: SDK is outside the copied checkout"
        );
        let plan = cheriot_compose::planning::create_build_plan(&project)
            .unwrap_or_else(|error| panic!("{name}: {error}"));
        for operation in &plan.operations {
            for input in &operation.inputs {
                if input.exists() {
                    assert!(
                        input.starts_with(&tree.root),
                        "{name}: input outside checkout: {}",
                        input.display()
                    );
                } else {
                    assert!(
                        plan.artifact_producer(input).is_some(),
                        "{name}: unresolved input {}",
                        input.display()
                    );
                }
            }
            for output in &operation.outputs {
                assert!(
                    output.starts_with(&tree.root),
                    "{name}: output outside checkout: {}",
                    output.display()
                );
            }
        }
    }
}

#[test]
#[cfg(feature = "rtos-verification")]
fn incomplete_or_incompatible_sdk_has_actionable_diagnostics() {
    let tree = rtos_tree::RtosTree::new();
    let project = tree
        .root
        .join("examples/06.producer-consumer/cheriot-compose.toml");
    std::fs::remove_file(tree.root.join("sdk/core/switcher/entry.S")).unwrap();
    let error = cheriot_compose::project::load_project(&project)
        .unwrap_err()
        .to_string();
    assert!(error.contains("incompatible CHERIoT RTOS SDK"), "{error}");
    assert!(error.contains("submodules"), "{error}");
    assert!(!error.contains("revision"), "{error}");
    let tree = rtos_tree::RtosTree::new();
    let project = tree
        .root
        .join("examples/06.producer-consumer/cheriot-compose.toml");
    std::fs::write(
        tree.root.join("sdk/include/cdefs.h"),
        "/* unsupported SDK */\n",
    )
    .unwrap();
    let error = cheriot_compose::project::load_project(&project)
        .unwrap_err()
        .to_string();
    assert!(error.contains("lacks required capability"), "{error}");
    assert!(error.contains("__cheri_compartment"), "{error}");
    let tree = rtos_tree::RtosTree::new();
    let project = tree
        .root
        .join("examples/06.producer-consumer/cheriot-compose.toml");
    std::fs::remove_dir_all(tree.root.join("sdk/include/libc++")).unwrap();
    let error = cheriot_compose::project::load_project(&project)
        .unwrap_err()
        .to_string();
    assert!(error.contains("required directory"), "{error}");
    assert!(error.contains("include/libc++"), "{error}");
    let tree = rtos_tree::RtosTree::new();
    let project = tree.root.join("tests/cheriot-compose.toml");
    let text = std::fs::read_to_string(&project)
        .unwrap()
        .replace("board = \"sail\"", "board = \"sonata-simulator\"");
    std::fs::write(&project, text).unwrap();
    std::fs::remove_file(tree.root.join("sdk/scripts/run-sonata-sim.sh")).unwrap();
    let error = cheriot_compose::project::load_project(&project)
        .unwrap_err()
        .to_string();
    assert!(
        error.contains("board runner file does not exist"),
        "{error}"
    );
}

#[test]
#[cfg(feature = "rtos-verification")]
fn workspace_rejects_paths_outside_its_root() {
    let tree = rtos_tree::RtosTree::new();
    let workspace = tree.root.join("cheriot-compose.toml");
    std::fs::write(
        &workspace,
        "[workspace.projects]\nbad = \"../outside.toml\"\n",
    )
    .unwrap();
    let error = configuration::read_workspace(&workspace)
        .err()
        .unwrap()
        .to_string();
    assert!(error.contains("paths must be relative"), "{error}");
}

#[test]
#[cfg(feature = "rtos-verification")]
fn relocated_sdk_and_external_application_do_not_depend_on_the_composer_checkout() {
    let first = rtos_tree::RtosTree::new();
    let second = rtos_tree::RtosTree::new();
    for tree in [&first, &second] {
        let installed = installed_binary(&tree.root);
        let output = Command::new(installed)
            .current_dir(&tree.root)
            .args([
                "plan",
                "--toolchain-dir",
                "/cheriot-tools/bin",
                "--project",
                "hello-world",
                "--format",
                "json",
            ])
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let plan = String::from_utf8(output.stdout).unwrap();
        assert!(plan.contains(tree.root.to_str().unwrap()));
        assert!(!plan.contains(env!("CARGO_MANIFEST_DIR")));
        assert!(!tree.root.join(".git").exists());
    }
    let external = first.root.with_extension("external-application");
    std::fs::create_dir_all(&external).unwrap();
    let sdk = second.root.join("sdk");
    let hello = second.root.join("examples/01.hello_world");
    let manifest = std::fs::read_to_string(hello.join("cheriot-compose.toml"))
        .unwrap()
        .replace(
            "../../sdk/cheriot-compose.toml",
            &sdk.join("cheriot-compose.toml").display().to_string(),
        )
        .replace(
            "../../sdk/boards/sail.toml",
            &sdk.join("boards/sail.toml").display().to_string(),
        )
        .replace("../../sdk", &sdk.display().to_string());
    std::fs::write(external.join("cheriot-compose.toml"), manifest).unwrap();
    std::fs::copy(hello.join("hello.cc"), external.join("hello.cc")).unwrap();
    let installed = installed_binary(&second.root);
    let output = Command::new(installed)
        .current_dir(&external)
        .args([
            "plan",
            "--toolchain-dir",
            "/cheriot-tools/bin",
            "--format",
            "json",
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let plan = String::from_utf8(output.stdout).unwrap();
    assert!(plan.contains(external.to_str().unwrap()));
    assert!(plan.contains(sdk.to_str().unwrap()));
    assert!(!plan.contains(env!("CARGO_MANIFEST_DIR")));
    std::fs::remove_dir_all(external).unwrap();
}

#[test]
#[cfg(feature = "rtos-verification")]
fn optional_sdk_component_is_required_only_when_selected() {
    let tree = rtos_tree::RtosTree::new();
    let missing = tree
        .root
        .join("sdk/third_party/microvium/dist-c/microvium.c");
    std::fs::remove_file(&missing).unwrap();
    let hello = tree
        .root
        .join("examples/01.hello_world/cheriot-compose.toml");
    cheriot_compose::project::load_project(&hello).unwrap();
    let javascript = tree
        .root
        .join("examples/09.javascript/cheriot-compose.toml");
    let error = cheriot_compose::project::load_project(&javascript)
        .unwrap_err()
        .to_string();
    assert!(
        error.contains("microvium") && error.contains(missing.to_str().unwrap()),
        "{error}"
    );
    assert!(error.contains("submodule"), "{error}");
    assert!(!error.contains("revision"), "{error}");
}

#[test]
#[cfg(feature = "rtos-verification")]
fn sdk_revision_allowlists_are_not_part_of_the_configuration() {
    let tree = rtos_tree::RtosTree::new();
    let sdk_manifest = tree.root.join("sdk/cheriot-compose.toml");
    let source = std::fs::read_to_string(&sdk_manifest).unwrap().replace(
        "integration = \"CHERIoT RTOS\"",
        "integration = \"CHERIoT RTOS\"\nsupported_revisions = [\"old\"]",
    );
    std::fs::write(sdk_manifest, source).unwrap();
    let project = tree
        .root
        .join("examples/01.hello_world/cheriot-compose.toml");
    let error = cheriot_compose::project::load_project(&project)
        .unwrap_err()
        .to_string();
    assert!(
        error.contains("unknown field") && error.contains("supported_revisions"),
        "{error}"
    );
}
