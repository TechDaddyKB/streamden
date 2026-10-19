export interface SubActionDef {
  id: string;
  name: string;
  category: string;
  group: string;
  description: string;
  keywords: string[];
}

export interface FuzzyResult {
  score: number;
  /** Indices into the matched text, ascending. */
  indices: number[];
}

const isBoundary = (text: string, i: number): boolean => {
  if (i === 0) return true;
  const prev = text[i - 1] ?? "";
  const cur = text[i] ?? "";
  if (/[\s\-_/.·&()]/.test(prev)) return true;
  return prev === prev.toLowerCase() && cur !== cur.toLowerCase();
};

/**
 * Subsequence fuzzy match. Every query character must appear in order.
 * Rewards consecutive runs and word starts, penalises gaps. Returns null on no match.
 */
export function fuzzyMatch(query: string, text: string): FuzzyResult | null {
  const q = query.toLowerCase();
  const t = text.toLowerCase();
  if (q.length === 0) return { score: 0, indices: [] };
  if (q.length > t.length) return null;

  // Prefer a contiguous substring hit when there is one: it is what users expect to see highlighted.
  const sub = findBestSubstring(q, t, text);
  if (sub >= 0) {
    const indices = Array.from({ length: q.length }, (_, k) => sub + k);
    return { score: scoreIndices(indices, text) + (sub === 0 ? 30 : 0) + 10, indices };
  }

  // Word-start-first can jump past characters a later query letter needs; a plain greedy pass never misses.
  const indices = greedyIndices(q, t, text, true) ?? greedyIndices(q, t, text, false);
  return indices ? { score: scoreIndices(indices, text), indices } : null;
}

function greedyIndices(q: string, t: string, text: string, preferBoundary: boolean): number[] | null {
  const indices: number[] = [];
  let from = 0;
  for (const ch of q) {
    const plain = t.indexOf(ch, from);
    if (plain < 0) return null;
    let pos = plain;
    const last = indices[indices.length - 1];
    // Jump ahead to a word start unless we are continuing a run.
    if (preferBoundary && !(last !== undefined && plain === last + 1)) {
      for (let i = plain; i < t.length; i++) {
        if (t[i] === ch && isBoundary(text, i)) {
          pos = i;
          break;
        }
      }
    }
    indices.push(pos);
    from = pos + 1;
  }
  return indices;
}

function findBestSubstring(q: string, t: string, text: string): number {
  let best = -1;
  let i = t.indexOf(q);
  while (i >= 0) {
    if (best < 0) best = i;
    if (isBoundary(text, i)) return i;
    i = t.indexOf(q, i + 1);
  }
  return best;
}

function scoreIndices(indices: number[], text: string): number {
  let score = 0;
  let prev = -2;
  for (const i of indices) {
    score += 10;
    if (i === prev + 1) score += 15;
    else if (prev >= 0) score -= Math.min(i - prev - 1, 10);
    if (isBoundary(text, i)) score += 20;
    prev = i;
  }
  // Shorter targets win ties.
  return score - Math.floor(text.length / 4);
}

export interface ItemMatch {
  score: number;
  nameIndices: number[];
}

/**
 * Multi-term match: the query is split on whitespace and every term must match the
 * item's name, a keyword, its group or its category. Name hits score highest.
 */
export function matchItem(query: string, item: SubActionDef): ItemMatch | null {
  const terms = query.trim().split(/\s+/).filter(Boolean);
  if (terms.length === 0) return { score: 0, nameIndices: [] };
  let score = 0;
  const nameIndices = new Set<number>();
  for (const term of terms) {
    const name = fuzzyMatch(term, item.name);
    let best = name ? name.score : -Infinity;
    for (const kw of item.keywords) {
      const m = fuzzyMatch(term, kw);
      if (m) best = Math.max(best, m.score * 0.6);
    }
    for (const ctx of [item.group, item.category]) {
      const m = fuzzyMatch(term, ctx);
      if (m) best = Math.max(best, m.score * 0.5);
    }
    if (best === -Infinity) return null;
    if (name && name.score >= best) name.indices.forEach((i) => nameIndices.add(i));
    score += best;
  }
  return { score, nameIndices: [...nameIndices].sort((a, b) => a - b) };
}

export type TreeNode =
  | { kind: "branch"; id: string; label: string; children: TreeNode[] }
  | { kind: "leaf"; id: string; item: SubActionDef; highlight: number[] };

