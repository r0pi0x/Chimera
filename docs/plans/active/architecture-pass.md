# Plan: architecture pass for many games

Status: started 2026-11-09 on main at 71a2d568. Steps 1-6 are done
(section 5); the rest is ordered in section 6. The second review
([architecture-review.md](architecture-review.md), 2026-10-20) asked
"can two games' gameplay run in one app"; its roadmap is done. This
pass asks the next question.

**The question (user, 2026-11-10):** port as many first-person shooters
as possible: older ones first, Halo: Combat Evolved soon, then
Battlefield Heroes, Battlefield: Bad Company 2, WolfTeam, maybe console
exclusives such as GoldenEye 007 (N64). Today there are three games
from two engine families (CS:S and HL2:DM on Source, Combat Arms on
LithTech). Later, game logic may come from servers as signed native
modules or WASM behind one batch-oriented module API (exploration only;
section 7 says what keeps that door open). So: **what does game number
30 cost, and what must not grow with the number of games?**

Method: `cargo metadata` for the crate graph, line counts, `git show
--stat` of HL2:DM's first commits (the cost of the most recent new
game), greps for game names below the games. Build times are quoted
from docs/performance.md and test-speed.md; the dev box was shared with
three other agents during the pass, so nothing here was timed fresh
except where it says so.

## 1. Facts (main at 71a2d568)

### 1.1 Size, in lines of Rust (src, tests)

| Layer | Crates | src | tests |
|---|---|---|---|
| Chimera | core 11.2k, world 26.1k, sim 25.0k, net 18.7k, present 25.9k, client 30.9k, assets 4.5k, vfs 0.7k, tools 1.8k | 144.9k | 10.8k |
| Source family | formats 6.6k, engine 28.3k, logic 27.8k, gameui 14.6k, tools 1.0k | 78.3k | in the games |
| LithTech family | formats 6.5k | 6.5k | 4.1k |
| Games | cs_source 36.0k, hl2dm 23.3k, combat_arms 102.4k, originals 0.1k | 161.8k | 141.0k |
| Apps | party_kit 5.4k, eleven thin ones 3.1k | 8.5k | 15.3k |

### 1.2 What the second game of an engine family cost (HL2:DM)

