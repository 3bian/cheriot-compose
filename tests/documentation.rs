// Copyright 3Bian Limited and Contributors
// SPDX-License-Identifier: MIT

use std::path::Path;

#[cfg(feature = "rtos-verification")]
use std::process::Command;

#[cfg(feature = "rtos-verification")]
#[path = "support/rtos_tree.rs"]
mod rtos_tree;

#[test]
fn product_readme_commands_put_the_toolchain_directory_first() {
    check_document_commands(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("README.md")
            .as_path(),
    );
}

#[test]
#[cfg(feature = "rtos-verification")]
fn repository_document_commands_put_the_toolchain_directory_first() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
    for document in [
        "README.md",
        "docs/architecture.md",
        "docs/configuration.md",
        "docs/rtos-integration.md",
        "docs/toolchain.md",
        "docs/verification.md",
    ] {
        check_document_commands(&root.join(document));
    }
}

fn check_document_commands(document: &Path) {
    let project_commands = [
        "check",
        "configuration",
        "plan",
        "build",
        "run",
        "explain",
        "clean",
        "audit",
        "package",
        "size",
        "compare-compilation-database",
    ];
    let text = std::fs::read_to_string(document)
        .unwrap()
        .replace("\\\n", " ");
    for line in text.lines() {
        for occurrence in line.match_indices("cheriot-compose ") {
            let words: Vec<_> = line[occurrence.0..].split_whitespace().collect();
            if words.len() >= 2 && project_commands.contains(&words[1]) {
                assert_eq!(
                    words.get(2).copied(),
                    Some("--toolchain-dir"),
                    "{}: {line}",
                    document.display()
                );
                assert!(words.get(3).is_some(), "{}: {line}", document.display());
            }
        }
    }
}

#[test]
#[cfg(feature = "rtos-verification")]
fn common_readme_commands_match_the_workspace_and_cli() {
    let tree = rtos_tree::RtosTree::new();
    let readme =
        std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("README.md")).unwrap();
    let common = readme
        .split("### Command-Line Options\n")
        .nth(1)
        .unwrap()
        .split("\n## ")
        .next()
        .unwrap()
        .split("#### Common Commands\n")
        .nth(1)
        .unwrap()
        .split("\n#### ")
        .next()
        .unwrap();
    let mut count = 0;
    for line in common
        .lines()
        .filter(|line| line.starts_with("cheriot-compose "))
    {
        let mut args: Vec<_> = line.split_whitespace().skip(1).collect();
        if matches!(args.first(), Some(&"build" | &"run")) {
            args.push("--dry-run");
        }
        let output = Command::new(env!("CARGO_BIN_EXE_cheriot-compose"))
            .current_dir(&tree.root)
            .args(&args)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{line}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        count += 1;
    }
    assert_eq!(count, 5);
}

#[test]
#[cfg(feature = "rtos-verification")]
fn readme_project_configuration_is_valid_in_an_rtos_checkout() {
    let tree = rtos_tree::RtosTree::new();
    let directory = tree.root.join("examples/readme-example");
    std::fs::create_dir_all(&directory).unwrap();
    let readme =
        std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("README.md")).unwrap();
    let project = readme
        .split("## Project Configuration\n")
        .nth(1)
        .unwrap()
        .split("```toml\n")
        .nth(1)
        .unwrap()
        .split("\n```")
        .next()
        .unwrap();
    std::fs::write(directory.join("cheriot-compose.toml"), project).unwrap();
    std::fs::write(directory.join("hello.cc"), "// source fixture\n").unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_cheriot-compose"))
        .current_dir(&directory)
        .args(["check", "--toolchain-dir", "/cheriot-tools/bin"])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn command_reference_in_readme_covers_the_commands_shown_in_help() {
    let readme =
        std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("README.md")).unwrap();
    let help = std::process::Command::new(env!("CARGO_BIN_EXE_cheriot-compose"))
        .arg("help")
        .output()
        .unwrap();
    assert!(help.status.success());
    let help = String::from_utf8(help.stdout).unwrap();
    let commands = help
        .split("Commands:\n")
        .nth(1)
        .unwrap()
        .split("Common options:")
        .next()
        .unwrap();
    for line in commands.lines().filter(|line| line.starts_with("  ")) {
        let name = line.split_whitespace().next().unwrap();
        assert!(
            readme.contains(&format!("`{name}`")),
            "README missing command {name}"
        );
    }
}
