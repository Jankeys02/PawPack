use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use tauri::Manager;

// ── Types ─────────────────────────────────────────────────────────────────────

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct PackMeta {
    pub id: String,
    pub name: String,
    pub author: String,
    pub description: String,
    /// "windows" | "linux" | "unknown"
    pub platform: String,
    pub cursor_count: usize,
    /// Unix timestamp (seconds)
    pub imported_at: u64,
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn packs_dir(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|e| e.to_string())?
        .join("packs");
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(dir)
}

fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn slugify(s: &str) -> String {
    s.chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' || c == '_' {
                c.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect::<String>()
        .split('-')
        .filter(|p| !p.is_empty())
        .collect::<Vec<_>>()
        .join("-")
}

fn detect_pack(dir: &Path) -> (String, usize) {
    let win_count = fs::read_dir(dir)
        .map(|entries| {
            entries
                .flatten()
                .filter(|e| {
                    let p = e.path();
                    p.is_file()
                        && p.extension()
                            .and_then(|x| x.to_str())
                            .map(|x| matches!(x.to_ascii_lowercase().as_str(), "cur" | "ani"))
                            .unwrap_or(false)
                })
                .count()
        })
        .unwrap_or(0);

    if win_count > 0 {
        return ("windows".into(), win_count);
    }

    let cursors_dir = dir.join("cursors");
    if cursors_dir.is_dir() {
        let linux_count = fs::read_dir(&cursors_dir)
            .map(|entries| {
                entries
                    .flatten()
                    .filter(|e| e.path().is_file() && e.path().extension().is_none())
                    .count()
            })
            .unwrap_or(0);
        if linux_count > 0 {
            return ("linux".into(), linux_count);
        }
    }

    if dir.join("install.inf").exists() || dir.join("Install.inf").exists() {
        return ("windows".into(), 0);
    }

    ("unknown".into(), 0)
}

fn read_pack_info(dir: &Path) -> (String, String) {
    for inf_name in ["install.inf", "Install.inf"] {
        if let Ok(content) = fs::read_to_string(dir.join(inf_name)) {
            let mut author = String::new();
            let mut description = String::new();
            for line in content.lines() {
                let line = line.trim();
                if author.is_empty() {
                    if let Some(v) = line
                        .strip_prefix("Author=")
                        .or_else(|| line.strip_prefix("author="))
                    {
                        author = v.trim().to_string();
                    }
                }
                if description.is_empty() {
                    if let Some(v) = line
                        .strip_prefix("Description=")
                        .or_else(|| line.strip_prefix("description="))
                    {
                        description = v.trim().to_string();
                    }
                }
            }
            if !author.is_empty() || !description.is_empty() {
                return (author, description);
            }
        }
    }

    if let Ok(content) = fs::read_to_string(dir.join("index.theme")) {
        let mut description = String::new();
        for line in content.lines() {
            if let Some(v) = line.trim().strip_prefix("Comment=") {
                description = v.trim().to_string();
            }
        }
        return (String::new(), description);
    }

    (String::new(), String::new())
}

fn read_pack_name(dir: &Path, fallback: &str) -> String {
    if let Ok(content) = fs::read_to_string(dir.join("index.theme")) {
        for line in content.lines() {
            if let Some(v) = line.trim().strip_prefix("Name=") {
                let name = v.trim().to_string();
                if !name.is_empty() {
                    return name;
                }
            }
        }
    }
    fallback.to_string()
}

fn copy_dir(src: &Path, dst: &Path) -> io::Result<()> {
    fs::create_dir_all(dst)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());
        if src_path.is_dir() {
            copy_dir(&src_path, &dst_path)?;
        } else {
            fs::copy(&src_path, &dst_path)?;
        }
    }
    Ok(())
}

/// Extract a zip archive into `dest`, creating it if needed.
fn extract_zip(zip_path: &Path, dest: &Path) -> io::Result<()> {
    let file = fs::File::open(zip_path)?;
    let mut archive = zip::ZipArchive::new(file)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;

    for i in 0..archive.len() {
        let mut entry = archive
            .by_index(i)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;

        // enclosed_name() rejects path traversal attempts
        let out_path = match entry.enclosed_name() {
            Some(p) => dest.join(p.to_path_buf()),
            None => continue,
        };

        if entry.is_dir() {
            fs::create_dir_all(&out_path)?;
        } else {
            if let Some(parent) = out_path.parent() {
                fs::create_dir_all(parent)?;
            }
            let mut out_file = fs::File::create(&out_path)?;
            io::copy(&mut entry, &mut out_file)?;
        }
    }

    Ok(())
}

