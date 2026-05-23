import { useState, useEffect, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import {
  MousePointer2,
  RefreshCw,
  AlertCircle,
  CheckCircle2,
  RotateCcw,
  Play,
  ShieldAlert,
} from "lucide-react";
import { cn } from "@/lib/utils";

// ── Types ─────────────────────────────────────────────────────────────────────

interface ActivePack {
  id: string;
  name: string;
  appliedAt: number; // unix ms
}

interface PackMeta {
  id: string;
  name: string;
  author: string;
  description: string;
  platform: "windows" | "linux" | "unknown";
  cursor_count: number;
  imported_at: number;
}

type CardStatus =
  | { kind: "idle" }
  | { kind: "applying" }
  | { kind: "reverting" }
  | { kind: "applied"; count: number }
  | { kind: "reverted" }
  | { kind: "error"; message: string };

// ── Sub-components ─────────────────────────────────────────────────────────────

function PlatformBadge({ platform }: { platform: PackMeta["platform"] }) {
  const styles = {
    windows: "bg-sky-500/10 text-sky-400 border-sky-500/20",
    linux:   "bg-emerald-500/10 text-emerald-400 border-emerald-500/20",
    unknown: "bg-zinc-500/10 text-zinc-500 border-zinc-500/20",
  } as const;
  const labels = { windows: "Windows", linux: "Linux", unknown: "Unknown" } as const;
  return (
    <span className={cn(
      "inline-flex items-center rounded border px-1.5 py-px font-mono text-[10px] uppercase tracking-wide",
      styles[platform],
    )}>
      {labels[platform]}
    </span>
  );
}

function PackRow({
  pack,
  isActive,
  onApplied,
  onReverted,
}: {
  pack: PackMeta;
  isActive: boolean;
  onApplied: () => void;
  onReverted: () => void;
}) {
  const [status, setStatus] = useState<CardStatus>({ kind: "idle" });

  const busy = status.kind === "applying" || status.kind === "reverting";

  const handleApply = async () => {
    setStatus({ kind: "applying" });
    try {
      const applied = await invoke<string[]>("apply_pack", { packId: pack.id });
      setStatus({ kind: "applied", count: applied.length });
      onApplied();
    } catch (e) {
      setStatus({ kind: "error", message: String(e) });
    }
  };

  const handleRevert = async () => {
    setStatus({ kind: "reverting" });
    try {
      await invoke("revert_pack", { packId: pack.id });
      setStatus({ kind: "reverted" });
      onReverted();
    } catch (e) {
      setStatus({ kind: "error", message: String(e) });
    }
  };

  return (
    <div className={cn(
      "flex flex-col gap-3 rounded-sm border bg-zinc-900 p-4 transition-colors",
      isActive ? "border-amber-500/40 bg-amber-500/5" :
      status.kind === "applied" ? "border-amber-500/30" :
      status.kind === "reverted" ? "border-zinc-700" :
      status.kind === "error" ? "border-red-500/30" :
      "border-zinc-800",
    )}>
      {/* Row: meta + actions */}
      <div className="flex items-center gap-4">
        {/* Pack info */}
        <div className="min-w-0 flex-1">
          <div className="flex items-center gap-2">
            <p className="truncate text-sm font-semibold text-zinc-100">{pack.name}</p>
            <PlatformBadge platform={pack.platform} />
            {isActive && (
              <span className="inline-flex items-center gap-1 rounded border border-amber-500/30 bg-amber-500/10 px-1.5 py-px font-mono text-[10px] uppercase tracking-wide text-amber-400">
                <span className="h-1.5 w-1.5 rounded-full bg-amber-400" />
                Active
              </span>
            )}
          </div>
          <div className="mt-0.5 flex items-center gap-3">
            {pack.author && (
              <span className="truncate text-xs text-zinc-500">{pack.author}</span>
            )}
            <span className="flex items-center gap-1 text-xs text-zinc-600">
              <MousePointer2 className="h-3 w-3" strokeWidth={1.5} />
              {pack.cursor_count > 0 ? `${pack.cursor_count} cursors` : "unknown"}
            </span>
          </div>
        </div>

        {/* Actions */}
        <div className="flex shrink-0 items-center gap-2">
          <button
            onClick={handleRevert}
            disabled={busy}
            title="Restore cursor state from before this pack was applied"
            className="flex items-center gap-1.5 rounded px-2.5 py-1.5 text-xs text-zinc-500 transition-colors hover:bg-zinc-800 hover:text-zinc-300 disabled:pointer-events-none disabled:opacity-40"
          >
            {status.kind === "reverting" ? (
              <RefreshCw className="h-3.5 w-3.5 animate-spin" strokeWidth={1.75} />
            ) : (
              <RotateCcw className="h-3.5 w-3.5" strokeWidth={1.75} />
            )}
            Revert
          </button>

          <button
            onClick={handleApply}
            disabled={busy || pack.platform !== "windows" || isActive}
            title={
              pack.platform !== "windows" ? "Only Windows packs can be applied" :
              isActive ? "This pack is already active" :
              "Apply this cursor pack system-wide"
            }
            className={cn(
              "flex items-center gap-1.5 rounded px-3 py-1.5 text-xs font-medium transition-colors",
              pack.platform === "windows" && !isActive
                ? "bg-amber-500 text-zinc-950 hover:bg-amber-400 disabled:pointer-events-none disabled:opacity-50"
                : "cursor-not-allowed bg-zinc-800 text-zinc-600",
            )}
          >
            {status.kind === "applying" ? (
              <RefreshCw className="h-3.5 w-3.5 animate-spin" strokeWidth={2} />
            ) : (
              <Play className="h-3.5 w-3.5" strokeWidth={2} />
            )}
            {status.kind === "applying" ? "Applying…" : "Apply"}
          </button>
        </div>
      </div>

      {/* Status feedback */}
      {status.kind === "applied" && (
        <div className="flex items-center gap-2 rounded border border-amber-500/20 bg-amber-500/10 px-3 py-2 text-xs text-amber-300">
          <CheckCircle2 className="h-3.5 w-3.5 shrink-0" strokeWidth={1.75} />
          Applied — {status.count} cursor role{status.count !== 1 ? "s" : ""} written.
          Cursors updated immediately.
        </div>
      )}
      {status.kind === "reverted" && (
        <div className="flex items-center gap-2 rounded border border-zinc-700 bg-zinc-800/60 px-3 py-2 text-xs text-zinc-400">
          <CheckCircle2 className="h-3.5 w-3.5 shrink-0" strokeWidth={1.75} />
          Reverted to the state before this pack was applied.
        </div>
      )}
      {status.kind === "error" && (
        <div className="flex items-start gap-2 rounded border border-red-500/20 bg-red-500/10 px-3 py-2 text-xs text-red-400">
          <AlertCircle className="mt-px h-3.5 w-3.5 shrink-0" strokeWidth={1.75} />
          <span className="break-all">{status.message}</span>
          <button
            onClick={() => setStatus({ kind: "idle" })}
            className="ml-auto shrink-0 text-red-500 hover:text-red-300"
          >
            ✕
          </button>
        </div>
      )}
    </div>
  );
}

function EmptyState() {
  return (
    <div className="flex flex-1 flex-col items-center justify-center gap-4">
      <div className="flex h-14 w-14 items-center justify-center rounded-lg border border-zinc-800 bg-zinc-900">
        <MousePointer2 className="h-6 w-6 text-zinc-700" strokeWidth={1.25} />
      </div>
      <div className="text-center">
        <p className="text-sm font-semibold text-zinc-400">No packs imported</p>
        <p className="mt-1 text-xs text-zinc-600">Import a cursor pack in the Browse tab first.</p>
      </div>
    </div>
  );
}

// ── Main view ─────────────────────────────────────────────────────────────────

export default function Apply({
  activePack,
  onApplied,
  onReverted,
}: {
  activePack: ActivePack | null;
  onApplied: (pack: PackMeta) => void;
  onReverted: () => void;
}) {
  const [packs, setPacks] = useState<PackMeta[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const loadPacks = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      setPacks(await invoke<PackMeta[]>("list_packs"));
    } catch (e) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => { loadPacks(); }, [loadPacks]);

  return (
    <div className="flex flex-1 flex-col overflow-hidden">
      {/* Header */}
      <div className="flex items-center justify-between border-b border-zinc-800/60 px-6 py-4">
        <div>
          <h1 className="text-sm font-semibold text-zinc-100">Apply Pack</h1>
          {activePack ? (
            <p className="text-xs text-zinc-500">
              <span className="text-amber-400">{activePack.name}</span>
              {" "}applied{" "}
              <span className="text-zinc-600">
                {new Date(activePack.appliedAt).toLocaleTimeString(undefined, { hour: "2-digit", minute: "2-digit" })}
              </span>
            </p>
          ) : !loading && (
            <p className="text-xs text-zinc-600">
              {packs.length === 0
                ? "No packs available"
                : `${packs.length} pack${packs.length === 1 ? "" : "s"} — select one to apply system-wide`}
            </p>
          )}
        </div>
        <button
          onClick={loadPacks}
          disabled={loading}
          className="flex h-8 w-8 items-center justify-center rounded text-zinc-600 transition-colors hover:bg-zinc-800 hover:text-zinc-400 disabled:opacity-40"
        >
          <RefreshCw className={cn("h-3.5 w-3.5", loading && "animate-spin")} strokeWidth={1.75} />
        </button>
      </div>

      {/* Info note */}
      <div className="mx-6 mt-4 flex items-start gap-2 rounded border border-zinc-700/50 bg-zinc-800/40 px-3 py-2.5 text-xs text-zinc-500">
        <ShieldAlert className="mt-px h-3.5 w-3.5 shrink-0 text-zinc-600" strokeWidth={1.75} />
        <span>
          Cursors are applied directly from the imported pack — no system files are modified.
          Registry values under{" "}
          <span className="font-mono text-zinc-400">HKCU\Control Panel\Cursors</span>
          {" "}are updated and take effect immediately without a reboot.
        </span>
      </div>

      {/* Load error */}
      {error && (
        <div className="mx-6 mt-3 flex items-start gap-2 rounded border border-red-500/20 bg-red-500/10 px-3 py-2.5 text-xs text-red-400">
          <AlertCircle className="mt-px h-3.5 w-3.5 shrink-0" strokeWidth={1.75} />
          <span>{error}</span>
          <button onClick={() => setError(null)} className="ml-auto shrink-0 hover:text-red-300">✕</button>
        </div>
      )}

      {/* Body */}
      {loading ? (
        <div className="flex flex-1 items-center justify-center">
          <RefreshCw className="h-5 w-5 animate-spin text-zinc-700" />
        </div>
      ) : packs.length === 0 ? (
        <EmptyState />
      ) : (
        <div className="flex-1 overflow-y-auto p-6">
          <div className="flex flex-col gap-2">
            {packs.map((pack) => (
              <PackRow
                key={pack.id}
                pack={pack}
                isActive={activePack?.id === pack.id}
                onApplied={() => onApplied(pack)}
                onReverted={onReverted}
              />
            ))}
          </div>
        </div>
      )}
    </div>
  );
}
