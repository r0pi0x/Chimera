# Combat Arms: special weapons

Source basis: reference material the user pointed the spec session to: the server's projectile object and its subclasses (impact projectiles, land traps, the hornet, claymores and remote grenades, caltrops, the nerve confuse mine, the auto turret and its targeting on the owner's client, the first-aid kit, the air strike marker, the crossbow bolt and the explosive bolt), the server's explosion object (instant blast, cone blast, lasting sphere), the server weapon's fire path and its projectile creation, the server player object's fire message and death handling, the damage receiver's over-time damage slots, the client weapon's fire-key handling and per-ammo-type fire path, and the shared weapon and effect attribute managers. Data was read from the Steam install (build 25595979): the attribute archive's `WEAPONS.TXT` and `FX.TXT`, the first-person weapon models in the `GUNS_M_PV_LAUNCHER`, `GUNS_M_PV_MELEE`, `GUNS_M_PV_THROWING` and `GUNS_M_PV_ETC` archives (animation lengths and key times) and the world models of the projectiles and devices (animation lengths and boxes), all with throwaway readers that are not in the repo. Nothing was checked against a running game; see Open questions. Section 5.1 (swing sounds) was added on 2026-10-30 from the client weapon's fire step, the shared weapon-sound player, the client's ray and impact-effect path and the server weapon's fire handling.
Status: draft

Sections: Summary; Units and conventions; Constants; The classes earlier specs skipped; Where the numbers live; Behavior (1 rules every projectile shares, 2 rocket launchers, 3 grenade launchers, 4 the flamethrower, 5 melee weapons beyond the M9 (5.1 swing sounds), 6 mines and devices, 7 crossbows, 8 the snowball gun, 9 classes left to other specs); Stock values; Per-tick order; Edge cases; Quirks; Follow-up (2026-10-30): projectile looks and sounds in flight; Test cases; Open questions.

This spec continues `weapon_firing.md` (fire cycle, spread cone, recoil, hitscan, armour), `weapon_classes.md` (class flags, shell-by-shell reload, server reload bookkeeping), `grenades.md` (projectile flight, collision, blast falloff, cover, friendly fire, over-time damage), `attribute_data.md` (file format and field types), `inventory.md` (slots, switching, the knife), `view_models.md` (animation selection, keys) and `effects_sounds.md` (impact effects). Terms used without definition here are theirs.

---

## Summary

