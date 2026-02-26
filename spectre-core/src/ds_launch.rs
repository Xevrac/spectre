//! HD2 dedicated server script builder.

use crate::server::{Server, ServerConfig};
use std::path::Path;
use std::process::Command;

/// Build HD2 DS console commands from server and current config. Order matches known working scripts.
pub fn build_ds_script(server: &Server, config: &ServerConfig) -> Vec<String> {
    let mut lines = Vec::new();
    let add = |lines: &mut Vec<String>, s: String| lines.push(s);

    add(
        &mut lines,
        format!("sessionname \"{}\"", config.session_name),
    );
    add(&mut lines, format!("style {}", config.style.to_lowercase()));
    for map in &config.maps {
        add(&mut lines, format!("mapname {}", map));
    }
    add(
        &mut lines,
        format!("domain {}", config.domain.to_lowercase()),
    );
    add(&mut lines, "dedicated 1".to_string());
    if config.domain.to_lowercase() != "local" {
        add(&mut lines, format!("port {}", server.port));
    }
    add(&mut lines, format!("password \"{}\"", config.password));
    if !config.admin_pass.is_empty() {
        add(&mut lines, format!("adminpass \"{}\"", config.admin_pass));
    }
    add(&mut lines, format!("maxclients {}", config.max_clients));
    add(&mut lines, format!("pointlimit {}", config.point_limit));
    add(&mut lines, format!("roundlimit {}", config.round_limit));
    add(&mut lines, format!("roundcount {}", config.round_count));
    add(&mut lines, format!("warmup {}", config.warmup));
    add(&mut lines, format!("respawntime {}", config.respawn_time));
    if config.allow_respawn {
        add(&mut lines, "allowrespawn 1".to_string());
    } else {
        add(&mut lines, "allowrespawn 0".to_string());
    }
    if config.friendly_fire {
        add(&mut lines, "friendlyfire 1".to_string());
    } else {
        add(&mut lines, "friendlyfire 0".to_string());
    }
    if config.auto_team_balance {
        add(&mut lines, "autoteambalance 1".to_string());
    } else {
        add(&mut lines, "autoteambalance 0".to_string());
    }
    if config.third_person_view {
        add(&mut lines, "3rdpersonview 1".to_string());
    } else {
        add(&mut lines, "3rdpersonview 0".to_string());
    }
    add(
        &mut lines,
        format!("spawnprotection {}", config.spawn_protection),
    );
    add(
        &mut lines,
        format!("inversedamage {}", config.inverse_damage),
    );
    if config.falling_dmg {
        add(&mut lines, "fallingdmg 1".to_string());
    } else {
        add(&mut lines, "fallingdmg 0".to_string());
    }
    add(&mut lines, format!("maxfreq {}", config.max_freq));
    add(&mut lines, format!("maxping {}", config.max_ping));
    add(
        &mut lines,
        format!("maxinactivity {}", config.max_inactivity),
    );
    if config.allow_vehicles {
        add(&mut lines, "allowvehicles 1".to_string());
    } else {
        add(&mut lines, "allowvehicles 0".to_string());
    }
    add(&mut lines, "autorestart 0".to_string());
    let d = config.difficulty.to_lowercase();
    let coopdiff = if d == "easy" {
        "1"
    } else if d == "normal" {
        "2"
    } else if d == "hard" {
        "3"
    } else if d == "very hard" {
        "4"
    } else {
        "3"
    };
    add(&mut lines, format!("coopdifficulty {}", coopdiff));
    let cooplives = if config.respawn_number == 0 {
        "-1".to_string()
    } else {
        config.respawn_number.to_string()
    };
    add(&mut lines, format!("cooplives {}", cooplives));
    if config.allow_crosshair {
        add(&mut lines, "allowcrosshair 1".to_string());
    } else {
        add(&mut lines, "allowcrosshair 0".to_string());
    }
    add(&mut lines, "spawnonstart 0".to_string());
    if config.team_respawn {
        add(&mut lines, "teamlives 1".to_string());
    } else {
        add(&mut lines, "teamlives 0".to_string());
    }
    if config.voice_chat != 0 {
        let voice = match config.voice_chat {
            1 => "vr12",
            2 => "sc03",
            3 => "sc06",
            4 => "truespeech",
            5 => "gsm",
            6 => "adpcm",
            7 => "pcm",
            _ => "none",
        };
        add(&mut lines, format!("voicechat {}", voice));
    }
    add(&mut lines, "server".to_string());

    lines
}

