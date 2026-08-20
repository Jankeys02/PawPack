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

/// CSS cursor keywords that have no Windows counterpart.
///
/// The CSS keyword set is the de-facto naming standard theme authors follow,
/// so it is the right vocabulary to recognise in ported packs. Only the
/// keywords Windows cannot express are listed here — `pointer`, `text`,
/// `progress`, `not-allowed`, `move` and the eight directional `*-resize`
/// names all map onto real roles in CURSOR_ROLES and must stay out of this
/// list, or a perfectly assignable file would be labelled impossible.
///
/// Windows only defines cursors the window manager itself arbitrates. These
/// are drawn by each application from its own bitmaps — Chromium, for one,
/// ships zoom-in/zoom-out as resources in its own module and falls back to
/// them precisely because LoadCursor(NULL, ...) has nothing to return.
///
/// Longest name first, so "grabbing" is reported before "grab" and "zoom-in"
/// before "zoom".
const APP_ONLY_CURSORS = [
  "vertical-text", "context-menu", "col-resize", "row-resize", "all-scroll",
  "zoom-in", "zoom-out", "grabbing", "no-drop", "alias", "grab", "cell",
  "copy", "zoom",
];

/// Match a cursor name as a whole word, tolerating the separators packs use
/// interchangeably ("zoom-in", "zoom_in", "zoom in", "zoomin").
function matchesCursorName(stem: string, name: string): boolean {
  const body = name.replace(/-/g, "[-_ ]?");
  return new RegExp(`(^|[^a-z0-9])${body}($|[^a-z0-9])`).test(stem);
}

/// Why a pack file ended up unassigned, for the Apply view's tooltip.
export function unmatchedReason(file: string): string {
  const stem = file
    .replace(/\.(cur|ani)$/i, "")
    .toLowerCase()
    // Explorer's duplicate suffix, so "arrow - Copy" and "arrow - Copy (2)"
    // are not read as the CSS "copy" cursor. The leading separator is
    // required, so a pack's own "copy.cur" survives intact.
    .replace(/[\s_-]+\(?copy\)?(\s*\(?\d+\)?)?$/, "");

  const appOnly = APP_ONLY_CURSORS.find((n) => matchesCursorName(stem, n));
  return appOnly
    ? `Windows has no "${appOnly}" cursor role. That cursor is drawn by each application from its own bitmaps, so no system setting can override it.`
    : "Filename didn't match a known role. Pick a role from the dropdown to assign it manually.";
}
