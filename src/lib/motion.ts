/// PawPack's own motion preference.
///
/// Deliberately app-scoped: changing it affects nothing outside PawPack. The
/// Windows setting is used only to seed the default, so someone who has already
/// asked the system for less motion gets it here too without having to find
/// this control — but once it is touched, this is the authority.

const KEY = "pawpack:motion";

/// Three steps, least motion first — the order the slider runs in.
export const MOTION_LEVELS = ["off", "slow", "full"] as const;

export type MotionLevel = (typeof MOTION_LEVELS)[number];

export const MOTION_LABELS: Record<MotionLevel, string> = {
  off: "Off",
  slow: "Slow",
  full: "Full",
};

export const MOTION_DESCRIPTIONS: Record<MotionLevel, string> = {
  off: "Nothing animates. Thumbnail strips get a scrollbar instead.",
  slow: "Thumbnail strips drift at a third of the usual speed.",
  full: "Thumbnail strips loop at full speed.",
};

function isLevel(v: string | null): v is MotionLevel {
  return v !== null && (MOTION_LEVELS as readonly string[]).includes(v);
}

/// The effective level: an explicit choice if one was made, otherwise seeded
/// from Windows — "off" when the system asks for reduced motion.
export function motionLevel(): MotionLevel {
  const raw = localStorage.getItem(KEY);
  if (isLevel(raw)) return raw;
  return window.matchMedia("(prefers-reduced-motion: reduce)").matches ? "off" : "full";
}

export function setMotionLevel(level: MotionLevel): void {
  localStorage.setItem(KEY, level);
  applyMotion();
}

/// Stamp the level on <html> so CSS can key off it. Called once at startup and
/// again on every change.
export function applyMotion(): void {
  document.documentElement.dataset.motion = motionLevel();
}
