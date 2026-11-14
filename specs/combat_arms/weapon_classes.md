# Combat Arms: weapon classes

Source basis: the client's weapon logic (fire, reload and per-round reload state machine, spread and the multi-ray path), the client's player manager (zoom levels, field of view, scope sway, mouse scaling while zoomed, automatic zoom-out), the client's interface overlay code (scope overlay), the shared weapon and modification attribute managers, the server's player object (reload bookkeeping per class), plus the shipped attribute data (weapons file, modifications file, projectile effects file) and the first-person view-model animation files, all from a reference client build (US data set). Numbers for typical weapons are that build's data; the Steam build may differ (see Open questions).
Status: draft

Sections: Summary; Units and conventions; Constants; How the data marks a class; Typical weapons; Behavior (1 assault rifles, 2 SMGs, 3 machine guns, 4 pistols, 5 shotguns, 6 sniper rifles, 7 zoom and scope, 8 launchers and other classes); Per-tick order; Edge cases; Quirks; Corrections (2026-10-19); Corrections (2026-10-28); Corrections (2026-10-30); Follow-up (2026-11-14) (zoom sounds, focus blur); Test cases; Open questions.

This spec builds on `weapon_firing.md` (one rifle, the M4A1) and says only what differs. Terms used without definition here (spread fraction $p$, firing meter $F$, movement meter $M$, cone $n$, fire cycle, view kick $J$) are defined there.

---

## Summary

- **A class is mostly data, not code.** The weapon record's `Guntype` names the class, but the firing code almost never branches on it. Pellets, pump and bolt cycles, shell-by-shell reloads, semi-auto and the scope are all switched on by other fields and by the view-model animations: `VectorsPerRound`, `RapidFireCount`, `ReloadPerBullet`, `ReloadAfterFire`, `DefaultScopeAttach`, the fire animation's length. A shotgun is "8 rays per shot, reload per shell, long fire animation". A bolt-action sniper is "fixed wide cone unless zoomed, 1.4 s fire animation, leave zoom after each shot".
- **Zoom is a property of the attached sight**, not of the class. Any weapon whose current scope attachment has zoom levels can zoom: all sniper rifles ship with one, a few rifles and SMGs have built-in sights, and the M4A1 and pistols have none. Right mouse cycles levels (toggle, not hold). The field of view slides at a fixed angular speed set by the weapon's `ZoomTime`. Zoomed accuracy, the overlay and the mouse scale apply **from the frame of the press**, which is what makes quick-scoping work.
- **What the code does branch on, by class**: right mouse is alt-fire instead of zoom for knives, the double barrel, the snowball gun and the crossbow. The server lets shotguns fire during its reload bookkeeping. Ammo pickups refill only rifle, SMG, machine gun, sniper and rocket primaries. A sniper-only perk stops scope sway. Everything else is the same rule set as `weapon_firing.md` with different numbers.

## Units and conventions

- Same as `weapon_firing.md`: LithTech world units, +Y up, left-handed (+Z forward, +X right). Angles in radians in the formulas; degrees where the data or the field of view uses degrees (stated each time). Positive pitch looks down; positive yaw turns right.
- Field of view: the horizontal angle `FovX` and vertical angle `FovY`, in degrees, from the console variables of those names (built-in values 90 and 75). "FOV" alone means the horizontal one.
- Time: animation lengths in milliseconds of animation time at rate 1; `ZoomTime` in seconds; frame time $\Delta t$ in seconds, capped at 0.1 s per frame. The automatic zoom-out uses the system millisecond clock.
- Zoom level $L$: 0 = not zoomed, 1..$L_{max}$ = scope steps. "Zoomed" ($Z$) means $L > 0$ **or** a zoom transition is running. Every rule below that says "while zoomed" uses $Z$, not $L$.
- Data fields are named as they appear in the attribute data. Weapon, ammo, scope and animation names ("L96A1", "Sniper Scope", "fire_0") are data names the game's files expose.

## Constants

Fixed in code. Per-weapon numbers are under "Typical weapons".

| Name (ours) | Value | Unit | Meaning |
|---|---|---|---|
| fov_x_normal | 90 | deg | Console variable `FovX` default; FOV when not zoomed |
| fov_y_normal | 75 | deg | Console variable `FovY` default |
| zoom_out_reference | 20 | deg | Zoom-out speed is always computed as if leaving a 20° view |
| zoom_time_fallback | 0.5 | s | Used only when no weapon is current; otherwise the weapon's `ZoomTime` (field default also 0.5) |
| max_zoom_levels | 10 | levels | Most `ZoomFov#` entries read per scope |
| sens_zoom_step | 3 | — | Mouse divisor is $1 + 3L$ |
| sens_base | 0.5 | × | Mouse deltas are always halved (zoomed or not); stated so the zoomed ratio is clear |
| bolt_zoom_out_delay | 300 | ms | After a zoomed shot of a `ReloadAfterFire` weapon, zoom steps once more after this much time (strictly greater) |
| sway_time_scale | 1/10 | — | Sway phase is game time (s) × this × frequency |
| sway_amount_scale | 1/1000 | — | Sway rate is speed × this, per second |
| frame_time_cap | 0.1 | s | Longest frame time any of these rules sees |
| lod_zoom_adjust | −5 | LOD steps | Model detail offset at full zoom (presentation only) |
| lod_zoom_reference | 2 | deg | FOV at which the full LOD offset applies (presentation only) |

## How the data marks a class

### `Guntype` (weapon record)

| Value | Class | Code that branches on it |
|---|---|---|
| 0 | knife | right mouse = alt-fire (see `inventory.md`) |
| 1 | pistol | HUD only |
| 2 | shotgun | server accepts shots during its reload bookkeeping (section 5.6); not recorded as "last primary" (no ammo-pickup refill) |
| 3 | SMG | ammo pickups ("ammo" boxes) |
| 4 | assault rifle | ammo pickups ("ammo" boxes) |
| 5 | machine gun | ammo pickups ("MG/SR" boxes) |
| 6 | sniper rifle | ammo pickups ("MG/SR" boxes); the breath perk (section 7.9) |
| 9 | rocket launcher | recorded as "last primary"; server reload bookkeeping as shotguns |
| 13 | double barrel | right mouse = alt-fire (both barrels); second reload animation |
| 14 | snowball gun | right mouse = alt-fire |
| 15 | special grenade launcher (M32) | server reload bookkeeping as shotguns |
| 21 | crossbow | right mouse = alt-fire |
| 7, 8, 10–12, 16–20, 22 | thrown, mission bomb, mine, sensor, air strike, kits, installables, turret, crossbow bomb | other specs (grenades, objectives, gadgets) |

Which **key slot** a weapon occupies is not `Guntype`: it is the weapon's position in the weapon order lists (`inventory.md`, section 1).

### The flags that really make the class

| Field (weapon record unless noted) | Effect | Typical use |
|---|---|---|
| `RapidFireCount` | 0 automatic, 1 semi-auto, n burst cap (`weapon_firing.md`) | 1 on pistols, pump shotguns, bolt and semi snipers |
| fire animation length (`fire_0`) | Fire cycle; also the **pump or bolt cycle** | 80 ms SMG, 105 ms rifle, 600 ms pump, 1400 ms bolt |
| `VectorsPerRound` | Rays per shot (pellets) | 8 on most shotguns, 6 on the AA-12 and SAIGA |
| `ReloadPerBullet` (0/1) | Reload one round per loop of the reload animation | shotguns, M32 |
| `ReloadAfterFire` (0/1) | Leave zoom 300 ms after a zoomed shot; no zooming in during the fire animation | bolt snipers, some launchers |
| `DefaultScopeAttach` | Names the scope attachment fitted at spawn (modifications file) | all snipers; some rifles and SMGs |
| `MinPerturb` = `MaxPerturb` = 100 | Fixed wide cone when not zoomed | snipers |
| ammo `WalkPerturb` = `RunPerturb` = `JumpPerturb` = 0 | Movement never widens the cone | shotguns |
| `AccumPreFire` (int) | Non-zero: spin-up and spin-down are reversible (section 3.3) | miniguns |
| `ContinuousFire` | Continuous-fire mode (`weapon_firing.md`) | some semi rifles and pistols |
| `DisableCrosshair` | No crosshair (`hud_crosshair.md`) | snipers |
| `ZoomTime`, `ZoomedMinPerturb`, `ZoomedMaxPerturb`, `ZoomedPerturbIncSpeed`, `ZoomedPerturbDecSpeed`, ammo `Zoomed*` | Zoom speed and zoomed accuracy (section 7) | any zoomable weapon |
| `CameraSwayXSpeed/YSpeed/XFreq/YFreq` | Scope sway (section 7.8); defaults 12 / 2 / 6 / 20 | any zoomable weapon |
| `ZoomedMovePenalty` | **Read but never used** (Q4) | — |

There is no per-weapon penetration field. Penetration depends only on the surface (`weapon_firing.md`), and applies per ray, so each shotgun pellet penetrates on its own.

### Scope records (modifications file)

A scope is a `Mod#` record with `Type` = 1 (scope; 2 would be a dot-sight toggle, below) and:

| Field | Meaning |
|---|---|
| `ZoomLevel` | $L_{max}$, the number of zoom steps (0 = cannot zoom) |
| `ZoomFov0..` | FOV in degrees for level 1, 2, … (`ZoomFov0` is level 1). Shipped scopes list one entry more than `ZoomLevel`; that last entry is never used. |
| `ZoomSpriteName`, `ZoomSpriteScale` | The overlay picture and its scale |
| `ZoomInSound`, `ZoomOutSound` | Played on each step in or out |
| `TargetRange`, `SightSystemStyle` | Range readout (`hud_crosshair.md`, A9) |
| `ZoomedMinPerturb`, `ZoomedMaxPerturb`, `ZoomedPerturbIncSpeed`, `ZoomedPerturbDecSpeed` | Added to the weapon's zoomed values (attachments spec); 0 on the default sniper scopes |

Shipped scopes (US data): "Sniper Scope" and "Dragunov Scope" 1 level at 20°; "PSG Scope" 1 level at 35°; "L115 Scope" and "M200 Scope" 1 level at 15°; "Barrett Scope" 2 levels at 30° and 10°; rifle sights ("G36 sight", "AUG-A1 sight", "L85A1 MOD Scope") 20°; dot sights and holo sights 50°; "P90 sight" and the ITECH sights 75°.

A scope attachment whose `Type` is 2 (dot sight) does not zoom: right mouse toggles a "dot-sighted" crosshair flag instead (`hud_crosshair.md`). The shipped dot-sight records are `Type` 1, so they zoom.

