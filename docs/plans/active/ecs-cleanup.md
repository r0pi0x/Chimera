# Plan: ECS cleanup where new games plug in

Status: the audit (roadmap item 1) done 2026-10-29 on main at
a5e70668, read-only. Section 4's step 1 (guards) done 2026-10-29:
`checks/tests/it/ecs.rs` and `determinism.rs` over every simulation
crate, DESIGN.md "ECS style" (what landed: step 1 below). Steps 2
(single-slot hooks keyed by game), 3 (the weapon seam), 4 (a mode's
per-tick hooks as systems), 6 (the minigame runtime's per-tick work)
7 (the HUD's status as a component), 9 (round and match lifecycle
events) and 10 done 2026-10-30. Next: step 5.
The roadmap item's other parts (docs/backlog.md, "First: ECS cleanup
where new games plug in") follow section 4's order. Goal (user, 2026-10-29): outcomes, not a rewrite. The
next games (Half-Life 2: Deathmatch, Team Fortress 2, later Call of
Duty) should plug in through components, observers, messages and
systems with declared queries, and nothing may get slower.

The target style is the `OnRung` change (a4fe0e5d,
`chimera/sim/src/minigame/ladder.rs:322-367`, DESIGN.md "Arriving on a
rung is a component, games react with observers"): a fact about an
entity is a component, put on where the fact becomes true (on the host
by `ladder::arm`, on a network client from replicated state by
`ladder::follow_rungs`), replaced only when it changes, and every game
reacts with its own observers (`On<Insert<OnRung>>`,
`On<Discard<OnRung>>`; Combat Arms' in `games/combat_arms/src/game/
rungs.rs:150`). It replaced a callback registry (`RungCharacterHooks`)
and works the same on host and client with nothing replicated twice.

## 0. Summary

Counts (non-test source; method in section 1):

| What | Count | Note |
|---|---|---|
| Functions taking the world | 1,103 | 882 take `&mut World`; 293 are exactly `(world: &mut World)` |
| Callback registries (fn pointers taking the world) | 33 types, 87 fields | plus 11 types of plain `fn` callbacks and 13 `Arc<dyn Fn>` ones (section 2.1) |
| Exclusive systems in per-tick or per-frame schedules | 117 | 82 fine (A), 9 reactions (B), 26 per-tick logic (C) (section 2.2) |
| Hooks the per-tick schedules call through registries | 7 fields | `Hooks::update`, `touches`, `died`, `decide`; `Flow::players`; `RulesModule::status`; `NetHooks::replay` |
| Single-slot hook resources (last plugin wins) | 6 | `JoinRules`, `Holds`, `BotHooks`, `NewGame`, `LogSubjects`, `ViewAnimRules` (section 2.1) |
| Process-wide mutable statics | 45 | 13 diagnostics and flags, 18 read-only tables, 9 install memos, 5 per-app state (section 2.3) |
| Raw `world.query` sites (new `QueryState` per call) | 240 | 38 kept states (`cached_query`/`first`); 55 directly in the 117 systems, at most 142 reachable per tick (section 2.4) |
| Observers | 25 registrations | 15 of them `add_observer` |
| `Died` read as a message | 31 systems | already the target style |

What we found:

- **The weapon frame and identity are already in the target style.**
  A weapon is components (`chimera/sim/src/weapon/mod.rs`), data hooks
  insert parts on the weapon entity (`WeaponHooks`,
  `weapon/data.rs:686`), games add systems before or after
  `WeaponFrame` in `Predict::Weapons`, and identity is a component
  switched by a normal system (`chimera/sim/src/identity.rs:91`). The
  exceptions are small: `LaunchHook` (grenades), CS:S's `bomb::arm`
  (an exclusive system in a predicted schedule), and `ViewAnimRules`,
  one per app.
- **The seams that will hurt a third game are single-slot hooks and
  per-tick mode hooks.** `JoinRules`, `Holds`, `BotHooks`, `NewGame`,
  `LogSubjects` and `ViewAnimRules` are resources that each game's
  plugin inserts, so the last one added wins. `chimera_core::gameplay`
  says so itself under "Known shortcuts": they aren't switched with the
  gameplay. A mode's `update` and `touches` are `fn(&mut World)` that
  one exclusive system dispatches every tick (`mode/flow.rs:98-110`).
  So a Team Fortress 2 mode would be a whole-world function with no
  declared access, not a system.
- **Most whole-world code is where it should be.** Map loading,
  spawning, network apply and capture, codecs, the console, menus and
  tools need the world. 82 of the 117 per-tick exclusive systems are of
  this kind and stay.
- **Performance is not the reason to do this.** The per-tick exclusive
  paths that showed in profiles were fixed by the kept query states
  (docs/performance.md, "Party frame-rate pass": new `QueryState`s 1.23
  to 0.40 ms a frame; ticks 24% fewer instructions). What is left is
  about 0.1 ms. The one cost to watch is the executor's per-system
  overhead (~600 systems a tick, 0.6-0.8 ms a frame), which splitting
  dispatchers into gated systems adds to (section 5).

Top 10 steps (section 4 has each with files, guards, effect and
behaviour flags):

1. Guards first: the `repo_checks` ECS rule as ratchets, the
   determinism ratchet extended to every game crate, DESIGN.md's ECS
   style. No behaviour change.
2. Single-slot hooks keyed by game and resolved through `Gameplay`
   (`JoinRules`, `Holds`, `BotHooks`, `NewGame`, `LogSubjects`).
3. The weapon seam: `ViewAnimRules` per game; `LaunchHook` as an
   entity event; CS:S's `bomb::arm` a normal system.
4. A mode's per-tick `update` and `touches` as systems gated by
   `mode_is(id)`, chained where the dispatcher ran.
5. Death and spawn pollers as normal systems on `Died` or observers
   (`give_starting_weapons`, `remove_killed_weapons`, `perk_deaths`,
   Combat Arms' `scoring::handle_deaths`, the bot roster's `arm`).
6. The minigame runtime's per-tick work as normal systems
   (`end_protection`, `respawn_dead`, `hold_the_end`, `join`).
7. The minigame HUD's `status` hook as a component the rules module
   keeps (the HUD-layer seam).
8. `RulesModule::joined` and `arming` as entity events; `CaMode::left`
   as an observer of the leaving event.
9. Round and match lifecycle events (`FlowEvent` triggered) so other
   parts observe instead of polling (results, breakables' restart,
   music).
10. Per-app statics into resources: map-loading `PROGRESS`, the binds'
    `GAME_DEFAULTS`/`OLD_DEFAULTS`, the console's `MAP_NAMES`/`CYCLE`.

Later or not at all: install memos into resources or assets (needs the
map loaders to get the app's context, E2's left part), the net capture's
remaining raw queries, `NetHooks::replay`, Combat Arms' bot brain (E3,
parked).

## 1. Method

Read-only, on a5e70668. Scans are text scans of comment- and
string-stripped source with `tools/arch_survey/survey.py`'s helpers
(`strip`, `split_tests`, `block_end`, its test-path rules), plus a
throwaway script (not committed). Counts are approximate in the same
way `survey.py` says: compare numbers made by the same scan.

- *Functions taking the world*: `fn` items with `&World`, `&mut World`
  or `DeferredWorld` in their parameters, outside tests, `checks/` and
  `tools/`.
- *Callback registries*: struct fields, tuple structs and type aliases
  whose type is a `fn(..)` pointer (with `Option<>`/`Vec<>`) mentioning
  `World` or `EntityWorldMut`; function parameters of that type are not
  counted.
- *Per-tick exclusive systems*: world-taking functions named in an
  `add_systems` call of a schedule other than `Startup`, `PreStartup`,
  `PostStartup`, `OnEnter`, `OnExit`. A plain name matches in the same
  file and a path (`flow::update`) matches its module in the same crate.
  Hooks called through registries are listed separately. The first,
  crate-wide name match gave 184 with many false hits (every mode's
  `update`); the file and module match gives 117, which were checked by
  hand.
- *Raw queries*: `.query::<`, `.query_filtered::<`, `.try_query::<`;
  kept states: `cached_query`, `query_cache::first`. "Reachable per
  tick" follows calls by name from the per-tick systems and hooks
  within a crate, so 142 is an upper bound.
- *Statics*: the patterns of `checks/tests/it/determinism.rs`
  (`OnceLock`, `LazyLock`, `Mutex`, `RwLock`, `Cell`, atomics,
  `thread_local!`), over every crate.

The backlog's earlier figures (~350 functions, ~720 queries, 78
registries, 26 observers) came from a different scan. Use the numbers
here as step 1's baseline.

## 2. Inventory

Classes: **A** fine as it is (map load, spawning, net apply or capture,
codecs, console, menus, tools, one-off); **B** better as an observer or
message (a reaction to a death, swap, rung change, spawn, join or round
step); **C** better as a normal system with declared queries (per-tick
logic); **D** a static to move into a resource or asset; **S** (for
registries) a single-slot hook resource that must follow the gameplay
(keyed by game, chosen by `Gameplay`).

### 2.1 Callback registries (33 types, 87 fields)

| Registry | file:line | Fields (world-taking) | Called | Class | Note |
|---|---|---|---|---|---|
| `mode::Hooks` | chimera/sim/src/mode.rs:74 | `setup`, `round_over`, `round_end`, `before_spawns`, `after_round` | each round step | A | the mode's own steps, in the flow's order; keep as the mode's definition (data plus hooks, DESIGN "data-driven first"); other parts get events (step 9) |
| | mode.rs:80, :99 | `update`, `touches` | **every server tick** | C | per-tick logic behind one dispatcher (`mode/flow.rs:98`, `:105`); step 4 |
| | mode.rs:86 | `died` | each death | B | the same `Died` 31 systems read as a message; keep the hook's place in the flow (it runs before the counting), step 5 |
| | mode.rs:88, :90, :104, :109 | `may_respawn`, `holds_joiner`, `decide`, `warmup_end` | decisions, per step | A | pure decisions (`decide` writes the result); fine as data hooks |
| `mode::Mode` | mode.rs:135 | `on_death`, `time_out`, `match_end` | per counted death, round end | A | rules of the mode, ordered by the flow |
| `mode::Flow` | mode.rs:206 | `players` | every step | C (low) | `&mut World` only to build queries; CA's (`rules/flow.rs:85`) is a raw query each step; a kept state is enough |
| | mode.rs:214, :216 | `respawn`, `length` | per round | A | |
| | mode.rs:218 | `event` (`FlowEvent`) | each round step | B | the game's work at a step; also triggered as `ModeStep` for others to observe (step 9, done) |
| `mode::ModeGame` | mode.rs:263 | `chosen` | per step | A | |
| `minigame::RulesModule` | chimera/sim/src/minigame.rs:121 | `start`, `loadout`, `no_kit` | start, each life | A | |
| | minigame.rs:138, :142 | `joined`, `arming` | per join, per life | B | step 8 |
| | | `status` | gone | | now `hud::MinigameStatus`, a component `ladder::write_status` writes when a line changes (step 7, done) |
| | minigame.rs:147, :150 | `notice`, `callout` | per event | A | pure `&World` functions |
| `minigame::Modifier` | minigame.rs:155 | `apply` | start | A | |
| `ladder::RungExtrasHook` | chimera/sim/src/minigame/ladder.rs:392 | `draw` | per ladder | A | seeded draw, host only |
| | ladder.rs:404 | `apply` | per arming | A (B later) | DESIGN keeps it for what replicates anyway; it could become a host observer of `On<Insert<OnRung>>`, but `arm` runs it *before* `OnRung` is inserted (ladder.rs:730-737) and before the weapons are given, so moving it changes order: not planned |
| `rule_module::RuleModule` | chimera/sim/src/rule_module.rs:19 | `applies` | map load, cvars | A | |
| `server_docs::DocApply` | chimera/sim/src/server_docs.rs:48 | `With` | console | A | |
| `weapon::grenade::LaunchHook` | chimera/sim/src/weapon/grenade.rs:577 | (alias) | per throw (server) | B | a per-weapon behaviour as a world function; HL2's SMG grenade, crossbow bolt and RPG are all launches; step 3 |
| `weapon::params::ParamCodec` | chimera/sim/src/weapon/params.rs:32 | `encode`, `apply` | per weapon sent | A | network codec |
| `weapon::sync::PartCodec` | chimera/sim/src/weapon/sync.rs:41 | `encode`, `decode` | per tick per inventory | A | network codec, generic per component type |
| `core::PredictedComponent` | chimera/core/src/core.rs:1641 | `save`, `restore` | per replayed command per entity | A | generic over the type (E6); fine |
| `core::NetCodec` | core.rs:1654 | `encode`, `decode` | per tick | A | generic codec |
| `core::NewGame` | core.rs:1432 | (tuple) | map start | **S** | inserted by CS:S (`games/cs_source/src/rules.rs:80`) and Combat Arms (`games/combat_arms/src/game/rules.rs:629`): last wins |
| `game_log::LogSubjects` | chimera/core/src/game_log.rs:137 | (tuple) | per log line | **S** | one per app (`chimera/net/src/net/server.rs:104`) |
| `slots::MovementImpl` | chimera/core/src/slots.rs:28 | `insert`, `remove` (`EntityWorldMut`), `footing`, `seat` | per movement swap | A | per entity, keyed by id: already a good seam |
| `net::OwnPart` | chimera/net/src/net/parts.rs:92 | `encode`, `decode` | per tick per client | A | codec, sorted by name |
| `net::NetHooks` | parts.rs:175 | `stopped_serving`, `manifest`, `accept` | one-off | A | |
| | parts.rs:183 | `replay` | **per replayed command** | A (watch) | Source movers' `place` (`source/logic/src/logic/net.rs:389`, 3 raw queries, clones `MapEntities`); moving it into a schedule is risky for little gain: not planned, but cache its queries |
| `net::JoinRules` | parts.rs:196 | `joined`, `enter`, `choice` | per join | **S** | inserted by both games (`games/cs_source/src/game/net.rs:201`, `games/combat_arms/src/game/net.rs:177`) |
| `net::BotHooks` | parts.rs:290 | `kick`, `sync_quota` | console, serving | **S** | CS:S's (`cs_source/src/game/net.rs:206`) |
| `net::Holds` | parts.rs:313 | `freeze_ends` (and `frozen`, not world-taking) | **every server tick** (`server.rs:924`), per command | **S** | both games insert one |
| `net::VoiceRules` | chimera/net/src/net/voice.rs:88 | `can_hear` | per voice packet | A | one per app, but voice rules are the app's, not a game's |
| `net::VoteIssue` | chimera/net/src/net/vote.rs:188 | `check`, `run`, `choices` | console, votes | A | |
| `minigame_net::NextMatchHooks` | chimera/net/src/minigame_net.rs:832 | (tuple) | match change | A | |
| `map::MapHooks` | chimera/world/src/map/mod.rs:284 | `unload`, `spawn`, `copied`, `prepare` | map load or unload | A | |
| `map::MapScopedResource` | map/mod.rs:308 | `remove` | unload | A | |
| `map::SpawnHazards` | chimera/world/src/map/hazards.rs:16 | (tuple) | map load | A | |
| `map::LogicInspect` | chimera/world/src/map/logic_view.rs:266 | 5 inspectors | tools | A | |
| `client::FrontEnd` | chimera/client/src/client/front_end.rs:18 | 6 transitions | one-off | A | shell to menus; a party app has one front end |
| `client::AppHooks` | chimera/client/src/client/mod.rs:367 | `unload_greybox`, `respawn_greybox` (and 3 loaders) | map load | A | app-level |
| `client::GameDebug` | chimera/client/src/client/debug_ui.rs:49 | `bot_state`, `round_status` | debug UI | A | tools |
| `combat_arms::CaMode` | games/combat_arms/src/game/rules.rs:291 | `left` | a player leaves | B | an `on_leave` observer exists already (rules.rs:630); step 8 |

Not world-taking but the same shape: `GameInfo` (5 fields,
`chimera/core/src/game_registry.rs:42-59`), `MapLoader`/`MapDressing`
(`chimera/world/src/map/sources.rs:163`, `:175`), `AppHooks`' three
loaders, `SurfaceNames` (`chimera/world/src/map/impact.rs:131`),
`FlashModel`/`FlashHearing` (`grenade.rs:424`, `:439`),
`Holds::frozen`, `CaMode::leaving`, `RulesModule::check`,
`Modifier::check`, `GameDebug::bot_skills` (A, load-time or pure). One
of them is single-slot: **`ViewAnimRules`**
(`chimera/sim/src/weapon/view_anim.rs:259`, draw and idle picks and
event reactions for weapons without sequence tables), inserted by
CS:S only today (`games/cs_source/src/game/view_anim.rs:292`). Half-Life
2 and TF2 are Source games with their own view-model rules, so it
must be per game (S, step 3). The `Arc<dyn Fn>` callbacks are A: the
console's `RunFn`/`GetFn`/`SetFn`/`CompleteFn`
(`chimera/core/src/console.rs:36-39`), weapon builds (`BuildWeapon`,
`HookBuild`: per weapon entity, already the target style), economy
`give`/`has`, the icon `Resolver`, `OpenMount`, the map offer's
`read`, `PrepareJob`, `MapLightField`.

Totals: A 24 types, S 5 (+ `ViewAnimRules`), B 2 whole types
(`LaunchHook`, `CaMode::left`) plus fields of `Hooks`, `Flow` and
`RulesModule` (B 5 fields), C 4 fields (`Hooks::update`, `touches`,
`Flow::players`, `RulesModule::status`).

### 2.2 Exclusive systems in per-tick and per-frame schedules (117)

By crate: Combat Arms 39, `chimera_net` 26, `chimera_sim` 13,
`cs_source` 12, `chimera_client` 8, `source_logic` 7, `party_kit` 5,
`chimera_core` 3, both dedicated servers 4, `chimera_world` 1. `q`:
raw queries in the function itself.

| file | systems (line, schedule, q) | Class | Note |
|---|---|---|---|
| apps/cs_source_server/src/main.rs | `finish_level_change` 269, `start_serving` 312 (Update) | A | server state changes |
| apps/combat_arms_server/src/{lib,main}.rs | `finish_level_change` lib:282, `start_serving` main:84 | A | |
| apps/party_kit/src/server.rs | `finish_level_change` 299, `start_serving` 403 | A | |
| apps/party_kit/src/{controls,esc_menu}.rs | `refresh_values` controls:316, esc_menu:318; `act` esc_menu:244 | A | menus |
| chimera/client/src/client/console.rs | `edit` 684, `overlay_text` 1304 (q1), `watch_text` 1420, `finish_map_load` 2030 (q1) | A | console, map load |
| chimera/client/src/client/debug_ui.rs | `draw` 483 (q1) | A | tool |
| chimera/client/src/client/net.rs | `react` 39 | A | connection state |
| chimera/client/src/client/hud.rs | `put_away_hud_look` 211 | C (low) | |
| chimera/client/src/client/night_vision.rs | `toggle` 105 (q1) | C (low) | per frame, the local player's lookup |
| chimera/core/src/console.rs | `run_queue` 497 | A | console |
| | `local_userinfo` 855 (q1, kept state) | C (low) | |
| chimera/core/src/game_log.rs | `write_lines` 317 (Last) | A | IO |
| chimera/net/src/minigame_net.rs | `send_offer` 398 (q1), `apply_minigame` 677 (q1), `serve_minigame` 765, `apply_protection` 882 (q2, PreUpdate, every frame) | A | net apply; `apply_protection`'s two raw queries per frame: kept states (step 11) |
| chimera/net/src/net/chat.rs | `rename_host` 427 (q1), `send_userinfo` 463 | A | |
| chimera/net/src/net/client.rs | `watch_join` 585, `watch_connection` 605, `check_map` 644 | A | |
| chimera/net/src/net/cvars.rs | `send_changes` 82 (q1), `receive_values` 107 | A | |
| chimera/net/src/net/{decals,motd,game}.rs | `send` decals:162, `send` motd:77, `apply_own` game:257 | A | |
| chimera/net/src/net/maps.rs | `watch_map` 357, `receive_chunks` 961 | A | |
| chimera/net/src/net/predict.rs | `start_command` 719 (FixedFirst), `record_state` 1055 (FixedLast) | A | prediction's generic save and restore |
| chimera/net/src/net/server.rs | `canonical_positions` 906 (q1), `capture_own_states` 917 (q1) (FixedLast, every server tick) | A | capture; raw queries filtered on the rare `OwnStateOut` (cheap); kept states anyway (step 11) |
| chimera/net/src/net/server_docs.rs | `receive_requests` 337, `send_uploads` 363 (q1), `receive_chunks` 433 | A | |
| chimera/net/src/net/{upnp,vote}.rs | `keep_mapping` upnp:161, `run_votes` vote:688, `show_news` vote:753 | A | |
| chimera/net/src/net/weapon_params.rs | `send_params` 67 (q1), `receive_params` 118 | A | |
| chimera/sim/src/minigame/hud.rs | `update` 224 (Update) | C | still exclusive (notices, callouts, moments); reads `MinigameStatus` (step 7, done) |
| chimera/sim/src/minigame/results.rs | `record` 148 | B | reacts to the match's end; now an observer of `ModeStep` (step 9, done) |
| chimera/sim/src/minigame/runtime.rs | `join` 367 (q1 kept), `respawn_dead` 429, `end_protection` 448 (kept), `hold_the_end` 600 (q1) | C (`join` B) | step 6 |
| | `step_phases` 394, `handle_deaths` 488 | A | the flow's driver and its death step: they call the mode's hooks in order; keep |
| chimera/sim/src/mode/flow.rs | `update` 98, `touches` 105 | C | dispatchers of the mode's per-tick hooks; step 4 |
| chimera/sim/src/weapon/equip.rs | `map_equipment` 170 | A | map's spawn equipment |
| | `remove_killed_weapons` 190 (q2) | B | reacts to deaths; step 5 |
| chimera/sim/src/weapon/mod.rs | `give_starting_weapons` 620 (kept) | B | polls for characters without an `Inventory` each tick; the giving stays world access (it spawns weapons); step 5 |
| chimera/world/src/map/breakables.rs | `round_restart` 982 (q2) | B | reacts to a restart; now an observer of `core::RoundRestart` (step 9, done) |
| games/combat_arms/src/game/bots/brain.rs | `ensure_nav` 384 (q3) | A | builds the nav once a map is in |
| | `think` 624 | C (parked) | the brain (E3); bots are parked |
| games/combat_arms/src/game/bots/roster.rs | `arm` 136 (q1 kept) | B | re-arms on a level or rung change; step 5 |
| | `keep_quota` 203 (kept) | A | spawning |
| games/combat_arms/src/game/characters.rs | `client_clock` 1104 | C (low) | |
| games/combat_arms/src/game/chat.rs | `serve_standalone` 686, `hear_chat` 791, `hear_radio` 811, `hear_services` 838, `housekeeping` 857 | A | messaging IO |
| games/combat_arms/src/game/death_cam.rs | `eligibility` 449 | C (low) | |
| games/combat_arms/src/game/lobby/host.rs | `serve_standalone` 284, `client_frame` 296, `host_frame` 322 | A | lobby state machine |
| games/combat_arms/src/game/music/mod.rs | `gather` 173 (q1) | C | per frame |
| games/combat_arms/src/game/music/trigger.rs | `late_join` 109 (kept) | B | now an observer of `On<Add<Player>>` (step 9, done) |
| games/combat_arms/src/game/net.rs | `write_match` 319 (q1), `apply_match` 387 (q1) | A | net |
| games/combat_arms/src/game/objects/mod.rs | `load` 441 (q1) | A | map objects |
| | `frame` 530, `gather` 575, `apply` 645 | C | mission objects per tick |
| games/combat_arms/src/game/objects/net.rs | `write` 79 (q1), `read` 125 (q1) | A | net |
| games/combat_arms/src/game/options/flow.rs | `save_at_exit` 80, `load` 91 | A | IO |
| games/combat_arms/src/game/perks.rs | `perk_tick` 786 (kept), `supplies` 1158 (q3) | C | per tick |
| | `perk_deaths` 938 | B | step 5 |
| games/combat_arms/src/game/radar/mod.rs | `send_round` 231 | A | net |
| games/combat_arms/src/game/rules.rs | `publish` 466, `update_team_counts` 490 | C | |
| games/combat_arms/src/game/rules/mercs.rs | `expire` 315 (q2) | C | |
| games/combat_arms/src/game/rules/scoring.rs | `handle_deaths` 126 | B | step 5 |
| games/combat_arms/src/game/rules/spawns.rs | `ask_for_respawns` 461 (q1) | C | |
| games/combat_arms/src/ui/{lobby,options,scoreboard,talk}.rs | `update` lobby:1289, options:910, scoreboard:174 (q1), talk:150; `typing` talk:251 | A | menus and HUD panels |
| games/cs_source/src/bot/mod.rs | `keep_quota` 563 (q1) | A | spawning |
| games/cs_source/src/game/demo.rs | `play` 1263, `apply_round` 1817 | A | demo playback |
| games/cs_source/src/game/net.rs | `write_round` 449 (q1), `apply_round` 838 (q2), `apply_bombs` 909 (q1) | A | net |
| games/cs_source/src/game/votes.rs | `bot_ballots` 105 | C (low) | |
| games/cs_source/src/harness.rs | `load_requested_maps` 117 | A | |
| games/cs_source/src/objectives/bomb.rs | `arm` 456 (q1, **`Predict::Weapons`**) | C | runs per replayed command on a client; a raw query over every character archetype; step 3 |
| games/cs_source/src/rules.rs | `respawn` 673 (q1) | C | |
| games/cs_source/src/rules/rounds.rs | `run_rounds` 568 (q1) | A | CS:S's round driver around `flow::step_game`; keep |
| games/cs_source/src/ui/{demo,spectator_menu}.rs | `start_when_loaded` demo:95, `read_cvars` spectator_menu:365 | A | |
| source/logic/src/logic/bridge.rs | `load` 289 (q1), `pre` 2285, `post` 2296, `damage` 2359 | A | the map-logic interpreter's I/O; needs the world |
| | `follow_bodies` 2138 (FixedPostUpdate) | C (low) | |
| source/logic/src/logic/net.rs | `write_movers` 222 (q1), `place` 389 (q3, also `NetHooks::replay`) | A | net |

Totals: A 82, B 9, C 26. Of the C rows, 8 are "low" or parked (small, local, no
seam). The ones a new game copies are the mode dispatchers, the
minigame runtime, the HUD's status, `bomb::arm` (the pattern for
HL2's and TF2's predicted objectives) and Combat Arms' per-tick rules
(`publish`, `update_team_counts`, `mercs::expire`,
`ask_for_respawns`).

The per-tick work reached through registries is also exclusive:
about 20 modes' `update`/`touches` (Combat Arms 13 mode files, the
ladder, CS:S's rounds), the ladder's `update` (`refill`, `follow_lead`,
ladder.rs:804) and the movers' replay. Combat Arms' clan modes wrap
another mode's hook to add to it (`clan_update(world, base)`,
`games/combat_arms/src/game/rules/clan.rs:192`): hooks don't compose,
and systems would.

### 2.3 Process-wide mutable statics (45)

`checks/tests/it/determinism.rs` ratchets them in the simulation crates
(31 today: core 2, world 1, CS:S 6, Combat Arms 22). The rest are in
the client and presentation crates, which it doesn't scan. It also
scans only the crates in its `BASELINE`: a new game crate would not be
checked at all (step 1 fixes this).

| Static | file:line | Class | Why it matters |
|---|---|---|---|
| `TIMES`, `STACK` (thread-local) | chimera/client/src/client/frame_metrics.rs:631, :634 | A | profiling, process-wide by nature |
| `ACTIVE`, `NEXT_THREAD`, `EPOCH`, `BUF`, thread-local | chimera/client/src/client/trace_capture.rs:41, :42, :59, :64, :68 | A | tracing layer |
| `HW_STATUS` | chimera/core/src/metrics/counters.rs:83 | A | hardware counters |
| `LOG` | chimera/client/src/client/console.rs:1709 | A | the log layer |
| `STEAM_FOUND` | chimera/core/src/mount/install.rs:470 | A | the machine's installs |
| `ON`, `DEBUG`, `DEBUG` | games/cs_source/src/game/movement.rs:1306, :1535, :1763 | A | environment debug flags |
| `SOURCE_LDR` | chimera/present/src/render/images.rs:43 | A | constant table |
| `DEFAULT_RECOIL`, `MALE`, `FEMALE`, `SET` (surfaces) | games/combat_arms/src/game/weapons.rs:1018, :1206, :1208, :1668 | A | built-in tables |
| `INTERNED` | games/combat_arms/src/game/weapons.rs:1371 | A | an interner (leaks by design, as weapon ids) |
| `T` (render styles) | games/combat_arms/src/game/render_style.rs:50 | A | read-only table (E2) |
| `DATA` (movement), `TABLE` (attachments), `DATA` (music) | games/combat_arms/src/game/movement.rs:549, attachments.rs:459, music/data.rs:141 | A | read-only tables; a resource only if they are to hot-reload |
| `SET`, `NONE` (fx tables) | games/combat_arms/src/game/effects/data.rs:225, :235 | A | read-only |
| `DATA_ONCE` (lobby) | games/combat_arms/src/game/lobby/data.rs:255 | A | read-only |
| `LISTS` (anim lists) | games/combat_arms/src/game/characters/driver.rs:968 | A | read-only (E2) |
| `BUILTIN` (`CaData`) | games/combat_arms/src/game/data/store.rs:119 | A | the built-in data, the same for every app |
| `SETTINGS` (rounds) | games/cs_source/src/rules/rounds.rs:153 | A | defaults |
| `SET`, `DOC` (CS:S weapons, economy) | games/cs_source/src/game/weapons.rs:131, :405 | A | built-in, the set leaked by design |
| `INSTALL` (`CaData`) | games/combat_arms/src/game/data/store.rs:133 | D (later) | install-derived; one install per machine, so sharing is correct, but it never frees |
| `LIBRARY`, `PARSED` (effects) | games/combat_arms/src/game/effects/library.rs:77, :259 | D (later) | loader-filled memo (E2 left): needs the loader to get the app's context |
| `MOUNT` | games/combat_arms/src/game/world.rs:279 | D (later) | the same |
| `RIGS`, `GROUPS`, `SHARED` | games/combat_arms/src/game/characters/models.rs:87, :162, :470 | D (later) | model caches, never freed |
| `CACHE` (view models) | games/combat_arms/src/game/view_model/on_demand.rs:64 | D (later) | "never freed" (tech-debt.md) |
| `UI` (install UI) | games/combat_arms/src/ui/install.rs:702 | D (later) | front end only |
| `GAME_DEFAULTS`, `OLD_DEFAULTS` | chimera/client/src/client/binds.rs:189, :193 | gone | now `binds::BindDefaults`, a resource keyed by game (step 10, done); a gameplay switch still doesn't re-read binds (known shortcut) |
| `MAP_NAMES`, `CYCLE` | chimera/client/src/client/console.rs:1890, :1923 | gone | now `console::MapLists`, a resource keyed by game (step 10, done) |
| `PROGRESS` | chimera/world/src/map/loading.rs:16 | gone | now `loading::LoadReporter`, a resource the load also holds through `MapLoadOptions::progress` (step 10, done) |

Totals: A 31 (13 diagnostics and flags, 18 read-only tables or
interners), D 14 (9 install memos for later, 5 per-app state now).

Leaks of the same kind (`Box::leak`): weapon sets
(`chimera/sim/src/weapon/data.rs:784`, `:850`, `:1156`, CS:S's and
Combat Arms' built-in sets), minigame definitions
(`chimera/sim/src/minigame.rs:211`, `:428`), auto gun-game ids
(`minigame/auto.rs:472`, `:490`), party host cvar names
(`apps/party_kit/src/host.rs:171`). These are by design (`&'static`
ids). They only matter for hot reload, which isn't planned.

### 2.4 Ad-hoc `world.query` in per-tick paths

240 raw sites in non-test code (Combat Arms 64, CS:S 57, `chimera_net`
48, `chimera_client` 22, `chimera_sim` 19, `source_logic` 14, others 16)
and 38 kept states. Each raw call builds a `QueryState` that walks every
archetype holding the query's rarest component: 2,250 to 3,000 in a
party match, growing with map changes (docs/performance.md, "Map
changes that slowed the tick").

- Directly in the 117 per-tick systems: 55. Kept states already cover
  what profiles named: the minigame runtime's `players`, `join`,
  `end_protection`, the logic bridge's syncs, the bots' quota and arming,
  `give_starting_weapons`, the perks, the HUD, moments and ladder.
- Reachable per tick by name: at most 142 in 111 functions. Most are on
  rare branches: a death, a round restart, a join, a vote, a map
  change.
- Every tick or frame, still raw: `server::canonical_positions` and
  `capture_own_states` (server, `OwnStateOut` filter: few archetypes),
  `minigame_net::apply_protection` (client, 2), CS:S's `bomb::arm`
  (characters' archetypes, per replay too), `net::write_round`/
  `apply_round`, Combat Arms' `net::write_match`/`apply_match`,
  `objects::net::write`/`read`, `mercs::expire`, `perks::supplies`,
  `rules/flow.rs` `players`/`alive`/`alive_players` (each step), the
  modes' `hunted`, `quarantine`, `search_destroy` updates, the movers'
  `place` (3, per replay too), `music::gather` (per frame).

Converting a per-tick system to declared `Query` params makes the
schedule keep its state, the same effect as a kept state, plus
declared access.

## 3. Classification summary

| Class | Registries (fields) | Per-tick exclusive systems | Statics |
|---|---|---|---|
| A fine | 24 types | 82 | 31 |
| B observer or message | 2 types + 5 fields | 9 | - |
| C normal system | 4 fields | 26 (8 low or parked) | - |
| D into a resource or asset | - | - | 14 (5 now, 9 later) |
| S follow the gameplay | 6 types | - | - |

Rules of thumb used (and proposed for DESIGN.md, section 6):

- A fact about an entity that others react to (a rung, an identity, a
  protection, a held kit) is a **component**. It is put on where the
  fact becomes true, on every machine that needs it (derived locally
  from replicated state, as `OnRung`). Reactions are **observers**
  (`On<Insert<C>>`, `On<Discard<C>>`).
- A per-tick flow (hits, deaths, weapon events) is a **message** read by
  normal systems in the tick's order (E5: 172 registered).
- A one-off moment at a known point in exclusive code (a round's end, a
  join, a launch) is an **event** triggered there with `world.trigger`.
  The observers run at once, in the same place the hook ran.
- A per-tick piece of a game's or mode's logic is a **system** with
  declared queries, gated by a run condition (`gameplay_is`, `hud_is`,
  a new `mode_is`). Never gate with a new system set: Bevy's set
  flattening reorders unordered systems (`chimera_core::gameplay`'s doc).
- A **decision** the runtime asks at a known point (may respawn, match
  end, loadout) stays a data hook, `fn(&World, ..) -> answer`.
- Whole-world access stays where the job is the world: map loading,
  spawning weapons and characters, network apply and capture, codecs,
  the console, menus, tools.

## 4. Steps

Each step is small, merges alone, and keeps every test. "Guards" names
what must pass, beyond each crate's fast tier. The guards:

- the schedule ratchets (`games/cs_source/tests/it/schedules.rs`,
  `games/combat_arms/tests/it/schedules.rs`): ambiguous pairs and
  server/client opposite pairs may only fall;
- system dumps before and after (`CHIMERA_SYSTEM_DUMP`,
  docs/OBSERVABILITY.md);
- the listen server (`apps/party_kit/tests/it/listen_server.rs`) and the
  party's network tests (`net_party.rs`, `net_gun_game.rs`);
- the network prediction suites (`games/cs_source/tests/it/
  net_prediction.rs`, `prediction.rs`; `games/combat_arms/tests/it/
  net.rs`, `net_modes.rs`, `net_modes_2.rs`);
- the determinism checks (`checks/tests/it/determinism.rs`) and
  `games/combat_arms/tests/it/data_isolation.rs`;
- performance: `perfgate` (`chimera/tools/src/perfgate.rs`,
  `tools/perfgate/baseline.json`) and `party_kit`'s `heavy::bot_cost`
  and `archetypes`, compared by instructions per tick before and after.

**Behaviour or order flags.** These apply to every conversion:

- (O1) A normal system's `Commands` apply at the next sync point. An
  exclusive system's writes are visible to the next system at once.
  Write through `Query<&mut T>` where a later system in the same tick
  reads the result, or order it explicitly.
- (O2) Observers triggered by `world.trigger` inside exclusive code run
  at once, where the hook ran. Triggered through `Commands`, they run at
  the next sync point, which is later. Use the former to keep order.
- (O3) Several observers of one event have no order we may rely on. A
  reaction that must come before another stays in one observer or a
  chain.
- (O4) Every added system can reorder unordered neighbours on the
  single-threaded executors. Add into the existing tuple and `.chain()`
  where the old dispatcher was, gate with run conditions, never a new
  set, and diff the system dumps.

### Step 1: guards and the rule (S; no behaviour change)

- A new `checks/tests/it/ecs.rs` (section 6): registries on an allowed
  list, and a per-crate ratchet of exclusive systems in per-tick
  schedules. Today's counts are the baseline.
- `determinism.rs` scans every `chimera/*/src`, `source/*/src` and
  `games/*/src` simulation crate, not only those in `BASELINE`. A crate
  not listed starts at 0, so `games/hl2dm` would start clean.
- DESIGN.md: the "ECS style" text (section 6).
- Guards: `cargo test -p repo_checks`. Effect: nothing gets worse while
  the steps land, and a new game crate starts in the target style.
- **Done 2026-10-29.** `ecs.rs`: 40 registry types allowed (section
  2.1's 33, the four console `Arc<dyn Fn>` types, `HookBuild`,
  `BuildWeapon`, `ItemPart`), S/B/C rows naming their step. Per-tick
  exclusive systems found: 110 (section 2.2's 117 less the 7 in `ui`
  files, which both scans skip); 70 in the 34 files of
  `ALLOWED_EXCLUSIVE_FILES` don't count; stored: `chimera_client` 1
  (`put_away_hud_look`), `chimera_sim` 13, `chimera_world` 1, Combat
  Arms 20, CS:S 5 (`chimera_core`'s `local_userinfo` is in the allowed
  `console.rs`). Each conversion lowers its crate's number.
  `determinism.rs` finds the crates (`chimera/*`, `source/*`,
  `lithtech/*`, `games/*` less `NOT_SIMULATION`); new rows: the two
  format crates (8 std hash uses each); `games/originals` and
  `chimera/vfs` are held at 0 without one.

### Step 2: single-slot hooks keyed by game (S-M; behaviour flag)

- `JoinRules`, `Holds`, `BotHooks`, `NewGame`, `LogSubjects` become one
  entry per game (`GameHooks<T>`: a map from game id to `T`, registered
  with `app.game_hook::<T>(game, hooks)`). Readers take the entry of
  `Gameplay::rules` (`BotHooks`: `Gameplay::bots`), with the default
  when there is none. Files: `chimera/net/src/net/parts.rs`,
  `chimera/net/src/net/server.rs:713`, `:747`, `:924`,
  `chimera/net/src/net/predict.rs:976`, `:1009`,
  `chimera/core/src/core.rs:1432-1440`, `chimera/core/src/game_log.rs`,
  both games' `net.rs` and `rules.rs`.
- **Flag: behaviour.** In an app with both games' parts (the party,
  the mashup, Ultimate Gun Game) the hooks used become the gameplay's,
  not the last plugin's. Check that each app's chosen gameplay gave the
  same hooks before (most likely; the plugin order was picked so).
  `switch_gameplay` then switches them, which removes one of
  `chimera_core::gameplay`'s known shortcuts.
- Guards: listen server, `net_party.rs`, both games' net suites,
  `apps/mashup_demo/tests/it/two_games.rs`.
- Effect: HL2DM and TF2 register their own join, hold and new-game
  rules without replacing CS:S's. 5 S registries become keyed.
- **Done 2026-10-30.** `chimera_core::gameplay::GameHooks<T>` (an entry
  per game id plus an `any` entry), `GameplayAppExt::game_hook`/
  `any_game_hook`, `gameplay::active_hook` (the entry of the gameplay's
  slot game, else `any`, else none: the caller's default; without a
  `Gameplay`, the first game's registered, as `default_gameplay` makes
  the first game's parts play). The network's `JoinRules::active`,
  `Holds::active` (rules' slot) and `BotHooks::active` (bots' slot);
  `predict`'s two systems read `GameHooks<Holds>` with the gameplay.
  `NewGame` and `new_game` moved from `core` to `gameplay` (core can't
  use gameplay: `module_layering`); `game_log` and CS:S's `rules` may now
  use `gameplay` (ARCHITECTURE.md's edges). The network registers its
  `LogSubjects` as `any`. CS:S and Combat Arms register theirs under
  their ids; none inserts one as a resource
  (`ecs.rs::keyed_hooks_are_not_single_slot_resources`). A minigame's
  rules (`MINIGAME`) register their own: `NewGame` counts the map load
  (`MinigamePlugin`), `Holds` keeps the look only
  (`runtime::end_frozen`, in `minigame_net`'s part); its join is the
  default. Behaviour, checked per app:
  - Single-game apps (CS:S, Combat Arms, their servers): unchanged (one
    game's entries, and it is the gameplay).
  - The mashup demo: unchanged (only Combat Arms' rules and network are
    in it). With both games' parts (`two_games.rs`), the hooks were the
    last plugin's (CS:S's: its network part sorts after Combat Arms',
    and `CsSourceParts` was added second); now Combat Arms' (the
    gameplay), CS:S's after a switch, in either plugin order
    (`each_hook_is_the_gameplays_in_either_plugin_order`).
  - The party (`party_kit`, so Lucker Party and Ultimate Gun Game):
    before a minigame, Combat Arms' (as before). While one plays (rules
    `chimera`), the hooks were Combat Arms' (the only game inserting
    them); now the minigame's: a map load no longer runs Combat Arms'
    `new_game` (which restarted `mode::Match`, the minigame's match
    resource too; the party server restarts the minigame itself after
    it) but still counts `MapLoads` (a joining client waits on it); a
    client's predicted freeze (the match's end) keeps the look only, as
    the server's `hold_the_end` does, where Combat Arms' kept firing and
    weapon use; a joiner comes in by the default `enter_at_spawn`, which
    is what Combat Arms' `enter_game` did under `ca_mode none` without
    the lobby. Bots stay Combat Arms' (the bots slot)
    (`apps/party_kit/tests/it/game_hooks.rs`).
  - Tests: `chimera_core` gameplay unit test (both orders),
    `chimera/net/tests/it/game_hooks.rs` (two stand-in games, either
    order: a network join, a bot kick, the freeze, a new game, the log;
    a switch switches each; a game with none gets the defaults).
  - Registries: the five stay plain `fn` data on `ALLOWED_REGISTRIES`
    (re-reasoned "keyed by game"); no exclusive system changed, so the
    ratchet counts are the same.

### Step 3: the weapon seam (S-M)

- `ViewAnimRules` per game (keyed by the weapon's game id, the prefix of
  its data id). The driver reads the entry of the held weapon's game
  (`chimera/sim/src/weapon/view_anim.rs:321`). HL2's and TF2's
  Source view models plug in beside CS:S's.
- `LaunchHook` becomes an `EntityEvent` (`Launch`) on the weapon,
  triggered where the hook was called. The game's observer filters by
  its own weapon component. Several games' launches then live side by
  side without a field per weapon. Flag O2: trigger with
  `world.trigger` in the throw's exclusive code.
- CS:S's `bomb::arm` (`games/cs_source/src/objectives/bomb.rs:456`)
  becomes a normal system with declared queries in `Predict::Weapons`.
  It runs per replayed command, so its query state is now kept. Flag O1:
  the plant spawns the bomb with `Commands` only where authoritative,
  and prediction compares state after the command.
- Guards: `games/cs_source/tests/it/net_prediction.rs`,
  `net_weapons.rs`, `weapons.rs`, the bomb's scenario tests, Combat
  Arms' `grenades.rs`, the schedule ratchets (`Predict::Weapons` must
  keep 0 opposite pairs).
- Effect: the weapon frame's three exceptions are gone. The pattern for
  HL2's predicted objectives (TF2's carts and points) is a system, not a
  world function.
- **Done 2026-10-30.** `view_anim::GameViewAnimRules` (a list of
  (game id, `ViewAnimRules`), registered with `ViewAnimAppExt::
  view_anim_rules(game, rules)`; a game registering again replaces its
  own entry); `drive` takes the entry of the held weapon's id prefix
  (draw, idle, events and animation effects); a weapon of a game without
  rules shows its reference pose. CS:S's `view_anim::add_rules` (its
  plugin, the party, the mashup). `grenade::Launch` is an `EntityEvent`
  on the weapon, `ThrowOptions::launch` became `game_launch: bool`; the
  throw triggers it with `commands.trigger` where the hook's
  `commands.queue` was (the throw was already a normal system, so the
  observers run where the hook ran: in the same command queue, O2), on
  the server only. Combat Arms' grenades and devices each observe it
  filtered by their own part (`CaGrenadeData`, `CaDeviceData`) and queue
  their unchanged `launch`. `bomb::arm` is a normal system (declared
  queries, an `ArmMessages` param) in the same place; `Arming`, the
  bomb's despawn and the planted bomb go through `Commands` (applied
  before `predict_defuse`, which is chained after, and by the end of the
  command's schedule, where prediction saves the state: O1); the
  inventory changes in place. Tests: `chimera_sim` `view_anim::
  two_games_rules_each_play_their_own_weapons_in_either_order`,
  `a_game_without_rules_leaves_its_weapons_alone`; Combat Arms
  `net_projectiles::net_a_throw_launches_once_on_the_server_and_never_
  in_prediction`; the bomb's unchanged `ui` tests (`objectives::`,
  `net_rules::arming_and_defusing_are_predicted`: 0 prediction errors
  at 100 ms). `Predict::Weapons` ambiguities unchanged (9, 0 opposite).
  `ecs.rs`: `LaunchHook` off the allowed list; CS:S ratchet 5 to 4.

