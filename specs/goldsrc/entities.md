# GoldSrc: map entities and their logic (CS 1.6's stock maps)

Source basis: the entity lumps of all 25 BSP files in `cstrike/maps/` of
the HLDS install (read with a throwaway script outside the repository:
class counts, every key's values, model bounds, which faces of each brush
entity carry `+0`/`+a` textures, origins against the floor); the
level-design file that ships with the server, `cstrike/halflife-cs.fgd`
(key names, choices and flag names are data the editor exposes); public
level-design documentation of Half-Life 1 entities; and, as reference
for this spec session only, Valve's public Half-Life SDK (the game code
of Half-Life, not of Counter-Strike). CS 1.6's own game code is not
public; where it may differ from Half-Life's (round restarts, the
objective entities, `+use` on doors) this spec says so and the point is
listed under Open questions for measurement. No engine source, no
ReGameDLL_CS or ReHLDS, no decompiled or leaked code.
Status: draft

## Summary

A GoldSrc map's logic is a graph of named entities. An entity "fires" its
`target` by name: every entity whose `targetname` equals it is "used"
(told to act), with the player who started the chain carried along as the
activator. `delay` postpones a fire, `killtarget` removes entities,
`multi_manager` fires several names on a timetable and `multisource`
gates others as their `master`. Moving brushes (doors, buttons, trains,
rotators) move at a set speed to a set position, wait, and come back;
when a player is in the way they hurt him (`dmg`) and usually reverse.
Buttons and `func_wall` switch the faces of their brush model to the
alternate texture set (`+a`..`+j`) by setting their frame to 1. Most of
what is drawn as decoration (`func_wall`, `func_illusionary`,
`env_sprite`, `env_glow`) has no logic beyond its render keys.

## Units and conventions

As [bsp.md](bsp.md) "Units and conventions": the unit (inch), Z up,
angles "pitch yaw roll" in degrees, `angle` alone is a yaw with -1 up and
-2 down. The forward vector of angles (p, y) is (cos p · cos y,
cos p · sin y, −sin p): positive pitch looks down, so "-90 0 0" is
straight up.

- **Time** is server time in seconds. Delays, waits and thinks are in
  seconds and run on the server's frame (CS 1.6's server frame rate is
  `sys_ticrate`, 100 by default; measure, cs16 movement spec). A moving
  brush keeps its own clock (below) for its move and wait timers.
- **Spawnflags** are the values the editor shows (bit values, summed).
- **Brush size** of a brush entity is its model's bounding box from the
  models lump (bsp.md "Models"): size = maxs − mins per axis. A brush
  entity without an `origin` key has its model in world coordinates (its
  entity origin is 0 0 0); one with an `origin` key (rotating entities,
  built with an `origin` brush) has its model around that point.
- **Names** (`targetname`, `target`, `killtarget`, `master`, `netname`)
  match exactly, case and spaces included (`bulb glow`, `flor green` are
  real names). The stock maps have no reference that matches only
  case-insensitively.
- **Keys** follow bsp.md: the last of a repeated key wins; key names are
  matched as written in this spec (the stock maps write them so).
- Names in this spec (state names, "fire", "use", "activator") are ours.

## Constants

| Name (ours) | Value | Unit | Meaning |
|---|---|---|---|
| door_default_speed | 100 | units/s or °/s | `speed` 0 on doors, rotating doors, `func_water` |
| door_size_shrink | 2 | units | taken off each axis of the brush size before the lip |
| button_default_speed | 40 | units/s | `speed` 0 on `func_button`, `func_rot_button` (°/s) |
| button_default_wait | 1 | s | `wait` 0 on buttons |
| button_default_lip | 4 | units | `lip` 0 on `func_button` |
| move_snap_tolerance | 1/32 | units | a finished linear move further than this from its goal moves again |
| move_resnap_speed | 100 | units/s | speed of that corrective move |
| door_lock_sound_gap | 3 | s | min time between a door's locked/unlocked sounds |
| button_lock_sound_gap | 0.5 | s | same for buttons |
| trigger_default_wait | 0.2 | s | `trigger_multiple` `wait` 0 |
| trigger_remove_delay | 0.1 | s | a fired one-shot trigger is removed this long after |
| hurt_interval | 0.5 | s | `trigger_hurt` applies half its per-second damage this often |
| multi_manager_max | 16 | targets | further keys are ignored |
| multisource_max | 32 | inputs | |
| multisource_setup | 0.1 | s | a `multisource` counts its inputs this long after spawn |
| auto_trigger_delay | 0.1 | s | `trigger_auto` fires this long after spawn |
| use_radius | 64 | units | `+use` searches entities whose box is within this of the player's origin |
| use_cone | 0.7 | cos | ... and in front of the view by at least this dot product (about 45°) |
| teleport_lift | 1 | units | added to the destination height |
| rotating_start_delay | 1.5 | s | a "Start ON" `func_rotating` starts this long after spawn |
| rotating_ramp_step | 0.1 | s | spin-up/down step of `func_rotating` |
| train_default_speed | 100 | units/s | `func_train` |
| train_default_dmg | 2 | hp | `func_train` crush damage |
| train_block_gap | 0.5 | s | `func_train` hurts a blocker at most this often |
| plat_default_volume | 0.85 | 0-1 | `func_train` sound volume |
| breakable_shard_life | 2.5 | s | debris pieces' life (sent to clients) |
| breakable_touch_factor | 0.01 | hp per unit/s | "Touch" breakables: damage = toucher speed × this |
| explosion_radius_factor | 2.5 | units per hp | explosion radius = magnitude × this (Half-Life base) |
| ambient_ramp_step | 0.2 | s | `ambient_generic` fade/pitch/LFO update period |
| light_style_rate | 10 | letters/s | light style patterns advance this fast (client) |

## Behavior

### Entity counts in the stock maps

Counted over the 25 maps (class: entities, maps). Classes in **bold**
have logic beyond being drawn or compiled.

| Class | Count | Maps | | Class | Count | Maps |
|---|---|---|---|---|---|---|
| func_wall | 1655 | 23 | | **func_door_rotating** | 31 | 10 |
| func_illusionary | 1002 | 24 | | worldspawn | 25 | 25 |
| **light** | 379 | 16 | | **multi_manager** | 23 | 6 |
| **func_breakable** | 354 | 20 | | **func_button** | 21 | 7 |
| **env_sprite** | 350 | 7 | | **trigger_relay** | 19 | 3 |
| info_player_deathmatch | 345 | 25 | | **path_track** | 19 | 1 |
| info_player_start | 344 | 25 | | **trigger_hurt** | 17 | 5 |
| **ambient_generic** | 343 | 19 | | **env_render** | 10 | 2 |
| env_glow | 242 | 13 | | **func_hostage_rescue** | 9 | 5 |
| infodecal | 193 | 18 | | **func_pushable** | 8 | 2 |
| cycler_sprite | 125 | 5 | | **trigger_auto** | 6 | 3 |
| func_ladder | 85 | 17 | | **info_hostage_rescue** | 6 | 3 |
| **trigger_multiple** | 79 | 15 | | item_generic | 4 | 1 |
| light_spot | 72 | 9 | | **info_map_parameters** | 4 | 4 |
| info_target | 69 | 17 | | **func_conveyor** | 4 | 1 |
| **func_door** | 63 | 11 | | **env_spark** | 3 | 3 |
| **trigger_camera** | 63 | 17 | | **env_beam** | 3 | 1 |
| light_environment | 60 | 25 | | **env_shooter** | 2 | 1 |
| **func_water** | 50 | 9 | | **armoury_entity** | 2 | 2 |
| **env_sound** | 44 | 3 | | **func_vip_safetyzone**, info_vip_start | 1 each | 1 |
| **func_buyzone** | 37 | 12 | | **multisource**, **func_rot_button**, **func_rotating**, **env_rain**, cycler, **func_tracktrain**, **func_pendulum** | 1 each | 1 |
| **hostage_entity** | 36 | 9 | | | | |
| **func_bomb_target** | 34 | 15 | | | | |
| **env_explosion** | 33 | 4 | | | | |

Not used by any stock map but covered below because custom CS maps and
the other GoldSrc games use them: `func_train` with `path_corner`,
`func_wall_toggle`, `trigger_once`, `trigger_push`, `trigger_teleport`
(with `info_teleport_destination`).

`de_dust2` has no doors, buttons or triggers: its logic is two buy zones
(`team` 1 and 2), two bomb targets and ten crates they break, plus three
intro cameras.

### Keys common to every entity

Read by the engine for every class before the class sees its own keys:
`classname`, `origin` ("x y z"), `angles` ("p y r"), `angle` (a yaw,
turned into angles "0 yaw 0"; −1 and −2 are kept as yaw −1/−2 and mean
up/down where a class derives a direction), `model` (`*n` = brush model
n, or a file path), `targetname`, `target`, `netname`, `message`,
`spawnflags`, `speed`, `health`, `dmg`, `skin`, `body`, `scale`,
`frame`, `framerate`, `rendermode`, `renderamt`, `rendercolor`,
`renderfx`, `team`, `gravity`, `friction`. A class's own keys (`wait`,
`lip`, `delay`, `killtarget`, `master`, ...) are listed per class.
Keys only the map compiler reads (`_minlight`, `zhlt_lightflags`,
`_light`, `_cone`, `_fade`, `style` on `light`) do nothing at run time
except where stated.

### Firing: target, delay, killtarget, use types

A **fire** of name N with activator A and caller C finds every entity,
in entity order, whose `targetname` is N and that is not being removed,
and **uses** each one with (A, C, use type). A use type is one of
*toggle* (the default for almost every sender), *on*, *off*, or *set*
with a number (only `+use` and momentary controls send *set*). An empty
or unmatched name fires nothing (three such references exist in the
stock maps; they are harmless).

