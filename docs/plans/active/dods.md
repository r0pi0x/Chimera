# Plan: Day of Defeat: Source on Chimera

Started 2026-11-23 (user's go, 2026-11-23; ranked first in
[next-games.md](next-games.md)). This is the plan only: no code lands
with it. CLAUDE.md asks for a plan before a new mount adapter or loader;
this is that plan, kept up to date as the work goes.

Goal: Day of Defeat: Source is a sixth game library (`games/dods`, crate
and id `dods`) that lends its guns, movement and bodies to the party
apps (the Ultimate Gun Game first), as CS:S, Combat Arms, HL2:DM, TF2
and Halo CE do ([new-game.md](../../new-game.md); HL2:DM and TF2 are
the model Source ports, [hl2dm.md](hl2dm.md), [tf2.md](tf2.md)). Parity
first: a DoD:S rung plays as the real game, from specs, measurements
and the game's own data.

## Status

| Milestone | State | Summary |
|---|---|---|
| Plan and first specs | done (2026-11-23) | this plan; specs/dods/: install, weapons, movement, classes, damage (measured on the dedicated server where marked) |
| M0 install, mount, maps, bodies in the sandbox | not started | §4 |
| M1 weapons | not started | §4 |
| M2 movement specifics | not started | §4 |
| M3 party rungs | not started | §4 |

## 1. Rules for this port

- **Clean room**, as for TF2. DoD:S's game code is **not** in Valve's
  public Source SDK 2013 (it holds HL2:DM's and TF2's game code, not
  DoD:S's; checked 2026-11-23), and any leaked DoD:S code is never read.
  Behaviour reaches us from: the game's own data files (weapon and class
  scripts, read in place), measurements of the dedicated server with
  puppet bots (a SourceMod plugin, as `tools/hl2dm_probe`), the cvars
  the server lists, and public pages (Valve Developer Community, the
  community wikis). Spec sessions write only `specs/dods/`, in our own
  words; implementation reads the specs only.
- **No game files in the repository.** Numbers that the scripts carry
  are read from the install (or the asset service) at run time; the
  specs name the fields and give the values only to check against.
- **Parity first, data-driven first**: numbers in `games/dods/data/`,
  behaviour as hooks registered under `dods`; our options later.
- **"Put guns into existing modes"** (user): DoD:S's guns become rungs
  of the Ultimate Gun Game. Its flag/point capture modes, class menus
  and its own app's rules are not on the path to the party (§3).

## 2. The install

| Layout | Steam app | What it has | Use |
|---|---|---|---|
| The retail client (`Day of Defeat: Source`) | 300 (paid; Linux build) | everything | play, refcmp captures, the packing source for `chimera_pack dods` |
| The dedicated server | 232290 (SteamCMD, anonymous) | **everything the party needs**: the nine stock maps (BSP 20), `dod/dod_pak_dir.vpk` with 1,699 textures, 800 sounds, 819 models and the plain-text weapon and class scripts, `hl2/` shared VPKs (specs/dods/install.md) | development, tests and measurements on this box (`~/games/dods_ds`, installed 2026-11-23) |

- **Found** as the other games are (`chimera_core::mount::install`):
  `$CHIMERA_DODS_ROOT`, then `[games.dods] path` in
  `chimera.local.toml`, then the first-run dialog's path, then Steam's
  libraries (app 300). The server layout from SteamCMD needs the config
  line or the variable.
- **Required files**: `dod/gameinfo.txt`, `dod/dod_pak_dir.vpk`,
  `hl2/hl2_misc_dir.vpk`.
- **The mount**: the search path from `dod/gameinfo.txt`
  (`source_engine::search_path`, as HL2:DM and TF2): `dod/custom/*`,
  `dod/dod_english.vpk` (client only; absent on the server),
  `dod/dod_pak.vpk`, then `hl2/hl2_textures`, `hl2_sound_vo_english`,
  `hl2_sound_misc`, `hl2_misc`, `platform/platform_misc`.
- **Unencrypted.** The weapon and class scripts ship twice, as `.txt`
  and as same-size `.ctx` copies; the `.txt` copies are plain
  KeyValues, so no key is needed (unlike TF2).

## 3. What DoD:S reuses, and what is new

### Reused as is (Source, the CS:S/HL2:DM/TF2 path)

