// Copyright 3Bian Limited and Contributors
// SPDX-License-Identifier: MIT

use std::collections::BTreeSet;

#[derive(Default)]
pub(super) struct RenderTrace {
    pub(super) partials: BTreeSet<String>,
    pub(super) variables: BTreeSet<String>,
}
