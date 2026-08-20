/** A cursor pack as `list_packs` returns it. Mirrors `PackMeta` in lib.rs. */
export interface PackMeta {
  id: string;
  name: string;
  author: string;
  description: string;
  platform: "windows" | "linux" | "unknown";
  cursor_count: number;
  /** Unix seconds. */
  imported_at: number;
  /**
   * Set when one download held several complete cursor sets. Packs sharing a
   * group render as a single entry with a variant switcher.
   */
  group?: string | null;
  /** This pack's name within its group ("Moga Black"). */
  variant?: string | null;
}

/**
 * One entry in the library: either a lone pack, or a download's variants.
 *
 * A variant is a full set of roles, so it is still the thing that gets applied
 * — grouping only changes how the list is drawn. `packs` always holds at least
 * one entry, and `packs[0]` is the default selection.
 */
export interface PackGroup {
  key: string;
  title: string;
  packs: PackMeta[];
}

/**
 * Fold `list_packs` output into library entries, preserving the incoming order
 * (newest first) — a group takes the position of its earliest member so the
 * list does not reshuffle when a variant is selected.
 */
export function groupPacks(packs: PackMeta[]): PackGroup[] {
  const groups: PackGroup[] = [];
  const byKey = new Map<string, PackGroup>();

  for (const pack of packs) {
    if (!pack.group) {
      groups.push({ key: pack.id, title: pack.name, packs: [pack] });
      continue;
    }
    const existing = byKey.get(pack.group);
    if (existing) {
      existing.packs.push(pack);
    } else {
      const group: PackGroup = { key: pack.group, title: pack.group, packs: [pack] };
      byKey.set(pack.group, group);
      groups.push(group);
    }
  }

  // Applicable variants first — packs[0] is the default selection, and
  // defaulting a download to its Linux theme on Windows offers something that
  // cannot be applied. Name breaks the tie, since read_dir order and identical
  // import timestamps are both unreliable.
  for (const group of groups) {
    if (group.packs.length > 1) {
      group.packs.sort(
        (a, b) =>
          Number(b.platform === "windows") - Number(a.platform === "windows") ||
          (a.variant ?? a.name).localeCompare(b.variant ?? b.name, undefined, {
            sensitivity: "base",
          }),
      );
    }
  }

  return groups;
}

/** What to show on a variant chip. */
export function variantLabel(pack: PackMeta): string {
  return pack.variant ?? pack.name;
}
