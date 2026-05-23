import { useState } from "react";
import {
  Layers,
  Pencil,
  MousePointer2,
  Settings,
  PawPrint,
  FlaskConical,
} from "lucide-react";
import { cn } from "@/lib/utils";
import Browse from "@/views/Browse";
import PackDetail from "@/views/PackDetail";
import Apply from "@/views/Apply";
import Debug from "@/views/Debug";

interface PackMeta {
  id: string;
  name: string;
  author: string;
  description: string;
  platform: "windows" | "linux" | "unknown";
  cursor_count: number;
  imported_at: number;
}

// ── Types ─────────────────────────────────────────────────────────────────────

type NavId = "browse" | "editor" | "apply" | "debug" | "settings";

interface NavEntry {
  id: NavId;
  label: string;
  Icon: React.ComponentType<{ className?: string; strokeWidth?: number }>;
}

// ── Nav config ────────────────────────────────────────────────────────────────

const mainNav: NavEntry[] = [
  { id: "browse",  label: "Browse",  Icon: Layers         },
  { id: "editor",  label: "Editor",  Icon: Pencil         },
  { id: "apply",   label: "Apply",   Icon: MousePointer2  },
  { id: "debug",   label: "Debug",   Icon: FlaskConical   },
];

const placeholders: Partial<Record<NavId, { title: string; sub: string }>> = {
  editor:   { title: "Cursor Editor", sub: "Adjust hotspot positions and frame timelines for animated .ani cursors." },
  settings: { title: "Settings",     sub: "Configure import paths, export formats, and application preferences."   },
};

// ── Sub-components ─────────────────────────────────────────────────────────────

function NavButton({
  entry,
  active,
  onClick,
}: {
  entry: NavEntry;
  active: boolean;
  onClick: () => void;
}) {
  return (
    <button
      onClick={onClick}
      className={cn(
        "relative flex w-full items-center gap-3 rounded-sm px-3 py-[7px] text-sm font-medium transition-colors duration-100",
        active
          ? "bg-zinc-800/70 text-zinc-100"
          : "text-zinc-500 hover:bg-zinc-800/40 hover:text-zinc-300",
      )}
    >
      {active && (
        <span className="absolute left-0 top-1/2 h-4 w-0.5 -translate-y-1/2 rounded-full bg-amber-400" />
      )}
      <entry.Icon
        className={cn("h-4 w-4 shrink-0", active ? "text-amber-400" : "")}
        strokeWidth={1.75}
      />
      {entry.label}
    </button>
  );
}

function PlaceholderPanel({ id }: { id: NavId }) {
  const info = placeholders[id];
  if (!info) return null;
  const entry = [...mainNav, { id: "settings" as NavId, label: "Settings", Icon: Settings }].find(
    (e) => e.id === id,
  );
  const Icon = entry?.Icon ?? Settings;

  return (
    <div className="flex flex-1 flex-col items-center justify-center gap-5">
      <div className="flex h-14 w-14 items-center justify-center rounded-lg border border-zinc-800 bg-zinc-900">
        <Icon className="h-6 w-6 text-zinc-600" strokeWidth={1.5} />
      </div>
      <div className="text-center">
        <p className="text-[15px] font-semibold text-zinc-300">{info.title}</p>
        <p className="mt-1.5 max-w-xs text-sm leading-relaxed text-zinc-600">{info.sub}</p>
      </div>
      <span className="font-mono text-[9px] uppercase tracking-[0.2em] text-zinc-700">
        Coming soon
      </span>
    </div>
  );
}

// ── App ───────────────────────────────────────────────────────────────────────

interface ActivePack {
  id: string;
  name: string;
  appliedAt: number; // unix ms
}

const ACTIVE_PACK_KEY = "pawpack:activePack";

export default function App() {
  const [active, setActive] = useState<NavId>("browse");
  const [selectedPack, setSelectedPack] = useState<PackMeta | null>(null);
  const [activePack, setActivePack] = useState<ActivePack | null>(() => {
    try {
      const raw = localStorage.getItem(ACTIVE_PACK_KEY);
      return raw ? (JSON.parse(raw) as ActivePack) : null;
    } catch {
      return null;
    }
  });

  const handleApplied = (pack: PackMeta) => {
    const next: ActivePack = { id: pack.id, name: pack.name, appliedAt: Date.now() };
    localStorage.setItem(ACTIVE_PACK_KEY, JSON.stringify(next));
    setActivePack(next);
  };

  const handleReverted = () => {
    localStorage.removeItem(ACTIVE_PACK_KEY);
    setActivePack(null);
  };

  return (
    <div className="flex h-screen w-screen overflow-hidden bg-zinc-950 font-sans text-zinc-100 select-none">
      {/* ── Sidebar ─────────────────────────────────────── */}
      <aside className="flex w-[220px] shrink-0 flex-col border-r border-zinc-800/60 bg-zinc-900">

        {/* Logo */}
        <div className="flex items-center gap-2.5 border-b border-zinc-800/60 px-4 py-3.5">
          <div className="flex h-7 w-7 items-center justify-center rounded-sm bg-amber-400/10">
            <PawPrint className="h-[15px] w-[15px] text-amber-400" strokeWidth={2} />
          </div>
          <span className="text-[13px] font-semibold tracking-wide text-zinc-100">
            PawPack
          </span>
          <span className="ml-auto font-mono text-[10px] tracking-widest text-zinc-600">
            0.1
          </span>
        </div>

        {/* Section label */}
        <p className="px-4 pb-1 pt-4 font-mono text-[9px] uppercase tracking-[0.14em] text-zinc-600">
          Library
        </p>

        {/* Main nav */}
        <nav className="flex flex-col gap-0.5 px-2">
          {mainNav.map((entry) => (
            <NavButton
              key={entry.id}
              entry={entry}
              active={active === entry.id}
              onClick={() => { setActive(entry.id); setSelectedPack(null); }}
            />
          ))}
        </nav>

        <div className="flex-1" />

        {/* Settings */}
        <div className="border-t border-zinc-800/60 px-2 pb-2 pt-2">
          <NavButton
            entry={{ id: "settings", label: "Settings", Icon: Settings }}
            active={active === "settings"}
            onClick={() => setActive("settings")}
          />
        </div>
      </aside>

      {/* ── Main area ───────────────────────────────────── */}
      <div className="flex flex-1 flex-col overflow-hidden">

        {/* Top bar */}
        <header className="flex h-10 shrink-0 items-center border-b border-zinc-800/60 px-5">
          <span className="font-mono text-[10px] uppercase tracking-[0.14em] text-zinc-600">
            {active}
            {selectedPack && (
              <>
                <span className="mx-1.5 text-zinc-700">/</span>
                {selectedPack.name}
              </>
            )}
          </span>
        </header>

        {/* Active panel */}
        {active === "browse" ? (
          selectedPack ? (
            <PackDetail pack={selectedPack} onBack={() => setSelectedPack(null)} />
          ) : (
            <Browse onSelect={setSelectedPack} />
          )
        ) : active === "apply" ? (
          <Apply
            activePack={activePack}
            onApplied={handleApplied}
            onReverted={handleReverted}
          />
        ) : active === "debug" ? (
          <Debug activePack={activePack} />
        ) : (
          <PlaceholderPanel id={active} />
        )}
      </div>
    </div>
  );
}
