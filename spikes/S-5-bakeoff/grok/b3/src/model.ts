export type SubAction = {
  id: string;
  name: string;
  category: readonly string[];
  description: string;
};

export type Section = "favourites" | "recent" | "catalog";

export type BranchNode = {
  kind: "branch";
  id: string;
  label: string;
  section: Section;
  children: TreeNode[];
};

export type LeafNode = {
  kind: "leaf";
  id: string;
  section: Section;
  item: SubAction;
};

export type TreeNode = BranchNode | LeafNode;

export type FlatRow = {
  id: string;
  depth: number;
  parentId: string | null;
  node: TreeNode;
  expanded: boolean;
  setsize: number;
  posinset: number;
};

export type TypeAheadState = {
  buffer: string;
  at: number;
};

export type TreeKey = "ArrowDown" | "ArrowUp" | "ArrowLeft" | "ArrowRight" | "Home" | "End" | "Enter" | " ";

export type KeyResult = {
  focusId: string | null;
  collapsed: string[];
  activateItemId?: string;
};

const TOP_ORDER = ["Core", "Twitch", "YouTube", "Kick", "OBS", "Streamlabs"];

export function fuzzyScore(query: string, text: string): number | null {
  const q = query.trim().toLowerCase();
  if (!q) return 0;
  const t = text.toLowerCase();
  let from = 0;
  let score = 0;
  let run = 0;
  for (const ch of q) {
    if (ch === " ") {
      run = 0;
      continue;
    }
    const at = t.indexOf(ch, from);
    if (at < 0) return null;
    const boundary = at === 0 || !/[a-z0-9]/.test(t[at - 1] ?? "");
    const contiguous = at === from;
    run = contiguous ? run + 1 : 0;
    score += 1 + run * 2 + (boundary ? 3 : 0);
    if (at === 0) score += 2;
    from = at + 1;
  }
  return score - t.length * 0.001;
}

export function matchItem(query: string, item: SubAction): number | null {
  const q = query.trim();
  if (!q) return 0;
  const name = fuzzyScore(q, item.name);
  const path = fuzzyScore(q, item.category.join(" "));
  const description = fuzzyScore(q, item.description);
  if (name === null && path === null && description === null) return null;
  return Math.max(name === null ? Number.NEGATIVE_INFINITY : name + 10, path ?? Number.NEGATIVE_INFINITY, description === null ? Number.NEGATIVE_INFINITY : description - 2);
}

function byName(a: SubAction, b: SubAction): number {
  return a.name.localeCompare(b.name) || a.id.localeCompare(b.id);
}

function topRank(name: string): number {
  const index = TOP_ORDER.indexOf(name);
  return index === -1 ? TOP_ORDER.length : index;
}

export function rowLabel(row: FlatRow): string {
  return row.node.kind === "branch" ? row.node.label : row.node.item.name;
}

export function buildTree(
  items: readonly SubAction[],
  query: string,
  favouriteIds: readonly string[],
  recentIds: readonly string[],
): TreeNode[] {
  const byId = new Map(items.map((item) => [item.id, item]));
  const searching = query.trim().length > 0;

  const sectionLeaves = (ids: readonly string[], section: Section): LeafNode[] => {
    const leaves: LeafNode[] = [];
    for (const id of ids) {
      const item = byId.get(id);
      if (!item) continue;
      if (matchItem(query, item) === null) continue;
      leaves.push({
        kind: "leaf",
        id: `leaf-${section}-${item.id}`,
        section,
        item,
      });
    }
    return leaves;
  };

  const nodes: TreeNode[] = [];
  const favourites = sectionLeaves(favouriteIds, "favourites");
  if (favourites.length > 0) {
    nodes.push({
      kind: "branch",
      id: "branch-favourites",
      label: `Favourites (${favourites.length})`,
      section: "favourites",
      children: favourites,
    });
  }
  const recent = sectionLeaves(recentIds, "recent");
  if (recent.length > 0) {
    nodes.push({
      kind: "branch",
      id: "branch-recent",
      label: `Recently used (${recent.length})`,
      section: "recent",
      children: recent,
    });
  }

  const groups = new Map<string, Map<string, { item: SubAction; score: number }[]>>();
  for (const item of items) {
    const score = matchItem(query, item);
    if (score === null) continue;
    const top = item.category[0] ?? "Other";
    const sub = item.category[1] ?? "General";
    let subs = groups.get(top);
    if (!subs) {
      subs = new Map();
      groups.set(top, subs);
    }
    let bucket = subs.get(sub);
    if (!bucket) {
      bucket = [];
      subs.set(sub, bucket);
    }
    bucket.push({ item, score });
  }

  const tops = [...groups.keys()].sort((a, b) => topRank(a) - topRank(b) || a.localeCompare(b));
  for (const top of tops) {
    const subs = groups.get(top);
    if (!subs) continue;
    const children: TreeNode[] = [];
    for (const sub of [...subs.keys()].sort((a, b) => a.localeCompare(b))) {
      const bucket = subs.get(sub) ?? [];
      const ordered = [...bucket].sort((a, b) => (searching ? b.score - a.score : 0) || byName(a.item, b.item));
      children.push({
        kind: "branch",
        id: `branch-catalog-${slug(top)}-${slug(sub)}`,
        label: sub,
        section: "catalog",
        children: ordered.map(({ item }) => ({
          kind: "leaf" as const,
          id: `leaf-catalog-${item.id}`,
          section: "catalog" as const,
          item,
        })),
      });
    }
    nodes.push({
      kind: "branch",
      id: `branch-catalog-${slug(top)}`,
      label: top,
      section: "catalog",
      children,
    });
  }

  return nodes;
}

