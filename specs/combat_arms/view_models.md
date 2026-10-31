# Combat Arms: first-person view models

Source basis: reference material the user pointed the spec session to: the
client's first-person weapon logic (model creation, skins, animation
selection and the animation state machine, model-key handling, muzzle-flash
and shell-ejection placement), the client's bob and sway manager, the
player manager's camera, field-of-view and per-frame update order, the
player-view attachment manager, the client effect manager's placement
modes, the first-person shell-casing effect, the engine's camera-relative
("really close") render path, its model socket lookup and its client frame
loop, and the US client's attribute data (weapons and character-model
tables). View-model files were read with a throwaway reader built on this
repo's LithTech format reader (not in the repo): the pistol archive of the
Steam install (Steam build 25595979, unkeyed), and, because the Steam
rifle, knife, grenade and hand archives are V2-keyed, the plaintext asset
tree of the BR 1810.04 client (rifle, knife, grenades; its M92FS pair is
byte-for-byte the size of Steam's and gives the same numbers) and an older
client release's extracted hand archive and effect file. Each number says
which source it comes from. A follow-up session (Open questions 9–16)
added the client's footing test, deselect, piece and attached-effect
handling, the hand-file reset and the flash-hider choice.
Status: draft

Sections: Summary; Units and conventions; Constants; Behavior (1 which
models are drawn, 2 skins: sleeves, gloves and marks, 3 attachments and
attached effects, 4 placement in camera space, 5 projection and field of
view, 6 bob and sway, 7 animation selection per action, 8 animation key
events, 9 muzzle flash, 10 shell ejection, 11 visibility); Grenade timings
settled here (grenades.md Q3 and Q5); Per-frame order; Edge cases; Quirks;
Corrections (2026-10-22); Corrections (2026-10-30); Follow-up (2026-10-31b) (view-model lighting, the player-view point); Test cases; Open questions (including contradictions with other specs).

This spec builds on `animation.md` (tracker, sampling, key strings, the
done-needs-L+1 rule), `models_ltb.md` (sockets, nodes), `inventory.md`
(switching, draw, knife), `weapon_firing.md` (fire cycle, reload, recoil),
`weapon_classes.md` (zoom), `attachments.md` (part models) and
`grenades.md` (the throw). It does not repeat their rules; it says which
animation the view model shows and where the model sits.

---

## Summary

- The first-person weapon is one **camera-relative model**: the weapon's
  `PV_*` model, which carries a full arm skeleton (`Bip01 ...` nodes) plus
  the gun's own nodes and its `Flash` and `Breach` sockets. Its child
  `ANI_*` model only supplies the animations. The arms and hands are a
  separate skinned **hand model** chosen by the player's character, bound to
  the weapon model's nodes by name. There is no separate arms rig and no
  per-team sleeve: sleeves follow the character, its face (skin tone) and
  the equipped uniform camo.
- The model sits at a fixed per-weapon offset `Pos` in camera space and
  turns exactly with the camera. There is no turn lag, no landing dip and
  no camera head-bob. The only motion added by code is a tiny **sway**
  while walking or running: at most 0.010 units sideways and 0.015 units
  up and down, a figure-eight at 1.4 cycles per second running or 0.6
  walking or crouched. Everything else the player sees (recoil kick of the
  gun, reload, sprint pose) is the animation.
- It is drawn with its own projection: a **50° vertical field of view**, near
  0.1 and far 7.0 units, always in front of the world. The world camera uses
  75° vertical with the horizontal angle derived from the screen's aspect
  ratio (91.3° at 4:3, 107.5° at 16:9). Zooming changes only the world FOV;
  the scope hides the view model.
- Every action is an animation looked up by name once (`select`, `idle_0`,
  `fire_0`, `reload`, `PreRush`, ...). Draw time, fire cycle, reload time and
  sprint in and out are those animations' lengths. The knife's hits, the
  grenade's release and every reload sound are key strings inside them.
- The muzzle flash and shells are placed from **data offsets** (`Pos` +
  `MuzzlePos`, `BreachOffset`) in camera axes, not from the sockets; the
  stock flash effect then pins its particles to the view model's `Flash`
  socket. The grenade leaves the hand 133 ms into its post-fire animation,
  not in the 67 ms throw animation, and holding the button keeps it cocked.

## Units and conventions

- Distance: LithTech world units ("u"), the same unit as the other Combat
  Arms specs. Left-handed frame, +Y up.
- **Camera space**: origin at the camera position, +X right, +Y up, +Z
  forward, axes rotating with the camera. The view model's position and
  rotation are stored in this space; its rotation is always identity in it.
- **Model space** of the view model: the space its node matrices are in
  (`animation.md`, "Node evaluation order"), including the root translation
  of the playing animation's binding. Camera space = model space + the
  model's camera-space position (no rotation, scale 1).
- Angles: degrees for fields of view, radians in the bob formulas.
- Time: the bob uses seconds (the game's frame time, clamped to 0.1 s, and
  the client clock). Animations use integer milliseconds (`animation.md`).
  Everything here runs once per **rendered frame**; nothing is on the 15 ms
  movement tick except the inputs it reads.
- "Update n" or "u_n" is the n-th rendered frame's game update. "Advance"
  is the engine's per-frame animation step, which comes after the game
  update in the same frame (Per-frame order).
- Names in `code font` are data the game exposes: animation, node, socket
  and piece names stored in model files, attribute fields, console
  variables, effect group names.

## Constants

| Name (ours) | Value | Unit | Meaning |
|---|---|---|---|
| pv_fov_y | 50.0 | deg | View-model vertical FOV, console variable `PVModelFOV` |
| pv_aspect | screen width / height | | View-model aspect ratio (the console variable `PVModelAspect` is written but not used by the renderer) |
| pv_near, pv_far | 0.1, 7.0 | u | View-model clip planes, console variables `reallyclose_near`, `reallyclose_far` |
| pv_depth_range | 0.0 to 0.1 | of depth buffer | View model drawn in this slice, the world in 0.1 to 1.0: the view model is never hidden by world geometry |
| world_fov_y | 75.0 | deg | Console variable `FovY` |
| world_fov_x | from aspect | deg | $2\arctan(a \tan 37.5°)$, recomputed into `FovX` at world start (5.2) |
| pace_run | 1.4 | cycles/s | Console variable `BobSwayRunPaceAdjust` |
| pace_walk | 0.6 | cycles/s | Console variable `BobSwayWalkPaceAdjust` |
| bob_ramp | 0.1 | s | Console variable `BobDecayTime`: ramp-up and decay time of the sway amplitude |
| amp_max | 10.0 | | Console variable `MaxBobAmp` |
| sway_h | 0.002 | u per amp | Console variable `SwayH` (sideways) |
| sway_v | 0.0015 | u per amp | Console variable `SwayV` (vertical) |
| sway_scale | 1.0 | × | Console variable `WeaponSway` = profile option `WeaponSway` (0 to 10) / 10 |
| sway_h_halving | 0.5 | × | Fixed extra factor on the sideways sway |
| sway_h_phase | π/3 | rad | Phase lag of the sideways sway |
| move_threshold | 0.1 | u per frame | Speed × frame time must exceed this to count as moving |
| frame_time_cap | 0.1 | s | Game frame time clamp used by the sway phase |
| idle_wait | 5 to 15 | s | Uniform random wait before a full idle (`animation.md`) |
| rate_cap | 2.0 | × | Any animation rate above this plays at 1.0 |
| shell_life | 3.0 | s | First-person shell casing lifetime (fixed) |
| shell_bounces | 3 | | Bounces before a shell rests (`ShellMaxBounceCount`) |
| shell_vel_up | 30 to 75 | u/s | `ShellMinUpVelocity`, `ShellMaxUpVelocity` (camera up) |
| shell_vel_right | 50 to 75 | u/s | `ShellMinRightVelocity`, `ShellMaxRightVelocity` (camera right) |
| shell_vel_fwd | 20 to 50 | u/s | `ShellMinForwardVelocity`, `ShellMaxForwardVelocity` (camera forward) |
| shell_scale | ×1 to ×2 over 0.5 s | | `ShellMaxScale` 2.0, `ShellScaleTime` 0.5 |
| flash_time | effect length (0.05 s stock) | s | How long the first-person muzzle effect is shown per shot |

Unused in this build (present, read, but they change nothing in first
person): `BobV` 0.45, `HeadBob` 1.0, `BobRollAdjust` 1.0 and `Reality`
(the camera head-bob and roll are compiled out, only the weapon sway runs),
`PVModelAspect`.

Per-weapon placement data (US attribute data; units u, camera axes):

| Weapon | `Pos` | `MuzzlePos` | `BreachOffset` | `PVMuzzleFXName` |
|---|---|---|---|---|
| Beretta_M92FS | (−0.1, −0.2, 0.7) | (2.84, −2.4, 8.559921) | (0, 0, 0) | `AK47_pv_muzz` |
| M4A1 | (0.3, −0.6, 1.15) | (4.4, −3.55, 7.75) | (0, 0, 0) | `AK47_pv_muzz` |
| M16A3 | (0.3, −0.6, 1.15) | (4.4, −3.55, 7.75) | (1, 1, 1) | `AK47_pv_muzz` |
| AK-47 | (0.3, −0.6, 1.2) | (4.24, −2.6, 6.859921) | (0, 0, 0) | `AK47_pv_muzz` |
| Remington_870 | (0.3, −0.7, 1.0) | (5.44, −3.2, 9.559921) | (0, 0, 0) (absent) | `AK47_pv_muzz` |
| L96A1 | (0.4, −0.7, 1.5) | (2.84, −2.6, 4.859921) | (0, 0, 0) | `AK47_pv_muzz` |
| M60 | (0.4, −0.6, 1.5) | (3.85, −2.35, 6.5) | (0, 0, 0) (absent) | `AK47_pv_muzz` |
| M9 (knife) | (0, 0, 0) | (0, 0, 0) | (0, 0, 0) | none |
| M67 | (0.5, −0.9, 1.5) | (1.94, −2.18, 10.359921) | (0, 0, 0) | none |

(Values printed as stored, e.g. 2.839999 is shown as 2.84.) Every weapon
record in the US data has `Recoil` = (0, 0, 0). 28 records have
`BreachOffset` (1, 1, 1), the rest (0, 0, 0).

## Behavior

### 1. Which models are drawn

Three model files make up what the player sees. All three share one
skeleton layout for the arms.

1. **The weapon model** (`PV_*`): the file named by the weapon's
   `PVModelNormal`, or by the weapon's player-view entry for the
   character's `PVWeaponModel` identifier (every one of the 426 character
   models in the US data says `Normal`, so in practice `PVModelNormal`).
   It holds the gun's pieces (rigid or skinned), the whole arm skeleton
   (`Scene Root`, `Bip01`, `Bip01 Pelvis`, `Bip01 Spine`, `Bip01 Neck`, both
   clavicles, upper arms, forearms, hands and 15 finger nodes per hand)
   with the gun's nodes parented under the right hand, and the sockets
   `Flash` and `Breach` on a gun node (names compared without case; the
   M92FS spells them `flash` and `breach`). Its own animation list has one
   bind-pose animation. It is the object that is positioned, animated and
   drawn.
2. **The animation child** (`ANI_*`): child model 1 of the weapon model.
   Its animations are appended to the weapon model's list
   (`animation.md`). Its own pieces (it contains a copy of the arms,
   `arm_SH` and `Object01`, and of the gun) are **never drawn**.
