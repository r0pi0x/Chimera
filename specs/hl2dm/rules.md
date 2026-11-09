# Half-Life 2: Deathmatch: rules (DM and TDM, teams, spawns, respawn, limits, scoring) (H6)

Source basis: the public Source SDK 2013 multiplayer game code (the
HL2:DM game rules and player, the shared multiplayer and team-play rules,
the base player's death, spawn and observer handling, chat and the server
event log, the HL2:DM client's dead-player view), read in place in a
separate spec session; the game's own files on this box (game build
24600434, dedicated server build 24600426): the configs inside
`hl2mp/hl2mp_pak.vpk` (`cfg/valve.rc`, `cfg/settings.scr`), the event
definitions (`resource/modevents.res` in that VPK, Half-Life 2's loose
`resource/gameevents.res` and `serverevents.res`), `cfg/mapcycle_default.txt`,
the English localization file, and cvar names present in the shipped
server library, read for names and values only. No game was launched and
no server was started.
Status: draft

## Summary

HL2:DM is a free-for-all deathmatch, or team deathmatch when `mp_teamplay`
is 1 at map load, with no rounds: players spawn at once on joining, respawn
on a button press (or by force after 5 s with `mp_forcerespawn`), and the
map ends on a frag or time limit with a 10-second frozen scoreboard before
the next map of the cycle. There is no freeze cam and no observer phase
after death: the dead look at their own ragdoll from 96 units behind until
they respawn. Teams are Combine and Rebels, chosen by player model; a
model of the wrong kind is swapped round-robin. Several things players
notice come from simple rules applied literally: a team kill still adds a
point to the killer's team, a suicide during a team change is charged to
the new team, `say_team` in plain DM reaches nobody, and the 5-second team
change limit the code prepares is never enforced.

This spec continues [source_deltas.md](source_deltas.md) (H0). Where H0 is
wrong it says so (section "Corrections to H0").

## Units and conventions

- Source units, Z up; degrees; seconds. Server tick 0.015 s (66.67 Hz).
- "Tick N after X" counts ticks of server time; comparisons are stated as
  strict (>) or not (≥) because at 0.015 s several thresholds land exactly
  on a tick (3.0 s = 200 ticks, 0.3 s = 20 ticks). The original compares
  32-bit floats, so an exact tie may fall either way there (Open
  questions 6); an implementation uses the integer tick counts given.
- Teams by number: **0 Unassigned, 1 Spectator, 2 Combine, 3 Rebels**
  (these are the names the scoreboard, logs and `player_team` use).
- Observer modes in this build's numbering: 0 none, 1 death cam, 2 freeze
  cam, 3 fixed, 4 in-eye, 5 chase, **6 point of interest, 7 roaming**. This
  differs from specs/cs_source/spectating.md, where 6 is roaming (that
  spec read an older CS:S); see Open questions 7.
- "Player" includes bots and fake clients unless said otherwise.

## Constants

| Name (ours) | Value | Unit | Meaning |
|---|---|---|---|
| `mp_teamplay` | 0 | | 1 = team deathmatch; read once when the map's rules are created |
| `mp_fraglimit` | 0 | frags | 0 = off |
| `mp_timelimit` | 0 | min | 0 = off; read as a whole number (Quirks) |
| `mp_chattime` | 10 | s | intermission length, clamped 1–120, read as a whole number |
| `mp_forcerespawn` | 1 registered, **0 set by `cfg/valve.rc`** | | force a respawn 5 s after death |
| `mp_friendlyfire` | 0 | | TDM: teammates can't hurt each other |
| `mp_allowspectators` | 1 | | `spectate` / `jointeam 1` allowed |
| `mp_forcecamera` | 1 | | spectating restricted to one's own team (Spectator-team members exempt) |
| `mp_fadetoblack` | 0 | | unused by HL2:DM's death view |
| `mp_restartgame` | 0 | s | >0: restart after that many seconds (max 60) |
| `mp_readyrestart` | 0 | | 1: start a ready-up |
| `mp_ready_signal` | `ready` | | the chat line each player must say |
| `hl2mp_spawn_frag_fallback_radius` | 48 | units | radius of the spawn telefrag when no spot is free |
| `mapcyclefile` | `mapcycle.txt` | | falls back to `cfg/mapcycle_default.txt` |
| `nextlevel` | "" | | overrides the next map once; cleared at every map load |
| `mp_mapcycle_empty_timeout_seconds` | 0 | s | >0: change map after this long with no human players |
| `sv_report_client_settings` | 0 | | 1: log each player's `cl_cmdrate` when their settings change |
| `RESPAWN_MIN_DEAD` | 3.0 | s | a respawn needs more than this since death (strict) |
| `FORCE_RESPAWN_AFTER` | 5.0 | s | forced respawn after more than this (strict) |
| `MODEL_CHANGE_INTERVAL` | 5.0 | s | minimum between model changes |
| `TEAM_CMD_RATE` | 0.3 | s | `jointeam` and `spectate` are each ignored if repeated sooner |
| `SPAWN_CHECK_RADIUS` | 128 | units | a spot with another player within this is occupied |
| `SPAWN_RAISE` | 1 | unit | the player is placed this far above the spot |
| `SPAWN_BLAST_GRACE` | 0.5 | s | after a spawn, pure blast damage is refused |
| `TELEFRAG_DAMAGE` | 300 | | generic damage from the world in the fallback |
| `SUICIDE_INTERVAL` | 5.0 | s | a suicide is refused within 5 s of the last one |
| `DEATH_CAM_DISTANCE` | 96 | units | dead view: behind the ragdoll |
| `DEATH_CAM_WALL` | 6 | units | half-size of the box traced for the dead view |
| `DEAD_VIEW_HEIGHT` | 14 | units | added to the ragdoll origin's height |
| `RULES_PERIOD` | 1.0 | s | restart and ready checks run once a second |
| `READY_RESTART_DELAY` | 5 | s | all ready to restart |
| Player limits | min 2, default 16, max 33 (32-bit server; 101 slots in the 64-bit build) | | |

## Corrections to H0

1. **No observer after 3 s dead** (H0 1.12). The base player tries to turn
   a dead player into an observer more than 3 s after death, but HL2:DM
   refuses that switch unless the player asked to spectate. A dead player
   stays in the ragdoll view until respawn. `cl_spec_mode` is read only
   when joining the Spectator team, and accepts modes 4–7 (in-eye, chase,
   point of interest, roaming), not "chase or roaming only".
2. **Team changes are not limited to one per 5 s** (H0 Part 2, "Player
   models"). The 5 s team-change timer is set but never checked. Only the
   0.3 s command rate applies to `jointeam`; a team change made through a
   model change obeys the model limit. Both timers are also reset by every
   spawn.
3. **Open question 11 is answered**: since the dead never become
   observers, `mp_forcerespawn` always applies to a dead player. Players
   on the Spectator team never respawn through it.
4. **The invalid-model fallback** (H0 Part 2): in DM an invalid
   `cl_playermodel` falls back to the player's current model if that is a
   valid one, and to `models/combine_soldier.mdl` only when it isn't. The
   client's cvar is rewritten to match.

The rest of H0's rules facts (loadout, team spawn classes, halls3 outside
the cycle, cvar defaults) hold.

