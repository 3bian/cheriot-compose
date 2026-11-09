// Copyright 3Bian Limited and Contributors
// SPDX-License-Identifier: MIT

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::diagnostic::{Diagnostic, Origin};

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Toolchain {
    pub directory: PathBuf,
    pub directory_origin: Origin,
    pub c_compiler: String,
    pub cpp_compiler: String,
    pub linker: String,
    pub object_dump: String,
    pub object_copy: String,
    pub strip: String,
    pub uf2_converter: String,
    pub uf2_converter_origin: Origin,
    pub rust_compiler: Option<String>,
    pub rust_compiler_origin: Option<Origin>,
    pub cargo: String,
    pub cargo_origin: Origin,
    pub rust_flags: Vec<String>,
}

impl Toolchain {
    pub fn cargo_error(&self, executable: &str) -> Diagnostic {
        Diagnostic::new(format!(
            "missing host Cargo; expected {executable} (names use the action's PATH). Set toolchain.cargo to the desired executable"
        )).at(self.cargo_origin.clone())
    }

    pub fn uf2_error(&self, executable: &str) -> Diagnostic {
        Diagnostic::new(format!(
            "missing UF2 converter; expected {executable} (names use the action's PATH). Set toolchain.uf2_converter to the desired executable"
        )).at(self.uf2_converter_origin.clone())
    }

    pub fn rust_error(&self, executable: &str) -> Diagnostic {
        let error = Diagnostic::new(format!(
            "firmware Rust compiler {executable} was not found or is not executable; configure toolchain.rust_compiler or RUSTC"
        ));
        match &self.rust_compiler_origin {
            Some(origin) => error.at(origin.clone()),
            None => error,
        }
    }

    pub fn llvm_error(&self, executable: &str) -> Option<Diagnostic> {
        let (name, _) = [
            ("clang", &self.c_compiler),
            ("clang++", &self.cpp_compiler),
            ("ld.lld", &self.linker),
            ("llvm-objdump", &self.object_dump),
            ("llvm-objcopy", &self.object_copy),
            ("llvm-strip", &self.strip),
        ]
        .into_iter()
        .find(|(_, value)| value.as_str() == executable)?;
        Some(Diagnostic::new(format!(
            "missing or non-executable tool {name}; expected {executable}. Select another directory with --toolchain-dir, CHERIOT_COMPOSE_TOOLCHAIN_DIR, or toolchain_dir"
        )).at(self.directory_origin.clone()))
    }
}
