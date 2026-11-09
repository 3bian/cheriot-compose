// Copyright 3Bian Limited and Contributors
// SPDX-License-Identifier: MIT

use serde::Serialize;

use super::fragment_sections::FragmentSections;

#[derive(Serialize)]
pub(crate) struct FragmentContext<'a, T: Serialize> {
    #[serde(flatten)]
    pub sections: &'a FragmentSections,
    #[serde(flatten)]
    pub segment: &'a T,
    pub heap_start: &'a str,
    pub thread_count: usize,
    pub library_count: usize,
    pub compartment_count: usize,
}
