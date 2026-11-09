// Copyright 3Bian Limited and Contributors
// SPDX-License-Identifier: MIT

use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use super::{BuildAction, BuildOperation, ProbeResults, Publication, validate_command};
use crate::diagnostic::{Diagnostic, Result};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BuildPlan {
    pub schema: String,
    pub profile: String,
    pub notes: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub toolchain: Option<crate::configuration::Toolchain>,
    pub build_dir: PathBuf,
    pub state_key: Option<String>,
    pub state_owner: Option<String>,
    pub publication: Option<Publication>,
    pub operations: Vec<BuildOperation>,
    pub probes: BTreeSet<String>,
}

impl BuildPlan {
    pub fn state_path(&self) -> PathBuf {
        match &self.state_key {
            Some(key) => self.build_dir.join(format!("state-{key}.json")),
            None => self.build_dir.join("state.json"),
        }
    }

    pub fn identity_path(&self) -> Option<PathBuf> {
        self.state_key
            .as_ref()
            .map(|key| self.build_dir.join(format!(".identity-{key}")))
    }

    pub fn unresolved_probes(&self) -> ProbeResults {
        self.probes.iter().map(|id| (id.clone(), None)).collect()
    }

    pub fn prerequisite_closure(&self, roots: &[String]) -> Result<BTreeSet<String>> {
        let operations: BTreeMap<_, _> = self
            .operations
            .iter()
            .map(|operation| (operation.id.as_str(), operation))
            .collect();
        let mut selected = BTreeSet::new();
        let mut pending = roots.to_vec();
        while let Some(identifier) = pending.pop() {
            if !selected.insert(identifier.clone()) {
                continue;
            }
            let operation = operations
                .get(identifier.as_str())
                .ok_or_else(|| Diagnostic::new(format!("unknown operation {identifier}")))?;
            pending.extend(operation.dependencies.iter().cloned());
        }
        Ok(selected)
    }

    pub fn artifact_producer(&self, path: &std::path::Path) -> Option<&BuildOperation> {
        self.operations.iter().find(|operation| {
            operation
                .outputs
                .iter()
                .chain(operation.depfile.iter())
                .any(|output| output == path)
        })
    }