const leaf = (prefix: string, item: SubActionDef, highlight: number[] = []): TreeNode => ({
  kind: "leaf",
  id: `${prefix}${item.id}`,
  item,
  highlight,
});

export const FAVOURITES_ID = "section:favourites";
export const RECENT_ID = "section:recent";

export interface BuildTreeInput {
  items: SubActionDef[];
  query: string;
  favourites: string[];
  recent: string[];
}

/** Builds the visible tree. With a query, branches without hits are dropped and everything sorts by score. */
export function buildTree({ items, query, favourites, recent }: BuildTreeInput): TreeNode[] {
  const byId = new Map(items.map((i) => [i.id, i]));
  const searching = query.trim().length > 0;

  type Scored = { item: SubActionDef; match: ItemMatch };
  const scored: Scored[] = [];
  for (const item of items) {
    const match = matchItem(query, item);
    if (match) scored.push({ item, match });
  }

  const categories = new Map<string, Map<string, Scored[]>>();
  for (const s of scored) {
    let groups = categories.get(s.item.category);
    if (!groups) categories.set(s.item.category, (groups = new Map()));
    let list = groups.get(s.item.group);
    if (!list) groups.set(s.item.group, (list = []));
    list.push(s);
  }

  const best = (list: Scored[]) => Math.max(...list.map((s) => s.match.score));
  const tree: TreeNode[] = [];

  if (!searching) {
    const section = (id: string, label: string, ids: string[], prefix: string) => {
      const children = ids.flatMap((i) => {
        const item = byId.get(i);
        return item ? [leaf(prefix, item)] : [];
      });
      if (children.length > 0) tree.push({ kind: "branch", id, label, children });
    };
    section(FAVOURITES_ID, "Favourites", favourites, "fav:");
    section(RECENT_ID, "Recently used", recent, "recent:");
  }

  const catEntries = [...categories.entries()].map(([cat, groups]) => {
    const groupEntries = [...groups.entries()].map(([group, list]) => {
      if (searching) list.sort((a, b) => b.match.score - a.match.score || a.item.name.localeCompare(b.item.name));
      return { group, list, top: best(list) };
    });
    if (searching) groupEntries.sort((a, b) => b.top - a.top);
    return { cat, groupEntries, top: Math.max(...groupEntries.map((g) => g.top)) };
  });
  if (searching) catEntries.sort((a, b) => b.top - a.top);

  for (const { cat, groupEntries } of catEntries) {
    tree.push({
      kind: "branch",
      id: `cat:${cat}`,
      label: cat,
      children: groupEntries.map(({ group, list }) => ({
        kind: "branch" as const,
        id: `grp:${cat}/${group}`,
        label: group,
        children: list.map((s) => leaf("item:", s.item, s.match.nameIndices)),
      })),
    });
  }
  return tree;
}

export interface VisibleRow {
  node: TreeNode;
  level: number;
  parentId: string | null;
  posInSet: number;
  setSize: number;
  expanded: boolean;
}

export const labelOf = (node: TreeNode): string => (node.kind === "branch" ? node.label : node.item.name);

/** Flattens the tree into the rows currently on screen, in display order. */
export function flattenVisible(tree: TreeNode[], isExpanded: (id: string) => boolean): VisibleRow[] {
  const rows: VisibleRow[] = [];
  const walk = (nodes: TreeNode[], level: number, parentId: string | null) => {
    nodes.forEach((node, idx) => {
      const expanded = node.kind === "branch" && isExpanded(node.id);
      rows.push({ node, level, parentId, posInSet: idx + 1, setSize: nodes.length, expanded });
      if (node.kind === "branch" && expanded) walk(node.children, level + 1, node.id);
    });
  };
  walk(tree, 1, null);
  return rows;
}

export function firstLeaf(tree: TreeNode[]): TreeNode | undefined {
  for (const node of tree) {
    if (node.kind === "leaf") return node;
    const hit = firstLeaf(node.children);
    if (hit) return hit;
  }
  return undefined;
}

export function countLeaves(tree: TreeNode[]): number {
  let n = 0;
  const walk = (nodes: TreeNode[]) => {
    for (const node of nodes) {
      if (node.kind === "leaf") n++;
      else walk(node.children);
    }
  };
  walk(tree);
  return n;
}
