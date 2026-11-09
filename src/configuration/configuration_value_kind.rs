// Copyright 3Bian Limited and Contributors
// SPDX-License-Identifier: MIT

use std::collections::BTreeMap;

use serde_json::Value;

use super::ConfigurationValue;

#[derive(Clone, Debug)]
pub(super) enum ConfigurationValueKind {
    Scalar(Value),
    Mapping(BTreeMap<String, ConfigurationValue>),
    Sequence(Vec<ConfigurationValue>),
}
