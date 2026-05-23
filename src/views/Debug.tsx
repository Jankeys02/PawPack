import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { ArrowUp, MoveHorizontal, MoveVertical, Move, Type, MousePointer2, Hand, Ban, HelpCircle, Crosshair, Clock, Loader, MoveUpRight, MoveDownRight, PenLine, MapPin, UserRound } from "lucide-react";

// ── Types ─────────────────────────────────────────────────────────────────────

interface CurFrame {
  width: number;
  height: number;
  hotspot_x: number;
  hotspot_y: number;
  rgba: number[];
}

interface CurInfo  { frames: CurFrame[]; }
interface AniInfo  { frames: CurInfo[];  }
interface Assignment { role: string; file: string; }
interface PackAssignmentResult { assigned: Assignment[]; }

// ── Cursor zones ──────────────────────────────────────────────────────────────

interface Zone {
  css: string;           // CSS cursor value (fallback when no pack loaded)
  label: string;
  reg: string;           // Windows registry key name
  Icon: React.ComponentType<{ className?: string; strokeWidth?: number }>;
  color: string;
}

const ZONES: Zone[] = [
  { css: "default",      label: "Arrow",       reg: "Arrow",       Icon: MousePointer2,  color: "text-zinc-400"   },
  { css: "pointer",      label: "Hand / Link", reg: "Hand",        Icon: Hand,           color: "text-amber-400"  },
  { css: "text",         label: "I-Beam",      reg: "IBeam",       Icon: Type,           color: "text-sky-400"    },
  { css: "wait",         label: "Wait",        reg: "Wait",        Icon: Clock,          color: "text-orange-400" },
  { css: "progress",     label: "Working",     reg: "AppStarting", Icon: Loader,         color: "text-yellow-400" },
  { css: "crosshair",    label: "Crosshair",   reg: "Crosshair",   Icon: Crosshair,      color: "text-emerald-400"},
  { css: "move",         label: "Move",        reg: "SizeAll",     Icon: Move,           color: "text-violet-400" },
  { css: "not-allowed",  label: "Unavailable", reg: "No",          Icon: Ban,            color: "text-red-400"    },
  { css: "help",         label: "Help",        reg: "Help",        Icon: HelpCircle,     color: "text-teal-400"   },
  { css: "ns-resize",    label: "Resize N↕S",  reg: "SizeNS",      Icon: MoveVertical,   color: "text-cyan-400"   },
  { css: "ew-resize",    label: "Resize E↔W",  reg: "SizeWE",      Icon: MoveHorizontal, color: "text-cyan-400"   },
  { css: "nwse-resize",  label: "Resize ↘",    reg: "SizeNWSE",    Icon: MoveDownRight,  color: "text-indigo-400" },
  { css: "nesw-resize",  label: "Resize ↗",    reg: "SizeNESW",    Icon: MoveUpRight,    color: "text-indigo-400" },
  { css: "cell",         label: "Alt Select",  reg: "UpArrow",     Icon: ArrowUp,        color: "text-lime-400"   },
  { css: "alias",        label: "Handwriting", reg: "NWPen",       Icon: PenLine,        color: "text-fuchsia-400"},
  { css: "copy",         label: "Location",    reg: "Pin",         Icon: MapPin,         color: "text-rose-400"   },
  { css: "alias",        label: "Person",      reg: "Person",      Icon: UserRound,      color: "text-sky-300"    },
];

// Roles with no CSS equivalent — must be driven by the pack's actual cursor image
const NO_CSS_ROLES = new Set(["UpArrow", "NWPen", "Pin", "Person"]);

// ── Helpers ───────────────────────────────────────────────────────────────────

function bestFrame(frames: CurFrame[]): CurFrame | null {
  if (!frames.length) return null;
  return (
    frames.find((f) => f.width === 32 && f.height === 32) ??
    frames.reduce((a, b) => a.width * a.height >= b.width * b.height ? a : b)
  );
}

function frameToCssUrl(frame: CurFrame): string {
  const canvas = document.createElement("canvas");
  canvas.width  = frame.width;
  canvas.height = frame.height;
  const ctx = canvas.getContext("2d")!;
  ctx.putImageData(
    new ImageData(new Uint8ClampedArray(frame.rgba), frame.width, frame.height),
    0, 0,
  );
  const dataUrl = canvas.toDataURL("image/png");
  return `url('${dataUrl}') ${frame.hotspot_x} ${frame.hotspot_y}, auto`;
}

// ── Main view ─────────────────────────────────────────────────────────────────

interface Props {
  activePack?: { id: string } | null;
}

