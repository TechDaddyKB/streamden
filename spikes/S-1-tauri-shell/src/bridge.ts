// One UI, two transports: Tauri IPC inside the webview, HTTP + SSE when the
// same page is opened in a browser (the crash-loop fallback).
import { invoke, isTauri } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

export interface Tick {
  n: number;
  uptime_ms: number;
  webviews: number;
}

export interface EngineState {
  ticks: number;
  uptime_ms: number;
  level: number;
  level_name: string;
  level_source: string;
  browser_fallback: boolean;
  env_applied: [string, string][];
  quirk_registered: boolean;
  quirk_status: string;
  session: Record<string, string | null>;
  first_paint_ms: number | null;
  http_url: string | null;
  pid: number;
}

export const inTauri = isTauri();

export async function getState(): Promise<EngineState> {
  if (inTauri) return invoke<EngineState>("get_state");
  const r = await fetch("/api/state");
  return (await r.json()) as EngineState;
}

export function onTick(cb: (t: Tick) => void): () => void {
  if (inTauri) {
    let un: (() => void) | undefined;
    let dead = false;
    void listen<Tick>("engine-tick", (e) => cb(e.payload)).then((u) => {
      if (dead) u();
      else un = u;
    });
    return () => {
      dead = true;
      un?.();
    };
  }
  const es = new EventSource("/api/ticks");
  es.onmessage = (e) => cb(JSON.parse(e.data) as Tick);
  return () => es.close();
}

export function onAnimationToggle(cb: (on: boolean) => void): () => void {
  if (!inTauri) return () => {};
  let un: (() => void) | undefined;
  let dead = false;
  void listen<boolean>("set-animation", (e) => cb(e.payload)).then((u) => {
    if (dead) u();
    else un = u;
  });
  return () => {
    dead = true;
    un?.();
  };
}

// Report first paint after two animation frames, i.e. after the first frame
// containing the rendered React tree has been produced by WebKit.
export function reportFirstPaint(): void {
  requestAnimationFrame(() =>
    requestAnimationFrame(() => {
      const webviewMs = performance.now();
      if (inTauri) {
        void invoke("first_paint", { webviewMs });
      }
    }),
  );
}

export async function command(name: string): Promise<void> {
  if (inTauri) await invoke(name);
}
