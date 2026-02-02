pub mod dta_unpacker;
pub mod inventory_editor;
pub mod server_launcher;
pub mod mpmaplist_editor;
pub mod items_editor;
pub mod gamedata_editor;

pub use dta_unpacker::DtaUnpacker;
pub use inventory_editor::InventoryEditor;
pub use server_launcher::ServerLauncher;
pub use mpmaplist_editor::MpmaplistEditor;
pub use items_editor::ItemsEditor;
pub use gamedata_editor::GamedataEditor;

use eframe::egui;

pub trait Module {
    #[allow(dead_code)]
    fn name(&self) -> &str;
    fn show(&mut self, ctx: &egui::Context, ui: &mut egui::Ui);
}
