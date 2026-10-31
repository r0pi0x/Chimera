# Plan: the party apps' main menu

Status: proposed 2026-10-30, decided 2026-10-31 (section 10), not
started; Ultimate Gun Game only while Lucker Party is shelved. Backlog: "A main menu for
Ultimate Gun Game (and the other party apps)". Comes after the asset
service's M1 and the ECS cleanup (backlog roadmap, "Order").

## 1. Goals

- Ultimate Gun Game and Lucker Party start at a menu of their own
  (`party_kit`, like the Escape menu: not CS:S's GameUI, not Combat
  Arms' lobby), in the party's panel look.
- From it: **Play** (host a game), **Join** (direct connect, then a
  server browser), **Options**, **Quit**.
- The Escape menu gets **Disconnect**, back to this menu; a dropped or
  refused join also lands there, with the reason.
- Loading (joining, map changes, later asset downloads) is shown by the
  party, with progress, and can be cancelled.
- Scripted runs keep working: `--map`, `+minigame`, `+connect`,
  `--screenshot` skip the menu exactly as today (`Args::starts_in_game`,
  `joins_server`), so `scripts\playtest.ps1`, the host/join scripts and
  every test are unchanged.

Non-goals: Combat Arms' lobby (rooms, loadouts; that is Combat Arms'
own front end), accounts or sign-in (the asset plan's), a map vote,
parties or friends lists.

## 2. What exists to reuse

| Need | Have | Where |
|---|---|---|
| Panel look, pages from data, options tabs | the Escape menu | `party_kit::esc_menu`, data/options.toml |
| Host settings (bots, rotation, rung extras, votes) | host page and its cvars | `party_kit::host`, data/host_settings.toml |
| Shade and panel alphas | `widgets::display_alpha` (the rule: alphas chosen as blended in display space) | `chimera_client::client::widgets` |
| Modal confirm and text input | `dialog::Dialog` (OK, OK/Cancel, text, password) | `chimera_client::client::dialog` |
| Menu owns mouse and keys | `front_end::AppMenu`, `MenuStatus`, `binds::CapturedKeys` | `chimera_client::client::front_end` |
| Menu lifecycle hooks | `FrontEnd` (`entered_game`, `left_game`, `joining`, `show_failure`, `map_load_failed`, `start_game`) | same |
| Back to the menu after leaving | `client::net::back_to_menu` (greybox, fresh local player) | `chimera_client::client::net` |
| Listen server | `listen_if_hosting` after a map loads with `maxplayers` > 1; `net_upnp` | `chimera_client::client::net`, `chimera_net::net::upnp` |
| Join stages and download progress | `net::client::JoinProgress` (`JoinStage`, `DownloadInfo`, `failure`, `log`) | `chimera_net::net::client` |
| Map download | over the connection or `sv_downloadurl`, hash-checked, content cache | `chimera_net::net::maps`, `net::http` |
| Server queries, LAN scan | A2S_INFO-shaped query on the game port, `ServerQueries` (LAN on `net_lan_ports`, ping, one entry per instance) | `chimera_net::net::query` |
| Server browser model | CS:S's Find Servers (`ServerBrowser::handle`: sort, filters, favourites and history in `serverbrowser.vdf`, password) | `games/cs_source/src/ui/server_browser.rs` |
| Match end | the shell's match-end screen and rotation (`NextMatch`) | `chimera_client::client::match_end` |
| Mount check at join | match manifest vs the client's mount report | `chimera_net::net::manifest` |
| Loading screen | Combat Arms' (`CaLoading`, the install's layout; nothing drawn without it) | `games/combat_arms/src/ui/loading.rs` |

