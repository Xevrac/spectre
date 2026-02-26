use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

const STYLE_TAG_TO_NAME: &[(&str, &str)] = &[
    ("hd2multiplayer", "Objectives"),
    ("teamplay", "Occupation"),
    ("deathmatch", "Deathmatch"),
    ("cooperative", "Cooperative"),
];

pub fn resolve_mpmaplist_path(path: &Path) -> PathBuf {
    let s = path.to_string_lossy();
    let ends_with_file = s.ends_with("mpmaplist.txt")
        || s.ends_with("mpmaplist.TXT")
        || s.ends_with("mpmaplist.Txt");
    if ends_with_file && path.exists() && !path.is_dir() {
        path.to_path_buf()
    } else {
        path.join("mpmaplist.txt")
    }
}

/// Parse mpmaplist.txt; returns maps by style. Empty if missing/unreadable.
pub fn load_from_path(path: &Path) -> HashMap<String, Vec<String>> {
    let resolved = resolve_mpmaplist_path(path);
    let content = match fs::read_to_string(&resolved) {
        Ok(c) => c,
        Err(_) => return HashMap::new(),
    };
    parse_mpmaplist(&content)
}

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

        if lower.contains("<gamestyle") {
            if let Some(tag) = extract_attr(&lower, trimmed, "type") {
                if !tag.is_empty() {
                    current_tag = Some(tag.to_lowercase());
                }
            }
            continue;
        }

        if lower.contains("<map") {
            if let Some(name) = extract_attr(&lower, trimmed, "name") {
                if !name.is_empty() {
                    if let Some(ref tag) = current_tag {
                        by_tag.entry(tag.clone()).or_default().push(name);
                    }
                }
            }
        }
    }

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
        assert_eq!(
            m.get("Occupation"),
            Some(&vec!["map_01".to_string(), "map_02".to_string()])
        );
        assert_eq!(m.get("Deathmatch"), Some(&vec!["dm_01".to_string()]));
    }
}
