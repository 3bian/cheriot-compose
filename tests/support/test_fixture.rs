// Copyright 3Bian Limited and Contributors
// SPDX-License-Identifier: MIT

#![allow(dead_code)]
use std::{
    fs,
    path::{Path, PathBuf},
};

#[path = "test_directory.rs"]
mod test_directory;

#[path = "rtos_tree.rs"]
pub mod rtos_tree;

pub struct TestFixture {
    pub directory: PathBuf,
    temporary_directory: test_directory::TestDirectory,
}

impl TestFixture {
    pub fn new() -> Self {
        let temporary_directory = test_directory::TestDirectory::new();
        let directory = temporary_directory.directory.clone();
        fs::create_dir_all(directory.join("boards")).unwrap();
        let overlay_sdk =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("integrations/cheriot-rtos/sdk");
        rtos_tree::copy_tree(&overlay_sdk, &directory.join("sdk"));
        fs::write(
            directory.join("cheriot-compose.toml"),
            include_str!("producer-consumer.toml"),
        )
        .unwrap();
        let libraries = directory.join("sdk/lib");
        for entry in fs::read_dir(libraries).unwrap().flatten() {
            let catalogue = entry.path().join("cheriot-compose.toml");
            if !catalogue.is_file() {
                continue;
            }
            let document: toml::Value =
                toml::from_str(&fs::read_to_string(catalogue).unwrap()).unwrap();
            if let Some(components) = document
                .get("sdk_components")
                .and_then(toml::Value::as_table)
            {
                for component in components.values() {
                    for field in ["sources", "force_includes", "includes"] {
                        if let Some(sources) = component
                            .get("build")
                            .and_then(|build| build.get(field))
                            .and_then(toml::Value::as_array)
                        {
                            for source in sources {
                                let relative =
                                    source.as_str().unwrap().strip_prefix("${sdk}/").unwrap();
                                let path = directory.join("sdk").join(relative);
                                if field == "includes" {
                                    fs::create_dir_all(path).unwrap();
                                } else {
                                    fs::create_dir_all(path.parent().unwrap()).unwrap();
                                    fs::write(path, "/* SDK source fixture */\n").unwrap();
                                }
                            }
                        }
                    }
                }
            }
        }
        let sdk_document: toml::Value = toml::from_str(
            &fs::read_to_string(directory.join("sdk/cheriot-compose.toml")).unwrap(),
        )
        .unwrap();
        for directory_name in sdk_document["sdk_compatibility"]["required_directories"]
            .as_array()
            .unwrap()
        {
            fs::create_dir_all(directory.join("sdk").join(directory_name.as_str().unwrap()))
                .unwrap();
        }
        for required in sdk_document["sdk_compatibility"]["required_files"]
            .as_array()
            .unwrap()
        {
            let path = directory
                .join("sdk")
                .join(required["path"].as_str().unwrap());
            if !path.exists() {
                fs::create_dir_all(path.parent().unwrap()).unwrap();
                fs::write(
                    path,
                    required
                        .get("contains")
                        .and_then(toml::Value::as_str)
                        .unwrap_or("/* SDK fixture */\n"),
                )
                .unwrap();
            }
        }
        for relative in [
            "core/allocator/main.cc",
            "core/software_revoker/revoker.cc",
            "software_revoker.ldscript",
        ] {
            let path = directory.join("sdk").join(relative);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            if !path.exists() {
                fs::write(path, "/* SDK test fixture */\n").unwrap();
            }
        }
        fs::write(
            directory.join("boards/sail.toml"),
            include_str!("../../integrations/cheriot-rtos/sdk/boards/sail.toml"),
        )
        .unwrap();
        for path in ["include/platform/sail", "include/platform/generic-riscv"] {
            fs::create_dir_all(directory.join("sdk").join(path)).unwrap();
        }
        for name in ["producer.cc", "consumer.cc"] {
            fs::write(
                directory.join(name),
                "// source existence fixture; not a firmware compilation test\n",
            )
            .unwrap();
        }
        Self {
            directory,
            temporary_directory,
        }
    }

    pub fn project_file(&self) -> PathBuf {
        self.directory.join("cheriot-compose.toml")
    }

