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
            Self::CompareCompilationDatabase => "compare-compilation-db",
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

    pub(super) fn description(self) -> &'static str {
        match self {
            Self::AnalyzeSwitcher => "Check annotated switcher assembly and produce a DOT graph.",
            Self::Audit => "Build firmware and run an independent audit.",
            Self::Build => "Build the selected firmware.",
            Self::Check => "Validate configuration without running compiler tools.",
            Self::Clean => "Remove files recorded as belonging to the selected build.",
            Self::CompareCompilationDatabase => {
                "Compare compiler commands against a reference database."
            }
            Self::Configuration => "Show the resolved configuration.",
            Self::Explain => "Show a build operation and the origins of its inputs.",
            Self::Package => "Build firmware and create its board-specific package.",
            Self::Plan => "Show the complete clean-build plan.",
            Self::Run => "Build as needed, then launch the configured runner.",
            Self::Size => "Report firmware or compartment section sizes.",
        }
    }

    pub(super) fn signature(self) -> String {
        let suffix = match self {
            Self::AnalyzeSwitcher => " --input FILE",
            Self::Audit => " --query QUERY",
            Self::Explain => " OPERATION-ID",
            Self::Size => " [--compartment NAME]",
            _ => "",
        };
        format!("{}{suffix}", self.name())
    }
}
