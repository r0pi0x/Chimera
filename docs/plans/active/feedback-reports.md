# Plan: in-game feedback and bug reports

Status: P1 built (2026-11-14, branch `feedback-p1`): P1a (bundle,
secret filter, outbox, the asset service's `/feedback` route, inbox,
limits, CLI, the triage page) and P1b (F10 and its migration, the Escape
menu's entry, the Signal dialog, the Controls row, the notice) joined:
the game's reports go through P1a's outbox (section 11). Left for P1:
the release (user's go) and the deploy (section 12). User decisions
(2026-11-14): the receiver is the asset service on the Linux box;
testers with their own installs get tokens; the page is this machine's
only; F10 reports in UGG and CS:S's F10 quit bind is dropped there; a
Controls row and an Escape entry; the screenshot before the dialog. The
user's idea: during a
playtest a tester presses a key (or picks an Escape menu entry), a dialog
opens, they type (later: speak) what went wrong, and Send files a bundle
of the game's state into a queue the host and the orchestrating agent
triage. The user decides what gets fixed; the agent recommends.

This plan is the MVP for the next playtest build, then the fuller system.
Section 9 lists the decisions the user still has to make.

## 1. What exists (build on it)

| Need | Have | Where |
|---|---|---|
| A report from inside the game | `bugreport [note]` (F9, CS:S's F4 through `bug`): `screenshot.png` + `report.txt` (build and `NET_VERSION`, map, a `setpos`/`setang` line, weapon, health, the last `chimera_perf` readout, the last 40 console lines) into `<data_local>/chimera/bugreports/report-<secs>/` | `chimera/client/src/client/capture.rs` (`report_text`), `input.rs`, `binds.rs` (`CHIMERA_BINDS`); docs/playtest.md "Reporting a bug" |
| The build's stamp | `core::BuildVersion`, `version` | `chimera/core/build_stamp.rs` |
| Perf and frames | `PerfReport`, `FrameMetrics` (distributions, hitch log) | `client/perf.rs`, `client/frame_metrics.rs` |
| Net stats | `net_graph` readout (ping, loss, choke, rates), `status` | `client/net_graph.rs`, `chimera/net/src/net/mod.rs` |
| Changed cvars | `config_text` (archived cvars, binds); `Cvar::{archive, scope}`; names with "password" are already hidden from logs and rcon echo | `chimera/core/src/console.rs`, `chimera/net/src/net/rcon.rs` |
| GPU | Bevy's `RenderAdapterInfo` (wgpu `AdapterInfo`) | bevy_render 0.20 `renderer/mod.rs` |
| Microphone and Opus | `client::voice`: capture and playback behind traits (cpal), Opus 48 kHz mono 20 ms frames | `chimera/client/src/client/voice.rs`; docs/plans/active/voice-chat.md |
| An HTTPS service every tester already reaches | the asset service: tiny_http behind its own TLS front (`tls::Front`), per-account tokens (`accounts`), per-account and per-address limits (`Limits`, `FrontLimits`), an access log | `chimera/assets/src/server.rs`, `tls.rs`, `accounts.rs` |
| Where to send, already on each tester's machine | `asset-access.toml`: the tester's token, bound to the service a game server offered (URL and certificate fingerprint, pinned) | `chimera/assets/src/access.rs`; the match manifest's `AssetOffer` (`chimera/net/src/net/assets.rs`) |
| A blocking HTTPS client with a pinned certificate | ureq 3 + `tls::PinnedTls` | `chimera/assets/src/tls.rs` |
| Kit controls | `controls::modal`, `text_field`, `segmented`, `toggle`, `button` (key caps), `kv` | `chimera/ui/src/controls.rs` |
| The Escape menu and its data | `party_kit::esc_signal`, `[[escape]]` in `data/main_menu.toml` | `apps/party_kit/` |
| Rebindable keys | Settings > Controls (`party_kit::controls::SHARED`) | `apps/party_kit/src/controls.rs`, `settings.rs` |

Missing: no crash reporter (no panic hook), no recording of our own
matches (`playdemo` plays CS:S demos only), no ring buffer of `tracing`
log lines (the console buffer holds console lines), no zip writer in the
workspace (flate2 is in the lock), no admin account kind on the asset
service.

## 2. The MVP (one playtest build)

### 2.1 What a tester sees

1. **F10** anywhere in a match or menu (or Escape > **Report a bug**).
   The game takes a screenshot of that very frame first, then opens the
   dialog: the screenshot shows the problem, not the dialog.
2. **The dialog** (Signal look, over the match; the match goes on, as
   with the Escape menu):
   - top bar "REPORT"; a segmented **Bug | Feedback** (Tab or click);
   - a text area, focused, "What happened? What did you expect?"
     (2000 characters; Enter is a new line, **Ctrl+Enter** sends);
   - the screenshot's thumbnail with a toggle **Include screenshot**
     (on);
   - **What's sent** (folded `kv` list: build, PC, map, rung, position,
     recent console and network stats) and one line of notice: "Sent to
     this playtest's host only. No passwords, tokens or game files.";
   - **Send** (primary, Ctrl+Enter) and **Cancel** (Esc), and the
     destination: "to <server name>'s report service" or "no report
     service: saved on this PC".
3. After Send a toast: "Report sent" or "Saved, sends when the service
   answers (N waiting)". The console prints the report id and the local
   folder.

Typing in the dialog runs no binds (as chat and the console). The plain
`bugreport` (F9, CS:S's F4) stays as it is: a silent local save, for
developers and the console.

### 2.2 The key: F10 (user, 2026-11-14)

- **Ultimate Gun Game binds F10 to `feedback`**, in the shared binds and
  in every game layer under it (CS:S, Combat Arms, HL2:DM). CS:S's
  default `F10 "quit prompt"` (its built-in table,
  `games/cs_source/src/game/mod.rs`, and the install's
  `cfg/config_default.cfg`) is **dropped, not moved**: quitting is the
  Escape menu's.
- Mechanism: app bind defaults that win over a game's
  (`BindDefaults::app_defaults`, registered by the app, applied after the
  game's defaults and over `CHIMERA_BINDS`), so the CS:S app itself keeps
  CS:S's F10 (parity first).
- Saved configs: testers' `config.cfg` already holds every key (`binds:
  all`), including `F10 "quit prompt"`, and saved binds win. A one-time
  migration (a new mark line in config.cfg, as `DEFAULTS_MARK` does)
  replaces F10 only where it still holds the old default; a tester who
  bound F10 to something else keeps it.
- Rebindable: a **Report a bug** row in Settings > Controls (Shared
  tab, `controls::SHARED`), so it can move like any bind.

### 2.3 The bundle

A zip (Explorer and every OS open it; the `zip` crate, deflate through
flate2, already in the lock) built from an allow-list of generated
entries, never from files on disk:

| Entry | Contents |
|---|---|
| `report.json` | schema version, report id (UUID v4, made by the client: retries are idempotent), category, the tester's text, UTC time, build (`BuildVersion`) and `NET_VERSION`, app, install id (a random id kept in the data folder, for grouping one machine's reports; no hardware serials) |
| `screenshot.jpg` | the frame at the key press, JPEG ~85, at most 1920 wide (about 0.3-0.6 MB; the `image` crate, already in the lock) |
| `game.txt` | the role (offline, listen host, client), characters and bots, tick rate, the ladder's rungs and the games they deal (`client::world_report::setup_text`), the weapon in hand with its draw, view-model sequence and pose, whether that view model is still coming (the stand-in gun showing), the code-built maps' kit's loads (loading now and for how long, each game's view models handed over ahead of it, the gun added ahead, the kit's time: `console::KitLoads`) and the sounds the player makes now (`hand_text`), map, minigame and rung, ladder position, match clock, players and bots, the server's name (not its address), the local player's `setpos`/`setang`, velocity, weapon, health, alive or spectating, the asset sets fetched and this match's on-demand fetches, and the server's last 48 hits (`core::RecentHits`, kept by `apply_damage`: each landed damage with the target's health after it, each death and its killer, and a game's hits that did nothing, Halo's on an immune body; entities by id, never a name; a listen host's or an offline game's only) |
| `system.txt` | OS and version, CPU, RAM, GPU name, driver and backend (`RenderAdapterInfo`), window size, scale factor, display mode, vsync, `fps_max` |
| `perf.txt` | the last `chimera_perf` readout, `FrameMetrics` distributions, the hitch log's last entries and the frame inspector's worst spikes with their breakdowns (`client::frame_inspector`) |
| `net.txt` | ping, loss, choke, in and out rates over the last seconds (`net_graph`'s readout kept in a ring), reconnects this session |
| `cvars.txt` | cvars whose value differs from the default, after the secret filter (2.5); changed binds |
| `console.txt` | the last 300 console lines, redacted (2.5) |
| `world.txt` | a census (entities by kind, archetypes, systems, asset stores), growth by kind since the session's first map and since this map, the archetypes made since the map and by what, what varies on the characters, memory by kind (`chimera_mem`), the last 5 minutes' event timeline (frame events, alike ones folded with counts); the UI: cameras (on/off), every UI root (its components, nodes, z, its camera and whether it is on) and the UI spawned a second over the last 10 s, roots by name (`client::world_report`) |
| `log.txt` | the last 500 `tracing` lines at INFO and above, from a ring-buffer layer (Bevy's `LogPlugin::custom_layer`), redacted, lines alike but for digits folded into the last with `(xN since <time>)` (`bundle::fold_repeats`) |

Typical size under 1 MB; the client refuses to queue more than 8 MB.
No demo yet (nothing records our own matches); see phase 3.

### 2.4 On the tester's machine

Beside `bugreports/` (the data folder that is not roaming, as bulky
files belong there): `<data_local>/chimera/feedback/`, so
`~/.local/share/chimera/feedback/` on Linux and
`%LOCALAPPDATA%\chimera\feedback\` on Windows (`env::user_folder`, the
old `mashup` folder while only it exists).

- `outbox/<id>.zip`: written atomically on Send, before any upload.
- `sent/<id>.zip`: moved there on a 2xx; the last 20 kept.
- A thread (never the main thread) sends the outbox: at once, then with
  backoff (1, 2, 4 ... 15 min) while the game runs, and on the next start.
  `feedback_status` (console) lists what waits and why.

### 2.5 Privacy and secrets

The bundle writer is the only door, and it is a pure function over a
`ReportState` struct, so tests can assert on its bytes.

- Never read: `chimera.local.toml` (or `mashup.local.toml`),
  `asset-access.toml`, `config.cfg` as a file, the asset store, any
  install or vfs file. A bundle holds no game file and no extracted data
  (CLAUDE.md hard rule); the screenshot is a picture of the screen, as
  `jpeg` already saves.
- Cvars: dropped when the name contains `password`, `token`, `key`,
  `secret`, `rcon`, `auth`, `fingerprint` or `asset_url`, and every
  server-scope cvar not replicated (`CvarScope::Server`: "bots,
  secrets").
- Console and log lines: a line naming such a cvar or command
  (`password ...`, `rcon_password ...`, `+chimera:asset_token ...`) has
  its arguments replaced by `(hidden)` (rcon's rule); `connect <addr>`
  keeps the command, drops the address; long hex or base64 runs (tokens,
  keys) become `(hidden)`; the user's home folder in a path becomes `~`
  (`C:\Users\<name>` and `/home/<name>`); IPv4 and IPv6 addresses become
  `(address)`.
- The dialog says what is sent (2.1); a tester can turn off the
  screenshot. Nothing is sent without Send.

### 2.6 Where it goes

**Endpoint**, first that applies:

1. `chimera:feedback_url` (client cvar, archived; empty by default) with
   `chimera:feedback_fingerprint` and `chimera:feedback_token`, for a
   service someone names by hand;
2. `[feedback] url` / `token` / `fingerprint` in the local config
   (developers; the repo ships no address: `chimera.local.example.toml`
   gets commented placeholders only);
3. **the current server's asset service** (the match manifest's
   `AssetOffer`: URL and fingerprint) or the bound one in
   `asset-access.toml`, with the tester's asset token: the playtest case,
   no setup;
4. none: saved locally, the dialog says so and names the folder.

**The upload**: `POST /feedback` over HTTPS with the pinned certificate,
`Authorization: Bearer <token>`, `Content-Type: application/zip`,
`X-Chimera-Report-Id`. Answers: 201 stored, 200 already stored (same
id), 400 bad bundle, 401 token, 413 too large, 429 limit (with
`Retry-After`). 4xx other than 429 is not retried; the zip stays in
`outbox/` marked failed with the reason.

No network protocol change: the offer already carries the URL and
fingerprint, so the game server needs nothing new.

## 3. The receiver

### 3.1 Options

| Option | For | Against |
|---|---|---|
| **A. A `/feedback` route on our asset service** (recommended for the MVP) | Testers already reach it over pinned HTTPS with their own tokens: identity per tester, limits, bans, the TLS front, no new port, no address anywhere; tiny_http, ureq and rustls are already in the crate; a few hundred lines | The asset crate grows a second job (split out later, 3.2); no ready-made web UI (we write one small page) |
| B. Our own receiver binary on another port | Clean separation | Another port to open, another URL to hand out, the token and TLS story again |
| C. Forgejo (or Gitea) issues | Self-hosted single binary with SQLite, a good web UI, labels, attachments, an API agents can use | Testers can't be given write tokens safely; needs a receiver in front anyway (A forwarding to it); a second service to run and back up |
| D. GlitchTip (Sentry-compatible ingest) | Made for crash reports with attachments and grouping | Docker, Postgres, Redis; the Sentry envelope is crash-shaped, not prose plus a screenshot; overkill for a few dozen reports |
| E. Plane | Nice tracker | Heavy (several services); no ingest |
| F. GitHub issues on the public repo | Zero hosting | Public: testers' screenshots, names and trolls in the open; rejected |
| G. Through the game connection | Works wherever the game does | Megabytes on the game channel mid-match, a `NET_VERSION` bump, the game server writing files; rejected for now |

**Recommended:** A for the MVP. Later: keep A as the ingest and, if the
user wants a real tracker, mirror *accepted* reports into a self-hosted
**Forgejo** (C) through its API (one issue per group, the screenshot
attached, labels for status); GlitchTip only if crash volume ever needs
it.

### 3.2 Code layout (layering)

- `chimera_assets::feedback` (no Bevy; the crate is "plain data and the
  asset service"): `bundle` (the schema, the zip writer and reader, the
  allow-list, redaction), `outbox` (the client's queue and its upload
  over `remote`/`tls`), `inbox` (the receiver: validate, store, limits,
  statuses), the route in `server.rs`, and a `feedback` subcommand of
  the `chimera_assets` CLI. If it outgrows the crate it moves to an
  engine crate `chimera_feedback` (a `workspace_crates` row, `cargo
  hakari generate`); not for the MVP.
- `chimera_client::client::feedback` (engine, every app): the `feedback`
  command (snapshot, then open the dialog state), `ReportState`
  collection from the world, the log ring layer, the outbox thread,
  `feedback_send <text>` (console fallback: CS:S, Lucker Party and any
  app without the Signal dialog), `feedback_status`.
- `party_kit::feedback_signal` (feature `present`): the dialog in the
  Signal look, `[layout.feedback]` in the UGG theme; the `[[escape]]`
  entry `action = "feedback"`; the Controls row.
- `apps/ultimate_gun_game`: F10 in its app bind defaults (2.2).

`checks/tests/it/architecture.rs` already allows this: `chimera_client`
uses `chimera_assets`; `party_kit` is an app crate.

### 3.3 Auth, limits, spam

- Auth: the tester's hub account (its ticket; `accounts::TicketAuth`),
  so each report names an account; a tester who plays from their own
  install signs in too. Revoking the account also stops its reports. (An
  asset service with no hub takes its own tokens, `chimera_pack token
  add`.)
- Limits (new `Limits` fields, CLI flags): body 8 MB; per account 10 an
  hour and 40 a day; per address 30 an hour; the existing bad-token ban.
  Past a limit: 429, logged. The hub's owner (the ticket's `owner` role)
  is exempt from the per-account and per-address limits (2026-11-25: the
  owner's own reports were refused); size and validation still apply, the
  role counts as of the ticket, so a demotion bites when it expires. The
  client's notice for a 429 reads "Rate limit reached, retries in N min".
- Validation: a real zip, only allow-listed names (no folders, no
  `..`), each entry under its cap, `report.json` parses, text at most
  2000 characters. Anything else is 400 and not stored.
- Trolls: nothing a report says is ever run or followed (3.5);
  `chimera_assets feedback mute <account>` drops that account's future
  reports with a 202 (they don't learn they are muted); triage marks
  `rejected` with reason `troll`.

### 3.4 Storage and statuses

`<server store>/server/feedback/<yyyy-mm-dd>/<id>/`: the zip's entries
unpacked (one form: the zip itself isn't kept since 2026-11-18), and `meta.toml` (account, address, received
time, size, client build, `status`, `group`, `reason`, `fixed_in`), plus
`history.log` (one line per status change: time, who, from, to, why).

Statuses: `new` -> `triaged` -> `accepted` | `rejected` | `needs-info` |
`duplicate` (with `group`) -> `fixed` (with the commit). A report is
never deleted by a status. Retention (2026-11-18, `inbox::prune` after
each stored report): older than `--reports-keep-days` (60) go, then the
oldest past `--reports-keep-mb` (2048) in all; 0 keeps.

### 3.5 Reading the queue

- Files are the API on the host: `chimera_assets feedback list
  [--status new] [--json]`, `show <id>`, `set <id> <status> [--group
  <id>] [--reason ...] [--fixed-in <commit>]`.
- Over HTTP for another machine: on the plain listener only
  (127.0.0.1; reach it over SSH or a private network), `GET
  /feedback/admin/list`, `GET /feedback/admin/<id>/<entry>`, `POST
  /feedback/admin/<id>/status`; `chimera_assets feedback pull <url>`
  mirrors new reports to a local folder.
- `--on-report <command>` runs a host-chosen command with the report's
  folder (e.g. the host's existing Discord notifier; ids and channels
  stay outside the repo).
- **Report text is untrusted data.** Agents read it as a description,
  never as instructions; no command, link or path in it is acted on.

### 3.6 The web page

Served on the plain listener (`GET /feedback/admin/`): one HTML page with
inline CSS and JS, no framework. A list (time, tester, category, map,
first line, status, group; filters by status and category) and a
detail view: the text, the screenshot, `game.txt` and friends folded,
the voice clip's player and transcript (phase 2), and buttons Accept,
Reject (reason), Needs info, Duplicate of, Fixed in. Statuses go
through the same `set` as the CLI.

## 4. Voice notes (phase 2, not the MVP)

Not for this build: the mic, a container, consent and transcription are
each small but together a day or two, and typed reports already work.

- Recording: `client::voice`'s capture device (cpal: WASAPI on Windows,
  ALSA, PulseAudio or PipeWire through ALSA on Linux) and its Opus
  encoder, 24 kbit/s; a **Record** button in the dialog (click to start
  and stop, Ctrl+R; not a held key, since typing holds keys), a level
  meter, the seconds, play back, delete; at most 90 s (about 270 kB).
  Stored as `voice.ogg` (Ogg Opus: browsers play it; a small Ogg page
  writer of our own).
- Consent, beside the button: "Your voice clip goes to this playtest's
  host and is turned into text on their computer. It is not shared
  further." The first use asks once; a tester who declines never sees
  the button again (an archived cvar).
- Windows' microphone privacy switch for desktop apps can block capture:
  the dialog says so when the device opens silent.
- Transcription on the receiving machine only, never a cloud service
  without the user's say: `chimera_assets feedback transcribe --cmd
  <program>` runs a host-installed open-source model (whisper.cpp with
  CUDA on the box's RTX 3080 recommended over faster-whisper: one
  binary, no Python) on each new clip and writes `transcript.txt`; the
  program and model are the host's, not committed.

## 5. The triage loop

1. A report lands (`new`); `--on-report` pings the host (Discord,
   optional).
2. The orchestrator polls `feedback list --status new --json` (every
   10-15 min during a playtest, or on the ping), reads each report, and
   writes `triage.md` into its folder: a one-line summary, what the
   bundle shows (map, rung, position, perf or net trouble, console
   errors), likely area and crate, severity, a duplicate group, and a
   recommendation (fix now, later, reject, needs info). Status
   `triaged`.
3. **Duplicates:** a signature per report (category, map, the weapon or
   rung, the normalized last error or warning line) groups the obvious
   ones; the orchestrator merges the rest by reading them, one group id
   per issue.
4. A digest goes to the user: groups ranked by severity and count, each
   with the recommendation.
5. **The user decides** (the web page's buttons, or by telling the
   orchestrator). Only `accepted` groups get fix agents, each briefed
   with the report folders; a merged fix sets `fixed --fixed-in
   <commit>`, and the next build's notes list them.
6. Later, once the user trusts it: the orchestrator may act on some
   classes alone (crashes with a clear trace, typos); the user's call.

## 6. Phases

| Phase | What | Size | Tests |
|---|---|---|---|
| **P0** (this playtest, no code) | F9 / F4 `bugreport` as today; testers zip the folder and send it in the group chat (docs/playtest.md "Reporting a bug") | none | none |
| **P1a** MVP, data and receiver (agent 1) | `chimera_assets::feedback`: bundle, redaction, outbox and upload, the `/feedback` route, inbox and statuses, limits, the CLI, the web page | 3-4 h | it tests: a bundle from a `ReportState` full of planted secrets (cvars `rcon_password`, `sv_password`, `chimera:asset_token`, a token-looking string, a home path, an address in console lines) contains none of them and only allow-listed entries; the receiver on port 0: 401, 413, 429, a `..` entry and an unknown entry refused, the same id twice stored once, statuses and history; the outbox retries against a server that fails then answers |
| **P1b** MVP, the game (agent 2) | `client::feedback` (snapshot first, state, log ring, outbox thread, `feedback_send`, `feedback_status`); `party_kit::feedback_signal`; the Escape entry; F10 app default, its config migration and the Controls row | 3-4 h | scenario: the `feedback` command asks for the screenshot before the dialog opens (a fake capture seam headless), the dialog's keys (type, Tab, Ctrl+Enter, Esc, binds silent while typing), the Escape entry closes the menu before the snapshot; binds: a fresh config has F10 `feedback` in UGG and `quit prompt` in the CS:S app, an old config's F10 `quit prompt` migrates once, a custom F10 stays; the Controls row; `ui_scene feedback` screenshots at 720p and 1440p |
| P1 release | A new playtest build for testers and server (all on one `NET_VERSION`); docs/playtest.md and playtest-hosting.md updated | user's go | `tools/post-push-tests.sh`; a pre-flight report from a clean Windows PC |
| **P2** voice | 4: recording, `voice.ogg`, consent, the transcription hook, the page's player | 1-2 days | Ogg pages read back by a decoder; size and time caps; consent off hides the button |
| **P3** depth | a panic hook writing a crash bundle sent on the next start; a rolling buffer of the last 30 s of received snapshots and own input (a mini demo `demosim` can replay); a report id sent to the game server so its log marks the moment; duplicate signatures; mirroring accepted groups to Forgejo; retention | days, separable | each with its own test |
| P3b | crash reporting (playtest 7 request): a panic or native crash writes a bundle (backtrace, log tail, versions, GPU and OS, map and rung; Windows minidumps) sent or queued on the next start; the rolling session log, health lines, unclean-exit detection (section 13) | built 2026-11-14 (branch `crash-diag`) | a forced panic leaves a bundle that the outbox sends; section 13's |
| P4 | the autonomous loop of 5.6 | later | |

Docs to change with P1: docs/playtest.md ("Reporting a bug": F10),
docs/playtest-hosting.md (reading reports, limits, muting),
OBSERVABILITY.md (the bundle), and each new file's `//!` doc
(`docs_index`).

## 7. Risks

- **Release timing:** testers on one build cannot get the dialog without
  a new build, and main's `NET_VERSION` moved on since the last playtest
  tag: the feature reaches testers only with the next release, which
  needs everyone (server too) to update.
- **Screenshot timing:** Bevy captures the frame being rendered; the
  dialog must spawn only after `ScreenshotCaptured`, and from the Escape
  menu only after the menu and its HUD cover are gone (two frames).
- **The match goes on** while a tester types: they may die. Accepted (as
  the Escape menu); the snapshot is from the key press.
- **Redaction misses something:** the allow-list and the planted-secret
  test are the guard; review new entries against 2.5.
- **Abuse:** limits per account and address, mute, revocation.

## 8. Alternatives considered (short)

A browser form instead of the dialog (no game state, extra steps);
reports through the game connection (bandwidth, protocol bump); a hosted
tracker (Sentry, GitHub issues: public or third-party, testers' data
leaves the host); tar instead of zip (testers can't open it on Windows
without a tool); voice in the MVP (more to build and to consent to than
the typed path needs).

## 9. Decisions for the user

1. **Release:** cut a new playtest build with the feedback dialog (all
   testers and the server update), or run this playtest on P0 (F9 plus
   the group chat) and ship the dialog in the build after.
2. **Where the receiver runs:** on the machine that runs the asset
   service (recommended; it is the asset service), this Linux box or the
   Windows PC.
3. **Tokens for everyone:** give testers who play from their own install
   an asset token too, so their reports upload (recommended), or accept
   that they save locally.
4. **Tracker:** our page for now (recommended); Forgejo for accepted
   issues later, or never.
5. **Voice:** phase 2 (recommended), not in the first build.
6. **Web page access:** only on the host (and over SSH or a private
   network; recommended), or exposed with an admin login.
7. **Discord pings** per report (no screenshot in the channel?), or a
   digest only.
8. **Retention:** how long reports are kept (suggested 90 days, then
   deleted).
9. **F9 / F4:** keep them as the silent local save (recommended), or
   make them open the dialog too.
10. **A main-menu entry** "Report" besides the Escape menu's, or not.

## 10. The API P1b builds on (`chimera_assets::feedback`)

No Bevy in it; `chimera_client::client::feedback` wraps it in a
resource. Everything below is `chimera_assets::feedback::*`.

**Start once per process** (a client app):

```
let folder = chimera_core::env::user_folder(&dirs::data_local_dir()?).join("feedback");
let targets = Targets {
    cvar: None,                                   // chimera:feedback_url/_token/_fingerprint, all set
    local: LocalFeedback::load().ok().and_then(|f| f.target()),   // [feedback] in the local config
    offer: None,                                  // (url, fingerprint) of the joined server's AssetOffer
    server_name: None,                            // its name, for "to <name>'s report service"
    access: Some(chimera_assets::access::Access::load()),
    assets: asset_settings.remote.clone(),        // a developer's [assets] service
};
let feedback = Feedback::start(folder, targets, OutboxConfig::default())?;   // spawns the sender thread
```

**On the key press, after the screenshot** (`ReportState` is plain data;
`Default` for what you don't have):

```
let submitted: Submitted = feedback.submit(&ReportState {
    kind: Kind::Bug,                  // or Kind::Feedback
    text,                             // at most MAX_TEXT_CHARS (2000); more is cut
    build, net_version, app,
    install_id: feedback::install_id(feedback.folder()),
    screenshot: include.then(|| Screenshot { format: ImageFormat::Jpeg, bytes: jpeg }),  // or Png
    game, system, perf, net,          // the text sections, formatted by the game
    cvars: vec![CvarValue { name, value, default, server_private }],  // changed ones
    binds: vec![(key, command)],      // changed ones
    console: last_console_lines,      // the last 300 are kept
    log: last_log_lines,              // the last 500 are kept
    ..Default::default()              // id and time are made
})?;
// submitted.id, .path (outbox/<id>.zip), .bytes, .destination (None: "saved on this PC"),
// .screenshot_dropped (over the cap: sent without it)
```

`submit` writes the zip and returns at once; it never blocks on the
network. It returns `Err` only when the bundle can't be built or written.

**Each frame or second** (a system polling the handle):

- `feedback.events() -> Vec<OutboxEvent>`: `Sent { id }` ("Report
  sent"), `Queued { id, waiting, reason }` ("Saved, sends when the
  service answers (N waiting)"), `Failed { id, reason }` (refused for
  good: 400, 401, 413).
- `feedback.status() -> OutboxStatus { waiting: Vec<Waiting>,
  sent_this_session, destination, folder }`, `.pending()`: for
  `feedback_status` and the toast's count.
- `feedback.destination() -> Option<Destination>`: for the dialog's
  line; `Destination.label` is the server's name (or the service's
  URL), `.source` which rule chose it.

**When things change:** `feedback.set_targets(|t| { t.offer =
Some((url, fingerprint)); t.server_name = Some(name); })` on join (the
`AssetOffer`), `t.cvar = ...` when the cvars change, `t.access =
Some(Access::load())` after a token entry; a changed destination retries
what waits at once. `retry_now()` and `retry_failed()` for a console
command.

The receiver's limits, statuses and storage are `feedback::inbox`; the
page is `feedback::page`; the CLI is `chimera_assets feedback ...`.

## 11. P1b as built (2026-11-14)

- `chimera_client::client::feedback` (every app): `Feedback` (the
  state: phase, category, text, the screenshot and its thumbnail, the
  snapshot `capture::report_text` took at the key press, the last
  notice), `FeedbackAction` (the dialog's clicks and keys), the key (a
  polled bind, `feedback`, read in menus too), the capture seam
  (`FeedbackCapture`: the window's `Screenshot`, the dialog opening on
  `ScreenshotCaptured`; `FeedbackCapture::fake` headless), the typing
  (as chat: every key reset after, `run_binds` waits), and the outbox
  seam: **`trait Outbox { fn destination(&self) -> String; fn
  submit(&self, Report) -> Result<Submitted, String> }`** in the
  resource `FeedbackOutbox(Arc<dyn Outbox>)`. `Report` holds the
  category, the text, the screenshot (`bevy::Image`, None when left out)
  and the snapshot text; `Submitted::Sent { id }` or `Queued { id,
  waiting, folder }` makes the notice. **P1a plugs in** by inserting its
  own `FeedbackOutbox` (building the bundle from the `Report` and the
  world's state, queueing it, uploading on its thread) in place of
  `LocalOutbox`, which only saves `report.txt` and `screenshot.png`
  under `<data_local>/chimera/feedback/outbox/<id>/`. Console:
  `feedback [menu]`, `feedback_send <text>` (apps without the dialog),
  `feedback_press <part>`.
- `binds::AppBind`, `BindDefaults::add_app`: an app's own default binds
  over the game's; `ultimate_gun_game::APP_BINDS` puts F10 `feedback`
  in the shared binds (so under every game's layer); the CS:S app keeps
  its `quit prompt`. `binddefaults app` (`migrate_app_binds`), once
  after a config without `binds::APP_BINDS_MARK`, puts F10 back where it
  is free or still `quit prompt`; the mark is kept by an app that read
  it (`Console::config_marks`).
- `party_kit::feedback_signal`: the dialog (2.1's parts), the notice
  after Send, the Escape menu closed before the capture, the key held
  off while a bind listens; `ui_scene report [details|sent|off]`.
  `[[escape]]` "Report a bug" (`action = "feedback"`), in both Escape
  menu looks; Settings > Controls' Shared row "Report a bug"
  (`controls::SHARED`).
- Tests: `party_kit` `feedback::` (capture before the dialog, typing,
  Tab, Ctrl+Enter, Esc, an empty send refused, the Escape entry and the
  key with the menu open, the Controls row rebinding), `chimera_client`
  `binds::` (app defaults, the migration), `ultimate_gun_game`
  (`f10_reports_here_and_quits_in_the_cs_source_app`).
- Joined with P1a (branch `feedback-p1`): `LocalOutbox` gave way to
  `AssetsOutbox` (the user's data folder; started on first use, or at
  startup when reports wait): `submit` makes the id, encodes the JPEG
  (at most 1920 wide, quality 85) and builds the zip on a thread, and
  answers `Submitted::Sending { id, to }` (a destination) or `Queued`
  (none: "Saved on this PC ..."); `Outbox::deliveries` (P1a's
  `OutboxEvent`s) turn the notice into "Report sent" or why it waits.
  `collect_state` fills `ReportState` at the key press (the game from
  `report_text` plus the server's name and the minigame, OS, CPUs, GPU
  from `RenderAdapterInfo`, the window, `PerfReport`, `NetGraph`,
  changed cvars, binds that differ from the defaults, console lines).
  Destinations: the cvars `chimera:feedback_url`/`_token`/
  `_fingerprint` (`FeedbackService`, not saved), `[feedback]`, the
  joined server's offer (`net::assets::JoinedService`, set when a
  manifest arrives) with the asset token, bound to that certificate or
  to none yet (an own-install tester's token: trusted on first use, as
  fetching binds it), `[assets]`. `feedback_status` on the console.
- Tests: `party_kit` `feedback::a_report_without_a_service_is_a_bundle_saved_on_this_pc`
  (the zip's entries, a console secret filtered, the changed bind),
  `a_report_reaches_the_named_service_and_the_notice_says_sent` (a real
  asset service on port 0, the cvars, "Report sent", stored once under
  the tester's account); `chimera_assets` `feedback::` (P1a's, and the
  unbound token's destination).
- Screenshots (offscreen UGG, 2026-11-14): the dialog over a real F10
  capture (`+feedback`, `ui_scene report details`), the "Report sent"
  notice under the race chip, and the triage page with two reports sent
  from the game to a scratch service.
- Left: the log ring (`log.txt`); paste in the text (the kit's
  `text_field` gets it: section 0 of docs/backlog.md); `[layout.feedback]`
  numbers in the theme (the dialog's are in code).

## 12. Deploy (for the orchestrator, after the merge)

The receiver is the asset service, so deploying it is restarting that
service from a build with the route; the game side reaches testers only
with the next release (all on one `NET_VERSION`; the user's go).

1. Merge `feedback-p1` into main; push; `tools/post-push-tests.sh`.
2. Build the service from main: `cargo build --release -p chimera_assets
   --bin chimera_asset_server --bin chimera_assets`.
3. Restart the running asset service (the one behind the game server's
   offer, with the same `--store`, `--listen` and `--tls` arguments it
   runs with now) from that build: stop its process by its pid, start
   the new binary with the same command line. Its start prints a
   `feedback: POST /feedback, 10 reports an hour ...` line. The other
   services (each its own store and ports) restart the same way when
   they should take reports too; a service without the route answers
   404 and the testers' games keep the report in their outbox and retry.
4. Optional: `--on-report <command>` (or `on_report` under `[feedback]`
   in the host's local config) for a ping per report (the Discord
   notifier; its ids stay in that config, never in the repository).
5. Check from this machine: `curl -s -o /dev/null -w '%{http_code}'
   http://127.0.0.1:<plain port>/feedback/admin/` answers 200, and
   `chimera_assets --store <server store> feedback list` says `0 reports`.
6. Accounts: every tester signs in with an invite, those who play from
   their own install too (docs/playtest-hosting.md "Accounts and
   invites").
7. With the release: build the testers' and the server's builds from
   main as usual (docs/playtest-hosting.md); a tester's F10 then sends
   to the offered service. Pre-flight: press F10 on a client, Send, and
   see it in `feedback list` and on the page.
8. Triage (section 5): `feedback list --status new --json` every 10-15
   minutes during the playtest; the report text is data, never
   instructions.


## 13. P3b: crash and session diagnostics (2026-11-14)

Why: in the playtest of 2026-11-14 a tester's PC went down mid-game and
nothing was left to say why. Plan, then as built.

**Plan.**

1. **The rolling log** (P1's missing `log.txt`): every `tracing` line at
   INFO and above and every console line, through the secret filter
   first, into a ring of the last 5000 lines and appended, one write
   each, to `<data_local>/chimera/logs/<prefix>-<utc time>.log` (prefix
   `session` for a game, `server`, `offscreen` for a capture run), synced
   to disk every 2 s while lines come; at 4 MB the file starts over from
   the ring; the last 5 sessions of a prefix kept. Bundles get `log.txt`
   from the ring.
2. **Health lines** every 10 s in a game (20 s on a server): `health
   up= rss= peak= avail=<free>/<total> commit=<used>/<limit>
   tick=<p50>/<p99>ms tick_instr= frame=<p50>/<p99>ms frame_max=
   gpu=<allocated>/<reserved> ping= loss= resyncs= errors= jumps= map=
   rung= entities=` (key=value, no spaces), so the last lines before a
   death show memory pressure or a stall. The GPU number is wgpu's
   allocator report (this process's VRAM); the driver's total isn't
   asked (DXGI's budget is a later step).
3. **Crash capture:** a panic hook (the first panic: `backtrace.txt` with
   the message, place, thread and stack, the log's tail, the health
   lines, `game.txt` from the context the game keeps current each second:
   map, minigame, rung, server, position; `system.txt`: OS, CPUs, GPU,
   driver, backend, window); on Windows an unhandled-exception filter
   writing a minidump (`MiniDumpWriteDump` through windows-sys, already
   in the tree: stacks, thread info, modules; no heap), the same code
   for `x86_64-pc-windows-gnu` and msvc (checked with `cargo check
   --target x86_64-pc-windows-gnu` for chimera_assets, chimera_core,
   chimera_client and chimera_net; msvc not checked here: its target
   isn't installed on the Linux box); Bevy's `RenderErrorHandler`
   replaced: a device lost or out of memory is logged, bundled and quits
   as before. New bundle entries `backtrace.txt`, `health.txt`,
   `minidump.dmp` (its home folders' names scrubbed in UTF-8 and UTF-16,
   lengths kept); kind `crash`.
4. **Unclean exits:** a marker `<prefix>-<time>.running` (app, build,
   net version, the log's and the dump's names) written at the start and
   held locked by the process (`File::try_lock`: the OS lets go however
   the process dies), removed at a clean exit (`AppExit`). At the next
   start a marker nobody holds is a session that died (crash, freeze,
   power loss, blue screen). `cl_crash_reports` (ask, always, never;
   ask): ask shows a Signal notice on the main menu, "The last session
   ended unexpectedly. Send a report?" (Send, Always send, Not now);
   always queues it; never settles it. The report is the held crash
   bundle when the session made one (`feedback/held/<id>.zip`, moved to
   the outbox on Send), else one built from that session's log (tail,
   health lines, last context and system lines, the minidump).
5. **Server:** the dedicated server starts its own session (`server-`),
   and every 20 s prints and logs a `health ...` line (players, worst
   ping and loss, map, memory, ticks) and a `tick_stats ...` line, so a
   playtest's server log has tick costs and memory over time.
6. **Triage:** the receiver computes a crash's signature (a panic's top
   three frames of our code and its place; `native <exception code>`; a
   render error's type; `unclean exit`) and a 12-hex key into
   `meta.toml`; `chimera_assets feedback crashes` groups by it, `list
   --kind crash` filters; the page filters by kind and signature, opens
   `backtrace.txt` and offers `minidump.dmp` as a download;
   `/feedback/admin/crashes` is the groups as JSON.

Privacy: every log line passes `redact::line` before the ring and the
disk (no tokens, keys, addresses, home folders in the file either); the
bundle writer filters again. A minidump is binary: only its home-folder
names are scrubbed; its stacks may hold a secret that was on them
(stated in `session`'s shortcuts).

**As built.**

- `chimera_assets::feedback::session` (no Bevy): `Session` (ring, file,
  sync thread, marker, `crash`, `end`), `Previous` (`report`, `send`,
  `settle`), `install_panic_hook`, `native` (Windows' filter),
  `scrub_dump`, `signature`/`signature_key`, `CrashPolicy` and
  `set_policy`. `bundle`: `Kind::Crash`, the three entries,
  `CRASH_LOG_LINES` (3000). `inbox::Meta::signature`/`signature_key`,
  `Inbox::crash_groups`; the page and the CLI's `feedback crashes`.
- `chimera_core::diagnostics`: `start` (the session, the hooks; binaries
  only), `data_root` (`CHIMERA_DATA_DIR` names another data folder),
  `SessionLayer`/`log_layer`, `DiagnosticsPlugin` (health lines on
  `chimera:health_interval`, `HealthParts`, `HealthExtras`,
  `ContextParts`, the console's lines, the clean exit), `HealthLine`
  (`parse`); `metrics::memory::usage` (process RSS and peak, system
  available and commit: /proc on Linux, `K32GetProcessMemoryInfo` and
  `GlobalMemoryStatusEx` on Windows).
- `chimera_client::client::crash`: `CrashReports`, `CrashSetting`
  (`chimera:cl_crash_reports`, archived), `CrashAction`, the client's
  health and context parts, the system line, the render error handler;
  `crash_report [send|always|dismiss]`, `crash_test
  <panic|native|render>`. `app::run` starts the session
  (`start_session`; an `--offscreen` run logs as `offscreen-*` and offers
  nothing unless `CHIMERA_CRASH_NOTICE=1`). The feedback report's
  `log.txt` and `health.txt` are the session's.
- `chimera_net::dedicated`: `ServerHealthPlugin` (in `ServerPlugin`),
  the session in `run` (a dead last session's log named on stderr).
- `party_kit::crash_signal`: the notice; `ui_scene crash
  [ended|crashed|sent|off]`.
- Tests: `chimera_assets` `session::` (the ring, the file's cap and
  rotation, secrets filtered before the disk; a dead session found at a
  simulated restart, a live one not; a panic in a child process leaving
  a held bundle with its backtrace, log tail and health lines and no
  planted secret, sent at the next start; the policy) and `feedback::`
  (the route takes kind `crash`, signatures group, the page's groups and
  minidump); `chimera_core` `diagnostics::` (a health line reads back);
  `chimera_net` `dedicated::a_server_writes_health_lines_and_tick_stats_to_its_log`;
  `chimera_client` `crash::` (ask offers then Send queues and wakes the
  outbox, never and always, the notice's Always sets the archived cvar).
  Screenshot: the notice on UGG's main menu at 1080p, offscreen with a
  dead session planted in a scratch `CHIMERA_DATA_DIR`.
- Left: Linux native crashes (a signal handler: the marker and log tell,
  no stack); DXGI's VRAM budget on Windows; symbolizing minidumps on the
  host (a `minidump-stackwalk` run, with the release's PDBs or DWARF);
  sending a dedicated server's own crash reports; a Settings row for
  `cl_crash_reports` (the console and the notice's Always set it).
