# PawPack Todo

## Phase 1 — Foundation

- [x] Install Tailwind CSS v4 + shadcn/ui
- [x] App shell: sidebar nav + main content layout
- [x] Fix app name / window config (`pawpack-tmp` → `PawPack`, bump window size)
- [x] Clean up boilerplate (greet command, demo UI)

## Phase 2 — Pack Storage

- [x] Rust commands: `list_packs`, `import_pack`, `delete_pack` — [src-tauri/src/lib.rs](src-tauri/src/lib.rs)
- [x] Packs stored in `app_data_dir/packs/` with `pack.json` manifest
- [x] Auto-detect Windows (`.cur` / `.ani`) and Linux (Xcursor) pack formats
- [x] Parse `install.inf` / `index.theme` for name, author, description
- [x] Zip import: extract to temp, handle wrapped folder, clean up — uses `zip` crate in [Cargo.toml](src-tauri/Cargo.toml)
- [x] Browse view: pack grid, import dropdown (`.zip` + folder), delete with confirm — [src/views/Browse.tsx](src/views/Browse.tsx)

## Phase 3 — Cursor Preview

- [x] **Parse `.cur` binary format in Rust** ([src-tauri/src/lib.rs](src-tauri/src/lib.rs))
  - `.cur` is an ICO-format file; the `ICONDIRENTRY` header holds hotspot `xHotspot`/`yHotspot` as two `uint16` fields — [ICO/CUR format spec](https://en.wikipedia.org/wiki/ICO_(file_format))
  - Use the [`ico` crate](https://docs.rs/ico) or the [`image` crate](https://docs.rs/image) (has ICO support) to decode pixel data; add to [Cargo.toml](src-tauri/Cargo.toml)
- [x] **Parse `.ani` binary format in Rust** ([src-tauri/src/lib.rs](src-tauri/src/lib.rs))
  - `.ani` is RIFF-ACON: `anih` chunk (frame count, rate, flags), `LIST fram` (each frame is a `.cur`/`.ico`), optional `rate` chunk (per-frame delays) — [ANI format reference](https://www.gdgsoft.com/anituner/help/aniformat.htm)
  - Use the [`riff` crate](https://docs.rs/riff) to walk chunks; decode each frame as ICO
- [ ] **Render cursor thumbnails on Browse pack cards** ([src/views/Browse.tsx](src/views/Browse.tsx))
  - Add a Tauri command `get_cursor_thumbnail(pack_id, cursor_name) -> String` that returns a Base64 PNG
  - Display as `<img src={`data:image/png;base64,${b64}`}>` inside `PackCard`
- [ ] **Pack detail view** — click a card to see all cursors in the pack
  - New view `src/views/PackDetail.tsx`; add navigation state to [src/App.tsx](src/App.tsx)
  - List all `.cur` / `.ani` files in `packs/<id>/`; show thumbnail grid
- [ ] **Live cursor preview** — hover a preview area to see the actual rendered cursor
  - Generate a data URL from the decoded cursor; apply via CSS `cursor: url(data:image/png;base64,...) <hx> <hy>, auto`
  - Hotspot `hx`/`hy` comes from `ICONDIRENTRY` parsed above

## Phase 4 — Editor

- [ ] **Hotspot editor** — click on canvas to position hotspot on a cursor image
  - Hotspot is two `uint16` values at byte offset 10 & 12 in the `.cur` `ICONDIRENTRY` — [CUR spec](https://en.wikipedia.org/wiki/ICO_(file_format)#PNG_stored_as_ICO)
  - Canvas `onClick` → compute pixel coords → Tauri command `set_hotspot(pack_id, cursor_name, x, y)` that rewrites those 4 bytes in [src-tauri/src/lib.rs](src-tauri/src/lib.rs)
- [ ] **`.ani` frame timeline** — frame list, per-frame delay editing, playback preview
  - Rewrite `anih` + `rate` RIFF chunks with new delays; re-encode frames — [ANI format](https://www.gdgsoft.com/anituner/help/aniformat.htm)
  - Use `requestAnimationFrame` loop in the frontend for playback preview
- [ ] **Save edited cursor back to pack storage**
  - New Tauri command `save_cursor(pack_id, cursor_name, data: Vec<u8>)` in [src-tauri/src/lib.rs](src-tauri/src/lib.rs)
  - Atomic write: write to `.tmp` then rename to avoid corruption

## Phase 5 — Apply / Remove

- [ ] **Windows: copy cursor files + write registry** ([src-tauri/src/lib.rs](src-tauri/src/lib.rs))
  - Copy `.cur` / `.ani` files to `%SystemRoot%\Cursors\<PackName>\`
  - Write `HKCU\Control Panel\Cursors` values: `Arrow`, `Wait`, `IBeam`, `Crosshair`, `SizeNWSE`, `SizeNESW`, `SizeWE`, `SizeNS`, `SizeAll`, `No`, `Hand`, `AppStarting`, `Help`, `Pin`, `Person` — [MSDN cursor registry](https://learn.microsoft.com/en-us/windows/win32/menurc/using-cursors)
  - Use [`winreg` crate](https://docs.rs/winreg) (add to [Cargo.toml](src-tauri/Cargo.toml)) — `cfg(target_os = "windows")` guard
  - Call `SystemParametersInfoW(SPI_SETCURSORS, 0, null, SPIF_SENDCHANGE)` to apply without reboot — [MSDN SystemParametersInfo](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-systemparametersinfow); use [`windows` crate](https://docs.rs/windows)
- [ ] **Windows: revert to previous / default cursor set**
  - Snapshot existing registry values before applying; store snapshot in `pack.json` or a separate `revert.json` in `app_data_dir`
  - Restore snapshot values + call `SPI_SETCURSORS` again
- [ ] **Linux: install to `~/.icons/`, update active cursor theme**
  - XDG cursor theme layout: `~/.icons/<ThemeName>/index.theme` + `~/.icons/<ThemeName>/cursors/<name>` (Xcursor binary) — [Arch Wiki: Cursor themes](https://wiki.archlinux.org/title/Cursor_themes)
  - Update `gtk-cursor-theme-name` in `~/.config/gtk-3.0/settings.ini` and `~/.config/gtk-4.0/settings.ini`
  - Optionally write `Xcursor.theme: <ThemeName>` to `~/.Xresources` and run `xrdb -merge`
  - `cfg(target_os = "linux")` guard in [src-tauri/src/lib.rs](src-tauri/src/lib.rs)
- [ ] **Apply view UI** — select pack, apply / revert buttons, status feedback
  - New view `src/views/Apply.tsx`; wire up in [src/App.tsx](src/App.tsx) (currently placeholder)
  - Show applied pack name + timestamp; disable Apply if already active pack

## Phase 6 — Export

- [ ] **Export pack as `.zip`**
  - Use `zip` crate (already in [Cargo.toml](src-tauri/Cargo.toml)); new Tauri command `export_pack_zip(pack_id, dest_path)`
  - Open save dialog via `tauri-plugin-dialog` ([docs](https://tauri.app/plugin/dialog/))
- [ ] **Export as Windows `install.inf` + cursor bundle**
  - Generate `install.inf` with `[Version]`, `[DefaultInstall]`, `[Strings]` sections — [Microsoft INF syntax](https://learn.microsoft.com/en-us/windows-hardware/drivers/install/inf-file-sections-and-directives)
  - Reference existing `.inf` files parsed in Phase 2 as format examples
- [ ] **Export as X11 Xcursor theme**
  - Xcursor binary format: 4-byte magic `0x72756358`, header, then chunks of type `0xFFFD0002` (image) — [Xcursor(3) man page](https://www.x.org/releases/X11R7.7/doc/man/man3/Xcursor.3.xhtml)
  - Use [`xcursor` crate](https://docs.rs/xcursor) if available, else write magic + header manually
  - Bundle as `index.theme` + `cursors/` in a `.tar.gz`

## UI / UX

- [ ] **Custom right-click context menu on pack cards** — trigger at cursor position
  - `@base-ui/react` Menu already wired in [src/components/ui/dropdown-menu.tsx](src/components/ui/dropdown-menu.tsx); add `onContextMenu` handler to `PackCard` in [src/views/Browse.tsx](src/views/Browse.tsx)
  - Position via `anchorRef` or a virtual anchor at `(e.clientX, e.clientY)` — [Base UI Menu docs](https://base-ui.com/react/components/menu)
  - Actions: Import, Delete, Export, View Detail
- [ ] **Pack search / filter bar in Browse view** — [src/views/Browse.tsx](src/views/Browse.tsx)
  - Client-side filter over `packs[]` state by name / author / platform
- [ ] **Drag-and-drop import** (zip or folder onto Browse view)
  - Listen for `tauri://drag-drop` event via `listen()` from `@tauri-apps/api/event` — [Tauri drag-drop docs](https://tauri.app/v1/guides/features/file-drop/)
  - Add listener in [src/views/Browse.tsx](src/views/Browse.tsx); pass dropped paths to existing `import_pack` command
- [ ] **Settings view** — configure default import path, apply behaviour
  - Wire up the Settings tab in [src/App.tsx](src/App.tsx) (currently placeholder)
  - Persist settings to `app_data_dir/settings.json` via a Tauri command
- [ ] **Replace default Tauri icon with PawPack icon**
  - Replace `src-tauri/icons/` assets; update `tauri.conf.json` `bundle.icon` paths — [Tauri icons guide](https://tauri.app/distribute/icons/)

## Fixes / Tech Debt

- [x] Fix app identifier → `com.jankeys.pawpack`
- [ ] **Move `PackMeta` type to shared `src/types.ts`** — currently duplicated in [src/views/Browse.tsx:26-34](src/views/Browse.tsx#L26-L34); import from `src/types.ts` in both the view and any future views
- [ ] **Fix unicode-unsafe `slugify`** — [src-tauri/src/lib.rs](src-tauri/src/lib.rs)
  - Current impl breaks on non-ASCII folder names (Japanese, Arabic, emoji, etc.)
  - Replace with [`slug` crate](https://docs.rs/slug) or [`unidecode`](https://docs.rs/unidecode) + [`unicode-normalization`](https://docs.rs/unicode-normalization) for transliteration before ASCII-clamping
