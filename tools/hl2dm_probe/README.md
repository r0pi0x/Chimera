# HL2:DM probe

A SourceMod plugin (`chimera_probe.sp`) that replays per-tick inputs on a
bot in a real Half-Life 2: Deathmatch dedicated server and logs its
position, velocity, ground and duck state, water level, move type, aux
power, health and armour after every tick, its sounds and its hurt
events; `probe.py` drives it over RCON and `measure.py` runs the cases of
specs/hl2dm/movement.md (P2-P13) and suit.md (P3-P5) and prints each beside
the spec's numbers. The CS:S probe (tools/css_probe) is the model; this
one has no CS:S-only calls.

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
```

Run (a port other than CS:S's probe's 27030 and the default 27015):

```
cd <server> && ./srcds_run -game hl2mp -console -port 27040 +ip 127.0.0.1 \
    -insecure -norestart +tv_enable 0 +maxplayers 4 +exec chimera_probe.cfg +map dm_lockdown
python3 -c "import sys; sys.path.insert(0, 'tools/hl2dm_probe'); import probe; print(probe.connect().exec('hl2mp_bot_add'))"
cd tools/hl2dm_probe && python3 measure.py            # or: measure.py accel jump aux
```

Quit the server afterwards (`quit` over RCON). `hl2mp_bot_add` prints
"invalid team index" in deathmatch but adds the bot; the plugin replaces
its commands every tick, so its AI moves nothing. The ladder cases need
`changelevel dm_resistance` and the bot added again.

Notes (build 24600426, 2026-11-03):
- The placement tick runs a move: a start velocity is already one tick
  old in the first row (`measure.py` reads it so).
- The bot's player is a Combine soldier: its steps are
  `npc/footsteps/hardboot_generic*.wav`.
- `m_vecMaxs` reads 60 while ducked (the view offset reads 28): the
  collision property's box, not necessarily the movement's ducked hull.
- `m_flMaxspeed` reads 0 (the max speed isn't kept there).
- Aux power is `m_flSuitPower`, armour `m_ArmorValue` (a data field).
