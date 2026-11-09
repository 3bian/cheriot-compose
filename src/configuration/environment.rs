// Copyright 3Bian Limited and Contributors
// SPDX-License-Identifier: MIT

use super::{ConfigurationValue, ConfigurationValueKind};
use crate::diagnostic::{Diagnostic, Result};

#[derive(Clone, Copy)]
pub enum EmptyEnvironmentValue {
    Allow,
    Reject,
}

pub fn read_environment(name: &str, empty: EmptyEnvironmentValue) -> Result<Option<String>> {
    match std::env::var(name) {
        Ok(value) if value.is_empty() && matches!(empty, EmptyEnvironmentValue::Reject) => {
            Err(Diagnostic::new(format!("{name} must not be empty")))
        }
        Ok(value) => Ok(Some(value)),
        Err(std::env::VarError::NotPresent) => Ok(None),
        Err(std::env::VarError::NotUnicode(_)) => {
            Err(Diagnostic::new(format!("{name} must be valid UTF-8")))
        }
    }
}

pub(super) fn expand_project_environment(node: &mut ConfigurationValue) -> Result<()> {
    expand(node, "")
}

fn expand(node: &mut ConfigurationValue, field: &str) -> Result<()> {
    match &mut node.kind {
        ConfigurationValueKind::Mapping(fields) => {
            for (name, child) in fields {
                expand(child, &format!("{field}/{name}"))?;
            }
        }
        ConfigurationValueKind::Sequence(values) => {
            for (index, child) in values.iter_mut().enumerate() {
                expand(child, &format!("{field}/{index}"))?;
            }
        }
        ConfigurationValueKind::Scalar(serde_json::Value::String(value))
            if value.contains("${env:") =>
        {
            let allowed = [
                "/sdk",
                "/board",
                "/mode",
                "/build_dir",
                "/output_dir",
                "/toolchain_dir",
                "/toolchain/uf2_converter",
                "/toolchain/rust_compiler",
                "/toolchain/cargo",
            ];
            if !allowed.contains(&field) {
                return Err(Diagnostic::new(
                    "environment references are not supported in this field",
                )
                .at(node.origin.clone()));
            }
            let name = value
                .strip_prefix("${env:")
                .and_then(|text| text.strip_suffix('}'))
                .ok_or_else(|| {
                    Diagnostic::new("environment reference must occupy the whole value")
                        .at(node.origin.clone())
                })?;
            if !super::resolution::is_identifier(name) {
                return Err(
                    Diagnostic::new("invalid environment variable name").at(node.origin.clone())
                );
            }
            let replacement = read_environment(name, EmptyEnvironmentValue::Reject)
                .map_err(|error| error.at(node.origin.clone()))?
                .ok_or_else(|| {
                    Diagnostic::new(format!("environment variable {name} is missing"))
                        .at(node.origin.clone())
                })?;
            node.origin.expanded_from_environment(name);
            *value = replacement;
        }
        _ => {}
    }
    Ok(())
}