/// Write script lines to a file next to the DS exe.
pub fn write_script_to_ds_dir(
    script: &[String],
    ds_exe_path: &Path,
    commands_basename: &str,
) -> Result<std::path::PathBuf, String> {
    let parent = ds_exe_path
        .parent()
        .ok_or_else(|| "DS exe path has no parent directory".to_string())?;
    let target = parent.join(commands_basename);
    let content = format!("{}\r\n\r\n", script.join("\r\n"));
    std::fs::write(&target, content)
        .map_err(|e| format!("Failed to write {}: {}", target.display(), e))?;
    Ok(target)
}

/// DS exe path (HD2DS vs Sabre Squadron) from this server's per-server paths.
fn get_ds_exe_path(server: &Server) -> Result<&str, String> {
    let path = if server.use_sabre_squadron {
        server.hd2ds_sabresquadron_path.as_str()
    } else {
        server.hd2ds_path.as_str()
    };
    if path.is_empty() {
        return Err(if server.use_sabre_squadron {
            "HD2DS Sabre Squadron path is not set for this server".to_string()
        } else {
            "HD2DS path is not set for this server".to_string()
        });
    }
    Ok(path)
}

fn sanitize_for_filename(s: &str) -> String {
    let s = s.trim();
    let out: String = s
        .chars()
        .map(|c| match c {
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '_',
            c if c.is_control() => '_',
            ' ' => '_',
            _ => c,
        })
        .collect();
    let out = out.trim_matches('_');
    if out.is_empty() {
        String::new()
    } else {
        out.to_string()
    }
}

fn get_current_config(server: &Server) -> Result<&ServerConfig, String> {
    server
        .configs
        .iter()
        .find(|c| c.name == server.current_config)
        .ok_or_else(|| {
            format!(
                "Config '{}' not found for server '{}'",
                server.current_config, server.name
            )
        })
}

/// Deploy config next to DS exe and start the DS process with -cmd -exec (working dir = exe dir).
/// Each server uses a separate commands file (by port) so multiple servers can run.
/// Returns the new process ID on success (process is detached).
pub fn start_ds(server: &Server) -> Result<u32, String> {
    let exe_path = get_ds_exe_path(server)?;
    let path = Path::new(exe_path);
    if !path.exists() {
        return Err(format!("DS exe not found: {}", exe_path));
    }

    let config = get_current_config(server)?;
    let script = build_ds_script(server, config);
    let name_part = sanitize_for_filename(&server.name);
    let commands_basename = if name_part.is_empty() {
        format!("spectre_ds_{}.txt", server.port)
    } else {
        format!("spectre_ds_{}.txt", name_part)
    };
    let written = write_script_to_ds_dir(&script, path, &commands_basename)?;
    println!(
        "[Server] Wrote {} ({} lines)",
        written.display(),
        script.len()
    );

    let parent = path
        .parent()
        .ok_or_else(|| "DS exe has no parent dir".to_string())?;
    let exe_os: std::ffi::OsString = path.as_os_str().to_owned();

    let child = Command::new(&exe_os)
        .current_dir(parent)
        .args(["-cmd", "-exec", &commands_basename])
        .spawn()
        .map_err(|e| format!("Failed to start DS process: {}", e))?;
    let pid = child.id();
    Ok(pid)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::server::ServerConfig;

    #[test]
    fn script_build_smoke() {
        let mut server = Server::default();
        server.port = 22000;
        let mut config = ServerConfig::default();
        config.session_name = "Test".to_string();
        config.style = "Occupation".to_string();
        config.maps = vec!["Burma1".to_string()];
        let script = build_ds_script(&server, &config);
        assert!(!script.is_empty());
        assert!(script.iter().any(|s| s.contains("sessionname")));
        // Default domain is "local" -> port line is omitted
        assert!(!script.iter().any(|s| s.starts_with("port ")));
        config.domain = "internet".to_string();
        let script_inet = build_ds_script(&server, &config);
        assert!(script_inet.iter().any(|s| s.contains("port 22000")));
    }
}
