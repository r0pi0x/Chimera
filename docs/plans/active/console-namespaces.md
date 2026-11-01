# Console namespaces

Approved by the user on 2026-10-31 (docs/backlog.md, "Console namespaces").
With several games in one app, flat console names collide (`bot_add`,
`options`, `cl_righthand`). Every cvar and command gets a namespace, like
our ids.

## Design as built

- **Full names.** `<namespace>:<name>`: `cs_source:sv_cheats`,
  `combat_arms:MaxPlayer`, `chimera:noblock`. The full name always works.
  Names are case-insensitive, as before.
- **Which namespace.** A name registered with a namespace is in it. A bare
  name is one of Source's and goes where the classification table says
  (below; `console_names::is_engine`): `chimera:` when Chimera's engine
  implements it for any Source game (`map`, `fps_max`, `noclip`), else
  `cs_source` (`console::SOURCE_NAMESPACE`) while it is a game's own rule
  or tuned value (`mp_roundtime`, `sv_gravity`). Combat Arms registers its
  names as `combat_arms:` (its `ca_*` names keep their prefix: they are
  its own established names). Chimera's own names are `chimera:`: the
  `chimera_*` ones lose the prefix (`chimera_noblock` is
  `chimera:noblock`), our general settings lose their `sv_`/`cl_`/`net_`
  prefix (`cl_hitmarker` is `chimera:hitmarker`, `OLD_NAMES`), and the
  console's own language (`bind`, `alias`, `exec`, `echo`, `help`, `find`,
  `scope`, ...) is `chimera:`, as every game's console uses it. The
  input actions (`+jump`) are Source's names, so `cs_source:`; binds keep
  them as typed.
- **Namespace aliases.** Each game's `GameInfo::console_namespaces`: the
  first is the canonical one (saved in configs, sent on the wire), the
  rest are accepted everywhere a namespace is (`css:` for CS:S, `ca:` for
  Combat Arms). Making `css` canonical is swapping that list's order.
- **Short names.** A short name resolves in the scope's game first, then
  `chimera`, then wherever it is the only one. Several left: an error
  with a hint listing the full names and `scope`.
- **Scope.** `scope <game>` (an alias works) pins it; `scope auto`
  follows the default again: the local player's `GameIdentity` (the
  party apps: the game of the gun in hand), else the app's default game.
  `scope` alone says what it is.
- **Old names.** `OLD_NAMES` (renamed settings) and the old prefixes
  (`chimera_*`, `mashup_*`) still resolve, hidden from completion, so
  scripts, tests, `playtest.ps1` and saved configs keep working.
- **Saving.** config.cfg writes cvars by full name. `bind`, `bindgame` and
  `alias` store each command's name in full (`Console::qualify_line`),
  except Chimera's own names that no game shares, `+`/`-` actions and
  the binds the client polls (`slot1`, `buymenu`, ...: `Console::keep_short`).
- **Network.** Replicated cvars travel by full name; a client's commands
  for its server (cheats, `kill`: `ClientCommand`, our RCON) are sent
  with the full name; the server checks `CHEATS` and friends by full name.
  Userinfo keys stay the short names (Source's userinfo keys, which the
  simulation reads: `cl_autowepswitch`).

## Engine and game names (2026-11-01)

The user's decision (HL2:DM plan, open question 2, option C): the
engine-level Source names moved from `cs_source:` to `chimera:`, in
Source's spelling; game-tuned values stay per game, so HL2:DM can have
`hl2dm:sv_gravity`. The classification is one table,
`chimera_core::console_names` (`ENGINE_NAMES`, `ENGINE_PREFIXES`); a name
in it is `chimera:`, any other bare name is `cs_source:`. It was made from
every name the CS:S app registered (343 under `cs_source:`):