3. **The hand model**: a skinned model from the `CHARS_M_HAND` archive,
   attached to the weapon model as an additional model. Its bones are
   matched to the weapon model's nodes **by name, case ignored**, once when
   it is attached and again whenever the skins are reset (2). It has no
   animation of its own on screen: each of its vertices is skinned with the
   weapon model's animated node matrices (`animation.md`, "Skinning"). The
   stock male hand `CHARS_M_HAND\Normal_M_Hand.ltb` has 43 nodes, exactly
   the arm nodes of the weapon skeleton, and two pieces: `arm_SH` (the
   sleeves) and `Object01` (the hands).

**Which hand model.** From the player's character:

- the model id is the cosmetic costume's model if one is equipped
  (cosmetic id 0 means "default character with the default face"), else the
  equipped character model;
- the hand index is the character model's `HandIndex`, looked up on the
  model that the model id and the face piece map to (its face-relation
  model); an equipped "overridable model" item's own `HandIndex` wins when
  it has one;
- the hand model file is that hand record's `ModelFile` (`[HandN]` records
  in the character-model table, 124 in the US data). A hand index of −1
  gives no hand model at all (arms invisible; the US code path).

When the player is dead or spectating in first person, the character of the
spectated player is used for the model id; the face and uniform still come
from the local player's own profile (Quirk 5).

The weapon model is created when the weapon is drawn and destroyed when it
is put away (holster is instant, `inventory.md`), so every draw starts
from a fresh model whose current animation is its bind pose, not playing.
The model always uses level of detail 0 (`models_ltb.md`).

### 2. Skins: sleeves, gloves and marks

Each model carries an ordered list of skin (texture) names and render
styles; piece texture index k uses skin k.

- **Weapon model**: skins = the weapon's `PVSkin0..`, render styles =
  `PVRenderStyle0..` (replaced by the player-view entry's lists when it has
  non-empty ones). In the stock data skin 0 is the placeholder `Hands` and
  skins 10 and 11 the tags `PV_L_ARM`, `PV_R_ARM`; the weapon's own pieces
  use skin 1 and up.
- **Hand model**: start from the hand record's colour-variant skin list for
  the character's model colour if that list is non-empty, else its `Skin0..`
  list, and its `RenderStyle0..` list. Then, if the player's character
  object exists, **skin 0 is replaced by the character's sleeve texture**:
  - entry f of the character model's `HandsSkin0..` list (or of its
    colour-variant list for the model colour, if non-empty), where f is the
    face piece id after the face's `RedirectForHandsSkin` redirect;
  - that entry is a pattern with one two-digit number, filled with the
    equipped **uniform (camo) item index**. Default character, face 0,
    uniform 0: `CHARS_T_HAND\HAND_00_00.dtx`; face 2, uniform 3:
    `CHARS_T_HAND\HAND_02_03.dtx`;
  - if the model has fewer `HandsSkin` entries than f + 1, the hand
    record's first skin is used instead.

  Any hand skin named `PV_L_ARM` or `PV_R_ARM` (tattoo slots) is replaced by
  `TEXTURES\8xNone.dtx`, so no tattoo shows.
- **Clan mark**: when the local player has a clan emblem and background,
  skins 7 and 8 of the hand model (and of the weapon model when it uses a
  player-view entry) become the clan emblem and background textures.
- Nothing depends on the team. Sleeves change with the character, the face
  (skin tone) and the uniform item. Every draw builds a fresh weapon model
  (1), whose file names are worked out from the current character, face and
  uniform, so a change shows on the next draw at the latest. Separately, the
  weapon in hand has its **file names reset** (weapon skins, hand model and
  skins, the player-view attachments, bone matching, and the draw marked to
  be refreshed, Edge cases) at exactly one moment: when the local player's
  own character effect is created on the client, i.e. when the client first
  receives its player object at world entry (the first weapon is often
  drawn before that, which is why its hands start without the sleeve
  texture). No other event resets it: a character or uniform change while a
  weapon is out keeps the old hands until the next draw (Open question 13).
- **Colour variants of a hand record**: `ColorSkin{c}_{k}` replaces the
  record's whole skin list (`Skin0..`) when the character's model colour
  index c selects a non-empty list. In the US data only two hand records
  have them, both with c = 1 and 2 and only skin 5 different from the plain
  list: `GhillieSuit_M_Hand` (`Hand8`) and `GhillieSuit_M_Zombie_Hand`
  (`Hand21`): c = 1 → skin 5 `CHARS_T_BODY\CM_Fur_Ghillie_Suit_DE_SD.DTX`,
  c = 2 → `CHARS_T_BODY\CM_Fur_Ghillie_Suit_UR_SD.DTX`; skin 0 stays `Hands`
  (replaced by the sleeve texture as above). The stock records start
  `Hand0` `Normal_M_Hand` (skin 0 `Hands`), `Hand1` `Normal_W_Hand` (skin 0
  `CHARS_T_HAND\Hand_W.DTX`), `Hand2` `Special_M_Hand`, `Hand3`
  `Special_W_Hand`; skins 1–9 are blanks, a `clouds` render-style texture and
  a specular map, skins 10 and 11 the tattoo tags. The stock bodies all use
  `HandIndex` 0; the female bodies carry four extra `HandsSkin` entries
  (`CHARS_T_HAND\HAND_W_00_%02d.dtx` … `HAND_W_03_%02d.dtx` at entries 4–7),
  which the female faces 4–7 select, so a female player gets the
  `Normal_M_Hand` model with women's sleeve textures.

### 3. Attachments and attached effects

- **Weapon parts** (scope, silencer, ...) are `attachments.md`'s: each
  fitted part with a socket is its own camera-relative model, placed every
  frame at that socket of the weapon model plus the weapon's
  `MuzzlePVPosOff` / `ScopePVPosOff` (socket axes) and rotation offsets,
  **after** the weapon model's position for the frame is set (Per-frame
  order). Parts hide and show with the weapon.
- **Player-view attachments** (character-dependent models at sockets of the
  weapon model) are supported but no character model in the US data
  defines any.
- **Attached effects** (Open question 12): a
  weapon's `PVAttachFXName0..` effects (chainsaw idle, PSY bomb fire, time
  bomb lights; the XK-2 bomb has five: `TimebombGreen`, `TimebombRed`,
  `TimebombYellow1..3`) are all created, **hidden**, when the weapon model
  is created, each as a looping, camera-relative effect parented to the
  weapon model. Placement is each key's own placement mode (9): a key
  pinned to a socket or node of the weapon model follows that socket's
  animated position in camera space plus its offset in the socket's axes;
  a "player view" key is placed by the weapon code; a fixed key stays where
  it was created. Showing:
  - when the weapon becomes visible (draw, end of scope), every attached
    effect that was not hidden by a key is shown; with `PVAttachFXAlwaysShow`
    1 all of them are;
  - a `HIDE_PVATTACHFX i j …` key hides effects i, j … (indices from 0) and
    remembers that; `SHOW_PVATTACHFX i …` shows them again; both keys are
    ignored while the weapon is invisible, and the hide key is ignored when
    `PVAttachFXAlwaysShow` is 1;
  - when the weapon is hidden, each effect remembers whether it was
    showing, and all are hidden; they are destroyed with the model.
  - **While hidden** an attached group is frozen: its clock stops, its
    sound keys' sounds are stopped at once, and when it is shown again
    those sounds start from the beginning of their files. A looping sound
    key that spans the whole group (the chainsaw idle) is started once
    when the group is shown and then loops by itself; it is not restarted
    at each pass (`effects_sounds.md` Corrections (2026-10-30) 1, its test
    cases 67–70).
- **Hidden pieces**: the weapon's `HiddenPiece0..` names are pieces of the
  weapon model hidden while the weapon's **total** ammunition is below 1
  (checked when the model is created and at the end of each fire sequence;
  a forced show skips the check). Missing piece names are skipped.
