# Team Fortress 2: class player models — third-person animation, aim, weapon attachment, hitboxes (T-A1)

Source basis: the retail client install (app 440, PatchVersion
11191711), read in place: the class models `models/player/<class>.mdl`
and their include models `models/player/<class>_animations.mdl` for all
nine classes (bones, pose parameters, sequences with activities,
activity weights, flags, blend grids and pose keys, autolayers, events,
fades, movement data, attachments, body groups, hitbox sets), the
`.phy` collision files of the Scout, Soldier, Heavy and Engineer (solid
names, masses, the text section), and `scripts/items/items_game.txt`
(item slots of the stock guns). Read with our dumper and a throwaway
reader of the model format as described in
[specs/cs_source/animation.md](../cs_source/animation.md) §1, §5, §7 and
[specs/cs_source/mdl_v48.md](../cs_source/mdl_v48.md). Public
documentation: the community TF2 wiki (classes, weapons, death
animations) and the Valve Developer Community pages on model
compilation options (sequence and autolayer options). No game, engine or
SDK code was read. How the game *chooses* among these sequences is not
in the data; where this spec states it, it is inferred from the data's
naming and structure and from the multiplayer animation state already
described in our [cs_source/animation.md §13](../cs_source/animation.md),
and is marked so.
Status: draft

## Summary

Each class model holds the mesh and a few stub sequences; its real
animation comes from `<class>_animations.mdl` (plus a user and a
workshop include, both for taunts). The body is built like the later
Source multiplayer games: a looping **main sequence** picked by movement
state (stand, crouch, run, crouch-walk, swim, airwalk, jump) and by the
held weapon's **kind** (`PRIMARY`, `SECONDARY`, `MELEE`, `ITEM1`, `PDA`,
`BUILDING`, …), with a 3 × 3 move grid on `move_x` / `move_y` whose
corners are authored at the class's own top speed; each main sequence
carries an **aim matrix** autolayer (3 × 4 additive grid on `body_yaw`
/ `body_pitch`); short **additive gestures** play fire, reload
(start / loop / end for one-at-a-time reloads), land and flinch on top.
The Heavy's Minigun has its own spun-up ("deployed") main sequences and
spin-up / spin-down gestures that cross-fade the aim matrix. The held
weapon is the same `c_` model as in first person, bone-merged by bone
name onto `weapon_bone` and its siblings under the right hand.

## Units and conventions

- Hammer units, Z up. Model space: the animated character faces +X, its
  left is +Y (cs_source/animation.md conventions hold).
