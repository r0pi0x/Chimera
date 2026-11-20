# Team Fortress 2: particle rendering (SpriteCard, sheets, sizes, view-model effects)

Source basis: Valve's Source SDK 2013 (2025 release) read in place in a
spec session: the SpriteCard shader (its parameter list and defaults, its
blend-state choice, vertex and pixel programs), the shared pixel-shader
helpers (depth feathering, gamma conversion), the math library's bias
curve, the public particle header (the particle attribute numbering), the
client's particle-effect object and particle property (attachment
following, the view-model-effect flag), the view-model attachment
re-projection and the view renderer's view-model pass. The particle
operator library itself is **not** in the SDK (shipped as a binary), so
operator meanings below are read from their names and parameters and
marked *inferred* where the SDK does not show them. Data: TF2's particle
files and effect materials read in place with our dumper (values only;
nothing committed), and texture sheet resources decoded from the `.vtf`
headers. Nothing copied; restated in our words. Section 9 (2026-11-20,
tf2-spec-psys) also read the particle library the SDK ships only as a
prebuilt static library: its exported symbols and the machine code of the
system step, the first-frame setup, the continuous and instantaneous
emitters and the control point's position at a given time, plus the
client's particle manager, its per-frame order and the entity particle
property (all in the SDK's source). Section 10 (2026-11-20,
tf2-spec-flashup) read the same library's machine code for the alpha
fade, radius scale, colour fade, basic movement, oscillators, the sphere
initializer's velocity, the sprite-trail renderer, the render list and
the skip-ahead, the operators' parameter tables (names, defaults), and
the SDK client's `info_particle_system` and particle manager.
Status: draft

## Summary

A TF2 particle is a camera-facing quad (or a velocity-stretched trail)
whose half-size is its **radius in world units**. Its brightness and
blending come from its material: TF2 effect materials are either
SpriteCard (with sheet animation, size clamps, soft depth edges and an
overbright factor) or plain UnlitGeneric sprites (additive or
alpha-blended, no clamps). Particle systems attached to the first-person
weapon are **simulated and drawn in the world view with the world FOV**,
at the muzzle position re-projected from the view model; they are never
drawn with the view-model FOV. Drawing a muzzle flash with the view-model
camera makes it tan(world/2)/tan(54°/2) too big on screen: 1.96× at the
default 90, 1.51× at 75.

## Units and conventions

Hammer units, z up, seconds. Colours in files are 0–255 gamma values;
particle alpha 0–255 in files is 0–1 at run time. "Life fraction" t is
age / lifetime in [0, 1]. Eye space is (forward, right, up). l is the
distance from the eye to the particle's centre in world units. NDC
half-size is the fraction of half the screen height a sprite covers.

## Constants

