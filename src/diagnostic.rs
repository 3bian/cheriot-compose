// Copyright 3Bian Limited and Contributors
// SPDX-License-Identifier: MIT

use std::{fmt, path::Path};

use serde::Serialize;

mod origin;
mod sourced;

pub use origin::{Origin, OriginExpansion, OriginSource};
pub use sourced::Sourced;

pub type Result<T> = std::result::Result<T, Diagnostic>;
#[derive(Clone, Debug, Serialize)]
pub struct Diagnostic {
    pub message: String,
    pub origin: Option<Box<Origin>>,
}

impl Diagnostic {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            origin: None,
        }
    }

    pub fn at(mut self, origin: Origin) -> Self {
        self.origin = Some(Box::new(origin));
        self
    }

    pub fn io(path: &Path, error: impl fmt::Display) -> Self {
        Self::new(format!("{}: {error}", path.display()))
    }
}

impl fmt::Display for Diagnostic {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(origin) = &self.origin {
            write!(
                formatter,
                "{}:{}:{} [{}]: ",
                origin.file, origin.line, origin.column, origin.field
            )?;
        }
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for Diagnostic {}
impl From<serde_json::Error> for Diagnostic {
    fn from(error: serde_json::Error) -> Self {
        Self::new(error.to_string())
    }
}
