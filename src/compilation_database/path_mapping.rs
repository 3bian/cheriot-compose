// Copyright 3Bian Limited and Contributors
// SPDX-License-Identifier: MIT

use crate::diagnostic::{Diagnostic, Result};

#[derive(Clone, Debug)]
pub struct PathMapping {
    pub old: String,
    pub new: String,
}

impl PathMapping {
    pub fn parse(mapping: &str) -> Result<Self> {
        let (old, new) = mapping
            .split_once('=')
            .ok_or_else(|| Diagnostic::new("path mapping must be OLD=NEW"))?;
        if old.is_empty() || new.is_empty() {
            return Err(Diagnostic::new("path mapping roots must not be empty"));
        }
        Ok(Self {
            old: old.trim_end_matches('/').into(),
            new: new.trim_end_matches('/').into(),
        })
    }
}
