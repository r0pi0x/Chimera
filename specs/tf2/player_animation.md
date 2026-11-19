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
game's rule where the code settles it. For the 2026-11-19 pass (install.md
Open questions 6 and the Heavy's spun pose), the engine's include-model
merge, activity-to-sequence pick, layer composition and sequence IK
locks were read in the same SDK, and the class models' sequence flags,
bone weights, autolayers, IK locks and IK rules were read from the
retail models; the poses quoted in §6 and the tests were computed from
the decoded animations with a throwaway reader (nothing kept).
Status: checked against the SDK's game code (2026-11-15; lookup, layers
and the Minigun's look 2026-11-19)

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
The Heavy's Minigun has its own spun-up ("deployed") main sequences,
which hold the gun level and forward where the idle holds it raised,
and spin-up / spin-down gestures whose full-body layers carry the gun
down and up and cross-fade the aim matrix. The held
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

## Corrections (2026-11-19)

- "Merging: a same-named local stub must lose" (inferred) -> the rule:
  the stubs are forward declarations (flag 0x800, no animation) and an
  include's same-named sequence takes their slot; any other duplicate
  name keeps the first (§1a).
- "0x400 hidden (not selectable by activity)" -> hidden sequences **are**
  selectable by activity (the Heavy's and Demoman's crouch, swim and
  airwalk reload gestures are hidden and carry activities); the flag
  only hides them from tool listings.
- Minigun spin gestures "own animation empty (delta, all zero)" -> the
  gesture sequence itself is non-additive with **all bone weights 0**
  (it contributes nothing); its autolayer `layer_attackStand_PRIMARY_spoolup`
  (or `…_spooldown`) is a **full-body, non-additive** pose with every
  bone weight 1, which overrides the body while it is ramped in and then
  re-adds the aim matrices (§5a, §6).
- Aim matrices and most gestures carry **foot IK locks** (both feet,
  position weight 1): an aim or gesture layer never moves the feet (§4a).
- The spun look: `stand_deployed_PRIMARY` holds the Minigun level and
  forward (barrels about 1° above horizontal), `stand_PRIMARY` holds it
  raised (about 34° up); the fire gesture changes either by about 1°
  (§6, "What it looks like").

## Units and conventions

- Hammer units, Z up. Model space: the animated character faces +X, its
  left is +Y (cs_source/animation.md conventions hold).
- Animations are sampled at their own fps; duration = (frames − 1) / fps.
  "Loop" = the looping flag. Flags: 0x1 looping, 0x2 snap (no
  cross-fade into it), 0x4 delta (additive), 0x10 post (with delta:
  applied in the bone's local frame after the base), 0x400 hidden
  (only hidden from tool listings; still selectable by activity), 0x800
  forward declaration (a stub, §1a). Animation (not sequence) flags: 0x4
  delta (values relative to identity / zero, not to the bind pose), 0x20
  all zero.
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
`medic`, `sniper`, `spy`). Merging bones by name is cs_source/animation.md
§8; which sequence a name finds is §1a. The class model's local
sequences are forward-declared stubs except `ref` and `ragdoll`
(`ACT_DIERAGDOLL`, 1 frame).

### 1a. Which sequence a name or an activity finds (install.md Open questions 6)

The engine builds one merged sequence list per model:

1. **Order**: the model's own sequences first, in file order; then each
   include in the order listed, depth first (an include's own includes
   come right after it, before the next sibling). For a class:
   `<class>.mdl`, then `<class>_user_animations.mdl`, then
   `<class>_animations.mdl`, then `<class>_workshop_animations.mdl`. For
   arms: `c_<class>_arms.mdl`, then `c_<class>_animations.mdl`.
2. **New name** (compared without case): appended at the end.
3. **Name already present**: if the sequence **currently in that slot**
   has the forward-declaration flag (0x800), the new one replaces it in
   the same slot (same merged index, the new one's flags, activity and
   weight); otherwise the new one is dropped and the earlier one stays.
   Since a replacement has no 0x800, a third same-named sequence never
   replaces it.
