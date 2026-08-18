# Animated Cursor Thumbnails Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make multi-frame `.ani` cursor thumbnails animate everywhere they appear, by encoding them as APNG instead of keeping only frame 0.

**Architecture:** A pure encoder function turns RGBA frames plus per-frame delays into APNG bytes. `cursor_path_to_b64` calls it for multi-frame `.ani` files and keeps the existing single-frame PNG path otherwise. APNG is a PNG — same magic bytes, same `image/png` mime — so every `data:image/png;base64,…` consumer animates with no frontend change.

**Tech Stack:** Rust (edition 2024), `png` 0.18 for APNG encoding, `image` 0.25 for the existing single-frame path, Tauri 2.

## Global Constraints

- Rust edition 2024; `cargo test` must pass after every task. The suite has 23 tests before this plan.
- The only new dependency permitted is `png = "0.18"`, already in `Cargo.lock` at 0.18.1 as a transitive dependency of `image`. No other new dependencies, npm or cargo.
- No frontend changes. If a task seems to need one, stop and report — it means the APNG assumption broke.
- APNG frame delays use `set_frame_delay(jiffies, 60)`, mapping `.ani` jiffies (1/60 s) exactly with no rounding.
- A frame rate of `0` is treated as `1` jiffy.
- Frames are centred on a transparent canvas, never scaled — scaling blurs pixel-art cursors.
- `.ani` `seq` chunk support is explicitly out of scope and must be marked with a `ponytail:` comment naming it as the upgrade path.
- Commit after every task.

---

### Task 1: APNG encoder

A pure function, so it is testable without touching the filesystem or a GUI.

**Files:**
- Modify: `src-tauri/Cargo.toml` (dependencies)
- Modify: `src-tauri/src/lib.rs` (encoder near `cursor_path_to_b64`, tests at the end of `mod tests`)

**Interfaces:**
- Consumes: nothing from earlier tasks
- Produces: `fn encode_animated_png(frames: &[(u32, u32, Vec<u8>)], delays_jiffies: &[u32]) -> Result<Vec<u8>, String>` — each frame is `(width, height, rgba)`; returns APNG bytes

- [ ] **Step 1: Add the dependency to `src-tauri/Cargo.toml`**

Add below the `image` line in `[dependencies]`:

```toml
png = "0.18"
```

- [ ] **Step 2: Write the failing tests at the end of the `mod tests` block in `src-tauri/src/lib.rs`**

Add after the `mix` module:

```rust
    mod apng {
        use crate::encode_animated_png;

        /// Index of a chunk's data, given its four-byte type.
        /// PNG chunk layout: [len: 4][type: 4][data: len][crc: 4].
        fn chunk_data(bytes: &[u8], kind: &[u8; 4]) -> Option<usize> {
            bytes.windows(4).position(|w| w == kind).map(|i| i + 4)
        }

        /// (frame_count, play_count) from the acTL chunk.
        fn actl(bytes: &[u8]) -> Option<(u32, u32)> {
            let d = chunk_data(bytes, b"acTL")?;
            let frames = u32::from_be_bytes(bytes[d..d + 4].try_into().ok()?);
            let plays = u32::from_be_bytes(bytes[d + 4..d + 8].try_into().ok()?);
            Some((frames, plays))
        }

        /// Every fcTL chunk's (delay_num, delay_den). fcTL data layout:
        /// seq(4) width(4) height(4) x(4) y(4) delay_num(2) delay_den(2) ...
        fn fctl_delays(bytes: &[u8]) -> Vec<(u16, u16)> {
            let mut out = Vec::new();
            for (i, w) in bytes.windows(4).enumerate() {
                if w == b"fcTL" {
                    let d = i + 4;
                    if bytes.len() >= d + 24 {
                        let num = u16::from_be_bytes([bytes[d + 20], bytes[d + 21]]);
                        let den = u16::from_be_bytes([bytes[d + 22], bytes[d + 23]]);
                        out.push((num, den));
                    }
                }
            }
            out
        }

        /// `n` solid frames of `w`x`h`, so tests need no real cursor data.
        fn frames(n: usize, w: u32, h: u32) -> Vec<(u32, u32, Vec<u8>)> {
            (0..n)
                .map(|i| (w, h, vec![(i * 10) as u8; (w * h * 4) as usize]))
                .collect()
        }

        #[test]
        fn writes_a_valid_png_signature_and_actl() {
            let out = encode_animated_png(&frames(3, 4, 4), &[5, 5, 5]).unwrap();
            assert_eq!(&out[..8], b"\x89PNG\r\n\x1a\n", "must still be a PNG");
            assert!(actl(&out).is_some(), "animated output must carry acTL");
        }

        #[test]
        fn actl_reports_frame_count_and_infinite_looping() {
            let out = encode_animated_png(&frames(3, 4, 4), &[5, 5, 5]).unwrap();
            assert_eq!(actl(&out).unwrap(), (3, 0), "3 frames, 0 = loop forever");
        }

        #[test]
        fn per_frame_delays_reach_the_encoded_output() {
            let out = encode_animated_png(&frames(3, 4, 4), &[3, 6, 9]).unwrap();
            assert_eq!(fctl_delays(&out), vec![(3, 60), (6, 60), (9, 60)]);
        }

        #[test]
        fn zero_delay_becomes_one_jiffy() {
            // A zero delay lets the viewer pick its own minimum, which makes
            // playback speed inconsistent between browsers.
            let out = encode_animated_png(&frames(2, 4, 4), &[0, 4]).unwrap();
            assert_eq!(fctl_delays(&out), vec![(1, 60), (4, 60)]);
        }

        #[test]
        fn canvas_is_the_max_of_each_dimension_independently() {
            // 32x16 beside 16x32 must give a 32x32 canvas.
            let mixed = vec![
                (32, 16, vec![0u8; 32 * 16 * 4]),
                (16, 32, vec![0u8; 16 * 32 * 4]),
            ];
            let out = encode_animated_png(&mixed, &[4, 4]).unwrap();
            // IHDR data starts right after the type: width(4) height(4).
            let d = chunk_data(&out, b"IHDR").unwrap();
            let width = u32::from_be_bytes(out[d..d + 4].try_into().unwrap());
            let height = u32::from_be_bytes(out[d + 4..d + 8].try_into().unwrap());
            assert_eq!((width, height), (32, 32));
        }

        #[test]
        fn shorter_delay_list_falls_back_to_the_last_delay() {
            // per_frame_rates can be shorter than the frame list; every frame
            // still needs a delay.
            let out = encode_animated_png(&frames(3, 4, 4), &[7]).unwrap();
            assert_eq!(fctl_delays(&out), vec![(7, 60), (7, 60), (7, 60)]);
        }

        #[test]
        fn rejects_an_empty_frame_list() {
            assert!(encode_animated_png(&[], &[]).is_err());
        }
    }
```

- [ ] **Step 3: Run the tests to verify they fail**

Run: `cd src-tauri && cargo test apng::`
Expected: FAIL to compile with `cannot find function encode_animated_png in crate root`.

- [ ] **Step 4: Implement the encoder immediately above `fn cursor_path_to_b64` in `src-tauri/src/lib.rs`**

