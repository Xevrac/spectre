use super::Module;
use eframe::egui;

pub struct ServerLauncher;

impl Default for ServerLauncher {
    fn default() -> Self {
        Self
    }
}

impl Module for ServerLauncher {
    fn name(&self) -> &str {
        "Server Launcher"
    }

    fn show(&mut self, _ctx: &egui::Context, ui: &mut egui::Ui) {
        ui.heading("Server Launcher");
        ui.label("This module will launch game servers.");
        // TODO: Implement server launcher functionality
    }
}
