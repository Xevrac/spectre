#[cfg(windows)]
mod windows {
    use std::fs;
    use std::path::PathBuf;
    use std::process::Command;

    const DIRECTPLAY_FEATURE_NAME: &str = "DirectPlay";
    const REG_PATH: &str = r"SOFTWARE\WOW6432Node\Microsoft\DirectPlay8\IPAddressFamilySettings";
    const REG_VALUE_HD2DS: &str = "HD2DS";
    const REG_VALUE_HD2DS_SS: &str = "HD2DS_SabreSquadron";
    const REG_VALUE_HD2_SS: &str = "HD2_SabreSquadron";
    const REG_VALUE_HD2: &str = "hd2";
    const REG_REQUIRED: u32 = 2;

    pub fn directplay_enabled() -> bool {
        if directplay_via_dism_featureinfo() {
            return true;
        }
        if directplay_via_dism_get_features() {
            return true;
        }
        directplay_via_powershell()
    }

    fn directplay_via_dism_featureinfo() -> bool {
        let out = match Command::new("dism")
            .args([
                "/online",
                "/get-featureinfo",
                &format!("/featurename:{}", DIRECTPLAY_FEATURE_NAME),
            ])
            .output()
        {
            Ok(o) => o,
            Err(_) => return false,
        };
        let text = String::from_utf8_lossy(&out.stdout);
        let text_lower = text.to_lowercase();
        if !text_lower.contains("directplay") {
            return false;
        }
        for line in text.lines() {
            let line_lower = line.trim().to_lowercase();
            if line_lower.contains("state") && line_lower.contains("enabled") {
                if line_lower.contains("disabled") {
                    return false;
                }
                return true;
            }
        }
        false
    }

    /// Fallback: list all features and look for any feature containing "DirectPlay" with state Enabled.
    /// Handles different DISM locales or feature display names.
    fn directplay_via_dism_get_features() -> bool {
        let out = match Command::new("dism")
            .args(["/online", "/get-features"])
            .output()
        {
            Ok(o) => o,
            Err(_) => return false,
        };
        let text = String::from_utf8_lossy(&out.stdout);
        let mut in_directplay = false;
        for line in text.lines() {
            let line_lower = line.trim().to_lowercase();
            // Feature name lines often look like "Feature Name : DirectPlay" or "FeatureName : DirectPlay"
            if line_lower.contains("directplay") {
                in_directplay = true;
            }
            if in_directplay {
                if line_lower.contains("state") && line_lower.contains("enabled") && !line_lower.contains("disabled") {
                    return true;
                }
                // If we see a new "Feature Name" that doesn't contain DirectPlay, we've left the DirectPlay block
                if (line_lower.contains("feature name") || line_lower.contains("featurename")) && !line_lower.contains("directplay") {
                    in_directplay = false;
                }
            }
        }
        false
    }

    fn directplay_via_powershell() -> bool {
        // Fallback: PowerShell Get-WindowsOptionalFeature (more reliable on some systems)
        let script = "Get-WindowsOptionalFeature -FeatureName DirectPlay -Online | Select-Object -ExpandProperty State";
        let out = match Command::new("powershell")
            .args(["-NoProfile", "-NonInteractive", "-Command", script])
            .output()
        {
            Ok(o) => o,
            Err(_) => return false,
        };
        let text = String::from_utf8_lossy(&out.stdout);
        let state = text.trim().to_lowercase();
        state == "enabled"
    }

    /// Run DirectPlay check and write result to a file (used by elevated process).
    /// Writes "enabled" or "disabled" to the given path.
    pub fn run_check_directplay_and_write_result(path: &std::path::Path) -> Result<(), String> {
        println!("[Spectre.dbg] DirectPlay: running detection (DISM/PowerShell)");
        let enabled = directplay_enabled();
        let s = if enabled { "enabled" } else { "disabled" };
        println!("[Spectre.dbg] DirectPlay: detection result={}, writing to {}", s, path.display());
        fs::write(path, s).map_err(|e| e.to_string())
    }