| Kind | Examples | Namespace |
|---|---|---|
| Maps, connection, server operation | `map`, `maps`, `changelevel`, `quit`, `connect`, `disconnect`, `listen`, `status`, `kick`, `kill`, `hostname`, `maxplayers`, `password`, `sv_cheats`, `sv_logfile`, `callvote`, `vote`, `sv_vote_*`, `sv_allow*` | `chimera:` |
| Network | `rate`, `cl_updaterate`, `cl_interp*`, `cl_extrapolate*`, `cl_lagcompensation`, `sv_minrate`/`sv_maxrate`, `sv_(min\|max)updaterate`, `sv_unlag`, `sv_maxunlag`, `net_*`, downloads | `chimera:` |
| Video, audio, camera | `fps_max`, `fov_desired`, `r_*`, `mat_*`, `snd_*`, `dsp_*`, `voice_*`, `volume`, `musicvolume`, `closecaption`, `cam_ideal*`, `firstperson` | `chimera:` |
| Input, chat, weapon switching | `sensitivity`, `m_*`, `+attack`..`-walk`, `say`, `slot1`..`slot10`, `invnext`, `drop`, `use`, `spec_mode`/`spec_next`/`spec_prev`/`spec_player`, `spectate`, `name`, `cl_autowepswitch` | `chimera:` |
| Console, tools, demos, screenshots | `developer`, `con_*`, `log`, `bug`, `screenshot`, `jpeg`, `host_timescale`, `playdemo`, `stopdemo`, `demo_*`, `cl_showfps`, `cl_showpos` | `chimera:` |
| Cheats the engine implements | `noclip`, `god`, `setpos`, `setang`, `getpos`, `give`, `impulse`, `ent_fire`, `ent_dump` | `chimera:` |
| Decals | `decalfrequency`, `mp_decals`, `cl_logofile` | `chimera:` |
| Movement and physics each game tunes | `sv_gravity`, `sv_maxspeed`, `sv_accelerate`, `sv_airaccelerate`, `sv_friction`, `sv_stopspeed`, `sv_stepsize`, `sv_bounce`, `sv_maxvelocity`, `sv_*bunnyhopping`, `sv_ladder_*`, `sv_footsteps`, `phys_*`, `ragdoll_*`, `rope_*`, `cl_forwardspeed`, `viewmodel_fov` | `cs_source:` |
| CS:S's rules and server settings | `mp_*` (but `mp_decals`), `bot_*`, `sv_alltalk`, `sv_legacy_grenade_damage`, `spec_freeze_*`, `timeleft`, `motd` | `cs_source:` |
| CS:S's buying, bomb, teams, radio | `buy*`, `autobuy`, `rebuy`, `cl_autobuy`, `jointeam`, `joinclass`, `chooseteam`, `radio1`..`radio3`, `cl_c4progressbar` | `cs_source:` |
| CS:S's HUD, crosshair, view | `cl_crosshair*`, `cl_dynamiccrosshair`, `cl_radar*`, `hud_*`, `overview_*`, `cl_bob*`, `cl_wpn_sway_*`, `cl_righthand`, `cl_ejectbrass`, `muzzleflash_light`, `nightvision`, the menus (`gameui_*`, `serverbrowser`, `spec_menu`) | `cs_source:` |

Result: 217 of the 343 names moved to `chimera:` (32 of them the `+`/`-` input actions), 126 stay `cs_source:`. The old full names are
aliases of exactly the names the table lists (`Console::home_of`:
`cs_source:fps_max` and `css:fps_max` are `chimera:fps_max`), so saved
configs, binds and scripts keep working; configs are written with the new
names from now on. `CHEATS`, `PLAYER_COMMANDS` and `SERVER_COMMANDS` name
`chimera:noclip`, `chimera:kill`, `chimera:changelevel`, ... (and stay
`cs_source:bot_*`, `cs_source:mp_restartgame`); a client's commands for
its server travel as `chimera:setpos ...`. A game registering its own
`hl2dm:sv_gravity` needs nothing here.

## Steps

1. Core (`chimera_core::console`): full-name registry, resolution, scope,
   aliases, old names, qualification, tables by full name. Tests.
2. Registrations: `chimera:` and `combat_arms:` names at the call sites;
   `GameInfo::console_namespaces`; the scope following the identity.
3. Client console: completion and help by display name; binds.
4. Network: full names on the wire; NetSim tests.
5. Docs (DESIGN, OBSERVABILITY, README/AGENTS names), docs_index.

## Later

- A game-tuned value HL2:DM also has (`sv_gravity`) is CS:S's own: HL2:DM
  registers `hl2dm:sv_gravity` in its namespace, backed by its movement
  config (decided 2026-11-01; engine names are `chimera:`, above).
