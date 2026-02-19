#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod config;
mod dialog;
mod modules;
mod server_prereqs;
mod splash;

use config::Config;
use eframe::egui;
use egui::{IconData, TextureHandle};
use image::GenericImageView;
use modules::{
    DtaUnpacker, GamedataEditor, InventoryEditor, ItemsEditor, MpmaplistEditor, Module,
    ServerLauncher,
};
use splash::SplashScreen;
use std::path::Path;
use std::sync::mpsc;
use std::sync::Arc;

const VERSION: &str = env!("CARGO_PKG_VERSION");
const AUTHOR: &str = "Xevrac";
const ABOUT: &str = "Spectre is a toolkit for Hidden & Dangerous 2, providing various editing and management tools for the game.";

#[cfg(windows)]
#[derive(serde::Deserialize)]
struct IpcSaveMessage {
    action: String,
    servers: Vec<spectre_core::server::Server>,
}

/// Path to hd2_server_config.json next to the executable so it works when run from file explorer.
#[cfg(windows)]
fn server_utility_config_path() -> std::path::PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(std::path::PathBuf::from))
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join("content")
        .join("server_utility")
        .join("hd2_server_config.json")
}

#[cfg(windows)]
fn ensure_server_utility_has_defaults(data: &mut spectre_core::server::ServerLauncherData) {
    use spectre_core::server::{Server, ServerConfig};
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
}

