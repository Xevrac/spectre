fn main() {
    increment_version();
    #[cfg(windows)]
    {
        embed_windows_icon();
    }
}

#[cfg(windows)]
fn embed_windows_icon() {
    use std::fs::File;
    use std::io::Write;
    use std::path::Path;

    let icon_path = "spectre_256.png";

    if Path::new(icon_path).exists() {
        if let Ok(icon_data) = std::fs::read(icon_path) {
            if let Ok(img) = image::load_from_memory(&icon_data) {
                let rgba = img.to_rgba8();
                let (width, height) = rgba.dimensions();
                let out_dir = std::env::var("OUT_DIR").unwrap();
                let ico_path = Path::new(&out_dir).join("icon.ico");
                if let Ok(mut file) = File::create(&ico_path) {
                    let mut ico_data = Vec::new();
                    ico_data.extend_from_slice(&[0u8, 0u8]); // Reserved
                    ico_data.extend_from_slice(&[1u8, 0u8]); // Type (1 = ICO)
                    ico_data.extend_from_slice(&[1u8, 0u8]);
                    let width_byte = if width >= 256 { 0 } else { width as u8 };
                    let height_byte = if height >= 256 { 0 } else { height as u8 };
                    ico_data.push(width_byte);
                    ico_data.push(height_byte);
                    ico_data.push(0);
                    ico_data.push(0);
                    ico_data.extend_from_slice(&1u16.to_le_bytes());
                    ico_data.extend_from_slice(&32u16.to_le_bytes());
                    let bmp_size = 14 + 40 + (width * height * 4) as u32;
                    ico_data.extend_from_slice(&bmp_size.to_le_bytes());
                    let offset = 6 + 16;
                    ico_data.extend_from_slice(&(offset as u32).to_le_bytes());
                    let mut bmp_data = Vec::new();
                    bmp_data.extend_from_slice(b"BM");
                    bmp_data.extend_from_slice(&bmp_size.to_le_bytes());
                    bmp_data.extend_from_slice(&[0u8; 4]);
                    bmp_data.extend_from_slice(&54u32.to_le_bytes());
                    bmp_data.extend_from_slice(&40u32.to_le_bytes());
                    bmp_data.extend_from_slice(&(width as i32).to_le_bytes());
                    bmp_data.extend_from_slice(&((height * 2) as i32).to_le_bytes());
                    bmp_data.extend_from_slice(&1u16.to_le_bytes());
                    bmp_data.extend_from_slice(&32u16.to_le_bytes());
                    bmp_data.extend_from_slice(&[0u8; 16]);
                    bmp_data.extend_from_slice(&[0u8; 8]);
                    let pixels = rgba.as_raw();
                    for y in (0..height).rev() {
                        let row_start = (y * width * 4) as usize;
                        let row_end = row_start + (width * 4) as usize;
                        bmp_data.extend_from_slice(&pixels[row_start..row_end]);
                    }

                    ico_data.extend_from_slice(&bmp_data);

                    if file.write_all(&ico_data).is_ok() {
                        let mut res = winres::WindowsResource::new();
                        res.set_icon(&ico_path.to_string_lossy());
                        let _ = res.compile();
                    }
                }
            }
        }
    }
}

fn increment_version() {
    use std::fs;
    use std::io::Write;
    use std::path::Path;

    let cargo_toml_path = Path::new("Cargo.toml");

    if let Ok(contents) = fs::read_to_string(cargo_toml_path) {
        if let Ok(mut doc) = toml::from_str::<toml::Value>(&contents) {
            if let Some(package) = doc.get_mut("package").and_then(|p| p.as_table_mut()) {
                if let Some(version) = package.get("version").and_then(|v| v.as_str()) {
                    let parts: Vec<&str> = version.split('.').collect();
                    if parts.len() == 3 {
                        if let (Ok(major), Ok(minor), Ok(patch)) = (
                            parts[0].parse::<u32>(),
                            parts[1].parse::<u32>(),
                            parts[2].parse::<u32>(),
                        ) {
                            let new_patch = patch + 1;
                            let new_version = format!("{}.{}.{}", major, minor, new_patch);
                            package.insert(
                                "version".to_string(),
                                toml::Value::String(new_version.clone()),
                            );
                            if let Ok(toml_string) = toml::to_string_pretty(&doc) {
                                if let Ok(mut file) = fs::File::create(cargo_toml_path) {
                                    if file.write_all(toml_string.as_bytes()).is_ok() {
                                        println!(
                                            "cargo:warning=Version incremented to {}",
                                            new_version
                                        );
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
