// Copyright 3Bian Limited and Contributors
// SPDX-License-Identifier: MIT

use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
};

use serde::de::DeserializeOwned;

use crate::diagnostic::{Diagnostic, Result};
use board_inheritance::{BoardDirectories, BoardPaths, apply_board_overlays, load_board};
use configuration_value::ConfigurationValue;
use configuration_value_kind::ConfigurationValueKind;
use project_includes::load_included_project;
use toml_input::read_toml_document;

pub mod resolution;

mod board_inheritance;
mod configuration_value;
mod configuration_value_kind;
mod environment;
mod patches;
mod project_includes;
mod project_overlays;
mod project_settings;
mod toml_input;
mod workspace;

mod allocator_debug_level;
mod audit_board;
mod benchmark_metric;
mod build_mode;
mod build_options;
mod cargo_package;
mod compiler_flag;
mod component_recipe;
mod component_role;
mod composed_project;
mod forced_include;
mod generator;
mod heap_region;
mod interrupt;
mod interrupt_source;
mod memory_extent;
mod packaging_kind;
mod raw_board;
mod raw_build;
mod raw_clock_source;
mod raw_firmware;
mod raw_linker_fragment;
mod raw_memory_extent;
mod raw_project;
mod raw_runner;
mod raw_sdk_compatibility;
mod raw_source;
mod raw_source_settings;
mod raw_target;
mod raw_test_suite;
mod raw_thread;
mod raw_toolchain;
mod resolved_board;
mod resolved_build_options;
mod resolved_clock_source;
mod resolved_firmware;
mod resolved_linker_fragment;
mod resolved_project;
mod resolved_runner;
mod resolved_source;
mod resolved_target;
mod resolved_test_suite;
mod resolved_thread;
mod revoker_kind;
mod runner_kind;
mod sdk_compatibility;
mod sdk_component_kind;
mod target_kind;
mod toolchain;
mod toolchain_resolution;

pub use allocator_debug_level::AllocatorDebugLevel;
pub use audit_board::AuditBoard;
pub use benchmark_metric::BenchmarkMetric;
pub use build_mode::BuildMode;
pub use build_options::BuildOptions;
pub use cargo_package::CargoPackage;
pub use compiler_flag::CompilerFlag;
pub use component_recipe::ComponentRecipe;
pub use component_role::ComponentRole;
pub(crate) use composed_project::ComposedProject;
pub use environment::{EmptyEnvironmentValue, read_environment};
pub use forced_include::ForcedInclude;
pub use generator::{RawGenerator, ResolvedGenerator};
pub use heap_region::HeapRegion;
pub use interrupt::Interrupt;
pub use interrupt_source::InterruptSource;
pub use memory_extent::MemoryExtent;
pub use packaging_kind::PackagingKind;
pub use project_settings::ProjectSettings;
pub use raw_board::RawBoard;
pub use raw_build::RawBuild;
pub use raw_clock_source::RawClockSource;
pub use raw_firmware::RawFirmware;
pub use raw_linker_fragment::RawLinkerFragment;
pub use raw_memory_extent::RawMemoryExtent;
pub use raw_project::RawProject;
pub use raw_runner::RawRunner;
pub use raw_sdk_compatibility::{RawSdkCompatibility, RawSdkRequirement};
pub use raw_source::RawSource;
pub use raw_source_settings::RawSourceSettings;
pub use raw_target::RawTarget;
pub use raw_test_suite::{RawTestCase, RawTestSuite};
pub use raw_thread::RawThread;
pub use raw_toolchain::RawToolchain;
pub use resolved_board::ResolvedBoard;
pub use resolved_build_options::ResolvedBuildOptions;
pub use resolved_clock_source::ResolvedClockSource;
pub use resolved_firmware::ResolvedFirmware;
pub use resolved_linker_fragment::{LinkerFragmentSource, ResolvedLinkerFragment};
pub use resolved_project::ResolvedProject;
pub use resolved_runner::ResolvedRunner;
pub use resolved_source::ResolvedSource;
pub use resolved_target::ResolvedTarget;
pub use resolved_test_suite::{ResolvedTestCase, ResolvedTestSuite};
pub use resolved_thread::ResolvedThread;
pub use revoker_kind::RevokerKind;
pub use runner_kind::RunnerKind;
pub use sdk_compatibility::SdkCompatibility;
pub use sdk_component_kind::SdkComponentKind;
pub use target_kind::TargetKind;
pub use toolchain::Toolchain;
pub use workspace::{Workspace, read_workspace, select_project};

