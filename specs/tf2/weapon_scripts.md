# Team Fortress 2: the per-weapon scripts' fields (T8)

Source basis: the dedicated server install's encrypted per-weapon scripts
(`scripts/tf_weapon_*.ctx`, all 78 surveyed for their field names, the
twelve stock guns' scripts plus the three class shotguns and the Engineer
pistol read field by field), decrypted in memory with the user's key and
compared against each other; the community TF2 wiki's weapon pages and
Valve Developer Community's public description of Source weapon scripts
for the general meaning of the shared fields; specs/tf2/weapons.md and
weapons_more.md for the public numbers. For the 2026-11-15 check, the
SDK's TF2 game code (Valve's Source SDK 2013 with TF2's client and
server code, read in place on the Linux box, nothing copied) was read
for how the weapon-script reader takes each field (its defaults, its
types, the per-mode keys) and for which fields the game's weapon logic
actually uses. This spec names the scripts' fields (data the game exposes) and says what
each means and where it goes in our weapon model. It deliberately gives
**none of the scripts' values**: the game supplies them at run time.
Status: checked against the SDK's game code (2026-11-15)

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

## Corrections from the SDK (2026-11-15)

- `Damage` "a real, parse as one" -> read as an **integer**; a fractional
  value is cut toward zero (7.5 -> 7).
- "No per-mode fields" -> every per-mode field also has a
  `Secondary_`-prefixed twin read for a second fire mode, defaulting to
  the primary's value; no stock gun sets one (§1).
- Defaults "absent means absent" -> the reader gives every field a
  default when absent (§1, "Defaults"); `default_clip` defaults to
  `clip_size`, `Range` to 8192.
- `Range` "not mapped, differs from the reach" -> it **is** the bullet's
  trace length: map it to `[hitscan] range` (Open questions 1 settled).
- Sniper Rifle `Damage` "unexplained" -> ignored by the rifle, whose shot
  is max(charge, 50) with the charge capped at 150 (Open questions 2
  settled).
- Flame Thrower `Damage` "a per-second rate, map × tick" -> **ignored**:
  the stock flame thrower uses a fixed 13 per flame, one flame every
  0.02 s, and 14 ammo per second; its `TimeFireDelay` is ignored too
  (Open questions 4 settled; the flame row leaves the Mapping).
- Interval "accumulate, open" -> the next shot is due at this shot's
  time + `TimeFireDelay`, so the gap is the interval rounded up to whole
  ticks (Open questions 3 settled: timing "set").
- `TimeReload` "per-round insert of every shell gun" -> the insert is the
  view model's reload-loop sequence length; only the **Grenade
  Launcher** times its insert by `TimeReload`. A whole-clip reload is the
  reload sequence's length − 0.2 s. `TimeReload` is the fallback when the
  view model has no such sequence (Open questions 6 settled).
- `TimeReloadStart` "a floor, open" -> only a fallback when the view
  model has no reload-start sequence; the start is that sequence's
  length (Open questions 7 settled).
- `PunchAngle` "not mapped, recoil unknown" -> a real view kick: after
  each shot the view pitch is kicked up by a random whole number of
  degrees from P − 1 to P + 1 (Open questions 8 settled).
- `TimeIdleEmpty` "seconds after the last shot" -> never read: the
  reader looks the key up misspelled (`TimeIdleEmpy`), so it is always 0.
- Malformed text "kept the TOML's" -> the game reads it as 0 (an integer
  field reads the leading digits); our mapping still keeps the TOML's
  (§5, deliberate).