### Step 4: a mode's per-tick hooks as systems (M; order flag)

- A `mode_is(id)` run condition (the playing mode,
  `mode::current_mode`). Each mode's `update` and `touches` are
  registered as systems with declared params in the tuple where
  `flow::update` and `flow::touches` ran, chained in that place and
  gated by `mode_is`. `Hooks::update`/`touches` and the two dispatchers
  go. Combat Arms' clan modes compose by chaining their system after the
  base mode's instead of wrapping it (`clan_update`).
- Files: `chimera/sim/src/mode.rs`, `mode/flow.rs`, `minigame/ladder.rs`
  (`update`: `refill`, `follow_lead`), the Combat Arms mode files under
  `games/combat_arms/src/game/rules/` (13), `games/cs_source/src/rules/
  rounds.rs`.
- Do it one game per commit: the ladder first (Chimera's), then Combat
  Arms in two halves.
- **Flag: order.** O1 and O4. A mode's update today writes the world
  immediately, before the phases step in the same tick. Keep the systems
  chained before `step_phases`; where an update spawns or despawns,
  keep that part exclusive (a one-shot system it calls) or flush with
  an explicit `ApplyDeferred` before the step.
- Guards: `games/combat_arms/tests/it/game_modes.rs`,
  `game_modes_followups.rs`, `capture_the_flag.rs`, `clan_modes.rs`,
  `search_destroy.rs`, `vip_escort.rs`, `hired_guns.rs`,
  `other_modes.rs`, `net_modes*.rs`; `chimera/sim/tests/it/mode.rs`,
  `minigame.rs`; `apps/party_kit/tests/it/gun_game.rs`; schedule
  ratchets; system dumps; `perfgate` and `bot_cost` (about 20 more
  systems a tick, most gated off: about 20 µs of executor work by the
  third pass's per-system cost, docs/performance.md).
- Effect: about 20 world functions become systems with declared
  access. A TF2 mode is a system set up like any other.
- **Done 2026-10-30.** `mode::mode_is(id)` and `mode_in(ids)` (a mode
  and its variants sharing a system): run conditions on the playing mode
  (`current_mode`). `Hooks::update`/`touches` and `flow::update`/
  `touches` are gone. The ladder: `refill`, `follow_lead`
  (`ladder::per_tick`, in the minigame runtime's chain after `join`,
  before `step_phases`). Combat Arms: every mode's update and touches
  are normal systems with declared queries in the rules' chain where
  the dispatchers ran (`rules::mode_updates` between
  `update_team_counts` and `flow::mission_update`, `mode_touches`
  between `scoring::handle_deaths` and `publish`), gated by `mode_is`/
  `mode_in` (S&D's also play in Go4CA and Clan S&D, CTF's in Clan CTF,
  Seize's in Squad Match 2, VIP Escort's in Clan VIP Escort) and chained
  with `chain_ignore_deferred` (one mode plays at a time; a mode's own
  steps keep a sync point: `hand_out` then `bombs`, `carry` then
  `bombs`). Clan modes compose by chaining (`clan::ctf_half`,
  `leaver_ends_round` after the base mode's systems), not by wrapping
  (`clan_update` gone). Shared pieces in `rules::objectives`: `Roster`
  (players, living, takers, alive from declared queries), `Edits<T>`
  (component writes a system reads back in the same run, applied at
  its end), the `_in` helpers (carry, gauges, statistics, weapons, the
  floor ray, draws); `missions::seize_in`, `activate_time_in`,
  `spy_hunt_in`, `quarantine_in`; `bomb::Bombs`. World work stays world
  work, queued through `Commands` in order and applied at the sync
  point before the step (O1): building and taking weapons (Arms Race's
  step, Hunted's fist, S&D's hand-out, a Bombing Run pick-up,
  Quarantine's kits), VIP ranking, Spy Hunt's stage 2. Where a mode read
  back in the same tick what it wrote (Quarantine's hosts counted
  infected by that tick's kits, heal and win test; Spy Hunt's free ids;
  CTF's take then capture; Hired Guns' pockets), the system keeps its
  writes (`Edits`, local lists) so the tick sees what it saw before.
  **Order flag (O4), checked:** the rules' chain keeps its order
  (`schedules::mode_systems_run_where_the_dispatchers_ran`, server and
  client); the single-threaded sort moved unordered neighbours that
  were already unordered with the dispatchers (the weapon chain's
  `pick_up`, `use_pick_up`, `debug_pickups`, the map's `record_poses`,
  `pose_hitboxes`, the client's `hold_local`, `bots::roster::
  keep_quota`, `read_map_objects`, a few more: `FixedUpdate` orders
  dumped before and after and compared); every Combat Arms, Chimera
  sim, network, CS:S and party test passes. The mode systems are
  `ambiguous_with` the client's `characters::client_clock` (it runs
  only where the rules don't). Ratchets: Combat Arms' `FixedUpdate`
  ambiguous pairs 718 to 662, server/client opposite pairs 253 to 149;
  `ecs.rs` `chimera/sim` 13 to 11 (Combat Arms' count is unchanged: its
  modes' hooks weren't counted and none of their systems is exclusive).
  Tests: `chimera_sim` `mode::a_modes_tick_runs_only_while_it_plays_
  before_the_step`, `minigame::the_ladders_tick_refills_each_tick_after_
  the_join_and_before_the_step`; Combat Arms `schedules::mode_systems_
  run_where_the_dispatchers_ran`, `free_for_all::a_modes_tick_runs_only_
  in_its_mode`. Combat Arms in one commit, not two halves: half the
  modes alone kept the dispatcher's pairs and added the systems', over
  the ratchet. Not in this step (the plan's other steps): `Hooks::died`
  (step 5), `decide` (a decision hook, kept), `Flow::players` (a kept
  state, later).

### Step 5: death and spawn pollers (S each)

- `remove_killed_weapons` (`chimera/sim/src/weapon/equip.rs:190`),
  `perk_deaths` (`games/combat_arms/src/game/perks.rs:938`),
  `scoring::handle_deaths` (`games/combat_arms/src/game/rules/
  scoring.rs:126`): normal systems reading `Died` (`MessageReader`),
  in the same place in the tick.
- `give_starting_weapons` (`chimera/sim/src/weapon/mod.rs:620`): an
  observer on a character's `Inventory` being needed (`On<Add<Health>>`
  on an `Intent` holder, or a `NeedsKit` marker the spawn adds) runs
  the giving, which stays world access. **Flag:** today the weapons
  arrive on the first tick after the spawn; an observer gives them at
  once (O2). Check the join and respawn tests before choosing. If any
  test depends on the tick, keep a normal system that reads the
  marker.
- The bot roster's `arm` (`games/combat_arms/src/game/bots/roster.rs:
  136`): a reaction to `OnRung`/level changes.
- Guards: `games/cs_source/tests/it/weapons.rs`, Combat Arms'
  `gear_perks.rs`, `free_for_all.rs`, `bots.rs`; `chimera/sim/tests/
  it/spawns.rs`; listen server.
- Effect: 5 exclusive systems fewer. Deaths and spawns are one pattern
  for every game.

### Step 6: the minigame runtime's per-tick work (S-M)

- `end_protection`, `respawn_dead`, `hold_the_end` as normal systems
  (`chimera/sim/src/minigame/runtime.rs:429`, `:448`, `:600`). `join`
  (`:367`) becomes an observer of the joining moment
  (`On<Add<Team>>` exists for numbering: `number_players`, :634).
- **Flag O1:** `unprotect` removes components that the same tick's
  damage reads. Keep the order with `.chain()` before damage, and
  remove the components with `Commands` before a sync point that
  damage follows (check `protection_look.rs`, `swap_effect.rs`).
- Guards: `apps/party_kit/tests/it/gun_game.rs`, `protection_look.rs`,
  `rotation.rs`, `net_gun_game.rs`; `chimera/sim/tests/it/minigame.rs`.
- Effect: the runtime that every minigame of every game runs on uses
  declared queries.
- **Done 2026-10-30.** `join`, `respawn_dead`, `end_protection` and
  `hold_the_end` are normal systems with declared queries in the same
  chained tuple (`runtime::plugin`), same places. `join` stayed a system,
  not an observer: the joining moment is several (a team given, a
  connection's end, a spectator's return, the minigame's start, a hold
  lifted), and its `Without<InMinigame>` query is an archetype filter, so
  as cheap as an observer; it finds the joiners (in the query's order, as
  before) and queues their taking in (the mode's team, the rules'
  `joined`, the spawn: world work) on `Commands`, applied at the sync
  point the chain puts before the mode's update. `respawn_dead` finds the
  players dead for the definition's delay (not while held or the match
  is over) and queues, in join order, the mode's say (`respawn_due`,
  whose `may_respawn` takes the world) and the respawns, applied before
  `hold_the_dead`. `end_protection` removes through `Commands`, applied
  at the sync point before `results::record` (exclusive), so before the
  tick's damage (O1). `hold_the_end` writes the intents and `FreezeTime`
  in place (seen by the rest of the tick as before), `EndFreeze` through
  `Commands` (only it and `minigame::restart` read it, later).
  Tests (`chimera/sim/tests/it/minigame.rs`):
  `a_shot_on_the_protections_last_tick_ends_it_before_that_ticks_damage`,
  `a_joiner_is_taken_in_and_spawned_in_its_first_tick`,
  `the_dead_respawn_on_the_tick_they_are_due`. `ecs.rs`: `chimera_sim`
  ratchet 13 to 9.

### Step 7: the HUD's status as a component (M)

Done 2026-10-30: `hud::MinigameStatus` (a component on each player) is
written by `ladder::write_status` (Update, before `hud::update`, on the
host and on each client from replicated rungs and scores) with
`set_if_neq`; `hud::update` reads the viewer's. `RulesModule::status` is
gone. Test: `each_players_status_lines_follow_the_ladder_in_a_component`;
`net_gun_game.rs` compares the clients' panels with the server's component.

- `RulesModule::status` (`fn(&mut World, Entity) -> Vec<HudLine>`,
  called per viewer per frame by `minigame/hud.rs:224`) becomes a
  `MinigameStatus` component on each player (or a resource keyed by
  viewer) that the rules module's own system writes when the standing
  changes. The HUD reads it with a query. `notice` and `callout` stay
  (pure).
- Flag: the lines are computed on the host and must reach clients.
  Today each client computes them from replicated state; keep that
  (each client's rules system writes its own component) rather than
  replicating text.
- Guards: `apps/party_kit/tests/it/hud.rs`, `results_hud.rs`,
  `kill_feed.rs`, `announcer.rs`; frozen-scene screenshots of the
  minigame panel (docs/performance.md's third pass method).
- Effect: the HUD-layer seam (DESIGN.md "One HUD layer over several
  games' HUDs") reads components only. A new game's HUD shows a
  minigame's standing without whole-world calls each frame.

### Step 8: rules modules' joins and armings as events (S)

- `RulesModule::joined` and `arming` become entity events (`Joined`,
  `Arming`) triggered where the runtime calls them now
  (`runtime::join`, `runtime::respawn`). The ladder observes them.
  Other parts (a game's look, a mode's kit) can observe them too,
  instead of wrapping the hook.
- `CaMode::left` (`games/combat_arms/src/game/rules.rs:317`): the mode's
  part of the existing `on_leave` observer (rules.rs:630).
- Flag O2 and O3: arming must come before the weapons are given;
  trigger with `world.trigger` at the same point. The ladder's observer
  is the only one that may give weapons.
- Guards: `rung_extras.rs`, `rung_characters.rs` (party_kit and
  chimera_sim), `gun_game.rs`, Combat Arms' `rung_character.rs`.

### Step 9: round and match lifecycle events (S-M)

- `Flow::event` (`FlowEvent`) is also triggered as an event (`ModeStep`)
  after the game's own handler runs. The mode's own `Hooks` stay.
  Pollers move to observers: `minigame::results::record` (match end),
  `breakables::round_restart` (`chimera/world/src/map/breakables.rs:
  982`), Combat Arms' `music::late_join`.
- Flag O2: these pollers run in later systems of the same tick today.
  An observer runs inside the step. Check that each reads nothing the
  rest of the step still writes; if it does, keep a message read by
  the same system instead.
- Guards: `games/combat_arms/tests/it/missions.rs`, `music_ambience.rs`;
  `apps/party_kit/tests/it/rotation.rs`, `results_hud.rs`,
  `party_music.rs`; `games/cs_source/tests/it/net_breakables.rs`.
- **Done 2026-10-30.** `mode::ModeStep(FlowEvent)`, an `Event` the flow
  triggers (`world.trigger`, then `world.flush()` so what observers queue
  applies there) right after the game's own `Flow::event` at every step
  (`flow::emit`), on the side that steps the match; one new step,
  `FlowEvent::MatchEnd` (the match has its result: at the end of the
  deciding round's end, after `after_round`, the intermission on, or the
  match over with `direct`), so the lifecycle is: the match's start
  (`Prepare { first: true }`, `ResetStats`), a round's start
  (`MissionStart`), its end (`RoundOver`, `RoundEnd`, `RoundEnded`), the
  match's end (`MatchEnd`), for CS:S's rounds, Combat Arms' modes and the
  minigames alike. `Hooks` and `Flow::event` stay. Reactions:
  `minigame::results::on_step` (observer: `MatchEnd` collects the results,
  counts the win and begins the match-end hold; `Prepare { first: true }`
  clears them; the per-tick `wins::publish` a normal system with a
  declared query in the chain where `record` was);
  `map::breakables::round_restart` an observer of a new
  `core::RoundRestart`, which `core::restart_round` triggers as it counts
  `RoundRestarts` up (CS:S's `MissionStart` handler; the tests that
  counted it up by hand call it now); Combat Arms' `music::trigger::
  late_join` an observer of `On<Add<Player>>` (where the server takes the
  join). **O2 checked:** each observer now runs inside the step (or the
  join) instead of in a later system of the same tick (frame for the
  music); the results read the match, scores, rungs and names, which
  nothing later in the tick changes (`respawn_dead`, the dead held,
  `end_protection` don't); the breakables clear gibs, particles and
  panes, which only `Update` and `FixedPostUpdate` systems add; the music
  goes out in the same frame's send. Map changes have no poller to
  convert (`MapHooks::unload`, class A). Tests (timing):
  `chimera_sim` `mode::an_observer_sees_each_step_of_a_match_once_in_
  order`, `minigame::the_results_come_in_the_tick_the_match_is_decided`;
  CS:S `rounds::rounds_steps_are_triggered_in_order_inside_the_step`,
  `net_breakables::a_new_round_makes_the_window_whole_in_its_tick`;
  Combat Arms `free_for_all::a_matchs_end_is_triggered_as_its_flows_
  steps`, `music_ambience::t12_a_joiner_gets_the_looping_trigger_at_its_
  join`. `ecs.rs`: `chimera/sim` 7 to 6, `chimera/world` 1 to 0,
  `games/combat_arms` 20 to 19; `Flow`'s allowed reason updated.
  **O4:** the poller was an exclusive system between the rules and
  movement; without it the single-threaded sort reordered the tick's
  unordered systems (CS:S's opposite pairs 56 to 66, Combat Arms' 149 to
  187, FixedUpdate orders dumped and diffed). An `ApplyDeferred` in its
  place (`map::registration`: a sync point after the rules, whose
  commands already applied before movement through the automatic ones)
  keeps the old order: FixedUpdate ratchets CS:S 279/279/56 to 264/264/54,
  Combat Arms 662/662/149 to 658/658/143.

### Step 10: per-app statics into resources (S each)

- **Done 2026-10-30.** `loading::LoadReporter` (an `Arc<Mutex<..>>`
  handle; `start_map_load` inserts a fresh resource per load and hands a
  clone to the loader as `MapLoadOptions::progress`, so `AppHooks::
  load_map`/`load_map_file` now take `&MapLoadOptions`, not an HDR level;
  `MapLoadOptions` is no longer `Copy`). `binds::BindDefaults` (defaults
  and old defaults keyed by game id, the active one the gameplay's
  `binds` game, read at start by the console's `autoexec`; the options
  menu keeps a copy of the list for "Use Defaults"; a gameplay switch
  still doesn't re-read: the "Known shortcuts" line in `binds.rs`).
  `console::MapLists` (names and cycle per game id, behind a lock so
  `&World` callers can fill it; `import` and a changed install forget
  it). Tests: `loading::tests`, `console::tests::
  two_apps_keep_their_own_load_progress`, `map_lists_read_each_games_
  install_once`. `chimera/world` ratchet 1 to 0.

- `map::loading::PROGRESS` (`chimera/world/src/map/loading.rs:16`) into
  a resource the loading task writes through a channel or `Arc` it
  owns. The listen server's and its client's loads stop sharing it.
- `binds::GAME_DEFAULTS`/`OLD_DEFAULTS` (`chimera/client/src/client/
  binds.rs:189`, `:193`): a resource of each registered game's defaults.
  The shell reads the gameplay's `binds` slot's entry (and could re-read
  it on a switch: another known shortcut gone).
- `console::MAP_NAMES`/`CYCLE` (`chimera/client/src/client/console.rs:
  1890`, `:1923`): a resource keyed by game.
- Guards: listen server, `chimera/client` binds tests, `apps/party_kit/
  tests/it/controls.rs`, a new test that two apps in one process keep
  their own load progress. Effect: client and server in one process
  stop sharing state. 5 D statics gone; lower the determinism ratchet
  where they were counted (`chimera/world` 1 to 0).

### Later, in this order

11. Kept states (or declared queries) for the per-tick raw queries left
    in section 2.4: `apply_protection`, `canonical_positions`,
    `capture_own_states`, the round and match net writers, the movers'
    `place`, Combat Arms' `rules/flow.rs` helpers, `supplies`,
    `mercs::expire`. Performance only. Do it only where `profsum` shows
    a gain (roadmap step 4).
12. Combat Arms' per-tick rules and objects (`publish`,
    `update_team_counts`, `objects::frame`/`gather`/`apply`,
    `perk_tick`, `ask_for_respawns`) as normal systems when a feature
    touches them (E3's "opportunistic" rule).
13. Install memos into resources or assets (9 D statics, E2's left part):
    only after the map loaders receive the app's context (`MapLoader`
    is a plain `fn` today, `chimera/world/src/map/sources.rs:163`).
14. Not planned: `NetHooks::replay` into a schedule (it runs inside
    `predict::reconcile`'s replay loop; little gain, high risk);
    `RungExtrasHook::apply` as an observer (it must precede `OnRung` and
    the weapons, see 2.1); Combat Arms' `bots::think` (bots are parked);
    the flow's driver (`step_phases`, `run_rounds`, `handle_deaths`),
    whose job is calling the mode's hooks in a fixed order.

## 5. Performance

No new profile was needed: the latest passes measured these paths.

- Kept query states cut new `QueryState`s from 1.23 to 0.40 ms a frame.
  The tick ran 24% (de_dust2) and 16% (cs_office) fewer instructions
  (docs/performance.md, "Party frame-rate pass", item 8). After ten map
  round trips the tick costs 25% more instead of 78% ("Map changes that
  slowed the tick"). The third pass kept the ladder's refill, the
  logic bridge's `sync_visuals`, Combat Arms' scoreboard and music
  states too. What was left of Chimera's own was ~0.12 ms (the
  minigame HUD, moments and ladder), part of which the third pass took.
- So no per-tick exclusive path is measurably hot today. The raw queries
  left in 2.4 are on rare branches or filter on rare components. The
  two to watch are `bomb::arm` (per replayed command, every character
  archetype) and the movers' `place` (per replay, and it clones
  `MapEntities`). Step 3 makes the first a declared query; item 11
  covers the second if a client profile shows it.
- The executor's cost is per system: ~600 systems a tick took
  0.6-0.8 ms of main-thread time a frame (third pass, "Executors"),
  about 1 µs a system. Steps 4-9 add about 25 systems a tick, most gated
  off, and remove about 15 exclusive ones. Expect a change of a few
  tens of µs a frame either way. Check each step with `perfgate`
  (instructions per tick) and `bot_cost`.
- Declared queries also allow parallel execution later. Today every
  simulation schedule runs single-threaded on purpose (determinism,
  E1), so don't count on it.

## 6. The rule and the wording

### `repo_checks`: `checks/tests/it/ecs.rs` (proposed)

Two checks, written like `determinism.rs` (text scans, comments and
`#[cfg(test)]` stripped, `tests/` and `ui` skipped), each a ratchet or
an allowed list:

1. `callback_registries_are_allowed`: every struct field, tuple struct
   or type alias whose type is a `fn(..)` pointer or `dyn Fn(..)`
   taking `World`, `DeferredWorld` or `EntityWorldMut` must be on
   `ALLOWED_REGISTRIES: &[(&str /* file */, &str /* type */, &str /* why */)]`,
   seeded with section 2.1's A and S types (S marked "until step 2"),
   and an entry that no longer exists fails too. The failure message:

   > `<file>`: `<Type>` stores whole-world callbacks (`fn(&mut World,
   > ..)`), which hide what they read and write and run outside the
   > schedule's order. Prefer, in order: a component that games observe
   > (`On<Insert<C>>`/`On<Discard<C>>`, as `ladder::OnRung`); an event
   > triggered at the same point (`world.trigger`, observers run at
   > once); a message read by a normal system; a system gated by a run
   > condition (`gameplay_is`, `hud_is`, `mode_is`). A decision the
   > runtime asks (`fn(&World, ..) -> answer`) is fine. If the job
   > really needs the whole world (map loading, spawning, network apply,
   > codecs, console, tools), add `("<file>", "<Type>", "<why>")` to
   > ALLOWED_REGISTRIES in checks/tests/it/ecs.rs. See DESIGN.md, "ECS
   > style".

2. `per_tick_exclusive_systems_ratchet`: per crate, the number of
   functions taking `&mut World` that an `add_systems` call registers in
   a schedule other than the startup ones and `OnEnter`/`OnExit`
   (section 1's matching). The stored counts are section 2.2's (CS:S
   12, Combat Arms 39, `chimera_net` 26, `chimera_sim` 13, ...). A
   crate not listed is 0. The counts only fall, like `determinism.rs`.
   The failure message:

   > `<crate>`: `<n>` exclusive systems run every tick or frame,
   > `<stored>` allowed (+`<k>`: `<names>`). Write it as a system with
   > declared parameters (`Query`, `Res`, `MessageReader`, `Commands`);
   > a reaction to a moment as an observer. If it is network apply or
   > capture, map loading, the console or a menu, put it in a file on
   > ALLOWED_EXCLUSIVE_FILES in checks/tests/it/ecs.rs with the reason.
   > When you remove one, lower the stored number.

   `ALLOWED_EXCLUSIVE_FILES` starts with the A rows' files of 2.2 whose
   job is the world (the net modules, `console.rs`, `game_log.rs`,
   the logic bridge, the demo, the lobbies and menus, the servers'
   `main.rs`). Systems in those files don't count.

Plus a scanner test (as `determinism_scanners_work`) and step 1's
extension of `determinism.rs` to every simulation crate.

### DESIGN.md (proposed text, under "Architecture", after "Components and categorization")

> ### ECS style
>
> Prefer, in order: components for facts about an entity, observers for
> reactions to them (`On<Insert<C>>`, `On<Discard<C>>`), messages for
> per-tick flows (hits, deaths, weapon events) read by normal systems,
> events triggered at a known moment (`world.trigger`), and systems
> with declared queries gated by run conditions (`gameplay_is`,
> `hud_is`, `mode_is`; never a new set) for per-tick logic. A fact
> other machines must see the same way is put on locally from
> replicated state, as `ladder::OnRung` is (see "Arriving on a rung").
> Data hooks stay for decisions the runtime asks (`fn(&World, ..) ->
> answer`) and for what is keyed by an id (weapon hooks, movements,
> codecs). Whole-world access (`&mut World`, exclusive systems,
> callback registries) only where the job is the world: map loading,
> spawning, network apply and capture, codecs, the console, menus,
> tools. `checks/tests/it/ecs.rs` lists those and ratchets the rest
> (docs/plans/active/ecs-cleanup.md).