Classes that send fires do it through one shared step ("fire my
targets") that reads their `target`, `killtarget` and `delay`:

1. If the entity has neither `target` nor `killtarget`, nothing happens.
2. If `delay` is not 0: a hidden one-shot timer is created that, `delay`
   seconds later, performs steps 3-4 with the same target, killtarget
   and use type, and the original activator if it was a player (any
   other activator is dropped). The firing entity is free at once (it
   may fire again, which makes another timer). Timers are independent:
   N fires within the delay produce N delayed fires.
3. **killtarget**: every entity named `killtarget` is removed (gone at
   the end of the frame; it can no longer be fired this frame).
4. **target**: fire `target` with the activator, caller = this entity,
   and the use type.

Receivers that honour the use type treat it with one rule: *toggle* and
*set* always act; *on* acts only if the receiver is currently off; *off*
only if it is on. Receivers that ignore it act on every use.

**master**: a door, button, `trigger_multiple`/`once`/`teleport` or
`trigger_hurt` with a `master` key asks the entity named `master` whether
it is triggered. If no entity has that name or it is not a master
(`multisource` is the only master class), the answer is yes. A locked
entity behaves as described per class (usually: locked sound, no
action).

### multi_manager

Fires several names at times after it is used.

- Keys: `targetname`, `spawnflags` (1 "multithreaded"), and every other
  key is a target name whose value is its delay in seconds. Because an
  editor cannot repeat a key, a suffix from `#` on is cut off the key:
  `l2` and `l2#1` both name `l2`. At most 16 targets; keys after the
  16th are ignored (`de_piranesi`'s `snd1a` has exactly 16).
- The list is sorted by delay at spawn (a stable sort: equal delays keep
  their file order).
- **Use** (activator A): if busy and not multithreaded, ignored. Else it
  records A and the start time and becomes busy; then, each server frame
  from that frame on, it fires (use type *toggle*, caller = itself) every
  listed name whose delay has elapsed, in list order, with activator A.
  A delay of 0 fires in the same frame as the use. When the last name has
  fired it becomes usable again.
- Multithreaded (flag 1): each use runs on a copy, so overlapping runs
  proceed independently. No stock map sets it.
- Several multi_managers may share a name (`de_torn` has four called
  `squarebreak`): a fire runs all of them.

Worked example, `cs_747`: `trigger_auto` (flag 1, fires once) fires
`pl_l_mm` 0.1 s after the map starts. `pl_l_mm` lists `l1` 0, `l1#1`
0.5, `l2` 1, `l2#1` 1.5, `pl_l_r` 3: it fires `l1` at 0 s and 0.5 s,
`l2` (no such entity: nothing) at 1 s and 1.5 s, and `pl_l_r` at 3 s.
`pl_l_r` is a `trigger_relay` with `delay` 1 whose target is `pl_l_mm`,
so the manager runs again 4 s after it started: a loop with a 4 s
period.

### multisource

A master that is triggered when all of its inputs are "on".

- Keys: `targetname`, `target`, `globalstate` (a Half-Life global state
  name; CS maps leave it empty), `killtarget`, `delay`.
- 0.1 s after spawn it builds its input list: every entity whose
  `target` equals its name, then every `multi_manager` that lists its
  name; at most 32. Until then it reports not triggered.
- **Use** by caller C: if C is in the list, C's input flips (off ↔ on);
  a use from an entity not in the list is ignored. If after the flip
  every input is on, it fires its own `target` (activator none, use type
  *toggle*).
- As a master it answers "triggered" while every input is on.
- A `func_button` that returns home also uses every `multisource` its
  `target` names (so a button's input flips on when pressed and off when
  it comes back, unless it is a toggle button, which flips at both ends
  anyway by firing).
- The stock maps' only `multisource` (`cs_assault`) has no name: it is
  never fired and is nobody's master.

### trigger_relay and trigger_auto

- `trigger_relay` (point): keys `targetname`, `target`, `delay`,
  `killtarget`, `triggerstate` (0 *off*, 1 *on*, 2 *toggle*; any other
  value *on*), spawnflags 1 "Remove On fire". When used, it fires its
  targets with its own use type (not the incoming one), **activator =
  the relay itself** (the player who started the chain is not passed
  on), then removes itself if flag 1. Stock use: `de_survivor`'s
  timed explosion chains (`BOOM_PLANE`, eight relays with delays 0.8 to
  3.3 s), `cs_assault`'s `spy_dev_destroyed` (flag 1, `killtarget`
  `spy_dev_mons`: when the spy device breaks, its three monitor buttons
  are removed).
- `trigger_auto` (point): keys `target`, `delay`, `killtarget`,
  `triggerstate` (as above), `globalstate`, spawnflags 1 "Remove On
  fire". 0.1 s after the map spawns it fires its targets (activator =
  itself); with flag 1 it then removes itself. Without flag 1 it stays
  but does not fire again by itself (it has no timer after the first;
  it would fire again only when the map respawns entities, see Open
  questions on round restarts).

### Moving brushes: positions, speed, the mover's clock

Doors, buttons, `func_water`, trains, rotators and pendulums are
"movers": brush entities that move by a velocity (or angular velocity)
and are solid to players unless a flag says otherwise. The common rules:

- **movedir** (linear movers): from `angles`/`angle`: yaw −1 → straight
  up (0 0 1), yaw −2 → straight down (0 0 −1), otherwise the forward
  vector of the angles. The entity's drawn angles are then reset to
  0 0 0 (the direction does not rotate the model).
- **Positions** (doors, `func_water`, `func_button`): closed position
  P1 = the spawn origin. Travel distance
  D = |dx|·(sx − 2) + |dy|·(sy − 2) + |dz|·(sz − 2) − lip,
  with (dx, dy, dz) = movedir and (sx, sy, sz) the brush size.
  Open position P2 = P1 + movedir · D. A negative `lip` moves further
  than the brush's own size (elevators: `as_oilrig` lip −485). For an
  axial movedir this is "the brush's size along the axis, minus 2,
  minus lip".
- **A linear move** to G at speed s: if already at G, it ends at once.
  Otherwise velocity = (G − origin) / t with t = |G − origin| / s, and
  the move ends when the mover's clock has advanced by t. At the end the
  mover is put exactly at G, unless it is more than 1/32 unit away (a
  push or a block drifted it), in which case it starts a new move to G
  at 100 units/s first.
- **An angular move** to angles G at speed s: the same with angles;
  the time is |G − angles| / s using the length of the angle difference
  vector; at the end the angles are set exactly to G.
- **The mover's clock** advances only by the time the mover actually
  moved. A blocked mover does not move that frame and its clock does not
  advance, so the end of its move and its wait are postponed by the
  blocked time. Waits (`wait` before returning) are counted on this
  clock too.
- **Pushing and riding** (engine physics, Quake lineage): each frame
  the mover moves by velocity × frame time (only as far as its next
  timer). Entities standing on it, and entities its new position
  overlaps, are moved with it by the same displacement (rotating
  movers: carried round the axis). Non-solid and trigger entities, and
  other movers, are not pushed. If a pushed entity would end inside the
  mover or a wall, the whole move is undone for that frame and the mover
  is **blocked** by that entity (its blocked rule runs, below).
- Collision with a mover uses its brush model's own clip hulls
  (bsp.md "Hulls"), translated (and rotated, for rotated movers) by the
  mover's position.

### func_door

Keys (with the stock maps' usage): `speed` (0 → 100; 22 doors at 100,
16 at 50, 15 at 180), `wait` (seconds open before returning; −1 never
returns; stock 10, 6, 4, −1 most), `lip` (stock 4, −1, −12, 2, −4096 ...),
`dmg` (damage per blocked frame), `movesnd` and `stopsnd` (sound
indices, below), `locked_sound`, `unlocked_sound` (button-sound
indices), `locked_sentence`, `unlocked_sentence` (Half-Life voice lines;
0 in every stock map: ignore), `target` and `netname` ("fire on close"),
`delay`, `killtarget`, `master`, `healthvalue` (health given to the
player who opens it; not in stock maps), `health` (ignored: doors are not
shot open), `message` (ignored), render keys.

Spawnflags: 1 "Starts Open", 4 "Don't link" (no effect in this logic),
8 "Passable" (not solid), 32 "Toggle" (no automatic return), 256 "Use
Only" (not opened by touch; see `+use` below).

**Spawn**: compute movedir, P1, P2 as above. Solid unless Passable. With
"Starts Open" the door is placed at P2 and P1/P2 are swapped (it now
"opens" back to where the mapper drew it). The door starts **closed**
(at P1, idle).

States: closed, opening, open, closing.

**Activation by touch**: only players touch-open doors, and only when
the door is closed (touch is off while moving, and while open unless it
is a Toggle door), not Use Only, and has **no `targetname`**. A named
door touched by a player only plays its locked sound. A door with a
`master` that is not triggered plays the locked sound and does not open.

**Activation by use** (a fire from a button, trigger or relay): only
acts when the door is closed, or open with the Toggle flag; uses while
moving are ignored, and so are uses of an open non-Toggle door. The
activator is remembered (rotating doors swing away from it; a player
activator is given `healthvalue`). If a `master` is set and not
triggered, nothing happens. Then:

- Toggle door that is open: close.
- Otherwise: play the unlocked sound (if any; at most once per 3 s),
  then open.