- `WeaponType` "check only" -> it also chooses the third-person
  animation set (player_animation.md §2).

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
- **Two fire modes by prefix (SDK).** The game keeps two modes per
  weapon. The primary reads the plain keys; the secondary reads the same
  keys with a `Secondary_` prefix (`Secondary_Damage`, `Secondary_Range`,
  `Secondary_BulletsPerShot`, `Secondary_Spread`, `Secondary_PunchAngle`,
  `Secondary_TimeFireDelay`, `Secondary_TimeIdle`,
  `Secondary_TimeReloadStart`, `Secondary_TimeReload`,
  `Secondary_DrawCrosshair`, `Secondary_AmmoPerShot`,
  `Secondary_ProjectileType`), each defaulting to the primary's value
  when absent; `Secondary_UseRapidFireCrits` defaults to 0 and
  `Secondary_SmackDelay` to 0.2 instead. The stock guns fire in the
  primary mode, so the plain keys are the ones that matter.
- **Defaults (SDK).** When a key is absent the game uses: `Damage` 0,
  `Range` 8192, `BulletsPerShot` 0, `Spread` 0, `PunchAngle` 0,
  `TimeFireDelay` 0, `TimeIdle` 0, `TimeReloadStart` 0, `TimeReload` 0,
  `DrawCrosshair` 1, `AmmoPerShot` 1, `UseRapidFireCrits` 0,
  `ProjectileType` none, `ProjectileSpeed` 0, `SmackDelay` 0.2,
  `clip_size` −1 (no clip), `default_clip` = `clip_size`, `weight` 0,
  `bucket` and `bucket_position` 0, `DamageRadius` 0, `PrimerTime` 0,
  `DoInstantEjectBrass` 1, `BuiltRightHanded` 1. Our mapping does not
  take these defaults: an absent field keeps the TOML's number (§4).
- **Types (SDK).** `Damage`, `BulletsPerShot`, `AmmoPerShot`,
  `clip_size`, `default_clip`, `bucket`, `weight` and the 0/1 flags are
  read as integers: a fractional value is cut toward zero, and text that
  is not a number reads as its leading digits (0 if none). The other
  number fields are reals; non-numeric text reads as 0. Keys are found
  without regard to case.
- **No inheritance.** No script includes or bases on another. Each
  per-class shotgun is a full copy, and so are the Scout's and Engineer's
  pistols. An absent field is simply absent, and the mapping keeps our
  TOML's value for it (§4).

### 2. Fields that carry gun numbers

Each entry gives: the field, its meaning, unit and type, which of the
twelve guns have it, and how it compares with the weapon specs. "Agrees"
means the script's value is the same as the spec's number.

- **`Damage`** (also written `damage`). An **integer** (the game reads
  it as one and cuts a fraction toward zero). Hit points. Present in all
  twelve.
  - Hitscan guns: base damage **per bullet/pellet**, before the distance
    modifier and crits. Agrees with weapons.md §1 (Scattergun, Shotgun,
    Minigun) and weapons_more.md §1 (Pistol, SMG, Revolver).
  - Rocket, Grenade and Stickybomb Launchers: blast damage at the centre.
    Agrees with weapons.md §5.2 and weapons_more.md §4.3 and §5.4.
  - Syringe Gun: damage per needle. Agrees with weapons_more.md §8.
  - Flame Thrower: labelled a rate, hit points per second, in the file,
    but **the game ignores it** (SDK): each flame the stock flame thrower
    emits carries a fixed 13, whatever the script says. Do not map it.
  - Sniper Rifle: **not the rifle's damage.** The game ignores it (SDK):
    the rifle's shot is max(charged damage, 50), the charge growing at
    50 per second to at most 150 (weapons_more.md §3). Do not map it.
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
    slightly smaller and not tick-aligned. The SDK settles why: the game
    sets the next shot's time to the current shot's time plus this value
    (it never carries the remainder), and a shot can only happen on a
    tick, so each gap is the value rounded up to whole ticks (Edge cases,
    "Tick rounding").
  - Flame Thrower: **ignored by the game** (SDK): the stock flame thrower
    emits a flame every 0.02 s whatever the script says, and spends 14
    ammo per second (a fractional remainder carried between flames).
    Not mapped.
