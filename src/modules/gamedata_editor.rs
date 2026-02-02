use super::Module;
use eframe::egui;

pub struct GamedataEditor;

impl Default for GamedataEditor {
    fn default() -> Self {
        Self
    }
}

impl Module for GamedataEditor {
    fn name(&self) -> &str {
        "Gamedata Editor"
    }

    fn show(&mut self, _ctx: &egui::Context, ui: &mut egui::Ui) {
        ui.heading("Gamedata Editor");
        ui.label("This module will edit gamedata00.gdt and gamedata01.gdt files.");
        // TODO: Implement gamedata editor functionality
    }
}
