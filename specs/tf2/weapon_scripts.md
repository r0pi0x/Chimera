# Team Fortress 2: the per-weapon scripts' fields (T8)

Source basis: the dedicated server install's encrypted per-weapon scripts
(`scripts/tf_weapon_*.ctx`, all 78 surveyed for their field names, the
twelve stock guns' scripts plus the three class shotguns and the Engineer
pistol read field by field), decrypted in memory with the user's key and
compared against each other; the community TF2 wiki's weapon pages and
Valve Developer Community's public description of Source weapon scripts
for the general meaning of the shared fields; specs/tf2/weapons.md and
weapons_more.md for the public numbers. No game or SDK code was read.
This spec names the scripts' fields (data the game exposes) and says what
each means and where it goes in our weapon model. It deliberately gives
**none of the scripts' values**: the game supplies them at run time.
Status: draft

## Summary

Each stock weapon has one script: a KeyValues tree with a single
top-level block of flat key/value pairs plus two nested blocks (sounds and
HUD sprites). A handful of top-level keys carry the gun's numbers: damage,
pellets, spread, interval, clip, reload times, and for the launchers a
blast radius. Those map one-to-one onto fields our TOML already has
(`[damage] amount`, `[hitscan] pellets`, `[hitscan] spread`, `[trigger]
cycle`, `[magazine] clip`, `shells.insert`, the hooks' `damage` and
`radius`). Everything that makes TF2 feel like TF2 (the distance ramp,
crit rules, projectile speeds, minigun spin, sniper charge, flame
particles, sticky charge, per-class reload starts, deploy time) is **not**
in these scripts and stays in our TOML from the two weapon specs.
Several script values look like weapons.md's numbers, but a few are a
different quantity under the same name. The mapping below says which
fields are safe to apply, and which are only checked or ignored.

## Units and conventions

- Lengths in Hammer units (inches), the same unit as TF2's TOML
  (`units = { length = 0.0254 }`): **no conversion**.
- Damage in hit points, the same as the TOML's (`units.damage = 0.01`
  makes TOML numbers hit points): **no conversion**.
- Times in seconds, written as decimal text; the server tick is 0.015 s
  (weapons.md). Script times are nominal and are **not** multiples of the
  tick; see Edge cases, "Tick rounding".
- Spread uses weapons.md §6.1's convention: s is the tangent of the widest
  offset on each axis (direction = forward + a·s·right + b·s·up,
  normalised, a and b in [−1, 1]). Our `[hitscan] spread = { template =
  s }` uses **the same unit**: our template pellet is normalise(forward +
  x·s·right + y·s·up) with x, y each the sum of two uniform halves in
  [−0.5, 0.5], so x, y span [−1, 1]. The script's value goes into
  `template` unchanged. It is **not** a cone half-angle in degrees and not
  the wiki's "N:1" (that is 2/s).
- Keys are compared without regard to case: the same field appears as
  `Damage` in some scripts and `damage` in others.

## Constants

| Name (ours) | Value | Unit | Meaning |
|---|---|---|---|
| tick | 0.015 | s | server tick (weapons.md); script intervals are rounded to ticks by the timing rule, not by the mapping |
| no_clip | −1 | – | a `clip_size` of −1 means the weapon has no clip |

## Behavior

### 1. The file's shape

- Text KeyValues once decrypted (ice.md says how they are encrypted; our
  `game::scripts` already reads them). Quoted keys and values. Some values are
  **unquoted bare tokens**, which the parser must accept. Some lines carry a
  trailing `//` comment after the value, and some whole entries are
  commented out. A commented-out entry counts as absent.
