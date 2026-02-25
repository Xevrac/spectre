//! HD2 DS helper: read player list from process memory, enforce ban/whitelist via console commands.

#![cfg(windows)]

use spectre_core::server::{ServerConfig, ServerManager};
use std::collections::HashSet;
use std::io::Write;
use windows::Win32::Foundation::{CloseHandle, HANDLE, LPARAM, WPARAM};
use windows::Win32::System::Diagnostics::Debug::ReadProcessMemory;
use windows::Win32::System::Threading::{OpenProcess, PROCESS_QUERY_INFORMATION, PROCESS_VM_READ};
use windows::Win32::UI::Input::KeyboardAndMouse::VK_RETURN;
use windows::Win32::UI::WindowsAndMessaging::{
    EnumWindows, GetWindowTextW, GetWindowThreadProcessId, PostMessageW, SetForegroundWindow,
    WM_CHAR, WM_KEYDOWN,
};

const PLAYER_BUFFER_POINTER_ADDR: u32 = 0x009D6A4C + 4;
const SLOT_COUNT: usize = 32;
const SLOT_STRIDE: usize = 196;
const SLOT_IP_OFFSET: usize = 4;
const SLOT_NAME_OFFSET: usize = 8;
const NAME_MAX: usize = SLOT_STRIDE - SLOT_NAME_OFFSET;

/// Main/console window for a process by PID; prefers title containing "Console".
pub fn find_main_window_by_pid(pid: u32) -> Option<windows::Win32::Foundation::HWND> {
    if pid == 0 {
        return None;
    }
    let mut found = None;
    unsafe {
        enum_windows_with_pid(pid, &mut found);
    }
    found
}

#[allow(non_upper_case_globals)]
static mut g_enum_pid: u32 = 0;
const MAX_WINDOWS: usize = 16;
#[allow(non_upper_case_globals)]
static mut g_enum_hwnds: [Option<windows::Win32::Foundation::HWND>; MAX_WINDOWS] = [None; MAX_WINDOWS];
#[allow(non_upper_case_globals)]
static mut g_enum_count: usize = 0;

unsafe fn enum_windows_with_pid(pid: u32, result: &mut Option<windows::Win32::Foundation::HWND>) {
    g_enum_pid = pid;
    g_enum_count = 0;
    let _ = EnumWindows(Some(enum_callback), LPARAM(0));
    let count = g_enum_count;
    let hwnds: Vec<_> = g_enum_hwnds[..count].iter().filter_map(|o| *o).collect();
    *result = pick_best_window(&hwnds);
}

fn get_window_title(hwnd: windows::Win32::Foundation::HWND) -> String {
    let mut buf = [0u16; 260];
    let len = unsafe { GetWindowTextW(hwnd, &mut buf) };
    if len <= 0 {
        return String::new();
    }
    String::from_utf16_lossy(&buf[..(len as usize).min(buf.len())])
}

fn pick_best_window(hwnds: &[windows::Win32::Foundation::HWND]) -> Option<windows::Win32::Foundation::HWND> {
    let mut fallback = None;
    for &hwnd in hwnds {
        if hwnd.0.is_null() {
            continue;
        }
        if fallback.is_none() {
            fallback = Some(hwnd);
        }
        let s = get_window_title(hwnd).to_uppercase();
        if s.contains("CONSOLE") {
            return Some(hwnd);
        }
        if s.contains("SERVER") {
            fallback = Some(hwnd);
        }
    }
    fallback
}

unsafe extern "system" fn enum_callback(
    hwnd: windows::Win32::Foundation::HWND,
    _lparam: windows::Win32::Foundation::LPARAM,
) -> windows::Win32::Foundation::BOOL {
    let mut window_pid: u32 = 0;
    GetWindowThreadProcessId(hwnd, Some(&mut window_pid));
    if window_pid != g_enum_pid {
        return windows::Win32::Foundation::BOOL(1);
    }
    if g_enum_count < MAX_WINDOWS {
        g_enum_hwnds[g_enum_count] = Some(hwnd);
        g_enum_count += 1;
    }
    windows::Win32::Foundation::BOOL(1)
}

