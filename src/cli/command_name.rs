// Copyright 3Bian Limited and Contributors
// SPDX-License-Identifier: MIT

use super::option_catalog;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum CommandName {
    Check,
    Configuration,
    Plan,
    CompareCompilationDatabase,
    Build,
    Run,
    Explain,
    Clean,
    AnalyzeSwitcher,
    Audit,
    Package,
    Size,
}

impl CommandName {
    pub(super) fn parse(name: &str) -> Option<Self> {
        option_catalog::ALL
            .iter()
            .copied()
            .find(|command| command.name() == name)
    }

    pub(super) fn name(self) -> &'static str {
        match self {
            Self::Check => "check",
            Self::Configuration => "configuration",
            Self::Plan => "plan",
            Self::CompareCompilationDatabase => "compare-compilation-database",
            Self::Build => "build",
            Self::Run => "run",
            Self::Explain => "explain",
            Self::Clean => "clean",
            Self::AnalyzeSwitcher => "analyze-switcher",
            Self::Audit => "audit",
            Self::Package => "package",
            Self::Size => "size",
        }
    }
}