## Behavior

### Game mode: DM or TDM

- The rules read `mp_teamplay` once, when they are created at map load.
  Changing it later has no effect until the next map. `mp_restartgame`
  does not apply it either.
- The server's game description is "Deathmatch" or "Team Deathmatch".
  Whether team play is on is networked to clients (scoreboard, HUD).
- DM: every playing player is on team 0 (Unassigned), and any request for
  team 2 or 3 is turned into team 0. Nobody is anyone's teammate:
  friendly fire, `say_team` and voice by team all treat everyone as an
  enemy, except voice (below).
- TDM: playing players are on 2 or 3. Two players are teammates when
  their team names are equal.

### Joining

On entering the game the player spawns at once (no team menu):

1. DM, first spawn only (the player has no model yet): if `cl_playermodel`
   is not one of the 19 valid models (H0's list, compared ignoring case),
   the server tells the client to set it to `models/combine_soldier.mdl`.
   The player stays on team 0 and its model is set as in "Models in DM".
2. TDM, while the player's team is 0: it joins the team with **fewer
   players**; if both have the same count, a random one of 2 and 3 (even
   chance). Counts include bots and dead players. There is no other
   balancing: HL2:DM has **no automatic team balance**.
   `mp_autoteambalance` and `mp_teams_unbalance_limit` exist in the
   shipped server library but HL2:DM's rules never read them. The player
   is told by a chat line which team they are on.
3. Everybody gets a "<name> connected" notice; the joining player is shown
   the message of the day panel.

### Models

The 19 valid models are H0's list. HL2:DM's two round-robin lists, in
their order:

- Combine (4): `models/combine_soldier.mdl`,
  `models/combine_soldier_prisonguard.mdl`,
  `models/combine_super_soldier.mdl`, `models/police.mdl`.
- Citizens (15): group03 `male_01`, `male_02`, `female_01`, `male_03`,
  `female_02`, `male_04`, `female_03`, `male_05`, `female_04`, `male_06`,
  `female_06`, `male_07`, `female_07`, `male_08`, `male_09`.

There is one counter per list, shared by all players, starting at 0 when
the server process starts and **never reset**, not even at a map change.
To take a model the counter is first advanced (modulo the list length) and
the model at the new index is used. The first replacement after a server
start is therefore the list's **second** entry.

Model kind: a name containing `models/human` (ignoring case) is a citizen;
otherwise one containing `police` is metro police; otherwise one containing
`combine` is a Combine soldier. The kind chooses footstep, pain and death
sounds (H9) and the melee weapon of the next spawn: stunstick for metro
police and Combine, crowbar for citizens (H0 1.3). The model's team kind
(citizen or not) also chooses the TDM team.

**Models in TDM** (when the player's team is set): take `cl_playermodel`;
if it is not valid, use `models/Combine_Soldier.mdl` and tell the client to
set it. Then on team 2, a citizen model is replaced by the next Combine
model; on team 3, a non-citizen model is replaced by the next citizen
model. A model that already fits the team is kept as chosen (a Combine
player may be metro police). On team 1 the model is left as resolved.

**Models in DM**: take `cl_playermodel`; if it is not valid, use the
player's current model if that is valid, else `models/Combine_Soldier.mdl`,
and tell the client to set its cvar to that.

Every model assignment starts the 5 s model timer.

**Changing `cl_playermodel` during play** (the server sees the client's
settings change). Nothing happens if the new value equals the current
model (ignoring case). Otherwise:

- if the model timer has not run out (the next change time is at or after
  now), the server sets the client's `cl_playermodel` back to the current
  model and prints in chat that the player must wait N more seconds,
  N = whole seconds left, rounded down;
- DM: the model changes at once, even while alive, and the player is told
  their new model in chat. Footsteps and death sound follow the new kind
  at once; the melee weapon changes only at the next spawn;
- TDM: a citizen model moves the player to team 3, anything else to team 2
  (see "Changing team"). If that is the current team, the model is updated
  within the team without a death.

The timer is reset to "free" by every spawn, so a model (and in TDM a
team) can always be changed right after respawning.

### Changing team

Commands: `jointeam <n>` and `spectate`. Each is ignored if the same
command was accepted less than 0.3 s ago; the first use is always
accepted. `jointeam` without an argument counts as 0. `joingame` is
accepted and does nothing.

`jointeam 0`, or a number above 3: refused (a server-console warning, no
change).

`spectate` and `jointeam 1`:

1. if `mp_allowspectators` is 0 (and the player is not SourceTV): refused,
   with the centre message "This server does not allow spectating";
2. if the player is on a team other than 0 and alive: it is killed as a
   suicide that ignores the 5 s suicide limit, then **given 1 frag back**
   (net frags 0, deaths +1, its old team −1 team point, a `player_death`
   with itself as attacker). A DM player is on team 0, so it goes to
   spectate **without dying** and without any score change;
3. it joins team 1, loses all weapons and the suit, and becomes an
   observer in mode `cl_spec_mode` if that is 4–7, else its last observer
   mode (roaming by default).

`jointeam 2` or `3`:

1. any observer mode is ended and the player's state set to active (it
   stays dead if it was dead or a spectator);
2. in DM the team is turned into 0;
3. if the team differs from the current one: `player_team` is fired, the
   player moves to it, and in TDM, if the old team was not 0, the player
   is killed as a suicide **after** the move, so the −1 team point goes to
   the **new** team. The usual 5 s suicide limit applies: a player who
   suicided less than 5 s earlier changes team alive;
4. the model is re-resolved (TDM or DM rules above), starting the model
   timer. This happens even when the team did not change.

A spectator who rejoins play is still dead: it respawns by the rules of
"Respawn", counting from its last real death (normally long past), so the
first button press or the force respawn brings it in at once.

### Spawn-point choice

Spot class: DM uses `info_player_deathmatch`. TDM uses `info_player_combine`
for team 2 and `info_player_rebel` for team 3, unless the map has no
entity of that class, in which case `info_player_deathmatch`. Only
dm_runoff has team spots (8 Combine, 6 Rebel); every other stock map plays
TDM on its deathmatch spots.

"Last spot": one remembered spot for the deathmatch class (shared by
everyone), and one each for the Combine and Rebel classes. They are
cleared at map load.

Choice, with the spots of the class in map entity order seen as a ring
with **one empty slot** between the last and the first:

1. start at the last spot of this class (or at the empty slot if none);
2. move forward N slots, N uniform in 1–5; if that lands on the empty slot,
   take the first spot. This is the **first candidate**. (The first spot
   is twice as likely as others when N reaches the empty slot.)
3. from the first candidate, walk forward through the ring (the empty slot
   is skipped) and take the first spot that is **free**, skipping any
   spot whose origin is exactly (0, 0, 0). Stop after a full turn.
   - Free: no player entity other than the spawning player within 128
     units of the spot's origin (any part of its bounds inside that
     sphere). Whether dead players' bodies and observers count is Open
     questions 3; the rule tests player entities without regard to life
     state.
