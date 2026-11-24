# Day of Defeat: Source: weapons (deltas from CS:S)

Source basis: the dedicated server's per-weapon scripts
(`scripts/weapon_*.txt`, all 29 in the manifest read in place, their
fields compared), the server's cvar list, the view-model list, and
measurements on the dedicated server (2026-11-23) with puppet bots
driven by a SourceMod plugin (fire intervals, magazines, deploy and
shoulder timings, stamina while firing; damage in damage.md), and
again with `tools/dods_probe` (draw and reload times, sights' fields of
view, fire select, the MGs' deploy and reload, grenades, rifle grenades
and rockets: `measure.py guns fov fireselect mg frag fragspeed rocket
riflegren`), and on 2026-11-24 with the plugin also logging the
server's per-shot bullets message (its spread field): spread by stance
and movement, when a gun refuses to fire, the SMGs' stock strike,
partial reloads, the launchers' and rifle grenades' reloads, rockets on
the world, the throw's speed and the thrower's velocity, grenade timings
and bounces, deploying on ledges and the MGs' limits (`measure.py
spread nofire stock partial launchers rocketworld throw nadetiming
bounce mgyaw mgside ledge`; what narrows the yaw limits later that
day, `mgsweep`). The
scripts' values are data the game exposes; they are given to check
against, and the game supplies them at run time. What the scripts do
not hold (the recoil curve) is marked unmeasured where no measurement
exists; the spread pattern is spread.md's.
Status: draft (scripts read, measured 2026-11-23 and 2026-11-24; recoil
unmeasured; the spread pattern in spread.md)

## Summary

DoD:S's guns are Source hitscan guns like CS:S's, with fixed class
loadouts (classes.md) instead of a shop, no damage falloff, and a few
mechanics CS:S lacks: iron sights and scopes on the alt fire, machine
guns that are only accurate when deployed (prone or on a ledge),
rockets that must be shouldered, rifle grenades as a separate weapon,
semi-automatic rifles that reload only when empty, and the SMGs' alt
fire as a melee strike.

## Units and conventions

Source units, seconds, degrees. Tick 0.015 s: a fire interval is
rounded **up** to whole ticks (measured: 0.085 s → 6 ticks = 0.09 s;
0.1 s → 7 ticks = 0.105 s).

## The scripts' fields

KeyValues, one `WeaponData` block per weapon, plus `SoundData` and
`TextureData` blocks (HUD sprites, as CS:S's). Fields that carry
gameplay, and where they map in our weapon model
([docs/weapon-data.md](../../docs/weapon-data.md)):

| Field | Meaning | Ours |
|---|---|---|
| `WeaponType` | Rifle, Sniper, SubMG, MG, Pistol, Sidearm, Melee, Grenade, RifleGrenade, Bazooka, Bomb, Camera | the gun's class and hooks |
| `Damage` | damage per bullet before hit zones (damage.md) | `[damage] amount` |
| `BulletsPerShot` | bullets per shot | `[hitscan] pellets` |
| `Accuracy` | spread, standing still, alt mode off | `[hitscan] spread` (base) |
| `SecondaryAccuracy` | spread with the alt mode on (sights, scope, deployed) | the alt mode's spread |
| `AccuracyMovePenalty` | spread added while moving | movement penalty |
| `Recoil` | view kick per shot (degrees, unmeasured shape) | `[recoil]` |
| `Penetration` | 1 on every gun (walls untested) | penetration power |
| `FireDelay` | seconds between shots | `[trigger] cycle` |
| `SecondaryFireDelay` | the alt fire's interval (BAR, StG44: 0.3) | the alt mode's cycle (open question 3) |
| `clip_size` | magazine (−1 = no magazine) | `[magazine] clip` |
| `DefaultAmmoClips` | spare magazines at spawn | reserve = clips × clip_size |
| `AmmoPickupClips` | magazines from an ammo pickup (MGs: 1) | — |
| `default_clip`, `item_flag_exhaustible` | grenades: one each, gone when thrown | grenade count |
| `CanDrop` | 0: never dropped (pistols, melee, grenades) | drop rule |
| `ViewModelFOV` | 45 on every gun | view-model FOV |
| `vm_normal_offset`, `vm_prone_offset`, `vm_ironsight_offset` | view-model offsets (x y z) per stance | view-model placement |
| `Tracer` | 0 none, 1 every bullet (MGs), 2 (BAR, StG44, SMGs: a tracer interval; open question 5) | tracers |
| `deployedmodel`, `reloadmodel`, `pronedeployedreloadmodel` + `ALTWPN_CRITERIA_*` | the MG42's and .30 cal's world models while deployed / reloading | third-person look |
| `anim_prefix`, `reload_anim_prefix` | the body's animation set | animation |

No field gives range, falloff, deploy time, spread by stance or the
recoil pattern.

## The guns (script values)

| id | Type | Damage | Bullets | Accuracy | Alt accuracy | Move pen. | Recoil | FireDelay | Clip | Spare clips |
|---|---|---|---|---|---|---|---|---|---|---|
| garand | Rifle | 80 | 1 | 0.014 | 0.0 | 0.1 | 4.0 | 0.37 | 8 | 11 |
| k98 | Rifle | 110 | 1 | 0.014 | 0.0 | 0.15 | 8.0 | 1.6 | 5 | 13 |
| spring | Sniper | 120 | 1 | 0.06 | 0.0 | 0.1 | 5.6 | 1.85 | 5 | 11 |
| k98_scoped | Sniper | 120 | 1 | 0.06 | 0.0 | 0.1 | 6.0 | 1.6 | 5 | 13 |
| thompson | SubMG | 40 | 1 | 0.055 | — | 0.1 | 2.15 | 0.085 | 30 | 7 |
| mp40 | SubMG | 40 | 1 | 0.055 | — | 0.1 | 2.2 | 0.09 | 30 | 7 |
| bar | SubMG | 50 | 1 | 0.025 | 0.02 | 0.1 | 5.0 | 0.12 (alt 0.3) | 20 | 13 |
| mp44 | SubMG | 50 | 1 | 0.025 | 0.02 | 0.1 | 5.0 | 0.12 (alt 0.3) | 30 | 7 |
| 30cal | MG | 85 | 1 | 0.2 | 0.01 | 0.1 | 20 | 0.1 | 150 | 3 |
| mg42 | MG | 85 | **2** | 0.2 | 0.025 | 0.1 | 20 | 0.05 | 250 | 2 |
| colt | Pistol | 40 | 1 | 0.055 | — | 0.1 | 1.4 | 0.1 | 7 | 3 |
| p38 | Pistol | 40 | 1 | 0.055 | — | 0.1 | 1.4 | 0.1 | 8 | 3 |
| c96 | Pistol | 40 | 1 | 0.065 | — | 0.1 | 3.0 | 0.065 | 20 | 3 |
| m1carbine | Sidearm | 40 | 1 | 0.019 | — | 0.1 | 1.4 | 0.1 | 15 | 3 |
| bazooka | Bazooka | 100 | — | — | — | — | 10.0 | 1.0 | 1 | 5 |
| pschreck | Bazooka | 100 | — | — | — | — | 10.0 | 1.0 | 1 | 5 |
| riflegren_us / _ger | RifleGrenade | 100 | — | — | — | — | 10.0 | 1.0 | 1 | 2 |
| frag_us / frag_ger | Grenade | 180 | — | — | — | — | 0 | 0 | −1 (1) | — |
| smoke_us / smoke_ger | Grenade | 0 | — | — | — | — | 0 | 0 | −1 (1) | — |
| amerknife / spade | Melee | 60 | — | — | — | — | 0 | 0.1 | — | — |

Also in the manifest: the `_live` copies of the frags and rifle
grenades (180 damage, held after being picked up or primed; open
question 6), `basebomb` (the bomb objective) and `ifm_steadycam` (a
film-making camera; it fails to load on the server: ignore).

Damage scaling by hit zone and the backstab are in damage.md; the
measured damage dealt equals `Damage` before scaling for every gun hit
(Garand, K98, Springfield, Thompson, MP40, BAR, StG44, MG42, Colt,
P38, C96, M1 Carbine, knife, spade).

The MG42's `BulletsPerShot` 2 does not fire two bullets: deployed, at a
standing victim 150 units away (spread 0.025, so a second bullet could
not miss), each shot logged one bullets message and one hit (85 to the
leg), the same as the .30 cal; the magazine loses one round a shot.

## Fire modes (measured)

- **Automatic** (SMGs, BAR, StG44, MGs): holding fire repeats at the
  tick-rounded interval: Thompson every 6 ticks, .30 cal every 7 ticks
  (deployed and not).
- **Semi-automatic** (Garand; pistols, carbine assumed): holding fire
  fires once; each shot needs a new press.
- **Bolt action** (K98, Springfield, K98 scoped): the next shot is
  blocked for `FireDelay` (Springfield: 1.85 s after a shot, measured).
- **Garand reload only when empty**: with 7 rounds left, holding reload
  for 4.5 s did nothing (one case, the bot prone; quirk to keep).
- **MG42**: 0.05 s rounds up to 4 ticks (0.06 s), a shot every 4 ticks
  deployed (measured).

## Spread (measured)

Each shot's spread is a single number the server sends with the shot
(the cone's half-angle as a tangent, as CS:S's): measured from it on
every gun, standing, ducked, prone, moving and in each alt mode.

    spread = base + (horizontal speed > 45 u/s ? AccuracyMovePenalty : 0)
    base   = SecondaryAccuracy while the alt mode is on (sights, scope,
             fire select's semi-auto, an MG deployed), else Accuracy

- **The move threshold is 45 u/s** of horizontal speed after the tick's
  movement (measured at steady speeds: 44.0 and 44.9 no penalty, 45.1
  and 46 the full penalty). The penalty is all or nothing: 46, 60, 100,
  150 and 220 u/s all add the whole `AccuracyMovePenalty`. Ducked
  moving (73.3 u/s) adds it too.
- **No stance bonus**: ducked and prone fire with the standing spread
  (Colt, P38, C96, M1 Carbine, BAR, StG44, the MGs undeployed and every
  rifle measured: identical numbers).
- **No growth while firing**: the Thompson's ten shots of a burst and
  the deployed MG42's eight all read the same spread.
- **Sights and scopes**: 0 (`SecondaryAccuracy`) standing, ducked and
  moving; a sighted player can't pass 45 u/s (movement.md), so the move
  penalty never applies while sighted.
- **Fire select**: the BAR and StG44 in semi-auto read 0.02 still, 0.12
  moving.
- **MGs undeployed** read 0.2 (0.3 moving), standing, ducked or prone;
  deployed 0.01 (.30 cal) / 0.025 (MG42).

| Gun | still | moving > 45 | alt mode |
|---|---|---|---|
| garand, k98 | 0.014 | 0.114 / 0.164 | 0 (sights) |
| spring, k98_scoped | 0.06 | 0.16 | 0 (scope) |
| thompson, mp40 | 0.055 | 0.155 | — |
| bar, mp44 | 0.025 | 0.125 | 0.02 (semi), 0.12 moving |
| 30cal, mg42 | 0.2 | 0.3 | 0.01 / 0.025 (deployed) |
| colt, p38 | 0.055 | 0.155 | — |
| c96 | 0.065 | 0.165 | — |
| m1carbine | 0.019 | 0.119 | — |

The shot carries a random seed (0-255: its command's seed & 255) with
the spread; how the seed spreads the bullet inside the cone (the
pattern) is in spread.md (measured from each bullet's damage position
and force on a victim). At a steady 45.0 u/s the penalty applies
(spread.md): the threshold is 45 itself.

## When a gun can't fire (measured)

- **Sprinting**: no gun fires and no grenade is thrown while the sprint
  flag is set (Thompson held fire through 30 sprinting ticks: no shot;
  the first shot came on the tick the sprint ended; a frag pressed while
  sprinting did nothing, pressed again running it threw).
- **In the air**: no shot (a press in the air is lost); fire held
  through a jump fires on the landing tick.
- **Crawling prone**: fire held while crawling did nothing; the gun fired
  once the player had stopped. Prone and still it fires as standing.

## Alt fire (measured where stated)

- **Iron sights and scopes** (Garand, K98: sights; Springfield, K98
  scoped: scope): the alt fire toggles them; the gun's zoom flag sets on
  the next tick and the next primary attack is blocked ≈0.49 s (Garand,
  measured). Holding alt fire toggles again when that time is up (the
  flag cleared after 35 ticks of holding). While sighted the spread is
  `SecondaryAccuracy` (0.0 for the rifles: a perfect shot; unmeasured).
  **Field of view** (measured, the player's FOV field; the default is
  90): Garand and K98 sights **55**, Springfield and K98 scoped scopes
  **20**. Zooming in: the FOV changes 21 ticks (0.315 s) after the press,
  over 0.1 s (the FOV's change rate field 0.1); zooming out: back to 90
  on the press tick, over 0.1 s.
- **Fire select** (BAR, StG44): the alt fire is **not** a sight: it
  toggles **semi-automatic** fire (the weapon's semi-auto flag; FOV
  unchanged, measured). In semi-auto one shot per press, the next 0.3 s
  later (`SecondaryFireDelay`), with `SecondaryAccuracy` (0.02) as the
  spread (measured, above); the alt fire again returns to full auto.
- **Deploying an MG** (.30 cal, MG42): alt fire while prone deploys
  (measured: the deployed flag sets at once; firing blocked 1.2163 s
  for the .30 cal, 1.1667 s for the MG42). The deployed height reads 10
  (the prone eye). While deployed the spread is `SecondaryAccuracy` and
  the yaw is limited: the player's deployed yaw limit fields read **45**
  to the left and **45** to the right of the deploy direction in the
  open.
- **The yaw limit is the client's** (measured 2026-11-24): the server
  neither clamps the view nor undeploys when the view turns past it
  (turned 1° a tick to ±70°, and in one step to +60°: still deployed,
  the server's view angles following the input). The limits are
  replicated for the client to hold its own view inside (the earlier
  "undeployed at about +58°" is not reproduced). Our client should
  clamp the view to the limits.
- **Pitch undeploys** (server, measured prone on flat ground, pitch
  changed 1° per 5 ticks, two runs): looking down past about **47°**
  (46 never undeployed; 47 did in one run, 48 in the other) or up past
  about **63°** (66 undeployed, 3° steps) undeploys the gun, the player
  staying prone.
- **Obstacles beside the gun narrow the limits**: a 64-unit crate
  sunk to stand 24 above the ground beside a prone deployed player
  changed the side it was on (other side 45):

| Crate centre (° left of the aim, units away) | Limit on that side |
|---|---|
| 90°, 50 (left or right) | 30 |
| 60°, 60 | 35 |
| 30°, 70 | 0 |
| 90° at 42, 70, 90 or 110; 45°, 60; 120°, 60; 50 ahead and 60 left; 60 behind and 60 left | 45 (unchanged) |

  The limits move in 5° steps (0, 5, 30, 35 seen).
- **What narrows them** (measured 2026-11-24, `measure.py mgsweep`, the
  same crate moved in 2-unit steps; MG42 deployed prone on flat ground):
  - The two placements above that left the limit at 45 (90° at 42; 45°
    at 60) put the crate **into the player's own box** (the prone box is
    ±16 around the origin): every placement whose crate overlapped that
    box left both limits at 45, whatever the overlap. A crate overlapping
    the box is not an obstacle to the check.
  - Crate beside (its centre 90° left), its near face s units to the
    side of the player's origin:

| s | 16 (touching the box) | 18 | 20 | 22 | 24 to 40 |
|---|---|---|---|---|---|
| limit on that side | 25 | 30 | 35 | 40 | 45 |

    The same with the crate's centre 20 or 40 units ahead: s = 20 gave
    35, s = 24 and more 45; with the centre 20 behind, every s gave 45.
  - Crate ahead (its near face 20 ahead of the origin), its edge e units
    to the left of the aim line: e = 1 **refused the deploy** (the alt
    fire did nothing); e = 2, 3 gave 0; e = 4, 5, 6 gave 5; e = 8 gave
    10; e = 10 gave 15 (the other side 45 throughout).
  - A rule that fits every valid placement (ours, a fit, not the
    game's): from 0° outwards in 5° steps to 45°, a check along that
    yaw from a point about 3-10 units behind the player's origin, about
    1.5 units wide on each side, reaching about 30-32 units from that
    point, at the deployed height; the first step whose check meets a
    solid ends the search and the limit is the step before it; a solid
    met at 0° refuses the deploy. The placements bound the numbers only
    to those ranges.
- **Deploying on a ledge** (measured with the crate 2 units ahead of
  the player's box, the MG42): alt fire standing or crouched deploys on
  a surface ahead and the deployed height reads **the surface's top +
  16**; the eye moves to that height (eased over about 20 ticks), at
  most the standing 58; the ducked flag clears (a crouched deploy stands
  the player at the deployed height, even above the crouched eye: a top
  38 put the eye at 54).

| Stance | Tops that deployed (above the feet) | Did not |
|---|---|---|
| standing | 10, 14, 20, 30, 38, 44, 50, 52, 54 | 56, 62 |
| crouched | 14, 20, 26, 32, 38 | 8, 44, 50, 56 |

  The surface must be close: a top of 40 deployed 12 units ahead of the
  player's box, not 22. Deployed on a ledge the gun fires with the
  deployed spread. `dod_bodyheightoffset` (4, "deploy height offset")
  is not visibly in these numbers (+16 above the surface).
- **Undeploying**: the alt fire again undeploys at once (the player stays
  prone); firing is then blocked 1.3635 s (.30 cal) / 1.1667 s (MG42).
  Undeployed, prone, the MG fires (undeployed spread and stamina cost).
- **MG reloads only deployed**: the reload key does nothing undeployed
  (standing or prone, measured for both); deployed, an empty .30 cal
  reloads in 4.6 s (307 ticks), an MG42 in 4.71 s (314 ticks).
- **Firing an MG undeployed drains stamina**: .30 cal held fire,
  standing: stamina 100 → 85 on the first shot, then −8.7 net per shot
  (7 ticks apart) to 0; deployed: no stamina change (measured). The
  first shot's 15 and a regeneration of ≈0.9 per tick between shots
  would explain both numbers (open question 4).
- **Shouldering a rocket** (bazooka; Panzerschreck assumed): alt fire
  shoulders it (flag sets next tick; firing blocked ≈0.79 s); holding
  alt fire past that unshoulders it (blocked ≈0.99 s). Unshouldered the
  launcher does not fire. Shouldered, the player moves at most
  ≈50 u/s; unshouldered at 220 (measured with forward held). The next
  rocket can't follow for 1.5 s. After a shot the launcher accepts
  nothing (not even the reload key) for **0.84 s** (bazooka) or
  **2.33 s** (Panzerschreck) (measured: the weapon's next-attack time;
  a Panzerschreck reload pressed 1.8 s after the shot was ignored, not
  queued). The launcher stays shouldered through the shot and the
  reload.
- **SMG stock strike** (Thompson, MP40; the assault classes carry no
  knife), measured: the alt fire is a melee strike, the same as the
  knife's (damage.md, "Melee"): 60 before hit zones, the hit landing 14
  ticks (0.21 s) after the press, the same reach, and every attack of the
  gun blocked 0.4 s from the press (the next primary shot came 0.4 s
  later); the magazine is untouched. Unlike the knife it has **no
  backstab**: from behind it still did 60. DoD:S has **no bayonets**: the K98's and Garand's alt
  fire is the iron sight (measured for the Garand; a K98 alt-fire
  swing at 30-70 units hit nothing).
- **Rifle grenades**: a separate weapon (the rifleman's secondary):
  switching to it fits the launcher; 2 grenades. The next shot 1.0 s
  after one. After its last grenade the German one switches back to the
  K98 (the US one stayed on the launcher in the one run). Flight below.

## Draw and reload times (measured)

Draw: switching to the gun blocks every attack (the player's next-attack
time) for this long; the first shot with fire held came that many ticks
later (Thompson: 63 ticks, 0.945 s). Reload: an empty magazine, the
reload key pressed; the gun can't fire for this long and the magazine
fills at its end (the whole magazine at once, also for the bolt rifles).

| Gun | Draw (s) | Empty reload (s) |
|---|---|---|
| garand | 0.9375 | 1.6286 |
| k98 | 1.1000 | 2.8125 |
| spring | 1.3334 | 4.5000 |
| k98_scoped | 1.1000 | 4.2122 |
| thompson | 0.9376 | 3.0000 |
| mp40 | 1.4001 | 3.3636 |
| bar | 1.3637 | 2.9767 |
| mp44 | 1.1291 | 3.2354 |
| 30cal | 1.3334 | 4.6 (deployed only) |
| mg42 | 1.5151 | 4.71 (deployed only) |
| colt | 1.2500 | 2.0589 |
| p38 | 1.0286 | 2.4688 |
| c96 | 1.2501 | 2.6471 |
| m1carbine | 0.7813 | 2.8126 |
| bazooka, pschreck | 1.0000 | 3.0 (200 ticks; shouldered or not) |
| riflegren_us | — | 2.91 (194 ticks) |
| riflegren_ger | — | 2.505 (167 ticks) |
| amerknife | 1.0000 | — |
| spade | 0.9091 | — |

The launchers and rifle grenades do not reload by themselves: the clip
stays empty until the reload key (measured; the rifle grenades' times
from the press to the grenade being in, ±1 tick).

**Partial reloads** (measured, the reload key with rounds left): the
same time as from empty for every gun except two pistols, and only the
missing rounds leave the reserve (Thompson 10 → 30 took 20; no
magazine is thrown away):

| Gun (rounds left) | Partial (s) | Empty (s) |
|---|---|---|
| thompson (10), mp40 (10), bar (10), mp44 (10) | 3.0, 3.375, 2.985, 3.24 | as the table above |
| p38 (3), m1carbine (7) | 2.475, 2.82 | as above |
| k98 (2), spring (2), k98_scoped (2) | 2.82, 4.5, 4.215 | as above (the whole clip at once) |
| **colt** (3) | **1.56** (104 ticks) | 2.0589 |
| **c96** (10) | **2.22** (148 ticks) | 2.6471 |

The Garand refuses a partial reload (above).

## Recoil (unmeasured)

Firing does not move the server's view punch (the player's punch angle
stayed 0 through a Thompson shot): DoD:S's kick is applied by the
client to its own view, and the server only sees the angles the client
sends. Measuring it needs the retail client (app 300) on a listen
server with the plugin logging the player's eye angles; the dedicated
server can't (open question 8).

## Grenades (measured)

Frag (US) and stick grenade (German) behave the same:

- **The pin is pulled on the press**: holding the attack keeps the
  grenade in the hand, releasing throws it. The fuse runs from the
  press, thrown or not: the blast comes 335-347 ticks (5.0-5.2 s) after
  the press (held 1, 60, 150 ticks: 347, 341, 340 ticks; held past the
  fuse it goes off in the hand at 342).
- **Thrown from the eye** (the grenade starts at the eye point), with a
  speed and angle from the aim's pitch (measured from the first steps):

| Aim (+ down) | −89 | −80 | −70 | −60 | −50 | −45 | −40 | −35 | −30 | −25 | −20 | −15 | 0 | 15 | 30 |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| speed (u/s) | 750 | 715-716 | 677 | 639-641 | 600 | 592 | 566 | 546 | 527-532 | 509 | 501 | 499-500 | 499-500 | 497 | 497 |
| elevation (° up) | 89.5 | 81.3-81.6 | 72.8 | 63.5-63.8 | 55.3 | 50.5 | 45.5 | 41.1 | 36.4-36.7 | 32.2 | 27.5 | 23.0-23.7 | 10.0-10.9 | −6.0 | −22.8 |

  The elevation is the aim raised by 10° and squeezed so straight up
  and straight down stay so: aiming A° up throws at 10 + A × 80/90; aiming
  D° down at 10 − D × 100/90. The speed is 500 (`dod_grenademinspeed`)
  up to aiming about 22.5° up, then grows linearly with the aim:

      speed = 500 + 250 × (A − 22.5) / 67.5    for an aim A° up above 22.5

  (a fit to eighteen throws, two runs from 25° to 89° up: residuals
  within ±4.5 u/s, the first-step method's own noise from the tumble;
  about 750 straight up; one throw at 20° up read 490-501).
  `dod_grenademaxspeed` 1400 is never reached by aiming (what it caps is
  open question 6). One stick grenade's own initial-velocity field read
  (348.18, −348.18, 86.82): 500 u/s at 10.0° exactly.
- **The thrower's velocity** (measured twice each, running at 220 and
  throwing level): running forward the grenade's horizontal speed along
  the aim was 600-608 (500 × cos 10° = 492 standing: +108 to +116),
  running backward 365-367 (−125 to −127), running sideways 490-491
  along the aim and 0-7 across it; the vertical part stayed 84-86. So
  about half the thrower's velocity along the aim is added and none
  across it (the exact law: open question 6).
- **Timing** (measured): the press pulls the pin and blocks the weapon
  for 0.667 s (44.4 ticks); the grenade leaves the hand on the release
  (the same tick, even released at once). The next grenade is drawn
  when both the 0.667 s are over and the grenade has left (the tick
  after the release when held longer) and is ready 0.667 s after that
  (released at once: the next press works 1.33 s after the first;
  held 60 or 100 ticks: 0.68 s after the release). After the last
  grenade the player switches to another weapon on the next tick.
- **No throwing while sprinting** (above, "When a gun can't fire").
- **Flight**: a tumbling physics object under about 420 u/s² of gravity
  (`dod_grenadegravity` −420; fits over the first 0.2-0.4 s gave 410-460,
  the origin wobbling a few units as it spins), bouncing and rolling on
  the ground until it goes off.
- **Bounce** (measured on dod_avalanche's flat ground, from the
  grenade's positions a tick apart, two runs): a physics-engine bounce
  that depends on how the grenade is turned when it lands, so the same
  throw bounced differently:

| Throw | Impact (down, horizontal u/s) | Rebound apex (two runs) | Horizontal after (two runs) |
|---|---|---|---|
| aimed 89° down | 531, 0 | 11.7 / 2.5 units | ≈100 / ≈113 u/s (the tumble sends it sideways) |
| aimed 45° down | 375, 382 | 15.5 / 0.4 units | 222 / 92 u/s |
| aimed level (lands 1.0 s later) | 200, 482 | > 11 (first run) | 339 (first run) |

  So the vertical speed comes back at up to about 0.3 of the impact's
  (often much less) and the horizontal at 0.25-0.7; it then rolls and
  slows (the 89° throw of the second run still moved 22 u/s 0.5 s after
  landing). For the party: a low bounce (restitution about 0.2) with
  strong rolling friction and some sideways scatter matches these; no
  fixed law was found.
- **Blast**: 150 damage, radius 375, linear falloff, a separate stun
  event, the thrower hurt too (damage.md, "Explosions").
- Smoke grenades and the live-grenade pick-up: unmeasured.

## Rifle grenades (measured)

- Fired from the eye along the aim (no extra lift: aiming 10° up flew at
  10.0°) at **2000 u/s** (29.98 units the first tick); a tumbling physics
  object, gravity and drag together fitting 440-456 u/s² over the first
  15-30 ticks with the horizontal speed falling (1943 → 1916 average).
- **Timed fuse**: it goes off 242 ticks (3.63 s) after firing, whatever
  it hits first (it bounced off a player, doing 4.9 crush damage, and
  went off later on the ground). Both nations' the same.
- **Blast**: 150 damage, radius 375 (the grenade entity's fields; the
  script's `Damage` 100 is not it), as the frags (damage.md).

## Rockets (measured)

- Bazooka and Panzerschreck: fired from the eye along the aim at
  **1300 u/s**, a flying object with **0.1 gravity** (80 u/s², the
  vertical speed dropping 1.2 a tick), not a physics object.
- **Hitting a player** it explodes (a direct hit did 129.5 to the
  victim; blast 150, radius 200: `mp_rocketdamage` / `mp_rocketradius`,
  damage.md).
- **Hitting the world it explodes** (measured 2026-11-24, both
  launchers): fired at the ground 45° down it was removed on the tick it
  reached the ground 55 units away and its blast hurt the thrower
  (96.1-98.7 blast and a 35.9-37.7 stun event, bazooka; 92.5-98.5 and
  40.7-41.1 Panzerschreck, two runs);
  fired level or 1-10° down it was removed where it met the ground
  (290-1100 units out), fired into a wall 20 units away it was removed
  at the wall. The first probe's "slid along the ground and stayed" is
  not reproduced.

## Grenade and explosion cvars

| Cvar | Value | Meaning (its description or name) |
|---|---|---|
| `dod_grenadegravity` | −420 | gravity on grenades (u/s²; matches the measured flight) |
| `dod_grenademinspeed` / `maxspeed` | 500 / 1400 | throw speed range (u/s; 500 measured for level throws) |
| `dod_grenadespeed` | 4 | (unclear; open question 7) |
| `mp_rocketdamage` / `mp_rocketradius` | 150 / 200 | rocket blast damage and radius |
| `dod_explosionforcescale`, `dod_bulletforcescale` | 1 | ragdoll force scales |
| `dod_meleeattackforcescale` | 8 | melee ragdoll force |

A thrown grenade lying on the ground can be picked up and thrown back
(the `_live` weapons); unmeasured.

## Test cases

| Setup | Input | Expected |
|---|---|---|
| Thompson, full clip | hold fire 100 ticks | a shot every 6 ticks (17 shots), clip 30 → 13 |
| .30 cal deployed | hold fire | a shot every 7 ticks, stamina unchanged |
| .30 cal undeployed, stamina 100 | first shot | stamina 85 |
| Garand, 8 rounds | hold fire | one shot |
| Garand, 7 rounds | hold reload | no reload |
| Springfield | fire | next shot ≥1.85 s later |
| prone with the .30 cal | alt fire | deployed; first shot ≈1.2 s later at the earliest |
| bazooka unshouldered | fire | no rocket |
| bazooka | alt fire | shouldered; can fire ≈0.79 s later; move ≤50 u/s |
| any gun, chest hit at 1,600 u | fire | `Damage` dealt |
| Thompson in hand, knife drawn | switch to the Thompson, hold fire | first shot 63 ticks later |
| Thompson, empty | reload | fires again 3.0 s later, 30 rounds |
| Garand | alt fire | FOV 90 → 55, 21 ticks after the press, over 0.1 s |
| Springfield | alt fire | FOV 20 |
| BAR | alt fire, then hold fire | one shot, next ≥0.3 s later; FOV unchanged |
| .30 cal deployed, empty | reload | 150 rounds after 4.6 s |
| .30 cal undeployed, empty | reload | nothing |
| .30 cal deployed | alt fire | undeployed at once, fires again 1.36 s later |
| frag | press and hold 150 ticks, release | thrown on release, goes off about 5.1 s after the press |
| frag | press, release at once, aim level | 500 u/s at 10° up from the eye |
| frag | aim 30° down | 500 u/s at 23.3° down |
| rifle grenade | fire, aim level | 2000 u/s along the aim; goes off 3.63 s later |
| bazooka shouldered | fire, aim level | 1300 u/s, falling 80 u/s²; next rocket 1.5 s later |
| Thompson, standing still | fire | spread 0.055 |
| Thompson, moving at 44.9 / 45.1 u/s | fire | spread 0.055 / 0.155 |
| Thompson, ducked moving (73.3) | fire | 0.155 |
| Colt, prone still | fire | 0.055 (no stance bonus) |
| Garand sighted | fire | spread 0 |
| BAR semi-auto, moving at 220 | fire | 0.12 |
| MG42 undeployed, prone | fire | 0.2 |
| .30 cal deployed | fire | 0.01 |
| Thompson, sprinting | hold fire | no shot until the sprint ends |
| Thompson, jumping | hold fire from the jump | first shot on the landing tick |
| Thompson, crawling prone | hold fire | no shot until stopped |
| Thompson, victim 45 u ahead | alt fire | 60 to the victim 0.21 s later; next attack 0.4 s |
| Thompson, victim facing away | alt fire | 60 (no backstab) |
| MG42 deployed, victim 150 u ahead | one shot | one hit (85 before zones) |
| Colt, 3 rounds | reload | 7 rounds 1.56 s later, reserve −4 |
| Thompson, 10 rounds | reload | 30 rounds 3.0 s later, reserve −20 |
| bazooka, empty, 200 ticks after the shot | reload | loaded 3.0 s later |
| Panzerschreck | reload 1.8 s after the shot | ignored |
| US rifle grenade, empty | reload | loaded 2.91 s later |
| frag | aim 60° up, throw | ≈639 u/s at 63.3° up |
| frag | aim 89° up | ≈750 u/s |
| frag, running forward at 220 | throw level | horizontal speed ≈608 along the aim |
| frag, running sideways at 220 | throw level | ≈492 along the aim, none sideways |
| frag, two in hand | press and release at once | thrown at once; the next press works 1.33 s after the first |
| frag, two in hand | hold 100 ticks, release | thrown on the release; next ready 0.667 s later |
| frag, the last one | throw | another weapon drawn the next tick |
| frag, sprinting | press | nothing |
| bazooka | fire 45° down at open ground | explodes on the ground 55 u ahead; the thrower takes ≈99 blast |
| MG42 deployed prone | turn the view 60° | still deployed (the client clamps) |
| MG42 deployed prone | look 46° / 48° down | deployed / undeployed, still prone |
| MG42 standing, a 30-high ledge 2 u ahead | alt fire | deployed, height 46, eye 46 |
| MG42 crouched, a 38-high ledge | alt fire | deployed, height 54, eye 54, not ducked |
| MG42 prone, a crate's near face 18 / 20 / 22 / 24 to the left (centre beside) | deploy | left limit 30 / 35 / 40 / 45, right 45 |
| MG42 prone, a crate's near face 20 ahead, its edge 1 / 3 / 5 / 10 left of the aim line | deploy | refused / left limit 0 / 5 / 15 |
| MG42 prone, a crate overlapping the player's box | deploy | both limits 45 |
| MG42 standing, a 56-high ledge | alt fire | nothing |

## Open questions

1. Spread: answered (above); the pattern inside the cone in spread.md.
2. Answered: one bullet a shot (one hit, one round).
3. Answered: the BAR's and StG44's alt fire is fire select; semi-auto
   fires at most every 0.3 s.
4. Stamina cost of undeployed MG fire: 15 on the first shot; the
   regeneration is movement.md's (0.9 a tick standing still); the
   per-shot cost after the first unmeasured (8.7 net per shot fits a
   cost of 15 a shot against 7 ticks × 0.9).
5. `Tracer` 2: every other bullet?
6. Grenades: answered (priming, fuses, radius, falloff, the speed above
   22.5° up, timings) except the exact law of the thrower's velocity
   (about half along the aim, none across: three throws, ±8 u/s), what
   `dod_grenademaxspeed` caps, and the live-grenade pick-up.
7. `dod_grenadespeed` 4.
8. Recoil: the view kick's shape and recovery; `Recoil` 20 on the MGs
   undeployed. Client-side: needs the retail client (above).
9. Reload times: answered (tables), including partial reloads and the
   launchers'.
10. Draw times: answered (table).
11. What narrows a deployed MG's yaw limit below 45: mostly answered
    (2026-11-24: solids within about 30 units of a point just behind the
    player, checked in 5° steps outwards; a crate inside the player's
    box doesn't count; tables above). Open: the check's exact start
    point, width and reach (bounded, not pinned), whether it is the
    same on a ledge deploy, the pitch limits on a ledge, and whether
    moving off the ledge undeploys.
12. Answered: rockets explode on the world.
13. The sighted speed (movement.md): from rest the sighted player
    accelerates very slowly; the numbers were not steady between runs.
