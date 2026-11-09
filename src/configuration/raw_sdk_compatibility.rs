// Copyright 3Bian Limited and Contributors
// SPDX-License-Identifier: MIT

use serde::{Deserialize, Serialize};

mod raw_sdk_requirement;
pub use raw_sdk_requirement::RawSdkRequirement;

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RawSdkCompatibility {
    pub integration: String,
    #[serde(default)]
    pub required_directories: Vec<String>,
    pub required_files: Vec<RawSdkRequirement>,
}