    /// Enable DirectPlay Windows Optional Feature. Requires administrator rights.
    pub fn enable_directplay() -> Result<(), String> {
        println!("[Spectre.dbg] DirectPlay: enabling via DISM");
        let out = Command::new("dism")
            .args([
                "/online",
                "/enable-feature",
                &format!("/featurename:{}", DIRECTPLAY_FEATURE_NAME),
            ])
            .output()
            .map_err(|e| e.to_string())?;
        let text = String::from_utf8_lossy(&out.stderr);
        if out.status.success() {
            println!("[Spectre.dbg] DirectPlay: DISM enable succeeded");
            return Ok(());
        }
        println!("[Spectre.dbg] DirectPlay: DISM enable failed: {}", text.trim());
        Err(format!("DISM failed: {}", text.trim()))
    }

    /// Spawn a thread that requests UAC and runs DirectPlay detection in an elevated process,
    /// writing the result to a temp file. Sends Ok(true)/Ok(false) or Err on the channel.
    /// In debug builds, set env SPECTRE_EMULATE_NO_DIRECTPLAY=1 to have the elevated process
    /// report DirectPlay as not installed (for testing the wizard flow).
    pub fn spawn_elevated_check_directplay(
        sender: std::sync::mpsc::Sender<Result<bool, String>>,
        result_path: PathBuf,
    ) {
        let exe = std::env::current_exe()
            .unwrap_or_else(|_| std::path::PathBuf::from("spectre.exe"));
        let emulate = cfg!(debug_assertions)
            && std::env::var("SPECTRE_EMULATE_NO_DIRECTPLAY").is_ok();
        if emulate {
            println!("[Spectre.dbg] DirectPlay: SPECTRE_EMULATE_NO_DIRECTPLAY set, elevated check will report NOT installed");
        }
        println!("[Spectre.dbg] DirectPlay: spawning elevated check, result_path={}", result_path.display());
        std::thread::spawn(move || {
            let status = if emulate {
                runas::Command::new(&exe)
                    .arg("--elevated-check-directplay")
                    .arg(&result_path)
                    .arg("--emulate-no-directplay")
                    .show(false)
                    .status()
            } else {
                runas::Command::new(&exe)
                    .arg("--elevated-check-directplay")
                    .arg(&result_path)
                    .show(false)
                    .status()
            };
            let result = match status {
                Ok(s) if s.success() => {
                    let content = fs::read_to_string(&result_path).unwrap_or_default();
                    let enabled = content.trim().to_lowercase() == "enabled";
                    let _ = fs::remove_file(&result_path);
                    println!("[Spectre.dbg] DirectPlay: elevated check finished, result={}", if enabled { "enabled" } else { "disabled" });
                    Ok(enabled)
                }
                Ok(_) => {
                    println!("[Spectre.dbg] DirectPlay: elevated check process exited with error");
                    Err("Elevated check process exited with an error.".to_string())
                }
                Err(e) => {
                    println!("[Spectre.dbg] DirectPlay: elevated check failed to run: {}", e);
                    Err(e.to_string())
                }
            };
            let _ = sender.send(result);
        });
    }

    /// Spawn a thread that requests UAC and enables DirectPlay in an elevated process.
    pub fn spawn_elevated_install_directplay(sender: std::sync::mpsc::Sender<Result<(), String>>) {
        let exe = std::env::current_exe()
            .unwrap_or_else(|_| std::path::PathBuf::from("spectre.exe"));
        std::thread::spawn(move || {
            let status = runas::Command::new(&exe)
                .arg("--elevated-install-directplay")
                .show(false)
                .status();
            let result = match status {
                Ok(s) if s.success() => Ok(()),
                Ok(_) => Err("Elevated process exited with an error.".to_string()),
                Err(e) => Err(e.to_string()),
            };
            let _ = sender.send(result);
        });
    }

    /// Check if the HD2 DirectPlay IP family registry fix is applied (all four values = 2).
    pub fn registry_fix_applied() -> bool {
        use winreg::enums::HKEY_LOCAL_MACHINE;
        use winreg::RegKey;

        let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
        let key = match hklm.open_subkey(REG_PATH) {
            Ok(k) => k,
            Err(_) => return false,
        };
        let read_u32 = |name: &str| -> Option<u32> { key.get_value(name).ok() };
        read_u32(REG_VALUE_HD2DS) == Some(REG_REQUIRED)
            && read_u32(REG_VALUE_HD2DS_SS) == Some(REG_REQUIRED)
            && read_u32(REG_VALUE_HD2_SS) == Some(REG_REQUIRED)
            && read_u32(REG_VALUE_HD2) == Some(REG_REQUIRED)
    }

