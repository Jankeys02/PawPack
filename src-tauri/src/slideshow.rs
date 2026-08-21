//! Per-role cursor playlists that advance on a schedule.
//!
//! The slideshow owns `packs/slideshow.json` and nothing else. Rotation
//! resolves each participating role to a file and hands the result to
//! `windows_cursor::write_roles` — the same chokepoint apply and mix use, so
//! the three cannot drift in how they treat unfilled roles.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use crate::pack_file_in;

/// One slide: a pack directory name plus a file inside it. Same shape as the
/// mix's `MixRef`, so the same path guards apply.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct SlideRef {
    pub pack: String,
    pub file: String,
}

/// One role's playlist and its position in the cycle.
#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct RolePlaylist {
    pub items: Vec<SlideRef>,
    /// Index of the slide currently showing. Advancing moves to the next.
    #[serde(default)]
    pub index: usize,
}

/// On-disk shape of `packs/slideshow.json`.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct SlideshowFile {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default = "default_interval")]
    pub interval_minutes: u32,
    /// Whether applying a pack or a mix stops the slideshow. User setting.
    #[serde(default = "default_true")]
    pub stop_on_apply: bool,
    #[serde(default)]
    pub roles: HashMap<String, RolePlaylist>,
}

fn default_interval() -> u32 {
    10
}

fn default_true() -> bool {
    true
}

impl Default for SlideshowFile {
    fn default() -> Self {
        Self {
            enabled: false,
            interval_minutes: default_interval(),
            stop_on_apply: true,
            roles: HashMap::new(),
        }
    }
}

pub fn path(packs_base: &Path) -> PathBuf {
    packs_base.join("slideshow.json")
}

