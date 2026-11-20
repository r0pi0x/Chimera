# Team Fortress 2: the first-person view model (T9)

Source basis: Valve's Source SDK 2013 as updated on 2025-02-18 with
TF2's client and server game code (read in place, outside this
repository): TF2's view-model entity (placement, lowering, the minimal
view models' offset, the Minigun's barrel), TF2's weapon base (which
model is the view model, deploy, idle, the activity translation per
slot, bob), the Minigun's weapon code (its states, animations and
barrel spin), the item entity's attachment model (how the `c_` weapon is
bone-merged onto the arms), the shared view-model entity and the client
view set-up (FOV, aspect, the view-model pass), the client animating
entity (bone merge, sequence cross-fades) and the shared animation
decoder. The model numbers (eye-space positions in the test cases) were
computed from the install's `c_*_animations.mdl` and `c_*.mdl` files with
a throwaway reader in a scratch folder (not in the repository), decoding
frame 0 of each sequence as Source's decoder does. Shared Source
behaviour that CS:S already has is referenced to
[specs/cs_source/view_models.md](../cs_source/view_models.md), not
repeated.
Status: checked against the SDK's game code (2026-11-15)

## Summary

- The view model is **the class's arms** (`c_<class>_arms.mdl`, whose
  sequences come from `c_<class>_animations.mdl`), placed every frame
  **exactly at the eye with the eye's angles**: no fixed offset in normal
  play. The **`c_` weapon** is a second model with the same origin whose
  bones are **bone-merged** by name onto the arms; it plays no animation
  of its own on the merged bones. All the placement of the gun on screen
  comes from the arms' animation.
- It is drawn in its own pass at **`viewmodel_fov` 54** (a horizontal
  angle for a 4:3 screen, so **41.83° vertical at every aspect**), near
  plane 1 unit, depth squeezed into the first tenth of the buffer, like
  CS:S. The only per-frame motion is TF2's bob (CS:S's primary bob
  model). **TF2 has no sway**: the lag behind view rotation is switched
  off.
- After the draw, the slot's **idle** sequence plays (the Heavy's
  Minigun: `m_idle`). In `m_idle` the Heavy holds the Minigun **pitched
  up about 56°**, low and to the right: at TF2's FOV only the front of the
  barrels (upper right of the screen, pointing up) and the top of the left
  glove (bottom edge) are visible. The gun levels out only while it spins
  (`m_spool_up` lowers it, `m_spool_down` raises it again).

## Why ours shows a huge, wrongly angled Minigun

Our first person draws the Minigun's **spun pose** (`m_spool_up` as the
draw, `m_spool_idle` as the idle) and then **raises the whole view model
11 units and moves it 7 to the right** (the gun data's `view_offset`).
Neither happens in TF2:

1. The idle at rest is `m_idle` (activity `ACT_PRIMARY_VM_IDLE`), and
   the draw is `m_draw` (`ACT_PRIMARY_VM_DRAW`), which ends exactly in
   `m_idle`'s first pose. The upright pose we decoded (gun below the eye,
   barrels pointing steeply up) **is the real one**: decoded with
   Source's rules it puts the muzzle at 55 forward, 17 right, 7 up of the
   eye, on screen at the upper right (section 8, test cases 1 and 2).
   That is the "small gun, barrels up, only a glove" of the real game.
2. There is **no extra offset** in TF2's normal mode. The offsets that
   exist (section 3) are off by default; the item's
   `min_viewmodel_offset` applies only with `tf_use_min_viewmodels 1`,
   and it is **(forward, right, up)**, not (forward, left, up).
