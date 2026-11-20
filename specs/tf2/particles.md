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
headers. Nothing copied; restated in our words.
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
- **render_sprite_trail** (inferred): a quad from the particle's position
  back along its velocity, length = speed × trail length (field 10, in
  seconds; "Trail Length Random" sets it) clamped to [`min length`,
  `max length`], grown in from 0 over `length fade in time` (seconds of
  age), as wide as the radius (the radius is the strip's full width, not
  its half-width) but never wider than it is long (inferred: §8.3's
  Minigun core), the texture's v running from 0 at the tail to 1 at the
  head (measured: Open questions 4). Used for sparks, tracers and the
  Minigun core.

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
specs/cs_source/impact_effects.md §14. The **bias curve** is
bias(x, b) = x^(−log₂ b) (b = 0.5 linear; b > 0.5 rises early). The
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
  `_underwater` adds 64 bubbles.
- `rockettrail`: smoke 150/s, life 0.8–1.2, radius 10 × 0.25 → 2,
  gravity +6, alpha 96–128, `effects/rocketrailsmoke`; plus `_burst`
  (sparks) and `_fire` (128/s glow, radius 5 × 3 → 0, life 0.2).
- `pipebombtrail_*` / `stickybombtrail_*`: soft glow 255/s for 2 s, life
  0.4, radius 64 × 1 → 0.1, alpha 16 (pipe) / 32 (sticky); the pipe adds a
  `pipebomb_timer_*` ring.
- `nailtrails_medic_*`: a trail of `effects/softglow`, 255/s, life 0.01,
  radius 1 (crit 1–6), alpha 128 (crit 255).

## Per-frame order

1. Control points updated from attachments (re-projected for first-person
   weapons) and from code (flames).
2. Systems simulated: emitters, initializers for new particles,
   operators in file order, forces, constraints; then dead particles
   removed. The maximum step is 0.1 s.
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
