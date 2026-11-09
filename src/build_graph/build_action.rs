// Copyright 3Bian Limited and Contributors
// SPDX-License-Identifier: MIT

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use super::{BuildCommand, ConditionalArguments};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum BuildAction {
    Publish,
    Copy {
        input: PathBuf,
    },
    Generate {
        content: String,
        template: String,
        template_fingerprint: String,
    },
    MemoryImage {
        input: PathBuf,
        word_bytes: u8,
    },
    GitDescription {
        command: BuildCommand,
    },
    Command {
        command: BuildCommand,
        conditional: Option<ConditionalArguments>,
    },
    Probe {
        command: BuildCommand,
    },
}
