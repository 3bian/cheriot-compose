// Copyright 3Bian Limited and Contributors
// SPDX-License-Identifier: MIT

#[derive(Debug)]
pub(super) enum TemplateToken {
    Text(String),
    Variable(String, bool),
    Section(String, bool, Vec<TemplateToken>),
    Partial(String),
}
