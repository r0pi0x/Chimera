// The look book side of the UI parity check (docs/OBSERVABILITY.md, "UI
// parity against the look book").
//
//   node tools/ui-parity/mockup.mjs <screen> <out-prefix> [--fixture <json>]
//
// Renders the screen's look book frame (reference/<page>, screens.json)
// at 1920 x 1080 CSS px, DPR 1, with reduced motion on and the frame's
// scene frozen at the screen's time, and writes <out-prefix>.json (every
// data-ui element of the screen: x, y, w, h in px; for a text, its text,
// font size, letter spacing, weight, family and colour) and
// <out-prefix>.png. A text element's box is its text's (a Range over its
// contents) across and its line box up and down, as the game's Text
// node is.
//
// The fixture (default fixtures/hud.json) is the look book's own data;
// each text the page shows for a fixture field is checked against it and
// differences are listed in the JSON's `fixture_mismatches`.

import { mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { browser, serve } from './cdp.mjs';

const here = dirname(fileURLToPath(import.meta.url));
const repo = resolve(here, '..', '..');
const args = process.argv.slice(2);
const flag = (n, d) => { const i = args.indexOf(n); if (i < 0) return d; const v = args[i + 1]; args.splice(i, 2); return v; };
const fixturePath = flag('--fixture', join(here, 'fixtures', 'hud.json'));
const [screen, outPrefix] = args;
const screens = JSON.parse(readFileSync(join(here, 'screens.json'), 'utf8'));
if (!screen || !outPrefix || !screens[screen] || screen.startsWith('_')) {
  console.error(`usage: node tools/ui-parity/mockup.mjs <${Object.keys(screens).filter((k) => !k.startsWith('_')).join('|')}> <out-prefix> [--fixture <json>]`);
  process.exit(2);
}
const cfg = screens[screen];
const fixture = JSON.parse(readFileSync(fixturePath, 'utf8'));
mkdirSync(dirname(resolve(outPrefix)), { recursive: true });

const server = await serve(repo);
const page = await browser({ width: 1920, height: 1080 });
try {
  await page.goto(`${server.url}/tools/ui-parity/reference/${cfg.page}`);
  const result = await page.eval(`(async () => {
    await document.fonts.ready;
    await new Promise(r => setTimeout(r, 250));
    const cfg = ${JSON.stringify(cfg)};
    const frame = document.querySelector('.pf .frame');
    frame.style.position = 'fixed'; frame.style.left = '0'; frame.style.top = '0';
    if (cfg.lead) document.querySelector('.ctl.ldm[data-m="lead"]').click();
    const scene = (window.__ugg.scenes || []).find(s => s.el === frame);
    if (cfg.time != null && scene) { scene.t = cfg.time; scene.update(cfg.time); }
    await new Promise(r => requestAnimationFrame(() => requestAnimationFrame(r)));
    const fr = frame.getBoundingClientRect();
    const inc = cfg.include.map(s => new RegExp(s));
    const out = {};
    const shown = (el) => {
      for (let e = el; e && e !== frame.parentElement; e = e.parentElement) {
        const cs = getComputedStyle(e);
        if (cs.display === 'none' || cs.visibility === 'hidden' || +cs.opacity < 0.01) return false;
      }
      return true;
    };
    let rows = [];
    for (const el of frame.querySelectorAll('[data-ui]')) {
      let id = el.getAttribute('data-ui');
      if (cfg.renumber && id.startsWith(cfg.renumber.from)) {
        const rest = id.slice(cfg.renumber.from.length);
        const n = rest.split('.')[0];
        rows.push({ el, n, rest: rest.slice(n.length) });
        continue;
      }
      if (!inc.some(r => r.test(id))) continue;
      out[id] = measure(el);
    }
    if (cfg.renumber) {
      const tops = {};
      for (const r of rows) if (r.rest === '' && shown(r.el)) tops[r.n] = r.el.getBoundingClientRect().top;
      const order = Object.keys(tops).sort((a, b) => tops[a] - tops[b]);
      for (const r of rows) {
        const k = order.indexOf(r.n);
        if (k < 0) continue;
        const id = cfg.renumber.to + (k + 1) + r.rest;
        if (inc.some(x => x.test(id))) out[id] = measure(r.el);
      }
    }
    function measure(el) {
      const b = el.getBoundingClientRect();
      const o = { x: b.left - fr.left, y: b.top - fr.top, w: b.width, h: b.height, visible: shown(el) };
      // a text: an element whose own text nodes carry the text
      const own = [...el.childNodes].filter(n => n.nodeType === 3 && n.textContent.trim()).length > 0;
      // a padded element (a tag) is a box with a text inside, as in the game
      const csb = getComputedStyle(el);
      const padded = ['paddingLeft', 'paddingRight', 'paddingTop', 'paddingBottom'].some(k => parseFloat(csb[k]) > 0);
      if (own && el.tagName !== 'svg') {
        // The text's own run: its first text node to its last (a mark
        // beside it, like a bot's robot, is another node in the game).
        const texts = [...el.childNodes].filter(n => n.nodeType === 3 && n.textContent.trim());
        const range = document.createRange();
        range.setStart(texts[0], 0);
        range.setEnd(texts[texts.length - 1], texts[texts.length - 1].textContent.length);
        const t = range.getBoundingClientRect();
        const cs = getComputedStyle(el);
        const ls = cs.letterSpacing === 'normal' ? 0 : parseFloat(cs.letterSpacing);
        const c = cs.color.match(/[\\d.]+/g).map(Number);
        const own_text = texts.map(n => n.textContent).join('');
        const shownText = (cs.textTransform === 'uppercase' ? own_text.toUpperCase() : own_text).trim();
        const info = { text: shownText, font_size: parseFloat(cs.fontSize), letter_spacing: ls, weight: +cs.fontWeight,
          family: cs.fontFamily, color: [c[0], c[1], c[2], c.length > 3 ? c[3] : 1] };
        if (padded) { o.inner = info; return o; }
        // Across: the glyph run's; up and down: the element's line box (a
        // Range's height is the font's ascent to descent, not its line); a
        // text beside other nodes (a mark, a hidden label): one font size
        // around the run's middle.
        Object.assign(o, { x: t.left - fr.left, w: t.width, box: { x: b.left - fr.left, y: b.top - fr.top, w: b.width, h: b.height } });
        if (el.children.length) {
          const fs = parseFloat(cs.fontSize);
          Object.assign(o, { y: t.top + t.height / 2 - fs / 2 - fr.top, h: fs });
        }
        o.text = { text: shownText, font_size: parseFloat(cs.fontSize), letter_spacing: ls, weight: +cs.fontWeight,
          family: cs.fontFamily, color: [c[0], c[1], c[2], c.length > 3 ? c[3] : 1] };
      }
      return o;
    }
    return { screen: { w: Math.round(fr.width), h: Math.round(fr.height) }, elements: out };
  })()`);
  // The fixture against what the page shows.
  const mism = [];
  const want = (id, text) => {
    const e = result.elements[id];
    if (e && e.text && e.text.text !== text) mism.push({ id, page: e.text.text, fixture: text });
  };
  // The HUD's fixture (hud.json) against the board's ids.
  const rowsOf = cfg.page !== 'hud.html' ? [] : cfg.lead ? fixture.rows_lead : fixture.rows;
  (rowsOf || []).forEach((r, i) => {
    const p = `board.row.${i + 1}`;
    want(`${p}.name`, r.name.toUpperCase());
    want(`${p}.rung`, String(r.rung).padStart(2, '0'));
    want(`${p}.gun`, r.gun.toUpperCase());
    want(`${p}.game`, r.game.toUpperCase());
    want(`${p}.k`, String(r.kills));
    want(`${p}.d`, String(r.deaths));
  });
  if (cfg.page === 'hud.html') want('board.head.map', (fixture.map || '').toUpperCase());
  (fixture.feed || []).forEach((k, i) => {
    want(`feed.row.${i + 1}.killer`, k.killer.toUpperCase());
    want(`feed.row.${i + 1}.victim`, k.victim.toUpperCase());
  });
  const doc = { source: 'mockup', screen_name: screen, page: cfg.page, time: cfg.time, include: cfg.include, ...result, fixture_mismatches: mism };
  writeFileSync(`${outPrefix}.json`, JSON.stringify(doc, null, 1));
  writeFileSync(`${outPrefix}.png`, await page.png());
  const n = Object.keys(result.elements).length;
  console.log(`mockup ${screen}: ${n} elements -> ${outPrefix}.json, ${outPrefix}.png` + (mism.length ? ` (${mism.length} fixture mismatches)` : ''));
  if (page.logs.some((l) => l.startsWith('exception'))) console.log(page.logs.filter((l) => l.startsWith('exception')).join('\n'));
} finally {
  await page.close();
  server.close();
}