## Typical weapons

From the US reference data and the view-model animation files. "Draw" is the `select` animation, "Fire" the `fire_0` animation (the fire cycle at rate 1). Damage bands are `Effectrange0..2` (units) with the ammo's `Rangedamage0..2`.

| | M4A1 (AR) | MP5A4 (SMG) | M60 (MG) | FN_Minimi (MG) | Beretta_M92FS (pistol) | Remington_870 (shotgun) | L96A1 (sniper, bolt) | Dragunov_SVD (sniper, semi) |
|---|---|---|---|---|---|---|---|---|
| `Guntype` | 4 | 3 | 5 | 5 | 1 | 2 | 6 | 6 |
| Clip / total | 30 / 120 | 30 / 120 | 100 / 200 | 100 / 200 | 15 / 45 | 7 / 35 | 5 / 20 | 10 / 40 |
| `RapidFireCount` | 0 | 0 | 0 | 0 | 1 | 1 | 1 | 1 |
| Fire (ms) | 105 | 80 | 115 | 125 | 180 + 150 post-fire | 600 (pump) | 1400 (bolt) | 400 |
| Rounds/min at rate 1 | 571 | 750 | 522 | 480 | ≤ 333 | 100 | 43 | ≤ 150 |
| Reload (ms) | 2300 | 2000 | 3500 | 3100 | 1500 | 333 + 700/shell + 900 | 2900 | 2600 |
| Draw (ms) | 1200 | 1000 | 1500 | 1500 | 650 | 1000 | 1100 | 1200 |
| Rays × damage | 1 × 35 | 1 × 29 | 1 × 47 | 1 × 38 | 1 × 26 | 8 × 9 | 1 × 160 | 1 × 100 |
| `Range` | 3500 | 2500 | 3000 | 3000 | 2500 | 1000 | 5000 | 5000 |
| Bands | 2500/3000/3500 | 500/2000/2500 | 1500/2500/3000 | 1500/2500/3000 | 1000/2000/2500 | 600/800/1000 | 4000/4500/5000 | 4000/4500/5000 |
| × per band | 1.0/0.7/0.35 | **1.1**/0.7/0.35 | 1.0/0.7/0.35 | 1.0/0.7/0.35 | 1.0/0.7/0.35 | 1.0/0.7/0.35 | 1.0/0.7/0.35 | 1.0/0.7/0.35 |
| `MinPerturb`/`MaxPerturb` | 3/100 | 8/100 | 13/100 | 12/100 | 10/100 | 35/100 | **100/100** | **100/100** |
| `DynamicPerturbFactor` | 1.0 | 1.0 | 1.1 | 1.1 | 1.1 | 1.2 | 1.0 | 1.0 |
| `PerturbIncSpeed`/`DecSpeed` | 1.3/0.3 | 0.8/0.37 | 1.5/0.3 | 1.2/0.25 | 1.0/0.7 | 5.0/1.0 | 1.0/0.1 | 1.0/0.1 |
| Fire cap stand / move / crouch / crouch-move | .30/.38/.30/.38 | .18/.20/.17/.19 | .22/.42/.14/.28 | .25/.47/.15/.30 | .20/.30/.18/.25 | .45 all | 1.0 all | 1.0 all |
| Walk / run / jump target | .04/.25/.70 | .09/.10/.58 | .18/.24/.80 | .17/.25/.80 | .17/.20/.70 | **0/0/0** | 1.0/1.0/1.0 | 1.0/1.0/1.0 |
| `DecDelay` (ms) | 95 | 180 | 100 | 100 | 100 | 100 | 2400 | 2400 |
| Zoomed min/max | 2/30 (no sight) | 7/17 (no sight) | — | — | 9/18 (no sight) | — | 4/60 | 2/30 |
| Zoomed inc/dec | 1.3/0.3 | 0.8/0.37 | — | — | 1.0/0.7 | — | 1.0/0.5 | 1.0/0.5 |
| `ZoomTime` (s) | 0.15 | 0.15 | — | — | 0.13 | — | 0.18 | 0.10 |
| Scope | none | none | none | none | none | none | "Sniper Scope" 20° | "Dragunov Scope" 20° |
| Sway X speed/freq, Y speed/freq | 5/15, 3/12 | defaults | — | — | — | — | 4.5/10, 1.1/10 | 6.5/6, 1.1/10 |
| Kick `BaseCamRecoilPitch` / `MaxCamRecoilPitch` (deg) | 1.3 / 5 | 1.4 / 7 | 3.0 / 18 | 2.3 / 15 | 3.5 / 14 | 5 / 25 | 5 / 5 | 4 / 5 |
| `MaxRandomPitch` | 0.02 | 0.03 | 0.03 | 0.03 | 0.02 | 0 | 0 | 0 |
| Wag R / L (deg), `MaxCamRecoilWag` | −1.2/1.2, 15 | −1.2/1.2, 15 | −1.25/1.25, 18 | −1.2/1.2, 15 | −1.1/1.1, 10 | −2/2, 50 | −1/1, 10 | −1/1, 10 |
| `MovePenalty` | 0.10 | 0.07 | 0.33 | 0.28 | 0.03 | 0.08 | 0.24 | 0.24 |
| `ReloadPerBullet` / `ReloadAfterFire` | 0/0 | 0/0 | 0/0 | 0/0 | 0/0 | 1/0 | 0/1 | 0/0 |
| `DisableCrosshair` | 0 | 0 | 0 | 0 | 0 | 0 | 1 | 1 |

Other weapons used below: SPAS-12 (shotgun, `RapidFireCount` 0, 8 × 8 damage, fire 450 ms, reload 334 + 400/shell + 933); Desert Eagle (pistol, `RapidFireCount` **0**, 70 damage, fire 600 ms, 7/14); M107CQ (sniper, automatic, "Barrett Scope", `ZoomTime` 0.16); ASW338LM (bolt sniper, "Barrett Scope", `ZoomTime` 0.15); Double_Barrel (`Guntype` 13, 2/14, 8 × 14, `DynamicPerturbFactor` 1.3, min/max 50/100, bands 250/500/1000 at 1.0/0.6/0.2); Minigun_Special (`Guntype` 5, `AccumPreFire` 1, 500/500, 25 damage); LAW and RPG-7 (rockets, 1/3); M32 (`Guntype` 15, 6/12, `ReloadPerBullet` 1).

Movement speeds follow from `movement.md` with the reference character (vest `MovePenalty` 0.1, run 285 u/s, walk 70 u/s): run speed $285\,(1 - 0.1 - P_{weapon})$, so M4A1 228, MP5A4 236.55, M60 162.45, Minimi 176.7, M92FS 247.95, R870 233.7, L96A1 188.1 u/s. Zoomed speed is walk speed with the same penalty: L96A1 $70 \times 0.66 = 46.2$ u/s.

## Behavior

### Rules shared by every class

- **Cone integers.** The cone limits and the dynamic factor $g$ follow `weapon_firing.md`, with the arithmetic made exact here. $g$ is a 32-bit float. Each limit is truncated toward zero after multiplying: $n'_{min} = \lfloor n_{min}\, g \rfloor$, $n'_{max} = \lfloor n_{max}\, g \rfloor$, then $n'_{min} \leftarrow \min(n'_{min}, n'_{max})$. The cone is

```math
n = \operatorname{trunc}\!\left(n'_{min} + \left\lfloor (n'_{max} - n'_{min})\, p \right\rfloor \cdot g\right)
```

  $g$ is the 32-bit float nearest the data value, and each product is truncated without first being rounded back to 32 bits (multiply in double precision, then truncate). Because 1.3 is stored as 1.29999995, $50 \times 1.3$ truncates to **64** and $100 \times 1.3$ to **129**. The common factors are safe: $10 \times 1.1 = 11$, $13 \times 1.1 = 14$, $35 \times 1.2 = 42$, $100 \times 1.2 = 120$, $100 \times 1.1 = 110$. See Open question 13.
