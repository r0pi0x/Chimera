# Half-Life 2: Deathmatch: player models, animation, hitboxes, ragdolls, faces, view models (H7)

Source basis: the public Source SDK 2013 multiplayer game code (the
HL2:DM player on server and client, its small player animation state, the
shared overlay-layer and flex code on both sides, the client ragdoll
record and its creation, the shared and HL2:DM view-model placement, bob
and lag, the client view's FOV handling, lag compensation, the Eyes and
Teeth shaders of the public shader source), read in place in a separate
spec session; the game's own files on this box (game build 24600434,
dedicated server build 24600426, both `steam.inf` PatchVersion 10889068):
the 19 player models, their `.phy` files, `hl2mp/`'s animation models and
the citizen materials, read with a throwaway reader for names, counts and
numbers only (deleted afterwards; nothing extracted was kept). No game was
launched and no server was started.
Status: draft

## Summary

HL2:DM's players are Half-Life 2's citizens, Combine soldiers and metro
police, but they animate with HL2:DM's own small animation set: every
model ends up using `models/player/male_anims.mdl` (or
`female_anims.mdl` for the six women), 105 sequences built around one
weapon-kind suffix (pistol, shotgun, smg1, ar2, gravgun, grenade, rpg,
crossbow, melee, slam). The animation state is the simple one HL2 shipped
with, not CS:S's: one main sequence chosen per tick on the server from
five activities (idle, run, crouch idle, crouch walk, jump; there is no
walk and no swim), played at speed ÷ the run cycle's authored ground
speed, a 9-way `move_yaw` blend for the legs, `aim_pitch` and `aim_yaw`
for the upper body with feet that stay put until the view turns more than
45° or 3 s pass, a client-only head that turns toward nearby players, and
server-side fire and reload gestures on overlay layers. Hitboxes come from
each model (17 or 18 boxes; only the head's ×3 matters for damage), the
death leaves a client-built `hl2mp_ragdoll` that is pushed only through
its pelvis and stays until its owner dies again, citizens blink and look
at you, and the view model bobs on HL2's fixed 0.45 s cycle and lags
behind turns with HL2's spring, not CS:S's 0.1 s history.

This spec continues [source_deltas.md](source_deltas.md) (H0) 1.6–1.8 and
1.11; [rules.md](rules.md) (H6) owns which model a player gets;
[weapons.md](weapons.md) (H3) owns the hitgroup cvars and the bullet force;
[movement.md](movement.md) (H1) owns the speeds, ducking and jumping that
drive the animation. Model file mechanics are
[../cs_source/mdl_v48.md](../cs_source/mdl_v48.md) and
[../cs_source/animation.md](../cs_source/animation.md) §1–§11; ragdoll
physics is [../cs_source/ragdolls.md](../cs_source/ragdolls.md); view-model
rendering is [../cs_source/view_models.md](../cs_source/view_models.md);
VertexLitGeneric is [../cs_source/shaders.md](../cs_source/shaders.md) §4.
Only what differs or is new is written here.

## Units and conventions

- Source units (inches), Z up, X forward, Y left; degrees; seconds.
  Server tick 0.015 s (66.67 Hz).
- Eye pitch is positive looking **down** (Source convention), yaw positive
  turning left.
- "normalize(a)" maps an angle into [−180, 180] by adding or subtracting
  360 as needed.
- Server animation runs once per user command (each tick at default
  rates) with dt = the tick interval; client animation runs once per
  rendered frame with dt = the frame time. Both are named where it
  matters.
- "Merged index" is a sequence's index in the player model's merged list
  (animation.md §8). For every one of the 19 models it is the index in
  `male_anims.mdl`/`female_anims.mdl` plus 1 (§1.2).
- Model, sequence, activity, pose-parameter, bone, attachment and flex
  controller names below are the names stored in the game's files.

## Constants

| Name (ours) | Value | Unit | Meaning |
|---|---|---|---|
| anim_model_sequences | 105 | – | sequences in `male_anims.mdl` and in `female_anims.mdl` (same names, same order) |
| anim_model_bones | 56 | – | bones in both animation models |
| moving_speed | 0.5 | units/s | horizontal speed above which the playback rate follows speed |
| moving_speed_feet | 1.0 | units/s | 3D speed above which the feet follow the eyes at once |
| run_ground_speed_n | 209.7902 | units/s | `a_RunN` (forward run cycle): 125.874 units per 0.6 s |
| run_ground_speed_ne_nw | 209.7868 | units/s | `a_RunNE`, `a_RunNW` |
| run_ground_speed_e_w | 149.8501 | units/s | `a_RunE`, `a_RunW` (also SE, SW: 149.8501) |
| run_ground_speed_s | 169.8302 | units/s | `a_RunS` |
| crouch_ground_speed | 60.0 | units/s | every `a_Crouch_walk*` direction |
| feet_turn_rate | 360 | °/s | feet yaw convergence rate (fixed; `mp_feetyawrate` 720 is not used) |
| feet_fade | 45 | ° | below this remaining angle the feet turn slower in proportion (floor 1 %) |
| feet_max_torso | 45 | ° | standing: a torso twist above this makes the feet turn to the eyes |
| aim_yaw_code_clamp | ±60 | ° | clamp applied before the pose range |
| mp_facefronttime | 3 | s | standing still this long after the last turn: feet face the eyes |
| gait_settle_rate | 4 | 1/s | standing: gait yaw approaches the body yaw by 4·dt of the gap a frame (dt < 0.25) |
| head_turn_rate | 130 | °/s | client head yaw and pitch approach rate |
| look_radius | 128 | units | other players within this (origin to origin), in front, are looked at |
| look_default_distance | 512 | units | otherwise the look target is this far ahead |
| blink_interval | U(1.5, 4.0) | s | time between blinks (client) |
| blink_duration | 0.2 | s | cvar `blink_duration` |
| eye_max_deflection | cos 30° = 0.866 | – | eyes never aim more than 30° off the eye attachment's forward |
| eye_min_forward | 6 | units | a look target closer than this in front is pushed out to 6 |
| ragdoll_bone_dt | 0.05 | s | look-back for ragdoll bone velocities (as ragdolls.md) |
| ragdoll_shot_impulse | 4000 | kg·in/s | push of a later bullet on a ragdoll (as ragdolls.md 6.2) |
| player_bodies / joints / pairs | 15 / 14 / 13 | – | every one of the 19 `.phy` files |
| bob_cycle | 0.45 | s of bob time | vertical bob period (lateral 0.9); fixed, `cl_bobcycle` ignored |
| bob_up | 0.5 | – | rising fraction of a cycle (fixed) |
| bob_speed_max | 320 | units/s | speed clamp; bob time advances at speed/320 |
| vm_lag_catchup | 5 | 1/s | view-model lag catch-up rate |
| vm_lag_max | 1.5 | – | lag vector length above which catch-up speeds up in proportion |
| vm_lag_gain | 5 | units | origin offset per unit of lag vector |
| vm_pitch_offsets | −0.035, −0.03, −0.02 | units/° | forward, right, up offset per degree of view pitch |
| viewmodel_fov | 54 | ° | cvar (client, cheat) |
| default_fov | 75 | ° | the player's base FOV (`fov_desired`, server-clamped to 75–90) |

Cvars this spec reads (defaults as registered in the shipped game; no
shipped config changes them): `mp_facefronttime` 3, `mp_feetyawrate` 720
(unused), `mp_ik` 1 (no effect), `tf2_feetyawrunscale` 2 (unused),
`blink_duration` 0.2, `flex_rules` 1, `flex_smooth` 1,
`ragdoll_sleepaftertime` 5, `cl_ragdoll_collide` 0, `g_ragdoll_maxcount`
8 (does not apply to player bodies), `viewmodel_fov` 54, `cl_bob` 0.002,
`cl_bobup` 0.5, `cl_bobcycle` 0.8 (all three unused),
`sv_wpn_sway_pred_legacy` **0** (HL2 builds; CS:S 1),
`sv_viewmodel_lag_do_angles` 1, `cl_wpn_sway_interp` 0.1 and
`cl_wpn_sway_scale` 1 (both unused while the legacy switch is 0),
`r_drawviewmodel` 1, `sk_player_*` (weapons.md).

## Behavior

### 1. The models

#### 1.1 The 19 models

Which model a player gets, the round-robin lists, the team kinds and
`cl_playermodel` are [rules.md](rules.md) "Models". The data of each:

| Model(s) | Kind | MDL version | Bones | Own sequences | Includes (HL2 paths) | Animation model reached | Flex ctl / desc | Hitboxes | `.phy` total mass (kg) |
|---|---|---|---|---|---|---|---|---|---|
| `models/humans/group03/male_01.mdl` … `male_07.mdl` | citizen | 44 | 68 | `ragdoll` | `models/humans/male_shared.mdl`, `male_ss.mdl`, `male_gestures.mdl`, `male_postures.mdl` | `male_anims.mdl` | 54 / 71 | 17 | 90 |
| `…/male_08.mdl`, `…/male_09.mdl` | citizen | **45** | 68 | `ragdoll` | same | `male_anims.mdl` | 54 / 71 | 17 | 90 |
| `…/female_01.mdl` … `_04`, `_06`, `_07` | citizen | 44 | 72 | `ragdoll` | `models/humans/female_shared.mdl`, `female_ss.mdl`, `female_gestures.mdl`, `female_postures.mdl` | `female_anims.mdl` | 54 / 71 | 18 | 70 (bodies sum to 70.65) |
| `models/combine_soldier.mdl`, `combine_super_soldier.mdl` | Combine | 46 | 44 | `ragdoll` | `models/combine_soldier_anims.mdl` | `male_anims.mdl` | 0 / 0 | 18 | 100 |
| `models/combine_soldier_prisonguard.mdl` | Combine | 44 | 44 | `ragdoll` | same | `male_anims.mdl` | 0 / 0 | 18 | 100 |
| `models/police.mdl` | metro police | 44 | 57 | `ragdoll` | `models/Police_animations.mdl`, `models/Police_ss.mdl` | `male_anims.mdl` | 10 / 0 | 17 | 90 |

- Every player model's only own sequence is `ragdoll` (activity
  `ACT_DIERAGDOLL`), merged index 0.
