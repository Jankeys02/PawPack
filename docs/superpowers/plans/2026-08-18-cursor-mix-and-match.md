# Cursor Mix & Match Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a Mix tab that assembles one cursor set from files across all imported packs and applies it to Windows, plus the Apply tab corrections it depends on.

**Architecture:** A mix is a `role → (pack, file)` map in `packs/mix.json`. Applying it writes absolute paths into `HKCU\Control Panel\Cursors` — nothing is copied, because the registry already stores one absolute path per role. "What is applied" moves from per-card state in `Apply.tsx` into a single slot, which is what lets an applied mix clear every pack card without a second mechanism.

**Tech Stack:** Tauri 2, Rust (edition 2024), React 19, TypeScript 7, Tailwind 4, Base UI, lucide-react.

## Global Constraints

- Rust edition 2024; `cargo test` must pass after every Rust task.
- No new npm or cargo dependencies. Everything here uses what is already installed.
- **There is no JavaScript test framework in this repo, and this plan does not add one.** Rust tasks are TDD. Frontend tasks verify with `npm run build` (which runs `tsc` first) plus the stated manual check. Do not add vitest.
- Base UI, not Radix: composition uses `render={<X />}`, never `asChild`.
- Every pack path built from user input must keep the existing `starts_with(&base)` guard.
- Registry writes go through `write_roles` only, once Task 3 lands.
- Styling follows the existing zinc/amber palette in `src/views/Apply.tsx`.
- Commit after every task.

---

### Task 1: Single applied slot

Fixes the stale "Applied — N cursor roles written" banners. `status` lives in `useState` inside each `PackRow` (`src/views/Apply.tsx:222`), so every pack applied this session keeps its own banner forever. Whether a pack is applied is one fact about the system, so it becomes one slot in the parent.

**Files:**
- Modify: `src/App.tsx:19-26` (type), `src/App.tsx:113-140` (state + handlers), `src/App.tsx:236-246` (props)
- Modify: `src/views/Apply.tsx:48-70` (types), `src/views/Apply.tsx:211-246` (PackRow), `src/views/Apply.tsx:380-482` (Apply)

**Interfaces:**
- Consumes: nothing from earlier tasks
- Produces: `AppliedTarget` and `Applied` exported from `src/App.tsx`; `Apply` prop `applied: Applied | null`, `onApplied(target: AppliedTarget)`, `onReverted()`

- [ ] **Step 1: Replace the ActivePack type in `src/App.tsx`**

Replace lines 19-26 (`interface PackMeta { ... }` stays; the `ActivePack` interface near line 113 is the target). Delete:

```ts
interface ActivePack {
  id: string;
  name: string;
  appliedAt: number; // unix ms
}
```

with:

```ts
/// What is currently applied to the system. Exactly one thing can be.
export type AppliedTarget =
  | { kind: "pack"; id: string; name: string }
  | { kind: "mix" };

export interface Applied {
  target: AppliedTarget;
  appliedAt: number; // unix ms
}
```

- [ ] **Step 2: Replace the state initialiser and handlers in `src/App.tsx`**

Replace:

```ts
  const [activePack, setActivePack] = useState<ActivePack | null>(() => {
    try {
      const raw = localStorage.getItem(ACTIVE_PACK_KEY);
      return raw ? (JSON.parse(raw) as ActivePack) : null;
    } catch {
      return null;
    }
  });

  const handleApplied = (pack: PackMeta) => {
    const next: ActivePack = { id: pack.id, name: pack.name, appliedAt: Date.now() };
    localStorage.setItem(ACTIVE_PACK_KEY, JSON.stringify(next));
    setActivePack(next);
  };

  const handleReverted = () => {
    localStorage.removeItem(ACTIVE_PACK_KEY);
    setActivePack(null);
  };
```

with:

```ts
  const [applied, setApplied] = useState<Applied | null>(readApplied);

  const handleApplied = (target: AppliedTarget) => {
    const next: Applied = { target, appliedAt: Date.now() };
    localStorage.setItem(ACTIVE_PACK_KEY, JSON.stringify(next));
    setApplied(next);
  };

  const handleReverted = () => {
    localStorage.removeItem(ACTIVE_PACK_KEY);
    setApplied(null);
  };
```

- [ ] **Step 3: Add the stored-value reader above `export default function App()` in `src/App.tsx`**

Builds before this one stored `{ id, name, appliedAt }` with no `target`. Read those as packs rather than dropping them.

```ts
function readApplied(): Applied | null {
  try {
    const raw = localStorage.getItem(ACTIVE_PACK_KEY);
    if (!raw) return null;
    const v = JSON.parse(raw);
    if (v && v.target) return v as Applied;
    // Pre-mix builds stored the pack fields flat, with no target.
    if (v && typeof v.id === "string") {
      return {
        target: { kind: "pack", id: v.id, name: v.name ?? v.id },
        appliedAt: typeof v.appliedAt === "number" ? v.appliedAt : Date.now(),
      };
    }
    return null;
  } catch {
    return null;
  }
}
```

- [ ] **Step 4: Update the two consumers in `src/App.tsx`**

Replace the `Apply` and `Debug` render calls:

```tsx
        ) : active === "apply" ? (
          <Apply
            applied={applied}
            onApplied={handleApplied}
            onReverted={handleReverted}
          />
        ) : active === "debug" ? (
          <Debug applied={applied} />
        ) : (
```

