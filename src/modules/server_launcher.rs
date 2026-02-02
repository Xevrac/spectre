use super::Module;
use eframe::egui;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerManager {
    pub server_ip: String,
    pub server_port: u16,
    pub hd2ds_path: String,
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
            session_name: "HD2 Server".to_string(),
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
            current_config: String::new(),
            configs: Vec::new(),
        }
    }
}

impl ServerLauncherData {
    pub fn load_from_file(path: &Path) -> Result<Self, String> {
        if !path.exists() {
            return Ok(Self::default());
        }

        let content = fs::read_to_string(path)
            .map_err(|e| format!("Failed to read config file: {}", e))?;

        Self::parse_config(&content)
    }

    pub fn save_to_file(&self, path: &Path) -> Result<(), String> {
        let content = self.to_config_string();
        fs::write(path, content)
            .map_err(|e| format!("Failed to write config file: {}", e))
    }

    fn parse_config(content: &str) -> Result<Self, String> {
        let mut data = ServerLauncherData::default();
        let lines: Vec<&str> = content.lines().collect();
        let mut i = 0;

        while i < lines.len() {
            let line = lines[i].trim();
            
            if line.starts_with("<ServerManager>") {
                i = Self::parse_server_manager(&lines, i + 1, &mut data.server_manager)?;
            } else if line.starts_with("<Users>") {
                i = Self::parse_users(&lines, i + 1, &mut data.users)?;
            } else if line.starts_with("<Servers>") {
                i = Self::parse_servers(&lines, i + 1, &mut data.servers)?;
            }
            i += 1;
        }

        Ok(data)
    }

    fn parse_server_manager(lines: &[&str], start: usize, sm: &mut ServerManager) -> Result<usize, String> {
        let mut i = start;
        while i < lines.len() {
            let line = lines[i].trim();
            if line.starts_with("</ServerManager>") {
                return Ok(i);
            }
            
            if line.starts_with("ServerIP") {
                sm.server_ip = Self::parse_string_value(line);
            } else if line.starts_with("ServerPort") {
                sm.server_port = Self::parse_u16_value(line).unwrap_or(2332);
            } else if line.starts_with("HD2DSPath") {
                sm.hd2ds_path = Self::parse_quoted_string_value(line);
            } else if line.starts_with("EnableWatchDog") {
                sm.enable_watchdog = Self::parse_bool_value(line);
            } else if line.starts_with("WatchdogInterval") {
                sm.watchdog_interval = Self::parse_u32_value(line).unwrap_or(15);
            } else if line.starts_with("EnableMessaging") {
                sm.enable_messaging = Self::parse_bool_value(line);
            } else if line.starts_with("MessagingInterval") {
                sm.messaging_interval = Self::parse_u32_value(line).unwrap_or(180);
            } else if line.starts_with("EnableReboot") {
                sm.enable_reboot = Self::parse_bool_value(line);
            } else if line.starts_with("RebootInterval") {
                sm.reboot_interval = Self::parse_u32_value(line).unwrap_or(48);
            } else if line.starts_with("EnableForcedMessages") {
                sm.enable_forced_messages = Self::parse_bool_value(line);
            } else if line.starts_with("ForcedMessages") && !line.contains("Enable") {
                let msg = Self::parse_string_value(line);
                if !msg.is_empty() {
                    sm.forced_messages = msg.split(',').map(|s| s.trim().to_string()).collect();
                }
            } else if line.starts_with("EnableForcedBanList") {
                sm.enable_forced_ban_list = Self::parse_bool_value(line);
            } else if line.starts_with("ForcedBanList") && !line.contains("Enable") {
                let ban = Self::parse_quoted_string_value(line);
                if !ban.is_empty() {
                    sm.forced_ban_list = ban.split(',').map(|s| s.trim().to_string()).collect();
                }
            }
            i += 1;
        }
        Ok(i)
    }

    fn parse_users(lines: &[&str], start: usize, users: &mut Vec<User>) -> Result<usize, String> {
        let mut i = start;
        while i < lines.len() {
            let line = lines[i].trim();
            if line.starts_with("</Users>") {
                return Ok(i);
            }
            
            if line.starts_with("user") {
                let parts: Vec<&str> = line.split('=').collect();
                if parts.len() >= 2 {
                    let user_str = parts[1].trim();
                    let user_parts: Vec<&str> = user_str.split(',').collect();
                    if user_parts.len() >= 3 {
                        users.push(User {
                            username: user_parts[0].trim_matches('"').to_string(),
                            password: user_parts[1].trim_matches('"').to_string(),
                            privilege_level: user_parts[2].trim().parse().unwrap_or(2),
                        });
                    }
                }
            }
            i += 1;
        }
        Ok(i)
    }

    fn parse_servers(lines: &[&str], start: usize, servers: &mut Vec<Server>) -> Result<usize, String> {
        let mut i = start;
        while i < lines.len() {
            let line = lines[i].trim();
            if line.starts_with("</Servers>") {
                return Ok(i);
            }
            
            if line.starts_with("<Server>") {
                let mut server = Server::default();
                i = Self::parse_server(&lines, i + 1, &mut server)?;
                servers.push(server);
            }
            i += 1;
        }
        Ok(i)
    }

