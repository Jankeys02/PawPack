use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use tauri::Manager;

// ── Types ─────────────────────────────────────────────────────────────────────

/// One size-variant image decoded from a `.cur` (or embedded ICO frame).
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct CurFrame {
    pub width: u32,
    pub height: u32,
    pub hotspot_x: u16,
    pub hotspot_y: u16,
    /// RGBA pixels, row-major, `width * height * 4` bytes.
    pub rgba: Vec<u8>,
}

/// All size variants inside a single `.cur` file.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct CurInfo {
    pub frames: Vec<CurFrame>,
}

/// Parsed contents of a `.ani` (RIFF ACON) animated cursor.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct AniInfo {
    /// Number of distinct cursor images in the file.
    pub frame_count: u32,
    /// Default display rate in jiffies (1/60 s) from the `anih` chunk.
    pub display_rate: u32,
    /// Per-step delays from the optional `rate` chunk; empty when absent.
    pub per_frame_rates: Vec<u32>,
    /// Decoded frames in `LIST fram` order.
    pub frames: Vec<CurInfo>,
}

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

fn has_cursor_files(dir: &Path) -> bool {
    fs::read_dir(dir)
        .map(|entries| {
            entries.flatten().any(|e| {
                let p = e.path();
                p.is_file()
                    && p.extension()
                        .and_then(|x| x.to_str())
                        .map(|x| matches!(x.to_ascii_lowercase().as_str(), "cur" | "ani"))
                        .unwrap_or(false)
            })
        })
        .unwrap_or(false)
}

