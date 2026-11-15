//! The triage page and its JSON (docs/plans/active/feedback-reports.md,
//! 3.5 and 3.6), on the asset service's plain listener only: the server
//! (`server`) answers `/feedback/admin/...` only to a connection from this
//! machine that didn't come through the TLS front, with a `Host` of
//! `localhost`, `127.0.0.1` or `[::1]` (another machine reaches it over
//! SSH: `ssh -L 8731:127.0.0.1:8731 <host>`); to everyone else it doesn't
//! exist (404).
//!
//! - `GET /feedback/admin/`: the page (`#<id>` opens that report) (inline CSS and JS, no framework):
//!   a list with filters by status and kind, a detail view with the text,
//!   the screenshot, the text entries folded, the history, and buttons
//!   Accept, Reject (reason), Needs info, Duplicate (group), Fixed
//!   (commit), Triaged.
//! - `GET /feedback/admin/list`: every report's `meta.toml` as JSON.
//! - `GET /feedback/admin/crashes`: crash reports grouped by signature
//!   (`Inbox::crash_groups`), the biggest group first. The page filters
//!   by kind (bug, feedback, crash) and by a crash's signature, opens a
//!   crash's `backtrace.txt` and offers its `minidump.dmp` as a download.
//! - `GET /feedback/admin/<id>`: one report (meta, text, entries, history).
//! - `GET /feedback/admin/<id>/<entry>`: an entry (allow-listed names,
//!   `meta.toml`, `history.log`), as text or the image it is.
//! - `POST /feedback/admin/<id>/status`: JSON `{status, group, reason,
//!   fixed_in}`; needs the header `X-Chimera-Admin: 1` (a page elsewhere
//!   can't send it without a preflight we never answer).
//!
//! Report text is untrusted: the page puts it in with `textContent` only,
//! and every answer says `nosniff` and a strict content security policy.

use serde::Deserialize;

use super::{
    bundle,
    inbox::{Change, Inbox, Status},
};

/// An answer: status, content type, body.
pub struct Reply {
    pub status: u16,
    pub content_type: &'static str,
    pub body: Vec<u8>,
}

fn reply(status: u16, content_type: &'static str, body: impl Into<Vec<u8>>) -> Reply {
    Reply { status, content_type, body: body.into() }
}

fn json(value: &serde_json::Value) -> Reply {
    reply(200, "application/json", serde_json::to_vec(value).unwrap_or_default())
}

#[derive(Deserialize)]
struct StatusBody {
    status: Option<String>,
    group: Option<String>,
    reason: Option<String>,
    fixed_in: Option<String>,
}

/// Answer an admin request: `parts` are the path's parts after
/// `feedback/admin`; `admin_header` whether `X-Chimera-Admin: 1` was sent.
pub fn handle(inbox: &Inbox, post: bool, parts: &[&str], body: &[u8], admin_header: bool) -> Reply {
    match (post, parts) {
        (false, [] | [""]) => reply(200, "text/html; charset=utf-8", PAGE),
        (false, ["list"]) => json(&serde_json::to_value(inbox.list()).unwrap_or_default()),
        (false, ["crashes"]) => json(&serde_json::Value::Array(
            inbox
                .crash_groups()
                .into_iter()
                .map(|(key, signature, reports)| {
                    serde_json::json!({
                        "key": key,
                        "signature": signature,
                        "count": reports.len(),
                        "latest": reports[0].received_utc,
                        "ids": reports.iter().map(|m| m.id.clone()).collect::<Vec<_>>(),
                    })
                })
                .collect(),
        )),
        (false, [id]) => {
            let (Some(dir), Some(meta)) = (inbox.find(id), inbox.meta(id)) else {
                return reply(404, "text/plain; charset=utf-8", "no such report\n");
            };
            let read = |name: &str| std::fs::read_to_string(dir.join(name)).unwrap_or_default();
            let info: serde_json::Value = serde_json::from_str(&read("report.json")).unwrap_or_default();
            let mut entries: Vec<String> = bundle::ENTRIES
                .iter()
                .map(|(n, _)| n.to_string())
                .filter(|n| n != "report.json" && dir.join(n).is_file())
                .collect();
            entries.sort();
            json(&serde_json::json!({
                "meta": meta,
                "text": info.get("text").cloned().unwrap_or_default(),
                "entries": entries,
                "history": inbox.history(id),
            }))
        }
        (false, [id, entry]) => {
            let allowed = bundle::cap(entry).is_some() || matches!(*entry, "meta.toml" | "history.log");
            let Some(dir) = inbox.find(id).filter(|_| allowed) else {
                return reply(404, "text/plain; charset=utf-8", "no such entry\n");
            };
            let content_type = match *entry {
                "screenshot.jpg" => "image/jpeg",
                "screenshot.png" => "image/png",
                "report.json" => "application/json",
                "minidump.dmp" => "application/octet-stream",
                _ => "text/plain; charset=utf-8",
            };
            match std::fs::read(dir.join(entry)) {
                Ok(b) => reply(200, content_type, b),
                Err(_) => reply(404, "text/plain; charset=utf-8", "no such entry\n"),
            }
        }
        (true, [id, "status"]) => {
            if !admin_header {
                return reply(403, "text/plain; charset=utf-8", "X-Chimera-Admin: 1 is needed\n");
            }
            let Ok(b) = serde_json::from_slice::<StatusBody>(body) else {
                return reply(400, "text/plain; charset=utf-8", "a JSON body {status, group, reason, fixed_in}\n");
            };
            let status = match b.status.as_deref().map(Status::parse) {
                Some(None) => return reply(400, "text/plain; charset=utf-8", "unknown status\n"),
                Some(s) => s,
                None => None,
            };
            let change = Change { status, group: b.group, reason: b.reason, fixed_in: b.fixed_in };
            match inbox.set_status(id, "page", &change) {
                Ok(meta) => json(&serde_json::to_value(meta).unwrap_or_default()),
                Err(e) => reply(400, "text/plain; charset=utf-8", format!("{e}\n")),
            }
        }
        _ => reply(404, "text/plain; charset=utf-8", "no such endpoint\n"),
    }
}