| Name (ours) | Value | Unit | Meaning |
|---|---|---|---|
| sc_minsize | 0 | × l | SpriteCard `$minsize` default |
| sc_maxsize | 20 | × l | `$maxsize` default (no practical clamp) |
| sc_startfadesize / sc_endfadesize | 10 / 20 | × l | `$startfadesize` / `$endfadesize` defaults |
| sc_depthblendscale | 50 | – | `$depthblendscale` default |
| sc_overbright | 1 | – | `$overbrightfactor` default |
| sc_addself | 0 | – | `$addself` default |
| sc_maxdistance / sc_farfade | 100000 / 400 | units | `$maxdistance`, `$farfadeinterval` defaults |
| sc_alphatest | 0.01 | – | SpriteCard discards pixels with alpha ≤ 0.01 unless `$addself` or a second texture is used |
| sc_depthblend_default | on | – | soft particles on by default (off with the client's "reduce particles" setting or hardware without pixel shader 2.0b) |
| vm_fov | 54 | deg | view-model FOV (viewmodel.md §5); not used for particles |

## Behavior

### 1. Blend modes (the material decides)

For a **SpriteCard** material, in this priority:

1. `$addself` ≠ 0 (or `$addbasetexture2` ≠ 0, or `$addoverblend` 1):
   **premultiplied** blending, dst = src + dst × (1 − src_alpha); no alpha
   test.
2. else `$additive` 1: **additive**, dst = src × src_alpha + dst.
3. else: **alpha blend**, dst = src × src_alpha + dst × (1 − src_alpha).

(`$translucent` alone selects 3.) Depth is tested, never written; back
faces drawn; destination alpha not written. Base texture read as sRGB,
output written as sRGB (DX9 path).

For an **UnlitGeneric** material on a particle renderer (most TF2
sparks, glows and smoke: `effects/brightglow_y_nomodel`,
`effects/softglow`, `effects/softglow_translucent`, `effects/wispy_smoke`,
`effects/singleflame`, `effects/tracer1`): `$additive` 1 → additive as 2;
`$translucent` 1 → alpha blend as 3; colour = texture × vertex colour,
alpha = texture alpha × vertex alpha (`$vertexcolor`, `$vertexalpha`).
No size clamps, sheets or depth feathering (inferred: those are SpriteCard
features).

### 2. SpriteCard pixel colour

Per pixel, with T = the sheet-frame-blended texel (§3), v = the particle's
vertex colour (rgb converted gamma → linear with exponent 2.2; alpha
unchanged), O = `$overbrightfactor`, S = `$addself`, f = the depth
feather (§4, 1 when off):

- T.rgb ← T.rgb × O.
- a_v = v.a × f.
- Without `$addself`: out = (T.rgb × v.rgb, T.a × a_v).
- With `$addself` S:

```math
\text{out.rgb} = v.\mathrm{rgb} \cdot P \cdot \left(1 + O\,S\,a_v\right),\quad P = T.\mathrm{rgb}\,T.a\,a_v,\qquad \text{out.a} = T.a\,a_v
```

  then blended premultiplied (§1.1). So a fully transparent particle
  (a_v → 0) adds nothing, and a bright one is added on top of its own
  alpha-covered colour.
- The `srgb?$overbrightfactor` spelling in a material applies the factor
  only on the sRGB (DX9 PC) path (Open questions 3).

### 3. Sprite sheets and frame choice

- A sheet is a resource inside the `.vtf` listing **sequences**; each has
  a number, a clamp/loop flag, a frame count, a total time and per frame a
  duration and a rectangle (u0, v0, u1, v1) in the texture (one rectangle
  per frame; version-1 sheets store four identical samples per frame, the
  first is used).
- A particle's sequence is its sequence attribute (field 9), set by
  "Sequence Random" (uniform integer in [min, max]) or the system's
  default `sequence_number` (0). A material without a sheet is one frame,
  the whole texture.
- **Frame position** p (in frames, fractional):
  - "animation_fit_lifetime" on: p = t × frame count (the whole sequence
    plays once over the particle's life);
  - otherwise p advances with age at `animation rate` (frames per second
    when "use animation rate as FPS" is set; otherwise the rate scales the
    sequence's own timing: Open questions 1).
  - A clamped sequence stops at its last frame; a looping one wraps.
- **Blending**: frames ⌊p⌋ and ⌊p⌋+1 are both sampled and mixed by the
  fraction p − ⌊p⌋ (`$blendframes` 1, the default).
- Example, `particle/flamethrowerfire/flamethrowerfire102` (1024², 10×10
  cells of ≈ 0.0986 each): sequences 0–4, all clamped, 19, 21, 18, 19 and
  19 frames, each frame 1 time unit; sequence 2 starts at cell row 4
  (v 0.3989), sequence 3 at (0.7974, 0.4985), sequence 4 at (0.6978,
  0.6978). The flame core uses sequences 2–4.

### 4. Size, clamps and fades (vertex stage)

For each SpriteCard particle with radius r (world units, after all
operators) at distance l from the eye:

1. r ← max(r, `$minsize` × l).
2. If r > `$startfadesize` × l: if r > `$endfadesize` × l the particle is
   not drawn; else its colour and alpha are multiplied by
   1 − (r − start·l) / ((end − start)·l).
3. Far fade: with s = max(1, `$maxdistance` − `$farfadeinterval`),
   colour × (1 − clamp((l − s) / (`$maxdistance` − s), 0, 1)); not drawn
   at 0.
4. r ← min(r, `$maxsize` × l).
5. Screen-facing quad of half-size r, rotated by the particle's roll
   (field 4) in the view plane (and its yaw, field 12, for flipped ones).

The clamps are **fractions of the distance**, so on screen they are
fractions of tan(FOV/2): a sprite clamped at `$maxsize` m covers an NDC
half-height of m / tan(vertical FOV / 2) (0.533 for m = 0.4 at world FOV
90 on 16:9, vertical tan = 0.75).

**Depth feathering** (`$depthblend` 1, pixel stage): with the scene's
depth and the sprite's depth at the pixel (both linearised view depths
from the depth stored in the frame's destination alpha), f =
saturate(|scene − sprite| × k), where k grows as `$depthblendscale`
shrinks (a smaller scale gives a harder edge; the exact normaliser is in
the closed shader API: Open questions 2). Near the far end of the stored
depth range f is pushed to 1. f multiplies the vertex alpha.

### 5. Renderers

- **render_animated_sprites**: one camera-facing quad per particle (§4),
  sheet-animated (§3). `orientation_type` 0 = facing the camera (all TF2
  weapon effects read here use 0).
- **render_sprite_trail**: a strip from the particle back along its
  motion (§5.1). Used for sparks, tracers, embers, the Minigun core and
  the explosions' streaks.
- A system may list **two renderers** (`Explosion_FlyingEmbers`: a trail
  and an animated sprite): each particle is drawn by both, the sprite
  sitting on the trail's head as a round dot of the same radius.

#### 5.1 Sprite trails

Settled by capture (2026-11-20, below) together with the tracer capture
of Open questions 4. Per particle, each frame, after the step's
operators:

1. **Head and direction.** The head H is the particle's position; the
   step's motion is m = H − (position before this step) (field 2). The
   speed is |m| / Δt of that step, so it is the speed the particle
   actually moved with after drag, gravity and collisions, not its
   initial velocity. A lock to a control point carries the particle's
   place before the step along with it, so the point's own motion is not
   part of m (measured 2026-11-20: the rocket's `rockettrail_burst`,
   locked to the rocket flying at 1100 units/s with τ 0.1, draws no
   110-unit streaks, only its short sparks; Open questions 7). A particle that did not move in its last step has no
   direction and draws nothing (measured: `new_flame_pilot`, locked to a
   still muzzle, shows no streak).
2. **Length.** With τ = the particle's trail length (field 10, seconds;
   set by "Trail Length Random", uniform between its two values in either
   order) and a = the particle's age in seconds:

```math
L = \mathrm{clamp}\Big(\,|v|\,\tau\cdot f,\ L_{min},\ L_{max}\Big),\qquad f = \begin{cases}\min\!\left(1, \dfrac{a}{t_{fade}}\right) & t_{fade} > 0\\ 1 & t_{fade} = 0\end{cases}
```

   with `min length`, `max length` and `length fade in time` from the
   renderer. The fade-in is by **age in seconds**, not by life fraction;
   the clamp comes after it (inferred order). Consequence: while
   a < t_fade a particle moving at a constant speed draws a streak of
   v·τ·a / t_fade, i.e. τ / t_fade of the distance it has travelled
   since birth, so the streaks of a burst reach back toward its centre
   and detach from it once a passes t_fade.
3. **Width.** The strip's **full width is the particle's radius** (field
   3) as the radius operators leave it this frame. "Radius Scale" and
   the other radius operators act on the radius exactly as for sprites;
   the trail renderer applies no scale of its own, but the width is
   capped at the length: **w = min(radius, L)** (read from the library,
   §10.7, which also gives the exact length rule and the clamp order).
4. **Geometry.** The strip runs from the tail H − L·m/|m| to H, turned
   about its own axis to face the eye; the texture's v goes from 0 at the
   tail to 1 at the head, u across the width. Colour and alpha are the
   particle's; blending by its material (§1). UnlitGeneric trail
   materials have no size clamps.

**Worked examples** (stock `particles/explosion.pcf`, all children of
`ExplosionCore_Wall`, delay 0):

| System | Material (blend) | Count, life | Radius r₀, scale | Trail | Speeds (local, along the surface normal = x) | Look |
|---|---|---|---|---|---|---|
| `Explosion_FlyingEmbers` | `effects/circle2` UnlitGeneric, additive; trail **and** sprite | 16 at once; life U(0, 0.35) | U(0.5, 1); × 3 → 0 with ease (bias 0.5) | τ U(0.05, 0.1); t_fade 0.1; L in [0, 2000] | x U(20, 2200), y, z U(−1500, 1500); gravity −500; drag 0.2; collide with brushes, bounce 1 | **the thin yellow rays** (≈ 1–3 units wide), each with a dot at its head |
| `Explosion_FloatieEmbers` | `effects/brightglow_y_nomodel` UnlitGeneric, additive; trail | 24 at once; life U(1.5, 2.5) | U(0.1, 0.7); × 3 → 0 eased | τ U(0.01, 0.05); t_fade 0; L in [0, 2000] | x U(0, 2330), y, z U(−1200, 1200); gravity +128; drag 0.4; random force ±1800, ±1800, ±800; bounce 2 | short sparks that drift up |
| `Explosion_CoreFlash` | `effects/softglow` UnlitGeneric, additive; trail | 12 at once; life U(0.05, 0.17) | U(1, 12); 3 → 11 with bias 0.9 | τ U(0.33, 0.4); t_fade 0.2; L in [2, 122] | x U(410, 1120), y, z U(−840, 840); no drag | **broad soft orange streaks** (width 3–132): the orange star round the blast for ~7 frames |
| `Explosion_Flashup` | `effects/softglow`, additive; trail | 3000/s for 0.01 s (max 45); life U(0, 0.2) | U(1, 5); 3 → 11, bias 0.9 | τ U(0.3, 0.4); t_fade 0.2; L in [32, 512] | x U(110, 920), y, z U(−840, 840); drag 0.1 | nothing when started alone by an `info_particle_system` (its skip ahead, §10.1); in a blast about a dozen soft orange strips 32+ long in the frame after it |
| `Explosion_Dustup` | `effects/softglow_translucent`, alpha; trail | 250/s for 0.12 s; life U(0, 0.3) | U(1, 2); 3 → 11, bias 0.9 | τ U(0.1, 0.4); t_fade 0.2; L in [32, 512] | x U(110, 1220), y, z U(−840, 840); drag 0.1 | brown streaks (alpha 128–212) |

Per frame, `Explosion_FlyingEmbers` with the formula: a particle leaving
at 1500 units/s with τ = 0.08 draws L = 1500 × 0.08 × (a / 0.1): 20 units
at 1 frame (17 ms), 40 at 33 ms, 60 at 50 ms (less the speed drag has
taken), then at most 120; its width at life fraction x is
r₀ × 3 × (1 − (3x² − 2x³)), e.g. 2.15 for r₀ 0.8 at x 0.2. A fast one
(|v| ≈ 2900) reaches 145 units at 50 ms. `Explosion_CoreFlash`'s widths
at x = 0.1 are r₀ × 8.6: these are the wide glows, correctly wide.

**Capture (2026-11-20, tools/tf2ref, real TF2, 1024x768, fov 90,
`host_framerate` 1/60):** each child above started **alone** with an
`info_particle_system` on 2fort's bridge floor about 160 units ahead,
zero angles (local x = world +x), 20 frames each:

- `Explosion_FlyingEmbers`: from the first frame thin orange-yellow rays
  about 3 pixels wide (about 1 unit) fan out from the point, each ending
  in a round dot about 3 times as wide (the sprite renderer); the rays
  reach back to about the burst centre for the first ~6 frames, then
  detach and fly off; most are out of view or gone by frame 10. These are
  the rays of the full rocket blast (Open questions 7).
- `Explosion_CoreFlash`: broad, soft orange streaks tens of units wide
  forming a star round the point, growing for ~4 frames, gone after ~7.
- `Explosion_Flashup`: no pixel changed by more than 3 % in any frame
  (at most 2 pixels). Not its look in a blast: the entity's client skipped
  the late effect ahead past its emitter's 0.01 s (§10.1, §10.10).

### 6. Effects on the first-person weapon (view-model particles)

- Systems attached to the first-person weapon (the view model, or the
  `c_` weapon attached to the arms) read the attachment's **re-projected**
  position: in eye space its right and up components are multiplied by
  tan(world FOV/2) / tan(view-model FOV/2), forward unchanged, so it
  projects to the same screen point under the world camera
  (specs/cs_source/view_models.md §5). The attachment's orientation is
  **not** changed.
- These systems are **world systems**: they simulate in world units,
  depth-test against the world and draw in the world's translucent pass
  with the world FOV.
- A system whose definition sets "view model effect" would instead
  un-project its control point back into view-model space and be drawn
  with the view model (inferred from the flag's handling). **No TF2
  weapon system read here sets it** (all `muzzle_*`, tracers, flames).
- Consequence for screen size: a particle of radius r at eye depth d
  covers NDC half-height r / (d × tan(world vertical FOV/2)). The
  view-model FOV plays no part. Drawn with the view-model camera instead,
  it would be (tan world/2)/(tan 27°) larger: 1.963 at world 90, 1.506 at
  75 (horizontal 4:3 values; the ratio is the same vertically).
- The screen size of a stock muzzle flash is therefore set by: the
  systems' radii and radius scales (§8), the muzzle's eye depth (≈ 37–70
  units for the stock guns, viewmodel.md test table), the world FOV, and
  for SpriteCard materials `$maxsize` (not set for the muzzle materials,
  so the default 20 × l: no clamp).

### 7. Operators and initializers (beyond specs/cs_source/impact_effects.md §14)

The basic meanings (emitters, position within sphere, lifetime, radius,
alpha, colour and rotation random, movement basic, alpha fade and decay,
lifespan decay, radius scale with the bias curve) are as
specs/cs_source/impact_effects.md §14, with the exact formulas of §10
(alpha fade and decay, radius scale, colour fade, movement basic). The
math library's **bias curve** is bias(x, b) = x^(−log₂ b) (b = 0.5
linear; b > 0.5 rises early), but the **Radius Scale operator uses the
rational form** s / ((1/b − 2)(1 − s) + 1) (§10.3). The
**ease in and out** flag passes the life fraction through 3x² − 2x³.
Additional ones in TF2's weapon effects (inferred unless noted):

- **Position Modify Offset Random**: adds a uniform random offset between
  `offset min` and `offset max`, in the control point's axes when "offset
  in local space" is 1.
- **remap initial scalar** / **remap scalar to vector** (at creation):
  reads an attribute (input field) and writes another: output = linear
  map of [input min, input max] onto [output min, output max], clamped;
  for the vector form, the output vector is in the control point's axes
  when "use local system" is set (field 0, position: an offset from the
  control point). Each runs only for particles created while the system's
  age is inside [emitter lifetime start, end] (−1 = no bound). Field 8
  (creation time) is the system age at creation.
- **Remap Scalar** (each step): same mapping applied every step (the
  Minigun core maps radius → trail length).
- **Remap Distance to Control Point to Scalar / to Vector**: maps the
  particle's distance to a control point onto a field; "output is scalar
  of initial random range" multiplies the particle's initial value
  instead of replacing it.
- **Movement Lock to Control Point**: particles move with the control
  point's motion (translation, and rotation when "lock rotation" is 1);
  the lock fades out between the `start_fadeout` and `end_fadeout` life
  fractions.
- **random force**: a per-particle constant acceleration chosen uniformly
  between `min force` and `max force` (world axes).
- **Alpha Fade In Random**: alpha ramps from 0 over a life fraction
  (`proportional` 1) chosen uniformly in [min, max].
- **Alpha Fade Out Random**: alpha ramps to 0 over the last fraction
  (proportional) of life chosen in [min, max], shaped by the fade bias
  and ease flag.
- **Rotation Spin Roll**: roll speed `spin_rate_degrees` °/s.
- **Lifetime From Sequence** (life = the sheet sequence's frames / fps)
  only acts on a material with a sheet: `eject_minigunbrass`'s
  `effects/mini_casing` has none (its texture has no sheet resource), and
  real TF2's Minigun brass flies and falls for well over a frame
  (capture, `minigun.tf2`'s chase view), so its 2 s from "Lifetime
  Random" stands (inferred).
- **Collision via traces**: particles collide with the world (bounce and
  slide fractions); not used by muzzle flashes.
- The flame-specific ones (**Assign target CP**, **Movement Follow CP**,
  **Lifetime From Control Point Life Time**) are in flames.md §3.

Particle attribute numbers used by the remaps (from the public header):
0 position, 1 lifetime, 2 previous position, 3 radius, 4 roll, 5 roll
speed, 6 tint rgb, 7 alpha, 8 creation time, 9 sequence, 10 trail length,
11 particle id, 12 yaw, 13 second sequence, 16 alpha 2, 21 control-point
index.

### 8. Worked examples (stock values)

#### 8.1 Scattergun: `muzzle_scattergun` (one shot)

Started on the muzzle; children start with it (delay 0). All local
velocities are along the muzzle's axes (x out of the barrel).

| System | Material (blend) | Emission | Life | Radius | Motion | Colour, alpha |
|---|---|---|---|---|---|---|
| `muzzle_scattergun` | `effects/starflash01` SpriteCard, additive | burst 24 | 0.1–0.2 | 1, scale 1 → 0 (linear) | offset 0–10 forward; velocity local (10..210, −30..30, −30..30) + random 0–12; drag 0.1 | random between (253,251,135) and (255,229,192), fading to (205,87,0) (eased); alpha 1–255, fade in to 0.3 of life, out from 0.3 to 1 |
| `muzzle_scattergun_flash` | starflash01, additive | 512/s for 0.05 s (≈ 26) | 0.1 → 0.05 by creation time 0 → 0.05 | 8 → 5 for creation time 0 → 0.015 s, then 3 → 0.5 for 0.015 → 0.05 s; scale 1 → 0 | placed 2 → 24 units forward by creation time 0 → 0.05 s; velocity local x 700–900; drag 0.1; locked to the muzzle (lock fades 0.5 → 1 of life) | colour between (255,223,94) and (218,157,77) → (255,131,11); alpha 0.6 → 0 by creation time, fade in 0 → 0.5, out 0.5 → 1 of life; roll ±15° |
| `muzzle_shotgun_sparks` | `effects/brightglow_y_nomodel` UnlitGeneric, additive, trail | burst 8 | 0.1–0.3 | 1–2, scale 0.4 → 0 | 4–6 forward, 0–1 random; local (151..451, −40..40, −40..40); gravity −30; drag 0.2; random force (−1110..1111, 201..211, −1110..1111) | white → (255,121,1) by half life; alpha 1, fade out from 0.1 of life; trail length 0.1 s, max 4 |
| `muzzle_shotgun_smoke` | `effects/softglow_translucent` UnlitGeneric, alpha | burst 5 | 0.6–1.3 | 7–16, scale 0.3 → 0.5 (bias 1) | local (10, −10..10, −10..10) + random 0–12; gravity +54; drag 0.1 | white..(138,130,119) → (212,212,212); alpha 1–12 (/255), fade in 0 → 0.3, out 0.3 → 1; roll ±25° |

Screen size, world FOV 90, 16:9 (vertical tan 0.75), muzzle at eye
(44.29, 5.22, −4.28) re-projected to (44.29, 10.25, −8.40): the first
flash particle (radius 8, 2 units forward, depth ≈ 46.3) has NDC
half-height 8 / (46.3 × 0.75) = 0.230; at base FOV 75 (vertical tan
0.5755) 0.300. With the view-model camera (vertical tan 0.3821) it would
be 0.452: twice the real size.

#### 8.2 Shotgun: `muzzle_shotgun`

As 8.1 with: the core burst is **64** (life 0.1–0.2, radius 1 → 0, same
motion and colours), it is additionally locked to the muzzle (lock to
the end of life), and the flash child is `muzzle_shotgun_flash`, whose
values equal `muzzle_scattergun_flash` exactly. Sparks and smoke: the
same `muzzle_shotgun_sparks` and `muzzle_shotgun_smoke`. Engineer's
first-person muzzle (52.32, 8.06, −6.38): flash NDC half-height at world
90 ≈ 8 / (54.3 × 0.75) = 0.196.

#### 8.3 Minigun: `muzzle_minigun_constant` (continuous while firing)

| Child | Material (blend) | Emission | Life | Radius | Motion | Colour, alpha |
|---|---|---|---|---|---|---|
| `_core` | brightglow_y_nomodel, additive, trail | 120/s | 0.1–0.15 | 400, scale 1 → 0.02 with bias 0.9 (falls fast: 0.37 of the way at 5 % of life) | local x 710–910, offset (0, ±3, ±3); drag 0.1; locked to the muzzle with rotation | (255,250,200)..(255,222,152); alpha 64–128, fade in 0 → 0.4, out 0.4 → 1; trail length 0.2 → 0.1 as radius 0 → 250 (so 0.1 here), max 50 |
| `_sparks` | brightglow_y_nomodel, trail | 100/s (max 5 alive) | 0.3–0.6 | 1–2, 0.4 → 0 | 8–10 forward; local (151..851, ±40, ±40); gravity −330; drag 0.2; random force as 8.1 | white → (255,121,1); trail max 4 |
| `_starflash` | `effects/softglow` UnlitGeneric, additive, trail | 256/s | 0.1 | 8 | local (−125, ±300, ±300); drag 0.3; locked | (255,159,46)..(244,144,0) → (205,87,0); alpha 128–200; trail length 0.5, max 122; random roll |
| `_smoke` | `effects/wispy_smoke` UnlitGeneric, alpha | 12/s | 2–3 | 13–17, 0.3 → 0.5 | local (10, ±10, ±10) + 0–5; gravity +54; drag 0.1; roll 21°/s | alpha 32 (/255) with fade in 0 → 0.3, out 0.3 → 1 |
| `_flare` | starflash01 SpriteCard, additive | 128/s | 0.08 | 32, scale 1 → 0.1 with bias 0.7 | local x 500–900, 0–10 forward, 1 unit sphere; drag 0.1; locked | (255,223,94)..(218,157,77) → (255,131,11); alpha 70 (/255), fade in over 0.1 of life, Alpha Fade and Decay in 0 → 0.5, out 0.5 → 1; roll ±15° |

The `_core`'s 400-unit radius on a trail renderer would be a strip far
wider than its 50-unit maximum length; real TF2 draws the firing Minigun's
glow as a compact bright ball a few tens of units across at the barrel
(capture, 2026-11-19, `muzzle.tf2`), so a trail is taken as never wider
than it is long (§5; inferred from that picture, not measured per
particle). Heavy's firing muzzle (eye (66.56, 4.86, −20.54) at `m_spool_idle`
frame 0) re-projects at world 90 to (66.56, 9.54, −40.31).

#### 8.4 Other muzzle systems (summary)

`muzzle_pistol`, `muzzle_revolver`, `muzzle_smg`, `muzzle_sniperrifle`,
`muzzle_syringe`: core burst 32 of starflash01 (life 0.05–0.1, radius
2–3 → 0, local x 60–110); the pistol/revolver add `muzzle_pistol_sparks`
(4) and `muzzle_pistol_smoke` (25/s for 0.1 s, radius 4–8); the SMG and
sniper rifle add `muzzle_pistol_smoke` and `muzzle_scattergun_flash`;
the syringe gun's core has alpha 1–21 only. `muzzle_grenadelauncher` and
`muzzle_pipelauncher`: 6–7 slow smoke puffs (radius 7–16) plus
`_core` (255/s for 0.05 s, radius 16, local **z** 710–910) and `_embers`.

#### 8.5 Explosions, trails (names and scale)

- `ExplosionCore_Wall` (children: debris 800/s for 0.2 s, dust-up trails,
  12-sparkle core flash radius 1–12 × 3 → 11, 24 floating embers, 8 smoke
  puffs radius 3–5 × 2 → 12, a single flash radius 7 × 1 → 32 for 0.1 s,
  16 flying embers, a 3000/s 0.01 s flash-up): velocities are along the
  surface normal (local x positive), spreading ±840–940 sideways.
  `ExplosionCore_MidAir` has the same parts with velocities symmetric in
  all directions plus an `airburst_shockwave` ring (radius × 20 → 48);
  `_underwater` adds 64 bubbles. Which children draw what: §5.1 (the
  thin rays are the flying embers; the core flash is the wide orange
  star; the flash-up adds soft orange strips in the frame after the
  blast, §10.1).
- `ExplosionCore_MidAir_underwater`: the mid-air children
  (`Explosions_MA_*`) plus `Explosion_bubbles` (64 at once, speed
  U(120, 300) in a random direction, radius U(0.5, 2) × 3 → 0 eased, life
  U(0.3, 1), gravity +100 (rising), drag 0.1, alpha 128, white to
  (186,218,219) fading to grey, `effects/bubbles/bubble` SpriteCard
  translucent, animated at 20 frames/s). `ExplosionCore_Wall_underwater`
  is the wall set plus the same bubbles. The orange glow of a blast under
  water is `Explosions_MA_Flash_1`: one `effects/sc_brightglow_y_nomodel`
  sprite (additive SpriteCard), radius 7 growing × 1 → 32 (bias 0.75)
  over its 0.1 s life, (253,228,169)..(224,185,153) → (252,119,0), alpha
  212, fading out from 0.6 of life.
- `rockettrail`: smoke 150/s, life 0.8–1.2, radius 10 × 0.25 → 2,
  gravity +6, alpha 96–128, `effects/rocketrailsmoke`; plus `_burst`
  (sparks) and `_fire` (128/s glow, radius 5 × 3 → 0, life 0.2).
  The smoke takes (247,194,117)..(251,142,0) and fades to the white-grey
  (195,190,202) over its first 0.1 of life: the long white trail.
- `rockettrail_underwater`: identical to `rockettrail` except the smoke's
  life U(0.5, 1.2) (not 0.8–1.2), plus a third child
  `rockettrail_waterbubbles` (`effects/bubbles/bubble`, 255/s, radius
  U(1, 2) × 1 → 3, life U(0.2, 1.4), 5 units forward of the trail point,
  local speed (0..10, ±11, ±11), gravity +55 (rising), alpha 128–200,
  fading to (87,138,174), with a "prevent passing through a plane"
  constraint on control point 1, normal down: meant to keep the bubbles
  under the surface; the rocket sets no control point 1, Open questions
  9). Used by effects.md §9.
- `pipebombtrail_*` / `stickybombtrail_*`: soft glow 255/s for 2 s, life
  0.4, radius 64 × 1 → 0.1, alpha 16 (pipe) / 32 (sticky); the pipe adds a
  `pipebomb_timer_*` ring.
- `nailtrails_medic_*`: a trail of `effects/softglow`, 255/s, life 0.01,
  radius 1 (crit 1–6), alpha 128 (crit 255).

### 9. When and where a step's new particles are made

Read from the particle library itself (above); it holds for every Source
game using these particle files (CS:S's, HL2:DM's too), not only TF2.

#### 9.1 A system's clock and steps

- A system has its own clock T (seconds), 0 when it is created.
- **The frame it is created in**, it is simulated with **zero time**:
  every control point's "previous position" is set to its current one,
  the system's `initial_particles` are made (creation time T = 0, at the
  control point's current place: §9.3 with a zero step), and nothing else
  runs: **no emitter emits** and the system's **children are not
  simulated at all** that frame. (The client's manager does this for any
  system in its first frame.)
- **Every later frame**: the frame's time Δ (the client's frame time) is
  stepped in sub-steps of at most the system's `maximum time step` (0.1
  s in every TF2 file read; 0.1 also when the file gives 0 or less), at
  most 10 × that step in all per frame. For each sub-step h: T += h;
  then the operators flagged to run before emitters; then each emitter
  in file order, the particles it made being initialised at once by all
  initializers; then the other operators on every live particle (the
  new ones too). After the parent's sub-steps, each child system is
  stepped the same way with the whole Δ (its own first-frame setup
  included, see below). **After all sub-steps** every control point's
  "previous position" becomes its current position.
- **Children's first frame.** A child is first simulated in the frame
  after its parent's creation frame. Its own first-frame setup runs then,
  at the start of that step, and sets its previous positions to the
  control points' places **of that frame**; so a child's first emitting
  step sees its points standing still (no spread, §9.3), wherever the
  parent's point was the frame before.
- **Consequence:** nothing a system emits shows in the frame it is
  created; a burst (or a continuous emitter's first particles) appears in
  the next frame, one frame later than the event that started it. Only
  `initial_particles` exist in the creation frame (e.g. 1 for
  `rockettrail` and `rockettrail_underwater`: one smoke puff).

#### 9.2 Continuous emitter (`emit_continuously`): how many and when

Fields: rate R (particles/s, × the client's particle multiplier, 1 at
default settings; a negative rate is 0), start S (`emission_start_time`),
duration D (`emission_duration`, 0 = forever; negative = 0). Each
emitter keeps a running total E (start 0) and a count N of particles
emitted (start 0). In a sub-step that ends at T and is h long:

1. The **window** [a, b]:
   - D = 0 (forever): nothing while S > T; otherwise [a, b] = [T − h, T].
     The window is **not** cut at S (quirk: an endless emitter with S
     inside the step counts the whole step).
   - D > 0: nothing if S > T or T − h > S + D; otherwise
     a = max(S, T − h), b = min(S + D, T).
2. E += R × (b − a); n = floor(E) − N; N += n. If n is 0, nothing.
3. n is cut to the free room (`max_particles` − particles alive); N is
   not reduced, so particles lost to the cap are never made up.
4. The new particles' **creation times** are spread evenly over the
   window by **this step's count**, not by 1/R:

```math
c_k = \min\!\Big(a + k\,\frac{b-a}{n},\ b\Big),\qquad k = 1, \dots, n
```

   The first is one share **after** the window's start, never at it; the
   last is at its end b (normally the step's end T).

Worked: 128/s at 60 frames/s (h = 1/60): E after each step 2.13, 4.27,
6.4, 8.53, 10.67, 12.8, 14.93, 17.07 → n = 2, 2, 2, 2, 2, 2, 2, 3; in the
first step c = 1/120 and 1/60 s (half-way and the end of the step). 150/s:
n = 2, 3, 2, 3, ... (E = 2.5 a step).

**Stop and restart**: a stopped emitter makes nothing; a restart sets its
start to the system's current time and its totals afresh (as if the
system had just started).

**Instantaneous emitter** (for contrast, same library): all its particles
take the creation time S (its start time), whatever the step: a burst at
S = 0 is made in the first emitting step with creation time 0 = that
step's start, so it is placed at the control point's **previous**
position (§9.3) and has aged a whole step by the step's end.

#### 9.3 Where: the control point at a particle's creation time

The position initializers read control point i at the new particle's
creation time c (`Position Within Sphere Random`, `Position Within Box
Random`, `Position Modify Offset Random`, the initial velocity ones, ...;
`Position on Model Random` reads the model at its current pose):

```math
P_i(c) = P_i^{prev} + \big(P_i^{now} - P_i^{prev}\big)\cdot \max\!\Big(0,\ \frac{c - (T - h)}{h}\Big)\qquad (h > 0);\qquad P_i(c) = P_i^{now}\quad (h = 0)
```

- P^prev is where the point was at the end of the previous frame's
  simulation (§9.1), P^now where it was set this frame before simulating;
  the motion is taken as a straight line at constant speed over the
  step. No clamp above 1 (creation times never pass T).
- With §9.2: a continuous emitter's n particles of a step sit at
  fractions k/n of the point's path this step: **the last one on the
  point's current place, none on its previous place**. 128/s at 60
  frames/s: two particles a step, half-way and at the end of the path.
- Only the position is interpolated: a control point keeps no previous
  orientation, so the axes used (local-space offsets and speeds) are the
  point's **current** axes for every particle of the step.
- With several sub-steps in one frame (frames longer than 0.1 s) P^prev
  is still the frame's start but h is one sub-step (quirk; frames that
  long are outside normal play).
- In the creation frame (h = 0) and in a child's first step (P^prev =
  P^now) every new particle is on the point's current place.

