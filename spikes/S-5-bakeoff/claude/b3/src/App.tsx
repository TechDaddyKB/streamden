import { useEffect, useRef, useState } from "react";
import { SubActionPicker } from "./SubActionPicker";
import { SUB_ACTIONS } from "./fixture";
import styles from "./App.module.css";

export function App() {
  const [open, setOpen] = useState(true);
  const [added, setAdded] = useState<string[]>([]);
  const trigger = useRef<HTMLButtonElement>(null);
  const wasOpen = useRef(open);

  // Return focus to the trigger when the picker closes.
  useEffect(() => {
    if (wasOpen.current && !open) trigger.current?.focus();
    wasOpen.current = open;
  }, [open]);
  return (
    <main className={styles.app}>
      {open ? (
        <SubActionPicker
          items={SUB_ACTIONS}
          onSelect={(item) => setAdded((a) => [...a, item.name])}
          onClose={() => setOpen(false)}
        />
      ) : (
        <button ref={trigger} type="button" onClick={() => setOpen(true)}>
          Add sub-action…
        </button>
      )}
      <p className={styles.log} aria-live="polite">
        {added.length > 0 ? `Added: ${added.join(", ")}` : "Nothing added yet."}
      </p>
    </main>
  );
}