HL2:DM is 23.3k lines on 78.3k lines of Source code it shares with
CS:S, most of it lifted out of `cs_source` when HL2:DM needed it
(`source_engine`, `source_gameui`; lifts S0-S10 of hl2dm.md).
`cs_source` shrank to 36.0k. So a second game of a family costs about
a quarter of the family: the lift rule ("generalize when the second
game needs it") worked.

Combat Arms, the first game of its family, is 102.4k lines in one crate
on 6.5k of formats: nothing of it is shared, because no second LithTech
game exists. **Every game on the roadmap is the first of a new family**
(Blam!, Refractor 2, Frostbite, an N64 ROM). The cost that matters from
here is the first-of-a-family cost, and whatever Chimera can take from
it.

### 1.3 The fixed cost of adding a game (HL2:DM's M0 and M1 commits)

Before any gameplay, `42220ec4` and `cd545826` changed 53 files:

| What | Lines | Grows with |
|---|---|---|
| The game crate's skeleton (`lib.rs`, `game/mod.rs`, `game/mount.rs`, Cargo.toml) | 167 | the game (fine) |
| Two app crates: the app (238 lines) and the dedicated server (456), manifests, build scripts | 831 | copied: the server was 74% identical to Combat Arms' after renaming |
| `checks/tests/it/architecture.rs`: the game's name added to every other crate's deny list, its own rows | 115 (36 mentions today) | **games x crates** |
| `checks/tests/it/ecs.rs`, `determinism.rs`: per-file allowlists for the copied servers | 4 | apps |
| Root tools (`dump`, `chimera_pack`, `mapsweep`): a match arm per game | 140 | games |
| `chimera/core/src/mount/install.rs` | 32 | (a one-off generalisation) |
| docs: ARCHITECTURE.md's paragraphs, CLAUDE.md's commands, modules.md | 80 | games |
| Cargo.toml: members, default-members, workspace dependencies (three lists) | 22 | crates |

And later, to join the party apps: 84 code lines of `apps/party_kit`
name a game crate today (the bundle list is 2 of them; the rest is the
base game's requirements text, loading screen, menu music, mouse
options, team map and surface tables).

### 1.4 Game knowledge below the games

- `chimera_core` and `chimera_world` depended on `source_formats` (for
  KeyValues text and the sound decoder), so an edit to a BSP lump
  reader rebuilt Chimera and every game; Combat Arms, a LithTech game,
  depended on Source's format crate for WAV. **Fixed, step 1.**
- `chimera_core::console`: `SERVER_COMMANDS` listed CS:S's bot commands
  and `mp_restartgame` (**fixed, step 3**); `SERVER_NAMES` still lists
  two CS:S cvars; `SOURCE_NAMESPACE = "cs_source"` is where a bare
  Source name without an engine home goes (step 10).
- `chimera_client`'s `--help` says `cs_source` (every app prints "named
  after the CS:S app").
- 90 `0.0254` literals in Chimera's source (80 at the second review):
  Source's inch as engine defaults. `GameInfo::meters_per_unit` exists;
  the literals don't use it.
- `MapData` (chimera/world/src/map/data.rs, 462 `pub` items): the
  world/kit split and the typed `extras` are in; a new engine's map
  features must go through `extras`, never a new field.
- 171 queries say `With<LocalPlayer>` and most assume one. Split screen
  (GoldenEye's whole multiplayer) is the first game feature that breaks
  an assumption this wide.

### 1.5 Apps and tools that know every game

- `apps/party_kit`: `BASE = combat_arms::game::BUNDLE`, `GUESTS = [cs_source,
  hl2dm]`; beyond the bundles it names Combat Arms' loading screen,
  music, options, rule settings and bots, and CS:S's GameUI look and
  view-model cvar.
- Two tables are per **pair** of games: `TEAMS` (a map game's spawn
  teams to the gameplay's) and `SURFACES` (a map game's surface names
  to the gameplay's, one TOML per pair, the HL2:DM row reusing CS:S's).
  At 30 games a pairwise table is 870 rows; it has to be per game, to a
  neutral vocabulary.
- `src/bin/{dump,chimera_pack,mapsweep}.rs` match on game ids.
- Twelve app crates for three games (an app and a server each, the
  mashup demo, the party kit and its four binaries). At 30 games that
  is 60 more crates of the same two shapes unless the shapes move down.

### 1.6 Build and tests

- Cold slot build 30-60 min, a leaf edit 14-30 s, the fast tier 46 test
  binaries run one after another (test-speed.md); `combat_arms it` is
  1,509 tests and half the fast tier's time.
- One test binary per crate keeps links few; the cost is that a game's
  whole suite is one unit. Per-game crates keep an edit in game A from
  rebuilding game B; the party apps are where everything meets, and
  they link every game.

## 2. Where it hurts at 10-30 games

1. **Hand-kept lists that grow with games x crates.** The architecture
   test's deny lists had to name each game in each lower crate's row; a
   new game missing from a row was silently allowed. Same shape:
   Cargo.toml's three lists, ARCHITECTURE.md's diagram, the checks'
   folder lists.
2. **Copied app crates.** Each game copies an app and a server; fixes
   then go to N copies (the four servers already differ in small ways:
   map cycles, `MapFiles`, options).
3. **First-of-a-family cost lands in one crate.** Combat Arms'
   `game/objects` (8.7k), `rules` (8.2k), `effects` (6.9k), `lobby`
   (4.3k), `bots` (4.3k) are where a new family's work goes. Some of it
   is the game; some is "an engine family's behaviour" that would be
   shared by a second game of it; some is Chimera's job done locally
   (both games carry their own bots; each has its own effects library).
4. **Chimera's vocabulary is Source's where nobody looked yet**: inches
   as defaults, the console's home namespace, hit groups, the shell's
   usage text. Each is cheap alone and each new engine pays for all of
   them.
5. **Pairwise tables** in the party kit (teams, surfaces).
6. **Apps reach into games.** An app that names `combat_arms::ui::
   loading::CaLoading` can't take a game it wasn't written for; a
   module loaded at run time has no such path at all.
7. **New engines stress the neutral contracts**: a ROM is not a
   filesystem; Refractor and Frostbite maps are terrain plus placed
   objects, kilometres across; Halo and Battlefield have vehicles and
   seats; GoldenEye is split screen with no network. None is built
   yet; each needs a seam that is a slot or an extension, not a field.
8. **Compile and test time** grow with the party apps' dependency on
   every game, and with one binary per game holding thousands of tests.

## 3. Target architecture

### Decisions

- **D1. A layer is a folder; the rules follow the path.** `chimera/*`
  is the engine, `<family>/*` a loader layer, `games/<id>` a game,
  `apps/*` an app. Engine: no loader, game or app under it. Loader:
  the engine and its own family. Game: the engine and the families its
  manifest declares (`[package.metadata.chimera] loaders`); never a
  game or an app. App: anything below, another app only when declared
  (`apps`). *Why:* the rule for game 30 exists before game 30 does;
  the manifests say in data what each crate is. (Step 2.)
- **D2. What no engine owns lives in Chimera's plain-data crates**:
  `chimera_vfs` (mounts), `chimera_formats` (sound files, KeyValues for
  Steam's library), `chimera_assets` (the store). *Why:* Chimera and a
  game of one family never build on another family's crate. (Step 1.)
- **D3. One descriptor per game, and apps read only descriptors.** A
  game's `GameBundle` (simulation), `GameClient` (front end) and
  `GameServer` (dedicated server) are statics of plain data and
  function pointers. Apps, the party kit and the tools take games as
  descriptors from one list (`games/roster`, a feature per game)
  instead of naming `game::...` paths. *Why:* the per-game cost of an
  app or tool becomes zero; a subset build is a feature set; and a
  descriptor is exactly what a loaded module would hand over
  (section 7). (Step 4 began it; steps 6-9.)
- **D4. App shapes live in Chimera, apps are data.** The dedicated
  server (`chimera_net::dedicated`), then the client app
  (`chimera_client`), each run a descriptor. A game's binaries are a
  static and one call. *Why:* 60 copied crates otherwise; one fix
  reaches every game. (Steps 4, 6, 7.)
- **D5. A loader family has up to four crates, created when used**:
  `<family>/formats` (plain data, no Bevy; always), `<family>/engine`
  (conversion into Chimera's types and behaviour every game of the
  family shares), `<family>/logic` (its map scripting), `<family>/
  gameui` (its menus). The first game of a family keeps everything
  but formats in its own crate; the second game triggers the lift.
  *Why:* one user doesn't prove an interface (the Source lifts did
  after HL2:DM; guessing Combat Arms' shared half would have been
  wrong).
- **D6. Chimera's contracts grow by slot and extension, not by field.**
  A map is `MapData` (world + kit) plus typed `extras`; movement,
  weapons, rules, HUD, bodies are slots filled by id. New engine
  features arrive as: a new slot (vehicles/seats: a movement the
  character swaps to), an extension (terrain as an extra the collision
  and drawer hooks read), or a mount kind (a ROM as a `FileSource` of
  named segments). *Why:* a field in `MapData` or a branch in Chimera
  is paid by every game.
- **D7. Per game, to a neutral vocabulary; never per pair.** Surfaces,
  teams, hit zones, units, actions: each game maps its own to
  Chimera's. *Why:* pairs are quadratic.
- **D8. Hooks move from `fn(&mut World)` to declared data in, data
  out** where they run per tick (the ECS cleanup's direction, the
  ratchets in `checks/tests/it/ecs.rs`). *Why:* testable, parallel,
  and the only shape a sandboxed module can have (section 7).
- **D9. Parity and independence stay as they are**: specs only, games
  never use each other, a lift goes down, not across.

### The picture

```
apps/<name>            a static descriptor + one call; party apps take the roster
games/roster           the list of games' descriptors, a feature per game      (step 8)
games/<id>             GameInfo, BUNDLE, CLIENT (ui), SimPlugins, harness; data/
<family>/gameui|logic|engine   what the family's games share (Bevy level)
<family>/formats       plain data
chimera/client|present|net|sim|world|core   slots, registries, app shapes, harnesses
chimera/formats|vfs|assets                  plain data no engine owns
```

## 4. A new game, end to end

[docs/new-game.md](../../new-game.md) is the checklist, kept true by
`repo_checks` (`games_share_one_shape`: every game crate has the same
public seams; `layers_follow_paths`: it builds on nothing it may not).
In short: specs first; formats as plain data in the family's folder; a
game crate with `game::INFO`, a mount, a `MapLoader`, `SimPlugins`,
`<Game>Plugins`, `game::BUNDLE`, `harness::<Game>Sim`; movement,
weapons (TOML + hooks), rules (rule modules on the mode runtime), its
net part; a `ui` feature with its HUD and `CLIENT`; a server static;
an app. What Combat Arms' skeleton taught is in that file's "Order of
work".

How loaders for other engines plug in (D5, D6), by what is different:

| Engine | Different from today | Seam |
|---|---|---|
| Blam! (Halo CE) | one cache file per map holding tags (models, bitmaps, sounds, scripts) | `blam/formats`: the map file as a `FileSource` of tags by path, so the mount and the asset store work unchanged; the map and the kit both come out of it |
| Refractor 2 (BF Heroes) | zip archives, `.con` scripts, heightmap terrain, vehicles | archives as mount layers; terrain as a `MapData` extra with a collision and a draw hook; vehicles as a seat slot |
| Frostbite (BC2) | `.fbrb` bundles, streamed terrain, destruction | bundles as mount layers; the same terrain extra; destruction as its family's logic crate, like Source's breakables |
| N64 (GoldenEye) | no filesystem: a ROM image; display lists; split screen | the ROM as a `FileSource` of named segments (the user's own dump, as an install); several local players (1.4) before its multiplayer |
| any | its own units, axes, tick | `GameInfo::meters_per_unit`, `tick_interval`; the loader converts axes |

## 5. Done in this pass

Each is one commit; behaviour unchanged.

1. **`chimera_formats`**: `wav` and `keyvalues` moved out of
   `source_formats` (which re-exports them at their old paths).
   `chimera_core`, `chimera_world` and `combat_arms` no longer depend
   on a Source crate. *Unlocks:* a `source_formats` edit rebuilds only
   Source's crates and games; a new family's formats crate has the
   sound decoder without Source.
2. **Layers by path** (`layers_follow_paths`): D1 as a test, with the
   loader families and app bases declared in manifests; the checks'
   crate folders are read from the workspace. *Unlocks:* a new game,
   family or app is under the rules with no table edit.
3. **Server commands are registered** (`Console::server_command`):
   CS:S's bot commands and `mp_restartgame` left Chimera's table.
   *Unlocks:* any game protects its own operator commands (Combat
   Arms' `ca_bot_add` and HL2:DM's are not marked yet: marking them is
   a behaviour change, for their owners).
4. **The dedicated server once** (`chimera_net::dedicated`,
   `GameServer`): HL2:DM's and Combat Arms' servers are a static and
   `run(&SERVER)`: the two crates went from 931 lines to 266. *Unlocks:*
   a new game's server is its usage text, a static and one call; steps
   6 and 8.
5. **The new-game guide and its check** (docs/new-game.md,
   `games_share_one_shape`). *Unlocks:* ports by other people and
   agents without reading three plans.
6. **Every dedicated server on `chimera_net::dedicated`**: `GameServer`
   gained `ServerHooks`, optional and neutral (`ServerHooks::NONE` for
   none): the player count, several games' maps (`ServerMaps`: the game
   a bare name is, a loader of whole ids), the map cycle following the
   map loaded (`GameInfo::map_cycle`), `MapFiles` from `GameInfo::
   map_file`, the greybox's own tick, a minigame (`ServerMinigame`:
   `+minigame`, its first map, its console line, restarted on a map
   change, in the banner), what the machine lacks before a map loads,
   and a launch-only part (`dedicated::app`). CS:S's server is a
   `GameServer` static on `cs_source` without `ui` and no longer
   depends on `cs_source_app`; the party kit's server is the `PARTY`
   const, Lucker Party's `SERVER`, and the Ultimate Gun Game's server
   is `PARTY` with its own minigame and launch. New check:
   `game_servers_use_no_app` (a game's `<id>_server` uses no app crate).
   CS:S's and the party's servers went from 879 lines (their own run
   loops) to 408 (statics, usage texts, tests); with `dedicated` (now
   743, hooks and their tests included) and the other two servers, the
   server code went from 1,632 lines to 1,419. *Unlocks:* one server shape for every game and party app;
   a fix to the run loop, `changelevel` or the console reaches them all.

## 6. Next, in order

| # | Step | Size | Unlocks |
|---|---|---|---|
| 7 | The client app once (`chimera_client::app`): window, log layer, physics, presentation, network, shell, start map from a `GameApp` descriptor; the three apps' `main.rs` become a static | M | a new game's app is a static; the shell's usage text from `GameInfo` |
| 8 | `games/roster`: the games' descriptors in one list, a cargo feature per game; `dump`, `chimera_pack`, `mapsweep` and the party kit iterate it (each game's tool hooks move into the game: `open_install`, what `chimera_pack` roots a set at) | M | tools and party apps cost nothing per game; subset builds; a check that apps name a game crate only for its descriptors |
| 9 | The party kit's base game by descriptor: the loading screen, menu music, mouse options, requirements text and bots settings it takes from Combat Arms become `GameClient`/`GameBundle` hooks; `TEAMS` and `SURFACES` per game to a neutral vocabulary (review A3's surface kinds) | M-L | any game as the base; no pairwise tables |
| 10 | Chimera's Source words: the console's home namespace from the registered games instead of `SOURCE_NAMESPACE`, `SERVER_NAMES` registered like step 3, the 90 `0.0254` literals to `GameInfo::meters_per_unit` or named constants of the game that measured them, a ratchet on the literal | M (slices) | a non-Source game doesn't inherit inches or CS:S's cvars |
| 11 | Cargo.toml's member lists and ARCHITECTURE.md's crate diagram generated or checked from the folders (`docs_index`) | S | a new crate edits one place |
| 12 | Several local players: `LocalPlayer` as "the player this view belongs to" (a view id), starting with the shell's camera and HUD | L | split screen (GoldenEye), a spectator wall |
| 13 | New contracts as the next family needs them, each with its plan: terrain extra, seats/vehicles slot, a ROM mount | L each | Halo, Battlefield |
| 14 | Compile and test scale: the party kit's tests against two reference games only; a game's fast tier split by module with nextest filters; `cargo test -p` of a game never builds another | M | time per game flat |
| 15 | Second user of LithTech or of a Combat Arms subsystem (objects, effects library, lobby): lift into `lithtech/engine` or Chimera, as the Source lifts went | L | the first-of-a-family cost falls for the next one |

