// Copyright 3Bian Limited and Contributors
// SPDX-License-Identifier: MIT

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum BuildOperationKind {
    Generate,
    Compile,
    RustCompile,
    CargoBuild,
    Assemble,
    PartialLink,
    Link,
    PostProcess,
    Probe,
}