- One top-level block holds everything. Its name is normally `WeaponData`,
  but three stock scripts (the Soldier's, Heavy's and Pyro's shotguns)
  name it differently. **Take the first top-level block, whatever its
  name** (our `Script::data` already does).
- Inside it: flat key/value pairs, then `SoundData` (a flat block of sound
  roles to sound-script entry names) and `TextureData` (HUD sprite cells,
  nested one level deeper). No stock script has any other nested block.
- **No fire-mode blocks.** Unlike some Source games, none of the 78
  scripts has a primary/secondary sub-block. Comments in the text group the
  fields under "primary" and "secondary" headings, but every field is top
  level and describes the primary attack. The one "secondary" field in the
  twelve guns is `secondary_ammo` (ignored). Across all 78 scripts the only
  other per-mode field is a melee key with a `Secondary_` prefix (melee is
  out of scope here).
- **No inheritance.** No script includes or bases on another. Each
  per-class shotgun is a full copy, and so are the Scout's and Engineer's
  pistols. An absent field is simply absent, and the mapping keeps our
  TOML's value for it (§4).

### 2. Fields that carry gun numbers

Each entry gives: the field, its meaning, unit and type, which of the
twelve guns have it, and how it compares with the weapon specs. "Agrees"
means the script's value is the same as the spec's number.

- **`Damage`** (also written `damage`). Number (written as an integer in
  every stock script; parse as a real). Hit points. Present in all twelve.
  - Hitscan guns: base damage **per bullet/pellet**, before the distance
    modifier and crits. Agrees with weapons.md §1 (Scattergun, Shotgun,
    Minigun) and weapons_more.md §1 (Pistol, SMG, Revolver).
  - Rocket, Grenade and Stickybomb Launchers: blast damage at the centre.
    Agrees with weapons.md §5.2 and weapons_more.md §4.3 and §5.4.
  - Syringe Gun: damage per needle. Agrees with weapons_more.md §8.
  - Flame Thrower: a **rate, hit points per second** of continuous contact
    (the file itself labels it per second). It is not per flame tick.
    Multiplied by our flame tick, it matches weapons_more.md's per-tick
    13 after rounding to a whole point (the product itself is not whole).
  - Sniper Rifle: **not the rifle's damage.** The value is far below
    weapons_more.md §3's body-shot base and has no evident relation to
    the charge rule. Do not map it (Open questions 2).
- **`BulletsPerShot`**. Integer, pellets per shot. All twelve; 1 for every
  non-shotgun. Agrees (10 for the Scattergun and Shotgun, 4 for the
  Minigun, 1 otherwise).
- **`Spread`**. Real, s in weapons.md §6.1's convention (unitless
  tangent). All twelve; zero for the launchers, the Flame Thrower,
  the Syringe Gun and the Sniper Rifle. Agrees for the Scattergun,
  Shotgun, Minigun, Pistol, SMG and Revolver. The rifle's and the Syringe
  Gun's zero agree with weapons_more.md's assumption of no spread (its
  Open questions 4 and 16).
- **`TimeFireDelay`**. Real, seconds between shots (nominal). All twelve.
  - Agrees for the Scattergun, Shotgun, Rocket Launcher, Pistol, Revolver,
    Sniper Rifle and both Demoman launchers.
  - SMG, Minigun, Syringe Gun: the specs' interval is this value
    **rounded up to a whole number of ticks**. The script's value is
    slightly smaller and not tick-aligned (Edge cases, "Tick rounding";
    Open questions 3).
  - Flame Thrower: not the damage tick and not our `cycle` (the TOML's
    cycle is the ammo rate, 12.5 per second). The script's value is a
    shorter period than either; it is probably the flame emission rate
    (Open questions 4). Not mapped.
- **`clip_size`**. Integer, rounds in a full clip; −1 means no clip. All
  twelve. Agrees with the specs' clips for the eight clip guns. It is −1
  for the Minigun, the Flame Thrower and the Sniper Rifle. Our TOML gives
  those a "clip" that is really their whole ammo (the class's maximum),
  and that clip **stays the TOML's**.
- **`default_clip`**. Integer, rounds in the clip when the weapon is
  given. Absent on the Minigun and the Flame Thrower. Equal to `clip_size`
  on every clip gun. On the Sniper Rifle it is set even though there is no
  clip, and it differs from weapons_more.md's 25 carried (Open
  questions 5). Not mapped (our guns spawn with a full clip). Use it only
  as a cross-check: on a clip gun, a `default_clip` different from
  `clip_size` is logged.
