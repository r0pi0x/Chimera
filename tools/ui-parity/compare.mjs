// Compares the look book's layout with the game's (docs/OBSERVABILITY.md,
// "UI parity against the look book").
//
//   node tools/ui-parity/compare.mjs <mockup-prefix> <game-prefix> <out-prefix>
//        [--tol-pos 2] [--tol-size 1] [--tol-font 0.5]
//
// Reads <prefix>.json and <prefix>.png from mockup.mjs and the game's
// `ui_dump` (+ its screenshot), matches elements by id and writes:
// - <out-prefix>.txt: every element ranked by its worst error past
//   tolerance, with dx, dy, dw, dh in px and the font size, letter
//   spacing and colour deltas for texts; then ids only one side has.
//   Boxes compare their edges; texts their left edge, width, vertical
//   centre and font size (a text's height is its line box, which CSS and
//   Bevy measure differently), so dy of a text is its centre's.
// - <out-prefix>.side.png: the mockup and the game side by side.
// - <out-prefix>.overlay.png: the game with the mockup at 50 % over it,
//   each element past tolerance boxed (mockup cyan, game magenta).
// Exits 1 when any element is past tolerance or missing in the game.

import { readFileSync, writeFileSync } from 'node:fs';
import { browser } from './cdp.mjs';

const args = process.argv.slice(2);
const flag = (n, d) => { const i = args.indexOf(n); if (i < 0) return d; const v = args[i + 1]; args.splice(i, 2); return +v; };
const tol = { pos: flag('--tol-pos', 2), size: flag('--tol-size', 1), font: flag('--tol-font', 0.5) };
const [mPre, gPre, out] = args;
if (!out) {
  console.error('usage: node tools/ui-parity/compare.mjs <mockup-prefix> <game-prefix> <out-prefix> [--tol-pos 2] [--tol-size 1] [--tol-font 0.5]');
  process.exit(2);
}
const M = JSON.parse(readFileSync(`${mPre}.json`, 'utf8'));
const G = JSON.parse(readFileSync(`${gPre}.json`, 'utf8'));
// The game's screen may be another size than 1920 x 1080: compare in the
// mockup's px.
const k = G.screen && G.screen.w ? M.screen.w / G.screen.w : 1;
const shown = (e) => e && e.visible !== false;
const r1 = (v) => (Math.abs(v) < 0.05 ? 0 : Math.round(v * 10) / 10);
const fmt = (v) => (v == null ? '' : (v > 0 ? '+' : '') + r1(v).toFixed(1));

const rows = [];
const onlyMock = [];
const onlyGame = [];
for (const [id, m] of Object.entries(M.elements)) {
  if (!shown(m)) continue;
  const g0 = G.elements[id];
  if (!shown(g0)) { onlyMock.push(id); continue; }
  const g = { ...g0, x: g0.x * k, y: g0.y * k, w: g0.w * k, h: g0.h * k };
  const text = m.text && g.text;
  const d = { id, m, g, text: !!text };
  d.dx = g.x - m.x;
  d.dw = g.w - m.w;
  if (text) {
    d.dy = (g.y + g.h / 2) - (m.y + m.h / 2);
    d.dh = null;
    d.dfont = g.text.font_size * k - m.text.font_size;
    d.dls = g.text.letter_spacing * k - m.text.letter_spacing;
    d.dcol = Math.max(...[0, 1, 2].map((i) => Math.abs(g.text.color[i] - m.text.color[i])));
    // the UGG faces are caps only: case never shows
    d.txt = m.text.text.toUpperCase() === g.text.text.toUpperCase() ? '' : `text "${m.text.text}" vs "${g.text.text}"`;
  } else {
    d.dy = g.y - m.y;
    d.dh = g.h - m.h;
  }
  // How far past tolerance, in px (0: within).
  const past = (v, t) => (v == null ? 0 : Math.max(0, Math.abs(v) - t));
  // A text set at a whole-pixel size (the game's `UiUnits::text_px`) runs
  // wider or narrower than the look book's fractional size in proportion:
  // a width that matches the mockup's scaled by the two sizes is expected,
  // not a failure (`~` in the report).
  let dwCheck = d.dw;
  if (text && m.text.font_size > 0 && past(d.dw, tol.size) > 0) {
    const scaled = g.w - m.w * (g.text.font_size * k) / m.text.font_size;
    if (past(scaled, tol.size) === 0 && past(d.dfont, tol.font) === 0) {
      d.fontWidth = true;
      dwCheck = scaled;
    }
  }
  d.err = Math.max(past(d.dx, tol.pos), past(d.dy, tol.pos), past(dwCheck, tol.size), past(d.dh, tol.size), past(d.dfont, tol.font));
  d.size = Math.max(Math.abs(d.dx), Math.abs(d.dy), Math.abs(d.dw), Math.abs(d.dh ?? 0), Math.abs(d.dfont ?? 0));
  rows.push(d);
}
// The game's ids of this screen only (the mockup's `include`).
const mine = (id) => !M.include || M.include.some((r) => new RegExp(r).test(id));
for (const [id, g] of Object.entries(G.elements)) if (mine(id) && shown(g) && !shown(M.elements[id])) onlyGame.push(id);
rows.sort((a, b) => b.err - a.err || b.size - a.size || a.id.localeCompare(b.id));
const over = rows.filter((r) => r.err > 0);

