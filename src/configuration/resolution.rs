// Copyright 3Bian Limited and Contributors
// SPDX-License-Identifier: MIT

use std::{
    collections::BTreeMap,
    path::{Component, Path, PathBuf},
};

use crate::{
    configuration::{
        BuildMode, RawBoard, RawFirmware, RawProject, ResolvedBuildOptions, ResolvedProject,
    },
    diagnostic::{Diagnostic, Origin, Result},
};

mod board;
mod board_project;
mod board_values;
mod loaded_project;
mod prepared_project;

pub(crate) use board_project::BoardProject;
pub(crate) use board_values::BoardValues;
pub(crate) use loaded_project::LoadedProject;
pub(crate) use prepared_project::PreparedProject;
pub(crate) use test_suite_sources::load_test_suite_sources;
mod linker_fragments;
mod sdk_compatibility;
mod targets;
mod test_suite_sources;

pub fn is_identifier(name: &str) -> bool {
    let mut characters = name.chars();
    characters
        .next()
        .is_some_and(|character| character.is_ascii_alphabetic() || character == '_')
        && characters.all(|character| character.is_ascii_alphanumeric() || character == '_')
}

pub fn is_target_name(name: &str) -> bool {
    !name.is_empty()
        && name.bytes().all(|byte| {
            byte.is_ascii_alphanumeric() || byte == b'_' || byte == b'-' || byte == b'.'
        })
        && !name.starts_with('.')
}

pub fn normalize_path(path: &Path) -> PathBuf {
    let mut normalized_path = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => (),
            Component::ParentDir => {
                normalized_path.pop();
            }
            component => normalized_path.push(component),
        }
    }
    normalized_path
}

pub fn resolve_path_value(
    value: &str,
    origin: &Origin,
    root: &Path,
    sdk: &Path,
) -> Result<PathBuf> {
    let source_directory = Path::new(&origin.file).parent().unwrap_or(root);
    let environment_literal = origin.is_literal_environment_value();
    let expanded_path = if environment_literal {
        value.to_owned()
    } else {
        value
            .replace("${project}", &root.to_string_lossy())
            .replace("${sdkboards}", &sdk.join("boards").to_string_lossy())
            .replace("${sdk}", &sdk.to_string_lossy())
            .replace("${board_dir}", &source_directory.to_string_lossy())
    };
    if (!environment_literal && expanded_path.contains("${"))
        || expanded_path.contains(['*', '?', '[', '\0', '\n', '\r'])
    {
        return Err(Diagnostic::new(
            "unsupported path token or glob; use explicit paths and supported substitutions",
        )
        .at(origin.clone()));
    }
    let resolved_path = Path::new(&expanded_path);
    Ok(normalize_path(&if resolved_path.is_absolute() {
        resolved_path.to_path_buf()
    } else {
        source_directory.join(resolved_path)
    }))
}

pub(crate) struct ResolutionContext {
    pub(super) root: PathBuf,
    pub(super) board_directories: super::board_inheritance::BoardDirectories,
    pub(crate) origins: BTreeMap<String, Origin>,
    pub(super) files: Vec<PathBuf>,
}

pub(crate) fn prepare(
    raw: RawProject,
    firmware: RawFirmware,
    input_board: RawBoard,
    context: ResolutionContext,
    options: ResolvedBuildOptions,
) -> Result<PreparedProject> {
    let ResolutionContext {
        root,
        board_directories,
        mut origins,
        files,
    } = context;
    let initial_origins = origins.clone();
    let origin_for = |field_path: &str| {
        initial_origins
            .get(field_path)
            .cloned()
            .unwrap_or_else(|| Origin::derived(field_path))
    };
    let sdk = resolve_path_value(&raw.sdk, &origin_for("/sdk"), &root, &root)?;
    let sdk_compatibility = sdk_compatibility::validate_sdk_compatibility(
        raw.sdk_compatibility.as_ref(),
        &sdk,
        &origins,
    )?;
    let toolchain = super::toolchain_resolution::resolve(
        &raw.toolchain,
        raw.toolchain_dir.as_deref(),
        &root,
        &sdk,
        &mut origins,
    )?;
    Ok(PreparedProject {
        raw,
        firmware,
        input_board,
        root,
        board_directories,
        origins,
        initial_origins,
        files,
        sdk,
        sdk_compatibility,
        toolchain,
        options,
    })
}