**Open**: unless already moving, start the moving sound (static channel,
volume 1, normal attenuation). State opening; linear move to P2 at
`speed`. **Arrived open**: stop the moving sound, play the stop sound;
state open. If Toggle: wait for the next use (touch re-enabled unless
Use Only). Else if `wait` is −1: stay open for good. Else after `wait`
seconds (mover's clock), close. Then fire `netname` if "Starts Open" is
set, and fire `target` (activator = the remembered activator, use type
*toggle*, with `delay`/`killtarget`).

**Close**: unless already moving, start the moving sound. State
closing; linear move to P1. **Arrived closed**: stop the moving sound,
play the stop sound; state closed; touch re-enabled unless Use Only.
Fire `target` (as above), then `netname` unless "Starts Open" is set.

So a door fires its `target` **twice per cycle**, once on each arrival.
`de_piranesi` relies on this: a hidden door (`*117`) fires the
`multi_manager` `roundreseter` each time it comes to rest.

**Blocked** (each frame a player or object stops the move):

1. If `dmg` is not 0: the blocker takes `dmg` crush damage (type 1),
   inflicted by the door.
2. If `wait` ≥ 0: reverse: a closing door opens, an opening (or any
   other) door closes. (So a door with `dmg` hurts once per reversal,
   not every frame, unless it is a `wait` −1 door.)
3. If the door has a name: every other `func_door` or
   `func_door_rotating` with the same name and `wait` ≥ 0 is reversed
   the same way (and if it was moving with exactly this door's velocity
   it is first snapped to this door's position or angles and stopped).
   This keeps double doors together.

A `wait` −1 door neither reverses nor stops trying: it pushes (and
hurts, if `dmg`) every frame until the blocker is gone.

**Sounds by index** (file paths are the game's data; all in `valve/`):

| Index | `movesnd` (door) | `stopsnd` (door) |
|---|---|---|
| 0 | none | none |
| 1-10 | `doors/doormove1.wav` .. `doormove10.wav` | 1-8: `doors/doorstop1.wav` .. `doorstop8.wav` |
| other | none | none |

Lock sounds (`locked_sound`, `unlocked_sound`, and buttons' `sounds`)
use the button-sound table under func_button. Locked and unlocked
sounds play on the item channel, volume 1, normal attenuation, at most
once per 3 s for doors (0.5 s for buttons).

Worked example, `cs_office` `door1` (model `*200`): `angles` −90 0 0,
`lip` 66, `speed` 40, `wait` 20, `movesnd` 6, `stopsnd` 1, named. Brush
16 × 304 × 176 at x 576..592, y −2024..−1720, z −336..−160. movedir =
(0 0 1); D = 176 − 2 − 66 = 108; P2 = P1 + (0 0 108). Being named, it
does not open by touch; the `func_button` `*199` fires it (below). It
rises 108 units in 2.7 s with `doors/doormove6.wav`, stops with
`doors/doorstop1.wav`, waits 20 s and comes down in 2.7 s.

Worked example, `de_nuke` `GlassDoor2` (four doors `*41`..`*44` with
the same name, spawnflags 4): two glass panes (`rendermode` 2,
`renderamt` 96, `lip` −12, 4 × 52 × 92) and two frames (`lip` 4,
8 × 68 × 108), yaw 270 or 90. Every one travels 62 units (52 − 2 + 12 or
68 − 2 − 4) at 50 units/s, 1.24 s, waits 10 s. The `trigger_multiple`
`*40` (`wait` 2) fires `GlassDoor2`: the four slide apart together. The
frames play `doors/doormove5.wav`/`doorstop5.wav`; the panes are silent.

Worked example, `cs_747` curtains (`curt1`..`curt99`, spawnflags 8
Passable, `speed` 180, `lip` −1, `wait` 6): each slides 63 units (64 −
2 + 1) in 0.35 s when its own `trigger_multiple` (`wait` 6) is entered,
and does not block anyone.

Worked example, `as_oilrig` `elevator` (`*22`): `angle` −2 (down),
spawnflags 1 Starts Open, `lip` −485, `speed` 100, `wait` 5, `dmg` 200,
brush 360 × 344 × 22. D = 22 − 2 + 485 = 505: it spawns 505 units below
where it was drawn (the bottom) and that is its "closed" place. The
`func_button` `elvbtnl` (touch-activated, `delay` 3) fires it: 3 s
later it rises 505 units in 5.05 s (`doors/doormove7.wav`), waits 5 s at
the top and goes back down. A player under it is hit for 200 and the
elevator reverses.

### func_door_rotating

As `func_door`, but it turns instead of sliding.

Keys: as `func_door` plus `distance` (degrees, stock 90 on 27 of 31;
`lip` is unused). The model is built round an `origin` brush; the entity
has an `origin` key (the hinge) and `angles` (stock: always 0 0 0).

Spawnflags: as `func_door`, plus 2 "Reverse Dir", 16 "One-way", 64 "X
Axis", 128 "Y Axis".

- **Axis**: by default the door turns its yaw (round the vertical); with
  64 it turns its roll (round the X axis); with 128 its pitch (round the
  Y axis). As a vector of angle components: yaw (0 1 0), roll (0 0 1),
  pitch (1 0 0); Reverse Dir negates it.
- Closed angles A1 = spawn angles; open angles A2 = A1 + axis ·
  `distance`. With Starts Open the door is placed at A2, A1/A2 swap and
  the axis is negated.
- **Opening direction**: when it opens, if it turns its yaw, is not
  One-way, and has a remembered activator: take v = activator origin −
  hinge, and w = (activator origin + 10 · forward(activator's view
  angles, pitch included)) − hinge. If the vertical component of v × w
  (vx · wy − vy · wx) is negative the door swings the other way (to
  −A2, i.e. A1 − axis · distance when A1 is 0). So it swings away from
  where the activator is heading. The activator of a door fired by a
  button or trigger is the player who pressed or entered it.
- Closing always returns to A1 (whichever way it opened).
- Movement, waits, sounds, blocking and touch rules: as `func_door`,
  with angular moves at `speed` degrees/s.

Worked example, `cs_militia` `*1`: hinge 175 162 −105, model x −3..61,
y −3..3 (a 64-unit door along +X from the hinge), `distance` 90, `speed`
100, `wait` 3, `movesnd` 9, unnamed. A player walking into it opens it
by touch: it turns 90° in 0.9 s away from him with
`doors/doormove9.wav`, waits 3 s and swings back.

Worked example, `de_storm` `door1` (`*11` and `*12`): a named pair,
`wait` −1, `speed` 150, `unlocked_sound` 13. Nothing in `de_storm` fires
`door1`, so these doors never open (touch only plays the locked sound,
which is empty). The map's unnamed rotating doors (`wait` −1, `speed`
150) open by touch in 0.6 s and stay open for the rest of the round.

### func_water

A `func_door` whose brush is a liquid volume.

- Keys: as `func_door`, plus `skin` = contents (−3 water, −4 slime,
  −5 lava; all 50 stock ones are −3) and `WaveHeight` (wave amplitude
  for the surface; stored as one eighth of the value and sent to
  clients for drawing).
- Never solid, always silent. Being not solid it is never touched, so an
  unnamed `func_water` never moves; a named one rises and falls like a
  door when fired. None of the stock ones is named; their `speed` 100,
  `wait` 4 are editor defaults with no effect.
- The volume counts as water for players (swimming, the view under
  water) because a brush entity with a contents `skin` is included in
  the movement's contents checks (cs16 movement spec). Stock render:
  `rendermode` 2 with `renderamt` 85, 128, 100 ...
- `func_illusionary` with `skin` −3 behaves the same way as a water
  volume (one in `cs_italy`, `*13`).

### func_button

Keys: `speed` (0 → 40), `wait` (0 → 1; −1 stays pressed), `lip` (0 → 4),
`sounds` (press sound index), `target`, `delay`, `killtarget`, `master`,
`health` (> 0: shootable), `locked_sound`, `unlocked_sound`,
`locked_sentence`, `unlocked_sentence` (ignored), `changetarget`
(unused here), render keys, `angles`/`angle` (press direction).

Spawnflags: 1 "Don't move", 32 "Toggle", 64 "Sparks", 256 "Touch
Activates".

**Spawn**: movedir and P1/P2 as a door (with the lip default 4). If the
travel is under 1 unit or "Don't move" is set, P2 = P1. Solid. A button
with `health` > 0 can be damaged and is not `+use`-able; otherwise it is
used by `+use` (unless Touch Activates, in which case players activate
it by touching it and `+use` does nothing). "Stays pressed" = `wait` −1.

States: out (home), going in, in, coming out.

- **Press** (by `+use`, a fire, touch with flag 256, or damage): ignored
  while moving, and while in unless it is a Toggle button (then the
  press brings it out, below). From out: play the press sound (voice
  channel). If a `master` is set and not triggered: play the locked
  sound and stop (stay out). Else play the unlocked sound, state going
  in, linear move to P2 at `speed` (Don't move: arrives at once).
- **Arrived in**: if a master is set and not triggered now, stop here
  (the button stays in, and never comes back). Otherwise state in;
  **frame = 1** (the alternate textures show); fire `target` (activator
  = presser, use type *toggle*, with `delay`). Then: if Toggle or Stays
  pressed, wait for the next press (touch re-enabled if flag 256); else
  after `wait` seconds come out.
- **Toggle button pressed while in**: play the press sound and come out
  (no fire now). A Stays-pressed button never comes out.
- **Come out**: state coming out; **frame = 0** (normal textures) at
  once; linear move to P1.
- **Arrived out**: state out. If Toggle: fire `target` again (so a
  toggle button fires once going in and once coming out). Then use every
  `multisource` named by `target` (the button is that multisource's
  input). Re-enable touch (flag 256). If Sparks: start sparking.
- **Damage** (shootable buttons): any damage presses it if it is out;
  for a Toggle button that is in, damage brings it out and fires its
  targets at once.
- **Sparks** (64): while out, every 0.1 + random [0, 1.5] s a spark
  effect at the button's box corner plus half its size, with one of
  `buttons/spark1.wav` .. `spark6.wav` at volume random [0.25, 0.75] ×
  0.4.

**Button-sound table** (`sounds`, `locked_sound`, `unlocked_sound`;
paths are game data in `valve/sound/`):

| Index | File | Index | File |
|---|---|---|---|
| 0 | none | 11 | `buttons/button11.wav` |
| 1-10 | `buttons/button1.wav` .. `button10.wav` | 12 | `buttons/latchlocked1.wav` |
| 13 | `buttons/latchunlocked1.wav` | 14 | `buttons/lightswitch2.wav` |
| 21-25 | `buttons/lever1.wav` .. `lever5.wav` | other | `buttons/button9.wav` |

Worked example, `cs_office` `*199`: spawnflags 33 (Don't move + Toggle),
`delay` 1, `speed` 5, `wait` 3 (ignored: Toggle), `target` `door1`;
its faces use `+0~sp_lock` and the map also holds `+a~sp_lock`. First
`+use`: it arrives in at once, shows `+a~sp_lock` (frame 1) and, 1 s
later, fires `door1` (which opens, see func_door). Second `+use`: it
comes out (frame 0, `+0~sp_lock` again) and 1 s later fires `door1`
again; the door opens if it is closed by then and ignores the fire if
it is still up.

Worked example, `cs_siege` `caveelev`: three buttons (spawnflags 1,
`delay` 2, `wait` 1, `sounds` 9 = `buttons/button9.wav`) fire a
`func_door` (spawnflags 36 = 4 + 32 Toggle, `angles` −90 0 0, `lip`
−504, `speed` 120, `wait` −1, brush height 136). The elevator rises
638 units (136 − 2 + 504) in 5.32 s two seconds after a press and stays
up; the next press sends it down.

### func_rot_button

A button that turns instead of sliding: keys as `func_button` plus
`distance` (degrees); `speed` is degrees/s (0 → 40); default `sounds`
in the editor is 21. Spawnflags: 1 "Not solid", 2 "Reverse Dir", 32
"Toggle", 64 "X Axis" (roll), 128 "Y Axis" (pitch), 256 "Touch
Activates". Axis and Reverse as `func_door_rotating`; in = spawn angles
+ axis · `distance`; out = spawn angles. Everything else as
`func_button` (frame 1 while in, fires on arrival in, toggles, multisource).

Worked example, `cs_militia` `*38`: spawnflags 97 (Not solid + Toggle +
roll), `distance` 180, `speed` 150, `sounds` 24 (`buttons/lever4.wav`),
`target` `slight`. A `+use` turns it 180° in 1.2 s, then fires `slight`:
the 25 `light` entities named `slight` (style 32) go dark. The next
`+use` turns it back in 1.2 s and fires `slight` again: the lights come
back on.

### func_breakable

Keys: `health` (hit points; stock 1 on 116 of them, 50 000 on 16 that
are in effect unbreakable), `material` (0 glass, 1 wood, 2 metal, 3
flesh, 4 cinder block, 5 ceiling tile, 6 computer, 7 unbreakable glass,
8 rocks; out of range → wood), `explosion` (gibs direction: 0 random,
1 away from the attack), `gibmodel` (overrides the material's debris
model), `spawnobject` (an item to drop; 0 in every stock map),
`explodemagnitude` (> 0: explodes when broken), `target` (fired when
broken), `delay`, `killtarget`, `targetname`, render keys.

Spawnflags: 1 "Only Trigger" (cannot be damaged; breaks only when
fired), 2 "Touch" (a running player breaks it), 4 "Pressure" (breaks
under a player standing on it), 256 "Instant Crowbar" (a melee hit by a
player breaks it at once).

- Solid. Damageable unless Only Trigger. Unbreakable glass (7) can never
  break: damage only makes ricochet effects.
- **Damage** d of type T from attacker A (inflictor I): melee ("club")
  damage counts double; poison 0.1×; with Instant Crowbar a player's
  melee hit does the remaining health. Health −= d. At health ≤ 0 it
  **breaks**. Otherwise a material damage sound plays (voice channel,
  volume random [0.75, 1], pitch 100 two times in three, else
  95 + random [0, 34]). The attack direction for debris is from the
  breakable's box centre towards the inflictor.
- Computer (6) and unbreakable glass (7) hit by a bullet make sparks
  (`buttons/spark5.wav`/`spark6.wav`) or a ricochet half the time.
- **Fired** (used): breaks at once (debris direction: the entity's
  original yaw).
- **Touch** (2): a player touching it at speed v does v × 0.01 damage if
  that is at least its health (it then breaks, and the player takes a
  quarter of that as slash damage). **Pressure** (4): a player whose box
  bottom is within 2 units of its top makes it break after `delay`
  (0 counts as 0.1 s).
- **Break**: a break sound by material (glass `debris/bustglass1-2`,
  wood `bustcrate1-2`, metal and computer `bustmetal1-2`, flesh
  `bustflesh1-2`, rocks and cinder `bustconcrete1-2`, ceiling
  `bustceiling`; volume random [0.85, 1] plus |health|/100, at most 1;
  pitch 95 + random [0, 29], snapped to 100 if 98-102). A debris effect
  at the box centre the size of the box: 10 pieces of the gib model
  (material default: `models/woodgibs.mdl`, `fleshgibs`, `computergibs`,
  `glassgibs`, `metalplategibs`, `cindergibs`, `rockgibs`,
  `ceilinggibs`; or `gibmodel`), velocity 200 along the attack
  direction if `explosion` is 1 else none, life 2.5 s. Entities
  standing on it lose their ground. It becomes non-solid, loses its
  name, fires `target` (activator none, *toggle*), and is removed 0.1 s
  later. If `explodemagnitude` > 0 an explosion of that magnitude
  happens at its centre (see env_explosion).

Worked example, `de_dust2`: ten crates (`gibmodel`
`models/mil_crategibs.mdl`, `material` 2, `health` 1, spawnflags 1 Only
Trigger, `explodemagnitude` 100), five named `tgt_a` and five `tgt_b`.
The bomb targets `*42` (`target` `tgt_a`) and `*16` (`target` `tgt_b`)
fire them when the bomb explodes there: the five crates of that site
burst into metal crate gibs, each with a magnitude-100 explosion. They
cannot be shot.

Worked example, `cs_assault` spy device `*54`: `material` 6, `health`
550, `explodemagnitude` 40, `target` `spy_dev_destroyed`: 550 damage
breaks it, a small explosion follows, and the relay of that name removes
the three monitor buttons (`killtarget` `spy_dev_mons`) while the
`func_wall` of that name toggles its frame.

### func_pushable

A movable crate. Keys: as `func_breakable`, plus `size` (the box it
collides with: 0 point ±8, 1 player 32 × 32 × 72, 2 twice the duck box,
3 player ducked; stock 3 on 7 of 8), `friction` (0-399; its maximum
speed is 400 − friction; stock 50 → 350), `buoyancy` (how strongly it
floats, scaled by its footprint area × 0.0005; stock 20). Spawnflags 128
"Breakable" (then it also breaks as a `func_breakable`; one stock
pushable has it).

- It is a box (not its brush shape), moved by physics with gravity, and
  floats. It spawns lifted 1 unit.
- A player walking into it (on the ground, not holding `+use`) adds his
  horizontal velocity to the crate's and is then given the crate's
  horizontal velocity; a player in the air does not push it unless the
  crate is in water (then 0.1 of his velocity). Holding `+use` on it
  while moving pulls it along. Other entities push it with 0.25 of their
  velocity. Its horizontal speed is capped at its maximum. A push sound
  (`debris/pushbox1-3.wav`) plays while it moves. The exact pull rule
  and the sound timing are in Open questions.
- Stock: `cs_havana` (7) and `cs_estate` (1).

### func_wall, func_illusionary, func_wall_toggle

- `func_wall`: a solid, static brush (angles forced to 0). **When used**
  it flips its frame between 0 and 1 by the use-type rule (*toggle*
  flips; *on* sets 1 only from 0; *off* sets 0 only from 1): this shows
  its model's alternate texture set. Stock: 1 655, mostly for render
  modes (`rendermode` 4 with `{` textures on 477, 2 on 65); 9 are named.
- `func_illusionary`: drawn, never solid, no use. `skin` is a contents
  value (−1 empty on 1 001 of 1 002; −3 water on `cs_italy` `*13`).
- `func_wall_toggle` (not in stock maps): a `func_wall` that, when used,
  turns itself off (non-solid and not drawn) or on by the use-type rule,
  instead of flipping its frame. Spawnflag 1 "Starts Invisible": spawns
  off.

### Alternate textures: when `+a` shows

This answers the question bsp.md left open. A face's texture name may
belong to a family with a normal set (`+0name`..`+9name`) and an
alternate set (`+aname`..`+jname`); bsp.md "Texture name prefixes" and
the textures lump describe the sets.

- The client draws each face of a brush model with the family of the
  texture the face names, chosen by the drawing entity's **frame**: frame
  0 → the set the face's own texture belongs to; frame not 0 → the
  **other** set of the same family, if the map holds it. So a face
  textured `+0x` shows the `+a` set when the entity's frame is 1, and a
  face textured `+ax` shows the `+0` set then; a family with only one
  set shows that set at either frame.
- The chosen set animates as usual: ten frames a second, on the
  client's clock, with every face in phase (the toggle does not restart
  the animation). The alternate set animates at the same rate if it has
  more than one frame (`+a`, `+b`, ...).
- The world (entity 0) always has frame 0, so world faces show the set
  of their own texture (stock maps use `+a` lights on the world:
  `+a~fifties_lgt2` etc. are drawn as such, unchanging).
- Who sets a brush entity's frame: `func_button` and `func_rot_button`
  (1 on arriving in, 0 when starting out), `func_wall` (flips when
  used). Doors, breakables, illusionaries, `func_water` and the others
  never change it.

Stock occurrences (faces of brush entities with a family that has both
sets): `cs_office` `*199` (`+0~sp_lock` on the door button), `cs_militia`
`*29` (`+0fiftsstch4`, a light switch button fired by nothing else:
`+a` shows for 1 s per press), `cs_siege` `*5`, `*6`, `*50`
(`+0~metalstch2`, the elevator buttons), `as_oilrig` `*23`
(`+0button4`), `de_survivor` `*54` (a `func_wall` named `CLIC` with
`+asuvb` and `+asuvs`: blinks, below), and unnamed `func_wall`s and
`func_illusionary`s in `de_aztec`, `de_piranesi` and `de_vertigo` with
`+a` faces that are never used (they show their `+a` set for good).

Worked example, `de_survivor` `CLIC`: the `trigger_multiple` `*53`
(`wait` −1: once) fires `CLIC`. Named `CLIC`: two `light`s (style 32),
four `env_sprite`s, an `ambient_generic` (`weapons/357_cock1.wav`), the
`func_wall` `*54`, and a `trigger_relay` (`triggerstate` 2, `delay` 0.5)
whose target is `CLIC` itself. Each fire toggles the lights, the sprites
and the sound, flips the wall's frame (its `+asuvb`/`+asuvs` faces swap
to `+0suvb`/`+0suvs` and back) and makes the relay fire `CLIC` again
0.5 s later: a self-sustaining blink with a 0.5 s half-period that runs
for the rest of the map (see Open questions on round restarts).

### trigger_multiple and trigger_once

Invisible brush volumes. Keys: `target`, `delay`, `killtarget`,
`master`, `wait` (multiple only; 0 → 0.2; ≤ 0 after that means once),
`message` (text shown to the activating player), `netname`, `sounds`
(ignored), `style` (ignored; every stock trigger has 32, an editor
default), `angles` (ignored). Spawnflags: 1 "Monsters" (monsters may
fire it: in CS, hostages), 2 "No Clients" (players may not), 4
"Pushables" (a `func_pushable` may fire it).

- Not drawn, not solid; overlap is tested every time a player (or other
  entity) moves (see Edge cases for the overlap test).
- Fired by: a player (unless flag 2), a monster with flag 1, a
  `func_pushable` with flag 4.
- **Activation**: ignored while waiting to re-arm, or if its master is
  not triggered. Otherwise it fires its targets (activator = the
  toucher, use type *toggle*, with `delay`/`killtarget`); if the
  toucher is a player and `message` is set, shows him the message
  (HUD text). Then: with `wait` > 0 it re-arms after `wait` seconds;
  otherwise it stops responding and is removed 0.1 s later.
- `trigger_once`: the same with `wait` fixed at −1.
- Stock waits are long (4, 6, 10, 100, 200 s): they fire sounds and
  doors at most that often, whoever enters. `cs_backalley`'s locked
  doors are triggers with `message` "Seems to be locked." that fire an
  `ambient_generic` playing `buttons/latchlocked2.wav` (`wait` 4).

### trigger_hurt

Keys: `dmg` (damage per second; negative heals), `damagetype` (bit
field: 0 generic, 1 crush, 2 bullet, 4 slash, 8 burn, 16 freeze, 32
fall, 64 blast, 128 club, 256 shock, 512 sonic, 1024 energy beam, 16384
drown, ... 262144 radiation; stock 32 fall on 10, 0, 8, 16), `target`,
`delay`, `killtarget`, `master`. Spawnflags: 1 "Target Once", 2 "Start
Off", 8 "No clients", 16 "Fire Client Only", 32 "Touch Client Only".

- Not drawn. Starts on unless flag 2 (off = not touchable). A named
  `trigger_hurt` flips on/off each time it is used (any use type);
  switching on re-tests everything already inside.
- **Touch** by an entity that can take damage (flag 32: players only;
  flag 8: not players): it applies `dmg` × 0.5 (half a second's worth)
  of `damagetype` damage, inflicted by the trigger, at most once per
  0.5 s per player: the first hurt starts a 0.5 s window; within the
  window a player not yet hurt in it is hurt (and marked), as is anyone
  touched in the same frame as the window's first hurt; when the window
  ends, the marks clear. Negative `dmg` heals by that amount (living
  players only).
- After hurting it fires `target` (activator = the victim; with flag 16
  only if the victim is a player); with flag 1 the target is then
  cleared (fires once).
- Stock: `as_oilrig` (10 × `dmg` 500 fall: 250 per half second, death
  in the sea), `de_prodigy` (`dmg` 999 and 999 999), `de_survivor`
  (`dmg` 10, freeze: 5 per half second).

### trigger_push (not in stock maps)

Keys: `speed` (0 → 100; the editor's default is 40), `angles` (push
direction; 0 0 0 means yaw 360, i.e. +X), `target`. Spawnflags: 1 "Once
Only", 2 "Start Off". Toggled on/off by use (any use type).

- Ignores movers, non-moving and noclip entities, and non-solid or
  brush entities.
- Normally adds speed × direction to the toucher's **base velocity**
  for that frame (added to any other base velocity it already has this
  frame); the movement code adds base velocity to the player's own
  (cs16 movement spec).
- Once Only: adds speed × direction to the toucher's velocity, lifts it
  off the ground if the result goes up, and removes the trigger.

### trigger_teleport (not in stock maps)

Keys: `target` (the destination: the first entity with that name,
usually `info_teleport_destination`, any point entity works), `master`.
Spawnflags: 1 "Monsters", 2 "No Clients".

On touch by a player or monster (flags as `trigger_multiple`; master
checked): if the destination exists, the toucher is placed at the
destination's origin raised by its box's half-height (players: the
origin is the box centre, so feet land on the destination) plus 1 unit,
its angles (and a player's view) set to the destination's angles, its
velocity and base velocity zeroed, and it leaves the ground.

### func_train and path_corner (not in stock maps)

A platform that moves from corner to corner.

- `func_train` keys: `target` (the first `path_corner`), `speed` (0 →
  100), `dmg` (0 → 2), `movesnd`, `stopsnd` (platform sound table
  below), `volume` (0 → 0.85), render keys. Spawnflag 8 "Not solid".
- `path_corner` keys: `targetname`, `target` (next corner), `wait`
  (seconds to stop here; −1 forever), `speed` (new speed from here on;
  0 keeps the current), `message` (a name fired when the train arrives
  here). Spawnflags: 1 "Wait for retrigger", 2 "Teleport", 4 "Fire once"
  (its `message` fires only the first time).
- **Placement**: once all entities exist, the train moves so that the
  **centre of its brush's bounding box** is on its first corner. An
  unnamed train starts at once (0.1 s); a named one waits to be fired.
- **Running**: it moves (linear move) so its box centre reaches the next
  corner at its speed, with the moving sound (static channel) playing.
  When it leaves a corner whose `speed` is not 0, that becomes its
  speed. On arrival: fire the corner's `message` (activator and caller
  the train); if the corner has "Wait for retrigger" (or the train was
  stopped by a fire), stop the moving sound, play the stop sound
  (voice channel) and wait for a fire; else if the corner's `wait` is
  not 0, stop the sound, play the stop sound and continue after `wait`
  (−1: forever); else go on at once. A "Teleport" corner is jumped to
  instead of travelled to. A corner with no next corner ends the run.
- **Fire** (use): a waiting train goes on to its next corner; a moving
  train stops where it is (its velocity zeroed, the stop sound plays)
  and will head back to the corner it was going to when fired again.
- **Blocked**: the blocker takes `dmg` crush damage at most once per
  0.5 s; the train neither stops nor reverses (it keeps pushing).
- Platform sound table (`movesnd` → `plats/`): 1 `bigmove1`, 2
  `bigmove2`, 3 `elevmove1`, 4 `elevmove2`, 5 `elevmove3`, 6
  `freightmove1`, 7 `freightmove2`, 8 `heavymove1`, 9 `rackmove1`, 10
  `railmove1`, 11 `squeekmove1`, 12 `talkmove1`, 13 `talkmove2`;
  (`stopsnd`) 1 `bigstop1`, 2 `bigstop2`, 3 `freightstop1`, 4
  `heavystop2`, 5 `rackstop1`, 6 `railstop1`, 7 `squeekstop1`, 8
  `talkstop1`; all `.wav`, other values none.

### func_tracktrain and path_track (`de_survivor`)

`de_survivor`'s one `func_tracktrain` (`*23`, origin −232 664 444,
`speed` and `startspeed` 240, `wheels` 100, spawnflags 10 = "No User
Control" + "Passable", `target` `1`) runs round a closed loop of 19
`path_track`s (named `1`..`19`, each `speed` 240) at 240 units/s from
the map's start; two of them have `message` `WOLF`/`WOLF2` (fired as it
passes). It is decoration: passable, not drivable. Its own spec (yaw and
pitch from the track, `wheels`, `height`, banking, the controls) is
deferred; for this map: the entity origin follows the path at 240
units/s, yaw facing the next point. See Open questions.

### func_rotating

Keys: `speed` (degrees/s; ≤ 0 → 0), `fanfriction` (0-100 %, the
spin-up/down step; 0 → 100 %), `volume` (0-10; see Quirks), `sounds`
(1-5: `fans/fan1.wav` .. `fan5.wav`), `message` (a sound file that
overrides `sounds`), `dmg` (damage when blocked), `spawnorigin`.
Spawnflags: 1 "Start ON", 2 "Reverse Direction", 4 "X Axis" (roll), 8
"Y Axis" (pitch), 16 "Acc/Dcc", 32 "Fan Pain", 64 "Not Solid", 128/256/
512 small/medium/large sound radius (idle, static, normal attenuation;
none set: normal).

- Axis vector: yaw (0 1 0) by default, roll (0 0 1) with 4, pitch
  (1 0 0) with 8; negated by 2.
- **Use**: if spinning, spin down; else: with Acc/Dcc, spin up (start
  the sound at volume 0.01 and pitch 30); without, jump to full speed
  (angular velocity = axis × speed) with the sound at full volume,
  pitch 100.
- Spin up/down (every 0.1 s): angular velocity ± axis × speed ×
  fanfriction until it reaches full speed (then the sound goes to full
  volume, pitch 100) or crosses zero (then stops, sound stopped);
  meanwhile the sound's volume is volume × fraction of full speed and
  its pitch 30 + 70 × fraction.
- Start ON: it is used once 1.5 s after spawn.
- Blocked: the blocker takes `dmg` crush damage (every blocked frame).
  Fan Pain (32): anything touching it takes |angular velocity| / 10
  crush damage and is thrown away from its centre at that speed.
- Stock: `de_airstrip` `*138` (one, spawnflags 1, `speed` 10,
  `fanfriction` 20, `rendermode` 4): a slowly turning prop, started
  1.5 s after the map loads at full speed (no Acc/Dcc).

### func_pendulum (`de_torn`)

Keys: `speed` (maximum, °/s; 0 → 100), `distance` (degrees of the full
swing; 0: it never moves), `damp` (0-1000, × 0.001), `dmg`. Spawnflags:
1 "Start ON" (used 0.1 s after spawn), 8 "Passable", 16 "Auto-return",
64/128 axis as `func_door_rotating`.

The swing's centre is the spawn angles + axis × distance / 2; every
0.1 s its angular speed changes by a constant acceleration
speed² / (2 |distance|) towards the centre, clamped to ± `speed`; with
`damp` the clamp itself shrinks by damp × clamp × dt each step and the
pendulum stops at the centre once the clamp falls under 30 °/s. Use
while swinging stops it (Auto-return: it goes back to the start first);
use while still starts it. Touch with `dmg` > 0 hurts by dmg × |speed| ×
0.01 and throws the toucher. `de_torn`'s `streetwind` (`distance` −65,
`speed` 220, `damp` 360, flag 64) is started by a `trigger_auto`.

### func_conveyor (`de_torn`)

Keys: `speed` (0 → 100), `angles` (direction), render keys. Spawnflags:
1 "No Push" (only the texture scrolls), 2 "Not Solid". A solid conveyor
moves what stands on it along its direction at `speed` (the movement
code reads the surface's conveyor speed), and its textures scroll (the
speed is sent to clients encoded in its render colour: sign, then speed
× 16 split over two bytes). Use reverses it. `de_torn`'s four (flags 3,
`speed` 105) are scrolling, non-solid water.

### func_ladder

Invisible (drawn in texture mode with amount 0), not solid; its volume
has contents "ladder" for player movement (cs16 movement spec). No keys
of its own. 85 in the stock maps.

### func_buyzone, func_bomb_target, func_hostage_rescue, func_vip_safetyzone

CS 1.6 objective volumes. Never drawn, never solid; they test players
inside them. Their run-time rules are CS game rules, not Half-Life's;
the rules below are the public, documented behaviour and the numbers are
to be measured (Open questions).

- `func_buyzone`: `team` 0 both teams, 1 Terrorists, 2
  Counter-Terrorists (stock: 19 with 1, 18 with 2). A player of that
  team inside it may buy while buying is allowed (buy time, and
  `info_map_parameters` `buying`). A map with no `func_buyzone` gives
  each team a buy area round its own spawn points.
- `func_bomb_target`: `target` (fired when the bomb planted in this zone
  explodes). A Terrorist carrying the bomb inside it may plant. Stock:
  `de_dust2` fires `tgt_a`/`tgt_b` (crates); `de_cbble` fires `bomb_a`,
  which names both a breakable and a `multi_manager` firing three
  `env_explosion`s at 0, 0.4, 1.2, 1.9, 2.1, 3.1 and 3.7 s.
  (`info_bomb_target` is the point form; not in stock maps.)
- `func_hostage_rescue`: a Counter-Terrorist leading hostages into it
  rescues them. `info_hostage_rescue` (`cs_havana` 4, `cs_italy`,
  `cs_militia`) is the point form with a radius. A hostage map with
  neither uses the Counter-Terrorist spawn points.
- `func_vip_safetyzone` and `info_vip_start` (`as_oilrig`): where the
  VIP must reach, and where he spawns.

### hostage_entity

A point entity (the editor draws it as a player-sized box 32 × 32 × 72
round its origin). Keys: `origin`, `angles` or `angle` (facing yaw),
`model` (stock `models/hostage.mdl` and `models/scientist.mdl`, once
with a leading `/`), `skin` (0-3), `body`, `gravity`.

**Placement**: the stock origins are not at a fixed height above the
floor: the floor is 36.5 units below them in `cs_office`, 32.5 in
`cs_italy`, 3.5 in `cs_militia` and 4.5 in `cs_747` (measured down the
point hull). So the origin is a "place near here" and the game settles
the hostage onto the floor below it at spawn; facing its yaw. Its box,
model choice (CS 1.6 ships `cstrike/models/hostage.mdl` and
`hostage01`..`08`; `scientist.mdl` only under `valve/`) and how it is
dropped are measured (Open questions).

### armoury_entity

A point entity: a weapon or item lying on the floor for pickup. Keys:
`item` (0 `weapon_mp5`, 1 `weapon_tmp`, 2 `weapon_p90`, 3 `weapon_mac10`,
4 `weapon_ak47`, 5 `weapon_sg552`, 6 `weapon_m4a1`, 7 `weapon_aug`,
8 `weapon_scout`, 9 `weapon_g3sg1`, 10 `weapon_awp`, 11 `weapon_m3`,
12 `weapon_xm1014`, 13 `weapon_m249`, 14 `weapon_flashbang`, 15
`weapon_hegrenade`, 16 `item_kevlar`, 17 `item_assaultsuit`, 18
`weapon_smokegrenade`), `count` (how many can be taken before it is
gone; 1 in both stock ones), `angles`. Stock: `de_storm` (`item` 4, an
AK-47, origin 17.5 units above the floor) and `de_torn` (`item` 3, a
MAC-10, on the floor, yaw 236). Drawn as the item's world model; a
player touching it picks it up as he would a dropped weapon; it comes
back each round. Details in Open questions.

### info_map_parameters

Point entity, at most one per map. Keys: `buying` (0 both teams may buy,
1 only Counter-Terrorists, 2 only Terrorists, 3 neither) and
`bombradius` (the bomb's explosion radius; default 500). Stock:
`de_chateau` 300, `de_nuke` 350 (`buying` 0), `de_piranesi` and
`de_survivor` 500. Read at map start; no logic.

### light, light_spot, light_environment: light styles

Light is baked into the map's lightmaps by the compiler; at run time
only **switchable** lights matter.

- A face's lightmap has up to four styles (bsp.md "Lightmaps"). Each
  style number has a **pattern**: a string of letters `a`..`z`, played
  at 10 letters a second on the client and looping, each letter giving
  a brightness: `a` dark, `m` normal, `z` about twice normal (the scale
  is linear in the letter; exact factor in Open questions). The face's
  light is the sum of its style lightmaps, each scaled by its style's
  current letter.
- Styles 0-12 have fixed patterns set by the game at map start (data
  the server sends to every client): 0 `m`; 1
  `mmnmmommommnonmmonqnmmo`; 2
  `abcdefghijklmnopqrstuvwxyzyxwvutsrqponmlkjihgfedcba`; 3
  `mmmmmaaaaammmmmaaaaaabcdefgabcdefg`; 4 `mamamamamama`; 5
  `jklmnopqrstuvwxyzyxwvutsrqponmlkj`; 6 `nmonqnmomnmomomno`; 7
  `mmmaaaabcdefgmmmmaaaammmaamm`; 8
  `mmmaaammmaaammmabcdefaaaammmmabcdefmmmaaaa`; 9 `aaaaaaaazzzzzzzz`;
  10 `mmamammmmammamamaaamammma`; 11
  `abcdefghijklmnopqrrqponmlkjihgfedcba`; 12 `mmnnmmnnnmmnn`; 63 `a`.
  These are the editor's "Appearance" choices (0 normal, 10 fluorescent
  flicker, 2 slow strong pulse, ...). Stock: style 0 on 21 lights, 10,
  4 and 1 once each.
- **Switchable lights**: a `light` or `light_spot` with a `targetname`
  is given its own style (32, 33, ...) by the compiler, which writes it
  into the entity's `style` key (stock: 68 with 32, 42 with 33; several
  lights with one name share one style). At spawn, if `style` ≥ 32: the
  style's pattern is set to `a` if spawnflag 1 "Initially dark", else
  to the light's `pattern` key if any, else `m`. **When used** (by the
  use-type rule; on = not dark): on → pattern (or `m`), off → `a`. The
  change is sent to all clients at once.
