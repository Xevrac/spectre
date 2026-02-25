use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

const CONFIG_DIR: &str = "content";
const CONFIG_FILE: &str = "content/spectre_config.json";

/// Stable machine ID (Windows: MachineGuid; else hostname).
pub fn get_machine_id() -> String {
    #[cfg(windows)]
    {
        use winreg::enums::HKEY_LOCAL_MACHINE;
        use winreg::RegKey;
        let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
        if let Ok(crypto) = hklm.open_subkey(r"SOFTWARE\Microsoft\Cryptography") {
            if let Ok(guid) = crypto.get_value::<String, _>("MachineGuid") {
                if !guid.is_empty() {
                    return guid;
                }
            }
        }
        std::env::var("COMPUTERNAME").unwrap_or_else(|_| "unknown".into())
    }
    #[cfg(not(windows))]
    {
        std::env::var("HOSTNAME").unwrap_or_else(|_| "unknown".to_string())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    #[serde(default = "default_fullscreen_dialogs")]
    pub fullscreen_dialogs: bool,
    #[serde(default)]
    pub directplay_detected: bool,
    #[serde(default)]
    pub machine_id: Option<String>,
    #[serde(default)]
    pub server_utility_wizard_completed: bool,
}

fn default_fullscreen_dialogs() -> bool {
    false
}

impl Default for Config {
    fn default() -> Self {
        Self {
            fullscreen_dialogs: false,
            directplay_detected: false,
            machine_id: None,
            server_utility_wizard_completed: false,
        }
    }
}

impl Config {
    pub fn load() -> Self {
        if Path::new(CONFIG_FILE).exists() {
            if let Ok(contents) = fs::read_to_string(CONFIG_FILE) {
                if let Ok(config) = serde_json::from_str::<Config>(&contents) {
                    println!("[Spectre.dbg] Config loaded from {}", CONFIG_FILE);
                    let current_id = get_machine_id();
                    let stored_id = config.machine_id.as_deref();
                    if stored_id != Some(current_id.as_str()) {
                        println!(
                            "[Spectre.dbg] Config: machine mismatch (stored={:?}, current={}), resetting config to defaults",
                            stored_id, current_id
                        );
                        let mut config = Config::default();
                        config.machine_id = Some(current_id);
                        config.save();
                        return config;
                    }
                    return config;
                } else {
                    println!("[Spectre.dbg] Failed to parse config file, creating default");
                }
            } else {
                println!("[Spectre.dbg] Failed to read config file, creating default");
            }
        } else {
            println!("[Spectre.dbg] Config file not found, creating default");
        }

        let default_config = Config::default();
        default_config.save();
        default_config
    }

    pub fn save(&self) {
        if let Ok(json) = serde_json::to_string_pretty(self) {
            if fs::create_dir_all(CONFIG_DIR).is_ok() && fs::write(CONFIG_FILE, json).is_ok() {
                println!("[Spectre.dbg] Config saved to {}", CONFIG_FILE);
            } else {
                println!("[Spectre.dbg] Failed to save config to {}", CONFIG_FILE);
            }
        } else {
            println!("[Spectre.dbg] Failed to serialize config");
        }
    }
}

