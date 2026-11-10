// Copyright 3Bian Limited and Contributors
// SPDX-License-Identifier: MIT

use cheriot_compose::configuration::RawMemoryExtent;
use test_fixture::TestFixture;

#[path = "support/test_fixture.rs"]
mod test_fixture;

#[test]
fn producer_consumer_calculations() {
    let fixture = TestFixture::new();
    let project = fixture.resolved_project();
    assert_eq!(
        project
            .threads
            .iter()
            .map(|thread| thread.priority)
            .collect::<Vec<_>>(),
        [0, 0]
    );
    assert_eq!(project.threads[0].id, 1);
    assert_eq!(project.threads[1].id, 2);
    assert_eq!(project.threads[0].entry_symbol, "__export_producer__Z3runv");
    assert_eq!(project.threads[0].requested.stack_size, 1280);
    assert_eq!(project.shared_objects["allocator_epoch"], 4);
    assert_eq!(project.shared_objects["allocator_hazard_pointers"], 32);
    assert_eq!(project.board.mmio_start, 0x2000000);
    assert_eq!(project.board.mmio_end, 0x83001000);
    assert_eq!(project.board.revocable_start, 0x80000000);
    assert_eq!(project.board.heap_start, ".");
}

#[test]
fn output_namespace_tracks_where_relative_inputs_are_declared() {
    let fixture = TestFixture::new();
    fixture.replace_text(
        "cheriot-compose.toml",
        "[services.producer]\nbuild = { sources = [\"producer.cc\"] }\nkind = \"compartment\"\n\n",
        "",
    );
    fixture.replace_text(
        "cheriot-compose.toml",
        "include = [\"./sdk/cheriot-compose.toml\"]",
        "include = [\"./sdk/cheriot-compose.toml\", \"./parts/a/component.toml\"]",
    );
    let component =
        "[services.producer]\nbuild = { sources = [\"producer.cc\"] }\nkind = \"compartment\"\n";
    fixture.write_file("parts/a/component.toml", component);
    fixture.write_file("parts/b/component.toml", component);
    fixture.write_file("parts/a/producer.cc", "// source a\n");
    fixture.write_file("parts/b/producer.cc", "// source b\n");
    let first = fixture.resolved_project();
    fixture.write_file(
        "parts/a/component.toml",
        &format!("# comment only\n{component}"),
    );
    assert_eq!(first.build_dir, fixture.resolved_project().build_dir);
    fixture.replace_text(
        "cheriot-compose.toml",
        "./parts/a/component.toml",
        "./parts/b/component.toml",
    );
    let second = fixture.resolved_project();
    assert_eq!(first.build_dir, second.build_dir);
    assert_ne!(first.configuration_identity, second.configuration_identity);
    assert_ne!(
        first.targets["producer"].sources[0].path,
        second.targets["producer"].sources[0].path
    );
}

#[test]
fn component_and_source_compilers_and_assembly_flags_are_explicit() {
    use cheriot_compose::build_graph::BuildAction;

    let fixture = TestFixture::new();
    fixture.write_file("driver.S", "/* assembly fixture */\n");
    fixture.replace_text(
        "cheriot-compose.toml",
        "build = { sources = [\"producer.cc\"] }",
        "build = { compiler = \"./tools/target-compiler\", assembly_flags = [\"-DASSEMBLY_TARGET\"], sources = [{ path = \"producer.cc\", compiler = \"./tools/source-compiler\" }, { path = \"driver.S\", assembly_flags = [\"-DASSEMBLY_SOURCE\"] }] }",
    );
    let plan = fixture.build_plan();
    let command = |id: &str| {
        let operation = plan.operations.iter().find(|item| item.id == id).unwrap();
        let BuildAction::Command { command, .. } = &operation.action else {
            panic!("expected command")
        };
        command
    };
    let cpp = command("compile.producer.0");
    assert_eq!(
        cpp.executable,
        fixture
            .directory
            .join("tools/source-compiler")
            .display()
            .to_string()
    );
    assert!(
        !cpp.argv
            .iter()
            .any(|argument| argument.value == "-DASSEMBLY_TARGET")
    );
    let assembly = command("compile.producer.1");
    assert_eq!(
        assembly.executable,
        fixture
            .directory
            .join("tools/target-compiler")
            .display()
            .to_string()
    );
    let values: Vec<_> = assembly
        .argv
        .iter()
        .map(|argument| argument.value.as_str())
        .collect();
    let target = values
        .iter()
        .position(|value| *value == "-DASSEMBLY_TARGET")
        .unwrap();
    let source = values
        .iter()
        .position(|value| *value == "-DASSEMBLY_SOURCE")
        .unwrap();
    assert!(target < source);
    assert!(
        assembly.argv[source].origins[0]
            .field
            .contains("assembly_flags")
    );
}