/// Types a command into the DS console window (PostMessage WM_CHAR + Enter).
pub fn send_command_to_ds(hwnd: windows::Win32::Foundation::HWND, command: &str) {
    let _ = unsafe { SetForegroundWindow(hwnd) };
    std::thread::sleep(std::time::Duration::from_millis(120));
    for ch in command.chars() {
        let code = ch as u32;
        if code <= 0xFFFF {
            let _ = unsafe { PostMessageW(hwnd, WM_CHAR, WPARAM(code as _), LPARAM(0)) };
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
    }
    let _ = unsafe {
        PostMessageW(
            hwnd,
            WM_KEYDOWN,
            WPARAM(VK_RETURN.0 as _),
            LPARAM(0),
        )
    };
    std::thread::sleep(std::time::Duration::from_millis(60));
}

pub fn get_player_count(pid: u32, max_clients: u32) -> Option<(u32, u32)> {
    if pid == 0 {
        return None;
    }
    let access = PROCESS_VM_READ | PROCESS_QUERY_INFORMATION;
    let handle = unsafe { OpenProcess(access, false, pid) }.ok()?;
    let slots = read_player_slots(handle)?;
    let _ = unsafe { CloseHandle(handle) };
    let active = slots.iter().filter(|(name, _)| !name.is_empty()).count() as u32;
    Some((active, max_clients))
}

pub fn get_player_list(pid: u32) -> Option<Vec<(String, String)>> {
    if pid == 0 {
        return None;
    }
    let access = PROCESS_VM_READ | PROCESS_QUERY_INFORMATION;
    let handle = unsafe { OpenProcess(access, false, pid) }.ok()?;
    let slots = read_player_slots(handle)?;
    let _ = unsafe { CloseHandle(handle) };
    let list: Vec<(String, String)> = slots
        .into_iter()
        .filter(|(name, _)| !name.is_empty())
        .collect();
    Some(list)
}

pub fn read_player_slots(process_handle: HANDLE) -> Option<Vec<(String, String)>> {
    let mut ptr_buf: [u8; 4] = [0; 4];
    let read_ok = unsafe {
        ReadProcessMemory(
            process_handle,
            PLAYER_BUFFER_POINTER_ADDR as *const _,
            ptr_buf.as_mut_ptr() as *mut _,
            4,
            None,
        )
    };
    if read_ok.is_err() {
        return None;
    }
    let base_ptr = u32::from_le_bytes(ptr_buf);
    if base_ptr == 0 {
        return None;
    }
    let mut buffer = vec![0u8; SLOT_COUNT * SLOT_STRIDE];
    let read_ok = unsafe {
        ReadProcessMemory(
            process_handle,
            base_ptr as *const _,
            buffer.as_mut_ptr() as *mut _,
            buffer.len(),
            None,
        )
    };
    if read_ok.is_err() {
        return None;
    }
    let mut slots = Vec::with_capacity(SLOT_COUNT);
    for i in 0..SLOT_COUNT {
        let base = i * SLOT_STRIDE;
        let ip_bytes: [u8; 4] = buffer[base + SLOT_IP_OFFSET..base + SLOT_IP_OFFSET + 4]
            .try_into()
            .unwrap_or([0, 0, 0, 0]);
        let ip = format!("{}.{}.{}.{}", ip_bytes[0], ip_bytes[1], ip_bytes[2], ip_bytes[3]);
        let name_start = base + SLOT_NAME_OFFSET;
        let name_end = (name_start + NAME_MAX).min(buffer.len());
        let name_slice = &buffer[name_start..name_end];
        let nul = name_slice.iter().position(|&b| b == 0).unwrap_or(name_slice.len());
        let name = String::from_utf8_lossy(&name_slice[..nul]).trim().to_string();
        slots.push((name, ip));
    }
    Some(slots)
}

fn entry_ip(entry: &str) -> &str {
    if let Some(pos) = entry.find(":>") {
        entry[..pos].trim()
    } else {
        entry.trim()
    }
}

fn entry_comment(entry: &str) -> Option<&str> {
    entry.find(":>").map(|pos| entry[pos + 2..].trim()).filter(|s| !s.is_empty())
}

pub const ASA_MAX_LEN: usize = 43;
pub const BAN_REASON_MAX_LEN: usize = 21;

fn asay_message_for_kick(player_name: &str, kick_reason: &str, matching_entry: Option<&str>) -> String {
    let name = player_name.trim();
    let msg = if kick_reason == "not in whitelist" {
        format!("{} not in whitelist.", name)
    } else {
        let reason = matching_entry.and_then(entry_comment).unwrap_or("(none)");
        let reason_trim = reason.chars().take(BAN_REASON_MAX_LEN).collect::<String>();
        format!("{} is banned. Reason: {}", name, reason_trim)
    };
    msg.chars().take(ASA_MAX_LEN).collect()
}

/// Enforces ban/whitelist: kicks matching players, sends asay then kickplayer via console.
pub fn enforce_player_lists(
    pid: u32,
    port: u16,
    config: &ServerConfig,
    manager: &ServerManager,
    kicked: &mut HashSet<String>,
    previous_slots: Option<&[(String, String)]>,
    log_line: Option<&dyn Fn(&str)>,
    _use_sabre_squadron: bool,
) -> Result<Vec<(String, String)>, String> {
    let access = PROCESS_VM_READ | PROCESS_QUERY_INFORMATION;
    let handle = unsafe { OpenProcess(access, false, pid) }
        .map_err(|e| format!("OpenProcess: {}", e))?;
    let slots = match read_player_slots(handle) {
        Some(s) => s,
        None => {
            let _ = unsafe { CloseHandle(handle) };
            return Err("ReadProcessMemory failed".to_string());
        }
    };
    let _ = unsafe { CloseHandle(handle) };

    let current_connected: Vec<(String, String)> = slots
        .iter()
        .filter(|(n, _)| !n.is_empty())
        .cloned()
        .collect();

    let previous_set: HashSet<(String, String)> = previous_slots
        .map(|s| s.iter().cloned().collect())
        .unwrap_or_default();

    for (name, ip) in &current_connected {
        if !previous_set.contains(&(name.clone(), ip.clone())) {
            let msg = format!("[DS-Helper] player joined: \"{}\" ({})", name, ip);
            println!("{}", msg);
            let _ = std::io::stdout().flush();
            if let Some(log) = log_line {
                log(&msg);
            }
        }
    }

    let current_names: HashSet<String> = current_connected.iter().map(|(n, _)| n.clone()).collect();
    kicked.retain(|name| current_names.contains(name));

    let should_do_forced_ban = manager.enable_forced_ban_list && !manager.forced_ban_list.is_empty();
    let should_do_ban = !config.ban_list.is_empty();
    let should_do_whitelist = config.enable_whitelist;

    if !should_do_forced_ban && !should_do_ban && !should_do_whitelist {
        return Ok(current_connected);
    }

    let hwnd = find_main_window_by_pid(pid);
    if hwnd.is_none() {
        let msg = format!("[DS-Helper] port {}: Could not find DS window (kick command will not be sent)", port);
        println!("{}", msg);
        if let Some(log) = log_line {
            log(&msg);
        }
    }

    if should_do_ban && !current_connected.is_empty() {
        let msg = format!(
            "[DS-Helper] port {} ban_list has {} entries (first: {:?})",
            port,
            config.ban_list.len(),
            config.ban_list.first().map(|s| s.as_str())
        );
        println!("{}", msg);
        let _ = std::io::stdout().flush();
        if let Some(log) = log_line {
            log(&msg);
        }
    }

    for (slot_index, (name, ip)) in slots.into_iter().enumerate() {
        if name.is_empty() {
            continue;
        }
        if kicked.contains(&name) {
            continue;
        }

        let ip_trimmed = ip.trim();

        let mut should_kick = false;
        let mut kick_reason = String::new();
        let mut matching_entry: Option<&str> = None;

        if should_do_forced_ban {
            for entry in &manager.forced_ban_list {
                if ip_trimmed == entry_ip(entry) {
                    should_kick = true;
                    kick_reason = format!("forced_ban list (entry: {})", entry);
                    matching_entry = Some(entry);
                    break;
                }
            }
        }
        if !should_kick && should_do_ban {
            for entry in &config.ban_list {
                if ip_trimmed == entry_ip(entry) {
                    should_kick = true;
                    kick_reason = format!("ban list (entry: {})", entry);
                    matching_entry = Some(entry);
                    break;
                }
            }
        }
        if !should_kick && should_do_whitelist {
            let in_whitelist = config.whitelist.iter().any(|e| ip_trimmed == entry_ip(e));
            if !in_whitelist {
                should_kick = true;
                kick_reason = "not in whitelist".to_string();
            }
        }

        if should_kick {
            let msg = format!(
                "[DS-Helper] KICK slot {} \"{}\" ({}) reason: {}",
                slot_index, name, ip_trimmed, kick_reason
            );
            println!("{}", msg);
            let _ = std::io::stdout().flush();
            if let Some(log) = log_line {
                log(&msg);
            }
            let asay_msg = asay_message_for_kick(&name, &kick_reason, matching_entry);
            if let Some(h) = hwnd {
                send_command_to_ds(h, &format!("asay {}", asay_msg));
                std::thread::sleep(std::time::Duration::from_millis(400));
                let cmd = format!("kickplayer {}", name.trim());
                send_command_to_ds(h, &cmd);
                kicked.insert(name);
            }
        }
    }

    Ok(current_connected)
}