const CREDITS: &[&str] = &[
    "Xevrac - Spectre",
    "Fis - Source code and concepts",
    "Stern - Community releases and concepts",
    "snowmanflo - Community contributons and commitments",
    "Jovan Stanojlovic - Community releases and knowledge",
    "RellHaiser - Community releases and concepts",
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

fn load_svg_icon(ctx: &egui::Context, name: &str) -> Option<TextureHandle> {
    let svg_bytes: &[u8] = match name {
        "server_launcher" => include_bytes!("../icons/server_launcher.svg"),
        "arrow_up" => include_bytes!("../icons/arrow_up.svg"),
        "arrow_down" => include_bytes!("../icons/arrow_down.svg"),
        "close" => include_bytes!("../icons/close.svg"),
        "home" => include_bytes!("../icons/home.svg"),
        "settings" => include_bytes!("../icons/settings.svg"),
        "info" => include_bytes!("../icons/info.svg"),
        "console" => include_bytes!("../icons/console.svg"),
        "refresh" => include_bytes!("../icons/refresh.svg"),
        _ => return None,
    };
    
    let opt = resvg::usvg::Options::default();
    let rtree = match resvg::usvg::Tree::from_data(svg_bytes, &opt) {
        Ok(tree) => tree,
        Err(_) => return None,
    };
    
    let size = if name == "server_launcher" { 64.0 } else { 16.0 };
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

// No separate exe: WebView is embedded in the main Spectre window (see update() and webview handling).

#[cfg(windows)]
fn get_webview_hwnd(frame: &eframe::Frame) -> Option<windows::Win32::Foundation::HWND> {
    use raw_window_handle::{HasWindowHandle, RawWindowHandle, Win32WindowHandle};
    use windows::Win32::Foundation::LPARAM;
    use windows::Win32::UI::WindowsAndMessaging::{EnumChildWindows, GetWindow, GW_CHILD};

    let handle = frame.window_handle().ok()?;
    let raw = handle.as_raw();
    let main_hwnd = match raw {
        RawWindowHandle::Win32(Win32WindowHandle { hwnd, .. }) => windows::Win32::Foundation::HWND(hwnd.get() as _),
        _ => return None,
    };
    // Try GetWindow(GW_CHILD) first (first child in Z-order).
    if let Ok(child) = unsafe { GetWindow(main_hwnd, GW_CHILD) } {
        if !child.0.is_null() {
            return Some(child);
        }
    }
    // Fallback: enumerate children (e.g. if WebView is created on another thread).
    let mut first_child = windows::Win32::Foundation::HWND::default();
    let _ = unsafe {
        EnumChildWindows(main_hwnd, Some(enum_child_first), LPARAM(&mut first_child as *mut _ as _))
    };
    if !first_child.0.is_null() {
        Some(first_child)
    } else {
        None
    }
}

#[cfg(windows)]
unsafe extern "system" fn enum_child_first(
    hwnd: windows::Win32::Foundation::HWND,
    lparam: windows::Win32::Foundation::LPARAM,
) -> windows::Win32::Foundation::BOOL {
    let ptr = lparam.0 as *mut windows::Win32::Foundation::HWND;
    if !ptr.is_null() && (*ptr).0.is_null() {
        *ptr = hwnd;
    }
    windows::Win32::Foundation::BOOL(0)
}

#[cfg(windows)]
fn set_webview_opacity(hwnd: windows::Win32::Foundation::HWND, alpha: f32) {
    use windows::Win32::Foundation::COLORREF;
    use windows::Win32::UI::WindowsAndMessaging::{
        GetWindowLongW, SetLayeredWindowAttributes, SetWindowLongW, LWA_ALPHA, GWL_EXSTYLE,
        WS_EX_LAYERED,
    };
    let ex = unsafe { GetWindowLongW(hwnd, GWL_EXSTYLE) };
    let new_ex = ex | (WS_EX_LAYERED.0 as i32);
    if new_ex != ex {
        let _ = unsafe { SetWindowLongW(hwnd, GWL_EXSTYLE, new_ex) };
    }
    let byte = (alpha.clamp(0.0, 1.0) * 255.0).round() as u8;
    let _ = unsafe { SetLayeredWindowAttributes(hwnd, COLORREF(0), byte, LWA_ALPHA) };
}

/// CLI args for elevated one-shot tasks (UAC). Handled before starting the GUI.
const ARG_ELEVATED_APPLY_REGISTRY: &str = "--elevated-apply-registry";
const ARG_ELEVATED_APPLY_HOSTS: &str = "--elevated-apply-hosts";
const ARG_ELEVATED_CHECK_DIRECTPLAY: &str = "--elevated-check-directplay";
const ARG_ELEVATED_INSTALL_DIRECTPLAY: &str = "--elevated-install-directplay";

fn main() -> Result<(), eframe::Error> {
    // If we were launched elevated to perform a one-shot fix, do it and exit (no GUI).
    let mut args = std::env::args();
    if let Some(arg) = args.nth(1) {
        if arg == ARG_ELEVATED_APPLY_REGISTRY {
            println!("[DEBUG] Elevated task: applying registry fix");
            match server_prereqs::apply_registry_fix() {
                Ok(()) => {
                    println!("[DEBUG] Registry fix applied successfully");
                    std::process::exit(0);
                }
                Err(e) => {
                    eprintln!("{}", e);
                    std::process::exit(1);
                }
            }
        }
        if arg == ARG_ELEVATED_APPLY_HOSTS {
            println!("[DEBUG] Elevated task: applying GameSpy hosts");
            match server_prereqs::apply_gamepy_hosts() {
                Ok(()) => {
                    println!("[DEBUG] GameSpy hosts applied successfully");
                    std::process::exit(0);
                }
                Err(e) => {
                    eprintln!("{}", e);
                    std::process::exit(1);
                }
            }
        }
        if arg == ARG_ELEVATED_CHECK_DIRECTPLAY {
            let path = args.next().unwrap_or_default();
            let emulate = args.next().as_deref() == Some("--emulate-no-directplay");
            let path = std::path::Path::new(&path);
            if emulate {
                println!("[DEBUG] DirectPlay check: emulating NOT installed (--emulate-no-directplay)");
                if let Err(e) = std::fs::write(path, "disabled") {
                    eprintln!("[DEBUG] Failed to write result file: {}", e);
                    std::process::exit(1);
                }
                std::process::exit(0);
            }
            println!("[DEBUG] DirectPlay check: running elevated detection, result path={}", path.display());
            match server_prereqs::run_check_directplay_and_write_result(path) {
                Ok(()) => std::process::exit(0),
                Err(e) => {
                    eprintln!("{}", e);
                    std::process::exit(1);
                }
            }
        }
        if arg == ARG_ELEVATED_INSTALL_DIRECTPLAY {
            println!("[DEBUG] Elevated task: enabling DirectPlay");
            match server_prereqs::enable_directplay() {
                Ok(()) => {
                    println!("[DEBUG] DirectPlay enabled successfully");
                    std::process::exit(0);
                }
                Err(e) => {
                    eprintln!("{}", e);
                    std::process::exit(1);
                }
            }
        }
    }

    println!("[DEBUG] Spectre v{} starting...", env!("CARGO_PKG_VERSION"));

    let banner_size = get_banner_size().unwrap_or((1024.0, 420.0));
    let splash_size = (banner_size.0 / 2.0, banner_size.1 / 2.0);
    println!(
        "[DEBUG] Splash window size: {}x{}",
        splash_size.0, splash_size.1
    );

    let mut viewport_builder = egui::ViewportBuilder::default()
        .with_inner_size([splash_size.0, splash_size.1])
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
    /// Error message when a card (e.g. Server Utility) fails to open.
    card_launch_error: Option<String>,
    /// When set, create an embedded WebView for this card on the next frame (so we have access to the window).
    #[cfg(windows)]
    pending_webview_card: Option<String>,
    #[cfg(windows)]
    /// WebView2 child window for web cards. Embedded in the same window; no separate exe.
    webview: Option<wry::WebView>,
    /// Fade opacity for WebView when modal opens/closes (0 = transparent, 1 = opaque).
    #[cfg(windows)]
    webview_fade_alpha: f32,
    /// Receives save result from IPC handler so we can run evaluate_script to update the page.
    #[cfg(windows)]
    ipc_save_rx: Option<mpsc::Receiver<Result<(), String>>>,
    /// When true, close WebView and reopen server_utility (refresh).
    #[cfg(windows)]
    pending_webview_refresh: bool,
    splash_screen: Option<SplashScreen>,
    window_centered: bool,
    center_attempts: u32,
    card_icon: Option<TextureHandle>,
    home_icon: Option<TextureHandle>,
    settings_icon: Option<TextureHandle>,
    info_icon: Option<TextureHandle>,
    refresh_icon: Option<TextureHandle>,
    #[cfg(debug_assertions)]
    console_icon: Option<TextureHandle>,
}

impl SpectreApp {
    fn new(cc: &eframe::CreationContext<'_>) -> Self {
        println!("[DEBUG] Creating SpectreApp instance...");
        let splash = SplashScreen::new(&cc.egui_ctx);
        println!("[DEBUG] Splash screen initialized");

        let config = Config::load();
        println!("[DEBUG] Configuration loaded");

        Self::apply_theme(&cc.egui_ctx);

        let card_icon = load_svg_icon(&cc.egui_ctx, "server_launcher");
        let home_icon = load_svg_icon(&cc.egui_ctx, "home");
        let settings_icon = load_svg_icon(&cc.egui_ctx, "settings");
        let info_icon = load_svg_icon(&cc.egui_ctx, "info");
        let refresh_icon = load_svg_icon(&cc.egui_ctx, "refresh");
        #[cfg(debug_assertions)]
        let console_icon = load_svg_icon(&cc.egui_ctx, "console");

        Self {
            version: VERSION.to_string(),
            config,
            current_module: None,
            show_about: false,
            show_options: false,
            card_launch_error: None,
            #[cfg(windows)]
            pending_webview_card: None,
            #[cfg(windows)]
            webview: None,
            #[cfg(windows)]
            webview_fade_alpha: 1.0,
            #[cfg(windows)]
            ipc_save_rx: None,
            #[cfg(windows)]
            pending_webview_refresh: false,
            splash_screen: Some(splash),
            window_centered: false,
            center_attempts: 0,
            card_icon,
            home_icon,
            settings_icon,
            info_icon,
            refresh_icon,
            #[cfg(debug_assertions)]
            console_icon,
        }
    }

    fn apply_theme(ctx: &egui::Context) {
        ctx.set_visuals(egui::Visuals::dark());
    }

    fn validate_server_path(which: u8, path: &str) -> bool {
        let path = path.trim();
        if path.is_empty() {
            return true;
        }
        let p = Path::new(path);
        let name = match p.file_name().and_then(|n| n.to_str()) {
            Some(n) => n,
            None => return false,
        };
        let expected = match which {
            0 => "HD2DS.exe",
            1 => "HD2DS_SabreSquadron.exe",
            _ => "mpmaplist.txt",
        };
        name.eq_ignore_ascii_case(expected) && p.exists()
    }

    fn server_path_expected_filename(which: u8) -> &'static str {
        match which {
            0 => "HD2DS.exe",
            1 => "HD2DS_SabreSquadron.exe",
            _ => "mpmaplist.txt",
        }
    }

    /// App-wide action bar: Home, Settings, Info. Thin (narrow height). Used above WebView and above modules.
    /// When `webview_active` is true, tooltips are drawn inline in the bar so they are not covered by the WebView (native window z-order).
    fn show_action_bar(&mut self, ui: &mut egui::Ui, webview_active: bool) {
        const ACTION_BAR_HEIGHT: f32 = 32.0;
        const ACTION_BAR_LEFT_MARGIN: f32 = 6.0;
        const ACTION_BAR_RIGHT_MARGIN: f32 = 6.0;
        const BTN_W: f32 = 32.0;
        const BTN_H: f32 = 24.0;
        const ICON_SZ: f32 = 14.0;
        const BTN_GAP: f32 = 8.0;

        let (bar_rect, _) = ui.allocate_exact_size(
            egui::vec2(ui.available_width(), ACTION_BAR_HEIGHT),
            egui::Sense::hover(),
        );
        ui.allocate_ui_at_rect(bar_rect, |ui| {
            ui.with_layout(
                egui::Layout::left_to_right(egui::Align::Center).with_main_justify(false),
                |ui| {
                    ui.add_space(ACTION_BAR_LEFT_MARGIN);
                    ui.spacing_mut().item_spacing = egui::vec2(BTN_GAP, 0.0);
                    // Home
                    let home_r = ui.allocate_response(egui::Vec2::new(BTN_W, BTN_H), egui::Sense::click());
                    let fill = if home_r.hovered() { ui.visuals().widgets.hovered.bg_fill } else { ui.visuals().widgets.inactive.bg_fill };
                    ui.painter().rect_filled(home_r.rect, 4.0, fill);
                    if let Some(ref t) = self.home_icon {
                        let r = egui::Rect::from_center_size(home_r.rect.center(), egui::vec2(ICON_SZ, ICON_SZ));
                        ui.painter().image(t.id(), r, egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)), ui.visuals().text_color());
                    } else {
                        let galley = ui.painter().layout_no_wrap("⌂".to_string(), egui::FontId::new(14.0, egui::FontFamily::Proportional), ui.visuals().text_color());
                        ui.painter().galley(home_r.rect.center() - galley.size() / 2.0, galley, ui.visuals().text_color());
                    }
                    if home_r.clicked() { ui.ctx().data_mut(|d| d.insert_temp(egui::Id::new("spectre_go_home"), ())); }
                    if home_r.hovered() {
                        ui.ctx().output_mut(|o| o.cursor_icon = egui::CursorIcon::PointingHand);
                        if webview_active {
                            ui.label(egui::RichText::new("Return to main screen").size(12.0).color(ui.visuals().weak_text_color()));
                        } else {
                            egui::show_tooltip(ui.ctx(), egui::Id::new("action_bar_home"), |ui| ui.label("Return to main screen"));
                        }
                    }
                    // Settings
                    let set_r = ui.allocate_response(egui::Vec2::new(BTN_W, BTN_H), egui::Sense::click());
                    let fill = if set_r.hovered() { ui.visuals().widgets.hovered.bg_fill } else { ui.visuals().widgets.inactive.bg_fill };
                    ui.painter().rect_filled(set_r.rect, 4.0, fill);
                    if let Some(ref t) = self.settings_icon {
                        let r = egui::Rect::from_center_size(set_r.rect.center(), egui::vec2(ICON_SZ, ICON_SZ));
                        ui.painter().image(t.id(), r, egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)), ui.visuals().text_color());
                    } else {
                        let galley = ui.painter().layout_no_wrap("⚙".to_string(), egui::FontId::new(14.0, egui::FontFamily::Proportional), ui.visuals().text_color());
                        ui.painter().galley(set_r.rect.center() - galley.size() / 2.0, galley, ui.visuals().text_color());
                    }
                    if set_r.clicked() { self.show_options = true; }
                    if set_r.hovered() {
                        ui.ctx().output_mut(|o| o.cursor_icon = egui::CursorIcon::PointingHand);
                        if webview_active {
                            ui.label(egui::RichText::new("Settings").size(12.0).color(ui.visuals().weak_text_color()));
                        } else {
                            egui::show_tooltip(ui.ctx(), egui::Id::new("action_bar_settings"), |ui| ui.label("Settings"));
                        }
                    }
                    // Info (About)
                    let info_r = ui.allocate_response(egui::Vec2::new(BTN_W, BTN_H), egui::Sense::click());
                    let fill = if info_r.hovered() { ui.visuals().widgets.hovered.bg_fill } else { ui.visuals().widgets.inactive.bg_fill };
                    ui.painter().rect_filled(info_r.rect, 4.0, fill);
                    if let Some(ref t) = self.info_icon {
                        let r = egui::Rect::from_center_size(info_r.rect.center(), egui::vec2(ICON_SZ, ICON_SZ));
                        ui.painter().image(t.id(), r, egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)), ui.visuals().text_color());
                    } else {
                        let galley = ui.painter().layout_no_wrap("ℹ".to_string(), egui::FontId::new(14.0, egui::FontFamily::Proportional), ui.visuals().text_color());
                        ui.painter().galley(info_r.rect.center() - galley.size() / 2.0, galley, ui.visuals().text_color());
                    }
                    if info_r.clicked() { self.show_about = true; }
                    if info_r.hovered() {
                        ui.ctx().output_mut(|o| o.cursor_icon = egui::CursorIcon::PointingHand);
                        if webview_active {
                            ui.label(egui::RichText::new("About").size(12.0).color(ui.visuals().weak_text_color()));
                        } else {
                            egui::show_tooltip(ui.ctx(), egui::Id::new("action_bar_info"), |ui| ui.label("About"));
                        }
                    }
                    // Spacer to push right-side buttons to the end (reserve space for Refresh + optional Dev Tools)
                    let rest = ui.available_width();
                    let right_w = if webview_active {
                        if cfg!(debug_assertions) { BTN_W * 2.0 + BTN_GAP } else { BTN_W }
                    } else {
                        0.0
                    };
                    if rest > right_w {
                        ui.add_space(rest - right_w);
                    }
                    // Right side: Dev Tools (left) then Refresh (rightmost), with right margin
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center).with_main_justify(false), |ui| {
                        ui.add_space(ACTION_BAR_RIGHT_MARGIN);
                        ui.spacing_mut().item_spacing = egui::vec2(BTN_GAP, 0.0);
                        // Refresh (when webview active) — rightmost
                        if webview_active {
                            let ref_r = ui.allocate_response(egui::Vec2::new(BTN_W, BTN_H), egui::Sense::click());
                            let fill = if ref_r.hovered() { ui.visuals().widgets.hovered.bg_fill } else { ui.visuals().widgets.inactive.bg_fill };
                            ui.painter().rect_filled(ref_r.rect, 4.0, fill);
                            if let Some(ref t) = self.refresh_icon {
                                let r = egui::Rect::from_center_size(ref_r.rect.center(), egui::vec2(ICON_SZ, ICON_SZ));
                                ui.painter().image(t.id(), r, egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)), ui.visuals().text_color());
                            } else {
                                let galley = ui.painter().layout_no_wrap("↻".to_string(), egui::FontId::new(14.0, egui::FontFamily::Proportional), ui.visuals().text_color());
                                ui.painter().galley(ref_r.rect.center() - galley.size() / 2.0, galley, ui.visuals().text_color());
                            }
                            if ref_r.clicked() {
                                #[cfg(windows)]
                                {
                                    self.pending_webview_refresh = true;
                                }
                            }
                            if ref_r.hovered() {
                                ui.ctx().output_mut(|o| o.cursor_icon = egui::CursorIcon::PointingHand);
                                ui.label(egui::RichText::new("Refresh").size(12.0).color(ui.visuals().weak_text_color()));
                            }
                        }
                        // Dev Tools (debug only) — left of Refresh
                        #[cfg(debug_assertions)]
                        if webview_active {
                            let dev_r = ui.allocate_response(egui::Vec2::new(BTN_W, BTN_H), egui::Sense::click());
                            let fill = if dev_r.hovered() { ui.visuals().widgets.hovered.bg_fill } else { ui.visuals().widgets.inactive.bg_fill };
                            ui.painter().rect_filled(dev_r.rect, 4.0, fill);
                            if let Some(ref t) = self.console_icon {
                                let r = egui::Rect::from_center_size(dev_r.rect.center(), egui::vec2(ICON_SZ, ICON_SZ));
                                ui.painter().image(t.id(), r, egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)), ui.visuals().text_color());
                            } else {
                                let galley = ui.painter().layout_no_wrap(">_".to_string(), egui::FontId::new(14.0, egui::FontFamily::Proportional), ui.visuals().text_color());
                                ui.painter().galley(dev_r.rect.center() - galley.size() / 2.0, galley, ui.visuals().text_color());
                            }
                            if dev_r.clicked() {
                                if let Some(ref wv) = self.webview {
                                    wv.open_devtools();
                                }
                            }
                            if dev_r.hovered() {
                                ui.ctx().output_mut(|o| o.cursor_icon = egui::CursorIcon::PointingHand);
                                ui.label(egui::RichText::new("Open DevTools").size(12.0).color(ui.visuals().weak_text_color()));
                            }
                        }
                    });
                },
            );
            let rect = ui.available_rect_before_wrap();
            if rect.height() >= 1.0 {
                let line_y = rect.bottom() - 1.0;
                ui.painter().line_segment(
                    [egui::pos2(rect.left(), line_y), egui::pos2(rect.right(), line_y)],
                    egui::Stroke::new(2.0, egui::Color32::from_gray(130)),
                );
            }
            ui.allocate_rect(rect, egui::Sense::hover());
        });
    }

    fn show_landing_page(&mut self, ui: &mut egui::Ui) {
        let available_width = ui.available_width();
        let side_padding = 20.0;
        let min_card_width = 240.0;
        let max_card_width = 280.0;
        let gap = 8.0;
        let usable_width = (available_width - (side_padding * 2.0)).max(min_card_width).min(available_width);

        // Pre-compute card layout to keep header and grid aligned within the same content area
        // Tuple layout: (title, description, category, module_index, is_ready)
        let cards: Vec<(&str, &str, &str, usize, bool)> = vec![
            ("Server Utility", "Launch and manage HD2 game servers", "Tool", 0, true),
            ("DTA Unpacker", "Extract and unpack DTA archive files", "Tool", 1, false),
            ("Inventory Editor", "Edit player inventory files", "Editor", 2, false),
            ("Items Editor", "Edit item values and create items", "Editor", 3, false),
            ("MP Maplist Editor", "Edit multiplayer maplist files", "Editor", 4, false),
            ("Gamedata Editor", "Edit gamedata00.gdt and gamedata01.gdt", "Editor", 5, false),
        ];

        let mut cards_per_row = ((usable_width + gap) / (max_card_width + gap)).floor() as usize;
        cards_per_row = cards_per_row.max(1).min(4);

        let total_gaps = gap * (cards_per_row.saturating_sub(1)) as f32;
        let card_width =
            ((usable_width - total_gaps) / cards_per_row as f32).max(min_card_width).min(max_card_width);

        let card_height = 160.0;
        let margin = 4.0;

        // Content block width (centered in panel by layout)
        let content_width = (card_width * cards_per_row as f32)
            + (gap * (cards_per_row.saturating_sub(1)) as f32);

        egui::ScrollArea::vertical()
            .auto_shrink([false; 2])
            .show(ui, |ui| {
                ui.set_width(available_width);
                // Center the whole content block (header + grid + version) in the panel
                ui.with_layout(
                    egui::Layout::top_down(egui::Align::Center),
                    |ui| {
                        ui.set_width(available_width);
                        ui.allocate_ui(egui::vec2(content_width, 0.0), |ui| {
                            ui.set_width(content_width);
                            ui.set_min_width(content_width);

                            ui.add_space(40.0);

                            // Heading row: title + subtitle centered in full content width, settings/about on the right
                            let button_group_width = 36.0 + 12.0 + 36.0;
                            ui.horizontal(|ui| {
                                ui.set_width(content_width);
                                ui.spacing_mut().item_spacing.x = 0.0; // no extra gap so centering math is exact
                                let strong = ui.visuals().strong_text_color();
                                let weak = ui.visuals().weak_text_color();
                                let font_56 = egui::FontId::new(56.0, egui::FontFamily::Proportional);
                                let font_18 = egui::FontId::new(18.0, egui::FontFamily::Proportional);
                                let g1 = ui.painter().layout_no_wrap("Spectre".into(), font_56, strong);
                                let g2 = ui.painter().layout_no_wrap("Hidden & Dangerous 2 Toolkit".into(), font_18, weak);
                                let title_w = g1.size().x.max(g2.size().x);
                                let title_h = g1.size().y + 8.0 + g2.size().y;
                                // Center title in full content width (title center at content_width/2)
                                let left_space = (content_width / 2.0 - title_w / 2.0).max(0.0);
                                let g1_w = g1.size().x;
                                // Align right edge of button group with right edge of "Spectre" title (content boundary)
                                let right_space = (g1_w / 2.0 - title_w / 2.0 - button_group_width).max(8.0);
                                let g1_h = g1.size().y;
                                let g2_w = g2.size().x;
                                ui.add_space(left_space);
                                ui.allocate_ui(
                                    egui::vec2(title_w, title_h),
                                    |ui| {
                                        // Paint using same galley as measurement so centering is exact
                                        let pos = ui.cursor().min;
                                        let x1 = pos.x + (title_w - g1_w) / 2.0;
                                        let x2 = pos.x + (title_w - g2_w) / 2.0;
                                        ui.painter().galley(egui::pos2(x1, pos.y), g1, strong);
                                        ui.painter().galley(egui::pos2(x2, pos.y + g1_h + 8.0), g2, weak);
                                    },
                                );
                                ui.add_space(right_space);
                                // Vertically center buttons with the title block (align with "Spectre" midline)
                                let button_h = 28.0;
                                let space_above_buttons = (title_h * 0.5 - button_h * 0.5).max(0.0);
                                ui.vertical(|ui| {
                                    ui.add_space(space_above_buttons);
                                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Min), |ui| {
                                        ui.spacing_mut().item_spacing = egui::vec2(12.0, 0.0);
                                    let about_response = ui.allocate_response(
                                        egui::Vec2::new(36.0, 28.0),
                                        egui::Sense::click(),
                                    );
                                    let is_hovered = about_response.hovered();
                                    let fill = if is_hovered {
                                        ui.visuals().widgets.hovered.bg_fill
                                    } else {
                                        ui.visuals().widgets.inactive.bg_fill
                                    };
                                    ui.painter().rect_filled(about_response.rect, 4.0, fill);
                                    ui.painter().rect_stroke(
                                        about_response.rect,
                                        4.0,
                                        egui::Stroke::new(1.0, ui.visuals().widgets.inactive.bg_stroke.color),
                                    );
                                    let font = egui::FontId::new(18.0, egui::FontFamily::Proportional);
                                    let galley = ui.painter().layout_no_wrap(
                                        "ℹ".to_string(),
                                        font.clone(),
                                        ui.visuals().text_color(),
                                    );
                                    let text_size = galley.size();
                                    let button_center = about_response.rect.center();
                                    let icon_pos = egui::pos2(
                                        button_center.x - text_size.x * 0.5,
                                        button_center.y - text_size.y * 0.5 - font.size * 0.15,
                                    );
                                    ui.painter().galley(icon_pos, galley, ui.visuals().text_color());
                                    if about_response.clicked() {
                                        self.show_about = true;
                                    }
                                    if about_response.hovered() {
                                        ui.ctx().output_mut(|o| o.cursor_icon = egui::CursorIcon::PointingHand);
                                        egui::show_tooltip(ui.ctx(), egui::Id::new("about_btn"), |ui| ui.label("About"));
                                    }
                                    let settings_response = ui.allocate_response(
                                        egui::Vec2::new(36.0, 28.0),
                                        egui::Sense::click(),
                                    );
                                    let is_hovered = settings_response.hovered();
                                    let fill = if is_hovered {
                                        ui.visuals().widgets.hovered.bg_fill
                                    } else {
                                        ui.visuals().widgets.inactive.bg_fill
                                    };
                                    ui.painter().rect_filled(settings_response.rect.expand(0.0), 4.0, fill);
                                    ui.painter().rect_stroke(
                                        settings_response.rect.expand(0.0),
                                        4.0,
                                        egui::Stroke::new(1.0, ui.visuals().widgets.inactive.bg_stroke.color),
                                    );
                                    let font = egui::FontId::new(16.0, egui::FontFamily::Proportional);
                                    let galley = ui.painter().layout_no_wrap(
                                        "⚙".to_string(),
                                        font.clone(),
                                        ui.visuals().text_color(),
                                    );
                                    let text_size = galley.size();
                                    let button_center = settings_response.rect.center();
                                    let icon_pos = egui::pos2(
                                        button_center.x - text_size.x * 0.5,
                                        button_center.y - text_size.y * 0.5,
                                    );
                                    ui.painter().galley(icon_pos, galley, ui.visuals().text_color());
                                    if settings_response.clicked() {
                                        self.show_options = true;
                                    }
                                    if settings_response.hovered() {
                                        ui.ctx().output_mut(|o| o.cursor_icon = egui::CursorIcon::PointingHand);
                                        egui::show_tooltip(ui.ctx(), egui::Id::new("settings_btn"), |ui| ui.label("Settings"));
                                    }
                                    });
                                });
                            });

                            ui.add_space(80.0);

                            // Card grid: same left edge for every row so columns align (e.g. 4+2 layout)
                            let mut row_start = 0;
                            while row_start < cards.len() {
                                let row_end = (row_start + cards_per_row).min(cards.len());
                                let row_cards = &cards[row_start..row_end];

                                ui.horizontal(|ui| {
                                    ui.set_width(content_width);
                                    for (i, (title, desc, cat, idx, is_ready)) in row_cards.iter().enumerate() {
                                        if i > 0 {
                                            ui.add_space(gap);
                                        }
                                        let clicked = ui.allocate_ui_with_layout(
                                            egui::Vec2::new(card_width, card_height),
                                            egui::Layout::top_down(egui::Align::LEFT),
                                            |ui| {
                                                Self::tool_card(
                                                    ui,
                                                    card_width,
                                                    title,
                                                    desc,
                                                    cat,
                                                    self.card_icon.as_ref(),
                                                    *is_ready,
                                                )
                                            },
                                        )
                                        .inner;
                                        if clicked && *is_ready {
                                            match idx {
                                                0 => {
                                                    // Server Utility: if wizard not completed, show egui wizard first; else embed WebView
                                                    if !self.config.server_utility_wizard_completed {
                                                        self.current_module = Some(Box::new(ServerLauncher::default()));
                                                    } else {
                                                        #[cfg(windows)]
                                                        {
                                                            self.pending_webview_card = Some("server_utility".to_string());
                                                        }
                                                        #[cfg(not(windows))]
                                                        {
                                                            self.current_module = Some(Box::new(ServerLauncher::default()));
                                                        }
                                                    }
                                                }
                                                1 => self.current_module = Some(Box::new(DtaUnpacker::default())),
                                                2 => self.current_module = Some(Box::new(InventoryEditor::default())),
                                                3 => self.current_module = Some(Box::new(ItemsEditor::default())),
                                                4 => self.current_module = Some(Box::new(MpmaplistEditor::default())),
                                                5 => self.current_module = Some(Box::new(GamedataEditor::default())),
                                                _ => {}
                                            }
                                        }
                                    }
                                });

                                if row_start + cards_per_row < cards.len() {
                                    ui.add_space(margin * 2.0);
                                }
                                row_start = row_end;
                            }

                            ui.add_space(20.0);
                            ui.with_layout(egui::Layout::top_down(egui::Align::Center), |ui| {
                                ui.set_width(content_width);
                                ui.label(
                                    egui::RichText::new(format!("Version {}", self.version))
                                        .family(egui::FontFamily::Monospace)
                                        .size(12.0)
                                        .color(ui.visuals().weak_text_color()),
                                );
                            });
                        });
                    },
                );
            });
    }
    
    
    fn tool_card(
        ui: &mut egui::Ui,
        card_width: f32,
        title: &str,
        description: &str,
        category: &str,
        icon: Option<&TextureHandle>,
        is_ready: bool,
    ) -> bool {
        let card_height = 160.0;

        let sense = if is_ready {
            egui::Sense::click()
        } else {
            egui::Sense::hover()
        };

        let (rect, response) =
            ui.allocate_exact_size(egui::Vec2::new(card_width, card_height), sense);

        let pointer_pos = ui.ctx().pointer_latest_pos();
        let is_hovered = if is_ready {
            response.hovered()
                || response.contains_pointer()
                || pointer_pos.map_or(false, |pos| rect.contains(pos))
        } else {
            // Disabled cards don't react to hover state for visuals
            false
        };

        let mut fill = ui.visuals().panel_fill;
        let mut stroke = ui.visuals().widgets.noninteractive.bg_stroke;

        if is_ready {
            if is_hovered {
                fill = ui.visuals().widgets.hovered.bg_fill;
                stroke = egui::Stroke::new(2.0, ui.visuals().widgets.hovered.bg_stroke.color);
            }
        } else {
            // Greyed-out background for cards that aren't ready yet
            fill = ui.visuals().extreme_bg_color;
            stroke = egui::Stroke::new(1.5, ui.visuals().widgets.inactive.bg_stroke.color);
        }

        ui.painter().rect_filled(rect, 8.0, fill);
        ui.painter().rect_stroke(rect, 8.0, stroke);

        let hover_state = is_hovered;

        let inner_rect = rect.shrink(12.0);
        let inner = ui.allocate_ui_at_rect(inner_rect, |ui| {
            ui.set_width(card_width - 24.0);
            ui.set_height(card_height - 24.0);

            ui.vertical(|ui| {
                ui.horizontal(|ui| {
                    ui.vertical(|ui| {
                        ui.set_width(card_width - 60.0);
                        
                        ui.horizontal(|ui| {
                            let category_color = if is_ready {
                                if category == "Tool" {
                                    egui::Color32::from_rgb(100, 150, 255)
                                } else {
                                    egui::Color32::from_rgb(150, 100, 255)
                                }
                            } else {
                                ui.visuals().weak_text_color()
                            };

                            ui.label(
                                egui::RichText::new(category)
                                    .size(10.0)
                                    .color(category_color)
                                    .strong(),
                            );
                        });
                        
                        ui.add_space(4.0);
                        
                        let title_color = if is_ready {
                            ui.visuals().strong_text_color()
                        } else {
                            ui.visuals().weak_text_color()
                        };

                        ui.label(
                            egui::RichText::new(title)
                                .size(16.0)
                                .strong()
                                .color(title_color),
                        );
                        
                        ui.add_space(8.0);
                        
                        let desc_color = if is_ready {
                            ui.visuals().weak_text_color()
                        } else {
                            ui.visuals().weak_text_color()
                        };

                        ui.label(
                            egui::RichText::new(description)
                                .size(12.0)
                                .color(desc_color),
                        );
                    });
                    
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Min), |ui| {
                        if let Some(icon_texture) = icon {
                            let icon_size = 14.0;
                            let available = ui.available_rect_before_wrap();
                            let right_x = available.right();
                            let top_y = available.top();
                            
                            let icon_rect = egui::Rect::from_min_max(
                                egui::pos2(right_x - icon_size - 4.0, top_y + 4.0),
                                egui::pos2(right_x - 4.0, top_y + icon_size + 4.0),
                            );
                            
                            // Desaturate icon slightly when not ready
                            let tint = if is_ready {
                                egui::Color32::WHITE
                            } else {
                                egui::Color32::from_rgba_unmultiplied(200, 200, 200, 220)
                            };

                            ui.painter().image(
                                icon_texture.id(),
                                icon_rect,
                                egui::Rect::from_min_max(
                                    egui::pos2(0.0, 0.0),
                                    egui::pos2(1.0, 1.0),
                                ),
                                tint,
                            );
                        } else {
                            let available = ui.available_rect_before_wrap();
                            let right_x = available.right() - 6.0;
                            let bottom_y = available.bottom() - 6.0;
                            
                            let color = if is_ready {
                                if hover_state {
                                    ui.visuals().strong_text_color()
                                } else {
                                    ui.visuals().weak_text_color()
                                }
                            } else {
                                ui.visuals().weak_text_color()
                            };
                            
                            let stroke_width = if hover_state { 2.0 } else { 1.5 };
                            let box_size = 6.0;
                            let box_rect = egui::Rect::from_min_max(
                                egui::pos2(right_x - box_size - 8.0, bottom_y - box_size),
                                egui::pos2(right_x - 8.0, bottom_y),
                            );
                            
                            ui.painter().rect_stroke(
                                box_rect,
                                1.0,
                                egui::Stroke::new(stroke_width, color),
                            );
                            
                            let arrow_base = egui::pos2(box_rect.right(), box_rect.top());
                            let arrow_tip = egui::pos2(right_x, bottom_y - box_size - 6.0);
                            
                            ui.painter().line_segment(
                                [arrow_base, arrow_tip],
                                egui::Stroke::new(stroke_width, color),
                            );
                            
                            let arrowhead_size = 4.0;
                            let dx = arrow_tip.x - arrow_base.x;
                            let dy = arrow_tip.y - arrow_base.y;
                            let angle = dy.atan2(dx);
                            
                            let arrowhead_left = arrow_tip + egui::vec2(
                                -arrowhead_size * (angle - std::f32::consts::PI * 0.4).cos(),
                                -arrowhead_size * (angle - std::f32::consts::PI * 0.4).sin(),
                            );
                            let arrowhead_right = arrow_tip + egui::vec2(
                                -arrowhead_size * (angle + std::f32::consts::PI * 0.4).cos(),
                                -arrowhead_size * (angle + std::f32::consts::PI * 0.4).sin(),
                            );
                            
                            ui.painter().line_segment(
                                [arrow_tip, arrowhead_left],
                                egui::Stroke::new(stroke_width, color),
                            );
                            ui.painter().line_segment(
                                [arrow_tip, arrowhead_right],
                                egui::Stroke::new(stroke_width, color),
                            );
                        }
                        
                        ui.add_space(12.0);
                    });
                });
            });
        });

        // Register click on the whole card content area so clicks on text count (on top in hit-test)
        let content_clicked = if is_ready {
            let id = ui.id().with("card_click").with(title);
            ui.interact(inner_rect, id, egui::Sense::click()).clicked()
        } else {
            false
        };

        // For cards that aren't ready yet, overlay a soft diagonal line pattern (clipped to card)
        if !is_ready {
            let stripe_color =
                egui::Color32::from_rgba_unmultiplied(255, 255, 255, 22);
            let stripe_spacing = 12.0;
            let diag = rect.height();
            let mut x = rect.left() - diag;

            while x < rect.right() + diag {
                let x1_clip = x.max(rect.left());
                let x2_clip = (x + diag).min(rect.right());
                if x1_clip < x2_clip {
                    let t1 = (x1_clip - x) / diag;
                    let t2 = (x2_clip - x) / diag;
                    let y1 = rect.bottom() + (rect.top() - rect.bottom()) * t1;
                    let y2 = rect.bottom() + (rect.top() - rect.bottom()) * t2;
                    let p1 = egui::pos2(x1_clip, y1);
                    let p2 = egui::pos2(x2_clip, y2);
                    ui.painter().line_segment(
                        [p1, p2],
                        egui::Stroke::new(1.0, stripe_color),
                    );
                }
                x += stripe_spacing;
            }
        }

        // Clicks on the card: outer area, inner content, or explicit content-area interact
        let clicked = response.clicked() || inner.response.clicked() || content_clicked;

        is_ready && clicked
    }
}