- **Projectile piece**: `ProjectilePiece` names one piece (for example
  a rocket launcher's visible rocket). It is **hidden** on every
  shot (right after the fire sound), **shown** when a reload starts (if
  ammo and a clip size remain) and again when the reload finishes, and at
  draw it is shown if the clip is not empty or the total equals the ammo's
  maximum, otherwise hidden.
- **Player-view attachments** (models from the character record's
  player-view attachment list, each placed on a named socket of the weapon
  model): created whenever the weapon's file names are reset (2) and on
  creation, drawn camera-relative with the weapon, hidden and shown with it
  and by the `HIDE_PVATTACHMENT` / `SHOW_PVATTACHMENT` keys. No character
  record in the US data defines any, so in stock play there are none.

### 4. Placement in camera space

Each frame, during the weapon update (Per-frame order), the weapon model's
camera-space position is set to

```math
\vec p_{vm} = \texttt{Pos} + (w,\ h,\ 0) + j\,(1, 1, 1)
```

- $(w, h)$ is the sway of 6, as last computed (it is frozen while it is not
  updated, see 6).
- $j$ is the **shot jiggle**: 0 on frames without a shot. On the frame a
  shot is fired, three uniform draws $r_x \in [-R_x, R_x]$,
  $r_y \in [-R_y, R_y]$, $r_z \in [-R_z, R_z]$ are made from the weapon's
  `Recoil` vector $R$ and

```math
j = r_x + r_y + r_z
```

  is added to **all three** axes (Quirk 2). It lasts that one frame. With
  the stock data ($R = 0$) it is always 0.
- The rotation in camera space is identity: the model's axes are the
  camera's axes, so it turns, pitches and recoils exactly with the camera
  with no lag. Camera shake and the backward fire push (`weapon_firing.md`,
  back push; the camera moves back along its forward axis by 10 × the
  back-push offset) move the world, not the view model on screen.
- Crouching lowers the camera (`movement.md`, eye height); the view model
  moves with it and does not change on screen.
- The model's node matrices are its animation pose in model space
  (`animation.md`), so the on-screen weapon is $\vec p_{vm}$ + pose.

**World-space flash point.** In the same update the game computes a world
point that stands for the muzzle:

```math
\vec o = \texttt{Pos} + \texttt{MuzzlePos} + (w,\ h,\ 0) + (r_x, r_y, r_z), \qquad
\vec F = \vec c + o_x \hat r + o_y \hat u + o_z \hat f
```

with $\vec c$ the camera position and $\hat r, \hat u, \hat f$ its right, up
and forward axes. Note that here the jiggle is added **per axis**, unlike
the model position. $\vec F$ is where weapon sounds sent to other players
are said to come from, where world-space parts of the muzzle effect are put
(9) and where shells start (10). It is **not** derived from the `Flash`
socket and does not move with the animation. For the M92FS, $\vec o$ =
(2.74, −2.6, 9.259921): 9.26 u ahead, beyond the view model's far plane,
chosen so that through the world's projection it lands near where the
drawn muzzle appears.

### 5. Projection and field of view

#### 5.1 View model

The view model, its parts and every effect flagged camera-relative are
drawn after the world with their own projection:

- view = identity (the objects are already in camera space);
- projection = left-handed perspective with vertical FOV `PVModelFOV`
  (50°), aspect = screen width / height, near 0.1, far 7.0;
- depth range of the viewport set to [0, 0.1] while they are drawn (the
  world uses [0.1, 1]).

So the view model's horizontal FOV is

```math
\phi_{pv,x} = 2\arctan\!\left(a\,\tan 25°\right)
```

63.74° at 4:3, 73.45° at 16:10, 79.32° at 16:9. A point $(x, y, z)$ in camera
space lands at normalised screen coordinates

```math
s_x = \frac{x}{a\,z\,\tan 25°}, \qquad s_y = \frac{y}{z\,\tan 25°}
```

($s$ in −1..1, +y up). Geometry beyond z = 7.0 u is clipped away.

The console variable `DrawGuns` 0 hides the view model (renderer cvar,
default 1).

#### 5.2 World

- The world camera's vertical FOV is `FovY` (75°). When the camera is reset
  (at client start and at each world start) the game **recomputes** the
  horizontal value from the screen size and stores it in `FovX`:

```math
\phi_x = 2\arctan\!\left(a\,\tan\frac{\texttt{FovY}}{2}\right)
```

  91.31° at 4:3, 87.61° at 5:4, 101.67° at 16:10, 107.51° at 16:9. The
  built-in 90 of `FovX` is overwritten (Open question C5).
- Zoom narrows only the world FOV (`weapon_classes.md`, with its `FovX` =
  this recomputed value). It never changes the view-model projection, and
  the view model is hidden while a scope is up (11).
- **Other aspect ratios** (Open question 14): the world is "vertical
  fixed": `FovY` stays 75° and the horizontal angle grows with the aspect
  ratio (the formula above, from the camera rectangle's pixel size at the
  reset). The value is recomputed only when the camera is reset (client
  start, world start); a window or resolution change between resets keeps
  the old `FovX`, so the picture is stretched horizontally until the next
  world start. The view model is also vertical-fixed (50°) but takes its
  aspect from the current screen at every draw, so it never stretches.
  The near and far planes 0.1 / 7.0 and the [0, 0.1] depth slice are the
  renderer's defaults (`reallyclose_near`, `reallyclose_far`) and are used
  for every camera-relative object, view model and camera-relative effects
  alike.

### 6. Bob and sway

Only a weapon sway exists; the camera does not bob or roll. The sway is
computed once per frame in the player update, before the camera and the
weapon update (Per-frame order).

**When it runs.** Only if all hold: not spectating in free mode, first
person, the player "can do footsteps" (on the ground, on a ladder, or a ray
150 u straight down hits something, and not standing on sky; false while
airborne after a jump), player movement allowed and not frozen; **or** the
player is spectating someone in first person. When it does not run, every
value below keeps its last value, including the offsets $(w, h)$
(Quirk 3).

**The footing test** (Open question 15), evaluated once per movement tick
before the motion step, in this order: standing on sky → false (and the
rest is skipped); on a ladder → true (even in water); on the ground → true
(even on the tick of a jump); jumped (and not yet landed) → false;
otherwise a ray from the hull centre straight down 150 u (to 100 u below
the standing feet), against solid world geometry and solid objects other
than the player's own, invisible polygons ignored: a hit → true, no hit →
false and the standing surface becomes "unknown". What it changes:

- the sway above runs only while it is true;
- own first-person footsteps (`effects_sounds.md`) need it;
- the **start of a fall**: on a tick where it is false, the player did not
  jump, is not already falling and is not in a free-movement volume, on a
  ladder or spectating, the client marks itself falling and tells the
  server (the third-person driver's "falling" motion status,
  `characters.md` 5). So walking off a ledge with ground less than 100 u
  below the feet never counts as a fall, and the run animation and the sway
  continue through the drop.

**Spectating in first person** (Open question 15): the spectator's own sway
state runs with the **spectated player's** control flags as last received
(its run flag picks the 1.4 or 0.6 pace and its movement keys the "moving"
test), unless that player is dead, in which case the spectator's own flags
are used. The distance part of the moving test is replaced by a fixed
3.5 u per frame, so it always passes: the spectated gun sways whenever that
player holds a movement key, at full amplitude after the 0.1 s ramp,
whatever their real speed (standing against a wall with forward held still
sways). The spectated player's own zoom is not read; the spectator's
`WeaponSway` option scales it.

**Inputs**, per frame: frame time $\Delta t$ (the game's, clamped to 0.1 s),
client clock $T$ (s), the control flags (run flag, moving flag) and the
player's velocity $\vec v$. When spectating in first person, the spectated
player's flags are used and the distance test below always passes.

- Pace: $P = 2\pi \cdot$ `BobSwayRunPaceAdjust` (1.4) if the run flag is
  set, else $2\pi \cdot$ `BobSwayWalkPaceAdjust` (0.6), in rad/s. The run flag
  is on by default and off while the walk key is held or while crouched
  (`movement.md`), so walking and crouching use 0.6 and running and
  sprinting 1.4. Speed itself does not scale the sway.
- Moving: $|\vec v|\,\Delta t > 0.1$ u **and** the moving flag (a movement key
  or jump held) is set. The distance test makes the speed needed depend on
  the frame rate: 6 u/s at 60 fps, 30 u/s at 300 fps (Quirk 4).

**Steps**, in this order:

1. Phase: if $A = 0$ (the amplitude from the previous frame), $\varphi \leftarrow 0$;
   else $\varphi \leftarrow \varphi + P\,\Delta t$ and, if $\varphi > 2\pi$,
   $\varphi \leftarrow \varphi - 2\pi$.
2. First-person footstep sounds are triggered from the same phase (one
   when $\varphi$ enters $(\pi/2, 3\pi/2]$, one when it passes $3\pi/2$, only
   while moving); their sound belongs to a sound spec.
3. Amplitude, with $\tau$ = `BobDecayTime` (0.1 s), $A_{max}$ = `MaxBobAmp`
   (10) and the two shaping curves

```math
S(x) = \frac{1 - \cos \pi x}{2}, \qquad
\mathrm{on}(x) = \begin{cases} S(x) & x < 0.5 \\ x & x \ge 0.5 \end{cases}, \qquad
\mathrm{off}(x) = \begin{cases} x & x < 0.5 \\ S(x) & x \ge 0.5 \end{cases}
```

   - Moving: on the first moving frame, $T_s \leftarrow T - \tau A/A_{max}$
     (and the decay start is cleared). Then with $d = T - T_s$:
     $A = A_{max}\,\mathrm{on}(d/\tau)$ if $d \le \tau$, else $A = A_{max}$
     (clamped to $A_{max}$).
   - Not moving: on the first such frame, $T_d \leftarrow T - \tau\,(1 - A/A_{max})$
     (and the ramp start is cleared). Then with $d = T - T_d$:
     $A = A_{max}\,(1 - \mathrm{off}(d/\tau))$ if $d \le \tau$, else $A = 0$
     (clamped at 0).

   The start times are back-dated linearly from the current amplitude, but
   the curve is not linear, so reversing direction mid-ramp makes the
   amplitude jump (Quirk 4).
4. Offsets, only if a weapon is in hand and the player is **not zoomed**
   (while zoomed they stay frozen; the model is hidden then anyway), with
   $k$ = `WeaponSway`:

```math
h = \texttt{SwayV}\cdot A \cdot \sin(2\varphi)\cdot k, \qquad
w = \tfrac12\,\texttt{SwayH}\cdot A \cdot \sin\!\left(\varphi - \tfrac{\pi}{3}\right)\cdot k
```

   At full amplitude and $k = 1$: $h = 0.015 \sin 2\varphi$ u,
   $w = 0.01 \sin(\varphi - \pi/3)$ u, a figure-eight; the vertical part has
   twice the frequency of the sideways part. Running, one sideways cycle
   takes 0.714 s and one vertical cycle 0.357 s; walking or crouched 1.667 s
   and 0.833 s.

`WeaponSway` is set from the profile option `WeaponSway` (an integer 0 to
10) divided by 10 whenever profile settings are applied; a profile without
the key reads 0, which turns the sway off. The Steam install's profile on
the measuring machine had 10.

**What does not move the model**: turning (no lag or sway from mouse
movement), jumping and landing (the sway just freezes in the air and
resumes on the ground), crouching, stance changes, firing (apart from the
data jiggle and the animation), zoom (frozen, hidden).

### 7. Animation selection per action

All animations are found once per draw by name in the weapon model's
combined list, first match, case ignored (`animation.md`). A name that is
missing leaves that step out. Every animation plays on the model's main
tracker, non-looping unless stated, at rate 1.0 unless stated; the fire,
reload and draw rates ($\rho_{fire}$, $\rho_{reload}$, $\rho_{draw}$) are the
other specs', and any rate above 2.0 plays at 1.0. "Reset" means the
animation restarts from 0 with the previous one as the blend source;
"non-reset" means a switch (one-frame artefact, Quirk 1). Lengths are the
stock files' (S = Steam 25595979, B = BR 1810.04).

| Action | Animation (name looked up) | Started | Rate | Length (stock) | Sets or must match |
|---|---|---|---|---|---|
| Draw | `select` | at the switch, reset | $\rho_{draw}$ | M92FS 650 (S), M4A1/M16A3 1200 (B), M9 450 (B), M67/XM84/M18 400 (B) | inventory.md draw time; fire is accepted on the update that sees it done |
| Holster | `deselect` | only if the file has it (none in any stock file): on a weapon switch the old weapon enters its deselect state and plays `deselect` non-reset at rate 1.0 (no rate bonus), the switch waits for "done" and the new weapon is drawn on the next update; without the animation the switch is instant | 1.0 | none in any stock file | inventory.md: holster time 0 (correct for every stock weapon) |
| Subtle idle | `idle_0` | when idle and nothing else is due; restarted (reset) on the update after it reports done | 1.0 | M92FS 1566, M4A1 1833, M9 2000, M67 933 | — |
| Full idle | `idle_1` (random among `idle_1` and further numbered idles; only `idle_1` exists) | when the idle wait has passed, the current animation is done and the player is not moving; non-reset | 1.0 | same lengths as `idle_0` in stock files | idle wait 5–15 s, rescheduled each time the weapon enters idle and each time the wait is checked |
| Pre-fire | `prefire` | from idle, the first update with a fire request only sets the state to firing; the next update starts it, reset; **skipped for secondary (pistol) and melee classes and for primary shotguns** | $\rho_{fire}$ | M4A1 0 (6 keys), M92FS 0 (skipped), M9 1 (skipped), M67 700 | weapon_firing.md fire cycle; grenades.md pin pull |
| Fire, primary | `fire_0` | after `prefire` is done, or directly; restarted (reset) on the update after it is done while the request holds and total ammo > 0 | $\rho_{fire}$ | M4A1 105, M92FS 180, M9 650, M67 67 | weapon_firing.md cycle = length / rate; its `FIRE_KEY` raises the shot |
| Fire, alternate | `alt_fire_0` | as `fire_0`, when the last fire type is alternate (knife stab, dual pistols' left gun) | $\rho_{fire}$ | M9 1100 | inventory.md knife stab |
| Post-fire | `postfire` | when the request ends after a fire animation, reset | $\rho_{fire}$ | M92FS 150, M4A1 0, M9 1, M67 366 | weapon_firing.md, grenades.md |
| Last round | none of its own | the clip-0 gate ends the request, so the last `fire_0` runs out, `postfire` plays, then the reload starts | | | weapon_firing.md empty clip |
| Dry fire | `dryFire` (matches `dryfire`) | only while the state is "firing with no ammo", which normal play never reaches (C6, confirmed): that state needs a fire request with a **total** of 0, but the total includes the clip and the request is cleared whenever the clip is 0. No dry-fire sound plays either: the direct dry-fire sound of the fire code is disabled, and `DryFireSnd` could only come from a sound key in `dryFire`, which never runs | 1.0 | M92FS 300, M4A1 0 | — |
| Reload, magazine | `reload`; `reload_1` instead when the weapon's gun type is the double barrel, its clip is above 0 and the file has `reload_1` (the "one barrel left" reload) | the first weapon update after the request (the same frame for the reload key, the next for the automatic reload), once the whole fire sequence including `postfire` is over; non-reset | $\rho_{reload}$ | M92FS 1500 (S), M4A1 2300 (B), M67 400 (B) | weapon_firing.md reload time; clip filled on the update that sees it done |
| Reload, empty | same `reload` | no separate empty reload | | | |
| Reload, shell by shell | `PreReload`, then `reload` per shell, then `PostReload`; all non-reset | | $\rho_{reload}$ | weapon_classes.md 5.5 | weapon_classes.md shell timing |
| Sprint start | `PreRush` | first update with sprint on (after a draw, reload or fire animation in progress finishes), reset; restarted from 0 if sprint resumes during `PostRush` | 1.0 | M92FS 250, M4A1 233, M9 167, M67 167 | movement.md / inventory.md sprint-in |
| Sprint loop | `rush`, **looping** | on the update that sees `PreRush` done, reset | 1.0 | M92FS 1500, M4A1 620, M9 666, M67 666 | — |
| Sprint end | `PostRush` | first update with sprint off while a sprint animation shows (cuts `PreRush` or `rush` at once), reset; then a **full** idle (`idle_1`), not `idle_0` | 1.0 | M92FS 400, M4A1 367, M9 167, M67 267 | inventory.md: fire ignored until it ends |
| Knife swings | `fire_0` (slash), `alt_fire_0` (stab) | as fire, held button repeats with no gap; no pre-fire (melee class) | $\rho_{fire}$ | 650 / 1100 (B) | inventory.md hit times 155 and 402 / 367 ms |
| Grenade pin pull | `prefire` | as pre-fire | $\rho_{fire}$ | 700 (B) | grenades.md |
| Grenade cocked | `fire_0` | after `prefire`; **loops while the button is held** | $\rho_{fire}$ | 67 (B) | grenades.md (C3) |
| Grenade throw | `postfire` | after the button is up and the current `fire_0` is done | $\rho_{fire}$ | 366 (B), release key at 133 | grenades.md release (Q3) |
| Grenade re-ready | `reload` | when `postfire` is done and total > 0 (automatic reload) | $\rho_{reload}$ | 400 (B), same keys as `select` | grenades.md Q5 |
| Inspect | none | no inspect animation is looked up in this build | | | |
| Scope in / out | none | no animation; the model is hidden on the frame the first zoom level is entered and shown on the frame the zoom-out to level 0 ends | | | weapon_classes.md 7.2 |

Names present in some files but never looked up: `fire_1`, `alt_prefire`,
`alt_postfire`, `alt_fire_1`, `alt_dryfire` (the M9 has all of them),
`prereload`/`reload`/`postreload` of 1 ms in the knife file (never
requested, knives have infinite ammo).

Selection rules beyond the table:

- **Semi-automatic guns** (`RapidFireCount` > 0): a new press during
  `postfire` cancels it and starts `fire_0` at once (the gun has no
  pre-fire, or its pre-fire is skipped). Fully automatic guns
  (`RapidFireCount` 0) wait for `postfire` to finish. So the M92FS can fire
  every 180 ms plus frame rounding, not every 330 ms (Quirk 6).
- **Reload after firing**: a reload request during a fire sequence waits;
  if the main fire animation has ended and a `postfire` exists, the
  `postfire` is started first and the reload follows it.
- **Fire during a reload**: magazine reloads ignore it until done
  (`weapon_firing.md`); shell reloads stop at once (`weapon_classes.md`).
- **Sprint**, each update while the sprint state is on:
  - reloading: the reload keeps playing; when it is done the clip is
    filled and `PreRush` starts;
  - drawing: the draw keeps playing; when done, `PreRush`;
  - a fire animation that is not done keeps playing (its keys still count,
    the state stays "firing"); when done: if it was `prefire` and a
    `fire_0` exists, `fire_0` starts even though sprinting; otherwise
    `PreRush`;
  - **throw-class weapons** (grenades, mines): while any fire animation of
    theirs is current, done or not, nothing changes: the stage plays to its
    end and holds its last pose; the next stage starts only after sprint
    ends;
  - otherwise the sprint animations as in the table. Fire requests are
    ignored.
- **Firing during the draw**: ignored until the update that sees `select`
  done; with the button held the state becomes "firing" on that update and
  the fire animation starts on the next.

### 8. Animation key events

Keys fire during the engine's advance of the view model (after the frame's
game update) and are handled at once; the weapon logic consumes their
effect on the next frame's update (`animation.md`, "First-person weapon
keys"). Keywords used by the stock view models (census over the BR
client's view-model files): `sound_key` 1,789 times, `FIRE_KEY` 718,
`ALT_FIRE_KEY` 133, `dryfire_key` 29 (matches nothing, no effect),
`THROW_KEY` 20, `perk_fire_key` 2 (matches nothing in the first-person
handler). No stock view model uses `SHELL_CASING`, `FX`, `FIREFX_KEY`,
`HIDE_PIECE_KEY` or the attachment keys, so in stock play shells and
flashes come from the shot itself (9, 10), not from keys.

- `FIRE_KEY` / `ALT_FIRE_KEY`: raise a shot (or knife hit) only if the
  current animation is part of the fire sequence (`prefire`, `fire_0`,
  `alt_fire_0`, `postfire`) and the state is "firing".

**What times the gameplay** (Open question 9, settles C1). On the shooter's
client the view model **is** the clock: a shot, a knife hit and a grenade
release happen only when one of these keys fires in the advance and the
next weapon update consumes it; the clip is refilled on the update that
sees `reload` done; the draw ends on the update that sees `select` done.
There is no separate client timer for any of them (the weapon's
`FireDelay` is not read by the client at all; only the triple-shot mode has
a timer of its own). The other specs' times are these animations' lengths
divided by the rates of 7. Consequences:

- **Full-automatic fire**: while the request holds, `fire_0` is restarted
  (reset) on the first update that sees it done, so one shot per
  ⌈(L + 1) / Δ⌉ frames of Δ ms (L the rate-scaled length, the done-needs-L+1
  rule of `animation.md`); for the M4A1 at 60 fps, 105 ms → 7 frames =
  116.7 ms per round. The server only refuses shots closer together than the
  weapon's `FireDelay` (default 30 ms), which never binds at that cadence.
- **Knife**: the hits are the `FIRE_KEY`s of `fire_0` (155 and 402 ms) and
  the `ALT_FIRE_KEY` of `alt_fire_0` (367 ms), at the fire rate; with no
  pre-fire (C1).
- **Reload asked during the fire sequence**: the request waits; if the main
  fire animation has ended and the weapon has a `postfire`, `postfire` plays
  first; `reload` starts (non-reset) on the first update after the whole
  sequence is over, and the clip is filled on the update that sees `reload`
  done.
- `THROW_KEY`: sends the server a throw notice (the third-person side of the
  throw, `grenades.md`); it does not release the grenade.
- `sound_key n`: weapon sound n played locally and relayed to other players
  from the flash point $\vec F$; ids per `animation.md`.
- `SHELL_CASING` (unused in stock files): would spawn a shell at the
  breach point (10) if the model is visible and the view is first person.
- Entering `prefire` or `postfire` also plays the weapon's `PreFireSnd` /
  `PostFireSnd` attribute sound locally, if it has one (spinning guns).

Stock key times (ms into the animation, at rate 1):

| File | Animation | Keys with strings |
|---|---|---|
| M92FS (S) | `select` | 296 `sound_key 3` |
| M92FS (S) | `fire_0` | 0 `FIRE_KEY` |
| M92FS (S) | `reload` | 226 `sound_key 2`, 453 `sound_key 1`, 962 `sound_key 3` |
| M4A1, M16A3 (B) | `select` | 666 `sound_key 3` |
| M4A1, M16A3 (B) | `fire_0` | 0 `FIRE_KEY` |
| M4A1, M16A3 (B) | `reload` | 403 `sound_key 1`, 834 `sound_key 2`, 1725 `sound_key 3` |
| M9 (B) | `select` | 32 `sound_key 4` |
| M9 (B) | `fire_0` | 155 `FIRE_KEY`, 402 `FIRE_KEY` |
| M9 (B) | `alt_fire_0` | 367 `ALT_FIRE_KEY` |
| M67 (B) | `prefire` | 513 `sound_key 10` (misc sound 1, the pin) |
| XM84 (B) | `prefire` | 497 `sound_key 10` |
| M18 (B) | `prefire` | 507 `sound_key 10` |
| M67, XM84, M18 (B) | `fire_0` | 0 `THROW_KEY` |
| M67, XM84, M18 (B) | `postfire` | 133 `FIRE_KEY` |

A key at time k of an animation started in the weapon update of frame n
fires in the m-th advance from then on (frames n … n + m − 1), with m the
smallest count whose accumulated rate-scaled time passes k,

```math
m = \min\{\, m \ge 1 : \textstyle\sum_{i=0}^{m-1} \Delta'_{n+i} > k \,\}
```

(a key at 0 fires in the first advance), and a shot or hit it raises is
consumed in the weapon update of frame n + m.

### 9. Muzzle flash

- **Which effect**: if the fitted muzzle part has a flash-hider level h in
  1..9, the weapon's `PVFlashHiderFXName`h; else, if a muzzle part is fitted
  and names a `PVMuzzlefxname`, that; else the weapon's `PVMuzzleFXName`
  (`attachments.md`). Empty name: no first-person flash.
- **The flash hider** (Open question 16) changes only **which group** is
  used, nothing else: not the flash point, the show time, the light rule
  or the shot itself. Level h comes from the muzzle part's `FlashHiderLevel`
  (default 0); the group from the weapon's `PVFlashHiderFXName`h (and
  `HHFlashHiderFXName`h for the third-person flash others see). A level
  whose weapon key is missing gives an **empty** name, which means no
  first-person flash at all. The choice is made when the effect is created
  (at the draw, 1); when spectating in first person it uses the spectated
  player's muzzle part. In the US data no part has a `FlashHiderLevel` and
  no weapon has a flash-hider group, so it changes nothing in stock play.
- The effect is created once when the weapon model is created, as a
  looping, camera-relative effect parented to the weapon model, and starts
  hidden. A shot records the time of the shot.
- Each weapon update, after the model is placed: if the weapon model is
  invisible, the effect is hidden. Otherwise it is **shown** while
  $T < T_{shot} + D$ ($D$ the effect's length) and the player is alive (or
  spectating in first person) and the camera is not in a liquid volume
  (unless the weapon's `UseUWMuzzleFX` allows it); else hidden.
- While shown, the effect receives two positions: the world flash point
  $\vec F$ and the camera-space offset $\vec o$ (4). Each element of the
  effect group is placed by its own placement mode, stored in the effect
  file: camera-relative elements use $\vec o$, world elements use $\vec F$,
  "player-view socket" elements follow the named socket of the weapon model
  (its animated position in camera space plus the element's offset),
  "player-view node" elements a node likewise. The effect system itself is
  the effects spec's.
- **Stock group** `AK47_PV_Muzz` (used by the M92FS, M4A1 and most guns),
  as read from an older client release's effect file (Open question 3): a
  particle element pinned to the weapon model's `Flash` socket with offset
  (0, 0, 0.5), and a dynamic light at the player-view position, both from
  0 to 0.05 s. So the visible flash follows the animated muzzle (it kicks
  with `fire_0`), and the light sits at $\vec F$. The flash is shown for
  50 ms after each shot (a whole number of frames in practice).
- A dual pistol's left gun has its own flash, flash point (`LeftMuzzlePos`
  side of the data) and timer, used when the shot came from `ALT_FIRE_KEY`.

### 10. Shell ejection

- First-person shells come from the shot's weapon effect when the ammo's
  fire effect has the shell flag and the console variable `ShellCasings` is
  1. The start point is the breach point

```math
\vec B = \vec F + b_x \hat r + b_y \hat u + b_z \hat f, \qquad \vec b = \texttt{BreachOffset}
```

  with $\vec F$ the flash point as last computed. It is not the `Breach`
  socket (that socket is looked up but only used for the third-person
  weapon). With `BreachOffset` (0, 0, 0), shells start at the flash point.
- Start velocity, in camera axes: up U(30, 75), right U(50, 75) (left for a
  dual pistol's left gun), forward U(20, 50) u/s; the ammo's fire effect
  velocity replaces it when non-zero. In first person the player's own
  velocity is **not** added at the start; instead, each frame the shell is
  also moved by the player's current velocity × frame time, so shells keep
  pace with a moving player.
- It lives 3.0 s, scales from the ammo's shell scale to twice that over
  0.5 s, spins, bounces up to 3 times and then rests; hitting sky or an
  invisible surface removes it. The rest of its physics and sound belongs
  to an effects spec.
- Shells still eject while the view model is hidden by a scope (the shell
  path does not check visibility).

### 11. Visibility

- The view model is shown in first person while alive with a weapon in
  hand. It is hidden on the frame zoom goes from level 0 to level 1 (scope
  overlay) and shown on the frame the zoom-out to level 0 ends; it is
  hidden while dead, in external or cinematic cameras, and while weapons
  are disabled.
- Hiding the weapon hides its flash, its parts, its attached effects and
  its player-view attachments. A hidden weapon still runs its state
  machine and animations, and its keys still fire.

## Grenade timings settled here (grenades.md Q3 and Q5)

These answer `grenades.md` Open questions 3 and 5 from the stock M67, XM84
and M18 view models of the BR 1810.04 client (the Steam throwing archive is
V2-keyed; Open question 1). `grenades.md` itself is not changed by this
spec.

**Q3, release time.** The three grenades have identical timings: `select`
400, `prefire` 700 (pin sound at 513 / 497 / 507), `fire_0` 67 with only
`THROW_KEY` at 0, `postfire` 366 with `FIRE_KEY` at **133**, `reload` 400.
The 67 ms "throw" animation carries no fire key. The grenade is released
by the `FIRE_KEY` 133 ms into `postfire`. With the button released before
`fire_0` ends, the release comes

```math
t_{release} = \frac{700 + 67 + 133}{\rho_{fire}} = 900\ \text{ms}\ (\rho_{fire} = 1)
```

after the pin pull starts, plus frame rounding: each of the three
animation boundaries waits for the update after "done" (L + 1 ms), and the
key's shot is consumed one update after it fires. At 60 fps, pressing just
before update 0: `prefire` starts on update 1, `fire_0` on 44, `postfire`
on 49, the key fires in the advance of update 56 and the grenade is
thrown on update **57 (950.0 ms)**.

Holding the button changes this (this also bears on `grenades.md` Open
question 4): while it is held, `fire_0` restarts each time it ends (every
67 ms, 5 frames at 60 fps), sending a throw notice each time, and the
grenade stays in hand; `postfire`, and with it the release, starts on the
first update after the button is up and the current `fire_0` loop is done.
The fuse still starts at the release (`grenades.md`), so holding does not
cook.

**Q5, second grenade.** With a total of 2 and a clip of 1, after the throw
the clip is 0 and the total 1. When `postfire` ends, the state returns to
idle and the automatic reload request (`weapon_firing.md`, step 10) starts
the `reload` animation on the next update: **400 ms**, the same keyframes as
`select` (a re-draw, no sound). Its end fills the clip; the next pin pull
can start on the update after that (fire held). Nominal times from the
first pin pull: release 900, post-fire end 1133, ready 1533 ms. At 60 fps
(as above): post-fire seen done on update 72 (1200.0 ms), `reload` from
update 73, seen done on update 98 (1633.3 ms, clip 1), second `prefire` on
update 100 (1666.7 ms) with the button held. With total 1 the post-fire end
instead triggers the auto-switch (`inventory.md`).

## Per-frame order

One rendered frame on the client, as far as the view model is concerned:

1. **Input and movement**: held keys become control flags, movement ticks
   run (`movement.md`).
2. **Player update**: camera shake offsets, player rotation, flags.
3. **Sway** (6): phase, amplitude, offsets $(w, h)$ written for the weapon.
4. **Camera**: position (eye height, crouch, camera offsets, backward fire
   push) and rotation (view angles, which already include last frame's
   recoil kick and return), zoom FOV step.
5. **Pre-render**: the player's body model and its attachments are placed;
   then the **weapon update**:
   1. recoil return (`weapon_firing.md`; it changes the view angles, which
      the camera shows next frame);
   2. the camera position and rotation are copied for the weapon;
   3. fire request, gates and the **animation state machine** (7): polls
      "done" as of the last advance, starts animations, consumes a shot
      raised by a key in the last advance;
   4. if a shot fired: the jiggle draws, the flash timer starts, the shot is
      sent (`weapon_firing.md`);
   5. **the view model's camera-space position is set** (4);
   6. the flash point $\vec F$ and offset $\vec o$, the muzzle effect shown or
      hidden and given its positions (9);
   7. weapon parts placed at their sockets (3), player-view attachments;
   8. spread sub-steps, auto-switch, auto-reload request;
   9. view kick if a shot fired (shows next frame).
6. **Effects update** (the muzzle effect's socket-attached particles take
   the socket's position from the current pose), then **render**: world with
   the camera FOV, then the view model and camera-relative effects with the
   view-model projection, using the pose as of the **last** advance.
7. **Engine advance** of all models by this frame's whole milliseconds
   (`animation.md`): the view model's time moves on, its keys fire
   (sounds play now, shots and hits are flagged for the next update).

Consequences:

- The pose drawn on frame n is the animation state after frame n − 1's
  advance; a key crossing is acted on one update after the advance in
  which it fires; an animation started in the weapon update of frame n is
  drawn at its start on frame n and first advances in frame n's step 7.
- The sway and the camera are decided before the weapon update; the
  weapon's position therefore uses this frame's sway and camera, and the
  recoil kick and return of this frame's weapon update move the view on
  the next frame, for the world and the view model alike.

## Edge cases

- **Firing during the draw**: held fire does nothing until the update that
  sees `select` done; that update switches to firing, the fire animation
  starts on the next and its `FIRE_KEY` at 0 gives the shot one update
  later. M92FS at 60 fps: switch on update 0, shot on update 42 (700 ms).
- **Reload during the draw** (magazine weapon, clip not full): the reload
  replaces the draw at once (`inventory.md`); the view model switches from
  `select` to `reload` non-reset (Quirk 1).
- **Sprinting mid-reload**: the reload finishes under sprint, then
  `PreRush` from 0. Sprinting mid-draw: same with the draw.
- **Sprint ends during `PreRush`**: `PreRush` is cut and `PostRush` starts
  at once from 0 and plays in full, so even a one-frame sprint tap blocks
  fire for the whole `PostRush` (M4A1 367 ms).
- **Sprint resumes during `PostRush`**: `PreRush` restarts from 0.
- **Leaving sprint** goes to a full idle (`idle_1`) without rescheduling the
  idle wait, so if the old wait had already passed another full idle can
  follow directly.
- **Switching while zoomed**: the zoom resets at once (`weapon_classes.md`),
  the view model reappears with the new weapon's draw.
- **The last round**: no special animation; `fire_0` runs out, `postfire`,
  then `reload` on the next update after idle is reached (the
  auto-reload request comes at the end of the update that went idle).
- **Grenade and sprint**: sprint started during the pin pull lets
  `prefire` finish and hold its last pose; the cocked and throw stages
  wait until sprint ends. Sprint started during `postfire` before 133 ms:
  the release still happens (the state stays "firing").
- **Rifle pre-fire and sprint**: if sprint starts while a rifle's 0 ms
  `prefire` is current, `fire_0` still starts and its shot fires during
  sprint (one round).
- **Hand model with no character yet** (first weapon at world load): the
  hand keeps its record's own skins; the sleeve texture appears when the
  file names are reset after the character exists.
- **File-name reset mid-draw** (character or uniform change): the draw is
  refreshed non-reset at rate 1.0, so a modified draw rate is lost for the
  rest of that draw.
- **Hand index −1**: no hand model, the gun floats.
- **Two full idles in the list**: the full idle is drawn at random among the
  numbered idles present; stock files have only `idle_1`.
- **Movement at very high frame rates**: below 0.1 u per frame the player
  does not count as moving and the sway decays (Quirk 4).

## Quirks

Keep all of these unless noted.

1. **One-frame pose pop on non-reset switches.** Starting `reload`, the
   full idle, the shell-reload stages and the draw refresh without reset
   draws, for one frame, the previous animation's next key (`animation.md`,
   Quirk 4), because the weapon logic and render run before the frame's
   advance. Between shells of a shell reload the frame shows the first key
   of `PostReload`. Keep for fidelity; it is a one-frame artefact.
2. **Jiggle summed onto all axes.** The view-model shot jiggle adds the sum
   of the three random components to x, y and z alike, while the flash
   point gets them per axis. Invisible with stock data (all `Recoil` zero).
   Keep the rule.
3. **Sway freezes in the air.** While the player cannot "do footsteps"
   (airborne), the sway offsets are not updated: the gun hangs at whatever
   offset it had at take-off and resumes on landing. Keep.
4. **Frame-rate dependent sway gate and amplitude pops.** Moving needs
   0.1 u of travel per frame, so slow movement bobs at low frame rates and
   not at high ones; reversing between moving and stopping mid-ramp
   back-dates the start time linearly against a curved ramp, so the
   amplitude jumps. Keep.
5. **Sleeves from the local profile when spectating.** In first-person
   spectating the hand model is the spectated character's, but its face and
   uniform texture are the spectator's own. Keep.
6. **Semi-auto post-fire cancel.** A new press during a semi-automatic
   gun's `postfire` skips it. Players tap pistols faster than the full
   fire + post-fire cycle. Keep.
7. **No pre-fire for pistols and knives.** Their `prefire` exists in the
   files but is never played, so their first shot or swing starts one
   update earlier than a rifle's. Keep.
8. **Grenade held in the cocked pose.** Holding fire loops the 67 ms
   `fire_0` and keeps the grenade in hand; the throw starts on release.
   Keep (Open question 4).
9. **The view model ignores world depth.** It draws in its own depth slice
   and never pokes into walls. Keep.

## Corrections (2026-10-22)

From a comparison pass over the reference's first-person weapon state
machine (fire, reload, idle, select and sprint animation choice), its bob
manager and its animation-rate sums, against this spec and our
implementation (`games/combat_arms/src/game/view_model.rs` and
`view_model/`, at the commit this section was written on). The sway, the
placement, the projection and the reload, draw and idle rules were found to
match. Ordered by what a player notices.

1. **A new press during or after `postfire` refires on the update that sees
   `postfire` done, with no idle in between** (corrects 7, "Post-fire" and
   "Semi-automatic guns").
   - Reference: while the state is still "firing" and the current
     animation is `postfire`, a fire request (with total ammo > 0, for a
     weapon without `AccumPreFire`) does this: a fully automatic gun
     (`RapidFireCount` 0) waits while `postfire` is not done; on the update
     that sees it done (or at once for a semi-automatic gun) it starts the
     sequence again **in that same update**: `prefire` (reset) if the weapon
     plays one (7: not pistols, knives, primary shotguns), else `fire_0`
     (reset). Only without a request does the end of `postfire` go to idle.
   - Ours: `view_model/machine.rs:649-655` sends a finished `postfire` to
     idle; the request is then seen as an idle request on the next update
     (state "firing", no animation) and the animation starts on the update
     after, two updates late.
   - Expect (60 fps): a full-automatic weapon with a 150 ms `postfire`,
     released, pressed again 50 ms into `postfire` and held: `postfire` is
     seen done on update n, and `prefire` (or `fire_0`) starts on update n
     (not n + 2), so the next shot comes 33.3 ms earlier than ours.

2. **The semi-automatic cancel goes through `prefire` when the weapon plays
   one** (corrects 7, "Semi-automatic guns", and Quirk 6's "starts
   `fire_0` at once").
   - Reference: the cancel restarts the whole sequence, so a semi-automatic
     primary rifle (class with pre-fire, not a shotgun) plays its `prefire`
     first; pistols and knives, which never play `prefire`, go to `fire_0`.
   - Ours: `view_model/machine.rs:644-648` always starts `fire_0`.
   - Expect: a semi-automatic primary (`RapidFireCount` > 0, not a
     shotgun) whose file has a `prefire`: a new press during `postfire` →
     `prefire` from 0 on that update, `fire_0` on the update that sees it
     done. M92FS: `fire_0` at once (unchanged).

3. **Sprinting cuts a knife swing and a double-barrel shot at once** (the
   build's "no firing poses during sprint" option, defined in the US, EU and
   BR configurations; corrects 7, "Sprint", third bullet).
   - Reference: when sprint is on and the current animation belongs to the
     fire sequence, the "done" test is forced true for the knife class and
     the double-barrel gun type, so the sprint-in (`PreRush`, reset)
     replaces `fire_0` / `alt_fire_0` / `postfire` on the first sprinting
     update. Key strings after the cut never fire, so a slash cut before
     its `FIRE_KEY` hits nothing. (A double barrel caught in `prefire`
     still starts `fire_0`, as for rifles.) Other classes keep the old rule
     (the fire animation plays out, then `PreRush`).
   - Ours: `view_model/machine.rs:623-656` lets every fire stage finish
     before the sprint-in (the post-fire cancel of item 2 even runs while
     sprinting).
   - Expect: M9 slash (`fire_0`, keys 155 and 402 ms), sprint starts at
     100 ms → `PreRush` from that update, no hit at 155 or 402. Stab
     (`alt_fire_0`, key 367) cut at 300 ms → no hit. Rifle `fire_0` under
     sprint: plays to its end, then `PreRush` (unchanged).

4. **A grenade or mine asked to fire but not yet in its pin pull goes to
   the sprint pose** (corrects 7, "Sprint", the throw-class bullet).
   - Reference: the throw-class hold applies only while one of their fire
     animations is current. On the update between the request and the
     `prefire` (state "firing", the idle still showing), sprint starts
     `PreRush` and the state becomes the sprint state: the request is
     dropped and no pin pull happens.
   - Ours: `view_model/machine.rs:612-621` exempts the throw class there
     and starts `prefire` under sprint.
   - Expect: M67 idle, fire pressed on update n with sprint already on →
     `PreRush` (no `prefire`, no pin sound); releasing sprint → `PostRush`,
     then idle.

5. **The post-sprint full idle starts from 0** (corrects 7, "Sprint end",
   and Quirk 1's list).
   - Reference: when `PostRush` is seen done the full idle is started with
     a reset, so there is no one-frame pop; without a full idle in the file
     nothing new starts on that update and the subtle idle follows on the
     next.
   - Ours: `view_model/machine.rs:707-713` starts it non-reset
     (`full_idle`, `machine.rs:486-494`), drawing one frame of `PostRush`'s
     next key.
   - Expect: the frame after `PostRush` is seen done shows `idle_1` at
     t = 0.

6. **The fire animation rate includes the weapon's own `FireAniRate`, and
   draw and reload include the character model's bonuses** (corrects 7's
   rates; `weapon_firing.md`'s r_fire already has the term).
   - Reference: one shared sum feeds the first-person animations and the
     third-person driver: fire = 1 + parts' fire bonuses + the weapon
     record's `FireAniRate` + set and conversion bonuses (then the
     mutation buff/nerf); draw = 1 + parts + the character model's select
     bonus + set; reload = 1 + parts + the model's reload bonus + set. Any
     rate above 2.0 plays at 1.0 (unchanged).
   - Ours: `view_model.rs:532-534` (and the weapon cycle,
     `attachments.rs:1189`) use only the parts' bonuses; the weapon's
     `FireAniRate`, read at `data/attributes.rs:384`, is never used, and no
     model bonus is read.
   - Expect: a weapon with `FireAniRate` 0.25 and no parts, `fire_0` of
     105 ms → rate 1.25, 84 ms per animation (at 60 fps one shot per
     6 frames instead of 7). Check which stock weapons carry a non-zero
     value (attribute data).

## Corrections (2026-10-30): attached effects beyond your own view model

From the client's effect database loader, the client weapon's attached-effect handling, the character effects' held-weapon effect and the effect plug-ins (light, particles). Answers the plan's attached-effect questions 1, 4 and 6 (2, 3 and 5 are in `effects_sounds.md` Corrections (2026-10-30) 1 and `special_weapons.md` 5.1).

1. **The XK-2's lights.** The client loads every `.fxf` file in the `ClientFX` folder of the mounted archives; the Steam install has exactly one (`CLIENTFX\CLIENTFX.FXF`), and it has no `Timebomb*` group. A group missing from the database plays nothing, so the stock XK-2 shows **no lights** in this build.
2. **Others and spectators.**
   - The **held** (third-person) weapon has its own single effect: the weapon record's `HHAttachFXName`. When another player's current weapon changes to one that names it, a looping instance is created on that held weapon object (the old one is shut down); it is made for **other** characters only (never for yourself, who are in first person), and its keys (sounds included) play like any group, on every client that sees that player. `PVAttachFXName#` groups are never shown on the held model. So others see and hear a chainsaw's idle or a PSY bomb's wick only if the weapon record names an `HHAttachFXName`.
   - A **first-person spectator** builds a real client view model for the watched player's weapon, so its `PVAttachFXName#` groups are created and shown exactly as for the owner (sounds included, played on the spectator's client).
3. **Space of an attached light and particles.**
   - A `DynaLight` key is never a camera-relative object: the engine creates every effect light as an ordinary world light (the "really close" flag is removed from it); its `ForceLightWorld` property additionally forces it to light the world geometry. Whether a world light also lights the camera-relative view model is the renderer's and stays open (Open question 18).
   - A particle system key's particles follow its `MoveParticlesWithSystem` property: Yes (the property's default) keeps every live particle relative to the emitter, so attached particles move with the view model; No leaves particles in world space where they were emitted. The stock attached groups' values are in the effect file.
   - A key in placement mode 2 (player view): no stock attached key uses it; not traced.

## Follow-up (2026-10-31b): view-model lighting and the player-view point

From the renderer's model lighting, the dynamic-light effect key, the client effect manager's instance placement and the client weapon's muzzle-flash and attached-effect code. Answers Open question 18 and the player-view placement point left open in Corrections (2026-10-30) 3. Test cases 48–52.

1. **A world light lights the view model** (Open question 18). Each frame the renderer gathers every dynamic light it is drawing (all effect lights, `DynaLight` keys included, while dynamic lights are on) into one list, and lights **every** model from that list, the camera-relative view model included: for a camera-relative model the light's position (and the sun's direction) is turned into camera space so the light falls on the view model where it really is relative to the camera; ambient light is looked up at the view model's world position. Only a light marked "light the world only" is skipped (no `DynaLight` key sets that; `ForceLightWorld` adds world lighting, it removes nothing). A dynamic light acts on a model as a point light of its colour and radius R with attenuation 1 / (1 + 19·d²/R²), cut at R. So the XK-2's lights, had the group existed (Corrections (2026-10-30) 1), and the muzzle flash's light do light the gun and hands; the model's light count limit and its choice of the strongest lights apply as for any model.
2. **The point of a "player view" key** (placement mode 2). A key in that mode never places itself; only the weapon code places it, and it does so only for the **muzzle flash** group (and the left gun's): each weapon update while the flash is shown, every key of the group is given the flash point: camera-relative keys (particles, sprites) the camera-space flash offset $ec o$ (section 9; 4), world keys (lights, which can never be camera-relative) the world flash point $ec F$. The key's own `Offset` is not added. Keys of other modes in the same group are placed by their own mode right after. In a **`PVAttachFXName#`** group nothing places a mode-2 key: it stays where it was created, at the view model object's position at that moment plus its unrotated `Offset`; for a light key that point is read as a world position (near the world origin, since the view model's position is a camera-space one), so such a light is effectively lost. No stock attached group uses mode 2.

## Test cases

Each case says which data it needs. **[S]** = needs the Steam install
(measured on Steam build 25595979, unkeyed `GUNS_M_PV_Pistol.rez`); **[B]**
= measured on the BR 1810.04 client's plaintext asset tree, to be
re-checked on Steam once its V2-keyed archive can be read; **[H]** =
measured on an older client release's hand archive; **[D]** = US attribute
data; unmarked cases are synthetic and need no install. Positions in u,
tolerance 1e-3 unless stated; screen coordinates for a 4:3 screen.

1. **[S][H] Hand-to-weapon bone match.** `CHARS_M_HAND\Normal_M_Hand.ltb`
   (43 nodes, pieces `arm_SH`, `Object01`) onto
   `GUNS_M_PV_PISTOL\PV_PST_M92FS_SH.LTB` (51 nodes): every hand bone finds
   a weapon node of the same name; hand bones 0–23 map to weapon nodes
   0–23 (`Scene Root` … `Bip01 R Finger42`), hand bones 24–42
   (`Bip01 L Clavicle` … `Bip01 L Finger42`) to weapon nodes 32–50. Weapon
   nodes 24–31 (`Box01`, `Box05`, `Box08`, `Box06`, `Box03`, `Box10`,
   `Box11`, `Box12`) are the gun's, parented under `Bip01 R Hand`.
2. **[S] Drawn pieces.** The M92FS view model draws the weapon model's 5
   pieces (`Object04`, `Object05`, `Object02`, `Object06`, `Object07`) and
   the hand model's 2; the `ANI_PST_M92FS_SH.LTB` pieces (`arm_SH`,
   `Object01`, `PV_COLT`) are not drawn.
