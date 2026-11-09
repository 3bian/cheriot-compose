// Copyright 3Bian Limited and Contributors
// SPDX-License-Identifier: MIT

#![cfg(feature = "rtos-verification")]

use std::path::Path;

use cheriot_compose::{build_graph::BuildAction, planning};
use test_fixture::{TestFixture, rtos_tree};

#[path = "support/test_fixture.rs"]
mod test_fixture;

#[test]
fn every_rtos_lua_build_file_has_a_toml_file() {
    let workspace = rtos_tree::RtosTree::new();
    let root = workspace.root.clone();
    let mut directories = vec![root.clone()];
    let mut discovered = Vec::new();
    while let Some(directory) = directories.pop() {
        for entry in std::fs::read_dir(&directory).unwrap() {
            let entry = entry.unwrap();
            let file_type = entry.file_type().unwrap();
            if file_type.is_dir() {
                let name = entry.file_name();
                if !["build", "target", ".git", ".cheriot-compose"]
                    .iter()
                    .any(|excluded| name == *excluded)
                {
                    directories.push(entry.path());
                }
            } else if entry.file_name() == "xmake.lua" {
                let configuration = directory.join("cheriot-compose.toml");
                assert!(
                    configuration.is_file(),
                    "missing {}",
                    configuration.display()
                );
                let document: toml::Value =
                    toml::from_str(&std::fs::read_to_string(&configuration).unwrap()).unwrap();
                assert!(
                    document.get("schema").is_none(),
                    "{}",
                    configuration.display()
                );
                let category = directory.strip_prefix(&root).unwrap().components().next();
                if category.is_some_and(|category| {
                    [
                        "examples",
                        "exercises",
                        "benchmarks",
                        "tests",
                        "tests.extra",
                    ]
                    .iter()
                    .any(|name| category.as_os_str() == *name)
                }) {
                    let project = cheriot_compose::project::load_project(&configuration)
                        .unwrap_or_else(|error| panic!("{}: {error}", configuration.display()));
                    planning::create_build_plan(&project)
                        .unwrap_or_else(|error| panic!("{}: {error}", configuration.display()));
                }
                discovered.push(configuration);
            }
        }
    }
    assert!(
        discovered.len() >= 16,
        "expected the RTOS example and test projects"
    );
}

fn generated_content<'a>(plan: &'a cheriot_compose::build_graph::BuildPlan, name: &str) -> &'a str {
    let operation = plan
        .operations
        .iter()
        .find(|operation| operation.id == format!("generate.{name}"))
        .unwrap();
    let BuildAction::Generate { content, .. } = &operation.action else {
        panic!("expected generated content")
    };
    content
}

