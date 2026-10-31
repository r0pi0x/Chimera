# Combat Arms: world objects

Source basis: reference material the user pointed the spec session to on the game server's map objects (the world-model object family: plain, active, sliding, rotating and spinning world models, doors, switches, breakables; the destructible aggregate; keyframers and their keys; triggers, command triggers, touch triggers, target triggers, event counters, start-up commands; props and prop types; explosions placed in maps; the game's object base messages), the server's command manager, the engine's object mover (pushing, carrying, crushing), world-model placement and network object updates, and the client's movement manager (how a local player is pushed and carried, and how moving world models are rewound for movement). Checked against every world file of the Steam install (Steam build id 25595979, all 105 maps, the 72 V2-keyed ones decrypted with the local keys), as numbers and structure only.
Status: draft

Sections: Summary; Units and conventions; Constants; Behavior (1 Placing and drawing a world model; 2 The active world model; 3 Sliding motion; 4 Rotating motion; 5 Waveforms; 6 Blocking, pushing and crushing; 7 Doors and switches; 8 Spinning world models; 9 Keyframers and keys; 10 Riders; 11 Breakables; 12 Triggers and other event objects; 13 Command strings; 14 Object sounds; 15 Spawned props and models; 16 Geometry that a map lacks: HGOILRIG; 17 What the server sends and what clients run); Per-tick order; Edge cases; Quirks; Test cases; Open questions; Corrections (2026-10-22); Follow-up (2026-10-31b) (event objects, cameras and markers; client motion and riders; world-model debris; prop debris; explosions and spawns).

