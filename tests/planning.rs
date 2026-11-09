// Copyright 3Bian Limited and Contributors
// SPDX-License-Identifier: MIT

#[cfg(feature = "rtos-verification")]
use std::fs;

#[cfg(feature = "rtos-verification")]
use cheriot_compose::build_graph::BuildOperationKind;
use cheriot_compose::{
    build_graph::BuildAction, calculations::sdk, compilation_database, execution,
};
use test_fixture::TestFixture;
#[cfg(feature = "rtos-verification")]
use test_fixture::repository_root;

#[path = "support/test_fixture.rs"]
mod test_fixture;
#[test]
#[cfg(feature = "rtos-verification")]
fn complete_plan_and_golden_artifacts() {
    let fixture = TestFixture::new();
    let plan = fixture.build_plan();
    assert_eq!(plan.operations.len(), 62);
    assert_eq!(
        plan.operations
            .iter()
            .filter(|operation| matches!(
                operation.kind,
                BuildOperationKind::Compile | BuildOperationKind::Assemble
            ))
            .count(),
        29
    );
    assert_eq!(
        plan.operations
            .iter()
            .filter(|operation| operation.kind == BuildOperationKind::PartialLink)
            .count(),
        16
    );
    assert!(plan.unresolved_probes().values().all(Option::is_none));
    let board = plan
        .operations
        .iter()
        .find(|operation| operation.id == "generate.board.json")
        .unwrap();
    let BuildAction::Generate { content, .. } = &board.action else {
        panic!("expected board JSON")
    };
    let actual_board: serde_json::Value = serde_json::from_str(content).unwrap();
    let reference_board: serde_json::Value = serde_json::from_str(
        &fs::read_to_string(
            repository_root().join("verification/reference/producer-consumer-build/board.json"),
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(actual_board, reference_board);
    for name in [
        "producer-consumer-firmware.ldscript",
        "producer-consumer-firmware.rocode.ldscript",
        "producer-consumer-firmware.rwdata.ldscript",
        "board-interrupts.h",
    ] {
        let op = plan
            .operations
            .iter()
            .find(|operation| operation.id == format!("generate.{name}"))
            .unwrap();
        let BuildAction::Generate { content, .. } = &op.action else {
            panic!("not a generator")
        };
        let project = fixture.resolved_project();
        let mut relocated = content.replace(&plan.build_dir.display().to_string(), "build");
        for recipe in &project.components {
            let Some(path) = plan
                .operations
                .iter()
                .find(|operation| operation.id == format!("link.{}", recipe.name))
                .and_then(|operation| operation.outputs.first())
            else {
                continue;
            };
            let current = path
                .display()
                .to_string()
                .replace(&plan.build_dir.display().to_string(), "build");
            let filename = path
                .file_name()
                .unwrap()
                .to_str()
                .unwrap()
                .replace(&format!("--{}", project.configuration_identity), "");
            relocated = relocated.replace(
                &current,
                &format!("build/cheriot/cheriot/release/{filename}"),
            );
        }
        for (current, reference) in [
            ("*.scheduler--*.compartment", "*.scheduler.compartment"),
            (
                "*/cheriot.allocator--*.compartment",
                "*/cheriot.allocator.compartment",
            ),
            (
                "*/cheriot.software_revoker--*.compartment",
                "*/cheriot.software_revoker.compartment",
            ),
            (
                "*/cheriot.token_library--*.library",
                "*/cheriot.token_library.library",
            ),
            ("*/cheriot.switcher--entry.S--*.o", "*/switcher/entry.S.o"),
        ] {
            relocated = relocated.replace(current, reference);
        }
        let reference = fs::read_to_string(
            repository_root()
                .join("verification/reference/producer-consumer-build")
                .join(name),
        )
        .unwrap();
        assert_eq!(relocated, reference, "{name}");
    }
    let BuildAction::Generate { content, .. } = &plan
        .operations
        .iter()
        .find(|operation| operation.id == "generate.mmio.ldscript")
        .unwrap()
        .action
    else {
        panic!()
    };
    fn statements(script: &str) -> Vec<String> {
        let mut statements = script
            .split(';')
            .map(|text| text.split_whitespace().collect::<String>())
            .filter(|text| !text.is_empty())
            .collect::<Vec<_>>();
        statements.sort();
        statements
    }
    assert_eq!(
        statements(content),
        statements(
            &fs::read_to_string(
                repository_root()
                    .join("verification/reference/producer-consumer-build/mmio.ldscript")
            )
            .unwrap()
        )
    );
    assert_eq!(
        serde_json::to_string(&plan).unwrap(),
        serde_json::to_string(&fixture.build_plan()).unwrap()
    );
}

#[test]
#[cfg(feature = "rtos-verification")]
fn reachable_compile_flags_match_archived_commands() {
    let fixture = TestFixture::new();
    let plan = fixture.build_plan();
    let mut probes = plan.unresolved_probes();
    for probe in probes.values_mut() {
        *probe = Some(true)
    }
    let mut actual = compilation_database::export_compilation_database(&plan, &probes).unwrap();
    assert_eq!(actual.len(), 29);
    for entry in &mut actual {
        let argument_index = entry
            .arguments
            .iter()
            .position(|text| text == "-MMD")
            .unwrap();
        assert_eq!(entry.arguments[argument_index + 1], "-MF");
        entry.arguments.drain(argument_index..argument_index + 3);
    }
    let mut reference = compilation_database::read_compilation_database(
        &repository_root().join("verification/reference/producer-consumer-reachable.json"),
    )
    .unwrap();
    let old = "/workspaces/cheriot-rtos/examples/06.producer-consumer";
    let maps = vec![
        compilation_database::PathMapping {
            old: format!("{old}/build"),
            new: plan.build_dir.display().to_string(),
        },
        compilation_database::PathMapping {
            old: old.into(),
            new: fixture.directory.display().to_string(),
        },
        compilation_database::PathMapping {
            old: "/workspaces/cheriot-rtos/sdk".into(),
            new: fixture.directory.join("sdk").display().to_string(),
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
fn offline_compdb_keeps_probes_explicit() {
    let fixture = TestFixture::new();
    let plan = fixture.build_plan();
    let db = compilation_database::export_compilation_database(&plan, &plan.unresolved_probes())
        .unwrap();
    assert_eq!(db.len(), 32);
    assert_eq!(
        db.iter()
            .filter(|error| error.cheriot_compose_condition.is_some())
            .count(),
        6
    );
    assert!(!compilation_database::compare_compilation_databases(&db, &db, &[]).is_match());
    assert!(db.iter().all(|error| {
        !error
            .arguments
            .iter()
            .any(|argument| argument.starts_with("--script="))
    }));
}

#[test]
fn closure_counts_and_revoker_choices() {
    let fixture = TestFixture::new();
    let project = fixture.resolved_project();
    let rs = sdk::resolve_dependencies(&project).unwrap();
    assert_eq!(
        rs.iter()
            .filter(
                |recipe| recipe.kind == cheriot_compose::configuration::SdkComponentKind::Library
            )
            .count(),
        8
    );
    assert_eq!(
        rs.iter()
            .filter(|recipe| recipe.kind
                == cheriot_compose::configuration::SdkComponentKind::Compartment)
            .count(),
        3
    );
    fixture.replace_text(
        "boards/sail.toml",
        "revoker = \"software\"",
        "revoker = \"none\"",
    );
    let project = fixture.resolved_project();
    assert!(
        !project
            .board
            .flags
            .iter()
            .any(|flag| flag.value.contains("TEMPORAL"))
    );
    assert!(
        !sdk::resolve_dependencies(&project)
            .unwrap()
            .iter()
            .any(|recipe| recipe.name == "cheriot.software_revoker")
    );
}

#[test]
fn depfiles_handle_escaped_names() {
    let parsed=execution::parse_dependency_file("a.o b.o: src.cc path\\ with\\ spaces.h \\\n escaped\\#name.h dollar$$name.h\npath\\ with\\ spaces.h:\n",std::path::Path::new("/tmp")).unwrap();
    assert_eq!(
        parsed
            .iter()
            .map(|path_2| path_2.file_name().unwrap().to_string_lossy().to_string())
            .collect::<Vec<_>>(),
        [
            "dollar$name.h",
            "escaped#name.h",
            "path with spaces.h",
            "src.cc"
        ]
    );
    assert!(execution::parse_dependency_file("broken", std::path::Path::new("/tmp")).is_err());
}

#[test]
fn split_memory_orders_segments() {
    let fixture = TestFixture::new();
    fixture.replace_text(
        "boards/sail.toml",
        "simulation = true",
        "simulation = true\ndata_memory = { start = 0x100000, length = 0x40000 }",
    );
    fixture.replace_text(
        "boards/sail.toml",
        "[heap]\nend = 2147745792",
        "[heap]\nend = 0x140000",
    );
    let plan = fixture.build_plan();
    let BuildAction::Generate { content, .. } = &plan
        .operations
        .iter()
        .find(|operation| operation.id == "generate.producer-consumer-firmware.ldscript")
        .unwrap()
        .action
    else {
        panic!()
    };
    assert!(content.find("0x100000").unwrap() < content.find("0x80000000").unwrap());
    assert!(content.contains("load2 PT_LOAD"));
}

#[test]
fn board_linker_fragments_are_ordered_rendered_and_tracked() {
    let fixture = TestFixture::new();
    fixture.write_file("external.ld", ".external : { *(.external) }\n");
    fixture.write_file(
        "generated.ld.mustache",
        "PROVIDE(fragment{{ix}} = {{{start_string}}});\nPROVIDE(threads = {{thread_count}});\n",
    );
    fixture.replace_text(
        "boards/sail.toml",
        "[devices.clint]",
        r#"linker_fragments = [
  { start = 0x5000, path = "../generated.ld.mustache", generated_name = "board-generated.ld" },
  { start = 0x1000, program_header = "NOTE", body = ".note{{ix}} : { LONG({{{start_string}}}) }" },
  { start = 0x4000, path = "../external.ld" },
]

[devices.clint]"#,
    );
    let plan = fixture.build_plan();
    let generated = plan
        .operations
        .iter()
        .find(|operation| operation.id == "generate.board-generated.ld")
        .unwrap();
    assert!(
        generated
            .inputs
            .contains(&fixture.directory.join("generated.ld.mustache"))
    );
    let BuildAction::Generate { content, .. } = &generated.action else {
        panic!("expected generated fragment")
    };
    assert_eq!(
        content,
        "PROVIDE(fragment3 = 0x5000);\nPROVIDE(threads = 2);\n"
    );
    let top = plan
        .operations
        .iter()
        .find(|operation| operation.id == "generate.producer-consumer-firmware.ldscript")
        .unwrap();
    let BuildAction::Generate { content, .. } = &top.action else {
        panic!("expected top-level script")
    };
    assert!(content.contains("load1 PT_NOTE"));
    assert!(content.contains(".note1 : { LONG(0x1000) }"));
    let inline = content.find(".note1 :").unwrap();
    let external = content.find("external.ld\"").unwrap();
    let generated_include = content.find("board-generated.ld\"").unwrap();
    assert!(inline < external && external < generated_include);
    let link = plan
        .operations
        .iter()
        .find(|operation| operation.id == "link.firmware")
        .unwrap();
    assert!(link.inputs.contains(&fixture.directory.join("external.ld")));
    assert!(
        link.inputs
            .contains(&plan.build_dir.join("board-generated.ld"))
    );
    assert!(link.dependencies.contains(&generated.id));
    let shell = cheriot_compose::render::render_shell_script(&plan).unwrap();
    assert!(shell.contains("PROVIDE(fragment3 = 0x5000)"));
}

#[test]
fn board_linker_fragments_reject_invalid_fields() {
    for fragment in [
        "{ start = 1, body = \"a\", path = \"../external.ld\" }",
        "{ start = 1, body = \"a\", program_header = \"LOAD; bad\" }",
        "{ start = 1, path = \"../missing.ld\" }",
        "{ start = 1, path = \"../external.ld\", generated_name = \"../escape\" }",
    ] {
        let fixture = TestFixture::new();
        fixture.write_file("external.ld", "/* fixture */");
        fixture.replace_text(
            "boards/sail.toml",
            "[devices.clint]",
            &format!("linker_fragments = [{fragment}]\n\n[devices.clint]"),
        );
        let error = cheriot_compose::project::load_project(&fixture.project_file()).unwrap_err();
        assert!(error.origin.unwrap().file.ends_with("sail.toml"));
    }
}

#[test]
fn interrupt_fallback() {
    let fixture = TestFixture::new();
    fixture.replace_text(
        "boards/sail.toml",
        "interrupts = [\n    { name = \"FakeInterrupt\", number = 4, priority = 2 },\n]",
        "interrupts = []",
    );
    let plan = fixture.build_plan();
    let BuildAction::Generate { content, .. } = &plan
        .operations
        .iter()
        .find(|operation| operation.id == "generate.board-interrupts.h")
        .unwrap()
        .action
    else {
        panic!()
    };
    assert!(content.contains("DummyInterrupt=0"));
}

#[test]
fn firmware_dependency_group_links_direct_object_members_once() {
    let fixture = TestFixture::new();
    fixture.write_file("helper.c", "/* source */\n");
    fixture.replace_text(
        "cheriot-compose.toml",
        "[[firmwares]]\n",
        "[libraries.helper_object]\nkind = \"object\"\nbuild = { sources = [\"helper.c\"] }\n\n[libraries.helpers]\nkind = \"group\"\ndependencies = [\"helper_object\"]\nbuild = { sources = [] }\n\n[[firmwares]]\n",
    );
    fixture.replace_text(
        "cheriot-compose.toml",
        "libraries = [\"freestanding\", \"message_queue\", \"debug\"]",
        "libraries = [\"freestanding\", \"message_queue\", \"debug\", \"helpers\", \"helper_object\"]",
    );
    let plan = fixture.build_plan();
    let link = plan
        .operations
        .iter()
        .find(|operation| operation.id == "link.firmware")
        .unwrap();
    let BuildAction::Command { command, .. } = &link.action else {
        panic!("expected link command")
    };
    let object = &plan
        .operations
        .iter()
        .find(|operation| operation.id == "compile.helper_object.0")
        .unwrap()
        .outputs[0];
    assert_eq!(
        command
            .argv
            .iter()
            .filter(|argument| argument.value == object.display().to_string())
            .count(),
        1
    );
    assert!(
        link.dependencies
            .iter()
            .any(|dependency| dependency == "compile.helper_object.0")
    );
    assert!(
        !plan
            .operations
            .iter()
            .any(|operation| operation.id == "link.helpers")
    );
}

#[test]
fn cargo_manifest_is_planned_as_metadata_build_and_component_input() {
    let fixture = TestFixture::new();
    fixture.write_file("crate/Cargo.toml", "[package]\nname = \"firmware-example\"\nversion = \"0.1.0\"\nedition = \"2024\"\n[lib]\ncrate-type = [\"staticlib\"]\n");
    fixture.write_file("crate/src/lib.rs", "#![no_std]\n");
    fixture.replace_text("cheriot-compose.toml", "build = { sources = [\"producer.cc\"] }", "build = { sources = [\"producer.cc\", { path = \"crate/Cargo.toml\", rust_flags = [\"-Cdebuginfo=1\"], cargo_flags = [\"--offline\"] }] }");
    let plan = fixture.build_plan();
    let metadata = plan
        .operations
        .iter()
        .find(|operation| operation.id == "metadata.producer.1")
        .unwrap();
    let BuildAction::Command { command, .. } = &metadata.action else {
        panic!("expected Cargo metadata command")
    };
    assert_eq!(command.executable, "cargo");
    assert!(
        command
            .argv
            .iter()
            .any(|argument| argument.value == "--offline")
    );
    assert_eq!(command.stdout.as_ref(), metadata.outputs.first());
    let cargo = plan
        .operations
        .iter()
        .find(|operation| operation.id == "compile.producer.1")
        .unwrap();
    assert_eq!(
        cargo.kind,
        cheriot_compose::build_graph::BuildOperationKind::CargoBuild
    );
    let BuildAction::Command { command, .. } = &cargo.action else {
        panic!("expected Cargo build command")
    };
    assert!(command.environment["CARGO_ENCODED_RUSTFLAGS"].contains("-Cdebuginfo=1"));
    assert_eq!(
        command.environment["RUSTC"],
        "<unresolved firmware Rust compiler>"
    );
    assert!(cargo.dependencies.contains(&metadata.id));
    assert!(
        cargo.outputs[0]
            .ends_with("riscv32cheriot-unknown-cheriotrtos/release/libfirmware_example.a")
    );
    assert!(
        plan.operations
            .iter()
            .find(|operation| operation.id == "link.producer")
            .unwrap()
            .dependencies
            .contains(&cargo.id)
    );
    let database =
        compilation_database::export_compilation_database(&plan, &plan.unresolved_probes())
            .unwrap();
    assert!(
        database
            .iter()
            .all(|entry| !entry.file.ends_with("Cargo.toml"))
    );
}

#[test]
fn cargo_manifest_needs_a_static_library_and_safe_options() {
    let fixture = TestFixture::new();
    fixture.write_file(
        "crate/Cargo.toml",
        "[package]\nname = \"bad-example\"\nversion = \"0.1.0\"\n[lib]\ncrate-type = [\"rlib\"]\n",
    );
    fixture.replace_text(
        "cheriot-compose.toml",
        "build = { sources = [\"producer.cc\"] }",
        "build = { sources = [\"producer.cc\", \"crate/Cargo.toml\"] }",
    );
    let error = cheriot_compose::project::load_project(&fixture.project_file()).unwrap_err();
    assert!(
        error
            .to_string()
            .contains("crate-type must include staticlib")
    );
}

#[test]
fn sonata_packages_are_explicit_graph_actions() {
    let fixture = TestFixture::new();
    fixture.replace_text(
        "boards/sail.toml",
        "name = \"sail\"",
        "name = \"sail\"\npackaging = \"sonata_v1\"",
    );
    fixture.build_plan();
    let project = fixture.resolved_project();
    let plan = cheriot_compose::planning::create_packaging_plan(&project).unwrap();
    let strip = plan
        .operations
        .iter()
        .find(|operation| operation.id == "package.strip")
        .unwrap();
    assert!(strip.dependencies.contains(&"publish.firmware".into()));
    for (slot, address) in [
        (1, "-b0x00000000"),
        (2, "-b0x10000000"),
        (3, "-b0x20000000"),
    ] {
        let operation = plan
            .operations
            .iter()
            .find(|operation| operation.id == format!("package.slot{slot}"))
            .unwrap();
        assert!(operation.dependencies.contains(&strip.id));
        let BuildAction::Command { command, .. } = &operation.action else {
            panic!("expected UF2 command")
        };
        assert!(
            command
                .argv
                .iter()
                .any(|argument| argument.value == address)
        );
        assert!(
            command
                .argv
                .iter()
                .any(|argument| argument.value == "-f0x6CE29E60")
        );
    }
    let shell = cheriot_compose::render::render_shell_script(&plan).unwrap();
    assert!(shell.contains("uf2conv"));
    fixture.replace_text("boards/sail.toml", "sonata_v1", "sonata_legacy");
    let legacy = fixture.resolved_project();
    let plan = cheriot_compose::planning::create_packaging_plan(&legacy).unwrap();
    assert!(
        plan.operations
            .iter()
            .any(|operation| operation.id == "package.binary")
    );
    assert!(
        plan.operations
            .iter()
            .any(|operation| operation.id == "package.uf2")
    );
    fixture.replace_text("boards/sail.toml", "sonata_legacy", "microsoft_safe");
    let safe = fixture.resolved_project();
    let plan = cheriot_compose::planning::create_packaging_plan(&safe).unwrap();
    for width in [4, 8] {
        let operation = plan
            .operations
            .iter()
            .find(|operation| operation.id == format!("package.iram{width}"))
            .unwrap();
        assert!(
            matches!(operation.action, BuildAction::MemoryImage { word_bytes, .. } if word_bytes == width)
        );
        assert!(operation.dependencies.contains(&"package.binary".into()));
        let boot = plan
            .operations
            .iter()
            .find(|operation| operation.id == format!("package.irom{width}"))
            .unwrap();
        let BuildAction::Generate { content, .. } = &boot.action else {
            panic!("expected boot image")
        };
        assert_eq!(content.lines().count(), 128 / usize::from(width) + 1);
        assert!(content.lines().last().unwrap().ends_with("7813f06f"));
    }
    assert!(
        cheriot_compose::render::render_shell_script(&plan)
            .unwrap()
            .contains("hexdump -v")
    );
}

#[test]
fn generated_artifacts_track_only_the_partials_they_use() {
    let fixture = TestFixture::new();
    let project = fixture.resolved_project();
    let partials = cheriot_compose::render::firmware::firmware_template_partials();
    let inputs = firmware_render_inputs(&project);
    let original =
        cheriot_compose::render::firmware::generate_with_partials(&project, &inputs, &partials)
            .unwrap();
    let mut changed_partials = partials.clone();
    changed_partials
        .get_mut("thread_stack_template")
        .unwrap()
        .push_str("\n# changed stack partial\n");
    let changed = cheriot_compose::render::firmware::generate_with_partials(
        &project,
        &inputs,
        &changed_partials,
    )
    .unwrap();
    let artifact = |items: &[cheriot_compose::render::firmware::GeneratedArtifact], name: &str| {
        items
            .iter()
            .find(|item| item.name == name)
            .unwrap()
            .template_fingerprint()
            .unwrap()
    };
    let rwdata = "producer-consumer-firmware.rwdata.ldscript";
    assert_ne!(artifact(&original, rwdata), artifact(&changed, rwdata));
    for name in [
        "mmio.ldscript",
        "board-interrupts.h",
        "producer-consumer-firmware.rocode.ldscript",
        "producer-consumer-firmware.ldscript",
    ] {
        assert_eq!(
            artifact(&original, name),
            artifact(&changed, name),
            "{name}"
        );
    }
    let rwdata = original.iter().find(|item| item.name == rwdata).unwrap();
    assert!(
        rwdata
            .template_dependencies
            .contains_key("thread_stack_template")
    );
    assert!(
        rwdata
            .template_dependencies
            .contains_key("section_thread_stacks")
    );
    assert!(!rwdata.template_dependencies.contains_key("layout_phdr"));
}

#[test]
fn linker_fragments_track_the_sections_their_templates_use() {
    let fixture = TestFixture::new();
    fixture.write_file(
        "stack-fragment.mustache",
        "{{#thread_stacks}}{{.}}{{/thread_stacks}}\n",
    );
    fixture.replace_text(
        "boards/sail.toml",
        "[devices.clint]",
        r#"linker_fragments = [
  { start = 0x1000, body = "{{{thread_stacks}}}" },
  { start = 0x2000, path = "../stack-fragment.mustache", generated_name = "stack-fragment.ld" },
]

[devices.clint]"#,
    );
    let project = fixture.resolved_project();
    let partials = cheriot_compose::render::firmware::firmware_template_partials();
    let inputs = firmware_render_inputs(&project);
    std::fs::remove_file(fixture.directory.join("stack-fragment.mustache")).unwrap();
    let original =
        cheriot_compose::render::firmware::generate_with_partials(&project, &inputs, &partials)
            .unwrap();
    let mut changed = partials;
    changed
        .get_mut("thread_stack_template")
        .unwrap()
        .push_str("\n# modified stack\n");
    let modified =
        cheriot_compose::render::firmware::generate_with_partials(&project, &inputs, &changed)
            .unwrap();
    let artifact = |items: &[cheriot_compose::render::firmware::GeneratedArtifact], name: &str| {
        items
            .iter()
            .find(|item| item.name == name)
            .unwrap()
            .template_fingerprint()
            .unwrap()
    };
    for name in ["stack-fragment.ld", "producer-consumer-firmware.ldscript"] {
        assert_ne!(
            artifact(&original, name),
            artifact(&modified, name),
            "{name}"
        );
    }
    assert_eq!(
        artifact(&original, "mmio.ldscript"),
        artifact(&modified, "mmio.ldscript")
    );
}

fn firmware_render_inputs(
    project: &cheriot_compose::configuration::ResolvedProject,
) -> cheriot_compose::render::firmware::FirmwareInputs {
    use cheriot_compose::configuration::LinkerFragmentSource;
    use cheriot_compose::render::firmware::{FirmwareComponent, FirmwareInputs};
    let segments = cheriot_compose::calculations::layout::firmware_linker_segments(
        &project.board,
        project.build_dir.join(format!(
            "{}-firmware.rocode.ldscript",
            project.firmware.name
        )),
        project.build_dir.join(format!(
            "{}-firmware.rwdata.ldscript",
            project.firmware.name
        )),
    );
    let fragment_templates = segments
        .iter()
        .filter_map(|segment| match &segment.source {
            LinkerFragmentSource::Generated { template, .. } => {
                Some((template.clone(), std::fs::read_to_string(template).unwrap()))
            }
            _ => None,
        })
        .collect();
    let components = project
        .components
        .iter()
        .map(|recipe| FirmwareComponent {
            name: recipe.name.clone(),
            kind: recipe.kind,
            object: project.build_dir.join(format!("fixture-{}.o", recipe.name)),
        })
        .collect();
    FirmwareInputs {
        components,
        segments,
        fragment_templates,
    }
}
