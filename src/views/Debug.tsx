import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { CURSOR_ROLES } from "@/lib/roles";

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

/// CSS cursor keyword per role, used as the fallback when no pack is applied.
const ROLE_CSS: Record<string, string> = {
  Arrow: "default", Hand: "pointer", IBeam: "text", Wait: "wait",
  AppStarting: "progress", Crosshair: "crosshair", SizeAll: "move",
  No: "not-allowed", Help: "help", SizeNS: "ns-resize", SizeWE: "ew-resize",
  SizeNWSE: "nwse-resize", SizeNESW: "nesw-resize", UpArrow: "cell",
  NWPen: "alias", Pin: "copy", Person: "alias",
};

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
  applied?: { target: { kind: "pack"; id: string } | { kind: "mix" } } | null;
}

export default function Debug({ applied }: Props) {
  // role → CSS cursor string built from the pack's actual image
  const [customCursors, setCustomCursors] = useState<Record<string, string>>({});

  useEffect(() => {
    if (!applied) { setCustomCursors({}); return; }

    let cancelled = false;

    async function load() {
      try {
        // role → [packId, file], from whichever thing is applied.
        let roleToSource: Record<string, [string, string]> = {};
        if (applied?.target.kind === "mix") {
          const mix = await invoke<{ roles: { role: string; pack: string; file: string }[] }>("get_mix");
          roleToSource = Object.fromEntries(mix.roles.map((e) => [e.role, [e.pack, e.file]]));
        } else if (applied?.target.kind === "pack") {
          const packId = applied.target.id;
          const result = await invoke<PackAssignmentResult>("get_pack_assignments", { packId });
          roleToSource = Object.fromEntries(result.assigned.map((a) => [a.role, [packId, a.file]]));
        }

        const cursors: Record<string, string> = {};

        for (const role of NO_CSS_ROLES) {
          const source = roleToSource[role];
          if (!source) continue;
          const [sourcePack, file] = source;
          try {
            let frame: CurFrame | null = null;
            if (file.toLowerCase().endsWith(".ani")) {
              const ani = await invoke<AniInfo>("parse_ani", {
                packId: sourcePack,
                cursorName: file,
              });
              frame = bestFrame(ani.frames[0]?.frames ?? []);
            } else {
              const cur = await invoke<CurInfo>("parse_cur", {
                packId: sourcePack,
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
  }, [applied]);

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
          {CURSOR_ROLES.map((role) => (
            <div
              key={role.reg}
              style={{ cursor: customCursors[role.reg] ?? ROLE_CSS[role.reg] }}
              className="group flex flex-col items-center justify-center gap-3 rounded-sm border border-zinc-800 bg-zinc-900 px-4 py-6 transition-colors hover:border-zinc-600 hover:bg-zinc-800/60 select-none"
            >
              <role.Icon
                className={`h-7 w-7 transition-transform group-hover:scale-110 ${role.color}`}
                strokeWidth={1.5}
              />
              <div className="text-center">
                <p className="text-xs font-medium text-zinc-300">{role.label}</p>
                <p className="mt-0.5 font-mono text-[10px] text-zinc-600">{role.reg}</p>
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