- **`clip_size`**. Integer, rounds in a full clip; −1 means no clip. All
  twelve. Agrees with the specs' clips for the eight clip guns. It is −1
  for the Minigun, the Flame Thrower and the Sniper Rifle. Our TOML gives
  those a "clip" that is really their whole ammo (the class's maximum),
  and that clip **stays the TOML's**.
- **`default_clip`**. Integer, rounds in the clip when the weapon is
  given; absent, it equals `clip_size` (SDK). Absent on the Minigun and
  the Flame Thrower. Equal to `clip_size`
  on every clip gun. On the Sniper Rifle it is set even though there is no
  clip, and it differs from weapons_more.md's 25 carried (Open
  questions 5). Not mapped (our guns spawn with a full clip). Use it only
  as a cross-check: on a clip gun, a `default_clip` different from
  `clip_size` is logged.
- **`TimeReload`**. Real, seconds. Present on the four one-round-at-a-time
  guns (Scattergun, Shotgun, Rocket, Grenade and Stickybomb Launchers)
  and, oddly, on the Pistol; absent on the SMG, Revolver, Syringe Gun,
  Minigun, Flame Thrower and Sniper Rifle.
  - On a gun that reloads **one round at a time**, the game times each
    round by the **view model's reload-loop sequence length** when the
    view model has that sequence (the stock ones all do), and uses
    `TimeReload` only as a fallback. The one exception is the **Grenade
    Launcher**, which always times its rounds by `TimeReload` (SDK). So
    the script's value is the insert only for the Grenade Launcher; for
    the Scattergun, Shotgun and Stickybomb Launcher it merely agrees with
    the animation, and for the Rocket Launcher it does not (slightly
    longer than weapons.md's 0.8, which is the animation's).
  - On the **Pistol** (a whole-clip reload) it is much shorter than
    weapons_more.md's whole-clip reload time. The game times a whole-clip
    reload as the view model's reload sequence length **minus 0.2 s**
    (the reload counts as done a little before the animation ends), and
    uses `TimeReload` only when there is no such sequence, so **never
    map `TimeReload` to a whole-clip reload**.
- **`TimeReloadStart`**. Real, seconds. Present on the same one-at-a-time
  guns only. Every script has the same short value, which differs from
  each gun's start in weapons.md §4 and weapons_more.md §9.3. Those starts
  vary by gun and by class and equal the class arms' reload-start
  sequence lengths. The SDK confirms it: the start is the view model's
  reload-start sequence length, and this field is used only when the view
  model has no such sequence. **Not mapped**: `shells.start` and
  `tf2:gun`'s `reload_start` stay the TOML's.
- **`DamageRadius`**. Real, Hammer units, blast radius. Only the Grenade
  and Stickybomb Launchers have it. It agrees with the specs' 146. The
  **Rocket Launcher's script has no radius field**: the game's rocket
  uses a fixed 146 (SDK), so its radius stays the TOML's.
- **`Range`**. Real, Hammer units; 8192 when absent. Hitscan guns have a
  value; the Rocket Launcher and Flame Thrower have zero; the Demoman
  launchers have none. The SDK settles its meaning: it is **the length of
  each bullet's trace**. A target beyond it is not hit. It therefore
  replaces weapons.md §6.1's effective reach (the wiki's ≈ 56 754 units)
  for the bullet guns: shorter, and shorter still on the Pistol and
  Revolver. **Mapped** to `[hitscan] range`.
