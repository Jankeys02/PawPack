import { ArrowUpRight, MoveHorizontal, MoveVertical, Move, Type, MousePointer2, Hand, Ban, HelpCircle, Crosshair, Clock, Loader, MoveUpRight, MoveDownRight } from "lucide-react";

// ── Cursor zones ──────────────────────────────────────────────────────────────

interface Zone {
  css: string;           // CSS cursor value
  label: string;         // display name
  reg: string;           // Windows registry key name
  Icon: React.ComponentType<{ className?: string; strokeWidth?: number }>;
  color: string;         // accent colour class
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
  { css: "zoom-in",      label: "Zoom in",     reg: "—",           Icon: ArrowUpRight,   color: "text-pink-400"   },
];

// ── Main view ─────────────────────────────────────────────────────────────────

export default function Debug() {
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
              key={z.css}
              style={{ cursor: z.css }}
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

            {/* Edge hit zones */}
            <div className="absolute inset-x-0 top-0 h-3"    style={{ cursor: "n-resize"  }} />
            <div className="absolute inset-x-0 bottom-0 h-3" style={{ cursor: "s-resize"  }} />
            <div className="absolute inset-y-0 left-0 w-3"   style={{ cursor: "w-resize"  }} />
            <div className="absolute inset-y-0 right-0 w-3"  style={{ cursor: "e-resize"  }} />
            <div className="absolute left-0 top-0 h-4 w-4"   style={{ cursor: "nw-resize" }} />
            <div className="absolute right-0 top-0 h-4 w-4"  style={{ cursor: "ne-resize" }} />
            <div className="absolute bottom-0 left-0 h-4 w-4"style={{ cursor: "sw-resize" }} />
            <div className="absolute bottom-0 right-0 h-4 w-4"style={{ cursor: "se-resize"}} />
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
