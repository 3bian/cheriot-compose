// Copyright 3Bian Limited and Contributors
// SPDX-License-Identifier: MIT

use crate::build_graph::{BuildAction, BuildCommand, BuildOperationKind, BuildPlan};
use crate::configuration::{BuildMode, ResolvedProject};

pub fn introduction(project: &ResolvedProject, verbose: bool, dry_run: bool) -> Vec<String> {
    let mode = match project.mode {
        BuildMode::Debug => "debug",
        BuildMode::Release => "release",
    };
    let mut lines = vec![format!(
        "{} {} (board: {}, mode: {})",
        if dry_run { "Checking" } else { "Building" },
        project.firmware.name,
        project.board.name,
        mode
    )];
    if verbose {
        let toolchain = &project.toolchain;
        lines.push(format!(
            "Toolchain directory: {} ({})",
            toolchain.directory.display(),
            toolchain.directory_origin.rule
        ));
        lines.push("LLVM tools:".into());
        for (label, executable) in [
            ("C compiler", &toolchain.c_compiler),
            ("C++ compiler", &toolchain.cpp_compiler),
            ("Linker", &toolchain.linker),
            ("Object dump", &toolchain.object_dump),
            ("Object copy", &toolchain.object_copy),
            ("Strip", &toolchain.strip),
        ] {
            lines.push(format!(
                "  {label}: {executable} ({})",
                toolchain.directory_origin.rule
            ));
        }
        lines.push(format!(
            "Optional tools: Cargo {}, UF2 converter {}",
            toolchain.cargo, toolchain.uf2_converter
        ));
        if let Some(rustc) = &toolchain.rust_compiler {
            let origin = toolchain
                .rust_compiler_origin
                .as_ref()
                .map(|origin| origin.file.as_str())
                .unwrap_or("configured toolchain");
            lines.push(format!("Firmware Rust compiler: {rustc} ({origin})"));
        } else {
            lines.push("Firmware Rust compiler: unresolved (set toolchain.rust_compiler or RUSTC if needed)".into());
        }
    }
    lines
}

pub fn stage(kind: &BuildOperationKind, action: &BuildAction) -> Option<&'static str> {
    if matches!(action, BuildAction::GitDescription { .. }) {
        return None;
    }
    Some(match kind {
        BuildOperationKind::Generate => "Generating files",
        BuildOperationKind::Compile => "Compiling sources",
        BuildOperationKind::RustCompile => "Compiling Rust sources",
        BuildOperationKind::CargoBuild => "Building Rust crate",
        BuildOperationKind::Assemble => "Assembling sources",
        BuildOperationKind::PartialLink => "Linking components",
        BuildOperationKind::Link => "Linking firmware",
        BuildOperationKind::PostProcess => "Writing build outputs",
        BuildOperationKind::Probe => "Checking compiler features",
    })
}

pub fn verbose_command(command: &BuildCommand) -> String {
    let mut display = command.clone();
    display.environment.clear();
    super::format_shell_command(&display)
}

pub fn conclusion(
    plan: &BuildPlan,
    succeeded: bool,
    dry_run: bool,
    count: usize,
    skipped: usize,
    elapsed_seconds: f64,
) -> Vec<String> {
    let mut lines = vec![if !succeeded {
        format!("Build failed after {elapsed_seconds:.1}s.")
    } else if dry_run {
        format!("{count} actions would run; {skipped} are up to date.")
    } else if count == 0 {
        format!("Firmware is up to date ({skipped} actions skipped).")
    } else {
        format!(
            "Build succeeded in {elapsed_seconds:.1}s ({count} actions run, {skipped} up to date)."
        )
    }];
    if succeeded && let Some(publication) = &plan.publication {
        lines.push(format!(
            "{} {}",
            if dry_run {
                "Firmware destination:"
            } else {
                "Firmware:"
            },
            publication.elf.display()
        ));
        if publication.report_enabled {
            lines.push(format!(
                "{} {}",
                if dry_run {
                    "Report destination:"
                } else {
                    "Report:"
                },
                publication.report.display()
            ));
        }
    }
    lines
}
