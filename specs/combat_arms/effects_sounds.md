# Combat Arms: effects and sounds

Source basis: the reference material the user pointed this session to. That
covers the client's weapon-effect path (muzzle flash, tracer, impact, decal,
shell casing, fly-by and hit-confirm handling), the client's effect manager
and effect plug-ins (particle systems, sprites, dynamic lights, camera
jitter), the client's character effects (footsteps, hurt and team call-outs),
the client's sound manager and the engine's sound mixer, the server's weapon,
projectile, explosion, damage and sound code, and the shipped data: the
effect, surface, weapon/ammo, modification, model, sound-set, auto-cast sound
and client attribute tables of the US client's attribute data, the effect
files (`CLIENTFX.FXF`, read as text) of the US 1501.04 and BR 1810.04 client
archives (identical for every group used here except the grenade
explosions' camera jitter, see section 9), texture headers from the
`FX` archive and footstep keys from the male character animation file.
Nothing was checked against a running game; see Open questions. A
follow-up session (Open questions 17–25) added the effect plug-ins' key
code, the liquid and bubble code, the sound manager's update and instance
handling, the call-out and map-sound code, the third-person weapon models'
sockets and the S&D site objects of an older extracted client release's
plaintext maps.

Status: draft

Sections: Summary; Units and conventions; Constants; How effects are
described as data (the effect file, the impact/fire/tracer tables, the
surface table); Behavior (1 who makes what, 2 visibility gates, 3 muzzle
flash, 4 tracers, 5 surface type of a hit, 6 impacts and decals, 7 blood,
8 shell casings, 9 grenade explosion, flash and smoke looks, 10 how a sound
is played, 11 sounds by event, 12 who hears what); Per-frame order; Edge
cases; Quirks; Corrections (2026-10-22); Corrections (2026-10-30);
Follow-up (2026-10-31) (trail keys); Follow-up (2026-10-31b) (prop debris,
empty texture slots, surface 0); Test cases; Open questions.

---

## Summary

- Every visible and audible consequence of a shot is built on the **client**
  from one "weapon effect" event: the shooter's own client builds it at once
  from its own ray, every other client builds it from the copy the server
  sends to **all** clients. Grenade detonations use the same event. The
  look of almost everything (muzzle flashes, impact bursts, explosions,
  smoke) is data: named effect groups in the game's effect file, picked by
  the weapon, the ammo, the attachment and the hit surface's type.
- Bullet holes are 64 px sprites at about 3–4 u, solid for 3 s, fading out
  over the next 3 s, at most 101 at a time. Tracers are flat textured
  strips drawn **without depth test** (visible through walls), one every
  n-th shot per weapon, flying at 6000–10000 u/s.
- Sounds are 3D samples with an inner radius of 7 % of the outer radius and
  a game-side gain that falls with the **square** of distance and reaches
  zero at about 93 % of the outer radius. Beyond 1200 u a low-pass filter
  muffles them; that is the only "far" variant of any sound. Nothing is
  occluded by walls. A footstep is heard through walls up to about 1119 u.
- Footsteps of other players come from their running and jumping animations
  only: walking (walk key) and crouch-walking are silent for everyone. Your
  own footsteps are timed by the view sway at 2.8 steps/s while running.

## Units and conventions

- Distance: LithTech world units ("u"), the same as the other Combat Arms
  specs. +Y is up; left-handed frame (+X right, +Z forward).
- Time: seconds unless stated. Effects run on the **client's real frame
  time**, so everything below is frame-rate dependent where said.
- "Effect group" means a named entry of the effect file (`.fxf`), for
  example `AK47_PV_Muzz`; "key" means one effect inside a group.
- Colours in the effect file are 0..1. Their alpha value is stored
  **inverted**: 0 means opaque, 1 means transparent. Tables below give the
  file's numbers and say "opaque"/"transparent" where it matters.
- Volume: 0..100 as in the data; gain: 0..1 as sent to the mixer.
- Field and record names in `code font` are keys and names of the shipped
  data or console variables a player can set.
- The client's special-effects detail value is the constant 3 in the
  release build, which equals none of the three levels the weapon-effect
  code tests (0 low, 1 medium, 2 high). Everything below is written for that
  value (see Quirk 9). The separate effect-file detail level
  (`ClientFXDetailLevel`, 0 low, 1 medium, 2 high) is the player's option.

## Constants

Fixed in code (console variables in `code font` can be changed by the
player; the value is the default).

| Name (ours) | Value | Unit | Meaning |
|---|---|---|---|
| view_cone_dot | 0.6 | cos | `WeaponFXMinFireDot`, `WeaponFXMinImpactDot`: a point is "seen" if camera forward · direction to it ≥ 0.6 (≈ 53°) |
| far_fire_dist | 1000 | u | `WeaponFXMaxFireDist`: beyond, no shell casing |
| far_impact_dist | 1000 | u | `WeaponFXMaxImpactDist`: beyond, own decals only while zoomed |
| near_other_impact | 300 | u | `WeaponFXMaxMultiImpactDist`: other players' impacts this close count as seen even behind you |
| tracer_min_dist | 100 | u | No tracer for a shot shorter than this |
| mark_solid_time | 3.0 | s | `MarkSolidTime` |
| mark_fade_time | 3.0 | s | `MarkFadeTime` |
| mark_max | 101 | marks | Client decal list size (100 + 1); oldest replaced |
| mark_region | 10 | u | Radius for the "too many marks here" rule (100 as a squared distance) |
| mark_region_max | 10 | marks | More than this inside the region: move one instead of adding |
| server_marks_per_mover | 40 | marks | Decals on one moving world model (doors), server side |
| tracer_list_max | 50 | tracers | Oldest replaced |
| shell_list_max | 600 | shells | Oldest replaced |
| shell_life | 3.0 | s | Every casing |
| shell_bounces | 3 | — | Then it rests |
| shell_grow_time / factor | 0.5 / 2 | s / × | Casing scale grows to 2× its data scale |
| shell_gravity | −1000 | u/s² | Client object gravity (applied at half strength, Shell physics) |
| shell_bounce_damp | 0.5 | × | Speed kept per bounce (water: 0.25, gravity −500) |
| flyby_radius | 600 | u | `FlyByRadius` |
| flyby_impact_guard | 0.75 | × | No fly-by if the impact is within 0.75 × its sound radius of you |
| inner_radius_fraction | 0.07 | × | Inner radius = 7 % of outer when not given |
| lowpass_start / span | 1200 / 600 | u | Distance low-pass ramp |
| voice_max | 255 | sounds | Mixer sample handles and live sound instances |
| footstep_radius | 1200 | u | `FootStepSoundRadius` |
| quiet_factor | 0.65 | × | `QuietMovementVolumeFactor` (effectively unused, Quirk 12) |
| ding_slots | 5 | — | Pending hit-confirm sounds |
| server_sound_send | 2 | × outer | Server sounds are sent only to clients within 2 × outer radius |

## How effects are described as data

### The effect file (`CLIENTFX.FXF`, in the `CLIENTFX` archive)

- One text file (a binary variant exists; the reader tells them apart by
  whether the file starts with `Groups:`). The archive also holds
  `CLIENTFX.FCF`, the editor's layout, which the game does not read.
- Text grammar, one `Tag: value` per line, indentation ignored:
  - `Groups: G`, then G groups.
  - A group: `GroupName: name`, `FxInGroup: K` (number of keys), `Phase: P`
    (group length in **ms**), then K keys.
  - A key: `FxName: Type;Label` (the effect type, optionally a label after
    `;`), `FxID`, `Linked` (0/1), `LinkID`, `LinkNode`, `StartTime` and
    `EndTime` (s, clamped to 0..P/1000), `Repeat`, then three values the
    game ignores (`TrackID`, `MinScale`, `MaxScale`), `Properties: N`, then
    N properties.
  - A property: `PropName: name`, `Type: t`, then one value line, by type:
    0 string, 1 integer, 2 float, 3 combo (`index,choice,choice…`; the index
    is the value), 4 vector (3 floats), 5 four floats, 6 colour key
    (`time r g b a`, alpha inverted), 7 path (`ext|in-archive path`).
  - Keys named `Ck` are colour keys and `Sk` are scale keys
    (`time value 0 0`). Both use time 0..1 over the key's own lifetime, or
    over each particle's lifetime for particle systems, with linear
    interpolation. Particle colour keys are evaluated per particle.
- Group names are looked up without regard to case. A name that is missing
  from the file plays **nothing**, silently.
- A key that lasts the whole group (length ≥ group length − 0.01 s) loops
  when the group is created as looping; otherwise every key plays once and
  then shuts down. With `SmoothShutDown` = Yes a shutting particle system
  stops emitting but keeps its live particles until they die.
- Common properties of every key: `UpdatePos` (0 fixed, 1 follow,
  2 player view, 3 node attach, 4 socket attach, 5/6 the same on the view
  model), `AttachName` (socket, e.g. `Flash`), `Offset`, `DetailLevel`
  (0 all, 1 high, 2 medium, 3 low, 4 medium+high, 5 low+medium, 6 low+high;
  a key whose level does not include the player's `ClientFXDetailLevel` is
  not created), `DisableAtDist`.
- Effect types used by the shipped weapon effects, with what their
  properties mean:
  - **ParticleSystem**: `Texture`, `EmissionInterval` (s; ≤ 0 means
    **one burst** on the key's first frame and nothing after),
    `ParticlesPerEmission`, `MinParticleLifeSpan`..`MaxParticleLifeSpan`,
    `Type` (0 sphere, 1 point, 2 circle, 3 cone, 4 cylinder) with
    `MinRadius`..`MaxRadius` and `EmissionPlane`, `MinParticleVelocity`..
    `MaxParticleVelocity` (per-axis uniform box in the effect's frame),
    `GravityAcceleration` (added to the Y acceleration; positive is up),
    `AdditionalAcceleration`, `Friction` f (velocity × (1 − f)^dt each
    update, dt in s), `PercentToBounce` (chance a particle bounces off the
    world, keeping 75 % of its speed), `SplatPercent` and `SplatEffect`
    (chance a particle that hits the world spawns that group at the hit
    point, oriented to the hit normal; `KillOnSplat` removes it),
    `BlendMode` (0 none, 1 additive, 2 multiply), `MoveParticlesWithSystem`,
    `RotateParticles` with angular velocity in degrees/s. Scale keys give
    the particle size s; a particle is drawn as a camera-facing square of
    side 4·s (Open questions 3).
  - **Sprite**: a sprite file, `Facing` (0 camera, 1 along the normal,
    2 parent aligned), `DisableZ` (draw on top of the world),
    `CastVisibleRay` (hide the sprite while the line from it to the camera
    is blocked). Scale keys scale the sprite.
  - **DynaLight**: a point light; scale keys are its radius in u, colour
    keys its colour. It lights the world only, with no shadows (Open
    questions 4). The engine light is created "world only" and "do not
    light back-facing polygons"; whether it reaches through a wall onto
    the other side is the engine renderer's and stays open. Planting the
    bomb in S&D forces the `DynamicLight` setting on for that client.
  - **CamJitter**: each frame moves the camera by a random offset in
    [−a, a] on each axis, a = scale × falloff; falloff is 1 within
    `InnerRadius`, 0 beyond `OuterRadius`, and linear in **squared**
    distance between. Several jitters active in the same frame **add up**
    (Open question 24): the camera position is reset to the player's view
    position at the start of each frame, and each jitter key then offsets
    the position the previous one left, so two equal grenades at the same
    distance give twice the range of offsets. Nothing caps the sum.
  - **Not an effect, but often mistaken for one: camera rise** (Open
    question 24). When the view height target is more than 1 u above the
    current camera height (stepping up stairs or a ledge, standing up), the
    camera rises toward it at max(gap / `CameraMoveUpTime` (0.1 s),
    `CameraMoveUpMinVel` (140 u/s)), the gap first capped at
    `CameraMoveMaxYDiff` (30 u, so the camera is never more than 30 u below
    the target); lower targets are taken at once. Inside free-movement
    volumes (ladders, liquids) and on the frame after a jump the camera
    snaps to the target.
  - **LightningFX** (spark arcs), **CreateFX** (plays another group,
    `FXName`), **PlaySound**.
- **Placement modes in detail** (Open question 17): every key that has a
  parent object (the effect group was created on an object) recomputes its
  position each update from the parent's current rotation R, with the
  offset `Offset` = (x, y, z) applied along R's right, up and forward axes:
  0 **fixed**: the position where the group was created + offset (the
  offset turns with the parent, the base point does not move); 1
  **follow**: the parent's current position + offset, the key's rotation
  stays its own; 3 **node** / 4 **socket attach**: the named node's or
  socket's current world position + offset, and the key takes that node's
  or socket's rotation (a socket the parent lacks is looked up on the
  parent's attachments, so a key can follow a character's weapon socket);
  5 / 6 the same on a camera-relative model (the view model): the node's
  or socket's model-local position + offset + the model's camera-space
  position; 2 **player view**: nothing is computed by the key, the owner
  (the first-person weapon code) sets the position. A key without a parent
  stays where it was created. `Follow` in the brief's sense is mode 1.
- **Key curves**: colour (`Ck`) and scale (`Sk`) keys are evaluated at
  u = (age / lifetime) mod 1 (so a looping key repeats its curve), linearly
  between the two keys around u, each channel separately, alpha included
  (the file's keys start at u = 0 and end at u = 1).
  Particle systems evaluate them per particle over the particle's own life.
- **Emitter shapes** (`Type`), with `MinRadius` = rmin, `MaxRadius` = rmax,
  `EmissionPlane` = the axis n and two unit axes p₁, p₂ perpendicular to it:
  0 sphere: a uniform direction (a normalised point of the unit cube)
  scaled by U(rmin, rmax); 1 point: the emitter's point; 2 circle: a
  normalised random direction in the plane of p₁, p₂ scaled by U(rmin,
  rmax); 3 **cone**: a height h = rmax − q², q ~ U(0, √rmax), the point
  n·h + (direction in the p₁p₂ plane) × U(0, rmin·h / rmax), so the cone's
  apex is at h = 0 with radius 0 and its base at h = rmax with radius rmin
  (more particles near the base); 4 **cylinder**: (direction in the p₁p₂
  plane) × U(0, rmin) + n × U(0, rmax): radius rmin, height rmax along n.
  `VelocityType` 0 "random" draws each axis of the velocity box
  separately; 1 "from centre" gives speed U(min.x, max.x) along the
  particle's start offset. Unless `MoveParticlesWithSystem` is on, start
  points and velocities are turned into world space with the system's
  rotation at emission.
- **Motion per update** (Δ = frame time in s): position += v·Δ; then
  v ← v·(1 − f)^Δ + a·Δ, with f = `Friction` and a = `AdditionalAcceleration`
  + (0, `GravityAcceleration`, 0) (a world vector; turned into the system's
  frame when particles move with the system). Angle += angular velocity × Δ.
- **Bounce**: a particle is marked "bounces" at birth with probability
  `PercentToBounce`/100 (splatting likewise with `SplatPercent`). Each
  update, for every marked particle, a ray from its position to
  position + v·Δ is tested against world polygons and objects (characters
  and local objects are skipped, non-solid objects ignored); on a hit, v is
  mirrored about the hit polygon's plane normal and multiplied by 0.75
  (position unchanged); a splat creates `SplatEffect` at the hit point
  facing out of the surface (with a random up axis) and, with
  `KillOnSplat`, removes the particle. So only marked particles bounce and
  the plane is whatever world polygon the next step would cross.
- **Sprites**: `DisableZ` draws the sprite without depth test (on top of the
  world); `CastVisibleRay`: each update a ray from the sprite to the camera
  against world polygons and solid objects hides the sprite while it is
  blocked and shows it again when clear (nothing else of the sprite is
  updated while hidden).
- **Other key types**: `LightningFX` draws 1–5 (`MinNumBolts`..`MaxNumBolts`)
  jagged bolts of 10–50 segments, 0.5–3 u wide, perturbed by 0–35 u, each
  living 0.1–3 s after a 0–0.1 s delay and pulsing (`Pulse` 5), between
  attractor nodes or sockets of the parent (or within
  `OmniDirectionalRadius`), textured, camera-aligned by default.
  `LTBModel` places a model (up to 15 skins and 4 render styles) facing the
  camera, along the normal or with the parent, optionally playing a named
  animation over a forced length or in sync with the parent's animation.
  `PolyTube` (the trail key) draws a camera-aligned textured strip that
  follows the key's path: a new section every `SectionInterval` (0.1 s), at
  most `TrailLen` (50) sections, each living `SectionLifespan` (1 s),
  `TrailWidth` (20 u) to each side of its path with a width style
  (constant, small→big, …), texture scrolled by `UAdd`; the file names this
  key type `PolyTrail` (full rules: Follow-up (2026-10-31) 1). `PlaySound` and `PlayRandomSound` play a wave
  (or one of `NumRand` numbered ones) 3D at the key's position, or local,
  with their own inner (100) and outer (500) radius, volume, priority,
  looping and pitch, started when the key starts. The sound lives only
  while its key is running: it is stopped when the key ends, looping or
  not, and while its group is hidden; a key that lasts its whole looping
  group is never restarted between passes (Corrections (2026-10-30) 1).
- The effect's frame: an effect created with a target normal (impacts,
  explosions) gets its local +Z along that normal; its local +Y and +X are
  built from the world up axis (from world +X when the normal is exactly
  vertical). For a floor (normal +Y) the local axes are X → world +Z,
  Y → world +X, Z → world +Y.

### Effect tables in the attribute data (`FX.TXT`)

- **ImpactFX** records (the ammo's `ImpactFXName`, `UWImpactFXName` under
  water): `Sound` (a file, or `surface` = pick by surface and ammo kind),
  `SoundRadius`, `CreateMark`, `Mark` (`surface` = the surface's bullet-hole
  sprite; empty = none), `MarkScale`, `MarkColorRatio` (default 0.5),
  `DoSurfaceFX` (also play the surface's impact group), `IgnoreLiquid`,
  `ScreenTint` with tint fields, `BlastMark` with blast-light fields,
  `PusherName`, `FXName` (an effect group always played), `ImpactOnSky`.
  AI fields are for bots.
- **FireFX** records (the ammo's `FireFXName`): `FXName` (a group the
  **shooter alone** sees at their muzzle), `EjectShells`, `ShellModel`,
  `ShellSkin`, `ShellScale`, `Velocity` (non-zero replaces the random
  casing velocity), `FireSound`, `ExitMark`, `ExitDebris`.
- **TracerFX** records (the ammo's or an attachment's `TracerFXName`):
  `Texture`, `Frequency`, `Velocity` (u/s), `Width` (u), `Length` (u),
  `Color` (0..255), `InitialAlpha`, `FinalAlpha`.
- The weapon record names the muzzle groups (`PVMuzzleFXName`,
  `HHMuzzleFXName`) and the sounds (`FireSnd`, `FireSndRadius`,
  `SilencedFireSnd1..`, `SilencedFireRadius1..`, `DryFireSnd`,
  `ReloadSnd1..3`, `ReloadSndRadius`, `SelectSnd`, `DeselectSnd`,
  `SelectSndRadius`). The muzzle attachment can name its own
  `PVMuzzlefxname`, `HHMuzzlefxname` and `TracerFXName`
  (`attachments.md`).

Stock records used by the common weapons:

| Record | Used by | Content |
|---|---|---|
| ImpactFX `BulletFmj` | every gun | sound `surface`, radius 900; decal `surface`, `MarkScale` 0.075; surface FX on; pusher `BulletExpPusher`; no group |
| ImpactFX `Melee` | knives | sound `surface`, radius 153; no decal; no surface FX |
| ImpactFX `handgrenade` | M67 | sound `gren.wav`, radius 1500; `CreateMark` 1 but `Mark` empty (no scorch); surface FX on; no group |
| ImpactFX `flashbang_exp` | XM-84 | sound `flash.wav`, radius 1500; group `Flashbang_Ini`; no surface FX |
| ImpactFX `Smoke_Grenade` | M18 | sound `Smoke_Exp_GAS.wav`, radius 1500; group `Smoke_Grenade_01`; no surface FX |
| FireFX `5.56mm`, `7.62mm`, `9mm`, `45ACP`, `12Gauge_G` | rifles, SMGs, pistols, shotgun | own-muzzle group `test`, shells on (casing model by calibre) |
| FireFX `FireSoundOnly` | knives | nothing but the fire sound |

Missing vs empty ammo keys: a present but empty `ImpactFXName`,
`UWImpactFXName` or `TracerFXName` falls back to `BulletFmj`, `UWBullet`
and `Standard` (`attribute_data.md`); a missing key gives no effect.

### The surface table (`SURFACE.TXT`)

Per surface record (`Id` = the surface type of `world_dat.md`): `ShowsMark`,
`BulletHoleSpr` (with `BulletHoleMinScale`/`MaxScale`), `ImpactFXName`,
`UWImpactFXName`, `GrenadeFXName`, `UWGrenadeFXName`, `ExitFXName`,
`BulletImpactSnd1..4`, `ProjectileImpactSnd1..4`, `MeleeImpactSnd1..4`,
`HandsImpactSnd1..4`, `FistImpactSnd0..3`, `BowImpactSnd0..4`,
`ShellImpactSnd1` with `ShellSndRadius`, `GrenadeImpactSnd` with
`GrenadeSndRadius`, `RtFootSnd1..2` and `LtFootSnd1..2`, body-fall sounds,
`Hardness` (grenades), `Invisible`, and penetration fields
(`weapon_firing.md`). A `[CustomSound]` list names extra per-surface sound
lists for special characters (robots, bosses, VIP).

| Id | Name | Decal (64 px sprite, scale range) | Impact group | Grenade group | Bullet sounds defined | Footsteps |
|---|---|---|---|---|---|---|
| 0 | Unknown | none | none | `Grenade_Dirt` | 3 (stone) | tile |
| 1 | Air | none | none | none | 0 | none |
| 4 / 6 | ImpactAdultHead / Body | none (characters take no decal) | `Impact_Head` / `Impact_Body` | `Grenade_Stone` | 4 (cloth) | — |
| 5 / 7 | ImpactTeenHead / Body | none | `T_Impact_Head` / `T_Impact_Body` | `Grenade_Stone` | 4 | — |
| 10 | Ceramic tile | `im_concrete` 0.6–0.8 | `Impact_Stone` | `Grenade_Stone` | 4 | tile |
| 11 | Cement | `imark_ceramic` 0.6–0.8 | `Impact_Stone` | `Grenade_Stone` | 4 | stone |
| 20 | Metal | `im_metal` 0.6–0.7 | `Impact_Metal` | `Grenade_Stone` | 4 (metal_bg) | metal |
| 30 | Wood | `im_wood` 0.8–0.9 | `Impact_Wood` | `Grenade_Dirt` | 4 | wood |
| 40 | Glass | `im_glass` 0.6–0.7 | none | `Grenade_Stone` | 4 | glass |
| 41 | Bullet-Proof Glass | `im_glass_bp` 1.0–1.1 | `Impact_Glass_BulletProof` | `Grenade_Stone` | 4 | glass |
| 60 | Desert_Sand | `im_sand` 1.2–1.3 | `Impact_Sand` | `Grenade_Sand` | 4 | sand |
| 70 / 74 | Dirt / Dirt Road | `imarkbasic` 0.6–1.0 / 0.6–0.8 | `Impact_Dirt` | `Grenade_Dirt` | 4 | dirt |
| 71 | Snow | none | `Impact_Snow` | `Grenade_Snow` | 2 | snow |
| 73 | Grass | none | `Impact_Dirt` | `Grenade_Stone` | 4 | grass |
| 80 / 81 | carpet / Cloth (shoot-through) | `cloth` / none | `Impact_Cloth` | `Grenade_Stone` | 4 | cloth |
| 90 | Linoleum | `imarkbasic` | `Impact_Stone` | `Grenade_Stone` | 2 | linoleum |
| 100 / 101 | Wood Door / Metal Door | none (`ShowsMark` 0) | `Impact_Wood` / `Impact_Metal` | `Grenade_Stone` | 4 | wood / metal |
| 110 | Sky | none | none | none | 0 | — |
| 200 | Ladder | none | none | `Grenade_Dirt` | 0 | ladder |
| 201 / 204 | Liquid / Water | none | `Impact_Water` | `Grenade_Water` | 4 | water |
| 202 | Invisible | none | none | none | 0 | none |

The other 40-odd records follow the same pattern (see the table for door,
cabinet, trash-bag, paper, foliage and so on). Scale ranges are
`BulletHoleMinScale`..`BulletHoleMaxScale`.

## Behavior

### 1. Who makes what

- **The shooter's client**, at the moment a round is fired (`weapon_firing.md`
  step 6), builds one weapon effect per ray it casts, from its own result:
  fire position = its first-person flash point, impact point = its ray hit
  backed off **1 u** along the ray, surface type and normal from its own
  test. It plays its own fire sound immediately (section 11).
- **The server**, for every shot it accepts and for every grenade
  detonation, sends one weapon-effect event to **all** clients (no
  distance or visibility filter). It carries the weapon, ammo, surface
  type, fire position, impact position, the normal compressed to one byte
  per axis, the five attachment ids, the impact kind, and a set of
  "ignore" bits the server chose: exit effects and alternate fire sounds
  always off; the normal fire sound off and the silenced one on when the
  weapon has a silencer; the tracer bit off unless this is a tracer shot
  (section 4); hit-confirm off unless the hit object is a character in a
  multiplayer game; decals off on movable objects (the server makes
  persistent decals on movable world models itself, section 6); the fire
  effects off for thrown and launched ammo.
- **The shooter's client receiving its own shot back** does nothing but the
  hit-confirm sound (section 11), except for critical hits, which it
  rebuilds in full. Thrown and launched ammo is not skipped.
- Everyone else builds the full effect from the event.

### 2. Visibility gates

Computed once per weapon effect from the camera at that moment:

- Fire point **seen** if camera forward · (fire point − camera)‾ ≥ 0.6; a
  fire point less than 0.001 u from the camera counts as straight up.
  **Far** if more than 1000 u away.
- Impact point **seen** if the same test passes, or, for another player's
  shot, if it is within 300 u of the camera. **Far** if more than 1000 u
  away.
- Muzzle flash and casing need a seen fire point; the casing also needs a
  near one. Decals and surface bursts need a seen impact. An impact's
  own-ammo group (smoke cloud, flash burst) is made when the impact is seen
  **or** the ammo does area or lasting damage, so smoke and flash looks
  always appear; a frag's actual explosion look comes from the surface
  burst and therefore needs a seen impact.
- No other culling: effects are not limited by distance.

### 3. Muzzle flash

- **First person** (your own gun): the view model carries a looping effect
  group named by the muzzle attachment's flash-hider level (unused in
  stock data), else the attachment's `PVMuzzlefxname`, else the weapon's
  `PVMuzzleFXName`. Nearly every stock weapon uses `AK47_PV_Muzz` (1353 of
  1361 weapon records; the rest are event and snowball weapons). It is shown for
  the group's length (50 ms) after each shot, at the view-model flash point,
  and hidden otherwise, when dead, and under water unless the weapon allows
  an under-water flash.
  - `AK47_PV_Muzz`: one additive particle of texture `AKPV_FX.DTX`
    (128×128), size 0.327 → 0.414 → 0.500 over its 40–50 ms life, white
    fading to black (additive, so fading out) from 28 % of its life; plus a
    dynamic light of radius 170 u, colour (0.97, 0.95, 0.87) → (0.56, 0.50,
    0.38), 50 ms, medium and high detail only.
- **Your own muzzle, extra**: the ammo's FireFX group (`test` for all guns)
  is created at the flash point, for you only: one smoke puff
  (`SHOOT_SMOKE.DTX`, size 4 → 16, life 0.5–0.8 s, velocity box
  (±64, ±64, 100..300) in the shot's frame, rising at 30 u/s², friction
  0.9998, medium/high detail) and a second light, radius 256 → 170 over
  50 ms, medium/high detail.
- **Third person** (other players, and you in a third-person camera): the
  muzzle attachment's `HHMuzzlefxname` if it has one, else the weapon's
  `HHMuzzleFXName`, created at the weapon's `Flash` socket (`Flash01` for a
  left-hand dual weapon), aligned to the shot. Not shown while you
  spectate in first-person chase view.
  - **Where the third-person points are** (Open question 18): "the
    weapon's socket" means the socket of the character's **attached
    third-person weapon model** (`HHModel`, on the character's `RightHand`
    socket, `characters.md` 2), at its current animated position when the
    effect is built; if the character has no such socket the server's
    fire position is used. The muzzle uses `Flash` (`Flash01` for a dual
    weapon's left gun), casings `Breach` (`Breach01`), both looked up
    without case (the M4A1 file spells it `breach`). Stock models
    (older extracted release; socket on node `Bone01`; offset, then the
    model-space point at bind): M4A1 `HH_AR_M4A1_SH` `Flash` (5.172, 0,
    27.250) → (0, 2.685, 24.440), `breach` (5.656, −0.766, 5.391) → (0.765,
    3.170, 2.580); AK-47 `Flash` → (0.119, 2.341, 30.910), `Breach` →
    (1.041, 2.419, 4.863); M92FS `Flash` → (0, 2.217, 7.158), `Breach` →
    (1.031, 1.795, −0.076); L96A1 `Flash` → (0.109, 2.545, 39.158); M60
    `Flash` → (0, 3.701, 34.096); R870 `Flash` → (0, 2.685, 35.330),
    `Breach` → (1.265, 2.982, 4.643). The weapon model's +Z is the barrel.
  - **Others' shot sounds** start at the same point (the `Flash` socket
    position the effect computed), 3D at a point (section 10).
  - `M4_HH_Muzz` (M4A1, AK-47, MK.48 …): one additive particle
    `M4_02.DTX`, size 7.59 → 9.38 → 10.0, life 40–50 ms, moving 200–400 u/s
    along the shot; light radius 170 u, colour (0.89, 0.84, 0.69) → (0.23,
    0.20, 0.16), 50 ms, medium/high.
  - `WEAPON_HH` (SMGs, snipers, shotguns, and every A/B silencer):
    additive particle `GUN_DE.DTX` (64×64), size 10.6 → 13.1 → 15.0, life
    40–50 ms, 200–400 u/s; light radius 170 u, (1.0, 0.98, 0.90) →
    (0.49, 0.49, 0.49) fading to transparent, 50 ms, medium/high.
