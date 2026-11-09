// Copyright 3Bian Limited and Contributors
// SPDX-License-Identifier: MIT

use serde::Serialize;

#[derive(Clone, Debug, Serialize)]
pub struct SdkCompatibility {
    pub integration: String,
}
