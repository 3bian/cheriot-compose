// Copyright 3Bian Limited and Contributors
// SPDX-License-Identifier: MIT

#![cfg(feature = "rtos-verification")]

use std::path::Path;

use cheriot_compose::{
    build_graph::BuildOperationKind, calculations::sdk, compilation_database, configuration,
    planning,
};

use test_fixture::rtos_tree;

fn repository_root() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("application directory has a repository parent")
}

#[test]
fn repository_example_uses_the_integration_overlay_and_builds_a_plan() {
    let root = repository_root();
    let project = cheriot_compose::project::load_project(
        &root.join("examples/producer-consumer/cheriot-compose.toml"),
    )
    .unwrap();
    assert_eq!(project.sdk, root.join("cheriot-rtos/sdk"));
    assert_eq!(project.toolchain.directory, Path::new("/cheriot-tools/bin"));
    assert!(
        project
            .sdk
            .join("include/magic_enum/magic_enum.hpp")
            .is_file()
    );
    assert!(
        project
            .config_files
            .iter()
            .any(|path| path.ends_with("integrations/cheriot-rtos/sdk/cheriot-compose.toml"))
    );
    let plan = planning::create_build_plan(&project).unwrap();
    for name in ["compile.producer.0", "compile.consumer.0", "link.firmware"] {
        assert!(
            plan.operations.iter().any(|operation| operation.id == name),
            "{name}"
        );
    }
}

#[test]
fn vendored_examples_and_regressions_have_valid_plans() {
    let workspace = rtos_tree::RtosTree::new();
    let projects = [
        "benchmarks/allocation",
        "benchmarks/compartment-call",
        "benchmarks/interrupt-latency",
        "benchmarks/interrupt-latency-ext",
        "benchmarks/stack-usage",
        "examples/01.hello_world",
        "examples/02.hello_compartment",
        "examples/03.hello_safe_compartment",
        "examples/04.temporal_safety",
        "examples/05.sealing",
        "examples/06.producer-consumer",
        "examples/07.error_handling",
        "examples/08.memory_safety",
        "examples/09.javascript",
        "examples/10.audit",
        "exercises/01.compartmentalisation",
        "tests.extra/hardware_revoker_IRQs",
        "tests.extra/keep_ticking",
        "tests.extra/regress-double_ref_shared",
        "tests.extra/regress-thread_exit_IRQ",
    ];
    for name in projects {
        let directory = workspace.root.join(name);
        assert!(directory.join("xmake.lua").is_file(), "{name}");
        let project =
            cheriot_compose::project::load_project(&directory.join("cheriot-compose.toml"))
                .unwrap_or_else(|error| panic!("{name}: {error}"));
        let plan =
            planning::create_build_plan(&project).unwrap_or_else(|error| panic!("{name}: {error}"));
        assert!(
            plan.operations
                .iter()
                .any(|operation| operation.kind == BuildOperationKind::Compile)
        );
        assert!(
            plan.operations
                .iter()
                .any(|operation| operation.kind == BuildOperationKind::Link)
        );
        assert!(
            project
                .config_files
                .iter()
                .any(|path| path.ends_with("sdk/lib/queue/cheriot-compose.toml"))
        );
        let recipes = sdk::resolve_dependencies(&project).unwrap();
        assert!(recipes.iter().any(|recipe| recipe.name == "debug"));
        let flags = &recipes
            .iter()
            .find(|recipe| recipe.name == "debug")
            .unwrap()
            .flags;
        assert!(flags.iter().any(|flag| {
            flag.origin
                .file
                .ends_with("sdk/lib/debug/cheriot-compose.toml")
        }));
    }
}

