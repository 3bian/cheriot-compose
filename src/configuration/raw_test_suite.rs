// Copyright 3Bian Limited and Contributors
// SPDX-License-Identifier: MIT

use serde::{Deserialize, Serialize};

use super::build_options::enabled_by_default;

mod raw_test_case;
pub use raw_test_case::RawTestCase;

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RawTestSuite {
    pub runner: String,
    #[serde(default = "enabled_by_default")]
    pub most_tests: bool,
    pub cases: Vec<RawTestCase>,
}