/// Find the actual pack root inside `extracted`.
///
/// Handles zips that wrap everything in a subfolder alongside other files
/// (READMEs, __MACOSX/, install.inf, etc.) by finding the unique child
/// directory that actually contains cursor files rather than requiring it to
/// be the only entry.
fn find_pack_root(extracted: &Path) -> PathBuf {
    if has_cursor_files(extracted) {
        return extracted.to_path_buf();
    }
    if let Ok(entries) = fs::read_dir(extracted) {
        let cursor_subdirs: Vec<PathBuf> = entries
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.is_dir() && has_cursor_files(p))
            .collect();
        if let [single] = cursor_subdirs.as_slice() {
            return single.clone();
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

// ── Cursor parsers ────────────────────────────────────────────────────────────

/// Decode a `.cur` (or plain `.ico`) byte blob into per-size-variant frames.
///
/// Hotspots are read directly from the raw `ICONDIRENTRY` bytes (fields
/// `xHotspot`/`yHotspot` at offsets +4 / +6 within each 16-byte entry),
/// which are reused as hotspot coords in CUR files (type == 2).
fn parse_cur_bytes(data: &[u8]) -> Result<CurInfo, String> {
    if data.len() < 6 {
        return Err("CUR/ICO file too short".into());
    }

    let image_count = u16::from_le_bytes([data[4], data[5]]) as usize;

    // Collect hotspots from raw directory entries (6-byte header + 16 bytes each).
    let hotspots: Vec<(u16, u16)> = (0..image_count)
        .map(|i| {
            let base = 6 + i * 16;
            if base + 8 > data.len() {
                return (0, 0);
            }
            let hx = u16::from_le_bytes([data[base + 4], data[base + 5]]);
            let hy = u16::from_le_bytes([data[base + 6], data[base + 7]]);
            (hx, hy)
        })
        .collect();

    let reader = io::Cursor::new(data);
    let icon_dir =
        ico::IconDir::read(reader).map_err(|e| format!("ICO/CUR read error: {e}"))?;

    let frames = icon_dir
        .entries()
        .iter()
        .enumerate()
        .map(|(i, entry)| {
            let (hotspot_x, hotspot_y) = hotspots.get(i).copied().unwrap_or((0, 0));
            let image = entry.decode().map_err(|e| format!("ICO decode error: {e}"))?;
            Ok(CurFrame {
                width: image.width(),
                height: image.height(),
                hotspot_x,
                hotspot_y,
                rgba: image.rgba_data().to_vec(),
            })
        })
        .collect::<Result<Vec<_>, String>>()?;

    Ok(CurInfo { frames })
}

/// Walk a `.ani` (RIFF ACON) byte blob and decode every embedded cursor frame.
///
/// Layout: `RIFF ACON [ anih … ] [ rate … ] [ LIST fram [ icon … ]+ ]`
/// All multi-byte values are little-endian.
fn parse_ani_bytes(data: &[u8]) -> Result<AniInfo, String> {
    if data.len() < 12 {
        return Err("ANI file too short".into());
    }
    if &data[0..4] != b"RIFF" {
        return Err("Not a RIFF file".into());
    }
    if &data[8..12] != b"ACON" {
        return Err("Not an ACON (ANI) file".into());
    }

    let riff_size = u32::from_le_bytes(data[4..8].try_into().unwrap()) as usize;
    // The RIFF size covers everything after the 8-byte RIFF header.
    let end = 8usize.saturating_add(riff_size).min(data.len());

    let mut pos = 12usize; // skip "RIFF" + size + "ACON"
    let mut frame_count = 0u32;
    let mut display_rate = 0u32;
    let mut per_frame_rates: Vec<u32> = Vec::new();
    let mut frames: Vec<CurInfo> = Vec::new();

    while pos + 8 <= end {
        let chunk_id = &data[pos..pos + 4];
        let chunk_size =
            u32::from_le_bytes(data[pos + 4..pos + 8].try_into().unwrap()) as usize;
        let data_start = pos + 8;
        let data_end = data_start.saturating_add(chunk_size);

        if data_end > data.len() {
            break;
        }

        match chunk_id {
            b"anih" if chunk_size >= 36 => {
                let b = &data[data_start..data_end];
                // ANIHEADER fields (each u32 LE):
                //   [0] cbSizeof  [1] nFrames  [2] nSteps  [3] iWidth  [4] iHeight
                //   [5] iBitCount [6] nPlanes  [7] iDispRate [8] bfAttributes
                frame_count = u32::from_le_bytes(b[4..8].try_into().unwrap());
                display_rate = u32::from_le_bytes(b[28..32].try_into().unwrap());
            }
            b"rate" => {
                per_frame_rates = data[data_start..data_end]
                    .chunks_exact(4)
                    .map(|b| u32::from_le_bytes(b.try_into().unwrap()))
                    .collect();
            }
            b"LIST"
                if chunk_size >= 4
                    && data_end <= data.len()
                    && &data[data_start..data_start + 4] == b"fram" =>
            {
                let mut inner = data_start + 4;
                while inner + 8 <= data_end {
                    let inner_id = &data[inner..inner + 4];
                    let inner_size = u32::from_le_bytes(
                        data[inner + 4..inner + 8].try_into().unwrap(),
                    ) as usize;
                    let inner_start = inner + 8;
                    let inner_end = inner_start.saturating_add(inner_size);

                    if inner_end > data.len() {
                        break;
                    }

                    if inner_id == b"icon" {
                        if let Ok(info) = parse_cur_bytes(&data[inner_start..inner_end]) {
                            frames.push(info);
                        }
                    }

                    // RIFF pads chunks to 2-byte boundaries.
                    inner = inner_end + (inner_size & 1);
                }
            }
            _ => {}
        }

        pos = data_end + (chunk_size & 1);
    }

    Ok(AniInfo {
        frame_count,
        display_rate,
        per_frame_rates,
        frames,
    })
}

// ── Thumbnail helpers ─────────────────────────────────────────────────────────

/// Return the largest frame by pixel area, along with its dimensions.
fn best_frame(cur: CurInfo) -> Result<(u32, u32, Vec<u8>), String> {
    cur.frames
        .into_iter()
        .max_by_key(|f| f.width * f.height)
        .map(|f| (f.width, f.height, f.rgba))
        .ok_or_else(|| "CUR has no frames".to_string())
}

/// Find the first `.cur` file in `pack_dir`, falling back to the first `.ani`.
fn find_first_cursor(pack_dir: &Path) -> Result<PathBuf, String> {
    let mut first_ani: Option<PathBuf> = None;
    for entry in fs::read_dir(pack_dir).map_err(|e| e.to_string())?.flatten() {
        let p = entry.path();
        if !p.is_file() {
            continue;
        }
        match p
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| e.to_ascii_lowercase())
            .as_deref()
        {
            Some("cur") => return Ok(p),
            Some("ani") if first_ani.is_none() => first_ani = Some(p),
            _ => {}
        }
    }
    first_ani.ok_or_else(|| "No cursor files found in pack".to_string())
}

// ── Commands ──────────────────────────────────────────────────────────────────

#[tauri::command]
fn get_cursor_thumbnail(
    app: tauri::AppHandle,
    pack_id: String,
    cursor_name: String,
) -> Result<String, String> {
    use base64::{engine::general_purpose::STANDARD, Engine};
    use image::{DynamicImage, ImageBuffer, Rgba};

    let base = packs_dir(&app)?;

    let path = if cursor_name.is_empty() {
        let pack_dir = base.join(&pack_id);
        if !pack_dir.starts_with(&base) {
            return Err("Invalid pack id".into());
        }
        find_first_cursor(&pack_dir)?
    } else {
        let p = base.join(&pack_id).join(&cursor_name);
        if !p.starts_with(&base) {
            return Err("Invalid path".into());
        }
        p
    };

    let data = fs::read(&path).map_err(|e| e.to_string())?;

    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_ascii_lowercase())
        .unwrap_or_default();

    let (width, height, rgba) = if ext == "ani" {
        let ani = parse_ani_bytes(&data)?;
        let first = ani.frames.into_iter().next().ok_or("ANI has no frames")?;
        best_frame(first)?
    } else {
        best_frame(parse_cur_bytes(&data)?)?
    };

    let img = DynamicImage::ImageRgba8(
        ImageBuffer::<Rgba<u8>, Vec<u8>>::from_raw(width, height, rgba)
            .ok_or("Failed to create image buffer")?,
    );

    let mut cursor = io::Cursor::new(Vec::new());
    img.write_to(&mut cursor, image::ImageOutputFormat::Png)
        .map_err(|e| e.to_string())?;

    Ok(STANDARD.encode(cursor.into_inner()))
}

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

