// Core logic for parsing mpmaplist.txt (HD2 server manager format).
// File contains <gamestyle type="..."> sections and <map name="..."> entries.
// Maps are grouped by game style for the UI: only maps from the pool can be added to the rotation.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

/// Style tag in file (lowercase) -> display name used in config/UI.
const STYLE_TAG_TO_NAME: &[(&str, &str)] = &[
    ("hd2multiplayer", "Objectives"),
    ("teamplay", "Occupation"),
    ("deathmatch", "Deathmatch"),
    ("cooperative", "Cooperative"),
    ("invasion", "Invasion"),
];

/// Resolve path: if it's a directory or doesn't end with "mpmaplist.txt", append "mpmaplist.txt".
pub fn resolve_mpmaplist_path(path: &Path) -> PathBuf {
    let s = path.to_string_lossy();
    let ends_with_file = s.ends_with("mpmaplist.txt") || s.ends_with("mpmaplist.TXT")
        || s.ends_with("mpmaplist.Txt");
    if ends_with_file && path.exists() && !path.is_dir() {
        path.to_path_buf()
    } else {
        path.join("mpmaplist.txt")
    }
}

/// Parse mpmaplist.txt and return maps grouped by game style.
/// Keys: "Objectives", "Occupation", "Deathmatch", "Cooperative", "Invasion".
/// If path is a directory or doesn't end with mpmaplist.txt, joins with "mpmaplist.txt".
/// Returns empty map if file is missing or unreadable.
pub fn load_from_path(path: &Path) -> HashMap<String, Vec<String>> {
    let resolved = resolve_mpmaplist_path(path);
    let content = match fs::read_to_string(&resolved) {
        Ok(c) => c,
        Err(_) => return HashMap::new(),
    };
    parse_mpmaplist(&content)
}

/// Extract attribute value: name="val" or name='val' (case-insensitive for attr name).
fn extract_attr(line_lower: &str, line_orig: &str, attr: &str) -> Option<String> {
    let search_dq = format!("{}=\"", attr);
    let search_sq = format!("{}='", attr);
    if let Some(start) = line_lower.find(&search_dq) {
        let after = &line_orig[start + search_dq.len()..];
        if let Some(end) = after.find('"') {
            return Some(after[..end].trim().to_string());
        }
    }
    if let Some(start) = line_lower.find(&search_sq) {
        let after = &line_orig[start + search_sq.len()..];
        if let Some(end) = after.find('\'') {
            return Some(after[..end].trim().to_string());
        }
    }
    None
}

/// Parse mpmaplist content (same format as mpmaplist.txt).
pub fn parse_mpmaplist(content: &str) -> HashMap<String, Vec<String>> {
    let mut by_tag: HashMap<String, Vec<String>> = HashMap::new();
    let tag_names: HashMap<String, String> = STYLE_TAG_TO_NAME
        .iter()
        .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
        .collect();

    let mut current_tag: Option<String> = None;

    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let lower = trimmed.to_lowercase();

        // <gamestyle type="teamplay"> or type='teamplay' -> current section
        if lower.contains("<gamestyle") {
            if let Some(tag) = extract_attr(&lower, trimmed, "type") {
                if !tag.is_empty() {
                    current_tag = Some(tag.to_lowercase());
                }
            }
            continue;
        }

        // <map name="Map Name"> or <map name='...' /> -> add map to current section
        if lower.contains("<map") {
            if let Some(name) = extract_attr(&lower, trimmed, "name") {
                if !name.is_empty() {
                    if let Some(ref tag) = current_tag {
                        by_tag
                            .entry(tag.clone())
                            .or_default()
                            .push(name);
                    }
                }
            }
        }
    }

    // Convert tag keys to display names
    let mut result = HashMap::new();
    for (tag, maps) in by_tag {
        if let Some(name) = tag_names.get(&tag) {
            result.insert(name.clone(), maps);
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_simple() {
        let s = r#"
<gamestyle type="teamplay">
<map name="map_01">
<map name="map_02">
<gamestyle type="deathmatch">
<map name="dm_01">
"#;
        let m = parse_mpmaplist(s);
        assert_eq!(m.get("Occupation"), Some(&vec!["map_01".to_string(), "map_02".to_string()]));
        assert_eq!(m.get("Deathmatch"), Some(&vec!["dm_01".to_string()]));
    }
}