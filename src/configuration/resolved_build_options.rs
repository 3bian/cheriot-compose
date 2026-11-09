// Copyright 3Bian Limited and Contributors
// SPDX-License-Identifier: MIT

use std::collections::BTreeMap;

use serde::Serialize;

use super::{AllocatorDebugLevel, BenchmarkMetric, InterruptSource};

#[derive(Clone, Debug, Serialize)]
pub struct ResolvedBuildOptions {
    pub allocator: bool,
    pub allocator_rendering: bool,
    pub scheduler_accounting: bool,
    pub scheduler_multiwaiter: bool,
    pub print_floats: bool,
    pub print_doubles: bool,
    pub debug_allocator: AllocatorDebugLevel,
    pub debug_loader: bool,
    pub debug_scheduler: bool,
    pub debug_token_library: bool,
    pub stack_usage_check_allocator: bool,
    pub stack_usage_check_scheduler: bool,
    pub testing_model_output: bool,
    pub benchmark_metric: BenchmarkMetric,
    pub interrupt_source: InterruptSource,
    pub component_debug: BTreeMap<String, bool>,
}