4. if no spot is free: every player except the spawning one within
   `hl2mp_spawn_frag_fallback_radius` (48) of the **first candidate** takes
   300 generic damage from the world (attacker and inflictor the world), and
   the player spawns there. 300 kills through any armour (H2: at most 100
   armour absorbs at most 240). The victims die as world kills (Scoring).
5. if the class has no spots at all: the first `info_player_start` (six of
   the eight stock maps have one; dm_underpass and halls3 don't). With
   none, the behaviour is undefined; the stock maps never reach it.
6. the chosen spot becomes the class's last spot (and the deathmatch
   one too: the shared last spot is always updated, even in TDM).

The spawn grace starts with the choice: for 0.5 s (now < spawn + 0.5)
the player refuses damage whose type is **exactly** blast (blast with no
other flag), whoever deals it. It is meant against S.L.A.M.s on spawn
points. Which projectiles deal pure blast is H4's. There is no other spawn
protection.

Map and prop explosions against the grace (answered 2026-11-09). The test
is equality of the whole damage-kind word with blast (64), so any second
bit lets the hit through:

| Source of the blast | Damage kind it carries | Refused in the 0.5 s grace? |
|---|---|---|
| map `env_explosion`, default flags | blast (64) alone | **yes** |
| map `env_explosion` with "surface only" (spawnflag 8192) | blast + blast-surface (64 + 134217728) | no |
| map `env_explosion` with "generic damage" (16384) | generic (0), no blast bit | no |
| exploding prop (`prop_physics` / `prop_dynamic` with explode damage or radius; both of its paths ask for "surface only") | blast + blast-surface | no |
| the 1-damage fallback blast of a prop with the onbreak-explode interaction (source/prop_damage.md 7.5 step 7) | blast alone | yes |
| `func_breakable` explosion on break | blast alone | yes |
| HL2:DM's grenades, rockets, S.L.A.M.s (H4) | blast alone | yes |

So a barrel blown up on a spawn point kills a player who has just spawned
there; an `env_explosion` fired by the map at the same moment does not.
The surface-only bit also gives the prop blast suit.md's under-water
refusal (a player whose eyes are under water is not hurt by it when the
prop is out of the water).

### The spawn itself

On every spawn (first join, respawn, restart):

- team: as "Joining" if the team is 0 in TDM; the model and team timers
  are reset to free first;
- position: the spot's origin + (0, 0, 1); velocity 0; body and view
  angles = the spot's angles (all three); view punch cleared; on-ground
  flag set; not ducked; not under water;
- health 100 (max 100), armour 0, suit on, aux power 100, flashlight
  battery 100 (H2); FOV reset to the default;
- solid, visible, not an observer (team 1: an observer again, with no
  weapons);
- loadout (not on team 1): H0 1.3 and H3 (weapons.md): the suit, the
  model kind's melee weapon, pistol, SMG1, frag, gravity gun and their
  ammunition; the weapon in hand per `cl_defaultweapon`;
- frozen if the game is in intermission (see Intermission), else unfrozen;
- the player's ready flag is cleared (Ready-up);
- `player_spawn` fires (also for team 1);
- `game_player_equip` entities would be touched here; no stock map has
  one.

### Death

When a player dies (killed, or a suicide):

1. scoring and the death notice (Scoring, Game events), unless the game is
   in intermission (then no `player_death`, frags and deaths unchanged,
   but the team-point rule still runs);
2. a ragdoll record is created at the player's position with its velocity
   and the accumulated bullet force (H7); killing damage that dissolves
   (the AR2's combine ball) dissolves the ragdoll and silences the death
   sound;
3. the dead player's live S.L.A.M. satchels go off 0.2 s later (H4);
   its flashlight goes off and its zoom ends;
4. the player is dead at once (no dying phase), is not solid, holsters
   its weapon, and its view drops to the dead view height 14. The death
   time is recorded. What it drops is H3's.

### The dead view

As long as the local player is dead and not an observer, the client
places the camera:

```math
O = R + (0, 0, 14), \qquad E = O - 96\,\hat f
```

where R is the ragdoll's origin (the player's own eye position if no
ragdoll is known), f̂ the unit forward vector of the player's view angles,
which the mouse still turns (so the camera orbits the body). A box of
half-size 6 is traced from O to E against world brushes only; if it hits,
the camera is at the hit position. No freeze cam, no fade to black, no
death cam mode and no target: in observer terms the player is in mode 0
throughout.

### Respawn

The dead player's state is checked on every one of its user commands
(each tick at the default rates), after its buttons are read:

1. **Release first.** "Buttons down" means any button except the score
   key; a toggled duck doesn't count. The first check with no button down
   marks the player respawnable. Until then nothing else happens: a player
   who holds a button through death must let go once.
2. When respawnable: a respawn is attempted if a button is down, or if
   `mp_forcerespawn` > 0 and more than 5.0 s have passed since death.
   The attempt clears the buttons and spawns the player **only if more
   than 3.0 s have passed since death**; otherwise nothing happens, and
   a later press (or a still-held button, read again on the next command)
   tries again.
3. Players on team 1 never respawn this way.
4. In intermission no dead player is checked, so nobody respawns.

Tick counts at 0.015 s, death on tick 0: a held or pressed button
respawns on tick **201** at the earliest (3.015 s); the force respawn
fires on tick **334** (5.01 s).

`mp_forcerespawn`: registered default 1, but both installs' `cfg/valve.rc`
(identical in game and server) set it to 0, the listen-server dialog
(`cfg/settings.scr`) defaults it to 0, and the dedicated server's engine
library contains the command that runs `valve.rc`. The shipped behaviour is
therefore most likely 0: respawn only on a button (Open questions 1).

### Spectating (Spectator team only)

As specs/cs_source/spectating.md for target choice and modes, except:

- only Spectator-team members are observers (deaths never make one);
- `mp_forcecamera` 1 doesn't restrict Spectator-team members;
- a dead player stops being a valid target as soon as it is respawnable,
  which in HL2:DM is the first command after death without a button held
  (CS:S keeps the dying target for 3 s). The watcher moves on at that
  point. Open questions 8.

### Frag and time limits

Checked once per tick in the rules' update (Per-tick order), time first:

- **Time limit**: if `mp_timelimit` (as a whole number) > 0 and
  game start + 60 × `mp_timelimit` − now < 0, the game goes to
  intermission. The game start is server time 0 at map load (server time
  restarts at each map) and is set to now by a restart.