#[test]
fn board_substitutions_follow_selected_and_base_board_directories() {
    let fixture = TestFixture::new();
    let base = std::fs::read_to_string(fixture.directory.join("boards/sail.toml"))
        .unwrap()
        .replace(
            "driver_includes = [\n    \"${sdk}/include/platform/sail\",\n    \"${sdk}/include/platform/generic-riscv\",\n]",
            "driver_includes = [\"${board}/drivers\", \"${boardBase}/drivers\", \"${sdkboards}/drivers\", \"${board_dir}/drivers\"]",
        );
    fixture.write_file("sdk/boards/bases/base.toml", &base);
    for directory in [
        "boards/selected/drivers",
        "sdk/boards/bases/drivers",
        "sdk/boards/drivers",
    ] {
        std::fs::create_dir_all(fixture.directory.join(directory)).unwrap();
    }
    fixture.write_file(
        "boards/selected/board.toml",
        "extends = \"${sdkboards}/bases/base.toml\"\nname = \"selected\"\n",
    );
    fixture.write_file(
        "boards/selected/overlay.toml",
        "patches = [{ op = \"add\", path = \"/defines/-\", value = \"SELECTED_OVERLAY\" }]\n",
    );
    fixture.write_file(
        "sdk/boards/bases/overlay.toml",
        "patches = [{ op = \"add\", path = \"/defines/-\", value = \"BASE_OVERLAY\" }]\n",
    );
    fixture.replace_text(
        "cheriot-compose.toml",
        "board = \"./boards/sail.toml\"",
        "board = \"./boards/selected/board.toml\"\nboard_overlays = [\"${board}/overlay.toml\", \"${boardBase}/overlay.toml\"]",
    );
    let project = fixture.resolved_project();
    let paths: Vec<_> = project
        .board
        .includes
        .iter()
        .map(|include| &include.value)
        .collect();
    assert_eq!(paths[0], &fixture.directory.join("boards/selected/drivers"));
    assert_eq!(
        paths[1],
        &fixture.directory.join("sdk/boards/bases/drivers")
    );
    assert_eq!(paths[2], &fixture.directory.join("sdk/boards/drivers"));
    assert_eq!(
        paths[3],
        &fixture.directory.join("sdk/boards/bases/drivers")
    );
    assert!(
        project
            .board
            .defines
            .ends_with(&["SELECTED_OVERLAY".into(), "BASE_OVERLAY".into()])
    );
}

#[test]
fn core_component_roles_are_explicit_and_reserved() {
    use cheriot_compose::configuration::ComponentRole;

    let fixture = TestFixture::new();
    let project = fixture.resolved_project();
    for (name, role) in [
        ("cheriot.allocator", ComponentRole::Allocator),
        ("cheriot.loader", ComponentRole::Loader),
        ("cheriot.switcher", ComponentRole::Switcher),
        ("cheriot.software_revoker", ComponentRole::SoftwareRevoker),
        ("cheriot.token_library", ComponentRole::TokenLibrary),
    ] {
        assert!(
            project
                .components
                .iter()
                .any(|component| component.name == name && component.role == Some(role))
        );
    }
    assert!(
        project
            .components
            .iter()
            .any(|component| component.role == Some(ComponentRole::Scheduler))
    );
    let mut document = std::fs::read_to_string(fixture.project_file()).unwrap();
    document.push_str("\n[sdk_components.spoof]\nkind = \"library\"\nrole = \"loader\"\nbuild.sources = [\"producer.cc\"]\n");
    fixture.write_file("cheriot-compose.toml", &document);
    assert!(cheriot_compose::project::load_project(&fixture.project_file()).is_err());
}

#[test]
fn dense_priorities_and_zero_threads() {
    let fixture = TestFixture::new();
    let original = std::fs::read_to_string(fixture.project_file()).unwrap();
    let prefix = original.split("[[firmwares.threads]]").next().unwrap();
    let mut definitions = String::new();
    for (index, priority) in [2, 5, 5, 9].into_iter().enumerate() {
        definitions.push_str(&format!("{{ name = \"thread{index}\", compartment = \"producer\", entry = \"run\", priority = {priority}, stack_size = 512, trusted_stack_frames = 1 }},\n"));
    }
    fixture.write_file(
        "cheriot-compose.toml",
        &format!("{prefix}threads = [\n{definitions}]\n"),
    );
    let project = fixture.resolved_project();
    assert_eq!(
        project
            .threads
            .iter()
            .map(|thread| thread.priority)
            .collect::<Vec<_>>(),
        [0, 1, 1, 2]
    );
    fixture.write_file("cheriot-compose.toml", &format!("{prefix}threads = []\n"));
    let zero = fixture.resolved_project();
    assert_eq!(zero.max_priority, 0);
    assert_eq!(zero.shared_objects["allocator_hazard_pointers"], 0);
}