- `hl2mp/`'s files at the include paths (H0 1.7) hold no animation:
  `male_shared.mdl`, `female_shared.mdl`, `combine_soldier_anims.mdl` and
  `police_animations.mdl` include `models/player\male_anims.mdl` (or
  `female_anims.mdl`) and have no sequence; `male_ss`, `male_gestures`,
  `male_postures`, the female ones and `police_ss` each hold one sequence
  named `reference`, which the merge drops as a duplicate name.
- The Combine and police skeletons (44 and 57 bones) are animated by the
  56-bone animation model through bone names (animation.md §8): bones the
  model lacks are ignored, the model's bones the animation lacks keep
  their defaults.

#### 1.2 Merged list, the same for every model

Merged order: the player model (index 0 `ragdoll`), then the animation
model's 105 sequences at merged indices 1–105, nothing else. So a
sequence's merged index is the same on all 19 models and survives a model
change. Male and female animation models have the same 105 names in the
same order and the same frame counts and rates; only their bone data
and two pose ranges differ.

Pose parameters (all from the animation model; the player models have
none). Merged order and ranges:

| # | Name | Male range | Female range | Set by |
|---|---|---|---|---|
| 0 | `aim_yaw` | −49.4432 … 52.5174 | −50.8596 … 56.8350 | server and client, §6.3 |
| 1 | `aim_pitch` | −80.8630 … 49.9848 | −76.6503 … 50.7197 | server and client, §6.2 |
| 2 | `move_yaw` | −180 … 180 | −180 … 180 | server and client, §6.1 |
| 3 | `body_yaw` | −29.7342 … 29.7343 | same | nobody: stays 0 |
| 4 | `spine_yaw` | −30.7085 … 30.7085 | same | nobody: stays 0 |
| 5 | `head_yaw` | −44.8190 … 44.8187 | same | client only, §6.4 |
| 6 | `head_pitch` | −20.0000 … 23.0000 | same | client only, §6.4 |
| 7 | `head_roll` | −10.9314 … 10.8524 | same | nobody: stays 0 |

None loops. A value outside its range is clamped by the pose encoding
(animation.md §5).

#### 1.3 Materials

Citizens: a face (VertexLitGeneric), `citizen_sheet` (VertexLitGeneric),
two eyeballs with the **`eyes`** shader (e.g. `dark_eyeball_r`,
`eyeball_l`; keys `$basetexture`, `$iris`, `$halflambert` 1) and a mouth
with the **`Teeth`** shader (`mouth` or `fmouth`; keys `$basetexture`,
`$model` 1, `$halflambert` 1). The model's material folders are searched
in their stored order and the first hit wins (`models/Humans/Male/Group03/`
before `models/Humans/Male/`), so a group03 face comes from the group03
folder and the eyes and mouth from the shared folder. Combine and police
use VertexLitGeneric only. All player materials use `$halflambert` 1.

### 2. The animation model's sequences

All at 30 frames/s except `range_melee` (45 frames/s). Duration =
(frames − 1)/fps. "Kind" is one of: `pistol`, `shotgun`, `smg1`, `ar2`,
`gravgun`, `grenade`, `rpg`, `crossbow`, `melee`, `slam`.

| Merged index | Name | Activity | Loops | Frames | Duration (s) | Blend | Notes |
|---|---|---|---|---|---|---|---|
| 1 | `reference` | – | no | 101 | – | – | fallback pose source only |
| 2–16 | aim matrices (`police_WalkN_pistol_aim_matrix`, `weapons_Pistol_aim_matrix`, `HL2DM_crouch_aim_pistol`, `soldier_Aim_9_directions`, `…_SG`, `soldier_Crouch_aim_9_directions`, `weapons_Aim_9_directions`, `…_Alert`, `weapons_RunN_SMG1_Aim_9_directions`, `weapons_WalkN_SMG1_Alert_Aim_9_directions`, `weapons_Aim_RPG_9_directions`, `HL2DM_crouch_aim_RPG`, `HL2DM_Aim_Gravgun`, `HL2DM_Run_Aim_Gravgun`, `HL2DM_crouch_aim_gravgun`) | – | no | 1 | – | 3×3 on `aim_yaw` × `aim_pitch` | hidden, additive (delta + post), legs weight 0 |
| 17–26 | `idle_<kind>` | `ACT_HL2MP_IDLE_<KIND>` | yes | 30 | 0.9667 | – | autolayer: an aim matrix |
| 27–46 | `run_<kind>_mod`, `run_<kind>` (pairs) | `run_<kind>`: `ACT_HL2MP_RUN_<KIND>` | `run_<kind>` yes | 19 | 0.6000 | `run_<kind>`: 9 cells on `move_yaw` | autolayers: `run_<kind>_mod` (hidden, non-additive, legs weight 0), then an aim matrix |
| 47–56 | `cidle_<kind>` | `ACT_HL2MP_IDLE_CROUCH_<KIND>` | yes | 30 (grenade, melee, slam: 1) | 0.9667 (one-frame ones: 0) | – | autolayer: a crouch aim matrix, except grenade, melee, slam (none) |
| 57–76 | `cwalk_<kind>_mod`, `cwalk_<kind>` (pairs) | `cwalk_<kind>`: `ACT_HL2MP_WALK_CROUCH_<KIND>` | yes | 31 | 1.0000 | 9 cells on `move_yaw` | autolayers: `cwalk_<kind>_mod`, then an aim matrix |
| 77–86 | `range_<kind>` | `ACT_HL2MP_GESTURE_RANGE_ATTACK_<KIND>` | no | pistol 9, shotgun 30, smg1 5, ar2 5, gravgun 15, grenade 25, rpg 21, crossbow 46, melee 19 @45, slam 16 | 0.2667, 0.9667, 0.1333, 0.1333, 0.4667, 0.8, 0.6667, 1.5, 0.4, 0.5 | – | additive (delta + post), legs weight 0 |
| 87–96 | `jump_<kind>` | `ACT_HL2MP_JUMP_<KIND>` | no | 27 | 0.8667 | – | full body |
| 97–100 | `reload_pistol`, `reload_shotgun`, `reload_smg1`, `reload_ar2` | `ACT_HL2MP_GESTURE_RELOAD_<KIND>` | no | 41, 66, 66, 66 | 1.3333, 2.1667, 2.1667, 2.1667 | – | **non-additive** overlay; weight 0 on legs, 0.1 pelvis, 0.2 `Spine`, 0.4 `Spine1`, 0.6 `Spine2`, 1 above |
| 101–105 | `body_rot_z`, `spine_rot_z`, `head_rot_z`, `head_rot_y`, `head_rot_x` | – | no | 1 | – | 3 cells on `body_yaw`, `spine_yaw`, `head_yaw`, `head_pitch`, `head_roll` | hidden, additive, **autoplay** (§7) |

(The order of kinds within each group is pistol, shotgun, smg1, ar2,
gravgun, grenade, rpg, crossbow, melee, slam; e.g. `idle_gravgun` is merged
21, `run_gravgun` 36, `run_smg1` 32, `jump_ar2` 90, `range_pistol` 77.)

- The 9-cell `move_yaw` grids have pose keys −180, −135, −90, −45, 0, 45,
  90, 135, 180 holding `a_RunS`, `a_RunSE`, `a_RunE`, `a_RunNE`, `a_RunN`,
  `a_RunNW`, `a_RunW`, `a_RunSW`, `a_RunS` (crouch: the `a_Crouch_walk*`
  set in the same order). So `move_yaw` −90 is running to the right (E),
  +90 to the left (W).
- **Missing on purpose**: no sequence for the untranslated activities
  `ACT_HL2MP_IDLE`, `ACT_HL2MP_RUN`, `ACT_HL2MP_IDLE_CROUCH`,
  `ACT_HL2MP_WALK_CROUCH`, `ACT_HL2MP_JUMP`; no walk, swim, ladder, turn,
  land or death sequence; reload gestures exist only for pistol,
  shotgun, smg1 and ar2. No sequence carries animation events.
- Every sequence's fade-in and fade-out time is 0.2 s.

### 3. Weapon kind (activity translation)

