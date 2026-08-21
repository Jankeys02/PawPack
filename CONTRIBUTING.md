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

```bash
cargo test --manifest-path src-tauri/Cargo.toml
```

`build` runs `tsc` then `vite build`. CI runs both of these on every PR and both
have to pass before it can merge.

## Making a change

1. Branch off `main` — `main` is protected, so direct pushes are rejected.
2. Keep the change focused — one PR, one purpose.
3. Open the PR against `main` and fill in the template.
4. A PR needs one approving review from a code owner
   ([CODEOWNERS](.github/CODEOWNERS)) before it can merge.

## Reporting bugs / requesting features

Use the [issue templates](.github/ISSUE_TEMPLATE) — they ask for the info needed to reproduce.

## Releasing

See [RELEASE_GUIDE.md](RELEASE_GUIDE.md) if you have push access and are cutting a version.