- **Silencers**: grades A and B (base and Gold) name `AK47_pv_muzz` and
  `WEAPON_HH`, so they still flash and light. Grade C (all colours) and
  every Red silencer name `Silenced01_PV_Muzz` / `Silenced01_HH_Muzz`,
  which do not exist in either effect file checked: **no flash and no light
  at all**, first or third person.
- The flash always lasts 50 ms regardless of fire rate; at 105 ms per M4A1
  round it is lit about half the time while firing.

### 4. Tracers

- **Which shots**, on the shooter's own client: a per-weapon counter
  c (starting at 0 when the weapon object is created or reset) advances
  once per ray:
  c ← (c + 1) mod F, with c ← 0 when F = 0; the shot gets a tracer when the
  new c is 0. F is the `Frequency` of the tracer of the **last fitted
  attachment** in slot order if that attachment names one, else of the
  ammo's tracer (a later fitted part without a tracer resets the choice:
  Corrections 2026-10-22, 3). F = 1 and F = 0
  both give a tracer on every shot. Only the first ray of a shot can carry
  one; extra pellets advance c but cannot draw.
- **Which shots, as others see them**: the server keeps its own counter per
  weapon, the same rule but always with the **ammo's** F.
- **Which look**: the muzzle-slot attachment's tracer if it names one, else
  the ammo's. A shot whose ammo has no tracer never draws one, whatever the
  attachments say. Stock: pistols, shotguns, knives and grenades have no
  tracer.
- **Shape and motion**: a flat strip from the fire position toward the
  impact, **`Width`/2 u wide**, turned toward the eye and showing only its
  `Texture` (colour and alpha fields have no visible effect), not
  additive, depth-**tested** but not written (Corrections (2026-10-30) 3
  replaces the earlier "constant width `Width`", "colour `Color`, alpha
  `InitialAlpha` → `FinalAlpha`" and "depth test off"). With t the time since its first update, v
  `Velocity`, L `Length`, D the fire-to-impact distance:

```math
\text{tail}(t) = P_{fire} + \hat d\, v t, \qquad \text{head}(t) = \text{tail}(t) + \hat d\,L\ \ (\text{clamped to } P_{impact}), \qquad \text{life} = D / v
```

- Not drawn when D ≤ 100 u, when the impact point is in liquid, or with
  `Tracers` 0. `TracerVeloctity` (sic) and `TracerLength` override v and L
  when > 0.

| Tracer | Used by | F | v (u/s) | Width | Length | Texture (px) |
|---|---|---|---|---|---|---|
| AR01 | AK-47, every sight and A silencer | 3 | 6000 | 25 | 300 | red 64×16 |
| AR02 | M4A1 | 4 | 6000 | 15 | 600 | yellow 64×16 |
| AR03 | G36E, C silencers | 4 | 6000 | 20 | 300 | yellow |
| AR04 | B silencers | 6 | 6000 | 15 | 600 | yellow |
| SMG01 / SMG03 | MP5 family / B SMG silencer | 6 / 8 | 10000 | 20 | 600 | yellow 128×16 |
| SMG02 | K1A | 6 | 8000 | 30 | 300 | red 128×16 |
| MG01 | machine guns | 3 | 6000 | 20 | 300 | yellow |
| SR01 / SR03 | M24 / B sniper silencer | 1 / 0 | 8000 | 20 | 600 | yellow |
| SR02 | SR-25 | 2 | 8000 | 20 | 600 | red |
| Pisol01 / Pisol02 | pistol silencers | 0 | 8000 | 20 | 600 | red (never drawn: pistol ammo has none) |

### 5. Surface type of a hit

- **World and world models**: the hit polygon's surface type, the u16 of
  `world_dat.md`'s surface record.
- **What other clients learn about a character hit** (Open question 6,
  answered): only the surface type in the server's event. The server
  derives it from the victim's struck node (for a client-reported hit, the
  node the shooter reported): the model's head surface (4) for a
  head-location node, its body surface (6) for every other location. So
  other clients can tell head from body (blood and burst groups differ)
  but not arm, leg or groin; the damage and zone are the server's.
- **If that is 0 (unknown)** and the hit object is a model:
  - a character: by hit location, the model's `HeadSurfaceType` for a head
    hit, else its `BodySurfaceType`; when the model sets none (stock player
    models), 4 (ImpactAdultHead) for heads and 6 (ImpactAdultBody)
    otherwise. Robot-like models set 20 (metal). Low-violence and the
    Brazilian "white blood" option swap 4/6 for the teen types 5/7.
  - any other model: the object's own surface type (set by the server from
    its properties).
- Other client tests (blood, casings, footsteps) look at the object first:
  a hit object that is not the main world gives its object surface type,
  and the polygon is used only if that is 0.
- **Liquid**: a ray that hits surface 201 (Liquid) makes that impact and
  then continues through the water, making a second impact at whatever is
  beneath.
- **Under water**: an impact whose point + normal is inside a liquid
  volume uses the ammo's under-water impact record, the surface's
  under-water groups (`UWImpactFXName`, `UWGrenadeFXName`, none defined in
  stock data) and half volume for the impact sound.
- A type that has no record in the surface table falls back to the record
  with `Id` 0 (Unknown).
- No ray hit at all: the client treats it as a sky impact at the end of the
  ray (type 110), and the server's event says sky too.

### 6. Impacts and decals

For a non-sky impact (or any impact when the ImpactFX has `ImpactOnSky`),
in this order:

1. **Sound** (section 11).
2. **Ammo effects**: the pusher (if any), the ImpactFX's own `FXName` group
   at the impact with the surface normal, and, for `BlastMark` records at
   high detail (never, Quirk 9), a light. A movable object hit with an
   ammo override record, a ricochet or a "blocked" impact uses those
   records instead. What each of these is (Open question 23):
   - **Pusher**: the impact's `PusherName` record adds a short push to the
     **local player's** movement if they are within its radius (start
     delay, duration, strength). Both stock pushers (`BulletExpPusher`
     strength 400, `RocketPusher` 800) have radius **0**, so they push
     nothing.
   - **Blast light**: for records with `BlastMark` (rockets, grenade
     launchers, mines, claymores and the device "disappear" effects; colour
     68.85 grey, hold U(10, 30), fade U(30, 60)) on a surface that shows
     marks, at **high** detail only and for a blast radius of at least
     100 u: a world-only light of radius blast/2 … blast/4. With the release
     build's detail value 3 it never appears (Quirk 9).
   - **Screen tint**: records with `ScreenTint` would flash the screen with
     their colour by distance and shake it within twice the blast radius;
     **no** record in the US data sets it, so it never happens. The white
     flash of flash grenades is `grenades.md`'s, not this.
   - **Ricochet**: only projectiles (rockets, grenades) can ricochet, when
     their projectile record has a ricochet angle and `MaxRicochets` above
     0; `MaxRicochets` is 0 in every stock record, so nothing ricochets. Bullets
     never ricochet.
   - **Critical hit**: a bullet attachment's special function "critical"
     (`gear_perks.md`) adds damage on its condition and marks the shot
     critical; the event then carries the flag and the surface burst is
     replaced by the attachment's critical effect group, if it names one.
     The shooter's own client rebuilds such an effect in full (section 1).
3. If the impact is seen:
   - **Decal**, when the ImpactFX has `CreateMark`, the surface
     `ShowsMark`, `MarkShow` is on, the hit object can take decals (the
     main world and static world models; not characters, not movable
     objects), and, for your own shot at a far impact, only while zoomed.
   - **Surface burst**: when the ImpactFX has `DoSurfaceFX`, the surface's
     `ImpactFXName` (or `GrenadeFXName` for thrown ammo; under water the
     `UW…` names) is created at the impact with the surface normal. A
     critical hit from a bullet attachment with a critical effect uses that
     effect instead.

**Decal details** (client):

- Sprite: the surface's `BulletHoleSpr` (all stock decals are 64×64 px),
  laid flat on the surface, rotated by a uniform random angle about the
  normal, at the impact point (already 1 u off the surface along the ray).
- Size: scale = `MarkScale` × U(`BulletHoleMinScale`, `BulletHoleMaxScale`).
  A sprite at scale k is drawn 64·k u wide (Open questions 3): 0.075 ×
  0.6..0.8 gives 2.9–3.8 u.
- Colour: grey `MarkColorRatio` (0.5) on each channel, lit by the world
  unless `MarkLight` is 0. Not clipped to the polygon unless `MarksClip`
  is above 0 (default **0**, Open question 19): when on, a ray 2 u either
  side of the decal along its facing axis finds the polygon and the sprite
  is cut to it; when off, a decal on an edge hangs over it. The decal's
  offset from the surface is the 1 u back-off along the **ray** (so
  1 u × the cosine of the angle between the ray and the normal, 0.5 u for
  a 60° glancing hit), with no extra push along the normal.
- Life, with t the decal's age (real frame time, frozen while the game is
  paused):

```math
\alpha(t) = \begin{cases} 1 & t \le T_s \\ \min\!\left(\alpha,\ \dfrac{T_s + T_f - t}{T_s}\right) & T_s < t \le T_s + T_f \\ \text{removed} & t > T_s + T_f\end{cases},\qquad T_s = 3,\ T_f = 3
```

  (the divisor is the solid time, not the fade time; equal by default).
- **Crowding**: before adding, every existing decal closer than 10 u is
  counted, in list order. When the count passes 10, the closest decal seen
  so far is moved to the new point (keeping its age, rotation and size)
  and no decal is added.
- **Cap**: 101 decals; a new one replaces the one that has been in the list
  longest.
- **Movable world models** (doors and the like): the server attaches
  persistent decal objects, up to 40 per object (the same 10-u crowding
  rule; at the cap the closest or farthest is moved), fading with the
  server's own solid/fade times (Open questions 9).

**Surface bursts** (effect groups; detail "m/h" = medium and high only):

| Group | Length | Content |
|---|---|---|
| `Impact_Stone` | 1.0 s | plays `Impact_Dirt` |
| `Impact_Dirt` | 1.0 s | 3 smoke puffs (`SHOOT_SMOKE`, sizes 5→10, 8→15, 10→20, lives 0.4–1.0 s, rising 300–700 u/s along the normal, 2 of them m/h); 6 dirt chips (size 0.5–1.2, gravity −1000, friction 0.5, 25 % bounce, m/h); a spark arc (m/h); an additive flash sprite 0→0.1 over 0.1 s |
| `Impact_Metal` | 1.25 s | spark arcs; additive arc sprite 0→0.25; 6 additive sparks (size →0.6, 0.2–0.3 s, gravity −100, m/h); 2 smoke puffs (m/h); a glow sprite (m/h) |
| `Impact_Wood` | 0.5 s | 6 wood splinters (3 textures, gravity −1000, 25–75 % bounce, 4 of them m/h); 2 brown smoke puffs (m/h); spark arc (m/h); flash sprite |
| `Impact_Cloth` | 1.25 s | 4 dust puffs (2 m/h); flash sprite; glow (m/h) |
| `Impact_Snow` / `Impact_Sand` | 1.0 s | like `Impact_Dirt` in white / sand colours |
| `Impact_Glass_BulletProof` | 0.5 s | dust puff; 2 glass chips and 2 debris (m/h); spark; flash sprite |
| `Impact_Water` | 2.0 s | splash ring sprite (1 s); 4 splash particles (gravity −500, m/h); a column particle |
| `Impact_Body` / `Impact_Head` | 1.0 / 0.5 s | blood (section 7) |

### 6a. Liquids (Open question 20)

- **Rays and water**: water and other liquid volumes are non-solid
  containers, so a shot's ray passes **through** the water surface and
  goes on to the first solid surface, in or out of the water; the surface
  itself gets no splash, decal or sound from a bullet.
- **Impact in liquid**: when the impact point lies inside a liquid volume
  the ammo's under-water impact record is used (`UWBullet` for stock
  bullets) instead of the normal one. The US data has **no** `UWBullet`
  record, so for every stock gun the name finds nothing and an under-water impact
  shows **no** impact effect, decal or surface burst; its impact sound plays
  at volume 50 (section 11). No tracer is drawn for a shot whose impact is
  in liquid.
- **Bubble trail**: when the **fire** point (a point 1 u along the shot
  from the flash point) is inside a liquid and the ammo does bullet damage,
  the client draws a bubble trail (not at the lowest detail): from the
  fire point along the shot up to the first world hit (at most 3000 u), the
  trail grows at that length / 0.3 s, adding particles along the newly
  covered part each frame, 30 in all (15 at medium detail) spread in
  proportion to the distance covered, each a bubble sprite drifting up at
  U(5, 6) u/s, coloured within the liquid's colour range (default
  (200..255) grey). After 0.3 s no
  more bubbles are added and the whole trail fades out linearly over 0.5 s
  from the last addition, then is removed. Others' shots fired under water
  give the same trail on every client.

### 7. Blood

- **On hit**: a hit on a character uses the character surface types of
  section 5, whose bursts are blood groups. Only the bullet's impact
  effects make blood; there is no separate damage blood.
  - `Impact_Body`: 3 mist puffs (`128XBLOODMIST01`, sizes 5→15, lives
    0.4–1.0 s), 3 splash particles (sizes 7→20, 0.3–0.6 s, gravity −300),
    2 blobs (`BLOOD_BLOB02`, size 2→6, life 1.0–1.2 s, gravity −1000,
    friction 0.8), an additive flash sprite, and a red-orange light of
    radius 0 → 150 u over 0.1 s (m/h).
  - `Impact_Head`: 3 (low detail: 1) mist puffs growing to 24, 3 large
    splashes (10→30), 7 blobs (3 textures), an impact sprite drawn on top
    of the world (hidden when its line to the camera is blocked), a splash
    sprite (m/h), a light of radius 0 → 300 u (m/h), and a burst particle
    growing to 80 over 0.67 s.
  - Detail "m/h" pieces vanish at low effect detail.
- **On walls and floors behind**: blood blobs that hit the world (any
  object that is not a character) splat with `SplatPercent` (body blobs
  50 %, head blobs 50 % / 70 % / 70 %) and are removed. A splat creates a
  stain group at the hit point, flat against the hit surface with a random
  spin: `Blood_Stain01..03`, one 128×128 sprite at scale 0.25 (32 u),
  fading linearly from opaque to transparent over 3.0 s. Expected stains:
  body hit 1.0, head hit 4.3. Stains are not decals: they ignore
  `ShowsMark`, the decal cap and the crowding rule.
- **At death**: nothing extra. The older "blood splats along the shot"
  routine and the death and gib effects exist but are never triggered in
  this build.
- **Footprints**: running characters leave footprint sprites on surfaces
  that define them (none in stock data); infected characters leave blood
  footprints (`bloodfs_l/r`, 7 s, scale 0.125).

### 8. Shell casings

- **When**: with a FireFX that has `EjectShells`, `ShellCasings` 1, and a
  seen, near fire point (section 2). Bolt-action and pump weapons whose
  FireFX does not eject instead drop a casing on the view model's
  `SHELL_CASING` animation key (first person only).
- **Where from**: first person, the view model's shell-eject point; third
  person, the shooter's `Breach` socket (`Breach01` for a left-hand dual
  weapon).
- **Velocity**, in the frame of the shot (third person) or of the camera
  (first person), each uniform:

```math
\vec v_0 = \hat u\,U(30,75) \pm \hat r\,U(50,75) + \hat f\,U(20,50)\quad(\text{u/s};\ -\hat r \text{ for a left-hand dual weapon})
```

  replaced by FireFX `Velocity` when that is non-zero. Spin about the up
  and right axes at U(2, 10) rad/s each, random sign.
- **First-person casings move with you**: they start without your
  velocity, but every frame until they rest your own displacement
  (velocity × frame time) is added to their position.
- **Motion** per frame with frame time Δ (frame-rate dependent):

```math
\vec x \leftarrow \vec x + \vec v\,\Delta, \qquad \vec v \leftarrow \vec v + \tfrac12\,\vec g\,\Delta, \qquad \vec g = (0,-1000,0)\ \text{u/s}^2\ \ (\text{water: } -500)
```

- **Bounce**: on a world hit between the old and new position (characters
  ignored), the casing is placed 0.3 u off the surface, its velocity is
  mirrored about the normal and multiplied by 0.5 (water 0.25), pitch spin
  × 0.75 and yaw spin × −0.75. It rests after 3 bounces, or at once on a
  floor (normal y > 0.6) when |v|² < 225/Δ (|v| < 116 u/s at 60 fps). A
  casing that hits sky or an invisible surface, or leaves the world, is
  removed.
- **Look**: the FireFX's casing model and skin at `ShellScale`, growing
  linearly to 2 × that over 0.5 s. Removed after exactly **3.0 s**.
- **Sound**: on the **first** bounce only, out of water: the surface's
  `ShellImpactSnd1` at radius `ShellSndRadius` (200 u on stock surfaces),
  low priority.

### 9. Grenade explosion, flash and smoke looks

`grenades.md` defines when and where; the client turns the detonation event
into the following.

- **Frag (M67)**: sound `gren.wav`, radius 1500 (section 11); then, if the
  impact is seen, the surface's `GrenadeFXName` group: `Grenade_Dirt` (dirt,
  wood, plaster, rock and unknown surfaces), `Grenade_Stone` (stone, metal, glass, most
  others), `Grenade_Water`, `Grenade_Snow`, `Grenade_Sand`. No decal (the
  record's mark is empty), no screen tint. The surface is the grenade's
  last bounce or touch, or the grenade's own when it never touched
  anything, never Air (Corrections 2026-10-22, 1): **a mid-air frag still
  shows its explosion** (the old reading "sound only" was wrong).
  - `Grenade_Stone` / `Grenade_Dirt` (0.9 s): from 0.1 s an additive core
    of 4 particles (20→60), a glow sprite (0→10) drawn on top of the
    world but hidden when blocked, 3 rising smoke columns (2/4/4 particles,
    sizes up to 100, 1.5–2.0 s, 500–2000 u/s along the normal), a smoke
    ring of 10 particles on a circle of radius 64, all with velocity
    (500, 500, 0..50) in the effect's frame, an
    expanding ground ring, 3 burst puffs (80→160), 18 debris pieces with
    gravity −1000 (12 of them always bouncing), a light of radius 0 → 512
    (Dirt) or 0 → 256 (Stone) peaking at 32 % of 0.3 s, and a heat-shimmer
    particle at high detail. Low detail swaps the smoke for fewer
    particles and drops debris.
  - **Camera jitter** in both (US effect file): amplitude 60 u at the key's
    start falling linearly to 0 over 0.6 s, full within 200 u of the blast,
    zero beyond 800 u (falloff linear in squared distance). `Grenade_Dirt`'s
    jitter is high-detail only, `Grenade_Stone`'s at every detail. The BR
    file has `Grenade_Dirt` at amplitude 20 u at every detail (Open
    questions 16).
- **Flash (XM-84)**: sound `flash.wav`, radius 1500; group
  `Flashbang_Ini` (0.5 s), always created: two additive sprites drawn on top
  of the world and hidden while their line to the camera is blocked (a
  splash growing to full in 0.025 s and shrinking by 0.23 s, a glow 10 → 0
  over 0.35 s), a light of radius 0 → 512 → 0 between 0.059 and 0.28 s
  (peak at 0.127 s, m/h), 17 smoke particles and a heat-shimmer burst. The
  blinding white screen is `grenades.md` section 10.
- **Smoke (M18)**: sound `Smoke_Exp_GAS.wav`, radius 1500; group
  `Smoke_Grenade_01` (2.0 s), always created, at the detonation point with
  the detonation normal:
  - a 0.1 s additive splash sprite and a world light of radius 150 → 0
    over 0.1 s (m/h);
  - the cloud, one particle system (all details), answering
    `grenades.md` open question 10:
    - **one burst of 10 particles on its first frame**; `EmissionInterval`
      −1 means no further emission. The 2.0 s key only decides when it
      stops trying to emit; with smooth shut-down the 10 particles live
      out their own lives;
    - start points uniform in a sphere of radius 0–10 u;
    - velocities uniform in the box (−1000..1000, −1000..1000, −200..400)
      **in the effect's frame** (local +Z along the detonation normal; on a
      floor: horizontal ±1000 on both axes, vertical −200..400);
    - acceleration +10 u/s² along world +Y (rising);
    - friction 0.98 is **per second**: velocity × 0.02^Δ each update, so
      a particle travels at most v₀/ln 50 = 0.2556·v₀ (≤ 256 u per
      horizontal axis, −51..+102 u vertically) and then drifts up at
      10/ln 50 = 2.56 u/s (exact value depends on frame time);
    - every particle may bounce (100 %), keeping 75 % of its speed; there is
      no "full bounce";
    - life U(8, 10) s each; size 200 → 300 over its life (drawn as a square
      of side 800 → 1200 u, Open questions 3); colour keys fade it in from
      transparent to opaque over the first 5.1 % of its life, hold it
      opaque, and fade it out over the last 14.9 %;
    - non-additive, unlit; random spin up to 1°/s.
  - So the cloud is fully formed after about 0.5 s, starts thinning at
    0.85 × 8 = 6.8 s at the earliest, and is gone **10 s** after detonation
    at the latest (not 12 s), unless frozen off-screen (Quirk 7).
- **Incendiary and gas**: their own impact records and groups; not covered
  here.
- **S&D and Bombing Run bomb detonation** (`game_modes.md` M3, S15): not a
  weapon effect at all. The map's site trigger turns on six map effect
  objects at the site in turn (about 0.4, 0.5, 2.7, 2.8, 5.2, 5.3 s after
  the detonation), with the groups `Explode_S` and `Explode_L` (some maps
  also `Explode_M`, `Grenade_Dirt`, glass blow-outs, `Nuke_Exp`,
  `S_Area_onFire`), while the shared map camera shows them; their sound is
  whatever those groups and the map's sound objects contain (3 of the 13
  maps read also turn on map `SoundFX` objects). No camera jitter or damage
  comes from the bomb itself.

### 10. How a sound is played

Every sound is one of:

- **Local** (2D, in your head, no distance effect): your own fire, reload,
  draw and dry-fire sounds, your jump voice, hit-confirm dings, interface
  sounds, and your own footsteps (played "client local").
- **3D at a point** (attached to nothing; a moving source keeps its start
  position unless it is a player sound that follows its player): every
  other gameplay sound. Map ambient sounds may instead be "ambient": they
  use the same distance rule but have no direction (always in front of
  you at their distance).

**Radii.** Each 3D sound has an outer radius R and an inner radius r. When
the caller gives no inner radius, r = 0.07·R. Sounds from a named sound set
(`SOUNDBUTES.TXT`) take the set's radii when the caller gives none, and pick
one of the set's files by weight. Server sounds reach clients with R
truncated to whole units and r quantised to ⌊255·r/R⌋/255 of R (7 % arrives
as 6.67 %).

**Gain from distance** (game side, recomputed only when the listener or the
source moved, with d the listener-to-source distance):

```math
g(d) = \operatorname{clamp}\!\left(1 - \frac{d^2 - r^2}{(R - r)^2},\ 0,\ 1\right)\cdot\frac{V}{100}\quad\text{for } r < d < R
```

- For d ≤ r the gain keeps its previous value (1 for a sound that started
  there). The gain reaches 0 at d₀ = √(r² + (R − r)²) ≈ 0.9326·R for
  r = 0.07·R, so the last 7 % of the radius is silent. Beyond R the audio
  library mutes the sound (Open questions 1).
