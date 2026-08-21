import type { PackMeta } from "@/types";
import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { ArrowLeft, MousePointer2, RefreshCw, AlertCircle } from "lucide-react";
import { cn } from "@/lib/utils";
import CursorThumb from "@/components/CursorThumb";

// ── Types ─────────────────────────────────────────────────────────────────────


interface CursorEntry {
  name: string;
  kind: string;
  thumbnail: string;
  still: string;
}

// ── Sub-components ─────────────────────────────────────────────────────────────

function CursorTile({ entry }: { entry: CursorEntry }) {
  const stem = entry.name.replace(/\.(cur|ani)$/i, "");
  const isAni = entry.kind === "ani";

  return (
    <div className="group flex flex-col items-center gap-2 rounded-sm border border-zinc-800 bg-zinc-900 p-3 transition-colors hover:border-zinc-700">
      <div className="flex h-14 w-14 items-center justify-center rounded bg-zinc-950/60">
        {entry.thumbnail ? (
          // badge off: this tile already spells out "ani" underneath.
          <CursorThumb entry={entry} className="h-10 w-10" badge={false} />
        ) : (
          <MousePointer2 className="h-6 w-6 text-zinc-700" strokeWidth={1.25} />
        )}
      </div>
      <p className="w-full truncate text-center text-[11px] text-zinc-400" title={stem}>
        {stem}
      </p>
      {isAni && (
        <span className="rounded border border-amber-500/20 bg-amber-500/10 px-1 py-px font-mono text-[9px] uppercase tracking-wide text-amber-400">
          ani
        </span>
      )}
    </div>
  );
}

// ── Main view ─────────────────────────────────────────────────────────────────

export default function PackDetail({
  pack,
  onBack,
}: {
  pack: PackMeta;
  onBack: () => void;
}) {
  const [cursors, setCursors] = useState<CursorEntry[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    setLoading(true);
    setError(null);
    invoke<CursorEntry[]>("list_pack_cursors", { packId: pack.id })
      .then((entries) => {
        if (!cancelled) {
          setCursors(entries);
          setLoading(false);
        }
      })
      .catch((e) => {
        if (!cancelled) {
          setError(String(e));
          setLoading(false);
        }
      });
    return () => { cancelled = true; };
  }, [pack.id]);

  return (
    <div className="flex flex-1 flex-col overflow-hidden">
      {/* Header */}
      <div className="flex items-center gap-3 border-b border-zinc-800/60 px-6 py-4">
        <button
          onClick={onBack}
          className="flex h-7 w-7 items-center justify-center rounded text-zinc-500 transition-colors hover:bg-zinc-800 hover:text-zinc-300"
        >
          <ArrowLeft className="h-4 w-4" strokeWidth={1.75} />
        </button>

        <div className="min-w-0 flex-1">
          <h1 className="truncate text-sm font-semibold text-zinc-100">{pack.name}</h1>
          {pack.author && (
            <p className="truncate text-xs text-zinc-500">{pack.author}</p>
          )}
        </div>

        {!loading && (
          <span className={cn(
            "flex items-center gap-1 text-xs",
            cursors.length > 0 ? "text-zinc-500" : "text-zinc-700",
          )}>
            <MousePointer2 className="h-3 w-3" strokeWidth={1.5} />
            {cursors.length} cursor{cursors.length !== 1 ? "s" : ""}
          </span>
        )}
      </div>

      {/* Error banner */}
      {error && (
        <div className="mx-6 mt-4 flex items-start gap-2 rounded border border-red-500/20 bg-red-500/10 px-3 py-2.5 text-xs text-red-400">
          <AlertCircle className="mt-px h-3.5 w-3.5 shrink-0" strokeWidth={1.75} />
          <span>{error}</span>
        </div>
      )}

      {/* Body */}
      {loading ? (
        <div className="flex flex-1 items-center justify-center">
          <RefreshCw className="h-5 w-5 animate-spin text-zinc-700" />
        </div>
      ) : cursors.length === 0 ? (
        <div className="flex flex-1 flex-col items-center justify-center gap-3">
          <MousePointer2 className="h-8 w-8 text-zinc-700" strokeWidth={1.25} />
          <p className="text-sm text-zinc-600">No cursor files found in this pack</p>
        </div>
      ) : (
        <div className="flex-1 overflow-y-auto p-6">
          <div className="grid grid-cols-4 gap-2 sm:grid-cols-5 md:grid-cols-6 xl:grid-cols-8">
            {cursors.map((entry) => (
              <CursorTile key={entry.name} entry={entry} />
            ))}
          </div>
        </div>
      )}
    </div>
  );
}
