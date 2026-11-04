# Half-Life 2: Deathmatch: map entities beyond specs/source/ (H10)

Source basis: the public Source SDK 2013 multiplayer game code — the
shared item base and the HL2 ammo, health-kit and battery items, the ammo
crate, the newer model-based wall chargers, the weapon base's fall,
materialize and respawn paths and the HL2:DM weapon base, the HL2:DM
rules (respawn times, respawn spots, object relocation, spawn-spot
validity and the restart keep list), the respawnable physics prop, the
headcrab canister (server, shared flight path and client), the flare
(server and client), the physics motor and its controller, the angular
velocity sensor, the physics-only clip brush, the ragdoll manager and the
ragdoll retirement list, the dust entities (server and client), the
combine camera, the AI node entity, the screen shake entity with the
server's shake helper and the client's shake view effect, the microphone
and the AI sound list, the think scheduler and the server frame order —
read in place in a separate spec session. From the game's files on this
box (game build 24600434 and dedicated server build 24600426, read from
the two app manifests; `hl2mp/steam.inf` PatchVersion 10889068 in both,
checked): the entity lumps of the 8 stock maps in `hl2mp/maps/` (parsed
in place for class counts and keyvalues only), the sequence, event and
attachment tables and hull sizes of the item, crate, charger, canister,
camera and flare models (Half-Life 2's misc VPK, HL2:DM's VPK where it
overrides), and Half-Life 2's loose sound scripts (entry names and
levels only). Nothing was launched; no server was started.
Status: draft

## Summary

HL2:DM's maps are made of Source entities that specs/source/ and
specs/cs_source/ already cover, plus a layer of Half-Life 2 classes: 329
respawning physics props, 238 ammo boxes, 129 health and armour items,
145 placed weapons, 20 wall chargers and 2 ammo crates, all of which come
back — items 30 s after they are taken (2000 ticks), weapons 20 s (1333
ticks), both at the spot where they first **came to rest**, not where the
mapper put them — and a periodic relocation that pulls any of them back
when it lies more than 64 units from that spot. Ammo boxes give a fixed
count (pistol 20, SMG 45, AR2 20, .357 6, bolts 6, buckshot 20, one
rocket, one SMG grenade, one energy ball; "large" boxes 100, 225, 100,
20) and are used up even when only part fits. The rest is a handful of
set pieces: dm_runoff's orbital strike (six `env_headcrabcanister`s that
fall in 4 s and blast 150 damage in 350 units), dm_lockdown's fan (a
`phys_motor` at 1500°/s watched by a `point_angularvelocitysensor` at
1400°/s), dm_steamlab's gas trap (an `env_microphone` that hears bullet
impacts), dm_runoff's `env_shake`, dm_resistance's burning `env_flare`s,
a sweeping `npc_combine_camera` on dm_underpass, and invisible
`func_clip_vphysics` walls that stop only physics objects. Every one of
the 107 classes that occur in the stock maps is listed below with the
spec that covers it.

This spec continues [source_deltas.md](source_deltas.md) (H0) and the
wave 1 and 2 specs; it does not repeat their behaviour. Pickup effects on
the player (health, armour, ammo limits, weapon pickup rules) are
[suit.md](suit.md) and [weapons.md](weapons.md); ladders are
[movement.md](movement.md); spawn-spot choice is [rules.md](rules.md);
respawnable props and physics damage are [gravity_gun.md](gravity_gun.md).

## Units and conventions

- Source units (inches), Z up, X forward, Y left; angles in degrees as
  (pitch, yaw, roll), pitch positive looks **down**; seconds; server
  tick 0.015 s (66.67 Hz). Tick n has time n · 0.015.
- **Think timing** (as projectiles.md): an entity that asks to think d
  seconds from now thinks on tick now + round(d / 0.015): 0.01 s → 1
  tick, 0.05 → 3, 0.1 → 7, 0.2 → 13, 0.25 → 17, 0.5 → 33, 1 → 67, 1.5
  → 100, 20 → 1333, 30 → 2000, 60 → 4000. Inside a think the time is
  that tick's time.
- **Compared timers** (a stored time T tested against the current time):
  the first tick whose time is ≥ T, i.e. ⌈d / 0.015⌉ ticks: 0.2 s → 14,
  1.5 → 100, 20 → 1334, 30 → 2000. A strict "greater than" is said where
  it applies.
- **Server frame order** used below: player commands and entity thinks
  (with the physics step) → the rules' think (relocation) → the entity
  I/O queue (entity_io.md).
- "Rest" means the entity's physics object reports asleep; for an entity
  without a physics object, that it stands on the ground.
- Damage kinds as suit.md (blast 64, generic 0). Impulse in kg·in/s,
  angular speeds in degrees per second.
- Keyvalue, spawnflag, input and output names are the map's; sound
  entries and model paths are the game's.
- Steam builds read: game 24600434, dedicated server 24600426.

## Constants

| Name (ours) | Value | Unit | Meaning |
|---|---|---|---|
| item_respawn | `sv_hl2mp_item_respawn_time` = 30 | s | taken item hidden this long (2000 ticks) |
| weapon_respawn | `sv_hl2mp_weapon_respawn_time` = 20 | s | taken map weapon's copy appears after this (1333 ticks) |
| relocate_distance | 64 | units | farther than this from its spot, at rest: sent back |
| settle_check | 0.1 | s | rest test of a new item or placed weapon (every 7 ticks) |
| weapon_floor_drop | 256 | units | placed weapons and respawned items search this far down for a floor |
| item_touch_bloat | 24 | units | item touch box grown 24 sideways, 12 up (suit.md) |
| item_touch_bloat_held | 48 | units | while held by the gravity gun: 48 sideways, 24 up |
| constrain_limit | 4535.9 | kg (10000 lb) | force and torque limit of a "start constrained" item or weapon |
| no_respawn_flag | 1073741824 | spawnflag | bit 30: taken item or weapon never comes back |
| respawn_light_radius | 32–64 | units | uniform; flash on reappearing, models with an attachment only |
| respawn_light_time | 0.05 | s | flash life |
| crate_close_delay | 1.5 | s | after the last use (100 ticks) |
| crate_think | 0.1 | s | crate animation think (7 ticks) |
| charger_refill_suit | 30 | s | 60 for the citadel flag |
| charger_refill_health | 60 | s | |
| canister_world_think | 0.2 | s | 13 ticks |
| canister_rotation | 90 | °/s | spin about its long axis in flight |
| canister_inner_fraction | 0.25 | – | of DamageRadius: full damage inside |
| canister_push | 750 | in/s | physics Δv per unit of blast scale (mass cancels) |
| canister_player_push | 250–1000 | in/s | players near the impact, by blast scale |
| `sk_env_headcrabcanister_shake_amplitude` | 50 | – | canister impact shake (capped to 16) |
| `sk_env_headcrabcanister_shake_radius` | 1024 | units | |
| `sk_env_headcrabcanister_shake_radius_vehicle` | 2500 | units | unused in DM |
| shake_amplitude_cap | 16 | – | every screen shake's amplitude is capped to this |
| shake_max_client | 32 | – | shakes a client runs at once |
| sensor_fire_interval default | 0.2 | s | `fireinterval` when absent |
| motor_spin | `speed` / `spinup` | °/s² | target ramp rate; `spinup` 0 → `speed` per second |
| mic_think | 0.1 | s | 7 ticks |
| bullet_ai_sound | 200 units, 0.5 s | – | every bullet impact (volume = radius) |
| blast_ai_sound | max(128, 1.5 R), 0.25 s | – | every blast radius damage of radius R |
| `g_ragdoll_maxcount` | 8 | – | replaced by `game_ragdoll_manager` (4 on two maps) |
| `muzzleflash_light` | 1 | – | client cvar gating the respawn flash light |

## Corrections to H0

1. **Part 1, specs/source/ table, game_entities.md "same (`env_fade`,
   `env_explosion`, `env_shake` occur)".** game_entities.md does not
   cover `env_shake`; no spec does. It is specced here (§15).
2. **1.13, sprites_dust.md: "HL2:DM adds `func_dustcloud` (halls3, 5)
   and `env_lightglow` (34), which it doesn't cover (H10)".**
   `env_lightglow` is fully covered by specs/source/visual_entities.md §6
   (HL2:DM adds nothing, §13 here gives the placement), and
   `func_dustcloud` is sprites_dust.md's `func_dustmotes` without the
   screen scaling (one line there, §13 here).
3. **Classes H0 does not mention at all** (no spec covers them):
   `env_microphone` (dm_steamlab), `npc_combine_camera` and `info_node`
   (dm_underpass, dm_lockdown), `func_lod`, `func_occluder`,
   `func_areaportal`, `env_sun`, `env_tonemap_controller`,
   `info_lighting_relative`. The first three are specced here; the
   engine ones are CS:S's too (census, §0).
4. **Part 2, "Placed pickups per map are in H10's brief".** They are in
   §0 here; H0's other counts (entities per map, spawn classes, ropes,
   sprites, decals, ladders 26/131, respawnable props 329, `env_fire` 5,
   `trigger_hurt`) are confirmed exactly.

## Corrections to wave 1/2

1. **weapons.md §10 and suit.md ("Pickups"): the respawn spot.** The
   "original spawn point" an item or placed weapon returns to is **where
   it first came to rest**, not the map's `origin`: an item is a physics
   object from the start and records its spot on the first 7-tick check
   at which it is asleep; a placed weapon is first dropped straight down
   to the floor (up to 256 units) and records its spot when it becomes a
   physics object on the first check after that (§2).
2. **weapons.md §10, relocation "once per 20 s (weapons) or 30 s (items)
   each, starting at map start".** An object joins the relocation list
   only when it records its spot (above); its first check is on the
   rules' think of that same tick, then every 20 s (weapons, 1334 ticks)
   or 30 s (items, 2000 ticks) after each check. A taken item stays in
   the list (it waits hidden at its spot); a taken weapon leaves it and
   its copy joins when the copy appears (§2).
3. **weapons.md §10 and suit.md: "shows a muzzle-flash effect" / "a
   flash".** The only visible effect is a 0.05 s dynamic light of radius
   32–64, colour (255, 192, 64), at the model's **first attachment**, and
   only when the model has one and the client's `muzzleflash_light` is 1.
   Of the stock pickups only the AR2 energy ball box, the RPG round and
   the world models of the .357, AR2, shotgun, RPG and S.L.A.M. have an
   attachment; kits, batteries, other ammo boxes, the crowbar, crossbow
   and frag reappear with the sound only (§2).
