// Copyright 3Bian Limited and Contributors
// SPDX-License-Identifier: MIT

use serde::{Deserialize, Serialize};

use super::Origin;

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct Sourced<T> {
    pub value: T,
    pub origins: Vec<Origin>,
}

impl<T> Sourced<T> {
    pub fn new(value: T, origin: Origin) -> Self {
        Self {
            value,
            origins: vec![origin],
        }
    }

    pub fn derived(value: T, rule: &str) -> Self {
        Self::new(value, Origin::derived(rule))
    }
}
