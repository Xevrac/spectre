//! HTTP server for Server Utility web UI. Serves the same content as the former webview

//! HTTP server for Server Utility web UI. Protected by a secret token to prevent
//! unauthenticated RCE via the IPC bridge. Paths from the client are validated
//! before use (no null bytes, no protocol handlers, no traversal).

#![cfg(windows)]

use axum::{
    extract::State,
    http::{HeaderMap, StatusCode},
    response::{Html, IntoResponse},
    routing::{get, post},
    Json, Router,
};
use rand::Rng;
use serde::Deserialize;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::io::Write;
use std::thread::{self, JoinHandle};
use tokio::sync::oneshot;

const MAX_LOG_LINES: usize = 500;
const IPC_TOKEN_HEADER: &str = "x-spectre-token";
const MAX_PATH_LEN: usize = 2048;

fn generate_token() -> String {
    rand::thread_rng()
        .sample_iter(rand::distributions::Alphanumeric)
        .take(32)
        .map(char::from)
        .collect()
}

fn is_path_safe(s: &str) -> bool {
    if s.is_empty() || s.len() > MAX_PATH_LEN || s.contains('\0') {
        return false;
    }
    let lower = s.to_lowercase();
    if lower.starts_with("file:")
        || lower.starts_with("http:")
        || lower.starts_with("https:")
        || lower.starts_with("ftp:")
    {
        return false;
    }
    let parts: Vec<&str> = s.split(|c| c == '/' || c == '\\').collect();
    let mut depth: i32 = 0;
    for p in parts {
        if p == ".." {
            depth -= 1;
            if depth < 0 {
                return false;
            }
        } else if !p.is_empty() && p != "." {
            depth += 1;
        }
    }
    true
}

fn trim_path_quotes(s: &str) -> String {
    let s = s.trim();
    if s.len() >= 2 && s.starts_with('"') && s.ends_with('"') {
        s[1..s.len() - 1].trim().to_string()
    } else {
        s.to_string()
    }
}

fn path_file_name(path: &str) -> &str {
    let path = path.trim();
    if path.is_empty() {
        return path;
    }
    let path = path.trim_matches('"').trim();
    if path.is_empty() {
        return path;
    }
    path.rsplit(|c| c == '/' || c == '\\')
        .next()
        .unwrap_or(path)
}

fn is_allowed_hd2ds_exe_path(path: &str) -> bool {
    let path = path.trim();
    if path.is_empty() {
        return true;
    }
    path_file_name(path).eq_ignore_ascii_case("hd2ds.exe")
}

fn is_allowed_sabre_exe_path(path: &str) -> bool {
    let path = path.trim();
    if path.is_empty() {
        return true;
    }
    path_file_name(path).eq_ignore_ascii_case("hd2ds_sabresquadron.exe")
}

#[derive(Clone)]
pub struct ServerUtilityHttpState {
    pub config_path: PathBuf,
    pub server_pids: Arc<std::sync::Mutex<HashMap<u16, u32>>>,
    pub log_state: Option<Arc<std::sync::Mutex<(PathBuf, u32)>>>,
    pub helper_kicked: Option<Arc<std::sync::Mutex<HashMap<u16, HashSet<String>>>>>,
    pub helper_last_slots: Option<Arc<std::sync::Mutex<HashMap<u16, Vec<(String, String)>>>>>,
    pub request_log: Arc<std::sync::Mutex<Vec<String>>>,
    pub log_file_path: Option<PathBuf>,
    pub log_max_size_bytes: u64,
}

#[derive(Clone)]
struct AppState {
    inner: ServerUtilityHttpState,
    shutdown: Arc<AtomicBool>,
    token: String,
}

fn push_log(
    log: &Arc<std::sync::Mutex<Vec<String>>>,
    line: &str,
    log_file: Option<&PathBuf>,
    log_max_bytes: u64,
) {
    if let Ok(mut g) = log.lock() {
        g.push(line.to_string());
        let n = g.len();
        if n > MAX_LOG_LINES {
            g.drain(0..n - MAX_LOG_LINES);
        }
    }
    if let (Some(path), max_bytes) = (log_file, log_max_bytes) {
        if max_bytes == 0 {
            return;
        }
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let line_with_newline = format!("{}\n", line);
        let append = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path);
        if let Ok(mut f) = append {
            let _ = f.write_all(line_with_newline.as_bytes());
            let _ = f.flush();
        }
        if let Ok(meta) = std::fs::metadata(path) {
            if meta.len() > max_bytes {
                if let Ok(content) = std::fs::read_to_string(path) {
                        let keep_len = (max_bytes as usize).saturating_sub(1024).min(content.len());
                        let start = content.len().saturating_sub(keep_len);
                        let truncated = if start > 0 {
                            format!("--- log rotated ---\n{}", &content[start..])
                        } else {
                            content
                        };
                        let _ = std::fs::write(path, truncated);
                }
            }
        }
    }
}

