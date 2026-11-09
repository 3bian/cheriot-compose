// Copyright 3Bian Limited and Contributors
// SPDX-License-Identifier: MIT

use serde::Serialize;

use super::RawThread;

#[derive(Clone, Debug, Serialize)]
pub struct ResolvedThread {
    pub requested: RawThread,
    pub id: u32,
    pub priority: u32,
    pub entry_symbol: String,
}
