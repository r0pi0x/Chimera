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
| M0 install, mount, maps, bodies in the sandbox | done (2026-11-23) | §4; the log |
| M1 weapons | M1a done (2026-11-23, its gaps fixed the same day); M1b done (2026-11-23): frags and stick grenades, rifle grenades, bazooka and Panzerschreck, the blast | §4; the log |
| M2 movement specifics | M2a done (2026-11-23): prone, deployed MGs, stamina pool, MGs in the ladders; M2b done (2026-11-23): sprint and stamina caps, jump, unduck, getting up, the hit slow, draw/reload times, FOVs, fire select, MG undeploy/reload/yaw (§4, "Specified since M2a") | §4; the log |
| M3 party rungs | M3a done (2026-11-23): the bodies animate (stances, movement, each gun's set, sights, deploy, fire and reload gestures, jump; death a ragdoll); M3b done (2026-11-23): each rung's class (body, melee; shared guns drawn with the ladder), DoD:S's HUD, dod_jagd and dod_flash in the pool, the explosives' body sets, the asset service's sets | §4; the log |
| M3c HUD and kill icons | done (2026-11-24): specs/dods/hud.md (from the data files); the player status strip from it, the install's pictures, DoD:S's kill icons in every app's feed | §4; the log |
| Spec measurements 2 | done (2026-11-24): spread, no-fire states, stock strike, partial and launcher reloads, rockets on the world, throws, grenade timings, backstab cone and reach, blast shielding, MG limits and ledges, prone edges; implemented the same day | §7; the log |
| Spec measurements 3 | done (2026-11-24): physics props under turbo physics, the stance-change cap, the sighted start, the backstab's timing and stances, what narrows the MG's yaw limit; implemented the same day, the movement's two after shared-movement S6 | §7; the log |
| Remaining | listed in §7: blocked on the retail client, unmeasured, or the map pool's walker | §7 |

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

- **Found** (M0, done) as the other games are (`chimera_core::mount::install`):
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
   - **M2a** (done): prone (measured going down; getting up a stand-in),
     the MGs deployed prone by the alt fire (the 1.2 s wait, the sights'
     spread, no movement), the stamina pool and the undeployed MGs' cost,
     the MGs as the ladders' `heavy` rungs; Source movement's generic
     `SourceStance` (a game's own box, eye, speed and no jump).
   - **Specified since M2a** (2026-11-23, `tools/dods_probe`; done in
     M2b and M1b, 2026-11-23: the log): sprint and stamina
     (movement.md: forward-only, starts at 80 u/s for 15 stamina, drains
     0.3 a tick above 100 u/s, regenerates 0.9 / 0.75 / 0.15 a tick still /
     ducked moving / moving, no delay; **the stamina caps the speed**:
     running 120 + stamina, sprinting 230 + stamina, 120 when empty);
     the jump (262.33 u/s, 268.33 ducked, apex 43 / 45; **no** stamina
     cost or scaling); unducking (0.21 s, the eye curve); getting up from
     prone (1.2 s, standing hull at once, movement allowed with a rising
     cap; duck goes to a crouch; jump does nothing); the slow when hit (a
     120 u/s cap; damage.md); the MGs' undeploy (alt fire, at once, then
     1.36 / 1.17 s before firing), deploy blocks (1.2163 / 1.1667 s), yaw
     limit fields (45 each side), reloads only deployed (4.6 / 4.71 s);
     draw and empty-reload times of every gun (weapons.md table); sights'
     FOV 55, scopes' 20 (0.315 s in, over 0.1 s); **the BAR's and StG44's
     alt fire is fire select (semi-auto, 0.3 s), not sights** (bar.toml
     and mp44.toml's sighted FOV is wrong); M1b's explosives (weapons.md,
     damage.md: frags pinned on the press, a 5.0-5.2 s fuse from the
     press, thrown from the eye at 500 u/s and 10° above the aim with the
     aim squeezed, about 420 u/s² gravity; rifle grenades 2000 u/s along
     the aim, a 3.63 s fuse; rockets 1300 u/s with 0.1 gravity; blasts
     150 × (1 − r / radius), radius 375 grenades / 200 rockets, a clear
     line needed, a no-health stun event of two thirds, the thrower hurt).
   - **Still unmeasured** (each an open question in its spec): the
     recoil kick (client-side: the server's punch angle stays 0, so it
     needs the retail client, app 300, on a listen server); deploying on
     a ledge (no ledge measured); what narrows the deployed yaw limit;
     the grenade throw speed's law above 15° up; rockets hitting the world
     (they didn't explode on the dedicated server); the rockets' and rifle
     grenades' reloads; a ceiling too low to get up; spread by stance.
4. **M3, party rungs**:
   - **M3a, body animation** (done): `player_anim` on Source's shared
     half (`source_engine::player_anim`; the move grid and the feet lifted
     there from TF2's), from the shared model's own activities
     (`ACT_DOD_<stance>_<IDLE|AIM|ZOOM>_<gun>`, `ACT_GET_DOWN_*`,
     `ACT_DOD_PRONE_DEPLOY_*`, `ACT_DOD_PRIMARYATTACK_*`,
     `ACT_DOD_RELOAD_*`, `ACT_HOP`), each gun's set in its data
     (`dods:body_anim`: the script's `anim_prefix` as the model names it);
     the rest-pose weapon bones (M2a's `bodies::HOLDS`) gone.
   - **Needs a spec** (no spec covers DoD:S's player animation; what the
     model's names and lengths don't answer is ours, a Known shortcut in
     `player_anim`): when the idle and when the aim set plays (ours: aim
     only with the sights up); walk against run (ours: the midpoint of the
     grids' ground speeds) and the sprint set (sprint now specified: movement.md); the
     feet's and the torso's rules (ours: TF2's) and the deployed yaw
     limits; `body_height`; the jump's sequence and landing; hand signals,
     the SMGs' stock strike, the bazooka's shouldering and zoomed reload,
     flinches; the MG42's deployed and reloading world models; how the
     game maps `anim_prefix` to the model's suffixes (`stg44` -> `MP44`,
     the Garand's and MG42's reload prefixes with no sequences).
   - **M3b, the rungs** (done): each DoD:S gun a rung of the Ultimate
     Gun Game, the holder its class (`classes`, data/classes.toml: a gun
     carried by one class only needs no draw; the pistols, frags, knife
     and spade are shared: the rung's class drawn before the match from
     the ladder's seed, as TF2's), the class's melee on its rungs (knife
     or spade; the Assault classes the kit's), DoD:S's HUD while its gun
     is in hand (`dods_ui::hud`: health, stamina, the gun and its rounds;
     the shell's crosshair), its maps in the pool (data/map_pool.toml:
     dod_jagd and dod_flash, the two whose spawn spots cover the whole
     map; data/map_fitness.toml), the explosives' own body sets, and the
     asset service's sets (`weapon/<id>` for every weapon, `players`,
     `map/<name>`; published 2026-11-23). Since done (M3c): the kill
     icons and the HUD from specs/dods/hud.md. Not done: the explosion's
     scorch decal and view shake (§7). Cross-game balance stays the host's opt-in
     (party_damage.toml gets a `dods` row only if asked).

Not planned: DoD:S's own rules (flags, bombs, waves, class menus) in its
app beyond free-for-all deathmatch for testing; Source TV; the winter
event's presents.

## 5. Risks

- **Prone and deployed MGs** need a third hull and eye height, timed
  transitions with input locked, and yaw limits on the view: Chimera's
  movement has stand/duck only. A generic "stance" extension in
  `chimera_sim::movement` (not DoD:S code) is the likely shape; it costs
  more than the rest of M2.
- **The probe** (`tools/dods_probe`, kept since 2026-11-23; HL2:DM's
  plugin with watched properties): the first plugin's "refused jumps,
  sprint and standing up" was its bot placed 2 units above the ground
  and a spot with something overhead (tools/dods_probe/README.md).
  What the dedicated server can't show: the recoil (client-side).
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

## 7. What remains (2026-11-24)

Every item left in this plan and in `games/dods`' Known shortcuts, by
what it waits on.

**Blocked on the retail client (app 300).** What the dedicated server's
files and its puppet bots can't show: the client draws or predicts it.
Installing it: SteamCMD with the cached login (dev-box-setup), `app_update
300` into `~/games/dods`, only if the account owns DoD:S (open question
1; not checked: a licence listing would print the account); about 4-5 GB
more on disk (an estimate: the server layout is 1.8 GB without the
client's `dod_english.vpk`, HL2's texture archives and the client
binaries); 216 GB free on 2026-11-24. Then a listen server measures it as
`tools/dods_probe` does (refcmp captures for the looks).
- The recoil kick (`gun`: `Recoil` read, no view kick; weapons.md OQ 8).
- The blast's stun and view shake (`explosives`: `dod_stun_*`; damage.md).
- The explosion's scorch decal (no data file names it; the client draws it).
- The view model: bob and sway, the iron-sight and prone offsets
  (`view`: `vm_ironsight_offset`, `vm_prone_offset` read, not applied; how
  they move needs seeing).
- The HUD's open questions (hud.md 1-7: the silhouette's red direction,
  the health bar's colour, the magazine box, the icon font, fades).
- Body groups: `helmetgroup` / `hairgroup` in the class scripts (which
  submodel each picks), team skins (`bodies`).
- The player animation choices (`player_anim`: idle against aim, walk
  against run and the sprint set, feet and torso rules, the jump's landing,
  hand signals, the SMGs' stock strike, the bazooka's shouldering and
  zoomed reload, flinches, the MG42's deployed and reloading models).

**Specified 2026-11-24, implemented the same day** (spec commit "DoD:S
spec measurements 2", then the implementation; the log): the spread
(base or alt + the whole move penalty over 45 u/s), no firing sprinting,
in the air or crawling, the SMGs' stock strike, the MG42's one bullet,
partial and launcher/rifle-grenade reloads by the key with the launchers'
locks, rockets on the world, the throw's speed, the thrower's half
velocity, the grenade windup and redraw, a low scattered bounce (the
spec's suggestion), the backstab by aim and the knife's reach and timing,
blasts not shielded by players, the deployed MG's client-held yaw limit
(`ViewYawLimit`), the pitch undeploy, ledge deploys, prone in the air /
sprinting / ducked, low ceilings, the eye curve, the still regeneration
at speed 0, the sighted cap 42. Left as shortcuts: the swing's shape
beyond the measured points, the ledge found by four downward traces,
what narrows the yaw limit (below), the speed while the sights rise.
Measured and not matched: going prone from a sprint (re-measured the
same day, below: it was the stance-change cap, not friction).

**Specified 2026-11-24 (measurements 3), implemented the same day**
(spec commit "DoD:S spec measurements 3"; `measure.py mpprops propmass
propsurvey pronecap sightstart backstab2 mgsweep`; the log):
- **The backstab** (damage.md): a slower swing settled on the press (a
  victim facing away in reach then): the 300 lands 0.405 s after it and
  every attack waits 0.9 s; a swing from the front keeps 0.21 / 0.4 s
  (`Backstab::Aim::slow`, the knife's and spade's data). Crouched and
  prone victims take the same cone (tests).
- **What narrows the deployed MG's yaw limit** (weapons.md): on the
  deploy, from 0° outwards in 5° steps, a box along that yaw from a
  point just behind the player at the deployed height; the first step
  meeting a solid ends that side (the limit the step before), a solid
  straight ahead refuses the deploy, a solid overlapping the player's
  own box doesn't count (`stance::Room::yaw_sides`; each side's limit
  held by the client as one `ViewYawLimit`). The numbers are a fit
  (Known shortcut): 3 units behind, 1.15 either side, 32 long, which
  match every placement the spec lists.
- **Sighted from rest**: 42 is already the wish speed (Source's movement
  clamps the wish to the stance's cap): +0.3 u/s a tick from 6.3 to 42 on
  the 120th tick, Garand and K98 (a test; no code change).
- **The spawn spots' walker** (physics_props.md 5): a per-game rule
  (`map::prop_physics::PlayerProps`, registered by the map's game in
  `PlayerPropRules`): DoD:S's `Turbo` passes only debris (the debris
  flag, `MapPhysics::debris`, or the non-solid modes) and multiplayer
  props still asleep since load (frozen ones too), every `prop_physics`
  and every settled multiplayer prop a wall; CS:S, HL2:DM and TF2 keep
  `Shadow` (multiplayer props, `prop_physics` up to 350 kg). On
  dod_avalanche as converted: 83 of its 138 server-side multiplayer
  props walls, 55 passed, as measured (heavy test). The maps swept again
  (below): the same three fit.
- The friction cvars are the shipped `sv_friction 8` / `sv_stopspeed
  50` (movement OQ 2 answered; the law ours uses is unchanged).

**Implemented after shared-movement S6** (the movement in
`chimera_movement`; was "waiting on shared-movement S4"):
- **Players and physics props under turbo physics**: a per-game choice in
  the shared movement, `QuakeConfig::turbo_physics` (off for CS:S, HL2:DM
  and TF2, whose golden runs are unchanged), on in DoD:S's config: no
  physics shadow (`prop_physics::NoShadow` on its players), so a
  `prop_physics` of any mass is a wall; a multiplayer prop is the
  push-away's soft prop (pushes the player back, 0.88 v − 33 a tick, and
  is shoved the less the heavier it is) only while in the push-away group
  (`PropGroup::live`: asleep since it spawned, or moving), a wall once
  settled; non-solid ones and debris are walked through. The shipped
  `sv_friction 8` / `sv_stopspeed 50` were already DoD:S's config.
  The spawn spots' walker and the movement now agree: a frozen prop
  (swept as the map's brushes) is a wall to both (the walker passed the
  frozen ones placed asleep before; in the game they are in the
  push-away group: a Known shortcut; the map pool not swept again).
  A push-away prop that can't be shoved (wedged in a doorway, very heavy)
  holds a player back, as in the game, where the walker (spec 5's rule)
  passes it. Tests: dods `props` (each spec test case, and a room the
  walker reaches past an asleep crate is one a player walks into, a
  settled crate shutting both out), `net_prediction`.
- **The speed cap while changing stance**: `stance::stance_cap`, A + (B −
  A) × smoothstep(k/80) both ways, the keys moving and steering as usual
  (`MoveStance::no_input` is gone); A or B the standing cap: the run's
  min(220, 120 + stamina), or the sprint's 230 + stamina with the sprint
  flag set and its keys held; the sprint starts getting up past 80 and
  drains only above 100 going down. Matches the spec's test cases to
  0.05 u/s but one: pressed sprinting, the flag ends a tick sooner than
  measured (62.67 on the 64th tick, the game's 74.12; Known shortcut in
  `stance`). Tests: dods `stance` (running, sprinting, keys let go, from
  rest and steering, getting up running and sprinting), `net_prediction`
  (going prone and getting up sprinting into the props, predicted with no
  errors). DoD:S's golden run re-taken (`0x7396bb2d05258af7`: the course
  moves while going prone); `NET_VERSION` net76 (`MoveStance` lost a
  field).

**Still unmeasured on the dedicated server** (specs' open questions):
the spread pattern inside the cone (no impact message on the server:
needs the retail client); the MG yaw check's exact point, width and
reach (weapons.md OQ 11; ours a fit), and whether a ledge deploy checks
the same; the thrower's velocity law beyond three
throws; what `dod_grenademaxspeed` caps; the swing's shape and what the
backstab's press-time check looks at; why a resting grenade's blast hurt
nobody; the rocket's stun amount; whether a shot or blast makes a
settled prop soft again (physics_props.md OQ 3); smoke grenades and the
live-grenade pick-up (ours by design, below).

**Ours by design** (not parity gaps for the party): ladders, water and
fall damage are Source's shared ones; the slow and the MG's stamina cost
are the server's (prediction learns them at the next correction); no
teams in a free-for-all (a gun's class is its nation's); smoke grenades
and the live-grenade throw-back (open question 2); objectives and team
walls not read; `INFO`'s binds and tick.

**The map pool** (checked 2026-11-24, then swept again the same day):
the seven maps out of the pool were not kept shut by doors or round
logic. No DoD:S stock map has a door entity; `func_teamblocker` /
`func_team_wall` (solid to one team, read as solid to all) made no
difference worth a converter change (dod_avalanche spot area 700 -> 731
m2 with them left out, so they stay). Two things confined the spawn
spots' walker, and the walker now takes both:
- the physics props (`prop_physics_multiplayer`, 105-348 a map: crates
  and sandbags): the movement already lets a player through them
  (specs/cs_source/physics_props.md 4.1-4.3, `source_movement::pushaway`
  and `shadow`), but the walker met them as walls; it now goes through
  every prop a player gets past (`map::prop_physics::walked_through`:
  multiplayer props, and `prop_physics` up to the 350 kg a shadow pushes),
  as does `map_fitness`'s bots' walk;
- DoD:S's jump-ups: the walker climbed a step only; it now climbs 0.8 m
  as CS:S's (`SpawnClimbs`; the jump's apex is 43 units, movement.md).

dod_avalanche's spot area went 700 -> 734 m2 with the props alone and
-> 3134 m2 (its bots' region 2984) with the climb: it joined the pool
(dod_jagd, dod_flash, dod_avalanche). Swept again under DoD:S's turbo
walker rule (measurements 3, above): dod_avalanche 3092 m2 of spots,
its bots' region 2942 of 2942 (its whole mesh), no spot off or stuck:
it stays; dod_jagd (1760 / 1714) and dod_flash (4881 / 4704) stay; the
six others move by at most 5% and stay out (their drops, below). On the six others the bots' own
main region is the part: their spawn points stand on ledges the rest of
the map is reached from by drops (0.9-1.8 m) with no way back
(`map_fitness --leaks`, "one-way", the area beyond each):
dod_anzio 2483 of 6576 m2 (drops at 197 -758 -316, -1270 -817 -333,
-541 -797 -303), dod_argentan 742 of 5794 (-2894 -1024 76, -2362 -1033
80, -2244 -1033 80), dod_colmar 448 of 5019 (226 2618 46, -128 2598 51,
-20 -2037 42, 246 -2028 50; 0.9 m), dod_donner 1402 of 5569 (-2372 -2490
-46), dod_kalt 612 of 5018 (689 5118 126, 1378 4744 120, 4803 4291 131,
5561 4281 121), dod_palermo 980 of 4520 (1909 3238 -300, 2904 1801 -234,
1545 -797 -315). The pool's rule counts only areas walked to from a spawn
and back (map-pool-fitness.md), so a drop isn't counted: these stay out.
Admitting them needs a rule change (the region as the largest part a
player can walk round in that the spawns drop into, the ledges left out)
or spots only below the drops; a decision, not a measurement.

2026-11-24, one-way drops count (the user: "we can try it, revert later
if not good"): the spots' region is every area walked to from the spawn
points, not only those walked to and back (`spawns::DROPS_REACHABLE`, one
constant; `spot_checks::region_linked` and `map_fitness` follow it; test
`an_area_reached_by_a_one_way_drop_is_in_the_region_while_drops_count`).
All nine maps now cover their whole mesh (spots cover 1760-6864 m2 against
mesh 1714-6576, none off the map or stuck) and all nine are in the pool;
to revert, flip the constant and deny the six again.

DoD:S's multiplayer props: answered 2026-11-24 (above, "measurements
3"): neither CS:S's soft props everywhere nor HL2:DM's solid shoved
ones; with turbo physics most are walls at load (the walker and, since
shared-movement S6, the movement follow it). Still open: how a player of a
game without Source's physics (Combat Arms', Halo's movement) meets a
Source map's props: both movements meet every prop as a wall, so on a
DoD:S map such a player can't pass a crate the spots' walk went through;
no spec says what a mashup should do (a design question for the user).

## Log

- 2026-11-24: the movement's two from measurements 3 implemented on
  `chimera_movement` (§7, "Implemented after shared-movement S6"): turbo
  physics as a per-game movement config (`turbo_physics`, DoD:S's), the
  stance-change speed cap (`stance::stance_cap`, `MoveStance::no_input`
  gone); frozen props walls to the spots' walker too; DoD:S's golden run
  re-taken, the other three unchanged; `NET_VERSION` net76.
- 2026-11-24, measurements 3 implemented: the backstab a slower swing
  settled on the press (`Backstab::Aim::slow`: 0.405 s to the hit, 0.9 s
  to the next attack; `deliver::Shot::backstab_ahead` traces the swing on
  the press), the MG's yaw limit narrowed by the 5° check on the deploy
  (`stance::Room::yaw_sides`, a solid ahead refusing it; `DodsStance::
  yaw_sides`), the spots' walker by a per-game rule
  (`prop_physics::PlayerProps`, `PlayerPropRules`, `MapPhysics::debris`
  and `DebrisProp` from the debris spawnflag; DoD:S registers `Turbo`),
  the nine maps swept again (the pool unchanged: dod_jagd, dod_flash,
  dod_avalanche). The sighted start needed nothing. Deferred to
  shared-movement S4: the players' prop push rule and the stance-change
  cap (§7). Tests: dods `weapons::a_backstab_lands_0_405_s_after_the_press_and_blocks_0_9_s`,
  `crouched_and_prone_victims_take_the_same_backstab`,
  `the_backstab_is_settled_on_the_press`, `stance::obstacles_beside_a_deployed_mg_narrow_its_yaw_limit_in_5_degree_steps`,
  `sighted_from_rest_accelerates_0_3_a_tick_to_42`,
  `heavy::install::dod_avalanche_s_settled_props_are_walls_to_the_spots_walker`;
  chimera_sim `spawns::under_turbo_physics_only_asleep_and_debris_props_open_the_room`.

- 2026-11-24, spec measurements 3 (a spec commit: specs/dods/ and
  tools/dods_probe only): multiplayer props and the push limit (new
  physics_props.md), the stance-change speed cap (the "prone from a
  sprint" mismatch), the shipped friction cvars, the sighted start,
  backstabs on crouched and prone victims and the backstab's timing,
  what narrows the MG's yaw limit. `probe_prop` takes `key=value`
  keyvalues and mass `-`; `measure.py` gained mpprops, propmass,
  propsurvey, pronecap, sightstart, backstab2, mgsweep. What they
  settled is in §7 ("measurements 3").

- 2026-11-24, the map pool again: the spawn spots' walker and
  `map_fitness`' walks go through the physics props a player gets past
  (`prop_physics::walked_through`), DoD:S's walker climbs 0.8 m
  (`SpawnClimbs`), `map_fitness --leaks` lists one-way drops and its
  probe says how players meet a physics prop; dod_avalanche joined the
  pool (§7). CS:S's 109 maps and HL2:DM's 8 swept again: no map newly
  fit, no spot area moved by more than 10% (cs_militia's wander at 4
  players 8.9 -> 10.6 s, its area the same: the sight samples; its pool
  and data left as they were); TF2's pool is its allow list, which the
  measurements can't change (not swept). Tests: chimera_sim `spawns::a_room_closed_by_a_crate_a_player_pushes_is_reached`,
  cs_source `logic_physics::a_player_pushes_a_light_crate_but_not_a_heavy_one`.

- 2026-11-24, spec measurements 2 implemented (an implementation session
  from specs/dods only): `gun` (the 45 u/s move threshold, partial reload
  times), `blocks` (no attack sprinting, in the air, crawling), the knife,
  spade and the SMGs' stock strike as data (a 77-unit line and a box
  swept as deep, the hit 0.21 s on, 0.4 s refires; Chimera's
  `Backstab::Aim` and `Melee::gun_primary`, a gun swinging on its alt
  fire only; `WeaponData::is_melee` keeps it out of the ladders' melee),
  `script_numbers` (no `BulletsPerShot`, no melee `FireDelay`),
  `explosives` (the throw's speed above 22.5°, half the thrower's velocity
  along the aim, the windup and redraw, `key_reloads`, the launchers'
  locks as their cycles, Chimera's `BounceRule` for the low scattered
  bounce), `stance` (prone in the air, from a sprint with
  `SourceStance::no_input`, from a crouch holding the ducked box, low
  ceilings, the 7/3 eye curve both ways, the pitch undeploy, the client's
  `ViewYawLimit` instead of the undeploy at the yaw limit, ledge deploys,
  the sighted cap) and `stamina` (still only at speed 0, the sprint flag
  going prone). Scenario tests from the specs' test cases in
  tests/it/{weapons,stance,explosives}.rs.

- 2026-11-24, spec measurements 2 (a spec commit: specs/dods/ and
  tools/dods_probe only, from measurements on the dedicated server and
  the game's data files): the probe plugin logs DoD:S's "FireBullets"
  message (each shot's spread, seed, mode); `measure.py` gained the
  cases spread, nofire, stock, reach, backstab, partial, launchers,
  rocketworld, throw, nadetiming, bounce, shield, mgyaw, mgside, ledge,
  proneedge, ceiling, stillregen. What they settled is in §7
  ("Specified 2026-11-24"). The same day the store's `players` set was
  republished with the HUD pictures and kill icons (`sets dods --build
  25338204 --only players`, manifests snapshotted first) and the guest
  dressing converted again for the new converter version (`convert
  dods`).

- 2026-11-24, M3c, the HUD and the kill icons: specs/dods/hud.md written
  from the server's data files alone (a spec commit), then implemented
  from it: `game::hud` reads the kill icons (`mod_textures.txt`'s
  `d_<weapon>` cells of `sprites/hud/death_icons`), each weapon's
  `weapon_s` / `weapon_g` / `ammo` / `ammo2` cells and clip fields and the
  classes' `healthimage` silhouettes through Source's new shared sheet
  reader (`source_engine::hud::sheets`); `dods_ui::hud` is hud.md's status
  strip on the 640 x 480 layout scaled to the window (background, the
  silhouette red from the top as health goes, the warning-coloured health
  bar, the weapon's picture, the magazine emptying from its clip fields,
  "x N" spare magazines, 53 stamina slices with the low colours below
  0.35), writing the weapon and rounds until the pictures are here;
  `load_pictures` registers the kill icons for every app's kill feed.
  `chimera_pack sets dods` records the pictures in `players` (the store's
  sets need `sets dods --build` again to serve them). Tests: `heavy::hud`
  (every weapon's icon and pictures, the twelve silhouettes, the layout
  equal to the install's), dods_ui's `hud_nodes`, the units' spec cases.
  Capture: apps/ultimate_gun_game/scenes/dods_hud.scene. The map pool
  checked (§7).

- 2026-11-23, M3b, the rungs: `classes` (data/classes.toml, classes.md's
  loadouts: each class's body, melee and weapons, primary first; a
  weapon's own class the first carrying it; `HOOK` draws a shared
  weapon's class per rung from the ladder's seed, `RungClass` follows the
  rung on every machine, the class's knife or spade replaces the kit's
  melee); `bodies::wear_classes` (a DoD:S player wears its weapon's class
  body by name; the targets keep theirs); `PARTS.hud` DoD:S's own
  (`dods_ui::hud`: health number and bar, stamina bar, the gun's name and
  clip | reserve; the shell's crosshair; layout ours, no spec); the map
  pool (the 2026-11-23 sweep of the nine maps: none stuck or off, but on
  seven the spawn spots cover a part of the map, the rest behind the round
  logic's doors: dod_jagd and dod_flash allowed); the explosives'
  `dods:body_anim` (`GREN_FRAG`, `GREN_STICK`, `BAZOOKA`, `PSCHRECK`, the
  rifle's with the `RIFLEGRENADE` reload, as the shared model names them);
  `chimera_pack sets dods` records every weapon's `weapon/<id>` (the knife,
  spade and explosives too). Tests: UGG's `dods::rungs` (body, movement,
  HUD; a shared gun's drawn class; the ladder's draws), `heavy::served`
  (an MG42 rung's kit from the asset service alone). The sweep's tool ran
  one map per process: a second map in one process found no install
  (fixed after: the first map's `harness::Sim` turned the process's
  installs off, `mount::offline`, which now leaves a tool's main thread
  alone; party's `heavy::sweep_installs`).
  Published into the host's store (manifests snapshotted first): the
  dedicated server's build 25338204 (`publish dods --build`, 40,279
  files, 2.2 GB stored), its sets (`sets dods --build`, new: every
  weapon's `weapon/<id>` about 5 MB each, `players` 8 MB, the nine maps
  100-132 MB each) and its converted guest dressing (`convert dods`).
  `chimera_pack sets` took `--build` for it. DoD:S's HUD declares its kill
  feed the app's (`register_game_kill_feed`), and party_roster's every-HUD
  tests have a DoD:S rung.

- 2026-11-23, M2b and M1b (§4, "Specified since M2a", from the specs):
  `stamina` (sprint on `+speed` forward, standing, from 80 u/s, 15 to
  start, 0.3 a tick above 100, regeneration 0.9 / 0.75 / 0.15; predicted),
  `stance::step` the tick's speed cap (120 + stamina running, 230 +
  stamina sprinting at most 320, 120 empty; 120 while hit-slowed; 50 with a
  rocket shouldered; ducked the run's 220 by Source's third), clamped the
  same tick (`ground_clamp`); the jump 262.33 / 268.33 ducked
  (one impulse of 268.33: Source's jump adds it standing to the half
  tick of gravity already taken, sets it ducked); the duck 0.4 s (the
  measured curve) and the unduck counting its release tick
  (`unduck_counts_first_tick`: 0.21 s); duck slows from its first tick;
  getting up from prone with the standing box at once and a smoothstep cap
  50 to 220 (leaving a game's stance is no duck for the input scale), duck
  while prone to a crouch (`SourceStance::crouch`); the
  slow's ×0.5 stand-in gone. Weapons: the measured draw and empty-reload
  table in every gun's data; sights FOV 55 and scopes 20, 0.315 s after
  the press (`gun::sights`); the BAR's and StG44's alt fire a semi-auto
  select (a burst of one, 0.3 s); the MGs' deploy 1.2163 / 1.1667, undeploy
  1.3635 / 1.1667, reload only deployed (4.6 / 4.71), undeployed past 45°.
  M1b (`weapons::explosives`): frags and stick grenades (`dods:grenade`:
  Chimera's `Throwable` with the game's launch, the fuse from the pin, 5.1
  s, off in the hand when held; the throw's lift and squeeze, 500 u/s up to
  a fit of the measured speeds aiming up; gravity 420), rifle grenades
  (`dods:rifle_grenade`: 2000 u/s, 3.63 s fuse), bazooka and Panzerschreck
  (`dods:rocket` on `chimera:projectile`: 1300 u/s, gravity 80, explode on
  players and the world; shouldering 0.79 / 0.99 s, no fire lowered), the
  blast 150 × (1 − r / radius) to the body's middle with a clear line,
  the thrower hurt; Source's explosion look and its particle materials on
  DoD:S's maps and guest dressing; the frags in the ladders' grenade
  band, the rifle grenades and rockets in the launcher band. Chimera:
  `grenade::Projectile::new`, `grenade::ThrowFrame`. Shortcuts left (each
  in its file): the stun event and view shake, characters not shielding
  a blast, the throw speed's fit above 15° up, our pin/throw/redraw times,
  CS:S's bounce, the rockets' and rifle grenades' reloads, shouldering not
  predicted, the getting-up eye curve, the yaw limit undeploying rather
  than holding the view, the still/moving threshold of the regeneration.

- 2026-11-23, spec measurements (`tools/dods_probe`, port 27050):
  movement.md (sprint, stamina, jump, unduck, getting up, the slow),
  weapons.md (draw and reload times, FOVs, fire select, MG deploy and
  reload, grenades, rifle grenades, rockets), damage.md (the slow's
  120 cap, explosions); §4, "Specified since M2a" and "Still
  unmeasured".
- 2026-11-23, M3a, body animation: DoD:S's bodies play the shared model's
  sequences (`player_anim`): going prone and getting up over the stance's
  time (the files' 37 frames at 30 fps are the measured 1.2 s), prone,
  crawling and deployed, crouched, standing, walking, running (the run
  grid's forward speed is the measured 220, the crouch-walk's 73, the
  crawl's 50), the sights' zoom set, `ACT_HOP` in the air after a jump;
  each gun's set from `dods:body_anim` (the BAR plays the rifle's set with
  its own reload, as its script says); `move_x` / `move_y`, `body_pitch`,
  `body_yaw` with the files' aim-matrix autolayers, IK locks and rules; fire
  and reload gestures by stance and deploy. The gun's bone is in a hand in
  every stance (`heavy::player_anim`), so `bodies::HOLDS` is gone. Death is
  Chimera's ragdoll (the model's only death activity is `ACT_DIERAGDOLL`).
  TF2's move grid and feet rules moved to `source_engine::player_anim`
  (`square`, `move_grid`, `turn_feet`). What needs a spec: §4, M3.

- 2026-11-23, M1 gaps and M2a: (a) targets and other players held no gun:
  nothing gave a DoD:S body its held model (`map::Held`, which each game's
  player animation sets: `bodies::hold_weapons` now), and the DoD:S `w_`
  models hang on the body's `ValveBiped.weapon_bone` (SMGs, pistols, MGs)
  or `weapon_bone_left` (rifles), which the bodies have but only the
  game's animations put in the hands (at rest they sit at the feet);
  until the bodies animate (M3a, since done) each weapon bone rests where the body's
  own `StandIdle_tommy` / `StandIdle_RIFLE` puts it (`bodies::HOLDS`). (b) The guns' sounds:
  every entry of `scripts/game_sounds_weapons.txt` loads with a DoD:S map
  and with its guest dressing, under `dods:` (`maps::own_sound`, TF2's
  rule), the scripts' shots, swings and world reloads and the view
  models' events (`Weapon_Thompson.Draw`) named so. (c) `dods:axis_rifleman`
  wore the US body: on a map loaded for another game's gameplay only
  unnamed bodies stayed (`GameKit::stay`); named ones stay now (hostages
  never load with a players' kit). M2a: `stance` (prone: `+alt1`, Z in the
  party; the eye eased to 10 over 80 ticks by a power ease fitted to the
  measured heights, then the 24-high box, 50 u/s crawl, no jump; getting
  up a stand-in), the MGs' `dods:mg` (deploy prone with the alt fire,
  first shot 1.2 s later, the sights' spread, no movement; undeployed 15
  stamina a shot), `stamina` (100, 60 a second back), the MGs as `heavy`
  rungs; Chimera's Source movement gained `SourceStance`. Not done (no
  measurement): §4, "Needs measuring".

- 2026-11-23, M0 fixes and M1a: (a) the dedicated server's
  `hl2/hl2_textures_dir.vpk` lacks its archives: an archive whose parts
  are missing is replaced, in its place in the search path, by the same
  archive of another Source install (`SearchPath::borrow_incomplete_archives`;
  donors: the other registered games' installs, every `[games.*] path`,
  Steam's Source games; HL2:DM's on the dev box), so the maps' HL2
  concrete, brick and detail textures load. (b) No game-name tables: a
  game's full name is `GameInfo::name`, named process-wide where no world
  is at hand (`chimera_core::game_registry::known`); the pickers, the sandbox's
  headings and the target cards read it; architecture.rs's layering lists
  say `<games>` (every `games/<id>` crate but the crate's own) instead of
  each game. A new game is its crates plus its roster lines (`GUESTS`,
  `CLIENTS`). (c) Sandbox targets on DoD:S maps: see the next entry's
  findings (`tests/dods/heavy/sandbox_targets.rs`). M1a: every gun of
  weapons.md's table but the explosives as data (data/weapons; the MGs in
  no ladder until they deploy, M2), each built from its script at run time
  (`script_numbers`: damage, bullets, accuracy, alt accuracy, move
  penalty, recoil, fire delay rounded up to ticks, clip, spare clips, the
  fire/empty/deploy sounds and the models; the committed numbers are the
  spec's fallback, checked equal in `heavy::scripts`), sent to clients as
  server documents; `dods:gun` (spread by sights and movement, the sights
  and scopes blocking the trigger 0.49 s, the Garand's reload only when
  empty), `dods:hit_slow` (0.5 s, a leg 2 s), the knife and spade (60,
  300 from behind, the hit zones on both through Chimera's new
  `[melee] hitgroup_scale`). Stand-ins, each a Known shortcut in its
  file: reload and draw times, the sights' and scopes' FOV, the slow's
  speed (×0.5), the swing's reach and backstab cone (CS:S's knife's); not
  done: the recoil kick (shape unmeasured), the BAR/StG44 sighted delay,
  the SMGs' stock strike, the view model's iron-sight offset, HUD and kill
  icons (M3).
- M1b (not started; only what weapons.md specifies): frag grenades (180
  damage; gravity −420, throw speed 500-1400 from the cvars; fuse,
  radius and priming unmeasured), rifle grenades (a separate weapon, 2
  rounds, 100 damage; flight, fuse and radius unmeasured), the bazooka
  and Panzerschreck (shoulder ≈0.79 s with alt fire, unshoulder ≈0.99 s,
  no fire unshouldered, ≤50 u/s shouldered; `mp_rocketdamage` 150 /
  `mp_rocketradius` 200; flight unmeasured). Each needs its unmeasured
  numbers measured first (a `tools/dods_probe`, §5). Measured since (the
  spec-measurement entry above): ready to implement.
- 2026-11-23, M0: `games/dods` and `games/dods/ui` (crates `dods`,
  `dods_ui`), a guest in the party roster (`GUESTS`, `CLIENTS`); no
  apps of its own yet (as TF2). The install found by `$CHIMERA_DODS_ROOT`
  / `[games.dods] path` / Steam app 300 and mounted from
  `dod/gameinfo.txt`; all nine stock maps convert (BSP 20, no LZMA seen)
  with 20 + 20 spawns (dod_palermo 16 + 16); the twelve class bodies
  (MDL 44, their sequences from the shared MDL 48) load with our reader
  unchanged, a head box each; worn by name, and the sandbox's practice
  targets (`dods:us_rifleman` ... `dods:axis_rocket`). `dods:movement`:
  Source's with the measured run speed (220, no back-pedal penalty),
  friction law (8 over 50), duck third, stand 72 / eye 58, duck 45 / eye
  34 (the envelope its maps carry for other games' characters); jump a
  CS:S stand-in. One stand-in gun so a body can be worn (a body follows
  the gun in hand's game): the Colt as data (data/weapons/colt.toml, the
  script's numbers, no sounds or recoil yet); as a pistol it can be a
  rung of the automatic gun games until M1. `chimera_pack` knows `dods`
  (`weapon/colt`, `players`, `map/<name>`, the converted guest
  dressing); not published. Found: the dedicated server's layout lacks
  `hl2/hl2_textures_*.vpk`'s archives (only the `_dir` file): the
  textures DoD:S's maps borrow from HL2 (concrete, brick, detail
  textures) are stand-ins there, as on TF2's server layout; install.md's
  "everything the party needs" holds for DoD:S's own files only.
- 2026-11-23: plan written; dedicated server installed on this box;
  first spec pass (weapon and class scripts read, cvars listed, models'
  hitboxes read, movement and damage measured with puppet bots).