#### 9.4 Answers this gives

- `Explosion_Flashup` (Open questions 8): 3000/s with S = 0, D = 0.01, in
  the first emitting step (h = 1/60) has the window [0, 0.01]: E = 30,
  so **30 particles are made**, at creation times 0.01 k / 30. It is not
  a step-boundary rule. **Corrected by §10.1:** their alpha is not 0 at
  birth, and in a rocket's blast they are drawn; the "nothing visible"
  capture started the system through an `info_particle_system`, whose
  client skips a late effect ahead past the emitter's 0.01 s window
  (§10.10), so there it makes no particles at all.
- A system started on a moving point (a projectile's trail): its first
  frame makes only its initial particles at the point's place in that
  frame; from the next frame each step's particles fill the point's path
  over the step, the newest on its current place. Its children start one
  frame later still with no spread in their first step.

### 10. Operator formulas read from the particle library

Read from the prebuilt particle library's machine code and its parameter
tables (above); every Source game using `.pcf` files shares them. Where
this section and §5.1, §7 or specs/cs_source/impact_effects.md §14
disagree, this section holds. Notation: t = age / lifetime (life
fraction), age = T − creation time (T the system clock at the end of the
current sub-step, §9.1), h = the current sub-step, S(s) = 3s² − 2s³ (the
"ease"/smoothstep), clamp01 = clamp to [0, 1]. "Initial" values are the
particle's attribute right after its initializers ran.

#### 10.1 Why `Explosion_Flashup` showed nothing, and what it does in a blast

The file (`particles/explosion.pcf`; no other file redefines it;
`drg_cowmangler.pcf` only lists it as a child) holds: emitter
`emit_continuously` rate 3000, start 0, duration 0.01, `max_particles`
45, no initial particles; initializers Rotation Random (0–360), Lifetime
Random U(0, 0.2), Radius Random U(1, 5), Color Random between
(191,54,0) and (255,42,0) (`tint_perc` 0), Sequence Random 0–5 (the
material has no sheet: no effect), Position Within Sphere Random
(distance 0, local speed x U(110, 920), y and z U(−840, 840)), Alpha
Random 255–255, Trail Length Random U(0.3, 0.4); operators in order
Radius Scale (3 → 11, bias 0.9, no ease, times 0–1), Movement Basic
(drag 0.1, no gravity), Color Fade (to (135,51,0) over 0–0.7, eased),
Rotation Spin Roll (212 °/s), Alpha Fade and Decay (start alpha 0, end
alpha 0, fade in 0.1–0.2, fade out 0.7–1.0); renderer
`render_sprite_trail` (min length 32, max 512, length fade-in 0.2 s,
visibility proxy off: control point −1, alpha and radius scales 1);
material `effects/softglow` (additive); every operator's own fade
("operator start/end fadein/fadeout", "fade oscillate") 0, which means
always fully on (§10.11).

**Nothing in it hides it.** By the formulas below, in a rocket's blast
(started by the client's particle dispatch, §9 timing, no skip): the
frame after the blast it makes 30 particles (§9.4) and draws them:

- Ages at the first draw a = 1/60 − k/3000 (k = 1…30): 0.0067–0.0163 s.
  Particles whose life ≤ a are removed in that same step (§10.2): about 2
  of 30.
- Alpha (§10.2): **1 while t < 0.1** (the fade-in has not started, the
  alpha is left at its initial 1), so every particle with life > 10 a is
  drawn at full alpha in its first frame: about 40 % of them (12 of 30).
  The others are inside the fade-in window: alpha = S((t − 0.1)/0.1),
  e.g. life 0.1, a 0.0117: t 0.117, alpha 0.077.
- Motion (§10.5, §10.6): this step's displacement is 0.949 v h (drag over
  half a 1/30 s period).
- Length (§10.7): τ × (a / 0.2) × 0.949 |v|, e.g. |v| 707, τ 0.35, a
  0.0117: 13.7 → **32** (the minimum).
- Width (§10.7): min(L, radius); radius = r₀ × (3 + 8 Bias(t)) with the
  rational bias of §10.3: r₀ 3, t 0.117 → 3 × 7.35 = 22.0 → width 22.
- Colour: the random orange, barely faded (S of 0.06/0.7 ≈ 0.02).

So in a blast it adds roughly a dozen soft orange strips 32 units long
and 3–40 wide (additive, full alpha) round the core in the frame after
the blast, longer the next frames (the length fade-in grows with age),
blending into `Explosion_CoreFlash`'s broad star. Ours drawing orange
streaks there is right in kind; check it against these numbers.

**The capture that showed nothing** (§5.1, "each child started alone")
started it with an `info_particle_system`. That entity's client creates
the effect on its first think and, when the client's time is more than
0.01 s past the server's start time (δ; the entity's first update always
arrives at least a tick after the server started it, *inferred*), skips
the new effect ahead to age δ (§10.10). The skip moves the system clock
to δ without simulating; a continuous emitter has no catch-up, so the
first real step's window starts at T − h ≥ δ > 0.01 = start + duration:
**no particle is ever made**. The same skip explains the other two
children in that capture: instantaneous bursts survive a skip under 2 s
and fire in the first step with creation time 0, already aged δ + h
(`Explosion_CoreFlash`, life ≤ 0.17 s, gone after about 7 frames instead
of 10, fits δ ≈ 0.04 s).

