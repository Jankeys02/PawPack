# Cursor Mix & Match

Date: 2026-08-18
Status: approved, not yet implemented

## Goal

Let the user build a cursor set that draws from every imported pack at once —
a hand from one pack, an arrow from another — and apply it to Windows as a
single set.

## Non-goals

- Multiple saved mixes. There is exactly one mix. Named loadouts can come later
  if switching between setups turns out to matter.
- Copying files into a generated pack. The mix references files where they
  already live.
- Editing cursors (hotspots, frame timing). That stays the Editor tab's job.

## Background

Two facts about the existing code shape this design.

`windows_cursor::apply` writes an absolute path per role into
`HKCU\Control Panel\Cursors`. Nothing in the registry ties those paths to a
single directory, so a set spanning several packs needs no new Windows
mechanism — only a different way of deciding which path goes where.

The per-pack `overrides.json` maps role to filename *within one pack*. A mix
needs pack plus filename, so it gets its own file. The two never read each
other, and existing overrides keep working untouched.

## Phase 1 — Apply tab corrections

Landing first, as its own commit. Phase 2 depends on the state change here.

### Single source of truth for what is applied

`CardStatus` currently lives in `useState` inside each `PackCard`
(`src/views/Apply.tsx:222`). Each card independently remembers having been
applied and nothing clears it, so every pack applied this session keeps showing
an "Applied — N cursor roles written" banner. The screenshots show three cards
each claiming to be applied.

Whether a pack is applied is one fact about the system, not one fact per card.
Move it to a single slot in the `Apply` parent:

```ts
type AppliedTarget =
  | { kind: "pack"; id: string; name: string }
  | { kind: "mix" }

type Applied = { target: AppliedTarget; result: PackAssignmentResult } | null
```

Each card derives its banner and ACTIVE badge from whether it is the target.
Applying anything replaces the slot, so every other banner clears on its own.
This also fixes an unreported case: reverting today resets only the card that
was reverted, leaving other stale banners in place.

Transient per-card state — `applying`, `reverting`, and `error` — stays local,
since those genuinely are per-card and concurrent.

This slot is the same value `App.tsx` persists to `localStorage` under
`pawpack:activePack`. Stored values from earlier builds have no `kind` field
and are read as `{kind: "pack"}`.

### Thumbnails in the assignment table

The expanded assignment table lists filenames from `get_pack_assignments`,
which carries no images. `list_pack_cursors` already returns a base64 PNG per
file. Fetch it when a card expands and join on filename to add a thumbnail
column.

The same call and the same join populate the Phase 2 gallery, so this is built
once and used in both places.

## Phase 2 — Mix tab

### Data model

New file `packs/mix.json`, a sibling of `packs/revert.json`:

```json
{
  "roles": {
    "Arrow": { "pack": "bog-cursor-pack", "file": "Arrow.cur" },
    "Hand":  { "pack": "phm-cursors-by-figuraine", "file": "Adrian_Select.cur" }
  }
}
```

A role absent from `roles` is empty. Role names are the same 17 registry value
names in `CURSOR_REG_NAMES`.

### Rust commands

`get_mix() -> MixResult`

Reads `mix.json`. Drops any entry whose pack directory or file no longer
exists and returns those separately so the UI can report them:

```rust
pub struct MixResult {
    pub roles: Vec<MixEntry>,   // role, pack, file — all still present on disk
    pub stale: Vec<MixEntry>,   // referenced a pack or file that is gone
}
```

Dropping is not persisted. `mix.json` is rewritten only by `set_mix_role`, so a
pack that is deleted and reimported under the same id restores its entries.

`set_mix_role(role, pack_id, file) -> MixResult`

Writes one entry and returns the updated mix. An empty `pack_id` clears the
role. Validates that `pack_id` resolves inside the packs directory, matching
the existing `starts_with(&base)` guard on every other pack command.

`apply_mix() -> ApplyMixResult`

Errors if the mix has no roles, rather than clearing all 17. Otherwise takes
the revert snapshot through the existing `snapshot_path` guard, writes filled
roles, deletes unfilled ones, and calls `SPI_SETCURSORS`.

Returns `{ written: usize, cleared: Vec<String> }` so the UI can state exactly
what happened.

### Registry write refactor

`windows_cursor::apply` currently both computes assignments and writes the
registry. `apply_mix` needs the second half with a different source for the
first. Extract:

```rust
fn write_roles(paths: &HashMap<&str, PathBuf>) -> Result<(), String>
```

Every role in `CURSOR_REG_NAMES` absent from `paths` has its value deleted.
`apply` and `apply_mix` both call it, so one function touches the registry.

### Frontend

New `src/views/Mix.tsx`, nav entry after Apply.

Left panel: the 17 roles, each showing its assigned thumbnail or an empty slot.
Clicking selects a role.

Right panel: every cursor from every pack as a thumbnail grid, grouped by pack.
Clicking one fills the selected role. Sourced from `list_packs` +
`list_pack_cursors` per pack — no new command. With no role selected, clicking
a cursor does nothing; the panel shows "Select a role first" until one is.

Header: a filled count ("9 / 17"), an Apply Mix button, and a "Fill from pack…"
dropdown. That dropdown copies in the roles the chosen pack actually assigns —
`get_pack_assignments`, so per-pack overrides are respected — and overwrites
only those roles. Roles the pack does not assign keep whatever the mix already
had, so seeding from Brush Buddy fills three roles rather than clearing
fourteen. When fewer than 17 roles are filled, an inline warning states how
many will reset to Windows defaults.

Stale entries render as a dismissible line: "2 cursors unavailable — pack
deleted."

### Shared role list

`src/views/Debug.tsx:30` hardcodes the 17 roles as `ZONES` with labels and
icons. Mix needs the same list. Move it to `src/lib/roles.ts` and have Debug
import it, so the two cannot drift.

The CSS-cursor fields on `ZONES` are Debug-specific and stay in Debug.

### Apply semantics

Applying a mix clears every role not in it, matching how applying a pack
behaves. What the tab shows is what the user gets — there is no inherited
state to explain.

Applying a mix sets the applied slot to `{kind: "mix"}`, so every pack card
loses its ACTIVE badge and banner. Reverting clears the slot to `null`.

The Debug tab resolves its custom cursors from the mix when the slot is
`{kind: "mix"}`. `parse_cur` and `parse_ani` already take pack id and cursor
name, so they need no change.

## Error handling

| Case | Behavior |
|---|---|
| Pack deleted while referenced by the mix | `get_mix` reports the entries as stale; UI shows a count |
| `apply_mix` with an empty mix | Error, registry untouched |
| `set_mix_role` with a pack id outside the packs directory | Error, same guard as other commands |
| Mix references a file that was removed from a pack | Treated as stale, same as a deleted pack |
| `mix.json` corrupt or unparseable | Treated as an empty mix; next `set_mix_role` rewrites it |

## Testing

Registry writes need a real registry and stay untested, which is why
`write_roles` is kept thin and the decisions sit above it.

- `mix.json` round-trips through save and load
- `get_mix` prunes entries whose pack directory is gone
- `get_mix` prunes entries whose file is gone but whose pack remains
- corrupt `mix.json` reads as an empty mix rather than erroring
- `apply_mix` on an empty mix returns an error and writes nothing
- the role→path map for a mix spanning two packs contains both packs' paths

## Build order

1. Phase 1: single applied slot, then thumbnails in the assignment table
2. `write_roles` extraction, verified by the existing pack-apply path
3. Mix commands with tests
4. `src/lib/roles.ts`, with Debug switched over
5. `Mix.tsx` and its nav entry
