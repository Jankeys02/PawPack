import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Sparkles } from "lucide-react";
import { cn } from "@/lib/utils";

// ── Sub-components ─────────────────────────────────────────────────────────────

/// One system preference: a label, a description, and a switch. The description
/// doubles as the error slot, so a failed read or write is visible where the
/// setting is rather than in a banner somewhere else.
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

/// System-wide "Enable pointer shadow" (SPI_SETCURSORSHADOW) — the same switch
/// as Mouse Properties > Pointers. Independent of which pack is applied, so it
/// keeps its own state rather than riding along with an apply.
function ShadowToggle() {
  const [enabled, setEnabled] = useState<boolean | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    invoke<boolean>("get_cursor_shadow")
      .then(setEnabled)
      .catch((e) => setError(String(e)));
  }, []);

  const toggle = async () => {
    if (enabled === null || busy) return;
    setBusy(true);
    setError(null);
    try {
      // The command returns the value Windows actually holds afterwards.
      setEnabled(await invoke<boolean>("set_cursor_shadow", { enabled: !enabled }));
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  };

  return (
    <ToggleRow
      Icon={Sparkles}
      label="Pointer shadow"
      description="Drop shadow drawn under every cursor, system-wide. Not part of a pack."
      enabled={enabled}
      busy={busy}
      error={error}
      onToggle={toggle}
    />
  );
}

// ── Main view ─────────────────────────────────────────────────────────────────

export default function Settings() {
  return (
    <div className="flex flex-1 flex-col overflow-hidden">
      {/* Header */}
      <div className="border-b border-zinc-800/60 px-6 py-4">
        <h1 className="text-sm font-semibold text-zinc-100">Settings</h1>
        <p className="text-xs text-zinc-600">
          Preferences that apply to every pack
        </p>
      </div>

      <div className="flex-1 overflow-y-auto px-6 py-4">
        <p className="mb-1.5 font-mono text-[10px] uppercase tracking-wide text-zinc-600">
          System cursor
        </p>
        <div className="divide-y divide-zinc-800/60 rounded border border-zinc-800 bg-zinc-950/60">
          <ShadowToggle />
        </div>
        <p className="mt-2 text-[11px] text-zinc-700">
          Changes here are written straight to Windows and take effect immediately.
        </p>
      </div>
    </div>
  );
}
