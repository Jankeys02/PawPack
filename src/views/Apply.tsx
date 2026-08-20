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
  ChevronDown,
  ChevronRight,
} from "lucide-react";
import { cn } from "@/lib/utils";
import { ROLE_LABELS, unmatchedReason } from "@/lib/roles";
import { groupPacks, variantLabel, type PackGroup, type PackMeta } from "@/types";
import type { Applied, AppliedTarget } from "@/App";

// ── Types ─────────────────────────────────────────────────────────────────────

interface CursorAssignment {
  role: string;
  file: string;
}

interface PackAssignmentResult {
  assigned: CursorAssignment[];
  unmatched: string[];
}

interface CursorEntry {
  name: string;
  kind: string;
  thumbnail: string; // base64 PNG, empty string when decoding failed
}


type CardStatus =
  | { kind: "idle" }
  | { kind: "applying" }
  | { kind: "reverting" }
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

const ALL_ROLES = Object.keys(ROLE_LABELS).sort();

function AssignmentsDropdown({ packId, isActive, prefetched }: {
  packId: string;
  isActive: boolean;
  prefetched: PackAssignmentResult | null;
}) {
  const [open, setOpen] = useState(false);
  const [result, setResult] = useState<PackAssignmentResult | null>(prefetched);
  const [loading, setLoading] = useState(false);
  const [saving, setSaving] = useState<string | null>(null);
  const [thumbs, setThumbs] = useState<Record<string, string>>({});

  const toggle = async () => {
    if (!open && result === null) {
      setLoading(true);
      try {
        setResult(await invoke<PackAssignmentResult>("get_pack_assignments", { packId }));
      } catch {
        setResult({ assigned: [], unmatched: [] });
      } finally {
        setLoading(false);
      }
    }
    if (!open && Object.keys(thumbs).length === 0) {
      try {
        const cursors = await invoke<CursorEntry[]>("list_pack_cursors", { packId });
        setThumbs(Object.fromEntries(cursors.map((c) => [c.name, c.thumbnail])));
      } catch {
        // Leave thumbs empty; rows fall back to the filename alone.
      }
    }
    setOpen((v) => !v);
  };

  const handleRoleChange = async (file: string, currentRole: string | null, newRole: string) => {
    setSaving(file);
    try {
      let updated: PackAssignmentResult;
      if (!newRole && currentRole) {
        updated = await invoke<PackAssignmentResult>("set_cursor_override", { packId, role: currentRole, file: "" });
      } else if (newRole) {
        updated = await invoke<PackAssignmentResult>("set_cursor_override", { packId, role: newRole, file });
      } else {
        return;
      }
      setResult(updated);
      if (isActive) {
        await invoke("apply_pack", { packId });
      }
    } catch (e) {
      console.error(e);
    } finally {
      setSaving(null);
    }
  };

  const displayed = result ?? prefetched;
  const assignedCount = displayed?.assigned.length ?? null;
  const unmatchedCount = displayed?.unmatched.length ?? 0;

  // Unified rows sorted by filename.
  const rows: { file: string; role: string | null }[] = [
    ...(displayed?.assigned.map((a) => ({ file: a.file, role: a.role })) ?? []),
    ...(displayed?.unmatched.map((f) => ({ file: f, role: null })) ?? []),
  ].sort((a, b) => a.file.localeCompare(b.file, undefined, { sensitivity: "base" }));

  return (
    <div className="border-t border-zinc-800/70 pt-2">
      <button
        onClick={toggle}
        className="flex w-full items-center gap-1.5 rounded px-1 py-1 text-xs text-zinc-500 transition-colors hover:text-zinc-300"
      >
        {loading ? (
          <RefreshCw className="h-3 w-3 animate-spin shrink-0" strokeWidth={1.75} />
        ) : open ? (
          <ChevronDown className="h-3 w-3 shrink-0" strokeWidth={2} />
        ) : (
          <ChevronRight className="h-3 w-3 shrink-0" strokeWidth={2} />
        )}
        {assignedCount !== null
          ? `${assignedCount} cursor role${assignedCount !== 1 ? "s" : ""} assigned${unmatchedCount > 0 ? `, ${unmatchedCount} unrecognized` : ""}`
          : "View cursor assignments"}
      </button>

      {open && displayed && rows.length > 0 && (
        <div className="mt-1.5 rounded border border-zinc-800 bg-zinc-950/60">
          <div className="grid grid-cols-[32px_1fr_152px_1fr] gap-x-3 px-3 py-1.5 text-[10px] font-medium uppercase tracking-wide text-zinc-600">
            <span className="w-8" />
            <span>File</span>
            <span>Role</span>
            <span>Description</span>
          </div>
          <div className="divide-y divide-zinc-800/60">
            {rows.map(({ file, role }) => (
              <div
                key={file}
                className={cn(
                  "grid grid-cols-[32px_1fr_152px_1fr] items-center gap-x-3 px-3 py-1",
                  !role && "opacity-50",
                )}
              >
                <span className="py-1 pl-1">
                  {thumbs[file] ? (
                    <img
                      src={`data:image/png;base64,${thumbs[file]}`}
                      alt=""
                      className="h-5 w-5 object-contain"
                      style={{ imageRendering: "pixelated" }}
                    />
                  ) : (
                    <span className="block h-5 w-5 rounded-sm bg-zinc-800/60" />
                  )}
                </span>
                <span className="font-mono text-[11px] text-zinc-400 truncate">{file}</span>
                <select
                  value={role ?? ""}
                  disabled={saving === file}
                  onChange={(e) => handleRoleChange(file, role, e.target.value)}
                  className="w-full rounded border border-zinc-700 bg-zinc-900 px-1.5 py-0.5 font-mono text-[11px] text-zinc-300 focus:border-amber-500/50 focus:outline-none disabled:opacity-40"
                >
                  <option value="">— unassigned —</option>
                  {ALL_ROLES.map((r) => (
                    <option key={r} value={r}>{r}</option>
                  ))}
                </select>
                <span
                  className={cn(
                    "text-[11px] truncate",
                    role ? "text-zinc-600" : "cursor-help italic text-zinc-600 underline decoration-dotted underline-offset-2",
                  )}
                  title={role ? undefined : unmatchedReason(file)}
                >
                  {role ? (ROLE_LABELS[role] ?? "—") : "no matching role"}
                </span>
              </div>
            ))}
          </div>
        </div>
      )}

      {open && displayed && rows.length === 0 && (
        <p className="mt-1.5 px-1 text-xs text-zinc-600">No cursor files found.</p>
      )}
    </div>
  );
}