- [ ] **Step 5: Update `Debug`'s prop in `src/views/Debug.tsx`**

Replace the `Props` interface and the two `activePack` reads in the effect:

```tsx
interface Props {
  applied?: { target: { kind: "pack"; id: string } | { kind: "mix" } } | null;
}

export default function Debug({ applied }: Props) {
  const [customCursors, setCustomCursors] = useState<Record<string, string>>({});

  useEffect(() => {
    const packId = applied?.target.kind === "pack" ? applied.target.id : null;
    if (!packId) { setCustomCursors({}); return; }
```

Inside the effect body, replace every `activePack!.id` with `packId`, and change the dependency array from `[activePack]` to `[applied]`.

Task 7 extends this to resolve cursors for `{kind:"mix"}`; until then a mix shows no custom cursors in Debug, which is correct — there is no mix yet.

- [ ] **Step 6: Replace the `CardStatus` type in `src/views/Apply.tsx`**

Only transient states stay per-card. `applied` is no longer one of them.

```ts
type CardStatus =
  | { kind: "idle" }
  | { kind: "applying" }
  | { kind: "reverting" }
  | { kind: "reverted" }
  | { kind: "error"; message: string };
```

- [ ] **Step 7: Rewrite `PackRow`'s signature and handlers in `src/views/Apply.tsx`**

```tsx
function PackRow({
  pack,
  isActive,
  appliedResult,
  onApplied,
  onReverted,
}: {
  pack: PackMeta;
  isActive: boolean;
  /** Non-null only when this pack is the applied target. */
  appliedResult: PackAssignmentResult | null;
  onApplied: (result: PackAssignmentResult) => void;
  onReverted: () => void;
}) {
  const [status, setStatus] = useState<CardStatus>({ kind: "idle" });

  const busy = status.kind === "applying" || status.kind === "reverting";

  const handleApply = async () => {
    setStatus({ kind: "applying" });
    try {
      const result = await invoke<PackAssignmentResult>("apply_pack", { packId: pack.id });
      setStatus({ kind: "idle" });
      onApplied(result);
    } catch (e) {
      setStatus({ kind: "error", message: String(e) });
    }
  };

  const handleRevert = async () => {
    setStatus({ kind: "reverting" });
    try {
      await invoke("revert_pack", { packId: pack.id });
      setStatus({ kind: "reverted" });
      onReverted();
    } catch (e) {
      setStatus({ kind: "error", message: String(e) });
    }
  };

  const showAssignments = isActive || appliedResult !== null;
```

- [ ] **Step 8: Update `PackRow`'s border and banner in `src/views/Apply.tsx`**

Replace the border expression:

```tsx
    <div className={cn(
      "flex flex-col gap-3 rounded-sm border bg-zinc-900 p-4 transition-colors",
      isActive ? "border-amber-500/40 bg-amber-500/5" :
      status.kind === "reverted" ? "border-zinc-700" :
      status.kind === "error" ? "border-red-500/30" :
      "border-zinc-800",
    )}>
```

Replace the applied banner block (currently `{status.kind === "applied" && (...)}`) with one driven by the prop:

```tsx
      {appliedResult && (
        <div className="flex items-center gap-2 rounded border border-amber-500/20 bg-amber-500/10 px-3 py-2 text-xs text-amber-400">
          <CheckCircle2 className="h-3.5 w-3.5 shrink-0" strokeWidth={1.75} />
          Applied — {appliedResult.assigned.length} cursor role{appliedResult.assigned.length !== 1 ? "s" : ""} written.
          Cursors updated immediately.
        </div>
      )}
```

And change the `AssignmentsDropdown` prop from `prefetched={status.kind === "applied" ? status.result : null}` to `prefetched={appliedResult}`.

- [ ] **Step 9: Hold the slot in `Apply` in `src/views/Apply.tsx`**

Replace the component signature and add the slot:

```tsx
export default function Apply({
  applied,
  onApplied,
  onReverted,
}: {
  applied: Applied | null;
  onApplied: (target: AppliedTarget) => void;
  onReverted: () => void;
}) {
  const [packs, setPacks] = useState<PackMeta[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  /** Result of the apply performed in this session, if any. */
  const [appliedResult, setAppliedResult] = useState<
    { packId: string; result: PackAssignmentResult } | null
  >(null);
```

Add the import at the top of the file:

```ts
import type { Applied, AppliedTarget } from "@/App";
```

- [ ] **Step 10: Update the header and the card list in `src/views/Apply.tsx`**

Header, replacing the `activePack ? (...)` branch:

```tsx
          {applied ? (
            <p className="text-xs text-zinc-500">
              <span className="text-amber-400">
                {applied.target.kind === "pack" ? applied.target.name : "Custom mix"}
              </span>
              {" "}applied{" "}
              <span className="text-zinc-600">
                {new Date(applied.appliedAt).toLocaleTimeString(undefined, { hour: "2-digit", minute: "2-digit" })}
              </span>
            </p>
          ) : !loading && (
```

Card list:

```tsx
            {packs.map((pack) => (
              <PackRow
                key={pack.id}
                pack={pack}
                isActive={applied?.target.kind === "pack" && applied.target.id === pack.id}
                appliedResult={appliedResult?.packId === pack.id ? appliedResult.result : null}
                onApplied={(result) => {
                  setAppliedResult({ packId: pack.id, result });
                  onApplied({ kind: "pack", id: pack.id, name: pack.name });
                }}
                onReverted={() => {
                  setAppliedResult(null);
                  onReverted();
                }}
              />
            ))}
```

Because `appliedResult` is a single slot, applying pack B replaces pack A's entry and A's banner disappears with no explicit clearing.

- [ ] **Step 11: Build**

Run: `npm run build`
Expected: PASS, no TypeScript errors.

- [ ] **Step 12: Manual check**

Run: `npm run tauri dev`
Apply Bog, then apply Brush Buddy. Expected: only Brush Buddy shows the ACTIVE badge and the "Applied — N cursor roles written" banner. Bog's banner is gone. Then click Revert on Brush Buddy: its banner clears and no other card claims to be applied.

- [ ] **Step 13: Commit**

```bash
git add src/App.tsx src/views/Apply.tsx src/views/Debug.tsx
git commit -m "fix: track one applied target instead of per-card state

Each PackRow held its own applied status, so every pack applied this
session kept a stale banner. Whether something is applied is one fact
about the system, so it becomes one slot in the parent and every other
banner clears on its own. Also fixes revert leaving sibling banners up."
```

---

### Task 2: Thumbnails in the assignment table

The expanded table lists filenames from `get_pack_assignments`, which carries no images. `list_pack_cursors` already returns a base64 PNG per file.

**Files:**
- Modify: `src/views/Apply.tsx:92-208` (`AssignmentsDropdown`)

**Interfaces:**
- Consumes: `PackRow` from Task 1 passes `prefetched={appliedResult}`
- Produces: nothing new; Task 7 reuses the same `list_pack_cursors` + join-on-filename shape

- [ ] **Step 1: Add the `CursorEntry` type near the other types in `src/views/Apply.tsx`**

```ts
interface CursorEntry {
  name: string;
  kind: string;
  thumbnail: string; // base64 PNG, empty string when decoding failed
}
```

- [ ] **Step 2: Fetch thumbnails when the dropdown opens in `src/views/Apply.tsx`**

Add the state beside the existing `useState` calls in `AssignmentsDropdown`:

```ts
  const [thumbs, setThumbs] = useState<Record<string, string>>({});
```

Extend `toggle` so the images load with the assignments:

```ts
  const toggle = async () => {
    if (!open && result === null) {
      setLoading(true);
      try {
        setResult(await invoke<PackAssignmentResult>("get_pack_assignments", { packId }));
      } catch {
        setResult({ assigned: [], unmatched: [] });
      } finally {
        setLoading(false);
      }
    }
    if (!open && Object.keys(thumbs).length === 0) {
      try {
        const cursors = await invoke<CursorEntry[]>("list_pack_cursors", { packId });
        setThumbs(Object.fromEntries(cursors.map((c) => [c.name, c.thumbnail])));
      } catch {
        // Leave thumbs empty; rows fall back to the filename alone.
      }
    }
    setOpen((v) => !v);
  };
```

- [ ] **Step 3: Add the thumbnail cell to the table in `src/views/Apply.tsx`**

In the table header row, add a leading empty header before the `FILE` header:

```tsx
                  <th className="w-8" />
```

In the body row, add this as the first cell of each row, before the filename cell:

```tsx
                  <td className="py-1 pl-1">
                    {thumbs[row.file] ? (
                      <img
                        src={`data:image/png;base64,${thumbs[row.file]}`}
                        alt=""
                        className="h-5 w-5 object-contain"
                        style={{ imageRendering: "pixelated" }}
                      />
                    ) : (
                      <span className="block h-5 w-5 rounded-sm bg-zinc-800/60" />
                    )}
                  </td>
```

- [ ] **Step 4: Build**

Run: `npm run build`
Expected: PASS.

- [ ] **Step 5: Manual check**

Run: `npm run tauri dev`
Expand a pack's assignment list. Expected: each row shows the cursor image beside its filename; rows whose thumbnail failed to decode show a grey placeholder square rather than a broken image.

- [ ] **Step 6: Commit**

```bash
git add src/views/Apply.tsx
git commit -m "feat: show cursor thumbnails in the assignment table"
```

---

### Task 3: Extract `write_roles`

`apply` both computes assignments and writes the registry. `apply_mix` needs the second half with a different source for the first, so one function must own the registry write.

**Files:**
- Modify: `src-tauri/src/lib.rs:868-894` (`windows_cursor::apply`)

**Interfaces:**
- Consumes: `CURSOR_REG_NAMES`, `call_spi_set_cursors` (both already in `windows_cursor`)
- Produces: `windows_cursor::write_roles(paths: &HashMap<&str, PathBuf>) -> Result<(), String>`; `windows_cursor::CURSOR_REG_NAMES` becomes `pub`

- [ ] **Step 1: Make `CURSOR_REG_NAMES` public in `src-tauri/src/lib.rs`**

Task 5 needs it to report which roles a mix clears.

```rust
    pub const CURSOR_REG_NAMES: &[&str] = &[
```

- [ ] **Step 2: Add `write_roles` immediately above `pub fn apply` in `src-tauri/src/lib.rs`**

```rust
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
```

