import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Sparkles, Radar, EyeOff, Target, RefreshCw } from "lucide-react";
import { cn } from "@/lib/utils";

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

// ── Main view ─────────────────────────────────────────────────────────────────

export default function Settings() {
  // A key absent from the map means Windows never reported it, which leaves its
  // switch disabled rather than showing a confident but invented "off".
  const [flags, setFlags] = useState<Record<string, boolean>>({});
  const [loading, setLoading] = useState(true);
  const [busy, setBusy] = useState<string | null>(null);
  const [errors, setErrors] = useState<Record<string, string>>({});

  const load = () => {
    setLoading(true);
    invoke<Record<string, boolean>>("get_pointer_flags")
      .then(setFlags)
      .catch((e) => setErrors({ shadow: String(e) }))
      .finally(() => setLoading(false));
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
      </div>
    </div>
  );
}