Each weapon maps the generic activities (idle, run, crouch idle, crouch
walk, jump, fire gesture, reload gesture) to its kind's. The mapping is a
fixed table per weapon class in code; the weapon script's `anim_prefix`
plays **no part** in the player's animation.

| Weapon | Kind | Fire gesture triggered by | Reload gesture |
|---|---|---|---|
| `weapon_pistol` | pistol | each shot | `reload_pistol` |
| `weapon_357` | pistol | each shot | `reload_pistol` |
| `weapon_smg1` | smg1 | each bullet shot; the grenade launch | `reload_smg1` |
| `weapon_ar2` | ar2 | each bullet shot; **not** the energy ball | `reload_ar2` |
| `weapon_shotgun` | shotgun | primary and double shot | none (its shell-by-shell reload never asks for one) |
| `weapon_crossbow` | crossbow | **never** | none (no `reload_crossbow` sequence) |
| `weapon_rpg` | rpg | the launch | none (no sequence) |
| `weapon_frag` | grenade | throw, lob and roll | none |
| `weapon_slam` | slam | throw, tripmine placement, detonation | none |
| `weapon_crowbar`, `weapon_stunstick` | melee | each swing (hit or miss) | none |
| `weapon_physcannon` | gravgun | **never** (punt and pull play none) | none |
| no weapon | untranslated | – | – |

A request whose translated activity has no sequence falls back as in
§4 (main sequence) or is dropped (gesture, §8).

### 4. Main sequence choice (server)

The server keeps a current "ideal activity" (untranslated) and the main
sequence. A request is one of: *idle-or-moving*, *jump*, *fire*,
*reload* (fire and reload are gestures, §8). Requests are made:

- **every tick** at the end of the weapon step (post-think): if the
  horizontal velocity is exactly 0 → idle-or-moving; else if on the
  ground → idle-or-moving; else if the water level is above 1 (waist) →
  idle-or-moving; else (moving in the air, not swimming) **no request**;
- by the movement when a jump starts (server side only) → jump;
- by the movement on landing at ≥ 303 units/s (the HL2 punch threshold;
  H0 1.1) → idle-or-moving (it changes nothing the post-think request
  would not);
- on spawn and on every weapon switch: the main sequence and activity are
  cleared, then one request is made from the state (velocity 0 →
  idle-or-moving; moving on the ground or in water → idle-or-moving; in
  the air → jump; else idle-or-moving). So spawning or switching weapons
  in mid-air shows the jump sequence.

Handling a request, with speed = |horizontal velocity|:

1. If the player is frozen (intermission) or "at controls": speed = 0 and
   the request becomes idle-or-moving.
2. Jump → activity `ACT_HL2MP_JUMP`.
3. Idle-or-moving → if not on the ground **and** the current activity is
   `ACT_HL2MP_JUMP`, keep it (still jumping). Otherwise: fully ducked (the
   ducked flag, set when the 0.4 s duck completes, movement.md) → speed > 0
   ? `ACT_HL2MP_WALK_CROUCH` : `ACT_HL2MP_IDLE_CROUCH`; else speed > 0 ?
   `ACT_HL2MP_RUN` : `ACT_HL2MP_IDLE`. Any non-zero speed counts (movement
   zeroes velocities under 1 unit/s on the ground).
4. The activity is stored, translated by the weapon in hand (§3) and a
   sequence with that activity chosen (one each; the weighted random choice
   of animation.md §12.1 always gives it). If there is none, the
   untranslated activity is tried (never exists), then merged sequence 0
   (`ragdoll`) is used.
5. If that sequence is already the main sequence, nothing changes (the
   cycle runs on). Otherwise the main sequence becomes it with **cycle 0
   and playback rate 1**, even between two looping sequences (unlike CS:S
   animation.md §9, which keeps the phase).

Consequences:

- Jumping holds `jump_<kind>` (cycle advancing, clamped at 1, holding the
  last frame) until the first tick that ends on the ground; then idle or
  run starts at cycle 0. A standing jump (35 ticks airborne, H0 test 1)
  reaches cycle ≈ 0.59, so the end of the jump sequence is never seen on
  flat ground.
- Walking off a ledge while moving keeps the run sequence playing in the
  air; dropping straight down (no horizontal speed) shows the idle.
- Swimming and climbing ladders use the same idle/run choice (no swim or
  climb sequence): a vertical ladder climb has horizontal speed 0 → idle;
  swimming with horizontal speed → run.
- `+walk` makes no difference to the sequence (no walk sequence): the run
  plays slower.
- A ducking player keeps standing sequences for the 0.4 s of the duck
  transition.
- With no weapon at all (only after every weapon is lost) the body shows
  `ragdoll`, the model's reference pose.
- Death requests nothing: the body is replaced by the ragdoll (§10).

### 5. Playback rate and cycle

After the request handling and the frame advance of each tick, the
server sets the main sequence's playback rate for the next advance:

```math
r = \begin{cases} \dfrac{s}{G} & s > 0.5 \text{ and } G > 0 \\ 1 & \text{otherwise} \end{cases}
```

s = horizontal speed, G = the main sequence's ground speed at the current
pose parameters: the length of its blended movement over one cycle ÷ its
duration. **No clamp** (CS:S's template clamps to 0.01–10). The blended
movement is the weighted sum of the grid cells' movement vectors with
the same cell weights as the cycle rate (animation.md §9) — *inferred*,
the blending helper is engine code (Open questions 2). Sequences without
movement (idle, jump) have G = 0 → r = 1.

Per tick the main cycle advances by dt · cycle rate · r (animation.md §9),
where for the run sequences the cycle rate is 1/0.6 = 1.6667 cycles/s,
crouch walk 1, idle 1/0.9667 = 1.0345, jump 1/0.8667 = 1.1538.

| Situation (male or female) | s | Cell | G | r | Cycle per tick |
|---|---|---|---|---|---|
| run forward | 190 | N | 209.7902 | 0.90567 | 0.022642 |
| sprint forward | 320 | N | 209.7902 | 1.52533 | 0.038133 |
| `+walk` forward | 150 | N | 209.7902 | 0.71500 | 0.017875 |
| run backward | 190 | S | 169.8302 | 1.11876 | 0.027969 |
| run right or left | 190 | E/W | 149.8501 | 1.26793 | 0.031698 |
| run forward-right | 190 | NE | 209.7868 | 0.90568 | 0.022642 |
| halfway N–NE (`move_yaw` −22.5) | 190 | N+NE | 193.8205 | 0.98029 | 0.024507 |
| crouch walk | 63.33 | any | 60.0 | 1.05556 | 0.015833 |
| crouch with sprint held | 106.67 | any | 60.0 | 1.77778 | 0.026667 |

The playback rate and cycle are networked; clients do not advance the
main cycle themselves, they interpolate the server's (§6.6).

### 6. Pose parameters

#### 6.1 `move_yaw` (legs direction)

A gait yaw g is kept per player:

- if the horizontal velocity is exactly 0: g ← g + e·k, where e =
  normalize(body yaw − g) and k = 4·dt if dt < 0.25, else dt; then g is
  reduced to (−360, 360) by truncation;
- else g = atan2(v_y, v_x) in degrees.

```math
\text{move\_yaw} = \operatorname{normalize}\big(-(\psi - g)\big)
```

ψ = the body yaw: on the server the view yaw of this command (normalized
to [−180, 180]); on a client the entity's networked yaw (the server's
feet yaw, §6.3). While moving the two are equal (the feet follow the
eyes), so `move_yaw` = gait yaw − view yaw: running straight ahead 0,
backpedalling ±180, strafing right −90, left +90.

#### 6.2 `aim_pitch`

aim_pitch = eye pitch, with pitch > 180 reduced by 360, clamped to
[−90, 90], then by the pose range (male [−80.86, 49.98]). Positive =
looking down. On the server the eye pitch used is the one stored at the
end of the **previous** command (one tick late); on clients it is the
networked eye pitch (8 bits over [−90, 90]: steps of 180/255 = 0.706°)
for other players.

#### 6.3 `aim_yaw`, the feet and the render yaw

State: feet yaw F, goal yaw Γ, last-turn time T (0 = "was moving"),
last eye yaw Y_l. Eye yaw Y (on the server the previous command's, as
§6.2; on clients the networked one, 10 bits: 0.352° steps). Each update,
with now and dt:

**Moving** (3D speed > 1 unit/s): T = 0; F = Γ = Y; torso = 0.

**Standing**:

1. If T ≤ 0 (just stopped): T = now, Y_l = Y, F = Γ = Y.
2. If Y ≠ Y_l: T = now, Y_l = Y.
3. If Γ ≠ F: T = now.
4. Converge F toward Γ: d = normalize(Γ − F); a = |Γ − F| taken
   **before** normalizing; scale = clamp(a/45, 0.01, 1) if a ≤ 45 else 1;
   step = 360·dt·scale; if |d| < step, F = Γ, else F moves by step toward
   Γ (sign of d); F normalized.
5. δ = normalize(Y − F). If |δ| > 45, or now > T + `mp_facefronttime`
   (3 s): Γ = Y and T = now (the feet start turning next update).
6. torso = δ (the step-5 change of Γ does not affect this update).

