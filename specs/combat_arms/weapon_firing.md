# Combat Arms: weapon firing

Source basis: the client's weapon logic (trigger, animation-driven fire cycle, spread, client-side hit test), the client's view recoil, the server's vector/damage and armour logic, the shared weapon attribute manager, and the shipped attribute data (weapon/ammo table, model/skeleton table, surface table) plus the M4A1 view-model animation file, all from a reference client build. Numbers for the worked example are the reference build's data; the Steam build's data may differ (see Open questions).
Status: draft

Sections: Summary; Units and conventions; Constants; Worked example (M4A1); Behavior (trigger and fire modes, ammo, reload, hitscan, penetration, damage, hit zones, armour, spread, recoil); Per-tick order; Edge cases; Quirks; Corrections (2026-10-19); Test cases; Follow-up (2026-10-31b) (shoot-through surfaces); Open questions.

---

## Summary

- A Combat Arms gun fires on **animation time**. Holding the trigger loops the view model's fire animation, and each loop releases one round. The cycle time is the length of that animation divided by a rate, so it is not a number in the weapon table. Semi-auto, burst and auto are gates in front of that loop.
- Each round is an **instant ray** from the camera. The shooter's client decides what it hit and which body sphere, and the server applies the damage: base damage × a random ±5–10 % roll × a stepped range multiplier × a body-part multiplier, after armour has taken its share.
- Accuracy is one number, the **spread fraction** (0..1). Two meters feed it: one for movement and one for firing. They rise and fall on a fixed 80 Hz sub-step. The spread fraction maps linearly onto a per-weapon min..max cone measured in thousandths. Recoil is a separate **view kick** that really moves the camera up and side to side, then pulls it back.

## Units and conventions

- Distance: LithTech world units, the same unit as the movement spec. Up axis +Y; the frame is left-handed, with +Z forward and +X right.
- Angles: the code works in radians. The recoil fields in the data are in **degrees**, except the two "random" thresholds (`MaxRandomPitch`, `MaxRandomwag`), which are compared directly against radian values. Positive yaw turns right; positive pitch looks down, so "kick up" lowers pitch.
- Spread: the `*Perturb` integers in the weapon table are **thousandths of a unit offset** per unit of forward distance. 32 means the ray may be pushed up to 0.032 along each of the right and up axes, about 1.83°.
- Time: delays in the data are in milliseconds, measured against the system millisecond clock. Spread runs on fixed sub-steps of 1/80 s. Recoil recovery uses the real frame time in seconds. Animation lengths are in milliseconds.
- "Total ammo" is everything the player carries for that weapon, **including** what is in the magazine. Reserve = total − clip.
- Data fields are named as they appear in the attribute data.

## Constants

Fixed in code. Values that come from the attribute data are listed under the worked example.

