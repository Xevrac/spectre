#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod config;
mod modules;
mod splash;

use config::Config;
use eframe::egui;
use egui::IconData;
use image::GenericImageView;
use std::sync::Arc;
use modules::{
    DtaUnpacker, GamedataEditor, InventoryEditor, ItemsEditor,
    MpmaplistEditor, Module, ServerLauncher,
};
use splash::SplashScreen;

const VERSION: &str = env!("CARGO_PKG_VERSION");
const AUTHOR: &str = "Xevrac";
const ABOUT: &str = "Spectre is a toolkit for Hidden & Dangerous 2, providing various editing and management tools for the game.";

const CREDITS: &[&str] = &[
    "Fis",
    "Stern",
    "snowmanflo",
    "Jovan Stanojlovic",
    "RellHaiser",
];

fn get_banner_size() -> Option<(f32, f32)> {
    let banner_bytes = include_bytes!("../spectre-banner.png");
    if let Ok(image) = image::load_from_memory(banner_bytes) {
        let (width, height) = image.dimensions();
        return Some((width as f32, height as f32));
    }
    None
}

fn load_icon() -> Option<Arc<IconData>> {
    let icon_bytes = include_bytes!("../spectre_256.png");
    
    if let Ok(image) = image::load_from_memory(icon_bytes) {
        let rgba = image.to_rgba8();
        let (width, height) = rgba.dimensions();
        let pixels = rgba.as_flat_samples();
        
        return Some(Arc::new(IconData {
            rgba: pixels.as_slice().to_vec(),
            width,
            height,
        }));
    }
    
    Some(Arc::new(create_default_icon()))
}

fn create_default_icon() -> IconData {
    let size: u32 = 256;
    let size_usize = size as usize;
    let mut rgba = Vec::with_capacity(size_usize * size_usize * 4);
    for y in 0..size {
        for x in 0..size {
            let r = ((x * 255) / size) as u8;
            let g = ((y * 255) / size) as u8;
            let b = 128u8;
            let a = 255u8;
            rgba.extend_from_slice(&[r, g, b, a]);
        }
    }
    IconData {
        rgba,
        width: size,
        height: size,
    }
}

fn main() -> Result<(), eframe::Error> {
    println!("[DEBUG] Spectre v{} starting...", env!("CARGO_PKG_VERSION"));
    
    let banner_size = get_banner_size().unwrap_or((1024.0, 420.0));
    let window_size = (banner_size.0 / 2.0, banner_size.1 / 2.0);
    println!("[DEBUG] Banner size: {}x{} (scaled window: {}x{})", banner_size.0, banner_size.1, window_size.0, window_size.1);
    
    let mut viewport_builder = egui::ViewportBuilder::default()
        .with_inner_size([window_size.0, window_size.1])
        .with_title("Spectre")
        .with_decorations(false); 
    
    if let Some(icon) = load_icon() {
        println!("[DEBUG] Application icon loaded successfully");
        viewport_builder = viewport_builder.with_icon(icon);
    } else {
        println!("[DEBUG] Warning: Failed to load application icon, using default");
    }
    
    let options = eframe::NativeOptions {
        viewport: viewport_builder,
        ..Default::default()
    };

    println!("[DEBUG] Initializing eframe application...");
    eframe::run_native(
        "Spectre",
        options,
        Box::new(|cc| Box::new(SpectreApp::new(cc))),
    )
}

struct SpectreApp {
    version: String,
    config: Config,
    current_module: Option<Box<dyn Module>>,
    show_about: bool,
    show_options: bool,
    splash_screen: Option<SplashScreen>,
    window_centered: bool,
    center_attempts: u32,
}

impl SpectreApp {
    fn new(cc: &eframe::CreationContext<'_>) -> Self {
        println!("[DEBUG] Creating SpectreApp instance...");
        let splash = SplashScreen::new(&cc.egui_ctx);
        println!("[DEBUG] Splash screen initialized");
        
        let config = Config::load();
        println!("[DEBUG] Configuration loaded: theme={}", config.theme);
        
        Self::apply_theme(&cc.egui_ctx, &config.theme);
        
        Self {
            version: VERSION.to_string(),
            config,
            current_module: None,
            show_about: false,
            show_options: false,
            splash_screen: Some(splash),
            window_centered: false,
            center_attempts: 0,
        }
    }
    
