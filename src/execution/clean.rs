// Copyright 3Bian Limited and Contributors
// SPDX-License-Identifier: MIT

use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
};

use super::{
    BuildLock, BuildPlan, Diagnostic, Result, calculate_fingerprint, check_build_identity,
    guarded_output, load_state, outputs_owned_by_other_builds, publication,
};

pub fn clean_build_plan(plan: &BuildPlan) -> Result<usize> {
    plan.validate()?;
    if !plan.build_dir.exists() {
        if let Some(publication) = &plan.publication
            && fs::symlink_metadata(&publication.marker).is_ok()
        {
            let _lock = publication::PublicationLock::acquire(publication)?;
            publication::preflight(plan, publication)?;
            return publication::clean(publication);
        }
        return Ok(0);
    }
    let _lock = BuildLock::acquire(&plan.build_dir)?;
    check_build_identity(plan, false)?;
    let shared_outputs = outputs_owned_by_other_builds(plan)?;
    let mut outputs = BTreeSet::new();
    let state_path = plan.state_path();
    if let Some(state) = load_state(plan)? {
        outputs.extend(state.owned_outputs);
    }
    let mut publications = BTreeMap::new();
    let mut registries = BTreeSet::new();
    if let Some(selected) = &plan.publication {
        let prefix = format!(".publication-{}-", selected.identity);
        let outputs_prefix = format!(".outputs-{}-", selected.identity);
        for entry in
            fs::read_dir(&plan.build_dir).map_err(|error| Diagnostic::io(&plan.build_dir, error))?
        {
            let entry = entry.map_err(|error| Diagnostic::io(&plan.build_dir, error))?;
            let path = entry.path();
            let Some(name) = entry.file_name().to_str().map(str::to_owned) else {
                continue;
            };
            if name.starts_with(&outputs_prefix) && name.ends_with(".json") {
                guarded_output(&path, &plan.build_dir)?;
                let contents = fs::read(&path).map_err(|error| Diagnostic::io(&path, error))?;
                let expected_name =
                    format!("{outputs_prefix}{}.json", calculate_fingerprint(&contents));
                if name != expected_name {
                    return Err(Diagnostic::new(format!(
                        "invalid owned-output record {}",
                        path.display()
                    )));
                }
                let saved: crate::build_graph::OwnershipRecord = serde_json::from_slice(&contents)?;
                if saved.owner != selected.owner {
                    return Err(Diagnostic::new(format!(
                        "owned-output record belongs to another project: {}",
                        path.display()
                    )));
                }
                for output in &saved.outputs {
                    guarded_output(output, &plan.build_dir)?;
                    if *output == plan.state_path()
                        || *output == plan.build_dir.join(".cheriot-compose.lock")
                    {
                        return Err(Diagnostic::new(
                            "owned-output record contains a reserved path",
                        ));
                    }
                }
                outputs.extend(saved.outputs);
                outputs.insert(path);
                continue;
            }
            if !name.starts_with(&prefix) || !name.ends_with(".json") {
                continue;
            }
            guarded_output(&path, &plan.build_dir)?;
            let saved: crate::build_graph::Publication = serde_json::from_slice(
                &fs::read(&path).map_err(|error| Diagnostic::io(&path, error))?,
            )?;
            let expected = crate::build_graph::Publication::new(
                &plan.build_dir,
                &saved.output_dir,
                &selected.firmware_name,
                &selected.identity,
                &selected.owner,
                saved.report_enabled,
            );
            if saved != expected
                || saved.registry != path
                || !saved.output_dir.is_absolute()
                || crate::configuration::resolution::normalize_path(&saved.output_dir)
                    != saved.output_dir
            {
                return Err(Diagnostic::new(format!(
                    "invalid publication record {}",
                    path.display()
                )));
            }
            publications.insert(saved.output_dir.clone(), saved);
            registries.insert(path);
        }
        if fs::symlink_metadata(&selected.marker).is_ok() {
            publications
                .entry(selected.output_dir.clone())
                .or_insert_with(|| selected.clone());
        }
    }
    for output in outputs
        .iter()
        .chain(registries.iter())
        .chain(std::iter::once(&state_path))
    {
        guarded_output(output, &plan.build_dir)?;
    }
    let mut publication_locks = Vec::new();
    for publication in publications.values() {
        if publication.output_dir.exists() {
            publication_locks.push(publication::PublicationLock::acquire(publication)?);
        }
        publication::preflight(plan, publication)?;
    }
    let mut count = 0;
    for publication in publications.values() {
        count += publication::clean(publication)?;
    }
    outputs.extend(registries);
    if let Some(path) = plan.identity_path() {
        outputs.insert(path);
    }
    outputs.insert(state_path);
    for output in outputs {
        if shared_outputs.contains(&output) {
            continue;
        }
        match fs::remove_file(&output) {
            Ok(()) => count += 1,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => (),
            Err(error) => return Err(Diagnostic::io(&output, error)),
        }
    }
    Ok(count)
}
