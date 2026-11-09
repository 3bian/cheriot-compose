// Copyright 3Bian Limited and Contributors
// SPDX-License-Identifier: MIT

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use serde_json::Value as JsonValue;
use toml_edit::{Document, Item, Table, Value};

use super::{ConfigurationValue, ConfigurationValueKind};
use crate::diagnostic::{Diagnostic, Origin, Result};

fn origin(path: &Path, source: &str, offset: usize, field: &str) -> Origin {
    let prefix = source.get(..offset).unwrap_or("");
    let line = prefix.bytes().filter(|byte| *byte == b'\n').count() as u64 + 1;
    let column = prefix.rsplit('\n').next().unwrap_or("").chars().count() as u64 + 1;
    Origin {
        file: path.display().to_string(),
        line,
        column,
        field: field.into(),
        rule: "TOML value".into(),
        source: crate::diagnostic::OriginSource::File,
        expansion: None,
    }
}

fn table_value(
    table: &Table,
    path: &Path,
    source: &str,
    field: &str,
) -> Result<ConfigurationValue> {
    let mut entries = BTreeMap::new();
    for (key, item) in table.iter() {
        entries.insert(
            key.to_string(),
            item_value(item, path, source, &format!("{field}/{key}"))?,
        );
    }
    Ok(ConfigurationValue {
        origin: origin(
            path,
            source,
            table.span().map(|span| span.start).unwrap_or(0),
            field,
        ),
        kind: ConfigurationValueKind::Mapping(entries),
    })
}

fn item_value(item: &Item, path: &Path, source: &str, field: &str) -> Result<ConfigurationValue> {
    match item {
        Item::Value(value) => typed_value(value, path, source, field),
        Item::Table(table) => table_value(table, path, source, field),
        Item::ArrayOfTables(tables) => {
            let mut entries = Vec::new();
            for (index, table) in tables.iter().enumerate() {
                entries.push(table_value(
                    table,
                    path,
                    source,
                    &format!("{field}/{index}"),
                )?);
            }
            Ok(ConfigurationValue {
                origin: origin(
                    path,
                    source,
                    item.span().map(|span| span.start).unwrap_or(0),
                    field,
                ),
                kind: ConfigurationValueKind::Sequence(entries),
            })
        }
        Item::None => Err(Diagnostic::new("empty TOML item").at(origin(path, source, 0, field))),
    }
}

fn typed_value(
    value: &Value,
    path: &Path,
    source: &str,
    field: &str,
) -> Result<ConfigurationValue> {
    let value_origin = origin(
        path,
        source,
        value.span().map(|span| span.start).unwrap_or(0),
        field,
    );
    let kind = match value {
        Value::String(string) => {
            ConfigurationValueKind::Scalar(JsonValue::String(string.value().clone()))
        }
        Value::Integer(integer) => {
            ConfigurationValueKind::Scalar(JsonValue::from(*integer.value()))
        }
        Value::Boolean(boolean) => {
            ConfigurationValueKind::Scalar(JsonValue::Bool(*boolean.value()))
        }
        Value::Float(float) => ConfigurationValueKind::Scalar(
            serde_json::Number::from_f64(*float.value())
                .map(JsonValue::Number)
                .ok_or_else(|| {
                    Diagnostic::new("non-finite numbers are not supported").at(value_origin.clone())
                })?,
        ),
        Value::Datetime(_) => {
            return Err(
                Diagnostic::new("date and time values are not supported in configuration")
                    .at(value_origin),
            );
        }
        Value::Array(array) => {
            let mut entries = Vec::new();
            for (index, item) in array.iter().enumerate() {
                entries.push(typed_value(
                    item,
                    path,
                    source,
                    &format!("{field}/{index}"),
                )?);
            }
            ConfigurationValueKind::Sequence(entries)
        }
        Value::InlineTable(table) => {
            let mut entries = BTreeMap::new();
            for (key, item) in table.iter() {
                entries.insert(
                    key.to_string(),
                    typed_value(item, path, source, &format!("{field}/{key}"))?,
                );
            }
            ConfigurationValueKind::Mapping(entries)
        }
    };
    Ok(ConfigurationValue {
        origin: value_origin,
        kind,
    })
}

pub(super) fn read_toml_document(path: &Path) -> Result<ConfigurationValue> {
    let source = fs::read_to_string(path).map_err(|error| Diagnostic::io(path, error))?;
    if source.len() > 4 * 1024 * 1024 {
        return Err(Diagnostic::new("configuration exceeds 4 MiB limit"));
    }
    let document = Document::parse(source.as_str()).map_err(|error| {
        let mut position = origin(
            path,
            &source,
            error.span().map(|span| span.start).unwrap_or(0),
            "/",
        );
        position.rule = "TOML syntax".into();
        Diagnostic::new(error.to_string()).at(position)
    })?;
    table_value(document.as_table(), path, &source, "")
}
