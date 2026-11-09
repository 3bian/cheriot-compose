// Copyright 3Bian Limited and Contributors
// SPDX-License-Identifier: MIT

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::cli) enum OutputFormat {
    Human,
    Text,
    Json,
    Toml,
    Shell,
    CompilationDatabase,
    Bytes,
}

impl OutputFormat {
    pub fn name(self) -> &'static str {
        match self {
            Self::Human => "human",
            Self::Text => "text",
            Self::Json => "json",
            Self::Toml => "toml",
            Self::Shell => "shell",
            Self::CompilationDatabase => "compilation-database",
            Self::Bytes => "bytes",
        }
    }
}
