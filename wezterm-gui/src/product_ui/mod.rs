//! Product UI host: embeds egui into WezTerm product mode without eframe/winit.

mod host;
pub mod input;

pub use host::{
    ProductLayoutSpec, ProductUiController, ProductUiFactory, ProductUiFrame, ProductUiHost,
    ProductUiResponse, TerminalVisibility,
};
pub use input::{egui_key_from_wez, egui_modifiers_from_wez, egui_pointer_button};
