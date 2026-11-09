// Copyright 3Bian Limited and Contributors
// SPDX-License-Identifier: MIT

use serde::Serialize;

use super::{
    owned_record_model::OwnedRecordModel, publication_render_model::PublicationRenderModel,
    state_identity_model::StateIdentityModel,
};

#[derive(Serialize)]
pub(super) struct PreludeModel {
    pub(super) build_lock: String,
    pub(super) build_dir: String,
    pub(super) publication: Option<PublicationRenderModel>,
    pub(super) owned_record: Option<OwnedRecordModel>,
    pub(super) state_identity: Option<StateIdentityModel>,
}
