#!/usr/bin/env node
// Browser half of the control-origin regression (DEN-3 review P2), zero dependencies (Node >= 22).
// Drives a headless Chromium over CDP: a widget page (w-a.localhost) fires simple no-cors POSTs at /emit and
// /api/chat, then the app dock page does the same. Prints the HTTP status the server logged for each.
// Usage: node origin-browser.mjs <port> <chromium-binary> <user-data-dir>
import { spawn } from 'node:child_process';

const [port, chrome, profile] = process.argv.slice(2);
const CDP_PORT = 9333 + (Number(port) % 100);
const app = `http://127.0.0.1:${port}`;
const sleep = ms => new Promise(r => setTimeout(r, ms));

const proc = spawn(chrome, ['--headless=new', `--remote-debugging-port=${CDP_PORT}`, `--user-data-dir=${profile}`,
  '--no-first-run', '--no-default-browser-check', 'about:blank'], { stdio: 'ignore' });

async function cdpTarget() {
  for (let i = 0; i < 100; i++) {
    try {
      const list = await (await fetch(`http://127.0.0.1:${CDP_PORT}/json/list`)).json();
      const page = list.find(t => t.type === 'page');
      if (page) return page.webSocketDebuggerUrl;
    } catch (_) { /* not up yet */ }
    await sleep(100);
  }
  throw new Error('chromium CDP did not come up');
}

function session(url) {
  return new Promise((resolve, reject) => {
    const ws = new WebSocket(url);
    let n = 0;
    const pending = new Map();
    ws.onerror = () => reject(new Error('cdp websocket error'));
    ws.onmessage = m => {
      const msg = JSON.parse(m.data);
      if (msg.id && pending.has(msg.id)) { pending.get(msg.id)(msg); pending.delete(msg.id); }
    };
    ws.onopen = () => resolve({
      send: (method, params = {}) => new Promise(res => { const id = ++n; pending.set(id, res); ws.send(JSON.stringify({ id, method, params })); }),
      close: () => ws.close(),
    });
  });
}

// Widget: simple (CORS-safelisted) no-cors POSTs, exactly what untrusted widget code can send without a preflight.
// Dock: the same-origin JSON fetch that assets/dock.html uses.
const fire = label => {
  const init = label === 'widget'
    ? `{ method: 'POST', mode: 'no-cors', headers: { 'content-type': 'text/plain' }, body }`
    : `{ method: 'POST', headers: { 'content-type': 'application/json' }, body }`;
  return `Promise.all(['/emit', '/api/chat'].map(p => {
    const body = JSON.stringify({ text: 'origin-browser ${label}' });
    return fetch('${app}' + p, ${init}).then(r => r.type === 'opaque' ? 'sent' : r.status, e => 'error ' + e.message);
  }))`;
};

async function main() {
  const cdp = await session(await cdpTarget());
  const out = {};
  for (const [label, url] of [['widget', `http://w-a.localhost:${port}/?run=origin-check`], ['dock', `${app}/dock/chat`]]) {
    await cdp.send('Page.enable');
    await cdp.send('Page.navigate', { url });
    await sleep(1500);
    const loc = await cdp.send('Runtime.evaluate', { expression: 'location.origin', returnByValue: true });
    const r = await cdp.send('Runtime.evaluate', { expression: fire(label), awaitPromise: true, returnByValue: true });
    out[label] = { origin: loc.result?.result?.value, fetch: r.result?.result?.value };
  }
  cdp.close();
  console.log(JSON.stringify(out));
}
main().catch(e => { console.error(e.message); process.exitCode = 1; }).finally(() => proc.kill());