    fn parse_server(lines: &[&str], start: usize, server: &mut Server) -> Result<usize, String> {
        let mut i = start;
        while i < lines.len() {
            let line = lines[i].trim();
            if line.starts_with("</Server>") {
                return Ok(i);
            }
            
            if line.starts_with("name") {
                server.name = Self::parse_quoted_string_value(line);
            } else if line.starts_with("running") {
                server.running = Self::parse_bool_value(line);
            } else if line.starts_with("watchdog") {
                server.watchdog = Self::parse_bool_value(line);
            } else if line.starts_with("messages") && !line.contains("Interval") {
                server.messages = Self::parse_bool_value(line);
            } else if line.starts_with("users") {
                let users_str = Self::parse_quoted_string_value(line);
                server.users = users_str.split(',').map(|s| s.trim().to_string()).collect();
            } else if line.starts_with("port") {
                server.port = Self::parse_u16_value(line).unwrap_or(22000);
            } else if line.starts_with("currentconfig") {
                server.current_config = Self::parse_quoted_string_value(line);
            } else if line.starts_with("<config>") {
                let mut config = ServerConfig::default();
                i = Self::parse_config_section(&lines, i + 1, &mut config)?;
                server.configs.push(config);
            }
            i += 1;
        }
        Ok(i)
    }

    fn parse_config_section(lines: &[&str], start: usize, config: &mut ServerConfig) -> Result<usize, String> {
        let mut i = start;
        while i < lines.len() {
            let line = lines[i].trim();
            if line.starts_with("</config>") {
                return Ok(i);
            }
            
            if line.starts_with("name") {
                config.name = Self::parse_quoted_string_value(line);
            } else if line.starts_with("domain") {
                config.domain = Self::parse_string_value(line);
            } else if line.starts_with("style") {
                config.style = Self::parse_string_value(line);
            } else if line.starts_with("sessionname") {
                config.session_name = Self::parse_quoted_string_value(line);
            } else if line.starts_with("maxclients") {
                config.max_clients = Self::parse_u8_value(line).unwrap_or(64);
            } else if line.starts_with("pointlimit") {
                config.point_limit = Self::parse_u8_value(line).unwrap_or(0);
            } else if line.starts_with("roundlimit") {
                config.round_limit = Self::parse_u8_value(line).unwrap_or(0);
            } else if line.starts_with("roundcount") {
                config.round_count = Self::parse_u8_value(line).unwrap_or(0);
            } else if line.starts_with("respawntime") {
                config.respawn_time = Self::parse_u16_value(line).unwrap_or(20);
            } else if line.starts_with("spawnprotection") {
                config.spawn_protection = Self::parse_u8_value(line).unwrap_or(0);
            } else if line.starts_with("warmup") {
                config.warmup = Self::parse_u8_value(line).unwrap_or(0);
            } else if line.starts_with("inversedamage") {
                config.inverse_damage = Self::parse_u8_value(line).unwrap_or(0);
            } else if line.starts_with("friendlyfire") {
                config.friendly_fire = Self::parse_bool_value(line);
            } else if line.starts_with("autoteambalance") {
                config.auto_team_balance = Self::parse_bool_value(line);
            } else if line.starts_with("3rdpersonview") {
                config.third_person_view = Self::parse_bool_value(line);
            } else if line.starts_with("allowcrosshair") {
                config.allow_crosshair = Self::parse_bool_value(line);
            } else if line.starts_with("fallingdmg") {
                config.falling_dmg = Self::parse_bool_value(line);
            } else if line.starts_with("allowrespawn") {
                config.allow_respawn = Self::parse_bool_value(line);
            } else if line.starts_with("allowvehicles") {
                config.allow_vehicles = Self::parse_bool_value(line);
            } else if line.starts_with("dificulty") {
                config.difficulty = Self::parse_string_value(line);
            } else if line.starts_with("respawnnumber") {
                config.respawn_number = Self::parse_i32_value(line).unwrap_or(0);
            } else if line.starts_with("teamrespawn") {
                config.team_respawn = Self::parse_bool_value(line);
            } else if line.starts_with("password") && !line.contains("admin") {
                config.password = Self::parse_quoted_string_value(line);
            } else if line.starts_with("adminpass") {
                config.admin_pass = Self::parse_quoted_string_value(line);
            } else if line.starts_with("maxping") {
                config.max_ping = Self::parse_u16_value(line).unwrap_or(0);
            } else if line.starts_with("maxfreq") {
                config.max_freq = Self::parse_u16_value(line).unwrap_or(0);
            } else if line.starts_with("maxinactivity") {
                config.max_inactivity = Self::parse_u16_value(line).unwrap_or(0);
            } else if line.starts_with("voicechat") {
                config.voice_chat = Self::parse_u8_value(line).unwrap_or(0);
            } else if line.starts_with("maps") {
                let maps_str = Self::parse_quoted_string_value(line);
                config.maps = maps_str.split(',').map(|s| s.trim().to_string()).collect();
            } else if line.starts_with("messages") && !line.contains("Interval") && !line.contains("Enable") {
                let msg = Self::parse_string_value(line);
                if !msg.is_empty() {
                    config.messages = msg.split(',').map(|s| s.trim().to_string()).collect();
                }
            } else if line.starts_with("banlist") {
                let ban = Self::parse_string_value(line);
                if !ban.is_empty() {
                    config.ban_list = ban.split(',').map(|s| s.trim().to_string()).collect();
                }
            } else if line.starts_with("enableautokick") {
                config.enable_auto_kick = Self::parse_bool_value(line);
            } else if line.starts_with("clantag") {
                config.clan_tag = Self::parse_quoted_string_value(line);
            } else if line.starts_with("clanside") {
                config.clan_side = Self::parse_string_value(line);
            } else if line.starts_with("clanreserve") {
                config.clan_reserve = Self::parse_u8_value(line).unwrap_or(0);
            }
            i += 1;
        }
        Ok(i)
    }

    fn parse_string_value(line: &str) -> String {
        let parts: Vec<&str> = line.split('=').collect();
        if parts.len() >= 2 {
            parts[1].trim().to_string()
        } else {
            String::new()
        }
    }

