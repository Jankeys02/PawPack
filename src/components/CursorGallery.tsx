import { cn } from "@/lib/utils";

export interface CursorEntry {
  name: string;
  kind: string;
  /** base64 PNG; empty string when decoding failed. */
  thumbnail: string;
}

export interface PackCursors {
  pack: { id: string; name: string };
  cursors: CursorEntry[];
}

/**
 * Every cursor in the library, grouped by pack, as clickable tiles.
 *
 * Shared by Mix (pick one cursor for a role) and Slideshow (toggle cursors in
 * and out of a role's playlist) — `badgeFor` is what separates the two: return
 * a cycle position to mark a tile as selected, or null to leave it plain.
 */
export default function CursorGallery({
  library,
  onPick,
  badgeFor,
  empty,
}: {
  library: PackCursors[];
  onPick: (packId: string, file: string) => void;
  badgeFor?: (packId: string, file: string) => number | null;
  empty?: React.ReactNode;
}) {
  if (empty) {
    return (
      <div className="flex h-full flex-col items-center justify-center gap-3 text-zinc-600">
        {empty}
      </div>
    );
  }

  return (
    <>
      {library.map((p) => (
        <div key={p.pack.id} className="mb-5">
          <p className="mb-2 font-mono text-[10px] uppercase tracking-[0.14em] text-zinc-600">
            {p.pack.name}
          </p>
          <div className="flex flex-wrap gap-2">
            {p.cursors.map((c) => {
              const badge = badgeFor?.(p.pack.id, c.name) ?? null;
              return (
                <button
                  key={c.name}
                  onClick={() => onPick(p.pack.id, c.name)}
                  title={c.name}
                  className={cn(
                    "relative flex h-14 w-14 items-center justify-center rounded border bg-zinc-950/60 transition-colors",
                    badge !== null
                      ? "border-amber-500 bg-amber-500/10"
                      : "border-zinc-800 hover:border-amber-500/40",
                  )}
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
                  {badge !== null && (
                    <span className="absolute -right-1 -top-1 flex h-4 w-4 items-center justify-center rounded-full bg-amber-500 font-mono text-[9px] font-semibold text-zinc-950">
                      {badge}
                    </span>
                  )}
                </button>
              );
            })}
          </div>
        </div>
      ))}
    </>
  );
}