- **Frag limit** if `mp_fraglimit` ≠ 0 (compared as a real number):
  - DM: any connected player (spectators included) with frags ≥ limit;
  - TDM: Combine's or Rebels' **team score** ≥ limit. Team scores are not
    the sum of players' frags (Scoring).
- `timeleft` (console command): replies with the time left as minutes and
  two-digit seconds (whole seconds, rounded down), or that there is no
  time limit when the whole seconds left are 0, which includes the last
  second of a running limit.
- Changing `mp_timelimit` mid-game takes effect at once against the same
  game start (a negative value is set back to 0).

### Intermission

On a limit:

1. the game is over; the intermission ends at now + `mp_chattime` (whole
   seconds; 1–120, default 10);
2. every connected player is shown the scoreboard and frozen: no movement,
   no buttons, no impulses, and the view angles stay where they were. Players
   who spawn now are frozen too;
3. the dead stay dead; deaths during intermission (grenades still in
   flight, trigger_hurt) give no frags, no deaths and no `player_death`,
   but do change team scores (Quirks);
4. chat works (that is what `mp_chattime` is for);
5. the restart and ready checks no longer run;
6. on the first tick with intermission end < now (tick 667 after the
   start for 10 s), the map changes, once.

### Map change

The next map is `nextlevel` if it is set (it is cleared at each map load),
else the map cycle's next entry:

- File: `cfg/<mapcyclefile>`, else `<mapcyclefile>` at the game folder's
  root, else, if `mapcyclefile` is the default `mapcycle.txt`,
  `cfg/mapcycle_default.txt`. Neither install has a `mapcycle.txt`, so the
  default's seven dm_ maps apply: dm_lockdown, dm_overwatch, dm_powerhouse,
  dm_resistance, dm_runoff, dm_steamlab, dm_underpass. Lines are cut at
  newlines, spaces and carriage returns are removed, and empty lines and
  lines starting with `//` are skipped. If the file changes on disk, it is
  reloaded at the next lookup.
- Position: at each map load, if the current map is in the cycle, the next
  map is the entry after it (wrapping; for a map listed twice, the
  position after the one just played is kept); if the current map is not
  in the cycle (halls3, or any map started by hand), the next map is the
  **first** entry. No file and no entries: the current map again.
- Admin commands `changelevel_next` (go now) and `skip_next_map`.
  There is no `nextmap` command in the game code.
- `mp_mapcycle_empty_timeout_seconds` > 0: the map changes after that
  long with no human player connected (bots and SourceTV don't count).

A map change makes new player entities: frags, deaths and team scores
start at 0.

### Restart

`mp_restartgame N` (N > 0, whole number): at the next once-a-second check,
N is clamped to 60, every player gets a centre and console message that
the game restarts in N seconds, the restart is set for now + N, and
`mp_restartgame` goes back to 0. On the first tick with now ≥ that time:

1. the game start becomes now (the time limit starts over);
2. the map's entities are rebuilt from the map: everything except the
   world, players, team and rules records, spawn points, ropes, decals,
   brushes that never change (`func_brush`, `func_wall`,
   `func_illusionary`), sun, wind, fog, sky camera, shadow control,
   soundscapes, `info_target`, ladders' `info_ladder`, AI nodes and view
   models is removed and spawned again; weapons are removed unless a
   player holds them; ragdolls and projectiles are removed; delayed entity
   I/O is cancelled. So pickups and breakables are reset;
3. every player: active weapon holstered, all weapons and the suit
   removed, then respawned (a player dead for 3 s or less is **not**
   respawned and stays dead with no weapons, respawning normally later),
   then frags and deaths set to 0;
4. Combine and Rebels team scores set to 0 (Unassigned's and Spectator's
   are not);
5. any intermission end time is cleared;
6. `round_start` fires with `fraglimit` 0, `timelimit` 0 and `objective`
   `DEATHMATCH`.

A restart requested during intermission is not seen (the checks are off);
since `mp_restartgame` keeps its value, it then fires on the next map
about a second after load. `mp_restartgame_immediate`, `mp_clan_readyrestart`
and the waiting-for-players cvars exist but HL2:DM never reads them.

### Ready-up

`mp_readyrestart 1`: at the next once-a-second check the server starts
waiting for ready players, sets `mp_readyrestart` back to 0, cancels any
pending `mp_restartgame` countdown and tries to fire a `hl2mp_ready_restart`
event, which no events file declares, so nothing is sent. No message is
shown.

While waiting, a player whose whole chat line (`say` or `say_team`, after
the server's chat clean-up) equals `mp_ready_signal` exactly (case
sensitive) is marked ready. Every second the server checks whether every
connected player (bots and spectators included) is ready; when all are,
every player gets a centre and console message that all are ready and the
game restarts in 5 seconds, the wait ends and the restart (above) follows
5 s later. A player's ready flag is cleared by every spawn, so a player
who dies and respawns after saying the signal must say it again. With no
players connected, everyone is trivially ready.

### Scoring

Definitions: the **scorer** of a death is the attacker if it is a player,
else the player an attacker or inflictor names as its owner (for example a
thrown or launched projectile), else none.

Per death (outside intermission), on the victim's player record:

| Case | Victim frags | Victim deaths | Scorer frags |
|---|---|---|---|
| Killed by another player (DM, or TDM enemy) | 0 | +1 | +1 |
| Team kill (TDM, needs `mp_friendlyfire` 1 or damage that ignores it) | 0 | +1 | **−1** |
| Suicide (scorer is the victim: own grenade, `kill`, `explode`, team change) | −1 | +1 | – |
| World kill (no scorer: falling, drowning, `trigger_hurt`, crushing doors, the spawn telefrag, an uncredited prop) | −1 | +1 | – |

Team points, separately, for every death, also during intermission: the
team of the **attacker entity** gets +1, or −1 when the attacker is the
victim itself. Consequences:

- TDM enemy kill: killer's team +1;
- TDM team kill: killer's team **+1** (while the killer loses a frag);
- suicide: the victim's current team −1 (after a team change, the new
  team; when moving to spectate, the old team);
- world kill: the attacker entity's team, normally 0 (Unassigned): no
  visible team changes;
- a kill by a projectile whose attacker is its player: as that player.

DM never uses team scores. TDM's frag limit and its scoreboard use them.

### Friendly fire (TDM)

With `mp_friendlyfire` 0 a player refuses all damage whose attacker is a
teammate other than itself, unless the damage is flagged to force friendly
fire. Covered: bullets (and they draw no blood or impact on the teammate),
melee, explosions of a teammate's grenade, rocket or S.L.A.M., physics
damage credited to a teammate (H5). Not covered: **self damage** (own
rockets and grenades hurt), damage with no player attacker. The check is
made at hit time with the current teams. In DM nobody is a teammate, so
`mp_friendlyfire` has no effect.

### Chat