/// The page.
const PAGE: &str = r##"<!doctype html>
<html lang="en"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1">
<title>Feedback reports</title>
<style>
:root{--bg:#f6f6f4;--fg:#1b1b1b;--mut:#666;--line:#ddd;--card:#fff;--acc:#2a6df4;--new:#d97706}
@media (prefers-color-scheme:dark){:root{--bg:#141517;--fg:#e8e8e8;--mut:#9a9a9a;--line:#2c2e33;--card:#1c1d21;--acc:#6d9bff;--new:#f59e0b}}
*{box-sizing:border-box}body{margin:0;background:var(--bg);color:var(--fg);font:14px/1.45 system-ui,sans-serif}
header{display:flex;gap:12px;align-items:center;padding:10px 16px;border-bottom:1px solid var(--line);flex-wrap:wrap}
h1{font-size:16px;margin:0 12px 0 0}select,button,input{font:inherit;color:inherit;background:var(--card);border:1px solid var(--line);border-radius:6px;padding:4px 8px}
button{cursor:pointer}button:hover{border-color:var(--acc)}
main{display:grid;grid-template-columns:minmax(280px,420px) 1fr;min-height:calc(100vh - 50px)}
@media (max-width:800px){main{grid-template-columns:1fr}}
#list{border-right:1px solid var(--line);overflow:auto;max-height:calc(100vh - 50px)}
.row{padding:8px 12px;border-bottom:1px solid var(--line);cursor:pointer}.row:hover,.row.sel{background:var(--card)}
.row .top{display:flex;gap:8px;color:var(--mut);font-size:12px}.row .sum{margin-top:2px;overflow:hidden;text-overflow:ellipsis;white-space:nowrap}
.tag{border:1px solid var(--line);border-radius:10px;padding:0 6px;font-size:11px}.tag.new{color:var(--new);border-color:var(--new)}
#detail{padding:16px;overflow:auto;max-height:calc(100vh - 50px)}#detail img{max-width:100%;border:1px solid var(--line);border-radius:6px}
pre{white-space:pre-wrap;word-break:break-word;background:var(--card);border:1px solid var(--line);border-radius:6px;padding:10px;margin:6px 0}
.actions{display:flex;gap:6px;flex-wrap:wrap;margin:10px 0}.kv{color:var(--mut);font-size:12px}
details{margin:6px 0}summary{cursor:pointer;color:var(--mut)}
</style></head><body>
<header><h1>Feedback reports</h1>
<label>Status <select id="fs"><option value="">all</option></select></label>
<label>Kind <select id="fk"><option value="">all</option><option>bug</option><option>feedback</option><option>crash</option></select></label>
<label>Crash <select id="fg"><option value="">any signature</option></select></label>
<button id="reload">Reload</button><span id="count" class="kv"></span></header>
<main><div id="list"></div><div id="detail"><p class="kv">Pick a report.</p></div></main>
<script>
const STATUSES=["new","triaged","accepted","rejected","needs-info","duplicate","fixed"];
const $=s=>document.querySelector(s);
function el(tag,text,cls){const e=document.createElement(tag);if(text!=null)e.textContent=text;if(cls)e.className=cls;return e}
for(const s of STATUSES){$("#fs").append(el("option",s))}
let all=[],current=null;
async function load(){const r=await fetch("list");all=await r.json();groups();render()}
function groups(){const g=$("#fg"),keep=g.value,n={};for(const m of all){if(m.signature_key){n[m.signature_key]=n[m.signature_key]||{c:0,s:m.signature};n[m.signature_key].c++}}
 g.replaceChildren(el("option","any signature"));g.firstChild.value="";for(const [k,v] of Object.entries(n).sort((a,b)=>b[1].c-a[1].c)){const o=el("option",v.c+"x "+v.s.slice(0,90));o.value=k;g.append(o)}g.value=keep}
function render(){const s=$("#fs").value,k=$("#fk").value,g=$("#fg").value;const list=$("#list");list.replaceChildren();
 const shown=all.filter(m=>(!s||m.status===s)&&(!k||m.kind===k)&&(!g||m.signature_key===g));$("#count").textContent=shown.length+" of "+all.length;
 for(const m of shown){const row=el("div",null,"row"+(m.id===current?" sel":""));const top=el("div",null,"top");
  top.append(el("span",m.received_utc),el("span",m.account.split("#")[0]),el("span",m.kind),el("span",m.map),el("span",m.status,"tag"+(m.status==="new"?" new":"")));
  if(m.group)top.append(el("span","group "+m.group,"tag"));if(m.signature_key)top.append(el("span","sig "+m.signature_key,"tag"));
  row.append(top,el("div",m.summary||"(no text)","sum"));row.onclick=()=>show(m.id);list.append(row)}}
async function show(id){current=id;history.replaceState(null,"","#"+id);render();const r=await fetch(encodeURIComponent(id));if(!r.ok)return;const d=await r.json();const m=d.meta;
 const box=$("#detail");box.replaceChildren();
 box.append(el("div",m.received_utc+" · "+m.account+" · "+m.kind+" · "+m.app+" "+m.build+" · map "+(m.map||"?"),"kv"));
 box.append(el("div","status "+m.status+(m.group?" · group "+m.group:"")+(m.reason?" · reason "+m.reason:"")+(m.fixed_in?" · fixed in "+m.fixed_in:"")+" · id "+m.id,"kv"));
 const acts=el("div",null,"actions");
 const act=(label,status,ask)=>{const b=el("button",label);b.onclick=async()=>{const body={status};
  if(ask){const v=prompt(ask);if(v===null)return;body[ask.split(" ")[0]==="Group"?"group":ask.startsWith("Commit")?"fixed_in":"reason"]=v}
  const r=await fetch(encodeURIComponent(id)+"/status",{method:"POST",headers:{"Content-Type":"application/json","X-Chimera-Admin":"1"},body:JSON.stringify(body)});
  if(!r.ok){alert(await r.text());return}await load();show(id)};acts.append(b)};
 act("Triaged","triaged");act("Accept","accepted");act("Reject","rejected","Reason (troll, not a bug, ...)");act("Needs info","needs-info");
 act("Duplicate","duplicate","Group id (e.g. another report's id)");act("Fixed","fixed","Commit it was fixed in");
 if(m.signature)box.append(el("div","crash signature "+m.signature_key+": "+m.signature,"kv"));
 box.append(acts,el("pre",d.text||"(no text)"));
 for(const e of d.entries){if(e.startsWith("screenshot")){const img=el("img");img.src=encodeURIComponent(id)+"/"+e;img.alt="screenshot";box.append(img)}}
 for(const e of d.entries){if(e.endsWith(".dmp")){const a=el("a","Download "+e);a.href=encodeURIComponent(id)+"/"+e;a.download=id+"-"+e;box.append(el("div",null,"kv"),a)}}
 for(const e of d.entries){if(e.startsWith("screenshot")||e.endsWith(".dmp"))continue;const det=el("details");det.append(el("summary",e));const pre=el("pre","...");det.append(pre);
  det.ontoggle=async()=>{if(det.open&&pre.textContent==="..."){pre.textContent=await (await fetch(encodeURIComponent(id)+"/"+e)).text()}};box.append(det);if(e==="backtrace.txt")det.open=true}
 const h=el("details");h.append(el("summary","history"),el("pre",d.history));box.append(h)}
$("#fs").onchange=render;$("#fk").onchange=render;$("#fg").onchange=render;$("#reload").onclick=load;load().then(()=>{const h=location.hash.slice(1);if(h)show(h)});
</script></body></html>
"##;
