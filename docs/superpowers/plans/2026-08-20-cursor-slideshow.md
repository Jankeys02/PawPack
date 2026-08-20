# Cursor Slideshow Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Let a user build a per-role playlist of cursors that cycles on a timer and keeps rotating after PawPack is closed.

**Architecture:** A new `slideshow.rs` module owns `packs/slideshow.json` (playlists, cycle indices, interval, `stop_on_apply`). Rotation layers the advanced entries over the existing mix and routes through the one existing registry chokepoint, `windows_cursor::write_roles`. Persistence across quit is delegated to the Windows Task Scheduler via `schtasks`, firing `pawpack.exe --rotate`, which `main()` intercepts before Tauri builds a window.

**Tech Stack:** Rust + Tauri v2, serde_json, `schtasks.exe`, React 19 + Tailwind v4.

## Global Constraints

- Windows-only feature. Every command is `#[cfg(target_os = "windows")]`-guarded and returns `"<command> is only supported on Windows"` elsewhere, matching `apply_mix`.
- Interval minimum is **1 minute** — Task Scheduler's repetition floor. Enforce in both Rust and the input.
- Scheduled task name is exactly `PawPack Slideshow`.
- All registry writes go through `windows_cursor::write_roles`. Never open `Control Panel\Cursors` anywhere else.
- Playlist entries are re-validated on read with the existing `pack_file_in`, which rejects traversal. `slideshow.json` is user-editable, so untrusted on every read.
- A corrupt or absent `slideshow.json` reads as `SlideshowFile::default()` (disabled, no roles) — never an error.
- UI colors follow the existing palette: amber-500 accents, zinc-800 borders, zinc-900 panels.

---

### Task 1: Slideshow data model and cycle advance