- A light without a name does nothing at run time (it is removed).
- `light_environment`: also sets the sky light colour used to light
  models (from `_light`); its sun direction (`angles`, `pitch`) and
  brightness are compile-time. The model-lighting part belongs to the
  studio-model lighting spec (Open questions).

Worked example, `cs_militia`: `shack` (2 lights, style 33) toggled by
the button `*29`; `slight` (25 lights, style 32) toggled by the rotating
lever (see func_rot_button): faces lit by those lights drop to only
their other styles' light while the style reads `a`.

### env_sprite, env_glow, cycler_sprite

- `env_sprite`: a sprite (`model` = a `.spr`; stock mostly
  `sprites/ikgrass1-4.spr` grass, `xsmoke1`, `flare1-2`, `snow`), drawn
  with the render keys (`rendermode` 2 texture, 5 additive, 3 glow;
  `renderamt`, `rendercolor`; `renderfx` 14 "constant glow" on 32),
  `scale` (size multiplier; stock .5 on half), `framerate` (frames per
  second; stock 10), `angles` (orientation for oriented sprite types; a
  yaw with no roll is moved into the roll, so `0 90 0` means roll 90 —
  the editor only writes yaw). Spawnflags: 1 "Start on", 2 "Play Once".
  - An unnamed sprite is always on; a named one starts off unless flag
    1. **Use** turns it on or off by the use-type rule; turning on
    restarts at frame 0.
  - Animation (server side, every 0.1 s): frame += framerate × elapsed
    time; past the last frame index: Play Once turns it off, otherwise
    the frame wraps modulo (frame count − 1) (Quirks). It animates only
    if it has more than two frames and a framerate, or Play Once.
  - The orientation types and drawing are spr.md's.
