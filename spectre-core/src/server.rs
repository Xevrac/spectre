use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerManager {
    pub server_ip: String,
    pub server_port: u16,
    pub hd2ds_path: String,
    pub hd2ds_sabresquadron_path: String,
    pub mpmaplist_path: String,
    pub enable_watchdog: bool,
    pub watchdog_interval: u32,
    pub enable_messaging: bool,
    pub messaging_interval: u32,
    pub enable_reboot: bool,
    pub reboot_interval: u32,
    pub enable_forced_messages: bool,
    pub forced_messages: Vec<String>,
    pub enable_forced_ban_list: bool,
    pub forced_ban_list: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    pub username: String,
    pub password: String,
    pub privilege_level: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ServerConfig {
    pub name: String,
    pub domain: String,
    pub style: String,
    pub session_name: String,
    pub max_clients: u8,
    pub point_limit: u8,
    pub round_limit: u8,
    pub round_count: u8,
    pub respawn_time: u16,
    pub spawn_protection: u8,
    pub warmup: u8,
    pub inverse_damage: u8,
    pub friendly_fire: bool,
    pub auto_team_balance: bool,
    pub third_person_view: bool,
    pub allow_crosshair: bool,
    pub falling_dmg: bool,
    pub allow_respawn: bool,
    pub allow_vehicles: bool,
    pub difficulty: String,
    pub respawn_number: i32,
    pub team_respawn: bool,
    pub password: String,
    pub admin_pass: String,
    pub max_ping: u16,
    pub max_freq: u16,
    pub max_inactivity: u16,
    pub voice_chat: u8,
    pub maps: Vec<String>,
    pub messages: Vec<String>,
    pub ban_list: Vec<String>,
    pub enable_auto_kick: bool,
    pub clan_tag: String,
    pub clan_side: String,
    pub clan_reserve: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Server {
    pub name: String,
    pub running: bool,
    pub watchdog: bool,
    pub messages: bool,
    pub users: Vec<String>,
    pub port: u16,
    /// When true, use HD2DS_SabreSquadron.exe; otherwise HD2DS.exe
    pub use_sabre_squadron: bool,
    pub current_config: String,
    pub configs: Vec<ServerConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerLauncherData {
    pub server_manager: ServerManager,
    pub users: Vec<User>,
    pub servers: Vec<Server>,
}

impl Default for ServerManager {
    fn default() -> Self {
        Self {
            server_ip: "10.0.0.1".to_string(),
            server_port: 2332,
            hd2ds_path: String::new(),
            hd2ds_sabresquadron_path: String::new(),
            mpmaplist_path: String::new(),
            enable_watchdog: true,
            watchdog_interval: 15,
            enable_messaging: true,
            messaging_interval: 180,
            enable_reboot: false,
            reboot_interval: 48,
            enable_forced_messages: false,
            forced_messages: Vec::new(),
            enable_forced_ban_list: true,
            forced_ban_list: Vec::new(),
        }
    }
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            name: String::new(),
            domain: "Internet".to_string(),
            style: "Cooperative".to_string(),
            session_name: "A Spectre Session".to_string(),
            max_clients: 64,
            point_limit: 0,
            round_limit: 25,
            round_count: 1,
            respawn_time: 20,
            spawn_protection: 0,
            warmup: 10,
            inverse_damage: 0,
            friendly_fire: true,
            auto_team_balance: false,
            third_person_view: false,
            allow_crosshair: true,
            falling_dmg: true,
            allow_respawn: true,
            allow_vehicles: true,
            difficulty: "Hard".to_string(),
            respawn_number: 1,
            team_respawn: false,
            password: String::new(),
            admin_pass: String::new(),
            max_ping: 0,
            max_freq: 0,
            max_inactivity: 0,
            voice_chat: 0,
            maps: Vec::new(),
            messages: Vec::new(),
            ban_list: Vec::new(),
            enable_auto_kick: false,
            clan_tag: String::new(),
            clan_side: "axis".to_string(),
            clan_reserve: 0,
        }
    }
}

impl Default for Server {
    fn default() -> Self {
        Self {
            name: String::new(),
            running: false,
            watchdog: false,
            messages: false,
            users: Vec::new(),
            port: 22000,
            use_sabre_squadron: true,
            current_config: String::new(),
            configs: Vec::new(),
        }
    }
}

impl ServerLauncherData {
    /// Load server launcher config from a JSON file. If the file does not exist, returns default.
    pub fn load_from_file(path: &Path) -> Result<Self, String> {
        if !path.exists() {
            return Ok(Self::default());
        }
        let content = fs::read_to_string(path)
            .map_err(|e| format!("Failed to read config file: {}", e))?;
        serde_json::from_str(&content).map_err(|e| format!("Invalid config JSON: {}", e))
    }

    /// Save server launcher config as pretty-printed JSON.
    pub fn save_to_file(&self, path: &Path) -> Result<(), String> {
        let content = serde_json::to_string_pretty(self)
            .map_err(|e| format!("Failed to serialize config: {}", e))?;
        fs::write(path, content)
            .map_err(|e| format!("Failed to write config file: {}", e))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_roundtrip_json() {
        let data = ServerLauncherData::default();
        let json = serde_json::to_string_pretty(&data).unwrap();
        let loaded: ServerLauncherData = serde_json::from_str(&json).unwrap();
        assert_eq!(data.servers.len(), loaded.servers.len());
    }
}

impl Default for ServerLauncherData {
    fn default() -> Self {
        Self {
            server_manager: ServerManager::default(),
            users: vec![User {
                username: "Admin".to_string(),
                password: String::new(),
                privilege_level: 2,
            }],
            servers: Vec::new(),
        }
    }
}


