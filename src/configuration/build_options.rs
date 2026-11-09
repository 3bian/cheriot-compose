// Copyright 3Bian Limited and Contributors
// SPDX-License-Identifier: MIT

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use super::{AllocatorDebugLevel, BenchmarkMetric, InterruptSource};

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct BuildOptions {
    #[serde(default = "enabled_by_default")]
    pub allocator: bool,
    pub allocator_rendering: bool,
    pub scheduler_accounting: bool,
    pub scheduler_multiwaiter: Option<bool>,
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

pub(super) fn enabled_by_default() -> bool {
    true
}

impl Default for BuildOptions {
    fn default() -> Self {
        Self {
            allocator: true,
            allocator_rendering: false,
            scheduler_accounting: false,
            scheduler_multiwaiter: None,
            print_floats: false,
            print_doubles: false,
            debug_allocator: AllocatorDebugLevel::None,
            debug_loader: false,
            debug_scheduler: false,
            debug_token_library: false,
            stack_usage_check_allocator: false,
            stack_usage_check_scheduler: false,
            testing_model_output: false,
            benchmark_metric: BenchmarkMetric::default(),
            interrupt_source: InterruptSource::default(),
            component_debug: BTreeMap::new(),
        }
    }
}
