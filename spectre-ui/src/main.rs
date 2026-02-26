#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod config;
mod dialog;
#[cfg(windows)]
mod ds_helper;
mod modules;
mod server_prereqs;
mod splash;

use config::Config;
use eframe::egui;
use egui::{IconData, TextureHandle};
use image::GenericImageView;
use modules::{
    DtaUnpacker, GamedataEditor, InventoryEditor, ItemsEditor, Module, MpmaplistEditor,
    ServerLauncher,
};
use splash::SplashScreen;
use std::collections::{HashMap, HashSet};
use std::io::Write;
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

const VERSION: &str = env!("CARGO_PKG_VERSION");
const AUTHOR: &str = "Xevrac";
const ABOUT: &str = "Spectre is a toolkit for Hidden & Dangerous 2, providing various editing and management tools for the game.";

#[cfg(windows)]
#[derive(serde::Deserialize)]
struct IpcSaveMessage {
    action: String,
    servers: Vec<spectre_core::server::Server>,
    #[serde(default)]
    server_index: Option<usize>,
    #[serde(default)]
    server_manager: Option<spectre_core::server::ServerManager>,
    /// For action "browse_hd2_dir": "hd2ds" or "sabre"
    #[serde(default)]
    browse_which: Option<String>,
}

/// Path to hd2_server_config.json next to the executable.
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

/// Path to the app log file in content/server_utility. Prefer canonical exe path so it is absolute.
#[cfg(windows)]
fn app_log_path(_config_path: &std::path::Path) -> std::path::PathBuf {
    let path = std::env::current_exe()
        .ok()
        .and_then(|p| {
            std::fs::canonicalize(&p).ok().or(Some(p)).and_then(|p| {
                p.parent().map(|d| {
                    d.join("content")
                        .join("server_utility")
                        .join("spectre_app.log")
                })
            })
        })
        .unwrap_or_else(|| {
            std::path::PathBuf::from("content")
                .join("server_utility")
                .join("spectre_app.log")
        });
    path
}

#[cfg(windows)]
fn ensure_log_file_exists(path: &std::path::Path) {
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let _ = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path);
}

/// Append a timestamped line to the app log. If rotation_days > 0 and the file is older than that many days, the file is truncated first.
#[cfg(windows)]
fn write_app_log(state: &Arc<Mutex<(std::path::PathBuf, u32)>>, line: &str) {
    let (path, rotation_days) = match state.lock() {
        Ok(guard) => (guard.0.clone(), guard.1),
        Err(_) => return,
    };
    use std::io::Write;
    let now = chrono::Local::now();
    let timestamp = now.format("%Y-%m-%d %H:%M:%S");
    let full_line = format!("[{}] {}\n", timestamp, line);
    if rotation_days > 0 && path.exists() {
        if let Ok(meta) = std::fs::metadata(&path) {
            if let Ok(modified) = meta.modified() {
                let age = std::time::SystemTime::now()
                    .duration_since(modified)
                    .unwrap_or_default();
                let rotation_secs = rotation_days as u64 * 24 * 3600;
                if age.as_secs() >= rotation_secs {
                    let _ = std::fs::OpenOptions::new()
                        .write(true)
                        .truncate(true)
                        .open(&path);
                }
            }
        }
    }
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
    {
        let _ = f.write_all(full_line.as_bytes());
        let _ = f.flush();
    }
}

#[cfg(windows)]
fn browse_mpmaplist_with_validation() -> String {
    use std::io::BufRead;
    let path = match rfd::FileDialog::new()
        .add_filter("Text (mpmaplist)", &["txt"])
        .pick_file()
    {
        Some(p) => p,
        None => return "MPMAPLIST_PATH_CANCELLED".to_string(),
    };
    let path_str = path.to_string_lossy();
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .map(|s| s.to_lowercase());
    if ext.as_deref() != Some("txt") {
        return "MPMAPLIST_PATH_INVALID:Unexpected file format.".to_string();
    }
    let file = match std::fs::File::open(&path) {
        Ok(f) => f,
        Err(_) => return "MPMAPLIST_PATH_INVALID:Unexpected file format.".to_string(),
    };
    let mut first_non_empty = String::new();
    for line in std::io::BufReader::new(file).lines() {
        let line = match line {
            Ok(l) => l,
            Err(_) => return "MPMAPLIST_PATH_INVALID:Unexpected file format.".to_string(),
        };
        let trimmed = line.trim();
        if !trimmed.is_empty() {
            first_non_empty = trimmed.to_string();
            break;
        }
    }
    if !first_non_empty.starts_with("<MAP_LIST>") {
        return "MPMAPLIST_PATH_INVALID:Unexpected file format.".to_string();
    }
    format!("MPMAPLIST_PATH:{}", path_str)
}

/// Open file dialog to select the exe. which: "hd2ds" or "sabre".
#[cfg(windows)]
fn browse_hd2_exe(which: &str) -> String {
    let file = match rfd::FileDialog::new()
        .add_filter("Executable", &["exe"])
        .pick_file()
    {
        Some(p) => p,
        None => return "HD2DS_PATH_CANCELLED".to_string(),
    };
    let path_str = file.to_string_lossy();
    let prefix = if which == "sabre" {
        "HD2DS_SABRE_PATH:"
    } else {
        "HD2DS_PATH:"
    };
    format!("{}{}", prefix, path_str)
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

#[cfg(windows)]
fn get_primary_monitor_size_pixels() -> Option<(f32, f32)> {
    use windows::Win32::UI::WindowsAndMessaging::{GetSystemMetrics, SM_CXSCREEN, SM_CYSCREEN};
    let w = unsafe { GetSystemMetrics(SM_CXSCREEN) };
    let h = unsafe { GetSystemMetrics(SM_CYSCREEN) };
    if w > 0 && h > 0 {
        Some((w as f32, h as f32))
    } else {
        None
    }
}

#[cfg(not(windows))]
fn get_primary_monitor_size_pixels() -> Option<(f32, f32)> {
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

#[cfg(windows)]
fn load_tray_icon() -> Option<tray_icon::Icon> {
    let icon_bytes = include_bytes!("../spectre_256.png");
    let image = image::load_from_memory(icon_bytes).ok()?;
    let rgba = image.to_rgba8();
    let small = image::imageops::resize(&rgba, 16, 16, image::imageops::FilterType::Triangle);
    let (w, h) = small.dimensions();
    let bytes = small.into_raw();
    tray_icon::Icon::from_rgba(bytes, w, h).ok()
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
        "tray" => include_bytes!("../icons/tray.svg"),
        _ => return None,
    };

    let opt = resvg::usvg::Options::default();
    let rtree = match resvg::usvg::Tree::from_data(svg_bytes, &opt) {
        Ok(tree) => tree,
        Err(_) => return None,
    };

    let size = if name == "server_launcher" {
        64.0
    } else {
        16.0
    };
    let mut pixmap = match tiny_skia::Pixmap::new(size as u32, size as u32) {
        Some(p) => p,
        None => return None,
    };

    let tree_size = rtree.size();
    let transform =
        tiny_skia::Transform::from_scale(size / tree_size.width(), size / tree_size.height());

    resvg::render(&rtree, transform, &mut pixmap.as_mut());

    let rgba = pixmap.data();
    let color_image =
        egui::ColorImage::from_rgba_unmultiplied([size as usize, size as usize], rgba);

    Some(ctx.load_texture(format!("icon_{}", name), color_image, Default::default()))
}

#[cfg(windows)]
fn get_main_window_hwnd(frame: &eframe::Frame) -> Option<windows::Win32::Foundation::HWND> {
    use raw_window_handle::{HasWindowHandle, RawWindowHandle, Win32WindowHandle};
    let handle = frame.window_handle().ok()?;
    let raw = handle.as_raw();
    match raw {
        RawWindowHandle::Win32(Win32WindowHandle { hwnd, .. }) => {
            Some(windows::Win32::Foundation::HWND(hwnd.get() as _))
        }
        _ => None,
    }
}

#[cfg(windows)]
fn get_main_window_hwnd_opt(
    frame: Option<&eframe::Frame>,
) -> Option<windows::Win32::Foundation::HWND> {
    frame.and_then(get_main_window_hwnd)
}

#[cfg(windows)]
fn get_webview_hwnd(frame: &eframe::Frame) -> Option<windows::Win32::Foundation::HWND> {
    use windows::Win32::Foundation::LPARAM;
    use windows::Win32::UI::WindowsAndMessaging::{EnumChildWindows, GetWindow, GW_CHILD};

    let main_hwnd = get_main_window_hwnd(frame)?;
    if let Ok(child) = unsafe { GetWindow(main_hwnd, GW_CHILD) } {
        if !child.0.is_null() {
            return Some(child);
        }
    }
    let mut first_child = windows::Win32::Foundation::HWND::default();
    let _ = unsafe {
        EnumChildWindows(
            main_hwnd,
            Some(enum_child_first),
            LPARAM(&mut first_child as *mut _ as _),
        )
    };
    if !first_child.0.is_null() {
        Some(first_child)
    } else {
        None
    }
}

#[cfg(windows)]
fn get_webview_hwnd_opt(frame: Option<&eframe::Frame>) -> Option<windows::Win32::Foundation::HWND> {
    frame.and_then(get_webview_hwnd)
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
fn process_is_alive(pid: u32) -> bool {
    use windows::Win32::Foundation::CloseHandle;
    use windows::Win32::System::Threading::{
        GetExitCodeProcess, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION,
    };
    if pid == 0 {
        return false;
    }
    let handle = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) };
    let Ok(h) = handle else { return false };
    let mut exit_code: u32 = 0;
    let ok = unsafe { GetExitCodeProcess(h, &mut exit_code).is_ok() };
    let _ = unsafe { CloseHandle(h) };
    ok && exit_code == 259 // STILL_ACTIVE
}

#[cfg(windows)]
fn kill_process_by_pid(pid: u32) -> bool {
    use windows::Win32::Foundation::CloseHandle;
    use windows::Win32::System::Threading::{OpenProcess, TerminateProcess, PROCESS_TERMINATE};
    if pid == 0 {
        return false;
    }
    let handle = unsafe { OpenProcess(PROCESS_TERMINATE, false, pid) };
    let Ok(h) = handle else { return false };
    let result = unsafe { TerminateProcess(h, 1) };
    let _ = unsafe { CloseHandle(h) };
    result.is_ok()
}

