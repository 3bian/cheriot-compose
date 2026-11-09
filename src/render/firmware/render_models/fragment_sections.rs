// Copyright 3Bian Limited and Contributors
// SPDX-License-Identifier: MIT

use serde::Serialize;

#[derive(Serialize)]
pub(crate) struct FragmentSections {
    pub compartment_exports: String,
    pub library_exports: String,
    pub cap_relocs: String,
    pub compartment_headers: String,
    pub pcc_ld: String,
    pub gdc_ld: String,
    pub software_revoker_code: String,
    pub software_revoker_globals: String,
    pub software_revoker_header: String,
    pub sealed_objects: String,
    pub thread_headers: String,
    pub thread_trusted_stacks: String,
    pub thread_stacks: String,
    pub shared_objects: String,
}