**Parity:** draw `Explosion_Flashup` in blasts (above). Make an
`info_particle_system` effect invisible only through the skip of §10.10
(every game: specs/source/particles_and_smoke.md 2.5), not through its
alpha or emitter.

#### 10.2 Alpha Fade and Decay

Parameters (defaults): start alpha (1), end alpha (0), start/end fade-in
time (0 / 0.5), start/end fade-out time (0.5 / 1), life fractions. On
load: an end before its start is raised to the start (both windows);
then if the fade-in's start is after the fade-out's start the two starts
swap, and likewise the two ends.

Each step, for each particle, with a₀ = its initial alpha:

1. If lifetime ≤ age: the particle is **removed** at the end of the step
   (this operator is what kills particles at the end of their life).
2. If start_in ≤ t < end_in: alpha = a₀ × (A_s + (1 − A_s) S(s)),
   s = clamp01((t − start_in)/(end_in − start_in)), A_s = start alpha.
3. If start_out ≤ t < end_out: alpha = a₀ × (1 + (A_e − 1) S(s)),
   s = clamp01((t − start_out)/(end_out − start_out)), A_e = end alpha.
   (Where the windows overlap this one is written last and wins.)
4. Otherwise the alpha is **left as it is**: the initial alpha before the
   fade-in, the last written value between the windows and after them.