4. Every reference an include makes to one of its own sequences (an
   autolayer, a sequence named in an event) resolves **by name** to the
   merged slot, so it lands on whichever sequence holds that slot.
5. Animations merge by name too, but **first wins with no exception**;
   a sequence's animation reference resolves through that merged list.
   In TF2's data the only animation names that repeat are `@user_ref`
   (user and workshop includes), so this never changes a pose.

What the data has (retail build, all checked for the Scout, Soldier,
Heavy and Engineer; the arms models alike):

| Model | Local sequences | Forward-declared (0x800) | Real |
|---|---|---|---|
| `heavy.mdl` | 259 | 257 (no animation, 0 × 0 grid, no activity) | `ref` (1 frame), `ragdoll` (`ACT_DIERAGDOLL`, 1 frame) |
| `c_heavy_arms.mdl` | 42 | 41 | `c_heavy_arms` (1 frame) |
| `heavy_user_animations.mdl` | 1 (`user_ref`) | 0 | 1 |
| `heavy_animations.mdl` | 317 | 0 | 317 |
| `heavy_workshop_animations.mdl` | 75 | 0 | 75 (its `user_ref` is dropped: the user include's came first) |

So every stub is replaced by the `_animations` include's sequence of the
same name: **the include's sequence wins** for name and activity
lookups alike, and the stub is never what plays. A few stubs have no
match in any include (the Heavy's `show1`; the Soldier's `show1`,
`taunt_slit_throat`, `taunt_gimmie20` and their layers) and stay empty:
playing one shows the pose underneath unchanged.

**Activity to sequence** (each time a sequence is chosen for an
activity, main or gesture):

- Candidates are all merged sequences whose activity equals the asked
  one (hidden ones included). None: "not available"; for the main
  sequence that means the merged list's first sequence (`ref`, the
  reference pose), for a gesture nothing plays (§2).
- If the asker passes its current sequence, that sequence has the
  activity and its activity weight is **negative**, it is kept (no TF2
  class sequence has a negative weight).
- Otherwise one is drawn at random with probability |weight| / Σ|weight|
  over the candidates, walking them in merged order. Weights in the
  data: 1 almost everywhere; the Engineer's reload loops 2 (`a_`) and 1
  (`b_`), his flinches 2, 10, 5, his crouch reload starts 1 and 100, the
  Soldier's five airwalk floats 1 each.
- The main sequence then changes only when the drawn sequence's activity
  differs from the current main sequence's (§3 step 6), so a draw among
  equal-activity sequences happens once, when the activity starts.

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

**One rule for every kind** (SDK; the same for primary, secondary,
melee, PDA, building and items). The state machine (§3, §5) asks for a
kind-less activity; the held weapon's **kind table** turns it into the
kind's activity; then an item's own activity replacements apply; the
result is looked up as in §1a. Each kind table maps the movement states
(stand, crouch, run, walk, airwalk, crouch-walk, jump, jump start / float
/ land, swim, the air-jump crouch), the fire gestures (stand, crouch,
swim, airwalk), the reload gestures (start, loop, end for stand, crouch,
swim, airwalk) and the flinch to `…_<KIND>`. Kinds and their suffix:
primary `PRIMARY`; secondary `SECONDARY`; melee `MELEE`; all-class melee
`MELEE_ALLCLASS`; building `BUILDING`; PDA `PDA`; item 1 to 4 `ITEM1` …
`ITEM4`; "secondary 2" `SECONDARY2`; "primary 2" uses the `PRIMARY`
movement set but fires `ACT_MP_ATTACK_STAND_PRIMARY_ALT` (and the crouch
/ swim ALT variants). The kind is the weapon script's type unless the
item schema gives an animation slot (and a server setting can force
one). Anything a table does not list is looked up untranslated.

The **deployed** (spun / aiming) activities are the exception:

| Asked | Primary and primary-2 tables | Item 1–4 tables | Secondary, melee, building, PDA tables |
|---|---|---|---|
| deployed idle `ACT_MP_DEPLOYED_IDLE` | not listed: stays `ACT_MP_DEPLOYED_IDLE` | `ACT_MP_DEPLOYED_IDLE_ITEMn` | not listed |
| deployed move `ACT_MP_DEPLOYED` | `ACT_MP_DEPLOYED_PRIMARY` | `ACT_MP_DEPLOYED_ITEMn` | not listed: stays `ACT_MP_DEPLOYED` (no sequence has it) |
| crouched deployed idle `ACT_MP_CROUCH_DEPLOYED_IDLE` | itself | `ACT_MP_CROUCH_DEPLOYED_IDLE_ITEMn` | not listed |
| crouched deployed move | `ACT_MP_CROUCHWALK_DEPLOYED` | `ACT_MP_CROUCHWALK_DEPLOYED_ITEMn` | not listed |
| swim deployed | `ACT_MP_SWIM_DEPLOYED_PRIMARY` | – | not listed |

So the kind-less `ACT_MP_DEPLOYED_IDLE` and `ACT_MP_CROUCH_DEPLOYED_IDLE`
are carried by each model's **primary** deployed sequences
(`stand_deployed_PRIMARY`, `crouch_deployed_PRIMARY`). Models that have
them: Heavy and Pyro (stand and crouch idle, deployed movement, and
stand / crouch / swim spin-up and spin-down gestures), Sniper (the same
idles and movement plus `deployed_crouch_walk_PRIMARY`
`ACT_MP_CROUCHWALK_DEPLOYED` and an `ITEM2` deployed set for the bow),
Medic (only stand and crouch pre-fire / post-fire gestures on his
`SECONDARY`). The Scout, Soldier, Demoman, Engineer and Spy have none.
Who is "aiming": a Heavy whose Minigun spins (from the start of the
wind-up), a zoomed Sniper, a Sniper drawing a bow; never a Soldier
(his class is excluded from the aiming state whatever he holds).

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

**Rest stance** (the same rule as everything else: the include's
`stand_<KIND>`, never a stub or the reference pose; the aim matrix's
centre cell is identity and the foot locks keep the feet where the
stand put them). Feet (`bip_foot_R` / `bip_foot_L` origins, model space,
frame 0, x forward, y left) and their lateral spread |yL − yR|:

| Class | `stand_PRIMARY` | `stand_SECONDARY` | `stand_MELEE` |
|---|---|---|---|
| Scout | R (−19.5, −5.0), L (20.6, 10.9): 15.9 | (−19.3, −6.1), (20.8, 9.7): 15.9 | (−19.8, −5.1), (20.3, 10.8): 15.9 |
| Soldier | (−6.5, −8.9), (13.7, 10.5): 19.4 | (−7.6, −10.6), (12.6, 8.7): 19.4 | (−6.5, −11.0), (13.7, 8.3): 19.4 |
| Pyro | (−5.4, −10.7), (4.0, 9.4): 20.0 | same | same |
| Demoman | (−10.7, −9.9), (12.0, 8.0): 17.9 | same | (−10.5, −8.0), (12.2, 10.5): 18.5 |
| Heavy | (−7.1, −7.7), (6.7, 8.2): 16.0 | (−8.9, −6.2), (4.3, 7.3): 13.5 | (−7.5, −9.0), (7.8, 7.2): 16.2 |
| Engineer | (−6.0, −7.1), (7.8, 6.0): 13.1 | same | (−6.2, −7.7), (7.6, 5.4): 13.1 |
| Medic | (−13.1, −7.5), (7.3, 5.1): 12.6 | (−14.9, −8.9), (7.5, 7.6): 16.5 | (−15.6, −9.7), (9.9, 8.9): 18.6 |
| Sniper | (−9.9, −5.1), (5.6, 9.0): 14.1 | same | same |
| Spy | – | (−10.6, −8.9), (5.9, 6.6): 15.5 | (−10.0, −8.9), (6.5, 6.6): 15.5 |

