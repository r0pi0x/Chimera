# Day of Defeat: Source: weapons (deltas from CS:S)

Source basis: the dedicated server's per-weapon scripts
(`scripts/weapon_*.txt`, all 29 in the manifest read in place, their
fields compared), the server's cvar list, the view-model list, and
measurements on the dedicated server (2026-11-23) with puppet bots
driven by a SourceMod plugin (fire intervals, magazines, deploy and
shoulder timings, stamina while firing; damage in damage.md). The
scripts' values are data the game exposes; they are given to check
against, and the game supplies them at run time. What the scripts do
not hold (spread by stance, the recoil curve, rifle-grenade flight) is
marked unmeasured where no measurement exists.
Status: draft (scripts read, partly measured 2026-11-23)

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

- **Iron sights** (Garand, K98, BAR, StG44, sniper rifles' scope): the
  alt fire toggles the sights; the gun's zoom flag sets on the next
  tick and the next primary attack is blocked ≈0.49 s (Garand,
  measured). Holding alt fire toggles again when that time is up (the
  flag cleared after 35 ticks of holding). While sighted the spread is
  `SecondaryAccuracy` (0.0 for the rifles: a perfect shot; unmeasured).
- **Deploying an MG** (.30 cal, MG42): alt fire while prone deploys
  (measured: the deployed flag sets at once, firing blocked ≈1.2 s
  after). Deploying on a ledge while standing or crouched (the
  `dod_bodyheightoffset` 4 cvar, "deploy height offset") is
  unmeasured. While deployed the yaw is limited (the player's left and
  right deployed yaw limits; values unmeasured) and the spread is
  `SecondaryAccuracy`.
- **Firing an MG undeployed drains stamina**: .30 cal held fire,
  standing: stamina 100 → 85 on the first shot, then −8.7 net per shot
  (7 ticks apart) to 0; deployed: no stamina change (measured). The
  first shot's 15 and a regeneration of ≈0.9 per tick between shots
  would explain both numbers (open question 4).
- **Shouldering a rocket** (bazooka; Panzerschreck assumed): alt fire
  shoulders it (flag sets next tick; firing blocked ≈0.79 s); holding
  alt fire past that unshoulders it (blocked ≈0.99 s). Unshouldered the
  launcher does not fire. Shouldered, the player moves at most
  ≈50 u/s; unshouldered at 220 (measured with forward held).
- **SMG melee** (Thompson, MP40; the assault classes carry no knife):
  public descriptions say the alt fire strikes with the stock;
  unmeasured. DoD:S has **no bayonets**: the K98's and Garand's alt
  fire is the iron sight (measured for the Garand; a K98 alt-fire
  swing at 30-70 units hit nothing).
- **Rifle grenades**: a separate weapon (the rifleman's secondary):
  switching to it fits the launcher; 2 grenades; 100 damage (script);
  flight, fuse, radius and fitting time unmeasured.

## Grenades and explosions (unmeasured beyond cvars)

| Cvar | Value | Meaning (its description or name) |
|---|---|---|
| `dod_grenadegravity` | −420 | gravity on grenades (u/s²) |
| `dod_grenademinspeed` / `maxspeed` | 500 / 1400 | throw speed range (u/s) |
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

## Open questions

1. Spread: how `Accuracy`, the move penalty, crouch and prone combine,
   and the spread pattern (CS:S's seeded spread assumed).
2. The MG42's two bullets per shot: both traced, both damaging?
3. The BAR's and StG44's `SecondaryFireDelay` 0.3: semi-automatic
   while sighted?
4. Stamina cost of undeployed MG fire (15 per shot?) and its
   regeneration.
5. `Tracer` 2: every other bullet?
6. Grenade priming, fuse (frag and rifle grenade), radius and falloff;
   the live-grenade pick-up.
7. `dod_grenadespeed` 4.
8. Recoil: the view kick's shape and recovery; `Recoil` 20 on the MGs
   undeployed.
9. Reload times (the view models' sequence lengths are the likely
   source, as CS:S's).
10. Deploy (draw) times when switching weapons.