Consequences: a fade-in that starts after 0 shows the particle at its
initial alpha first, drops it to A_s at start_in and ramps back up
(`Explosion_Flashup`: full alpha, then a pop to 0 at 0.1 of life). If a
step jumps past the end of the fade-in window the alpha stays at the last
in-window value (S of the last sampled s, not 1) until the fade-out
window, where it is recomputed from a₀. A window whose start equals its
end does nothing. Both ramps are eased (S), never linear. The fade-out
rescales a₀, not the current alpha.

#### 10.3 Radius Scale

Parameters (defaults): start time (0), end time (1) (life fractions),
start scale (1), end scale (1), ease in and out (0), bias (0.5; 0 read
as 0.5). If start time ≥ end time the operator does nothing. For
particles with lifetime > 0 and start ≤ t < end, with
s = (t − start)/(end − start):

```math
r = r_0 \cdot \big(k_s + (k_e - k_s)\,g(s)\big),\qquad g(s) = \begin{cases} S(s) & \text{ease on}\\ s & b = 0.5\\ \dfrac{s}{(1/b - 2)(1 - s) + 1} & \text{otherwise}\end{cases}
```

r₀ = the initial radius. Outside the window the radius is left as it is.
The bias here is the **rational** form (g(0.5) = b), not the math
library's power curve x^(−log₂ b) of §7: b 0.9 at s 0.05 gives 0.321, not
0.634; at s 0.1, 0.5. Ease, when on, replaces the bias.