aim_yaw = clamp(torso, −60, 60), then the pose range (male [−49.44,
52.52], female [−50.86, 56.83]) clamps further.

Render and hitbox yaw = F, pitch and roll 0 (the model never tilts; pitch
lives in `aim_pitch`). On the server the entity's angles are set to
(0, F, 0) **before** step 4 runs, so the server's body yaw (and its
hitboxes) is the feet yaw of the previous update: one tick behind while
the feet turn.

Consequences: standing, the upper body follows the view up to 45° each
way with the feet planted; past 45° the feet swing round at up to 360°/s
(slowing in the last 45°: each server tick removes 12 % of the remaining
gap, 0.12 = 360·0.015/45); 3 s after the last turn the feet face the view.
There is no turn-in-place sequence (the turning flag never becomes
set; `mp_ik` has no effect).

#### 6.4 Head and eyes (clients only)

Each frame, for every player entity on a client:

1. **Look target**: going through players in index order (dead ones and
   the local player included, the player itself excluded), the first
   whose origin is within 128 units of this player's origin and not
   behind it (dot of this player's forward, from its (0, F, 0) angles,
   with the direction ≥ 0) gives the target: that player's eye position.
   Otherwise the target is origin + 512·forward.
2. The eyes aim at the target (§11.3).
3. **Blink timer**: when it has run out, the blink toggles and the timer
   restarts with U(1.5, 4.0) s (§11.2).
4. **Head yaw**: desired = normalize(yaw of (target − eye position) − F),
   clamped to the `head_yaw` range (±44.82); the current head yaw moves
   toward it by at most 130·dt degrees (shortest way); then the change of
   F since the last frame is subtracted (the head counter-rotates while the
   body turns) and the result normalized and stored in `head_yaw`.
5. **Head pitch**: desired = normalize(pitch of (target − eye position)),
   clamped to [−20, 23]; current moves toward it by at most 130·dt;
   stored in `head_pitch`.

The server never sets `head_yaw`/`head_pitch` (they stay 0 for its
hitboxes), and the head pose is not networked; each client computes its
own. All 19 models get this (Combine and police heads turn too; only
citizens have eyes and lids to show it).

#### 6.5 Initial values

On a new model (spawn, model change) a client sets every pose parameter
to 0 (value, not normalized), the feet state to "was moving" (T = 0) and
the gait yaw to 0. `body_yaw`, `spine_yaw`, `head_roll` stay 0 forever.

#### 6.6 What is networked

| Item | Networked? | Notes |
|---|---|---|
| main sequence, cycle, playback rate | yes | interpolated on clients |
| overlay layers (sequence, cycle, weight, order) | yes | gestures, §8 |
| entity angles | yes | (0, F_server, 0) |
| eye angles | yes | pitch 8 bits in [−90, 90], yaw 10 bits; full precision only in the local player's own prediction |
| pose parameters | **no** | each client runs §6.1–§6.4 itself every frame from interpolated data and its own velocity estimate (origin change between frames) |
| flex weights | yes | all 0 for players (§11.1) |
| eye look target, blink | **no** | client side (§6.4) |
| origin | yes | high precision for the local player, low for others (H0) |

A client's render yaw for another player is its **own** F (from §6.3 run
at frame rate on networked eye yaw), not the networked angles. The
server's F (hitboxes) and the client's F agree up to timing.

### 7. Building the body

Bone setup, per animation.md §6–§10, in this order:

1. Model defaults.
2. Main sequence at its cycle, weight 1, with its autolayers in file
   order: for idle and crouch-idle the aim matrix; for run and crouch
   walk first `<run|cwalk>_<kind>_mod` (a non-additive one-pose upper
   body at full weight, legs untouched), then the aim matrix. All these
   autolayers have start = peak = tail = end = 0 and flags 0: full weight,
   parent's cycle (animation.md §7).
3. On clients only, the outgoing main sequences cross-fading out over
   0.2 s (animation.md §9). The server's bone setup (hitboxes) uses the
   current main sequence only.
4. Overlay layers by order (§8).
5. The five autoplay sequences (`body_rot_z`, `spine_rot_z`,
   `head_rot_z`, `head_rot_y`, `head_rot_x`): additive, weight 1, applied
   to every model every frame regardless of the main sequence, driven by
   their pose parameters (`body_yaw`, `spine_yaw`, `head_yaw`,
   `head_pitch`, `head_roll`). Their animations are one frame, so the
   cycle doesn't matter. With the pose at 0 they add nothing
   (*inferred*: the middle cell of a 3-cell additive grid is the neutral
   pose; Open questions 3).
6. Entity transform: origin, angles (0, F, 0).

IK is off for players on clients and HL2:DM's models have no IK rules
the game uses.

### 8. Gestures (fire and reload)

- Server only. A fire request translates `ACT_HL2MP_GESTURE_RANGE_ATTACK`
  by the weapon (§3); a reload request `ACT_HL2MP_GESTURE_RELOAD`.
  (HL2's single-player exception for hovering, swimming or leaping
  activities never applies: HL2:DM never sets them.)
- If an active, not-dying overlay layer already plays that activity, its
  cycle is set to 0 (restart). Otherwise a new layer is added if the
  model has a sequence for the activity at merged index > 0; if not, the
  request is silently dropped (crossbow, RPG, grenade, slam, gravgun and
  melee reloads; no-weapon gestures).
- A new layer: cycle 0, playback rate 1, weight 1, order after every
  other active layer, loops if the sequence loops (none do), removed when
  done. Each tick the layer's cycle advances by dt · cycle rate (1/
  duration); when it reaches 1 the layer's weight drops to 0 and it is
  removed on the next advance. Weight stays 1 until then (no blend in or
  out).
- Spawn sets the player up with three empty layer slots; more are added
  on demand (up to 15). Death clears all layers.
- The fire request comes from the weapon code in the weapon step, before
  that tick's frame advance, so the gesture is already advanced one tick
  on the tick it starts: `range_pistol` (3.75 cycles/s, 0.05625 a tick)
  ends on its 18th advance (cycle 1.0125); `range_smg1` and `range_ar2`
  (7.5 cycles/s) would end on the 9th but full-auto fire restarts them
  every 5 ticks (SMG1 0.075 s) or 7 (AR2 0.1 s, rounded per H3), so
  holding the trigger replays their first part.
- Fire gestures are additive on the upper body (legs weight 0) and do not
  replace the aim; the reload overlay replaces the arms and blends into
  the spine (§2 weights).
- Layers are networked and interpolated; clients do not predict them
  (the local player never sees its own body in first person).
- There is no jump or land gesture (jumping is the main sequence, §4).

### 9. Hitboxes and hitgroups

Each model has one hitbox set (`default`). Each box is an oriented box in
the named bone's space: min and max corners in units.

Hitgroup numbers: 0 generic, 1 head, 2 chest, 3 stomach, 4 left arm, 5
right arm, 6 left leg, 7 right leg. Damage scale per group:
`sk_player_head` (3), `sk_player_chest`, `_stomach`, `_arm`, `_leg` (1),
generic ×1 (weapons.md §6). In practice: head ×3, everything else ×1.

**Male citizens (all nine identical) and the shared bone list:**

| Bone (`ValveBiped.Bip01_…`) | Group | Min | Max |
|---|---|---|---|
| `Head1` | 1 | (−1.25, −6.50, −3.19) | (8.25, 3.50, 3.31) |
| `L_UpperArm` | 4 | (0, −2.75, −2.75) | (12.51, 1.75, 2.25) |
| `L_Forearm` | 4 | (0, −2.19, −2.38) | (13.00, 1.81, 1.62) |
| `L_Hand` | 4 | (0.06, −2.00, −1.50) | (4.06, 1.00, 2.50) |
| `R_UpperArm` | 5 | (0, −3.00, −2.25) | (12.39, 2.00, 2.75) |
| `R_Forearm` | 5 | (−0.50, −2.20, −1.55) | (12.50, 1.80, 2.45) |
| `R_Hand` | 5 | (0.06, −2.00, −2.50) | (4.06, 1.00, 1.50) |
| `L_Thigh` | 6 | (0, −3.75, −3.25) | (17.85, 3.75, 3.25) |
| `L_Calf` | 6 | (0, −3.51, −3.28) | (15.64, 3.49, 2.72) |
| `L_Foot` | 6 | (−1.07, −2.00, −2.84) | (5.04, 5.00, 2.16) |
| `L_Toe0` | 6 | (−0.50, −3.00, −2.19) | (2.50, 0, 2.46) |
| `R_Thigh` | 7 | (0, −3.75, −3.25) | (17.85, 3.75, 3.25) |
| `R_Calf` | 7 | (0, −3.51, −2.82) | (15.64, 3.49, 3.18) |
| `R_Foot` | 7 | (−1.06, −2.01, −2.28) | (5.05, 4.99, 2.72) |
| `R_Toe0` | 7 | (−0.50, −3.00, −2.60) | (2.50, 0, 2.10) |
| `Pelvis` | 3 | (−7.50, −5.50, −6.00) | (7.50, 8.50, 5.00) |
| `Spine2` | 2 | (−2.50, −2.50, −7.00) | (14.50, 7.50, 7.00) |

(values rounded to 0.01 here; implementations read the model.)