**Files:**
- Create: `src-tauri/src/slideshow.rs`
- Modify: `src-tauri/src/lib.rs` (add `pub mod slideshow;` and re-export helpers it needs)
- Test: `src-tauri/src/slideshow.rs` (inline `#[cfg(test)] mod tests`, matching lib.rs's existing convention)

**Interfaces:**
- Consumes: `pack_file_in(base, pack, file) -> Result<PathBuf, String>` from lib.rs (must be made `pub(crate)`).
- Produces:
  - `pub struct SlideRef { pub pack: String, pub file: String }`
  - `pub struct RolePlaylist { pub items: Vec<SlideRef>, pub index: usize }`
  - `pub struct SlideshowFile { pub enabled: bool, pub interval_minutes: u32, pub stop_on_apply: bool, pub roles: HashMap<String, RolePlaylist> }`
  - `pub fn load(packs_base: &Path) -> SlideshowFile`
  - `pub fn save(packs_base: &Path, f: &SlideshowFile) -> Result<(), String>`
  - `pub fn advance(packs_base: &Path, f: &mut SlideshowFile) -> HashMap<String, PathBuf>`

- [ ] **Step 1: Write the failing tests**

Append to `src-tauri/src/slideshow.rs`:

```rust
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
                .map(|(p, f)| SlideRef { pack: p.to_string(), file: f.to_string() })
                .collect(),
            index,
        }
    }

    fn with_role(role: &str, list: RolePlaylist) -> SlideshowFile {
        let mut f = SlideshowFile::default();
        f.enabled = true;
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
                m["Arrow"].file_name().unwrap().to_string_lossy().into_owned()
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
```

- [ ] **Step 2: Add the `tempfile` dev-dependency and run the tests to verify they fail**

Add to `src-tauri/Cargo.toml` under `[dev-dependencies]` (create the section if absent):

```toml
[dev-dependencies]
tempfile = "3"
```

Run: `cd src-tauri && cargo test slideshow`
Expected: FAIL — `cannot find function `advance`` / unresolved module.

- [ ] **Step 3: Write the implementation**

Prepend to `src-tauri/src/slideshow.rs`:

```rust
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

fn default_interval() -> u32 { 10 }
fn default_true() -> bool { true }

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
            if let Ok(p) = pack_file_in(packs_base, &slide.pack, &slide.file) {
                if p.is_file() {
                    list.index = next;
                    out.insert(role.clone(), p);
                    break;
                }
            }
        }
    }

    out
}
```

Add to `src-tauri/src/lib.rs`, just below the existing `use` block:

```rust
pub mod slideshow;
```

and change `fn pack_file_in` to `pub(crate) fn pack_file_in`.

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cd src-tauri && cargo test slideshow`
Expected: PASS — 6 passed.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/slideshow.rs src-tauri/src/lib.rs src-tauri/Cargo.toml
git commit -m "feat: add slideshow playlist model and cycle advance"
```

---

### Task 2: Rotation tick

**Files:**
- Modify: `src-tauri/src/slideshow.rs`

**Interfaces:**
- Consumes: `advance`, `load`, `save` from Task 1; `crate::read_mix`, `crate::pack_file_in`, `crate::windows_cursor::{write_roles, CURSOR_REG_NAMES}`.
- Produces: `pub fn rotate_once(packs_base: &Path) -> Result<(), String>`

- [ ] **Step 1: Write the implementation**

Append to `src-tauri/src/slideshow.rs` (above the test module):

```rust
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
```

- [ ] **Step 2: Verify it compiles**

Run: `cd src-tauri && cargo check`
Expected: no errors. If `windows_cursor` or `read_mix` are private, make them `pub(crate)` in lib.rs.

- [ ] **Step 3: Commit**

```bash
git add src-tauri/src/slideshow.rs src-tauri/src/lib.rs
git commit -m "feat: rotate slideshow roles over the current mix"
```

---

### Task 3: Scheduled task registration

**Files:**
- Modify: `src-tauri/src/slideshow.rs`

**Interfaces:**
- Produces:
  - `pub const TASK_NAME: &str = "PawPack Slideshow";`
  - `pub fn register_task(interval_minutes: u32) -> Result<(), String>`
  - `pub fn delete_task() -> Result<(), String>`
  - `pub fn task_exists() -> bool`

- [ ] **Step 1: Write the implementation**

Append to `src-tauri/src/slideshow.rs`:

```rust
/// Name of the Windows scheduled task. Surfaced in the UI so the user can find
/// it in Task Scheduler, and used verbatim by every schtasks call here.
pub const TASK_NAME: &str = "PawPack Slideshow";

/// Run schtasks and turn a non-zero exit into the stderr text it printed.
#[cfg(target_os = "windows")]
fn schtasks(args: &[&str]) -> Result<std::process::Output, String> {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;

    std::process::Command::new("schtasks")
        .args(args)
        .creation_flags(CREATE_NO_WINDOW) // no console flash on each call
        .output()
        .map_err(|e| format!("Could not run schtasks: {e}"))
}

/// Register (or re-register) the rotation task. `/f` makes this idempotent, so
/// changing the interval is just another create.
///
/// The task runs as the current user with no elevation: rotation only writes
/// HKCU and reads files PawPack already owns.
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

#[cfg(target_os = "windows")]
pub fn task_exists() -> bool {
    schtasks(&["/query", "/tn", TASK_NAME]).is_ok_and(|o| o.status.success())
}

#[cfg(not(target_os = "windows"))]
pub fn register_task(_interval_minutes: u32) -> Result<(), String> {
    Err("The slideshow is only supported on Windows".into())
}

#[cfg(not(target_os = "windows"))]
pub fn delete_task() -> Result<(), String> { Ok(()) }

#[cfg(not(target_os = "windows"))]
pub fn task_exists() -> bool { false }
```

- [ ] **Step 2: Verify it compiles**

Run: `cd src-tauri && cargo check`
Expected: no errors.

- [ ] **Step 3: Commit**

```bash
git add src-tauri/src/slideshow.rs
git commit -m "feat: register the slideshow rotation as a Windows scheduled task"
```

---

### Task 4: Headless `--rotate` entry point

**Files:**
- Modify: `src-tauri/src/lib.rs` (`run()`)
- Modify: `src-tauri/src/slideshow.rs` (packs base from environment)

**Interfaces:**
- Produces: `pub fn packs_base_from_env() -> Result<PathBuf, String>`

- [ ] **Step 1: Write the environment path helper**

Append to `src-tauri/src/slideshow.rs`:

```rust
/// The packs directory, resolved without a running Tauri app.
///
/// `packs_dir()` needs an `AppHandle`, which the `--rotate` process has no
/// reason to build. This mirrors Tauri's own `app_data_dir` convention on
/// Windows: `%APPDATA%\<identifier>`. If the identifier in tauri.conf.json
/// ever changes, change it here too — a mismatch shows up immediately as a
/// slideshow that ticks but never changes anything.
#[cfg(target_os = "windows")]
pub fn packs_base_from_env() -> Result<PathBuf, String> {
    let appdata = std::env::var("APPDATA")
        .map_err(|_| "APPDATA is not set; cannot locate PawPack's data directory".to_string())?;
    Ok(PathBuf::from(appdata).join("com.jankeys.pawpack").join("packs"))
}
```

- [ ] **Step 2: Intercept the flag in `run()`**

In `src-tauri/src/lib.rs`, make `run()` begin with:

```rust
pub fn run() {
    // Fired by the "PawPack Slideshow" scheduled task. Rotate and exit without
    // ever building a window or an event loop — this must stay before
    // `tauri::Builder`, or every tick would flash the UI.
    #[cfg(target_os = "windows")]
    if std::env::args().any(|a| a == "--rotate") {
        if let Ok(base) = slideshow::packs_base_from_env() {
            let _ = slideshow::rotate_once(&base);
        }
        return;
    }

    tauri::Builder::default()
```

- [ ] **Step 3: Verify it compiles**

Run: `cd src-tauri && cargo check`
Expected: no errors.

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/lib.rs src-tauri/src/slideshow.rs
git commit -m "feat: run a headless rotation on --rotate"
```

---

### Task 5: Tauri commands and stop-on-apply

**Files:**
- Modify: `src-tauri/src/lib.rs`

**Interfaces:**
- Produces commands: `get_slideshow`, `set_slideshow_role`, `set_slideshow`, `set_slideshow_stop_on_apply`, `remove_slideshow_task`.
- Produces: `pub struct SlideshowState { pub enabled: bool, pub interval_minutes: u32, pub stop_on_apply: bool, pub task_registered: bool, pub roles: HashMap<String, Vec<SlideRef>>, pub stale: Vec<MixEntry> }`

- [ ] **Step 1: Write the command block**

Add to `src-tauri/src/lib.rs`, after `apply_mix`:

```rust
/// The slideshow as the UI sees it: playlists, plus entries whose files are
/// gone, plus whether the scheduled task is actually registered right now.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct SlideshowState {
    pub enabled: bool,
    pub interval_minutes: u32,
    pub stop_on_apply: bool,
    /// Read from Windows, not from our own file — the user may have deleted
    /// the task in Task Scheduler behind our back.
    pub task_registered: bool,
    pub task_name: String,
    pub roles: HashMap<String, Vec<slideshow::SlideRef>>,
    pub stale: Vec<MixEntry>,
}