- **`AmmoPerShot`**. Integer, ammo used per shot. Present only on the
  Scattergun and the four Shotgun scripts (value 1); absent means 1 (the
  game's default too). Our
  model uses one round per shot, so a value other than 1 cannot be
  expressed yet. Log it and keep one.
- **`PunchAngle`**. Real, degrees of view kick per shot. Present (non-zero)
  on the Scattergun, Shotguns and the three launchers. The Engineer's
  primary Shotgun kicks more than the three class-secondary Shotguns.
  The SDK shows it is used: right after each shot (bullet or projectile)
  the player's punch pitch moves **up** by a random whole number of
  degrees between P − 1 and P + 1 (the bounds cut to whole numbers;
  shared-seed random, so client and server agree). For the
  Scattergun/Shotgun family this happens on every shot; for the other
  guns only when P > 0. The punch then decays by the shared Source punch
  decay, and the next shot's direction includes the current punch.
  weapons.md's "no recoil" and the TOML's `punch_scale` 0 are therefore
  wrong for these guns. Not mapped yet (our punch is a per-shot scale,
  not this kick); a `tf2:` punch hook should read it.
- **`UseRapidFireCrits`**. Flag 0/1; absent means 0. Set on the Minigun,
  Flame Thrower, Pistol (both), SMG and Syringe Gun. It selects the
  rapid-fire crit rule (weapons.md §2.6: a roll once a second, 2 s bursts)
  over a roll per shot. Agrees with weapons_more.md §1, and settles its
  Open questions 5 (the SMG is rapid-fire). Our TOML has no crit field
  yet, so this field has no target until crits become data. A crit hook
  would read it.
- **`ProjectileType`**. String naming what a shot produces: a bullet, a
  rocket, a pipe grenade, a remote (sticky) pipe or a syringe. Absent on
  the Flame Thrower. The game compares it, ignoring case, with its own
  list of projectile names; absent or unknown means "none" (the shared
  firing path then fires nothing; the flame thrower has its own attack).
  It is used to **check** that the TOML's delivery matches (Mapping,
  "kind check"), never to change it.

### 3. Fields that place or present the gun (not gun numbers)

- **`WeaponType`**: `primary` or `secondary`, the loadout slot. It agrees
  with our `slot` (0 and 1) for all twelve. Check it; don't map it. The
  game also uses it (unless the item schema gives an animation slot) to
  choose the third-person activity set (player_animation.md §2). Other
  values the game knows: `melee`, `grenade`, `building`, `pda`, `item1`,
  `item2`.
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
  equal the fire interval. Quirk (SDK): the game looks the second one up
  as `TimeIdleEmpy` (sic), so the scripts' `TimeIdleEmpty` is never read
  and the empty idle time is always 0. They could feed `[anim] idle_after`, but TF2's
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
- **`DrawCrosshair`** (SDK; absent means 1): 0 hides the weapon's own
  crosshair (hud.md 3.4). Ignored.
- **`ProjectileSpeed`** (SDK; absent means 0): a launch speed some
  projectile weapons read; the twelve guns' speeds stay
  `chimera:projectile`'s.

### 4. What has no script field (stays the TOML's)

