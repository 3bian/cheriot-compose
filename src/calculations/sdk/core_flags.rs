// Copyright 3Bian Limited and Contributors
// SPDX-License-Identifier: MIT

use crate::configuration::{CompilerFlag, ComponentRole, ResolvedProject};
use crate::diagnostic::Origin;

fn derived(value: impl Into<String>, rule: &str) -> CompilerFlag {
    CompilerFlag {
        value: value.into(),
        origin: Origin::derived(rule),
    }
}

fn option(project: &ResolvedProject, name: &str, value: String) -> CompilerFlag {
    let origin = project
        .provenance
        .get(&format!("/options/{name}"))
        .or_else(|| {
            (name == "scheduler_multiwaiter")
                .then(|| project.provenance.get("/options/allocator"))
                .flatten()
        })
        .cloned()
        .unwrap_or_else(|| Origin::derived(&format!("SDK option {name}")));
    CompilerFlag { value, origin }
}

fn board_defines(project: &ResolvedProject, role: &str, flags: &mut Vec<CompilerFlag>) {
    if let Some(definitions) = project.board.rtos_defines.get(role) {
        flags.extend(definitions.iter().enumerate().map(|(index, definition)| {
            CompilerFlag {
                value: format!("-D{definition}"),
                origin: project
                    .provenance
                    .get(&format!("/board/rtos_defines/{role}/{index}"))
                    .cloned()
                    .unwrap_or_else(|| Origin::derived("board rtos_defines")),
            }
        }));
    }
}

fn allocator_flags(project: &ResolvedProject) -> Vec<CompilerFlag> {
    let mut flags = vec![
        derived(
            "-DCHERIOT_AVOID_CAPRELOCS",
            "cheriot.privileged-compartment",
        ),
        option(
            project,
            "allocator_rendering",
            format!("-DHEAP_RENDER={}", project.options.allocator_rendering),
        ),
        derived(
            format!(
                "-DREVOKABLE_MEMORY_START=0x{:x}",
                project.board.revocable_start
            ),
            "CAL-06",
        ),
        option(
            project,
            "debug_allocator",
            format!(
                "-DDEBUG_ALLOCATOR=DebugLevel::{}",
                project.options.debug_allocator.sdk_name()
            ),
        ),
        option(
            project,
            "stack_usage_check_allocator",
            format!(
                "-DCHERIOT_STACK_CHECKS_ALLOCATOR={}",
                project.options.stack_usage_check_allocator
            ),
        ),
    ];
    board_defines(project, "allocator", &mut flags);
    flags
}

fn loader_flags(project: &ResolvedProject) -> Vec<CompilerFlag> {
    let mut flags = vec![
        derived("-DCHERIOT_AVOID_CAPRELOCS", "SDK loader rule"),
        derived("-DCHERIOT_NO_AMBIENT_MALLOC", "SDK loader rule"),
        option(
            project,
            "debug_loader",
            format!("-DDEBUG_LOADER={}", project.options.debug_loader),
        ),
    ];
    board_defines(project, "loader", &mut flags);
    flags
}

fn token_flags(project: &ResolvedProject) -> Vec<CompilerFlag> {
    vec![
        derived("-DCHERIOT_NO_AMBIENT_MALLOC", "cheriot.library"),
        option(
            project,
            "debug_token_library",
            format!(
                "-DDEBUG_TOKEN_LIBRARY={}",
                project.options.debug_token_library
            ),
        ),
    ]
}

fn scheduler_flags(project: &ResolvedProject) -> Vec<CompilerFlag> {
    let mut flags = vec![
        derived("-DCHERIOT_AVOID_CAPRELOCS", "sdk/xmake.lua component rules"),
        option(
            project,
            "scheduler_accounting",
            format!(
                "-DSCHEDULER_ACCOUNTING={}",
                project.options.scheduler_accounting
            ),
        ),
        option(
            project,
            "scheduler_multiwaiter",
            format!(
                "-DSCHEDULER_MULTIWAITER={}",
                project.options.scheduler_multiwaiter
            ),
        ),
    ];
    if !project.board.interrupts.is_empty() {
        let interrupts = project
            .board
            .interrupts
            .iter()
            .map(|interrupt| {
                format!(
                    "{{{},{},{}}},",
                    interrupt.number, interrupt.priority, interrupt.edge_triggered
                )
            })
            .collect::<String>();
        flags.push(derived(
            format!("-DCHERIOT_INTERRUPT_CONFIGURATION={interrupts}"),
            "sdk/xmake.lua component rules",
        ));
    }
    flags.extend([
        option(
            project,
            "debug_scheduler",
            format!("-DDEBUG_SCHEDULER={}", project.options.debug_scheduler),
        ),
        option(
            project,
            "stack_usage_check_scheduler",
            format!(
                "-DCHERIOT_STACK_CHECKS_SCHEDULER={}",
                project.options.stack_usage_check_scheduler
            ),
        ),
        derived(
            format!("-DCONFIG_THREADS_NUM={}", project.threads.len()),
            "sdk/xmake.lua component rules",
        ),
        derived(
            format!("-DCONFIG_THREAD_MAX_PRIORITY={}", project.max_priority),
            "sdk/xmake.lua component rules",
        ),
    ]);
    board_defines(project, "scheduler", &mut flags);
    flags
}

pub(super) fn for_role(
    project: &ResolvedProject,
    role: Option<ComponentRole>,
) -> Option<Vec<CompilerFlag>> {
    Some(match role? {
        ComponentRole::Allocator => allocator_flags(project),
        ComponentRole::Loader => loader_flags(project),
        ComponentRole::Scheduler => scheduler_flags(project),
        ComponentRole::TokenLibrary => token_flags(project),
        ComponentRole::Switcher => Vec::new(),
        ComponentRole::SoftwareRevoker => vec![
            derived(
                "-DCHERIOT_AVOID_CAPRELOCS",
                "cheriot.privileged-compartment",
            ),
            derived("-DCHERIOT_NO_AMBIENT_MALLOC", "SDK software revoker rule"),
        ],
        _ => return None,
    })
}