- [ ] **Step 3: Replace the body of `pub fn apply` in `src-tauri/src/lib.rs`**

```rust
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
```

- [ ] **Step 4: Verify the existing tests still pass**

Run: `cd src-tauri && cargo test`
Expected: PASS, 14 tests. This is a pure refactor — no test should change.

- [ ] **Step 5: Manual check**

Run: `npm run tauri dev`
Apply Bog. Expected: still reports 16 cursor roles written, and the cursors change.

- [ ] **Step 6: Commit**

```bash
git add src-tauri/src/lib.rs
git commit -m "refactor: route every registry write through write_roles

Applying a pack and applying a mix need the same registry write with a
different source for the role map, so one function owns it."
```

---

### Task 4: Mix data model

Platform-neutral on purpose — only *applying* a mix is Windows-specific, so these live at crate level and their tests run anywhere.

**Files:**
- Modify: `src-tauri/src/lib.rs` (types near `PackAssignmentResult`, helpers near `load_overrides`, tests at the end)

**Interfaces:**
- Consumes: nothing from earlier tasks
- Produces: `MixEntry { role, pack, file }`, `MixResult { roles, stale }`, `read_mix(base: &Path) -> MixResult`, `write_mix_role(base: &Path, role: &str, pack: &str, file: &str) -> Result<MixResult, String>`

- [ ] **Step 1: Write the failing tests at the end of the `mod tests` block in `src-tauri/src/lib.rs`**

Add inside `mod tests`, after the `role_match` module:

```rust
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
    }
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cd src-tauri && cargo test mix`
Expected: FAIL to compile with `cannot find function read_mix in crate root`.

- [ ] **Step 3: Add the mix types after `PackAssignmentResult` in `src-tauri/src/lib.rs`**

```rust
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
```

- [ ] **Step 4: Add the mix helpers after `save_overrides` in `src-tauri/src/lib.rs`**

```rust
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
        if packs_base.join(&entry.pack).join(&entry.file).is_file() {
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
```

- [ ] **Step 5: Run the tests to verify they pass**

Run: `cd src-tauri && cargo test mix`
Expected: PASS, 6 tests.

- [ ] **Step 6: Run the full suite**

Run: `cd src-tauri && cargo test`
Expected: PASS, 20 tests.

- [ ] **Step 7: Commit**

```bash
git add src-tauri/src/lib.rs
git commit -m "feat: add mix.json data model

A mix maps each role to a (pack, file) pair. Stale entries are reported
but left on disk so reimporting a pack under the same id restores them."
```

---

### Task 5: Mix commands

**Files:**
- Modify: `src-tauri/src/lib.rs` (commands near `apply_pack`, handler list at `run()`)

**Interfaces:**
- Consumes: `read_mix`, `write_mix_role` (Task 4); `windows_cursor::write_roles`, `windows_cursor::CURSOR_REG_NAMES` (Task 3); `windows_cursor::snapshot_path`, `windows_cursor::CursorSnapshot`
- Produces: commands `get_mix`, `set_mix_role`, `apply_mix`; `ApplyMixResult { written: usize, cleared: Vec<String> }`

- [ ] **Step 1: Write the failing test for the empty-mix guard, inside `mod mix` in `src-tauri/src/lib.rs`**

The registry write needs a real registry, so the guard is what gets tested: an empty mix must produce an error before anything is written.

```rust
        #[test]
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
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cd src-tauri && cargo test mix::`
Expected: FAIL to compile with `cannot find function mix_paths_for_apply`.

- [ ] **Step 3: Add `mix_paths_for_apply` above the commands in `src-tauri/src/lib.rs`**

Splitting the decision out from the registry write is what makes it testable.

```rust
/// Resolve the mix into absolute paths, plus the roles that will be cleared.
///
/// Errors on an empty mix rather than clearing all 17 roles, which is what a
/// half-built mix would otherwise do the first time Apply is pressed.
#[cfg(target_os = "windows")]
fn mix_paths_for_apply(
    packs_base: &Path,
) -> Result<(HashMap<String, PathBuf>, Vec<String>), String> {
    let mix = read_mix(packs_base);
    if mix.roles.is_empty() {
        return Err("Mix is empty — assign at least one cursor before applying".into());
    }

    let paths: HashMap<String, PathBuf> = mix
        .roles
        .iter()
        .map(|e| (e.role.clone(), packs_base.join(&e.pack).join(&e.file)))
        .collect();

    let cleared: Vec<String> = windows_cursor::CURSOR_REG_NAMES
        .iter()
        .filter(|r| !paths.contains_key(**r))
        .map(|r| r.to_string())
        .collect();

    Ok((paths, cleared))
}
```

Mark the test `#[cfg(target_os = "windows")]` too, directly above `fn empty_mix_is_refused_before_any_write`.

- [ ] **Step 4: Run the test to verify it passes**

Run: `cd src-tauri && cargo test mix::`
Expected: PASS, 8 tests.

- [ ] **Step 5: Extract the snapshot guard in `src-tauri/src/lib.rs`**

`apply_pack` and `apply_mix` need identical snapshot behaviour. Add above `apply_pack`:

```rust
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
```

Then replace the snapshot block inside `apply_pack` with `ensure_snapshot(&base)?;`, keeping the surrounding validation untouched.

