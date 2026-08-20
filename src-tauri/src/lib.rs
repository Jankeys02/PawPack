use std::collections::HashMap;
use std::fs;
use std::io;
use std::path::{Component, Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use tauri::Manager;

pub mod slideshow;

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

/// One cursor file entry returned by `list_pack_cursors`.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct CursorEntry {
    pub name: String,
    pub kind: String,   // "cur" | "ani"
    pub thumbnail: String, // base64 PNG, empty string on decode error
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

/// A single cursor file mapped to its Windows system role.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct CursorAssignment {
    /// Registry value name, e.g. "Arrow", "Wait", "Hand".
    pub role: String,
    /// Filename inside the pack, e.g. "aero_arrow.cur".
    pub file: String,
}

/// Result of scanning a pack's cursor files.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct PackAssignmentResult {
    /// Files that matched a known system cursor role.
    pub assigned: Vec<CursorAssignment>,
    /// Filenames that could not be matched to any role.
    pub unmatched: Vec<String>,
}

/// One role in the mix, resolved to the pack and file that fill it.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct MixEntry {
    pub role: String,
    /// Pack directory name, e.g. "bog-cursor-pack".
    pub pack: String,
    pub file: String,
}

/// The mix as the UI sees it: entries whose files are present, and entries
/// left dangling by a deleted pack or file.
#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct MixResult {
    pub roles: Vec<MixEntry>,
    pub stale: Vec<MixEntry>,
}