/// Holds the selected variant for a grouped download and renders the ordinary
/// row for it. A lone pack goes straight through, so its row is unchanged.
function PackGroupRow({
  group,
  isActive,
  appliedResult,
  onApplied,
  onReverted,
}: {
  group: PackGroup;
  isActive: (pack: PackMeta) => boolean;
  appliedResult: (pack: PackMeta) => PackAssignmentResult | null;
  onApplied: (pack: PackMeta, result: PackAssignmentResult) => void;
  onReverted: () => void;
}) {
  const [selectedId, setSelectedId] = useState(group.packs[0].id);
  const pack = group.packs.find((p) => p.id === selectedId) ?? group.packs[0];

  // Follow the applied variant, so the row shows what is actually on screen
  // rather than whichever variant sorted first.
  const active = group.packs.find(isActive);
  const shown = active ?? pack;

  return (
    <PackRow
      pack={shown}
      group={group.packs.length > 1 ? group : null}
      selectedId={shown.id}
      onSelectVariant={setSelectedId}
      isActive={isActive(shown)}
      appliedResult={appliedResult(shown)}
      onApplied={(result) => onApplied(shown, result)}
      onReverted={onReverted}
    />
  );
}

function PackRow({
  pack,
  group,
  selectedId,
  onSelectVariant,
  isActive,
  appliedResult,
  onApplied,
  onReverted,
}: {
  pack: PackMeta;
  /** Non-null when this row stands for a download with several variants. */
  group: PackGroup | null;
  selectedId: string;
  onSelectVariant: (id: string) => void;
  isActive: boolean;
  /** Non-null only when this pack is the applied target. */
  appliedResult: PackAssignmentResult | null;
  onApplied: (result: PackAssignmentResult) => void;
  onReverted: () => void;
}) {
  const [status, setStatus] = useState<CardStatus>({ kind: "idle" });

  const busy = status.kind === "applying" || status.kind === "reverting";

  const handleApply = async () => {
    setStatus({ kind: "applying" });
    try {
      const result = await invoke<PackAssignmentResult>("apply_pack", { packId: pack.id });
      setStatus({ kind: "idle" });
      onApplied(result);
    } catch (e) {
      setStatus({ kind: "error", message: String(e) });
    }
  };

  const handleRevert = async () => {
    setStatus({ kind: "reverting" });
    try {
      await invoke("revert_cursors");
      setStatus({ kind: "idle" });
      onReverted();
    } catch (e) {
      setStatus({ kind: "error", message: String(e) });
    }
  };

  const showAssignments = isActive || appliedResult !== null;

  return (
    <div className={cn(
      "flex flex-col gap-3 rounded-sm border bg-zinc-900 p-4 transition-colors",
      isActive ? "border-amber-500/40 bg-amber-500/5" :
      status.kind === "error" ? "border-red-500/30" :
      "border-zinc-800",
    )}>
      {/* Row: meta + actions */}
      <div className="flex items-center gap-4">
        {/* Pack info */}
        <div className="min-w-0 flex-1">
          <div className="flex items-center gap-2">
            <p className="truncate text-sm font-semibold text-zinc-100">
              {group ? group.title : pack.name}
            </p>
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

          {group && (
            <div className="mt-1.5 flex flex-wrap gap-1">
              {group.packs.map((p) => (
                <button
                  key={p.id}
                  onClick={() => onSelectVariant(p.id)}
                  aria-pressed={p.id === selectedId}
                  className={cn(
                    "rounded-sm border px-1.5 py-0.5 text-[11px] transition-colors",
                    p.id === selectedId
                      ? "border-amber-500/40 bg-amber-500/15 text-amber-300"
                      : "border-zinc-800 text-zinc-500 hover:border-zinc-700 hover:text-zinc-300",
                  )}
                >
                  {variantLabel(p)}
                </button>
              ))}
            </div>
          )}
        </div>

        {/* Actions */}
        <div className="flex shrink-0 items-center gap-2">
          <button
            onClick={handleRevert}
            disabled={busy}
            title="Restore your original cursors"
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
      {appliedResult && (
        <div className="flex items-center gap-2 rounded border border-amber-500/20 bg-amber-500/10 px-3 py-2 text-xs text-amber-400">
          <CheckCircle2 className="h-3.5 w-3.5 shrink-0" strokeWidth={1.75} />
          Applied — {appliedResult.assigned.length} cursor role{appliedResult.assigned.length !== 1 ? "s" : ""} written.
          Cursors updated immediately.
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

      {/* Assignments dropdown */}
      {showAssignments && (
        <AssignmentsDropdown
          packId={pack.id}
          isActive={isActive}
          prefetched={appliedResult}
        />
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
  applied,
  onApplied,
  onReverted,
}: {
  applied: Applied | null;
  onApplied: (target: AppliedTarget) => void;
  onReverted: () => void;
}) {
  const [packs, setPacks] = useState<PackMeta[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  /** Result of the apply performed in this session, if any. */
  const [appliedResult, setAppliedResult] = useState<
    { packId: string; result: PackAssignmentResult } | null
  >(null);
  /** Whether the last thing that happened was a revert — one shared fact, not per-card. */
  const [justReverted, setJustReverted] = useState(false);

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
          {applied ? (
            <p className="text-xs text-zinc-500">
              <span className="text-amber-400">
                {applied.target.kind === "pack" ? applied.target.name : "Custom mix"}
              </span>
              {" "}applied{" "}
              <span className="text-zinc-600">
                {new Date(applied.appliedAt).toLocaleTimeString(undefined, { hour: "2-digit", minute: "2-digit" })}
              </span>
            </p>
          ) : !loading && (
            <p className="text-xs text-zinc-600">
              {justReverted
                ? "Restored your original cursors."
                : packs.length === 0
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
            {groupPacks(packs).map((group) => (
              <PackGroupRow
                key={group.key}
                group={group}
                isActive={(p) =>
                  applied?.target.kind === "pack" && applied.target.id === p.id
                }
                appliedResult={(p) =>
                  appliedResult?.packId === p.id ? appliedResult.result : null
                }
                onApplied={(p, result) => {
                  setAppliedResult({ packId: p.id, result });
                  setJustReverted(false);
                  onApplied({ kind: "pack", id: p.id, name: p.name });
                }}
                onReverted={() => {
                  setAppliedResult(null);
                  setJustReverted(true);
                  onReverted();
                }}
              />
            ))}
          </div>
        </div>
      )}
    </div>
  );
}
