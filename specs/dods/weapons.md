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
riflegren`). The
scripts' values are data the game exposes; they are given to check
against, and the game supplies them at run time. What the scripts do
not hold (spread by stance, the recoil curve) is marked unmeasured
where no measurement exists.
Status: draft (scripts read, measured 2026-11-23; recoil and spread
unmeasured)

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

The MG42's `BulletsPerShot` 2: one trace-attack per hit was logged in
the one MG42 hit measured; whether both bullets can hit is open
question 2.

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
  spread (the spread unmeasured); the alt fire again returns to full
  auto.
- **Deploying an MG** (.30 cal, MG42): alt fire while prone deploys
  (measured: the deployed flag sets at once; firing blocked 1.2163 s
  for the .30 cal, 1.1667 s for the MG42). The deployed height reads 10
  (the prone eye). While deployed the spread is `SecondaryAccuracy` and
  the yaw is limited: the player's deployed yaw limit fields read **45**
  to the left and **45** to the right of the deploy direction (one deploy
  read 5 on the left: something nearby narrows it; open question 11);
  turning past the limit undeploys the gun (seen once, at about +58°).
  Deploying on a ledge while standing or crouched (the
  `dod_bodyheightoffset` 4 cvar, "deploy height offset") is unmeasured
  (no ledge measured yet).
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
  rocket can't follow for 1.5 s.
- **SMG melee** (Thompson, MP40; the assault classes carry no knife):
  public descriptions say the alt fire strikes with the stock;
  unmeasured. DoD:S has **no bayonets**: the K98's and Garand's alt
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
| bazooka, pschreck | 1.0000 | unmeasured (open question 9) |
| amerknife | 1.0000 | — |
| spade | 0.9091 | — |

A partial reload was not measured (the Garand refuses one).

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

| Aim (+ down) | −80 | −60 | −45 | −30 | −15 | 0 | 15 | 30 |
|---|---|---|---|---|---|---|---|---|
| speed (u/s) | 716 | 641 | 592 | 527-532 | 499 | 499-500 | 497 | 497 |
| elevation (° up) | 81.3 | 63.5 | 50.5 | 36.7 | 23.7 | 10.0-10.9 | −6.0 | −22.8 |

  The elevation is the aim raised by 10° and squeezed so straight up
  and straight down stay so: aiming A° up throws at 10 + A × 80/90; aiming
  D° down at 10 − D × 100/90. The speed is 500 (`dod_grenademinspeed`)
  from aiming 30° down to about 15° up, then grows with the elevation
  (the law, and where `dod_grenademaxspeed` 1400 applies, are open
  question 6). The thrower's own velocity: unmeasured (thrown standing).
  One stick grenade's own initial-velocity field read (348.18, −348.18,
  86.82): 500 u/s at 10.0° exactly.
- **Flight**: a tumbling physics object under about 420 u/s² of gravity
  (`dod_grenadegravity` −420; fits over the first 0.2-0.4 s gave 410-460,
  the origin wobbling a few units as it spins), bouncing and rolling on
  the ground until it goes off.
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
- **Hitting the world** it did not explode on the dedicated server: it
  slid along the ground or stopped against a wall and stayed (not
  removed in 60-150 ticks). Whether that is the game's rule (an arming
  time?) or this server's (open question 12): the party should explode
  rockets on world hits until measured otherwise, as everyone expects.

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

## Open questions

1. Spread: how `Accuracy`, the move penalty, crouch and prone combine,
   and the spread pattern (CS:S's seeded spread assumed).
2. The MG42's two bullets per shot: both traced, both damaging?
3. Answered: the BAR's and StG44's alt fire is fire select; semi-auto
   fires at most every 0.3 s.
4. Stamina cost of undeployed MG fire: 15 on the first shot; the
   regeneration is movement.md's (0.9 a tick standing still); the
   per-shot cost after the first unmeasured (8.7 net per shot fits a
   cost of 15 a shot against 7 ticks × 0.9).
5. `Tracer` 2: every other bullet?
6. Grenades: answered (priming, fuses, radius, falloff) except the
   throw speed's law above 15° up, the thrower's velocity, and the
   live-grenade pick-up.
7. `dod_grenadespeed` 4.
8. Recoil: the view kick's shape and recovery; `Recoil` 20 on the MGs
   undeployed. Client-side: needs the retail client (above).
9. Reload times: answered (table) except the rockets' and rifle
   grenades', and partial reloads.
10. Draw times: answered (table).
11. What narrows a deployed MG's yaw limit below 45, and the limits'
    behaviour at the edge (clamp or undeploy).
12. Rockets hitting the world: explode or not (above).