#[test]
fn toml_errors_retain_file_line_and_field() {
    for (from, to, expected) in [
        ("simulation = true", "simulation = yes", "simulation"),
        ("timer_hz = 2000", "timer_hz = 1.5", "timer_hz"),
        (
            "simulation = true",
            "simulation = true\nsimulation = false",
            "duplicate",
        ),
        ("timer_hz = 2000", "timer_hz = -1", "timer_hz"),
        (
            "simulation = true",
            "simulation = true\nmisspelled = 1",
            "misspelled",
        ),
    ] {
        let fixture = TestFixture::new();
        fixture.replace_text("boards/sail.toml", from, to);
        let error = cheriot_compose::project::load_project(&fixture.project_file()).unwrap_err();
        let origin = error.origin.as_ref().unwrap();
        assert!(origin.line > 0 && origin.column > 0);
        assert!(origin.file.ends_with("sail.toml"));
        assert!(
            error.to_string().to_lowercase().contains(expected),
            "{error}"
        );
    }
    let fixture = TestFixture::new();
    fixture.replace_text(
        "cheriot-compose.toml",
        "stack_size = 0x500",
        "stack_szie = 0x500",
    );
    let error = cheriot_compose::project::load_project(&fixture.project_file()).unwrap_err();
    assert!(error.to_string().contains("stack_szie"));
    assert!(error.origin.unwrap().field.contains("threads/0"));
}

#[test]
fn extent_bounds() {
    for extent in [
        RawMemoryExtent {
            start: u32::MAX,
            end: None,
            length: Some(1),
        },
        RawMemoryExtent {
            start: 9,
            end: Some(1),
            length: None,
        },
        RawMemoryExtent {
            start: 9,
            end: Some(9),
            length: None,
        },
        RawMemoryExtent {
            start: 1,
            end: Some(5),
            length: Some(3),
        },
        RawMemoryExtent {
            start: 1,
            end: None,
            length: None,
        },
    ] {
        assert!(cheriot_compose::calculations::board::resolve_memory_extent(&extent).is_err());
    }
    assert_eq!(
        cheriot_compose::calculations::board::resolve_memory_extent(&RawMemoryExtent {
            start: 1,
            end: None,
            length: Some(4)
        })
        .unwrap()
        .end,
        5
    );
}

#[test]
fn stack_limit_and_symbol_validation() {
    let fixture = TestFixture::new();
    fixture.replace_text(
        "cheriot-compose.toml",
        "stack_size = 0x500",
        "stack_size = 65280",
    );
    assert!(cheriot_compose::project::load_project(&fixture.project_file()).is_ok());
    fixture.replace_text(
        "cheriot-compose.toml",
        "stack_size = 65280",
        "stack_size = 65281",
    );
    assert!(
        cheriot_compose::project::load_project(&fixture.project_file())
            .unwrap_err()
            .to_string()
            .contains("65,280")
    );
    let fixture = TestFixture::new();
    fixture.replace_text(
        "cheriot-compose.toml",
        "entry = \"run\"",
        "entry = \"ns::run\"",
    );
    assert!(cheriot_compose::project::load_project(&fixture.project_file()).is_err());
}

#[test]
fn shared_conflicts_and_reachability() {
    let fixture = TestFixture::new();
    fixture.replace_text(
        "cheriot-compose.toml",
        "[[firmwares]]",
        "[[firmwares]]\nshared_objects = { allocator_epoch = 8 }",
    );
    assert!(
        cheriot_compose::project::load_project(&fixture.project_file())
            .unwrap_err()
            .to_string()
            .contains("conflicting sizes")
    );
    let fixture = TestFixture::new();
    fixture.replace_text(
        "cheriot-compose.toml",
        "services = [\"producer\", \"consumer\"]",
        "services = [\"consumer\"]",
    );
    assert!(
        cheriot_compose::project::load_project(&fixture.project_file())
            .unwrap_err()
            .to_string()
            .contains("not a reachable")
    );
}

#[test]
fn inheritance_patches_and_origins() {
    let fixture = TestFixture::new();
    fixture.write_file("boards/derived.toml", "extends = \"sail.toml\"\nname = \"derived\"\npatches = [\n  { op = \"replace\", path = \"/simulation\", value = false },\n  { op = \"remove\", path = \"/defines/0\" },\n  { op = \"add\", path = \"/defines/-\", value = \"EXTRA\" },\n  { op = \"replace\", path = \"/heap/start\", value = 1 },\n]\n");
    fixture.replace_text(
        "cheriot-compose.toml",
        "boards/sail.toml",
        "boards/derived.toml",
    );
    assert!(
        cheriot_compose::project::load_project(&fixture.project_file())
            .unwrap_err()
            .to_string()
            .contains("patch path")
    );
    fixture.replace_text(
        "boards/derived.toml",
        "  { op = \"replace\", path = \"/heap/start\", value = 1 },\n",
        "",
    );
    let project = fixture.resolved_project();
    assert!(!project.board.simulation);
    assert_eq!(project.board.defines, ["RISCV_HTIF", "EXTRA"]);
    assert!(
        project.provenance["/board/devices/uart/start"]
            .file
            .ends_with("sail.toml")
    );
    assert!(
        project.provenance["/board/simulation"]
            .file
            .ends_with("derived.toml")
    );
    assert_eq!(project.provenance["/board/simulation"].line, 4);
}