3. **[D] Sleeve texture.** Default character (`Model0`, `HandIndex` 0),
   face 0, uniform 0 → hand model `CHARS_M_HAND\Normal_M_Hand.ltb`, hand
   skin 0 `CHARS_T_HAND\HAND_00_00.dtx`; face 2, uniform 3 →
   `CHARS_T_HAND\HAND_02_03.dtx`. Team JSF or Creed gives the same.
4. **[D] Rest placement.** M92FS in hand, idle, standing still, sway 0:
   view-model origin in camera space (−0.1, −0.2, 0.7), rotation identity
   relative to the camera for any camera yaw and pitch.
5. **[S] Muzzle socket at rest.** M92FS, `idle_0` at t = 0 (root
   translation (−0.4, −3.6, −1.2) included): `flash` socket in model space
   (0.2688, −0.2043, 1.6648), camera space (0.1688, −0.4043, 2.3648),
   screen (0.1148, −0.3666); `breach` camera (0.3409, −0.4217, 1.5837).
   At t = 600: `flash` camera (0.1703, −0.4642, 2.3734).
6. **[S] Muzzle socket through the shot.** M92FS `fire_0` (Pos added),
   `flash` in camera space: t = 0 (0.1688, −0.4043, 2.3648); t = 29
   (0.2111, −0.4272, 2.2522); t = 52 (0.2032, −0.2787, 2.0992); t = 103
   (0.1503, 0.0527, 1.8799), screen (0.1286, 0.0602): the muzzle rises
   above the centre line; t = 180 (0.2164, −0.0832, 2.1452).