fn slideshow_state(base: &Path) -> SlideshowState {
    let f = slideshow::load(base);
    let mut stale = Vec::new();

    for (role, list) in &f.roles {
        for item in &list.items {
            if !pack_file_in(base, &item.pack, &item.file).is_ok_and(|p| p.is_file()) {
                stale.push(MixEntry {
                    role: role.clone(),
                    pack: item.pack.clone(),
                    file: item.file.clone(),
                });
            }
        }
    }
    stale.sort_by(|a, b| a.role.cmp(&b.role));

    SlideshowState {
        enabled: f.enabled,
        interval_minutes: f.interval_minutes,
        stop_on_apply: f.stop_on_apply,
        task_registered: slideshow::task_exists(),
        task_name: slideshow::TASK_NAME.to_string(),
        roles: f.roles.into_iter().map(|(r, l)| (r, l.items)).collect(),
        stale,
    }
}

#[tauri::command]
fn get_slideshow(app: tauri::AppHandle) -> Result<SlideshowState, String> {
    let base = packs_dir(&app)?;
    Ok(slideshow_state(&base))
}

/// Replace one role's playlist. An empty `items` removes the role entirely.
#[tauri::command]
fn set_slideshow_role(
    app: tauri::AppHandle,
    role: String,
    items: Vec<slideshow::SlideRef>,
) -> Result<SlideshowState, String> {
    let base = packs_dir(&app)?;

    // Validate every slide before storing it: these are persisted and later
    // resolved into registry paths, so none may escape the packs directory.
    for item in &items {
        if !pack_file_in(&base, &item.pack, &item.file)?.is_file() {
            return Err(format!("Cursor file not found: {}", item.file));
        }
    }

    let mut f = slideshow::load(&base);
    if items.is_empty() {
        f.roles.remove(&role);
    } else {
        f.roles.insert(role, slideshow::RolePlaylist { items, index: 0 });
    }
    slideshow::save(&base, &f)?;
    Ok(slideshow_state(&base))
}

