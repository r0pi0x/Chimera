# DoD:S probe

The HL2:DM probe (`tools/hl2dm_probe`) for Day of Defeat: Source's
dedicated server, with the few additions DoD:S needed and nothing
DoD:S-specific in the plugin:

- `probe_watch <bot|weapon|tracked|index> <data|send> <prop> <type>`:
  any entity property logged after every tick ("P" lines), so DoD:S's own
  fields (stamina, sprint, prone, deploy, yaw limits, FOV, a grenade's
  damage and radius) are read without new plugin code. `probe.DODS` is
  the player list `measure.py` uses.
- `probe_settime <index> <data|send> <prop> <seconds>`: a time property
  set to now plus seconds (the hit slow's "slowed until").
- `probe_tprel <client> <dx dy dz>`: a player placed relative to the
  first tracked entity (a blast's victim beside a grenade).
- `probe_hull <from> <to>`: a standing player's box swept through the
  world (finding open, flat ground).
- No "Shotgun Shot" temp-entity hook (DoD:S has none); armour and aux
  power read −1.

`probe.py` drives it over RCON (as HL2:DM's), `measure.py` runs the
cases the specs quote (specs/dods/movement.md, weapons.md, damage.md):

```
python3 tools/dods_probe/measure.py              # every case
python3 tools/dods_probe/measure.py sprint jump  # some
```

Setup (once per machine; nothing here changes the game's own files):

1. Dedicated server, Steam app 232290, anonymous SteamCMD login, into a
   folder of your choice (`$CHIMERA_DODS_SERVER`, default
   `~/games/dods_ds`).
2. Metamod:Source 1.12 and SourceMod 1.12 (Linux) extracted into `dod/`
   (32-bit server, as HL2:DM's).
3. Compile and install the plugin:
   `cd <server>/dod/addons/sourcemod/scripting && ./spcomp <repo>/tools/dods_probe/chimera_probe.sp -o ../plugins/chimera_probe.smx`
4. `<server>/dod/cfg/chimera_probe.cfg`:

```
sv_lan 1
sv_cheats 1
rcon_password chimera-probe
mp_timelimit 0
mp_winlimit 0
mp_limitteams 0
mp_autoteambalance 0
volume 0
```

Run (port 27050: not the default 27015, nor the CS:S probe's 27030 or
HL2:DM's 27040):

```
cd <server> && ./srcds_run -game dod -console -port 27050 +ip 127.0.0.1 \
    -insecure -norestart -nobreakpad +tv_enable 0 +maxplayers 8 +exec chimera_probe.cfg +map dod_avalanche
```

`measure.py` adds the bots itself (`probe.setup(r, bots=2)`: the game's
own `bot` command; the second joins the Axis as the victim). Quit the
server afterwards (`quit` over RCON).

Notes (build 11003710, 2026-11-23):

- **Place the bot on the ground.** Placed even 2 units above it, the bot
  never leaves its stance: it can't stand up from a duck or from prone
  (the unduck timer stays at its start). That, and a spot with something
  low overhead (jumps cut at 30 units, then refused), were why the first
  probe "couldn't jump, sprint or get up". `measure.START` is open
  ground on dod_avalanche (z 128.031), about 1,800 units clear towards
  yaw 315.
- State carries over between runs: a bot left prone or deployed stays so
  (a deployed MG keeps it prone: `measure.standing` strips it first).
- The bot's command time (the rows' `gametime`) is its tick base, not
  the server's clock; time fields of entities (a grenade's detonate
  time) are in another base: compare ticks, not times, across them.
- Recoil is client-side: the server's punch angle stays 0 when firing.
  Measuring it needs the retail client on a listen server.
- `probe_give` gives guns without spare ammunition: `measure.gun` sets
  200 rounds of the gun's type.
- A grenade's or rocket's entity index changes from run to run: watch
  `tracked` properties, not an index.
- Rockets explode on players but not on the world on this server (they
  slide or stop and stay): put the victim in the rocket's path.
- Blast damage needs a clear line to the victim; the stun event that
  comes with it doesn't (specs/dods/damage.md, "Explosions").
- `dod_debugmgdeploy` prints nothing on a dedicated server (it wants a
  listen server's host).