impl eframe::App for SpectreApp {
    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        // When wizard is finished, switch to web view in the same frame (never show old egui launcher UI)
        if ctx.data_mut(|d| d.get_temp::<()>(egui::Id::new("spectre_open_web_after_wizard")).is_some()) {
            ctx.data_mut(|d| d.remove::<()>(egui::Id::new("spectre_open_web_after_wizard")));
            self.current_module = None;
            #[cfg(windows)]
            {
                self.pending_webview_card = Some("server_utility".to_string());
            }
        }

        #[cfg(windows)]
        {
            if self.pending_webview_refresh && self.webview.is_some() {
                self.webview = None;
                self.pending_webview_card = Some("server_utility".to_string());
                self.ipc_save_rx = None;
                self.pending_webview_refresh = false;
            }
        }
        #[cfg(windows)]
        if let Some(card_name) = self.pending_webview_card.take() {
            let initial_json = if card_name == "server_utility" {
                let config_path = server_utility_config_path();
                let path_exists = config_path.exists();
                if path_exists {
                    println!("[IPC] Server utility load: path={}", config_path.display());
                } else {
                    println!("[IPC] Server utility: config file not found at {} (using defaults)", config_path.display());
                }
                let mut data = spectre_core::server::ServerLauncherData::load_from_file(&config_path)
                    .unwrap_or_else(|e| {
                        println!("[IPC] Load failed (using defaults): {}", e);
                        spectre_core::server::ServerLauncherData::default()
                    });
                ensure_server_utility_has_defaults(&mut data);
                // When config was missing or empty, use app Settings mpmaplist path so maplist still loads
                if data.server_manager.mpmaplist_path.is_empty() && !self.config.server_mpmaplist_path.is_empty() {
                    data.server_manager.mpmaplist_path = self.config.server_mpmaplist_path.clone();
                }
                let available_maps = if data.server_manager.mpmaplist_path.is_empty() {
                    std::collections::HashMap::new()
                } else {
                    let path = std::path::Path::new(&data.server_manager.mpmaplist_path);
                    let resolved = spectre_core::mpmaplist::resolve_mpmaplist_path(path);
                    let maps = spectre_core::mpmaplist::load_from_path(path);
                    let total: usize = maps.values().map(|v| v.len()).sum();
                    if total > 0 {
                        for (style, list) in &maps {
                            println!("[IPC] mpmaplist style {}: {} maps", style, list.len());
                        }
                        println!("[IPC] mpmaplist total: {} maps from {}", total, resolved.display());
                    } else if !data.server_manager.mpmaplist_path.is_empty() {
                        println!("[IPC] mpmaplist: no maps parsed from {} (check path and file format)", resolved.display());
                    }
                    maps
                };
                match serde_json::to_value(&data) {
                    Ok(mut value) => {
                        value["availableMapsByStyle"] = serde_json::to_value(&available_maps).unwrap_or(serde_json::Value::Object(serde_json::Map::new()));
                        match serde_json::to_string(&value) {
                            Ok(json) => {
                                let source = if path_exists { "from file" } else { "defaults" };
                                println!("[IPC] Initial state: {} servers, {} bytes ({})", data.servers.len(), json.len(), source);
                                Some(json)
                            }
                            Err(e) => {
                                println!("[IPC] Serialize initial state failed: {}", e);
                                None
                            }
                        }
                    }
                    Err(e) => {
                        println!("[IPC] Serialize initial state failed: {}", e);
                        None
                    }
                }
            } else {
                None
            };
            let html_result = spectre_web::embedded_card_html(
                &card_name,
                initial_json.as_deref(),
            );
            if let Ok(html) = html_result {
                let scale = ctx
                    .input(|i| i.viewport().native_pixels_per_point)
                    .unwrap_or(1.0);
                let screen = ctx.screen_rect();
                const ACTION_BAR_HEIGHT: f32 = 32.0;
                let bounds = wry::Rect {
                    x: 0,
                    y: (ACTION_BAR_HEIGHT * scale) as i32,
                    width: (screen.width() * scale) as u32,
                    height: ((screen.height() - ACTION_BAR_HEIGHT) * scale).max(1.0) as u32,
                };
                let config_path = server_utility_config_path();
                let (ipc_tx, ipc_rx) = mpsc::channel();
                let builder = wry::WebViewBuilder::new_as_child(&*frame)
                    .with_bounds(bounds)
                    .with_ipc_handler({
                        let config_path = config_path.clone();
                        let ipc_tx = ipc_tx.clone();
                        move |request: http::Request<String>| {
                            let body = request.body();
                            println!("[IPC] postMessage received, body_len={}", body.len());
                            match serde_json::from_str::<IpcSaveMessage>(body) {
                                Ok(msg) if msg.action == "save" => {
                                    println!("[IPC] Save: {} servers", msg.servers.len());
                                    let mut data = spectre_core::server::ServerLauncherData::load_from_file(&config_path)
                                        .unwrap_or_else(|_| spectre_core::server::ServerLauncherData::default());
                                    data.servers = msg.servers;
                                    if let Some(parent) = config_path.parent() {
                                        let _ = std::fs::create_dir_all(parent);
                                    }
                                    let result = data.save_to_file(&config_path).map_err(|e| e.to_string());
                                    if result.is_ok() {
                                        println!("[IPC] Save OK -> {}", config_path.display());
                                    } else {
                                        println!("[IPC] Save failed: {:?}", result);
                                    }
                                    let _ = ipc_tx.send(result);
                                }
                                Ok(_) => {}
                                Err(e) => {
                                    println!("[IPC] Parse postMessage failed: {}", e);
                                    let _ = ipc_tx.send(Err(e.to_string()));
                                }
                            }
                        }
                    })
                    .with_devtools({
                        // Only enable DevTools (F12) in debug builds; disabled in release.
                        cfg!(debug_assertions)
                    })
                    .with_html(&html);
                if let Ok(wv) = builder.build() {
                    self.webview = Some(wv);
                    self.webview_fade_alpha = 1.0;
                    self.ipc_save_rx = Some(ipc_rx);
                } else {
                    self.card_launch_error = Some("Failed to create WebView.".to_string());
                }
            } else {
                self.card_launch_error = Some("Card not found.".to_string());
            }
        }