**Female citizens (all six identical)**: 18 boxes in the same order with
their own sizes (head (−2.25, −5.00, −3.15)–(7.25, 5.00, 3.35)), plus a
`Spine1` box in group **3** between `Pelvis` (3) and `Spine2` (2).

**Combine (all three identical)**: 18 boxes: `Spine` group **2** first,
then head (−1.75, −8.00, −3.50)–(7.25, 4.00, 3.50), arms 4/5, legs 6/7, then
**`Pelvis` and `Spine2` in group 0 (generic)**. The Combine have no
stomach box and their upper chest is "generic".

**Metro police**: 17 boxes, groups as the male citizens (pelvis 3,
`Spine2` 2), own sizes (head (−3.50, −7.71, −3.85)–(7.50, 5.71, 3.96)).

Server side:

- Bullets (and the shotgun's box pellets, weapons.md Q5) are traced
  against these boxes as posed by the **server's** bone setup: the main
  sequence and cycle, the gesture layers, the pose parameters `move_yaw`,
  `aim_pitch`, `aim_yaw` from §6 (head ones 0), angles (0, F_server, 0),
  origin. The nearest box along the ray wins; its group scales the
  damage.
- Lag compensation (weapons.md §12) rewinds each compensated player's
  origin, angles, collision box, main sequence and cycle and overlay
  layers to the shooter's view time; **pose parameters are not rewound**:
  the current `aim_pitch`/`aim_yaw`/`move_yaw` are used with the old
  position.
- Melee uses the collision box and never a hitbox (weapons.md §11).
- A ducked player's collision box is (−16, −16, 0)–(16, 16, 36) as soon as
  the ducked flag is set (H1); hitboxes follow the crouch sequences.

Clients use the same boxes only for their own effects (blood, ragdoll
pushes); the client's head pose and cross-fades make its picture differ
slightly from the server's boxes.

### 10. Ragdolls (`hl2mp_ragdoll`)

Mechanism as [ragdolls.md](../cs_source/ragdolls.md) (server record,
client-built ragdoll, seed poses, joints, impulse distribution, bone
velocities, collisions, settling, separation repair). HL2:DM's
differences:

#### 10.1 The record

Created on the server at the moment of death (before scoring and the
death sound), class `hl2mp_ragdoll`, sent to **every** client regardless
of visibility. Fields: the dead player (handle), origin (1/32 unit
precision), velocity, model, hit physics bone, force. Values:

- origin and velocity: the player's at death;
- model: the player's current model;
- hit physics bone: **always 0** for players (only NPCs record the bone
  of the hit); so the killing force always goes into body 0, the pelvis;
- force: the sum of the damage forces the player received since the
  start of its last command (bullets: ammo impulse × `phys_pushscale`
  along the shot, summed over pellets; melee damage × 300; blasts their
  own; weapons.md §6, §11, H4);
- **no death pose and no death frame** (ragdolls.md 2.3 does not apply:
  the B1 pose is the normal animated pose).

#### 10.2 On the client

- When the record arrives and the dead player is known and not dormant:
  the player's model instance (with its decals) moves to the ragdoll; for
  another player the ragdoll copies its interpolation history, render
  angles, sequence and playback rate; for the local player it is placed
  at the record's origin with the render angles, the record's velocity and
  the player's sequence at cycle 0, history reset. The three bone sets are
  taken from the player entity (bone dt 0.05 s, ragdolls.md 2.2).
- Otherwise (dormant or unknown player) it is placed at the record's
  origin with reset history; bone velocities are 0.
- Bodies: the model's `.phy`: 15 bodies (`Pelvis`, `Spine2`, upper arms,
  forearms, hands, thighs, calves, `Head1`, feet), 14 joints, 13
  collision pairs, surface `flesh` (police also `no_decal` bodies); root
  `ValveBiped.Bip01_Pelvis`, with `Bip01` and `Spine1` merged into it.
  Masses (kg):

| Body | Male citizen | Female citizen | Combine (all) | Police |
|---|---|---|---|---|
| Pelvis (body 0) | 12.7749 | 12.5099 | 11.0025 | 9.3023 |
| Spine2 | 24.3659 | 19.2201 | 22.4570 | 17.7393 |
| Head1 | 5.1694 | 5.1181 | 7.9935 | 10.0779 |
| Total | 90.0 | 70.65 | 100.0 | 90.0 |

- Death impulse (ragdolls.md 2.5 with k = 0): the pelvis gets F at its
  mass centre (Δv = F/m_pelvis); every other body gets (m_j/M)·F applied
  at the pelvis body's origin (Δv = F/M, plus a spin about its own mass
  centre). A head-shot does not snap the head back; the whole body is
  shoved from the hips.
- Bone velocities are added on top (ragdolls.md 2.6): a running player's
  ragdoll keeps running speed.
- The non-simulated bones take the model's `ragdoll` sequence
  (`ACT_DIERAGDOLL`, merged 0).
- Face: every flex weight of the ragdoll is forced to 0 each frame (no
  blinking, neutral face, §11.1 "rest" without the blink) and the eyes look
  1000 units straight ahead along the `eyes` attachment.
- Later bullets: as ragdolls.md 6.2 (4000 along the shot on body 0;
  blasts 4000 × the unnormalized trace) and the 5 s settle timer
  (`ragdoll_sleepaftertime`) restarts.
- Collisions and `cl_ragdoll_collide` 0: as ragdolls.md §4.
- Gravity: `sv_gravity` 600 (H0).
- The dead player entity is not drawn (it keeps being sent so its
  position is known); the dead view looks at this ragdoll (rules.md).

#### 10.3 Lifetime

- One per player. A player's ragdoll stays on the map **after the player
  respawns** and is removed only when that player dies again (the old
  record is deleted immediately and a new one made), when the player
  leaves, on `mp_restartgame` (the map clean-up removes it) or at map
  change.
- No fade: `g_ragdoll_maxcount` and the fade speeds apply only to purely
  client-side ragdoll copies, which HL2:DM's players never make.
- A kill whose damage dissolves (the AR2's energy ball) dissolves the
  ragdoll (and mutes the death sound): H4 and H9 own the dissolve.

### 11. Faces: flexes, blinking, eyes and teeth

#### 11.1 Flex controllers at rest

Citizens have 54 controllers (all ranges [0, 1] except: `head_rightleft`
±30, `head_updown` ±15, `head_tilt` ±15, `eyes_updown` ±30,
`eyes_rightleft` ±30, `body_rightleft` ±30, `chest_rightleft` ±30,
`head_forwardback` ±0.2, `gesture_updown` ±1, `gesture_rightleft` ±1),
71 flex descriptors and 63 flex rules. Metro police have the last 10 of
those (head, eyes, body, chest, gesture) and no descriptors or rules, so
nothing on them moves. Combine have none.

- Players never play scenes or speak with visemes in HL2:DM. Every
  controller's value is 0 (stored normalized as (0 − min)/(max − min):
  0 or 0.5), networked in 12 bits rounded down (the ±ranges therefore
  arrive as about −0.0002 of their span; negligible).
- On clients the controllers go through the model's flex rules each frame
  (`flex_rules` 1). At rest the only non-zero descriptors are
  `upper_right_neutral`, `lower_right_neutral`, `upper_left_neutral`,
  `lower_left_neutral` = 1 (eyelids in their neutral position); the
  mouth, brows and the rest are 0.
- `flex_smooth` 1 smooths flex changes over time (only the blink changes).

#### 11.2 Blinking (clients)

The blink toggle flips every U(1.5, 4.0) s (§6.4). On each flip the blink
ends at t_e = now + `blink_duration` (0.2 s). Each frame, with
u = (t_e − now)·(π/2)/0.2:

```math
\text{if } u > 0:\ c = \cos u;\ \text{if } 0 < c < 1:\ b = 2\sqrt{c},\ b \leftarrow 2 - b \text{ if } b > 1;\ \text{blink} = \operatorname{clamp}(\text{blink} + b, 0, 1)
```

So the lid shuts in 32 ms (b = 1 at c = 0.25, 0.0322 s after the flip)
and opens over the remaining 168 ms. With blink = 1 the rules give
`upper_right_lowerer` = `upper_left_lowerer` = 1, the lower lids half
`neutral` half `raiser` (0.5 each) and the upper `neutral` 0.

#### 11.3 Eye aim (clients)

The eyes' look target (§6.4 step 1) is taken into the `eyes` attachment's
space (forward = +x). If its x < 6 it is set to 6. Its direction ℓ
(unit) then gets the eye flex deflection (`eyes_updown`,
`eyes_rightleft`: 0 at rest, so none). If ℓ_x < 0.866 (more than 30° off
forward) it is pulled back onto the 30° cone: ℓ_x = 0.866 and (ℓ_y, ℓ_z)
scaled to length 0.5. The point at the original distance along ℓ is
handed to the eye renderer as the eyes' target. Models without the
attachment (none of the 19) would aim straight at the target.

#### 11.4 The Eyes shader

Each eyeball (two per citizen: radius 0.5, iris scale 1.4706 for men
(1/0.68), 1.5152 for women (1/0.66), offset ±0.07, on `Bip01_Head1`)
is drawn with its material's `eyes` shader:

- Lighting per vertex as VertexLitGeneric (shaders.md §4; `$halflambert`
  1), but with the **normal replaced** by the direction from the eye
  centre E to the vertex, flattened along the eyeball's up vector: n =
  normalize(p − E − ½((p − E)·û)û) with û the half-length up vector the
  renderer supplies.
- Iris texture coordinates: a planar projection of the world position,
  u = P_u·(p, 1), v = P_v·(p, 1), with the two projection planes built by
  the renderer from the eye centre, the look direction (§11.3) and the
  iris scale (Open questions 4).
- Glint texture coordinates the same way from two glint planes; the glint
  texture is generated by the engine (`r_glint_procedural`).
- Colour = lerp(base, iris.rgb, iris.a) × vertex light + glint × g, where
  g = 1 if the ambient-cube luminance > 0.01, else smoothly to 0
  (luminance × spline(luminance/0.01)); alpha = base alpha. Fog as other
  models.
- `$dilation` is unused in the DX9 path.

#### 11.5 The Teeth shader

- Vertex lighting as VertexLitGeneric **without** half-Lambert, then
  multiplied by `$illumfactor` × saturate(n · `$forward`), where the
  engine sets `$forward` to the mouth's forward (the model's mouth entry:
  bone `Bip01_Head1`, direction (0, 1, 0) in that bone's space) in world
  space and `$illumfactor` from the opening of the `mouth` descriptor
  (Open questions 5). Colour = base × that light; alpha = base alpha.
- At rest the `mouth` descriptor is 0 (§11.1).

### 12. View models

#### 12.1 What differs from view_models.md

- **No handedness**: HL2's view models are drawn as stored, right-handed.
  There is no `cl_righthand`; nothing is mirrored (view_models.md §2 does
  not apply).
- The hands and arms are part of each `v_` model (no separate arms
  model).
- **Bob**: HL2:DM's own (12.2), not the cvar-driven one of view_models.md
  §3.
- **Sway**: HL2's spring lag plus pitch offsets (12.3), not the 0.1 s
  history sway of view_models.md §4 (`sv_wpn_sway_pred_legacy` is 0 in
  HL2 builds; `cl_wpn_sway_interp` and `cl_wpn_sway_scale` do nothing).
- FOV: as view_models.md §5 with HL2:DM's base FOV (12.4).
- Placement start, shake (10 %), the depth-range pass, attachment
  re-projection and lighting: as view_models.md §1, §5, §9. Muzzle flash
  and shell events: H9.
- Bob and sway are skipped while the client re-runs prediction (placement
  happens once per rendered frame).

#### 12.2 Bob

One global state on the client (shared by all weapons, never reset): bob
time T_b and last time t_l, both starting at 0. Each frame (skipped when
the frame time is 0):

```math
s = \min\!\big(\lvert v_{xy} \rvert, 320\big), \qquad T_b \mathrel{+}= (t - t_l)\,\frac{s}{320}, \qquad t_l = t
```

v = the local player's (predicted) velocity. No slew limit. Then

```math
V = \operatorname{clamp}\!\Big(0.005\,s\,\big(0.3 + 0.7\sin\tfrac{2\pi T_b}{0.45}\big), -7, 4\Big), \qquad
L = \operatorname{clamp}\!\Big(0.005\,s\,\big(0.3 + 0.7\sin\tfrac{2\pi T_b}{0.9}\big), -7, 4\Big)
```

(The general cycle shape of view_models.md §3 with P = 0.45 and U = 0.5
reduces to these sines; HL2:DM's lateral phase is computed with a
mis-grouped modulo that still gives exactly sin(2πT_b/0.9).) With f and r
the forward and right vectors of the view-model angles before this step:

- origin += f · 0.1V; origin.z += 0.1V (world up)
- roll += 0.5V; pitch −= 0.4V; yaw −= 0.3L
- origin += r · 0.8L

The vertical cycle repeats every 0.45 · 320 = 144 units travelled, the
lateral every 288. In the air the bob keeps cycling at the horizontal
speed.

#### 12.3 Lag (sway) and pitch offsets

After the bob, per frame (frame time dt):

1. f = forward vector of the view-model angles (after the bob). State:
   λ, the lagging forward (a unit vector; starts as the zero vector).
2. If dt ≠ 0: Δ = f − λ; k = 5, and if |Δ| > 1.5, k = 5·|Δ|/1.5;
   λ ← normalize(λ + k·dt·Δ); origin ← origin − 5·Δ (with the Δ computed
   before λ moved).
3. Pitch offsets (`sv_viewmodel_lag_do_angles` 1), with the **eye**
   angles (before the bob): p = eye pitch normalized to [−180, 180];
   f_e, r_e, u_e its forward, right, up vectors:

```math
\text{origin} \mathrel{+}= -0.035\,p\,\hat f_e - 0.03\,p\,\hat r_e - 0.02\,p\,\hat u_e
```

Effect: turning by δ in one frame shifts the gun opposite the turn by
5 · 2 sin(δ/2) units, decaying by about 5·dt of itself each frame
(frame-rate dependent: plain Euler steps). Looking down 30° moves it
1.05 back, 0.9 left and 0.6 down (in view axes); looking up the
opposite. The first frame of a new view model, with λ = 0, has Δ = f and
pops the gun 5 units back for that frame (λ becomes f).

#### 12.4 FOV

- Base FOV: the player's `fov_desired` clamped by the server to 75–90
  (default 75; H0); the client's `default_fov` follows it each frame.
- View-model FOV = `viewmodel_fov` (54) − (base FOV − current FOV): 54
  whenever not zoomed, whatever the base; zooms (suit zoom, crossbow:
  H2, H4) shrink it by the same number of degrees. Then the 4:3-to-aspect
  conversion of view_models.md §5 (the view model always with the real
  aspect).

### 13. Attachments used by other systems

| Attachment | On | Used for |
|---|---|---|
| `eyes` | all 19 (`Bip01_Head1`) | eye aim (§11.3), ragdoll eyes |
| `anim_attachment_RH` | all (via the animation model) | the third-person flashlight beam of other players (H2/H9) |
| `anim_attachment_LH`, `forward` | animation model | not used by HL2:DM's player code |
| `anim_attachment_head`, `mouth`, `chest` | citizens, police | not used by HL2:DM's player code |

The active weapon's world model (`playermodel` key, H0) is drawn
bone-merged onto the player: each of its bones takes the player bone of
the same name (as CS:S's weapons; animation.md §8 naming rules).

## Per-tick order

**Server, one user command** (movement.md and weapons.md give the rest):

1. Weapon selection in the command: a switch resets the main sequence
   (§4).
2. Pre-think; at its end the per-command damage-force sum is cleared.
3. Movement; a jump requests the jump activity; a hard landing requests
   idle-or-moving. The entity's angles become the view angles (pitch
   clamped to ±90).
4. Weapon step: fire and reload gestures (§8); pickup auto-switch (§4).
5. Idle-or-moving request from velocity, ground and water (§4).
6. Frame advance: main cycle (previous playback rate, or 1 for a new
   sequence), gesture layers (finish, remove).
7. Ducked collision box if ducked.
8. Animation state: `move_yaw` (body yaw = view yaw); entity angles ←
   (previous eye pitch, previous F, 0); `aim_pitch` (previous eye pitch);
   feet and `aim_yaw` (§6.3, dt 0.015); playback rate (§5).
9. Eye angles stored for the next command; entity pitch set to 0.
10. End of the server frame: lag-compensation record (origin, angles,
    box, sequence, cycle, layers).

Bullets fired in step 4 by other players' commands later in the same
frame see this player as left by step 9.

**Client, each rendered frame, for each other player**:

1. Interpolate origin, angles, eye angles, sequence, cycle, playback rate,
   layers.
2. Look target (§6.4 step 1).
3. Animation state: `move_yaw` (estimated velocity, networked yaw),
   `aim_pitch`, feet and `aim_yaw` (frame dt), playback rate (cosmetic),
   eyes target, blink timer, `head_yaw`, `head_pitch`.
4. Bones (§7) with render angles (0, F_client, 0).
5. Flex: controllers → blink → rules → eye aim (§11).
6. Draw (Eyes, Teeth, VertexLitGeneric).

**Client, local player view model, each rendered frame**: eye position and
angles (with punch) → bob (§12.2) → lag (§12.3 steps 1–2) → pitch offsets
(§12.3 step 3) → 10 % shake → place → FOV → draw.

## Edge cases

- **Dormant victim**: a death out of the client's view builds the
  ragdoll at the record's origin with no bone velocity; the force still
  applies (ragdolls.md edge cases).
- **Second death before the first ragdoll settles**: the first is
  deleted at once, wherever it is.
- **Model change while alive** (DM, rules.md): the new model keeps the
  same merged sequence indices, so the sequence and cycle carry over; the
  client resets all pose parameters to 0 and the feet state (§6.5), so
  `aim_yaw` and the head snap for a frame. Flex weights are reset to 0
  (they already are).
- **Teleport or spawn**: a spawn counter tells clients to snap the
  player's position history (no gliding from the death spot); the gait
  yaw and feet carry over (the first standing update snaps F to the eyes).
