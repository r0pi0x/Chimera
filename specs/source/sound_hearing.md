# Source: how a listener hears a sound (distance, walls, panning, mix, rooms)

Source basis: the engine's mixer and spatializer are not public, so this
spec is worked out from behaviour and data. Measured 2026-11-21 in the
real TF2 client (listen server, `tools/tf2ref`, muted, on `itemtest`):
sounds were started at chosen world points with TF2's VScript sound call,
and the engine's own debug output was read: the per-start report
(`snd_showstart` 2, 5 and 6: distance gain, DSP mix, distance-variant
mix, matched mixer groups with `snd_showmixer 1`) and the live channel
overlay (`snd_show 1`, per-ear levels of a looping sound, read from
screenshots). Console variables were read from the running game (names
and values are data the game exposes). Game data read in place: TF2's,
HL2:DM's and CS:S's sound scripts, sound mixer scripts, DSP preset
scripts and soundscape scripts. The public SDK game code was read for who
emits what to whom (recipient filters, explosion temp entities, the
explosion ear-ringing effect). TF2, HL2:DM and CS:S run the same
multiplayer engine branch, so the engine rules here are assumed to hold
for all three; per-game differences come from their data and game code
and are listed in section 8. Nothing copied.
Status: draft (measured in TF2; HL2:DM and CS:S spot checks are open
questions)

## Summary