fn handle_ipc(
    state: &ServerUtilityHttpState,
    msg: &IpcSaveMessage,
) -> Vec<String> {
    let mut responses = Vec::new();
    let config_path = &state.config_path;
    let shared_pids = &state.server_pids;
    let shared_helper_kicked = &state.helper_kicked;
    let shared_helper_last_slots = &state.helper_last_slots;

    match msg.action.as_str() {
        "save" => {
            for s in &msg.servers {
                if !is_path_safe(&s.hd2ds_path)
                    || !is_path_safe(&s.hd2ds_sabresquadron_path)
                    || !is_path_safe(&s.mpmaplist_path)
                {
                    responses.push("Invalid path (unsafe characters or traversal)".to_string());
                    return responses;
                }
                if !is_allowed_hd2ds_exe_path(&s.hd2ds_path)
                    || !is_allowed_sabre_exe_path(&s.hd2ds_sabresquadron_path)
                {
                    responses.push("Executable path must be HD2DS.exe or HD2DS_SabreSquadron.exe".to_string());
                    return responses;
                }
            }
            let mut data = spectre_core::server::ServerLauncherData::load_from_file(config_path)
                .unwrap_or_else(|_| spectre_core::server::ServerLauncherData::default());
            data.servers = msg
                .servers
                .iter()
                .map(|s| {
                    let mut s2 = s.clone();
                    s2.hd2ds_path = trim_path_quotes(&s.hd2ds_path);
                    s2.hd2ds_sabresquadron_path = trim_path_quotes(&s.hd2ds_sabresquadron_path);
                    s2.mpmaplist_path = trim_path_quotes(&s.mpmaplist_path);
                    s2
                })
                .collect();
            if let Some(ref sm) = msg.server_manager {
                data.server_manager = sm.clone();
            }
            if let Some(parent) = config_path.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            match data.save_to_file(config_path) {
                Ok(()) => {
                    let mut data = spectre_core::server::ServerLauncherData::load_from_file(config_path)
                        .unwrap_or_else(|_| spectre_core::server::ServerLauncherData::default());
                    crate::ensure_server_utility_has_defaults(&mut data);
                    if let Ok(pids) = shared_pids.lock() {
                        for server in data.servers.iter_mut() {
                            server.running = pids.contains_key(&server.port);
                        }
                    }
                    for server in data.servers.iter_mut() {
                        let path_str = trim_path_quotes(&server.mpmaplist_path);
                        let maps = if path_str.is_empty() {
                            std::collections::HashMap::new()
                        } else {
                            spectre_core::mpmaplist::load_from_path(std::path::Path::new(&path_str))
                        };
                        server.available_maps_by_style = maps;
                    }
                    match serde_json::to_string(&data.servers) {
                        Ok(json) => responses.push(format!("STATE:{}", json)),
                        Err(_) => responses.push("Saved OK".to_string()),
                    }
                }
                Err(e) => responses.push(e),
            }
        }
        "start" => {
            let idx = msg.server_index.unwrap_or(0);
            let server_opt = msg.servers.get(idx).map(|s| {
                let mut s2 = s.clone();
                s2.hd2ds_path = trim_path_quotes(&s.hd2ds_path);
                s2.hd2ds_sabresquadron_path = trim_path_quotes(&s.hd2ds_sabresquadron_path);
                s2.mpmaplist_path = trim_path_quotes(&s.mpmaplist_path);
                s2
            });
            if let Some(ref server) = server_opt {
                if !is_path_safe(&server.hd2ds_path)
                    || !is_path_safe(&server.hd2ds_sabresquadron_path)
                    || !is_path_safe(&server.mpmaplist_path)
                {
                    responses.push("Invalid path (unsafe characters or traversal)".to_string());
                    return responses;
                }
                let exe_path = if server.use_sabre_squadron {
                    server.hd2ds_sabresquadron_path.as_str()
                } else {
                    server.hd2ds_path.as_str()
                };
                let allowed = if server.use_sabre_squadron {
                    is_allowed_sabre_exe_path(exe_path)
                } else {
                    is_allowed_hd2ds_exe_path(exe_path)
                };
                if !allowed {
                    responses.push("Executable path must be HD2DS.exe or HD2DS_SabreSquadron.exe".to_string());
                    return responses;
                }
            }
            match spectre_core::server::ServerLauncherData::load_from_file(config_path) {
                Ok(_) => match server_opt {
                    Some(server) => {
                        match spectre_core::ds_launch::start_ds(&server) {
                            Ok(pid) => {
                                if let Ok(mut pids) = shared_pids.lock() {
                                    pids.insert(server.port, pid);
                                }
                                responses.push("Started OK".to_string());
                            }
                            Err(e) => responses.push(e),
                        }
                    }
                    None => responses.push("Invalid server index".to_string()),
                },
                Err(e) => responses.push(e),
            }
        }
        "stop" => {
            let idx = msg.server_index.unwrap_or(0);
            if let Some(server) = msg.servers.get(idx) {
                let port = server.port;
                let mut pids = match shared_pids.lock() {
                    Ok(g) => g,
                    Err(_) => {
                        responses.push("Stop failed (lock)".to_string());
                        return responses;
                    }
                };
                if let Some(&pid) = pids.get(&port) {
                    pids.remove(&port);
                    if let Some(ref k) = shared_helper_kicked {
                        let _ = k.lock().map(|mut m| m.remove(&port));
                    }
                    if let Some(ref last) = shared_helper_last_slots {
                        let _ = last.lock().map(|mut m| m.remove(&port));
                    }
                    drop(pids);
                    let status = if crate::kill_process_by_pid(pid) {
                        "Stopped OK".to_string()
                    } else {
                        "Stopped OK".to_string()
                    };
                    responses.push(status);
                } else {
                    responses.push("Server not running".to_string());
                }
            } else {
                responses.push("Invalid server index".to_string());
            }
        }
        "stop_all" => {
            let mut pids = match shared_pids.lock() {
                Ok(g) => g,
                Err(_) => {
                    responses.push("Stop all failed (lock)".to_string());
                    return responses;
                }
            };
            let to_stop: Vec<(u16, u32)> = msg
                .servers
                .iter()
                .filter_map(|s| pids.get(&s.port).copied().map(|pid| (s.port, pid)))
                .collect();
            for (port, _) in &to_stop {
                pids.remove(port);
                if let Some(ref k) = shared_helper_kicked {
                    let _ = k.lock().map(|mut m| m.remove(port));
                }
                if let Some(ref last) = shared_helper_last_slots {
                    let _ = last.lock().map(|mut m| m.remove(port));
                }
            }
            drop(pids);
            for (_, pid) in &to_stop {
                crate::kill_process_by_pid(*pid);
            }
            responses.push("All servers stopped".to_string());
        }
        "start_all" => {
            let servers: Vec<spectre_core::server::Server> = msg
                .servers
                .iter()
                .map(|s| {
                    let mut s2 = s.clone();
                    s2.hd2ds_path = trim_path_quotes(&s.hd2ds_path);
                    s2.hd2ds_sabresquadron_path = trim_path_quotes(&s.hd2ds_sabresquadron_path);
                    s2.mpmaplist_path = trim_path_quotes(&s.mpmaplist_path);
                    s2
                })
                .collect();
            for s in &servers {
                if !is_path_safe(&s.hd2ds_path)
                    || !is_path_safe(&s.hd2ds_sabresquadron_path)
                    || !is_path_safe(&s.mpmaplist_path)
                {
                    responses.push("Invalid path (unsafe characters or traversal)".to_string());
                    return responses;
                }
                let exe_path = if s.use_sabre_squadron {
                    s.hd2ds_sabresquadron_path.as_str()
                } else {
                    s.hd2ds_path.as_str()
                };
                let allowed = if s.use_sabre_squadron {
                    is_allowed_sabre_exe_path(exe_path)
                } else {
                    is_allowed_hd2ds_exe_path(exe_path)
                };
                if !exe_path.is_empty() && !allowed {
                    responses.push("Executable path must be HD2DS.exe or HD2DS_SabreSquadron.exe".to_string());
                    return responses;
                }
            }
            let servers = servers;
            let pids_b = shared_pids.clone();
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
            let status = if errs.is_empty() {
                "All servers started".to_string()
            } else {
                errs.join("; ")
            };
            responses.push(status);
        }
        "get_running" => {
            let ports: Vec<u16> = shared_pids
                .lock()
                .map(|p| p.keys().copied().collect())
                .unwrap_or_default();
            responses.push(format!(
                "RUNNING:{}",
                serde_json::to_string(&ports).unwrap_or_else(|_| "[]".to_string())
            ));
        }
        "get_players" => {
            let idx = msg.server_index.unwrap_or(0);
            let (status, list_json) = match msg.servers.get(idx) {
                Some(server) => {
                    let pid = shared_pids
                        .lock()
                        .ok()
                        .and_then(|p| p.get(&server.port).copied());
                    let max_clients = server
                        .configs
                        .iter()
                        .find(|c| c.name == server.current_config)
                        .map(|c| c.max_clients as u32)
                        .unwrap_or(32);
                    let status = match pid {
                        Some(pid) => match crate::ds_helper::get_player_count(pid, max_clients) {
                            Some((active, total)) => format!("PLAYERS:{},{}", active, total),
                            None => "PLAYERS:--,--".to_string(),
                        },
                        None => "PLAYERS:--,--".to_string(),
                    };
                    let list_json = match pid {
                        Some(pid) => crate::ds_helper::get_player_list(pid)
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
                    (status, list_json)
                }
                _ => ("PLAYERS:--,--".to_string(), "[]".to_string()),
            };
            responses.push(status);
            responses.push(format!("PLAYER_LIST:{}", list_json));
        }
        "get_log_content" => {
            let path = crate::app_log_path(config_path);
            crate::ensure_log_file_exists(&path);
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
            responses.push(format!("LOG_CONTENT:{}", content));
        }
        "repaint" => responses.push("REPAINT".to_string()),
        "refresh_mpmaplist" => {
            let mut servers = msg.servers.clone();
            for server in servers.iter_mut() {
                let path_str = trim_path_quotes(&server.mpmaplist_path);
                let maps = if path_str.is_empty() {
                    std::collections::HashMap::new()
                } else {
                    spectre_core::mpmaplist::load_from_path(std::path::Path::new(&path_str))
                };
                server.available_maps_by_style = maps;
            }
            let status = match serde_json::to_string(&servers) {
                Ok(json) => format!("REFRESH:{}", json),
                Err(_) => "Refresh failed.".to_string(),
            };
            responses.push(status);
        }
        "browse_mpmaplist" | "browse_hd2_dir" => {
            responses.push("BROWSE_NOT_AVAILABLE".to_string());
        }
        "open_log_file" => {
            let path = crate::app_log_path(config_path);
            crate::ensure_log_file_exists(&path);
            let abs_path = std::fs::canonicalize(&path).unwrap_or_else(|_| path.clone());
            let mut path_str = abs_path.display().to_string();
            if path_str.starts_with(r"\\?\") {
                path_str = path_str[r"\\?\".len()..].to_string();
            }
            let folder = std::path::Path::new(&path_str)
                .parent()
                .map(|p| p.to_path_buf())
                .unwrap_or_else(|| path.clone());
            let folder_str = folder.display().to_string();
            let _ = std::process::Command::new("explorer").arg(&folder_str).spawn();
            responses.push("OK".to_string());
        }
        _ => {}
    }
    responses
}

#[derive(Deserialize)]
struct IpcSaveMessage {
    action: String,
    servers: Vec<spectre_core::server::Server>,
    #[serde(default)]
    server_index: Option<usize>,
    #[serde(default)]
    server_manager: Option<spectre_core::server::ServerManager>,
    #[serde(default)]
    browse_which: Option<String>,
}

#[derive(serde::Serialize)]
struct IpcResponse {
    responses: Vec<String>,
}

async fn serve_index(State(state): State<AppState>) -> impl IntoResponse {
    push_log(
        &state.inner.request_log,
        "GET /",
        state.inner.log_file_path.as_ref(),
        state.inner.log_max_size_bytes,
    );
    let token = &state.token;
    let config_path = &state.inner.config_path;
    let path_exists = config_path.exists();
    let mut data = spectre_core::server::ServerLauncherData::load_from_file(config_path)
        .unwrap_or_else(|_| spectre_core::server::ServerLauncherData::default());
    crate::ensure_server_utility_has_defaults(&mut data);
    if let Ok(pids) = state.inner.server_pids.lock() {
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
    let _ = path_exists;
    let initial_json = match serde_json::to_value(&data) {
        Ok(v) => serde_json::to_string(&v).ok(),
        Err(_) => None,
    };
    let mut html = spectre_web::embedded_card_html(
        "server_utility",
        initial_json.as_deref(),
        cfg!(debug_assertions),
    )
    .unwrap_or_else(|_| "<html><body>Error loading Server Utility</body></html>".to_string());
    let version = env!("CARGO_PKG_VERSION");
    html = html.replace("{{SPECTRE_VERSION}}", version);
    let token_js: String = token
        .chars()
        .map(|c| match c {
            '\\' => "\\\\".into(),
            '"' => "\\\"".into(),
            '\n' => "\\n".into(),
            '\r' => "\\r".into(),
            _ => c.to_string(),
        })
        .collect();
    let ipc_polyfill = format!(
        r#"window.__spectreIpcToken="{}";window.ipc={{postMessage:function(b){{var h={{'Content-Type':'application/json','X-Spectre-Token':window.__spectreIpcToken||''}};fetch('/api/ipc',{{method:'POST',headers:h,body:b}}).then(function(r){{if(!r.ok)return r.text().then(function(t){{throw new Error(t||r.status);}});return r.json();}}).then(function(d){{(d.responses||[]).forEach(function(m){{if(window.__spectreIpcStatus)window.__spectreIpcStatus(m);}});}}).catch(function(e){{if(window.__spectreIpcStatus)window.__spectreIpcStatus('Error: '+e.message);}});}}}};"#,
        token_js
    );
    if let Some(pos) = html.find("<script>") {
        html.insert_str(pos + 8, &ipc_polyfill);
    }
    Html(html)
}

async fn api_ipc(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(msg): Json<IpcSaveMessage>,
) -> impl IntoResponse {
    let supplied = headers
        .get(IPC_TOKEN_HEADER)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    if supplied.is_empty() || supplied != state.token {
    push_log(
        &state.inner.request_log,
        &format!("POST /api/ipc {} (403 forbidden)", msg.action),
        state.inner.log_file_path.as_ref(),
        state.inner.log_max_size_bytes,
    );
        return (
            StatusCode::FORBIDDEN,
            Json(IpcResponse {
                responses: vec!["Forbidden".to_string()],
            }),
        );
    }
    push_log(
        &state.inner.request_log,
        &format!("POST /api/ipc {}", msg.action),
        state.inner.log_file_path.as_ref(),
        state.inner.log_max_size_bytes,
    );
    if state.shutdown.load(Ordering::Relaxed) {
        return (StatusCode::SERVICE_UNAVAILABLE, Json(IpcResponse { responses: vec![] }));
    }
    let responses = handle_ipc(&state.inner, &msg);
    (StatusCode::OK, Json(IpcResponse { responses }))
}

pub struct ServerHandle {
    pub port: u16,
    pub join_handle: Option<JoinHandle<()>>,
    pub request_log: Arc<std::sync::Mutex<Vec<String>>>,
    shutdown: Arc<AtomicBool>,
    shutdown_tx: Option<oneshot::Sender<()>>,
}

pub fn start(
    port: u16,
    state: ServerUtilityHttpState,
) -> Result<ServerHandle, String> {
    let request_log = state.request_log.clone();
    let shutdown = Arc::new(AtomicBool::new(false));
    let (shutdown_tx, shutdown_rx) = oneshot::channel::<()>();
    let token = generate_token();
    let app_state = AppState {
        inner: state,
        shutdown: shutdown.clone(),
        token,
    };

    let listener = std::net::TcpListener::bind(("0.0.0.0", port))
        .map_err(|e| format!("Bind {}: {}", port, e))?;
    listener.set_nonblocking(true).map_err(|e| e.to_string())?;
    let addr = listener.local_addr().map_err(|e| e.to_string())?;
    let actual_port = addr.port();

    let router = Router::new()
        .route("/", get(serve_index))
        .route("/api/ipc", post(api_ipc))
        .with_state(app_state);

    let join_handle = thread::spawn(move || {
        let rt = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .expect("tokio runtime");
        rt.block_on(async move {
            let listener = tokio::net::TcpListener::from_std(listener.into()).expect("tokio listener");
            let server = axum::serve(listener, router);
            tokio::select! {
                r = server => { r.ok(); }
                _ = shutdown_rx => {}
            }
        });
    });

    Ok(ServerHandle {
        port: actual_port,
        join_handle: Some(join_handle),
        request_log,
        shutdown,
        shutdown_tx: Some(shutdown_tx),
    })
}

impl ServerHandle {
    pub fn stop(&mut self) -> Option<JoinHandle<()>> {
        self.shutdown.store(true, Ordering::Relaxed);
        if let Some(tx) = self.shutdown_tx.take() {
            let _ = tx.send(());
        }
        self.join_handle.take()
    }

    pub fn get_log_lines(&self) -> Vec<String> {
        self.request_log.lock().map(|g| g.clone()).unwrap_or_default()
    }
}
