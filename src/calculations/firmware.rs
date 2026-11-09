// Copyright 3Bian Limited and Contributors
// SPDX-License-Identifier: MIT

use std::collections::BTreeMap;

use crate::configuration::resolution::{is_identifier, is_target_name};
use crate::configuration::{
    BuildOptions, ComponentRecipe, ComponentRole, RawFirmware, ResolvedBuildOptions,
    ResolvedProject, ResolvedThread, SdkComponentKind,
};
use crate::diagnostic::{Diagnostic, Origin, Result};

pub(crate) fn resolve_options(
    firmware: &RawFirmware,
    options: &BuildOptions,
    origins: &BTreeMap<String, Origin>,
) -> Result<ResolvedBuildOptions> {
    let origin_for = |field_path: &str| {
        origins
            .get(field_path)
            .cloned()
            .unwrap_or_else(|| Origin::derived(field_path))
    };
    let fail =
        |field_path: &str, message: &str| Diagnostic::new(message).at(origin_for(field_path));
    if !is_target_name(&firmware.name) {
        return Err(fail("/firmware/name", "invalid firmware name"));
    }
    if !options.allocator && options.allocator_rendering {
        return Err(fail(
            "/options/allocator_rendering",
            "allocator rendering requires the allocator",
        ));
    }
    if !options.allocator && options.scheduler_multiwaiter == Some(true) {
        return Err(fail(
            "/options/scheduler_multiwaiter",
            "scheduler multiwaiter requires the allocator",
        ));
    }
    let resolved = ResolvedBuildOptions {
        allocator: options.allocator,
        allocator_rendering: options.allocator_rendering,
        scheduler_accounting: options.scheduler_accounting,
        scheduler_multiwaiter: options.scheduler_multiwaiter.unwrap_or(options.allocator),
        print_floats: options.print_floats,
        print_doubles: options.print_doubles,
        debug_allocator: options.debug_allocator,
        debug_loader: options.debug_loader,
        debug_scheduler: options.debug_scheduler,
        debug_token_library: options.debug_token_library,
        stack_usage_check_allocator: options.stack_usage_check_allocator,
        stack_usage_check_scheduler: options.stack_usage_check_scheduler,
        testing_model_output: options.testing_model_output,
        benchmark_metric: options.benchmark_metric,
        interrupt_source: options.interrupt_source,
        component_debug: options.component_debug.clone(),
    };
    Ok(resolved)
}

pub(crate) fn merge_shared_objects(
    shared_objects: &mut BTreeMap<String, u32>,
    values: &BTreeMap<String, u32>,
) -> Result<()> {
    for (name, size) in values {
        if !is_identifier(name) {
            return Err(Diagnostic::new(format!(
                "invalid shared-object name {name}"
            )));
        }
        if let Some(old) = shared_objects.insert(name.clone(), *size)
            && old != *size
        {
            return Err(Diagnostic::new(format!(
                "shared object {name} has conflicting sizes {old} and {size}"
            )));
        }
    }
    Ok(())
}

pub(crate) fn shared_objects(
    firmware: &RawFirmware,
    threads: &[ResolvedThread],
    origins: &BTreeMap<String, Origin>,
) -> Result<BTreeMap<String, u32>> {
    let origin_for = |field_path: &str| {
        origins
            .get(field_path)
            .cloned()
            .unwrap_or_else(|| Origin::derived(field_path))
    };
    let fail =
        |field_path: &str, message: &str| Diagnostic::new(message).at(origin_for(field_path));
    let mut shared_objects = BTreeMap::from([
        ("allocator_epoch".into(), 4),
        (
            "allocator_hazard_pointers".into(),
            (threads.len() as u32)
                .checked_mul(16)
                .ok_or_else(|| fail("/firmware/threads", "hazard pointer size overflow"))?,
        ),
    ]);
    merge_shared_objects(&mut shared_objects, &firmware.shared_objects)
        .map_err(|error| error.at(origin_for("/firmware/shared_objects")))?;
    Ok(shared_objects)
}

pub(crate) fn finish_project(
    project: &mut ResolvedProject,
    recipes: Vec<ComponentRecipe>,
) -> Result<()> {
    if !project.clock_sources.is_empty()
        && !recipes
            .iter()
            .any(|recipe| recipe.role == Some(ComponentRole::WallClock))
    {
        return Err(
            Diagnostic::new("clock sources require a reachable wall-clock component").at(project
                .provenance
                .get("/clock_sources")
                .cloned()
                .unwrap_or_else(|| Origin::derived("/clock_sources"))),
        );
    }
    for recipe in &recipes {
        if let Some(target) = project.targets.get(&recipe.name) {
            merge_shared_objects(&mut project.shared_objects, &target.shared_objects)?;
        }
    }
    for (thread_index, thread) in project.threads.iter().enumerate() {
        if !recipes.iter().any(|recipe| {
            recipe.name == thread.requested.compartment
                && recipe.kind == SdkComponentKind::Compartment
        }) {
            return Err(Diagnostic::new(format!(
                "thread compartment {} is not a reachable compartment",
                thread.requested.compartment
            ))
            .at(project
                .provenance
                .get(&format!("/firmware/threads/{thread_index}/compartment"))
                .cloned()
                .unwrap_or_else(|| Origin::derived("thread reachability"))));
        }
    }
    project.components = recipes;
    Ok(())
}