#[test]
fn dependency_groups_contribute_their_members_without_a_fake_output() {
    let workspace = rtos_tree::RtosTree::new();
    let file = workspace
        .root
        .join("exercises/01.compartmentalisation/cheriot-compose.toml");
    let project = cheriot_compose::project::load_project(&file).unwrap();
    let plan = planning::create_build_plan(&project).unwrap();
    for member in ["atomic1", "atomic2", "atomic4", "atomic8", "atomiccap"] {
        assert!(
            plan.operations
                .iter()
                .any(|operation| operation.id == format!("compile.{member}.0")),
            "{member}"
        );
    }
    assert!(
        !plan
            .operations
            .iter()
            .any(|operation| operation.id.contains("atomic_fixed"))
    );
    let compile = plan
        .operations
        .iter()
        .find(|operation| operation.id == "compile.js.0")
        .unwrap();
    let cheriot_compose::build_graph::BuildAction::Command { command, .. } = &compile.action else {
        panic!("expected exercise compile command")
    };
    assert!(command.argv.windows(2).any(|arguments| {
        arguments[0].value == "-include"
            && arguments[1].value
                == project
                    .sdk
                    .join("include/allocator.h")
                    .display()
                    .to_string()
    }));
    assert!(command.argv.iter().any(|argument| argument.value == "-O0"));
    let compile = plan
        .operations
        .iter()
        .find(|operation| operation.id == "compile.js.1")
        .unwrap();
    let cheriot_compose::build_graph::BuildAction::Command { command, .. } = &compile.action else {
        panic!("expected exercise compile command")
    };
    assert!(
        !command
            .argv
            .iter()
            .any(|argument| argument.value.ends_with("/allocator.h"))
    );
    assert!(!command.argv.iter().any(|argument| argument.value == "-O0"));
}

#[test]
fn hardware_revoker_target_uses_its_selected_board_and_microvium_keeps_c_flags() {
    let workspace = rtos_tree::RtosTree::new();
    let rtos = &workspace.root;
    let project = cheriot_compose::project::load_project(
        &rtos.join("tests.extra/hardware_revoker_IRQs/cheriot-compose.toml"),
    )
    .unwrap();
    assert_eq!(project.board.revocable_start, 0x20040000);
    assert!(project.targets["top"].flags.iter().any(|flag| {
        flag.value == "-DREVOKABLE_MEMORY_START=0x20040000"
            && flag.origin.field.ends_with("/revocable_memory")
    }));
    let plan = planning::create_build_plan(&project).unwrap();
    let allocator = plan
        .operations
        .iter()
        .find(|operation| operation.id == "compile.cheriot.allocator.0")
        .unwrap();
    let cheriot_compose::build_graph::BuildAction::Command {
        command: allocator_command,
        ..
    } = &allocator.action
    else {
        panic!("expected allocator compile command")
    };
    assert!(
        allocator_command
            .argv
            .iter()
            .any(|argument| argument.value == format!("-iquote{}", project.build_dir.display()))
    );
    let compile = plan
        .operations
        .iter()
        .find(|operation| operation.id == "compile.top.0")
        .unwrap();
    let cheriot_compose::build_graph::BuildAction::Command { command, .. } = &compile.action else {
        panic!("expected compile command")
    };
    assert!(
        command
            .argv
            .iter()
            .any(|argument| argument.value == "-mcpu=cheriot-ibex")
    );

    let project = cheriot_compose::project::load_project(
        &rtos.join("examples/09.javascript/cheriot-compose.toml"),
    )
    .unwrap();
    let plan = planning::create_build_plan(&project).unwrap();
    let compile = plan
        .operations
        .iter()
        .find(|operation| operation.id == "compile.microvium.0")
        .unwrap();
    let cheriot_compose::build_graph::BuildAction::Command { command, .. } = &compile.action else {
        panic!("expected compile command")
    };
    let index = command
        .argv
        .iter()
        .position(|argument| argument.value == "-Wno-constant-logical-operand")
        .unwrap();
    assert!(
        command.argv[index].origins[0]
            .file
            .ends_with("sdk/lib/microvium/cheriot-compose.toml")
    );
}

