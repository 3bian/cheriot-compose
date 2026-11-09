// Copyright 3Bian Limited and Contributors
// SPDX-License-Identifier: MIT

use std::collections::BTreeMap;

use serde_json::Value;

use crate::diagnostic::{Diagnostic, Result};
use template_token::TemplateToken;

mod escape_mode;
mod render_trace;
mod template_token;
mod tracked_render;

pub use escape_mode::EscapeMode;
pub use tracked_render::TrackedRender;

use render_trace::RenderTrace;

fn parse_template(
    template_text: &str,
    position: &mut usize,
    closing_section: Option<&str>,
    depth: usize,
) -> Result<Vec<TemplateToken>> {
    if depth > 64 {
        return Err(Diagnostic::new("template nesting limit exceeded"));
    }
    let mut tokens = Vec::new();
    while *position < template_text.len() {
        let Some(offset) = template_text[*position..].find("{{") else {
            tokens.push(TemplateToken::Text(template_text[*position..].into()));
            *position = template_text.len();
            break;
        };
        let start = *position + offset;
        if start > *position {
            tokens.push(TemplateToken::Text(template_text[*position..start].into()))
        }
        let triple = template_text[start..].starts_with("{{{");
        let opening = if triple { 3 } else { 2 };
        let closing = if triple { "}}}" } else { "}}" };
        let from = start + opening;
        let closing_index = template_text[from..]
            .find(closing)
            .ok_or_else(|| Diagnostic::new("unclosed Mustache tag"))?;
        let tag = template_text[from..from + closing_index].trim();
        *position = from + closing_index + closing.len();
        if let Some(name) = tag.strip_prefix('/') {
            if closing_section != Some(name.trim()) {
                return Err(Diagnostic::new(format!("unmatched section {name}")));
            }
            return Ok(tokens);
        }
        if tag.starts_with('#') || tag.starts_with('^') {
            let name = tag[1..].trim();
            let children = parse_template(template_text, position, Some(name), depth + 1)?;
            tokens.push(TemplateToken::Section(
                name.into(),
                tag.starts_with('^'),
                children,
            ));
        } else if let Some(partial_name) = tag.strip_prefix('>') {
            tokens.push(TemplateToken::Partial(partial_name.trim().into()));
        } else if tag.starts_with('!') {
        } else if tag.starts_with('=') {
            return Err(Diagnostic::new(
                "Mustache delimiter changes are unsupported",
            ));
        } else {
            tokens.push(TemplateToken::Variable(
                tag.trim_start_matches('&').trim().into(),
                triple || tag.starts_with('&'),
            ))
        }
    }
    if let Some(section_name) = closing_section {
        return Err(Diagnostic::new(format!("unclosed section {section_name}")));
    }
    Ok(tokens)
}

fn lookup_value<'a>(contexts: &[&'a Value], key: &str) -> Option<&'a Value> {
    if key == "." {
        return contexts.last().copied();
    }
    for context in contexts.iter().rev() {
        let mut parts = key.split('.');
        let mut value = context.get(parts.next()?);
        for part in parts {
            value = value.and_then(|value| value.get(part))
        }
        if value.is_some() {
            return value;
        }
    }
    None
}

fn is_truthy(value: Option<&Value>) -> bool {
    match value {
        None | Some(Value::Null) | Some(Value::Bool(false)) => false,
        Some(Value::Array(items)) => !items.is_empty(),
        _ => true,
    }
}

fn scalar_to_string(value: &Value) -> Result<String> {
    match value {
        Value::String(string) => Ok(string.clone()),
        Value::Number(number) => Ok(number.to_string()),
        Value::Bool(boolean) => Ok(boolean.to_string()),
        Value::Null => Ok(String::new()),
        _ => Err(Diagnostic::new("template variable must be scalar")),
    }
}

fn render_tokens(
    tokens: &[TemplateToken],
    contexts: &mut Vec<&Value>,
    partials: &BTreeMap<String, String>,
    escape_mode: EscapeMode,
    depth: usize,
    rendered_text: &mut String,
    trace: &mut RenderTrace,
) -> Result<()> {
    if depth > 64 {
        return Err(Diagnostic::new("recursive template partial"));
    }
    for token in tokens {
        match token {
            TemplateToken::Text(text) => rendered_text.push_str(text),
            TemplateToken::Variable(key, raw) => {
                trace.variables.insert(key.clone());
                let value = lookup_value(contexts, key)
                    .ok_or_else(|| Diagnostic::new(format!("missing template field {key}")))?;
                let rendered_value = scalar_to_string(value)?;
                if !raw && matches!(escape_mode, EscapeMode::Html) {
                    rendered_text.push_str(
                        &rendered_value
                            .replace('&', "&amp;")
                            .replace('<', "&lt;")
                            .replace('>', "&gt;")
                            .replace('"', "&quot;")
                            .replace('\'', "&#39;"),
                    )
                } else {
                    rendered_text.push_str(&rendered_value)
                }
            }
            TemplateToken::Section(key, invert, children) => {
                trace.variables.insert(key.clone());
                let value = lookup_value(contexts, key);
                if *invert {
                    if !is_truthy(value) {
                        render_tokens(
                            children,
                            contexts,
                            partials,
                            escape_mode,
                            depth + 1,
                            rendered_text,
                            trace,
                        )?
                    }
                } else if is_truthy(value)
                    && let Some(section_value) = value
                {
                    if let Value::Array(items) = section_value {
                        for item in items {
                            contexts.push(item);
                            render_tokens(
                                children,
                                contexts,
                                partials,
                                escape_mode,
                                depth + 1,
                                rendered_text,
                                trace,
                            )?;
                            contexts.pop();
                        }
                    } else {
                        contexts.push(section_value);
                        render_tokens(
                            children,
                            contexts,
                            partials,
                            escape_mode,
                            depth + 1,
                            rendered_text,
                            trace,
                        )?;
                        contexts.pop();
                    }
                }
            }
            TemplateToken::Partial(key) => {
                trace.partials.insert(key.clone());
                let partial_template = partials
                    .get(key)
                    .ok_or_else(|| Diagnostic::new(format!("missing partial {key}")))?;
                let tokens = parse_template(partial_template, &mut 0, None, 0)?;
                render_tokens(
                    &tokens,
                    contexts,
                    partials,
                    escape_mode,
                    depth + 1,
                    rendered_text,
                    trace,
                )?;
            }
        }
    }
    Ok(())
}

pub fn render(
    template_text: &str,
    data: &Value,
    partials: &BTreeMap<String, String>,
    mode: EscapeMode,
) -> Result<String> {
    Ok(render_tracked(template_text, data, partials, mode)?.content)
}

pub fn render_tracked(
    template_text: &str,
    data: &Value,
    partials: &BTreeMap<String, String>,
    mode: EscapeMode,
) -> Result<TrackedRender> {
    let tokens = parse_template(template_text, &mut 0, None, 0)?;
    let mut rendered_text = String::new();
    let mut trace = RenderTrace::default();
    render_tokens(
        &tokens,
        &mut vec![data],
        partials,
        mode,
        0,
        &mut rendered_text,
        &mut trace,
    )?;
    let used = trace
        .partials
        .into_iter()
        .filter_map(|name| partials.get(&name).map(|text| (name, text.clone())))
        .collect();
    Ok(TrackedRender {
        content: rendered_text,
        partials: used,
        variables: trace.variables,
    })
}

pub fn render_text(template_text: &str, data: &Value) -> Result<String> {
    render(template_text, data, &BTreeMap::new(), EscapeMode::Text)
}

pub(crate) mod catalog;