    fn parse_quoted_string_value(line: &str) -> String {
        let parts: Vec<&str> = line.split('=').collect();
        if parts.len() >= 2 {
            parts[1].trim().trim_matches('"').to_string()
        } else {
            String::new()
        }
    }

    fn parse_bool_value(line: &str) -> bool {
        let value = Self::parse_string_value(line);
        value.to_lowercase() == "true"
    }

    fn parse_u8_value(line: &str) -> Option<u8> {
        Self::parse_string_value(line).parse().ok()
    }

    fn parse_u16_value(line: &str) -> Option<u16> {
        Self::parse_string_value(line).parse().ok()
    }

    fn parse_u32_value(line: &str) -> Option<u32> {
        Self::parse_string_value(line).parse().ok()
    }

    fn parse_i32_value(line: &str) -> Option<i32> {
        Self::parse_string_value(line).parse().ok()
    }

    fn to_config_string(&self) -> String {
        let mut output = String::new();
        output.push_str("// HD2DS Server configuration file\n");
        output.push_str("// Generated and managed by Spectre\n");
        output.push_str("// Please edit this file using Spectre's Server Utility module\n");
        output.push_str("// Manual editing may cause formatting issues\n");
        output.push_str("// Value changes are allowed, but use Spectre for best results\n\n");
        output.push_str("// Part of service configuration\n\n");
        output.push_str("<ServerManager>\n\n");
        output.push_str(&format!("   ServerIP             = {}\n", self.server_manager.server_ip));
        output.push_str(&format!("   ServerPort           = {}\n", self.server_manager.server_port));
        output.push_str(&format!("   HD2DSPath            = \"{}\"\n\n", self.server_manager.hd2ds_path));
        output.push_str(&format!("   EnableWatchDog       = {}\n", self.bool_to_str(self.server_manager.enable_watchdog)));
        output.push_str(&format!("   WatchdogInterval     = {}\n\n", self.server_manager.watchdog_interval));
        output.push_str(&format!("   EnableMessaging      = {}\n", self.bool_to_str(self.server_manager.enable_messaging)));
        output.push_str(&format!("   MessagingInterval    = {}\n\n", self.server_manager.messaging_interval));
        output.push_str(&format!("   EnableReboot         = {}\n", self.bool_to_str(self.server_manager.enable_reboot)));
        output.push_str(&format!("   RebootInterval       = {}\n\n", self.server_manager.reboot_interval));
        output.push_str(&format!("   EnableForcedMessages = {}\n", self.bool_to_str(self.server_manager.enable_forced_messages)));
        output.push_str(&format!("   ForcedMessages       = {}\n\n", self.server_manager.forced_messages.join(",")));
        output.push_str(&format!("   EnableForcedBanList  = {}\n", self.bool_to_str(self.server_manager.enable_forced_ban_list)));
        output.push_str(&format!("   ForcedBanList        = \"{}\"\n\n", self.server_manager.forced_ban_list.join(",")));
        output.push_str("</ServerManager>\n\n");
        output.push_str("// Part of users configuration\n\n");
        output.push_str("<Users>\n\n");
        for user in &self.users {
            output.push_str(&format!("   user = \"{}\",\"{}\",{}\n\n", user.username, user.password, user.privilege_level));
        }
        output.push_str("</Users>\n\n");
        output.push_str("// Dedicated servers configurations\n\n");
        output.push_str("<Servers>\n\n");
        for server in &self.servers {
            output.push_str("   <Server>\n\n");
            output.push_str(&format!("      name          = \"{}\"\n", server.name));
            output.push_str(&format!("      running       = {}\n", self.bool_to_str(server.running)));
            output.push_str(&format!("      watchdog      = {}\n", self.bool_to_str(server.watchdog)));
            output.push_str(&format!("      messages      = {}\n\n", self.bool_to_str(server.messages)));
            output.push_str(&format!("      users         = \"{}\"\n\n", server.users.join(",")));
            output.push_str(&format!("      port          = {}\n\n", server.port));
            output.push_str(&format!("      currentconfig = \"{}\"\n\n", server.current_config));
            for config in &server.configs {
                output.push_str("      <config>\n\n");
                output.push_str(&format!("         name            = \"{}\"\n\n", config.name));
                output.push_str(&format!("         domain          = {}\n", config.domain));
                output.push_str(&format!("         style           = {}\n", config.style));
                output.push_str(&format!("         sessionname     = \"{}\"\n", config.session_name));
                output.push_str(&format!("         maxclients      = {}\n", config.max_clients));
                output.push_str(&format!("         pointlimit      = {}\n", config.point_limit));
                output.push_str(&format!("         roundlimit      = {}\n", config.round_limit));
                output.push_str(&format!("         roundcount      = {}\n", config.round_count));
                output.push_str(&format!("         respawntime     = {}\n", config.respawn_time));
                output.push_str(&format!("         spawnprotection = {}\n", config.spawn_protection));
                output.push_str(&format!("         warmup          = {}\n", config.warmup));
                output.push_str(&format!("         inversedamage   = {}\n", config.inverse_damage));
                output.push_str(&format!("         friendlyfire    = {}\n", self.bool_to_str(config.friendly_fire)));
                output.push_str(&format!("         autoteambalance = {}\n", self.bool_to_str(config.auto_team_balance)));
                output.push_str(&format!("         3rdpersonview   = {}\n", self.bool_to_str(config.third_person_view)));
                output.push_str(&format!("         allowcrosshair  = {}\n", self.bool_to_str(config.allow_crosshair)));
                output.push_str(&format!("         fallingdmg      = {}\n", self.bool_to_str(config.falling_dmg)));
                output.push_str(&format!("         allowrespawn    = {}\n", self.bool_to_str(config.allow_respawn)));
                output.push_str(&format!("         allowvehicles   = {}\n", self.bool_to_str(config.allow_vehicles)));
                output.push_str(&format!("         dificulty       = {}\n", config.difficulty));
                output.push_str(&format!("         respawnnumber   = {}\n", config.respawn_number));
                output.push_str(&format!("         teamrespawn     = {}\n\n", self.bool_to_str(config.team_respawn)));
                output.push_str(&format!("         password        = \"{}\"\n", config.password));
                output.push_str(&format!("         adminpass       = \"{}\"\n\n", config.admin_pass));
                output.push_str(&format!("         maxping         = {}\n", config.max_ping));
                output.push_str(&format!("         maxfreq         = {}\n", config.max_freq));
                output.push_str(&format!("         maxinactivity   = {}\n", config.max_inactivity));
                output.push_str(&format!("         voicechat       = {}\n\n", config.voice_chat));
                output.push_str(&format!("         maps            = \"{}\"\n\n", config.maps.join(",")));
                output.push_str(&format!("         messages        = {}\n", config.messages.join(",")));
                output.push_str(&format!("         banlist         = {}\n\n", config.ban_list.join(",")));
                output.push_str(&format!("         enableautokick  = {}\n", self.bool_to_str(config.enable_auto_kick)));
                output.push_str(&format!("         clantag         = \"{}\"\n", config.clan_tag));
                output.push_str(&format!("         clanside        = {}\n", config.clan_side));
                output.push_str(&format!("         clanreserve    = {}\n\n", config.clan_reserve));
                output.push_str("      </config>\n\n");
            }
            output.push_str("   </Server>\n\n");
        }
        output.push_str("</Servers>\n\n");
        output
    }

