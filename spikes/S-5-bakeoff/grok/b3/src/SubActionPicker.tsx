import { useEffect, useId, useMemo, useRef, useState } from "react";
import type { KeyboardEvent as ReactKeyboardEvent, RefObject } from "react";
import {
  bestLeafId,
  buildTree,
  defaultCollapsed,
  flatten,
  neighbourId,
  onTreeKey,
  pushTypeAhead,
  typeAhead,
  type FlatRow,
  type SubAction,
  type TreeKey,
  type TypeAheadState,
} from "./model";
import styles from "./SubActionPicker.module.css";

const TREE_KEYS = new Set<TreeKey>(["ArrowDown", "ArrowUp", "ArrowLeft", "ArrowRight", "Home", "End", "Enter", " "]);

export type SubActionPickerProps = {
  items: readonly SubAction[];
  favourites: readonly string[];
  recents: readonly string[];
  onToggleFavourite: (id: string) => void;
  onSelect: (item: SubAction) => void;
  onClose: () => void;
  returnFocusRef?: RefObject<HTMLElement | null>;
};

function isTreeKey(key: string): key is TreeKey {
  return TREE_KEYS.has(key as TreeKey);
}

function accessibleName(row: FlatRow, favourite: boolean): string {
  if (row.node.kind === "branch") return row.node.label;
  const path = row.node.item.category.join(" / ");
  return `${row.node.item.name}, ${path}${favourite ? ", favourite" : ""}`;
}

