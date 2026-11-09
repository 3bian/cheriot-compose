// Copyright 3Bian Limited and Contributors
// SPDX-License-Identifier: MIT

use std::collections::BTreeMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use super::{
    BuildAction, BuildCommand, BuildOperationKind, CommandResolution, OperationPolicy,
    calculate_fingerprint,
};
use crate::diagnostic::Origin;
use crate::diagnostic::{Diagnostic, Result};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BuildOperation {
    pub id: String,
    pub kind: BuildOperationKind,
    pub inputs: Vec<PathBuf>,
    pub outputs: Vec<PathBuf>,
    pub dependencies: Vec<String>,
    pub action: BuildAction,
    #[serde(default)]
    pub policy: OperationPolicy,
    pub depfile: Option<PathBuf>,
    pub provenance: Vec<Origin>,
    pub fingerprint: String,
}

impl BuildOperation {
    pub fn update_fingerprint(&mut self) -> Result<()> {
        self.fingerprint.clear();
        self.fingerprint = calculate_fingerprint(&serde_json::to_vec(self)?);
        Ok(())
    }

    pub fn command_resolution(
        &self,
        probes: &BTreeMap<String, Option<bool>>,
    ) -> Result<Option<CommandResolution>> {
        let BuildAction::Command {
            command,
            conditional,
        } = &self.action
        else {
            return Ok(None);
        };
        let Some(condition) = conditional else {
            return Ok(Some(CommandResolution::Concrete(command.clone())));
        };
        if condition.insert_at > command.argv.len() {
            return Err(Diagnostic::new("conditional argument position is invalid"));
        }
        let mut when_true = command.clone();
        when_true.argv.splice(
            condition.insert_at..condition.insert_at,
            condition.arguments.clone(),
        );
        let resolution = match probes.get(&condition.probe).copied().flatten() {
            Some(true) => CommandResolution::Concrete(when_true),
            Some(false) => CommandResolution::Concrete(command.clone()),
            None => CommandResolution::Alternatives {
                probe: condition.probe.clone(),
                when_false: command.clone(),
                when_true,
            },
        };
        Ok(Some(resolution))
    }

    pub fn resolve_command(
        &self,
        probes: &BTreeMap<String, Option<bool>>,
    ) -> Result<Option<BuildCommand>> {
        match self.command_resolution(probes)? {
            None => Ok(None),
            Some(CommandResolution::Concrete(command)) => Ok(Some(command)),
            Some(CommandResolution::Alternatives { probe, .. }) => Err(Diagnostic::new(format!(
                "compiler probe {probe} is unresolved"
            ))),
        }
    }
}