- [ ] **Step 6: Add the three commands after `apply_pack` in `src-tauri/src/lib.rs`**

```rust
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
        let pack_dir = base.join(&pack_id);
        if !pack_dir.starts_with(&base) {
            return Err("Invalid pack id".into());
        }
        if !pack_dir.join(&file).is_file() {
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
```

- [ ] **Step 7: Register the commands in `run()` in `src-tauri/src/lib.rs`**

Add to the `tauri::generate_handler!` list, after `revert_pack`:

```rust
            revert_pack,
            get_mix,
            set_mix_role,
            apply_mix
```

- [ ] **Step 8: Run the full suite**

Run: `cd src-tauri && cargo test`
Expected: PASS, 22 tests.

- [ ] **Step 9: Commit**

```bash
git add src-tauri/src/lib.rs
git commit -m "feat: add get_mix, set_mix_role and apply_mix commands

apply_mix shares the revert snapshot guard with apply_pack, so reverting
a mix returns to the user's own cursors exactly as reverting a pack does."
```

---

### Task 6: Shared role list

`Debug.tsx` hardcodes the 17 roles as `ZONES`; `Apply.tsx` hardcodes them again as `ROLE_LABELS`. Task 7 needs them a third time.

**Files:**
- Create: `src/lib/roles.ts`
- Modify: `src/views/Debug.tsx:19-49`, `src/views/Apply.tsx:28-46`

**Interfaces:**
- Consumes: nothing from earlier tasks
- Produces: `CURSOR_ROLES: CursorRole[]`, `ROLE_LABELS: Record<string, string>` from `@/lib/roles`

- [ ] **Step 1: Create `src/lib/roles.ts`**

```ts
import {
  ArrowUp, MoveHorizontal, MoveVertical, Move, Type, MousePointer2, Hand,
  Ban, HelpCircle, Crosshair, Clock, Loader, MoveUpRight, MoveDownRight,
  PenLine, MapPin, UserRound,
} from "lucide-react";

export interface CursorRole {
  /** Registry value name under HKCU\Control Panel\Cursors. */
  reg: string;
  /** Windows' own name for this cursor, as shown in Mouse Properties. */
  label: string;
  Icon: React.ComponentType<{ className?: string; strokeWidth?: number }>;
  color: string;
}

/// The 17 roles, in the order Windows lists them in Mouse Properties.
export const CURSOR_ROLES: CursorRole[] = [
  { reg: "Arrow",       label: "Normal select",        Icon: MousePointer2,  color: "text-zinc-400"    },
  { reg: "Help",        label: "Help select",          Icon: HelpCircle,     color: "text-teal-400"    },
  { reg: "AppStarting", label: "Working in background", Icon: Loader,        color: "text-yellow-400"  },
  { reg: "Wait",        label: "Busy",                 Icon: Clock,          color: "text-orange-400"  },
  { reg: "Crosshair",   label: "Precision select",     Icon: Crosshair,      color: "text-emerald-400" },
  { reg: "IBeam",       label: "Text select",          Icon: Type,           color: "text-sky-400"     },
  { reg: "NWPen",       label: "Handwriting",          Icon: PenLine,        color: "text-fuchsia-400" },
  { reg: "No",          label: "Unavailable",          Icon: Ban,            color: "text-red-400"     },
  { reg: "SizeNS",      label: "Vertical resize",      Icon: MoveVertical,   color: "text-cyan-400"    },
  { reg: "SizeWE",      label: "Horizontal resize",    Icon: MoveHorizontal, color: "text-cyan-400"    },
  { reg: "SizeNWSE",    label: "Diagonal resize ↖↘",   Icon: MoveDownRight,  color: "text-indigo-400"  },
  { reg: "SizeNESW",    label: "Diagonal resize ↗↙",   Icon: MoveUpRight,    color: "text-indigo-400"  },
  { reg: "SizeAll",     label: "Move",                 Icon: Move,           color: "text-violet-400"  },
  { reg: "UpArrow",     label: "Alternate select",     Icon: ArrowUp,        color: "text-lime-400"    },
  { reg: "Hand",        label: "Link select",          Icon: Hand,           color: "text-amber-400"   },
  { reg: "Pin",         label: "Location select",      Icon: MapPin,         color: "text-rose-400"    },
  { reg: "Person",      label: "Person select",        Icon: UserRound,      color: "text-sky-300"     },
];

export const ROLE_LABELS: Record<string, string> = Object.fromEntries(
  CURSOR_ROLES.map((r) => [r.reg, r.label]),
);
```

- [ ] **Step 2: Switch `src/views/Apply.tsx` to the shared list**

Delete the local `ROLE_LABELS` object (lines 28-46) and add to the imports:

```ts
import { ROLE_LABELS } from "@/lib/roles";
```

`ALL_ROLES` keeps working unchanged, since it derives from `ROLE_LABELS`.

- [ ] **Step 3: Switch `src/views/Debug.tsx` to the shared list**

Delete the `Zone` interface and the `ZONES` array. Replace with a CSS lookup keyed by registry name — the CSS fallbacks are Debug-only and stay here:

```ts
import { CURSOR_ROLES } from "@/lib/roles";

/// CSS cursor keyword per role, used as the fallback when no pack is applied.
const ROLE_CSS: Record<string, string> = {
  Arrow: "default", Hand: "pointer", IBeam: "text", Wait: "wait",
  AppStarting: "progress", Crosshair: "crosshair", SizeAll: "move",
  No: "not-allowed", Help: "help", SizeNS: "ns-resize", SizeWE: "ew-resize",
  SizeNWSE: "nwse-resize", SizeNESW: "nesw-resize", UpArrow: "cell",
  NWPen: "alias", Pin: "copy", Person: "alias",
};
```

Replace every `ZONES.map((zone) => ...)` with `CURSOR_ROLES.map((role) => ...)`, and inside those bodies replace `zone.reg` with `role.reg`, `zone.label` with `role.label`, `zone.Icon` with `role.Icon`, `zone.color` with `role.color`, and `zone.css` with `ROLE_CSS[role.reg]`.

Debug's labels change slightly as a result — "Hand / Link" becomes "Link select", matching Windows' own naming and the Apply tab.

- [ ] **Step 4: Build**

Run: `npm run build`
Expected: PASS.

- [ ] **Step 5: Manual check**

Run: `npm run tauri dev`
Open Debug. Expected: all 17 zones still render, each with its icon and colour, and hovering still shows the applied pack's cursor.

- [ ] **Step 6: Commit**

```bash
git add src/lib/roles.ts src/views/Debug.tsx src/views/Apply.tsx
git commit -m "refactor: share the 17 cursor roles from src/lib/roles.ts"
```

---

### Task 7: Mix tab

**Files:**
- Create: `src/views/Mix.tsx`
- Modify: `src/App.tsx` (nav entry, render branch), `src/views/Debug.tsx` (resolve mix cursors)

**Interfaces:**
- Consumes: `CURSOR_ROLES` (Task 6); `get_mix`, `set_mix_role`, `apply_mix` (Task 5); `Applied`, `AppliedTarget` (Task 1)
- Produces: default-exported `Mix` component taking `{ applied, onApplied }`

- [ ] **Step 1: Create `src/views/Mix.tsx`**

