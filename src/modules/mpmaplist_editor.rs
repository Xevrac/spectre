use super::Module;
use eframe::egui;

pub struct MpmaplistEditor;

impl Default for MpmaplistEditor {
    fn default() -> Self {
        Self
    }
}

impl Module for MpmaplistEditor {
    fn name(&self) -> &str {
        "MP Maplist Editor"
    }

    fn show(&mut self, _ctx: &egui::Context, ui: &mut egui::Ui) {
        ui.heading("MP Maplist Editor");
        ui.label("This module will edit multiplayer maplist files.");
        // TODO: Implement MP maplist editor functionality
    }
}