#[cfg(windows)]
fn set_webview_opacity(hwnd: windows::Win32::Foundation::HWND, alpha: f32) {
    use windows::Win32::Foundation::COLORREF;
    use windows::Win32::UI::WindowsAndMessaging::{
        GetWindowLongW, SetLayeredWindowAttributes, SetWindowLongW, GWL_EXSTYLE, LWA_ALPHA,
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

const ARG_ELEVATED_APPLY_REGISTRY: &str = "--elevated-apply-registry";
const ARG_ELEVATED_APPLY_HOSTS: &str = "--elevated-apply-hosts";
const ARG_ELEVATED_CHECK_DIRECTPLAY: &str = "--elevated-check-directplay";
const ARG_ELEVATED_INSTALL_DIRECTPLAY: &str = "--elevated-install-directplay";

#[cfg(windows)]
const WEBVIEW2_CLIENT_GUID: &str = "{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}";

#[cfg(windows)]
fn is_webview2_runtime_installed() -> bool {
    use winreg::enums::{HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE};
    use winreg::RegKey;
    let paths = [
        (
            HKEY_LOCAL_MACHINE,
            format!(
                r"SOFTWARE\Microsoft\EdgeUpdate\Clients\{}",
                WEBVIEW2_CLIENT_GUID
            ),
        ),
        (
            HKEY_LOCAL_MACHINE,
            format!(
                r"SOFTWARE\WOW6432Node\Microsoft\EdgeUpdate\Clients\{}",
                WEBVIEW2_CLIENT_GUID
            ),
        ),
        (
            HKEY_CURRENT_USER,
            format!(
                r"Software\Microsoft\EdgeUpdate\Clients\{}",
                WEBVIEW2_CLIENT_GUID
            ),
        ),
    ];
    for (hkey, path) in &paths {
        let root = RegKey::predef(*hkey);
        if let Ok(key) = root.open_subkey(path) {
            if let Ok(pv) = key.get_value::<String, _>("pv") {
                if !pv.is_empty() && pv != "0.0.0.0" {
                    return true;
                }
            }
        }
    }
    false
}

#[cfg(windows)]
fn show_messagebox(title: &str, message: &str) {
    #[cfg(not(debug_assertions))]
    {
        use windows::Win32::System::Console::AllocConsole;
        unsafe {
            let _ = AllocConsole();
        }
        eprintln!("{}: {}", title, message);
    }
    use std::iter::once;
    use windows::core::PCWSTR;
    use windows::Win32::UI::WindowsAndMessaging::{MessageBoxW, MB_ICONERROR, MB_OK};
    let title_wide: Vec<u16> = title.encode_utf16().chain(once(0)).collect();
    let msg_wide: Vec<u16> = message.encode_utf16().chain(once(0)).collect();
    unsafe {
        MessageBoxW(
            None,
            PCWSTR::from_raw(msg_wide.as_ptr()),
            PCWSTR::from_raw(title_wide.as_ptr()),
            MB_OK | MB_ICONERROR,
        );
    }
}

#[cfg(windows)]
fn show_dependency_error_and_exit(title: &str, message: &str) -> ! {
    show_messagebox(title, message);
    std::process::exit(1);
}

#[cfg(windows)]
fn set_panic_messagebox_hook() {
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info: &std::panic::PanicHookInfo<'_>| {
        let msg = format!("{}", info);
        let _ = std::panic::catch_unwind(|| show_messagebox("Spectre – Crash", &msg));
        default_hook(info);
    }));
}

fn main() -> Result<(), eframe::Error> {
    #[cfg(windows)]
    set_panic_messagebox_hook();

    let mut args = std::env::args();
    if let Some(arg) = args.nth(1) {
        if arg == ARG_ELEVATED_APPLY_REGISTRY {
            println!("[Spectre.dbg] Elevated task: applying registry fix");
            match server_prereqs::apply_registry_fix() {
                Ok(()) => {
                    println!("[Spectre.dbg] Registry fix applied successfully");
                    std::process::exit(0);
                }
                Err(e) => {
                    eprintln!("{}", e);
                    std::process::exit(1);
                }
            }
        }
        if arg == ARG_ELEVATED_APPLY_HOSTS {
            println!("[Spectre.dbg] Elevated task: applying GameSpy hosts");
            match server_prereqs::apply_gamepy_hosts() {
                Ok(()) => {
                    println!("[Spectre.dbg] GameSpy hosts applied successfully");
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
                println!("[Spectre.dbg] DirectPlay check: emulating NOT installed (--emulate-no-directplay)");
                if let Err(e) = std::fs::write(path, "disabled") {
                    eprintln!("[Spectre.dbg] Failed to write result file: {}", e);
                    std::process::exit(1);
                }
                std::process::exit(0);
            }
            println!(
                "[Spectre.dbg] DirectPlay check: running elevated detection, result path={}",
                path.display()
            );
            match server_prereqs::run_check_directplay_and_write_result(path) {
                Ok(()) => std::process::exit(0),
                Err(e) => {
                    eprintln!("{}", e);
                    std::process::exit(1);
                }
            }
        }
        if arg == ARG_ELEVATED_INSTALL_DIRECTPLAY {
            println!("[Spectre.dbg] Elevated task: enabling DirectPlay");
            match server_prereqs::enable_directplay() {
                Ok(()) => {
                    println!("[Spectre.dbg] DirectPlay enabled successfully");
                    std::process::exit(0);
                }
                Err(e) => {
                    eprintln!("{}", e);
                    std::process::exit(1);
                }
            }
        }
    }

    #[cfg(windows)]
    if !is_webview2_runtime_installed() {
        show_dependency_error_and_exit(
            "Spectre – Missing dependency",
            "Microsoft Edge WebView2 Runtime is required but not installed.\n\n\
             Install it from: https://go.microsoft.com/fwlink/p/?LinkId=2124703\n\n\
             Then start Spectre again.",
        );
    }

    println!(
        "[Spectre.dbg] Spectre v{} starting...",
        env!("CARGO_PKG_VERSION")
    );
    if std::env::var("SPECTRE_PERF").is_ok() {
        println!("[Spectre.dbg] SPECTRE_PERF=1: IPC and drain timing enabled");
    }

    let banner_size = get_banner_size().unwrap_or((1024.0, 420.0));
    let mut splash_size = (banner_size.0 / 2.0, banner_size.1 / 2.0);
    if let Some((monitor_w, monitor_h)) = get_primary_monitor_size_pixels() {
        if splash_size.0 > monitor_w || splash_size.1 > monitor_h {
            let scale = (monitor_w / splash_size.0)
                .min(monitor_h / splash_size.1)
                .min(1.0);
            splash_size = (splash_size.0 * scale, splash_size.1 * scale);
            println!(
                "[Spectre.dbg] Clamped splash to fit display: {}x{}",
                splash_size.0, splash_size.1
            );
        }
    }
    println!(
        "[Spectre.dbg] Splash window size: {}x{}",
        splash_size.0, splash_size.1
    );

    if std::env::var_os("SPECTRE_USE_SOFTWARE").as_deref() == Some(std::ffi::OsStr::new("1")) {
        use egui_software_backend::{
            run_app_with_software_backend, SoftwareBackendAppConfiguration,
        };
        let settings = SoftwareBackendAppConfiguration::new()
            .inner_size(Some(egui::vec2(splash_size.0, splash_size.1)))
            .title(Some("Spectre".to_string()));
        return run_app_with_software_backend(settings, |ctx| SpectreApp::new_with_ctx(&ctx))
            .map_err(|e| {
                eframe::Error::AppCreation(Box::new(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    e.to_string(),
                )))
            });
    }

    let mut viewport_builder = egui::ViewportBuilder::default()
        .with_inner_size([splash_size.0, splash_size.1])
        .with_title("Spectre")
        .with_decorations(false);

    if let Some(icon) = load_icon() {
        println!("[Spectre.dbg] Application icon loaded successfully");
        viewport_builder = viewport_builder.with_icon(icon);
    } else {
        println!("[Spectre.dbg] Warning: Failed to load application icon, using default");
    }

    println!("[Spectre.dbg] Initializing eframe application...");
    #[cfg(windows)]
    let (options, _) = {
        let use_glow =
            std::env::var_os("SPECTRE_USE_GLOW").as_deref() == Some(std::ffi::OsStr::new("1"));
        let use_wgpu_gl =
            std::env::var_os("SPECTRE_USE_WGPU_GL").as_deref() == Some(std::ffi::OsStr::new("1"));
        let use_software_gpu = std::env::var_os("SPECTRE_USE_SOFTWARE_GPU").as_deref()
            == Some(std::ffi::OsStr::new("1"));
        if use_glow {
            let opts = eframe::NativeOptions {
                viewport: viewport_builder.clone(),
                renderer: eframe::Renderer::Glow,
                ..Default::default()
            };
            (opts, ())
        } else {
            let mut opts = eframe::NativeOptions {
                viewport: viewport_builder.clone(),
                renderer: eframe::Renderer::Wgpu,
                ..Default::default()
            };
            if use_software_gpu || use_wgpu_gl {
                use eframe::egui_wgpu::{WgpuSetup, WgpuSetupCreateNew};
                let mut create_new = match &opts.wgpu_options.wgpu_setup {
                    WgpuSetup::CreateNew(c) => c.clone(),
                    _ => WgpuSetupCreateNew::default(),
                };
                create_new.power_preference = eframe::egui_wgpu::wgpu::PowerPreference::LowPower;
                if use_wgpu_gl {
                    create_new.instance_descriptor.backends = eframe::egui_wgpu::wgpu::Backends::GL;
                }
                opts.wgpu_options.wgpu_setup = WgpuSetup::CreateNew(create_new);
            }
            (opts, ())
        }
    };
    #[cfg(not(windows))]
    let options = eframe::NativeOptions {
        viewport: viewport_builder.clone(),
        renderer: eframe::Renderer::Glow,
        ..Default::default()
    };
    let result = eframe::run_native(
        "Spectre",
        options,
        Box::new(|cc| Ok(Box::new(SpectreApp::new(cc)))),
    );
    if let Err(ref e) = result {
        #[cfg(windows)]
        {
            let err_str = e.to_string();
            let lower = err_str.to_lowercase();
            let msg = if lower.contains("webview2") || lower.contains("webview") {
                format!(
                    "{}\n\nInstall the WebView2 Runtime: https://go.microsoft.com/fwlink/p/?LinkId=2124703",
                    err_str
                )
            } else if lower.contains("no suitable adapter")
                || lower.contains("failed to create wgpu adapter")
            {
                #[cfg(windows)]
                let already_software = std::env::var_os("SPECTRE_USE_SOFTWARE_GPU").as_deref()
                    == Some(std::ffi::OsStr::new("1"));
                #[cfg(windows)]
                let already_wgpu_gl = std::env::var_os("SPECTRE_USE_WGPU_GL").as_deref()
                    == Some(std::ffi::OsStr::new("1"));
                #[cfg(windows)]
                let already_glow = std::env::var_os("SPECTRE_USE_GLOW").as_deref()
                    == Some(std::ffi::OsStr::new("1"));
                #[cfg(not(windows))]
                let (already_software, already_wgpu_gl, already_glow) = (false, false, false);

                #[cfg(windows)]
                if !already_software && !already_wgpu_gl {
                    let exe = std::env::current_exe().ok();
                    if let Some(exe) = exe {
                        let mut cmd = std::process::Command::new(&exe);
                        cmd.env("SPECTRE_USE_SOFTWARE_GPU", "1");
                        cmd.env_remove("SPECTRE_USE_WGPU_GL");
                        cmd.env_remove("SPECTRE_USE_GLOW");
                        cmd.stdin(std::process::Stdio::null());
                        if let Ok(mut child) = cmd.spawn() {
                            let _ = child.wait();
                            return Ok(());
                        }
                    }
                }

                #[cfg(windows)]
                if already_software && !already_wgpu_gl {
                    let exe = std::env::current_exe().ok();
                    if let Some(exe) = exe {
                        let mut cmd = std::process::Command::new(&exe);
                        cmd.env("SPECTRE_USE_SOFTWARE_GPU", "1");
                        cmd.env("SPECTRE_USE_WGPU_GL", "1");
                        cmd.env_remove("SPECTRE_USE_GLOW");
                        cmd.stdin(std::process::Stdio::null());
                        if let Ok(mut child) = cmd.spawn() {
                            let _ = child.wait();
                            return Ok(());
                        }
                    }
                }

                #[cfg(windows)]
                if already_wgpu_gl && !already_glow {
                    let exe = std::env::current_exe().ok();
                    if let Some(exe) = exe {
                        let mut cmd = std::process::Command::new(&exe);
                        cmd.env("SPECTRE_USE_SOFTWARE_GPU", "1");
                        cmd.env("SPECTRE_USE_WGPU_GL", "1");
                        cmd.env("SPECTRE_USE_GLOW", "1");
                        cmd.stdin(std::process::Stdio::null());
                        if let Ok(mut child) = cmd.spawn() {
                            let _ = child.wait();
                            return Ok(());
                        }
                    }
                }

                #[cfg(windows)]
                let already_software_app = std::env::var_os("SPECTRE_USE_SOFTWARE").as_deref()
                    == Some(std::ffi::OsStr::new("1"));
                #[cfg(not(windows))]
                let already_software_app = false;

                #[cfg(windows)]
                if already_glow && !already_software_app {
                    let exe = std::env::current_exe().ok();
                    if let Some(exe) = exe {
                        let mut cmd = std::process::Command::new(&exe);
                        cmd.env("SPECTRE_USE_SOFTWARE", "1");
                        cmd.stdin(std::process::Stdio::null());
                        if let Ok(mut child) = cmd.spawn() {
                            let _ = child.wait();
                            return Ok(());
                        }
                    }
                }

                if already_wgpu_gl {
                    format!(
                        "{}\n\nwgpu with OpenGL backend did not find a suitable adapter (e.g. Microsoft Basic Display Adapter). Run on a machine with a display adapter or use RDP with graphics enabled.",
                        err_str
                    )
                } else if already_glow && already_software_app {
                    format!(
                        "{}\n\nSpectre tried GPU, WARP, OpenGL, and the CPU software renderer — none worked. Run in release mode (cargo build --release) for better software rendering performance, or use a machine with a display adapter.",
                        err_str
                    )
                } else if already_software && already_glow {
                    format!(
                        "{}\n\nNo graphics adapter or OpenGL available. Spectre will try the CPU software renderer next.",
                        err_str
                    )
                } else if already_software {
                    format!(
                        "{}\n\nSoftware rendering (WARP) is not available on this system. Run Spectre on a machine with a display adapter, or use RDP with a session that has graphics enabled.",
                        err_str
                    )
                } else {
                    format!(
                        "{}\n\nNo graphics adapter found. Spectre will try wgpu (OpenGL), then software (WARP), then OpenGL. In remote or headless environments, use a session with GPU (e.g. RDP with graphics) or run on a machine with a display adapter.",
                        err_str
                    )
                }
            } else if lower.contains("opengl")
                || lower.contains("gl ")
                || lower.contains("egui_glow")
            {
                #[cfg(windows)]
                let used_glow_fallback = std::env::var_os("SPECTRE_USE_GLOW").as_deref()
                    == Some(std::ffi::OsStr::new("1"));
                #[cfg(not(windows))]
                let used_glow_fallback = false;
                if used_glow_fallback {
                    format!(
                        "{}\n\nSpectre tried GPU, WARP, then OpenGL — none are available. It will try the CPU software renderer next; if that fails, set SPECTRE_USE_SOFTWARE=1 to run without a GPU.",
                        err_str
                    )
                } else {
                    format!(
                        "{}\n\nUpdate your graphics drivers or use a system that supports OpenGL 2.0 or wgpu (DX12/Vulkan).",
                        err_str
                    )
                }
            } else if lower.contains("recreation") || lower.contains("event loop") {
                format!(
                    "{}\n\nThis usually means the graphics backend failed to start. Try updating display drivers or running on a machine with a supported adapter (e.g. Microsoft Basic Display Adapter).",
                    err_str
                )
            } else {
                err_str
            };
            show_messagebox("Spectre – Failed to start", &msg);
        }
    }
    result
}

