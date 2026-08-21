import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Sparkles, Radar, EyeOff, Target, RefreshCw, Clapperboard, Gauge } from "lucide-react";
import { cn } from "@/lib/utils";
import {
  MOTION_LEVELS,
  MOTION_LABELS,
  MOTION_DESCRIPTIONS,
  motionLevel,
  setMotionLevel,
  type MotionLevel,
} from "@/lib/motion";

// ── Settings table ─────────────────────────────────────────────────────────────

/// The system pointer switches, in the order Mouse Properties lists them.
/// `key` must match a name in the Rust `POINTER_FLAGS` table.
const POINTER_FLAGS = [
  {
    key: "shadow",
    Icon: Sparkles,
    label: "Pointer shadow",
    description: "Drop shadow drawn under every cursor. Not part of a pack.",
  },
  {
    key: "vanish",
    Icon: EyeOff,
    label: "Hide pointer while typing",
    description: "The cursor disappears until the mouse moves again.",
  },
  {
    key: "sonar",
    Icon: Radar,
    label: "Show location on Ctrl",
    description: "Rings close in on the pointer when Ctrl is pressed.",
  },
  {
    key: "snap_to_default",
    Icon: Target,
    label: "Snap to default button",
    description: "Jump the pointer to the default button of a new dialog.",
  },
] as const;

// ── Sub-components ─────────────────────────────────────────────────────────────

/// One switch: label, description, control. The description doubles as the
/// error slot, so a failed read or write shows where the setting is rather than
/// in a banner somewhere else.
function ToggleRow({
  Icon,
  label,
  description,
  enabled,
  busy,
  error,
  onToggle,
}: {
  Icon: React.ComponentType<{ className?: string; strokeWidth?: number }>;
  label: string;
  description: string;
  enabled: boolean | null;
  busy: boolean;
  error: string | null;
  onToggle: () => void;
}) {
  return (
    <div className="flex items-center gap-3 px-4 py-3">
      <Icon className="h-4 w-4 shrink-0 text-zinc-600" strokeWidth={1.75} />
      <div className="min-w-0 flex-1">
        <div className="text-xs text-zinc-300">{label}</div>
        <div className={cn("text-[11px]", error ? "text-red-400" : "text-zinc-600")}>
          {error ?? description}
        </div>
      </div>
      <button
        role="switch"
        aria-checked={enabled ?? false}
        aria-label={label}
        disabled={enabled === null || busy}
        onClick={onToggle}
        className={cn(
          "relative h-5 w-9 shrink-0 rounded-full transition-colors disabled:opacity-40",
          enabled ? "bg-amber-500" : "bg-zinc-700",
        )}
      >
        <span
          className={cn(
            "absolute top-0.5 h-4 w-4 rounded-full bg-zinc-950 transition-all",
            enabled ? "left-[18px]" : "left-0.5",
          )}
        />
      </button>
    </div>
  );
}

/// Three-step motion slider. A native range input rather than a custom track:
/// it is keyboard-operable and screen-reader-labelled for free, which a div
/// with a drag handler is not.
function MotionRow({
  level,
  onChange,
}: {
  level: MotionLevel;
  onChange: (level: MotionLevel) => void;
}) {
  const index = MOTION_LEVELS.indexOf(level);

  return (
    <div className="flex items-start gap-3 px-4 py-3">
      <Gauge className="mt-px h-4 w-4 shrink-0 text-zinc-600" strokeWidth={1.75} />
      <div className="min-w-0 flex-1">
        <div className="text-xs text-zinc-300">Animation</div>
        <div className="text-[11px] text-zinc-600">{MOTION_DESCRIPTIONS[level]}</div>

        <input
          type="range"
          min={0}
          max={MOTION_LEVELS.length - 1}
          step={1}
          value={index}
          aria-label="Animation level"
          aria-valuetext={MOTION_LABELS[level]}
          onChange={(e) => onChange(MOTION_LEVELS[Number(e.target.value)])}
          className="mt-2.5 h-1 w-full cursor-pointer appearance-none rounded-full bg-zinc-700 accent-amber-500"
        />

        <div className="mt-1 flex justify-between">
          {MOTION_LEVELS.map((l) => (
            <button
              key={l}
              onClick={() => onChange(l)}
              className={cn(
                "font-mono text-[10px] uppercase tracking-wide transition-colors",
                l === level ? "text-amber-400" : "text-zinc-600 hover:text-zinc-400",
              )}
            >
              {MOTION_LABELS[l]}
            </button>
          ))}
        </div>
      </div>
    </div>
  );
}

// ── Main view ─────────────────────────────────────────────────────────────────

/** Only the fields Settings needs from `SlideshowState`. */
interface SlideshowSetting { stop_on_apply: boolean }

