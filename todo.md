# PawPack Todo

## Phase 1 — Foundation

- [x] Install Tailwind CSS v4 + shadcn/ui
- [x] App shell: sidebar nav + main content layout
- [x] Fix app name / window config (`pawpack-tmp` → `PawPack`, bump window size)
- [x] Clean up boilerplate (greet command, demo UI)

## Phase 2 — Pack Storage

- [x] Rust commands: `list_packs`, `import_pack`, `delete_pack`
- [x] Packs stored in `app_data_dir/packs/` with `pack.json` manifest
- [x] Auto-detect Windows (`.cur` / `.ani`) and Linux (Xcursor) pack formats
- [x] Parse `install.inf` / `index.theme` for name, author, description
- [x] Zip import: extract to temp, handle wrapped folder, clean up
- [x] Browse view: pack grid, import dropdown (`.zip` + folder), delete with confirm

## Phase 3 — Cursor Preview

- [ ] Parse `.cur` / `.ani` binary formats in Rust (extract pixel data + frame info)
- [ ] Render cursor thumbnails on Browse pack cards
- [ ] Pack detail view: click a card to see all cursors in the pack
- [ ] Live cursor preview: hover a preview area to see the actual rendered cursor

## Phase 4 — Editor

- [ ] Hotspot editor: click on canvas to position hotspot on a cursor image
- [ ] `.ani` frame timeline: frame list, per-frame delay editing, playback preview
- [ ] Save edited cursor back to pack storage

## Phase 5 — Apply / Remove

- [ ] Windows: copy cursor files + write `HKCU\Control Panel\Cursors` registry keys
- [ ] Windows: revert to previous / default cursor set from registry
- [ ] Linux: install to `~/.icons/`, update active cursor theme
- [ ] Apply view UI: select pack, apply / revert buttons, status feedback

## Phase 6 — Export

- [ ] Export pack as `.zip`
- [ ] Export as Windows `install.inf` + cursor bundle
- [ ] Export as X11 Xcursor theme

## UI / UX

- [ ] Custom right-click context menu on pack cards — use `@base-ui/react` Menu, trigger at cursor position
- [ ] Pack search / filter bar in Browse view
- [ ] Drag-and-drop import (zip or folder onto Browse view)
- [ ] Settings view: configure default import path, apply behaviour
- [ ] Replace default Tauri icon with PawPack icon

## Fixes / Tech Debt

- [ ] Fix app identifier: `com.jankeys.PawPack` → `com.jankeys.pawpack`
- [ ] Move `PackMeta` type to shared `src/types.ts` (currently duplicated in Browse.tsx)
- [ ] Fix unicode-unsafe `slugify` (breaks on non-ASCII folder names)