```rust
/// Encode RGBA frames as a looping APNG.
///
/// Frames are centred on a canvas sized to the largest width and the largest
/// height present, each taken independently, and padded with transparency.
/// Padding rather than scaling: these are pixel-art cursors and scaling blurs
/// them.
///
/// `delays_jiffies` holds one delay per frame in jiffies (1/60 s), the unit
/// `.ani` files use. When it is shorter than `frames`, the last value repeats.
///
/// ponytail: frames encode in file order. `.ani` also allows a `seq` chunk
/// that reorders playback; `parse_ani_bytes` does not read it, so a pack using
/// `seq` animates in the wrong order. Parse `seq` in `parse_ani_bytes` and
/// reorder here if that ever shows up.
fn encode_animated_png(
    frames: &[(u32, u32, Vec<u8>)],
    delays_jiffies: &[u32],
) -> Result<Vec<u8>, String> {
    if frames.is_empty() {
        return Err("ANI has no frames".into());
    }

    let canvas_w = frames.iter().map(|(w, _, _)| *w).max().unwrap_or(0);
    let canvas_h = frames.iter().map(|(_, h, _)| *h).max().unwrap_or(0);
    if canvas_w == 0 || canvas_h == 0 {
        return Err("ANI frames have zero size".into());
    }

    let mut buf: Vec<u8> = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut buf, canvas_w, canvas_h);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        // The frame count must match the number of frames written below, or
        // the APNG is malformed. 0 plays means loop forever.
        encoder
            .set_animated(frames.len() as u32, 0)
            .map_err(|e| e.to_string())?;

        let mut writer = encoder.write_header().map_err(|e| e.to_string())?;

        for (i, (w, h, rgba)) in frames.iter().enumerate() {
            let jiffies = delays_jiffies
                .get(i)
                .or_else(|| delays_jiffies.last())
                .copied()
                .unwrap_or(1)
                .max(1);
            let delay = u16::try_from(jiffies).unwrap_or(u16::MAX);
            writer
                .set_frame_delay(delay, 60)
                .map_err(|e| e.to_string())?;
            writer
                .write_image_data(&center_on_canvas(*w, *h, rgba, canvas_w, canvas_h))
                .map_err(|e| e.to_string())?;
        }

        writer.finish().map_err(|e| e.to_string())?;
    }

    Ok(buf)
}

/// Copy one RGBA frame into the middle of a transparent canvas.
fn center_on_canvas(
    w: u32,
    h: u32,
    rgba: &[u8],
    canvas_w: u32,
    canvas_h: u32,
) -> Vec<u8> {
    if w == canvas_w && h == canvas_h {
        return rgba.to_vec();
    }

    let mut out = vec![0u8; (canvas_w * canvas_h * 4) as usize];
    let off_x = (canvas_w.saturating_sub(w) / 2) as usize;
    let off_y = (canvas_h.saturating_sub(h) / 2) as usize;
    let row_bytes = (w * 4) as usize;

    for y in 0..h as usize {
        let src = y * row_bytes;
        if src + row_bytes > rgba.len() {
            break;
        }
        let dst = ((y + off_y) * canvas_w as usize + off_x) * 4;
        if dst + row_bytes > out.len() {
            break;
        }
        out[dst..dst + row_bytes].copy_from_slice(&rgba[src..src + row_bytes]);
    }

    out
}
```

- [ ] **Step 5: Run the tests to verify they pass**

Run: `cd src-tauri && cargo test apng::`
Expected: PASS, 7 tests.

- [ ] **Step 6: Run the full suite**

Run: `cd src-tauri && cargo test`
Expected: PASS, 30 tests (23 before, plus 7).

- [ ] **Step 7: Commit**

```bash
git add src-tauri/Cargo.toml src-tauri/Cargo.lock src-tauri/src/lib.rs
git commit -m "feat: add an APNG encoder for animated cursors

A pure function over RGBA frames and per-frame delays, so it is testable
without files. Frames are centred on a canvas sized to the largest width
and height rather than scaled, because scaling blurs pixel-art cursors."
```

---

### Task 2: Use the encoder for multi-frame cursors

**Files:**
- Modify: `src-tauri/src/lib.rs:612` (`cursor_path_to_b64`), tests at the end of `mod tests`

**Interfaces:**
- Consumes: `encode_animated_png(frames: &[(u32, u32, Vec<u8>)], delays_jiffies: &[u32]) -> Result<Vec<u8>, String>` from Task 1
- Produces: no new interface — `cursor_path_to_b64` keeps its signature, and every caller is unchanged

- [ ] **Step 1: Write the failing tests at the end of the `mod apng` block in `src-tauri/src/lib.rs`**

Add inside `mod apng`, after `rejects_an_empty_frame_list`:

```rust
        use crate::cursor_path_to_b64;
        use base64::{engine::general_purpose::STANDARD, Engine};
        use std::fs;
        use std::path::PathBuf;

        /// A unique temp directory for one test.
        fn scratch_dir(tag: &str) -> PathBuf {
            let dir = std::env::temp_dir().join(format!(
                "pawpack-apng-{tag}-{}",
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ));
            fs::create_dir_all(&dir).unwrap();
            dir
        }

        /// Decode what the command hands the frontend, so assertions run
        /// against the same bytes the browser would.
        fn decoded(path: &std::path::Path) -> Vec<u8> {
            STANDARD.decode(cursor_path_to_b64(path).unwrap()).unwrap()
        }

        #[test]
        fn a_cur_file_stays_a_still_png() {
            let dir = scratch_dir("cur");
            let path = dir.join("Arrow.cur");
            fs::write(&path, super::make_cur(0, 0)).unwrap();

            let out = decoded(&path);
            assert_eq!(&out[..8], b"\x89PNG\r\n\x1a\n");
            assert!(actl(&out).is_none(), "a .cur must not animate");

            fs::remove_dir_all(&dir).ok();
        }

        #[test]
        fn a_single_frame_ani_stays_a_still_png() {
            let dir = scratch_dir("one");
            let path = dir.join("Busy.ani");
            fs::write(&path, super::make_ani(1)).unwrap();

            let out = decoded(&path);
            assert!(actl(&out).is_none(), "one frame is not an animation");

            fs::remove_dir_all(&dir).ok();
        }

        #[test]
        fn a_multi_frame_ani_animates() {
            let dir = scratch_dir("many");
            let path = dir.join("Busy.ani");
            fs::write(&path, super::make_ani(3)).unwrap();

            let out = decoded(&path);
            assert_eq!(actl(&out).unwrap(), (3, 0), "3 frames, looping forever");
            // make_ani writes no rate chunk, so every frame takes the anih
            // display rate of 4 jiffies.
            assert_eq!(fctl_delays(&out), vec![(4, 60), (4, 60), (4, 60)]);

            fs::remove_dir_all(&dir).ok();
        }
```

- [ ] **Step 2: Add the multi-frame ANI builder beside the other test helpers in `src-tauri/src/lib.rs`**

Place it directly after `make_ani_header_only`, inside `mod tests`. The existing `ani_with_rate_chunk` test builds a one-icon `LIST fram` inline; this generalises that to `n` icons so the new tests can vary frame count.

```rust
    /// A RIFF ACON blob with `n` identical 1x1 frames and a display rate of 4.
    fn make_ani(n: u32) -> Vec<u8> {
        let cur_data = make_cur(0, 0);
        let cur_size = cur_data.len() as u32;

        let mut list_content = Vec::new();
        list_content.extend_from_slice(b"fram");
        for _ in 0..n {
            list_content.extend_from_slice(b"icon");
            list_content.extend_from_slice(&cur_size.to_le_bytes());
            list_content.extend_from_slice(&cur_data);
            if cur_size % 2 == 1 {
                list_content.push(0);
            }
        }

        let mut anih_data = vec![0u8; 36];
        anih_data[0..4].copy_from_slice(&36u32.to_le_bytes());
        anih_data[4..8].copy_from_slice(&n.to_le_bytes()); // nFrames
        anih_data[28..32].copy_from_slice(&4u32.to_le_bytes()); // iDispRate

        let mut buf = Vec::new();
        buf.extend_from_slice(b"RIFF");
        buf.extend_from_slice(&[0u8; 4]); // size placeholder
        buf.extend_from_slice(b"ACON");
        buf.extend_from_slice(b"anih");
        buf.extend_from_slice(&36u32.to_le_bytes());
        buf.extend_from_slice(&anih_data);
        buf.extend_from_slice(b"LIST");
        buf.extend_from_slice(&(list_content.len() as u32).to_le_bytes());
        buf.extend_from_slice(&list_content);

        let riff_size = (buf.len() - 8) as u32;
        buf[4..8].copy_from_slice(&riff_size.to_le_bytes());
        buf
    }
```

- [ ] **Step 3: Run the tests to verify they fail**