- **Formats**: BSP 20 (all nine maps; no LZMA lumps seen yet), VPK v2
  `_dir` + numbered archives, VTF/VMT, MDL (player models are **v44**,
  the shared animation model v48; check our reader takes v44 headers),
  WAV/MP3 sounds, soundscripts, `surfaceproperties.txt`.
- **The world**: `source_engine`'s map conversion, props, decals, water,
  the kept dressing for guests on other maps.
- **Movement base**: Source's ground/air movement with `sv_accelerate`
  10, `sv_airaccelerate` 10, `sv_friction` 4, `sv_stopspeed` 100,
  `sv_gravity` 800, `sv_stepsize` 18 (cvars), tick 0.015 s (66.7 Hz).
  Measured: run speed 220 u/s in every direction (no back-pedal
  penalty), duck speed ×1/3, eye 58 (CS:S 64); ground friction is
  stronger than the cvars read (movement.md: drop = max(v, 50) × 8 ×
  dt).
- **Weapons**: hitscan bullets with the shared seed, hitgroups (our
  `Hitgroup`), magazines and reloads, view kick, the weapon data model
  ([weapon-data.md](../../weapon-data.md)); melee on Chimera's delayed
  swing; grenades on CS:S's thrown grenade; rockets on TF2's projectile.

### New in DoD:S (specs/dods/)

| Feature | What it is | Party gun game? |
|---|---|---|
| Classes and loadouts | 6 per team, each a fixed primary, secondary, melee and grenades (classes.md) | **Yes, as the rung's look**: a DoD:S gun makes its holder the class that carries it (body, speed), as TF2's rungs do; no class menu |
| Stamina sprint | `+speed` sprints while stamina lasts; jumping costs stamina (movement.md) | **Yes**: part of the gun's movement while a DoD:S gun is in hand |
| Prone | `+alt1` lies down: low hull and eyes, slow crawl, timed transitions (movement.md) | **Yes**, needed for MG deploy; camera/hull work Chimera lacks today (§5) |
| Deployed MGs | MG42/.30 cal deploy prone or on a ledge: spread 0.2 → 0.01-0.025, yaw limits, ≈1.2 s before the first shot; fired undeployed they drain stamina (weapons.md) | **Yes, later rungs** (M2): the MGs are useless undeployed |
| Iron sights and scopes | Garand/K98 alt fire = sights; Springfield/K98 scoped zoom | **Yes** (M1) |
| Rifle grenades | the Garand's and K98's second weapon: a launcher with 2 rounds, fired as a projectile (weapons.md) | **Yes**, as a rung's alt weapon or its own grenade rung (M1b) |
| SMG melee (no bayonets) | the Thompson's and MP40's alt fire strikes with the stock (the assault classes carry no knife); DoD:S has no bayonets, the rifles' alt fire is the iron sight (weapons.md) | **Yes** with the SMG's rung |
| Melee weapons | knife (US) and spade (Axis), 60 damage | **Yes**: DoD:S's kit knife (`GameParts::knife`) |
| Frag and smoke grenades; live-grenade pick-up and throw-back | frags 180 damage; smokes; a thrown grenade can be picked up and thrown back (the `_live` weapons) | frags **yes** (grenade rungs); smoke **later**; throw-back **later** (an option) |
| Rockets (bazooka, Panzerschreck) | must be shouldered (alt fire, ≈0.8 s) before firing; ≤50 u/s while shouldered; 150 damage, 200 radius (cvars) | **Yes**, as rungs (M1b) |
| Hit zones, being slowed, stun | head ×2.5, limbs ×0.75, no stomach zone, no falloff, backstab ×5; a hit slows the victim 0.5 s (leg 2 s); blasts shake the view (damage.md) | **Yes** (M2), part of the feel |
| Flags, capture zones, bomb objectives, wave respawns | the game's modes | **No**: not on the party's path; only if its own app ever plays DoD:S rules |
| Class limits, team-pick menus, win panel, freeze cam | its app's front end | **No** for the party |

## 4. Milestones

