# Animated Cursor Thumbnails

Date: 2026-08-18
Status: approved, not yet implemented

## Goal

Show animated `.ani` cursors actually animating, everywhere a thumbnail
appears, looping continuously.

## Non-goals

- Hover-to-play or any playback toggle. Thumbnails always loop.
- Animating the Debug tab's hover cursors. Those are real CSS `cursor:`
  values, which cannot animate — a platform limit, not a gap in this work.
- Parsing the `.ani` `seq` chunk. See Known limitations.

## Background

`cursor_path_to_b64` (`src-tauri/src/lib.rs:612`) decodes a cursor file and
returns a base64 PNG. For an `.ani` it takes `ani.frames.into_iter().next()` —
frame 0 — and discards every other frame.

Four places render the result, all through the same string:

| Site | Command |
|---|---|
| `src/views/Browse.tsx:65` | `get_pack_thumbnails` |
| `src/views/PackDetail.tsx:35` | `list_pack_cursors` |
| `src/views/Apply.tsx:98` | `list_pack_cursors` |
| `src/views/Mix.tsx:65,298` | `list_pack_cursors` |

All four build `data:image/png;base64,${thumbnail}`.

## Approach: APNG

`cursor_path_to_b64` encodes APNG when the file is an `.ani` with more than
one frame, and a plain PNG otherwise.

APNG is a PNG: same magic bytes, same `image/png` mime type, and a viewer
without APNG support renders the first frame. So every existing
`data:image/png;base64,…` usage keeps working unchanged and **all four sites
animate with no frontend changes at all**.

The alternatives lose something this does not:

- **GIF** has 1-bit alpha. Cursors are antialiased against transparency, so
  every edge would gain a halo.
- **Sprite sheet with CSS `steps()`** keeps alpha but forces uniform frame
  timing, and `.ani` files carry per-frame delays.

### Dependency

`png = "0.18"` becomes a direct dependency of `src-tauri`. It is already in
`Cargo.lock` at 0.18.1 as a transitive dependency of `image`, so this adds a
manifest line and nothing to the build tree.

`image` cannot do this itself — its PNG encoder has no APNG support, which is
why the encoder is written against `png` directly.

### Encoding

```rust
let mut encoder = png::Encoder::new(&mut buf, width, height);
encoder.set_color(png::ColorType::Rgba);
encoder.set_depth(png::BitDepth::Eight);
// frame_count MUST equal the number of frames actually written below.
// A mismatch produces a corrupt APNG, so count after skipping empty frames.
encoder.set_animated(frame_count, 0)?; // 0 repeats = loop forever
let mut writer = encoder.write_header()?;
for (rgba, jiffies) in frames {
    writer.set_frame_delay(jiffies, 60)?;
    writer.write_image_data(&rgba)?;
}
writer.finish()?;
```

### Frame delays

`.ani` delays are in jiffies (1/60 s); APNG delays are a fraction. Writing
`set_frame_delay(jiffies, 60)` maps them exactly, with no rounding.

Per-frame values come from `AniInfo::per_frame_rates` when that vector is
non-empty. Otherwise every frame uses `AniInfo::display_rate`. A rate of `0`
is treated as `1`, since a zero delay makes browsers pick their own minimum
and produces inconsistent playback speed.

### Mismatched frame sizes

APNG requires every frame to share one canvas size, but nothing in the `.ani`
format guarantees that.

Target dimensions are the maximum width and the maximum height across all
frames' selected variants, each taken independently — a 32x16 frame beside a
16x32 frame yields a 32x32 canvas. Each frame is centred into that canvas on
transparent padding. Padding rather than scaling — scaling would blur
pixel-art cursors.

Within a single animation frame, the variant is chosen by the existing
`best_frame` rule (largest area), unchanged.

## Behavior by input

| Input | Output |
|---|---|
| `.cur` | Plain PNG, exactly as today |
| `.ani`, one frame | Plain PNG, no `acTL` chunk |
| `.ani`, many frames | APNG, looping forever, per-frame delays |
| `.ani`, no frames | Error `"ANI has no frames"`, as today |

## Known limitations

`.ani` supports a `seq` chunk that reorders frames into an arbitrary playback
sequence. `parse_ani_bytes` does not parse it and this change does not add it,
so frames play in file order. That is correct for the overwhelming majority of
packs and wrong for the few that use `seq`, where playback order will differ
from the real cursor. Mark it with a `ponytail:` comment naming `seq` parsing
as the upgrade path.

Payload size grows roughly linearly with frame count. `get_pack_thumbnails`
already caps Browse at 9 thumbnails per pack, and a 32×32 RGBA frame is small,
so no cap is added here.

## Error handling

| Case | Behavior |
|---|---|
| APNG encoding fails | Propagate the error as `Err(String)`; callers already render an empty thumbnail as a grey placeholder |
| A frame has no variants | Skip that frame rather than failing the whole cursor |
| Every frame is skipped | Error `"ANI has no frames"` |
| Frame rate of 0 | Treated as 1 jiffy |

## Testing

Encoding is pure — no registry, no GUI — so it is tested directly. The
existing test helpers already build `.cur` and `.ani` byte blobs by hand.

- A multi-frame `.ani` encodes to output whose PNG signature is intact and
  which contains an `acTL` chunk
- That output's `acTL` reports the expected frame count and infinite looping
- Per-frame delays from a `rate` chunk reach the encoded `fcTL` chunks
- Without a `rate` chunk, every frame carries `display_rate`
- A single-frame `.ani` produces output with no `acTL`
- A `.cur` produces output with no `acTL`
- Frames of differing sizes produce one canvas sized to the maximum

## Build order

1. The APNG encoder and its tests, behind the existing `cursor_path_to_b64`
2. Verify the four render sites animate, unchanged
