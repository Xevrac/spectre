use super::Module;
use crate::config::Config;
use crate::server_prereqs::{
    gamepy_hosts_applied, registry_fix_applied, spawn_elevated_apply_hosts,
    spawn_elevated_apply_registry, spawn_elevated_check_directplay, spawn_elevated_install_directplay,
};
use eframe::egui;
use egui::TextureHandle;
use spectre_core::server::{Server, ServerConfig, ServerLauncherData};
use std::fs;
use std::path::Path;
use std::sync::mpsc;
use std::time::Instant;

const CONFIGS_DIR: &str = "content/server_utility";
const CONFIG_FILENAME: &str = "hd2_server_config.json";

const WIZARD_STEPS: usize = 1;

pub struct ServerLauncher {
    data: ServerLauncherData,
    config_path: String,
    show_first_time_wizard: bool,
    first_time_wizard_step: usize,
    check_icon: Option<TextureHandle>,
    cross_icon: Option<TextureHandle>,
    config: Config,
    registry_fix_error: Option<String>,
    hosts_fix_error: Option<String>,
    registry_elevate_rx: Option<mpsc::Receiver<Result<(), String>>>,
    hosts_elevate_rx: Option<mpsc::Receiver<Result<(), String>>>,
    prereq_cache: Option<(bool, bool)>,
    prereq_cache_time: Option<Instant>,
    directplay_detection_result: Option<bool>,
    directplay_check_rx: Option<mpsc::Receiver<Result<bool, String>>>,
    directplay_install_rx: Option<mpsc::Receiver<Result<(), String>>>,
    directplay_error: Option<String>,
}

impl ServerLauncher {
    fn load_icons(ctx: &egui::Context) -> (Option<TextureHandle>, Option<TextureHandle>) {
        fn load_svg_icon(ctx: &egui::Context, name: &str, size: f32) -> Option<TextureHandle> {
            let svg_bytes: &[u8] = match name {
                "check" => include_bytes!("../../icons/check.svg"),
                "cross" => include_bytes!("../../icons/cross.svg"),
                _ => return None,
            };
            let opt = resvg::usvg::Options::default();
            let rtree = match resvg::usvg::Tree::from_data(svg_bytes, &opt) {
                Ok(tree) => tree,
                Err(_) => return None,
            };
            let mut pixmap = match tiny_skia::Pixmap::new(size as u32, size as u32) {
                Some(p) => p,
                None => return None,
            };
            let tree_size = rtree.size();
            let transform = tiny_skia::Transform::from_scale(
                size / tree_size.width(),
                size / tree_size.height(),
            );
            resvg::render(&rtree, transform, &mut pixmap.as_mut());
            let rgba = pixmap.data();
            let color_image = egui::ColorImage::from_rgba_unmultiplied(
                [size as usize, size as usize],
                rgba,
            );
            Some(ctx.load_texture(format!("icon_{}", name), color_image, Default::default()))
        }
        let size = 16.0;
        let check = load_svg_icon(ctx, "check", size);
        let cross = load_svg_icon(ctx, "cross", size);
        (check, cross)
    }
}

