// Copyright 3Bian Limited and Contributors
// SPDX-License-Identifier: MIT

use std::collections::{BTreeMap, BTreeSet};

use crate::diagnostic::{Diagnostic, Result};

mod interrupt_state;
mod label;

use interrupt_state::InterruptState;
use label::Label;

const EXPORTS: [&str; 4] = [
    ".Lhandle_error_handler_return",
    "exception_entry_asm",
    "switcher_after_compartment_call",
    "__Z26compartment_switcher_entryz",
];
const ENTRY: &str = "__Z26compartment_switcher_entryz";

#[derive(Default)]
struct Analysis {
    labels: BTreeMap<String, Label>,
    incoming: BTreeMap<String, BTreeSet<String>>,
    outgoing: BTreeMap<String, BTreeMap<String, bool>>,
    exports: BTreeSet<String>,
    recent: Vec<String>,
}

fn invalid(line: usize, message: impl AsRef<str>) -> Diagnostic {
    Diagnostic::new(format!("switcher line {line}: {}", message.as_ref()))
}

impl Analysis {
    fn current(&self, line: usize) -> Result<&str> {
        self.recent
            .last()
            .map(String::as_str)
            .ok_or_else(|| invalid(line, "instruction or annotation has no preceding label"))
    }

    fn finish_label(&mut self, line: usize, clear: bool) -> Result<()> {
        let name = self.current(line)?.to_owned();
        let label = self
            .labels
            .get_mut(&name)
            .ok_or_else(|| invalid(line, format!("unknown label {name}")))?;
        if label.assumed.is_none() {
            label.assumed = Some(label.required.ok_or_else(|| {
                invalid(
                    line,
                    format!(
                        "Missing IRQ requirement for prior label {name}, cannot inherit assumption"
                    ),
                )
            })?);
        }
        if clear {
            self.recent.clear();
        }
        Ok(())
    }

    fn add_label(&mut self, name: &str, line: usize) -> Result<()> {
        if self.labels.contains_key(name) {
            return Err(invalid(line, format!("duplicate label {name}")));
        }
        if !self.recent.is_empty() {
            self.finish_label(line, false)?;
        }
        self.labels.insert(
            name.into(),
            Label {
                line,
                ..Label::default()
            },
        );
        self.recent.push(name.into());
        if self.recent.len() > 2 {
            self.recent.remove(0);
        }
        Ok(())
    }

    fn preceding(&self, line: usize) -> Result<(String, String)> {
        if self.recent.len() < 2 {
            return Err(invalid(line, "FROM: above needs a preceding label"));
        }
        Ok((
            self.recent[self.recent.len() - 2].clone(),
            self.recent[self.recent.len() - 1].clone(),
        ))
    }

    fn incoming_edge(&mut self, from: &str, to: &str) {
        self.incoming
            .entry(to.into())
            .or_default()
            .insert(from.into());
    }

    fn declared_edge(&mut self, from: &str, to: &str, dashed: bool) {
        self.outgoing
            .entry(from.into())
            .or_default()
            .insert(to.into(), dashed);
    }

    fn add_fallthrough(&mut self, line: usize, dashed: bool) -> Result<()> {
        let (from, to) = self.preceding(line)?;
        self.declared_edge(&from, &to, dashed);
        let assumed = self
            .labels
            .get(&from)
            .and_then(|label| label.assumed)
            .ok_or_else(|| {
                invalid(
                    line,
                    format!("Missing IRQ disposition for prior label {from}"),
                )
            })?;
        self.labels
            .get_mut(&to)
            .ok_or_else(|| invalid(line, format!("unknown label {to}")))?
            .required = Some(assumed);
        Ok(())
    }