#[tauri::command]
fn parse_cur(
    app: tauri::AppHandle,
    pack_id: String,
    cursor_name: String,
) -> Result<CurInfo, String> {
    let base = packs_dir(&app)?;
    let path = base.join(&pack_id).join(&cursor_name);
    if !path.starts_with(&base) {
        return Err("Invalid path".into());
    }
    let data = fs::read(&path).map_err(|e| e.to_string())?;
    parse_cur_bytes(&data)
}

#[tauri::command]
fn parse_ani(
    app: tauri::AppHandle,
    pack_id: String,
    cursor_name: String,
) -> Result<AniInfo, String> {
    let base = packs_dir(&app)?;
    let path = base.join(&pack_id).join(&cursor_name);
    if !path.starts_with(&base) {
        return Err("Invalid path".into());
    }
    let data = fs::read(&path).map_err(|e| e.to_string())?;
    parse_ani_bytes(&data)
}

// ── Entry point ───────────────────────────────────────────────────────────────

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            list_packs,
            import_pack,
            delete_pack,
            parse_cur,
            parse_ani,
            get_cursor_thumbnail
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a minimal 1×1 32-bit CUR byte blob by hand.
    ///
    /// Layout: ICONDIR(6) + ICONDIRENTRY(16) + BITMAPINFOHEADER(40) +
    ///         XOR pixels(4) + AND mask(4) = 70 bytes total.
    fn make_cur(hotspot_x: u16, hotspot_y: u16) -> Vec<u8> {
        let mut buf = Vec::<u8>::with_capacity(70);
        // ICONDIR (6 bytes)
        buf.extend_from_slice(&[0, 0]); // reserved
        buf.extend_from_slice(&[2, 0]); // type = 2 (cursor)
        buf.extend_from_slice(&[1, 0]); // count = 1
        // ICONDIRENTRY (16 bytes)  — offsets 6..22
        buf.push(1); // width
        buf.push(1); // height
        buf.push(0); // color count
        buf.push(0); // reserved
        buf.extend_from_slice(&hotspot_x.to_le_bytes()); // xHotspot  (file offset 10)
        buf.extend_from_slice(&hotspot_y.to_le_bytes()); // yHotspot  (file offset 12)
        buf.extend_from_slice(&48u32.to_le_bytes()); // image size = 48
        buf.extend_from_slice(&22u32.to_le_bytes()); // image offset = 22
        // BITMAPINFOHEADER (40 bytes)
        buf.extend_from_slice(&40u32.to_le_bytes()); // biSize
        buf.extend_from_slice(&1u32.to_le_bytes()); // biWidth = 1
        buf.extend_from_slice(&2u32.to_le_bytes()); // biHeight = 2 (XOR+AND rows)
        buf.extend_from_slice(&1u16.to_le_bytes()); // biPlanes
        buf.extend_from_slice(&32u16.to_le_bytes()); // biBitCount = 32
        buf.extend_from_slice(&0u32.to_le_bytes()); // biCompression = BI_RGB
        buf.extend_from_slice(&0u32.to_le_bytes()); // biSizeImage
        buf.extend_from_slice(&0u32.to_le_bytes()); // biXPelsPerMeter
        buf.extend_from_slice(&0u32.to_le_bytes()); // biYPelsPerMeter
        buf.extend_from_slice(&0u32.to_le_bytes()); // biClrUsed
        buf.extend_from_slice(&0u32.to_le_bytes()); // biClrImportant
        // XOR pixel row: 1×1 at 32 bpp → 4 bytes (BGRA)
        buf.extend_from_slice(&[0x00u8, 0x00, 0xFF, 0xFF]); // opaque blue
        // AND mask row: 1×1 at 1 bpp, padded to 4 bytes
        buf.extend_from_slice(&[0u8, 0, 0, 0]);
        buf
    }

    /// Build a minimal RIFF ACON byte blob with an `anih` chunk so we can
    /// verify header field extraction without needing real cursor images.
    fn make_ani_header_only(frame_count: u32, display_rate: u32) -> Vec<u8> {
        // ANIHEADER: 9 × u32 LE = 36 bytes
        let mut anih_data = vec![0u8; 36];
        let write_u32 = |buf: &mut Vec<u8>, off: usize, v: u32| {
            let b = v.to_le_bytes();
            buf[off..off + 4].copy_from_slice(&b);
        };
        write_u32(&mut anih_data, 0, 36); // cbSizeof
        write_u32(&mut anih_data, 4, frame_count); // nFrames
        write_u32(&mut anih_data, 8, frame_count); // nSteps
        write_u32(&mut anih_data, 28, display_rate); // iDispRate

        let mut buf = Vec::new();
        // RIFF header (filled in at the end)
        buf.extend_from_slice(b"RIFF");
        buf.extend_from_slice(&[0u8; 4]); // size placeholder
        buf.extend_from_slice(b"ACON");
        // anih chunk
        buf.extend_from_slice(b"anih");
        buf.extend_from_slice(&(36u32).to_le_bytes());
        buf.extend_from_slice(&anih_data);
        // Patch RIFF size = total - 8
        let total = buf.len();
        let riff_size = (total - 8) as u32;
        buf[4..8].copy_from_slice(&riff_size.to_le_bytes());
        buf
    }

    #[test]
    fn cur_hotspot_round_trip() {
        let data = make_cur(7, 12);
        let info = parse_cur_bytes(&data).unwrap();
        assert_eq!(info.frames.len(), 1);
        let f = &info.frames[0];
        assert_eq!((f.hotspot_x, f.hotspot_y), (7, 12));
        assert_eq!(f.width, 1);
        assert_eq!(f.height, 1);
        assert_eq!(f.rgba.len(), 4); // 1×1×4 bytes
    }

    #[test]
    fn cur_hotspot_zero() {
        let data = make_cur(0, 0);
        let info = parse_cur_bytes(&data).unwrap();
        let f = &info.frames[0];
        assert_eq!((f.hotspot_x, f.hotspot_y), (0, 0));
    }

    #[test]
    fn cur_rejects_short_input() {
        assert!(parse_cur_bytes(&[0u8; 3]).is_err());
    }

    #[test]
    fn ani_anih_fields_parsed() {
        let data = make_ani_header_only(8, 6);
        let info = parse_ani_bytes(&data).unwrap();
        assert_eq!(info.frame_count, 8);
        assert_eq!(info.display_rate, 6);
        assert!(info.frames.is_empty()); // no LIST fram chunk
        assert!(info.per_frame_rates.is_empty());
    }

    #[test]
    fn ani_rejects_non_riff() {
        assert!(parse_ani_bytes(b"WAVE\x00\x00\x00\x00ACON").is_err());
    }

    #[test]
    fn ani_rejects_non_acon() {
        let mut data = make_ani_header_only(1, 1);
        data[8..12].copy_from_slice(b"WAVE");
        assert!(parse_ani_bytes(&data).is_err());
    }

    #[test]
    fn ani_with_rate_chunk() {
        let cur_data = make_cur(0, 0);
        let cur_size = cur_data.len() as u32;

        // rate chunk: 3 delays
        let rates: [u32; 3] = [3, 6, 9];
        let rate_data: Vec<u8> = rates.iter().flat_map(|r| r.to_le_bytes()).collect();

        // LIST fram with one icon sub-chunk
        let icon_content = &cur_data;
        let mut list_content = Vec::new();
        list_content.extend_from_slice(b"fram");
        list_content.extend_from_slice(b"icon");
        list_content.extend_from_slice(&cur_size.to_le_bytes());
        list_content.extend_from_slice(icon_content);
        if cur_size % 2 == 1 {
            list_content.push(0);
        }

        let mut buf = Vec::new();
        buf.extend_from_slice(b"RIFF");
        buf.extend_from_slice(&[0u8; 4]); // placeholder
        buf.extend_from_slice(b"ACON");

        // anih
        let mut anih_data = vec![0u8; 36];
        anih_data[0..4].copy_from_slice(&36u32.to_le_bytes());
        anih_data[4..8].copy_from_slice(&1u32.to_le_bytes()); // nFrames
        anih_data[28..32].copy_from_slice(&4u32.to_le_bytes()); // iDispRate
        buf.extend_from_slice(b"anih");
        buf.extend_from_slice(&36u32.to_le_bytes());
        buf.extend_from_slice(&anih_data);

        // rate
        buf.extend_from_slice(b"rate");
        buf.extend_from_slice(&(rate_data.len() as u32).to_le_bytes());
        buf.extend_from_slice(&rate_data);

        // LIST fram
        buf.extend_from_slice(b"LIST");
        buf.extend_from_slice(&(list_content.len() as u32).to_le_bytes());
        buf.extend_from_slice(&list_content);

        let riff_size = (buf.len() - 8) as u32;
        buf[4..8].copy_from_slice(&riff_size.to_le_bytes());

        let info = parse_ani_bytes(&buf).unwrap();
        assert_eq!(info.frame_count, 1);
        assert_eq!(info.display_rate, 4);
        assert_eq!(info.per_frame_rates, vec![3, 6, 9]);
        assert_eq!(info.frames.len(), 1);
        assert_eq!(info.frames[0].frames[0].hotspot_x, 0);
    }
}


