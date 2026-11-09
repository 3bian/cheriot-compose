// Copyright 3Bian Limited and Contributors
// SPDX-License-Identifier: MIT

use std::{collections::BTreeMap, path::Path};

use super::{RawToolchain, Toolchain};
use crate::diagnostic::{Diagnostic, Origin, Result};

pub(super) fn resolve(
    raw: &RawToolchain,
    configured_directory: Option<&str>,
    root: &Path,
    sdk: &Path,
    origins: &mut BTreeMap<String, Origin>,
) -> Result<Toolchain> {
    let origin = origins
        .get("/toolchain_dir")
        .cloned()
        .unwrap_or_else(|| Origin::derived("built-in toolchain directory"));
    let directory = match configured_directory {
        Some("") => return Err(Diagnostic::new("toolchain directory must not be empty").at(origin)),
        Some(value) => super::resolution::resolve_path_value(value, &origin, root, sdk)?,
        None => std::path::PathBuf::from("/cheriot-tools/bin"),
    };
    if directory.to_string_lossy().contains(['\0', '\n', '\r']) {
        return Err(Diagnostic::new("invalid toolchain directory").at(origin));
    }
    origins.insert("/toolchain_dir".into(), origin.clone());
    for field in [
        "c_compiler",
        "cpp_compiler",
        "linker",
        "object_dump",
        "object_copy",
        "strip",
    ] {
        origins.insert(format!("/toolchain/{field}"), origin.clone());
    }
    let executable = |name: &str| directory.join(name).display().to_string();
    let exceptional = |field: &str, value: &str| -> Result<String> {
        let source = origins
            .get(&format!("/toolchain/{field}"))
            .cloned()
            .unwrap_or_else(|| Origin::derived("default exceptional tool"));
        if value.is_empty() || value.starts_with('-') || value.contains(['\0', '\n', '\r']) {
            return Err(Diagnostic::new("tool must be an executable name or path").at(source));
        }
        if value.contains('/') || value.contains("${") {
            Ok(
                super::resolution::resolve_path_value(value, &source, root, sdk)?
                    .display()
                    .to_string(),
            )
        } else {
            Ok(value.to_owned())
        }
    };
    let rust_compiler = raw
        .rust_compiler
        .as_deref()
        .map(|value| exceptional("rust_compiler", value))
        .transpose()?;
    let rust_compiler_origin = rust_compiler.as_ref().map(|_| {
        origins
            .get("/toolchain/rust_compiler")
            .cloned()
            .unwrap_or_else(|| Origin::derived("firmware Rust compiler"))
    });
    let cargo = exceptional("cargo", raw.cargo.as_deref().unwrap_or("cargo"))?;
    let cargo_origin = origins
        .get("/toolchain/cargo")
        .cloned()
        .unwrap_or_else(|| Origin::derived("host Cargo on PATH"));
    let uf2_converter = exceptional(
        "uf2_converter",
        raw.uf2_converter.as_deref().unwrap_or("uf2conv"),
    )?;
    let uf2_converter_origin = origins
        .get("/toolchain/uf2_converter")
        .cloned()
        .unwrap_or_else(|| Origin::derived("UF2 converter on PATH"));
    if raw.rust_flags.iter().any(|flag| flag.contains('\0')) {
        return Err(Diagnostic::new("Rust flags cannot contain NUL").at(origins
            .get("/toolchain/rust_flags")
            .cloned()
            .unwrap_or_else(|| Origin::derived("Rust flags"))));
    }
    Ok(Toolchain {
        c_compiler: executable("clang"),
        cpp_compiler: executable("clang++"),
        linker: executable("ld.lld"),
        object_dump: executable("llvm-objdump"),
        object_copy: executable("llvm-objcopy"),
        strip: executable("llvm-strip"),
        directory,
        directory_origin: origin,
        uf2_converter,
        uf2_converter_origin,
        rust_compiler,
        rust_compiler_origin,
        cargo,
        cargo_origin,
        rust_flags: raw.rust_flags.clone(),
    })
}
