// Copyright 3Bian Limited and Contributors
// SPDX-License-Identifier: MIT

use std::collections::{BTreeMap, BTreeSet};

use serde_json::json;

use crate::build_graph::{BuildAction, BuildCommand, BuildPlan, CommandResolution};
use crate::diagnostic::{Diagnostic, Result};
use crate::render::template;

pub fn quote_shell_argument(argument: &str) -> String {
    format!("'{}'", argument.replace('\'', "'\"'\"'"))
}

pub fn format_shell_command(command: &BuildCommand) -> String {
    let mut words = vec!["env".to_string(), "--".to_string()];
    for (name, value) in &command.environment {
        words.push(quote_shell_argument(&format!("{name}={value}")))
    }
    words.push(quote_shell_argument(&command.executable));
    words.extend(
        command
            .argv
            .iter()
            .map(|argument| quote_shell_argument(&argument.value)),
    );
    let mut line = format!(
        "(cd {} && {}",
        quote_shell_argument(&command.cwd.to_string_lossy()),
        words.join(" ")
    );
    if let Some(output) = &command.stdout {
        line.push_str(&format!(
            " > {}",
            quote_shell_argument(&output.to_string_lossy())
        ))
    }
    if let Some(output) = &command.stderr {
        line.push_str(&format!(
            " 2> {}",
            quote_shell_argument(&output.to_string_lossy())
        ))
    }
    line.push(')');
    line
}

fn program_check(command: &BuildCommand, executable: &str, error: &Diagnostic) -> String {
    let mut discovery = command.clone();
    discovery.executable = "/bin/sh".into();
    let check = concat!(
        "case $1 in ",
        "*/*) test -f \"$1\" && test -x \"$1\" ;; ",
        "*) path=$(command -v \"$1\") && test -f \"$path\" && test -x \"$path\" ;; ",
        "esac"
    );
    discovery.argv = ["-c", check, "sh", executable]
        .into_iter()
        .map(|value| crate::diagnostic::Sourced::derived(value.to_owned(), "shell tool validation"))
        .collect();
    discovery.stdout = None;
    discovery.stderr = None;
    format!(
        "{} || {{ printf '%s\\n' {} >&2; exit 127; }}\n",
        format_shell_command(&discovery),
        quote_shell_argument(&error.to_string())
    )
}

fn write_changed(output: &std::path::Path, produce: &str) -> String {
    let output = quote_shell_argument(&output.to_string_lossy());
    format!(
        "tmp=$(mktemp {output}.cheri-tmp.XXXXXX)\ntrap 'rm -f -- \"$tmp\"' 0 1 2 3 15\n{produce}\nif test -f {output} && cmp -s -- \"$tmp\" {output}; then rm -f -- \"$tmp\"; else mv -- \"$tmp\" {output}; fi\ntrap - 0 1 2 3 15\n"
    )
}