/// Read `slideshow.json`, treating absent or unparseable as an empty, disabled
/// slideshow. Losing a corrupt slideshow is recoverable; refusing to open the
/// tab is not — the same trade `load_mix` makes.
pub fn load(packs_base: &Path) -> SlideshowFile {
    fs::read_to_string(path(packs_base))
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

pub fn save(packs_base: &Path, f: &SlideshowFile) -> Result<(), String> {
    let json = serde_json::to_string_pretty(f).map_err(|e| e.to_string())?;
    fs::write(path(packs_base), json).map_err(|e| e.to_string())
}

/// Advance every participating role one slide and resolve it to a path.
///
/// Mutates `f`'s indices in place; the caller persists. A slide whose file has
/// gone is skipped and the index moves past it, so a deleted pack cannot stall
/// a cycle. A role whose every slide is stale contributes nothing, leaving that
/// role to the mix rather than clearing it.
pub fn advance(packs_base: &Path, f: &mut SlideshowFile) -> HashMap<String, PathBuf> {
    let mut out = HashMap::new();

    for (role, list) in f.roles.iter_mut() {
        if list.items.is_empty() {
            continue;
        }
        let n = list.items.len();
        // At most `n` steps: after a full lap every slide has been tried, so an
        // all-stale playlist terminates instead of spinning.
        for step in 1..=n {
            let next = (list.index + step) % n;
            let slide = &list.items[next];
            let Ok(resolved) = pack_file_in(packs_base, &slide.pack, &slide.file) else {
                continue;
            };
            if resolved.is_file() {
                list.index = next;
                out.insert(role.clone(), resolved);
                break;
            }
        }
    }

    out
}

/// Advance every playlist and write the result to the registry.
///
/// Slideshow roles are layered *over* the mix so roles the slideshow does not
/// own keep their mix assignment instead of being reset to a Windows default.
/// This is the function the scheduled task calls.
#[cfg(target_os = "windows")]
pub fn rotate_once(packs_base: &Path) -> Result<(), String> {
    let mut f = load(packs_base);
    if !f.enabled {
        return Ok(());
    }

    let picked = advance(packs_base, &mut f);
    if picked.is_empty() {
        // Nothing usable to show. Leave the registry untouched rather than
        // clearing every role, which is what an empty map would otherwise do.
        return Ok(());
    }

    let mut paths: HashMap<String, PathBuf> = crate::read_mix(packs_base)
        .roles
        .iter()
        .filter_map(|e| {
            pack_file_in(packs_base, &e.pack, &e.file)
                .ok()
                .map(|p| (e.role.clone(), p))
        })
        .collect();
    paths.extend(picked);

    let borrowed: HashMap<&str, PathBuf> = paths
        .iter()
        .filter(|(r, _)| crate::windows_cursor::CURSOR_REG_NAMES.contains(&r.as_str()))
        .map(|(r, p)| (r.as_str(), p.clone()))
        .collect();

    crate::windows_cursor::write_roles(&borrowed)?;
    save(packs_base, &f)
}

#[cfg(not(target_os = "windows"))]
pub fn rotate_once(_packs_base: &Path) -> Result<(), String> {
    Err("rotate_once is only supported on Windows".into())
}

// ── Scheduled task ────────────────────────────────────────────────────────────

/// Name of the Windows scheduled task. Surfaced in the UI so the user can find
/// it in Task Scheduler, and used verbatim by every schtasks call here.
pub const TASK_NAME: &str = "PawPack Slideshow";

/// Run schtasks, with no console window flashing on each call.
#[cfg(target_os = "windows")]
fn schtasks(args: &[&str]) -> Result<std::process::Output, String> {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;

    std::process::Command::new("schtasks")
        .args(args)
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .map_err(|e| format!("Could not run schtasks: {e}"))
}

/// Register (or re-register) the rotation task. `/f` makes this idempotent, so
/// changing the interval is just another create.
///
/// The task runs as the current user with no elevation: rotation only writes
/// HKCU and reads files PawPack already owns.
///
/// It also runs "interactive only" (schtasks' default), and must. `rotate_once`
/// ends in `SystemParametersInfoW(SPI_SETCURSORS)`, whose broadcast only
/// reaches the desktop from inside the user's own session. Registering with
/// `/ru <user> /np` instead — "run whether the user is logged on or not" — puts
/// the task in session 0, where the registry write still lands but the
/// broadcast goes nowhere, so cursors would not visibly change until the next
/// sign-in.
///
/// The visible cost of staying interactive: in a *debug* build the exe is
/// console-subsystem (see the cfg_attr in main.rs, which is what keeps
/// `println!` working during development), so Windows gives each tick a console
/// window that briefly flashes on screen. `run()` calls `FreeConsole` as its
/// first act, which shortens the flash but cannot prevent it — the console is
/// allocated before `main` runs. Release builds are windows-subsystem and
/// allocate none, so this is a development-only annoyance and deliberately not
/// worth a second binary to fix.
///
/// One more consequence worth knowing: the task stores whichever executable
/// registered it. Registering from `tauri dev` points it at `target/debug`,
/// which breaks on `cargo clean` or if the repo moves. Starting the slideshow
/// from an installed build re-points it somewhere stable.
#[cfg(target_os = "windows")]
pub fn register_task(interval_minutes: u32) -> Result<(), String> {
    // Task Scheduler cannot repeat faster than once a minute.
    let every = interval_minutes.max(1).to_string();

    let exe = std::env::current_exe()
        .map_err(|e| format!("Cannot locate the PawPack executable: {e}"))?;
    let command = format!("\"{}\" --rotate", exe.display());

    let out = schtasks(&[
        "/create", "/tn", TASK_NAME, "/sc", "minute", "/mo", &every, "/tr", &command, "/f",
    ])?;

    if out.status.success() {
        Ok(())
    } else {
        Err(format!(
            "Could not register the background task: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        ))
    }
}

/// Remove the task. A task that is already gone is success, not an error.
#[cfg(target_os = "windows")]
pub fn delete_task() -> Result<(), String> {
    if !task_exists() {
        return Ok(());
    }
    let out = schtasks(&["/delete", "/tn", TASK_NAME, "/f"])?;
    if out.status.success() {
        Ok(())
    } else {
        Err(format!(
            "Could not remove the background task: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        ))
    }
}

/// Read from Windows rather than from our own file: the user may have deleted
/// the task in Task Scheduler behind our back.
#[cfg(target_os = "windows")]
pub fn task_exists() -> bool {
    schtasks(&["/query", "/tn", TASK_NAME]).is_ok_and(|o| o.status.success())
}

#[cfg(not(target_os = "windows"))]
pub fn register_task(_interval_minutes: u32) -> Result<(), String> {
    Err("The slideshow is only supported on Windows".into())
}

#[cfg(not(target_os = "windows"))]
pub fn delete_task() -> Result<(), String> {
    Ok(())
}

#[cfg(not(target_os = "windows"))]
pub fn task_exists() -> bool {
    false
}

/// The packs directory, resolved without a running Tauri app.
///
/// `packs_dir()` needs an `AppHandle`, which the `--rotate` process has no
/// reason to build. This mirrors Tauri's own `app_data_dir` convention on
/// Windows: `%APPDATA%\<identifier>`. If the identifier in tauri.conf.json ever
/// changes, change it here too — a mismatch shows up immediately as a slideshow
/// that ticks but never changes anything.
#[cfg(target_os = "windows")]
pub fn packs_base_from_env() -> Result<PathBuf, String> {
    let appdata = std::env::var("APPDATA")
        .map_err(|_| "APPDATA is not set; cannot locate PawPack's data directory".to_string())?;
    Ok(PathBuf::from(appdata)
        .join("com.jankeys.pawpack")
        .join("packs"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    /// Build a packs base with `pack/file` present for each pair given.
    fn base_with(files: &[(&str, &str)]) -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        for (pack, file) in files {
            let p = dir.path().join(pack);
            fs::create_dir_all(&p).unwrap();
            fs::write(p.join(file), b"x").unwrap();
        }
        dir
    }

    fn playlist(items: &[(&str, &str)], index: usize) -> RolePlaylist {
        RolePlaylist {
            items: items
                .iter()
                .map(|(p, f)| SlideRef {
                    pack: p.to_string(),
                    file: f.to_string(),
                })
                .collect(),
            index,
        }
    }

    fn with_role(role: &str, list: RolePlaylist) -> SlideshowFile {
        let mut f = SlideshowFile {
            enabled: true,
            ..Default::default()
        };
        f.roles.insert(role.to_string(), list);
        f
    }

    #[test]
    fn advance_cycles_and_wraps_at_any_length() {
        let dir = base_with(&[("p", "a.cur"), ("p", "b.cur"), ("p", "c.cur")]);
        let mut f = with_role(
            "Arrow",
            playlist(&[("p", "a.cur"), ("p", "b.cur"), ("p", "c.cur")], 0),
        );

        // index 0 is the *current* slide; advancing moves to the next one.
        let picked: Vec<String> = (0..4)
            .map(|_| {
                let m = advance(dir.path(), &mut f);
                m["Arrow"]
                    .file_name()
                    .unwrap()
                    .to_string_lossy()
                    .into_owned()
            })
            .collect();

        assert_eq!(picked, vec!["b.cur", "c.cur", "a.cur", "b.cur"]);
    }

    #[test]
    fn advance_skips_missing_files_without_stalling() {
        // b.cur is never created — its pack was deleted.
        let dir = base_with(&[("p", "a.cur"), ("p", "c.cur")]);
        let mut f = with_role(
            "Arrow",
            playlist(&[("p", "a.cur"), ("p", "b.cur"), ("p", "c.cur")], 0),
        );

        let m = advance(dir.path(), &mut f);
        assert_eq!(m["Arrow"].file_name().unwrap(), "c.cur");
    }

    #[test]
    fn all_stale_playlist_yields_no_role() {
        let dir = base_with(&[]);
        let mut f = with_role("Arrow", playlist(&[("p", "a.cur"), ("p", "b.cur")], 0));
        assert!(advance(dir.path(), &mut f).is_empty());
    }

    #[test]
    fn empty_playlist_is_a_noop() {
        let dir = base_with(&[]);
        let mut f = with_role("Arrow", playlist(&[], 0));
        assert!(advance(dir.path(), &mut f).is_empty());
    }

    #[test]
    fn traversing_entry_is_treated_as_stale() {
        let dir = base_with(&[("p", "a.cur")]);
        let mut f = with_role("Arrow", playlist(&[("..", "evil.cur")], 0));
        assert!(advance(dir.path(), &mut f).is_empty());
    }

    #[test]
    fn corrupt_file_loads_as_default() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("slideshow.json"), b"{ not json").unwrap();
        let f = load(dir.path());
        assert!(!f.enabled);
        assert!(f.roles.is_empty());
        assert_eq!(f.interval_minutes, 10);
        assert!(f.stop_on_apply);
    }
}
