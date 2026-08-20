import { useState, useEffect, useCallback, useMemo } from "react";
import { invoke } from "@tauri-apps/api/core";
import {
  RefreshCw, AlertCircle, CheckCircle2, Crosshair, Play, Pause, MousePointer2, Save,
} from "lucide-react";
import { cn, bestFrame, frameToDataUrl, frameToCssCursor, type CurFrame } from "@/lib/utils";

// ── Types ─────────────────────────────────────────────────────────────────────

interface PackMeta { id: string; name: string; cursor_count: number }
interface CursorEntry { name: string; kind: string; thumbnail: string }
interface CurInfo { frames: CurFrame[] }
interface AniInfo {
  frame_count: number;
  display_rate: number;
  per_frame_rates: number[];
  frames: CurInfo[];
}

/** Everything the canvas needs, whether the source was a .cur or a .ani. */
interface Loaded {
  /** One image per animation step; a .cur yields exactly one. */
  frames: CurFrame[];
  /** Delay per step in jiffies (1/60 s). Same length as `frames`. */
  delays: number[];
  animated: boolean;
}

const JIFFY_MS = 1000 / 60;

/// Hotspot shortcuts. Coordinates are pixel indices, so the far edge is size - 1.
const PRESETS: { label: string; at: (w: number, h: number) => [number, number] }[] = [
  { label: "Top left",     at: () => [0, 0] },
  { label: "Top right",    at: (w) => [w - 1, 0] },
  { label: "Center",       at: (w, h) => [Math.floor(w / 2), Math.floor(h / 2)] },
  { label: "Bottom left",  at: (_w, h) => [0, h - 1] },
  { label: "Bottom right", at: (w, h) => [w - 1, h - 1] },
];

// ── Loading ───────────────────────────────────────────────────────────────────

async function loadCursor(packId: string, entry: CursorEntry): Promise<Loaded> {
  if (entry.kind === "ani") {
    const ani = await invoke<AniInfo>("parse_ani", { packId, cursorName: entry.name });
    const frames = ani.frames
      .map((f) => bestFrame(f.frames))
      .filter((f): f is CurFrame => f !== null);
    // The `rate` chunk is optional — fall back to the anih display rate.
    const delays = frames.map((_, i) => ani.per_frame_rates[i] || ani.display_rate || 6);
    return { frames, delays, animated: frames.length > 1 };
  }

  const cur = await invoke<CurInfo>("parse_cur", { packId, cursorName: entry.name });
  const frame = bestFrame(cur.frames);
  return { frames: frame ? [frame] : [], delays: [0], animated: false };
}

// ── Sub-components ─────────────────────────────────────────────────────────────

/** Zoomed cursor image; click anywhere to drop the hotspot on that pixel. */
function HotspotCanvas({
  frame,
  hotspot,
  onPick,
}: {
  frame: CurFrame;
  hotspot: { x: number; y: number };
  onPick: (x: number, y: number) => void;
}) {
  // Integer zoom only — a fractional one blurs the pixel grid and puts the
  // hotspot between pixels.
  const scale = Math.min(16, Math.max(1, Math.floor(320 / frame.width)));
  const src = useMemo(() => frameToDataUrl(frame), [frame]);

  const pick = (e: React.MouseEvent<HTMLDivElement>) => {
    const rect = e.currentTarget.getBoundingClientRect();
    const x = Math.floor((e.clientX - rect.left) / scale);
    const y = Math.floor((e.clientY - rect.top) / scale);
    onPick(
      Math.min(frame.width - 1, Math.max(0, x)),
      Math.min(frame.height - 1, Math.max(0, y)),
    );
  };

  return (
    <div
      onClick={pick}
      style={{ width: frame.width * scale, height: frame.height * scale, cursor: "crosshair" }}
      className="relative shrink-0 rounded-sm bg-zinc-950/60 ring-1 ring-zinc-800"
    >
      <img
        src={src}
        alt=""
        draggable={false}
        className="pointer-events-none absolute inset-0 h-full w-full"
        style={{ imageRendering: "pixelated" }}
      />
      {/* Crosshair sits on the centre of the target pixel. */}
      <div
        className="pointer-events-none absolute inset-y-0 w-px bg-amber-400/70"
        style={{ left: (hotspot.x + 0.5) * scale }}
      />
      <div
        className="pointer-events-none absolute inset-x-0 h-px bg-amber-400/70"
        style={{ top: (hotspot.y + 0.5) * scale }}
      />
      <div
        className="pointer-events-none absolute ring-1 ring-amber-400"
        style={{ left: hotspot.x * scale, top: hotspot.y * scale, width: scale, height: scale }}
      />
    </div>
  );
}

