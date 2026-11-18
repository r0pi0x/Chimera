# Rounds, money and buying

Started 2026-10-06. Counter-Strike's match flow as a rules mode beside
deathmatch, so CS:S maps play like the game and minigame maps get round
restarts.

## Done (slice 1)

- `rules::rounds` (`chimera_rounds 1`): freeze (`mp_freezetime`, nobody
  moves or shoots), a timed round (`mp_roundtime`), elimination wins,
  defenders win on time on bomb maps (on maps without objectives the clock
  ends nothing, as CS:S's, measured; `chimera_untimed_win 1`: the defenders
  win), a pause (5 s), then everyone back at their
  spawns: the dead with starting weapons, survivors with theirs; loose
  weapons cleared. `RoundEnded` messages, `RoundState` (phase, number,
  wins).
- Money (`weapon::economy`): `Money` per character (`mp_startmoney`,
  capped at 16000), kill reward 300, team-kill penalty 3300, elimination
  bonus 3250 on bomb maps and 3000 elsewhere, loss bonus 1400 + 500 per
  consecutive loss up to 3400 (a draw pays nobody: measured 2026-10-17).
- Buying: `buy` charges the price (spec weapons.md's `WeaponPrice`;
  kevlar 650, kevlar+helmet 1000, helmet alone 350), refuses without the
  money or outside the freeze and the buy period (`mp_buytime`), and drops
  the weapon it replaces in the same slot.
- HUD: `HudAccount` (money) and `HudRoundTimer` panels, the round result
  across the screen.
- Tests: games/cs_source/tests/it/rounds.rs, `weapon::economy` unit tests.

## Next

1. Measure on the probe server: the defaults (CS:S's own mp_roundtime and
   mp_freezetime defaults, the end delay), whether the buy period counts
   from the freeze's end, the clock during the freeze,
   rewards per weapon (CS:S pays 300 for every gun and the knife?).
2. Buy zones: `func_buyzone` by team is in (`economy::in_buy_zone`; maps
   without zones let you buy anywhere, CS:S would make zones around the
   spawns), the buy menu doesn't yet say why a buy failed. Ammo buying
   is in (`primammo`/`secammo` on the menu's 6 and 7, `buyammo1`/
   `buyammo2`: boxes per ammo type up to the reserve maximum; bought
   guns come with their clip only, the spawn pistols with two more
   clips; bots buy their ammo before armour; prices unmeasured), bot
   buying preferences (ours: `economy::autobuy`, a primary for the team by `Prices::bot_weights` among the dearer half it can afford, then armour), the game's own VGUI buy-menu look
   (ours: `games/cs_source/ui/buy_menu.rs`, B or `buymenu`, CS:S's category keys
   and order, each team its own guns; rounds start from the team pistol
   and knife).
3. Round restarts re-creating map entities: done. `start_round` counts
   `core::RoundRestarts` up; the logic bridge re-creates its world from
   the map's entities (`LogicWorld::round_restart`: doors, buttons,
   movers and trains at their start, triggers re-armed, timers,
   counters and relays as spawned, the event queue dropped, logic_auto
   firing again 0.2 s later) and shows the nodes of broken or killed
   entities again (they are hidden, no longer despawned); the map layer
   clears gibs and particles and makes broken windows whole; game_text
   messages are cleared. Kept as they are: `world::ROUND_KEEP`, the
   preserve list from the public multiplayer code the spec quotes
   (func_brush, func_wall, func_buyzone, info_target, soundscapes,
   ropes, sky_camera). Tests: logic unit tests (`restart_tests.rs`),
   games/cs_source/tests/it/map_logic.rs, de_nuke vents and doors, cs_office windows.
   Measured 2026-10-15 (entity_io.md question 1): CS:S's keep list (now
   `ROUND_KEEP`; a func_brush is kept, so stays as it was) and
   logic_auto's outputs each round (OnNewGame, OnMapSpawn, OnMultiNewMap,
   as at map load; never OnMultiNewRound). Open: whether decals,
   prop_physics and env_global states reset; whether the restart runs
   at the freeze's start (ours) or the round's end. Also done: the
   announcer (`map::RoundSounds`: `Event.TERWin`/`Event.CTWin`/
   `Event.RoundDraw`, a radio line when the round goes live; whether the
   game plays it at freeze end for both teams is to check) and team
   scores on the scoreboard.
4. Objectives (specs/cs_source/objectives.md): done, first slice.
   `objectives` module: the C4 to a random terrorist each round, arming
   in a `func_bomb_target` (3 s, clicks, view-model press and third-person
   gesture), the planted bomb (`mp_c4timer`, beeps on the provisional
   schedule, defuse with +use 10 s / 5 s with a $200 kit, interrupts
   reset), the explosion through the HE `ExplosionRule` with the map's
   `bombradius` (H1 falloff over 3.5 × radius) and the target's
   `BombExplode` outputs through the logic layer; the round-end table and
   its money (measured 2026-10-17, objectives.md Q5: exploded 3500,
   defused and target saved 3250, +800 to planting losers, no planter or
   defuser reward, hostage touch 150, rescue 1000, 20 a point for hurting
   one, the killing hit twice that and 500; hostage maps 3000/2500/3250
   plus 150 a living, 100 a led and 850 a rescued hostage; the round's
   bonuses paid as the next round starts, nothing to losers alive at a
   time-out);
   hostages as team-less characters led by +use over the nav mesh and
   rescued in `func_hostage_rescue`; Ts win on time on hostage maps.
   HUD icons, progress bar, messages, radar bomb marker. Tests:
   games/cs_source/tests/ui/objectives.rs (O1, O5-O7, O10-O24 shape, O32-O41),
   games/cs_source/tests/it/heavy/map_objectives.rs (dust2 A, cs_office). Left: see backlog §3.
5. Done (2026-10-13, `rules::game_end`): `mp_timelimit`, `mp_maxrounds`,
   `mp_winlimit`, `mp_chattime`, the intermission and the next map of the
   install's map cycle; `timeleft`. Team balance
   (`mp_autoteambalance`, `rules::balance_teams`), `mp_tkpunish` and the
   round's win panel (`RoundSummary`) followed in parity round 2. Left:
   the captures C9, C11 and C12 in docs/plans/active/parity-gaps.md
   (CS:S's defaults, its intermission, who balancing moves, the panel's
   choices).

## Decision log

- 2026-10-07: objectives are their own module between weapon and rules
  (rules end rounds from its state, bots and games use it), not part of
  rules: bots and the game's data need them. Hostages are characters
  with no team moved by the game's movement (collision, stairs, ragdolls
  for free); everything that counts players skips `Hostage`.

- 2026-10-06: rounds are a rules mode (off by default) rather than
  replacing deathmatch: minigame maps and quick playtests keep instant
  respawns. Money lives in the weapon layer next to buying, which the
  rules layer drives through `BuyWindow`.
- 2026-10-06: round restarts reach the logic and map layers through a
  core counter (`RoundRestarts`) rather than a message, so a restart is
  never missed between fixed ticks and layers that load later see the
  current count; rules run in `SimSet::Rules`, before the logic.
  Re-creating the whole logic world (instead of resetting each class)
  matches CS:S, which re-creates map entities from the entity lump.