export default function Settings() {
  // A key absent from the map means Windows never reported it, which leaves its
  // switch disabled rather than showing a confident but invented "off".
  const [flags, setFlags] = useState<Record<string, boolean>>({});
  const [loading, setLoading] = useState(true);
  const [busy, setBusy] = useState<string | null>(null);
  const [errors, setErrors] = useState<Record<string, string>>({});
  // Null until read. PawPack's own settings live in slideshow.json, not in a
  // preferences store — this is currently the only one.
  const [slideshow, setSlideshow] = useState<SlideshowSetting | null>(null);
  // Local to PawPack and read synchronously — no round trip to Windows, and
  // nothing here writes a system setting.
  const [motion, setMotion] = useState<MotionLevel>(motionLevel);

  const load = () => {
    setLoading(true);
    invoke<Record<string, boolean>>("get_pointer_flags")
      .then(setFlags)
      .catch((e) => setErrors({ shadow: String(e) }))
      .finally(() => setLoading(false));

    invoke<SlideshowSetting>("get_slideshow")
      .then(setSlideshow)
      .catch((e) => setErrors((prev) => ({ ...prev, stop_on_apply: String(e) })));
  };

  useEffect(load, []);

  const toggle = async (key: string) => {
    if (busy || !(key in flags)) return;
    setBusy(key);
    setErrors((e) => ({ ...e, [key]: "" }));
    try {
      // The command returns the value Windows actually holds afterwards.
      const now = await invoke<boolean>("set_pointer_flag", { name: key, enabled: !flags[key] });
      setFlags((f) => ({ ...f, [key]: now }));
    } catch (e) {
      setErrors((prev) => ({ ...prev, [key]: String(e) }));
    } finally {
      setBusy(null);
    }
  };

  const toggleStopOnApply = async () => {
    if (busy || !slideshow) return;
    setBusy("stop_on_apply");
    setErrors((e) => ({ ...e, stop_on_apply: "" }));
    try {
      setSlideshow(
        await invoke<SlideshowSetting>("set_slideshow_stop_on_apply", {
          enabled: !slideshow.stop_on_apply,
        }),
      );
    } catch (e) {
      setErrors((prev) => ({ ...prev, stop_on_apply: String(e) }));
    } finally {
      setBusy(null);
    }
  };

  return (
    <div className="flex flex-1 flex-col overflow-hidden">
      {/* Header */}
      <div className="flex items-center justify-between border-b border-zinc-800/60 px-6 py-4">
        <div>
          <h1 className="text-sm font-semibold text-zinc-100">Settings</h1>
          <p className="text-xs text-zinc-600">Preferences that apply to every pack</p>
        </div>
        <button
          onClick={load}
          disabled={loading}
          aria-label="Reload settings"
          className="flex h-8 w-8 items-center justify-center rounded text-zinc-600 transition-colors hover:bg-zinc-800 hover:text-zinc-400 disabled:opacity-40"
        >
          <RefreshCw className={cn("h-3.5 w-3.5", loading && "animate-spin")} strokeWidth={1.75} />
        </button>
      </div>

      <div className="flex-1 overflow-y-auto px-6 py-4">
        <p className="mb-1.5 font-mono text-[10px] uppercase tracking-wide text-zinc-600">
          Pointer
        </p>
        <div className="divide-y divide-zinc-800/60 rounded border border-zinc-800 bg-zinc-950/60">
          {POINTER_FLAGS.map(({ key, Icon, label, description }) => (
            <ToggleRow
              key={key}
              Icon={Icon}
              label={label}
              description={description}
              enabled={key in flags ? flags[key] : null}
              busy={busy === key}
              error={errors[key] || null}
              onToggle={() => toggle(key)}
            />
          ))}
        </div>
        <p className="mt-2 text-[11px] text-zinc-700">
          These are Windows' own pointer settings, the same ones in Mouse Properties.
          PawPack stores nothing — changes are written straight to Windows and take effect
          immediately.
        </p>

        <p className="mb-1.5 mt-5 font-mono text-[10px] uppercase tracking-wide text-zinc-600">
          Appearance
        </p>
        <div className="rounded border border-zinc-800 bg-zinc-950/60">
          <MotionRow
            level={motion}
            onChange={(l) => { setMotionLevel(l); setMotion(l); }}
          />
        </div>
        <p className="mt-2 text-[11px] text-zinc-700">
          PawPack only. This does not change Windows' animation settings — it starts off
          matching them, and after that it is the one that counts.
        </p>

        <p className="mb-1.5 mt-5 font-mono text-[10px] uppercase tracking-wide text-zinc-600">
          Slideshow
        </p>
        <div className="divide-y divide-zinc-800/60 rounded border border-zinc-800 bg-zinc-950/60">
          <ToggleRow
            Icon={Clapperboard}
            label="Applying stops the slideshow"
            description="Applying a pack or a mix turns the rotation off. Off, the slideshow reclaims its roles on the next tick."
            enabled={slideshow?.stop_on_apply ?? null}
            busy={busy === "stop_on_apply"}
            error={errors.stop_on_apply || null}
            onToggle={toggleStopOnApply}
          />
        </div>
        <p className="mt-2 text-[11px] text-zinc-700">
          Reverting your cursors always stops the slideshow, whatever this is set to.
        </p>
      </div>
    </div>
  );
}
