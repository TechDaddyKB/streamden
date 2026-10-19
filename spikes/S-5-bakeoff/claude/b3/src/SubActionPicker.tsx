import { useCallback, useEffect, useId, useMemo, useRef, useState, type KeyboardEvent, type ReactNode } from "react";
import {
  FAVOURITES_ID,
  RECENT_ID,
  buildTree,
  countLeaves,
  firstLeaf,
  flattenVisible,
  labelOf,
  type SubActionDef,
  type VisibleRow,
} from "./model";
import styles from "./SubActionPicker.module.css";

export type StorageLike = Pick<Storage, "getItem" | "setItem">;

export interface SubActionPickerProps {
  items: SubActionDef[];
  onSelect: (item: SubActionDef) => void;
  onClose?: () => void;
  /** Where favourites and recents persist. Defaults to localStorage; pass null to disable. */
  storage?: StorageLike | null;
  maxRecent?: number;
  /** Type-ahead buffer reset delay in ms. */
  typeAheadMs?: number;
}

export const STORAGE_KEY = "streamden.subActionPicker.v1";

interface Persisted {
  favourites: string[];
  recent: string[];
}

function load(storage: StorageLike | null): Persisted {
  try {
    const raw = storage?.getItem(STORAGE_KEY);
    if (raw) {
      const p = JSON.parse(raw) as Partial<Persisted>;
      const strings = (v: unknown) => (Array.isArray(v) ? v.filter((x): x is string => typeof x === "string") : []);
      return { favourites: strings(p.favourites), recent: strings(p.recent) };
    }
  } catch {
    // Corrupt or unavailable storage: start empty.
  }
  return { favourites: [], recent: [] };
}

const defaultStorage = (): StorageLike | null => {
  try {
    return typeof window !== "undefined" ? window.localStorage : null;
  } catch {
    return null;
  }
};

function Highlight({ text, indices }: { text: string; indices: number[] }): ReactNode {
  if (indices.length === 0) return text;
  const set = new Set(indices);
  const parts: ReactNode[] = [];
  let i = 0;
  while (i < text.length) {
    const hit = set.has(i);
    let j = i;
    while (j < text.length && set.has(j) === hit) j++;
    const chunk = text.slice(i, j);
    parts.push(hit ? <mark key={i} className={styles.mark}>{chunk}</mark> : chunk);
    i = j;
  }
  return parts;
}

