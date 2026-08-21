# How to Create a Release

Releases are automated: pushing a `v*` tag runs
[`.github/workflows/release.yml`](.github/workflows/release.yml), which builds
the Windows installers, signs them, and opens a **draft** GitHub release with the
bundles and a `latest.json` attached.

`latest.json` is what the in-app updater reads. It is served from
`releases/latest/download/latest.json`, which only resolves once the release is
**published** — a draft release is invisible to the updater, which is exactly the
point: nothing ships to users until you press publish.

## One-time setup

The updater only installs bundles signed with the key whose public half is in
`src-tauri/tauri.conf.json` under `plugins.updater.pubkey`. The private half
lives at `~/.tauri/pawpack-updater.key` and must **never** be committed.

Put it in the repo's Actions secrets (this pipes the file straight to GitHub —
it does not print the key):

```bash
gh secret set TAURI_SIGNING_PRIVATE_KEY < ~/.tauri/pawpack-updater.key
```

That is the only secret to set. The key was generated without a password, and
`TAURI_SIGNING_PRIVATE_KEY_PASSWORD` interpolates to an empty string when the
secret does not exist — which is the value we want, so there is nothing to add.

If you ever regenerate the key, **every already-installed copy of PawPack stops
being able to update** — those users have to reinstall by hand. Back the file up.

## Cutting a release

### 1. Bump the version in all three places

They must match — npm reads `package.json`, Tauri reads
`src-tauri/tauri.conf.json`, and Cargo reads `src-tauri/Cargo.toml`:

```json
// package.json
{ "version": "1.1.0" }
```
```json
// src-tauri/tauri.conf.json
{ "version": "1.1.0" }
```
```toml
# src-tauri/Cargo.toml
version = "1.1.0"
```

Follow [SemVer](https://semver.org/): breaking → MAJOR, new features → MINOR,
fixes → PATCH.

### 2. Update the changelog

Move the `[Unreleased]` entries in [CHANGELOG.md](CHANGELOG.md) into a dated
`[1.1.0]` section and add the comparison links at the bottom.

### 3. Commit, tag, push

`main` is protected, so the version bump goes through a PR like any other change.
Once it is merged:

```bash
git checkout main && git pull
```

```bash
git tag -a v1.1.0 -m "Release version 1.1.0" && git push origin v1.1.0
```

### 4. Publish

The workflow leaves a draft release. Check the bundles are attached and
`latest.json` is there, write the release notes, then publish it. Existing
installs pick the update up the next time someone hits **Settings → About →
Check**.

## Building locally

```bash
npm run tauri build
```

A local build produces installers but **no `.sig` files** unless the signing env
vars are set, so do not upload local bundles to a release — the updater would
reject them. Let the workflow do it.