/// Start or stop the slideshow, and set its interval.
#[tauri::command]
fn set_slideshow(
    app: tauri::AppHandle,
    enabled: bool,
    interval_minutes: u32,
) -> Result<SlideshowState, String> {
    let base = packs_dir(&app)?;
    let mut f = slideshow::load(&base);

    // Task Scheduler cannot repeat faster than once a minute.
    f.interval_minutes = interval_minutes.max(1);

    if enabled {
        if f.roles.values().all(|l| l.items.is_empty()) {
            return Err("Add at least one cursor to a role before starting the slideshow".into());
        }
        ensure_snapshot(&base)?;
        // Register first: leaving `enabled` true with no task would have the UI
        // claim a rotation that is not actually scheduled.
        slideshow::register_task(f.interval_minutes)?;
    } else {
        slideshow::delete_task()?;
    }

    f.enabled = enabled;
    slideshow::save(&base, &f)?;
    Ok(slideshow_state(&base))
}

/// Whether applying a pack or a mix stops the slideshow.
#[tauri::command]
fn set_slideshow_stop_on_apply(
    app: tauri::AppHandle,
    enabled: bool,
) -> Result<SlideshowState, String> {
    let base = packs_dir(&app)?;
    let mut f = slideshow::load(&base);
    f.stop_on_apply = enabled;
    slideshow::save(&base, &f)?;
    Ok(slideshow_state(&base))
}

/// Delete the scheduled task, keeping the saved playlists.
#[tauri::command]
fn remove_slideshow_task(app: tauri::AppHandle) -> Result<SlideshowState, String> {
    let base = packs_dir(&app)?;
    slideshow::delete_task()?;
    let mut f = slideshow::load(&base);
    f.enabled = false;
    slideshow::save(&base, &f)?;
    Ok(slideshow_state(&base))
}
```

- [ ] **Step 2: Hook stop-on-apply into the three actions**

Add this helper next to `ensure_snapshot` in `src-tauri/src/lib.rs`:

```rust
/// Stop a running slideshow so it cannot overwrite what was just applied.
///
/// `force` is for revert, which always stops: restoring the user's cursors
/// while a background task keeps re-applying them is broken in a way nobody
/// wants. Apply respects the `stop_on_apply` setting instead.
///
/// Returns true when a running slideshow was actually stopped, so the UI can
/// say so rather than changing state silently.
#[cfg(target_os = "windows")]
fn stop_slideshow_for_apply(base: &Path, force: bool) -> bool {
    let mut f = slideshow::load(base);
    if !f.enabled || (!force && !f.stop_on_apply) {
        return false;
    }
    if slideshow::delete_task().is_err() {
        return false;
    }
    f.enabled = false;
    let _ = slideshow::save(base, &f);
    true
}
```

Then, in `apply_pack`, replace `windows_cursor::apply(&pack_dir)` with:

```rust
        stop_slideshow_for_apply(&base, false);
        windows_cursor::apply(&pack_dir)
```

In `apply_mix`, insert before `Ok(ApplyMixResult { ... })`:

```rust
        stop_slideshow_for_apply(&base, false);
```

In `revert_cursors`, insert before `windows_cursor::revert(&snapshot)`:

```rust
        stop_slideshow_for_apply(&base, true);
```

- [ ] **Step 3: Register the commands**

In `run()`, add to `tauri::generate_handler![...]` after `apply_mix`:

```rust
            get_slideshow,
            set_slideshow_role,
            set_slideshow,
            set_slideshow_stop_on_apply,
            remove_slideshow_task,
