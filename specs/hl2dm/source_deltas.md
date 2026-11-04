# Half-Life 2: Deathmatch: Source deltas and install facts (H0)

Source basis: the public Source SDK 2013 multiplayer game code (the shared
movement, the HL2 player and suit, the HL2:DM player, rules and weapons,
the client view and HUD lists), read in place in a separate spec session;
the game's own files on this box (game build 24600434, dedicated server
build 24600426, `steam.inf` PatchVersion 10889068), read for names, sizes,
versions and counts only. No game was launched and no server was started.
Status: draft

## Summary

HL2:DM runs on the same Source engine branch as CS:S, so file formats,
rendering, sound scripts, entity I/O and most map entities behave as
specs/source/ and the engine parts of specs/cs_source/ say. What changes is
almost all game code: gravity is 600 (not 800), the jump is a 160 units/s
impulse (about 20 units high), speeds come from a walk/run/sprint triple
(150/190/320) with sprint paid for by the suit's aux power, the player box
is 72 tall with eyes at 64, ladders are `func_useableladder` entities, a
fixed 10 fall damage replaces CS:S's formula, bullets have no penetration
or range falloff and take their damage against players from the weapon
script, the suit absorbs 80 % of damage, footsteps are per player model
(not per surface) and silent while ducked, and the match is plain
deathmatch or team deathmatch with respawning pickups. The install
mounts `hl2mp/` over Half-Life 2's VPKs; its 8 stock maps are BSP v20 with
both LDR and HDR lighting; its player models are HL2's citizens and
Combine with HL2:DM's own animation models swapped in through the search
path.

## Units and conventions

- Source units (inches), Z up, X forward, Y left; degrees; seconds.
- Tick 0.015 s (66.67 Hz) by default (section "Tick and network").
- "CS:S" values below are the ones in the named CS:S spec (measured or
  read there), not re-measured here.
- "SDK" marks a value read from the reference code; "install" a value read
  from the game's files; "predicted" a number worked out from them with
  the per-tick order of specs/cs_source/movement.md. Values marked SDK are
  the registered defaults; the real game can override them in a config it
  ships, so each is to be read back over RCON at M0 (Open questions 1).

## Part 1: the existing specs, one by one

Verdicts: **same** (holds for HL2:DM as written; later waves need not
repeat it), **same, re-derive** (the behaviour holds but a number in it
comes from gravity, the hull or a CS:S cvar and must be recomputed with
HL2:DM's values), **differs** (the list below it is what a later wave
writes), **CS:S only** (does not apply).

### specs/cs_source/

