# Combat Arms: weapon attachments (modifications)

Source basis: the shared weapon-modification manager (record reader and lookups), the client weapon (attachment slots, spread meters, cone inputs, ray length, view-model parts, muzzle effects, fire-sound choice), the client player manager (view recoil, zoom, victim-side stun), the shared animation-rate calculation, the HUD crosshair and weapon panel, the scope overlay, the server weapon and weapon inventory (fitting, clip size, damage), the server damage path (special-function damage), the server projectile (server ray length, AI hearing radius), plus the shipped weapon-modification file, weapons file and character attachments file of a reference client build (US). Numbers are that build's data; the Steam build may differ (see Open questions).
Status: draft

Sections: Summary; Units and conventions; Constants; How attachments are described in the data; Which weapon accepts which; Fitting (when and where); Effects by kind (muzzle, sight, magazine, bullet, mechanical sight); The combined formulas (damage, range, spread, recoil, rates, clip, zoom); Sound; Crosshair, HUD and overlay; View model and world model; Kill feed; Character attachments and gear mods (pointer); Per-tick order; Edge cases; Quirks; Corrections (2026-10-19); Corrections (2026-10-22); Test cases; Open questions.

---

## Summary

- Combat Arms calls weapon attachments **modifications** ("mods"). A weapon has five slots: **muzzle**, **sight**, **magazine**, **bullet** and **mechanical sight**. At most one attachment sits in each slot. There are no grips, stocks, compensators or flash hiders in the shipped data: the code knows a flash-hider and a generic muzzle-attachment type, but no record uses them.
- Each attachment is a record in the weapon-modification file. Each weapon has a record in the same file that **lists every attachment it accepts**. An attachment that is not on the weapon's list can never be fitted to it. The weapon record in the weapons file can also name a **default** attachment per slot (sniper scopes, a few event weapons).
- Attachments are picked in the lobby and fixed for the life of the weapon. The server resolves them when it gives the weapon (spawn, respawn, pickup) and tells the client. Their numbers are then re-summed every time a value is used, so nothing is baked in except the clip already in the magazine.
- What the shipped attachments actually do:
  - Silencers: −10 % damage (some +5 %), a little less recoil, a quiet sound with a shorter radius, no muzzle light for others, and the side kick starts one shot earlier.
  - Sights: zooming (the only way an assault rifle can zoom), the zoomed spread cone, an overlay, a range readout or a health/ammo gauge, and faster spread recovery on the better ones.
  - Magazines: clip size up or down, and reload faster or slower.
  - Bullets: more range, and extra damage against chosen target groups. Some also stun.
  - Mechanical sights: faster spread recovery.
- Two quirks matter for feel. First, the recoil bonus is taken from the **last** fitted attachment in slot order, not summed, so a magazine, bullet or mechanical sight cancels a silencer's recoil bonus. Second, the server's own wall-penetrating ray only gets the **sight's** range bonus.

## Units and conventions

- Distances in world units (as in `movement.md` and `weapon_firing.md`). Angles in degrees in the data; the code applies them in radians. Field of view is horizontal, in degrees.
- Times: animation lengths in ms, zoom time in seconds (`ZoomTime`), stun durations in seconds.
- Percent-style fields are **fractions**: `Instdamage = -0.1` means −10 %, `RangePer = 0.25` means +25 %.
- "Spread meters", "cone", the global spread scales (`move_inc_scale`, `move_dec_scale`, `fire_inc_scale`, `fire_dec_scale`), the view-kick accumulator $J$ and the sub-step $\Delta = 1/80$ s are as defined in `weapon_firing.md`.
- Slot numbers are the data's `Kinds` values: 0 muzzle, 1 sight, 2 magazine, 3 bullet, 4 mechanical sight. Values 5, 6 and 7 are character gear mods (head, face, vest), not weapon parts.
- The muzzle, sight and magazine slots are the **socket slots** (they can put a model on the gun). The bullet and mechanical-sight slots are **stat-only** slots.
- Data fields are named as they appear in the attribute data. Key lookups are case-insensitive (`attribute_data.md`), so `Instdamage` and `InstDamage` are the same key.

## Constants

| Name (ours) | Value | Unit | Meaning |
|---|---|---|---|
| slot_count | 5 | slots | muzzle, sight, magazine, bullet, mechanical sight |
| socket_slots | 3 | slots | slots 0–2 may carry a model and count for animation rates |
| model_variants | 10 | entries | max model variants per attachment (`AttachModel0..9`, also the magazine bonus columns 0..9) |
| zoom_levels_max | 10 | levels | max `ZoomFov#` entries read |
| silenced_sounds_max | 10 | entries | `SilencedFireSnd1..10`, `SilencedFireRadius1..10`; silencer level must be 1..9 to be used |
| accepted_list_max | 255 | entries | capacity of a weapon's accepted list on the server (shipped max is 60) |
| fov_normal | 90 | degrees | unzoomed horizontal FOV (player console value) |
| fov_zoom_out_ref | 20 | degrees | reference FOV used for the zoom-out speed (Quirk Q9) |
| default_probability | 1.0 | — | `SFProbability` when the key is missing |
| default_target | All | — | `SFTargetType` when the key is missing |
| gauge_step | 10 | % | the gauge sight's images are chosen in steps of 10 % |

## How attachments are described in the data

Three kinds of record, linked as follows.