```tsx
import { useState, useEffect, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import { RefreshCw, AlertCircle, CheckCircle2, Play, Shuffle } from "lucide-react";
import { cn } from "@/lib/utils";
import { CURSOR_ROLES } from "@/lib/roles";
import type { AppliedTarget, Applied } from "@/App";

interface PackMeta { id: string; name: string; cursor_count: number }
interface CursorEntry { name: string; kind: string; thumbnail: string }
interface MixEntry { role: string; pack: string; file: string }
interface MixResult { roles: MixEntry[]; stale: MixEntry[] }
interface ApplyMixResult { written: number; cleared: string[] }
interface CursorAssignment { role: string; file: string }
interface PackAssignmentResult { assigned: CursorAssignment[]; unmatched: string[] }

/** All cursors of one pack, ready for the gallery. */
interface PackCursors { pack: PackMeta; cursors: CursorEntry[] }

export default function Mix({
  applied,
  onApplied,
}: {
  applied: Applied | null;
  onApplied: (target: AppliedTarget) => void;
}) {
  const [mix, setMix] = useState<MixResult>({ roles: [], stale: [] });
  const [library, setLibrary] = useState<PackCursors[]>([]);
  const [selectedRole, setSelectedRole] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);
  const [applying, setApplying] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [result, setResult] = useState<ApplyMixResult | null>(null);

  const load = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const packs = await invoke<PackMeta[]>("list_packs");
      const withCursors = await Promise.all(
        packs.map(async (pack) => ({
          pack,
          cursors: await invoke<CursorEntry[]>("list_pack_cursors", { packId: pack.id }),
        })),
      );
      setLibrary(withCursors);
      setMix(await invoke<MixResult>("get_mix"));
    } catch (e) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => { load(); }, [load]);

  const byRole = Object.fromEntries(mix.roles.map((e) => [e.role, e]));
  const filled = mix.roles.length;

  const thumbFor = (entry: MixEntry | undefined) => {
    if (!entry) return null;
    const pack = library.find((p) => p.pack.id === entry.pack);
    return pack?.cursors.find((c) => c.name === entry.file)?.thumbnail ?? null;
  };

  const assign = async (packId: string, file: string) => {
    if (!selectedRole) return;
    try {
      setMix(await invoke<MixResult>("set_mix_role", { role: selectedRole, packId, file }));
      setResult(null);
    } catch (e) {
      setError(String(e));
    }
  };

  const clearRole = async (role: string) => {
    try {
      setMix(await invoke<MixResult>("set_mix_role", { role, packId: "", file: "" }));
      setResult(null);
    } catch (e) {
      setError(String(e));
    }
  };

  /** Copy in the roles this pack assigns, leaving the rest of the mix alone. */
  const fillFromPack = async (packId: string) => {
    try {
      const assignments = await invoke<PackAssignmentResult>("get_pack_assignments", { packId });
      let latest = mix;
      for (const a of assignments.assigned) {
        latest = await invoke<MixResult>("set_mix_role", {
          role: a.role, packId, file: a.file,
        });
      }
      setMix(latest);
      setResult(null);
    } catch (e) {
      setError(String(e));
    }
  };

  const handleApply = async () => {
    setApplying(true);
    setError(null);
    try {
      setResult(await invoke<ApplyMixResult>("apply_mix"));
      onApplied({ kind: "mix" });
    } catch (e) {
      setError(String(e));
    } finally {
      setApplying(false);
    }
  };

  const isActive = applied?.target.kind === "mix";

  return (
    <div className="flex flex-1 flex-col overflow-hidden">
      {/* Header */}
      <div className="flex items-center justify-between border-b border-zinc-800/60 px-6 py-4">
        <div>
          <h1 className="flex items-center gap-2 text-sm font-semibold text-zinc-100">
            Mix &amp; Match
            {isActive && (
              <span className="inline-flex items-center gap-1 rounded border border-amber-500/30 bg-amber-500/10 px-1.5 py-px font-mono text-[10px] uppercase tracking-wide text-amber-400">
                <span className="h-1.5 w-1.5 rounded-full bg-amber-400" />
                Active
              </span>
            )}
          </h1>
          <p className="text-xs text-zinc-500">
            {filled} / {CURSOR_ROLES.length} roles filled
            {filled > 0 && filled < CURSOR_ROLES.length && (
              <span className="text-zinc-600">
                {" "}— {CURSOR_ROLES.length - filled} will reset to Windows defaults
              </span>
            )}
          </p>
        </div>
        <div className="flex items-center gap-2">
          <select
            value=""
            onChange={(e) => { if (e.target.value) fillFromPack(e.target.value); }}
            className="h-8 rounded border border-zinc-700 bg-zinc-800 px-2 text-xs text-zinc-300"
          >
            <option value="">Fill from pack…</option>
            {library.map((p) => (
              <option key={p.pack.id} value={p.pack.id}>{p.pack.name}</option>
            ))}
          </select>
          <button
            onClick={handleApply}
            disabled={applying || filled === 0}
            className="flex h-8 items-center gap-1.5 rounded bg-amber-500 px-3 text-xs font-medium text-zinc-950 transition-colors hover:bg-amber-400 disabled:opacity-40"
          >
            <Play className="h-3 w-3" strokeWidth={2.5} />
            {applying ? "Applying…" : "Apply Mix"}
          </button>
          <button
            onClick={load}
            disabled={loading}
            className="flex h-8 w-8 items-center justify-center rounded text-zinc-600 transition-colors hover:bg-zinc-800 hover:text-zinc-400 disabled:opacity-40"
          >
            <RefreshCw className={cn("h-3.5 w-3.5", loading && "animate-spin")} strokeWidth={1.75} />
          </button>
        </div>
      </div>

      {error && (
        <div className="mx-6 mt-3 flex items-start gap-2 rounded border border-red-500/20 bg-red-500/10 px-3 py-2.5 text-xs text-red-400">
          <AlertCircle className="mt-px h-3.5 w-3.5 shrink-0" strokeWidth={1.75} />
          <span className="break-all">{error}</span>
          <button onClick={() => setError(null)} className="ml-auto shrink-0 hover:text-red-300">✕</button>
        </div>
      )}

      {result && (
        <div className="mx-6 mt-3 flex items-center gap-2 rounded border border-amber-500/20 bg-amber-500/10 px-3 py-2 text-xs text-amber-400">
          <CheckCircle2 className="h-3.5 w-3.5 shrink-0" strokeWidth={1.75} />
          Applied — {result.written} cursor role{result.written !== 1 ? "s" : ""} written,
          {" "}{result.cleared.length} reset to Windows defaults.
        </div>
      )}

      {mix.stale.length > 0 && (
        <div className="mx-6 mt-3 rounded border border-zinc-700/50 bg-zinc-800/40 px-3 py-2 text-xs text-zinc-500">
          {mix.stale.length} cursor{mix.stale.length !== 1 ? "s" : ""} unavailable — pack deleted.
        </div>
      )}

      {/* Two panels */}
      <div className="flex flex-1 gap-4 overflow-hidden p-6">
        {/* Roles */}
        <div className="flex w-[280px] shrink-0 flex-col gap-1 overflow-y-auto">
          {CURSOR_ROLES.map((role) => {
            const entry = byRole[role.reg];
            const thumb = thumbFor(entry);
            const selected = selectedRole === role.reg;
            return (
              <button
                key={role.reg}
                onClick={() => setSelectedRole(role.reg)}
                className={cn(
                  "flex items-center gap-2.5 rounded-sm border px-2.5 py-2 text-left transition-colors",
                  selected
                    ? "border-amber-500/40 bg-amber-500/10"
                    : "border-zinc-800 bg-zinc-900 hover:border-zinc-700",
                )}
              >
                <div className="flex h-7 w-7 shrink-0 items-center justify-center rounded bg-zinc-950/60">
                  {thumb ? (
                    <img
                      src={`data:image/png;base64,${thumb}`}
                      alt=""
                      className="h-5 w-5 object-contain"
                      style={{ imageRendering: "pixelated" }}
                    />
                  ) : (
                    <role.Icon className={cn("h-3.5 w-3.5", role.color)} strokeWidth={1.75} />
                  )}
                </div>
                <div className="min-w-0 flex-1">
                  <p className="truncate text-xs font-medium text-zinc-200">{role.label}</p>
                  <p className="truncate font-mono text-[10px] text-zinc-600">
                    {entry ? entry.file : "empty"}
                  </p>
                </div>
                {entry && (
                  <span
                    role="button"
                    tabIndex={0}
                    onClick={(e) => { e.stopPropagation(); clearRole(role.reg); }}
                    onKeyDown={(e) => { if (e.key === "Enter") { e.stopPropagation(); clearRole(role.reg); } }}
                    className="shrink-0 px-1 text-zinc-600 hover:text-zinc-300"
                  >
                    ✕
                  </span>
                )}
              </button>
            );
          })}
        </div>

        {/* Gallery */}
        <div className="flex-1 overflow-y-auto rounded-sm border border-zinc-800 bg-zinc-900/40 p-4">
          {!selectedRole ? (
            <div className="flex h-full flex-col items-center justify-center gap-3 text-zinc-600">
              <Shuffle className="h-6 w-6" strokeWidth={1.5} />
              <p className="text-sm">Select a role first</p>
            </div>
          ) : (
            library.map((p) => (
              <div key={p.pack.id} className="mb-5">
                <p className="mb-2 font-mono text-[10px] uppercase tracking-[0.14em] text-zinc-600">
                  {p.pack.name}
                </p>
                <div className="flex flex-wrap gap-2">
                  {p.cursors.map((c) => (
                    <button
                      key={c.name}
                      onClick={() => assign(p.pack.id, c.name)}
                      title={c.name}
                      className="flex h-14 w-14 items-center justify-center rounded border border-zinc-800 bg-zinc-950/60 transition-colors hover:border-amber-500/40"
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
                    </button>
                  ))}
                </div>
              </div>
            ))
          )}
        </div>
      </div>
    </div>
  );
}
```

