# Changelog

All notable changes to PawPack are documented here.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [1.0.0] - 2026-08-21

First stable release.

### Added

- **Packs** — import cursor packs from a `.zip` or a folder, browse them as a
  grid with live animated thumbnails, open a pack to see every cursor in it, and
  delete packs you no longer want. Windows (`.cur` / `.ani`) and X11 Xcursor
  packs are both detected and previewed; name, author, and description are read
  from `install.inf` or `index.theme`.
- **Apply** — apply a pack to all 17 Windows cursor roles and revert to the set
  that was active before, from a snapshot taken at apply time.
- **Mix** — assign a different pack to each cursor role and apply the result as
  one set.
- **Slideshow** — give each role a playlist of cursors and rotate through them on
  a schedule, driven by a Windows scheduled task so rotation continues while
  PawPack is closed. Optionally stops when you apply a pack by hand.
- **Editor** — reposition the hotspot on any cursor, with a zoomed canvas, a live
  hover preview, and `.ani` playback. Multi-size cursors and every `.ani` frame
  are patched together so the sizes stay aligned.
- **Settings** — Windows' own pointer switches (shadow, hide-while-typing,
  show-location-on-Ctrl, snap-to-default), a PawPack-only animation level, and a
  hover-only mode for animated cursor previews.
- **In-app updater** — check for and install new releases from Settings. Updates
  are downloaded from GitHub releases and only install if their signature
  verifies against the key built into the app.

### Known limitations

- Applying, reverting, and the slideshow are **Windows only**. Linux Xcursor
  packs import and preview, but cannot yet be installed to `~/.icons/`.
- No export yet — packs go in, they do not come back out.
- `.ani` frame timing (per-frame delays) is not editable; only the hotspot is.
- Cursor size is not exposed, because writing `CursorBaseSize` does not actually
  resize anything without a logoff.

[Unreleased]: https://github.com/Jankeys02/PawPack/compare/v1.0.0...HEAD
[1.0.0]: https://github.com/Jankeys02/PawPack/releases/tag/v1.0.0