- `say` reaches every connected player; `say_team` reaches the sender and
  players who are its teammates. In **DM nobody is a teammate**, so
  `say_team` reaches only the sender. In TDM two Spectator-team members
  are teammates (same team name).
- **The dead talk to everyone**: there is no dead or spectator filter.
  The localization file has "*DEAD*" variants of the chat lines, but the
  server never picks them.
- Formats (localization keys and their English text, data of the game):
  `HL2MP_Chat_All` "name :  text"; `HL2MP_Chat_AllSpec` "*SPEC* name :
  text" for `say` from a Spectator-team member; `HL2MP_Chat_Team`
  "(TEAM) name :  text"; `HL2MP_Chat_Spec` "(Spectator) name :  text" for
  `say_team` from a spectator; `HL2MP_Chat_Team_Loc` (with a location) is
  never used, as HL2:DM gives no chat location.
- The name is coloured by the speaker's team: Combine (153, 204, 255),
  Rebels (255, 63, 63), anyone else and the server console (255, 178, 0).
- Each received chat line plays `HudChat.Message` on the receiving
  client.
- Lines are cut to 127 characters; the server log gets a line per chat
  (below), and `player_say` fires with `userid` and `text`.
- Voice: a listener hears a talker on the same team number (`sv_alltalk`
  aside), so in DM everyone playing hears everyone; spectators hear
  spectators.

### Suicide commands

`kill` and `explode`: if alive and the last suicide was at least 5 s ago,
the player dies at once (health 0) with itself as attacker and inflictor;
`explode` adds blast and always-gib. The death notice's weapon is the held
weapon's name (for example `physcannon`), or `world` if none. `killvector`
and `explodevector` are cheats.

### Disconnecting

The player is taken off its team and moved to team 0 (`player_team` with
`disconnect` true; no log line for it). In TDM a living player is killed
as a suicide by that move (frag −1, a `player_death`, the −1 team point to
team 0); in DM nothing more happens. Open questions 9.

### Game events (as HL2:DM fires them)

Definitions: HL2:DM's `resource/modevents.res` (in its VPK) redefines
`player_death`; the rest come from Half-Life 2's loose
`resource/gameevents.res` and `serverevents.res`.

| Event | Fields | When |
|---|---|---|
| `player_death` | `userid` (victim), `attacker` (scorer's user id, 0 for none), `weapon` (string) | every death outside intermission; also a team kill (HL2:DM's version, not team play's two-field variant) |
| `player_team` | `userid`, `team`, `oldteam`, `disconnect`, `autoteam` (always false), `silent` (always false), `name` | an actual team change (not a join to the same team; DM joins never fire one) |
| `player_spawn` | `userid` | every spawn |
| `player_hurt` | `userid`, `attacker` (player's user id or 0), `health` (left, at least 0) | every damage a living player takes |
| `player_say` | `userid`, `text` | every chat line |
| `player_changename` | `userid`, `oldname`, `newname` | name change |
| `round_start` | `timelimit` 0, `fraglimit` 0, `objective` `DEATHMATCH` | a restart only (not at map start) |
| engine: `player_connect`, `player_disconnect`, `player_activate`, `server_spawn`, … | as Source | as Source |

Not fired by HL2:DM's rules: `team_score`, `team_info`, `game_start`,
`game_end`, `round_end`, `player_score`, `teamplay_round_start`,
`hl2mp_ready_restart` (undeclared).

**Death-notice weapon name** (`weapon`):

1. if the scorer is the inflictor (bullets, melee, suicides by command):
   the class name of the scorer's **held** weapon at the moment of death,
   else `world`;
2. else the inflictor's class name (whether or not there is a scorer);
3. then, in order: a leading `weapon_`, `npc_` or `func_` is dropped
   (only one); otherwise a name containing `physics` becomes `physics`;
4. then `prop_combine_ball` → `combine_ball`, `grenade_ar2` →
   `smg1_grenade`, `satchel` and `tripmine` → `slam`.

Resulting names in play: `crowbar`, `stunstick`, `physcannon`, `pistol`,
`357`, `smg1`, `ar2`, `shotgun`, `crossbow_bolt` (the crossbow's bolt is
the inflictor, so `crossbow` itself appears only if the bow is somehow the
weapon in hand for a hit by the player), `grenade_frag` (frag), `rpg_missile`,
`smg1_grenade`, `combine_ball`, `slam`, `physics` (`prop_physics`,
`prop_physics_respawnable`, `prop_physics_multiplayer`), `physbox`
(`func_physbox`), `door` (a crushing `func_door`), `trigger_hurt`,
`worldspawn` (falling, drowning, the spawn telefrag), `world` (a command
suicide with no weapon). A bullet kill names the weapon held **when the
victim dies**, so a weapon switch in between is reflected.

### Server log lines

(Engine-prefixed with the date and time, as every Source log line.) A
player is written as name, user id, network id and team name in angle
brackets after the name, in quotes. Team names are Unassigned, Spectator,
Combine, Rebels; DM players show Unassigned.

- kill: killer, the word killed, victim, then with and the death-notice
  weapon name;
- suicide (scorer is the victim): the player, committed suicide with, the
  weapon name;
- world kill: the player, committed suicide with, the literal `world` (not
  the event's `weapon`, so a fall reads "world" in the log but
  `worldspawn` in the event);
- team change: the player **with its old team** in the brackets, joined
  team, the new team's name;
- chat: the player, `say` or `say_team`, the text in quotes;
- connect (with address), entered the game, disconnected (with reason),
  changed name: as Source;
- with `sv_report_client_settings` 1: the player's name and its
  `cl_cmdrate` on each settings change;
- the dedicated server logs that it executes its config file at each map
  load.

### Server configs

- Dedicated server: `server.cfg` (`servercfgfile`) is executed at every
  map load, when the rules are created; a listen server runs
  `listenserver.cfg` (`lservercfgfile`). Neither install ships either file.
- `cfg/valve.rc` (in the VPK, both installs, identical): runs
  `joystick.cfg`, `autoexec.cfg`, the command line, sets `hostname` "Half-Life
  2 Deathmatch" and `mp_forcerespawn 0`.
- `cfg/settings.scr` (the listen-server dialog): `maxplayers` 32 (range
  1–32), `mp_timelimit` 0, `mp_fraglimit` 0, `mp_teamplay` 0,
  `mp_weaponstay` 0, `mp_footsteps` 1, `mp_flashlight` 0,
  `mp_forcerespawn` 0.
- The rules' own registered defaults: H0's cvar table and the Constants
  above.

## Per-tick order

One server tick:

1. Each player's user commands (in the engine's simulation order). For
   each command: buttons are read; if the game is over, the rules clear
   the buttons and the player does nothing else (frozen); if the player
   is dead, the Respawn checks run and nothing else; otherwise movement
   and weapons (H1, H3), during which damage, deaths, scoring and events
   happen at once.
2. Other entities' thinks (projectiles, pickups and their respawn timers,
   delayed I/O).
