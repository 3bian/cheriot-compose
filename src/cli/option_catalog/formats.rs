// Copyright 3Bian Limited and Contributors
// SPDX-License-Identifier: MIT

use super::output_format::OutputFormat;

pub(in crate::cli) struct Formats {
    pub default: OutputFormat,
    pub supported: &'static [OutputFormat],
}
