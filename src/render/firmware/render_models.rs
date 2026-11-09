// Copyright 3Bian Limited and Contributors
// SPDX-License-Identifier: MIT

mod component_model;
mod firmware_layout_model;
mod firmware_model;
mod fragment_context;
mod fragment_sections;
mod interrupt_entry;
mod interrupt_model;
mod memory_span;
mod mmio_model;
mod shared_object_model;
mod thread_model;

pub(super) use firmware_layout_model::FirmwareLayoutModel;
pub(super) use firmware_model::FirmwareModel;
pub(super) use fragment_context::FragmentContext;
pub(super) use fragment_sections::FragmentSections;
pub(super) use interrupt_model::InterruptModel;
pub(super) use mmio_model::MmioModel;

fn hexadecimal(value: u32) -> String {
    format!("0x{value:x}")
}
