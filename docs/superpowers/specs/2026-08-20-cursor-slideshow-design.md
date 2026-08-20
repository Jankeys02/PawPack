# Cursor Slideshow — Design

**Date:** 2026-08-20
**Status:** Approved, ready for implementation planning

## Problem

PawPack applies one fixed cursor set at a time. There is no way to keep a role
varied — to have "Busy" cycle between several hourglasses over the course of a
day. The Mix view already assigns one cursor per role; this feature makes a
role's assignment a *playlist* that advances on a timer.

## Goals

- Per-role playlists of hand-picked cursors, any length, chosen from a gallery.
- The user picks which roles participate; unlisted roles are untouched.
- Playlists advance as a sequential cycle on a user-set interval.
- Rotation continues after PawPack is closed, and across reboot.
- The background mechanism is visible and removable from inside the app.

## Non-goals (YAGNI)

Random or weighted ordering, per-role intervals, pausing one role without
clearing it, transitions/cross-fade, time-of-day scheduling. The cycle is the
whole feature. Add these only if the basic cycle proves boring in use.

## Key constraint that shapes everything

`windows::write_roles` (lib.rs) writes only `HKCU\Control Panel\Cursors` and
points values at files already inside PawPack's own packs directory — no copy
into `%SystemRoot%`, **no elevation**. A rotation tick is therefore ~17
registry writes plus one `SPI_SETCURSORS` broadcast: cheap, non-privileged, and
safe to run from an unattended background task.

## Data

New file `packs/slideshow.json`, beside the existing `mix.json`:

```json
{
  "enabled": true,
  "interval_minutes": 10,
  "roles": {
    "Wait":  { "items": [{"pack":"abc","file":"busy1.ani"},
                         {"pack":"def","file":"hourglass.ani"}], "index": 1 },
    "Arrow": { "items": [{"pack":"abc","file":"arrow.cur"}],     "index": 0 }
  }
}
```

- `items` — ordered playlist, any length. Same shape as `MixRef`, so the
  existing `is_safe_segment` / `pack_file_in` path guards apply unchanged.
  `slideshow.json` is a plain file a user can edit, so entries are re-validated
  on read, exactly as `read_mix` already does.
- `index` — cycle position, advanced and persisted on every tick.
- A role absent from `roles`, or present with an empty `items`, does not
  participate.
- Absent or unparseable file reads as an empty, disabled slideshow — losing a
  corrupt slideshow is recoverable, refusing to open the tab is not. This
  mirrors `load_mix`.

## Rotation engine

New module `src-tauri/src/slideshow.rs`. `lib.rs` is already past 2000 lines;
this logic is self-contained (load/save, advance, resolve, task registration)
and belongs in its own file.

`rotate_once(packs_base) -> Result<(), String>`:

1. Load `slideshow.json`. If disabled or no role has a usable playlist, return
   without touching the registry.
2. For each participating role, advance `index` and resolve the entry to an
   absolute path. An entry whose file no longer exists is skipped and the index
   advances past it, so a deleted pack cannot stall a cycle. A role whose every
   entry is stale is treated as not participating.
3. Layer the resolved roles **over the current mix** (`read_mix`), so roles the
   slideshow does not own keep their mix assignment rather than being cleared.
4. Call `windows::write_roles` with the merged map — the single existing
   chokepoint for cursor registry writes, so slideshow cannot drift from apply
   and mix in how it treats unfilled roles.
5. Persist the advanced indices.

Windows-only, guarded the same way `apply_mix` is; other platforms return an
explanatory error.

## Surviving quit

`main()` inspects argv before Tauri builds anything. With `--rotate` present it
resolves the packs directory, calls `rotate_once`, and exits — no window, no
event loop, well under a second.

The headless path cannot use `packs_dir(&AppHandle)`, which needs a running
Tauri app. It resolves `%APPDATA%\com.jankeys.pawpack\packs` from the
environment instead. This duplicates Tauri's `app_data_dir` convention in one
place; that coupling is noted in a comment at the helper, and a mismatch would
surface immediately as a slideshow that does nothing.

Registration uses the Windows scheduler rather than a daemon of our own:

```
schtasks /create /tn "PawPack Slideshow" /sc minute /mo <N> /tr "\"<exe>\" --rotate" /f
```

`/f` makes re-registration idempotent, so changing the interval is just another
create. Stopping runs `schtasks /delete /tn "PawPack Slideshow" /f`. Status is
read with `schtasks /query /tn "PawPack Slideshow"` — a non-zero exit means no
task.

**Accepted limitation:** Task Scheduler's repetition floor is one minute, so
sub-minute intervals are impossible. The interval input enforces a minimum of 1
and says why.

## UI — new "Slideshow" view

New nav entry and `src/views/Slideshow.tsx`.

- **Role rail** (left): all 17 roles from `CURSOR_ROLES`, each showing a
  playlist count badge.
- **Gallery** (right): every cursor in the library. Clicking toggles membership
  in the selected role's playlist; members show their cycle position. Stale
  members are flagged, matching how Mix surfaces stale mix entries.
- **Header bar**: interval in minutes (min 1, with the Task Scheduler reason
  stated), Start / Stop.
- **Background task disclosure**: a persistent status line naming the task —
  *PawPack Slideshow* — reporting whether it is currently registered, with a
  **Remove background task** action that works independently of the Start/Stop
  toggle. The user must never have to discover this in Task Scheduler.

### Shared gallery component

Mix already renders this gallery. It moves to
`src/components/CursorGallery.tsx` and both views consume it. This is a
targeted extraction to avoid duplicating the grid, thumbnails and selection
affordance — not a broader refactor of Mix.

## Interaction with existing actions

- **Revert stops the slideshow** (deletes the task). Otherwise reverted cursors
  silently return on the next tick, which reads as a bug.
- **Applying a pack stops the slideshow**, for the same reason: an explicit
  "use this set" must not be overwritten a minute later.
- **Applying a mix** likewise stops it — same argument.

In every case the UI reports that the slideshow was stopped, so the state
change is never silent.

## Tauri commands

| Command | Purpose |
|---|---|
| `get_slideshow` | Current config plus per-role stale entries, mirroring `MixResult` |
| `set_slideshow_role(role, items)` | Replace one role's playlist; empty clears it |
| `set_slideshow(enabled, interval_minutes)` | Toggle and interval; registers or deletes the task |
| `slideshow_status` | Whether the scheduled task is currently registered |
| `remove_slideshow_task` | Delete the task without altering saved playlists |

## Error handling

- Missing cursor file — skipped on tick, flagged as stale in UI, playlist kept
  (a pack may be re-imported), mirroring how `read_mix` preserves stale mix
  entries.
- `schtasks` failure — surfaced verbatim in the UI, `enabled` left false so the
  UI never claims a rotation that is not scheduled.
- Non-Windows — commands return the same explanatory error `apply_mix` does.
- Corrupt `slideshow.json` — treated as empty and disabled.

## Verification

One Rust unit test over the advance logic, covering: cycle wraps at arbitrary
length; a stale entry is skipped without stalling the index; an all-stale
playlist leaves the role alone; an empty playlist is a no-op. That is the only
non-trivial logic in the feature — the rest is existing, already-exercised code
paths.
