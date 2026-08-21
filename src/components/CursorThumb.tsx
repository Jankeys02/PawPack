import { useState } from "react";
import { cn } from "@/lib/utils";
import { cursorHoverOnly, motionLevel } from "@/lib/motion";

/// The two images the backend hands over for one cursor, plus what it is.
export interface Thumb {
  name: string;
  kind: string; // "cur" | "ani"
  /// APNG for `.ani`. Plays the moment it decodes — no CSS can pause it.
  thumbnail: string;
  /// One frozen frame. Same as `thumbnail` for `.cur`.
  still: string;
}

/**
 * One cursor thumbnail that animates only when it should.
 *
 * APNG cannot be paused, so holding still means rendering a genuinely
 * different image and swapping to the animation on hover. When a cursor is
 * being held still, a `.ani` tag marks it as having something to show —
 * otherwise an animated cursor is indistinguishable from a static one.
 *
 * ponytail: the preference is read during render rather than subscribed to, so
 * a view already on screen keeps its old behaviour until it remounts. Settings
 * is its own tab, so switching back always remounts. Give motion.ts a change
 * event if the setting ever moves somewhere visible at the same time.
 */
export default function CursorThumb({
  entry,
  className,
  badge = true,
}: {
  entry: Thumb;
  className?: string;
  /// Off where the surrounding tile already labels `.ani` itself.
  badge?: boolean;
}) {
  const [hovered, setHovered] = useState(false);

  // Falling back to the animation keeps a thumbnail visible if `still` is ever
  // missing — a frontend running ahead of a not-yet-rebuilt backend would
  // otherwise render a broken image rather than a playing one.
  const still = entry.still || entry.thumbnail;
  const animated = entry.kind === "ani" && still !== entry.thumbnail;
  // "off" freezes everything; otherwise the hover-only setting decides.
  const mayAnimate = motionLevel() !== "off" && (!cursorHoverOnly() || hovered);
  const playing = animated && mayAnimate;

  if (!entry.thumbnail) {
    return <span className="font-mono text-[9px] text-zinc-700">?</span>;
  }

  return (
    <span
      className="relative inline-flex shrink-0"
      onMouseEnter={() => setHovered(true)}
      onMouseLeave={() => setHovered(false)}
    >
      <img
        src={`data:image/png;base64,${playing ? entry.thumbnail : still}`}
        alt={entry.name}
        // 32px boxes elsewhere in the app, so the common 32x32 cursor lands 1:1
        // and is never resampled.
        className={cn("object-contain", className)}
        style={{ imageRendering: "pixelated" }}
      />
      {badge && animated && !playing && (
        <span className="pointer-events-none absolute -bottom-1 -right-1 rounded-sm bg-zinc-800 px-0.5 font-mono text-[8px] leading-[1.3] text-zinc-400">
          ani
        </span>
      )}
    </span>
  );
}
