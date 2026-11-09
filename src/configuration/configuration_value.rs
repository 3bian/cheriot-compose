// Copyright 3Bian Limited and Contributors
// SPDX-License-Identifier: MIT

use std::collections::BTreeMap;

use serde_json::Value;

use super::ConfigurationValueKind;
use crate::diagnostic::{Diagnostic, Origin, Result};

#[derive(Clone, Debug)]
pub(super) struct ConfigurationValue {
    pub(super) origin: Origin,
    pub(super) kind: ConfigurationValueKind,
}

impl ConfigurationValue {
    pub(super) fn value(&self) -> Value {
        match &self.kind {
            ConfigurationValueKind::Scalar(value) => value.clone(),
            ConfigurationValueKind::Mapping(mapping) => Value::Object(
                mapping
                    .iter()
                    .map(|(key, value)| (key.clone(), value.value()))
                    .collect(),
            ),
            ConfigurationValueKind::Sequence(values) => {
                Value::Array(values.iter().map(ConfigurationValue::value).collect())
            }
        }
    }

    pub(super) fn map(&self) -> Result<&BTreeMap<String, ConfigurationValue>> {
        if let ConfigurationValueKind::Mapping(mapping) = &self.kind {
            Ok(mapping)
        } else {
            Err(Diagnostic::new("expected a mapping").at(self.origin.clone()))
        }
    }

    pub(super) fn map_mut(&mut self) -> Result<&mut BTreeMap<String, ConfigurationValue>> {
        if let ConfigurationValueKind::Mapping(mapping) = &mut self.kind {
            Ok(mapping)
        } else {
            Err(Diagnostic::new("expected a mapping").at(self.origin.clone()))
        }
    }

    pub(super) fn string(&self) -> Result<String> {
        self.value()
            .as_str()
            .map(str::to_owned)
            .ok_or_else(|| Diagnostic::new("expected a string").at(self.origin.clone()))
    }

    pub(super) fn origins(&self, path: &str, origins: &mut BTreeMap<String, Origin>) {
        origins.insert(path.into(), self.origin.clone());
        match &self.kind {
            ConfigurationValueKind::Mapping(mapping) => {
                for (key, child) in mapping {
                    child.origins(&format!("{path}/{key}"), origins)
                }
            }
            ConfigurationValueKind::Sequence(values) => {
                for (index, value) in values.iter().enumerate() {
                    value.origins(&format!("{path}/{index}"), origins)
                }
            }
            _ => (),
        }
    }
}
