// Copyright 3Bian Limited and Contributors
// SPDX-License-Identifier: MIT

use std::path::Path;

#[cfg(feature = "rtos-verification")]
use std::process::Command;

#[cfg(feature = "rtos-verification")]
#[path = "support/rtos_tree.rs"]
mod rtos_tree;

#[test]
fn product_readme_build_examples_specify_the_toolchain_directory() {
    let readme = std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("README.md"))
        .unwrap()
        .replace("\\\n", " ");
    for line in readme.lines() {
        for occurrence in line.match_indices("cheriot-compose ") {
            let words: Vec<_> = line[occurrence.0..].split_whitespace().collect();
            if matches!(words.get(1), Some(&"build" | &"run"))
                && !words.contains(&"--help")
                && !words.contains(&"-h")
            {
                let index = words
                    .iter()
                    .position(|word| *word == "--toolchain-dir")
                    .unwrap_or_else(|| panic!("missing toolchain directory: {line}"));
                assert!(words.get(index + 1).is_some(), "{line}");
            }
        }
    }
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

#[cfg(feature = "rtos-verification")]
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
        "compare-compilation-db",
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
        .split("### Common Commands\n")
        .nth(1)
        .unwrap()
        .split("\n## ")
        .next()
        .unwrap()
        .replace("\\\n", " ");
    let mut count = 0;
    for line in common
        .lines()
        .filter(|line| line.starts_with("cheriot-compose "))
    {
        let runnable = line.replace("\"$CHERIOT_TOOLS/bin\"", "/cheriot-tools/bin");
        let mut args: Vec<_> = runnable.split_whitespace().skip(1).collect();
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
    assert_eq!(count, 4);
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
        .split("### TOML Projects and Firmware\n")
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
fn general_help_in_the_readme_matches_the_executable() {
    let readme =
        std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("README.md")).unwrap();
    let help = std::process::Command::new(env!("CARGO_BIN_EXE_cheriot-compose"))
        .arg("--help")
        .output()
        .unwrap();
    assert!(help.status.success());
    let help = String::from_utf8(help.stdout).unwrap();
    let documented = readme
        .split("```text\n")
        .nth(1)
        .unwrap()
        .split("\n```")
        .next()
        .unwrap();
    assert_eq!(documented, help.trim_end());
}
