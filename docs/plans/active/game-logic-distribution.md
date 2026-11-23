# Game logic distribution: one binary or modules

The question (user, 2026-11-22): every game's logic sits in one
executable, "already 100-something MB; with all these games, is it too
much at runtime? Should we download game logic in packs at runtime, like
assets?" This plan measures what the games cost in size, memory, start
and per tick today, projects it to about 15 games
([next-games.md](next-games.md)), weighs four ways to ship game logic and
recommends one with the thresholds that would change it. Related:
[architecture-for-iteration.md](architecture-for-iteration.md) (options
A, C, H and section 4, server-distributed modules),
[architecture-pass.md](architecture-pass.md) section 7 (the module API),
[docs/DESIGN.md](../../DESIGN.md) ("Assets, code and legal").

**Short answer.** Size is not the problem and will not be one at 15
games: game code is 16% of the client's code today, a game adds 2-16 MB
of machine code (0.4-4 MB in the download), and code pages are clean,
file-backed memory (about 5% of the client's resident set). What does
grow with every game is **per-tick work**: the games not in the match
already cost 23% of a tick's instructions, because their systems run and
scan every character whether or not their game is playing. Fix that in
the one binary (gate each game's systems behind a cheap run condition,
load its data on first use); do not split the code into downloaded
native modules, which would buy a few MB of download for an ABI, signing
and trust problem. Keep WASM for later rule modules from servers and
others, not for ports.

## 1. Measurements (2026-11-23, main at c806b120b)

Linux dev box (Ryzen 5 5600X, 31 GB, RTX 3080), shared with other
agents; sizes and instruction counts do not depend on load, times are
indicative only. Builds as `.github/workflows/release.yml` makes them:
`cargo build --release --locked -p ultimate_gun_game` and `-p
ultimate_gun_game_server`, each alone (feature unification as shipped).

### 1.1 Binary size

| | client (`ultimate_gun_game`) | server (`ultimate_gun_game_server`) |
|---|---|---|
| as linked | 358.8 MB | 159.1 MB |
| stripped (what the Linux zip holds) | 260.7 MB | 115.0 MB |
| gzip -6 (about the zip) | 93.9 MB | 39.2 MB |
| zstd -19 | 67.5 MB | 27.3 MB |
| `.text` (machine code) | 187.7 MB | 89.4 MB |
| `.rodata` | 27.0 MB | 4.2 MB |
| `.eh_frame` + `.gcc_except_table` (unwinding) | 22.6 MB | 10.5 MB |
| `.rela.dyn` + `.data.rel.ro` | 18 MB | about 8 MB |

The "100-something MB" is the zip: client and server together about 133 MB
compressed. Release builds have no debug info; the 98 MB between linked
and stripped is the symbol table (strip removes it; a Windows `.exe`
never carries it).

**Machine code by who caused it.** Each function symbol (`nm -S -C`) is
charged to the first of our crates named anywhere in it, a game crate
before a Chimera one, so a Bevy system or query generic instantiated for
a game's type counts as that game's; symbols naming none of ours go to
the crate that leads their path. Of the client's 187.7 MB of code, 94.8
MB is ours by that rule (35.6 MB in functions of our crates' own paths,
the rest generic instances of Bevy and std for our types).

| layer | client | share | server | share |
|---|---|---|---|---|
| Chimera (`chimera_*`) | 55.6 MB | 29.9% | 26.8 MB | 30.4% |
| Bevy, wgpu, naga, egui | 54.8 MB | 29.5% | 14.6 MB | 16.5% |
| other third-party | 16.7 MB | 9.0% | 7.1 MB | 8.1% |
| std, core, alloc, hashbrown | 14.2 MB | 7.6% | 5.6 MB | 6.3% |
| avian, parry, glam | 4.6 MB | 2.5% | 4.4 MB | 5.0% |
| engine families (`source_*`, `lithtech_formats`, `blam_formats`, parsers) | 6.0 MB | 3.2% | 5.9 MB | 6.7% |
| party and app (`party_kit`, `party_roster`, `ultimate_gun_game`) | 4.3 MB | 2.3% | 0.3 MB | 0.3% |
| **Combat Arms** (`combat_arms` + `_ui`) | 15.9 MB | 8.6% | 12.8 MB | 14.5% |
| **Halo CE** | 5.7 MB | 3.1% | 5.0 MB | 5.6% |
| **HL2:DM** | 3.6 MB | 1.9% | 3.1 MB | 3.5% |
| **TF2** | 2.5 MB | 1.3% | 2.3 MB | 2.6% |
| **CS:S** | 1.8 MB | 1.0% | 0.6 MB | 0.6% |
| **five games together** | **29.5 MB** | **15.9%** | **23.8 MB** | **27%** |

A game costs about 0.14-0.17 MB of client code per thousand lines of its
Rust (Combat Arms 111k lines incl. its front end: 15.9 MB; Halo 39.5k:
5.7; HL2:DM 24.6k: 3.6; TF2 15.1k: 2.5). CS:S is small because its
logic lives in the Source family's crates, which HL2:DM and TF2 share.
Most of the client is the engine and its renderer, not the games.

### 1.2 Memory and start

The release client, offscreen 1280x720, muted, 75 s a run, sampled from
`/proc/<pid>/smaps` and `pagemap` (scratch script, not kept). TF2's and
Halo's installs are absent on this box, so their code is linked and
registered but their content never loads: a natural "game not in play".

| run | RSS steady | heap | anonymous | device/driver maps | binary code mapped | binary code touched in the last 20 s |
|---|---|---|---|---|---|---|
| de_dust2 | 2.93 GB | 1.14 GB | 1.24 GB | 0.40 GB | 160 MB | 47.8 MB |
| dm_lockdown | 3.09 GB | 1.38 GB | 1.11 GB | 0.40 GB | 161 MB | 41.7 MB |
| server, de_dust2, 8 bots | 1.50 GB | 0.89 GB | 0.52 GB | - | 75 MB | 14.0 MB |

- **Code is mapped almost whole, and that is cheap.** 160 of 188 MB of
  the client's code is resident, every crate 85-100% of it, Halo's and
  TF2's too: the kernel maps a faulting page's neighbours that are in
  the page cache (fault-around) and reads the file ahead, so "untouched
  code is never loaded" holds only for code far from anything used
  (`blam_formats`, never called without Halo's disc: 27-30% mapped).
  These pages are clean and file-backed: shared between processes, never
  written to swap, dropped first under memory pressure. They are 5% of
  the client's resident set; assets, the heap and the driver are the
  other 95%.
- **The hot set is small**: 42-48 MB of code referenced in 20 s of play
  (`clear_refs`, then `Referenced`), 14 MB on the server.
- **Start**: exec and argument parsing 30-45 ms (`--help`; a large
  binary is mapped lazily, its size does not show). First frame at 14 s
  on de_dust2, all of it the map and the installs (Combat Arms' 771
  mission pairs, its UI install's 37,329 strings and 935 textures, the
  map's 1,594 textures), none of it code size. What a game adds at start
  is its install scans, which belong in "lazy data" (section 3a).

### 1.3 Per tick: games that are not in the match

`ultimate_gun_game`'s `heavy::idle_games` (added with this plan): the
party's simulation with every game's parts composed
(`PartyPartsPlugin`), a ladder drawn from Combat Arms' guns only, eight
Combat Arms bots on the greybox, 300 ticks counted per system by work
counts (docs/performance.md, "Work counts"; dev profile, so our crates at
`opt-level` 1 and counts higher than a release build's), summed by the
crate each system comes from. The four other games have nothing to do.

836 systems registered, 4.41 M instructions a tick:

| crate | systems registered | ran | instructions a tick | share |
|---|---|---|---|---|
| chimera_sim | 95 | 68 | 859k | 19.5% |
| combat_arms (in play) | 185 | 128 | 843k | 19.1% |
| bevy_app (schedule running) | 4 | 3 | 735k | 16.7% |
| avian3d | 104 | 99 | 605k | 13.7% |
| **hl2dm** (absent) | 111 | 92 | **435k** | **9.9%** |
| **tf2** (absent) | 60 | 51 | **307k** | **7.0%** |
| **halo_ce** (absent) | 89 | 69 | **229k** | **5.2%** |
| **cs_source** (absent) | 9 | 6 | **35k** | **0.8%** |
| chimera_core | 17 | 14 | 135k | 3.1% |
| source_engine, source_logic | 48 | 31 | 94k | 2.1% |
| everything else | 104 | 26 | 132k | 3.0% |

**The four absent games cost 1.01 M instructions a tick, 23% of it.**
Most of that is a few systems that scan every character and then check
whose it is: HL2:DM's `think` 194k, TF2's `wear_bodies` 136k (a query
over all players with nine optional components, filtered by
`GameIdentity` inside), HL2:DM's weapons frame 40k, Halo's `fly` 29k.
The rest is the floor of running a system with nothing to do: 218
absent-game systems ran, at a few thousand instructions each (the
executor, an empty query, the counting wrapper). HL2:DM, TF2 and Halo
hold 85 `run_if`s between them, but nothing skips a game's systems as a
whole while the game is not in play.
The client's front ends (`<game>_ui`) add their own per-frame systems,
not counted here.

## 2. Projection to about 15 games

next-games.md's candidates add about ten games (DoD:S, the GoldSrc
family, Insurgency/L4D2/Black Mesa, id Tech 3, F.E.A.R., Halo 2, UT) and
three or four new engine families. From the per-game numbers above
(client code 1.8-15.9 MB a game, mean 5.9, median 3.6; a family's shared
crates about 6 MB; an absent game 35k-435k instructions a tick, mean
250k):

| | today (5 games) | about 15 games | growth a game |
|---|---|---|---|
| client code (`.text`) | 188 MB | 235-270 MB | 4-6 MB (+1.5-6 a new family) |
| client stripped | 261 MB | 325-375 MB | about 6-8 MB |
| client download (zstd / zip) | 67 / 94 MB | 85-97 / 120-135 MB | about 1.5-2 / 2-3 MB |
| server stripped | 115 MB | 170-200 MB | about 5-7 MB |
| code resident (clean, file-backed) | 160 MB of 2.9-3.1 GB | 220-250 MB (about 7%) | 4-6 MB |
| absent games a tick (as today) | 1.0 M instr, 23% | about 3.5 M, about half the tick | about 250k instr |
| absent games a tick (gated) | - | under 0.1 M | a few hundred instr |

Size grows linearly and slowly: fifteen games are a third more client,
the download stays under the size of one game's asset pack (packs are
hundreds of MB). The tick does not grow slowly: ungated, games that are
not playing would cost as much as the match itself, on every client and
server, at every tick, and the front ends' frames add their share. That
is the cost worth acting on, and the only one that matters at runtime.

## 3. Options

### (a) One binary; every game gated, its data loaded on first use

- **Systems**: each game puts its systems in one `SystemSet` per
  schedule (`GameSystems::<G>`), configured once in its plugin with a
  run condition that reads a resource the match keeps
  (`GamesInPlay`, set when a ladder, a map or a character of that game
  enters; a bitset, so the condition is a few instructions).
  Bevy skips a set whose condition is false without running its
  systems' queries. Systems that must run for a game's characters
  anywhere (a mixed ladder) keep running whenever its game is in play.
  The per-character scans that check `GameIdentity` inside the loop
  become queries filtered by the game's marker component, so they cost
  nothing for other games' characters even when theirs is in play.
- **Data**: a game's install scans and its parsed documents (mission
  tables, UI installs, string tables) load when the game first enters
  play, on a task, behind the loading screen; the asset service already
  fetches content on demand.
- **Check**: `heavy::idle_games` becomes a bound (an absent game under,
  say, 10k instructions a tick; absent games together under 1% of the
  tick), and the same for a client frame with the front ends.
- Cost: small, per game, mechanical; no change to how games are built
  or shipped. Code size unchanged (it was never the problem).

### (b) Per-game native modules downloaded at run time

- **ABI**: Rust has no stable ABI. A Rust `dylib` must be built by the
  same compiler, with the same Bevy, avian, features and workspace-hack
  as the client that loads it, so a module is valid for exactly one
  client build: shipping it separately saves nothing, it is the release
  cut into files. A C ABI (`cdylib`, `abi_stable`) means no `Query` or
  `Res` across it, i.e. every game rewritten against a host API
  (architecture-for-iteration.md option A: 224k lines).
- **What it would save**: 0.4-4 MB of download a game (zstd), and code
  pages that are clean and cheap anyway. Per tick it saves no more than
  (a) does. Generic instances shared between a game and the host
  (Bevy's systems and queries for its types are half a game's code)
  would be duplicated or forced into the host.
- **Platforms**: Windows DLLs cap at 65,535 exports (Bevy's own dylib is
  313 MB here); `dlopen` of a downloaded file needs it written to disk
  with execute rights; Windows SmartScreen and antivirus treat
  downloaded unsigned DLLs harshly.
- **Trust**: a native module runs with the player's rights. It must be
  signed with a key the client pins, verified before load, versioned in
  the match manifest; a server must never be able to hand a client
  native code of its own (any server operator could then run code on
  every player's machine). That makes it an update channel for our
  builds, which the release pipeline already is.
- **Legal and assets**: fine for our own code only (DESIGN.md); packs
  must stay code-free (ingest.md: no scripts, binaries, Lua or WASM), so
  modules would be a separate, signed channel beside the asset service.
- Verdict: no, for ports. Revisit only as a delta-update mechanism if
  the client's own updates become a burden (and try binary deltas first,
  section 4).

### (c) WASM rule modules

Sandboxed, portable, safe to accept from a server or a community author:
fuel limits, no clock or filesystem, determinism pinned at the boundary.
But a port's systems are ECS systems over hundreds of components; behind
WASM each becomes a batch call through copied columns (architecture-pass
section 7.3), slower per tick and a rewrite. The right tool for original
minigames' and servers' rule modules (stage 6 of
architecture-for-iteration.md), never for a ported game's logic.

### (d) Split apps per game family

Single-game apps already exist (`cs_source`, `hl2dm`, `halo_ce`,
`combat_arms`, each with a server). Ultimate Gun Game and Lucker Party
are mixes by definition: any match may hold every game's guns, so a
lean client per family cannot play them. A party server could be linked
with only the games its minigames compose, but the server's 115 MB is
not a cost anyone pays. Verdict: keep the single-game apps for what they
are; do not split the party.

## 4. Recommendation and thresholds

1. **Do (a) now**, game by game, starting with the three that cost most
   while absent (HL2:DM 435k, TF2 307k, Halo 229k): a `GamesInPlay`
   resource in Chimera, a per-game system set with its run condition,
   marker-filtered character queries, and `heavy::idle_games` turned
   into a bound. Then the front ends' frame systems the same way, and
   the install scans and parsed data on first use. Every new game is
   written gated from its first commit (a line in new-game.md).
2. **Keep one binary per app.** Fine up to at least 30 games at current
   growth (code about 350-400 MB, stripped client about 450 MB, zstd
   download about 120 MB, code resident under 10% of RSS).
3. **Act on distribution only when one of these is true**:
   - the stripped client passes 600 MB or its compressed download 200 MB
     (about 50 games at 6-8 MB each): first ship binary deltas
     (`zstd --patch-from` or bsdiff between releases, a few MB per
     update), then reconsider;
   - a platform or store imposes a cap the binary nears;
   - players or servers must get new game logic without a client
     release: then (c), WASM rule modules for minigames and community
     content, behind the batch-shaped hooks; never native code from a
     server;
   - code resident passes 15% of the client's RSS on the smallest
     supported machine.
4. **Watch**: re-measure section 1 at every second new game (the
   scripts' method is above: `nm -S -C` attribution, `size -A`,
   `/proc/<pid>/smaps`, `heavy::idle_games`).

## 5. Steps

- [x] Measure size, memory, start and absent-game ticks (this plan;
      `apps/ultimate_gun_game/tests/it/heavy/idle_games.rs`)
- [x] `GamesInPlay` and each guest game's run condition in Chimera
      (`chimera_core::games_in_play`: `gate_game` around a bundle's `sim`
      in `party_kit`; Bevy 0.20 can't put systems already added into a
      set, so the condition, an atomic flag, goes on each of the game's
      own systems). HL2:DM, TF2, Halo and CS:S gated as guests; the base
      game (Combat Arms in the party) never is. A game enters play on its
      identity, weapon, ladder rung, map or any entity with its crate's
      components, and stays in play (section 6)
- [ ] Marker-filtered character queries in `think`, `wear_bodies` and the
      like, for matches where several games are in play (section 6)
- [x] `heavy::idle_games` as a bound (absent games under 1% of a tick) and
      a game entering play mid-match running that tick
- [ ] The same for the front ends' per-frame systems, counted in a client
      frame
- [ ] Per-game install scans and parsed data loaded when the game enters
      play (`GameEntersPlay` (an observer) is the hook; the guests' simulations
      load no install data at build: their weapon data feed the catalogue
      every ladder draws from, their bodies already load by the match's
      offer. What remains is the front ends' installs, with the step above)
- [x] new-game.md: a new game is gated from its first commit

## 6. Gating done (2026-11-23)

`chimera_core::games_in_play` (module doc): `party_kit` composes each
guest's `GameBundle::sim` inside `gate_game`, which gives every per-tick
system whose type path starts in the game's crate the game's switch as a
run condition (an atomic flag, no world access). A game enters play when
a character takes its `GameIdentity`, a weapon of its `game:` ids is made
(both observers: the same tick), the ladder names its guns, its map
spawns, or an entity holds a component of its crate that no other
crate's component requires (every game's parts on every character are
no sign). It stays in play for the app's lifetime, so whatever it put on
characters (TF2's bodies' names and skins, a burn, a projectile in
flight) keeps being handled as before; games leaving play is left for
later (it needs each game to say what it leaves behind).

`heavy::idle_games` (now attributed by crate from each system's full
type path, `metrics::work::WorkCounts::take_with_crates`: section 1.3's
rows by short name gave Combat Arms' `think` and `wear_bodies` to HL2:DM
and TF2), the same Combat Arms match:

| | instructions a tick | absent games' systems |
|---|---|---|
| every game in play (as before) | 4,463k | 817k (18.3%): TF2 317k, HL2:DM 244k, Halo 231k, CS:S 24k |
| gated | 3,383k | 0 |

24.2% of the tick saved (the schedule runner's own share fell by 221k
too: fewer systems to start). The test bounds the absent games under 1%
and checks a game entering play mid-match (a rung's gun of a game not in
the ladder) runs its systems that same tick.

Still open: the front ends' frame systems (`GameClient`), per-game
character lists for matches with several games in play (each in-play
game's loops still visit every character), games leaving play, and data
loaded on first entry where a game has any worth deferring.
