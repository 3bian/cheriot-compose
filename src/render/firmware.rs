// Copyright 3Bian Limited and Contributors
// SPDX-License-Identifier: MIT

use std::collections::{BTreeMap, BTreeSet};

use crate::{
    configuration::{LinkerFragmentSource, ResolvedProject},
    diagnostic::{Diagnostic, Origin, Result},
    render::template::{self, EscapeMode},
};

pub(crate) mod clock_render_model;
mod firmware_component;
mod firmware_inputs;
mod generated_artifact;
mod linker_segment_render_model;
mod render_models;
pub(crate) mod test_suite_render_model;

pub use firmware_component::FirmwareComponent;
pub use firmware_inputs::FirmwareInputs;
pub use generated_artifact::GeneratedArtifact;
use linker_segment_render_model::LinkerSegmentRenderModel;
use render_models::{
    FirmwareLayoutModel, FirmwareModel, FragmentContext, FragmentSections, InterruptModel,
    MmioModel,
};

fn hexadecimal(value: u32) -> String {
    format!("0x{value:x}")
}

type SectionDependencies = BTreeMap<String, BTreeMap<String, String>>;

fn render_artifact<T: serde::Serialize>(
    project: &ResolvedProject,
    partials: &BTreeMap<String, String>,
    name: &str,
    template: &str,
    model: &T,
) -> Result<GeneratedArtifact> {
    Ok(GeneratedArtifact::render(
        name,
        project.build_dir.join(name),
        template,
        model,
        partials,
        project.config_files.clone(),
        vec![Origin::derived("CAL-04,05,12–17,21")],
    )?
    .0)
}

fn fragment_sections(
    model: &FirmwareModel,
    partials: &BTreeMap<String, String>,
) -> Result<(FragmentSections, SectionDependencies)> {
    let mut dependencies = BTreeMap::new();
    let value = serde_json::to_value(model)?;
    let mut section = |name: &str| -> Result<String> {
        let reference = format!("{{{{>section_{name}}}}}");
        let rendered = template::render_tracked(&reference, &value, partials, EscapeMode::Text)?;
        dependencies.insert(name.to_owned(), rendered.partials);
        Ok(rendered.content)
    };
    let fields = FragmentSections {
        compartment_exports: section("compartment_exports")?,
        library_exports: section("library_exports")?,
        cap_relocs: section("cap_relocs")?,
        compartment_headers: section("compartment_headers")?,
        pcc_ld: section("pcc_ld")?,
        gdc_ld: section("gdc_ld")?,
        software_revoker_code: section("software_revoker_code")?,
        software_revoker_globals: section("software_revoker_globals")?,
        software_revoker_header: section("software_revoker_header")?,
        sealed_objects: section("sealed_objects")?,
        thread_headers: section("thread_headers")?,
        thread_trusted_stacks: section("thread_trusted_stacks")?,
        thread_stacks: section("thread_stacks")?,
        shared_objects: section("shared_objects")?,
    };
    Ok((fields, dependencies))
}

fn add_section_dependencies(
    target: &mut BTreeMap<String, String>,
    fields: &BTreeSet<String>,
    sections: &SectionDependencies,
) {
    for field in fields {
        if let Some(dependencies) = sections.get(field) {
            target.extend(dependencies.clone());
        }
    }
}

pub fn generate(
    project: &ResolvedProject,
    inputs: &FirmwareInputs,
) -> Result<Vec<GeneratedArtifact>> {
    generate_with_partials(project, inputs, &firmware_template_partials())
}

pub fn firmware_template_partials() -> BTreeMap<String, String> {
    template::catalog::load_partials()
}

