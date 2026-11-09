// Copyright 3Bian Limited and Contributors
// SPDX-License-Identifier: MIT

use serde::Serialize;

use crate::configuration::ResolvedTestSuite;
use crate::diagnostic::Result;

mod header_entry;
mod test_call;

use header_entry::HeaderEntry;
use test_call::TestCall;

#[derive(Serialize)]
pub(crate) struct TestSuiteRenderModel {
    headers: Vec<HeaderEntry>,
    calls: Vec<TestCall>,
}

impl TestSuiteRenderModel {
    pub fn new(suite: &ResolvedTestSuite) -> Result<Self> {
        let mut headers = Vec::new();
        let mut calls = Vec::new();
        for case in &suite.cases {
            if !case.prototype || case.enabled {
                let compartment = if case.prototype && case.compartment {
                    format!(
                        "__cheri_compartment({})",
                        serde_json::to_string(&case.target)?
                    )
                } else {
                    String::new()
                };
                headers.push(HeaderEntry {
                    name: case.name.clone(),
                    constant: !case.prototype,
                    prototype: case.prototype,
                    enabled: u8::from(case.enabled),
                    compartment,
                    separator: "\n",
                });
            }
            if case.enabled && case.run {
                calls.push(TestCall {
                    name: case.name.clone(),
                    display_name: serde_json::to_string(&case.display_name)?,
                    separator: "\n",
                });
            }
        }
        if let Some(last) = headers.last_mut() {
            last.separator = "";
        }
        if let Some(last) = calls.last_mut() {
            last.separator = "";
        }
        Ok(Self { headers, calls })
    }
}