    /// Apply the registry fix. Requires administrator rights.
    pub fn apply_registry_fix() -> Result<(), String> {
        println!("[Spectre.dbg] Registry fix: applying IPAddressFamilySettings to {}", REG_PATH);
        use winreg::enums::{HKEY_LOCAL_MACHINE, KEY_WRITE};
        use winreg::RegKey;

        let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
        let (key, _) = hklm
            .create_subkey_with_flags(REG_PATH, KEY_WRITE)
            .map_err(|e| format!("Registry create/open failed: {} (try running as Administrator)", e))?;
        key.set_value(REG_VALUE_HD2DS, &REG_REQUIRED)
            .map_err(|e| format!("Set HD2DS: {}", e))?;
        key.set_value(REG_VALUE_HD2DS_SS, &REG_REQUIRED)
            .map_err(|e| format!("Set HD2DS_SabreSquadron: {}", e))?;
        key.set_value(REG_VALUE_HD2_SS, &REG_REQUIRED)
            .map_err(|e| format!("Set HD2_SabreSquadron: {}", e))?;
        key.set_value(REG_VALUE_HD2, &REG_REQUIRED)
            .map_err(|e| format!("Set hd2: {}", e))?;
        println!("[Spectre.dbg] Registry fix: applied successfully");
        Ok(())
    }

    // --- GameSpy hosts file (for HD2 multiplayer / server list) ---
    const GAMESPY_IP: &str = "78.47.255.224";
    const GAMESPY_HOSTS: &[&str] = &[
        "key.gamespy.com",
        "master.gamespy.com",
        "master0.gamespy.com",
        "hd2.available.gamespy.com",
        "hd2.master.gamespy.com",
        "hd2.ms14.gamespy.com",
        "natneg1.gamespy.com",
        "natneg2.gamespy.com",
        "natneg3.gamespy.com",
    ];

    fn hosts_file_path() -> PathBuf {
        let root = std::env::var("SystemRoot").unwrap_or_else(|_| "C:\\Windows".to_string());
        PathBuf::from(root).join("System32").join("drivers").join("etc").join("hosts")
    }