7. **[S] Draw starts off screen.** M92FS `select` at t = 0: `flash` camera
   (0.3420, −2.1896, 2.3924), screen y −1.963 (below the bottom edge); at
   t = 650 it equals the `idle_0` rest pose of case 5.
8. **[B] Rifle socket.** M4A1 (`PV_AR_M4A1_SH.LTB`, `Flash` and `Breach` on
   `Box12`, node 2), `idle_0` at t = 0, Pos (0.3, −0.6, 1.15): `Flash`
   camera (0.6281, −0.6335, 4.5961), screen (0.2198, −0.2956); `Breach`
   camera (0.7388, −0.7128, 1.9400). Same values at `fire_0` t = 0 and
   t = 105.
9. **[D] World flash point.** M92FS, camera at the origin facing +Z, no
   sway, no shot: $\vec F$ = (2.74, −2.6, 9.259921). Camera yawed 90° right
   (forward +X, right −Z): $\vec F$ = (9.259921, −2.6, −2.74).
10. **[D] Shell start.** M16A3 (`BreachOffset` (1, 1, 1)), camera at the
    origin facing +Z: breach point (5.7, −3.15, 9.9). M4A1: (4.7, −4.15,
    8.9).
11. **World and view-model FOV.** 1024×768: world 91.309° × 75°, view
    model 63.742° × 50°. 1920×1080: 107.512° × 75° and 79.317° × 50°.
    1680×1050: 101.673° and 73.453°. 1280×1024: 87.612° and 60.474°.