Steps 7-8 are mechanical moves like steps 4 and 6 and can go to parallel
agents (different files). Step 9 touches the party kit's client, which
other work edits often: do it in one sitting. Step 10's literal sweep
is the "big mechanical move": last, alone.

## 7. The module API (exploration only)

The idea: a game's logic arrives from a server as a signed native
library (official games, full speed) or as WASM (untrusted), behind one
batch-oriented, data-oriented API. Nothing is built. What would make it
easier, in the order it pays off even without modules:

1. **Descriptors as the only door** (D3). A module's entry point
   returns what `BUNDLE`/`CLIENT`/`SERVER` are today. Every app-to-game
   path that is not a descriptor field is a path a module can't offer.
   Steps 8 and 9 close them; the check in step 8 keeps them closed.
2. **Data before code.** Weapons, minigames, modes, HUD layouts and
   server documents are already data (`weapon-data.md`, `minigames/`,
   `server_docs`). Each rule that becomes data needs no module at all.
3. **Hooks shaped like batches** (D8). Today 78 registries hold
   `fn(&mut World, ..)`; none can cross a sandbox. The shape that can:
   a hook declares the component columns it reads and writes and gets
   slices (`fn(batch: &mut MoveBatch)`), returning commands or events.
   The movement slot is the first candidate (one call per tick over
   all characters of that movement, its inputs already plain: intent,
   state, a collision query). New hooks should be written this way;
   the `&mut World` ones convert as the ECS cleanup touches them.
