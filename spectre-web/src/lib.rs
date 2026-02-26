#![cfg_attr(windows, doc = "Windows WebView2 host and app runner.")]
#![cfg_attr(not(windows), doc = "Stub when not on Windows.")]

pub mod app;

#[cfg(windows)]
pub use app::{card_url, embedded_card_html, run_app, run_app_with_card, AppState};
#[cfg(not(windows))]
pub fn run_app() -> Result<(), String> {
    Err("spectre-web is only supported on Windows (WebView2).".to_string())
}
#[cfg(not(windows))]
pub fn card_url(_card_name: &str) -> Result<String, String> {
    Err("spectre-web is only supported on Windows (WebView2).".to_string())
}
#[cfg(not(windows))]
pub fn run_app_with_card(_card_name: &str) -> Result<(), String> {
    Err("spectre-web is only supported on Windows (WebView2).".to_string())
}
#[cfg(not(windows))]
pub fn embedded_card_html(
    _card_name: &str,
    _initial_state_json: Option<&str>,
    _debug_mode: bool,
) -> Result<String, String> {
    Err("spectre-web is only supported on Windows (WebView2).".to_string())
}
#[cfg(not(windows))]
pub struct AppState;
#[cfg(not(windows))]
impl AppState {
    pub fn new() -> Self {
        Self
    }
}