- **Ladders**: no climb sequence; `move_yaw` follows the climb velocity
  only when it has a horizontal part.
- **Pose ranges vs code clamps**: `aim_yaw` is clamped to ±60 first, then
  to the model's (asymmetric) range: male +52.5 left, −49.4 right.
- **Looking straight down**: `aim_pitch` 90 is clamped to the pose range
  maximum 49.98 (male); hitboxes stop following the view past ~50°.
- **Crouch idle with grenade, melee or slam**: one-frame sequences with no
  aim matrix: the upper body does not aim (`aim_pitch`/`aim_yaw` have no
  effect).
- **Gesture for a missing sequence**: silently nothing (crossbow fire is
  not even requested).
- **Shotgun double shot**: one gesture restart, like a single shot.
- **Spectating in-eye**: the observer sees the target's view model with
  the observer's own bob state (one global bob per client).
- **Hitboxes outside the collision box**: arms and head can extend beyond
  the 32×32×72 box (e.g. aiming down while crouched); whether a ray that
  misses the box can still hit them is engine behaviour (Open questions
  6).

## Quirks

- **Fire and run animations restart at cycle 0** on every sequence
  change, including idle → run. Keep.
- **No walk animation**: walking plays the run at 0.715×. Keep.
- **The crossbow, physcannon and AR2 ball play no third-person fire
  animation**; the shotgun's reload plays none. Keep.
- **The ragdoll is shoved from the pelvis only**, whatever was hit. Keep.
- **Ragdolls outlive the respawn** until their owner dies again. Keep.
- **Combine pelvis and upper chest are "generic"** in their hitbox set.
  With all non-head scales at 1 this changes nothing; it would if a server
  changed `sk_player_chest` or `_stomach`. Keep.
- **The server's hitboxes lag the feet yaw by one tick** and use the
  previous command's eye pitch for `aim_pitch`. Keep (sub-degree effect
  except during fast turns).
- **Pose parameters are not lag-compensated**: a rewound player aims
  where it aims now. Keep.
- **Heads turn and eyes look at whoever is within 128 units**, dead
  players and the local player included, and only on clients. Keep.
- **The feet fade uses the un-normalized angle gap**: when goal and feet
  straddle ±180 the gap reads as > 45 and the feet turn at full speed to
  the end. Keep.
- **Bob is shared by all weapons and never resets**; its phase follows
  distance travelled. Keep.
- **The first frame of a new view model pops 5 units back** (§12.3). Keep
  (one frame).

## Corrections to H0

1. **The bob row** (H0 1.8: "forward 0.1 per vertical bob, `cl_bobcycle`
   0.8"): HL2:DM's bob does **not** read `cl_bobcycle` (registered with
   0.8 but unused), `cl_bobup` or `cl_bob`; it uses a fixed 0.45 s cycle
   of bob time (lateral 0.9) and the rising fraction 0.5. Right offset
   0.8L, forward 0.1V, z 0.1V as view_models.md's alternative row says,
   which is right; H0's parenthesis is not. The sway also differs from
   the "rest" of view_models.md: HL2's spring lag (§12.3), not the history
   sway.