pub fn render_shell_script(plan: &BuildPlan) -> Result<String> {
    plan.validate()?;
    let mut operations = Vec::new();
    for (index, operation) in plan.operations.iter().enumerate() {
        let mut script = String::new();
        let command = match &operation.action {
            BuildAction::Command { command, .. } | BuildAction::Probe { command } => Some(command),
            _ => None,
        };
        if let Some(command) = command {
            for check in operation
                .policy
                .tool_checks(plan.toolchain.as_ref(), command)
            {
                if let Some(executable) = check.executable {
                    script.push_str(&program_check(command, &executable, &check.error));
                } else {
                    script.push_str(&format!(
                        "printf '%s\\n' {} >&2\nexit 1\n",
                        quote_shell_argument(&check.error.to_string())
                    ));
                }
            }
        }
        let mut directories = BTreeSet::new();
        for output in operation.outputs.iter().chain(operation.depfile.iter()) {
            script.push_str(&format!(
                "guard_path {}\n",
                quote_shell_argument(&output.to_string_lossy())
            ));
            if let Some(directory) = output.parent() {
                directories.insert(directory);
            }
        }
        for directory in directories {
            script.push_str(&format!(
                "mkdir -p -- {}\n",
                quote_shell_argument(&directory.to_string_lossy())
            ))
        }
        let local_trap = matches!(
            operation.action,
            BuildAction::Generate { .. }
                | BuildAction::MemoryImage { .. }
                | BuildAction::GitDescription { .. }
                | BuildAction::Copy { .. }
        );
        if local_trap {
            script.push_str("(\n");
        }
        match &operation.action {
            BuildAction::Publish => {
                script.push_str(&super::publication_shell::publish(plan, &operation.inputs)?)
            }
            BuildAction::Copy { input } => {
                let input = quote_shell_argument(&input.to_string_lossy());
                script.push_str(&write_changed(
                    &operation.outputs[0],
                    &format!("cp -- {input} \"$tmp\""),
                ));
            }
            BuildAction::Generate { content, .. } => {
                script.push_str(&write_changed(
                    &operation.outputs[0],
                    &format!("printf '%s' {} > \"$tmp\"", quote_shell_argument(content)),
                ));
            }
            BuildAction::MemoryImage { input, word_bytes } => {
                let input = quote_shell_argument(&input.to_string_lossy());
                let format = match word_bytes {
                    4 => r#"1/4 "%08X" "\n""#,
                    8 => r#"1/8 "%016X" "\n""#,
                    _ => return Err(Diagnostic::new("invalid memory image word size")),
                };
                let format = quote_shell_argument(format);
                script.push_str(&write_changed(
                    &operation.outputs[0],
                    &format!("hexdump -v -e {format} {input} > \"$tmp\"\nprintf '\\n' >> \"$tmp\""),
                ));
            }
            BuildAction::GitDescription { command } => {
                let escape = quote_shell_argument(r#"s/\\/\\\\/g; s/"/\\"/g"#);
                script.push_str(&format!("if description=$( {} 2>/dev/null ); then :; else description=unknown; fi\ndescription=$(printf '%s' \"$description\" | tr -d '\\r\\n' | sed {escape})\n", format_shell_command(command)));
                script.push_str(&write_changed(&operation.outputs[0], "printf '#define CHERIOT_RTOS_GIT_DESCRIPTION \"%s\"\\n' \"$description\" > \"$tmp\""));
            }
            BuildAction::Probe { command } => {
                let probe_variable = format!("probe_{index}");
                script.push_str(&format!("if {} >/dev/null 2>&1; then {probe_variable}=true; else {probe_variable}=false; fi\n",format_shell_command(command)));
            }
            BuildAction::Command { .. } => {
                let command_script = match operation.command_resolution(&BTreeMap::new())? {
                    Some(CommandResolution::Alternatives {
                        probe,
                        when_false,
                        when_true,
                    }) => {
                        let probe_ix = plan
                            .operations
                            .iter()
                            .position(|candidate_operation| candidate_operation.id == probe)
                            .ok_or_else(|| Diagnostic::new("missing probe"))?;
                        format!(
                            "if test \"$probe_{probe_ix}\" = true; then\n  {}\nelse\n  {}\nfi\n",
                            format_shell_command(&when_true),
                            format_shell_command(&when_false)
                        )
                    }
                    Some(CommandResolution::Concrete(command)) => {
                        format!("{}\n", format_shell_command(&command))
                    }
                    None => return Err(Diagnostic::new("expected command")),
                };
                if operation.policy.output_handling == crate::build_graph::OutputHandling::Fresh {
                    script.push_str(&super::fresh_link::render(operation, &command_script)?);
                } else {
                    script.push_str(&command_script);
                }
            }
        }
        if local_trap {
            script.push_str(")\n");
        }
        if !matches!(operation.action, BuildAction::Probe { .. }) {
            for output in operation.outputs.iter().chain(operation.depfile.iter()) {
                let quoted = quote_shell_argument(&output.to_string_lossy());
                let message = quote_shell_argument(&format!(
                    "{} succeeded without producing {}",
                    operation.id,
                    output.display()
                ));
                script.push_str(&format!(
                    "test -f {quoted} || {{ printf '%s\\n' {message} >&2; exit 1; }}\n"
                ));
            }
        }
        operations.push(json!({"id":operation.id,"script":script}));
    }
    template::render_text(
        include_str!("../../templates/shell.mustache"),
        &json!({"operations":operations, "prelude": super::publication_shell::prelude(plan)?}),
    )
}