How loud a sound is at the listener, and where it seems to come from,
is decided in this order: the server decides **whether** a client hears
it at all (send radius, PAS/PVS), the client turns the sound's **sound
level** into a **distance gain** with a fixed curve (inverse distance
plus 1 dB of air loss per 300 units, softly saturating near 1, capped),
multiplies a flat **−8.1 dB** if one line trace from the listener to the
source is blocked by world geometry, multiplies the **mixer** value of
the sound's group (0.72 for almost everything, 1.0 for 140 dB player
weapons), **pans** it hard by direction (any source more than ~45° off
the view axis is in one speaker only in stereo-speaker mode), and sends
a distance-dependent share of it through the **room DSP** (reverb), whose
preset is chosen automatically from the room shape around the listener.
Sounds from the listener's own entity skip all of that and play centred
at full level. Very loud levels (HL2:DM's 140 dB guns) barely fall off
at all inside a map; TF2's 94 dB guns lose about half their gain by
1800 units.

## Units and conventions

- Distance d in Hammer units (u), from the listener to the sound's
  origin. The listener is the view origin (the eye).
- Gain: linear amplitude factor, 1 = the wave's full level. dB =
  20·log10(gain).
- Sound level L: the script's "dB" number (0–255). It is not physical;
  it selects the curve. 0 means "no distance falloff, not spatialized".
- Azimuth: horizontal angle of the source from the view's forward axis,
  positive to the left.
- The engine's per-channel meter (`snd_show 1`) shows per-ear levels on
  a 0–255 scale; "per-ear factor" below is that level divided by the
  channel's volume (0–255) and by the distance gain.

## Constants

| Name (ours) | Value | Unit | Meaning |
|---|---|---|---|
| ref_level | 60 | dB | level at which a sound has gain 1 at ref_dist (game cvar `snd_refdb`) |
| ref_dist | 36 | u | reference distance (`snd_refdist`) |
| air_loss | 1/300 | dB per u | extra loss per unit of distance (1 dB per 300 u, 1 dB per 25 ft) |
| raw_gain_cap | 10 | gain | raw distance gain is clamped to at most this (+20 dB) before the knee |
| knee_start | 0.5 | gain | raw gains above this are compressed toward 1 |
| knee_power_low | 2.5 | – | knee exponent for L ≤ 90 |
| knee_power_slope | 0.034 | per dB | exponent falls by this per dB above 90 (2.5 − 0.034·(L − 90)); 0.8 at 140 |
| fade_top | 0.008 | gain | below this raw gain, output fades linearly to 0 |
| fade_zero | 0.0045 | gain | raw gain at which the output reaches 0 (approximate, see Open questions) |
| obscured_db | −2.7 | dB | game cvar `snd_obscured_gain_dB` |
| obscured_factor | 3 | – | the applied loss is 3 × obscured_db = −8.1 dB (gain × 0.393) |
| mix_all | 0.72 | gain | Default_Mix value of the catch-all group |
| mix_player_weapons_loud | 1.0 | gain | player weapon sounds at L ≥ 140 |
| mix_explosions | 0.90 | gain | "explo" waves at L ≥ 120 |
| dsp_dist_min / max | 0 / 1440 | u | DSP wet-mix distance range (cvars `dsp_dist_min`, `dsp_dist_max`) |
| dsp_db_min | 80 | dB | sounds below this level get their wet mix scaled by dsp_db_mixdrop |
| dsp_db_mixdrop | 0.5 | – | scale for those quiet sounds |
| distvar_near | 240 | u | distance-variant blend starts (20 ft) |
| distvar_far | 1320 | u | distance-variant blend complete (110 ft) |
| send_radius(L) | 100·(L − 50) | u | server-side cut for sounds sent with the attenuation filter (L > 50; 500 for L ≤ 50) |
| origin_grid | 8 | u | world-position sounds arrive with coordinates truncated toward zero to a multiple of 8 |
| compat_range | 1000 | u | compatibility-attenuation sounds: gain 1 − d·a/1000 |
| ear_ring_dist | 240 | u | HL2:DM/CS:S: blast within this of the player rings the ears |
| shock_damage | 30 | hp | HL2:DM/CS:S: blast damage at or above this gives the "shock" muffle |

## Behavior

### 1. Who receives a sound (server side, before any gain)

- Weapon fire (TF2, HL2:DM, CS:S): the shooter's own client plays it
  itself (prediction); everyone else gets it if they are in the source's
  PAS **and** within send_radius(L) of it. TF2 guns at 94 dB: 4400 u.
  HL2:DM guns at 140 dB: 9000 u. Past the radius there is no sound at
  all, whatever the curve says.
- TF2 explosions are not a sound message: they are an explosion temp
  entity sent to clients whose **PVS** contains the blast point; each
  client plays the explosion sound locally from the world at that point.
  A blast in an area the listener's PVS doesn't include is never heard.
- Sounds sent from the world at a position (not attached to an entity)
  arrive with each coordinate truncated toward zero to a multiple of 8
  units (300 → 296, −46.9 → −40, −346.9 → −344, 253.1 → 248). Sounds
  attached to an entity follow the entity.

### 2. Distance gain (normal sound levels, 1 ≤ L ≤ ~160)

For a sound of level L at distance d:

1. Raw gain (amplitude):
   raw = 10^((L − 60)/20) · (36 / d) · 10^(−d/6000)
   i.e. in dB: (L − 60) − 20·log10(d/36) − d/300.
   Clamp raw to at most 10. (d = 0 counts as very close: raw = 10.)
2. Knee: if raw ≥ 0.5,
   gain = 1 − 0.5 · (0.5 / raw)^p, with
   p = 2.5 for L ≤ 90, p = 2.5 − 0.034·(L − 90) above (p(94) = 2.364,
   p(95) = 2.33, p(100) = 2.16, p(120) = 1.48, p(140) = 0.80,
   p(150) = 0.46). The knee is continuous at raw = 0.5 and approaches 1;
   because of the cap, a 140 dB sound never exceeds 0.954 and a 120 dB
   one 0.994.
3. Middle: if 0.008 ≤ raw < 0.5, gain = raw (plain inverse distance with
   air loss).
4. Fade-out: if raw < 0.008, gain = 0.008 · (raw − 0.0045)/(0.008 − 0.0045),
   and 0 below raw = 0.0045 (about −47 dB). The sound is then inaudible.

The script's volume multiplies the result. L = 0 gives gain 1 at any
distance and no spatialization.

Resulting table (gain; "silent" = where the fade reaches 0, "½" = where
gain drops below 0.5):

| L | 36 u | 100 | 250 | 500 | 1000 | 1500 | 2000 | 3000 | 4000 | 6000 | 9000 | ½ at | silent |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| 60 | 0.909 | 0.346 | 0.131 | 0.059 | 0.025 | 0.013 | 0.008 | 0 | 0 | 0 | 0 | 71 | 2770 |
| 64 | 0.971 | 0.604 | 0.207 | 0.094 | 0.039 | 0.021 | 0.013 | 0.003 | 0 | 0 | 0 | 111 | 3420 |
| 70 | 0.995 | 0.930 | 0.414 | 0.188 | 0.078 | 0.043 | 0.026 | 0.012 | 0.004 | 0 | 0 | 211 | 4500 |
| 75 | 0.999 | 0.983 | 0.810 | 0.334 | 0.138 | 0.076 | 0.047 | 0.021 | 0.011 | 0 | 0 | 361 | 5480 |
| 80 | 1.000 | 0.996 | 0.955 | 0.675 | 0.245 | 0.135 | 0.084 | 0.038 | 0.019 | 0.003 | 0 | 581 | 6530 |
| 85 | 1.000 | 0.999 | 0.989 | 0.923 | 0.436 | 0.240 | 0.149 | 0.067 | 0.034 | 0.011 | 0 | 911 | 7630 |
| 90 | 1.000 | 1.000 | 0.997 | 0.982 | 0.833 | 0.427 | 0.264 | 0.120 | 0.061 | 0.019 | 0 | 1361 | 8760 |
| 94 | 1.000 | 1.000 | 0.999 | 0.993 | 0.940 | 0.755 | 0.419 | 0.190 | 0.097 | 0.030 | 0.004 | 1811 | 9700 |
| 95 | 1.000 | 1.000 | 0.999 | 0.994 | 0.953 | 0.811 | 0.470 | 0.213 | 0.109 | 0.034 | 0.006 | 1931 | 9940 |
| 100 | 0.999 | 0.999 | 0.999 | 0.998 | 0.984 | 0.941 | 0.835 | 0.379 | 0.194 | 0.060 | 0.013 | 2631 | 11140 |
| 105 | 0.999 | 0.999 | 0.999 | 0.999 | 0.993 | 0.978 | 0.943 | 0.725 | 0.345 | 0.107 | 0.022 | 3441 | 12370 |
| 110 | 0.998 | 0.998 | 0.998 | 0.998 | 0.997 | 0.990 | 0.976 | 0.898 | 0.655 | 0.190 | 0.040 | 4331 | 13620 |
| 120 | 0.994 | 0.994 | 0.994 | 0.994 | 0.994 | 0.994 | 0.992 | 0.975 | 0.933 | 0.618 | 0.126 | 6341 | 16170 |
| 140 | 0.954 | 0.954 | 0.954 | 0.954 | 0.954 | 0.954 | 0.954 | 0.954 | 0.954 | 0.932 | 0.762 | 10921 | 21430 |

Every measured point (levels 50–150, 20–12000 u, 90 starts) matched
this to the third decimal the engine prints, except in the fade-out
region, where the fit is ±0.001.

### 3. Compatibility attenuation (CS:S's "CompatibilityAttenuation" entries)

A sound whose level is in the 256–511 range is a compatibility sound:
its attenuation is a = 20/(L − 256 − 50) (as for normal levels) and its
gain is linear in distance:

gain = max(0, 1 − d·a/1000)

with no knee, no cap and no air loss. Measured with level 256 + 70
(a = 1.0): 0.899 at 104 u, 0.695 at 304, 0.492 at 504, 0.399 at 600
(prediction 0.896, 0.696, 0.496, 0.400). A level of 256 + 51 (a = 20)
was silent at 104 u. The engine reports such a sound's level as the
part below 256 (here "70 dB").

The script value goes through the level encoding first: level =
trunc(50 + 20/a), so a = 0.52 becomes level 88 and back a = 20/38 =
0.526: silent beyond 1900 u (not 1923). CS:S's values: footsteps,
impacts, reloads a = 1.0 → silent beyond 1000 u; AK-47/M4A1/M249/XM1014
0.52 → 1900 u; Galil/M3/SG552/FAMAS/AUG 0.48 → level 91, a = 0.488,
2050 u; G3SG1/SG550 0.40 → level 100, 2500 u; AWP 0.28 → level 121,
a = 0.282, 3550 u; Desert Eagle 0.6 → level 83, a = 0.606, 1650 u;
MAC-10/MP5/P90/UMP45 0.64 → level 81, a = 0.645, 1550 u; TMP and Scout
1.6 → level 62, a = 1.667, 600 u; pistols "ATTN_NORM" (0.8) → level 75,
1250 u.

### 4. Walls: obscuring

- Each sound gets **one** line trace from the listener's ear to the
  sound's origin, against world geometry. If it is blocked, the sound's
  gain is multiplied by 10^(3·obscured_db/20) = 10^(−8.1/20) = **0.393**.
  Nothing else changes: no low-pass filter, no change of pan or DSP mix
  was seen.
- All or nothing: moving a source one 8-unit step past a floor surface
  took it from unblocked (157 on the meter) to fully blocked (61 =
  157 × 0.39); there are no partial states and no dependence on wall
  thickness or on how many walls lie between (one floor, a 600 u room
  wall and the far side of a building all gave 0.38–0.40).
- The loss is the same at every distance and level (ratio 0.386–0.395
  measured at 145–2550 u, levels 75 and 94) and is applied after the
  distance gain and knee.
- Changing the cvar scales it: −1 gives 0.707 (−3 dB), 0 gives no loss,
  −10 gives about 0.03.
- The trace is deferred (game cvar `snd_defer_trace` 1): it is not done
  on the sound's first mixed frame. How often it is redone for a long
  or moving sound, and whether the change is ramped, is open (Q2).
- A separate foliage loss (cvar `snd_foliage_db_loss` 4 dB) exists for
  traces through foliage surfaces; not measured (Q3).
- Sounds from the listener's own entity are never obscured (they are
  not spatialized, section 5).