CS:S's main menu (`game_menu`) is a model of what the flow needs
(loading dialog with Cancel, Disconnected dialog with the game's
words, Create Server's pages as console lines) but it is CS:S's look and
is not in the party apps; nothing of its drawing is reused.

## 3. Screens and entries

One menu, pages like the Escape menu's. Out of a game it is always open
(Esc does nothing on its first page).

- **Main page:** the app's title (`EscMenuTitle`), then Play, Join,
  Find servers (M2), Options, Quit.
- **Play (host a game):**
  - Minigame: the app's list (Lucker Party: `originals`' minigames;
    Ultimate Gun Game: its one, with a "New ladder" seed line).
  - Map: "Rotation" (the minigame's pool, as today) or one map of the
    pool.
  - Bots (`MaxPlayer`), and the rest of the host page
    (host_settings.toml) shown here too: the same rows, before the
    match instead of during it.
  - Server: name (`hostname`), password (`sv_password`), max players
    (`maxplayers`; 1 = alone, no listen server), port (`hostport`),
    open the router port (`net_upnp`).
  - Start: console lines only (`hostname ..; maxplayers ..; minigame
    <id>; map <map>`), as CS:S's Create Server; the listen server starts
    after the load as it does now.
- **Join:** an address entry (`dialog` text input; host:port, the port
  defaulting to 27015), a password entry, Connect (`password ..;
  connect ..`). The last few addresses as buttons (the history file).
- **Find servers (M2):** a list (name, minigame, map, players/max,
  bots, ping, lock), sorted by ping; Refresh; tabs LAN, Favourites,
  History (Internet greyed until M3); a click selects, Connect or a
  double click joins (password dialog for a locked server); "Add to
  favourites".
- **Options:** the Escape menu's options tabs (data/options.toml) and
  Binds (the Controls page), the same code.
- **Quit.**
- **Escape menu, in a game:** Disconnect added above Quit. A host's
  confirm says it ends the game for the others ("End the game for 3
  players?"); a client's leaves at once.

Data-driven pieces: the main page's entries and their console lines in
`data/main_menu.toml` (label, command, `when` = always, in_game,
out_of_game, host), so Lucker Party and Ultimate Gun Game differ by
data; the Play page's rows reuse `host::Setting` rows (the server rows
added to host_settings.toml with a `page = "play"` tag); options stay
options.toml. The Escape menu's own entries move to the same file, so
Disconnect is one data line gated on in_game.

## 4. The flow

```
start --(no map/connect)--> MainMenu --Play/Start--> Loading --> InGame
                               ^  |--Join/Connect--> Joining --> Loading --> InGame
                               |  '--Quit
   Disconnect / dropped / refused / Cancel (from Joining, Loading, InGame)
```

- **Start:** without a map, minigame or connect the app loads the
  greybox behind the menu (as `client_app` loads `--map` today; it
  stops exiting when no map is named) and opens the menu. Background:
  open question Q1.
- **Joining:** `connect` from Join or Find servers; the loading panel
  shows `JoinProgress` (connecting, server info, verifying, downloading
  with bytes and rate, loading the map), Cancel = `disconnect`.
- **Asset downloads (asset plan M3/M4):** the same panel gains a line
  per set being fetched (base, map, weapons, players: bytes of total,
  rate), from the asset client's progress resource next to
  `JoinProgress::download`. `cl_allowdownload 0` or a refused game turns
  into a failure that lists what is missing (the asset plan's "joining
  says exactly what is missing"); the mount check's refusal already says
  which game. Until the asset plan lands only the map line exists.
- **In game:** the Escape menu as now, plus Disconnect.
- **Match end:** unchanged: the match-end screen, then the rotation's
  next map, shown in the loading panel as a map change (the players stay
  in). No return to the menu between matches.
- **Leaving:** Disconnect, a drop, a refusal or a failed load:
  `back_to_menu` (greybox, fresh local player), the menu opens with the
  reason in a message box (`JoinProgress::failure`). A host leaving ends
  its listen server: clients are dropped with "The host left" (a new
  reason, `JoinFailure`).

## 5. ECS shape

- **State as a resource the menu owns** (`PartyMenu`: `screen` =
  MainMenu | Joining | Loading | InGame, `page`, notice), set at known
  moments, not polled. No Bevy `States`: the repo has none, and DESIGN
  "ECS style" orders run conditions and triggered events instead; a
  run condition `party_screen_is(..)` gates the per-frame systems.
- **The front-end hooks become events** (prerequisite, first commit of
  M1): `FrontEnd`'s six fn hooks are single-slot, and the party menu
  and Combat Arms' loading screen both register `entered_game`,
  `left_game`, `joining`, `map_load_failed` (the second replaces the
  first). They become triggered events (`front_end::EnteredGame`,
  `LeftGame`, `Joining { label }`, `JoinFailed`, `MapLoadFailed`,
  `StartGame`) observed by each front end; CS:S's game menu and
  Combat Arms' loading screen move to observers in the same commit.
  This is what the ECS style asks ("events triggered at a known
  moment"), and it takes one entry off the callback registries list
  (`checks/tests/it/ecs.rs`).
- **One front end at a time:** with the party menu in, Combat Arms'
  loading screen and its lobby screen stay shut (as its Esc menu yields
  to `AppMenu`): an `AppFrontEnd` marker resource the CA observers
  check. Open question Q5 asks whether to keep CA's loading art instead.
- **Actions as messages** (`MainMenuAction`, like `EscMenuAction`):
  clicks and tests send the same; a single exclusive `act` turns them
  into console lines (menus are an allowed whole-world place).
- **Drawing:** Bevy UI rebuilt when the shown state changes (as
  `esc_menu::sync`/`show`), z between the match-end screen and the
  console (`MENU_Z`).
- **The server browser model** moves from cs_source into Chimera
  (`chimera_client::client::server_list`: entries from `ServerQueries`,
  sort, filters, favourites and history in `serverbrowser.vdf`), per
  "Generalize for the second game"; CS:S's Find Servers draws the
  lifted model in its look, the party menu in its own.

## 6. The look

- The Escape menu's panel: the shade and panel colours through
  `display_alpha`, its buttons, text sizes and accent; full screen
  without the world's shade when out of a game (the background shows).
- The title: the app's name in text at first; title art is Q4.
- No other game's UI art (it would need that install and would be one
  game's look); a background picture taken from an install (a map
  screenshot, a loading picture) only as an option behind Q1.
- Sounds: hover and click from an install where one is found (CS:S's
  `ui/buttonrollover`, `buttonclick`), silent without; music is Q2.

## 7. Networking needs

- **M1 needs nothing new** on the wire: connect, listen, passwords,
  map download and join stages exist. One addition: a "host left"
  disconnect reason.
- **M2 (LAN):** `ServerQueries`' LAN scan already finds our servers
  (folder `chimera`). Add to the info reply's keywords: the app
  (`lucker_party`, `ultimate_gun_game`) and the minigame id, so the
  list shows the minigame and filters out the other app's servers (or
  marks them). Still to see: broadcast between two machines (only
  127.0.0.1 was seen live, multiplayer.md slice 8), Windows firewall
  prompts for the query port.
- **M3 (Internet):** a master server of our own: servers register
  (heartbeat with address and the query port), clients get a list and
  query each server directly with A2S as today. Small HTTP service; it
  could run beside the asset service's dev server (asset plan section
  5) on the same host. Behind NAT, listings reach only servers with a
  forwarded port or `net_upnp` (docs/plans/active/nat-traversal.md);
  a relay is out of scope here. Not A2S_PLAYER/A2S_RULES unless a
  player list in the browser is asked for.