3. The rules' update:
   1. if the game is over: change map once when intermission end < now;
      stop;
   2. time limit → intermission; stop;
   3. frag limit → intermission; stop;
   4. once a second (now > the next check time; next = now + 1.0): the
      all-ready check, then the `mp_restartgame` / `mp_readyrestart` check;
   5. a restart due (time > 0 and ≤ now) → restart;
   6. waiting for ready and all ready → message, restart at now + 5;
   7. the return of displaced placed pickups (H10).
4. The empty-server map-change timer.
5. Delayed entity I/O is serviced.

## Edge cases

- **A spectator in DM leaving to play**: `jointeam 2` puts it on team 0
  dead; with `mp_forcerespawn` 0 it waits for a button (after a release),
  which spawns it at once since its last death is long past.
- **A DM player going to spectate** does not die, loses no frag and gets
  no death.
- **TDM team change while the suicide limit runs**: the player changes
  team alive, keeps its loadout and gets the new team's model.
- **Model-driven team change in TDM** requires the 5 s model timer, which
  every spawn resets: right after a respawn a model change switches team
  (and kills the player) at once.
- **Invalid `cl_playermodel` on a Rebel**: the server sets the client's
  cvar to the Combine soldier while giving a citizen; the cvar change that
  follows arrives within the 5 s timer and is answered by setting the cvar
  to the citizen model actually used. No team switch results.
- **Spawn spot at the world origin** is never taken as free.
- **All spots occupied**: the telefrag victims are those near the first
  candidate, not the spot nearest anyone; they lose a frag each (world
  kill) and their deaths show `worldspawn`.
- **Restart with a player dead for less than 3 s**: not respawned, no
  weapons, frags and deaths zeroed; it respawns by the normal rules.
- **Negative `mp_fraglimit`**: any player (or TDM team) at 0 frags meets
  it: the game goes to intermission on the first tick.
- **`mp_timelimit` 0.5** is read as 0: no limit.
- **Map start freeze**: for the first 3 s of server time after a map
  loaded as a new game, with `developer` 0, players' commands are
  zeroed as if frozen (Open questions 10).
- **Team kill with the scorer dead**: credited normally (frags can go
  negative).
- **Frags below zero** are allowed everywhere.

## Quirks

Keep all of these (parity first):

1. A team kill gives the killer's team +1 while the killer loses a frag.
2. A suicide from a team change is charged to the new team.
3. `say_team` in DM reaches only the sender.
4. The 5 s team-change limit is never enforced; only the 0.3 s command
   rate and the model limit apply.
5. Dead players' chat goes to the living, without a "*DEAD*" mark.
6. Deaths during intermission change team scores but not frags or deaths.
7. The `timeleft` reply says there is no time limit during the last
   second.
8. The round-robin model counters survive map changes; the first
   replacement after a server start is the list's second model.
9. A respawn must wait for all buttons to be released once after death,
   then more than 3 s; a press before 3 s does nothing.
10. Going to spectate in DM costs nothing; in TDM it costs a death but no
    frag.
11. The first spawn spot of a class is twice as likely as the first
    candidate as any other (for N reaching the empty slot of the ring).
12. A restart can't be ordered during intermission and then fires on the
    next map.
13. The ready flag is lost on every spawn.
14. The world-kill log line says `world` while the event says
    `worldspawn` or `trigger_hurt`.

## Test cases

Implementer checks (headless; tick 0.015 s; default cvars unless said;
"DM" = `mp_teamplay` 0 at map load, "TDM" = 1):