```

- [ ] **Step 4: Verify it compiles and tests still pass**

Run: `cd src-tauri && cargo test`
Expected: PASS, no warnings about unused items.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/lib.rs
git commit -m "feat: expose slideshow commands and stop-on-apply"
```

---

### Task 6: Extract the cursor gallery

**Files:**
- Create: `src/components/CursorGallery.tsx`
- Modify: `src/views/Mix.tsx`

**Interfaces:**
- Produces:
  - `export interface CursorEntry { name: string; kind: string; thumbnail: string }`
  - `export interface PackCursors { pack: { id: string; name: string }; cursors: CursorEntry[] }`
  - `export default function CursorGallery({ library, onPick, badgeFor, empty }: { library: PackCursors[]; onPick: (packId: string, file: string) => void; badgeFor?: (packId: string, file: string) => number | null; empty?: React.ReactNode })`

`badgeFor` returns a 1-based cycle position to draw on the tile, or null. Mix passes nothing; Slideshow uses it to number playlist members.

- [ ] **Step 1: Write the component**

Create `src/components/CursorGallery.tsx`:

```tsx
import { cn } from "@/lib/utils";

export interface CursorEntry {
  name: string;
  kind: string;
  /** base64 PNG; empty string when decoding failed. */
  thumbnail: string;
}

export interface PackCursors {
  pack: { id: string; name: string };
  cursors: CursorEntry[];
}

/**
 * Every cursor in the library, grouped by pack, as clickable tiles.
 *
 * Shared by Mix (pick one cursor for a role) and Slideshow (toggle cursors in
 * and out of a role's playlist) — `badgeFor` is what separates the two: return
 * a cycle position to mark a tile as selected, or null to leave it plain.
 */
export default function CursorGallery({
  library,
  onPick,
  badgeFor,
  empty,
}: {
  library: PackCursors[];
  onPick: (packId: string, file: string) => void;
  badgeFor?: (packId: string, file: string) => number | null;
  empty?: React.ReactNode;
}) {
  if (empty) {
    return (
      <div className="flex h-full flex-col items-center justify-center gap-3 text-zinc-600">
        {empty}
      </div>
    );
  }

  return (
    <>
      {library.map((p) => (
        <div key={p.pack.id} className="mb-5">
          <p className="mb-2 font-mono text-[10px] uppercase tracking-[0.14em] text-zinc-600">
            {p.pack.name}
          </p>
          <div className="flex flex-wrap gap-2">
            {p.cursors.map((c) => {
              const badge = badgeFor?.(p.pack.id, c.name) ?? null;
              return (
                <button
                  key={c.name}
                  onClick={() => onPick(p.pack.id, c.name)}
                  title={c.name}
                  className={cn(
                    "relative flex h-14 w-14 items-center justify-center rounded border bg-zinc-950/60 transition-colors",
                    badge !== null
                      ? "border-amber-500 bg-amber-500/10"
                      : "border-zinc-800 hover:border-amber-500/40",
                  )}
                >
                  {c.thumbnail ? (
                    <img
                      src={`data:image/png;base64,${c.thumbnail}`}
                      alt={c.name}
                      className="h-8 w-8 object-contain"
                      style={{ imageRendering: "pixelated" }}
                    />
                  ) : (
                    <span className="font-mono text-[9px] text-zinc-700">?</span>
                  )}
                  {badge !== null && (
                    <span className="absolute -right-1 -top-1 flex h-4 w-4 items-center justify-center rounded-full bg-amber-500 font-mono text-[9px] font-semibold text-zinc-950">
                      {badge}
                    </span>
                  )}
                </button>
              );
            })}
          </div>
        </div>
      ))}
    </>
  );
}
```

- [ ] **Step 2: Use it from Mix**

In `src/views/Mix.tsx`, delete the local `CursorEntry` and `PackCursors` interfaces and import instead:

```tsx
import CursorGallery, { type CursorEntry, type PackCursors } from "@/components/CursorGallery";
```

Replace the whole gallery `<div className="flex-1 overflow-y-auto ...">` body with:

