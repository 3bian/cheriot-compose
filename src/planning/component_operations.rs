// Copyright 3Bian Limited and Contributors
// SPDX-License-Identifier: MIT

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use super::component_outputs::ComponentOutputs;
use super::{arguments_from_strings, create_command, create_operation, create_sourced_command};
use crate::build_graph::{
    BuildAction, BuildOperationKind, BuildPlan, ConditionalArguments, calculate_fingerprint,
    path_to_string,
};
use crate::configuration::{BuildMode, ComponentRole, ResolvedProject, RevokerKind};
use crate::configuration::{ComponentRecipe, SdkComponentKind};
use crate::diagnostic::{Diagnostic, Origin, Result, Sourced};

fn default_argument(value: impl Into<String>) -> Sourced<String> {
    Sourced::derived(value.into(), "CAL-18 SDK compiler rules")
}

fn base_flags(project: &ResolvedProject, recipe: &ComponentRecipe) -> Vec<String> {
    let mut compiler_flags = arguments_from_strings(&[
        "-c",
        "-Qunused-arguments",
        "-target",
        if recipe.uses_bare_metal_abi() {
            "riscv32-unknown-unknown"
        } else {
            "riscv32-unknown-cheriotrtos"
        },
    ]);
    compiler_flags.extend([
        format!("-mcpu={}", project.board.cpu),
        format!(
            "-mabi={}",
            if recipe.uses_bare_metal_abi() {
                "cheriot-baremetal"
            } else {
                "cheriot"
            }
        ),
    ]);
    compiler_flags.extend(arguments_from_strings(&[
        "-mxcheri-rvc",
        "-mrelax",
        "-fshort-wchar",
        "-g",
        "-ffunction-sections",
        "-fdata-sections",
        "-fomit-frame-pointer",
        "-fno-builtin-setjmp",
        "-fno-builtin-longjmp",
        "-fno-builtin-printf",
        "-fno-exceptions",
        "-fno-asynchronous-unwind-tables",
        "-fno-c++-static-destructors",
        "-fno-rtti",
        "-nostdinc",
    ]));
    for directory in ["include/c++-config", "include/libc++", "include"] {
        compiler_flags.push(format!("-I{}", project.sdk.join(directory).display()))
    }
    compiler_flags
}

pub(super) fn object_path(
    project: &ResolvedProject,
    recipe: &ComponentRecipe,
    source_file: &Path,
) -> Result<PathBuf> {
    let basename = source_file
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| Diagnostic::new("source filename must be UTF-8"))?;
    if recipe.role == Some(ComponentRole::Loader) {
        return Ok(project
            .build_dir
            .join(format!("loader-{}", project.configuration_identity))
            .join(format!("{basename}.o")));
    }
    let readable = |value: &str| {
        value
            .chars()
            .take(48)
            .map(|character| {
                if character.is_ascii_alphanumeric() || "_- .".contains(character) {
                    character
                } else {
                    '_'
                }
            })
            .collect::<String>()
    };
    let identity = calculate_fingerprint(&serde_json::to_vec(&(
        &project.configuration_identity,
        &recipe.name,
        source_file,
    ))?);
    Ok(project.build_dir.join(format!(
        "{}--{}--{identity}.o",
        readable(&recipe.name),
        readable(basename)
    )))
}