- V is the sound's volume (100 unless stated). The player's master and
  per-class sliders (weapons, speech, default) multiply on top.
- No pitch variation is audible: a random pitch stored in a sound file and
  any requested pitch only change the engine's timer for the sound, never
  its playback (Quirk 10).

**Distance low-pass**, also only for r < d < R:

```math
u = \operatorname{clamp}\!\left(1 - \frac{d - 1200}{600},\,0,\,1\right),\qquad c = \operatorname{clamp}\!\left(1 - \cos\frac{\pi u}{2},\ 0.1,\ 1\right)
```

c is the filter cut-off as a fraction of full bandwidth: no filtering up to
1200 u, heavy muffling from about 1650 u on. This is the only "far" version
of any sound.

**Updates while playing** (Open question 21): every client frame, for
every playing 3D sound, if the listener moved since the last frame or the
source moved, the gain g(d) and the low-pass cut-off c(d) above are
recomputed from the new distance (the same formulas, only for r < d < R);
outside that band the last values are kept. So a sound fades and muffles
continuously as you walk away from it, and one you approach from beyond R
starts at whatever it was given when it started. At the start of a sound
both are computed once. Ambient sounds use the same rule without
direction.

**Occlusion**: none. Walls and floors do not lower or muffle anything (the
engine's occlusion and obstruction calls are stubs).

**How many at once**: at most 255 live sounds and 255 mixer voices. The
priority values the game passes (misc 0–2, AI 2–4, player 4–6) are stored
but never used to pick: when every voice is taken a new sound plays
silently and when 255 sounds are live a new one is refused. Gameplay never
gets close; the per-source limits below are what drop sounds in practice:

- a new fire or alt-fire sound from a shooter stops that shooter's
  previous fire or alt-fire sound of the same weapon (one voice for both
  buttons); silenced fire sounds neither stop nor are stopped
  (Corrections (2026-10-30) 2);
- each grenade has one bounce sound; a new bounce stops the old one;
- server hurt and death sounds: one at a time **per kind, for the whole
  server** (section 11);
- team call-outs: one at a time per kind on each client.

When 255 sounds are live the new one is simply refused (nothing older is
stopped and no priority is compared: the `Priority` values in the data are
never used) (Open question 21).

### 11. Sounds by event

| Event | Sound | Played as | Radius R (heard to ≈ 0.93 R) |
|---|---|---|---|
| Own shot | `FireSnd` (silenced: `SilencedFireSnd`ℓ; a secondary-button swing or shot: `Alt_FireSnd`) | local, volume 100 | — |
| Other's shot | `FireSnd` or the silenced file, never `Alt_FireSnd` | 3D at their flash socket, volume 100 | `FireSndRadius` (silenced: `SilencedFireRadius`ℓ) |
| Dry fire | `DryFireSnd` | never plays (corrected, `view_models.md` Open question 10): the dry-fire state is unreachable and its direct sound is disabled | — |
| Reload / draw keys | view-model `sound_key n`: 1–3 → `ReloadSnd1..3`, 4 → `SelectSnd`, 5 → `DeselectSnd`, 10–14 → `MiscSound1..5` (the grenade pin is `MiscSound1`, `grenades.md` Corrections (2026-10-27)) | local; others 3D at your flash point | `ReloadSndRadius` / `SelectSndRadius` (300); misc: a fixed 2000 |
| Bullet impact | surface list by ammo kind (below) | 3D at the impact, volume 100 (50 under water) | ImpactFX `SoundRadius` (900; melee 153) |
| Grenade bounce | surface `GrenadeImpactSnd` | server 3D, volume per `grenades.md` | `GrenadeSndRadius` (1000) |
| Grenade detonation | ImpactFX `Sound` (`gren.wav`, `flash.wav`, `Smoke_Exp_GAS.wav`) | 3D at the blast | 1500 |
| Casing | surface `ShellImpactSnd1` | 3D, first bounce | `ShellSndRadius` (200) |
| Footstep | surface `RtFootSnd`/`LtFootSnd` 1 or 2 | yours local, others' 3D | 1200 |
| Fly-by | sound set `bulletflyby` (`flyby.wav`) | 3D (Quirk 6) | set: r 100, R 300 |
| Hurt | auto-cast sound by damage kind and by hit zone (below) | server 3D at the victim | 500 |
| Hit confirm | `MultiDing1.wav` or `MultiDing2.wav` (50/50) | local | — |
| Jump | your voice from the auto-cast `Jump` list (`jump1.wav`/`jump2.wav`), you only | local | — |
| Kill call-outs | team auto-cast sounds (`EnemyDown` …) | 3D, **own team only** | 500–1000 |
| Grenade throw call-out | team auto-cast `GrenadeThrowing` at the release (`grenades.md` Corrections (2026-10-27)) | 3D, follows the thrower, **own team only**, thrower included | 500 |
| Map sounds | placed `SoundFX` objects | server 3D or ambient | object's radii |

Details:

- **Fire sound choice**: a muzzle attachment of type silencer or "muzzle
  attachment" with `SilencerLevel` ℓ in 1..9 selects the ℓ-th silenced file
  and radius; level 0 plays nothing. Otherwise `FireSnd`, except that
  your own client plays `Alt_FireSnd` for a shot or swing started with the
  secondary button (others still play `FireSnd` for it; melee:
  `special_weapons.md` 5.1). There is no
  separate near/far or first/third-person file (the code for those is
  disabled); far shots differ only by gain and low-pass. Stock radii: M4A1
  2000 (silenced 1400), AK-47 2000 (1600), MP5A4 1800 (1200), MK.48 2200,
  SPAS-12 1600, M24 and L96A1 3000 (1500), M9 knife 300. Your own fire
  sound plays even when the shot does not fire on the server.
- **Impact sound choice** when the ImpactFX says `surface`: a surface
  custom list named by the ammo (bow ammo) if the surface has one; else for
  ray ammo by damage kind: melee and sword → `MeleeImpactSnd`, infection →
  `HandsImpactSnd`, fist → `FistImpactSnd`, anything else →
  `BulletImpactSnd`; for thrown and launched ammo: burn → none, crossbow →
  `BowImpactSnd`, otherwise `ProjectileImpactSnd`. The slot is uniform among
  **all four** slots 1..4, so a surface that defines fewer than four plays
  nothing on the empty draws: Unknown (3 defined) is silent 25 % of the
  time, Snow and Linoleum (2) 50 %, melee lists (usually 2) 50 %.
- **Footsteps**:
  - Others: each `FOOTSTEP_KEY` in a third-person animation (`animation.md`)
    plays one. Stock male keys: running `LRF`/`LRB`/`LRL`/`LRR` and
    diagonals two per 600 ms loop (e.g. 50 and 350 ms); sprinting `LRR*`
    two per 500 ms; jumps `LJ*` one at 600 ms (the landing). Walking `LW*`
    and crouching `LC*` carry only footprint keys: **silent**. Cadence
    scales with the animation rate, which is always 1 for the leg trackers
    (`characters.md` 7), so others' footsteps come every 300 ms running
    and about every 250 ms sprinting, whatever their real speed (Open
    question 8, answered for rate and gait). The gait is the leg animation
    the **server's** driver chose for that player from the flags the player
    sent (run or walk flag, direction, sprint; `characters.md` 4, When the
    pose is decided), replicated to every client and advanced there each
    rendered frame; each client fires the keys as its own copy of that
    animation passes them. The surface comes from a ray at the foot node
    (left or right alternately).
  - You (first person): the view sway phase φ advances by 2π·1.4·Δ while
    the run flag is set (2π·0.6 otherwise) whenever you move (speed·Δ >
    0.1 u with a movement key held) and can step (on the ground, on a
    ladder, or within 150 u above ground and not jumping); a step plays
    when φ enters (π/2, 3π/2] (right foot) and again when it passes 3π/2
    (left foot): 2.8 steps/s running, about 0.357 s apart. Starting to
    move, jumping and landing each add one immediate step.
  - Your own steps are silent while the run flag is off (walk key held or
    crouched) and in spectator mode; dead players make none.
  - Surface: the surface under the foot (a ray from 50 u above the foot node
    to 50 u below, world and world models only); for you, the surface you
    stand on. Ladders use the ladder volume's override surface; an unknown
    surface inside a water volume counts as Water; invisible surfaces are
    silent. File: `RtFootSnd` or `LtFootSnd` for the foot, slot 1 or 2 at
    random. Special characters (robots, VIP, bosses) use their custom list.
  - Volume 100 × the character's stealth value (1.0), then + the model's
    `FootprintSndVolume` share (−0.5 for the 65 ghillie-type models, so
    volume 50); perks can change volume and radius. Radius 1200.
- **Fly-by**: for another player's shot with bullet-type ray ammo, when
  your camera is not within 0.75 × the impact sound radius (675 u) of the
  impact, and the perpendicular distance from your camera to the **infinite
  line** of the shot is under 600 u (also behind the shooter or past the
  impact). See Quirk 6 for where it is played.
- **Hurt and death** (server): on every damage > 0 to a player, two sounds
  at the victim, radius 500, each only if no sound of the same kind is
  still playing anywhere on the server:
  - by damage type: bullet, melee, crossbow → `DeathbyGuns`
    (`Hurtpain_A_*`, 3 files per sex); explosion → `DeathbyExplosions`
    (`Hurtpain_B_*`); burn and bleed → `Burning`; poison → `CoughinGaszone`;
    falls → `FallingDMG`; freeze → `Frozen`;
  - by hit zone: head → `HitHead` (`HEADSHOT.WAV`), groin → `HitNut`
    (`NutPain.wav`), torso, arm, leg → empty (none).
  - The file set follows the victim's sound template (male, female,
    infected types, VIP). The killing hit is just another hurt; the
    dedicated death cry is never played in this build.
- **Hit confirm**: when your client receives the server's copy of your own
  hit on a character in a multiplayer game, a ding is queued (heal and
  stamina ammo play their own sounds instead). It plays on the next
  character update (Quirk 11), local, priority high. At most 5 can be
  pending; more in one frame are dropped.
- **Jumps of others**: the server sends a jump event whose sounds
  (`CharJumpSound1/2`) are not defined in the shipped client attribute
  data, and the server's own jump entry is empty, so a jump take-off is
  silent for others; the landing is a footstep key (above).
- **Kill call-outs**: on a kill the server sends the killer's team an
  `EnemyDown` call-out at the killer (radius 1000) and the victim's nearest
  teammate's team a `ManDown` call-out (empty in stock data); clients of
  the other team ignore them. Details (Open question 22): `EnemyDown` is
  sent only when the killer's and the victim's teams differ (no call-out
  for a team kill or a suicide), from the killer's position, in the
  killer's voice set; `ManDown` is spoken by the **living teammate of the
  victim nearest to the victim, within 1000 u** (none if no living
  teammate is that close), from that teammate's position. Both are sent
  at the moment of the death, as team
  call-outs: only clients on that team play them, 3D with radius 1000,
  attached to the speaking player so they move with them, in the
  speaker's sound template.
- **Map sounds**: a placed `SoundFX` object has `Sound`, `OuterRadius`
  (default 500), `InnerRadius` (100), `Volume` (100), `PitchShift`,
  `Ambient` (default on), `Loop` (on), `PlayAttached`, `StartOn` and
  `Priority`; it is a server sound. Messages
  ON, OFF, TOGGLE, TRIGGER, KILL control it. Details (Open question 22):
  `StartOn` plays it when the object is created; ON and TRIGGER (re)start
  it from the beginning (a playing copy is stopped first); OFF and KILL
  stop it; TOGGLE stops a playing one or starts a stopped one. A
  non-looping sound plays once and is considered finished after its
  length; it is not repeated. `Ambient` plays it with distance gain but no
  direction, otherwise 3D at the object; a `Sound` that names a sound set
  picks a file of the set, otherwise the file is played directly; both use
  the object's own radii and volume. Census of the plaintext maps read
  (older extracted release): 355 objects, 290 looping and 65 not; outer
  radii from 150 to 6000 u. Server sounds are sent only to clients within
  2R (section 12).

### 12. Who hears what

- Client-built sounds (fire, impacts, casings, footsteps, explosions,
  fly-bys) are made by every client from events that reach every client, so
  everyone hears them by distance alone. Teams do not matter.
- Server sounds (hurt, grenade bounces, map sounds) are sent only to
  clients whose view position is within 2R of the sound.
- Team call-outs are dropped by clients of the other team.
- Nothing is occluded; positions are not checked against walls.
- Your own weapon sounds are never doubled: the server's echo of your shot
  and of your reload, draw and dry-fire sounds is ignored by your client.

## Per-frame order

Client, one frame, as far as effects and sounds go:

1. Incoming server events are processed as they arrive: each weapon-effect
   event runs the steps in "One weapon effect" at once; player-sound events
   (others' reload, draw, dry fire, jump, team call-outs) start their
   sounds; server sounds start.
2. Player update: movement (`movement.md`), view sway and your footsteps
   (section 11), weapon update (`weapon_firing.md`): on a fired round your
   own weapon effect is built at once and your fire sound starts; view-model
   animation keys (reload, draw, casing) fire.
3. Effect objects update: decals (fade), tracers (move), casings (move,
   bounce, sound), character effects (pending dings play, footstep keys of
   others' animations).
4. Effect groups update (particles, sprites, lights, camera jitter).
5. Render. Then the mixer updates the listener and every playing 3D sound's
   gain and low-pass.

One weapon effect (client), in order:

1. Visibility gates (section 2); liquid tests at impact + normal and at
   fire point + 1 u along the shot.
2. Fire effects (unless suppressed): muzzle flash if the fire point is
   seen; casing if seen and near; fire sound (not for your own shot).
3. If this is the server's echo of your own non-thrown shot: queue the
   ding and stop (critical hits continue).
4. If not sky: impact sound; ammo effects; if the impact is seen, decal
   then surface burst (and exit decal/debris, Edge cases).
5. Bubble trail if the fire point is in liquid (bullet ammo).
6. Tracer if its bit is set, `Tracers` is on and the impact is not in
   liquid.
7. Fly-by check (others' bullet shots).

## Edge cases

- **Surface with no type** (0): no decal, no burst, stone impact sounds
  with a 25 % chance of silence, tile footsteps, `Grenade_Dirt` explosion,
  silent grenade bounces (its `GrenadeImpactSnd` is empty), casings play
  `linoleum.wav`.
- **Surface id not in the table**: treated as Unknown (0).
- **Sky**: no impact sound, no effects, no decal (no stock ImpactFX has
  `ImpactOnSky`); tracers still fly to the ray's end. A grenade detonating
  on sky makes no sound at all.
- **Effect group name not in the effect file**: nothing is shown (the C and
  Red silencers' flashes).
- **Many impacts at once** (shotgun): every pellet is a full weapon effect
  (sound, decal, burst); only the first carries the fire effects and the
  tracer. Ten pellets on one spot add decals until 10 are within 10 u,
  then move existing ones. Ten impact sounds start at once (no limit).
- **Sound beyond range**: started anyway (the "play only if heard" option
  `SoundPlayOnlyIfHeard` is off); muted by the audio library while beyond R
  and recomputed when someone moves into range.
- **Sound starting inside the inner radius**: full gain; if the listener
  then moves out past r the formula takes over.
- **Shot from inside a wall**: an exit decal can appear only when the 2 u
  segment centred on the **fire position** crosses a shoot-through surface
  that shows decals, because the exit test is made at the muzzle, not at the
  impact. Exit debris and the light shaft through thin walls are off in all
  stock FireFX records.
- **Your own far shot**: a decal beyond 1000 u appears only while zoomed;
  the burst and sound still play.
- **Impact behind you by another player**: seen only within 300 u; a frag
  exploding behind you farther away shows no explosion even if you turn at
  once (its sound plays).
- **Mid-air frag**: full look, by the surface of its last bounce or its
  own surface (Corrections 2026-10-22, 1). Mid-air smoke and flash: full
  look (their groups come from the ammo).
- **Grenade in water**: impact point + normal inside liquid → under-water
  records (none for grenades) and half-volume sound; on the water surface
  from above → `Grenade_Water`.
- **Paused**: decals, casings and effects stop ageing.
- **Dead or spectating**: no own footsteps; others' footsteps still play;
  your muzzle flash is hidden while dead unless you chase-cam another
  player, whose silencer then decides your flash and fire sound.

## Quirks

1. **Tracers ignore depth**: withdrawn (Corrections (2026-10-30) 3):
   tracers are hidden by walls like other world geometry; they only do
   not write depth, so they never hide each other or later effects.
2. **Silent walkers**: others' walking and crouching animations have no
   footstep keys, and your own steps stop while the run flag is off. Walk
   or crouch to move silently. Keep.
3. **Footsteps and shots through walls**: no occlusion; the only falloff is
   distance (footsteps fade out at ≈ 1119 u, an M4A1 at ≈ 1865 u). Keep.
4. **Muzzle and explosion lights give positions away**: every unsilenced
   (and A/B silenced) shot lights the world within 170 u for 50 ms at
   medium/high detail, and engine lights have no shadows, so they also
   light the near side of a wall the shooter stands behind. Keep.
5. **Squared-distance gain**: gain falls with d², and reaches 0 at about
   93 % of the radius. Keep (it sets how far things are audible).
6. **Fly-by played near the world origin**: the fly-by position is computed
   as the camera position **normalised to length 1** plus the vector from
   the camera to the nearest point of the shot line. It therefore lands
   within about 600 u of the world origin, not near you, and with R = 300
   is effectively inaudible except near the origin. Recommendation: play it
   at the nearest point of the line; offer the original as an option
   (Open questions 2).
7. **Off-screen particle systems freeze**: a particle system that was drawn
   once and is not drawn in a frame skips its update, so its particles stop
   moving and ageing until seen again. A smoke cloud you are not looking at
   does not age. Keep, but check (Open questions 5).
8. **Muzzle flash length**: always 50 ms; automatic fire flickers. Keep.
9. **Detail value 3**: the weapon-effect code's detail value is 3 in
   release builds, so its "low" sound thinning (half or a third of impact
   sounds) and "medium" bubble count never apply, and blast lights (high
   only) never appear. Keep the "3" behaviour.
10. **No pitch variation**: pitch values never reach the mixer. Keep.
11. **Hit-confirm delay inverted**: the ding is meant to wait `DingDelay`
    (1 s) but the check fires while its time is still in the future, so it
    plays on the next character update. Keep.
12. **Quiet movement factor unused**: the 0.65 "moving quietly" factor
    applies only to your own steps while walking or crouched, which are
    already silent. Omit.
13. **Dry fire heard far** (withdrawn): the radius rule would apply if a
    dry-fire sound were ever sent, but no dry-fire sound is played in this
    build (section 11). Nothing to implement.
14. **Server-wide hurt limit**: one hurt sound per kind across the whole
    server; while one player's `DeathbyGuns` plays, nobody else's does.
    Keep.
15. **Casings ride along**: first-person casings are carried with you while
    they fall. Keep.
16. **Empty sound slots**: random slot choice includes undefined slots
    (impact sounds silent 25–50 % of the time on some surfaces). Keep.
17. **Compounding casing spin**: a casing's orientation is turned each
    frame by its accumulated spin angle, so it tumbles faster with age and
    with frame rate (Corrections 2026-10-22, 11). Keep.

## Corrections (2026-10-22)

A second pass over the reference's client weapon-effect, casing, tracer,
character-sound, view-bob, sound-manager and kill-message code and the
server's projectile, weapon and player-damage code, compared with this
spec and with our implementation (`games/combat_arms/src/game/effects/`,
`games/combat_arms/src/ui/`). Each entry: what the original does, what ours
does (file:line), and a testable expectation. Ordered by what a player
would notice first. Where the body above disagrees, the entry here wins;
section 9, the Edge cases and test case 32 are amended in place.

1. **A grenade that goes off in mid-air still shows its explosion**
   (section 9, Edge cases "Mid-air frag", case 32).
   - Original: a grenade detonates only at its fuse, and the server then
     names the grenade itself as the thing it "hit". So the null-object
     branch that turns an unknown surface into Air is never taken for a
     thrown grenade. The surface sent to clients is the surface of the
     grenade's **last bounce or touch**; a grenade that never touched
     anything gets its own object surface (`grenades.md` 8: metal), never
     Air. The normal is always world up. Clients therefore create that
     surface's grenade group (`GrenadeFXName`) whenever the impact is seen,
     mid-air included. A mid-air frag that never bounced shows Metal's
     `Grenade_Stone`. One that last bounced off a wooden wall shows Wood's
     `Grenade_Dirt`, even though it is now far from the wall.
   - Ours: `grenade_effects` takes the nearest tagged triangle within 16 u
     of the blast and otherwise uses Air (1), which has no grenade group, so
     a mid-air frag makes only its sound (`effects/mod.rs:1010`). The body
     of this spec said the same; that was wrong.
   - Expect: an M67 thrown straight up in the open, exploding before it
     lands, 500 u in front of you → `gren.wav` (gain 0.8772) **and**
     `Grenade_Stone` with its camera jitter (39 u at the start). An M67
     that bounces once off a Wood wall (30) and goes off 200 u from it in
     the air → `Grenade_Dirt`. A frag resting on Desert_Sand (60) after
     bouncing there → `Grenade_Sand`. The effect frame's +Z is world up in
     every case.

2. **Fly-bys are also heard for shots that hit nothing** (section 11
   Fly-by).
   - Original: every weapon effect from another player's bullet weapon
     runs the fly-by test after the impact part, whether or not the ray
     hit anything. A miss is a sky impact at the ray's end, and the 0.75 ×
     sound-radius guard measures to that end point.
   - Ours: `shot_effects` leaves the shot at `let Some(hit) = hit else
     { continue }` (`effects/mod.rs:743`) before the fly-by test
     (`effects/mod.rs:780`). Shots into the sky or out of the map, and
     rounds absorbed by a spawn-protected player (the `continue` right after
     it), never make a fly-by.
   - Expect: an enemy 1500 u away fires over your head into the open sky,
     the shot's line passing 200 u from your camera → one `bulletflyby`
     sound (played as `original_flyby` says). The same shot hitting a wall
     2000 u behind you → also one fly-by (as now).

3. **Own tracer cadence comes from the last fitted part, tracer or not**
   (section 4, case 12, Open question 14).
   - Original: the shooter's client walks the five slots in order (muzzle,
     scope, magazine, bullet, mechanical sight) and keeps the tracer record
     of **every fitted part it passes**, so a later fitted part with no
     tracer resets the choice to none. If the last fitted part names a
     tracer, its `Frequency` is used; otherwise the ammo's. The look is
     still the muzzle part's tracer, else the ammo's, and the server's
     cadence is still the ammo's.
   - Ours: `own_tracer_frequency` keeps the last fitted part that **names**
     a tracer (`effects/mod.rs:425-436`).
   - Expect: M4A1 (ammo AR02, F 4) with an A silencer (AR01, F 3) and a
     magazine part that names no tracer → own tracers on shots 4 and 8
     (F 4), with the AR01 look. With the A silencer alone → shots 3, 6, 9
     (as now). With a sight (AR01) and then a bullet part that names no
     tracer → shots 4, 8.
   - Not determined: whether the mechanical-sight slot always holds a part.
     If it does, part tracers never set the own cadence.

4. **Others hear a weapon's misc key sounds to 2000 u** (section 11,
   "Reload / draw keys").
   - Original: view-model `sound_key` 10–14 (the weapon's misc sounds) go
     to the server like reload and draw keys, and other clients play them
     through the weapon-sound routine. For misc sounds it sets no radius,
     so the weapon default of **2000 u** applies. Reload keys use
     `ReloadSndRadius`, select and deselect `SelectSndRadius`. No key sound
     of any kind is sent while the player is dead.
   - Ours: `key_sound` gives misc keys the reload radius
     (`effects/data.rs:398`).
   - Expect: another player's misc key sound at 1000 u → gain 0.7166 (R
     2000, r 140), not silent. A reload key at the same distance → silent
     (R 300).

5. **Your own footstep rhythm restarts after you stop** (section 11,
   Footsteps "You").
   - Original: the sway phase advances every frame while the view bob's
     amplitude is non-zero. The bob ramps from 0 to full over 0.1 s once
     you move and decays to 0 over 0.1 s once you stop, each on a smooth
     curve. So the phase keeps running for that 0.1 s after you stop, and
     is reset to 0 on any frame the amplitude is 0. Foot crossings during
     the decay use up that foot without a sound. On a new start the
     amplitude is still 0 for the first two frames, so the phase starts
     moving on the third frame. The immediate start step comes from the
     movement **key** being pressed (the control flag turning on, or
     crouch pressed from a standstill: silent, the run flag being off),
     not from actual displacement.
   - Ours: `OwnSteps::step` keeps the phase across stops and advances it
     only while moving (`effects/rules.rs:538-576`). The start step needs
     more than 0.1 u of movement with a key held (`effects/mod.rs:1218`).
     After a stop the first periodic step can come one frame after the
     start step, and pushing into a wall from rest makes no start step.
   - Expect: run, stop for 0.5 s, run again at 60 fps → a step at once,
     the next 0.179 s + 2 frames later (≈ 0.21 s), then every 0.357 s.
     Stop for 0.05 s and go again → no reset (the phase carries on). From
     rest, hold forward while facing a wall → one step on the press.

6. **Ghillie-type models' footsteps are at half volume, and every step
   sounds from the body's centre** (section 11, Footsteps; case 4).
   - Original: after the perk adjustments, the step volume gets the
     model's `FootprintSndVolume` share added (volume += volume × share;
     −0.5 for the 65 ghillie-type models). Others' steps play at the
     character object's position (the centre of its box). Only the surface
     ray starts at the foot node.
   - Ours: no model share (`effects/mod.rs:1268`; it is not read
     anywhere), and the sound plays at the bottom of the hull
     (`effects/mod.rs:1194`).
   - Expect: case 4. A running ghillie enemy at 500 u on Metal → gain
     0.4025 (ours today 0.8049). A step from a player whose centre is 500 u
     from your camera is heard at d = 500, not at the distance to their
     feet.

7. **Awareness adds the perk's own sound and overrides Specter**
   (`gear_perks.md` Awareness, section 11 Footsteps).
   - Original: when the listener's Awareness is on and the walker is on
     another team, the step volume and radius are scaled by the perk and
     the walker's Specter quieting is **not** applied: the perk routine
     returns before it. The extra sound is one file drawn at random from
     the perk's own effect-sound list (four files), not a footstep. It is
     played at the step's volume **before** the perk scaling, with the
     perk-scaled radius, at the same position.
   - Ours: the extra sound is the surface's footstep again, and Specter ×
     Awareness are multiplied (`effects/mod.rs:1268-1285`).
   - Expect: a Specter enemy (`FootSoundVolume` v) heard by an Awareness
     user (1.0, radius × 0.9) → the step at volume 100 × 1.0 (not × v),
     radius 1080, plus one of the perk's four sounds at volume 100, radius
     1080. The same enemy heard without Awareness → volume 100 × v.

8. **Missing local voice sounds: kill, sprint, idle, heartbeat** (section
   11; `hud_crosshair.md` D2).
   - Original: the client has a local auto-cast list, played in your head
     in your model's sound template. In order its entries are dash, enemy
     kill, medic kit, idle, infected, stamina, first-aid kit, jump and
     heartbeat; this spec has only jump, medic kit and stamina. Four more
     are played:
     - **Enemy kill**: when a kill message names you as the killer and the
       killing hit was not a groin hit (no check for suicide or team kill),
       one file of the kill entry, unless the previous kill voice is still
       playing.
     - **Sprint**: every HUD frame while you are sprinting, the dash entry
       starts again as soon as the previous one has ended, so it repeats
       for as long as you sprint.
     - **Idle**: while your control flags are exactly "run" (no movement,
       jump, crouch or walk key) and movement is allowed, a 5 s timer runs.
       When it expires the idle entry plays and then repeats back to back.
       Any other flag stops the timer and kills the playing idle sound. An
       empty entry stops the timer.
     - **Heartbeat**: the low-health beat of `hud_crosshair.md` D2 plays the
       heartbeat entry.
   - Ours: none of the four is played. The heartbeat is counted with no
     file (`ui/hud.rs:107-108`, `ui/hud.rs:961-964`), and the effects module
     has no kill, sprint or idle sound.
   - Expect: kill an enemy with a body shot → one kill voice, local. A
     second kill 0.2 s later while it still plays → none. A groin kill →
     none. Sprint for 3 s → the dash sound repeated back to back for 3 s.
     Stand still with no keys for 5 s → the idle sound starts at 5.0 s and
     stops the frame you press a key. At 25 % health → heartbeats ≈ 1 s
     apart, each a file of the heartbeat entry.

9. **Hurt sounds sound from the victim's centre** (section 11, Hurt).
   - Original: both hurt sounds play at the victim object's position, not
     at the hit point. Each kind takes its own radius from its 3D auto-cast
     record. Burn, poison and bleed damage count as a torso hit, so they
     never add a head or groin sound. Bleed uses the burn kind and shares
     its one-at-a-time limit.
   - Ours: plays at the damage point (`effects/mod.rs:1150`), with one
     shared radius.
   - Expect: a head hit on a victim whose centre is 400 u from you and
     whose head is 370 u from you → both sounds at d = 400. Burning damage
     to the head → only the burn sound.

10. **Shotgun pellets each re-create the shot's fire effects** (section 1;
    Edge cases "Many impacts at once").
    - Original: after the first pellet, the shooter's client and the server
      mark the fire sound, casing, light, muzzle and tracer as ignored for
      the other pellets of the round. The flag that gates the whole fire
      part is not among them. So every pellet's weapon effect creates the
      muzzle group again at the same point: the third-person muzzle for
      others, your own FireFX group (`test` smoke and its 256 u light) for
      you. The flash groups' own lights are part of those groups, so they
      stack too. The fire sound plays once; the casing and tracer come once.
    - Ours: one muzzle or FireFX group per round (`effects/mod.rs`
      `Fired` handling).
    - Expect: an enemy's 8-pellet SPAS-12 shot seen from the front → 8
      `WEAPON_HH` groups created in the same frame at the muzzle (additive,
      so brighter), one fire sound, one casing. Your own shot → 8 `test`
      smoke puffs.
    - Not checked against a running game; low priority.

11. **Casings tumble faster and faster** (section 8; Quirk).
    - Original: each frame, until the casing rests, its current orientation
      is turned about its own up axis by the yaw angle **accumulated so
      far**, then about its right axis by the accumulated pitch. The
      rotation therefore compounds: the turn per frame grows with age, and
      the visible spin depends on frame rate. On coming to rest it turns by
      the accumulated yaw once more. Keep (Quirk 17).
    - Ours: the orientation is the accumulated angles themselves
      (`effects/rules.rs:442`, `effects/mod.rs:1358`): a steady spin.
    - Expect: yaw spin 6 rad/s at 60 fps → after 0.1 s the casing turns
      about 0.6 rad per frame (≈ 36 rad/s); after 0.2 s about 1.2 rad per
      frame. Ours: 6 rad/s throughout.

12. **Third-person flash-hider groups** (section 3).
    - Original: when the muzzle part names no third-person group of its
      own and has `FlashHiderLevel` h in 1 … (max − 1), others see the
      weapon's third-person flash-hider group h−1 instead of
      `HHMuzzleFXName`. This mirrors the first-person rule already
      implemented.
    - Ours: `hh_muzzle` ignores the flash-hider level
      (`effects/mod.rs:450`).
    - Expect: unused by stock parts (no stock muzzle part sets a level), so
      no visible change with stock data. A data test with h = 1 → the
      weapon's first third-person flash-hider group.

13. **Hit-confirm dings on teammates** (Open question 7, answered).
    - Original: the shooter's client does not skip teammates in its hit
      test, and the server clears the ding only when the hit object is not
      a character (or the game is not multiplayer). So a hit on a teammate
      dings, with friendly fire on or off.
    - Ours: `Hit` events are sent for teammates (`game/weapons.rs:2896`),
      so the ding plays. No code change; the open question is closed.
    - Expect: friendly fire off, shoot a teammate → one ding, no damage.

Cases 58–66 below test entries 1–9.

## Corrections (2026-10-30)

From the client effect manager's group update (looping, hiding and
suspension), its two sound keys (`PlaySound`, `PlayRandomSound`), the
client weapon's fire step and attached-effect handling, the shared
weapon-sound player, the server weapon's fire handling, the tracer and
poly-line effects and the engine's primitive renderer. They answer
`view_models.md` attached-effect questions 2 and 3 and the melee-sound
question of the Combat Arms plan and the backlog's tracer questions. Where the body above disagrees, the
entry here wins; "The effect file", sections 10 and 11 are amended in
place.

1. **How long a sound key's sound lives** ("The effect file"; replaces
   "whether a looping one is cut at the key's end was not traced").
   - Original: a `PlaySound` or `PlayRandomSound` key starts its sound on
     its first update after becoming active, once per activation. Its
     sound is stopped:
     - **at the key's end** (the update in which the group's clock passes
       the key's end time), whatever its `Loop` setting and however long
       the file is. So a non-looping file longer than its key is cut too,
       and a looping file sounds exactly as long as its key. In a looping
       group the key starts again from the beginning of the file at its
       next start time, once per pass;
     - **while its group is hidden** (an attached group hidden by
       `HIDE_PVATTACHFX` or with its weapon model, `view_models.md` 3):
       the sound is stopped at once and
       the group's clock is frozen (no time passes for it, its keys keep
       their place in the pass); when the group is shown again its active
       sound keys start their files **from the beginning**. A file never
       pauses and resumes;
     - when the group is destroyed or shut down.
   - A key that lasts its whole group (length ≥ group length − 0.01 s) in
     a **looping** group never ends: at the end of each pass its time is
     reset but it is not restarted, so its sound started once keeps
     playing. With `Loop` Yes the file loops by itself for as long as the
     group lives and is shown (the chainsaw idle: `IDLE.WAV` loops
     seamlessly, not restarted every 3 s). With `Loop` No it plays once
     and the key then stays silent until the group is hidden and shown
     again or created anew. In a non-looping group the same key ends at
     the group's end and its sound is cut there.
   - A sound key that starts while the game window does not have focus
     plays nothing for that activation (no retry until the key starts
     again or the group is hidden and shown).
   - Ours: attached groups' looping sound keys play while shown, stop
     while hidden and start from the beginning when shown
     (`game/view_model/attached.rs`), which matches; the freezing of the
     group's clock while hidden and the key-end cut of other groups' sound
     keys are to be checked against this entry.
   - Expect: test cases 67–70.

2. **Alt-fire sound and the fire voice** (sections 10 and 11).
   - Original: the attacker's own client plays `Alt_FireSnd` instead of
     `FireSnd` for every shot or melee hit of a swing started with the
     secondary button (a silencer-type part with a silenced file wins over
     both). The event the server relays is marked alt-fire, but other
     clients end up playing `FireSnd` for it (Quirk:
     `special_weapons.md` Quirk 10). A fire or alt-fire start first stops
     the same shooter's still-playing fire or alt-fire sound of the same
     weapon (one shared voice for both buttons); silenced fire sounds
     are never stopped and never stop one. Each melee hit key is one such
     start (`special_weapons.md` 5.1).
   - Ours: each knife hit plays `FireSnd` on the player's fire channel,
     replacing the last (`game/effects/mod.rs`), so the voice rule
     matches; the secondary uses `FireSnd` instead of `Alt_FireSnd` on the
     attacker's own client.
   - Expect: test cases 71 and 72; `special_weapons.md` cases 40–47.