    fn annotation(&mut self, text: &str, line: usize) -> Result<bool> {
        if text.ends_with("Switcher-exported library functions.") {
            return Ok(true);
        }
        if let Some(value) = text.strip_prefix("IRQ ASSUME:") {
            let state = InterruptState::parse(value.split_whitespace().next().unwrap_or(""), line)?;
            let current = self.current(line)?.to_owned();
            self.labels
                .get_mut(&current)
                .ok_or_else(|| invalid(line, "unknown label"))?
                .assumed = Some(state);
        } else if let Some(value) = text.strip_prefix("IRQ REQUIRE:") {
            let state = InterruptState::parse(value.split_whitespace().next().unwrap_or(""), line)?;
            let current = self.current(line)?.to_owned();
            self.labels
                .get_mut(&current)
                .ok_or_else(|| invalid(line, "unknown label"))?
                .required = Some(state);
        } else if let Some(value) = text.strip_prefix("FROM:") {
            let from = value.split_whitespace().next().unwrap_or("");
            match from {
                "malice" => {
                    let label = self.current(line)?.to_owned();
                    self.exports.insert(label);
                }
                "above" => self.add_fallthrough(line, false)?,
                "cross-call" | "interrupt" => (),
                "" => return Err(invalid(line, "FROM annotation needs a source")),
                _ => {
                    let to = self.current(line)?.to_owned();
                    self.declared_edge(from, &to, false);
                }
            }
        } else if let Some(value) = text.strip_prefix("IFROM:") {
            let from = value.split_whitespace().next().unwrap_or("");
            if from == "above" {
                self.add_fallthrough(line, true)?;
            } else if from.is_empty() {
                return Err(invalid(line, "IFROM annotation needs a source"));
            } else {
                let to = self.current(line)?.to_owned();
                self.declared_edge(from, &to, true);
            }
        } else if let Some(value) = text.strip_prefix("ITO:") {
            let to = value.split_whitespace().next().unwrap_or("");
            if to.is_empty() {
                return Err(invalid(line, "ITO annotation needs a destination"));
            }
            let from = self.current(line)?.to_owned();
            self.incoming_edge(&from, to);
        }
        Ok(false)
    }

    fn parse_line(&mut self, text: &str, line: usize) -> Result<bool> {
        if let Some(label) = text.strip_suffix(':') {
            if label.bytes().all(|byte| byte.is_ascii_digit()) {
                return Ok(false);
            }
            let documented = label
                .strip_prefix("//")
                .filter(|name| name.starts_with(".L"));
            let local = label.starts_with(".L");
            let global = !label.is_empty()
                && label
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_');
            if documented.is_some() || local || global {
                let name = documented.unwrap_or(label);
                self.add_label(name, line)?;
                if self.recent.len() > 1 {
                    let (from, to) = self.preceding(line)?;
                    self.incoming_edge(&from, &to);
                    if documented.is_some() {
                        self.add_fallthrough(line, false)?;
                    }
                } else if documented.is_some() {
                    return Err(invalid(line, "documentation label needs a preceding label"));
                }
                if global {
                    self.exports.insert(name.into());
                }
                return Ok(false);
            }
        }
        let instruction = text.trim_start();
        if instruction == "mret" || instruction == "cret" {
            self.finish_label(line, true)?;
            return Ok(false);
        }
        let mut instruction_parts = instruction.split_whitespace();
        if instruction_parts.next() == Some("j")
            && let Some(target) = instruction_parts.next()
            && instruction_parts.next().is_none()
        {
            let from = self.current(line)?.to_owned();
            self.incoming_edge(&from, target);
            self.finish_label(line, true)?;
            return Ok(false);
        }
        if instruction.starts_with('b')
            && let Some((_, target)) = instruction.rsplit_once(',')
        {
            let target = target.trim();
            if target.starts_with(".L") && !target.contains(char::is_whitespace) {
                let from = self.current(line)?.to_owned();
                self.incoming_edge(&from, target);
                return Ok(false);
            }
        }
        let comment = instruction
            .strip_prefix("//")
            .or_else(|| instruction.strip_prefix('*'));
        if let Some(comment) = comment {
            return self.annotation(comment.trim_start(), line);
        }
        Ok(false)
    }