pub(crate) fn load_board(
    prepared: PreparedProject,
    board_values: BoardValues,
) -> Result<BoardProject> {
    let PreparedProject {
        raw,
        firmware,
        input_board,
        root,
        board_directories,
        origins,
        initial_origins,
        files,
        sdk,
        sdk_compatibility,
        toolchain,
        options,
    } = prepared;
    let board_paths = super::board_inheritance::BoardPaths {
        project: &root,
        sdk: &sdk,
        directories: &board_directories,
    };
    let board = board::resolve_board(input_board.clone(), board_values, &board_paths, &origins)?;
    let audit_board = crate::configuration::AuditBoard::new(&input_board, &board);
    Ok(BoardProject {
        raw,
        firmware,
        root,
        board_directories,
        origins,
        initial_origins,
        files,
        sdk,
        sdk_compatibility,
        toolchain,
        options,
        board,
        audit_board,
    })
}

pub(crate) fn load_project_inputs(
    input: BoardProject,
    test_suite: Option<crate::configuration::ResolvedTestSuite>,
) -> Result<LoadedProject> {
    let BoardProject {
        raw,
        firmware,
        root,
        board_directories,
        origins,
        initial_origins,
        files,
        sdk,
        sdk_compatibility,
        toolchain,
        options,
        board,
        audit_board,
    } = input;
    let origin_for = |field_path: &str| {
        initial_origins
            .get(field_path)
            .cloned()
            .unwrap_or_else(|| Origin::derived(field_path))
    };
    let fail =
        |field_path: &str, message: &str| Diagnostic::new(message).at(origin_for(field_path));
    let mut clock_sources = Vec::new();
    for (index, source) in raw.clock_sources.iter().enumerate() {
        let field = format!("/clock_sources/{index}");
        if source.header.is_empty()
            || source.header.starts_with('/')
            || source
                .header
                .split('/')
                .any(|part| part == ".." || part.is_empty())
            || !source
                .header
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || b"_/.-+".contains(&byte))
        {
            return Err(fail(
                &format!("{field}/header"),
                "clock source header must be a safe relative include name",
            ));
        }
        if !source.type_name.split("::").all(is_identifier) {
            return Err(fail(
                &format!("{field}/type_name"),
                "clock source type must be a qualified C++ identifier",
            ));
        }
        let include_origin = origin_for(&format!("{field}/include_dir"));
        let include_dir = resolve_path_value(&source.include_dir, &include_origin, &root, &sdk)?;
        if !include_dir.is_dir() {
            return Err(Diagnostic::new(format!(
                "clock source include directory does not exist: {}",
                include_dir.display()
            ))
            .at(include_origin));
        }
        if !include_dir.join(&source.header).is_file() {
            return Err(Diagnostic::new(format!(
                "clock source header does not exist: {}",
                include_dir.join(&source.header).display()
            ))
            .at(origin_for(&format!("{field}/header"))));
        }
        clock_sources.push(crate::configuration::ResolvedClockSource {
            header: source.header.clone(),
            type_name: source.type_name.clone(),
            include_dir,
            origin: origin_for(&field),
        });
    }
    for name in &firmware.services {
        if !raw.services.contains_key(name) {
            return Err(fail(
                "/firmware/services",
                &format!("unknown service {name}"),
            ));
        }
    }
    Ok(LoadedProject {
        raw,
        firmware,
        root,
        board_directories,
        origins,
        initial_origins,
        files,
        sdk,
        sdk_compatibility,
        toolchain,
        options,
        board,
        audit_board,
        test_suite,
        clock_sources,
    })
}

