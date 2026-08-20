import {
  ArrowUp, MoveHorizontal, MoveVertical, Move, Type, MousePointer2, Hand,
  Ban, HelpCircle, Crosshair, Clock, Loader, MoveUpRight, MoveDownRight,
  PenLine, MapPin, UserRound,
} from "lucide-react";

export interface CursorRole {
  /** Registry value name under HKCU\Control Panel\Cursors. */
  reg: string;
  /** Windows' own name for this cursor, as shown in Mouse Properties. */
  label: string;
  Icon: React.ComponentType<{ className?: string; strokeWidth?: number }>;
  color: string;
}

/// The 17 roles, in the order Windows lists them in Mouse Properties.
export const CURSOR_ROLES: CursorRole[] = [
  { reg: "Arrow",       label: "Normal select",        Icon: MousePointer2,  color: "text-zinc-400"    },
  { reg: "Help",        label: "Help select",          Icon: HelpCircle,     color: "text-teal-400"    },
  { reg: "AppStarting", label: "Working in background", Icon: Loader,        color: "text-yellow-400"  },
  { reg: "Wait",        label: "Busy",                 Icon: Clock,          color: "text-orange-400"  },
  { reg: "Crosshair",   label: "Precision select",     Icon: Crosshair,      color: "text-emerald-400" },
  { reg: "IBeam",       label: "Text select",          Icon: Type,           color: "text-sky-400"     },
  { reg: "NWPen",       label: "Handwriting",          Icon: PenLine,        color: "text-fuchsia-400" },
  { reg: "No",          label: "Unavailable",          Icon: Ban,            color: "text-red-400"     },
  { reg: "SizeNS",      label: "Vertical resize",      Icon: MoveVertical,   color: "text-cyan-400"    },
  { reg: "SizeWE",      label: "Horizontal resize",    Icon: MoveHorizontal, color: "text-cyan-400"    },
  { reg: "SizeNWSE",    label: "Diagonal resize ↖↘",   Icon: MoveDownRight,  color: "text-indigo-400"  },
  { reg: "SizeNESW",    label: "Diagonal resize ↗↙",   Icon: MoveUpRight,    color: "text-indigo-400"  },
  { reg: "SizeAll",     label: "Move",                 Icon: Move,           color: "text-violet-400"  },
  { reg: "UpArrow",     label: "Alternate select",     Icon: ArrowUp,        color: "text-lime-400"    },
  { reg: "Hand",        label: "Link select",          Icon: Hand,           color: "text-amber-400"   },
  { reg: "Pin",         label: "Location select",      Icon: MapPin,         color: "text-rose-400"    },
  { reg: "Person",      label: "Person select",        Icon: UserRound,      color: "text-sky-300"     },
];

export const ROLE_LABELS: Record<string, string> = Object.fromEntries(
  CURSOR_ROLES.map((r) => [r.reg, r.label]),
);

/// Cursor names that show up in packs ported from X11/freedesktop themes but
/// have no Windows counterpart. Windows only defines cursors the window manager
/// itself arbitrates (resize, drag, wait, reject); these are drawn by each
/// application from its own bitmaps, so they cannot be set system-wide.
/// Ordered longest-match-first so "grabbing" wins over "grab".
const APP_ONLY_CURSORS = [
  "zoom-in", "zoom-out", "zoom",
  "col-resize", "row-resize", "all-scroll",
  "context-menu", "vertical-text", "no-drop",
  "closedhand", "openhand", "grabbing", "grab", "alias",
];

/// Why a pack file ended up unassigned, for the Apply view's tooltip.
export function unmatchedReason(file: string): string {
  const stem = file.replace(/\.(cur|ani)$/i, "").toLowerCase();
  const appOnly = APP_ONLY_CURSORS.find((n) => stem.includes(n));
  return appOnly
    ? `Windows has no "${appOnly}" cursor role. That cursor is drawn by each application from its own bitmaps, so no system setting can override it.`
    : "Filename didn't match a known role. Pick a role from the dropdown to assign it manually.";
}