#### 10.4 Color Fade

Parameters (defaults): target colour (255,255,255), fade start time (0),
fade end time (1) (life fractions), ease (1). If start = end it does
nothing. Each step, every particle with lifetime > 0:
s = clamp01((t − start)/(end − start)), eased by S when ease is on;
colour = c₀ + (target − c₀) s per channel, c₀ the initial colour; colours
are 0–1 gamma values (byte / 255), mixed in that space. Before the start
the particle keeps c₀; after the end it holds the target.

#### 10.5 Movement Basic

Parameters: gravity (vector), drag, max constraint passes. Positions are
integrated from the current and previous position (field 2); there is no
stored velocity. Each step, with P_prev = the previous dt (the whole
frame time of the system's last simulated frame; 0.05 for a new system
before its first real frame; the skip length after a skip, §10.10):

```math
x' = x + (x - x_{prev})\,\frac{h}{P_{prev}}\,(1 - \max(\text{drag}, 0))^{30h} + a\,h^2
```

a = gravity + the system's forces. Then the old position becomes the
previous position, and constraints run. **Drag is the fraction of
velocity lost per 1/30 s**, not per step (0.1 → 5.1 % per 1/60 s step).

#### 10.6 Position Within Sphere Random: the initial velocity

The initial velocity v (random direction × U(speed min, max) plus the
per-axis U(local min, local max) in the control point's axes) is stored
by setting the previous position to x − v × P_prev (§10.5). With §10.5's
h / P_prev factor the first step moves the particle v h (less drag)
whatever P_prev is.

#### 10.7 Sprite trail renderer (replaces §5.1 steps 2–3 where they differ)

Parameters (defaults): min length (0), max length (2000), length fade-in
time (0), animation rate, the visibility-proxy set. Per particle, after
the step:

1. m = previous position − position (this step's motion, pointing to the
   tail); k = 1/h for the last sub-step's h (k = 1 when h is 0).
2. f = age / t_fade if age < t_fade, else 1 (t_fade 0: f = 1).
3. L₀ = k × τ × f × √(|m|² + 10⁻¹⁰). If L₀ ≤ 0 (f = 0: a particle of age
   exactly 0; or τ ≤ 0) the particle is **not drawn**.
4. L = max(min(L₀, max length), min length): the minimum wins when the
   two cross.
5. Full width w = **min(radius, L)** (radius after the operators; this
   cap is confirmed).
6. The strip runs from the head (the position) toward m/|m| for L, its
   half-widths ±w/2 across, facing the eye. A particle that did not move
   has m ≈ 0: its direction is 0, so the strip has no length whatever L
   is; nothing visible (still true with a min length).
7. Vertex colour = the particle's colour × 255; vertex alpha = its render
   alpha byte (§10.8); a particle with byte 0 is skipped.

#### 10.8 The render alpha

The renderers draw from a list built each frame: per particle the
squared distance to the eye (the sort key), the radius and an alpha byte
= round(255 × alpha × alpha 2) (field 16; 1 unless an operator writes
it). The sprite trail skips particles whose byte is 0 (alpha below
1/510); for sprites the same list is used (*inferred*: same skip).

#### 10.9 Oscillate Scalar (and Oscillate Vector)

Parameters (defaults): field (7, alpha), rate min/max (0/0), frequency
min/max (1/1), "proportional 0/1" (1), start time min/max (0/0), end time
min/max (1/1), "start/end proportional" (1), multiplier (2), start phase
(0.5). Each particle has four fixed random numbers for its life (from the
system's random table by particle id; independent): rate
R = U(rate min, max), frequency F = U(frequency min, max), start
t₁ = U(start min, max), end t₂ = U(end min, max).

Each step, for particles with lifetime > 0 and t₁ ≤ τ_p < t₂ (τ_p = the
life fraction when "start/end proportional" is 1, else the age in
seconds):

```math
x = \begin{cases} F \cdot M \cdot \dfrac{\text{age}}{\text{lifetime}} + \varphi & \text{proportional}\\[4pt] F \cdot (M\,T + \varphi) & \text{not proportional}\end{cases}\qquad v \mathrel{+}= R \cdot h \cdot \text{str} \cdot W(x)
```

M = multiplier, φ = start phase, T = the **system** clock (so with
proportional 0 all particles of a system share the phase, differing only
by F), str = the operator's strength (1 unless its own fade, §10.11). W is
a parabolic stand-in for sin(πx), period 2 in x: with f = the fractional
part of |x| measured from the last even integer, W = sign(x) × 4f(1 − f)
for f in [0, 1) and −sign(x) × 4(f − 1)(2 − f) for f in [1, 2). The
operator **adds** to the field each step (it oscillates the field's rate
of change, not the field itself), sampled once per step at the step's
end: at frame rates below 2 × M × F the result aliases. When the field is
alpha (7) the result is clamped to [0, 1]; other fields are not clamped.

**Oscillate Vector**: the same per component of a vector field (default
field 0, position), with per-component rate and frequency (vector
min/max, defaults 0 and 1) and per-component random numbers; tint (field
6) is clamped to [0, 1] per component.

Worked (the crit glow, effects.md §14.1; all four have proportional 0,
start/end proportional 1 over 0–1): `critgun_weaponmodel_*` radius: R 64,
F U(0.5, 2), M 16, φ 0.5 → x = F (16 T + 0.5); one half-period of x (W ≥
0) lasts 1/(16 F) s and adds 64 × (2/3) / (16 F) = 2.67/F units, then the
next half takes it back: the radius swings by about 2.7 units at F 1, at
8 F Hz. Alpha: R U(1, 128), F U(2, 8), M 64: x advances 64 F per second
(128–512 per second, 2–8.5 per 1/60 s step), so at 60 frames a second W
is sampled at effectively unrelated points each step: the alpha jitters
by up to R/60 per step (≈ 2 at R 128, clamped to 0–1), a flicker that
depends on the frame rate. `_glow` variants: radius R U(32, 64), M 12;
alpha R U(1, 2), M 64 (a slow drift of at most 0.03 a step).

#### 10.10 Skip ahead (a late `info_particle_system`)

Skipping a system to time t (the client does it for an
`info_particle_system` effect when its time is more than 0.01 s past the
server's start time: t = that difference):

- If t ≤ T nothing happens. Otherwise: P_prev (§10.5) = t − T; every
  control point's previous position = its current one; T = t; the
  first-frame state is cleared, so the zero-time creation step of §9.1
  never runs: **`initial_particles` are not made**.
- Nothing is simulated over the skipped time. Each operator gets a skip
  hook: an **instantaneous emitter** cancels its burst if t > its start
  time + 2 s, otherwise the burst fires in the next step with creation
  time = its start time (already aged t + h when first drawn); a
  **continuous emitter** has no hook: from the next step its window rule
  (§9.2) applies to [T − h, T] with T − h ≥ t, so an emitter that ends
  before t never emits and an endless one starts from t without
  back-filling.
- Children are skipped to t minus their delay, the same way.

#### 10.11 An operator's own fade

Every operator has "operator start/end fadein/fadeout" and "fade
oscillate" (all 0 in every system read here). With them all 0 the
operator always runs at strength 1; renderers likewise. (Other values
scale the operator's strength over the system's time; not used by these
effects.)

## Per-frame order

1. Control points updated from attachments (re-projected for first-person
   weapons) and from code (flames).
2. Systems simulated: emitters, initializers for new particles,
   operators in file order, forces, constraints; then dead particles
   removed. The maximum step is 0.1 s. A system in its first frame is
   simulated with zero time and its children not at all (§9.1).
3. Translucent world pass: systems drawn sorted back to front (a system's
   particles sorted when "Sort particles" is set), after opaque geometry,
   before the view model.

## Edge cases

- A sprite larger than `$endfadesize` × l vanishes instead of filling the
  screen (the flame core at point-blank).
- r_drawviewmodel 0 also suppresses first-person muzzle flashes
  (effects.md §2).
- On DX8-level hardware: no depth feathering, overbright and addself are
  quartered for the pixel-constant range, the flame material's `$maxsize`
  is 0.15 (its `<dx90` block).

## Quirks

- The flame material's `$endfadesize` (0.5) is larger than its
  `$maxsize` (0.4): fading starts at 0.3 × l, and a sprite is clamped to
  0.4 × l while still faded by the not-yet-clamped size.
- UnlitGeneric particles (most sparks and smoke) have no size clamp at
  all.

## Test cases

| Setup | Expected |
|---|---|
| SpriteCard r = 10 at l = 20, `$maxsize` 0.4, `$startfadesize` 0.3, `$endfadesize` 0.5 | 10 > 6: fade factor 1 − (10 − 6)/(10 − 6) = 0 → not drawn |
| same, r = 7 | factor 1 − (7 − 6)/4 = 0.75; r clamped to 8 → 7 (unchanged) |
| same, r = 7.5 at l = 20, `$maxsize` 0.35 | factor 0.625, r → 7 |
| first-person flash, world FOV 90, view FOV 54, attachment at eye (40, 6, −5) | world point (40, 11.78, −9.81) |
| a particle of radius 8 at depth 46.3, world FOV 90, 16:9 | NDC half-height 0.230 (not 0.452) |
| `flamethrowerfire102`, sequence 3, fit lifetime, t = 0.5 | frame position 9.5: frames 9 and 10 of sequence 3 mixed 50/50 |
| bias(0.05, 0.9) | 0.05^0.152 = 0.634 |
| trail, |v| = 1500 (this step's motion / Δt), τ = 0.08, t_fade 0.1, age 1/60 s, L in [0, 2000] | L = 1500 × 0.08 × 0.1667 = 20 |
| same, age 0.2 s | L = 120 (fade complete) |
| `Explosion_CoreFlash` particle, |v| = 1400, τ = 0.4, age 0.05 | 1400 × 0.4 × 0.25 = 140 → clamped to 122 |
| same, age 1/60 | 46.7 (above the minimum 2) |
| trail particle locked to a still control point, no movement operator, start velocity 1000 | nothing drawn (no motion this step) |
| trail particle r₀ 0.8, × 3 → 0 eased (bias 0.5), life fraction 0.2 | strip full width 0.8 × 3 × (1 − 0.104) = 2.15 |
| `Explosion_FlyingEmbers` alone on a floor 160 units ahead, fov 90, 1024x768 | thin rays ~1 unit wide, each with a dot ~3× wider at its head, from the first frame; streaks reach back to ~the centre for ~0.1 s, then detach |
| `Explosion_CoreFlash` alone, same view | broad soft orange streaks (tens of units wide), gone after ~7 frames at 60 a second |
| `Explosion_Flashup` alone by an `info_particle_system`, same view, stepped at 60 a second, client 0.04 s behind the start | skipped to 0.04: no particle ever made, no visible change in any frame |
| `Explosion_Flashup` in a blast (no skip), first emitting step 1/60 | 30 made; those with life > 10 × age (≈ 12) at alpha 1; lengths 32 (min); widths min(32, r₀ (3 + 8 g(t))) |
| Alpha Fade and Decay, start alpha 0, fade in 0.1–0.2, out 0.7–1, a₀ 1; t = 0.05, 0.15, 0.25 (steps sampled at those t) | 1 (untouched), S(0.5) = 0.5, 0.5 (left from the last in-window step) |
| Alpha Fade and Decay, lifetime 0.01, age 0.0117 | removed this step |
| Radius Scale 3 → 11, bias 0.9, no ease, r₀ 2, t 0.05 | g = 0.05/0.1556 = 0.321; r = 2 × (3 + 8 × 0.321) = 11.1 |
| Movement Basic, drag 0.1, h 1/60, previous dt 1/60, last step's motion (10, 0, 0) | this step's motion (9.49, 0, 0) |
| Sprite trail, L₀ 13.7, min 32, max 512, radius 22 | L 32, width 22 |
| Sprite trail, min 32 > max 20, L₀ 50 | L 32 (the minimum wins) |
| Oscillate Scalar on alpha, R 1.2, h 1/60, x 0.5 (W = 1), alpha 0.99 | 0.99 + 0.02 = 1.01 → clamped to 1 |
| Oscillate Scalar, x = 1.25 | W = −4 × 0.25 × 0.75 = −0.75 |
| Skip to 0.04 of a system with an instantaneous burst at 0 and a 0.01 s continuous emitter | next step: the burst made with creation time 0 (aged 0.04 + h); the continuous emitter nothing; no initial particles |
| System with `initial_particles` 1 and a 128/s continuous emitter created this frame | this frame: 1 particle (at the point's place), emitter makes nothing; children not stepped |
| Same system's next frame (h = 1/60), point moved from A to B this frame | 2 new particles, creation times 1/120 and 1/60 s, at (A + B)/2 and B; none at A |
| Its child's first step, same frame | prev = now = B: the child's particles all at B |
| 150/s, 4 steps of 1/60 | 2, 3, 2, 3 particles; within a step at fractions k/n of the path |
| Endless emitter, S = 0.005, step [0, 1/60], 120/s | window [0, 1/60] (not cut at S): E = 2, particles at 1/120 and 1/60 |
| Emitter 3000/s, S 0, D 0.01, first step 1/60 | 30 particles, creation times 0.01 k/30 |
| Instantaneous burst at S = 0, point moved A → B in the first emitting step | all at A (creation time = window start) |
| addself material, T = (1, 0.5, 0.2, 0.8), v = (1, 1, 1, 0.25), O = 5, S = 0.5, f = 1 | P = (1, 0.5, 0.2) × 5 × 0.8 × 0.25 = (1, 0.5, 0.2); out.rgb = P × (1 + 0.625) = (1.625, 0.8125, 0.325); out.a = 0.2 |

## Open questions

1. The sheet frame rate when "animation_fit_lifetime" is off and "use
   animation rate as FPS" is off (rate 0.1 on almost every TF2 sprite):
   whether the rate is sequence loops per second or a scale on the
   sheet's own frame times. Matters only for sheeted materials (the flame
   core uses fit-to-lifetime; the muzzle materials have no sheet).
2. The depth-feather normaliser (the constant computed from
   `$depthblendscale` by the shader API) and the depth range stored in
   destination alpha.
3. Whether the `srgb?` condition holds on TF2's default PC renderer
   (overbright 5 for the flame core) or not (1). **Answered by capture
   (2026-11-19, tools/tf2ref: the stock Flame Thrower held by a puppet
   bot, watched in its eyes, 0.5 and 1.0 s into the stream on ctf_2fort's
   bridge, real time at 60 frames a second, 1024x768):** it holds, 5. In
   open air the stream's far ball is a bright yellow-white fireball only
   a factor near 5 reaches; at a wall 60 units off the stream is a soft
   translucent violet cloud, adding about (6, 4, 15) to (8, 5, 20) to the
   mean colour (0-255) of a 300 x 200 pixel area around it. The factor
   multiplies the texel in linear space (§2: the vertex colour is taken
   to linear first): applied to the gamma colour instead it is about 34
   in linear and the cloud at the wall burns white (+25 on every
   channel).
4. How a sprite trail uses the radius (half-width or width) and the trail
   length attribute's unit. **Answered by capture (2026-11-19,
   tools/tf2ref `tracers.tf2`: the stock Shotgun's and SMG's first shot in
   first person on ctf_2fort's bridge, 1024x768, fov 90, every frame at 60
   frames a second, beside ours with the same view):** the radius is the
   strip's full width (radius 2: about 2 pixels wide at 500 units, half
   what a half-width of 2 drew); the trail length is in seconds of travel
   (0.1 s default: the streak grows to about 600 units at 6000 units/s,
   the same in both). `effects/tracer1` is dark at its top row (v = 0) and
   bright at the bottom (v = 1), centred across u; real TF2's tracers are
   brightest at the bullet and fade back toward the gun, so v = 1 is the
   head.
5. (Implementation note.) §4 reads the SpriteCard size limits as
   fractions of the distance from the eye; specs/cs_source/
   impact_effects.md 14 said "of the screen". The shader reading here is
   used for every Source game's sprite cards.
6. Where the Minigun's casings (`eject_minigunbrass`) land in third
   person. **Measured (2026-11-20, tools/tf2ref, a puppet Heavy firing
   level on 2fort's bridge, watched from 260 units straight above):**
   after 1.5 s of fire the casings lie in a band about 40 to 75 units out
   to the gun's left (seen from above, the Heavy facing down the screen),
   most near 58, few under the gun; seen from the side they leave upward
   past the ammo box before falling. Read with this file's numbers (local
   x 100-200 units/s, drag 0.05 a frame, gravity −252, no bounce or
   slide), a casing leaving level from about 40 units up lands about 40
   units out; the farther band needs the eject attachment's x axis
   tilted upward (a longer flight), which depends on the spun Heavy's
   pose (player_animation.md). Unconfirmed.
7. **A sprite trail's length fade-in and a still particle** (measured
   2026-11-20, tools/tf2ref): the Flame Thrower's `new_flame_pilot`
   (locked to the muzzle, no movement operator, a start speed of
   800-1200 forward, trail length 0.1) shows no streak while the gun is
   still, so a trail seems to follow the particle's motion, not its
   start velocity. A rocket's blast on the floor at 150 units shows long
   yellow streaks (Explosion_Flashup / CoreFlash, trail lengths 0.3-0.4,
   length fade-in 0.2) already 33-50 ms after the blast: longer than a
   fade-in by age (length x age / 0.2) gives. Whether the fade-in is by
   age, by life fraction or from the minimum length is unconfirmed.
   **Frame by frame** (2026-11-20, `streaks.tf2`: the same rocket into
   the bridge's floor about 200 units off, every frame at 60 frames a
   second, 1024x768, fov 90, `mat_hdr_level` 2, no colour correction):
   frame 0 of the blast shows the flash only; at 17 ms short rays
   (about 25-50 units) stand round the core; at 33 ms thin yellow rays
   of about 100 units; at 50 ms some reach past the top of the view
   (150 units and more); from 67 ms they thin out among the debris. Every
   ray is thin: 2-4 pixels across at that distance, about 1 unit, while
   the two systems' radius operators scale their radius 3 to 11 times
   (Explosion_CoreFlash: life 0.05-0.17 s, trail length 0.33-0.4,
   lengths 2-122; Explosion_Flashup: life 0-0.2 s, lengths 32-512). Drawn
   with the radius-scaled radius as the width (ours), the streaks are 10
   to 110 units wide and merge into an orange haze; drawn with the
   unscaled radius and a fade-in by life fraction (tried) they were still
   no rays like these. Both the width and the fade-in rule stay open.
   **Answered (2026-11-20, each child of the blast started alone, §5.1):**
   the rays are not `Explosion_CoreFlash` or `Explosion_Flashup` but
   `Explosion_FlyingEmbers` (radius 0.5–1 × 3 → 0, `effects/circle2`,
   trail plus a head sprite). The trail rule is §5.1: full width = the
   radius after the radius operators (so the core flash's 10–110-unit
   glows are right), length = this step's speed × τ × min(1, age /
   t_fade), clamped. Ours' haze is the core flash, correctly; what was
   missing is the flying embers' rays. Things in that system an
   implementation may trip on: its speed range has min above max on
   every axis (x 2200 → 20, y and z 1500 → −1500) and its trail length
   range too (0.1 → 0.05): pick uniformly between the two either way;
   its Alpha Fade and Decay starts at 0 and ends at 1 (fades in over
   0–0.7 of life and never fades out); it has two renderers.