| Name (ours) | Value | Unit | Meaning |
|---|---|---|---|
| spread_substep | 1/80 = 0.0125 | s | Spread meters advance in steps of this size |
| spread_substep_max | 100 | steps/frame | Cap on sub-steps run in one frame |
| spread_unit | 1/1000 | — | Scale from table perturb integers to offset |
| fast_turn_rate | 2.3 | — | Divisor term in turn-induced spread (player console/profile value) |
| fire_inc_scale | 3.0 | — | Global scale on firing-spread growth (from the table's global section, see quirk Q1) |
| fire_dec_scale | 3.0 | — | Global scale on firing-spread recovery (Q1) |
| move_inc_scale | 3.0 | — | Global scale on movement-spread growth (Q1) |
| move_dec_scale | 9.0 | — | Global scale on movement-spread recovery (Q1) |
| turn_effect | 3.0 | — | Global scale on turn-induced spread |
| recoil_bookkeeping | ½ | — | The recoil accumulator stores half of the applied view offset |
| wag_flip_chance | 50 | % | Chance the side-to-side kick changes direction once over threshold |
| behind_bonus | 1.30 | × | Damage multiplier when hit from behind (client-reported hits only) |
| behind_sectors | 12 | — | Direction sectors; sectors 7..11 count as "behind" |
| death_threshold | 1.0 | HP | A character dies when HP ≤ this after positive damage |
| anim_rate_cap | 2.0 | × | Any animation rate above this is replaced by 1.0 (Q6) |
| hitbox_widen | 2 | × | Horizontal (X, Z) widening of a character box in the client's first ray test |
| server_trace_steps | 4 | — | Max ray intersections the server's own walk makes (every surface, object or skipped character box counts one); corrected from 3, see Corrections 4 |
| server_hitbox_step | 5 | units | A server-side character box whose spheres the line misses is passed by restarting this far beyond its entry point (Corrections 5) |
| ai_damage_factor | 0.8 | × | Server console variable `AIDamageFactor`: any damage dealt by an AI (bots included) is scaled by it (Corrections 7) |
| ai_damage_cap | 50 (990 at ≥ 1000) | HP | Final non-melee damage an AI deals to a player in one hit (Corrections 7) |
| server_impact_slack | 2 | × dims | The server accepts a client-reported object hit only inside ±2× the object's half-dims |
| damage_roll_default | 0.1 | — | `DamageRFacter` when the field is missing; 0.2 if the value is outside [0,1] |
| fire_delay_default | 30 | ms | `fireDelay` when missing (server minimum shot spacing) |
| rapid_fire_default | 1 | shots | `RapidFireCount` when missing, so a weapon without it is semi-auto |
| burst_rounds_default | 3 | shots | `TripleShotRoundNum` when missing |

Global spread scales come from the weapon table's `[Perturb]` section (`MovementPerturbIncreaseSpeed` 3.0, `MovementPerturbDecreaseSpeed` 3.0, `FiringPerturbIncreaseSpeed` 3.0, `FiringPerturbDecreaseSpeed` 9.0, `PerturbRotationEffect` 3.0). The code built in defaults of 3.0 / 0.7 / 4.0 / 1.0 / 2.0 for use when the section is missing. The table above already shows which scale lands where after the swap described in Q1.

## Worked example: M4A1

The M4A1 is a starter assault rifle: full-auto only, 30-round magazine.

### From the weapon record (`Name="M4A1"`)

| Field | Value | Used for |
|---|---|---|
| `ShotsPerClip` | 30 | Magazine size |
| `RapidFireCount` | 0 | 0 = no per-press shot limit, so full auto |
| `TripleShotMode` | 0 | Burst not available (burst fields below are present but unused) |
| `fireDelay` | 0 | Server minimum shot spacing, ms (0 = none) |
| `FireAniRate` | absent → 0 | Added to the fire animation rate |
| `VectorsPerRound` | 1 | Rays per shot (shotguns use more) |
| `Range` | 3500 | Ray length, units |
| `Effectrange0/1/2` | 2500 / 3000 / 3500 | Damage band edges, units |
| `DamageRFacter` | 0.05 | Damage random roll ±5 % |
| `MinPerturb` / `MaxPerturb` | 3 / 100 | Spread cone min/max, thousandths |
| `DuckPerturb` / `MoveDuckPerturb` | 2 / 3 | Crosshair-only crouch minimum (see Q3) |
| `PerturbIncSpeed` / `PerturbDecSpeed` | 1.3 / 0.3 | Per-weapon spread growth/recovery rates |
| `ZoomedMinPerturb` / `ZoomedMaxPerturb` | 2 / 30 | Cone while zoomed (if the weapon can zoom) |
| `ZoomedPerturbIncSpeed` / `ZoomedPerturbDecSpeed` | 1.3 / 0.3 | Rates while zoomed |
| `DynamicPerturbFactor` | 1 | Multiplies the cone |
| `BaseCamRecoilPitch` | 1.3 | Vertical kick per shot, degrees |
| `BaseCamRecoilAccelPitch` | 0.2 | Extra kick per radian already accumulated |
| `MaxCamRecoilPitch` | 5 | Cap on the vertical accumulator, degrees (view offset cap is 2× this, Q4) |
| `MaxRandomPitch` | 0.02 | Accumulator (radians) above which side kick starts |
| `BaseCamRecoilWagR` / `BaseCamRecoilWagL` | −1.2 / 1.2 | Side kick, degrees (R turns right, L turns left) |
| `MaxCamRecoilWag` | 15 | Side kick scale-down reference, degrees |
| `MaxRandomwag` | 0.01 | Side accumulator (radians) above which direction may flip |
| `BaseCamBackJitter` / `BackJitterRecover` | 0.0 / 1.0 | Backward camera push (none on this gun) |
| `Recoil` | <0,0,0> | Random view-model position jiggle per shot (none) |
| `TripleShotDelay` / `TripleShotSelectDelay` | 190 / 300 | Burst lockouts, ms (unused here) |
| `MovePenalty` | 0.1 | Carry speed penalty (movement spec) |
| `PerturbPathFactor` | 50 | Read by nothing |

### From the ammo record (`Name="5.56mm M4A1"`)

| Field | Value | Used for |
|---|---|---|
| `Type` | 0 | Instant ray |
| `InstDamage` | 35 | Base damage per ray |
| `InstDamageType` | 2 | Bullet |
| `Rangedamage0/1/2` | 1.0 / 0.7 / 0.35 | Multipliers for the three range bands |
| `SelectionAmount` / `SpawnedAmount` / `MaxAmount` | 120 / 120 / 120 | Total ammo at spawn and its cap (30 in the clip + 90 reserve) |
| `FirePerturb` | 0.30 | Firing-spread cap, standing still |
| `FireMovePerturb` | 0.38 | Firing-spread cap, moving |
| `FireDuckPerturb` | 0.30 | Firing-spread cap, crouched still |
| `FireMoveDuckPerturb` | 0.38 | Firing-spread cap, crouched moving |
| `WalkPerturb` / `RunPerturb` / `JumpPerturb` | 0.04 / 0.25 / 0.70 | Movement-spread targets |
| `DecDelay` | 95 | ms after a shot before firing spread may recover |
| `JitterDecDelay` | 95 | ms after a kick before recoil uses the faster return rate |
| `JitterWaitRecover` / `JitterRecover` | 0.1 / 0.2 | Recoil return rates, radians per second (before the correction factor) |
| `PerturbRecoil` | 1.0 | Cap of an internal value nothing reads (Q7) |
| `DamageMovePenalty` / `DamageMovePenaltyDuration` | 40 / 0.3 | Victim slow on hit (movement spec) |

### From the view-model animation file

| Animation | Length | Note |
|---|---|---|
| `fire_0` | 105 ms | Fire key at 0 ms. One round per play, about 571 rounds/min at rate 1 |
| `prefire` / `postfire` / `dryfire` | 0 ms | Present but empty |
| `reload` | 2300 ms | Clip is filled when it ends; sound cues at 403, 834, 1725 ms |
| `select` | 1200 ms | Draw time (inventory spec) |
| `PreRush` / `PostRush` | 233 / 367 ms | Sprint in/out (inventory/movement specs) |

The M16A3, which can burst, has the same animation lengths. It is used below for the burst example: `TripleShotMode=1`, `TripleShotDelay=210`, `TripleShotSelectDelay=300`, `TripleShotMinPerturb=3`, `TripleShotMaxPerturb=60`, `TripleShotPerturbIncSpeed=1.4`, `TripleShotPerturbDecSpeed=0.2`, `TripleShotBaseCamRecoilPitch=1.6`, `TripleShotMaxCamRecoilPitch=8`.

### From the model table (characters)

Every player model has `FigureType` 0 (male) or 1 (female). Both figure types have every figure modifier (`VariableDamage`, `FirePerturb`, `WalkPerturb`, …) set to 0.0, so they drop out of every formula below. Their HP is 100; models carry `MaxArmor=100` and a `GearID` that selects armour (see Armour).

## Behavior

### Trigger and fire cycle

Inputs: fire button held, clip count, total ammo, burst mode flag, burst counter, rounds fired since press, burst lockout timers.

- The weapon **may fire** this frame only if all of these hold: the fire button is held, the clip is > 0, the player is alive, not spectating, the game is in the playing state, and no weapon/ammo chooser is open. Otherwise the trigger counts as released.
- On release (any frame without fire), the per-press round counter and the "burst started" flag are cleared.
- **Gates**, applied in this order to decide whether the fire request reaches the animation state machine:
  1. **Per-press limit**: if `RapidFireCount` > 0 and the rounds fired since the press ≥ `RapidFireCount`, block. 1 means semi-auto; 0 means no limit.
  2. **Burst continuation**: in burst mode, if 0 < burst counter < burst rounds, force the request on. A burst finishes even if the trigger is let go.
  3. **Burst hold block**: in burst mode, if the counter is 0 and a burst has started since the press, block. Holding the trigger after a burst does nothing until release.
  4. **Continuous-fire override**: weapons with the continuous-fire option, when switched to it, force the request on while the button is held.
  5. **Burst lockout**: block while less than `TripleShotDelay` ms have passed since a burst completed, or less than `TripleShotSelectDelay` ms since the fire mode was toggled.
- **Animation state machine** (view model):
  - Idle + fire request: the update that sees it only sets the state to firing. The **next** update plays `prefire` if the model has one, **even a 0 ms one** (only weapons whose shop category is primary shotgun skip it), else `fire_0` from its start. A finished `prefire` is followed by `fire_0` on the update that sees it done. So the first round of a press is spent on the third weapon update after the one that saw the press (second without a pre-fire); see Corrections 10. The same holds when a draw, a magazine reload or the post-sprint pose ends with fire held.
  - Firing: when the fire animation finishes (it is one-shot, not looping) and the request is still on with ammo left, restart `fire_0` from 0. When the request is off, play `postfire` if present, else go idle.
  - The `fire_0` animation carries a fire marker at 0 ms. When the animation passes the marker while the weapon is firing, a shot is raised. On the next update the weapon spends one round (clip −1 and total −1) and fires. One round per loop.
  - Cycle time:

```math
T_{cycle} = \frac{L_{fire}}{r_{fire}}, \qquad r_{fire} = 1 + \texttt{FireAniRate} + \sum \text{mod fire-rate bonuses}
```

  M4A1: $T_{cycle} = 105/1 = 105$ ms. A rate above 2.0 is replaced by 1.0 (Q6).
  - The restart happens on the first frame after the animation reports done, and leftover time is not carried over. The real interval is therefore $T_{cycle}$ rounded **up** to whole frames: 7 frames = 116.7 ms at 60 fps, 11 frames = 110 ms at 100 fps (Q2).
- **Burst counting**: every fired shot in burst mode raises the burst counter. When it reaches the burst rounds (3), the counter resets to 0 and the burst lockout timer starts. If the counter is ever greater than the clip, it resets to 0. Starting a reload or a sprint also resets it.
- **Toggling burst** (fire-mode key) is only possible on weapons with `TripleShotMode=1`. Each toggle starts the select lockout. Weapons that also have continuous fire cycle through auto → burst → continuous.
- **Server rate limit**: the server drops a shot that arrives sooner than `fireDelay` ms after the previous one, but **still spends the round**. The times compared are the **client's** fire timestamps carried in the shot message (whole ms), not arrival times. "The previous one" is the last shot that passed this check; a dropped shot does not restart the clock. A shot that passes the check but is then refused for another reason (no ammo) does restart it. The check is strict: exactly `fireDelay` ms after is allowed. **Corrected:** only the **last** ray of a shot carries the check, and every ray that is not checked still restarts the clock with the shot's own timestamp. So on a shot of two or more rays the last ray always sees 0 ms since "the previous shot" and, whenever `fireDelay` > 0, it is dropped (the round is still spent once); the other rays are never rate-limited (Corrections 3).

Writes: the clip, total ammo, the per-press counter, the burst counter and the lockout timers.

### Ammo

- Spawn: total = `SelectionAmount` (120), clip = min(total, `ShotsPerClip`) = 30.
- Each shot: clip −1 and total −1 (unless `InfiniteAmmo`). The server keeps its own copy and does the same.
- **Empty clip with reserve**: the weapon reloads by itself once it leaves the firing state. Because the fire request is forced off at clip 0, the last fire animation runs out, the weapon goes idle, and the reload starts that frame, even if the trigger is still held.
- **Total = 0**: holding fire plays the dry-fire animation (empty on the M4A1, so nothing visible). The gun stays in hand. Only `HideWhenEmpty` weapons (throwables) switch away. When total ammo reaches 0, the server plays an out-of-ammo voice line to the team.

### Reload

- **Allowed** unless: already reloading; clip == `ShotsPerClip`; or clip == total (no reserve). A reload request while zoomed first zooms out and starts the reload when the zoom-out finishes.
- **Amount**: the clip target is min(total, `ShotsPerClip`). Because total includes the clip, a partial reload **never loses rounds**: reloading at 12/30 with 70 total gives 30/30 with 70 total.
- **Timing**: `reload` plays once at

```math
r_{reload} = 1 + \sum \text{mod reload bonuses} + \text{character model reload bonus} + \text{set-item bonus}
```

  2300 ms at rate 1 (rate > 2.0 becomes 1.0, Q6). If a fire animation is playing, the reload waits until it finishes. The clip value changes **only when the reload animation completes**, with frame rounding like the fire cycle.
- **Interruptions**:
  - Firing during a magazine reload is ignored. The reload runs to the end, and if fire is still held, firing starts on the next update.
  - Sprinting does not cancel it: the reload finishes, then the sprint pose plays.
  - Switching weapons abandons it. The clip keeps its old count and nothing is gained; on the next draw, an empty clip triggers the automatic reload again.
  - Per-round reloaders (`ReloadPerBullet=1`, shotguns) add one round per loop of the reload animation (with a pre- and post-reload animation around the loops), and firing **does** cancel them after the current round.

### Hitscan

- **Origin**: the camera position in first person; the weapon model position in third-person or external cameras. **Direction**: camera forward, then deflected by spread (below). One ray per entry in `VectorsPerRound` (1 here), each with its own spread roll.
- **Length**:

```math
L_{ray} = \texttt{Range}\cdot\left(1 + \sum \text{mod range bonuses}\right)
```

  3500 for the M4A1. Past the end nothing is hit (sky impact).
- **Client test** (the shooter's machine decides player hits):
  - The ray ignores the shooter and invisible surfaces.
  - Each character has an axis-aligned hit box. For the first test the box is widened 2× on X and Z (height unchanged). The box counts as hit only if the ray segment, over its whole length, passes within the radius of at least one of the skeleton's **hit spheres** (table under Hit zones).
  - If the first hit is a character, a second test without widening decides what to report (corrected, Corrections 8): if it hits the same character, the reported point is the entry into the **widened** box from the first test; if it hits something else whose surface plane has the victim's origin behind it, that other thing is reported instead and no player hit is sent; otherwise the character is reported at the widened-box entry.
  - **Which sphere**: of all spheres the full segment passes through, the one with the smallest value of the expression below wins, where $p$ is the sphere centre relative to the ray start. This is roughly "the sphere entered first", but it favours big spheres.

```math
\lVert p \rVert^2 - r^2
```

  - Spheres with radius 0 never count.
- **Server**:
  - When the client reports a player hit, the server applies the damage below to that victim with the reported sphere, and does not re-test the geometry.
  - It then walks the ray itself from the fire position, skipping the first character box it meets, so collateral hits on objects or a second character are possible (see Edge cases).
  - When the client reports no player, the server walks the ray against walls, objects and characters for up to **4** intersections (corrected from 3; every surface, object or skipped box counts one).
  - A character found by the server's own walk is tested on the server's own, **unwidened** box. The sphere is chosen by the skeleton's node hit priority, not by distance (Corrections 5). The walk **ends** at the first character it damages: there is never a third victim.

### Penetration (server-side ray only)

- When the server's ray hits a surface whose type allows shooting through:
  - If its `MaxShootThroughThickness` is 0, the ray always passes.
  - Otherwise the server casts a ray back from `MaxShootThroughThickness + 1` units beyond the entry point to 1 unit before it. The ray passes when that back-cast hits **anything** (any surface or object but the shooter), so the effective limit is `MaxShootThroughThickness + 1` units, and the measured thickness itself is never compared (Corrections 9). For an object (not the level, not a world model), the object must also be thinner than the limit on at least one axis (2× half-dims < limit).
- On passing:
  - The remaining range is `Range` minus the straight distance from the fire position to the restart point, times `BulletRangeDampen`. The ray restarts from the exit point when the back-cast's surface has the same surface flags as the entry, else from the entry point.
  - Damage is multiplied by `BulletDamageDampen`. This is cumulative, and it also applies to whatever the ray hits next.
  - The direction is re-perturbed on the right and up axes of a look rotation along the ray with world up, each by a whole number of thousandths drawn uniformly from $\{-P, \dots, P\}$, $P$ = `MaxShootThroughPerturb` (integers, not a continuous range; Corrections 9).
- The ray stops if less than 1 unit of range is left. Melee never penetrates.
- Shoot-through surfaces in the surface table:

| Surface | Thickness limit | Re-perturb | Range × | Damage × |
|---|---|---|---|---|
| Wood | 10 | 10 | 1.0 | 0.5 |
| Wood door, plywood, chain-link fence, ice | 10 | 10 | 1.0 | 0.6 |
| Glass | 10 | 5 | 1.0 | 0.9 |
| Cloth (shoot-through) | 10 | 10 | 1.0 | 0.5 |
| Paper | 10 | 10 | 1.0 | 0.8 |
| Wood ladder | 10 | 10 | 1.0 | 0.7 |
| Water | 0 (always) | 30 | 1.0 | 0.8 |
| Flesh, armour | 55 | 15 | 1.0 | 0.3 |
| "Can shoot through", range target hole | 50 | 1 | 1.0 | 1.0 |

  All other surfaces (concrete, metal, …) stop the ray. The client-side test never penetrates anything, so shots through walls only land through the server path.

### Damage per ray

All on the server, in this order. $D$ is a running value.

1. **Base**: $D = \texttt{InstDamage}\cdot(1 + \sum \text{mod damage bonuses})$.
2. **Random roll**, uniform. M4A1: $D \in [33.25, 36.75]$, mean 35.

```math
D \leftarrow D \cdot U(1-k,\,1+k), \qquad k = \texttt{DamageRFacter}
```

3. **Client-reported character hit only**: $D \leftarrow D\cdot(1+\text{figure VariableDamage})\cdot b$, with $b = 1.3$ if hit from behind, else 1.
   - "Behind" is computed from the bullet direction $\hat d$ and the victim's forward $\hat f$ and right $\hat r$. These are the victim's **body** orientation on the server (the one its client sends: view yaw minus the upper-body twist of the Spread section), not its view (Corrections 6):

```math
\theta = \pi + \operatorname{atan2}(\hat f\cdot\hat d,\ \hat r\cdot\hat d), \qquad s = \left\lfloor \frac{\theta}{\pi/6} \right\rfloor,\qquad \text{behind} \iff 7 \le s \le 11
```

   - That is, the bullet travels with a forward component, at an angle between 30° and 180° counter-clockwise from the victim's right. See Q5 for the lopsided edge.
4. **Range band**, with $d$ the distance from the fire position to the reported impact point:

```math
m_{range} = \begin{cases} \texttt{Rangedamage0} = 1.0 & 0 \le d \le E_0 = 2500 \\ \texttt{Rangedamage1} = 0.7 & E_0 < d \le E_1 = 3000 \\ \texttt{Rangedamage2} = 0.35 & E_1 < d \le E_2 = 3500 \\ 1 & d > E_2 \ (\text{no change, Q8}) \end{cases}
```

   - The band edges are read as integers. $D \leftarrow D\cdot m_{range}$. This step is skipped (no multiplier) if the weapon or ammo has fewer than three entries.
   - The multiplier is a step function, not a curve. The M4A1 does 35 out to 2500 units, 24.5 to 3000, 12.25 to 3500.
5. **Penetration dampening** from earlier surfaces, if any (server path).
6. **Victim side**:
   - Buff and room hooks: mutation items, set items, game-mode rules, attachments that boost bullet damage. All are identity in a plain match.
   - **AI damager** (bots included): $D \leftarrow 0.8\,D$ (`AIDamageFactor`) here, before armour (Corrections 7).
   - Then armour and hit zone, as below. Then $D \leftarrow \max(D, 0)$.
   - **AI damager, player victim, damage type not melee**: $D \leftarrow 990$ if $D \ge 1000$, else $\min(D, 50)$ (Corrections 7).
   - $HP \leftarrow HP - D$.
   - The victim dies when $HP \le 1.0$ (and $D > 0$).

### Hit zones

Each node in a skeleton has a hit-sphere radius, a location code and a damage factor. The male skeleton (used by `FigureType` 0 models) has these hittable spheres (radius > 0):

| Body part | Location | Spheres (radius, units) | Factor |
|---|---|---|---|
| Head | head (1) | skull 1.0, jaw 3.0, upper face 4.6 | 6.0 |
| Neck | torso (2) | 3.0 | 1.0 |
| Chest / upper chest | torso (2) | 9.0, 10.0 | 1.0 |
| Groin | groin (5) | 3.0 | 1.0 |
| Hips (left / right) | leg (4) | 8.0 / 8.5 | 0.9 |
| Shoulders | arm (3) | 5.5 each, plus 7.0 each | 0.9 |
| Upper arms | arm (3) | 4.5 each, plus 4.0 each | 0.9 |
| Forearms | arm (3) | 4.0 each, plus 4.0 each | 0.9 |
| Hands | arm (3) | 3.2 each | 0.9 |
| Thighs | leg (4) | 8.0 each, plus 7.0 each | 0.9 |
| Shins | leg (4) | 6.0 each, plus 6.0 each | 0.75 |
| Feet | leg (4) | 5.0 each | 0.5 |
| Toes | leg (4) | 3.0 each | 0.5 |

- The female skeleton has the same body and limb spheres, but its head is different. The upper face sphere has radius 0 (unhittable) and there are two head spheres of radius 4.7 with factor 6.0. One head-located sphere of radius 3.0 has factor **1.0** (Q9).
- Sphere centres follow the animated bones, so crouching, leaning and running animations move them. The zone factor is applied at the armour step below, once.
- Explosions, fire (burn), poison, bleeding, flash, freeze, bump and similar area types, and any weapon whose `Guntype` is 7 (thrown), never use the zone factor (factor 1, armour checks torso). AI attackers and non-human victim models never use it either. **Melee does use it**: a knife hit on the head is ×6 like a bullet (corrected; see inventory.md 8.4). Infected players and the final bounty target ignore head and groin factors.

### Armour

- A player carries armour points AP (spawn: model `MaxArmor` = 100) and a list of gear pieces: body gear from the character model, then head, face and goggle gear and gear-granting attachments.
- Each gear piece has, per damage type, a protection fraction for head, torso, arm and leg, and a "consumable" flag.
- Which damage types gear protects against is data (Open question 15). In the stock data the body gears (default, light, heavy, tactical) list bullet (type 2) and burn (type 3, 0.5 on every location including the head); the head gears list bullet only; a gas mask lists poison (type 9, not consumable). Nothing in the stock gear lists melee (13) or explosion (7), so those are never reduced. Crush damage (falls) skips the gear step entirely.
- For a hit with damage type $t$ on location $\ell$ (unknown location counts as torso; for types that can't headshot, always torso), $P$ is the sum of each piece's first entry for $t$ at $\ell$. The **groin** (location 5) is not one of the four protected locations: a groin hit gets $P = 0$ from every piece, so it takes full damage and consumes no armour, even with AP > 0. (Infected victims have their groin counted as torso.) The blocked amount is

```math
B = D\cdot\operatorname{clamp}(P, 0, 1), \qquad c = \frac{\sum_{\text{consumable}} P_i}{\sum P_i}\ (c=0 \text{ if } P=0)
```

- With zone factor $z$:

```math
D' = \begin{cases} D\cdot z & AP = 0 \\ (D - B)\cdot z & AP > 0,\ AP - B > 0 \ (\text{or } c \le 0.001) \\ (D - AP)\cdot z & AP > 0,\ AP - B \le 0 \end{cases}
\qquad AP \leftarrow \max\!\left(0,\ AP - B\cdot c\right)\ \text{(only if } AP>0)
```

- **Zero AP means zero protection**, whatever gear is worn. Armour is subtracted **before** the zone factor, so a headshot multiplies the post-armour damage.
- Common gear (bullet entries): the default body gear protects head 0, torso/arm/leg 0.15. The light gear: 0, 0.05. The heavy gear: 0, 0.30. The default head gear: head 0.25. All are consumable.

### Spread

There are two meters, firing spread $F$ and movement spread $M$, both ≥ 0. Each frame they advance in fixed sub-steps of $\Delta = 1/80$ s (see Per-tick order for how many). Each sub-step:

**State values** (zoomed values replace these when the weapon is zoomed and the zoomed field is non-zero; burst values replace the per-weapon speeds and the cone in burst mode):
- Firing cap $C$ comes from the control flags: moving and crouched → `FireMoveDuckPerturb`; moving → `FireMovePerturb`; crouched → `FireDuckPerturb`; else `FirePerturb`. M4A1: 0.38 / 0.38 / 0.30 / 0.30.
- Movement target $S$:
  - Run flag off: `JumpPerturb` if airborne from a jump (not a jump pad), else `WalkPerturb`.
  - Run flag on: `RunPerturb` unless airborne from a jump, then `JumpPerturb`.
  - On a ladder, $S$ = `RunPerturb`.
  - "Airborne from a jump" is movement.md's "in a jump" state: from the jump tick until the landing is detected (the tick after ground contact). Walking off a ledge, falling after being pushed, or any other time off the ground without a jump does **not** count; those use the walk or run target (Open question 12).
  - The run flag is movement.md's control flag: set whenever crouch is not set and the walk key is not held (with always-run on), **including while standing still**. It has nothing to do with speed. Crouching clears it, so crouched (moving or not) the target is `WalkPerturb` (Open question 6).
- Movement fraction $\mu = \min(1, |v|/v_{max})$, with the full velocity (vertical included, except on jump pads) and $v_{max}$ = movement.md's speed limit $V$ for the current tick, which already includes the weapon's and gear's movement penalty, the crouch blend, the zoom (walk) limit, a confirmed sprint and the damage slow (Open question 6). The velocity is the one left by the frame's last movement tick, which ends above the limit (movement.md, Integration), so steady running or walking gives $\mu = 1$.

**Firing meter**, with $f$ = "a shot was fired this sub-step" (only the first sub-step of a frame can be a firing one):
- If $f$: $F \leftarrow \min(C,\ F + \Delta\cdot\text{fire\_inc\_scale}\cdot\texttt{PerturbIncSpeed})$ and stamp the time of the shot. M4A1: +0.04875 per shot.
- Else, if less than `DecDelay` ms have passed since the stamp, nothing changes. "Less than" is strict: the elapsed time is whole milliseconds of the OS clock, and at exactly `DecDelay` ms (95 for the M4A1) the meter already recovers (Open question 11). Once a recovery step has run, the stamp is cleared and later steps recover without waiting.
- Else, if $F > 0$: $F \leftarrow \max(0,\ F - \Delta\cdot\text{fire\_dec\_scale}\cdot\texttt{PerturbDecSpeed})$. M4A1: −0.01125 per sub-step, which is 0.9 per second.
- The cap $C$ is only applied when growing. Changing stance to a lower cap does not cut an existing $F$.

**Movement meter**:
- Turn term, from the change in view pitch and yaw (radians) since the previous sub-step:

```math
\rho = \min\!\left(1,\ \frac{\text{turn\_effect}\cdot(|\Delta\text{pitch}|+|\Delta\text{yaw}|)}{2 + \text{fast\_turn\_rate}\cdot\Delta}\right), \qquad A = \max(\rho,\ \mu S)
```

  Poison or stun forces $\mu S$ to 1. On a ladder $\mu S$ = $S$.
- What the turn term measures (Open question 13):
  - It reads the player's **body** orientation (the one sent to the server), not the camera. In this build the body pitch is only set when the server orients the player (spawn) and otherwise stays put, so pitch changes never count: neither mouse pitch nor the recoil's vertical kick and its return.
  - The body yaw is the view yaw minus the upper-body twist. With no movement key held, mouse yaw first goes into the twist, up to ±45° (half of `PlayerTwistYawMax` 90°); only turning beyond that moves the body yaw. With any of forward, back or strafe held the twist is 0 and the body yaw equals the view yaw. The recoil's side wag and its return change the view yaw directly (not the twist), so they always count.
  - The "previous" angles are updated on every sub-step. The angles change only between frames, so the first sub-step of a frame sees the whole frame's change and the other sub-steps of that frame see 0. The denominator uses the fixed $\Delta$: $2 + 2.3/80 = 2.02875$.
- If $A > M$: $M \leftarrow M + \min(\Delta\cdot\text{move\_inc\_scale},\ A-M)\cdot\texttt{PerturbIncSpeed}$, then $M \leftarrow \min(M, S, \mu S)$. M4A1: at most +0.04875 per sub-step.
- If $A < M$:
  - On the ground: $M \leftarrow M - \min(\Delta\cdot\text{move\_dec\_scale},\ M-A)\cdot\texttt{PerturbDecSpeed}$. M4A1: at most −0.03375, and below a gap of 0.1125 it falls by 30 % of the gap per step.
  - While airborne from a jump it instead **rises**, using the decrease step size with the increase speed: $M \leftarrow M + \min(\Delta\cdot\text{move\_dec\_scale},\ M-A)\cdot\texttt{PerturbIncSpeed}$. M4A1: at most $+0.1125 \times 1.3 = 0.14625$ per sub-step, and $1.3\times$ the gap below that (Open question 12).
  - Then $M \leftarrow \min(M, \texttt{JumpPerturb})$ and $M \ge 0$.

**Cone and ray**, at the moment of the shot:
- $p = \max(F, M)$, using the meters as they were **before** this frame's sub-steps. A shot never feels its own growth.
- Dynamic factor $g$ = `DynamicPerturbFactor` + mod bonuses (1). The per-stance overrides exist in code but are unreachable (Q10).
- Cone limits: $n_{min}$ = int(`MinPerturb`), $n_{max}$ = int(`MaxPerturb`). Zoomed: the zoomed values (plus mod bonuses). Burst mode: the burst values. Both are multiplied by $g$ (truncated to int), then $n_{min} \leftarrow \min(n_{min}, n_{max})$. If $n_{max} = 0$ the weapon has no spread and both meters are held at 0.

```math
n = n_{min} + \left\lfloor (n_{max}-n_{min})\cdot p \right\rfloor \cdot g
```

- Two independent uniform draws $u_1, u_2 \in [0,1]$:

```math
\delta_r = \frac{(2u_1 - 1)\,n}{1000}, \quad \delta_u = \frac{(2u_2 - 1)\,n}{1000}, \quad \hat d = \operatorname{normalize}(\hat f + \delta_r \hat r + \delta_u \hat u)
```

  The pattern is a uniform **square**, not a disc. The reference draws from a pre-generated table of uniform values with a running index; any uniform RNG is equivalent.
- M4A1, standing still, trigger held, no recovery between shots: shot k uses $p = \min(0.30, (k-1)\cdot 0.04875)$, so $n$ = 3, 7, 12, 17, 21, 26, 31, 32, 32, … The cap is first reached on shot 8 (meter capped after the 7th shot), at $n = 32$ (±1.83°).
- Running at full speed alone gives $p = 0.25$ → $n = 27$. Jumping gives 0.70 → $n = 70$.

### Recoil (view kick)

- Applied on the client in first person only, on the frame a shot fires, **after** the shot's ray was cast. The kick affects the next shot.
- State: vertical accumulator $J \ge 0$ (radians), side accumulator $J_y$ (signed radians), a side direction flag, the backward push and the time of the last kick. **All of it belongs to the player, not to the weapon** (Corrections 1): one set per player, never reset by a switch, a death or a respawn (only returned). The direction flag is drawn once when the player enters the map. Every kick and every return reads the data of the weapon **in hand at that moment**.
- Each accumulator holds **half** of the view offset it has caused (Q4).

**Kick**, per shot. $J_{max}$ = rad(`MaxCamRecoilPitch`) (×(1 + mod %)), $b$ = rad(`BaseCamRecoilPitch`) (×(1 + mod %)), $a$ = `BaseCamRecoilAccelPitch` (+ mods). Burst mode uses the burst pitch values when both are non-zero.

```math
J \leftarrow \operatorname{clamp}(J, 0, J_{max}),\quad k = b\left(1 - \frac{J}{J_{max}}\right),\quad k' = \begin{cases}k + aJ & k>0\\ k & \text{else}\end{cases},\quad h = \min\!\left(\frac{k'}{2},\ J_{max}-J\right)
```

```math
J \leftarrow J + h, \qquad \text{pitch} \leftarrow \text{pitch} - 2h \ \ (\text{view moves up by } 2h)
```

- M4A1 with no recovery between shots, view rise per shot (degrees): 1.300, 1.261, 1.223, 1.187, 1.151, 1.116, 1.083, 1.050, 0.629, then 0. The total climbs 1.30, 2.56, 3.78, 4.97, 6.12, 7.24, 8.32, 9.37, **10.00°** and stays there.
- The M16A3's numbers make $k'$ constant: every shot kicks exactly 1.6°.

**Side wag**: after the kick, if $|J|$ > `MaxRandomPitch` (radians, + mods):

```math
q = 1 - \frac{|J_y|}{\operatorname{rad}(\texttt{MaxCamRecoilWag})}, \qquad w = q\cdot\operatorname{rad}(\texttt{BaseCamRecoilWagR or L})
```

- The flag picks R or L. Then $J_y \leftarrow J_y + w/2$ and yaw $\leftarrow$ yaw − $w$. For the M4A1, R turns the view 1.2° right and L 1.2° left, scaled by $q$.
- If afterwards $|J_y|$ > `MaxRandomwag`, the flag flips with 50 % chance (the reference uses the millisecond clock's parity).
- M4A1: the wag starts on the second shot of a burst without recovery ($J$ = 0.0223 > 0.02).

**Back push**: `BaseCamBackJitter` is added to a backward camera offset per shot. It recovers at 5 × `BackJitterRecover` per second. Zero on the M4A1.

**Return**, every frame before firing is processed, with $\Delta t$ the frame time in seconds:
- Vertical, while $J > 0$:

```math
\text{rate} = \begin{cases} \texttt{JitterWaitRecover} & t - t_{kick} < \texttt{JitterDecDelay} \\ \texttt{JitterRecover} & \text{else}\end{cases}, \quad v = \min\!\left(J,\ \Delta t\cdot\text{rate}\cdot\left(1+\frac{J}{J_{max}}\right)\right)
```

```math
J \leftarrow J - v/2, \qquad \text{pitch} \leftarrow \text{pitch} + v
```

  Here $J_{max}$ is the cap of the weapon in hand, scaled by its **muzzle** attachment's `MaxCamRecoilPitchPer` only. In burst mode it is rad(`TripleShotMaxCamRecoilPitch`) whenever that field is non-zero, even when `TripleShotBaseCamRecoilPitch` is 0 (the kick needs both non-zero; the return needs only the cap). Corrections 2.
- Side, while $J_y \ne 0$: the same with $|J_y|$ and rad(`MaxCamRecoilWag`), and the sign of $J_y$.
- The return is applied to the real view angles, so it pulls the aim down even if the player has already corrected with the mouse. The cap $v \le J$ means the last step can move the view by up to $J$ itself, not the full remaining offset $2J$, so recovery tails off gently.

### Burst example (M16A3)

- Toggle burst, wait 300 ms, press fire. Three shots fire at the fire-cycle spacing (105 ms nominal), even if the button is released after the first. Then 210 ms of lockout counted from the third shot.
- With the button held, nothing more fires until it is released and pressed again.
- During burst the cone uses min 3 / max 60, the per-weapon speeds 1.4 / 0.2, and the burst pitch values.

## Per-tick order

One client frame, for the local player's current weapon:

1. **Recoil return** for vertical, side and back offsets, using the frame time.
2. **Fire request**: button held and clip > 0 and alive and playing (Trigger and fire cycle).
3. **Trigger edges**: on release, clear the per-press counter and the burst-started flag. A clip of 0 forces the request off. A burst counter above the clip resets to 0.
4. **Gates** 1–5 (Trigger and fire cycle).
5. **Animation state machine**: sprint / select / reload / fire / idle transitions. If a fire marker was raised since the last update, spend a round (clip −1, total −1) and mark "fired".
6. **If fired**: count the round for the per-press limit and burst; start the burst lockout if the burst completed. Compute the cone from the meters as they stand, cast the ray(s) and report the hit to the server. Play the shot sound.
7. **View-model jiggle** (`Recoil` vector, random per axis) for this frame only.
8. **Spread sub-steps**: $N = \lfloor \text{elapsed}/12.5\,\text{ms} \rfloor$ since the last batch, capped at 100. If $N = 0$ but a shot fired, $N = 1$. For $i = 1..N$: update the movement meter, then the firing meter, with $f$ true only for $i = 1$ of a firing frame. If $N > 0$, the batch clock resets to now, **discarding the remainder** (Q2).
9. **Auto-switch** for `HideWhenEmpty` weapons with total 0.
10. **Auto-reload**: if total > 0, clip ≤ 0, and not reloading or firing.
11. **View kick** (if fired, first person), then the side wag check.

Server, per received shot, for each ray in order:
1. Rate check (`fireDelay`), **last ray only**; every ray that passes or is not checked stamps the clock (Corrections 3).
2. Spend the round (on the last ray).
3. If the client reported a player: figure modifier, behind bonus, range band, armour and zone, apply damage. Then walk the ray from the fire position, skipping the first character box.
4. Else walk the ray: walls, penetration, objects, characters (at most 4 intersections; it ends at the first character it damages).

Within the client frame, steps 1 to 11 run after the frame's camera update: the shot's direction is the camera rotation set before step 1's return (Corrections 11).

## Edge cases

- **Band edges are inclusive at the top**: exactly 2500 units gives 1.0; 2500.01 gives 0.7.
- **Range vs bands**: with `Range` 3500 and the last edge 3500, an unmodified M4A1 never reaches the "beyond" case. A range attachment can, and then the damage is unscaled (Q8).
- **Clip = total**: no reload possible (nothing in reserve).
- **Clip 0, total > 0, trigger held**: the last fire animation completes, then the reload starts by itself. The held trigger then fires again after the reload ends.
- **Firing perturb at the cap while moving**: crouching and stopping while $F$ = 0.38 leaves $F$ at 0.38 until it recovers. The new cap 0.30 is only enforced on the next shot.
- **Fire held through weapon select**: the update that sees the select animation done sets the state to firing; the pre-fire (0 ms on the M4A1) plays on the next, `fire_0` starts on the one after, and the round is spent on the third update after the select ended (Corrections 10).
- **Sprinting while the fire animation plays**: the current fire animation completes, then the sprint pose plays and the burst counter resets. No firing during sprint.
- **AP exactly equal to blocked**: the third armour case, $(D - AP)\cdot z$, applies, which gives the same result as $(D-B)\cdot z$.
- **Server-side second victim**: after a client-reported hit, the server ray skips only the **first** character box it meets. If a second character is behind the first on the line, the server can damage it too (Open question 5). Its damage starts from the running value left by the first hit, i.e. base × roll × the first victim's figure modifier and behind bonus × the range multiplier, and the range band is then applied **again**, using the same distance as before (fire position to the client-reported impact point on the first victim), not the second victim's own distance. So inside the first band nothing changes, but at 2500–3000 units the second victim gets ×0.7 twice. The first victim's armour and zone factor are not carried over (they are applied at the victim side); the second victim's own armour and zone apply. Surfaces passed between the two victims dampen it as usual; the first victim's body does not. The second victim's sphere is picked by node hit priority on the server's model (Corrections 5), it gets no behind bonus and no figure modifier of its own, and the walk ends there: a third character in line is never hit (Corrections 4).
- **Server rejects a far-off object hit**: a reported object hit point outside ±2× the object's half-dims is ignored, and the server traces itself.
- **Zero-length prefire**: weapons whose `prefire` exists but is 0 ms spend one extra update before `fire_0` starts, so the first shot is a frame later (Q2). This comes on top of the update that only sets the firing state (Corrections 10).

## Quirks

- **Q1, swapped global scales**: the client loads the table's four global spread speeds into the opposite slots. The "firing" increase/decrease values drive the movement meter and the "movement" values drive the firing meter. Keep this: the Constants table already shows the effective values.
- **Q2, frame-rate dependence**:
  - The fire cycle and the reload end on whole frames.
  - The 80 Hz spread sub-step discards its remainder each frame. At 60 fps the meters advance once per frame (60 steps/s). At 100 fps, every other frame (50 steps/s). At 80 fps, 80 steps/s.
  - Recommendation: run the meters on an exact 80 Hz accumulator that keeps the remainder, and the fire cycle on exact animation time. Offer the frame-quantised behaviour as an option for side-by-side checks.
- **Q3, crouch doesn't tighten the real cone**: `DuckPerturb` / `MoveDuckPerturb` lower only the **crosshair's** drawn minimum (HUD spec). The ray uses `MinPerturb`. Crouching helps only through the firing cap (`FireDuckPerturb`), which equals the standing value on the M4A1. Keep.
- **Q4, half bookkeeping**: the recoil accumulator holds half the view offset, so the real cap on view climb is 2 × `MaxCamRecoilPitch` (10° on the M4A1). Keep.
- **Q5, lopsided "behind"**: the behind test covers bullet directions from 30° to 180° measured from the victim's right, not a symmetric rear cone. A bullet travelling forward but angled toward the victim's right (0–30°) does not get the bonus; one angled toward the victim's left does. Keep.
- **Q6, rate cap**: any fire or reload rate above 2.0 is replaced by 1.0, so stacking past double speed returns to normal speed. Keep.
- **Q7, dead values**: an internal "perturb recoil" value is grown, capped by `PerturbRecoil`, decayed, and multiplied by the movement state value every sub-step, but nothing reads it. `PerturbPathFactor` is never read. Omit both.
- **Q8, beyond the last band**: past `Effectrange2` the range step applies no multiplier, so damage jumps back to full.
- **Q9, female head sphere**: one head-located sphere on the female skeleton has factor 1.0. Because the smallest $\lVert p\rVert^2 - r^2$ wins, the larger 6.0 spheres usually win, but not always. Keep as data.
- **Q10, dead per-stance factor**: the per-stance overrides of `DynamicPerturbFactor` are read from keys spelled differently from anything in the data, so they always stay at 1. Omit.
- **Q11, turning barely matters**: the movement meter is clamped to $\mu S$ after rising, so the turn term cannot raise spread. It does change one thing: turning while the meter is above $\mu S$ takes the rising branch and **snaps** $M$ down to $\mu S$ instead of letting it decay. Keep.
- **Q12, airborne decay inverted**: when the movement target falls below $M$ while airborne from a jump, $M$ grows instead (capped at `JumpPerturb`). Keep.
- **Q13, wall-bang falloff distance**: for hits found only by the server ray (through walls), the impact point used for the range band is the one the client reported. If the client hit plain world geometry it reported none, and the distance is measured from the shooter to the **world origin**. Recommendation: measure the real distance, and record this in the implementation's notes (see Open questions).

## Corrections (2026-10-19)

A second pass over the reference's client weapon, player view, server shot handling, server ray and victim damage code, compared behaviour by behaviour with this spec and with our implementation (`games/combat_arms/src/game/weapons.rs` and its submodules). The body text above is already corrected; each entry says what was wrong. Test cases 50–69 cover them.

1. **The view kick belongs to the player, not to the weapon.**
   - Reference: one vertical accumulator $J$, one side accumulator $J_y$, one side-direction flag, one backward push and one "last kick" time per player. Kicks from every weapon add to the same values. Nothing resets them on a weapon switch, a death or a respawn; they only return. The direction flag is drawn once (50/50) when the player enters the map. The kick and the return always read the data of the weapon in hand at that moment: its caps, base kick, wag sizes and its ammo's return rates and delay.
   - Ours: `CaRecoil` is a part of each weapon (`weapons.rs:748-771`), with its own accumulators and its own direction flag picked at that weapon's first side kick (`weapons.rs:812`). `before_shots` returns every carried weapon's accumulator with that weapon's own data (`weapons.rs:1459-1475`). The spec said "random at spawn" and did not say whose state it is.
   - Why it matters: switching to a pistol right after a spray. In the game the pistol's first kick is cut by the rifle's leftover climb, $k = b\,(1 - J/J_{max})$ with the **pistol's** cap, and the climb keeps returning at the pistol's ammo rates. Ours gives the pistol a fresh full kick. M4A1 sprayed to its cap ($J$ = rad 5° = 0.0872665), then the M92FS (3.5°, cap 14°): the first pistol shot raises the view 2.250°, not 3.500°.

2. **The vertical return in burst mode uses the burst cap.**
   - Reference: the return's correction factor $1 + J/J_{max}$ uses rad(`TripleShotMaxCamRecoilPitch`) whenever burst mode is on and that field is non-zero. The base kick field does not matter here (the kick needs both non-zero, the return only the cap). The cap is scaled by the muzzle attachment's `MaxCamRecoilPitchPer` only.
   - Ours: `CaRecoil::recover` always takes the normal cap (`pitch_params(false, …)`, `weapons.rs:839`). The spec's Return paragraph did not mention burst.
   - Why it matters: burst weapons settle at a different speed. With a burst cap 8° against a normal 5°, $J$ = 0.0418879 rad (2.4°), a 16 ms frame at the waiting rate 0.1: the view comes down 0.002080 rad in the game, 0.002368 rad in ours (14 % faster).

3. **The server's spacing check is on a shot's last ray, and every earlier ray restarts its clock.**
   - Reference: the server handles a shot's rays one by one. A ray that is not the last is never checked, but it stores the shot's timestamp as "the last shot that passed". The last ray is checked: if less than `fireDelay` ms have passed since that stored time it is dropped, and the shot's round is spent through the drop path. On a shot of two or more rays the stored time is the shot's own, so the difference is 0. With `fireDelay` > 0 (30 ms when the record omits it) the **last ray of every multi-ray shot is dropped**, and no other ray is ever rate-limited. Single-ray weapons behave as the old text said. The round is spent once either way.
   - Ours: the spec said "only the first ray carries the check" (Trigger and fire cycle, "Server rate limit"). `after_shots` checks once per round, before any pellet, and keeps or drops the whole round (`weapons.rs:1795-1806`).
   - Why it matters: every shotgun whose record has `FireDelay` > 0 (or none) deals one pellet less per shot. A Remington 870 with `FireDelay` 30 does at most 7 × 9 = 63 at close range, not 72. With the pellet carry-over (`weapon_classes.md` 5.7) the dropped report is the last one.

4. **The server's walk makes up to 4 intersections and ends at the first character it damages.**
   - Reference: the walk loops until it hits nothing, and gives up after the 4th intersection; every surface, object or skipped character box counts one. A server-found character (any character after the skipped one) is damaged and the walk **returns** at once, even when its body surface could be shot through.
   - Ours: `SERVER_TRACE_STEPS` 3 including the client test (`weapons.rs:240`), and the walk goes on past every character it damages (`weapons.rs:2131-2137`). The spec said "up to 3 segments" and was silent on what follows a second victim.
   - Why it matters: three thin panels can be shot through in the game (the 4th intersection is the target); ours stops after the third panel. In a line of three players, the game hits at most two.

5. **A character found by the server's own walk gets its sphere by hit priority, on the unwidened box.**
   - Reference: the server meets the character's own hit box (not widened). From the entry point $P$ it takes the **infinite line** through $P$ along the ray. A node counts if its perpendicular distance from that line is at most its hit radius, whatever its position along the line. Of the counted nodes, the one with the highest `Node#Priority` of the skeleton data wins; on equal priority the one later in the node order wins. If none counts, the character is not hit: the walk goes on from $P + 5\hat d$ with that box filtered out. The damage on a server-found character gets no behind bonus and no figure modifier (those belong to client-reported hits). Its range band is applied again, cumulatively.
   - Ours: every segment uses the client rule, the 2×-widened box and the sphere with the smallest $\lVert p\rVert^2 - r^2$ (`weapons.rs:2041-2052`, `pick_sphere` at `weapons.rs:931-947`). The priorities are in no spec (`characters.md` 11 lists radius, location and factor only).
   - Why it matters: through-wall and second-victim hits land on whichever node the data ranks highest. For example, a wall-bang whose line grazes the head and passes the chest counts as the chest if the chest ranks higher, 6× less damage than ours. A grazing line that misses every sphere passes the player in the game but hits the widened box in ours.

6. **"From behind" is measured against the victim's body, not its view.**
   - Reference: the sector test uses the server object's rotation. That is the body yaw the client sends: the view yaw minus the upper-body twist (up to ±45° with no movement key held, Spread section).
   - Ours: the test uses the view yaw (`intent.yaw_rotation()` at `weapons.rs:1620` and `r.yaw` at `weapons.rs:1608`, giving `forward`/`right` at `weapons.rs:1627-1628`).
   - Why it matters: a standing player who has turned the mouse is protected or exposed by where the legs point. Body 45° to the right of the view, bullet at 150° from the view's right: ours gives ×1.3, the game ×1.0.

7. **Damage from an AI (bots included) is scaled by 0.8 and capped at 50 per hit on players.**
   - Reference, on the victim side: if the damager is an AI (any bot or AI character), first $D \leftarrow D\cdot$ `AIDamageFactor` (server console variable, built-in 0.8), before set items, gear and the zone factor. Then, after armour and the zone factor, if the victim is a player and the damage type is not melee: $D \ge 1000 \Rightarrow D = 990$; $50 < D < 1000 \Rightarrow D = 50$.
   - Ours: neither exists (`hurt`, `weapons.rs:2191-2288`); the spec listed "AI damage scales" among identity hooks.
   - Why it matters: bot matches. A bot headshot with an M4A1 does 50, not 210; a bot's sniper or shotgun cannot one-shot a player. A bot's knife is not capped (×0.8 only).

8. **The client reports the widened-box entry, and can drop a hit on a near miss.**
   - Reference: when the first (widened, sphere-checked) test hits a character, the plain-box test is run again. Same character hit: the reported point, and so the band distance, is the **widened** box's entry. Something else hit, and its surface plane has the victim's origin behind it: that other thing is reported, with no player hit. Otherwise: the character at the widened entry.
   - Ours: the reported point is the plain box's entry (`weapons.rs:2101-2103`) and the near-miss drop does not exist. The spec said "the victim is the same either way".
   - Why it matters: the band distance is shorter by up to one box half-width, which moves hits near a band edge into the better band. Against a box of half-width 16 whose front is 2510 units away the game does 35, ours 24.5.

9. **Penetration: what passes, where the ray restarts, and the integer re-perturb.**
   - Reference:
     - The back-cast from limit + 1 units beyond the entry accepts any surface or object it hits, so a panel up to limit + 1 units thick passes. The thickness found is never compared with the limit.
     - The restart point is the exit when its surface flags equal the entry's, else the entry.
     - The remaining range is `Range` − |restart − fire position|, times the range dampen.
     - The new direction adds $k_r/1000$ and $k_u/1000$ on the right and up axes of a look rotation along the ray with world up, with $k_r, k_u$ whole numbers drawn uniformly from $\{-P..P\}$.
   - Ours: the back-cast only accepts the same entity (`weapons.rs:2156-2158`), the exit must be within the limit (`weapons.rs:2164`), and the re-perturb is a continuous uniform draw on an arc-rotation basis (`weapons.rs:2180-2182`).
   - Why it matters: boards 10–11 units thick are shot through in the game (wood: 17.5 instead of nothing). Exit spread is quantised (glass: 11 possible offsets per axis).

10. **First-round latency of a press.**
    - Reference: the weapon update that first sees the fire request only switches the state to firing. The next update starts the pre-fire if the model has one (a 0 ms pre-fire counts; only primary shotguns skip it), else `fire_0`. A finished pre-fire starts `fire_0` on the update that sees it done. The fire key at 0 ms is consumed on the update after `fire_0` started. So the first round is spent on the **3rd** update after the press update (2nd without a pre-fire). The same chain runs when a draw, a magazine reload or the post-sprint pose ends with fire held: the update that sees it end sets the firing state. The loop restart of a held automatic adds nothing (the restart and its key are one update apart, as the cycle already counts).
    - Ours: Chimera's trigger fires on the tick of the press, and at the tick a draw or reload ends. The spec's state machine played the pre-fire on the press update and only for "non-empty" pre-fires.
    - Why it matters: 50 ms of extra input latency at 60 fps on every first shot (M4A1, pistols), 33 ms on shotguns. It also shifts every "ready" time after a draw or reload by those updates.

11. **Shot direction timing (answers Open question 10).**
    - Reference: the camera's rotation is set from the view angles in the frame's camera update. The weapon update (recoil return, firing, kick) runs afterwards, just before rendering. A shot therefore leaves along the angles as they were **before** this frame's return. The previous frame's kick and return are in it, and so are this frame's mouse and scope sway.
    - Ours: `before_shots` adds this tick's return to `ViewPunch` before the frame fires (`weapons.rs:1464-1474`), so the shot includes it.
    - Why it matters: under 0.1° per shot. It is listed so that the per-tick order is exact.

12. **State-value details of the spread meters.**
    - Reference:
      - With the run flag **on** and airborne from a jump, the movement target is the plain `JumpPerturb`, even zoomed with a non-zero `ZoomedJumpPerturb`. That zoomed value is used only with the run flag off.
      - The falling branch's clamp is always the plain `JumpPerturb`.
      - The "no spread" hold of both meters tests the weapon's plain `MaxPerturb`, not the zoomed or burst one.
      - In burst mode each burst field replaces its plain counterpart only when non-zero, field by field. The burst cone needs `TripleShotMinPerturb`/`TripleShotMaxPerturb`; the meter speeds `TripleShotPerturbIncSpeed`/`TripleShotPerturbDecSpeed`.
    - Ours: `CaSpread::states` uses the zoomed jump whenever zoomed (`weapons.rs:610`), the clamp is that same value (`weapons.rs:1978`), the hold tests the mode's cone (`weapons.rs:1934`), and the burst values replace all four at once (`weapons.rs:581`).
    - Why it matters: no stock ammo has `ZoomedJumpPerturb` and the install reader fills every burst field. These only bite on custom or partial data.

## Corrections (2026-10-30)

From the client weapon's fire-animation choice and its pre-fire lookup, the shared animation-rate calculation and the client's zoom command. Settles `view_models.md` contradictions C1 and C2 in favour of `view_models.md`.

1. **Zero-length prefire** (Constants/rules above): the extra update applies only to weapons that play a pre-fire at all: not pistols (secondary class), not knives (melee class), not primary shotguns. A pre-fire longer than 0 delays the first round by its whole length, on every weapon that plays one (`weapon_classes.md` Corrections (2026-10-30) 1, its cases 43 and 44).

## Test cases

M4A1 data as above unless stated. "Roll 1.0" means the damage random roll returns exactly 1. Victim: male skeleton, 100 HP, no armour unless stated. Distances are fire position to impact point.

1. **Range bands**. Torso hit, roll 1.0, at d = 100 / 2500 / 2500.5 / 3000 / 3000.5 / 3500 → damage 35 / 35 / 24.5 / 24.5 / 12.25 / 12.25.
2. **Beyond last band**. A range mod +10 % makes the ray 3850. Torso hit at d = 3600, roll 1.0 → 35.
3. **Random roll bounds**. Torso hit, d = 100, roll at minimum / maximum → 33.25 / 36.75.
4. **Zones**. d = 100, roll 1.0, unarmoured: head 210, neck 35, chest 35, groin 35, hip 31.5, upper arm 31.5, hand 31.5, thigh 31.5, shin 26.25, foot 17.5.
5. **Behind bonus**. Victim forward +Z, right +X. Bullet direction (0,0,1): sector 9 → torso 45.5. Direction (0,0,−1): sector 3 → 35. Direction (0.95, 0, 0.312) (about 18° from right toward forward): sector 6 → 35. Direction (−0.95, 0, 0.312): sector 11 → 45.5.
6. **Body armour**. AP 100, default body gear (torso 0.15). Torso hit, d = 100, roll 1.0 → D' = 29.75, AP → 94.75, HP → 70.25.
7. **Armour plus headshot**. AP 100, default body gear plus default head gear (head 0.25). Head hit, roll 1.0 → B = 8.75, D' = (35 − 8.75) × 6 = 157.5, AP → 91.25, victim dead.
8. **Body gear does not cover the head**. AP 100, default body gear only (head 0). Head hit, roll 1.0 → 210, AP stays 100.
9. **Armour runs out**. AP 3, default body gear, torso hit, roll 1.0 → D' = 35 − 3 = 32, AP → 0.
10. **Zero AP means no protection**. AP 0, heavy body gear (torso 0.30), torso hit, roll 1.0 → 35.
11. **Banded damage with armour**. AP 100, default body gear, torso hit at d = 2800, roll 1.0 → D = 24.5, B = 3.675, D' = 20.825, AP → 96.325.
12. **Death threshold**. Victim HP 36, torso hit 35 → HP 1 → dead. HP 36.01 → 1.01 → alive.
13. **Hits to kill**. Unarmoured torso, roll 1.0, d ≤ 2500 → 3 hits (HP 65, 30, −5). Armoured (AP 100, default body gear) → 4 hits (HP 70.25, 40.5, 10.75, then dead).
14. **Spread from rest**. Standing still, both meters 0, hold fire, ignore recovery between shots (all shots within `DecDelay`). Cone n for shots 1..9: 3, 7, 12, 17, 21, 26, 31, 32, 32. Meter after shot 7 = 0.30 (capped).
15. **Firing cap by stance**. Moving (not crouched), F = 0.36, fire one shot → F = 0.38 (cap); the next shot's cone is 3 + ⌊97 × 0.38⌋ = 39. Standing still with F = 0.36 left over from moving: no change until the next shot, which sets F = min(0.30, 0.40875) = 0.30.
16. **Firing recovery delay**. F = 0.30, last shot at t = 0. Sub-steps at t = 50 and 95 ms → F unchanged. Sub-step at t = 100 ms → F = 0.28875. *Correction:* the 95 ms sub-step already recovers (the wait is strictly less than `DecDelay`); see case 38.
17. **Firing recovery length**. F = 0.30, recovery allowed, one sub-step per 12.5 ms → cone back to n = 3 after 26 sub-steps (F = 0.0075); F reaches 0 after 27 sub-steps.
18. **Movement rise**. Run flag on, on the ground, μ = 1, M = 0 → M after sub-steps 1..6: 0.04875, 0.0975, 0.14625, 0.195, 0.24375, 0.25 (clamped). Cone for a shot then (F = 0): 3 + ⌊97 × 0.25⌋ = 27.
19. **Movement fall**. From M = 0.25 to standing still (μ = 0): M after sub-steps 1..11: 0.21625, 0.1825, 0.14875, 0.115, 0.08125, 0.05688, 0.03981, 0.02787, 0.01951, 0.01366, 0.00956. The cone returns to n = 3 at sub-step 11.
20. **Turn snap (Q11)**. Standing still, M = 0.20, F = 0. One sub-step with a yaw change of 0.2 rad → ρ ≈ 0.296 > M → M = 0 at once. The same sub-step with no turn → M = 0.20 − min(0.1125, 0.20) × 0.3 = 0.16625. *Correction:* "yaw change" here must be a change of the body yaw; with no movement key held a 0.2 rad mouse turn from an untwisted body goes into the upper-body twist and the measured change is 0, giving 0.16625. See case 42.
21. **Jump**. Run flag on, airborne from a jump, μ = 1, M = 0, F = 0 → after 15 sub-steps M = 0.70 (0.04875 per step, the 15th clamps). Cone 3 + ⌊97 × 0.7⌋ = 70.
22. **Ray deflection**. Forward (0,0,1), right (1,0,0), up (0,1,0), n = 32, draws u₁ = 1, u₂ = 0 → direction ∝ (0.032, −0.032, 1), normalised (0.03198, −0.03198, 0.99898).
23. **View kick sequence**. M4A1, J = 0, no recovery between shots. Upward view change per shot (°): 1.300, 1.261, 1.223, 1.187, 1.151, 1.116, 1.083, 1.050, 0.629, 0, 0. Cumulative after shot 9: 10.000°. J after shot 1 = 0.011345 rad, after shot 2 = 0.022349 rad.
24. **Wag start**. Same as case 23: after shot 1 no side kick (J = 0.0113 ≤ 0.02). After shot 2 a side kick of exactly 1.2° (q = 1) in the flag's direction, and J_y = ∓0.010472 rad.
25. **Recoil return**. J = 0.0113446 rad, 16 ms frame, 50 ms after the kick → view moves down 0.0018080 rad (0.1036°), J → 0.0104406. Same frame 120 ms after the kick → down 0.0036160 rad (0.2072°), J → 0.0095366.
26. **Fire cycle**. Exact-time mode: hold fire from t = 0 with a full clip; the shots fire at t = 0, 105, 210, … ms, with 30 shots by t = 3045 ms. Frame-quantised mode at 60 fps: one shot every 7 frames (116.7 ms).
27. **Semi-auto**. A weapon with `RapidFireCount = 1` and the same animations; hold fire for 1 s → exactly 1 shot. Release and press again → 1 more.
28. **Burst (M16A3)**. Burst on, 300 ms after toggling, hold fire 2 s → 3 shots (t = 0, 105, 210 ms), then none. Tap fire at t = 0 and release at t = 20 ms → still 3 shots. Press again at t = 300 ms (90 ms after the third shot) → nothing until t ≥ 420 ms. A new press then gives the next burst.
29. **Burst toggle lockout**. Toggle burst at t = 0, press fire at t = 200 ms and hold → the first shot fires at t ≥ 300 ms.
30. **Empty and auto-reload**. Clip 1, total 61, hold fire → 1 shot (clip 0, total 60). About 105 ms later the reload starts by itself. 2300 ms after that, clip = 30, total = 60. Still holding fire → firing resumes on the next update.
31. **Partial reload**. Clip 12, total 70 → reload → clip 30, total 70 (reserve 40). Clip 12, total 20 → reload → clip 20, total 20.
32. **Reload refused**. Clip 30, total 120 → nothing. Clip 25, total 25 → nothing.
33. **Reload cancelled by switch**. Clip 5, total 95, start reload, switch weapon at t = 2000 ms, switch back → clip 5, total 95, and the auto-reload does not start (clip > 0).
34. **Fire during reload**. Clip 0, total 90, reload starts at t = 0, fire held from t = 500 ms → no shot before t = 2300 ms. The first shot after that has clip 30 → 29.
35. **Server rate limit**. A weapon with `fireDelay = 100`: shots arrive at 0 and 60 ms → the second deals no damage but total ammo drops by 2.
36. **Penetration**. The server ray passes a 6-unit "Wood" panel and then hits a torso 1000 units away, roll 1.0, unarmoured, band measured on the real distance (Q13 recommendation) → 35 × 0.5 = 17.5. The same with a 12-unit wood panel → the ray stops, no damage.
37. **Two panels**. Glass (×0.9) then plywood (×0.6), then a torso 1000 units away, roll 1.0 → 35 × 0.9 × 0.6 = 18.9.
38. **Recovery at exactly `DecDelay` (corrects 16)**. F = 0.30, last shot stamped at OS time 0 ms. Sub-step at 94 ms → 0.30 (unchanged). Sub-step at 95 ms → 0.28875. Next sub-step (any time) → 0.2775.
39. **Airborne rise after a jump**. In a jump, M4A1, M = 0.50, $\mu S$ = 0.20 (so A = 0.20 < M), S = `JumpPerturb` 0.70. Sub-step 1 → M = 0.50 + min(0.1125, 0.30) × 1.3 = 0.64625. Sub-step 2 → 0.64625 + 0.1125 × 1.3 = 0.79250, capped to 0.70.
40. **A fall is not a jump**. Walked off a ledge (not in a jump), run flag on, falling at 400 u/s with limit 285 → μ = 1, S = `RunPerturb` 0.25, A = 0.25. M = 0.50 → sub-step 1: 0.50 − min(0.1125, 0.25) × 0.3 = 0.46625 (it falls; it does not rise and does not use `JumpPerturb`).
41. **Run flag at rest and crouched**. Standing still, no keys, M = 0: S = `RunPerturb` 0.25 (run flag on at rest), μ = 0, so A = 0 and M stays 0. Crouched and moving at μ = 1: run flag off, S = `WalkPerturb` 0.04, so M rises to at most 0.04 (cone 3 + ⌊97 × 0.04⌋ = 6).
42. **Turn term and the twist (corrects 20)**. Standing still, no movement key, M = 0.20, twist 0: a 0.2 rad mouse turn in one frame → measured change 0 → M = 0.16625. Twist already at its +45° limit and the same turn further right → measured 0.2 rad → ρ = 3 × 0.2 / 2.02875 = 0.29575 → M = 0 (as case 20). Holding a strafe key (twist 0), μ = 1 with S = 0.25: the same turn gives ρ = 0.29575, A = 0.29575 > M → M = min(0.20 + 0.04875, 0.25, 0.25) = 0.24875.
43. **Turn split over sub-steps**. Moving (twist 0), a frame with 3 sub-steps and a 0.3 rad yaw change: ρ = min(1, 0.9 / 2.02875) = 0.44362 on sub-step 1, ρ = 0 on sub-steps 2 and 3.
44. **Recoil kick is not a turn**. Standing still, M = 0, F = 0.30 held, one M4A1 shot raising the view 1.3°: ρ = 0 on the following sub-steps (pitch is not measured). A side wag of 1.2° (0.020944 rad) on the same frame: ρ = 3 × 0.020944 / 2.02875 = 0.03097 on the next sub-step.
45. **μ uses the penalised limit**. Reference loadout (P = 0.2, run limit 228), run flag on, horizontal speed 114, no vertical speed → μ = 0.5, μS = 0.125 (without the penalty, 114/285 = 0.4 would give 0.10).
46. **Second victim**. Two unarmoured victims in line, first hit (client-reported) on the torso at d = 2800, roll 1.0, from the front → first: 35 × 0.7 = 24.5. Second, torso → 24.5 × 0.7 = 17.15 (banded twice). The same at d = 2000 → 35 and 35. The first hit from behind (×1.3) at d = 2000 → first 45.5, second 45.5, whatever the second victim's facing. Second victim armoured (AP 100, default gear) at d = 2000, first not from behind → 35 − 5.25 = 29.75, its AP → 94.75.
47. **Fire delay clock**. A weapon with `fireDelay` 100, client timestamps 0, 60, 110 ms → shot 1 fires, shot 2 dropped (ammo −1), shot 3 fires (110 − 0 ≥ 100). Timestamps 0, 60, 100 → shot 3 fires (not less than 100). Timestamps 0, 150, 200 → shots 1 and 2 fire, shot 3 is dropped (50 ms after shot 2).
48. **Armour by damage type**. AP 100, default body gear only, roll 1.0, unarmoured zone factors as in case 4:
    - burn 10 (no zone factor) → B = 5, D' = 5, AP → 95;
    - melee 45 on the torso → 45, AP stays 100;
    - explosion 50 → 50, AP stays 100;
    - bullet 35 on the groin → 35, AP stays 100;
    - bullet 35 on the torso → 29.75, AP → 94.75 (case 6).
49. **Groin on an infected victim**. Infected victim, AP 100, default body gear, bullet 35 on the groin → counted as torso: 29.75, AP → 94.75 (no groin factor).
50. **Kick carries over a switch (Corrections 1).** M4A1 fired 9 times with no recovery, so $J$ = 0.0872665 rad (its cap). Switch to the M92FS (`BaseCamRecoilPitch` 3.5, `MaxCamRecoilPitch` 14, accel 0) with no time passing, fire once → $k$ = 0.0610865 × (1 − 0.0872665/0.2443461) = 0.0392699 rad: the view rises 2.250° (not 3.500°), $J$ → 0.1069015.
51. **One wag direction per player (Corrections 1).** Player enters the map with the direction flag on R. M4A1 shot 2 kicks R; switch to the MP5A4 and fire twice with no time passing → the MP5A4's first side kick goes in the direction left by the M4A1's last flip test, not a fresh random one; $J_y$ starts from the M4A1's value.
52. **Burst return cap (Corrections 2).** Burst mode on, `MaxCamRecoilPitch` 5, `TripleShotMaxCamRecoilPitch` 8, ammo `JitterWaitRecover` 0.1, $J$ = 0.0418879 rad, 16 ms frame within `JitterDecDelay` → view down 0.0020800 rad, $J$ → 0.0408479. (Normal cap would give 0.0023680 and 0.0407039.) Same with `TripleShotBaseCamRecoilPitch` 0 → unchanged (0.0020800).
53. **Last pellet dropped (Corrections 3).** 8-ray weapon, 9 damage a ray, `fireDelay` 30, all rays on an unarmoured torso at 500 units, roll 1.0 → 7 rays land: 63 damage; total ammo −1. Same with `fireDelay` 0 → 72. A single-ray weapon with `fireDelay` 30 firing every 105 ms → every round lands.
54. **Rays are not rate-limited between shots (Corrections 3).** 8-ray weapon, `fireDelay` 100, shots stamped at 0 and 40 ms → each shot lands rays 1–7, loses ray 8; total ammo −2.
55. **Four intersections (Corrections 4).** No client player hit. Three 6-unit "Wood" panels (world surfaces) then a torso 1000 units away, roll 1.0, unarmoured, band on the real distance → 35 × 0.5³ = 4.375. Four panels → no damage.
56. **No third victim (Corrections 4).** Unarmoured A, B, C in line at 2000, 2100, 2200 units, client-reported torso hit on A from the front, roll 1.0, the line passing only torso spheres of B and C → A 35, B 35, C 0.
57. **Server-side sphere by priority (Corrections 5).** Male skeleton with (for the test) chest priority 2 and head priority 1. A wall-bang through one 6-unit "Wood" panel at 1000 units whose line passes within both the head and the chest sphere radii, head first → chest: 35 × 0.5 = 17.5 (smallest-$\lVert p\rVert^2 - r^2$ would give the head, 105). All priorities equal → the later of the two in the node order. A line through the unwidened box that is within no sphere's radius → no damage; the walk restarts 5 units past the box entry.
58. **Behind from the body (Corrections 6).** Victim's body yaw 45° to the right of its view (no movement key). View forward (0,0,1), right (1,0,0); body forward (0.7071, 0, 0.7071), right (0.7071, 0, −0.7071). Bullet direction (−0.8660, 0, 0.5) → body sector 0, not behind: torso 35 (by the view it would be sector 11, 45.5).
59. **AI damage (Corrections 7).** Bot with an M4A1, roll 1.0, d = 100, unarmoured player: torso 35 × 0.8 = 28; head 28 × 6 = 168 → 50. Armoured (AP 100, default body gear) torso: B = 4.2 → 23.8, AP → 95.8. Bot knife primary on the head: 45 × 0.8 × 6 = 216 (melee, no cap). AI damage of 1500 after armour → ×0.8 first: 1200 → 990.
60. **Widened entry for the band (Corrections 8).** Shooter at the origin aiming along +Z at the chest of a victim whose plain box has half-width 16 on X and Z and its front face at z = 2510 → reported point z = 2494, band 1.0, roll 1.0 → 35 (the plain entry would give 24.5).
61. **Near-miss drop (Corrections 8).** The ray passes the widened box and a sphere but misses the plain box, and the next thing on the line is a wall whose plane has the victim's origin behind it → no player hit is reported; the wall gets the impact. Same with the victim's origin in front of the wall's plane → the player is hit at the widened entry.
62. **Board at limit + 1 (Corrections 9).** "Wood" (limit 10): a 10.5-unit panel, then a torso at 1000 units, roll 1.0 → 17.5. An 11.5-unit panel → the ray stops.
63. **Integer re-perturb (Corrections 9).** Exit from "Glass" ($P$ = 5) with the ray along (0,0,1): draws $k_r$ = 5, $k_u$ = −5 → new direction ∝ (0.005, −0.005, 1) = (0.0049999, −0.0049999, 0.9999750). No draw can give 0.0025.
64. **First-round latency (Corrections 10).** 60 fps, M4A1 (0 ms pre-fire), fire pressed just before the update at t = 0 and held → update 0 sets firing, update 1 plays the pre-fire, update 2 starts `fire_0`, the first round is spent on update 3 (50.0 ms); the next rounds on updates 10 and 17 (166.7, 283.3 ms). Remington 870 (primary shotgun, no pre-fire) → first round on update 2 (33.3 ms).
65. **After a draw (Corrections 10).** 60 fps, M4A1 drawn with fire held; the select animation is seen done on the update at t = 1200 ms → first round on the update at 1250 ms.
66. **Shot direction (Corrections 11).** M4A1 kick at t = 0 leaves $J$ = 0.0113446 (view 0.0226892 rad up). Frames at 16, 32, 48, 64, 80 ms (all within `JitterDecDelay`) return 0.0018080, 0.0017914, 0.0017750, 0.0017587, 0.0017426 rad. A shot on the 80 ms frame leaves 0.0155561 rad above the pre-kick aim (the 80 ms return is not yet in the camera); the view after that frame is 0.0138135 rad up.
67. **Zoomed jump target (Corrections 12).** Ammo `JumpPerturb` 0.7 and (for the test) `ZoomedJumpPerturb` 0.3, zoomed, airborne from a jump, μ = 1: run flag on → $S$ = 0.7; run flag off → $S$ = 0.3.
68. **Jump clamp (Corrections 12).** Same ammo, M4A1 zoomed rates (inc 1.3), run flag off, airborne, $M$ = 0.5, μS = 0.3 → $M$ = 0.5 + 0.1125 × 1.3 = 0.64625, clamped to the plain 0.7: stays 0.64625.
69. **Partial burst fields (Corrections 12).** Burst mode, `TripleShotMinPerturb` 0, `TripleShotMaxPerturb` 60, plain 3/100, meters 0 → cone limits 3/60, n = 3.

## Follow-up (2026-10-31b): Unknown and the other shoot-through surfaces

The "Penetration" table above lists only some shoot-through surfaces. The install's surface table (Steam build 26236938) also marks these, with the same pass rules:

| Surface (id) | Thickness limit | Re-perturb | Range × | Damage × |
|---|---|---|---|---|
| Unknown (0) | 10 | 10 | 1.0 | 0.6 |
| trussing (23) | 10 | 10 | 1.0 | 1.0 |
| Invisible metal (206) | 0 (always) | 0 | 1.0 | 0.9 |
| Air (1) | 0 (always) | 0 | 1.0 | 1.0 |
| Boss3_weak (65) | 55 | 15 | 1.0 | 0.3 |

Unknown is the surface of every solid polygon stored with surface type 0 (2.5 % of the level polygons and 12 % of the world-model polygons across the install) and of objects with no surface of their own, so ordinary walls of that type up to about 11 u thick are shot through at damage × 0.6 (`effects_sounds.md` Follow-up (2026-10-31b) 3, its cases 94–96). Test case 70.

70. **Unknown wall.** Server ray on a surface-0 wall 8 u thick, a torso behind it: damage × 0.6; on a 12 u surface-0 wall: stopped.

## Open questions

1. **Which build's data?** The reference attribute data may not match the Steam build. Damage, spread and recoil values should be re-read from the install once the attribute reader (item 3) works. Check: diff the M4A1 and "5.56mm M4A1" records.
2. **Fire-rate in the live game**: does the real game fire a 105 ms animation at 571 rounds/min or at a frame-rounded rate? Measure rounds per second at locked 60 and 144 fps on a range map. Also check whether the empty `prefire` delays the first shot by a frame.
3. **Spread sub-step rate in the live game**: is the remainder really discarded? Measure the crosshair settle time after a 7-round burst at 60 vs 144 fps. The millisecond clock's coarse resolution on Windows (~15.6 ms) may also blur the 95 ms delays.
4. **Wall-bang falloff (Q13)**: confirm in game that a through-wood hit at about 2800 units does the same damage as at about 1000 units on a map where the world origin is far away.
5. **Second victim**: can one bullet damage two players standing in line? The reference suggests yes, through the server ray. Test with two bots in line.
   **Answered (damage rule):** the second victim's damage is banded again from the already banded running value, not from the base damage, and with the same distance as the first hit (fire position to the first victim's reported impact point); it also inherits the first victim's figure modifier and behind bonus. Inside the first band this changes nothing; at 2500–3000 units it gives ×0.49 of the base. See Edge cases ("Server-side second victim"); test case 46. Whether it happens at all in the live game still needs the two-bots test above (the server ray must not stop on the first victim's own body).
   **Answered (reach):** in the reference code the walk skips the first character box it meets and then damages the next character on the line; it ends there, so at most two players are hit by one ray (Corrections 4, test case 56). The live two-bots test remains the confirmation.
6. **Run flag at rest**: whether the "run" control flag is set while standing still or walking is a movement-spec question. It only selects between `WalkPerturb` and `RunPerturb`.
   **Answered:** the run flag is "not crouched and not walking" (walk key XOR always-run), set at rest too; walking with the walk key or crouching clears it (crouched → `WalkPerturb`). The maximum speed in μ is movement.md's full speed limit for the tick, with the weapon and gear penalty included, plus the crouch blend, zoom, confirmed sprint and damage slow. See Spread, "State values"; test cases 41 and 45.
7. **Character gear**: which `GearID` each character uses, and the full gear table, belong to the characters/inventory specs. The values here are examples.
8. **Hidden multipliers**: perks (they can shrink the cone), set items, mutation buffs, game-room damage scaling and attachments all hook into damage, rate, cone and recoil. They are identity in a plain match; a later spec should list them.
9. **Hit sphere centres**: the sphere positions depend on the skeleton's animated bones (models spec). Until then, place spheres on our placeholder skeleton by body part.
10. **Shot direction timing**: does the ray use the camera orientation from before or after this frame's recoil return? The reference reads the camera object, which may lag the view angles by a frame. This is a minor effect on the second shot onward.
    **Answered:** before. The camera rotation is set in the frame's camera update; the recoil return, the shot and the kick run later in the same frame (just before rendering). The shot includes the previous frame's kick and return and this frame's mouse and sway, not this frame's return. Corrections 11; test case 66.
11. **Spread recovery delay, `<` or `≤`?** Does recovery wait while the time since the last shot is at most `DecDelay`, or strictly less (case 16's 95 ms)?
    **Answered:** strictly less. The elapsed time is whole OS milliseconds; a sub-step at exactly `DecDelay` ms (95) recovers. After the first recovery step the stamp is cleared. Applies in Spread, firing meter; case 16 corrected by case 38.
12. **"Airborne" for spread.** Only from a jump until landing, or any time off the ground? How does the airborne rise grow?
    **Answered:** only movement.md's "in a jump" state: from the jump tick until the landing is detected; walking off ledges and other falls do not count (they use the walk or run target and decay normally), jump pads do not count either. The inverted rise is `M + min(Δ·9.0, M − A)·PerturbIncSpeed` per sub-step (decrease step size, increase speed), then capped at `JumpPerturb`. Applies in Spread, movement meter; test cases 39 and 40.
13. **The turn term.** Does it include the recoil kick, and how is one frame's turn split across sub-steps?
    **Answered:** it measures the body orientation, whose pitch does not follow the view in this build, so the vertical kick (and any pitch) never counts; the side wag does count through the yaw. Mouse yaw counts only beyond the ±45° upper-body twist while no movement key is held, fully while one is. The whole frame's change lands on the frame's first sub-step; the others see 0. Applies in Spread, movement meter; case 20 corrected by case 42; cases 43 and 44. The twist detail is easy to confirm live: stand still with the M4A1, flick the mouse 30° back and forth repeatedly and check that the crosshair never shrinks faster than when standing still without turning; then repeat while strafing.
14. **Fire delay origin.** Does the delay count from the previous shot fired, or from the previous round received even if dropped or refused?
    **Answered:** from the last shot that passed the rate check, by the client's own fire timestamps; a dropped shot does not restart the clock, a shot refused later (no ammo) does. Exactly `fireDelay` ms is allowed. Applies in Trigger and fire cycle ("Server rate limit"); test case 47.
    **Answered further (Corrections 3):** "the last shot that passed" includes every ray that was not checked. Only a shot's last ray is checked, and the rays before it have already stored the shot's own timestamp. A multi-ray shot with `fireDelay` > 0 therefore always loses its last ray; test cases 53 and 54.
15. **Armour coverage.** Does armour protect against bullets only, and is the groin torso for armour?
    **Answered:** coverage is per gear piece and per damage type in the data; stock body gear covers bullets and burn (0.5 everywhere for burn), head gear bullets only, the gas mask poison; melee and explosions are not covered by any stock gear, and crush skips gear. The groin is not torso: it gets no protection (full damage, no AP used), except on infected victims, whose groin counts as torso. Applies in Armour; test cases 48 and 49.
16. **Node hit priorities** (Corrections 5). The server picks a server-found character's sphere by the skeleton data's `Node#Priority`, which no spec lists yet. Read them from the install's model file (male and female skeletons) and add them to `characters.md` 11. Until then, give every node priority 0 (the later node in the table wins a tie) and say so in the implementation's notes.
17. **Which stock weapons have `FireDelay` > 0** (Corrections 3). The shotguns' and the double barrel's `FireDelay` (missing or a non-integer value both mean 30) decides whether they lose a pellet per shot. Read the records. Live check: an R870 at point blank on an unarmoured bot, all pellets on the torso, should do 63, not 72.
