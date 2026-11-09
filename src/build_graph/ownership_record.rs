// Copyright 3Bian Limited and Contributors
// SPDX-License-Identifier: MIT

use std::{collections::BTreeSet, path::PathBuf};

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct OwnershipRecord {
    pub owner: String,
    pub outputs: BTreeSet<PathBuf>,
}