    pub fn set_fake_llvm(&self, executable: &Path) {
        let directory = self.directory.join("fake-llvm");
        fs::create_dir_all(&directory).unwrap();
        for name in [
            "clang",
            "clang++",
            "ld.lld",
            "llvm-objdump",
            "llvm-objcopy",
            "llvm-strip",
        ] {
            fs::copy(executable, directory.join(name)).unwrap();
        }
        self.set_toolchain_directory(&directory);
    }

    pub fn set_toolchain_directory(&self, directory: &Path) {
        let path = self.project_file();
        let contents = fs::read_to_string(&path).unwrap();
        let rest = contents
            .lines()
            .filter(|line| !line.starts_with("toolchain_dir = "))
            .collect::<Vec<_>>()
            .join("\n");
        fs::write(
            path,
            format!(
                "toolchain_dir = {:?}\n{rest}\n",
                directory.to_str().unwrap()
            ),
        )
        .unwrap();
    }

    pub fn replace_text(&self, name: &str, from: &str, to: &str) {
        let path = self.directory.join(name);
        let contents = fs::read_to_string(&path).unwrap();
        assert!(contents.contains(from), "missing {from}");
        fs::write(path, contents.replace(from, to)).unwrap();
    }

    pub fn write_file(&self, name: &str, contents: &str) {
        self.temporary_directory.write_file(name, contents);
    }

    pub fn resolved_project(&self) -> cheriot_compose::configuration::ResolvedProject {
        cheriot_compose::project::load_project(&self.project_file()).unwrap()
    }

    pub fn build_plan(&self) -> cheriot_compose::build_graph::BuildPlan {
        let project = self.resolved_project();
        for recipe in cheriot_compose::calculations::sdk::resolve_dependencies(&project).unwrap() {
            for source in recipe.sources {
                let source = source.path;
                if !source.exists() {
                    fs::create_dir_all(source.parent().unwrap()).unwrap();
                    fs::write(source, "// source existence fixture\n").unwrap();
                }
            }
            let script = project.sdk.join(recipe.script);
            fs::create_dir_all(script.parent().unwrap()).unwrap();
            fs::write(script, "/* linker input existence fixture */\n").unwrap();
        }
        cheriot_compose::planning::create_build_plan(&project).unwrap()
    }
}

pub fn repository_root() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("application directory has a repository parent")
}

#[cfg(feature = "rtos-verification")]
pub fn align_reference_object_paths(
    reference: &mut [cheriot_compose::compilation_database::CompilationEntry],
    actual: &[cheriot_compose::compilation_database::CompilationEntry],
    mappings: &[cheriot_compose::compilation_database::PathMapping],
) {
    use cheriot_compose::configuration::resolution::normalize_path;
    for old in reference {
        let file = if Path::new(&old.file).is_absolute() {
            PathBuf::from(&old.file)
        } else {
            Path::new(&old.directory).join(&old.file)
        };
        let file = normalize_path(&file).display().to_string();
        let mapped = mappings
            .iter()
            .filter(|mapping| file == mapping.old || file.starts_with(&format!("{}/", mapping.old)))
            .max_by_key(|mapping| mapping.old.len())
            .map(|mapping| format!("{}{}", mapping.new, &file[mapping.old.len()..]))
            .unwrap_or(file);
        let old_output = old.output.clone().expect("reference compile output");
        let target = old_output
            .split("/.objs/")
            .nth(1)
            .and_then(|path| path.split('/').next());
        let candidates: Vec<_> = actual
            .iter()
            .filter(|entry| {
                entry.file == mapped
                    && target.is_none_or(|target| {
                        entry.cheriot_compose_target.as_deref() == Some(target)
                    })
            })
            .collect();
        assert_eq!(
            candidates.len(),
            1,
            "could not identify output for {} ({target:?})",
            old.file
        );
        let output = candidates[0]
            .output
            .clone()
            .expect("generated compile output");
        for argument in &mut old.arguments {
            if *argument == old_output {
                *argument = output.clone();
            } else if *argument == format!("-o{old_output}") {
                *argument = format!("-o{output}");
            }
        }
        old.output = Some(output);
    }
}