    fn bool_to_str(&self, value: bool) -> &str {
        if value { "true" } else { "false" }
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

pub struct ServerLauncher {
    data: ServerLauncherData,
    config_path: String,
    selected_server: Option<usize>,
    selected_config: Option<usize>,
    show_server_dialog: bool,
    show_config_dialog: bool,
    editing_server: Option<Server>,
    editing_config: Option<ServerConfig>,
    new_config_name: String,
    icon_textures: IconTextures,
}

struct IconTextures {
    new: Option<egui::TextureHandle>,
    edit: Option<egui::TextureHandle>,
    delete: Option<egui::TextureHandle>,
    save: Option<egui::TextureHandle>,
    active: Option<egui::TextureHandle>,
}

impl Default for ServerLauncher {
    fn default() -> Self {
        let config_path = "hd2_server_config.txt".to_string();
        let data = ServerLauncherData::load_from_file(Path::new(&config_path))
            .unwrap_or_else(|_| ServerLauncherData::default());
        
        Self {
            data,
            config_path,
            selected_server: None,
            selected_config: None,
            show_server_dialog: false,
            show_config_dialog: false,
            editing_server: None,
            editing_config: None,
            new_config_name: String::new(),
            icon_textures: IconTextures::default(),
        }
    }
}

impl IconTextures {
    fn load(ctx: &egui::Context) -> Self {
        let placeholder_bytes = include_bytes!("../../icons/placeholder.png");
        println!("[DEBUG] Placeholder icon bytes size: {} bytes", placeholder_bytes.len());
        
        let load_icon = |bytes: &[u8], id: &str| -> Option<egui::TextureHandle> {
            match image::load_from_memory(bytes) {
                Ok(image) => {
                    let rgba = image.to_rgba8();
                    let size = [rgba.width() as usize, rgba.height() as usize];
                    let pixels = rgba.as_flat_samples();
                    let color_image = egui::ColorImage::from_rgba_unmultiplied(size, pixels.as_slice());
                    println!("[DEBUG] Loaded icon {}: {}x{}", id, size[0], size[1]);
                    Some(ctx.load_texture(id, color_image, Default::default()))
                }
                Err(e) => {
                    println!("[DEBUG] Failed to load icon {}: {}", id, e);
                    None
                }
            }
        };
        
        Self {
            new: load_icon(placeholder_bytes, "icon_new"),
            edit: load_icon(placeholder_bytes, "icon_edit"),
            delete: load_icon(placeholder_bytes, "icon_delete"),
            save: load_icon(placeholder_bytes, "icon_save"),
            active: load_icon(placeholder_bytes, "icon_active"),
        }
    }
}

impl Default for IconTextures {
    fn default() -> Self {
        Self {
            new: None,
            edit: None,
            delete: None,
            save: None,
            active: None,
        }
    }
}

impl Module for ServerLauncher {
    fn name(&self) -> &str {
        "Server Launcher"
    }

    fn show(&mut self, ctx: &egui::Context, ui: &mut egui::Ui) {
        if self.icon_textures.new.is_none() {
            println!("[DEBUG] Loading toolbar icons...");
            self.icon_textures = IconTextures::load(ctx);
            println!("[DEBUG] Icons loaded - new: {}, edit: {}, delete: {}, save: {}, active: {}", 
                     self.icon_textures.new.is_some(),
                     self.icon_textures.edit.is_some(),
                     self.icon_textures.delete.is_some(),
                     self.icon_textures.save.is_some(),
                     self.icon_textures.active.is_some());
        }
        
        ui.vertical(|ui| {
            ui.heading("Server Utility");
            ui.separator();
            
            let available_width = ui.available_width();
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing = egui::vec2(6.0, 0.0);
                ui.set_max_width(available_width);
                
                ui.label(egui::RichText::new("Server:").strong().size(12.0));
                
                if Self::toolbar_button_with_icon(ui, self.icon_textures.new.as_ref(), "+", "New Server").clicked() {
                    let mut new_server = Server::default();
                    new_server.name = format!("Server {}", self.data.servers.len() + 1);
                    let mut default_config = ServerConfig::default();
                    default_config.name = "Config 1".to_string();
                    new_server.current_config = default_config.name.clone();
                    new_server.configs.push(default_config);
                    self.data.servers.push(new_server);
                    self.selected_server = Some(self.data.servers.len() - 1);
                    self.selected_config = Some(0);
                    let _ = self.data.save_to_file(Path::new(&self.config_path));
                }
                
                if let Some(idx) = self.selected_server {
                    if Self::toolbar_button_with_icon(ui, self.icon_textures.edit.as_ref(), "✎", "Edit Server").clicked() {
                        self.editing_server = Some(self.data.servers[idx].clone());
                        self.show_server_dialog = true;
                    }
                    
                    if Self::toolbar_button_with_icon(ui, self.icon_textures.delete.as_ref(), "×", "Delete Server").clicked() {
                        self.data.servers.remove(idx);
                        self.selected_server = None;
                        self.selected_config = None;
                        let _ = self.data.save_to_file(Path::new(&self.config_path));
                    }
                    
                    ui.add_space(12.0);
                    ui.separator();
                    ui.add_space(12.0);
                    
                    ui.label(egui::RichText::new("Config:").strong().size(12.0));
                    
                    if Self::toolbar_button_with_icon(ui, self.icon_textures.new.as_ref(), "+", "New Config").clicked() {
                        let configs_count = self.data.servers[idx].configs.len();
                        self.new_config_name = format!("Config {}", configs_count + 1);
                        let mut new_config = ServerConfig::default();
                        new_config.name = self.new_config_name.clone();
                        self.editing_config = Some(new_config);
                        self.show_config_dialog = true;
                    }
                    
                    if let Some(config_idx) = self.selected_config {
                        if Self::toolbar_button_with_icon(ui, self.icon_textures.edit.as_ref(), "✎", "Edit Config").clicked() {
                            self.editing_config = Some(self.data.servers[idx].configs[config_idx].clone());
                            self.show_config_dialog = true;
                        }
                        
                        if Self::toolbar_button_with_icon(ui, self.icon_textures.delete.as_ref(), "×", "Delete Config").clicked() {
                            self.data.servers[idx].configs.remove(config_idx);
                            self.selected_config = None;
                            let _ = self.data.save_to_file(Path::new(&self.config_path));
                        }
                        
                        if Self::toolbar_button_with_icon(ui, self.icon_textures.active.as_ref(), "✓", "Set Active Config").clicked() {
                            self.data.servers[idx].current_config = self.data.servers[idx].configs[config_idx].name.clone();
                            let _ = self.data.save_to_file(Path::new(&self.config_path));
                        }
                    }
                }
                
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if Self::toolbar_button_with_icon(ui, self.icon_textures.save.as_ref(), "💾", "Save Configuration").clicked() {
                        if let Err(e) = self.data.save_to_file(Path::new(&self.config_path)) {
                            println!("[DEBUG] Failed to save config: {}", e);
                        } else {
                            println!("[DEBUG] Configuration saved successfully");
                        }
                    }
                });
            });
            
            ui.add_space(15.0);
            ui.separator();
            ui.add_space(10.0);

            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    ui.set_min_width(280.0);
                    ui.label(egui::RichText::new("Servers").strong().size(16.0));
                    ui.separator();
                    ui.add_space(5.0);
                    
                    egui::ScrollArea::vertical()
                        .id_source("servers_list")
                        .show(ui, |ui| {
                            for (idx, server) in self.data.servers.iter().enumerate() {
                                let is_selected = self.selected_server == Some(idx);
                                let label = if is_selected {
                                    egui::RichText::new(&server.name).strong()
                                } else {
                                    egui::RichText::new(&server.name)
                                };
                                
                                if ui.selectable_label(is_selected, label).clicked() {
                                    self.selected_server = Some(idx);
                                    self.selected_config = if !server.configs.is_empty() { Some(0) } else { None };
                                }
                            }
                            
                            if self.data.servers.is_empty() {
                                ui.label(egui::RichText::new("No servers configured").italics().color(egui::Color32::GRAY));
                            }
                        });
                });

                ui.add_space(20.0);
                ui.separator();
                ui.add_space(20.0);

                ui.vertical(|ui| {
                    ui.set_min_width(280.0);
                    if let Some(server_idx) = self.selected_server {
                        let current_config_name = self.data.servers[server_idx].current_config.clone();
                        
                        ui.label(egui::RichText::new("Configurations").strong().size(16.0));
                        ui.separator();
                        ui.add_space(5.0);
                        
                        let mut clicked_config = None;
                        
                        egui::ScrollArea::vertical()
                            .id_source("configs_list")
                            .show(ui, |ui| {
                                for (idx, config) in self.data.servers[server_idx].configs.iter().enumerate() {
                                    let is_selected = self.selected_config == Some(idx);
                                    let is_active = config.name == current_config_name;
                                    let label_text = if is_active {
                                        format!("✓ {}", config.name)
                                    } else {
                                        config.name.clone()
                                    };
                                    let label = if is_selected {
                                        egui::RichText::new(&label_text).strong()
                                    } else {
                                        egui::RichText::new(&label_text)
                                    };
                                    
                                    if ui.selectable_label(is_selected, label).clicked() {
                                        clicked_config = Some(idx);
                                    }
                                }
                                
                                if self.data.servers[server_idx].configs.is_empty() {
                                    ui.label(egui::RichText::new("No configurations").italics().color(egui::Color32::GRAY));
                                }
                            });

                        if let Some(idx) = clicked_config {
                            self.selected_config = Some(idx);
                        }
                    } else {
                        ui.label(egui::RichText::new("Select a server to view configurations").italics().color(egui::Color32::GRAY));
                    }
                });
            });

            ui.add_space(15.0);
            ui.separator();
            ui.add_space(5.0);
            
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new(format!("Config: {}", self.config_path)).size(12.0).color(egui::Color32::GRAY));
            });
        });

        if self.show_server_dialog {
            self.show_server_dialog(ctx);
        }

        if self.show_config_dialog {
            self.show_config_dialog(ctx);
        }
    }
}

