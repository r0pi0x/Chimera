# Team Fortress 2: the Flame Thrower's flames (motion, damage volume, and the visible stream)

Source basis: Valve's Source SDK 2013 (2025 TF2 release) read in place in
a spec session: the flame thrower weapon, its flame manager and the
shared point manager (point creation, motion, wall handling, the hit
test, the damage scale, the networking of points and the client's
copy of them, and how the client drives the flame particle system from
the points); the item schema's flame attributes. TF2's
`particles/flamethrower.pcf` and its materials and texture sheet read in
place with our dumper (values only; nothing committed). The particle
operator library is not in the SDK, so the meaning of the three
flame-specific operators is *inferred* from their names and parameters.
Nothing copied; restated in our words. This continues
[weapons_more.md](weapons_more.md) §7, which holds the damage numbers.
Status: draft

## Summary

The stock Flame Thrower (since the 2017 rework) fires **flame points**:
up to 30 small boxes per Pyro that fly forward from just ahead of his eye,
slow down hard and drift up. The server hurts whatever a point's swept
box touches. **Every client runs the same points** (same spawn ticks,
same random seed) and feeds them to one particle system per burst,
`new_flame`, as its control points: each flame sprite is born at the
gun's muzzle and rides one point. So the visible fire *is* the damage
volume, drawn larger: a sprite's half-size grows from about 9 to 23 units
while the hit box's half-size is 12. Flames that look different or hurt
somewhere else come from not tying the sprites to the points.

## Units and conventions

Hammer units, z up, seconds; server tick 0.015 s. "Eye" is the Pyro's eye
position; forward/right/up are his eye angles' axes (as networked).
Control point (CP) numbers are the particle system's.

## Constants

| Name (ours) | Value | Unit | Meaning |
|---|---|---|---|
| flame_origin | 40 forward, 5 right, 0 up (× model scale) | units | where a point spawns, from the eye |
| flame_speed | 2450 | units/s | initial speed (item attribute) |
| flame_drag | 8.5 | 1/s | speed loss per second, as a fraction of speed |
| flame_up | 50 | units/s | constant upward drift added to every step |
| flame_gravity | 0 | units/s² | (stock) |
| flame_spread | 2.8 | deg | spread scale per axis |
| flame_life | 0.6 ± 0.1 | s | uniform |
| flame_box | 12 × size mult | units | half-size of the point's hit box; size mult = lerp(start mult, end mult, age / life), both 1 for the stock weapon |
| points_max | 30 | – | live points per manager (Pyro) |
| emit | 0.02 nominal (every 2nd tick at 0.015) | s | one point per attack |
| cp0_radius | 3 | units | the muzzle CP's radius |
| min_density | 0.01 | – | a point's CP density at the end of its life |
| core_rate | 100 | 1/s | flame sprites emitted while firing |
| core_radius | 23 | units | the sprite's base radius |

The old flame-entity tuning values (2300 speed, 0.87 drag, 350 maximum
damage distance, 1.2 short-range multiplier) belong to the pre-2017
flames and are **not** used by the stock weapon.

## Behavior

### 1. Points: creation and motion (server and every client)

Restating weapons_more.md §7.1 with the details the visuals need:

- **Spawn**: while firing (eyes not under water), each attack adds a point
  if fewer than 30 are alive. Its slot index cycles 0..29; the server
  records the spawn tick for that slot and networks it with a per-manager
  random seed (chosen once, 0–9999).
- **Initial state**, seeded with spawn tick + manager seed + slot:
  position = eye + 40·forward + 5·right (scaled by the player's model
  scale); velocity = 2450 × the aim (eye angles) deviated by the spread
  (weapons_more.md §7.1: per screen axis (u₁ + u₂) × 2.8° in radians,
  uᵢ uniform in [−0.5, 0.5]); life = 0.6 + uniform[−0.1, 0.1]; the
  Pyro's own velocity at that moment is stored with it.