Each lands with its scenario tests (from the specs' test cases) and,
where the install is needed, `heavy::` tests that skip without it.

1. **M0, install, mount, maps, bodies in the sandbox.**
   - `games/dods` (crate `dods`, id `dods`, content folder `dod`, tick
     0.015 s, 0.0254 m per unit, Steam app 300, console namespaces `dods`
     and `dod`), `games/dods/ui`, `apps/dods`, `apps/dods_server`, as
     new-game.md's checklist.
   - `game::mount::open` from `gameinfo.txt`; a heavy test that the
     server layout mounts and the nine maps list.
   - Maps through `source_engine`'s conversion; spawns from
     `info_player_allies` / `info_player_axis` (to confirm in
     install.md's open questions); `dods:<map>` in the party apps; a map
     pool data file after a fitness sweep.
   - The twelve class bodies (`models/player/{american,german}_*.mdl`,
     animations in `dod_player_shared.mdl`) on placeholder movement in
     the sandbox; hitboxes from the models' `dod` set (damage.md).
   - The roster: `GUESTS` line and `CLIENT`; `chimera_pack dods`
     (`base`, `common`, `players`, `map/<name>`), not published until
     the orchestrator says so.
2. **M1, weapons** (weapons.md):
   - **M1a**: the rifles, SMGs, pistols and the carbine as data, numbers
     read from the install's scripts (as TF2 reads its `.ctx`), the
     specs' numbers as the last resort; DoD:S's spread, recoil and
     movement penalty; iron sights and scopes; the knife and spade.
   - **M1b**: frag grenades, rifle grenades, bazooka/Panzerschreck
     (shoulder before firing), as hooks on Chimera's projectile and
     grenade parts.
   - View models (`v_*.mdl`, one model with arms), sounds, kill icons.
3. **M2, movement specifics** (movement.md): `dods:movement` on Source
   movement with DoD:S's config: per-class speeds, stamina sprint and
   its jump cost, prone (hull, eyes, transition times, crawl speed),
   being slowed when hit, the deployed-MG state (yaw limits, no
   movement); then the MGs as rungs.
4. **M3, party rungs**: each DoD:S gun a rung of the Ultimate Gun Game,
   the holder its class (a gun carried by one class only: no draw
   needed; the pistols and melee are shared: the rung's class drawn
   before the match from the ladder's seed, as TF2's), DoD:S's HUD while
   its gun is in hand (health, stamina, ammo, crosshair), its kill
   icons, `GameParts::knife`. Cross-game balance stays the host's
   opt-in (party_damage.toml gets a `dods` row only if asked).

Not planned: DoD:S's own rules (flags, bombs, waves, class menus) in its
app beyond free-for-all deathmatch for testing; Source TV; the winter
event's presents.

## 5. Risks

- **Prone and deployed MGs** need a third hull and eye height, timed
  transitions with input locked, and yaw limits on the view: Chimera's
  movement has stand/duck only. A generic "stance" extension in
  `chimera_sim::movement` (not DoD:S code) is the likely shape; it costs
  more than the rest of M2.
- **No DoD:S probe yet**: the measurement plugin of 2026-11-23 (a
  scratch SourceMod plugin, not kept) made its puppet bot move, duck,
  go prone, fire, deploy and shoulder, but the game refused it jumps,
  sprint and standing back up (specs/dods/movement.md, "Probe
  limits"). Sprint, stamina, jump and leaving prone stay unmeasured
  until a `tools/dods_probe` (as `tools/hl2dm_probe`) finds why; that
  tool is M2's first task. SourceMod and Metamod are installed in the
  dedicated server on this box (`~/games/dods_ds/dod/addons`).
- **Model versions**: player models are MDL v44 (CS:S's are v44/48;
  check `source_formats` accepts v44 with DoD:S's extra animation
  layers in `dod_player_shared.mdl`).
- **Hard-coded numbers**: DoD:S keeps its spread-by-stance, recoil
  curve, deploy times, rifle-grenade speed and hitgroup multipliers in
  code, not in the scripts; without reference code they come only from
  measurement. Unmeasured ones are flagged in each spec.
- **Asset service**: DoD:S is paid (app 300); the ownership check works
  as for CS:S. The server layout lacks `dod_english.vpk` (localised
  strings): weapon display names come from our own data
  (gun-display-names.md) as for the other games.
- **Disk**: the dedicated server is 1.8 GB on this box.

## 6. Open questions

1. Does the user own DoD:S (app 300) for refcmp captures, or is the
   dedicated server enough for now? (Not blocking: M0-M2 run on the
   server's files.)
2. Smoke grenades and the live-grenade throw-back in the party: later
   options, or never?

## Log

- 2026-11-23: plan written; dedicated server installed on this box;
  first spec pass (weapon and class scripts read, cvars listed, models'
  hitboxes read, movement and damage measured with puppet bots).