    fn validate(&self) -> Result<()> {
        for name in self.labels.keys() {
            if let Some(incoming) = self.incoming.get(name) {
                for from in incoming {
                    if !self.labels.contains_key(from)
                        || !self
                            .outgoing
                            .get(from)
                            .is_some_and(|edges| edges.contains_key(name))
                    {
                        return Err(Diagnostic::new(format!(
                            "{name} in from {from} but no declared out edge"
                        )));
                    }
                    self.check_interrupts(from, name)?;
                }
            }
            if let Some(outgoing) = self.outgoing.get(name) {
                for to in outgoing.keys() {
                    if !self.labels.contains_key(to)
                        || !self
                            .incoming
                            .get(to)
                            .is_some_and(|edges| edges.contains(name))
                    {
                        return Err(Diagnostic::new(format!(
                            "{name} out to {to} but no observed in edge"
                        )));
                    }
                    self.check_interrupts(name, to)?;
                }
            }
        }
        if self.exports.len() != EXPORTS.len()
            || EXPORTS.iter().any(|name| !self.exports.contains(*name))
        {
            return Err(Diagnostic::new(
                "switcher exports do not match the expected entry points",
            ));
        }
        Ok(())
    }

    fn check_interrupts(&self, from: &str, to: &str) -> Result<()> {
        let assumed = self.labels.get(from).and_then(|label| label.assumed);
        let required = self.labels.get(to).and_then(|label| label.required);
        if required != Some(InterruptState::Any) && assumed != required {
            return Err(Diagnostic::new(format!(
                "IRQ-invalid arc from {from} ({assumed:?}) to {to} ({required:?})"
            )));
        }
        Ok(())
    }

    fn render(&self) -> Result<String> {
        let quote = |value: &str| serde_json::to_string(value).map_err(Diagnostic::from);
        let mut graph = String::from("digraph switcher {\n { rank=min; edge [style=invis];\n");
        for name in EXPORTS {
            graph.push_str(&format!("  {};\n", quote(name)?));
        }
        for pair in EXPORTS.windows(2) {
            graph.push_str(&format!("  {} -> {};\n", quote(pair[0])?, quote(pair[1])?));
        }
        graph.push_str(" }\n");
        for (name, label) in &self.labels {
            let inputs = self.incoming.get(name).map_or(0, BTreeSet::len);
            let outputs = self.outgoing.get(name).map_or(0, BTreeMap::len);
            let shape = if self.exports.contains(name) {
                "box"
            } else {
                match (inputs == 1, outputs) {
                    (_, 0) => "octagon",
                    (true, 1) => "oval",
                    (true, _) => "trapezium",
                    (false, 1) => "invtrapezium",
                    (false, _) => "hexagon",
                }
            };
            let mut attributes = vec![format!("tooltip={}", label.line), format!("shape={shape}")];
            if let Some(state) = label.assumed {
                attributes.push(format!("fontname=\"{}\"", state.font()));
            }
            let prefix = name
                .strip_prefix('.')
                .unwrap_or(name)
                .strip_prefix('L')
                .unwrap_or(name.strip_prefix('.').unwrap_or(name));
            let color = if prefix.starts_with("switch") || name == ENTRY {
                Some("cyan")
            } else if prefix.starts_with("exception") {
                Some("red")
            } else if prefix.starts_with("handle") {
                Some("orange")
            } else {
                None
            };
            if let Some(color) = color {
                attributes.push("style=filled".into());
                attributes.push(format!("fillcolor={color}"));
            }
            graph.push_str(&format!(" {} [{}];\n", quote(name)?, attributes.join(",")));
            if let Some(edges) = self.outgoing.get(name) {
                for (to, dashed) in edges {
                    graph.push_str(&format!(
                        " {} -> {} [{}];\n",
                        quote(name)?,
                        quote(to)?,
                        if *dashed { "style=dashed" } else { "" }
                    ));
                }
            }
        }
        graph.push_str("}\n");
        Ok(graph)
    }
}

pub fn analyze(source: &str) -> Result<String> {
    let mut lines = source.lines().enumerate();
    lines
        .find(|(_, line)| line.contains(".globl __Z26compartment_switcher_entryz"))
        .ok_or_else(|| Diagnostic::new("switcher entry declaration was not found"))?;
    let mut analysis = Analysis::default();
    for (index, line) in lines {
        if analysis.parse_line(line, index)? {
            break;
        }
    }
    analysis.validate()?;
    analysis.render()
}
