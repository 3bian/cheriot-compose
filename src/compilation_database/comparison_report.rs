// Copyright 3Bian Limited and Contributors
// SPDX-License-Identifier: MIT

use serde::Serialize;

use super::{CompilationDifference, CompilationEntry};

#[derive(Debug, Serialize)]
pub struct ComparisonReport {
    pub reference_count: usize,
    pub generated_count: usize,
    pub missing: Vec<CompilationEntry>,
    pub extra: Vec<CompilationEntry>,
    pub different: Vec<CompilationDifference>,
    pub ambiguous: Vec<String>,
    pub conditional: Vec<String>,
}

impl ComparisonReport {
    pub fn matches(&self) -> bool {
        self.is_match()
    }

    pub fn is_match(&self) -> bool {
        self.missing.is_empty()
            && self.extra.is_empty()
            && self.different.is_empty()
            && self.ambiguous.is_empty()
            && self.conditional.is_empty()
    }
}