    /// Returns true if the line contains any of our GameSpy hostnames as a word (any IP).
    fn line_has_gamepy_host(line: &str) -> bool {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            return false;
        }
        line.split_ascii_whitespace()
            .any(|word| GAMESPY_HOSTS.iter().any(|h| word.eq_ignore_ascii_case(h)))
    }

    /// Check if all required GameSpy host entries exist with the *current* IP.
    /// If the program's IP is updated later, old entries (different IP) do not count,
    /// so step 1 will be required again and apply will replace them.
    pub fn gamepy_hosts_applied() -> bool {
        let path = hosts_file_path();
        let content = match fs::read_to_string(&path) {
            Ok(c) => c,
            Err(_) => return false,
        };
        for host in GAMESPY_HOSTS {
            let mut found_with_current_ip = false;
            for line in content.lines() {
                let line = line.trim();
                if line.is_empty() || line.starts_with('#') {
                    continue;
                }
                let parts: Vec<&str> = line.split_ascii_whitespace().collect();
                // Must be current IP and this hostname
                if parts.len() >= 2
                    && parts[0] == GAMESPY_IP
                    && parts[1..].iter().any(|p| p.eq_ignore_ascii_case(host))
                {
                    found_with_current_ip = true;
                    break;
                }
            }
            if !found_with_current_ip {
                return false;
            }
        }
        true
    }

    /// Sync GameSpy entries in the hosts file: remove any line containing our hostnames (any IP),
    /// then append all current entries with the current IP. Handles partial presence and IP updates.
    /// Requires administrator rights.
    pub fn apply_gamepy_hosts() -> Result<(), String> {
        let path = hosts_file_path();
        println!("[Spectre.dbg] GameSpy hosts: applying to {}", path.display());
        let content = fs::read_to_string(&path)
            .map_err(|e| format!("Cannot read hosts file: {} (try running as Administrator)", e))?;

        // Keep only lines that do not contain any of our GameSpy hostnames (so we remove old IP or partial entries)
        let kept: Vec<&str> = content
            .lines()
            .filter(|line| !line_has_gamepy_host(line))
            .collect();

        // Build new file: kept lines (preserve trailing newline behavior), then our block
        let mut new_content = kept.join("\n");
        if !new_content.is_empty() && !new_content.ends_with('\n') {
            new_content.push('\n');
        }
        new_content.push('\n');
        new_content.push_str("# HD2 GameSpy (added by Spectre)\n");
        for host in GAMESPY_HOSTS {
            new_content.push_str(&format!("{}  {}\n", GAMESPY_IP, host));
        }

        fs::write(&path, new_content)
            .map_err(|e| format!("Cannot write hosts file: {} (try running as Administrator)", e))?;
        println!("[Spectre.dbg] GameSpy hosts: applied successfully");
        Ok(())
    }

    /// Spawn a thread that requests UAC and runs the registry fix in an elevated process.
    /// Sends the result on `sender` when done (success or error, or if user cancels UAC).
    pub fn spawn_elevated_apply_registry(sender: std::sync::mpsc::Sender<Result<(), String>>) {
        let exe = std::env::current_exe()
            .unwrap_or_else(|_| std::path::PathBuf::from("spectre.exe"));
        std::thread::spawn(move || {
            let status = runas::Command::new(&exe)
                .arg("--elevated-apply-registry")
                .show(false)
                .status();
            let result = match status {
                Ok(s) if s.success() => Ok(()),
                Ok(_) => Err("Elevated process exited with an error.".to_string()),
                Err(e) => Err(e.to_string()),
            };
            let _ = sender.send(result);
        });
    }

    /// Spawn a thread that requests UAC and runs the hosts file fix in an elevated process.
    pub fn spawn_elevated_apply_hosts(sender: std::sync::mpsc::Sender<Result<(), String>>) {
        let exe = std::env::current_exe()
            .unwrap_or_else(|_| std::path::PathBuf::from("spectre.exe"));
        std::thread::spawn(move || {
            let status = runas::Command::new(&exe)
                .arg("--elevated-apply-hosts")
                .show(false)
                .status();
            let result = match status {
                Ok(s) if s.success() => Ok(()),
                Ok(_) => Err("Elevated process exited with an error.".to_string()),
                Err(e) => Err(e.to_string()),
            };
            let _ = sender.send(result);
        });
    }
}

#[cfg(not(windows))]
mod windows {
    use std::path::PathBuf;
    pub fn directplay_enabled() -> bool {
        true
    }
    pub fn run_check_directplay_and_write_result(_path: &std::path::Path) -> Result<(), String> {
        Ok(())
    }
    pub fn enable_directplay() -> Result<(), String> {
        Err("DirectPlay install is only supported on Windows.".to_string())
    }
    pub fn spawn_elevated_check_directplay(
        sender: std::sync::mpsc::Sender<Result<bool, String>>,
        _result_path: PathBuf,
    ) {
        let _ = sender.send(Ok(true));
    }
    pub fn spawn_elevated_install_directplay(sender: std::sync::mpsc::Sender<Result<(), String>>) {
        let _ = sender.send(Err("UAC elevation is only supported on Windows.".to_string()));
    }
    pub fn registry_fix_applied() -> bool {
        true
    }
    pub fn apply_registry_fix() -> Result<(), String> {
        Err("Registry fix is only supported on Windows.".to_string())
    }
    pub fn gamepy_hosts_applied() -> bool {
        true
    }
    pub fn apply_gamepy_hosts() -> Result<(), String> {
        Err("Hosts file fix is only supported on Windows.".to_string())
    }
    pub fn spawn_elevated_apply_registry(sender: std::sync::mpsc::Sender<Result<(), String>>) {
        let _ = sender.send(Err("UAC elevation is only supported on Windows.".to_string()));
    }
    pub fn spawn_elevated_apply_hosts(sender: std::sync::mpsc::Sender<Result<(), String>>) {
        let _ = sender.send(Err("UAC elevation is only supported on Windows.".to_string()));
    }
}

pub use windows::{
    apply_gamepy_hosts, apply_registry_fix, enable_directplay, gamepy_hosts_applied,
    registry_fix_applied, run_check_directplay_and_write_result, spawn_elevated_apply_hosts,
    spawn_elevated_apply_registry, spawn_elevated_check_directplay, spawn_elevated_install_directplay,
};
