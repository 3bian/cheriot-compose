// Copyright 3Bian Limited and Contributors
// SPDX-License-Identifier: MIT

use serde::Serialize;

use crate::{
    configuration::SdkComponentKind,
    configuration::{ResolvedProject, RevokerKind},
    diagnostic::{Diagnostic, Result},
};

use crate::render::firmware::FirmwareComponent;

use super::{
    component_model::ComponentModel, shared_object_model::SharedObjectModel,
    thread_model::ThreadModel,
};

#[derive(Serialize)]
pub(crate) struct FirmwareModel {
    threads: Vec<ThreadModel>,
    libraries: Vec<ComponentModel>,
    compartments: Vec<ComponentModel>,
    components: Vec<ComponentModel>,
    shared_objects: Vec<SharedObjectModel>,
    software_revoker: bool,
    pub heap_start: String,
    pub thread_count: usize,
    pub library_count: usize,
    pub compartment_count: usize,
}
impl FirmwareModel {
    pub fn new(project: &ResolvedProject, components: &[FirmwareComponent]) -> Result<Self> {
        let mut libraries = Vec::new();
        let mut compartments = Vec::new();
        for kind in [SdkComponentKind::Library, SdkComponentKind::Compartment] {
            for recipe in components.iter().filter(|recipe| recipe.kind == kind) {
                let obj = recipe.object.to_string_lossy().to_string();
                if obj.contains(['"', '\n', '\r', '\\']) {
                    return Err(Diagnostic::new(
                        "linker input path contains unsupported quotes or control characters",
                    ));
                }
                let component = ComponentModel {
                    compartment: recipe.name.clone(),
                    obj,
                };
                if kind == SdkComponentKind::Library {
                    libraries.push(component);
                } else {
                    compartments.push(component);
                }
            }
        }
        let components = libraries.iter().chain(&compartments).cloned().collect();
        Ok(Self {
            threads: project
                .threads
                .iter()
                .map(|thread| ThreadModel {
                    thread_id: thread.id,
                    stack_size: thread.requested.stack_size,
                    trusted_stack_frames: thread.requested.trusted_stack_frames,
                    priority: thread.priority,
                    mangled_entry_point: format!("\"{}\"", thread.entry_symbol),
                })
                .collect(),
            library_count: libraries.len(),
            compartment_count: compartments.len(),
            libraries,
            compartments,
            components,
            shared_objects: project
                .shared_objects
                .iter()
                .map(|(name, size)| SharedObjectModel {
                    global: name.clone(),
                    size: *size,
                })
                .collect(),
            software_revoker: project.board.revoker == RevokerKind::Software,
            heap_start: project.board.heap_start.clone(),
            thread_count: project.threads.len(),
        })
    }
}