        #[cfg(windows)]
        if let Some(ref wv) = self.webview {
            let scale = ctx
                .input(|i| i.viewport().native_pixels_per_point)
                .unwrap_or(1.0);
            let screen = ctx.screen_rect();
            const ACTION_BAR_HEIGHT: f32 = 32.0;
            let bounds = wry::Rect {
                x: 0,
                y: (ACTION_BAR_HEIGHT * scale) as i32,
                width: (screen.width() * scale) as u32,
                height: ((screen.height() - ACTION_BAR_HEIGHT) * scale).max(1.0) as u32,
            };
            let _ = wv.set_bounds(bounds);
        }

        #[cfg(windows)]
        if let Some(ref rx) = self.ipc_save_rx {
            if let Ok(result) = rx.try_recv() {
                let status_msg = result.as_ref().map_or_else(|e| format!("Save failed: {}", e), |()| "Saved OK".to_string());
                let script = format!(
                    "window.__spectreIpcStatus && window.__spectreIpcStatus({});",
                    serde_json::to_string(&status_msg).unwrap_or_else(|_| "window.__spectreIpcStatus('Saved OK')".to_string())
                );
                if let Some(ref wv) = self.webview {
                    if let Err(e) = wv.evaluate_script(&script) {
                        println!("[IPC] evaluate_script status failed: {}", e);
                    }
                }
            }
        }

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
                let splash_size = (banner_size.0 / 2.0, banner_size.1 / 2.0);
                let center_x = (monitor_size.x - splash_size.0) / 2.0;
                let center_y = (monitor_size.y - splash_size.1) / 2.0;

