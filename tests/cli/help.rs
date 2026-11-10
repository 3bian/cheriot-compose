// Copyright 3Bian Limited and Contributors
// SPDX-License-Identifier: MIT

use std::path::Path;
use std::process::Command;

use super::TestFixture;

const COMMANDS: &[&str] = &[
    "analyze-switcher",
    "audit",
    "build",
    "check",
    "clean",
    "compare-compilation-db",
    "configuration",
    "explain",
    "package",
    "plan",
    "run",
    "size",
];

const OPTIONS: &[(&str, Option<&str>)] = &[
    ("--against", Some("reference.json")),
    ("--audit-tool", Some("audit-tool")),
    ("--board", Some("board")),
    ("--build-dir", Some("build")),
    ("--compartment", Some("compartment")),
    ("--dry-run", None),
    ("--file", Some("project.toml")),
    ("--firmware", Some("firmware")),
    ("--format", Some("json")),
    ("--generated", Some("generated.json")),
    ("--input", Some("input.s")),
    ("--jobs", Some("1")),
    ("--map-path", Some("old=new")),
    ("--mode", Some("release")),
    ("--module", Some("module")),
    ("--output", Some("output")),
    ("--output-dir", Some("output")),
    ("--overlay", Some("overlay.toml")),
    ("--package", None),
    ("--probe-toolchain", None),
    ("--project", Some("project")),
    ("--query", Some("query")),
    ("--quiet", None),
    ("--resolved", None),
    ("--toolchain-dir", Some("tools")),
    ("--verbose", None),
    ("--vm-profile", Some("profile.toml")),
];

fn invoke(directory: &Path, arguments: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_cheriot-compose"))
        .current_dir(directory)
        .args(arguments)
        .env("PATH", directory.join("missing-executables"))
        .env("CHERIOT_COMPOSE_FILE", "")
        .env("CHERIOT_COMPOSE_TOOLCHAIN_DIR", "")
        .env("CHERIOT_COMPOSE_JOBS", "invalid")
        .output()
        .unwrap()
}

fn entries(section: &str) -> Vec<&str> {
    section
        .trim_end()
        .split("\n\n")
        .map(|entry| {
            let mut lines = entry.lines();
            let signature = lines.next().unwrap();
            assert!(signature.starts_with("  ") && !signature.starts_with("   "));
            let description: Vec<_> = lines.collect();
            assert!(!description.is_empty(), "missing description: {signature}");
            for line in description {
                assert!(
                    line.starts_with("    ") && !line.starts_with("     "),
                    "{line}"
                );
            }
            signature.trim()
        })
        .collect()
}

fn options(text: &str, heading: &str) -> Vec<String> {
    let section = text.split_once(heading).unwrap().1;
    let signatures = entries(section);
    let names: Vec<_> = signatures
        .iter()
        .map(|signature| {
            signature
                .split_whitespace()
                .find(|word| word.starts_with("--"))
                .unwrap()
                .to_owned()
        })
        .collect();
    assert_eq!(names.last().map(String::as_str), Some("--help"));
    assert!(text.ends_with("  -h, --help\n    Display this help and exit.\n"));
    let mut sorted = names[..names.len() - 1].to_vec();
    sorted.sort_unstable();
    assert_eq!(names[..names.len() - 1], sorted);
    if names.iter().any(|name| name == "--file") {
        assert!(signatures.contains(&"-f, --file FILE"));
    }
    names
}

#[test]
fn general_and_command_help_are_ordered_and_work_without_tools() {
    let fixture = TestFixture::new();
    let directory = fixture.directory.join("no-project");
    std::fs::create_dir(&directory).unwrap();

    let general = invoke(&directory, &[]);
    assert!(general.status.success());
    for arguments in [&["-h"][..], &["--help"][..]] {
        let result = invoke(&directory, arguments);
        assert!(result.status.success());
        assert_eq!(general.stdout, result.stdout);
    }
    let general = String::from_utf8(general.stdout).unwrap();
    for line in general.lines() {
        assert!(line.chars().count() <= 80, "{line}");
    }
    let command_section = general
        .split_once("Commands:\n")
        .unwrap()
        .1
        .split_once("\n\nOptions follow")
        .unwrap()
        .0;
    let displayed: Vec<_> = entries(command_section)
        .iter()
        .map(|signature| signature.split_whitespace().next().unwrap())
        .collect();
    assert_eq!(displayed, COMMANDS);
    options(&general, "Common options:\n");

    for command in COMMANDS {
        let short = invoke(&directory, &[command, "-h"]);
        let long = invoke(&directory, &[command, "--help"]);
        assert!(short.status.success(), "{command}");
        assert_eq!(short.stdout, long.stdout, "{command}");
        let text = String::from_utf8(long.stdout).unwrap();
        assert!(text.starts_with(&format!("Usage: cheriot-compose {command}")));
        options(&text, "\nOptions:\n");
        for line in text.lines() {
            assert!(line.chars().count() <= 80, "{command}: {line}");
        }
    }
    for arguments in [
        &["help"][..],
        &["help", "build"][..],
        &["version"][..],
        &["compare-compilation-database"][..],
    ] {
        assert!(
            !invoke(&directory, arguments).status.success(),
            "{arguments:?}"
        );
    }
}

#[test]
fn command_help_lists_only_options_the_parser_accepts() {
    let fixture = TestFixture::new();
    let directory = fixture.directory.join("no-project");
    std::fs::create_dir(&directory).unwrap();

    for command in COMMANDS {
        let help = invoke(&directory, &[command, "--help"]);
        let text = String::from_utf8(help.stdout).unwrap();
        let advertised = options(&text, "\nOptions:\n");
        for (option, value) in OPTIONS {
            let mut arguments = vec![*command, *option];
            arguments.extend(*value);
            arguments.push("--help");
            let result = invoke(&directory, &arguments);
            let is_advertised = advertised.iter().any(|name| name == option);
            assert_eq!(
                result.status.success(),
                is_advertised,
                "{command} {option}: {}",
                String::from_utf8_lossy(&result.stderr)
            );
        }
    }

    let build = String::from_utf8(invoke(&directory, &["build", "--help"]).stdout).unwrap();
    assert!(build.contains("--output requires --format json"));
    assert!(build.contains("JSON output cannot be combined with --verbose or --quiet"));
    let run = String::from_utf8(invoke(&directory, &["run", "--help"]).stdout).unwrap();
    assert!(run.contains("[-- RUNNER-ARGS...]"));
    assert!(run.contains("--vm-profile FILE"));
    let explain = String::from_utf8(invoke(&directory, &["explain", "--help"]).stdout).unwrap();
    assert!(explain.contains("explain OPERATION-ID"));
    let compare =
        String::from_utf8(invoke(&directory, &["compare-compilation-db", "--help"]).stdout)
            .unwrap();
    assert!(compare.contains("--generated compares a file"));
}
