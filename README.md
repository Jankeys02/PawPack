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

## Project Structure

```
PawPack/
├── src/                        # React frontend
│   ├── components/
│   │   └── ui/                 # shadcn/ui components (button, dropdown-menu, …)
│   ├── lib/
│   │   └── utils.ts            # Tailwind class helper
│   ├── views/
│   │   └── Browse.tsx          # Pack browser view
│   ├── App.tsx                 # Root component & routing
│   ├── main.tsx                # React entry point
│   └── index.css               # Global styles / Tailwind directives
├── src-tauri/                  # Tauri / Rust backend
│   ├── src/
│   │   ├── main.rs             # Binary entry point
│   │   └── lib.rs              # Tauri commands & app logic
│   ├── capabilities/
│   │   └── default.json        # Tauri permission scopes
│   ├── icons/                  # App icons for all platforms
│   ├── Cargo.toml              # Rust dependencies
│   └── tauri.conf.json         # Tauri app configuration
├── public/                     # Static assets served by Vite
├── index.html                  # Vite HTML entry
├── vite.config.ts
├── tsconfig.json
└── package.json
```

## License

MIT
