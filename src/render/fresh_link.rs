// Copyright 3Bian Limited and Contributors
// SPDX-License-Identifier: MIT

use serde_json::json;

use super::quote_shell_argument;
use crate::{build_graph::BuildOperation, diagnostic::Result, render::template};

pub(super) fn render(operation: &BuildOperation, command: &str) -> Result<String> {
    let outputs: Vec<_> = operation.outputs.iter().enumerate().map(|(index, path)| {
        let text = path.display().to_string();
        json!({
            "index": index,
            "path": quote_shell_argument(&text),
            "invalid_message": quote_shell_argument(&format!("link output is not a regular file: {text}")),
            "missing_message": quote_shell_argument(&format!("{} succeeded without producing {text}", operation.id)),
        })
    }).collect();
    template::render_text(
        include_str!("../../templates/fresh-link.mustache"),
        &json!({"outputs": outputs, "command": command}),
    )
}