3. The spun pose's weapon bone sits only 8.5 units ahead of the eye and
   21 below it (`m_idle`'s: 19 ahead, 37 below); raised 11 units and
   moved right it comes close to the view centre, so the gun fills the
   screen and the right forearm enters the frame.
4. The view-model FOV must be 54 when the player is not zoomed,
   **whatever the player's own base FOV is** (section 5): TF2 measures
   the zoom against the player's own base FOV (`fov_desired`, 75 to 90),
   not against a fixed 75.

So: draw `m_draw`, idle `m_idle`, no `view_offset`, view-model FOV 54
horizontal at 4:3 (41.83° vertical), and the result matches the game.

## Units and conventions

- Source units (inches); 1 unit = 0.0254 m.
- The view model's own space is the eye's: **x forward, y left, z up**
  (right-handed), origin at the eye. "Right" below is −y. Angles in
  degrees (pitch, yaw, roll), pitch positive looks down.
- A bone's rotation in a model is stored as Euler angles in radians
  (x, y, z) = rotation about x, then y, then z, composed as
  R = Rz(z) · Ry(y) · Rx(x) (the same convention as everywhere in Source's
  models, specs/cs_source/view_models.md).
- Time: the view model is placed and posed once per rendered frame on
  the client; the weapon's state (which sequence) runs on the 66.67 Hz
  tick (0.015 s) and is predicted.

## Constants

| Name (ours) | Value | Unit | Meaning |
|---|---|---|---|
| viewmodel_fov | 54 | deg (horizontal, 4:3) | `viewmodel_fov`, client, saved; range 0.1 to 179.9 (54 to 70 on competitive servers) |
| vm_vertical_fov | 41.828 | deg | 2·atan(tan 27° · 3/4); the same at every aspect |
| fov_desired | 75 | deg | the player's base FOV, client, saved; the server clamps it to 75..90 |
| default_fov | = the player's base FOV | deg | re-set every frame from the player's base FOV (so zoom is the only offset) |
| vm_znear | 1 | unit | the view-model pass's near plane |
| vm_depth_range | 0 .. 0.1 | | depth range of the view-model pass |
| cl_bobcycle | 0.8 | s of bob time | vertical bob period (cheat) |
| cl_bobup | 0.5 | fraction | rising part of a cycle (cheat) |
| bob_speed_max | 320 | unit/s | horizontal speed is clamped to this for the bob |
| cl_wpn_sway_interp | 0 | s | TF2's sway history length; **0 = no sway** (cheat, development-only) |
| cl_wpn_sway_scale | 5 | | the sway's scale if it were on |
| shake_scale | 0.1 | | the view model shakes at 10 % of the view's shake |
| cl_gunlowerangle | 90 | deg | pitch added when the weapon is lowered |
| cl_gunlowerspeed | 2 | deg per frame | how fast it lowers or rises |
| tf_use_min_viewmodels | 0 | bool | minimal view models (the item's `min_viewmodel_offset`) |
| deploy_time | 0.5 | s | weapon switch time (stock) |
| draw_playback | 0.67 / 0.5 = 1.34 | x | the draw sequence's playback rate (stock) |
| seq_fade | 0.2 | s | cross-fade into a sequence without the snap flag |
| minigun_spinup | 0.75 | s | wind-up; the view model's playback rate is 0.75 / (wind-up time) = 1 |
| minigun_idle_after_winddown | 2.0 | s | from the wind-down until the idle sequence is sent |
| barrel_max_speed | 20 | rad/s | the Minigun's barrel spin at full speed |
| barrel_accel_step | 0.1 | rad/s per update | change of the barrel's spin speed per update (0.5 for "can holster while spinning" items) |
| cl_flipviewmodels | 0 | bool | mirror the view model (left-handed); off |

## Behavior

### 1. The two models

- **Which arms.** A weapon whose item says it attaches to the hands (every
  stock `c_` weapon: `attach_to_hands 1` in the item schema) uses the
  owner's **class arms model** as its view model:
  `models/weapons/c_models/c_<class>_arms.mdl` (the Engineer with the
  Gunslinger uses a second hands model; not needed here). Other weapons
  use the one-piece model named in their weapon script.
- **Which sequences.** The arms model has 1-frame stubs and includes
  `c_<class>_animations.mdl`, which holds the real sequences
  (install.md 4.3). Sequences are chosen **by activity**, so the include's
  sequences are the ones played. The include has the **same skeleton**
  as the arms (same 58 to 68 bones, same names, same parents, same
  default poses and scales; checked for the Heavy), so its animation data
  decodes against either.
- **The weapon.** The item's `model_player` (`c_minigun.mdl` …) is
  created on the client as a separate model **parented to the view model
  at zero offset**, drawn in the view-model pass, with **bone merge**:
  - every weapon bone whose **name** exists in the arms takes the arms
    bone's full transform (position and rotation in the eye's space) as
    posed this frame;
  - every other weapon bone (the Minigun's `barrel`, any `c_weapon_stattrack`)
    is posed from the weapon model's **own** first sequence (`idle`,
    1 frame) relative to its parent bone, which is usually the merged
    `weapon_bone`;
  - the weapon's attachments (`muzzle`, `eject_brass`) hang off its own
    bones, so they follow the merged `weapon_bone`.
  No pose flows back from the weapon to the arms.
- **Skins.** The arms use skin 0 on RED and 1 on BLU (when the weapon
  has team skins for its view model; stock ones do); the weapon model
  takes the view model's skin unless the item names its own.
- **Lighting.** The view model is lit as if it stood at the **centre of
  the player's body** (its owner's world-space centre), not at the eye.
