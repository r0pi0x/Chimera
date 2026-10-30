# Observability: how to check your work

Agents can't see the screen. These tools make the game's state readable.
Prefer them, in this order, over asking a human to look.

## 1. Headless scenario tests (fastest, most precise)

`cargo test` runs `chimera/sim/tests/it/movement.rs` and friends on
`harness::Sim` (chimera_world): no window, time advanced by exact fixed
ticks, `Intent` driven directly. A `Sim` runs the simulation plugins it
is given as its base, the first argument: the engine's alone
(`chimera_sim::SimBasePlugins`) in Chimera's crates, CS:S's
(`cs_source::harness::CsSim`: `cs_source::SimPlugins`, map logic's
random picks seeded) in `cs_source`'s. Its schedules run on the
single-threaded executors the game and the dedicated server run
(`Sim::multi_threaded` or `CHIMERA_EXECUTOR=multi` for Bevy's), with the
task pools capped at 4 threads, so a test passes with the order players
get.

```rust
let mut sim = Sim::new(SimBasePlugins, GreyboxMapPlugin);
let p = sim.spawn_character(Vec3::new(0.0, 1.0, 12.0), placeholder::ID);
sim.intent(p).move_axis = Vec2::Y;
sim.seconds(1.0);
assert!(sim.velocity(p).xz().length() > 4.9);
```

`NetSim` (chimera_net's `harness`) is a server and clients, each a `Sim`
on the same base, over an in-memory link: `NetSim::new(CsSim,
conditions, seed, clients, setup)`; `cs_source::harness`'s `TestMaps`
and `serve_maps` give network tests maps built in code.

Every spec test case becomes one of these. New behavior gets a scenario test
before it is called done.

A `Sim` has no render world. Code that rewrites mesh assets can still be
checked against Bevy's extraction with `harness::emulate_mesh_extraction`
(each frame it takes a `RENDER_WORLD`-only mesh's data as the render world
does; `MeshExtraction::failures` lists meshes announced changed after their
data was gone, which a real run logs as "cannot be extracted: The asset has
already been extracted"; `chimera/present/tests/it/prop_shadows.rs` uses it).

Each crate keeps its own tests, so a change runs only the tests of the
crates it can affect (cargo's crate graph; the hooks below). A test goes
in the lowest crate whose public API it needs:

- unit tests beside the code (`#[cfg(test)]`), for one module's logic;
- a crate's integration tests, one test binary per crate:
  `<crate>/tests/it/main.rs` lists each file as a module (the crate links
  once for all of them: docs/performance.md, "Test cycle");
  `tests/it/heavy/` (inside `mod heavy { }`) for tests that load real
  maps from the install or simulate long (bot rounds);
- `chimera_sim` (chimera/sim/tests/it/) and `chimera_client`
  (chimera/client/tests/it/): scenario tests of the engine's movement,
  weapons or shell parts on the greybox (`chimera_world::greybox`) with
  the engine's base, `Sim::new(SimBasePlugins, ..)`;
  The **test kit** (`chimera_sim::testkit`, feature `testkit`, turned
  on by dev-dependencies only) is what these run on instead of a game's
  data: a weapon set as TOML in round numbers (`kit:pistol`, `kit:rifle`,
  `kit:shotgun`), `kit_sim()` (the base on the greybox with those
  loaded), `with_player`, `active`, `active_id`, `feet`, `aim_at`. A test
  that needs nothing of a game goes down onto it: **contract tests** per
  Chimera crate (the weapon frame's timing, magazines and damage:
  `chimera/sim/tests/it/weapon_frame.rs`; a code-built map's spawn points
  and unload: `map_lifetime.rs`; noclip's damage rules: `noclip_damage.rs`;
  prediction with zero errors for a scripted run, loss and jitter, and
  own-state parts over `NetSim`: `chimera/net/tests/it/prediction.rs`).
  When a lift moves code into Chimera, the generic part of its tests moves
  down with it (`cargo test -p chimera_sim -p chimera_net` checks the lift
  in seconds, before the games' suites). A test that names a game's
  weapon, hook, map or rules stays in the game;
- each game's tests share one support module (`games/cs_source/tests/common/`,
  `games/combat_arms/tests/common/`): the install skip rule and map
  loading by id (CS:S), the usual `sim()` builder, the weapon in hand
  (`active`, `active_id`). A helper a second test file needs goes there,
  not into another copy of `fn sim(`;
- `chimera_present` (chimera/present/tests/it/): the renderer's and
  audio's spec cases with nothing of a game, and what it draws on a
  `Sim` with `core` and the console as the base (prop LODs, shadows);
- `cs_source` (games/cs_source/tests/it/): anything with CS:S in it:
  its loaders and spec test cases (the install through `mount`,
  `bsp::load_level`; tests/common/'s `load_map`), and the scenario tests
  on `Sim::new(CsSim, ..)` or `NetSim::new(CsSim, ..)`: movement,
  weapons, rules, bots, map logic, network play, and the heavy map
  tests. tests/common/stack.rs re-exports what they run on at the app's
  paths (`crate::stack::{core, map, harness, games, ...}`, `map::MapPlugin`
  from the presentation, a dev-dependency). Its front end's (menus, HUD)
  and the scenario tests that read the client shell's or the front end's
  state go in tests/ui/, built with `ui` (`stack::client`), so an edit to
  `src/ui/` runs only those (and the crates above);
- `combat_arms` (games/combat_arms/tests/it/): Combat Arms' scenario
  tests (the options' profile, bindings and settings: `options.rs`) on `Sim::new(CaSim, ..)` (`combat_arms::harness::CaSim`: its
  whole simulation, placeholders included) and the greybox, or a map
  converted from a world built in code (`world.rs`); its spec test cases
  as each subsystem is implemented; in `heavy` the install's maps
  (`heavy::world`: every map converts and a player lands at its first
  spawn, the plain ones in one test and the V2-keyed ones one test per
  archive, two maps at a time; a player walks and collides on
  WAREHOUSE; `heavy::model`: a prop model converts and animates as
  LithTech samples it; `heavy::movement_volumes`: the volume census of
  the 105 maps and players in their sea, ladders, gravity volumes, safety
  nets and jump pads); gear and perks (`gear_perks.rs`: the spec's 56
  cases on the movement tests' player and the greybox, the console's
  loadout, a loadout over `NetSim`); the death camera (`death_killcam.rs`: the
  pull-back and orbit maths, the phase machine and the respawn countdown
  headless, and `CaDeathCam` on players in a match); radio and chat
  (`radio_chat.rs`: a room of players through each side's own functions;
  `radio_chat_net.rs`: a server and two clients; `heavy::radio`: the
  install's radio file); its front end's (the
  HUD's state, `ui::hud::CaHud`, headless; the UI system's layout,
  text and input cases and the in-game menu, `ui_system.rs`; the kill
  feed, score panel, scoreboard and loading screen bound by name,
  `bound.rs`; the radar and the full map's state, `ui::hud::radar` and
  `full_map`, `radar.rs`; the death screen bound by role and the dead player's name
  tags, `death_screen.rs`; the chat panel, input and radio window,
  `radio_chat.rs`; the options screen, its key capture and what it
  hands the shell, with the HUD on/off key and the radar zoom in the
  profile, `options.rs`) in tests/ui/, built with `ui` (`heavy::install_ui`: the
  install's own layouts and string tables through the UI system's
  readers, the game's HUD panels, scoreboard and loading screen bound
  by name and its in-game menu, V1 key; `heavy::radar`: the map images'
  sizes and formats, the radar art and icons, the mission files' corners
  on four maps, the large-map attribute file, V1 key; `heavy::options`
  (tests/it/: the key-name file; tests/ui/: the install's five options
  layouts and the controls the screen binds in them, V1 key);
  `heavy::view_model`: the M4A1's and knife's view models loaded on
  demand and drawn, visible, on the greybox, V2 key);
- `combat_arms_server` (apps/combat_arms_server/tests/it/): Combat
  Arms' dedicated server as its binary composes it (`ServerPlugin`, its
  command line) with headless clients over UDP on loopback (`dedicated`:
  two clients play a TDM kill on the greybox; `MaxPlayer` fills it with
  bots);
- `combat_arms_app` (apps/combat_arms/tests/it/): the simulation as the
  game binary composes it (`combat_arms_app::SimPlugin`: the install's
  data and the room's default mode, unlike `CaSim`) on the headless
  harness, maps loaded as `--map` loads them (`heavy::map_start`: a map
  started on plays its mission, V1 key);
- `cs_source_app` (apps/cs_source/tests/it/): only what the app alone
  has (its build's version, its hooks, map loading by id);
- `repo_checks` (checks/tests/it/architecture.rs): the repository's
  rules (layering, assets, specs, docs);
- `lithtech_formats` (lithtech/formats/tests/it/): LithTech's readers on
  fixtures written in code from the specs, and in `heavy` on the Combat
  Arms install (skipped without one; decryption only with keys in
  `chimera.local.toml`);
- `mashup` (the root, tests/it/): `dump`'s test.

A file directly in a `tests/` folder would be another test binary:
`architecture::tests_are_one_binary_per_crate` fails and says how to move
it. Test names read `weapons::name` and `heavy::map_de_dust2::name` within
their binary (nextest: `cs_source::it heavy::map_de_dust2::name`).
Commands at the repository root cover every crate; `-p <crate>` picks one:
`cargo test -p source_formats` builds and runs only the format readers'
tests, in seconds, without building the game.

Which crates' tests a change can affect: `cargo run -q -p repo_checks
--bin affected -- --base main` (or `--staged`, `--files <paths>`) prints
what it picked and why, and the arguments for `cargo nextest run`
(checks/src/affected.rs has the rules). A crate's change selects its
tests and those of every crate depending on it (dev-dependencies too);
a test file's change only its own test module (and modules naming it);
a file in a folder only a feature compiles (a manifest's
`[package.metadata.affected] feature-paths`: cs_source's `src/ui/`) its
crate's test binaries built with the feature and the crates above, not
the crate's other tests (an edit to CS:S's HUD doesn't run its map
tests); a Cargo.lock change the crates whose resolved dependencies
changed between its two versions (the lockfile's graph, around the
workspace-hack); the workspace manifest, .config/, the hooks, the
workspace-hack or a build.rs select everything (the summary says
"EVERY TEST" and which files); Markdown and docs only `repo_checks`. A
compared commit missing from the clone (the remote moved since the last
fetch) is fetched first.

- Before every commit (`.githooks/pre-commit`): the tests of the crates
  the staged changes can affect are checked (`affected --staged`; `cargo
  check -p <crates> --features dev --all-targets`: type-checked without
  codegen or linking; `CHIMERA_COMMIT_DEPENDENTS=build` builds them
  instead), and the fast tier (unit tests and everything outside `heavy`)
  runs for the crates whose own files changed and `repo_checks`
  (`affected --staged --direct`; a change that selects everything runs only
  `repo_checks`). The dependents' tests run after the push. `CHIMERA_COMMIT_TESTS=affected` runs every affected
  crate's fast tier (the hook before 2026-10-27), `=all` the whole fast
  tier (`cargo test --features dev -- --skip heavy::`, about 90 s warm),
  `=0` none. In an agent checkout (`.claude/worktrees/*`,
  `.claude/slots/*`) the hook builds with `tools/agent-env.sh`'s settings,
  as the agent does (section 3e), so it reuses the agent's build.
- Before every push, `.githooks/pre-push` checks the commits' identity
  (UTC, the repository's author) and runs `cargo test -p repo_checks`.
  Nothing slower: push first, test after (below).
- Before every push, `.githooks/pre-push` refuses personal traces in what's
  pushed (commit messages and added lines): machine paths (`/home/<name>/`,
  `/Users/<name>/`, `C:\\Users\\<name>`) always, plus the extended regexes in
  the git-ignored `.githooks/private-patterns` (one per line; e.g. real
  names), so the names themselves never reach the repository.
- After every push, `tools/post-push-tests.sh` (run it right after the
  push; it tests the remote's main in its own warm checkout,
  `.claude/slots/post-push`, so main's working tree isn't touched)
  checks the build without
  `--features dev` (`cargo check --workspace --lib --bins --features tools`;
  the playtest and release builds use that configuration and the tests
  don't), even with `CHIMERA_PUSH_TESTS=0`. The debugging binaries are
  built only with `--features tools` (so `cargo test` doesn't link them).
  It also checks the dedicated servers as released, alone and so without
  the presentation (`cargo check -p cs_source_server -p
  combat_arms_server`; `architecture::server_builds_without_presentation`
  checks their dependencies).
- After every push (`tools/post-push-tests.sh [base]`): the tests,
  heavy ones included, of the crates the push can affect (`affected
  --range <base>..<pushed>`; `base` defaults to the last commit the script
  passed, else the pushed commit's first parent; it prints the selection
  and why). `CHIMERA_PUSH_TESTS=all` runs the full suite, `=0` no tests
  (the build checks still run). The heavy tests that need the games'
  installs run there too: the script points `CHIMERA_CONFIG` at the main
  checkout's `chimera.local.toml` (or `mashup.local.toml`) when it has one
  and the caller set none (the post-push checkout has no local config of
  its own, and those tests used to skip there with a note, hiding their
  failures). It writes `.claude/post-push/<commit>.log`
  (`last.log`) and a one-line `PASS`/`FAIL` summary to
  `.claude/post-push/last.txt` (and `history.txt`), exits non-zero on a
  failure, and trims its checkout's `target/` to 40 GB with `cargo sweep`.
  The pre-push hook trims the clone's own `target/` to 150 GB (install
  `cargo install cargo-sweep`; skipped without it). The full
  suite is `cargo nextest run --features dev` (at most 8 tests at once
  and 5 heavy slots, the biggest heavy tests taking 2-3,
  `.config/nextest.toml`: memory, not CPU, is the limit with other
  builds running; a heavy test that loops over maps belongs split into
  one test per map, as `map_courses`, `map_surf` and `map_ladders` are,
  or per archive, as Combat Arms' `heavy::world::keyed_maps_*`; a hung
  test is killed after 5 minutes, a heavy one after 12, the soak after
  30), or
  `cargo test --features dev` without nextest (twice as slow: in one
  process the heavy tests contend). nextest runs each test in its own
  process; install it once with `cargo install cargo-nextest --locked` (or
  its prebuilt binary, https://nexte.st); without it the script runs whole
  crates through `cargo test`. It accepts the same `-- --skip heavy::`
  filter. It doesn't run doctests (there are none); `cargo test --doc`
  does.
- One file: `cargo test -p cs_source --features dev --test it
  weapons::`; one test: `cargo test -p cs_source --features dev
  --test it weapons::name` (a front end's: `--features dev,ui --test
  ui`); `-- --ignored` runs
  the ignored debug aids (filters work the same).

## 2. Screenshots

```
cargo run --features dev -- --screenshot shot.png --frames 90 \
    --spawn 0,1,12 --look 30,-10 --movement chimera:noclip
```

With no `--map` the game starts at CS:S's main menu (the greybox loaded
unseen behind it), unless `--spawn`, `--look`, `--movement` or `--views`
is given: those start playing on the greybox, as above. `--map greybox`
starts on the greybox too. So `--window 1280x720 --screenshot menu.png`
photographs the main menu.
Add `--map cs_source:de_dust2` to load a real map (noclip plus
`--spawn x,75,z --look 0,-89` gives a top-down view). `--lightmap-only`
renders surfaces white so only baked lighting shows: misoriented lightmaps
appear as shadows that break at face edges.

Joining: a game started on a map shows CS:S's join flow (the message
of the day, the team menu, the class menu; `games::cs_source::ui::join_menus`) and the
player has no body until it picks, but `--screenshot`, `--views`,
`--spawn` and `--look` runs join the smaller team at once, as do
`+jointeam <n>` (that team) and `+spectate`; `--join menus|auto|1|2|3`
says which outright (e.g. `--join menus` to photograph the panels: the
message of the day opens at the start, `+chooseteam` the team menu, and
`+jointeam 2` after it the class menu). Live runs over the remote
protocol that expect a player on a team: add `+jointeam 3`.

Runs 90 frames, saves the primary window, exits. Read the PNG to check
rendering. Write screenshots to a scratch directory, never into the repo.
Needs a display: on the Linux dev box set `WAYLAND_DISPLAY=wayland-1` and
`XDG_RUNTIME_DIR=/run/user/1000` if the shell lacks them. `--frames N`
without `--screenshot` just runs N frames and exits (smoke test).
`--scale 1.25` gives the window that scale factor (Windows at 125 %: UI
placed in logical pixels, e.g. the crosshair after a dialog; with
`--window` it is 1 unless given). `--window 1920x1080` fixes the window's size in pixels (a fixed-size window
floats under Hyprland instead of being tiled), for UI checked at a known
resolution. `--help` lists all options.

System order: `CHIMERA_SYSTEM_DUMP=<file>` builds the game (either app:
`client::app::run`) as it would run (with the same options and plugins; needs the display) and writes
every schedule's systems in the order they run, the main world's then the
render world's (taken before pipelined rendering moves it to its own
thread), to the file instead of running. Dump before and after a
refactor that moves the client's plugins or systems and `diff` the files
(names differ by their module paths); also with and without an added
system, to see which others it reorders (the single-threaded executors
run unordered systems in whatever order the graph sorts to).
`CHIMERA_AMBIGUITY=1` with it logs the `Render` schedule's ambiguous
pairs (systems with conflicting access and no order; Bevy exempts its
own shared render resources): one today, ours (`frame_metrics`'
`render_start` and `apply_extract_commands`). The simulation's own order on real
maps is `games/cs_source/tests/it/heavy/map_spawn_dump.rs` (`CHIMERA_SPAWN_DUMP`; `CHIMERA_SPAWN_DUMP_ONLY=<map>` dumps one map; `CHIMERA_SPAWN_DUMP_PREPARED=1` spawns each with its parts prepared ahead, `map::prepare`, as the client's background load does: the dumps must equal the plain ones; and the run prints the process's resident MB after the load, the memory check for map-lifetime changes). What `unload_map` must leave of a map is `chimera/present/tests/it/map_unload.rs`: a resource a spawn phase inserts needs `map_scoped` or it fails there.
Each game's `tests/it/schedules.rs` ratchets the simulation's order: per
fixed and `Predict` schedule, its dedicated server's and its game's
ambiguous pairs, and the conflicting pairs the two run in opposite orders
(prediction's at zero); a count that rises fails with the pairs to order.

To photograph a live run step by step (e.g. before and after a round
restart, where frame counts are too uncertain), run the game without
`--screenshot` and send `screenshot <file.png>` over the remote console
(section 3b) between other commands: `setpos ...; setang ...`, `+attack`,
`-attack`, then `screenshot /scratch/after.png`.

Ports: tool runs (`--screenshot`, `--views`, `--frames`, `--bench`, so
also refcmp and mapsweep) never bind UDP 27015, so several agents can
capture at once beside a real CS:S. They start with `ip 127.0.0.1;
hostport 0`: a run that hosts (`+maxplayers 4 +map ...`) listens on
loopback on a port the system picks (`status` shows it; `-port <n>`
still names one), and one that doesn't host opens no game socket at all
(single player). With `dev` their remote protocol (section 3) takes a
free loopback port too, logged as `remote protocol on 127.0.0.1:<port>`,
unless `CHIMERA_REMOTE_PORT` names one. Headless tests use the in-memory
link or loopback ports the system picks. Check with `ss -lunp` / `ss
-ltnp` while runs are going.

Config: tool runs never write the user's `config.cfg` on exit (the one
seam is `Args::tool_run()`, checked by the client's exit save, so every
app). A normal session does. An explicit `host_writeconfig` still writes;
`CHIMERA_CFG_DIR=<dir>` points the config folder (read and write) at a
throwaway one.

## 3. Live ECS over HTTP (Bevy Remote Protocol)

With `--features dev`, the game serves JSON-RPC on `localhost:15702`
(tool runs: a free port, section 2).

```
curl -s localhost:15702 -d '{"jsonrpc":"2.0","id":1,"method":"world.query","params":{
  "data":{"components":["chimera_core::core::MovementState","chimera_core::core::Velocity"]},
  "filter":{"with":["chimera_core::core::LocalPlayer"]}}}'
```

```
curl -s localhost:15702 -d '{"jsonrpc":"2.0","id":2,"method":"world.get_resources",
  "params":{"resource":"chimera_core::core::SimTick"}}'
```

Other methods: `world.list_components`, `world.get_components`,
`world.mutate_components`, `world.list_resources`. Don't try to drive the
local player by mutating its `Intent`: local input rewrites it every frame
(see docs/tech-debt.md). Use a scenario test instead.
Only `Reflect`-registered types are visible; register new core components in
`CorePlugin`.

## 3b. The console from outside

- Command line, Source style: `cargo run --features dev -- +sv_airaccelerate
  150 +cl_showpos 1 +bind f noclip` runs those console commands at startup;
  `--console` starts with the console open (screenshots of it).
  `++attack` (one `+` for the command line, one for the action) holds an
  action from the start, e.g. to fire in a `--screenshot` run; held
  console actions work without mouse capture. Automated runs never write
  config.cfg, and only archived cvars are saved there.
- What a client's console commands print (results, errors) goes to the
  log too, as `INFO console: <command>: <output>` (errors as `<command>:
  error: <text>`; `Console::log_output`, set by the client shell). Before
  that only the in-game console showed it, so a `+minigame x` that failed
  left nothing for an agent to read. `grep 'console:'` the run's output
  for a `+command`'s result. The dedicated servers print it on stdout.
  A startup test of an app's command line: `apps/lucker_party/tests/it/
  startup.rs` (parse, `with_defaults`, the lines on the simulation).
- As CS:S's (checked against reference captures), `getpos` prints the
  view's origin (the eye: standing on a floor at z it prints z + 64,
  ducked z + 47; noclip keeps the offset) and `setpos` takes the player's
  origin, Source's: the feet. So the same `setpos` line puts our camera
  where CS:S's was (64 above its z standing), and `setpos` with `getpos`'s
  numbers lands a view offset higher, in both games.
- `menu [main|newgame|game|bot|bots|team|options|mpadvanced|extras|players]` opens the
  game menu (Esc) on a page, for screenshots of it: `--window 1280x720
  --screenshot menu.png +menu options`; `menu newgame` is Create Server
  (`game`, `bot`: its other pages), `menu keyboard` (or `mouse`, `audio`,
  `video`, `voice`, `multiplayer`) opens the options on that tab, `menu
  advanced` the keyboard tab's Advanced dialog, `menu videoadvanced` the
  video tab's, `menu gamma` its brightness dialog (with
  `+mat_monitorgamma 1.6` or `2.6` before it: the whole frame lighter or
  darker), `menu mpadvanced` the multiplayer tab's (its list from the
  install's `cfg/user.scr`), `menu extras` Lucker Party Options, `menu bots` and `menu team` our Bots and Team dialogs, `menu players` the player list (mute); the menus list only CS:S's entries, so these (and `chimera_quickstart`: rounds on de_dust2 with bots; `map greybox`; `callvote` alone for the vote setup dialog) are how ours open. `menuinput <input>` drives
  the open dialog as keys would (`focus <cvar|map|bots|botcount|botteam|
  difficulty|aspect>`, `row <n>` (the page's nth row), `open` a focused combo box's list, `down`, `up`, `enter`,
  `space`, `escape`, `tab`, `backtab`, `type <text>`, `pick <n>`, `sheet
  <n>`, `scroll <rows>` the open list as the wheel) and prints the focused
  control: a drop-down open in a screenshot is `+menu newgame +menuinput
  open` (after `+menu`, `+wait 10` before inputs that need the page drawn).
  `ui_click <x> <y>` and `ui_drag <x0> <y0> <x1> <y1> [<frames>]` drive the
  real pointer path (Bevy's input, `Interaction`) in window pixels over the
  remote console: a scroll bar's thumb dragged, a drop-down's box clicked. `vgui_windows` lists the frames
  (place, size, stacking); `vgui_windows <frame> <x> <y> [<wide> <tall>]`
  moves one as a title-bar drag would (and sizes a sizeable one), e.g.
  `+openserverbrowser lan +wait 30 +vgui_windows servers 40 30 760 520`.
  The widget state machines (drop-down, slider, text entry, Tab order,
  frame drags and stacking) are unit-tested in `client::widgets`, the
  dialogs driven by scripted input in `games/cs_source/tests/ui/menus.rs`. Cvars it shows are read
  when it opens: put `+cl_crosshaircolor 3` before `+menu`. The log
  line `game menu: GameUI look (...)` says the install's look loaded,
  with how many main menu backgrounds and the title it found (else the
  built-in one is drawn). Its logic is unit-tested in
  `games::cs_source::ui::game_menu` (keys and clicks in, console lines out; rebinding
  with real key presses headless; which entries show in and out of a
  game; `disconnect` and `map greybox` on a world). The main menu over
  a map: `--map cs_source:de_dust2 --screenshot m.png +wait 30 +menu`;
  back to the main menu: `+disconnect`; `map greybox` plays the greybox
  from anywhere; `toggleconsole` opens the console as the menu's
  Console entry does.
- Which CS:S install a run uses: the startup log line
  `Counter-Strike: Source install: <folder> (from config <file> | saved
  in <file> | Steam library N (<folder>), build <id>)`, or `... none
  found (<why>)`; `chimera_install` prints the same, `chimera_install
  <folder>` sets and saves it, `chimera_install auto` forgets it. The
  first-run dialog (CS:S not found) without touching real files:
  `CHIMERA_CONFIG` at an empty file, `CHIMERA_STEAM_ROOT` at an empty
  folder (it replaces the usual Steam folders; a fake Steam tree there
  is searched instead) and `CHIMERA_SETTINGS` at a scratch
  `settings.toml`, then `--window 1280x720 --screenshot fr.png`;
  `+chimera_firstrun type <folder>` types into it, `+chimera_firstrun
  retry` presses Retry. Discovery is unit-tested with fake Steam trees
  (`mount::install`), and `games/cs_source/tests/it/mount_install.rs` mounts an install
  found through one.
- Fonts: the startup log line `fonts: tahoma -> tahoma.ttf, ...` says
  which file each scheme family resolved to (`(stand-in)` when the
  system lacks the real face, e.g. Liberation Sans for Tahoma on Linux;
  `client::fonts`). Text sizes and faces per element are unit-tested
  there; screenshots of the console, chat, scoreboard, buy menu, main
  menu and `debugui` at 1280x720 show them.
- Binds: `bindlist` lists them; every game key is one (`client::binds`),
  `binddefaults` puts the defaults back.
- `buymenu [n]` and `chooseteam` open the buy menu (on category n) and
  the team menu, e.g. `--map cs_source:de_dust2 --screenshot buy.png
  +wait 30 +buymenu 4` (the `wait` lets the map's HUD and menu layouts
  load first, so the game-look pages are used). Their key and button
  logic is tested headless in `games::cs_source::ui::buy_menu` and `games::cs_source::ui::team_menu`
  (the join flow's in `games/cs_source/tests/ui/join_flow.rs`);
  `games/cs_source/tests/it/heavy/map_de_dust2.rs` checks the install's layouts load.
- `ent_fire <target> <input> [value]` sends a map entity an input through
  the logic layer (names, `*` wildcards, classnames; the local player is
  the activator), e.g. `+wait 30 +ent_fire logic_timer Disable +ent_fire
  computer0* Skin 2` in a `--screenshot` run on de_nuke (the `wait` lets
  the map's logic load first). Logged at info level. `ent_dump <target>`
  prints what the logic holds for each entity it reaches (class, name,
  origin, a mover's pose, velocity and what carries it, a trigger's volume,
  what it follows, a physics body's box, and the class state: door and
  button states, relays, timers, counters, cases), entity space; e.g.
  `ent_dump el_elevator*` over the remote console while a lift runs.
- `chimera_drawsound 1` draws a sphere for each ambient_generic's audible
  radius (green while it plays, grey while silent) and each
  env_soundscape's (cyan) within 80 m of you (2: all);
  `chimera_drawtriggers 1` outlines the brush triggers' boxes, coloured by
  kind (hurt red, push blue, teleport magenta, gravity cyan, remove orange,
  multiple/once yellow, others grey; a disabled one dim; 2: all). Both are
  check boxes on `menu debug` (`games/cs_source/src/ui/debug_world.rs`).
  `fog_enable 0` turns the map's fog off in the world, prop, detail, rope,
  decal and particle materials (water fog stays) without rebuilding them
  (a debug-UI and `menu debug` check box); `fog_enable 1` brings it back.
- `chimera_drawnav 1` outlines the nav areas near you (2: all), coloured by
  place, with half-links toward their neighbours, and the mesh's ladders
  (yellow line, green cross where bots get on at the foot, orange where
  they start down from behind the top); `chimera_drawbots 1`
  shows the map's sites (yellow rings) with where routes from the
  attackers' (orange balls) and defenders' (blue balls) spawns come onto
  them, and per bot: its role (ring at its feet: orange attacker, blue
  defender, grey roaming), look direction (white) and look target
  (yellow line; a yellow ball on the hiding spot it is checking),
  target (red), last known enemy position (orange), the teammate it
  answers (green), route (cyan), goal (purple), hold spot (a box in its
  role's colour, lines to the approaches it watches), its planned
  grenade arc (red HE, pale yellow flash, grey smoke) with the target
  (green cross) and where it should go off (sphere), and a flash it is
  looking away from (white), and a teammate's radio command it carries
  out (magenta: a line to whom it follows or where it regroups or falls
  back to, a ring where it holds, and the order as text over its head,
  "following Player"). `bot_debug 1` lists each team's plan and
  every bot's team, role, site, activity (ToSite, Following, Waiting,
  Holding, Chasing, Assisting, Obeying, ...; `*` marks the group
  leader), health and radio order on screen. To watch a round on dust2 from above: `--map
  cs_source:de_dust2 +chimera_rounds 1 +bot_add 1 +bot_add 1 +bot_add 1
  +bot_add 2 +bot_add 2 +bot_add 2 +chimera_drawbots 1 +bot_debug 1
  +noclip +setpos -400 3000 1200 +setang 89 90 0`. Headless round
  statistics (winners, kills, time to first contact, and every spot where
  a bot walked a route without getting anywhere for 4 s, summed up; and
  per round whether the bomb was planted, defused or exploded and what
  each team carried at the start (primaries, helmets, kits), with deaths
  per team by place and by what the bot was doing before the fight at
  the end; `CHIMERA_BOT_CONSOLE="bot_grenades 0"` runs console commands
  first):
  `CHIMERA_BOT_MAP=de_dust2 CHIMERA_BOT_ROUNDS=8 cargo test --features dev
  --test it bot_rounds:: -- --ignored --nocapture`. Runs aren't
  repeatable (the same build gives other winners), so compare win rates
  over 30 rounds or more. To look at such a spot:
  `CHIMERA_NAV_MAP=de_nuke CHIMERA_NAV_AT=11.4,-15.2,34.4 cargo test
  --features dev --test it bot_nav::nav_near -- --ignored --nocapture` prints
  the nav areas, links, ladders, ladder brushes and entities (breakables
  with their keyvalues) around it (engine meters, as the report prints
  them; `CHIMERA_NAV_BRUSHES=1` also lists every collision brush within
  1 m, e.g. player clip flush with a ladder's face); add a case to `games/cs_source/tests/it/heavy/bot_nav.rs` (a lone bot sent from a start
  to a goal, `CHIMERA_BOT_CASE=<name>` to run one, `CHIMERA_BOT_TRACE=0`
  for every tick of its intent, ladder and ground state), and try raw
  inputs with `probe_walk` (`P_AT=x,y,z P_OPT=yaw,crouch,jump,seconds,
  pitch,forward,side`; `P_OPT2`, `P_OPT3`: further phases, e.g. climb a
  ladder, then step off its top). `nuke_ladders_climb_both_ways` sends a
  bot up and down every ladder on de_nuke's mesh; the ignored
  `ladders_climb_both_ways` does it on `CHIMERA_NAV_MAP` (comma-separated
  maps) and lists the ladders bots fail, one `CHIMERA_BOT_CASE="ladder 3
  up"` at a time (`CHIMERA_BOT_TRACE_CASE="ladder 3 up"` with
  `CHIMERA_BOT_TRACE` traces just that one while running the whole map:
  earlier cases leave stuck reports and broken grilles behind). Bot dice come from bot numbers, never entity ids:
  `CHIMERA_TEST_PAD=<n>` spawns n entities and a resource in every `Sim`
  first, and a test's outcome must not change with it (run the bot tests
  with a few values after touching bot randomness). In game, `bot_goto <x> <y> <z>` (CS:S units) sends
  every bot somewhere; `bot_goto` alone lets them go back. `bot_grenades 2` makes bots throw whenever
  an arc works (`chimera_watch 1` to follow one). On the greybox map:
  `--spawn 0,1,-5 +bot_dont_shoot 1 +bot_stop 1 +bot_grenades 2 +bot_add 1
  +bot_give weapon_hegrenade +chimera_drawbots 1 +chimera_watch 1
  +cam_idealdist 600 +cam_idealyaw 70 ++attack --frames 130` shows the bot
  hearing you behind the long crate and lobbing an HE over it.
- Spectating: `chimera_hurtme chest 500` (in a rounds game, `+chimera_rounds 1`) starts the
  death cam; 6.5 s after the death (no freeze cam) the camera watches a
  living teammate (CS:S's timings: specs/cs_source/spectating.md).
  `chimera_hurtme chest 500 0 <name>` is a hit from that player: its
  death cam turns to it, then 2 s after the death the freeze cam (0.7 s
  toward it, the picture held 5 s with the freeze sound and panel;
  `cl_disablefreezecam 1` skips it, the server's
  `spec_freeze_traveltime`/`spec_freeze_time` time it;
  `games/cs_source/tests/ui/spectate.rs`). The target ID (the name under the
  crosshair; `hud_centerid`, `mp_playerid`) is the `TargetId`
  resource (`games/cs_source/tests/ui/target_id.rs`). Drive it
  with `spec_mode 4|5|6` (first person, chase, free look), `spec_next`
  and `spec_prev`, `mp_forcecamera 0` to watch enemies too; the state is
  the `Spectator` resource (`games/cs_source/tests/ui/spectate.rs` drives it headless).
  The spectator bars on a real map: `--map cs_source:de_dust2 --window
  1280x720 --screenshot spec.png --frames 400 +chimera_rounds 1
  +mp_freezetime 0 +bot_add 2 +bot_add 2 +wait 60 +chimera_hurtme chest
  500` (the death cam ends 6.5 s after the death, then a teammate is
  watched in first person, with their crosshair). The spectator team:
  `+spectate` (or `+jointeam 1`) flies free at once; `spec_player <name>`
  watches one; pressing duck opens the spectator menu; the scoreboard lists
  spectators (`games/cs_source/tests/ui/spectate.rs::the_spectator_team`, over the
  network `games/cs_source/tests/ui/net_rules.rs::a_client_spectates_then_joins_a_team`).
  `spec_menuinput <options|players|view> [entry [subentry]]` opens the
  spectator menu's drop-down (screenshots) or picks an entry (0 first):
  `--map cs_source:de_dust2 --window 1280x720 --frames 400 --screenshot
  s.png +wait 30 +bot_add 1 +bot_add 2 +spectate +wait 120 +overview_mode 1
  +spec_menuinput options 2` shows Options with its Overview submenu over
  the small map (`overview_mode 2`, `overview_zoom 2`: the large one);
  `games/cs_source/tests/ui/spectate.rs::the_spectator_menu_picks_the_camera_player_and_overview`.
- Scoreboard: `++showscores` holds it from the start (`--map
  cs_source:de_dust2 --window 1280x720 --screenshot sb.png +bot_add 1
  +bot_add 2 ++showscores`); a dead player shows the skull icon,
  the bomb carrier the bomb to Ts, a kit owner the defuser to CTs.
- Loading dialog: from the main menu, `+menu newgame` and Start Game, or
  over the remote console while at the main menu, `map de_dust2` then
  screenshots while it loads (the bar follows `map::loading`).
- `chimera_objectives 1` prints the objectives' state on screen: who
  carries the bomb, a planted bomb's place, site and time left, a defuse's
  progress (who, kit, seconds), the outcome, and each hostage's health,
  leader and position. To plant in a screenshot: `--map
  cs_source:de_dust2 --spawn 29.46,3.5,-62.99 +give cs_source:weapon_c4`
  (de_dust2's A site), then `+attack` over the remote console; hostages
  appear with `mp_freezetime 0; chimera_rounds 1` on a cs_ map.
- `chimera_drawhitboxes 1` outlines other characters' hitboxes, coloured
  by hitgroup, where shots test them (e.g. `+bot_stop 1 +bot_add 2
  +chimera_drawhitboxes 1` with `--screenshot` to check they follow the
  animated body).
- `chimera_debughud 1` shows the top-left debug readout (movement mode,
  speed, ground/crouch state and the debug keys); off by default.
- Debug keys: default binds on keys CS:S's `config_default.cfg` leaves
  free (the rest are CS:S's: F1 autobuy, F2 rebuy, the wheel
  `invprev`/`invnext`, ...; `client::binds`): F3 collision shapes
  (`toggle chimera_drawcollision`), F8 the debug UI (`debugui`, section
  3d), F11 the entity inspector (`chimera_inspector`; frees the mouse),
  V `noclip`, F9 (and CS:S's F4) a bug report, Left Alt free look. All
  are console lines, so `bind` moves them and Options > Keyboard lists
  them.
- `chimera_drawphys 1` outlines physics props by state (green moving,
  blue asleep, grey still or frozen; multiplayer props darker) and
  players' physics shadows (white, a yellow line to each prop the shadow
  touches), and lists the props nearest the local player (distance, mass,
  push mode, state) on the debug HUD.
- `chimera_ragdoll_debug 1` draws ragdoll bodies (bounds, axes) and
  joints (yellow to the anchor on the parent; a red dot when the child
  drifted from it). `cl_ragdoll_physics_enable 0` turns ragdolls off,
  `ragdoll_sleepaftertime` sets when still ones are frozen. To see one:
  `--map cs_source:de_dust2 --frames 400 +mp_respawn_delay 1000
  +thirdperson +cam_idealyaw 90 +wait 150 +chimera_hurtme head` (or
  `chest 100 90`: a body shot from the side) with `--screenshot`. The
  log line `ragdoll ... (death pose Some((Front, 1)))` names the death
  pose. To shoot a body after death, run live with `+bot_stop 1
  +bot_dont_shoot 1 +bot_add 1`, find the bot with a `world.query` of
  `Transform` on `chimera_core::core::Intent` entities, `setpos` near it, and
  tap `+attack`/`-attack` over the remote console between
  `screenshot`s.
- Flashbang look (white plus the frozen after-image): run live on
  de_dust2 with `+mp_freezetime 0 +god`, then over the remote console
  `give cs_source:weapon_flashbang`, `setang 35 255 0`, `+attack`,
  `-attack` a second later, and `screenshot`s from 1.7 s after the
  release; a `setang` to another direction after the flash shows the
  frozen frame over the new view. Hearing (muffle, ringing) can't be
  photographed: `audio::hearing::Hearing` and its tests
  (`cargo test --lib hearing`) show the curves; `games/cs_source/tests/it/cs_grenades.rs`
  checks which effect each blast or flash gives.
- `chimera_healthbars 1` draws a health bar over every other living
  character (green full, red nearly dead), e.g. to watch damage land in a
  `--screenshot` run with `+bot_add 2`.
- `chimera_particles` prints the live particle groups and counts (and how
  many count against the 2048 cap), e.g. over the remote console after
  shooting. Impact effects obey `r_drawflecks`, `cl_show_splashes` and
  `violence_hblood` (0/1). To see a shot's effects in a `--screenshot`
  run, fire with `++attack` and keep `--frames` low enough that they are
  still alive (dust lives about a second, sparks a tenth of one).
- **Bug reports while playtesting:** press F4 or F9 (or `bugreport [note]` in the
  console). It saves `screenshot.png` and `report.txt` (build version,
  map, a `setpos`/`setang` line to paste back, weapon, health, the last
  40 console lines) to `<data dir>/chimera/bugreports/report-<time>/`
  (`~/.local/share` on Linux, `%LOCALAPPDATA%` on Windows) and prints the
  path.
- `version` (console, client and server) prints the build: tag or `git
  describe`, short commit hash and the commit's UTC date, e.g.
  `v0.1.0-playtest3 (feac156, 2026-10-11)` (no tag: `feac156,
  2026-10-11`; `-dirty` on the hash with uncommitted changes), then the
  network protocol (`NET_VERSION`). Each app's build.rs stamps it
  (chimera/core/build_stamp.rs; `core::VERSION`); the main menu shows it bottom right and
  `cs_source_server` prints it at startup.
- `thirdperson` (back with `firstperson`; distance `cam_idealdist`, CS:S
  units) puts the camera behind the local player and draws its own
  animated body: `+thirdperson` in a `--screenshot` run shows the local
  player's model and animation. `cam_idealyaw` orbits the camera
  (degrees; `+cam_idealyaw 140` looks at the player's front, e.g. to see
  how the hands hold the weapon). Holding Left Alt (`+freelook`) turns
  only the camera; the aim stays put. `chimera_hand_ik 0` draws bodies
  without the hands locked onto the weapon (the animation's own arms),
  to compare: `+setpos 185 300 40 +setang -30 0 0 +thirdperson
  +cam_idealyaw 130 +cam_idealdist 60` on de_dust2 with a pistol.
- `chimera_freecam 1` detaches the camera and flies it (WASD, mouse,
  space/ctrl up and down, shift faster) while your body stands still and
  is drawn; `chimera_freecam 2` leaves the camera where it is and gives
  the controls back, so you can walk, crouch and shoot in front of it
  (watching your own animation and held weapon); 0 returns to the eye. In
  a `--screenshot` run: `+thirdperson +cam_idealyaw 180 +wait 20
  +chimera_freecam 2 +wait 3 +firstperson +wait 20 ++forward` films you
  walking toward the camera.
- `chimera_watch <n>` chases bot n from behind (`cam_idealdist`,
  `cam_idealyaw` apply; 0 returns): `+bot_add 2 +chimera_watch 1
  +cam_idealyaw 150` shows a bot running with its weapon.
- View model (CS:S cvars): `viewmodel_fov` (54 as CS:S, kept within 54-110, saved; `ca_viewmodel_fov` is Combat Arms', 63.74-110; the party has both, each for its game's players), `cl_righthand` (1;
  0 puts weapons in the left hand), `r_drawviewmodel 0` hides it,
  `cl_bobcycle`/`cl_bobup` (bob), `cl_wpn_sway_interp`/`cl_wpn_sway_scale`
  (sway: e.g. 20 to exaggerate it), `muzzleflash_light 0` turns the
  flash's light off, `cl_ejectbrass 0` stops shells. The first two and
  `muzzleflash_light` are archived. To check the flash lighting a wall,
  stand at one and fire: `+setpos 470 2330 -78 +setang 0 15 0 ++attack`
  on de_dust2 with a few `--frames` values (a flash lasts 0.05 s, every
  0.1 s), against the same without `++attack`. `dump cs_source
  --sequences <model>` shows a model's animation events and attachments,
  and each sequence's motion (its largest bone turn from the first frame
  and over which cycles; for an idle, how far its first frame is from
  the draw's end): which idles visibly move (the knife's, the TMP's).
- Water (CS:S cvars): `r_WaterDrawReflection 0` / `r_WaterDrawRefraction
  0` turn the planar reflection / refraction off (to tell which one an
  artefact comes from), `mat_drawwater 0` hides water surfaces,
  `r_waterforcereflectentities 1` reflects models too (CS:S's "Water
  detail": Simple reflections = `r_waterforceexpensive 0` and
  `r_waterforcereflectentities 0`, Reflect world = 1 0, the default,
  Reflect all = 1 1). Under water the
  material's screen warp shows (de_port: `+setpos 700 2700 235 +setang
  -5 140 0` with `+noclip +god`); with the eye a few units above a surface
  (`+setpos 700 2700 258 +setang 20 140 0`) the strip under the waterline
  shows the water's height fog.
- Shadow and model detail (Video > Advanced; CS:S cvars, at once):
  `r_shadowrendertotexture 0` swaps props' silhouette shadows for blobs
  (`r_shadows 0`: none; the log line `prop shadows (Blob): N casters
  reach the world`); characters' blobs show in third person
  (`+thirdperson +setang 45 120 0`). `r_rootlod 2` (Low) draws props'
  coarsest levels, `r_lod n` forces one; de_dust2's pots at `+noclip
  +setpos 160 2760 60 +setang 10 0 0` show it (`magick compare` of two
  shots finds the props that switched). The map's log line `props: ...;
  N have levels of detail` counts the models with levels; `dump --lods`
  lists each model's.
- Pickups: `chimera_debug_pickup 1` prints (console and log) why the
  loose weapon nearest the local player is or isn't taken, once per
  change: `pickup Player: cs_source:weapon_ak47: dropped 0.4 s ago,
  touchable from 1 s`, `out of reach, 0.31 m from its touch box`, `slot
  full (cs_source:weapon_m4a1 carried; ...)`, `not in sight from the eye,
  blocked by <entity>`, `only the other team takes it`, `taken`. `2`
  does it for every character (a server's remote players and bots);
  pickups are the server's, so on a network client set it on the server.
  A negative "dropped ... ago" means the drop and the tick disagree on
  the time (the bug this was added for).
- Remote: `curl -s localhost:15702 -d '{"jsonrpc":"2.0","id":1,
  "method":"chimera/console","params":{"line":"getpos; cvarlist sv_"}}'`
  runs a line now and returns the lines it printed (`mashup/console`, its
  old name, still works).
- Old names (before the rename to Chimera) still work, hidden: `mashup_*`
  cvars and commands run their `chimera_*` ones (configs and binds saved
  with them are written back with the new names), `MASHUP_*` environment
  variables are read when their `CHIMERA_*` one isn't set,
  `mashup.local.toml` when there is no `chimera.local.toml`, the per-user
  `mashup` folder while there is no `chimera` one, `--movement
  mashup:noclip` and `map mashup:greybox`.
- Configs live in `~/.local/share/chimera/cfg` (Windows `%APPDATA%\chimera\cfg`):
  config.cfg (written on quit when binds/cvars changed; every bind, as
  `bind` lines after `unbindall`), autoexec.cfg,
  history.txt, bookmarks.txt (the debug UI's places); `exec name` runs
  name.cfg from there.
- Typing in the console: suggestions show under it as you type, with a
  help line for the command being typed (its arguments from the help
  text, the one being typed in green; a cvar's value, default, range and
  values). Up/Down pick a suggestion (with an empty line they browse
  history), Enter or Tab takes it and adds a space, Esc hides the list
  (a second Esc closes the console). Completion knows each command's
  arguments (keys and what they are bound to, maps, `ent_fire` targets
  and inputs, teams, hitgroups, debug UI tabs, bot numbers, weapons) and
  works on the last command of a `;` line. `help <name>` prints the
  same; Ctrl+Backspace / Ctrl+Delete delete a word, Ctrl+V pastes,
  Ctrl+L clears, Ctrl+Home / Ctrl+End jump through the output. Its
  logic is tested headless in `client::console` (keys typed into the
  editing system). For a screenshot of it, `con_input "<text>" [n]`
  opens the console with that input and suggestion n picked:
  `--window 1280x720 --screenshot con.png +con_input "sv_a" 2`.

## 3b2. Network play

Plan and progress: [plans/active/multiplayer.md](plans/active/multiplayer.md).
The default port is UDP 27015 (Source's). **On the Linux dev box a CS:S
server holds 27015: never bind it there.** Use another port for every run
(`-port 27016`, `hostport 27016`); tests use ports the system picks, and
tool runs host on loopback on one (section 2). `ip <address>` (Source's)
is the address a server binds: every interface (0.0.0.0) by default.

Headless first: `harness::NetSim` runs a server and N clients in one
process over an in-memory link (`net::memory`) with seeded latency, jitter
and loss, stepped tick by tick (`games/cs_source/tests/it/net.rs`):

```rust
let mut sim = NetSim::new(LinkConditions { latency: Duration::from_millis(30), ..default() }, 1, 2, |app| {
    app.add_plugins((GreyboxMapPlugin, SourceMovementPlugin)).insert_resource(Loadout { movement: source::ID });
});
sim.until_joined(200);
let me = sim.local_player(0).unwrap();          // client 1's own character
sim.clients[0].app.world_mut().get_mut::<Intent>(me).unwrap().move_axis = Vec2::Y;
sim.ticks(64);
let a = sim.character_of(0).unwrap();           // the server's copy
```

`net::status(world)` is the `status` text; `net::LastDisconnect` says why
a game ended. `netcode_over_loopback` covers the real UDP transport.
`NetSim::set_conditions(i, ...)` gives client `i` a link of its own (a
mix of good and bad links on one server), `remove_client(i)` makes it
leave (ids are never given out again, as real clients' random ones
aren't); the link counts the data bytes it carried to and from each
client (`Link::bytes_to`, `bytes_from`: bandwidth per client).

Lucker Party's gun game over the network (`apps/lucker_party/tests/it/
net_gun_game.rs`: a server and two clients that only `connect`, the ladder
climbed to the win, the HUD, guns and match state read on each client).
Live on the dev box (the game needs `WAYLAND_DISPLAY=wayland-1
XDG_RUNTIME_DIR=/run/user/1000`; never 27015): a server `lucker_party_server
-port 28930 +maxplayers 4 +MaxPlayer 0 +sv_cheats 1` (commands on its stdin),
two clients `CHIMERA_REMOTE_PORT=15711 lucker_party --window 960x540 +name
Alice +connect 127.0.0.1:28930` (then 15712, Bob), and over each client's
remote console (section 3b): `screenshot`, `getpos`; a client's `setpos`
moves its player on the server, `setang` doesn't (its own input aims), so
to shoot, `setpos` Alice a few units south of Bob (`getpos` on Bob) and
`+attack` while she faces +Y. A listen server is `lucker_party -port 28931
+maxplayers 4 +MaxPlayer 1` (the app hosts once its `--map` is loaded and
the command line has run: `chimera_host_if_asked`). `minigame_load <file>`
on the server and on the clients, then `minigame <id>` on the server only,
tries a shorter ladder (a client can't start minigames: it plays the
server's, `minigame_net`; a client without the definition gets the
server's in the match manifest). `change_match <id> [map]` on the server
switches minigame with everyone connected. Party networking's NetSim
tests (manifest, mount doctor refusals, `ChangeMatch`):
`apps/lucker_party/tests/it/net_party.rs`; a client's
`net::manifest::ServerManifest` is the manifest it plays, its
`LocalMounts` the report it sent (the log's `mount doctor:` lines);
`party_offer` lists what every connected player can play (the server
names who lacks what for the rest; a client's `minigame_net::PartyOffer`). `grep 'console:'` the clients' output; the
server prints on stdout.

Cross-game parts in the party (`chimera_sim::weapon::impact`): which
game draws a gun's impacts is `ImpactRoute::by_weapon` (the map's
`map::impact::ActiveImpacts` says whose looks it carries); the scenario
tests are `apps/lucker_party/tests/it/cross_game.rs` (pushes, recoil at a
swap) and `heavy/impacts.rs` (each gun on each game's map leaves one
game's impact); the recoil at a swap over the network is in
`net_gun_game.rs`. Live: `lucker_party --map combat_arms:warehouse
+minigame originals:gun_game_dust2` holds CS:S's Glock first: shoot a wall
and `screenshot`.

Votes (`games/cs_source/tests/it/net_votes.rs`): the server's
`net::vote::Votes` has the running vote (voters, ballots) and the passed
and failed counts, each side's `VoteView` what it shows (the vote,
its end, a refused call). To photograph the panels alone on a listen
server: `-port 27016 --window 1280x720 --join 3 +maxplayers 4 +map de_dust2
+sv_vote_holder_may_vote_no 1 +wait 300 +callvote restartgame` (the vote
stays open; add `+wait 60 +vote option1 +wait 60 +screenshot <file>` for
the passed panel). The vote setup dialog: `+wait 2000 +callvote +wait 60
+menuinput row 1 +menuinput space +menuinput row 4 +menuinput space`
(Change Level, then the second map; `menuinput row <n>` focuses the
page's nth row); a joiner's and bots' votes: `net_votes.rs`.

Prediction (`games/cs_source/tests/it/net_prediction.rs`): a client's `net::predict::NetGraph`
counts the server states it compared (`checked`), the ones that differed
from its prediction (`errors`, `last_error`/`worst_error` in m, which
components differed in the last one: `last_differing`, at server tick
`last_error_tick`) and restarts (`resyncs`: nothing predicted to compare,
or the server let in, spawned or killed the player, `OwnState::alive`;
so a test needn't reset the counters after joining); `CommandClock` has the command tick and the lead
the server reports; the server's `net::server::CommandBuffer` on a
player's character has its queued commands, `missed`, `late` and `early`
counts. `PredictionHistory` holds the client's predicted ticks not yet
confirmed: compare them with the server's state at the same tick
(`PredictedComponents::encode`), never the two worlds' present (the
client runs ahead); `PredictedComponents::parts` splits a state blob by
component to see what differs (the soak's `CHIMERA_SOAK_DIAGNOSE`
decodes positions and velocities). `cargo test --features dev --test it net_prediction
-- --nocapture` prints the numbers per latency, jitter and loss.

**Soak** (`games/cs_source/tests/it/heavy/net_soak.rs`, multiplayer.md "Soak"): a
server on de_dust2 with four clients on their own links (0, 50, 150 ms
with 2 % loss, 300 ms with 5 % loss; one leaves, another joins), bots
filling to twelve, scripted players (walking, shooting, buying,
grenades, drops, the bomb planted and defused, chat, radio, names) and
three map changes (greybox, a community map from the content cache,
de_dust2), checking all along: settled values the server's on every
client, others' bodies as the server had them at that tick, prediction
errors by kind (contacts with others and hits are the server's to say;
runs of errors in a row are bugs, and so is any of a player's position
alone below the physics' 0.01 mm tolerance, alive or dead: the physics'
copy of the position had a say), entity counts per round and map
visit, memory, bandwidth and the server's frame time; Bevy errors
(failed commands) are counted too. `CHIMERA_SOAK_MINUTES` (3 by default,
30 for the long run), `CHIMERA_SOAK_COMMUNITY=<map>`,
`CHIMERA_SOAK_UPDATERATE`/`CHIMERA_SOAK_RATE` (the clients' `cl_updaterate`
and `rate`; the bandwidth budget follows the update rate; it prints each
client's updates sent and choked, and the own state's delta and whole
sizes), `CHIMERA_SOAK_DUMP=<file>` (what the server sends each client per
frame, to try compression on);
`CHIMERA_SOAK_DIAGNOSE=1` prints what differed in the first prediction
errors of each kind and in each long run of them,
`CHIMERA_SOAK_TRACE_HEALTH=1` every health change on the server and each
client by step. It prints the summary tables (`cargo test --features dev
--test it heavy::net_soak -- --nocapture`; ~1 real minute per game minute
in the dev build).

**Capacity** (`games/cs_source/tests/it/heavy/net_capacity.rs`, multiplayer.md
"Capacity"): a de_dust2 server full of players (32: a few `NetSim`
clients, bots for the rest; deathmatch, so everyone is alive and
moving) measured for 20 game seconds after a 10 s warm-up: the server's
CPU time per tick summed over the process's threads while its frame
runs (`metrics::AllThreads`; the clients run after it), p50 and p99,
and instructions per tick (stable under load: compare those between
runs), its main thread's share, bytes a second to each client, each
client's CPU per frame (headless), memory, the share of character pairs
in each other's PVS (what Source-style culling would keep), updates
choked by each client's `rate`. It asserts, at the default size, the
server's instructions a tick (p50 and mean under 35M, p99 under 75M;
~28M in the dev build, within 5% from load 1 to 5 per CPU), CPU time
p50 under two thirds of the 66-tick budget and p99 under it only while
the 1-minute load average stays under half the CPUs (CPU time a tick
grows with load: 6.5 ms at 1.2 per CPU, 10.5 ms at 3.8, same
instructions) or where there are no hardware counters (Windows: the
mean), and under 40 KB/s a client. Instructions count the whole
process, so the test runs alone in one: nextest gives each test its
own; under `cargo test` it runs itself again in a child process
(`CHIMERA_CAPACITY_CHILD`). Bigger runs print the same table:
`CHIMERA_CAPACITY_PLAYERS=64 CHIMERA_CAPACITY_CLIENTS=8
CHIMERA_CAPACITY_MAP=mg_jacks_multigames_v1 CHIMERA_CAPACITY_SECONDS=30
cargo test --profile playtest --features dev --test it heavy::net_capacity
-- --nocapture` (each client world takes ~0.8 GB: keep clients to a
handful; the per-client cost is the difference between runs).
Schedules run single-threaded as the game and the dedicated server do;
`CHIMERA_EXECUTOR=multi` measures Bevy's default executor. A
`--features profile` build with `CHIMERA_CAPACITY_CLIENTS=0
TRACE_CHROME=<file>` traces the server alone for `tracesum` (it shows
some systems only: the bots' and many of ours don't appear; with
`ptrace_scope` 1, gdb started on the test binary and interrupted every
fraction of a second, collecting the test thread's stacks, found what
the trace hid). A live
check of the real binary: start `cs_source_server -port 27141 +map de_dust2
+maxplayers 32 +chimera_rounds 0 +bot_join_after_player 0 +bot_quota_mode
fill +bot_quota 32` and read its CPU time (`/proc/<pid>/stat` fields 14
and 15) and RSS over a minute.

A live soak (real processes, this box): `cs_source_server -port 27111 +map
de_dust2 +maxplayers 12 +chimera_rounds 1 +bot_quota_mode fill +bot_quota
10 +bot_join_after_player 0 +sv_cheats 1` with its stdin on a fifo
(`changelevel` mid-run), three `cs_source --window 800x450 +name LiveN
+chimera_perf_log 1 +connect 127.0.0.1:27111` with their own
`CHIMERA_REMOTE_PORT` (two with `+net_fakelag 100 +net_fakejitter 10`,
`+net_fakelag 200 +net_fakejitter 20 +net_fakeloss 3`), each driven
through `chimera/console` (`+forward`/`-forward`, `setang`, `+attack`,
`buy`, `drop`, `say`, radio names, `screenshot`, `name`); RSS from
`/proc/<pid>/status`; then grep the logs for `WARN`, `ERROR` and `hitch:`
and read the server's `status` (missed commands per player).

Interpolation and movers (`games/cs_source/tests/it/net_interp.rs`, at 240 frames a
second with `NetSim::set_frame(1.0 / 240.0)`): `net::interp::InterpClock`
has the render tick others are drawn at (fractional, server ticks) and
the delay; `net::interp::Snapshots<NetBody>` on another player's character
(and `<NetMover>`, `<NetProp>` on the mover and prop proxies) holds what
arrived, by server tick. To check a drawn path, record the server's
position per `SimTick` and compare the client's drawn `Transform` with
the server's between the two ticks around the render tick; `NetGraph`
counts frames extrapolated or held past the newest snapshot and
teleports snapped, and `carried` (ticks the client's mover step pushed
its own player). `cargo test --features dev --test it net_interp --
--nocapture` prints the off-path, per-frame step and error numbers.

Weapons (`games/cs_source/tests/it/net_weapons.rs`, `games/cs_source/tests/it/heavy/map_net_weapons.rs`):
what a client's player carries is part of the predicted state
(`PredictedComponents::encode` includes `weapon::sync`'s blob), so the
prediction checks above cover clips, timers, punch and zoom too. On the
server, `weapon::lagcomp::LagCompStats` counts rewinds (`last`: the view
tick asked for, the tick traced at, characters moved) and each
character's `HitHistory` has its hit volume per tick (to aim a test shot
where a client saw someone: interpolate it at that client's
`InterpClock::render_tick`). Server `WeaponEvent`s say what was fired
(`Fired`: origin, angles, seed, spread) and hit (`Hit`); a client's
`WeaponEvent`s with `replay` set come from re-run commands. Messages last
two updates: gather them after every `NetSim::step` (the tests' `Log`).
`sv_showlagcompensation 1` logs each rewind (how far back, how many
moved); the server's `status` ends with a `lagcomp:` line. `cargo test
--features dev --test it net_weapons -- --nocapture` prints hits with and
without compensation, rewind times, prediction errors while firing and
the pellet comparison.

Rules (`games/cs_source/tests/ui/net_rules.rs`): a client's `RoundState`, `BuyWindow`,
`BombState`, `HostageTally`, `PlantedBomb` and its own `Money` are the
server's, with times on the client's fixed clock: compare a time left
(e.g. `RoundState::clock`, `PlantedBomb::explode_at` less
`Time<Fixed>::elapsed`) with the server's at the same step (the client's
runs about the one-way latency behind). `net::NetScore` on each
character has the scoreboard's kills, deaths, ping and flags;
`net::Notice` (a client's) the server's refusals; `net::ChatMessage`
and `net::RadioCall` who read and heard what. Give `NetSim` clients names
with `add_client(|w| w.resource_mut::<NetSettings>().name = ...)`; with
`chimera_rounds 1` set before they join, they join the running round
dead: `mp_restartgame 1` puts everyone in play. `cargo test --features
dev --test it net_rules -- --nocapture` prints the clocks and
scoreboards.

In a game: `net_graph 1` shows Source's readout bottom right (fps,
ping, lerp, in and out per packet, k/s and packets/s, loss, choke, the
server's frame rate, tick, update and command rates; `net_graph 2` and
`3` add the latency and bytes graph; `net::stats` has the numbers, the
server's once a second; a listen server's host shows its own: ping 0,
no traffic; `games/cs_source/tests/it/net_graph.rs`). The perf overlay
(`chimera_perf 1`) and the debug UI's Perf tab show,
while connected, `net:` (ping, loss, KB/s), `cmds:` (lead in ticks and
its target, the clock's speed nudge and jumps, the server's buffer of
our commands, missed and late), `prediction:` (errors per second and
their worst, totals, commands replayed, the view's correction still
being eased out) and `interp:` (how far in the past others are drawn,
the update interval, characters drawn, snapshots buffered ahead of the
render time and how far ahead the newest is, frames extrapolated or
held, snaps, movers stepped and ticks the player was carried);
`cl_interp` (0.1 s), `cl_interp_ratio` (2), `cl_extrapolate` (1) and
`cl_extrapolate_amount` (0.25 s) set the drawing; `status` on a client prints the prediction and interp lines too,
on a server each player's buffered and missed commands. `cl_showerror 1`
logs every prediction error (tick, metres, which components differed)
and clock jump. `cl_smoothtime` (0.1 s) eases corrections out of the
view; 0 snaps. Fake network conditions on a client (Source's names; its
UDP transport only): `net_fakelag <ms>` delays what it receives (ping
grows by that), `net_fakejitter <ms>` adds up to that much at random,
`net_fakeloss <percent>` drops packets both ways. `cl_lagcompensation 0`
asks the server to trace your shots against the present; on the server
`sv_unlag 0` turns lag compensation off, `sv_maxunlag` bounds it (1 s).

Two real games on this box (`--features dev`; each needs its own remote
port, `CHIMERA_REMOTE_PORT`, so both answer `curl`):

```
CHIMERA_REMOTE_PORT=15791 cargo run --features dev -- --window 1280x720 -port 27031 +name Host +maxplayers 4 +map greybox
CHIMERA_REMOTE_PORT=15792 cargo run --features dev -- --window 1280x720 +name Client +connect 127.0.0.1:27031
```

Then drive each through its console (section 3b): `status` (players,
ping), `getpos`, `+moveleft`/`-moveleft`, `setang`, `screenshot
<file.png>`, `disconnect`. The host's `setpos` moves the host; a client's
position is predicted and the server's state corrects it (a client's `setpos` snaps back). Add
`+net_fakelag 100 +net_fakeloss 5 +cl_showerror 1` to the client's line to feel
a bad link. To shoot each other: `setang <pitch> <yaw>` and `+attack`/`-attack`
through each game's `chimera/console` (aim from the other's `Transform` as
the client draws it, `world.query` on its `Name`), the host's health from
its `LocalPlayer`'s `Health`, `+sv_showlagcompensation 1` on the host to
log each rewind. Cheat commands (`setpos`, `setang`, `noclip`, `god`,
`give`, ...) need `+sv_cheats 1` on the host in a network game; a
client's go to the server and act on its player there (`net::cheats`:
the answer prints in the client's console), so a client's `noclip` is
the server's and predicted. `chimera_noblock 1` (or 2) on the server lets
teammates (everyone) pass through each other. Chat says who joined and
left ("Player X left the game (Disconnect by user.)"); a client whose
server stopped shows "Server shutting down" at once. A round
on de_dust2: host `... -port 27031 +name Host +maxplayers 4 +sv_cheats 1
+chimera_rounds 1 +map de_dust2` (or `+map cs_source:de_dust2`), then `mp_restartgame 1` once the client is in;
`buymenu`, `buy <weapon>`, `jointeam 2`, `say`/`say_team`,
`+showscores`/`-showscores` and `screenshot` through each game's
`chimera/console`. The client's `differences` shows the server's
replicated values while connected; setting one says it can't. On the server, `kick <name>` and `kickid <id> [message]` (ids from
`status`) drop a player ("Kicked by Console"); `name <new>` on a client
renames it for everyone ("* X changed name to Y"); bots take names from
the install's botprofile.db at `bot_difficulty` (0-3; `bot_prefix`),
`bot_kick t|ct|easy|...|expert|<name>`, and with `bot_join_after_player 1`
(the default) the quota waits for a player. The dedicated server:
`cargo run --features dev --bin cs_source_server -- -port 27032 +map greybox
+bot_add` (console on stdin: `status`, `bot_add`, `bot_quota 6`, `changelevel de_dust2`, `quit`). Logs show
`listening on UDP ...`, `<name> joined`, `<name> left`, `disconnected:
<reason>`.

Bots (`games/cs_source/tests/it/net_bots.rs`): `bot_add`, `bot_kick [name]`, `bot_quota
<n>` and `bot_quota_mode normal|fill|match` on the server's console (a
client is refused: `console::SERVER_COMMANDS`); `bot::BotQuota` has the
quota, and a server keeps it (fill counts the host and remote players).
A client sees bots as characters with `NetCharacter::owner` None and
`score_flags::BOT` in their `NetScore`; their shots come as `FireBullets`
(a client's `WeaponEvent`s owned by the bot's copy), kills as `Killed`,
radio as `RadioCall`. `cargo test --features dev --test it net_bots --
--nocapture` prints what each client saw and the server's frame time
with ten bots and two clients.

Maps (`games/cs_source/tests/it/net_maps.rs`): tests build maps in code from a folder of
files standing for an install (`harness::TestMaps`, `serve_maps`; a map
id `test:<name>`, the file any bytes, hashed like a .bsp);
`harness::begin_level_change` then `harness::load_level` on the server is
`changelevel`. A client's `net::client::JoinProgress` has where joining
is (`stage`, each stage's time in `log`, `download` bytes, the server's
info) and why it ended (`failure`: a `JoinFailure` and the words); the
server's `net::maps::ServedMap` what it offers (id, SHA-256, size) and a
client entity's `net::maps::Upload` how far a download over the
connection has come. Downloads land in the content cache
(`<data>/chimera/content/cs_source/maps/<name>.bsp`, noted in its
`index.toml`). On a server, `sv_allowdownload` (1), `sv_downloadurl` (an
HTTP folder with `maps/<name>.bsp.bz2` or `.bsp`; quote it, `//` starts a
comment) and `net_maxfilesize` (512 MB, the most a server sends and a
client takes; CS:S's 16, a deliberate difference: `net::maps::MAX_FILE_MB`) say how clients get the map;
`hostname` names the server. The log shows `changing level to ...`,
`now serving <map> (<bytes>, sha256 ...)`, `sending maps/x.bsp`, and on a
client `saved maps/x.bsp to ...`.

The loading dialog: `connect`, a server's map change and a download show
in it with the game's words; `chimera_loading_details 1` adds a panel
with each stage's time, bytes and percentage and the server's name,
map, players and ping (screenshot it while joining: `+chimera_loading_details
1` on the client's command line).

In game: `maxplayers 4; map <name>` hosts (Source's way), `listen` hosts
the loaded map, `changelevel <name>` (or `map <name>`) while hosting
takes the players along, `connect <ip[:port]>` joins, `disconnect` leaves (or stops
hosting), `status`, `name <you>`.

**LAN test with Windows.** The host's firewall must let the game take UDP
on its port: the first time `cs_source.exe` (or `cs_source_server.exe`) listens,
Windows asks; allow it on private networks. Without the prompt (or on a
public network), add a rule in an administrator PowerShell:
`New-NetFirewallRule -DisplayName "Chimera" -Direction Inbound -Protocol UDP -LocalPort 27015 -Action Allow`
(use the port you host on). Clients need no rule. Find the host's address
with `ipconfig` (IPv4 Address) and join with `connect 192.168.x.y:27015`.
Both ends give up after 30 s without a packet (Source's `cl_timeout`;
`net::client::TIMEOUT_SECONDS`), so a slow map load doesn't drop anyone.
Over the internet the host forwards that UDP port on its router (UDP
only, to the host's LAN address; a fixed LAN address or a DHCP
reservation keeps the rule pointing at it) and gives friends its public
address (what a "what is my IP" page shows); behind carrier-grade NAT
(a public address the router's WAN side doesn't have) forwarding can't
work: one of the friends hosts, or a VPN (Tailscale, ZeroTier) puts
everyone on one network. `net_upnp 1` (before hosting; `+net_upnp 1` on
the dedicated server) asks the router to forward the port itself (UPnP
IGD, `net::upnp`): `status` shows `upnp : UDP <port> forwarded by
router <ip>; friends connect to <public>:<port>`, why it failed, or a
warning when the router's outside address is itself private (CGNAT);
the mapping goes when serving stops. Tests use a fake router
(`games/cs_source/tests/it/net_upnp.rs`): never try it against this box's network.
docs/plans/active/nat-traversal.md has the other options (Steam
networking needs an app id of our own). The host's upload carries every client: about
10-13 KB/s (~0.1 Mbit/s) each with twelve characters at the default
`cl_updaterate 20`, ~15-19 at 33, ~25-31 at 66 (held there by the
default `rate` 30000; ~28-49 without it), more on lossy links (~1.3×);
joins and map changes burst to ~30-40; ~17 KB/s each with 32 players
(multiplayer.md "Capacity"). A client caps what it is sent
with `rate` (bytes a second) and `cl_updaterate`, the host with
`sv_maxrate` and `sv_maxupdaterate` (multiplayer.md, "Soak",
"Bandwidth"). Both
games must be the same build (`status` shows the version); another build
is refused with a message.

**Finding games** (slice 8; `games/cs_source/tests/ui/net_query.rs`). Every server
answers info queries on its own port (`net::query`, A2S_INFO's layout):
`serverinfo <ip[:port]> ...` prints name, map, players/max (bots), ping,
dedicated or listen, password; `lanscan` lists servers on the LAN
(broadcast to `net_lan_ports`, 27015-27020 by default as Source's LAN
tab, at `net_lan_broadcast`'s addresses, and this machine). On this box
use other ports: `net_lan_ports 27031-27032`. A server's `status` shows
`hostname` and `queries : N answered, N challenged, N dropped` (rate
limited per address and in total). `openserverbrowser [lan|favorites|history]`
(the main menu's Find Servers) opens the browser; `serverbrowser <input>`
drives it as a click or key would (`row <n>`, `doubleclick <n>`,
`connect`, `refresh`, `filters`, `sort <column>`, `tab <name>`, `type
<text>`, `enter`, `escape`, `addserver`, `find`, `delete`, `latency` (its
drop-down's list), `hover <n>`, `pick <n>`, `check <name>`, `map`,
`maxplayers` (their text entries), `next` (Tab), `space`) and prints its
rows (through `chimera/console` for screenshots); in a test, the model is
`games::cs_source::ui::server_browser::ServerBrowser` (`handle` an `Input`, its `rows`,
`set_results` from `net::query::ServerQueries::results`); favourites and
history live in the cfg folder's `serverbrowser.vdf`. `sv_password` on a
server, `password` on a client (the browser asks for it). Two dedicated
servers and a client on this box:

```
cargo run --features dev --bin cs_source_server -- -port 27031 +hostname "LAN one" +maxplayers 8 +map greybox
cargo run --features dev --bin cs_source_server -- -port 27032 +hostname "LAN two" +sv_password pw +map de_dust2
cargo run --features dev -- --window 1280x720 +net_lan_ports 27031-27032 +openserverbrowser lan
```

On this Linux box broadcasts don't come back to local servers (the host
firewall); the scan finds them on 127.0.0.1. A LAN scan on Windows: broadcasts leave by one interface (the one with
the default route); with several adapters (VPN, virtual switches) add
the LAN's own broadcast address, e.g. `net_lan_broadcast "255.255.255.255
192.168.1.255"`. The server's firewall rule above covers queries too
(same port).

## 3c. Performance

Details and baseline numbers: [performance.md](performance.md).

- `chimera_perf 1` (2: every render pass) shows frame times (avg, p95,
  max), the main world's CPU time and the render world's time (its
  `Render` schedule), GPU time per render pass (every camera's run of
  it together), entity, mesh and triangle counts, cameras drawing (the
  view model's draws only with something on its layer), the world's
  archetypes (`archetypes N (+M)`: added since the last readout; a count
  that keeps rising makes every query check new archetypes) and visibility culling (camera cluster,
  clusters and map parts potentially visible), and what changes each
  frame (transforms written, under the map's root, mesh and material
  assets modified), and `physics: N dynamic bodies, M awake` (an awake
  body costs the solver, transform propagation and its shadow's checks
  every tick; a settled map has none awake). `chimera_perf 3` also logs
  each second which entities (by name) write transforms and which bodies
  are awake, the modified meshes and materials by an entity drawing them
  (`assets modified by user: PropMaterial Body: 13.9, Mesh Rope : 5.0`),
  and the UI trees `bevy_ui` lays out (`ui: N nodes in M roots, K in
  hidden roots; trees: ...; changed per frame by root: ...`; an unnamed
  root by its first named node, else by its own component types,
  `<HudRoot+InstallRoot>`): anything
  writing every frame for nothing costs transform propagation, layout
  and GPU re-preparation.
- Load times: a Source map load logs `<map>: loaded in <total> s
  (<slowest stages>)` and keeps every stage's seconds in
  `MapData::load_times` (`map::loading::LoadTimer`; a heavy test can
  print them). Time spent putting the map into the world (colliders,
  decal and surface grids) is not in it: time `Sim::new(MapPlugin::new(map))`
  plus a tick. Combat Arms' map loads (first and second of a session,
  peak memory, the spawn into a simulation, the bots' navigation mesh,
  a view model on demand), each under a budget: `cargo test -p
  combat_arms --features dev --test it heavy::map_load -- --nocapture`
  (performance.md, "Combat Arms map loads").
- Load-independent numbers (`metrics`, `client::frame_metrics`;
  performance.md, "Load-independent metrics"): the readout also gives
  distributions over the last 4096 frames and ticks: `frame ms (N): p50
  .. p90 .. p99 .. p99.9 .. max ..; 1% low N fps` (1% low = 1000 / the
  mean of the slowest 1% of frames), then for the main world and the
  render world (each on its own thread) wall and thread-CPU ms
  percentiles and, on Linux, `instr` (user-space instructions retired by
  that thread per frame), IPC and cache misses; `gpu ms` (latest
  timestamps); `tick (N)`: each simulation tick's main-thread CPU ms and
  instructions. How to read them: wall ms move with the machine's load
  (2-10x on the dev box); CPU ms move less (not with waiting or being
  descheduled, but with caches and clock speed); instructions hardly at
  all, so compare instructions between runs and builds, and wall times
  only within one run. CPU far below wall for a world means its thread
  waited or wasn't scheduled. `hardware counters: unavailable (...)`:
  the kernel refused them (`/proc/sys/kernel/perf_event_paranoid` above
  2, a VM without a PMU) or not Linux (Windows has CPU time only, in
  15.6 ms scheduler steps: read it over many frames).
- Hitch log: `chimera_hitch_ratio 2` (default; 0 off) logs any frame
  slower than that many times the median frame (`hitch: frame 41.20 ms =
  5.1x median 8.07; main world ... (cpu ..., N instr, ... ticks):
  RunFixedMainLoop 30.10 (cpu 3.00), ...; between frames ...; render
  world ...: queue ...`): the main world's slowest schedules and the
  render world's time and its slowest `Render` sets
  (`CHIMERA_RENDER_PHASES=0` leaves their marks out), each with wall
  and thread CPU ms, at most two lines a second (`(N more since the last)`). A profile
  build adds the frame's spans with the most self time (`spans by self
  ms: <system> 12.3 x2, ...`). The debug UI Perf tab has the same percentiles as
  a table, with a frame-time histogram.
- What a world holds: `chimera_census [<top>]` (console; any app) counts
  entities by archetype (the `<top>` largest component sets), every
  archetype and table made (Bevy frees none), component types,
  observers, systems per schedule, resources and each reflect-registered
  asset store; `chimera_census save <name>`, then `chimera_census diff
  [<name>]` lists what changed since (the last census or the named one),
  new archetypes' and tables' component sets included: take one, change
  maps a few times, diff (performance.md, "Map changes that slowed the
  tick"). Headless, `metrics::census::Census::take(world)` and `diff`.
- Instructions per system, headless: with `--features dev,trace`,
  `metrics::span_cost::SpanCosts::install()` before the `Sim` is built
  (a system's span is made with it), then `take()` after each window and
  `rises(&before, &after, n)`: every span's own instructions on the
  schedules' thread, load-independent (`party_kit`'s `archetypes` and
  `heavy::bot_cost` print them). Where the time goes outside spans
  (Bevy's command application is in none): run the test under gdb and
  sample it (a thread sending the process `SIGUSR1`, gdb set to stop on
  it and print the test thread's backtrace).
- `chimera_perf_log 1` logs the same readout as one text line a second
  (`chimera_perf: 60 fps  frame 16.67 ms ... | vis: cluster ...`), so runs
  can be compared without reading screenshots (`--frames N ... 2>&1 |
  grep chimera_perf:`); `bugreport` saves the latest readout in
  report.txt whether or not the overlay is on.
- Drawing between ticks (`map::interp`): the readout's `interpolation:`
  line gives the tick rate, the frame's blend between the last two ticks
  (`Time<Fixed>` overstep fraction), ticks run that frame and teleport
  snaps so far; the debug UI's Perf tab shows it live every frame.
  `cl_interpolate 0` draws the latest tick instead (stepping at the tick
  rate), for A/B. `cargo test --features dev --test it interpolation::
  -- --nocapture` prints a walking, ducking camera eye per frame at 240 fps on the CS:S
  tick with it on and off.
- `refcmp bench --views tools/refcmp/<map>.toml` times 200 frames at each
  view (vsync off) and prints a table (frame, main-world CPU, render-world
  (its `Render` schedule), process CPU and GPU ms; with pipelined
  rendering a frame takes about the longer of the main world and the
  render world); `-- <args>` passes options to Chimera (e.g. `-- +r_novis
  1`, `-- --view-size 3840x2160` to time 4K, `-- +mat_antialias 0`).
  Build first; it runs the `cs_source` binary next to it (`--profile playtest` for
  optimized numbers). `CHIMERA_EXECUTOR=multi` runs Bevy's multi-threaded
  executor instead of ours (performance.md, "Frame time pass"). A second
  table gives per view and over all views (`ALL`) frame ms p50/p99/p99.9/max,
  1% low fps, each world's CPU ms and instructions per frame (p50, p99)
  and the ticks' mean instructions; `bench.json` also has
  `bodies_awake` and `cameras_drawing` per frame
  (`jq '.overall.metrics.bodies_awake'`);
  `bench.json` and `bench.csv` in the
  output folder (`<out>/<map>/bench/`) have every metric's n, mean, p50,
  p90, p99, p99.9, max per view and overall, for scripts
  (`jq '.overall.metrics.main_instructions.p50' bench.json`).
- `perfgate` (`games/cs_source/tools/src/bin/perfgate.rs`, the gate
  itself `chimera/tools/src/perfgate.rs`): the instructions-per-tick
  regression gate: `cargo build --profile playtest --features tools --bin perfgate &&
  target/playtest/perfgate` runs greybox, de_dust2 (rounds, 10 bots) and
  mg_lego_multigames_v2 headless for 2000 ticks each, prints instructions
  per tick (all threads: p50, p99, total; main thread) and CPU and wall
  ms per tick, and exits 1 when a scenario's total rose more than the
  tolerance above `tools/perfgate/baseline.json` for that build
  profile. A wanted rise: `perfgate --update` and commit the file, saying
  why. Skips where counters are unavailable (Windows); `--scenario`,
  `--ticks`, `--json <file>`. performance.md, "Regression gate", has the
  spread it was measured with.
- `r_portalsopenall 1` ignores areaportals (closed doors no longer hide
  what's behind them, no clipping through openings): PVS culling only.
  `chimera_perf 1` shows the camera's area, the areas it reaches and how
  many areaportals logic closed.
- `r_occlusion 0` ignores the map's occluders (func_occluder);
  `chimera_perf 1` shows how many are active and how many map parts they
  hide.
- `r_novis 1` draws every map part (no visibility culling);
  `CHIMERA_MERGED_WORLD=1` spawns the world as one mesh per material with no
  culling, as before chunking (A/B comparisons); `CHIMERA_MERGE_BRUSHES=0`
  draws every brush entity from its own meshes (no merged combined
  meshes, `map::merge`).
- `chimera_skyocclude 1` makes sky faces hide what lies behind them even
  in the view's PVS (`render::sky_occluder`; off by default, unchecked
  against CS:S). Where something shows through the sky, the remote
  protocol (section 3) finds it: query meshes with a non-zero
  `ViewVisibility` and their parents' `Name` (mg_creative's were placed
  weapons and water surfaces, then not PVS-culled).
- `refcmp vischeck --views tools/refcmp/<map>.toml` renders the views
  plus views from spawns and nav areas with culling off (`r_novis 1`,
  `r_occlusion 0`) and on (game time
  frozen with `host_timescale 0`), lists the views that differ and fails
  if any differs by more than 0.5% of its pixels (small differences:
  geometry seen through sky brushes, which culling hides as the game
  does; see performance.md). `cargo test --features dev --test it map_vis::` checks the same with rays from
  ~300 player positions per map (`CHIMERA_VIS_FULL=1`: every nav area).
- `refcmp ordercheck --views tools/refcmp/<map>.toml` captures the same
  views (vischeck's) in order and in reverse order and fails if any view
  differs by more than 100 pixels: a capture that depends on the views before it (sort
  order, fades, exposure an earlier view left) shows there, and would
  make vischeck and refcmp comparisons differ by chance. To find what a
  view depends on, capture short `--views` lists with Chimera itself (the
  view, then the views before it in the run, dropping one at a time) and
  compare the view's PNGs (`magick compare -metric AE a.png b.png null:`).
- `REFCMP_OUT=<dir> refcmp ...` writes Chimera's captures, reports, bench
  and vischeck output under `<dir>/<map>` (references are still read from
  the shared dump folder): use a folder in your own `target/` when other
  sessions may be using the shared captures.
- Traces: a `--features profile` build (Bevy's `trace_chrome`) writes a
  Chrome trace with every system, schedule and render pass as a span;
  `TRACE_CHROME=<file>` names it (else `trace-*.json` in the working
  directory). Keep runs short (`--frames 400`: about 1.5 GB) and the
  file in your `target/`. `tracesum <file> [--skip N] [--top N]
  [--filter text] [--by-total] [--folded out.txt]` (`chimera/tools/src/bin/tracesum.rs`) prints the main
  world's (`main app`) and render world's (`sub app: name=RenderApp`)
  time per frame and the spans with the most self time per frame, by
  thread (systems run on the `workers`); open the file in
  https://ui.perfetto.dev for timelines. Our exclusive systems carry
  their own spans where Bevy's per-system ones say too little (the
  logic bridge: `logic: phase`, `logic: sync to the ECS`, `logic:
  sync_movers`). Example (performance.md has real output):
  `cargo build --profile playtest --features profile`, then
  `TRACE_CHROME=target/t.json target/playtest/cs_source --map cs_source:de_dust2 --frames 400`,
  then `cargo build --profile playtest --features tools --bin tracesum` and
  `target/playtest/tracesum target/t.json --skip 200`.
  Read systems' own spans, not schedules' self time: writing the trace
  stalls every thread now and then (2-3 ms gaps without any span), which
  lands in whichever schedule was running, and tracing makes a frame of
  ~1500 small systems about twice as slow. For `--bench` runs, start
  Chimera itself (`--views <refcmp's views.json> --bench`); through
  refcmp the trace came out empty.
  The profile build replaces `target/playtest/cs_source`: copy it aside
  or rebuild without the feature before benchmarking. A short window
  instead (any app with a window, Windows too): `--features trace` and
  `trace_capture 1` in the console, or `CHIMERA_TRACE_SECONDS=1`
  (`client::trace_capture`; `scripts/profile.sh`, `scripts\profile.ps1`);
  Tracy: `--features tracy` and the Tracy 0.14.1 profiler (not set up on
  the dev box). performance.md, "See a frame yourself". CPU sampling (`perf record -g`, `cargo flamegraph`) works on
  any optimized build on Linux: `tools/perf-profile.sh` (perf on a
  `profiling` build with frame pointers, then `profsum`, the
  `chimera/tools` bin: our code and Bevy's systems by ms per frame, a
  flame graph, before/after with `--baseline`; `tracesum --folded` feeds
  `profsum` a span trace on Windows; performance.md, "Measuring"). The
  dev box has no `perf` package installed: the script takes `PERF=<path>`
  (a `perf` binary unpacked from the distribution's package works without
  installing). A main-thread schedule with much self time
  in `tracesum` is waiting for workers (or descheduled under load).

## 3d. The debug UI

F8 (a bind: `debugui`; F1 and F2 are CS:S's autobuy and rebuy) opens a tabbed window; `debugui <tab>` opens it
on a tab (`player`, `world`, `movement`, `bots`, `rendering`, `perf`,
`audio`, `logic`, `rounds`, `cvars`), `debugui close` closes it, e.g.
`--window 1280x720 --screenshot ui.png +debugui perf` photographs one.
Every control runs a console line (hover a button for it), so anything
done there can be typed, bound or scripted too; the window only reads
state. The mouse stays free while it is open (the game ignores input),
and keys typed into its fields don't reach binds.

- Player: position, angles, velocity, movement state, team; god, noclip,
  respawn, `impulse 101`; health, armour and money sliders
  (`chimera_sethealth`, `chimera_setarmor <n> [helmet]`,
  `chimera_setmoney`); weapons held (clip/reserve), give any registered
  weapon, drop; teleport to each spawn point; bookmarks (saved places,
  kept in bookmarks.txt).
- World: the map and a map list to load, tick and game time, entity
  counts, map logic, time scale presets, `host_timescale`, `sv_gravity`.
- Movement: every CS:S movement cvar with its range and a reset, all
  back to defaults, surf preset, the mouse cvars.
- Bots: add T / CT, kick, back to orders, skill presets (the new game
  page's), the bot cvars (`bot_stop` freezes them), a table of each
  bot's team, health, role and activity with watch (`chimera_watch`) and
  go-to buttons.
- Rendering: the overlays (`cl_showpos`, `net_graph`,
  `chimera_drawhitboxes`, `chimera_drawcollision` (F3: `toggle chimera_drawcollision`),
  `chimera_drawphys`, `chimera_drawnav`, `chimera_drawbots`, ...), culling
  (`r_novis`, `r_portalsopenall`, `r_occlusion`, with the vis readout),
  HDR, water, view model and effect cvars, third person and free camera.
- Perf: a frame-time graph of the last 300 frames (white frame, green
  main-world CPU, 60 and 30 fps lines), the live interpolation line, the
  `chimera_perf` readout, `chimera_perf`, `chimera_perf_log`,
  `cl_interpolate`, how to take a trace.
- Audio: volume, DSP, `snd_show`, the soundscape and room readout.
- Logic: the map's entities (filter by name or class), an entity's
  keyvalues and output connections, fire an input at it (`ent_fire`);
  `chimera_logic_record 1` lists the outputs fired (latest 40 shown) and
  the logic's messages.
- Rounds: the round, its phase and score; restart (`mp_restartgame 1`),
  rounds on or off, the round cvars.
- Cvars: every cvar, searchable, "changed only", edited in place (Enter
  sets it), reset to default; matching commands with their help.

Headless: `games/cs_source/tests/ui/debug_ui.rs` opens it, walks the tabs and runs the
lines its tabs run in a `Sim` (`DebugUiStatePlugin`).

## 3e. Several agents at once (builds and tests)

Source `tools/agent-env.sh` in each agent shell, then run builds and tests
through `agent_run`:

```
source tools/agent-env.sh
agent_run cargo nextest run -p chimera_world --features dev
```

It joins one box-wide jobserver FIFO (one token per CPU thread, created once
and kept open by a `sleep infinity` holder) through `MAKEFLAGS`, which cargo
and rustc honour, and `agent_run` puts the command in the `agents.slice`
user slice (CPUWeight=50, MemoryHigh=24G). Measured on the 12-thread, 31 GB
dev box with 4 agents building and testing `chimera_world` and `chimera_sim`
from scratch at once: without it the box ran at load 32 (peak 49), 48 rustc
processes, swapped out 7.5 GB, and a `git status` took 68 ms at p95; with
the jobserver alone load was 16, rustc peaked at 16, swap out was 0.2 GB,
the builds finished 10% sooner and `git status` took 26 ms; the slice on
top cut that to 14 ms. A cap on concurrent test suites (flock semaphore) and
the slice by itself did not help once the jobserver was in, so they are not
included. The holder's tokens are lost if the holder dies (the next `source`
recreates it); a client killed mid-build loses at most the tokens it held.

### Shared compile cache (sccache)

`tools/agent-env.sh` also points cargo at one box-wide sccache, so a fresh
worktree's first build reuses the registry crates (Bevy and the rest) that
any other worktree already built. Nothing else to do: `source` it, build as
usual. It sets `RUSTC_WRAPPER` to sccache 0.19 (`~/.local/sccache-0.19/bin/sccache`;
`cargo install sccache --locked --no-default-features --root ~/.local/sccache-0.19`),
`SCCACHE_SERVER_PORT=4227`, `CARGO_INCREMENTAL=0` and unsets `CARGO_TARGET_DIR`.
The cache (100 GiB, `~/.cache/sccache-0.19`) is set in `~/.config/sccache/config`.
`AGENT_NO_SCCACHE=1` builds without it.

What the keys need, learned the hard way:

- sccache cannot cache incremental builds (`CARGO_INCREMENTAL=0`; the dev
  profile otherwise reports "non-cacheable: incremental").
- The key covers the whole rustc command line and every `CARGO_*` variable
  of the client, so a `CARGO_TARGET_DIR` that differs per worktree defeats
  every entry. Leave it unset: cargo builds in `<worktree>/target`.
- Registry crates hit across worktrees. Workspace crates (`chimera_*`,
  `cs_source`...) have their path in the key, so they recompile in each
  worktree (`basedirs` only helps C/C++, not rustc). That is the
  minutes-long part of a first build, not the Bevy part.
- The server must not inherit the agents' `MAKEFLAGS` jobserver, and
  `agent-env.sh` starts it without. sccache uses its own jobserver (one token
  per CPU thread) and takes a token for every rustc it spawns, including the
  dep-info pass that computes the key. When a client is killed mid-build (a
  tool timeout) the server kills its rustc, and the tokens that rustc held
  are lost. After a few such kills all are gone and every later request
  waits at 0% CPU forever: the "hung server" of 2026-10. `tools/sccache-heal.sh`
  (run by `agent-env.sh`) restarts the server when it has no children and
  fewer tokens than CPU threads on two looks 3 s apart. A killed build also
  never stores the crates it was compiling, so a long one (bevy_pbr under
  load 30) can miss again and again until one run finishes.

Check it: `$RUSTC_WRAPPER --show-stats` (hits, misses, "Non-cacheable
reasons", "Cache location"); a hang shows as requests waiting while
`pgrep -P $(ss -ltnpH 'sport = :4227' | sed 's/.*pid=\([0-9]*\).*/\1/')`
lists no rustc: run `tools/sccache-heal.sh`, or `$RUSTC_WRAPPER --stop-server`
when no build is running. Measured (12 threads, load 30 from other
agents, `cargo build -p source_formats --tests`, new worktree each): cold
79 s with 0 of 20 hits, second worktree 25 s with 19 of 20 (the miss is
`source_formats` itself).

### Build slots (warm checkouts for agents)

A fresh worktree rebuilds every workspace crate: cargo fingerprints them
by absolute path, so neither sccache nor a shared `CARGO_TARGET_DIR`
helps across paths. Slots are git worktrees at fixed paths,
`.claude/slots/slot1..3`, whose `target/` stays from one agent to the
next:

```
slot=$(tools/agent-slot.sh acquire <branch> [base])   # prints the path
cd "$slot"; source tools/agent-env.sh; agent_run cargo test ...
git commit ...                                        # in the slot
tools/agent-slot.sh release                           # when done
tools/agent-slot.sh status                            # who holds which
```

`acquire` takes a free slot under a lock (the one that last held the
branch first), creates its worktree the first time, saves whatever a
previous holder left uncommitted as `.claude/slots/slotN-discarded-*.patch`,
resets and cleans it (ignored files, `target/`, stay) and checks out the
branch: reset to `base` when given (`git checkout -B`), else the existing
branch, else a new one from main. Only the files that differ from the
previous holder's get new mtimes, so cargo rebuilds just the crates they
touch. `release` detaches HEAD (the branch can be merged or deleted
elsewhere) and frees the slot; a slot stays held until then (`release
slotN` frees one whose agent is gone). `AGENT_SLOTS` raises the count.
In a slot `agent-env.sh` leaves cargo's incremental builds on (they only
apply to workspace crates, which sccache can't share across paths
anyway; registry crates still hit the cache) and links with mold where
`ld.mold` is installed (`brew install mold`; `AGENT_NO_MOLD=1` turns it
off). The hooks source `agent-env.sh` in `.claude/` checkouts, so a
commit reuses the agent's build: toggling `CARGO_INCREMENTAL` between
the agent's builds and the hook's (as before) kept two builds of every
workspace crate, each rebuilt per change.

`tools/agent-slot.sh trim` (and `tools/clean-worktrees.sh`, which
otherwise leaves slots alone) shrinks a free slot's `target/` past
`AGENT_SLOT_MAX_GB` (60) with `cargo sweep`; `acquire` trims the slot it
takes. A whole-workspace test build is about 40 GB with the test
profile's line tables, a `cargo run --features dev` build more.

Measured 2026-10-27 (12 threads, load 10-50 from other agents;
docs/performance.md, "Test cycle"): `cargo test -p chimera_sim --features
dev --test it <one test>` from cold in a new slot 1,964 s; after a
one-line change in chimera_sim 4.3-5.5 s (with `CARGO_INCREMENTAL=0`, as
in a worktree: 25-47 s); a second agent taking the slot for a new branch
from main (31 files ahead of the first's) and changing one line, 28.5 s,
then 6.3 s per change.

## 4. Logs

`RUST_LOG=mashup=debug cargo run --features dev`. Log state changes that matter
(movement swaps, mounts, match events) at `info`; per-tick detail at `trace`.

The server log (CS:S's, `game_log`): `log on` writes the half-life
standard's lines (`"Name<userid><uniqueid><TEAM>" triggered
"Planted_The_Bomb"`, `World triggered "Round_End"`) to the console
(`sv_logecho`) and the data folder's `logs/` (`sv_logfile`); a test reads
`game_log::ServerLog::lines` and the `game_log::GameEvent` messages
(`bomb_planted`, `player_death`, `weapon_fire`...; tests/it/game_log.rs).

## 5. Exploring game files

`dump` lists, summarizes and extracts files from a game install, using the
same readers as the runtime mount. Install paths come from
`chimera.local.toml`, else the saved or Steam-found one (`mount::install`).

```
cargo run --bin dump -- cs_source                      # counts and sizes by type
cargo run --bin dump -- cs_source --list --filter de_dust2
cargo run --bin dump -- cs_source --filter materials/de_dust --extract
cargo run --bin dump -- cs_source --sequences models/weapons/v_rif_ak47.mdl  # bones, sequences, activities
cargo run --bin dump -- cs_source --lods models/props   # each model's LODs: switch point:triangles
cargo run --bin dump -- combat_arms --archives         # per-archive title, entropy, file count
cargo run --bin dump -- combat_arms --list --filter worlds2
cargo run --bin dump -- combat_arms --filter worlds2/warehouse.dat --extract
```

Combat Arms' maps: `cargo run --features dev -p combat_arms_app --bin
combat_arms -- --map combat_arms:warehouse` loads `worlds*/warehouse.dat`
from the install (any of the 33 maps in WORLDS2/WORLDS3 without keys; the
others need the V2 key); add `--screenshot shot.png --frames 120 --window
1280x720` for a picture, `--spawn`/`--look` as for CS:S. The load prints
triangles, textures, spawns and warnings. The world files' numbers:
`cargo test -p lithtech_formats -- heavy::world_install --nocapture`
(`every_plain_map_summary` prints each map's version and counts).
`+ca_lobby 1 +wait 40 +ca_lobby_demo 6` turns the lobby on and puts the player in a demo room with six made-up players (`--screenshot lobby.png` photographs the waiting room; the lobby's tests: tests/it/lobby*.rs, tests/ui/lobby.rs).
`+ingamemenu` opens Combat Arms' in-game menu (Esc) for a screenshot of
it, e.g. `--window 1280x720 --screenshot menu.png --frames 120
+ingamemenu`. With the V1 key the HUD's panels (health, weapon, kill
feed, score panel, pickup prompt), the scoreboard (`+showscores`), the
loading screen and the menu are the install's own layouts, strings,
fonts and art (`ui::install`; the
log says `combat_arms ui: the install's layouts ...`); the string table
is the system language's, `CHIMERA_CA_LANGUAGE=DEU` (ENG, POR, RUS, TUR,
DEU, ITA, POL, ARB, SCN) picks one. To see a bot from another player's
view, run the game live and use the remote console: `ca_bot_add alpha`,
then `world.query` the `Transform` of entities with `Intent` and `Name`
for the bot, `setpos`/`setang` beside it (Source-style inches, z up)
and `screenshot <file.png>`. Your own look: `ca_model`, `ca_face`,
`ca_head`, `ca_camo`, `ca_goggle`, `ca_backpack` (a client sends it to the
server, which shows it to every player; bots draw theirs from data/characters.toml's
`[loadouts]`); these choose the gear too (the body is the vest, the head
piece the helmet, the goggle and backpack as such: armour, speed and
stamina from data/gear.toml). Your loadout (gear_perks.md; it goes to the
server, which checks it and shows it to every player): `ca_perks "16012
16001"` (perk items; they must fit 3 slots, 4 with `ca_skill_extender 1`),
`ca_head_mods`, `ca_face_mods`, `ca_vest_mods` (up to 3 gear mod ids each),
`ca_items` (equipped item numbers for set tiers), `ca_fast_respawns`,
`ca_fast_respawns_max`, `ca_fast_pass` (F7's allowance), `ca_infected_item`
(Fighting Chance in Quarantine), `ca_perks_enabled` (the room's Perks
option) and `ca_gear_byte_ids 1` (the reference's byte-wide gear ids: quirk
1). `ca_perk <1-4>` starts the active perk in that position. Live: `world.query`
`Armor` (points / 100) and `ArmorPieces` (the gear's coverage), `CaCharacter` (the figure's and worn terms),
`CaPerks`, `CaPerkEffects`, `CaLoadout` on a player.
When a spot hurts or kills (a playtest's
`setpos` line: the feet, in inches as CS:S's, not Combat Arms units),
`ca_volumes_here [radius u]` lists the map's volumes at the player's
centre or within the radius: name, class, bounds (Combat Arms units,
engine axes), damage per second, pending time, hidden, and `INSIDE`
for the ones the player is in (`heavy::junk_flea2` is such a spot as a
test).

Combat Arms' world objects (`game::objects`: doors, moving and spinning
world models, keyframers, breakables, triggers and the command strings
they run; the server's): `ca_objects [filter]` lists the map's objects
whose class or name contains the filter (without one, every world model
and logic object) with their state: pose in Combat Arms units (the world
file's axes), visible/solid/ray-hit, a mover's machine state, angles and
timer, a trigger's fires and pending time, a counter's value, a
keyframer's clock and next key, hit points. `ca_object_msg <name>
<message...>` delivers a message now as a map's `msg` would (`ca_object_msg
VIP_Door trigger`, `ca_object_msg SlidingWater on`, `ca_object_msg
Level04StartTrigger trigger`) and prints the target's state (a console
message has no player behind it, so a door's `DoorLink` isn't told:
message both leaves); `ca_object_props <name>` prints an object's state,
a `setpos` line at its centre and every property as the map stores it
(its commands: what opens it, which mode or trigger gates it);
`ca_object_cmd <command string>` runs a whole command string (`delay`,
`rand`, variables); `ca_object_vars` lists the command manager's variables
and pending delayed commands. A map's objects and their properties as
stored: `cargo run --bin dump -- combat_arms --objects <map> [--filter
text]` (keyed maps need the V2 key). The spec's cases: `--test it
world_objects::` (synthetic) and `heavy::world_objects` (FREEZER,
SNOWSTORM, HGOILRIG, BLOODMONEY, CITYOFWINGS, the census); a map started
with `--map` playing its mission as the app composes the simulation:
`cargo test -p combat_arms_app --features dev --test it heavy::` (V1 key).

Most stock objects are gated by the mode: the mission's start triggers
(`Level01StartTrigger` in TDM, ...) show, hide or remove them (SHORTFUSE's
dumpster lids are Quarantine's and TDM removes them; its sliding doors
open from a JSF switch in Quarantine), so check `ca_objects` after the
match started and `ca_object_props` for what opens a door. Recipes that
film each kind live (dev build, `noclip` and `god` first, the remote
console's `screenshot <file>` between steps; `ca_objects <name>` says
when it moved):

- FREEZER, the chicken's beak and wings (sliding doors `Chk_mouth`,
  `Chk_wing`): `--map combat_arms:freezer`, `setpos 89 1100 60; setang 0
  90 0`, then `ca_object_msg Chk_mouth TRIGGER; ca_object_msg Chk_wing
  TRIGGER` (open in 0.2 s; again closes). In play a player walking into
  the `Chk_ctl` trigger at its feet (`noclip` off, `setpos 85 1352 5`)
  toggles both. The radio knob (`CMD_A_Radio`, a rotating door) turns 1°
  on `+use` from `setpos 390 261 -17; setang 0 180 0` (its lit model
  swaps in); nothing else on FREEZER moves.
- HGOILRIG, the rising sea (`SlidingWater` and its `Water` and
  `PoisonGas00` volumes): `--map combat_arms:hgoilrig`, `noclip; god`,
  `setpos 397 -3300 361; setang 30 270 0` (south of the rig, the eye
  about 100 u above the risen sea, looking down at it), then
  `ca_object_msg SlidingWater on`: 500 u up over 120 s, slow at first
  (about 150 u at 60 s, 370 u at 100 s; `host_timescale 6`: 20 s).
  Before, the sea's texture is small and far off with floating crates
  near the horizon; after, it fills the view a few times larger and the
  crates are under it. The textured sea near the rig is `SlidingWater`
  (a plane 376 × 315 m); the darker sea beyond its edge and the horizon
  are the world's and stay put, so a view of the distant open sea looks
  the same before and after. `ca_object_msg SlidingWater visible 0`
  hides it, to see which surface is which.
  `ca_object_msg SlidingWater init` puts it back. In play only
  `Level04StartTrigger` starts it (90 s later), and no room or mode of
  the install's mission data plays HGOILRIG, so it never rises on its
  own.
- SHORTFUSE, a dumpster lid (`RotatingDoor00`, used through
  `RotatingDoor01`): `--map combat_arms:shortfuse +ca_mode QRT` (in TDM
  the lids are removed), `setpos 322 300 -2903; setang 0 90 0`, then
  `ca_object_msg RotatingDoor01 trigger`: the lid swings up 90° in 1 s.
  Rounds restart every few minutes with one player and respawn it. In
  play the lid itself takes no use key (`PlayerActivate` false): its
  handle bar on the east end (`RotatingDoor01`, a thin 2 u frustum) does;
  `setpos 350 416 -2860`, `setang 50 180 0` puts the crosshair on it, then
  `+use`. Short Fuse's sliding doors (`Door_OUT_L01`/`R01`, `Door_IN_*`;
  QRT opens them from the JSF switch): `setpos 200 620 -2690; setang 0 0
  0` faces them from 80 u (the room behind is `EndlessFall60`, which kills
  after 4 s until `Qrt_TimeSet` fires at 120 s; god mode doesn't stop it).
- NEMEXISHQ, a double door (`UserDoorA00`/`A01`, used by players):
  `--map combat_arms:nemexishq`, `setpos -2030 153 1; setang 5 180 0`,
  then `+use`, `-use` 0.3 s later: both leaves swing 90° in 0.5 s. Its
  elevators are only doors (no car moves; no stock keyframer moves a
  world model, world_objects.md section 9):
  `setpos -880 368 1; setang 0 180 0`, `ca_object_msg ElevatorDoor00
  trigger; ca_object_msg ElevatorDoor01 trigger` (slide 46 u in 0.5 s).

Combat Arms' music and ambience (`game::music`, the client's): `ca_music`
shows the screen the music thinks it is on, the player's streams with
their slot gains, the playlist (the cursor, unticked entries, rotation or
random) and the interface sounds the cues started; `ca_ambient` lists
the map's `SoundFX` (file, radii, volume, looping, on) with the gain at
the local player. Install facts: `heavy::music_ambience` (BGM.rez, the
loose folder and our table against it, `training`'s ambience, the 33
maps' census); the logic cases: `--test it music_ambience::` and
`--features dev,ui --test ui music::`.

Combat Arms' dedicated server: `cargo run --features dev --bin
combat_arms_server -- -port 27042 +map combat_arms:warehouse +ca_mode TDM
+ca_goal 50 +MaxPlayer 6 +BotLevel 1` (`--help`; console on stdin:
`status`, `ca_bot_add`, `ca_bot_kick`, `MaxPlayer <n>`, `ca_mode <mode>`,
`ca_restart`, `changelevel <map>`, `quit`); players join with
`combat_arms +connect 127.0.0.1:27042` (their own install's copy of the
map). The startup line says `Chimera Combat Arms dedicated server
<version>: <map> on UDP <addr>, <n> players`; bots log whether they walk
the map's AI volumes (`bots: the map's N AI volumes as navigation
areas`) or generated ones (`bots: generated a navigation mesh of N
areas`).

Combat Arms' models: `CHIMERA_CA_PROP=<model>[,<skin>...][;<render style>...]` (archive
paths; render styles by index, e.g. `;RS\default.ltb,RS\glass.ltb`) puts
that model, playing its first animation, 3 m in front of the
first spawn of the `--map` loaded (`game::props::dev_prop`), e.g.
`CHIMERA_CA_PROP=props/headquaters/hq_flag01.ltb` with `--map
combat_arms:warehouse --screenshot shot.png --frames 120`. Readable
without keys: props, models, FX, the gunsmith NPC and the first-person
pistols, shotguns, machine guns and launchers; characters, rifles, SMGs,
snipers, knives, grenades, third-person weapons and the maps' flags need the V2 key. The
readers' numbers: `cargo test -p lithtech_formats -- heavy::ltb_install
--nocapture` (`census` prints the install's model and animation counts).

Combat Arms' view models (`game::view_model`, from the weapon data's
`[models] view`): each loads in the background the first time its
weapon wants drawing (`view_model::on_demand`; the placeholder box
until then), once per session; one that doesn't load logs a warning. To see one, give its weapon in a map run: `--map
combat_arms:warehouse --window 1280x720 --frames 240 --screenshot
pistol.png +wait 30 +give combat_arms:beretta_m92fs` (also
`combat_arms:remington_870`, `combat_arms:m60`); a firing frame: drop
`--screenshot` and end with `+wait 120 ++attack +wait 3 +screenshot
fire.png` (`--frames 300`); the sprint pose: `+wait 120 ++sprint
++forward +wait 40 +screenshot sprint.png` (`+speed` also sets walk).
What each loads and plays: `cargo test -p combat_arms --features dev
--test it heavy::view_model -- --nocapture` (meshes, textured pieces,
bones, sockets, animations with their lengths and key strings, the
`flash` socket through the shot against view_models.md's numbers); the
state machine, sway and placement cases without the install: `--test it
view_model::`.

Combat Arms' radar and full-screen map (`game::radar`, `ui::hud::radar`,
`ui::hud::full_map`): the arithmetic (pixels, range, rotation, wedges, the
guide, edge arrows, the image window, the full map's transform, zoom, the
corners from the mission data), who is shown (`radar::look`), the legend
and what the server sends at a round start are `--test it radar::`
(spec cases 1-26, 37-44 and the arithmetic of 48-51); the panels' state
from the world (zoom keys and the profile, the scanner items, deaths,
objectives, the legend and markers) is `--features dev,ui --test ui
radar::` (`CaRadar`, `CaScene`, `CaFullMap`, headless: read them in a
test to see what would be drawn); the install checks (46-52) are `--test it
heavy::radar` (V1 key for the mission files). To see it drawn: `--map
combat_arms:warehouse --window 1280x720 --frames 200 +wait 60 ++fulltacticalmap
+wait 20 +screenshot map.png` (M held; the radar is in the corner of every
shot; `+incradarscale` / `+decradarscale` zoom while held; with the install
and its keys the game's art and the mission's picture, else flat boxes).

Combat Arms' effects and sounds (`game::effects`): fire at a wall in a
map run to see impacts, decals and tracers, e.g. `--map
combat_arms:warehouse --window 1280x720 --frames 400 +wait 30 +give
combat_arms:m60 +wait 150 ++attack +wait 60 +screenshot fx.png` (the M60:
a tracer every third round). The rules (gain, low-pass, gates, tracer
cadence, decals, casings, footsteps, hurt limit) and the effects in play
on the greybox, sounds read as `CaSound` messages: `--test it
effects_sounds::`; the install's sound and effect archives (which are
unkeyed), every named file, the effect file's numbers and a shot on
WAREHOUSE: `--test it heavy::effects_sounds -- --nocapture`. A map load
warns when effect textures don't read; a sound that doesn't read logs
one warning and stays silent.

Combat Arms' archives are read by `lithtech_formats::rez`. Encrypted ones
list but skip extraction until their scheme's key is in
`chimera.local.toml`: a `[games.combat_arms.keys.V1]` table and a
`[games.combat_arms.keys.V2]` table, each with `key` (64 hex characters)
and `iv` (32 hex characters). The table's name picks the cipher (V1
Twofish, V2 AES-256); there is no cipher field. Keys stay in that local
file only: never in the repo, tests, logs or commit messages (`cargo test
-p repo_checks` refuses tracked keys). With both keys,
`cargo test -p lithtech_formats --test it heavy::keyed_install --
--nocapture` checks each family decrypts to the formats' own headers;
without them those tests pass with a note. Extraction goes to `~/.local/share/chimera/dump/<game>/` (or `--out`) and
refuses any folder inside the repository.

### Which files a run reads (`CHIMERA_VFS_LOG`) and the asset store

`CHIMERA_VFS_LOG=<file>` records every file any run reads through a game's
mount (the game, a test, a tool, a dedicated server): one line per file
and process, `<game>\t<path>\t<size>\t<sha256>`, appended (several test
processes may share one log; `sort -u` it to count). Files a load only
checks for (`contains`) are listed too. It is how the asset store's sets
are recorded (docs/plans/active/asset-service.md, M1):

```
CHIMERA_VFS_LOG=/tmp/dust2.log cargo test -p cs_source --features dev --test it heavy::map_de_dust2
cut -f1 /tmp/dust2.log | sort | uniq -c                  # files per game
awk -F'\t' '{s+=$3} END {print s/1e6 " MB"}' /tmp/dust2.log   # bytes read
cargo run --features dev,tools --bin chimera_pack -- pack cs_source --set de_dust2 --from /tmp/dust2.log
cargo run -p chimera_assets --bin chimera_assets -- status   # verify; gc [--dry-run]
CHIMERA_ASSET_USE_INSTALL=0 cargo test -p cs_source --features dev --test it heavy::map_de_dust2
```

`chimera_pack` reads the logged files through the game's own mount of its
install (Combat Arms' decrypted with this host's keys, never written) into
the store (`~/.cache/chimera/assets`: `records/<ab>/<id>.zst`,
`manifests/<game>/<build>.txt`); the last line runs with the install
hidden, from those records alone (a missing one fails with `asset miss
<game>: <path>`). AGENTS.md, "Game files from the asset store", has the
settings. The heavy tests `heavy::served` (cs_source, combat_arms) record,
pack and reload de_dust2 and WAREHOUSE in a scratch store and print the
sizes and times.

### Sweeping every cached map (mapsweep)

`mapsweep` loads every map in Chimera's content cache
(`~/.local/share/chimera/content/cs_source/maps/`) headless, each under
`catch_unwind`, runs its map logic for a few seconds, and writes
`report.md` (problems ranked by how many maps they affect, then one row
per map), `report.csv` and `warnings.txt` (every warning and logic line)
to `target/mapsweep/` (or `--out`); the console says each map's load
time and its stages over half a second (`MapData::load_times`). Load warnings are grouped by kind
(missing or unreadable materials, textures, models, sounds; decals and
overlays without a surface), plus entity classes nothing handles, classes
handled only as static brushes, and logic complaints (refused server
commands, unhandled inputs). With `--shots <Chimera build>` it also runs
that build per map for a 1280x720 screenshot from the first spawn and the
last `chimera_perf_log` line (needs a display).

```
cargo run --features dev,tools --bin mapsweep                      # every cached map
cargo run --features dev,tools --bin mapsweep -- --filter surf_,kz_ # some of them
cargo run --features dev,tools --bin mapsweep -- --stock            # the install's own maps
cargo run --features dev,tools --bin mapsweep -- --shots target/playtest/cs_source --filter mg_
cargo run --features dev,tools --bin mapsweep -- --audit           # plus the map logic audit
```

`--audit` adds `audit.md` (ranked by maps affected) and `audit.txt` (per
map) from `logic::audit::audit_map`: every output connection resolved
(targets that match nothing; `!activator` taken as a player) and its
input delivered once per target class to a probe world, which says
whether that class handles it (AddOutput by the key it sets); outputs
maps connect that no literal in `src/` names (so nothing fires them);
keyvalues no literal names and the spawnflags set, per handled class (to
check against the code by hand); inputs aimed at brushes the loader baked
into the world. Then a scripted run: one player stands in every trigger
and presses +use at every door and button from four sides, then a round
restart (doors and buttons only where +use can find them: a solid one;
aimed at one of its pieces; a visit where the map's triggers move the
player away before the press, a teleport or a kill zone guarding a
room, is listed apart, not as a miss);
it lists what the logic logged (complaints, and apart from them the
notes: known, intended gaps the logic logs once per map,
`LogicWorld::note`, such as VScript inputs, server plugin commands,
triggers naming a missing filter, inputs to names nothing has), movers
told to move that never did, movers that jumped (moved much further in a tick than their speed),
children of movers that stay where they spawned, usable brushes +use
didn't find, and panics. Physics props aren't simulated there (no
bounds), so bodies never touch triggers in it. A unit test for it is in
`logic::audit::tests`; small entity lists for the fixes it led to are in
`source/logic/src/logic/community_tests.rs`. `--fire <target>,<input>[,<value>]`
(repeatable) skips the sweep: it loads the filtered maps' logic with one
player, sends those inputs and prints every half second the queued
messages, output connections, effects by kind and logic log lines for
`--secs` seconds, to chase run-away map logic headless (what one button
of a minigame map sets off).

The committed summary is in docs/plans/active/community-maps.md.

Minigame maps' flows end to end: `games/cs_source/tests/it/heavy/map_flows.rs` plays
each cached mg_ map headless (the whole game, rounds on) with scripted
characters on both teams (`Flow`: `press` +uses an entity from each
side until it fires, `touch` stands players in a trigger, `kill`,
`said`/`said_ever` read the console, `got` the inputs delivered) and
checks each step: chooser teleports, selector buttons, retargeted
teleports, loadouts, announcements, round ends and what the next round
resets. `CHIMERA_FLOW_TABLE=1 cargo test --features dev --test it
map_flows:: -- --nocapture` prints a row per step (pass, FAIL, known),
`CHIMERA_FLOW_TRACE=<name,...>` the outputs and inputs of entities whose
names contain those. A live check of the same steps: run the game with
`CHIMERA_REMOTE_PORT`, `-port` (not 27015), `+chimera_rounds 1 +jointeam 2
+bot_add 1 ...`, then `setpos`, `setang`, `+use`/`-use`, `ent_dump` and
`screenshot` over the remote console.

Course maps (surf_, bhop_, kz_) and gungame arenas (gg_):
`games/cs_source/tests/it/heavy/map_courses.rs` plays each cached one with a scripted
player (one test per map, the same `Flow` and table) and checks, from
the map's own entities: spawns; every trigger_teleport (stood in with
the name or class its filter wants: lands on the destination the same
tick, velocity kept, view snapped; the player doesn't hang there
afterwards); stage checkpoints (a trigger naming the player, then the
stage's filtered fail teleport); touch-open doors (bhop blocks) moving
and coming back; "multihop" blocks (landing on one sends the player
back, a 2-tick touch doesn't); trigger_push and AddOutput basevelocity
boosters (the velocity change against triggers.md); gravity and
player_speedmod zones; what triggers set off; buttons; the next round.
Each step's detail counts its items, with the ones it can't check
("known") grouped by reason: no room for a player in the trigger,
overlapping teleports or gravity zones (touch order, triggers.md open
question 3), chained destinations. `CHIMERA_FLOW_TABLE=1 cargo test
--features dev --test it map_courses:: -- --nocapture` prints the rows;
`CHIMERA_COURSE_DEBUG=<entity index>` prints the ticks around that
trigger's check (brushes, links, velocity before and after the
movement and the logic). `heavy::map_surf` rides every open ramp of the
cached surf maps, one test per map (`CHIMERA_SURF_MAPS=surf_a,surf_b`
with the test `map_surf::listed_maps` for others,
`CHIMERA_SURF_DEBUG=1` to list what the hull is near at each stop;
`CHIMERA_SLIDE_DEBUG=1`, read by the CS:S movement itself, prints every
slide move's sweeps: position, velocity, fraction, start solid, the plane
hit and whether it was a brush with its own surface or a physics
collider, and when a full move's end tests solid, whether a brush did it.
The lines just before a stop are its tick's).

## 6. Comparing with the real game (refcmp)

`refcmp` captures the same camera views in real CS:S and in Chimera and
reports, per view, brightness (luma), saturation, sharpness (mean absolute
Laplacian) and mean absolute difference, plus side-by-side images
(reference | ours | difference x4).

```
cargo build --features dev && cargo run --features dev,tools --bin refcmp
cargo run --features dev,tools --bin refcmp -- report            # re-compare existing captures
cargo run --features dev,tools --bin refcmp -- capture-ours --only a_sign
```

- Views: `tools/refcmp/<map>.toml` (Source eye positions and angles; add
  more as needed; pass `--views tools/refcmp/<map>.toml` for maps other
  than dust2). de_aztec, cs_office and de_nuke have views generated from
  their own entities (intermission cameras, spawns, bomb/rescue zones);
  cs_assault, de_port and cs_compound (maps with occluders, for `bench`)
  have spawn views only, with no reference captures yet.
  `camera` names the map's `point_viewcontrol`; maps whose cameras have
  no name (de_aztec) use the class name, which moves all of them.
  Output: `~/.local/share/chimera/dump/refcmp/<map>/` (game imagery; never
  in the repo). `--out <dir>` writes our captures, reports, bench and
  vischeck output under `<dir>/<map>/` instead (reference captures are
  still read from the shared folder), so parallel worktrees don't
  overwrite each other's captures. Our captures hide the view model, as
  the references do (`+r_drawviewmodel 0`; `bench` and `vischeck` too).
  They hang ropes still (`+rope_wind_dist 0`): a reference is one moment
  of its wind, and swaying cables would move views by chance.
- Load warnings of every stock map: `cargo test --features dev --test
  it all_stock_maps_warnings -- --ignored --nocapture`.
- CS:S runs through the Steam client (logged in once on this machine) and
  is driven over RCON on 127.0.0.1:27015, so it works with the desktop
  locked: the tool waits for the map to load, repositions the map's
  `point_viewcontrol` for each view and takes `jpeg` screenshots. Keys and
  `setpos` don't work for this (the lock screen owns the keyboard; RCON has
  no player).
- `refcmp fit` also captures Chimera's `--debug-view albedo` and
  `--debug-view lighting` renders and fits, per pixel, how the real game
  combines texture and light. That is where `cs_source::bsp::source_look`
  (no tonemapping, bilinear mip snapping) comes from. Its 1.22 exposure
  turned out to be the bump page encoding; `report`'s luma column checks it.
- `refcmp` builds nothing: run `cargo build --features dev` after code
  changes, or it captures the old binary.
- The report's `detail` column (correlation of high-pass detail) is noisy
  on bump-mapped walls: it preferred a green-flipped normal map that crops
  show is wrong. Check close-up crops (`magick ... -crop`) side by side
  before trusting it.
- A texture mismatch reported at a `setpos`/`setang` spot: `CHIMERA_VIEW_MAP=cs_militia
  CHIMERA_VIEW_POS="261 -334 -94" CHIMERA_VIEW_ANG="10 178 0" cargo test -p
  cs_source --features dev --test it heavy::map_surfaces::surfaces_in_view
  -- --ignored --nocapture` casts rays over that view (a 1280x720
  window's framing) and prints a letter map of the surfaces it hits (world
  faces, displacements, overlays, brush entities, props), then each one
  with its share of the view and what we resolved its material to (base
  and blend textures, detail, alpha, tint, transform, surface property).
  Screenshots of the spot: run live and send `noclip`, the `setpos`/`setang`
  line and `screenshot` over the remote console (on cs_militia a
  `--screenshot` run's `+noclip +wait 30 +setpos ...` photographed
  another place; `getpos` says where you are).
- Measuring thin features (cables): sample columns of the reference and
  ours for dark pixels (`magick <img> -crop 1x260+X+0 txt:-`) and compare
  their rows; that's how rope gravity was found.
- The reference CS:S install runs at mat_hdr_level 0, mat_trilinear 0,
  mat_forceaniso 1, no AA (queried over RCON). HDR (level 2, the game's
  default) is an option (`+mat_hdr_level 2`, client/hdr.rs) but not
  matched yet: captures stay LDR unless a run sets it.
- `refcmp skyconv` measures how the engine samples sky cubemaps (an encoded
  debug sky, `CHIMERA_SKY_DEBUG=1`) and fits each layer's texture and
  orientation to the reference: Bevy's skybox flips z, so looking toward -Z
  shows the +Z layer.
- Chimera renders views off-screen at 1280x720 (`--views`), matching CS:S's
  framing (90 degrees horizontal at 4:3 = 74 vertical).

### Comparing the HUD (`refcmp hudcmp`)

```
cargo build --features dev
cargo run --features dev,tools --bin refcmp -- capture-hud --out <scratch>
cargo run --features dev,tools --bin refcmp -- hudcmp --out <scratch> --hud-ref <dir>
```

- `--hud-ref <dir>` holds CS:S captures named `hudref_<state>_<W>x<H>.jpg`
  (default `<dump>/refcmp/hud/ref`; game imagery, never in the repo). The
  coordinator takes them with the reference client: de_dust2, a CT with
  100 health, no armour, $800, the USP drawn and an AK-47 in slot 1;
  state `ak` with `hud_fastswitch 0` and the selection open on slot 1,
  `select` open on slot 2; at 1280x720, 1920x1080 and 1024x768.
- `capture-hud` runs Chimera into the same states at the same sizes
  (`ours_<state>_<W>x<H>.png`; console commands, then `screenshot`).
- `hudcmp` crops each panel part (health, armour, clock, money and ammo
  digits and icons, the panel boxes, the selection's numbers, icon, name
  and box, the pickup history, the radar's box) from both and prints, per
  part, the offset of ours (the shift that best correlates the two images'
  HUD-coloured ink) and both ink (or box) rectangles; `hudcmp.txt`,
  `hudcmp.json` and enlarged crops (reference | ours | overlay: reference
  red, ours green) go to `<out>/hud/report/`. Different text (the clock,
  ammo counts) still lines up by its left or right edge.
- The measured reference rectangles are numbers in
  `games/cs_source/tests/ui/heavy/hud_layout.rs`, checked against `game_hud::layout`,
  `weapon_select::selection_layout` and `history_layout` through
  `hud_text::ink_rect` (no images needed).

## Measuring CS:S behaviour live

The reference CS:S (see refcmp) can also be measured directly over RCON:
- Client commands run too: `getpos` returns the local player's view
  origin (the eye; in third person the camera; `setpos` takes the feet),
  `+jump`/`+forward`/`setang` move it. A `point_viewcontrol` holds the
  view (and `getpos`) until removed (`ent_remove cam1`; a map reload
  restores it for refcmp).
- Console output that RCON doesn't return (e.g. `ent_dump`) goes to a file
  with `con_logfile <name>` (under `cstrike/`).
- `ent_fire player addoutput "targetname <x>"` names the player for
  `ent_dump`/`ent_fire`.
- Sampling `getpos` over RCON is ~20 Hz with timing noise; fine for
  heights (jump apex), not for per-tick speeds. `host_timescale` slows the
  game but doesn't remove the noise.
- Weapon scripts are ICE-encrypted (`.ctx`); decrypt copies in a scratch
  folder only, never in the repo.

## Tick-exact CS:S comparisons (movecmp)

```
cargo run --features dev,tools --bin movecmp            # all scenarios
cargo run --features dev,tools --bin movecmp -- --only bhop --keep-running
cargo run --features dev,tools --bin movecmp -- fuzz --runs 16 --ticks 150   # ladders and water
cargo run --features dev,tools --bin movecmp -- --offline   # against the last CS:S logs, no server
```

- Each scenario (in `games/cs_source/tools/src/bin/movecmp.rs`) is a start position and per-tick
  inputs. A CS:S dedicated server with the `tools/css_probe` SourceMod
  plugin plays them on a bot and logs every tick; Chimera plays the same
  inputs from the game's tick-0 state on the same map at the same 0.015 s
  tick. Setup: tools/css_probe/README.md.
- Report: per scenario, the largest position/velocity difference and the
  first tick that differs by more than 0.1 unit (or in ground/duck
  state), with both states. Per-tick CSVs go to
  `~/.local/share/chimera/dump/movecmp/`.
- The probe can also be driven directly over RCON (127.0.0.1:27030,
  `mashup_run <in> <out>`, `mashup_weapon <weapon>`); its log has the
  player's box top, eye height and the buttons the movement ran with.
  The plugin resets the bot's move type, remembered ladder and jump
  stamina when it places it, and logs its water level and ladder state.
  CS:S bots duck-jump on their own (inside the game, not through their
  buttons): movecmp holds duck on our side while the bot is ducked in the
  air, so jumps differ only by the bot's instant 8.5-unit duck on the jump
  tick. On that tick the bot's duck makes the jump set vz like a ducked
  one (289.99, not 284) but doesn't slow its input as a held key would
  (0.34; a strafe there got the full 37.5 u/s of air acceleration), so
  movecmp plays it unslowed. The report compares those ticks without the
  8.5-unit lift and counts them ("bot ducks"), so "max pos diff" is our
  movement's own difference.
- `--offline` replays the scenarios against the CS:S logs the last online
  run left in the server's `cstrike/mashup/<name>.out` (same inputs), for
  checking movement changes without starting the server.
- Damage measurements: `tools/css_probe/fallmeas.py` (fall damage; see
  tools/css_probe/README.md). `player_hurt` rows of `mashup_wrun` logs
  carry each hit's damage.
- `movecmp fuzz`: seeded random inputs (keys, jump, duck, turning and
  pitch, in bursts) from ladder spots on de_nuke and water spots on
  de_aztec (found and checked in our sim), plus a straight climb per
  ladder. Each scenario names its map; the probe server changes level as
  needed. Runs fail past `--tolerance` units or when water level or ladder
  state disagree; the exit code says whether all passed. Same `--seed`,
  same inputs.
- It has found: CS:S's fixed 0.015 s tick, the alternating diagonals of
  displacement triangles (walking and landing on dust2 terrain now match
  to 0.002 units), the 0.34 ducked speed and the 8.5-unit air-duck lift.
  Fuzzing water and ladders found: the 260 swim lift, CS:S's duck scale
  rule (duck held, or ducked/mid-duck at the tick's start; on ladders
  too), the per-tick contents cache at the water surface, and that a
  coincident player-clip face can win over a ladder brush (which one wins
  follows the BSP tree's order, `MapBrushTree`). Fuzzing seeds 101-110
  (2026-10-16) found CS:S's duck timer left running when duck is released
  partway down, a jump pressed while unducking (no jump, stamina spent,
  stands up next tick) and the ladder's duck scale taken after the tick's
  unduck; the `duck_release_*` and `unduck_jump_*` scenarios replay them.
  With the bot's jump duck taken out of the report, the base scenarios
  showed a crease bevel our terrain triangles dropped and the 1/32 u/s a
  stopped move keeps off the surface (`wall_slide`). The cs_militia porch
  runs (`militia_porch_wall*`, `hover_*`) found that a trace ending short
  of a plane, however near, doesn't hit it (the 2-unit ground probe's
  exact reach).
- `movecmp brushes <map> <x> <y> <z> [radius]` lists the collision
  brushes near a point (index, ladder flag, bounds, planes in Source
  units) and the terrain triangles: what a diverging run's trace there
  can hit. `movecmp fuzz ... --offline --run <name>` replays one fuzz run
  (same seed and options); with `CHIMERA_SLIDE_DEBUG=1` the ladder probe
  prints its sweep each tick too, and `CHIMERA_GROUND_DEBUG=1` prints each
  ground test (feet, the full box's sweep, the ground found and the brush
  it is).

## CS:S demos (demodump)

A SourceTV demo recorded on the probe server (tools/css_probe/README.md,
"Recording a SourceTV demo") holds everything the server sent, per
snapshot: entity props, game events, user messages, sounds, temp
entities. `demodump` prints it as `tick seconds kind ...` lines
(parser: `source_formats::dem`; never commit `.dem` files):

```
cargo run --features tools --bin demodump -- <path>.dem --summary
cargo run --features tools --bin demodump -- <path>.dem --events round_*,player_death,bomb_*
cargo run --features tools --bin demodump -- <path>.dem --usermsgs TextMsg,HintText,Fade,Shake
cargo run --features tools --bin demodump -- <path>.dem --sounds 'weapons/*'
cargo run --features tools --bin demodump -- <path>.dem --entity CCSPlayer --props m_iAccount,m_flFlash*
cargo run --features tools --bin demodump -- <path>.dem --list-props ParticleSmokeGrenade
cargo run --features tools --bin demodump -- <path>.dem --tempents CTEFireBullets --ticks 1000..1100
```

- `--entity` takes a class name (glob) or an entity index; values print
  when they change, players with their names. `--list-props <class>` shows
  the flattened prop paths to pass to `--props` (nested tables as
  `m_Local.m_flFallVelocity`, array elements as `m_iAmmo.003`).
- Sound lines give the `soundprecache` name, entity, channel, volume,
  level, pitch, flags and origin (8-unit steps). Levels of 256 and up are
  "compatibility attenuation" sounds (256 + value; sounds.md).
- What a SourceTV recording of the sample bot round on de_dust2 showed:
  `round_end winner=3 reason=7 message="#CTs_Win"` then `round_start`
  5 s later (tick 2510 to 2850); `m_iAccount` +300 on a kill and the new
  round's money at `round_start`'s tick; two bots flashed by one flashbang
  at tick 770 with `m_flFlashDuration` 6.84 and 6.65 and
  `m_flFlashMaxAlpha` 255; `RadioText`/`SendAudio Radio.FireInTheHole`
  for a thrown grenade; bot chatter as `RawAudio` (`bot/*.wav`).
- Not in the sample SourceTV demos: the game events `player_hurt`,
  `weapon_fire` and grenade detonations, and the user messages `Fade`,
  `Shake`, `HintText`, `TextMsg` (SourceTV gets only some of what players
  get), nor anything between snapshots (`tv_snapshotrate`, default 16/s:
  one frame per ~4 ticks; set 66 for per-tick values). Their effects are
  in the entity state (health, flash duration, smoke entities); gunshots
  are `CTEFireBullets` temp entities, not sounds. For per-player messages
  record a client demo (`record <name>` in a game client; same format).

### Playing a demo in our game (`playdemo`)

`cargo run --features dev -- --playdemo <path>.dem` (or `playdemo <path>`
in the console; a bare name is looked for in the install's `cstrike/`)
loads the demo's map and plays it: its players are puppets
(`games/cs_source/src/game/demo.rs`), the camera a spectator's (jump: mode;
attack/attack2: next/previous player). `demo_timescale <x>`,
`demo_pause`/`demo_resume`/`demo_togglepause`, `demo_gototick <tick> [1]`
(relative with 1), `demo_info`, `stopdemo`. Screenshots of a tick: run it
with the remote console (`chimera/console` `{"line": "demo_gototick 3000"}`,
then `demo_pause` and a capture). Tests: `demo_playback` (a demo built in
code on the greybox) and `heavy::demo_playback` (the sample demos on
de_dust2; skipped without them: `$CHIMERA_DEMOS` or `target/demos/`).
Spectating a demo: first person hides the watched player (body and
weapon), chase and free look show both, as CS:S; the demo's players
coming in say no "is joining" lines (`core::Playback`); a jump
(`demo_gototick`) clears the death notices, which age by the demo's
clock (`game::demo::DemoClock`: paused, they stay).

### Re-simulating a client demo (`demosim`)

A client (POV) demo holds the recording player's user commands every
tick (`dem_usercmd` frames: buttons, move amounts, view angles, weapon
picks, the command number that seeds spread and recoil;
`source_formats::dem::usercmd`) and that player's own-client values
(full-precision origin, velocity, ducked, tick base, clip, reserve,
shots fired). `demosim` plays the commands again through our movement
and weapons on the demo's map and compares, command by command
(`games/cs_source/src/game/resim.rs`):

```
cargo run --features dev,tools --bin demosim -- <file.dem> [--player N] [--offset N] [--limit N] [--tolerance UNITS]
cargo run --features dev,tools --bin demosim -- --probe <name> [--map de_dust2]
```

- It prints the first divergence (command, what differs: position past
  0.1 u, velocity past 0.5 u/s, on ground, ducked, weapon, clip, reserve,
  shots fired, accuracy penalty, alt mode; both states), maxima, means
  and counts per kind, and the shots (paired in order: same command, seed, the
  recorded `CTEFireBullets` inaccuracy against ours, mean degrees off
  the aim). Per-tick CSV: `~/.local/share/chimera/dump/demosim/`.
- The server's settings in the demo (`net_SetConVar`: `sv_maxspeed`,
  `sv_accelerate`, `sv_airaccelerate`, `sv_friction`, `sv_gravity`, ...)
  are applied first and listed; the tick is the demo's (`svc_ServerInfo`);
  move amounts above 400 (`cl_forwardspeed 450`) are taken as sent.
- The player starts as the first state has it: weapons, clip, reserve,
  and (where recorded) the weapon's accuracy penalty, shots fired, its
  timers (the probe's `next_primary`/`next_secondary`/`next_attack`) and
  its alternate state, silencer on or burst mode (the probe's `special`,
  a demo's `m_bSilencerOn`/`m_bBurstMode`). That alternate state is
  compared every tick ("alt mode", silencer and burst weapons only): a
  toggle in the recording is replayed as the command's attack2 (a probe
  bot's own attack2 from the buttons its movement ran with); one without
  attack2 in its command (a weapon drawn, a bot's AI) is set on ours as
  recorded and counted apart ("set from the recording"), not a divergence.
  So is a weapon switch no command picked (a bot's AI; the probe's bot
  goes to its knife when the USP runs dry with no reserve).
- States are keyed by the player's tick base, which runs a fixed amount
  ahead of the command numbers; `demosim` finds that offset by trying the
  nearby ones on the first 64 commands (`--offset` sets it).
- SourceTV demos (the sample ones, a dedicated server's `tv_record`) have
  no user commands: "not a client demo".
- `--probe <name>` reads the probe server's last run of `<name>` instead
  (`cstrike/mashup/<name>.in`/`.out`, from movecmp's and weapcmp's runs;
  tools/css_probe), the real game's states per tick: `spread` and
  `recoil` (pistol fire while strafing; `recoil`'s USP silenced) match tick for tick, clip and
  accuracy penalty included, and so does `walk_stop` (down dust2's
  terrain, within 0.001 u); the bot's seeds don't come from its command
  numbers, so they aren't compared there. To see why a tick differs:
  the per-tick CSV gives the tick, `CHIMERA_SLIDE_DEBUG=1` prints every
  sweep of the slide move. Tests: `demo_resim` (a client
  demo built in code: commands through the demo writer and reader;
  synthetic recordings of a silenced USP, a FAMAS in burst mode and an
  XM1014 started mid-recovery),
  `heavy::demo_resim` (the probe's logs; skipped without them).

Recording one on the Windows PC (the user; nothing here can drive a CS:S
client): start CS:S, open the console (enable it in Options, Keyboard,
Advanced), start a game (Create Server, any stock map, or join any
server), then `record <name>` once in the map (in the game, not the
menu), play (bunny hops, ladders, strafing while shooting, reloads), and
`stop`. The file is `<name>.dem` in the game's folder:
`...\steamapps\common\Counter-Strike Source\cstrike\<name>.dem`
(Steam: Properties, Installed Files, Browse). Recording on a listen server
you host keeps its settings and lag known; set `cl_cmdrate 66` and
`cl_updaterate 66` so every tick is sent and received. Then on that PC
(after `scripts\playtest.ps1` has pulled) or after copying the file here
(never into the repo): `cargo run --features dev,tools --bin demosim --
"<path>\<name>.dem"`. The demo's map must be in the install.

## Planned

- Per-tick JSONL traces of chosen entities, for comparing movement against
  the original game's measurements.
- Scripted intent playback in the windowed game (record a demo, replay it,
  screenshot at given ticks).