    pub fn validate(&self) -> Result<()> {
        if !self.build_dir.is_absolute()
            || crate::configuration::resolution::normalize_path(&self.build_dir) != self.build_dir
        {
            return Err(Diagnostic::new(
                "build directory must be an absolute, normalized path",
            ));
        }
        let mut identifiers = BTreeSet::new();
        let mut publication_count = 0;
        let mut ownership_count = 0;
        if self
            .state_key
            .as_ref()
            .is_some_and(|key| key.len() != 16 || !key.bytes().all(|byte| byte.is_ascii_hexdigit()))
        {
            return Err(Diagnostic::new("invalid build state identity"));
        }
        if self.state_key.as_deref()
            != self
                .state_owner
                .as_ref()
                .map(|owner| crate::build_graph::calculate_fingerprint(owner.as_bytes()))
                .as_deref()
        {
            return Err(Diagnostic::new(
                "build state owner does not match its identity",
            ));
        }
        if let Some(publication) = &self.publication
            && (publication.identity.len() != 16
                || !publication
                    .identity
                    .bytes()
                    .all(|byte| byte.is_ascii_hexdigit())
                || !crate::configuration::resolution::is_target_name(&publication.firmware_name)
                || !publication.output_dir.is_absolute()
                || crate::build_graph::calculate_fingerprint(publication.owner.as_bytes())
                    != publication.identity
                || self.state_key.as_deref() != Some(publication.identity.as_str())
                || self.state_owner.as_deref() != Some(publication.owner.as_str())
                || crate::configuration::resolution::normalize_path(&publication.output_dir)
                    != publication.output_dir
                || *publication
                    != Publication::new(
                        &self.build_dir,
                        &publication.output_dir,
                        &publication.firmware_name,
                        &publication.identity,
                        &publication.owner,
                        publication.report_enabled,
                    ))
        {
            return Err(Diagnostic::new("invalid final artifact destination"));
        }
        let mut outputs = BTreeSet::new();
        let producers: BTreeMap<_, _> = self
            .operations
            .iter()
            .flat_map(|candidate_operation| {
                candidate_operation
                    .outputs
                    .iter()
                    .chain(candidate_operation.depfile.iter())
                    .map(move |output_path| (output_path, &candidate_operation.id))
            })
            .collect();
        let mut ancestors: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
        for operation in &self.operations {
            if matches!(operation.action, BuildAction::Probe { .. })
                && !operation
                    .policy
                    .required_tools
                    .contains(&super::RequiredTool::ProbeCompiler)
            {
                return Err(Diagnostic::new(
                    "compiler probe must declare its required executable",
                ));
            }
            if operation.policy.output_handling == super::OutputHandling::Fresh
                && !matches!(operation.action, BuildAction::Command { .. })
            {
                return Err(Diagnostic::new("fresh outputs require a command action"));
            }
            if !operation.policy.required_tools.is_empty()
                && !matches!(
                    operation.action,
                    BuildAction::Command { .. } | BuildAction::Probe { .. }
                )
            {
                return Err(Diagnostic::new(
                    "required tools need a command or probe action",
                ));
            }
            if operation.policy.output_handling == super::OutputHandling::OwnershipRecord {
                ownership_count += 1;
                let BuildAction::Generate { content, .. } = &operation.action else {
                    return Err(Diagnostic::new("invalid owned-output action"));
                };
                let record: super::OwnershipRecord = serde_json::from_str(content)
                    .map_err(|_| Diagnostic::new("invalid owned-output record"))?;
                let Some(owner) = &self.state_owner else {
                    return Err(Diagnostic::new("owned-output record has no owner"));
                };
                let expected = self.build_dir.join(format!(
                    ".outputs-{}-{}.json",
                    crate::build_graph::calculate_fingerprint(owner.as_bytes()),
                    crate::build_graph::calculate_fingerprint(content.as_bytes()),
                ));
                if record.owner != *owner
                    || operation.outputs.as_slice() != [expected]
                    || ownership_count > 1
                {
                    return Err(Diagnostic::new("invalid owned-output record identity"));
                }
            }
            if !crate::configuration::resolution::is_target_name(&operation.id)
                || !identifiers.insert(operation.id.clone())
            {
                return Err(Diagnostic::new(format!(
                    "invalid or duplicate operation id {}",
                    operation.id
                )));
            }
            for dependency in &operation.dependencies {
                if dependency == &operation.id || !identifiers.contains(dependency) {
                    return Err(Diagnostic::new(format!(
                        "{} has unordered, missing or cyclic prerequisite {dependency}",
                        operation.id
                    )));
                }
            }
            let mut reachable = BTreeSet::new();
            for dependency in &operation.dependencies {
                reachable.insert(dependency.clone());
                if let Some(prior) = ancestors.get(dependency) {
                    reachable.extend(prior.clone());
                }
            }
            for input in &operation.inputs {
                if let Some(producer) = producers.get(input)
                    && !reachable.contains(*producer)
                {
                    return Err(Diagnostic::new(format!(
                        "{} reads {} without a dependency on {producer}",
                        operation.id,
                        input.display()
                    )));
                }
                if input.starts_with(&self.build_dir) && !producers.contains_key(input) {
                    return Err(Diagnostic::new(format!(
                        "{} reads generated input {} without a producer",
                        operation.id,
                        input.display()
                    )));
                }
            }
            ancestors.insert(operation.id.clone(), reachable);
            for output in operation.outputs.iter().chain(operation.depfile.iter()) {
                let reserved_metadata = output.parent() == Some(self.build_dir.as_path())
                    && output
                        .file_name()
                        .and_then(|name| name.to_str())
                        .is_some_and(|name| {
                            (name == "state.json"
                                || name.starts_with("state-") && name.ends_with(".json")
                                || name.starts_with(".publication-")
                                || name.starts_with(".outputs-")
                                || name.starts_with(".identity-"))
                                && !(matches!(operation.action, BuildAction::Publish)
                                    && self
                                        .publication
                                        .as_ref()
                                        .is_some_and(|publication| *output == publication.registry))
                                && !(operation.policy.output_handling
                                    == super::OutputHandling::OwnershipRecord
                                    && name.starts_with(".outputs-"))
                        });
                if !output.is_absolute()
                    || crate::configuration::resolution::normalize_path(output) != *output
                    || (!output.starts_with(&self.build_dir)
                        && !(matches!(operation.action, BuildAction::Publish)
                            && self
                                .publication
                                .as_ref()
                                .is_some_and(|publication| publication.outputs().contains(output))))
                    || output == &self.build_dir
                    || *output == self.state_path()
                    || *output == self.build_dir.join(".cheriot-compose.lock")
                    || reserved_metadata
                    || !outputs.insert(output.clone())
                {
                    return Err(Diagnostic::new(format!(
                        "duplicate or relative output {}",
                        output.display()
                    )));
                }
            }
            match &operation.action {
                BuildAction::Publish => {
                    publication_count += 1;
                    let publication = self
                        .publication
                        .as_ref()
                        .ok_or_else(|| Diagnostic::new("publication has no destination"))?;
                    if operation.outputs != publication.outputs()
                        || operation.inputs.len() != if publication.report_enabled { 2 } else { 1 }
                        || operation.depfile.is_some()
                        || operation
                            .inputs
                            .iter()
                            .any(|input| !input.starts_with(&self.build_dir))
                    {
                        return Err(Diagnostic::new("invalid firmware publication action"));
                    }
                }
                BuildAction::Copy { input } => {
                    if operation.outputs.len() != 1
                        || !operation.inputs.contains(input)
                        || operation.outputs.contains(input)
                    {
                        return Err(Diagnostic::new(
                            "copy needs one declared input and a distinct output",
                        ));
                    }
                }
                BuildAction::Generate { .. } => {
                    if operation.outputs.len() != 1 {
                        return Err(Diagnostic::new("generation needs one output"));
                    }
                }
                BuildAction::MemoryImage { input, word_bytes } => {
                    if operation.outputs.len() != 1
                        || !operation.inputs.contains(input)
                        || ![4, 8].contains(word_bytes)
                    {
                        return Err(Diagnostic::new(
                            "memory image needs a declared input, one output and 4- or 8-byte words",
                        ));
                    }
                }
                BuildAction::GitDescription { command } => {
                    validate_command(command)?;
                    if operation.outputs.len() != 1
                        || command.stdout.is_some()
                        || command.stderr.is_some()
                    {
                        return Err(Diagnostic::new(
                            "Git description needs one generated header and no redirection",
                        ));
                    }
                }
                BuildAction::Command {
                    command,
                    conditional,
                } => {
                    validate_command(command)?;
                    if command
                        .stdout
                        .iter()
                        .chain(command.stderr.iter())
                        .any(|path| !operation.outputs.contains(path))
                        || command.stdout.is_some() && command.stdout == command.stderr
                    {
                        return Err(Diagnostic::new(format!(
                            "{} redirects output to an undeclared or shared path",
                            operation.id
                        )));
                    }
                    if let Some(condition) = conditional
                        && (condition.insert_at > command.argv.len()
                            || !operation.dependencies.contains(&condition.probe)
                            || !self.probes.contains(&condition.probe))
                    {
                        return Err(Diagnostic::new("invalid conditional command"));
                    }
                }
                BuildAction::Probe { command } => {
                    validate_command(command)?;
                    if !self.probes.contains(&operation.id) {
                        return Err(Diagnostic::new("probe state missing"));
                    }
                }
            }
        }
        if self.publication.is_some() && publication_count != 1 {
            return Err(Diagnostic::new(
                "firmware plan needs exactly one publication action",
            ));
        }
        Ok(())
    }
}
