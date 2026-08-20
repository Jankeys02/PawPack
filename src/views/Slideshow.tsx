import { useState, useEffect, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import {
  RefreshCw,
  AlertCircle,
  Play,
  Square,
  Clapperboard,
  CalendarClock,
  Trash2,
} from "lucide-react";
import { cn } from "@/lib/utils";
import { CURSOR_ROLES } from "@/lib/roles";
import CursorGallery, { type CursorEntry, type PackCursors } from "@/components/CursorGallery";

// ── Types ─────────────────────────────────────────────────────────────────────

interface PackMeta { id: string; name: string }

/** One slide. Mirrors `SlideRef` in slideshow.rs. */
interface SlideRef { pack: string; file: string }

interface StaleEntry { role: string; pack: string; file: string }

/** Mirrors `SlideshowState` in lib.rs. */
interface SlideshowState {
  enabled: boolean;
  interval_minutes: number;
  stop_on_apply: boolean;
  task_registered: boolean;
  task_name: string;
  roles: Record<string, SlideRef[]>;
  stale: StaleEntry[];
}

// ── Main view ─────────────────────────────────────────────────────────────────

export default function Slideshow() {
  const [state, setState] = useState<SlideshowState | null>(null);
  const [library, setLibrary] = useState<PackCursors[]>([]);
  const [selectedRole, setSelectedRole] = useState<string | null>(null);
  const [interval, setIntervalValue] = useState(10);
  const [loading, setLoading] = useState(true);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const load = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const packs = await invoke<PackMeta[]>("list_packs");
      setLibrary(
        await Promise.all(
          packs.map(async (pack) => ({
            pack,
            cursors: await invoke<CursorEntry[]>("list_pack_cursors", { packId: pack.id }),
          })),
        ),
      );
      const s = await invoke<SlideshowState>("get_slideshow");
      setState(s);
      setIntervalValue(s.interval_minutes);
    } catch (e) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => { load(); }, [load]);

  const playlist = (role: string): SlideRef[] => state?.roles[role] ?? [];
  const totalSlides = state
    ? Object.values(state.roles).reduce((n, items) => n + items.length, 0)
    : 0;
  const rolesInPlay = state ? Object.values(state.roles).filter((i) => i.length > 0).length : 0;

  /** Add a cursor to the selected role's playlist, or remove it if already in. */
  const toggleSlide = async (packId: string, file: string) => {
    if (!selectedRole) return;
    const current = playlist(selectedRole);
    const without = current.filter((s) => !(s.pack === packId && s.file === file));
    const items = without.length < current.length ? without : [...current, { pack: packId, file }];
    try {
      setState(await invoke<SlideshowState>("set_slideshow_role", { role: selectedRole, items }));
    } catch (e) {
      setError(String(e));
    }
  };

  const clearRole = async (role: string) => {
    try {
      setState(await invoke<SlideshowState>("set_slideshow_role", { role, items: [] }));
    } catch (e) {
      setError(String(e));
    }
  };

  const toggleRunning = async () => {
    if (!state) return;
    setBusy(true);
    setError(null);
    try {
      setState(await invoke<SlideshowState>("set_slideshow", {
        enabled: !state.enabled,
        intervalMinutes: Math.max(1, interval),
      }));
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  };

  const removeTask = async () => {
    setBusy(true);
    setError(null);
    try {
      setState(await invoke<SlideshowState>("remove_slideshow_task"));
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  };

  const running = state?.enabled ?? false;

  return (
    <div className="flex flex-1 flex-col overflow-hidden">
      {/* Header */}
      <div className="flex items-center justify-between border-b border-zinc-800/60 px-6 py-4">
        <div>
          <h1 className="flex items-center gap-2 text-sm font-semibold text-zinc-100">
            Slideshow
            {running && (
              <span className="inline-flex items-center gap-1 rounded border border-amber-500/30 bg-amber-500/10 px-1.5 py-px font-mono text-[10px] uppercase tracking-wide text-amber-400">
                <span className="h-1.5 w-1.5 animate-pulse rounded-full bg-amber-400" />
                Rotating
              </span>
            )}
          </h1>
          <p className="text-xs text-zinc-500">
            {totalSlides} cursor{totalSlides !== 1 ? "s" : ""} across {rolesInPlay} role
            {rolesInPlay !== 1 ? "s" : ""}
            {rolesInPlay > 0 && <span className="text-zinc-600"> — the rest keep your mix</span>}
          </p>
        </div>
        <div className="flex items-center gap-2">
          <label className="flex items-center gap-1.5 text-xs text-zinc-500">
            every
            <input
              type="number"
              min={1}
              value={interval}
              disabled={running}
              onChange={(e) => setIntervalValue(Math.max(1, Number(e.target.value) || 1))}
              className="h-8 w-14 rounded border border-zinc-700 bg-zinc-800 px-2 text-xs text-zinc-300 disabled:opacity-40"
            />
            min
          </label>
          <button
            onClick={toggleRunning}
            disabled={busy || totalSlides === 0}
            className={cn(
              "flex h-8 items-center gap-1.5 rounded px-3 text-xs font-medium transition-colors disabled:opacity-40",
              running
                ? "border border-zinc-700 text-zinc-300 hover:bg-zinc-800"
                : "bg-amber-500 text-zinc-950 hover:bg-amber-400",
            )}
          >
            {running ? (
              <><Square className="h-3 w-3" strokeWidth={2.5} />Stop</>
            ) : (
              <><Play className="h-3 w-3" strokeWidth={2.5} />{busy ? "Starting…" : "Start"}</>
            )}
          </button>
          <button
            onClick={load}
            disabled={loading}
            aria-label="Reload"
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

      {/* Background task disclosure — never leave this running unannounced. */}
      {state && (
        <div className="mx-6 mt-3 flex items-center gap-2 rounded border border-zinc-800 bg-zinc-900/60 px-3 py-2 text-[11px] text-zinc-500">
          <CalendarClock className="h-3.5 w-3.5 shrink-0 text-zinc-600" strokeWidth={1.75} />
          <span>
            {state.task_registered ? (
              <>
                A Windows scheduled task named{" "}
                <span className="font-mono text-zinc-400">{state.task_name}</span> runs every{" "}
                {state.interval_minutes} min and keeps rotating after PawPack is closed.
              </>
            ) : (
              <>
                No background task registered. Rotation only happens while a task exists — one
                minute is the shortest interval Windows allows.
              </>
            )}
          </span>
          {state.task_registered && (
            <button
              onClick={removeTask}
              disabled={busy}
              className="ml-auto flex shrink-0 items-center gap-1 rounded px-1.5 py-0.5 text-zinc-500 transition-colors hover:bg-zinc-800 hover:text-zinc-300 disabled:opacity-40"
            >
              <Trash2 className="h-3 w-3" strokeWidth={1.75} />
              Remove background task
            </button>
          )}
        </div>
      )}

      {state && state.stale.length > 0 && (
        <div className="mx-6 mt-3 rounded border border-zinc-700/50 bg-zinc-800/40 px-3 py-2 text-xs text-zinc-500">
          {state.stale.length} slide{state.stale.length !== 1 ? "s" : ""} unavailable — pack
          deleted. They are skipped, and come back if the pack is reimported.
        </div>
      )}

      {/* Two panels */}
      <div className="flex flex-1 gap-4 overflow-hidden p-6">
        {/* Roles */}
        <div className="flex w-[280px] shrink-0 flex-col gap-1 overflow-y-auto">
          {CURSOR_ROLES.map((role) => {
            const items = playlist(role.reg);
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
                  <role.Icon className={cn("h-3.5 w-3.5", role.color)} strokeWidth={1.75} />
                </div>
                <div className="min-w-0 flex-1">
                  <p className="truncate text-xs font-medium text-zinc-200">{role.label}</p>
                  <p className="truncate font-mono text-[10px] text-zinc-600">
                    {items.length === 0
                      ? "not rotating"
                      : `${items.length} cursor${items.length !== 1 ? "s" : ""} in cycle`}
                  </p>
                </div>
                {items.length > 0 && (
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

        {/* Gallery — click to add or remove; the badge is the cycle position. */}
        <div className="flex-1 overflow-y-auto rounded-sm border border-zinc-800 bg-zinc-900/40 p-4">
          <CursorGallery
            library={library}
            onPick={toggleSlide}
            badgeFor={(packId, file) => {
              if (!selectedRole) return null;
              const i = playlist(selectedRole).findIndex(
                (s) => s.pack === packId && s.file === file,
              );
              return i === -1 ? null : i + 1;
            }}
            empty={
              !selectedRole ? (
                <>
                  <Clapperboard className="h-6 w-6" strokeWidth={1.5} />
                  <p className="text-sm">Pick a role, then click cursors to build its cycle</p>
                </>
              ) : undefined
            }
          />
        </div>
      </div>
    </div>
  );
}
