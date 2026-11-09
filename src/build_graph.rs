// Copyright 3Bian Limited and Contributors
// SPDX-License-Identifier: MIT

use std::path::Path;

use crate::diagnostic::{Diagnostic, Result};

mod build_action;
mod build_command;
mod build_operation;
mod build_operation_kind;
mod build_plan;
mod command_resolution;
mod conditional_arguments;
mod operation_policy;
mod ownership_record;
mod publication;
mod runner_plan;

pub use build_action::BuildAction;
pub use build_command::BuildCommand;
pub use build_operation::BuildOperation;
pub use build_operation_kind::BuildOperationKind;
pub use build_plan::BuildPlan;
pub use command_resolution::CommandResolution;
pub use conditional_arguments::ConditionalArguments;
pub use operation_policy::{FreshnessPolicy, OperationPolicy, OutputHandling, RequiredTool};
pub use ownership_record::OwnershipRecord;
pub use publication::{MARKER_VERSION, Publication};
pub use runner_plan::RunnerPlan;

pub type ProbeResults = std::collections::BTreeMap<String, Option<bool>>;

pub fn calculate_fingerprint(bytes: &[u8]) -> String {
    let mut hash = 0xcbf29ce484222325u64;
    for byte in bytes {
        hash ^= *byte as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    format!("{hash:016x}")
}

pub(crate) fn validate_command(command: &BuildCommand) -> Result<()> {
    if !command.cwd.is_absolute()
        || command.executable.is_empty()
        || command.executable.contains('\0')
        || command
            .argv
            .iter()
            .any(|argument| argument.value.contains('\0'))
    {
        return Err(Diagnostic::new(
            "invalid command, NUL argument or relative cwd",
        ));
    }
    for (name, value) in &command.environment {
        if !crate::configuration::resolution::is_identifier(name) || value.contains('\0') {
            return Err(Diagnostic::new("invalid environment override"));
        }
    }
    for output in command.stdout.iter().chain(command.stderr.iter()) {
        if !output.is_absolute() {
            return Err(Diagnostic::new("redirect paths must be absolute"));
        }
    }
    Ok(())
}

pub fn path_to_string(path: &Path) -> Result<String> {
    path.to_str()
        .map(str::to_string)
        .ok_or_else(|| Diagnostic::new("paths must be UTF-8"))
}
