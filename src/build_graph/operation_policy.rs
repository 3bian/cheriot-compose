// Copyright 3Bian Limited and Contributors
// SPDX-License-Identifier: MIT

use serde::{Deserialize, Serialize};

use super::{BuildAction, BuildCommand, BuildOperationKind};
use crate::{configuration::Toolchain, diagnostic::Diagnostic};

mod freshness_policy;
mod output_handling;
mod required_tool;
mod tool_check;

pub use freshness_policy::FreshnessPolicy;
pub use output_handling::OutputHandling;
pub use required_tool::RequiredTool;
use tool_check::ToolCheck;

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct OperationPolicy {
    pub required_tools: Vec<RequiredTool>,
    pub output_handling: OutputHandling,
    pub freshness: FreshnessPolicy,
}

impl OperationPolicy {
    pub fn for_action(kind: &BuildOperationKind, action: &BuildAction) -> Self {
        let mut policy = Self::default();
        if matches!(
            action,
            BuildAction::Command { .. } | BuildAction::Probe { .. }
        ) {
            policy.required_tools.push(RequiredTool::SelectedLlvm);
        }
        if matches!(action, BuildAction::Probe { .. }) {
            policy.required_tools.push(RequiredTool::ProbeCompiler);
        }
        match kind {
            BuildOperationKind::RustCompile => {
                policy.required_tools.push(RequiredTool::FirmwareRust)
            }
            BuildOperationKind::CargoBuild => {
                policy
                    .required_tools
                    .extend([RequiredTool::Cargo, RequiredTool::FirmwareRust]);
                policy.freshness = FreshnessPolicy::CheckCargo;
            }
            _ => (),
        }
        if matches!(action, BuildAction::GitDescription { .. }) {
            policy.freshness = FreshnessPolicy::CheckGitDescription;
        }
        if matches!(
            action,
            BuildAction::Generate { .. }
                | BuildAction::MemoryImage { .. }
                | BuildAction::GitDescription { .. }
                | BuildAction::Publish
                | BuildAction::Copy { .. }
        ) || *kind == BuildOperationKind::CargoBuild
            || *kind == BuildOperationKind::Generate
                && matches!(action, BuildAction::Command { .. })
        {
            policy.output_handling = OutputHandling::CompareContents;
        }
        policy
    }

    pub fn compare_outputs(&self) -> bool {
        matches!(
            self.output_handling,
            OutputHandling::CompareContents | OutputHandling::OwnershipRecord
        )
    }

    pub fn always_rebuild_reason(&self) -> Option<&'static str> {
        match self.freshness {
            FreshnessPolicy::WhenChanged => None,
            FreshnessPolicy::CheckCargo => Some("Cargo checks its own dependencies"),
            FreshnessPolicy::CheckGitDescription => Some("Git description may have changed"),
        }
    }

    pub(crate) fn tool_checks(
        &self,
        toolchain: Option<&Toolchain>,
        command: &BuildCommand,
    ) -> Vec<ToolCheck> {
        let mut checks = Vec::new();
        for tool in &self.required_tools {
            let check = match tool {
                RequiredTool::SelectedLlvm => {
                    toolchain.and_then(|toolchain| toolchain
                        .llvm_error(&command.executable)
                        .map(|error| ToolCheck {
                            executable: Some(command.executable.clone()),
                            error,
                        }))
                }
                RequiredTool::ProbeCompiler => {
                    if checks.iter().any(|check: &ToolCheck| check.executable.as_deref() == Some(command.executable.as_str())) {
                        None
                    } else {
                        Some(ToolCheck { executable: Some(command.executable.clone()), error: Diagnostic::new(format!("missing compiler: {}", command.executable)) })
                    }
                }
                RequiredTool::Cargo => toolchain.map(|toolchain| ToolCheck {
                    executable: Some(command.executable.clone()),
                    error: toolchain.cargo_error(&command.executable),
                }),
                RequiredTool::FirmwareRust => toolchain.map(|toolchain| match &toolchain.rust_compiler {
                    Some(executable) => ToolCheck {
                        executable: Some(executable.clone()),
                        error: toolchain.rust_error(executable),
                    },
                    None => ToolCheck {
                        executable: None,
                        error: Diagnostic::new(
                            "firmware Rust compiler is unresolved; configure toolchain.rust_compiler or set RUSTC to a CHERIoT-capable compiler",
                        ),
                    },
                }),
                RequiredTool::Uf2Converter => toolchain.map(|toolchain| ToolCheck {
                    executable: Some(command.executable.clone()),
                    error: toolchain.uf2_error(&command.executable),
                }),
            };
            if let Some(check) = check {
                checks.push(check);
            }
        }
        checks
    }
}