- **Each update** with step dt (server: every tick; client: every
  frame, its frame time):
  1. remove the point if its age exceeds its life, or its centre is in
     water;
  2. step velocity = velocity + (0, 0, gravity·dt) + (0, 0, 50) + the
     stored Pyro velocity projected onto the point's current direction;
  3. trace the box (half-size 12) from the position to position +
     dt × step velocity against the world (solid only). Start in solid:
     remove. A hit: count a wall hit; the step velocity loses its
     component into the surface; the stored Pyro velocity is cleared;
     the new position is the hit point + 12 × the surface normal, moved
     on by the unused fraction of the step along the slid velocity. No
     hit: the step velocity is discarded and the old velocity is used for
     the drag below;
  4. drag: speed' = clamp(speed − dt × 8.5 × speed, 0, speed), direction
     unchanged; new velocity = speed' × direction (+ gravity·dt, 0 here);
  5. previous position ← position; position ← the new position.
- **Quirk of the slide**: after a wall hit the slid velocity, which
  includes the +50 drift, becomes the point's velocity, so a point running
  along a wall keeps gaining 50 units/s upward per update (before drag).
- **Client copy**: the client creates each point when its clock reaches
  the slot's networked spawn tick (a late one is advanced by the time it
  missed), with the same seed, so its positions match the server's up to
  the step size (frame vs tick) and the eye angles it knows. A Pyro's own
  flames are not predicted: they appear one network update after he
  fires.

### 2. Damage (server; weapons_more.md §7.2 unchanged)