pub(super) fn add_component_operations(
    project: &ResolvedProject,
    recipes: &[ComponentRecipe],
    generated: &[String],
    plan: &mut BuildPlan,
) -> Result<ComponentOutputs> {
    let mut objects = BTreeMap::<String, Vec<PathBuf>>::new();
    let mut producers = BTreeMap::<String, Vec<String>>::new();
    for recipe in recipes {
        for source in &recipe.sources {
            let source_file = &source.path;
            if !source_file.is_file() && plan.artifact_producer(source_file).is_none() {
                return Err(Diagnostic::new(format!(
                    "missing SDK source {}; set sdk to the extracted archive's sdk directory",
                    source_file.display()
                )));
            }
        }
        let probe_id = format!("probe.{}", recipe.name);
        if recipe.needs_subobject_bounds_probe() {
            let probe_source = project.build_dir.join(format!("{}.probe.cc", recipe.name));
            let id = format!("generate.{}.probe", recipe.name);
            let content = "static_assert(__CHERIOT__ >= 20250812);\n".to_string();
            let artifact = crate::render::firmware::GeneratedArtifact::literal(
                "CHERIOT feature probe",
                probe_source.clone(),
                content,
                vec![],
                vec![Origin::derived("CAL-19")],
            );
            super::generated_operations::register_artifact(plan, id.clone(), artifact, vec![])?;
            let mut arguments = base_flags(project, recipe);
            arguments.remove(0);
            arguments.extend(arguments_from_strings(&["-std=c++23", "-fsyntax-only"]));
            arguments.push(path_to_string(&probe_source)?);
            plan.probes.insert(probe_id.clone());
            plan.operations.push(create_operation(
                probe_id.clone(),
                BuildOperationKind::Probe,
                BuildAction::Probe {
                    command: create_command(
                        project,
                        project.toolchain.cpp_compiler.clone(),
                        arguments,
                    ),
                },
                vec![probe_source],
                vec![],
                vec![id],
                vec![Origin::derived("__CHERIOT__ >= 20250812")],
            )?);
        }
        let mut object_files = Vec::new();
        let mut dependencies = Vec::new();
        for (source_index, source) in recipe.sources.iter().enumerate() {
            let source_file = &source.path;
            if source_file
                .file_name()
                .is_some_and(|name| name == "Cargo.toml")
            {
                let (object, id) = super::rust_operations::add_cargo_source(
                    project,
                    recipe,
                    source,
                    source_index,
                    generated,
                    plan,
                )?;
                object_files.push(object);
                dependencies.push(id);
                continue;
            }
            if source_file.extension().and_then(|text| text.to_str()) == Some("rs") {
                let (object, id) = super::rust_operations::add_rust_source(
                    project,
                    recipe,
                    source,
                    source_index,
                    generated,
                    plan,
                )?;
                object_files.push(object);
                dependencies.push(id);
                continue;
            }
            let assembly = matches!(
                source_file.extension().and_then(|text| text.to_str()),
                Some("S" | "s")
            );
            let cxx = matches!(
                source_file.extension().and_then(|text| text.to_str()),
                Some("cc" | "cpp" | "cxx")
            );
            let sdk_target = (!project.targets.contains_key(&recipe.name)
                || project.sdk_component_names.contains(&recipe.name))
                && !recipe.name.ends_with(".scheduler");
            let mut arguments: Vec<Sourced<String>> = base_flags(project, recipe)
                .into_iter()
                .map(default_argument)
                .collect();
            if let Some(cpu_origin) = project.provenance.get("/board/cpu")
                && let Some(cpu_argument) = arguments.get_mut(4)
            {
                cpu_argument.origins.push(cpu_origin.clone());
            }
            if sdk_target {
                arguments.push(default_argument("-fvisibility=hidden"));
                if cxx {
                    arguments.push(default_argument("-fvisibility-inlines-hidden"))
                }
            }
            arguments.push(default_argument(
                if recipe.kind == SdkComponentKind::Loader {
                    "-O1"
                } else {
                    "-Oz"
                },
            ));
            arguments.push(default_argument(if cxx {
                "-std=c++23"
            } else {
                "-std=c23"
            }));
            if recipe.role == Some(ComponentRole::WallClock) {
                arguments.push(default_argument(format!(
                    "-I{}",
                    project.build_dir.display()
                )));
                for source in &project.clock_sources {
                    arguments.push(Sourced::new(
                        format!("-I{}", source.include_dir.display()),
                        source.origin.clone(),
                    ));
                }
            }
            for directory in recipe
                .includes
                .iter()
                .chain(&source.includes)
                .chain(&project.board.includes)
            {
                arguments.push(Sourced {
                    value: format!("-I{}", directory.value.display()),
                    origins: directory.origins.clone(),
                })
            }
            let release = project.mode == BuildMode::Release && sdk_target;
            if assembly && release {
                arguments.push(default_argument("-DNDEBUG"))
            }
            arguments.extend(
                recipe
                    .flags
                    .iter()
                    .chain(&source.flags)
                    .filter(|compiler_flag| !assembly || compiler_flag.value.starts_with("-D"))
                    .map(|compiler_flag| {
                        Sourced::new(compiler_flag.value.clone(), compiler_flag.origin.clone())
                    }),
            );
            arguments.extend(project.board.flags.iter().map(|compiler_flag| {
                Sourced::new(compiler_flag.value.clone(), compiler_flag.origin.clone())
            }));
            if assembly {
                arguments.extend(
                    recipe
                        .assembly_flags
                        .iter()
                        .chain(&source.assembly_flags)
                        .map(|flag| Sourced::new(flag.value.clone(), flag.origin.clone())),
                );
            }
            if !assembly {
                if recipe.kind == SdkComponentKind::Compartment
                    || recipe.role == Some(ComponentRole::Scheduler)
                    || (recipe.role == Some(ComponentRole::Allocator)
                        && project.board.revoker == RevokerKind::Hardware)
                {
                    arguments.push(default_argument(format!(
                        "-iquote{}",
                        project.build_dir.display()
                    )))
                }
                if let Some(compartment) = &recipe.compartment {
                    arguments.push(default_argument(format!(
                        "-cheri-compartment={compartment}"
                    )))
                }
                for (index, flag) in project.board.cxflags.iter().enumerate() {
                    let origin = project
                        .provenance
                        .get(&format!("/board/cxflags/{index}"))
                        .cloned()
                        .unwrap_or_else(|| Origin::derived("board compiler flags"));
                    arguments.push(Sourced::new(flag.clone(), origin));
                }
                for include in recipe.force_includes.iter().chain(&source.force_includes) {
                    arguments.push(Sourced::new("-include".into(), include.origin.clone()));
                    arguments.push(Sourced::new(
                        path_to_string(&include.path)?,
                        include.origin.clone(),
                    ));
                }
                if let Some(origin) = &source.rtos_git_description {
                    arguments.push(Sourced::new("-include".into(), origin.clone()));
                    arguments.push(Sourced::new(
                        path_to_string(&project.build_dir.join("rtos-git-description.h"))?,
                        origin.clone(),
                    ));
                }
                arguments.extend(
                    if cxx {
                        &recipe.cpp_flags
                    } else {
                        &recipe.c_flags
                    }
                    .iter()
                    .chain(if cxx {
                        &source.cpp_flags
                    } else {
                        &source.c_flags
                    })
                    .map(|flag| Sourced::new(flag.value.clone(), flag.origin.clone())),
                );
            }
            let conditional = (recipe.needs_subobject_bounds_probe() && !assembly).then(|| {
                ConditionalArguments {
                    probe: probe_id.clone(),
                    insert_at: arguments.len(),
                    arguments: arguments_from_strings(&["-Xclang", "-cheri-bounds=subobject-safe"])
                        .into_iter()
                        .map(|value| Sourced::derived(value, "compiler feature probe"))
                        .collect(),
                }
            });
            if !assembly && release {
                arguments.push(default_argument("-DNDEBUG"))
            }
            let object = object_path(project, recipe, source_file)?;
            let depfile = object.with_extension("o.d");
            arguments.extend(
                [
                    "-MMD".into(),
                    "-MF".into(),
                    path_to_string(&depfile)?,
                    "-o".into(),
                    path_to_string(&object)?,
                    path_to_string(source_file)?,
                ]
                .into_iter()
                .map(default_argument),
            );
            let mut parents = generated.to_vec();
            if recipe.needs_subobject_bounds_probe() {
                parents.push(probe_id.clone())
            }
            if source.rtos_git_description.is_some() {
                parents.push("generate.rtos-git-description.h".into());
            }
            let id = format!("compile.{}.{source_index}", recipe.name);
            let mut provenance = vec![Origin::derived(
                "CAL-18; archived compiler argument ordering",
            )];
            provenance.extend(
                recipe
                    .flags
                    .iter()
                    .chain(&recipe.c_flags)
                    .chain(&recipe.cpp_flags)
                    .chain(&recipe.assembly_flags)
                    .chain(&source.flags)
                    .chain(&source.c_flags)
                    .chain(&source.cpp_flags)
                    .chain(&source.assembly_flags)
                    .map(|compiler_flag| compiler_flag.origin.clone()),
            );
            provenance.extend(
                recipe
                    .force_includes
                    .iter()
                    .chain(&source.force_includes)
                    .map(|include| include.origin.clone()),
            );
            let compiler = source.compiler.as_ref().or(recipe.compiler.as_ref());
            if let Some(compiler) = compiler {
                provenance.extend(compiler.origins.clone());
            }
            provenance.extend(
                project
                    .board
                    .flags
                    .iter()
                    .map(|compiler_flag| compiler_flag.origin.clone()),
            );
            let mut input = vec![source_file.clone()];
            if source.rtos_git_description.is_some() {
                input.push(project.build_dir.join("rtos-git-description.h"));
            }
            if project.test_suite.is_some() && !assembly {
                input.push(project.build_dir.join("tests-prototypes.h"));
                input.push(project.build_dir.join("tests-all.inc"));
            }
            if recipe.role == Some(ComponentRole::WallClock) {
                input.push(project.build_dir.join("rtc_sources.hh"));
                input.extend(
                    project
                        .clock_sources
                        .iter()
                        .map(|clock| clock.include_dir.join(&clock.header)),
                );
            }
            if !assembly {
                input.extend(
                    recipe
                        .force_includes
                        .iter()
                        .chain(&source.force_includes)
                        .map(|include| include.path.clone()),
                );
            }
            input.extend(project.config_files.clone());
            input.push(project.build_dir.join("board-interrupts.h"));
            let mut operation = create_operation(
                id.clone(),
                if assembly {
                    BuildOperationKind::Assemble
                } else {
                    BuildOperationKind::Compile
                },
                BuildAction::Command {
                    command: create_sourced_command(
                        project,
                        compiler.map(|tool| tool.value.clone()).unwrap_or_else(|| {
                            if cxx {
                                project.toolchain.cpp_compiler.clone()
                            } else {
                                project.toolchain.c_compiler.clone()
                            }
                        }),
                        arguments,
                    ),
                    conditional,
                },
                input,
                vec![object.clone()],
                parents,
                provenance,
            )?;
            operation.depfile = Some(depfile);
            plan.operations.push(operation);
            object_files.push(object);
            dependencies.push(id);
        }
        for dependency in &recipe.dependencies {
            if let Some(extra) = objects.get(dependency) {
                object_files.extend(extra.clone());
                dependencies.extend(producers.get(dependency).cloned().unwrap_or_default());
            }
        }
        if matches!(
            recipe.kind,
            SdkComponentKind::Object | SdkComponentKind::Group
        ) {
            objects.insert(recipe.name.clone(), object_files);
            producers.insert(recipe.name.clone(), dependencies);
            continue;
        }
        let script = project.sdk.join(&recipe.script);
        if !script.is_file() {
            return Err(Diagnostic::new(format!(
                "missing linker script {}",
                script.display()
            )));
        }
        let output = super::component_output(project, recipe);
        let mut arguments = if recipe.kind == SdkComponentKind::Loader {
            arguments_from_strings(&["--relocatable", "--gc-sections"])
        } else {
            arguments_from_strings(&["--compartment", "--gc-sections", "--relax"])
        };
        arguments.insert(0, format!("--script={}", script.display()));
        arguments.extend(["-o".into(), path_to_string(&output)?]);
        for object_file in &object_files {
            arguments.push(path_to_string(object_file)?)
        }
        let mut inputs = object_files;
        inputs.push(script);
        let id = format!("link.{}", recipe.name);
        plan.operations.push(create_operation(
            id.clone(),
            BuildOperationKind::PartialLink,
            BuildAction::Command {
                command: create_command(project, project.toolchain.linker.clone(), arguments),
                conditional: None,
            },
            inputs,
            vec![output],
            dependencies,
            vec![Origin::derived("cheriot.component / cheriot.loader.base")],
        )?);
        producers.insert(recipe.name.clone(), vec![id]);
    }
    Ok(ComponentOutputs { objects, producers })
}