- `env_glow`: a sprite (`model`, stock `sprites/glow01-04.spr`,
  `flare1`) always on, no use; `rendermode` 3 glow (212 of 242: its size
  on screen does not shrink with distance and it fades when hidden) or 5
  additive; `scale`. Animates like `env_sprite` (never turns off).
- `cycler_sprite` (stock 125, in 5 maps): despite its name holds studio
  models in the stock maps (`models/palmtree.mdl`, `pred_plant`,
  `bush`, `cs_italy`'s `chick.mdl` chickens); shows the model, playing
  its first sequence at `framerate`, with the render keys. Its use and
  damage behaviour are in Open questions; the `cs_italy` chickens are
  "killed" only by `env_render` making them invisible.

### env_render

Point entity: when used, copies its own render keys to every entity
named `target`. Spawnflags mask what is copied: 1 not `renderfx`, 2 not
`renderamt`, 4 not `rendermode`, 8 not `rendercolor`. A key absent from
the `env_render` copies as 0.

Worked example, `cs_italy` chickens: the flesh breakable `*66` (`health`
1) fires `chicken1kill`: an `ambient_generic` (`misc/killChicken.wav`),
an `env_shooter` (15 `models/feather.mdl` gibs) and an `env_render`
(`rendermode` 4, no `renderamt` → 0) aimed at the `cycler_sprite`
`chicken1`, which becomes invisible. A `trigger_multiple` (`wait` 180)
fires `chicken1respawn`, an `env_render` with `renderamt` 255: the
chicken is visible again. The breakable itself does not come back
until the round restarts.

### ambient_generic

A positioned sound. Keys: `message` (the sound file, relative to
`sound/`; stock e.g. `ambience/wind2.wav`, `de_torn/torn_ambience.wav`;
a name starting with `!` is a sentence), `health` (volume 0-10; × 10
percent, clamped 0-100), `pitch` (0-255, 100 normal; stock 100 on 316,
also 110, 150, −35 → clamped to 0), `pitchstart`, `volstart` (0-10),
`fadein`, `fadeout`, `spinup`, `spindown` (0-100), `lfotype` (0 off, 1
square, 2 triangle, 3 random), `lforate`, `lfomodpitch`, `lfomodvol`,
`cspinup`, `preset` (0-27 named presets; 0 in every stock one).
Spawnflags: 1 "Play Everywhere" (no attenuation), 2 "Small Radius"
(idle attenuation), 4 "Medium Radius" (static attenuation), 8 "Large
Radius" (normal attenuation), 16 "Start Silent", 32 "Is NOT Looped".
With no radius flag the attenuation is static. Stock flag sums: 36
(medium, not looped) on 94, 2, 56, 4, 8, 40, 52, 34 ...

- **Spawn**: with an empty `message` it is removed. A looped sound
  (flag 32 clear) that is not Start Silent starts at once (sent as a
  spawning ambient sound: clients joining later hear it too). A
  not-looped sound never starts by itself.
- **Use** (use-type rule; *set* with a value changes only the pitch of
  a playing sound to value × 255): if playing: stop (with fade-out or
  spin-down if set). If not playing: a looped sound starts and is
  playing; a not-looped one is stopped first (a long one-shot is
  restarted) and played once — it stays "not playing", so every use
  plays it again.
- Looping is the sound file's: a looped `ambient_generic` with a wave
  that has no loop markers plays once (mount/sound spec).
- Ramps (every 0.2 s while any is active): fade-in from `volstart`×10 to
  `health`×10 percent, in steps of (101 − fadein) × 64 / 256 percent;
  fade-out the same way down (the sound stops at the start volume);
  spin-up/down the same with pitch from `pitchstart` to `pitch`; the
  LFO modulates pitch and volume by ± modpitch/modvol percent of its
  wave. Stock use: 9 sounds fade in/out with 30, two have an LFO.

Worked example, `de_aztec` thunder: two `trigger_multiple`s (`wait` 45)
fire the `multi_manager` `thunder`, which fires `boom` at 0 s: an
`ambient_generic` with `ambience/thunder_clap.wav`, `health` 10,
spawnflags 49 (everywhere + start silent + not looped): every player
hears one thunder clap at full volume, at most every 45 s per trigger.

### Other entities in the stock maps

- `env_explosion` (33): when used, an explosion of `iMagnitude` (stock
  150-2200) at its origin: a fireball sprite, smoke, a scorch decal,
  sparks and radius damage of magnitude (blast type) falling off
  linearly to 0 at magnitude × 2.5 units (Half-Life's rule; CS's in
  Open questions). Spawnflags: 1 no damage, 2 repeatable (otherwise
  removed after use), 4 no fireball, 8 no smoke, 16 no decal, 32 no
  sparks (stock 51 = 1+2+16+32, 19, 2, 3: most stock explosions do no
  damage).
- `env_spark` (3): random sparks every random [0, `MaxDelay`] s;
  spawnflags 32 "Toggle" (use switches it), 64 "Start ON".
- `env_shooter` (2, `cs_italy`): when used, throws `m_iGibs` copies of
  `shootmodel` at `m_flVelocity` with spread `m_flVariance`, living
  `m_flGibLife` s; flag 1 repeatable.
- `env_beam` (3, `de_cbble`): a beam between two named entities
  (`LightningStart`, `LightningEnd`), sprite `texture`, width, noise,
  strike times; flags as the editor lists them.
- `env_sound` (44, mostly `cs_siege`): a sphere of `radius` units in
  which the listener's room effect becomes `roomtype` (stock 17-19
  concrete, 22 big, 24 cavern, 3 metal); belongs to the sound spec.