12. **Zoom leaves the view-model FOV alone.** L96A1 zooming to level 1
    (world FOV 91.309° → 20°, `weapon_classes.md` case 1 with the
    recomputed `FovX`): view-model vertical FOV stays 50°; the model is
    hidden from the press frame and shown on the frame the zoom-out to
    level 0 ends.
13. **Sway ramp, running, 60 fps.** Run flag on, moving flag on, speed
    above 6 u/s, $\Delta t$ = 1/60 s and the clock advancing by the same,
    sway 0 before frame 1, `WeaponSway` 1. Per frame (A, φ, w, h):
    frame 1: 0, 0, 0, 0; frame 2: 0.66987, 0, −0.00058, 0; frame 3: 2.5,
    0.146608, −0.0019592, 0.0010839; frame 4: 5.0, 0.293215, −0.0034227,
    0.0041504; frame 5: 6.66667, 0.439823, −0.0038048, 0.0077051; frame 6:
    8.33333, 0.586431, −0.0037053, 0.0115233; frame 7: 10, 0.733038,
    −0.0030902, 0.0149178; frame 12: 10, 1.466077, 0.0040674, 0.0031187.
    M4A1 view-model origin on frame 7: (0.2969098, −0.5850822, 1.15).
14. **Sway steady state.** As case 13, frame 30: φ = 4.105014, w =
    0.0008368, h = 0.0140592; frame 60: φ = 2.220059, w = 0.0092186,
    h = −0.0144474. Extremes over a cycle: |w| ≤ 0.010, |h| ≤ 0.015.