8. Why `Explosion_Flashup` draws nothing (measured alone, §5.1): its
   emitter runs 3000/s for only 0.01 s, less than one 60 Hz step, and
   its alpha starts at 0 until 0.1 of life. Whether a continuous emitter
   whose window ends inside the first step emits nothing (a step-boundary
   rule), or its particles are emitted and simply never visible, is
   unconfirmed. Parity target either way: nothing visible from it at
   60 frames a second.
   **Answered (§9.4):** there is no step-boundary rule; the 30 particles
   are emitted in the first emitting step (one frame after the blast's
   system is created) and are simply not visible.
   **Corrected (2026-11-20, tf2-spec-flashup, §10.1):** they are visible
   in a blast (alpha 1 before 0.1 of life, lengths at least 32). The
   alone capture showed nothing because its `info_particle_system`
   client skipped the late effect ahead (§10.10) past the emitter's
   0.01 s window, so no particle was made. Parity target: draw it in
   blasts; implement the skip for `info_particle_system`.
9. `rockettrail_waterbubbles`' plane constraint reads control point 1,
   which the rocket never sets; what an unset control point holds (the
   world origin, or control point 0) decides whether bubbles of a rocket
   are clipped at all. Unconfirmed; a capture of the underwater trail's
   bubbles near the surface answers it.