## 8. Milestones

- **M1: the menu, host, join, options, Disconnect.**
  1. `FrontEnd` hooks as triggered events (CS:S and CA moved over).
  2. `party_kit::main_menu`: start at the menu without a map;
     data/main_menu.toml; Play page (minigame, map or rotation, bots,
     host settings, server name, password, max players, port, UPnP);
     Join (address, password, recent addresses); Options (the Escape
     menu's tabs and Binds); Quit.
  3. The party loading panel (`JoinProgress` stages, map download
     bytes, Cancel) and the failure message box.
  4. Escape menu: Disconnect (host confirm), "host left" reason; DESIGN
     "The party apps' Escape menu" and the backlog updated.
- **M2: server browser (LAN).** The browser model lifted into
  `chimera_client` (CS:S's Find Servers on it, its tests kept);
  keywords with app and minigame in the query reply; the party's Find
  servers page: LAN, Favourites, History, password dialog. A LAN test
  between the Windows PC and the Linux box.
- **M3: Internet list and favourites polish.** The master server
  (register, heartbeat, list), the Internet tab, favourites from the
  in-game Escape menu ("Add this server"), the asset plan's per-set
  download lines if its M3 is in by then.

## 9. Tests per milestone

All headless in `apps/party_kit/tests/it/` (`main_menu.rs`, like
`esc_menu.rs`), driven by `MainMenuAction`s and the console; network
ones on `NetSim` in the lowest crate that has the pieces.

- **M1:**
  - Startup without a map opens the menu on the greybox; with `--map`,
    `+minigame` or `+connect` it does not (startup.rs extended).
  - Play/Start submits the expected console lines and ends in game with
    the minigame playing; `maxplayers` > 1 listens.
  - Join with an address and password on a NetSim server: in game;
    a wrong password and a full server come back to the menu with the
    reason.
  - Cancel during joining and during loading returns to the menu.
  - Disconnect from the Escape menu: back at the menu, the local player
    fresh, the menu open; a host's Disconnect drops its clients with
    "host left".
  - The front-end events: CS:S's game menu and CA's loading screen tests
    pass unchanged on observers; with the party menu in, CA's loading
    screen stays shut.
  - Data: every main_menu.toml entry parses and its command exists
    (as the options data test does).
  - Screenshots (the playtest skill): main page, Play, Join, loading,
    the failure box, Disconnect confirm.
- **M2:** the lifted model's unit tests (CS:S's server_browser tests
  moved); a query reply carries app and minigame keywords; a LAN scan
  of two party servers lists both, a CS:S-only server marked; joining
  from the list; favourites and history round trip.
- **M3:** the master server against servers on localhost (register,
  expire on missed heartbeats, list); the Internet tab lists and joins
  one; a heavy test on the dev host if it runs there.

## 10. Decisions (user, 2026-10-31)

Lucker Party is shelved for now: the menu is built for **Ultimate Gun
Game only**. Lucker Party keeps starting straight into its minigame, and
its playlist question and `playtest.ps1 lucker_party` wait until it is
picked up again. Where this plan says "Lucker Party", read it as later.

1. **Background:** a live map with a slow camera flight (the map load at
   start is accepted).
2. **Music:** the installed game's own menu music (Combat Arms' BGM),
   silent when there is none.
3. **LAN browser:** direct connect in M1, the browser in M2.
4. **Branding:** a text title; a logo may replace it later.
5. **Loading screen:** the game's own where it has one (Combat Arms'
   loading screen with its map picture), the party panel otherwise.
6. **Hosting default:** Play always opens a joinable listen server;
   bots fill the empty slots.
7. **Minigame choice:** shelved with Lucker Party (its answer was a
   playlist).
8. **Master server:** beside the asset dev server (one box over
   Tailscale, the VPS together later).
9. **Default launch:** shelved with Lucker Party.

Order: the asset service's M2 (the dev server with internal accounts)
comes first, then this plan's M1.