function slug(value: string): string {
  return value
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, "-")
    .replace(/^-|-$/g, "");
}

export function defaultCollapsed(nodes: readonly TreeNode[]): string[] {
  const ids: string[] = [];
  const walk = (list: readonly TreeNode[]) => {
    for (const node of list) {
      if (node.kind !== "branch") continue;
      ids.push(node.id);
      walk(node.children);
    }
  };
  walk(nodes);
  const open = new Set(["branch-favourites", "branch-recent", "branch-catalog-core", "branch-catalog-core-logic"]);
  return ids.filter((id) => !open.has(id));
}

export function flatten(nodes: readonly TreeNode[], collapsed: ReadonlySet<string>, searching: boolean, depth = 0, parentId: string | null = null): FlatRow[] {
  const rows: FlatRow[] = [];
  nodes.forEach((node, index) => {
    const expanded = node.kind === "branch" && (searching || !collapsed.has(node.id));
    rows.push({
      id: node.id,
      depth,
      parentId,
      node,
      expanded: node.kind === "branch" ? expanded : false,
      setsize: nodes.length,
      posinset: index + 1,
    });
    if (node.kind === "branch" && expanded) {
      rows.push(...flatten(node.children, collapsed, searching, depth + 1, node.id));
    }
  });
  return rows;
}

export function bestLeafId(rows: readonly FlatRow[], query: string): string | null {
  let best: { id: string; score: number; catalog: boolean } | null = null;
  for (const row of rows) {
    if (row.node.kind !== "leaf") continue;
    const score = matchItem(query, row.node.item);
    if (score === null) continue;
    const catalog = row.node.section === "catalog";
    if (!best || score > best.score || (score === best.score && catalog && !best.catalog)) {
      best = { id: row.id, score, catalog };
    }
  }
  return best?.id ?? null;
}

export function onTreeKey(rows: readonly FlatRow[], collapsed: readonly string[], focusId: string | null, key: TreeKey, searching: boolean): KeyResult {
  const nextCollapsed = new Set(collapsed);
  const index = focusId ? rows.findIndex((row) => row.id === focusId) : -1;
  const current = index >= 0 ? rows[index] : null;

  const finish = (nextFocus: string | null, activateItemId?: string): KeyResult => ({
    focusId: nextFocus,
    collapsed: [...nextCollapsed],
    activateItemId,
  });

  if (rows.length === 0) return finish(null);

  if (key === "ArrowDown") {
    const next = index < 0 ? 0 : Math.min(rows.length - 1, index + 1);
    return finish(rows[next]?.id ?? null);
  }
  if (key === "ArrowUp") {
    if (index <= 0) return finish(rows[0]?.id ?? null);
    return finish(rows[index - 1]?.id ?? null);
  }
  if (key === "Home") return finish(rows[0]?.id ?? null);
  if (key === "End") return finish(rows[rows.length - 1]?.id ?? null);

  if (!current) return finish(rows[0]?.id ?? null);

  if (key === "ArrowRight") {
    if (current.node.kind === "branch" && !searching && nextCollapsed.has(current.id)) {
      nextCollapsed.delete(current.id);
      return finish(current.id);
    }
    const child = rows[index + 1];
    if (current.node.kind === "branch" && child && child.depth > current.depth) return finish(child.id);
    return finish(current.id);
  }

  if (key === "ArrowLeft") {
    if (current.node.kind === "branch" && !searching && !nextCollapsed.has(current.id)) {
      nextCollapsed.add(current.id);
      return finish(current.id);
    }
    if (current.parentId) return finish(current.parentId);
    return finish(current.id);
  }

  if (key === "Enter" || key === " ") {
    if (current.node.kind === "branch") {
      if (!searching) {
        if (nextCollapsed.has(current.id)) nextCollapsed.delete(current.id);
        else nextCollapsed.add(current.id);
      }
      return finish(current.id);
    }
    return finish(current.id, current.node.item.id);
  }

  return finish(focusId);
}

export function typeAhead(rows: readonly FlatRow[], focusId: string | null, buffer: string): string | null {
  const q = buffer.trim().toLowerCase();
  if (!q) return focusId;
  const start = focusId ? rows.findIndex((row) => row.id === focusId) : -1;
  let order: FlatRow[];
  if (start < 0) {
    order = [...rows];
  } else if (q.length > 1) {
    order = [...rows.slice(start), ...rows.slice(0, start)];
  } else {
    order = [...rows.slice(start + 1), ...rows.slice(0, start + 1)];
  }
  const match = order.find((row) => rowLabel(row).toLowerCase().startsWith(q));
  return match?.id ?? focusId;
}

export function pushTypeAhead(state: TypeAheadState | null, char: string, now: number, windowMs = 700): TypeAheadState {
  if (!state || now - state.at > windowMs) return { buffer: char.toLowerCase(), at: now };
  return { buffer: state.buffer + char.toLowerCase(), at: now };
}

export function neighbourId(previous: readonly FlatRow[], next: readonly FlatRow[], focusId: string): string | null {
  if (next.some((row) => row.id === focusId)) return focusId;
  const oldIndex = previous.findIndex((row) => row.id === focusId);
  if (next.length === 0) return null;
  const index = oldIndex < 0 ? 0 : Math.min(oldIndex, next.length - 1);
  return next[index]?.id ?? null;
}
