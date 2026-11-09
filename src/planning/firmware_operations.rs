// Copyright 3Bian Limited and Contributors
// SPDX-License-Identifier: MIT

use std::{collections::BTreeSet, path::PathBuf};

use super::component_outputs::ComponentOutputs;
use super::{create_command, create_operation};
use crate::build_graph::{
    BuildAction, BuildOperationKind, BuildPlan, OutputHandling, path_to_string,
};
use crate::calculations::sdk;
use crate::configuration::{ComponentRecipe, SdkComponentKind};
use crate::configuration::{LinkerFragmentSource, ResolvedProject};
use crate::diagnostic::{Origin, Result};

pub(super) fn add_firmware_operations(
    project: &ResolvedProject,
    recipes: &[ComponentRecipe],
    generated: Vec<String>,
    component_outputs: ComponentOutputs,
    plan: &mut BuildPlan,
) -> Result<()> {
    let ComponentOutputs { objects, producers } = component_outputs;
    let firmware = project.build_dir.join(format!(
        "{}--{}.elf",
        project.firmware.name, project.configuration_identity
    ));
    let mut inputs = vec![
        project.build_dir.join("mmio.ldscript"),
        project
            .build_dir
            .join(format!("{}-firmware.ldscript", project.firmware.name)),
    ];
    let mut arguments = vec![
        "-n".into(),
        format!("--script={}", inputs[0].display()),
        format!("--script={}", inputs[1].display()),
        "--relax".into(),
        "-o".into(),
        path_to_string(&firmware)?,
    ];
    let mut outputs = vec![firmware.clone()];
    if project.firmware.compartment_report {
        let report = PathBuf::from(format!("{}.json", firmware.display()));
        arguments.push(format!("--compartment-report={}", report.display()));
        outputs.push(report)
    }
    let mut parents = generated;
    parents.push("generate.board.json".into());
    let mut direct_objects = BTreeSet::new();
    if let Some(switcher_objects) = objects.get("cheriot.switcher") {
        for object_file in switcher_objects {
            arguments.push(path_to_string(object_file)?);
            inputs.push(object_file.clone());
            direct_objects.insert(object_file.clone());
        }
        parents.extend(producers["cheriot.switcher"].clone());
    }
    for name in project
        .firmware
        .services
        .iter()
        .chain(&project.firmware.libraries)
    {
        if let Some(root_objects) = objects.get(name) {
            for object_file in root_objects {
                if direct_objects.insert(object_file.clone()) {
                    arguments.push(path_to_string(object_file)?);
                    inputs.push(object_file.clone());
                }
            }
            if let Some(root_producers) = producers.get(name) {
                parents.extend(root_producers.iter().cloned());
            }
        }
    }
    for recipe in sdk::link_input_order(recipes) {
        if !matches!(
            recipe.kind,
            SdkComponentKind::Object | SdkComponentKind::Group
        ) {
            let component_output = super::component_output(project, recipe);
            arguments.push(path_to_string(&component_output)?);
            inputs.push(component_output);
            parents.extend(producers[&recipe.name].clone());
        }
    }
    for extension in ["rocode", "rwdata"] {
        inputs.push(project.build_dir.join(format!(
            "{}-firmware.{extension}.ldscript",
            project.firmware.name
        )))
    }
    for fragment in &project.board.linker_fragments {
        match &fragment.source {
            LinkerFragmentSource::Inline { .. } => (),
            LinkerFragmentSource::File { path } => inputs.push(path.clone()),
            LinkerFragmentSource::Generated { name, .. } => {
                inputs.push(project.build_dir.join(name))
            }
        }
    }
    let mut link = create_operation(
        "link.firmware".into(),
        BuildOperationKind::Link,
        BuildAction::Command {
            command: create_command(project, project.toolchain.linker.clone(), arguments),
            conditional: None,
        },
        inputs,
        outputs,
        parents,
        vec![Origin::derived("CAL-20")],
    )?;
    link.policy.output_handling = OutputHandling::Fresh;
    plan.operations.push(link);
    let dump = PathBuf::from(format!("{}.dump", firmware.display()));
    let mut dump_command = create_command(
        project,
        project.toolchain.object_dump.clone(),
        vec![
            "-glxsdrS".into(),
            "--demangle".into(),
            path_to_string(&firmware)?,
        ],
    );
    dump_command.stdout = Some(dump.clone());
    plan.operations.push(create_operation(
        "dump.firmware".into(),
        BuildOperationKind::PostProcess,
        BuildAction::Command {
            command: dump_command,
            conditional: None,
        },
        vec![firmware.clone()],
        vec![dump],
        vec!["link.firmware".into()],
        vec![Origin::derived("CAL-20")],
    )?);
    let publication = plan.publication.as_ref().ok_or_else(|| {
        crate::diagnostic::Diagnostic::new("firmware publication destination is missing")
    })?;
    let mut published_inputs = vec![firmware.clone()];
    if project.firmware.compartment_report {
        published_inputs.push(PathBuf::from(format!("{}.json", firmware.display())));
    }
    plan.operations.push(create_operation(
        "publish.firmware".into(),
        BuildOperationKind::PostProcess,
        BuildAction::Publish,
        published_inputs,
        publication.outputs(),
        vec!["link.firmware".into(), "dump.firmware".into()],
        vec![Origin::derived(
            "final firmware and compartment report publication",
        )],
    )?);
    Ok(())
}