impl ServerLauncher {
    fn calculate_window_size(ctx: &egui::Context, preferred_width: f32, preferred_height: f32) -> egui::Vec2 {
        let screen_rect = ctx.screen_rect();
        let available_size = screen_rect.size();
        
        let max_width = (available_size.x * 0.9).min(preferred_width);
        let max_height = (available_size.y * 0.9).min(preferred_height);
        
        egui::vec2(max_width, max_height)
    }
    
    fn toolbar_button(ui: &mut egui::Ui, icon: &str, tooltip: &str) -> egui::Response {
        let button_size = egui::vec2(32.0, 32.0);
        let button = egui::Button::new(icon)
            .min_size(button_size)
            .frame(false);
        let response = ui.add(button);
        
        if response.hovered() {
            let rect = response.rect;
            ui.painter().rect_filled(
                rect,
                2.0,
                ui.style().visuals.widgets.hovered.bg_fill,
            );
        }
        
        response.on_hover_text(tooltip)
    }
    
    fn toolbar_button_with_icon(ui: &mut egui::Ui, icon_texture: Option<&egui::TextureHandle>, icon: &str, tooltip: &str) -> egui::Response {
        let button_size = egui::vec2(28.0, 28.0);
        let icon_size = egui::vec2(16.0, 16.0);
        
        let response = if let Some(texture) = icon_texture {
            let button = egui::Button::image((texture.id(), icon_size))
                .min_size(button_size);
            ui.add(button).on_hover_text(tooltip)
        } else {
            let button = egui::Button::new(icon)
                .min_size(button_size);
            ui.add(button).on_hover_text(tooltip)
        };
        
        response
    }
    