    fn apply_theme(ctx: &egui::Context, theme: &str) {
        match theme {
            "light" => {
                println!("[DEBUG] Applying light theme");
                ctx.set_visuals(egui::Visuals::light());
            }
            "dark" | _ => {
                println!("[DEBUG] Applying dark theme");
                ctx.set_visuals(egui::Visuals::dark());
            }
        }
    }
}

impl eframe::App for SpectreApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if !self.window_centered && self.center_attempts < 15 {
            self.center_attempts += 1;
            
            let monitor_size = ctx.input(|i| i.viewport().monitor_size);
            let screen_size = ctx.screen_rect().size();
            
            let size_to_use = monitor_size
                .filter(|s| s.x > 100.0 && s.y > 100.0)
                .or_else(|| {
                    if screen_size.x > 100.0 && screen_size.y > 100.0 {
                        Some(screen_size)
                    } else {
                        None
                    }
                });
            
            if let Some(monitor_size) = size_to_use {
                let banner_size = get_banner_size().unwrap_or((1024.0, 420.0));
                let window_size = (banner_size.0 / 2.0, banner_size.1 / 2.0);
                let center_x = (monitor_size.x - window_size.0) / 2.0;
                let center_y = (monitor_size.y - window_size.1) / 2.0;
                
                if self.center_attempts == 1 {
                    println!("[DEBUG] Centering splash window (attempt {}): monitor={}x{}, window={}x{}, pos=({}, {})", 
                             self.center_attempts, monitor_size.x, monitor_size.y, window_size.0, window_size.1, center_x, center_y);
                }
                
                ctx.send_viewport_cmd(egui::ViewportCommand::OuterPosition(egui::pos2(center_x.max(0.0), center_y.max(0.0))));
                
                if self.center_attempts >= 3 {
                    self.window_centered = true;
                    println!("[DEBUG] Splash window centering complete");
                }
            }
        }
        
        if let Some(ref mut splash) = self.splash_screen {
            if !splash.show(ctx) {
                println!("[DEBUG] Splash screen finished, transitioning to main application");
                self.splash_screen = None;
                ctx.send_viewport_cmd(egui::ViewportCommand::Decorations(true));
                ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(egui::vec2(1000.0, 700.0)));
                println!("[DEBUG] Window resized to 1000x700 with decorations enabled");
                let monitor_size = ctx.input(|i| i.viewport().monitor_size);
                if let Some(monitor_size) = monitor_size {
                    let center_x = (monitor_size.x - 1000.0) / 2.0;
                    let center_y = (monitor_size.y - 700.0) / 2.0;
                    ctx.send_viewport_cmd(egui::ViewportCommand::OuterPosition(egui::pos2(center_x, center_y)));
                    println!("[DEBUG] Main window re-centered at: ({}, {})", center_x, center_y);
                } else {
                    let screen_size = ctx.screen_rect().size();
                    let center_x = (screen_size.x - 1000.0) / 2.0;
                    let center_y = (screen_size.y - 700.0) / 2.0;
                    ctx.send_viewport_cmd(egui::ViewportCommand::OuterPosition(egui::pos2(center_x, center_y)));
                    println!("[DEBUG] Main window re-centered (fallback) at: ({}, {})", center_x, center_y);
                }
            } else {
                return;
            }
        }
        
        if let Some(ref splash) = self.splash_screen {
            if splash.is_fading_out() {
                egui::Area::new(egui::Id::new("fade_overlay"))
                    .interactable(false)
                    .show(ctx, |ui| {
                        let screen_rect = ctx.screen_rect();
                        let painter = ui.painter();
                        let fade_alpha = splash.get_fade_out_alpha();
                        painter.rect_filled(
                            screen_rect,
                            0.0,
                            egui::Color32::from_rgba_unmultiplied(128, 128, 128, (255.0 * fade_alpha) as u8),
                        );
                    });
            }
        }
        
        egui::TopBottomPanel::top("menu_bar").show(ctx, |ui| {
            egui::menu::bar(ui, |ui| {
                ui.menu_button("Tool", |ui| {

                    ui.label(egui::RichText::new("Tools").strong());

                    if ui.button("Server Utility").clicked() {
                        println!("[DEBUG] Module switched: Server Launcher");
                        self.current_module = Some(Box::new(ServerLauncher::default()));
                        ui.close_menu();
                    }
                    if ui.button("DTA Unpacker").clicked() {
                        println!("[DEBUG] Module switched: DTA Unpacker");
                        self.current_module = Some(Box::new(DtaUnpacker::default()));
                        ui.close_menu();
                    }

                    ui.separator();
                    ui.label(egui::RichText::new("Editors").strong());
                    
                    if ui.button("Inventory").clicked() {
                        println!("[DEBUG] Module switched: Inventory Editor");
                        self.current_module = Some(Box::new(InventoryEditor::default()));
                        ui.close_menu();
                    }

                    if ui.button("Items").clicked() {
                        println!("[DEBUG] Module switched: Items Editor");
                        self.current_module = Some(Box::new(ItemsEditor::default()));
                        ui.close_menu();
                    }

                    if ui.button("MP Maplist").clicked() {
                        println!("[DEBUG] Module switched: MP Maplist Editor");
                        self.current_module = Some(Box::new(MpmaplistEditor::default()));
                        ui.close_menu();
                    }
                    if ui.button("Gamedata").clicked() {
                        println!("[DEBUG] Module switched: Gamedata Editor");
                        self.current_module = Some(Box::new(GamedataEditor::default()));
                        ui.close_menu();
                    }
                });

                if ui.button("Options").clicked() {
                    println!("[DEBUG] Options dialog opened");
                    self.show_options = true;
                }

                if ui.button("About").clicked() {
                    println!("[DEBUG] About dialog opened");
                    self.show_about = true;
                }
            });
        });

        if self.show_options {
            egui::Window::new("Options")
                .collapsible(false)
                .resizable(true)
                .show(ctx, |ui| {
                    ui.label("Theme:");
                    egui::ComboBox::from_id_source("theme")
                        .selected_text(&self.config.theme)
                        .show_ui(ui, |ui| {
                            if ui.selectable_value(&mut self.config.theme, "dark".to_string(), "Dark").clicked() {
                                println!("[DEBUG] Theme changed to: dark");
                                Self::apply_theme(ctx, "dark");
                                self.config.save();
                            }
                            if ui.selectable_value(&mut self.config.theme, "light".to_string(), "Light").clicked() {
                                println!("[DEBUG] Theme changed to: light");
                                Self::apply_theme(ctx, "light");
                                self.config.save();
                            }
                        });

                    ui.separator();

                    if ui.button("Close").clicked() {
                        println!("[DEBUG] Options dialog closed");
                        self.show_options = false;
                    }
                });
        }

        if self.show_about {
            egui::Window::new("About")
                .collapsible(false)
                .resizable(true)
                .default_size([400.0, 500.0])
                .show(ctx, |ui| {
                    egui::ScrollArea::vertical().show(ui, |ui| {
                        ui.vertical_centered(|ui| {
                            ui.heading("Spectre");
                            ui.add_space(10.0);
                            ui.label(format!("Version: {}", self.version));
                            ui.add_space(10.0);
                            ui.label(format!("Author: {}", AUTHOR));
                            ui.add_space(10.0);
                            ui.separator();
                            ui.add_space(10.0);
                            ui.label(ABOUT);
                            ui.add_space(20.0);
                            
                            ui.separator();
                            ui.add_space(10.0);
                            
                            ui.label(egui::RichText::new("Credits").strong().size(16.0));
                            ui.add_space(10.0);
                            
                            if CREDITS.is_empty() {
                                ui.label(egui::RichText::new("No credits to display.").italics());
                            } else {
                                ui.label(CREDITS.join(", "));
                            }
                            
                            ui.add_space(20.0);
                            if ui.button("Close").clicked() {
                                println!("[DEBUG] About dialog closed");
                                self.show_about = false;
                            }
                        });
                    });
                });
        }

        egui::CentralPanel::default().show(ctx, |ui| {
            if let Some(ref mut module) = self.current_module {
                module.show(ctx, ui);
            } else {
                ui.vertical_centered(|ui| {
                    ui.add_space(50.0);

                    ui.heading(egui::RichText::new("Spectre")
                        .size(48.0)
                        .color(egui::Color32::from_rgb(180, 180, 180)));

                    ui.add_space(20.0);

                    ui.label(egui::RichText::new("Hidden & Dangerous 2 Toolkit")
                        .size(20.0)
                        .color(egui::Color32::from_rgb(100, 100, 100)));

                    ui.add_space(40.0);

                    ui.label(egui::RichText::new("Select a tool from the Tool menu to get started.")
                        .size(16.0));

                    ui.add_space(30.0);

                    ui.label(egui::RichText::new(format!("Version: {}", self.version))
                        .family(egui::FontFamily::Monospace)
                        .size(14.0)
                        .color(egui::Color32::from_rgb(180, 180, 180)));
                });
            }
        });
    }
}
