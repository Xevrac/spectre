use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

const CONFIG_FILE: &str = "spectre_config.json";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub theme: String,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            theme: "dark".to_string(),
        }
    }
}

impl Config {
    pub fn load() -> Self {
        if Path::new(CONFIG_FILE).exists() {
            if let Ok(contents) = fs::read_to_string(CONFIG_FILE) {
                if let Ok(config) = serde_json::from_str::<Config>(&contents) {
                    println!("[DEBUG] Config loaded from {}", CONFIG_FILE);
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
        if let Ok(json) = serde_json::to_string_pretty(self) {
            if fs::write(CONFIG_FILE, json).is_ok() {
                println!("[DEBUG] Config saved to {}", CONFIG_FILE);
            } else {
                println!("[DEBUG] Failed to save config to {}", CONFIG_FILE);
            }
        } else {
            println!("[DEBUG] Failed to serialize config");
        }
    }
}