#[test]
fn inheritance_dependency_and_include_cycles() {
    let fixture = TestFixture::new();
    fixture.replace_text(
        "boards/sail.toml",
        "name = \"sail\"",
        "name = \"sail\"\nextends = \"sail.toml\"",
    );
    assert!(
        cheriot_compose::project::load_project(&fixture.project_file())
            .unwrap_err()
            .to_string()
            .contains("cycle")
    );
    let fixture = TestFixture::new();
    fixture.replace_text(
        "cheriot-compose.toml",
        "[services.producer]",
        "[services.producer]\ndependencies = [\"producer\"]",
    );
    assert!(
        cheriot_compose::project::load_project(&fixture.project_file())
            .unwrap_err()
            .to_string()
            .contains("cycle")
    );
    let fixture = TestFixture::new();
    fixture.write_file("parts/a.toml", "include = [\"b.toml\"]\n");
    fixture.write_file("parts/b.toml", "include = [\"a.toml\"]\n");
    fixture.replace_text(
        "cheriot-compose.toml",
        "include = [",
        "include = [\"parts/a.toml\", ",
    );
    assert!(
        cheriot_compose::project::load_project(&fixture.project_file())
            .unwrap_err()
            .to_string()
            .contains("cycle")
    );
}

#[test]
fn nested_includes_preserve_paths_and_reject_conflicts() {
    let fixture = TestFixture::new();
    fixture.write_file("parts/common.toml", "[sdk_components.local_sdk]\nkind = \"library\"\nbuild = { sources = [\"local.c\"], c_flags = [\"-Wno-unused\"] }\n");
    fixture.write_file("parts/local.c", "/* fixture */\n");
    fixture.write_file("parts/a.toml", "include = [\"common.toml\"]\n");
    fixture.write_file("parts/b.toml", "include = [\"common.toml\"]\n");
    fixture.replace_text(
        "cheriot-compose.toml",
        "include = [",
        "include = [\"parts/a.toml\", \"parts/b.toml\", ",
    );
    let project = fixture.resolved_project();
    assert_eq!(
        project.targets["local_sdk"].sources[0].path,
        fixture.directory.join("parts/local.c")
    );
    assert_eq!(project.targets["local_sdk"].c_flags[0].value, "-Wno-unused");
    assert!(
        project.targets["local_sdk"].c_flags[0]
            .origin
            .file
            .ends_with("common.toml")
    );
    assert_eq!(
        project
            .config_files
            .iter()
            .filter(|path| path.ends_with("common.toml"))
            .count(),
        1
    );
    fixture.write_file("parts/b.toml", "include = [\"common.toml\"]\n[sdk_components.local_sdk]\nkind = \"library\"\nbuild = { sources = [\"local.c\"] }\n");
    assert!(
        cheriot_compose::project::load_project(&fixture.project_file())
            .unwrap_err()
            .to_string()
            .contains("duplicate included definition")
    );
}

#[test]
fn included_documents_use_structure_without_schema_markers() {
    let fixture = TestFixture::new();
    fixture.write_file(
        "parts/missing.toml",
        "[libraries.example]\nkind = \"group\"\nbuild.sources = []\n",
    );
    fixture.replace_text(
        "cheriot-compose.toml",
        "include = [",
        "include = [\"parts/missing.toml\", ",
    );
    assert!(fixture.resolved_project().targets.contains_key("example"));
    fixture.replace_text(
        "parts/missing.toml",
        "[libraries.example]",
        "schema = \"obsolete\"\n[libraries.example]",
    );
    let error = cheriot_compose::project::load_project(&fixture.project_file()).unwrap_err();
    assert!(error.to_string().contains("included files may define only"));
    assert!(error.origin.unwrap().file.ends_with("missing.toml"));
}

#[test]
fn missing_sources_and_tool_paths() {
    let fixture = TestFixture::new();
    fixture.replace_text("cheriot-compose.toml", "producer.cc", "absent.cc");
    assert!(
        cheriot_compose::project::load_project(&fixture.project_file())
            .unwrap_err()
            .to_string()
            .contains("source file does not exist")
    );
    let fixture = TestFixture::new();
    fixture.set_toolchain_directory(&fixture.directory.join("tools"));
    assert_eq!(
        fixture.resolved_project().toolchain.c_compiler,
        fixture.directory.join("tools/clang").display().to_string()
    );
}