| # | Setup | Input | After | Expected |
|---|---|---|---|---|
| 1 | DM, map loaded | set `mp_teamplay 1` | 10 s | still DM (all on team 0, game description "Deathmatch") |
| 2 | TDM: Combine 2 players, Rebels 1 | a player joins | spawn | team 3; `player_team` team 3, oldteam 0, autoteam false |
| 3 | TDM: 1 and 1, seed fixed | 1000 joins from an empty team-0 state | – | about half on each team (random choice) |
| 4 | fresh server, TDM, three players with citizen `cl_playermodel` joining Combine in turn | join | – | prisonguard, super soldier, police; a fourth gets `combine_soldier` |
| 5 | fresh server, TDM, Rebels, `cl_playermodel models/police.mdl` | join | – | `group03/male_02` (second citizen); its melee weapon is the crowbar |
| 6 | as 4, then map change, a fifth such player | join | – | `combine_soldier_prisonguard` (counter kept: index 1 again after wrapping) |
| 7 | DM, first join, `cl_playermodel none` | join | – | `models/Combine_Soldier.mdl`, stunstick, client told to set `cl_playermodel models/combine_soldier.mdl` |
| 8 | DM, model set 2 s ago | change `cl_playermodel` to a valid citizen | – | refused: cvar set back, chat says 3 more seconds (int(5 − 2)); model unchanged |
| 9 | DM, model set 6 s ago, alive | change to `models/police.mdl` | – | model police at once, chat names it; melee weapon unchanged until next spawn |
| 10 | TDM, Combine, alive, model set 6 s ago, last suicide long ago | `cl_playermodel` a citizen | – | team 3, a `player_death` with itself as attacker, frags −1, deaths +1, **Rebels** −1 team point |
| 11 | TDM, Combine, alive | `jointeam 3`, then again 0.15 s later | – | the first: team 3, death as 10; the second ignored |
| 12 | TDM, Combine, alive, suicided 2 s earlier (and respawned) | `jointeam 3` | – | team 3, **alive**, no death; model becomes a citizen |
| 13 | TDM, Combine, alive, frags 4 | `spectate` | – | team 1, observer, frags 4, deaths +1, Combine −1 |
| 14 | DM, alive, frags 4, deaths 2 | `spectate` | – | team 1, observer, frags 4, deaths 2, no `player_death` |
| 15 | `mp_allowspectators 0` | `jointeam 1` | – | refused, centre message; team unchanged |
| 16 | any | `jointeam 0` or `jointeam 5` | – | nothing changes |
| 17 | dead on tick 0, no button held | press attack on tick 100 | – | no respawn |
| 18 | as 17 | hold attack from tick 100 | – | respawn on tick 201 |
| 19 | dead on tick 0 holding attack, held until tick 300, then pressed on tick 305 | – | – | respawnable from the first command without buttons (tick 301); respawn on tick 305 |
| 20 | `mp_forcerespawn 1`, dead on tick 0, no input | – | – | respawn on tick 334; never an observer before it (mode 0 all along) |
| 21 | `mp_forcerespawn 0`, dead, no input | 60 s | – | still dead, mode 0, dead view |
| 22 | dead, ragdoll at (0, 0, 0), view yaw 0 pitch 0, open space | – | – | camera at (−96, 0, 14) |
| 23 | as 22, a wall plane at x = −50 | – | – | camera at x = −44 (box half-size 6 stops on the wall) |
| 24 | DM, spots A, B, C in entity order, last spot A, all free, N = 2 | spawn | – | C; the new last spot C |
| 25 | as 24, last spot C, N = 1 | spawn | – | A (the empty slot maps to the first) |
| 26 | spots A, B, C; A's candidate; a player 100 units from A, none near B | spawn | – | B |
| 27 | spots A, B; players 30 units from A and 60 units from A, and one within 128 of B; first candidate A | spawn | – | at A; the player at 30 dies (300 world damage, `worldspawn`, attacker 0, frags −1); the one at 60 lives |
| 28 | TDM on a map without `info_player_rebel` | Rebel spawns | – | on an `info_player_deathmatch` |
| 29 | a map with no spots of any class but one `info_player_start` | spawn | – | at it |
| 30 | spawned on tick 0 | pure blast damage 50 on tick 33; again on tick 34 | – | the first refused, the second applied |
| 31 | spawned on tick 0 | blast + burn damage on tick 10 | – | applied |
| 32 | spawn at a spot (100, 200, 300), angles (10, 90, 0) | – | – | origin (100, 200, 301), velocity 0, view (10, 90, 0), health 100, armour 0, aux 100 |
| 33 | DM, `mp_fraglimit 3`, a player at 2 frags | kills once | that tick | intermission: scoreboard shown, all frozen |
| 34 | TDM, `mp_fraglimit 3`, Combine 2 team points, a Combine player kills a Combine teammate (`mp_friendlyfire 1`) | – | – | killer frags −1, Combine 3 → intermission |
| 35 | DM, `mp_timelimit 1` from map load | – | tick 4000 / 4001 | not yet / intermission |
| 36 | intermission started on tick 0, `mp_chattime 10` | – | tick 666 / 667 | same map / map change to the next in the cycle |
| 37 | current map dm_steamlab, default cycle | map ends | – | next dm_underpass; from dm_underpass, dm_lockdown |
| 38 | current map halls3 | map ends | – | dm_lockdown |
| 39 | `nextlevel dm_runoff` set | map ends | – | dm_runoff |
| 40 | intermission | a dead player presses buttons | 10 s | stays dead |
| 41 | intermission | a frag grenade kills a player | – | no `player_death`, frags and deaths unchanged, thrower's team +1 |
| 42 | frags 5/3 and 2/7, Combine 9, Rebels 4, a player dead 1 s | `mp_restartgame 2` on tick 0 | ≤ 1 s + 2 s | all frags and deaths 0, team scores 0, the 1 s-dead player still dead, others respawned, `round_start` objective `DEATHMATCH`, time limit restarted |
| 43 | `mp_restartgame 90` | – | – | restart after 60 s |
| 44 | `mp_readyrestart 1`, two players | both say `ready` | – | at the next 1 s check: all-ready message; restart 5 s later |
| 45 | as 44, one says `Ready` | – | – | not ready (case sensitive); no restart |
| 46 | as 44, A says `ready`, then dies and respawns, B says `ready` | – | – | not all ready (A's flag cleared) |
| 47 | DM | A says `say_team hi` | – | only A sees it |
| 48 | TDM, dead Combine player | `say hi` | – | every player sees "name :  hi" (no *DEAD*), name in (153, 204, 255) |
| 49 | Spectator member | `say hi` | – | "*SPEC* name :  hi" to everyone |
| 50 | TDM, `mp_friendlyfire 0` | own rocket at own feet; teammate's rocket; teammate's bullet | – | own rocket hurts; the others do nothing (no blood for the bullet) |
| 51 | DM, A kills B with the SMG1, then A switched weapons before B died of the same hit | – | – | `weapon` names A's held weapon at B's death |
| 52 | death notices | kills by frag, RPG, crossbow, AR2 ball, SMG grenade, satchel, tripmine, thrown barrel, fall, `trigger_hurt`, `kill` holding the gravity gun | – | `grenade_frag`, `rpg_missile`, `crossbow_bolt`, `combine_ball`, `smg1_grenade`, `slam`, `slam`, `physics`, `worldspawn`, `trigger_hurt`, `physcannon` |
| 53 | DM, A dies of a fall | – | – | A frags −1, deaths +1; log "committed suicide with" `world`; event weapon `worldspawn`, attacker 0 |
| 54 | `kill` twice 3 s apart | – | – | the second refused (5 s suicide limit) |
| 55 | `timeleft` with 0.6 s left | – | – | reply: no time limit |
| 56 | `mp_fraglimit -1`, DM, one player at 0 frags | – | first tick | intermission |
| 57 | TDM, a living Combine player disconnects | – | – | `player_team` disconnect true, a `player_death` (suicide), frags −1 (record gone) |

Probe checks on the real dedicated server (game build 24600434, server
build 24600426):

| # | How | Expected |
|---|---|---|
| P1 | probe: RCON `mp_forcerespawn` right after start, with and without `+exec`ing nothing extra | 0 if the dedicated server runs `valve.rc` (Open questions 1) |
| P2 | probe: a fake client killed; read its observer mode and life state on the server every tick for 10 s | mode 0 throughout; never 4–7 (Correction 1) |
| P3 | probe: `mp_forcerespawn 1`, kill a fake client with no input | respawn about 5.01 s (tick 334) after the death |
| P4 | probe: hold a button across death, release, press at 2.5 s and at 3.1 s | no respawn at 2.5 s, respawn at 3.1 s |
| P5 | probe: TDM, `jointeam` back and forth 1 s apart | every change accepted (no 5 s limit); each from a team other than 0 costs a death |
| P6 | probe: fresh server, TDM, join three fake clients with citizen models to Combine | prisonguard, super soldier, police |
| P7 | probe: TDM team kill with `mp_friendlyfire 1`; read team scores and frags | killer −1 frag, killer's team +1 |
| P8 | probe: TDM team change by `jointeam`; team scores | the new team −1 |
| P9 | probe: crowd all of a small map's spots (more fake clients than spots, standing on them), add one | telefrag of players within 48 units of one spot; `worldspawn` in `player_death` |
| P10 | probe: does a dead body (ragdolled player) or a roaming spectator standing on a spot make it occupied | Open questions 3 |
| P11 | probe: `mp_timelimit 1` from a fresh map; time of intermission and of the map change | ≈ 60 s and ≈ 70 s after load |
| P12 | probe: server log for a kill, a fall, a chat line, a team change | the line shapes in "Server log lines" |
| P13 | probe: a spectator watching a fake client that dies; time until the watcher moves on | soon after the death (not 3 s): Open questions 8 |
| P14 | probe: `mp_readyrestart 1` with fake clients that can't chat | never restarts until every human and bot is ready |

## Open questions

1. **Does the dedicated server run `valve.rc`?** Its engine library
   contains the command; read `mp_forcerespawn` over RCON after a plain
   start (P1). If it does, every stock server respawns only on a button.
2. **H0's "5 s team change limit"**: wrong per the SDK (Correction 2);
   confirm on the probe (P5).
3. **Do dead or observing players occupy a spawn spot?** The occupancy
   test asks the spatial index for every player entity near the spot,
   without a life-state check; whether dead (non-solid) and observer
   players are in that index is engine behaviour. P10.
4. **The ring with an empty slot**: the 1–5 step and the doubled weight of
   the first spot follow from the entity-list walk; confirm with many
   spawns on one spot set (a histogram of spots used).
5. **Gravity-gun and prop kills**: which player is the scorer of a
   physics kill (the thrower, the last to touch) and for how long is H5's;
   this spec only fixes the name `physics`.
6. **Float ties**: 3.0 s and 0.3 s are whole tick counts at 0.015 s; the
   game compares floats, so the real respawn may come on tick 200 or 201.
   P4 at tick resolution would settle it; we use 201.
7. **Observer mode numbers**: this build inserts "point of interest" at 6
   and roaming at 7; `cl_spec_mode` 6 is accepted. Check the networked
   mode of a roaming spectator on the probe (7 expected), and whether
   CS:S's numbering in spectating.md still holds for CS:S's current build.
8. **Watching a dying target**: in HL2:DM a dead player is respawnable
   (and so no longer watchable) after its first command without a button
   held, which for a fake client is the next tick; the watcher's switch
   time is then the observer think's, not 3 s. P13.
9. **Disconnect in TDM** kills a living player as a suicide (frag −1 on a
   record about to vanish, a death notice, team 0 −1). Confirm the death
   notice appears (P-test on the probe with a fake client kicked).
10. **Map-start freeze**: the base player zeroes commands for the first 3 s
    after a map loaded as a new game with `developer` 0; whether a
    dedicated server's `changelevel` counts as a new game is engine
    behaviour. Check on the probe (move a fake client 1 s after a
    `changelevel`).
11. **SourceTV** joins are exempt from `mp_allowspectators`; not
    relevant to us unless a SourceTV-like recorder is built.
12. **Plan row C5** (only a question, not answered here): can Chimera's
    mode runtime host this: a per-player or per-team-score frag limit, a
    whole-minute time limit from map load, an intermission that freezes
    everyone and then changes map from a cycle file, join-time team choice
    by fewest players, team spawn classes with a fallback class, button
    respawn with a minimum dead time, and the team-point rule of Scoring?
    Combat Arms' TDM may already have most of it; M7 checks before
    lifting anything from cs_source.
13. **Probe results, round 2 (2026-11-03, dedicated server build
    24600426).** P11: with `mp_timelimit 1` the map changes 71 s after
    the load (the intermission's own start isn't in the server log),
    also with no player in the game. P5-P10 and P13-P14 aren't measurable
    with this server's bots: with `mp_teamplay 1` every `hl2mp_bot_add`
    bot is kicked at once (each join and kick costs the unassigned team,
    and later team 2, a point: their scores read −2 and −3), so the TDM
    cases need human clients or another bot.
14. **Probe results, round 3 (2026-11-04, build 24600426, `cases.py tdm
    tdm_spawns`).** The kicks were the bot manager's, not the rules':
    with `hl2mp_bot_join_after_player` 1 (its default) it removes bots
    while no human plays; with 0 they stay in TDM, and `hl2mp_bot_add
    combine` / `rebels` picks the team (a bot that first joined the other
    team moves, a death charged to its new team). The bot manager also
    moves its bots between teams on some spawns (not a rule). Measured,
    all as this spec says: P5: changes 1 s apart all accepted; an alive
    player's change to the other team kills it (frag −1, death +1, the
    **new** team −1), a dead player's change costs nothing (1 s or more
    than 5 s after its death), a change to its own team does nothing;
    test 11: a second `jointeam` 0.15 s after the first is ignored;
    test 12: a change 3.9 s after a `kill` (respawned by a button at
    3.08 s) leaves the player alive, one 5.5 s after kills it; test 13:
    `spectate` alive: a death, the frag given back, the old team −1.
    Friendly fire: with `mp_friendlyfire` 0 a teammate's pistol bullet
    traces (TraceAttack) but deals nothing, damage named for a teammate
    is refused; with 1 both hurt (8 for the pistol). P7: a team kill
    (ff 1): killer −1 frag, its team +1, the victim's death +1; an enemy
    kill: +1 and +1. A world kill (a fall) in TDM gives team 0 its point
    (+1, not shown), 2 and 3 unchanged. A bot removed in TDM costs team 0
    a point (Open questions 9). Spawns on dm_runoff: 48 Combine spawns all
    at `info_player_combine`, 36 Rebel at `info_player_rebel`.