### 5. Panning and the listener's own sounds

Measured in the reference box's stereo-speaker mode (`snd_surround_speakers`
2, front and rear stereo speaker positions 90°, stereo pan exponent 1.5,
radial weight 0). The ratio of left to right level for a source at
azimuth a (positive = left), same distance:

| a | 0° | 3° | 6° | 9° | 12° | 15° | 21° | 24° | 27° | 30° | 33° | 36° | 39° | ≥ 44° |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| L/R | 1.00 | 1.05 | 1.17 | 1.31 | 1.50 | 1.69 | 2.26 | 2.74 | 3.32 | 3.85 | 5.0 | 7.7 | 10.4 | ∞ (right ear 0) |

- Mirror image for negative azimuth (−30° gave R/L = 3.85) and for
  sources behind (150° gave 3.88, the same as 30°; 165° the same as
  15°). From about 44° to 136° on either side the far ear gets nothing.
  A fit within ~10% between 9° and 39°: L/R ≈ tan(45° + a).
- Overall level by direction, relative to the near ear of a source
  straight to the side (1.0): straight ahead 0.64 per ear, straight
  behind 0.48 per ear, straight above or below 0.90 per ear, 45° up and
  ahead 0.65 per ear. Elevation does not pan.
- Level 0 ("no falloff") sounds are not spatialized: 0.89 per ear,
  centred, wherever they are.