#[test]
fn missing_include_directories_are_reported_at_their_configuration() {
    let fixture = TestFixture::new();
    fixture.replace_text(
        "cheriot-compose.toml",
        "build = { sources = [\"producer.cc\"] }",
        "build = { sources = [\"producer.cc\"], includes = [\"missing-headers\"] }",
    );
    let error = cheriot_compose::project::load_project(&fixture.project_file()).unwrap_err();
    assert!(
        error
            .to_string()
            .contains("include directory does not exist")
    );
    assert!(error.origin.unwrap().field.contains("includes"));
    let fixture = TestFixture::new();
    fixture.replace_text(
        "boards/sail.toml",
        "${sdk}/include/platform/sail",
        "${sdk}/missing-board-headers",
    );
    let error = cheriot_compose::project::load_project(&fixture.project_file()).unwrap_err();
    assert!(
        error
            .to_string()
            .contains("board include directory does not exist")
    );
    assert!(error.origin.unwrap().file.ends_with("sail.toml"));
}

#[test]
fn revocation_and_heap_precedence() {
    let fixture = TestFixture::new();
    fixture.replace_text(
        "boards/sail.toml",
        "simulation = true",
        "simulation = true\ndata_memory = { start = 0x90000000, end = 0x90040000 }",
    );
    fixture.replace_text(
        "boards/sail.toml",
        "[heap]\nend = 2147745792",
        "[heap]\nend = 0x90040000",
    );
    assert_eq!(fixture.resolved_project().board.revocable_start, 0x90000000);
    fixture.replace_text(
        "boards/sail.toml",
        "revoker = \"software\"",
        "revoker = \"software\"\nrevokable_memory_start = 0x70000000",
    );
    assert_eq!(fixture.resolved_project().board.revocable_start, 0x70000000);
    let fixture = TestFixture::new();
    fixture.replace_text(
        "boards/sail.toml",
        "[heap]\nend = 2147745792",
        "[heap]\nstart = 0x80020000\nend = 0x80040000",
    );
    assert_eq!(fixture.resolved_project().board.heap_start, "0x80020000");
}

#[test]
fn multiple_firmware_targets_have_separate_plans_and_output_directories() {
    let fixture = TestFixture::new();
    let mut source = std::fs::read_to_string(fixture.project_file()).unwrap();
    source.push_str("\n[[firmwares]]\nname = \"other.image\"\nservices = [\"producer\"]\nlibraries = [\"debug\"]\nthreads = [{ name = \"main\", compartment = \"producer\", entry = \"run\", priority = 3, stack_size = 0x400, trusted_stack_frames = 1 }]\n");
    fixture.write_file("cheriot-compose.toml", &source);
    assert!(
        cheriot_compose::project::load_project(&fixture.project_file())
            .unwrap_err()
            .to_string()
            .contains("select one")
    );
    let first = cheriot_compose::project::load_project_for_firmware(
        &fixture.project_file(),
        Some("producer-consumer"),
    )
    .unwrap();
    let second = cheriot_compose::project::load_project_for_firmware(
        &fixture.project_file(),
        Some("other.image"),
    )
    .unwrap();
    assert_eq!(first.threads.len(), 2);
    assert_eq!(second.threads.len(), 1);
    assert_eq!(first.build_dir, second.build_dir);
    assert_ne!(first.publication_identity, second.publication_identity);
    assert!(second.build_dir.ends_with("sail/cheriot/release"));
    for project in [&first, &second] {
        for recipe in cheriot_compose::calculations::sdk::resolve_dependencies(project).unwrap() {
            for source in recipe.sources {
                let source = source.path;
                if !source.exists() {
                    std::fs::create_dir_all(source.parent().unwrap()).unwrap();
                    std::fs::write(source, "/* fixture */\n").unwrap();
                }
            }
            let script = project.sdk.join(recipe.script);
            std::fs::create_dir_all(script.parent().unwrap()).unwrap();
            std::fs::write(script, "/* fixture */\n").unwrap();
        }
    }
    let first_plan = cheriot_compose::planning::create_build_plan(&first).unwrap();
    let second_plan = cheriot_compose::planning::create_build_plan(&second).unwrap();
    assert!(
        first_plan
            .operations
            .iter()
            .any(|operation| operation.id.contains("producer-consumer.scheduler"))
    );
    assert!(
        second_plan
            .operations
            .iter()
            .any(|operation| operation.id.contains("other.image.scheduler"))
    );
    let first_outputs: std::collections::BTreeSet<_> = first_plan
        .operations
        .iter()
        .filter(|operation| operation.id.starts_with("compile."))
        .flat_map(|operation| &operation.outputs)
        .collect();
    assert!(
        second_plan
            .operations
            .iter()
            .filter(|operation| operation.id.starts_with("compile."))
            .flat_map(|operation| &operation.outputs)
            .all(|output| !first_outputs.contains(output))
    );
    assert_ne!(
        first_plan.publication.as_ref().unwrap().elf,
        second_plan.publication.as_ref().unwrap().elf
    );
    assert!(
        cheriot_compose::project::load_project_for_firmware(
            &fixture.project_file(),
            Some("missing")
        )
        .unwrap_err()
        .to_string()
        .contains("unknown firmware")
    );
    source = source.replace("name = \"other.image\"", "name = \"producer-consumer\"");
    fixture.write_file("cheriot-compose.toml", &source);
    assert!(
        cheriot_compose::project::load_project_for_firmware(
            &fixture.project_file(),
            Some("producer-consumer")
        )
        .unwrap_err()
        .to_string()
        .contains("duplicate firmware")
    );
}

