# PawPack

> Desktop app for importing, previewing, editing, and applying cursor packs.

Built with Tauri v2 + React 19 + TypeScript.

**Windows app.** Applying cursors goes through the Windows registry and Win32,
so that half is Windows-only. Linux Xcursor packs import and preview fine, but
installing them to `~/.icons/` is not implemented yet — see
[CHANGELOG.md](CHANGELOG.md) for the full list of limitations.

## Install

Grab the latest `.msi` or `.exe` from
[Releases](https://github.com/Jankeys02/PawPack/releases/latest).

PawPack updates itself: **Settings → About → Check**. Updates are downloaded
from this repo's releases and only install if their signature verifies against
the key built into the app.

## Features

- **Browse** — import packs from a `.zip` or folder, see every cursor as a live
  animated thumbnail
- **Apply** — set all 17 Windows cursor roles from one pack, and revert to
  whatever was there before
- **Mix** — a different pack per role, applied as one set
- **Slideshow** — rotate each role through a playlist on a schedule, via a
  Windows scheduled task, so it keeps going with PawPack closed
- **Editor** — reposition hotspots on `.cur` and `.ani` cursors, with zoomed
  canvas and playback
- **Settings** — Windows' pointer switches, animation level, in-app updates

Formats: `.cur`, `.ani`, `install.inf`, and X11 Xcursor (read-only).

## Stack

| Layer | Tool |
|---|---|
| Desktop | [Tauri v2](https://v2.tauri.app/) (Rust) |
| Frontend | React 19 + TypeScript |
| Build | Vite |
| UI | Tailwind CSS + shadcn/ui |

## Development

Prerequisites: [Node.js](https://nodejs.org/) 20+, [Rust](https://rustup.rs/)
via rustup, and the [Tauri prerequisites](https://v2.tauri.app/start/prerequisites/)
for your OS.

```bash
git clone https://github.com/Jankeys02/PawPack.git
cd PawPack
npm install
npm run tauri dev
```

`npm run dev` alone runs only the Vite frontend without the native shell — use
`tauri dev` for the real app.

```bash
npm run build                                # typecheck + build frontend
cargo test --manifest-path src-tauri/Cargo.toml   # backend tests
npm run tauri build                          # native installer
```

Installers land under `src-tauri/target/release/bundle/`.

## Project Structure

```
PawPack/
├── src/                        # React frontend
│   ├── components/
│   │   ├── CursorGallery.tsx   # Cursor grid used by detail / mix / slideshow
│   │   ├── CursorThumb.tsx     # One cursor: static frame or .ani playback
│   │   └── ui/                 # shadcn/ui primitives
│   ├── lib/
│   │   ├── motion.ts           # App-wide animation level + hover-only mode
│   │   ├── roles.ts            # The 17 Windows cursor roles
│   │   └── utils.ts            # Tailwind class helper
│   ├── views/                  # Browse, PackDetail, Apply, Mix, Slideshow,
│   │   │                       # Editor, Settings, Debug
│   ├── App.tsx                 # Sidebar nav + state-based view routing
│   ├── types.ts                # PackMeta and friends, shared with Rust
│   └── main.tsx
├── src-tauri/                  # Tauri / Rust backend
│   ├── src/
│   │   ├── lib.rs              # Tauri commands: packs, parsing, apply, registry
│   │   ├── slideshow.rs        # Playlist model + scheduled-task rotation
│   │   └── main.rs             # Entry point, also the --rotate headless path
│   ├── capabilities/default.json
│   ├── Cargo.toml
│   └── tauri.conf.json
├── .github/workflows/          # CI on PRs, tag-triggered release
└── docs/                       # Feature specs and implementation plans
```

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md). Releases: [RELEASE_GUIDE.md](RELEASE_GUIDE.md).
Security issues: [SECURITY.md](SECURITY.md).

## License

[MIT](LICENSE)