- **Every special weapon is a projectile weapon or a melee weapon.** There is no lock-on, no back-blast, no arming distance, no under-barrel launcher, no lunge and no fuel gauge in this build. What makes a weapon special is its ammo's `Type` (1 projectile, 2 grenade) and its projectile record's `Class` in the effects file: an impact projectile (rockets, launcher grenades, flame puffs, snowballs), a bouncing grenade (`grenades.md`), a land trap (mines, the hornet, the auto turret, the nerve mine), a claymore (claymores, remote grenades), caltrops, a first-aid kit, an air strike marker or a crossbow bolt.
- **Impact projectiles** leave the camera along the aim (with the weapon's spread) at the record's `Velocity`, fly straight (rockets, flames) or under the object gravity of `grenades.md` (launcher grenades), and detonate on the **first** thing they touch, or in the air when their `Lifetime` ends. A rocket goes 1300 u/s for 10 s; a launcher grenade 1550 u/s with no fuse. Their damage is the blast of `grenades.md` section 9 (linear falloff from a quarter of the radius, cover traces, players hit twice) from the ammo's `AreaDamage` and `AreaDamageRadius`; the direct hit adds nothing in the stock data. The LAW does up to 80 to a player, the RPG-7 86, an M32 grenade 80; the M136_AT4's ammo has `AreaDamage` 0 and does **no damage at all**.
- **The flamethrower** is a projectile stream: eight fire keys per 734 ms loop, each spending one of 300 rounds and launching a flame puff at 500 u/s that lives 1 s. Each puff leaves a 75 u burning sphere for 2 s that keeps a 12 HP/s burn entry alive on everyone inside; entries from different puffs stack. The shooter never burns from his own flames.
- **Melee weapons** are knives with other numbers: reach (`Range`, 105–160 u), damage per hit (ammo `InstDamage` for the left button, `Alt_InstDamage` for the right), and swing timing (the hit keys in `fire_0` and `alt_fire_0`). Heavy secondaries hit twice; the chainsaw's secondary hits seven times. Head hits are ×6 and rear hits ×1.3 as for bullets.
- **Mines and devices** are server objects that are thrown (600 u/s, gravity) or placed, **stick to the first floor they touch** (surface normal y ≥ 0.75) and then act: land mines and the hornet blow up when an enemy touches or stands over them, claymores and remote grenades when their owner presses the detonator (or dies), caltrops and the nerve mine damage enemies near them every frame, the auto turret shoots the enemy its owner's client picks, the first-aid kit heals teammates. All of them are removed silently when their owner leaves and at the end of every round. How many a player can have is only their ammo, except the auto turret and the nerve mine, of which a new one replaces the old.

## Units and conventions

- Distances in LithTech world units (u), +Y up, left-handed (+X right, +Z forward), as in the other Combat Arms specs. Velocity u/s, acceleration u/s².
- Time: projectile flight, blasts, devices and all damage run on the **server**, once per server frame; the shipped server configuration runs 20 frames per second (dt = 0.05 s, `grenades.md`). The fire cycle, reload and swing timings run on the **client**, per rendered frame, on animation time (`weapon_firing.md`); they are given in milliseconds at animation rate 1.
- "Blast" means the instant area damage of `grenades.md` section 9; "sphere" means its lasting over-time part (section 12 there). $m(d)$ is that section's falloff:

```math
r_0 = 0.25\,R,\qquad m(d) = \begin{cases} 1 & d \le r_0 \\ 1 - \dfrac{d - r_0}{R - r_0} & r_0 < d \le R \\ \text{no damage} & d > R \end{cases}
```

- "Applied twice" means the blast reaches both the player object and its hitbox object (`grenades.md` Quirk 3). It holds for the damage types bullet (2), burn (3), explode (7), claymore (38), infected explode (40) and EMP (46); every other area type reaches only the player object.
- Field names in `code font` are keys of the attribute data (`WEAPONS.TXT` records `Weapon#` and `Ammo#`; `FX.TXT` records `ProjectileFX#`, `ImpactFX#`, `Turret#`, `LandTrapClassData#`). Weapon, ammo, projectile and animation names are data names. Damage types are the game's numbers (`gear_perks.md`).

## Constants

Fixed in code. Everything else is data (next sections).

| Name (ours) | Value | Unit | Meaning |
|---|---|---|---|
| object_gravity | (0, −1600, 0) | u/s² | Gravity for projectiles whose record has `Gravity` 1, with the ¼ displacement quirk (`grenades.md` section 5). `GravityOverride` is never read |
| floor_normal_y | 0.75 | — | A thrown device sticks only to a surface whose normal y is at least this |
| device_resnap | 0.1 | s | A landed land trap, remote grenade or first-aid kit re-snaps to the floor below it at this interval (caltrops every frame) |
| blast_back_off | 1 | u | A projectile that hits world geometry detonates this far back along its flight direction from the surface point |
| char_hit_lookback | 100 | u | For a projectile hitting a character, the hit point is found on the hit spheres along the flight line starting this far behind the projectile |
| arc_cos_eps | — | — | The claymore cone test is a strict "dot product greater than cos(angle)" |
| caltrops_recount | 1000 | ms | Caltrops count a victim again only after this much time since they last counted him |
| prog_slots | 20 | entries | Over-time damage entries a character can hold; entries are keyed by the object that applies them |
| bolt_stuck_time | 5 | s | A crossbow bolt stuck in a wall or a body is removed after this |
| airstrike_floor_probe | 5000 / 10000 | u | The air strike looks for the floor below its impact point from y = 5000 down 10000 u |
| burn_self_exempt | rocket class only | — | Burn from a sphere never hurts its own shooter when the weapon's `Guntype` is 9 |

## The classes earlier specs skipped

`weapon_classes.md` covers `Guntype` 0–6, 13 and gives the M32 and rockets one paragraph each. The others, as the Steam data uses them (Guntype from `attribute_data.md`; count of weapon records in parentheses):

| `Guntype` | Name (`attribute_data.md`) | What the Steam data puts there | Covered in |
|---|---|---|---|
| 7 | thrown | grenades (69) | `grenades.md` |
| 8 | mission bomb | `XK-2` (1) | `game_modes.md` |
| 9 | rocket | **rocket launchers** (`LAW`, `RPG-7`, `M136_AT4` and skins), **stand-alone grenade launchers** (`M79`, `MZP_1`, `XM_25`), the **flamethrower** (`Flamethrower`, `RMS12`) and AI boss launchers (35) | 2, 3, 4 |
| 10 | mine | `M16A1_Mine`, `JackintheBox_LandMine`, `Hornet`, `Caltrops`, `Nerve_Confuse_Mine`, `Claymore_old` (8) | 6 |
| 11 | heartbeat sensor | sensors (`Heartbeat_sensor`, `HeartBeat_Detector`, `Satellite_Scanner`) **and** the air strike designators `LGB`, `Airstrike_Napalm` (6) | radar spec; 6.10 |
| 12 | air strike | `SA-18` and the shells the designators drop (`LGB_SHELL`, `Napalm_Shell`) (4) | 6.10 |
| 14 | snowball gun | `Snow_Ball`, `Ice_Ball` (2) | 8 |
| 15 | special grenade launcher | the **M32** family and infection-mode stun launchers (21) | 3 |
| 16 | medic kit | `Medkit`, `StaminaKit` (3) | 6.9 |
| 17 | install controller | `Claymore_Controller`, `Mobile_Grenade_Controller`, `Assassin_Bowgun_Expl_Controller` (3) | 6.3, 6.4, 7 |
| 18 | install bomb | `Claymore`, `Mobile_Grenade` (2) | 6.3, 6.4 |
| 19 | first-aid kit | `FirstAidKit` (2) | 6.8 |
| 20 | turret | `Auto-Turret`, `HG2_Auto_Turret`, `PerkAmmoSupply` (6) | 6.7 |
| 21 | crossbow | `Assassin_Bowgun_Nor` (1) | 7 |
| 22 | crossbow bomb | `Assassin_Bowgun_Expl` (1) | 7 |
| 23 | (not in `attribute_data.md`) | `Defence_Dummy_01..03` (3) | 9 |

Melee (`Guntype` 0) has 198 records; section 5 gives the common ones. No weapon in the Steam data has an under-barrel launcher: no weapon field, modification or attachment adds a second ammo type to a rifle.

## Where the numbers live

Each special weapon is three linked records: the **weapon** (`Weapon#`, its `AmmoName0` names the ammo), the **ammo** (`Ammo#`, its `ProjectileFXName` and `ImpactFXName` name effect records) and the **projectile record** (`ProjectileFX#` in `FX.TXT`, linked by `Name`). Types and defaults are `attribute_data.md`'s; the fields below are the ones this spec uses that it does not list.

**Projectile record** (`ProjectileFX#`, `FX.TXT`):

| Field | Type | Meaning |
|---|---|---|
| `Class` | str | Behaviour: `CProjectile` (impact), `CGrenade` (bouncing, `grenades.md`), `CLandTrap`, `CHornet`, `CAutoTurret`, `CPerkAmmoBoxProjectile`, `CNerveConfuseMine`, `CClaymore`, `CMobileGrenade`, `CCaltrops`, `CFirstAidKit`, `CAirStrike`, `CCrossBow`, `CCrossBowFunc`, `CMissionBomb`, `CDefenceDummyProjectile`. A weapon whose ammo names no projectile record or an empty class fires nothing |
| `Velocity`, `AltVelocity` | int, u/s | Launch speed for a normal and an alternate fire |
| `Lifetime` | num, s | Time from creation to detonation in the air (impact projectiles) or to removal (devices); see each class |
| `Gravity` | int | 1: the object gravity applies in flight |
| `GravityOverride` | num | Read, never used |
| `FireOffset` | num, u | Moves the start point this far along the aim, unless geometry is in the way; 0 in every player record |
| `CanHitSameKind` | int | 0: the projectile passes through other projectiles of the same shooter |
| `MaxRicochetAngle`, `MaxRicochets` | num (deg), int | Ricochet off the world; no player record has `MaxRicochets` > 0, so nothing ricochets (`effects_sounds.md`) |
| `Model`, `ModelScale` | str, vec | World model; its first animation's box × the scale is the projectile's box |
| `Guided`, `GuidedDelayTime`, `FireDelayTime` | num | Homing and delayed launch; only AI boss records set them |
| `DamageMaxCount` | int | Caltrops: victims counted before the caltrops disappear |
| `ExplosionDelayTime` | num, s | Explosive bolt: time from the detonator press to the blast |
| `TurretName` | str | Auto turret: names a `Turret#` record |
| `WeakDamageType#`, `WeakDamageRate#` | int list, num list | Damage types that hurt this object more, and by how much (×) |
| `ClassData` | str | `LandTrapData` (`LandTrapClassData0`, `ArmDelay` 2.0 s); only the mission bomb reads it |

**Ammo fields** beyond `attribute_data.md`: `AreaArcDamage` (int, HP), `AreaArcDamageRadius` (int, u), `AreaArcDamageAngle` (num, degrees, half-angle of the claymore cone); `ProgDamageHeight` (num, u, caltrops' cylinder height); `AreaDamageTickTime` (num, s), `AreaDamageTickDamage` (num, HP), `AreaDamageTotal` (num, HP) (first-aid kit); `SensorAreaRadius` (int, u; hornet trigger radius, turret search radius); `LandTrapStartFXName` (str; the effect played when a trap is set off).

**Weapon fields**: `SubControlWeaponName` (str; the detonator that goes with a claymore, remote grenade or explosive bolt), `OnceActivate` (bool; a new one replaces the old one), `AirStrikeID` (int; the weapon record index of the shell an air strike drops), `AirDropHeight` (num, u), `AirDropDelay` (int, ms), `AirDropPoint#` (point list, u; drop offsets), `TargetIsMe` (int; the shot hits the shooter: kits), `NoSelect` (detonators).

**Turret record** (`Turret#`, `FX.TXT`): `TopWeaponID` (weapon record index of the turret's gun), `MaxAmmo`, `FireDelay` (s), `Perturb` (u), `FirePosUpOffset`, `FirePosFrontOffset` (u), `SensorOffset` (vec, u), `ShootingAngle` (point, degrees), `BottomLinkNode`, `TopLinkNode`, `Model`.

Animation lengths and key times come from each weapon's first-person animation file (the `Ani_…` model named inside its `PVModelNormal`), as in `view_models.md`.

## Behavior

### 1. Rules every projectile shares

#### 1.1 Firing

- The fire cycle, per-press limit, clip, reload and draw are exactly `weapon_firing.md` and `weapon_classes.md`: one `FIRE_KEY` (or `ALT_FIRE_KEY`; both fire) in the fire animation raises one shot, and the shot spends one round from clip and total.
- **Origin**: the camera position (third person: the weapon's fire position), sent by the client. **Direction**: the camera forward perturbed by the weapon's spread cone (`weapon_firing.md`, Spread; limits multiplied by `DynamicPerturbFactor` and truncated). The client does not test what a projectile will hit; it only sends origin, direction and the fire.
- **Speed**: $\vec v_0 = s\,\hat d$ with $s$ = `Velocity` (or `AltVelocity` for an alternate fire). The shooter's own velocity is not added. The throwing perk's force multiplies $s$ only for owners of that perk (`grenades.md` section 4).
- The server creates the projectile when the fire message is handled, at the start of its frame; it records the previous frame's time as its start and takes its first physics step in that frame (`grenades.md` section 5, Open question 6 there). Lifetimes therefore end after $\lceil (\text{Lifetime} - dt)/dt \rceil$ physics steps at 20 Hz: 19 steps for 1 s, 199 for 10 s.
- At creation the projectile gets the ammo's `Health` as hit points (land traps get theirs when they land, 6.1), is turned to face its direction, and its box is the first animation box of its model × `ModelScale`.

#### 1.2 Flight

- Records with `Gravity` 1 fly under the object gravity with the ¼ displacement of `grenades.md` section 5; records with `Gravity` 0 fly in a straight line at constant speed. There is no drag. Water volumes act as in `grenades.md` section 7 (Open question 10).
- Collision with the world is the point test of `grenades.md` section 5: the projectile is a point, ignores clip (invisible) surfaces and stops at every other polygon of the physics tree and of solid world models. Touching a sky surface removes it silently.
- Characters are touched through their boxes and accepted only when the line of flight passes a hit sphere (`grenades.md` section 6, "Characters"); the shooter and the shooter's hitbox are never touched. Other objects are touched when solid or ray-hit. Projectiles of the same shooter pass through each other when `CanHitSameKind` is 0; all player rockets and the flame have 0, launcher grenades 1.

#### 1.3 Impact projectiles (`Class` `CProjectile`)

- The **first** accepted touch detonates the projectile. There is no fuse, no bounce, no arming distance and no dud: a projectile that touches a wall 1 u from the muzzle explodes there.
- **Where**:
  - World or a world model: the server traces from $\vec p - \vec v\,\Delta t$ to $\vec p$ (with $\vec v$ the post-gravity velocity of the step) to find the surface point $\vec q$, and the blast point is $\vec q - \hat v$ (1 u back along the flight direction). The impact surface is that polygon's.
  - A character: the hit point is found on the hit spheres along the flight line, starting 100 u behind the projectile; the blast point is that hit point.
  - The lifetime ends in the air: the blast point is the projectile's position, surface "air".
- **Direct-hit damage**: the struck object takes the ammo's `InstDamage` of type `InstDamageType`, scaled by the range band of `weapon_firing.md` with the distance from the fire position to the **client-reported impact point**. Projectile shots report none, so the reference measures to the world origin (Quirk 4). A type of 0 deals nothing. In the stock data every rocket and launcher grenade has `InstDamage` 0 (rockets type 0, launcher grenades type 8, freeze), so only the blast hurts.
- **Blast and sphere**: if the ammo's `AreaDamage` > 0 (and the surface is not sky) or `ProgDamageLifetime` > 0, an explosion is created at the blast point with the ammo's area and over-time numbers and runs `grenades.md` sections 9 and 12: damage $D = $ `AreaDamage` $\cdot\, m(d)$ with $R$ = `AreaDamageRadius`, players applied twice for explode, cover traces both ways (with a second try to the head top), objects other than characters, hitboxes and projectiles without a cover test, friendly-fire filter, armour with the torso entries, kill credit to the shooter. A character struck directly skips the cover test.
- The impact effect (`ImpactFXName`) and its sound play at the blast point on every client (`effects_sounds.md`).

#### 1.4 Shooting projectiles down

- Every projectile, rocket and device accepts bullets and blasts (`grenades.md` section 13): when its hit points fall to 1 or less, it detonates where it is, credited to its shooter, with the full blast. Rockets have 50 HP, launcher grenades 300.
- Its surface is metal and its box is the model box above (rocket 44.8 × 5.6 × 5.6 half-extents; M32 shell 128 × 16 × 16, Quirk 5).
- Projectile `WeakDamageType#` / `WeakDamageRate#` multiply damage of those types (the auto turret takes ×3 from explode and infected explode; caltrops ×3.5 from infected explode).

#### 1.5 Ownership

- The shooter is held by reference. If he **leaves the game**, the reference clears: an impact projectile still detonates but its explosion does nothing (an explosion without a shooter is not set up), and every device removes itself on its next update.
- The shooter **dying** changes nothing for projectiles in flight: they detonate normally and the kill credit is his. Devices: see each class (claymores and remote grenades detonate, 6.3).
- At the end of each round (the mode's "object pre-end" state), every land trap, claymore, remote grenade, caltrops, nerve mine, first-aid kit, air strike and lasting explosion sphere removes itself on its next update. Impact projectiles and bouncing grenades in flight are not removed (Open question 1).

#### 1.6 Server and client

- All projectiles and devices are server objects. Clients draw them from a create message (weapon, ammo, shooter, speed, direction) and from position updates sent unguaranteed. Detonations, blasts and device states are server events; the only client decision is the auto turret's target (6.7).

### 2. Rocket launchers (`LAW`, `RPG-7`, `M136_AT4`)

- One-round clip; the projectile record is `Missile` (`Class` `CProjectile`, 1300 u/s, `Gravity` 0, `Lifetime` 10 s, model scale 0.35, `CanHitSameKind` 0, 50 HP).
- **Flight**: a straight line at 1300 u/s, 65 u per 20 Hz step. It detonates on its first touch or, after 199 steps (12 935 u), in the air.
- **Damage**: blast only. LAW `AreaDamage` 40, RPG-7 43, both radius 550, type 7. A player within 137.5 u of the blast takes 80 (LAW) or 86 (RPG-7) in total from the double application, so **a direct hit on a full-health player does not kill**. M136_AT4: `AreaDamage` 0, so no explosion object is created: it does no damage (Quirk 1).
- **Spread**: cone 1..30 × 1.01 gives $n_{min} = 1$, $n_{max} = 30$; the rocket ammo has no spread keys, so both meters stay 0 and every rocket leaves within ±0.001 of the aim on each axis.
- **Timing** (animation files): LAW draw 1333, fire 1000, reload 3333 ms; RPG-7 draw 1300, fire 900, reload 3000 ms; M136_AT4 draw 766, fire 1333, reload 2166 ms. The empty clip starts the reload when the fire animation ends (`weapon_firing.md`). The visible rocket piece (`ProjectilePiece`) follows `view_models.md` section 3.
- **Kick**: `BaseCamRecoilPitch` 5.5° with a 200° cap: each shot lifts the view 5.5° (`weapon_firing.md`, Recoil). `MovePenalty` 0.2.
- **No lock-on**: homing (`Guided`) is set only on an AI boss missile. **No back-blast**: nothing damages or pushes anyone behind the launcher. **No knock-back** in stock play: the rocket ammo's `DamageMovePenalty` 135 / 0.4 s is `attribute_data.md`'s victim slow and knock-back field (infected victims only); the rocket impact's pusher has radius 0 (`effects_sounds.md`). The ammo's `BLPhysicsDirectionVector` and `BLPhysicsForceRatio` feed ragdolls only (Open question 9).
- `ReloadAfterFire` 1 and the zoomed fields do nothing without a scope (`weapon_classes.md` section 8). The server lets rocket shots through during its reload bookkeeping (`weapon_classes.md` 5.6).

### 3. Grenade launchers

There is no under-barrel launcher in this build. All launchers fire the projectile record `M32_Grenade` (or a variant): `Class` `CProjectile`, 1550 u/s, `Gravity` 1, `Lifetime` 10000 s (no fuse), model `M32_Shell.ltb` at scale 1, `CanHitSameKind` 1, 300 HP. The ammo's `Type` is 2.

#### 3.1 Flight and detonation

- The shell flies exactly like a thrown grenade (`grenades.md` section 5: object gravity 1600 with the ¼ term, point collision, no drag, 20 Hz) from the camera at 1550 u/s along the aim, but **detonates on its first touch** instead of bouncing. Looking level from an eye 60 u above flat ground, it lands 442.86 u away in its sixth step.
- There is no minimum range and no arming: point-blank shots explode at the wall. A shell that never touches anything flies until it leaves the world through the sky (removed silently) or for 10000 s.
- **Damage**: blast `AreaDamage` (M32 40, M79 40, MZP_1 42, XM_25 22, M32_GodRa 60) in radius 300, type explode, applied twice to players: an M32 shell within 75 u of a player does 80.
- **Spread**: cone 8..8 × 1.2 gives $n = 9$: every shell leaves within ±0.009 on each axis (±0.52°), whatever the movement.

#### 3.2 The M32 (revolving, `Guntype` 15)

- Six-round cylinder, 12 total (`M32`), `RapidFireCount` 0: **holding fire shoots one shell every 1000 ms** (the `fire_0` length; its fire key is at 0 ms). Draw 1200 ms.
- **Reload, shell by shell** (`ReloadPerBullet` 1, `weapon_classes.md` 5.5): `prereload` 2100 ms, then `reload` 1000 ms per shell, then `postreload` 1034 ms. With clip $c$ and total $T$, the number of shells loaded is $k = \min(6, T) - c$ and the reload ends at $2100 + 1000k + 1034$ ms. Fire with a non-empty clip stops it at once (`weapon_classes.md` 5.5). Reloading 0 → 6 takes 9134 ms.
- Variants: `M32_IncendiaryBomb` (6 total): `AreaDamage` 0, so no instant blast, but a burning sphere of radius 200 for 9 s that keeps a 10 HP/s burn entry of 6 s on everyone inside (`grenades.md` section 12; the shooter is **not** exempt, unlike the flamethrower). `M32_Stun` (6 total): `AreaDamage` 15, `StunnedTime` 8 s (victim-side stun, 6.5's rule).
- The server's reload bookkeeping treats the M32 like a shotgun (`weapon_classes.md` 5.6).

#### 3.3 Stand-alone launchers (`Guntype` 9)

| | `M79` | `MZP_1` | `XM_25` |
|---|---|---|---|
| Clip / total | 1 / 3 | 1 / 4 | 4 / 8 |
| `RapidFireCount` | 1 | 1 | 1 |
| Fire animation (cycle) | 1000 ms | 1000 ms | 800 ms |
| Reload (whole magazine) | 2835 ms | 2935 ms | 2935 ms |
| Draw | 1000 ms | 1000 ms | 1000 ms |
| `AreaDamage` / radius | 40 / 300 | 42 / 300 | 22 / 300 |
| `MovePenalty` | 0.18 | 0.18 | 0.19 |

The `XM_25` has no airburst or range-setting behaviour in this build; it is a four-round semi-automatic impact launcher.

### 4. The flamethrower (`Flamethrower`; `RMS12`)

- `Guntype` 9, 300-round clip and total, `RapidFireCount` 0, `MovePenalty` 0.2, draw 1200 ms, reload 2133 ms.
- **Fire cycle**: `prefire` 200 ms, then `fire_0` (734 ms) loops while fire is held; it carries **eight fire keys** at 0, 100, …, 700 ms. Each key spends one round and launches one flame puff. Releasing plays `postfire` (333 ms). Holding from a full clip empties it after 300 puffs: about 27.5 s plus frame rounding.
- **Puff**: projectile record `Yellow_Flame`: `Class` `CProjectile`, 500 u/s, `Gravity` 0, `Lifetime` 1 s, no model (box 0), `CanHitSameKind` 0. It flies straight from the camera with cone $n = 9$ (8 × 1.2; the movement and firing targets of 0.2 cannot widen a cone whose min equals its max).
- **Detonation**: on the first touch (wall, floor, a character's hit sphere) or in the air after 19 server steps (475 u from the muzzle). The ammo has `AreaDamage` 0 and `ProgDamageLifetime` 2 s, so it creates **only a lasting sphere**: radius 75 u (`ProgDamageRadius`), alive 2 s, which every server frame sets or refreshes, on each player and AI inside (with the cover traces cast from 1 u toward the target), an over-time entry of 12 HP/s (`ProgDamage`) lasting 1 s (`ProgDamageDuration`), damage type burn. Effective reach: about 475 + 75 = 550 u.
- **Stacking**: entries are keyed by the sphere that applies them, so each puff's sphere is a separate entry and overlapping spheres add up, up to 20 entries per character; when all 20 are busy a new entry replaces the one with the least time left only if it would do more damage (rate × remaining time). A target standing in one puff's sphere for its whole life loses 12 × (2 + 1) = 36 HP.
- **Who burns**: the shooter never takes burn from a sphere of a `Guntype` 9 weapon. Teammates: the friendly-fire filter of `grenades.md` section 9 (full burn with friendly fire on, none with it off). Armour: burn uses the gear's torso burn entry (stock body gear 0.5 while armour points remain).
- Burn spheres also damage world objects and props (burn is one of the area types that reach non-player objects).
- `RMS12` is the same weapon class with a 134 ms single-key `fire_0` and `ProgDamage` 0: it creates spheres that do nothing (Open question 8).

### 5. Melee weapons beyond the M9

- Every stock melee weapon follows the M9's rules (`inventory.md` section 8): left button = `fire_0` with ammo `InstDamage`, right button = `alt_fire_0` with `Alt_InstDamage`; one attack ray per hit key from the camera, length `Range`; no pre-fire; a held button repeats the swing with no gap; the random roll is `DamageRFacter` (0.1 on all); the range band uses the weapon's `Effectrange#` and the ammo's `Rangedamage#` (1.0 everywhere in the stock data); damage type melee (13); head ×6, arms and thighs ×0.9, shins ×0.75, feet ×0.5 (`weapon_firing.md` Hit zones); rear sector ×1.3 (`inventory.md` 8.4); no armour reduction; no penetration.
- `FIRE_KEY` and `ALT_FIRE_KEY` keys both deliver a hit; which damage applies depends only on which button started the swing.
- **No lunge, no back-stab bonus of its own, no charge**: no field or code exists for them. Reach is the only geometric difference.
- Range above the last band edge applies no multiplier (`weapon_firing.md` Q8); `Balisong_Knife` reaches 130 u with bands ending at 115 u.

| Weapon (model) | `Range` | L dmg | L swing (ms) / hits at | R dmg | R swing (ms) / hits at | Draw (ms) | `MovePenalty` |
|---|---|---|---|---|---|---|---|
| `M9` | 105 | 45 | 650 / 155, 402 | 75 | 1100 / 367 | 450 | 0 |
| `M11-Tactical_Knife` | 105 | 50 | 450 / 32 | 68 | 1100 / 267 | 250 | 0 |
| `Tracker_Knife` | 130 | 75 | 450 / 65 | 90 | 1000 / 243 | 260 | 0.02 |
| `Tiger_Strider` | 110 | 80 | 467 / 67 | 90 | 1067 / 367 | 666 | 0 |
| `Sickle` | 130 | 55 | 467 / 33 | 72 | 1100 / 417 | 666 | 0.03 |
| `UAMC_KA_Bar` (`USMC_Ka_bar`) | 135 | 65 | 600 / 200 | 75 | 1000 / 425 | 450 | 0.03 |
| `Bowie_Knife` | 135 | 70 | 700 / 133 | 70 | 1100 / 467 | 666 | 0.03 |
| `Balisong_Knife` | 130 | 52 | 650 / 179 | 30 | 900 / 354, 498 | 500 | 0 |
| `Knuckle` (`Brass_Knuckles`) | 130 | 55 | 800 / 412 | 72 | 1000 / 424 | 800 | 0.01 |
| `Ninjato` | 150 | 68 | 800 / 200 | 68 | 1800 / 367, 634 | 600 | 0.03 |
| `Guillotine` | 150 | 68 | 750 / 188 | 68 | 1800 / 367, 634 | 600 | 0.03 |
| `Kukuri` (`Kukri`) | 135 | 70 | 800 / 343 | 70 | 1800 / 535, 984 | 666 | 0.03 |
| `Machete` | 135 | 70 | 900 / 267 | 70 | 2134 / 667, 1167 | 600 | 0.03 |
| `GI_Shovel` | 135 | 70 | 900 / 267 | 70 | 2134 / 667, 1167 | 600 | 0.03 |
| `Strider_Axe` (`Tomahawk`) | 130 | 70 | 900 / 233 | 76 | 1950 / 609, 1066 | 666 | 0.03 |
| `Survival_Machete` | 130 | 80 | 1000 / 343 | 80 | 1200 / 386, 692 | 600 | 0.035 |
| `SledgeHammer` (`3LB_Sledge`) | 130 | 70 | 1000 / 333 | 82 | 1900 / 655, 1146 | 650 | 0.03 |
| `DomPedroSword` | 160 | 85 | 750 / 322 | 85 | 1500 / 445, 820 | 500 | 0.03 |
| `Chainsaw` | 150 | 75 | 600 / 257 | 15 | 900 / 334, 386, 437, 489, 540, 591, 643 | 1000 | 0.05 |

The model column names the first-person model where it differs from the weapon name. A secondary with two hit keys hits twice for the listed damage each: a `Machete` secondary is 140 before the roll, the `Chainsaw` secondary 7 × 15 = 105.

#### 5.1 What a swing sounds like (added 2026-10-30)

Worked out from the client weapon's fire step, the shared weapon-sound player, the client's ray and impact-effect path and the server weapon's fire handling. Terms (local, 3D, R, r, gain g(d)) are `effects_sounds.md` sections 10 and 11.

- **One fire sound per hit key, not per swing.** A hit key that raises a hit (`view_models.md` 8) runs the same fire step as a gun's shot, and that step ends by starting the weapon's fire sound. So every `FIRE_KEY` and every `ALT_FIRE_KEY` plays it once: the M9 primary twice per swing (155 and 402 ms), a two-hit secondary twice, the chainsaw secondary seven times per 900 ms swing. There is no separate swing ("whoosh") sound and no per-swing sound of the fire kind; the fire sound **is** the swing sound. It plays on a miss exactly as on a hit (the attacker's own client plays it whatever its ray finds and whatever the server later decides).
- **Which file.** On the attacker's own client the button that started the swing decides, for every hit key of that swing: primary → `FireSnd`, secondary → `Alt_FireSnd`. The button is latched for the swing (the same latch that picks `InstDamage` or `Alt_InstDamage`; `inventory.md` 8.1 for changing buttons between swings). A silencer-type muzzle part would take precedence over both (no melee weapon takes one). **Other players always hear `FireSnd`**, never `Alt_FireSnd`: the server's event says "alt-fire", but the receiving client's choice ends with plain fire whenever there is no silencer-type part (Quirk 10). For the stock chainsaws both keys name the same file (`fire.wav`, `fire2.wav` for The Shining's), so nothing is audible; it matters only for data where the two differ.
- **How it is played.** Yours: local, volume 100, in the weapons volume class. Others': 3D at the attacker's fire position (not following him), radius `FireSndRadius`, volume 100, one per relayed hit (`effects_sounds.md` 11). The server refuses a fire message that comes less than the weapon's `FireDelay` after the attacker's previous one; a refused hit does no damage and is not relayed, so others hear nothing for it (the attacker already heard his).
- **One fire voice per attacker and weapon.** Starting a fire or alt-fire sound first stops the still-playing fire or alt-fire sound of the **same attacker with the same weapon** (primary and secondary share the one voice), then starts the new one from its beginning. Other weapons of the same attacker and other attackers are not touched. Every client applies this to its own copies (yours to your own swings, each other client to each attacker it hears). Silenced fire sounds are outside the rule: they never stop anything and are never stopped. The voices are also all stopped when a match is left or restarted.
- **Chainsaw, rate 1, secondary held** (`fire.wav` is 2.9 s): the fire sound starts at 334, 386, 437, 489, 540, 591 and 643 ms of each 900 ms swing; each start stops the previous one, so the first six play only 51 or 52 ms of the file and the seventh plays until the next swing's first key at 900 + 334 = 1234 ms (591 ms of the file). Released after a swing, the last one plays its full 2.9 s. Primary held: one start at 257 ms of each 600 ms swing, each heard for 600 ms; released, 2.9 s.
- **View-model sound keys** (`sound_key n`, `view_models.md` 8) are separate sounds and are never stopped by fire sounds or by each other. The chainsaw secondary's `sound_key 1` at 0 ms (`ReloadSnd1`, `RELOAD1.wav`, 1.23 s) therefore plays once per swing, and with the button held each copy overlaps the next swing's copy by 0.33 s. A weapon that names a `PreFireSnd` or `PostFireSnd` plays it on entering the pre-fire or post-fire animation; only one of those plays at a time (a new one stops the old).
- **Several hit keys in one frame.** The key handler only raises a single "fire now" flag, consumed by the next weapon update, so hit keys that pass in the same view-model advance give **one** hit and one fire sound. The chainsaw secondary's keys are 51–52 ms apart, so this happens whenever a frame is longer than that (below about 20 fps) (test case 46).
- **Impact sound of a hit** (`effects_sounds.md` 6 and 11). The attacker's own client makes its impact effect, sound included, from its own ray **before** it starts the fire sound of the same hit; other clients make it from the server's event. The ammo's ImpactFX `Melee` plays the surface's `MeleeImpactSnd` list, one of the four slots uniformly, so the stock two-file lists are silent half the time; 3D at the hit point, R 153 (r = 0.07 · 153 = 10.71 u), volume 100 (50 under water); no decal, no surface effect.
  - World, world models and props: the struck surface (`melee\wood\impact1..2.wav` on Wood, and so on).
  - A character: the model's head surface for a head-location node, its body surface otherwise (4 and 6 for stock player models, `effects_sounds.md` 5), both of which list `melee\flesh\impact1..2.wav`. The victim's server hurt sounds (`DeathbyGuns` kind; `HitHead` for a head hit) and the attacker's hit-confirm ding come on top (`effects_sounds.md` 11).
  - **A miss**: when the ray meets nothing within `Range`, the client treats it as a sky impact at the ray's end, and a sky impact makes no sound and no effect (the `Melee` record does not set `ImpactOnSky`). A hit on a sky polygon is the same. A miss is therefore heard as the fire sound alone.
  - **Out of earshot for the attacker.** The attacker's camera is both the listener and the ray's start, so d is the hit distance and g(d) = 1 − (d² − 10.71²) / 142.29²: 0.69 at 80 u, 0.11 at 135 u, 0 from 142.7 u on. Hits beyond 142.7 u (possible with `Range` 150 and 160) make no impact sound for the attacker; a bystander nearer the hit can hear it.

### 6. Mines and devices

All devices below are thrown or placed with the fire key of their animation (table in 6.12), spend one round, and are owned by the thrower. Thrown devices leave the camera at the projectile record's `Velocity` (600 u/s for all thrown traps) under the object gravity, with the weapon's cone (1..10 at factor 1.0, meters 0: $n = 1$).

#### 6.0 Landing (land traps, remote grenades, caltrops, first-aid kits)

- In flight, a touch on a surface whose normal y is below 0.75 (a wall, a steep slope, a ceiling) does nothing of the device's own: no bounce, no stop. The engine's contact response removes the speed into the surface and the device keeps falling (Open question 3).
- The first touch on a surface with normal y ≥ 0.75 **lands** it: velocity 0, gravity off, rotation set to the throw direction's horizontal part, hit points set to the ammo's `Health`, its zone effect (`ZoneLoopSFX`) shown to clients. From then on it re-snaps to the floor below it every 0.1 s (so it follows a floor that moves or disappears under it).
- Devices do not bounce off other devices or players while flying (they pass through characters whose spheres the flight line misses, as in 1.2).

#### 6.1 Land mines (`M16A1_Mine`, `JackintheBox_LandMine`; `Class` `CLandTrap`)

- Thrown at 600 u/s (`LandTrap` record, model `Landtrap.ltb` × 2: box half-extents 5.8 × 6.6 × 5.8), `Lifetime` 60000 s. 3 per spawn, `Health` 50 once landed.
- **Arming**: immediate. The mine is armed on its first server update (the land-trap arm timer is never started, so it counts as run out; the 2 s `ArmDelay` belongs to the mission bomb only, Quirk 6).
- **Trigger**: once landed and armed, a **character object** whose box touches the mine's box sets it off, unless the character is the owner, is on the owner's team, or is dead. In free-for-all every other player is an enemy.
- **Detonation**: the trigger plays the mine's `active` animation (267 ms) and the `LandTrapStartFXName` effect (`M16mine_ini`); when the animation has finished (checked each server frame), the blast goes off at the mine's position: `AreaDamage` 40, radius 550, explode, applied twice (80 at ≤ 137.5 u). After the trigger, further touches do nothing.
- A landed mine destroyed by damage (50 HP: an M67 within about 174 u, a few bullets) is set off the same way, credited to its owner. While still in flight it has 1 HP but cannot be set off (Quirk 7).
- Stays when its owner dies; removed when the owner leaves and at round end (1.5).

#### 6.2 The hornet (`Hornet`; `Class` `CHornet`)

- Thrown like a land mine (model × 2: 9.4 × 14 × 9.4), 2 per spawn, `Health` 150.
- After landing it plays `ready` (666 ms) with the `Hornet_Ready` sound, then loops `set` (3966 ms). Only in `set` does it look for targets: every server frame, every object touching a sphere of radius `SensorAreaRadius` (100 u) around it is tested, and the first that passes sets it off:
  - a player or an AI, not the owner, not on the owner's team, not dead, not an observer;
  - its position (centre) is **not below** the hornet's position;
  - a line from the hornet to its centre and back is not blocked by solid world or solid world models.
- Detonation: `active` (534 ms), then the blast: `AreaDamage` 150, radius 150, explode, twice (300 within 37.5 u: lethal through any armour).
- Destroyed by damage: as a land mine, but it then detonates without the target tests.

#### 6.3 Claymores (`Claymore` with `Claymore_Controller`; `Class` `CClaymore`)

- **Placing**: the projectile record has `Velocity` 0, so the claymore is not thrown: it is put on the floor in front of the owner (owner position + horizontal aim × the owner box's Z half-extent, then dropped to the floor below), facing the owner's yaw. 2 per spawn, `Health` 300. Placing takes the `postfire` key at 534 ms after a 1000 ms `prefire`.
- **Detonator**: selecting the claymore's `SubControlWeaponName` (`Claymore_Controller`, `NoSelect`, swapped in by `inventory.md`'s controller pairing) and firing it (its `fire_0`, fire key at 25 ms) detonates **every** claymore of that weapon the owner has placed. Touches never set it off.
- **Also detonated**: when the owner dies (all of his claymores and remote grenades, 6.4), and when its 300 HP are shot or blasted away. Removed silently when the owner leaves and at round end.
- **Delay**: the press marks it; it plays `Active` and blows on its next server update (one frame, 50 ms).
- **Blast** (damage type claymore, 38, applied twice to players; no stock gear protects it): every object within 500 u is tested. With $\hat f$ the claymore's horizontal facing and $\hat u$ the unit vector from the claymore to the object's centre:

```math
D = \begin{cases} 120 \cdot m(d)\ \text{with}\ R = 500 & d \le 500\ \text{and}\ \hat f\cdot\hat u > \cos 80^\circ = 0.17365 \\ 60 \cdot m(d)\ \text{with}\ R = 120 & \text{otherwise, if}\ d \le 120 \\ 0 & \text{otherwise} \end{cases}
```

  (`AreaArcDamage` 120, `AreaArcDamageRadius` 500, `AreaArcDamageAngle` 80; `AreaDamage` 60, `AreaDamageRadius` 120.) The cone is 160° wide and three-dimensional (a victim above the claymore has a smaller dot product). Cover traces run from the point 1 u from the claymore toward the object; the directly-struck exception does not exist here.
- `Claymore_old` has `AreaDamage` 0 and a land-trap class: it does nothing (Open question 8).

#### 6.4 Remote grenades (`Mobile_Grenade` with `Mobile_Grenade_Controller`; `Class` `CMobileGrenade`)

- Thrown at 600 u/s like a land mine (model × 2), lands by 6.0, 2 per spawn, `Health` 300. Throw: `prefire` 1767 ms, `fire_0` 67 ms, `postfire` 633 ms with the release key at 333 ms (release 2167 ms after the press at rate 1).
- Detonated only by its controller (`fire_0` 1167 ms, key at 400 ms), the owner's death, or damage, exactly as claymores (6.3), one frame after the press.
- **Blast**: type claymore with no cone data, so the plain part applies in every direction: `AreaDamage` 100 with $R$ = 450 ($r_0$ = 112.5), applied twice (200 within 112.5 u).

#### 6.5 Caltrops (`Caltrops`; `Class` `CCaltrops`)

- Thrown at 600 u/s (model × 1.2: 18 × 6 × 18), 1 per spawn (5 at most with pickups), `Health` 50. Throw: `prefire` 600, `fire_0` 67, `postfire` 366 with the key at 92 ms.
- On landing (only if the owner is a player) it becomes active and, **every server frame** while it lives, damages each character whose centre is in an upright cylinder on it: horizontal distance ≤ 50 u (`ProgDamageRadius`) and height above the caltrops 0..80 u (`ProgDamageHeight`). Owner and owner's teammates are exempt. The damage is an over-time entry, refreshed each frame: bleeding (1), 3 HP/s (`ProgDamage`) for 5 s (`ProgDamageDuration`). The ammo also carries `StunnedTime` 5 s and `StunnedMovePenalty` 0.4: the victim's client adds 0.4 to his movement penalty for 5 s after each damage message from this ammo (non-teammates).
- **Count**: a victim is counted when first damaged and again whenever more than 1000 ms have passed since his last count; at 12 counts (`DamageMaxCount`) the caltrops disappear (with their impact effect). They also disappear 60 s after landing (`Lifetime`), or 60 s after the throw by the projectile lifetime, whichever comes first.

#### 6.6 The nerve confuse mine (`Nerve_Confuse_Mine`; `Class` `CNerveConfuseMine`)

- Thrown at 800 u/s (model × 2: 20 × 6 × 20), 2 per spawn, `OnceActivate` (6.11), `Health` 300, `Lifetime` 60 s. Throw: `prefire` 1100, `postfire` 633 with the key at 333 ms.
- It becomes active on its **first touch of any surface** (walls included) and lands by 6.0 if that surface is a floor. While active, every server frame it applies to every enemy character within 250 u (`AreaDamageRadius`; no cover test, no height test, owner and teammates exempt) damage of type 44 (camera reverse) with damage 3 and duration 3 s (`AreaDamage` is used for both). Its effect on the victim belongs to the damage-effects spec (Open question 8).

#### 6.7 The auto turret (`Auto-Turret`; `Class` `CAutoTurret`)

- Thrown at 600 u/s, lands by 6.0, 1 per spawn, `OnceActivate`, `Health` 350, takes ×3 from explode and infected explode. `MovePenalty` 0.35 while carried. Throw: `fire_0` 3666 ms with the release key at 3100 ms.
- **Setting up**: on landing it plays `Lower_set` (2833 ms) and the `AutoTurret_Ready` sound; at the end it shows its gun and starts firing. `Turret#` record `Auto-Turret`: gun `TopWeaponID` 331 (`Turret-TopGun`), `MaxAmmo` 100, `FireDelay` 0.13 s, `Perturb` 50, `SensorOffset` (0, 10, 0), `ShootingAngle` (−45, 60).
- **Target**: chosen by the **owner's client** every frame and sent to the server when it changes. Candidates: characters that are not observers, not dead and not on the owner's team (free-for-all: everyone but the owner), whose centre is within `SensorAreaRadius` (800 u) of the turret, sorted by distance. For each, in order, three points (centre, centre ± half its box height) are tested: a line from the turret's sensor point (position + `SensorOffset`) to the point must reach the character or its hitbox first, and the elevation angle $\alpha$ of the line from the sensor to the centre must satisfy the shooting angle: up to 60° above the horizontal, up to 45° below. The first passing candidate becomes the target. If the sensor point itself is blocked from the turret, the target is not updated that frame.
- **Firing**: on the server, at most once per 0.13 s while a target is set and the gun has rounds, the gun turns to the target and fires one instant ray of `Turret-TopGun` (12 damage, bullet, ±5 % roll, bands 800/900/1000 at 1.0/0.7/0.35, range 1000) from its `Flash` socket toward the target's centre, the unnormalised direction $\vec t - \vec o$ perturbed by a uniform ±50 u on each of x, y and z (so the error is 50 u at any range). Each ray spends one of the 100 rounds; at 0 it removes itself with its disappear effect. The kill credit is the owner's; a turret never damages its own team (`game_modes.md`).
- No blast when destroyed or empty: the turret's ammo has `AreaDamage` 0.

#### 6.8 The first-aid kit (`FirstAidKit`; `Class` `CFirstAidKit`)

- Thrown at 600 u/s, lands by 6.0 (only if the owner is a player), 3 per spawn, `Health` 30. Throw: `fire_0` 1000 ms, key at 600 ms.
- Once landed, every 1.0 s (`AreaDamageTickTime`, first tick 1 s after landing) it heals each player whose centre is within 120 u and **not below** the kit, on the owner's team (the owner included), alive and not at full health: 5 HP (`AreaDamageTickDamage`) of type 42, from a pool of 50 (`AreaDamageTotal`); the last heal is capped to what is left in the pool. The kit disappears when the pool is empty.
- But its projectile record's `Lifetime` is 2.0 s from the throw, after which it is removed: in practice a kit heals at most once or twice (Quirk 2).

#### 6.9 The medkit and the stamina kit (`Medkit`, `StaminaKit`; `Guntype` 16)

- Not projectiles: ammo `Type` 0 with `TargetIsMe` 1, so the "shot" hits the user himself (the server checks the client's flag against the data and refuses a mismatch). Medkit: `fire_0` 1500 ms, key at 493 ms; 2 per spawn.
- `Medkit`: an over-time heal entry, type 37, 11 HP/s for 3 s, capped at 33 HP in total and at the maximum hit points. `StaminaKit`: an over-time entry of type 39 (stamina) for 5 s with rate 0; what it restores belongs to `gear_perks.md` (Open question 8).

#### 6.10 Air strikes (`LGB`, `Airstrike_Napalm`; shells `LGB_SHELL`, `Napalm_Shell`)

- The designator fires a projectile record `Invisible` / `Invisible_Napalm` (`Class` `CAirStrike`, 5000 u/s, `Gravity` 0, `Lifetime` 8 s / 7.5 s). Its first touch (world, a world model, a character) records a target point: the touch point, moved to the floor found straight below it from y = 5000 (if found, at the marker box's half-height above it). Later touches are ignored.
- When the marker's lifetime ends (8 s after firing, wherever it is), the strike begins: shells of the weapon record `AirStrikeID` (32 = `LGB_SHELL`, 265 = `Napalm_Shell`) are dropped one per `AirDropDelay` (100 ms; 200 ms), one per `AirDropPoint#` entry: target point + the offsets along the course (x to the right, y forward; the course is the horizontal direction from the designator's fire position to the target), from `AirDropHeight` (3000 u) above it, straight down.
- Shells are impact projectiles (`LGB_Shell` record: 1550 u/s, gravity): `LGB_SHELL` one shell, blast 110 in radius 1200, explode (220 to a player within 300 u). `Napalm_Shell` six shells at 0, 200, …, 1000 u forward: no instant blast, a burning sphere each of radius 350 for 5 s that keeps an 80 HP/s, 2 s burn entry on everyone inside.
- `WorldCoolTime` 180000 ms and `HaveWorldCoolTime` 1 on both designators: a shared cool-down between uses (Open question 8). `SA-18` (`Guntype` 12) throws a smoke-grenade marker (`AS_Grenade`, a bouncing grenade, 3.5 s) whose ammo does nothing in the Steam data.

#### 6.11 One at a time (`OnceActivate`)

- For weapons with `OnceActivate` true (`Auto-Turret`, `Nerve_Confuse_Mine`), creating a new one sets the lifetime of the owner's previous one of that weapon to 0: it is detonated (removed with its effect) on its next update. Every other device is limited only by ammo.

#### 6.12 Throw and use timings

From the first-person animation files; the device appears on the server when the client's fire message for that key arrives. Stages with a pre-fire, a looped `fire_0` and a post-fire follow `grenades.md` section 3 (the device leaves on the post-fire's key).

| Weapon | Draw | Pre-fire | `fire_0` | Post-fire (key) | Device created after |
|---|---|---|---|---|---|
| `M16A1_Mine` | 1000 | 0 | 1300 (key 0) | — | ≈ 0 ms (+ updates) |
| `Hornet` | 1000 | 0 | 3500 (key 2834) | — | 2834 |
| `Claymore` | 900 | 1000 | 0 | 700 (534) | 1534 |
| `Mobile_Grenade` | 833 | 1767 | 67 | 633 (333) | 2167 |
| `Caltrops` | 300 | 600 | 67 | 366 (92) | 759 |
| `Nerve_Confuse_Mine` | 833 | 1100 | 0 | 633 (333) | 1433 |
| `Auto-Turret` | 1333 | 0 | 3666 (key 3100) | — | 3100 |
| `FirstAidKit` | 900 | 0 | 1000 (key 600) | — | 600 |
| `Medkit` | 1000 | 0 | 1500 (key 493) | — | 493 |
| `Claymore_Controller` | 500 | 0 | 500 (key 25) | — | 25 (detonation 1 frame later) |
| `Mobile_Grenade_Controller` | 833 | 0 | 1167 (key 400) | — | 400 |

### 7. Crossbows (`Assassin_Bowgun_Nor`, `Assassin_Bowgun_Expl`)

- **Bolt** (`CrossBow` record, `Class` `CCrossBow`): 5000 u/s, `Gravity` 0, `Lifetime` 10 s; alternate fire 1300 u/s. `Assassin_Bowgun_Nor`: 5-round clip, 25 total, automatic, `fire_0` 1100 ms, `alt_fire_0` 550 ms (right mouse is alt-fire, `weapon_classes.md`), reload 2300 ms, draw 1200 ms.
- **On a character**: if its hit spheres accept the bolt and the character's centre is nearer the fire position than the first world surface along the shot, the bolt sticks to the character's socket nearest the hit point and deals the direct damage: `InstDamage` 105 (alt 41) of type crossbow (45), with the range band of 1.3 (Quirk 4), through the normal armour and zone steps. It stays attached for 5 s or until the character dies.
- **On the world**: it stops at the surface point (from the fire position to its position), becomes non-solid, deals its direct damage to world models it struck, and is removed 5 s later.
- **Explosive bolt** (`Assassin_Bowgun_Expl`, record `CrossBowFunc`, `Class` `CCrossBowFunc`): sticks the same way (10 direct damage). Detonated by its controller (`Assassin_Bowgun_Expl_Controller`): the press starts a beep on the struck object and the blast follows `ExplosionDelayTime` (1.743 s) later: `AreaDamage` 100, radius 250, type claymore (twice to players). It also detonates at once when it flies farther than the weapon's `Range` (5000 u) from its fire position, when it enters the sky, when the owner dies or leaves, when the character it sticks in dies, when it stuck in something that is gone, or when shot to 0 HP.

### 8. The snowball gun (`Snow_Ball`; mode weapon)

- `Guntype` 14, 20-round clip, right mouse is alt-fire. Projectile `Snow_Ball` (impact class, 1750 u/s, alternate 2000 u/s, gravity); direct `InstDamage` 10 and blast 30 in radius 100, both of type snowball (47), which reaches the player object only (no double). Its effects belong to the snowball mode (Open question 8).

### 9. Classes left to other specs

- Heartbeat sensors and the satellite scanner (`Guntype` 11, ammo `Type` 2 without a projectile): HUD and radar, the radar spec.
- The mission bomb (`Guntype` 8, `Class` `CMissionBomb`, the only reader of `ArmDelay`): `game_modes.md`.
- `Defence_Dummy_01..03` (`Guntype` 23) and `PerkAmmoSupply` (a turret-class ammo box placed by a perk): mode and perk items, `gear_perks.md` and `game_modes_2.md`.
- AI boss weapons (`HQRAID_*`, `Powered_Suit_Boss_*`, guided `Boss_Missile`): `bots.md` and `missions.md`.

## Stock values

Projectiles (Steam build 25595979). "Twice" is the most a player in the open can take from one blast.

| Weapon | Proj. record / `Class` | Speed (u/s) | Gravity | `Lifetime` (s) | HP | Direct | Blast (`AreaDamage` / R / type) | Twice at ≤ $r_0$ | Sphere (rate × dur., R, life) |
|---|---|---|---|---|---|---|---|---|---|
| `LAW` | `Missile` / impact | 1300 | no | 10 | 50 | 0 | 40 / 550 / 7 | 80 | — |
| `RPG-7` | `Missile` / impact | 1300 | no | 10 | 50 | 0 | 43 / 550 / 7 | 86 | — |
| `M136_AT4` | `Missile` / impact | 1300 | no | 10 | 50 | 0 | 0 / 550 / 7 | **0** | — |
| `M32` | `M32_Grenade` / impact | 1550 | yes | 10000 | 300 | 0 (type 8) | 40 / 300 / 7 | 80 | — |
| `M32_IncendiaryBomb` | `M32_IncendiaryBomb` / impact | 1550 | yes | 10000 | 300 | 0 | 0 / 300 / 7 | 0 | 10 HP/s × 6 s, 200, 9 s |
| `M32_Stun` | `M32_Grenade` / impact | 1550 | yes | 10000 | 300 | 0 | 15 / 300 / 7 | 30 | — |
| `M79` | `M32_Grenade` / impact | 1550 | yes | 10000 | 300 | 0 | 40 / 300 / 7 | 80 | — |
| `MZP_1` | `M32_Grenade` / impact | 1550 | yes | 10000 | 300 | 0 | 42 / 300 / 7 | 84 | — |
| `XM_25` | `M32_Grenade` / impact | 1550 | yes | 10000 | 300 | 0 | 22 / 300 / 7 | 44 | — |
| `Flamethrower` | `Yellow_Flame` / impact | 500 | no | 1 | — | 0 (type 3) | 0 | 0 | 12 HP/s × 1 s, 75, 2 s |
| `Assassin_Bowgun_Nor` | `CrossBow` / bolt | 5000 | no | 10 | 50 | 105 / alt 41 (type 45) | — | — | — |
| `Assassin_Bowgun_Expl` | `CrossBowFunc` / bolt | 5000 | no | 10 | 50 | 10 | 100 / 250 / 38 | 200 | — |
| `M16A1_Mine` | `LandTrap` / land trap | 600 | yes | 60000 | 50 | — | 40 / 550 / 7 | 80 | — |
| `Hornet` | `Hornet` / hornet | 600 | yes | 60000 | 150 | — | 150 / 150 / 7 | 300 | — |
| `Claymore` | `Claymore` / claymore | 0 (placed) | no | 60000 | 300 | — | 60 / 120 / 38; cone 120 / 500 / 80° | 120; cone 240 | — |
| `Mobile_Grenade` | `Mobile_Grenade` / remote | 600 | yes | 60000 | 300 | — | 100 / 450 / 38 | 200 | — |
| `Caltrops` | `Caltrops` | 600 | yes | 60 | 50 | — | — | — | 3 HP/s × 5 s (bleeding), cylinder 50 × 80, 12 counts |
| `Nerve_Confuse_Mine` | `NerveConfuseMine` | 800 | yes | 60 | 300 | — | 3 (type 44, 3 s) / 250, every frame | — | — |
| `Auto-Turret` | `Auto-Turret` / turret | 600 | yes | 60000 | 350 | gun: 12 per ray, 100 rays | — | — | — |
| `FirstAidKit` | `FirstAidKit` | 600 | yes | **2** | 30 | — | heal 5 / s, 120, pool 50 | — | — |
| `LGB` → `LGB_SHELL` | `Invisible` → `LGB_Shell` | 5000 → 1550 | no → yes | 8 → 10 | 100 | 0 | 110 / 1200 / 7 | 220 | — |
| `Airstrike_Napalm` → `Napalm_Shell` ×6 | `Invisible_Napalm` → `LGB_Shell` | 5000 → 1550 | no → yes | 7.5 → 10 | 100 | 0 | 0 | 0 | 80 HP/s × 2 s, 350, 5 s |

Weapons (clip / total at spawn, cycle, reload, draw, from the records and animation files):

| Weapon | Clip / total | `RapidFireCount` | Cycle (ms) | Reload (ms) | Draw (ms) | `MovePenalty` |
|---|---|---|---|---|---|---|
| `LAW` | 1 / 3 | 1 | 1000 | 3333 | 1333 | 0.2 |
| `RPG-7` | 1 / 3 | 1 | 900 | 3000 | 1300 | 0.2 |
| `M136_AT4` | 1 / 5 | 0 | 1333 | 2166 | 766 | 0.2 |
| `M32` | 6 / 12 | 0 | 1000 | 2100 + 1000/shell + 1034 | 1200 | 0.2 |
| `M79` | 1 / 3 | 1 | 1000 | 2835 | 1000 | 0.18 |
| `MZP_1` | 1 / 4 | 1 | 1000 | 2935 | 1000 | 0.18 |
| `XM_25` | 4 / 8 | 1 | 800 | 2935 | 1000 | 0.19 |
| `Flamethrower` | 300 / 300 | 0 | 8 puffs / 734 (pre-fire 200) | 2133 | 1200 | 0.2 |
| `Assassin_Bowgun_Nor` | 5 / 25 | 0 | 1100 (alt 550) | 2300 | 1200 | 0.18 |

## Per-tick order

Client, per rendered frame: `weapon_firing.md` and `inventory.md` order. A fire key raises a shot; on the next weapon update the round is spent and the fire message (origin, direction, alt flag, throw force) is sent. The owner's client also runs the turret target search (6.7) once per frame for each of its landed turrets and sends a change.

Server, one frame (dt = 0.05 s), extending `grenades.md`:

1. Messages: a fire message creates the projectile or device (start time = previous frame time; spends the round on the server), or runs a controller (claymores and remote grenades are marked; explosive bolts start their delay). A turret-target message sets that turret's target. A death detonates the dead player's claymores and remote grenades (marks them).
2. The clock advances; every object updates in turn:
   1. Impact projectile: lifetime test (detonate in the air). Air strike marker: drop the next shell if its delay has passed. Explosive bolt: range, owner, target and delay tests.
   2. Devices: removal tests (owner gone, end of round); claymores and remote grenades marked last frame detonate now; land traps whose `active` animation has finished detonate; the hornet searches its sphere; caltrops and nerve mines apply their per-frame damage; the turret fires if 0.13 s have passed; the first-aid kit ticks; landed devices re-snap to the floor (every 0.1 s).
   3. Physics for flying objects: integration (gravity if set), then the move; a touch runs inside the move: impact projectiles detonate (blast applied at once, inside the move), devices land, bolts stick.
3. Lasting explosions refresh their over-time entries (`grenades.md` section 12).
4. Each character's over-time entries deal rate × frame time (heal entries capped by their total).
5. Removed objects leave at the end of the frame.

## Edge cases

- **Point-blank into a wall.** A rocket or launcher shell fired with a wall 30 u in front of the camera detonates in its first step, 1 u in front of the wall: the shooter's centre is about 30–40 u from the blast, well inside $r_0$, and he takes the full double blast (LAW 80, M32 80). There is no arming distance and no dud.
- **Firing straight down.** A launcher shell aimed at the shooter's feet explodes on the floor at once; the same.
- **Rocket at the sky.** Removed silently; no blast. A rocket that misses everything detonates in the air after 199 steps, 12 935 u away.
- **Owner dies with a rocket or shell in flight.** It detonates normally; damage and kills are credited to the dead player. Owner leaves the game: it explodes visually but deals no damage.
- **Shooting a rocket down.** 50 HP: one sniper round or two rifle rounds detonate it in flight, where it is, credited to the rocket's shooter.
- **Two launcher shells meeting.** Shells of the same shooter touch each other (`CanHitSameKind` 1): the first detonates on the second; its blast (40 at the second's position) does not set the second off (300 HP).
- **A mine under a door.** A landed mine re-snaps to the floor below it every 0.1 s; a closed door's polygons do not trigger it (only characters do). A door that slides over it changes nothing. A mine that landed on a moving platform follows it down but not sideways (the snap is vertical). A mine on a floor that disappears falls to the next floor.
- **Mine against a wall.** Hitting a wall in flight does not land it; it slides down and lands on the floor below.
- **Teammate on a mine.** Never sets it off, whatever the friendly-fire setting. An enemy's blast or bullets can still set it off; the damage is credited to the mine's owner.
- **Mine triggered, owner leaves during the 267 ms.** It is removed without a blast.
- **Claymore placed against a wall.** It is put at the owner's position + half his depth along the aim and dropped to the floor; nothing checks that the point is outside walls (Open question 4).
- **Claymore detonator with no claymores.** The press does nothing; the controller spends its round.
- **Hornet with an enemy below it.** An enemy whose centre is lower than the hornet (stairs below, a lower floor through a grating) never sets it off.
- **Flamethrower into a wall at 30 u.** Every puff detonates at the wall in its first step; spheres of radius 75 around a point 29 u in front: the shooter is inside but exempt; a teammate beside him burns only with friendly fire on.
- **Auto turret with two visible enemies.** See Quirk 3: the target alternates between them every client frame of the owner.
- **First-aid kit thrown far.** If the flight takes longer than 2 s, the kit is removed before it lands.
- **Crossbow bolt through a character behind cover.** A bolt whose hit sphere test passes but whose character is farther than the first wall along the line from the fire position sticks in the wall instead.

## Quirks

Keep all unless noted.

1. **The M136_AT4 does nothing.** Its ammo's `AreaDamage` is 0 and its direct damage type is 0, so no explosion object is created and nothing is damaged. The US reference data has the same. Keep as data; confirm in game (Open question 2).
2. **First-aid kits live 2 s.** The kit's projectile lifetime (2.0 s from the throw) removes it long before its 50-point pool (10 ticks of 5 at 1 s) can be spent. Keep as data; confirm (Open question 5).
3. **Turret target flicker.** The owner's client keeps scanning past the current target when it is still the nearest valid one, ends on the farthest valid candidate and switches to it; the next frame switches back to the nearest. With two enemies in view the turret alternates targets every frame and its 0.13 s shots spread between them. Keep; verify.
4. **Projectile range bands from the world origin.** The direct-hit range band measures from the fire position to the client-reported impact point, which projectile shots never send (it stays at the origin). So a crossbow bolt's direct damage depends on how far the **shooter** stands from the map origin: beyond 3500 u it is ×0.7, beyond 4000 ×0.35, beyond 5000 ×1. Keep as an option with the reference behaviour; recommend measuring to the actual hit point by default only if live tests disagree (Open question 6).
5. **The M32 shell's box is 128 × 16 × 16.** Its model's box (built for a rocket) at scale 1 makes the shell easy to hit with bullets and moves its blast distance 31 u closer in other blasts (the excess of its dims vector over 100). Keep.
6. **Land traps arm at once.** The land-trap arm timer is never started; the `ArmDelay` data belongs to the mission bomb. A mine thrown at an enemy's feet goes off on its first touch after landing. Keep.
7. **Mines are fragile in flight.** A thrown mine has 1 HP until it lands (its `Health` is applied on landing), but damage before landing cannot set it off. Keep.
8. **The flamethrower stacks.** Each puff's sphere is its own over-time entry; standing in the stream adds 12 HP/s per sphere the victim is in. Keep.
9. **Rockets kill only with help.** A direct rocket hit deals 80 (LAW) or 86 (RPG-7) to a full-health player. Keep.
10. **Others hear the primary sound for a secondary swing.** Your own secondary plays `Alt_FireSnd`; every other client plays `FireSnd` for it (section 5.1). Inaudible with the stock chainsaws (same file); keep as data-driven behaviour.

## Follow-up (2026-10-30): projectile looks and sounds in flight

Answers brief 24b of the Combat Arms plan, from the client's projectile effect, its particle smoke trail, the server projectile's set-up and launch, the projectile record reader, the client weapon-effect path and the effect manager. Applies to every projectile that has a projectile record (impact projectiles, grenades, devices), on **every client** that receives the projectile object, the shooter's included (projectiles are server objects; the client makes none of its own). Nothing here depends on the release build's special-effects detail value (3) except where said.

1. **`FXName`, `FXLoop`, `FXSmoothShutdown`.** One effect group, created **once**, when the projectile's effect starts (at launch, or when `FireDelayTime` runs out), at the projectile's position, with the **projectile object as its parent**. It is not re-emitted. Each key places itself by its own placement mode (`effects_sounds.md` "The effect file"): keys that follow or attach to the parent move with the projectile every frame, keys with fixed placement stay at the launch point. The group loops only if the record's `FXLoop` is 1; otherwise it plays its length once and ends even while the projectile flies on. Smooth shut-down only if `FXSmoothShutdown` is 1. When the projectile goes away (detonation, sky, lifetime, removal) the group is shut down: with smooth shut-down its particle systems stop emitting and their live particles finish their lives where they are; without it, every key is removed at once. Each key's own `DetailLevel` against the player's `ClientFXDetailLevel` still decides whether it is created.
2. **Sounds.** A projectile record names up to three sounds, all with the same outer radius `SoundRadius` (inner radius the default 0.07 R), volume 100, low priority, 3D, created by the client:
   - `FlySound`: started with the effect (step 1's moment), **looping**, at the projectile and moved to its position every client frame (it follows the projectile). Stopped when the projectile effect ends (detonation, sky, lifetime, removal) or when one of the two below replaces it.
   - `ActivateSound`: replaces the fly sound (stops it) on the first frame the projectile's velocity is exactly 0 (a device that has come to rest); looping, at that point, not moved after.
   - `FinishSound`: replaces the current one when the time left of the record's `Lifetime` falls below `FinishSoundTime`; played once (not looping).
   - The record's `Sound` field is **not read** for projectiles (as `grenades.md` says for grenades). A `FlySound` written as a number (`0`) reads as empty: no sound. `CreateSound` with `CreateSoundRadius` > 0 is a **server** sound at the fire position, played once at launch (skipped when the weapon is a mine or thrown weapon that plays the automatic throw call-out instead).
3. **`Flare`, `FlareSprite`, `FlareScale`, `FlareYOffset`.** With `Flare` 1 and a sprite named: an unlit sprite object (camera-facing, as sprites are) at the projectile's position raised by `FlareYOffset` u in world Y, scaled `FlareScale` in its two screen axes, moved with the projectile every frame, removed with it. At every detail value. **`Light`, `LightRadius`, `LightColor`: never drawn** (the light is never created in this build). **`SmokeTrail`, `SmokeTrailType`**: with `SmokeTrail` 1 (and the detail value not "low", so always in the release build) a particle smoke trail follows the projectile:
   - every 0.1 s a new trail segment starts; each segment fills the path from the projectile's previous position to its current one with puffs (5 interpolated steps per update; half as many unless the detail value is "high", so 2 in the release build), one particle per step, ±0.5 u jitter;
   - type 1 (or any other value): white short trail, sprite `SFX\Smoke\Spr\WhiteTrailShort.spr`, segment life 0.75 s, emitting for 0.2 s; type 2: white long, `WhiteTrailLong.spr`, 1.5 s, 0.2 s; type 3: black short, `BlackTrailShort.spr`, 0.75 s, 0.3 s;
   - particles: grey, each a random colour between (150, 150, 150) and (230, 230, 230) of 255; alpha 0.9 → 0 and scale 1 → 15 over the segment's life; drift velocity: none along x, 5–6 u/s up, uniform in [−5, 5] u/s along z, plus the world wind.
4. **A model-less projectile** (the flame puff `Yellow_Flame`, empty `Model`): nothing is drawn for the object itself (because its `ModelScale` is 0: Corrections (2026-10-30) 3); what you see is its `FXName` group (and a trail or flare if its record set them, which the flame's does not).
5. **World model in flight.** The record's `Model` with its `Skin` list (`Skin0` …, every listed skin applied in order; the stock records list one), scaled by `ModelScale`, playing its **first animation** (index 0, the same one whose box is the projectile's box). Orientation is set **once at launch**: the model's forward axis (+Z) along the fire direction, its up axis the world up made perpendicular to it (for a shot exactly straight up or down, built from world +X instead). It is **not** turned to follow a falling arc in flight. On touching the world it is turned so its forward axis is the surface normal.
6. **Flamethrower fire sound** (`Flamethrower`, `Powered_Suit_Boss_Launcher`). The weapon's fire step plays `FireSnd`, which these records do not have, so the shot itself is silent (**corrected**: they do, so each shot plays it; Corrections (2026-10-30) 1). Any sound you hear comes from the other sources, each per its own rule: the weapon's `PreFireSnd` / `PostFireSnd` on entering pre-fire and post-fire (one at a time), the view model's `sound_key`s, the projectile record's `CreateSound` (server, per puff), its `FlySound` (per puff, looping, following it) and any `PlaySound` key inside its `FXName` group. Which of these the stock records fill was not readable here (Open question 14).
7. **`FireFXName` of a launcher.** Yes, as for guns: on every launch the server sends every client a fire-only weapon event at the server's fire position, which makes the shooter's own client create the FireFX group (`effects_sounds.md` 3, own-muzzle group) and a casing if the FireFX record asks for shells, and others the third-person muzzle group; only while that point is in front of the viewer (the fire-position visibility gate of `effects_sounds.md` 2). No fire sound comes with it (it is ignored for a player's projectile; the shooter's own client played `FireSnd` itself). `NoShell`, as its name says, asks for no casing. Unlike a gun's, these come with the server's delay and from the server's fire position, not the view model's flash point.
8. **`DoSurfaceFX` for launched ammo.** The surface group is chosen by the ammo's `Type`: grenade type (2) → the surface's `GrenadeFXName` (under water `UWGrenadeFXName`); every other type, launched projectiles (1) included → the surface's `ImpactFXName` (under water `UWImpactFXName`), the same group a bullet makes. Only when the ImpactFX record has `DoSurfaceFX` and the impact point is in view (`effects_sounds.md` 6).
9. **Copied flame launchers' fire keys.** Fire keys are always the weapon's **own** first-person model's `fire_0` keys (`view_models.md` 8: the view model is the clock); a weapon whose view model is another's gets that model's keys. The boss flame launcher's numbers could not be read in this session (Open question 15; now Corrections (2026-10-30) 2).

## Corrections (2026-10-30): the flame weapons' data on Steam build 26236938

Read from the Steam install updated to build 26236938 (the attribute archive's weapons and effect-table files, the effect file, the launcher view models and sound archives), and, for 3, the server projectile's set-up and the engine's model-file change. Answers Open questions 14 to 16.

1. **Flame weapons' sound fields** (Follow-up 6 corrected: the records **do** name a `FireSnd`, so each shot is heard).
   - `Flamethrower` (and its copies `Flamethrower_CFS`, `FlamethrowerALLSTAR`, same fields): `FireSnd` `GUNS_SND_LAUNCHER\Flamethrower\fire.wav` (0.331 s, 22,050 Hz mono), `FireSndRadius` 1000; `DryFireSnd`, `SelectSnd`, `DeselectSnd`, `ReloadSnd1`, `ReloadSnd2` in the same folder; `ReloadSndRadius` and `SelectSndRadius` 300. No `PreFireSnd`, `PostFireSnd`, `Alt_FireSnd` or silenced sounds. View model `PV_LCH_Flamethrower_SH.ltb`: `prefire` 200 ms, `fire_0` 734 ms and `postfire` 333 ms carry no `sound_key`; `reload` (2133 ms) has `sound_key 1` at 466 ms and `sound_key 2` at 1300 ms; `select` (1200 ms) none.
   - `RMS12`, `RMS12_Bounty`: `FireSnd` `GUNS_SND_LAUNCHER\RMS12\fire.wav` (0.331 s), `FireSndRadius` 1000; dry-fire, select, deselect and `ReloadSnd1..3` name `GUNS_SND_LAUNCHER\AK47\*`, a folder the install does not ship, so those are silent. Its view model's `fire_0` (134 ms, one `FIRE_KEY` at 0), `select` (800 ms) and `reload` (1834 ms) have no `sound_key`.
   - `Powered_Suit_Boss_Launcher`: `FireSnd` `GUNS_SND_ETC\Powered_Suit_Boss_Launcher\fire.wav` (0.331 s), `FireSndRadius` 1000; `SelectSnd` and `ReloadSnd1` both `select.wav` of that folder (0.355 s), radii 300; no dry-fire, deselect, pre-fire or post-fire sound. View model keys in 2.
   - The projectile all three fire, `Yellow_Flame` (`ProjectileFX6`): `FlySound` written `0` (no fly sound), `Sound` empty (not read anyway), `SoundRadius` 900 (unused), no `ActivateSound`, `FinishSound`, `CreateSound` or `CreateSoundRadius`; `Light` 0, `Flare` 0, `SmokeTrail` 0, `FXSmoothShutdown` 1. Its `FXName` group `Flame_Yellow` (phase 100 ms) is six `ParticleSystem` keys (`UpdatePos` Fixed, smooth shut-down) and no `PlaySound` key.
   - So the only flame sound is `FireSnd`, started by the shooter's client at each `FIRE_KEY` shot, under the one-fire-voice rule of 5.1 (each start stops the previous copy of the same shooter's same weapon). Flamethrower held: starts at 200, 300, …, 900 ms, then 934, 1034, … (case 14's puff times); each copy is heard for 100 ms (the 700 ms → next-loop gap: 34 ms plus frame rounding); released, the last copy plays its whole 331 ms. What other clients hear of a player's projectile shot is unchanged (Follow-up 7).
   - Expect: test cases 54 and 55.

2. **The boss flame launcher's view model** (Open question 15). `PV_LCH_Powered_Suit_Basic_Hephaestus_SH.ltb` (also its `HHModel` name in `GUNS_M_HH`): `select` 1200 ms with `sound_key 4` at 0 (the select sound); `prefire` 600 ms; `fire_0` **160 ms** with six `FIRE_KEY`s at **0, 33, 64, 96, 128 and 160 ms**; `fire_1` identical; `postfire` 600 ms; `reload` 2100 ms with `sound_key 1` at 485 and 1390 ms. The key at 160 ms is the animation's last key, so it fires before the animation counts as done (`animation.md`: done at length + 1 ms). Held from the press at rate 1: puffs at 600, 633, 664, 696, 728, 760 ms, then each restart of `fire_0` repeats the six (the 160 and the next 0 ms key one restart apart, about a frame). Clip 999 (`ShotsPerClip`), ammo `ProgDamage` 10 HP/s (75 u sphere for 2 s, 1 s entries, burn), `MovePenalty` 0.2, `Guntype` 9.
   - Expect: test case 56.

3. **A projectile record with an empty `Model`** (Open question 16, Follow-up 4 refined). The server creates every projectile object with the placeholder model `MODELS\DEFAULT_PROJECTILE.LTB` (not visible yet); at set-up it sets the record's model file, skins and `ModelScale`, takes the box from the model's first animation's dimensions times that scale, and makes the object visible. An empty model name leaves the already bound placeholder in place (changing to an empty file name on a model that has one is a no-op), so the placeholder stays bound and is drawn **at the record's `ModelScale`**. The placeholder (in `MODELS.rez`, unkeyed, 1,720 bytes, the same file as the engine's fall-back `models\default.ltb`): one piece of 52 triangles spanning about 6 × 6.2 × 0.9 u, no skin (untextured), one animation `x` of 6566 ms with dimensions (3, 3, 3).
   - `Yellow_Flame` has `ModelScale` (0, 0, 0): the placeholder is scaled to nothing and the box is (0, 0, 0). **Nothing is drawn** for a flame puff (Follow-up 4 holds for it).
   - `Scorpion_Venom` (`ProjectileFX68`), the only other record with an empty `Model`, has `ModelScale` (0.35, 0.35, 0.35): it **does** draw the placeholder at that scale (about 2.1 u across, untextured) with box (1.05, 1.05, 1.05).
   - Expect: test case 57.

## Corrections (2026-10-31): launcher muzzle looks, smoke puffs, projectile sounds

From the client weapon effect (fire position, visibility test, muzzle effect, fire-only path), the server's fire message handling and projectile set-up, the engine's compressed-position encoding, the effect manager's placement of parented groups, the client projectile effect's sound handling and the projectile record reader, checked against the Steam build 26236938 data. Answers brief 24b's open question (b) and the plan's "activate, finish and create sounds"; corrects Follow-up 7 and case 52. Where the sections above disagree, this wins.

1. **Launcher FireFX: who sees it, where, when** (Follow-up 7 corrected).
   - The fire position the server gets for a player's shot is the shooter's **camera position at the moment his client fired** (sent in his fire message), or, if he fired from a third-person camera, his held weapon's flash point. For a launched projectile the server sends every client one fire-only weapon event with that point as its fire position. Positions in that event are **compressed**: each axis is quantised to a grid over the world's bounding box (2¹⁹ cells on x and z, 2¹⁸ on y; the receiver gets the cell's centre), so the point arrives off by up to half a cell (about 0.01–0.03 u on a typical map) in an arbitrary direction. The event's impact position is never set, so it is the world origin (clamped into the world box).
   - **On the shooter's own client** (first-person camera): the fire point is used as received (no socket lookup). The visibility test of section 2 is made from the camera **at the time the event arrives** (the round trip later): direction d = fire point − current camera; if |d| < 0.001 u it counts as straight up; seen when forward · d̂ ≥ 0.6. So the shooter sees his own FireFX group only if, between firing and the event's arrival, his camera has moved so that the old eye point lies within 53.13° of where he now looks (in practice: he moved backward), or, standing still, when the quantisation error happens to point ahead of him (or, in the "straight up" case, when he looks at least 36.87° up). Most launches from a standing or forward-moving shooter show nothing. In a third-person camera his own held weapon's `Flash` socket is used as for others.
   - When it is seen, the own client creates the ammo's FireFX group (`M136_Rocket` → group `test`) **parented to the shooter's character**, not at the fire point: the group starts at the character's position and each key places itself by its own mode. `test`'s two keys (the smoke puff and the light) are socket-attached to `Flash`, which the character lacks, so they use its attachment's (the held third-person launcher's) `Flash` socket, offset 4 u (puff) and 0 u (light) along the character's forward axis. No casing (`EjectShells` 0), no fire sound (ignored for a player's projectile).
   - **On every other client**: the fire point is replaced by the shooter's held weapon's `Flash` socket (`Flash01` for a left-hand dual weapon), at its current animated place, when the socket exists; the visibility test uses that point. If seen, they create the third-person muzzle group (the muzzle part's `HHMuzzlefxname`, else the weapon's `HHMuzzleFXName`: `WEAPON_HH` for LAW, RPG-7, M136_AT4 and M79; none for M32 and the flamethrower) at that point, not in first-person chase view, oriented with its +Z toward the event's impact position, i.e. **toward the world origin**, not along the shot (Quirk). Others never get the ammo's FireFX group.
   - Expect: test cases 58–60.

2. **Smoke-trail puffs** (Follow-up 3 corrected): the alpha 0.9 → 0 and scale 1 → 15 run over each **puff's own 10 s life**, not over the segment's, so a visible puff only reaches a scale of about 3.4 (long) or 2.3 (short); a puff's square is 4·(2000 / screen width in pixels)·scale u; the segment's whole alpha falls from 1 to 0 over its life after it stops emitting; and the trail's sprite files are not in the install, so puffs are untextured grey squares. Full rules: `effects_sounds.md` Follow-up (2026-10-31) 2. Case 50 stands except its "scale 1 → 15".

3. **Activate, finish and create sounds** (Follow-up 2 refined). A record's `FlySound`, `ActivateSound` and `FinishSound` each enable their step only when the name is non-empty. All three are 3D at radius `SoundRadius` (inner radius the default fraction), volume 100, low priority, made by the client; at most one of them plays at a time (starting one stops the current one). Checked every client update of the projectile effect, in this order:
   - If the finish sound has started, nothing more changes.
   - The fly sound's position follows the projectile (only the fly sound moves).
   - **Activate**: if the record has one, the current sound is not already the activate sound, and the projectile object's velocity is exactly zero, the activate sound starts, **looping**, at the projectile's position then (not moved after). This also fires before launch while a projectile waits out its `FireDelayTime` with zero velocity; the fly sound then replaces it at launch, and it returns once the projectile comes to rest.
   - **Finish**: if the record has one and the client's time is past T₀ + `Lifetime` − `FinishSoundTime` (T₀ = when this client created the projectile's effect, i.e. first received the object; `FinishSoundTime` default 0), the finish sound starts **once** (not looping) at the projectile's position then, replacing the current one. It is cut when the projectile effect is removed, so it is heard for at most `FinishSoundTime` (less if the server removes the projectile earlier).
   - **Create**: on the server, once, when a player's shot makes the projectile: if `CreateSoundRadius` > 0 and `CreateSound` is non-empty, a server sound at the shot's fire position (section 1's point), radius `CreateSoundRadius`, volume 100, medium priority, played once; skipped (in favour of the automatic throw or mine call-out) when the weapon is a mine or thrown weapon whose weapon record has `PlayAutoThrowSound` ≠ 0 (default 1). Not for shots by AI or turrets.
   - Stock records using them: `AssassinBomb_Projectile` (`ProjectileFX22`): fly `GUNS_PT\Snd\Grenade.wav`, activate `GUNS_PT\Snd\Snow_Fly.wav`, finish `GUNS_PT\Snd\Grenade.wav`, `FinishSoundTime` 5, `SoundRadius` 1000, `Lifetime` 30. `NerveConfuseMine` (`ProjectileFX34`): fly `GUNS_PT\Snd\Bomb_Beep.wav`, activate `GUNS_SND_ETC\Nerve_Confuse_Mine\ActiveLoop.wav`, finish `GUNS_SND_ETC\Nerve_Confuse_Mine\AutoFinish.wav`, `FinishSoundTime` 2.4, `SoundRadius` 600, `Lifetime` 60. `Caltrops` (`ProjectileFX27`): `CreateSound` `GUNS_SND_Throwing\Caltrops\fire.wav`, `CreateSoundRadius` 300; its weapon `Caltrops` has `PlayAutoThrowSound` 0, so the create sound plays. All these files ship. No other record names any of the three.
   - Expect: test cases 61–63.

## Test cases

Common setup unless stated: server dt = 0.05 s; flat floor y = 0 (hardness irrelevant for impact projectiles); stock Steam data; shooter's camera at (0, 60, 0) looking along +Z; spread draws 0 (direction exactly the aim); victims standing players with 100 HP whose centre is the stated distance from the blast point, in the open, armour points 0, no gear; friendly fire off; step 1 is the physics step of the frame the projectile is created in.

1. **Rocket flight.** LAW fired level. → After step k (k ≤ 199, nothing hit): position (0, 60, 65k). Wall plane z = 500 facing −Z: contact in step 8 at (0, 60, 500); blast at (0, 60, 499).
2. **Rocket blast.** LAW blast at 100 u from a victim → 80 (two applications of 40); 137.5 u → 80; 300 u → 48.48; 500 u → 9.70; 550 u → 0. RPG-7 at 100 u → 86; 300 u → 52.12.
3. **Rocket direct hit.** LAW fired at a standing victim 400 u away, chest sphere on the line → blast at the chest hit point; victim takes 80 and survives with 20; no cover test for him even if the blast point is inside a wall polygon.
4. **AT4.** M136_AT4 rocket hitting a victim directly or 50 u away → no damage; impact effect only.
5. **Rocket at the sky.** LAW fired at a sky polygon 300 u away → removed in step 5, no effect, no damage.
6. **Rocket lifetime.** LAW fired into an empty map → detonates in the air at the start of the 200th frame, at (0, 60, 12935).
7. **Shooting a rocket.** A LAW rocket in flight hit by rifle rounds of 35 → the second round detonates it (70 ≥ 49) at its position; blast credited to the rocket's shooter.
8. **Launcher shell, level.** M32 fired level. → Steps 1–5 as `grenades.md` case 1 ((0, 59, 77.5) … (0, 15, 387.5)); step 6 touches the floor at (0, 0, 442.857) with post-gravity velocity (0, −480, 1550); blast at (0, 0.296, 441.902). A victim centred at (0, 40, 600) is 163.01 u from it → 48.71 in total.
9. **Launcher blast.** M32 blast at 75 u → 80; 100 u → 71.11; 200 u → 35.56; 300 u → 0. XM_25 at 100 u → 39.11; MZP_1 at 50 u → 84.
10. **No arming distance.** M32 with a wall plane z = 30 facing −Z → contact in step 1 at (0, 59.61, 30); blast 1 u back along the direction; the shooter (centre (0, 20, 0)) is about 49 u from it → 80 to himself.
11. **M32 automatic fire.** Fire held with 6 in the clip, 12 total, rate 1, exact time → shells at 0, 1000, …, 5000 ms; clip 0 at 5000 ms; reload starts when the fire animation ends (6000 ms) and loads shells at 8100, 9100, …, 13100 ms; idle at 14134 ms; clip 6, total 6.
12. **M32 reload interrupted.** Clip 2, total 12, reload at 0 (shells complete at 3100, 4100, … ms); fire pressed at 4000 ms → the reload stops with clip 3 (the shell in progress is lost), one shell fires at once → clip 2, total 11.
13. **Incendiary shell.** M32_IncendiaryBomb hits the floor 100 u from a victim who stays → no instant damage; burn 10 HP/s while inside the 200 u sphere (9 s) and 6 s after: about 150 HP lost by 15 s (dead at about 10 s). The shooter standing in the sphere also burns.
14. **Flamethrower puffs.** Fire held from t = 0 → puffs at 200, 300, …, 900 ms (pre-fire 200, then keys every 100 ms in the first loop), the next loop from 934 ms: 934, 1034, … (frame rounding aside). 300 puffs, then the clip is 0.
15. **Flame puff in the air.** One puff fired level into empty space → detonates at the start of frame 20 at (0, 60, 475); a sphere of radius 75 lives 2 s there; a victim centred 50 u from that point for the whole time takes 12 × 3 = 36 (2 s of refresh + 1 s tail); at 80 u → 0.
16. **Flame stacking.** A victim inside the spheres of three puffs at once for 1 s → 36 HP in that second.
17. **Flame self.** The shooter inside his own spheres → 0 damage. A teammate inside: friendly fire off → 0; on → full.
18. **Land mine.** M16A1_Mine thrown level at 600 u/s from (0, 60, 0) → lands on the floor in the step that crosses y = 0, sticks; an enemy walks onto it at t → `active` 267 ms, blast at the start of the first frame after t + 267 ms: victim 0–137.5 u from the mine → 80; at 300 u → 48.48. The owner or a teammate walking over it → nothing.
19. **Mine against a wall.** Thrown at a wall 100 u away → no landing at the wall; lands on the floor below.
20. **Mine chain.** A landed M16A1_Mine 150 u from an M67 blast (60 · m(150) = 53.3 ≥ 49) → set off; it blows 267 ms later, credited to the mine's owner.
21. **Hornet.** Landed hornet: no trigger in the first 666 ms (`ready`); then an enemy whose centre is 90 u away horizontally and 40 u higher, in the open → set off; `active` 534 ms; blast 150 / 150: at 30 u → 300; at 100 u → 133.33. The same enemy 40 u **lower** → no trigger. 110 u away → no trigger.
22. **Claymore cone.** Claymore facing +Z at the origin, detonated: victim centre at (0, 40, 100) (dot 0.928) → 240; at (0, 40, 300) → 2 × 120 · m(302.7, 500) = 127.6; at (300, 40, 0) (dot 0, outside the cone, beyond 120) → 0; at (100, 0, 0) (outside the cone) → 2 × 60 · m(100, 120) = 26.67; at (0, 0, −50) → 93.33.
23. **Claymore detonator.** Two claymores placed, controller fired → both blow on the server frame after the fire message. Owner killed instead → both blow on the frame after his death. Owner leaves → both removed, no blast.
24. **Remote grenade.** Landed `Mobile_Grenade`, controller fired → blast 100 / 450 claymore type in all directions: victim at 100 u → 200; 200 u → 148.15; 400 u → 29.63.
25. **Caltrops.** Landed caltrops; an enemy whose centre is 30 u away horizontally and 40 u above stands there 3 s → a bleeding entry of 3 HP/s refreshed every frame: 9 HP by 3 s, then 15 HP more over the following 5 s after he leaves; movement penalty +0.4 for 5 s after each damage message. Centre 90 u above → no damage. 60 u away → no damage.
26. **Caltrops count.** Three enemies each standing in caltrops for 4.5 s → counted 5 times each (0, ~1.05, ~2.1, ~3.15, ~4.2 s at 20 Hz) → 15 ≥ 12: the caltrops disappear at the 12th count.
27. **Nerve mine.** Thrown against a wall → active from that touch; every frame an enemy within 250 u (through walls) receives the type 44 effect (3, 3 s). A second nerve mine thrown → the first is removed on its next update.
28. **Turret target.** Landed turret, one enemy 500 u away in view, 20° above the sensor → targeted, shots every 0.13 s, 12 damage each within 800 u (8.4 at 850 u); 100 shots then removed. An enemy 70° above → not targeted. 900 u away → not targeted.
29. **First-aid kit.** Kit lands at 0.3 s on a floor with an injured teammate (centre 50 u away, above the kit) → +5 HP at 1.3 s; removed at 2.0 s (no second tick at 2.3 s). An enemy beside it → never healed. A teammate at full health → not healed, pool unchanged.
30. **Medkit.** Use at 40 HP → +11 HP/s for 3 s from the server frame the message arrives: 73 HP after 3 s; at 90 HP → capped at 100.
31. **LGB.** Designator fired at a floor point P 1000 u away → the marker touches the floor at about 0.2 s; at 8 s one shell is created 3000 u above P falling straight down at 1550 u/s; it hits P about 1.3 s later (gravity with the ¼ term); blast 110 / 1200: victim at 200 u → 220; 600 u → 146.67; 1000 u → 48.89.
32. **Napalm.** `Airstrike_Napalm` → six shells 200 ms apart at P, P + 200, …, P + 1000 along the course; each leaves a 350 u burning sphere for 5 s with 80 HP/s × 2 s entries.
33. **Crossbow bolt.** `Assassin_Bowgun_Nor` fired from (100, 60, 0) at a torso 1000 u away → sticks after 1 step (5000 u/s covers 250 u per step: 4 steps); 105 × roll, × band for the shooter's distance from the origin (100.2 u → ×1.0): 94.5–115.5 torso, head ×6. The same shot fired from (4500, 60, 0) → band ×0.35 (Quirk 4).
34. **Explosive bolt.** Stuck in a wall; controller fired at t → blast at about t + 1.743 s: 100 / 250 claymore type: victim at 50 u → 200. Owner dies before → blast at once.
35. **Melee reach.** `DomPedroSword` primary at a victim 155 u away → hits (85 × roll; head ×6 = 459–561); at 165 u → miss. `M9` at 110 u → miss.
36. **Melee double secondary.** `Machete` right button, both hits on the torso of a full-health victim, roll 1.0 → 70 at 667 ms and 70 at 1167 ms after the swing starts (dead on the second). From behind (rear sector) → 91 each.
37. **Chainsaw secondary.** Right button held on a victim in reach → hits at 334, 386, 437, 489, 540, 591, 643 ms of each 900 ms swing, 15 each (105 per swing at roll 1.0).
38. **No lock-on, no back-blast.** LAW fired with an enemy 30 u behind the shooter → the enemy takes nothing; the rocket flies along the aim regardless of targets.
39. **Round end.** A landed mine, a claymore, caltrops and a burning sphere exist when a round is decided → all removed on their next update after the mode enters the end-of-round state; a rocket in flight at that moment still detonates and damages.

Swing sounds (section 5.1). Rate 1, exact timing unless stated; "starts" means the attacker's own client starts the sound, local.

40. **Chainsaw secondary sound.** `Chainsaw`, right button held from swing 1 → `Alt_FireSnd` starts at 334, 386, 437, 489, 540, 591, 643 ms and again at 1234, 1286, … ms; at each start the previous copy stops (heard lengths 52, 51, 52, 51, 51, 52 ms and 591 ms); `ReloadSnd1` starts at 0 and 900 ms and both play during 900–1230 ms. Exactly one fire-voice copy is playing at any time after 334 ms.
41. **Chainsaw secondary released.** One right-button swing, then nothing → seven starts as in case 40; the last plays 643–3543 ms (2.9 s, uncut).
42. **Chainsaw primary.** Left button held → `FireSnd` starts at 257, 857, 1457 ms …, each heard for 600 ms; released after the first swing → heard 257–3157 ms.
43. **Shared voice.** `M9`, a primary swing then at once a secondary (button changed) → the secondary's `Alt_FireSnd` start at 367 ms of the second swing stops the primary's `FireSnd` copy started at 402 ms of the first if it is still playing; a different weapon's fire sound (switch to a pistol and shoot) does not stop it, nor does another player's swing.
44. **What others hear.** An enemy 500 u away swings a weapon whose `Alt_FireSnd` differs from its `FireSnd` (data test), secondary → his client starts `Alt_FireSnd`; yours plays `FireSnd`, 3D at his fire position with his `FireSndRadius`, once per relayed hit, each stopping the previous. A hit the server refuses for `FireDelay` → nothing for others.
45. **Miss, hit, flesh.** `M9` primary at empty space → two fire sounds, no impact sound, no effect. At a Wood wall 80 u away (both hits) → two fire sounds, and per hit `melee\wood\impact1.wav` or `impact2.wav` with probability ¼ each (silent ½), 3D at the hit point, gain 1 − (80² − 10.71²)/142.29² = 0.690; each impact starts before that hit's fire sound. At an enemy's torso 80 u away → `melee\flesh\impact1..2.wav` (same odds) plus his server hurt sound and your ding; his head → also `HitHead`.
46. **Merged keys at low frame rate.** `Chainsaw` secondary with frames of exactly 1000/15 ms and the swing starting on a frame boundary → keys 334 and 386 pass in the same advance, as do 540 and 591 → 5 hits and 5 fire-sound starts per swing (at the updates after 400, 466.7, 533.3, 600 and 666.7 ms), not 7.
47. **Impact out of earshot.** `DomPedroSword` primary hitting a wall 150 u away → no impact sound for the attacker (gain 0); at 140 u → gain 0.038; a bystander 50 u from the hit point → gain 0.88 (if the drawn slot is defined).
Projectile looks and sounds (Follow-up (2026-10-30)).

48. **Fly sound follows.** A record with `FlySound` f, `SoundRadius` 1200, `Velocity` 1300, fired level → f starts at launch, looping, 3D (R 1200, r 84), its position the projectile's every client frame; a listener 300 u to the side of the line hears its gain change as it passes; detonation at 2 s → f stops in that frame. `FlySound` `0` → no sound.
49. **Attached group.** `FXName` g with `FXLoop` 0, `FXSmoothShutdown` 1, a follow-mode particle key: g starts once at launch and moves with the projectile; at detonation the emitter stops and live particles finish their lives in place. `FXSmoothShutdown` 0 → every key gone in the detonation frame.
50. **Light and trail.** `Light` 1, `LightRadius` 200 → no light. `SmokeTrail` 1, type 2 → a new segment every 0.1 s, each living 1.5 s with alpha 0.9 → 0 and scale 1 → 15, `WhiteTrailLong.spr`.
51. **Orientation.** A rocket fired 30° up → its model's forward axis 30° up for the whole flight even if gravity bends the path; fired straight up → up axis built from world +X.
52. **Launcher muzzle.** LAW fired → one fire-only event: own client shows the `M136_Rocket` group once at the server's fire position (none if behind the viewer), no casing, no second fire sound.
53. **Surface group.** A projectile-type (1) round hitting Wood with `DoSurfaceFX` → `Impact_Wood`; a grenade-type (2) ammo on Wood → `Grenade_Dirt`.

Flame weapons' data (Corrections (2026-10-30), Steam build 26236938).

54. **Flame sound fields (data).** `Flamethrower`: `FireSnd` `GUNS_SND_LAUNCHER\Flamethrower\fire.wav`, `FireSndRadius` 1000, no `PreFireSnd`/`PostFireSnd`; `RMS12`: `FireSnd` `GUNS_SND_LAUNCHER\RMS12\fire.wav`; `Powered_Suit_Boss_Launcher`: `FireSnd` `GUNS_SND_ETC\Powered_Suit_Boss_Launcher\fire.wav`, `SelectSnd` = `ReloadSnd1` = that folder's `select.wav`. `Yellow_Flame`: no fly, create, activate or finish sound; `Flame_Yellow` has 6 keys, all `ParticleSystem`. Each `fire.wav` is 0.331 s.
55. **Flamethrower held 1 s, rate 1, exact timing.** Fire sound starts at 200, 300, …, 900 ms (each stopping the previous) and 934 ms; released at 1000 ms → the copy started at 934 ms plays to 1265 ms; no other weapon sound in that time.
56. **Boss launcher keys (data).** Its view model's `fire_0` is 160 ms with `FIRE_KEY`s at 0, 33, 64, 96, 128, 160; `prefire` and `postfire` 600 ms; held from 0 at rate 1 → the first six puffs at 600, 633, 664, 696, 728 and 760 ms.
57. **Model-less projectiles.** A `Yellow_Flame` puff in flight → no model drawn, box (0, 0, 0). A `Scorpion_Venom` projectile → the placeholder model `MODELS\DEFAULT_PROJECTILE.LTB` drawn at scale 0.35, untextured, box (1.05, 1.05, 1.05).

Launcher muzzle looks and projectile sounds (Corrections (2026-10-31)). Case 52 is replaced by 58–60.

58. **Own launcher FireFX (logic).** LAW fired in first person, camera at E looking along +Z; the event arrives 100 ms later. Shooter stood still, received point E + (0, 0, 0.012) → seen (straight ahead): group `test` made, its puff at the held launcher's `Flash` socket + 4 u along the character's forward, its light at the socket. Received point E + (0, 0, −0.012) → not seen, nothing. Shooter walked backward at 100 u/s (camera now E − (0, 0, 10)) → seen. Strafed sideways 10 u → not seen (forward · d̂ = 0). Received point exactly E (|d| < 0.001) → seen only if looking at least 36.87° up.
59. **Others' launcher muzzle (logic).** Another player 500 u ahead fires a LAW toward you, his held launcher's `Flash` socket in your view → `WEAPON_HH` at that socket with its +Z toward the world origin (not along his shot); no `test` group, no casing, no fire sound from this event. An M32 shot → nothing from the event.
60. **Compressed fire point (logic).** On a world whose box spans 8192 u in x, a fire point is received within 8192 / 2¹⁹ / 2 = 0.0078 u of where it was sent on that axis.
61. **Nerve mine sounds (logic).** `NerveConfuseMine` thrown, client effect created at t = 0, landing (velocity 0) at 1.2 s: `Bomb_Beep.wav` loops and follows from launch; at 1.2 s it stops and `ActiveLoop.wav` starts looping at the landing point; at 57.6 s (60 − 2.4) `AutoFinish.wav` starts once, replacing it; the object removed at 60 s cuts it (2.4 s heard). Radius 600 throughout.
62. **Assassin bomb finish (logic).** `AssassinBomb_Projectile` still in flight at 25 s after its effect was created → `Grenade.wav` (finish) starts once at its position then, replacing the fly sound; it is not moved afterwards.
63. **Create sound (logic).** A player throws `Caltrops` → one server sound `GUNS_SND_Throwing\Caltrops\fire.wav` at his fire position, radius 300, no throw call-out. A weapon of mine or throw type with `PlayAutoThrowSound` 1 and a record with a create sound → the call-out only. An AI throwing → neither from this rule.

## Open questions

1. **Contradiction with `grenades.md` section 15.** It says nothing in the round logic touches lasting explosions, so fire and gas spheres carry over into the next round. The lasting explosion object removes itself on its next update once the mode's mission state is "object pre-end", which the round-based modes enter when a round is decided; land traps, claymores, caltrops, nerve mines, first-aid kits and air strikes do the same. So spheres (and devices) do **not** carry over. Bouncing grenades and impact projectiles have no such check and do carry over. Check: throw an M-23F just before a round ends and watch whether the fire survives the respawn.
2. **M136_AT4 damage.** Confirm in game that the AT4 does no damage (Quirk 1); if it does, the live data or server has a value this build lacks.
3. **Devices against walls.** Land traps and remote grenades do nothing of their own on a wall touch. Whether the engine's response slides them down the wall or stops them dead (the projectile has the "no sliding" flag) was not traced. Throw a mine at a wall and see where it lands.
4. **Claymore placement inside walls.** Placement does not test the line from the owner to the placement point; the floor drop may put the claymore behind a thin wall. Test against a wall.
5. **First-aid kit lifetime.** Confirm the 2 s removal (Quirk 2) in game: a kit should heal at most twice.
6. **Projectile range bands** (Quirk 4). Confirm on a crossbow: the same body shot from far away and near the map origin should differ. Also whether any client sends an object impact for projectile shots.
7. **Turret flicker** (Quirk 3). Two enemies in front of a turret: count hits on each.
8. **Effects left to other specs.** The type 44 (camera reverse) effect of the nerve mine, the stamina kit's type 39 entry, the snowball and ice types (47, 48), `M32_Stun`'s stun beyond the move penalty, the infection-mode stun launchers, the designators' world cool-down and HUD gauge, `RMS12` and `Claymore_old` (data that does nothing), and whether the 0-damage freeze (type 8) of a launcher shell's direct hit has any effect.
9. **Ragdoll push.** Rocket and grenade ammo carry `BLPhysicsDirectionVector` and `BLPhysicsForceRatio`; they appear to drive only the corpse's physics. Confirm that live players are not pushed by rockets.
10. **Water.** Impact projectiles with gravity presumably use the liquid gravity and viscosity of `grenades.md` section 7 (the same engine path); not traced for this class.
11. **Server frame rate.** All flight numbers assume 20 Hz (`grenades.md` Open question 6).
12. **Contradiction with `weapon_classes.md` section 8.** It lists the rocket class as LAW, RPG-7 and M136_AT4 only; in the Steam data `Guntype` 9 also holds the flamethrower and the M79, MZP_1 and XM_25 launchers, so the server's rocket-style reload bookkeeping and the "last primary" rule apply to them too. Its LAW numbers (`AreaDamage` 40, radius 550, 1300 u/s, 10 s) agree. Its "M136_AT4 0 area damage" (its Open question 8) is confirmed for the Steam data.
13. **Contradiction with `inventory.md` section 1.** It puts rockets and the M32 in the special class (key 5) and launchers in the primary class; the class comes from the weapon order lists, which this session did not re-read for each special weapon. Check the `[WeaponOrder]` lists of the Steam data for `LAW`, `M32`, `Flamethrower` and the devices.
14. **Answered (2026-10-30, build 26236938)**, Corrections 1: every flame record names a `FireSnd` (so the shot is heard); nothing else sounds. Was: **Flame sounds in the data.** Which of `PreFireSnd`, `PostFireSnd`, view-model sound keys, `CreateSound`/`CreateSoundRadius`, `FlySound` and `PlaySound` keys in `Flame_Yellow` the `Flamethrower`, `RMS12` and `Powered_Suit_Boss_Launcher` records fill (Follow-up 6); read the install's `WEAPONS.TXT`, `FX.TXT` and effect file.
15. **Answered (2026-10-30, build 26236938)**, Corrections 2: `fire_0` 160 ms, keys 0, 33, 64, 96, 128, 160. Was: **Boss flame launcher keys.** The `fire_0` length and key times of `Powered_Suit_Boss_Launcher`'s first-person model (Follow-up 9); read the install's model.
16. **Answered (2026-10-30)**, Corrections 3: the placeholder stays bound at the record's `ModelScale`; the flame's is 0, so nothing shows (but `Scorpion_Venom` shows it). Was: **Model-less objects.** A projectile with an empty `Model` is assumed to draw nothing (Follow-up 4); confirm that the engine's placeholder projectile model does not show for a flame puff.