15. **Probe results, round 4 (2026-11-04, build 24600426, `cases.py
    models log_lines ready spawns_more watcher`).**
    - P6: on a fresh server process, TDM, bots on Rebels given a citizen
      `cl_playermodel` and sent to Combine in turn got
      `combine_soldier_prisonguard`, `combine_super_soldier`, `police`,
      then `Combine_Soldier` (the list's first entry, written with the
      case it was first precached in), as Models says.
    - P9: dm_lockdown cut to two spots (the others removed right after a
      map load, before anyone spawned: removing the spot a spawn last
      used crashes the server), both taken: the player near the first
      candidate takes 300 from the world (`player_death` attacker 0, the
      log's "committed suicide with "world""), **also at 60 and 62 units
      from the spot**; one 66 away lived. The radius test is the
      player's box reaching within 48 (60 − 16 = 44), as the occupancy
      test, not its origin: test 27's "the one at 60 lives" is wrong.
    - P10 (Open questions 3): both take a spot. A bot killed 30 units
      from spot A (its body there) with B taken: a newcomer spawned at B
      and telefragged B's player on some rounds (no spot free), never
      at A while it was free. A spectating bot 40 units from B with A
      taken: every newcomer (5 of 5) spawned at A and killed A's player.
    - P12: `"Bot Name<2><BOT><Unassigned>" killed "Bot Name<3><BOT><Unassigned>"
      with "357"` (the attacker's weapon in hand), `... say "hello there"`,
      `... committed suicide with "357"` (`kill`), `... joined team
      "Spectator"` with the old team in the brackets, as "Server log
      lines" says. A DM player's `say_team` writes **no line**; a TDM
      player's writes `... say_team "text"`. A second chat line from the
      same player right after the first (under about a second) is
      dropped: no line, no `player_say` (a chat flood limit).
    - P13: a spectating bot in mode 4 watching a bot that died (`kill`;
      respawnable at once, life state 3) kept it as its target for the
      7.5 s measured. Bots may not run the observer's think as a human's
      client does: not settled; Open questions 8 stays open.
    - P14: one bot saying `ready`: no restart in 8 s; `Ready` doesn't
      count; a bot that said it, died and respawned has to say it again;
      the last `ready` restarted the game (deaths 0) 5.8 s later (the
      next 1 s check, then 5 s). No log line for the restart.