impl Default for ServerLauncher {
    fn default() -> Self {
        if let Err(e) = fs::create_dir_all(CONFIGS_DIR) {
            println!("[Spectre.dbg] Could not create configs dir {}: {}", CONFIGS_DIR, e);
        }
        let app_config = Config::load();
        let config_path = format!("{}/{}", CONFIGS_DIR, CONFIG_FILENAME);
        let mut data = ServerLauncherData::load_from_file(Path::new(&config_path))
            .unwrap_or_else(|_| ServerLauncherData::default());

        if data.servers.is_empty() {
            let mut server = Server::default();
            server.name = "Server 1".to_string();
            server.port = 22000;
            let mut default_config = ServerConfig::default();
            default_config.name = "Default".to_string();
            default_config.session_name = "A Spectre Session".to_string();
            default_config.style = "Occupation".to_string();
            server.current_config = default_config.name.clone();
            server.configs.push(default_config);
            data.servers.push(server);
        }
        let show_first_time_wizard = !app_config.server_utility_wizard_completed;
        let directplay_from_config = app_config.directplay_detected;
        let directplay_detection_result = if directplay_from_config {
            println!("[Spectre.dbg] DirectPlay: loaded from config (previously detected as enabled)");
            Some(true)
        } else {
            None
        };

        Self {
            data,
            config_path,
            show_first_time_wizard,
            first_time_wizard_step: 0,
            check_icon: None,
            cross_icon: None,
            config: app_config,
            registry_fix_error: None,
            hosts_fix_error: None,
            registry_elevate_rx: None,
            hosts_elevate_rx: None,
            prereq_cache: None,
            prereq_cache_time: None,
            directplay_detection_result,
            directplay_check_rx: None,
            directplay_install_rx: None,
            directplay_error: None,
        }
    }
}

impl Module for ServerLauncher {
    fn name(&self) -> &str {
        "Server Utility"
    }

    fn show(&mut self, ctx: &egui::Context, ui: &mut egui::Ui) {
        if self.check_icon.is_none() {
            let (check, cross) = Self::load_icons(ctx);
            self.check_icon = check;
            self.cross_icon = cross;
        }

        self.config = Config::load();
        if !self.show_first_time_wizard && !self.config.server_utility_wizard_completed {
            self.show_first_time_wizard = true;
        }

        if self.show_first_time_wizard {
            self.show_first_time_wizard_dialog(ctx);
            return;
        }

        ui.label(
            egui::RichText::new("Server Utility is available as a web interface on Windows. Use the first-time setup to configure prerequisites.")
                .color(ui.visuals().weak_text_color()),
        );
    }
}

impl ServerLauncher {
    fn show_first_time_wizard_dialog(&mut self, ctx: &egui::Context) {
        const WIZARD_WIDTH: f32 = 520.0;
        const WIZARD_HEIGHT: f32 = 420.0;

        let screen = ctx.screen_rect();
        let center_x = screen.center().x - WIZARD_WIDTH / 2.0;
        let center_y = screen.center().y - WIZARD_HEIGHT / 2.0;

        let step = self.first_time_wizard_step.min(WIZARD_STEPS.saturating_sub(1));

        const PREREQ_CACHE_TTL_SECS: u64 = 2;
        let (registry_ok_cached, hosts_ok_cached) = if step == 0 {
            let now = Instant::now();
            let stale = self
                .prereq_cache_time
                .map(|t| now.duration_since(t).as_secs() >= PREREQ_CACHE_TTL_SECS)
                .unwrap_or(true);
            if stale {
                let r = registry_fix_applied();
                let h = gamepy_hosts_applied();
                self.prereq_cache = Some((r, h));
                self.prereq_cache_time = Some(now);
                (r, h)
            } else {
                self.prereq_cache.unwrap_or((false, false))
            }
        } else {
            (false, false)
        };

        let directplay_ok = step == 0 && self.directplay_detection_result == Some(true);
        let step_valid = directplay_ok && registry_ok_cached && hosts_ok_cached;

        let mut next_clicked = false;
        let mut back_clicked = false;
        let mut finish_clicked = false;
        let mut apply_registry_clicked = false;
        let mut apply_hosts_clicked = false;

        egui::Window::new("Server Utility — First-time Setup")
            .collapsible(false)
            .resizable(false)
            .movable(false)
            .fixed_pos(egui::pos2(center_x, center_y))
            .fixed_size(egui::vec2(WIZARD_WIDTH, WIZARD_HEIGHT))
            .show(ctx, |ui| {
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new(format!("Step {} of {}", step + 1, WIZARD_STEPS))
                            .strong(),
                    );
                });
                ui.add_space(12.0);

