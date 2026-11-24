# Day of Defeat: Source: the spread pattern

Source basis: measurements of the dedicated server (build 11003710) with
puppet bots through `tools/dods_probe` (2026-11-24; `measure.py pattern`):
the shot message the server sends for every shot (origin, aim, seed,
spread), and every bullet's damage position and force on a victim placed
in front (the force points along the bullet's direction), with the user
command's random seed forced per shot. The uniform generator is the one
specs/cs_source/weapons.md 4.2.1 describes; no game code was read.
Status: draft (measured)

## Summary

Every bullet leaves along the aim bent by an offset drawn from the user
command's random seed: a triangular draw right and one up, scaled by the
shot's spread (weapons.md, "Spread"), drawn again while the pair falls
outside the unit circle. The pattern is a pure function of `seed & 255`
and the spread, the same for every gun, so a client that knows its
command's seed predicts every bullet exactly.

## Units and conventions

Source units, z up, angles in degrees (pitch positive down), tick
0.015 s. Offsets are tangent units: a bullet with offset (x, y) leaves
along `normalise(f + x·r + y·u)`, with f, r, u the forward, right and up
vectors of the eye angles (positive x right, positive y up). The spread
`s` is the shot's number from weapons.md, "Spread", in the same units.

## Constants

| Name (ours) | Value | Unit | Meaning |
|---|---|---|---|
| seed_mask | 255 | – | a shot uses its command's random seed & 255 |
| seed_offset | 0 | – | the generator is seeded with `seed & 255` itself (CS:S adds 1) |
| draws_per_try | 4 | – | x's two draws, then y's two |
| bullets | 1 | – | one bullet a shot (weapons.md, open question 2) |

## Behavior

### The seed

- Each user command carries a random seed `S` (the engine's, as
  specs/cs_source/weapons.md 4.2). A shot fired while running a command
  uses `seed = S & 255`, and the shot message carries that value
  (measured: a command seed of 12345 gives the same bullet as 57, and its
  message says 57).
- **Per command**: the same command seed gives the same bullet every
  time, shot after shot (seven Thompson shots with the seed forced to
  12345 left along one direction, to 10⁻⁶); nothing else (time, shots
  fired, the shooter's state) enters.
- Seeds 0 and 1 give the same bullet (the generator takes a seed of 0
  as 1): the generator is seeded with `seed` itself, not `seed + 1`.
- No server-only seed: forcing the command's seed reproduces the
  server's bullets, so client prediction can match them.
- A bot's commands carry seeds that look random (their command number
  is 0). A human client's seed comes from its command number as CS:S's
  (4.2 there); not measured for DoD:S (its client can't run here).

### The draw

With the generator of specs/cs_source/weapons.md 4.2.1 (`U(lo, hi)` a
float from it), for each shot:

1. Seed the generator with `seed` (0-255).
2. Draw `x = U(−0.5, 0.5) + U(−0.5, 0.5)`, then
   `y = U(−0.5, 0.5) + U(−0.5, 0.5)`.
3. If `x² + y² > 1`, go back to step 2 (the same stream, no reseed)
   until the pair lies in the unit circle.
4. The bullet's offset is `(x·s, y·s)`: it leaves along
   `normalise(f + x·s·r + y·s·u)`.

Measured on all 256 seeds (Thompson, `s` 0.055, standing still): every
bullet within 1.2·10⁻⁶ tangent units of this draw; 4 of the 256 (seeds
8, 167, 206, 219) needed a second pair. Checked on other guns and
spreads (Garand 0.014, C96 0.065, BAR 0.025, MG42 0.2; a seed gives the
same offset in units of `s` on every gun): all within 10⁻⁶.

So each axis is triangular on [−s, s] before the circle cut; about 2.5 %
of first pairs fall outside the circle (the square's corners) and are
drawn again: the bullets fill a disc of radius `s`, densest at the
middle.

### The aim

The offset bends the eye angles of the command (the shot message's
angles are exactly the command's view angles). The server's punch angle
stays 0 when firing (weapons.md), so no recoil enters the server's
bullet; what the client's view kick does to the aim is unmeasured.

### What changes with state

Only `s` (weapons.md, "Spread": the alt mode, and the move penalty).
The shape and the seed's draws are the same standing, moving, ducked,
prone, sighted, deployed. One refinement of the move threshold seen
here: at a steady 45.0 u/s the Thompson read 0.155 (the penalty), at
44.9 0.055; ducked 44.67 no penalty, 45.33 the penalty: the penalty
applies from 45 u/s itself (to the 0.01 u/s the speed was read to).

## Per-tick order

1. The command's movement runs (the move threshold reads its result).
2. The gun fires if it may (weapons.md, "When a gun can't fire").
3. `s` for the shot (weapons.md, "Spread").
4. The generator is seeded with `S & 255`; one offset is drawn (with the
   circle's redraws); one bullet is traced along it.

## Edge cases

- A pair exactly on the circle (`x² + y² = 1`): not seen; kept or drawn
  again is unknown (immaterial).
- A pair can need more than one redraw; the stream just goes on.
- `s` = 0 (sights, scopes): the bullet goes along the aim whatever the
  seed.

## Quirks

- Seeds 0 and 1 make the same bullet: 255 patterns, not 256. Keep it.
- The circle cut keeps every bullet inside the spread's disc, unlike
  CS:S's two random-angle discs (specs/cs_source/weapons.md 4.2.2).

## Test cases

Offsets `(x, y)` in units of the spread (multiply by `s`), measured with
`s` 0.055 (Thompson standing still), to ±1·10⁻⁴.

| Setup | Input | After | Expected |
|---|---|---|---|
| command seed 0 | one shot | – | (−0.4920, 0.2861) |
| command seed 1 | one shot | – | same as seed 0 |
| command seed 2 | one shot | – | (−0.2553, 0.1285) |
| command seed 3 | one shot | – | (0.4562, 0.3560) |
| command seed 7 | one shot | – | (−0.4487, 0.1116) |
| command seed 8 | one shot | – | first pair (−0.8807, 0.9046) outside the circle; the bullet is the second pair (0.1701, −0.3028) |
| command seed 100 | one shot | – | (0.8393, 0.0860) |
| command seed 167 | one shot | – | drawn again: (−0.5936, −0.2505) |
| command seed 206 | one shot | – | drawn again: (−0.0635, 0.1127) |
| command seed 219 | one shot | – | drawn again: (−0.2297, −0.6196) |
| command seed 255 | one shot | – | (0.6639, −0.5735) |
| command seed 12345 | one shot | – | as seed 57: (0.6905, −0.2992) |
| the same seed on every command | full-auto burst | 7 shots | every bullet along one direction |
| Thompson aimed at pitch 12.68°, yaw 315, seed 0, still | one shot | – | direction ∝ f − 0.02706·r + 0.01574·u |
| MG42 undeployed (`s` 0.2), seed 8 | one shot | – | offset (0.0340, −0.0606) |
| Garand sighted (`s` 0), any seed | one shot | – | along the aim |
| Thompson at a steady 45.0 u/s | fire | – | spread 0.155 |

## Open questions

1. A human client's seed: from its command number as in CS:S (MD5),
   assumed; needs the retail client.
2. Whether the view kick (client-side) bends where bullets go.
