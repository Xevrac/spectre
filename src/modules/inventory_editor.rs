use super::Module;
use eframe::egui;

pub struct InventoryEditor;

impl Default for InventoryEditor {
    fn default() -> Self {
        Self
    }
}

impl Module for InventoryEditor {
    fn name(&self) -> &str {
        "Inventory Editor"
    }

    fn show(&mut self, _ctx: &egui::Context, ui: &mut egui::Ui) {
        ui.heading("Inventory Editor");
        ui.label("This module will edit inventory files.");
        // TODO: Implement inventory editor functionality
    }
}
