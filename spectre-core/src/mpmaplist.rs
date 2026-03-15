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

fn extract_attr(line: &str, attr: &str) -> Option<String> {
    let mut out = extract_all_attr(line, attr);
    if out.len() == 1 {
        Some(out.pop().unwrap())
    } else {
        out.into_iter().next()
    }
}

fn extract_all_attr(line: &str, attr: &str) -> Vec<String> {
    let mut result = Vec::new();
    let search_dq = format!("{}=\"", attr);
    let search_sq = format!("{}='", attr);
    let lower = line.to_lowercase();
    let mut pos = 0;
    while pos < lower.len() {
        let rest_lower = &lower[pos..];
        let (offset, quote_len, end_char) = {
            let next_dq = rest_lower.find(&search_dq);
            let next_sq = rest_lower.find(&search_sq);
            match (next_dq, next_sq) {
                (Some(a), None) => (a, search_dq.len(), '"'),
                (None, Some(b)) => (b, search_sq.len(), '\''),
                (Some(a), Some(b)) if a <= b => (a, search_dq.len(), '"'),
                (Some(_), Some(b)) => (b, search_sq.len(), '\''),
                (None, None) => break,
            }
        };
        let start_in_line = pos + offset + quote_len;
        let after = line.get(start_in_line..).unwrap_or("");
        if let Some(end) = after.find(end_char) {
            let val = after[..end].trim().to_string();
            if !val.is_empty() {
                result.push(val);
            }
            pos = start_in_line + end + 1;
        } else {
            pos = pos + offset + 1;
        }
    }
    result
}

const MAX_MAP_LINES: usize = 5;

pub fn parse_mpmaplist(content: &str) -> HashMap<String, Vec<String>> {
    let mut by_tag: HashMap<String, Vec<String>> = HashMap::new();
    let tag_names: HashMap<String, String> = STYLE_TAG_TO_NAME
        .iter()
        .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
        .collect();

    let mut current_tag: Option<String> = None;
    let lines: Vec<&str> = content.lines().map(str::trim).filter(|s| !s.is_empty()).collect();
    let mut i = 0;

    while i < lines.len() {
        let line = lines[i];

        if line.to_lowercase().contains("<gamestyle") {
            if let Some(tag) = extract_attr(line, "type") {
                if !tag.is_empty() {
                    current_tag = Some(tag.to_lowercase());
                }
            }
            i += 1;
            continue;
        }

        if line.to_lowercase().contains("<map") {
            let mut combined = line.to_string();
            let mut names = extract_all_attr(&combined, "name");
            let mut j = i + 1;
            while names.is_empty() && j < lines.len() && j < i + MAX_MAP_LINES {
                combined.push(' ');
                combined.push_str(lines[j]);
                names = extract_all_attr(&combined, "name");
                j += 1;
            }
            if let Some(ref tag) = current_tag {
                for n in names.into_iter().filter(|s| !s.is_empty()) {
                    by_tag.entry(tag.clone()).or_default().push(n);
                }
            }
            i += 1;
            continue;
        }

        i += 1;
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

    #[test]
    fn parse_map_list_format() {
        let s = r#"
<MAP_LIST>
  <GAMESTYLE type="cooperative">
    <MAP name="Brest" dir="Brest_mp">
      <ALLOWEDITEMS version="0.1">
        <ITEM item_id="0" item_num="1" ammo_num="0" />
      </ALLOWEDITEMS>
    <MAP name="Libya1" dir="Libya1_mp">
    <MAP name="Sicily1" dir="Sicily1_mp">
  <GAMESTYLE type="deathmatch">
    <MAP name="Africa1NS" dir="Africa1_mp">
"#;
        let m = parse_mpmaplist(s);
        let coop = m.get("Cooperative").expect("Cooperative style");
        assert!(coop.contains(&"Brest".to_string()));
        assert!(coop.contains(&"Libya1".to_string()));
        assert!(coop.contains(&"Sicily1".to_string()));
        assert_eq!(coop.len(), 3);
        let dm = m.get("Deathmatch").expect("Deathmatch style");
        assert_eq!(dm, &vec!["Africa1NS".to_string()]);
    }

    #[test]
    fn parse_multiple_maps_per_line() {
        let s = r#"
<GAMESTYLE type="cooperative">
</MAP><MAP name="Br(Silent-Op)" dir="Co_Brest_siop">
<MAP name="Libya1" dir="Co_Libye1">
<MAP name="Brest" dir="Co_Brest"><ALLOWEDITEMS></ALLOWEDITEMS>
"#;
        let m = parse_mpmaplist(s);
        let coop = m.get("Cooperative").expect("Cooperative style");
        assert!(coop.contains(&"Br(Silent-Op)".to_string()));
        assert!(coop.contains(&"Libya1".to_string()));
        assert!(coop.contains(&"Brest".to_string()));
        assert_eq!(coop.len(), 3);
    }
}
