// Copyright 3Bian Limited and Contributors
// SPDX-License-Identifier: MIT

use std::collections::BTreeMap;

use super::is_identifier;
use crate::configuration::board_inheritance::BoardPaths;
use crate::configuration::{RawBoard, ResolvedBoard, ResolvedRunner};
use crate::diagnostic::{Diagnostic, Origin, Result};

pub(super) fn resolve_board(
    input_board: RawBoard,
    values: super::BoardValues,
    paths: &BoardPaths<'_>,
    origins: &BTreeMap<String, Origin>,
) -> Result<ResolvedBoard> {
    let at = |field_path: &str| {
        origins
            .get(field_path)
            .cloned()
            .unwrap_or_else(|| Origin::derived(field_path))
    };
    let fail = |field_path: &str, message: &str| Diagnostic::new(message).at(at(field_path));
    let super::BoardValues {
        instruction_memory,
        data_memory,
        devices,
        mmio_start,
        mmio_end,
        revocable_start,
        flags,
    } = values;
    let includes = input_board
        .driver_includes
        .iter()
        .enumerate()
        .map(|(index, include)| {
            let origin = at(&format!("/board/driver_includes/{index}"));
            let path = paths.resolve(include, &origin)?;
            if !path.is_dir() {
                return Err(Diagnostic::new(format!(
                    "board include directory does not exist: {}",
                    path.display()
                ))
                .at(origin));
            }
            Ok(crate::diagnostic::Sourced::new(path, origin))
        })
        .collect::<Result<Vec<_>>>()?;
    let runner =
        input_board
            .runner
            .map(|runner| {
                let executable_origin = at("/board/runner/executable");
                if runner.executable.is_empty() || runner.executable.contains('\0') {
                    return Err(Diagnostic::new(
                        "runner executable must not be empty or contain NUL",
                    )
                    .at(executable_origin));
                }
                let sdk_runner = runner.executable.contains("${sdk}")
                    || runner.executable.contains("${sdkboards}")
                    || runner.executable.contains("${board}")
                    || runner.executable.contains("${boardBase}");
                let executable =
                    if runner.executable.contains('/') || runner.executable.contains("${") {
                        let path = paths.resolve(&runner.executable, &executable_origin)?;
                        if sdk_runner && !path.is_file() {
                            return Err(Diagnostic::new(format!(
                                "board runner file does not exist: {}; check the RTOS checkout",
                                path.display()
                            ))
                            .at(executable_origin));
                        }
                        path.to_string_lossy().into_owned()
                    } else {
                        runner.executable
                    };
                let arguments = runner
                    .arguments
                    .into_iter()
                    .enumerate()
                    .map(|(index, argument)| {
                        let origin = at(&format!("/board/runner/arguments/{index}"));
                        if argument.contains('\0') {
                            return Err(
                                Diagnostic::new("runner argument cannot contain NUL").at(origin)
                            );
                        }
                        Ok(crate::diagnostic::Sourced::new(argument, origin))
                    })
                    .collect::<Result<Vec<_>>>()?;
                for (name, value) in &runner.environment {
                    if !is_identifier(name) || value.contains('\0') {
                        return Err(fail(
                            "/board/runner/environment",
                            "invalid runner environment variable",
                        ));
                    }
                }
                if runner.failure_patterns.iter().any(|pattern| {
                    pattern.is_empty() || pattern.len() > 1024 || pattern.contains('\0')
                }) {
                    return Err(fail(
                        "/board/runner/failure_patterns",
                        "runner failure patterns must be nonempty text up to 1024 bytes",
                    ));
                }
                Ok(ResolvedRunner {
                    kind: runner.kind,
                    executable,
                    arguments,
                    failure_patterns: runner.failure_patterns,
                    environment: runner.environment,
                })
            })
            .transpose()?;
    if runner
        .as_ref()
        .is_some_and(|value| value.kind == crate::configuration::RunnerKind::MicrosoftSafe)
        && input_board.packaging != Some(crate::configuration::PackagingKind::MicrosoftSafe)
    {
        return Err(fail(
            "/board/runner",
            "Microsoft SAFE runner requires microsoft_safe packaging",
        ));
    }
    let linker_fragments = super::linker_fragments::resolve_linker_fragments(
        input_board.linker_fragments,
        paths,
        origins,
    )?;
    Ok(ResolvedBoard {
        heap_start: input_board
            .heap
            .start
            .map(|start| format!("0x{start:x}"))
            .unwrap_or_else(|| ".".into()),
        name: input_board.name,
        cpu: input_board.cpu.unwrap_or_else(|| "cheriot".into()),
        heap: input_board.heap,
        revoker: input_board.revoker,
        interrupts: input_board.interrupts,
        defines: input_board.defines,
        cxflags: input_board.cxflags,
        rtos_defines: input_board.rtos_defines,
        timer_hz: input_board.timer_hz,
        tickrate_hz: input_board.tickrate_hz,
        simulation: input_board.simulation,
        runner,
        packaging: input_board.packaging,
        linker_fragments,
        devices,
        instruction_memory,
        data_memory,
        mmio_start,
        mmio_end,
        revocable_start,
        flags,
        includes,
    })
}