struct SpectreApp {
    version: String,
    config: Config,
    current_module: Option<Box<dyn Module>>,
    show_about: bool,
    show_options: bool,
    card_launch_error: Option<String>,
    #[cfg(windows)]
    pending_webview_card: Option<String>,
    #[cfg(windows)]
    webview_pending_creation: Option<String>,
    #[cfg(windows)]
    webview: Option<wry::WebView>,
    #[cfg(windows)]
    webview_fade_alpha: f32,
    #[cfg(windows)]
    ipc_save_rx: Option<mpsc::Receiver<String>>,
    #[cfg(windows)]
    pending_webview_refresh: bool,
    #[cfg(windows)]
    webview_repaint_frames: u8,
    #[cfg(windows)]
    server_pids: Arc<Mutex<HashMap<u16, u32>>>,
    #[cfg(windows)]
    last_watchdog_check: Option<Instant>,
    #[cfg(windows)]
    tray_icon: Option<tray_icon::TrayIcon>,
    #[cfg(windows)]
    tray_show_id: Option<tray_icon::menu::MenuId>,
    #[cfg(windows)]
    tray_quit_id: Option<tray_icon::menu::MenuId>,
    #[cfg(windows)]
    window_hidden_to_tray: bool,
    #[cfg(windows)]
    pending_hide_to_tray: bool,
    /// When minimized to tray: (x, y, width, height) to restore. Window is moved off-screen instead of SW_HIDE so the event loop keeps running.
    #[cfg(windows)]
    saved_tray_rect: Option<(i32, i32, i32, i32)>,
    #[cfg(windows)]
    helper_kicked: Arc<Mutex<HashMap<u16, HashSet<String>>>>,
    #[cfg(windows)]
    helper_last_slots: Arc<Mutex<HashMap<u16, Vec<(String, String)>>>>,
    /// (log file path, rotation_days) for app log. Set when Server Launcher webview is created.
    #[cfg(windows)]
    log_state: Option<Arc<Mutex<(std::path::PathBuf, u32)>>>,
    #[cfg(windows)]
    background_timer_set: bool,
    splash_screen: Option<SplashScreen>,
    window_centered: bool,
    center_attempts: u32,
    card_icon: Option<TextureHandle>,
    home_icon: Option<TextureHandle>,
    settings_icon: Option<TextureHandle>,
    info_icon: Option<TextureHandle>,
    #[cfg(windows)]
    tray_button_icon: Option<TextureHandle>,
    refresh_icon: Option<TextureHandle>,
    #[cfg(debug_assertions)]
    console_icon: Option<TextureHandle>,
}

impl SpectreApp {
    fn new(cc: &eframe::CreationContext<'_>) -> Self {
        Self::new_with_ctx(&cc.egui_ctx)
    }

