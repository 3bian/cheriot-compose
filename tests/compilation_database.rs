// Copyright 3Bian Limited and Contributors
// SPDX-License-Identifier: MIT

#[cfg(feature = "rtos-verification")]
use cheriot_compose::compilation_database::read_compilation_database;
use cheriot_compose::compilation_database::{
    CompilationEntry, PathMapping, compare_compilation_databases, split_command_arguments,
};
use cheriot_compose::render::compilation_database::format_comparison_report;

fn entry(file: &str, output: &str, arguments: &[&str]) -> CompilationEntry {
    CompilationEntry {
        directory: "/old/project".into(),
        file: file.into(),
        output: Some(output.into()),
        arguments: arguments.iter().map(|text_2| text_2.to_string()).collect(),
        command: None,
        cheriot_compose_condition: None,
        cheriot_compose_target: None,
    }
}

#[test]
fn shell_words_preserve_boundaries() {
    assert_eq!(
        split_command_arguments("'compiler path' -D'X=a b' \"x\\\"y\" '' foo\\ bar").unwrap(),
        ["compiler path", "-DX=a b", "x\"y", "", "foo bar"]
    );
    for command in [
        "cc x; rm x",
        "cc $(bad)",
        "cc `bad`",
        "cc x | bad",
        "cc \"$HOME\"",
        "cc 'open",
    ] {
        assert!(split_command_arguments(command).is_err(), "{command}")
    }
}

#[test]
fn exact_flag_order_abi_and_path_changes() {
    let first_entry = entry(
        "a.cc",
        "a.o",
        &[
            "cc",
            "-target",
            "cheriot",
            "-mabi=cheriot",
            "-DA",
            "-DB",
            "a.cc",
        ],
    );
    let mut modified_entry = first_entry.clone();
    modified_entry.arguments.swap(4, 5);
    modified_entry.arguments[3] = "-mabi=baremetal".into();
    modified_entry.output = Some("other.o".into());
    let report = compare_compilation_databases(&[first_entry], &[modified_entry], &[]);
    assert_eq!(report.different.len(), 1);
    let text = format_comparison_report(&report);
    assert!(text.contains("ABI"));
    assert!(text.contains("argv[4]"));
    assert!(text.contains("output path"));
}

#[test]
fn explicit_mapping_and_component_boundary() {
    let first_entry = entry(
        "a.cc",
        "a.o",
        &["/old/tool/cc", "-I/old/project/include", "a.cc"],
    );
    let mut modified_entry = first_entry.clone();
    modified_entry.directory = "/new/project".into();
    modified_entry.arguments = vec![
        "/new/tool/cc".into(),
        "-I/new/project/include".into(),
        "a.cc".into(),
    ];
    let path_mappings = [PathMapping::parse("/old=/new").unwrap()];
    assert!(
        compare_compilation_databases(
            std::slice::from_ref(&first_entry),
            &[modified_entry],
            &path_mappings
        )
        .is_match()
    );
    let mut wrong_path_entry = first_entry.clone();
    wrong_path_entry.arguments[1] = "-I/oldish/project/include".into();
    assert!(
        !compare_compilation_databases(&[first_entry], &[wrong_path_entry], &path_mappings)
            .is_match()
    );
}

#[test]
fn variants_missing_extra_and_ambiguity() {
    let first_entry = entry("a.cc", "a.o", &["cc", "-DX"]);
    let second_entry = entry("a.cc", "b.o", &["cc", "-DY"]);
    assert!(
        compare_compilation_databases(
            &[first_entry.clone(), second_entry.clone()],
            &[second_entry.clone(), first_entry.clone()],
            &[]
        )
        .is_match()
    );
    let report = compare_compilation_databases(
        &[first_entry.clone(), second_entry],
        std::slice::from_ref(&first_entry),
        &[],
    );
    assert_eq!(report.missing.len(), 1);
    let report = compare_compilation_databases(
        std::slice::from_ref(&first_entry),
        &[first_entry.clone(), first_entry.clone()],
        &[],
    );
    assert_eq!(report.ambiguous.len(), 1);
    assert_eq!(report.extra.len(), 2);
}

#[test]
fn interleaved_variants_match_once_and_keep_unmatched_order() {
    let first = entry("a.cc", "shared.o", &["cc", "-DFIRST"]);
    let second = entry("a.cc", "shared.o", &["cc", "-DSECOND"]);
    let other = entry("b.cc", "b.o", &["cc"]);
    let extra = entry("extra.cc", "extra.o", &["cc"]);
    let report = compare_compilation_databases(
        &[first.clone(), other.clone(), second.clone(), first],
        &[
            second,
            other,
            extra,
            entry("a.cc", "shared.o", &["cc", "-DFIRST"]),
        ],
        &[],
    );
    assert!(report.different.is_empty());
    assert!(report.ambiguous.is_empty());
    assert_eq!(report.missing.len(), 1);
    assert_eq!(report.missing[0].file, "/old/project/a.cc");
    assert_eq!(report.extra.len(), 1);
    assert_eq!(report.extra[0].file, "/old/project/extra.cc");
}

#[test]
#[cfg(feature = "rtos-verification")]
fn original_archive_databases_are_counted() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../verification/reference");
    assert_eq!(
        read_compilation_database(&root.join("compile_commands.json"))
            .unwrap()
            .len(),
        174
    );
    assert_eq!(
        read_compilation_database(
            &root.join("examples/06.producer-consumer/compile_commands.json")
        )
        .unwrap()
        .len(),
        139
    );
    assert_eq!(
        read_compilation_database(&root.join("producer-consumer-reachable.json"))
            .unwrap()
            .len(),
        29
    );
}