- Each tick, for each enemy whose bounds overlap the manager's bounds
  (the union of all its points' boxes), the points are tested oldest
  first: the point's box swept from its previous to its current position
  against the target's hitboxes; the first that hits is the touch.
- Heat, the 0.075 s per-target interval, D = max(1, 13 × age factor ×
  heat factor), crits and afterburn: as weapons_more.md §7.2–7.3. The age
  factor falls from 1 to 0.5 over the first half of the point's life
  (lifetime mode; the distance mode is off).
- There is **no range check and no cone**: the damage reaches exactly as
  far as the points travel (≈ 340 units ahead of the eye for a standing
  Pyro, more when he runs forward, since his velocity along the flame is
  added while the point lives).
- Wall hits do not reduce damage for the stock weapon (factor 1 per hit).

### 3. The visible stream

#### 3.1 Which system, when

- When the manager starts firing (each new burst, also when a manager is
  created mid-burst), the client creates a fresh particle system for it,
  not attached to any model, named: `new_flame`; for a critical burst
  `new_flame_crit_red` / `new_flame_crit_blue` (the Pyro's team); with
  the eyes under water `flamethrower_underwater`. When firing stops, the
  system stops emitting (sprites alive finish) and is removed once it has
  no particles; a manager with no points left removes everything.
- A burst that turns critical (or stops being critical) restarts the
  weapon's effect with the other name.
- Airblast: `pyro_blast` on the effect model's `muzzle` (first person:
  the view model's, re-projected; particles.md §6).

#### 3.2 Control points (set every client frame)

- **CP 0** = the **weapon's own (third-person) model's** `muzzle`
  attachment: its position and orientation; radius 3; density 1;
  velocity = the newest point's velocity + its stored Pyro velocity.
- **CP i + 1** for the point in slot i: position = the point's position;
  density = 1 → 0.01 linearly over the point's life; radius = the point's
  box half-size (12); duration = the point's life; velocity = its
  velocity + its stored Pyro velocity.
- When a point is added, its CP becomes the system's **active** CP: the
  sprites emitted from then on are assigned to it.

#### 3.3 `new_flame` (parent: no particles of its own, max 512)

| Child | Material | Emission | What it is |
|---|---|---|---|
| `new_flame_core` | `particle/flamethrowerfire/flamethrowerfire102` SpriteCard, premultiplied (`$addself` 0.5, `$overbrightfactor` 5 on the sRGB path), soft edges (`$depthblend` 1, scale 100), size fade 0.3 → 0.5 × l, `$maxsize` 0.4 (0.15 below DX9) | 100/s while the system emits; max 350 | the fire: one sprite per 0.01 s, each riding a point |
| `new_flame_pilot` | `effects/singleflame` UnlitGeneric, additive, trail | 64/s, max 20 | the blue jet at the nozzle |
| `new_flame_drips_a` | `effects/brightglow_y_nomodel` additive trail | 12/s, max 12 | falling burning drops |
| `new_flame_drips_b` | `effects/sc_brightglow_y_nomodel` SpriteCard additive | 50/s, max 30 | fast embers along the stream |

**Core sprite** (all values from the file; operator meanings inferred
where marked):

- Born at CP 0 (the muzzle), no velocity of its own.
- Assigned to the active CP (the newest point) and **follows it**
  ("Movement Follow CP", catch-up speed 100; inferred: it is pulled from
  the muzzle onto its point and then moves with it; Open questions 1).
- Lifetime = its point's duration (0.5–0.7 s) ("Lifetime From Control
  Point Life Time"; inferred).
- Radius = 23 × s_d × s_t where s_d = 0.5 + 0.5 × clamp(distance to CP 0
  / 64, 0, 1) (multiplies the initial value) and s_t = 0.4 + 0.6 × t
  (linear over life). So about 4.6 units at the muzzle, 9.2 just after
  leaving it, 23 at the end of life.
- Colour: the system's colour (245, 240, 239) with its tint set from the
  distance to CP 0: (0.2, 0.2, 1) at 0 → (1, 1, 1) at 200 units and
  beyond (blue near the nozzle, natural further out; whether the tint
  replaces or multiplies the system colour is unconfirmed, ≤ 6 %
  difference).
- Alpha: uniform 32–64 (/255), then faded out over the last 75 % of its
  life (eased, bias 0.5: "Alpha Fade Out Random" 0.75–0.75, proportional),
  and finally the material's size fade.
- Roll uniform 0–360°, half the sprites mirrored; sheet sequence uniform
  in 2–4 (particles.md §3), the whole sequence (18–19 frames) played once
  over the sprite's life, frames blended.

**Pilot**: born at CP 0 with local velocity (800–1200, ±25, ±25), life
0.2 s, trail length 0.1 s, radius 7 × 1 → 3 over the first half of life
(bias 0.1), colour (0,126,255)..(89,85,255) → (243,144,27) by half life,
alpha 1 fading in over 0.1 and out from 0.85 of life; locked to CP 0 and
assigned to the active CP with catch-up 0 (it does not chase the point).

**Drips a**: 1 unit from CP 0, local x speed 264–710, life 0.1–1.0,
gravity −150, drag 0.1, radius 0–2, black → (255,150,0), trail; collide
with the world (bounce 0.2, slide 0.2). **Drips b**: local
(1200–1500, −200..150, −200..150), life 0.1–0.6, gravity +25, drag 0.2,
radius 0.2–2 × 1 → 0.5, (255,162,0)..(255,216,0) → (255,42,0), alpha
100–255 faded in to 0.125 and out from 0.75 of life, a per-axis
oscillation, world collision (bounce 1).

**Crit** (`new_flame_crit_<team>`): the same core and pilot, plus
`new_flame_crit_<team>_glow` (30/s, life 1, radius 5–7 × 2 → 3, local
800–1000, gravity +250), `…_sparks` (360/s trails, local 1500–1800) and
`…_drips_b` (75/s); no drips a.

#### 3.4 How the picture lines up with the damage

- Each core sprite sits on a damage point (after its pull from the
  muzzle), so the stream bends, slides along walls, stops at walls and
  ends where the points end, exactly as the damage does.
- The sprite's half-size is 0.4–1 × 23 × (0.5–1): smaller than the hit box
  (12) for its first ~25 % of life, then larger, up to 23. The visible
  fire is therefore up to ~11 units wider than where it hurts near the
  end of the reach, and slightly narrower close to the gun.
- The points start 40 units ahead of the eye, not at the muzzle; the
  sprites start at the muzzle (CP 0) and are pulled forward onto them.
  Damage never happens between the muzzle and 40 − 12 = 28 units ahead
  of the eye.
- The density value given to each CP (1 → 0.01) is not read by the
  stock system's operators (unused).
- The sprites are world particles: in first person they are seen with the
  world FOV (particles.md §6). CP 0 is the third-person model's muzzle
  even for the local Pyro (Open questions 2).

## Per-frame order

Server tick: weapon attack (maybe a point added) → points updated
(§1) → touches resolved (§2). Client frame: new points created from the
networked spawn ticks (and advanced if late) → points updated with the
frame's dt → CP 0 and every point's CP set (§3.2) → particles simulated
and drawn.

## Edge cases

- Firing with the eye under water: no points, the bubble system only.
- Pressed against a wall (eye-to-muzzle line blocked by something that
  cannot be damaged): no points (weapons_more.md §7.1), but the burst's
  particle system still starts at the muzzle and emits sprites whose
  active CP is stale or absent (Open questions 3).
- A tap shorter than 0.2 s still emits for 0.2 s (≈ 7 points), so it is
  always visible.
- 30 points in flight block new points: at 2 ticks per point and a
  0.5–0.7 s life about 17–23 are alive, so the cap is not reached at
  66 ticks/s unless lives are extended.

## Test cases

| Setup | Expected |
|---|---|
| Stationary Pyro, eye at origin, aim +x, no spread (u = 0), dt 0.015 | point at (40, −5, 0) at spawn (right is −y when facing +x); after 1 update at (76.75, −5, 0.75); speed then 2450 × (1 − 0.1275) = 2137.6 |
| same, 0.5 s (≈ 33 updates) | ≈ 285 forward of the start, ≈ 25 up; hit box half-size 12 |
| point slides on a wall with normal (−1, 0, 0) | its stored Pyro velocity becomes 0; next velocity has no x component and z grows by 50 per update before drag |
| core sprite 0 s after birth, 0 units from the muzzle | radius 23 × 0.5 × 0.4 = 4.6, tint (0.2, 0.2, 1) |
| core sprite at life fraction 0.5, 120 units from the muzzle | radius 23 × 1 × 0.7 = 16.1; tint 0.2 + 0.8 × 0.6 = 0.68 in r and g |
| core sprite at life fraction 0.2 | alpha = initial (fade-out starts at 0.25 of life) |
| burst turns critical, RED | system `new_flame_crit_red` replaces `new_flame` (old one finishes its sprites) |
| client with 50 ms more latency | the same points, appearing 50 ms later, at the same places |

## Open questions

1. **Movement Follow CP** (catch-up speed 100): whether it is an
   exponential pull (fraction 100 × dt per step, i.e. effectively instant)
   or a speed in units/s (100 units/s would leave sprites far behind
   2450-unit/s points, so the first reading is far more likely). A
   frame capture of the first 0.05 s of a burst would settle it. Until
   then: place each core sprite at its point.
2. The weapon also starts the same system name attached to its effect
   model's `muzzle` (the view model in first person) when a burst starts,
   with no point CPs. What that second instance draws (its sprites have
   no target point: they may sit at the muzzle) is unconfirmed; a
   first-person capture would show a small fixed glow at the nozzle if
   so.
3. The sprites' behaviour when their assigned CP is never set or its
   point has died (CP left at its last position, or the sprite dies):
   unconfirmed.
4. The size-fade/maxsize of the flame material (0.3/0.5/0.4 × l) on the
   flame core when the Pyro looks at a wall at point-blank: sprites over
   0.5 × l vanish; check against the real game.