/// On-disk shape of `packs/mix.json`.
#[derive(Serialize, Deserialize, Clone, Debug, Default)]
struct MixFile {
    roles: HashMap<String, MixRef>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
struct MixRef {
    pack: String,
    file: String,
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
    /// Set when one download held several complete cursor sets. Packs sharing
    /// a group are shown as one entry with a variant switcher.
    ///
    /// Each variant stays its own pack on disk: a variant *is* a full set of
    /// roles, so it is the thing that gets applied. Grouping is presentation,
    /// which is why nothing in the apply, mix or editor paths has to know.
    #[serde(default)]
    pub group: Option<String>,
    /// This pack's name within its group ("Moga Black"). None when ungrouped.
    #[serde(default)]
    pub variant: Option<String>,
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

/// True only for a single, ordinary path segment — one `Component::Normal`.
///
/// Rejects `..`, `.`, anything containing a separator, and Windows prefixes
/// like `C:`. This is what the path guards need, rather than the
/// `path.starts_with(&base)` they used to do: `starts_with` compares
/// components, so `base.join("..")` starts with `base` and a `..` walked
/// straight out of the packs directory. Checking the untrusted segment itself
/// is both stricter and works on paths that do not exist yet, which
/// `canonicalize` cannot.
fn is_safe_segment(seg: &str) -> bool {
    let mut parts = Path::new(seg).components();
    matches!(parts.next(), Some(Component::Normal(_))) && parts.next().is_none()
}

/// Resolve `<packs>/<pack_id>`, refusing any id that escapes `base`.
fn pack_dir_in(base: &Path, pack_id: &str) -> Result<PathBuf, String> {
    if !is_safe_segment(pack_id) {
        return Err("Invalid pack id".into());
    }
    Ok(base.join(pack_id))
}

/// Resolve `<packs>/<pack_id>/<file>`, refusing either segment if it escapes.
pub(crate) fn pack_file_in(base: &Path, pack_id: &str, file: &str) -> Result<PathBuf, String> {
    if !is_safe_segment(file) {
        return Err("Invalid path".into());
    }
    Ok(pack_dir_in(base, pack_id)?.join(file))
}

/// Load per-pack role overrides. Keys are registry role names; values are
/// filenames (`Some`) or explicit clears (`None` → role gets no file).
fn load_overrides(pack_dir: &Path) -> HashMap<String, Option<String>> {
    fs::read_to_string(pack_dir.join("overrides.json"))
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

fn save_overrides(pack_dir: &Path, overrides: &HashMap<String, Option<String>>) -> Result<(), String> {
    let json = serde_json::to_string_pretty(overrides).map_err(|e| e.to_string())?;
    fs::write(pack_dir.join("overrides.json"), json).map_err(|e| e.to_string())
}

fn mix_path(packs_base: &Path) -> PathBuf {
    packs_base.join("mix.json")
}

/// Read `mix.json`, treating an absent or unparseable file as an empty mix.
/// Losing a corrupt mix is recoverable; refusing to open the tab is not.
fn load_mix(packs_base: &Path) -> MixFile {
    fs::read_to_string(mix_path(packs_base))
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

/// Split the stored mix into usable and stale entries.
///
/// Stale entries are reported but deliberately left in `mix.json`: a pack that
/// is deleted and reimported under the same id gets its assignments back.
pub fn read_mix(packs_base: &Path) -> MixResult {
    let mix = load_mix(packs_base);
    let mut roles = Vec::new();
    let mut stale = Vec::new();

    for (role, r) in mix.roles {
        let entry = MixEntry { role, pack: r.pack, file: r.file };
        // Validated here, not just in `set_mix_role`: mix.json is a plain file
        // that an older build or a hand edit could have filled with a
        // traversing pack/file pair. Anything that escapes counts as stale.
        if pack_file_in(packs_base, &entry.pack, &entry.file).is_ok_and(|p| p.is_file()) {
            roles.push(entry);
        } else {
            stale.push(entry);
        }
    }

    roles.sort_by(|a, b| a.role.cmp(&b.role));
    stale.sort_by(|a, b| a.role.cmp(&b.role));
    MixResult { roles, stale }
}

/// Set or clear one role. An empty `pack` clears it.
pub fn write_mix_role(
    packs_base: &Path,
    role: &str,
    pack: &str,
    file: &str,
) -> Result<MixResult, String> {
    let mut mix = load_mix(packs_base);
    if pack.is_empty() {
        mix.roles.remove(role);
    } else {
        mix.roles.insert(
            role.to_string(),
            MixRef { pack: pack.to_string(), file: file.to_string() },
        );
    }

    let json = serde_json::to_string_pretty(&mix).map_err(|e| e.to_string())?;
    fs::write(mix_path(packs_base), json).map_err(|e| e.to_string())?;
    Ok(read_mix(packs_base))
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
/// True if `dir` is itself a cursor pack: Windows cursor files sitting directly
/// in it, or an Xcursor theme's `cursors/` subdirectory.
fn is_pack_dir(dir: &Path) -> bool {
    if has_cursor_files(dir) {
        return true;
    }
    // Xcursor files carry no extension, so require the conventional layout
    // rather than treating any directory of extensionless files as a theme.
    let cursors = dir.join("cursors");
    cursors.is_dir()
        && fs::read_dir(&cursors)
            .map(|e| e.flatten().any(|e| e.path().is_file() && e.path().extension().is_none()))
            .unwrap_or(false)
}

/// How far below the archive root to look for pack directories. Real packs nest
/// their variants one or two levels down (`Windows/Moga Black/`); anything
/// deeper is a payload we have no business importing.
const VARIANT_SEARCH_DEPTH: usize = 3;

fn collect_variants(dir: &Path, depth: usize, out: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else { return };
    for sub in entries.flatten().map(|e| e.path()).filter(|p| p.is_dir()) {
        if is_pack_dir(&sub) {
            // A pack directory is a leaf: its own subfolders are its contents,
            // not further variants.
            out.push(sub);
        } else if depth > 0 {
            collect_variants(&sub, depth - 1, out);
        }
    }
}

/// Every directory in `extracted` that is a pack in its own right.
///
/// Archives from the theme sites routinely ship several at once — a static and
/// an animated set, or light/dark/colour variants under `Windows/`, alongside
/// GIF previews, PDFs and macOS tooling. Returning just one of those (or, when
/// none sits exactly one level down, falling back to the archive root) imports
/// a pack with no cursors in it and megabytes of unrelated payload.
///
/// Sorted so the order does not depend on `read_dir`, which is not stable
/// across machines. Falls back to the root so a shape we do not recognise is
/// still imported rather than silently dropped.
fn find_pack_variants(extracted: &Path) -> Vec<PathBuf> {
    // A root holding cursors is one pack, not a container of them.
    if is_pack_dir(extracted) {
        return vec![extracted.to_path_buf()];
    }

    let mut out = Vec::new();
    collect_variants(extracted, VARIANT_SEARCH_DEPTH, &mut out);
    out.sort();

    if out.is_empty() {
        out.push(extracted.to_path_buf());
    }
    out
}

/// Core import logic shared by both zip and folder imports.
/// Strip the noise download sites bolt onto a filename, so a group is titled
/// "miyabi-zzz" rather than "miyabi-zzz_6qcd_VSTHEMES-ORG".
///
/// Two kinds of suffix come off, repeatedly: the site's own marker, and the
/// short generated id these sites append to make each download unique. Splitting
/// only on `_` matters — the marker contains a hyphen of its own, and plenty of
/// real pack names are hyphenated.
fn tidy_download_name(raw: &str) -> String {
    const MARKERS: &[&str] = &["vsthemes-org", "vsthemes"];
    fn sep(s: &str) -> &str {
        s.trim_end_matches(['-', '_', ' ', '.'])
    }

    let mut out = sep(raw);
    loop {
        let lower = out.to_ascii_lowercase();
        if let Some(m) = MARKERS.iter().find(|m| lower.ends_with(*m)) {
            out = sep(&out[..out.len() - m.len()]);
            continue;
        }
        // "_6qcd", "_992bd4d17b" — alphanumeric, has a digit, and short enough
        // that a real word like "cursors" or a version like "v2" is not eaten.
        let id_like = out.rsplit_once('_').filter(|(head, tail)| {
            !head.is_empty()
                && (4..=12).contains(&tail.len())
                && tail.chars().all(|c| c.is_ascii_alphanumeric())
                && tail.chars().any(|c| c.is_ascii_digit())
        });
        match id_like {
            Some((head, _)) => out = sep(head),
            None => break,
        }
    }

    // A pack legitimately named after the site keeps its name.
    if out.is_empty() { sep(raw).to_string() } else { out.to_string() }
}

fn do_import(
    app: &tauri::AppHandle,
    source: &Path,
    raw_name: &str,
    group: Option<(String, String)>,
) -> Result<PackMeta, String> {
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
        group: group.as_ref().map(|(g, _)| g.clone()),
        variant: group.map(|(_, v)| v),
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

    // Bytes 4..8 of an ICONDIRENTRY hold the hotspot only when the directory's
    // idType is 2 (cursor). For idType 1 (icon), which an .ani may legally
    // embed, they are wPlanes/wBitCount — reading those as coordinates reports
    // a nonsense hotspot. `set_cur_hotspots` refuses to write them for the same
    // reason, so the two sides agree.
    let is_cursor = u16::from_le_bytes([data[2], data[3]]) == 2;

    // Collect hotspots from raw directory entries (6-byte header + 16 bytes each).
    let hotspots: Vec<(u16, u16)> = (0..image_count)
        .map(|i| {
            let base = 6 + i * 16;
            if !is_cursor || base + 8 > data.len() {
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

/// Byte ranges of every embedded `icon` chunk in a RIFF ACON blob, in
/// `LIST fram` order. Shared by the ANI parser and the hotspot editor.
fn ani_icon_ranges(data: &[u8]) -> Vec<(usize, usize)> {
    let mut out = Vec::new();
    if data.len() < 12 || &data[0..4] != b"RIFF" || &data[8..12] != b"ACON" {
        return out;
    }

    let riff_size = u32::from_le_bytes(data[4..8].try_into().unwrap()) as usize;
    let end = 8usize.saturating_add(riff_size).min(data.len());
    let mut pos = 12usize; // skip "RIFF" + size + "ACON"

    while pos + 8 <= end {
        let chunk_size =
            u32::from_le_bytes(data[pos + 4..pos + 8].try_into().unwrap()) as usize;
        let data_start = pos + 8;
        let data_end = data_start.saturating_add(chunk_size);
        if data_end > data.len() {
            break;
        }

        if &data[pos..pos + 4] == b"LIST"
            && chunk_size >= 4
            && &data[data_start..data_start + 4] == b"fram"
        {
            let mut inner = data_start + 4;
            while inner + 8 <= data_end {
                let inner_size =
                    u32::from_le_bytes(data[inner + 4..inner + 8].try_into().unwrap()) as usize;
                let inner_start = inner + 8;
                let inner_end = inner_start.saturating_add(inner_size);
                if inner_end > data.len() {
                    break;
                }
                if &data[inner..inner + 4] == b"icon" {
                    out.push((inner_start, inner_end));
                }
                // RIFF pads chunks to 2-byte boundaries.
                inner = inner_end + (inner_size & 1);
            }
        }

        pos = data_end + (chunk_size & 1);
    }

    out
}

/// Rewrite the hotspot of every `ICONDIRENTRY` in a CUR/ICO blob, in place.
///
/// `x`/`y` are pixel coordinates inside a `ref_w`x`ref_h` reference frame — the
/// image the user actually clicked on. Each entry is rescaled to its own
/// dimensions so multi-resolution cursors keep the same visual hotspot.
/// Byte length never changes, so this is safe on ANI-embedded frames too.
fn set_cur_hotspots(data: &mut [u8], x: u32, y: u32, ref_w: u32, ref_h: u32) {
    if data.len() < 6 || ref_w == 0 || ref_h == 0 {
        return;
    }

    // Bytes 4..8 of an ICONDIRENTRY are the hotspot only for idType 2
    // (cursor). For idType 1 (icon), which an ANI may legally embed, they are
    // wPlanes/wBitCount — writing coordinates there would corrupt the entry.
    if u16::from_le_bytes([data[2], data[3]]) != 2 {
        return;
    }

    let count = u16::from_le_bytes([data[4], data[5]]) as usize;
    for i in 0..count {
        let base = 6 + i * 16;
        if base + 16 > data.len() {
            break;
        }
        // Width/height bytes are 0 for 256 px.
        let dim = |b: u8| if b == 0 { 256u32 } else { b as u32 };
        let (w, h) = (dim(data[base]), dim(data[base + 1]));

        let hx = (x * w / ref_w).min(w.saturating_sub(1)) as u16;
        let hy = (y * h / ref_h).min(h.saturating_sub(1)) as u16;
        data[base + 4..base + 6].copy_from_slice(&hx.to_le_bytes());
        data[base + 6..base + 8].copy_from_slice(&hy.to_le_bytes());
    }
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

    let frames: Vec<CurInfo> = ani_icon_ranges(data)
        .into_iter()
        .filter_map(|(s, e)| parse_cur_bytes(&data[s..e]).ok())
        .collect();

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

/// Longest side of a generated thumbnail, in pixels.
///
/// Thumbnails render into 20-40px boxes, so anything larger is downscaled by
/// the browser regardless. Packs ship cursors up to 256x256 with dozens of
/// frames: one 46-frame 256x256 cursor decodes to 12 MB of RGBA to produce a
/// 40px preview. 96 leaves room for a 2x display and, conveniently, leaves the
/// common 32x32 and 96x96 cursors untouched.
const THUMBNAIL_MAX_PX: u32 = 96;

/// Downscale one frame so its longest side fits `THUMBNAIL_MAX_PX`, keeping
/// the aspect ratio. Frames already within the cap are returned untouched, so
/// the usual cursor sizes are never resampled and stay pixel-exact.
fn cap_frame(w: u32, h: u32, rgba: Vec<u8>) -> (u32, u32, Vec<u8>) {
    let expected = w as usize * h as usize * 4;
    if w.max(h) <= THUMBNAIL_MAX_PX || rgba.len() != expected {
        return (w, h, rgba);
    }

    let scale = THUMBNAIL_MAX_PX as f32 / w.max(h) as f32;
    let nw = ((w as f32 * scale).round() as u32).max(1);
    let nh = ((h as f32 * scale).round() as u32).max(1);

    let buf = image::ImageBuffer::<image::Rgba<u8>, Vec<u8>>::from_raw(w, h, rgba)
        .expect("buffer length checked above");
    let out = image::imageops::resize(&buf, nw, nh, image::imageops::FilterType::Triangle);
    (nw, nh, out.into_raw())
}

/// Encode RGBA frames as a looping APNG.
///
/// Frames are centred on a canvas sized to the largest width and the largest
/// height present, each taken independently, and padded with transparency.
/// Padding rather than scaling: these are pixel-art cursors and scaling blurs
/// them.
///
/// `delays_jiffies` holds one delay per frame in jiffies (1/60 s), the unit
/// `.ani` files use. When it is shorter than `frames`, the last value repeats.
///
/// ponytail: frames encode in file order. `.ani` also allows a `seq` chunk
/// that reorders playback; `parse_ani_bytes` does not read it, so a pack using
/// `seq` animates in the wrong order. Parse `seq` in `parse_ani_bytes` and
/// reorder here if that ever shows up.
fn encode_animated_png(
    frames: &[(u32, u32, Vec<u8>)],
    delays_jiffies: &[u32],
) -> Result<Vec<u8>, String> {
    if frames.is_empty() {
        return Err("ANI has no frames".into());
    }

    let canvas_w = frames
        .iter()
        .map(|(w, _, _)| *w)
        .max()
        .expect("frames is non-empty");
    let canvas_h = frames
        .iter()
        .map(|(_, h, _)| *h)
        .max()
        .expect("frames is non-empty");
    if canvas_w == 0 || canvas_h == 0 {
        return Err("ANI frames have zero size".into());
    }
    // canvas_w/canvas_h are maxima taken independently across frames, so they
    // need never have coexisted on one image (e.g. a 65535x1 frame beside a
    // 1x65535 frame). Without this ceiling, canvas_w * canvas_h * 4 can
    // overflow u32 or demand a multi-GB allocation for a file no real cursor
    // produces.
    if canvas_w as u64 * canvas_h as u64 > 4096 * 4096 {
        return Err("ANI canvas too large".into());
    }

    let mut buf: Vec<u8> = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut buf, canvas_w, canvas_h);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        // The frame count must match the number of frames written below;
        // the png crate enforces this itself and writer.finish() below
        // returns a MissingFrames error on mismatch. 0 plays means loop
        // forever.
        encoder
            .set_animated(frames.len() as u32, 0)
            .map_err(|e| e.to_string())?;

        let mut writer = encoder.write_header().map_err(|e| e.to_string())?;

        for (i, (w, h, rgba)) in frames.iter().enumerate() {
            let jiffies = delays_jiffies
                .get(i)
                .or_else(|| delays_jiffies.last())
                .copied()
                .unwrap_or(1)
                .max(1);
            let delay = u16::try_from(jiffies).unwrap_or(u16::MAX);
            writer
                .set_frame_delay(delay, 60)
                .map_err(|e| e.to_string())?;
            writer
                .write_image_data(&center_on_canvas(*w, *h, rgba, canvas_w, canvas_h))
                .map_err(|e| e.to_string())?;
        }

        writer.finish().map_err(|e| e.to_string())?;
    }

    Ok(buf)
}

/// Copy one RGBA frame into the middle of a transparent canvas.
fn center_on_canvas(
    w: u32,
    h: u32,
    rgba: &[u8],
    canvas_w: u32,
    canvas_h: u32,
) -> Vec<u8> {
    if w == canvas_w && h == canvas_h {
        return rgba.to_vec();
    }

    let mut out = vec![0u8; (canvas_w * canvas_h * 4) as usize];
    let off_x = (canvas_w.saturating_sub(w) / 2) as usize;
    let off_y = (canvas_h.saturating_sub(h) / 2) as usize;
    let row_bytes = (w * 4) as usize;

    for y in 0..h as usize {
        let src = y * row_bytes;
        if src + row_bytes > rgba.len() {
            break;
        }
        let dst = ((y + off_y) * canvas_w as usize + off_x) * 4;
        if dst + row_bytes > out.len() {
            break;
        }
        out[dst..dst + row_bytes].copy_from_slice(&rgba[src..src + row_bytes]);
    }

    out
}

fn cursor_path_to_b64(path: &Path) -> Result<String, String> {
    use base64::{engine::general_purpose::STANDARD, Engine};

    let data = fs::read(path).map_err(|e| e.to_string())?;
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_ascii_lowercase())
        .unwrap_or_default();

    if ext == "ani" {
        let ani = parse_ani_bytes(&data)?;
        let display_rate = ani.display_rate;
        let rates = ani.per_frame_rates;

        // Drop frames with no image variants rather than failing the cursor.
        // Each delay is zipped to its frame before filtering, so a dropped
        // frame takes its delay with it instead of shifting the delays of
        // the frames that follow it out of alignment.
        let (frames, delays): (Vec<(u32, u32, Vec<u8>)>, Vec<u32>) = ani
            .frames
            .into_iter()
            .enumerate()
            .filter_map(|(i, f)| {
                best_frame(f)
                    .ok()
                    .map(|(w, h, rgba)| cap_frame(w, h, rgba))
                    .map(|fr| (fr, rates.get(i).copied().unwrap_or(display_rate)))
            })
            .unzip();

        if frames.is_empty() {
            return Err("ANI has no frames".into());
        }

        if frames.len() > 1 {
            return Ok(STANDARD.encode(encode_animated_png(&frames, &delays)?));
        }

        let (width, height, rgba) = frames.into_iter().next().unwrap();
        return Ok(STANDARD.encode(still_png(width, height, rgba)?));
    }

    let (width, height, rgba) = best_frame(parse_cur_bytes(&data)?)?;
    let (width, height, rgba) = cap_frame(width, height, rgba);
    Ok(STANDARD.encode(still_png(width, height, rgba)?))
}

/// Encode one RGBA frame as a plain PNG.
fn still_png(width: u32, height: u32, rgba: Vec<u8>) -> Result<Vec<u8>, String> {
    use image::{DynamicImage, ImageBuffer, Rgba};

    let img = DynamicImage::ImageRgba8(
        ImageBuffer::<Rgba<u8>, Vec<u8>>::from_raw(width, height, rgba)
            .ok_or("Failed to create image buffer")?,
    );

    let mut buf = io::Cursor::new(Vec::new());
    img.write_to(&mut buf, image::ImageFormat::Png)
        .map_err(|e| e.to_string())?;

    Ok(buf.into_inner())
}

// ── Windows cursor apply/revert ───────────────────────────────────────────────

// Items are only reachable from cfg(windows) command bodies; suppress the
// cross-platform dead-code noise that appears before the crate is first built.
#[cfg_attr(not(target_os = "windows"), allow(dead_code))]
#[cfg(target_os = "windows")]
mod windows_cursor {
    use std::collections::HashMap;
    use std::fs;
    use std::path::{Path, PathBuf};

    use serde::{Deserialize, Serialize};
    use winreg::{
        enums::{HKEY_CURRENT_USER, KEY_SET_VALUE},
        RegKey,
    };
    use windows::Win32::UI::WindowsAndMessaging::{
        SystemParametersInfoW, SPI_GETCURSORSHADOW, SPI_GETMOUSESONAR, SPI_GETMOUSEVANISH,
        SPI_GETSNAPTODEFBUTTON, SPI_SETCURSORS, SPI_SETCURSORSHADOW, SPI_SETMOUSESONAR,
        SPI_SETMOUSEVANISH, SPI_SETSNAPTODEFBUTTON, SPIF_SENDCHANGE, SPIF_UPDATEINIFILE,
        SYSTEM_PARAMETERS_INFO_ACTION,
    };

    /// All `HKCU\Control Panel\Cursors` value names we snapshot and restore.
    pub const CURSOR_REG_NAMES: &[&str] = &[
        "Arrow", "Help", "AppStarting", "Wait", "Crosshair", "IBeam",
        "NWPen", "No", "SizeNS", "SizeWE", "SizeNWSE", "SizeNESW",
        "SizeAll", "UpArrow", "Hand", "Pin", "Person",
    ];

    /// Registry snapshot taken before a pack is applied.
    ///
    /// `values` maps each cursor registry name to its previous path.
    /// `None` means the value was absent — on revert we delete it so Windows
    /// falls back to its built-in default rather than seeing an empty string.
    /// `scheme` is the `(Default)` value of the Cursors key (the active scheme
    /// name shown in the control panel), also restored on revert.
    #[derive(Serialize, Deserialize, Clone, Debug)]
    pub struct CursorSnapshot {
        pub values: HashMap<String, Option<String>>,
        #[serde(default)]
        pub scheme: Option<String>,
    }

    impl CursorSnapshot {
        /// The state Windows ships with: every role absent, so the OS falls
        /// back to its built-in cursors, and no named scheme.
        pub fn windows_default() -> Self {
            CursorSnapshot {
                values: CURSOR_REG_NAMES.iter().map(|n| (n.to_string(), None)).collect(),
                scheme: None,
            }
        }

        /// True when any value points inside `packs_base`, meaning this
        /// captured a state PawPack applied rather than the user's own cursors.
        pub fn is_pack_owned(&self, packs_base: &Path) -> bool {
            let base = packs_base.to_string_lossy().to_lowercase();
            self.values
                .values()
                .flatten()
                .any(|v| v.to_lowercase().starts_with(&base))
        }
    }

    /// Keyword → registry role, ordered most specific first.
    ///
    /// Filenames are matched keyword-by-keyword rather than by exact stem, so
    /// packs that decorate their names ("Brushbuddy-link-pointer-static.cur")
    /// still resolve. Order is the tie-breaker when a name contains several
    /// keywords: that example holds both "link" and "pointer", and the first
    /// entry found wins, so it lands on Hand rather than Arrow.
    ///
    /// Two consequences worth preserving when editing this table:
    /// - Longer variants must precede the shorter ones they contain
    ///   ("crosshair" before "cross", "uparrow" before "up").
    /// - "select" is last of all. Windows' own names are "Normal Select",
    ///   "Help Select", "Precision Select" and so on, where the *qualifier*
    ///   carries the role and a bare "select" only means Hand ("Link Select")
    ///   once every qualifier has already been ruled out.
    const ROLE_KEYWORDS: &[(&str, &str)] = &[
        ("appstarting", "AppStarting"),
        ("working in bg", "AppStarting"),
        ("arrow_wait", "AppStarting"),
        ("busy2", "AppStarting"),
        ("working", "AppStarting"),
        ("work", "AppStarting"),
        ("hourglass", "Wait"),
        ("loading", "Wait"),
        ("busy", "Wait"),
        ("wait", "Wait"),
        ("helpsel", "Help"),
        ("arrow_help", "Help"),
        ("help", "Help"),
        ("personselect", "Person"),
        ("person", "Person"),
        ("handpoint", "Hand"),
        ("pointing", "Hand"),
        ("finger", "Hand"),
        ("link", "Hand"),
        ("hand", "Hand"),
        ("point", "Hand"),
        ("crosshair", "Crosshair"),
        ("precision", "Crosshair"),
        ("cross", "Crosshair"),
        ("unavailable", "No"),
        ("forbidden", "No"),
        ("nodrop", "No"),
        ("no", "No"),
        ("uparrow", "UpArrow"),
        ("alternate", "UpArrow"),
        ("alt", "UpArrow"),
        ("up", "UpArrow"),
        ("nwpen", "NWPen"),
        ("pen", "NWPen"),
        ("ibeam", "IBeam"),
        ("beam", "IBeam"),
        ("text", "IBeam"),
        ("sizenwse", "SizeNWSE"),
        ("sizenws", "SizeNWSE"),
        ("sizenw", "SizeNWSE"),
        ("nwse", "SizeNWSE"),
        ("diag 1", "SizeNWSE"),
        ("sizenewsw", "SizeNESW"),
        ("sizenes", "SizeNESW"),
        ("sizene", "SizeNESW"),
        ("nesw", "SizeNESW"),
        ("diag 2", "SizeNESW"),
        ("sizens", "SizeNS"),
        ("sizev", "SizeNS"),
        ("ns", "SizeNS"),
        ("ver", "SizeNS"),
        ("sizewe", "SizeWE"),
        ("sizeh", "SizeWE"),
        ("we", "SizeWE"),
        ("hor", "SizeWE"),
        ("sizeall", "SizeAll"),
        ("fleur", "SizeAll"),
        ("move", "SizeAll"),
        ("location", "Pin"),
        ("loaction", "Pin"), // common misspelling seen in the wild
        ("pin", "Pin"),
        ("pointer", "Arrow"),
        ("standard", "Arrow"),
        ("classic", "Arrow"),
        ("normal", "Arrow"),
        ("default", "Arrow"),
        ("idle", "Arrow"),
        ("arrow", "Arrow"),
        ("select", "Hand"),
    ];

    /// Lowercase a stem and reduce every run of non-alphanumerics to a single
    /// space, padded at both ends, so keywords can be matched as whole words.
    ///
    /// Whole-word matching is what keeps this safe: a plain substring search
    /// would find "no" inside "normal" and "we" inside "Sweep".
    fn normalize_stem(stem: &str) -> String {
        let mut out = String::with_capacity(stem.len() + 2);
        out.push(' ');
        for c in stem.chars() {
            if c.is_alphanumeric() {
                out.extend(c.to_lowercase());
            } else if !out.ends_with(' ') {
                out.push(' ');
            }
        }
        if !out.ends_with(' ') {
            out.push(' ');
        }
        out
    }

    /// Find the role for a cursor file stem, plus the index of the keyword that
    /// matched. A lower index means a more specific keyword, which
    /// `get_assignments` uses to break ties when two files claim one role.
    pub fn role_match(stem: &str) -> Option<(&'static str, usize)> {
        let norm = normalize_stem(stem);
        ROLE_KEYWORDS
            .iter()
            .enumerate()
            .find(|(_, (keyword, _))| norm.contains(&format!(" {keyword} ")))
            .map(|(rank, (_, role))| (*role, rank))
    }

    /// Path of the install-wide revert snapshot, migrating any leftover
    /// per-pack `revert.json` on the way.
    ///
    /// Older builds kept one snapshot per pack and rewrote it on every apply,
    /// so a pack applied twice ended up recording *itself* as the original. A
    /// leftover file is therefore only trustworthy when nothing in it points
    /// into the packs directory:
    ///
    /// - trustworthy, and no shared snapshot yet → promote it, so an install
    ///   that applied a pack exactly once keeps its real cursors;
    /// - points into a pack → delete it, since restoring it could only ever
    ///   reinstate a pack;
    /// - trustworthy but a shared snapshot already exists → leave it alone.
    ///   Two plausible originals is not a reason to destroy one.
    pub fn snapshot_path(packs_base: &Path) -> PathBuf {
        let shared = packs_base.join("revert.json");

        for entry in fs::read_dir(packs_base).into_iter().flatten().flatten() {
            let legacy = entry.path().join("revert.json");
            if !legacy.is_file() {
                continue;
            }

            let salvageable = fs::read_to_string(&legacy)
                .ok()
                .and_then(|s| serde_json::from_str::<CursorSnapshot>(&s).ok())
                .is_some_and(|snap| !snap.is_pack_owned(packs_base));

            if salvageable {
                if !shared.exists() {
                    let _ = fs::rename(&legacy, &shared);
                }
            } else {
                let _ = fs::remove_file(&legacy);
            }
        }

        shared
    }

    /// Read the current registry state into a snapshot.
    pub fn snapshot() -> Result<CursorSnapshot, String> {
        let hkcu = RegKey::predef(HKEY_CURRENT_USER);
        let key = hkcu
            .open_subkey("Control Panel\\Cursors")
            .map_err(|e| format!("Cannot open cursor registry key: {e}"))?;

        let mut values = HashMap::new();
        for &name in CURSOR_REG_NAMES {
            // Use Option: None = key absent, Some("") = key present but empty.
            let val: Option<String> = key.get_value(name).ok();
            values.insert(name.to_string(), val);
        }

        // The (Default) value holds the active scheme name (e.g. "Windows Default").
        let scheme: Option<String> = key.get_value("").ok();

        Ok(CursorSnapshot { values, scheme })
    }

    /// Scan `source_dir` for cursor files, apply any saved overrides, and return
    /// matched (assigned) and unmatched files separately.
    pub fn get_assignments(source_dir: &Path) -> Result<super::PackAssignmentResult, String> {
        // 1. Auto-detect role for each cursor file.
        //
        // Values carry the matched keyword's rank so a later file can only
        // displace an earlier one on merit; without it the winner would depend
        // on `read_dir` order, which is not stable across machines.
        let mut all_files: Vec<String> = Vec::new();
        let mut role_map: HashMap<String, (String, usize)> = HashMap::new(); // role → (file, rank)

        for entry in fs::read_dir(source_dir).map_err(|e| e.to_string())?.flatten() {
            let path = entry.path();
            if !path.is_file() { continue; }
            let ext = path.extension().and_then(|e| e.to_str())
                .map(|e| e.to_ascii_lowercase()).unwrap_or_default();
            if ext != "cur" && ext != "ani" { continue; }
            let file = path.file_name().and_then(|n| n.to_str()).unwrap_or("").to_string();
            let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or_default();
            if let Some((reg_name, rank)) = role_match(stem) {
                // Rank first, then animated over static (a pack shipping both
                // means the .ani is the interesting one), then name order.
                let is_static = ext == "cur";
                let better = match role_map.get(reg_name) {
                    None => true,
                    Some((held, held_rank)) => {
                        let held_static = held.to_ascii_lowercase().ends_with(".cur");
                        (rank, is_static, file.as_str()) < (*held_rank, held_static, held.as_str())
                    }
                };
                if better {
                    role_map.insert(reg_name.to_string(), (file.clone(), rank));
                }
            }
            all_files.push(file);
        }

        // 2. Apply per-pack overrides on top of auto-detection.
        //
        // One file may serve several roles — the registry allows it, and packs
        // routinely reuse a single arrow for both Arrow and AppStarting — so an
        // override claims its role without evicting the file's other roles.
        let overrides = super::load_overrides(source_dir);
        for (role, file_opt) in &overrides {
            role_map.remove(role);
            match file_opt {
                Some(file) => {
                    if source_dir.join(file).is_file() {
                        role_map.insert(role.clone(), (file.clone(), 0));
                    }
                }
                None => {} // role explicitly cleared; already removed above
            }
        }

        // 3. Build result.
        let assigned: Vec<super::CursorAssignment> = role_map.into_iter()
            .map(|(role, (file, _rank))| super::CursorAssignment { role, file })
            .collect();
        let assigned_files: Vec<&str> = assigned.iter().map(|a| a.file.as_str()).collect();
        let unmatched: Vec<String> = all_files.into_iter()
            .filter(|f| !assigned_files.contains(&f.as_str()))
            .collect();

        Ok(super::PackAssignmentResult { assigned, unmatched })
    }

    /// Point each named role at an absolute cursor path and broadcast the
    /// change. Any role in `CURSOR_REG_NAMES` absent from `paths` has its
    /// value deleted so Windows falls back to its built-in cursor.
    ///
    /// This is the only function that writes `HKCU\Control Panel\Cursors`.
    /// Both applying a pack and applying a mix route through it, so the two
    /// cannot drift apart in how they treat unfilled roles.
    pub fn write_roles(paths: &HashMap<&str, PathBuf>) -> Result<(), String> {
        let hkcu = RegKey::predef(HKEY_CURRENT_USER);
        let reg_key = hkcu
            .open_subkey_with_flags("Control Panel\\Cursors", KEY_SET_VALUE)
            .map_err(|e| format!("Cannot open cursor registry key for writing: {e}"))?;

        for &reg_name in CURSOR_REG_NAMES {
            match paths.get(reg_name) {
                Some(path) => {
                    let path_str = path.to_string_lossy().into_owned();
                    reg_key
                        .set_value(reg_name, &path_str)
                        .map_err(|e| format!("Cannot set registry value {reg_name}: {e}"))?;
                }
                None => {
                    let _ = reg_key.delete_value(reg_name);
                }
            }
        }

        call_spi_set_cursors()
    }

    /// Write `HKCU\Control Panel\Cursors` registry values pointing directly at
    /// the cursor files in `source_dir` (the app's own packs directory — no
    /// copy to %SystemRoot% needed, no elevation required).
    ///
    /// Returns matched and unmatched files.
    pub fn apply(source_dir: &Path) -> Result<super::PackAssignmentResult, String> {
        let result = get_assignments(source_dir)?;

        let paths: HashMap<&str, PathBuf> = result
            .assigned
            .iter()
            .map(|a| (a.role.as_str(), source_dir.join(&a.file)))
            .collect();

        write_roles(&paths)?;
        Ok(result)
    }

    /// Restore the registry to a previously captured snapshot and broadcast
    /// `SPI_SETCURSORS` so the change takes effect immediately.
    pub fn revert(snapshot: &CursorSnapshot) -> Result<(), String> {
        let hkcu = RegKey::predef(HKEY_CURRENT_USER);
        let key = hkcu
            .open_subkey_with_flags("Control Panel\\Cursors", KEY_SET_VALUE)
            .map_err(|e| format!("Cannot open cursor registry key for writing: {e}"))?;

        for (name, value) in &snapshot.values {
            match value {
                // Key was present with a path — write it back.
                Some(v) => key
                    .set_value(name.as_str(), v)
                    .map_err(|e| format!("Cannot restore registry value {name}: {e}"))?,
                // Key was absent — delete it so Windows uses its built-in default.
                // Ignore errors: the key may already be gone.
                None => { let _ = key.delete_value(name.as_str()); }
            }
        }

        // Restore the active scheme name (the (Default) value).
        match &snapshot.scheme {
            Some(s) => { let _ = key.set_value("", s); }
            None    => { let _ = key.delete_value(""); }
        }

        call_spi_set_cursors()
    }

    fn call_spi_set_cursors() -> Result<(), String> {
        unsafe {
            // SPIF_SENDCHANGE broadcasts WM_SETTINGCHANGE so apps update live.
            //
            // No SPIF_UPDATEINIFILE: on Windows 10/11 SPI_SETCURSORS always
            // returns FALSE when that flag is set (the legacy win.ini flush is
            // gone) and leaves a stale GetLastError behind, which the windows
            // crate then reports as a bogus failure. The registry write above is
            // the persistence — win.ini is not involved — so the flag buys
            // nothing. Verified: flags 1 and 3 return FALSE, 0 and 2 return TRUE.
            SystemParametersInfoW(SPI_SETCURSORS, 0, None, SPIF_SENDCHANGE)
                .map_err(|e| format!("SystemParametersInfoW failed: {e}"))?;
        }
        Ok(())
    }

    /// The system pointer switches Settings exposes. Each is a plain on/off
    /// that Windows already stores, so none of them need any state of ours.
    ///
    /// The final flag is the one that bites: the modern actions carry their
    /// value *in* `pvParam`, while the older `SPI_SETSNAPTODEFBUTTON` carries
    /// it in `uiParam`. Verified on Win11 — passing it the other way is
    /// accepted and silently ignored, leaving the setting unchanged.
    #[allow(clippy::type_complexity)]
    const POINTER_FLAGS: &[(
        &str,
        SYSTEM_PARAMETERS_INFO_ACTION,
        SYSTEM_PARAMETERS_INFO_ACTION,
        bool,
    )] = &[
        ("shadow", SPI_GETCURSORSHADOW, SPI_SETCURSORSHADOW, false),
        ("sonar", SPI_GETMOUSESONAR, SPI_SETMOUSESONAR, false),
        ("vanish", SPI_GETMOUSEVANISH, SPI_SETMOUSEVANISH, false),
        (
            "snap_to_default",
            SPI_GETSNAPTODEFBUTTON,
            SPI_SETSNAPTODEFBUTTON,
            true,
        ),
    ];

    fn flag_entry(
        name: &str,
    ) -> Result<
        (
            SYSTEM_PARAMETERS_INFO_ACTION,
            SYSTEM_PARAMETERS_INFO_ACTION,
            bool,
        ),
        String,
    > {
        POINTER_FLAGS
            .iter()
            .find(|(n, ..)| *n == name)
            .map(|(_, get, set, in_uiparam)| (*get, *set, *in_uiparam))
            .ok_or_else(|| format!("Unknown pointer setting: {name}"))
    }

    /// Read one switch by name.
    pub fn get_flag(name: &str) -> Result<bool, String> {
        let (get, ..) = flag_entry(name)?;
        // SPI writes a 4-byte BOOL here.
        let mut enabled: i32 = 0;
        unsafe {
            SystemParametersInfoW(
                get,
                0,
                Some(&mut enabled as *mut _ as *mut core::ffi::c_void),
                Default::default(),
            )
            .map_err(|e| format!("Cannot read {name}: {e}"))?;
        }
        Ok(enabled != 0)
    }

    /// Every switch, for the Settings view's single round trip.
    pub fn get_all_flags() -> HashMap<String, bool> {
        POINTER_FLAGS
            .iter()
            .filter_map(|(name, ..)| get_flag(name).ok().map(|v| (name.to_string(), v)))
            .collect()
    }

    pub fn set_flag(name: &str, enabled: bool) -> Result<(), String> {
        let (_, set, in_uiparam) = flag_entry(name)?;
        let (uiparam, pvparam) = if in_uiparam {
            (enabled as u32, None)
        } else {
            (0, Some(enabled as usize as *mut core::ffi::c_void))
        };
        unsafe {
            // SPIF_UPDATEINIFILE is correct here (unlike SPI_SETCURSORS): these
            // live in HKCU\Control Panel\Desktop's UserPreferencesMask, and
            // without the flag they are lost at logoff.
            SystemParametersInfoW(
                set,
                uiparam,
                pvparam,
                SPIF_UPDATEINIFILE | SPIF_SENDCHANGE,
            )
            .map_err(|e| format!("Cannot set {name}: {e}"))?;
        }
        Ok(())
    }
}

// ── Commands ──────────────────────────────────────────────────────────────────

#[tauri::command]
async fn get_cursor_thumbnail(
    app: tauri::AppHandle,
    pack_id: String,
    cursor_name: String,
) -> Result<String, String> {
    let base = packs_dir(&app)?;
    let path = if cursor_name.is_empty() {
        let pack_dir = pack_dir_in(&base, &pack_id)?;
        find_first_cursor(&pack_dir)?
    } else {
        pack_file_in(&base, &pack_id, &cursor_name)?
    };
    cursor_path_to_b64(&path)
}

#[tauri::command]
async fn get_pack_thumbnails(
    app: tauri::AppHandle,
    pack_id: String,
    limit: usize,
) -> Result<Vec<String>, String> {
    let base = packs_dir(&app)?;
    let pack_dir = pack_dir_in(&base, &pack_id)?;

    let mut files: Vec<PathBuf> = fs::read_dir(&pack_dir)
        .map_err(|e| e.to_string())?
        .flatten()
        .map(|e| e.path())
        .filter(|p| {
            p.is_file()
                && p.extension()
                    .and_then(|x| x.to_str())
                    .map(|x| matches!(x.to_ascii_lowercase().as_str(), "cur" | "ani"))
                    .unwrap_or(false)
        })
        .collect();

    files.sort_by_key(|p| p.file_name().map(|n| n.to_os_string()));

    Ok(files
        .into_iter()
        .take(limit)
        .filter_map(|p| cursor_path_to_b64(&p).ok())
        .collect())
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
fn import_pack(app: tauri::AppHandle, source_path: String) -> Result<Vec<PackMeta>, String> {
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

        let result = import_variants(&app, &temp_dir, raw_name);

        // Always clean up temp, even on error
        let _ = fs::remove_dir_all(&temp_dir);

        result
    } else if source.is_dir() {
        let raw_name = source
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown-pack");

        // A chosen folder gets the same treatment: it may be the download's
        // top level rather than a pack, and picking one is no easier by hand.
        import_variants(&app, &source, raw_name)
    } else {
        Err("Source must be a .zip file or a folder".into())
    }
}

/// Import every pack found under `root`, naming them apart when there is more
/// than one. Fails only if nothing at all could be imported — one unreadable
/// variant should not cost the user the others.
fn import_variants(
    app: &tauri::AppHandle,
    root: &Path,
    raw_name: &str,
) -> Result<Vec<PackMeta>, String> {
    let variants = find_pack_variants(root);
    let single = variants.len() == 1;

    let mut imported = Vec::new();
    let mut errors = Vec::new();

    for dir in variants {
        let folder = dir.file_name().and_then(|n| n.to_str());
        let grouped = match (single, folder) {
            (false, Some(folder)) if dir != root => Some(folder.to_string()),
            _ => None,
        };

        // The id must stay unique per variant, so it keeps the download prefix
        // even though the card will show only the variant's own name.
        let id_source = match &grouped {
            Some(folder) => format!("{} - {folder}", tidy_download_name(raw_name)),
            None => raw_name.to_string(),
        };
        let group = grouped
            .clone()
            .map(|folder| (tidy_download_name(raw_name), folder));

        match do_import(app, &dir, &id_source, group) {
            Ok(meta) => imported.push(meta),
            Err(e) => errors.push(e),
        }
    }

    if imported.is_empty() {
        return Err(if errors.is_empty() {
            "No cursor packs found in that download".into()
        } else {
            errors.join("; ")
        });
    }
    Ok(imported)
}

#[tauri::command]
fn delete_pack(app: tauri::AppHandle, pack_id: String) -> Result<(), String> {
    let base = packs_dir(&app)?;
    let pack_dir = pack_dir_in(&base, &pack_id)?;
    if !pack_dir.exists() {
        return Err("Pack not found".into());
    }

    fs::remove_dir_all(&pack_dir).map_err(|e| e.to_string())
}

#[tauri::command]
async fn list_pack_cursors(
    app: tauri::AppHandle,
    pack_id: String,
) -> Result<Vec<CursorEntry>, String> {
    let base = packs_dir(&app)?;
    let pack_dir = pack_dir_in(&base, &pack_id)?;

    let mut files: Vec<PathBuf> = fs::read_dir(&pack_dir)
        .map_err(|e| e.to_string())?
        .flatten()
        .map(|e| e.path())
        .filter(|p| {
            p.is_file()
                && p.extension()
                    .and_then(|x| x.to_str())
                    .map(|x| matches!(x.to_ascii_lowercase().as_str(), "cur" | "ani"))
                    .unwrap_or(false)
        })
        .collect();

    files.sort_by_key(|p| p.file_name().map(|n| n.to_os_string()));

    let entries = files
        .into_iter()
        .map(|p| {
            let name = p
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("")
                .to_string();
            let kind = p
                .extension()
                .and_then(|e| e.to_str())
                .map(|e| e.to_ascii_lowercase())
                .unwrap_or_default();
            let thumbnail = cursor_path_to_b64(&p).unwrap_or_default();
            CursorEntry { name, kind, thumbnail }
        })
        .collect();

    Ok(entries)
}

#[tauri::command]
async fn parse_cur(
    app: tauri::AppHandle,
    pack_id: String,
    cursor_name: String,
) -> Result<CurInfo, String> {
    let base = packs_dir(&app)?;
    let path = pack_file_in(&base, &pack_id, &cursor_name)?;
    let data = fs::read(&path).map_err(|e| e.to_string())?;
    parse_cur_bytes(&data)
}

#[tauri::command]
async fn parse_ani(
    app: tauri::AppHandle,
    pack_id: String,
    cursor_name: String,
) -> Result<AniInfo, String> {
    let base = packs_dir(&app)?;
    let path = pack_file_in(&base, &pack_id, &cursor_name)?;
    let data = fs::read(&path).map_err(|e| e.to_string())?;
    parse_ani_bytes(&data)
}

/// Move the hotspot of a `.cur` or `.ani` cursor and write it back to the pack.
///
/// `x`/`y` are pixel coordinates in the `ref_w`x`ref_h` frame the user clicked;
/// every image inside the file is rescaled to match.
#[tauri::command]
async fn set_hotspot(
    app: tauri::AppHandle,
    pack_id: String,
    cursor_name: String,
    x: u32,
    y: u32,
    ref_w: u32,
    ref_h: u32,
) -> Result<(), String> {
    let base = packs_dir(&app)?;
    let path = pack_file_in(&base, &pack_id, &cursor_name)?;

    let mut data = fs::read(&path).map_err(|e| e.to_string())?;

    let ranges = ani_icon_ranges(&data);
    if ranges.is_empty() {
        set_cur_hotspots(&mut data, x, y, ref_w, ref_h);
    } else {
        for (start, end) in ranges {
            set_cur_hotspots(&mut data[start..end], x, y, ref_w, ref_h);
        }
    }

    // Write a sibling temp file then rename over the original, so a crash
    // mid-write cannot leave a truncated cursor in the pack.
    let tmp = path.with_extension("pawpack-tmp");
    fs::write(&tmp, &data).map_err(|e| e.to_string())?;
    // A rename over a cursor Windows currently has loaded fails with a sharing
    // violation. Clean up, or every retry leaves another orphan in the pack.
    if let Err(e) = fs::rename(&tmp, &path) {
        let _ = fs::remove_file(&tmp);
        return Err(e.to_string());
    }
    Ok(())
}

/// Report which cursor files map to which system roles, auto-detection and
/// saved overrides combined, without touching the registry.
#[tauri::command]
fn get_pack_assignments(app: tauri::AppHandle, pack_id: String) -> Result<PackAssignmentResult, String> {
    #[cfg(target_os = "windows")]
    {
        let base = packs_dir(&app)?;
        let pack_dir = pack_dir_in(&base, &pack_id)?;
        if !pack_dir.is_dir() {
            return Err("Pack not found".into());
        }
        windows_cursor::get_assignments(&pack_dir)
    }
    #[cfg(not(target_os = "windows"))]
    Err("get_pack_assignments is only supported on Windows".into())
}

/// Persist a manual role→file override for a pack and return the updated assignments.
///
/// Pass an empty `file` to explicitly clear a role (it will receive no cursor
/// even if auto-detection would otherwise assign one).
#[tauri::command]
fn set_cursor_override(
    app: tauri::AppHandle,
    pack_id: String,
    role: String,
    file: String,
) -> Result<PackAssignmentResult, String> {
    let base = packs_dir(&app)?;
    let pack_dir = pack_dir_in(&base, &pack_id)?;
    if !pack_dir.is_dir() {
        return Err("Pack not found".into());
    }

    let mut overrides = load_overrides(&pack_dir);
    if file.is_empty() {
        // Explicitly clear the role.
        overrides.insert(role, None);
    } else {
        // A file may back several roles at once, so claiming it here leaves any
        // other role already pointing at it untouched.
        overrides.insert(role, Some(file));
    }
    save_overrides(&pack_dir, &overrides)?;

    #[cfg(target_os = "windows")]
    return windows_cursor::get_assignments(&pack_dir);
    #[cfg(not(target_os = "windows"))]
    Err("set_cursor_override is only supported on Windows".into())
}

/// Read every system pointer switch in one call.
#[tauri::command]
async fn get_pointer_flags() -> Result<HashMap<String, bool>, String> {
    #[cfg(target_os = "windows")]
    return Ok(windows_cursor::get_all_flags());
    #[cfg(not(target_os = "windows"))]
    Err("get_pointer_flags is only supported on Windows".into())
}

/// Toggle one system pointer switch, returning what Windows holds afterwards.
#[tauri::command]
async fn set_pointer_flag(name: String, enabled: bool) -> Result<bool, String> {
    #[cfg(target_os = "windows")]
    {
        windows_cursor::set_flag(&name, enabled)?;
        // Report the value Windows actually holds, not the one we asked for.
        return windows_cursor::get_flag(&name);
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = (name, enabled);
        Err("set_pointer_flag is only supported on Windows".into())
    }
}

/// Resolve the mix into absolute paths, plus the roles that will be cleared.
///
/// Errors on an empty mix rather than clearing all 17 roles, which is what a
/// half-built mix would otherwise do the first time Apply is pressed.
#[cfg(target_os = "windows")]
fn mix_paths_for_apply(
    packs_base: &Path,
) -> Result<(HashMap<String, PathBuf>, Vec<String>), String> {
    let mix = read_mix(packs_base);
    let known: HashMap<String, PathBuf> = mix
        .roles
        .iter()
        .filter(|e| windows_cursor::CURSOR_REG_NAMES.contains(&e.role.as_str()))
        .filter_map(|e| {
            pack_file_in(packs_base, &e.pack, &e.file)
                .ok()
                .map(|path| (e.role.clone(), path))
        })
        .collect();
    if known.is_empty() {
        return Err("Mix is empty — assign at least one cursor before applying".into());
    }

    let paths = known;

    let cleared: Vec<String> = windows_cursor::CURSOR_REG_NAMES
        .iter()
        .filter(|r| !paths.contains_key(**r))
        .map(|r| r.to_string())
        .collect();

    Ok((paths, cleared))
}

/// Capture the user's own cursors once, before the first apply of any kind.
#[cfg(target_os = "windows")]
fn ensure_snapshot(base: &Path) -> Result<(), String> {
    let snapshot_path = windows_cursor::snapshot_path(base);
    if snapshot_path.exists() {
        return Ok(());
    }

    let captured = windows_cursor::snapshot()?;
    let snapshot = if captured.is_pack_owned(base) {
        windows_cursor::CursorSnapshot::windows_default()
    } else {
        captured
    };
    let json = serde_json::to_string_pretty(&snapshot).map_err(|e| e.to_string())?;
    fs::write(&snapshot_path, &json).map_err(|e| e.to_string())
}

#[tauri::command]
fn apply_pack(app: tauri::AppHandle, pack_id: String) -> Result<PackAssignmentResult, String> {
    #[cfg(target_os = "windows")]
    {
        let base = packs_dir(&app)?;
        let pack_dir = pack_dir_in(&base, &pack_id)?;
        if !pack_dir.is_dir() {
            return Err("Pack not found".into());
        }

        // Capture the user's own cursors once, on the first apply, and keep it
        // for the lifetime of the install. Snapshotting on every apply meant a
        // second apply recorded the *already applied* pack as the original, so
        // reverting restored a pack instead of the user's real cursors.
        ensure_snapshot(&base)?;

        windows_cursor::apply(&pack_dir)
    }

    #[cfg(not(target_os = "windows"))]
    Err("apply_pack is only supported on Windows".into())
}

/// Report the current mix, splitting entries whose files are gone.
#[tauri::command]
fn get_mix(app: tauri::AppHandle) -> Result<MixResult, String> {
    let base = packs_dir(&app)?;
    Ok(read_mix(&base))
}

/// Set or clear one role in the mix. An empty `pack_id` clears it.
#[tauri::command]
fn set_mix_role(
    app: tauri::AppHandle,
    role: String,
    pack_id: String,
    file: String,
) -> Result<MixResult, String> {
    let base = packs_dir(&app)?;

    if !pack_id.is_empty() {
        // Validates `file` too — it is persisted into the mix and later
        // resolved into a registry path, so it must not escape the pack.
        if !pack_file_in(&base, &pack_id, &file)?.is_file() {
            return Err("Cursor file not found".into());
        }
    }

    write_mix_role(&base, &role, &pack_id, &file)
}

/// Result of applying a mix.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ApplyMixResult {
    pub written: usize,
    /// Roles reset to Windows defaults because the mix does not fill them.
    pub cleared: Vec<String>,
}

/// Write the mix to the registry, clearing every role it does not fill.
#[tauri::command]
fn apply_mix(app: tauri::AppHandle) -> Result<ApplyMixResult, String> {
    #[cfg(target_os = "windows")]
    {
        let base = packs_dir(&app)?;
        let (paths, cleared) = mix_paths_for_apply(&base)?;

        ensure_snapshot(&base)?;

        let borrowed: HashMap<&str, PathBuf> =
            paths.iter().map(|(r, p)| (r.as_str(), p.clone())).collect();
        windows_cursor::write_roles(&borrowed)?;

        Ok(ApplyMixResult { written: paths.len(), cleared })
    }

    #[cfg(not(target_os = "windows"))]
    {
        let _ = app;
        Err("apply_mix is only supported on Windows".into())
    }
}

/// Restore the cursors the user had before any pack was applied and broadcast
/// `SPI_SETCURSORS` so the revert takes effect immediately.
///
/// The snapshot is per-install, written once by the first `apply_pack`, so this
/// always returns to the user's own cursors rather than to a previous pack.
#[tauri::command]
fn revert_cursors(app: tauri::AppHandle) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        let base = packs_dir(&app)?;

        // One snapshot for the whole install, not one per pack: it records the
        // cursors the user had before PawPack ever touched them.
        let revert_path = windows_cursor::snapshot_path(&base);
        if !revert_path.exists() {
            return Err("No revert snapshot found — apply a pack first".into());
        }

        let json = fs::read_to_string(&revert_path).map_err(|e| e.to_string())?;
        let snapshot: windows_cursor::CursorSnapshot =
            serde_json::from_str(&json).map_err(|e| e.to_string())?;

        windows_cursor::revert(&snapshot)
    }

    #[cfg(not(target_os = "windows"))]
    {
        let _ = app;
        Err("revert_cursors is only supported on Windows".into())
    }
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
            set_hotspot,
            get_cursor_thumbnail,
            get_pack_thumbnails,
            list_pack_cursors,
            get_pack_assignments,
            set_cursor_override,
            apply_pack,
            revert_cursors,
            get_mix,
            set_mix_role,
            apply_mix,
            get_pointer_flags,
            set_pointer_flag
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

    /// A RIFF ACON blob with `n` identical 1x1 frames and a display rate of 4.
    fn make_ani(n: u32) -> Vec<u8> {
        let cur_size = make_cur(0, 0).len() as u32;

        let mut list_content = Vec::new();
        list_content.extend_from_slice(b"fram");
        for i in 0..n {
            // Vary the XOR pixel's green channel (offset 63, see make_cur's
            // BGRA layout) so frames differ; identical frames could not
            // reveal reordering or duplication bugs.
            let mut cur_data = make_cur(0, 0);
            cur_data[63] = i as u8;

            list_content.extend_from_slice(b"icon");
            list_content.extend_from_slice(&cur_size.to_le_bytes());
            list_content.extend_from_slice(&cur_data);
            if cur_size % 2 == 1 {
                list_content.push(0);
            }
        }

        let mut anih_data = vec![0u8; 36];
        anih_data[0..4].copy_from_slice(&36u32.to_le_bytes());
        anih_data[4..8].copy_from_slice(&n.to_le_bytes()); // nFrames
        anih_data[28..32].copy_from_slice(&4u32.to_le_bytes()); // iDispRate

        let mut buf = Vec::new();
        buf.extend_from_slice(b"RIFF");
        buf.extend_from_slice(&[0u8; 4]); // size placeholder
        buf.extend_from_slice(b"ACON");
        buf.extend_from_slice(b"anih");
        buf.extend_from_slice(&36u32.to_le_bytes());
        buf.extend_from_slice(&anih_data);
        buf.extend_from_slice(b"LIST");
        buf.extend_from_slice(&(list_content.len() as u32).to_le_bytes());
        buf.extend_from_slice(&list_content);

        let riff_size = (buf.len() - 8) as u32;
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

    #[cfg(target_os = "windows")]
    /// A cursor directory with two sizes (32 and 64 px) and no image data —
    /// `set_cur_hotspots` only ever reads the entry table.
    fn make_multisize_dir() -> Vec<u8> {
        let mut buf = vec![0u8, 0, 2, 0, 2, 0]; // reserved, type=2, count=2
        for dim in [32u8, 64] {
            buf.extend_from_slice(&[dim, dim, 0, 0]); // w, h, colors, reserved
            buf.extend_from_slice(&[0u8; 4]);         // hotspot x, y
            buf.extend_from_slice(&[0u8; 8]);         // size, offset
        }
        buf
    }

    fn hotspot_at(data: &[u8], i: usize) -> (u16, u16) {
        let base = 6 + i * 16;
        (
            u16::from_le_bytes([data[base + 4], data[base + 5]]),
            u16::from_le_bytes([data[base + 6], data[base + 7]]),
        )
    }

    #[test]
    fn hotspot_scales_to_each_image_size() {
        let mut data = make_multisize_dir();
        // Click at (8, 4) on a 32x32 view: a quarter across, an eighth down.
        set_cur_hotspots(&mut data, 8, 4, 32, 32);
        assert_eq!(hotspot_at(&data, 0), (8, 4), "32 px entry is 1:1");
        assert_eq!(hotspot_at(&data, 1), (16, 8), "64 px entry doubles");
    }

    #[test]
    fn hotspot_stays_inside_the_image() {
        let mut data = make_multisize_dir();
        // Bottom-right corner of a 32x32 view — must not land one pixel past.
        set_cur_hotspots(&mut data, 32, 32, 32, 32);
        assert_eq!(hotspot_at(&data, 0), (31, 31));
        assert_eq!(hotspot_at(&data, 1), (63, 63));
    }

    #[test]
    fn icon_typed_directories_are_left_alone() {
        // Same table, but idType 1 (icon). Bytes 4..8 are wPlanes/wBitCount
        // there, not a hotspot, so neither side may touch them.
        let mut data = make_multisize_dir();
        data[2] = 1;
        for i in 0..2 {
            let base = 6 + i * 16;
            data[base + 4..base + 8].copy_from_slice(&[1, 0, 32, 0]); // 1 plane, 32 bpp
        }
        let before = data.clone();

        set_cur_hotspots(&mut data, 8, 4, 32, 32);
        assert_eq!(data, before, "an icon directory must survive untouched");
    }

    #[test]
    fn hotspot_patches_every_ani_frame_without_resizing_it() {
        let mut ani = make_ani(3);
        let before = ani.len();
        let ranges = ani_icon_ranges(&ani);
        assert_eq!(ranges.len(), 3, "one icon chunk per frame");

        // make_ani's frames are 1x1; widen the entry table so scaling has room.
        for &(start, _) in &ranges {
            ani[start + 6] = 32;
            ani[start + 7] = 32;
        }
        for &(start, end) in &ranges {
            set_cur_hotspots(&mut ani[start..end], 8, 4, 32, 32);
        }

        assert_eq!(ani.len(), before, "in-place patch keeps RIFF sizes valid");
        for &(start, end) in &ranges {
            assert_eq!(hotspot_at(&ani[start..end], 0), (8, 4));
        }
        // The parser still walks the file, so the chunk table survived.
        assert_eq!(parse_ani_bytes(&ani).unwrap().frame_count, 3);
    }

    /// Layouts taken from two real VSTHEMES downloads. The archives themselves
    /// are third-party art and far too large to vendor, so the shapes are
    /// rebuilt here — detection only ever looks at names and extensions, so the
    /// files can be empty.
    mod download_names {
        use crate::tidy_download_name;

        #[test]
        fn site_markers_and_hashes_are_stripped() {
            assert_eq!(tidy_download_name("moga_992bd4d17b_VSTHEMES-ORG"), "moga");
            assert_eq!(tidy_download_name("miyabi-zzz_6qcd_VSTHEMES-ORG"), "miyabi-zzz");
            assert_eq!(
                tidy_download_name("windows-11-cursors-concept-v2_6qcd_VSTHEMES-ORG"),
                "windows-11-cursors-concept-v2",
            );
        }

        #[test]
        fn ordinary_names_survive() {
            assert_eq!(tidy_download_name("Bog cursor pack"), "Bog cursor pack");
            assert_eq!(tidy_download_name("brush-buddy-cursor"), "brush-buddy-cursor");
            // "v2" is short enough not to look like a hash.
            assert_eq!(tidy_download_name("my-pack-v2"), "my-pack-v2");
        }

        #[test]
        fn stripping_never_empties_the_name() {
            assert_eq!(tidy_download_name("VSTHEMES-ORG"), "VSTHEMES-ORG");
            assert_eq!(tidy_download_name("deadbeef"), "deadbeef");
        }
    }

    mod pack_variants {
        use crate::{detect_pack, find_pack_variants};
        use std::fs;
        use std::path::{Path, PathBuf};

        struct TempTree(PathBuf);

        impl Drop for TempTree {
            fn drop(&mut self) {
                let _ = fs::remove_dir_all(&self.0);
            }
        }

        fn tree(name: &str, entries: &[&str]) -> TempTree {
            let root = std::env::temp_dir().join(format!("pawpack-variants-{name}"));
            let _ = fs::remove_dir_all(&root);
            for rel in entries {
                let path = root.join(rel);
                if rel.ends_with('/') {
                    fs::create_dir_all(&path).unwrap();
                } else {
                    fs::create_dir_all(path.parent().unwrap()).unwrap();
                    fs::write(&path, b"").unwrap();
                }
            }
            TempTree(root)
        }

        fn names(root: &Path, found: &[PathBuf]) -> Vec<String> {
            found
                .iter()
                .map(|p| {
                    p.strip_prefix(root)
                        .unwrap_or(p)
                        .to_string_lossy()
                        .replace('\\', "/")
                })
                .collect()
        }

        /// Two sibling variants plus GIF previews, PDFs and macOS tooling.
        #[test]
        fn miyabi_yields_both_animated_and_static() {
            let t = tree("miyabi", &[
                "Cursors/Normal.ani", "Cursors/Busy.ani", "Cursors/install.inf",
                "Static/Normal.cur", "Static/Busy.cur", "Static/install.inf",
                "Gifs/1.gif", "Gifs/link.gif",
                "MacOS/Mousecape_1813.zip",
                "EN.pdf", "RU.pdf", "source.url",
            ]);
            let found = find_pack_variants(&t.0);
            assert_eq!(names(&t.0, &found), ["Cursors", "Static"]);
            for dir in &found {
                assert_eq!(detect_pack(dir).0, "windows");
            }
            // Previously the two candidates failed the "exactly one" test and
            // the archive root was imported instead: no cursors, 6 MB of junk.
            assert_ne!(found[0], t.0);
        }

        /// Three colour variants two levels down, plus a full Xcursor theme.
        #[test]
        fn moga_yields_three_windows_variants_and_the_linux_theme() {
            let t = tree("moga", &[
                "Windows/Moga/Normal Select.cur", "Windows/Moga/Busy.ani", "Windows/Moga/Install.inf",
                "Windows/Moga Black/Normal Select.cur", "Windows/Moga Black/Busy.ani",
                "Windows/Moga White/Normal Select.cur", "Windows/Moga White/Busy.ani",
                "Linux/index.theme", "Linux/cursors/left_ptr", "Linux/cursors/zoom-in",
                "EN.pdf", "source.url",
            ]);
            let found = find_pack_variants(&t.0);
            assert_eq!(
                names(&t.0, &found),
                ["Linux", "Windows/Moga", "Windows/Moga Black", "Windows/Moga White"],
            );
            assert_eq!(detect_pack(&t.0.join("Linux")).0, "linux");
            assert_eq!(detect_pack(&t.0.join("Windows/Moga Black")).0, "windows");
        }

        /// The ordinary case must not regress: cursors at the top level are one
        /// pack, and its subfolders are contents rather than further variants.
        #[test]
        fn a_plain_pack_is_a_single_variant() {
            let t = tree("plain", &["Arrow.cur", "Busy.ani", "extras/readme.txt"]);
            assert_eq!(find_pack_variants(&t.0), vec![t.0.clone()]);
        }

        /// A single wrapping folder, the shape the old code did handle.
        #[test]
        fn a_wrapped_pack_unwraps_to_the_inner_folder() {
            let t = tree("wrapped", &["Some Pack/Arrow.cur", "Some Pack/Busy.ani"]);
            assert_eq!(names(&t.0, &find_pack_variants(&t.0)), ["Some Pack"]);
        }

        /// Nothing recognisable still imports as the root rather than vanishing.
        #[test]
        fn an_unrecognised_download_falls_back_to_the_root() {
            let t = tree("empty", &["EN.pdf", "Gifs/1.gif"]);
            assert_eq!(find_pack_variants(&t.0), vec![t.0.clone()]);
        }
    }

    mod path_guard {
        use crate::{is_safe_segment, pack_dir_in, pack_file_in};
        use std::path::Path;

        #[test]
        fn traversal_is_refused() {
            let base = Path::new("/packs");
            // `starts_with` used to accept every one of these, because it
            // compares components and "/packs/.." does start with "/packs".
            for bad in ["..", "../..", "../evil", "a/b", ".", ""] {
                assert!(!is_safe_segment(bad), "{bad:?} should be refused");
                assert!(pack_dir_in(base, bad).is_err(), "pack id {bad:?}");
                assert!(pack_file_in(base, "ok", bad).is_err(), "file {bad:?}");
            }
        }

        #[cfg(target_os = "windows")]
        #[test]
        fn windows_separators_and_prefixes_are_refused() {
            for bad in [r"a\b", "C:", r"C:\evil", r"..\evil"] {
                assert!(!is_safe_segment(bad), "{bad:?} should be refused");
            }
        }

        #[test]
        fn ordinary_names_still_resolve() {
            let base = Path::new("/packs");
            assert_eq!(pack_dir_in(base, "bog-cursor-pack").unwrap(), base.join("bog-cursor-pack"));
            assert_eq!(
                pack_file_in(base, "bog-cursor-pack", "Diag 1.cur").unwrap(),
                base.join("bog-cursor-pack").join("Diag 1.cur"),
            );
            // A doubled extension is an ordinary name, not a traversal.
            assert!(pack_file_in(base, "p", "Rocky_Idle_Curso.ani.ani").is_ok());
        }
    }

    mod role_match {
        use crate::windows_cursor::role_match;

        fn role(stem: &str) -> Option<&'static str> {
            role_match(stem).map(|(r, _)| r)
        }

        #[test]
        fn plain_stems_keep_matching() {
            assert_eq!(role("Arrow"), Some("Arrow"));
            assert_eq!(role("Busy"), Some("Wait"));
            assert_eq!(role("Diag 1"), Some("SizeNWSE"));
            assert_eq!(role("Working in bg"), Some("AppStarting"));
            assert_eq!(role("Loaction"), Some("Pin"));
        }

        #[test]
        fn decorated_names_now_match() {
            assert_eq!(role("Brushbuddy-standard-cursor-static"), Some("Arrow"));
            assert_eq!(role("Animated Loading Brushbuddy"), Some("Wait"));
            assert_eq!(role("Animated Classic cursor Brushbuddy"), Some("Arrow"));
        }

        #[test]
        fn specific_keyword_beats_generic_one() {
            // Contains both "link" (Hand) and "pointer" (Arrow).
            assert_eq!(role("Brushbuddy-link-pointer-static"), Some("Hand"));
            assert_eq!(role("Animated Link pointer Brushbuddy"), Some("Hand"));
            // Windows' own names qualify a trailing "Select".
            assert_eq!(role("Normal Select"), Some("Arrow"));
            assert_eq!(role("Help Select"), Some("Help"));
            assert_eq!(role("Precision Select"), Some("Crosshair"));
            assert_eq!(role("Alternate Select"), Some("UpArrow"));
            assert_eq!(role("Link Select"), Some("Hand"));
        }

        #[test]
        fn matches_whole_words_only() {
            // "normal" contains "no", "sweep" contains "we" — neither may match
            // the short role keyword hiding inside them.
            assert_eq!(role("Normal"), Some("Arrow"));
            assert_eq!(role("Sweep"), None);
            assert_eq!(role("Crosshair"), Some("Crosshair"));
            assert_eq!(role("Uparrow"), Some("UpArrow"));
        }

        #[test]
        fn unrelated_names_stay_unmatched() {
            assert_eq!(role("Tau Ceti"), None);
            assert_eq!(role("Capsule_Open"), None);
        }

        #[test]
        fn resting_and_pointing_pointers_match() {
            // A pack shipping both means the resting pointer and the link
            // pointer; "idle" is as common a name for the default as "normal".
            assert_eq!(role("Rocky_Idle"), Some("Arrow"));
            assert_eq!(role("Rocky_Pointing_Cursor"), Some("Hand"));
            // Whole-word matching means "point" alone never reaches "pointing",
            // which is why it needs its own entry.
            assert_eq!(role("Pointing hand"), Some("Hand"));
            // A doubled extension leaves ".ani" as just another word in the stem.
            assert_eq!(role("Rocky_Idle_Curso.ani"), Some("Arrow"));
        }

        #[test]
        fn legacy_snapshot_migration() {
            use crate::windows_cursor::snapshot_path;
            use std::fs;

            let base = std::env::temp_dir().join(format!(
                "pawpack-migrate-{}",
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ));
            let genuine = base.join("pack-a");
            let poisoned = base.join("pack-b");
            fs::create_dir_all(&genuine).unwrap();
            fs::create_dir_all(&poisoned).unwrap();

            // Points at a real Windows cursor — worth keeping.
            fs::write(
                genuine.join("revert.json"),
                r#"{"values":{"Arrow":"C:\\Windows\\Cursors\\aero_arrow.cur"},"scheme":"Windows Default"}"#,
            )
            .unwrap();
            // Points back into the packs directory — restoring it would only
            // reinstate a pack, so it must not survive.
            let inside = poisoned.join("Arrow.cur").to_string_lossy().replace('\\', "\\\\");
            fs::write(
                poisoned.join("revert.json"),
                format!(r#"{{"values":{{"Arrow":"{inside}"}},"scheme":"Bog"}}"#),
            )
            .unwrap();

            let shared = snapshot_path(&base);

            assert_eq!(shared, base.join("revert.json"));
            assert!(shared.is_file(), "genuine snapshot should be promoted");
            assert!(
                fs::read_to_string(&shared).unwrap().contains("aero_arrow.cur"),
                "promoted snapshot should be the genuine one"
            );
            assert!(!genuine.join("revert.json").exists(), "promoted file should move, not copy");
            assert!(!poisoned.join("revert.json").exists(), "pack-owned snapshot should be deleted");

            // Running again is a no-op and must not clobber the shared file.
            let before = fs::read_to_string(&shared).unwrap();
            snapshot_path(&base);
            assert_eq!(fs::read_to_string(&shared).unwrap(), before);

            fs::remove_dir_all(&base).ok();
        }

        #[test]
        fn rank_orders_specific_above_generic() {
            let (_, link) = role_match("Link pointer").unwrap();
            let (_, plain) = role_match("Pointer").unwrap();
            assert!(link < plain, "specific keyword must outrank generic one");
        }
    }

    mod mix {
        use crate::{read_mix, write_mix_role};
        use std::fs;
        use std::path::PathBuf;

        /// Fresh packs dir with one real pack holding one real cursor file.
        fn scratch(tag: &str) -> PathBuf {
            let base = std::env::temp_dir().join(format!(
                "pawpack-mix-{tag}-{}",
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ));
            fs::create_dir_all(base.join("pack-a")).unwrap();
            fs::write(base.join("pack-a").join("Arrow.cur"), b"not a real cursor").unwrap();
            base
        }

        #[test]
        fn round_trips_through_disk() {
            let base = scratch("round");
            write_mix_role(&base, "Arrow", "pack-a", "Arrow.cur").unwrap();

            let mix = read_mix(&base);
            assert_eq!(mix.roles.len(), 1);
            assert_eq!(mix.roles[0].role, "Arrow");
            assert_eq!(mix.roles[0].pack, "pack-a");
            assert_eq!(mix.roles[0].file, "Arrow.cur");
            assert!(mix.stale.is_empty());

            fs::remove_dir_all(&base).ok();
        }

        #[test]
        fn empty_pack_id_clears_the_role() {
            let base = scratch("clear");
            write_mix_role(&base, "Arrow", "pack-a", "Arrow.cur").unwrap();
            let mix = write_mix_role(&base, "Arrow", "", "").unwrap();

            assert!(mix.roles.is_empty());
            assert!(mix.stale.is_empty());

            fs::remove_dir_all(&base).ok();
        }

        #[test]
        fn missing_pack_becomes_stale() {
            let base = scratch("nopack");
            write_mix_role(&base, "Arrow", "pack-a", "Arrow.cur").unwrap();
            fs::remove_dir_all(base.join("pack-a")).unwrap();

            let mix = read_mix(&base);
            assert!(mix.roles.is_empty(), "entry must not count as usable");
            assert_eq!(mix.stale.len(), 1);
            assert_eq!(mix.stale[0].pack, "pack-a");

            fs::remove_dir_all(&base).ok();
        }

        #[test]
        fn missing_file_in_present_pack_becomes_stale() {
            let base = scratch("nofile");
            write_mix_role(&base, "Arrow", "pack-a", "Gone.cur").unwrap();

            let mix = read_mix(&base);
            assert!(mix.roles.is_empty());
            assert_eq!(mix.stale.len(), 1);
            assert_eq!(mix.stale[0].file, "Gone.cur");

            fs::remove_dir_all(&base).ok();
        }

        #[test]
        fn stale_entries_are_not_erased_from_disk() {
            let base = scratch("keep");
            write_mix_role(&base, "Arrow", "pack-a", "Arrow.cur").unwrap();
            fs::rename(base.join("pack-a"), base.join("pack-moved")).unwrap();
            assert_eq!(read_mix(&base).stale.len(), 1);

            // Reimporting under the same id restores the entry.
            fs::rename(base.join("pack-moved"), base.join("pack-a")).unwrap();
            assert_eq!(read_mix(&base).roles.len(), 1);

            fs::remove_dir_all(&base).ok();
        }

        #[test]
        fn corrupt_file_reads_as_empty_mix() {
            let base = scratch("corrupt");
            fs::write(base.join("mix.json"), b"{ this is not json").unwrap();

            let mix = read_mix(&base);
            assert!(mix.roles.is_empty());
            assert!(mix.stale.is_empty());

            // And a write recovers the file rather than failing.
            write_mix_role(&base, "Hand", "pack-a", "Arrow.cur").unwrap();
            assert_eq!(read_mix(&base).roles.len(), 1);

            fs::remove_dir_all(&base).ok();
        }

        #[test]
        #[cfg(target_os = "windows")]
        fn empty_mix_is_refused_before_any_write() {
            let base = scratch("empty");
            assert!(
                crate::mix_paths_for_apply(&base).is_err(),
                "an empty mix must be refused, not applied as 17 deletions"
            );

            write_mix_role(&base, "Arrow", "pack-a", "Arrow.cur").unwrap();
            let (paths, cleared) = crate::mix_paths_for_apply(&base).unwrap();
            assert_eq!(paths.len(), 1);
            assert_eq!(paths.get("Arrow").unwrap(), &base.join("pack-a").join("Arrow.cur"));
            assert_eq!(cleared.len(), 16, "the other 16 roles reset to Windows defaults");
            assert!(!cleared.contains(&"Arrow".to_string()));

            fs::remove_dir_all(&base).ok();
        }

        #[test]
        #[cfg(target_os = "windows")]
        fn mix_spanning_two_packs_resolves_both() {
            let base = scratch("twopack");
            fs::create_dir_all(base.join("pack-b")).unwrap();
            fs::write(base.join("pack-b").join("Hand.cur"), b"not a real cursor").unwrap();

            write_mix_role(&base, "Arrow", "pack-a", "Arrow.cur").unwrap();
            write_mix_role(&base, "Hand", "pack-b", "Hand.cur").unwrap();

            let (paths, cleared) = crate::mix_paths_for_apply(&base).unwrap();
            assert_eq!(paths.len(), 2);
            assert_eq!(paths.get("Arrow").unwrap(), &base.join("pack-a").join("Arrow.cur"));
            assert_eq!(paths.get("Hand").unwrap(), &base.join("pack-b").join("Hand.cur"));
            assert_eq!(cleared.len(), 15);

            fs::remove_dir_all(&base).ok();
        }

        #[test]
        #[cfg(target_os = "windows")]
        fn unknown_roles_are_refused_not_applied_as_deletions() {
            let base = scratch("unknown");
            write_mix_role(&base, "NotARealRole", "pack-a", "Arrow.cur").unwrap();

            assert!(
                crate::mix_paths_for_apply(&base).is_err(),
                "a mix of only unknown roles must be refused, not applied as 17 deletions"
            );

            // A known role alongside an unknown one applies only the known one.
            write_mix_role(&base, "Arrow", "pack-a", "Arrow.cur").unwrap();
            let (paths, cleared) = crate::mix_paths_for_apply(&base).unwrap();
            assert_eq!(paths.len(), 1, "unknown role must not reach the registry write");
            assert!(paths.contains_key("Arrow"));
            assert_eq!(cleared.len(), 16);

            fs::remove_dir_all(&base).ok();
        }
    }

    mod apng {
        use crate::encode_animated_png;

        /// Index of a chunk's data, given its four-byte type.
        /// PNG chunk layout: [len: 4][type: 4][data: len][crc: 4].
        fn chunk_data(bytes: &[u8], kind: &[u8; 4]) -> Option<usize> {
            bytes.windows(4).position(|w| w == kind).map(|i| i + 4)
        }

        /// (frame_count, play_count) from the acTL chunk.
        fn actl(bytes: &[u8]) -> Option<(u32, u32)> {
            let d = chunk_data(bytes, b"acTL")?;
            let frames = u32::from_be_bytes(bytes[d..d + 4].try_into().ok()?);
            let plays = u32::from_be_bytes(bytes[d + 4..d + 8].try_into().ok()?);
            Some((frames, plays))
        }

        /// Every fcTL chunk's (delay_num, delay_den). fcTL data layout:
        /// seq(4) width(4) height(4) x(4) y(4) delay_num(2) delay_den(2) ...
        fn fctl_delays(bytes: &[u8]) -> Vec<(u16, u16)> {
            let mut out = Vec::new();
            for (i, w) in bytes.windows(4).enumerate() {
                if w == b"fcTL" {
                    let d = i + 4;
                    if bytes.len() >= d + 24 {
                        let num = u16::from_be_bytes([bytes[d + 20], bytes[d + 21]]);
                        let den = u16::from_be_bytes([bytes[d + 22], bytes[d + 23]]);
                        out.push((num, den));
                    }
                }
            }
            out
        }

        /// `n` solid frames of `w`x`h`, so tests need no real cursor data.
        fn frames(n: usize, w: u32, h: u32) -> Vec<(u32, u32, Vec<u8>)> {
            (0..n)
                .map(|i| (w, h, vec![(i * 10) as u8; (w * h * 4) as usize]))
                .collect()
        }

        #[test]
        fn writes_a_valid_png_signature_and_actl() {
            let out = encode_animated_png(&frames(3, 4, 4), &[5, 5, 5]).unwrap();
            assert_eq!(&out[..8], b"\x89PNG\r\n\x1a\n", "must still be a PNG");
            assert!(actl(&out).is_some(), "animated output must carry acTL");
        }

        #[test]
        fn actl_reports_frame_count_and_infinite_looping() {
            let out = encode_animated_png(&frames(3, 4, 4), &[5, 5, 5]).unwrap();
            assert_eq!(actl(&out).unwrap(), (3, 0), "3 frames, 0 = loop forever");
        }

        #[test]
        fn per_frame_delays_reach_the_encoded_output() {
            let out = encode_animated_png(&frames(3, 4, 4), &[3, 6, 9]).unwrap();
            assert_eq!(fctl_delays(&out), vec![(3, 60), (6, 60), (9, 60)]);
        }

        #[test]
        fn zero_delay_becomes_one_jiffy() {
            // A zero delay lets the viewer pick its own minimum, which makes
            // playback speed inconsistent between browsers.
            let out = encode_animated_png(&frames(2, 4, 4), &[0, 4]).unwrap();
            assert_eq!(fctl_delays(&out), vec![(1, 60), (4, 60)]);
        }

        #[test]
        fn canvas_is_the_max_of_each_dimension_independently() {
            // 32x16 beside 16x32 must give a 32x32 canvas.
            let mixed = vec![
                (32, 16, vec![0u8; 32 * 16 * 4]),
                (16, 32, vec![0u8; 16 * 32 * 4]),
            ];
            let out = encode_animated_png(&mixed, &[4, 4]).unwrap();
            // IHDR data starts right after the type: width(4) height(4).
            let d = chunk_data(&out, b"IHDR").unwrap();
            let width = u32::from_be_bytes(out[d..d + 4].try_into().unwrap());
            let height = u32::from_be_bytes(out[d + 4..d + 8].try_into().unwrap());
            assert_eq!((width, height), (32, 32));
        }

        #[test]
        fn shorter_delay_list_falls_back_to_the_last_delay() {
            // per_frame_rates can be shorter than the frame list; every frame
            // still needs a delay.
            let out = encode_animated_png(&frames(3, 4, 4), &[7]).unwrap();
            assert_eq!(fctl_delays(&out), vec![(7, 60), (7, 60), (7, 60)]);
        }

        #[test]
        fn cap_leaves_common_cursor_sizes_pixel_exact() {
            // 32x32 and 96x96 are the usual sizes and must not be resampled.
            let rgba = vec![0xAB; 32 * 32 * 4];
            let (w, h, out) = crate::cap_frame(32, 32, rgba.clone());
            assert_eq!((w, h), (32, 32));
            assert_eq!(out, rgba, "a frame within the cap must be untouched");

            let (w, h, _) = crate::cap_frame(96, 96, vec![0u8; 96 * 96 * 4]);
            assert_eq!((w, h), (96, 96));
        }

        #[test]
        fn cap_downscales_oversized_frames_keeping_aspect_ratio() {
            let (w, h, out) = crate::cap_frame(256, 256, vec![0u8; 256 * 256 * 4]);
            assert_eq!((w, h), (96, 96));
            assert_eq!(out.len(), 96 * 96 * 4);

            // 256x128 keeps its 2:1 shape.
            let (w, h, _) = crate::cap_frame(256, 128, vec![0u8; 256 * 128 * 4]);
            assert_eq!((w, h), (96, 48));
        }

        #[test]
        fn cap_passes_through_a_buffer_of_the_wrong_length() {
            // Never panic on malformed input; leave it for the encoder to reject.
            let short = vec![0u8; 10];
            let (w, h, out) = crate::cap_frame(256, 256, short.clone());
            assert_eq!((w, h), (256, 256));
            assert_eq!(out, short);
        }

        #[test]
        fn rejects_an_empty_frame_list() {
            assert!(encode_animated_png(&[], &[]).is_err());
        }

        #[test]
        fn refuses_a_canvas_that_would_overflow_the_allocation() {
            // Independent maxima: 65535x1 beside 1x65535 gives a canvas whose
            // byte count overflows u32. Must error, not panic or allocate 4 GB.
            let huge = vec![
                (65535, 1, vec![0u8; 65535 * 4]),
                (1, 65535, vec![0u8; 65535 * 4]),
            ];
            assert!(encode_animated_png(&huge, &[4, 4]).is_err());
        }

        #[test]
        fn a_smaller_frame_is_centred_not_corner_pinned() {
            // A 2x1 opaque frame into a 4x3 canvas lands at row 1, col 1.
            let out = crate::center_on_canvas(2, 1, &[0xAA; 8], 4, 3);
            assert_eq!(out.len(), 4 * 3 * 4);
            let lit: Vec<usize> = out.iter().enumerate()
                .filter(|(_, v)| **v != 0).map(|(i, _)| i).collect();
            assert_eq!(lit, (20..28).collect::<Vec<_>>());
        }

        use crate::cursor_path_to_b64;
        use base64::{engine::general_purpose::STANDARD, Engine};
        use std::fs;
        use std::path::PathBuf;

        /// A unique temp directory for one test.
        fn scratch_dir(tag: &str) -> PathBuf {
            let dir = std::env::temp_dir().join(format!(
                "pawpack-apng-{tag}-{}",
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ));
            fs::create_dir_all(&dir).unwrap();
            dir
        }

        /// Decode what the command hands the frontend, so assertions run
        /// against the same bytes the browser would.
        fn decoded(path: &std::path::Path) -> Vec<u8> {
            STANDARD.decode(cursor_path_to_b64(path).unwrap()).unwrap()
        }

        #[test]
        fn a_cur_file_stays_a_still_png() {
            let dir = scratch_dir("cur");
            let path = dir.join("Arrow.cur");
            fs::write(&path, super::make_cur(0, 0)).unwrap();

            let out = decoded(&path);
            assert_eq!(&out[..8], b"\x89PNG\r\n\x1a\n");
            assert!(actl(&out).is_none(), "a .cur must not animate");

            fs::remove_dir_all(&dir).ok();
        }

        #[test]
        fn a_single_frame_ani_stays_a_still_png() {
            let dir = scratch_dir("one");
            let path = dir.join("Busy.ani");
            fs::write(&path, super::make_ani(1)).unwrap();

            let out = decoded(&path);
            assert!(actl(&out).is_none(), "one frame is not an animation");

            fs::remove_dir_all(&dir).ok();
        }

        #[test]
        fn a_multi_frame_ani_animates() {
            let dir = scratch_dir("many");
            let path = dir.join("Busy.ani");
            fs::write(&path, super::make_ani(3)).unwrap();

            let out = decoded(&path);
            assert_eq!(actl(&out).unwrap(), (3, 0), "3 frames, looping forever");
            // make_ani writes no rate chunk, so every frame takes the anih
            // display rate of 4 jiffies.
            assert_eq!(fctl_delays(&out), vec![(4, 60), (4, 60), (4, 60)]);

            fs::remove_dir_all(&dir).ok();
        }
    }
}