export function SubActionPicker({
  items,
  favourites,
  recents,
  onToggleFavourite,
  onSelect,
  onClose,
  returnFocusRef,
}: SubActionPickerProps) {
  const titleId = useId();
  const helpId = useId();
  const searchRef = useRef<HTMLInputElement>(null);
  const closeRef = useRef<HTMLButtonElement>(null);
  const treeRef = useRef<HTMLDivElement>(null);
  const [query, setQuery] = useState("");
  const [focusId, setFocusId] = useState<string | null>(null);
  const [collapsed, setCollapsed] = useState<Set<string>>(() => new Set(defaultCollapsed(buildTree(items, "", [], []))));
  const typeAheadState = useRef<TypeAheadState | null>(null);
  const previousRows = useRef<FlatRow[]>([]);
  const queryRef = useRef(query);

  const searching = query.trim().length > 0;
  const tree = useMemo(() => buildTree(items, query, favourites, recents), [items, query, favourites, recents]);
  const rows = useMemo(() => flatten(tree, collapsed, searching), [tree, collapsed, searching]);
  const favouriteSet = useMemo(() => new Set(favourites), [favourites]);

  const focused = rows.find((row) => row.id === focusId) ?? null;
  const footer = focused?.node.kind === "leaf" ? focused.node.item.description : "Choose a sub-action to add it to the action.";

  useEffect(() => {
    searchRef.current?.focus();
    const returnTo = returnFocusRef?.current ?? null;
    return () => {
      returnTo?.focus();
    };
  }, [returnFocusRef]);

  useEffect(() => {
    const queryChanged = queryRef.current !== query;
    queryRef.current = query;
    if (queryChanged) {
      setFocusId(query.trim() ? bestLeafId(rows, query) : null);
      previousRows.current = rows;
      return;
    }
    if (focusId && !rows.some((row) => row.id === focusId)) {
      const next = searching ? bestLeafId(rows, query) : neighbourId(previousRows.current, rows, focusId);
      setFocusId(next);
      if (next && document.activeElement !== searchRef.current) {
        document.getElementById(next)?.focus();
      }
    }
    previousRows.current = rows;
  }, [rows, query, focusId, searching]);

  useEffect(() => {
    if (!focusId) return;
    document.getElementById(focusId)?.scrollIntoView?.({ block: "nearest" });
  }, [focusId]);

  function move(key: TreeKey) {
    const result = onTreeKey(rows, [...collapsed], focusId, key, searching);
    if (!searching) setCollapsed(new Set(result.collapsed));
    setFocusId(result.focusId);
    if (result.focusId && document.activeElement !== searchRef.current) {
      document.getElementById(result.focusId)?.focus();
    }
    if (key === "ArrowDown" || key === "ArrowUp" || key === "Home" || key === "End" || key === "ArrowLeft" || key === "ArrowRight") {
      if (result.focusId && document.activeElement === searchRef.current) {
        document.getElementById(result.focusId)?.focus();
      }
    }
    if (result.activateItemId) {
      const item = items.find((entry) => entry.id === result.activateItemId);
      if (item) onSelect(item);
    }
  }

  function onKeyDown(event: ReactKeyboardEvent<HTMLDivElement>) {
    if (event.altKey && event.key.toLowerCase() === "f") {
      event.preventDefault();
      if (focused?.node.kind === "leaf") onToggleFavourite(focused.node.item.id);
      return;
    }
    if (event.key === "Escape") {
      event.preventDefault();
      event.stopPropagation();
      if (query) {
        setQuery("");
        setFocusId(null);
        searchRef.current?.focus();
        return;
      }
      onClose();
      return;
    }
    if (event.key === "Tab") {
      const nodes: HTMLElement[] = [];
      if (closeRef.current) nodes.push(closeRef.current);
      if (searchRef.current) nodes.push(searchRef.current);
      if (treeRef.current) nodes.push(treeRef.current);
      const active = document.activeElement;
      const current = nodes.findIndex((node) => node === active || node.contains(active));
      const index = current === -1 ? 0 : current;
      const nextIndex = event.shiftKey ? (index - 1 + nodes.length) % nodes.length : (index + 1) % nodes.length;
      event.preventDefault();
      nodes[nextIndex]?.focus();
      return;
    }
    if (event.ctrlKey || event.metaKey || event.altKey) return;

    const inSearch = event.target === searchRef.current;
    if (isTreeKey(event.key)) {
      if (inSearch && (event.key === "ArrowLeft" || event.key === "ArrowRight" || event.key === " ")) return;
      event.preventDefault();
      move(event.key);
      return;
    }
    if (inSearch || event.key.length !== 1) return;
    event.preventDefault();
    const now = Date.now();
    const next = pushTypeAhead(typeAheadState.current, event.key, now);
    typeAheadState.current = next;
    const match = typeAhead(rows, focusId, next.buffer);
    if (match) {
      setFocusId(match);
      document.getElementById(match)?.focus();
    }
  }

  function onRowClick(row: FlatRow) {
    setFocusId(row.id);
    if (row.node.kind === "branch") {
      if (searching) return;
      setCollapsed((current) => {
        const next = new Set(current);
        if (next.has(row.id)) next.delete(row.id);
        else next.add(row.id);
        return next;
      });
      return;
    }
    onSelect(row.node.item);
  }

  return (
    <div className={styles.backdrop} onMouseDown={(event) => event.target === event.currentTarget && onClose()}>
      <div
        className={styles.panel}
        role="dialog"
        aria-modal="true"
        aria-labelledby={titleId}
        aria-describedby={helpId}
        data-testid="subaction-picker"
        onKeyDown={onKeyDown}
      >
        <div className={styles.header}>
          <h2 id={titleId} className={styles.title}>
            Add sub-action
          </h2>
          <button ref={closeRef} type="button" className={styles.close} onClick={onClose}>
            Close
          </button>
        </div>
        <p id={helpId} className={styles.srOnly}>
          Search filters the tree. Arrow keys move, Enter adds the focused sub-action, Escape closes, and Alt+F toggles a favourite.
        </p>
        <div className={styles.searchWrap}>
          <input
            ref={searchRef}
            className={styles.search}
            role="searchbox"
            aria-label="Search sub-actions"
            aria-controls="subaction-tree"
            aria-activedescendant={focusId ?? undefined}
            placeholder="Search sub-actions"
            value={query}
            onChange={(event) => setQuery(event.target.value)}
          />
        </div>
        <div ref={treeRef} id="subaction-tree" className={styles.tree} role="tree" aria-label="Sub-actions" tabIndex={-1}>
          {rows.length === 0 ? <p className={styles.empty}>No sub-actions match.</p> : null}
          {rows.map((row) => {
            const node = row.node;
            const favourite = node.kind === "leaf" && favouriteSet.has(node.item.id);
            return (
              <div
                key={row.id}
                id={row.id}
                role="treeitem"
                aria-level={row.depth + 1}
                aria-setsize={row.setsize}
                aria-posinset={row.posinset}
                aria-expanded={node.kind === "branch" ? row.expanded : undefined}
                aria-selected={row.id === focusId}
                aria-label={accessibleName(row, favourite)}
                tabIndex={row.id === focusId ? 0 : -1}
                className={`${styles.row} ${node.kind === "branch" ? styles.branch : ""}`}
                style={{ paddingLeft: `${8 + row.depth * 16}px` }}
                onClick={() => onRowClick(row)}
              >
                <span className={styles.twist} aria-hidden="true">
                  {node.kind === "branch" ? (row.expanded ? "▾" : "▸") : ""}
                </span>
                <span className={styles.name}>{node.kind === "branch" ? node.label : node.item.name}</span>
                {node.kind === "leaf" ? <span className={styles.path}>{node.item.category.join(" / ")}</span> : null}
                {node.kind === "leaf" ? (
                  <span
                    className={favourite ? styles.starOn : styles.star}
                    aria-hidden="true"
                    data-testid={`star-${node.item.id}`}
                    onClick={(event) => {
                      event.stopPropagation();
                      onToggleFavourite(node.item.id);
                    }}
                  >
                    ★
                  </span>
                ) : null}
              </div>
            );
          })}
        </div>
        <div className={styles.footer} aria-live="polite">
          {footer}
        </div>
      </div>
    </div>
  );
}