pub fn generate_with_partials(
    project: &ResolvedProject,
    render_inputs: &FirmwareInputs,
    partials: &BTreeMap<String, String>,
) -> Result<Vec<GeneratedArtifact>> {
    let mut artifacts = Vec::new();
    let inputs = project.config_files.clone();
    let origins = vec![Origin::derived("CAL-04,05,12–17,21")];
    artifacts.push(render_artifact(
        project,
        partials,
        "mmio.ldscript",
        include_str!("../../templates/mmio.mustache"),
        &MmioModel::new(project),
    )?);
    artifacts.push(render_artifact(
        project,
        partials,
        "board-interrupts.h",
        include_str!("../../templates/sdk/board-interrupts.h.mustache"),
        &InterruptModel::new(project),
    )?);
    let firmware = FirmwareModel::new(project, &render_inputs.components)?;
    let name = &project.firmware.name;
    let read_only = format!("{name}-firmware.rocode.ldscript");
    let writable = format!("{name}-firmware.rwdata.ldscript");
    artifacts.push(render_artifact(
        project,
        partials,
        &read_only,
        include_str!("../../templates/sdk/firmware.rocode.ldscript.mustache"),
        &firmware,
    )?);
    artifacts.push(render_artifact(
        project,
        partials,
        &writable,
        include_str!("../../templates/sdk/firmware.rwdata.ldscript.mustache"),
        &firmware,
    )?);
    let (sections, section_dependencies) = fragment_sections(&firmware, partials)?;
    let mut inline_dependencies = BTreeMap::new();
    let mut segments = Vec::new();
    for (index, fragment) in render_inputs.segments.iter().enumerate() {
        let mut segment = LinkerSegmentRenderModel {
            ix: index + 1,
            after_first: index > 0,
            start: hexadecimal(fragment.start),
            start_string: fragment
                .start_expression
                .clone()
                .unwrap_or_else(|| hexadecimal(fragment.start)),
            phtype: fragment.program_header.clone(),
            body: String::new(),
        };
        let include = |path: &std::path::Path| -> Result<String> {
            let path = crate::build_graph::path_to_string(path)?;
            if path.contains(['"', '\n', '\r', '\\']) {
                return Err(Diagnostic::new("unsafe linker include path"));
            }
            Ok(format!("INCLUDE \"{path}\""))
        };
        let context = FragmentContext {
            sections: &sections,
            segment: &segment,
            heap_start: &firmware.heap_start,
            thread_count: firmware.thread_count,
            library_count: firmware.library_count,
            compartment_count: firmware.compartment_count,
        };
        let rendered_body = match &fragment.source {
            LinkerFragmentSource::Inline { body } => {
                let rendered = template::render_tracked(
                    body,
                    &serde_json::to_value(&context)?,
                    partials,
                    EscapeMode::Text,
                )?;
                inline_dependencies.extend(rendered.partials);
                add_section_dependencies(
                    &mut inline_dependencies,
                    &rendered.variables,
                    &section_dependencies,
                );
                inline_dependencies.insert(format!("inline:{index}"), body.clone());
                Ok(rendered.content)
            }
            LinkerFragmentSource::File { path } => include(path),
            LinkerFragmentSource::Generated { template, name } => {
                let text = render_inputs
                    .fragment_templates
                    .get(template)
                    .ok_or_else(|| {
                        Diagnostic::new(format!(
                            "linker fragment template was not supplied: {}",
                            template.display()
                        ))
                    })?;
                let mut fragment_inputs = inputs.clone();
                fragment_inputs.push(template.clone());
                let fragment_origins = fragment.origin.clone().into_iter().collect();
                let (mut artifact, variables) = GeneratedArtifact::render(
                    name,
                    project.build_dir.join(name),
                    text,
                    &context,
                    partials,
                    fragment_inputs,
                    fragment_origins,
                )?;
                add_section_dependencies(
                    &mut artifact.template_dependencies,
                    &variables,
                    &section_dependencies,
                );
                artifacts.push(artifact);
                include(&project.build_dir.join(name))
            }
        }
        .map_err(|error| match &fragment.origin {
            Some(origin) => error.at(origin.clone()),
            None => error,
        })?;
        segment.body = rendered_body;
        segments.push(segment);
    }
    let layout = FirmwareLayoutModel { segments };
    let mut top = render_artifact(
        project,
        partials,
        &format!("{name}-firmware.ldscript"),
        include_str!("../../templates/sdk/firmware.ldscript.mustache"),
        &layout,
    )?;
    top.template_dependencies.extend(inline_dependencies);
    artifacts.push(top);
    let board = toml::to_string_pretty(&project.board)
        .map_err(|error| Diagnostic::new(error.to_string()))?;
    artifacts.push(GeneratedArtifact::literal(
        "board.resolved.toml",
        project.build_dir.join("board.resolved.toml"),
        board,
        inputs,
        origins,
    ));
    Ok(artifacts)
}