Every class stands with the left foot forward and the feet 13–20 units
apart sideways (the Scout's 40 units front to back). The reference pose
(bind) has the feet side by side about 8–17 apart along a different
axis, so a body showing the bind or stub pose on its legs looks
narrower. The Heavy's spun stand (`stand_deployed_PRIMARY` frame 0)
widens his: R (−5.8, −7.5), L (12.2, 8.4), 24.0 apart (15.9 sideways).

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

### 4a. Foot locks and the left hand

- **Foot locks** (data: every aim-matrix sequence and most gestures of
  all four checked classes list two IK locks, chains `rfoot` and
  `lfoot`, position weight 1, local-rotation weight 0; the chains are
  hip → knee → foot). When such a sequence is layered on: before it is
  applied, each locked foot's model-space position and rotation are
  recorded; after it (and its own autolayers) is applied, each foot is
  put back: the target position is the recorded one (position weight 1),
  the hip–knee–foot chain is solved two-bone style to reach it, keeping
  the knee in the plane the chain currently bends in (the chains carry
  no knee direction), skipped when the leg is within 1° of straight;
  then the foot's model-space rotation is set to the recorded one
  (local-rotation weight 0 keeps that). Result: aim and gestures turn
  the pelvis and spine, the feet stay planted where the main sequence
  put them and the knees bend to follow. The class models' own IK
  chains are the ones used (they have no knee directions).
- The centre cells of the aim matrices are identity, so at
  `body_pitch` = `body_yaw` = 0 the locks change nothing. Off centre
  they matter: without them the aim cells move the feet as a rigid unit
  with the pelvis (Scout `up_center` 7 units, Soldier `up_center` 8–9,
  Heavy `mid_left` / `mid_right` 2–4, Engineer up to 5).
- **Left hand on the gun**: every Heavy `PRIMARY` aim cell's animation
  carries a "self" IK rule on the `lhand` chain relative to
  `weapon_bone` (the left hand is pulled to the place the cell's
  animation has it relative to the right hand's weapon bone, so blends
  of cells keep the left hand on the Minigun); the main and spin
  sequences release all four chains. This needs the client's IK
  context, which TF2's players have.

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

### 5a. How one layer is composed (main sequence, autolayer or gesture)

The same steps apply to every sequence put on the pose with a weight w
(the main sequence with w = 1, a fading main sequence with its fade
weight, a gesture with its slot weight 1, an autolayer with the weight
below), in this order:

1. Record the foot-lock positions if the sequence has IK locks (§4a).
2. Sample the sequence's own animation(s) at its cycle and blend
   parameters; bones whose own weight in the sequence's bone-weight
   list is 0 are not sampled.
3. Apply it per bone with s = w × the bone's weight: a **non-additive**
   sequence slerps rotation and lerps position from the current pose
   toward its pose by s (s capped at 1); an **additive** one scales its
   delta rotation by s (about its axis) and its delta position by s and
   combines them with the current pose: with the post flag (0x10, all
   TF2 gestures and aim matrices) the delta is applied in the bone's
   own frame after the current rotation (current × delta), without it
   before (delta × current).
4. Then each of the sequence's autolayers in order, each composed by
   these same steps with weight w × r and its own cycle. The ramp r
   comes from the autolayer's (start, peak, tail, end) on the parent's
   cycle c (or on a pose parameter, flag 0x4000): if start = end, r = 1
   and the layer runs on the parent's cycle; otherwise the layer is
   skipped when c < start or c ≥ end (so it is off at c = 1 exactly),
   r = (c − start) / (peak − start) while c < peak (when start ≠ peak),
   r = (end − c) / (end − tail) while c > tail (when end ≠ tail), else
   1; flag 0x40 eases r with a smooth spline (3r² − 2r³); its cycle is
   (c − start) / (end − start).
5. Solve the foot locks recorded in 1.

A gesture is therefore not always additive: a non-additive gesture
(Scout melee, the Minigun spin layers) overrides the pose underneath by
its bone weights, and its autolayers then add on top of that.

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
| `ACT_MP_ATTACK_STAND_PREFIRE` | `attackStand_PRIMARY_spoolup` | 16 f, 0.5 s, flags 0 (non-additive, not looping), **every bone weight 0** (its animation is flagged delta + all zero): contributes nothing itself. One autolayer, `layer_attackStand_PRIMARY_spoolup`, ramp (0, 0.667, 1, 1) spline: in over cycle 0–0.667, full until the end. That layer: 16 f, flags 0 (**non-additive full-body pose**, every bone weight 1, its own animation of the gun coming down), fade 0.7; its autolayers `PRIMARY_aimmatrix_idle` ramp (0, 0, 0, 0.333) spline (out over 0–0.333) and `PRIMARY_aimmatrix_deployed` ramp (0, 0.333, 1, 1) spline (in over 0–0.333) |
| `ACT_MP_ATTACK_STAND_POSTFIRE` | `attackStand_PRIMARY_spooldown` | 31 f, 1.0 s, flags 0, every bone weight 0; autolayer `layer_attackStand_PRIMARY_spooldown` ramp (0, 0, 0.667, 1) spline (full, out over 0.667–1). That layer: 31 f, non-additive full body (the gun going back up), fade 0.5; its autolayers `PRIMARY_aimmatrix_deployed` (0, 0, 0, 0.333) out, `PRIMARY_aimmatrix_idle` (0, 0.333, 1, 1) in |
| `ACT_MP_ATTACK_CROUCH_PREFIRE` / `POSTFIRE` | `attackCrouch_PRIMARY_spoolup` / `_spooldown` | 16 f, 0.5 s each, same build: layer `layer_attackCrouch_PRIMARY_spoolup` (ramp (0, 0, 1, 1): full from the start) / `…_spooldown` (ramp (0, 0, 1, 1)), with `PRIMARY_aimmatrix_crouch_idle` ↔ `PRIMARY_aimmatrix_deployed_crouch` cross-fades as the stand ones |
| `ACT_MP_ATTACK_SWIM_PREFIRE` / `POSTFIRE` | `attackSwim_PRIMARY_spoolup` / `_spooldown` | 16 f, 0.5 s, non-additive with every bone weight 1 (a full-body pose of its own), no layers |
| `ACT_MP_DEPLOYED_IDLE` | `stand_deployed_PRIMARY` | 61 f, 2.0 s, loop; aim `PRIMARY_aimmatrix_deployed` |
| `ACT_MP_DEPLOYED_PRIMARY` | `PRIMARY_deployed_movement` | 3 × 3, 24 f, 0.7667 s, loop, 80 u/s; footsteps 0.2083 / 0.7083 |
| `ACT_MP_CROUCH_DEPLOYED_IDLE` | `crouch_deployed_PRIMARY` | 16 f, 0.5 s, loop (no crouched deployed movement: a spun Heavy can't move crouched, weapons.md 7.1) |
| `ACT_MP_SWIM_DEPLOYED_PRIMARY` | `PRIMARY_swim_deployed_movement` | 3 × 3, 31 f, 1.0 s, 80 u/s |
| `ACT_MP_ATTACK_STAND_PRIMARY` | `attackStand_PRIMARY` | 21 f, 0.6667 s, additive post (0x14), every bone weight 1: the firing recoil gesture (`attackCrouch_PRIMARY` 16 f, weights tapering from 1 at the upper spine to 0.1 at the pelvis) |

Every sequence in this table, the aim matrices and the main deployed
sequences are in `heavy_animations.mdl`; `heavy.mdl` has only 0x800
stubs of the same names, which never play (§1a).

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

**What it looks like** (decoded poses, model space: +X forward, +Y
left, +Z up, origin between the feet on the floor; `body_pitch` =
`body_yaw` = 0 unless said; "barrels" = `weapon_bone`'s +Z axis, which
carries the Minigun's barrels; muzzle = `weapon_bone` × (0, −7.76,
57.76)):

| Pose | `weapon_bone` origin | Barrels | Elevation | Muzzle |
|---|---|---|---|---|
| `stand_PRIMARY` frame 0 (+ `PRIMARY_aimmatrix_idle`, identity at the centre) | (−5.1, −14.7, 43.8) | (0.81, 0.18, 0.56) | 34° up, 13° left | (46.0, −6.7, 70.5) |
| `stand_deployed_PRIMARY` frame 0 (+ `PRIMARY_aimmatrix_deployed`) | (−8.6, −13.7, 41.4) | (1.00, 0.02, 0.02) | 1° up | (49.4, −14.2, 35.2) |
| deployed + `attackStand_PRIMARY` frame 6 (cycle 0.3) | (−8.9, −13.0, 40.8) | (1.00, 0.01, 0.03) | 2° | – |
| `layer_attackStand_PRIMARY_spoolup` frames 0 / 5 / 10 / 15 | – | – | 34° / 12° / 2° / 8° | – |
| `layer_attackStand_PRIMARY_spooldown` frames 0 / 15 / 30 | – | – | 8° / 26° / 34° | – |

Over its cycle `stand_PRIMARY` stays at 33–34° and `stand_deployed_PRIMARY`
at 0.9–1.4°. So the Heavy at rest carries the Minigun **raised** (barrels
up and slightly left, muzzle above head height), and from the start of
the wind-up he **lowers it level and forward** (the deployed main
sequence), firing adds only a shake of about 1° and under a unit; the
fire gesture is the same whether spun or not. Aim follows the view
much more closely when spun: at the `up` row (`body_pitch` 45) the
barrels point 48° up spun but 39° at rest, at the `down` row (−45) −25°
spun but +25° at rest; the `straight_up` row (90) points them 78° up
spun, 47° at rest. Correct poses come only from playing **both** the
deployed main sequence **and** its own aim matrix with the right
`body_pitch`: an aim matrix driven to its `straight_up` row (`body_pitch`
90, e.g. a pitch parameter mapped by index instead of name, or a sign
error) raises the spun gun nearly vertical.

Spin-up and spin-down as the layers compose them (the main sequence
cross-fading over 0.2 s, the gesture on top; elevation of the barrels):

| t after the wind-up starts | 0 | 0.1 | 0.2 | 0.3 | 0.4 | 0.49 | 0.5 (gesture over) |
|---|---|---|---|---|---|---|---|
| main only | 34° | 18° | 1.4° | 1.4° | 1.3° | 1.3° | 1.3° |
| with the spin-up layer (its weight) | 34° (0) | 19° (0.22) | 5° (0.65) | 2° (0.97) | 5° (1) | 8° (1) | 1.3° |

| t after the wind-down starts | 0 | 0.2 | 0.4 | 0.6 | 0.7 | 0.8 | 0.9 | 1.0 |
|---|---|---|---|---|---|---|---|---|
| main only (back to `stand_PRIMARY`) | 1.3° | 34° | 34° | 34° | 34° | 33° | 33° | 33° |
| with the spin-down layer (its weight) | 8° (1) | 9° (1) | 20° (1) | 33° (1) | 36° (0.97) | 35° (0.65) | 34° (0.22) | 33° (0) |

(Layer weights are the splined ramps of §5a; the main cross-fade is
taken as linear over 0.2 s; the layer's own aim cross-fade is identity
at the centre.) The spin-down layer holds the gun low for the first 0.2–0.4 s
although the main sequence has already gone back to the raised idle:
that is how TF2's Heavy keeps the gun forward while the barrels run
down.

First person follows the same pattern with the arms' own sequences
(viewmodel.md §7): `m_idle` raised, `m_spool_up` lowering it, `m_fire` /
`m_spool_idle` level, `m_spool_down` raising it again; the arms' stubs
resolve as §1a.

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
| 20 | merged `heavy.mdl` | look up `stand_PRIMARY`, `PRIMARY_aimmatrix_deployed`, `attackStand_PRIMARY_spoolup` by name | – | each resolves to the `heavy_animations.mdl` sequence (61 frames; 3 × 4 grid of 1-frame cells; 16 frames), at the stub's merged index (the stub's position in `heavy.mdl`); none is 1 frame or empty |
| 21 | merged `heavy.mdl` | activity `ACT_MP_STAND_PRIMARY` | – | `stand_PRIMARY` from the include (the stub has no activity) |
| 22 | merge rule, synthetic models | A has `x` with 0x800; include B has `x` (no flag); include C has `x` | – | merged `x` is B's, at A's index; C's dropped |
| 23 | merge rule, synthetic models | A has `x` without 0x800 (1 frame); include B has `x` with an activity | – | merged `x` stays A's (no activity); B's dropped |
| 24 | merged `heavy.mdl` | `user_ref` | – | the user include's (the workshop include's is dropped) |
| 25 | weighted pick | Engineer `ACT_MP_RELOAD_STAND_PRIMARY_LOOP`, 30 000 seeded draws | – | `a_PRIMARY_reload_loop` 2/3 ± 0.01, `b_…` 1/3 |
| 26 | hidden + activity | Heavy, Shotgun, ducked reload | – | `ACT_MP_RELOAD_CROUCH_SECONDARY` finds `SECONDARY_crouch_reload` though it is hidden |
| 27 | Heavy, Minigun, idle | `body_pitch` = `body_yaw` = 0, frame 0 | – | `weapon_bone` at (−5.1, −14.7, 43.8) ± 0.2, barrels (0.81, 0.18, 0.56) ± 0.02 (34° up); muzzle (46.0, −6.7, 70.5) ± 0.5 |
| 28 | Heavy spun, still | same | 0.5 s after the wind-up | main `stand_deployed_PRIMARY`; `weapon_bone` (−8.6, −13.7, 41.4) ± 0.2, barrels (1.00, 0.02, 0.02) ± 0.02 (level); muzzle (49.4, −14.2, 35.2) ± 0.5 |
| 29 | Heavy spun, firing | hold attack | any time | barrels' elevation 0–3° (never above 10° at `body_pitch` 0); `weapon_bone` within 2.5 units of test 28's (the idle's own sway included) |
| 30 | Heavy spun | `body_pitch` 45 / −45 / 90 | – | barrels 48° / −25° / 78° up (idle: 39° / 25° / 47°) ± 2° |
| 31 | Heavy wind-up, standing | – | t = 0.1 / 0.2 / 0.3 / 0.49 s | spin-up layer weight 0.22 / 0.65 / 0.97 / 1 (splined ramp on cycle t / 0.5); barrels 19° / 5° / 2° / 8° ± 2° |
| 32 | Heavy wind-down | – | t = 0.2 / 0.4 / 0.6 / 0.8 s | main `stand_PRIMARY`; barrels 9° / 20° / 33° / 35° ± 2°; slot free at 1.0 s |
| 33 | spin gesture's own sequence | `attackStand_PRIMARY_spoolup` alone at weight 1 over any pose | – | the pose is unchanged (all bone weights 0): only its layer moves bones |
| 34 | foot locks | Soldier `stand_PRIMARY` + `PRIMARY_aimmatrix_idle` at `body_pitch` 45 | – | feet within 0.1 of `stand_PRIMARY`'s own (without locks they move 7.7 / 9.0); the knees re-bend |
| 35 | rest stance | each class, `stand_<KIND>` frame 0 | – | feet as in the §3 "Rest stance" table ± 0.2 |
| 36 | Scout with the Minigun (mash-up), spun | – | – | real TF2: `ACT_MP_DEPLOYED_IDLE` not in the Scout's model → reference pose (Open questions 13 for ours) |

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
12. **Stance width against ours** (2026-11-19 brief: TF2's idle stances
    look slightly wider than Chimera's): TF2's rest stance is the
    include's `stand_<KIND>` (table in §3) with nothing on the legs at
    a level view, so it follows from the same lookup rule; aiming up or
    down keeps the feet only through the foot locks (§4a), which
    Chimera lacks. Whether ours differs at a level view (and why) is to
    measure against test 35; not confirmed from here.
13. **Deployed state on a class without deployed sequences** (mash-ups:
    a Scout spinning the Minigun): TF2 would show the reference pose
    (§2, `ACT_MP_DEPLOYED_IDLE` missing); Chimera may prefer to keep the
    kind's normal set and add only the fire gesture. A design choice,
    not parity.
9. Whether the server traces bullets against the full aim-matrix pose
   or a simplified one, and the server's bone-setup rate.
10. Ragdoll start velocity and the force from the killing hit (the
    shared Source ragdoll rules are assumed).