- `env_rain` (1, `de_aztec`, a brush): CS's weather volume; rain falls
  inside it. Measure.
- `trigger_camera` (63) with `info_target` (69): a camera at its origin
  looking at `target` for `wait` seconds (stock 10), moving along
  `moveto` path corners at `speed` with `acceleration`/`deceleration`.
  Stock uses: security monitors (`cs_militia`'s `CAMERA1`..`5`, fired
  by `func_button`s) and, in CS, the views shown to a player while he
  picks a team (every map from `de_dust2` up has some). Belongs to the
  spectator/UI spec.
- `infodecal` (193): a decal by texture name at its origin, applied at
  map start (bsp.md, wad3.md decals). A named one is applied when fired
  (none of the stock ones is named; to confirm).
- `item_generic` (4, `de_airstrip`) and `cycler` (1, `de_storm`): a
  studio model playing a sequence (`sequencename`); decoration.

### +use: what a player can use

Half-Life's rule (CS 1.6's may differ; Open questions): when `+use` is
pressed (and every frame it is held, for continuously usable
entities), the game looks at every entity whose bounding box is within
64 units of the player's origin and is usable, computes the direction
from the eye to that entity's box (the vector from the eye to the box
centre, shortened by the box's half-size on each axis, zero if inside),
and picks the one whose direction has the greatest dot product with the
view's forward vector, if above 0.7. No line-of-sight trace. Found: the
entity is used with use type *set*, value 1 (the press makes
`common/wpn_select.wav`); none: `common/wpn_denyselect.wav`.