                if step == 0 {
                    if let Some(rx) = &self.registry_elevate_rx {
                        if let Ok(result) = rx.try_recv() {
                            match &result {
                                Ok(()) => println!("[Spectre.dbg] Prereqs: registry fix elevated process succeeded"),
                                Err(e) => println!("[Spectre.dbg] Prereqs: registry fix elevated process failed: {}", e),
                            }
                            self.registry_fix_error = result.err();
                            self.registry_elevate_rx = None;
                            self.prereq_cache = None;
                            self.prereq_cache_time = None;
                        }
                    }
                    if let Some(rx) = &self.hosts_elevate_rx {
                        if let Ok(result) = rx.try_recv() {
                            match &result {
                                Ok(()) => println!("[Spectre.dbg] Prereqs: GameSpy hosts elevated process succeeded"),
                                Err(e) => println!("[Spectre.dbg] Prereqs: GameSpy hosts elevated process failed: {}", e),
                            }
                            self.hosts_fix_error = result.err();
                            self.hosts_elevate_rx = None;
                            self.prereq_cache = None;
                            self.prereq_cache_time = None;
                        }
                    }
                    if let Some(rx) = &self.directplay_check_rx {
                        if let Ok(result) = rx.try_recv() {
                            self.directplay_check_rx = None;
                            self.directplay_error = result.as_ref().err().cloned();
                            self.directplay_detection_result = result.ok();
                            match &self.directplay_detection_result {
                                Some(true) => {
                                    println!("[Spectre.dbg] DirectPlay: detection result=enabled, saving to config (bound to this machine)");
                                    self.config.directplay_detected = true;
                                    self.config.machine_id = Some(crate::config::get_machine_id());
                                    self.config.save();
                                }
                                Some(false) => println!("[Spectre.dbg] DirectPlay: detection result=disabled"),
                                None => println!("[Spectre.dbg] DirectPlay: detection failed ({})", self.directplay_error.as_deref().unwrap_or("unknown")),
                            }
                        }
                    }
                    if let Some(rx) = &self.directplay_install_rx {
                        if let Ok(result) = rx.try_recv() {
                            self.directplay_install_rx = None;
                            self.directplay_error = result.as_ref().err().cloned();
                            if result.is_ok() {
                                println!("[Spectre.dbg] DirectPlay: install succeeded, saving to config (bound to this machine)");
                                self.directplay_detection_result = Some(true);
                                self.config.directplay_detected = true;
                                self.config.machine_id = Some(crate::config::get_machine_id());
                                self.config.save();
                            } else {
                                println!("[Spectre.dbg] DirectPlay: install failed ({})", self.directplay_error.as_deref().unwrap_or("unknown"));
                            }
                        }
                    }

                    ui.label(
                        "HD2 dedicated servers require these Windows prerequisites before you set paths:",
                    );
                    ui.add_space(12.0);

                    let registry_ok = registry_ok_cached;
                    let hosts_ok = hosts_ok_cached;

                    let directplay_pending = self.directplay_check_rx.is_some() || self.directplay_install_rx.is_some();
                    ui.horizontal(|ui| {
                        if directplay_ok {
                            if let Some(ref icon) = self.check_icon {
                                let size = 16.0;
                                ui.image((icon.id(), egui::vec2(size, size)));
                                ui.add_space(6.0);
                            }
                            ui.colored_label(
                                egui::Color32::from_rgb(80, 180, 80),
                                "DirectPlay (Windows Optional Feature) is enabled.",
                            );
                        } else {
                            if let Some(ref icon) = self.cross_icon {
                                let size = 16.0;
                                ui.image((icon.id(), egui::vec2(size, size)));
                                ui.add_space(6.0);
                            }
                            let msg = match self.directplay_detection_result {
                                None if directplay_pending => "Checking…",
                                None => "DirectPlay status unknown.",
                                Some(false) => "DirectPlay is not enabled.",
                                Some(true) => "",
                            };
                            if !msg.is_empty() {
                                ui.colored_label(
                                    egui::Color32::from_rgb(220, 80, 80),
                                    msg,
                                );
                            }
                        }
                    });
                    if !directplay_ok {
                        ui.label(
                            egui::RichText::new("Click Run detection to check if DirectPlay is installed on your system (a UAC prompt will appear).")
                                .size(12.0)
                                .color(ui.visuals().weak_text_color()),
                        );
                    }
                    if !directplay_ok && !directplay_pending {
                        match self.directplay_detection_result {
                            None => {
                                if ui.button("Run detection").on_hover_text("Runs as administrator to detect DirectPlay.").clicked() {
                                    println!("[Spectre.dbg] DirectPlay: user clicked Run detection");
                                    self.directplay_error = None;
                                    let (tx, rx) = mpsc::channel();
                                    let path = std::env::temp_dir().join("spectre_directplay_check.txt");
                                    spawn_elevated_check_directplay(tx, path);
                                    self.directplay_check_rx = Some(rx);
                                }
                                #[cfg(debug_assertions)]
                                if ui.button("Emulate: not found").on_hover_text("Debug: simulate DirectPlay not installed (no UAC, config not saved).").clicked() {
                                    println!("[Spectre.dbg] DirectPlay: user clicked Emulate not found (debug)");
                                    self.directplay_error = None;
                                    self.directplay_detection_result = Some(false);
                                }
                            }
                            Some(false) => {
                                if ui.button("Install DirectPlay").on_hover_text("Runs as administrator to enable DirectPlay.").clicked() {
                                    println!("[Spectre.dbg] DirectPlay: user clicked Install DirectPlay");
                                    self.directplay_error = None;
                                    let (tx, rx) = mpsc::channel();
                                    spawn_elevated_install_directplay(tx);
                                    self.directplay_install_rx = Some(rx);
                                }
                            }
                            Some(true) => {}
                        }
                    }
                            if let Some(ref err) = self.directplay_error {
                        ui.add_space(4.0);
                        ui.colored_label(
                            egui::Color32::from_rgb(220, 80, 80),
                            format!("Error: {}", err),
                        );
                    }
                    if !directplay_ok {
                        ui.add_space(12.0);
                    }

                    ui.horizontal(|ui| {
                        if registry_ok {
                            if let Some(ref icon) = self.check_icon {
                                let size = 16.0;
                                ui.image((icon.id(), egui::vec2(size, size)));
                                ui.add_space(6.0);
                            }
                            ui.colored_label(
                                egui::Color32::from_rgb(80, 180, 80),
                                "IPv6/DirectPlay registry fix is applied (64-bit).",
                            );
                        } else {
                            if let Some(ref icon) = self.cross_icon {
                                let size = 16.0;
                                ui.image((icon.id(), egui::vec2(size, size)));
                                ui.add_space(6.0);
                            }
                            ui.colored_label(
                                egui::Color32::from_rgb(220, 80, 80),
                                "Registry fix for HD2/DirectPlay is not applied.",
                            );
                        }
                    });
                        if !registry_ok {
                            ui.add_space(4.0);
                            ui.label(
                                egui::RichText::new(
                                    "Hidden & Dangerous 2 relies on DirectPlay via IPv4. This fix allows for servers to work correctly without disabling IPv6 by adding a registry entry for DirectPlay8 IPAddressFamilySettings. \
                                     Click below to apply (a UAC prompt will appear).",
                                )
                                .size(12.0)
                                .color(ui.visuals().weak_text_color()),
                            );
                            ui.add_space(8.0);
                            if ui.button("Apply network fix").clicked() {
                                apply_registry_clicked = true;
                                self.registry_fix_error = None;
                            }
                            if let Some(ref err) = self.registry_fix_error {
                                ui.add_space(4.0);
                                ui.colored_label(
                                    egui::Color32::from_rgb(220, 80, 80),
                                    format!("Error: {}", err),
                                );
                            }
                            ui.add_space(12.0);
                        } else {
                            self.registry_fix_error = None;
                        }

                    ui.horizontal(|ui| {
                        if hosts_ok {
                            if let Some(ref icon) = self.check_icon {
                                let size = 16.0;
                                ui.image((icon.id(), egui::vec2(size, size)));
                                ui.add_space(6.0);
                            }
                            ui.colored_label(
                                egui::Color32::from_rgb(80, 180, 80),
                                "GameSpy hosts file entries are present.",
                            );
                        } else {
                            if let Some(ref icon) = self.cross_icon {
                                let size = 16.0;
                                ui.image((icon.id(), egui::vec2(size, size)));
                                ui.add_space(6.0);
                            }
                            ui.colored_label(
                                egui::Color32::from_rgb(220, 80, 80),
                                "GameSpy hosts file entries are missing.",
                            );
                        }
                    });
                    if !hosts_ok {
                        ui.add_space(4.0);
                        ui.label(
                            egui::RichText::new(
                                "HD2 multiplayer/server list needs GameSpy hostnames in the Windows hosts file. \
                                 Click below to add them (a UAC prompt will appear).",
                            )
                            .size(12.0)
                            .color(ui.visuals().weak_text_color()),
                        );
                        ui.add_space(8.0);
                        if ui.button("Add GameSpy hosts").clicked() {
                            apply_hosts_clicked = true;
                            self.hosts_fix_error = None;
                        }
                        if let Some(ref err) = self.hosts_fix_error {
                            ui.add_space(4.0);
                            ui.colored_label(
                                egui::Color32::from_rgb(220, 80, 80),
                                format!("Error: {}", err),
                            );
                        }
                        ui.add_space(12.0);
                    } else {
                        self.hosts_fix_error = None;
                    }

                    ui.add_space(16.0);
                }

