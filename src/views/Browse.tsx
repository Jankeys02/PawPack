import { useState, useEffect, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import {
  Layers,
  Plus,
  Trash2,
  MousePointer2,
  RefreshCw,
  AlertCircle,
  CheckCircle2,
  ChevronDown,
  FileArchive,
  FolderOpen,
} from "lucide-react";
import { cn } from "@/lib/utils";
import { groupPacks, variantLabel, type PackGroup, type PackMeta } from "@/types";
import { Button } from "@/components/ui/button";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";

// ── Types ─────────────────────────────────────────────────────────────────────


// ── Sub-components ─────────────────────────────────────────────────────────────

function PlatformBadge({ platform }: { platform: PackMeta["platform"] }) {
  const styles = {
    windows: "bg-sky-500/10 text-sky-400 border-sky-500/20",
    linux:   "bg-emerald-500/10 text-emerald-400 border-emerald-500/20",
    unknown: "bg-zinc-500/10 text-zinc-500 border-zinc-500/20",
  } as const;

  const labels = { windows: "Windows", linux: "Linux", unknown: "Unknown" } as const;

  return (
    <span
      className={cn(
        "inline-flex items-center rounded border px-1.5 py-px font-mono text-[10px] uppercase tracking-wide",
        styles[platform],
      )}
    >
      {labels[platform]}
    </span>
  );
}

function CursorThumbnails({ packId }: { packId: string }) {
  const [srcs, setSrcs] = useState<string[]>([]);
  const [loaded, setLoaded] = useState(false);

  useEffect(() => {
    let cancelled = false;
    invoke<string[]>("get_pack_thumbnails", { packId, limit: 9 })
      .then((b64s) => {
        if (!cancelled) {
          setSrcs(b64s.map((b) => `data:image/png;base64,${b}`));
          setLoaded(true);
        }
      })
      .catch(() => { if (!cancelled) setLoaded(true); });
    return () => { cancelled = true; };
  }, [packId]);

  if (loaded && srcs.length === 0) return null;

  return (
    // Scrolls horizontally rather than overflowing the card. The inner
    // `w-max mx-auto` is what centres a short strip while leaving a long one
    // fully reachable — `justify-center` on the scroller itself would push the
    // first thumbnails past the left edge, where they cannot be scrolled back
    // into view.
    <div className="h-16 overflow-x-auto rounded bg-zinc-950/60 px-2 [scrollbar-width:thin]">
      <div className="mx-auto flex h-full w-max items-center gap-2">
        {!loaded
          ? Array.from({ length: 9 }).map((_, i) => (
              <div key={i} className="h-8 w-8 shrink-0 animate-pulse rounded bg-zinc-800" />
            ))
          : srcs.map((src, i) => (
              <img
                key={i}
                src={src}
                alt=""
                className="h-10 w-10 shrink-0 object-contain"
                style={{ imageRendering: "pixelated" }}
              />
            ))}
      </div>
    </div>
  );
}

/// A variant switcher. Only rendered when a download held more than one set,
/// so an ordinary pack's card is unchanged.
function VariantChips({
  packs,
  selected,
  onSelect,
}: {
  packs: PackMeta[];
  selected: PackMeta;
  onSelect: (pack: PackMeta) => void;
}) {
  return (
    <div className="flex flex-wrap gap-1" onClick={(e) => e.stopPropagation()}>
      {packs.map((p) => (
        <button
          key={p.id}
          onClick={() => onSelect(p)}
          aria-pressed={p.id === selected.id}
          className={cn(
            "rounded-sm border px-1.5 py-0.5 text-[11px] transition-colors",
            p.id === selected.id
              ? "border-amber-500/40 bg-amber-500/15 text-amber-300"
              : "border-zinc-800 text-zinc-500 hover:border-zinc-700 hover:text-zinc-300",
          )}
        >
          {variantLabel(p)}
        </button>
      ))}
    </div>
  );
}

function PackCard({
  group,
  onDelete,
  onSelect,
}: {
  group: PackGroup;
  onDelete: (id: string) => void;
  onSelect: (pack: PackMeta) => void;
}) {
  const [confirming, setConfirming] = useState(false);
  const [selectedId, setSelectedId] = useState(group.packs[0].id);

  // A deleted variant leaves a stale id behind, so fall back rather than
  // rendering nothing.
  const pack = group.packs.find((p) => p.id === selectedId) ?? group.packs[0];
  const grouped = group.packs.length > 1;

  const handleDelete = (e: React.MouseEvent) => {
    e.stopPropagation();
    if (!confirming) {
      setConfirming(true);
      setTimeout(() => setConfirming(false), 2500);
    } else {
      onDelete(pack.id);
    }
  };

  const date = new Date(pack.imported_at * 1000).toLocaleDateString(undefined, {
    year: "numeric",
    month: "short",
    day: "numeric",
  });

  return (
    <div
      onClick={() => onSelect(pack)}
      className="group flex flex-col gap-3 rounded-sm border border-zinc-800 bg-zinc-900 p-4 transition-colors hover:border-zinc-700 cursor-pointer"
    >
      {/* Thumbnails — keyed on the pack so switching variant re-fetches */}
      <CursorThumbnails key={pack.id} packId={pack.id} />

      {/* Header */}
      <div className="flex items-start justify-between gap-2">
        <div className="min-w-0">
          <p className="truncate text-sm font-semibold text-zinc-100">
            {grouped ? group.title : pack.name}
          </p>
          {pack.author && (
            <p className="mt-0.5 truncate text-xs text-zinc-500">{pack.author}</p>
          )}
        </div>
        <PlatformBadge platform={pack.platform} />
      </div>

      {grouped && (
        <VariantChips packs={group.packs} selected={pack} onSelect={(p) => setSelectedId(p.id)} />
      )}

      {/* Description */}
      {pack.description && (
        <p className="line-clamp-2 text-xs leading-relaxed text-zinc-500">
          {pack.description}
        </p>
      )}

      {/* Footer */}
      <div className="mt-auto flex items-center justify-between gap-2">
        <div className="flex items-center gap-3">
          <span className="flex items-center gap-1 text-xs text-zinc-600">
            <MousePointer2 className="h-3 w-3" strokeWidth={1.5} />
            {pack.cursor_count > 0 ? `${pack.cursor_count} cursors` : "unknown count"}
          </span>
          <span className="text-xs text-zinc-700">{date}</span>
        </div>

        <button
          onClick={(e) => handleDelete(e)}
          className={cn(
            "flex items-center gap-1 rounded px-2 py-1 text-xs transition-all",
            confirming
              ? "bg-red-500/15 text-red-400 hover:bg-red-500/25"
              : "text-zinc-700 opacity-0 hover:bg-zinc-800 hover:text-zinc-400 group-hover:opacity-100",
          )}
        >
          <Trash2 className="h-3 w-3" strokeWidth={1.5} />
          {confirming ? "Confirm?" : grouped ? `Delete ${variantLabel(pack)}` : "Delete"}
        </button>
      </div>
    </div>
  );
}

function EmptyState({ onImport }: { onImport: (mode: "zip" | "folder") => void }) {
  return (
    <div className="flex flex-1 flex-col items-center justify-center gap-5">
      <div className="flex h-16 w-16 items-center justify-center rounded-lg border border-zinc-800 bg-zinc-900">
        <Layers className="h-7 w-7 text-zinc-700" strokeWidth={1.25} />
      </div>
      <div className="text-center">
        <p className="text-sm font-semibold text-zinc-400">No cursor packs yet</p>
        <p className="mt-1 max-w-xs text-xs leading-relaxed text-zinc-600">
          Import a <span className="text-zinc-500">.zip</span> archive or a folder
          containing <span className="text-zinc-500">.cur</span> /{" "}
          <span className="text-zinc-500">.ani</span> files (Windows) or an X11
          Xcursor theme (Linux).
        </p>
      </div>
      <div className="flex gap-2">
        <Button
          variant="outline"
          size="sm"
          onClick={() => onImport("zip")}
          className="gap-2 border-zinc-700 bg-zinc-900 text-zinc-300 hover:border-zinc-600 hover:bg-zinc-800"
        >
          <FileArchive className="h-3.5 w-3.5" />
          Import .zip
        </Button>
        <Button
          variant="outline"
          size="sm"
          onClick={() => onImport("folder")}
          className="gap-2 border-zinc-700 bg-zinc-900 text-zinc-300 hover:border-zinc-600 hover:bg-zinc-800"
        >
          <FolderOpen className="h-3.5 w-3.5" />
          Import folder
        </Button>
      </div>
    </div>
  );
}

// ── Main view ─────────────────────────────────────────────────────────────────

export default function Browse({ onSelect }: { onSelect: (pack: PackMeta) => void }) {
  const [packs, setPacks] = useState<PackMeta[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  // Set only when one download produced several packs, which is surprising
  // enough to say out loud.
  const [notice, setNotice] = useState<string | null>(null);
  const [importing, setImporting] = useState(false);

  const loadPacks = useCallback(async () => {
    setLoading(true);
    setError(null);
    setNotice(null);
    try {
      const result = await invoke<PackMeta[]>("list_packs");
      setPacks(result);
    } catch (e) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    loadPacks();
  }, [loadPacks]);

  const handleImport = async (mode: "zip" | "folder") => {
    const selected = await open(
      mode === "zip"
        ? { directory: false, multiple: false, title: "Select Cursor Pack (.zip)", filters: [{ name: "Zip archive", extensions: ["zip"] }] }
        : { directory: true,  multiple: false, title: "Select Cursor Pack Folder" },
    );
    if (!selected) return;

    setImporting(true);
    try {
      // A download may hold several packs — animated and static sets, or
      // colour variants — so import_pack returns every one it found.
      const added = await invoke<PackMeta[]>("import_pack", { sourcePath: selected });
      setPacks((prev) => [...added, ...prev]);
      if (added.length > 1) {
        setNotice(`Imported ${added.length} packs: ${added.map((p) => p.name).join(", ")}`);
      }
    } catch (e) {
      setError(String(e));
    } finally {
      setImporting(false);
    }
  };

  const handleDelete = async (id: string) => {
    try {
      await invoke("delete_pack", { packId: id });
      setPacks((prev) => prev.filter((p) => p.id !== id));
    } catch (e) {
      setError(String(e));
    }
  };

  return (
    <div className="flex flex-1 flex-col overflow-hidden">
      {/* Header */}
      <div className="flex items-center justify-between border-b border-zinc-800/60 px-6 py-4">
        <div>
          <h1 className="text-sm font-semibold text-zinc-100">Cursor Packs</h1>
          {!loading && (
            <p className="text-xs text-zinc-600">
              {packs.length === 0
                ? "No packs imported"
                : `${packs.length} pack${packs.length === 1 ? "" : "s"} imported`}
            </p>
          )}
        </div>

        <div className="flex items-center gap-2">
          <button
            onClick={loadPacks}
            disabled={loading}
            className="flex h-8 w-8 items-center justify-center rounded text-zinc-600 transition-colors hover:bg-zinc-800 hover:text-zinc-400 disabled:opacity-40"
          >
            <RefreshCw className={cn("h-3.5 w-3.5", loading && "animate-spin")} strokeWidth={1.75} />
          </button>

          <DropdownMenu>
            <DropdownMenuTrigger
              disabled={importing}
              render={
                <Button
                  size="sm"
                  className="gap-2 bg-amber-500 text-zinc-950 hover:bg-amber-400 disabled:opacity-60"
                />
              }
            >
              <Plus className="h-3.5 w-3.5" strokeWidth={2.5} />
              {importing ? "Importing…" : "Import Pack"}
              <ChevronDown className="h-3 w-3 opacity-70" strokeWidth={2.5} />
            </DropdownMenuTrigger>
            <DropdownMenuContent align="end" className="w-44">
              <DropdownMenuItem onClick={() => handleImport("zip")} className="gap-2">
                <FileArchive className="h-3.5 w-3.5" strokeWidth={1.75} />
                Import .zip
              </DropdownMenuItem>
              <DropdownMenuItem onClick={() => handleImport("folder")} className="gap-2">
                <FolderOpen className="h-3.5 w-3.5" strokeWidth={1.75} />
                Import folder
              </DropdownMenuItem>
            </DropdownMenuContent>
          </DropdownMenu>
        </div>
      </div>

      {/* Error banner */}
      {notice && (
        <div className="mx-6 mt-4 flex items-start gap-2 rounded border border-amber-500/20 bg-amber-500/10 px-3 py-2.5 text-xs text-amber-300">
          <CheckCircle2 className="mt-px h-3.5 w-3.5 shrink-0" strokeWidth={1.75} />
          <span>{notice}</span>
          <button
            onClick={() => setNotice(null)}
            className="ml-auto shrink-0 text-amber-500 hover:text-amber-300"
          >
            ✕
          </button>
        </div>
      )}

      {error && (
        <div className="mx-6 mt-4 flex items-start gap-2 rounded border border-red-500/20 bg-red-500/10 px-3 py-2.5 text-xs text-red-400">
          <AlertCircle className="mt-px h-3.5 w-3.5 shrink-0" strokeWidth={1.75} />
          <span>{error}</span>
          <button
            onClick={() => setError(null)}
            className="ml-auto shrink-0 text-red-500 hover:text-red-300"
          >
            ✕
          </button>
        </div>
      )}

      {/* Body */}
      {loading ? (
        <div className="flex flex-1 items-center justify-center">
          <RefreshCw className="h-5 w-5 animate-spin text-zinc-700" />
        </div>
      ) : packs.length === 0 ? (
        <EmptyState onImport={handleImport} />
      ) : (
        <div className="flex-1 overflow-y-auto p-6">
          <div className="grid grid-cols-2 gap-3 xl:grid-cols-3">
            {groupPacks(packs).map((group) => (
              <PackCard
                key={group.key}
                group={group}
                onDelete={handleDelete}
                onSelect={onSelect}
              />
            ))}
          </div>
        </div>
      )}
    </div>
  );
}