Run: `cd src-tauri && cargo test apng::a_multi_frame`
Expected: FAIL — `a_multi_frame_ani_animates` panics on `actl(&out).unwrap()` being `None`, because `cursor_path_to_b64` still keeps only frame 0.

- [ ] **Step 4: Rewrite `cursor_path_to_b64` in `src-tauri/src/lib.rs`**

Replace the whole function:

```rust
fn cursor_path_to_b64(path: &Path) -> Result<String, String> {
    use base64::{engine::general_purpose::STANDARD, Engine};

    let data = fs::read(path).map_err(|e| e.to_string())?;
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_ascii_lowercase())
        .unwrap_or_default();

    if ext == "ani" {
        let ani = parse_ani_bytes(&data)?;

        // Drop frames with no image variants rather than failing the cursor.
        let frames: Vec<(u32, u32, Vec<u8>)> = ani
            .frames
            .into_iter()
            .filter_map(|f| best_frame(f).ok())
            .collect();

        if frames.is_empty() {
            return Err("ANI has no frames".into());
        }

        if frames.len() > 1 {
            let delays: Vec<u32> = if ani.per_frame_rates.is_empty() {
                vec![ani.display_rate; frames.len()]
            } else {
                ani.per_frame_rates
            };
            return Ok(STANDARD.encode(encode_animated_png(&frames, &delays)?));
        }

        let (width, height, rgba) = frames.into_iter().next().unwrap();
        return Ok(STANDARD.encode(still_png(width, height, rgba)?));
    }

    let (width, height, rgba) = best_frame(parse_cur_bytes(&data)?)?;
    Ok(STANDARD.encode(still_png(width, height, rgba)?))
}

/// Encode one RGBA frame as a plain PNG.
fn still_png(width: u32, height: u32, rgba: Vec<u8>) -> Result<Vec<u8>, String> {
    use image::{DynamicImage, ImageBuffer, Rgba};

    let img = DynamicImage::ImageRgba8(
        ImageBuffer::<Rgba<u8>, Vec<u8>>::from_raw(width, height, rgba)
            .ok_or("Failed to create image buffer")?,
    );

    let mut buf = io::Cursor::new(Vec::new());
    img.write_to(&mut buf, image::ImageFormat::Png)
        .map_err(|e| e.to_string())?;

    Ok(buf.into_inner())
}
```

Note that the rewritten `cursor_path_to_b64` no longer imports `DynamicImage`, `ImageBuffer`, or `Rgba` — `still_png` owns those now. Leaving the old import in place would produce an unused-import warning, and the task requires warning-free output.

- [ ] **Step 5: Run the tests to verify they pass**

Run: `cd src-tauri && cargo test apng::`
Expected: PASS, 10 tests.

- [ ] **Step 6: Run the full suite**

Run: `cd src-tauri && cargo test`
Expected: PASS, 33 tests. No compiler warnings.

- [ ] **Step 7: Confirm the frontend is untouched**

Run: `git diff --name-only HEAD`
Expected: only `src-tauri/src/lib.rs`. If any file under `src/` appears, the APNG assumption broke — stop and report it.

- [ ] **Step 8: Build**

Run: `npm run build`
Expected: PASS.

- [ ] **Step 9: Commit**

```bash
git add src-tauri/src/lib.rs
git commit -m "feat: animate multi-frame .ani thumbnails

cursor_path_to_b64 kept only frame 0. Multi-frame cursors now encode as
APNG, which is a PNG by signature and mime type, so all four render sites
animate without a single frontend change."
```

---

## Verification

After Task 2:

```bash
cd src-tauri && cargo test && cd .. && npm run build && npm run tauri build
```

Expected: 33 Rust tests pass, TypeScript compiles, installers build.

Manual check — the one thing tests cannot cover:

```bash
npm run tauri dev
```

Open Browse and PackDetail on the Bog pack, which ships `Busy.ani` and
`Working in bg.ani`. Expected: those two thumbnails loop; the `.cur`
thumbnails stay still. Then open the Mix tab's gallery and confirm the same
files animate there.