- **Zoomed values** replace the plain ones, field by field, whenever $Z$ holds and the zoomed field is non-zero: cone limits (`ZoomedMinPerturb`/`ZoomedMaxPerturb` plus the scope's additions), meter speeds (`ZoomedPerturbIncSpeed`/`DecSpeed`), and the ammo's `Zoomed*` caps and targets. A zero zoomed field keeps the plain value. Shipped sniper ammo has no `Zoomed*` keys, so its plain caps and targets apply zoomed too.
- **Fire cycle.** The cycle is the fire animation (`weapon_firing.md`). A long fire animation **is** the pump or bolt: nothing in the data shortens it, and the next shot can only start when it ends.
- **Movement fraction** $\mu = |v| / V$ uses the current speed limit $V$ from `movement.md`, which is already walk speed while zoomed. Walking at full scoped speed therefore gives $\mu = 1$, not 70/285.

### 1. Assault rifles (`Guntype` 4)

- Exactly `weapon_firing.md` (M4A1, and the M16A3 for burst). No class-specific rule.
- The M4A1 has no scope attachment, so right mouse does nothing on a stock M4A1. Its zoomed fields (2/30, `ZoomTime` 0.15, sway 5/15, 3/12) apply only when a scope attachment is fitted (attachments spec). Rifles with built-in sights (G36, AUG-A1, L85A1 MOD: 20°; ITECH/HDS-4 sights: 75°; dot and holo sights: 50°) zoom by section 7.
- Some semi rifles (M14 family, K2, M1 Garand) have `ContinuousFire` 1 (`weapon_firing.md`, gate 4).

### 2. SMGs (`Guntype` 3)

No class-specific rule. The SMG feel is the data:
- Fast cycle: MP5A4 80 ms, 750 rounds/min at rate 1. Frame-quantised: 5 frames = 83.3 ms at 60 fps.
- Moving barely matters: the run target is 0.10 against the rifle's 0.25. Full-speed running gives $n = 8 + \lfloor 92 \times 0.10 \rfloor = 17$, against 27 for the M4A1. In return the floor is wider (8 vs 3).
- Close-range bonus: the first band multiplier is 1.1, so the MP5A4 does 31.9 out to 500 units, 20.3 to 2000 and 10.15 to 2500. Shorter `Range` (2500).
- Lighter: `MovePenalty` 0.07 (236.55 u/s run), draw 1000 ms, reload 2000 ms.
- Firing spread per shot: $\Delta \cdot 3 \cdot 0.8 = 0.03$, cap 0.18 standing. Cones from rest: 8, 10, 13, 16, 19, 21, 24, then 24 (cap reached after the 6th shot).

### 3. Machine guns (`Guntype` 5)

#### 3.1 Regular LMGs (M60, Minimi, K3, MK.48)

- No bipod, deploy, prone, spin-up or overheating exists in this build: no field or code for any of them. An LMG is a rifle with a 100-round clip, long reload (3100–3500 ms), long draw (1500 ms) and a heavy `MovePenalty` (0.28–0.33).
- **Crouching pays off** through the firing cap only (the ray floor stays `MinPerturb`, `weapon_firing.md` Q3): M60 cap 0.22 standing, 0.14 crouched, 0.42 moving.
- M60 cones from rest, standing ($g = 1.1$: $n'_{min} = 14$, $n'_{max} = 110$; $+0.05625$ per shot): 14, 19, 25, 31, 37, 37, … Crouched: 14, 19, 25, 28, 28, …
- Heavy kick: M60 3.0° per shot with a 36° view-climb ceiling (2 × `MaxCamRecoilPitch`). From rest, no recovery: 3.000°, 2.750°, 2.521°, 2.311°, …

#### 3.2 Fire rate

M60 115 ms (522/min), Minimi 125 ms (480/min). Same frame rounding as the rifle (`weapon_firing.md` Q2).

#### 3.3 Miniguns (`AccumPreFire` ≠ 0)

The M134 family (`Minigun_Special` and variants, `Guntype` 5) has the only spin-up in the game. It is built from the pre-fire and post-fire animations:
- `prefire` 1300 ms is the spin-up; `postfire` 1333 ms is the spin-down; `fire_0` is a 170 ms loop carrying fire keys at 0, 34, 68, 102, 135 and 170 ms (one round each, so about 34 ms per round; see Open questions for the key at the loop end).
- Press from idle: spin-up from 0; the first round fires when the spin-up ends (1300 ms). Release: the loop finishes its current play, then the spin-down plays.
- **Reversible spin**: releasing during the spin-up starts the spin-down part-way, and pressing during the spin-down starts the spin-up part-way, so that the spin "position" carries over. With $u$ the elapsed time in the current animation and $L$ its length:

```math
t_{start}(\text{spin-down}) = \left\lfloor L_{post}\left(1 - \frac{u_{pre}}{L_{pre}}\right)\right\rfloor,\qquad t_{start}(\text{spin-up}) = \left\lfloor L_{pre}\left(1 - \frac{u_{post}}{L_{post}}\right)\right\rfloor
```

  Times are whole milliseconds. Weapons with `AccumPreFire` = 0 always restart a pre-fire from 0.
- No overheating; the 500-round clip is the only limit.

### 4. Pistols (`Guntype` 1)

- Secondary class (`inventory.md`): 650 ms draw, `MovePenalty` 0.03.
- **Semi-auto rate limit** (`RapidFireCount` 1, M92FS): there is no time-based limit (`FireDelay` 0). The limit is the fire animation:
  - One press fires one round. Holding does nothing more.
  - The next round can start only when `fire_0` (180 ms) has ended. A new press that is **still held** when `fire_0` ends fires at that moment. A press that starts and ends inside the 180 ms is lost.
  - After `fire_0` ends without a held press, `postfire` (150 ms) plays. A new press during `postfire` cuts it short and fires at once. The pre-fire is zero-length, so this costs one extra update (`weapon_firing.md`, edge cases).
  - Fastest possible rate: one round per 180 ms (333/min) at rate 1, frame-rounded up.
  - Automatic weapons (`RapidFireCount` 0) do **not** cut a post-fire short: they wait for it to finish.
- M92FS spread ($g = 1.1$: $n'_{min} = 11$, $n'_{max} = 110$, $+0.0375$ per shot, cap 0.20): 11, 14, 18, 23, 26, 30, 31, 31, … The kick is large: 3.500°, 3.063°, 2.680°, 2.345°, … with no recovery.
- **Desert Eagle is automatic**: `RapidFireCount` 0, so holding fire shoots every 600 ms (its `fire_0`). Players hold the trigger. Keep it as data.
- Pistols have zoomed fields and `ZoomTime` but no scope, so they never zoom.

### 5. Shotguns (`Guntype` 2)

#### 5.1 Pellets and pattern

- `VectorsPerRound` rays per shot (Remington_870: 8; AA-12 and SAIGA-20K: 6). One shot spends one round.
- Every pellet uses the **same** cone $n$ (from the meters at the moment of the shot) and two **fresh** uniform draws, so the pattern is random: each pellet lands uniformly in the square $[-n, n]^2 / 1000$ around the aim, independently. There is no fixed spread pattern and no centre pellet.
- The draws come from the same running random table as single shots, two per pellet, in pellet order.
- Remington_870 cone: $g = 1.2$, $n'_{min} = 42$, $n'_{max} = 120$. With $p = 0$, $n = 42$: each pellet within ±0.042 on each axis (±2.41°).

#### 5.2 Why the shotgun cone never changes

- The ammo's walk, run and jump targets are all 0, so the movement meter stays at 0. Running, jumping and turning never widen a shotgun (turning only raises the target, and the meter is then clamped to $\mu S = 0$).
- The firing meter grows 0.1875 per shot ($\Delta \cdot 3 \cdot 5.0$) up to 0.45, but recovers at 0.0375 per sub-step after a 100 ms delay, so it is back to 0 within 162.5 ms. The 600 ms pump (450 ms on the SPAS-12) is longer, so **every shot uses $n = 42$** in practice.

#### 5.3 Damage per pellet and falloff

- Each pellet is a separate ray with its own damage roll, range band, hit zone and armour step (`weapon_firing.md`, "Damage per ray"). Remington_870: 9 per pellet, ×1.0 to 600 units, ×0.7 to 800, ×0.35 to 1000; past 1000 the ray ends (`Range` 1000), so nothing is hit.
- All 8 pellets in the torso at ≤ 600 units: 72. One head (54) and 7 torso (63): 117.

#### 5.4 Pump action

- Pump shotguns (Remington_870, M590 family: `RapidFireCount` 1) are semi-auto with a 600 ms fire animation that contains the pump. A held press fires again when the animation ends; there is no post-fire.
- Semi-auto shotguns (SPAS-12, Benelli M4: `RapidFireCount` 0) fire automatically every `fire_0` (450 ms) while held.
- Weapons whose shop item category is "primary shotgun" never play a pre-fire animation, even if the model has one.

#### 5.5 Shell-by-shell reload (`ReloadPerBullet` 1)

- **Start**: as `weapon_firing.md` (refused if the clip is full or equals the total). Refused while the draw animation plays (the automatic reload at 0 waits for the draw to end, `inventory.md`).
- **Sequence**: `PreReload` once, then `reload` once per shell, then `PostReload`. Remington_870: 333 ms, 700 ms per shell, 900 ms. SPAS-12: 334, 400, 933.
- **Shell count**: each time a `reload` loop ends, the clip rises by one. If the clip is still below both `ShotsPerClip` and the total, another loop starts at once; otherwise `PostReload` plays. With clip $c$, clip size $C$ and total $T$, the number of loops is $k = \min(C, T) - c$, and

```math
t_{shell\,i} = t_0 + L_{pre} + i\,L_{loop}\ (i = 1..k), \qquad t_{done} = t_0 + L_{pre} + k\,L_{loop} + L_{post}
```

  All times are divided by the reload rate (`weapon_firing.md`); frame rounding applies at each animation boundary.
- **Interrupting by fire**: pressing fire while the clip is > 0 stops the reload **at once**, in pre-reload, mid-shell or post-reload. The shell being inserted is not added. The weapon goes to firing and shoots on the next updates. No post-reload plays. Shells already loaded stay. (With clip 0 the fire request is off, so fire cannot interrupt until the first shell is in.)
- **Holding fire on an empty pump gun** therefore loads one shell (pre + one loop: 1033 ms), fires it, and restarts the reload when the pump animation ends: one shot every 1633 ms.
- **Switching** abandons the reload with the loaded shells kept (`inventory.md`). Sprint does not stop it.

#### 5.6 Server reload bookkeeping (why shotguns can fire mid-reload)

- When a reload starts, the server marks the player as reloading and drops shots that arrive while the mark is set (dropped whole: no damage and no server-side ammo spent). For magazine weapons the mark lasts the **third-person** reload animation's length for the player's current stance, divided by $\min(r_{reload}, 2.0)\cdot(2 - c)$, where $c$ is the server setting `ReloadCheckTime` (section `[TimeWeight]`, built-in 0.8, so ×1.2). The server fills its own clip when the mark ends. A weapon whose record gives its own third-person reload length is timed by the animation instead (Corrections 2).
- For shotgun-class weapons (`Guntype` 2), rockets, the M32 and gadgets, the server instead fills its own clip at once and clears the mark on the next shot: for shotguns when the clip is not 0; for rockets, the M32 and gadgets when the total is not 0. Our implementation: let a shotgun shot through whenever the shooter's clip is > 0.

#### 5.7 Multi-pellet hit carry-over (Q1)

- The client resets its "hit a player" record once per shot, not once per pellet (console variable `AllowMultipleVectorsPerRoundBug`, default 1).
- So, once one pellet of a shot hits a player, **every later pellet of the same shot** is also reported as a hit on that player, at the node and along the direction of the most recent pellet that really hit. Pellets before the first hit are judged normally. The server applies each report (it does not re-test client-reported hits).
- Example: 8 pellets, pellet 3 is the only real hit (torso, 500 units). Pellets 3–8 all count: 6 × 9 = 54 damage. With the variable at 0: 9.
- **Corrected:** these numbers hold only when the weapon's `FireDelay` is 0. With `FireDelay` > 0 (30 when the record omits it), the server drops the shot's **last** pellet report (`weapon_firing.md` Corrections 3). The same example then gives 5 × 9 = 45. If only pellet 8 hits, it gives 0. See Corrections 1.

#### 5.8 Double barrel (`Guntype` 13)

- Left mouse fires one barrel (`fire_0` 800 ms, 8 pellets × 14). Right mouse is alt-fire, not zoom: `alt_fire_0` (800 ms) carries two fire keys at 0 and 67 ms, so both barrels fire 67 ms apart, each a full 8-pellet shot. With one round left, right mouse fires a normal single shot.
- Reload: `reload` (3000 ms) from empty; `reload_1` (3033 ms) when one round is left.
- Cone: $g = 1.3$ gives $n'_{min} = 64$, $n'_{max} = 129$. The second barrel of an alt-fire sees the first barrel's firing growth (0.225): $n = 64 + \lfloor 65 \times 0.225 \rfloor \times 1.3 = 82$.

### 6. Sniper rifles (`Guntype` 6)

#### 6.1 Unscoped and scoped accuracy

- **Unscoped**: `MinPerturb` = `MaxPerturb` = 100, so $n = 100$ whatever the meters are: every round lands uniformly within ±0.1 on each axis (±5.71°). There is no crosshair (`DisableCrosshair` 1). This is the "no-scope" shot.
- **Scoped** (L96A1): cone 4..60 with zoomed meter speeds 1.0 / 0.5.
  - Standing still: $n = 4$ (±0.23°).
  - Moving: all movement targets are 1.0 and $\mu = 1$ at full scoped speed, so the movement meter rises 0.0375 per sub-step to 1.0 (27 sub-steps, 0.34 s) and the cone reaches 60. Stopping, it falls by $\min(0.1125, M) \times 0.5$ per sub-step: 19 sub-steps (0.24 s) to get back to $n = 4$.
  - Each shot adds 0.0375 to the firing meter, which then **does not recover for 2400 ms** (`DecDelay`). Consecutive scoped shots within 2.4 s: 4, 6, 8, 10, 12, 14. Dragunov (2..30): 2, 3, 4, 5, 6, 7.
- Zoomed accuracy starts on the frame zoom is pressed (section 7.2), not when the FOV arrives.

#### 6.2 Bolt action (`ReloadAfterFire` 1: L96A1, M24, ASW338LM)

- The bolt cycle is the 1400 ms fire animation (L96A1; bolt sound at 840 ms). `RapidFireCount` 1: one round per press.
- **After a zoomed shot** the zoom steps once more after 300 ms (section 7.6), which leaves the scope on a one-level scope.
- **No zooming in during the bolt**: while the fire animation plays and the weapon is not zoomed, the zoom key is ignored. The weapon leaves the firing state on the first update after the fire animation ends, and only then can it zoom again. Pressing zoom while still zoomed (before the 300 ms) zooms out normally.
- **What interrupts the bolt**: nothing shortens it. Fire is ignored until it ends. A reload request is queued until it ends (`inventory.md`, section 7). Sprint lets it finish. Switching weapons abandons it, but the draw that follows (1100 ms for the L96A1, plus the other weapon's draw) is longer than the remaining bolt, so switching never saves time.
- The automatic reload after the last round waits for the bolt animation to end (`weapon_firing.md`). By then the scope has already left, so the reload starts at about 1400 ms.

#### 6.3 Semi-auto and automatic snipers

- Dragunov_SVD (`ReloadAfterFire` 0, `RapidFireCount` 1): stays zoomed after a shot; one round per press, at most one per 400 ms. PSG1 and M107CQ are `RapidFireCount` 0 (automatic while held).

#### 6.4 Recoil

- One kick per shot that equals the cap: L96A1 `BaseCamRecoilPitch` = `MaxCamRecoilPitch` = 5°, so the view jumps 5.000° up. A second shot with no recovery adds 2.500°, a third 1.250°.
- `MaxRandomPitch` 0, so the side wag happens on **every** shot: 1.0° (q = 1 on a fresh shot) in the current direction. `MaxRandomwag` 0 lets the direction flip after every shot (50 %).
- `JitterDecDelay` 2400 ms: the slow return rate (0.1 rad/s × the correction factor) lasts 2.4 s after each kick.

### 7. Zoom and scope (any weapon whose scope attachment has `ZoomLevel` > 0)

#### 7.1 Which weapons can zoom

- The current scope attachment is the one fitted (attachments spec) or, by default, the weapon's `DefaultScopeAttach`. No scope attachment, or `ZoomLevel` 0: the zoom key does nothing.
- On knives, the double barrel, the snowball gun and the crossbow the same key is alt-fire (`inventory.md`).

#### 7.2 Entering and stepping (key, toggle)

- The zoom command (default right mouse, profile binding `ZoomIn`) acts on the **press** only. Holding or releasing does nothing. It is a toggle that cycles $0 \to 1 \to \dots \to L_{max} \to 0$.
- A press is **ignored** (dropped, not queued) when any of these hold: a zoom transition is running; the weapon is reloading (magazine or shell); a `ReloadAfterFire` weapon is in its fire animation and not zoomed; weapons are disabled (e.g. a cinematic camera).
- On an accepted press from level $L_{old}$ to $L_{new} > L_{old}$:
  - $L \leftarrow L_{new}$ at once. The mouse scale (7.7), the zoomed spread values and the scoped speed limit all switch on now, because $Z$ is true from this moment.
  - If $L_{old} = 0$: the scope overlay appears and the view model is hidden, at once (7.10).
  - The zoom-in sound plays and the FOV transition starts.
- On an accepted press from $L_{max}$: $L \leftarrow 0$, the zoom-out sound plays and the zoom-out transition starts. The overlay stays until the zoom-out ends.
- Zooming is allowed while drawing, firing (non-bolt), sprinting, crouching and in the air.

#### 7.3 Field of view per level

```math
\phi_L = \texttt{ZoomFov}_{L-1} \ (\text{degrees, } L \ge 1), \qquad \phi_0 = \texttt{FovX}, \qquad \phi_y = \phi_x \cdot \frac{\texttt{FovY}}{\texttt{FovX}}
```

The vertical FOV is scaled in proportion to the angle (not through the tangent). With 90/75: 20° → 16.67°, 30° → 25°, 10° → 8.33°, 50° → 41.67°, 75° → 62.5°.

#### 7.4 Transition speed and time to scope in

- The FOV moves at a constant angular speed each frame, $\phi \leftarrow \phi \mp v\,\Delta t$, and is clamped to the target on the frame it reaches or passes it. That frame ends the transition.

```math
v_{in} = \frac{\texttt{FovX} - \phi_{L}}{\texttt{ZoomTime}}, \qquad v_{out} = \frac{\texttt{FovX} - 20^\circ}{\texttt{ZoomTime}}
```

- Zoom-in speed is set by the **target** level's distance from the normal FOV, so going from normal to any level takes exactly `ZoomTime`. Stepping from level 1 to level 2 is quicker: $(\phi_1 - \phi_2)/v_{in}$.
- Zoom-out speed always assumes a 20° scope (`zoom_out_reference`). Leaving 20° takes `ZoomTime`; leaving 10° takes 8/7 of it; leaving 50° takes 4/7.
- L96A1: in 0.18 s at 388.9°/s; out 0.18 s. Dragunov: 0.10 s each way. M107CQ (Barrett Scope, 0.16 s): to 30° in 0.16 s (375°/s); 30° → 10° in 0.04 s (500°/s); 10° → 90° in 0.1829 s (437.5°/s).
- Frame-quantised: the last step is clamped, so the transition ends on the first frame at or past the target. L96A1 at 60 fps: 11 frames (183.3 ms).

#### 7.5 Leaving zoom

- **Toggle**: the last press of the cycle (above).
- **Switching weapons, dying, an external or cinematic camera**: instant. $L \leftarrow 0$, FOV set to normal on that frame, overlay removed, view model shown, no transition and no sound. A pending bolt zoom-out is cancelled.
- **Reload request while zoomed** (key or the automatic reload at clip 0):
  - If a transition is running, the request is ignored (the automatic reload repeats each frame, so it happens later).
  - If the reload would be allowed, then for a non-bolt weapon the zoom goes straight to 0 (zoom-out transition). For a `ReloadAfterFire` weapon the 300 ms bolt zoom-out is armed (if not armed already) instead.
  - Either way the reload is marked "after zoom" and starts on the frame a zoom-out transition completes.
  - If the reload would be refused (full clip, or clip = total), nothing happens and the zoom stays.
- **Firing**: only `ReloadAfterFire` weapons leave zoom after a shot (7.6).

#### 7.6 Automatic zoom-out after a bolt shot

- Arm: on the frame a `ReloadAfterFire` weapon fires while the last zoom transition was a zoom-in (zoomed in, or still zooming in), store the millisecond clock.
- Trigger: on every later frame where it is armed, $Z$ holds and more than 300 ms have passed, issue one **zoom press** (7.2). The press is ignored while a transition runs, so it retries each frame.
- Disarm: when a zoom-out transition starts, or on a weapon switch.
- Because the trigger is a normal press, a **two-level scope at level 1 first steps to level 2** and only then, on the next accepted press, zooms out (Q3). One-level scopes and level 2 leave directly.

#### 7.7 Mouse sensitivity while zoomed

```math
\Delta\text{yaw} = \frac{0.5\,m_x}{1 + 3L}, \qquad \Delta\text{pitch} = \frac{0.5\,m_y}{1 + 3L}
```

- $m$ is the raw mouse delta after the player's sensitivity setting. Relative to unzoomed: level 1 × 1/4, level 2 × 1/7.
- $L$ changes on the press, so the scale is applied for the whole zoom-in transition and dropped for the whole zoom-out transition.
- The scale does not follow the actual FOV ratio (90 → 20 is 4.5× narrower, but turning is only 4× slower).

#### 7.8 Sway

- Every frame where $Z$ holds, before mouse look, the view angles are nudged:

```math
\text{pitch} \mathrel{+}= \frac{Y_s}{1000}\,\sin\!\left(\frac{t\,Y_f}{10}\right)\Delta t, \qquad \text{yaw} \mathrel{+}= \frac{X_s}{1000}\,\sin\!\left(\frac{t\,X_f}{10}\right)\Delta t
```

  Here $t$ is game time in seconds, $\Delta t$ the frame time, and $X_s, Y_s, X_f, Y_f$ are `CameraSwayXSpeed`, `CameraSwayYSpeed`, `CameraSwayXFreq` and `CameraSwayYFreq` plus any attachment additions.
- The nudge moves the real aim (it is integrated into the view angles), so the aim point drifts and the player corrects with the mouse. Zooming out does not undo the drift. The phase is global game time, not time since zooming.
- Integrated, the drift has amplitude $S/(1000\,\omega)$ rad with $\omega = F/10$ rad/s. L96A1: yaw ±0.258°, pitch ±0.063°, period 6.28 s. Tiny next to the unscoped cone, noticeable against a 20° view.
- No breathing or hold-breath key: the hold-breath command exists in the client but has no binding or menu entry in this build. The **breath perk**, for sniper-class (`Guntype` 6) weapons only, suppresses sway while it is active and stamina is above 0 (perks spec).
- Sway is skipped in the chase death camera.

#### 7.9 Scoped movement speed

- While $Z$ holds, the speed limit is walk speed (70 u/s) with the normal carry penalty, even when running or sprinting (`movement.md`). L96A1 with the reference vest: 46.2 u/s.
- `ZoomedMovePenalty` is read but never used.

#### 7.10 Scope overlay

- One picture, the scope's `ZoomSpriteName`, drawn in front of the world with no depth test, no fog and no lighting, centred on the view. The picture itself holds the reticle and the surround. There is no separate drawn reticle.
- By role:
  - the surround (black, opaque) around the lens;
  - the reticle, centred exactly on the aim (the ray's unperturbed direction);
  - for scopes with a range readout, the number at the HUD position of `hud_crosshair.md` A9 (not part of the picture; see "Range readout" below).
- While the overlay is up: the view model is hidden and the HUD crosshair is not drawn (`hud_crosshair.md`). The rest of the HUD (health, ammo, kill feed) stays and draws over the picture.
- Lifetime: from the press that leaves level 0 until the zoom-out transition completes (or an instant reset). Stepping between levels keeps it, at the same size (the size does not depend on the zoom FOV).
- Model level of detail is raised as the FOV narrows (offset $-5 \times (\texttt{FovX} - \phi)/(\texttt{FovX} - 2^\circ)$), presentation only.

**Size and placement at any resolution (2026-10-28).** The picture is not a 2D HUD image in the 1024 x 768 frame of `ui_system.md`. It is a flat square-pixelled card drawn by the view-model camera (`view_models.md`: vertical FOV `PVModelFOV` 50°, aspect = the screen's width / height). So it keeps its own proportions at every aspect ratio and its size follows the screen **height** only:

- The picture file: `ZoomSpriteName` names a sprite file whose (first) frame is a texture of w_t x h_t texels (its full-size level). Stock scope and dot-sight pictures are 1024 x 1024.
- The scale: s = `ZoomSpriteScale` (the attribute reader's default is 0.001 when the key is missing).
- The card sits 1 unit straight ahead of the eye and spans ±w_t·s horizontally and ±h_t·s vertically. Projected through the 50° vertical FOV, on a W x H screen its size in pixels is

```math
w_{px} = \frac{w_t\, s}{\tan 25^\circ}\, H, \qquad h_{px} = \frac{h_t\, s}{\tan 25^\circ}\, H
```

  with tan 25° = 0.466308, centred on (W/2, H/2): left = W/2 − w_px/2, top = H/2 − h_px/2. One texel is s·H / tan 25° pixels in **both** axes. W does not enter the size: there is no reference resolution and no stretch to the screen's width.
- With the stock 1024 x 1024 picture and s = 0.001: the card is 2.19598·H pixels square, more than twice the screen height. Only its middle shows: the texel rows from 278.8 to 745.2 at every resolution, and the columns 512 ± 227.7·(W/H) (201.1 to 822.9 at 4:3; 97.5 to 926.5 at 16:9). Everything outside the screen is simply cut off.
- **Nothing fills the margins.** Where the picture's alpha is low (outside its opaque surround) or where the card does not reach, the world shows. There are no black bars and no stretched fill. In the stock sniper picture (for example `SR_Scope_Style_A`) the opaque black ring reaches out to texel column 173 on the left and 856 on the right of the 1024, and the lens opening runs about from 294 to 733 along the centre row. With s = 0.001 that ring covers the full width at 4:3 (its outer edge lands at x ≈ −46 and 1080 on 1024 x 768) but **not** at 16:9 or 16:10: at 1920 x 1080 the ring ends at x ≈ 175 and 1759, so a strip of world about 175 px wide shows on each side (Open question 4).
- **Focus blur** (only with the game's post-processing shaders on): after the world and the overlay are drawn, while zoomed, the whole frame is mixed with a blurred copy of itself through the scope's `FocusTex` picture. That picture is mapped once over the **whole screen** (stretched to W x H, so at 16:9 it is wider than it is drawn at 4:3); where its alpha is 1 the sharp frame shows, where it is 0 the blurred one. Stock focus pictures are 512 x 512 (some 256 x 256). The HUD is drawn after it, unblurred. The blur kernel itself is not specified here.
- **Range readout**: the HUD text of `hud_crosshair.md` A9, placed in the 1024 x 768 frame and stretched like every HUD item (x by W/1024, y by H/768). It does not move with the picture, which scales by H only. So at wide aspects the readout drifts left of the reticle relative to the picture: its right end is 22 px left of the centre at 1024 x 768 (22 px = 13.4 texels at s = 0.001), 41.25 px (17.8 texels) at 1920 x 1080 and 55 px (17.8 texels) at 2560 x 1440.

### 8. Launchers and other classes

- **Rocket launchers** (`Guntype` 9: LAW, RPG-7, M136_AT4): one-round clip (`ShotsPerClip` 1, 3 to 5 total), ammo `Type` 1 (projectile). Example: LAW projectile "Missile", 1300 u/s, no gravity, 10 s lifetime, area damage 40 in a 550-unit radius. M136_AT4 view model: fire 1333 ms, reload 2166 ms, draw 766 ms. They are recorded as "last primary" and use the shotgun-style server reload bookkeeping (5.6). Launchers carry zoomed fields and sway but no scope, so they never zoom. `ReloadAfterFire` on them does nothing without a scope. Projectile flight and explosion damage belong to a projectile spec.
- **M32** (`Guntype` 15): 6-round revolving grenade launcher, shell-by-shell reload (pre 2100, 1000 per round, post 1034 ms), ammo `Type` 2 (grenade, area damage 40 in 300 units).
- **Snowball gun** (14), **crossbow** (21): right mouse is alt-fire. A crossbow with an empty clip cannot alt-fire.
- Knives, grenades, mines, the bomb, sensors and kits: `inventory.md` and later specs.

## Per-tick order

One client frame, for the local player (extends `weapon_firing.md`'s order):

1. **Input events** (before the frame update): a zoom press runs 7.2 (level, overlay, sound, transition start); a reload press runs 7.5 or the normal reload start.
2. **Movement** (`movement.md`): the speed limit uses $Z$.
3. **Camera**:
   1. Sway (7.8), if $Z$.
   2. Mouse look with the $1 + 3L$ divisor (7.7).
   3. FOV transition step (7.4), if one is running. On completion of a zoom-out to level 0: remove the overlay, show the view model, then start a pending "after zoom" reload.
4. **Weapon**: `weapon_firing.md` steps 1–11 (recoil return, fire request, gates, animation state machine, which also covers the shell loop, minigun spin and pump or bolt length; then ray(s) with zoomed values if $Z$, spread sub-steps with zoomed speeds if $Z$, auto-reload through 7.5 when zoomed, kick).
5. **Bolt zoom-out** (7.6): arm if this frame's shot qualified; trigger if armed, $Z$ and more than 300 ms have passed.

## Edge cases

- **Firing during the zoom-in transition**: allowed (non-bolt weapons, and bolt weapons when not mid-bolt). The shot uses zoomed values. A bolt shot fired while zooming in arms the 300 ms zoom-out. The zoom-out press is ignored until the zoom-in finishes, then accepted.
- **Fire while zooming out**: still zoomed values (until the transition ends).
- **Reload pressed during a transition**: ignored, not queued.
- **Zoom pressed during a magazine reload**: ignored. During a shell reload: ignored too (the weapon is in the reloading state).
- **Zoom pressed during the draw**: allowed by the zoom rules. The select animation keeps playing behind the overlay (see Open question 12).
- **Switching while zoomed or zooming**: instant reset (7.5). The new weapon's draw starts at normal FOV.
- **Bolt rifle, last round fired zoomed**: zoom-out starts at > 300 ms, ends 300 + 180 ms later (L96A1); the automatic reload starts when the 1400 ms fire animation ends.
- **Two-level bolt scope at level 1**: shot → 300 ms → zoom in to level 2 → zoom out (Q3).
- **Reload press while scoped with a full clip**: refused; the zoom is untouched.
- **Shotgun fire with clip 0 during a shell reload**: not possible (fire request off). The first shell must finish.
- **Shotgun reload with total = clip + 1**: one loop, then post-reload.
- **Partial minigun spin**: release at 650 ms of spin-up → spin-down starts at 666 ms of its 1333, ending 667 ms later. Press again 300 ms into that (at 966 ms of the spin-down) → spin-up starts at $\lfloor 1300 \times (1 - 966/1333) \rfloor = 357$ ms, first round 943 ms later.
- **Pistol tap shorter than the fire animation**: lost (section 4).
- **Death or external camera while zoomed**: instant reset.
- **Spectating in first person**: zoom level and overlay follow the spectated player's reported zoom state and scope (not specified further here).

## Quirks

- **Q1, pellet hit carry-over** (5.7): once one pellet hits a player, all later pellets of that shot count as hits. It makes shotguns far more lethal than their spread suggests, and players rely on it. Keep it as an option, on by default (matching the client default), and record it in the implementation notes.
- **Q2, quick-scope**: zoomed accuracy, the mouse scale and the zoomed speed limit switch on the frame zoom is pressed, before the FOV narrows. Firing on the frame of the press gets the 4..60 scoped cone instead of the fixed 100. Keep.
- **Q3, bolt zoom-out is a zoom press**: on two-level scopes it steps in before it steps out. Keep.
- **Q4, `ZoomedMovePenalty` unused**: scoped speed is walk speed with the normal penalty. Keep (ignore the field).
- **Q5, zoom-out speed assumes 20°**: wide sights (50°, 75°) leave faster than they enter; narrow ones (10°) leave slower. Keep.
- **Q6, sensitivity by level, not by FOV**: 1/4 and 1/7 whatever the scope's FOV. A 75° dot sight still quarters the mouse. Keep.
- **Q7, float truncation in the cone** (shared rules): $50 \times 1.3 \to 64$. Keep, by taking $g$ as a 32-bit float and truncating the double-precision product.
- **Q8, Desert Eagle automatic** (`RapidFireCount` 0). Keep as data.
- **Q9, sway integrates into aim**: it is a slow drift the player fights, not an oscillation around the aim. Keep.
- **Q10, stale "reload after zoom"**: if a weapon switch resets the zoom while a reload is waiting for a zoom-out, the waiting flag is not cleared. The next completed zoom-out (on any weapon) issues a reload request. Minor. Recommendation: clear it on reset, and note the difference.
- **Q11, no-scope**: an unscoped sniper is not "inaccurate by movement"; it is a fixed ±5.71° square even standing still, and has no crosshair. Keep.

## Corrections (2026-10-19)

From a second pass over the reference's server shot handling, its reload bookkeeping and the client's fire state machine, compared with this spec and with our implementation. Test cases 37–41.

1. **Multi-pellet shots lose their last pellet when `FireDelay` > 0.**
   - Reference: the server checks the spacing on a shot's last ray only. The earlier rays have already stored the shot's own timestamp, so with `FireDelay` > 0 that last ray is always dropped (`weapon_firing.md` Corrections 3). This applies to every pellet weapon (shotguns, the double barrel's two volleys, each a separate shot) and to the carried-over reports of 5.7: the last report of each shot is the one lost.
   - Ours: 5.3, 5.7 and test cases 22–23 counted all pellets. `after_shots` checks once per round and then delivers every pellet (`games/combat_arms/src/game/weapons.rs:1795-1806`, `1831-1845`). Our Remington 870 document has no `fire_delay`, so it uses the default 30 ms (`games/combat_arms/data/weapons/remington_870.toml`; `weapons.rs:342-344`).
   - Why it matters: the shotgun's close-range damage drops by one pellet in eight (72 → 63), and a single lucky last pellet scores nothing.

2. **The server's reload mark is shorter than the reload, and refills the server clip at its end.**
   - Reference: see the corrected 5.6. Mark length = third-person reload length (by stance) ÷ ($\min(r, 2.0)\cdot 1.2$). The server's rate is capped at 2.0, unlike the client's "above 2.0 plays at 1.0". Shots during the mark are dropped without spending server ammo. The server clip is filled when the mark ends, not when the reload starts.
   - Ours: one simulation, no mark (`games/combat_arms/src/game/weapons/classes.rs:46-47`, known shortcut). The spec said the mark lasts the reload animation.
   - Why it matters: only with lag or a modified client. It is listed so that a split server keeps the right window: with stock rates the mark ends at 1/1.2 of the third-person reload, before the client can fire again.

3. **First-round latency by class.**
   - Reference (`weapon_firing.md` Corrections 10): a press from idle spends its first round on the 3rd weapon update when the model has a pre-fire animation (rifles, SMGs, MGs, pistols: their 0 ms pre-fires count), on the 2nd for primary shotguns (their pre-fire is always skipped). The minigun's spin-up is its pre-fire: its first round comes on the update after the one that sees the 1300 ms spin-up done, plus the one update that only sets the firing state.
   - Ours: the trigger fires on the press tick (`classes.rs:278-282` for the minigun waits only for the spin-up time).
   - Why it matters: pump shotguns answer one frame faster than rifles and pistols. A pistol tap fires 50 ms after the press at 60 fps.

## Corrections (2026-10-28)

From the reference's overlay creation, the engine's sprite drawing for objects in the view-model ("really close") pass, the post-processing pass and the install's scope pictures (dimensions and alpha only), after a playtest report that our scopes do not fill the screen.

4. **The scope overlay scales with the screen height, keeps its proportions, and has no black fill.**
   - Reference: see the corrected 7.10. The picture is a card 1 unit ahead in the view-model projection (50° vertical FOV, screen aspect), half-size `ZoomSpriteScale` x texture size, so it is H·w_t·s / tan 25° pixels wide and H·h_t·s / tan 25° tall, centred, cropped by the screen; the world shows wherever the picture is transparent or absent.
   - Ours: `games/combat_arms/src/ui/scope.rs` hands the picture to the shell's overlay, which draws it centred at its own aspect, filling the view's height, between black bars; `ZoomSpriteScale` is not applied.
   - Why it matters: with the stock picture and s = 0.001 the game's lens fills about 94 % of the screen height and the black ring reaches past the screen's top, bottom and (at 4:3) sides; ours shows the whole picture, ring and all, at less than half that size, with black bars instead of the world at the sides.

## Corrections (2026-10-30)

From the client weapon's fire-animation choice and its pre-fire lookup, the shared animation-rate calculation and the client's zoom command. Settles `view_models.md` contradictions C1 and C2 in favour of `view_models.md`.

1. **Pistols play no pre-fire** (replaces "pistols: their 0 ms pre-fires count" in Corrections (2026-10-28) 3 and case 40's M92FS number).
   - Reference: the pre-fire animation is looked up only for weapons outside the secondary (2) and melee (3) classes, and skipped for shop items of the primary-shotgun type; for those the press goes straight to `fire_0`. Every other weapon plays its model's `prefire` if the file has one. A pistol press from idle therefore fires on the **2nd** weapon update (as a primary shotgun), a rifle with a 0 ms `prefire` on the 3rd.
   - A **non-zero** pre-fire (on any weapon that plays one, not only the minigun) delays the first round by its full length: `fire_0` starts on the update that sees `prefire` done (`animation.md`'s done-needs-L+1 rule), and its key at 0 ms is consumed on the next update.
   - Ours: the M92FS's latency 3 (as the old case 40) and one update for any non-spinning pre-fire.
   - Expect: test cases 43 and 44.
2. **Special weapons zoom like guns.** There is no class rule for zoom: a special weapon zooms exactly when its scope slot holds a scope part with zoom levels (its `DefaultScopeAttach`, e.g. `XM25 Scope`, `LGB_scope`), with that part's levels, FOVs, overlay and `ZoomTime` (`attachments.md` "Zoom"), refused while reloading or (unzoomed) while firing a `ReloadAfterFire` weapon, the same as for a rifle. A dot-sight-type part only toggles dot-sight mode.

## Follow-up (2026-11-14): zoom sounds and the focus blur

From the client's zoom command and zoom reset, its post-processing pass, the install's full-screen blur shader file (`shaders/fullscreen/gblur.fx`, read in place) and the Steam install's modification file (`attributes/mod.txt`, build 26236938) and archive directories. Where 7.2, 7.5 or 7.10 disagree, this section wins. Confidence is given per item; "(inferred)" marks what was not read directly. Test cases 45–51.

**1. Zoom sounds: which records name them** (high).

- The sounds belong to the **scope part** (the slot-1 modification record, `[Mod#]` in `attributes/mod.txt`), keys `ZoomInSound` and `ZoomOutSound`. Weapon records have no zoom-sound key.
- Every part with `ZoomLevel` > 0 in the Steam data names both keys, and always the same file for in and out:

| Parts | `ZoomInSound` = `ZoomOutSound` |
|---|---|
| 72 parts: every sniper scope, every built-in rifle and SMG sight, every red-dot, holo, ITECH and HDS-4 sight, `XM25 Scope`, `NEMEXIS_Scope` | `GUNS_SND_SR\M24\zoom.wav` |
| 2 parts: `LGB_scope`, `napalm_scope` (air-strike designators) | `GUNS_SND_ETC\MOAB\Zoom.wav` |

- So in the stock game every zoom step, in or out, on every weapon, plays the same M24 click; only the two air-strike designators differ. The archives also hold about 30 per-weapon `guns_snd_sr/<weapon>/zoom.wav` files (Dragunov, L96A1, M200, TRG-21 and others). No attribute record names them, so the zoom command never plays them (Open question 14).
- An empty key plays nothing; no other file is tried.

**2. Zoom sounds: when they play** (high, except where marked).

- **On every accepted press** (7.2), at the moment of the press, as the FOV transition starts: the zoom-in sound when the level goes up (0 → 1, and 1 → 2 on two-level scopes), the zoom-out sound when it goes back to 0. A refused press plays nothing.
- **On an instant reset while zoomed** (7.5: weapon switch, death and the other player-mode clears, an external or cinematic camera): the zoom-out sound **does** play. This corrects 7.5's "no sound". The reset is silent only when the level was already 0 and no transition ran. The record used is the scope part the client's player statistics hold at that moment (inferred: normally the part of the weapon being put away, since the reset runs from the weapon-change notice; not traced to the frame).
- **Automatic zoom-outs** (7.6 bolt, 7.5 reload) are ordinary presses, so they play the zoom-out sound (and the intermediate step of a two-level scope plays the zoom-in sound).
- **Spectating in first person**: the spectated player's scope part is used for the press and for the sound.
- **How**: a local sound (2D, not positioned, no distance falloff), priority "misc medium", default volume. Only the zooming player hears it (and a first-person spectator of them, whose own client runs the same steps). Other players never hear anyone's zoom.

**3. The focus blur** (`FocusTex`; high for the rule, the order and the kernel).

While zoomed, the frame is mixed between itself and a blurred copy through a mask picture: the lens stays sharp and the rest of the screen is blurred.

- **When**: on every frame on which the local player is zoomed in the sense of $Z$ (level > 0 **or** a zoom transition running, so also during the zoom-out transition) **and** the game's post-processing shaders run. That needs the `Shader` option on (`options.md`; default off) on a machine whose pixel and vertex shader versions pass the client's minimum (inferred, medium: `Shader` is the switch the post-process tests). With shaders off there is no blur at all, only the overlay of 7.10.
- **The mask**: the `FocusTex` of the local weapon's current scope part, looked up every frame. If the current weapon has no scope part, the previous mask stays (at start-up `shaders/image/ScopeFocus.dtx`). Every zoomable stock part names one (list below). Only the mask's **alpha** is used.
- **Order in the frame**: after the world, the view model and the scope overlay card of 7.10 are rendered, and before the 2D HUD. The HUD is never blurred.
- **Steps** ($W \times H$ the screen in pixels; texture coordinates $u, v \in [0, 1]$ over the whole screen; all sampling bilinear and clamped at the edges):
  1. **Scene copy.** The back buffer is copied, then passed through the game's saturation pass into a full-size scene image $S$. That pass moves each pixel halfway towards its grey value: per channel $S = \tfrac12 c + \tfrac12 (0.3 r + 0.59 g + 0.11 b)$. (Inferred, medium: the shader fixes its mix factor at 0.5 as a constant, so the client's attempt to set it from the `PureSaturate` console value, default 1.0, should not take effect. If it did, $S$ is the frame unchanged. Open question 15.) With bloom on, $S$ is the bloomed scene instead (bloom is not specified here).
  2. **Downsample** into an image $D$ of $\lfloor W/4\rfloor \times \lfloor H/4\rfloor$ pixels. Each pixel is the mean of four samples of $S$ at $(u \pm 1/W,\ v \pm 1/H)$ around its own centre: one **full-screen** pixel diagonally, not one small pixel.
  3. **Vertical blur** of $D$, same size: $B_1(u,v) = \tfrac12 D(u,v) + \tfrac14 D(u, v + 8/H) + \tfrac14 D(u, v - 8/H)$.
  4. **Horizontal blur** of $B_1$, same size: $B(u,v) = \tfrac12 B_1(u,v) + \tfrac14 B_1(u + 8/W, v) + \tfrac14 B_1(u - 8/W, v)$. The tap distance is 8 full-screen pixels, i.e. 2 pixels of the quarter-size image. One pass each way, no repeats.
  5. **Merge** over the whole screen: with $a$ the mask's alpha at $(u, v)$ (the mask stretched once over the full $W \times H$, whatever its own size or the screen's aspect), the output pixel is $a\,S + (1 - a)\,B$, with $B$ sampled bilinearly from the quarter-size image. Alpha 1 = sharp, alpha 0 = blurred.
- **Parameters**: none in the data besides the mask. The weights 0.5 / 0.25 / 0.25, the 8-pixel tap distance, the quarter-size images and the 4-tap downsample are fixed. There is no time ramp: the blur is at full strength on the frame of the press and gone on the frame the zoom-out transition ends (or on an instant reset).
- **Stock masks** (`FocusTex` in `UI_HUD_Weapon\Scopes\DTX\`, number of parts in brackets): `DotSight_Focus_A` (23: red-dot A and B grades, `AR Dot`, `INSAS_Scope`, `MP5_SD_DOT_B/C`, `DotSight_HKG38_AR`), `DotSight_Focus_C` (11: HDS-4 grades, `pp19mod Scope`), `DotSight_Focus_B` (10: ITECH grades, `ITECH_CQB`), `DotSight_Focus_D` (9: holo sights), `SR_Focus_Dragunov_01` (5: `Dragunov Scope`, `L115 Scope`, `M200 Scope`, `XM25 Scope`, `Barrett Scope`), `SR_Scope_Focus_A` (3: `Sniper Scope`, `PSG Scope`, `Sniper_Scope_EagleEye_FAMAS`), `AR_Focus_G36_01` (3: the two G36 sights, `Scope_M4A1_AdvancedEra`), `AirStrike_Focus` (2: `LGB_scope`, `napalm_scope`), and one part each: `AR_Focus_AUG_01`, `AR_Focus_F2000`, `AR_Focus_L85A1MOD`, `SMG_Focus_P90_01`, `SMG_Focus_SR2_Veresk`, `SR_Focus_m1866`, `SR_Focus_BlackDragon`, `PVE_Robot_SR_focus` (`NEMEXIS_Scope`).
- **Quirk** (inferred, medium): the mask comes from the **local** weapon's scope part, while $Z$ follows the spectated player's zoom when spectating in first person. A spectator whose own weapon has a different scope (or none) sees the spectated zoom blurred through their own (or the last used) mask.

**4. Stock `ZoomSpriteScale`** (answers Open question 4 (a); high): 71 of the 74 zooming parts carry 0.001; `LGB_scope`, `napalm_scope` and `NEMEXIS_Scope` carry 0.00061.

## Test cases

Data as in "Typical weapons". Reference character for speeds. "Exact time" means the fire cycle, transitions and spread run on exact time (`weapon_firing.md` Q2 recommendation); frame-quantised cases say so. Unarmoured male victim, damage roll 1.0, unless stated.

1. **L96A1 zoom-in FOV**: L = 0, FovX 90, FovY 75. Press zoom at t = 0 (exact time) → FOV 55.0° at t = 0.09 s; 20.0° at 0.18 s, transition over. FovY 45.83° at 0.09 s, 16.67° at the end. Overlay visible and view model hidden from t = 0.
2. **Frame-quantised zoom-in**: same at 60 fps → FOV after frame k = 90 − 6.4815 k for k = 1..10 (25.19° after frame 10), 20° after frame 11 (183.3 ms).
3. **Zoom out**: L96A1 at L = 1, press at t = 0 → L = 0 at once; FOV 55° at 0.09 s, 90° at 0.18 s; overlay removed at 0.18 s.
4. **Two-level scope**: M107CQ (Barrett Scope, `ZoomTime` 0.16). Press at 0 → 30° at 0.16 s. Press at 0.5 s → 10° at 0.54 s. Press at 1.0 s → 90° at 1.1829 s.
5. **Press during a transition**: L96A1, press at 0 and again at 0.1 s → the second press is ignored; at 0.18 s L = 1 and FOV 20°.
6. **Dot sight speeds**: a 50° sight on a weapon with `ZoomTime` 0.15 → in 0.15 s; out 0.0857 s. A 75° sight → in 0.15 s; out 0.0321 s.
7. **Mouse scale**: a mouse delta that turns the view 1.000° at L = 0 turns it 0.250° at L = 1 and 0.143° at L = 2. During a zoom-in from 0 to 1 (FOV still 80°) it is already 0.250°. During the zoom-out it is already 1.000°.
8. **No-scope cone**: L96A1, L = 0, any meter values → n = 100. With draws u₁ = 1, u₂ = 0 and forward (0,0,1) → direction ∝ (0.1, −0.1, 1).
9. **Quick-scope cone**: L96A1 standing still, meters 0, press zoom and fire on the same frame → n = 4 (FOV still 90°).
10. **Scoped while moving**: L96A1 zoomed, walking at full scoped speed (46.2 u/s, μ = 1) for 27 sub-steps from M = 0 → M = 1.0, n = 60. Stop: n back to 4 after 19 sub-steps.
11. **Scoped follow-ups**: L96A1 zoomed, standing, shots within 2400 ms of each other (re-scoped each time) → n = 4, 6, 8, 10, 12, 14. Dragunov zoomed (stays zoomed) → n = 2, 3, 4, 5, 6, 7.
12. **Scoped speed**: L96A1, reference character, holding run → 188.1 u/s unzoomed; press zoom → limit 46.2 u/s from that frame.
13. **Bolt cycle and auto zoom-out**: L96A1 at L = 1 (transition done), fire at t = 0 → one round; zoom-out starts on the first frame after t = 300 ms and ends 180 ms later; FOV 90° by t ≈ 480 ms. Zoom press at t = 1000 ms → ignored. Zoom press at t = 1400 ms + one update → accepted. Fire held from t = 0 to 3000 ms → still one round.
14. **Bolt with two-level scope**: ASW338LM (`ZoomTime` 0.15) at L = 1 (30°), fire at t = 0 → at t > 300 ms L = 2, FOV 30° → 10° in 0.0375 s; then on the next frame L = 0 and FOV 10° → 90° in 0.1714 s.
15. **Semi sniper stays zoomed**: Dragunov at L = 1, fire at t = 0 → still L = 1 at t = 1000 ms. Second press at t = 200 ms and held → second round at t = 400 ms.
16. **Reload while scoped (semi)**: Dragunov at L = 1, clip 5, total 40, press reload at t = 0 → zoom-out 0..0.1 s; reload animation from 0.1 s; clip 10 at 2.7 s.
17. **Reload while scoped (bolt)**: L96A1 at L = 1, clip 3, total 20, no recent shot, press reload at t = 0 → zoom-out from t > 300 ms to 480 ms; reload from 480 ms; clip 5 at 3380 ms.
18. **Reload refused while scoped**: L96A1 at L = 1, clip 5 of 20 → reload press does nothing; still L = 1.
19. **Switch while zoomed**: L96A1 at L = 1 (or mid-transition), switch to pistol → FOV 90° on that frame, no transition, overlay gone; the pistol draw (650 ms) starts.
20. **Sway drift**: L96A1 zoomed with no mouse input, exact integration from game time t = 0 to t = π s → yaw changes by +0.0090 rad (0.516°) and pitch by +0.0022 rad (0.126°, downward). From t = 0 to 2π s → net 0 on both.
21. **Shotgun cone is constant**: Remington_870, standing, running or airborne after a jump, shots 600 ms apart → n = 42 for every pellet of every shot.
22. **Pellet damage and falloff**: Remington_870, all 8 pellets on the torso, at d = 500 / 600 / 700 / 900 → 72 / 72 / 50.4 / 25.2. One head pellet and 7 torso at d = 500 → 117. Target at d = 1100 → 0 (rays end at 1000).
23. **Pellet carry-over**: Remington_870, 8 pellets, only pellet 3 geometrically hits (torso, d = 500) → with `AllowMultipleVectorsPerRoundBug` = 1: 6 reported hits, 54 damage; with 0: 1 hit, 9 damage. Only pellet 1 hits → 8 hits, 72. Only pellet 8 hits → 9.
24. **Shell reload timing**: Remington_870 clip 3, total 35, reload at t = 0 (rate 1, exact time) → clip 4 / 5 / 6 / 7 at t = 1033 / 1733 / 2433 / 3133 ms; idle at 4033 ms. Clip 0, total 7 → 7 shells, last at 5233 ms, idle at 6133 ms.
25. **Shell reload interrupted**: as case 24 (clip 3), fire pressed at t = 1500 ms → reload stops, clip 4 (the third loop is lost), one round fires immediately → clip 3, total 34. No post-reload plays.
26. **Empty pump gun, trigger held**: Remington_870 clip 0, total 35, fire held from t = 0 → the automatic reload loads one shell by 1033 ms, it fires at once (clip 0), the pump animation runs to 1633 ms, the reload restarts. Rounds at about t = 1033, 2666, 4299 ms.
27. **Pistol semi-auto**: M92FS (exact time). Press at 0 and hold → one round at 0, none after. Press at 0, release at 50, press at 100 and hold → second round at 180. Press at 0, release at 50, press at 100, release at 150 → no second round. Press at 0, release at 50, press at 250 → second round at 250 (plus one update for the zero-length pre-fire).
28. **Desert Eagle hold**: fire held from t = 0 with a full clip of 7 → rounds at 0, 600, …, 3600 ms (7 rounds), then the automatic reload.
29. **Pistol cones and kick**: M92FS standing still, all shots within `DecDelay` → n = 11, 14, 18, 23, 26, 30, 31, 31. View rise per shot with no recovery: 3.500°, 3.063°, 2.680°, 2.345°.
30. **SMG**: MP5A4 standing → n = 8, 10, 13, 16, 19, 21, 24, 24. Running at full speed, F = 0 → n = 17. Damage at d = 400 / 1000 / 2200 → 31.9 / 20.3 / 10.15. Exact-time fire cycle: 30 rounds by t = 2320 ms.
31. **LMG crouch**: M60 standing → n = 14, 19, 25, 31, 37, 37. Crouched and still → 14, 19, 25, 28, 28. Kick per shot with no recovery: 3.000°, 2.750°, 2.521°, 2.311°.
32. **Minigun spin-up**: Minigun_Special idle, fire held from t = 0 → first round at 1300 ms, then rounds every 34 ms or so (5–6 per 170 ms loop). Release at t = 650 ms during spin-up → no round; spin-down from 666 to 1333 ms (ends at t = 1317 ms).
33. **Double barrel float truncation**: Double_Barrel, meters 0 → n = 64 (not 65). Alt-fire with 2 rounds → two 8-pellet shots 67 ms apart; the second shot's n = 82. Alt-fire with 1 round → one normal shot.
34. **Sniper kick**: L96A1 first shot from rest → view up 5.000°, side 1.000° in the wag direction. A second shot with no recovery → up 2.500°.
35. **Damage**: L96A1 torso at d = 3000 / 4200 / 4800 → 160 / 112 / 56; head at d = 3000 → 960. Dragunov torso at d = 3000 → 100.
36. **No zoom without a scope**: M4A1 (no attachment), press zoom → nothing changes (L = 0, FOV 90°, normal cone and speed).
37. **Shotgun with spacing (Corrections 1)**: Remington_870, `FireDelay` 30, all 8 pellets on the torso at d = 500 → 63 (7 land), total −1. `FireDelay` 0 → 72. At d = 700 → 7 × 6.3 = 44.1.
38. **Carry-over with spacing (Corrections 1)**: Remington_870, `FireDelay` 30, carry-over on. Only pellet 3 really hits (torso, d = 500) → reports 3–8, the 8th dropped: 45. Only pellet 1 hits → 63. Only pellet 8 hits → 0. With the carry-over off and only pellet 3 → 9.
39. **Server reload mark (Corrections 2)**: magazine weapon whose third-person reload (standing) is 2400 ms, rate 1, `ReloadCheckTime` 0.8 → mark 2000 ms. A shot message stamped 1990 ms after the reload message → dropped, server ammo unchanged. At 2010 ms → applied, and the server clip was refilled at 2000 ms. With a rate bonus summing to 2.4 → the server uses 2.0: mark 2400 / 2.4 = 1000 ms.
40. **Latency by class (Corrections 3)**: 60 fps, press seen on update 0. M92FS → round on update 3 (50.0 ms). Remington_870 → update 2 (33.3 ms). Minigun_Special → spin-up started on update 1, seen done on the first update at or after 1300 ms of it, `fire_0` started on that update, first round on the next.
41. **Pump re-press (Corrections 3)**: Remington_870 held → the second round comes on the update after the one that sees the 600 ms `fire_0` done (no extra state update inside a held loop).

42. **Scope overlay size, 1024 x 768** (Corrections 4): stock 1024 x 1024 picture, `ZoomSpriteScale` 0.001 → card 1686.5 x 1686.5 px at (−331.3, −459.3); 1.6470 px per texel; visible texels x 201.1–822.9, y 278.8–745.2; the black ring covers both side edges (its outer edge at x ≈ −46.3 and 1080.2); no world strip; the range readout's right end at (490, 400), font height 20.
43. **Scope overlay size, 1920 x 1080**: same scope → card 2371.7 x 2371.7 px at (−225.8, −645.8); 2.3161 px per texel; visible texels x 97.5–926.5, y 278.8–745.2; the ring's outer edge at x ≈ 174.9 and 1759.0, world visible (not black) for x < 175 and x > 1759; the lens opening about x 455 to 1474; readout right end (918.75, 562.5), font height 28.125.
44. **Scope overlay size, 2560 x 1440**: card 3162.2 x 3162.2 px at (−301.1, −861.1); 3.0881 px per texel; visible texels as at 1920 x 1080; ring edge at x ≈ 233.1 and 2345.4, world visible outside; readout right end (1225, 750), font height 37.5.
45. **Scale and zoom independence**: the same scope with `ZoomSpriteScale` 0.002 at 1920 x 1080 → card 4743.3 px square (twice 43). Stepping the Barrett from 30° to 10° → card size unchanged. A 512 x 256 picture at s = 0.001, 1024 x 768 → 843.3 x 421.6 px, centred (texels stay square: no stretch at any aspect).
46. **No fill outside the picture**: a test picture 256 x 256, opaque, s = 0.001, 1920 x 1080 → card 592.9 px square centred at (960, 540); every pixel outside it shows the world (with the focus blur's mix where that pass is on), none is black from the overlay.
43. **Pistol latency (Corrections 2026-10-30, 1).** 60 fps, press seen on update 0: M92FS → round on update 2 (33.3 ms), the same as Remington_870; M4A1 (0 ms `prefire`) → update 3 (50.0 ms). Case 40's M92FS value is replaced by this.
44. **Long pre-fire (Corrections 2026-10-30, 1).** A rifle copy whose `prefire` is 200 ms at rate 1, 60 fps, press on update 0 → `prefire` from update 1, seen done on the first update at least 200 ms (+1 ms) later (update 14, 233.3 ms), `fire_0` started there, round on update 15 (250 ms).
45. **Zoom sounds, one level (Follow-up 2026-11-14, 1–2).** L96A1 (`Sniper Scope`): press → `GUNS_SND_SR\M24\zoom.wav` once, local, on the press frame; press again → the same file once more (zoom-out). A press refused during a reload → no sound.
46. **Zoom sounds, two levels.** M107CQ (`Barrett Scope`): presses 0 → 1 → 2 → 0 → zoom-in, zoom-in, zoom-out sounds, all `GUNS_SND_SR\M24\zoom.wav`. `LGB_scope` → `GUNS_SND_ETC\MOAB\Zoom.wav` both ways.
47. **Reset sound.** Zoomed at level 1 with the L96A1, switch weapon → FOV normal on that frame and the zoom-out sound plays once. Not zoomed, switch weapon → no sound.
48. **Who hears it.** Player A zooms; player B 100 u away → B hears nothing.
49. **Focus blur off.** `Shader` option off → zoomed frame has no blur (overlay only).
50. **Focus blur kernel.** `Shader` on, bloom off, 1024 x 768 → work images 256 x 192. (a) A uniform frame of colour (1, 0, 0): $S$ = (0.65, 0.15, 0.15) everywhere (step 1 with the fixed 0.5), $B = S$, so the output is (0.65, 0.15, 0.15) whatever the mask. (b) The 1-D kernel of steps 3–4 applied to a single bright quarter-image pixel of value 1 with neighbours 0 gives 0.5 at it and 0.25 at the pixels 2 to each side (taps at ±8 screen px = ±2 quarter pixels), 0 at ±1 quarter pixel. (c) Mask alpha 1 everywhere → output = $S$; alpha 0.5 → the mean of $S$ and $B$.
51. **Focus blur timing.** Zoom in with the L96A1 (0.18 s) → the blur is on from the press frame; zoom out → it stays until the frame the zoom-out transition ends, then is gone; no fade.

## Open questions

1. **Which build's data?** All numbers are the US reference data and view models; the Steam build may differ. Re-read the eight typical weapons and the scope records once the attribute reader works.
2. **Animation per weapon**: the view-model animation file was matched to each weapon by name (model table mapping not re-traced here). Confirm the M60, Minimi, R870, L96A1 and M92FS lengths in game: rounds per minute, pump and bolt time, shell reload time.
3. **Minigun loop keys**: the 170 ms loop has fire keys at both 0 and 170 ms. Whether a loop restart fires one round or two at the seam decides 5 or 6 rounds per loop (1765 or 2118 per minute). Count rounds spent in 10 s of holding.
4. **Scope overlay layout.** **Answered (2026-10-28)** from the reference: size, placement and margins are in 7.10 (scales with the screen height, square texels, centred, no fill). (a) **Answered (2026-11-14)**: the Steam data's zooming parts carry 0.001, except `LGB_scope`, `napalm_scope` and `NEMEXIS_Scope` at 0.00061 (Follow-up (2026-11-14) 4). Still open: (b) The user remembers the Steam build's scope "filling the screen, a bit stretched". The reference says it is not stretched and, at s = 0.001, leaves a world strip about 175 px wide each side at 1920 x 1080. Either the stock scale is at least 0.00123 (enough for the ring to reach the 16:9 edges), or the Steam build differs, or the memory is of the full-screen focus blur (which *is* stretched to the screen) or of 4:3 play. Check: a screenshot at 1920 x 1080 zoomed with the L96A1 (`Sniper Scope`): measure the lens width in pixels (≈ 1019 px wide at s = 0.001) and look at the side edges.
5. **Hold-breath command**: a hold-breath command exists in code with no binding or menu entry here. Check whether the Steam build exposes it.
6. **Pellet carry-over in the live game**: is `AllowMultipleVectorsPerRoundBug` still 1 in the Steam client, and does the live server still accept the repeated reports? Test: one pellet clipping a bot's edge at 500 units should do far more than 9.
7. **Two-level bolt zoom-out** (Q3): confirm on the ASW338LM that the scope steps to level 2 before leaving.
8. **Explosive damage**: launcher area damage (40 in 550 units for the LAW, 0 for the M136_AT4 in this data) looks too low or missing for a rocket. The explosion falloff and any server-side scaling belong to a projectile and explosion spec.
9. **Widescreen FOV**: `FovX`/`FovY` 90/75 suit 4:3. How the Steam build sets them on wide screens, and whether the zoom-out reference stays 20°, is unknown.
10. **Pistol rate in game**: confirm that a held re-press during `fire_0` fires on the frame the animation ends, and that the M92FS tops out near 333/min.
11. **Spectator zoom**: how a spectated player's zoom and scope are mirrored (the reference has code for it) belongs to a spectator spec.
12. **Zoom during the draw**: the zoom reset runs from the weapon-change notice. The reference notes that notice may be sent twice (at the start and the end of a switch). If it is sent again when the draw ends, a zoom started during the draw is cancelled then. Test: switch to the L96A1 and press zoom 200 ms into the 1100 ms draw.
13. **Cone arithmetic precision**: whether the reference client truncates the product $n \cdot g$ at double precision (64 for 50 × 1.3) or after rounding to a 32-bit float (65) depends on how it was compiled. Only factors whose products land within a rounding step of an integer differ (1.3 on the double barrel is the shipped case found). Check: the double barrel's crosshair gap at rest.
14. **Per-weapon zoom files** (Follow-up (2026-11-14) 1): about 30 `guns_snd_sr/<weapon>/zoom.wav` files exist in the archives but no attribute record names them. Whether a view-model animation key plays them (so a bolt sniper would sound its own click besides the scope's M24 click) was not traced. Check: zoom the M200 in game and listen for two different clicks; or read the M200 view model's animation keys.
15. **Saturation of the blurred frame** (Follow-up (2026-11-14) 3, step 1): the shader fixes its grey mix at 0.5, so while zoomed with `Shader` on the whole frame (lens included) should look half desaturated. Check: a screenshot zoomed and unzoomed with `Shader` on, bloom off, comparing a saturated colour inside the lens.