#[test]
fn build_directory_changes_with_effective_configuration() {
    let fixture = TestFixture::new();
    let original = fixture.resolved_project().build_dir;
    fixture.replace_text(
        "cheriot-compose.toml",
        "mode = \"release\"",
        "mode = \"debug\"",
    );
    let debug_project = fixture.resolved_project();
    let debug = debug_project.build_dir.clone();
    assert_ne!(original, debug);
    fixture.replace_text(
        "boards/sail.toml",
        "simulation = true",
        "simulation = false",
    );
    let board_change = fixture.resolved_project();
    assert_eq!(debug, board_change.build_dir);
    assert_ne!(
        debug_project.configuration_identity,
        board_change.configuration_identity
    );
}

#[test]
fn rtos_options_resolve_dependencies_and_compiler_defines() {
    let fixture = TestFixture::new();
    fixture.replace_text("cheriot-compose.toml", "mode = \"release\"", "mode = \"release\"\noptions = { allocator_rendering = true, scheduler_accounting = true, scheduler_multiwaiter = false, debug_allocator = \"warning\", debug_loader = true, debug_scheduler = true, debug_token_library = true, stack_usage_check_allocator = true, stack_usage_check_scheduler = true }");
    let project = fixture.resolved_project();
    let flags = |name: &str| {
        project
            .components
            .iter()
            .find(|component| component.name == name)
            .unwrap()
            .flags
            .iter()
            .map(|flag| flag.value.as_str())
            .collect::<Vec<_>>()
    };
    let allocator = flags("cheriot.allocator");
    assert!(allocator.contains(&"-DHEAP_RENDER=true"));
    assert!(allocator.contains(&"-DDEBUG_ALLOCATOR=DebugLevel::Warning"));
    assert!(allocator.contains(&"-DCHERIOT_STACK_CHECKS_ALLOCATOR=true"));
    let scheduler = flags("producer-consumer.scheduler");
    assert!(scheduler.contains(&"-DSCHEDULER_ACCOUNTING=true"));
    assert!(scheduler.contains(&"-DSCHEDULER_MULTIWAITER=false"));
    assert!(scheduler.contains(&"-DDEBUG_SCHEDULER=true"));
    assert!(scheduler.contains(&"-DCHERIOT_STACK_CHECKS_SCHEDULER=true"));
    assert!(flags("cheriot.loader").contains(&"-DDEBUG_LOADER=true"));
    assert!(flags("cheriot.token_library").contains(&"-DDEBUG_TOKEN_LIBRARY=true"));
    let allocator_recipe = project
        .components
        .iter()
        .find(|component| component.name == "cheriot.allocator")
        .unwrap();
    let origin = &allocator_recipe
        .flags
        .iter()
        .find(|flag| flag.value == "-DHEAP_RENDER=true")
        .unwrap()
        .origin;
    assert!(origin.field.ends_with("/options/allocator_rendering"));

    for setting in ["allocator_rendering = true", "scheduler_multiwaiter = true"] {
        let fixture = TestFixture::new();
        fixture.replace_text(
            "cheriot-compose.toml",
            "mode = \"release\"",
            &format!("mode = \"release\"\noptions = {{ allocator = false, {setting} }}"),
        );
        assert!(
            cheriot_compose::project::load_project(&fixture.project_file())
                .unwrap_err()
                .to_string()
                .contains("requires the allocator")
        );
    }
    let fixture = TestFixture::new();
    fixture.replace_text(
        "cheriot-compose.toml",
        "mode = \"release\"",
        "mode = \"release\"\noptions = { allocator = false }",
    );
    let project = fixture.resolved_project();
    assert!(!project.options.scheduler_multiwaiter);
    assert!(
        !project
            .components
            .iter()
            .any(|component| component.name == "cheriot.allocator")
    );
}

#[test]
fn float_printing_selects_softfloat_families_and_preserves_forced_headers() {
    let fixture = TestFixture::new();
    fixture.replace_text(
        "cheriot-compose.toml",
        "mode = \"release\"",
        "mode = \"release\"\noptions = { print_floats = true, print_doubles = true }",
    );
    let project = fixture.resolved_project();
    let debug = project
        .components
        .iter()
        .find(|component| component.name == "debug")
        .unwrap();
    assert!(
        !debug
            .flags
            .iter()
            .any(|flag| flag.value.contains("NO_FLOAT_PRINTING")
                || flag.value.contains("NO_DOUBLE_PRINTING"))
    );
    for name in [
        "softfloat32add",
        "softfloat32convert",
        "softfloat64compare",
        "softfloat64mul",
        "softfloat3264convert",
    ] {
        assert!(
            project
                .components
                .iter()
                .any(|component| component.name == name),
            "{name}"
        );
    }
    let plan = fixture.build_plan();
    let operation = plan
        .operations
        .iter()
        .find(|operation| operation.id == "compile.softfloat32add.0")
        .unwrap();
    let cheriot_compose::build_graph::BuildAction::Command { command, .. } = &operation.action
    else {
        panic!("expected compiler command")
    };
    let define = command
        .argv
        .iter()
        .position(|argument| argument.value == "-D__SOFTFP__")
        .unwrap();
    let ambient = command
        .argv
        .iter()
        .position(|argument| argument.value == "-DCHERIOT_NO_AMBIENT_MALLOC")
        .unwrap();
    let include = command
        .argv
        .iter()
        .position(|argument| argument.value == "-include")
        .unwrap();
    assert!(define < ambient && ambient < include);
    assert!(
        command.argv[include + 1]
            .value
            .ends_with("sdk/lib/softfloat/softfloat.h")
    );
    assert!(
        command.argv[include + 1].origins[0]
            .file
            .ends_with("softfloat/cheriot-compose.toml")
    );
}