#[test]
fn main_test_suite_matches_sdk_selection_and_generated_templates() {
    let workspace = rtos_tree::RtosTree::new();
    let directory = workspace.root.join("tests");
    let project =
        cheriot_compose::project::load_project(&directory.join("cheriot-compose.toml")).unwrap();
    let suite = project.test_suite.as_ref().unwrap();
    assert_eq!(suite.cases.len(), 28);
    assert_eq!(suite.cases.iter().filter(|case| case.enabled).count(), 26);
    assert!(
        !suite
            .cases
            .iter()
            .find(|case| case.name == "bigdata")
            .unwrap()
            .enabled
    );
    assert!(
        !suite
            .cases
            .iter()
            .find(|case| case.name == "rust")
            .unwrap()
            .enabled
    );
    assert_eq!(
        project
            .threads
            .iter()
            .map(|thread| thread.priority)
            .collect::<Vec<_>>(),
        [2, 1, 0]
    );
    assert_eq!(project.shared_objects["allocator_hazard_pointers"], 48);
    assert_eq!(project.shared_objects["exampleK"], 1024);
    assert_eq!(project.shared_objects["test_word"], 4);

    let plan = planning::create_build_plan(&project).unwrap();
    let tests = [
        ("debug_cxx", "Debug helpers (C++)", true),
        ("debug_c", "Debug helpers (C)", true),
        ("mmio", "MMIO", true),
        ("unwind_cleanup", "Unwind cleanup", true),
        ("softfloat", "softfloat", true),
        ("stdio", "stdio", true),
        ("static_sealing", "Static sealing", true),
        ("crash_recovery", "Crash recovery", true),
        ("compartment_calls", "compartment_calls", true),
        ("check_pointer", "check_pointer", true),
        ("misc", "misc", true),
        ("stack", "Switcher stack handling", true),
        ("thread_pool", "Thread pool", true),
        ("global_constructors", "global_constructors", false),
        ("queue", "Queue", true),
        ("futex", "Futex", true),
        ("locks", "Locks", true),
        ("list", "List", true),
        ("eventgroup", "Event groups", true),
        ("multiwaiter", "Multiwatier", true),
        ("allocator", "Allocator", true),
        ("bitpacks", "bitpacks", true),
        ("time", "time", true),
    ];
    let mut header = vec![
        "static const int TestEnabled_ccompile = 1;".to_owned(),
        "static const int TestEnabled_extra_compiles = 1;".to_owned(),
        "static const int TestEnabled_preflight = 1;".to_owned(),
    ];
    let mut calls = Vec::new();
    for (name, display, compartment) in tests {
        header.push(format!(
            "{}int test_{name}();",
            if compartment {
                format!("__cheri_compartment(\"{name}_test\")")
            } else {
                String::new()
            }
        ));
        calls.push(format!("\t\trun_timed(\"{display}\", test_{name});"));
    }
    let expected_header = std::fs::read_to_string(directory.join("tests-prototypes.h.in"))
        .unwrap()
        .replace("@test_header@", &header.join("\n"));
    let expected_calls = std::fs::read_to_string(directory.join("tests-all.inc.in"))
        .unwrap()
        .replace("@test_calls@", &calls.join("\n"));
    assert_eq!(
        generated_content(&plan, "tests-prototypes.h"),
        expected_header
    );
    assert_eq!(generated_content(&plan, "tests-all.inc"), expected_calls);
    for variant in [
        "semaphore-only",
        "mutex-only",
        "recursive-mutex-only",
        "all-options",
    ] {
        assert!(
            plan.operations
                .iter()
                .any(|operation| operation.id == format!("compile.freertos-compile-{variant}.0"))
        );
    }
    assert!(
        project.targets["test_runner"]
            .sources
            .iter()
            .any(|source| source.path.ends_with("global_constructors-test.cc"))
    );
    assert!(
        !plan
            .operations
            .iter()
            .any(|operation| operation.id.starts_with("compile.rust_test"))
    );
    let git = plan
        .operations
        .iter()
        .find(|operation| operation.id == "generate.rtos-git-description.h")
        .unwrap();
    assert!(
        matches!(&git.action, BuildAction::GitDescription { command } if command.executable == "git")
    );
    let version = plan
        .operations
        .iter()
        .find(|operation| operation.id == "compile.test_runner.1")
        .unwrap();
    assert!(version.dependencies.contains(&git.id));
    assert!(
        version
            .inputs
            .contains(&plan.build_dir.join("rtos-git-description.h"))
    );
    assert!(
        version
            .inputs
            .contains(&plan.build_dir.join("tests-prototypes.h"))
    );
}

fn copy_test_project(fixture: &TestFixture, rtos: &Path) -> String {
    let source = rtos.join("tests");
    for entry in std::fs::read_dir(&source).unwrap() {
        let entry = entry.unwrap();
        if entry.file_type().unwrap().is_file()
            && matches!(
                entry
                    .path()
                    .extension()
                    .and_then(|extension| extension.to_str()),
                Some("c" | "cc" | "rs")
            )
        {
            std::fs::copy(entry.path(), fixture.directory.join(entry.file_name())).unwrap();
        }
    }
    let sdk = rtos.join("sdk");
    let sdk_path = toml::Value::String(sdk.display().to_string()).to_string();
    let catalogue =
        toml::Value::String(sdk.join("cheriot-compose.toml").display().to_string()).to_string();
    std::fs::read_to_string(source.join("cheriot-compose.toml"))
        .unwrap()
        .replace("sdk = \"../sdk\"", &format!("sdk = {sdk_path}"))
        .replace(
            "include = [\"../sdk/cheriot-compose.toml\"]",
            &format!("include = [{catalogue}]"),
        )
}