// ── Main view ─────────────────────────────────────────────────────────────────

export default function Editor() {
  const [packs, setPacks] = useState<PackMeta[]>([]);
  const [packId, setPackId] = useState<string>("");
  const [cursors, setCursors] = useState<CursorEntry[]>([]);
  const [selected, setSelected] = useState<CursorEntry | null>(null);

  const [loaded, setLoaded] = useState<Loaded | null>(null);
  const [hotspot, setHotspot] = useState({ x: 0, y: 0 });
  const [step, setStep] = useState(0);
  const [playing, setPlaying] = useState(true);

  const [loading, setLoading] = useState(true);
  const [saving, setSaving] = useState(false);
  const [saved, setSaved] = useState(false);
  const [error, setError] = useState<string | null>(null);

  // Packs, once.
  useEffect(() => {
    invoke<PackMeta[]>("list_packs")
      .then((p) => {
        setPacks(p);
        setPackId((id) => id || p[0]?.id || "");
      })
      .catch((e) => setError(String(e)))
      .finally(() => setLoading(false));
  }, []);

  // Cursor list follows the selected pack.
  useEffect(() => {
    if (!packId) return;
    let cancelled = false;
    setSelected(null);
    setLoaded(null);
    invoke<CursorEntry[]>("list_pack_cursors", { packId })
      .then((list) => { if (!cancelled) setCursors(list); })
      .catch((e) => { if (!cancelled) setError(String(e)); });
    return () => { cancelled = true; };
  }, [packId]);

  const open = useCallback(async (entry: CursorEntry) => {
    setSelected(entry);
    setLoaded(null);
    setError(null);
    setSaved(false);
    setStep(0);
    try {
      const data = await loadCursor(packId, entry);
      setLoaded(data);
      const first = data.frames[0];
      setHotspot({ x: first?.hotspot_x ?? 0, y: first?.hotspot_y ?? 0 });
    } catch (e) {
      setError(String(e));
    }
  }, [packId]);

  // Animation clock. Each step waits its own delay, so uneven timings play right.
  useEffect(() => {
    if (!playing || !loaded?.animated) return;
    const ms = Math.max(1, loaded.delays[step] ?? 6) * JIFFY_MS;
    const timer = setTimeout(() => setStep((s) => (s + 1) % loaded.frames.length), ms);
    return () => clearTimeout(timer);
  }, [playing, step, loaded]);

  const frame = loaded?.frames[step] ?? loaded?.frames[0] ?? null;
  const original = loaded?.frames[0];
  const dirty =
    !!original && (hotspot.x !== original.hotspot_x || hotspot.y !== original.hotspot_y);

  /// The one way the hotspot moves — canvas, preset, or typed number. Clamps to
  /// the image so a stray keystroke can't write an off-image hotspot.
  const place = (x: number, y: number) => {
    if (!frame) return;
    const clamp = (v: number, max: number) =>
      Number.isFinite(v) ? Math.min(max, Math.max(0, Math.round(v))) : 0;
    setHotspot({ x: clamp(x, frame.width - 1), y: clamp(y, frame.height - 1) });
    setSaved(false);
  };

  const save = async () => {
    if (!selected || !frame) return;
    setSaving(true);
    setError(null);
    try {
      await invoke("set_hotspot", {
        packId,
        cursorName: selected.name,
        x: hotspot.x,
        y: hotspot.y,
        refW: frame.width,
        refH: frame.height,
      });
      // Re-read from disk so the "original" baseline matches what was written.
      setLoaded(await loadCursor(packId, selected));
      setCursors(await invoke<CursorEntry[]>("list_pack_cursors", { packId }));
      setSaved(true);
    } catch (e) {
      setError(String(e));
    } finally {
      setSaving(false);
    }
  };

  return (
    <div className="flex flex-1 flex-col overflow-hidden">
      {/* Header */}
      <div className="flex items-center justify-between border-b border-zinc-800/60 px-6 py-4">
        <div>
          <h1 className="text-sm font-semibold text-zinc-100">Cursor Editor</h1>
          <p className="text-xs text-zinc-600">
            Click the canvas to place the hotspot — the pixel a click actually lands on
          </p>
        </div>
        <div className="flex items-center gap-2">
          <select
            value={packId}
            onChange={(e) => setPackId(e.target.value)}
            className="h-8 rounded border border-zinc-700 bg-zinc-800 px-2 text-xs text-zinc-300"
          >
            {packs.length === 0 && <option value="">No packs</option>}
            {packs.map((p) => (
              <option key={p.id} value={p.id}>{p.name}</option>
            ))}
          </select>
          <button
            onClick={save}
            disabled={!dirty || saving}
            className="flex h-8 items-center gap-1.5 rounded bg-amber-500 px-3 text-xs font-medium text-zinc-950 transition-colors hover:bg-amber-400 disabled:opacity-40"
          >
            <Save className="h-3 w-3" strokeWidth={2.5} />
            {saving ? "Saving…" : "Save Hotspot"}
          </button>
        </div>
      </div>

      {error && (
        <div className="flex items-center gap-2 border-b border-red-900/40 bg-red-950/30 px-6 py-2.5 text-xs text-red-300">
          <AlertCircle className="h-3.5 w-3.5 shrink-0" strokeWidth={1.75} />
          {error}
        </div>
      )}
      {saved && !dirty && (
        <div className="flex items-center gap-2 border-b border-emerald-900/40 bg-emerald-950/30 px-6 py-2.5 text-xs text-emerald-300">
          <CheckCircle2 className="h-3.5 w-3.5 shrink-0" strokeWidth={1.75} />
          Hotspot written to {selected?.name}. Re-apply the pack to update your system cursors.
        </div>
      )}

      <div className="flex flex-1 overflow-hidden">
        {/* Cursor rail */}
        <div className="w-[220px] shrink-0 overflow-y-auto border-r border-zinc-800/60 p-2">
          {loading && (
            <div className="flex justify-center py-6">
              <RefreshCw className="h-4 w-4 animate-spin text-zinc-600" strokeWidth={1.75} />
            </div>
          )}
          {!loading && cursors.length === 0 && (
            <p className="px-2 py-4 text-xs text-zinc-600">No cursors in this pack.</p>
          )}
          {cursors.map((entry) => (
            <button
              key={entry.name}
              onClick={() => open(entry)}
              className={cn(
                "flex w-full items-center gap-2.5 rounded-sm px-2 py-1.5 text-left transition-colors",
                selected?.name === entry.name ? "bg-zinc-800/70" : "hover:bg-zinc-800/40",
              )}
            >
              <div className="flex h-7 w-7 shrink-0 items-center justify-center rounded bg-zinc-950/60">
                {entry.thumbnail ? (
                  <img
                    src={`data:image/png;base64,${entry.thumbnail}`}
                    alt=""
                    className="h-5 w-5 object-contain"
                    style={{ imageRendering: "pixelated" }}
                  />
                ) : (
                  <MousePointer2 className="h-3.5 w-3.5 text-zinc-700" strokeWidth={1.25} />
                )}
              </div>
              <span className="flex-1 truncate text-[11px] text-zinc-400">
                {entry.name.replace(/\.(cur|ani)$/i, "")}
              </span>
              {entry.kind === "ani" && (
                <span className="font-mono text-[9px] uppercase text-amber-500/70">ani</span>
              )}
            </button>
          ))}
        </div>

        {/* Canvas */}
        <div className="flex-1 overflow-y-auto p-6">
          {!selected && (
            <div className="flex h-full flex-col items-center justify-center gap-3 text-zinc-600">
              <Crosshair className="h-7 w-7" strokeWidth={1.25} />
              <p className="text-xs">Pick a cursor to edit its hotspot</p>
            </div>
          )}

          {selected && !loaded && !error && (
            <div className="flex h-full items-center justify-center">
              <RefreshCw className="h-4 w-4 animate-spin text-zinc-600" strokeWidth={1.75} />
            </div>
          )}

          {frame && loaded && (
            <div className="flex flex-wrap items-start gap-8">
              <HotspotCanvas
                frame={frame}
                hotspot={hotspot}
                onPick={place}
              />

              <div className="flex min-w-[240px] flex-col gap-4">
                <div>
                  <p className="font-mono text-[9px] uppercase tracking-[0.14em] text-zinc-600">
                    Hotspot
                  </p>
                  <div className="mt-1.5 flex items-center gap-2">
                    <input
                      type="number"
                      min={0}
                      max={frame.width - 1}
                      value={hotspot.x}
                      onChange={(e) => place(e.target.valueAsNumber, hotspot.y)}
                      className="h-7 w-16 rounded border border-zinc-700 bg-zinc-800 px-2 font-mono text-xs text-zinc-200"
                    />
                    <input
                      type="number"
                      min={0}
                      max={frame.height - 1}
                      value={hotspot.y}
                      onChange={(e) => place(hotspot.x, e.target.valueAsNumber)}
                      className="h-7 w-16 rounded border border-zinc-700 bg-zinc-800 px-2 font-mono text-xs text-zinc-200"
                    />
                    {dirty && original && (
                      <button
                        onClick={() => place(original.hotspot_x, original.hotspot_y)}
                        className="text-[11px] text-zinc-500 underline-offset-2 hover:text-zinc-300 hover:underline"
                      >
                        was {original.hotspot_x}, {original.hotspot_y}
                      </button>
                    )}
                  </div>
                  <p className="mt-1.5 text-[11px] text-zinc-600">
                    {frame.width}×{frame.height} px
                    {loaded.animated && ` · ${loaded.frames.length} frames`}
                    {" · applied to every size in the file"}
                  </p>
                </div>

                <div className="flex flex-wrap gap-1.5">
                  {PRESETS.map((p) => {
                    const [px, py] = p.at(frame.width, frame.height);
                    const on = hotspot.x === px && hotspot.y === py;
                    return (
                      <button
                        key={p.label}
                        onClick={() => place(px, py)}
                        className={cn(
                          "rounded border px-2 py-1 text-[11px] transition-colors",
                          on
                            ? "border-amber-500/40 bg-amber-500/10 text-amber-400"
                            : "border-zinc-700 text-zinc-400 hover:bg-zinc-800 hover:text-zinc-200",
                        )}
                      >
                        {p.label}
                      </button>
                    );
                  })}
                </div>

                {loaded.animated && (
                  <div className="flex items-center gap-2">
                    <button
                      onClick={() => setPlaying((p) => !p)}
                      className="flex h-7 items-center gap-1.5 rounded border border-zinc-700 px-2 text-[11px] text-zinc-300 transition-colors hover:bg-zinc-800"
                    >
                      {playing
                        ? <Pause className="h-3 w-3" strokeWidth={2} />
                        : <Play className="h-3 w-3" strokeWidth={2} />}
                      {playing ? "Pause" : "Play"}
                    </button>
                    <span className="font-mono text-[10px] text-zinc-600">
                      frame {step + 1}/{loaded.frames.length} · {loaded.delays[step]} jiffies
                    </span>
                  </div>
                )}

                {/* Live check: the real OS cursor, with the unsaved hotspot. */}
                <div>
                  <p className="mb-2 font-mono text-[9px] uppercase tracking-[0.14em] text-zinc-600">
                    Live preview
                  </p>
                  <div
                    style={{ cursor: frameToCssCursor(frame, hotspot) }}
                    className="flex h-24 items-center justify-center rounded-sm border border-zinc-700 bg-zinc-900/60 text-[11px] text-zinc-600"
                  >
                    Hover here — the tip should sit where you clicked
                  </div>
                </div>
              </div>
            </div>
          )}
        </div>
      </div>
    </div>
  );
}
