//! The owner's admin page (docs/plans/active/hub.md, 7.2): plain HTML and
//! a little JavaScript served by the accounts service at
//! `http://127.0.0.1:8741/admin/`, only to a connection from this machine
//! that no front relayed, with a `Host` of `localhost`, `127.0.0.1` or
//! `[::1]` (another machine reaches it over SSH: `ssh -L
//! 8741:127.0.0.1:8741 <host>`); to everyone else it doesn't exist (404).
//!
//! The page holds nothing secret: it calls the same admin API as the
//! command line (`/admin/...`, `accounts::http`), which wants the
//! services' secret in `X-Chimera-Service`. `chimera_accounts page` prints
//! the page's address with the secret in its fragment (`#key=...`): a
//! fragment never leaves the browser, the page keeps it in the tab's
//! session storage and drops it from the address bar. A page elsewhere
//! can't send that header without a preflight the service never answers.
//!
//! - **Invites**: make one (name, role, content, games, expiry, a
//!   re-invite `--for` an account with `keep`), the link or code shown
//!   once with a Copy button; the list with each one's state; Revoke.
//! - **Accounts**: username, display name, role, rights, devices (Revoke
//!   each), last seen; Set (role, content, games, list slots, note, mute,
//!   display name, username), Refresh, Re-invite, Ban, Revoke, Delete.
//!
//! Text from the database (names, labels, notes) goes in with
//! `textContent` only; every answer says `nosniff` and a strict content
//! security policy.

