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
compilation options (sequence and autolayer options). For the
2026-11-15 check, the SDK's TF2 game code (Valve's Source SDK 2013 with
TF2's client and server code, read in place on the Linux box, nothing
copied) was read for how the game chooses the main activity, sets the
move and aim pose parameters and the feet yaw, and plays gestures
(fire, reload, jump, flinch, the Minigun's spin), and for the footstep
rule. What the earlier draft marked inferred is now stated as the
game's rule where the code settles it.
Status: checked against the SDK's game code (2026-11-15)

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

## Corrections from the SDK (2026-11-15)

- Kind "the item's slot for the holding class" -> the **weapon's own**
  `WeaponType` (its script), unless the item schema gives an animation
  slot; a mash-up gun keeps its own kind (Open questions 1 settled).
- Main-activity order "swim first" -> jump/airwalk, then **duck**, then
  swim (water at waist or deeper), then run/stand; "moving" is
  horizontal speed > 0.5 u/s.
- Airwalk "airborne without a jump (walked off a ledge, blast-jumped)"
  -> only while rising faster than **300 u/s** (or already in it), not
  ducked; walking off a ledge keeps the ground activity.
- Jump "start plays once, then float" -> `JUMP_START` for the first
  **0.5 s** after the jump, then `JUMP_FLOAT`; landing is checked only
  after 0.2 s; the land gesture plays on landing from a jump or an
  airwalk.
- Feet yaw "standing: feet stay until > 45°" -> standing, once the eye
  is more than 45° from the feet's goal, the goal moves **45° toward**
  the eye (not onto it); moving (any 3D speed > 1), the goal is the eye
  yaw; turn rate 720 °/s × clamp(|Δ| / 60, 0.01, 1) (Open questions 2
  settled).
- `body_pitch` "inferred" -> = −eye pitch; `body_yaw` = −(eye yaw − feet
  yaw), normalised.
- Move parameters "model space" -> the movement direction relative to
  the **eye** yaw; the ground speed is the current sequence's at the
  pushed-out pose; main playback rate stays 1 (feet slide; Open
  questions 8 settled).
- Fire gesture "airborne: open" -> swim if swimming, else crouch if
  ducked, else stand (airborne included); `AIRWALK` fire is never used
  (Open questions 3 settled).
- Reload "start once, loop per round, end" -> the reload-stand gesture
  starts with the **first round** (after the view model's start), the
  loop restarts at cycle 0 for each later round, the end after the
  last; airwalk variants only while in airwalk (Open questions 5
  settled: rate 1, restarted, not scaled).
- Flinch "every damage, inferred" -> on any non-generic damage while
  alive and not disguised; chosen by the hit group, else chest; not
  restarted while one plays (Open questions 7 settled).
- Footsteps "events or distance" -> both: the run's 7001 events force a
  step sound, and a timed rule plays them too (Open questions 6
  settled).
- Minigun deployed set "after the wind-up" -> from the **start** of the
  wind-up (the spun condition is set with the pre-fire gesture); a spun
  Heavy shows no jump or airwalk; ducked and moving spun, the plain
  crouch-walk.

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

**Which kind each M2 gun uses**. The game (SDK) picks the activity set
from the **weapon's script `WeaponType`** (`primary`, `secondary`,
`melee`, `building`, `pda`, `item1`, …), replaced by the item schema's
animation slot when the item has one (no stock gun among these does;
the Grenade Launcher does, see weapons.md). The class shotguns are
separate weapons whose scripts say primary (Engineer) or secondary
(Soldier, Pyro, Heavy), which is why the table matches the class
slots:

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
class not in this table (a mash-up) keeps **its own kind** (a Scout with
the Minigun uses `PRIMARY`, a Heavy with the Rocket Launcher
`PRIMARY`), animated with the holder's sequences for that kind. If the
holder's model has no sequence for the translated activity, the game
falls back to the model's first sequence (the reference pose); and a
ducked player whose kind has no crouch-walk sequence is treated as not
ducked.

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
events or from movement distance: both (Quirks, "Footsteps").

**Choosing the main sequence** (SDK), each client frame; "moving" means
horizontal speed > 0.5 u/s; "ducked" means the duck flag is set and the
kind has a crouch-walk sequence; "in water" means water at waist level
or deeper:

1. **Airwalk** (classes whose class data does not set `DontDoAirwalk`):
   if not ducked and either the vertical velocity is above +300 u/s or
   the player is already in airwalk: on the ground → leave airwalk,
   restart the main sequence at cycle 0 and play `ACT_MP_JUMP_LAND` in
   the jump gesture slot; in water → leave airwalk, restart the main
   sequence; else in the air → `ACT_MP_AIRWALK` (or a falling-stomp
   activity for items with that ability, when falling faster than the
   safe fall speed). A spun Heavy (Minigun spinning) skips this step and
   step 2 entirely.
2. **Jump** (else, if a jump started): on the jump event the state
   starts (time t0, main sequence restarted at cycle 0). In water →
   end it, restart the main sequence. After 0.2 s, on the ground → end
   it, restart the main sequence, play `ACT_MP_JUMP_LAND` in the jump
   slot. While it lasts: `ACT_MP_JUMP_START` while now − t0 ≤ 0.5 s,
   then `ACT_MP_JUMP_FLOAT`. (A class whose data sets `DontDoNewJump`
   uses `ACT_MP_JUMP` throughout and gets no land gesture.)
3. **Ducked**: not moving → `ACT_MP_CROUCH` (the deployed crouch idle
   while spun or within the deployed hold); moving → `ACT_MP_CROUCHWALK`
   (the Scout with air dashes used: `ACT_MP_DOUBLEJUMP_CROUCH`). Ducking
   wins over swimming.
4. **In water** → `ACT_MP_SWIM` (spun Minigun: `ACT_MP_SWIM_DEPLOYED`);
   entering water restarts the main sequence.
5. Else **moving** → `ACT_MP_RUN`, standing → `ACT_MP_STAND` (spun:
   `ACT_MP_DEPLOYED` / `ACT_MP_DEPLOYED_IDLE`). There is no walk
   activity.
6. Translate by the weapon kind (`ACT_MP_RUN` → `ACT_MP_RUN_PRIMARY`;
   swimming also turns stand fire/reload into the swim ones). The main
   sequence changes only when the translated activity differs from the
   current sequence's activity; then one sequence is picked at random
   weighted by activity weight (Soldier's airwalk: five equal choices).
   Staying in the same activity never re-rolls.

Walking off a ledge or falling without a jump is therefore **not**
airwalk: the ground activity (run or stand) goes on in the air. A normal
jump (≈ 289 u/s up) is below the airwalk threshold; blast jumps and
launches above 300 u/s airwalk.

**Move parameters** (SDK): the movement yaw is the horizontal velocity's
direction when moving (when exactly still it drifts toward the body's
yaw, which only matters for the next start). With d = movement yaw −
eye yaw, normalised (relative to the **eye**, not the rendered feet),
(`move_x`, `move_y`) = (cos d, −sin d) pushed out to the unit square
(divided by max(|x|, |y|)); G = the current main sequence's ground speed
at that pushed-out pose; if the horizontal speed is below G both are
scaled by speed / G; never above 1. Not moving (speed ≤ 0.5): both 0.
The main sequence's playback rate stays 1 (the game never scales it to
speed), so above G the feet slide. Because G equals the class's top
speed, a running player at top speed has |move| = 1 and his feet match
the ground; a Scout crouch-walking at 133 (a third of 400) has |move| =
133 / 150 = 0.89.

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
- So `body_pitch` = +90 aims straight up and −45 is the lowest aim. The
  game (SDK) sets body_pitch = −(eye pitch, normalised to ±180) with
  Source's "positive pitch looks down" convention; the pose parameter's
  own range clamps it to −45 … 90, so looking more than 45° down shows
  the 45° pose. `body_yaw` = +45 is the cell named "right".
- **Feet yaw** (the rendered model's yaw, F) and its goal Fg, each
  frame (SDK):
  - Moving (3D speed > 1 u/s): Fg = eye yaw E.
  - Standing: on the first update after spawn (or after a "snap yaw"
    event) Fg = F = E. Otherwise, if |normalise(Fg − E)| > 45°, Fg moves
    45° toward E (Fg += 45° × sign(E − Fg)); the feet stay put inside
    ±45°.
  - Then, if F ≠ Fg, F turns toward Fg by 720 °/s × dt × k, k =
    clamp(|Fg − F| / 60, 0.01, 1), taking the shorter way round; if that
    step is at least the remaining gap, F = Fg. (Quirk: the |Fg − F| in
    k is taken before wrapping to ±180, so across the ±180 seam the
    turn runs at full rate.)
  - While taunting, in a kart or on the match-summary stage the goal is
    forced and F jumps to it.
- body_yaw = −normalise(E − F): the torso's twist from the feet, clamped
  by the pose parameter to ±45. Turning the view right (yaw decreasing)
  makes E − F negative and body_yaw positive, the "right" cells. On the
  server the player entity's yaw is F as well.
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
sequence (cs_source/animation.md §10). The game (SDK) keeps **one
gesture per slot**, the slots layered in this order: attack-and-reload
(fire, reload, Minigun pre/post fire), grenade, jump (land, air jump),
swim, flinch, scene, custom (stun and the like). Rules:

- Starting a gesture whose activity is already playing in its slot
  restarts it at cycle 0; starting a different one replaces it (the old
  one's remaining events are fired first, on the client).
- Weight 1, no blend in or out; playback rate 1 (× item attributes:
  a gesture-time attribute for all, the reload-time attributes as
  1 / value for reloads, clamped to −4 … 12). Never scaled to the
  weapon's timing.
- A gesture ends (the slot frees) when its cycle passes 1, except a
  non-Minigun pre-fire gesture, which holds its last frame.
- The variant is chosen when the gesture starts.

Fire (`ACT_MP_ATTACK_STAND_K`, `…_CROUCH_K`, `…_SWIM_K`): on each shot,
the swim variant if the player is in the swim state, else the crouch
variant if ducked, else the stand variant, **also in the air**
(`ACT_MP_ATTACK_AIRWALK_*` exists in the activity tables but is never
asked for). Restarted at cycle 0 on every shot, except the Minigun's
(§6):

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
CROUCH, SWIM, AIRWALK). Variant when it starts: airwalk if the player is
in airwalk; else crouch if the duck flag is set; else swim if swimming;
else stand. A one-at-a-time reload (SDK): the weapon's reload start runs
on the view model only; when the **first round** begins, the reload
gesture (the "start" sequence) plays; each **later round** restarts the
loop gesture at cycle 0; after the last round the end gesture plays. A
whole-clip reload plays only the reload gesture.

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
  (weapons.md §4: Scattergun first shell 0.7 s then 0.5 s): the game
  restarts the loop for each round at rate 1 and does not fit the rate,
  so a loop longer than a round is cut and a shorter one holds its end
  until the next round. Crouch / swim / airwalk variants have the same shape
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

Jump land: `ACT_MP_JUMP_LAND_K` (additive, 0.6667–1.0 s) in the jump
slot on landing from a jump (after at least 0.2 s) or from airwalk (§3).

Air jump (Scout): each air jump starts the jump state if it is not
already on (t0 = now, main sequence restarted), turns airwalk off and
plays `ACT_MP_DOUBLEJUMP` (translated by kind) in the jump slot. If the
model has no sequence for that activity nothing plays (whether
`jump_double_primary` carries it is not in the dump: Open questions 4).

Flinch (`ACT_MP_GESTURE_FLINCH_CHEST`, the only flinch activity in the
models): Scout and Soldier `a_flinch01` 13 f, 0.4 s; Heavy one per kind
(`PRIMARY_flinch`, `SECONDARY_flinch`, `MELEE_flinch`, 0.4 s);
Engineer `a_flinch01` (weight 2, 0.4 s), `a_flinch02` (weight 10,
0.3333 s), `a_flinch04` (weight 5, 0.4 s). When played (SDK): on each
damage the player takes whose damage type is not plain generic, while
alive and not disguised (and not a fully charged scoped Sniper with a
no-flinch item). The server also sets the victim's view punch pitch to
−2° (a kick up) at the same moment. The activity is chosen from the hit
group (head, left/right arm, left/right leg; anything else chest) and
falls back to chest when the model lacks it, which in TF2's models is
always. A flinch is not started while another is still playing in the
flinch slot.

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

Sequence of states (SDK, matched to weapons.md 7.1):

- **Wind-up starts**: the spun ("aiming") state begins at once, so the
  main sequence switches to the deployed set immediately (stand →
  `ACT_MP_DEPLOYED_IDLE`, moving → `ACT_MP_DEPLOYED`, swimming →
  `ACT_MP_SWIM_DEPLOYED`, ducked and still → `ACT_MP_CROUCH_DEPLOYED_IDLE`,
  ducked and moving → the **plain** `ACT_MP_CROUCHWALK`, since the Heavy
  has no deployed crouch-walk). The PREFIRE gesture for the stance
  (swim, else crouch, else stand) plays in the attack slot and ends
  when done. While spun the jump and airwalk activities are skipped.
- **Firing**: on each shot the fire gesture for the stance (swim, else
  crouch, else stand) starts **only if it is not already playing**, so
  it runs to its end and starts again on the next shot after that.
- **Wind-down**: the POSTFIRE gesture for the stance; the spun state
  ends and the normal main sequences return.
- Spun movement: the deployed grid's 80 u/s against the spun speed 110:
  the move parameters reach 1 and the playback rate stays 1, so the feet
  slide a little at full spun speed (no rate scaling in the game).

The same deployed set serves a scoped Sniper; after a scoped shot the
deployed idle is also held for 2 s unless he moves (the "deployed
hold" of §3). The weapon model's own
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
   sequence only when the activity changes (0.2 s cross-fade); restart
   it at cycle 0 on the §3 events (jump start, landing, entering water,
   leaving airwalk).
3. Advance the gestures' cycles (rate 1 × attributes); free the slots
   whose cycle passed 1 (§5).
4. Set `move_x` / `move_y` (§3), then `body_pitch`, then the feet yaw
   and `body_yaw` (§4). Gestures for events (fire, reload stages, land,
   air jump, flinch, Minigun pre / post fire) start when the events
   arrive, between frames.
5. Advance the main cycle: dt × (frames − 1 per fps rate of the blended
   cells).
6. Build the pose: default → main sequence (with its aim and noise
   autolayers) → fading main sequences → gesture layers in slot order
   (attack-and-reload, grenade, jump, swim, flinch, scene, custom).
7. Bone-merge the weapon model; place attachments.

## Edge cases

- Standing still on a moving platform: the speed used is the player's
  own velocity (assumed as cs_source §13).
- Walking off a ledge: no airwalk and no jump: the run or stand activity
  continues through the fall; on landing nothing restarts and no land
  gesture plays.
- Ducked in water: the crouch activities win over swim (§3 order).
- Rocket jump: rising above 300 u/s → airwalk (also if the jump event
  came first: airwalk is checked before the jump state); landing plays
  the land gesture.
- Turning the view while standing: the feet hold until the view is 45°
  off the goal, then the goal steps 45° toward it and the feet swing
  there at up to 720 °/s, slowing inside 60°.
- A Scout moving faster than 400 (air control, knockback) on the ground:
  `move_*` capped at 1, playback rate 1, the feet slide.
- Spy (300 authored, 320 measured): slight slide at full speed.
- Heavy firing the Shotgun while spun is impossible (no switching while
  spun).
- Grid cell exactly at a key: no blend (cs_source §5 epsilons).
- A reload gesture started while in airwalk uses the `AIRWALK`
  variants; one started in an ordinary jump uses the stand (or crouch)
  variants. Fire never uses an airwalk variant.
- A kind without a crouch-walk sequence: ducking is ignored for the
  main activity and for gesture variants.

## Quirks

- The Soldier's airwalk picks one of five floats at random each time it
  starts, so two airborne Soldiers rarely look alike.
- Jump start for the Scout is a moving 3 × 3 grid (its cells carry
  446.8 u/s of motion), for the other three a single animation.
- The Scout's run cycle holds four strides (eight footstep events in
  1.6 s); the Soldier's one stride (two events in 0.7 s).
- `ReloadStand_PRIMARY` of the Engineer is 2.5 s long, far longer than
  a shell; it reads like a whole-magazine animation reused as the start.
  Since the second round restarts the loop gesture, only its first round
  or so is ever seen.
- The jump start is cut at 0.5 s whatever its length (the Scout's is
  0.7333 s).
- The feet-turn rate's slow-down uses the unwrapped angle difference,
  so a turn across ±180° runs at full rate.
- Footsteps (SDK): each 7001 event of the run cycle forces a footstep
  sound at once; independently, the shared timed footstep rule plays
  steps every T ms while moving fast enough, T = 400 … 200 for a class
  maximum speed of 200 … 450 u/s (linear, clamped), + 100 when walking
  (below 0.8 of the maximum speed, above 0.3) and + 100 ducked or on a
  ladder; on a ladder 350, in water to the knees 600 … 400.

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
| 10 | Heavy winds up the Minigun standing | – | 0 / 0.5 s | main sequence `stand_deployed_PRIMARY` from t = 0; `attackStand_PRIMARY_spoolup` in the attack slot, done (slot free) at 0.5 s; aim layer `PRIMARY_aimmatrix_deployed` |
| 11 | Heavy spun, moving at 80 forward | – | – | `PRIMARY_deployed_movement`, `move_x` = 1 |
| 12 | Heavy firing | hold 1 s | – | `attackStand_PRIMARY` (0.6667 s) not restarted by shots while playing; restarted by the first shot after it ends |
| 12b | Heavy spun, ducked, moving | – | – | `crouch_walk_PRIMARY` (no deployed crouch-walk) |
| 12c | Heavy spun | jump | – | no jump activity: the deployed set stays |
| 13 | any class looking straight up | eye pitch −89 | – | `body_pitch` = 89 → row between `up` (45) and `straight_up` (90) |
| 14 | looking 60° down | eye pitch 60 | – | `body_pitch` = −45 (clamped) → the `down` row |
| 15 | Soldier rocket-jumps (rising at 600 u/s) | – | – | `ACT_MP_AIRWALK_PRIMARY`: one of `airwalk_PRIMARY`, `primary_floatA`…`D`; on landing `jumpland_PRIMARY` in the jump slot |
| 15b | Soldier walks off a ledge | – | – | stays `run_PRIMARY` (or `stand_PRIMARY`) while falling; no land gesture |
| 16 | Scout jumps (289 u/s up), lands at 0.6 s | – | 0.3 / 0.55 / 0.6 s | `Jump_Start_primary` / `Jump_Float_primary` / landed: main restarted, `jumpland_primary` additive |
| 16b | jump, still on the ground flag at 0.1 s | – | 0.1 s | still jumping (ground ignored for 0.2 s) |
| 16c | standing, eye yaw turns from 0 to 50° at once | – | – | goal 45°, feet turn at 720 × 0.75 = 540 °/s at first, slowing; body_yaw = −(50 − F) |
| 16d | standing, eye yaw 30° from the feet | – | – | feet do not move; body_yaw = −30 |
| 16e | Engineer reload of 3 shells | – | – | `ReloadStand_PRIMARY` at the 1st shell, the a/b loop restarted at the 2nd and 3rd, `PRIMARY_reload_end` after |
| 16f | Scout fires the Scattergun in mid-jump | – | – | `AttackStand_PRIMARY` (stand variant) |
| 17 | Scattergun on the Scout's player model | – | – | `c_scattergun.mdl` bone-merged; its `weapon_bone` world transform = the Scout's `weapon_bone` (child of `bip_hand_R`) |
| 18 | merged model pose parameters | – | – | names `body_pitch`, `body_yaw`, `r_hand_grip`, `r_arm`, `move_x`, `move_y`; a Soldier grid's X axis is `move_y` by name though its local index is 4 |
| 19 | head hitbox, Heavy | – | – | bone `bip_head`, group 1, (−6, −9.35, −8.7)–(6, 4.65, 4.3) |

## Open questions

1. ~~Mash-up kinds~~ Settled: the weapon's own `WeaponType` kind.
   Open: what a holder whose model lacks that kind's sequences should
   show (the game shows the reference pose; Chimera may prefer a
   fallback kind).
2. ~~Signs and feet-yaw rules~~ Settled (§4).
3. ~~Airborne fire~~ Settled: stand (or crouch / swim) variant.
4. **The Scout's air jump**: the game plays `ACT_MP_DOUBLEJUMP` in the
   jump slot; which sequence carries that activity (the dump shows
   `jump_double_primary` with none) is a data question: re-dump the
   activity list of `scout_animations.mdl`.
5. ~~Gesture playback rate~~ Settled: rate 1, restarted per round / shot.
6. ~~Footsteps~~ Settled: both (Quirks).
7. ~~Flinch triggers~~ Settled (§5): any non-generic damage, not while
   disguised; not suppressed by firing.
8. ~~Spun speed against the grid~~ Settled: the feet slide.
11. The class-data flags `DontDoAirwalk` and `DontDoNewJump` (in the
    class scripts): which classes set them; read with the class-scripts
    spec.
9. Whether the server traces bullets against the full aim-matrix pose
   or a simplified one, and the server's bone-setup rate.
10. Ragdoll start velocity and the force from the killing hit (the
    shared Source ragdoll rules are assumed).