                if self.center_attempts == 1 {
                    println!("[DEBUG] Centering splash window (attempt {}): monitor={}x{}, window={}x{}, pos=({}, {})",
                        self.center_attempts, monitor_size.x, monitor_size.y, splash_size.0, splash_size.1, center_x, center_y);
                }

                ctx.send_viewport_cmd(egui::ViewportCommand::OuterPosition(egui::pos2(
                    center_x.max(0.0),
                    center_y.max(0.0),
                )));

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
                let is_fullscreen = self.config.fullscreen_dialogs;
                if is_fullscreen {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Maximized(true));
                    println!("[DEBUG] Application set to windowed fullscreen (maximized)");
                } else {
                    const APP_WINDOW_SIZE: (f32, f32) = (1280.0, 1000.0);
                    ctx.send_viewport_cmd(egui::ViewportCommand::Maximized(false));
                    ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(egui::vec2(APP_WINDOW_SIZE.0, APP_WINDOW_SIZE.1)));
                    println!("[DEBUG] Application window resized to {}x{} with decorations enabled", APP_WINDOW_SIZE.0, APP_WINDOW_SIZE.1);
                    
                    let monitor_size = ctx.input(|i| i.viewport().monitor_size);
                    if let Some(monitor_size) = monitor_size {
                        let center_x = (monitor_size.x - APP_WINDOW_SIZE.0) / 2.0;
                        let center_y = (monitor_size.y - APP_WINDOW_SIZE.1) / 2.0;
                        ctx.send_viewport_cmd(egui::ViewportCommand::OuterPosition(egui::pos2(
                            center_x, center_y,
                        )));
                        println!("[DEBUG] Main window re-centered at: ({}, {})", center_x, center_y);
                    } else {
                        let screen_size = ctx.screen_rect().size();
                        let center_x = (screen_size.x - APP_WINDOW_SIZE.0) / 2.0;
                        let center_y = (screen_size.y - APP_WINDOW_SIZE.1) / 2.0;
                        ctx.send_viewport_cmd(egui::ViewportCommand::OuterPosition(egui::pos2(
                            center_x, center_y,
                        )));
                        println!(
                            "[DEBUG] Main window re-centered (fallback) at: ({}, {})",
                            center_x, center_y
                        );
                    }
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
                            egui::Color32::from_rgba_unmultiplied(
                                128,
                                128,
                                128,
                                (255.0 * fade_alpha) as u8,
                            ),
                        );
                    });
            }
        }


        if self.show_options {
            let options_size = egui::vec2(560.0, 520.0);
            let options_max = egui::vec2(600.0, 900.0);
            let screen = ctx.screen_rect();
            let options_pos = egui::pos2(
                screen.center().x - options_size.x / 2.0,
                screen.center().y - options_size.y / 2.0,
            );
            egui::Window::new("Options")
                .collapsible(false)
                .resizable(true)
                .default_size(options_size)
                .min_size(egui::vec2(400.0, 400.0))
                .max_size(options_max)
                .default_pos(options_pos)
                .show(ctx, |ui| {
                    if ui.checkbox(&mut self.config.fullscreen_dialogs, "Fullscreen Application").changed() {
                        if self.config.fullscreen_dialogs {
                            ctx.send_viewport_cmd(egui::ViewportCommand::Maximized(true));
                            println!("[DEBUG] Application set to windowed fullscreen (maximized)");
                        } else {
                            const APP_WINDOW_SIZE: (f32, f32) = (1280.0, 1000.0);
                            ctx.send_viewport_cmd(egui::ViewportCommand::Maximized(false));
                            ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(egui::vec2(APP_WINDOW_SIZE.0, APP_WINDOW_SIZE.1)));
                            let monitor_size = ctx.input(|i| i.viewport().monitor_size);
                            if let Some(monitor_size) = monitor_size {
                                let center_x = (monitor_size.x - APP_WINDOW_SIZE.0) / 2.0;
                                let center_y = (monitor_size.y - APP_WINDOW_SIZE.1) / 2.0;
                                ctx.send_viewport_cmd(egui::ViewportCommand::OuterPosition(egui::pos2(center_x, center_y)));
                            } else {
                                let screen_size = ctx.screen_rect().size();
                                let center_x = (screen_size.x - APP_WINDOW_SIZE.0) / 2.0;
                                let center_y = (screen_size.y - APP_WINDOW_SIZE.1) / 2.0;
                                ctx.send_viewport_cmd(egui::ViewportCommand::OuterPosition(egui::pos2(center_x, center_y)));
                            }
                            println!("[DEBUG] Application restored to windowed mode (1280x1000, centered)");
                        }
                        self.config.save();
                    }
                    
                    ui.add_space(15.0);
                    ui.separator();
                    ui.add_space(10.0);

                    ui.label(egui::RichText::new("Server Utility").size(14.0).strong());
                    ui.label("Paths used by the Server Utility. Configs are saved in content/server_utility.");
                    let paths_enabled = self.config.server_utility_wizard_completed;
                    if !paths_enabled {
                        ui.colored_label(
                            ui.visuals().weak_text_color(),
                            "Complete the Server Utility first-time setup (open Server Utility and finish the wizard) to edit paths.",
                        );
                        ui.add_space(4.0);
                    }
                    ui.add_space(8.0);
                    const PATH_INPUT_WIDTH: f32 = 400.0;
                    ui.label("HD2DS.exe path:");
                    ui.horizontal(|ui| {
                        ui.add_enabled(
                            paths_enabled,
                            egui::TextEdit::singleline(&mut self.config.server_hd2ds_path)
                                .desired_width(PATH_INPUT_WIDTH),
                        );
                        if ui.add_enabled(paths_enabled, egui::Button::new("📁 Browse…")).clicked() {
                            if let Some(p) = rfd::FileDialog::new().add_filter("Executable", &["exe"]).pick_file() {
                                self.config.server_hd2ds_path = p.to_string_lossy().into_owned();
                            }
                        }
                    });
                    let valid_hd2ds = Self::validate_server_path(0, &self.config.server_hd2ds_path);
                    if !valid_hd2ds && !self.config.server_hd2ds_path.trim().is_empty() {
                        ui.colored_label(
                            egui::Color32::from_rgb(220, 80, 80),
                            format!("Must be a file named \"{}\" that exists.", Self::server_path_expected_filename(0)),
                        );
                    }
                    ui.label("HD2DS Sabre Squadron path:");
                    ui.horizontal(|ui| {
                        ui.add_enabled(
                            paths_enabled,
                            egui::TextEdit::singleline(&mut self.config.server_sabresquadron_path)
                                .desired_width(PATH_INPUT_WIDTH),
                        );
                        if ui.add_enabled(paths_enabled, egui::Button::new("📁 Browse…")).clicked() {
                            if let Some(p) = rfd::FileDialog::new().add_filter("Executable", &["exe"]).pick_file() {
                                self.config.server_sabresquadron_path = p.to_string_lossy().into_owned();
                            }
                        }
                    });
                    let valid_sabre = Self::validate_server_path(1, &self.config.server_sabresquadron_path);
                    if !valid_sabre && !self.config.server_sabresquadron_path.trim().is_empty() {
                        ui.colored_label(
                            egui::Color32::from_rgb(220, 80, 80),
                            format!("Must be a file named \"{}\" that exists.", Self::server_path_expected_filename(1)),
                        );
                    }
                    ui.label("mpmaplist.txt location:");
                    ui.horizontal(|ui| {
                        ui.add_enabled(
                            paths_enabled,
                            egui::TextEdit::singleline(&mut self.config.server_mpmaplist_path)
                                .desired_width(PATH_INPUT_WIDTH),
                        );
                        if ui.add_enabled(paths_enabled, egui::Button::new("📁 Browse…")).clicked() {
                            if let Some(p) = rfd::FileDialog::new().add_filter("Text", &["txt"]).pick_file() {
                                self.config.server_mpmaplist_path = p.to_string_lossy().into_owned();
                            }
                        }
                    });
                    let valid_mpmaplist = Self::validate_server_path(2, &self.config.server_mpmaplist_path);
                    if !valid_mpmaplist && !self.config.server_mpmaplist_path.trim().is_empty() {
                        ui.colored_label(
                            egui::Color32::from_rgb(220, 80, 80),
                            format!("Must be a file named \"{}\" that exists.", Self::server_path_expected_filename(2)),
                        );
                    }

                    ui.add_space(15.0);
                    ui.separator();

                    if ui.button("Close").clicked() {
                        self.config.save();
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
                                ui.vertical_centered(|ui| {
                                    for credit in CREDITS {
                                        ui.label(*credit);
                                    }
                                });
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

        let card_error_msg = self.card_launch_error.clone();
        if let Some(ref msg) = card_error_msg {
            let mut acknowledged = false;
            egui::Window::new("Server Utility — Error")
                .collapsible(false)
                .resizable(true)
                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                .default_size([420.0, 180.0])
                .show(ctx, |ui| {
                    ui.label(egui::RichText::new("Could not open Server Utility.").strong());
                    ui.add_space(8.0);
                    ui.label("The web app could not be started. Common causes:");
                    ui.add_space(4.0);
                    ui.label("• WebView2 runtime may be missing or failed to create the embedded view.");
                    ui.add_space(12.0);
                    egui::ScrollArea::vertical()
                        .max_height(80.0)
                        .show(ui, |ui| {
                            ui.label(egui::RichText::new(msg.as_str()).color(ui.visuals().error_fg_color));
                        });
                    ui.add_space(8.0);
                    if ui.button("OK").clicked() {
                        acknowledged = true;
                    }
                });
            if acknowledged {
                self.card_launch_error = None;
            }
        }

        // Fade WebView in/out when modals open or close.
        #[cfg(windows)]
        if let Some(ref wv) = self.webview {
            const FADE_SPEED: f32 = 4.0; // ~0.25s for full fade
            let any_modal = self.show_options || self.show_about || self.card_launch_error.is_some();
            let dt = ctx.input(|i| i.unstable_dt).max(0.0).min(0.1);
            if let Some(hwnd) = get_webview_hwnd(frame) {
                if any_modal {
                    self.webview_fade_alpha = (self.webview_fade_alpha - dt * FADE_SPEED).max(0.0);
                    set_webview_opacity(hwnd, self.webview_fade_alpha);
                    if self.webview_fade_alpha <= 0.0 {
                        let _ = wv.set_visible(false);
                    } else {
                        ctx.request_repaint();
                    }
                } else {
                    let _ = wv.set_visible(true);
                    if self.webview_fade_alpha < 1.0 {
                        self.webview_fade_alpha = (self.webview_fade_alpha + dt * FADE_SPEED).min(1.0);
                        set_webview_opacity(hwnd, self.webview_fade_alpha);
                        ctx.request_repaint();
                    }
                    if self.webview_fade_alpha >= 1.0 {
                        set_webview_opacity(hwnd, 1.0);
                    }
                }
            } else {
                // Fallback if we can't get HWND: instant show/hide
                let show_webview = !any_modal;
                let _ = wv.set_visible(show_webview);
                self.webview_fade_alpha = if show_webview { 1.0 } else { 0.0 };
            }
        }

        egui::CentralPanel::default()
            .frame(egui::Frame::none().fill(ctx.style().visuals.extreme_bg_color))
            .show(ctx, |ui| {
                #[cfg(windows)]
                if self.webview.is_some() {
                    let any_modal = self.show_options || self.show_about || self.card_launch_error.is_some();
                    self.show_action_bar(ui, true);
                    let content_rect = ui.available_rect_before_wrap();
                    if any_modal && content_rect.width() > 0.0 && content_rect.height() > 0.0 {
                        ui.painter().rect_filled(
                            content_rect,
                            0.0,
                            ctx.style().visuals.extreme_bg_color,
                        );
                    }
                    return;
                }
                if self.current_module.is_some() {
                    self.show_action_bar(ui, false);
                    ui.add_space(4.0);
                    if let Some(ref mut module) = self.current_module {
                        let module_rect = ui.available_rect_before_wrap();
                        ui.allocate_ui_at_rect(module_rect, |ui| {
                            module.show(ctx, ui);
                        });
                    }
                } else {
                    self.show_landing_page(ui);
                }
            });

        // Apply "go home" from home button (done after panel so we don't double-borrow self)
        if ctx.data_mut(|d| d.get_temp::<()>(egui::Id::new("spectre_go_home")).is_some()) {
            ctx.data_mut(|d| d.remove::<()>(egui::Id::new("spectre_go_home")));
            #[cfg(windows)]
            if self.webview.is_some() {
                self.webview = None;
            }
            self.current_module = None;
            // Reload config so card readiness (e.g. wizard completed) is up to date
            self.config = Config::load();
        }
    }
}