- **Sounds from the listener's own entity** (the local player's gun,
  footsteps, voice, all emitted from the player) are not spatialized,
  not distance-attenuated and not obscured: centred, 1.29 per ear per
  unit of volume on the meter where a level-0 world sound shows 0.64,
  i.e. +6 dB over a non-spatial world sound (likely because a stereo
  wave plays as true stereo there instead of being folded to mono, Q4).
- The ")" prefix (spatial stereo) is on every TF2 gun wave and most HL2:DM gun
  wave; such a stereo wave from another entity is panned like a mono
  source.
- Headphone mode (`snd_surround_speakers` 0) and surround modes use
  other laws (headphone pan exponent 1.0, radial weight 1; surround
  speakers at 45°/135°); not measured (Q5).

### 6. Mixer groups (both games' "Default_Mix")

Each sound is matched against the sound mixer script's group rules, in
file order; a rule matches on a substring of the wave path, a substring
of the emitting entity's class name, the channel and a level range. The
sound's mix value is the value of the **first matched group that has a
value in the active mixer**; the active mixer is `Default_Mix` in all
three games (no TF2 or HL2:DM soundscape picks another). Values that
matter:

| Sound | Groups it matches (in order) | Mix |
|---|---|---|
| TF2 gun from a player (94 dB, "weapons/") | Player_Weapons, Loud, All | 0.72 |
| any gun from a player at L ≥ 140 (HL2:DM) | Player_Weapons_Loud, Player_Weapons, Combat, Weapons, VeryLoud, All | 1.0 |
| TF2 explosion (95 dB, world) | Loud, All | 0.72 |
| HL2:DM explosion (140 dB, "explode" wave, world) | Explosions, Combat, Weapons, ... | 0.90 |
| a world sound at L ≥ 120 with "weapon" in its path | Combat, Weapons, ... | 0.79 |
| bullet impacts ("impact_bullet") | bullethit | 0.67 |
| "fvox/" suit voice from a player | Player_Suit | 0.56 |
| music ("music/") | Music | 0.81 |
| everything else | All | 0.72 |

