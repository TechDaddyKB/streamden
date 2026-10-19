import { StrictMode, useEffect, useState } from "react";
import { createRoot } from "react-dom/client";
import { getCurrentWindow } from "@tauri-apps/api/window";
import {
  command,
  getState,
  inTauri,
  onAnimationToggle,
  onTick,
  reportFirstPaint,
  type EngineState,
  type Tick,
} from "./bridge";
import "./styles.css";

function App() {
  const [state, setState] = useState<EngineState | null>(null);
  const [tick, setTick] = useState<Tick | null>(null);
  const [animate, setAnimate] = useState(
    !new URLSearchParams(location.search).has("still"),
  );
  const [size, setSize] = useState(`${innerWidth}×${innerHeight}`);

  useEffect(() => {
    void getState().then(setState);
    const offTick = onTick(setTick);
    const offAnim = onAnimationToggle(setAnimate);
    const onResize = () => setSize(`${innerWidth}×${innerHeight}`);
    addEventListener("resize", onResize);
    reportFirstPaint();
    return () => {
      offTick();
      offAnim();
      removeEventListener("resize", onResize);
    };
  }, []);

  // Refresh the static part (first-paint ms) once after the ack round-trip.
  useEffect(() => {
    const t = setTimeout(() => void getState().then(setState), 1500);
    return () => clearTimeout(t);
  }, []);

  const toggleFullscreen = async () => {
    if (!inTauri) return;
    const w = getCurrentWindow();
    await w.setFullscreen(!(await w.isFullscreen()));
  };

  return (
    <main className={animate ? "" : "paused"}>
      <header>
        <h1>StreamDen · S-1 Tauri shell</h1>
        <span className="badge">
          {inTauri ? "Tauri webview" : "browser fallback"} · level{" "}
          {state?.level ?? "?"} ({state?.level_name ?? "…"})
        </span>
      </header>

      <section className="stage" aria-label="animation">
        <div className="spinner" />
        <div className="slider" />
        <div className="pulse">60 fps CSS</div>
      </section>

      <section className="panel" aria-label="engine status">
        <h2>Engine</h2>
        <dl>
          <dt>tick</dt>
          <dd className="big" data-testid="tick">
            {tick?.n ?? state?.ticks ?? "—"}
          </dd>
          <dt>engine uptime</dt>
          <dd>{(((tick?.uptime_ms ?? state?.uptime_ms) ?? 0) / 1000).toFixed(0)} s</dd>
          <dt>open webviews</dt>
          <dd>{tick?.webviews ?? "—"}</dd>
          <dt>viewport</dt>
          <dd>{size} @ dpr {devicePixelRatio}</dd>
          <dt>first paint</dt>
          <dd>
            {state?.first_paint_ms != null
              ? `${state.first_paint_ms.toFixed(0)} ms after process start`
              : "—"}
          </dd>
          <dt>compat source</dt>
          <dd>{state?.level_source ?? "—"}</dd>
          <dt>env applied</dt>
          <dd>
            {state?.env_applied.length
              ? state.env_applied.map(([k, v]) => `${k}=${v}`).join(" ")
              : "none"}
          </dd>
          <dt>NVIDIA quirk</dt>
          <dd>{state?.quirk_status ?? "—"}</dd>
          <dt>GDK_BACKEND</dt>
          <dd>{state?.session.GDK_BACKEND ?? "(unset)"}</dd>
          <dt>UI in browser</dt>
          <dd>{state?.http_url ?? "—"}</dd>
        </dl>
        {inTauri && (
          <div className="buttons">
            <button onClick={() => void toggleFullscreen()}>Fullscreen</button>
            <button onClick={() => void command("open_chat_cmd")}>Chat window</button>
            <button onClick={() => void command("hide_to_tray_cmd")}>
              Close to tray
            </button>
            <button onClick={() => setAnimate((a) => !a)}>
              {animate ? "Pause" : "Resume"} animation
            </button>
          </div>
        )}
      </section>
    </main>
  );
}

createRoot(document.getElementById("root")!).render(
  <StrictMode>
    <App />
  </StrictMode>,
);