#[test]
fn math_group_builds_both_widths_with_ordered_forced_headers() {
    let fixture = TestFixture::new();
    fixture.replace_text(
        "cheriot-compose.toml",
        "libraries = [\"freestanding\", \"message_queue\", \"debug\"]",
        "libraries = [\"freestanding\", \"message_queue\", \"debug\", \"math\"]",
    );
    let plan = fixture.build_plan();
    for width in ["math32", "math64"] {
        let operation = plan
            .operations
            .iter()
            .find(|operation| operation.id == format!("compile.{width}.0"))
            .unwrap();
        let cheriot_compose::build_graph::BuildAction::Command { command, .. } = &operation.action
        else {
            panic!("expected compiler command")
        };
        let headers: Vec<_> = command
            .argv
            .windows(2)
            .filter(|pair| pair[0].value == "-include")
            .map(|pair| pair[1].value.as_str())
            .collect();
        assert_eq!(headers.len(), 2);
        assert!(headers[0].ends_with("sdk/lib/math/math.h"));
        assert!(headers[1].ends_with("sdk/lib/math/compat.h"));
        assert!(
            operation
                .inputs
                .iter()
                .any(|path| path.ends_with("sdk/lib/math/math.h"))
        );
    }
}

#[test]
fn per_source_settings_apply_only_to_the_declared_source() {
    let fixture = TestFixture::new();
    fixture.write_file("only-producer.h", "/* header */\n");
    fixture.replace_text("cheriot-compose.toml", "build = { sources = [\"producer.cc\"] }", "build = { sources = [{ path = \"producer.cc\", defines = [\"SOURCE_ONLY=1\"], c_flags = [\"-Wc-only\"], cpp_flags = [\"-Wcpp-only\"], force_includes = [\"only-producer.h\"] }] }");
    let plan = fixture.build_plan();
    let producer = plan
        .operations
        .iter()
        .find(|operation| operation.id == "compile.producer.0")
        .unwrap();
    let consumer = plan
        .operations
        .iter()
        .find(|operation| operation.id == "compile.consumer.0")
        .unwrap();
    let cheriot_compose::build_graph::BuildAction::Command {
        command: producer_command,
        ..
    } = &producer.action
    else {
        panic!("expected command")
    };
    let cheriot_compose::build_graph::BuildAction::Command {
        command: consumer_command,
        ..
    } = &consumer.action
    else {
        panic!("expected command")
    };
    assert!(
        producer_command
            .argv
            .iter()
            .any(|argument| argument.value == "-DSOURCE_ONLY=1")
    );
    assert!(
        producer_command
            .argv
            .iter()
            .any(|argument| argument.value == "-Wcpp-only")
    );
    assert!(
        !producer_command
            .argv
            .iter()
            .any(|argument| argument.value == "-Wc-only")
    );
    assert!(
        !consumer_command
            .argv
            .iter()
            .any(|argument| argument.value == "-DSOURCE_ONLY=1")
    );
    assert!(
        producer
            .inputs
            .contains(&fixture.directory.join("only-producer.h"))
    );
    let index = producer_command
        .argv
        .iter()
        .position(|argument| argument.value == "-DSOURCE_ONLY=1")
        .unwrap();
    assert!(
        producer_command.argv[index].origins[0]
            .field
            .ends_with("/sources/0/defines/0")
    );
}