#[test]
fn modular_producer_consumer_matches_the_archived_compile_commands() {
    let workspace = rtos_tree::RtosTree::new();
    let rtos = &workspace.root;
    let example = rtos.join("examples/06.producer-consumer");
    let project =
        cheriot_compose::project::load_project(&example.join("cheriot-compose.toml")).unwrap();
    let unique: std::collections::BTreeSet<_> = project.config_files.iter().collect();
    assert_eq!(unique.len(), project.config_files.len());
    assert!(
        project.provenance["/services/producer/build/sources/0"]
            .file
            .ends_with("components/cheriot-compose.toml")
    );
    assert!(
        project.provenance["/sdk_components/message_queue/build/sources/0"]
            .file
            .ends_with("sdk/lib/queue/cheriot-compose.toml")
    );
    let plan = planning::create_build_plan(&project).unwrap();
    let mut probes = plan.unresolved_probes();
    for value in probes.values_mut() {
        *value = Some(true);
    }
    let mut actual = compilation_database::export_compilation_database(&plan, &probes).unwrap();
    assert_eq!(actual.len(), 29);
    for entry in &mut actual {
        let index = entry
            .arguments
            .iter()
            .position(|arg| arg == "-MMD")
            .unwrap();
        entry.arguments.drain(index..index + 3);
    }
    let mut reference = compilation_database::read_compilation_database(
        &repository_root().join("verification/reference/producer-consumer-reachable.json"),
    )
    .unwrap();
    let old = "/workspaces/cheriot-rtos/examples/06.producer-consumer";
    let maps = vec![
        compilation_database::PathMapping {
            old: format!("{old}/build"),
            new: project.build_dir.display().to_string(),
        },
        compilation_database::PathMapping {
            old: old.into(),
            new: example.display().to_string(),
        },
        compilation_database::PathMapping {
            old: "/workspaces/cheriot-rtos/sdk".into(),
            new: rtos.join("sdk").display().to_string(),
        },
        compilation_database::PathMapping {
            old: "/cheriot-tools/bin/clang".into(),
            new: "clang".into(),
        },
        compilation_database::PathMapping {
            old: "/cheriot-tools/bin/clang++".into(),
            new: "clang++".into(),
        },
    ];
    test_fixture::align_reference_object_paths(&mut reference, &actual, &maps);
    let report = compilation_database::compare_compilation_databases(&reference, &actual, &maps);
    assert!(
        report.is_match(),
        "{}",
        cheriot_compose::render::compilation_database::format_comparison_report(&report)
    );
}

#[test]
fn microvium_c_compile_matches_its_archived_command() {
    let workspace = rtos_tree::RtosTree::new();
    let rtos = &workspace.root;
    let example = rtos.join("examples/09.javascript");
    let project =
        cheriot_compose::project::load_project(&example.join("cheriot-compose.toml")).unwrap();
    let plan = planning::create_build_plan(&project).unwrap();
    let mut probes = plan.unresolved_probes();
    for result in probes.values_mut() {
        *result = Some(true);
    }
    let mut actual = compilation_database::export_compilation_database(&plan, &probes).unwrap();
    actual.retain(|entry| entry.file.ends_with("/microvium.c"));
    assert_eq!(actual.len(), 1);
    let index = actual[0]
        .arguments
        .iter()
        .position(|arg| arg == "-MMD")
        .unwrap();
    actual[0].arguments.drain(index..index + 3);
    let mut reference = compilation_database::read_compilation_database(
        &repository_root()
            .join("verification/reference/examples/09.javascript/compile_commands.json"),
    )
    .unwrap();
    reference.retain(|entry| entry.file.ends_with("/microvium.c"));
    assert_eq!(reference.len(), 1);
    let old = "/workspaces/cheriot-rtos/examples/09.javascript";
    let maps = vec![
        compilation_database::PathMapping {
            old: format!("{old}/build"),
            new: project.build_dir.display().to_string(),
        },
        compilation_database::PathMapping {
            old: old.into(),
            new: example.display().to_string(),
        },
        compilation_database::PathMapping {
            old: "/workspaces/cheriot-rtos/sdk".into(),
            new: rtos.join("sdk").display().to_string(),
        },
        compilation_database::PathMapping {
            old: "/cheriot-tools/bin/clang".into(),
            new: "clang".into(),
        },
    ];
    test_fixture::align_reference_object_paths(&mut reference, &actual, &maps);
    let report = compilation_database::compare_compilation_databases(&reference, &actual, &maps);
    assert!(
        report.is_match(),
        "{}",
        cheriot_compose::render::compilation_database::format_comparison_report(&report)
    );
}

