// Copyright 3Bian Limited and Contributors
// SPDX-License-Identifier: MIT

use std::process::Command;

use serde::Serialize;

use cheriot_compose::{
    configuration::ResolvedProject,
    diagnostic::{Diagnostic, Result},
    execution, planning,
};

use super::{execute_with_progress, option_catalog::OutputFormat, options::Options};

#[derive(Serialize)]
struct SizeReport {
    method: &'static str,
    compartment: Option<String>,
    bytes: u64,
    section_count: usize,
    loader_start_found: bool,
    cap_relocs_found: bool,
}

fn parse_sections(text: &str, compartment: Option<&str>) -> Result<SizeReport> {
    let mut bytes = 0_u64;
    let mut count = 0;
    let mut active = false;
    let mut loader_start_found = false;
    let mut cap_relocs_found = false;
    let needle = compartment.map(str::to_ascii_lowercase);
    for line in text.lines() {
        let fields: Vec<_> = line.split_whitespace().collect();
        if fields.len() < 3 || fields[0].parse::<usize>().is_err() {
            continue;
        }
        let is_start = line.contains("loader_start");
        let is_end = line.contains("__cap_relocs");
        if is_start {
            active = true;
            loader_start_found = true;
        }
        if is_end {
            active = false;
            cap_relocs_found = true;
        }
        let selected = match &needle {
            Some(needle) => line.to_ascii_lowercase().contains(needle),
            None => active || is_end,
        };
        if selected {
            let size = u64::from_str_radix(fields[2], 16)
                .map_err(|_| Diagnostic::new("cannot parse section size from llvm-objdump"))?;
            bytes = bytes
                .checked_add(size)
                .ok_or_else(|| Diagnostic::new("section sizes overflow"))?;
            count += 1;
        }
    }
    if count == 0 {
        return Err(Diagnostic::new(match compartment {
            Some(name) => format!("no firmware sections match compartment {name:?}"),
            None => {
                "no SDK size sections found; check the selected firmware and objdump tool".into()
            }
        }));
    }
    Ok(SizeReport {
        method: "sdk_section_sum",
        compartment: compartment.map(str::to_owned),
        bytes,
        section_count: count,
        loader_start_found,
        cap_relocs_found,
    })
}

pub(super) fn execute(arguments: &Options, project: &ResolvedProject) -> Result<String> {
    let format = arguments.output_format;
    let plan = planning::create_build_plan(project)?;
    let firmware = plan
        .publication
        .as_ref()
        .map(|publication| &publication.elf)
        .ok_or_else(|| Diagnostic::new("firmware plan has no ELF output"))?;
    execute_with_progress(arguments, project, &plan, execution::ToolStdout::Stderr)?;
    if arguments.dry_run {
        return if format == OutputFormat::Json {
            Ok("{\"status\":\"not measured\",\"reason\":\"dry run\"}".into())
        } else {
            Ok("Size not measured (dry run).".into())
        };
    }
    execution::validate_llvm_executable(
        &project.toolchain,
        &project.toolchain.object_dump,
        &project.root,
    )?;
    let output = Command::new(&project.toolchain.object_dump)
        .current_dir(&project.root)
        .arg("--headers")
        .arg(firmware)
        .output()
        .map_err(|error| {
            Diagnostic::new(format!(
                "cannot run {}: {error}",
                project.toolchain.object_dump
            ))
        })?;
    if !output.stderr.is_empty() {
        use std::io::Write;
        std::io::stderr()
            .lock()
            .write_all(&output.stderr)
            .map_err(|error| {
                Diagnostic::new(format!("cannot show objdump diagnostics: {error}"))
            })?;
    }
    if !output.status.success() {
        return Err(Diagnostic::new(format!(
            "{} failed with {}",
            project.toolchain.object_dump, output.status
        )));
    }
    let text = std::str::from_utf8(&output.stdout)
        .map_err(|_| Diagnostic::new("objdump section output is not UTF-8"))?;
    let report = parse_sections(text, arguments.compartment_name.as_deref())?;
    if report.compartment.is_none() && (!report.loader_start_found || !report.cap_relocs_found) {
        eprintln!(
            "Warning: the SDK section markers are incomplete; this SDK-compatible sum is not a full firmware size."
        );
    }
    match format {
        OutputFormat::Json => Ok(serde_json::to_string_pretty(&report)?),
        OutputFormat::Bytes => Ok(report.bytes.to_string()),
        _ => Ok(format!(
            "SDK section sum{}: {} bytes ({:.2} KiB)",
            report
                .compartment
                .as_ref()
                .map(|name| format!(" for {name}"))
                .unwrap_or_default(),
            report.bytes,
            report.bytes as f64 / 1024.0
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::parse_sections;
    #[test]
    fn matches_sdk_selection_and_marks_missing_start() {
        let text = "  1 .loader_start 00000010 80000000 TEXT\n  2 .producer_code 00000020 80000010 TEXT\n  3 .producer_globals 00000004 80000030 DATA\n  4 __cap_relocs 00000008 80000034 DATA\n  5 .debug_info 00000100 00000000 DEBUG\n";
        let all = parse_sections(text, None).unwrap();
        assert_eq!(all.bytes, 0x3c);
        assert!(all.loader_start_found && all.cap_relocs_found);
        assert_eq!(parse_sections(text, Some("PRODUCER")).unwrap().bytes, 0x24);
        let missing = parse_sections(
            " 1 .loader_code 00000010 00000000 TEXT\n 2 __cap_relocs 00000008 00000010 DATA\n",
            None,
        )
        .unwrap();
        assert_eq!(missing.bytes, 8);
        assert!(!missing.loader_start_found);
        assert!(parse_sections(text, Some("missing")).is_err());
    }
}