#[test]
#[cfg(feature = "rtos-verification")]
fn wall_clock_header_preserves_source_order_and_tracks_inputs() {
    let fixture = TestFixture::new();
    fixture.write_file("clocks/first.hh", "/* first */\n");
    fixture.write_file("clocks/second.hh", "/* second */\n");
    fixture.replace_text("cheriot-compose.toml", "mode = \"release\"", "mode = \"release\"\nclock_sources = [{ header = \"first.hh\", type_name = \"FirstClock\", include_dir = \"./clocks\" }, { header = \"second.hh\", type_name = \"drivers::SecondClock\", include_dir = \"./clocks\" }]");
    fixture.replace_text(
        "cheriot-compose.toml",
        "libraries = [\"freestanding\", \"message_queue\", \"debug\"]",
        "libraries = [\"freestanding\", \"message_queue\", \"debug\", \"wall_clock\"]",
    );
    let project = fixture.resolved_project();
    assert_eq!(project.shared_objects["wall_clock_time"], 24);
    let plan = fixture.build_plan();
    let generator = plan
        .operations
        .iter()
        .find(|operation| operation.id == "generate.rtc_sources.hh")
        .unwrap();
    let cheriot_compose::build_graph::BuildAction::Generate { content, .. } = &generator.action
    else {
        panic!("expected generated header")
    };
    let expected = include_str!("../../cheriot-rtos/sdk/lib/clock/rtc_sources.hh.in")
        .replace(
            "@include_implementations@",
            "#include <first.hh>\n#include <second.hh>\n",
        )
        .replace("@clock_sources@", "FirstClock, drivers::SecondClock, ");
    assert_eq!(content, &expected);
    let compiler = plan
        .operations
        .iter()
        .find(|operation| operation.id == "compile.wall_clock.0")
        .unwrap();
    assert!(compiler.dependencies.contains(&generator.id));
    assert!(
        compiler
            .inputs
            .contains(&project.build_dir.join("rtc_sources.hh"))
    );
    assert!(
        generator
            .inputs
            .contains(&fixture.directory.join("clocks/first.hh"))
    );
}

#[test]
fn benchmark_defines_follow_board_and_selected_options() {
    let fixture = TestFixture::new();
    fixture.replace_text(
        "cheriot-compose.toml",
        "build = { sources = [\"producer.cc\"] }",
        "build = { sources = [\"producer.cc\"], defines = [\"BOARD=${board}\", \"METRIC=${options.benchmark_metric}\", \"IRQ_SOURCE_${options.interrupt_source}\", \"DEBUG_PRODUCER=${options.component_debug}\"] }",
    );
    let mut text = std::fs::read_to_string(fixture.project_file()).unwrap();
    text.push_str("\n[options]\nbenchmark_metric = \"rdinstret\"\ninterrupt_source = \"sunburst_uart1\"\n[options.component_debug]\nproducer = true\n");
    fixture.write_file("cheriot-compose.toml", &text);
    let project = fixture.resolved_project();
    let flags = &project.targets["producer"].flags;
    for expected in [
        "-DBOARD=sail",
        "-DMETRIC=rdinstret",
        "-DIRQ_SOURCE_sunburst_uart1",
        "-DDEBUG_PRODUCER=true",
    ] {
        assert!(
            flags.iter().any(|flag| flag.value == expected),
            "{expected}"
        );
    }
    assert!(
        flags
            .iter()
            .find(|flag| flag.value == "-DMETRIC=rdinstret")
            .unwrap()
            .origin
            .rule
            .contains("/options/benchmark_metric")
    );
    fixture.replace_text("cheriot-compose.toml", "rdinstret", "unsupported_metric");
    let error = cheriot_compose::project::load_project(&fixture.project_file()).unwrap_err();
    assert!(error.to_string().contains("unknown variant"));
}

#[test]
fn environment_expansion_does_not_depend_on_diagnostic_wording() {
    use cheriot_compose::{configuration::resolution::resolve_path_value, diagnostic::Origin};
    use std::path::Path;
    let root = Path::new("/projects/app");
    let sdk = Path::new("/opt/sdk");
    let mut interpolated = Origin::derived("original description");
    interpolated.file = "/projects/app/cheriot-compose.toml".into();
    interpolated.expanded_from_environment("TEST_PATH");
    interpolated.rule = "A human-readable message can change".into();
    assert_eq!(
        resolve_path_value("${sdk}/literal", &interpolated, root, sdk).unwrap(),
        root.join("${sdk}/literal")
    );
    let mut ordinary = Origin::derived("environment variable appears only in this description");
    ordinary.file = "/projects/app/cheriot-compose.toml".into();
    assert_eq!(
        resolve_path_value("${sdk}/expanded", &ordinary, root, sdk).unwrap(),
        sdk.join("expanded")
    );
}

#[test]
fn domain_and_input_errors_keep_their_existing_order() {
    let fixture = TestFixture::new();
    fixture.replace_text(
        "cheriot-compose.toml",
        "stack_size = 0x500",
        "stack_size = 65281",
    );
    fixture.replace_text("boards/sail.toml", "tickrate_hz = 10", "tickrate_hz = 0");
    let error = cheriot_compose::project::load_project(&fixture.project_file())
        .unwrap_err()
        .to_string();
    assert!(error.contains("clock rates must be positive"), "{error}");

    fixture.replace_text("boards/sail.toml", "tickrate_hz = 0", "tickrate_hz = 10");
    let error = cheriot_compose::project::load_project(&fixture.project_file())
        .unwrap_err()
        .to_string();
    assert!(error.contains("65,280"), "{error}");
}