- **Handedness.** `c_` models are built right-handed and are **not
  mirrored** (`cl_flipviewmodels` 0). With it set, the view model is
  mirrored across the eye's vertical plane and drawn with reversed
  winding (as CS:S's flip).

### 2. Activities: what the weapon asks for and what plays

The weapon code asks for generic view-model activities (draw, idle,
primary attack, secondary attack, reload, reload start, reload finish,
pre-fire, post-fire). Before a sequence is chosen, each is translated by
the weapon's **slot role** (primary, secondary, melee, item1 …) into the
slot's activity: draw becomes `ACT_PRIMARY_VM_DRAW` for a primary,
`ACT_SECONDARY_VM_DRAW` for a secondary, and so on; the Minigun's
pre-fire and post-fire become `ACT_PRIMARY_ATTACK_STAND_PREFIRE` and
`ACT_PRIMARY_ATTACK_STAND_POSTFIRE`. An item's `anim_slot` replaces the
role, and its `animation_replacement` list can remap single activities
first. The sequence is then the arms' sequence carrying that activity
(install.md 4.3, weapons.md 8.2, weapons_more.md 9.2 list them).

Sending a sequence **always restarts it at cycle 0**, even when it is
already playing (the Minigun avoids this for its fire loop; section 7).

### 3. Placement each frame

With the eye position E and the eye angles A (the camera's):

1. Start with origin O = E and angles Q = A.
2. **Lowered weapon** (only when the game lowers it: the losing team in
   the round's bonus time, and similar): a pitch offset that approaches
   90° (`cl_gunlowerangle`) by 2° per rendered frame (`cl_gunlowerspeed`;
   frame-rate dependent) and returns to 0 the same way; added to Q's
   pitch. When fully lowered (within 0.1° of 90) the view model is not
   drawn. Normal play: 0.
3. **Minimal view models** (`tf_use_min_viewmodels` 1, off by default, or
   the cheat `tf_viewmodels_offset_override` "x y z"): O += x·forward +
   y·**right** + z·up of A, with (x, y, z) the item's
   `min_viewmodel_offset` (or the override). The stock values
   (weapons.md 8.1, e.g. the Minigun 10 0 −9) belong here only.
4. **Bob** (section 4): changes O and Q.
5. **Sway: none.** TF2 replaces Source's generic spring lag with a
   history-based sway that is gated on `cl_wpn_sway_interp`, and that is
   0. Neither the spring lag nor its pitch-dependent offsets apply.
6. **Shake**: the view's screen shake applied at 10 % amplitude.
7. The arms model is placed at O with angles Q; the weapon follows by
   bone merge.

The inspect animation's own offset (an item attribute) is ignored here:
we have no inspect.

### 4. Bob

Exactly the primary model of specs/cs_source/view_models.md section 3
(TF2's is the code that model was taken from): speed = the player's
horizontal speed, slew-limited to 320 units/s per second, clamped to
320; bob time advances at speed/320; vertical V = 0.005·s·(0.3 + 0.7 sin θ)
on a 0.8 period with `cl_bobup` 0.5, lateral L the same at half the
frequency; applied as O += 0.4V·forward, O.z += 0.1V, roll += 0.5V,
pitch −= 0.4V, yaw −= 0.3L, O += 0.2L·right (forward and right from Q
before this step).

Two facts that correct the CS:S spec's remark about TF2:

- The bob state lives on the **view model**, so it is **shared by all of
  the player's weapons** (not one state per weapon).
- The lateral phase is computed as bob time minus a whole number of
  double periods taken with a doubled count (an operator-order slip in
  the original). With `cl_bobup` 0.5 the phase stays on the rising
  branch and the integer part drops out of the sine, so L = 0.005·s·
  (0.3 + 0.7·sin(π·T_b/0.8)): exactly the half-frequency sine. Only with
  a changed `cl_bobup` would it differ.

A Scout at 400 units/s bobs as at 320 (the clamp).

### 5. FOV, projection and the pass

- **View-model FOV** each frame: fov_vm = `viewmodel_fov` − (base − current),
  where base is the player's **own** base FOV (`fov_desired`, clamped by
  the server to 75..90; the client copies it into `default_fov` every
  frame) and current is the player's FOV now (smaller only while zoomed).
  Not zoomed: fov_vm = 54, at base 75 and at base 90 alike.
- fov_vm is horizontal for a 4:3 screen. For aspect a: horizontal
  2·atan(tan(fov_vm/2)·a/(4/3)); vertical 2·atan(tan(fov_vm/2)·3/4) =
  41.828° for 54. The world FOV is converted the same way (with the
  optional 1.85:1 cap of specs/cs_source/view_models.md section 5; the
  view model never uses the cap).
- Near plane 1 unit, far = the world's; drawn after the world with depth
  range [0, 0.1], opaque parts then translucent; attachment positions
  used by world-pass effects are re-projected by
  tan(world fov/2) / tan(view-model fov/2) (specs/cs_source/view_models.md
  section 5).
- Zoomed (the Sniper's scope): fov_vm becomes negative or tiny, but the
  scope hides the view model, so it is never drawn so.

### 6. Draw, idle and the cross-fades

**Deploy.** Switching to a weapon sends the draw activity, then sets the
view model's **playback rate to 1.34** (0.67 s / 0.5 s at stock deploy
multipliers), so a draw sequence plays 1.34 times faster than its frame
rate (`m_draw`, 1.0 s long, is over after 0.746 s). The weapon can fire
0.5 s after the switch. Any later sequence sent by the weapon resets the
rate to 1 (sending a sequence resets it), except where the weapon sets
it again right after (reloads, the Minigun's wind-up).

**Idle.** The weapon's idle time is set to the draw's duration at rate 1
(the sequence's frames/fps). When it passes and nothing else is going
on, the idle activity is sent (`m_idle` for the Minigun, `sg_idle` for
the Scattergun …) and re-sent every idle duration (restarting a looping
sequence at 0 is seamless only if its last frame equals its first; the
stock idles loop). Between the end of a fast draw and the idle, the view
model holds the draw's last frame (0.25 s for `m_draw`), which for every
stock gun is the idle's first pose.

**Cross-fades.** When the sequence changes (or is re-sent), the client
blends:

- if the new sequence has the **snap** flag, the old one is dropped at
  once;
- otherwise the old sequence keeps playing (its cycle advancing at its
  own rate) and is blended over the new one with weight
  w = 3s² − 2s³, s = 1 − (t − t_change) / f, where f = the smaller of the
  old sequence's fade-out and the new one's fade-in (0.2 s for every
  sequence of these guns: f = 0.2 s); the blend is a per-bone spherical
  interpolation of rotations and a linear one of positions from the new
  pose toward the old with weight w. Several old sequences can be
  fading at once.

Snap or blend, from the animation files (fade-in 0.2 s, fade-out 0.2 s
throughout):

| Arms | Sequences that **snap** | Sequences that **blend in** |
|---|---|---|
| Heavy, Minigun | `m_draw`, `m_spool_up`, `m_spool_idle`, `m_fire`, `m_spool_down` | `m_idle` |
| Scout, Scattergun | `sg_draw` | `sg_idle`, `sg_fire`, `sg_reload_start`, `sg_reload_loop`, `sg_reload_end` |
| Scout, Pistol | `p_draw`, `p_fire` | `p_idle`, `p_reload` |
| Soldier, Rocket Launcher | `dh_draw`, `dh_idle`, `dh_fire`, `dh_reload_loop`, `dh_reload_finish` | `dh_reload_start` |
| Engineer, Shotgun | `fj_draw`, `fj_fire`, `fj_reload_start`, `fj_reload_end` | `fj_idle`, `fj_reload_loop` |
| Pyro, Flame Thrower | `ft_draw` | `ft_idle`, `ft_fire` |
| Demoman, Grenade Launcher | `g_draw`, `g_idle`, `g_fire`, `g_reload_start`, `g_reload_loop`, `g_reload_end` | |
| Demoman, Stickybomb Launcher | `sb_draw`, `sb_fire`, `sb_autofire`, `sb_reload_start`, `sb_reload_loop`, `sb_reload_end` | `sb_idle` |
| Sniper, Sniper Rifle | `draw`, `fire` | `idle` |
| Sniper, SMG | `smg_draw`, `smg_fire` | `smg_idle`, `smg_reload` |
| Medic, Syringe Gun | `sg_draw`, `sg_fire` | `sg_idle`, `sg_reload` |
| Spy, Revolver | `draw`, `fire`, `reload` | `idle` |

None of these sequences has inverse-kinematics rules, locks, automatic
layers, per-bone weights or blend grids: each is one plain animation.

### 7. The Minigun

**States and sequences** (weapons.md 7 has the timing rules; this is
what the view model shows):

| What happens | Activity sent | Heavy's sequence | Notes |
|---|---|---|---|
| switched to | draw | `m_draw` (snap) | at 1.34 x; ends in the idle pose |
| at rest | idle | `m_idle` (0.2 s blend) | gun raised, barrels up |
| attack or attack2 pressed: wind-up | pre-fire | `m_spool_up` (snap) | rate 0.75 / wind-up = 1; the 1.0 s sequence is cut at 0.75 s (frame 22.5 of 30), by then almost level |
| spun up, firing | primary attack | `m_fire` (snap, loops) | not restarted while it already plays: it loops through the burst |
| spun up, attack2 only (or out of ammo) | secondary attack | `m_spool_idle` (snap, loops) | re-sent every tick while spinning without firing (quirk below) |
| released: wind-down | post-fire | `m_spool_down` (snap) | 1.333 s, ends in the idle pose |
| 2.0 s after the wind-down | idle | `m_idle` | the 0.67 s between the end of `m_spool_down` and this holds its last frame (= the idle pose) |

**The barrel's spin.** Not animated by any sequence in first person: the
`barrel` bone of the `c_minigun` (not merged; its parent is
`weapon_bone`, at (0, −7.765, 27.145) in that bone's frame, its local
z axis along the barrels) is turned procedurally:

- a target spin speed: 20 rad/s from the wind-up and while firing, 0 at
  the wind-down;
- the current speed moves toward the target by 0.1 rad/s per update
  (stock; 0.5 for items that allow holstering while spinning), with one
  update per predicted command (first prediction only) and one per bone
  set-up of the weapon; the angle advances by speed · frame time per
  update. At 66.67 updates per second that is 6.67 rad/s², full speed
  3.0 s after the wind-up starts and a stop 3.0 s after the wind-down
  (frame-rate dependent in the original; use per tick);
- the barrel's local rotation is rebuilt with its Euler z component
  replaced by the accumulated angle (its x and y kept, both 0 here): a
  roll about the barrels' own axis.

In the third person the same procedural angle turns the world model's
`barrel`; the `c_minigun` model's own `spool_up`/`fire_loop`/`spool_down`
sequences are not what spins it.

## Per-frame order

1. (Tick, predicted) The weapon's state machine runs and may send a new
   view-model activity: translate by slot, pick the arms' sequence,
   restart it at cycle 0, reset the playback rate to 1 (then set it, for
   the draw, reloads and the wind-up). The Minigun updates its barrel
   speed.
2. (Frame) The client's base FOV is copied into `default_fov`; the view
   model's FOV is computed (section 5).
3. Placement (section 3): lowering, the minimal offset if on, bob, no
   sway, shake.
4. Posing the arms: the current sequence at its cycle, then the fading
   old sequences blended over it (section 6).
5. Posing the weapon: merged bones copied from the arms; its other bones
   from its own 1-frame `idle`; the Minigun's barrel roll replaced.
6. Draw: world, then the view-model pass (arms, then the weapon), lit at
   the player's centre.

## Edge cases

- **Spectating in first person** draws the target player's view model.
- **Dead**: not drawn.
- **`r_drawviewmodel 0`**: not drawn, and the Minigun's first-person
  muzzle and brass effects are not made either.
- **A weapon re-drawn after a long time**: the bob's first frame sees a
  large time step (a phase jump at the current speed; invisible).
- **Wind-up interrupted** (released before 0.75 s): the wind-down follows
  at once from wherever `m_spool_up` was (`m_spool_down` snaps, so the
  gun jumps to the spun-down start pose: `m_spool_down` frame 0 is the
  level pose). Ours may keep this jump.

## Quirks

- **The Minigun's spin-idle is restarted every tick** while spinning
  without firing (the weapon re-sends it each time it runs its attack
  code and nothing throttles it), so in practice `m_spool_idle` shows
  only its first frames. It is a 10-frame shake loop; showing its frame 0
  is close enough. Keep or loop it: either is invisible at a glance.
- **The draw is 1.34 x fast** and then holds its last frame until the idle
  is sent at the sequence's full-rate length.
- **The barrel's acceleration is per update, not per second** in the
  original, so it spins up faster at higher frame rates. Use the per-tick
  figure (3.0 s to full speed).

## Test cases

Eye-space positions are (forward, right, up) in units from the eye, the
view model at the eye with zero angles, decoded at frame 0 of the named
sequence, the weapon bone-merged. "Screen" is the position in normalized
device coordinates (−1..1, x right, y up) at vertical FOV 41.828° and
aspect 16:9.

| # | Setup | Expected |
|---|---|---|
| 1 | Heavy, `m_idle` frame 0, `c_minigun` merged | `weapon_bone` at (19.22, 2.93, −36.82); its z axis (the barrels) = (0.51, 0.24, 0.83): pitched up 56° and slightly right |
| 2 | as 1, the `muzzle` attachment | (55.32, 16.82, 6.78): screen (0.45, 0.32), upper right |
| 3 | as 1, which bones are on screen | only the left hand's pinky and ring knuckles (screen y ≈ −1.0) and the barrels' front; both hands' wrists and forearms below the screen |
| 4 | `m_draw` last frame (30), `m_spool_down` last frame (40) | the same pose as test 1 (to 0.01 units) |
| 5 | `m_spool_up` frame 0 | the pose of test 1; frame 30 = `m_spool_idle` frame 0 |
| 6 | Heavy, `m_spool_idle` frame 0 | `weapon_bone` at (8.52, 10.11, −20.73), barrels along (0.98, −0.12, 0.13); muzzle (66.56, 4.86, −20.54), screen (0.11, −0.81) |
| 7 | Scout, `sg_idle` frame 0, `c_scattergun` | `weapon_bone` (21.54, 7.68, −10.53); muzzle (44.29, 5.22, −4.28), screen (0.17, −0.25) |
| 8 | Scout, `p_idle`, `c_pistol` | muzzle (37.36, 5.06, −2.60), screen (0.20, −0.18) |
| 9 | Soldier, `dh_idle`, `c_rocketlauncher` | muzzle (24.23, 10.78, −5.28), screen (0.66, −0.57) |
| 10 | Engineer, `fj_idle`, `c_shotgun` | muzzle (52.32, 8.06, −6.38), screen (0.23, −0.32) |
| 11 | Pyro, `ft_idle`, `c_flamethrower` | muzzle (60.57, 10.80, −7.29), screen (0.26, −0.32) |
| 12 | Demoman, `g_idle`, `c_grenadelauncher` | muzzle (58.16, 4.78, −5.08), screen (0.12, −0.23) |
| 13 | Demoman, `sb_idle`, `c_stickybomb_launcher` | muzzle (52.60, 8.56, −5.67), screen (0.24, −0.28) |
| 14 | Sniper, `idle`, `c_sniperrifle` | muzzle (69.45, 8.98, −7.44), screen (0.19, −0.28) |
| 15 | Sniper, `smg_idle`, `c_smg` | muzzle (52.93, 9.60, −6.10), screen (0.27, −0.30) |
| 16 | Medic, `sg_idle`, `c_syringegun` | muzzle (55.73, 8.11, −6.50), screen (0.21, −0.31) |
| 17 | Spy, `idle`, `c_revolver` | muzzle (43.57, 6.08, −3.51), screen (0.21, −0.21) |
| 18 | view-model FOV, base FOV 90, not zoomed | 54 horizontal at 4:3; 41.828° vertical; 68.38° horizontal at 16:9 |
| 19 | the same, base FOV 75 | 54 (not 69) |
| 20 | standing still, then moving at 320 | no sway on turning; bob as specs/cs_source/view_models.md's bob tests |
| 21 | switch to the Minigun | `m_draw` at 1.34 x (over at 0.746 s), held to 1.0 s, then `m_idle` |
| 22 | hold attack 0.75 s | `m_spool_up` from 0 to cycle 0.75, then `m_fire` from 0 |
| 23 | release after firing | `m_spool_down`, then at +2.0 s `m_idle` blended in over 0.2 s |
| 24 | barrel, wind-up at t = 0, per tick | speed 0.1·n rad/s after n ticks, 20 rad/s after 200 ticks (3.0 s) |

## Open questions

1. A screenshot of the real game (Heavy, Minigun at rest, default
   settings, 16:9) would confirm tests 2 and 3 on screen. The decode
   matches the user's description (barrels up, only a glove).
2. `m_spool_idle`'s restart every tick: whether the client shows frame 0
   or a few frames is a matter of prediction timing; not visible.
3. The Engineer's Gunslinger hands, inspect offsets and the minimal view
   models are out of scope.
4. Captured on the live client (2026-11-19, a listen server on
   ctf_2fort, the own player at `viewmodel_fov` 54, every frame at 60
   fps), against ours: the bob while walking (the Soldier's Rocket
   Launcher) shows, with the hand rising into the lower edge twice a
   cycle as section 4 has it; the Scattergun's and the Rocket Launcher's
   draws are over by frame 16-20 (the 1.34 x rate); the Scattergun's
   fire, reload start, loop and end agree frame by frame. The Rocket
   Launcher's reload rises into its pose over about 0.25 s from its
   start where ours (the start phase 0.12 s, then `dh_reload_loop`,
   which snaps) jumped in: TF2 plays `dh_reload_start` whole (0.5 s,
   blending in over 0.2 s), then `dh_reload_loop` once per round
   (weapons.md 4, measured with the view model's sequence read every
   tick); ours now does the same. On a turn made of `setang` steps the view model moved a
   few pixels between frames, on and off, also after the turn had ended:
   read as the console's angles against the client's prediction, not a
   sway.
