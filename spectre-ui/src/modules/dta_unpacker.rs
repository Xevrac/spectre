use super::Module;
use eframe::egui;

pub struct DtaUnpacker;

impl Default for DtaUnpacker {
    fn default() -> Self {
        Self
    }
}

impl Module for DtaUnpacker {
    fn name(&self) -> &str {
        "DTA Unpacker"
    }

    fn show(&mut self, _ctx: &egui::Context, ui: &mut egui::Ui) {
        ui.heading("DTA Unpacker");
        ui.label("This module will unpack DTA files.");
    }
}

