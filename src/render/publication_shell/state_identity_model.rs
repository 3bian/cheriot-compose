// Copyright 3Bian Limited and Contributors
// SPDX-License-Identifier: MIT

use serde::Serialize;

#[derive(Serialize)]
pub(super) struct StateIdentityModel {
    pub(super) identity_path: String,
    pub(super) owner: String,
}
