// Copyright 3Bian Limited and Contributors
// SPDX-License-Identifier: MIT

pub mod board;
pub mod firmware;
pub mod layout;
pub mod linker_segment;
pub mod sdk;
pub mod test_suite;
pub mod threads;

#[cfg(test)]
#[path = "../tests/calculations/firmware.rs"]
mod firmware_tests;