    fn new_with_ctx(ctx: &egui::Context) -> Self {
        println!("[Spectre.dbg] Creating SpectreApp instance...");
        let splash = SplashScreen::new(ctx);
        println!("[Spectre.dbg] Splash screen initialized");

        let config = Config::load();
        println!("[Spectre.dbg] Configuration loaded");

        Self::apply_theme(ctx);

        let card_icon = load_svg_icon(ctx, "server_launcher");
        let home_icon = load_svg_icon(ctx, "home");
        let settings_icon = load_svg_icon(ctx, "settings");
        let info_icon = load_svg_icon(ctx, "info");
        #[cfg(windows)]
        let tray_button_icon = load_svg_icon(ctx, "tray");
        let refresh_icon = load_svg_icon(ctx, "refresh");
        #[cfg(debug_assertions)]
        let console_icon = load_svg_icon(ctx, "console");

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
            webview_pending_creation: None,
            #[cfg(windows)]
            webview: None,
            #[cfg(windows)]
            webview_fade_alpha: 1.0,
            #[cfg(windows)]
            ipc_save_rx: None,
            #[cfg(windows)]
            pending_webview_refresh: false,
            #[cfg(windows)]
            webview_repaint_frames: 0,
            #[cfg(windows)]
            server_pids: Arc::new(Mutex::new(HashMap::new())),
            #[cfg(windows)]
            last_watchdog_check: None,
            #[cfg(windows)]
            tray_icon: None,
            #[cfg(windows)]
            tray_show_id: None,
            #[cfg(windows)]
            tray_quit_id: None,
            #[cfg(windows)]
            window_hidden_to_tray: false,
            #[cfg(windows)]
            pending_hide_to_tray: false,
            #[cfg(windows)]
            saved_tray_rect: None,
            #[cfg(windows)]
            helper_kicked: Arc::new(Mutex::new(HashMap::new())),
            #[cfg(windows)]
            helper_last_slots: Arc::new(Mutex::new(HashMap::new())),
            #[cfg(windows)]
            log_state: None,
            background_timer_set: false,
            splash_screen: Some(splash),
            window_centered: false,
            center_attempts: 0,
            card_icon,
            home_icon,
            settings_icon,
            info_icon,
            #[cfg(windows)]
            tray_button_icon,
            refresh_icon,
            #[cfg(debug_assertions)]
            console_icon,
        }
    }

    fn apply_theme(ctx: &egui::Context) {
        ctx.set_visuals(egui::Visuals::dark());
    }

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
                    let home_r =
                        ui.allocate_response(egui::Vec2::new(BTN_W, BTN_H), egui::Sense::click());
                    let fill = if home_r.hovered() {
                        ui.visuals().widgets.hovered.bg_fill
                    } else {
                        ui.visuals().widgets.inactive.bg_fill
                    };
                    ui.painter().rect_filled(home_r.rect, 4.0, fill);
                    if let Some(ref t) = self.home_icon {
                        let r = egui::Rect::from_center_size(
                            home_r.rect.center(),
                            egui::vec2(ICON_SZ, ICON_SZ),
                        );
                        ui.painter().image(
                            t.id(),
                            r,
                            egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                            ui.visuals().text_color(),
                        );
                    } else {
                        let galley = ui.painter().layout_no_wrap(
                            "⌂".to_string(),
                            egui::FontId::new(14.0, egui::FontFamily::Proportional),
                            ui.visuals().text_color(),
                        );
                        ui.painter().galley(
                            home_r.rect.center() - galley.size() / 2.0,
                            galley,
                            ui.visuals().text_color(),
                        );
                    }
                    if home_r.clicked() {
                        ui.ctx()
                            .data_mut(|d| d.insert_temp(egui::Id::new("spectre_go_home"), ()));
                    }
                    if home_r.hovered() {
                        ui.ctx()
                            .output_mut(|o| o.cursor_icon = egui::CursorIcon::PointingHand);
                        if webview_active {
                            ui.label(
                                egui::RichText::new("Return to main screen")
                                    .size(12.0)
                                    .color(ui.visuals().weak_text_color()),
                            );
                        } else {
                            egui::show_tooltip(
                                ui.ctx(),
                                ui.layer_id(),
                                egui::Id::new("action_bar_home"),
                                |ui| ui.label("Return to main screen"),
                            );
                        }
                    }
                    let set_r =
                        ui.allocate_response(egui::Vec2::new(BTN_W, BTN_H), egui::Sense::click());
                    let fill = if set_r.hovered() {
                        ui.visuals().widgets.hovered.bg_fill
                    } else {
                        ui.visuals().widgets.inactive.bg_fill
                    };
                    ui.painter().rect_filled(set_r.rect, 4.0, fill);
                    if let Some(ref t) = self.settings_icon {
                        let r = egui::Rect::from_center_size(
                            set_r.rect.center(),
                            egui::vec2(ICON_SZ, ICON_SZ),
                        );
                        ui.painter().image(
                            t.id(),
                            r,
                            egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                            ui.visuals().text_color(),
                        );
                    } else {
                        let galley = ui.painter().layout_no_wrap(
                            "⚙".to_string(),
                            egui::FontId::new(14.0, egui::FontFamily::Proportional),
                            ui.visuals().text_color(),
                        );
                        ui.painter().galley(
                            set_r.rect.center() - galley.size() / 2.0,
                            galley,
                            ui.visuals().text_color(),
                        );
                    }
                    if set_r.clicked() {
                        self.show_options = true;
                    }
                    if set_r.hovered() {
                        ui.ctx()
                            .output_mut(|o| o.cursor_icon = egui::CursorIcon::PointingHand);
                        if webview_active {
                            ui.label(
                                egui::RichText::new("Settings")
                                    .size(12.0)
                                    .color(ui.visuals().weak_text_color()),
                            );
                        } else {
                            egui::show_tooltip(
                                ui.ctx(),
                                ui.layer_id(),
                                egui::Id::new("action_bar_settings"),
                                |ui| ui.label("Settings"),
                            );
                        }
                    }
                    let info_r =
                        ui.allocate_response(egui::Vec2::new(BTN_W, BTN_H), egui::Sense::click());
                    let fill = if info_r.hovered() {
                        ui.visuals().widgets.hovered.bg_fill
                    } else {
                        ui.visuals().widgets.inactive.bg_fill
                    };
                    ui.painter().rect_filled(info_r.rect, 4.0, fill);
                    if let Some(ref t) = self.info_icon {
                        let r = egui::Rect::from_center_size(
                            info_r.rect.center(),
                            egui::vec2(ICON_SZ, ICON_SZ),
                        );
                        ui.painter().image(
                            t.id(),
                            r,
                            egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                            ui.visuals().text_color(),
                        );
                    } else {
                        let galley = ui.painter().layout_no_wrap(
                            "ℹ".to_string(),
                            egui::FontId::new(14.0, egui::FontFamily::Proportional),
                            ui.visuals().text_color(),
                        );
                        ui.painter().galley(
                            info_r.rect.center() - galley.size() / 2.0,
                            galley,
                            ui.visuals().text_color(),
                        );
                    }
                    if info_r.clicked() {
                        self.show_about = true;
                    }
                    if info_r.hovered() {
                        ui.ctx()
                            .output_mut(|o| o.cursor_icon = egui::CursorIcon::PointingHand);
                        if webview_active {
                            ui.label(
                                egui::RichText::new("About")
                                    .size(12.0)
                                    .color(ui.visuals().weak_text_color()),
                            );
                        } else {
                            egui::show_tooltip(
                                ui.ctx(),
                                ui.layer_id(),
                                egui::Id::new("action_bar_info"),
                                |ui| ui.label("About"),
                            );
                        }
                    }
                    #[cfg(windows)]
                    if self.tray_icon.is_some() {
                        let tray_r = ui
                            .allocate_response(egui::Vec2::new(BTN_W, BTN_H), egui::Sense::click());
                        let fill = if tray_r.hovered() {
                            ui.visuals().widgets.hovered.bg_fill
                        } else {
                            ui.visuals().widgets.inactive.bg_fill
                        };
                        ui.painter().rect_filled(tray_r.rect, 4.0, fill);
                        if let Some(ref t) = self.tray_button_icon {
                            let r = egui::Rect::from_center_size(
                                tray_r.rect.center(),
                                egui::vec2(ICON_SZ, ICON_SZ),
                            );
                            ui.painter().image(
                                t.id(),
                                r,
                                egui::Rect::from_min_max(
                                    egui::pos2(0.0, 0.0),
                                    egui::pos2(1.0, 1.0),
                                ),
                                ui.visuals().text_color(),
                            );
                        } else {
                            let galley = ui.painter().layout_no_wrap(
                                "▢".to_string(),
                                egui::FontId::new(14.0, egui::FontFamily::Proportional),
                                ui.visuals().text_color(),
                            );
                            ui.painter().galley(
                                tray_r.rect.center() - galley.size() / 2.0,
                                galley,
                                ui.visuals().text_color(),
                            );
                        }
                        if tray_r.clicked() {
                            self.pending_hide_to_tray = true;
                        }
                        if tray_r.hovered() {
                            ui.ctx()
                                .output_mut(|o| o.cursor_icon = egui::CursorIcon::PointingHand);
                            if webview_active {
                                ui.label(
                                    egui::RichText::new("Minimize to tray")
                                        .size(12.0)
                                        .color(ui.visuals().weak_text_color()),
                                );
                            } else {
                                egui::show_tooltip(
                                    ui.ctx(),
                                    ui.layer_id(),
                                    egui::Id::new("action_bar_tray"),
                                    |ui| ui.label("Minimize to tray"),
                                );
                            }
                        }
                    }
                    let rest = ui.available_width();
                    let right_w = if webview_active {
                        if cfg!(debug_assertions) {
                            BTN_W * 2.0 + BTN_GAP
                        } else {
                            BTN_W
                        }
                    } else {
                        0.0
                    };
                    if rest > right_w {
                        ui.add_space(rest - right_w);
                    }
                    ui.with_layout(
                        egui::Layout::right_to_left(egui::Align::Center).with_main_justify(false),
                        |ui| {
                            ui.add_space(ACTION_BAR_RIGHT_MARGIN);
                            ui.spacing_mut().item_spacing = egui::vec2(BTN_GAP, 0.0);
                            if webview_active {
                                let ref_r = ui.allocate_response(
                                    egui::Vec2::new(BTN_W, BTN_H),
                                    egui::Sense::click(),
                                );
                                let fill = if ref_r.hovered() {
                                    ui.visuals().widgets.hovered.bg_fill
                                } else {
                                    ui.visuals().widgets.inactive.bg_fill
                                };
                                ui.painter().rect_filled(ref_r.rect, 4.0, fill);
                                if let Some(ref t) = self.refresh_icon {
                                    let r = egui::Rect::from_center_size(
                                        ref_r.rect.center(),
                                        egui::vec2(ICON_SZ, ICON_SZ),
                                    );
                                    ui.painter().image(
                                        t.id(),
                                        r,
                                        egui::Rect::from_min_max(
                                            egui::pos2(0.0, 0.0),
                                            egui::pos2(1.0, 1.0),
                                        ),
                                        ui.visuals().text_color(),
                                    );
                                } else {
                                    let galley = ui.painter().layout_no_wrap(
                                        "↻".to_string(),
                                        egui::FontId::new(14.0, egui::FontFamily::Proportional),
                                        ui.visuals().text_color(),
                                    );
                                    ui.painter().galley(
                                        ref_r.rect.center() - galley.size() / 2.0,
                                        galley,
                                        ui.visuals().text_color(),
                                    );
                                }
                                if ref_r.clicked() {
                                    #[cfg(windows)]
                                    {
                                        self.pending_webview_refresh = true;
                                    }
                                }
                                if ref_r.hovered() {
                                    ui.ctx().output_mut(|o| {
                                        o.cursor_icon = egui::CursorIcon::PointingHand
                                    });
                                    ui.label(
                                        egui::RichText::new("Refresh")
                                            .size(12.0)
                                            .color(ui.visuals().weak_text_color()),
                                    );
                                }
                            }
                            #[cfg(debug_assertions)]
                            if webview_active {
                                let dev_r = ui.allocate_response(
                                    egui::Vec2::new(BTN_W, BTN_H),
                                    egui::Sense::click(),
                                );
                                let fill = if dev_r.hovered() {
                                    ui.visuals().widgets.hovered.bg_fill
                                } else {
                                    ui.visuals().widgets.inactive.bg_fill
                                };
                                ui.painter().rect_filled(dev_r.rect, 4.0, fill);
                                if let Some(ref t) = self.console_icon {
                                    let r = egui::Rect::from_center_size(
                                        dev_r.rect.center(),
                                        egui::vec2(ICON_SZ, ICON_SZ),
                                    );
                                    ui.painter().image(
                                        t.id(),
                                        r,
                                        egui::Rect::from_min_max(
                                            egui::pos2(0.0, 0.0),
                                            egui::pos2(1.0, 1.0),
                                        ),
                                        ui.visuals().text_color(),
                                    );
                                } else {
                                    let galley = ui.painter().layout_no_wrap(
                                        ">_".to_string(),
                                        egui::FontId::new(14.0, egui::FontFamily::Proportional),
                                        ui.visuals().text_color(),
                                    );
                                    ui.painter().galley(
                                        dev_r.rect.center() - galley.size() / 2.0,
                                        galley,
                                        ui.visuals().text_color(),
                                    );
                                }
                                if dev_r.clicked() {
                                    if let Some(ref wv) = self.webview {
                                        wv.open_devtools();
                                    }
                                }
                                if dev_r.hovered() {
                                    ui.ctx().output_mut(|o| {
                                        o.cursor_icon = egui::CursorIcon::PointingHand
                                    });
                                    ui.label(
                                        egui::RichText::new("Open DevTools")
                                            .size(12.0)
                                            .color(ui.visuals().weak_text_color()),
                                    );
                                }
                            }
                        },
                    );
                },
            );
            let rect = ui.available_rect_before_wrap();
            if rect.height() >= 1.0 {
                let line_y = rect.bottom() - 1.0;
                ui.painter().line_segment(
                    [
                        egui::pos2(rect.left(), line_y),
                        egui::pos2(rect.right(), line_y),
                    ],
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
        let usable_width = (available_width - (side_padding * 2.0))
            .max(min_card_width)
            .min(available_width);

        let cards: Vec<(&str, &str, &str, usize, bool)> = vec![
            (
                "Server Utility",
                "Launch and manage HD2 game servers",
                "Tool",
                0,
                true,
            ),
            (
                "DTA Unpacker",
                "Extract and unpack DTA archive files",
                "Tool",
                1,
                false,
            ),
            (
                "Inventory Editor",
                "Edit player inventory files",
                "Editor",
                2,
                false,
            ),
            (
                "Items Editor",
                "Edit item values and create items",
                "Editor",
                3,
                false,
            ),
            (
                "MP Maplist Editor",
                "Edit multiplayer maplist files",
                "Editor",
                4,
                false,
            ),
            (
                "Gamedata Editor",
                "Edit gamedata00.gdt and gamedata01.gdt",
                "Editor",
                5,
                false,
            ),
        ];

        let mut cards_per_row = ((usable_width + gap) / (max_card_width + gap)).floor() as usize;
        cards_per_row = cards_per_row.max(1).min(4);

        let total_gaps = gap * (cards_per_row.saturating_sub(1)) as f32;
        let card_width = ((usable_width - total_gaps) / cards_per_row as f32)
            .max(min_card_width)
            .min(max_card_width);

        let card_height = 160.0;
        let margin = 4.0;
        let content_width =
            (card_width * cards_per_row as f32) + (gap * (cards_per_row.saturating_sub(1)) as f32);

        egui::ScrollArea::vertical()
            .auto_shrink([false; 2])
            .show(ui, |ui| {
                ui.set_width(available_width);
                ui.with_layout(egui::Layout::top_down(egui::Align::Center), |ui| {
                    ui.set_width(available_width);
                    ui.allocate_ui(egui::vec2(content_width, 0.0), |ui| {
                        ui.set_width(content_width);
                        ui.set_min_width(content_width);

                        ui.add_space(40.0);
                        let button_group_width = 36.0 + 12.0 + 36.0;
                        ui.horizontal(|ui| {
                            ui.set_width(content_width);
                            ui.spacing_mut().item_spacing.x = 0.0;
                            let strong = ui.visuals().strong_text_color();
                            let weak = ui.visuals().weak_text_color();
                            let font_56 = egui::FontId::new(56.0, egui::FontFamily::Proportional);
                            let font_18 = egui::FontId::new(18.0, egui::FontFamily::Proportional);
                            let g1 = ui
                                .painter()
                                .layout_no_wrap("Spectre".into(), font_56, strong);
                            let g2 = ui.painter().layout_no_wrap(
                                "Hidden & Dangerous 2 Toolkit".into(),
                                font_18,
                                weak,
                            );
                            let title_w = g1.size().x.max(g2.size().x);
                            let title_h = g1.size().y + 8.0 + g2.size().y;
                            let left_space = (content_width / 2.0 - title_w / 2.0).max(0.0);
                            let g1_w = g1.size().x;
                            let right_space =
                                (g1_w / 2.0 - title_w / 2.0 - button_group_width).max(8.0);
                            let g1_h = g1.size().y;
                            let g2_w = g2.size().x;
                            ui.add_space(left_space);
                            ui.allocate_ui(egui::vec2(title_w, title_h), |ui| {
                                let pos = ui.cursor().min;
                                let x1 = pos.x + (title_w - g1_w) / 2.0;
                                let x2 = pos.x + (title_w - g2_w) / 2.0;
                                ui.painter().galley(egui::pos2(x1, pos.y), g1, strong);
                                ui.painter()
                                    .galley(egui::pos2(x2, pos.y + g1_h + 8.0), g2, weak);
                            });
                            ui.add_space(right_space);
                            let button_h = 28.0;
                            let space_above_buttons = (title_h * 0.5 - button_h * 0.5).max(0.0);
                            ui.vertical(|ui| {
                                ui.add_space(space_above_buttons);
                                ui.with_layout(
                                    egui::Layout::right_to_left(egui::Align::Min),
                                    |ui| {
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
                                            egui::Stroke::new(
                                                1.0,
                                                ui.visuals().widgets.inactive.bg_stroke.color,
                                            ),
                                            egui::StrokeKind::Inside,
                                        );
                                        let font =
                                            egui::FontId::new(18.0, egui::FontFamily::Proportional);
                                        let galley = ui.painter().layout_no_wrap(
                                            "ℹ".to_string(),
                                            font.clone(),
                                            ui.visuals().text_color(),
                                        );
                                        let text_size = galley.size();
                                        let button_center = about_response.rect.center();
                                        let icon_pos = egui::pos2(
                                            button_center.x - text_size.x * 0.5,
                                            button_center.y - text_size.y * 0.5,
                                        );
                                        ui.painter().galley(
                                            icon_pos,
                                            galley,
                                            ui.visuals().text_color(),
                                        );
                                        if about_response.clicked() {
                                            self.show_about = true;
                                        }
                                        if about_response.hovered() {
                                            ui.ctx().output_mut(|o| {
                                                o.cursor_icon = egui::CursorIcon::PointingHand
                                            });
                                            egui::show_tooltip(
                                                ui.ctx(),
                                                ui.layer_id(),
                                                egui::Id::new("about_btn"),
                                                |ui| ui.label("About"),
                                            );
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
                                        ui.painter().rect_filled(
                                            settings_response.rect.expand(0.0),
                                            4.0,
                                            fill,
                                        );
                                        ui.painter().rect_stroke(
                                            settings_response.rect.expand(0.0),
                                            4.0,
                                            egui::Stroke::new(
                                                1.0,
                                                ui.visuals().widgets.inactive.bg_stroke.color,
                                            ),
                                            egui::StrokeKind::Inside,
                                        );
                                        let font =
                                            egui::FontId::new(16.0, egui::FontFamily::Proportional);
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
                                        ui.painter().galley(
                                            icon_pos,
                                            galley,
                                            ui.visuals().text_color(),
                                        );
                                        if settings_response.clicked() {
                                            self.show_options = true;
                                        }
                                        if settings_response.hovered() {
                                            ui.ctx().output_mut(|o| {
                                                o.cursor_icon = egui::CursorIcon::PointingHand
                                            });
                                            egui::show_tooltip(
                                                ui.ctx(),
                                                ui.layer_id(),
                                                egui::Id::new("settings_btn"),
                                                |ui| ui.label("Settings"),
                                            );
                                        }
                                    },
                                );
                            });
                        });

                        ui.add_space(80.0);

                        let mut row_start = 0;
                        while row_start < cards.len() {
                            let row_end = (row_start + cards_per_row).min(cards.len());
                            let row_cards = &cards[row_start..row_end];

                            ui.horizontal(|ui| {
                                ui.set_width(content_width);
                                for (i, (title, desc, cat, idx, is_ready)) in
                                    row_cards.iter().enumerate()
                                {
                                    if i > 0 {
                                        ui.add_space(gap);
                                    }
                                    let clicked = ui
                                        .allocate_ui_with_layout(
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
                                                if !self.config.server_utility_wizard_completed {
                                                    self.current_module =
                                                        Some(Box::new(ServerLauncher::default()));
                                                } else {
                                                    #[cfg(windows)]
                                                    {
                                                        self.pending_webview_card =
                                                            Some("server_utility".to_string());
                                                    }
                                                    #[cfg(not(windows))]
                                                    {
                                                        self.current_module = Some(Box::new(
                                                            ServerLauncher::default(),
                                                        ));
                                                    }
                                                }
                                            }
                                            1 => {
                                                self.current_module =
                                                    Some(Box::new(DtaUnpacker::default()))
                                            }
                                            2 => {
                                                self.current_module =
                                                    Some(Box::new(InventoryEditor::default()))
                                            }
                                            3 => {
                                                self.current_module =
                                                    Some(Box::new(ItemsEditor::default()))
                                            }
                                            4 => {
                                                self.current_module =
                                                    Some(Box::new(MpmaplistEditor::default()))
                                            }
                                            5 => {
                                                self.current_module =
                                                    Some(Box::new(GamedataEditor::default()))
                                            }
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
                });
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
            fill = ui.visuals().extreme_bg_color;
            stroke = egui::Stroke::new(1.5, ui.visuals().widgets.inactive.bg_stroke.color);
        }

        ui.painter().rect_filled(rect, 8.0, fill);
        ui.painter()
            .rect_stroke(rect, 8.0, stroke, egui::StrokeKind::Inside);

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
                                egui::StrokeKind::Inside,
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

                            let arrowhead_left = arrow_tip
                                + egui::vec2(
                                    -arrowhead_size * (angle - std::f32::consts::PI * 0.4).cos(),
                                    -arrowhead_size * (angle - std::f32::consts::PI * 0.4).sin(),
                                );
                            let arrowhead_right = arrow_tip
                                + egui::vec2(
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

        let content_clicked = if is_ready {
            let id = ui.id().with("card_click").with(title);
            ui.interact(inner_rect, id, egui::Sense::click()).clicked()
        } else {
            false
        };

        if !is_ready {
            let stripe_color = egui::Color32::from_rgba_unmultiplied(255, 255, 255, 22);
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
                    ui.painter()
                        .line_segment([p1, p2], egui::Stroke::new(1.0, stripe_color));
                }
                x += stripe_spacing;
            }
        }

        let clicked = response.clicked() || inner.response.clicked() || content_clicked;

        is_ready && clicked
    }
}

impl eframe::App for SpectreApp {
    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        self.update_ui(ctx, Some(frame));
    }
}

impl egui_software_backend::App for SpectreApp {
    fn update(
        &mut self,
        ctx: &egui::Context,
        _backend: &mut egui_software_backend::SoftwareBackend,
    ) {
        self.update_ui(ctx, None);
    }
}

impl SpectreApp {
    fn update_ui(&mut self, ctx: &egui::Context, frame_opt: Option<&mut eframe::Frame>) {
        Self::apply_theme(ctx);
        ctx.request_repaint_after(Duration::from_millis(250));
        #[cfg(windows)]
        let frame_ref = frame_opt.as_deref();
        #[cfg(windows)]
        {
            if !self.background_timer_set {
                if let Some(hwnd) = get_main_window_hwnd_opt(frame_ref) {
                    use windows::Win32::UI::WindowsAndMessaging::SetTimer;
                    if unsafe { SetTimer(hwnd, 1, 500, None) } != 0 {
                        self.background_timer_set = true;
                    }
                }
            }
            if self.pending_hide_to_tray {
                if let Some(hwnd) = get_main_window_hwnd_opt(frame_ref) {
                    use windows::Win32::Foundation::RECT;
                    use windows::Win32::UI::WindowsAndMessaging::{
                        GetWindowRect, SetWindowPos, HWND_BOTTOM, SWP_NOACTIVATE,
                    };
                    let mut rect = RECT::default();
                    if unsafe { GetWindowRect(hwnd, &mut rect).is_ok() } {
                        let x = rect.left;
                        let y = rect.top;
                        let w = rect.right - rect.left;
                        let h = rect.bottom - rect.top;
                        self.saved_tray_rect = Some((x, y, w, h));
                        let _ = unsafe {
                            SetWindowPos(hwnd, HWND_BOTTOM, -32000, -32000, 1, 1, SWP_NOACTIVATE)
                        };
                        self.window_hidden_to_tray = true;
                    }
                }
                self.pending_hide_to_tray = false;
            }
            if self.window_hidden_to_tray {
                ctx.request_repaint_after(std::time::Duration::from_millis(500));
            }
            if self.splash_screen.is_none() && self.tray_icon.is_none() {
                if let Some(icon) = load_tray_icon() {
                    use tray_icon::menu::{Menu, MenuItem};
                    let show_item = MenuItem::with_id("show", "Show Spectre", true, None);
                    let show_id = show_item.id().clone();
                    let quit_item = MenuItem::with_id("quit", "Exit", true, None);
                    let quit_id = quit_item.id().clone();
                    let menu = Menu::new();
                    let _ = menu.append(&show_item);
                    let _ = menu.append(&quit_item);
                    match tray_icon::TrayIconBuilder::new()
                        .with_menu(Box::new(menu))
                        .with_tooltip("Spectre - HD2 toolkit")
                        .with_icon(icon)
                        .build()
                    {
                        Ok(tray) => {
                            self.tray_icon = Some(tray);
                            self.tray_show_id = Some(show_id);
                            self.tray_quit_id = Some(quit_id);
                        }
                        Err(e) => println!("[Tray] Failed to create tray icon: {}", e),
                    }
                }
            }
            while let Ok(event) = tray_icon::menu::MenuEvent::receiver().try_recv() {
                let is_show = self
                    .tray_show_id
                    .as_ref()
                    .is_some_and(|show_id| event.id.as_ref() == show_id.as_ref());
                if is_show {
                    self.window_hidden_to_tray = false;
                    if let Some(hwnd) = get_main_window_hwnd_opt(frame_ref) {
                        use windows::Win32::UI::WindowsAndMessaging::{
                            SetForegroundWindow, SetWindowPos, HWND_TOP, SWP_NOACTIVATE,
                        };
                        if let Some((x, y, w, h)) = self.saved_tray_rect.take() {
                            let _ =
                                unsafe { SetWindowPos(hwnd, HWND_TOP, x, y, w, h, SWP_NOACTIVATE) };
                        }
                        let _ = unsafe { SetForegroundWindow(hwnd) };
                    }
                    ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
                    ctx.request_repaint();
                } else {
                    // Exit: custom "quit" item or any other menu click (only Show and Exit in tray)
                    std::process::exit(0);
                }
            }
        }

        if ctx.data_mut(|d| {
            d.get_temp::<()>(egui::Id::new("spectre_open_web_after_wizard"))
                .is_some()
        }) {
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
        if frame_ref.is_some() {
            if let Some(card_name) = self.pending_webview_card.take() {
                self.webview_pending_creation = Some(card_name);
            }
        } else {
            let _ = self.pending_webview_card.take();
        }
        #[cfg(windows)]
        if frame_ref.is_some() {
            if let Some(card_name) = self.webview_pending_creation.take() {
                let initial_json = if card_name == "server_utility" {
                    let config_path = server_utility_config_path();
                    let path_exists = config_path.exists();
                    if path_exists {
                        println!(
                            "[Service] Server utility load: path={}",
                            config_path.display()
                        );
                    } else {
                        println!("[Service] Server utility: config file not found at {} (using defaults)", config_path.display());
                    }
                    let mut data =
                        spectre_core::server::ServerLauncherData::load_from_file(&config_path)
                            .unwrap_or_else(|e| {
                                println!("[Service] Load failed (using defaults): {}", e);
                                spectre_core::server::ServerLauncherData::default()
                            });
                    ensure_server_utility_has_defaults(&mut data);
                    if let Ok(pids) = self.server_pids.lock() {
                        for server in data.servers.iter_mut() {
                            server.running = pids.contains_key(&server.port);
                        }
                    }
                    for (i, server) in data.servers.iter_mut().enumerate() {
                        let maps = if server.mpmaplist_path.is_empty() {
                            std::collections::HashMap::new()
                        } else {
                            let path = std::path::Path::new(&server.mpmaplist_path);
                            let resolved = spectre_core::mpmaplist::resolve_mpmaplist_path(path);
                            let maps = spectre_core::mpmaplist::load_from_path(path);
                            let total: usize = maps.values().map(|v| v.len()).sum();
                            if total > 0 {
                                for (style, list) in &maps {
                                    println!(
                                        "[Service] mpmaplist server {} style {}: {} maps",
                                        i,
                                        style,
                                        list.len()
                                    );
                                }
                                println!(
                                    "[Service] mpmaplist server {} total: {} maps from {}",
                                    i,
                                    total,
                                    resolved.display()
                                );
                            } else if !server.mpmaplist_path.is_empty() {
                                println!(
                                    "[Service] mpmaplist server {}: no maps from {}",
                                    i,
                                    resolved.display()
                                );
                            }
                            maps
                        };
                        server.available_maps_by_style = maps;
                    }
                    match serde_json::to_value(&data) {
                        Ok(value) => match serde_json::to_string(&value) {
                            Ok(json) => {
                                let source = if path_exists { "from file" } else { "defaults" };
                                println!(
                                    "[Service] Initial state: {} servers, {} bytes ({})",
                                    data.servers.len(),
                                    json.len(),
                                    source
                                );
                                Some(json)
                            }
                            Err(e) => {
                                println!("[Service] Serialize initial state failed: {}", e);
                                None
                            }
                        },
                        Err(e) => {
                            println!("[Service] Serialize initial state failed: {}", e);
                            None
                        }
                    }
                } else {
                    None
                };
                let html_result = spectre_web::embedded_card_html(
                    &card_name,
                    initial_json.as_deref(),
                    cfg!(debug_assertions),
                );
                if let Ok(html) = html_result {
                    if let Some(ref frame) = frame_opt {
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
                        let shared_pids = self.server_pids.clone();
                        let shared_helper_kicked: Option<
                            Arc<Mutex<HashMap<u16, HashSet<String>>>>,
                        > = {
                            #[cfg(windows)]
                            {
                                Some(self.helper_kicked.clone())
                            }
                            #[cfg(not(windows))]
                            {
                                None
                            }
                        };
                        #[cfg(windows)]
                        let shared_helper_last_slots = self.helper_last_slots.clone();
                        let builder = wry::WebViewBuilder::new_as_child(frame)
                    .with_bounds(bounds)
                    .with_ipc_handler({
                        let config_path = config_path.clone();
                        let ipc_tx = ipc_tx.clone();
                        let shared_pids = shared_pids.clone();
                        let shared_helper_kicked = shared_helper_kicked.clone();
                        #[cfg(windows)]
                        let shared_helper_last_slots = shared_helper_last_slots.clone();
                        move |request: http::Request<String>| {
                            let body = request.body();
                            let t0 = Instant::now();
                            let perf = std::env::var("SPECTRE_PERF").is_ok();
                            if let Ok(ref msg) = serde_json::from_str::<IpcSaveMessage>(body) {
                                if msg.action != "get_players" && msg.action != "repaint" {
                                    println!("[Service] {} body_len={}", msg.action, body.len());
                                    let _ = std::io::stdout().flush();
                                }
                            }
                            match serde_json::from_str::<IpcSaveMessage>(body) {
                                Ok(msg) if msg.action == "save" => {
                                    println!("[Service] Save: {} servers", msg.servers.len());
                                    let mut data = spectre_core::server::ServerLauncherData::load_from_file(&config_path)
                                        .unwrap_or_else(|_| spectre_core::server::ServerLauncherData::default());
                                    data.servers = msg.servers;
                                    if let Some(sm) = msg.server_manager {
                                        data.server_manager = sm;
                                    }
                                    if let Some(parent) = config_path.parent() {
                                        let _ = std::fs::create_dir_all(parent);
                                    }
                                    let result = data.save_to_file(&config_path).map_err(|e| e.to_string());
                                    let status = if result.is_ok() {
                                        println!("[Service] Save OK -> {}", config_path.display());
                                        let mut data = spectre_core::server::ServerLauncherData::load_from_file(&config_path)
                                            .unwrap_or_else(|_| spectre_core::server::ServerLauncherData::default());
                                        ensure_server_utility_has_defaults(&mut data);
                                        if let Ok(pids) = shared_pids.lock() {
                                            for server in data.servers.iter_mut() {
                                                server.running = pids.contains_key(&server.port);
                                            }
                                        }
                                        for server in data.servers.iter_mut() {
                                            let maps = if server.mpmaplist_path.is_empty() {
                                                std::collections::HashMap::new()
                                            } else {
                                                let path = std::path::Path::new(&server.mpmaplist_path);
                                                spectre_core::mpmaplist::load_from_path(path)
                                            };
                                            server.available_maps_by_style = maps;
                                        }
                                        match serde_json::to_string(&data.servers) {
                                            Ok(json) => format!("STATE:{}", json),
                                            Err(_) => "Saved OK".to_string(),
                                        }
                                    } else {
                                        println!("[Service] Save failed: {:?}", result);
                                        result.unwrap_err()
                                    };
                                    if perf && t0.elapsed().as_millis() >= 1 {
                                        println!("[Spectre.dbg] IPC save took {} ms", t0.elapsed().as_millis());
                                    }
                                    let _ = ipc_tx.send(status);
                                }
                                Ok(msg) if msg.action == "start" => {
                                    let idx = msg.server_index.unwrap_or(0);
                                    match spectre_core::server::ServerLauncherData::load_from_file(&config_path) {
                                        Ok(_) => match msg.servers.get(idx).cloned() {
                                            Some(server) => {
                                                let ipc_tx_b = ipc_tx.clone();
                                                let pids_b = shared_pids.clone();
                                                std::thread::spawn(move || {
                                                    let result = spectre_core::ds_launch::start_ds(&server).map(|pid| (server.port, pid));
                                                    if let Ok((port, pid)) = &result {
                                                        if let Ok(mut pids) = pids_b.lock() {
                                                            pids.insert(*port, *pid);
                                                        }
                                                        println!("[Service] Start server {} OK (port {} pid {})", idx, port, pid);
                                                    } else {
                                                        println!("[Service] Start server failed: {:?}", result);
                                                    }
                                                    let status = result.map_or_else(|e| e, |_| "Started OK".to_string());
                                                    let _ = ipc_tx_b.send(status);
                                                });
                                            }
                                            None => {
                                                let _ = ipc_tx.send(format!("Invalid server index {}", idx));
                                            }
                                        },
                                        Err(e) => {
                                            let _ = ipc_tx.send(e);
                                        }
                                    }
                                }
                                Ok(msg) if msg.action == "browse_mpmaplist" => {
                                    let status = browse_mpmaplist_with_validation();
                                    let _ = ipc_tx.send(status);
                                }
                                Ok(msg) if msg.action == "browse_hd2_dir" => {
                                    let which = msg.browse_which.as_deref().unwrap_or("hd2ds");
                                    let status = browse_hd2_exe(which);
                                    let _ = ipc_tx.send(status);
                                }
                                Ok(msg) if msg.action == "refresh_mpmaplist" => {
                                    let mut servers = msg.servers;
                                    for server in servers.iter_mut() {
                                        let maps = if server.mpmaplist_path.trim().is_empty() {
                                            std::collections::HashMap::new()
                                        } else {
                                            let path = std::path::Path::new(&server.mpmaplist_path);
                                            spectre_core::mpmaplist::load_from_path(path)
                                        };
                                        server.available_maps_by_style = maps;
                                    }
                                    let status = match serde_json::to_string(&servers) {
                                        Ok(json) => format!("REFRESH:{}", json),
                                        Err(_) => "Refresh failed.".to_string(),
                                    };
                                    let _ = ipc_tx.send(status);
                                }
                                Ok(msg) if msg.action == "start_all" => {
                                    let pre = match spectre_core::server::ServerLauncherData::load_from_file(&config_path) {
                                        Ok(_) => Some(msg.servers.clone()),
                                        Err(e) => {
                                            let _ = ipc_tx.send(e);
                                            None
                                        }
                                    };
                                    if let Some(servers) = pre {
                                        let ipc_tx_b = ipc_tx.clone();
                                        let pids_b = shared_pids.clone();
                                        std::thread::spawn(move || {
                                            let mut errs = Vec::new();
                                            let mut started = Vec::new();
                                            for server in &servers {
                                                match spectre_core::ds_launch::start_ds(server) {
                                                    Ok(pid) => started.push((server.port, pid)),
                                                    Err(e) => errs.push(format!("{}: {}", server.name, e)),
                                                }
                                            }
                                            if let Ok(mut pids) = pids_b.lock() {
                                                for (port, pid) in started {
                                                    pids.insert(port, pid);
                                                }
                                            }
                                            if errs.is_empty() {
                                                println!("[Service] Start all servers OK");
                                            } else {
                                                println!("[Service] Start all had errors: {:?}", errs);
                                            }
                                            let status = if errs.is_empty() { "All servers started".to_string() } else { errs.join("; ") };
                                            let _ = ipc_tx_b.send(status);
                                        });
                                    }
                                }
                                Ok(msg) if msg.action == "stop" => {
                                    let idx = msg.server_index.unwrap_or(0);
                                    let status = match msg.servers.get(idx) {
                                        Some(server) => {
                                            let port = server.port;
                                            let mut pids = match shared_pids.lock() {
                                                Ok(g) => g,
                                                Err(_) => {
                                                    let _ = ipc_tx.send("Stop failed (lock)".to_string());
                                                    return;
                                                }
                                            };
                                            if let Some(&pid) = pids.get(&port) {
                                                pids.remove(&port);
                                                if let Some(ref k) = shared_helper_kicked {
                                                    let _ = k.lock().map(|mut m| m.remove(&port));
                                                }
                                                #[cfg(windows)]
                                                if let Ok(mut last) = shared_helper_last_slots.lock() {
                                                    last.remove(&port);
                                                }
                                                drop(pids);
                                                if kill_process_by_pid(pid) {
                                                    println!("[Service] Stopped server {} (port {} pid {})", idx, port, pid);
                                                    "Stopped OK".to_string()
                                                } else {
                                                    println!("[Service] Stop: process {} already gone", pid);
                                                    "Stopped OK".to_string()
                                                }
                                            } else {
                                                "Server not running".to_string()
                                            }
                                        }
                                        None => "Invalid server index".to_string(),
                                    };
                                    if perf && t0.elapsed().as_millis() >= 1 {
                                        println!("[Spectre.dbg] IPC stop took {} ms", t0.elapsed().as_millis());
                                    }
                                    let _ = ipc_tx.send(status);
                                }
                                Ok(msg) if msg.action == "stop_all" => {
                                    let mut pids = match shared_pids.lock() {
                                        Ok(g) => g,
                                        Err(_) => {
                                            let _ = ipc_tx.send("Stop all failed (lock)".to_string());
                                            return;
                                        }
                                    };
                                    let to_stop: Vec<(u16, u32)> = msg.servers.iter().filter_map(|s| pids.get(&s.port).copied().map(|pid| (s.port, pid))).collect();
                                    for (port, _) in &to_stop {
                                        pids.remove(port);
                                        if let Some(ref k) = shared_helper_kicked {
                                            let _ = k.lock().map(|mut m| m.remove(port));
                                        }
                                        #[cfg(windows)]
                                        if let Ok(mut last) = shared_helper_last_slots.lock() {
                                            last.remove(port);
                                        }
                                    }
                                    drop(pids);
                                    for (_, pid) in &to_stop {
                                        kill_process_by_pid(*pid);
                                    }
                                    println!("[Service] Stop all: {} processes", to_stop.len());
                                    let _ = ipc_tx.send("All servers stopped".to_string());
                                }
                                Ok(msg) if msg.action == "get_running" => {
                                    let ports: Vec<u16> = shared_pids.lock().map(|p| p.keys().copied().collect()).unwrap_or_default();
                                    let status = format!("RUNNING:{}", serde_json::to_string(&ports).unwrap_or_else(|_| "[]".to_string()));
                                    if perf && t0.elapsed().as_millis() >= 1 {
                                        println!("[Spectre.dbg] IPC get_running took {} ms", t0.elapsed().as_millis());
                                    }
                                    let _ = ipc_tx.send(status);
                                }
                                Ok(msg) if msg.action == "repaint" => {
                                    let _ = ipc_tx.send("REPAINT".to_string());
                                }
                                Ok(msg) if msg.action == "get_players" => {
                                    let idx = msg.server_index.unwrap_or(0);
                                    let (status, pid_opt) = match msg.servers.get(idx) {
                                        Some(server) => {
                                            let pid = shared_pids.lock().ok().and_then(|p| p.get(&server.port).copied());
                                            let max_clients = server
                                                .configs
                                                .iter()
                                                .find(|c| c.name == server.current_config)
                                                .map(|c| c.max_clients as u32)
                                                .unwrap_or(32);
                                            let status = match pid {
                                                Some(pid) => match ds_helper::get_player_count(pid, max_clients) {
                                                    Some((active, total)) => format!("PLAYERS:{},{}", active, total),
                                                    None => "PLAYERS:--,--".to_string(),
                                                },
                                                None => "PLAYERS:--,--".to_string(),
                                            };
                                            (status, pid)
                                        }
                                        None => ("PLAYERS:--,--".to_string(), None),
                                    };
                                    let _ = ipc_tx.send(status);
                                    let list_json = match pid_opt {
                                        Some(pid) => ds_helper::get_player_list(pid)
                                            .map(|list| {
                                                let arr: Vec<serde_json::Value> = list
                                                    .iter()
                                                    .map(|(n, i)| serde_json::json!({"name": n, "ip": i}))
                                                    .collect();
                                                serde_json::to_string(&arr).unwrap_or_else(|_| "[]".to_string())
                                            })
                                            .unwrap_or_else(|| "[]".to_string()),
                                        None => "[]".to_string(),
                                    };
                                    if perf && t0.elapsed().as_millis() >= 1 {
                                        println!("[Spectre.dbg] IPC get_players took {} ms", t0.elapsed().as_millis());
                                    }
                                    let _ = ipc_tx.send(format!("PLAYER_LIST:{}", list_json));
                                }
                                Ok(msg) if msg.action == "get_log_content" => {
                                    let path = app_log_path(&config_path);
                                    ensure_log_file_exists(&path);
                                    const MAX_LOG_BYTES: usize = 32 * 1024;
                                    let content = match std::fs::read(&path) {
                                        Ok(b) => {
                                            let start = b.len().saturating_sub(MAX_LOG_BYTES);
                                            String::from_utf8_lossy(&b[start..])
                                                .replace('\r', "")
                                                .replace('\0', "")
                                        }
                                        Err(_) => String::new(),
                                    };
                                    if perf && t0.elapsed().as_millis() >= 1 {
                                        println!("[Spectre.dbg] IPC get_log_content took {} ms", t0.elapsed().as_millis());
                                    }
                                    let _ = ipc_tx.send(format!("LOG_CONTENT:{}", content));
                                }
                                Ok(msg) if msg.action == "open_log_file" => {
                                    let path = app_log_path(&config_path);
                                    ensure_log_file_exists(&path);
                                    let abs_path = std::fs::canonicalize(&path).unwrap_or_else(|_| path.clone());
                                    let mut path_str = abs_path.display().to_string();
                                    if path_str.starts_with(r"\\?\") {
                                        path_str = path_str[r"\\?\".len()..].to_string();
                                    }
                                    let folder = std::path::Path::new(&path_str).parent().map(|p| p.to_path_buf()).unwrap_or_else(|| path.clone());
                                    let folder_str = folder.display().to_string();
                                    println!("[Log] open_log_file: path={} folder={}", path.display(), folder_str);
                                    let _ = std::process::Command::new("explorer").arg(&folder_str).spawn();
                                    let _ = ipc_tx.send("OK".to_string());
                                }
                                Ok(_) => {}
                                Err(e) => {
                                    println!("[Service] Parse postMessage failed: {}", e);
                                    let _ = ipc_tx.send(format!("Error: {}", e));
                                }
                            }
                            let _ = std::io::stdout().flush();
                        }
                    })
                    .with_devtools({
                        cfg!(debug_assertions)
                    })
                    .with_html(&html);
                        match builder.build() {
                            Ok(wv) => {
                                self.webview = Some(wv);
                                self.webview_fade_alpha = 1.0;
                                self.ipc_save_rx = Some(ipc_rx);
                                if card_name == "server_utility" {
                                    let log_path = app_log_path(&config_path);
                                    ensure_log_file_exists(&log_path);
                                    let rotation_days =
                                        spectre_core::server::ServerLauncherData::load_from_file(
                                            &config_path,
                                        )
                                        .map(|d| d.server_manager.log_rotation_days)
                                        .unwrap_or(0);
                                    self.log_state =
                                        Some(Arc::new(Mutex::new((log_path, rotation_days))));
                                }
                            }
                            Err(e) => {
                                let msg = format!(
                            "Failed to create WebView: {}.\n\nIf the app just started, the WebView2 runtime may be missing. Install it from: https://go.microsoft.com/fwlink/p/?LinkId=2124703",
                            e
                        );
                                self.card_launch_error = Some(msg);
                            }
                        }
                    }
                } else {
                    self.card_launch_error = Some("Card not found.".to_string());
                }
            }
        }

        #[cfg(windows)]
        if let Some(ref wv) = self.webview {
            let scale = ctx
                .input(|i| i.viewport().native_pixels_per_point)
                .unwrap_or(1.0);
            let screen = ctx.screen_rect();
            const ACTION_BAR_HEIGHT: f32 = 32.0;
            let h = ((screen.height() - ACTION_BAR_HEIGHT) * scale).max(1.0) as u32;
            let bounds = wry::Rect {
                x: 0,
                y: (ACTION_BAR_HEIGHT * scale) as i32,
                width: (screen.width() * scale) as u32,
                height: h,
            };
            let _ = wv.set_bounds(bounds);
        }

        #[cfg(windows)]
        if let Some(ref rx) = self.ipc_save_rx {
            fn is_critical(msg: &str) -> bool {
                msg == "REPAINT"
                    || msg == "Stopped OK"
                    || msg == "All servers stopped"
                    || msg == "Started OK"
                    || msg == "All servers started"
                    || msg == "Saved OK"
                    || msg.starts_with("STATE:")
            }
            let perf = std::env::var("SPECTRE_PERF").is_ok();
            let t_drain = Instant::now();
            let mut critical = Vec::new();
            let mut other = Vec::new();
            while let Ok(m) = rx.try_recv() {
                if is_critical(&m) {
                    critical.push(m);
                } else {
                    other.push(m);
                }
            }
            let n_msg = critical.len() + other.len();
            if perf && n_msg > 0 {
                println!(
                    "[Spectre.dbg] IPC drain: {} ms to collect {} messages ({} critical, {} other)",
                    t_drain.elapsed().as_millis(),
                    n_msg,
                    critical.len(),
                    other.len()
                );
            }
            let coalesced: Vec<String> = {
                let mut by_type: HashMap<String, String> = HashMap::new();
                for msg in other {
                    let key = msg
                        .find(':')
                        .map(|i| msg[..i].to_string())
                        .unwrap_or_else(|| msg.clone());
                    by_type.insert(key, msg);
                }
                by_type.into_values().collect()
            };
            let n_eval = critical.len() + coalesced.len();
            for status_msg in &critical {
                if status_msg == "REPAINT" {
                    self.webview_repaint_frames = 10;
                    ctx.request_repaint();
                } else {
                    let script = format!(
                        "window.__spectreIpcStatus && window.__spectreIpcStatus({});",
                        serde_json::to_string(status_msg)
                            .unwrap_or_else(|_| "window.__spectreIpcStatus('OK')".to_string())
                    );
                    if let Some(ref wv) = self.webview {
                        if let Err(e) = wv.evaluate_script(&script) {
                            println!("[Service] evaluate_script status failed: {}", e);
                        }
                    }
                    self.webview_repaint_frames = 15;
                    ctx.request_repaint();
                }
            }
            for status_msg in &coalesced {
                let script = format!(
                    "window.__spectreIpcStatus && window.__spectreIpcStatus({});",
                    serde_json::to_string(status_msg)
                        .unwrap_or_else(|_| "window.__spectreIpcStatus('OK')".to_string())
                );
                if let Some(ref wv) = self.webview {
                    if let Err(e) = wv.evaluate_script(&script) {
                        println!("[Service] evaluate_script status failed: {}", e);
                    }
                }
                self.webview_repaint_frames = 3;
                ctx.request_repaint();
            }
            if perf && n_msg > 0 {
                if n_eval < n_msg {
                    println!(
                        "[Spectre.dbg] IPC drain: total {} ms ({} messages -> {} eval scripts)",
                        t_drain.elapsed().as_millis(),
                        n_msg,
                        n_eval
                    );
                } else {
                    println!(
                        "[Spectre.dbg] IPC drain: total {} ms (eval scripts)",
                        t_drain.elapsed().as_millis()
                    );
                }
            }
        }

        #[cfg(windows)]
        if self.webview_repaint_frames > 0 {
            self.webview_repaint_frames = self.webview_repaint_frames.saturating_sub(1);
            ctx.request_repaint();
            // Invalidate webview every frame so we repaint after async script updates DOM (unsaved indicator, etc.).
            if let Some(hwnd) = get_webview_hwnd_opt(frame_ref) {
                use windows::Win32::Foundation::BOOL;
                use windows::Win32::Graphics::Gdi::InvalidateRect;
                let _ = unsafe { InvalidateRect(hwnd, None, BOOL(1)) };
            }
        }

        #[cfg(windows)]
        {
            let now = Instant::now();
            let should_run = self
                .last_watchdog_check
                .map_or(true, |t| now.duration_since(t) >= Duration::from_secs(5));
            if should_run {
                self.last_watchdog_check = Some(now);
                let config_path = server_utility_config_path();
                if let Ok(data) =
                    spectre_core::server::ServerLauncherData::load_from_file(&config_path)
                {
                    if data.server_manager.enable_watchdog {
                        let dead_ports: Vec<u16> = match self.server_pids.lock() {
                            Ok(pids) => pids
                                .iter()
                                .filter(|(_, &pid)| !process_is_alive(pid))
                                .map(|(&port, _)| port)
                                .collect(),
                            Err(_) => Vec::new(),
                        };
                        if !dead_ports.is_empty() {
                            if let Ok(mut pids) = self.server_pids.lock() {
                                for port in &dead_ports {
                                    pids.remove(port);
                                }
                            }
                            #[cfg(windows)]
                            if let Ok(mut k) = self.helper_kicked.lock() {
                                for port in &dead_ports {
                                    k.remove(port);
                                }
                            }
                            #[cfg(windows)]
                            if let Ok(mut last) = self.helper_last_slots.lock() {
                                for port in &dead_ports {
                                    last.remove(port);
                                }
                            }
                            for port in dead_ports {
                                if let Some(server) = data.servers.iter().find(|s| s.port == port) {
                                    match spectre_core::ds_launch::start_ds(server) {
                                        Ok(pid) => {
                                            if let Ok(mut pids) = self.server_pids.lock() {
                                                pids.insert(port, pid);
                                            }
                                            println!(
                                                "[Watchdog] Restarted server port {} (pid {})",
                                                port, pid
                                            );
                                        }
                                        Err(e) => println!(
                                            "[Watchdog] Restart port {} failed: {}",
                                            port, e
                                        ),
                                    }
                                }
                            }
                        }
                    }
                    if data.server_manager.restart_interval_days > 0 && !data.servers.is_empty() {
                        let last_restart_path = config_path
                            .parent()
                            .map(|p| p.join("last_restart.txt"))
                            .unwrap_or_else(|| std::path::PathBuf::from("last_restart.txt"));
                        let now_secs = std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap_or(Duration::ZERO)
                            .as_secs();
                        let do_restart = match std::fs::read_to_string(&last_restart_path) {
                            Ok(s) => {
                                let then: u64 = s.trim().parse().unwrap_or(0);
                                then > 0
                                    && now_secs >= then
                                    && (now_secs - then) / 86400
                                        >= data.server_manager.restart_interval_days as u64
                            }
                            Err(_) => true,
                        };
                        if do_restart {
                            let to_kill: Vec<(u16, u32)> = match self.server_pids.lock() {
                                Ok(pids) => data
                                    .servers
                                    .iter()
                                    .filter_map(|s| {
                                        pids.get(&s.port).copied().map(|pid| (s.port, pid))
                                    })
                                    .collect(),
                                Err(_) => Vec::new(),
                            };
                            if let Ok(mut pids) = self.server_pids.lock() {
                                for (port, _) in &to_kill {
                                    pids.remove(port);
                                }
                            }
                            #[cfg(windows)]
                            if let Ok(mut k) = self.helper_kicked.lock() {
                                for (port, _) in &to_kill {
                                    k.remove(port);
                                }
                            }
                            #[cfg(windows)]
                            if let Ok(mut last) = self.helper_last_slots.lock() {
                                for (port, _) in &to_kill {
                                    last.remove(port);
                                }
                            }
                            for (_, pid) in &to_kill {
                                kill_process_by_pid(*pid);
                            }
                            std::thread::sleep(Duration::from_secs(2));
                            for server in &data.servers {
                                if let Ok(pid) = spectre_core::ds_launch::start_ds(server) {
                                    if let Ok(mut pids) = self.server_pids.lock() {
                                        pids.insert(server.port, pid);
                                    }
                                    println!(
                                        "[Watchdog] Timed restart: started {} (port {} pid {})",
                                        server.name, server.port, pid
                                    );
                                }
                                std::thread::sleep(Duration::from_millis(500));
                            }
                            let _ = std::fs::write(&last_restart_path, now_secs.to_string());
                        }
                    }
                    let pids_copy: Vec<(u16, u32)> = match self.server_pids.lock() {
                        Ok(pids) => pids.iter().map(|(&port, &pid)| (port, pid)).collect(),
                        Err(_) => Vec::new(),
                    };
                    for (port, pid) in pids_copy {
                        let server = match data.servers.iter().find(|s| s.port == port) {
                            Some(s) => s,
                            None => continue,
                        };
                        let config = match server
                            .configs
                            .iter()
                            .find(|c| c.name == server.current_config)
                        {
                            Some(c) => c,
                            None => match server.configs.first() {
                                Some(c) => {
                                    println!(
                                        "[Daemon] port {}: no profile \"{}\", using \"{}\"",
                                        port, server.current_config, c.name
                                    );
                                    let _ = std::io::stdout().flush();
                                    c
                                }
                                None => continue,
                            },
                        };
                        let mut kicked = {
                            if let Ok(kicked_map) = self.helper_kicked.lock() {
                                kicked_map.get(&port).cloned().unwrap_or_default()
                            } else {
                                continue;
                            }
                        };
                        let previous_slots = self
                            .helper_last_slots
                            .lock()
                            .ok()
                            .and_then(|m| m.get(&port).cloned());
                        let log_state = self.log_state.clone();
                        let log_callback = move |line: &str| {
                            if let Some(ref state) = log_state {
                                write_app_log(state, line);
                            }
                        };
                        let log_ref: Option<&dyn Fn(&str)> = Some(&log_callback);
                        match ds_helper::enforce_player_lists(
                            pid,
                            port,
                            config,
                            &data.server_manager,
                            &mut kicked,
                            previous_slots.as_deref(),
                            log_ref,
                            server.use_sabre_squadron,
                        ) {
                            Ok(current_slots) => {
                                if let Ok(mut last) = self.helper_last_slots.lock() {
                                    last.insert(port, current_slots);
                                }
                            }
                            Err(e) => {
                                let line = format!("[DS-Helper] port {}: {}", port, e);
                                println!("{}", line);
                                if let Some(ref state) = self.log_state {
                                    write_app_log(state, &line);
                                }
                            }
                        }
                        if let Ok(mut kicked_map) = self.helper_kicked.lock() {
                            kicked_map.insert(port, kicked);
                        }
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
                    println!("[Spectre.dbg] Centering splash window (attempt {}): monitor={}x{}, window={}x{}, pos=({}, {})",
                        self.center_attempts, monitor_size.x, monitor_size.y, splash_size.0, splash_size.1, center_x, center_y);
                }

                ctx.send_viewport_cmd(egui::ViewportCommand::OuterPosition(egui::pos2(
                    center_x.max(0.0),
                    center_y.max(0.0),
                )));

                if self.center_attempts >= 3 {
                    self.window_centered = true;
                    println!("[Spectre.dbg] Splash window centering complete");
                }
            }
        }

        if let Some(ref mut splash) = self.splash_screen {
            if !splash.show(ctx) {
                println!("[Spectre.dbg] Splash screen finished, transitioning to main application");
                self.splash_screen = None;
                ctx.send_viewport_cmd(egui::ViewportCommand::Decorations(true));
                let is_fullscreen = self.config.fullscreen_dialogs;
                if is_fullscreen {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Maximized(true));
                    println!("[Spectre.dbg] Application set to windowed fullscreen (maximized)");
                } else {
                    const APP_WINDOW_SIZE: (f32, f32) = (1280.0, 1000.0);
                    const MIN_WINDOW_SIZE: (f32, f32) = (640.0, 480.0);
                    let monitor_size = ctx.input(|i| i.viewport().monitor_size)
                        .or_else(|| Some(ctx.screen_rect().size()));
                    let (w, h) = if let Some(ref m) = monitor_size {
                        let max_w = (m.x * 0.95).max(MIN_WINDOW_SIZE.0);
                        let max_h = (m.y * 0.95).max(MIN_WINDOW_SIZE.1);
                        (
                            APP_WINDOW_SIZE.0.min(max_w),
                            APP_WINDOW_SIZE.1.min(max_h),
                        )
                    } else {
                        APP_WINDOW_SIZE
                    };
                    ctx.send_viewport_cmd(egui::ViewportCommand::Maximized(false));
                    ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(egui::vec2(w, h)));
                    println!("[Spectre.dbg] Application window resized to {}x{} with decorations enabled", w, h);

                    if let Some(monitor_size) = monitor_size {
                        let center_x = (monitor_size.x - w) / 2.0;
                        let center_y = (monitor_size.y - h) / 2.0;
                        ctx.send_viewport_cmd(egui::ViewportCommand::OuterPosition(egui::pos2(
                            center_x.max(0.0), center_y.max(0.0),
                        )));
                        println!("[Spectre.dbg] Main window re-centered at: ({}, {})", center_x, center_y);
                    } else {
                        let screen_size = ctx.screen_rect().size();
                        let center_x = (screen_size.x - w) / 2.0;
                        let center_y = (screen_size.y - h) / 2.0;
                        ctx.send_viewport_cmd(egui::ViewportCommand::OuterPosition(egui::pos2(
                            center_x.max(0.0), center_y.max(0.0),
                        )));
                        println!("[Spectre.dbg] Main window re-centered (fallback) at: ({}, {})", center_x, center_y);
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
                            println!("[Spectre.dbg] Application set to windowed fullscreen (maximized)");
                        } else {
                            const APP_WINDOW_SIZE: (f32, f32) = (1280.0, 1000.0);
                            const MIN_WINDOW_SIZE: (f32, f32) = (640.0, 480.0);
                            let monitor_size = ctx.input(|i| i.viewport().monitor_size)
                                .or_else(|| Some(ctx.screen_rect().size()));
                            let (w, h) = if let Some(ref m) = monitor_size {
                                let max_w = (m.x * 0.95).max(MIN_WINDOW_SIZE.0);
                                let max_h = (m.y * 0.95).max(MIN_WINDOW_SIZE.1);
                                (APP_WINDOW_SIZE.0.min(max_w), APP_WINDOW_SIZE.1.min(max_h))
                            } else {
                                APP_WINDOW_SIZE
                            };
                            ctx.send_viewport_cmd(egui::ViewportCommand::Maximized(false));
                            ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(egui::vec2(w, h)));
                            if let Some(monitor_size) = monitor_size {
                                let center_x = (monitor_size.x - w) / 2.0;
                                let center_y = (monitor_size.y - h) / 2.0;
                                ctx.send_viewport_cmd(egui::ViewportCommand::OuterPosition(egui::pos2(center_x.max(0.0), center_y.max(0.0))));
                            } else {
                                let screen_size = ctx.screen_rect().size();
                                let center_x = (screen_size.x - w) / 2.0;
                                let center_y = (screen_size.y - h) / 2.0;
                                ctx.send_viewport_cmd(egui::ViewportCommand::OuterPosition(egui::pos2(center_x.max(0.0), center_y.max(0.0))));
                            }
                            println!("[Spectre.dbg] Application restored to windowed mode ({}x{}, centered)", w, h);
                        }
                        self.config.save();
                    }

                    ui.add_space(15.0);
                    ui.separator();

                    if ui.button("Close").clicked() {
                        self.config.save();
                        println!("[Spectre.dbg] Options dialog closed");
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
                                println!("[Spectre.dbg] About dialog closed");
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
                    ui.label(
                        "• WebView2 runtime may be missing or failed to create the embedded view.",
                    );
                    ui.add_space(12.0);
                    egui::ScrollArea::vertical()
                        .max_height(80.0)
                        .show(ui, |ui| {
                            ui.label(
                                egui::RichText::new(msg.as_str())
                                    .color(ui.visuals().error_fg_color),
                            );
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

        #[cfg(windows)]
        if let Some(ref wv) = self.webview {
            const FADE_SPEED: f32 = 4.0;
            let any_modal =
                self.show_options || self.show_about || self.card_launch_error.is_some();
            let dt = ctx.input(|i| i.unstable_dt).max(0.0).min(0.1);
            if let Some(hwnd) = get_webview_hwnd_opt(frame_ref) {
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
                        self.webview_fade_alpha =
                            (self.webview_fade_alpha + dt * FADE_SPEED).min(1.0);
                        set_webview_opacity(hwnd, self.webview_fade_alpha);
                        ctx.request_repaint();
                    }
                    if self.webview_fade_alpha >= 1.0 {
                        set_webview_opacity(hwnd, 1.0);
                    }
                }
            } else {
                let show_webview = !any_modal;
                let _ = wv.set_visible(show_webview);
                self.webview_fade_alpha = if show_webview { 1.0 } else { 0.0 };
            }
        }

        egui::CentralPanel::default()
            .frame(egui::Frame::NONE.fill(ctx.style().visuals.extreme_bg_color))
            .show(ctx, |ui| {
                #[cfg(windows)]
                if self.webview_pending_creation.is_some() {
                    self.show_action_bar(ui, true);
                    ui.allocate_ui_at_rect(ui.available_rect_before_wrap(), |ui| {
                        ui.vertical_centered(|ui| {
                            ui.spinner();
                            ui.add_space(8.0);
                            ui.label(
                                egui::RichText::new("Loading…")
                                    .color(ui.visuals().weak_text_color()),
                            );
                        });
                    });
                    return;
                }
                #[cfg(windows)]
                if self.webview.is_some() {
                    let any_modal =
                        self.show_options || self.show_about || self.card_launch_error.is_some();
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

        if ctx.data_mut(|d| d.get_temp::<()>(egui::Id::new("spectre_go_home")).is_some()) {
            ctx.data_mut(|d| d.remove::<()>(egui::Id::new("spectre_go_home")));
            #[cfg(windows)]
            if self.webview.is_some() {
                self.webview = None;
            }
            self.current_module = None;
            self.config = Config::load();
        }

        // Keep updating when window is not focused (e.g. on second monitor) so server status,
        // player count, and webview content stay current.
        ctx.request_repaint_after(Duration::from_millis(250));
    }
}