const pad = (s, n) => String(s).padEnd(n);
const lpad = (s, n) => String(s).padStart(n);
const lines = [];
lines.push(`UI parity: ${M.screen_name || mPre} (mockup ${M.page || ''} @ ${M.time ?? 'still'} s) vs game`);
lines.push(`tolerance: position ${tol.pos} px, size ${tol.size} px, font ${tol.font} px; game scaled by ${r1(k)}`);
lines.push(`${rows.length} matched, ${over.length} over tolerance, ${rows.filter((r) => r.fontWidth).length} widths from whole-pixel font sizes (~, expected), ${onlyMock.length} only in the mockup, ${onlyGame.length} only in the game`);
lines.push('');
lines.push(`${pad('id', 30)} ${lpad('dx', 7)} ${lpad('dy', 7)} ${lpad('dw', 7)} ${lpad('dh', 7)} ${lpad('dfont', 6)}  notes`);
for (const r of rows) {
  const notes = [];
  if (r.text) {
    notes.push(`font ${r1(r.m.text.font_size)}/${r1(r.g.text.font_size * k)}`);
    if (Math.abs(r.dls) >= 0.1) notes.push(`spacing ${fmt(r.dls)}`);
    if (r.dcol > 8) notes.push(`colour ${r.m.text.color.slice(0, 3).map(Math.round)} vs ${r.g.text.color.slice(0, 3).map(Math.round)}`);
    if (r.txt) notes.push(r.txt);
    if (r.fontWidth) notes.push('width from whole-pixel font size: expected');
    notes.push('(text: dy is the centre)');
  }
  lines.push(`${r.err > 0 ? '!' : r.fontWidth ? '~' : ' '}${pad(r.id, 29)} ${lpad(fmt(r.dx), 7)} ${lpad(fmt(r.dy), 7)} ${lpad(fmt(r.dw), 7)} ${lpad(r.dh == null ? '' : fmt(r.dh), 7)} ${lpad(r.dfont == null ? '' : fmt(r.dfont), 6)}  ${notes.join('; ')}`);
}
if (onlyMock.length) lines.push('', 'only in the mockup (missing in the game): ' + onlyMock.join(', '));
if (onlyGame.length) lines.push('', 'only in the game: ' + onlyGame.join(', '));
if (M.fixture_mismatches && M.fixture_mismatches.length) {
  lines.push('', 'the mockup page against the fixture: ' + M.fixture_mismatches.map((f) => `${f.id} "${f.page}" vs "${f.fixture}"`).join('; '));
}
writeFileSync(`${out}.txt`, lines.join('\n') + '\n');

// The pictures, drawn on a canvas in the same headless browser.
const data = (p) => 'data:image/png;base64,' + readFileSync(p).toString('base64');
const boxes = over.map((r) => ({ id: r.id, m: [r.m.x, r.m.y, r.m.w, r.m.h], g: [r.g.x, r.g.y, r.g.w, r.g.h] }));
const W = M.screen.w;
const H = M.screen.h;
async function draw(width, height, body) {
  const page = await browser({ width, height, reducedMotion: false });
  try {
    await page.goto('about:blank', 50);
    await page.eval(`(async () => {
      document.body.style.margin = '0';
      const c = document.createElement('canvas'); c.width = ${width}; c.height = ${height}; document.body.appendChild(c);
      const x = c.getContext('2d');
      const img = (src) => new Promise((r, j) => { const i = new Image(); i.onload = () => r(i); i.onerror = j; i.src = src; });
      ${body}
    })()`);
    return await page.png();
  } finally {
    await page.close();
  }
}
const mImg = JSON.stringify(data(`${mPre}.png`));
const gImg = JSON.stringify(data(`${gPre}.png`));
writeFileSync(`${out}.side.png`, await draw(W * 2, H, `
  x.drawImage(await img(${mImg}), 0, 0, ${W}, ${H});
  x.drawImage(await img(${gImg}), ${W}, 0, ${W}, ${H});
  x.font = '600 22px monospace'; x.fillStyle = '#ff0'; x.fillText('MOCKUP', 12, ${H} - 14); x.fillText('GAME', ${W} + 12, ${H} - 14);
  x.fillStyle = '#ff0'; x.fillRect(${W} - 1, 0, 2, ${H});`));
writeFileSync(`${out}.overlay.png`, await draw(W, H, `
  x.drawImage(await img(${gImg}), 0, 0, ${W}, ${H});
  x.globalAlpha = 0.5; x.drawImage(await img(${mImg}), 0, 0, ${W}, ${H}); x.globalAlpha = 1;
  const boxes = ${JSON.stringify(boxes)};
  x.lineWidth = 1; x.font = '11px monospace';
  for (const b of boxes) {
    x.strokeStyle = '#0ff'; x.strokeRect(b.m[0] + .5, b.m[1] + .5, b.m[2] - 1, b.m[3] - 1);
    x.strokeStyle = '#f0f'; x.strokeRect(b.g[0] + .5, b.g[1] + .5, b.g[2] - 1, b.g[3] - 1);
  }
  x.fillStyle = '#f0f';
  for (const b of boxes.slice(0, 40)) x.fillText(b.id.replace(/^board\\.row\\./, 'r'), b.g[0], b.g[1] + b.g[3] + 10);`));

console.log(lines.slice(0, 3).join('\n'));
console.log(`report ${out}.txt; pictures ${out}.side.png, ${out}.overlay.png`);
process.exit(over.length || onlyMock.length ? 1 : 0);
