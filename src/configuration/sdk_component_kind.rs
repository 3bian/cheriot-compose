// Copyright 3Bian Limited and Contributors
// SPDX-License-Identifier: MIT

use serde::Serialize;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub enum SdkComponentKind {
    Compartment,
    Library,
    PrivilegedCompartment,
    PrivilegedLibrary,
    Loader,
    Object,
    Group,
}
