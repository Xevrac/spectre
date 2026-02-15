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
            use_sabre_squadron: false,
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

    fn parse_server_manager(
        lines: &[&str],
        start: usize,
        sm: &mut ServerManager,
    ) -> Result<usize, String> {
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

    fn parse_servers(
        lines: &[&str],
        start: usize,
        servers: &mut Vec<Server>,
    ) -> Result<usize, String> {
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

    fn parse_server(
        lines: &[&str],
        start: usize,
        server: &mut Server,
    ) -> Result<usize, String> {
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
            } else if line.starts_with("usesabresquadron") {
                server.use_sabre_squadron = Self::parse_bool_value(line);
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

    fn parse_config_section(
        lines: &[&str],
        start: usize,
        config: &mut ServerConfig,
    ) -> Result<usize, String> {
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
            } else if line.starts_with("messages")
                && !line.contains("Interval")
                && !line.contains("Enable")
            {
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
        output.push_str(&format!(
            "   ServerIP             = {}\n",
            self.server_manager.server_ip
        ));
        output.push_str(&format!(
            "   ServerPort           = {}\n",
            self.server_manager.server_port
        ));
        output.push_str(&format!(
            "   EnableWatchDog       = {}\n",
            self.bool_to_str(self.server_manager.enable_watchdog)
        ));
        output.push_str(&format!(
            "   WatchdogInterval     = {}\n\n",
            self.server_manager.watchdog_interval
        ));
        output.push_str(&format!(
            "   EnableMessaging      = {}\n",
            self.bool_to_str(self.server_manager.enable_messaging)
        ));
        output.push_str(&format!(
            "   MessagingInterval    = {}\n\n",
            self.server_manager.messaging_interval
        ));
        output.push_str(&format!(
            "   EnableReboot         = {}\n",
            self.bool_to_str(self.server_manager.enable_reboot)
        ));
        output.push_str(&format!(
            "   RebootInterval       = {}\n\n",
            self.server_manager.reboot_interval
        ));
        output.push_str(&format!(
            "   EnableForcedMessages = {}\n",
            self.bool_to_str(self.server_manager.enable_forced_messages)
        ));
        output.push_str(&format!(
            "   ForcedMessages       = {}\n\n",
            self.server_manager.forced_messages.join(",")
        ));
        output.push_str(&format!(
            "   EnableForcedBanList  = {}\n",
            self.bool_to_str(self.server_manager.enable_forced_ban_list)
        ));
        output.push_str(&format!(
            "   ForcedBanList        = \"{}\"\n\n",
            self.server_manager.forced_ban_list.join(",")
        ));
        output.push_str("</ServerManager>\n\n");
        output.push_str("// Part of users configuration\n\n");
        output.push_str("<Users>\n\n");
        for user in &self.users {
            output.push_str(&format!(
                "   user = \"{}\",\"{}\",{}\n\n",
                user.username, user.password, user.privilege_level
            ));
        }
        output.push_str("</Users>\n\n");
        output.push_str("// Dedicated servers configurations\n\n");
        output.push_str("<Servers>\n\n");
        for server in &self.servers {
            output.push_str("   <Server>\n\n");
            output.push_str(&format!("      name          = \"{}\"\n", server.name));
            output.push_str(&format!(
                "      running       = {}\n",
                self.bool_to_str(server.running)
            ));
            output.push_str(&format!(
                "      watchdog      = {}\n",
                self.bool_to_str(server.watchdog)
            ));
            output.push_str(&format!(
                "      messages      = {}\n\n",
                self.bool_to_str(server.messages)
            ));
            output.push_str(&format!("      users         = \"{}\"\n\n", server.users.join(",")));
            output.push_str(&format!("      port          = {}\n\n", server.port));
            output.push_str(&format!(
                "      usesabresquadron = {}\n\n",
                self.bool_to_str(server.use_sabre_squadron)
            ));
            output.push_str(&format!(
                "      currentconfig = \"{}\"\n\n",
                server.current_config
            ));
            for config in &server.configs {
                output.push_str("      <config>\n\n");
                output.push_str(&format!("         name            = \"{}\"\n\n", config.name));
                output.push_str(&format!("         domain          = {}\n", config.domain));
                output.push_str(&format!("         style           = {}\n", config.style));
                output.push_str(&format!(
                    "         sessionname     = \"{}\"\n",
                    config.session_name
                ));
                output.push_str(&format!(
                    "         maxclients      = {}\n",
                    config.max_clients
                ));
                output.push_str(&format!(
                    "         pointlimit      = {}\n",
                    config.point_limit
                ));
                output.push_str(&format!(
                    "         roundlimit      = {}\n",
                    config.round_limit
                ));
                output.push_str(&format!(
                    "         roundcount      = {}\n",
                    config.round_count
                ));
                output.push_str(&format!(
                    "         respawntime     = {}\n",
                    config.respawn_time
                ));
                output.push_str(&format!(
                    "         spawnprotection = {}\n",
                    config.spawn_protection
                ));
                output.push_str(&format!("         warmup          = {}\n", config.warmup));
                output.push_str(&format!(
                    "         inversedamage   = {}\n",
                    config.inverse_damage
                ));
                output.push_str(&format!(
                    "         friendlyfire    = {}\n",
                    self.bool_to_str(config.friendly_fire)
                ));
                output.push_str(&format!(
                    "         autoteambalance = {}\n",
                    self.bool_to_str(config.auto_team_balance)
                ));
                output.push_str(&format!(
                    "         3rdpersonview   = {}\n",
                    self.bool_to_str(config.third_person_view)
                ));
                output.push_str(&format!(
                    "         allowcrosshair  = {}\n",
                    self.bool_to_str(config.allow_crosshair)
                ));
                output.push_str(&format!(
                    "         fallingdmg      = {}\n",
                    self.bool_to_str(config.falling_dmg)
                ));
                output.push_str(&format!(
                    "         allowrespawn    = {}\n",
                    self.bool_to_str(config.allow_respawn)
                ));
                output.push_str(&format!(
                    "         allowvehicles   = {}\n",
                    self.bool_to_str(config.allow_vehicles)
                ));
                output.push_str(&format!("         dificulty       = {}\n", config.difficulty));
                output.push_str(&format!(
                    "         respawnnumber   = {}\n",
                    config.respawn_number
                ));
                output.push_str(&format!(
                    "         teamrespawn     = {}\n\n",
                    self.bool_to_str(config.team_respawn)
                ));
                output.push_str(&format!(
                    "         password        = \"{}\"\n",
                    config.password
                ));
                output.push_str(&format!(
                    "         adminpass       = \"{}\"\n\n",
                    config.admin_pass
                ));
                output.push_str(&format!("         maxping         = {}\n", config.max_ping));
                output.push_str(&format!("         maxfreq         = {}\n", config.max_freq));
                output.push_str(&format!(
                    "         maxinactivity   = {}\n",
                    config.max_inactivity
                ));
                output.push_str(&format!("         voicechat       = {}\n\n", config.voice_chat));
                output.push_str(&format!(
                    "         maps            = \"{}\"\n\n",
                    config.maps.join(",")
                ));
                output.push_str(&format!(
                    "         messages        = {}\n",
                    config.messages.join(",")
                ));
                output.push_str(&format!(
                    "         banlist         = {}\n\n",
                    config.ban_list.join(",")
                ));
                output.push_str(&format!(
                    "         enableautokick  = {}\n",
                    self.bool_to_str(config.enable_auto_kick)
                ));
                output.push_str(&format!(
                    "         clantag         = \"{}\"\n",
                    config.clan_tag
                ));
                output.push_str(&format!("         clanside        = {}\n", config.clan_side));
                output.push_str(&format!(
                    "         clanreserve    = {}\n\n",
                    config.clan_reserve
                ));
                output.push_str("      </config>\n\n");
            }
            output.push_str("   </Server>\n\n");
        }
        output.push_str("</Servers>\n\n");
        output
    }

    fn bool_to_str(&self, value: bool) -> &str {
        if value {
            "true"
        } else {
            "false"
        }
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


