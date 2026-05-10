# PawPack

> Cross-platform desktop app for selecting, previewing, and editing cursor packs.

Supports Windows (`.cur`, `.ani`) and Linux (X11 Xcursor). Built with Tauri v2 + React 19 + TypeScript.

## Features

- Browse and preview cursor packs with live cursor rendering
- Edit cursors — hotspot positioning, frame timeline for animated `.ani` cursors
- Apply and remove cursor packs system-wide
- Import/export `.cur`, `.ani`, `install.inf`, and X11 Xcursor formats

## Stack

| Layer | Tool |
|---|---|
| Desktop | [Tauri v2](https://v2.tauri.app/) (Rust) |
| Frontend | React 19 + TypeScript |
| Build | Vite |
| UI | Tailwind CSS + shadcn/ui |

## Prerequisites

- [Node.js](https://nodejs.org/) 18+
- [Rust](https://rustup.rs/) (via rustup)
- Linux: `libwebkit2gtk`, `libappindicator` — see [Tauri prerequisites](https://v2.tauri.app/start/prerequisites/)

## Getting Started

```bash
git clone https://github.com/Jankeys02/PawPack.git
cd PawPack
npm install
npm run tauri dev
```

## Building

```bash
npm run tauri build
```

Outputs a native installer for your platform under `src-tauri/target/release/bundle/`.

## License

MIT