    fn show_server_dialog(&mut self, ctx: &egui::Context) {
        let window_size = Self::calculate_window_size(ctx, 450.0, 300.0);
        egui::Window::new("Server Settings")
            .collapsible(false)
            .resizable(true)
            .default_size(window_size)
            .constrain(true)
            .show(ctx, |ui| {
                if let Some(ref mut server) = self.editing_server {
                    let mut save_clicked = false;
                    let mut cancel_clicked = false;
                    
                    ui.vertical(|ui| {
                        ui.label("Server Name:");
                        ui.text_edit_singleline(&mut server.name);
                        
                        ui.add_space(15.0);
                        
                        ui.horizontal(|ui| {
                            ui.label("Port:");
                            let mut port_str = server.port.to_string();
                            if ui.text_edit_singleline(&mut port_str).changed() {
                                if let Ok(port) = port_str.parse::<u16>() {
                                    if port >= 1024 {
                                        server.port = port;
                                    }
                                }
                            }
                        });
                        
                        ui.add_space(15.0);
                        
                        ui.checkbox(&mut server.watchdog, "Enable Watchdog");
                        ui.checkbox(&mut server.messages, "Enable Messages");
                        
                        ui.add_space(20.0);
                        ui.separator();
                        ui.add_space(10.0);
                        
                        ui.horizontal(|ui| {
                            if ui.button("Save").clicked() {
                                save_clicked = true;
                            }
                            
                            if ui.button("Cancel").clicked() {
                                cancel_clicked = true;
                            }
                        });
                    });
                    
                    if save_clicked {
                        let server_clone = server.clone();
                        let selected_idx = self.selected_server;
                        if let Some(idx) = selected_idx {
                            self.data.servers[idx] = server_clone;
                        } else {
                            self.data.servers.push(server_clone);
                        }
                        let _ = self.data.save_to_file(Path::new(&self.config_path));
                        self.show_server_dialog = false;
                        self.editing_server = None;
                    }
                    
                    if cancel_clicked {
                        self.show_server_dialog = false;
                        self.editing_server = None;
                    }
                }
            });
    }