- **`TimeReload`**. Real, seconds. Present on the four one-round-at-a-time
  guns (Scattergun, Shotgun, Rocket, Grenade and Stickybomb Launchers)
  and, oddly, on the Pistol; absent on the SMG, Revolver, Syringe Gun,
  Minigun, Flame Thrower and Sniper Rifle.
  - On a gun that reloads **one round at a time**, it is the time per
    round inserted. Agrees with the inserts of weapons.md §4 (Scattergun,
    Shotgun) and weapons_more.md §9.3 (Grenade and Stickybomb Launchers).
    For the **Rocket Launcher** the script's per-rocket time is slightly
    longer than weapons.md's 0.8 (Open questions 6).
  - On the **Pistol** (a whole-clip reload) it is much shorter than
    weapons_more.md's whole-clip reload time. The clip guns' real reload
    time comes from elsewhere (most likely the reload animation), so
    **never map `TimeReload` to a whole-clip reload**.
- **`TimeReloadStart`**. Real, seconds. Present on the same one-at-a-time
  guns only. Every script has the same short value, which differs from
  each gun's start in weapons.md §4 and weapons_more.md §9.3. Those starts
  vary by gun and by class and equal the class arms' reload-start
  sequence lengths. So this field is at most a floor under the animation,
  not the start. **Not mapped**: `shells.start` and `tf2:gun`'s
  `reload_start` stay the TOML's (Open questions 7).
- **`DamageRadius`**. Real, Hammer units, blast radius. Only the Grenade
  and Stickybomb Launchers have it. It agrees with the specs' 146. The
  **Rocket Launcher's script has no radius field**, so its radius stays
  the TOML's.
- **`Range`**. Real, Hammer units. Hitscan guns have a value; the Rocket
  Launcher and Flame Thrower have zero; the Demoman launchers have none.
  It **differs** from weapons.md §6.1's effective bullet reach (the wiki's
  ≈ 56 754 units, the map's diagonal): it is far shorter, and shorter
  still on the Pistol and Revolver than on the other bullet guns. Not
  mapped until measured (Open questions 1).
- **`AmmoPerShot`**. Integer, ammo used per shot. Present only on the
  Scattergun and the four Shotgun scripts (value 1); absent means 1. Our
  model uses one round per shot, so a value other than 1 cannot be
  expressed yet. Log it and keep one.
- **`PunchAngle`**. Real, degrees of view kick per shot. Present (non-zero)
  on the Scattergun, Shotguns and the three launchers. The Engineer's
  primary Shotgun kicks more than the three class-secondary Shotguns.
  weapons.md says TF2 has no recoil, and the TOML's `punch_scale` is 0.
  **Not mapped** (Open questions 8).
- **`UseRapidFireCrits`**. Flag 0/1; absent means 0. Set on the Minigun,
  Flame Thrower, Pistol (both), SMG and Syringe Gun. It selects the
  rapid-fire crit rule (weapons.md §2.6: a roll once a second, 2 s bursts)
  over a roll per shot. Agrees with weapons_more.md §1, and settles its
  Open questions 5 (the SMG is rapid-fire). Our TOML has no crit field
  yet, so this field has no target until crits become data. A crit hook
  would read it.
- **`ProjectileType`**. String naming what a shot produces: a bullet, a
  rocket, a pipe grenade, a remote (sticky) pipe or a syringe. Absent on
  the Flame Thrower. It is used to **check** that the TOML's delivery
  matches (Mapping, "kind check"), never to change it.

### 3. Fields that place or present the gun (not gun numbers)

- **`WeaponType`**: `primary` or `secondary`, the loadout slot. It agrees
  with our `slot` (0 and 1) for all twelve. Check it; don't map it.
- **`bucket`**, **`bucket_position`**: the HUD weapon-selection column and
  row. They are not our `slot`: the Spy's Revolver is in the first column
  (his first weapon) while its loadout slot is secondary. Ignored.
- **`weight`**: auto-switch priority. TF2 has no gun pickups (see the next
  item), so this never matters. Ignored (our `weight` stays unset).
- **`ITEM_FLAG_NOITEMPICKUP`**: the weapon can't be picked up from the
  world. Ignored.
- **`primary_ammo`**: the ammo pool, a primary or secondary pool of the
  class. The class's maxima are in the class scripts, not here. Ignored
  here; our ammo types stay per gun. **`secondary_ammo`**: none on most;
  the Minigun names its primary pool (its spin uses no ammo, weapons.md
  §7.1). Ignored.
- **`TimeIdle`**, **`TimeIdleEmpty`**: seconds after a shot before the
  view model's idle animation, with ammo and empty. On the launchers they
  equal the fire interval. They could feed `[anim] idle_after`, but TF2's
  TOMLs have no `[anim]` table. Ignored.
