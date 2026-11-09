// Copyright 3Bian Limited and Contributors
// SPDX-License-Identifier: MIT

use cheriot_compose::switcher_analysis;

const VALID_SWITCHER: &str = r#".globl __Z26compartment_switcher_entryz
__Z26compartment_switcher_entryz:
 // IRQ REQUIRE: enabled
 mret
exception_entry_asm:
 // IRQ REQUIRE: enabled
 mret
switcher_after_compartment_call:
 // IRQ REQUIRE: deferred
 mret
.Lhandle_error_handler_return:
 // FROM: malice
 // IRQ REQUIRE: deferred
 mret
 // Switcher-exported library functions.
"#;

#[test]
fn annotated_switcher_produces_deterministic_dot() {
    let graph = switcher_analysis::analyze(VALID_SWITCHER).unwrap();
    assert_eq!(graph, switcher_analysis::analyze(VALID_SWITCHER).unwrap());
    assert!(graph.starts_with("digraph switcher {"));
    assert!(graph.contains("\"__Z26compartment_switcher_entryz\" [tooltip=1,shape=box,fontname=\"Times-Italic\",style=filled,fillcolor=cyan]"));
    assert!(graph.contains("\".Lhandle_error_handler_return\" [tooltip=10,shape=box,fontname=\"Times-Bold\",style=filled,fillcolor=orange]"));
}

#[test]
fn missing_irq_requirements_and_invalid_arcs_are_errors() {
    let missing = VALID_SWITCHER.replacen(" // IRQ REQUIRE: enabled\n", "", 1);
    assert!(
        switcher_analysis::analyze(&missing)
            .unwrap_err()
            .to_string()
            .contains("Missing IRQ requirement")
    );
    let invalid = VALID_SWITCHER.replacen(
        " // IRQ REQUIRE: enabled\n mret\n",
        " // IRQ REQUIRE: enabled\n beq a0,a1,.Ltarget\n.Ltarget:\n // FROM: above\n // IRQ REQUIRE: deferred\n mret\n",
        1,
    );
    assert!(
        switcher_analysis::analyze(&invalid)
            .unwrap_err()
            .to_string()
            .contains("IRQ-invalid arc")
    );
    let no_edge = invalid.replace(" // FROM: above\n", "");
    assert!(
        switcher_analysis::analyze(&no_edge)
            .unwrap_err()
            .to_string()
            .contains("no declared out edge")
    );
}

#[test]
fn matching_branch_and_from_annotation_create_an_edge() {
    let source = VALID_SWITCHER.replacen(
        " // IRQ REQUIRE: enabled\n mret\n",
        " // IRQ REQUIRE: enabled\n beq a0,a1,.Ltarget\n.Ltarget:\n // FROM: above\n mret\n",
        1,
    );
    let graph = switcher_analysis::analyze(&source).unwrap();
    assert!(graph.contains("\"__Z26compartment_switcher_entryz\" -> \".Ltarget\" [];"));
    assert!(graph.contains("\".Ltarget\" [tooltip=4,shape=octagon,fontname=\"Times-Italic\""));
}

#[test]
fn cli_reads_input_and_writes_dot() {
    let directory = std::env::temp_dir().join(format!("cheriot-switcher-{}", std::process::id()));
    std::fs::create_dir_all(&directory).unwrap();
    let input = directory.join("entry.S");
    let output = directory.join("switcher.dot");
    std::fs::write(&input, VALID_SWITCHER).unwrap();
    let result = std::process::Command::new(env!("CARGO_BIN_EXE_cheriot-compose"))
        .arg("analyze-switcher")
        .arg("--input")
        .arg(&input)
        .arg("--output")
        .arg(&output)
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert!(result.stdout.is_empty());
    assert_eq!(
        std::fs::read_to_string(&output).unwrap(),
        switcher_analysis::analyze(VALID_SWITCHER).unwrap()
    );
    std::fs::remove_dir_all(directory).unwrap();
}

#[cfg(feature = "rtos-verification")]
#[test]
fn supplied_switcher_reports_the_same_missing_irq_annotation_as_the_lua_reference() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../cheriot-rtos/sdk/core/switcher/entry.S");
    let error = switcher_analysis::analyze(&std::fs::read_to_string(path).unwrap()).unwrap_err();
    assert!(
        error.to_string().contains("Missing IRQ requirement"),
        "{error}"
    );
}
