
#![cfg(windows)]

use std::sync::Arc;
use tao::event_loop::{ControlFlow, EventLoop};
use tao::window::WindowBuilder;
use wry::WebViewBuilder;

#[derive(Default)]
pub struct AppState {
    _placeholder: (),
}

impl AppState {
    pub fn new() -> Self {
        Self::default()
    }
}


const SERVER_UTILITY_HTML: &str = include_str!("../content/server_utility/index.html");
const SERVER_UTILITY_CSS: &str = include_str!("../content/server_utility/css/style.css");
const SERVER_UTILITY_JS: &str = include_str!("../content/server_utility/js/app.js");

fn embed_server_utility(initial_state_json: Option<&str>) -> String {
    let initial_script = if let Some(json) = initial_state_json {
        let escaped = json
            .replace('\\', "\\\\")
            .replace('"', "\\\"")
            .replace('\r', "\\r")
            .replace('\n', "\\n")
            .replace("</script>", "<\\/script>");
        format!(
            r#"<script>window.__spectreInitialState=JSON.parse("{}");</script>"#,
            escaped
        )
    } else {
        String::new()
    };
    SERVER_UTILITY_HTML
        .replace(
            r#"<link rel="stylesheet" href="css/style.css">"#,
            &format!("<style>{}</style>", SERVER_UTILITY_CSS),
        )
        .replace(
            r#"<script src="js/app.js"></script>"#,
            &format!("{}<script>{}</script>", initial_script, SERVER_UTILITY_JS),
        )
}

/// Inlined HTML for a card by name (embedded at build time).
pub fn embedded_card_html(card_name: &str, initial_state_json: Option<&str>) -> Result<String, String> {
    match card_name {
        "server_utility" => Ok(embed_server_utility(initial_state_json)),
        _ => Err(format!("Unknown card: '{}'. Cards are built into the binary at compile time.", card_name)),
    }
}

pub fn card_url(card_name: &str) -> Result<String, String> {
    embedded_card_html(card_name, None).map(|_| "embedded".to_string())
}

pub fn run_app() -> Result<(), String> {
    run_app_with_card("server_utility")
}

pub fn run_app_with_card(card_name: &str) -> Result<(), String> {
    let _state = Arc::new(AppState::new());
    let html = embedded_card_html(card_name, None)?;

    let event_loop = EventLoop::new();
    let window = WindowBuilder::new()
        .with_title("Spectre")
        .with_inner_size(tao::dpi::LogicalSize::new(1000.0, 700.0))
        .build(&event_loop)
        .map_err(|e| e.to_string())?;

    let _webview = WebViewBuilder::new(&window)
        .with_html(&html)
        .build()
        .map_err(|e| e.to_string())?;

    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Wait;
        if let tao::event::Event::WindowEvent {
            event: tao::event::WindowEvent::CloseRequested,
            ..
        } = event
        {
            *control_flow = ControlFlow::Exit;
        }
    });
}