15. **Sway decay.** As case 13, moving for frames 1–60, not moving from
    frame 61: A = 10 (frame 61), 8.33333, 6.66667, 5.0, 2.5, 0.66987,
    0 (frame 67); φ keeps advancing until frame 67 (3.246312) and is 0
    from frame 68; offsets on frame 64: w = 0.0049114, h = −0.0046586;
    from frame 67 on, (0, 0).
16. **Walk and crouch pace.** As case 13 with the run flag off (walk key or
    crouch): φ per frame 0.0628319; frame 3: w = −0.0020823, h = 0.00047;
    frame 7: A = 10, φ = 0.314159, w = −0.0066913, h = 0.0088168.
17. **Ramp at 100 fps.** $\Delta t$ = 0.01 s: A on frames 2..11 = 0.24472,
    0.95492, 2.06107, 3.45492, 5.0, 6.0, 7.0, 8.0, 9.0, 10.0 (full after
    0.1 s of wall time, as at 60 fps).
18. **Frame-rate gate.** Moving at 20 u/s with the moving flag on: at
    60 fps (0.333 u per frame) the sway ramps up; at 250 fps (0.08 u per
    frame) it decays to 0 within 0.1 s and stays 0.
19. **Airborne freeze.** Running at full sway, the last grounded update
    leaves (w, h) = (0.0092186, −0.0144474), A = 10, φ = 2.220059. On every
    update while airborne after the jump ("can do footsteps" false) the
    sway is not updated and the view-model origin keeps that offset. On the
    first grounded update the steps resume from the stored A and φ (φ
    advances by one frame's P·Δt).
20. **Sway scale.** Profile option `WeaponSway` 5 → console variable 0.5 →
    case 14's offsets halved; profile without the key → 0 → no sway at
    all.
21. **Shot jiggle (synthetic data).** A weapon with `Recoil` (0.1, 0.2,
    0.3), draws (0.05, −0.1, 0.2), no sway: on the shot frame the
    view-model origin is Pos + (0.15, 0.15, 0.15) and the flash offset is
    Pos + MuzzlePos + (0.05, −0.1, 0.2); the next frame both are back to
    Pos and Pos + MuzzlePos.
22. **[S] Draw with fire held, 60 fps.** Switch to the M92FS on update 0
    (draw rate 1), fire held: `sound_key 3` fires in the advance of update
    17 (animation time 300.0 ms); update 40 (666.7 ms) sees `select` done
    and enters firing; `fire_0` starts on update 41; the first shot is
    consumed on update 42 (700.0 ms). Without fire: `idle_0` starts on
    update 41.
23. **[S] Pistol post-fire cancel.** M92FS, tap at update 0 (fire held one
    frame): `fire_0` 180 ms then `postfire` 150 ms. A second press while
    `postfire` is playing starts `fire_0` (reset) on the next update; the
    rest of `postfire` is skipped. No press: `idle_0` follows `postfire`.
24. **[B] Knife without pre-fire, 60 fps.** M9 idle, fire pressed just
    before update 0: update 0 enters firing, `fire_0` starts on update 1
    (16.7 ms), the first hit is consumed on update 11 (183.3 ms), the
    second (402 ms key) on update 26 (433.3 ms). `prefire` (1 ms) never
    plays.
25. **[B] Grenade release, 60 fps.** M67, fire pressed just before update
    0 and released any time before update 49: `prefire` from update 1, `fire_0`
    from 44, `postfire` from 49, release (shot consumed) on update 57
    (950.0 ms), idle on 72, `reload` from 73, clip 1 on 98 (1633.3 ms),
    second `prefire` on 100 with fire held. Exact-time values: 900, 1133,
    1533 ms.
26. **[B] Grenade held.** As case 25 but fire held through update 119 and
    up before update 120 (2000 ms): `fire_0` restarts on updates 49, 54,
    59, …, 119 (every 5 updates, a throw notice each time); the loop
    started on 119 is seen done on 124, `postfire` starts there, and the
    grenade is released on update 132 (2200.0 ms).
27. **[S] Sprint cycle, 60 fps.** M92FS idle, sprint on from update 0:
    `PreRush` from update 0, `rush` (looping) from update 16; sprint off on
    update 60: `PostRush` from update 60, update 85 sees it done and plays
    `idle_1` (not `idle_0`). Fire held from update 60 is ignored through
    update 85; update 86 enters firing and `fire_0` starts on 87.
28. **[B] Sprint mid-reload.** M4A1, reload started at t = 0 (rate 1),
    sprint on at t = 1000 ms: `reload` keeps playing; the update that sees
    it done (first after 2301 ms of advance) fills the clip and starts
    `PreRush` from 0 in the same update.
29. **[S] Last round of the M92FS.** Clip 1, total 16, fire held: the shot
    on `fire_0`'s key, then `fire_0` to its end, `postfire` (150 ms), idle
    reached and the reload requested in the same update, `reload` (non-reset)
    from the next update; the clip becomes 15 on the update that sees
    `reload` done.
30. **[S] Dry fire not reached.** M92FS clip 0, total 0, fire held: no
    `dryfire` animation, the view model stays on `idle_0` / `idle_1`.
31. **Muzzle flash visibility.** A shot at clock time T with a 0.05 s flash
    effect: the effect is shown on every weapon update with clock < T +
    0.05 s and hidden from the first one at or after it; at 60 fps that is
    the shot frame and the 2 following (3 frames). Zoomed (model hidden):
    never shown.
32. **One-frame pop.** M4A1 in `idle_0` with its time between keys 0 and 1
    (so key b = 1), reload key pressed before frame n: `reload` starts
    non-reset in frame n's weapon update, and the image of frame n shows
    `idle_0`'s key 1 pose for every node; the image of frame n + 1 shows
    `reload` sampled at the time reached by frame n's advance.

Follow-up cases (Open questions 9–16):

33. **[B] Full-automatic cadence.** M4A1, rate 1, fire held, 60 fps
    (advances of 16 or 17 ms): `fire_0` (105 ms) is seen done after 7
    advances, so shots are consumed on updates n, n + 7, n + 14, …:
    116.7 ms per round. At 100 fps (10 ms advances): 11 advances, 110 ms
    per round. No client timer shortens or lengthens this.
34. **Dry fire unreachable.** M4A1, clip 0, total 30, fire held: the request
    is cleared, the automatic reload starts; no `dryFire`, no `DryFireSnd`.
    Clip 0, total 0: idle animations only, silent.
35. **Holster.** Switch from M4A1 to M92FS with the stock files (no
    `deselect`): the M92FS `select` starts in the weapon update of the
    switch. A test file that adds a 400 ms `deselect` to the M4A1: it plays
    at rate 1.0 even with a draw-rate bonus, and the M92FS `select` starts
    on the first update after it is seen done (after 401 ms of advance; at
    60 fps on update 25, 416.7 ms).
36. **[data] `reload_1`.** A double-barrel weapon whose file has `reload`
    and `reload_1`: clip 1 of 2 → `reload_1`; clip 0 → `reload`. Any other
    gun type with clip > 0 → `reload`.
37. **Projectile piece.** A launcher with clip 1, total 1, drawn: the
    piece shows. After the shot (clip 0, total 0) it is hidden, and the
    `HiddenPiece` list is hidden too (total < 1) at the end of the fire
    sequence. Drawn again with clip 0, total 0: hidden.
38. **[data] Hand colour.** A character using `Hand8` with model colour 1:
    hand skins = `ColorSkin1_0..11`, i.e. skin 5
    `CHARS_T_BODY\CM_Fur_Ghillie_Suit_DE_SD.DTX`; colour 0 (none) → `Skin0..`.
39. **Hand reset.** The local character effect is created while the M4A1
    is already out → one file-name reset (sleeve texture applied, draw
    refreshed non-reset at rate 1.0). A uniform change later in the same
    life → no change until the next draw.
40. **Projection.** 1920 × 1080 at world start: `FovX` 107.51°, view model
    horizontal 79.32°. The window becomes 1280 × 1024 without a world
    start: `FovX` stays 107.51° (stretched picture), the view model's
    horizontal becomes 2 arctan(1.25 tan 25°) = 60.47°. Next world start:
    `FovX` = 2 arctan(1.25 tan 37.5°) = 87.61°.
41. **Footing ray.** Not on the ground, not jumped, hull centre 140 u above
    a floor → can step (sway continues, no fall status); 160 u above → cannot
    step, the client sends "falling" on that tick (once).
42. **Spectated sway.** Spectating a player who holds forward with the run
    flag set while standing against a wall: the spectator's gun sways at
    1.4 cycles/s, at full amplitude 0.1 s after the key went down; the same
    player standing with no key → the sway decays to 0 over 0.1 s.
43. **Flash hider.** Muzzle part with `FlashHiderLevel` 2 on a weapon whose
    record has `PVFlashHiderFXName2` = `G`: group `G` is used (not the part's
    or the weapon's own muzzle group). The same part on a weapon without that
    key → no first-person flash at all.
44. **Attached effects.** XK-2 drawn: its five `PVAttachFXName` effects are
    created hidden and shown when the model becomes visible. A key
    `HIDE_PVATTACHFX 1` hides `TimebombRed`; after hiding and showing the
    weapon (scope in and out) it is still hidden until a `SHOW_PVATTACHFX 1`
    key.
45. **Missing groups (Corrections 2026-10-30, 1).** XK-2 drawn with the Steam effect file → its five `Timebomb*` groups are created as nothing: no light, no sound.
46. **Held weapon effect (Corrections 2026-10-30, 2).** Another player switches to a weapon with `HHAttachFXName` g → g starts looping on his held weapon on your client; he switches away → g is shut down; the same weapon in your own hands → no held-model effect, only its `PVAttachFXName#` groups.
47. **Particles in emitter space (Corrections 2026-10-30, 3).** An attached particle key with `MoveParticlesWithSystem` Yes, the view model swaying 2 u right over 0.1 s → its live particles shift 2 u right with it; with No → they stay where emitted.

48. **Light on the gun (Follow-up 2026-10-31b, 1).** A `DynaLight` key (radius 200, white) 20 u in front of the camera: the view model's faces toward it brighten; at a view-model point 20 u from the light the light's share is 1 / (1 + 19·400/40000) = 1/1.19 ≈ 0.84 of its colour; a point 210 u away gets none.
49. **Light follows the camera frame (logic).** Turning the camera 90° while the light stays put in the world: the lit side of the gun turns with the light's direction in camera space (the light is not stuck to the camera).
50. **Muzzle-flash light point.** `AK47_PV_Muzz`'s light key (mode 2) sits at the world flash point $\vec F$ each update while shown, its `Offset` ignored; its particle key (socket mode) at the `Flash` socket + (0, 0, 0.5).
51. **Mode-2 particle in the flash group.** A camera-relative particle key in mode 2 is placed at $\vec o$ (camera space), e.g. (6, −5, 30) for a flash offset of that value.
52. **Mode-2 key in an attached group.** A mode-2 key in a `PVAttachFXName0` group stays at the view model's creation position + its `Offset` for the group's whole life; it does not follow the gun's sway.

## Open questions

1. **Steam build data.** Only the pistol archive of the Steam install could
   be read (Steam build 25595979; an update to build 25901084 was pending on
   the measuring machine and not applied). The rifle, knife, grenade, SMG,
   sniper and hand archives are V2-keyed. Re-run cases 1, 8, 24–26 and 28
   and the BR lengths in 7 and 8 against the Steam files when they can be
   read; the M92FS pair is identical in size in both builds, which suggests
   the view models did not change.
2. **Console defaults on Steam.** `PVModelFOV` 50, `reallyclose_near` /
   `reallyclose_far` 0.1 / 7.0, `FovY` 75 and the bob variables come from
   the reference code; the Steam binaries are packed and their defaults
   could not be read. Check in game by printing these console variables.
3. **Muzzle effect file.** The Steam effect database is a binary file in a
   different form from the text effect group read here (from an older
   release). Confirm that `AK47_PV_Muzz` still pins its particles to the
   `Flash` socket with offset (0, 0, 0.5) and lasts 0.05 s: frame-capture a
   pistol shot.
4. **Grenade hold (relates to `grenades.md` Q4).** The view-model logic
   loops `fire_0` while fire is held and throws only after release. Hold
   fire on a frag in game for 3 s: does the grenade stay in hand?
5. **Drawn pose lag.** Per-frame order says the render uses last frame's
   advance; confirm with a capture of the first frames after a switch
   (case 32).
6. **Sway option default.** A profile without `WeaponSway` gives no sway;
   whether a fresh Steam profile writes 10 is not known.
7. **Hand coverage.** Whether every stock hand model's bone names exist in
   every weapon model (women's hands, cosmetic hands) needs the V2-keyed
   hand archive (`animation.md` Q8).
8. **Flash point age for shells.** The shell starts at the flash point "as
   last computed"; whether that is this frame's or the previous frame's
   depends on when the shot's effect object is created relative to the
   flash update. The difference is at most one frame of camera motion.
9. **C1 and the gameplay clock.** Does the view model's key time the shot,
   the knife hit and the clip refill, or the weapon's own timers? Full-auto
   restart and a reload asked during the fire sequence.
   **Answered:** the view model's animations and keys are the client's only
   clock for shots, knife hits, releases, refills and draws; the client
   never reads `FireDelay` (the server's 30 ms default floor never binds).
   Full-auto: `fire_0` restarts on the first update that sees it done, so
   the cadence is the frame-rounded length (116.7 ms for the M4A1 at
   60 fps). A reload asked during the sequence starts after `postfire`, and
   the clip fills when `reload` is seen done. C1 stands. Applied in 8 (What
   times the gameplay); case 33. The server's own reload window is
   `characters.md` 7 (it ends before the client's refill).
10. **C6, dry fire.** **Answered:** never plays, and no dry-fire sound
    plays (the direct sound path is disabled and the animation that could
    carry its key never runs). Applied in 7; case 34. `effects_sounds.md`
    section 11 is corrected to match.
11. **Holster and `reload_1`.** **Answered:** `deselect` plays only when the
    file has one (none stock), at rate 1.0, and the switch waits for it;
    `reload_1` replaces `reload` for the double-barrel gun type with rounds
    left in the clip. Applied in 7; cases 35, 36.
12. **Pieces and attachments on the view model.** **Answered:** hidden
    pieces follow the total ammunition, the projectile piece follows shots,
    reloads and the draw, attached effects are created hidden and follow
    visibility and their keys, player-view attachments come from the
    character record (none stock). Applied in 3; cases 37, 44.
13. **The hand model per character.** **Answered:** hand index from the
    effective character record (all stock bodies 0), skin list from the
    hand record or its `ColorSkin{c}_` list for the model colour (only the
    two ghillie hands have them), sleeve from the body's `HandsSkin` entry
    for the face. The file names are reset only when the local character
    effect is created; otherwise changes show at the next draw. Applied in
    2; cases 38, 39.
14. **Projection.** **Answered:** confirmed 0.1 u / 7.0 u, 50° vertical for
    the view model with the live screen aspect; the world keeps 75° vertical
    and derives `FovX` from the screen aspect at each camera reset only.
    Applied in 5.2; case 40.
15. **Spectating and the 150 u footing ray.** **Answered:** the spectated
    player's control flags drive the pace and the moving test, with a fixed
    3.5 u per frame distance; the footing ray tests solid world and objects
    150 u below the hull centre and gates the sway, own footsteps and the
    start of a fall. Applied in 6; cases 41, 42.
16. **The flash hider.** **Answered:** it only selects another effect group
    (`PVFlashHiderFXName`h / `HHFlashHiderFXName`h); a missing key removes
    the flash. Nothing in the US data uses it. Applied in 9; case 43.
17. **Left open by the 2026-10-22 corrections.** (a) Which stock weapons
    carry a non-zero `FireAniRate` and which character models carry
    select or reload bonuses (item 6 matters only for those). (b) Which
    stock primaries are semi-automatic with a `prefire` in their file
    (item 2). (c) Whether restarting an animation that is already current
    without a reset rewinds it in the engine (the draw refresh of Edge
    cases) was not traced.

Contradictions with existing specs (this spec's reading of the reference
differs; the other specs are not edited here):

- **C1, `inventory.md` 8.2 (knife timeline).** It plays a 1 ms pre-fire
  before the swing. The reference never plays `prefire` for secondary or
  melee weapons, so the swing starts one update earlier: first hit at
  183.3 ms, not 200.0 ms, at 60 fps (case 24).
- **C2, `weapon_firing.md` "Zero-length prefire".** The extra update applies
  only to weapons whose class plays `prefire` (primaries other than
  shotguns, throwables, specials); pistols and knives skip it.
- **C3, `grenades.md` 3 and 4.** It puts the release on the 67 ms throw
  animation's fire event and says the throw happens whether or not the
  button is held. The stock files carry `THROW_KEY` (a server notice) in
  `fire_0` and the releasing `FIRE_KEY` at 133 ms of `postfire`; holding
  fire keeps the grenade cocked. Its release-delay formula becomes
  $(700 + 67 + 133)/\rho_{fire}$ plus frame steps when released early (Q3
  above); Q5's re-ready is the 400 ms `reload` (above).
- **C4, `animation.md` "Per-frame order" and its Open question 2.** It runs
  the model advance before the game logic. In the reference client frame
  the game update, including the weapon logic and the render, runs first
  and the engine advances the models afterwards, so its Quirk 4 is visible
  for one frame on every non-reset start (Quirk 1 here).
- **C5, `weapon_classes.md` `FovX` 90.** The game recomputes `FovX` from
  `FovY` and the screen aspect at each world start (91.31° at 4:3), so its
  zoom speeds and test FOVs use that value, not 90.
- **C6, `weapon_firing.md` "Total = 0: holding fire plays the dry-fire
  animation".** The fire request is cleared whenever the clip is 0, before
  the state machine, and the clip cannot be above 0 with a total of 0, so
  the dry-fire state is never entered and no dry-fire animation plays.
- **C7, `weapon_firing.md` step 7 (view-model jiggle "random per axis").**
  The model position gets the sum of the three draws on all axes; only the
  flash point gets them per axis (Quirk 2). No stock weapon has a non-zero
  `Recoil`, so nothing visible changes.
18. **An attached light on the view model** (Corrections 2026-10-30, 3). Answered (Follow-up (2026-10-31b) 1): yes, every dynamic light lights the view model, placed in camera space.
