import { useEffect, useRef, useState } from "react";
import styles from "./App.module.css";
import fixture from "./fixture/subactions.json";
import type { SubAction } from "./model";
import { SubActionPicker } from "./SubActionPicker";

const FAVOURITES_KEY = "streamden.b3.grok.favourites";
const RECENTS_KEY = "streamden.b3.grok.recents";

const items: SubAction[] = fixture;

function readIds(key: string): string[] {
  try {
    const raw = localStorage.getItem(key);
    if (!raw) return [];
    const parsed: unknown = JSON.parse(raw);
    if (!Array.isArray(parsed)) return [];
    return parsed.filter((entry): entry is string => typeof entry === "string");
  } catch {
    return [];
  }
}

export function App() {
  const triggerRef = useRef<HTMLButtonElement>(null);
  const [open, setOpen] = useState(false);
  const [favourites, setFavourites] = useState<string[]>(() => readIds(FAVOURITES_KEY));
  const [recents, setRecents] = useState<string[]>(() => readIds(RECENTS_KEY));
  const [added, setAdded] = useState<SubAction[]>([]);

  useEffect(() => {
    localStorage.setItem(FAVOURITES_KEY, JSON.stringify(favourites));
  }, [favourites]);

  useEffect(() => {
    localStorage.setItem(RECENTS_KEY, JSON.stringify(recents));
  }, [recents]);

  function toggleFavourite(id: string) {
    setFavourites((current) => (current.includes(id) ? current.filter((entry) => entry !== id) : [...current, id]));
  }

  function select(item: SubAction) {
    setAdded((current) => [item, ...current.filter((entry) => entry.id !== item.id)].slice(0, 12));
    setRecents((current) => [item.id, ...current.filter((entry) => entry !== item.id)].slice(0, 8));
    setOpen(false);
  }

  return (
    <main className={styles.page}>
      <div className={styles.layout}>
        <p className={styles.kicker}>Action editor</p>
        <h1>Welcome raid</h1>
        <p className={styles.lede}>Build the sub-action list. Search, star favourites, and pick with the keyboard.</p>
        <button ref={triggerRef} type="button" className={styles.add} onClick={() => setOpen(true)}>
          Add sub-action
        </button>
        <section className={styles.added} aria-label="Added sub-actions">
          <h2>Added</h2>
          {added.length === 0 ? <p className={styles.empty}>Nothing added yet.</p> : null}
          <ul className={styles.list}>
            {added.map((item) => (
              <li key={item.id} className={styles.chip}>
                {item.name}
                <span className={styles.empty}> · {item.category.join(" / ")}</span>
              </li>
            ))}
          </ul>
        </section>
      </div>
      {open ? (
        <SubActionPicker
          items={items}
          favourites={favourites}
          recents={recents}
          onToggleFavourite={toggleFavourite}
          onSelect={select}
          onClose={() => setOpen(false)}
          returnFocusRef={triggerRef}
        />
      ) : null}
    </main>
  );
}