/// If the extracted directory contains exactly one subdirectory (and nothing
/// else), the zip was wrapped in a folder — return that inner directory as the
/// real pack root. Otherwise return the directory itself.
fn find_pack_root(extracted: &Path) -> PathBuf {
    if let Ok(mut entries) = fs::read_dir(extracted) {
        if let Some(first) = entries.next().and_then(|e| e.ok()) {
            if entries.next().is_none() && first.path().is_dir() {
                return first.path();
            }
        }
    }
    extracted.to_path_buf()
}

/// Core import logic shared by both zip and folder imports.
fn do_import(app: &tauri::AppHandle, source: &Path, raw_name: &str) -> Result<PackMeta, String> {
    let id = slugify(raw_name);
    if id.is_empty() {
        return Err("Could not generate a valid ID from the pack name".into());
    }

    let dest = packs_dir(app)?.join(&id);
    if dest.exists() {
        return Err(format!("A pack named '{}' is already imported", id));
    }

    let name = read_pack_name(source, raw_name);
    let (platform, cursor_count) = detect_pack(source);
    let (author, description) = read_pack_info(source);

    copy_dir(source, &dest).map_err(|e| format!("Failed to copy files: {}", e))?;

    let meta = PackMeta {
        id,
        name,
        author,
        description,
        platform,
        cursor_count,
        imported_at: now_unix(),
    };

    let json = serde_json::to_string_pretty(&meta).map_err(|e| e.to_string())?;
    fs::write(dest.join("pack.json"), json).map_err(|e| e.to_string())?;

    Ok(meta)
}

// ── Commands ──────────────────────────────────────────────────────────────────

#[tauri::command]
fn list_packs(app: tauri::AppHandle) -> Result<Vec<PackMeta>, String> {
    let dir = packs_dir(&app)?;
    let mut packs: Vec<PackMeta> = fs::read_dir(&dir)
        .map_err(|e| e.to_string())?
        .flatten()
        .filter(|e| e.path().is_dir())
        .filter_map(|e| {
            let json = fs::read_to_string(e.path().join("pack.json")).ok()?;
            serde_json::from_str::<PackMeta>(&json).ok()
        })
        .collect();

    packs.sort_by(|a, b| b.imported_at.cmp(&a.imported_at));
    Ok(packs)
}

#[tauri::command]
fn import_pack(app: tauri::AppHandle, source_path: String) -> Result<PackMeta, String> {
    let source = PathBuf::from(&source_path);

    let is_zip = source
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.eq_ignore_ascii_case("zip"))
        .unwrap_or(false);

    if is_zip {
        if !source.is_file() {
            return Err("Zip path does not point to a file".into());
        }

        let raw_name = source
            .file_stem()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown-pack");

        // Extract into a unique temp directory
        let temp_dir = std::env::temp_dir().join(format!("pawpack-{}", now_unix()));
        extract_zip(&source, &temp_dir)
            .map_err(|e| format!("Failed to extract zip: {}", e))?;

        let pack_root = find_pack_root(&temp_dir);
        let result = do_import(&app, &pack_root, raw_name);

        // Always clean up temp, even on error
        let _ = fs::remove_dir_all(&temp_dir);

        result
    } else if source.is_dir() {
        let raw_name = source
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown-pack");

        do_import(&app, &source, raw_name)
    } else {
        Err("Source must be a .zip file or a folder".into())
    }
}

#[tauri::command]
fn delete_pack(app: tauri::AppHandle, pack_id: String) -> Result<(), String> {
    let base = packs_dir(&app)?;
    let pack_dir = base.join(&pack_id);

    if !pack_dir.starts_with(&base) {
        return Err("Invalid pack id".into());
    }
    if !pack_dir.exists() {
        return Err("Pack not found".into());
    }

    fs::remove_dir_all(&pack_dir).map_err(|e| e.to_string())
}

// ── Entry point ───────────────────────────────────────────────────────────────

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![list_packs, import_pack, delete_pack])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