So HL2:DM's guns sit 2.9 dB above everything else in the mix and TF2's
do not. The mixer value multiplies the gain (measured: a world sound's
meter level = 255 × gain × 0.72 × per-ear factor).

The groups also carry ducking columns (priority, "is ducked", "causes
ducking", duck-to percentage, threshold): in Default_Mix ambient groups
(alarms, atmosphere, wind, water, fire, gas, levels, creatures,
machines) are duckable to 60%, vehicles to 68%, music to 75%; loud
player and NPC weapons, explosions (≥ 120 dB), voices and dialog cause
ducking. The duck's timing and trigger rule are engine-side and not
measured (Q6). The separate dialog ducker is off by default
(`snd_ducking_off` 1).

### 7. Room effects (DSP)

- **Room preset**: chosen by the soundscape the listener is in
  (`dsp` key, specs/cs_source/sounds.md soundscapes). Preset 1 means
  **automatic**: the engine analyses the space around the listener and
  builds a reverb by blending two templates (presets 100–133: room,
  duct, hall, tunnel, street, alley, courtyard, open space, each as
  "small bright" and "huge dull", empty and diffuse variants) by room
  size and reflectivity, storing the result in a slot 60–99. Its debug
  output reports, per analysed spot, the size (two horizontal extents
  and a height, in feet), diffusion, reflectivity, a count of objects
  and whether sky is visible. TF2's soundscapes use `dsp` 1 in 101 of
  their entries (others: 2, 3, 4, 17, 21, 26, 137–139); HL2:DM's use 1
  everywhere except one 19 (Concrete L). On TF2's itemtest the
  automatic room gave mix range 0.32–0.80 (slot 60).
- **Wet share per sound**: wet = mix_min + (mix_max − mix_min) ·
  clamp((d − dsp_dist_min)/(dsp_dist_max − dsp_dist_min), 0, 1), with
  the preset's mix range (most fixed presets 0.2–0.7). Halved
  (dsp_db_mixdrop 0.5) for sounds below 80 dB. Measured with the
  automatic room: 94 dB: 0.32 at 0, 0.35 at 100, 0.49 at 500, 0.65 at
  1000, 0.80 from 1440; 75 dB: 0.16 at 0, 0.24 at 500, 0.33 at 1000,
  0.40 from 1440. Changing dsp_dist_min/max moved it as the formula
  says. Distant sounds are mostly reverb; near ones mostly dry.
- **Distance-variant waves** ("^" prefix, two-channel file: left = near
  version, right = far version): far weight = clamp((d − 240)/1080, 0, 1),
  near weight = 1 − far weight; fixed distances (20 ft and 110 ft), not
  tied to the DSP cvars and the same for every level (measured 0.05 at
  296, 0.15 at 400, 0.24 at 496, 0.52 at 800, 0.70 at 1000, 0.98 at
  1296, 1.00 from 1320, at 75 and 94 dB, and unchanged with
  dsp_dist_max 2880 or dsp_dist_min 400). HL2:DM's explosions use it;
  TF2's guns and explosions do not.
- **Underwater**: preset `dsp_water` (14, Water S) replaces the room
  while the listener is under water.
- **Facing away**: an optional low-pass for sounds behind the listener
  (presets 30/31) is off by default (`dsp_facingaway` 0); always 1.0.
- **Blast ear ringing (HL2:DM; CS:S assumed, its player code is not public)**: when a player takes blast
  damage, if the inflictor is within 240 u of the player's origin or the
  damage is ≥ 30, the player gets a temporary player DSP: random preset
  35–37 ("shock muffle") if damage ≥ 30, else 32–34 ("explosion ring").
  They last 1.6 s and fade out exponentially (preset data). TF2 turns
  this off (its players ignore the hook). Mortar shells in HL2 skip it.
- The DSP preset file is the same in all three games (TF2 adds presets
  134–135); the processors (reverbs, delays, filters, diffusors) are
  engine-side; the preset parameters are data loaded from the install.

### 8. Per-game weapon and explosion data

TF2 (game_sounds_weapons.txt; all CHAN_WEAPON, volume 1.0, ")" waves,
unless noted):

| Entry | Level | Note |
|---|---|---|
| Weapon_RPG.Single / SingleCrit (Rocket Launcher) | 94 | |
| Weapon_Minigun.WindUp | 94 | |
| Weapon_Minigun.Fire / FireCrit | 94 | CHAN_STATIC, looping wave |
| Weapon_Minigun.Spin | 75 | CHAN_STATIC, loop |
| Weapon_Scatter_Gun.Single | 94 | |
| Weapon_Shotgun.Single | 94 | |
| Weapon_SniperRifle.Single, GrenadeLauncher, StickyBombLauncher, Pistol, SMG, Revolver, SyringeGun | 94 | |
| Weapon_FlameThrower.FireStart / FireLoop | 94 | Start on CHAN_STATIC |
| BaseExplosionEffect.Sound (rocket explosion) | 95 | 3 waves, world, client-local from the TE |
| Weapon_Grenade_Pipebomb.Explode (pipes, stickies) | 95 | 3 waves |

Most-used levels across TF2's weapon script: 74 (227 entries), 75
(209), 94 (176), 64, 84, 95. Mixer: 0.72. Send radius 4400 u.
So a TF2 gun at 1000 u: 0.94 × 0.72 = 0.68 of full scale, 0.27 behind
a wall; at 2000 u: 0.30 (0.12 behind a wall); at 4000 u: 0.07.

HL2:DM (hl2mp's game_sounds_weapons.txt):

| Entry | Level | Volume | Note |
|---|---|---|---|
| Weapon_357.Single | 140 | 0.93 | pitch 88–93 |
| Weapon_SMG1.Single | 140 | 0.55 | the entry has a 90 dB key then a 140 one; the later wins |
| Weapon_SMG1.Double (grenade launch) | 140 | 0.61 | |
| Weapon_Shotgun.Single / Double | 140 | 0.86 / 1.0 | mono waves |
| Weapon_AR2.Single, Weapon_Pistol.Single, Weapon_RPG.Single | 140 | 0.8, 0.55, 0.55 | |
| Weapon_Crossbow.Single | 75 | 0.61 | quiet on purpose |
| Weapon_Crossbow.BoltHitWorld | 80 | 0.95–1.0 | CHAN_BODY |
| BaseExplosionEffect.Sound | 140 | 1.0 | CHAN_STATIC, "^" distance-variant waves |

HL2:DM's 140 dB guns: gain 0.954 out to 6000 u, 0.76 at 9000 u (where
the send radius cuts them), mixer 1.0 — effectively the same loudness
anywhere on a DM map; only the wall loss (−8.1 dB), panning and the DSP
wet share tell distance and direction. This is the original's
behaviour, not a bug.

CS:S: guns use compatibility attenuation (section 3): linear fade to
silence at 1550–3550 u; the mixer value of those sounds is Q7.

## Per-frame order (client, per playing sound)

1. (Start) Look up the entry; pick wave, volume, pitch, level.
2. Distance d from the ear to the sound's origin (an entity sound: the
   entity's current position; a world sound: its 8-unit-truncated
   position; the listener's own entity: none, go to step 8).
3. Distance gain (section 2, or 3 for compatibility levels).
4. Obscuring trace result (deferred), × 0.393 if blocked.
5. × script volume × mixer value (× any duck).
6. Distance-variant blend for "^" waves.
7. Pan by azimuth into per-speaker levels (section 5).
8. Split dry/wet by the DSP mix; wet goes through the room preset (and
   the player DSP if set).

## Edge cases

- d = 0 (or within a few units): raw gain clamps at 10, gain 1 −
  0.5·0.05^p: 0.9996 at 94 dB, 0.954 at 140 dB.
- Levels above ~163 make the knee exponent ≤ 0: 180 dB measured 0.020 at
  9000 u (nonsense). No sound in these games uses it.
- A sound exactly on a surface (trace ends on it) is not obscured; one
  step behind it is.
- TF2's Minigun spin (75 dB) is much quieter at range than its fire
  loop (94 dB): 0.33 vs 0.99 at 500 u.

## Quirks

- The −8.1 dB wall loss is all-or-nothing from one ray; a source behind
  a thin pillar is as muffled as one behind a mountain, and one with any
  clear line is not muffled at all. Keep.
- Hard panning past ~45°: in stereo-speaker mode a sideways source is in
  one ear only. Keep (it is the main direction cue).
- 140 dB sounds are capped at 0.954 even point-blank, slightly quieter
  than a 94 dB sound up close (before the mixer, which gives them 1.0
  vs 0.72).
- The 8-unit position truncation of world sounds. Harmless; keep or not.

## Test cases

| Setup | Input | Expected |
|---|---|---|
| L 75, d 300 | distance gain | raw 0.601 → 0.673 (knee) |
| L 75, d 1000 | distance gain | 0.138 (raw 0.1383) |
| L 75, d 4800 | distance gain | ≈ 0.005 (fade region) |
| L 75, d 5600 | distance gain | 0 |
| L 94, d 1100 | distance gain | 0.919 |
| L 94, d 1900 | distance gain | 0.458 |
| L 94, d 2400 | distance gain | 0.299 |
| L 110, d 3400 | distance gain | 0.831 |
| L 120, d 6000 | distance gain | 0.618 |
| L 140, d 100 | distance gain | 0.954 |
| L 140, d 7000 | distance gain | 0.895 |
| L 150, d 9000 | distance gain | 0.808 |
| L 64, d 50 | distance gain | 0.934 (raw 1.12) |
| L 0, d 5000 | distance gain | 1.0, centred |
| compatibility a = 1.0, d 504 | distance gain | 0.496 |
| CS:S AK-47 (a 0.52) d 1950 | distance gain | 0 |
| L 75 world sound at 297 u behind a floor | obscured | gain × 0.393 (0.673 → 0.265) |
| same, snd_obscured_gain_dB −1 | obscured | × 0.708 |
| source at azimuth 30°, stereo speakers | pan | L/R = 3.85 |
| source at azimuth 60° | pan | right ear 0 |
| source at azimuth 150° | pan | L/R = 3.88 |
| own player's gun | spatial | centred, no distance or wall loss |
| automatic room 0.32–0.80, L 94, d 500 | wet share | 0.487 |
| same, L 75 | wet share | 0.243 |
| "^" wave at d 800 | far weight | 0.519 |
| TF2 gun from a player | mixer | 0.72 |
| HL2:DM 357 from a player | mixer | 1.0 |
| HL2:DM explosion | mixer | 0.90 |
| HL2:DM player hit by 20 damage of blast 200 u away | player DSP | random 32–34, 1.6 s |
| TF2 player, same | player DSP | none |

## Open questions

1. **Fade-out shape.** Below raw gain 0.008 the printed gain (3
   decimals) fits a straight line to 0 at raw ≈ 0.0045; the true shape
   (it may be a dB-domain fade or a different end point) needs finer
   output. Check: the per-channel meter of a looping 120 dB sound at
   distances where raw is 0.004–0.009.
2. **Obscuring update rate and smoothing.** How often the deferred trace
   is redone for a playing sound, and whether the 0.393 step is ramped.
   Check: a looping sound, the listener stepped from clear to blocked
   with `host_framerate`, the meter read every frame (`tf2ref` movie).
3. **What blocks.** Only world geometry was tested. Do brush entities
   (doors, func_wall), props, players or water surfaces block? What does
   the foliage loss apply to? Check with a door and a prop between.
4. **Own-entity +6 dB.** Whether it is stereo playback of ")" waves
   (mono waves would then show 0.64 like a level-0 sound) or a rule for
   the local player. Check with a mono wave from the player entity.
5. **Pan law in other speaker modes**, and the exact stereo-speaker law
   (the table is measured; the tan fit is approximate, with a soft
   centre below 6°). Changing `snd_surround_speakers` is archived on the
   reference account; measure on a scratch config only.
6. **Mixer ducking dynamics** (attack, release, threshold use) and
   whether loud weapons audibly duck ambient sounds in TF2.
7. **CS:S compatibility sounds in the mixer and DSP**: which level (the
   encoded 256+ value or the part below 256) the group ranges and the
   80 dB DSP rule see. Check in CS:S with `snd_showmixer 1`.
8. **HL2:DM and CS:S spot checks**: the curve, wall loss and pan were
   measured in TF2 only. Repeat three points (L 75 at 300 u, L 140 at
   9000 u, the floor test) with `snd_showstart 5` in each.
9. **Automatic room analysis**: how the space is sampled (traces, how
   many, how often), how size/reflectivity map to the template blend,
   and when a new room is built. Read with `adsp_debug 1` while walking.
10. TF2 soundscapes name DSP presets 137–139, which its preset file does
    not define; what plays then.