- Animations are sampled at their own fps; duration = (frames − 1) / fps.
  "Loop" = the looping flag. Flags: 0x1 looping, 0x2 snap (no
  cross-fade into it), 0x4 delta (additive), 0x10 post (with delta:
  applied in the bone's local frame after the base), 0x400 hidden
  (not selectable by activity).
- Ground speed = the authored displacement of the grid's forward (N)
  cell divided by its duration (our dumper prints the same).
- Pose parameters by **name**: the include models list them in
  different orders (Scout and Heavy: `move_x` before `move_y`; Soldier
  and Engineer: `move_y` before `move_x`), so grids must be mapped to
  the merged model's parameters by name, never by index.
- "Kind" = the suffix of the activity, e.g. `ACT_MP_RUN_PRIMARY`.
- Fade = the sequence's fade-in / fade-out time: 0.2 / 0.2 s for every
  sequence listed unless shown.

## Constants

| Name (ours) | Value | Unit | Meaning |
|---|---|---|---|
| `body_pitch` | −45 … 90 (loop 360) | ° | aim up/down; row keys 90, 45, 0, −45 |
| `body_yaw` | −45 … 45 (loop 360) | ° | torso twist; column keys 45, 0, −45 |
| `move_x` | −1 … 1 | – | forward (+1) / back (−1) on the move grid |
| `move_y` | −1 … 1 | – | right (+1) / left (−1) on the move grid |
| `r_hand_grip` | 0 … 16 | – | hand-pose library index (`r_handposes`, 17 one-frame poses for special items); 0 for the four guns (assumed) |
| `r_arm` | 0 … 3 | – | arm-pose library (`r_armposes`); 0 for the four guns (assumed) |
| main_fade | 0.2 | s | cross-fade between main sequences |
| ragdoll_total_mass | 100 | kg | every class's `.phy` `totalmass` |

Authored ground speeds (forward cell of the run / crouch-walk / swim
grids; same for every kind of a class unless noted):

| Class | run | crouch-walk | swim | Heavy spun ("deployed") move |
|---|---|---|---|---|
| Scout | 400 | 150 | 360 | – |
| Soldier | 240 | 80 | 240 (`swim_MELEE` 224) | – |
| Heavy | 230 | 76.7 | 230 | 80 (swim 80) |
| Engineer | 300 | 100 | 240 (`swim_PDA` 156.5) | – (`run_BUILDING_DEPLOYED` 180) |
| Pyro | 300 | 100 | – | – |
| Demoman | 280 | 100 | – | – |
| Medic | 320 | 106.6 | – | – |
| Sniper | 300 | 100 | – | – |
| Spy | 300 | 100 | – | – |

The run speeds equal the classes' top speeds (classes.md) except the
Spy's (300 authored vs 320 measured).

## Behavior

### 1. The models

| Class | Model | Bones | Includes (in order) | Animation model: bones / sequences / animations |
|---|---|---|---|---|
| Scout | `models/player/scout.mdl` | 78 | `models/player/scout_user_animations.mdl`, `models/player/scout_animations.mdl`, `models/workshop/player/animations/scout_workshop_animations.mdl` | 76 / 377 / 1012 |
| Soldier | `models/player/soldier.mdl` | 86 | same pattern (`soldier_…`) | 86 / 419 / 858 |
| Heavy | `models/player/heavy.mdl` | 79 | same pattern (`heavy_…`) | 77 / 317 / 819 |
| Engineer | `models/player/engineer.mdl` | 73 | same pattern (`engineer_…`) | 73 / 402 / 1131 |

The other five classes follow the same pattern (`pyro`, `demo`,
`medic`, `sniper`, `spy`). Merging (bones by name, the include's
sequences after the model's own; a same-named local stub must lose to
the include's real sequence) is cs_source/animation.md §8 and
install.md 4.3. The class model's local sequences are 1-frame stubs
except `ref` and `ragdoll` (`ACT_DIERAGDOLL`, 1 frame).

Skeleton (all classes): root `bip_pelvis`; spine `bip_spine_0` … `_3`,
`bip_neck`, `bip_head`; arms `bip_collar_L/R`, `bip_upperArm_L/R`,
`bip_lowerArm_L/R`, `bip_hand_L/R`, fingers `bip_thumb/index/middle/ring/pinky_0..2_L/R`;
legs `bip_hip_L/R`, `bip_knee_L/R`, `bip_foot_L/R`, `bip_toe_L/R`;
weapon bones `weapon_bone` (child of `bip_hand_R`) with children
`weapon_bone_1` … `_4` (Scout; fewer on others), `weapon_bone_L` (child
of `bip_hand_L`), `effect_hand_L/R`; prop bones `prop_bone`,
`prop_bone_1` … `_6` (taunt props); helper bones `hlp_*`. Class extras:
Scout `bip_dogtag_0..3`, `bip_packtop`, `bip_packmiddle`, `medal_bone`;
Soldier `handle_bone`; Engineer `taunt_effect`.

Body groups (index 0 = shown unless an item hides it): Scout `scout`,
`hat`, `headphones`, `shoes_socks`, `dogtags`; Soldier `soldier`,
`rocket` (2 options: the rocket held during reload, 3.4), `hat`,
`medal`, `grenades`; Heavy `heavy`, `hands` (3), `bullets`; Engineer
`engineer`, `hat`, `rightarm` (3: arm, Gunslinger …).

Attachments the HUD and effects use: `head`, `eyes` (all but the
Engineer), `partyhat`, `flag`, `back_lower`, `foot_L`, `foot_R`,
`effect_hand_L/R`, `weapon_bone` (Scout, Soldier), `hand_L/R` (Scout).

### 2. Activities and kinds

A movement state name plus the held weapon's kind gives the activity.
Kinds present: Scout `PRIMARY`, `SECONDARY`, `MELEE`, `MELEE_ALLCLASS`,
`ITEM1`, `ITEM2`, `LOSER`, `PASSTIME`; Soldier and Heavy the same set
without `ITEM2`; Engineer adds `PDA` and `BUILDING` (and
`BUILDING_DEPLOYED`, carrying a building). The losing team's
"humiliation" uses the `LOSERSTATE` activities.

**Which kind each M2 gun uses** (from `items_game.txt`: no stock gun has
an `anim_slot`, so the kind is the item's slot for that class):

| Gun | Holder | Slot | Third-person kind | First-person arms activities (install.md 4.3) |
|---|---|---|---|---|
| Scattergun (13) | Scout | primary | `PRIMARY` | `ACT_PRIMARY_VM_*` (`sg_*`) |
| Rocket Launcher (18) | Soldier | primary | `PRIMARY` | `ACT_PRIMARY_VM_*` (`dh_*`) |
| Shotgun (9) | Engineer | primary | `PRIMARY` | `ACT_PRIMARY_VM_*` (`fj_*`) |
| Shotgun (10) | Soldier | secondary | `SECONDARY` | `ACT_SECONDARY_VM_*` (unprefixed) |
| Shotgun (11) | Heavy | secondary | `SECONDARY` | `ACT_SECONDARY_VM_*` (unprefixed) |
| Shotgun (12) | Pyro | secondary | `SECONDARY` | `ACT_SECONDARY_VM_*` |
| Minigun (15) | Heavy | primary | `PRIMARY` (+ deployed set, 3.5) | `ACT_PRIMARY_VM_*` (`m_*`) |

The multi-class Shotgun (199) prefab states the same per class:
engineer primary, soldier / pyro / heavy secondary. A gun held by a
class not in this table (a mash-up) has no authored pose; Chimera must
pick a kind (Open questions 1).

### 3. Main sequences (lower body and stance)

Every main sequence below is non-additive, loops unless said, and owns
autolayers that add the aim matrix (§4). Run, crouch-walk and swim are
3 × 3 grids on (`move_y` columns −1, 0, 1) × (`move_x` rows −1, 0, 1):
row −1 = backward cells (`…SW`, `…S`, `…SE`), row 0 = (`…W`, `…Center`,
`…E`), row +1 = (`…NW`, `…N`, `…NE`). W cells move toward +Y (left),
E cells toward −Y (right). The centre cell has no motion. Diagonal cells
move at the full ground speed (e.g. Scout (283, 283) = 400), so the
grid corners are the unit square, not the unit circle.

Scout:

| Activity (kind K) | `PRIMARY` | `SECONDARY` | `MELEE` |
|---|---|---|---|
| `ACT_MP_STAND_K` | `stand_PRIMARY` 45 f, 1.4667 s | `stand_SECONDARY` 45, 1.4667 | `stand_MELEE` 31, 1.0 |
| `ACT_MP_CROUCH_K` | `crouch_PRIMARY` 46, 1.5 | `crouch_SECONDARY` 46, 1.5 | `crouch_MELEE` 31, 1.0 |
| `ACT_MP_RUN_K` | `run_PRIMARY` 3×3, 49, 1.6 s, 400 u/s | `run_SECONDARY` same | `run_MELEE` same |
| `ACT_MP_CROUCHWALK_K` | `crouch_walk_PRIMARY` 3×3, 46, 1.5, 150 | `crouch_Walk_SECONDARY` same | `crouch_walk_MELEE` 31, 1.0, 150 |
| `ACT_MP_SWIM_K` | `swim_PRIMARY` 3×3, 61, 2.0, 360 | `Swim_SECONDARY` same | `Swim_MELEE` same |
| `ACT_MP_AIRWALK_K` | `airwalk_PRIMARY` 61 @ 36 fps, 1.6667 | `airwalk_SECONDARY` same | `airwalk_MELEE` same |
| `ACT_MP_JUMP_START_K` | `Jump_Start_primary` 3×3, 23, 0.7333, **not looping**, forward cell moves 446.8 u/s | `Jump_Start_secondary` same | `Jump_Start_melee` same |
| `ACT_MP_JUMP_FLOAT_K` | `Jump_Float_primary` 3×3, 2 frames, loop, 461 u/s | `Jump_Float_secondary` | `Jump_Float_melee` |
| `ACT_MP_JUMP_LAND_K` | `jumpland_primary` 31, 1.0, **additive** (0x14) | `jumpland_secondary` | `jumpland_melee` |

(The Scout also has `jump_double_primary` and `Jump_crouchjump_primary`
without activities: the air jump; Open questions 4.)

Soldier:

| Activity | `PRIMARY` | `SECONDARY` | `MELEE` |
|---|---|---|---|
| STAND | `stand_PRIMARY` 51, 1.6667 | `stand_SECONDARY` 51, 1.6667 | `stand_MELEE` 51, 1.6667 |
| CROUCH | `crouch_PRIMARY` 51, 1.6667 | `crouch_SECONDARY` same | `crouch_MELEE` same |
| RUN | `run_PRIMARY` 3×3, 22, 0.7 s, 240 | `run_SECONDARY` same | `run_MELEE` same |
| CROUCHWALK | `crouch_walk_PRIMARY` 3×3, 31, 1.0, 80 | `crouch_walk_SECONDARY` 51, 1.6667, 80 | `crouch_walk_MELEE` 51, 1.6667, 80 |
| SWIM | `swim_PRIMARY` 3×3, 61, 2.0, 240 | `swim_SECONDARY` same | `swim_MELEE` 224 |
| AIRWALK | five sequences, activity weight 1 each: `airwalk_PRIMARY` 31, 1.0; `primary_floatA` 31, 1.0; `primary_floatB` 27, 0.8667; `primary_floatC` 25, 0.8; `primary_floatD` 37, 1.2 | `airwalk_SECONDARY` 25, 0.8 | `airwalk_MELEE` 25, 0.8 |
| JUMP_START | `jump_start_PRIMARY` 1×1, 24, 0.7667, not looping | `jump_start_SECONDARY` 22, 0.7 | `a_jumpstart_MELEE` 26, 0.8333 |
| JUMP_FLOAT | `jump_float_PRIMARY` 24, 0.7667, loop | `jump_float_SECONDARY` 20, 0.6333 | `a_jumpfloat_MELEE` 78, 2.5667 |
| JUMP_LAND | `jumpland_PRIMARY` 31, 1.0, additive | `jumpland_SECONDARY` 21, 0.6667 | `jumpland_MELEE` 21, 0.6667 |

Heavy:

| Activity | `PRIMARY` | `SECONDARY` | `MELEE` |
|---|---|---|---|
| STAND | `stand_PRIMARY` 61, 2.0 | `stand_SECONDARY` 61, 2.0 | `stand_MELEE` 61, 2.0 |
| CROUCH | `crouch_PRIMARY` 61, 2.0 | `crouch_SECONDARY` same | `crouch_MELEE` same |
| RUN | `run_PRIMARY` 3×3, 17, 0.5333, 230 | `run_SECONDARY` same | `run_MELEE` same |
| CROUCHWALK | `crouch_walk_PRIMARY` 3×3, 61, 2.0, 76.7 | `crouch_walk_SECONDARY` same | `crouch_walk_MELEE` 31, 1.0, 76.7 |
| SWIM | `swim_PRIMARY` 3×3, 61, 2.0, 230 | `swim_SECONDARY` same | `swim_MELEE` same |
| AIRWALK | `airwalk_PRIMARY` 21, 0.6667 | `airwalk_SECONDARY` 27, 0.8667 | `airwalk_MELEE` 21, 0.6667 |
| JUMP_START | `jumpstart_PRIMARY` 25, 0.8 | `jumpstart_SECONDARY` 21, 0.6667 | `jumpstart_MELEE` 25, 0.8 |
| JUMP_FLOAT | `jumpfloat_PRIMARY` 17, 0.5333 | `jumpfloat_SECONDARY` 31, 1.0 | `jumpfloat_MELEE` 17, 0.5333 |
| JUMP_LAND | `jumpland_PRIMARY` 21, 0.6667, additive | `jumpland_SECONDARY` same | `jumpland_MELEE` same |

Engineer:

| Activity | `PRIMARY` | `SECONDARY` | `MELEE` |
|---|---|---|---|
| STAND | `stand_PRIMARY` 47, 1.5333 | `stand_SECONDARY` 47, 1.5333 | `stand_MELEE` 97, 3.2 |
| CROUCH | `crouch_PRIMARY` 47, 1.5333 | `crouch_SECONDARY` same | `crouch_MELEE` 61, 2.0 |
| RUN | `run_PRIMARY` 3×3, 51, 1.6667, 300 | `run_SECONDARY` 47, 1.5333, 300 | `run_MELEE` 19, 0.6, 300 |
| CROUCHWALK | `crouch_walk_PRIMARY` 3×3, 27, 0.8667, 100 | `crouch_walk_SECONDARY` same | `crouch_walk_MELEE` 61, 2.0, 100 |
| SWIM | `swim_PRIMARY` 3×3, 61, 2.0, 240 | same | same |
| AIRWALK | `airwalk_PRIMARY` 31, 1.0 | `airwalk_SECONDARY` same | `airwalk_MELEE` same |
| JUMP_START | `a_jumpstart_PRIMARY` 26, 0.8333 | `a_jumpstart_SECONDARY` same | `a_jumpstart_MELEE` same |
| JUMP_FLOAT | `a_jumpfloat_PRIMARY` 31, 1.0 | `a_jumpfloat_SECONDARY` same | `a_jumpfloat_MELEE` 35, 1.1333 |
| JUMP_LAND | `jumpland_PRIMARY` 21, 0.6667, additive | same | same |

All rows are 30 fps unless shown. The Engineer's `PDA` and `BUILDING`
kinds have the same set (`stand_PDA`, `run_BUILDING`, …); carrying a
building uses `…_BUILDING_DEPLOYED` (run 180 u/s).

**Footstep events** (event 7001, option "left" / "right", at cycle):
Scout run, every kind: right 0.0208, left 0.1458, right 0.2708, left
0.3958, right 0.5208, left 0.6458, right 0.7708, left 0.8958 (eight
steps per 1.6 s cycle). Soldier run: right 0.0952, left 0.5238. Heavy
`run_PRIMARY`: right 0.3125, left 0.8125 (`run_SECONDARY` 0.4375 /
0.9375, `run_MELEE` 0.375 / 0.875); Heavy crouch-walk 0.2333 / ~0.75;
Heavy `PRIMARY_deployed_movement` right 0.2083, left 0.7083. Engineer
run: right 0.3889, left 0.8889. Whether TF2 plays footsteps from these
events or from movement distance is Open questions 6.

**Choosing the main sequence** (inferred; the multiplayer animation
state of cs_source/animation.md §13 with TF2's activities):

1. In water deep enough to swim → `ACT_MP_SWIM`.
2. Else in the air: after a jump → `ACT_MP_JUMP_START` (plays once),
   then `ACT_MP_JUMP_FLOAT` until landing; airborne without a jump
   (walked off a ledge, blast-jumped) → `ACT_MP_AIRWALK`.
3. On landing → back to ground states and the additive
   `ACT_MP_JUMP_LAND` gesture (§5).
4. On the ground: ducked → `ACT_MP_CROUCHWALK` if moving, else
   `ACT_MP_CROUCH`; standing → `ACT_MP_RUN` if moving, else
   `ACT_MP_STAND`. There is no walk activity.
5. Then translate the activity by the weapon kind (`ACT_MP_RUN` →
   `ACT_MP_RUN_PRIMARY`). If several sequences carry the activity, pick
   one at random weighted by activity weight (Soldier's airwalk:
   five equal choices).

**Move parameters** (cs_source/animation.md §13): with v the horizontal
velocity in model space and d its direction, (`move_x`, `move_y`) =
(cos d, −sin d) pushed out to the unit square (divided by
max(|x|, |y|)), then scaled by speed / G if the speed is below the
grid's ground speed G there; never above 1. The playback rate is 1.
Because G equals the class's top speed, a running player at top speed
has |move| = 1 and his feet match the ground; a Scout crouch-walking at
133 (a third of 400) has |move| = 133 / 150 = 0.89.

### 4. Aim: `body_pitch`, `body_yaw` and the aim matrix

- Each main sequence's autolayers add one aim-matrix sequence:
  `<KIND>_aimmatrix_idle` (stand, jump), `…_crouch_idle` (crouch,
  crouch-walk), `…_run` (run), `…_swim` (swim, airwalk). Examples:
  `stand_PRIMARY` → `PRIMARY_aimmatrix_idle` (+ `layer_idleNoise` on the
  Scout), `run_PRIMARY` → `PRIMARY_aimmatrix_run`.
- Aim-matrix sequences are hidden, additive and post (flags 0x414), a
  3 (columns) × 4 (rows) grid: columns on `body_yaw` keys 45, 0, −45
  (cells named `…_right`, `…_center`, `…_left`), rows on `body_pitch`
  keys 90, 45, 0, −45 (`…_straight_up` (all three columns the same
  animation), `…_up_*`, `…_mid_*`, `…_down_*`). Each cell is a 1-frame
  additive pose (animation flag delta).
- So `body_pitch` = +90 aims straight up and −45 is the lowest aim:
  body_pitch = −(eye pitch) with Source's "positive pitch looks down"
  convention, clamped to −45 … 90 (inferred from the cell names; the
  clamp means looking more than 45° down shows the 45° pose).
  `body_yaw` = +45 is the cell named "right".
- `body_yaw` is the torso's offset from the feet: the feet (model) yaw
  lags the eye yaw; standing still the feet stay until the difference
  exceeds 45°, moving they turn to face the eye yaw (cs_source §13: goal
  rules, 720 °/s, scale clamp(|diff|/60, 0.01, 1)). body_yaw = the
  remaining difference with the sign that makes "right" match turning
  right (Open questions 2).
- **Running backward**: the Scout's `run_PRIMARY` has two aim
  autolayers ramped by `move_x` (autolayer flags 0x4040, ramp on a pose
  parameter): `PRIMARY_aimmatrix_run` with (start, peak, tail, end) =
  (−0.8, −0.2, 1, 2) and `PRIMARY_aimmatrix_runS` with (−2, −1, −0.8,
  −0.2): moving backward (`move_x` ≤ −0.8) uses the backward aim set,
  forward (≥ −0.2) the forward set, and between them they cross-fade
  linearly. Other classes have one run aim layer.

### 5. Gestures (additive overlays)

Gestures are additive post sequences (flags 0x14; the Scout's primary
fire also snap, 0x16) played once on an overlay layer over the main
sequence (cs_source/animation.md §10); they do not loop unless said.
Each has a stand, crouch, swim and (for reloads) airwalk variant; the
variant follows the current stance when the gesture starts (inferred).

Fire (`ACT_MP_ATTACK_STAND_K`, `…_CROUCH_K`, `…_SWIM_K`; there are no
`ACT_MP_ATTACK_AIRWALK_*` sequences, so airborne fire uses the stand
variant: Open questions 3):

| Class, kind | Stand | Crouch | Swim |
|---|---|---|---|
| Scout `PRIMARY` (Scattergun) | `AttackStand_PRIMARY` 21 f, 0.6667 s | `AttackCrouch_PRIMARY` 0.6667 | `AttackSwim_PRIMARY` 0.6667 |
| Scout `SECONDARY` (Pistol) | `AttackStand_SECONDARY` 8, 0.2333 | 0.2333 | 31, 1.0 |
| Soldier `PRIMARY` (Rocket Launcher) | `AttackStand_PRIMARY` 25, 0.8 | 0.8 | 0.8 |
| Soldier `SECONDARY` (Shotgun) | `AttackStand_SECONDARY` 26, 0.8333 | 0.8333 | 0.8333 |
| Heavy `PRIMARY` (Minigun) | `attackStand_PRIMARY` 21, 0.6667 | `attackCrouch_PRIMARY` 16, 0.5 | `attackSwim_PRIMARY` 31, 1.0 |
| Heavy `SECONDARY` (Shotgun) | `AttackStand_SECONDARY` 20, 0.6333 | 0.6333 | 16, 0.5 |
| Engineer `PRIMARY` (Shotgun) | `AttackStand_PRIMARY` 26, 0.8333 | 0.8333 | 0.8333 |
| Engineer `SECONDARY` (Pistol) | `AttackStand_SECONDARY` 8, 0.2333 | 0.2333 | 0.2333 |

Reload. Activities `ACT_MP_RELOAD_STAND_K`, `…_LOOP`, `…_END` (and
CROUCH, SWIM, AIRWALK). A one-at-a-time reload plays the start once,
the loop once per round loaded, and the end after the last round
(inferred from the names and weapons.md §4):

| Class, kind | Start | Loop (per round) | End |
|---|---|---|---|
| Scout `PRIMARY` (Scattergun) | `PRIMARY_reload_start` 21, 0.6667 | `PRIMARY_reload_loop` 20, 0.6333, loop | `PRIMARY_reload_end` 17, 0.5333 |
| Soldier `PRIMARY` (Rocket Launcher) | `ReloadStand_PRIMARY` 33, 1.0667 | `ReloadStand_PRIMARY_loop` 28, 0.9, loop | `ReloadStand_PRIMARY_end` 19, 0.6 |
| Soldier `SECONDARY` (Shotgun) | `ReloadStand_SECONDARY` 21, 0.6667 | `ReloadStand_SECONDARY_loop` 20, 0.6333, loop | `ReloadStand_SECONDARY_end` 15, 0.4667 |
| Heavy `SECONDARY` (Shotgun) | `SECONDARY_reload` 18, 0.5667 | `SECONDARY_reload_loop` 18, 0.5667 (not looping) | `a_SECONDARY_reload_end` 16, 0.5 |
| Engineer `PRIMARY` (Shotgun) | `ReloadStand_PRIMARY` 76, 2.5 | `a_PRIMARY_reload_loop` (weight 2) or `b_PRIMARY_reload_loop` (weight 1), 21, 0.6667 | `PRIMARY_reload_end` 16, 0.5, fade-out 1.0 |
| Scout `SECONDARY` (Pistol, single) | `ReloadStand_SECONDARY` 31, 1.0 | – | – |
| Heavy `PRIMARY` (Minigun) | none | | |

- The sequence lengths do not match the weapons' reload times
  (weapons.md §4: Scattergun first shell 0.7 s then 0.5 s): the loop is
  restarted for each round (or played at a rate that fits; Open
  questions 5). Crouch / swim / airwalk variants have the same shape
  (Scout crouch loop 0.5 s; Soldier `ReloadSwim_PRIMARY` is a single
  89-frame 44 fps sequence without loop / end).
- The Scout's and Soldier's reload sequences carry arm layers
  (`layer_reload_standing_arms_primary_start` ramped in over cycle 0 →
  0.2; `…_end` ramped out over 0.38 → 0.62).
- **Soldier rocket in hand**: the Soldier's reload sequences fire the
  named event `AE_CL_BODYGROUP_SET_VALUE` with options "rocket 1" /
  "rocket 0": start at cycles 0.3438 (show) and 0.6562 (hide); loop at
  0.037 (hide), 0.4444 (show), 0.8148 (hide); end at 0.0556 (hide). They
  switch the player model's `rocket` body group.
- The Engineer's crouch start has two choices: `ReloadCrouch_PRIMARY`
  (76 f, weight 1) and `PRIMARY_reloadcrouch` (21 f, weight 100).

Jump land: `ACT_MP_JUMP_LAND_K` (additive, 0.6667–1.0 s) on landing.

Flinch (`ACT_MP_GESTURE_FLINCH_CHEST`, the only flinch activity):
Scout and Soldier `a_flinch01` 13 f, 0.4 s; Heavy one per kind
(`PRIMARY_flinch`, `SECONDARY_flinch`, `MELEE_flinch`, 0.4 s);
Engineer `a_flinch01` (weight 2, 0.4 s), `a_flinch02` (weight 10,
0.3333 s), `a_flinch04` (weight 5, 0.4 s). Played when the player
takes damage (inferred; Open questions 7).

Melee for reference: Scout `MELEE_swing` 34 f @ 60 fps = 0.55 s, **not**
additive (a full-body overlay with the swing layer ramped by `move_x`
like the aim layers); Heavy `attackStand_MELEE_L` (0.75 s) and `_R`
(0.8571 s) both `ACT_MP_ATTACK_STAND_MELEE` (random), `_U`
`ACT_MP_ATTACK_STAND_MELEE_SECONDARY` (crit).

### 6. The Minigun (Heavy)

Data (all `heavy_animations.mdl`):

| Activity | Sequence | Shape |
|---|---|---|
| `ACT_MP_ATTACK_STAND_PREFIRE` | `attackStand_PRIMARY_spoolup` | 16 f, 0.5 s; its own animation is empty (delta, all zero); its autolayer `layer_attackStand_PRIMARY_spoolup` (weight ramped in over cycle 0–0.67, then full) cross-fades `PRIMARY_aimmatrix_idle` (out over 0–0.33) into `PRIMARY_aimmatrix_deployed` (in over 0.33–1) |
| `ACT_MP_ATTACK_STAND_POSTFIRE` | `attackStand_PRIMARY_spooldown` | 31 f, 1.0 s; the reverse cross-fade (deployed out over 0–0.33, idle in 0.33–1), ramped out after cycle 0.67 |
| `ACT_MP_ATTACK_CROUCH_PREFIRE` / `POSTFIRE` | `attackCrouch_PRIMARY_spoolup` / `_spooldown` | 16 f, 0.5 s each; crouch aim matrices |
| `ACT_MP_ATTACK_SWIM_PREFIRE` / `POSTFIRE` | `attackSwim_PRIMARY_spoolup` / `_spooldown` | 16 f, 0.5 s; no layers |
| `ACT_MP_DEPLOYED_IDLE` | `stand_deployed_PRIMARY` | 61 f, 2.0 s, loop; aim `PRIMARY_aimmatrix_deployed` |
| `ACT_MP_DEPLOYED_PRIMARY` | `PRIMARY_deployed_movement` | 3 × 3, 24 f, 0.7667 s, loop, 80 u/s; footsteps 0.2083 / 0.7083 |
| `ACT_MP_CROUCH_DEPLOYED_IDLE` | `crouch_deployed_PRIMARY` | 16 f, 0.5 s, loop (no crouched deployed movement: a spun Heavy can't move crouched, weapons.md 7.1) |
| `ACT_MP_SWIM_DEPLOYED_PRIMARY` | `PRIMARY_swim_deployed_movement` | 3 × 3, 31 f, 1.0 s, 80 u/s |
| `ACT_MP_ATTACK_STAND_PRIMARY` | `attackStand_PRIMARY` | 21 f, 0.6667 s additive: the firing recoil gesture |

Sequence of states (inferred, matched to weapons.md 7.1): wind-up →
play the PREFIRE gesture for the stance and switch the main sequence
to the deployed set (idle or movement; 80 u/s grid against the spun
speed 110, so move parameters are scaled at most to 1 and the feet slide
a little at full spun speed: Open questions 8); while firing → repeat
`attackStand_PRIMARY` (or the crouch / swim one) for as long as the
trigger is held (restart each time it ends, assumed); wind-down → the
POSTFIRE gesture and the normal main sequences. The weapon model's own
barrel sequences (`spool_up`, `fire_loop`, `spool_down`, weapons.md 8.1)
play on the bone-merged Minigun at the same time.

### 7. The held weapon

- The third-person weapon is the item's `model_player` (the same `c_`
  model as first person, weapons.md 8.1), **bone-merged**: every bone of
  the weapon model whose name matches a player bone (`weapon_bone`,
  `weapon_bone_1`, …) takes that player bone's world transform; weapon
  bones without a match (`c_weapon_stattrack`, the Minigun's `barrel`)
  keep their bind pose relative to their parent (install.md 4.3, 4.4).
- `weapon_bone` is a child of `bip_hand_R`, so the gun follows the
  right hand as animated by the main sequence, aim matrix and gestures.
- Muzzle flash and tracers start at the weapon model's `muzzle`
  attachment (on its `weapon_bone`).
- Team skin: player skin 0 RED, 1 BLU (install.md 4.5); the stock guns
  look the same in skins 0–7.

### 8. Hitboxes

One set per class, named after the class (Scout set `scout`, 19 boxes;
the others 18). Box per bone, hit group, and bounds in the bone's frame
(min, max):

| Bone | Group | Scout | Soldier | Heavy | Engineer |
|---|---|---|---|---|---|
| `bip_head` | 1 head | (−5.5, −9, −7.05)–(5.5, 4, 4.95) | (−6.25, −9, −7.55)–(6.25, 5, 5.45) | (−6, −9.35, −8.7)–(6, 4.65, 4.3) | (−6, −11, −8)–(6, 4, 6) |
| `bip_pelvis` | 0 generic | (−7.5, −1, −4.5)–(7.5, 11, 6.5) | (−10.5, −3, −9)–(10.5, 11, 7) | (−11, 0, −9)–(11, 10, 3) | (−9, −0.5, −7.5)–(9, 11.5, 3.5) |
| `bip_spine_0`, `bip_spine_1` | 3 stomach | | | | |
| `bip_spine_2`, `bip_spine_3` | 2 chest | | | | |
| `bip_upperArm_L`, `bip_lowerArm_L`, `bip_hand_L` | 4 left arm | | | | |
| `bip_upperArm_R`, `bip_lowerArm_R`, `bip_hand_R` | 5 right arm | | | | |
| `bip_hip_L`, `bip_knee_L`, `bip_foot_L` | 6 left leg | | | | |
| `bip_hip_R`, `bip_knee_R`, `bip_foot_R` | 7 right leg | | | | |
| `bip_packmiddle` | 10 gear | (−2.75, −10, −3)–(4.25, 8, 5) | – | – | – |

Only the head group matters for the four guns' damage (none of them
headshots; classes.md Hitboxes). Bullets trace these boxes on the
animated pose (server pose; Open questions 9).

### 9. Death and ragdolls

- Death normally hands the body to a ragdoll (public behaviour) built
  from the class's `.phy`: Scout 17 solids, Soldier 17, Heavy 16,
  Engineer 18; `totalmass` 100 for each; root `bip_pelvis`; solids on
  the pelvis, hips, knees, feet, spine (Scout `bip_spine_0`; Heavy
  `bip_spine_0` and `_1`), upper and lower arms, hands, head (Scout
  also `bip_neck` and `bip_dogtag_2`); ragdoll constraints with per-axis
  limits (Scout pelvis–hip: x −20…12, y −10…50, z −79…45). The
  ragdoll's start pose is the `ragdoll` sequence (`ACT_DIERAGDOLL`, 1
  frame) blended with the current animated pose (Source rule; specs of
  the shared ragdoll behaviour apply).
- Gibs: each `.phy` lists `break` models (`player/gibs/<class>gib00N`,
  `random_organ`, fade 10 s) for explosive deaths.
- Death animations exist and play instead of a ragdoll for some kills
  (public: headshots, backstabs, burning): Scout
  `primary_death_headshot`, `primary_death_backstab`,
  `primary_death_burning`; all four have `dieviolent`
  (`ACT_DIEVIOLENT`, 65 f, 2.1333 s, events `AE_WPN_HIDE` at 0.0156 and
  `AE_WPN_UNHIDE` at 1.0); the Heavy also `diesimple` (`ACT_DIESIMPLE`,
  55 f, 1.8 s). None of the four guns triggers them (headshot needs
  the Sniper Rifle; burning needs fire).

## Per-frame order

Per player, on each client frame (bone setup cs_source/animation.md
§6–§10):

1. Read velocity, eye angles, ground / water / duck state, held weapon
   kind, firing and reload events.
2. Pick the main activity (§3), translate by kind, start the new
   sequence on a change (0.2 s cross-fade; carry the cycle between
   looping sequences).
3. Set `move_x` / `move_y` (§3), update the feet yaw and set
   `body_yaw`, `body_pitch` (§4).
4. Start gestures for this frame's events (fire, reload stages, land,
   flinch, Minigun pre / post fire); advance their cycles; drop finished
   ones.
5. Advance the main cycle: dt × (frames − 1 per fps rate of the blended
   cells).
6. Build the pose: default → main sequence (with its aim and noise
   autolayers) → fading main sequences → gesture layers in start order.
7. Bone-merge the weapon model; place attachments.

## Edge cases

- Standing still on a moving platform: the speed used is the player's
  own velocity (assumed as cs_source §13).
- A Scout moving faster than 400 (air control, knockback) on the ground:
  `move_*` capped at 1, playback rate 1, the feet slide.
- Spy (300 authored, 320 measured): slight slide at full speed.
- Heavy firing the Shotgun while spun is impossible (no switching while
  spun).
- Grid cell exactly at a key: no blend (cs_source §5 epsilons).
- A gesture started while airborne for reload uses the `AIRWALK`
  variants; fire has none (Open questions 3).

## Quirks

- The Soldier's airwalk picks one of five floats at random each time it
  starts, so two airborne Soldiers rarely look alike.
- Jump start for the Scout is a moving 3 × 3 grid (its cells carry
  446.8 u/s of motion), for the other three a single animation.
- The Scout's run cycle holds four strides (eight footstep events in
  1.6 s); the Soldier's one stride (two events in 0.7 s).
- `ReloadStand_PRIMARY` of the Engineer is 2.5 s long, far longer than
  a shell; it reads like a whole-magazine animation reused as the start.

## Test cases

| # | Setup | Input | After | Expected |
|---|---|---|---|---|
| 1 | Scout, Scattergun, standing | – | – | main `stand_PRIMARY` (45 f, 1.4667 s, loop) + `PRIMARY_aimmatrix_idle` + `layer_idleNoise` |
| 2 | Scout running forward at 400 | – | – | `run_PRIMARY`, `move_x` = 1, `move_y` = 0, playback 1, cycle period 1.6 s, 8 footstep events per period |
| 3 | Scout running straight left at 400 | – | – | `move_y` = −1, `move_x` = 0 (the `a_runW_PRIMARY` cell, moving +Y) |
| 4 | Scout running diagonally forward-right at 400 | – | – | (cos 45°, sin 45°) pushed to the square = (1, 1): `move_x` = 1, `move_y` = 1, cell `a_runNE_PRIMARY` |
| 5 | Scout crouch-walking at 133.3 | – | – | `crouch_walk_PRIMARY`, |move| = 133.3 / 150 = 0.889 |
| 6 | Soldier at 120 forward | – | – | `run_PRIMARY`, `move_x` = 0.5 |
| 7 | Heavy holding the Shotgun | stand | – | `stand_SECONDARY`; fire → `AttackStand_SECONDARY` (0.6333 s additive) |
| 8 | Engineer holding the Shotgun | stand | – | `stand_PRIMARY`; reload → `ReloadStand_PRIMARY`, then a/b loop (2:1), then `PRIMARY_reload_end` |
| 9 | Soldier reload start | cycle 0.3438 / 0.6562 | – | `rocket` body group 1 then 0 |
| 10 | Heavy winds up the Minigun standing | – | 0.5 s | `attackStand_PRIMARY_spoolup` done; main sequence `stand_deployed_PRIMARY`; aim layer `PRIMARY_aimmatrix_deployed` |
| 11 | Heavy spun, moving at 80 forward | – | – | `PRIMARY_deployed_movement`, `move_x` = 1 |
| 12 | Heavy firing | hold 1 s | – | `attackStand_PRIMARY` (0.6667 s) restarted after it ends |
| 13 | any class looking straight up | eye pitch −89 | – | `body_pitch` = 89 → row between `up` (45) and `straight_up` (90) |
| 14 | looking 60° down | eye pitch 60 | – | `body_pitch` = −45 (clamped) → the `down` row |
| 15 | Soldier walks off a ledge | – | – | `ACT_MP_AIRWALK_PRIMARY`: one of `airwalk_PRIMARY`, `primary_floatA`…`D` |
| 16 | Scout jumps | – | – | `Jump_Start_primary` once (0.7333 s), then `Jump_Float_primary`; on landing `jumpland_primary` additive |
| 17 | Scattergun on the Scout's player model | – | – | `c_scattergun.mdl` bone-merged; its `weapon_bone` world transform = the Scout's `weapon_bone` (child of `bip_hand_R`) |
| 18 | merged model pose parameters | – | – | names `body_pitch`, `body_yaw`, `r_hand_grip`, `r_arm`, `move_x`, `move_y`; a Soldier grid's X axis is `move_y` by name though its local index is 4 |
| 19 | head hitbox, Heavy | – | – | bone `bip_head`, group 1, (−6, −9.35, −8.7)–(6, 4.65, 4.3) |

## Open questions

1. **Mash-up kinds**: which kind to use when a class holds a gun not
   made for it (Scout with a Minigun, Heavy with a Rocket Launcher).
   Suggest: the slot the gun has for its own class (Minigun →
   `PRIMARY`), with the holder's arms; check visually.
2. The sign convention of `body_yaw` and `body_pitch` relative to the
   eye angles (cells named "right" at +45, "straight_up" at +90 suggest
   body_pitch = −eye pitch); and TF2's exact feet-yaw rules (turn
   threshold, rate). Check in the live client with `cl_showpos` and
   third person (`thirdperson` needs `sv_cheats`).
3. Airborne fire and swim / airwalk gesture variants: what the game
   plays when firing in the air.
4. The Scout's air jump: `jump_double_primary` has no activity; when is
   it played (as a gesture or main sequence)?
5. Gesture playback rate: are reload and fire gestures played at rate
   1 and cut (restarted) per round / shot, or scaled to the weapon's
   timing?
6. Footsteps: from the run events (7001 "left" / "right") or from
   distance travelled.
7. Flinch triggers: every damage, a minimum amount, or only some damage
   types; and whether the flinch is suppressed while firing.
8. The spun Heavy's speed (110) against the deployed grid's 80 u/s: is
   the playback rate raised, or do the feet slide?
9. Whether the server traces bullets against the full aim-matrix pose
   or a simplified one, and the server's bone-setup rate.
10. Ragdoll start velocity and the force from the killing hit (the
    shared Source ragdoll rules are assumed).