/// The page.
pub const PAGE: &str = r#"<!doctype html>
<html lang="en"><head><meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>Hub accounts</title>
<style>
:root { --bg:#f6f7f9; --fg:#16181d; --muted:#5d6470; --line:#d9dde4; --card:#fff; --accent:#2457d6; --bad:#b3261e; }
@media (prefers-color-scheme: dark) { :root { --bg:#121418; --fg:#e7e9ee; --muted:#9aa2af; --line:#2c313a; --card:#1a1d23; --accent:#7ea2ff; --bad:#ff8a80; } }
* { box-sizing:border-box; }
body { margin:0; background:var(--bg); color:var(--fg); font:14px/1.45 system-ui,sans-serif; }
main { max-width:1100px; margin:0 auto; padding:16px; }
h1 { font-size:20px; margin:4px 0 12px; } h2 { font-size:16px; margin:20px 0 8px; }
section { background:var(--card); border:1px solid var(--line); border-radius:8px; padding:12px 14px; margin-bottom:14px; }
table { width:100%; border-collapse:collapse; } td, th { text-align:left; padding:5px 6px; border-bottom:1px solid var(--line); vertical-align:top; }
th { color:var(--muted); font-weight:600; font-size:12px; }
input, select { font:inherit; padding:4px 6px; border:1px solid var(--line); border-radius:5px; background:var(--bg); color:var(--fg); }
button { font:inherit; padding:4px 10px; border:1px solid var(--line); border-radius:5px; background:var(--card); color:var(--fg); cursor:pointer; margin:1px; }
button.go { background:var(--accent); color:#fff; border-color:var(--accent); } button.bad { color:var(--bad); }
.muted { color:var(--muted); } .row { display:flex; flex-wrap:wrap; gap:8px; align-items:center; }
.once { font-family:ui-monospace,monospace; padding:8px; border:1px dashed var(--accent); border-radius:6px; word-break:break-all; margin-top:8px; }
#msg { min-height:20px; } #msg.err { color:var(--bad); }
.dev { font-size:12px; } code { font-size:12px; }
</style></head>
<body><main>
<h1>Hub accounts</h1>
<div id="msg" class="muted"></div>
<section id="nokey" hidden>
  <p>This page needs the services' secret. Open it with <code>chimera_accounts page</code>, which prints its address with the key, or paste the key:</p>
  <div class="row"><input id="keyin" size="70" type="password" autocomplete="off"><button class="go" id="keygo">Use</button></div>
</section>
<div id="app" hidden>
<section>
  <h2>New invite</h2>
  <div class="row">
    <input id="iname" placeholder="name (suggested username)" size="22">
    <select id="irole"><option>tester</option><option>host</option><option>owner</option></select>
    <input id="icontent" placeholder="content: * or a,b (role's default)" size="26">
    <input id="igames" placeholder="games: * or a,b" size="16">
    <input id="iexpires" placeholder="days (14)" size="8">
    <input id="ifor" placeholder="re-invite into username" size="20">
    <label><input type="checkbox" id="ikeep"> keep its devices</label>
    <button class="go" id="igo">Invite</button>
  </div>
  <div id="ionce"></div>
</section>
<section>
  <h2>Invites</h2>
  <table><thead><tr><th>Name</th><th>Role</th><th>State</th><th>Made</th><th>Until</th><th>Into</th><th></th></tr></thead><tbody id="invites"></tbody></table>
</section>
<section>
  <h2>Accounts</h2>
  <table><thead><tr><th>Username</th><th>Display name</th><th>Role</th><th>Rights</th><th>Devices</th><th>Last seen</th><th></th></tr></thead><tbody id="accounts"></tbody></table>
</section>
</div>
</main>
<script>
"use strict";
const $ = (id) => document.getElementById(id);
let key = "";
try { key = sessionStorage.getItem("chimera-admin-key") || ""; } catch (e) {}
const frag = new URLSearchParams(location.hash.slice(1));
if (frag.get("key")) { key = frag.get("key"); try { sessionStorage.setItem("chimera-admin-key", key); } catch (e) {} history.replaceState(null, "", location.pathname); }
function say(text, bad) { const m = $("msg"); m.textContent = text; m.className = bad ? "err" : "muted"; }
function when(t) { if (!t) return "never"; return new Date(t * 1000).toISOString().replace("T", " ").slice(0, 16) + " UTC"; }
function el(tag, text, cls) { const e = document.createElement(tag); if (text !== undefined) e.textContent = text; if (cls) e.className = cls; return e; }
function button(text, fn, cls) { const b = el("button", text, cls); b.addEventListener("click", fn); return b; }
async function api(method, path, body) {
  const r = await fetch(path, { method, headers: Object.assign({ "X-Chimera-Service": key }, body ? { "Content-Type": "application/json" } : {}), body: body ? JSON.stringify(body) : undefined });
  const text = await r.text();
  if (!r.ok) throw new Error(r.status + " " + text.trim());
  return text ? JSON.parse(text) : null;
}
async function act(what, fn) { try { await fn(); say(what); await load(); } catch (e) { say(what + ": " + e.message, true); } }
async function load() {
  const [invites, accounts] = await Promise.all([api("GET", "/admin/invites"), api("GET", "/admin/accounts")]);
  const ib = $("invites"); ib.replaceChildren();
  for (const i of invites.slice().reverse()) {
    const tr = el("tr");
    for (const v of [i.name, i.role, i.state, when(i.created), when(i.expires), i.for_username || ""]) tr.append(el("td", v));
    const td = el("td");
    if (i.state === "unused") td.append(button("Revoke", () => act("revoked " + i.name, () => api("POST", "/admin/invite/revoke", { name: i.name })), "bad"));
    tr.append(td); ib.append(tr);
  }
  const ab = $("accounts"); ab.replaceChildren();
  for (const a of accounts) {
    const tr = el("tr");
    tr.append(el("td", a.username + (a.state !== "active" ? " (" + a.state + ")" : "")));
    tr.append(el("td", a.display_name)); tr.append(el("td", a.role));
    tr.append(el("td", "content " + (a.content || "none") + ", games " + a.games + ", list " + a.list_slots + (a.report ? ", reports" : "") + (a.muted ? ", muted" : "") + (a.note ? " / " + a.note : "")));
    const dv = el("td");
    for (const d of a.devices) {
      const line = el("div", "", "dev");
      line.append(el("span", d.label + " (" + d.kind + ", " + (d.revoked ? "revoked" : "seen " + when(d.last_seen)) + ") "));
      if (!d.revoked) line.append(button("Revoke", () => act("device revoked", () => api("POST", "/admin/device/revoke", { device: d.id })), "bad"));
      dv.append(line);
    }
    tr.append(dv); tr.append(el("td", when(a.last_seen)));
    const td = el("td");
    td.append(button("Set", () => setAccount(a)));
    td.append(button("Refresh", () => act("refreshed " + a.username, () => api("POST", "/admin/account/refresh", { username: a.username }))));
    td.append(button("Re-invite", () => { $("iname").value = a.username; $("ifor").value = a.username; $("irole").value = a.role; window.scrollTo(0, 0); say("Re-invite: press Invite to make the link into " + a.username); }));
    td.append(button("Ban", () => { const reason = prompt("Ban " + a.username + ": reason"); if (!reason) return; const days = prompt("For how many days? (empty: for good)"); act("banned " + a.username, () => api("POST", "/admin/ban", { target: a.username, reason, days: days ? Number(days) : null })); }, "bad"));
    if (a.state === "active") td.append(button("Revoke", () => { if (confirm("Revoke " + a.username + " and all its devices?")) act("revoked " + a.username, () => api("POST", "/admin/account/revoke", { username: a.username })); }, "bad"));
    td.append(button("Delete", () => { if (prompt("Type the username to delete " + a.username + " for good") === a.username) act("deleted " + a.username, () => api("POST", "/admin/account/delete", { username: a.username })); }, "bad"));
    tr.append(td); ab.append(tr);
  }
}
function setAccount(a) {
  const ask = (what, now) => { const v = prompt(what + " (now: " + now + "; empty: unchanged)"); return v === null || v === "" ? undefined : v; };
  const change = { username: a.username };
  change.role = ask("Role: tester, host or owner", a.role);
  change.content = ask("Content: * or a,b", a.content || "none");
  change.games = ask("Games: * or a,b", a.games);
  const list = ask("List slots", a.list_slots); if (list !== undefined) change.list = Number(list);
  change.note = ask("Note", a.note);
  change.display_name = ask("Display name", a.display_name);
  change.new_username = ask("Username", a.username);
  const mute = ask("Muted: yes or no", a.muted ? "yes" : "no"); if (mute !== undefined) change.muted = mute.startsWith("y");
  act("set " + a.username + " (Refresh applies it at once)", () => api("POST", "/admin/account/set", change));
}
$("igo").addEventListener("click", () => {
  const spec = { name: $("iname").value.trim(), role: $("irole").value, keep: $("ikeep").checked };
  if ($("icontent").value.trim()) spec.content = $("icontent").value.trim();
  if ($("igames").value.trim()) spec.games = $("igames").value.trim();
  if ($("iexpires").value.trim()) spec.expires_days = Number($("iexpires").value.trim());
  if ($("ifor").value.trim()) spec.for_username = $("ifor").value.trim();
  act("invited " + spec.name, async () => {
    const r = await api("POST", "/admin/invite", spec);
    const box = $("ionce"); box.replaceChildren();
    const text = r.link || r.code;
    const once = el("div", text, "once"); box.append(once);
    box.append(el("div", "Shown once: send it privately. Valid until " + when(r.expires) + (r.link ? "" : " (a code: the service has no public address, chimera_accounts serve --public)"), "muted"));
    box.append(button("Copy", () => navigator.clipboard.writeText(text)));
    for (const id of ["iname", "icontent", "igames", "iexpires", "ifor"]) $(id).value = "";
    $("ikeep").checked = false;
  });
});
$("keygo").addEventListener("click", () => { key = $("keyin").value.trim(); try { sessionStorage.setItem("chimera-admin-key", key); } catch (e) {} start(); });
async function start() {
  if (!key) { $("nokey").hidden = false; return; }
  try { await load(); $("nokey").hidden = true; $("app").hidden = false; say(""); }
  catch (e) { $("nokey").hidden = false; say(e.message, true); }
}
start();
</script>
</body></html>
"#;