export default function Debug({ activePack }: Props) {
  // role → CSS cursor string built from the pack's actual image
  const [customCursors, setCustomCursors] = useState<Record<string, string>>({});

  useEffect(() => {
    if (!activePack) { setCustomCursors({}); return; }

    let cancelled = false;

    async function load() {
      try {
        const result = await invoke<PackAssignmentResult>("get_pack_assignments", {
          packId: activePack!.id,
        });
        const roleToFile = Object.fromEntries(result.assigned.map((a) => [a.role, a.file]));

        const cursors: Record<string, string> = {};

        for (const role of NO_CSS_ROLES) {
          const file = roleToFile[role];
          if (!file) continue;
          try {
            let frame: CurFrame | null = null;
            if (file.toLowerCase().endsWith(".ani")) {
              const ani = await invoke<AniInfo>("parse_ani", {
                packId: activePack!.id,
                cursorName: file,
              });
              frame = bestFrame(ani.frames[0]?.frames ?? []);
            } else {
              const cur = await invoke<CurInfo>("parse_cur", {
                packId: activePack!.id,
                cursorName: file,
              });
              frame = bestFrame(cur.frames);
            }
            if (frame) cursors[role] = frameToCssUrl(frame);
          } catch {
            // leave this role without a custom cursor
          }
        }

        if (!cancelled) setCustomCursors(cursors);
      } catch {
        // pack may not be applied on this OS
      }
    }

    load();
    return () => { cancelled = true; };
  }, [activePack?.id]);

  return (
    <div className="flex flex-1 flex-col overflow-hidden">
      {/* Header */}
      <div className="flex items-center border-b border-zinc-800/60 px-6 py-4">
        <div>
          <h1 className="text-sm font-semibold text-zinc-100">Cursor Test</h1>
          <p className="text-xs text-zinc-600">
            Hover each tile to preview the active cursor for that role
          </p>
        </div>
      </div>

      {/* Grid */}
      <div className="flex-1 overflow-y-auto p-6">
        <div className="grid grid-cols-2 gap-3 sm:grid-cols-3 md:grid-cols-4 xl:grid-cols-5">
          {ZONES.map((z) => (
            <div
              key={z.reg}
              style={{ cursor: customCursors[z.reg] ?? z.css }}
              className="group flex flex-col items-center justify-center gap-3 rounded-sm border border-zinc-800 bg-zinc-900 px-4 py-6 transition-colors hover:border-zinc-600 hover:bg-zinc-800/60 select-none"
            >
              <z.Icon
                className={`h-7 w-7 transition-transform group-hover:scale-110 ${z.color}`}
                strokeWidth={1.5}
              />
              <div className="text-center">
                <p className="text-xs font-medium text-zinc-300">{z.label}</p>
                <p className="mt-0.5 font-mono text-[10px] text-zinc-600">{z.reg}</p>
              </div>
            </div>
          ))}
        </div>

        {/* Resize playground */}
        <div className="mt-8">
          <p className="mb-3 font-mono text-[9px] uppercase tracking-[0.14em] text-zinc-600">
            Resize edges
          </p>
          <div
            className="relative flex h-36 items-center justify-center rounded-sm border border-zinc-700 bg-zinc-900/60"
            style={{ cursor: "move" }}
          >
            <p className="pointer-events-none text-xs text-zinc-600">
              Drag area — hover the edges for resize cursors
            </p>
            <div className="absolute inset-x-0 top-0 h-3"     style={{ cursor: "n-resize"  }} />
            <div className="absolute inset-x-0 bottom-0 h-3"  style={{ cursor: "s-resize"  }} />
            <div className="absolute inset-y-0 left-0 w-3"    style={{ cursor: "w-resize"  }} />
            <div className="absolute inset-y-0 right-0 w-3"   style={{ cursor: "e-resize"  }} />
            <div className="absolute left-0 top-0 h-4 w-4"    style={{ cursor: "nw-resize" }} />
            <div className="absolute right-0 top-0 h-4 w-4"   style={{ cursor: "ne-resize" }} />
            <div className="absolute bottom-0 left-0 h-4 w-4" style={{ cursor: "sw-resize" }} />
            <div className="absolute bottom-0 right-0 h-4 w-4"style={{ cursor: "se-resize" }} />
          </div>
        </div>

        {/* Text selection area */}
        <div className="mt-6">
          <p className="mb-3 font-mono text-[9px] uppercase tracking-[0.14em] text-zinc-600">
            Text selection
          </p>
          <div
            className="rounded-sm border border-zinc-700 bg-zinc-900/60 px-5 py-4 leading-relaxed text-sm text-zinc-400"
            style={{ cursor: "text" }}
          >
            Hover here to see the I-Beam cursor. This simulates a text input area — the cursor
            should switch to your pack's <span className="font-mono text-zinc-300">IBeam</span> shape.
          </div>
        </div>
      </div>
    </div>
  );
}
