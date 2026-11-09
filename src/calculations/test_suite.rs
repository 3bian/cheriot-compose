// Copyright 3Bian Limited and Contributors
// SPDX-License-Identifier: MIT

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

use crate::configuration::resolution::is_identifier;
use crate::configuration::{
    RawFirmware, RawProject, RawSource, ResolvedTestCase, ResolvedTestSuite, TargetKind,
};
use crate::diagnostic::{Diagnostic, Origin, Result};

pub(crate) fn resolve_test_suite(
    raw: &mut RawProject,
    firmware: &RawFirmware,
    origins: &mut BTreeMap<String, Origin>,
    mut source_inputs: Vec<Option<Result<PathBuf>>>,
) -> Result<Option<ResolvedTestSuite>> {
    let Some(suite) = raw.test_suite.take() else {
        return Ok(None);
    };
    let at = |origins: &BTreeMap<String, Origin>, field: &str| {
        origins
            .get(field)
            .cloned()
            .unwrap_or_else(|| Origin::derived(field))
    };
    let suite_origin = at(origins, "/test_suite");
    if !firmware.services.contains(&suite.runner) || !raw.services.contains_key(&suite.runner) {
        return Err(
            Diagnostic::new("the test runner must be a selected firmware service").at(suite_origin),
        );
    }
    let mut selection = BTreeMap::new();
    let mut targets = BTreeSet::new();
    for (index, case) in suite.cases.iter().enumerate() {
        let origin = at(origins, &format!("/test_suite/cases/{index}/name"));
        if !is_identifier(&case.name)
            || selection
                .insert(
                    case.name.clone(),
                    case.enabled
                        .unwrap_or(suite.most_tests && case.include_in_most),
                )
                .is_some()
        {
            return Err(Diagnostic::new("test names must be unique C identifiers").at(origin));
        }
        if !targets.insert(case.target.clone()) {
            return Err(Diagnostic::new("each test must have its own target").at(origin));
        }
        if case.target == suite.runner {
            return Err(Diagnostic::new("a test target cannot be the test runner").at(origin));
        }
    }
    let mut cases = Vec::new();
    for (index, case) in suite.cases.into_iter().enumerate() {
        let prefix = format!("/test_suite/cases/{index}");
        let origin = at(origins, &format!("{prefix}/name"));
        let enabled = selection
            .get(&case.name)
            .copied()
            .ok_or_else(|| Diagnostic::new("test selection is missing").at(origin.clone()))?;
        for conflict in &case.conflicts {
            match selection.get(conflict) {
                None => {
                    return Err(
                        Diagnostic::new(format!("unknown conflicting test {conflict}")).at(origin),
                    );
                }
                Some(true) if enabled => {
                    return Err(Diagnostic::new(format!(
                        "test {} conflicts with test {conflict}",
                        case.name
                    ))
                    .at(origin));
                }
                _ => {}
            }
        }
        if case.run && !case.prototype {
            return Err(Diagnostic::new("a test without a prototype cannot be called").at(origin));
        }
        let display_name = case
            .display_name
            .clone()
            .unwrap_or_else(|| case.name.clone());
        if display_name.chars().any(char::is_control) {
            return Err(
                Diagnostic::new("test display names cannot contain control characters").at(origin),
            );
        }
        let target = if case.compartment {
            raw.services.get_mut(&case.target)
        } else {
            raw.libraries.get_mut(&case.target)
        }
        .ok_or_else(|| {
            Diagnostic::new(format!(
                "test {} references missing target {}",
                case.name, case.target
            ))
            .at(origin.clone())
        })?;
        let expected = if case.compartment {
            TargetKind::Compartment
        } else {
            TargetKind::Group
        };
        if target.kind != expected || (case.compartment && !case.absorbed_sources.is_empty()) {
            return Err(Diagnostic::new(
                "test target kind or absorbed sources do not match its compartment setting",
            )
            .at(origin));
        }
        if case.firmware_rust_source.is_some() {
            let source_origin = at(origins, &format!("{prefix}/firmware_rust_source"));
            let source = source_inputs
                .get_mut(index)
                .and_then(Option::take)
                .ok_or_else(|| {
                    Diagnostic::new("Rust test source was not supplied").at(source_origin.clone())
                })??;
            if enabled {
                let index = target.build.sources.len();
                target
                    .build
                    .sources
                    .push(RawSource::Path(source.display().to_string()));
                let section = if case.compartment {
                    "services"
                } else {
                    "libraries"
                };
                origins.insert(
                    format!("/{section}/{}/build/sources/{index}", case.target),
                    source_origin,
                );
            }
        }
        if enabled {
            for (option, define) in [
                ("print_floats", "CHERIOT_PRINT_FLOATS"),
                ("print_doubles", "CHERIOT_PRINT_DOUBLES"),
            ] {
                let is_enabled = if option == "print_floats" {
                    raw.options.print_floats
                } else {
                    raw.options.print_doubles
                };
                if is_enabled {
                    if case.compartment {
                        let definition_index = target.build.defines.len();
                        target.build.defines.push(define.into());
                        origins.insert(
                            format!("/services/{}/build/defines/{definition_index}", case.target),
                            at(origins, &format!("/options/{option}")),
                        );
                    }
                    if !target.dependencies.iter().any(|name| name == "softfloat") {
                        target.dependencies.push("softfloat".into());
                    }
                }
            }
            let runner = raw
                .services
                .get_mut(&suite.runner)
                .ok_or_else(|| Diagnostic::new("test runner is missing"))?;
            if !runner.dependencies.contains(&case.target) {
                runner.dependencies.push(case.target.clone());
            }
            for (source_index, source) in case.absorbed_sources.iter().enumerate() {
                let index = runner.build.sources.len();
                runner.build.sources.push(RawSource::Path(source.clone()));
                origins.insert(
                    format!("/services/{}/build/sources/{index}", suite.runner),
                    at(
                        origins,
                        &format!("{prefix}/absorbed_sources/{source_index}"),
                    ),
                );
            }
        }
        cases.push(ResolvedTestCase {
            name: case.name,
            target: case.target,
            display_name,
            enabled,
            compartment: case.compartment,
            prototype: case.prototype,
            run: case.run,
            origin,
        });
    }
    Ok(Some(ResolvedTestSuite {
        runner: suite.runner,
        cases,
    }))
}
