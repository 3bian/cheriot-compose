// Copyright 3Bian Limited and Contributors
// SPDX-License-Identifier: MIT

use test_fixture::TestFixture;

#[path = "support/test_fixture.rs"]
mod test_fixture;

#[test]
fn graph_rejects_cycles_duplicate_outputs_and_dependencies() {
    let fixture = TestFixture::new();
    let plan = fixture.build_plan();
    let mut modified_plan = plan.clone();
    let id = modified_plan.operations[0].id.clone();
    modified_plan.operations[0].dependencies.push(id);
    assert!(modified_plan.validate().is_err());
    let mut modified_plan = plan.clone();
    modified_plan.operations[1].outputs = modified_plan.operations[0].outputs.clone();
    assert!(modified_plan.validate().is_err());
    let mut modified_plan = plan;
    modified_plan.operations[1]
        .dependencies
        .push("absent".into());
    assert!(modified_plan.validate().is_err());
}

#[test]
fn generated_input_needs_dependency() {
    let fixture = TestFixture::new();
    let mut plan = fixture.build_plan();
    let compile_operation = plan
        .operations
        .iter_mut()
        .find(|operation| operation.id == "compile.producer.0")
        .unwrap();
    compile_operation.dependencies.clear();
    assert!(
        plan.validate()
            .unwrap_err()
            .to_string()
            .contains("without a dependency")
    );
}

#[test]
fn graph_tracks_transitive_prerequisites_and_rejects_unowned_generated_inputs() {
    let fixture = TestFixture::new();
    let plan = fixture.build_plan();
    let closure = plan
        .prerequisite_closure(&["link.firmware".into()])
        .unwrap();
    assert!(closure.contains("link.firmware"));
    assert!(closure.contains("compile.producer.0"));
    assert!(closure.contains("generate.board-interrupts.h"));
    let generated = plan.build_dir.join("board-interrupts.h");
    assert_eq!(
        plan.artifact_producer(&generated).unwrap().id,
        "generate.board-interrupts.h"
    );
    let mut invalid = plan.clone();
    invalid
        .operations
        .iter_mut()
        .find(|operation| operation.id == "compile.producer.0")
        .unwrap()
        .inputs
        .push(plan.build_dir.join("unowned.h"));
    assert!(
        invalid
            .validate()
            .unwrap_err()
            .to_string()
            .contains("without a producer")
    );
    let mut invalid = plan;
    invalid.operations[0].outputs[0] = fixture.directory.join(".cheriot-compose/../outside");
    assert!(invalid.validate().is_err());
}