pub(crate) fn finish_project_inputs(
    loaded: LoadedProject,
    threads: Vec<crate::configuration::ResolvedThread>,
    max_priority: u32,
    shared_objects: BTreeMap<String, u32>,
) -> Result<ResolvedProject> {
    let LoadedProject {
        raw,
        firmware,
        root,
        board_directories,
        origins,
        initial_origins,
        files,
        sdk,
        sdk_compatibility,
        toolchain,
        options,
        board,
        audit_board,
        test_suite,
        clock_sources,
    } = loaded;
    let origin_for = |field_path: &str| {
        initial_origins
            .get(field_path)
            .cloned()
            .unwrap_or_else(|| Origin::derived(field_path))
    };
    let fail =
        |field_path: &str, message: &str| Diagnostic::new(message).at(origin_for(field_path));
    let board_paths = super::board_inheritance::BoardPaths {
        project: &root,
        sdk: &sdk,
        directories: &board_directories,
    };
    let author_files: BTreeMap<_, _> = origins
        .iter()
        .filter(|(field, _)| field.as_str() != "/output_dir")
        .map(|(field, origin)| (field.as_str(), origin.file.as_str()))
        .collect();
    let configuration_identity =
        crate::build_graph::calculate_fingerprint(&serde_json::to_vec(&(
            &firmware,
            &root,
            &sdk,
            &sdk_compatibility,
            &board,
            raw.mode,
            &toolchain,
            &options,
            &raw.services,
            &raw.libraries,
            &raw.sdk_components,
            &raw.clock_sources,
            &raw.generators,
            &test_suite,
            &author_files,
        ))?);
    let build_root = match &raw.build_dir {
        Some(value) if value.is_empty() => {
            return Err(fail("/build_dir", "build directory must not be empty"));
        }
        Some(value) => resolve_path_value(value, &origin_for("/build_dir"), &root, &sdk)?,
        None => root.join(".cheriot-compose"),
    };
    if build_root.to_string_lossy().contains(['\n', '\r']) {
        return Err(fail(
            "/build_dir",
            "build directory cannot contain a line break",
        ));
    }
    let build_dir = build_root
        .join(&board.name)
        .join("cheriot")
        .join(match raw.mode {
            BuildMode::Release => "release",
            BuildMode::Debug => "debug",
        });
    let output_dir = match &raw.output_dir {
        Some(value) if value.is_empty() => {
            return Err(fail("/output_dir", "output directory must not be empty"));
        }
        Some(value) => resolve_path_value(value, &origin_for("/output_dir"), &root, &sdk)?,
        None => root.clone(),
    };
    if output_dir.exists() && !output_dir.is_dir() {
        return Err(fail("/output_dir", "output directory is not a directory"));
    }
    if output_dir.to_string_lossy().contains(['\n', '\r']) {
        return Err(fail(
            "/output_dir",
            "output directory cannot contain a line break",
        ));
    }
    let manifest = files
        .first()
        .cloned()
        .ok_or_else(|| Diagnostic::new("project manifest path is missing"))?;
    let board_overlays = raw
        .board_overlays
        .iter()
        .enumerate()
        .map(|(index, path)| {
            let origin = origin_for(&format!("/board_overlays/{index}"));
            super::canonicalize_path(&board_paths.resolve(path, &origin)?)
                .map_err(|error| error.at(origin))
        })
        .collect::<Result<Vec<_>>>()?;
    let publication_owner = serde_json::to_string(&(
        &manifest,
        &firmware.name,
        &board_directories.selected_file,
        &board_overlays,
        &board.name,
        raw.mode,
    ))?;
    let publication_identity =
        crate::build_graph::calculate_fingerprint(publication_owner.as_bytes());
    let generators =
        super::generator::resolve_generators(&raw.generators, &root, &sdk, &build_dir, &origins)?;
    let generator_outputs = generators
        .iter()
        .flat_map(|generator| generator.outputs.iter().cloned())
        .collect();
    let generated_paths = targets::GeneratedPaths {
        build: &build_dir,
        outputs: &generator_outputs,
    };
    let targets = targets::resolve_targets(
        &raw,
        &board,
        &options,
        &root,
        &sdk,
        &origins,
        &generated_paths,
    )?;
    let resolved_project = ResolvedProject {
        name: raw.name,
        mode: raw.mode,
        options,
        toolchain,
        sdk_component_names: raw.sdk_components.keys().cloned().collect(),
        firmware: crate::configuration::ResolvedFirmware {
            name: firmware.name,
            services: firmware.services,
            libraries: firmware.libraries,
            compartment_report: firmware.compartment_report,
        },
        test_suite,
        clock_sources,
        generators,
        root: root.clone(),
        manifest,
        sdk,
        sdk_compatibility,
        build_dir: build_dir.clone(),
        output_dir,
        configuration_identity,
        publication_identity,
        publication_owner,
        board,
        audit_board,
        threads,
        max_priority,
        shared_objects,
        targets,
        components: Vec::new(),
        provenance: origins,
        config_files: files,
    };
    Ok(resolved_project)
}

pub(crate) use targets::validate_reachable_inputs;
