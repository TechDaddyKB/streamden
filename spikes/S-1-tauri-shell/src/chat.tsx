import { StrictMode, useEffect, useRef, useState } from "react";
import { createRoot } from "react-dom/client";
import { onTick, reportFirstPaint } from "./bridge";
import "./styles.css";

const USERS = ["den_bot", "viewer42", "modkitty", "raidboss", "lurker"];
const LINES = [
  "hello chat!",
  "PogChamp",
  "that alert was clean",
  "!so @raidboss",
  "first time here, love the overlay",
];

interface Line {
  id: number;
  user: string;
  text: string;
}

// Mock multichat: one fake message every 3 engine ticks.
function Chat() {
  const [lines, setLines] = useState<Line[]>([]);
  const end = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const off = onTick((t) => {
      if (t.n % 3 !== 0) return;
      setLines((prev) =>
        [
          ...prev,
          {
            id: t.n,
            user: USERS[t.n % USERS.length],
            text: `${LINES[t.n % LINES.length]} (tick ${t.n})`,
          },
        ].slice(-40),
      );
    });
    reportFirstPaint();
    return off;
  }, []);

  useEffect(() => end.current?.scrollIntoView(), [lines]);

  return (
    <div className="chat">
      <div className="chat-head">Chat (always on top)</div>
      <div className="chat-body">
        {lines.length === 0 && <div className="msg muted">waiting for engine ticks…</div>}
        {lines.map((l) => (
          <div className="msg" key={l.id}>
            <b>{l.user}</b> {l.text}
          </div>
        ))}
        <div ref={end} />
      </div>
    </div>
  );
}

createRoot(document.getElementById("root")!).render(
  <StrictMode>
    <Chat />
  </StrictMode>,
);