- **`printname`**: the localisation token of the HUD name. Our `[hud]
  name` is a plain display name. Ignored (a token-resolving HUD could use
  it later).
- **`viewmodel`**, **`playermodel`**: legacy model paths, present on a few
  (Pistol, SMG, Flame Thrower, Grenade Launcher) and commented out on the
  rest. The item schema (`items_game.txt`) is what defines the models
  today (install.md). Ignored; `[models]` stays the TOML's.
- **`anim_prefix`**: the third-person animation family. Ignored
  (player_animation.md chooses sequences by weapon kind).
- **`BuiltRightHanded`**, **`HasTeamSkins`**, **`HasTeamSkins_Viewmodel`**,
  **`HasTeamSkins_Worldmodel`**: model handedness and team-skin flags.
  Ignored. (The TOMLs' `view_right_handed` comes from the `c_` rig, not
  from this field.)
- **`BrassModel`**, **`DoInstantEjectBrass`**, **`TracerEffect`**,
  **`MuzzleFlashParticleEffect`**, **`ExplosionEffect`**,
  **`ExplosionPlayerEffect`**, **`ExplosionWaterEffect`**: shell casings,
  tracer and particle-system names. Ignored (no particle systems yet).
- **`ExplosionSound`**: the blast's sound entry (Rocket, Grenade and
  Stickybomb Launchers). Agrees with the TOMLs' hook `sound` for all
  three; this also answers weapons_more.md Open questions 18 by name.
  Check only.
- **`SoundData`**: sound roles to entry names. `single_shot` is the shot
  (it agrees with `[sounds] fire` on all ten guns that have it).
  `empty` is the dry fire (agrees with `[sounds] empty`). `reload`
  names the **third-person** reload entry (`…WorldReload`), not the
  first-person entries our `[sounds] reload` times, so it is not mapped.
  `burst` is the crit shot sound (no target yet). `double_shot` and
  `special1`–`special3` are per gun:
  - Minigun: fire loop, wind-up, wind-down and spin loop. All agree with
    `tf2:minigun`'s sound params.
  - Flame Thrower: start, loop, airblast, pilot light and wind-down.
    **Its start and end entries differ from the TOML's** `start_sound` and
    `end_sound` (Open questions 9).
  - Scattergun and Shotguns: a pump sound.
  - Launchers: a mode-switch sound; the Stickybomb Launcher's agrees with
    `detonate_sound`.
  Sounds are checked, not mapped.
- **`TextureData`**: HUD sprite cells (bucket images, ammo icon,
  crosshair). Ignored (hud.md uses the kill-icon table and HUD scheme).

### 4. What has no script field (stays the TOML's)

The draw time (`draw`, weapons.md §3) and `class`; trigger `automatic`
and `timing`; shell `start` and `back_to_back`; `auto_reload`; whole-clip
reload times; ammo reserves (`ammo.toml`); `[hitscan] range` (see
`Range` above) and `punch_scale`; `[damage]` falloff, hitgroups, impulse
and quantum; the models and HUD; and every hook parameter except those in
the Mapping. In detail:

- `tf2:gun`: `ramp`, `arms`, `reload_start`, `first_pellet_pause`,
  `flat`.
- `tf2:rocket`: `radius` (no field on the rocket), `edge`,
  `self_air_scale` and the pushes.
- `chimera:projectile`: speed, gravity, offsets, life.
- `tf2:minigun`: all of it.
- `tf2:sniper`: all of it, `base` included.
- `tf2:grenade` and `tf2:sticky`: all keys except `damage` and `radius`.
- `tf2:flame`: all keys except `damage`.

### 5. Applying a script (once per gun load)

1. Read the gun's script by `tf2:gun`'s `script` name. If it is missing,
   keep every TOML number and log "spec fallback" (as today).
2. Take the first top-level block. If there is none, treat the script as
   missing.
3. **Kind check**: compare `ProjectileType` (or its absence) and
   `WeaponType` with the TOML (Mapping). On a mismatch, log it and apply
   nothing for this gun.
4. Apply each mapped field that is present and well-formed (Edge cases).
   Each field is independent: a bad or absent field leaves only its own
   target at the TOML's value.