3. **How a tracer strip is drawn** (section 4, Quirk 1; the backlog's
   tracer questions).
   - Original, orientation: each frame both ends of the strip are taken
     into camera space (camera at the origin, x right, y up, z forward).
     With Δ = (x₂ − x₁, y₂ − y₁) the difference of the two ends' camera
     x and y (no perspective divide; depth ignored), the width direction
     is n = (Δy, −Δx, 0)/|Δ| in camera space: perpendicular to Δ and lying
     in a plane parallel to the screen. Both ends use the same n (one
     segment, shared). Each end gets two corners, end ± (w/2)·n, mapped
     back to the world. So the strip always faces the camera squarely
     (never seen edge-on, never twisted between its ends); a strip that
     points straight at the eye (Δ = 0) has no defined width direction
     and is not seen.
   - Width: the half-width at each end is **Width/4**, so the strip is
     **w = Width/2 world units** across (AR02: 7.5 u, AR01: 12.5 u, SMG02:
     15 u). It is a world-space width: it shrinks with distance by
     perspective and grows by the same factor as everything else when the
     field of view narrows (zoom); there is no floor, cap or zoom
     compensation.
   - Texture and colour: texture u runs 0 → 1 from the tail to the head
     (the whole texture stretched over the strip's current length, not
     tiled), v 0 → 1 across. The draw takes colour **and** alpha from the
     texture alone, so the record's `Color`, `InitialAlpha` and
     `FinalAlpha` and the vertices' edge fade have no visible effect; the
     tracer does not fade out over its life. It is not additive; it does
     not set a blend mode of its own and draws with whatever the client's
     primitive drawing last set, in practice ordinary alpha blending with
     the texture's alpha (Open question 30).
   - Depth: tested against the scene, not written. A wall between you and
     a tracer hides it; tracers do not hide one another.
   - Start point of your own tracer: the world flash point F of
     `view_models.md` 4 (camera + `Pos` + `MuzzlePos` + bob + the shot's
     recoil offset, in camera axes), from data, not from the model's
     socket. It is computed the same way while zoomed, and nothing hides
     the tracer while scoped: a scoped shooter's own tracer is drawn,
     starting at that same camera-relative offset (beside and below the
     eye, outside the scope picture's centre) and running to the impact.
     Others' tracers start at the fire position of the server's event.
   - Ours: each end turned about the strip's axis toward the eye from its
     own place, full `Width`, colour and alpha applied, no depth test
     (`ui::effects::strip_corners`); start at the view model's flash
     point.
   - Expect: test cases 73–76.

## Follow-up (2026-10-31): trail keys and smoke-trail puffs

From the effect library's trail key, the base effect's colour curve, the
client's particle smoke trail and its segments, the engine's particle
system and particle drawing, and the install's effect file and archives.
Answers the open question (a) of brief 24b in the Combat Arms plan ("how
the trail key draws") and why our smoke-trail puffs look too small. Where
"The effect file" above disagrees, this section wins.

1. **The trail key (`PolyTrail` in the file).**
   - Properties and editor defaults: `Texture`, `TrailWidth` W (20),
     `WidthStyle` (0 Constant, 1 SmallToBig, 2 SmallToSmall, 3 BigToSmall;
     default 0), `TrailLen` N (50, the most points the trail may hold),
     `SectionLifespan` L (1.0 s), `UAdd` (0), `SectionInterval` I (0.1 s),
     `BlendMode` (0 none, 1 add, 2 saturate, 3 by source alpha, 4 by source
     colour, 5 by destination colour, 6 source × destination colour,
     7 source alpha + one, 8 source alpha, 9 source colour + one,
     10 destination colour × zero), `AlphaTest` (0 none, 1 <, 2 ≤, 3 >,
     4 ≥, 5 =, 6 ≠), `ColorOp` (0 no texture, 1 modulate, 2 additive,
     3 no colour), `FillMode` (0 wire, 1 fill), `Allignment` (sic; 0 Camera,
     1 Up, 2 Right, 3 Forward). A key with N = 0 or W = 0 is not created.
   - Stock data (200 trail keys): every one has `BlendMode` 1 (additive)
     and `Allignment` Camera; `WidthStyle` BigToSmall 142, Constant 52,
     SmallToBig 6; `UAdd` 0 in 187, 1.0 in 12 (the cinematic `Cine_A10*`
     groups and `Snow_Tail_New`), 0.1 in one (`Cine_F16`).
   - **Points.** The trail is a list of points, oldest first. It is empty
     when the key becomes active (the list is cleared whenever the key is
     hidden, and the effect manager hides every key once right after making
     it). Each update, with Δ the frame time in seconds:
     1. E += Δ (E starts at 0). If the key is not shutting down, the list
        holds fewer than N points, and (E > I or the list holds exactly one
        point): append a point at the key's current position (after its
        placement mode has moved it) and set E = 0. So the first point
        comes in the first update where E exceeds I, the second in the very
        next update, then one in each update where E exceeds I again (with
        fixed frames: every k-th update, k the smallest whole number with
        k·Δ > I).
     2. While not shutting down, the newest point is moved to the key's
        current position every update: it follows the emitter, and the
        older points stay where they were left.
     3. Every point's age grows by Δ (a point made this update included).
     4. The width and colours below are computed for all points, then every
        point except the newest whose age is ≥ L is removed. Drawing uses
        what remains; with fewer than 2 points nothing is drawn.
   - **Width.** One half-width w applies to the whole strip at a time (it
     does not vary along the trail); it changes over time with n, the
     number of points at step 4 before removal:
     - Constant: w = W;
     - SmallToBig: w = W·(n − 2)/N;
     - SmallToSmall: w = W·(n − 2)/N while n < (N + 1)/2, else
       W·(N − (n − 2))/N;
     - BigToSmall: w = W·(N − (n − 2))/N.
     The strip spans w to each side of the path, so it is 2w wide. A trail
     whose length is held steady by the lifespan keeps a steady width (for
     BigToSmall just under W, for SmallToBig a small fraction of it).
   - **Shape (Camera alignment, every stock key).** The points are taken
     into camera space; for a segment from a to b the side direction is
     (b.y − a.y, −(b.x − a.x), 0) (perpendicular to the segment within the
     camera's view plane, depth ignored). The first and last points use
     their one segment's direction, every inner point the average of its
     two segments' directions; each is scaled to length w. Each segment is
     a quad a + s_a, b + s_b, b − s_b, a − s_a (two triangles). Two-sided,
     depth-tested, not written to depth. Up, Right and Forward alignment
     (unused in stock data) use instead the key's own up, right or forward
     axis at the moment the point was made, in world space.
   - **Texture coordinates.** v = 0 on the a + s side, 1 on the a − s side.
     u of the first point is 0; each new point gets u = u_prev +
     |p − p_prev| / W, measured to the previous newest point where it was
     at that moment (the following newest point keeps its u while it moves,
     so the last segment's texture stretches until the next point is
     made). The texture therefore repeats once per W u of path length, not
     once over the whole trail. Every drawn u has an offset O added, where
     O += `UAdd`·Δ each update: `UAdd` is in texture repeats per second,
     and a positive value moves the pattern from the newest point toward
     the oldest (away from the emitter) at `UAdd`·W u/s.
   - **Colour.** Each point has its own colour from the key's colour keys
     (`Ck`) evaluated at (age / L) mod 1 (alpha stored inverted, as
     everywhere in the file), and each segment blends linearly between its
     two points' colours. So the curve runs over each section's own life,
     not over the key's life: a curve ending black (with additive blend) or
     transparent fades every section out as it ages, and the newest section
     always shows the start of the curve. The key's scale keys (`Sk`) do
     nothing for a trail.
   - **Ending.** When the key shuts down (smooth shut-down) no points are
     added and the newest point stops following; the points age out and
     the trail disappears once fewer than 2 remain. Without smooth
     shut-down the key is removed at once.
   - Stock example `Eye_Red_Tail_L` (the infected's eyes): W 2, BigToSmall,
     N 50 (25 for its low-detail copy), L 0.2 s, I 0.03 s, `UAdd` 0,
     additive, `ColorOp` modulate, texture
     `FX\WEAPONS\GRENADE_TAIL\32XTAILBASE01.DTX`, colour keys (time, r, g,
     b, stored alpha): (0, 1, 0, 0, 0), (0.0347, 1, 0.502, 0, 0), (0.1025,
     1, 0, 0, 0), (0.6661, 1, 0, 0, 0), (1, 0, 0, 0, 0): opaque red with a
     brief orange flash at birth, fading to black (invisible under additive
     blend) over each section's last third.
   - Ours (per the plan): constant width, the texture once from tail to
     head, the key's colour at key time, no fade. Each of those differs.
   - Expect: test cases 77–81.

2. **Smoke-trail puffs** (projectile `SmokeTrail`, `special_weapons.md`
   Follow-up 3 and its Corrections (2026-10-31) 2, which these rules
   refine).
   - **Size.** Each puff is a camera-facing square of side 4·s u, with
     s = (2000 / P) · k, where P is the width of the game's screen in
     pixels (the trail's particle radius 2000 is divided by it once when
     the segment is made) and k the puff's scale factor. At 1024 pixels
     wide s starts at 1.953 (a 7.8 u square); at 1920, 1.042 (4.2 u).
   - **Growth.** k = 1 + 14·f and the puff's own alpha is 0.9·(1 − f),
     where f = 1 − (life left / total life) of **that puff**, not of its
     segment. Every puff gets a total life of 10 s (5 s at the "high"
     special-effects detail, which the release build never has), plus a
     tiny extra per emission step: of the S steps one update emits (S = 2
     in the release build), step i (0-based) gets 10 + i·Δ_e/S s, Δ_e the
     time since that segment's previous emission. The segment is removed
     long before its puffs would die, so a puff only ever reaches about
     f ≈ 0.17 (k ≈ 3.4, a 26 u square at 1024 pixels) on a long white
     trail and f ≈ 0.10 (k ≈ 2.3) on a short one.
   - **Segment alpha.** A segment emits on every update for its first
     emitting time (0.2 s white, 0.3 s black) after it was made; after
     that it stops and its whole particle system's alpha falls linearly
     from 1 at its last emission to 0 one segment life (1.5 s long white,
     0.75 s short) later, when the segment is removed. A puff's drawn
     alpha is its own alpha × the segment's alpha.
   - **Look.** The three sprite files the trail names
     (`SFX\Smoke\Spr\WhiteTrailLong.spr`, `WhiteTrailShort.spr`,
     `BlackTrailShort.spr`) are **not in the install** (no `SFX` folder
     ships at all), so the particle system has no texture and each puff is
     drawn as a plain untextured square in its vertex colour (Open question
     31): a grey between 150/255 and 230/255 (one random value, the three
     channels equal), ordinary alpha blending. The black type is grey too
     (only its sprite would have made it dark).
   - Under water (the segment starts inside a liquid volume): particle
     radius 500 instead of 2000, 3 puffs per step, the particle system's
     gravity set to 5 instead of 0, no wind, the default bubble texture and the liquid's
     colour range.
   - Expect: test cases 82–83.

## Follow-up (2026-10-31b): prop debris, empty texture slots, surface 0

From the reference's debris table reader, the destructible aggregate's death handling, the client's debris effect and its moving-object physics, the renderer's per-stage texture binding, the server's bullet impact and shoot-through handling and its surface lookups, and the install (Steam build 26236938: `ATTRIBUTES\DEBRIS.TXT`, `ATTRIBUTES\SURFACE.TXT`, all 105 world files). Answers Open questions 15 and 31 and gives `world_objects.md` its debris looks (its Follow-up (2026-10-31b) 4). Test cases 84–96.

1. **Prop debris** (a destructible world model's `DebrisType`).
   - **The table** `ATTRIBUTES\DEBRIS.TXT`: records `[Debris0]`, `[Debris1]`, … read until one is missing; a record's number is its id. Keys: `Name`, `SurfaceId`, `MinScale`/`MaxScale`, `MinVel`/`MaxVel`, `MinWorldVel`/`MaxWorldVel` (vectors, default 0), `MinDebrisOffset`/`MaxDebrisOffset`, `MinLifetime`/`MaxLifetime`, `Fadetime`, `Number`, `Rotate` (an integer, non-zero = on), `MinBounce`/`MaxBounce`, `GravityScale`, `Alpha`, `Model1`…`Model5`, `Skin0…` and `RenderStyle0…` lists, `BounceSnd1`/`BounceSnd2`, `ExplodeSnd1`/`ExplodeSnd2`, `WorldSpaceFX`, `ImpactSpaceFX` (effect group names). A map object names a record by `Name`, case ignored.
   - **The install's six records** (paths as stored; vectors are (right, forward, up) for `MinVel`/`MaxVel` and world (x, y, z) for the others):

| Id | `Name` | Surface | Scale | `MinVel` .. `MaxVel` | World vel | Offset | Life, fade (s) | Number | Rotate | Bounces | Gravity × | Alpha | Models | Skin, style | Bounce / explode sounds (`FX\Debris\Snd\…`) |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| 0 | `None` | 0 | 1–1 | 0 .. 0 | 0 | (10, 10, 10) | 0, 0 | 0 | off | 0–0 | 1.0 | 1.0 | none | none | none |
| 1 | `Window` | 40 | 0.5–1.2 | (−800, −500, −800) .. (800, 800, 800) | 0 | (10, 10, 10) | 4.0–5.0, 3.0 | 30 | on | 10–15 | 2.0 | 0.7 | `FX\Debris\Models\glass_01.ltb`, `glass_02.ltb`, `glass_03.ltb` | `FX\Debris\Skins\glass_01.dtx`, `RS\translucent.ltb` | `Smallglass\gib_1.wav`, `gib_2.wav` / `Smallglass\glass_crash1.wav`, `glass_crash3.wav` |
| 2 | `Window_smoll` | 40 | 0.2–0.5 | (−400, −250, −400) .. (400, 400, 400) | 0 | (10, 10, 10) | 0.8–1.0, 2.0 | 15 | on | 10–15 | 1.0 | 0.7 | as `Window` | as `Window` | `Smallglass\gib_1.wav`, `gib_2.wav` / `Smallglass\explode_1.wav`, `explode_2.wav` |
| 3 | `Window_Large` | 40 | 4–7 | (−600, −400, −600) .. (600, 400, 600) | 0 | (−30, −30, −30) .. (30, 30, 30) | 0.5–0.7, 2.0 | 10 | on | 15–20 | 1.0 | 0.7 | `FX\Debris\Models\GLASS_02A.ltb`, `GLASS_02B.ltb`, `GLASS_02C.ltb` | as `Window` | as `Window` |
| 4 | `56x56WoodBox` | 30 | 5–8 | (−150, −150, 300) .. (150, 150, 500) | 0 | (−28, −28, 10) .. (28, 28, 50) | 1.0–1.2, 1.0 | 6 | on (stored as 360) | 10–15 | 3.0 | 1.0 | `FX\Debris\Models\WOOD1A.ltb`, `WOOD1C.ltb` | `FX\Debris\Skins\WOOD.dtx`, `RS\DEFAULT.ltb` | `Wood\gib_1.wav`, `gib_2.wav` / `Wood\explode_1.wav`, `explode_2.wav` |
| 5 | `Stone` | 54 | 4–8 | (−200, −50, −200) .. (200, 300, 200) | (−50, 100, −50) .. (50, 300, 50) | (10, 10, 10) | 2.0–3.0, 2.0 | 8 | on | 5–8 | 2.5 | 1.0 | `FX\Debris\Models\chunk1.ltb` … `chunk4.ltb` | `FX\Debris\Skins\STONE.dtx`, `RS\default.ltb` | `STONE\gib_1.wav`, `gib_2.wav` / `STONE\explode_1.wav`, `explode_2.wav` |

   No record names a `WorldSpaceFX` or `ImpactSpaceFX`. Users in the stock maps: `world_objects.md` Follow-up (2026-10-31b) 4.
   - **Sending**: on the object's death (after its `Spawn`, before its world-model debris) the server sends an effect message at the object's position (the world model's box centre) carrying that position, the record id and a rotation whose forward is the death direction (the killing damage's direction) with up toward +Y. Every client that receives it builds the effect below.
   - **Count**: N = min(`Number`, 20); then by the console variable `DebrisFXLevel` (default high): low N × 0.333, medium N × 0.666, truncated; at least 1. The lifetimes are scaled by the same factor (low 0.333, medium 0.666).
   - **Each piece i = 1 … N**: model `Model`k with k = ((i − 1) mod the number of models) + 1, the record's first skin and render style; scale U(0.8, 1.2) × U(`MinScale`, `MaxScale`) on all axes; alpha `Alpha`. With F, R, U the message rotation's forward, right and up axes, the velocity bounds are lo = `MinWorldVel` + R·`MinVel`.x + F·`MinVel`.y + U·`MinVel`.z and hi likewise from the `Max` values; each world component of the piece's velocity is drawn uniformly between lo and hi on that component. The start is the message position + (U(`MinDebrisOffset`.x, `MaxDebrisOffset`.x), U(… .y), U(… .z)) in world axes. Life L = U(`MinLifetime`, `MaxLifetime`) (scaled), bounce budget b = a whole number U{`MinBounce` … `MaxBounce`}. With `Rotate`: angular speeds pitch U(−π, π), yaw U(−2π, 2π), roll U(−2π, 2π) rad/s, angles from 0. A piece that starts inside a liquid volume uses the liquid physics below for its whole life; one whose model is missing is not drawn but still counts.
   - **At creation** one of the explode sounds (uniform) plays at the message position, radius 1000, low priority.
   - **Motion**, each client frame of length dt (0 while the game is paused): new position = position + v·dt; then v += ½·`GravityScale`·g·dt with g = (0, −1000, 0) u/s² (liquid: (0, −500, 0)), so the effective fall acceleration is 500 × `GravityScale` u/s² (`Window` 1000, `56x56WoodBox` 1500, `Stone` 1250, the others 500). The segment from the old to the new position is tested against the level's polygons and solid objects (non-solid ones ignored). On a hit with plane normal n: the piece is placed 0.3 u off the hit point along n; v is mirrored about the plane and multiplied by 0.5 (liquid 0.25); the hit is a **ground** bounce when n.y > 0.6. A ground bounce costs one from b, and also puts the piece **at rest** when |v|² < 225 / dt (dt clamped to [0.001, 0.5], a frame longer than 0.5 s counting as 0.1). After any hit, if not in a liquid and b > 0, one bounce sound (uniform of the two) plays at the piece with probability 1/4, radius 1000, low priority; when b ≤ 0 the piece is at rest. A segment hitting **sky** or an **invisible** surface removes that piece at once, and the other pieces skip their motion for that frame.
   - **Spin**: with `Rotate`, each bounce re-draws the three angular speeds; while moving the angles grow by speed × dt and the piece takes rotation R(pitch, yaw, roll); on coming to rest it is set to its yaw alone (lying flat).
   - **Fade and end**: from the effect's first frame, a piece lives L, then, if `Fadetime` > 0.1 s, fades over `Fadetime` (alpha = min(its alpha, time left / `Fadetime`)); it is removed at L + `Fadetime` (no camera check). The effect ends when no piece is left.
   - `None` gives N = 1 with no model and no sounds: nothing visible or audible.
2. **An empty texture slot** (Open question 31; `models_ltb.md` Open question 10). Traced in the renderer: for each texture stage of a render-style pass, the engine binds the model's texture for the slot the stage names; when that slot holds no texture (or the texture cannot be created) it binds **nothing** to that stage, with no substitute or default image, and leaves the stage's colour and alpha operations exactly as the render style sets them (it does not disable the stage). A stage told to use no texture likewise binds nothing. So what an empty slot reads is whatever the graphics API returns for a stage with no texture: on Direct3D 9's fixed-function stages, opaque white, as assumed (that part is the API's, not the game's). One quirk: a texture that exists but has no device image leaves the stage's **previous** texture bound. (The engine also has an optional deferred renderer that puts only the colour map on its first stage; it is off unless enabled and not considered here.)
3. **Surface 0, Unknown** (Open question 15). The install's own table (Steam build 26236938, `ATTRIBUTES\SURFACE.TXT`) has `[Surface0]` `Name` `Unknown`, `CanShootThrough` 1, `MaxShootThroughThickness` 10, `MaxShootThroughPerturb` 10, `BulletRangeDampen` 1.0, `BulletDamageDampen` 0.6, `ShowsMark` 0, stone impact sounds. Its shoot-through surfaces are Unknown (0), Air (1), Flesh (2), Armor (3), Chain Link Fence (9), trussing (23), Wood (30), Plywood (32), Glass (40), Shooting range target's hole (50), paper (53), Boss3_weak (65), Ice (69), "Can shoot through" (76), Cloth (81), Wood Door (100), Water (204), Invisible metal (206), WoodLadder (207).
   - **Where surface 0 comes from**: the server takes a hit's surface from the object hit (its surface flags; the level itself has none) and, if that gives Unknown, from the polygon hit (its stored surface type). Characters' surfaces come from their model data, never Unknown. So a bullet meets Unknown on any solid polygon whose surface type is 0 and on objects with no surface of their own. Install census: 100,922 of 4,057,293 solid physics polygons (2.5 %, in 98 of 105 maps; WAREHOUSE 312 of 32,688, FREEZER 348 of 23,314, JUNK_FLEA 126 of 15,442) and 72,275 of 594,095 solid world-model polygons (12 %) have surface type 0.
   - **What a round does on it** (server ray, `weapon_firing.md` "Penetration"): the impact effect plays with the Unknown look (no bullet hole, stone sounds); a non-level object hit is damaged; then the ray **shoots through**: for a non-level, non-world-model object only if one of its dimensions (2 × half-extent) is below 10; a back-cast from 11 u beyond the entry point to 1 u before it must hit something; the ray restarts from the exit (same surface flags) or the entry, with the range left reduced by the distance travelled (× 1.0), damage × 0.6 from then on (cumulative), and its direction re-perturbed by whole thousandths in −10 … 10 on the right and up axes. So walls and floors of surface 0 up to about 11 u thick are shot through, as `weapon_firing.md`'s table must add (its Follow-up (2026-10-31b)).

## Test cases

Distances are listener (camera) to source unless stated. "Gain" is g(d) of
section 10 with V = 100 and default sliders. Detail: effect detail high.

1. **Gain, M4A1 shot by another player**, R 2000, r 140: d = 100 → 1.0;
   500 → 0.9334; 1000 → 0.7166; 1500 → 0.3553; 1800 → 0.0691; 1900 → 0.
   Zero point 1865.26 u.
2. **Gain, silenced M4A1** (level 1, `SILENCED1.wav`), R 1400, r 98:
   d = 500 → 0.8582; 1000 → 0.4158; 1300 → 0.0087; zero at 1305.68.
3. **Gain, footstep** of a running enemy on Metal, R 1200, r 84: file
   `metal1a.wav` or `metal2a.wav` (right foot) at 50/50; d = 500 → 0.8049;
   1000 → 0.2027; 1150 → 0; through a wall the same values.
4. **Gain, ghillie footstep**: same as 3 with V = 50 → d = 500 → 0.4025.
5. **Gain, server hurt sound**, R 500, r = 17/255·500 = 33.33: d = 100 →
   0.9592; 300 → 0.5918; 460 → 0.0335; a client 1001 u away receives
   nothing.
6. **Gain, bullet impact** on Metal (`metal_bg\impact1..4.wav`, one of four
   uniformly), R 900, r 63: d = 200 → 0.9486; 500 → 0.6488; 800 → 0.0921.
7. **Gain, frag detonation** `gren.wav`, R 1500, r 105: d = 500 → 0.8772;
   1000 → 0.4918; 1400 → 0.
8. **Low-pass**: d = 1000 → c = 1; 1200 → 1; 1350 → 0.6173;
   1500 → 0.2929; 1650 → 0.1; 1800 → 0.1 (only while d < R).
9. **Impact silence**: 10 000 bullet impacts on surface 0 → about 2500
   without sound; on Snow (71) about 5000; on Metal (20) none.
10. **M4A1 tracers, own view**: no attachments, 8 shots → tracers on shots
    4 and 8 (AR02: 15 u wide, 600 u long, 6000 u/s). AK-47 (AR01): shots 3,
    6, 9. M24 (SR01, F = 1): every shot.
11. **Tracer timing**: M4A1 shot hitting 1500 u away → tracer life 0.25 s;
    at t = 0.1 s its tail is 600 u and its head 1200 u from the muzzle; at
    t = 0.2 s tail 1200, head clamped at 1500. A shot hitting 90 u away →
    no tracer.
12. **Tracer with a dot sight**: M4A1 with any sight (sights name AR01,
    F 3) and no muzzle attachment → own client draws tracers on shots 3, 6,
    9 with the **AR02** look; other players see the server's cadence
    (F 4: shots 4, 8) with the AR02 look.
13. **Tracer with A silencer** on M4A1: own cadence F 3 (shots 3, 6, 9) and
    AR01 look (red, 25 u, 300 u); others: shots 4, 8 with the AR01 look.
14. **Pistol with silencer**: Colt 1911 with `Silencer_A_Pistol` (Pisol01,
    F 0) → no tracer on any shot (ammo has none).
15. **Decal size**: M4A1 on Metal (20) → `im_metal`, scale 0.075 ×
    U(0.6, 0.7), width 2.88–3.36 u; on Wood (30) 3.84–4.32 u; on Cement
    (11) 2.88–3.84 u; on Glass (40) 2.88–3.36 u and no burst.
16. **Decal life**: a decal at t = 2.9 s has alpha 1; t = 4.5 s → 0.5;
    t = 6.01 s → removed.
17. **Decal crowding**: 10 decals within 10 u of a point, shoot that point
    → 11 decals. One more shot → still 11, and the closest decal has moved
    to the new point with its old age.
18. **Decal cap**: 101 decals alive, a 102nd anywhere → 101, the oldest
    gone.
19. **No decal on doors client side**: shooting a Wood Door (100) → burst
    `Impact_Wood` and wood sounds, no client decal (`ShowsMark` 0).
20. **No decal on characters**: torso hit → `Impact_Body`, cloth impact
    sound (radius 900), no decal; expected blood stains on the wall behind
    1.0 (2 blobs × 50 %), each 32 u wide, gone after 3.0 s.
21. **Head hit**: `Impact_Head`; light radius rises to 300 u in 0.1 s
    (medium/high); expected stains 4.3.
22. **Muzzle flash, others**: an enemy fires an M4A1 at you from 500 u
    straight ahead → `M4_HH_Muzz` for 50 ms with a 170 u light (none at low
    effect detail). The same enemy 500 u **behind** you → no flash, but the
    fire sound at gain 0.9334.
23. **Silencer flash**: enemy with `Silencer_C_AR` fires → no flash, no
    light; with `Silencer_A_AR` → `WEAPON_HH` with light. Both sound
    `SILENCED1.wav`, R 1400.
24. **Own muzzle**: firing the M4A1 → `AK47_PV_Muzz` visible 50 ms after
    each shot plus the `test` smoke puff (0.5–0.8 s) and a 256 → 170 u light.
25. **Casing**: M4A1 third-person shot seen from 300 u → a 5.56 casing from
    the `Breach` socket, initial speed components up 30–75, right 50–75,
    forward 20–50 u/s; it grows to 2× in 0.5 s, plays `metal.wav`
    (R 200) at its first bounce if that is on Metal and nothing at later
    bounces, and is removed at 3.0 s. The
    same shot seen from 1100 u → no casing.
26. **Casing rest**: at 60 fps a casing reaching a floor at 100 u/s rests
    at once; at 144 fps (threshold 180 u/s) also; at 30 fps (82.2 u/s) it
    bounces (at most 3 times).
27. **Fly-by condition**: another player's rifle shot whose line passes
    400 u from your camera with its impact 2000 u from you → fly-by
    triggered (played per Quirk 6); impact 600 u from you → no fly-by.
28. **Hit confirm**: your M4A1 hits an enemy in a multiplayer match → one
    `MultiDing1.wav` or `MultiDing2.wav` on the frame after the server's
    echo arrives. Eight shotgun pellets confirmed in one frame → 5 dings.
29. **Hurt limit**: two players are shot in the same server frame → only
    the first plays `Hurtpain_A_*`; a head hit also plays `HEADSHOT.WAV`
    (separate kind).
30. **Others' footsteps by stance**: an enemy running forward at animation
    rate 1 → steps at 50 and 350 ms of each 600 ms loop (3.33 steps/s);
    the same enemy walking with the walk key or crouching → no step sound.
31. **Own footsteps**: running in first person from rest → one step at
    once, then steps every 0.357 s; holding the walk key → none.
32. **Frag on stone, 500 u away, seen**: `Grenade_Stone`; camera jitter
    amplitude 60 × 0.65 = 39 u at its start, 0 at 800 u; light radius
    peaks at 256 u 0.199 s after detonation. The same frag in mid-air,
    never having bounced → `gren.wav` and `Grenade_Stone` (its own metal
    surface; Corrections 2026-10-22, 1).
33. **Frag behind you, 500 u**: `gren.wav` at gain 0.8772, no explosion
    look.
34. **Smoke cloud** on a floor: 10 particles at the first frame; each
    starts within 10 u; horizontal travel ≤ 256 u per axis; size 200 → 300
    over 8–10 s; opaque from 5.1 % of its life; all gone 10 s after
    detonation. No second emission at any time.
35. **Flash look**: `Flashbang_Ini` light radius 512 u at 0.127 s, 0 at
    0.28 s; sprites hidden when a wall is between them and you.
36. **Dry fire**: an enemy pulls the trigger of an empty M4A1 1500 u away
    → `dryfire.wav` at gain 0.3553. (Superseded by case 48: no dry-fire
    sound plays in this build; the gain holds only if one ever did.)
37. **Reload keys**: M4A1 reload → `reload1.wav` at 403 ms, `reload2.wav`
    at 834 ms, `reload3.wav` at 1725 ms; others hear them with R 300 (to
    ≈ 280 u). Drawing the M4A1 plays `reload3.wav` at 666 ms (its select
    animation's key 3).
38. **Grenade bounce on unknown surface**: silent; on Cement →
    `stone\impactgrenade.wav`, R 1000.
39. **Melee impact**: knife on Wood → one of `melee\wood\impact1..2.wav`,
    R 153, silent in half of the hits.

**Follow-up cases** (Open questions 1, 4, 6, 8, 17–25). "Old release" =
map objects and model files of an older extracted client release;
"install" = re-measure on the Steam install (build 25595979).

40. **Placement mode, socket attach (logic).** An effect key in mode 4 on a
    weapon's `Flash` socket, offset (0, 0, 2): when the parent turns 90°
    the key moves with the socket and is 2 u along the socket's forward
    axis; in mode 0 the same key stays at the creation point, only its
    offset turning.
41. **Key curve (logic).** Colour keys at u = 0 (alpha 1) and u = 0.5
    (alpha 0) on a 2 s looping key: at 0.5 s alpha 0.5, at 1.0 s 0, at
    1.5 s 0 (held), at 2.5 s 0.5 again.
42. **Cone emitter (logic).** `Type` 3, rmin 10, rmax 40: every start point
    lies within the cone of height 40 along n whose radius grows from 0 at
    the apex to 10 at h = 40; more than half the points lie at h > 20.
43. **Bounce (logic).** A marked particle moving down at 100 u/s toward a
    floor: after the step that would cross it, v = (0, +75, 0) and its
    position is unchanged; an unmarked one passes through.
44. **Third-person muzzle point (old release; install).** Another player's
    M4A1 shot: the muzzle group and the shot sound start at the attached
    `HH_AR_M4A1_SH` `Flash` socket, model-space (0, 2.685, 24.440) at
    bind, moved by the character's current pose.
45. **Head vs limb impact (logic).** Another player hits a victim's head:
    every client shows `Impact_Head`; on an arm or a leg: `Impact_Body`,
    the same as on the chest.
46. **Decal offset (logic).** A bullet hitting a wall at 60° from the
    normal: the decal centre is 0.5 u off the wall; with `MarksClip` 0 a
    decal over the wall's edge hangs past it.
47. **Under-water shot (data; install).** Standing in water, firing an M4A1
    at a wall 800 u away inside the water: a bubble trail reaching the wall
    in 0.3 s, 30 bubbles (15 at medium detail) rising at 5–6 u/s, all gone
    0.5 s after the last; no tracer, no decal, no burst; the impact sound at
    volume 50. A shot from dry ground into a pool: the ray ignores the
    water surface and hits the pool floor with no splash.
48. **Dry fire silent (logic; install).** An enemy pulls the trigger of an
    empty gun next to you → no sound for you or for them.
49. **Others' footsteps cadence (old release).** An enemy running forward
    slowly (half speed, run flag set): footsteps every 300 ms, the same as
    at full speed; walking: silent.
50. **Kill call-outs (logic).** Alpha kills Bravo; Bravo's teammate C is
    800 u from Bravo and teammate D 1200 u: Alpha's team hears `EnemyDown`
    from Alpha (radius 1000), Bravo's team hears C's `ManDown` (if the set
    has a file); with C dead, nobody speaks `ManDown`. Alpha kills a
    teammate → no `EnemyDown`.
51. **Map sound messages (logic).** A looping `SoundFX` with `StartOn` off:
    TOGGLE → plays; TOGGLE → stops; TRIGGER twice → one copy playing,
    restarted from the beginning.
52. **No special impacts (data).** A rocket hitting a wall 50 u from you:
    no push on your movement (pusher radius 0), no screen tint, no
    ricochet, no blast light at the release detail.
53. **Gain follows motion (logic).** A looping map sound, r 100, R 1000;
    you stand 500 u away and walk to 900 u: its gain goes down smoothly
    with the formula's values at each frame; walk out to 1100 u and back:
    from 1000 u inward it follows the formula again.
54. **Instance cap (logic).** With 255 sounds playing a 256th fails to
    start; no playing sound is stopped.
55. **Jitters add (logic).** Two frag detonations 300 u away in the same
    frame, each giving amplitude a: the camera offset per axis ranges over
    [−2a, 2a].
56. **Camera rise (logic).** Stepping onto an 18 u ledge: the camera rises
    at 180 u/s (18 / 0.1) and reaches the new height in 0.1 s; a 4 u step:
    140 u/s, about 29 ms; a 1 u step: no smoothing.
57. **Bomb detonation look (old release).** Ghost Town site A after the
    bomb goes off: `Explode_S` / `Explode_L` effects appear at the site in
    turn between about 0.4 and 5.3 s while the map camera shows them; no
    camera jitter and no damage to players at the site.

**Corrections (2026-10-22) cases.**

58. **Mid-air frag (logic).** An M67 thrown up in the open, never touching
    anything, explodes 500 u in front of you → `gren.wav` at gain 0.8772 and
    `Grenade_Stone` with its frame's +Z world up; one that bounced off a
    Wood wall last → `Grenade_Dirt`.
59. **Fly-by of a miss (logic).** Another player's rifle shot that hits
    nothing, its line 200 u from your camera and its ray end 3000 u away →
    one fly-by.
60. **Own tracer cadence (logic).** M4A1, A silencer + a magazine part
    without a tracer, 8 shots → own tracers on shots 4 and 8 with the AR01
    look; A silencer alone → shots 3, 6 (as case 13).
61. **Misc key radius (logic).** Another player's misc key sound (key 10) at
    1000 u → gain 0.7166 (R 2000); dead players send none.
62. **Own step restart (logic).** Run, stop 0.5 s, run at 60 fps → steps at
    0 s, ≈ 0.212 s, ≈ 0.569 s; stop 0.05 s → no restart.
63. **Ghillie step (data; install).** As case 4 → gain 0.4025, heard from the
    walker's centre.
64. **Awareness (logic).** Specter enemy, listener's Awareness on → step at
    volume 100 (Specter ignored), radius 1080, plus one of the perk's four
    sounds at volume 100.
65. **Local voices (logic; install for files).** A body-shot kill → one kill
    voice; a groin kill → none; 5 s idle with no keys → idle sound starts,
    stops on the next key; sprinting → dash sound back to back; low health
    → heartbeat entry files about 1 s apart.
66. **Hurt sound place (logic).** Head hit, victim centre 400 u away, head
    370 u → both hurt sounds at d = 400; burn damage to the head → only the
    burn kind.

**Corrections (2026-10-30) cases.**

67. **Key-end cut (logic).** Non-looping group of 1 s with one
    `PlaySound` key 0–0.5 s, `Loop` No, a 2 s file → the file is heard
    0–0.5 s and stopped there; the same with `Loop` Yes → also 0–0.5 s.
    The group created looping → the file starts again at 1 s, 2 s, …, each
    copy heard 0.5 s.
68. **Whole-group loop key (logic).** Looping group, `Phase` 3000, one
    local `PlaySound` key 0–3 s with `Loop` Yes and a 1.3 s file → one
    start at creation; the file loops by itself; no new start at 3 s, 6 s,
    …. With `Loop` No → heard 0–1.3 s, then silent while the group runs.
69. **Hidden group (logic).** An attached group built like
    `Chainsaw_PV_idle` (looping, `Phase` 3000, one local looping
    `PlaySound` key over the whole group) on a weapon with
    `PVAttachFXAlwaysShow` 0: shown at t = 0, `HIDE_PVATTACHFX 0` at
    1.0 s, `SHOW_PVATTACHFX 0` at 4.0 s → the file plays 0–1.0 s, nothing
    1.0–4.0 s, and starts again from its first sample at 4.0 s; the
    group's clock reads 1.0 s at 4.0 s (frozen while hidden). The stock
    chainsaws (`PVAttachFXAlwaysShow` 1) ignore the hide key; hiding the
    whole weapon does the same as the key here.
70. **Not-yet-started key while hidden (logic).** Looping 2 s group with a
    sound key 1.5–1.8 s, hidden at 1.0 s and shown at 3.0 s → the key
    starts at 3.5 s (group time 1.5 s), not at 1.5 s or 3.0 s.
71. **Own alt-fire sound (data).** A weapon with `FireSnd` a and
    `Alt_FireSnd` b → primary hits play a, secondary hits b, on the
    attacker's client; others hear a for both.
72. **Shared fire voice (logic).** One shooter, same weapon: a fire sound
    started at 0, an alt-fire sound at 0.2 s → the first stops at 0.2 s.
    Silenced shots 0.1 s apart → all copies overlap; an unsilenced shot
    after a silenced one → the silenced copy keeps playing.
73. **Tracer width (logic).** M4A1 (AR02, `Width` 15) tracer seen from the
    side, its middle 1000 u ahead → 7.5 u across in the world; at a
    vertical FOV of 1/4 the unzoomed one (zoomed) it covers 4 times as
    many pixels; no minimum.
74. **Tracer facing (logic).** Camera at the origin looking along +Z;
    tracer ends at camera-space (10, −10, 50) and (0, 0, 1050) → Δ =
    (−10, 10), n = (0.7071, 0.7071, 0); corners at each end ± 3.75·n
    (AR02), both ends with the same n.
75. **Tracer behind a wall (logic).** Another player's tracer passing
    behind a wall between you and it → hidden where the wall covers it;
    two tracers crossing → both visible (no depth write).
76. **Scoped own tracer (logic).** M24 (SR01, every shot) zoomed: the
    tracer is drawn, its tail starting at F (camera + `Pos` + `MuzzlePos`
    in camera axes, as unzoomed); `Color` and alpha changes in the record
    → no visible change.

Trail keys and smoke-trail puffs (Follow-up (2026-10-31)).

77. **Trail width (logic).** W = 2, N = 50: n = 2 → Constant 2,
    SmallToBig 0, SmallToSmall 0, BigToSmall 2; n = 8 → 2, 0.24, 0.24,
    1.76; n = 30 → 2, 1.12, 0.88 (30 ≥ 25.5), 0.88. The strip's full width
    is twice these, the same at every point of the trail.
78. **Trail cadence (logic).** I = 0.03 s, L = 0.25 s, N = 50, frames of
    exactly 1/64 s from activation: points are made at updates 2, 3, 5, 7,
    9, … (every second update after the third); before removal n = 1, 2,
    2, 3, 3, …, 8, 8, 9 at update 17, where the point made at update 2
    reaches age 0.25 and is removed; from then on n alternates 8 and 7
    after removal. BigToSmall with W = 2 at n = 8 → w = 1.76.
79. **Trail texture (logic).** W = 20; points at (0, 0, 0), (10, 0, 0),
    (30, 0, 0) in that order → u = 0, 0.5, 1.5 (the texture repeats every
    20 u of path); with `UAdd` 1 after 0.25 s every u is 0.25 larger and a
    texture feature has moved 5 u toward the oldest point.
80. **Trail colour (logic).** `Eye_Red_Tail_L`'s curve with L = 0.2 s: a
    section of age 0.1 s → (1, 0, 0), opaque; age 0.17 s (curve time 0.85)
    → red 0.449, green 0, blue 0; age 0.00347 s (curve time 0.01735) →
    (1, 0.251, 0). The newest section is always near the curve start.
81. **Trail ending (logic).** A trail of 8 points whose key shuts down with
    smooth shut-down: no point is added, the newest stays where it was,
    the others vanish as they reach L; when one point is left nothing is
    drawn. Hiding the group clears all points; shown again, the trail
    starts empty (first point after I).
82. **Puff size (logic).** Screen 1024 pixels wide: a new puff's square is
    7.8125 u; with total life 10 s, after 1.0 s k = 2.4 → 18.75 u, its own
    alpha 0.81; at 1920 pixels the same puff is 4.17 u, then 10.0 u.
83. **Puff fade and look (logic).** Long white segment (life 1.5 s,
    emitting 0.2 s): 0.75 s after its last emission its alpha is 0.5, so a
    puff emitted then with f = 0.075 shows alpha 0.9·0.925·0.5 = 0.416; at
    1.5 s the segment and all its puffs are gone. Every puff is an
    untextured grey square (channels equal, 150–230 of 255).

Prop debris, empty texture slots and surface 0 (Follow-up (2026-10-31b)).

84. **Debris table.** Parsing the install's `ATTRIBUTES\DEBRIS.TXT` gives six records, ids 0–5, values as in the table; `Rotate` of `56x56WoodBox` reads as on; `Window` has three models and two bounce and two explode sounds; `Stone` has `MinWorldVel` (−50, 100, −50).
85. **Debris count.** `Window` (`Number` 30) at high detail → 20 pieces; medium → 13; low → 6; `56x56WoodBox` low → 1 (6 × 0.333 = 1.998 → 1); `None` → 1 piece, nothing drawn, no sound.
86. **Debris velocity bounds.** `56x56WoodBox`, death direction (0, 0, 1) (right (1, 0, 0), up (0, 1, 0)): lo = (−150, 300, −150), hi = (150, 500, 150); every piece's velocity lies in that box. Death direction (1, 0, 0) (right (0, 0, −1)): lo = (−150, 300, 150), hi = (150, 500, −150); z is drawn between 150 and −150, so the pieces fill the same box.
87. **Debris start offsets.** `Window`: every piece starts at the death position + (10, 10, 10). `Window_Large`: within ±30 per axis.
88. **Debris models and scale.** `Window`, 20 pieces: pieces 1, 4, 7 … use `glass_01.ltb`, 2, 5 … `glass_02.ltb`, 3, 6 … `glass_03.ltb`; every scale lies in [0.4, 1.44] (0.8 × 0.5 to 1.2 × 1.2); alpha 0.7.
89. **Debris fall.** A `56x56WoodBox` piece launched at (0, 400, 0) in open air at 60 fps: after 12 frames (0.2 s) its height gain is 400·0.2 − 1500·(1/60)²·66 = 52.5 u (the position steps with the speed before each frame's gravity) and its vertical speed 100 u/s.
90. **Debris bounce.** A `Window` piece hitting a flat floor (n = (0, 1, 0)) at v = (100, −300, 0): it continues at (50, 150, 0), 0.3 u above the hit point, b down by one; at 60 fps a floor hit at v = (0, −200, 0) leaves |v| = 100 < √(225·60) ≈ 116 → at rest, rotated to its yaw only. A wall hit (n = (1, 0, 0)) does not count against b.
91. **Debris sky.** A piece whose segment hits a sky polygon disappears that frame; the effect's other pieces do not move in that frame.
92. **Debris end.** `Window` piece with L = 4.5 s, fade 3 s: alpha 0.7 until 4.5 s, 0.5 at 6.0 s (min(0.7, 1.5 / 3)), 0.2 at 6.9 s, removed after 7.5 s.
93. **Empty slot (logic).** A model piece whose render style uses texture slot 2 on stage 1 with the slot empty: stage 1 has no texture bound and keeps the style's colour operation; no default or white image is created by the game.
94. **Unknown surface table.** The install's surface table: surface 0 has shoot-through 1, thickness 10, perturb 10, range × 1.0, damage × 0.6; the shoot-through list is the 19 ids of Follow-up (2026-10-31b) 3.
95. **Shot through an Unknown wall.** A server ray hits a level wall of surface type 0, 8 u thick, then a torso 500 u away: the torso takes the shot's damage × 0.6 (no other dampening); a 12 u wall of surface 0 stops the ray (limit 10 + 1 u, `weapon_firing.md` "Penetration") and nothing behind it is hit.
96. **Unknown census.** WAREHOUSE's physics BSP has 312 solid polygons of surface type 0 out of 32,688; across the install 100,922 of 4,057,293 (98 maps) and 72,275 of 594,095 world-model polygons.

## Open questions

1. **Beyond the outer radius**: the game's gain is not recomputed beyond R,
   and the audio library's own distance roll-off is set to 0. We assume the
   library mutes samples beyond their maximum distance (R + 0.01). If it
   does not, sounds started out of range play at full gain. Test: stand
   2100 u from a firing M4A1 in an open map.
   Still open. Checked again: the sound manager sets gain and low-pass only
   for distances inside R (at start and on each move, Open question 21)
   and sets the library's maximum distance to R + 0.01; nothing else mutes
   the sample, so the answer is the library's own behaviour.
2. **Fly-by position** (Quirk 6): confirm the fly-by is inaudible away from
   the map origin. Test on a map with the origin far from the play area.
3. **Particle and sprite size convention**: we read a particle of size s as
   a square of side 4s, and a sprite at scale k as texture pixels × k
   units wide. Both affect every number given as a size here (a bullet hole
   2.9–3.8 u, a smoke particle 800–1200 u). Check against a screenshot:
   a bullet hole beside a known 64 u crate.
4. **Light through walls**: engine dynamic lights are assumed unshadowed.
   Check a muzzle flash behind a thin wall at medium detail.
   Partly answered: the lights are created world-only and do not light
   back-facing polygons (DynaLight, "The effect file"), so a wall's side facing
   away from the light stays dark; a surface facing the light behind a
   thin wall may still be lit. Still open for that case; measure as
   above.
5. **Frozen off-screen smoke** (Quirk 7): confirm that a smoke cloud lasts
   longer when you look away (look away for 10 s after detonation, turn
   back).
6. **Server-side surface type**: others' impacts use the server's surface
   type and normal; whether the server re-derives the character surface
   (head/body) for client-reported hits or sends what the shooter reported
   was not traced.
   **Answered:** the server derives it from the reported node: head
   surface for head-location nodes, body surface for every other one
   (section 5, What other clients learn); case 45.
7. **Hit confirm on teammates**: whether the server sends the hit event (and
   so a ding) for hits on teammates with friendly fire off.
   **Answered:** yes, any character hit dings (Corrections 2026-10-22, 13).
8. **Others' footstep keys while unseen**: footsteps of characters you
   cannot see depend on their animation being advanced on your client; not
   traced.
   Partly answered: the keys come from the server-chosen leg animation,
   replicated and advanced on each client at rate 1 (section 11); case 49.
   Still open: whether the client advances a character's animation while
   it is outside the view or not drawn. Measure: stand behind a wall while
   an enemy runs on the other side and listen for steps.
9. **Server decal times on movers**: the server's solid-time setter writes
   the fade time instead, so the solid time keeps its initial value (not
   traced). Check how long a decal on a door lasts.
10. **Silenced C/Red flashes**: confirm in the Steam build's effect file
    that `Silenced01_PV_Muzz` and `Silenced01_HH_Muzz` are still missing.
11. **Frame order**: the exact place where incoming events are handled in
    the client frame (before or after the player update) was not traced.
12. **Contradiction with `grenades.md`, section 11 (smoke)**: it says the
    particle system "runs 2.0 s", has "full bounce off the world" and that
    the cloud "lasts at most about 12 s". From the effect code: one burst at
    the first frame, bounce keeps 75 % of speed, and the cloud ends at most
    10 s after detonation (barring Quirk 7). The velocity box is in the
    effect's frame (local +Z along the detonation normal) and the friction
    is per second (×0.02 per s). This answers its open question 10; that
    spec should be updated by its owner.
13. **Contradiction with `attachments.md`, "Muzzle flash"**: it says
    shipped silencers name ordinary effects and still flash in first
    person. True for grades A and B; grade C and all Red silencers name
    effects that do not exist, so they show nothing. Its "Sound" section
    also says the muzzle light of a silenced shot is not shown to others;
    the routine that would hide it is never called, the light is part of the
    flash group, so A/B silencers show their light and C/Red show neither.
14. **Contradiction with `attachments.md`, "Tracer"**: the last fitted
    attachment's tracer decides only the **cadence** on the shooter's own
    screen; the look comes from the muzzle-slot attachment (else the ammo),
    nothing is drawn when the ammo has no tracer, and other players see
    the server's cadence from the ammo alone (section 4).
15. **Contradiction with `weapon_firing.md`, penetration table**: it lists
    the shoot-through surfaces and says all others stop the ray. The
    surface table also marks Unknown (0: limit 10, re-perturb 10, damage
    × 0.6), trussing (23: limit 10, damage × 1.0), Invisible metal (206:
    always, damage × 0.9), Air (1) and Boss3_weak (65) as shoot-through.
    Unknown matters: untyped world polygons would be penetrable up to 10 u.
    Answered (Follow-up (2026-10-31b) 3): the install's own table marks
    Unknown shoot-through; surface-0 walls up to about 11 u are shot through
    at damage × 0.6.
16. **Build differences**: data from the US client; effect files identical
    in the US 1501.04 and BR 1810.04 archives for every group named here
    except the grenade groups' camera jitter (BR `Grenade_Dirt`: 20 u at all
    details instead of 60 u at high detail; small end-time differences).
    Re-check against the Steam build's archives when the readers work.
17. **Effect file details** (placement modes, key curves, emitter shapes,
    motion, bounce, sprites, other key types).
    **Answered:** "The effect file", Placement modes in detail and after; cases
    40–43.
18. **Third-person effect points and others' shot sounds.**
    **Answered:** section 3 (the attached weapon model's `Flash` and
    `Breach` sockets, stock socket points); case 44. Install: re-measure
    the sockets on the Steam models.
19. **Decal clipping and offset.**
    **Answered:** `MarksClip` defaults to 0; offset 1 u back along the ray
    (section 6); case 46.
20. **Liquids** (impacts, bubbles, rays through water).
    **Answered:** section 6a; case 47.
21. **Gain and low-pass while a sound plays; priorities and the instance
    cap.**
    **Answered:** section 10 (recomputed each frame the listener or the
    source moved, inside R only; 255 live sounds, a new one is refused,
    priorities unused); cases 53, 54.
22. **Kill call-outs and map sounds.**
    **Answered:** section 11, Kill call-outs and Map sounds details; cases
    50, 51.
23. **Special impacts** (pusher, blast light, screen tint, ricochet,
    critical hit).
    **Answered:** section 6, step 2; case 52. All are absent or inert in
    the stock data except the critical effect, which depends on the
    attachment (`gear_perks.md`).
24. **Several camera jitters at once; camera rise.**
    **Answered:** "The effect file", CamJitter (they add up) and camera rise; cases
    55, 56.
25. **Bomb detonation look and dry fire.**
    **Answered:** section 9 (map effects and camera, no weapon effect);
    dry fire never plays (section 11, Quirk 13 withdrawn); cases 48, 57.
26. **Damage screen effects by damage type** (from Corrections 2026-10-22).
    The client has a data-driven table of damage effects keyed by damage
    type, started by progressive damage (burn, poison, bleed and the like).
    Each can tint the screen (several active tints combine by the per-channel
    maximum), scale the scene's light (per-channel minimum), lock movement or
    input, spawn particles on the player and play sounds. The stock records
    were not readable on this box, so which damage types have a visible
    effect, and how strong, is open. Read the damage-effect attribute file
    from the install.
27. **Local auto-cast files** (Corrections 2026-10-22, 8): the files of the
    kill, dash, idle and heartbeat entries per sound template, and each 3D
    hurt kind's own radius, are to be read from the install's auto-cast
    attribute data.
28. **Mechanical-sight slot** (Corrections 2026-10-22, 3): whether every
    weapon always has a part in the last slot; if so, attachment tracers
    never set the shooter's own cadence.
29. **Idle-sound flags** (Corrections 2026-10-22, 8): whether holding fire
    sets a movement control flag (which would also stop the idle sound).
30. **Tracer blending** (Corrections (2026-10-30) 3): the tracer sets no
    blend mode and inherits the last one the client's primitive drawing
    set. Ordinary alpha blending with the texture's alpha is assumed;
    check a tracer's edges in game (soft edges: alpha blend; a hard
    rectangle: no blend).
31. **Untextured drawing** (Follow-up (2026-10-31) 2): with no texture
    bound, the fixed-function stages read the texture as opaque white on
    Direct3D 9 hardware, so a smoke puff shows its plain vertex colour.
    That is the graphics API's behaviour, not traced in the game's code;
    check a rocket's smoke trail in game (grey squares, no soft smoke).
    Traced (Follow-up (2026-10-31b) 2): the game binds nothing to such a
    stage and keeps its operations; the white is the API's.
32. **Debris offset mode** (Follow-up (2026-10-31b) 1): the debris effect
    has a second offset mode (start points along each piece's velocity
    direction) whose switch is never set for prop debris; the per-axis
    offsets are assumed. Check: `Window` glass shards all start from one
    point 10 u off the pane's centre.