Usable in this sense: `func_button` and `func_rot_button` without
`health` (and without Touch Activates, which ignore the use),
`func_pushable` (continuously, for pulling), and doors with Use Only
(256). In Half-Life a door without Use Only cannot be `+use`d.

## Per-tick order

One server frame, Half-Life lineage (to confirm for CS 1.6):

1. Each client's commands run: movement, during which the player
   touches triggers and other entities as it moves, and `+use`.
2. Every entity is run in entity index order: movers advance by their
   velocity for the frame (pushing, carrying, or blocked), then run
   their timer if their own clock has reached it (move done, wait over);
   other entities run their timer if server time has reached it
   (delayed-fire timers, `multi_manager`, sprites, sounds).
3. Fires happen immediately inside whatever step caused them (a touch,
   a timer, a damage): a fire uses its receivers at once, in entity
   order, and those may fire further.
4. Entities marked for removal are freed at the end of the frame.

## Edge cases

- **Trigger overlap**: a brush trigger is touched when an entity's box
  overlaps the trigger's bounding box and, for its brush shape, the
  entity's origin is inside the trigger's clip hull for that box size
  (so sloped triggers are exact for players). To confirm (Open
  questions).
- A one-shot trigger (`wait` ≤ 0) is removed 0.1 s after firing; a
  second toucher in that 0.1 s does nothing.
- `trigger_multiple` re-arm uses server time; `func_door` waits use the
  mover's clock (a blocked door waits longer).
- A door fired while moving ignores the fire; a button pressed while
  moving ignores the press; a `multi_manager` used while running
  ignores the use (unless multithreaded).
- `killtarget` naming the firing entity itself removes it after its
  fire.
- A `func_breakable` loses its name when it breaks: later fires of that
  name do not reach it while it is being removed.
