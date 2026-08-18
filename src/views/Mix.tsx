import { useState, useEffect, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import { RefreshCw, AlertCircle, CheckCircle2, Play, Shuffle } from "lucide-react";
import { cn } from "@/lib/utils";
import { CURSOR_ROLES } from "@/lib/roles";
import type { AppliedTarget, Applied } from "@/App";

interface PackMeta { id: string; name: string; cursor_count: number }
interface CursorEntry { name: string; kind: string; thumbnail: string }
interface MixEntry { role: string; pack: string; file: string }
interface MixResult { roles: MixEntry[]; stale: MixEntry[] }
interface ApplyMixResult { written: number; cleared: string[] }
interface CursorAssignment { role: string; file: string }
interface PackAssignmentResult { assigned: CursorAssignment[]; unmatched: string[] }

/** All cursors of one pack, ready for the gallery. */
interface PackCursors { pack: PackMeta; cursors: CursorEntry[] }

export default function Mix({
  applied,
  onApplied,
}: {
  applied: Applied | null;
  onApplied: (target: AppliedTarget) => void;
}) {
  const [mix, setMix] = useState<MixResult>({ roles: [], stale: [] });
  const [library, setLibrary] = useState<PackCursors[]>([]);
  const [selectedRole, setSelectedRole] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);
  const [applying, setApplying] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [result, setResult] = useState<ApplyMixResult | null>(null);

  const load = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const packs = await invoke<PackMeta[]>("list_packs");
      const withCursors = await Promise.all(
        packs.map(async (pack) => ({
          pack,
          cursors: await invoke<CursorEntry[]>("list_pack_cursors", { packId: pack.id }),
        })),
      );
      setLibrary(withCursors);
      setMix(await invoke<MixResult>("get_mix"));
    } catch (e) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => { load(); }, [load]);

  const byRole = Object.fromEntries(mix.roles.map((e) => [e.role, e]));
  const filled = mix.roles.length;

  const thumbFor = (entry: MixEntry | undefined) => {
    if (!entry) return null;
    const pack = library.find((p) => p.pack.id === entry.pack);
    return pack?.cursors.find((c) => c.name === entry.file)?.thumbnail ?? null;
  };

  const assign = async (packId: string, file: string) => {
    if (!selectedRole) return;
    try {
      setMix(await invoke<MixResult>("set_mix_role", { role: selectedRole, packId, file }));
      setResult(null);
    } catch (e) {
      setError(String(e));
    }
  };

  const clearRole = async (role: string) => {
    try {
      setMix(await invoke<MixResult>("set_mix_role", { role, packId: "", file: "" }));
      setResult(null);
    } catch (e) {
      setError(String(e));
    }
  };

  /** Copy in the roles this pack assigns, leaving the rest of the mix alone. */
  const fillFromPack = async (packId: string) => {
    try {
      const assignments = await invoke<PackAssignmentResult>("get_pack_assignments", { packId });
      let latest = mix;
      for (const a of assignments.assigned) {
        latest = await invoke<MixResult>("set_mix_role", {
          role: a.role, packId, file: a.file,
        });
      }
      setMix(latest);
      setResult(null);
    } catch (e) {
      setError(String(e));
    }
  };

  const handleApply = async () => {
    setApplying(true);
    setError(null);
    try {
      setResult(await invoke<ApplyMixResult>("apply_mix"));
      onApplied({ kind: "mix" });
    } catch (e) {
      setError(String(e));
    } finally {
      setApplying(false);
    }
  };

  const isActive = applied?.target.kind === "mix";

  return (
    <div className="flex flex-1 flex-col overflow-hidden">
      {/* Header */}
      <div className="flex items-center justify-between border-b border-zinc-800/60 px-6 py-4">
        <div>
          <h1 className="flex items-center gap-2 text-sm font-semibold text-zinc-100">
            Mix &amp; Match
            {isActive && (
              <span className="inline-flex items-center gap-1 rounded border border-amber-500/30 bg-amber-500/10 px-1.5 py-px font-mono text-[10px] uppercase tracking-wide text-amber-400">
                <span className="h-1.5 w-1.5 rounded-full bg-amber-400" />
                Active
              </span>
            )}
          </h1>
          <p className="text-xs text-zinc-500">
            {filled} / {CURSOR_ROLES.length} roles filled
            {filled > 0 && filled < CURSOR_ROLES.length && (
              <span className="text-zinc-600">
                {" "}— {CURSOR_ROLES.length - filled} will reset to Windows defaults
              </span>
            )}
          </p>
        </div>
        <div className="flex items-center gap-2">
          <select
            value=""
            onChange={(e) => { if (e.target.value) fillFromPack(e.target.value); }}
            className="h-8 rounded border border-zinc-700 bg-zinc-800 px-2 text-xs text-zinc-300"
          >
            <option value="">Fill from pack…</option>
            {library.map((p) => (
              <option key={p.pack.id} value={p.pack.id}>{p.pack.name}</option>
            ))}
          </select>
          <button
            onClick={handleApply}
            disabled={applying || filled === 0}
            className="flex h-8 items-center gap-1.5 rounded bg-amber-500 px-3 text-xs font-medium text-zinc-950 transition-colors hover:bg-amber-400 disabled:opacity-40"
          >
            <Play className="h-3 w-3" strokeWidth={2.5} />
            {applying ? "Applying…" : "Apply Mix"}
          </button>
          <button
            onClick={load}
            disabled={loading}
            className="flex h-8 w-8 items-center justify-center rounded text-zinc-600 transition-colors hover:bg-zinc-800 hover:text-zinc-400 disabled:opacity-40"
          >
            <RefreshCw className={cn("h-3.5 w-3.5", loading && "animate-spin")} strokeWidth={1.75} />
          </button>
        </div>
      </div>

      {error && (
        <div className="mx-6 mt-3 flex items-start gap-2 rounded border border-red-500/20 bg-red-500/10 px-3 py-2.5 text-xs text-red-400">
          <AlertCircle className="mt-px h-3.5 w-3.5 shrink-0" strokeWidth={1.75} />
          <span className="break-all">{error}</span>
          <button onClick={() => setError(null)} className="ml-auto shrink-0 hover:text-red-300">✕</button>
        </div>
      )}

      {result && (
        <div className="mx-6 mt-3 flex items-center gap-2 rounded border border-amber-500/20 bg-amber-500/10 px-3 py-2 text-xs text-amber-400">
          <CheckCircle2 className="h-3.5 w-3.5 shrink-0" strokeWidth={1.75} />
          Applied — {result.written} cursor role{result.written !== 1 ? "s" : ""} written,
          {" "}{result.cleared.length} reset to Windows defaults.
        </div>
      )}

      {mix.stale.length > 0 && (
        <div className="mx-6 mt-3 rounded border border-zinc-700/50 bg-zinc-800/40 px-3 py-2 text-xs text-zinc-500">
          {mix.stale.length} cursor{mix.stale.length !== 1 ? "s" : ""} unavailable — pack deleted.
        </div>
      )}

      {/* Two panels */}
      <div className="flex flex-1 gap-4 overflow-hidden p-6">
        {/* Roles */}
        <div className="flex w-[280px] shrink-0 flex-col gap-1 overflow-y-auto">
          {CURSOR_ROLES.map((role) => {
            const entry = byRole[role.reg];
            const thumb = thumbFor(entry);
            const selected = selectedRole === role.reg;
            return (
              <button
                key={role.reg}
                onClick={() => setSelectedRole(role.reg)}
                className={cn(
                  "flex items-center gap-2.5 rounded-sm border px-2.5 py-2 text-left transition-colors",
                  selected
                    ? "border-amber-500/40 bg-amber-500/10"
                    : "border-zinc-800 bg-zinc-900 hover:border-zinc-700",
                )}
              >
                <div className="flex h-7 w-7 shrink-0 items-center justify-center rounded bg-zinc-950/60">
                  {thumb ? (
                    <img
                      src={`data:image/png;base64,${thumb}`}
                      alt=""
                      className="h-5 w-5 object-contain"
                      style={{ imageRendering: "pixelated" }}
                    />
                  ) : (
                    <role.Icon className={cn("h-3.5 w-3.5", role.color)} strokeWidth={1.75} />
                  )}
                </div>
                <div className="min-w-0 flex-1">
                  <p className="truncate text-xs font-medium text-zinc-200">{role.label}</p>
                  <p className="truncate font-mono text-[10px] text-zinc-600">
                    {entry ? entry.file : "empty"}
                  </p>
                </div>
                {entry && (
                  <span
                    role="button"
                    tabIndex={0}
                    onClick={(e) => { e.stopPropagation(); clearRole(role.reg); }}
                    onKeyDown={(e) => { if (e.key === "Enter") { e.stopPropagation(); clearRole(role.reg); } }}
                    className="shrink-0 px-1 text-zinc-600 hover:text-zinc-300"
                  >
                    ✕
                  </span>
                )}
              </button>
            );
          })}
        </div>

        {/* Gallery */}
        <div className="flex-1 overflow-y-auto rounded-sm border border-zinc-800 bg-zinc-900/40 p-4">
          {!selectedRole ? (
            <div className="flex h-full flex-col items-center justify-center gap-3 text-zinc-600">
              <Shuffle className="h-6 w-6" strokeWidth={1.5} />
              <p className="text-sm">Select a role first</p>
            </div>
          ) : (
            library.map((p) => (
              <div key={p.pack.id} className="mb-5">
                <p className="mb-2 font-mono text-[10px] uppercase tracking-[0.14em] text-zinc-600">
                  {p.pack.name}
                </p>
                <div className="flex flex-wrap gap-2">
                  {p.cursors.map((c) => (
                    <button
                      key={c.name}
                      onClick={() => assign(p.pack.id, c.name)}
                      title={c.name}
                      className="flex h-14 w-14 items-center justify-center rounded border border-zinc-800 bg-zinc-950/60 transition-colors hover:border-amber-500/40"
                    >
                      {c.thumbnail ? (
                        <img
                          src={`data:image/png;base64,${c.thumbnail}`}
                          alt={c.name}
                          className="h-8 w-8 object-contain"
                          style={{ imageRendering: "pixelated" }}
                        />
                      ) : (
                        <span className="font-mono text-[9px] text-zinc-700">?</span>
                      )}
                    </button>
                  ))}
                </div>
              </div>
            ))
          )}
        </div>
      </div>
    </div>
  );
}