This spec continues `world_dat.md` (file layout, object and property encodings, "Moving world models and volumes", rotation-to-matrix), `movement.md` (volumes, the local player's mover), `game_modes.md` (S11 round cleanup, S15 map commands, objective objects), `effects_sounds.md` (section 10 and 11: how a sound plays, `SoundFX`), `grenades.md` (section 9: blast damage) and `models_ltb.md` / `animation.md` (prop models). Where it corrects them, it says so (Open questions 1 and 2).

---

## Summary

- A map's moving parts are **world models**: BSPs in the world file named after an object. The object is placed at the centre of its BSP's box; its `Rotation` property only defines the local axes for the motion, because the BSP is already stored in place. Every movement is computed on the server, once per server frame, and reaches clients as ordinary object position and rotation updates.
- Doors, sliding and rotating world models, switches and breakables share one four-state machine (off, powering on, on, powering off) driven by messages (`TRIGGER`, `ON`, `OFF`, `ACTIVATE`, `LOCK`, …), by a player's use key, by timers (auto-close) and by the commands they send at each transition. Motion advances by speed × frame time each frame, shaped by a waveform, and **stops** (does not push, does not crush) while a player is in the way. Nothing in the game deals crush damage.
- Keyframers move named objects along a list of keys stored as blind data in the world file, by time stamps or at a constant path speed, with straight or Bézier segments. In the stock maps they move only cameras and sound emitters; no stock platform or elevator is keyframed.
- Breakables are world models with hit points: at 1 HP or less they run their death command, spawn debris and an optional explosion or item, and either vanish (removed) or become invisible, non-solid and not ray-hit.
- Triggers are boxes that run up to ten command strings when a player touches them or when they receive `TRIGGER`. Command strings are a small language (`msg`, `delay`, `rand`, `loop`, variables, `if`/`when`) run by the server's command manager; messages are delivered at once, delays count in real frame time.
- HGOILRIG's spawn decks do not exist anywhere: the map is a copy of DARKHILLS with the terrain and buildings removed; its spawn points, lights, cars and crates kept their Dark Hills positions over what is now open sea (section 16).

## Units and conventions

- Distances in world units (u), file frame (+X right, +Y up, +Z forward, left-handed; `world_dat.md`). Angles in the file are radians; angle **properties** of the moving classes (`RotationAngles`) are **degrees**. Times in seconds.
- "Frame" is one server frame; dt its frame time in seconds, clamped to [0, 0.2]. The stock server runs at 20 Hz (dt = 0.05) (`game_modes.md`, Per-tick order); examples use that.
- Object and property names, class names, message words and command words are data the maps carry and are named as such. Message words are matched without regard to case.
- R(p, y, r) is the rotation of `world_dat.md` "Rotation to matrix" (yaw, then pitch, then roll); R_y(θ) rotates +Z toward +X for positive θ: x' = x cos θ + z sin θ, z' = −x sin θ + z cos θ.
- "Player" means a player character object on the server; bots (AI characters) are named where they differ.

## Constants

| Name (ours) | Value | Unit | Meaning |
|---|---|---|---|
| next_frame | 0.001 | s | "Update next frame": the smallest update delay; an object asking for it updates on the next server frame |
| frame_clamp | 0.2 | s | Largest server dt |
| min_rotate_delta | 1 | ° (0.0174533 rad) | Threshold of the per-axis rotate-direction choice (section 4) |
| sound_inner_fraction | 0.25 | × | Inner radius of an active world model's sounds = 0.25 × `SoundRadius` |
| awm_sound_radius | 1000 | u | Default `SoundRadius` of the active world models and keyframers |
| trigger_dims | (16, 16, 16) | u | Default `Dims` (half-extents) of `Trigger` and `TouchTrigger` |
| trigger_timed_period | 0.1 | s | Update period of a timed or attached trigger |
| trigger_sound_radius | 200 | u | Default `SoundRadius` of a trigger's `ActivationSound` |
| cmd_pending_max | 512 | commands | Pending delayed/looping commands at once |
| cmd_length_max | 256 | bytes | Longest stored delayed command (longer is cut) |
| cmd_id_max | 16 | bytes | Longest command id (`delayid`, `loopid`, `repeatid`), 15 characters kept |
| cmd_vars_max | 512 | variables | Command-manager variables (`int`, `obj`) |
| cmd_when_max | 32 | entries | Pending `when` conditions |
| carried_max | 32 | objects | Objects one mover carries in one move (more are detached; the engine's bound check lets a 33rd slip into a 32-entry list, never reached in practice) |
| breakable_death_hp | 1 | HP | A destructible dies when hit points fall to this or below |
| kf_slowon_add | 0.003 | × rate | Keyframer `SlowOn` additive term (world models use 0.001) |
| awm_slow_add | 0.001 | × rate | Active world model `SlowOn` / `SlowOff` additive term |
| activation_distance | 100 | u | Client use range default when the client attribute `ActivationDistance` is absent (`inventory.md` owns the use ray) |
| debris_speed_xz | ±200 | u/s | World-model debris launch speed per horizontal axis (uniform) |
| debris_speed_y | 100..300 | u/s | World-model debris upward launch speed (uniform) |

## Behavior

### 1. Placing and drawing a world model

**Finding the geometry.** Every world-model class (`WorldModel`, the door, switch and moving classes, `Breakable`, the volumes) asks the engine for the BSP whose name equals the object's `Name` (`world_dat.md`, Objects). The object then:

1. is **moved to the centre of that BSP's box** (the BSP header's translation, which is the centre of min..max for ordinary world models). The object's `Pos` property is overwritten; it survives only as the default rotation pivot (section 4).
2. gets half-extents (max − min)/2 of the BSP box (then re-fitted around the rotated box whenever it rotates);
3. **drops its `Rotation`**: the stored rotation is set aside and the object starts with the identity rotation. The BSP points are already in world space, in place (`world_dat.md`), so applying the rotation again would rotate the geometry twice. The set-aside rotation Q only defines the local axes of `MoveDir` and `RotationAngles`.

**Transform.** With T the BSP translation, p a BSP point, P the object's current position and Rₒ its current rotation, the point is at

```math
p_{world} = R_o\,(p - T) + P
```

At load P = T and Rₒ = I, so the geometry sits exactly where the file stores it. Collision, ray hits and drawing all use this transform. The render data of the world model (the nested render world of the same name, `world_dat.md`) is drawn with the same transform.

**Flags** (properties, class defaults for `WorldModel`): `Visible` true, `Solid` true, `Rayhit` true, `Gravity` false, `BoxPhysics` true (the volumes and rotating classes override it; with `BoxPhysics` the engine collides the object as its box instead of its BSP, see Open question 6), `BlockLight` true (compiler only), `FogDisable` false, `Alpha` 1 (compiler only), `BlendMode` `None` / `Additive` / `Translucent` (drawn additive-translucent / translucent), `IsKeyframed` false, `StartHidden` false, `Attachments` empty, `RemoveAttachments` true, `AttachDir` (0, 200, 0). The solid and ray-hit rules are `world_dat.md`'s ("Moving world models and volumes").

- `StartHidden` true: the object sends itself `HIDDEN 1` when created (invisible, non-solid, not ray-hit).
- A world model is **movable** (the engine moves it at all) when `IsKeyframed` is true, when it can be destroyed (can take damage and is not `NeverDestroy`), or for the active classes when `MoveDist` ≠ 0 or `RotationAngles` has length² ≥ 0.01. Every ordinary BSP carries the movable flag (0x02) anyway.
- **Attachments**: after all objects exist, each name in `Attachments` (`;`-separated) is looked up and every object of that name is attached with its current offset from the world model, expressed in the world model's local frame: offset = Rₒ⁻¹ (pos − P), rotation offset = rot · Rₒ⁻¹. Attached objects follow every move and rotation of the world model (teleported to P + Rₒ·offset each move). HGOILRIG's sea (`SlidingWater`, a `SlidingDoor`) attaches the `Water` and `PoisonGas00` volumes this way, so the rising sea carries its liquid and gas volumes (Test case 8). An attached world model forwards `ACTIVATE` messages to the world model it hangs on.
- On removal, attachments are removed with it (`RemoveAttachments` true) or sent infinite explode damage (false).

**Messages every world model takes** (on top of the base messages of section 13): `ATTACH [names]`, `DETACH [names]`, `ATTACHCLASS <class>` (attach objects of a class found along `AttachDir` from the object position, or, when `AttachDir` is shorter than 1 u, every object of that class whose position is inside the world model's box), `DESTROY` (allow destruction: clears `NeverDestroy`), `DAMAGEON` / `DAMAGEOFF`, `PLAYERDAMAGEON` / `PLAYERDAMAGEOFF` (whether players' damage counts), `AIDAMAGEON` / `AIDAMAGEOFF`, `ACTIVATE` (forwarded to the parent when attached).

**Bullet marks** on a world model are attached to it and move with it; at most 40 per object, after which the closest mark (or the farthest, or the first) is moved instead of creating a new one.

### 2. The active world model

`SlidingWorldModel`, `RotatingWorldModel`, `SpinningWorldModel`, `SlidingDoor`, `RotatingDoor`, `RotatingSwitch` and `Breakable` are active world models: a world model with a motion type (static, sliding, rotating; stored as the hidden long property `AWMType` 0, 1, 2) and a state machine.

**Properties** (base defaults; door and other class overrides in sections 7 and 8):

| Property | Type | Default | Meaning |
|---|---|---|---|
| `PlayerActivate` | bool | false | The player's use key can activate it |
| `AIActivate` | bool | false | Bots can activate it |
| `StartOn` | bool | false | Starts by powering on at load |
| `TriggerOff` | bool | true | In the on state, a trigger turns it off (else it re-arms the on state) |
| `RemainOn` | bool | true | Stays on until triggered (else turns itself off after `OnWaitTime`) |
| `ForceMove` | bool | false | Never reverses on a re-trigger and ignores players in the way in its own test (section 6) |
| `ForceMoveOn` / `ForceMoveOff` | bool | false | The same, only while powering on / off |
| `Locked` | bool | false | Ignores triggers and activation; plays `LockedSound`, runs `LockedCommand` instead |
| `RotateAway` | bool | true | A rotating one opens away from whoever activated it (section 4) |
| `Waveform` | string | `Linear` | `Linear`, `Sine`, `SlowOff`, `SlowOn` (section 5) |
| `ActivateType` | string | `<none>` | Name of the use prompt the client shows; read only when activatable |
| `PowerOnSound`, `OnSound`, `PowerOffSound`, `OffSound`, `LockedSound` | string | empty | Sounds at each transition (section 14) |
| `SoundPos` | vector | (0, 0, 0) | Sound offset from the object; (0, 0, 0) exactly means "attached to the object" |
| `SoundRadius` | real | 1000 | Outer radius of those sounds |
| `LoopSounds` | bool | false | Transition sounds loop until the next one |
| `OnCommand`, `OffCommand`, `PowerOnCommand`, `PowerOffCommand`, `LockedCommand` | string | empty | Command strings run at each transition (section 13) |
| `ActivateAlarmLevel` | long | 0 | Bot alarm stimulus (bots only) |
| `MoveDir` | vector | (0, 0, 0) | Local slide direction; length² < 0.0001 means (0, 1, 0) |
| `MoveDist` | real | 0 | Slide distance, u |
| `RotationPoint` | string | empty | Name of a `Point` object whose position is the pivot |
| `RotationAngles` | vector | (0, 0, 0) | Pitch, yaw, roll to turn when on, degrees, local axes |
| `PowerOnTime` | real | 1 | Duration of powering on, s |
| `PowerOffTime` | real | 0 | Duration of powering off, s; 0 or less means `PowerOnTime` |
| `MoveDelay` | real | 0 | Delay before a move starts, s (Quirk 2) |
| `OnWaitTime` / `OffWaitTime` | real | 0 | Minimum time in the on / off state before a trigger is obeyed, s; `OnWaitTime` is also the auto-off delay |
| `Solid`, `Rayhit`, `Visible` | bool | true | As in section 1 |

Plus the destructible properties of section 11 (`HitPoints`, `CanDamage`, `NeverDestroy` and so on; for most active classes `NeverDestroy` is true, for `Breakable` false).

**States.** Off, PowerOn (moving to the on pose), On, PowerOff (moving back), Pause. At load: `StartOn` → enter PowerOn (with its sound and command); else Off silently (no sound, no command, stop time = −10 s so no wait applies).

**Entering a state** (each does everything listed, in order):

- **PowerOn**: if coming from Off, re-read the current position as the off position and set on = off + d̂·`MoveDist` (d̂ the world slide direction), and for rotating ones re-derive the pivot from the current position (so a world model moved by something else still opens correctly); choose the rotate direction away from the activator (section 4); start `PowerOnSound`; run `PowerOnCommand`; move start time = now + `MoveDelay`; move timer τ = 0; update every frame.
- **On**: stop updating if `TriggerOff` or `RemainOn`, else update again after `OnWaitTime` + next_frame (auto-off); start `OnSound`; run `OnCommand`; stop time = now. The `SETPOWERONMULTIPLIER` factor resets to 1.
- **PowerOff**: if coming from On, re-read the current position as the on position and set off = on − d̂·`MoveDist`; start `PowerOffSound`; run `PowerOffCommand`; move start time = now + `MoveDelay`; τ = 0; update every frame.
- **Off**: stop updating; restore the un-flipped rotate direction; start `OffSound`; run `OffCommand`; stop time = now.

The command runs with the world model as sender and the activating object as default target (section 13).

**Each frame**, by state:

- PowerOn: if now < move start time, nothing. Else advance one step toward the on pose (sections 3–5) using duration `PowerOnTime` × multiplier. If the step is refused because a player is in the way (section 6), nothing changes this frame (τ does not advance). Otherwise move and rotate; if the on pose is reached, enter On; τ += dt.
- PowerOff: if now < move start time **+ `MoveDelay`**, nothing (the delay counts twice, Quirk 2). Else the same toward the off pose with `PowerOffTime`; on arrival enter Off; τ += dt.
- On (only reached when auto-off is armed): enter PowerOff.
- Off, Pause: nothing.

**Trigger** (the message `TRIGGER`, an accepted `ON`/`OFF`, or an accepted activation):

1. If locked: start `LockedSound` (one-shot), run `LockedCommand`, stop.
2. Off: if now ≥ stop time + `OffWaitTime`, enter PowerOn; else ignore.
3. PowerOn or PowerOff: **reverse** (section 6 "Reversing"), unless `ForceMove`, or `ForceMoveOn` while powering on, or `ForceMoveOff` while powering off.
4. On: with `TriggerOff`: if now ≥ stop time + `OnWaitTime`, enter PowerOff, else ignore; without `TriggerOff`: re-enter On without sound or command (re-arms the auto-off timer).

**Messages** (all case-insensitive):

| Message | Effect |
|---|---|
| `TRIGGER` | Trigger as above |
| `ON` | Only if Off or PowerOff: the sender becomes the activator, then trigger |
| `OFF` | Only if On or PowerOn: the sender becomes the activator, then trigger |
| `ACTIVATE` | From a character only; bots need `AIActivate`, players `PlayerActivate`; the sender becomes the activator, then trigger |
| `LOCK` / `UNLOCK` | Set / clear locked (also greys the use prompt) |
| `PAUSE` | Only while PowerOn or PowerOff: remember the state and position, enter Pause |
| `RESUME` | From Pause: shift the on, off and original positions by how far something else moved the object meanwhile, return to the remembered state |
| `SETPOWERONMULTIPLIER f` | Set the factor on `PowerOnTime` to f until the next On (it replaces, does not compound; Corrections (2026-10-22) 5) |
| `MAXHITPOINT h` | Set maximum hit points to h, clamp current hit points to it |
| `INIT` | Immediately: state Off, teleport to the original position (the box centre at load, or as last re-read), timers cleared (move start and stop time −10 s, τ 0), sliding on/off positions both = original, rotate direction un-flipped. No sound, no command. **The angles are not reset** (Quirk 3) |

**Use key.** The client shows the prompt and sends the activation when the object is activatable (`PlayerActivate` or `AIActivate`), within the use range of the camera ray (`inventory.md`; `activation_distance`). The server applies `ACTIVATE` as above.

### 3. Sliding motion

World direction d̂ = normalise(R(Q) · `MoveDir`), Q the dropped `Rotation` (section 1); off position = the box centre at load; on = off + d̂ · `MoveDist`. Rate r = `MoveDist` / T, T the state's duration (when T ≤ 0.001, r = `MoveDist`). Each moving frame, with p = clamp(τ / T, 0, 1) (p = 1 when T ≤ 0.001):

```math
s = w(r, p)\,\Delta t,\qquad \vec x_{new} = \begin{cases}\vec x_{target} & s \ge \lVert \vec x_{target}-\vec x\rVert \ \text{or}\ p \ge 1\\ \vec x + s\,\dfrac{\vec x_{target}-\vec x}{\lVert \vec x_{target}-\vec x\rVert} & \text{otherwise}\end{cases}
```

w is the waveform (section 5). Arrival is tested by exact equality with the target, so the step that clamps to the target ends the move. With `MoveDist` = 0 the object never moves but still runs through the states (a pure "switch" with sounds and commands).

### 4. Rotating motion

**Angles.** The object keeps three running angles (pitch a_x, yaw a_y, roll a_z), all 0 at load. Off angles = (0, 0, 0); on angles = `RotationAngles` converted to radians. For each axis the **direction** is chosen once at load: δ = off − on; if δ > 1° the direction is −1; if δ < 1° it is +1; exactly 1° gives 0 (Quirk 4).

**Pose.** The world rotation and position for running angles a are

```math
R_o = R(Q)\,R(a_x, a_y, a_z)\,R(Q)^{-1},\qquad \vec P = R_o\,(\vec P_0 - \vec c) + \vec c
```

where P₀ is the original position (the box centre at load) and c the pivot: the position of the object named by `RotationPoint` if one exists (it must be a `Point`; it is removed from the world once read; another class is used but left alone with a console warning), otherwise the object's **`Pos` property** (not the box centre). So `RotationAngles` are about the object's local axes as its `Rotation` property placed them, and around its editor position.

**Stepping an axis** with initial angle a₀, target a₁, direction s and duration T, at p = clamp(τ/T, 0, 1): if p ≥ 1 the angle snaps to a₁ and the axis is done. Otherwise, with rate ρ = (a₁ − a₀)/T (ρ = a₁ − a₀ when T ≤ 0.001):

```math
a \leftarrow a + w(\rho, p)\,\Delta t\quad\text{clamped so as not to pass } a_1
```

when s > 0 and a < a₁ (or s < 0 and a > a₁); an axis already at or past its target snaps to it and is done; an axis with s = 0 is done. Powering off steps from the on angles to the off angles with direction −s. The move ends when all three axes are done.

**Rotate away** (`RotateAway`, doors' `OpenAway`; needs an activator): when powering on from Off, the game evaluates the box centre for two trial poses, each turning every non-zero axis by **±1 radian** (not by its angle; Quirk 5): trial 1 uses +1 on axes whose on angle is positive and −1 on negative ones, trial 2 the opposite. If trial 1's centre is **closer** to the activator's position than trial 2's, the on angles are negated and the directions flipped for this opening; otherwise the stored ones are used. The flip is undone on entering Off or `INIT`. (The roll sign is taken from the yaw angle, Quirk 5.)

Rotating world models are not predicted on clients (section 17).

### 5. Waveforms

With r the linear rate of the move (u/s or rad/s) and p ∈ [0, 1] its fraction of the duration done:

| `Waveform` | Speed w(r, p) |
|---|---|
| `Linear` | r |
| `Sine` | (π/2)·r·sin(πp) |
| `SlowOff` | (π/2)·r·cos(πp/2) − 0.001·r |
| `SlowOn` | (π/2)·r·sin(πp/2) + 0.001·r |

Over a whole move each integrates to about r·T (`SlowOn` and `SlowOff` ±0.1 %). A `Sine` move's first frame (p = 0) has speed 0. Unknown names keep the default (`Linear`). Keyframers use the same names with their own constants (section 9).

### 6. Blocking, pushing and crushing

**Server.** Before each step of a powering active world model (unless `ForceMove`, or `ForceMoveOn` while powering on, `ForceMoveOff` while powering off), the game asks which objects the world model would overlap at the new pose. If any **player** is among them, the step is refused for this frame (nothing moves, τ does not advance): the door waits. For rotating ones, a player overlapped at the new pose is ignored when the activator and the new pose is farther from the player than the current pose (the door swings away from them). Bots, bodies, props and pickups never block this test.

Then the engine moves the world model. All objects have the same blocking priority, so **the engine never pushes anything** out of a moving world model's way: when a solid world model's move would intersect a solid object (player, bot, solid prop), the move is undone and the world model stays where it was this frame; the game sees "did not arrive" and also does not advance τ. So even a `ForceMove` door stops while a bot or a player stands in it, and goes on once they leave.

**Crush.** The engine's crush notification is only raised when something is actually pushed, which never happens; and the game's crush handler deals no damage anyway (it returns before applying any). **Nothing in the game takes crush damage from a world model.** Players are only hurt by world models through volumes and explosions.

**Reversing** (a trigger while moving): unless forced, if there is an activator, test the pose **one frame's step** along the reversed move taken at p = 0.1 of the opposite state's duration (Corrections (2026-10-22) 1); if the activator would overlap the world model there, the trigger is ignored. Otherwise enter the opposite state (PowerOff from PowerOn and vice versa): positions are re-read as in section 2, τ restarts at 0, so a door reversed half-way takes its **full** duration's speed profile again from where it is (a linear door simply moves back at the same speed).

**Client.** The local player is pushed and carried on the client (section 10); this is what keeps a player from being "inside" a door that the server let move.

### 7. Doors and switches

`SlidingDoor` and `RotatingDoor` are active world models with door names for the same properties and different defaults:

| Door property | Same as | Door default |
|---|---|---|
| `PlayerActivate`, `AIActivate` | — | true, true |
| `StartOpen` | `StartOn` | false |
| `TriggerClose` | `TriggerOff` | true |
| `RemainOpen` | `RemainOn` | true |
| `OpenAway` | `RotateAway` | true |
| `ActivateType` | — | `OpenClose` |
| `OpeningSound`, `OpenSound`, `ClosingSound`, `ClosedSound` | `PowerOnSound`, `OnSound`, `PowerOffSound`, `OffSound` | empty |
| `OpeningCommand`, `OpenCommand`, `ClosingCommand`, `ClosedCommand` | `PowerOnCommand`, `OnCommand`, `PowerOffCommand`, `OffCommand` | empty |
| `OpeningTime`, `ClosingTime` | `PowerOnTime`, `PowerOffTime` | 1, 0 (= opening time) |
| `OpenWaitTime`, `CloseWaitTime` | `OnWaitTime`, `OffWaitTime` | 0, 0 |
| `MoveDir`, `MoveDist` | — | (0, 1, 0), 64 |
| `RotationAngles` | — | (0, 90, 0) |
| `Mass` | — | 10000 (no effect on motion) |
| `NeverDestroy` | — | true |
| `DoorLink` | — | empty: name of another door |

- **Door link**: whenever a trigger (any `TRIGGER`, accepted `ON`/`OFF` or activation) leaves a door that is not locked in PowerOn, and its activator is not a bot, the linked door is told: it takes the same activator and, if it is Off or PowerOff, is triggered (without passing the link on). This includes a closing door reversed into opening and doors triggered by map commands. The link is the first object of that name, found once after load. So double doors open together; closing is not linked (Corrections (2026-10-22) 2).
- `TRIGGERCLOSE`: if On or PowerOn and now > stop time + `OpenWaitTime`, enter PowerOff (no lock check, no reversal test).
- `LOCK`/`UNLOCK` on a door also makes bots forget their paths (`bots.md`, doors).
- `RotatingSwitch` (and the other switch classes, none in stock maps) is an active world model with `PlayerActivate` and `AIActivate` true, `RotateAway` false, `MoveDir` (0, 0, 1), `MoveDist` 8, `RotationAngles` (0, 90, 0).
- `Breakable` (section 11) is a static active world model: it never moves.

Census (all 105 maps): `RotatingDoor` 392 objects in 45 maps, `SlidingDoor` 345 in 39, `RotatingWorldModel` 28 in 7, `SlidingWorldModel` 30 in 2, `SpinningWorldModel` 35 in 16, `RotatingSwitch` 7 in 4, `Breakable` 1087 in 73, `WorldModel` 3256 in 100. No map has `Door`, `Switch`, `SlidingSwitch`, `ActiveWorldModel` or a plain `Prop`.

### 8. Spinning world models

`SpinningWorldModel` turns continuously. Defaults: `RotationAngles` (0, 5, 0), `PowerOnTime` 100, `PowerOffTime` 0 (= `PowerOnTime`), `BoxPhysics` true, `RemainOn` true, `RotateAway` false.

- Each non-zero component of `RotationAngles` is the **time in seconds for one full turn** about that local axis: final angular speed ω = 2π / `RotationAngles` (rad/s). The on angles are then set to 359.9° per such axis (used only to report a fraction for animated lightmaps).
- **Spin-up** (PowerOn) and **spin-down** (PowerOff) change the angular speed, not the angle: each frame, with p = (now − move start)/T (T the state's duration), ω_axis += w(ω_final/T, p)·dt·s (spin-up) or −= (spin-down), clamped to [0, ω_final]; done when every axis reached its end speed, or as soon as p > 1. With T = 0 the change is immediate (p and the rate are infinite).
- **Every frame** while spinning up, on, or spinning down, each angle advances by ω·Δt where Δt = now − time of the last spin frame, the pose is set as in section 4 (pivot `Pos` or `RotationPoint`), and angles are wrapped into (−2π, 2π] by subtracting or adding 2π once.
- If a player is in the way at the new pose (and not `ForceMove`), the angles are kept, the spin does not advance this frame, and the move start time is pushed back by dt (the spin-up profile waits).
- A negative component gives a negative final speed, which the clamp turns into instant full (negative) speed.

Stock maps: all 35 have `StartOn` true, `PowerOnTime` 0 and `PowerOffTime` 0, so they spin at full speed from load; `RotationAngles` values seen: (1, 0, 0) ×14, (0, 5, 0) ×11, (0, 7, 0) ×3, (0, 2, 0) ×2, (0, 30, 0) ×2, (0, 6, 0) ×2, (0, 15, 0) ×1; one is non-solid.

### 9. Keyframers and keys

A `KeyFramer` moves the objects named by its `ObjectName` (several names separated by `;`; every object of each name) along its keys.

**Properties**: `ObjectName`, `BaseKeyName` (key name prefix), `KeyDataIndex` (real, hidden: the blind-data chunk index, truncated; negative = none), `ExternalKeyFile` (unused), `PushObjects` (false), `StartActive` (false), `StartPaused` (false), `Looping` (false), `AlignToPath` (false), `IgnoreOffsets` (false), `TargetName`, `TargetOffset` (vector), `ActiveSound`, `SoundRadius` (1000), `TotalPathTime` (0), `Wavetype` (`Linear`). The keyframer's own position and rotation play no part.

**Where the keys are.** Keys are not objects at run time (the editor's `Key` objects are compiled out). Each keyframer's keys are one blind-data chunk of id 0x6AAF0884 (`world_dat.md`, Blind object data), chunk number = `KeyDataIndex` truncated (stock values 0.1, 1.1, …, so index 0, 1, …). Chunk layout, little-endian:

| Size | Type | Meaning |
|---|---|---|
| 4 | u32 | Key count N |
| … | key × N | Keys, in path order |

Key:

| Size | Type | Meaning |
|---|---|---|
| 2 | u16 | Control bits: bit 0 previous tangent present, bit 1 next tangent present |
| 1 | u8 | Sound name length S |
| 1 | u8 | Command length C |
| 12 | vec3 | Position (world) |
| 12 | 3 × f32 | Pitch, yaw, roll (radians): the key's rotation R(p, y, r) |
| 4 | f32 | Time stamp: seconds after the **previous** key (the first key's is its start time) |
| 4 | f32 | Sound radius |
| S | bytes | Sound file name (no terminator) |
| C | bytes | Command string (no terminator) |
| 12 | vec3 | Previous tangent, **relative to the key position** (only with bit 0) |
| 12 | vec3 | Next tangent, relative (only with bit 1) |

A key's absolute time is the running sum of time stamps. Keys are **named** `BaseKeyName` followed by the key index written as `0` for index 0, two digits for 1–9 (`01` … `09`) and plain decimal from 10 (`10`, `11`, …); `GOTO` and `MOVETO` find keys by these names (prefix compared without case, the number exactly; at most 10 characters after the prefix).

**Offsets.** On the keyframer's first update (one frame after load, so all objects exist) every controlled object's offset from key 0 is recorded: position offset = object position − key 0 position, and a rotation offset taken **component-wise on the quaternions** (object quaternion minus key 0 quaternion, Quirk 7). With `IgnoreOffsets` the objects are put exactly on the path with the key rotations.

**Segment and position.** The current segment runs from key A to key B (the key just passed and the next one in the direction of travel). The segment is a cubic Bézier when A has a next tangent and B a previous tangent, with control points A, A + next(A), B + prev(B), B; otherwise a straight line.

- **Time mode** (`TotalPathTime` ≤ 0): with t the keyframer's clock, k = (t − t_A)/(t_B − t_A) (k = 0 when |t_B − t_A| < 0.001), and the position is the segment point at k. Objects are only updated while t_A ≤ t ≤ t_B.
- **Velocity mode** (`TotalPathTime` > 0): path length L = sum of segment lengths (Bézier segments by arc length, Open question 7), v = L / `TotalPathTime`. Each frame, with q = clamp(t / `TotalPathTime`, 0, 1), the distance d = W(v, q)·dt is added: k += d / ℓ (ℓ the segment's length; ℓ ≤ ε → k = 1). On a Bézier segment k is further corrected by the ratio of the requested distance to the arc length the step actually covers, so the speed along the curve stays near d/dt. When k ≥ 1 or q ≥ 1 the object is placed exactly on B ("at key") and k restarts from its overshoot.
- **Rotation**: at a key, the rotation is the arrival key's; between keys, the spherical interpolation of A's and B's rotations at k. `AlignToPath` instead faces the direction of this frame's motion (reversed when travelling backward) with up +Y, then pitches about its right axis and rolls about its forward axis by the keys' pitch and roll interpolated linearly at k. With a target (`TargetName`, or `TARGET`), the objects face the target's position + `TargetOffset` instead, with up +Y, and the rotation offset is not added.
- **Placing**: position = path point + offset (unless `IgnoreOffsets`); with `PushObjects` the object is moved through the world (collisions as in section 6), otherwise teleported. The rotation is applied as a rotation (interpolated on clients) except after `GOTO`, which snaps.

Keyframer waveform W(v, q): `Linear` v; `Sine` (π/2)·v·sin(πq); `SlowOff` (π/2)·v·cos(πq/2); `SlowOn` (π/2)·v·sin(πq/2) + 0.003·v. Only velocity mode uses it.

**Keys passing.** Each frame, before moving, every key the clock (time mode) or the position (velocity mode: the current position equals the next key's exactly) has reached is processed in order: the segment advances; the key's sound plays at the key's position (radius from the key, one-shot); its command runs with the keyframer as sender and default target; and if it is the `MOVETO` destination, the keyframer pauses and runs the destination command once.

**End of the keys.** When the last key (first key, backward) has been passed, the keyframer goes back to the start of the list (clock 0 and key 0 forward; end time and last key backward). Without `Looping` it then stops after this frame (finished) and the objects stay on the last key. With `Looping` the next frame jumps to the start (a teleport for velocity mode: the position is reset to key 0, Quirk 8).

**Clock.** After each frame's move, t += dt forward, −= dt backward.

**Messages**: `ON` (go active; before the first update, just "start active"), `OFF`, `PAUSE` (stop, keep place), `RESUME` (go on from where it was), `FORWARD`, `BACKWARD` / `REVERSE`, `TOGGLEDIR` (change direction; the segment's next key becomes the one behind; k is fixed up at the ends), `GOTO <key>` (resume, jump to the key: clock to its time, objects snapped there, the key's sound and command run), `MOVETO <key> [command]` (resume and travel to the key, reversing if it lies behind; on arrival pause and run the command; if already exactly at it, just process it), `TARGET <name>`, `CLEARTARGET`, `TARGETOFFSET x y z`. `PAUSE`, `GOTO`, `MOVETO`, `TARGET`, `CLEARTARGET` and `TARGETOFFSET` received before the first update are queued and run right after it. Going active from Off resets the clock and position to the start of the direction of travel; `RESUME` does not. Each activation and deactivation also sends every client a "cinematic camera" flag (off when active), used by the end-of-round camera.

**Active sound**: `ActiveSound` loops attached to the first controlled object while active, radius `SoundRadius`, stopped when inactive.

**Census.** 31 keyframers in 10 maps. Every one controls either `Camera0` (7: end-of-round and boss cut cameras) or a `SND_Water*` `SoundFX` (24, in DARKHILLS, DARKTHANDARK and HGOILRIG, 8 each). **No stock keyframer moves a world model**; `IsKeyframed` world models exist only as targets of doors' own motion. All are inactive at load, not looping, `PushObjects` false; `TotalPathTime` 90 (24), 0 (4) or 2 (3); `Wavetype` `Linear` (24) or `Sine` (7).

### 10. Riders

**Server.** When the engine moves an object, objects standing on it (the engine's standing-on link, set when a mover lands on it) are carried by the same displacement: each carried object is moved to its new place minus 0.1 u in Y (to re-detect standing), up to `carried_max`; one that no longer touches the mover afterwards stops riding. World models never follow what they stand on. Rotations carry riders by rotating their positions about the pivot. Players' server positions, however, come from their clients' movement (`movement.md`), so what matters for a player is the client.

**Client (local player).** Every time a server update moves or rotates a **solid** world model on the client (and it is not a teleport), the client moves that world model with only its own player as the object to push: the player is pushed out of the way and, if standing on it, carried by the same displacement (rotation: about the world model's transform). The displacement is added to the player's next movement report as "pushed by other objects". Before each of the player's movement ticks the client puts every active world model (doors, sliding, rotating, spinning; flagged by the server as such) at its interpolated position and rotation for the server time the player is seeing, runs the tick, and restores them (`movement.md` Per-tick order step 1). A player standing on a sliding platform therefore moves with it on their own screen at the server's update rate.

Nothing in the stock maps carries players except doors and the rising sea's volumes (section 9 census).

### 11. Breakables

`Breakable` is a static active world model with the destructible aggregate. The same aggregate gives hit points to every world-model class (`WorldModel`, doors…), but only breakables default to `NeverDestroy` false.

**Properties** (destructible; defaults as declared, `Breakable` overrides in brackets): `HitPoints` 100, `MaxHitPoints` 100, `Armor` 100 [0], `MaxArmor` 100, `Mass` 2000 (blocking priority only), `CanDamage` true, `CanPlayerDamage` true, `CanAIDamage` true, `CanHeal` true, `CanRepair` true, `NeverDestroy` false, `IgnoreDamageType` `UNSPECIFIED`, `DamageCommand`, `DamageTriggerCounter` 0, `DamageTriggerNumSends` 1, `DamagePercentCommand` with `DamagePercentForCommand` 0.5, `DamageIntervalCommand1..3` with `DamageIntervalForCommand1..3` 0.5, `HitPointCommand1..3` with `MinHitPoints1..3` / `MaxHitPoints1..3` (fractions), `DeathCommand`, `PlayerDeathCommand`, `PlayerDamageCommand`, `DamagerMessage`, `KillerMessage`, `CoopScore` false; model aggregate: `DebrisType` (`None`), `CreateExplosion` false, `WeaponId` 0, `FireAlongForward` false, `DamageFactor` 1, `Spawn` (an object description), `SurfaceOverride`, `RemoveOnDeath` true.

**What damages them.** Any damage message: bullets that hit the world model (solid or ray-hit, `world_dat.md`), blast damage (`grenades.md` section 9: breakables are in the "any other object" group of explode damage, with the 100 u dims allowance), map `Explosion` objects, `DESTROY`-style infinite damage. The amount is the incoming damage, multiplied only by the attacker's mutation-box and set-item damage factors when the attacker is a player (`game_modes.md` S12, `gear_perks.md`) and by the global AI damage factor for bots. Armour, gear, hit locations and headshots do not apply to world models. Damage is ignored when: the world model is already dead; the type is in its can't-damage set or equals `IgnoreDamageType`; the attacker is a player and `CanPlayerDamage` is false; `CanDamage` is false; the type is a heal (adds hit points up to the maximum instead, if `CanHeal`); the mission is in its roulette phase. Projectiles count as their shooter. `CanAIDamage` has no effect on world models (it only scales damage between bots).

**Each damage**: hit points −= damage (≥ 0), then the damage commands: `DamageCommand` (and `PlayerDamageCommand` for a player attacker) once the `DamageTriggerCounter` is used up (it counts damage events down), at most `DamageTriggerNumSends` times; `DamagePercentCommand` every time hit points < max × `DamagePercentForCommand`; each interval command every time hit points cross the next multiple of its interval below the maximum; a `HitPointCommand` when the hit-point fraction enters its [min, max] band (once per entry); `DamagerMessage` sent to the attacker.

**Death**: when hit points ≤ 1, `NeverDestroy` is false and this damage was > 0: hit points 0, dead; `DeathCommand` runs (sender and target the breakable), `PlayerDeathCommand` too for a player killer, `KillerMessage` goes to the killer. Then, on the same damage message:

1. the `Spawn` description is spawned at the breakable's position and rotation;
2. prop debris of `DebrisType` is thrown from the position along the death direction (`effects_sounds.md` owns the debris look);
3. every `WorldModelDebris` named `<Name>Debris0`, `<Name>Debris1`, … (consecutive numbers until one is missing) is launched with velocity (U(−200, 200), U(100, 300), U(−200, 200)) u/s and rotation periods U(−1, 1) per axis (one stock map has one);
4. with `CreateExplosion`, weapon `WeaponId` is fired from the position, down (0, −1, 0) with zero lifetime (in place), or along the forward vector with `FireAlongForward`, damage × `DamageFactor`;
5. `RemoveOnDeath` true: the object is removed. False: it becomes invisible, non-solid and not ray-hit and loses its attachments, but stays (stock HGOILRIG drums and crates do this and use their `DeathCommand` to show a client effect and fire a separate `Explosion` object; Test case 10).

A dead breakable takes no more damage of any kind. `FIRE [target]` (alive only) fires the explosion weapon, aimed at the named object if given. `HIDDEN 1` saves and turns off damage (and sets never-destroy); `HIDDEN 0` restores them.

**Round reset.** The stock maps reset breakables with map commands, not by code: the controller's `ObjectStartCommand` (`msg memorize trigger;`) fires a trigger that sends `memorize` to every breakable; each mode's level start trigger then sends `(visible 1)`, `(solid 1)` and `remember`. `MEMORIZE` stores the object's own saved state (the world-model and active-world-model state: positions, angles, state, timers, flags of that layer) and `REMEMBER` reloads it. The destructible's hit points and its dead mark are **not** part of that state. So a breakable broken in one round comes back visible and solid in the next but, being dead, cannot be broken again for the rest of the map (Quirk 9, Open question 5).

### 12. Triggers and other event objects

**`Trigger`.** A box of half-extents `Dims` (default 16, 16, 16) at its position, non-solid, touch-notifying, ignored by projectiles. Properties: `NumberOfActivations` 1, `SendDelay` 0, `TriggerDelay` 0, `Command1` … `Command10`, `TriggerTouch` false with `CommandTouch`, `PlayerTriggerable` true, `AITriggerable` false, `AITriggerName`, `BodyTriggerable` false, `BodyTriggerName`, `WeightedTrigger` false with `Message1Weight` 0.5, `TimedTrigger` false with `MinTriggerTime` 0 and `MaxTriggerTime` 10, `ActivationCount` 1, `Locked` false, `ActivationSound` with `SoundRadius` 200, `AttachToObject`, `PlayerOnceActivate` false, `HUDLookAtDist` −1, `HUDAlwaysOnDist` −1, `TriggerType`.

- **Who fires it by touch**: a player (if `PlayerTriggerable`) or a body (only if `BodyTriggerable`, and only the body named `BodyTriggerName` when set). **Bots never fire a trigger by touch** (`AITriggerable` is read but not used). A touch is the engine's touch notification while a player's server-side box moves into or within the box; there is **no leave event** and no "while inside" event.
- **Firing** (touch or the `TRIGGER` message; the sender of the message counts as the toucher): ignored if now < last fire attempt + `TriggerDelay` (the attempt time is recorded even when it then does nothing); the toucher is recorded; if the trigger is idle and not locked it goes "pending" and becomes busy: after `SendDelay` (counted at the trigger's next updates; immediately when `SendDelay` is 0 and `TriggerDelay` is 0) it **activates**. While pending, further touches only replace the recorded toucher (the last toucher before activation is the one used).
- **Activation**: the activation counter goes up; nothing happens until it reaches `ActivationCount` (then it restarts at 0). Then, if `NumberOfActivations` > 0 and the trigger already fired that many times, nothing; otherwise the fire count goes up, `ActivationSound` plays at the trigger (radius `SoundRadius`, high priority), and `Command1` … `Command10` run in order, each with the toucher as sender and the trigger as default target. With `WeightedTrigger` exactly one of `Command1` (probability `Message1Weight`) and `Command2` runs; the others always run. If fired by touch and `TriggerTouch`, `CommandTouch` runs with the toucher as both sender and target. The recorded toucher is cleared.
- **Timed**: with `TimedTrigger` the trigger also fires itself (no toucher) whenever now passes the next time, then picks the next one U(`MinTriggerTime`, `MaxTriggerTime`) later; it checks every 0.1 s. The first time is `MinTriggerTime` after load.
- **`AttachToObject`**: on its first update the trigger attaches itself to the named object (zero offset) and follows it.
- Messages: `TRIGGER`, `LOCK`, `UNLOCK` (unlock also fires the trigger by touch for every character whose position is strictly inside the box), `CLEARPLAYERLIST`.
- `PlayerOnceActivate`: a player who fires such a trigger is put on a list **shared by every trigger of the map** and told so (the client greys its prompt); the server-side refusal only applies while another toucher is pending (Quirk 10). `CLEARPLAYERLIST` empties the shared list.
- Stock use: almost all of the 2,919 triggers in 104 maps are message relays (no touch: placed out of reach, fired by `msg <name> trigger`), such as each mode's `LevelNNStartTrigger` / `LevelNNEndTrigger` (`game_modes.md` S15).

**`CommandTrigger`** (508 in 28 maps): `Locked` false, `Command0` … `Command3`. On `TRIGGER` (if not locked) the four commands run at once with the sender as sender and the object as default target. No box, no delays, no counts. `LOCK`/`UNLOCK` as above.

**`TouchTrigger`** (49 in 2 maps): box `Dims` (16, 16, 16), `Locked` false, `NumberOfActivations` 1 (0 = unlimited), `Command0` … `Command3`. Every player touch (not bodies, not bots) while unlocked and under the activation limit runs the four commands with the player as sender. No delay: a player standing in it fires on every touch notification until the limit.

**`TargetTrigger`** (208 in 38 maps): a `Trigger` used as an objective marker: `CheckTeam` true with `Team` (only that team's marker), `GameType` (only active when it names the running mode), `RadarTexture`, `LargeMapTexture`, `HUDTexture`. `ON`/`OFF` show and hide it on clients' radar, map and HUD (`hud_crosshair.md`; icons and rules: Follow-up (2026-10-31b) 1.6).

**`EventCounter`** (48 in 19 maps): an integer starting at `StartingValue` (0) with up to twelve events `Event<n>Value` (−1 = unused), `Event<n>IncToValCmd`, `Event<n>DecToValCmd`. `INC [n]` / `INCREMENT [n]` and `DEC [n]` / `DECREMENT [n]` step it n times (n < 1 counts as 1); each single step up runs the increment command of every event whose value equals the new count; each step down (never below 0) the decrement command. `LOCK` freezes it. Commands run with the counter as sender and target.

**`StartupCommand`** (97 in 68 maps): runs `Command` once on the first server frame after load (sender and target itself), then removes itself.

**Volumes that fire events**: none. The volume classes (`world_dat.md`) act every physics step on what is inside; `DamageVolume` filters by `TargetTeam`. Team and player filters on event objects exist only in `TargetTrigger` (display), `ActivateTrigger` (`game_modes.md` M10: use by a team) and `PlayerOnceActivate`. Objective triggers (`BombZoneTrigger`, `FlagZoneTrigger`, `EscapeZoneTrigger`, `ActivateTrigger`, `BullionSaveTrigger`) belong to `game_modes.md` and `game_modes_2.md`. `TimerTrigger`, `LightActivateTrigger`, `CaptionTrigger`, `MessageTrigger`, `MissionTrigger`, cameras and `TargetTrigger` markers: Follow-up (2026-10-31b) 1; `BGMTrigger` is `music_ambience.md`'s.

### 13. Command strings

A command string is what map objects store in their command properties, key commands and `msg` arguments. The server's command manager runs it.

**Syntax.**

- Statements are separated by `;` (line breaks inside a property are whitespace). Each statement is a command word followed by arguments separated by whitespace. Parentheses group several words into one argument, and a group may itself contain `;`-separated statements; double quotes also group. The outer parentheses of a group are removed when it is used as an argument.
- Command words (case-insensitive) and their exact argument counts (word included):

| Command | Args | Meaning |
|---|---|---|
| `MSG <targets> <message>` | 3 | Send the message to the target(s) now |
| `VMSG <var> <message>` | 3 | The same, to the object held in an object variable |
| `DELAY <s> <command>` | 3 | Run the command after s seconds |
| `DELAYID <id> <s> <command>` | 4 | The same, with an id for `ABORT` |
| `DELAYVARIABLE <var> <command>` | 3 | Delay by an integer variable's value in seconds |
| `REPEAT <min n> <max n> <min s> <max s> <command>` | 6 | Run it n = U{min..max} times, U(min s, max s) apart, the first after one such wait |
| `REPEATID <id> …` | 7 | The same, with an id |
| `LOOP <min s> <max s> <command>` | 4 | Run it forever, U(min s, max s) apart |
| `LOOPID <id> <min s> <max s> <command>` | 5 | The same, with an id |
| `ABORT <id>` | 2 | Cancel the first pending command with that id (case-insensitive) |
| `RAND <p> <cmd1> <cmd2>` | 4 | cmd1 with probability p (0.001 ≤ p ≤ 1, else an error), else cmd2 |
| `RAND2` … `RAND8 <cmd1> … <cmdN>` | N + 1 | One of N at random, uniformly |
| `INT <var> <value>` | 3 | Declare an integer variable (fails if it exists) |
| `OBJ <var> <name>` | 3 | Declare an object variable holding the named object |
| `SET <var> <value or var>` | 3 | Assign (the value must start with a digit, else it is read as a variable or, for object variables, an object name) |
| `ADD <var> <n>`, `SUB <var> <n>` | 3 | Add, subtract |
| `IF <expression> THEN <commands>` | 4 | If true, run the commands (one command, or a group of commands each in its own parentheses) |
| `WHEN <expression> THEN <commands>` | 4 | As `IF`; if false, store it (up to 32) and run it once the first time a `SET`/`ADD`/`SUB` of one of its variables makes it true |
| `SHOWVAR <var> <0/1>` | 3 | Debug display |
| `LISTCOMMANDS` | 1 | Debug listing |

- Expressions: `(a op b)` with integer operators `==`/`EQUALS`, `!=`/`NOT_EQUALS`, `>`/`GREATER_THAN`, `<`/`LESS_THAN`, `>=`/`GREATER_THAN_OR_EQUAL_TO`, `<=`/`LESS_THAN_OR_EQUAL_TO`, and `AND`/`&&`, `OR`/`||` joining parenthesised sub-expressions (`==` and `!=` also compare object variables). Variables are global to the map; stock maps use them for round counting and the FREEZER radio (Test case 3).
- A statement with the wrong argument count is skipped silently. A statement that **fails** (unknown variable, bad delay or `rand` percentage, …) **stops the rest of that string** (Quirk 11).

**Targets of `MSG`.** The target argument is one name or a group of names. Per name:

- `player`: every player; `ActivePlayer`: the last player who sent any trigger message (a toucher or activator); `OtherPlayers`: every player but that one; `ai`: every bot (and pattern object); `Sender`: the command's sender, if it is a character.
- An empty name: the command's default target object.
- Any other name: **every object with that name** (names are not unique by force), compared as the engine compares names; if none, a lightweight object of that name.
- A `<Class>` prefix before a name is accepted and ignored.
- If the message itself is a valid command (it starts with a command word), it is run as a command with the first object of that name as default target, instead of being sent.

**Delivery.** A message is delivered **immediately and synchronously**: the target handles it before `MSG` returns, and anything it does (its own commands) runs nested, before the rest of the sender's string. A message text with several `;`-separated parts inside one group (`msg X (a; b)`) is sent as separate messages in order. Every object understands the base messages: `VISIBLE 0/1`, `SOLID 0/1` (solid 0 also clears ray-hit; solid 1 also restores ray-hit if the object had it at its first message; Corrections (2026-10-22) 4), `HIDDEN 0/1` (1: not visible, not solid, not ray-hit; 0: restores what the object had at its first message), `REMOVE` (not for players), `MEMORIZE`, `REMEMBER`, `NEXT_UPDATE <s>`; `0`/`1` may be written `FALSE`/`TRUE`. Unknown messages are ignored.

**Delayed commands** sit in one table of 512 slots. Each server frame (before objects update, section "Per-tick order") every pending command's delay is reduced by the **real frame time** (unclamped wall-clock time, not game time; Open question 9); when it reaches 0 or less it runs (as a whole command string, with the sender and default target that were current when it was queued), then repeats, loops or is freed. A delay of 0 does not run at once: it runs at the manager's next walk of its table (the same frame for a command queued by the frame's client packets, the next frame for one queued by an object update; a command queued during the walk into a later slot is reached in that same walk; Corrections (2026-10-22) 13). A command queued while the table is full runs immediately. Commands are run in slot order, so two that fall due in the same frame run in the order of their slots, not of their due times.

**Common messages by class** (besides the base ones): active world models and doors (section 2, 7), keyframers (section 9), triggers (section 12), `SoundFX` (`ON`, `OFF`, `TOGGLE`, `TRIGGER`, `KILL`; `effects_sounds.md`), `LightGroup` (`ON`, `OFF`, `TOGGLE`, `COLOR`; `world_dat.md`), `Explosion` (`ON`/`START`: explode), `Camera` (`ON`, `OFF`, `MOVETO <point>`), mode controllers and objective objects (`game_modes.md` S15).

### 14. Object sounds

- **Active world models**: each transition starts its sound (PowerOn, On, PowerOff, Off, Locked); starting one stops the previous one. All are server 3D sounds with reverb, outer radius `SoundRadius`, inner 0.25 × `SoundRadius`, medium priority, default volume. With `SoundPos` exactly (0, 0, 0) the sound is attached to the object and moves with it; otherwise it plays at position + `SoundPos` (world offset, not rotated) and stays there. With `LoopSounds` the transition sounds loop until the next transition; the locked sound never loops. With radius 0 (a property left at 0) the sound is inaudible.
- **Keyframers**: per-key one-shot sounds at the key position with the key's radius; `ActiveSound` looping on the first object while active.
- **Triggers**: `ActivationSound` one-shot at the trigger.
- **Map sounds**: `SoundFX` objects (`effects_sounds.md` section 11). A `SoundFX` with `PlayAttached` follows its own object: HGOILRIG's eight `SND_Water*` sounds (looping, radius 3000, inner 250) are carried by their keyframers from the sea edge (y = −14) toward the rig (y 946–1010) over 90 s while the sea rises.
- How a server sound reaches and sounds on clients (gain, low-pass, 2R send range) is `effects_sounds.md` section 10–12.

### 15. Spawned props and models

- **`PropType`** (79 in 6 maps): a model prop whose model, skins, scale, flags (visible, solid, gravity, shadow, ray-hit, activatable), hit points (negative: indestructible) and debris come from the prop-type table of the attribute data, named by `Type` (stock: `Travel_Bag` 31, `Auto_Turret` 10, `Dead_Woman`, `Dead_Man01/02`, `SH_Laptop_*`, `SH_Paper`, …). It is moved down to the floor on its first update (`MoveToFloor` true). Destroyed: switches to the type's destroyed model if it has one, else is removed. Messages `ANIM`, `ANIMLOOP`, `ACTIVATE`, `REMOVE`, `DESTROY`. The model files and their animation are `models_ltb.md` / `animation.md`.
- **`Spawner`** (1701 in 25 co-op maps): creates an object from a template (`Target` names a template object, i.e. an object with `Template` true that is not itself created) on `SPAWN`; used for co-op bots and items (`bots.md`).
- **`ObjectSpawnPoint`**: mode item drop points (`game_modes.md` S12).
- **Mode objects**: flags, flag poles, bombs, intel, bullion and the like are created by the mode code (`game_modes.md`, `game_modes_2.md`).
- **`SpecialFX`** objects play a client effect at their position (`effects_sounds.md`); stock breakables pair with one (`FX70Drum02` and so on, effect `EDrumG` / `70x70WdnBox`) turned on by their death command, which draws the debris.
- **`Explosion`** (340 in 45 maps): `ImpactFXName`, `DamageType` `EXPLODE`, `DamageRadius` 200, `MaxDamage` 200, `RemoveWhenDone` true. `ON` or `START` explodes at its position: the effect, and area damage with the same rule as a grenade blast (`grenades.md` section 9) with these radius and damage values. The bomb sites' explosion effects send nothing to `Explosion` objects (`game_modes.md` S15).
- **`WorldModelDebris`** (1 in 1 map): a world-model piece launched by its parent breakable (section 11), falling with gravity and spinning (Follow-up (2026-10-31b) 3: it never lands).

### 16. Geometry that a map lacks: HGOILRIG

**Finding.** HGOILRIG (WORLDS4) has its 16 unlocked (warm-up) `HungerGame` points around (7250, 324, −4100), its 16 locked (round) points all at (4709, 1558, 3076), and its 150 supply drop points over open sea, with nothing under any of them: no physics-BSP polygon except light fixtures (325 polygons of wall lights, field lights and black, y 99..720, in the box x 6900..7600, z −4400..−3700), no render geometry but those lights, no world model but three light-ray brushes, no collision blocker, no `PropType`, `Spawner`, `TeleportPoint` or `PolyGrid`. Its 8 keyframers move only sound emitters. The ground texture its scatter volumes name (`512xterrainsnowed02`) appears nowhere in its geometry, and its scatter volumes have no blind data (`BlindDataIndex` −1).

**Where it came from.** HGOILRIG is a copy of **DARKHILLS** (WORLDS3, the Hunted map) with the terrain and buildings removed and the oil rig and sea added: both have the same world extents ±(14016, 10240, 13184), world offset (−64, 0, 1088) and ambient light, and the same warm-up points, lights, cars, crates, scatter volumes, water-sound keyframers and drop points. In DARKHILLS the warm-up points stand in a wooden barn on snowy terrain (1,677 physics polygons in the same box, including `512xterrainsnowed02` and barn wood), and its 15 round points are spread over the hills at y 1558 (HGOILRIG moved all 16 to one of them). So the decks never existed in HGOILRIG's file, and there is **no other source in the game**: the engine has no terrain or per-map extra geometry (`world_dat.md`), no object of the map supplies a floor there, and the archives hold no other file named for the map (`gms\oilrig_hg.gms` is its mission file). In the reference game a Hunted match on this world starts players over the sea: the spawn's drop to the floor (`movement.md`, Falling) finds nothing, they fall into the `Water` volume (top y −58 at load) whose current (0, 1500, 0) floats them (`movement.md` Volumes), and the sea itself rises 500 u from 90 s into the round (Test case 8). Our conversion is right to show sea there (Open question 3).

**How to find a map's missing floors** (for any map): for each spawn or drop point, look straight down (−Y) for (1) physics-BSP polygons with the solid bit; (2) solid world models, including ones a door, keyframer or `INIT` would move there (their on and off poses), and ones `StartHidden` that a `HIDDEN 0` / `VISIBLE 1` / `SOLID 1` command shows; (3) collision blockers (players only); (4) solid `PropType`/prop models and mode-created objects. If none, the map relies on a volume or on falling, as HGOILRIG does. A map derived from another (same extents and world offset) can be compared with its parent to see what was removed.

### 17. What the server sends and what clients run

- **Server-only**: every state machine, timer, trigger, counter, command, variable, keyframer clock and hit point. Clients never run map logic.
- **Sent as object updates** (engine-level, every frame an object changed, unreliable for movement): position and rotation of every moving world model (doors, sliding, rotating, spinning, keyframed objects, attached volumes and sounds), object flags (visible, solid, ray-hit) changed by messages, removals (broken breakables with `RemoveOnDeath`, triggers that remove themselves, `StartupCommand`). Active world models are flagged as such in their creation message. Sounds are server sounds (`effects_sounds.md`); client effects (debris, explosions, `SpecialFX` on/off) are effect messages.
- **Client motion**: sliding objects are interpolated 100 ms in the past between the server's updates; keyframed objects without `PushObjects` jump to each update (every placement is a teleport); rotating and spinning world models have prediction disabled and **snap** to each received rotation and position (20 Hz steps). The local player is pushed and carried as in section 10. Details: Follow-up (2026-10-31b) 2.
- **Late joiners** get every object as a new object with its **current** position, rotation and flags, so open doors, risen water, hidden or removed breakables and moved cameras are right on arrival. They do not get past one-shot sounds or effects; a looping transition sound already playing reaches them only if the engine re-sends looping sounds to new clients (Open question 10). Trigger state, counters, variables, pending delays and the `PlayerOnceActivate` list live only on the server.
- **Collision on each side**: the server collides with world models at their server pose; the client's own movement uses the interpolated pose for the time it shows (section 10). Bullets are traced on the server against the server pose (`weapon_firing.md`).

## Per-tick order

Server, one frame:

1. Incoming client packets: players' movement reports (server positions move, touch notifications fire triggers and touch triggers synchronously, with all their nested commands), use-key activations (doors' `ACTIVATE`), fire.
2. Server sounds update.
3. Game shell update: the command manager first (every pending delayed, repeating and looping command whose delay ran out runs, in slot order), then the AI stimulus manager, then the mission manager (phases, which may fire `LevelNN*Trigger` and controller commands).
4. Objects, in the server's object list order (roughly creation order): each object whose update countdown expires runs its update (active world models step their motion, keyframers process keys then move their objects, timed triggers, `StartupCommand`), then its physics. Damage and its death commands run whenever damage arrives (during steps 1 and 4).
5. Changed objects are sent to clients.

So a trigger fired by a player's movement in step 1 starts a door moving in step 4 of the same frame (the door's first step is that frame if it has no delay); a `delay 0` command queued in step 1 runs in step 3 of the same frame, one queued in step 4 in step 3 of the next frame (Corrections (2026-10-22) 13); a door blocked by a player in step 4 stays put for the whole frame. A keyframer's first update after load only builds its key list; its objects move from the second update. A player standing on a platform is moved by the server mover in step 4 and by their own client between frames (section 10).

Client, per movement tick: put active world models at their pose for the shown server time, move the local player (`movement.md`), restore; when a world-model update arrives, push or carry the local player.

## Edge cases

- **A door blocked by a player** stops each frame the player overlaps its next pose and continues the same profile afterwards; the open or close takes longer by the blocked time. A `ForceMove` door skips the game's player test but is still stopped by the engine. Neither hurts anyone.
- **A door re-triggered while moving** reverses (unless forced or the activator stands in the reversed path), restarting its timer, so a `Sine` door reverses from rest speed.
- **`Locked` door used**: locked sound and command only; door links are not told.
- **Auto-close** (`RemainOpen` false, `TriggerClose` false): the door closes `OpenWaitTime` (+ one frame) after it finished opening; triggering it while open re-arms that wait.
- **A keyframer with one key**: the key is processed on the first active frame, then the list ends; no segment exists, so the objects never move; with `Looping` the key's sound and command repeat every frame.
- **A keyframer whose keys have zero time stamps** in time mode: every key is passed on the first active frame; objects are not moved (no segment has t_A ≤ t ≤ t_B with a range), so time-mode keyframers need non-zero stamps; velocity mode ignores the stamps.
- **A keyframer whose object is removed**: it is dropped from the list; with none left the keyframer cannot go active.
- **`GOTO` before the first update** is queued and run right after the key list is built.
- **A breakable hit by a grenade**: it gets the blast's explode damage by distance (`grenades.md` section 9) like any object; with 10 HP (stock drums) a frag within range breaks it; its death command can chain to an `Explosion` object that damages players around it.
- **A breakable destroyed by several pellets in one shot**: the first damage message that takes it to ≤ 1 HP kills it; later pellets of the same shot find it dead and do nothing.
- **A trigger touched by two players during its `SendDelay`**: one activation, credited to the last toucher.
- **`TriggerDelay` with a touch every frame**: each touch inside the delay is refused but also does not extend it.
- **A command string longer than 256 bytes** queued with `delay` is cut at 255 bytes.
- **`SET x -1`**: fails (a value must start with a digit), stopping the string.
- **Rotation exactly 1° away from the start on an axis** gets direction 0 and never turns on that axis (Quirk 4).
- **A world model with `Pos` far from its brush** rotates about `Pos`, swinging widely.

## Quirks

1. **Pos overwritten, kept as pivot.** World-model objects start at their BSP centre, not at `Pos`; `Pos` is only the default rotation pivot. Keep.
2. **Double delay when closing.** Powering off waits `MoveDelay` twice (the start time already includes it and the test adds it again); powering on waits it once. Keep (stock doors use 0).
3. **`INIT` keeps the angles.** A rotating world model sent `INIT` teleports to its original position but keeps its current running angles, so a half-open rotating door is left rotated. Keep; stock maps send `INIT` only to HGOILRIG's sliding sea.
4. **The 1° direction test** uses "greater than +1°" and "less than +1°": any offset below 1° (including 0 and negative small ones) gives +1, exactly 1° gives 0. Keep.
5. **Rotate-away trial by 1 radian, roll from yaw.** The away test turns each axis by ±1 rad instead of its angle, and the roll axis's sign is taken from the yaw angle. Keep; it only matters for doors whose rotation is not purely yaw.
6. **The game's player test uses the unconjugated rotation** for rotating world models (the running angles without the `Rotation` frame), so for a rotated door the "would it hit a player" test is done at a slightly wrong pose; the engine's own collision is right. Keep (only changes which frames wait).
7. **Quaternion offsets subtracted component-wise** in keyframers. Without `IgnoreOffsets`, an object not aligned with key 0 gets a non-unit rotation added. Stock keyframers either ignore offsets (cameras) or move sound emitters, where rotation does not matter. Keep.
8. **A looping velocity-mode keyframer teleports back to key 0** at the end of the list instead of continuing.
9. **Breakables stay dead across rounds.** `REMEMBER` restores the world-model layer but not hit points, so a breakable broken in round 1 reappears from round 2 on but can no longer break. Keep until checked (Open question 5).
10. **`PlayerOnceActivate` barely refuses.** The server only refuses a listed player while an earlier toucher is still pending; the effective block is the client's greyed prompt. The list is shared by all triggers.
11. **A failing statement stops its string.** Map authors rely on statement order; keep.
12. **Bots never fire triggers by touch.**
13. **Delays count real time.** A delayed command fires after its delay in wall-clock time even when the server's frame time is clamped (long frames), and command delays are not paused by the game.

## Test cases

Map numbers: Combat Arms: the Classic, Steam build id 25595979 (`COMBAT ARMS: THE CLASSIC`, app 1263550), all 105 world files (WORLDS, WORLDS4 and WORLDS5 decrypted). Synthetic cases use dt = 0.05 s unless said. Positions in file axes; compare with 1e-3 relative tolerance unless said.

**Objects and placement**

1. **Census (105 maps).** Objects / maps: `KeyFramer` 31 / 10, `RotatingDoor` 392 / 45, `SlidingDoor` 345 / 39, `RotatingWorldModel` 28 / 7, `SlidingWorldModel` 30 / 2, `SpinningWorldModel` 35 / 16, `RotatingSwitch` 7 / 4, `Breakable` 1087 / 73, `WorldModel` 3256 / 100, `WorldModelDebris` 1 / 1, `Trigger` 2919 / 104, `CommandTrigger` 508 / 28, `TouchTrigger` 49 / 2, `TargetTrigger` 208 / 38, `TimerTrigger` 17 / 10, `EventCounter` 48 / 19, `StartupCommand` 97 / 68, `Explosion` 340 / 45, `PropType` 79 / 6, `Spawner` 1701 / 25, `SoundFX` 1637 / 88, `Camera` 104 / 100. No `Door`, `Switch`, `SlidingSwitch`, `ActiveWorldModel`, `Prop`.
2. **Per map.** WORLDS2\WAREHOUSE.DAT: `Breakable` 12, `WorldModel` 41, `Trigger` 11, `Explosion` 6, `Camera` 1, no door, no keyframer. WORLDS3\FREEZER.DAT: `RotatingDoor` 1, `SlidingDoor` 2, `WorldModel` 6, `Trigger` 18, `StartupCommand` 1, `SoundFX` 10. WORLDS3\SNOWSTORM.DAT: `Breakable` 45, `SlidingDoor` 15, `RotatingDoor` 1, `RotatingWorldModel` 2, `KeyFramer` 1, `WorldModel` 48, `EventCounter` 3, `Trigger` 21, `Spawner` 45, `PropType` 2. WORLDS4\HGOILRIG.DAT: `KeyFramer` 8 (blind chunks 8, each 124 bytes, id 0x6AAF0884), `SpinningWorldModel` 6, `SlidingDoor` 5, `RotatingDoor` 1, `Breakable` 11, `WorldModel` 14, `Trigger` 25, `CommandTrigger` 2.
3. **FREEZER `CMD_A_Radio`** (`RotatingDoor`, world model 3). Decoded: `Pos` (490, 80, 369.367), `Rotation` (0, π, 0), `RotationPoint` empty, `RotationAngles` (0, 1, 0), `OpeningTime` 2, `ClosingTime` 0, `MoveDelay` 0, `OpenWaitTime` 0, `CloseWaitTime` 0, `PlayerActivate` true, `StartOpen` false, `TriggerClose` true, `RemainOpen` true, `ForceMove` false, `Locked` false, `OpenAway` true, `Waveform` `Linear`, `ActivateType` `OnOff`, `Solid` and `Rayhit` true, `AWMType` 2, `OpeningCommand` shows `RadioA_On` and plays `SoundFX_A_Search`, `OpenCommand` adds 1 to variable `RadioA` and plays `SoundFX_A_Play1..4` by its value (resetting it to 0 at 4), `ClosingCommand` kills the four sounds and swaps the visibility back. BSP centre (508.5, 66.5, 367.8677). At load the object is at the BSP centre with identity rotation; pivot = `Pos`; closing time = 2 s.
   - Activated by a player at (520, 66.5, 400): trial +1 rad centre (498.734, 66.5, 352.990) is farther than trial −1 rad (501.257, 66.5, 384.124) → normal direction: after 1.0 s the yaw is 0.5° and the centre (508.486, 66.5, 367.706); after 2.0 s (40 linear steps) yaw 1° and centre (508.471, 66.5, 367.545); state On; `OpenCommand` runs once.
   - Activated by a player at (520, 66.5, 340): trial +1 rad is closer → reversed: after 2.0 s yaw −1°, centre (508.523, 66.5, 368.191).
   - Used again while On: closes over 2 s back to yaw 0; `ClosingCommand` at the start of closing.
4. **FREEZER `Chk_mouth`** (`SlidingDoor`): `Rotation` (0, −π, −π), `MoveDir` (0, 1, 0), `MoveDist` 32, `OpeningTime` 0.2, `ClosingTime` 0.2, `PlayerActivate` false, `ActivateType` `OpenClose`, `OpeningSound` `SND\Freezer\Crazy_Chicken.wav`, `AWMType` 1; BSP centre (126.708, 271.6895, 2042.4663). The world slide direction is R(0, −π, −π)·(0, 1, 0) = (0, −1, 0) (within 1e-6): on `TRIGGER` the beak moves **down** 8 u per frame (160 u/s), reaching (126.708, 239.6895, 2042.4663) on the 4th moving frame, state On; the opening sound starts on the trigger frame, attached, radius 1000, inner 250. `Chk_wing` (`Rotation` (π/4, 0, 0), `MoveDist` 16, `ForceMove` true) slides along (0, 0.7071, 0.7071): open at centre + (0, 11.314, 11.314).
5. **Sliding waveforms (synthetic).** `MoveDist` 100, `PowerOnTime` 1, dt 0.05. `Linear`: 5 u per frame, on after 20 moving frames. `Sine`: frame n (n = 0, 1, …) moves (π/2)·100·sin(πn/20)·0.05; frame 0 moves 0; after 10 frames (n = 0..9) the distance is 7.854 × Σ sin(πn/20) = 7.854 × 5.8531 = 45.97 u; the frame with p ≥ 1 (n = 20) snaps to the end if not there yet.
6. **Rotating door, reversed half-way (synthetic).** `RotationAngles` (0, 90, 0), `OpeningTime` 1, `ClosingTime` 1, `Linear`, pivot at the hinge, no activator: trigger at t = 0 → after 10 moving frames yaw 45°; `TRIGGER` → PowerOff from 45°: rate is (0 − 90°)/1 s, so it reaches 0° after 10 more frames; state Off; `ClosedCommand` runs once; total 20 moving frames.
7. **Blocked door (synthetic).** A sliding door (`MoveDist` 100, 1 s, `Linear`) whose next pose would overlap a standing player from its 6th moving frame on: it stops at 25 u; τ stays at 0.25 s; the player takes no damage; when the player steps out, it goes on at 5 u per frame and opens 100 u in total, 20 moving frames after the trigger plus the blocked frames. The same with a bot in the way: the engine refuses the move; the door also waits.
8. **HGOILRIG's rising sea.** `SlidingWater` (`SlidingDoor`, `Solid` false, `Rayhit` true, `BlendMode` `Translucent`, `IsKeyframed` true, `Attachments` `PoisonGas00;Water;`, `MoveDir` (0, 1, 0), `MoveDist` 500, `OpeningTime` 120, `ClosingTime` 0.1, `Waveform` `SlowOn`, `ForceMove` true, `OpeningCommand` triggers `CMD_SNDWaterON` and turns `SoundFX_Siren` on, `OpenCommand` triggers `CMD_SNDWaterOFF` and turns `SoundFX_WaterEnd` on; BSP centre (560.708, 1.0, −1951.5889)). Level 4 start trigger runs `delayid Wup 90 (msg SlidingWater on);` → 90 s later the sea starts rising at 0.0042 u/s, 4.632 u/s at half time (p = 0.5), and is 500 u up after about 120 s (the `SlowOn` additive term makes it arrive a few frames early); the `Water` and `PoisonGas00` volumes keep their offsets to it (the water's top from y −58 to 442). The level 4 end trigger sends `msg SlidingWater init;` and `abort Wup;`: the sea jumps back to y 1.0 (centre) at once with no sound and no command; an `abort` after the delay ran finds nothing.
9. **Spinning fan.** HGOILRIG `RotatingWorldModel0` (`SpinningWorldModel`, `RotationAngles` (1, 0, 0), `StartOn` true, `PowerOnTime` 0, `Rotation` (0, 0, 0), `Pos` (1091.943, 638.410, −809.688), BSP centre (1091.9429, 638.4099, −809.6885)): full speed 2π rad/s about X from its first update; its pitch after t s is 2πt wrapped into (−2π, 2π]; its centre does not move (pivot = centre to 1e-3). With a player standing in its sweep, it does not turn on frames where the new pose would overlap them.
10. **Breakable drum.** HGOILRIG `BRK70Drum02` (`Breakable`, `HitPoints` 10, `MaxHitPoints` 10, `RemoveOnDeath` false, `DebrisType` `None`, `Solid` and `Rayhit` true, `DeathCommand` `msg FX70Drum02 on; msg DExp02 on; msg Level19GameControl (Randommapitem Spawner141_Box);`). A bullet of 8 damage → 2 HP, alive. A second → hit points 0 (≤ 1): dead; `FX70Drum02` (effect `EDrumG`) on; `DExp02` (`Explosion` at (7046, 368, −2409.235), `MaxDamage` 5, `DamageRadius` 200, `ImpactFXName` `handgrenade`) explodes; the controller spawns supply items at `Spawner141_Box`; the drum turns invisible, non-solid, not ray-hit, not removed. A third bullet passes through. A bullet of 9 damage on a fresh drum leaves 1 HP → **dead** (≤ 1).
11. **Breakable reset.** After the drum of case 10 broke, a round setup runs `msg memorize trigger;` (memorize), then the level start trigger's `Visible_1`, `Solid_1`, `Remember` triggers: the drum is visible and solid again, takes bullets (stops them), and does not break (dead) (Quirk 9).
12. **Keyframer velocity mode, `Sine`.** SNOWSTORM `Cut_Boss_Camera`: `ObjectName` `Camera0`, `BaseKeyName` `Boss_Cam`, `IgnoreOffsets` true, `TotalPathTime` 2, `Wavetype` `Sine`, `KeyDataIndex` 0.1 → chunk 0 (100 bytes): 2 keys; key 0 control 0, position (40, −2909, −903), pitch 0.2618, yaw −3.1416, time 0; key 1 control 3, position (40, −3012, −1923), pitch 0, yaw 3.1416, time 0, previous tangent (0, 0, 739), next tangent (−2, 0, −93). Key 0 has no next tangent, so the segment is **straight**: L = 1025.187, v = 512.594 u/s. Names: `Boss_Cam0`, `Boss_Cam01`. After `ON`, update n (n = 0, 1, …) adds (π/2)·v·sin(π·min(n·0.05/2, 1))·0.05; after update 20 the camera is at (40, −2962.496, −1432.765) (fraction 0.51938); update 40 (q = 1) puts it exactly on key 1; the next update ends the list and the keyframer goes inactive, camera left on key 1.
13. **Keyframer time mode (synthetic).** Keys at times 0, 1, 3 at (0, 0, 0), (100, 0, 0), (100, 0, 200), no tangents, `TotalPathTime` 0, `IgnoreOffsets` true. Clock 0.5 → (50, 0, 0); clock 2.0 → (100, 0, 100); clock 3.0 → key 2 passed, list ends, not looping → stops at (100, 0, 200). The object's position at the frame the clock is t is the path at t (the clock advances after the move).
14. **Key blind data (HGOILRIG `KeyFramerA`).** Chunk 0, 124 bytes = 4 + 2 × (36 + 12 + 12): key 0 control 3 at (−5216, −14, 2596), yaw −π/2, previous tangent (−612, 0, 624), next tangent (508, −14, −352); key 1 control 3 at (−2336, 946, 740), time stamp 90, yaw −3π/4, previous (−512, −210, 336), next (344, 0, −252). Straight distance 3558.19 u; the Bézier with control points key 0, key 0 + (508, −14, −352), key 1 + (−512, −210, 336), key 1 has arc length 3566.53 u, so the sound moves at about 39.63 u/s over `TotalPathTime` 90 (Open question 7 for the engine's exact length). Straight distances of the other seven: B 2066.17, C 5344.00, D 7093.98, E 6386.63, F 5655.04, G 6053.42, H 4658.17.
15. **Key names.** `BaseKeyName` `Lift`: `GOTO Lift0` → key 0; `Lift01` → key 1; `Lift1` → not found; `Lift10` → key 10; `lift05` → key 5 (prefix without case).
16. **Trigger counting (synthetic).** `Trigger` with `ActivationCount` 2, `NumberOfActivations` 2: messages `TRIGGER` ×5 (one per frame) → commands run on the 2nd and 4th only. With `NumberOfActivations` 0: on the 2nd, 4th, 6th, …
17. **Trigger delays (synthetic).** `SendDelay` 1, `TriggerDelay` 0: touch at t = 0 → commands at the first update with t ≥ 1.0. A second player touching at t = 0.5 replaces the toucher: the commands run with the second player as sender. `TriggerDelay` 2: touches at 0, 1, 2.5 → fires at 0 and 2.5 only.
18. **Weighted trigger.** `Message1Weight` 0.25: over many activations `Command1` runs in 25 % and `Command2` in 75 %, never both; `Command3` always.
19. **Bots and triggers.** A bot walking through a `Trigger` with `AITriggerable` true: nothing. A player: fires.
20. **Command syntax.** `msg Door01 trigger; delay 2 (msg Door01 trigger); msg (A B) (visible 0)` → Door01 triggered now and again 2 s later (real time); objects named A and B made invisible now, before the delay's frame. `if ((Check == 0) AND (TRound == 1)) then (set TRound 0)` with `Check` 0 and `TRound` 1 → `TRound` 0. `rand 0.5 (msg A on) (msg B on)` → exactly one. `msg Unknown on; msg A on` → A turned on (an unknown target is not a failure). `set Undeclared 1; msg A on` → A **not** turned on.
21. **`when` (synthetic).** `int X 0`, then `when (X == 3) then (msg A on)` → stored; `add X 1` twice → nothing; once more → A on, once; further changes → nothing.
22. **Event counter (synthetic).** `StartingValue` 0, `Event1Value` 3 with `Event1IncToValCmd` `msg A on`, `Event1DecToValCmd` `msg A off`: `INC 2` → nothing; `INC` → A on; `DEC` → count 2, A off; `DEC 5` → stops at 0.
23. **Door link (synthetic).** Doors L (`DoorLink` R) and R, closed: player uses L → both open; player uses R while both open → only R closes; a bot uses L → only L opens.
24. **Auto-close (synthetic).** `RemainOpen` false, `TriggerClose` false, `OpenWaitTime` 3, `OpeningTime` 1: open finished at t = 1 → starts closing at the first update after t = 4; a `TRIGGER` at t = 3 → closing starts after t = 6.
25. **No crush.** A `ForceMove` sliding door closing down onto a crouched player: the player takes no damage at any point; the door stops above the player.
26. **HGOILRIG has no decks.** For each of the 16 unlocked `HungerGame` points (for example `GameStartPoint01` at (6984, 387.781, −4272), `Locked` false), a ray straight down to y −10240 hits no physics-BSP polygon with the solid bit except light fixtures (none under the points), no world model and no collision blocker; the first volume it enters is `Water` (its top at y −58) after the sea's surface `SlidingWater` (non-solid). The 16 locked points are all at (4709, 1558, 3076) except `GameStartPoint20` at (4709.924, 1558.125, 3076.678). In DARKHILLS the same unlocked points stand on solid geometry (barn floor), and its 15 locked points are at distinct positions at y 1558.125.

## Open questions

1. **Correction to `world_dat.md`** ("Moving world models and volumes"): its row "`Breakable`: Shakes, then falls or vanishes when broken; `MoveDir`, `MoveDist`, hit points" describes older code that is disabled in this game. A breakable here never moves: it dies at ≤ 1 HP and is removed or hidden (section 11). Its shared-property list also gives `PowerOnTime` 1 s for the doors' `OpeningTime` and `ClosingTime`; the doors' closing time default is 0, which means "the opening time". Should `world_dat.md` be amended?
2. **Correction to `game_modes.md` (Per-tick order)**: it puts the mission update after the objects; the server runs the game shell (command manager, then mission manager) **before** the object updates each frame (Per-tick order above). Check which order `game_modes.md` relied on.
3. **HGOILRIG in the live game.** Is the world reachable from any room (the room data's map list and `gms\oilrig_hg.gms`), and do players really start in the water there? Check: create a Hunted room on Oil Rig (if offered) and see where the warm-up spawn lands.
4. **Box physics.** A world model with `BoxPhysics` true is collided as its bounding box by the engine for some movers (the flag also skips transforming its BSP fully). Which movers use the box (players? rays?) was not traced; stock `WorldModel`s default to true. Check by walking into a non-box-shaped `WorldModel` brush.
5. **Breakables across rounds.** Section 11 derives that a broken breakable comes back unbreakable after `REMEMBER`. Check in game: break a drum in round 1 of an Elimination Pro match on Warehouse and shoot it in round 2.
6. **Pushing on the client.** Answered (Follow-up (2026-10-31b) 2.2): the drawn world model pushes the local player clear and only stops at them when the player is wedged.
7. **Bézier length.** The engine measures a Bézier segment's length with its own approximation (the subdivision used was not read). Test case 14 gives the exact arc length; the engine's value may differ slightly.
8. **Classes not covered**: answered for `TimerTrigger`, `LightActivateTrigger`, `CaptionTrigger`, `MessageTrigger`, `MissionTrigger`, `Camera` / `CameraPoint` and the `TargetTrigger` markers (Follow-up (2026-10-31b) 1); `BGMTrigger` is `music_ambience.md`'s. Still open: `ScaleSprite`, `PolyGrid`.
9. **Real versus game time for delays.** The command manager counts down with the real frame time (unclamped); whether the server's "real frame time" differs from the game frame time other than by the 0.2 s clamp was not checked.
10. **Looping sounds and late joiners.** Whether a looping server sound started before a client joined (a door's looping transition sound, a `SoundFX`) is sent to that client.
11. **`ActivateType` names** (`OpenClose`, `OnOff`, …) map to prompt strings in a data table not read here (`hud_crosshair.md`).
12. **Object update order.** "Roughly creation order" for the server's object list was not verified; it decides which of two doors triggered in the same frame moves first (no visible effect expected).
13. **Touches while standing still.** Whether the engine raises a touch notification for a player standing still inside a `Trigger` or `TouchTrigger` (Corrections (2026-10-22), could not be determined). Check: stand still in an unlimited `TouchTrigger` whose command plays a sound and count the plays.
14. **Client interpolation of sliding world models.** Answered (Follow-up (2026-10-31b) 2.1 and 2.3): 100 ms in the past by server time stamps; keyframer jumps carry riders on the server but not on clients.
15. **Client side of the co-op messages** (Follow-up (2026-10-31b) 1.1–1.5): how a client shows the timer, captions, messages and the objective list is not in the material (the client's handler for those messages is missing); the dialog-text lookup by id is assumed to be the mission file's `[CooperativeModeDialogMsg]`.
16. **Live servers' network settings** (Follow-up (2026-10-31b) 2): the interpolation delay (default 100 ms) and the player-state protocol (default legacy, no rewind) are server settings whose live values are not in the material. Check: walk into a moving sliding door on a live-like setup and compare the drawn lag with 100 ms.
17. **Guide marker re-showing** (Follow-up (2026-10-31b) 1.6): turning a marker off hides its guide marker; turning it on again relies on the UI library's world-anchored placement (not in the material) to show it. Expected: shown again.

## Corrections (2026-10-22)

A behaviour-by-behaviour comparison of the reference's world objects (active world models, doors, spinning world models, triggers, command and touch triggers, event counters, start-up commands, the command manager, the base and world-model messages) with this spec and with our implementation (`games/combat_arms/src/game/objects/`). The reference is the truth. "Spec" means this file before these corrections; the body above is fixed where marked. Ordered by how much a player would notice. Each gives a testable expectation; synthetic cases use dt = 0.05 s.

1. **Reversal test is one frame's step, not 10 % of the move** (spec and implementation). Reference: when a moving door or world model is triggered and has an activator, the pose it tests against the activator is the current pose advanced by **one frame** (speed × frame time) toward the reversed target, the speed taken from the waveform at p = 0.1 of the opposite state's duration. For a sliding one the rate is `MoveDist` / that duration; for a rotating one each axis's rate is (target angle − **current** angle) / that duration, so a half-open door tests a much smaller turn than a full step. The duration used is the plain `PowerOffTime` when reversing from PowerOn and the plain `PowerOnTime` (without the `SETPOWERONMULTIPLIER` factor) when reversing from PowerOff. Ours (movers.rs:418-433) steps the whole 10 % of the duration (0.1 × T, two frames' worth at T = 1 s) using the full-range rate, so it refuses reversals the original allows. Expectation: sliding door, `MoveDist` 100, both times 1 s, `Linear`, opening and at 50 u, activator's box 7 u behind the door's leading face in the closing direction: the reference tests 5 u (100 u/s × 0.05) and **reverses**; with the activator 4 u away it refuses. Rotating door `RotationAngles` (0, 90, 0), both times 1 s, at yaw 45° while opening: the tested pose is yaw 45° − 2.25° (rate −45°/s × 0.05), not 45° − 9°.
2. **Door link: also on reversal and on map-command triggers** (spec and implementation). Reference: after a door handles any trigger, if it is not locked and is now in PowerOn, and its activator is not a bot, the linked door is told (it takes the same activator; triggered only if Off or PowerOff). That includes (a) a closing door re-triggered into opening, and (b) triggers whose activator is nothing or a map object (a `TRIGGER` sent by a command keeps the previous activator; `ON` from a command makes the sending object the activator). The link is resolved once after all objects exist, to the **first** object of that name. Ours (runtime.rs:1190) links only when the door went from Off to PowerOn and the activator is a player, and (runtime.rs:1205) tells every object of the name. Expectations: doors L (`DoorLink` R) and R, both closing after a use: a player uses L half-way → L reverses to opening and R reverses too. A trigger running `msg L trigger` on closed doors → both open. Two objects named R → only the first one opens.
3. **A looping transition sound stops at every transition** (implementation). Reference: entering any state (PowerOn, On, PowerOff, Off) and a locked use first stop the object's current transition sound, even when the new state has no sound of its own; only sounds started with `LoopSounds` can be stopped this way (one-shot sounds play to their end). The re-arm of the auto-off (a trigger while On without `TriggerOff`) and `INIT` start and stop nothing. Ours (movers.rs:274-281) only emits a sound when the new state has one, and its sounds do not loop (objects/mod.rs known shortcuts). Expectation: a `SlidingDoor` with `LoopSounds` true, `OpeningSound` set and `OpenSound` empty, `OpeningTime` 1: the opening sound loops while it moves and is silent from the frame it reaches On (20 frames after the trigger); a locked use of a looping door that is moving silences its loop.
4. **`SOLID 0` also clears ray-hit** (spec and implementation). Reference: the base message `SOLID 0` turns off both the solid and the ray-hit flags (bullets then pass through); `SOLID 1` sets solid and restores ray-hit only if the object had ray-hit at its first message. `HIDDEN 0` adds back each of visible, solid and ray-hit that the object had at its first message and never clears a flag. Ours (runtime.rs:958-966) leaves ray-hit on with `SOLID 0`, and `HIDDEN 0` sets the flags to exactly the first-message ones (clearing a flag that was not set originally but was set since). Expectation: a solid, ray-hit `WorldModel` sent `(solid 0)`: a bullet aimed at it passes through and hits what is behind; then `(solid 1)`: it stops bullets again. An object first sent `(visible 1)` while it started invisible, then `(hidden 1)`, `(hidden 0)`: it stays visible.
5. **`SETPOWERONMULTIPLIER` replaces the factor** (spec and implementation). Reference: the message sets the factor to f; a second message replaces the first; the factor goes back to 1 on entering On. Ours (runtime.rs:1159-1162) multiplies the current factor. Expectation: `SETPOWERONMULTIPLIER 2` then `SETPOWERONMULTIPLIER 3`, `PowerOnTime` 1: the next opening lasts 3 s, not 6 s.
6. **Commands only run when their first statement is valid** (spec and implementation). Reference: every place that runs a stored command string (the active world models' and doors' transition and locked commands, `Trigger` `Command1..10` and `CommandTouch`, `CommandTrigger` and `TouchTrigger` commands, event counter commands, `StartupCommand`, a `msg` whose text is itself a command, the bodies of `delay`, `delayid`, `delayvariable`, `repeat`, `repeatid`, `loop`, `loopid` and the chosen branch of `rand`/`randN`) first checks the string's **first** statement: its first word must be a command word and its argument count exact. If not, the **whole string is ignored** (and for the delays, repeats and loops the statement fails, stopping the rest of its own string; for `rand` the failing branch also stops the outer string). Statements after the first that have an unknown word or the wrong count are still skipped silently. Ours (commands.rs:308-322, 385-396) runs each statement on its own. Expectation: a trigger with `Command1` = `mgs Door1 trigger; msg Door2 trigger` (typo first) does nothing; `msg Door2 trigger; mgs Door1 trigger` triggers Door2 only. `delay 1 (mgs A on); msg B on` → B is **not** turned on and nothing is queued.
7. **`ABORT` that finds nothing fails** (spec and implementation). Reference: `abort <id>` with no pending command of that id stops the rest of its string, like any failing statement. Ids are kept to 15 characters when stored but compared in full, so an id longer than 15 characters can never be aborted (and fails). A delayed command that has to run at once because the 512-slot table is full also counts as a failure after running. Ours (commands.rs:447-458) always succeeds. Expectation: `abort Wup; msg A on` after `Wup` already ran → A is **not** turned on; `msg A on; abort Wup` → A on (HGOILRIG's level end trigger puts its `abort` last, so the stock map is unaffected).
8. **A nested command run clears the outer string's sender and default target** (implementation). Reference: whenever a command string is run (by any object, synchronously inside a `msg`), the command manager's current sender and default target are cleared to nothing when that run finishes. So in an outer string, every statement **after** a `msg` whose receiver ran a command of its own (a door's `OpeningCommand`, a trigger's commands, an event counter's command, a message that is itself a command) runs with no sender and no default target: an empty target name sends to nobody, `Sender` matches nothing, `ON`/`OFF` to an active world model make "nothing" the activator (a rotating door then opens in its stored direction, not away from the player), and `delay`/`repeat`/`loop` queued later store no sender and no target. Statements before that point are unaffected; delayed commands run with the sender and target they stored. Ours (commands.rs:308) keeps the outer string's context throughout. Expectation: trigger touched by player P with `Command1` = `msg DoorA trigger; msg DoorB on` where DoorA has an `OpeningCommand`: DoorB's activator is nothing (it opens in its stored direction, never flipped away from P); with DoorA's `OpeningCommand` empty, DoorB's activator is P.
9. **Trigger `CommandTouch` targets the toucher** (implementation). Reference: `CommandTouch` runs with the toucher as both sender and default target. Ours (runtime.rs:1482-1484) gives it no default target. Expectation: `CommandTouch` = `msg () (visible 0)` hides the player who touched it (as a message to the toucher), not nothing.
10. **`VMSG` with an unknown variable does not fail** (implementation). Reference: `vmsg <var> <message>` whose variable does not exist, is not an object variable or holds no object sends nothing and the string goes on. Ours (commands.rs:402-411) fails and stops the string. Expectation: `vmsg Nope (on); msg A on` → A on.
11. **`RAND` passes the branch's failure on; a failing branch stops the outer string** (implementation). Reference: the chosen branch is run as a command string; if it fails (or is not a valid command, 6), the `rand` statement fails. Ours (commands.rs:459-466) ignores the branch's result. Expectation: `rand 1 (set Undeclared 1) (msg A on); msg B on` → B not turned on.
12. **`REPEAT` with a rolled count of 0 still runs once** (implementation). Reference: the count is rolled uniformly in min..max; the command runs at its first due time, and only then is the count decremented and the slot freed when it reaches 0 or less. So a roll of 0 (or 1) runs once. Ours (commands.rs:429-438) drops the command when the roll is 0. Expectation: `repeat 0 0 1 1 (msg A on)` → A on once, 1 s later.
13. **Commands queued inside the manager's own update can run in the same frame** (spec and implementation). Reference: each frame the manager walks its slots in order, subtracting the frame's real time from each pending delay; a command run from slot i that queues another takes the first free slot other than i; if that slot lies after i, the new command is reached later in the **same** walk and, with a delay not above the frame time (`delay 0`), runs this frame. Commands queued outside the walk (by touches, use keys, damage, object updates) are first counted down in the next walk: one queued while the frame's client packets are processed runs in that same frame's walk (the walk comes after them), one queued by an object update runs next frame. Ours (commands.rs:714-750) never runs a command queued during the walk in that walk. Expectation (slots empty, 20 Hz): `delay 0.05 (delay 0 (msg A on))` queued by a trigger touch: A on in the frame after the touch (both delays in one walk), not two frames after.
14. **Event counter commands run between steps** (implementation). Reference: `INC n`/`DEC n` takes n single steps and runs each step's matching commands right after that step, before the next; a `LOCK` sent by such a command stops the remaining steps; n is read as a whole number (`INC 2.5` = 2). Ours (runtime.rs:1265-1290) takes all steps first, then runs the commands, and reads a non-integer n as 1. Expectation: counter at 0, `Event1Value` 1 with `Event1IncToValCmd` `msg Counter lock`, `Event2Value` 2 with `msg A on`: `INC 2` → count stops at 1, A not on.
15. **`REMEMBER` restores only the object's logic state** (implementation). Reference: `MEMORIZE` stores, and `REMEMBER` reloads, the world-model and active-world-model state (machine state, on/off and original positions, running angles, timers, pivot, flags of the machine, sounds and commands), and restarts the remembered state's transition sound if `LoopSounds`; it does **not** move or rotate the object, does not change its visible/solid/ray-hit flags and does not change when it next updates. Ours (runtime.rs:1010-1020) also sets the remembered flags and teleports the object to the remembered pose. The stock reset (breakables, which never move, and an explicit `visible 1` / `solid 1`) looks the same either way. Expectation: a sliding door memorized closed, then opened, then sent `REMEMBER`: it stays where it is (open), its state is Off; the next use re-reads its current (open) position as the off position and slides a further `MoveDist`.
16. **Switching a volume on nudges every player** (implementation; world_dat.md Corrections (2026-10-22) 6). Reference: switching a volume on moves every player 10 u along (+1, +1, +1) and straight back on the server so that containers and touches are re-evaluated; any `Trigger` or `TouchTrigger` the player's box touches during that nudge is touch-fired. Ours: volumes switch without touching anything. Expectation: a player standing in an unlocked `TouchTrigger` (limit 0) when a `msg Gas on` arrives → the touch trigger's commands run once more on that frame.

**Could not be determined** (added to Open questions):

- Whether the engine sends a touch notification for a player who stands still inside a `Trigger` or `TouchTrigger` (ours fires one every frame while overlapping, runtime.rs:1489-1520). The reference's touches come from the engine's mover when the player object is moved by a client update; whether a zero-length move notifies was not traced (the engine's touch code is not in the material).
- Whether the client's rotating world models really snap (prediction disabled) while sliding ones interpolate: confirmed that rotating ones are flagged "no prediction" by the server; the client's interpolation of the others is engine code not in the material. Ours snaps both (objects/mod.rs known shortcuts).
- Whether a looping transition sound already playing is re-sent to a late joiner (Open question 10, still open).

## Follow-up (2026-10-31b)

From the reference's timer, light-activate, caption, message, mission and target triggers, the camera and camera point, the world-model debris and the destructible aggregate's death handling, the debris table reader, the engine's client object interpolation and client mover, the client's movement manager, and the install (Steam build 26236938, all 105 world files, `ATTRIBUTES\TRIGGERRESOURCE.TXT`, `ATTRIBUTES\DEBRIS.TXT`). Answers Open questions 6, 8 (in part) and 14 and the plan's item 23 gaps. Where the body above disagrees, this section wins. Test cases 27–52 follow at its end.

### 1. Event objects of Open question 8

Census (install): `TimerTrigger` 17 in 10 maps, `LightActivateTrigger` 79 (all in INFECTEDSHIP_CP), `CaptionTrigger` 18 (one per map, always named `CapTRG01`), `MessageTrigger` 8 (`MsgTRG01`), `MissionTrigger` 7 (`MishTRG01`), `TargetTrigger` 208 in 38, `Camera` 104 (`Camera0` ×99, `Camera00` ×5), `CameraPoint` 389. None of the first five has a box, a touch or a model; they are message receivers placed out of reach. All take the base messages of section 13 as well.

**1.1 `TimerTrigger`** (a stopwatch that runs one of three command strings depending on when it is stopped).

| Property | Type | Default | Meaning |
|---|---|---|---|
| `TimeAmount` | real | 15 | The time T, s |
| `InTime_Command` | string | empty | Run when stopped before T (or at `InTimeCommandTime`, below) |
| `OverTime_Command` | string | empty | Run when stopped after T |
| `JustTime_Command` | string | empty | Run when stopped at exactly T |
| `AutoStop` | bool | false | Stop by itself when the clock runs out |
| `InTimeCommandTime` | real | 0 | If > 0: run `InTime_Command` once when the elapsed time reaches this, instead of at an early stop |

- **Clock**: server game time. `START` (ignored while running) starts it with a run-out time of **T + 1 s** (not T), clears the "in-time command already run" mark, tells every client (guaranteed) "timer on, T seconds" and, in co-op modes, sets the mission's sub-mission time to T. How the client shows that timer is not in the material (Open question 15).
- **Updates**: decided once at load. If `AutoStop` is true or `InTimeCommandTime` ≠ 0, the object updates every server frame for the rest of the map; otherwise it never updates, and later `INTIMECOMMANDTIME` messages cannot start updates (its early command then never runs on its own).
- **Each update**: (1) if running, `InTimeCommandTime` > 0, the early command has not run since `START`, and elapsed ≥ `InTimeCommandTime`: run `InTime_Command` and mark it run. (2) If `AutoStop` and running and the clock has run out (elapsed ≥ the run-out time fixed at `START`, T + 1 with T as it was then): stop as below.
- **Stopping** (`STOP` or the automatic stop; ignored when not running): with e the elapsed time, exactly one of: e = T exactly → `JustTime_Command`; e > T → `OverTime_Command`; e < T → `InTime_Command`, but only when `InTimeCommandTime` is exactly 0. Then the clock is turned off, every client is told "timer off", and in co-op the sub-mission time is set to 0. Each command runs with the timer as sender and default target.
- So the automatic stop always runs `OverTime_Command` (at the first update with e ≥ T + 1), never `JustTime_Command`; e = T exactly is practically never met. Stock maps that do not auto-stop pair `START` with a delayed `STOP` one second after T (`msg Gene_Timer Start` … `delay 181 (msg Gene_Timer Stop)` for T = 180), so the late branch is reached by that stop; an earlier `STOP` from the objective (a generator switched on) gives the in-time branch.
- Messages: `START`, `STOP`, `TIMEAMOUNT t` (sets T; the running clock is not changed), `INTIMECOMMANDTIME t`.
- **Correction to `game_modes_2.md` M13 "Opening" and its case 24**: the stock `VipExitTimer` (`TimeAmount` 60, `AutoStop` true, `InTimeCommandTime` 54; its `JustTime_Command` and `OverTime_Command` are identical) runs the helicopter's arrival at 54 s and the unlock commands at the first update at or after **61 s** after `START`, not at 60 s.

**1.2 `LightActivateTrigger`** (a hold-to-use spot, like the mode `ActivateTrigger` of `game_modes.md` M10 without teams, game type, shared use or icons).

| Property | Type | Default | Meaning |
|---|---|---|---|
| `Dims` | vector | (16, 16, 16) | Half-extents, sent to clients for the use test and prompt |
| `TimeAmount` | real | 10 | Hold time to complete a use, s |
| `NumberOfActivations` | long | 0 | Completed uses allowed; 0 = unlimited |
| `Locked` | bool | false | Starts locked |
| `WorkCommand`, `ReleaseCommand`, `ActivateCommand` | string | empty | Run when the first user starts, when the last user stops, at each completed use |

- Clients get the trigger as an activate-type trigger effect with its name, dims, locked state, use time and team "any"; the use prompt, gauge title, icon and the locked and wait texts come from the client's trigger resource table by the trigger's **name** (`hud_crosshair.md` "custom" gauge; the table in 1.6).
- A player's start request (the client's hold of the use key; the request path is not in the material) is refused with a "wait" answer while a wait-until time set by mode code lies in the future; otherwise the player is put on (or back on) the trigger's user list as working, with a start time on the **wall clock** (ms), the user count goes up, the player is told "can start", and the trigger updates every frame. An end request takes a working player off (count down).
- **Each update**, for every listed player: disconnected → dropped (count down if working); dead → stops working; trigger locked → stops working and is told "locked"; working for more than `TimeAmount` × 1000 ms of wall-clock time → **completes**.
- **Completing**: ignored while locked. The player stops working (count down). If `NumberOfActivations` > 0 and it has already completed that many times, it **locks itself** instead (clients are told) and nothing runs. Otherwise the completion count goes up, the player becomes the trigger's "last activated player" (map commands such as a character change aimed at the controller use it), and, if there is an `ActivateCommand`, it runs (sender and target the trigger), the player is told "success" and a co-op sub-mission success is counted for them. Several players can work at once, each with their own start time; each completes on their own.
- **Work and release**: after the per-player pass, if the user count changed since the last update: from 0 → `WorkCommand` (unless locked or the activation limit is used up); to 0 → `ReleaseCommand` and the trigger stops updating.
- Messages: `LOCK`, `UNLOCK` (both tell every client the new state). No touch, no `TRIGGER`.
- Stock (INFECTEDSHIP_CP): 50 search spots (`<n>RD_ActTRG_Bag…`, `…_Cadaver…`, `…_ATM…`, `Guide…`: unlocked, `TimeAmount` 5, one activation; their `ActivateCommand` rolls `rand 0.9` between a supply spawn from the mission data and a `Spawner`, removes the searched prop, locks the trigger and turns a sound on) and 29 `Path_Blocker`s (locked, unlimited, 10 s, no commands).

**1.3 `CaptionTrigger`**: property `Showtime` (real, 10 s; stock 4 or 10). `PLAY <id> [team]` (ignored while locked) looks up the text named by id (stock ids `Cap…`, e.g. `Cap404`; the co-op mission file's `[CooperativeModeDialogMsg]`, `missions.md`) and sends every client (guaranteed) "show caption: text, `Showtime`, team" (team 0 when omitted). `CLEAR` sends "hide caption". `LOCK` / `UNLOCK` set a lock that only `PLAY` obeys. No update.

**1.4 `MessageTrigger`**: the same with one message kind: property `Showtime` (10; stock 4); `PLAY <id> [team]` sends every client "show message: text (stock ids `Msg…`), `Showtime`, team"; no lock, no clear. How the client draws captions and messages (the HUD's `Caption` control, `ui_system.md`) is not in the material.

**1.5 `MissionTrigger`** (the co-op objective list). No properties. In a co-op mode it registers itself with the mission manager at load. It keeps a list of objectives in the order added, each with an id, a team, a state (normal, in progress, success, failed), a text (looked up by id and the co-op difficulty in the map's mission data) and, for counted objectives, a current and a maximum count. Messages (each change is also sent to every client, guaranteed):
- `ADD <id>` (team 0) and `ADDTEAM <id> [team]`: add a plain objective in state normal; an id already listed is ignored.
- `ADD_POINT_MISSION <id> <max>`: add a counted objective in state in progress; the maximum comes from the mission data for the difficulty (the message's own number is ignored).
- `UPDATE_POINT_MISSION <id> <count>`: set its count, state in progress, re-read its text.
- `CHECK <id>` → success, `FAIL <id>` → failed (in co-op also reported to the mission manager with the objective's team); `DELETE <id>` removes it. Unknown ids are ignored.
- Stock use: `msg MishTRG01 (check 0)`, `(fail 1)` and the like from timers and counters.

**1.6 `TargetTrigger` markers.** A `Trigger` (all of section 12's touch and command behaviour, with its commands group hidden and `NumberOfActivations` 1) plus a marker. Properties: `CheckTeam` (true), `Team` (team name, or `<none>`), `GameType` (a mode name), `RadarTexture`, `LargeMapTexture`, `HUDTexture`.
- **Team**: with `CheckTeam` and a team name the marker belongs to that team; with `CheckTeam` false it belongs to "all teams"; with `CheckTeam` true and `<none>` it belongs to no team. Stock: 6 `JSF`, 6 `Creed`, 107 `CheckTeam` false, 89 `CheckTeam` true with `<none>`.
- **Game type**: the marker exists for clients only when `GameType` names the running mode (stock: `CooperativeMission` 95, `HeadQuaters` 60, `SquadMatch2` 15, `CabinFever2` 15, `InfectedShip` 11, `VIPEscort` 9, `SquadMatch` 3). Otherwise clients never hear of it (it still works as a trigger).
- **On/off**: starts off. `ON` / `OFF` set it and send every client the marker's full state; late joiners get the state with the object.
- **Icons come from the client's trigger resource table, by the trigger's name**, not from the three texture properties (they are sent but ignored). `ATTRIBUTES\TRIGGERRESOURCE.TXT` has records `[Trigger0]`, `[Trigger1]`, … (308 in the install) with `Name`, `CheckTeam` (0), `ShowDist` (0), `TargetGuideTex`, `TargetLargeMapText`, `AllyTargetGuideTex`, `AllyTargetLargeMapText`, `EnemyTargetGuideTex`, `EnemyTargetLargeMapText` (the target-marker keys; the rest, `ActionMessage`, `ActionTitle`, `ActionIconTex`, `LockedMessage`, `WaitMessage`, serve the use triggers). The first record whose `Name` equals the trigger's name exactly (case counts) is used; 104 of the 107 stock marker names have one (`DynamiteTargeticon00`, `WeaponSpawnMarker00`, `WeaponSpawnMarker01` have none: no icon). A viewer is an **ally** when their team equals the marker's team. With the record's `CheckTeam` 1, an ally takes the `Ally…` picture if it is non-empty and anyone else the `Enemy…` picture if non-empty; otherwise (and always with `CheckTeam` 0) the plain picture is used. The `…LargeMapText` picture is the large-map and radar icon (`radar_minimap.md`); the `…GuideTex` picture is the on-screen guide marker.
- **On-screen guide marker** (the `WorldItem` template of the main HUD layout): one per marker that has a guide picture, placed over the marker object's position each frame; hidden while the marker is off, during a video or while the game freezes players. Fully opaque, or alpha 125 of 255 when the camera's line to the point is blocked (that test itself is not in the material). With the record's `ShowDist` 1 it shows the distance from the local player's position to the point as whole metres, ⌊d / 40⌋ followed by ` M`. The escape-zone trigger of VIP Escort is drawn the same way. Stock example: VIP Escort's `Indicator_EZone_unable` (guide `UI_HUD_Common\Guide\Guide_area_Exit_notOpen.DTX`, map icon `UI_HUD_Map\icon\area_Exit_notOpen.dtx`, distance shown), swapped by the exit timer's commands for `Indicator_EZone_disable` (`Guide_area_Exit.DTX` / `area_Exit.dtx`).

**1.7 Cameras.** `Camera` properties (stock values in brackets): `ActiveTime` (−1: until turned off; > 0: seconds), `AllowPlayerMovement` (false), `OneTime` (true [all false]), `StartActive` (false), `IsListener` (true), `CanSkip` (true [all false]), `CleanupCommand` (empty), `FovX` 90, `FovY` 78 [75 on 94 cameras], `OnSkipCleanupOnly` (false).
- `ON`: the camera becomes live (a "live" flag on the object, which reaches every client with the object); with `ActiveTime` > 0 its turn-off time is now + `ActiveTime`. While live it updates every server frame: it sets every player's view position on the server to the camera's position (what the server uses as that client's point of view for what it sends), and turns itself off once past its turn-off time.
- `OFF`, and the timed turn-off: run `CleanupCommand` (if it is a valid command; this happens even when the camera was not live), clear live, stop updating, and remove the camera if `OneTime`. `SKIP`: only if `CanSkip` and live; the same, except that with `OnSkipCleanupOnly` only the cleanup command runs.
- `MOVETO <name>`: the first object of that name, only if it is a `CameraPoint` (otherwise ignored), gives the camera its position and rotation at once (a jump).
- `FOV <x> <y> [t]`: sets the camera's field of view (degrees) and tells every client, which blends linearly from the previous values over t seconds of client frame time (t omitted or 0: at once).
- `CameraPoint`: a named position and rotation that never updates.
- Stock use: `msg Camera0 (moveto <point>)` (977 times), `on` (772), `off` (558), `(fov 100 84)`, `(fov 55 41.25)`, `(fov 70 52.5)` and a few others; the points are named `A_Exp_Cam…`, `B_Exp_Cam…`, `C_Exp_Cam…`, `D_Exp_Cam…`, `HQ_Cam…`, `Cut_Scene_…`, `Nuke_Cam…`. Seven keyframers move `Camera0` too (section 9). Which camera a client looks through and how the death screen yields to it is `death_killcam.md`; the client's own switching code is not in the material.

### 2. How clients move world models and carry players (Open questions 6 and 14)

**2.1 Which objects interpolate.** Moving objects reach clients as ordinary object updates stamped with the server time. A client shows them in one of two ways:
- **Rotating active world models** (motion type 2: `RotatingDoor`, `RotatingWorldModel`, `RotatingSwitch`, `SpinningWorldModel`) are flagged "no prediction": every received position and rotation is applied **on arrival** (a step at each 20 Hz update), with no delay.
- **Everything else that moves** (sliding doors and world models, attached volumes, keyframed objects) is **interpolated in the past**: each client frame it is shown at the pose for the time τ = (the client's estimate of the current server time) − D, with D the server's interpolation delay sent at connect (the dedicated server's setting `MaxInterpolateTime`, **100 ms** by default; the live servers' setting file is not in the material). The pose is the linear blend of positions (spherical blend of rotations) between the two received updates whose server times bracket τ; at or past the newest update it holds the newest pose (no extrapolation). If the newer of the two updates was a **teleport** (or a new object), or both have the same position, the position jumps to the newer one instead; the same for rotation with a snap. An update flagged as a teleport also jumps the object at once on arrival. The server-time estimate is the local time minus a smoothed offset that never lets it run backwards.
- **Keyframed objects without `PushObjects`** are placed by the server with a teleport every frame (section 9 "Placing"), so every update they send is a teleport: on clients they **jump** to each new pose on arrival (20 Hz steps, no 100 ms delay), position and rotation alike. With `PushObjects` their per-frame moves are ordinary moves and interpolate like sliding doors; only `GOTO` jumps. A camera's `MOVETO` is a teleport too, so cameras jump.
- This replaces section 17's "about one server period × 1.1", which is an older code path the stock server settings do not use.

**2.2 Pushing and carrying the local player (Open question 6).** Whenever a client changes the pose of a **solid** world model (each interpolation step of a sliding one, each received pose of a rotating one) and the change is **not a teleport**, the client moves the world model through its own collision with **only its own player** as an object to test:
- **Sliding**: the world model goes to its new place. If it now overlaps the player, the player is pushed out along the smallest push-away; a push upward makes the player stand on it. If the player still overlaps it by more than 0.1 u afterwards (wedged against the level), the world model's own move is cut back to where it touches the player for this step: so the drawn door stops at the player only when the player cannot be pushed clear. Next frame the interpolation computes a fresh pose from the server's updates, so the door catches up as soon as the player is out of the way. A player standing on it is carried by the same displacement, put 0.1 u lower so standing is re-detected, and stays "standing on" while still touching.
- **Rotating**: the world model always takes its new rotation (it is never stopped). A player overlapping its geometry afterwards is moved as little as possible: from where the rotation would carry their position (pushed further out from the world model's centre in 1 u steps, at most 200, until clear of its geometry) back toward their old position until they touch the geometry; a player standing on it is carried by the rotation about its position, put 0.5 u lower. A world model with `BoxPhysics` true skips both (stock: every rotating and spinning one has `BoxPhysics` false; 144 of 345 `SlidingDoor`s and one `SlidingWorldModel` have it true).
- The player's resulting displacement is added to the next movement report as "pushed by other objects" (section 10). Teleports (new objects, `INIT`, keyframer teleports, `GOTO`) neither push nor carry the local player on the client.
- **Rewind for movement**: putting active world models at their pose for the shown server time during the player's own movement tick (section 10, `movement.md` Per-tick order step 1) happens only when the server uses the newer player-state protocol (setting `UseLegacyPlayerStateUpdatePacket` 0). The dedicated server's built-in default is 1, under which the player moves against the world models where they are currently drawn (the same interpolated pose for sliding ones). Which the live servers used is not in the material.

**2.3 Riders on keyframer jumps (Open question 14 and the plan's "`GOTO` doesn't carry").**
- **On the server** every teleport of an object (the per-frame placement without `PushObjects`, `GOTO`, `INIT`) still carries what stands on it: each rider is moved by the same displacement, 0.1 u lower, also as a teleport, up to 32 riders. So a `GOTO` **does** carry riders on the server, correcting the implementation note.
- **The jump back at the end of a looping list and the reset when going active from Off** are not teleports of their own: they only set the keyframer's path position to key 0 (or the last key), and the ordinary placement of that frame moves the objects there, as a teleport without `PushObjects` and as a swept move with it (blocked by and carrying objects like any move).
- **On a client** teleports neither push nor carry the local player (2.2). Since a player's server position follows their own client's movement (section 10), a player standing on a keyframed solid world model moved by teleports is in effect not carried in the original, and `GOTO` jumps leave them behind; with `PushObjects` the per-frame moves carry them on their client like a sliding door. No stock keyframer moves a world model (section 9 census), so none of this happens on stock maps.

### 3. `WorldModelDebris`

- A `WorldModelDebris` is a door (`SlidingDoor` properties, section 7) until launched; the one stock object (TRAINING `Block01`, `MoveDist` 0, solid) is never launched: a piece is launched only when named `<parent name>Debris<n>` (section 11 step 3), and `Block01` is not.
- **Launch** (at its parent's death): rotation periods P = (U(−1, 1), U(−1, 1), U(−1, 1)) s and velocity (U(−200, 200), U(100, 300), U(−200, 200)) u/s. If |P|² > 0.001 it spins about **one axis only, pitch**, at 2π / p rad/s, where p is P's z if |P.z| > 0.001, else P's y if |P.y| > 0.001, else P's x (each non-zero period overwrites the pitch rate; yaw and roll rates stay 0). Its flags are replaced by exactly visible + gravity + ray-hit: it becomes **non-solid and no longer touch-notified**, so the engine moves it without any collision. Blocking priority 100, friction coefficient U(10, 20) (both moot without collision).
- **Flight**: it falls under the engine's object gravity (0, −1600, 0) u/s² (`grenades.md` constants) from that velocity, through floors and everything else: it **never lands**. Each server frame it checks whether it stands on something (it never does) and otherwise sets its rotation to R(pitch, yaw, roll), where the angles start from its own `Rotation` property and pitch grows by its rate × the time since the last frame. On the first spin frame the object therefore takes its `Rotation` property as a world rotation (the world-model placement had dropped it, section 1), which turns its geometry a second time when that property is not zero.
- **Lifetime**: none; the game never removes it. It stays ray-hit, so bullets along its path still hit it. Clients see it fall like any non-rotating moving object (2.1).

### 4. `DebrisType` (prop debris)

A destructible world-model object's `DebrisType` names a record of the debris table `ATTRIBUTES\DEBRIS.TXT` by its `Name` (case ignored); its death sends every client a debris effect at the object's position, facing the death direction. The look and motion are `effects_sounds.md` Follow-up (2026-10-31b) 1, which carries the table. Install census of non-`None` users (390): `WorldModel` `Window` 86, `Window_Large` 61, `Window_smoll` 52, `Stone` 52; `Breakable` `56x56WoodBox` 120, `Window` 13, `Window_Large` 6. 47 of the `Window` world models have `NeverDestroy` true and never break unless sent `DESTROY`. `None` (record 0) is a real record with no models: its death still sends an effect, which shows nothing and plays nothing. For world models the record's surface id does not set the object's surface (`SurfaceOverride`, else the polygons, decide it).

### 5. `CreateExplosion` / `FIRE` and `Spawn` (no stock map uses them: all 5,173 destructible world models have `CreateExplosion` false, `WeaponId` 0, `FireAlongForward` false and an empty `Spawn`)

- **Explosion**: at death with `CreateExplosion`, and on `FIRE [target]` while alive (regardless of `CreateExplosion`), the object fires weapon `WeaponId` with its ammo once from its own position, damage × `DamageFactor`, as the shooter. Direction: straight down (0, −1, 0) with the projectile's lifetime set to 0 (it goes off in place) unless `FireAlongForward`, then along the object's forward vector with the weapon's normal lifetime. With a target name, the first object of that name: the shot aims at its position and the object is turned to face it.
- **Spawn**: at death, before the debris, the `Spawn` string is created at the object's position and rotation: its first word is a class name, the rest the property list in the engine's text form (each `(Name value)`); an unknown class creates nothing.

### Test cases (Follow-up (2026-10-31b))

27. **Timer, late stop.** `TimerTrigger` T = 180, `AutoStop` false, `InTimeCommandTime` 0, commands I, O, J. `START` at t = 0, `STOP` at t = 181 → O runs once; clients got "on, 180" at 0 and "off" at 181. `STOP` at t = 100 instead → I runs. A second `START` at t = 50 changes nothing.
28. **Timer, auto-stop** (stock `VipExitTimer`: T 60, `AutoStop` true, `InTimeCommandTime` 54). `START` at t = 0, 20 Hz → I runs at the first update with t ≥ 54.0; O (not J) runs at the first update with t ≥ 61.0; the zone unlocks then. A `STOP` at t = 30 → nothing runs (in-time suppressed because `InTimeCommandTime` > 0), clock off.
29. **Timer without updates.** T = 10, `AutoStop` false, `InTimeCommandTime` 0 at load (so it never updates); then `msg X (intimecommandtime 5)`, `START` at t = 0 → nothing happens at t = 5; `STOP` at t = 6 → **nothing** runs (e < T, but `InTimeCommandTime` is now 5, which suppresses the early-stop command).
30. **`TIMEAMOUNT` while running.** T = 10, `AutoStop` true, `InTimeCommandTime` 0; `START` at t = 0 (run-out fixed at 11 s); `TIMEAMOUNT 30` at t = 5 → the timer still stops itself at the first update with t ≥ 11, and since e ≈ 11 < 30, **I** runs (not O).
31. **Light-activate use.** `TimeAmount` 5, `NumberOfActivations` 1, `ActivateCommand` A, `WorkCommand` W, `ReleaseCommand` R. Player P starts at wall time 0 → W at the next update; P still holding at 5.05 s → completes: A runs and P is told success; R runs in that update (user count back to 0). Player Q starts at 6 s → no W (the one activation is used up); Q holds to 11.05 s → the trigger locks itself and tells clients, A does not run, R runs.
32. **Light-activate, two users.** P starts at 0, Q at 1 s, `TimeAmount` 5 → W once at P's start; P completes at about 5 s, Q at about 6 s (A twice with an unlimited count); R once after Q's completion. P dies at 3 s instead → P stops working, no A for P.
33. **Caption.** `CapTRG01` `Showtime` 4: `msg CapTRG01 (play Cap404)` → every client receives the `Cap404` text, 4 s, team 0; after `LOCK` the same message sends nothing; `CLEAR` still sends "hide".
34. **Mission list.** `ADD 0`, `ADD 0` again, `ADD_POINT_MISSION 2 99` (data maximum 5 at this difficulty), `UPDATE_POINT_MISSION 2 3`, `CHECK 0`, `FAIL 7` → list [0: success; 2: in progress, 3 of 5]; clients received four changes (the duplicate add and the unknown id send nothing).
35. **Marker icons.** A record like `Indicator_A00` (`CheckTeam` 1, `AllyTargetGuideTex` D, `EnemyTargetGuideTex` A, `TargetLargeMapText` M, `ShowDist` 1) on a `TargetTrigger` with `Team` `JSF`: a JSF viewer gets guide D, a Creed viewer guide A, both map icon M; the guide text reads `12 M` at 500 u (⌊500 / 40⌋). With the marker's `Team` `<none>` and `CheckTeam` true, every viewer gets A.
36. **Marker game type.** A `TargetTrigger` with `GameType` `HeadQuaters` in a TDM match: `ON` shows nothing on any client. In Seize and Secure: shown.
37. **Marker name lookup.** A marker named `WeaponSpawnMarker00` (no record) turned on: no guide marker and no map icon. Map textures in `HUDTexture` are never drawn.
38. **Camera move.** `msg Camera0 (moveto A_Exp_Cam01)`: the camera takes that point's position and rotation at once; `moveto` naming a `Trigger` does nothing. `(fov 55 41.25 2)`: clients blend from 90 × 75 to 55 × 41.25 over 2 s (half way at 1 s: 72.5 × 58.125).
39. **Camera off.** Camera with `OneTime` false, `CleanupCommand` C: `OFF` while not live → C runs; the camera stays. With `OneTime` true → removed after `OFF`. `SKIP` with `CanSkip` false → nothing.
40. **Sliding door on a client.** D = 100 ms; the server door's x is 100·t u, sent at 20 Hz (updates at server times 0.20, 0.25, 0.30 … carrying x = 20, 25, 30 …). When the client's estimate of the server time is 0.325 s (the 0.30 update has arrived), the drawn door is at x = 22.5 (τ = 0.225, half way between the 0.20 and 0.25 updates), not at 30.
41. **Rotating door on a client**: the drawn yaw equals the latest received yaw, changing in 20 Hz steps, with no delay.
42. **Keyframed object on a client** (`PushObjects` false): drawn position equals the latest received position at every frame (steps, no blend); with `PushObjects` true it interpolates like case 40.
43. **Client push, free.** Sliding door moving +x at 100 u/s toward a standing local player with open space behind: the player is pushed along +x by the door's advance each step; the drawn door does not stop; the next movement report carries the summed push.
44. **Client push, wedged.** The same with a wall 2 u behind the player: once the player is against the wall, the drawn door stops where it touches the player (overlap ≤ 0.1 u) on every step; the server's door (section 6) also waits; once the player leaves, the drawn door catches up to the interpolated pose in one frame.
45. **Client carry.** Local player standing on a solid sliding platform rising 50 u/s: each interpolation step raises the player by the platform's step (put 0.1 u lower, then re-landing).
46. **Teleport, no carry.** A solid sliding door with the local player standing on it receives `INIT`: on the client it jumps back at once and the player is not moved with it (no push, no carry).
47. **Server carry on `GOTO`.** A keyframer (no `PushObjects`) moving solid world model W with a crate (a non-player object) standing on it; `GOTO` to a key 300 u up → the crate is moved 300 u up (0.1 u lower) in the same frame.
48. **Debris launch.** Breakable B with a `WorldModelDebris` `BDebris0` and `BDebris1`; B dies → both launched (different random velocities within the ranges); `BDebris2` absent → stop. With velocity (100, 200, −50) and object gravity 1600: about 0.5 s later the piece is about (50, −100, −25) u from its start (± 10 % for the engine's integration step), passing through the floor below; its yaw and roll never change; it still exists after 60 s.
49. **Debris spin axis.** Periods (0.5, 0, 0) → pitch at 4π rad/s; (0.5, 0.25, 0) → pitch at 8π rad/s; (0.5, 0.25, −1) → pitch at −2π rad/s; (0.0005, 0, 0) → no spin (|P|² ≤ 0.001).
50. **Debris type census**: 390 non-`None` users as in 4; `56x56WoodBox` only on `Breakable`s with `SurfaceOverride` `Wood`, `HitPoints` 300, `RemoveOnDeath` false.
51. **`FIRE`** on a live destructible with `WeaponId` w, no target, `FireAlongForward` false: one shot of w's ammo, straight down, exploding in place; on a dead one: nothing.
52. **`Spawn`.** `Spawn` = `Explosion (DamageRadius 300)` on a breakable: at its death an `Explosion` object exists at its position (it does nothing until sent `ON`).