4. **Stable names and plain layouts.** Predicted and replicated parts
   are registered by name (`predicted_net`, `net_part`, `own_part`);
   keep every game-defined component that crosses the tick behind such
   a name, `Copy`/POD where it is hot, no `String` or `Vec` in it. A
   module can then describe its components to the host.
5. **No process state** (the determinism ratchets: no new statics, no
   std hash maps). A module gets a world, not a process.
6. **The collision and query surface as a narrow API**: what movement
   and weapons ask the world (sweep, ray, overlap, surface at a point)
   through one trait or batch call rather than avian's types directly;
   it is what a WASM module would import.
7. **Versions in the match manifest** (already in the handshake): a
   module's hash joins the weapon sets' there.

What would make it harder, to avoid: games reaching into each other or
into apps (D1 forbids it); new Chimera APIs that hand out `&mut World`
or Bevy types in their signatures for per-tick work; game logic in
front ends.

## 8. Not doing, and why

- **A plugin discovery crate (`inventory`, `linkme`)** for the roster:
  linker-section registries behave differently under Bevy's dynamic
  linking (`--features dev`) and on Windows; a roster crate with one
  line per game is explicit and costs nothing.
- **A shared bots framework or effects library now**: decided before
  (plan after the rename: bots framework skipped); two implementations
  that differ by game are not yet a proven interface. Revisit at the
  third (step 15).
- **Splitting `combat_arms` (102k) into crates**: it would cut its
  build time, not the cost of the next game; its owner's edits land
  everywhere in it. Step 14 measures first.
- **Renaming `mashup`, `cs_source_app`'s old paths, or reformatting**:
  conflicts with every branch in flight for no behaviour.
- **Terrain, vehicles, split screen**: no game needs them this week;
  each gets a plan with its first user (steps 12, 13).
