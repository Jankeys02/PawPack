# How to Create a Release

There is no release automation (`.github/workflows`) for PawPack yet — releases are manual.

## Prerequisites

1. Ensure `npm run build` succeeds
2. Clean git working tree

## Steps

### 1. Bump the version in both places

They must match — Tauri reads `src-tauri/tauri.conf.json`, Cargo reads `src-tauri/Cargo.toml`:

```json
// package.json
{ "version": "0.2.0" }
```
```json
// src-tauri/tauri.conf.json
{ "version": "0.2.0" }
```
```toml
# src-tauri/Cargo.toml
version = "0.2.0"
```

### 2. Commit and tag

```bash
git add package.json src-tauri/tauri.conf.json src-tauri/Cargo.toml
git commit -m "Update version to 0.2.0"
git tag -a v0.2.0 -m "Release version 0.2.0"
git push origin main v0.2.0
```

### 3. Build the installers

```bash
npm run tauri build
```

Output bundles (per-OS installers: `.msi`/`.exe` on Windows, `.dmg` on macOS, `.deb`/`.AppImage` on Linux) land under `src-tauri/target/release/bundle/`.

### 4. Publish

Create a GitHub release for the pushed tag and upload the bundle files manually from `src-tauri/target/release/bundle/`.

## Automating this later

If releases become frequent, wrap step 3 in a `.github/workflows/release.yml` using [`tauri-apps/tauri-action`](https://github.com/tauri-apps/tauri-action) triggered on `v*` tags, matrixed across OS runners — see BooruView's `RELEASE_GUIDE.md` for the shape of a tag-triggered workflow.