| Spec | Verdict | Owner wave |
|---|---|---|
| movement.md | differs | H1 |
| fall_damage.md | differs | H1 (fall), H10 (trigger_hurt: same) |
| weapons.md | differs (frame same, ballistics and data differ) | H3 |
| grenades.md | differs (HL2's frag is a physics object) | H4 |
| physics_props.md | same, re-derive (gravity); additions | H5 |
| ragdolls.md | same mechanism, other entity class and bodies | H7 |
| animation.md | same model mechanics; the player animation state differs | H7 |
| mdl_v48.md | same (versions 44–48 all occur; its §9 is needed) | – |
| view_models.md | differs (no mirroring; HL2MP bob row; FOV) | H7 |
| sounds.md | same system; footsteps, pain and death differ | H9 |
| impact_effects.md | same, small additions | H9 |
| tracers.md | same, with whizz sounds and other tracer kinds | H9 |
| shaders.md | same, plus three shaders it doesn't cover | H7 (Eyes, Teeth), H9 (UnlitTwoTexture) |
| shaders_two_texture_blend.md | same (no stock HL2:DM map uses it) | – |
| shaders_phong.md | same; HL2:DM is its main user (view models, Combine soldiers) | H7 |
| water.md | same | – |
| ropes.md | same | – |
| overlays_decals.md | same | – |
| sprites_dust.md | same | – |
| shadows_sky.md | same | – |
| nav.md | same format (v16); no `.nav` ships | – |
| spectating.md | differs (no freeze cam) | H6 |
| objectives.md | CS:S only | – |

### specs/source/

| Spec | Verdict |
|---|---|
| entity_io.md | same |
| triggers.md | same (trigger_hurt, trigger_push, trigger_teleport, trigger_multiple, trigger_once all occur in stock maps) |
| doors_buttons.md | same, except the `+use` paragraph's carry of physics props (H5: off by default) |
| breakables.md | same |
| prop_damage.md | same; adds `prop_physics_respawnable` (H10) |
| physics_brushes.md | same, re-derive gravity; `func_physbox_multiplayer` push group is CS:S's, unused in HL2:DM's stock maps |
| physics_constraints.md | same (`phys_ballsocket`, `phys_hinge`, `phys_lengthconstraint` occur); `phys_motor` not covered (H10) |
| fire.md | same mechanism; HL2:DM's 5 `env_fire` are on dm_steamlab, not bomb sites |
| particles_and_smoke.md | same |
| visual_entities.md | same |
| game_entities.md | same (`env_fade`, `env_explosion`, `env_shake` occur) |
| viewcontrol_and_templates.md | same (`point_template` occurs) |

There is no network spec in specs/; HL2:DM's network defaults are in Part
2 ("Tick and network").

### 1.1 Movement (specs/cs_source/movement.md) — differs

The controller (per-tick order, friction, acceleration, the air cap of 30,
slide move, stepping, ground probe, ducking curve, water and swim) is the
same shared code. What differs:

| Item | CS:S (movement.md) | HL2:DM |
|---|---|---|
| `sv_gravity` | 800 | **600** (SDK default for HL2 builds) |
| Jump impulse | 301.993 units/s (57-unit jump, measured) | **160 units/s** (SDK: a fixed constant while the movement optimisation switch is on, which is the default; it is not recomputed from `sv_gravity`) |
| Standing jump peak | 54.75 units | **20.145 units**, 35 ticks airborne on flat ground (predicted); a jump started ducked or mid-duck sets vz to 160 instead of adding it: peak 21.33 |
| Jump stamina | yes (1315.79 ms, scales jump and ground speed) | **none** |
| Bunny-hop speed cap | 286 (1.1 × 260) with `sv_enablebunnyhopping 0` | **none in the SDK**; a cvar `player_limit_jump_speed` (default 1) exists in the shipped server but the SDK never reads it (Open questions 3) |
| Jump forward boost | none | the SDK adds a forward boost only when the server has **one** player slot (single player); on a multiplayer server there is none (Open questions 3) |
| Toggle duck uncrouched by a jump | – | `xc_uncrouch_on_jump` 1 (controller toggle duck only) |
| Speeds | weapon `MaxPlayerSpeed` (250 knife), `+speed` walks at 0.52× | **no per-weapon speed**: `hl2_normspeed` 190 run, `+walk` (ALT) `hl2_walkspeed` 150, `+speed` (SHIFT) sprint `hl2_sprintspeed` 320; `sv_maxspeed` 320 is the hard cap |
| Sprint | – | needs the suit and at least 10 aux power to start (else the sound `HL2Player.SprintNoPower`); stops when aux power falls below 0.01; walking wins over sprint; holding duck cancels walking; sprint starts with `HL2Player.SprintStart` |
| Sprint drain | – | 25 aux power per second (HL2:DM's value; HL2 single player uses 12.5), not drained while horizontal velocity is 0 |
| Standing hull | (−16,−16,0)–(16,16,62) measured | **(−16,−16,0)–(16,16,72)**, eye 64 |
| Ducked hull | top 45, eye 47 (measured) | **top 36, eye 28**; dead view height 14 |
| Air duck lift | half the height difference (8.5) | to be measured (shared code moves the whole difference, 36); Open questions 4 |
| Duck input scale | 0.34 (measured) | 1/3 expected (shared value); to be measured |
| Duck times | 0.4 s down, 0.2 s up | same (shared) |
| `cl_forwardspeed`, `cl_sidespeed`, `cl_backspeed` | 400 | **450** |
| `sv_accelerate` | 5 (set by CS:S's skill1.cfg) | **10** (registered default; HL2:DM ships no config that changes it) |
| `sv_stopspeed` | 75 | **100** (same reason) |
| `sv_friction`, `sv_airaccelerate`, `sv_stepsize`, `sv_maxvelocity`, `sv_bounce`, `sv_backspeed`, water accelerate/friction | 4, 10, 18, 3500, 0, 0.6, 10/1 | same |
| Acceleration while water-jumping or dead | blocked | **not blocked**: HL2:DM's movement always allows acceleration (the shared "no acceleration while dead or water-jumping" rule is skipped) |
| Ladders | brush ladders (ladder contents), climb 200 | **`func_useableladder` entities** only: the shared contents-ladder code is replaced. Climb speed 200 along the ladder's axis; auto-mount within 64 units, from the air or pressing forward, within 15° and 32 units of the ladder; `+use` mounts; dismount points from `info_ladder_dismount`; jump-off is 200 along the view (reversed when moving back) plus 50 up unless looking down; falling off when more than 6 units from the ladder line; `sv_ladder_useonly` makes `+use` the only way on. 26 such ladders and 131 dismount points in the stock maps |
| Ladder-contents brushes | climbable | **not climbable** (dm_lockdown has 10 such brushes; Open questions 5) |
| Water | as spec | same movement; breath is different (1.4) |
| Combine ball as ground | – | a combine ball can never become the player's ground |
| Fall thresholds (shared code) | punch 350, safe 580, fatal 1024, min bounce 200, floating 200 | **punch 303, safe 526.5, fatal 922.5, min bounce 173, floating 173** (the HL2 set, scaled for gravity 600) |

### 1.2 Fall damage (specs/cs_source/fall_damage.md) — differs

- `mp_falldamage` 0 (default): any landing faster than **526.5 units/s**
  (about 231 units of fall at gravity 600) costs a **fixed 10** health.
- `mp_falldamage` 1: damage = (v − 526.5) × 100 / (922.5 − 526.5)
  = (v − 526.5) × 0.252525…; 100 at 922.5 units/s.
- Landing on a floating object first subtracts 173 from the landing
  speed; landing on something moving down adds its (negative) vertical
  speed, with a floor of 0.1.
- Armour does not absorb it (as CS:S); no attacker.
- The landing sound's volume tiers use the same thresholds (safe, half
  safe, min bounce) from the HL2 set above.
- `trigger_hurt`: same as CS:S's spec (D / 2 every half second).

### 1.3 Weapons (specs/cs_source/weapons.md) — differs

Same: one state machine per weapon driven once per command after
movement; next-primary, next-secondary and next-attack times; times from
the script, the code or a view-model sequence's length; shots traced from
the eye; hitboxes and hitgroups on the model; lag compensation around the
shot; client prediction with a seed from the command.

Different (H3 writes these out):

- **No penetration, no range falloff, no armour ratio per weapon.** A
  bullet stops at the first thing it hits.
- **Damage to players is the script's `damage` key**, a whole number per
  bullet (pellet); damage to anything that isn't a player comes from the
  ammo type and is a different number (Open questions 6).
- **Hitgroup scales** are cvars set by `cfg/skill.cfg`: `sk_player_head`
  3, `sk_player_chest` 1, `sk_player_stomach` 1, `sk_player_arm` 1,
  `sk_player_leg` 1 (the SDK's registered head default is 2; the shipped
  config sets 3). CS:S: head 4, stomach 1.25, legs 0.75.
- **Spread** is a fixed cone per weapon (degrees of half-angle as the
  usual Source cone vectors): SMG1 5°, AR2 3°, shotgun 10°, the
  machine-gun base default 3°. The pistol and the .357 fire with **no
  spread at all** even though the pistol has a 1°–6° accuracy ramp (its
  penalty is tracked but the shot ignores it) — a quirk to keep, Open
  questions 7.
- **Fire rates** (s between shots): pistol 0.5 held (each click may refire
  after 0.1 s, dry 0.2 s), .357 0.75, SMG1 0.075, AR2 0.1, shotgun one
  pump sequence, crowbar 0.4, stunstick 0.8, RPG 1.
- **Shotgun**: 7 pellets primary, 12 pellets for the double shot, 9 per
  pellet.
- **Melee**: crowbar 25 damage, reach 75, refire 0.4; stunstick 40, reach
  75, refire 0.8; a 16-unit hull sweep when the line misses (club damage).
- **Auto-reload** of an empty, holstered-long-enough weapon after
  `sk_auto_reload_time` 3 s.
- **Machine-gun view kick** (SMG1 max vertical 1° over 2 s of fire, AR2
  8° over 5 s), not CS:S's recoil.
- **Weapon slots** come from the script's `bucket` and `bucket_position`
  (six buckets), choice by `weight` on auto-switch; `cl_autowepswitch`.
- **Ammo** is a pool per type with a carry limit (Part 2, "Ammo types").
- **Pickups**: walking over a weapon gives it; placed weapons respawn
  after `sv_hl2mp_weapon_respawn_time` 20 s, items after
  `sv_hl2mp_item_respawn_time` 30 s; with `mp_weaponstay` 1 weapons
  without the "limit in world" flag stay and only owners of none can take
  one. Players and dropped weapons don't collide.
- **Spawn loadout**: the suit, crowbar (citizen models) or stunstick
  (Combine and metro police models), pistol, SMG1, one frag, gravity gun;
  ammo given: 255 pistol (clamped to the carry limit 150), 45 SMG1, 1
  grenade, 6 buckshot, 6 .357. The weapon in hand is `cl_defaultweapon`
  (default `weapon_physcannon`).
- **Scripts**: HL2:DM's weapon scripts are plain text (`.txt`) next to
  an encrypted copy (`.ctx`) of the same name; CS:S's are encrypted only.
  Chimera reads the `.txt` files and needs no key (Part 2, "Weapon
  scripts").

### 1.4 Grenades (specs/cs_source/grenades.md) — differs

CS:S's grenade is a custom box with its own bounce rule. HL2:DM's frag
(H4 writes it):

- a **physics object** (VPhysics) with a 4-unit radius box for traces;
  rolls and bounces by the physics engine, so none of CS:S's 45 % bounce
  rule applies;
- fuse 2.5 s from the throw; damage 150 (script), radius 250; warning
  blips every 1 s, faster (0.3 s) in the last 1.5 s;
- primary (pull back, release primary): throw at 1200 units/s along the
  view plus the thrower's velocity, from 18 forward and 8 right of the
  eye, random spin; secondary released without duck held: lob at 350
  along the view plus 50 up plus the thrower's velocity, from 18 forward,
  8 right and 8 down; secondary released with duck held: roll at 700
  along the ground direction from the feet; the projectile leaves on the
  view model's throw event, not on the release itself (H4 gives the
  times); the next pull-back 0.5 s later;
- a primed grenade is dropped live when its holder dies.

### 1.5 Physics props (specs/cs_source/physics_props.md) — same, re-derive

- Physics gravity follows `sv_gravity`: **600**, not 800. Anything the
  spec derived from 800 is recomputed.
- Linear and angular speed caps, `.phy` decoding, mass and surface data,
  damage impulses, push-away and the player shadow: same.
- Players take physics impact damage with an energy scale of **4**
  (HL2:DM's player value). H5 states the impact damage table and how a
  thrower is credited.
- New in HL2:DM: `prop_physics_respawnable` (329 in the stock maps, H10),
  the gravity gun (H5), and `+use` carrying, which HL2:DM **turns off by
  default** (`hl2mp_allow_pickup` 0 in the shipped server).
- Bullet impulse per ammo type: mass in grains and speed in ft/s turned
  into an impulse and exaggerated 3.5× (H3 lists them; same helper as
  CS:S's).

### 1.6 Ragdolls (specs/cs_source/ragdolls.md) — same mechanism

The server spawns a small networked record and each client builds its
own ragdoll, as CS:S. The class is `hl2mp_ragdoll`. Bodies, joints and
limits come from each HL2 player model's `.phy` (H7). The dead player's
view (1.12) follows this ragdoll.

### 1.7 Animation and models (animation.md, mdl_v48.md) — same mechanics, new state

- Model loading, include models, bone merging by name, sequence blending,
  events and attachments: same.
- MDL versions in Half-Life 2's misc VPK: 2116 files of version 44, 6 of
  45, 9 of 46, 2 of 47, 68 of 48; `hl2mp/`'s own 25 models are all 44.
  The player models are 44, 45 and 46 (Part 2, "Player models"):
  mdl_v48.md's §9 (versions 45–47) is needed, not optional.
- **The animation comes from the search path, not the player model.**
  Each HL2 player model includes HL2's shared animation models
  (citizens: 4 includes, Combine: 1, metro police: 2). `hl2mp/` ships its
  own files at those same paths, which win because `hl2mp/` is searched
  first; they hold no animation themselves and include
  `models/player/male_anims.mdl` or `models/player/female_anims.mdl`
  (56 bones, 105 sequences, 8 pose parameters each). A loader that mounts
  `hl2/` before `hl2mp/` gets single-player NPC animation instead.
- Include paths in these files use mixed case and backslashes
  (`models/Police_animations.mdl`, `models/player\male_anims.mdl`):
  lookups must ignore case and treat `\` as `/`.
- The player animation state is HL2:DM's activity-based one (run, walk,
  crouch, jump, swim; aim and fire, reload and jump gestures; per held
  weapon kind from the script's `anim_prefix`), not CS:S's: H7.
- Citizens have flex controllers (54) and flex descriptors (71); the
  metro police 10 controllers and no descriptors; Combine none.

### 1.8 View models (view_models.md) — differs

- **No mirroring**: HL2's view models are built right-handed and the
  client has no `cl_righthand` (CS:S only). Draw as stored.
- `viewmodel_fov` 54 (same). The world FOV is **75** (`default_fov` 75;
  a client's `fov_desired` is clamped to 75–90 by the server) against
  CS:S's 90.
- Bob: the HL2MP alternative already in view_models.md (its B13 row:
  forward 0.1 per vertical bob, `cl_bobcycle` 0.8) is HL2:DM's; H7
  confirms the rest of the row.
- View-model sequences are chosen by activity (as CS:S); the per-weapon
  rules (shotgun shell reload, AR2 charge, S.L.A.M. modes) are H3/H4/H7's.

### 1.9 Sounds (sounds.md) — same system; footsteps differ

- Sound scripts, channels, levels, soundscapes and ambient_generic: same.
  The sound-script manifest is Half-Life 2's loose
  `hl2/scripts/game_sounds_manifest.txt` (63 files listed), with
  `hl2mp/`'s `scripts/game_sounds_player.txt` and
  `scripts/game_sounds_weapons.txt` and its own soundscape manifest and
  three soundscape files in front of HL2's by search order.
- **Footsteps ignore the surface.** Each step plays the player model
  type's entry: `NPC_Citizen.RunFootstepLeft` / `Right`,
  `NPC_CombineS.RunFootstep…`, `NPC_MetroPolice.RunFootstep…`,
  alternating sides, on the body channel at the step's volume.
- **No footsteps while ducked** (fully ducked, the ducked flag set), and
  so no jump or landing sound from a ducked player.
- Step timing and volume (the shared walk/run speed bands 90 and 220,
  ladder 0.5 volume, every-fourth-step skipping at waist depth) are the
  shared rules, not CS:S's knee-depth variant.
- `sv_footsteps` / `mp_footsteps` 1 gate them; on a multiplayer server
  the server sends steps only to players outside the walker's PVS, the
  others make them themselves.
- Death sound: the model type's `.Die` entry (`NPC_CombineS.Die`,
  `NPC_MetroPolice.Die`; citizens' to be found, H9). Flashlight:
  `HL2Player.FlashlightOn` / `Off`.

### 1.10 Impact effects and tracers (impact_effects.md, tracers.md) — same, additions

- `cl_new_impact_effects` is 0 and HL2's particle manifest doesn't load
  `impact_fx.pcf` either: the old-style surface effects of
  impact_effects.md apply unchanged. The manifest loads `blood_impact.pcf`,
  `water_impact.pcf`, `fire_01.pcf`, `burning_fx.pcf`,
  `combineball.pcf`, `rocket_fx.pcf` (plus `error.pcf` and two NPC files).
- Tracers: the machine-gun weapons (SMG1, AR2) draw one every **2**
  bullets (the HL2 base value tracers.md already names); others the
  generic 4. The AR2 uses its own tracer kind (`AR2Tracer`) and impact
  effect (H9).
- **Whizz-by sounds exist**: the pistol, SMG1, AR2 and .357 ammo types use
  "line and whizz" tracers, and `Bullets.DefaultNearmiss` is defined in
  HL2's `game_sounds_weapons.txt` (CS:S lacks the entry: tracers.md Q4).

### 1.11 Shaders (shaders.md) — same, plus three

World materials used by the 8 stock maps (resolved through each map's
pak and the VPKs, patch materials followed): LightmappedGeneric 1316,
WorldVertexTransition 18, UnlitGeneric 5, VertexLitGeneric 5, Water 4,
UnlitTwoTexture 4 (dm_underpass), DecalModulate 2, Cable 1. No material
was missing. Not covered by shaders.md: **UnlitTwoTexture**; on player
models, **Eyes** (2 materials per citizen) and **Teeth** (1). No player
model uses EyeRefract. Every stock map carries both LDR and HDR lighting
lumps (equal sizes), so the `mat_hdr_level 0` path of shaders.md works
as is; several skies are `_hdr` names (Open questions 9).

### 1.12 Spectating (spectating.md) — differs

- No freeze cam. While dead (and not yet an observer) the camera sits
  96 units behind the ragdoll's origin raised by the dead view height 14,
  along the player's view direction, pulled in by a 6-unit box trace
  against the world.
- After 3 s dead the player becomes an observer in its last observer
  mode (`cl_spec_mode`, chase or roaming only).
- Respawn: any button, or forced 5 s after death with `mp_forcerespawn`
  1 (H6 settles how the observer state interacts with this).

### 1.13 Everything else listed as "same"

- **nav.md**: the shipped server has bots (NextBot, `hl2mp_bot_*`
  cvars, 109 such names in the 32-bit and 121 in the 64-bit server
  library) which use `.nav` version 16, but no `.nav` file ships with the
  stock maps (they are generated in game). Bots are a non-goal of the
  plan; the format is already covered.
- **water.md**: Water shader on dm_runoff (2 materials) and dm_underpass
  (2); water brushes in those two maps only.
- **ropes.md**: 395 `keyframe_rope` and 131 `move_rope` in the stock maps.
- **sprites_dust.md**: 264 `env_sprite`, 8 `func_dustmotes`; HL2:DM adds
  `func_dustcloud` (halls3, 5) and `env_lightglow` (34), which it doesn't
  cover (H10).
- **overlays_decals.md**: 1236 `infodecal`, overlays in every map.
- **shadows_sky.md**: `shadow_control` in 6 maps, `sky_camera` in 6.

## Part 2: install facts (M0)

### Builds

| What | Value |
|---|---|
| Game | app 320 "Half-Life 2: Deathmatch", build 24600434 |
| Dedicated server | app 232370, build 24600426 |
| `hl2mp/steam.inf` (both) | PatchVersion, ClientVersion, ServerVersion 10889068; ProductName hl2mp; appID 320; ServerAppID 232370 |
| Client binaries | 64-bit Linux launcher `hl2mp_linux64`; game libraries in `hl2mp/bin/linux64/` (and 32-bit ones in `hl2mp/bin/`) |
| CS:S for comparison | PatchVersion 11003710 (movement.md) |

### gameinfo.txt

`hl2mp/gameinfo.txt` is identical in the game and the server. Keys: game
"Half-Life 2 DM", type multiplayer_only, `nomodels` 0, `nohimodel` 1,
`nocrosshair` 1, `supportsvr` 1, hidden maps `test_speakers` and
`test_hardware`, SteamAppId 320.

Search paths, in order (first wins). `|all_source_engine_paths|` is the
install root; an entry whose file is missing is skipped.

| # | Path ids | Path | Present on this install |
|---|---|---|---|
| 1 | game, mod | `hl2mp/custom/*` (each VPK and folder, alphabetical) | folder exists, only a readme |
| 2 | game, mod | `hl2mp/hl2mp_english.vpk` | **absent** |
| 3 | game, mod | `hl2mp/hl2mp_pak.vpk` | yes: 1 archive, 11 325 904 bytes with the directory, 344 files |
| 4 | game | `hl2_complete/hl2_complete_textures.vpk` | absent |
| 5 | game | `hl2_complete/hl2_complete_sound_vo_english.vpk` | absent |
| 6 | game | `hl2_complete/hl2_complete_sound_misc.vpk` | absent |
| 7 | game | `hl2_complete/hl2_complete_misc.vpk` | yes: 1 archive, 50 566 bytes, 4 files (menu art only) |
| 8 | game | `hl2/hl2_textures.vpk` | yes: 12 archives, 1 286 150 906 bytes |
| 9 | game | `hl2/hl2_sound_vo_english.vpk` | yes: 5 archives, 599 168 821 bytes |
| 10 | game | `hl2/hl2_sound_misc.vpk` | yes: 3 archives, 399 776 559 bytes |
| 11 | game | `hl2/hl2_misc.vpk` | yes: 4 archives, 483 776 838 bytes |
| 12 | platform | `platform/platform_misc.vpk` | yes: 1 archive, 4 821 314 bytes |
| 13 | mod, mod_write, default_write_path | `hl2mp/` (the gameinfo folder) | yes |
| 14 | game, game_write | `hl2mp/` | yes (same folder, game id) |
| 15 | gamebin | `hl2mp/bin` | binaries only |
| 16 | game | `hl2_complete/` (loose) | yes: `resource/` (the GorDIN TTFs, a scheme, HUD layout and animation files of HL2's anniversary update) |
| 17 | game | `hl2/` (loose) | yes: `scripts/` (133 files: sound manifest, `propdata.txt`, `surfaceproperties*.txt`, soundscapes, HL2's own weapon scripts), `resource/`, `media/` |
| 18 | platform | `platform/` (loose) | yes |
| 19 | game, download | `hl2mp/download` | yes, searched last |

Consequences for the mount (S1):

- HL2:DM's VPKs are searched **before any loose folder**, including
  `hl2mp/` itself. CS:S's mount has no second game's VPK layer like
  `hl2_complete/`.
- Shared scripts HL2:DM doesn't override (sound manifest, prop data,
  surface properties) come from `hl2/scripts/` **loose**, not from a
  VPK.
- `hl2/scripts/` also holds HL2's single-player weapon scripts at the
  same names as HL2:DM's; HL2:DM's VPK copies win by order and must.
- A map's own pak (BSP lump 40) is mounted in front of everything while
  the map is loaded (as CS:S).

### Stock maps

All in `hl2mp/maps/` (loose), byte-identical in the game and the server;
every one is VBSP **version 20** with no LZMA-compressed lumps; each has a
pak lump and both LDR and HDR lighting.

| Map | Map revision | Bytes | Pak lump bytes | Entities | `info_player_deathmatch` | Team spawns | Sky |
|---|---|---|---|---|---|---|---|
| dm_lockdown | 10860 | 32 482 508 | 16 171 387 | 648 | 33 | – | sky_day02_09 |
| dm_overwatch | 3247 | 12 946 224 | 7 218 844 | 364 | 17 | – | sky_day03_04_hdr |
| dm_powerhouse | 2263 | 25 089 532 | 7 405 122 | 1400 | 18 | – | sky_day03_06 |
| dm_resistance | 1259 | 17 230 780 | 7 769 198 | 630 | 20 | – | sky_day01_09_hdr |
| dm_runoff | 4324 | 21 875 152 | 7 783 322 | 608 | 9 | 8 combine, 6 rebel | sky_day02_10_hdr |
| dm_steamlab | 2539 | 26 203 912 | 9 863 846 | 1090 | 24 | – | sky_day01_07_hdr |
| dm_underpass | 11202 | 27 209 640 | 15 208 396 | 878 | 17 | – | sky_day01_05_hdr |
| halls3 | 715 | 9 680 896 | 5 068 700 | 337 | 10 | – | sky_day02_07_hdr |

- `cfg/mapcycle_default.txt` lists the seven dm_ maps; **halls3 is not in
  the cycle** (it ships with `maps/halls3.txt` in the pak).
- Spawn choice: team spawns (`info_player_combine`, `info_player_rebel`)
  in team play when the map has them, else `info_player_deathmatch`, else
  `info_player_start` (H6).
- Placed pickups per map are in H10's brief; no stock map places
  `weapon_pistol` or `weapon_smg1` (the loadout gives them) and only
  dm_powerhouse places a `weapon_crowbar`.

### Tick and network

- Default tick interval **0.015 s** (66.67 Hz), the game library's default
  as in CS:S. Unlike CS:S, HL2:DM's server **accepts `-tickrate N`** (any
  N > 10) on its command line.
- Update and command rates, `rate`, `sv_maxupdaterate` and friends are
  engine settings, not in the game code: to read over RCON at M0.
- Client interpolation defaults are the shared client's: `cl_interp`
  0.1, `cl_interp_ratio` 2 (same as CS:S).
- The server sends the local player a high-resolution origin and other
  players a low-resolution one, and never networks the player's max
  speed (each side computes it from the buttons, so sprint and walk are
  predicted).

### Default cvars that differ from CS:S

Registered defaults (SDK), then what the shipped configs set. CS:S column
from movement.md and CS:S's code where noted.

| Cvar | HL2:DM | CS:S | Note |
|---|---|---|---|
| `sv_gravity` | 600 | 800 | |
| `sv_accelerate` | 10 | 5 | CS:S's comes from its skill1.cfg |
| `sv_stopspeed` | 100 | 75 | same |
| `cl_forwardspeed`, `cl_sidespeed`, `cl_backspeed` | 450 | 400 | |
| `hl2_walkspeed`, `hl2_normspeed`, `hl2_sprintspeed` | 150, 190, 320 | – | HL2 only |
| `default_fov` / `fov_desired` range | 75 / 75–90 | 90 | |
| `mp_flashlight` | 0 (SDK) | 1 | Open questions 1 |
| `mp_falldamage` | 0 (fixed 10) | CS:S formula | |
| `mp_forcerespawn` | 1, but the game's `cfg/valve.rc` sets **0** | – | Open questions 1 |
| `mp_teamplay` | 0 | – | |
| `mp_fraglimit`, `mp_timelimit` | 0, 0 | | |
| `mp_weaponstay` | 0 | – | |
| `mp_footsteps`, `sv_footsteps` | 1, 1 | | |
| `mp_chattime` | 10 (1–120) | | |
| `mp_friendlyfire` | 0 | | |
| `sv_hl2mp_weapon_respawn_time` | 20 | – | |
| `sv_hl2mp_item_respawn_time` | 30 | – | |
| `hl2mp_allow_pickup` | 0 | – | `+use` carry of props off |
| `hl2mp_spawn_frag_fallback_radius` | 48 | – | spawn telefrag radius when every spawn is blocked |
| `player_limit_jump_speed` | 1 | – | unused in the SDK |
| `cl_playermodel` | "none" | – | |
| `cl_defaultweapon` | `weapon_physcannon` | – | |
| `sk_player_head` | 3 (skill.cfg; SDK 2) | – | |
| `sk_player_chest`, `_stomach`, `_arm`, `_leg` | 1 | – | |
| `sk_battery` | 15 | – | skill.cfg |
| `sk_healthkit`, `sk_healthvial` | 25, 10 | – | skill.cfg |
| `sk_suitcharger`, `sk_healthcharger` | 75, 50 | – | skill.cfg (also `_citadel` 200 values, unused in DM) |
| `sk_auto_reload_time` | 3 | – | |
| `sk_plr_num_shotgun_pellets` | 7 | – | |
| `physcannon_maxmass`, `_tracelength`, `_minforce`, `_maxforce`, `_pullforce`, `_cone`, `_chargetime`, `player_throwforce` | 250, 250, 700, 1500, 4000, 0.97, 2, 1000 | – | H5 |
| `sk_weapon_ar2_alt_fire_radius`, `_duration`, `_mass` | 10, 4, 150 | – | H4 |
| `xc_uncrouch_on_jump` | 1 | – | |

`cfg/config_default.cfg` binds (first run only): W/S/A/D move, SPACE
jump, CTRL duck, E use, R reload, **ALT `+walk`**, **SHIFT `+speed`
(sprint)**, F `impulse 100` (flashlight), C `impulse 50` (suit zoom),
Z `+zoom`, G `phys_swap` (to and from the gravity gun), Q `lastinv`, 1–7
`slot1`–`slot7`, wheel `invprev`/`invnext`, TAB scores, Y/U chat, K voice.
CS:S binds SHIFT to walking; HL2:DM to sprinting.

### Player models

The selectable models are the 19 with an icon in `hl2mp/`'s
`materials/vgui/playermodels/`; all live in Half-Life 2's misc VPK.

| Model | Team kind | Version | Bones | Includes | Flex ctl / desc |
|---|---|---|---|---|---|
| `models/humans/group03/male_01.mdl` … `male_09.mdl` (9) | Rebels (citizen) | 44 (male_09: 45) | 68 | 4 | 54 / 71 |
| `models/humans/group03/female_01.mdl` … `_04`, `_06`, `_07` (6) | Rebels (citizen) | 44 | 72 | 4 | 54 / 71 |
| `models/combine_soldier.mdl` | Combine | 46 | 44 | 1 | 0 / 0 |
| `models/combine_soldier_prisonguard.mdl` | Combine | 44 | 44 | 1 | 0 / 0 |
| `models/combine_super_soldier.mdl` | Combine | 46 | 44 | 1 | 0 / 0 |
| `models/police.mdl` | Combine (metro police) | 44 | 57 | 2 | 10 / 0 |

- In team play a player on the Combine team with a citizen model is
  given the next Combine model in turn, and a Rebel without a citizen
  model the next citizen model (a round-robin shared by all players,
  not random); an invalid or missing
  `cl_playermodel` becomes `models/combine_soldier.mdl`. Model changes
  are limited to one per 5 s, team changes too (H6).
- HL2:DM's animation models in `hl2mp/`: `models/player/male_anims.mdl`
  and `female_anims.mdl` (56 bones, 105 sequences, 8 pose parameters),
  and the pass-through files at HL2's include paths
  (`models/humans/male_shared.mdl`, `female_shared.mdl`,
  `male_gestures`, `male_postures`, `male_ss`, the female ones,
  `models/combine_soldier_anims.mdl`, `models/police_animations.mdl`,
  `models/police_ss.mdl`) that redirect to them (1.7).
- Player-model materials use VertexLitGeneric, and on citizens Eyes and
  Teeth.

### Weapon scripts

`hl2mp/hl2mp_pak.vpk` holds `scripts/weapon_<name>.txt` (plain) and
`.ctx` (encrypted) for: 357, ar2, crossbow, crowbar, frag, physcannon,
pistol, rpg, shotgun, slam, smg1, stunstick; plus a plain
`weapon_cubemap.txt` (a developer tool, not a weapon to implement).

Keys and values (our summary of the plain files; `-1` means "no
magazine"; an absent key is the engine default):

| Weapon | Bucket, position | Clip | Default clip | Primary ammo | Secondary ammo | Weight | Item flags | `damage` | `anim_prefix` |
|---|---|---|---|---|---|---|---|---|---|
| `weapon_crowbar` | 0, 0 | −1 | – | None | None | 0 | 0 | – (code: 25) | crowbar |
| `weapon_stunstick` | 0, 1 | −1 | – | None | None | 0 | 0 | – (code: 40) | stunbaton |
| `weapon_physcannon` | 0, 2 | 0 (clip2 0) | 0 (0) | None | None | 0 | 2 | – | gauss |
| `weapon_pistol` | 1, 0 | 18 | – | Pistol | None | 2 | 0 | 8 | pistol |
| `weapon_357` | 1, 1 | 6 | 6 | 357 | None | 7 | 0 | 75 | python |
| `weapon_smg1` | 2, 0 | 45 (clip2 −1) | 45 (−1) | SMG1 | SMG1_Grenade | 3 | 0 | 5 | smg2 |
| `weapon_ar2` | 2, 1 | 30 (clip2 −1) | 60 (−1) | AR2 | AR2AltFire | 5 | 0 | 11 | ar2 |
| `weapon_shotgun` | 3, 0 | 6 | – | Buckshot | None | 4 | 0 | 9 | shotgun |
| `weapon_crossbow` | 3, 1 | 1 | 5 | XBowBolt | None | 6 | 0 | 100 | bow |
| `weapon_frag` | 4, 0 | −1 (−1) | 1 (−1) | grenade | None | 1 | 18 | 150 | Grenade |
| `weapon_rpg` | 4, 1 | −1 (−1) | 3 (−1) | rpg_round | None | 0 | 0 | 150 | missile launcher |
| `weapon_slam` | 4, 2 | 3 | – (clip2 default 3) | None | slam | 1 | 2 | – | slam |

Other keys: `weapon_physcannon` has `autoswitchto` 0 and `autoswitchfrom`
0; `weapon_rpg` has `autoswitchfrom` 1. Model keys (`viewmodel`,
`playermodel`):

| Weapon | View model | World model | In `hl2mp/`'s VPK |
|---|---|---|---|
| crowbar | `v_crowbar.mdl` | `w_crowbar.mdl` | world |
| stunstick | `v_stunstick.mdl` | `w_stunbaton.mdl` | both |
| physcannon | `v_physcannon.mdl` | `w_Physics.mdl` (mixed case) | both |
| pistol | `v_pistol.mdl` | `w_pistol.mdl` | neither |
| 357 | `v_357.mdl` | `w_357.mdl` | world |
| smg1 | `v_smg1.mdl` | `w_smg1.mdl` | neither |
| ar2 | `v_irifle.mdl` | `w_irifle.mdl` | neither |
| shotgun | `v_shotgun.mdl` | `w_shotgun.mdl` | neither |
| crossbow | `v_crossbow.mdl` | `w_crossbow.mdl` | world |
| frag | `v_grenade.mdl` | `w_grenade.mdl` | world |
| rpg | `v_rpg.mdl` | `w_rocket_launcher.mdl` | world |
| slam | `v_slam.mdl` | `w_slam.mdl` | both |

(all under `models/weapons/`; "neither" means Half-Life 2's VPK copy is
used.)

Sound keys (`SoundData`; entry names are data):

| Weapon | Entries |
|---|---|
| crowbar | single_shot `Weapon_Crowbar.Single`, melee_hit `Weapon_Crowbar.Melee_Hit`, melee_hit_world `Weapon_Crowbar.Melee_HitWorld` |
| stunstick | single_shot `Weapon_StunStick.Swing`, melee_miss `…Melee_Miss`, melee_hit `…Melee_Hit`, melee_hit_world `…Melee_HitWorld` |
| physcannon | single_shot `Weapon_PhysCannon.Launch`, reload `.Charge`, empty `.DryFire`, special1 `.Pickup`, special2 `.OpenClaws`, melee_hit `.CloseClaws`, melee_miss `.Drop`, special3 `.TooHeavy` |
| pistol | reload, empty, single_shot, special1, special2, burst: `Weapon_Pistol.*` (plus NPC entries) |
| 357 | empty `Weapon_Pistol.Empty`, single_shot `Weapon_357.Single` |
| smg1 | reload, empty, single_shot, special1, special2, double_shot `Weapon_SMG1.Double`, burst: `Weapon_SMG1.*` |
| ar2 | single_shot `Weapon_AR2.Single`, reload `Weapon_AR2.Reload`, empty `Weapon_IRifle.Empty`, double_shot `Weapon_IRifle.Single`, special1 `Weapon_CombineGuard.Special1` |
| shotgun | empty, reload, special1, single_shot, double_shot: `Weapon_Shotgun.*` |
| crossbow | single_shot `Weapon_Crossbow.Single`, reload `.Reload`, special1 `.BoltElectrify`, special2 `.BoltFly` |
| frag | single_shot and double_shot `common/null.wav`, special1 `WeaponFrag.Roll` |
| rpg | single_shot `Weapon_RPG.Single`, special1 `.LaserOn`, special2 `.LaserOff`, empty `Weapon_SMG1.Empty` |
| slam | none (the code names its sounds) |

HUD icons (`TextureData`): each weapon names a character of a font, not a
sprite: `weapon`, `weapon_s` (selected) and `weapon_small` from the
`WeaponIcons`, `WeaponIconsSelected` and `WeaponIconsSmall` fonts;
`ammo` and `ammo2` from `WeaponIconsSmall` or `WeaponIconsSmallSecondary`;
`crosshair` from the `Crosshairs` font (character `Q` for all but the
crowbar and S.L.A.M., which use the `sprites/crosshairs` sprite);
`autoaim` from `sprites/crosshairs` (0, 48, 24 × 24). Characters:

| Weapon | weapon / selected / small | ammo | ammo2 |
|---|---|---|---|
| smg1 | a | r | t (secondary font) |
| shotgun | b | s | – |
| crowbar | c | c (WeaponIcons) | – |
| pistol | d | p | – |
| 357 | e | q | – |
| crossbow | g | w (secondary font) | – |
| rpg | i | x (secondary font) | – |
| frag | k | v | – |
| ar2 | l | u | z |
| physcannon | m | – | – |
| stunstick | n | n (WeaponIcons) | sprite `sprites/640hud7` |
| slam | o | o (WeaponIcons) | – |

`crossbow` also has `zoom` and `zoom_autoaim` entries.

### Ammo types

From the game code's ammo table (names are the scripts' ammo values;
damage per hit against players is the weapon script's, not this table's):

| Ammo | Damage kind | Tracer | Carry limit |
|---|---|---|---|
| AR2 | bullet | line and whizz | 60 |
| AR2AltFire | dissolve | none | 3 |
| Pistol | bullet | line and whizz | 150 |
| SMG1 | bullet | line and whizz | 225 |
| 357 | bullet | line and whizz | 12 |
| XBowBolt | bullet | line | 10 |
| Buckshot | bullet, buckshot | line | 30 |
| RPG_Round | burn | none | 3 |
| SMG1_Grenade | burn | none | 3 |
| Grenade | burn | none | 5 |
| slam | burn | none | 5 |

### HUD resources

`hl2mp/`'s VPK has its own `resource/clientscheme.res`,
`scripts/hudlayout.res` and `scripts/hudanimations.txt`. The scheme's
fonts are mostly **GorDIN** (59 font entries; the TTFs are in
`hl2_complete/resource/`, found through search path 16), then HalfLife2
(16, the weapon and number glyphs), HL2MP, HL2cross; custom font files:
`resource/HALFLIFE2.ttf`, `resource/HL2crosshairs.ttf`,
`resource/HL2MP.ttf` and Linux fallbacks. `hl2mp/resource/` (loose)
holds `halflife2.ttf`, `hl2mp.ttf` and the `hl2mp_<language>.txt`
localization files. H8 builds on this.

### The dedicated server and SourceMod

- The server install (app 232370) has the same `hl2mp/` pak, maps and
  gameinfo as the game, and Half-Life 2's complete misc VPK (models), but
  only the **directory files** of the textures and sound VPKs (no
  archives): maps and models are readable from it, textures and sounds
  are not.
- Its launcher `srcds_run` starts **`srcds_linux`, a 32-bit program**,
  with 32-bit engine and game libraries in `bin/` and `hl2mp/bin/`. It
  also ships 64-bit libraries (`bin/linux64/`,
  `hl2mp/bin/linux64/server_srv.so`) but **no 64-bit launcher**; the
  only `srcds_linux64` on this box is in the game's install.
- So the probe can run exactly as CS:S's does: the 32-bit server with
  Metamod:Source and SourceMod 1.12's 32-bit builds. The SourceMod 1.12
  package already on this box (in the CS:S probe server) contains
  HL2:DM builds of both Metamod and SourceMod for 32-bit **and** 64-bit
  (`*.2.hl2dm.so` in `bin/` and `bin/x64/`, `bin/linux64/`), and its
  SDK-tools game data for `hl2mp` has 64-bit offsets (19 entries).
  Whether those offsets match build 24600426 can't be known from files:
  the first probe start at M0 says (Open questions 2).
- HL2:DM has no CS-style bots for the probe's replay bot, but it now
  ships NextBot bots (`hl2mp_bot_add`, `hl2mp_bot_quota`; `cfg/
  autumnbot.cfg` adds one gravity-gun-only bot); the probe should use a
  SourceMod fake client instead so no AI moves it.

## Behavior

Part 1 is the behaviour; this section only names the order in which a
later wave should treat the rows above.

- H1 (movement) takes 1.1 and 1.2 and the movement rows of the cvar table.
- H2 (suit) takes the aux power numbers (sprint in 1.1, the rest here) and the
  armour rule: the suit takes 80 % of the damage and the player 20 %;
  each point of damage absorbed costs one armour point (two against
  blasts), at least one point a hit; if the armour runs out the rest goes
  to health; fall, drowning, poison and radiation bypass armour. Aux power
  (0–100): recharge 12.5 per second once every device has been off for
  0.5 s; sprint drains 25/s (1.1); breathing under water drains 6.7/s
  and **while any aux power is left the player doesn't use up air**; the
  air lasts 7 s after that, then drowning does 10 a second (HL2's values;
  CS:S measured 12 s and 3, 4, 5…). HL2:DM's flashlight is its own: it
  needs `mp_flashlight` above 0 and adds no drain (Open questions 8).
- H3 takes 1.3 and the weapon-script and ammo tables.
- H6 takes 1.12, the spawn and respawn rows and the rules cvars.

## Per-tick order

Unchanged from specs/cs_source/movement.md and weapons.md, with one
addition at the start of the movement step: the walk/run/sprint speed is
chosen from the buttons (1.1) before the movement parameters are checked,
and aux power is charged or drained for the tick when the movement's
timers are reduced.

## Edge cases

- `hl2mp_english.vpk` and three `hl2_complete` VPKs are named but absent;
  a mount must skip missing entries without error.
- Include-model paths with backslashes and mixed case (1.7).
- World model `models/weapons/w_Physics.mdl` (mixed case) against the file
  `w_physics.mdl`.
- dm_lockdown's ladder-contents brushes do nothing (1.1).
- halls3 is a stock map outside the map cycle.

## Quirks

- The pistol and .357 shoot perfectly straight (1.3). Keep.
- Damage against players and against props differ for the same bullet
  (1.3). Keep.
- Sprinting in place costs no aux power (1.1). Keep.
- A ducked player is silent (1.9). Keep.

## Test cases

Implementer checks (no game needed):

| # | Setup | Input | After | Expected |
|---|---|---|---|---|
| 1 | flat ground, standing, gravity 600, tick 0.015 | jump once | until landing | vz at the end of the jump tick 146.5 (160 minus three half-gravity steps of 4.5, as movement.md's 283.99 for CS:S); peak 20.145 units; back on the ground 35 ticks after the jump |
| 2 | as 1, ducked flag set | jump | – | vz set to 160 (not added): 151.0 at the end of the tick, peak 21.33 |
| 3 | standing, aux 100 | hold forward + `+speed` | 1 tick | max speed 320; aux 99.625 after the tick (25 × 0.015) |
| 4 | aux 9 | press `+speed` | – | no sprint, `HL2Player.SprintNoPower` once; speed 190 |
| 5 | sprinting, horizontal velocity 0 | hold `+speed` 1 s | – | aux unchanged |
| 6 | holding `+walk` and `+speed` | forward | – | sprint (walk only applies when not sprinting) — speed 320 |
| 7 | holding `+walk` and duck | forward | – | not walking: run speed 190 before the duck scale |
| 8 | landing at 600 units/s, `mp_falldamage` 0, armour 50 | – | – | health −10, armour unchanged |
| 9 | landing at 800, `mp_falldamage` 1 | – | – | damage (800 − 526.5) × 0.252525 = 69.07 |
| 10 | armour 100, health 100 | 30 bullet damage | – | health takes 6, armour loses 24 |
| 11 | armour 100 | 30 blast damage | – | health takes 6, armour loses 48 |
| 12 | armour 10 | 30 bullet damage | – | armour 0, health takes 30 − 10 = 20 |
| 13 | pistol at the head of an unarmoured player | one shot | – | 24 damage (8 × 3) |
| 14 | the mount with the stock search path | resolve `models/humans/male_shared.mdl` | – | `hl2mp/`'s copy (no animation, includes `models/player/male_anims.mdl`), not `hl2/`'s |
| 15 | resolve `scripts/weapon_pistol.txt` | – | – | `hl2mp/`'s (bucket 1, clip 18, damage 8), not `hl2/scripts/`'s loose single-player file |
| 16 | resolve `scripts/game_sounds_manifest.txt` | – | – | `hl2/scripts/` loose (no copy in any VPK on the path) |
| 17 | ducked player walking | 2 s | – | no footstep sound |
| 18 | citizen model on metal | walk | – | `NPC_Citizen.RunFootstepLeft`/`Right` alternating, never a metal step entry |
| 19 | spawn, DM, citizen model, default cvars | – | – | crowbar, pistol, SMG1, frag, gravity gun in hand; pistol 150, SMG1 45, grenade 1, buckshot 6, .357 6 |
| 20 | spawn, team play, Combine team with `cl_playermodel` a citizen | – | – | a Combine model; stunstick instead of crowbar |

Checks against this install or the probe (build 24600434 / 24600426):

| # | How | Expected |
|---|---|---|
| 21 | read each stock BSP header | version 20, map revisions as in the table |
| 22 | list `hl2mp/hl2mp_pak_dir.vpk` | 344 entries, 13 `scripts/weapon_*.txt`, 12 `.ctx` |
| 23 | count MDL versions in `hl2/hl2_misc_dir.vpk` | 44: 2116, 45: 6, 46: 9, 47: 2, 48: 68 |
| 24 | probe: standing jump on flat ground | peak about 20.1 (confirms gravity 600 and impulse 160) |
| 25 | probe RCON at startup | the values in "Default cvars" (Open questions 1) |
| 26 | probe: repeated jumps holding forward, then holding back | speed after many hops: no cap expected, no forward boost expected (Open questions 3) |

## Open questions

1. **Shipped values vs registered defaults.** Read over RCON on the
   probe server and on a listen server: `sv_gravity`, `sv_accelerate`,
   `sv_stopspeed`, `sv_friction`, `sv_airaccelerate`, `mp_flashlight`,
   `mp_forcerespawn` (the game's `valve.rc` sets 0; does a dedicated
   server run `valve.rc`?), `mp_teamplay`, `sv_maxupdaterate`,
   `sv_minupdaterate`, `sv_maxrate`, `sv_mincmdrate`, `sv_maxcmdrate`,
   `fps_max`, `mat_hdr_level`, `fov_desired`.
2. **SourceMod on build 24600426.** Start the 32-bit server with
   Metamod and SourceMod 1.12 as CS:S's probe does and check that the
   SDK tools extension loads (the `hl2mp` offsets match); then the same
   with the game's `srcds_linux64` and the x64 builds if 64-bit is ever
   needed.
3. **Jump speed in multiplayer.** The SDK has no bunny-hop cap and gives
   the forward jump boost only on one-slot servers, yet a cvar
   `player_limit_jump_speed` ships. Measure with the probe: repeated
   jumps with forward held, with back held (the "accelerated back hop"
   players talk about), and with `player_limit_jump_speed` 0 and 1. H1
   writes what the probe shows.
4. **The ducked hull and the air duck lift.** Expected (−16,−16,0)–
   (16,16,36), eye 28, lift 36 in the air; CS:S measured differently from
   its own shared values, so measure as movement.md did.
5. **Ladder-contents brushes on dm_lockdown.** Expected not climbable;
   confirm in the game and say whether they block or are passable.
6. **Bullet damage to props and breakables.** The weapon script's
   `damage` is used against players only; against other targets the ammo
   type's damage applies, which in HL2:DM's table is 0 and so falls back
   to a rules-level value. H3 must state that value per ammo type
   (measure: shots to break a `func_breakable` of known health).
7. **Pistol and .357 spread.** The code shoots with no spread; confirm on
   the probe that 20 pistol shots at a wall from 512 units land on one
   point.
8. **Flashlight and aux power.** The HL2 suit drains aux power for the
   flashlight in single player; HL2:DM's flashlight code adds no drain.
   Confirm in the game (aux bar while the flashlight is on).
9. **HDR.** All stock maps carry HDR lighting and most skies are `_hdr`.
   If the game runs with `mat_hdr_level` 2 by default, refcmp captures
   must set 0 to compare with Chimera's LDR path, or H9 specs HDR.
10. **`.ctx` vs `.txt`.** Which of the two the game reads first, and
    whether they ever differ, is unknown; the `.txt` files are assumed to
    be the truth.
11. **Respawn and observer mode** (1.12): whether `mp_forcerespawn` still
    respawns a player who has become an observer 3 s after death (H6).
