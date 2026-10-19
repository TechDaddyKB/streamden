#!/usr/bin/env node
// Spike S-2 OBS driver: a zero-dependency obs-websocket v5 client (Node >= 22 global WebSocket).
// Usage: node s2.mjs <command> [...args]
//   version | setup <port> <run> | add <input> <url> [w h] | refresh <input> | seturl <input> <url> | recreate <input>
//   shot <source> <file.png> [width] [height] | call <RequestType> [json]
// Env: OBS_WS_URL (default ws://127.0.0.1:4466), OBS_WS_CONFIG (obs-websocket config.json with the password)
import { createHash } from 'node:crypto';
import { readFileSync, writeFileSync } from 'node:fs';

const URL_ = process.env.OBS_WS_URL || 'ws://127.0.0.1:4466';
const CFG = process.env.OBS_WS_CONFIG || '/tmp/den-s2-obs/config/obs-studio/plugin_config/obs-websocket/config.json';
const SCENE = 'DEN-S2';
const b64sha = s => createHash('sha256').update(s).digest('base64');

function connect() {
  const password = JSON.parse(readFileSync(CFG, 'utf8')).server_password;
  return new Promise((resolve, reject) => {
    const ws = new WebSocket(URL_, 'obswebsocket.json');
    const pending = new Map();
    let n = 0;
    const call = (requestType, requestData = {}) => new Promise((res, rej) => {
      const requestId = `r${++n}`;
      pending.set(requestId, { res, rej, requestType });
      ws.send(JSON.stringify({ op: 6, d: { requestType, requestId, requestData } }));
    });
    ws.onerror = e => reject(new Error('websocket error: ' + (e.message || 'connect failed')));
    ws.onmessage = m => {
      const { op, d } = JSON.parse(m.data);
      if (op === 0) {
        const identify = { rpcVersion: 1, eventSubscriptions: 0 };
        if (d.authentication) identify.authentication = b64sha(b64sha(password + d.authentication.salt) + d.authentication.challenge);
        ws.send(JSON.stringify({ op: 1, d: identify }));
      } else if (op === 2) {
        resolve({ call, close: () => ws.close() });
      } else if (op === 7) {
        const p = pending.get(d.requestId);
        if (!p) return;
        pending.delete(d.requestId);
        if (d.requestStatus.result) p.res(d.responseData || {});
        else p.rej(new Error(`${p.requestType}: ${d.requestStatus.code} ${d.requestStatus.comment || ''}`));
      }
    };
    ws.onclose = e => { if (e.code !== 1000 && e.code !== 1005) reject(new Error(`closed ${e.code} ${e.reason}`)); };
  });
}

// Probe CSS for the LNA check: a PUBLIC page (example.com) gets OBS custom CSS whose background images point at loopback.
const lnaCss = (port, tag) => `body { background-color: rgba(0,0,0,0); margin: 0px auto; overflow: hidden; }
html { background-image: url("http://127.0.0.1:${port}/probe/lna-${tag}-img-127"), url("http://localhost:${port}/probe/lna-${tag}-img-localhost"), url("http://w-lna.localhost:${port}/probe/lna-${tag}-img-wlocalhost"); }`;

function inputs(port, run) {
  const app = `http://127.0.0.1:${port}`;
  return [
    { name: 'S2 overlay', url: `${app}/o/alerts?debug=1`, w: 1920, h: 1080 },
    { name: 'S2 widgets iframes', url: `${app}/o/widgets?run=${run}`, w: 1920, h: 820 },
    { name: 'S2 w-a top', url: `http://w-a.localhost:${port}/?run=${run}`, w: 1600, h: 260 },
    { name: 'S2 w-b top', url: `http://w-b.localhost:${port}/?run=${run}`, w: 1600, h: 260 },
    { name: 'S2 se-sample', url: `http://w-se.localhost:${port}/sew/se-sample/`, w: 1280, h: 480 },
    { name: 'S2 lna https', url: 'https://example.com/', w: 800, h: 600, css: lnaCss(port, 'https') },
    { name: 'S2 lna http', url: 'http://example.com/', w: 800, h: 600, css: lnaCss(port, 'http') },
  ];
}
const settingsOf = i => ({ url: i.url, width: i.w, height: i.h, fps_custom: false, reroute_audio: false,
  shutdown: false, restart_when_active: false, ...(i.css ? { css: i.css } : {}) });

async function main() {
  const [cmd, ...args] = process.argv.slice(2);
  const obs = await connect();
  try {
    switch (cmd) {
      case 'version': console.log(JSON.stringify(await obs.call('GetVersion'))); break;
      case 'setup': {
        const [port, run = 'r1'] = args;
        const { scenes } = await obs.call('GetSceneList');
        if (scenes.some(s => s.sceneName === SCENE)) await obs.call('RemoveScene', { sceneName: SCENE });
        for (const i of inputs(port, run)) {
          try { await obs.call('RemoveInput', { inputName: i.name }); } catch (_) { /* not present */ }
        }
        await obs.call('CreateScene', { sceneName: SCENE });
        await obs.call('SetCurrentProgramScene', { sceneName: SCENE });
        for (const i of inputs(port, run)) {
          const r = await obs.call('CreateInput', { sceneName: SCENE, inputName: i.name, inputKind: 'browser_source',
            inputSettings: settingsOf(i), sceneItemEnabled: true });
          console.log(`created ${i.name} -> ${i.url} (item ${r.sceneItemId})`);
        }
        break;
      }
      case 'add': {
        const [name, url, w = 1600, h = 260] = args;
        await obs.call('CreateInput', { sceneName: SCENE, inputName: name, inputKind: 'browser_source',
          inputSettings: settingsOf({ url, w: Number(w), h: Number(h) }), sceneItemEnabled: true });
        console.log('added', name, url); break;
      }
      case 'refresh': await obs.call('PressInputPropertiesButton', { inputName: args[0], propertyName: 'refreshnocache' });
        console.log('refreshed', args[0]); break;
      case 'seturl': await obs.call('SetInputSettings', { inputName: args[0], inputSettings: { url: args[1] }, overlay: true });
        console.log('url set', args[0], args[1]); break;
      case 'recreate': {
        const { inputSettings } = await obs.call('GetInputSettings', { inputName: args[0] });
        await obs.call('RemoveInput', { inputName: args[0] });
        await new Promise(r => setTimeout(r, 500));
        await obs.call('CreateInput', { sceneName: SCENE, inputName: args[0], inputKind: 'browser_source', inputSettings });
        console.log('recreated', args[0]); break;
      }
      case 'shot': {
        const [source, file, w, h] = args;
        const req = { sourceName: source, imageFormat: 'png' };
        if (w) req.imageWidth = Number(w);
        if (h) req.imageHeight = Number(h);
        const { imageData } = await obs.call('GetSourceScreenshot', req);
        writeFileSync(file, Buffer.from(imageData.split(',')[1], 'base64'));
        console.log('wrote', file); break;
      }
      case 'call': console.log(JSON.stringify(await obs.call(args[0], args[1] ? JSON.parse(args[1]) : {}), null, 1)); break;
      default: throw new Error('unknown command ' + cmd);
    }
  } finally { obs.close(); }
}
main().catch(e => { console.error(e.message); process.exit(1); });