#[test]
fn test_selection_honors_opt_in_conflicts_and_plans_rust() {
    let fixture = TestFixture::new();
    let workspace = rtos_tree::RtosTree::new();
    let original = copy_test_project(&fixture, &workspace.root);
    let bigdata = "include_in_most = false, name = \"bigdata\", target = \"bigdata_test\"";
    let selected = original
        .replace("most_tests = true", "most_tests = false")
        .replace(bigdata, &format!("enabled = true, {bigdata}"));
    fixture.write_file("cheriot-compose.toml", &selected);
    let project = cheriot_compose::project::load_project(&fixture.project_file()).unwrap();
    let suite = project.test_suite.as_ref().unwrap();
    assert_eq!(
        suite
            .cases
            .iter()
            .filter(|case| case.enabled)
            .map(|case| case.name.as_str())
            .collect::<Vec<_>>(),
        ["bigdata"]
    );
    assert!(
        !project.targets["test_runner"]
            .sources
            .iter()
            .any(|source| source
                .path
                .ends_with(Path::new("global_constructors-test.cc")))
    );
    let plan = planning::create_build_plan(&project).unwrap();
    assert!(generated_content(&plan, "tests-prototypes.h").contains("TestEnabled_preflight = 0"));
    assert!(
        generated_content(&plan, "tests-all.inc").contains("run_timed(\"bigdata\", test_bigdata)")
    );

    fixture.write_file(
        "cheriot-compose.toml",
        &original.replace(bigdata, &format!("enabled = true, {bigdata}")),
    );
    assert!(
        cheriot_compose::project::load_project(&fixture.project_file())
            .unwrap_err()
            .to_string()
            .contains("conflicts with test allocator")
    );
    let rust = "firmware_rust_source = \"rust-test.rs\", include_in_most = false, name = \"rust\", target = \"rust_test\"";
    fixture.write_file(
        "cheriot-compose.toml",
        &original.replace(rust, &format!("enabled = true, {rust}")),
    );
    let project = cheriot_compose::project::load_project(&fixture.project_file()).unwrap();
    let plan = planning::create_build_plan(&project).unwrap();
    let rust_compile = plan
        .operations
        .iter()
        .find(|operation| operation.id == "compile.rust_test.1")
        .unwrap();
    assert_eq!(
        rust_compile.kind,
        cheriot_compose::build_graph::BuildOperationKind::RustCompile
    );
    let BuildAction::Command { command, .. } = &rust_compile.action else {
        panic!("expected Rust compiler")
    };
    assert!(
        command
            .argv
            .iter()
            .any(|argument| argument.value == "--target=riscv32cheriot-unknown-cheriotrtos")
    );
    assert!(
        command
            .argv
            .iter()
            .any(|argument| argument.value == "--crate-type=staticlib")
    );
    assert!(rust_compile.depfile.is_some());
    assert!(
        plan.operations
            .iter()
            .find(|operation| operation.id == "link.rust_test")
            .unwrap()
            .dependencies
            .contains(&rust_compile.id)
    );
    let database = cheriot_compose::compilation_database::export_compilation_database(
        &plan,
        &plan.unresolved_probes(),
    )
    .unwrap();
    assert!(
        database
            .iter()
            .all(|entry| !entry.file.ends_with("rust-test.rs"))
    );
}

#[test]
fn selected_tests_receive_float_options_and_dependencies() {
    let fixture = TestFixture::new();
    let workspace = rtos_tree::RtosTree::new();
    let mmio = "name = \"mmio\", target = \"mmio_test\"";
    let document = copy_test_project(&fixture, &workspace.root)
        .replace("most_tests = true", "most_tests = false")
        .replace(mmio, &format!("enabled = true, {mmio}"))
        .replace(
            "[services.test_runner]",
            "[options]\nprint_doubles = true\nprint_floats = true\n\n[services.test_runner]",
        );
    fixture.write_file("cheriot-compose.toml", &document);
    let project = cheriot_compose::project::load_project(&fixture.project_file()).unwrap();
    let target = &project.targets["mmio_test"];
    assert!(
        target
            .flags
            .iter()
            .any(|flag| flag.value == "-DCHERIOT_PRINT_FLOATS"
                && flag.origin.field.ends_with("print_floats"))
    );
    assert!(
        target
            .flags
            .iter()
            .any(|flag| flag.value == "-DCHERIOT_PRINT_DOUBLES"
                && flag.origin.field.ends_with("print_doubles"))
    );
    assert!(
        target
            .dependencies
            .iter()
            .any(|dependency| dependency == "softfloat")
    );
    let plan = planning::create_build_plan(&project).unwrap();
    assert!(
        plan.operations
            .iter()
            .any(|operation| operation.id.starts_with("compile.softfloat"))
    );
}