#[test]
fn compose_command_finds_project_configuration_in_the_current_directory() {
    let workspace = rtos_tree::RtosTree::new();
    for directory in [
        "examples/06.producer-consumer",
        "tests.extra/regress-double_ref_shared",
    ] {
        let output = std::process::Command::new(env!("CARGO_BIN_EXE_cheriot-compose"))
            .arg("check")
            .current_dir(workspace.root.join(directory))
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{directory}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

#[path = "support/test_fixture.rs"]
mod test_fixture;

#[test]
fn native_board_variants_and_ordered_sram_overlay_resolve() {
    let workspace = rtos_tree::RtosTree::new();
    let sdk = workspace.root.join("sdk");
    let sdk_string = serde_json::to_string(&sdk.display().to_string()).unwrap();
    for name in [
        "sail",
        "sail-ibex",
        "ibex-safe-simulator",
        "ibex-arty-a7-100",
        "kudu-safe-simulator",
        "sonata-0.2",
        "sonata-1.0",
        "sonata-1.1",
        "sonata-1.3",
        "sonata-prerelease",
        "sonata-simulator",
        "sonata",
    ] {
        let fixture = test_fixture::TestFixture::new();
        fixture.replace_text(
            "cheriot-compose.toml",
            "sdk = \"./sdk\"",
            &format!("sdk = {sdk_string}"),
        );
        fixture.replace_text(
            "cheriot-compose.toml",
            "board = \"./boards/sail.toml\"",
            &format!("board = \"{name}\""),
        );
        let project = cheriot_compose::project::load_project(&fixture.project_file())
            .unwrap_or_else(|error| panic!("board {name}: {error}"));
        assert_eq!(project.board.name, name);
        if name == "ibex-safe-simulator" || name == "kudu-safe-simulator" {
            let runner = project.board.runner.as_ref().unwrap();
            assert_eq!(runner.kind, configuration::RunnerKind::MicrosoftSafe);
            assert_eq!(
                runner.executable,
                format!("cheriot_{}_safe_sim", name.split('-').next().unwrap())
            );
            assert!(runner.arguments.is_empty());
            assert_eq!(
                project.board.packaging,
                Some(configuration::PackagingKind::MicrosoftSafe)
            );
        }
        if name == "ibex-arty-a7-100" {
            assert!(project.board.runner.is_none());
        }
        let plan = planning::create_build_plan(&project)
            .unwrap_or_else(|error| panic!("board {name}: {error}"));
        assert!(
            plan.operations
                .iter()
                .any(|operation| operation.id == "link.firmware")
        );
    }
    let fixture = test_fixture::TestFixture::new();
    fixture.replace_text(
        "cheriot-compose.toml",
        "sdk = \"./sdk\"",
        &format!("sdk = {sdk_string}"),
    );
    fixture.replace_text(
        "cheriot-compose.toml",
        "board = \"./boards/sail.toml\"",
        "board = \"sonata-1.1\"\nboard_overlays = [\"${sdk}/boards/sonata-1.x-sram-mixin.toml\"]",
    );
    let project = fixture.resolved_project();
    assert!(project.board.data_memory.is_none());
    assert_eq!(project.board.instruction_memory.start, 0x00101000);
    assert_eq!(project.board.instruction_memory.end, 0x00120000);
    assert!(
        project.provenance["/board/instruction_memory/start"]
            .file
            .ends_with("sonata-1.x-sram-mixin.toml")
    );
}

#[test]
fn benchmark_settings_flow_into_each_affected_component() {
    let tree = rtos_tree::RtosTree::new();
    let overlay = tree.root.join("benchmark-overrides.toml");
    std::fs::write(&overlay, "patches = [{ op = \"add\", path = \"/options\", value = { benchmark_metric = \"rdinstret\", component_debug = { caller = true } } }]\n").unwrap();
    let settings = configuration::ProjectSettings {
        overlays: vec![overlay.clone()],
        ..Default::default()
    };
    let project = cheriot_compose::project::load_project_with_settings(
        &tree
            .root
            .join("benchmarks/compartment-call/cheriot-compose.toml"),
        None,
        &settings,
    )
    .unwrap();
    for target in ["libcallee", "callee", "caller"] {
        assert!(
            project.targets[target]
                .flags
                .iter()
                .any(|flag| flag.value == "-DMETRIC=rdinstret"),
            "{target}"
        );
    }
    let caller = &project.targets["caller"].flags;
    assert!(caller.iter().any(|flag| flag.value == "-DBOARD=sail"));
    assert!(
        caller
            .iter()
            .any(|flag| flag.value == "-DDEBUG_CALLER=true")
    );
    assert!(
        caller
            .iter()
            .find(|flag| flag.value == "-DMETRIC=rdinstret")
            .unwrap()
            .origin
            .rule
            .contains("benchmark-overrides.toml")
    );
}

#[test]
fn sonata_alias_inherits_the_full_board_definition() {
    let tree = rtos_tree::RtosTree::new();
    let file = tree
        .root
        .join("examples/01.hello_world/cheriot-compose.toml");
    let original = std::fs::read_to_string(&file).unwrap();
    let load = |board: &str| {
        let document = original.replace(
            "../../sdk/boards/sail.toml",
            &format!("../../sdk/boards/{board}.toml"),
        );
        std::fs::write(&file, document).unwrap();
        let mut resolved =
            serde_json::to_value(cheriot_compose::project::load_project(&file).unwrap().board)
                .unwrap();
        resolved.as_object_mut().unwrap().remove("name");
        resolved
    };
    assert_eq!(load("sonata"), load("sonata-1.1"));
}