5. Run the cross-checks (default_clip, sounds, ExplosionSound), log the
   differences, and change nothing for them.
6. Log, once per gun, that its numbers came from the script, listing which
   targets the script set and which stayed the TOML's.

## Mapping

"All bullet guns" means the Scattergun, Shotgun, Minigun, Pistol, SMG and
Revolver; the Sniper Rifle is listed separately. "Shell guns" means
Scattergun, Shotgun, Rocket, Grenade and Stickybomb Launchers.

| Script field | Our target | Conversion | Guns |
|---|---|---|---|
| `Damage` / `damage` | `[damage] amount` (per pellet) | none (hit points) | all bullet guns; **not** the Sniper Rifle |
| `Damage` | `tf2:rocket` `damage` | none | Rocket Launcher |
| `damage` | `tf2:grenade` `damage` | none | Grenade Launcher |
| `damage` | `tf2:sticky` `damage` | none | Stickybomb Launcher |
| `Damage` | `tf2:needle` `damage` | none | Syringe Gun |
| `Damage` | `tf2:flame` `damage` (per flame tick) | × `tf2:flame` `tick` (the TOML's); rounding: Open questions 4 | Flame Thrower |
| `BulletsPerShot` | `[hitscan] pellets` | integer | all bullet guns and the Sniper Rifle |
| `Spread` | `[hitscan] spread = { template }` | none (same unit) | all bullet guns and the Sniper Rifle; ignored on projectiles and the flame |
| `TimeFireDelay` | `[trigger] cycle` | none (s); the timing rule rounds to ticks | all except the Flame Thrower |
| `clip_size` (≥ 1) | `[magazine] clip` | integer | the eight clip guns; −1 leaves the TOML's |
| `TimeReload` | `[magazine] shells.insert` and `[magazine] reload` | none (s) | shell guns only (those whose TOML has `shells`) |
| `DamageRadius` | `tf2:grenade` / `tf2:sticky` `radius` | none (units) | Grenade, Stickybomb Launchers |
| `ProjectileType` | kind check: bullet ⇒ the TOML has `[damage]`; rocket ⇒ `tf2:rocket`; pipe ⇒ `tf2:grenade`; remote pipe ⇒ `tf2:sticky`; syringe ⇒ `tf2:needle`; absent ⇒ `tf2:flame` | – | all |
| `WeaponType` | check: primary ⇔ `slot` 0, secondary ⇔ `slot` 1 | – | all |
| `default_clip` | check: equals `clip_size` on clip guns | – | clip guns |
| `SoundData` `single_shot`, `empty`; `ExplosionSound`; minigun roles | check against `[sounds] fire`, `empty`, the hooks' sounds | – | as listed in §3 |
| `UseRapidFireCrits` | (future crit hook) | flag | – |
| `Range`, `TimeReloadStart`, `PunchAngle`, `AmmoPerShot` | not mapped (Open questions 1, 7, 8; `AmmoPerShot` ≠ 1 logged) | – | – |

The existing seam's `ScriptNumbers` (damage, pellets, interval, clip,
spread) covers the first block of rows. The reload insert, the radius and
the hook damages need fields added to it.

## Edge cases

- **Missing field**: the target keeps the TOML's value. There are no
  defaults from another script and no inheritance.