2. **MDL versions** (H0 Part 2, "Player models": "male_01 … male_09 44
   (male_09: 45)"): `male_08.mdl` is version **45** too.
3. **Animation state** (H0 1.7: "run, walk, crouch, jump, swim; aim and
   fire, reload and jump gestures; per held weapon kind from the script's
   `anim_prefix`"): there is **no walk and no swim** animation, the jump is
   the main sequence (not a gesture), reload gestures exist for four kinds
   only, and the weapon kind comes from a per-class table in code,
   **not from `anim_prefix`** (§3).
4. **Pass-through files** (H0 Part 2): `male_ss`, `male_gestures`,
   `male_postures`, the female ones and `police_ss` are not empty
   redirects: each holds one `reference` sequence and no include; only
   `male_shared`, `female_shared`, `combine_soldier_anims` and
   `police_animations` include the animation model (§1.1). Lookups still
   end in `male_anims.mdl`/`female_anims.mdl` as H0 says.
5. **Ragdolls** (H0 1.6): add that HL2:DM sends no death pose and always
   uses physics body 0 for the killing force (§10.1); the rest holds.

Wave 1: none found. (rules.md's "ragdoll record ... with ... the
accumulated bullet force (H7)" and weapons.md §6's "summed forces become
the force of their death" hold; §10.1 states the accumulation window
precisely: since the start of the victim's last command.)

## Test cases

Implementer checks, no game needed (tick 0.015 s; male animation model
unless said; ground speeds and durations as in Constants and §2; the
model data is read from the install at test time):

| # | Setup | Input | After | Expected |
|---|---|---|---|---|
| 1 | load `models/humans/group03/male_01.mdl` with the stock search path | merge | – | 106 sequences: 0 `ragdoll`, 1 `reference`, 17 `idle_pistol`, 21 `idle_gravgun`, 32 `run_smg1`, 36 `run_gravgun`, 77 `range_pistol`, 90 `jump_ar2`; 8 pose parameters in §1.2's order and ranges |
| 2 | load `combine_soldier.mdl`, `police.mdl`, `female_01.mdl` | merge | – | the same 106 names at the same indices; female `aim_yaw` range −50.86…56.83 |
| 3 | on ground, standing, physcannon | idle request | 1 tick | main = 21 (`idle_gravgun`), cycle 0 then +0.015/0.9667 = 0.015517, rate 1 |
| 4 | on ground, SMG1, moving forward at 190, view yaw 0 | – | steady | main 32 (`run_smg1`), `move_yaw` 0, rate 0.90567, cycle +0.022642 a tick |
| 5 | as 4, speed 320 (sprint) | – | – | rate 1.52533 |
| 6 | as 4, moving straight back | – | – | `move_yaw` ±180, cell `a_RunS`, rate 190/169.8302 = 1.11876 |
| 7 | view yaw 30, velocity direction yaw 120 at 190 | – | – | `move_yaw` +90 (left), cell `a_RunW`, rate 1.26793 |
| 8 | `move_yaw` −22.5 at 190 | – | – | G = 193.8205, rate 0.98029 (blend N/NE half and half) |
| 9 | ducked flag set, pistol, moving 63.333 | – | – | `cwalk_pistol` (merged 58), rate 1.05556, cycle +0.015833 a tick |
| 10 | ducked flag set, still, grenade | – | – | `cidle_grenade` (merged 52), one-frame, cycle stays 0 |
| 11 | in the 0.4 s duck transition (flag not set), still, pistol | – | – | `idle_pistol` |
| 12 | on ground, AR2, standing | jump | 1 tick | main `jump_ar2` (90), cycle 0.015·1.15385 = 0.017308 after the jump tick |
| 13 | as 12, a 35-tick standing jump | – | landing tick | until the landing tick the main stays `jump_ar2` (cycle ≈ 0.59 last airborne tick); landing tick: `idle_ar2` (merged 20) at cycle 0 then advanced one tick |
| 14 | running at 190 on ground with pistol | walk off a ledge (no jump) | airborne ticks | main stays `run_pistol` (merged 28); no request is made while moving in the air |
| 15 | airborne, horizontal velocity 0, never jumped | – | 1 tick | `idle_<kind>` |
| 16 | on ground, running `run_pistol` at cycle 0.4 | stop (velocity 0) | 1 tick | `idle_pistol` at cycle 0 (not 0.4) |
| 17 | running, `run_pistol` | switch to SMG1 | same tick | `run_smg1` at cycle 0 |
| 18 | no weapon, standing | – | – | main = merged 0 (`ragdoll`) |
| 19 | player frozen (intermission), moving | jump | – | request handled as idle-or-moving with speed 0: `idle_<kind>` |
| 20 | standing, pistol | fire | same tick | a layer with `range_pistol` (77), weight 1, cycle 0.05625 after the tick's advance; weight 0 after the 18th advance (cycle ≥ 1), removed on the 19th |
| 21 | SMG1, holding fire (shot every 5 ticks) | – | 20 ticks | one `range_smg1` layer, cycle reset to 0 at each shot, max cycle 0.5625 (5 advances of 0.1125), never finishes |
| 22 | crossbow / physcannon / AR2 alt-fire | fire | – | no layer added |
| 23 | SMG1 | reload | – | layer `reload_smg1` (99), 2.1667 s; shotgun reload: none; crossbow reload: none |
| 24 | dying with two active layers | – | – | all layers cleared |
| 25 | standing, F = 0, eye yaw 0, T > 0 | eye yaw → 30 | 1 tick | `aim_yaw` 30, F 0 |
| 26 | as 25 | eye yaw → 50 | ticks 1, 2, 3, 4 | tick 1: `aim_yaw` 50, F 0 (Γ becomes 50); tick 2: F 5.4, torso 44.6; tick 3: 39.248; tick 4: 34.538 (×0.88 a tick below 45) |
| 27 | as 25 with eye yaw 30 held still | – | 3 s after the last change | Γ = 30 at the first update with now > T + 3; then torso 30 → 26.4 → 23.232… (F converging) |
| 28 | moving at 1.5 units/s (3D), eye yaw 70 | – | – | F = 70, `aim_yaw` 0 |
| 29 | male, eye yaw 80 relative to F after a snap (torso 80 for one update) | – | – | aim_yaw = clamp(80, ±60) = 60 → pose clamp 52.5174 |
| 30 | eye pitch 60 (down) / −85 (up) | – | – | `aim_pitch` 49.9848 / −80.8630 (male) |
| 31 | standing, velocity exactly 0, gait yaw 0, body yaw 90 | 1 tick (dt 0.015) | – | gait yaw 5.4, `move_yaw` = −(90 − 5.4) = −84.6 |
| 32 | server entity yaw | F changes 0 → 5.4 in tick N | tick N | entity yaw (hitboxes) 0 during tick N, 5.4 from tick N+1 |
| 33 | client: another player 100 units in front, eye heights equal, head yaw 0 | frames of dt 0.01 with the target 30° left | – | `head_yaw` +1.3 a frame, reaches 30 on the 24th frame (if F constant); target 60° left: clamped at 44.82 |
| 34 | client: no player within 128 units | – | – | look target = origin + 512 · forward(0, F, 0) |
| 35 | blink toggled at t0 | – | t0 + 0.0, 0.05, 0.1, 0.15, 0.2 | blink weight 0, 0.76278, 0.31821, 0.07764, 0; peak 1 at t0 + 0.0322 |
| 36 | citizen at rest | run flex rules | – | descriptors `upper_right_neutral`, `lower_right_neutral`, `upper_left_neutral`, `lower_left_neutral` = 1, all others 0; with blink 1: upper lowerers 1, lower neutral 0.5 and raiser 0.5 |
| 37 | eye target 45° to the side in the `eyes` attachment's frame, 100 units away | – | – | target moved onto the 30° cone: direction (0.866, 0.5 sideways), still 100 units away |
| 38 | eye target 2 units in front | – | – | x pushed to 6 before aiming |
| 39 | pistol kill, F = 666.57 along +X, male citizen ragdoll | create | – | pelvis Δv 52.18 in/s (666.57/12.7749); every other body 7.406 in/s (666.57/90), plus bone velocities |
| 40 | .357 kill on a Combine | create | – | pelvis Δv 10882.81/11.0025 = 989.1 in/s; others 108.83 in/s |
| 41 | crowbar kill (force 7500) on a female citizen | create | – | pelvis 599.5 in/s (7500/12.5099), others 7500/70.65 = 106.2 in/s |
| 42 | player dies, respawns, plays 60 s, dies again | – | – | one `hl2mp_ragdoll` for that player at all times after the first death; the first is removed at the second death |
| 43 | ragdoll exists | `mp_restartgame` | – | removed |
| 44 | bob, T_b 0, steady 190 | 0.1 s of frames | – | T_b 0.059375; V 0.775289, L 0.552827: origin +0.077529 forward, +0.077529 z, +0.442261 right; roll +0.387645, pitch −0.310116, yaw −0.165848 |
| 45 | bob at T_b 0.1125, s 190 | – | – | V 0.95 (crest), L 0.755226 |
| 46 | bob at T_b 0.3375, s 320 | – | – | V −0.64 (trough), L 1.271960 |
| 47 | standing still for 1 s after moving | – | – | V, L → 0 at once (s = 0; no slew), T_b frozen |
| 48 | view model, steady view | – | – | lag offset 0 |
| 49 | view model, yaw turns 10° in one frame | that frame | – | gun offset 5 · 2 sin 5° = 0.8716 units opposite the turn; next frame (dt 0.01) about 0.83 |
| 50 | view model, eye pitch 30 (down) | – | – | origin + (−1.05) f_e + (−0.9) r_e + (−0.6) u_e |
| 51 | FOV base 75 / base 90, no zoom | – | – | view-model FOV 54 / 54 |
| 52 | base 75, current FOV 50 | – | – | view-model FOV 29 (before the aspect conversion) |
| 53 | a CS:S-style `cl_righthand 0` setting | – | – | no such cvar; HL2:DM view models are never mirrored |
| 54 | bullet ray through a male citizen's `Head1` box only | pistol | – | hitgroup 1, damage 24; through `Pelvis`: group 3, 8 |
| 55 | ray through a Combine's `Pelvis` or `Spine2` box | pistol | – | hitgroup 0 (generic), damage 8 |
| 56 | ray through a female citizen's `Spine1` box | – | – | hitgroup 3 |
| 57 | lag-compensated target that has since raised its aim | shot at its old position | – | old origin/angles/sequence/cycle/layers, **current** pose parameters |

Checks against this install or the probe server (game build 24600434,
server build 24600426, PatchVersion 10889068; "(install)" = reading the
files, "(probe)" = the probe server with a SourceMod fake client):

| # | How | Expected |
|---|---|---|
| P1 (install) | `hl2mp/steam.inf`, `steamapps` manifests of game and server | PatchVersion 10889068; build ids 24600434 and 24600426 |
| P2 (install) | read `models/player/male_anims.mdl` and `female_anims.mdl` from `hl2mp/hl2mp_pak_dir.vpk` | version 44, 56 bones, 105 sequences, 275 animations, 8 pose parameters with §1.2's ranges, 5 attachments (male) / 4 (female, no `anim_attachment_head`) |
| P3 (install) | read the 19 models from `hl2/hl2_misc_dir.vpk` | versions as §1.1 (male_08 and male_09: 45; Combine soldier and super soldier 46); hitbox counts 17/18/18/17 and groups as §9 |
| P4 (install) | read the 19 `.phy` files | 15 solids, 14 `ragdollconstraint`, 13 `collisionpair`, masses as §10.2 |
| P5 (install) | citizen flex data and eyeballs | 54 controllers, 71 descriptors, 63 rules, 2 eyeballs radius 0.5, iris scale 1.4706 (male) / 1.5152 (female) |
| P6 (probe) | RCON `mp_facefronttime`, `sv_wpn_sway_pred_legacy`, `sv_viewmodel_lag_do_angles` | 3, 0, 1 |
| P7 (probe) | fake client with the physcannon runs forward at 190; read its `m_nSequence`, `m_flPlaybackRate` | 36, 0.9057 (± float) |
| P8 (probe) | fake client standing; turn its view 30° then 50° in one command each; read its absolute angles | yaw unchanged after 30°; after 50° the yaw starts moving one tick later at 5.4° a tick |
| P9 (probe) | SourceMod trace-attack hook; shoot a Combine fake client in the hips and upper chest, a citizen in the hips | hitgroups 0, 0, 3 |
| P10 (probe) | kill a fake client, let it respawn, count `hl2mp_ragdoll` entities over 30 s; kill it again | 1 all along; a new entity index after the second death |
| P11 (probe) | a fake client fires the crossbow, the pistol, reloads the shotgun; read its overlay layers' sequences | pistol: 77 for 18 ticks; crossbow and shotgun reload: no layer |

## Open questions

1. **SDK vs build 24600434.** The player animation state, gesture rules
   and view-model code here are from the current public SDK; the shipped
   game's binaries carry the same cvar names (checked in its client and
   server libraries), but behaviour can only be confirmed with P6–P11 and
   a refcmp capture of a running player.
2. **Blended ground speed.** The ground speed at a `move_yaw` between two
   cells is taken as the bilinear-weighted sum of the cells' movement
   vectors (engine helper, not in the SDK). Check with P7 strafing at
   `move_yaw` ≈ −22.5 (expected rate 0.980).
3. **Autoplay sequences.** That `body_rot_z` … `head_rot_x` are applied to
   every frame with weight 1 and that a pose value 0 adds nothing is the
   engine's autoplay rule (not in the SDK). A refcmp of a citizen next to
   another player (head turned) vs alone would confirm the head turn is
   visible; the server applies them with pose 0.
4. **Iris projection.** The Eyes shader's iris and glint planes come from
   the engine's eyeball setup (not in the SDK). Expected: two planes
   perpendicular to the gaze through the eye centre, scaled by the iris
   scale so the iris texture spans 1/iris scale units (0.68 for men),
   with the ±0.07 offset per eye. Needs a close refcmp of a citizen's face
   to settle orientation and scale.
5. **Teeth illumination.** `$illumfactor` and `$forward` are set by the
   engine from the mouth entry and the `mouth` descriptor (0 at rest). How
   dark the teeth are at rest needs a refcmp of an open-mouthed citizen
   (there is none in DM) or a close capture; assume the factor is the
   descriptor weight clamped to [0, 1].
6. **Hitboxes vs the collision box.** Whether a bullet ray must touch the
   player's collision box (32×32×72, ducked 36) before its hitboxes are
   tested is engine code. Probe: shoot past a fake client's outstretched
   arm outside its box.
7. **`cl_playermodel` change mid-life** keeps the sequence and cycle
   (same merged indices); whether the client's pose reset is visible
   should be checked in a refcmp, it is cosmetic.
8. **Eye pitch over the wire.** The 8-bit pitch over [−90, 90] assumes
   the server's eye pitch is already in that range (it is clamped to ±89
   by the client's input); a pitch stored as 270–360 would clamp to the
   wrong end. Confirm with a probe fake client looking up.
9. **Head/eye targets at 128 units** are measured origin to origin in 3D;
   whether players a floor above or below are looked at the same way
   needs a refcmp (expected yes).
