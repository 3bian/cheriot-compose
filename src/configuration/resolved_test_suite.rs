// Copyright 3Bian Limited and Contributors
// SPDX-License-Identifier: MIT

use serde::Serialize;

mod resolved_test_case;
pub use resolved_test_case::ResolvedTestCase;

#[derive(Clone, Debug, Serialize)]
pub struct ResolvedTestSuite {
    pub runner: String,
    pub cases: Vec<ResolvedTestCase>,
}