- [ ] **Step 2: Add the nav entry in `src/App.tsx`**

Add `Shuffle` to the lucide import, extend the `NavId` union and `mainNav`:

```ts
type NavId = "browse" | "editor" | "apply" | "mix" | "debug" | "settings";

const mainNav: NavEntry[] = [
  { id: "browse",  label: "Browse",  Icon: Layers         },
  { id: "editor",  label: "Editor",  Icon: Pencil         },
  { id: "apply",   label: "Apply",   Icon: MousePointer2  },
  { id: "mix",     label: "Mix",     Icon: Shuffle        },
  { id: "debug",   label: "Debug",   Icon: FlaskConical   },
];
```

- [ ] **Step 3: Render the tab in `src/App.tsx`**

Add the import and the branch, between the `apply` and `debug` branches:

```tsx
import Mix from "@/views/Mix";
```

```tsx
        ) : active === "mix" ? (
          <Mix applied={applied} onApplied={handleApplied} />
        ) : active === "debug" ? (
```

- [ ] **Step 4: Resolve mix cursors in `src/views/Debug.tsx`**

Replace the effect's role→file lookup so a mix resolves too. Inside `load()`, replace the `get_pack_assignments` call and the `roleToFile` construction with:

```ts
        // role → [packId, file], from whichever thing is applied.
        let roleToSource: Record<string, [string, string]> = {};
        if (applied?.target.kind === "mix") {
          const mix = await invoke<{ roles: { role: string; pack: string; file: string }[] }>("get_mix");
          roleToSource = Object.fromEntries(mix.roles.map((e) => [e.role, [e.pack, e.file]]));
        } else if (applied?.target.kind === "pack") {
          const packId = applied.target.id;
          const result = await invoke<PackAssignmentResult>("get_pack_assignments", { packId });
          roleToSource = Object.fromEntries(result.assigned.map((a) => [a.role, [packId, a.file]]));
        }
```

Then in the loop over `NO_CSS_ROLES`, replace `const file = roleToFile[role]` and the two `packId: activePack!.id` arguments with:

```ts
          const source = roleToSource[role];
          if (!source) continue;
          const [sourcePack, file] = source;
```

passing `packId: sourcePack` to both `parse_ani` and `parse_cur`.

Also change the early return at the top of the effect from `if (!packId)` to `if (!applied) { setCustomCursors({}); return; }`.

- [ ] **Step 5: Build**

Run: `npm run build`
Expected: PASS.

- [ ] **Step 6: Manual check**

Run: `npm run tauri dev`

1. Open Mix. Expected: 17 empty roles on the left, "Select a role first" on the right.
2. Click "Link select", then click a cursor from PHM. Expected: that role shows the thumbnail and filename.
3. Click "Fill from pack…" → Bog. Expected: Bog's 16 roles fill, and Link select now shows Bog's `Hand.cur`, not the PHM cursor — seeding overwrites the roles the pack assigns. IBeam stays empty, because Bog assigns no text cursor.
4. Press Apply Mix. Expected: banner reports roles written and cleared; the Apply tab now shows no pack as ACTIVE; the header on Apply reads "Custom mix applied".
5. Go to Apply and press Revert on any pack. Expected: cursors return to the snapshot, and Mix loses its ACTIVE badge.

- [ ] **Step 7: Commit**

```bash
git add src/views/Mix.tsx src/App.tsx src/views/Debug.tsx
git commit -m "feat: add Mix tab for building a cursor set across packs

Applying a mix sets the applied slot to {kind:'mix'}, so every pack card
clears without a second mechanism."
```

---

## Verification

After Task 7:

```bash
cd src-tauri && cargo test && cd .. && npm run build && npm run tauri build
```

Expected: 22 Rust tests pass, TypeScript compiles, installers build.