export function SubActionPicker({
  items,
  onSelect,
  onClose,
  storage = defaultStorage(),
  maxRecent = 5,
  typeAheadMs = 500,
}: SubActionPickerProps) {
  const baseId = useId();
  const treeId = `${baseId}-tree`;
  const headingId = `${baseId}-heading`;
  const hintId = `${baseId}-hint`;
  const domId = (nodeId: string) => `${baseId}-n-${nodeId.replace(/[^A-Za-z0-9_-]/g, "_")}`;

  const [{ favourites, recent }, setPersisted] = useState(() => load(storage));
  const [query, setQuery] = useState("");
  const [expanded, setExpanded] = useState<Set<string>>(() => new Set([FAVOURITES_ID, RECENT_ID]));
  // While searching every branch starts open; this tracks the ones the user closed for the current query.
  const [searchCollapsed, setSearchCollapsed] = useState<Set<string>>(() => new Set());
  const [activeId, setActiveId] = useState<string | null>(null);

  const searchRef = useRef<HTMLInputElement>(null);
  const treeRef = useRef<HTMLDivElement>(null);
  const lastIndex = useRef(0);
  const rowRefs = useRef(new Map<string, HTMLDivElement>());
  const focusPending = useRef(false);
  const typeAhead = useRef({ buffer: "", timer: 0 as ReturnType<typeof setTimeout> | 0 });

  const searching = query.trim().length > 0;

  const tree = useMemo(() => buildTree({ items, query, favourites, recent }), [items, query, favourites, recent]);
  const isExpanded = useCallback(
    (id: string) => (searching ? !searchCollapsed.has(id) : expanded.has(id)),
    [searching, searchCollapsed, expanded],
  );
  const rows = useMemo(() => flattenVisible(tree, isExpanded), [tree, isExpanded]);
  const matchCount = useMemo(() => (searching ? countLeaves(tree) : 0), [searching, tree]);

  const bestLeaf = searching ? firstLeaf(tree) : undefined;
  let currentId: string | null;
  if (activeId !== null && rows.some((r) => r.node.id === activeId)) currentId = activeId;
  // The active row vanished (e.g. un-favourited inside Favourites): stay at the same position.
  else if (activeId !== null && rows.length > 0) currentId = rows[Math.min(lastIndex.current, rows.length - 1)]!.node.id;
  else if (bestLeaf && rows.some((r) => r.node.id === bestLeaf.id)) currentId = bestLeaf.id;
  else currentId = rows[0]?.node.id ?? null;
  const currentIndex = rows.findIndex((r) => r.node.id === currentId);
  const current: VisibleRow | undefined = rows[currentIndex];

  useEffect(() => {
    if (storage === null) return;
    try {
      storage.setItem(STORAGE_KEY, JSON.stringify({ favourites, recent }));
    } catch {
      // Quota or privacy mode: favourites just won't persist.
    }
  }, [storage, favourites, recent]);

  useEffect(() => {
    searchRef.current?.focus();
  }, []);

  useEffect(() => {
    if (!focusPending.current || currentId === null) return;
    focusPending.current = false;
    const el = rowRefs.current.get(currentId);
    el?.focus();
    el?.scrollIntoView?.({ block: "nearest" });
  });

  useEffect(() => {
    lastIndex.current = Math.max(currentIndex, 0);
  });

  useEffect(() => () => clearTimeout(typeAhead.current.timer || undefined), []);

  const moveTo = (id: string | undefined) => {
    if (id === undefined) return;
    focusPending.current = true;
    setActiveId(id);
  };

  const setBranchOpen = (id: string, open: boolean) => {
    if (searching) {
      setSearchCollapsed((prev) => {
        const next = new Set(prev);
        if (open) next.delete(id);
        else next.add(id);
        return next;
      });
    } else {
      setExpanded((prev) => {
        const next = new Set(prev);
        if (open) next.add(id);
        else next.delete(id);
        return next;
      });
    }
  };

  const choose = (item: SubActionDef) => {
    setPersisted((p) => ({ ...p, recent: [item.id, ...p.recent.filter((i) => i !== item.id)].slice(0, maxRecent) }));
    onSelect(item);
  };

  const toggleFavourite = (item: SubActionDef) => {
    // The row may disappear (Favourites section); keep keyboard focus inside the tree.
    if (treeRef.current?.contains(document.activeElement)) focusPending.current = true;
    setPersisted((p) => ({
      ...p,
      favourites: p.favourites.includes(item.id) ? p.favourites.filter((i) => i !== item.id) : [...p.favourites, item.id],
    }));
  };

  const activate = (row: VisibleRow) => {
    if (row.node.kind === "leaf") choose(row.node.item);
    else setBranchOpen(row.node.id, !row.expanded);
  };

  const clearOrClose = () => {
    if (query !== "") {
      setQuery("");
      setSearchCollapsed(new Set());
      setActiveId(null);
      searchRef.current?.focus();
    } else {
      onClose?.();
    }
  };

  const runTypeAhead = (ch: string) => {
    const ta = typeAhead.current;
    clearTimeout(ta.timer || undefined);
    ta.buffer += ch.toLowerCase();
    ta.timer = setTimeout(() => (ta.buffer = ""), typeAheadMs);
    // Repeating one key cycles through items with that initial, like a native tree.
    const repeated = ta.buffer.length > 1 && [...ta.buffer].every((c) => c === ta.buffer[0]);
    const needle = repeated ? ta.buffer[0]! : ta.buffer;
    const start = needle.length === 1 ? currentIndex + 1 : Math.max(currentIndex, 0);
    for (let k = 0; k < rows.length; k++) {
      const row = rows[(start + k) % rows.length]!;
      if (labelOf(row.node).toLowerCase().startsWith(needle)) {
        moveTo(row.node.id);
        return;
      }
    }
  };

  const onTreeKeyDown = (e: KeyboardEvent<HTMLDivElement>) => {
    if (!current) return;
    const { node } = current;
    let handled = true;
    switch (e.key) {
      case "ArrowDown":
        moveTo(rows[currentIndex + 1]?.node.id);
        break;
      case "ArrowUp":
        if (currentIndex === 0) searchRef.current?.focus();
        else moveTo(rows[currentIndex - 1]?.node.id);
        break;
      case "Home":
        moveTo(rows[0]?.node.id);
        break;
      case "End":
        moveTo(rows[rows.length - 1]?.node.id);
        break;
      case "ArrowRight":
        if (node.kind === "branch") {
          if (!current.expanded) setBranchOpen(node.id, true);
          else moveTo(node.children[0]?.id);
        }
        break;
      case "ArrowLeft":
        if (node.kind === "branch" && current.expanded) setBranchOpen(node.id, false);
        else if (current.parentId !== null) moveTo(current.parentId);
        break;
      case "Enter":
      case " ":
        activate(current);
        break;
      case "Escape":
        clearOrClose();
        break;
      default:
        if (e.altKey && !e.ctrlKey && !e.metaKey && e.code === "KeyF") {
          if (node.kind === "leaf") toggleFavourite(node.item);
        } else if (e.key === "/") {
          searchRef.current?.focus();
        } else if (e.key.length === 1 && !e.ctrlKey && !e.metaKey && !e.altKey) {
          runTypeAhead(e.key);
        } else {
          handled = false;
        }
    }
    if (handled) {
      e.preventDefault();
      e.stopPropagation();
    }
  };

  const onSearchKeyDown = (e: KeyboardEvent<HTMLInputElement>) => {
    switch (e.key) {
      case "ArrowDown":
        moveTo(currentId ?? undefined);
        break;
      case "Enter": {
        // With a query, Enter adds the best-scoring match; without one it only adds an explicitly current leaf.
        const node = searching ? firstLeaf(tree) : undefined;
        if (node?.kind === "leaf") choose(node.item);
        break;
      }
      case "Escape":
        clearOrClose();
        break;
      default:
        return;
    }
    e.preventDefault();
    e.stopPropagation();
  };

  const activeItem = current?.node.kind === "leaf" ? current.node.item : undefined;

  return (
    <section className={styles.picker} aria-labelledby={headingId}>
      <header className={styles.header}>
        <h2 id={headingId} className={styles.title}>
          Add sub-action
        </h2>
        {onClose && (
          <button type="button" className={styles.close} onClick={onClose} aria-label="Close">
            ×
          </button>
        )}
      </header>

      <input
        ref={searchRef}
        type="search"
        className={styles.search}
        placeholder="Search sub-actions…"
        aria-label="Search sub-actions"
        aria-controls={treeId}
        aria-describedby={hintId}
        autoComplete="off"
        spellCheck={false}
        value={query}
        onChange={(e) => {
          setQuery(e.target.value);
          setSearchCollapsed(new Set());
          setActiveId(null);
        }}
        onKeyDown={onSearchKeyDown}
      />

      <div role="status" className={styles.status}>
        {searching ? (matchCount === 0 ? "No sub-actions match" : `${matchCount} sub-action${matchCount === 1 ? "" : "s"} match`) : ""}
      </div>

      <div ref={treeRef} id={treeId} role="tree" aria-labelledby={headingId} className={styles.tree} onKeyDown={onTreeKeyDown}>
        {rows.map((row) => {
          const { node } = row;
          const isCurrent = node.id === currentId;
          const isLeaf = node.kind === "leaf";
          const fav = isLeaf && favourites.includes(node.item.id);
          return (
            <div
              key={node.id}
              id={domId(node.id)}
              ref={(el) => {
                if (el) rowRefs.current.set(node.id, el);
                else rowRefs.current.delete(node.id);
              }}
              role="treeitem"
              aria-level={row.level}
              aria-setsize={row.setSize}
              aria-posinset={row.posInSet}
              aria-expanded={isLeaf ? undefined : row.expanded}
              aria-selected={isLeaf ? isCurrent : undefined}
              tabIndex={isCurrent ? 0 : -1}
              className={[styles.row, isLeaf ? styles.leaf : styles.branch, isCurrent ? styles.current : ""].join(" ")}
              style={{ paddingLeft: `${0.5 + (row.level - 1) * 1.1}rem` }}
              onClick={() => {
                setActiveId(node.id);
                activate(row);
              }}
            >
              <span className={styles.chevron} aria-hidden="true">
                {isLeaf ? "" : row.expanded ? "▾" : "▸"}
              </span>
              <span className={styles.label}>
                {isLeaf ? <Highlight text={node.item.name} indices={node.highlight} /> : node.label}
              </span>
              {isLeaf && (
                <span
                  className={[styles.star, fav ? styles.starOn : ""].join(" ")}
                  aria-hidden="true"
                  title={fav ? "Remove from favourites (Alt+F)" : "Add to favourites (Alt+F)"}
                  data-testid={`star-${node.id}`}
                  onClick={(e) => {
                    e.stopPropagation();
                    toggleFavourite(node.item);
                  }}
                >
                  {fav ? "★" : "☆"}
                </span>
              )}
              {fav && <span className={styles.srOnly}> (favourite)</span>}
            </div>
          );
        })}
      </div>

      <footer className={styles.footer}>
        <p className={styles.description}>{activeItem ? activeItem.description : " "}</p>
        <p id={hintId} className={styles.hint}>
          ↑↓ move · ←→ fold · Enter add · Alt+F favourite · a–z jump · / search · Esc clear/close
        </p>
      </footer>
    </section>
  );
}