- Two `func_door`s with the same name both receive every fire; the
  blocked rule keeps them in step.
- `angle` −1/−2 apply to linear movers only; rotating ones ignore
  `angle` for their axis.
- A `trigger_relay` replaces the activator with itself, so a door
  opened through a relay remembers the relay, not the player: a rotating
  door then swings by the relay's position.

## Quirks

- `func_rotating` reads its volume only from a key spelled `Volume`
  (capital V); the stock maps write `volume`, so the volume stays at the
  default 10 (full). Keep (it is what players hear).
- `env_sprite` and `env_glow` wrap their frame modulo (frame count − 1),
  so the last frame of a looping sprite is shown only as a fraction of
  a frame. Keep.
- `trigger_multiple`'s `sounds` and `style` keys do nothing; the editor
  writes them anyway.
- Door spawnflag 4 "Don't link" has no effect; doors that share a name
  always act together when blocked.
- Doors and buttons fire `target` with the use type *toggle* on both
  arrivals; a `func_wall` with the same name as a door's target flips
  its frame twice per door cycle.
- `de_survivor`'s `CLIC` blinker and `cs_747`'s `pl_l_mm` loop never
  stop once started.
- A Toggle `func_button` whose master turns false while it is going in
  stays in for good (no fire, no return).

## Test cases

| Setup | Input | After | Expected |
|---|---|---|---|
| every stock map | count classes | | as the table in "Entity counts" (e.g. 1 655 `func_wall`, 63 `func_door`, 31 `func_door_rotating`, 21 `func_button`) |
| `de_dust2` | list logic entities | | 10 `func_breakable` (5 `tgt_a`, 5 `tgt_b`, flag 1), 2 `func_bomb_target` (`*42` → `tgt_a`, `*16` → `tgt_b`), 2 `func_buyzone` (`team` 2 `*29`, 1 `*30`); no door, button or trigger |
| `cs_office` `door1` `*200` | spawn | | movedir (0 0 1), D = 108, P2 = P1 + (0 0 108), closed |
| same | fired once | 2.7 s | at P2, stop sound `doors/doorstop1.wav` |
| same | fired once | 22.7 s | starts closing; 25.4 s back at P1 |
| `cs_office` `*199` (flags 33, `delay` 1) | `+use` | same frame | frame 1 (`+a~sp_lock` drawn); `door1` not yet fired |
| same | | 1 s | `door1` fired, opening |
| same | second `+use` at 5 s | same frame; 6 s | frame 0; `door1` fired again at 6 s and ignored (door is open) |
| `de_nuke` `*40` trigger | player enters | | 4 `GlassDoor2` doors open; each 62 units in 1.24 s; close 10 s after arriving |
| same | player stays inside | 2 s | the trigger fires again; doors (open) ignore it |
| `cs_747` `curt1` `*11` | its trigger entered | 0.35 s | moved 63 units along +X; players pass through it throughout |
| `as_oilrig` `elevator` `*22` | spawn | | origin 505 units below its drawn place |
| same | button `*23` touched | 3 s; 8.05 s; 13.05 s; 18.1 s | starts up; at the top; starts down; at the bottom |
| same | player under it on the way down | | player takes 200 crush damage; elevator reverses up |
| `cs_siege` `caveelev` `*121` | a button used | 2 s; 7.32 s | starts up; at the top (638 units), stays |
| same | a button used again | 2 s later | starts down |
| `cs_militia` rotating door `*1` | player walks into it from +Y side heading −Y | 0.9 s | turned 90° about Z, away from the player; back closed 3.9 s + 0.9 s later |
| same | a player blocks it while closing | | it reopens (wait 3 ≥ 0), no damage (`dmg` 0) |
| `de_storm` `door1` pair | touched | | not opened (named); no sound |
| `cs_militia` lever `*38` | `+use` | 1.2 s | turned 180° about X; style 32 pattern `a` |
| same | `+use` again | 1.2 s | back; style 32 pattern `m` |
| `de_survivor` `*53` trigger | entered | 0 s, 0.5 s, 1 s | `*54` frame 1, 0, 1 ...; style 32 alternates `a`/`m`; the 4 `CLIC` sprites alternate on/off |
| `cs_747` | map start | 0.1 s, 0.6 s, 3.1 s, 4.1 s | `l1` fired at 0.1 and 0.6; `pl_l_r` at 3.1; `pl_l_mm` restarts at 4.1 |
| `de_piranesi` `snd1a` | spawn | | 16 targets read, sorted by delay (`brdG` 0 first, `frmG` 125 last) |
| a `multi_manager` with a 17th key | spawn | | 17th ignored |
| `de_dust2` | bomb explodes at the `*42` site | | the five `tgt_a` crates break (each a magnitude-100 explosion); `tgt_b` intact |
| `de_dust2` `tgt_a` crate | shot for 1000 | | no damage (Only Trigger) |
| `cs_italy` `*66` | shot | | breaks (flesh); 15 feathers; `chicken1` `renderamt` 0 |
| `as_oilrig` `*31` trigger_hurt (500, fall) | player stands inside | 0, 0.5 s | 250 damage each half second |
| a trigger_hurt `dmg` 10, two players enter on different frames within 0.5 s | | | both hurt 5 within the first window, once each |
| `de_aztec` thunder | trigger `*71` entered twice 10 s apart | | one clap (`wait` 45) |
| `de_survivor` tracktrain `*23` | map start | | moves along `path_track` `1`→`2`→... at 240 units/s, passable |
| face textured `+ax` on an entity with frame 1, family has `+0x`,`+1x` | draw at client time 0.15 s | | `+1x` (the normal set's frame 1 at 10 frames/s) |
| face textured `+0x`, family without `+a` | frame 1 | | `+0x` set (no alternate) |

## Open questions

1. **Round restarts (CS game rules).** What CS 1.6 puts back at the
   start of each round: broken `func_breakable`s (believed restored),
   doors and buttons (believed reset to closed/out, mid-move ones too),
   `func_pushable` positions, hostages, `armoury_entity` counts, `light`
   styles, `env_render` changes (`de_piranesi`'s `roundreseter` suggests
   render changes are **not** restored), `env_sprite` on/off,
   `ambient_generic`s, `trigger_once`/`trigger_multiple` re-arm, removed
   (`killtarget`ed) entities, running `multi_manager`s and delayed
   fires, `trigger_auto` (does it fire each round?). Measure on the
   server: break, move and fire one of each, then `sv_restartround 1`
   and log each entity's state.
2. **`+use` in CS 1.6**: whether CS opens doors without Use Only by
   `+use` (players report pressing use on doors), its search radius and
   cone, and whether it traces for line of sight. Probe: a puppet player
   at measured distances and angles from `cs_militia`'s `*1` and
   `cs_office`'s `*199`.
3. Whether hostages (monsters) open doors by touch, fire triggers with
   flag 1, and are pushed and crushed by movers.
4. **Objective volumes**: buy zone size when a map has none (round each
   spawn point?), the bomb's plant rule inside `func_bomb_target`, the
   radius of `info_bomb_target` and `info_hostage_rescue`, the rescue
   area round CT spawns, the VIP zone; whether every bomb target fires
   its `target` when the bomb explodes or only the one it was planted
   in; whether `bombradius` also scales the bomb's damage.
5. **hostage_entity**: the box size, whether it is dropped to the floor
   (the stock origins say yes), the model used (`models/hostage.mdl`,
   the `hostage01`..`08` set, or the map's `model` key — `scientist.mdl`
   is only in `valve/`), and what `skin` and `body` select.
6. **armoury_entity**: its world models per item, drop to floor, the
   pickup rules (team, already carrying a primary), ammo given, and
   whether `count` > 1 respawns at once.
7. Explosion radius and falloff in CS 1.6 (`env_explosion`, breakable
   `explodemagnitude`): Half-Life's magnitude × 2.5 linear falloff, or
   CS's own; armour interaction belongs to `specs/cs16/damage.md`.
8. `func_breakable` damage types in CS: whether the knife counts as a
   melee ("club") hit for the double damage and Instant Crowbar, how
   bullets that pass through glass count, and whether HE grenades use
   blast damage on breakables.
9. Pusher details from the engine: rotating movers turning a rider's
   yaw; whether a blocked mover's clock really pauses (door timings with
   a blocker); the order of pushed entities. Measure with a puppet
   standing in `cs_office`'s `door1` path and on `as_oilrig`'s elevator.
10. Trigger overlap test: confirm the clip-hull check against a sloped
    trigger (none in stock maps is sloped; use a test map) versus a plain
    box overlap.
11. Light style brightness scale per letter (`a` 0 to `z`), and how it
    combines with the lightmap brightening (bsp.md question 1);
    `light_environment`'s sky colour for model lighting (a power curve
    of the `_light` values; belongs to the studio lighting spec).
12. `func_water` `WaveHeight` drawing (amplitude, wavelength, speed) and
    the `!` warp: already bsp.md's; capture `de_aztec` water.
13. `cycler_sprite` holding a studio model: does it react to `+use` or
    damage (Half-Life's cycler toggles animation on use), and the
    `cs_italy` chicken's sequence and frame rate. `cycler` and
    `item_generic` likewise.
14. `func_tracktrain`/`path_track`: a full spec (orientation from the
    track and `wheels`, `height`, banking, speed changes at path
    points, `altpath`, user control) if a later game needs it;
    `de_survivor` only needs the decoration loop.
15. `func_pushable`: the exact `+use` pull rule (it depends on a server
    tick factor in later Half-Life builds), the push sound's rate.
16. `env_rain`: CS's rain volume (drop sprite, density, wind) for
    `de_aztec`.
17. `trigger_multiple` `message`: whether CS shows it as centre text, as
    a HUD message, or looks it up in `titles.txt` first.