    fn show_config_dialog(&mut self, ctx: &egui::Context) {
        let window_size = Self::calculate_window_size(ctx, 900.0, 750.0);
        egui::Window::new("Configuration Settings")
            .collapsible(false)
            .resizable(true)
            .default_size(window_size)
            .constrain(true)
            .show(ctx, |ui| {
                if let Some(ref mut config) = self.editing_config {
                    let mut save_clicked = false;
                    let mut cancel_clicked = false;
                    let style = config.style.clone();
                    let available_maps = Self::get_available_maps_static(&style);
                    
                    let available_rect = ui.available_rect_before_wrap();
                    let column_width = (available_rect.width() - 20.0) / 2.0;
                    
                    egui::ScrollArea::vertical()
                        .id_source("config_dialog_scroll")
                        .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.vertical(|ui| {
                            ui.set_min_width(column_width);
                            ui.set_max_width(column_width);
                            
                            ui.label(egui::RichText::new("Basic Settings").strong().size(14.0));
                            ui.separator();
                            ui.add_space(5.0);
                            
                            ui.label("Configuration Name:");
                            ui.text_edit_singleline(&mut config.name);
                            
                            ui.add_space(10.0);
                            
                            ui.horizontal(|ui| {
                                ui.label("Domain:");
                                egui::ComboBox::from_id_source("domain")
                                    .selected_text(&config.domain)
                                    .show_ui(ui, |ui| {
                                        ui.selectable_value(&mut config.domain, "Internet".to_string(), "Internet");
                                        ui.selectable_value(&mut config.domain, "Local".to_string(), "Local");
                                    });
                                
                                ui.add_space(20.0);
                                
                                ui.label("Game Style:");
                                egui::ComboBox::from_id_source("style")
                                    .selected_text(&config.style)
                                    .show_ui(ui, |ui| {
                                        ui.selectable_value(&mut config.style, "Cooperative".to_string(), "Cooperative");
                                        ui.selectable_value(&mut config.style, "Occupation".to_string(), "Occupation");
                                        ui.selectable_value(&mut config.style, "Objectives".to_string(), "Objectives");
                                        ui.selectable_value(&mut config.style, "Deathmatch".to_string(), "Deathmatch");
                                    });
                            });
                            
                            ui.add_space(10.0);
                            ui.label("Session Name:");
                            ui.text_edit_singleline(&mut config.session_name);
                            
                            ui.add_space(15.0);
                            ui.separator();
                            ui.add_space(10.0);
                            
                            ui.label(egui::RichText::new("Game Limits").strong().size(14.0));
                            ui.separator();
                            ui.add_space(5.0);
                            
                            egui::Grid::new("config_grid")
                                .num_columns(2)
                                .spacing([20.0, 8.0])
                                .show(ui, |ui| {
                                    ui.label("Max Clients:");
                                    ui.horizontal(|ui| {
                                        ui.add(egui::Slider::new(&mut config.max_clients, 1..=64).show_value(false));
                                        ui.label(format!("{}", config.max_clients));
                                    });
                                    ui.end_row();
                                    
                                    ui.label("Point Limit:");
                                    ui.horizontal(|ui| {
                                        ui.add(egui::Slider::new(&mut config.point_limit, 0..=255).show_value(false));
                                        ui.label(if config.point_limit == 0 { "no limit".to_string() } else { config.point_limit.to_string() });
                                    });
                                    ui.end_row();
                                    
                                    ui.label("Round Limit:");
                                    ui.horizontal(|ui| {
                                        ui.add(egui::Slider::new(&mut config.round_limit, 0..=255).show_value(false));
                                        ui.label(if config.round_limit == 0 { "no limit".to_string() } else { format!("{} min", config.round_limit) });
                                    });
                                    ui.end_row();
                                    
                                    ui.label("Round Count:");
                                    ui.horizontal(|ui| {
                                        ui.add(egui::Slider::new(&mut config.round_count, 0..=255).show_value(false));
                                        ui.label(if config.round_count == 0 { "no limit".to_string() } else { config.round_count.to_string() });
                                    });
                                    ui.end_row();
                                    
                                    ui.label("Respawn Time:");
                                    ui.horizontal(|ui| {
                                        ui.add(egui::Slider::new(&mut config.respawn_time, 0..=60).show_value(false));
                                        ui.label(format!("{} sec", config.respawn_time));
                                    });
                                    ui.end_row();
                                    
                                    ui.label("Spawn Protection:");
                                    ui.horizontal(|ui| {
                                        ui.add(egui::Slider::new(&mut config.spawn_protection, 0..=60).show_value(false));
                                        ui.label(format!("{} sec", config.spawn_protection));
                                    });
                                    ui.end_row();
                                    
                                    ui.label("Warmup:");
                                    ui.horizontal(|ui| {
                                        ui.add(egui::Slider::new(&mut config.warmup, 0..=60).show_value(false));
                                        ui.label(format!("{} sec", config.warmup));
                                    });
                                    ui.end_row();
                                    
                                    ui.label("Difficulty:");
                                    egui::ComboBox::from_id_source("difficulty")
                                        .selected_text(&config.difficulty)
                                        .show_ui(ui, |ui| {
                                            ui.selectable_value(&mut config.difficulty, "Easy".to_string(), "Easy");
                                            ui.selectable_value(&mut config.difficulty, "Normal".to_string(), "Normal");
                                            ui.selectable_value(&mut config.difficulty, "Hard".to_string(), "Hard");
                                        });
                                    ui.end_row();
                                    
                                    ui.label("Password:");
                                    ui.text_edit_singleline(&mut config.password);
                                    ui.end_row();
                                    
                                    ui.label("Admin Password:");
                                    ui.text_edit_singleline(&mut config.admin_pass);
                                    ui.end_row();
                                    
                                    ui.label("Max Ping:");
                                    ui.add(egui::Slider::new(&mut config.max_ping, 0..=1000).show_value(false));
                                    ui.end_row();
                                    
                                    ui.label("Max Frequency:");
                                    ui.add(egui::Slider::new(&mut config.max_freq, 0..=100).show_value(false));
                                    ui.end_row();
                                });
                            
                            ui.add_space(15.0);
                            ui.separator();
                            ui.add_space(10.0);
                            
                            ui.label(egui::RichText::new("Game Options").strong().size(14.0));
                            ui.separator();
                            ui.add_space(5.0);
                            
                            ui.horizontal(|ui| {
                                ui.vertical(|ui| {
                                    ui.checkbox(&mut config.friendly_fire, "Friendly Fire");
                                    ui.checkbox(&mut config.auto_team_balance, "Auto Team Balance");
                                    ui.checkbox(&mut config.third_person_view, "3rd Person View");
                                    ui.checkbox(&mut config.allow_crosshair, "Allow Crosshair");
                                    ui.checkbox(&mut config.falling_dmg, "Falling Damage");
                                });
                                
                                ui.add_space(20.0);
                                
                                ui.vertical(|ui| {
                                    ui.checkbox(&mut config.allow_respawn, "Allow Respawn");
                                    ui.checkbox(&mut config.allow_vehicles, "Allow Vehicles");
                                    ui.checkbox(&mut config.team_respawn, "Team Respawn");
                                    ui.checkbox(&mut config.enable_auto_kick, "Enable Auto Kick");
                                });
                            });
                        });
                        
                        ui.add_space(15.0);
                        ui.separator();
                        ui.add_space(15.0);
                        
                        ui.vertical(|ui| {
                            ui.set_min_width(column_width);
                            ui.set_max_width(column_width);
                            
                            ui.label(egui::RichText::new("Map Rotation").strong().size(14.0));
                            ui.separator();
                            ui.add_space(5.0);
                            
                            ui.horizontal(|ui| {
                                ui.vertical(|ui| {
                                    ui.set_min_width(220.0);
                                    ui.label("Available Maps");
                                    ui.separator();
                                    egui::ScrollArea::vertical()
                                        .id_source("available_maps")
                                        .max_height(200.0)
                                        .show(ui, |ui| {
                                            for map in available_maps {
                                                if !config.maps.contains(&map) {
                                                    if ui.button(&map).clicked() {
                                                        config.maps.push(map.clone());
                                                    }
                                                }
                                            }
                                        });
                                });
                                
                                ui.add_space(10.0);
                                
                                ui.vertical(|ui| {
                                    ui.set_min_width(240.0);
                                    ui.label("Selected Maps (Rotation Order)");
                                    ui.separator();
                                    let mut to_remove = None;
                                    let mut move_up = None;
                                    let mut move_down = None;
                                    
                                    egui::ScrollArea::vertical()
                                        .id_source("selected_maps")
                                        .max_height(200.0)
                                        .show(ui, |ui| {
                                            for (idx, map) in config.maps.iter().enumerate() {
                                                ui.horizontal(|ui| {
                                                    ui.label(map);
                                                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                                        if idx < config.maps.len() - 1 && ui.small_button("↓").clicked() {
                                                            move_down = Some(idx);
                                                        }
                                                        if idx > 0 && ui.small_button("↑").clicked() {
                                                            move_up = Some(idx);
                                                        }
                                                        if ui.small_button("×").clicked() {
                                                            to_remove = Some(idx);
                                                        }
                                                    });
                                                });
                                            }
                                            
                                            if config.maps.is_empty() {
                                                ui.label(egui::RichText::new("No maps selected").italics().color(egui::Color32::GRAY));
                                            }
                                        });
                                    
                                    if let Some(idx) = to_remove {
                                        config.maps.remove(idx);
                                    }
                                    if let Some(idx) = move_up {
                                        config.maps.swap(idx, idx - 1);
                                    }
                                    if let Some(idx) = move_down {
                                        config.maps.swap(idx, idx + 1);
                                    }
                                });
                            });
                            
                            ui.add_space(15.0);
                            ui.separator();
                            ui.add_space(10.0);
                            
                            ui.label(egui::RichText::new("Ban List").strong().size(14.0));
                            ui.separator();
                            ui.add_space(5.0);
                            
                            let mut to_remove = None;
                            
                            egui::ScrollArea::vertical()
                                .id_source("ban_list")
                                .max_height(120.0)
                                .show(ui, |ui| {
                                    let ban_list_len = config.ban_list.len();
                                    for idx in 0..ban_list_len {
                                        ui.horizontal(|ui| {
                                            ui.text_edit_singleline(&mut config.ban_list[idx]);
                                            if ui.button("Remove").clicked() {
                                                to_remove = Some(idx);
                                            }
                                        });
                                    }
                                    
                                    if config.ban_list.is_empty() {
                                        ui.label(egui::RichText::new("No ban entries").italics().color(egui::Color32::GRAY));
                                    }
                                });
                            
                            if let Some(idx) = to_remove {
                                config.ban_list.remove(idx);
                            }
                            
                            ui.add_space(5.0);
                            if ui.button("Add Ban Entry").clicked() {
                                config.ban_list.push(String::new());
                            }
                        });
                    });
                        });
                    
                    ui.add_space(15.0);
                    ui.separator();
                    ui.add_space(10.0);
                    
                    ui.horizontal(|ui| {
                        if ui.button("Save").clicked() {
                            save_clicked = true;
                        }
                        
                        if ui.button("Cancel").clicked() {
                            cancel_clicked = true;
                        }
                    });
                    
                    if save_clicked {
                        let config_clone = config.clone();
                        let server_idx = self.selected_server;
                        let config_idx = self.selected_config;
                        if let Some(server_idx) = server_idx {
                            if let Some(config_idx) = config_idx {
                                self.data.servers[server_idx].configs[config_idx] = config_clone;
                            } else {
                                self.data.servers[server_idx].configs.push(config_clone);
                            }
                        }
                        let _ = self.data.save_to_file(Path::new(&self.config_path));
                        self.show_config_dialog = false;
                        self.editing_config = None;
                    }
                    
                    if cancel_clicked {
                        self.show_config_dialog = false;
                        self.editing_config = None;
                    }
                }
            });
    }

    fn get_available_maps_static(style: &str) -> Vec<String> {
        match style {
            "Cooperative" => vec![
                "Brest".to_string(),
                "Burma1".to_string(),
                "Africa1".to_string(),
                "Norway1".to_string(),
                "Crete1".to_string(),
            ],
            "Occupation" => vec![
                "Brest".to_string(),
                "Burma1".to_string(),
                "Africa1".to_string(),
                "Norway1".to_string(),
                "Crete1".to_string(),
            ],
            "Objectives" => vec![
                "Brest".to_string(),
                "Burma1".to_string(),
                "Africa1".to_string(),
                "Norway1".to_string(),
                "Crete1".to_string(),
            ],
            "Deathmatch" => vec![
                "Brest".to_string(),
                "Burma1".to_string(),
                "Africa1".to_string(),
                "Norway1".to_string(),
                "Crete1".to_string(),
            ],
            _ => Vec::new(),
        }
    }
}