```tsx
        <div className="flex-1 overflow-y-auto rounded-sm border border-zinc-800 bg-zinc-900/40 p-4">
          <CursorGallery
            library={library}
            onPick={assign}
            empty={
              !selectedRole ? (
                <>
                  <Shuffle className="h-6 w-6" strokeWidth={1.5} />
                  <p className="text-sm">Select a role first</p>
                </>
              ) : undefined
            }
          />
        </div>
```

- [ ] **Step 3: Verify the build is clean**

Run: `npm run build`
Expected: `tsc` passes, vite build succeeds.

- [ ] **Step 4: Commit**

```bash
git add src/components/CursorGallery.tsx src/views/Mix.tsx
git commit -m "refactor: extract the cursor gallery shared by Mix and Slideshow"
```

---

### Task 7: Slideshow view

**Files:**
- Create: `src/views/Slideshow.tsx`
- Modify: `src/App.tsx`

**Interfaces:**
- Consumes: commands from Task 5, `CursorGallery` from Task 6, `CURSOR_ROLES` from `@/lib/roles`.

- [ ] **Step 1: Write the view**

Create `src/views/Slideshow.tsx` with: state loaded from `get_slideshow`; a 17-row role rail with playlist count badges; `CursorGallery` with `badgeFor` returning the 1-based playlist position and `onPick` toggling membership through `set_slideshow_role`; a header with a minutes input (`min={1}`), Start/Stop calling `set_slideshow`, and a task disclosure line naming `state.task_name` with a **Remove background task** button calling `remove_slideshow_task`.

Full source is written during implementation following the exact markup conventions of `Mix.tsx` (same header bar, error banner, two-panel body).

- [ ] **Step 2: Wire it into the nav**

In `src/App.tsx`: import `Slideshow`, add `"slideshow"` to `NavId`, add `{ id: "slideshow", label: "Slideshow", Icon: Clapperboard }` to `mainNav` after `mix`, import `Clapperboard` from lucide-react, and add the `active === "slideshow" ? <Slideshow /> :` branch.

- [ ] **Step 3: Verify the build**

Run: `npm run build`
Expected: clean.

- [ ] **Step 4: Commit**

```bash
git add src/views/Slideshow.tsx src/App.tsx
git commit -m "feat: add the Slideshow view"
```

---

### Task 8: Stop-on-apply setting in Settings

**Files:**
- Modify: `src/views/Settings.tsx`

- [ ] **Step 1: Add the section**

Load slideshow state alongside the pointer flags, and render a second section below "Pointer" reusing `ToggleRow`:

```tsx
<p className="mb-1.5 mt-5 font-mono text-[10px] uppercase tracking-wide text-zinc-600">
  Slideshow
</p>
<div className="divide-y divide-zinc-800/60 rounded border border-zinc-800 bg-zinc-950/60">
  <ToggleRow
    Icon={Clapperboard}
    label="Applying stops the slideshow"
    description="Applying a pack or a mix turns the rotation off. Off, the slideshow reclaims its roles on the next tick."
    enabled={slideshow?.stop_on_apply ?? null}
    busy={busy === "stop_on_apply"}
    error={errors.stop_on_apply || null}
    onToggle={toggleStopOnApply}
  />
</div>
```

`toggleStopOnApply` calls `set_slideshow_stop_on_apply` with the negated value and stores the returned state.

- [ ] **Step 2: Verify the build**

Run: `npm run build`
Expected: clean.

- [ ] **Step 3: Commit**

```bash
git add src/views/Settings.tsx
git commit -m "feat: add the stop-on-apply slideshow setting"
```

---

## Manual verification (after Task 8)

1. `npm run tauri dev`
2. Slideshow tab → pick "Busy" → click three hourglass cursors → badges read 1, 2, 3.
3. Set interval 1, Start. Disclosure line reports the task as registered.
4. `schtasks /query /tn "PawPack Slideshow"` in a terminal shows the task.
5. Wait a minute; the Busy cursor changes. Close PawPack; wait; it changes again.
6. Stop → `schtasks /query` reports the task is gone.
7. Settings → turn "Applying stops the slideshow" off → start the slideshow → apply a pack → the task survives.
