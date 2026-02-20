use super::Module;
use eframe::egui;

pub struct ItemsEditor;

impl Default for ItemsEditor {
    fn default() -> Self {
        Self
    }
}

impl Module for ItemsEditor {
    fn name(&self) -> &str {
        "Items Editor"
    }

    fn show(&mut self, _ctx: &egui::Context, ui: &mut egui::Ui) {
        ui.heading("Items Editor");
        ui.label("This module will edit item values and create/edit items for the game.");
        // TODO: Implement items editor
    }
}
