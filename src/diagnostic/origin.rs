// Copyright 3Bian Limited and Contributors
// SPDX-License-Identifier: MIT

use serde::{Deserialize, Serialize};

mod origin_expansion;
mod origin_source;

pub use origin_expansion::OriginExpansion;
pub use origin_source::OriginSource;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Origin {
    pub file: String,
    pub line: u64,
    pub column: u64,
    pub field: String,
    pub rule: String,
    #[serde(default)]
    pub source: OriginSource,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expansion: Option<Box<OriginExpansion>>,
}

impl Origin {
    pub fn derived(rule: &str) -> Self {
        Self {
            file: "<SDK profile>".into(),
            line: 0,
            column: 0,
            field: String::new(),
            rule: rule.into(),
            source: OriginSource::Derived,
            expansion: None,
        }
    }

    pub fn command_line(option: &str) -> Self {
        let description = format!("command-line option {option}");
        Self {
            file: format!("<{description}>"),
            rule: description,
            source: OriginSource::CommandLine,
            ..Self::derived("")
        }
    }

    pub fn environment(variable: &str) -> Self {
        let description = format!("environment variable {variable}");
        Self {
            file: format!("<{description}>"),
            rule: description,
            source: OriginSource::Environment,
            ..Self::derived("")
        }
    }

    pub fn expanded_from_environment(&mut self, variable: &str) {
        self.rule = format!("environment variable {variable}");
        self.expansion = Some(Box::new(OriginExpansion::Environment {
            variable: variable.into(),
        }));
    }

    pub fn is_literal_environment_value(&self) -> bool {
        self.source == OriginSource::Environment || self.expansion.is_some()
    }
}