**1. Attachment records** (`Mod0`, `Mod1`, … in the weapon-modification file; the index is the attachment's runtime ID). Fields that do anything, all read as numbers unless noted, default 0 (or empty) when missing:

| Field | Meaning |
|---|---|
| `Name` (string) | Unique name; target of all links |
| `Kinds` (int) | Slot, 0–4 (5–7 gear mods). Missing: invalid, so the attachment can never be fitted |
| `Type` (int) | 0 silencer, 1 scope, 2 dot sight, 3 flash hider, 4 muzzle attachment, 5 magazine. Missing: unknown (6). Shipped: silencers 0, every sight 1 (even the red dots), magazines 5; bullets and mechanical sights have none |
| `Integrated` (int) | Must be non-zero, or the attachment can never be fitted (Edge cases). Every shipped weapon attachment has 1 except one unused "Sniper Silencer" record |
| `Socket` (string) | View-model socket for the part: muzzle parts use the muzzle-flash socket, red dots the breach socket, scopes the scope socket; magazines and stat-only slots have none |
| `AttachModel#`, `AttachSkin#_#`, `AttachRenderStyle#`, `AttachScale` | First-person part model, one variant per index |
| `HHModelSocket`, `AttachHHModel#`, `AttachHHSkin#_#`, `HHRenderStyle#`, `AttachHHModelScale` | Third-person (world) part model, one variant per index |
| `SilencerLevel` (int) | 1–9: which silenced sound/radius pair of the weapon is used (1 in all shipped silencers) |
| `FlashHiderLevel` (int) | 1–9: which flash-hider muzzle effect of the weapon is used (no shipped record) |
| `PVMuzzlefxname`, `HHMuzzlefxname` (string) | First- and third-person muzzle effect that replaces the weapon's (muzzle slot only) |
| `TracerFXName` (string) | Tracer effect (see View model) |
| `ZoomLevel` (int) | Number of zoom steps this sight gives (0: none) |
| `ZoomFov0..9` | FOV of zoom step 1, 2, …, degrees |
| `ZoomSpriteName` (string), `ZoomSpriteScale` (default 0.001), `FocusTex` (string) | Overlay image while zoomed, its scale, and a focus texture used by the post-process |
| `ZoomInSound`, `ZoomOutSound` (string) | Played on zoom in / out |
| `TargetRange` (int) | Range-readout ray length (HUD A9) |
| `SightSystemStyle` (int) | 0 normal, 1 range readout, 2 health/ammo gauge |
| `DSightColor`, `CrossHairA/R/G/B` (int) | Dot-sight crosshair colour (only for `Type` 2; dormant, Quirk Q7) |
| `ShotsPerClipPlus0..9`, `MaxAmountPlus0..9`, `SelectionAmountPlus0..9` (int) | Magazine bonuses, one column per weapon variant |
| `Instdamage` | Damage fraction added to the base damage |
| `InstDamagePer` | Same for the secondary attack (no shipped record) |
| `RangePer` | Ray-length fraction |
| `Fireanirate`, `Reloadanirate`, `Selectanirate` | Added to the fire, reload and draw animation rates |
| `PerturbIncSpeed`, `PerturbDecSpeed`, `ZoomedPerturbIncSpeed`, `ZoomedPerturbDecSpeed` | Spread-rate bonuses (see formulas) |
| `WalkPerturb`, `RunPerturb`, `FirePerturb`, `FireDuckPerturb` | Added to the ammo's movement targets and firing caps |
| `ZoomedMinPerturb`, `ZoomedMaxPerturb`, `DynamicPerturbFactor` | Added to the zoomed cone limits and the dynamic factor |
| `BaseCamRecoilPitch`, `BaseCamRecoilPitchPer`, `MaxCamRecoilPitchPer` | Recoil fractions (last fitted wins, Quirk Q1) |
| `BaseCamRecoilAccelPitch`, `MaxRandomPitch` | Added to the weapon's kick acceleration and side-kick threshold |
| `VectorsPerRoundPer` | Pellet-count fraction (no shipped record) |
| `CameraSwayXSpeed`, `CameraSwayYSpeed`, `CameraSwayXFreq`, `CameraSwayYFreq` | Added to the weapon's scope sway (all 0 in shipped data; sway belongs to `weapon_classes.md`) |
| `SFType#`, `SFTargetType#`, `SFValue#`, `SFDuration#`, `SFProbability#` | Special functions, list `#` = 0, 1, … (see Bullet slot) |
| `GearID`, `MovePenalty`, `Maxstamina`, `DashSpeed` | Gear mods only (Character attachments) |

Fields present in the shipped records but read by nothing in a match: `MinPerturb`, `DuckPerturb`, `MaxCamRecoilWag`, `BaseCamRecoilWagL`, `BaseCamRecoilWagR`, `DynamicLightRadius`, `NightVision`, `TintColor`, `TintTime`, `Priority` (attachment), `PickUpSound`, `RespawnSound`, `PowerupModel…`, `SFKillDtx#`, and the misspelled `SRangePer` (Quirk Q5). `MinPerturbPer`, `PerturbIncSpeedPer` and `PerturbDecSpeedPer` are read, but nothing uses the last two. `MinPerturbPer` only reaches the crosshair (HUD A3).

**2. Per-weapon accepted lists** (`Weapon0`, `Weapon1`, … in the weapon-modification file):

- `ModName0..` (string list): names of the attachments this weapon accepts. `ModModel0..` (int, default 0): the variant index for the same position. The index picks the part model **and** the magazine bonus column.
- A name that matches no attachment is dropped and the list closes up. Its variant index is dropped with it, so the pairs stay matched. Names match case-insensitively, first record wins.
- `MuzzleOPtion`, `SightOption`, `MagazineOption`, `BulletOption`, `MechSightOption` (int, default −1): the lobby's per-slot state. 0 = no part can go here, 1 = free slot, 2 = taken by a built-in part. These drive the inventory screen only; gameplay checks the list.
- **The record is matched to the weapon by position, not by name.** The weapon with runtime ID $i$ (its index in the weapons file) uses the `Weapon<i>` record of the modification file. The `Name` in the modification file is informational. In the reference data some names differ in spelling from the weapons file (event weapons around index 964–1110, one record with no name), and the modification file has one record fewer than the weapons file.

**3. Defaults in the weapons file** (weapon record): `DefaultScopeAttach`, `DefaultMuzzleAttach`, `DefaultMagazineAttach`, `DefaultBulletAttach`, `DefaultMechSightAttach` (string, attachment `Name`), and `DefaultScopeVisible` (int, default 1: the default sight's model is drawn). Also in the weapon record: `SilencedFireSnd1..10`, `SilencedFireRadius1..10`, `PVFlashHiderFXName1..10`, `HHFlashHiderFXName1..10`, `MuzzlePVPosOff`/`RotOff`, `ScopePVPosOff`/`RotOff`, `MuzzleHHPosOff`/`RotOff`, `ScopeHHPosOff`/`RotOff`.

Shipped examples: the M24 and L96A1 have `DefaultScopeAttach="Sniper Scope"`. Eight event weapons set default muzzle, magazine, bullet and mechanical-sight parts. A typical rifle (M4A1) lists 51 attachments, all with variant index 0: silencers A/B/C, red dots A/B/ITECH/HDS-4/Holosight, magazines A/B/C/D (each also in Gold and Red grades), nine bullets and three mechanical sights. The AK-47's red dots use variant 2 (the AK mount model). The M24's magazines use column 4 or 1.

## Which weapon accepts which

- An attachment **can be fitted** to weapon $W$ when all of these hold:
  1. it appears in $W$'s accepted list;
  2. its `Integrated` is non-zero;
  3. its `Kinds` is 0–4;
  4. for socket slots (0–2) only, its `Type` is 0–5 on the server. The client allows 0–6, see Edge cases.
- The slot an attachment occupies is its own `Kinds`, not where it came from.
- Per weapon class in the reference data, roughly: assault rifles, SMGs and pistols accept silencers, red dots, magazines, bullets and mechanical sights. Shotguns accept magazines, bullets and mechanical sights (no muzzle or sight parts). Sniper rifles accept silencers and magazines, take the sight slot with their built-in scope, and accept bullets and mechanical sights. Families come in AR, SMG, SR, MG, Pistol and SG (shotgun) variants that differ only in models, tracer and magazine columns.

## Fitting: when and where

- **Server, when it gives a weapon** (spawn and respawn, mid-round weapon grant, pickup):
  1. It reads the weapon's accepted list and marks every listed, integrated attachment as "available on this weapon".
  2. For each slot in order 0–4 it takes the player's chosen attachment for that slot. If there is none, it takes the weapon's default for that slot, if one is named and found.
  3. It fits each one that passes the checks above. A rejected one leaves the slot empty, silently.
  4. Fitting a magazine refills the clip to the new clip size, **except** on pickup (the picked-up clip is kept).
- The server sends the five slot IDs (−1 for empty) with the weapon. The client empties each slot and fits what it was sent, with the same checks.
- **Lifetime**: the fitted set belongs to that weapon instance. It survives weapon switching and is dropped with the weapon. A picked-up weapon carries the dropper's parts (`inventory.md`, "Drop and death drop"). Changing parts needs the lobby; there is no in-match way except developer console commands.
- **When the numbers apply**: every attachment term below is re-summed from the fitted set **each time the value is used**: every spread sub-step, every shot, every animation start, every zoom step, every HUD frame. Nothing is cached at spawn except the clip count.
- **Who computes what**:

| Effect | Computed by |
|---|---|
| Damage bonus, special-function damage, clip size (authoritative copy), total ammo, server-side ray length, validation of reported hits, AI hearing radius | server |
| Spread meters and cone, view recoil, client ray length (the hit test), zoom, overlay, crosshair and HUD, own fire sound, muzzle effects, view-model parts, stun and knock-back on the victim | client (the victim's client for stun) |
| Fire, reload and draw animation rates | both (client for the view model, server for the third-person animation) |

## Effects by kind

### Muzzle (slot 0)

- Shipped: `Silencer_A/B/C_<class>`, each in a base, Gold and Red grade. All have `Type` 0 and `SilencerLevel` 1.

| Silencer | `Instdamage` | `BaseCamRecoilPitch` | `MaxRandomPitch` |
|---|---|---|---|
| A | −0.10 | −0.10 | −2 |
| B, C | −0.10 | 0 | 0 |
| A Gold | 0 | −0.15 | −2 |
| B Gold | 0 | −0.05 | −2 |
| C Gold | 0 | −0.10 | −2 |
| A Red | +0.05 | −0.20 | −2 |
| B Red | +0.05 | −0.10 | −2 |
| C Red | +0.05 | −0.20 | −2 |

- What a silencer changes:
  - **Damage**: the base damage by `Instdamage`, on the server.
  - **Sound**: the silenced sound and radius.
  - **Muzzle effects**: its own first- and third-person muzzle effect names replace the weapon's, and other players see no muzzle light.
  - **Recoil**: the kick scale (subject to Q1) and the side-kick threshold.
- It does not change range, spread, clip, rates or movement.

### Sight (slot 1)

- Shipped red dots (`DotSight_A/B/ITECH/HDS-4/Holosight_<class>` and grades) and scopes: built-in sniper scopes such as `Sniper Scope`, `Barrett Scope`, `L115 Scope`, `M200 Scope`, plus weapon-specific sights.
- All sights have `Type` 1 (scope), so every sight **zooms**. A weapon with nothing in the sight slot **cannot zoom at all**: pressing zoom does nothing (assault rifles, SMGs, pistols).

| Sight | Zoom FOV | Style | Spread-rate bonuses |
|---|---|---|---|
| DotSight A, Holosight | 50° | 1 range readout (`TargetRange` 10000) | none |
| DotSight B | 50° | 2 gauge | none |
| ITECH | 75° | 0 | `PerturbDecSpeed` +0.1, `ZoomedPerturbDecSpeed` +0.03 |
| HDS-4 | 75° | 0 | +0.1, +0.05 |
| Gold grades (A, B, ITECH, HDS-4, Holosight) | as base | A, ITECH, Holosight: 1 (range); B, HDS-4: 2 (gauge) | +0.1, +0.05 |
| Red grades | as base | as Gold | +0.1, +0.08, plus `BaseCamRecoilPitch` −0.03 and `MaxRandomPitch` −2 |
| Sniper Scope | 20° | 0 | none |
| Barrett Scope | 30°, then 10° | 0 | none |
| L115 / M200 Scope | 15° | 0 | none |

- What a sight changes:
  - **Zoom**: FOV steps (below). While zoomed, the weapon's `Zoomed…` spread values apply (`weapon_firing.md` "Spread"). With a red dot, that is the only way an M4A1 reaches its 2..30 zoomed cone.
  - **Overlay and HUD**: the overlay image, the range readout or gauge, and the zoom sounds.
  - **Spread rates**: the bonuses in the table.
  - **Server ray**: the sight is the only slot whose `RangePer` lengthens the server's own ray (none of the shipped sights has one).
  - **Recoil**: Red grades only.
- Zooming also brings in the zoomed movement rules of `movement.md` and the weapon's `ZoomedMovePenalty`. The sight itself changes neither.

### Magazine (slot 2)

- Shipped `Magazine_A/B/C/D_<class>` and grades. The bonus is read from the column given by the weapon's variant index $c$ for that magazine.

| Magazine (column 0, rifles) | `ShotsPerClipPlus` | `MaxAmountPlus` | `Reloadanirate` |
|---|---|---|---|
| A | +10 | +40 | −0.1 |
| B | +15 | +40 | −0.2 |
| C | −6 | +30 | +0.2 |
| D | −9 | +30 | +0.3 |
| A Gold / B Gold | +14 / +28 | +40 | 0 / −0.1 |
| C Gold / D Gold | 0 / −4 | +30 | +0.3 / +0.45 |
| A Red / B Red | +20 / +40 | +40 | 0 / −0.1 |
| C Red / D Red | 0 / −4 | +30 | +0.4 / +0.6 |

- Other columns hold smaller values: column 4 of A for snipers is +5, column 1 of C for snipers is −1.
- What a magazine changes:
  - **Clip size**, on both sides.
  - **Reload rate**.
  - The client-side ammo ceiling (`MaxAmountPlus`), which the server ignores (Quirk Q3). The client uses that ceiling only to clamp and display.
- `SelectionAmountPlus` is read and never applied.

### Bullet (slot 3)

Stat-only. Its effects come from `RangePer` and from **special functions**: entries $j = 0, 1, …$ with a type, a target group, a value, a duration and a probability.

| Bullet | `RangePer` | Special functions |
|---|---|---|
| FT I / II / III | +0.25 | InstDamage +2 / +8 / +11 vs NZ |
| PVP Damage I / II | +0.25 | InstDamage +13 / +20 vs NZ |
| PVP Damage III | — | InstDamage +0.22 vs Player |
| PVP Range I / II / III (pistol, shotgun only) | +0.25 / +0.50 / +0.70 | InstDamage +0.03 / +0.07 / +0.10 vs All |
| PVP Stun I | — | InstDamage −0.1 vs Player; Stun value 0.20, duration 1.0 s, probability 0.5, vs Player |
| PVP Stun II / III | +2 / +7 | InstDamage 0 vs NZ |
| QR I | +9 | InstDamage 0 vs NZ |
| QR II / III | (misspelled key, not read) | InstDamage 0 vs NZ |

- Target groups, by the victim:

| Group | Victim |
|---|---|
| All | any player or AI |
| Player | any player |
| Npc | any AI |
| Human | uninfected player or human AI |
| Zombie | infected player or zombie AI |
| PH | uninfected player |
| PZ | infected player |
| NH | human AI |
| NZ | zombie AI |

- **Probability**: a value $q > 0$ passes when a uniform draw in $[0, 1]$ is $\le q$. A value $\le 0$ always passes. The default $q$ is 1.0 (always).
- **InstDamage** functions are applied on the server, on the victim side (Damage formula). **Stun** and **Knockback** functions are applied on the victim's client, to its movement. **Crit** adds damage like InstDamage and also plays a critical-hit impact effect (no shipped record).
- Several shipped values look like data-entry mistakes: QR I's range ×10, Stun II's ×3, NZ-only damage of 200–2000 %. They are faithfully what the data says (Open questions).

### Mechanical sight (slot 4)

Stat-only. Shipped `MechSight_Aim_I/II/III_<class>`:

| Mechanical sight | `PerturbDecSpeed` | `ZoomedPerturbDecSpeed` | Special |
|---|---|---|---|
| Aim I | +0.10 | +0.05 | InstDamage −0.03 vs All |
| Aim II | +0.13 | +0.13 | — |
| Aim III | +0.16 | +0.20 | — |

Not an iron-sight zoom: it changes recovery rates only, and Aim I costs 3 % damage.

## The combined formulas

$S$ is the set of fitted attachments (up to 5). Each sum runs over $S$ unless it says "socket slots" (slots 0–2) or names a slot. A missing field is 0.

### Damage (server)

Steps of `weapon_firing.md` "Damage per ray", with the attachment terms:

1. Base, with $c_{conv}$ the gunsmith conversion bonus (0 for stock weapons):

```math
D = \texttt{InstDamage}_{ammo}\cdot\Big(1 + \sum_{m\in S}\texttt{Instdamage}_m + c_{conv}\Big)
```

   Secondary attacks use `Alt_InstDamage` and `InstDamagePer`.
2. Random roll, behind bonus, range band and penetration: unchanged. The band uses the reported distance; past the last band edge the multiplier is 1 (`weapon_firing.md` Q8). A longer ray can therefore land full-damage hits beyond `Effectrange2`.
3. Victim side, after the buff, room and set-item hooks and **before armour**, for bullet damage fired by a player:

```math
D \leftarrow D\cdot\Big(1 + \sum_{m\in S}\ \sum_{j:\ \text{type}_j\in\{\text{InstDamage},\text{Crit}\},\ \text{target and probability pass}} \texttt{SFValue}_{m,j}\Big)
```

4. Armour, hit zone and death: unchanged.

### Ray length

- Client hit test (decides player hits), with the product truncated toward zero:

```math
L_{client} = \texttt{Range} + \Big\lfloor \texttt{Range}\cdot\sum_{m\in S}\texttt{RangePer}_m \Big\rfloor
```

- Server check of a reported hit: the same $L$, from the same fitted set.
- Server's own ray (through walls, collateral hits): only the sight slot counts:

```math
L_{server} = \texttt{Range}\cdot(1 + \texttt{RangePer}_{sight})
```

  With a bullet's range bonus, a target between `Range` and $L_{client}$ can be hit directly but never through a wall.

### Spread (client)

The meters and cone of `weapon_firing.md`, with these attachment sums:

```math
\Sigma_{inc}=\sum\texttt{PerturbIncSpeed}_m,\quad \Sigma_{dec}=\sum\texttt{PerturbDecSpeed}_m,\quad Z_{inc}=\sum\texttt{ZoomedPerturbIncSpeed}_m,\quad Z_{dec}=\sum\texttt{ZoomedPerturbDecSpeed}_m
```

**Weapon rates.** $I$ and $R$ are the weapon's own increase and decrease rates for the state, before attachments:
- zoomed and the weapon's zoomed rate is non-zero: `ZoomedPerturbIncSpeed` / `ZoomedPerturbDecSpeed`;
- else in burst mode with a non-zero burst rate: `TripleShotPerturbIncSpeed` / `TripleShotPerturbDecSpeed`;
- else `PerturbIncSpeed` / `PerturbDecSpeed`.

The "zoomed" terms below apply only in the first case.

**Movement target $S_{move}$.**
- Walking: `WalkPerturb` + $\sum$`WalkPerturb`$_m$.
- Running, and on a ladder: `RunPerturb` + $\sum$`RunPerturb`$_m$.
- Jumping: `JumpPerturb` (no attachment term).
- If zoomed and the ammo has a non-zero `Zoomed…` value for the state, that value replaces the whole sum, attachments included.

**Movement meter, rising.** The bonus multiplies the rate, and when zoomed a flat amount is also added every sub-step:

```math
M \leftarrow M + \min(\Delta\cdot\text{move\_inc\_scale},\ A-M)\cdot I\cdot(1+\Sigma_{inc}) \;+\; [\text{zoomed}]\cdot Z_{inc}
```

  Then the clamps of `weapon_firing.md`.

**Movement meter, falling on the ground.** The bonuses are added to the rate:

```math
M \leftarrow M - \min(\Delta\cdot\text{move\_dec\_scale},\ M-A)\cdot\big(R + [\text{zoomed}]\,Z_{dec} + \Sigma_{dec}\big)
```

  While airborne from a jump it rises instead, with rate $I + [\text{zoomed}]\,Z_{inc} + \Sigma_{inc}$.

**Firing cap $C$.**
- Standing still: `FirePerturb` + $\sum$`FirePerturb`$_m$.
- Crouched still: `FireDuckPerturb` + $\sum$`FireDuckPerturb`$_m$.
- Moving, with or without crouch: the ammo value alone.
- Zoomed with a non-zero ammo `Zoomed…` value: that value alone.

**Firing meter, per shot.** Note the zoomed **decrease** sum here (Quirk Q4):

```math
F \leftarrow \min\!\Big(C,\ F + \Delta\cdot\text{fire\_inc\_scale}\cdot\big(I + [\text{zoomed}]\,Z_{dec} + \Sigma_{inc}\big)\Big)
```

**Firing meter, recovery** (after `DecDelay`):

```math
F \leftarrow \max\!\Big(0,\ F - \Delta\cdot\text{fire\_dec\_scale}\cdot\big(R + [\text{zoomed}]\,Z_{dec} + \Sigma_{dec}\big)\Big)
```

**Cone** (at the shot):
- Dynamic factor: $g$ = `DynamicPerturbFactor` + $\sum$`DynamicPerturbFactor`$_m$.
- Zoomed, when the weapon's zoomed limit is non-zero: $n_{min}$ = `ZoomedMinPerturb` + $\sum$`ZoomedMinPerturb`$_m$, and likewise for $n_{max}$, each truncated to an integer.
- The rest is `weapon_firing.md` unchanged. The attachment `MinPerturb` field is **not** applied to the ray.
- The crosshair computes its own min and max (`hud_crosshair.md` A3). It differs from the ray in two ways: its zoomed minimum adds the doubled sum of `MinPerturbPer` instead of `ZoomedMinPerturb`, and its zoomed maximum uses the doubled sum $S \leftarrow 2S + v$. Every term involved is 0 in shipped data.

### Recoil (client, first person)

`weapon_firing.md` "Recoil", with:

- **Kick scale and cap.** Walk the fitted attachments in slot order 0→4. For **each** fitted one, overwrite $p_b$ = `BaseCamRecoilPitch` + `BaseCamRecoilPitchPer` and $p_J$ = `MaxCamRecoilPitchPer`. The values left after the walk are used (Quirk Q1; both 0 if nothing is fitted). In burst mode the burst values replace the weapon's.

```math
b = \operatorname{rad}\big(\texttt{BaseCamRecoilPitch}\cdot(1+p_b)\big),\qquad J_{max} = \operatorname{rad}\big(\texttt{MaxCamRecoilPitch}\cdot(1+p_J)\big)
```

- **Acceleration**: $a$ = `BaseCamRecoilAccelPitch` + $\sum$`BaseCamRecoilAccelPitch`$_m$ (all 0 in shipped data).
- **Side-kick threshold.** The side kick happens when the inequality below holds. A silencer's −2 makes the threshold negative, so the side kick starts on the **first** shot.

```math
|J| > \texttt{MaxRandomPitch} + \sum_{m\in S}\texttt{MaxRandomPitch}_m
```

- **Side-kick sizes** (`MaxCamRecoilWag`, `BaseCamRecoilWagR/L`, `MaxRandomWag`): weapon values only. The attachment fields of the same names are ignored.
- **Return**: the vertical return's correction uses $J_{max}$ scaled by the **muzzle** attachment's `MaxCamRecoilPitchPer` only (0 in shipped data).

### Animation rates (client and server)

Sums over the **socket slots** only. Bullet and mechanical-sight rate fields are ignored.

```math
r_{fire} = \text{mutation}\Big(1 + \sum_{socket}\texttt{Fireanirate}_m + \texttt{FireAniRate}_{weapon} + f_{set} + f_{conv}\Big)
```

```math
r_{reload} = 1 + \sum_{socket}\texttt{Reloadanirate}_m + r_{model} + r_{set},\qquad r_{draw} = 1 + \sum_{socket}\texttt{Selectanirate}_m + s_{model} + s_{set}
```

- These are the "mod bonuses" in `weapon_firing.md` (fire cycle, reload) and `inventory.md` (draw).
- The cap of `weapon_firing.md` Q6 applies (a rate above 2.0 plays at 1.0).
- Durations are length ÷ rate. M4A1 reload 2300 ms: with magazine A (−0.1) 2555.6 ms, B (−0.2) 2875 ms, C (+0.2) 1916.7 ms, D (+0.3) 1769.2 ms, D Red (+0.6) 1437.5 ms.
- No shipped attachment changes the fire or draw rate.

### Clip and ammo

$c$ is the weapon's variant index for the fitted magazine (from its accepted list; first match if listed twice):

```math
\text{clip}_{max} = \texttt{ShotsPerClip} + \begin{cases}\texttt{ShotsPerClipPlus}_c & 0\le c\le 9\\ 0 & \text{otherwise}\end{cases}
```

- Used by both sides: reload target $\min(\text{total}, \text{clip}_{max})$, the "full clip" test, the server's clip refill when the magazine is fitted, the HUD and the gauge sight.
- **Total ammo** (server): unchanged by attachments, still the ammo's `MaxAmount` / `SelectionAmount` rules (`inventory.md`). The client's ceiling is `MaxAmount` + `MaxAmountPlus`$_c$. Because the server never sends more than its own ceiling, the larger client ceiling has no visible effect.
- So an extended magazine gives more rounds per reload, **not** more rounds in total. M4A1 + magazine A: 40 in the clip, 120 total (reserve 80).

### Zoom (client)

- The sight in slot 1 gives `ZoomLevel` steps. The zoom key moves the step $z$ to $z+1$, and past `ZoomLevel` back to 0 (zoomed out). It is refused while reloading, and for weapons with "reload after fire" while a shot is cycling and not already zoomed.
- The target FOV of step $z \ge 1$ is `ZoomFov`$_{z-1}$ (degrees). Zoom FOV entries beyond `ZoomLevel` are never used (most sights carry a spare second entry of 5°).
- The FOV moves toward the target by $v\cdot\Delta t$ per frame and snaps when it passes it:

```math
v_{in} = \frac{\text{fov\_normal} - \text{fov}_{target}}{\texttt{ZoomTime}},\qquad v_{out} = \frac{\text{fov\_normal} - 20^\circ}{\texttt{ZoomTime}}
```

  M4A1 + red dot A, `ZoomTime` 0.15 s: in 90°→50° at 266.7°/s, 0.15 s. Out at 466.7°/s, 0.0857 s (Q9).
- The weapon's model is **hidden** while zoomed, and the overlay is drawn: the sight's `ZoomSpriteName` at `ZoomSpriteScale`.
- The sight's zoom-in and zoom-out sounds play on each change.
- When the step reaches 0 the FOV goes back to normal. The rest of the scope behaviour (sensitivity, sway, quick-scope rules) belongs to `weapon_classes.md`.
- Spectating in first person uses the spectated player's sight.
- A sight of `Type` 2 (dot sight) would instead toggle "dot-sight mode" (the crosshair colour of HUD A6) without zooming. No shipped sight has that type (Q7).

## Sound

- **Own shots** (the shooter's client): if the muzzle attachment has `SilencerLevel` $\ell \in [1, 9]$, the shot plays the weapon's `SilencedFireSnd`$_\ell$ with audible radius `SilencedFireRadius`$_\ell$ instead of `FireSnd` / `FireSndRadius`. Shipped: M4A1 2000 → 1400, AK-47 2000 → 1600, MP5A4 1800 → 1200, Colt 1911 1500 → 1000, M24 and L96A1 3000 → 1500.
- **Other players' shots** (observers): treated as silenced when the shooter's muzzle attachment has `Type` silencer (0) or muzzle attachment (4). The sound is then picked by its silencer level as above. A silenced type with level 0 would play **nothing** (no shipped record).
- **AI hearing** (server stimulus radius): uses the weapon's silenced radius entry at position $\ell + 1$ instead of $\ell$ (Quirk Q6). Every shipped weapon has equal first and second entries, so the result is the same.
- The muzzle light of a silenced shot is not shown to others.
- Zoom sounds: the sight's own (`ZoomInSound` / `ZoomOutSound`, often a sniper "zoom" click).

## Crosshair, HUD and overlay

All per `hud_crosshair.md`; this lists what an attachment feeds in.

- While a sight's overlay is up, no crosshair is drawn. Every shipped sight has an overlay, so the HUD's "zoomed with no overlay" case (A3) never happens with stock data.
- **Range readout** (A9): a sight with `SightSystemStyle` 1 and `TargetRange` > 0, zoomed or not. Its colour test uses `Range` + trunc(`Range`·Σ`RangePer`) over all fitted attachments (the client ray length).
- **Gauge** (style 2: red dot B, HDS-4 Gold and Red grades, some built-in sights). Every HUD frame while that sight is fitted, zoomed or not, two images are shown beside the centre: health on the left, clip on the right. With $p$ the integer percentage ($\lfloor 100\cdot HP/HP_{max}\rfloor$, or $\lfloor 100\cdot\text{clip}/\text{clip}_{max}\rfloor$), the image number is

```math
k = (p+5) - \big((p+5) \bmod 10\big)
```

  so 0, 10, …, 100, rounding to the nearest 10 with 5 rounding up.
- **Crosshair min/max** (A3): see Spread. **Crosshair colour** (A6 step 2): dormant (Q7).
- **Weapon panel**: clip numbers use clip$_{max}$ with the magazine.

## View model and world model

- **First person**: for each **socket slot** whose attachment has a `Socket`, the client creates the part model `AttachModel`$_c$ with its skins, render style and `AttachScale`, where $c$ is the weapon's variant index for that attachment. It is attached at that socket of the gun model.
  - Muzzle parts are offset by the weapon's `MuzzlePVPosOff` / `MuzzlePVRotOff`, sight parts by `ScopePVPosOff` / `ScopePVRotOff`. The offsets are in the socket's frame (right, up, forward), the rotation offset as added yaw, pitch and roll.
  - Parts follow the gun every frame, and hide and show with it. A default sight is hidden when the weapon's `DefaultScopeVisible` is 0.
  - Magazines and stat-only slots add no model.
  - $c$ outside 0–9, or an empty model name, gives no part, but the effects still apply.
- **Third person / world**: the same with `AttachHHModel`$_c$ at `HHModelSocket`, offset by the weapon's `MuzzleHHPosOff` / `ScopeHHPosOff` values.
- **Muzzle flash**:
  - If the muzzle attachment has a flash-hider level $h \in [1, 9]$, the weapon's `PVFlashHiderFXName`$_h$ / `HHFlashHiderFXName`$_h$ is used (unused in shipped data).
  - Otherwise any muzzle attachment's own `PVMuzzlefxname` / `HHMuzzlefxname` replaces the weapon's. Even an empty name replaces it, which gives no flash.
  - Shipped silencers name ordinary effects, so a silenced gun still shows a flash in first person.
- **Tracer**: walk the fitted attachments in slot order. The **last** one fitted decides. If it names a tracer effect, that effect (with its own frequency) replaces the ammo's; if it names none, the ammo's tracer is used.

## Kill feed

Attachments change nothing in the kill feed: it shows the weapon's own kill icon and the head-shot mark. The bullets' `SFKillDtx#` field (icon names "AR" / "FR") is read by nothing in the reference client.

## Character attachments and gear mods (pointer)

- The **character attachments** file holds backpack and goggle records, not weapon parts:
  - fields: `Name`, `Type`, `Socket`, `Model0/1` (male/female), skins, `DeleteOnDeath`, `DetachWhenShot`;
  - gameplay fields: `MovePenalty`, `Maxstamina` and `GearID`.
  - Their speed and stamina arithmetic is in `movement.md`, and `GearID` adds an armour piece (`weapon_firing.md` "Armour").
- **Gear mods** are attachment records with `Kinds` 5 (head), 6 (face) and 7 (vest), fitted to head, face and vest items through the `Hair#`, `Goggle#` and `Model#` lists of the modification file:
  - per-slot `GearMod1_Option..GearMod3_Option`;
  - effects: `GearID` (protection), `MovePenalty` (summed into $P_{gear}$ of `movement.md`), `Maxstamina`, `DashSpeed`.
  - They never touch a weapon. They are a later addition and not needed for the slice.

## Per-tick order

Server, when a weapon is given:
1. Look up the weapon's accepted list (by weapon index). If there is no record, see Quirk Q2.
2. Mark the listed, integrated attachments as available.
3. Fill each slot from the player's choice, else the weapon default; fit each that passes the checks.
4. If a magazine was fitted and this is not a pickup, set the clip to clip$_{max}$.
5. Send the weapon with its five slot IDs.

Client, one frame (attachment terms only; the full order is in `weapon_firing.md`):
1. Recoil return: the muzzle slot's `MaxCamRecoilPitchPer` scales $J_{max}$ in the correction term.
2. Zoom key, if pressed: step the zoom level (sight slot); start the FOV move, overlay and sound.
3. Animation state machine: fire, reload and draw rates include the socket-slot sums.
4. On a shot:
   - cone limits and dynamic factor with attachment sums;
   - ray length $L_{client}$;
   - send the shot with the muzzle attachment's ID;
   - play the plain or silenced sound;
   - muzzle effect and tracer.
5. Spread sub-steps: movement then firing meter, with attachment sums.
6. View kick (if fired): kick scale from the last fitted attachment; side kick if $|J|$ exceeds the summed threshold.
7. FOV update toward the zoom target.
8. HUD: crosshair (not while the overlay is up), range readout or gauge from the sight slot.

Server, per received shot:
1. Check the client's pellet count against the weapon's.
2. Check the reported hit against $L_{client}$, computed from the fitted set.
3. Damage: base with $\sum$`Instdamage`; roll; behind bonus; range band; set-item hook; special-function factor; armour; zone.
4. Own ray with $L_{server}$ (sight bonus only).
5. AI hearing stimulus with the silenced radius if a silencer is fitted.

Victim's client, on being hit by a player's bullet from another team:
- Add the shooter's Stun and Knockback functions that pass their target and probability tests to the ammo's stun time and penalty, then apply them (`movement.md`). Functions with target All never pass here (Quirk Q8).

## Edge cases

- **Two attachments for one slot.** The data holds one ID per slot. If the player's loadout or the default list puts two attachments of the same `Kinds` in play, they are fitted in slot order and the **later one replaces the earlier**. An attachment always lands in its own `Kinds` slot, whatever slot it was offered for.
- **Attachment not in the weapon's list.** Rejected on both server and client. The slot stays empty and every number is stock. This also happens to a weapon default that the weapon does not list.
- **Attachment with `Integrated` 0 or missing.** Never "available", so never fitted, even if listed.
- **Unknown name in an accepted list**: dropped, and the remaining pairs keep their variant indices.
- **Same attachment listed twice** (the M24 lists "Sniper Scope" three times): the first entry's variant index is used.
- **Variant index outside 0–9 or missing**: missing means 0. Outside the range there is no part model and no magazine bonus; all other effects still apply.
- **Socket-slot attachment with no `Type`.** The server rejects it; the client accepts it. It would then run with client-only effects (spread, recoil, sound) and no server effects. No shipped record. Ours: reject on both.
- **Gear mod (`Kinds` 5–7) in a weapon list**: rejected for weapons.
- **Picked-up weapon.** It keeps the dropper's fitted set, clip and total. Its accepted list is re-read from the weapon's record. Fitting the magazine does **not** refill the clip.
- **Magazine that shrinks the clip below the current count.** Only reachable at fitting time, when the clip is refilled to the new size anyway. A later reload is refused when clip = clip$_{max}$.
- **Clip size larger than the total**: the clip holds what there is, $\min(\text{total}, \text{clip}_{max})$.
- **Zoom level above the number of FOV entries**: the reference's target FOV becomes undefined. Ours: stay at the previous FOV. No shipped record.
- **Rates at or below 0** (several negative bonuses): not reachable with shipped data. The lowest is 0.8. Ours: clamp to a small positive rate and log.
- **Spectating in first person**: the HUD, sounds, muzzle effects and zoom use the spectated player's fitted set, as sent by the server.

## Quirks

- **Q1, recoil bonus is last-wins.** The kick-scale fraction is overwritten by every fitted attachment in slot order, so the highest fitted slot decides. A magazine (slot 2), bullet (3) or mechanical sight (4) — all with 0 — cancels the silencer's or red dot's recoil reduction. A Red red dot (slot 1) overrides the silencer's −10 or −20 % with its own −3 %. Players with a full set of parts therefore get no recoil reduction at all. Keep (it is what the game does), behind a fidelity flag; the "fixed" reading would sum the fractions.
- **Q2, no modification record, no weapon.** When the server gives a weapon that has no record in the modification file, the weapon is silently not given, and a pickup of it fails. The reference modification file is one record shorter than its weapons file. Do not copy: treat a missing record as an empty accepted list and log it.
- **Q3, extended magazines don't add reserve.** The client's ceiling grows by `MaxAmountPlus`, the server's does not, and the server's total wins. Keep: the bigger magazine only redistributes the same total.
- **Q4, zoomed shot growth uses the zoomed decrease bonus.** While zoomed, the firing meter grows with the attachments' `ZoomedPerturbDecSpeed` sum, not their `ZoomedPerturbIncSpeed` sum. So better sights make zoomed spread grow slightly *faster* per shot. Keep.
- **Q5, misspelled range keys.** Several bullets (QR II and III, PVP Damage I for pistols) carry `SRangePer`, which nothing reads, so they give no range. Keep as data.
- **Q6, AI hearing off by one.** The server picks silenced radius entry $\ell + 1$ while clients play entry $\ell$. No effect with shipped data. Keep or fix; it is invisible.
- **Q7, dot-sight mode is dormant.** All red dots are typed as scopes, so they zoom with an overlay. The crosshair-colour "dot-sight mode" never engages. Keep the data as is.
- **Q8, client stun ignores "All".** On the victim's client a Stun or Knockback function with target All (or Npc, NH, NZ) never applies; Player always does. On the server's AI path, All applies. Keep.
- **Q9, asymmetric zoom speed.** Zooming out always runs at $(90° - 20°)/\texttt{ZoomTime}$, whatever sight is fitted. So a 50° red dot zooms out in 57 % of its zoom-in time, and a 15° scope zooms out in 107 %. Keep.
- **Q10, assault rifles cannot zoom without a sight.** The weapon's own zoomed spread values exist, but they are unreachable until a sight is fitted. Keep.
- **Q11, a silencer moves the side kick forward.** Its `MaxRandomPitch` of −2 makes the side-kick threshold negative. The first shot already kicks sideways at full size. Keep.
- **Q12, server ray ignores bullet range.** See Ray length. A long-range bullet extends direct hits only, never wall-bangs. Keep.
- **Q13, link by position.** Modification records are matched to weapons by index. Our loader should do the same when it reads the install's files, and log every record whose `Name` differs from the weapon's. Our own TOML data may link by name.

## Corrections (2026-10-19)

From a second pass over the reference's client and server shot handling and
view kick, compared with this spec and our implementation. Test cases 38–40.

1. **`VectorsPerRoundPer`: the client and the server read different slots.**
   - Reference: the client sends $n + \lfloor n\cdot v_{magazine}\rfloor$ rays,
     with $v$ the **magazine** part's `VectorsPerRoundPer`. The server
     expects $n + \lfloor n\cdot v_{muzzle}\rfloor$ from the **muzzle** part's.
     When the counts differ the server ignores the whole shot: no damage,
     and no server-side ammo is spent.
   - Ours: the field is read and never applied
     (`games/combat_arms/src/game/attachments.rs:302-303`). The spec only said "pellet-count
     fraction (no shipped record)".
   - Why it matters: nothing with stock data. A data mod that gives a
     magazine a pellet bonus makes that weapon deal no damage at all.
2. **The burst return uses the burst cap with the muzzle's scale.**
   - Reference: in burst mode the return of `weapon_firing.md` uses
     rad(`TripleShotMaxCamRecoilPitch` × (1 + the muzzle part's
     `MaxCamRecoilPitchPer`)) whenever that field is non-zero. See
     `weapon_firing.md` Corrections 2.
   - Ours: the "Return" bullet of the Recoil formulas named only the
     normal cap; `recover` uses the normal one
     (`games/combat_arms/src/game/weapons.rs:839`).
   - Why it matters: same as there; no shipped part has a non-zero
     `MaxCamRecoilPitchPer`, so only the burst cap itself changes.
3. **Recoil terms follow the weapon in hand, not the weapon that kicked.**
   - Reference: the view kick state is the player's (`weapon_firing.md`
     Corrections 1). After a switch, the return and the next kick read
     the attachment terms of the weapon now in hand.
   - Ours: the terms sit on each weapon's own recoil state
     (`weapons.rs:748-759`).
   - Why it matters: switching from a weapon with a muzzle part to a bare
     one changes the cap the leftover climb returns against.

## Corrections (2026-10-22)

From the inventory, attachments, grenades and gear comparison (the server's
weapon pickup). Entry: the reference behaviour, what we do now (file:line),
and a testable expectation.

1. **The fitted set decides whether a primary pickup is a swap at all**
   ("Fitting: when and where", Lifetime; `inventory.md` Corrections
   (2026-10-22) 1).
   - Reference: when a primary item is used, its empty slots are first
     filled with its weapon's default parts; then its five slot ids and
     its sight style are compared with the class's last-selected weapon's
     fitted set. If they are equal, the carried weapon (and its parts) is
     kept and only its total and clip are replaced by the item's: the item's
     parts are never fitted and the weapon is not swapped, even when the
     item is a different weapon. Only a different set (or sight style)
     swaps, and then the picked weapon keeps the dropper's parts as this
     spec says, without a clip refill from the magazine.
   - Ours: every primary pickup swaps, so the item's parts always arrive
     with it (`games/combat_arms/src/game/inventory.rs:1220-1263`).
   - Expect: an M4A1 with Silencer_A_AR and DotSight_A_AR carried; an item
     M4A1 with the same two parts, total 50, clip 20 → the carried weapon
     stays (same entity, same parts), total 50, clip 20. The same item with
     DotSight_B_AR instead → swap: the item's M4A1 (with the B sight) is
     drawn with clip 20 and the carried one dropped.

## Corrections (2026-10-30): placing part models

From the client weapon's part-model creation and per-frame update, the server's held-weapon part attachments, the engine's rotation and Euler-angle helpers and the install's `RS\TRANSPARENT.LTB`. Answers the plan's "Attachment models" questions; refines "View model and world model".

1. **First-person frame (no turn).** Every client frame, for each fitted part with a model: take the gun model's socket transform (the socket's node in its animated pose times the socket's own stored position, rotation and scale, as a world transform), then
   - position = socket position + o_x · x̂ + o_y · ŷ + o_z · ẑ, where x̂, ŷ, ẑ are the socket's **own local x, y, z axes** (the engine calls them right, up, forward) and o the weapon's `MuzzlePVPosOff` or `ScopePVPosOff` in u;
   - orientation: the socket rotation split into yaw (about y, applied first), pitch (about x) and roll (about z) in radians; the weapon's `…PVRotOff` (x yaw, y pitch, z roll, radians) is **added to those angles** and the rotation rebuilt from them in the same order. With zero rotation offsets (every stock first-person value) the part's axes are exactly the socket's axes: **no half turn or any other fixed turn is applied**;
   - scale: the part's `AttachScale` when created; but on any frame where the socket transform's scale is not exactly 1, that scale replaces it.
   So a part model that points along its own −x sits in the gun if the socket's +x points out of the barrel: the game would draw it that way. If the stock rifle silencers appear inside the gun in ours, the difference is in our reading of the socket (its stored rotation, the quaternion's component order or the left-to-right-handed conversion of `models_ltb.md`), not a rule of the game; our "turn ahead" search has no counterpart (Open question 8).
2. **Third-person frame.** The server attaches the part model (`AttachHHModel`c, `AttachHHSkin`, the part's third-person render styles, scale `AttachHHModelScale`) to the held weapon object at `HHModelSocket` as an engine attachment with offset position o (socket-local x, y, z, u) and offset rotation built from `…HHRotOff` as (pitch = its y, yaw = its x, roll = its z, radians), **composed after** the socket's rotation (unlike first person, which adds the angles). Shown in third person only (hidden in first person). The Luger P08's `ScopeHHRotOff` (0, −3.14, −1.65) is therefore pitch −3.14 and roll −1.65 rad, yaw 0, in the socket's frame.
3. **Muzzle flash with a silencer.** It does not move to the part: the first-person flash group and its light stay at the weapon's flash point F (`view_models.md` 4, from `Pos` + `MuzzlePos`, not even from the gun's `Flash` socket), whatever part is fitted; a part only changes which group plays (above). Part models carry no socket the game uses.
4. **A sight's lens** (`RS\TRANSPARENT.LTB`, style index 1 of the red-dot models): texture × lit colour, **alpha-tested** (drawn where the texture's alpha > 160/255, discarded elsewhere), not blended, **two-sided** (no culling), depth written. So the `clouds.dtx` lens shows as hard-edged lit texels, not a see-through tint (`models_ltb.md` "Render styles and sprite skins").
5. **Part models' own animations and sockets.** None: a part model is created with its file, skins and styles and left on its default (first) animation; nothing plays, reads or attaches to its sockets.

## Corrections (2026-10-31): default parts outside the accepted list

From the server's equipment building and weapon giving, its weapon part fitting, the client's view-model part visibility and the lobby's inventory preview; data from Steam build 26236938 (weapons and modification files).

1. **A default part the weapon does not accept is not fitted** ("Fitting: when and where" 2-3 confirmed for defaults). When the player has no part of his own for a slot, the server takes the weapon's default for it (`DefaultScopeAttach` etc., looked up by attachment `Name`; a name that matches no attachment gives none). The part is then fitted by the same check as a chosen one: it must be on the weapon's accepted list (with `Integrated` non-zero), and its `Kinds` and `Type` must be valid. A default that is not on the list fails that check and the slot stays **empty**: no part model, no zoom (a sight's `ZoomLevel` comes only from a fitted sight), no stat changes. The server sends −1 for the slot and the client fits nothing. The game never fits a default "anyway".
   - `DefaultScopeVisible` only matters when the default sight actually got fitted: the client then hides that sight's model when it is 0.
   - Only the lobby's inventory preview of the character differs: it shows the weapon's default muzzle part and (if `DefaultScopeVisible` is not 0) default sight whenever the player has none, without checking the list. In a match the weapon has no such part.
   - Data: 909 weapon records name a default sight; for 49 of them it is not on their accepted list (48 that name an existing attachment, plus `Advanced starshing M4A1`, whose default names no attachment at all): `INSAS_Scope` 14 (the double-barrel AR-15 variants), `MP5_SD_DOT_C` 13, `P90 sight` 11 (the P90 SE Kraken records), `Sniper Scope` 8 (`M14 Cyanide Echo` Lv.1-4 and their limited copies), `DotSight_HDS-4_SMG` 1 (`loaded R870 MCS MOD`), `DotSight_ITECH_AR` 1 (`M417 Green Chrome`). These weapons have no sight in a match unless the player fits one from their list; the M14 Cyanide Echo cannot zoom without one.
   - Ours: whatever we do with such defaults should leave the slot empty.
   - Expect: test cases 44-45.

## Test cases

M4A1 data as in `weapon_firing.md` unless stated. "Roll 1.0" means the damage random roll returns 1. Victim: male skeleton, 100 HP, no armour, torso hit, unless stated. Distances are fire position to impact.

1. **Fitting, accepted.** M4A1 + `Silencer_A_AR` → muzzle slot holds it, damage 31.5 at d = 100.
2. **Fitting, not listed.** M4A1 + `Silencer_A_SR` (not in the M4A1 list) → muzzle slot empty. Damage 35, sound radius 2000, first kick 1.300°.
3. **Fitting, not integrated.** Any weapon + "Sniper Silencer" → not fitted.
4. **Default fill.** M24 with no chosen sight → sight slot holds "Sniper Scope", and the zoom key gives 20°. An event weapon with four named defaults and no player choices → all four slots filled with the defaults. Same weapon with a player-chosen muzzle part that it lists → the player's part in the muzzle slot, defaults in the others.
5. **Same slot twice.** Offer `Silencer_A_AR`, then `Silencer_B_AR` (both slot 0) → `Silencer_B_AR` fitted. First kick 1.300° (B has no recoil field), side kick from shot 2.
6. **Silencer damage by range.** M4A1 + `Silencer_A_AR`, roll 1.0: d = 100 → 31.5; d = 2800 → 22.05; d = 3200 → 11.025. Head at d = 100 → 189.
7. **Hits to kill.** M4A1 + `Silencer_A_AR`, unarmoured torso, d ≤ 2500, roll 1.0 → 4 hits (HP 68.5, 37, 5.5, dead). With `Silencer_A_AR_Red` (+5 %) → 36.75 per hit, 3 hits.
8. **Target-group damage.** M4A1 + `Bullet_PVP_Damage_III_AR`, roll 1.0: vs a player → 42.7; vs a human AI → 35.
9. **Stacked damage.** M4A1 + `Silencer_A_AR` + `Bullet_PVP_Damage_III_AR` vs a player, roll 1.0 → 35 × 0.9 × 1.22 = 38.43.
10. **Mechanical sight cost.** M4A1 + `MechSight_Aim_I_AR`, roll 1.0 → 33.95 vs a player and vs an AI.
11. **NZ-only bonus.** M4A1 + `Bullet_FT_I_AR`, roll 1.0: vs a zombie AI → 105; vs a player → 35.
12. **Bullet range, client.** M4A1 + `Bullet_FT_I_AR` → client ray 4375. Hit at d = 3600, roll 1.0 → 35 (past the last band). Target at d = 4400 → no hit.
13. **Bullet range, server ray.** Same weapon, a 6-unit wood panel, then a torso at d = 3800 → no damage (the server's own ray is 3500). With no panel, the client-reported hit at d = 3800 is accepted and deals 35.
14. **Range ×10 and the dead key.** M4A1 + `Bullet_QR_I_AR` → client ray 35000. M4A1 + `Bullet_QR_II_AR` → ray 3500.
15. **Extended magazine.** M4A1 + `Magazine_A_AR` → clip 40, total 120 at spawn. Fire 28 → 12/92. Reload → 40/92.
16. **Short magazines.** M4A1 + `Magazine_C_AR` → clip 24. + `Magazine_D_AR` → 21. + `Magazine_B_AR_Red` → 70 (total still 120).
17. **Magazine column.** M24 (clip 7) + `Magazine_A_SR` (column 4) → 12. M24 + `Magazine_C_SR` (column 1) → 6. L96A1 (clip 5) + `Magazine_C_SR` (column 0) → 4.
18. **Reload time.** M4A1 reload 2300 ms → with `Magazine_A_AR` 2555.6 ms; `Magazine_B_AR` 2875 ms; `Magazine_C_AR` 1916.7 ms; `Magazine_D_AR` 1769.2 ms; `Magazine_D_AR_Red` 1437.5 ms.
19. **Rate fields in stat-only slots are ignored.** A test attachment in the bullet slot with `Reloadanirate` +0.5 → reload still 2300 ms.
20. **Silencer recoil.** M4A1 + `Silencer_A_AR`, no recovery between shots. View rise per shot (°): 1.170, 1.150, 1.131, 1.111, 1.092, 1.074, 1.056, 1.038, 1.020, 0.158, 0. Total 10.000°.
21. **Recoil last-wins.** First kick: M4A1 + `Silencer_A_AR` + `Magazine_A_AR` → 1.300°. + `Silencer_A_AR` + `DotSight_A_AR_Red` → 1.261°. + `Silencer_A_AR` + `MechSight_Aim_II_AR` → 1.300°. + `Silencer_A_AR_Red` alone → 1.040°.
22. **Side kick from shot 1.** M4A1 + `Silencer_A_AR`, J = 0, one shot → view up 1.170° and sideways exactly 1.2° in the flag's direction. M4A1 + `Silencer_B_AR`, one shot → no side kick (stock: from shot 2).
23. **Recovery with a red dot.** M4A1 + `DotSight_ITECH_AR`, unzoomed, F = 0.30, recovery allowed, one sub-step per 12.5 ms → −0.015 per sub-step; cone back to n = 3 after 20 sub-steps (stock 26). With `MechSight_Aim_III_AR` instead → 17 sub-steps.
24. **Movement recovery.** M4A1 + `MechSight_Aim_III_AR`, M = 0.25 to standing still on the ground → M after sub-steps 1..7: 0.19825, 0.1465, 0.09475, 0.05116, 0.02763, 0.01492, 0.00806. Cone back to n = 3 at sub-step 7 (stock 11).
25. **No sight, no zoom.** M4A1 with nothing in slot 1, press zoom → FOV stays 90°, crosshair stays, the cone stays 3..100.
26. **Zoomed cone through a red dot.** M4A1 + `DotSight_A_AR`, zoomed, standing still, both meters 0, hold fire, no recovery → cone n for shots 1..8: 2, 3, 4, 6, 7, 8, 10, 10.
27. **Zoomed recovery with ITECH.** M4A1 + `DotSight_ITECH_AR`, zoomed:
    - growth per shot +0.049875;
    - from F = 0.30, recovery −0.016125 per sub-step, cone back to n = 2 after 17 sub-steps;
    - with `DotSight_A_AR` zoomed (no bonus): −0.01125 per sub-step, 24 sub-steps.
28. **Zoom steps.** Barrett + "Barrett Scope": presses → 30°, 10°, back to 90°. M24 + "Sniper Scope": 20°, then 90°. M4A1 + `DotSight_ITECH_AR`: 75°, then 90°.
29. **Zoom timing.** M4A1 + `DotSight_A_AR`, exact time:
    - zoom in: 90° → 50° in 0.150 s;
    - zoom out: 50° → 90° in 0.0857 s;
    - the weapon model is hidden from the start of zoom-in until zoom-out completes.
30. **Overlay hides the crosshair.** M4A1 + `DotSight_A_AR` zoomed → no crosshair lines. Range readout shown for a wall at 2000 units: "50.0", gold.
31. **Gauge sight.** M4A1 + `DotSight_B_AR`, HP 73/100, clip 24/30 → health image 70, clip image 80. HP 95 → 100. HP 4 → 0. Shown while unzoomed too.
32. **Silenced sound.** M4A1 + any silencer → silenced sound 1 with radius 1400 (stock 2000); AK-47 → 1600. An observer hears the silenced sound and sees no muzzle light.
33. **Part model variant.** M4A1 + `DotSight_A_AR` → variant 0 model at the breach socket. AK-47 + `DotSight_A_AR` → variant 2. M4A1 + `Magazine_A_AR` → no part model.
34. **Picked-up weapon.** A dropped M4A1 with `Magazine_A_AR`, clip 37, total 97 → the picker has clip 37, total 97, clip size 40. After a reload, 40/97.
35. **Stun bullet.** M4A1 + `Bullet_PVP_Stun_I_AR` vs a player, roll 1.0 → damage 31.5. Probability draw ≤ 0.5 → the victim's movement gets the stun penalty 0.20 for 1.0 s (`movement.md` $P_{stun}$); draw 0.6 → no stun.
36. **Missing record (ours, Q2).** A weapon whose index has no modification record → given normally with an empty accepted list (log once). Every attachment offered for it is rejected.
37. **Link by position (Q13).** Weapons file index 964 is named `M6A2_Event`; modification-file record 964 is named `M6A2_(Event)` → that record's list is used for the weapon.
38. **Pellet-count mismatch (Corrections 1).** A shotgun with 8 rays and (for the test) a magazine part with `VectorsPerRoundPer` 0.25 and no muzzle part → the client sends 10 rays, the server expects 8 → the shot does nothing on the server. The same 0.25 on the muzzle part and none on the magazine → the client sends 8 and the server expects 10 → nothing either. The same value on both parts → 10 rays land.
39. **Burst return with a muzzle part (Corrections 2).** Burst mode, `TripleShotMaxCamRecoilPitch` 8, muzzle part `MaxCamRecoilPitchPer` 0.25 (for the test) → the return's cap is rad 10°.
40. **Switch after a kick (Corrections 3).** M4A1 with a silencer kicks; switch to a weapon without parts at once → the next return step uses that weapon's cap and its ammo's rates, with no muzzle term.
41. **Part frame (Corrections 2026-10-30, 1).** Socket at (10, 2, 30) with local axes x̂ = (0, 0, 1), ŷ = (0, 1, 0), ẑ = (−1, 0, 0) (yaw +90°), `MuzzlePVPosOff` (−2.2, 0, 0) and zero rotation offset → part origin at (10, 2, 27.8), its axes equal to the socket's.
42. **Rotation offset adds angles (Corrections 2026-10-30, 1).** Socket rotation yaw 0.3, pitch 0.2, roll 0; `ScopePVRotOff` (0.1, 0, 0) → part rotation yaw 0.4, pitch 0.2, roll 0 (rebuilt yaw-pitch-roll); third person with the same numbers as `ScopeHHRotOff` → socket rotation × rotation(pitch 0, yaw 0.1, roll 0).
43. **Lens cut-out (Corrections 2026-10-30, 4).** A lens texel with alpha 150/255 → not drawn; 170/255 → drawn opaque, from both sides.
44. **Default outside the list (logic).** A weapon whose `DefaultScopeAttach` names an existing sight not on its accepted list, player with no sight of his own → sight slot −1, no sight model, zoom key does nothing; the same sight on the list → fitted, zoom steps from its `ZoomLevel`; on the list with `DefaultScopeVisible` 0 → fitted (zoom works) but its model hidden.
45. **Stock cases (data).** `M14 Cyanide Echo lv.1` (default `Sniper Scope`, not accepted) → no sight in a match; `P90 SE - Kraken common` (`P90 sight`) → none; a stock `M24` (`Sniper Scope` accepted) → the scope fitted.

## Open questions

1. **Steam build data.** All values are from the US reference build. The Steam weapons and modification files may add parts (grips or stocks would be new `Kinds` values) and change numbers. Check: list `Kinds`, `Type` and every key of the Steam modification file once it can be decrypted, and diff against the tables above.
2. **Do the odd bullet values ship?** QR I's +900 % range, Stun II/III's +200/+700 % range and the NZ-only damage of +200 % … +2000 % look like entry errors (perhaps meant as percent points for zombie modes). Check in the live game: M4A1 + QR I bullet against a wall at ~5000 units; FT I bullet against a zombie bot.
3. **Is Q1 visible?** Measure the first-shot view climb of an M4A1 with only a silencer A versus silencer A + any magazine (expect 1.17° versus 1.30°).
4. **Zoomed state during the FOV move.** Whether the zoomed spread values and the overlay apply from the zoom key press or only once the FOV reaches the target belongs to `weapon_classes.md`. Check: fire one shot 50 ms after pressing zoom with a red dot and compare the crosshair.
5. **Remote silenced sound path.** One third-person sound path picks the silenced sound from the **local** player's muzzle part instead of the shooter's. It may be unused in multiplayer. Check: does a silenced enemy sound different when you yourself carry no silencer?
6. **Fitting rules in the lobby.** The per-slot option fields (0/1/2) and which parts a player owns come from the account and shop, outside this spec. For the slice, a loadout file can name the parts per weapon directly.
7. **Character gear.** The full effect of gear mods and character attachments (protection tables, stamina) belongs to the characters and inventory specs.
8. **Socket frames in ours vs the game** (Corrections 2026-10-30, 1): the game applies no turn, so check our socket rotations against the game's on screen with a rifle silencer; if it sits inside the barrel there too, the stock data really does that.
