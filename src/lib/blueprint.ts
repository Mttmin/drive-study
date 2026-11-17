export interface WeightedSection {
  id: string;
  weight: number;
}

/**
 * Largest-remainder apportionment. Ties in the remainder go to the
 * section id in ascending order, which matches the published FINRA
 * question counts for the SIE (75) and Series 7 (125) weights.
 */
export function allocateQuestions(sections: WeightedSection[], total: number): Record<string, number> {
  const result: Record<string, number> = {};
  for (const section of sections) result[section.id] = 0;
  if (total <= 0 || sections.length === 0) return result;
  const weightSum = sections.reduce((sum, section) => sum + section.weight, 0);
  if (weightSum <= 0) return result;

  const rows = sections.map((section) => {
    const exact = (section.weight / weightSum) * total;
    const n = Math.floor(exact);
    return { id: section.id, n, frac: exact - n };
  });
  let used = rows.reduce((sum, row) => sum + row.n, 0);
  const order = [...rows].sort((a, b) => b.frac - a.frac || a.id.localeCompare(b.id));
  let index = 0;
  while (used < total) {
    order[index % order.length].n += 1;
    used += 1;
    index += 1;
  }
  for (const row of rows) result[row.id] = row.n;
  return result;
}

export function drawWeighted<T extends { id: string; section: string }>(
  items: T[],
  sections: WeightedSection[],
  count: number,
  rng: () => number = Math.random,
): T[] {
  const target = Math.max(0, Math.min(count, items.length));
  if (target === 0) return [];
  const quotas = allocateQuestions(sections, target);
  const pools = new Map<string, T[]>();
  for (const item of items) {
    const list = pools.get(item.section) ?? [];
    list.push(item);
    pools.set(item.section, list);
  }
  for (const list of pools.values()) shuffle(list, rng);

  const picked: T[] = [];
  for (const section of sections) {
    const pool = pools.get(section.id) ?? [];
    const take = Math.min(quotas[section.id] ?? 0, pool.length);
    picked.push(...pool.slice(0, take));
  }
  if (picked.length < target) {
    const pickedIds = new Set(picked.map((item) => item.id));
    const rest = items.filter((item) => !pickedIds.has(item.id));
    shuffle(rest, rng);
    while (picked.length < target && rest.length > 0) {
      const next = rest.pop();
      if (next) picked.push(next);
    }
  }
  shuffle(picked, rng);
  return picked;
}

export function shuffle<T>(list: T[], rng: () => number = Math.random): T[] {
  for (let i = list.length - 1; i > 0; i--) {
    const j = Math.floor(rng() * (i + 1));
    const swap = list[i];
    list[i] = list[j];
    list[j] = swap;
  }
  return list;
}