4. **weapons.md Units ("a delay of d seconds lands ⌈d / 0.015⌉ ticks
   later").** That holds for compared timers. The respawn delays are
   thinks and land on the nearest tick: a taken weapon's copy appears
   **1333** ticks (19.995 s) later, a taken item 2000 ticks later
   (Units; Open questions 1).
5. **suit.md, "Spawn-flag variants of `item_suitcharger`" and charger
   dial.** Holds; this spec adds the entity side (inputs `Recharge` and
   `SetCharge`, `OnFull` at map load, the citadel dial running 5× ahead
   of its pool) in §4.

No other wave 1/2 fact was found wrong; movement.md's ladder facts
(three dm_underpass ladders named `ladder_03`, none of its nodes
targeted, all ladders plain) and gravity_gun.md §13's respawnable counts
(`RespawnTime` 60 × 229, 6000 × 68, 15 × 12, 90 × 11, 120 × 5, 5 × 3,
25 × 1; spawnflag bits) are confirmed exactly.

## Behavior

### 0. The stock maps: every class, per map

Counts from the 8 entity lumps (worldspawn included; totals equal H0's
per-map entity counts). "here §n" is a section of this spec. No stock
map contains `point_combine_ball_launcher` or `func_physbox_multiplayer`
(confirmed), nor `item_suit`, `weapon_pistol`, `weapon_smg1`,
`weapon_stunstick`, `weapon_physcannon`, `game_player_equip` or any
brush charger (`func_recharge`, `func_healthcharger`).

| Class | lockdown | overwatch | powerhouse | resistance | runoff | steamlab | underpass | halls3 | Total | Covered by |
|---|---|---|---|---|---|---|---|---|---|---|
| `ambient_generic` | 21 | 1 | 47 | 18 | 28 | 37 | 11 | 16 | 179 | cs_source/sounds.md |
| `env_beam` | – | – | – | – | – | 2 | – | – | 2 | source/visual_entities.md §4 |
| `env_explosion` | – | – | – | – | – | 5 | – | – | 5 | source/game_entities.md §6 |
| `env_fade` | – | – | – | – | 1 | – | – | – | 1 | source/game_entities.md §3 |
| `env_fire` | – | – | – | – | – | 5 | – | – | 5 | source/fire.md |
| `env_flare` | – | – | – | 7 | – | – | – | – | 7 | here §9 |
| `env_fog_controller` | 1 | 1 | 1 | – | 1 | 1 | 1 | 1 | 7 | cs_source/shaders.md (fog), water.md |
| `env_headcrabcanister` | – | – | – | – | 6 | – | – | – | 6 | here §8 |
| `env_lightglow` | – | – | – | 28 | – | 6 | – | – | 34 | source/visual_entities.md §6; here §13 |
| `env_microphone` | – | – | – | – | – | 1 | – | – | 1 | here §16 |
| `env_shake` | – | – | – | – | 1 | – | – | – | 1 | here §15 |
| `env_smokestack` | – | – | – | – | – | – | – | 1 | 1 | source/particles_and_smoke.md §3 |
| `env_soundscape` | 62 | 2 | – | – | 3 | 5 | 8 | 26 | 106 | cs_source/sounds.md |
| `env_soundscape_proxy` | – | 8 | – | – | 25 | 34 | 25 | – | 92 | cs_source/sounds.md |
| `env_spark` | – | – | – | 3 | – | 2 | – | – | 5 | source/visual_entities.md §8 |
| `env_sprite` | 19 | 5 | 112 | 28 | 21 | 40 | 24 | 15 | 264 | cs_source/sprites_dust.md |
| `env_steam` | – | – | 17 | – | – | 13 | – | – | 30 | source/visual_entities.md §7 |
| `env_sun` | – | 1 | 1 | – | 1 | – | 1 | 1 | 5 | engine, as CS:S; no spec |
| `env_tonemap_controller` | 1 | 1 | 1 | 1 | 1 | 1 | 1 | 1 | 8 | engine HDR, as CS:S; no spec (H0 Q9) |
| `filter_activator_name` | 1 | – | – | – | – | – | – | – | 1 | source/entity_io.md (filters) |
| `func_areaportal` | – | – | – | 9 | 17 | 15 | 48 | – | 89 | engine visibility, as CS:S; no spec |
| `func_breakable` | 4 | – | – | – | 6 | – | – | – | 10 | source/breakables.md |
| `func_breakable_surf` | 13 | 4 | 2 | – | 6 | – | – | 6 | 31 | source/breakables.md |
| `func_brush` | 5 | 3 | – | – | – | – | 11 | 1 | 20 | brush entity as CS:S; no dedicated spec (Q8) |
| `func_button` | 1 | – | – | – | 1 | – | 3 | – | 5 | source/doors_buttons.md |
| `func_clip_vphysics` | 3 | 6 | – | – | 10 | – | 13 | – | 32 | here §11 |
| `func_door` | – | – | – | – | 7 | – | – | – | 7 | source/doors_buttons.md |
| `func_door_rotating` | 1 | – | – | – | – | 2 | – | – | 3 | source/doors_buttons.md |
| `func_dustcloud` | – | – | – | – | – | – | – | 5 | 5 | cs_source/sprites_dust.md §7–9; here §13 |
| `func_dustmotes` | – | – | – | 1 | – | – | 2 | 5 | 8 | cs_source/sprites_dust.md |
| `func_illusionary` | – | – | – | 2 | – | – | 3 | – | 5 | brush entity as CS:S; no dedicated spec (Q8) |
| `func_lod` | – | – | – | – | 3 | – | 3 | – | 6 | engine, as CS:S; no spec |
| `func_occluder` | – | 11 | 8 | – | 3 | – | – | – | 22 | engine visibility, as CS:S; no spec |
| `func_physbox` | 2 | 1 | – | 6 | 2 | – | – | – | 11 | source/physics_brushes.md §2 |
| `func_useableladder` | 3 | – | 6 | 1 | 7 | 4 | 5 | – | 26 | hl2dm/movement.md; here §7 |
| `func_wall` | – | – | – | – | – | – | 4 | – | 4 | brush entity as CS:S; no dedicated spec (Q8) |
| `game_ragdoll_manager` | 1 | – | 1 | – | – | – | – | – | 2 | here §12 |
| `info_ladder_dismount` | 14 | – | 28 | 4 | 29 | 24 | 32 | – | 131 | hl2dm/movement.md; here §7 |
| `info_lighting_relative` | 2 | – | – | – | – | – | – | – | 2 | engine lighting, as CS:S; no spec |
| `info_node` | 1 | – | – | – | – | – | 2 | – | 3 | here §14 |
| `info_player_combine` | – | – | – | – | 8 | – | – | – | 8 | hl2dm/rules.md; here §6 |
| `info_player_deathmatch` | 33 | 17 | 18 | 20 | 9 | 24 | 17 | 10 | 148 | hl2dm/rules.md; here §6 |
| `info_player_rebel` | – | – | – | – | 6 | – | – | – | 6 | hl2dm/rules.md; here §6 |
| `info_player_start` | 1 | 1 | 1 | 1 | 1 | 1 | – | – | 6 | hl2dm/rules.md; here §6 |
| `info_target` | 8 | – | – | – | – | 9 | 14 | 1 | 32 | source/entity_io.md (named targets) |
| `info_teleport_destination` | – | – | – | – | – | – | – | 5 | 5 | source/triggers.md |
| `infodecal` | 81 | 91 | 221 | 213 | 96 | 241 | 277 | 16 | 1236 | cs_source/overlays_decals.md |
| `item_ammo_357` | 1 | 1 | 2 | 1 | 1 | 2 | 2 | 1 | 11 | here §1–2 |
| `item_ammo_357_large` | – | – | – | – | – | – | 1 | – | 1 | here §1–2 |
| `item_ammo_ar2` | – | – | 2 | – | 1 | 1 | 3 | – | 7 | here §1–2 |
| `item_ammo_ar2_altfire` | 6 | 1 | 3 | 2 | 1 | – | 6 | 1 | 20 | here §1–2 |
| `item_ammo_ar2_large` | 1 | – | 4 | – | – | – | 4 | – | 9 | here §1–2 |
| `item_ammo_crate` | – | – | – | – | – | 2 | – | – | 2 | here §3 |
| `item_ammo_crossbow` | – | – | 4 | – | 1 | 1 | 5 | 1 | 12 | here §1–2 |
| `item_ammo_pistol` | 4 | – | 12 | 1 | – | – | – | – | 17 | here §1–2 |
| `item_ammo_pistol_large` | – | – | – | – | – | – | 1 | – | 1 | here §1–2 |
| `item_ammo_smg1` | 3 | 1 | 10 | 1 | 3 | 1 | 2 | 2 | 23 | here §1–2 |
| `item_ammo_smg1_grenade` | 6 | 5 | 3 | 1 | 6 | 10 | 6 | 4 | 41 | here §1–2 |
| `item_ammo_smg1_large` | – | – | 4 | – | – | – | 7 | – | 11 | here §1–2 |
| `item_battery` | 5 | 1 | 8 | – | 11 | 7 | 10 | 2 | 44 | hl2dm/suit.md; here §1–2 |
| `item_box_buckshot` | 6 | 1 | 7 | 1 | 4 | 8 | 7 | 1 | 35 | here §1–2 |
| `item_healthcharger` | 1 | – | 4 | – | – | – | 2 | 1 | 8 | hl2dm/suit.md; here §4 |
| `item_healthkit` | 22 | 2 | 24 | 1 | 12 | 4 | 9 | 2 | 76 | hl2dm/suit.md; here §1–2 |
| `item_healthvial` | – | – | – | – | – | – | 9 | – | 9 | hl2dm/suit.md; here §1–2 |
| `item_rpg_round` | 2 | 1 | 3 | 1 | 1 | – | 3 | 1 | 12 | here §1–2 |
| `item_suitcharger` | 2 | – | 2 | 2 | – | 2 | 3 | 1 | 12 | hl2dm/suit.md; here §4 |
| `keyframe_rope` | 8 | 22 | 142 | 46 | 20 | 123 | 21 | 13 | 395 | cs_source/ropes.md |
| `light` | – | 6 | 172 | 41 | 22 | 18 | – | 18 | 277 | compile-time lighting; switchable styles as source/prop_damage.md |
| `light_environment` | 3 | 8 | 2 | 2 | 2 | 2 | 2 | 1 | 22 | cs_source/shadows_sky.md |
| `light_spot` | 57 | 12 | 152 | 65 | 21 | 115 | 84 | 18 | 524 | compile-time lighting; switchable styles as source/prop_damage.md |
| `logic_auto` | 2 | 1 | 2 | 1 | 2 | 2 | 3 | 1 | 14 | source/entity_io.md |
| `logic_relay` | 9 | 1 | 1 | 1 | 7 | 1 | 1 | – | 21 | source/entity_io.md |
| `logic_timer` | – | – | – | 5 | – | – | – | – | 5 | source/entity_io.md |
| `math_counter` | 2 | 2 | 2 | – | – | – | – | – | 6 | source/entity_io.md |
| `move_rope` | 1 | 10 | 42 | 31 | 10 | 17 | 15 | 5 | 131 | cs_source/ropes.md |
| `npc_combine_camera` | – | – | – | – | – | – | 1 | – | 1 | here §14 |
| `phys_ballsocket` | – | 2 | 9 | – | – | 5 | – | – | 16 | source/physics_constraints.md §3 |
| `phys_hinge` | 1 | – | – | – | – | – | – | – | 1 | source/physics_constraints.md §4 |
| `phys_lengthconstraint` | 1 | – | – | – | – | – | – | – | 1 | source/physics_constraints.md §6 |
| `phys_motor` | 1 | – | – | – | – | – | – | – | 1 | here §10 |
| `point_angularvelocitysensor` | 1 | – | – | – | – | – | – | – | 1 | here §10 |
| `point_spotlight` | 9 | – | 134 | 13 | 1 | 10 | 15 | – | 182 | source/visual_entities.md §2 |
| `point_template` | – | – | 2 | – | 1 | – | – | – | 3 | source/viewcontrol_and_templates.md |
| `prop_door_rotating` | – | – | – | – | 1 | – | – | – | 1 | source/doors_buttons.md |
| `prop_dynamic` | 7 | – | 6 | 1 | 2 | 6 | – | 5 | 27 | source/prop_damage.md |
| `prop_dynamic_override` | – | – | – | – | – | 2 | – | – | 2 | source/prop_damage.md |
| `prop_physics` | 100 | 98 | 48 | 49 | 104 | 201 | – | 90 | 690 | source/prop_damage.md, cs_source/physics_props.md, hl2dm/gravity_gun.md |
| `prop_physics_multiplayer` | – | 7 | 11 | – | 2 | 1 | 92 | 1 | 114 | source/prop_damage.md, hl2dm/gravity_gun.md |
| `prop_physics_override` | – | 1 | – | – | – | – | 7 | – | 8 | source/prop_damage.md |
| `prop_physics_respawnable` | 80 | 14 | 68 | 14 | 45 | 50 | 23 | 35 | 329 | hl2dm/gravity_gun.md §13; here §5 |
| `shadow_control` | 1 | 1 | 1 | – | 1 | 1 | – | 1 | 6 | cs_source/shadows_sky.md |
| `sky_camera` | – | 1 | 1 | 1 | 1 | – | 1 | 1 | 6 | cs_source/shadows_sky.md |
| `trigger_hurt` | – | – | – | – | 8 | 7 | 1 | – | 16 | source/triggers.md; hl2dm/suit.md (armour) |
| `trigger_multiple` | 7 | – | – | – | – | 2 | 2 | – | 11 | source/triggers.md |
| `trigger_once` | – | – | – | – | – | – | – | 5 | 5 | source/triggers.md |
| `trigger_push` | 1 | – | – | – | – | – | 1 | – | 2 | source/triggers.md |
| `trigger_teleport` | 1 | – | – | – | – | 1 | – | 5 | 7 | source/triggers.md |
| `water_lod_control` | – | – | – | – | 2 | – | 1 | – | 3 | cs_source/water.md |
| `weapon_357` | 3 | 1 | 1 | – | 1 | 1 | 3 | 1 | 11 | hl2dm/weapons.md §8, §10; here §1–2 |
| `weapon_ar2` | 1 | 1 | 2 | 1 | 2 | 1 | 3 | 1 | 12 | hl2dm/weapons.md §8, §10; here §1–2 |
| `weapon_crossbow` | 1 | 1 | 2 | – | 1 | 1 | 2 | 1 | 9 | hl2dm/weapons.md §8, §10; here §1–2 |
| `weapon_crowbar` | – | – | 1 | – | – | – | – | – | 1 | hl2dm/weapons.md §8, §10; here §1–2 |
| `weapon_frag` | 8 | 4 | 24 | 2 | 7 | 5 | 6 | 5 | 61 | hl2dm/weapons.md §8, §10; here §1–2 |
| `weapon_rpg` | 1 | 1 | 1 | 1 | 1 | 1 | 2 | 1 | 9 | hl2dm/weapons.md §8, §10; here §1–2 |
| `weapon_shotgun` | 4 | 1 | 5 | 1 | 2 | 2 | 2 | 1 | 18 | hl2dm/weapons.md §8, §10; here §1–2 |
| `weapon_slam` | – | 2 | 12 | 1 | 1 | 4 | 4 | – | 24 | hl2dm/weapons.md §8, §10; here §1–2 |
| `worldspawn` | 1 | 1 | 1 | 1 | 1 | 1 | 1 | 1 | 8 | engine |
| **all** | 648 | 364 | 1400 | 630 | 608 | 1090 | 878 | 337 | 5955 | |

Placement keys seen on the pickups: `origin`, `angles` (mostly flat
yaws; some weapons rolled ±90 or tilted), `fadescale` 1 and
`fademindist` −1 (no distance fade). No item has `spawnflags`; 17
weapons carry `spawnflags` (16 × 0, 1 × 256, which means nothing to a
weapon). Leftover keys from other classes (`IdleModifier`,
`cheapwaterstartdistance`, `cheapwaterenddistance`, and one `OnBreak`
output on dm_lockdown's `weapon_357` naming an entity that doesn't
exist) are ignored.

### 1. Ammo boxes and the other walk-over items

Every `item_*` below except the chargers and the crate is the same kind
of entity: a model with a physics object that players touch to take.
Kits, vials and batteries give health and armour (suit.md); the others
give ammunition:

| Class (alias the code also accepts) | Model (`models/…`) | Gives | Carry limit | Stock count |
|---|---|---|---|---|
| `item_ammo_pistol` (`item_box_srounds`) | `items/boxsrounds.mdl` | 20 Pistol | 150 | 17 |
| `item_ammo_pistol_large` (`item_large_box_srounds`) | `items/boxsrounds.mdl` | 100 Pistol | 150 | 1 |
| `item_ammo_smg1` (`item_box_mrounds`) | `items/boxmrounds.mdl` | 45 SMG1 | 225 | 23 |
| `item_ammo_smg1_large` (`item_large_box_mrounds`) | `items/boxmrounds.mdl` | 225 SMG1 | 225 | 11 |
| `item_ammo_ar2` (`item_box_lrounds`) | `items/combine_rifle_cartridge01.mdl` | 20 AR2 | 60 | 7 |
| `item_ammo_ar2_large` (`item_large_box_lrounds`) | `items/combine_rifle_cartridge01.mdl` | 100 AR2 | 60 | 9 |
| `item_ammo_357` | `items/357ammo.mdl` | 6 357 | 12 | 11 |
| `item_ammo_357_large` | `items/357ammobox.mdl` | 20 357 | 12 | 1 |
| `item_ammo_crossbow` | `items/crossbowrounds.mdl` | 6 XBowBolt | 10 | 12 |
| `item_box_buckshot` | `items/boxbuckshot.mdl` | 20 Buckshot | 30 | 35 |
| `item_rpg_round` (`item_ml_grenade`) | `weapons/w_missile_closed.mdl` | 1 RPG_Round | 3 | 12 |
| `item_ammo_smg1_grenade` (`item_ar2_grenade`) | `items/ar2_grenade.mdl` | 1 SMG1_Grenade | 3 | 41 |
| `item_ammo_ar2_altfire` | `items/combine_rifle_ammo01.mdl` | 1 AR2AltFire | 3 | 20 |
| `item_box_flare_rounds`, `item_flare_round`, `item_box_sniper_rounds` | `items/boxflares.mdl`, `items/flare.mdl`, `items/boxsniperrounds.mdl` | 5 / 1 FlareRound, 10 SniperRound: **ammo types HL2:DM doesn't have** | – | 0 |

(The ammo quantity scale is 1 in HL2:DM; a box never gives less than 1.)

**Spawn (map load).** The item becomes a physics object at its placed
origin and angles, in the weapon collision group (players walk through
it; it collides with the world, props and other items), not standable,
its touch box active at once (it can be taken while still falling). It
takes damage events only (bullets and blasts push it, nothing breaks
it). Its physics also collides with player-clip brushes. If its model has no physics shape it becomes
a plain box dropped to the floor (up to 256 units; removed if no floor).
From 0.1 s after spawn (7 ticks) and every 7 ticks it checks for rest;
on the first check at rest it records its **spot** (current origin and
angles), joins the relocation list (§2) and stops checking.

**Spawnflags** (item):

| Bit | Value | Meaning |
|---|---|---|
| 0 | 1 | start constrained: fixed to the world by a weld that breaks above 4535.9 kg of force or torque, or when the gravity gun picks the item up |
| 30 | 1073741824 | no respawn: removed when taken (and leaves the relocation list) |

**Touch** (a living player's box against the item's touch box, every
tick of overlap): the line-of-sight test of suit.md ("Pickups"); if it
passes, the item fires **`OnCacheInteraction`** (activator: the player)
— every tick, whether or not the item is then taken. Then the item's own
rule: kits, vials, batteries as suit.md; an ammo box adds min(count,
limit − current) to the player's pool of that ammo type, needing no
weapon of that type. If nothing was added (pool full, or the ammo type
doesn't exist) the touch is refused and the item stays. If anything was
added, the **whole box is used up** even when only part fitted; the
player plays `BaseCombatCharacter.AmmoPickup` (item channel, 75 dB) and
the game event `ammo_pickup` (ammo index, amount added, new total) is
fired; the item fires **`OnPlayerTouch`** (activator: the player), stops
touching and either respawns (§2) or, with the no-respawn flag, is
removed.

**Other inputs and outputs.** Items have the base inputs only
(entity_io.md). `+use` on an item asks to carry it, which HL2:DM refuses
while `hl2mp_allow_pickup` is 0 (gravity_gun.md §8). When the gravity
gun picks an item up, the item fires `OnCacheInteraction` (activator: the
holder), any start-constrained weld breaks, and its touch box grows to
48 sideways and 24 up until it is dropped or launched — a held box can
be "fed" to a player (or to the holder) by moving it onto them.

### 2. Respawn and relocation (items and placed weapons)

**Placed weapons, before anything happens** (weapons.md §10 continues):
a map weapon (no no-respawn flag) spawns as a non-physical trigger box,
is moved straight down onto the first floor within 256 units (no floor
in range: it stays where it is, in the air), and checks every 7 ticks
from 0.1 s after spawn whether it stands on the ground. On the first
check that it does, it becomes a physics object (still a trigger for
touches), records its spot (if it has none yet) and joins the
relocation list. A weapon hanging in the air never does either. A weapon
with the no-respawn flag skips the drop: it is a physics object from the
start (its start-constrained flag, bit value 1, is honoured only then),
records no spot and never joins the list. Weapon spawnflags: 1 start
constrained, 2 players can't pick it up, 4 the gravity gun can't punt
it, 1073741824 no respawn.

**Item taken** at tick T (respawning item): at once it stops touching,
is hidden, loses its physics object, moves to its spot and angles, and
is pushed down onto the first floor within 256 units of the spot. On
tick T + 2000 (30 s, `sv_hl2mp_item_respawn_time` read at T) it gets a
new physics object (awake: it settles again), becomes visible, plays
`AlyxEMP.Charge`, makes the respawn light (below) and becomes touchable.
It is the same entity (same name and outputs); it stays in the
relocation list throughout and does not record a new spot.

**Weapon taken** at tick T (respawning map weapon; new class or ammo
only, weapons.md §8): a copy of the same class is created at the taken
weapon's spot with the taken weapon's angles at that moment, invisible
and untouchable, dropped to the floor as above; on tick T + 1333
(`sv_hl2mp_weapon_respawn_time` 20 s, read at T) it appears: visible,
`AlyxEMP.Charge`, the respawn light, becomes a physics object, records
its own spot (where it now stands), joins the relocation list, and
becomes touchable with fresh default ammo. With `mp_weaponstay` above 0
the delay is 0 (weapons.md §10). The taken weapon itself leaves the
relocation list (picked up) or is deleted (ammo only). The copy has no
name: map outputs aimed at the original's name don't reach it.

**Respawn light** (client): when an item or weapon turns visible this
way it sends a "muzzle flash" event; a client with `muzzleflash_light` 1
makes a dynamic light at the model's first attachment, radius uniform in
32–64, colour (255, 192, 64) with exponent 5, lasting 0.05 s, decaying
to 0 over that time. Models without an attachment show nothing. Stock
models with one: `items/combine_rifle_ammo01.mdl`,
`weapons/w_missile_closed.mdl`, `weapons/w_357.mdl`,
`weapons/w_irifle.mdl`, `weapons/w_shotgun.mdl`,
`weapons/w_rocket_launcher.mdl`, `weapons/w_slam.mdl`.

**Relocation** (rules' think, every tick, after entity thinks): for each
object in the list whose next check time has come (now ≥ its next check;
0 for a newly listed object, so it is checked on the tick it joins), set
its next check to now + 30 s (item) or + 20 s (weapon), with the cvar's
current value. If its origin is more than 64 units from its spot and it
is at rest, it is teleported to its spot and angles (velocity untouched),
plays `AlyxEMP.Charge`, and its physics object is woken. Nothing else:
no hiding, no light, no check that the spot is free. An object pushed
away and still moving at its check time waits a whole interval for the
next. Objects never leave the list except when picked up as a weapon,
removed as a no-respawn item, or deleted.

**What moves pickups.** Bullets, blasts, the gravity gun (punt, carry),
physics props and other pickups move them (they never collide with
players). Moving one never makes it respawn; only relocation brings it
back, and only once at rest and beyond 64 units. A respawning item that
was moved before its first rest records the spot where it first rested
(wherever that is).

**`mp_restartgame` and map change**: every item and weapon is re-created
from the map (rules.md, Restart); spots and the list start afresh.

### 3. `item_ammo_crate`

A static, solid crate model (static physics; blocks players and bullets;
`+use`-able, one use per press, also found by the `+use` radius search
of doors_buttons.md; takes damage events only, never breaks).

Keys: `AmmoType` (0–9, default 0), `angles`, base keys. Its model is
chosen by the type (the map's `model` is ignored). Output **`OnUsed`**
(activator: the player) on every use, whatever happens. Input `Kill`
(as the base one).

| `AmmoType` | Model (`models/items/…`) | Gives per opening | Also |
|---|---|---|---|
| 0 | `ammocrate_pistol.mdl` | 300 Pistol | |
| 1 | `ammocrate_smg1.mdl` | 300 SMG1 | |
| 2 | `ammocrate_ar2.mdl` | 300 AR2 | |
| 3 | `ammocrate_rockets.mdl` | 3 RPG_Round | dm_steamlab 1 |
| 4 | `ammocrate_buckshot.mdl` | 120 Buckshot | |
| 5 | `ammocrate_grenade.mdl` | 5 Grenade | a `weapon_frag` (no ammo of its own) if the user has none |
| 6 | `ammocrate_smg1.mdl` | 50 357 | |
| 7 | `ammocrate_smg1.mdl` | 50 XBowBolt | |
| 8 | `ammocrate_ar2.mdl` | 3 AR2AltFire | dm_steamlab 1 (an AR2-cartridge crate that gives energy balls) |
| 9 | `ammocrate_smg2.mdl` | 5 SMG1_Grenade | |

All capped by the carry limits (so in practice a crate tops the pool
up). The crate models (version 44, no attachments) have bodygroup 1
"ammo" (shown/empty), sequences `idle` (1 frame), `Open` and `Close`
(35 frames at 30 fps: 1.1333 s each), and `Open` carries the server
event `AE_AMMOCRATE_PICKUP_AMMO` at cycle 0.882353 (frame 30 of 34).

**Use** by a player (or a crowbar hit, below):

1. `OnUsed` fires.
2. If the crate is not already playing `Open`: test a box for room on
   the lid — centred at the crate's origin raised by the height h of its
   world bounds, from h below that point up to 20 % of the bounds' top
   above the origin, and 20 % of the bounds horizontally — against solid
   things other than the crate. If anything is there (a player standing
   on it, a prop on the lid), stop: no opening, no sound, no change to
   the close time. Otherwise the user becomes the **receiver**, `Open`
   starts from cycle 0, `AmmoCrate.Open` plays, and the crate thinks every
   0.1 s (7 ticks) from now.
3. The close time becomes now + 1.5 s (also when already open: every use
   extends it, but a second user during `Open` never becomes the
   receiver).

**Each crate think**: advance the animation by the time since the last
advance (at most 0.2 s), fire model events passed; at
`AE_AMMOCRATE_PICKUP_AMMO`, if there is a receiver: for type 5 give the
frag weapon if missing; give the ammo; if anything was given hide the
ammo bodygroup; forget the receiver. Then, if not yet closing and the
close time has come (≤ now): forget the receiver, start `Close`. If
closing and `Close` has finished: `AmmoCrate.Close`, back to `idle`, ammo
bodygroup shown, stop thinking. A use while `Close` plays re-opens it
(step 2) with a new receiver.

Timeline (crate idle for more than 0.2 s, used on tick T, no further
use): `Open` cycle 0.176471 at T + 7, + 0.0926471 per think; the ammo
is given on T + 63 (cycle 0.917647, think 9); `Close` starts on T + 105
(first think at or after T + 100); `AmmoCrate.Close` and the full crate
on T + 182.

**Crowbar**: when the crate takes damage from a player whose weapon in
hand is `weapon_crowbar`, the player plays `HL2Player.Use` and the crate
is used by that player (OnUsed, open, ammo). The damage itself is
ignored. Any damage counts, e.g. a blast from that player's grenade
while he holds the crowbar.

### 4. Wall chargers: the entity side

Behaviour on the player is suit.md ("Wall chargers"). The entity:

- Model fixed: `models/props_combine/suit_charger001.mdl` (version 44,
  sequences `idle` 52 frames, `emptyclick` 9 frames at 6 fps, `empty`)
  and `models/props_combine/health_charger001.mdl` (version 45, `idle`
  28 frames, `emptyclick` 3 frames, `empty`); the map's `model` is
  ignored. Static physics, solid, casts no shadow, no attachments.
- Keys: `spawnflags` (suit charger: 8192 citadel, 16384 Kleiner; none for
  health), base keys. `style`, `height`, `value1`, `value2`, `value3`
  are accepted and ignored; `dmdelay` is read and then reset at spawn
  (ignored). No stock charger has a name except dm_lockdown's citadel
  charger `megacharger`.
- The **dial**: the model's cycle is 1 − display / pool max (the suit
  charger divides by max + 0.1 while being used, by max at spawn and
  refill). The display value equals the pool except while a run is on:
  each tick of use it drops by 1/7 (suit, health) or 10/7 (citadel), and
  it is reset to the pool when the charger switches off. The citadel
  dial therefore runs 5× ahead of its pool while held (pool −2 per 7
  ticks, display −10). When used empty the charger plays `emptyclick`
  and at switch-off sets cycle 1.

`item_suitcharger`:

| Input | Effect |
|---|---|
| `Recharge` | pool to max now: `SuitRecharge.Start`, `idle`, dial reset; fires `OnFull` if the pool was below max; cancels a pending switch-off or refill (a loop sound already playing is not stopped until the next run ends) |
| `SetCharge` (integer n) | pool **and pool max** set to n (the flag's 75/200/25 is replaced; later refills give n), `idle`, dial redrawn; fires no output |

| Output | When | Activator |
|---|---|---|
| `OnPlayerUse` | first charge of each run | the player |
| `OutRemainingCharge` (float) | every charge: pool / max | the player |
| `OnHalfEmpty` | a charge takes the pool from above ⌊max/2⌋ to at or below it | the charger |
| `OnEmpty` | a charge takes the pool to ≤ 0 | the charger |
| `OnFull` | the pool is set to max from another value: **at map load** (pool 0 → max), on refill, on `Recharge` | the charger |

`item_healthcharger` has no inputs beyond the base ones and outputs
`OnPlayerUse` and `OutRemainingHealth` (float pool / max) only; refill
plays `WallHealth.Recharge`. The charger refill delays are the rules'
(30 s suit, 60 s citadel, 60 s health; 2000 and 4000 ticks), scheduled
from the switch-off that finds the pool at exactly 0.

Stock use of outputs: only `megacharger` (dm_lockdown, flag 8192):
`OnEmpty` → `HideSprite` its glow sprite and `TurnOff` its green
`light_spot`; `OnFull` → `ShowSprite`, `TurnOn`. The light is on while
the charger has charge, off for the 60 s after it is drained.

Placement (counts in §0): suit chargers plain on dm_lockdown 1,
dm_powerhouse 2, dm_steamlab 2, dm_underpass 3, halls3 1; citadel
dm_lockdown 1; Kleiner dm_resistance 2. Health chargers dm_lockdown 1,
dm_powerhouse 4, dm_underpass 2, halls3 1. dm_underpass and halls3 each
have a suit and a health charger side by side (42 and 63 units apart).

### 5. `prop_physics_respawnable`: additions to gravity_gun.md §13

- Reappearing makes no sound and no light (unlike items and weapons) and
  needs no player to be absent: only the stored box at the stored spot
  must be free of solid things; otherwise it retries every 1 s (67
  ticks).
- It is the same entity throughout: its name and outputs stay; `OnBreak`
  and the prop's other outputs fire on every break. Stock names: 97
  `crates`, 46 `exploding_barrels`, 13 `toilets`, 6 `sinks1`, 6
  `sinks2`; no stock entity targets them.
- A motion-disabled respawnable has motion enabled at its death (before
  the break), then respawns motion-disabled again (its spawnflags).
- Per map, `RespawnTime` (all present): dm_lockdown 60 × 80;
  dm_overwatch 60 × 14; dm_powerhouse 6000 × 68 (effectively never
  within a match); dm_resistance 60 × 3, 90 × 11; dm_runoff 60 × 44,
  5 × 1; dm_steamlab 60 × 36, 15 × 12, 5 × 2; dm_underpass 60 × 17,
  120 × 5, 25 × 1; halls3 60 × 35. In ticks: 60 s → 4000, 90 → 6000,
  120 → 8000, 15 → 1000, 25 → 1667, 5 → 333, 6000 → 400000.

### 6. Spawn points

`info_player_deathmatch`, `info_player_combine`, `info_player_rebel`,
`info_player_start`: point entities, not solid, not drawn, no outputs,
no inputs beyond the base ones. Keys: `origin`, `angles` (the spawned
player's body and view angles, rules.md), `targetname` (none in stock
maps), and on `info_player_deathmatch` only, `master`: if it names an
entity that acts as a master (a `multisource`, none in stock maps), the
spot is valid only while that master is "triggered"; a `master` naming
nothing or a non-master entity is ignored (the spot stays valid). The
other three classes have no `master`. Choice, the 128-unit free test, the
telefrag fallback and the ring order are rules.md's. The three HL2:DM
classes are kept across `mp_restartgame`; `info_player_start` is
re-created (same data).

Stock data: all have `spawnflags` absent or 0 (33 carry the key);
`info_player_start` on six maps (not dm_underpass, halls3); team spots
only on dm_runoff (8 Combine, 6 Rebel).

### 7. Ladders: placement data

Keys, mounting and dismounting: movement.md ("Ladders"). Stock data:

| Map | Ladders | Dismount nodes | Nodes with `target` | Length (units) | `point0` is the top |
|---|---|---|---|---|---|
| dm_lockdown | 3 | 14 | 8 | 128–135 | 0 of 3 |
| dm_powerhouse | 6 | 28 | 28 | 160–291 | 4 of 6 |
| dm_resistance | 1 | 4 | 4 | 268 | 1 of 1 |
| dm_runoff | 7 | 29 | 13 | 126–214 | 3 of 7 |
| dm_steamlab | 4 | 24 | 24 | 103–174 | 4 of 4 |
| dm_underpass | 5 | 32 | 0 | 160–437 | 0 of 5 |

All ladders are vertical within 0.46° (dm_steamlab's `portal_ladder`
is the most slanted). Every ladder's `origin` is unrelated to its
points (the editor handle); only `point0` and `point1` matter. Names
repeat across maps (`ladder_03` on dm_lockdown, dm_powerhouse and three
times on dm_underpass); a node's `target` matches the first entity of
that name, which on dm_lockdown and dm_powerhouse is the ladder the node
belongs to. `StartDisabled` 0 on the 9 ladders that carry it; no ladder
is disabled or enabled by I/O in any stock map.

### 8. `env_headcrabcanister`

A Half-Life 2 headcrab shell fired from the sky. In the stock maps only
as dm_runoff's orbital strike, with no headcrabs.

Keys (defaults when absent):

| Key | Default | Meaning |
|---|---|---|
| `HeadcrabType` | 0 | 0 `npc_headcrab`, 1 `npc_headcrab_fast`, 2 `npc_headcrab_poison` |
| `HeadcrabCount` | 0 | headcrabs released after opening |
| `FlightSpeed` | 0 | units/s in the world |
| `FlightTime` | 0 | s from launch to impact |
| `StartingHeight` | 0 | extra height of the skybox arc |
| `Damage` | 0 | blast damage |
| `DamageRadius` | 0 | units |
| `SmokeLifetime` | 0 | s of smoke after landing |
| `LaunchPositionName` | – | entity to launch from inside the world |
| `MinSkyboxRefireTime`, `MaxSkyboxRefireTime` | −1, −1 | skybox-only refire |
| `SkyboxCannisterCount` | 0 | skybox-only shots (0 = endless) |
| `angles` | – | its forward axis points **back along the flight** (toward where it comes from) |

Spawnflags: 1 no impact sound, 2 no launch sound, 4096 start impacted,
8192 land at its own origin (else at the end of a 10000-unit trace
backwards along its forward axis against the world), 16384 wait for
`OpenCanister`, 32768 wait for `SpawnHeadcrabs`, 65536 no smoke,
131072 no shake, 262144 removed on impact (no damage, gone 3 s later),
524288 no impact effects.

Inputs: `FireCanister` (once per entity; a second has no effect),
`OpenCanister`, `SpawnHeadcrabs` (with their wait flags, after landing),
`StopSmoke`. Outputs: `OnLaunched` (the canister), `OnImpacted`,
`OnOpened`.

dm_runoff's six (all named `can1`): `FlightSpeed` 3000, `FlightTime` 4,
`Damage` 150, `DamageRadius` 350, `SmokeLifetime` 20,
`HeadcrabCount` 0, `StartingHeight` 0, `SkyboxCannisterCount` 1,
pitch −71 (forward 71° up, so they fall 71° down) with various yaws;
spawnflags 565248 (8192 + 32768 + 524288) on one and 565250 (+ 2, no
launch sound) on the other five: they land at their own origins, make
no impact effects, shake, smoke, open and release nothing.

**Before firing**: not solid, no model, invisible.

**Firing** (not "start impacted", no launch position; the stock case):
launch sound `HeadcrabCanister.LaunchSound` (heard everywhere) unless
flag 2. The entry point E is where a world-only trace from the canister
origin + 100·forward to + 10000·forward hits solid (the sky); the flight
direction is d = −forward. With the map's `sky_camera` (dm_runoff: scale
16):

```math
v_h = |d_{xy}|\,v,\qquad
t_{enter} = T - \frac{|E_{xy} - I_{xy}|}{v_h}
```

(v = `FlightSpeed`, T = `FlightTime`, I the impact point). If
t_enter > 0 the canister first flies in the 3D skybox for t_enter
seconds on a parabola drawn by the client at skybox scale (horizontal
speed v_h / scale, starting `StartingHeight`/scale above the line,
reaching E's skybox image with the in-world descent rate), then
appears at E. Otherwise (or with no sky camera) it starts in the world
v·T units from I on the side of E (the world flight then takes all of
T). In the world it moves in a straight
line at v along d:

```math
p(t) = E + (t - t_{enter})\,v\,d,\qquad t \le T
```

and spins about its long axis at 90°/s (angle 90·(t − t_enter)). With
the stock values v_h = 3000·cos 71° = 976.70 and v·T = 12000.

Server steps in the world: think on entering, then every 0.2 s (13
ticks). Each think: new position p(t) (t clamped to T); the first time
it is within 3000 units (1 s of flight) of I, `HeadcrabCanister.IncomingSound`
(heard everywhere) unless flag 1; then the **sweep**: every solid entity
other than the canister touched by a box of half-size 126.44 (see
below) swept from the old to the new position gets Δv = 750 along d (as
an impulse of mass × 750 at its centre, if it has a physics object) and,
if it takes damage and `Damage` ≠ 0, the full `Damage` as blast (64)
with attacker and inflictor the canister. Then, if t ≥ T: detonate (if
not yet) and land; else move there, touch triggers along the way, and
detonate early if within 126.44 of I (it still flies on and lands at T).

The half-size: in the world the canister uses `headcrabcannister01a.mdl`
(hull ±66.51 × ±21.27 × ±21.29) and makes its box a cube of half-size
r = 73.00 (the hull's half-diagonal); the sweep and the early-detonation
distance use that cube's own half-diagonal, r·√3 = 126.44.

**Detonation** at I: `OnImpacted`; `HeadcrabCanister.Explosion` (140 dB)
unless flag 1, stopping the incoming sound. With flag 262144: nothing
else, hidden, removed 3 s later. Otherwise the blast: every solid entity
whose bounds touch the sphere of radius R = `DamageRadius` about I, with
ρ = distance from I to the entity's **origin**:

```math
s = \min\!\left(1,\ \left(\frac{\rho - R}{0.75\,R}\right)^{2}\right)
```

(s = 1 within 0.25 R; 0 at R; it grows again past R, see Quirks). The
entity gets Δv = 750·s along (origin − I) normalized if it has a physics
object, and, if it takes damage and `Damage` ≠ 0, `Damage`·s blast (with
the usual explosive damage force), attacker and inflictor the canister
(no player is credited: a world kill, rules.md). A player not in a
vehicle then gets a velocity impulse along the same direction with its z
raised to at least 0.1 (and renormalized):

```math
|\Delta v| = 250 + 750\,(3s^{2} - 2s^{3})
```

(on top of projectiles.md's damage shove, which also applies). Then,
unless flag 131072, a screen shake at I: amplitude
`sk_env_headcrabcanister_shake_amplitude` (50, capped to 16), frequency
150, duration 1 s, radius `sk_env_headcrabcanister_shake_radius` 1024
(§15; players on the ground only). Unless flag 524288: an explosion
effect (fireball scale 50, magnitude 500, no damage, sound, sparks or
light) and a 10 s dust cloud.

**Landing**: `HeadcrabCanister.AfterLanding` (hiss, 70 dB) starts; the
model becomes `headcrabcannister01b.mdl` (attachments `headcrab`,
`smoke`, `trail`; sequences `idle_closed`, `idle_open`, `open` 35 frames
at 30 fps), solid with static physics at I, not moving. The flight trail
moves to the `trail` attachment. Unless flag 65536 a smoke trail at the
`smoke` attachment: 8 particles/s, life 2 s, grey 0.7 → 0.6, size 32 →
64, spawn radius 8, random speed 0–8 plus 32–64 along its direction,
opacity 0.35, for `SmokeLifetime` s. Unless waiting for `OpenCanister`:
`HeadcrabCanister.Open`, the `open` sequence, then `idle_open` and
`OnOpened`; headcrabs (count > 0, not waiting for `SpawnHeadcrabs`)
from 3 s later, one every 1–2 s at the `headcrab` attachment.

The flight trail (from launch): sprite trail `sprites/smoke.vmt`,
additive, colour (224, 224, 255), start width 32 ± 15, end width 200,
life 3 s, texture resolution 0.002, minimum fade length 1000.

**Start impacted** (4096): spawns landed at its origin (broken model,
smoke, opening per flags); `FireCanister` then only starts headcrabs.
**Launch position** (`LaunchPositionName`): flies from that entity's
origin to I on a world parabola in time T (horizontal speed = horizontal
distance / T; vertical speed and acceleration chosen to end at I with
the slope of d); a missing entity removes the canister when fired.
**Skybox-only** (the canister itself placed in the 3D skybox): flies and
explodes there (`HeadcrabCanister.SkyboxExplosion`), refiring after a
random time in [Min, Max] + 3 s, `SkyboxCannisterCount` times (0:
endless). None in the stock maps.

**dm_runoff's strike** (map scripting, entity_io.md semantics): pressing
the console button (parented to a `func_door`) fires an alarm at once,
closes the blast doors from 2 s, and 86 s later triggers the relay
`explosion_sequence`: at +0 `can1 FireCanister`; +2 an incoming sound;
+4 explosion sounds, `env_shake` `shaker` (§15), `env_fade` `fader`,
and seven `trigger_hurt` named `killer` (damage 500, generic) enabled
for 0.3 s; +5 `can1 Kill`; +6 the `point_template` `can_temp`
(spawnflag 2) spawns six fresh `can1`; +10 the blast doors open; the
console returns at 272 s. Because `can_temp` lacks the "don't remove
template entities" flag, the six map canisters are **removed at map
load**: the first strike after a map load or restart fires nothing (no
canisters fall; the killers, shake and fade still happen), and every
later strike fires the six spawned at +6 s of the previous one (Quirks;
Open questions 5). The canisters land 4 s after `FireCanister`, i.e.
together with the killers.

### 9. `env_flare`

The flame of a Half-Life 2 flare as a point entity. Model
`models/weapons/flare.mdl` (1 frame, no attachment), box ±2, not solid,
no movement, no shadows.

Keys: `scale` (default 1), `duration` (**read and never used** by a
map-placed flare). Spawnflags: 1 no dynamic light, 2 no smoke, 4
infinite, 8 start off (hidden). Inputs: `Start` (float lifetime: burn
sound fades in over 2 s to 0.8 volume, pitch 60 → 100; lifetime > 0 sets
burn-out to now + lifetime, else infinite; visible; thinks every 0.1 s),
`Die` (float: burn-out now + value), `Launch` (float speed, 0 → 1500:
flies along its angles with gravity, burns 8 s or infinite with flag 4).

**A map flare that receives no input** (all 7 stock ones): visible from
spawn, burn-out 30 s after spawn, or never with flag 4. It never thinks,
so it makes no sparks, no sound (its burn sound is prepared but never
played: dm_resistance places an `ambient_generic` playing
`weapons/flaregun/burn.wav` beside each flare) and is never removed.

**Drawing** (client, every frame while visible and not burnt out):
base scale b = `scale`, times (burn-out − now)/10 in the last 10 s
before a burn-out; not drawn when b < 0.01. Visibility v = fraction of
an 8-unit proxy visible. Two particles at the origin jittered every
frame within ±0.5b (first) and ±b (second): `effects/redflare` of size
b·U{32..48}, grey U{100..128}·v; `effects/yellowflare_noz` (no depth
test) of size b·U{2..4}, white 255·v. Unless flag 1, a dynamic light at
the origin, radius b·U(110, 128), colour (255, U{32..64}, same), renewed
every frame (life 0.1 s). Unless flag 2, 8 smoke puffs per second
(`particle/particle_noisesphere`) at the origin moved 2 units toward the
viewer and raised 4b, velocity (U(−16, 16), U(−16, 16), U(40, 48)), life
1 s, colour (255, 48, 48), alpha U{64..90} → 0, size U{2..4} → 8×.

Stock: dm_resistance, 7 flares, spawnflags 5 (no light, infinite),
`scale` 1, `duration` 30: they burn forever, silent but for their
ambient sounds, light nothing, and smoke.

### 10. `phys_motor`, `point_angularvelocitysensor` (dm_lockdown's fan)

**`phys_motor`** (logical, not networked). Keys: `attach1` (the physics
entity to drive, found by name at activation; must be a moving physics
object), `speed` (target angular speed, °/s), `spinup` (s to reach it;
0 → the ramp is `speed` per second), `addangaccel` (extra torque
headroom), `inertiafactor`, `axis` (a **world point**: the axis is the
unit vector from the motor's origin to it, snapped to a world axis when
within rounding of one; a point within 1 unit of the origin removes the
motor at spawn). Spawnflags: 1 start on, 2 no world collisions (only
with 4), 4 also hinge the object to the world on this axis through the
motor's origin (and forbid player pickup), 8 keep a local relationship.

Inputs: `TurnOn` (ramp from the current motor target toward `speed`),
`TurnOff` (motor target 0 at once; the object coasts), `SetSpeed`
(float: new `speed`, ramp toward it).

Ramp (every tick while ramping, including the `TurnOn` tick):

```math
\omega_{tgt} \leftarrow \operatorname{approach}\!\left(\text{speed},\ \omega_{tgt},\ a\,\Delta t\right),\qquad
a = \frac{|\text{speed}|}{\text{spinup}}\ \ (\text{spinup}>0),\quad a = |\text{speed}|\ \text{otherwise}
```

Drive (each physics step of length h, on the object's angular speed ω
about the axis, I its inertia about the axis, k = `inertiafactor`):
torque τ = (ω_tgt − ω)·I·k, clamped to ±I·k·(a + `addangaccel`), plus a
resistance term (the previous step's torque, scaled by how much of the
expected speed change failed to happen) — i.e. per step the speed moves
toward the target by about k·h of the gap. Nothing when ω_tgt = 0.

**`point_angularvelocitysensor`**. Keys: `target` (entity watched, found
at activation; none → never thinks), `threshold` (°/s, absolute value),
`fireinterval` (s, default 0.2), `axis` (world point) with `usehelper`
(1: measure the component along the direction from the sensor to
`axis`; 0: the magnitude). Measure each tick: a physics target's angular
speed (or 0 if its orientation changed by less than 1 % of what that
speed implies since the last tick — a stuck object); a non-physics
target's largest local angular velocity component.

Each tick: compare with the threshold (+1 greater, 0 equal, −1 less).
On a change of the comparison: if the new value equals the last one
fired, cancel any pending fire; else, unless this is the very first
measurement, fire at now + `fireinterval`. If unchanged and a fire time
is pending and has come (now ≥ it): fire, and remember the value fired.
Firing: −1 → `OnLessThan`, `OnLessThanOrEqualTo`; +1 → `OnGreaterThan`,
`OnGreaterThanOrEqualTo`; 0 → `OnEqualTo` and both "or equal" outputs
(activator: the sensor). The output `AngularVelocity` (float) is set
whenever the measured value changes. Inputs: `Test` (fire the current
comparison now, activator the caller), `TestWithInterval` (restart the
comparison and fire after the interval).

**dm_lockdown's fan.** A `func_physbox` `fan` (crush damage 1) on a
`phys_hinge` (axis +X) driven by `fan_motor` (`speed` 1500, `spinup` 0,
`inertiafactor` 125, axis +X, off at start) and watched by `fan_sensor`
(threshold 1400, `fireinterval` 0.2, magnitude). Putting the
`func_physbox` `plug2` into the `trigger_teleport` `plug_teleporter`
(filtered to `plug2`) triggers `fan_enable`: motor on at +0.1, plug
frozen and dropped; `fan_disable` at +5 s: motor off, wind off at +0.5,
plug released, teleporter re-enabled at +2. Damaging the button
`breakable_plug_bit2` also frees the plug and disables the fan. While
the fan exceeds 1400°/s the sensor enables the `trigger_push` `fan_wind`
(speed 800) and the `trigger_multiple` `fan_gib_trigger`, and plays the
running sounds; below it, stops them. Several outputs name entities that
do not exist in the map (`fan_gib_death`, `disabled_fan_sensor`,
`fan_death_exp`, `fan_death_spark`); they do nothing.

### 11. `func_clip_vphysics`

An invisible brush that collides only with moving physics objects.
Not solid to traces, players, bullets, the crossbow bolt or the RPG
missile; static physics shape from its brush. Keys: `filtername` (a
filter entity, found at activation). Inputs: `Enable`, `Disable`
(collisions on/off; it starts enabled — no `StartDisabled` key).

Rule: with a filter, it blocks exactly the physics objects the filter
passes; without one, every entity whose movement is physics and whose
physics object can move: props, `func_physbox`, items, dropped and
placed weapons, frag grenades, S.L.A.M.s, energy balls, objects held by
the gravity gun. Players (their physics shadow included) pass. Stock: 32
(dm_lockdown 3, dm_overwatch 6, dm_runoff 10, dm_underpass 13), none
with a filter or name.

### 12. `game_ragdoll_manager`

Keys: `MaxRagdollCount` (default −1), `MaxRagdollCountDX8` (−1),
`SaveImportant` (single player only). Inputs `SetMaxRagdollCount`,
`SetMaxRagdollCountDX8` (integer). Sent to all clients always. At
activation and on the inputs: the current maximum is `MaxRagdollCountDX8`
when the server's DirectX level is below 90 and that value is ≥ 0, else
`MaxRagdollCount`; a value of −1 means `g_ragdoll_maxcount` (8). Server
and clients replace the size of their ragdoll retirement list with it:
the oldest retirable ragdolls beyond the count fade out (as
cs_source/ragdolls.md's fade queue).

HL2:DM's player bodies are not in that list (player_models.md §10.3),
so in DM this only limits other ragdolls (none in stock maps). Stock:
dm_lockdown and dm_powerhouse, `MaxRagdollCount` 4, DX8 −1.

### 13. `env_lightglow` and `func_dustcloud`

`env_lightglow`: exactly specs/source/visual_entities.md §6. Stock
placement: dm_resistance 28, dm_steamlab 6; `HDRColorScale` 0.7 on all;
`OuterMaxDist` 0 or absent; glow sizes 3 × 16, 8 × 8, 16 × 5, 12 × 3,
9 × 1, 32 × 1 (vertical = horizontal on all); `GlowProxySize` 4 × 16,
2 × 9, 6 × 6, 10 × 2, 20 × 1. Distance keys: `MinDist` 500 / `MaxDist`
2000 on 17, 10/600 on 6 — and dm_steamlab's six have `MaxDist` 1 below
`MinDist` (400, 550 or 700): by visual_entities.md's remap
f = (dist − Min)/(Max − Min) clamped, these glows are **full when the
viewer is within 1 unit and fade out linearly to nothing at `MinDist`**,
the reverse of the usual fade-in. Keep (Quirks).

`func_dustcloud`: as `func_dustmotes` (cs_source/sprites_dust.md §7–9:
same keys, spawn, simulation, `TurnOn`/`TurnOff`, alpha curve and depth
fade), except the particle half-width is U(`SizeMin`, `SizeMax`) **in
world units**, not scaled by view depth. Stock: halls3 only, the five
`teleportclouds1`–`5` (`StartDisabled` 1, `SpawnRate` 160, sizes 16–32,
`SpeedMax` 150, life 3–5 s, `DistMax` 512, colour (114, 171, 182),
alpha 64), each turned on for 1 s by its teleporter's `OnStartTouch`
together with a `func_dustmotes` `teleportmotes`n.

### 14. `npc_combine_camera` and `info_node`

**`npc_combine_camera`** (dm_underpass 1: ceiling camera, yaw 227).
Model `models/combine_camera/combine_camera.mdl` (attachments `eyes`,
`light`). Keys: `innerradius` (≤ 0 → 450), `outerradius` (≤ 0 → 300;
swapped with inner if smaller), `minhealthdmg` (hits below this do
nothing), `defaulttarget`; NPC keys (`sleepstate`, `wakeradius`,
`wakesquad`, …) have no effect on it. Spawnflags: 32 become angry (flash
photos and alarm on a player in the inner radius), 64 ignore enemies,
128 start inactive. Inputs `Enable`, `Disable`, `Toggle`, `SetAngry`,
`SetIdle`; outputs `OnFoundPlayer`, `OnFoundEnemy`, `OnDamaged`. Health
50; while inactive, damage ×0.1.

Deployed (stock: flags 36 = become angry + an NPC flag): solid box
hanging 64 units below its origin, ±16 along its own forward and right
axes but built from only two corners and made axis-aligned — for yaw 227
that is x ±22.61, y ±0.79, z −64..0, a thin slab that blocks players and
bullets. Search mode, thinking every 0.05 s (3 ticks): aim goal pitch
15°, yaw = its yaw + 45°·sin(2·time), turning its pose parameters by at
most 6° per think; `NPC_CombineCamera.Move` at most once a second while
turning; `NPC_CombineCamera.Ping` once a second; eye glow
`sprites/glow1.vmt` green. It locks on to an enemy within the outer
radius, in its 120° view cone and visible within a 90° cone: amber eye,
`NPC_CombineCamera.Active` or (angry) `NPC_CombineCamera.Angry` and up to
four white flashes (`sprites/light_glow03.vmt`) with
`NPC_CombineCamera.Click` every 0.5 s from 0.4 s, tracking at 18° per
think. HL2:DM's rules set no AI relationships, so whether players count
as enemies is undefined in the SDK (Open questions 2); the expectation
is that it never locks on and only sweeps and pings. dm_underpass's:
inner 200, outer 210, `minhealthdmg` 5000000 (indestructible), no name,
no outputs.

**`info_node`** (dm_lockdown 1, dm_underpass 2): an AI navigation node.
It is consumed at map load (feeds the AI node graph, which no HL2:DM map
uses) and leaves no entity. No gameplay effect; Chimera may skip it.

### 15. `env_shake`

Keys: `amplitude` (units of view offset; capped to 16 when sent),
`frequency` (Hz of re-randomization), `duration` (s), `radius` (0 or
flag 1: everyone). Spawnflags: 1 everyone (radius forced to 0), 4 also
players in the air, 8 shake physics objects, 16 shake ropes, 32 no view
shake, 64 no controller rumble. Inputs `StartShake`, `StopShake`,
`Amplitude` (float), `Frequency` (float).

**Start** (view part; sent unless both 32 and 64 are set): for every player — skipping, on a
start, any player not on the ground unless flag 4 — with c the player's
box centre:

```math
A_p = \min(A,16)\cdot\left(1 - \frac{|c - o|}{r}\right)\ \ (r>0,\ |c-o|\le r),\qquad A_p = \min(A,16)\ \ (r = 0)
```

players beyond r get nothing; A_p ≤ 0 sends nothing. Each client keeps
up to 32 shakes; a start adds one (A_p, frequency f, duration D, ends at
now + D); a stop clears all; an amplitude or frequency command changes
the longest-lasting one.

**Client, each frame**, for each shake (time τ = time left = end − now;
removed when τ < 0 or any of A, f, D ≤ 0): when the time passes its
next re-randomization (every 1/f s), pick an offset vector with each
component U(−A, A) and a roll U(−A/4, A/4); then with φ = τ/D,

```math
k = \varphi^{2}\,\sin\!\left(\text{now}\cdot\frac{f}{\varphi}\right),\qquad
\text{view origin} \mathrel{+}= k\cdot\text{offset},\qquad
\text{roll} \mathrel{+}= k\cdot\text{roll}
```

(the sine's argument uses the absolute client time, capped at 10⁸), and
A ← A − A·Δt/(D·f). The sums of all shakes are applied to the view.

**Ropes** (flag 16): a "shake ropes" effect at the origin with radius r
and magnitude f: each rope node within r gets an upward impulse
(1 − dist/r)·f, applied as 20× acceleration and decaying ×0.95 per rope
step. With flag 1 the radius is 0, so **no rope moves**.

**Physics** (flag 8): moving physics objects whose bounds touch a box of
half-size r (512 when r is 0; at least 100 high) around the origin are
woken and, for D seconds, every tick while they touch something, pushed
at their contact point by a force F·(1 + 0.4·mass) where F is a random
horizontal-plus-up direction (x, y in U(−1, 1), z 4, normalized) times
400·A_cur, re-randomized every 1/f s, times φ²·sin(now·f/φ); A_cur
decays like the view's.

Stock: dm_runoff `shaker`, flags 25 (everyone, physics, ropes),
amplitude 14, frequency 200, duration 1, radius 1000, started by the
strike (§8): every player on the ground gets amplitude 14; ropes do
not shake (flag 1, Quirks).

### 16. `env_microphone` (dm_steamlab's gas trap)

Listens to the server's AI sound list (sounds the game inserts for
NPCs, not audio) and to played sounds for routing.

Keys: `StartDisabled`, `SoundMask`, `Sensitivity` (0 → 1, capped 10),
`SmoothFactor` (0–0.9), `MaxRange` (0: unlimited), `target` (measure
from that entity instead of itself), `SpeakerName`,
`speaker_dsp_preset`, `ListenFilter`. Spawnflags add to the mask: 1
combat, 2 world, 4 player, 8 bullet impact, 32 explosion; 16 swallow
routed sounds, 64 ignore sounds with no attenuation. Inputs `Enable`,
`Disable`, `SetSpeakerName`. Outputs `OnHeardSound` (activator: the
microphone), `SoundLevel` (float), `OnRoutedSound`.

Every 0.1 s while enabled (7 ticks; first 0.1 s after spawn or
`Enable`): for each live AI sound whose type matches the mask (and whose
owner passes the filter): heard when its distance δ to the measure point
is 0, or δ ≤ `MaxRange` (if set) and δ ≤ volume·sensitivity, with level
1 − δ/(volume·sensitivity). If any is heard, fire `OnHeardSound`. Set
`SoundLevel` to the loudest level, smoothed: new = max·(1 − s) + old·s
(s = `SmoothFactor`), exactly the max when within 0.005 of it.

AI sounds HL2:DM makes: every bullet impact (any hit of a traced bullet
or pellet) inserts a bullet-impact sound of volume 200 lasting 0.5 s at
the hit point; every blast radius damage inserts a combat sound with the
explosion context, volume max(128, 1.5 R), lasting 0.25 s; the energy
ball's explosion one of volume 180. Melee hits insert none.

With a speaker name, every sound played within range is also replayed
at the speaker entity (routing); `Enable` with a speaker sets the
client's `dsp_speaker` to `speaker_dsp_preset`. Unused in stock maps.

dm_steamlab's `bullet_mic`: `StartDisabled` 1, spawnflags 40 (bullet
impact + explosion), `MaxRange` 256, `Sensitivity` 1. Opening the valve
wheel `RotateSpeedWheel2` turns on gas and enables it; closing the wheel
(8 s later, automatic) disables it. Heard → it disables itself, and
`gasfire_2` (an `env_fire`, `StartFire`) at once and `gasfire_3` 0.4 s
later ignite with their sounds; `gasfire_2`'s `OnIgnited` explodes
`explosion_2` and enables the `trigger_hurt` `trap2_hurter` for 0.3 s,
3 s later. So a shot landing within 200 units of the microphone (or a
blast within 256) while the gas flows sets the trap off.

## Per-tick order

1. Player commands: movement (item and weapon touches during the move's
   trigger touches: suit.md, weapons.md §8), `+use` (chargers every tick
   while held, crates per press, ladders), weapons (bullet impacts insert
   AI sounds; crowbar hits on crates).
2. Physics step: motor and shake forces, sleeping and waking.
3. Entity thinks in their scheduled ticks: item rest checks and
   materialize (2000 ticks after taking), weapon fall checks and copies
   appearing (1333), crate animation, charger switch-off and refill,
   canister flight (13-tick steps), motor ramp, sensor, microphone (7
   ticks), camera (3 ticks), respawnable props.
4. Rules' think: relocation checks.
5. Entity I/O queue: outputs fired above with delay 0 land here
   (entity_io.md).
6. Clients: respawn lights, flare and dust particles, shakes, dials.

## Edge cases

- **Taken before its first rest**: an item or weapon whose spot was
  never recorded (taken while still falling, or a weapon hanging in the
  air more than 256 units above any floor) respawns at the world origin
  (0, 0, 0), dropped to the floor below it if any (Open questions 3).
- **A weapon hanging in the air** (no floor within 256 units) is never a
  physics object, never relocated, can still be taken, and its copy
  appears at the world origin.
- An ammo box whose type HL2:DM lacks (flare, sniper boxes) can never be
  taken; it still fires `OnCacheInteraction` every tick of touch.
- Two players touching one box in one tick: the first in touch order
  takes it; the other's touch finds no touch active.
- An item respawns into a player: nothing stops it; it is touched (and
  taken if useful) on the next tick.
- `sv_hl2mp_item_respawn_time` 0: items reappear on the next tick;
  relocation checks then run every tick.
- Ammo crate used by two players at once: only the one whose use opened
  it gets ammo; a use during `Close` re-opens it for the new user.
- A crate whose user dies before the event: the ammo still goes to the
  dead player's pools (harmless: the pools are reset at spawn).
- Citadel suit charger whose pool was set odd by `SetCharge`: the pool
  can step from 1 to −1; the refill is scheduled only when the pool is
  exactly 0, so it **never refills** (no stock case).
- A suit charger given `Recharge` while held keeps its loop sound until
  the run ends by release (the switch-off was cancelled; it is scheduled
  again by the next use tick).
- Canister blast and entities with distant origins: the falloff uses the
  origin; a large brush entity whose bounds touch the sphere but whose
  origin is 612.5 or more units away (with R = 350) gets the full damage
  and push.
- Canister sweep: a player in the 126-unit box around the last flight
  segment takes the full 150 blast once per think it overlaps, before
  and in addition to the impact blast. Within 0.5 s of spawning the
  pure-blast hits are refused (rules.md spawn grace).
- Microphone: the AI sound list holds a limited number of sounds; with
  heavy fire some impacts may not be inserted (engine limit; Open
  questions 7).
- The sensor's first measurement never fires; a target removed later
  stops the measurement (no output).
- Spawn spot with a `master` that is never triggered is skipped like an
  occupied one (rules.md choice).

## Quirks

- **Respawn spot = first rest spot**, not the placed origin (Keep).
- **Partial ammo pickups use up the box** (Keep).
- **Ammo can be taken without the weapon** (Keep).
- **`OnCacheInteraction` fires every tick of touch, even when refused**
  (Keep).
- **Gravity-gun feeding**: a held item's touch box doubles (Keep).
- **Weapon copies come 1333 ticks after the pickup, items 2000**, and
  relocation checks run per object from when it first rested (Keep; Open
  questions 1).
- **The respawn "flash" is a light only on models with an attachment**
  (Keep).
- **`OnFull` fires at map load** on every suit charger (Keep; harmless
  for the one stock user).
- **The citadel dial runs 5× ahead of its pool while held** (Keep).
- **dm_runoff's first strike after a map load drops no canisters**
  (template removal; Keep if Open questions 5 confirms).
- **The canister blast falloff rises again outside its radius** for
  entities whose bounds reach in (Keep).
- **Every player whose box reaches the canister's sphere gets at least
  250 units/s**, even with zero damage (Keep).
- **dm_runoff's shake moves no ropes** (the everyone flag zeroes the rope
  radius) (Keep).
- **Map flares ignore `duration`** and burn 30 s or forever, silently
  (Keep).
- **dm_steamlab's reversed light glows** (Keep).
- **The camera's collision box is a thin slab on diagonal yaws** (Keep).

## Test cases

Implementer checks (tick 0.015; times in ticks from tick 0; a "player"
is a standing player box; ammo pools start at 0 unless said).

| # | Setup | Input | After | Expected |
|---|---|---|---|---|
| 1 | each ammo box class of §1, empty pools | touch | – | pools: Pistol 20 / 100, SMG1 45 / 225, AR2 20 / 60 (large capped), 357 6 / 12 (large capped), XBowBolt 6, Buckshot 20, RPG_Round 1, SMG1_Grenade 1, AR2AltFire 1; each box hidden |
| 2 | Pistol 140 | touch `item_ammo_pistol` | – | Pistol 150; the box is used up (10 rounds lost); `OnPlayerTouch` once; `BaseCombatCharacter.AmmoPickup`; event `ammo_pickup` (amount 10, total 150) |
| 3 | Pistol 150 | stand in the box 10 ticks | 10 ticks | not taken; `OnCacheInteraction` 10 times; no `OnPlayerTouch` |
| 4 | no crossbow, XBowBolt 0 | touch `item_ammo_crossbow`, then a `weapon_crossbow` | – | XBowBolt 6; then clip 1, XBowBolt 10 (6 + 4, limit 10) |
| 5 | `item_box_flare_rounds` | touch | – | never taken |
| 6 | item spawned at tick 0, 40 units above a floor; asleep from tick 50 | – | – | rest checks at 7, 14, …; spot recorded at tick 56 (first check at rest) at the floor; joins the relocation list at tick 56; first relocation check tick 56 |
| 7 | item at rest, taken at tick 1000, default cvars | – | – | hidden 1000–2999; tick 3000: visible at its spot, `AlyxEMP.Charge`, touchable |
| 8 | as 7, `sv_hl2mp_item_respawn_time` 10 | – | – | reappears tick 1667 |
| 9 | placed `weapon_shotgun` on the floor, picked up (new class) at tick 1000 | – | – | copy visible and touchable at tick 2333 with `AlyxEMP.Charge`; its spot = where it stands then |
| 10 | `item_battery` model, `muzzleflash_light` 1 | respawn | – | no light; `item_rpg_round`: one light at the model's first attachment, radius in [32, 64], colour (255, 192, 64), gone after 0.05 s |
| 11 | item listed at tick 56, pushed 65 units at tick 100, asleep at tick 300 | – | – | checks at 56, 2056: moved back at tick 2056 with `AlyxEMP.Charge`; pushed only 64 units: never moved back |
| 12 | weapon listed at tick 10 | – | – | relocation checks at 10, 1344, 2678 (now ≥ check time, 20 s = 1334 ticks) |
| 13 | item pushed 200 units, still sliding at its check | – | – | not moved; moved at the following check if at rest then |
| 14 | item with spawnflags 1073741824 | take | – | removed; never reappears; leaves the relocation list |
| 15 | item held by the gravity gun 40 units from a player's side | – | – | touched (bloat 48); dropped: not touched at 40 (bloat 24) |
| 16 | item taken at tick 3 (before any rest check) | – | – | reappears 2000 ticks later at the world origin (dropped to a floor within 256 below it, if any) |
| 17 | `item_ammo_crate` `AmmoType` 3, idle, RPG_Round 0 | `+use` at tick T | – | `OnUsed` at T; `AmmoCrate.Open` at T; RPG_Round 3 at T + 63, ammo bodygroup hidden; `Close` starts T + 105; `AmmoCrate.Close`, bodygroup shown, thinking stops at T + 182 |
| 18 | as 17, player B uses at T + 30 | – | – | B gets nothing; close starts at T + 133 (first think ≥ T + 130); full again at T + 210 |
| 19 | as 17, a player stands on the lid | `+use` | – | `OnUsed` fires; no opening, no sound, no ammo |
| 20 | as 17, RPG_Round already 3 | `+use` | – | opens and closes as 17; nothing given; bodygroup stays shown |
| 21 | as 17 | hit with the crowbar | – | `HL2Player.Use` from the hitter, then as 17 for the hitter |
| 22 | `AmmoType` 5, no frag owned | `+use` | T + 63 | `weapon_frag` added; Grenade pool +5 (cap 5) |
| 23 | `item_suitcharger` flag 8192, armour 0, health 100 | map load; then hold `+use` from tick 1 | – | `OnFull` at load; charges at 1, 8, …, 694 (+2 each); `OnHalfEmpty` at 344 (pool 100); `OnEmpty` at 694 (pool 0, armour 200); tick 695: deny, emptyclick, refill at 4695 with `SuitRecharge.Start` and `OnFull` |
| 24 | flag 8192, held from tick 1 | – | tick 8 | pool 196, display 190 (cycle 1 − 190/200.1 = 0.0505); released after tick 8: display = pool (196) at switch-off, tick 25 |
| 25 | plain suit charger | `SetCharge 50`, then drain | – | `OnHalfEmpty` when pool reaches 25; `OnEmpty` at 0; refill 30 s later to 50 with `OnFull` |
| 26 | `item_healthcharger` with `dmdelay` 5, drained at tick k | – | – | refill at k + 4000 (60 s; `dmdelay` ignored) |
| 27 | `info_player_deathmatch` with `master` naming no entity | spawn choice | – | spot valid |
| 28 | canister `Damage` 150, `DamageRadius` 350; entities with origins at ρ = 0, 87.5, 100, 150, 200, 250, 300, 349 from the impact | detonate | – | damage 150, 150, 136.05, 87.08, 48.98, 21.77, 5.44, 0.002; physics Δv 750, 750, 680.3, 435.4, 244.9, 108.8, 27.2, 0.01; player impulse 1000, 1000, 981.8, 714.8, 437.7, 292.8, 252.9, 250.0 |
| 29 | as 28, a big brush entity with origin 700 away whose bounds touch the sphere | detonate | – | s = (350/262.5)² capped → 1: full 150 and Δv 750 |
| 30 | player at ρ = 200 directly beside the impact (direction (1, 0, 0)) | detonate | – | impulse direction normalize(1, 0, 0.1) = (0.995, 0, 0.0995) × 437.7 |
| 31 | canister `FlightSpeed` 3000, `FlightTime` 4, no sky camera, fired at tick 0 | – | – | starts 12000 units back along its flight; world thinks at 0, 13, 26, …; detonation and landing at tick 273 (first think with time ≥ 4 s); early detonation on the first think within 126.44 of the impact |
| 32 | a player in the canister's path, spawned 0.3 s ago | sweep overlaps | – | refused (pure blast within the 0.5 s spawn grace); 1 s after spawn: 150 blast |
| 33 | `env_flare` spawnflags 0, no inputs | – | – | drawn with base scale 1 until 20 s after spawn, then scale (30 − t)/10, invisible from 29.9 s; never removed, no sound, no sparks |
| 34 | `env_flare` spawnflags 5 | – | – | drawn forever, no light, smoke 8/s |
| 35 | `phys_motor` `speed` 1500, `spinup` 0 | `TurnOn` at tick k | – | motor target 22.5 at k, +22.5 per tick, 1485 at k + 65, 1500 from k + 66; `TurnOff`: target 0 at once |
| 36 | as 35, `spinup` 3 | `TurnOn` | – | target +7.5 per tick, 1500 after 200 ticks |
| 37 | sensor threshold 1400, interval 0.2; target speed crosses above at tick k and stays | – | – | `OnGreaterThan` and `OnGreaterThanOrEqualTo` at k + 14; nothing at map start while below |
| 38 | as 37, speed dips below at k + 5 and back above at k + 8 | – | – | no output for the dip; `OnGreaterThan` at k + 22 (14 ticks after k + 8) |
| 39 | after 37 fired, speed drops below at m and back above at m + 3 | – | – | nothing fires (back to the last fired value cancels the pending `OnLessThan`) |
| 40 | `func_clip_vphysics` (no filter) | a prop, a frag, a dropped weapon, a player, a bullet, a crossbow bolt cross it | – | prop, frag, weapon stop; player, bullet, bolt pass |
| 41 | `game_ragdoll_manager` `MaxRagdollCount` 4, DX8 −1, server DX level 95 | activate | – | retirement list size 4; with −1 / −1: 8 |
| 42 | `env_lightglow` `MinDist` 700, `MaxDist` 1 | viewer at 0.5, 350, 699, 800 units | – | fade 1, 0.5007, 0.0014, 0 |
| 43 | `func_dustcloud` `SizeMin` 16, `SizeMax` 32 | a particle at depth 500 | – | half-width in [16, 32] units (motes would be [0.8, 1.6] at that depth per sprites_dust.md) |
| 44 | `npc_combine_camera` yaw 227, deployed | – | – | box x ±22.61, y ±0.79, z −64..0 relative to its origin |
| 45 | camera searching, own yaw 227 | time 0.25 s | – | goal yaw 227 + 45·sin(0.5) = 248.57, goal pitch 15 |
| 46 | `env_shake` amplitude 50, radius 1024, players on the ground with centres 0, 512, 1024, 1100 away; one in the air at 0 | `StartShake` | – | amplitudes 16, 8, 0 (nothing sent), nothing, nothing |
| 47 | `env_shake` flags 1, amplitude 14 | `StartShake` | – | every player on the ground: 14; ropes unaffected |
| 48 | client shake A 14, f 200, D 1, started at client time 100.0 | frame at 100.25 | – | φ = 0.75, k = 0.5625·sin(100.25·266.67); offset re-randomized at most every 0.005 s |
| 49 | microphone mask bullet impact, `MaxRange` 256, sensitivity 1, enabled | bullet hits 150 / 210 units away | ≤ 7 ticks | `OnHeardSound` / nothing (volume 200) |
| 50 | as 49 with the explosion flag | frag blast (R 250) 250 / 300 units away | ≤ 7 ticks | heard (375 ≥ 250, within 256) / not (beyond `MaxRange`) |
| 51 | microphone `SmoothFactor` 0.5, level 0 | sound at level 0.8 for 3 thinks | – | `SoundLevel` 0.4, 0.6, 0.7 |
| 52 | `prop_physics_respawnable` `RespawnTime` 25, broken at tick 0, its spot clear | – | – | visible again at tick 1667, no sound |

Checks against the install or the probe server (game build 24600434,
dedicated server build 24600426):

| # | How | Expected |
|---|---|---|
| I1 (install) | count classnames in each stock BSP's entity lump | the §0 table exactly (648, 364, 1400, 630, 608, 1090, 878, 337 entities) |
| I2 (install) | search all 8 lumps for `point_combine_ball_launcher`, `func_physbox_multiplayer`, `item_suit`, `weapon_pistol`, `weapon_smg1`, `func_recharge` | none |
| I3 (install) | `spawnflags` of every `item_*` and `weapon_*` | items: key absent; weapons: 16 × 0, 1 × 256 |
| I4 (install) | `RespawnTime` per map of `prop_physics_respawnable` | §5's list |
| I5 (install) | ladder and dismount stats per map | §7's table |
| I6 (install) | dm_steamlab `item_ammo_crate` `AmmoType`s; dm_lockdown `megacharger` flags and outputs | 8 and 3; 8192, `OnEmpty`/`OnFull` to `megacharger_light` and `megacharger_light_` |
| I7 (install) | attachment counts of the pickup world models (§2 list) | ≥ 1 for the 7 listed, 0 for the others |
| I8 (install) | `models/items/ammocrate_*.mdl`: sequences and the pickup event cycle | `Open`/`Close` 35 frames at 30 fps; event at cycle 0.882353 |
| I9 (install) | dm_runoff `can_temp` spawnflags and `explosion_sequence` outputs | 2; the timeline of §8 |
| I10 (install) | `headcrabcannister01a.mdl` hull | ±66.51, ±21.27, −21.31..21.27 (half-diagonal 73.00) |
| P1 (probe) | take a kit on a known tick; log when it is visible again | 2000 ticks |
| P2 (probe) | take a placed shotgun; log the copy's appearance tick | 1333 ticks (1334 if the think grid is ⌈⌉; Open questions 1) |
| P3 (probe) | read an item's origin after map load vs its map `origin`; push it 100 units and log when it is moved back | rest spot ≠ placed origin for items placed above a floor; returned at its next 30 s check |
| P4 (probe) | dm_steamlab crate: log `OnUsed`, ammo, close sound ticks | test 17 |
| P5 (probe) | dm_runoff: list `env_headcrabcanister` after map load; trigger the strike twice | none after load; none fall on the first strike; six on the second (Open questions 5) |
| P6 (probe) | dm_runoff second strike: bots at measured distances from a canister's impact | test 28's damage and pushes |
| P7 (probe) | dm_lockdown: insert the plug; log the fan's angular speed and the sensor outputs | target 1500 after ~1 s; `OnGreaterThan` 14 ticks after crossing 1400 |
| P8 (probe) | dm_underpass: walk a bot under the camera; log its sounds and outputs | Open questions 2 |
| P9 (probe) | dm_steamlab: open the wheel, fire at a wall 150 / 230 units from `bullet_mic` | fires / not |
| P10 (probe) | RCON `sv_hl2mp_item_respawn_time`, `sv_hl2mp_weapon_respawn_time`, `g_ragdoll_maxcount`, `sk_env_headcrabcanister_shake_amplitude`, `sk_env_headcrabcanister_shake_radius` | 30, 20, 8, 50, 1024 |

## Open questions

1. **Think grid of respawns.** The SDK schedules the weapon copy with a
   think 20 s ahead, which lands on the nearest tick (1333); weapons.md
   assumed ⌈⌉ (1334). P2 decides; the difference is one tick.
2. **The combine camera and players.** HL2:DM's rules allocate the AI
   relationship table but set nothing in it, so the camera's disposition
   toward players is whatever the fresh table holds (expected "none", so
   it never locks on). P8 settles it; if it does lock on, it only flashes
   and makes sounds (no outputs on the stock camera).
3. **Items and weapons taken before they rest.** The SDK leaves the
   respawn spot at the world origin until the first rest check. Whether
   any stock pickup is placed where it never rests (in water, on a moving
   object, more than 256 units above a floor) is not knowable from the
   entity lump alone; check on the probe by listing every item's and
   weapon's origin 5 s after map load against its spot.
4. **Motor dynamics.** The motor's torque rule depends on the physics
   engine's integration (per-step force application, substeps, damping);
   Chimera should reach the target within about the ramp time and hold
   it, not reproduce the per-step overshoot. P7 measures the real curve.
5. **dm_runoff's first strike.** Derived from source/viewcontrol_and_templates.md
   (templates without flag 1 are removed at load) and the map data; P5
   confirms that no canister falls on the first strike after a load.
6. **Client ragdolls and `func_clip_vphysics`.** The clip brush is a
   server entity that is not networked; client-side ragdolls (all player
   bodies) probably fall through it. Check on the probe with a body on
   such a brush.
7. **AI sound list capacity.** The number of simultaneous AI sounds is
   an engine-side limit not in the reviewed code; under heavy fire some
   bullet impacts may not reach the microphone.
8. **Brush and engine classes with no spec anywhere** (`func_brush`,
   `func_wall`, `func_illusionary`, `func_areaportal`, `func_occluder`,
   `func_lod`, `env_sun`, `env_tonemap_controller`,
   `info_lighting_relative`): shared with CS:S and presumably handled by
   Chimera's Source map layer already; a Source spec for `func_brush`'s
   inputs (`Enable`/`Disable`, `Toggle`, solidity) would close the gap
   for both games.
9. **Shipped build vs SDK.** All numbers are the SDK's; the 2024–2025
   HL2:DM updates could have changed item or crate amounts. P1–P4 and P6
   compare.
10. **Probe results, round 2 (2026-11-03, dedicated server build
    24600426).** P3: items settle from their placed origins at the map
    load (pistol rounds placed at z 21 rest at 0.4; an AR2 ball box rolled
    33 units off its spot); being moved back at the next 30 s check
    wasn't measured. P4-P9 not measured.
