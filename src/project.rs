// Copyright 3Bian Limited and Contributors
// SPDX-License-Identifier: MIT

use std::path::Path;

use crate::calculations::{board, firmware, sdk, test_suite, threads};
use crate::configuration::{self, ProjectSettings, ResolvedProject};
use crate::diagnostic::Result;

pub fn load_project(path: &Path) -> Result<ResolvedProject> {
    load_project_for_firmware(path, None)
}

pub fn load_project_for_firmware(
    path: &Path,
    selected_firmware: Option<&str>,
) -> Result<ResolvedProject> {
    load_project_with_settings(path, selected_firmware, &ProjectSettings::default())
}

pub fn load_project_with_settings(
    path: &Path,
    selected_firmware: Option<&str>,
    settings: &ProjectSettings,
) -> Result<ResolvedProject> {
    let composed = configuration::compose_project(path, selected_firmware, settings)?;
    let options = firmware::resolve_options(
        &composed.firmware,
        &composed.raw.options,
        &composed.context.origins,
    )?;
    let prepared = configuration::resolution::prepare(
        composed.raw,
        composed.firmware,
        composed.board,
        composed.context,
        options,
    )?;
    let board_values = board::resolve_board(&prepared.input_board, &prepared.origins)?;
    let mut input = configuration::resolution::load_board(prepared, board_values)?;
    let test_sources = configuration::resolution::load_test_suite_sources(
        &input.raw,
        &input.root,
        &input.sdk,
        &input.origins,
    );
    let test_suite = test_suite::resolve_test_suite(
        &mut input.raw,
        &input.firmware,
        &mut input.origins,
        test_sources,
    )?;
    let loaded = configuration::resolution::load_project_inputs(input, test_suite)?;
    let (resolved_threads, max_priority) =
        threads::resolve_threads(&loaded.firmware, &loaded.origins)?;
    let shared_objects =
        firmware::shared_objects(&loaded.firmware, &resolved_threads, &loaded.initial_origins)?;
    let mut project = configuration::resolution::finish_project_inputs(
        loaded,
        resolved_threads,
        max_priority,
        shared_objects,
    )?;
    let recipes = sdk::resolve_dependencies(&project)?;
    configuration::resolution::validate_reachable_inputs(&project, &recipes)?;
    firmware::finish_project(&mut project, recipes)?;
    Ok(project)
}