fn deserialize_configuration<T: DeserializeOwned>(node: &ConfigurationValue) -> Result<T> {
    let json_value = node.value();
    serde_path_to_error::deserialize(json_value).map_err(|error| {
        let mut field_node = node;
        for segment in error.path().iter() {
            field_node = match (segment, &field_node.kind) {
                (
                    serde_path_to_error::Segment::Map { key },
                    ConfigurationValueKind::Mapping(mapping),
                ) => mapping.get(key).unwrap_or(field_node),
                (
                    serde_path_to_error::Segment::Seq { index },
                    ConfigurationValueKind::Sequence(values),
                ) => values.get(*index).unwrap_or(field_node),
                _ => field_node,
            };
        }
        let message = error.inner().to_string();
        if let Some(key) = message
            .strip_prefix("unknown field `")
            .and_then(|text_value| text_value.split('`').next())
            && let ConfigurationValueKind::Mapping(mapping) = &field_node.kind
        {
            field_node = mapping.get(key).unwrap_or(field_node)
        }
        Diagnostic::new(message).at(field_node.origin.clone())
    })
}

pub fn canonicalize_path(path: &Path) -> Result<PathBuf> {
    fs::canonicalize(path).map_err(|error| Diagnostic::io(path, error))
}

pub(crate) fn compose_project(
    path: &Path,
    selected_firmware: Option<&str>,
    settings: &ProjectSettings,
) -> Result<ComposedProject> {
    let path = canonicalize_path(path)?;
    let root = path
        .parent()
        .ok_or_else(|| Diagnostic::new("project file needs a directory"))?
        .to_path_buf();
    let mut files = Vec::new();
    let mut node = load_included_project(&path, &mut BTreeSet::new(), &mut files)?;
    project_overlays::apply_overlays(&mut node, &settings.overlays, &mut files)?;
    apply_project_settings(&mut node, settings)?;
    environment::expand_project_environment(&mut node)?;
    let raw: RawProject = deserialize_configuration(&node)?;
    let mut origins = BTreeMap::new();
    node.origins("", &mut origins);
    let mut names = BTreeSet::new();
    for (index, firmware) in raw.firmwares.iter().enumerate() {
        if !names.insert(&firmware.name) {
            return Err(
                Diagnostic::new(format!("duplicate firmware name {}", firmware.name)).at(origins
                    .get(&format!("/firmwares/{index}/name"))
                    .cloned()
                    .unwrap_or_else(|| node.origin.clone())),
            );
        }
    }
    let index = match selected_firmware {
        Some(name) => raw
            .firmwares
            .iter()
            .position(|firmware| firmware.name == name)
            .ok_or_else(|| {
                Diagnostic::new(format!(
                    "unknown firmware {name}; available firmware: {}",
                    raw.firmwares
                        .iter()
                        .map(|firmware| firmware.name.as_str())
                        .collect::<Vec<_>>()
                        .join(", ")
                ))
            })?,
        None if raw.firmwares.len() == 1 => 0,
        None if raw.firmwares.is_empty() => {
            return Err(Diagnostic::new(
                "project must declare at least one firmware",
            ));
        }
        None => {
            return Err(Diagnostic::new(format!(
                "project has multiple firmware targets; select one with --firmware: {}",
                raw.firmwares
                    .iter()
                    .map(|firmware| firmware.name.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            )));
        }
    };
    let firmware = raw.firmwares[index].clone();
    let firmware_prefix = format!("/firmwares/{index}");
    for (field, source) in origins.clone() {
        if let Some(suffix) = field.strip_prefix(&firmware_prefix)
            && (suffix.is_empty() || suffix.starts_with('/'))
        {
            origins.insert(format!("/firmware{suffix}"), source);
        }
    }
    let sdk_origin = node
        .map()?
        .get("sdk")
        .map(|number| &number.origin)
        .unwrap_or(&node.origin);
    let sdk =
        crate::configuration::resolution::resolve_path_value(&raw.sdk, sdk_origin, &root, &root)?;
    let board_origin = node
        .map()?
        .get("board")
        .map(|number| &number.origin)
        .unwrap_or(&node.origin);
    let board_selection_origin = board_origin.clone();
    let board_path = if !raw.board.contains(['/', '\\'])
        && !raw.board.ends_with(".toml")
        && !raw.board.contains("${")
    {
        sdk.join("boards").join(format!("{}.toml", raw.board))
    } else {
        crate::configuration::resolution::resolve_path_value(&raw.board, board_origin, &root, &sdk)?
    };
    let board_path =
        canonicalize_path(&board_path).map_err(|error| error.at(board_origin.clone()))?;
    let selected_directory = board_path
        .parent()
        .ok_or_else(|| Diagnostic::new("board file needs a directory").at(board_origin.clone()))?
        .to_path_buf();
    let (mut board_node, base_directory) = load_board(
        &board_path,
        &mut BTreeSet::new(),
        &mut files,
        &root,
        &sdk,
        &selected_directory,
    )
    .map_err(|error| {
        if error.origin.is_none() {
            error.at(board_origin.clone())
        } else {
            error
        }
    })?;
    let board_directories = BoardDirectories {
        selected_file: board_path.clone(),
        selected: selected_directory,
        base: base_directory,
    };
    apply_board_overlays(
        &mut board_node,
        &raw.board_overlays,
        &origins,
        &mut files,
        &BoardPaths {
            project: &root,
            sdk: &sdk,
            directories: &board_directories,
        },
    )?;
    let board: RawBoard = deserialize_configuration(&board_node)?;
    board_node.origins("/board", &mut origins);
    origins.insert("/board/selection".into(), board_selection_origin);
    Ok(ComposedProject {
        raw,
        firmware,
        board,
        context: resolution::ResolutionContext {
            root,
            board_directories,
            origins,
            files,
        },
    })
}

fn apply_project_settings(node: &mut ConfigurationValue, settings: &ProjectSettings) -> Result<()> {
    fn set(
        node: &mut ConfigurationValue,
        key: &str,
        value: String,
        origin: &crate::diagnostic::Origin,
    ) -> Result<()> {
        node.map_mut()?.insert(
            key.into(),
            ConfigurationValue {
                origin: origin.clone(),
                kind: ConfigurationValueKind::Scalar(serde_json::Value::String(value)),
            },
        );
        Ok(())
    }
    if let Some((value, origin)) = &settings.board {
        set(node, "board", value.clone(), origin)?;
    }
    if let Some((value, origin)) = &settings.mode {
        set(node, "mode", value.clone(), origin)?;
    }
    if let Some((value, origin)) = &settings.build_directory {
        set(node, "build_dir", value.display().to_string(), origin)?;
    }
    if let Some((value, origin)) = &settings.output_directory {
        set(node, "output_dir", value.display().to_string(), origin)?;
    }
    if let Some((value, origin)) = &settings.toolchain_directory {
        set(node, "toolchain_dir", value.display().to_string(), origin)?;
    }
    if settings.rust_compiler.is_some() || settings.rust_flags.is_some() {
        let origin = settings
            .rust_compiler
            .as_ref()
            .map(|(_, origin)| origin)
            .or_else(|| settings.rust_flags.as_ref().map(|(_, origin)| origin))
            .ok_or_else(|| Diagnostic::new("missing Rust setting origin"))?;
        let toolchain = node
            .map_mut()?
            .entry("toolchain".into())
            .or_insert_with(|| ConfigurationValue {
                origin: origin.clone(),
                kind: ConfigurationValueKind::Mapping(BTreeMap::new()),
            });
        if let Some((value, origin)) = &settings.rust_compiler {
            set(toolchain, "rust_compiler", value.clone(), origin)?;
        }
        if let Some((values, origin)) = &settings.rust_flags {
            toolchain.map_mut()?.insert(
                "rust_flags".into(),
                ConfigurationValue {
                    origin: origin.clone(),
                    kind: ConfigurationValueKind::Sequence(
                        values
                            .iter()
                            .map(|value| ConfigurationValue {
                                origin: origin.clone(),
                                kind: ConfigurationValueKind::Scalar(serde_json::Value::String(
                                    value.clone(),
                                )),
                            })
                            .collect(),
                    ),
                },
            );
        }
    }
    Ok(())
}
