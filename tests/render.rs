// Copyright 3Bian Limited and Contributors
// SPDX-License-Identifier: MIT

use std::collections::BTreeMap;

use serde_json::json;

use cheriot_compose::render::template::{EscapeMode, render, render_text};

#[test]
fn sections_raw_dotted_and_partials() {
    let partials = BTreeMap::from([("p".into(), "{{user.name}}:{{.}};".into())]);
    let data = json!({"user":{"name":"A"},"items":[1,2],"empty":[],"value":"<&\""});
    assert_eq!(
        render(
            "{{#items}}{{>p}}{{/items}}{{^empty}}ok{{/empty}} {{{value}}} {{&value}} {{value}}",
            &data,
            &partials,
            EscapeMode::Html
        )
        .unwrap(),
        "A:1;A:2;ok <&\" <&\" &lt;&amp;&quot;"
    );
}

#[test]
fn linker_expressions_are_literal() {
    assert_eq!(
        render_text("{{v}}", &json!({"v":"ALIGN(8) < CAPALIGN & ASSERT"})).unwrap(),
        "ALIGN(8) < CAPALIGN & ASSERT"
    );
}

#[test]
fn bad_templates_are_errors() {
    for template in ["{{missing}}", "{{#x}}", "{{/x}}", "{{>unknown}}"] {
        assert!(render_text(template, &json!({"x":true})).is_err())
    }
    let partials = BTreeMap::from([("loop".into(), "{{>loop}}".into())]);
    assert!(render("{{>loop}}", &json!({}), &partials, EscapeMode::Text).is_err());
}

#[test]
fn tracked_templates_include_only_reachable_transitive_partials() {
    let partials = BTreeMap::from([
        ("outer".into(), "before {{>inner}}".into()),
        ("inner".into(), "{{name}}".into()),
        ("unused".into(), "unrelated".into()),
    ]);
    let result = cheriot_compose::render::template::render_tracked(
        "{{>outer}}",
        &json!({"name":"value"}),
        &partials,
        EscapeMode::Text,
    )
    .unwrap();
    assert_eq!(result.content, "before value");
    assert_eq!(
        result
            .partials
            .keys()
            .map(String::as_str)
            .collect::<Vec<_>>(),
        ["inner", "outer"]
    );
    assert!(result.variables.contains("name"));
}
