# HL2:DM probe

A SourceMod plugin (`chimera_probe.sp`) that replays per-tick inputs on a
bot in a real Half-Life 2: Deathmatch dedicated server and logs its
position, velocity, ground and duck state, water level, move type, aux
power, health, armour and life state after every tick, its active
weapon (clip, ammo, next attack times), tracked entities (projectiles,
props, items: position, velocity, think tick, effects), its sounds,
damage and traced hits, bullets (the "Shotgun Shot" temp entity: origin,
direction, seed, spread) and deaths and spawns; server commands give
weapons and ammo, hurt, kill, place players, spawn props and read or set
any entity property (the list in the plugin's header). `probe.py`
drives it over RCON; `measure.py` runs the cases of
specs/hl2dm/movement.md (P2-P13) and suit.md (P3-P5), `cases.py` those of
rules.md, suit.md, weapons.md, projectiles.md, gravity_gun.md and
maps_entities.md, each printed beside the spec's numbers. The CS:S probe
(tools/css_probe) is the model; this one has no CS:S-only calls.

Setup (once per machine; nothing here touches the game's install):

1. Dedicated server, Steam app 232370, anonymous SteamCMD login, into a
   folder of your choice (`$CHIMERA_HL2DM_SERVER`, default
   `~/games/hl2dm-ds`).
2. Metamod:Source 1.12 and SourceMod 1.12 (Linux builds; the same
   package as the CS:S probe's, which carries HL2:DM builds) extracted
   into `hl2mp/`. The server is 32-bit (`srcds_run` starts `srcds_linux`):
   no `addons/metamod_x64.vdf`. Leave out the CS:S probe's plugins.
3. Compile and install the plugin:
   `cd <server>/hl2mp/addons/sourcemod/scripting && ./spcomp <repo>/tools/hl2dm_probe/chimera_probe.sp -o ../plugins/chimera_probe.smx`
4. `<server>/hl2mp/cfg/chimera_probe.cfg`:

```
sv_lan 1
sv_cheats 1
rcon_password chimera-probe
mp_forcerespawn 1
mp_timelimit 0
mp_fraglimit 0
hl2mp_bot_quota 0
nb_player_stop 1
```

`nb_player_stop 1` matters: an `hl2mp_bot_add` bot's NextBot behaviour
crashes the server (a segfault in the bot's action update) as soon as
the bot dies, whatever kills it (`sm_slay`, damage, a fall). Stopped,
the bot still runs the commands the plugin writes, dies and respawns
safely. `probe.setup` sets it again with `mp_forcerespawn 1`, which the
dedicated server resets to 0 at the map load.

Run (a port other than CS:S's probe's 27030 and the default 27015):

```
cd <server> && ./srcds_run -game hl2mp -console -port 27040 +ip 127.0.0.1 \
    -insecure -norestart -nobreakpad +tv_enable 0 +maxplayers 4 +exec chimera_probe.cfg +map dm_lockdown
python3 -c "import sys; sys.path.insert(0, 'tools/hl2dm_probe'); import probe; probe.setup(probe.connect())"
cd tools/hl2dm_probe && python3 measure.py            # or: measure.py accel jump aux
python3 cases.py                                      # or: cases.py weapons gravity
```

Quit the server afterwards (`quit` over RCON). `-nobreakpad` keeps a
crash's minidump on the machine. `hl2mp_bot_add` prints "invalid team
index" in deathmatch but adds the bot. A map change drops the bots: run
`probe.setup` again. The ladder cases need `changelevel dm_resistance`,
the water cases (`cases.py swim drown`) `changelevel dm_underpass`.
`cases.py tdm tdm_spawns` change the map themselves (TDM on dm_lockdown,
then dm_runoff for its team spots) and come back to DM on dm_lockdown.

Notes (build 24600426, 2026-11-03):
- The placement tick runs a move: a start velocity is already one tick
  old in the first row (`measure.py` reads it so).
- The bot's player is a Combine soldier: its steps are
  `npc/footsteps/hardboot_generic*.wav`.
- `m_vecMaxs` reads 60 while ducked (the view offset reads 28): the
  collision property's box, not necessarily the movement's ducked hull.
- `m_flMaxspeed` reads 0 (the max speed isn't kept there).
- Aux power is `m_flSuitPower`, armour `m_ArmorValue` (a data field),
  the health remainder `m_flDamageAccumulator`.
- The bot's tick base runs 4 ticks ahead of the server's tick
  (`sv_clockcorrection_msecs` 60): its command time (the rows'
  `gametime`) is that clock, so timers its commands start (fuses, an
  item's respawn) land 4-5 server ticks after the specs' counts.
- A bot whose behaviour is stopped doesn't animate: a level shot at
  its head height passes over it. Its head hitbox sits low and to one
  side: from 200 units ahead of a bot facing the shooter, pitch 1.25
  and yaw +1 hit it (hitbox 0, hitgroup 1; `cases.py heads`); pitch 3
  the chest, 8.5 a thigh.
- Tracked entities' rows end with their angles; `probe_prop` takes
  spawnflags (72: hanging motion-disabled until the gravity gun punts
  it, so punts can be measured with nothing under the prop). Above
  `measure.ROOF` there is open sky for in-air throws and punts.
- A frag's origin circles its mass centre as it spins (3-5 units):
  fit its velocity past the circle (`cases.window_velocity`), not from
  one tick's step.
- Bullet impacts aren't dispatched as effects with no client connected;
  the bullets' temp entity carries the spread instead.

Notes (round 3, 2026-11-04):
- TDM bots: with `hl2mp_bot_join_after_player` 1 (the default) the bot
  manager kicks every bot while no human plays; `cases.change_map` sets
  it to 0. `hl2mp_bot_add combine` / `rebels` picks a bot's team. The
  manager still moves its bots between teams on some spawns: a team
  change no command asked for is its own.
- An input file may hold `cmd <row> <client> <command>` lines (`probe.
  run_full(..., cmds=...)`): the client runs the command on that row
  (0: the server); "K" lines log them, "V" lines now carry the death's
  attacker and `player_team`'s new and old team. `probe_score` prints
  every player's team, frags, deaths and life state and the team scores
  (`probe.score`).
- A bot kept across a map change is put in the server again: the plugin
  unhooks before hooking its damage, so each hit is logged once (before
  version 3 each map change doubled the "D" lines).
- A tracked physics prop's `m_vecAbsVelocity` reads 0: take its velocity
  from its position's steps (`cases.speed`).
