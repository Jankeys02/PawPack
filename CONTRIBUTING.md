# Contributing to PawPack

Thanks for taking a look — this is a small solo project, but issues and PRs are welcome.

## Prerequisites

- Node.js 20+
- Rust (stable) + the [Tauri prerequisites](https://tauri.app/start/prerequisites/) for your OS

## Setup

```bash
git clone git@github.com:Jankeys02/PawPack.git
cd PawPack
npm install
npm run tauri dev
```

`npm run dev` alone only runs the Vite frontend without the native shell — use `tauri dev` to run the actual app.

## Before opening a PR

```bash
npm run build
```

`build` runs `tsc` then `vite build`; there's no separate test suite yet.

## Making a change

1. Branch off `main`.
2. Keep the change focused — one PR, one purpose.
3. Open the PR against `main` and fill in the template.

## Reporting bugs / requesting features

Use the [issue templates](.github/ISSUE_TEMPLATE) — they ask for the info needed to reproduce.

## Releasing

See [RELEASE_GUIDE.md](RELEASE_GUIDE.md) if you have push access and are cutting a version.
