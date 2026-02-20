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
    pub server_hd2ds_path: String,
    #[serde(default)]
    pub server_sabresquadron_path: String,
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
            server_hd2ds_path: String::new(),
            server_sabresquadron_path: String::new(),
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
                if let Ok(mut config) = serde_json::from_str::<Config>(&contents) {
                    println!("[DEBUG] Config loaded from {}", CONFIG_FILE);
                    let current_id = get_machine_id();
                    let stored_id = config.machine_id.as_deref();
                    if stored_id != Some(current_id.as_str()) {
                        println!(
                            "[DEBUG] Config: machine mismatch (stored={:?}, current={}), resetting config to defaults",
                            stored_id, current_id
                        );
                        let mut config = Config::default();
                        config.machine_id = Some(current_id);
                        config.save();
                        return config;
                    }
                    if config.server_hd2ds_path.trim().is_empty()
                        || config.server_sabresquadron_path.trim().is_empty()
                    {
                        config.server_utility_wizard_completed = false;
                    }
                    return config;
                } else {
                    println!("[DEBUG] Failed to parse config file, creating default");
                }
            } else {
                println!("[DEBUG] Failed to read config file, creating default");
            }
        } else {
            println!("[DEBUG] Config file not found, creating default");
        }

        let default_config = Config::default();
        default_config.save();
        default_config
    }

    pub fn save(&self) {
        let to_save = if self.server_hd2ds_path.trim().is_empty()
            || self.server_sabresquadron_path.trim().is_empty()
        {
            let mut c = self.clone();
            c.server_utility_wizard_completed = false;
            c
        } else {
            self.clone()
        };
        if let Ok(json) = serde_json::to_string_pretty(&to_save) {
            if fs::create_dir_all(CONFIG_DIR).is_ok() && fs::write(CONFIG_FILE, json).is_ok() {
                println!("[DEBUG] Config saved to {}", CONFIG_FILE);
            } else {
                println!("[DEBUG] Failed to save config to {}", CONFIG_FILE);
            }
        } else {
            println!("[DEBUG] Failed to serialize config");
        }
    }
}

