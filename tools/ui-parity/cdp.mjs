// A headless Chromium driven over the DevTools protocol, with no npm
// packages (Node 22+ has fetch and WebSocket): what mockup.mjs and
// compare.mjs render with. The browser is `CHIMERA_CHROME`, else the
// first of chromium, chromium-browser, google-chrome on PATH (Linux) or
// Chrome's and Edge's usual install paths (Windows).
//
// Also a static file server rooted at the repo, so the reference pages'
// fonts (apps/ultimate_gun_game/fonts) load over http (Chromium refuses
// fonts from file:// pages).

import { spawn } from 'node:child_process';
import { createServer } from 'node:http';
import { existsSync, mkdtempSync, readFileSync, rmSync, statSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { extname, join, normalize, resolve } from 'node:path';

const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

function findChrome() {
  if (process.env.CHIMERA_CHROME) return process.env.CHIMERA_CHROME;
  const names = process.platform === 'win32'
    ? [
        `${process.env['ProgramFiles']}\\Google\\Chrome\\Application\\chrome.exe`,
        `${process.env['ProgramFiles(x86)']}\\Google\\Chrome\\Application\\chrome.exe`,
        `${process.env['ProgramFiles(x86)']}\\Microsoft\\Edge\\Application\\msedge.exe`,
      ]
    : ['chromium', 'chromium-browser', 'google-chrome', 'google-chrome-stable'];
  for (const n of names) {
    if (n.includes('\\') || n.includes('/')) {
      if (existsSync(n)) return n;
      continue;
    }
    for (const dir of (process.env.PATH || '').split(':')) {
      if (dir && existsSync(join(dir, n))) return join(dir, n);
    }
  }
  throw new Error('no Chromium found: set CHIMERA_CHROME to its executable');
}

const TYPES = { '.html': 'text/html', '.css': 'text/css', '.js': 'text/javascript', '.json': 'application/json',
  '.ttf': 'font/ttf', '.woff2': 'font/woff2', '.png': 'image/png', '.svg': 'image/svg+xml' };

/** Serves `root` on a free loopback port; `{ url, close }`. */
export async function serve(root) {
  root = resolve(root);
  const server = createServer((req, res) => {
    const path = normalize(join(root, decodeURIComponent(new URL(req.url, 'http://x').pathname)));
    if (!path.startsWith(root) || !existsSync(path) || !statSync(path).isFile()) {
      res.writeHead(404).end();
      return;
    }
    res.writeHead(200, { 'content-type': TYPES[extname(path)] || 'application/octet-stream' });
    res.end(readFileSync(path));
  });
  await new Promise((r) => server.listen(0, '127.0.0.1', r));
  return { url: `http://127.0.0.1:${server.address().port}`, close: () => server.close() };
}

/** Starts a headless browser with one page at `width` x `height` CSS px, DPR 1. */
export async function browser({ width = 1920, height = 1080, reducedMotion = true } = {}) {
  const profile = mkdtempSync(join(tmpdir(), 'ui-parity-'));
  const ch = spawn(findChrome(), ['--headless=new', '--hide-scrollbars', '--remote-debugging-port=0',
    '--font-render-hinting=none', '--force-color-profile=srgb', `--user-data-dir=${profile}`, 'about:blank'],
  { stdio: ['ignore', 'ignore', 'pipe'] });
  let port = null;
  let errText = '';
  ch.stderr.on('data', (d) => {
    errText += d;
    const m = /DevTools listening on ws:\/\/[^:]+:(\d+)\//.exec(errText);
    if (m) port = +m[1];
  });
  for (let i = 0; i < 100 && !port; i++) await sleep(100);
  if (!port) throw new Error('Chromium did not start:\n' + errText.slice(0, 2000));
  let tabs;
  for (let i = 0; i < 50; i++) {
    try { tabs = await (await fetch(`http://127.0.0.1:${port}/json`)).json(); break; } catch { await sleep(100); }
  }
  const ws = new WebSocket(tabs.find((t) => t.type === 'page').webSocketDebuggerUrl);
  await new Promise((r, j) => { ws.onopen = r; ws.onerror = j; });
  let id = 0;
  const pending = {};
  const logs = [];
  ws.onmessage = (m) => {
    const d = JSON.parse(m.data);
    if (d.id && pending[d.id]) { pending[d.id](d); delete pending[d.id]; }
    if (d.method === 'Runtime.exceptionThrown') logs.push('exception: ' + JSON.stringify(d.params.exceptionDetails).slice(0, 800));
    if (d.method === 'Runtime.consoleAPICalled') logs.push('console: ' + d.params.args.map((a) => a.value).join(' '));
  };
  const send = (method, params = {}) => new Promise((r, j) => {
    const i = ++id;
    pending[i] = (d) => (d.error ? j(new Error(`${method}: ${d.error.message}`)) : r(d.result));
    ws.send(JSON.stringify({ id: i, method, params }));
  });
  await send('Runtime.enable');
  await send('Page.enable');
  await send('Emulation.setDeviceMetricsOverride', { width, height, deviceScaleFactor: 1, mobile: false });
  if (reducedMotion) await send('Emulation.setEmulatedMedia', { features: [{ name: 'prefers-reduced-motion', value: 'reduce' }] });
  const page = {
    logs,
    send,
    async goto(url, waitMs = 300) {
      const loaded = new Promise((r) => {
        const prev = ws.onmessage;
        ws.onmessage = (m) => { prev(m); if (JSON.parse(m.data).method === 'Page.loadEventFired') { ws.onmessage = prev; r(); } };
      });
      await send('Page.navigate', { url });
      await loaded;
      await sleep(waitMs);
    },
    async eval(expression) {
      const r = await send('Runtime.evaluate', { expression, returnByValue: true, awaitPromise: true });
      if (r.exceptionDetails) throw new Error('page: ' + JSON.stringify(r.exceptionDetails).slice(0, 1500));
      return r.result.value;
    },
    async png() {
      const r = await send('Page.captureScreenshot', { format: 'png', clip: { x: 0, y: 0, width, height, scale: 1 } });
      return Buffer.from(r.data, 'base64');
    },
    async close() {
      try { ws.close(); } catch { /* gone */ }
      ch.kill();
      await sleep(200);
      try { rmSync(profile, { recursive: true, force: true }); } catch { /* still held */ }
    },
  };
  return page;
}