                ui.add_space(16.0);
                ui.separator();
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    if step > 0 {
                        if ui.button("Back").clicked() {
                            back_clicked = true;
                        }
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if step + 1 == WIZARD_STEPS {
                            let finish_enabled = step_valid;
                            if ui
                                .add_enabled(finish_enabled, egui::Button::new("Finish"))
                                .clicked()
                            {
                                finish_clicked = true;
                            }
                        } else {
                            let next_enabled = step_valid;
                            if ui
                                .add_enabled(next_enabled, egui::Button::new("Next"))
                                .clicked()
                            {
                                next_clicked = true;
                            }
                        }
                    });
                });
            });

        if apply_registry_clicked {
            println!("[Spectre.dbg] Prereqs: user clicked Apply network fix, spawning elevated process");
            self.registry_fix_error = None;
            self.prereq_cache = None;
            self.prereq_cache_time = None;
            let (tx, rx) = mpsc::channel();
            spawn_elevated_apply_registry(tx);
            self.registry_elevate_rx = Some(rx);
        }
        if apply_hosts_clicked {
            println!("[Spectre.dbg] Prereqs: user clicked Add GameSpy hosts, spawning elevated process");
            self.hosts_fix_error = None;
            self.prereq_cache = None;
            self.prereq_cache_time = None;
            let (tx, rx) = mpsc::channel();
            spawn_elevated_apply_hosts(tx);
            self.hosts_elevate_rx = Some(rx);
        }

        if back_clicked {
            self.first_time_wizard_step = step.saturating_sub(1);
        }
        if next_clicked {
            self.first_time_wizard_step = (step + 1).min(WIZARD_STEPS.saturating_sub(1));
        }
        if finish_clicked {
            self.config.server_utility_wizard_completed = true;
            self.config.save();
            let _ = self.data.save_to_file(Path::new(&self.config_path));
            self.show_first_time_wizard = false;
            self.first_time_wizard_step = 0;
            ctx.data_mut(|d| d.insert_temp(egui::Id::new("spectre_open_web_after_wizard"), ()));
        }
    }
}