- **Zero or negative**: `clip_size` −1 means no clip (keep the TOML's);
  a `clip_size` of 0 or below −1 is malformed. `Spread` 0 is a valid value
  (a perfectly accurate gun). A `Damage`, `TimeFireDelay`, `TimeReload` or
  `DamageRadius` of zero or less is malformed: keep the TOML's and log it.
- **Integer versus real**: `clip_size`, `default_clip`, `BulletsPerShot`
  and `AmmoPerShot` are integers. A fractional value there is malformed
  (keep the TOML's; our `KvView::integer` already refuses fractions).
  Damage is a real even though every stock script writes whole numbers;
  parse it as a real so a fractional value works.
- **Unparsable text** (e.g. a stray word) in a number field: malformed,
  keep the TOML's.
- **Case**: `Damage` and `damage` are the same field. If a block had both
  (no stock script does), use the first in file order, as our view does.
- **Root block name**: not always `WeaponData` (§1); never match on it.
- **Tick rounding**: script intervals are nominal. With the TOMLs'
  `timing = "accumulate"` (the next shot due at last due + interval, firing
  on the first tick at or after it), a non-tick-aligned interval gives
  uneven tick gaps whose average is the nominal interval. With weapons.md's
  measured "rounded up" numbers, every gap is the same whole number of
  ticks. The two give different shots per second (Open questions 3).
  Implementations must compare due times with a small tolerance: a due
  time that lands exactly on a tick in exact arithmetic (a multiple of
  0.015) must fire on that tick, not the next, despite floating-point
  error.
- **Class shotguns**: the Engineer's primary shotgun and the three class
  secondaries agree on every mapped number. They differ in `PunchAngle`,
  `weight`, `WeaponType` and `primary_ammo`, none of which is mapped. Our
  single `tf_weapon_shotgun_primary` gun therefore maps from
  `tf_weapon_shotgun_primary` alone. There is no plain
  `tf_weapon_shotgun` script.
- **Pistol**: the Scout's and the Engineer's scripts agree on every mapped
  number. Their different reserve and reload time (weapons_more.md §1)
  come from elsewhere.
- **Shell guns' reload**: the TOMLs set `[magazine] reload` equal to
  `shells.insert`, so set both from `TimeReload` to keep them equal.

## Quirks

- The same name means different things on different guns: `Damage` is
  per pellet, per blast, per needle or **per second** (flame), and on the
  Sniper Rifle it is not the shot's damage at all. `TimeReload` is per
  round on shell guns and meaningless for the Pistol's clip reload. The
  mapping must decide per gun kind, never per field alone.
- `TimeReloadStart` is the same everywhere and is not the start players
  see. Keep the animation-derived starts.

## Test cases

All use **synthetic scripts** (invented text in the test, never the
game's). They use the field names above with invented values, and start
from an invented TOML gun or a copy of a real TOML gun.

| Setup | Input (synthetic script fields) | After | Expected |
|---|---|---|---|
| a bullet gun TOML (amount 5, pellets 1, cycle 0.3, clip 9, template 0.01) | `Damage` 7, `BulletsPerShot` 3, `TimeFireDelay` 0.2, `clip_size` 8, `Spread` 0.05, `ProjectileType` bullet, `WeaponType` primary, TOML slot 0 | apply | amount 7, pellets 3, cycle 0.2, clip 8, template 0.05; log says "script" |
| same | the same with the key written `damage` | apply | amount 7 (case-insensitive) |
| same | root block named anything other than `WeaponData` | apply | the same as row 1 |
| same | only `Damage` 7 | apply | amount 7; pellets, cycle, clip, spread are the TOML's (1, 0.3, 9, 0.01) |
| same | `clip_size` −1 | apply | clip stays 9 |
| same | `clip_size` "8.5", `BulletsPerShot` "two", `TimeFireDelay` 0 | apply | clip 9, pellets 1, cycle 0.3 (all malformed, kept); others applied |
| same | `Damage` "7.5" | apply | amount 7.5 |
| same | `ProjectileType` rocket | apply | nothing applied; a kind-mismatch log |
| same | `WeaponType` secondary (TOML slot 0) | apply | nothing applied; a mismatch log |
| a shell gun TOML (insert 0.4, reload 0.4, start 0.25) | `TimeReload` 0.3, `TimeReloadStart` 0.05 | apply | insert 0.3, reload 0.3, start 0.25 (unchanged) |
| a clip gun TOML (no `shells`, reload 1.2) | `TimeReload` 0.3 | apply | reload 1.2 (unchanged) |
| a rocket TOML (`tf2:rocket` damage 50, radius 100) | `Damage` 60, `DamageRadius` 120 | apply | damage 60, radius 100 (the rocket's radius is never mapped) |
| a grenade TOML (`tf2:grenade` damage 50, radius 100) | `damage` 60, `DamageRadius` 120, `ProjectileType` pipe | apply | damage 60, radius 120 |
| a sniper TOML (amount 50, `tf2:sniper` base 50) | `Damage` 3, `TimeFireDelay` 1.1 | apply | amount 50, base 50 (not mapped); cycle 1.1 |
| a flame TOML (`tf2:flame` tick 0.1, damage 9, cycle 0.08) | `Damage` 150, `TimeFireDelay` 0.03, no `ProjectileType` | apply | flame damage 15 (150 × 0.1); cycle 0.08 (unchanged) |
| a needle TOML | `Damage` 12, `ProjectileType` syringe | apply | `tf2:needle` damage 12 |
| any gun | `Range` 1000, `PunchAngle` 2, `TimeReloadStart` 0.05 | apply | range, punch_scale and starts are the TOML's |
| any gun | `AmmoPerShot` 2 | apply | one round per shot; a log naming the field |
| a clip gun | `clip_size` 8, `default_clip` 6 | apply | clip 8; a cross-check log |
| no script | – | load | every number is the TOML's; log says "spec fallback" |
| tick timing, `accumulate` | synthetic `TimeFireDelay` 0.05 held from t = 0 | 10 shots | shot ticks 0, 4, 7, 10, 14, 17, 20, …: gaps 4, 3, 3 repeating (due 0.15 lands exactly on tick 10 and fires there) |

With the key present (a heavy test, skipped without it): for each of the
twelve guns, the mapped damage, pellets, clip and spread equal the
committed TOML's (the specs' public numbers), except the cases already
named as differing here: the SMG, Minigun and Syringe Gun interval
(rounding), the Rocket Launcher insert (Open questions 6) and the flame
damage (rounding). The test lists those exceptions explicitly so that any
new disagreement fails.

## Open questions

1. **`Range`** versus weapons.md's ≈ 56 754-unit effective reach (the
   wiki's "Hitscan" page). The script's range is much shorter, and shorter
   still on the Pistol and Revolver. Measure on the dedicated server: fire
   the Pistol and the Shotgun at a puppet beyond the script's range but
   within the wiki's. Until then `[hitscan] range` stays the TOML's.
2. **Sniper Rifle `Damage`** is a small number unrelated to the
   50–150 charge rule. Perhaps a leftover, or a per-something rate the
   game's code doesn't use. Not mapped; the charge stays `tf2:sniper`'s.
3. **Interval and ticks** for the SMG, Minigun and Syringe Gun: the
   script's nominal interval under our `accumulate` timing gives a higher
   rate than the specs' measured tick-rounded interval. Count the shots in
   10 s of held fire on the server (Minigun spun: 200 rounds, time to
   empty). If the measured rate matches the rounded interval, TF2 times
   each shot from the tick it fired: switch these guns' `timing` to "set"
   and map the script's value. If it matches the nominal value, keep
   "accumulate". Until measured, map the script's value and keep the
   TOMLs' timing.
4. **Flame Thrower**: what `TimeFireDelay` governs (most likely the flame
   particle emission period; it is shorter than both the damage tick and
   the ammo period). Also whether per-tick damage is the per-second rate ×
   0.075 kept fractional (rounded per hit by the damage rule) or rounded
   when computed. weapons_more.md's 13 is the rounded value.
5. **Sniper `default_clip`**: set although the rifle has no clip, and
   different from weapons_more.md's 25 carried. The class script's ammo
   maximum probably governs. Check the Sniper's ammo on spawn on the
   server; the class scripts (`scripts/playerclasses/*.ctx`) are a later
   spec.
6. **Rocket Launcher reload**: the script's per-rocket time is slightly
   longer than weapons.md's 0.8 (from the wiki). The mapping applies the
   script's. Measure the full reload from empty on the server (weapons.md
   probe P4) to confirm.
7. **`TimeReloadStart`**: whether the start players see is the longer of
   this and the arms' reload-start sequence, or the sequence alone. Either
   way it is never shorter than the TOML's starts, so not mapping it is
   safe.
8. **`PunchAngle`**: TF2's Scattergun, Shotguns and launchers are commonly
   described as having no recoil, yet the scripts give a view kick. Check
   on the Windows client whether firing moves the view (a pitch change in
   `getpos` before and after one shot). If it does, a `tf2:` punch hook
   would read this field.
9. **Flame sounds**: the script's start and wind-down roles name entries
   different from the TOML's `start_sound` and `end_sound`. Check which
   entries exist in `game_sounds_weapons.txt` and which the live game
   plays.
10. **Crit field**: `UseRapidFireCrits` has no target until crits are
    data; the crit spec work should read it from here.