The draw time (`draw`, weapons.md §3) and `class`; trigger `automatic`
and `timing`; shell `start` and `back_to_back`; `auto_reload`; whole-clip
reload times; ammo reserves (`ammo.toml`); `punch_scale` (see
`PunchAngle` above); `[damage]` falloff, hitgroups, impulse
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
- `tf2:flame`: all of it (the game ignores the script's flame numbers).

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
| `BulletsPerShot` | `[hitscan] pellets` | integer | all bullet guns and the Sniper Rifle |
| `Spread` | `[hitscan] spread = { template }` | none (same unit) | all bullet guns and the Sniper Rifle; ignored on projectiles and the flame |
| `TimeFireDelay` | `[trigger] cycle` | none (s); with `timing = "set"` (Edge cases, "Tick rounding") | all except the Flame Thrower |
| `clip_size` (≥ 1) | `[magazine] clip` | integer | the eight clip guns; −1 leaves the TOML's |
| `TimeReload` | `[magazine] shells.insert` and `[magazine] reload` | none (s) | the Grenade Launcher only (the other shell guns time a round by their animation) |
| `Range` | `[hitscan] range` | none (units) | all bullet guns and the Sniper Rifle |
| `DamageRadius` | `tf2:grenade` / `tf2:sticky` `radius` | none (units) | Grenade, Stickybomb Launchers |
| `ProjectileType` | kind check: bullet ⇒ the TOML has `[damage]`; rocket ⇒ `tf2:rocket`; pipe ⇒ `tf2:grenade`; remote pipe ⇒ `tf2:sticky`; syringe ⇒ `tf2:needle`; absent ⇒ `tf2:flame` | – | all |
| `WeaponType` | check: primary ⇔ `slot` 0, secondary ⇔ `slot` 1 | – | all |
| `default_clip` | check: equals `clip_size` on clip guns | – | clip guns |
| `SoundData` `single_shot`, `empty`; `ExplosionSound`; minigun roles | check against `[sounds] fire`, `empty`, the hooks' sounds | – | as listed in §3 |
| `UseRapidFireCrits` | (future crit hook) | flag | – |
| `TimeReloadStart`, `PunchAngle`, `AmmoPerShot` | not mapped (the start is the animation's; the kick waits for a punch hook; `AmmoPerShot` ≠ 1 logged) | – | – |
| `Damage`, `TimeFireDelay` | not mapped: the game ignores them | – | Flame Thrower |

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
- **Integer versus real**: `clip_size`, `default_clip`, `BulletsPerShot`,
  `AmmoPerShot` and `Damage` are integers in the game. A fractional value
  in the first four is malformed (keep the TOML's; our `KvView::integer`
  already refuses fractions). A fractional `Damage` is cut toward zero,
  as the game does (7.5 -> 7).
- **Unparsable text** (e.g. a stray word) in a number field: the game
  reads it as 0 (or as its leading digits for an integer field); we
  treat it as malformed and keep the TOML's, deliberately, so a damaged
  record cannot zero a gun.
- **Case**: `Damage` and `damage` are the same field. If a block had both
  (no stock script does), use the first in file order, as our view does.
- **Root block name**: not always `WeaponData` (§1); never match on it.
- **Tick rounding**: script intervals are nominal. The game (SDK) sets
  the next shot's due time to the time of the tick the shot fired on plus
  the interval, and a held trigger fires on the first tick at or after
  it. So every gap is the interval rounded **up** to whole ticks:
  gap = ⌈interval / 0.015⌉ ticks, the same every shot. This is our
  `timing = "set"`, not "accumulate" (which carries the remainder and
  gives uneven gaps averaging the nominal interval). Implementations
  must compare due times with a small tolerance: a due time that lands
  exactly on a tick in exact arithmetic (a multiple of 0.015) must fire
  on that tick, not the next, despite floating-point error.
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
  `shells.insert`, so set both from `TimeReload` to keep them equal (the
  Grenade Launcher only; the others keep the animation-derived insert).
- **Whole-clip reload** (SDK): the reload sequence's length − 0.2 s; the
  script's `TimeReload` only when the view model has no reload sequence.

## Quirks

- The same name means different things on different guns: `Damage` is
  per pellet, per blast or per needle, and on the Sniper Rifle and the
  Flame Thrower the game ignores it. `TimeReload` is the per-round time
  only on the Grenade Launcher and a fallback elsewhere. The mapping must
  decide per gun kind, never per field alone.
- `TimeReloadStart` is the same everywhere and is not the start players
  see (the animation's is). Keep the animation-derived starts.
- `TimeIdleEmpty` is never read (the game asks for a misspelled key).

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
| same | `Damage` "7.5" | apply | amount 7 (cut toward zero, as the game) |
| same | `ProjectileType` rocket | apply | nothing applied; a kind-mismatch log |
| same | `WeaponType` secondary (TOML slot 0) | apply | nothing applied; a mismatch log |
| a grenade TOML with shells (insert 0.4, reload 0.4, start 0.25) | `TimeReload` 0.3, `TimeReloadStart` 0.05, `ProjectileType` pipe | apply | insert 0.3, reload 0.3, start 0.25 (unchanged) |
| a shotgun TOML with shells (insert 0.4, reload 0.4, start 0.25) | `TimeReload` 0.3 | apply | insert 0.4, reload 0.4 (unchanged: its insert is the animation's) |
| a clip gun TOML (no `shells`, reload 1.2) | `TimeReload` 0.3 | apply | reload 1.2 (unchanged) |
| a rocket TOML (`tf2:rocket` damage 50, radius 100) | `Damage` 60, `DamageRadius` 120 | apply | damage 60, radius 100 (the rocket's radius is never mapped) |
| a grenade TOML (`tf2:grenade` damage 50, radius 100) | `damage` 60, `DamageRadius` 120, `ProjectileType` pipe | apply | damage 60, radius 120 |
| a sniper TOML (amount 50, `tf2:sniper` base 50) | `Damage` 3, `TimeFireDelay` 1.1 | apply | amount 50, base 50 (not mapped); cycle 1.1 |
| a flame TOML (`tf2:flame` tick 0.1, damage 9, cycle 0.08) | `Damage` 150, `TimeFireDelay` 0.03, no `ProjectileType` | apply | flame damage 9, cycle 0.08 (both unchanged) |
| a needle TOML | `Damage` 12, `ProjectileType` syringe | apply | `tf2:needle` damage 12 |
| a bullet gun TOML (range 56754) | `Range` 1000 | apply | range 1000 |
| any gun | `PunchAngle` 2, `TimeReloadStart` 0.05 | apply | punch_scale and starts are the TOML's |
| any gun | `AmmoPerShot` 2 | apply | one round per shot; a log naming the field |
| a clip gun | `clip_size` 8, `default_clip` 6 | apply | clip 8; a cross-check log |
| no script | – | load | every number is the TOML's; log says "spec fallback" |
| tick timing, `set` (the game's rule) | synthetic `TimeFireDelay` 0.05 held from t = 0 | 10 shots | shot ticks 0, 4, 8, 12, …: every gap 4 ticks (⌈0.05 / 0.015⌉) |
| tick timing, `set` | synthetic `TimeFireDelay` 0.045 held from t = 0 | 3 shots | shot ticks 0, 3, 6 (0.045 is exactly 3 ticks and must not slip to 4) |

With the key present (a heavy test, skipped without it): for each of the
twelve guns, the mapped damage, pellets, clip and spread equal the
committed TOML's (the specs' public numbers), except the cases already
named as differing here: the SMG, Minigun and Syringe Gun interval
(the TOMLs' tick-rounded value; equal under `set` timing) and the
`Range` (the TOMLs' wiki reach). The test lists those exceptions
explicitly so that any new disagreement fails.

## Open questions

1. ~~`Range` versus the wiki's reach~~ Settled by the SDK: `Range` is
   each bullet's trace length; mapped.
2. ~~Sniper Rifle `Damage`~~ Settled: the rifle ignores it (charge rule).
3. ~~Interval and ticks~~ Settled: the next shot is due at this shot's
   tick time + interval, so gaps are rounded up to whole ticks; TF2's
   guns want `timing = "set"`.
4. ~~Flame Thrower `Damage` and `TimeFireDelay`~~ Settled: both ignored;
   13 per flame, a flame every 0.02 s, 14 ammo per second (the flame
   entity's own damage timing is weapons_more.md's subject).
5. **Sniper `default_clip`**: set although the rifle has no clip, and
   different from weapons_more.md's 25 carried. The class script's ammo
   maximum probably governs. Check the Sniper's ammo on spawn on the
   server; the class scripts (`scripts/playerclasses/*.ctx`) are a later
   spec.
6. ~~Rocket Launcher reload~~ Settled: its per-rocket time is the view
   model's reload-loop sequence length, not `TimeReload`; weapons.md's
   0.8 stands.
7. ~~`TimeReloadStart`~~ Settled: the sequence alone; this field only
   when the view model lacks the sequence.
8. ~~`PunchAngle`~~ Settled: a real kick of P ± 1 whole degrees up per
   shot. Open: the shared punch decay's constants are not in TF2's own
   code (they are the shared Source movement's); a `tf2:` punch hook
   needs them.
9. **Flame sounds**: the script's start and wind-down roles name entries
   different from the TOML's `start_sound` and `end_sound`. Check which
   entries exist in `game_sounds_weapons.txt` and which the live game
   plays.
10. **Crit field**: `UseRapidFireCrits` has no target until crits are
    data; the crit spec work should read it from here.
