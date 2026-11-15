# Halo: Combat Evolved: weapons

Source basis: the weapon tags of the original Xbox disc's multiplayer
maps (all 15 `weap` tags of `bloodgulch.map`, compared field by field
with the other 12 multiplayer maps), the first-person and weapon
animation graphs they reference, the globals tag and the multiplayer
biped, read with throwaway scripts; and a decompilation of an Xbox
build of the engine (the weapon object and its triggers and magazines,
the unit's weapon handling, inventory, pickup and drop, player melee,
grenade throwing as far as the weapon gates it, the player's input
mapping, aim assist, the random number generator), used for behaviour
only. The decompilation describes a different Xbox build than the
disc; where they differ the disc wins, and the spec says so.
Status: draft

Sections: Summary; Units and conventions; Constants; The weapon tag;
What is on the disc; Behaviour (1 runtime state, 2 drawing and
switching, 3 inventory, pickup, swap and drop, 4 the trigger, 5 firing
a round, 6 where the shot starts and where it goes, 7 error and
spread, 8 heat, 9 the battery, 10 charging, 11 magazines and reloading,
12 zoom, 13 melee, 14 grenades, 15 aim assist, 16 recoil and the
first-person weapon, 17 smaller systems); Multiplayer weapon numbers;
Per-tick order; Edge cases; Quirks; Test cases; Confidence per section;
Open questions.

---

## Summary

- A weapon is an item object whose tag adds one struct (0x200 bytes at
  offset 0x308 of the tag) with up to two **magazines** and two
  **triggers**. A trigger fires one projectile tag; this spec ends where
  the projectile is created (`projectiles.md` takes it from there).
- Everything is counted in whole ticks. A trigger fires when the
  number of ticks it has been idle, plus one, reaches 30 / rate; so a
  rate becomes the smallest whole number of ticks at least 30 / rate.
  The assault rifle's 15 per second is exactly 2 ticks; the pistol's
  3.5 is 9 ticks (3.33 per second); the shotgun's 1 is 30 ticks.
- Accuracy is one number per trigger, the **error** (0 to 1), that
  rises at a fixed rate while the trigger is held and falls when it is
  not. A shot leaves at a random angle drawn **uniformly** between the
  trigger's minimum error and lerp(lower, upper, error), which makes
  spread heavier in the middle than a uniform cone.
- Lengths of draws, reloads and melee come from **animation frame
  counts** (one frame per tick), not from the tag's ready and reload
  times, which the engine does not read.
- Plasma weapons have **heat** (overheat at 1, back below the recovery
  threshold to fire again) and a **battery** ("age", 0 fresh to 1
  dead) that slows the rate of fire, slows cooling and, near empty,
  makes rounds misfire. The plasma pistol charges while held and fires
  on release: a tap is a normal bolt, 18 ticks or more is the charged
  bolt.
- There is no camera recoil in the weapon: the kick a player feels is
  the per-shot "firing damage" effect applied to the shooter
  (`projectiles.md` owns damage effects) plus a first-person model
  push.

## Units and conventions

- World units (WU, 3.048 m), +Z up, radians, the 30 Hz tick and the tag
  primitives are those of [cache_file.md](cache_file.md). Offsets in
  the layout tables are from the start of the weapon tag's main struct
  unless a table says otherwise.
- Angles in tags are radians; this spec gives degrees beside them.
- Time fields in weapon tags are seconds. The tool that built the maps
  also stored, per trigger, six **per-tick rates** derived from them
  (the "runtime" fields below); the engine uses those, not the
  seconds. The derivation is r = 1 / (30 t) for a time t > 0 and r = 1
  for t = 0 (checked on all 15 tags: 1.0 s gives 0.0333333, 0.6 s
  gives 0.0555556, 0 gives 1).
- Where the engine turns seconds into a tick count it multiplies by 30
  in single precision and **truncates** toward zero (charging time
  0.6 s gives 18 ticks, charged time 45 s gives 1350, chamber time 0
  gives 0).
- "Tick t" in timelines: tick 0 is the first tick on which the input in
  question is down. A weapon held by a player is updated after its
  holder in the same tick (see Per-tick order), so an input read on
  tick t can fire on tick t.
- All reals are IEEE single precision and the test cases were computed
  that way; rounding to double can move a shot by one tick where a
  quotient is exactly whole (see Edge cases).
- "Rounds loaded" is what is in the magazine; "rounds total" is the
  reserve **not** counting the loaded rounds.

## Constants

| Name (ours) | Value | Unit | Meaning |
|---|---|---|---|
| TICK_RATE | 30 | ticks/s | game tick |
| WEAPON_PART_OFFSET | 0x308 | bytes | weapon struct begins here in a `weap` tag (object part 0x000-0x17B, item part 0x17C-0x307 belong to objects.md) |
| WEAPON_PART_SIZE | 0x200 | bytes | weapon struct; whole `weap` main struct is 0x508 |
| MAX_TRIGGERS | 2 | | triggers per weapon the engine runs |
| MAX_MAGAZINES | 2 | | magazines per weapon |
| MAX_WEAPONS_PER_UNIT | 4 | slots | inventory slots of a unit |
| WEAPON_LIMIT | 2 | weapons | weapons that count toward the maximum a player may hold (flag and ball don't count) |
| IDLE_TICKS_CAP | 127 | ticks | the trigger's idle counter saturates here (a signed 8-bit count) |
| ROF_EPSILON | 0.0001 | rounds/s | a rate at or below this means "no limit" (required ticks 0) |
| ANALOG_TRIGGER_THRESHOLD | 0.05 | | an analog-rate trigger counts as down above this pull |
| HEAT_CAP_PLAYER | 1.0 | | heat clamp for a player's weapon (an AI's clamps at its overheat threshold) |
| MISFIRE_SPEW_FACTOR | 2 | x | misfire chance doubles while a trigger spews |
| SWAP_HOLD_TICKS | 7 | input frames | hold the action button this long to swap for a weapon on the ground (globals, player control, offset 0x6C; disc value) |
| MELEE_SPEEDUP_SHIFT | 2 | bits | a melee lasts F - floor(F / 4) ticks for a first-person melee animation of F frames |
| MELEE_RAY_FORWARD | 0.8 | WU | forward length of the melee probe rays |
| MELEE_RAY_SPREAD | 0.1 | WU per step | lateral step of the 5 x 5 probe grid |
| MELEE_AIRBORNE_TICKS | 15 | ticks | airborne longer than this: melee damage scale 1.5 |
| MELEE_VEHICLE_PUSH | 0.035 | WU/tick per unit of acceleration scale | push given to a vehicle hit by a melee |
| DROP_CONE | pi/8 (22.5 deg) | rad | dropped item's random direction around the aim |
| DROP_SPEED | 0.0266667 to 0.04 | WU/tick | dropped item's speed (0.8 to 1.2 WU/s), plus the dropper's velocity |
| AUTOAIM_TRACE | 128 | WU | length of the camera trace that corrects shot convergence |
| MAGNETISM_YAW_CAP | 6 deg | per tick | clamp of the target's angular velocity used by magnetism |
| MAGNETISM_PITCH_CAP | 3 deg | per tick | same, pitch |
| MAGNETISM_FRICTION | 0.6 | | globals (player control +0x00) |
| MAGNETISM_ADHESION | 0.7 | | globals (player control +0x04) |
| MAGNETISM_INCONSEQUENTIAL | 0.5 | | globals (player control +0x08): magnetism scale on "inconsequential" units |
| CAMO_DING_DEFAULT | 0.1 | | camouflage lost per shot when the weapon's ding is 0 |
| CAMO_FLOOR | 0.05 | | a shot never takes camouflage below this |
| CAMO_REGROWTH_DEFAULT | 1/120 | per tick | camouflage regrowth when the weapon's rate is 0 |
| LIGHT_DECAY | 1/24 | per tick | weapon integrated light power decay when the unit does not drive it |
| ZOOM_FOV_MIN / MAX | pi/100 / 99 pi/100 | rad | a zoomed field of view outside this range is not applied |
| RNG_A, RNG_C | 1664525, 1013904223 | | the game-state random generator (below) |
| DIRECTION_TABLE | 1026 | directions | unit vectors drawn from for shot spread |

The random generator shared by the game state: a 32-bit seed s is
replaced by s' = (1664525 s + 1013904223) mod 2^32 on every draw. An
integer draw returns r = s' >> 16 (0..65535); an integer range draw
(lo, hi) returns lo + floor((hi - lo) r / 65536); a real draw returns
r / 65535 (0 to 1 inclusive); a real range draw returns
lo + (hi - lo) * real draw. The direction table is the vertex set of an
octahedron (the six axis directions) whose faces are subdivided 16
times along each edge and pushed out to the unit sphere: 8 x 14 x 15 /
2 + 12 x 15 + 6 = 1026 vertices. Its order and positions are fixed and
given in "The direction table" below, so a seeded implementation can
reproduce the original's shot pattern.

## The weapon tag

A `weap` tag's main struct is the object part (objects.md), the item
part (objects.md), then the weapon part below, which starts at
**0x308** and is 0x200 bytes. Offsets in this section are from 0x308
(add 0x308 for the offset in the tag). "Unused" means no code reads it
in the reference; "AI" means only actors read it.

### Weapon part

| Offset | Size | Type | Field | Unit | Meaning |
|---|---|---|---|---|---|
| 0x000 | 4 | flags32 | flags | | see Weapon flags |
| 0x004 | 32 | tag string | label | | weapon class label the holder's animation graph looks up (`ar`, `hp`, `sg`...) |
| 0x024 | 2 | enum16 | secondary trigger mode | | 0 normal, 1 slaved to primary, 2 inhibits primary, 3 loads alternate ammunition, 4 loads multiple primary ammunition |
| 0x026 | 2 | i16 | maximum alternate shots loaded | shots | for modes 3 and 4, and the overload state |
| 0x028 | 8 | 4 x enum16 | A in, B in, C in, D in | | function sources exported to the object's A-D inputs: 0 none, 1 heat, 2 primary ammunition, 3 secondary ammunition, 4 primary rate of fire, 5 secondary rate of fire, 6 ready, 7 primary ejection port, 8 secondary ejection port, 9 overheated, 10 primary charged, 11 secondary charged, 12 illumination, 13 age, 14 integrated light, 15 primary firing, 16 secondary firing |
| 0x030 | 4 | f32 | ready time | s | unused (the draw length is the first-person ready animation) |
| 0x034 | 16 | tag reference | ready effect | `effe` or `snd!` | played when the weapon is readied |
| 0x044 | 4 | fraction | heat recovery threshold | | overheated weapon may fire again once heat < this |
| 0x048 | 4 | fraction | overheated threshold | | heat >= this overheats |
| 0x04C | 4 | fraction | heat detonation threshold | | heat > this risks a detonation per round |
| 0x050 | 4 | fraction | heat detonation fraction | | chance per round of that detonation |
| 0x054 | 4 | f32 | heat loss per second | 1/s | heat lost per second (per tick: / 30) |
| 0x058 | 4 | fraction | heat illumination | | illumination function = this x heat (minimum of the triggers' glow) |
| 0x05C | 16 | | padding | | |
| 0x06C | 16 | tag reference | overheated | `effe` or `snd!` | looping effect while overheated |
| 0x07C | 16 | tag reference | detonation | `effe` or `snd!` | effect when the weapon detonates (it is then deleted) |
| 0x08C | 16 | tag reference | melee damage | `jpt!` | damage a player's melee deals with this weapon in hand |
| 0x09C | 16 | tag reference | melee response | `jpt!` | damage applied to the attacker when the melee hits something |
| 0x0AC | 8 | | padding | | |
| 0x0B4 | 16 | tag reference | actor firing parameters | `actv` | AI |
| 0x0C4 | 4 | f32 | near reticle range | WU | HUD/AI (hud.md) |
| 0x0C8 | 4 | f32 | far reticle range | WU | HUD/AI |
| 0x0CC | 4 | f32 | intersection reticle range | WU | HUD/AI |
| 0x0D0 | 2 | | padding | | |
| 0x0D2 | 2 | i16 | magnification levels | levels | zoom levels (0: no zoom) |
| 0x0D4 | 8 | real_bounds | magnification range | x | lower, upper magnification |
| 0x0DC | 4 | angle | autoaim angle | rad | aim assist: bullet bending cone |
| 0x0E0 | 4 | f32 | autoaim range | WU | |
| 0x0E4 | 4 | angle | magnetism angle | rad | aim assist: look stickiness cone |
| 0x0E8 | 4 | f32 | magnetism range | WU | |
| 0x0EC | 4 | angle | deviation angle | rad | most a shot may be bent from the aim (the larger of this and the autoaim angle is used) |
| 0x0F0 | 4 | | padding | | |
| 0x0F4 | 2 | enum16 | movement penalized | | 0 always, 1 when zoomed, 2 when zoomed or reloading; unused in the reference (open question 7) |
| 0x0F6 | 2 | | padding | | |
| 0x0F8 | 4 | fraction | forward movement penalty | | unused in the reference |
| 0x0FC | 4 | fraction | sideways movement penalty | | unused in the reference |
| 0x100 | 4 | | padding | | |
| 0x104 | 4 | f32 | minimum target range | WU | AI |
| 0x108 | 4 | f32 | looking time modifier | | AI |
| 0x10C | 4 | | padding | | |
| 0x110 | 4 | f32 | light power-on time | s | unused in the reference |
| 0x114 | 4 | f32 | light power-off time | s | unused in the reference |
| 0x118 | 16 | tag reference | light power-on effect | `effe` or `snd!` | |
| 0x128 | 16 | tag reference | light power-off effect | `effe` or `snd!` | |
| 0x138 | 4 | f32 | age heat recovery penalty | | heat loss x (1 - age x this) |
| 0x13C | 4 | f32 | age rate of fire penalty | | required ticks x (1 + age x this) |
| 0x140 | 4 | fraction | age misfire start | | misfires possible above this age |
| 0x144 | 4 | fraction | age misfire chance | | misfire chance at age 1 |
| 0x148 | 12 | | padding | | |
| 0x154 | 16 | tag reference | first person model | `mode` | |
| 0x164 | 16 | tag reference | first person animations | `antr` | its first-person weapon animations time draws, reloads and melee |
| 0x174 | 4 | | padding | | |
| 0x178 | 16 | tag reference | hud interface | `wphi` | hud.md |
| 0x188 | 16 | tag reference | pickup sound | `snd!` | played when ammunition is taken from a weapon of the same type |
| 0x198 | 16 | tag reference | zoom-in sound | `snd!` | |
| 0x1A8 | 16 | tag reference | zoom-out sound | `snd!` | |
| 0x1B4 | 4 | | padding | | |
| 0x1B8 | 12 | tag block | (unused block) | | empty on the disc |
| 0x1C4 | 4 | fraction | active camo ding | | camouflage lost per shot (0: 0.1) |
| 0x1C8 | 4 | f32 | active camo regrowth rate | per tick | camouflage regained per tick after a shot (0: 1/120) |
| 0x1CC | 12 | | padding | | |
| 0x1D8 | 2 | | padding | | |
| 0x1DA | 2 | enum16 | weapon type | | 0 undefined, 1 shotgun, 2 needler, 3 plasma pistol, 4 plasma rifle |
| 0x1DC | 12 | tag block | predicted resources | 8-byte elements | see below |
| 0x1E8 | 12 | tag block | magazines | 0x70-byte elements | at most 2 run |
| 0x1F4 | 12 | tag block | triggers | 0x114-byte elements | at most 2 run |

Weapon flags (bit: name, meaning):

| Bit | Name | Meaning |
|---|---|---|
| 0 | vertical heat display | HUD |
| 1 | mutually exclusive triggers | not read by the reference |
| 2 | attacks automatically on bump | AI |
| 3 | must be readied | the holder always has it in hand; pressing either trigger drops it; it cannot be picked up while a trigger is held (the CTF flag, the ball) |
| 4 | doesn't count toward maximum | not counted in the two-weapon limit |
| 5 | aim assists only when zoomed | no autoaim or magnetism unless zoomed |
| 6 | prevents grenade throwing | |
| 7 | must be picked up | game engine rules (multiplayer_rules.md) |
| 8 | holds triggers when dropped | not read by the reference |
| 9 | prevents melee attack | |
| 10 | detonates when dropped | the item detonates as soon as it has no parent |
| 11 | cannot fire at maximum age | a weapon at age 1 clicks empty |
| 12 | secondary trigger overrides grenades | the grenade button pulls trigger 1 instead of throwing |
| 13 | does not depower active camo | not read by the weapon code |
| 14 | enables integrated night vision | the holder's night vision works while zoomed |
| 15 | AI use weapon melee damage | a non-player's melee uses this weapon's melee damage |

Predicted resource element (8 bytes): 0x0 enum16 type (0 bitmap, 1
sound), 0x2 i16 resource index, 0x4 tag id. Loading hints only.

### Magazine element (0x70 bytes)

| Offset | Size | Type | Field | Unit | Meaning |
|---|---|---|---|---|---|
| 0x00 | 4 | flags32 | flags | | bit 0 wastes rounds when reloaded (loaded rounds are thrown away); bit 1 every round must be chambered |
| 0x04 | 2 | i16 | rounds recharged | rounds/s | added to the loaded rounds over time |
| 0x06 | 2 | i16 | rounds total initial | rounds | a new weapon's rounds (loaded + reserve) |
| 0x08 | 2 | i16 | rounds total maximum | rounds | reserve cap |
| 0x0A | 2 | i16 | rounds loaded maximum | rounds | magazine size |
| 0x0C | 8 | | padding | | |
| 0x14 | 4 | f32 | reload time | s | unused (the reload length is the first-person animation) |
| 0x18 | 2 | i16 | rounds reloaded | rounds | rounds moved into the magazine by one reload |
| 0x1A | 2 | | padding | | |
| 0x1C | 4 | f32 | chamber time | s | length of a chambering, x 30 truncated |
| 0x20 | 24 | | padding | | (unused interface fields; one map has junk here, ignore) |
| 0x38 | 16 | tag reference | reloading effect | `effe` or `snd!` | at reload start |
| 0x48 | 16 | tag reference | chambering effect | `effe` or `snd!` | at chamber start |
| 0x58 | 12 | | padding | | |
| 0x64 | 12 | tag block | magazines (ammunition objects) | 0x1C-byte elements | equipment that refills this magazine |

Ammunition object element (0x1C bytes): 0x00 i16 rounds (given by one
pickup), 0x02 10 bytes padding, 0x0C tag reference equipment (`eqip`).

### Trigger element (0x114 bytes)

| Offset | Size | Type | Field | Unit | Meaning |
|---|---|---|---|---|---|
| 0x000 | 4 | flags32 | flags | | see Trigger flags |
| 0x004 | 8 | real_bounds | rounds per second | rounds/s | rate at rate-of-fire 0 and at 1 |
| 0x00C | 4 | f32 | acceleration time | s | 0 to 1 rate-of-fire while held (runtime field used) |
| 0x010 | 4 | f32 | deceleration time | s | 1 to 0 when released (runtime field used) |
| 0x014 | 4 | fraction | blurred rate of fire | | above this the weapon shows its `~primary-blur` / `~secondary-blur` permutation |
| 0x018 | 8 | | padding | | |
| 0x020 | 2 | index16 | magazine | | magazine this trigger draws from; NONE: needs none |
| 0x022 | 2 | i16 | rounds per shot | rounds | |
| 0x024 | 2 | i16 | minimum rounds loaded | rounds | a fresh pull needs at least this many loaded |
| 0x026 | 2 | i16 | rounds between tracers | rounds | |
| 0x028 | 6 | | padding | | |
| 0x02E | 2 | enum16 | firing noise | | AI hearing: 0 silent, 1 medium, 2 loud |
| 0x030 | 8 | real_bounds | error | | unused by the reference (see Quirks) |
| 0x038 | 4 | f32 | error acceleration time | s | error 0 to 1 while held (runtime field used) |
| 0x03C | 4 | f32 | error deceleration time | s | error 1 to 0 when released (runtime field used) |
| 0x040 | 8 | | padding | | |
| 0x048 | 4 | f32 | charging time | s | x 30 truncated: ticks to full charge |
| 0x04C | 4 | f32 | charged time | s | x 30 truncated: ticks a full charge can be held |
| 0x050 | 2 | enum16 | overcharged action | | after the charged time: 0 none, 1 explodes, 2 discharges |
| 0x052 | 2 | | padding | | |
| 0x054 | 4 | fraction | charged illumination | | glow while charging, x charged fraction |
| 0x058 | 4 | f32 | spew time | s | discharge length after a charge (0: single release) |
| 0x05C | 16 | tag reference | charging effect | `effe` or `snd!` | |
| 0x06C | 2 | enum16 | distribution function | | 0 point, 1 horizontal fan |
| 0x06E | 2 | i16 | projectiles per shot | | |
| 0x070 | 4 | angle | distribution angle | rad | fan step |
| 0x074 | 4 | | padding | | |
| 0x078 | 4 | angle | minimum error | rad | lower limit of the random angle |
| 0x07C | 8 | angle_bounds | error angle | rad | angle limit at error 0 and at error 1 |
| 0x084 | 12 | real_point3d | first person offset | WU | forward, left, up from the shot origin, players only |
| 0x090 | 4 | | padding | | |
| 0x094 | 16 | tag reference | projectile | `proj` (any object) | what one round creates |
| 0x0A4 | 4 | f32 | ejection port recovery time | s | export function only |
| 0x0A8 | 4 | f32 | illumination recovery time | s | muzzle glow fade |
| 0x0AC | 12 | | padding | | |
| 0x0B8 | 4 | fraction | heat generated per round | | |
| 0x0BC | 4 | fraction | age generated per round | | |
| 0x0C0 | 4 | | padding | | |
| 0x0C4 | 4 | f32 | overload time | s | x 30 truncated |
| 0x0C8 | 40 | | padding | | |
| 0x0F0 | 4 | f32 | (runtime) illumination recovery | per tick | 1 / (30 x illumination recovery time), 1 if 0 |
| 0x0F4 | 4 | f32 | (runtime) ejection port recovery | per tick | same from 0x0A4 |
| 0x0F8 | 4 | f32 | (runtime) rate of fire acceleration | per tick | same from 0x00C |
| 0x0FC | 4 | f32 | (runtime) rate of fire deceleration | per tick | same from 0x010 |
| 0x100 | 4 | f32 | (runtime) error acceleration | per tick | same from 0x038 |
| 0x104 | 4 | f32 | (runtime) error deceleration | per tick | same from 0x03C |
| 0x108 | 12 | tag block | firing effects | 0x84-byte elements | |

Trigger flags:

| Bit | Name | Meaning |
|---|---|---|
| 0 | tracks fired projectile | after a shot the trigger waits while held and the tracked object lives |
| 1 | random firing effects | firing effect entries chosen at random |
| 2 | can fire with partial ammo | fires even with fewer than rounds per shot loaded |
| 3 | does not repeat automatically | semi-automatic for a player: the trigger must be released between shots |
| 4 | locks in on/off state | each press toggles the trigger held |
| 5 | projectiles use weapon origin | shots start at the weapon's trigger marker(s), one set per marker |
| 6 | sticks when dropped | a dropped weapon keeps this trigger held |
| 7 | ejects during chamber | ejection port function runs at chambering, not firing |
| 8 | discharging spews | not read by the reference (spew time decides) |
| 9 | analog rate of fire | the analog pull sets the rate of fire and the error |
| 10 | use error when unzoomed | no spread at all while zoomed |
| 11 | projectile vector cannot be adjusted | shots use the trigger marker's own position and direction: no camera correction, no aim assist, no first-person offset |
| 12 | projectiles have identical error | every projectile of a shot takes the first one's direction |
| 13 | projectile is client-side only | networking; not read by the weapon code |

Firing effect element (0x84 bytes): 0x00 short_bounds shot count
(lower, upper), 0x04 32 bytes padding, 0x24 firing effect, 0x34 misfire
effect, 0x44 empty effect (tag references, `effe` or `snd!`), 0x54
firing damage, 0x64 misfire damage, 0x74 empty damage (tag references,
`jpt!`).

## What is on the disc

All 13 multiplayer maps hold the same 15 `weap` tags with identical
field values (checked: every field of every tag and block, every
reference's class and path; the first-person animation frame counts
too). Tag order and ids differ per map. By kind:

| Kind | Tags |
|---|---|
| Held weapons, in the globals' multiplayer weapon list | `weapons\assault rifle\assault rifle`, `weapons\pistol\pistol`, `weapons\shotgun\shotgun`, `weapons\plasma rifle\plasma rifle`, `weapons\sniper rifle\sniper rifle`, `weapons\plasma pistol\plasma pistol`, `weapons\needler\needler`, `weapons\rocket launcher\rocket launcher`, `weapons\flamethrower\flamethrower` |
| Unused test weapon (in the list, no models or effects) | `weapons\gravity rifle\gravity rifle` |
| Game-type objects (must be readied, don't count) | `weapons\flag\flag`, `weapons\ball\ball` |
| Vehicle turrets (vehicles.md) | `vehicles\warthog\warthog gun`, `vehicles\ghost\ghost gun`, `vehicles\scorpion\scorpion cannon` |

The globals' multiplayer weapon list (globals +0x14C) orders them:
assault rifle, flamethrower, gravity rifle, needler, pistol, plasma
pistol, plasma rifle, rocket launcher, shotgun, sniper rifle, ball,
flag, then the two grenade equipment tags.

## Behaviour

### 1. Runtime state of a weapon

Per weapon: heat, age (0..1, reals), an "overheated" flag and an
"overheat exit" flag (first-person only), an "overheat recoil" flag
(plasma pistol), a "needs to reload" flag, a weapon state (idle,
primary/secondary recoil, primary/secondary chamber,
primary/secondary reload, primary/secondary charged, ready, put away)
with a **state timer** in ticks, the control inputs the holder passed
this tick, the "overcharged" amount, the integrated light power, the
tracked object, the alternate shots loaded and the tick it last fired.

Per trigger: an idle counter (ticks, 0..127), a state (idle,
overloading, charging, charged, recovering, tracking, spewing, locked,
uninitialised) with a timer, a "released since last shot" flag, a
"fired before charging" flag, the rate of fire and the error (both
0..1), the ejection port position and the illumination (both 0..1),
the firing-effect cursor (index, shots remaining, a used-entries bit
set) and a non-tracer round counter.

Per magazine: a state (idle, reloading, unchambered, chambering) with a
timer, rounds loaded, rounds total (reserve), a fractional recharge
counter.

A new weapon: weapon state idle; per magazine rounds loaded =
min(rounds total initial, rounds loaded maximum), rounds total =
rounds total initial - rounds loaded; per trigger idle counter 127,
state idle, everything else 0; heat 0, age 0. (Game types may then set
other amounts: multiplayer_rules.md.)

**The weapon's own animation graph.** The weapon object's world
animation graph (object part, objects.md) maps weapon states to
animations by fixed index: idle 0, ready 1, put away 2, reload 3 (both
magazines), chamber primary 5 and secondary 6, charged 8 (both
triggers), recoil primary 9 and secondary 10. When the weapon is told
to enter a state, the state is **only recorded if the graph has an
animation for it** (or
the state is idle); a weapon with no graph never leaves idle. The
animation plays one frame per tick; when it reaches its last frame
(frame index + 1 = frame count) the state returns to idle, except the
charged and put-away states, which stay. On the disc only three held
weapons have such animations: pistol and sniper rifle (recoil, 5
frames), shotgun (chamber, 22 frames; a secondary-reload animation that
is never entered). This matters because a magazine can only start a
reload or a chamber while the weapon state is idle (section 11).
Entering a state other than through a forced change is refused unless
the current state is idle, or is a recoil and the new state's number is
not lower.

### 2. Drawing, putting away and switching

- **Draw (ready).** When a weapon is put in hand: its triggers become
  "uninitialised" with timer 0 (idle again on its next update), any
  reload in progress is settled (section 11, interruptions), the weapon
  state is forced to ready, the ready effect plays, and the weapon's
  state timer is set to the frame count of its **first-person "ready"
  animation**. Each update first lowers the state timer by one; while
  it is above 0 the triggers read as released. So with the draw on tick
  R (the weapon's update runs on R too) the first shot can be on tick
  R + F - 1 for a ready animation of F frames: assault rifle 28,
  pistol 34, shotgun 22, plasma rifle 28, sniper 28, needler 22, rocket
  launcher 21, flamethrower 29 ticks. The tag's ready time is not used.
- **Put away** is instant in a switch: the old weapon is put away and
  the new one readied in the same tick. Putting away clears the
  weapon's control inputs, settles any reload as above, sets the
  triggers uninitialised, stops the overheated effect, and hides the
  weapon in the holder's inventory. A weapon "not busy" (both triggers
  idle, both magazines idle, weapon state idle) may be put away without
  forcing; a switch always forces.
- **Switching.** The switch button moves the desired weapon to the next
  slot (in slot order, wrapping) holding a weapon the unit can use, and
  unzooms. The unit acts on a desired weapon that differs from the
  current one in its own update, provided its animation is not busy
  (busy: melee, grenade throw, being hit hard, entering or leaving a
  seat, dying and similar), and unzooms. When nothing is chosen the
  unit picks the usable weapon used most recently. A weapon flagged
  "must be readied" in the inventory is always the desired one.
- While the holder's chosen weapon is not the one in hand (a switch
  pending), the weapon in hand gets no trigger inputs, only the
  "switching" input.
- Weapons in the inventory but not in hand are still updated every
  tick (heat drains, rate of fire and error decay), but get no inputs
  and never start a reload on their own.

### 3. Inventory, pickup, swap and drop

- A unit has 4 slots. Weapons that count toward the maximum (all but
  flag and ball) are limited to **two** by the pickup rules.
- **Auto pickup.** Walking onto a weapon picks it up at once when the
  unit is allowed (it does not already hold one of that tag) and the
  weapon does not count toward the maximum, or the unit holds no
  weapon, or the game type forces it (the CTF flag). The new weapon
  becomes the desired one (it is drawn). Picking up unzooms.
  (Reference note: the reference also auto-picks a second weapon into
  an empty slot in multiplayer; it marks that as its own change from
  the Xbox game, so the Xbox rule is the one above.)
- **Swap.** Otherwise, if a swap is allowed, the weapon is offered: as
  "add to inventory" when the unit holds exactly one weapon of another
  tag, else as "swap". The swap happens when the **action button has
  been held for 7 input frames**, once per hold. A swap drops the
  weapon in hand and adds the new one, which is drawn. A swap is not
  allowed while the weapon in hand does not count toward the maximum
  (flag, ball) and the new one does. A swap for a weapon of the same
  tag is allowed only for the weapon in hand, only when it has been
  used (age > 0) and the one on the ground has a lower age (a fresher
  plasma weapon).
- A weapon flagged "must be readied" is not picked up while either
  trigger is held.
- A short press of the action button that finds nothing to do (no
  pickup, vehicle or device) is a **reload** request.
- **Ammunition from weapons.** Walking over a weapon of a tag the unit
  holds moves rounds from the ground weapon's reserve into the held
  weapon's reserve, up to its reserve maximum, per magazine; the ground
  weapon is deleted when its reserve reaches 0 that way. Its loaded
  rounds are not taken. The weapon's pickup sound plays for a local
  player. Equipment named in a magazine's ammunition objects gives
  min(its rounds, rounds needed) and is deleted. (The trigger for
  these pickups is multiplayer_rules.md's.)
- **Drop.** A dropped weapon leaves from the unit's "left hand" marker
  if it was not attached, with velocity = a direction drawn in a cone of
  pi/8 around the aim times a speed drawn in 0.0266667..0.04 WU/tick,
  plus the dropper's velocity; it ignores the dropper for collision.
  In a multiplayer game a dropped weapon that cannot be fired (age 1,
  or both its loaded rounds and its reserve 0 in a magazine of nonzero
  size) is deleted. A unit that dies drops every weapon (the one in
  hand last). Pressing a trigger while holding a must-be-readied weapon
  drops it.

### 4. The trigger

**Inputs.** The holder passes each tick: primary trigger, secondary
trigger (the grenade button), reload, zoomed, "user busy" and
"switching". User busy is set while the holder's animation is busy and
while a player melee is running (section 13). A trigger reads as
**down** only if the input is held, user busy is clear and the weapon's
state timer is 0 (after this tick's decrement). Trigger 1 is driven by
the grenade button only when the weapon has "secondary trigger
overrides grenades" (no held multiplayer weapon has it). Secondary mode
1 (slaved) lets trigger 1 count as down only while trigger 0's rate of
fire is exactly 1; mode 2 (inhibits) releases trigger 0 while trigger 1
is down. A "locks in on/off" trigger toggles a latched state on each
press and reads as that state. A "sticks when dropped" trigger reads
down while the weapon has no holder.

Whenever a trigger reads as not down this tick, its "released since
last shot" flag is set.

**State machine (per tick, per trigger, after the timer decrement).**

- *Idle:* first the automatic reload check (section 11). Then, if the
  trigger is down and **can fire again** (below), firing begins;
  otherwise the idle counter goes up by one, to at most 127.
- *Charging:* if the timer is still above 0 and the trigger is not
  down: a weapon with two triggers that did not fire on the press fires
  trigger 0 now (forced); otherwise the trigger goes idle. Either way
  the charging effect stops. When the timer reaches 0 (held through),
  the trigger becomes charged (section 10).
- *Charged:* section 10.
- *Spewing:* fires (forced) every tick while the timer is above 0, then
  recovers.
- *Overloading:* if released, fires (forced). If still held when the
  timer is 0 and fewer than the maximum alternate shots are loaded, the
  next trigger fires and the overload time restarts.
- *Tracking:* when released or the tracked object is gone, recovers.
- *Locked:* goes idle when released.
- *Uninitialised:* goes idle when its timer is 0.
- *Recovering:* no code path enters it in the reference (recovering
  goes straight to idle, below).

"Recover" means: idle counter = 0, state idle.

**Can fire again.** With f the trigger's rate of fire (or the analog
pull for an analog trigger):

rate = lower + (upper - lower) f

required = 30 / rate if rate > 0.0001, else 0

required = required x (1 + age x age rate of fire penalty), if that
penalty > 0

The trigger may fire when idle counter + 1 >= required. In addition, a
"does not repeat automatically" trigger on a player's weapon may not
fire until it has been released since its last shot. Since the idle
counter is 0 on the tick after a shot and rises by one per tick, a
held trigger fires every **k = max(1, ceil(required))** ticks (exactly
whole quotients fire on that tick: 30 / 15 = 2 gives 2). The idle
counter does not rise on a tick where firing was attempted but the
magazine or overheat refused it.

**Begin firing.** Refused (silently, no effect) if the trigger's
magazine is not idle (reloading, unchambered or chambering), if the
weapon is overheated, or if the weapon is under water. Otherwise:
- a trigger with a charging time, when not forced: at maximum age with
  "cannot fire at maximum age", fire (it will click). Otherwise, with
  two triggers, start the charging effect; with one trigger and a rate
  of fire above 0, fire now and remember "fired before charging"; then
  enter charging with timer = truncate(30 x charging time);
- a trigger with an overload time, when not forced: enter overloading
  with timer = truncate(30 x overload time);
- otherwise fire (section 5).

**Rate of fire and error, after the state machine, every tick:**

- if down: rate of fire += runtime acceleration, clamp to 1; above the
  blurred rate (if nonzero) the weapon switches to its blur
  permutation once;
- if not down: rate of fire -= runtime deceleration, clamp to 0; back
  below the blurred rate the blur permutation is removed;
- if the trigger state is spewing or recovering, or it is down: error
  += runtime error acceleration, clamp to 1;
- otherwise: error -= runtime error deceleration, clamp to 0.

Note the error grows while the trigger is held even when nothing fires
(empty, reloading, overheated).

Ejection port position and illumination fall by their runtime rates
each tick (before the state machine), clamped to 0.

### 5. Firing a round

When a trigger fires:

1. **Ammunition.** If the trigger names a magazine: if loading
   alternate ammunition (secondary modes 3 and 4) and the alternate
   shots are already at maximum, nothing is fired. Otherwise the round
   fires when (rounds loaded >= rounds per shot, or "partial ammo") and
   not ("cannot fire at maximum age" and age >= 1) and (rounds loaded >=
   minimum rounds loaded, or the trigger has not been released since its
   last shot). Then rounds loaded -= rounds per shot; if that is <= 0 it
   becomes 0; otherwise, with "every round must be chambered", the
   magazine becomes unchambered. A trigger with no magazine always
   fires. (The plasma weapons point at a magazine of all zeros: 0 >= 0,
   so they always fire.)
2. **Firing effect choice.** If the trigger has firing effects and the
   current entry's shots are used up: start from the current entry
   index (or a random integer draw modulo the entry count, with "random
   firing effects"); repeatedly step to the next index (wrapping) that
   is not marked used (when all are marked, clear the marks first),
   mark it, and draw its shot count as an integer range draw (lower,
   upper); stop when the count is above 0 or the start index is
   reached. Then shots remaining -= 1.
3. **Misfire.** If 0 < age misfire start < 1 and age > start: chance =
   (age - start) x misfire chance / (1 - start), doubled while spewing;
   a real draw below it misfires.
4. Effect selection: not fired: the entry's empty effect and empty
   damage, scale 1; misfired: the misfire effect and damage, scale =
   rate of fire; fired: the firing effect and damage, scale = rate of
   fire, error = heat / overheated threshold (0 if the threshold is 0).
5. If fired: the game engine is told the player fired (camouflage,
   section 17); the last-fired tick is stored; the first-person weapon
   is told (fire or misfire); the ejection port and illumination are
   set to 1 (if their recovery times are nonzero); **heat** += heat per
   round, then clamped (player: to 1; AI: to its overheated
   threshold); a player's weapon's **age** += age per round, clamped
   to 1; the weapon enters the recoil state for this trigger; unless
   misfired, the projectiles are created (section 6) and the AI hears
   the firing noise (alternate-ammunition modes instead count one more
   alternate shot loaded); the chosen damage effect is applied to the
   holder (direction = minus the aim, at the holder's centre: the
   per-shot kick, damage effects are projectiles.md's); a plasma
   pistol's trigger 1 sets the "overheat recoil" flag.
6. **Detonation.** If heat > heat detonation threshold and a real draw
   is below the detonation fraction, the weapon plays its detonation
   effect and is deleted. (No multiplayer weapon can reach this: heat
   is clamped to 1 and the thresholds are 1 or 0 with fraction 0.)
7. Trigger state: not fired: **locked** until released (so an empty
   gun clicks once per pull); fired and not spewing (or misfired):
   tracking if the trigger tracks its projectile, else recover.
8. The "released since last shot" flag is cleared, and the chosen
   effect is created on the weapon (on its holder when the weapon is
   invisible), as a sound if the reference is a sound.

### 6. Where the shot starts and where it goes

For each projectile of a player's shot:

1. Origin = the weapon's "primary trigger" marker (trigger 1: "secondary
   trigger") on the weapon's world model in the holder's hand; forward
   = that marker's forward. A trigger with "projectiles use weapon
   origin" makes one set of projectiles per marker of that name.
2. Unless the trigger has "projectile vector cannot be adjusted" or the
   holder is dead: direction = the holder's **aiming vector**. A unit
   flagged "fires from camera" (the multiplayer Spartan is) moves the
   origin onto the camera ray: origin = C + d ((M - C) . d), C the
   camera position, M the marker, d the direction. The inherited speed
   is v = d . (holder's velocity), in WU per tick.
3. First-person offset (players only): with L = normalise(Z x d) (the
   left vector; if d is vertical, L = +Y) and U = d x L, origin += x d +
   y L + z U, (x, y, z) the trigger's first person offset. The rocket
   launcher's (0, -0.1, 0) starts rockets 0.1 WU to the right.
4. Aim correction (section 15), which also applies with aim assist
   switched off whenever the weapon has aim assist parameters, since it
   corrects convergence: the shot is turned toward the point the
   camera looks at, within the deviation cone.
5. With "projectiles use weapon origin" the origin returns to the
   marker's position (the direction keeps the steps above).
6. Error (section 7) turns the direction.
7. The projectile is created at the origin with direction d and
   initial velocity d x v (the projectile adds its own speed:
   projectiles.md); a player's projectile is never auto-deleted and is
   first pushed inside the level from the camera position. Its target
   is the aim assist target if there was one. **Tracers:** a round is a
   tracer when the trigger's rate of fire is 0 (the first shot from
   rest) or the non-tracer counter (incremented per projectile) has
   reached rounds between tracers, which resets it; the assault rifle
   (3) shows every fourth round after the first. A non-tracer round has
   its tracer removed (projectiles.md).

An AI's shot replaces steps 3-4 with the actor's own aiming, which also
supplies the error angle.

### 7. Error and spread

**Error angle.** For the first projectile of a shot:

e = the trigger's error (or, for an analog trigger, the pull)

theta_max = (1 - e) x error angle lower + e x error angle upper

The error value used is the one left by the previous tick's update, so
the first shot from rest uses e = 0.

**Cone.** Unless the trigger has "use error when unzoomed" and the
holder is zoomed (then the shot is perfectly straight), the direction
d is replaced by a random vector in the cone (minimum error, theta_max):
draw an integer range (0, 1026) and take that table direction t; axis =
normalise(d x t); if the axis has length 0 nothing else is drawn and d
is unchanged; otherwise draw theta = real range (minimum error,
theta_max) and rotate d by theta about the axis (toward t). So the
**tilt angle is uniform** between the bounds and the tilt's direction
around the aim follows the table direction (close to uniform). Per
unit of solid angle the density is about 1 / sin(theta): shots
cluster toward the centre.

**Several projectiles** (the shotgun's 15 pellets): each projectile
draws its own cone vector with the same theta_max. With "projectiles
have identical error" every projectile after the first is given the
first one's direction (the draws still happen). Then a horizontal fan
(distribution 1) turns projectile i about the shot's up vector by
offset(i) x distribution angle, where for an odd count n the offsets of
i = 0, 1, 2, 3, 4, 5, 6... are 0, 0, 0, -1, 1, -2, 2... and for an
even count -0.5, 0.5, 0.5, -0.5, 1.5, -1.5... (this is what the
reference computes, duplicates included; no multiplayer weapon uses a
fan).

**The direction table** (built once at start; the order and the
positions below reproduce the original's table, so the same seed gives
the same tilt direction). A draw takes entry floor(1026 x r / 65536),
r the 16-bit value of the draw. Construction, with 16 segments:

1. Entries 0 to 5 are the corners: +Z, +Y, +X, -Y, -X, -Z.
2. The eight faces are taken in this order, each as three corners
   (a, b, c): (0, 1, 2), (0, 2, 3), (0, 3, 4), (0, 4, 1), (5, 1, 4),
   (5, 4, 3), (5, 3, 2), (5, 2, 1) (corner numbers as in step 1).
3. Within a face, the grid points are (row, col) with 0 <= col <= row
   <= 16: col = 0 is on the edge a to c (point number row counted from
   a), col = row on the edge a to b (point number row from a), row = 16
   on the edge c to b (point number col from c); the others are
   interior. The face is walked for row = 1 to 16 and col = 1 to row,
   and each step visits the points (row - 1, col - 1), (row, col - 1),
   (row, col) and, if col < row, (row - 1, col), in that order. A grid
   point gets its entries the first time it is visited.
4. An edge point makes the **whole edge** at once, unless an earlier
   face has made it (edges are shared by two faces; the corners
   already exist): the 15 inner points of the edge, in order from the
   edge's lower-numbered corner to its higher-numbered one, each the
   normalised blend (1 - k/16) x lower + (k/16) x higher for k = 1 to 15.
   An edge point's entry is then the one for its number counted from a
   or c (so on an edge whose a-side corner has the higher number the
   count runs backward through the stored 15).
5. An interior point (row, col) takes the next free entry, the blend of
   the face's points (row) on the edge a to c and (row) on the edge a to
   b with weight col / row on the second: normalised ((1 - col/row) x
   P_ac + (col/row) x P_ab). (The edge points it needs already exist
   by then: row 1 makes both edges.)

Arithmetic is in single precision; a table that differs in the last
bit is as good. Checks: 1026 entries, all of unit length and distinct;
entry 6 = (0.0665190, 0, 0.9977852), entry 7 = (0.1414214, 0, 0.9899495),
entry 21 = (0, 0.0665190, 0.9977852), entry 28 = (0, 0.7071068,
0.7071068), entry 100 = (0.1825742, 0.9128709, 0.3651484), entry 500 =
(-0.8741572, 0.3885143, 0.2913858), entry 1025 = (0.9949367, 0.0710669,
-0.0710669). Edge points are blends of the corners, not equal angles
apart, and interior points blend edge points, so the table is not a
face subdivided flat and pushed out.

**Random draws per shot, in order** (game-state generator):
(1) firing-effect choice, when an entry runs out: one integer draw if
"random firing effects", then one integer range draw per entry visited
(every multiplayer weapon: one draw per shot, its count 1..1);
(2) misfire: one real draw if age is above the misfire start;
(3) per projectile, if the cone applies: one integer range draw and,
unless the axis is degenerate, one real range draw;
(4) detonation: one real draw if heat is above the detonation
threshold. Effects, damage effects and projectiles created along the
way may draw from the same generator (their specs say).

**Error over time.** While the trigger is held the error rises by
1 / (30 x error acceleration time) per tick (1 per tick if that time is
0) and when released falls by 1 / (30 x error deceleration time) (all
at once if 0). Examples (test cases have more):
- assault rifle: +1/18 per tick held, -1/30 per tick released: 2 deg
  for the first shot, +0.5 deg per shot (2 ticks), 6.5 deg from the
  10th shot on (tick 18); back to 2 deg 30 ticks after release.
- pistol: +1/15 held, -1/6 released: shot 1 0.2 deg, shot 2 (tick 9)
  1.28 deg, shot 3 on 2 deg; a pause of 6 ticks restores 0.2 deg.
- plasma rifle: +1/75 held, back to 0 the first tick released: 0.5 deg
  rising to 5 deg after 76 updates (the 75th leaves 0.99999934 in
  single precision, the 76th clamps to 1.0; the angle limit differs
  from 5 deg by 3e-6 deg until then).
- shotgun and sniper rifle: lower = upper, so fixed (10 deg per pellet,
  0.5 deg unzoomed and 0 zoomed respectively).

### 8. Heat and overheating

Every tick, if heat > 0 (in this order):
1. if heat >= overheated threshold and not overheated: overheated set;
   the first-person weapon plays its overheating animation (the plasma
   pistol's "super" one if its overheat recoil flag was set, which is
   then cleared); the overheated effect starts looping.
2. if the weapon's overcharged amount (left from the previous tick,
   section 10) is 0: loss = heat loss per second x (1/30), times (1 -
   age x age heat recovery penalty) if that penalty > 0; heat -= loss,
   clamped at 0. If overheated and (heat - recovery threshold) / loss
   <= 1, the "overheat exit" flag is set (first-person exit animation
   only).
3. if overheated and heat < heat recovery threshold: overheated and
   overheat exit are cleared and the overheated effect stops.

While overheated, firing cannot begin. Cooling time from 1 to below the
threshold: plasma rifle (0.01 per tick, less with age) 76-77 ticks;
plasma pistol (0.0216667 per tick) 35 ticks. The flamethrower also has
heat (0.04125 per round, loss 0.475/s, recovery below 0.9).

### 9. The battery (age)

Age is 0 for a full battery and 1 for an empty one (the HUD shows 1 -
age). A player's shot adds age per round (plasma rifle 0.005: 200
shots; plasma pistol 0.002 a bolt, 0.11 a charged bolt). Age:
- slows the rate of fire: required ticks x (1 + 0.2 age) (plasma rifle
  only; see Quirks for what this does to its top rate);
- slows cooling: heat loss x (1 - 0.2 age) (plasma rifle only);
- above 0.9 causes misfires with chance (age - 0.9) x 0.5 / 0.1 per
  round, 0.5 at age 1 (both plasma weapons). A misfire uses heat and
  battery like a shot but creates no projectile, plays the misfire
  effect and animation and applies the misfire damage effect;
- at 1, with "cannot fire at maximum age", every pull clicks empty.

### 10. Charging and the plasma pistol

The plasma pistol has two triggers sharing nothing but the weapon:
trigger 0 (normal bolt, rate limits 0, so no rate limit, charging time
0.6 s = 18 ticks, charged time 45 s = 1350 ticks, overcharged action
**none**) and trigger 1 (the charged bolt, `weapons\plasma
rifle\charged bolt`, heat 1.0 and age 0.11 per round, "can fire with
partial ammo"; charging and charged times 0, overcharged action
"explodes", which can never run because this trigger never enters
charging). Only trigger 0 is wired to the fire button. (Disc, checked
by script on bloodgulch, hangemhigh, damnation and sidewinder: the
overcharged action is 0 on trigger 0 and 1 on trigger 1.)

- **Press** (tick P): trigger 0 begins firing, but because the weapon
  has two triggers nothing fires; it enters charging with 18 ticks.
- **Release before the charge completes** (tick R, P < R < P + 18):
  trigger 0 fires a normal bolt **on tick R** and recovers. So the
  plasma pistol fires on release; the fastest tap (press one tick,
  release the next) fires every 2 ticks (15 per second).
- **Held through tick P + 18:** the timer reaches 0, the trigger
  becomes charged with 1350 ticks, the weapon enters its charged state
  and the first-person weapon shows the charge. While charged and held,
  the weapon's overcharged amount = 1 - (timer / 30) / 45, which stops
  cooling on the following ticks; melee is impossible while charging or
  charged. The amount reaches exactly 1.0 on tick P + 18 + 1350 (0.9992592
  one tick before) and stays 1.0 while the button is held.
- **Release while charged** (tick R): trigger 1 fires the charged bolt
  on tick R, trigger 0 recovers and its rate of fire becomes 0. Heat
  becomes 1, so the weapon overheats on tick R + 1 (with the super
  recoil animation) and can fire again on tick R + 35.
- **Held for the whole charged time:** when the timer reaches 0 the
  trigger's overcharged action runs, every tick, while the button stays
  down. The plasma pistol's trigger 0 has action "none", so **nothing
  happens**: the weapon stays charged for as long as the button is held
  (overcharged amount 1.0, no cooling, heat unchanged), and releasing it
  at any later tick fires the charged bolt as above. ("Explodes" would
  play the weapon's detonation effect, none on the disc, and delete the
  weapon; "discharges" would release the charge as above; no
  multiplayer weapon has either on a trigger that charges.)
- A weapon with one trigger and a charging time fires on the press if
  its rate of fire is above 0 and charges in the background; releasing
  early then just returns to idle (no such multiplayer weapon).
- Spew (spew time > 0): after a release the trigger fires every tick
  for the spew time. No multiplayer weapon spews.

### 11. Magazines and reloading

**Per tick, per magazine** (before the triggers): recharge (rounds
recharged per second r > 0 and not full: rounds loaded += floor(r /
30), the fractional counter += r mod 30, and at 30 one more round, the
counter -30; clamp to the magazine size); timer -= 1 if above 0; then
by state:
- reloading: if timer - 1 <= 0, finish the reload;
- unchambered: start chambering (if allowed);
- chambering: if the timer is 0, idle.

**Starting a reload** (magazine 0 by the reload input; a trigger's
magazine by the automatic check): only from idle or unchambered, and
only when both triggers are idle and the weapon state is idle. Then,
if reserve > 0 and the magazine is not full: the weapon enters its
reload state, the reloading effect plays, the first-person weapon plays
its reload, which **unzooms**, and the magazine becomes reloading with
timer = the frame count of the first-person **"reload-empty"**
animation (always that one, empty or not; the tag's reload time is not
used). Whenever the start was allowed the "needs to reload" flag is
cleared, reload or not.

- The reload input sets "needs to reload"; the weapon retries the start
  every tick until it is allowed (for example after the pistol's
  5-frame recoil animation).
- **Automatic reload:** a held weapon's idle trigger, when the holder is
  not busy, starts a reload of its magazine if rounds loaded < rounds
  per shot (without partial ammo), or < minimum rounds loaded, or 0.
  It runs every tick, with the trigger held or not.

**Finishing a reload:** if the magazine wastes rounds, rounds loaded =
0. Then n = min(rounds reloaded, reserve), loaded' = min(loaded + n,
magazine size), and (for a player's weapon) reserve -= loaded' -
loaded. The magazine becomes unchambered with timer 0. Then, if
reserve > 0, the magazine is not full, it does not waste rounds and the
holder is pressing neither trigger nor switching, a new reload starts
at once (the shotgun's shell-by-shell loop; the other weapons reload
everything at once and stop).

**Chambering** (from unchambered, only when both triggers and the
weapon state are idle): the weapon enters its chamber state (the
shotgun's 22-frame chamber animation runs), the chambering effect
plays, the magazine becomes chambering with timer = truncate(30 x
chamber time). A trigger can fire only from an idle magazine, so after
any reload a weapon needs two more ticks (unchambered, chambering with
timer 0) before it can fire.

**Timeline from empty** (last round fired on tick 0, trigger released,
F = reload-empty frames): reload starts on tick 1 (pistol and sniper:
tick 4, after their 5-frame recoil animation), rounds go in on start +
F - 1, the weapon can fire on start + F + 1.

| Weapon | F | Start | Rounds in | First shot |
|---|---|---|---|---|
| assault rifle | 87 | 1 | 87 | 89 |
| pistol | 67 | 4 | 70 | 72 |
| sniper rifle | 94 | 4 | 97 | 99 |
| needler | 70 | 1 | 70 | 72 |
| rocket launcher | 125 | 1 | 125 | 127 |
| flamethrower | 104 | 1 | 104 | 106 |

**Shotgun (weapon type shotgun):** rounds reloaded 1, so one shell per
reload. A reload started by the input or the automatic check (the
first shell) lasts the **"enter"** animation's 15 frames; a reload
started by the loop lasts the "reload-empty" animation's 12 frames.
From empty: first shell on tick 15, then one every 11 ticks, the 12th
on tick 136. Holding the trigger stops the loop after the shell in
progress: that shell goes in, then the shotgun chambers and fires 2
ticks later if its rate allows. Because a shotgun round must be
chambered and its chamber animation lasts 22 frames, a reload cannot
start until 22 ticks after a shot that left shells in the magazine.

**Interruptions.** Switching weapons, throwing a grenade and starting a
melee settle a reload in progress: with T the reload timer and F the
reload-empty frame count, if 2T < F (more than half done) the reload
finishes at once (rounds added); otherwise it is abandoned (no rounds).
Then all magazines go idle and the triggers uninitialised.

### 12. Zoom

- Each press of the zoom button (the first tick it is down) steps the
  player's zoom level: none, 0, 1, ..., levels - 1, none. A weapon with
  no levels stays at none. While magazine 0 is reloading the press does
  nothing.
- Magnification of level i (n levels, lower m0, upper m1; each 1 if
  not above 0): m = m0 (m1 / m0)^(i / (n - 1)) for n > 1, m0 for n =
  1. Pistol and rocket launcher: 2x. Sniper rifle: 2x then 8x.
- Field of view while zoomed = base field of view / m (the angle
  itself, not its tangent), used only if the result lies between pi/100
  and 99 pi/100. Look input is divided by m while zoomed.
- **What that angle is.** The unit tag's 70 degrees (and the zoomed
  angle derived from it) is a nominal angle *measured across a 4:3
  picture and then shrunk*: the renderer turns it into the camera's
  **vertical** field of view by tan(v / 2) = 0.75 x 0.85 x tan(a / 2),
  with a the nominal angle (0.75 is the 3:4 height to width ratio, 0.85
  a fixed narrowing of the tangent; both checked as constants in the
  disc's executable, the 0.85 in a small routine that multiplies a
  half-angle tangent by it). The horizontal extent is the vertical one
  times the viewport's width over height, whatever the viewport. So
  with a = 70 degrees: vertical 48.11 degrees (0.83968 rad); at 4:3
  horizontal 2 atan(0.85 tan 35 degrees) = 61.52 degrees (not 70); at
  16:9 horizontal 76.87 degrees. Zoomed pistol or rocket launcher (a =
  35 degrees): vertical 22.73 degrees (0.39672 rad); sniper 8x (a =
  8.75 degrees): vertical 5.58 degrees (0.09747 rad). The HUD's screen
  effect compares this vertical angle (`hud.md`).
- The zoom-in sound plays with scale i / (n - 1) (1 for one level), the
  zoom-out sound when leaving zoom.
- Zoom is cancelled by: starting a reload, switching weapons, picking
  up a weapon, throwing a grenade, taking any shield or body damage,
  entering a vehicle. Leaving zoom also turns night vision off.
- Zoomed: "use error when unzoomed" triggers fire with no spread (the
  sniper rifle; the pistol has no such flag and is equally accurate
  zoomed or not); aim assist angles are divided by m and ranges
  multiplied by m; a weapon with "aim assists only when zoomed" has aim
  assist only while zoomed.
- A player cannot start a melee while zoomed.
- Movement is not changed by zoom in the reference (open question 7).

### 13. Melee

A player's melee (a biped with a player; the melee button):

- Starts when no melee is running, the melee button is held (it
  repeats while held), the weapon in hand does not prevent melee (flag
  9, or charging/charged) and the player is **not zoomed**. It settles
  any reload (section 11), starts the holder's melee action animation
  and the first-person melee animation.
- Timing: F = frame count and K = key frame of the weapon's first-
  person "melee" animation; s = floor(F / 4); the melee counter starts
  at F - s and the hit tick is when the counter equals F - K - s.
  The counter counts down one per tick from the tick after the press.
  With the press on tick 0: **damage on tick K + 1**; the counter
  reaches 0 at the end of tick F - s; the next melee (and firing) can
  start on tick F - s + 1. While the counter is above 0 the weapon's
  triggers read as released (user busy), starting the tick after the
  press; on the press tick itself the weapon can still fire.
- The hit: from the holder's "head" marker, 25 probe segments
  0.8 a + 0.1 (i c + j p) for i, j in -2..2, a the aiming vector, p a
  unit vector perpendicular to a and c = a x p; each is traced against
  everything a projectile hits, except the attacker. A hit on an object
  that is not a weapon and has a parent counts as its parent. The
  target is the nearest biped hit (any biped beats any other object);
  failing that the first non-biped object; failing that the level
  surface (its material, and a breakable surface). No lunge.
- Damage: the weapon's melee damage (no weapon: the unit's own), area
  of effect, from the head marker, direction a, owner the attacker.
  Only a **biped** target is damaged. A vehicle target is pushed by a x
  acceleration scale x 0.035. A breakable surface takes it when no
  object was hit. Damage scale = clamp((velocity . forward) x 30 / run
  forward speed, 0, 1) with run forward speed 2.25 WU/s from the
  globals, and 1.5 when the attacker has been airborne more than 15
  ticks. The scale is the damage event's scale: amount = (1 - s) x
  minimum + U x s (`health_shields.md`), so a standing melee deals the
  effect's minimum (open question 5).
  Damage effects: health_shields.md and projectiles.md.
- If anything with a material was hit, the melee sound plays and the
  weapon's melee response damage is applied to the attacker.

| Weapon | F | K | Counter F - s | Damage tick | Next melee or shot |
|---|---|---|---|---|---|
| assault rifle | 37 | 3 | 28 | 4 | 29 |
| pistol | 45 | 4 | 34 | 5 | 35 |
| shotgun | 36 | 4 | 27 | 5 | 28 |
| plasma rifle | 36 | 6 | 27 | 7 | 28 |
| sniper rifle | 36 | 6 | 27 | 7 | 28 |
| plasma pistol | 33 | 4 | 25 | 5 | 26 |
| needler | 48 | 6 | 36 | 7 | 37 |
| rocket launcher | 52 | 4 | 39 | 5 | 40 |
| flamethrower | 51 | 8 | 39 | 9 | 40 |
| ball | 28 | 6 | 21 | 7 | 22 |
| flag | 40 | 6 | 30 | 7 | 31 |

### 14. Grenades (the weapon's part)

A grenade throw (projectiles.md owns the grenade) can begin only if the
unit has a grenade of the current type, its animation is not busy, and
the weapon in hand does not prevent it: flag 6 (flag, ball), or the
weapon state being reload, charged, ready or put away (only weapons
whose own graph has those animations can be in those states; none of
the held multiplayer weapons can). Starting a throw settles any reload
(section 11), cancels a melee, unzooms and plays the first-person
"throw-grenade" animation. The holder's throw animation (multiplayer
Spartan: `stand rifle throw-grenade`, 38 frames, key frame 8, the same
for every weapon class) makes the holder busy, so the weapon's triggers
read as released for the throw's duration. The grenade button pulls
trigger 1 instead of throwing only with flag 12.

### 15. Aim assist

Aim assist has two parts. The **autoaim** (the target, the bending of
the shot toward it, and the target that guided projectiles follow:
`projectiles.md` behaviour 12) runs for every player's shot whatever the
input device: the reference's port keeps the bullets' autoaim for the
mouse too. The **magnetism** (look stickiness, at the end of this
section) is the controller feature that the port makes optional for a
mouse and leaves off by default for one. Only the port's Hardcore game
types (not on the disc) remove both. Without a target (nothing in the
cone, or no parameters in force) the convergence correction below (step
4 with no target) remains, since it is part of where a shot goes.

**Parameters** of the weapon in hand at zoom level i (none if the
weapon has "aim assists only when zoomed" and the player is not
zoomed): autoaim angle / m, autoaim range x m, magnetism angle / m,
magnetism range x m, deviation = max(deviation angle, autoaim angle) /
m, m the magnification (1 unzoomed).

**Targets:** living bipeds of enemy teams, not fully camouflaged
(camouflage < 1), not flagged "ignored by autoaim", whose bounding
sphere meets the cone of max(autoaim, magnetism) angle and range
around the camera ray. A target's aim point is the point of its
autoaim pill (objects.md) closest to the aim line, then moved toward
the line by at most the pill's width. With distance D and angle A
from the camera ray to that point:

g(x, X) = 1 if x <= X / 2; (X - x) / (X / 2) if X / 2 < x < X; 0 if
x >= X

autoaim level = g(D, autoaim range) g(A, autoaim angle); magnetism
level = g(D, magnetism range) g(A, magnetism angle), times 0.5 for
inconsequential units. Targets with both levels 0 are dropped. They
are sorted by autoaim level, then magnetism level (both descending),
then distance, then angle, then object index; the first with a clear
line of sight from the camera wins.

**Bending the shot** (every player shot when parameters exist):
1. Trace from the camera, starting |camera - holder origin| along the
   camera direction, 128 WU along it against everything a projectile
   hits, ignoring the shooter; P = the hit point (open question 3 for a
   miss). Convergence direction c = normalise(P - origin) (the origin
   of section 6).
2. Target direction t = normalise(target aim point - origin) if there
   is a target, else the current direction.
3. Desired = normalised interpolation from c toward t by the autoaim
   level.
4. The desired direction is pinned into the cone of half-angle
   deviation around the original direction (rotated onto the cone's
   edge if outside). Pistol unzoomed: 3 deg; sniper rifle zoomed 2x:
   0.5 deg, 8x: 0.125 deg, unzoomed: no correction at all; rocket
   launcher: deviation 0, so rockets fly parallel to the view from
   their offset origin.

**Magnetism** (look stickiness, controller only; the reference turns it
off for the mouse): when the magnetism level is above 0 and the player
is turning or moving, the look change per tick becomes target angular
velocity x level x 0.7 + input x (1 - level x 0.6), the target's
angular velocity (its velocity relative to the player's, seen from the
camera) clamped to 6 deg per tick in yaw and 3 deg per tick in pitch.

### 16. Recoil and the first-person weapon

- The weapon applies no camera kick of its own. Each shot applies the
  trigger's firing damage effect to the holder (`weapons\<w>\trigger`),
  whose camera shake, rumble and impulse are projectiles.md's.
- The first-person weapon (animations.md) gets a push-back on each
  primary fire: velocity += 0.05; the push-back moves toward 0 with
  acceleration 0.01 and speed at most 0.2, clamped to 0..1 (velocity
  zeroed at 1), and scales the overlay animation's frame 8. Visual
  only.
- First-person animations that gate gameplay are only: ready (draw),
  reload-empty (reload length), enter (shotgun first shell), melee
  (length and key frame). Fire, overheat, misfire, charged, exit and
  idle animations are visual.

First-person animation frame counts on the disc (`weapons\<w>\fp\fp`;
key frames in brackets):

| Weapon | ready | reload-empty | reload-full | melee | throw-grenade | others |
|---|---|---|---|---|---|---|
| assault rifle | 29 | 87 | 87 (same) | 37 [3] | 37 | light 27, no fire animation |
| pistol | 35 | 67 | 65 | 45 [4] | 38 | fire-1 15 |
| shotgun | 23 | 12 | 12 (same) | 36 [4] | 37 | fire-1 26, enter 15, exit-empty 42, exit-full 24 |
| plasma rifle | 29 | none | none | 36 [6] | 37 | fire-1 22, misfire-1 18, overheating 58, overheating-again 50, overheated 50, o-h-exit 35, throw-overheated 35 |
| sniper rifle | 29 | 94 | 83 | 36 [6] | 37 | fire-1 19 |
| plasma pistol | 14 | (overheated, 9) | (same) | 33 [4] | 37 | fire-1 13, fire-2 24, overcharged 20, overcharged-jitter 16, misfire-1 19, overheating 34, overheating-again 25, o-h-exit 24, o-h-s-enter 24, throw-overheated 33 |
| needler | 23 | 70 | 70 (same) | 48 [6] | 37 | fire-1 8, ammunition 21 |
| rocket launcher | 22 | 125 | 111 | 52 [4] | 43 | fire-1 28 |
| flamethrower | 30 | 104 | 104 (same) | 51 [8] | 39 | |
| ball | 28 | none | none | 28 [6] | none | |
| flag | 21 | none | none | 40 [6] | none | |

Weapon world animation graphs: pistol and sniper rifle "fire-1" 5
frames (recoil); shotgun "chamber-1" 22 frames and "reload-2" 27
frames; the others have none.

### 17. Smaller systems

- **Export functions** (A-D inputs, for shaders and effects): ready 1;
  heat; overheated = (heat - recovery threshold) / (1 - threshold)
  while overheated; illumination = max over triggers of charged
  fraction x charged illumination, the charged glow (1 - charged
  illumination) x overcharged + charged illumination, and the trigger's
  muzzle glow, then max with heat illumination x heat; ammunition =
  loaded / magazine size; ejection port; rate of fire; firing = rate of
  fire if the weapon fired within the last tick else 0; charged
  fraction = 1 - (timer / 30) / charging time while charging, 1 when
  charged; integrated light; age. The weapon writes them into its
  holder's inputs when the weapon itself is invisible.
- **Active camouflage:** each shot takes the weapon's camo ding (0.1 if
  0) from a camouflaged holder's camouflage, not below 0.05 (only if it
  was at least 0.05), and marks it as shot; the camouflage then grows
  back by the weapon's regrowth rate per tick (1/120 if 0) until full.
- **Integrated light:** the weapon's light power falls 1/24 per tick
  unless the holder drives it directly. The power-on and power-off
  times are not read.
- **Night vision:** a weapon with flag 14 lets the holder's night vision
  work while zoomed (hud.md / rendering); leaving zoom turns it off.

## Multiplayer weapon numbers

All values from `bloodgulch.map`, identical on the other multiplayer
maps. Angles in radians with degrees in brackets. "Ticks" is the
interval between shots of a held trigger at rate of fire 0 and 1, by
the rule of section 4 with no battery penalty.

### Weapon part

| Weapon | Flags | Label | Type | Zoom levels | Magnification | Ready time (unused) |
|---|---|---|---|---|---|---|
| assault rifle | 0x0000 | ar | 0 | 0 | | 1.3 |
| pistol | 0x0000 | hp | 0 | 1 | 2 - 2 | 0 |
| shotgun | 0x0000 | sg | 1 shotgun | 0 | | 0 |
| plasma rifle | 0x0800 (11) | pr | 4 plasma rifle | 0 | | 0 |
| sniper rifle | 0x4020 (5, 14) | sr | 0 | 2 | 2 - 8 | 0 |
| plasma pistol | 0x0800 (11) | pp | 3 plasma pistol | 0 | | 0 |
| needler | 0x0000 | ne | 2 needler | 0 | | 0 |
| rocket launcher | 0x0000 | rl | 0 | 1 | 2 - 2 | 0 |
| flamethrower | 0x0000 | ft | 0 | 0 | | 0 |
| gravity rifle | 0x0000 | ar | 0 | 0 | | 0 |
| ball | 0x00D8 (3, 4, 6, 7) | b | 0 | 0 | | 0 |
| flag | 0x00D8 (3, 4, 6, 7) | f | 0 | 0 | | 0 |
| warthog gun | 0x0000 | cg | 0 | 0 | | 0 |
| ghost gun | 0x0000 | gun | 0 | 0 | | 0 |
| scorpion cannon | 0x1000 (12) | cannon | 0 | 0 | | 0 |

Secondary trigger mode and maximum alternate shots are 0 on all 15.
Function sources A, B, C, D: assault rifle 14, 2, 12, 0; pistol 12, 0,
0, 0; shotgun 7, 0, 12, 14; plasma rifle 1, 12, 0, 13; sniper rifle 12,
6, 0, 0; plasma pistol 1, 10, 12, 13; needler 12, 2, 0, 0; rocket
launcher 14, 0, 0, 0; flamethrower 15, 2, 1, 0; gravity rifle 12, 0,
0, 0; warthog gun 12, 0, 0, 4; ghost gun 4, 0, 0, 0; scorpion cannon
12, 0, 0, 4; ball and flag 0.

### Aim assist and movement

| Weapon | Autoaim angle | Autoaim range (WU) | Magnetism angle | Magnetism range (WU) | Deviation | Movement penalized, fwd, side |
|---|---|---|---|---|---|---|
| assault rifle | 0.10472 (6) | 25 | 0.20944 (12) | 25 | 0 | 0, 0, 0 |
| pistol | 0.0523599 (3) | 30 | 0.10472 (6) | 30 | 0 | 0, 0, 0 |
| shotgun | 0.10472 (6) | 15 | 0.20944 (12) | 15 | 0 | 0, 0, 0 |
| plasma rifle | 0.0872665 (5) | 25 | 0.20944 (12) | 25 | 0 | 0, 0, 0 |
| sniper rifle | 0.0174533 (1) | 35 | 0.0349066 (2) | 35 | 0 | 0, 0, 0 |
| plasma pistol | 0.0872665 (5) | 20 | 0.20944 (12) | 20 | 0 | 0, 0, 0 |
| needler | 0.10472 (6) | 25 | 0.20944 (12) | 25 | 0 | 0, 0, 0 |
| rocket launcher | 0 | 35 | 0.20944 (12) | 35 | 0 | 2, 0.25, 0.25 |
| flamethrower | 0 | 0 | 0 | 0 | 0 | 0, 0, 0 |
| gravity rifle, ball, flag | 0 | 0 | 0 | 0 | 0 | 0, 0, 0 |
| warthog gun | 0.10472 (6) | 25 | 0.20944 (12) | 25 | 0.174533 (10) | 0, 0, 0 |
| ghost gun | 0.0872665 (5) | 25 | 0.20944 (12) | 25 | 0.785398 (45) | 0, 0, 0 |
| scorpion cannon | 0.10472 (6) | 35 | 0.20944 (12) | 35 | 0.349066 (20) | 0, 0, 0 |

### Heat and battery

| Weapon | Recovery | Overheat | Detonation threshold, fraction | Loss /s | Heat illum. | Age heat pen. | Age ROF pen. | Misfire start, chance |
|---|---|---|---|---|---|---|---|---|
| plasma rifle | 0.25 | 1 | 1, 0 | 0.3 | 1 | 0.2 | 0.2 | 0.9, 0.5 |
| plasma pistol | 0.25 | 1 | 1, 0 | 0.65 | 1 | 0 | 0 | 0.9, 0.5 |
| flamethrower | 0.9 | 1 | 0, 0 | 0.475 | 0 | 0 | 0 | 0, 0 |
| all others | 0 | 0 | 0, 0 | 0 | 0 | 0 | 0 | 0, 0 |

### Magazines

| Weapon | Flags | Recharge /s | Total initial | Total max | Loaded max | Reload time (unused) | Rounds reloaded | Chamber time | Ammunition objects |
|---|---|---|---|---|---|---|---|---|---|
| assault rifle | 0 | 0 | 240 | 600 | 60 | 3.4 | 60 | 0 | 240 x `powerups\assault rifle ammo\assault rifle ammo` |
| pistol | 0 | 0 | 60 | 120 | 12 | 2.17 | 12 | 0 | 120 x `powerups\pistol ammo\pistol ammo` |
| shotgun | 2 (chamber every round) | 0 | 24 | 60 | 12 | 0.4 | 1 | 0 | 60 x `powerups\shotgun ammo\shotgun ammo` |
| plasma rifle | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | none |
| sniper rifle | 0 | 0 | 12 | 24 | 4 | 0 | 4 | 0 | 16 x `powerups\sniper rifle ammo\sniper rifle ammo` |
| plasma pistol | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | none |
| needler | 0 | 0 | 80 | 80 | 20 | 1 | 20 | 0 | 80 x `powerups\needler ammo\needler ammo` |
| rocket launcher | 0 | 0 | 4 | 8 | 2 | 5 | 2 | 0 | 2 x `powerups\rocket launcher ammo\rocket launcher ammo` |
| flamethrower | 1 (wastes rounds) | 0 | 600 | 1200 | 100 | 3.4 | 100 | 0 | 100 x `powerups\flamethrower ammo\flamethrower ammo` |
| gravity rifle | 0 | 0 | 200 | 400 | 200 | 3.4 | 200 | 0 | none |
| warthog gun | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | none |
| ghost gun | 0 | 0 | 300 | 300 | 50 | 2 | 50 | 0.3 | none |
| scorpion cannon | 2 (chamber every round) | 0 | 9999 | 9999 | 1 | 0 | 1 | 4 | none |

Ball and flag have no magazines or triggers. A new weapon therefore
holds: assault rifle 60 + 180, pistol 12 + 48, shotgun 12 + 12, sniper
rifle 4 + 8, needler 20 + 60, rocket launcher 2 + 2, flamethrower 100 +
500. The only reloading/chambering effect on the disc is the pistol's
`weapons\pistol\effects\reload`; the only ready effects are
`weapons\pistol\effects\ready` and `weapons\needler\effects\ready`.

### Triggers: rate of fire and ammunition

| Weapon (trigger) | Flags | Rounds/s lower - upper | Ticks | Accel., decel. time (s) | Runtime accel., decel. /tick | Blur | Magazine | Rounds/shot | Min loaded | Tracer gap | Noise |
|---|---|---|---|---|---|---|---|---|---|---|---|
| assault rifle | 0 | 15 - 15 | 2 - 2 | 1, 0.1 | 0.0333333, 0.333333 | 0 | 0 | 1 | 0 | 3 | 2 |
| pistol | 0 | 3.5 - 3.5 | 9 - 9 | 0, 0 | 1, 1 | 0 | 0 | 1 | 0 | 0 | 2 |
| shotgun | 0 | 1 - 1 | 30 - 30 | 0, 0 | 1, 1 | 0 | 0 | 1 | 0 | 0 | 2 |
| plasma rifle | 0 | 7 - 10 | 5 - 3 (see Quirks: 5 - 4) | 1.8, 0.25 | 0.0185185, 0.133333 | 0 | 0 | 0 | 0 | 3 | 2 |
| sniper rifle | 0x0408 (3, 10) | 2 - 2 | 15 - 15 | 0, 0 | 1, 1 | 0 | 0 | 1 | 0 | 0 | 1 |
| plasma pistol (0) | 0 | 0 - 0 | press-limited | 0, 0 | 1, 1 | 0 | 0 | 0 | 0 | 0 | 2 |
| plasma pistol (1) | 0x0004 (2) | 0 - 0 | | 0, 0 | 1, 1 | 0 | 0 | 0 | 0 | 0 | 2 |
| needler | 0 | 3 - 10 | 10 - 3 | 0.5, 0.15 | 0.0666667, 0.222222 | 0 | 0 | 1 | 0 | 0 | 2 |
| rocket launcher | 0x0008 (3) | 0.5 - 0.5 | 60 - 60 | 0, 0 | 1, 1 | 0 | 0 | 1 | 0 | 0 | 2 |
| flamethrower | 0x0800 (11) | 15 - 15 | 2 - 2 | 0.2, 0 | 0.166667, 1 | 0 | 0 | 1 | 0 | 0 | 2 |
| gravity rifle | 0 | 30 - 30 | 1 - 1 | 0, 0 | 1, 1 | 0 | 0 | 1 | 0 | 0 | 2 |
| warthog gun | 0 | 8 - 15 | 4 - 2 | 1, 4 | 0.0333333, 0.00833333 | 0.5 | 0 | 0 | 0 | 3 | 2 |
| ghost gun | 0x1020 (5, 12) | 5 - 6 | 6 - 5 | 1, 0.25 | 0.0333333, 0.133333 | 0 | 0 | 1 | 0 | 2 | 2 |
| scorpion cannon (0) | 0x0028 (3, 5) | 0 - 0 | chamber-limited | 0, 0 | 1, 1 | 0 | 0 | 1 | 0 | 0 | 2 |
| scorpion cannon (1) | 0x0020 (5) | 15 - 15 | 2 - 2 | 0, 0 | 1, 1 | 0 | NONE | 1 | 0 | 3 | 2 |

### Triggers: error and projectiles

| Weapon (trigger) | Error (unused) | Min. error | Error angle lower | Error angle upper | Error accel., decel. time (s) | Runtime error accel., decel. /tick | Projectiles | Distribution, angle | Projectile |
|---|---|---|---|---|---|---|---|---|---|
| assault rifle | 0 - 1 | 0 | 0.0349066 (2) | 0.113446 (6.5) | 0.6, 1 | 0.0555556, 0.0333333 | 1 | point | `weapons\assault rifle\bullet` |
| pistol | 0 - 2 | 0 | 0.00349066 (0.2) | 0.0349066 (2) | 0.5, 0.2 | 0.0666667, 0.166667 | 1 | point | `weapons\pistol\bullet` |
| shotgun | 0 - 0 | 0 | 0.174533 (10) | 0.174533 (10) | 0, 0 | 1, 1 | 15 | point | `weapons\shotgun\pellet` |
| plasma rifle | 0 - 1 | 0 | 0.00872665 (0.5) | 0.0872665 (5) | 2.5, 0 | 0.0133333, 1 | 1 | point | `weapons\plasma rifle\bolt` |
| sniper rifle | 1 - 1 | 0 | 0.00872665 (0.5) | 0.00872665 (0.5) | 0, 0 | 1, 1 | 1 | point | `weapons\sniper rifle\sniper bullet` |
| plasma pistol (0) | 0 - 1 | 0 | 0 | 0 | 0, 0 | 1, 1 | 1 | point | `weapons\plasma pistol\bolt` |
| plasma pistol (1) | 0 - 0 | 0 | 0 | 0 | 0, 0 | 1, 1 | 1 | point | `weapons\plasma rifle\charged bolt` |
| needler | 0 - 1 | 0.0349066 (2) | 0.0698132 (4) | 0.0698132 (4) | 0.5, 0 | 0.0666667, 1 | 1 | point | `weapons\needler\needle` |
| rocket launcher | 0 - 0 | 0 | 0 | 0 | 0, 0 | 1, 1 | 1 | point | `weapons\rocket launcher\rocket` |
| flamethrower | 0 - 0 | 0 | 0.0349066 (2) | 0.0698132 (4) | 0, 0 | 1, 1 | 1 | point | `weapons\flamethrower\flame` |
| gravity rifle | 0 - 0 | 0 | 0.0698132 (4) | 0.20944 (12) | 0, 0 | 1, 1 | 1 | point | `weapons\flamethrower\flame` |
| warthog gun | 0 - 1 | 0 | 0.0174533 (1) | 0.0349066 (2) | 3, 1 | 0.0111111, 0.0333333 | 1 | point | `vehicles\warthog\bullet` |
| ghost gun | 0 - 1 | 0.00872665 (0.5) | 0.00872665 (0.5) | 0.0523599 (3) | 2, 0.5 | 0.0166667, 0.0666667 | 1 | point | `vehicles\ghost\ghost bolt` |
| scorpion cannon (0) | 0 - 0 | 0 | 0.0174533 (1) | 0.0349066 (2) | 0, 0 | 1, 1 | 1 | point | `vehicles\scorpion\tank shell` |
| scorpion cannon (1) | 0 - 1 | 0 | 0.0349066 (2) | 0.113446 (6.5) | 0.6, 1 | 0.0555556, 0.0333333 | 1 | point | `vehicles\scorpion\bullet` |

### Triggers: heat, charge and the rest

| Weapon (trigger) | Heat/round | Age/round | Charging, charged (s) | Overcharged action | Charged illum. | Spew, overload (s) | First person offset (WU) | Ejection port, illumination recovery (s) | Runtime ejection, illumination /tick |
|---|---|---|---|---|---|---|---|---|---|
| assault rifle | 0 | 0 | 0, 0 | 0 | 0 | 0, 0 | 0, 0, 0 | 0.15, 0.1 | 0.222222, 0.333333 |
| pistol | 0 | 0 | 0, 0 | 0 | 0 | 0, 0 | 0, 0, 0 | 0, 0.1 | 1, 0.333333 |
| shotgun | 0 | 0 | 0, 0 | 0 | 0 | 0, 0 | 0, 0, 0 | 0.15, 0.1 | 0.222222, 0.333333 |
| plasma rifle | 0.08 | 0.005 | 0, 0 | 0 | 0 | 0, 0 | 0, 0, 0 | 0, 0.15 | 1, 0.222222 |
| sniper rifle | 0 | 0 | 0, 0 | 0 | 0 | 0, 0 | 0, 0, 0 | 0.15, 0.1 | 0.222222, 0.333333 |
| plasma pistol (0) | 0.16 | 0.002 | 0.6, 45 | 0 none | 1 | 0, 0 | 0, 0, 0 | 0, 0.15 | 1, 0.222222 |
| plasma pistol (1) | 1 | 0.11 | 0, 0 | 1 explodes (never reached: this trigger never charges) | 0 | 0, 0 | 0, 0, 0 | 0, 1 | 1, 0.0333333 |
| needler | 0 | 0 | 0, 0 | 0 | 0 | 0, 0 | 0, 0, 0 | 0, 0.2 | 1, 0.166667 |
| rocket launcher | 0 | 0 | 0, 0 | 0 | 0 | 0, 0 | 0, -0.1, 0 | 0, 0.3 | 1, 0.111111 |
| flamethrower | 0.04125 | 0 | 0, 0 | 0 | 0 | 0, 0 | 0, 0, 0 | 0, 1 | 1, 0.0333333 |
| gravity rifle | 0 | 0 | 0, 0 | 0 | 0 | 0, 0 | 0, 0, 0 | 0, 1 | 1, 0.0333333 |
| vehicle turrets | 0 | 0 | 0, 0 | 0 | 0 | 0, 0 | 0, 0, 0 | warthog and scorpion 0.15, 0.1; ghost 0, 0 | 0.222222, 0.333333; ghost 1, 1 |

No trigger on the disc has a charging effect.

### Firing effects (all shot counts 1 - 1)

| Weapon (trigger) | Entry | Firing effect | Misfire effect | Empty effect | Firing damage | Misfire damage |
|---|---|---|---|---|---|---|
| assault rifle | 0 | `weapons\assault rifle\effects\fire bullet` | none | `weapons\assault rifle\effects\empty` | `weapons\assault rifle\trigger` | none |
| pistol | 0 | `weapons\pistol\effects\fire bullet` | none | `weapons\pistol\effects\empty` | `weapons\pistol\trigger` | none |
| shotgun | 0 | `weapons\shotgun\effects\shotgun firing` | `weapons\shotgun\effects\shotgun empty` | `sound\sfx\weapons\assault rifle\dryfire` (sound) | `weapons\shotgun\trigger` | none |
| plasma rifle | 0 | `weapons\plasma rifle\effects\plasma rifle upper fire` | none | `sound\sfx\weapons\plasma rifle\overheat` (sound) | `weapons\plasma rifle\trigger` | `weapons\plasma rifle\misfire` |
| plasma rifle | 1 | `weapons\plasma rifle\effects\plasma rifle lower fire` | `weapons\plasma rifle\effects\misfire` | none | `weapons\plasma rifle\trigger` | `weapons\plasma rifle\misfire` |
| sniper rifle | 0 | `weapons\sniper rifle\effects\fire bullet` | none | `sound\sfx\weapons\assault rifle\dryfire` (sound) | `weapons\sniper rifle\trigger` | none |
| plasma pistol (0) | 0 | `weapons\plasma pistol\effects\fire bolt` | `weapons\plasma pistol\effects\misfire` | `sound\sfx\weapons\plasma rifle\overheat` (sound) | `weapons\plasma pistol\trigger` | `weapons\plasma pistol\misfire` |
| plasma pistol (1) | 0 | `weapons\plasma rifle\effects\charged bolt fire` | none | none | `weapons\plasma pistol\trigger overcharge` | none |
| needler | 0 | `weapons\needler\effects\fire needle` | none | `weapons\needler\effects\empty` | `weapons\needler\trigger` | none |
| rocket launcher | 0 | `weapons\rocket launcher\effects\fire rocket` | none | `sound\sfx\weapons\assault rifle\dryfire` (sound) | `weapons\rocket launcher\trigger` | none |
| flamethrower | 0 | `weapons\flamethrower\effects\flame thrower jet` | none | none | none | none |
| warthog gun | 0 | `vehicles\warthog\effects\warthog gun fire` | none | none | `vehicles\warthog\trigger` | none |
| ghost gun | 0 | `vehicles\ghost\ghost gun fire beam` | none | none | `vehicles\ghost\trigger` | none |
| scorpion cannon (0) | 0 | `vehicles\scorpion\scorpion cannon fire` | none | none | `vehicles\scorpion\shell shock wave` | none |
| scorpion cannon (1) | 0 | `vehicles\scorpion\secondary fire bullet` | none | none | `vehicles\scorpion\secondary trigger` | none |

The plasma rifle alternates its two entries (upper, lower barrel). No
entry has an empty damage. The gravity rifle has no firing effects.

### References

| Weapon | Melee damage | Melee response | First person model and animations | HUD interface |
|---|---|---|---|---|
| assault rifle | `weapons\assault rifle\melee` | `weapons\assault rifle\melee_response` | `weapons\assault rifle\fp\fp` | `weapons\assault rifle\assault rifle` |
| pistol | `weapons\pistol\melee` | `weapons\pistol\melee_response` | `weapons\pistol\fp\fp` | `weapons\pistol\pistol` |
| shotgun | `weapons\shotgun\melee` | `weapons\shotgun\melee_response` | `weapons\shotgun\fp\fp` | `weapons\shotgun\shotgun` |
| plasma rifle | `weapons\plasma rifle\melee` | `weapons\plasma rifle\melee_response` | `weapons\plasma rifle\fp\fp` | `weapons\plasma rifle\plasma rifle` |
| sniper rifle | `weapons\sniper rifle\melee` | `weapons\sniper rifle\melee_response` | `weapons\sniper rifle\fp\fp` | `weapons\sniper rifle\sniper rifle` |
| plasma pistol | `weapons\plasma pistol\melee` | `weapons\plasma pistol\melee_response` | `weapons\plasma pistol\fp\fp` | `weapons\plasma pistol\plasma pistol` |
| needler | `weapons\needler\melee` | `weapons\needler\melee_response` | `weapons\needler\fp\fp` | `weapons\needler\needler` |
| rocket launcher | `weapons\rocket launcher\melee` | `weapons\rocket launcher\melee_response` | `weapons\rocket launcher\fp\fp` | `weapons\rocket launcher\rocket_launcher` |
| flamethrower | `weapons\flamethrower\melee` | `weapons\flamethrower\melee_response` | `weapons\flamethrower\fp\fp` | `weapons\flamethrower\flame thrower` |
| ball | `weapons\ball\melee` | `weapons\ball\melee_response` | `weapons\ball\fp\fp` | none |
| flag | `weapons\flag\melee` | `weapons\flag\melee_response` | `weapons\flag\fp\fp` | none |
| turrets | none | none | none | `vehicles\<v>\<v> gun` / `vehicles\scorpion\scorpion cannon` |

Sounds: pickup `sound\sfx\weapons\weapon_pickup_ammo\ar_ammo`,
`pistol_ammo`, `shotgun_ammo`, `sniper_ammo`, `needle_ammo`,
`rocket_ammo` (same folder; plasma weapons, flamethrower: none). Zoom
in / out (pistol, sniper rifle, rocket launcher): `sound\sfx\weapons\
sniper rifle\sniper_2x_zoom` and `...\sniper_10x_zoom`. Overheated
effects: `weapons\plasma rifle\effects\overheated`, `weapons\plasma
pistol\effects\overheated`. Light effects (assault rifle, shotgun has
times only): `weapons\assault rifle\effects\flashlight` for on and off,
power times 0.1 / 0.8 s. Actor firing parameters (`actv`, turrets
only): `vehicles\warthog\warthog gunner`, `vehicles\ghost\ghost pilot`,
`vehicles\scorpion\scorpion pilot`. No detonation effects.

Other fields: active camo ding / regrowth: assault rifle 0.25 / 0.04,
pistol 0.65 / 0.01, shotgun 1 / 0.036, plasma rifle 0.3 / 0.05, sniper
rifle 0.55 / 0, plasma pistol 1 / 0.04, needler 0.16 / 0.015, rocket
launcher 1 / 0, the rest 0 / 0. Reticle ranges (near, far,
intersection): warthog gun 4, 16, 80; scorpion cannon 4, 0, 0; others
0. AI minimum target range 2 and looking time modifier 3 (warthog gun,
scorpion cannon), ghost gun 2 / 0; others 0.

### Headline (the four M2 weapons)

| Weapon | Magazine + reserve (max reserve) | Ticks between shots | Error (deg) | Projectile |
|---|---|---|---|---|
| assault rifle | 60 + 180 (600) | 2 (15/s), automatic | 2 to 6.5 over 18 held ticks | `weapons\assault rifle\bullet` |
| pistol | 12 + 48 (120) | 9 (3.33/s), automatic when held | 0.2 to 2 over 15 held ticks | `weapons\pistol\bullet` |
| plasma rifle | battery 200 shots, heat 0.08/shot | 5, 5, then 4 (6 to 7.5/s; the tag's 10/s is never reached, Quirk 1) | 0.5 to 5 over 75 held ticks | `weapons\plasma rifle\bolt` |
| shotgun | 12 + 12 (60) | 30 (1/s) | 10, 15 pellets | `weapons\shotgun\pellet` |

## Per-tick order

Within one tick, for a player holding a weapon:

1. Input: the player's buttons give the unit its controls; switch,
   zoom and swap requests are handled (zoom rotates on the press tick;
   the swap fires at 7 held frames).
2. The holder's unit update: switching (if the desired weapon differs
   and the unit is not busy: put away and ready, both immediate);
   zoom changes applied; grenade throw state; then the weapon's control
   inputs are computed and handed to it (user busy if the unit
   animation is busy or a player melee counter is above 0 from earlier
   ticks).
3. The holder's biped update: a player melee may start (settling any
   reload) or tick down, dealing its damage on its hit tick.
4. The held weapon's update (children update after their parent):
   1. world animation step (a finished recoil or chamber animation
      returns the weapon state to idle);
   2. detonate if it detonates when dropped and has no holder;
   3. integrated light decay;
   4. heat: overheat check, cooling (skipped if overcharged from last
      tick), overheat clear;
   5. overcharged amount reset to 0; weapon state timer - 1;
   6. trigger "down" values from the inputs (released if busy or the
      state timer is above 0), secondary trigger mode;
   7. reload input sets "needs to reload"; if set, try to start a
      reload of magazine 0;
   8. each magazine: recharge, timer - 1, then reloading (finish when
      timer - 1 <= 0), unchambered (start chambering), chambering (idle
      at timer 0);
   9. each trigger in order (trigger 0 may fire trigger 1 inside its
      step): analog and sticky overrides, timer - 1, toggle, released
      flag, ejection port and illumination decay, the state machine
      (idle: automatic reload check, then fire or count up), then rate
      of fire and error ramps.
5. Weapons in the inventory but not in hand are updated as root objects
   in the same tick with no inputs.

## Edge cases

- **Exact quotients.** 30 / rate is compared in single precision with
  the idle counter + 1 (a whole number). 30 / 15 = 2.0 exactly, so the
  assault rifle fires every 2 ticks; a quotient a rounding error above
  a whole number costs one extra tick. Example: the plasma rifle's rate
  of fire, summed in single precision from 0.0185185 per tick, is
  0.9999996 after 54 held ticks (required 3.0000004, so 4 ticks even
  with a fresh battery) and is clamped to 1 only on the 55th. Implement
  the ramps in single precision with the runtime per-tick rates from
  the tag.
- **Pressing fire during a reload** does nothing (magazine not idle)
  but keeps the error and rate of fire rising and, for the shotgun,
  stops the shell loop after the current shell.
- **Empty click:** with no rounds a held trigger tries every tick but a
  fire attempt with too few rounds plays the empty effect once and locks
  the trigger until it is released. An empty magazine with reserve
  starts its reload on the next tick instead (the automatic check runs
  before the fire attempt), so the click is heard only when the reserve
  is empty too.
- **Rounds below rounds per shot** with "partial ammo": the shot fires
  and the magazine goes to 0.
- **Minimum rounds loaded** only gates the first shot of a pull; a held
  trigger keeps firing below it.
- **Reload input with a full magazine or no reserve:** clears the
  request, nothing happens.
- **Reloading a weapon in the inventory:** never starts on its own (the
  automatic check needs a holder).
- **The pistol and sniper rifle** cannot start a reload during the 5
  ticks of their recoil animation (the request waits), and the shotgun
  during the 22 ticks of its chamber animation.
- **A degenerate cone axis** (the table direction parallel to the aim)
  leaves the shot unbent and draws one random number instead of two.
- **Looking straight up or down:** the first-person offset's left
  vector falls back to +Y.
- **Charged plasma pistol and melee:** melee is refused while charging
  or charged.
- **Releasing a charge** fires trigger 1 directly, without the overheat
  and under-water checks that guard every other start of fire.
- **Under water** no trigger can begin firing. A plasma pistol tap
  released under water is refused and retried every tick while the
  charge runs; if the charge completes first, the next tick (trigger
  still up) releases a charged bolt.
- **Magazine index out of range** in a tag: that trigger never fires
  (treat the tag as damaged).

## Quirks

Keep all of these unless noted.

1. **The plasma rifle never reaches 10 rounds per second.** The
   battery penalty multiplies the required ticks by 1 + 0.2 age, and
   any age above 0 lifts the 3.0 required at full rate of fire above 3,
   so the interval stays 4 ticks (7.5 per second). Full rate of fire
   (exactly 1) is reached only on the 55th held tick (Edge cases), long
   after the first shot has aged the battery, so 3 ticks never occurs.
   The intervals are 5, 5, then 4 from the third shot (4 ticks is
   already reached at about 1/6 of the ramp).
   This is what the reference computes; verify in the game (open
   question 1).
2. **The plasma pistol fires on release**, not on the press.
3. **Reload length comes from the first-person animation**, and the
   reload-empty animation is used even for a partial reload (the
   pistol's 65-frame reload-full and the sniper rifle's 83-frame one
   are never used for timing).
4. **Half-way reload rule:** switching weapons, throwing a grenade or
   meleeing after more than half the reload animation completes the
   reload instantly; before half, it is lost. Players use this to
   shorten reloads (melee or grenade at the right moment).
5. **Ready time, reload time, initial/final error, movement penalties
   and light power times are tag fields the engine ignores.**
6. **The pistol's scope does not change its accuracy** (no "use error
   when unzoomed" flag); only the sniper rifle is perfect when zoomed.
7. **Error is a uniform angle**, so spread is centre-heavy.
8. **No melee while zoomed:** the press is ignored until unzoomed.
9. **A shot can fire on the same tick a melee starts** (the busy flag
   is read before the melee begins).
10. **The rocket launcher is never bent toward the crosshair's
    target** (autoaim angle 0 means deviation 0): its rocket leaves
    0.1 WU right of the camera line and stays parallel to it.
11. **The plasma pistol's charge can be held indefinitely.** Its first
    trigger's overcharged action is none on the disc, so after 45
    seconds nothing happens: the gun stays charged, does not cool, and
    fires the charged bolt on release (open question 4).
12. **Error rises while the trigger is held even when nothing fires**
    (empty, reloading, overheated, pistol between shots).
13. The fan distribution places three projectiles on the centre line
    for odd counts (unused by multiplayer weapons; keep for fidelity).

## Test cases

Computed with a scratch tick simulation of the rules above in single
precision, from the disc's numbers. Tick 0 is the first tick the
trigger is down; the weapon has been in hand for a long time
(idle counter 127, rate of fire and error 0, heat 0, age 0) and holds
its new-weapon ammunition unless stated.

Values are printed to four digits unless more are needed; compare
fractions (error, heat, rate of fire) with a tolerance of 1e-5 except
where a row prints the single-precision value on purpose.

| Setup | Input | After | Expected |
|---|---|---|---|
| assault rifle | hold | 1 s / 2 s / 5 s (ticks 0-29 / 0-59 / 0-149) | 15 / 30 / 60 shots (the magazine; the reload starts tick 119) |
| assault rifle | hold | first six shots | ticks 0, 2, 4, 6, 8, 10 |
| assault rifle | hold | error angle limit of shots 1-10 | 2.0, 2.5, 3.0, 3.5, 4.0, 4.5, 5.0, 5.5, 6.0, 6.5 deg (shot 10 at tick 18; 6.5 thereafter) |
| assault rifle | hold 30 ticks, release | error at end of tick 0, 17, 29, 30, 35, 45, 59 | 0.0556, 1, 1, 0.9667, 0.8, 0.4667, 0 (limit 2.25, 6.5, 6.5, 6.35, 5.6, 4.1, 2.0 deg) |
| pistol | hold | 1 s / 2 s / 5 s | 4 / 7 / 12 shots (the 12th at tick 99; reload starts tick 103) |
| pistol | hold | first six shots | ticks 0, 9, 18, 27, 36, 45 |
| pistol | hold | error limit of shots 1-3 | 0.2, 1.28, 2.0 deg (error 0, 0.6, 1) |
| pistol | hold 30 ticks, release | error at end of tick 30, 31, 32, 35 | 0.8333, 0.6667, 0.5, 0 |
| pistol | press 1 tick, release 1 tick, repeat | 5 s | shots every 10 ticks (0, 10, 20...): the 9-tick limit meets the 2-tick press pattern |
| plasma rifle | hold | 1 s / 2 s / 5 s | 7 / 15 / 25 shots (overheat stops it at 25) |
| plasma rifle | hold | first six shots | ticks 0, 5, 10, 14, 18, 22 (then every 4) |
| plasma rifle | hold | heat after shots 1, 2, 5, 10, 20, 24, 25 | 0.08, 0.1101, 0.2204, 0.4218, 0.8276, 0.9911, 1.0 |
| plasma rifle | hold | overheat | 25th shot on tick 98, overheated on tick 99, clears on tick 175 (heat 0.2492), next shot tick 175 if held |
| plasma rifle | hold | error limit of shots 1-6 | 0.5, 0.8, 1.1, 1.34, 1.58, 1.82 deg |
| plasma rifle | hold 90 ticks, release | error at end of tick 73, 74, 75, 89, 90 | 0.9866660, 0.99999934 (0x3F7FFFF5: 75 steps of the f32 1/75, one short of 1 in single precision), 1.0 (the 76th step clamps), 1.0, 0 |
| plasma rifle | age 0.005, rate of fire 1 | required ticks | 3.003 (interval 4); age 0: 3.0 (interval 3); age 0.995, rate of fire 0: 5.1386 (interval 6) |
| shotgun | hold | 1 s / 2 s / 5 s | 1 / 2 / 5 shots |
| shotgun | hold | first shots | ticks 0, 30, 60, 90, 120, 150 |
| shotgun | fire once (shells left) | chamber | chambering starts tick 1, magazine idle tick 2, weapon state chamber until tick 22 (reload blocked until then) |
| shotgun | fire the last shell on tick 0, release | reload | shell 1 starts tick 1 (15-frame enter), in on tick 15; shells then every 11 ticks: 26, 37, 48, 59, 70, 81, 92, 103, 114, 125, 136 |
| sniper rifle | hold | 1 s / 2 s / 5 s | 1 / 1 / 1 shot (semi-automatic) |
| sniper rifle | press 1 tick, release 1 tick, repeat | first shots | ticks 0, 16, 32, 48 (15-tick limit on the 2-tick pattern); reload after the 4th starts tick 52 |
| sniper rifle | zoomed | any shot | no spread; unzoomed 0.5 deg at any error |
| plasma pistol | hold | 1 s / 2 s / 5 s | 0 / 0 / 0 shots (charged at tick 18; fires on release) |
| plasma pistol | press 1 tick, release 1 tick, repeat | first shots | ticks 1, 3, 5, ..., 17 (9 shots); heat after shots 1-9: 0.16, 0.2767, 0.3933, 0.51, 0.6267, 0.7433, 0.86, 0.9767, 1.0 |
| plasma pistol | same | overheat | overheated tick 18, clears tick 52 (heat 0.2417), next shot tick 53 |
| plasma pistol | hold ticks 0-16, release tick 17 | | normal bolt on tick 17, no charge |
| plasma pistol | hold ticks 0-17, release tick 18 | | charged on tick 18; charged bolt (trigger 1) on tick 19; overheated tick 20; clears tick 54 |
| plasma pistol | hold ticks 0-39, release tick 40 | overcharge timeline | charging tick 0 (18 ticks); charged tick 18 (1350 ticks); charged bolt tick 40, heat 1.0, age 0.11; overheated tick 41; clears tick 75 (heat 0.2417) |
| plasma pistol | hold from tick 0 through tick 1499 | long hold | nothing happens on tick 1368 (charged 18 + 1350, overcharged action none): still charged, no shot, no deletion; overcharged amount 0.9992592 on tick 1367 and 1.0 on tick 1368 and every tick after |
| plasma pistol | hold ticks 0..1499, release tick 1500 | long hold | charged bolt (trigger 1) on tick 1500, heat 1.0; overheated tick 1501; clears tick 1535 (heat 0.2417) |
| assault rifle | last round fired tick 0, release | reload | start tick 1, rounds in tick 87 (60 loaded, reserve 120), can fire tick 89 |
| pistol | last round fired tick 0, release | reload | start tick 4, rounds in tick 70, can fire tick 72 |
| sniper rifle | last round fired tick 0, release | reload | start tick 4, rounds in tick 97, can fire tick 99 |
| assault rifle | hold through the magazine | reload while held | last shot tick 118, reload starts 119, rounds in 205, next shot 207 |
| assault rifle / pistol / shotgun / plasma rifle / sniper rifle | readied on tick 0, hold | first shot | tick 28 / 34 / 22 / 28 / 28 |
| assault rifle | melee pressed tick 0 | | damage tick 4; can fire or melee again tick 29 |
| pistol | melee pressed tick 0 | | damage tick 5; free again tick 35 |
| assault rifle, reloading (87 frames) | grenade thrown with timer 43 left | | 2 x 43 = 86 < 87: rounds added at once |
| assault rifle, reloading | grenade thrown with timer 44 left | | 88 >= 87: reload lost, magazine stays empty |
| sniper rifle | zoom presses 1, 2, 3 | magnification | 2x, 8x, unzoomed |
| pistol | zoomed | field of view 70 deg base (nominal) | 35 deg nominal (vertical 22.73 deg, 0.39672 rad; unzoomed vertical 48.11 deg); autoaim angle 1.5 deg, range 60 WU, deviation 1.5 deg |
| random generator | seed 0 | first draw | seed 1013904223 (0x3C6EF35F); integer draw 0x3C6E = 15470; real 0.236057 |
| random generator | seed 0x3C6EF35F | integer range draw (0, 1026) | seed 0x47502932 (1196435762); r = 0x4750 = 18256; 1026 x 18256 >> 16 = 285 |
| plasma rifle | hold | rate of fire after 54 / 55 held ticks | 0.9999996 / 1.0 |

## Confidence per section

| Section | Basis |
|---|---|
| Tag layout (weapon part, magazine, ammunition object, trigger, firing effect, predicted resource) | checked against the disc: all 15 tags on 13 maps parse; every reference resolves to a tag of the expected class; enumerations in range; reals plausible; block element sizes confirmed by how the blocks pack (firing effects start 0x114 or 0x228 after the triggers, magazines 0x70 + 0x1C per ammunition object); runtime fields equal 1 / (30 t) |
| Field meanings | reference, checked for plausibility on the disc |
| Flag and enumeration names | reference and the tags' own usage on the disc |
| Multiplayer numbers and animation frame counts | checked against the disc |
| Runtime state, trigger state machine, firing, magazines, heat, battery, charging | reference only (the disc confirms only that the numbers it feeds make sense: 15/s, 12 rounds, 2x scope, 4 rounds and two zoom levels for the sniper, battery of 1) |
| Ticks between shots, reload and draw timelines, melee timing, heat timelines | worked out from the reference's rules with the disc's numbers (scratch simulation) |
| Shot origin, aim correction, aim assist, magnetism | reference only |
| Random generator, direction table size and order | reference only (the table's construction was rebuilt and checked for count and unit length; not compared with a recording) |
| Pickup and swap | reference only; the swap's 7 frames is from the disc's globals |
| Unused fields (ready time, reload time, error bounds, movement penalties, light times) | reference only: no code reads them |

## Open questions

1. **Plasma rifle top rate.** Does the Xbox game really fire the plasma
   rifle every 4 ticks once its battery is touched (7.5/s), or every 3
   (10/s)? Measure: record a full-auto burst at 60 fps from a fresh
   plasma rifle and count bolts per second after 2 s of holding.
2. **Direction table order.** Settled (section 7, "The direction
   table"): the order is fully determined by the construction. What is
   still unmeasured is that the Xbox build uses the same table as the
   reference's; a recording of a seeded shot pattern would confirm it.
3. **Convergence trace that hits nothing** within 128 WU: the point used
   is assumed to be the trace's end point. Check by shooting at the sky
   from a high place and comparing where bullets land against the
   crosshair at close range.
4. **Plasma pistol held 45 s.** The disc's first trigger has overcharged
   action none, so the gun should simply stay charged. Confirm in the
   game by holding the charge for a minute and then releasing it (a
   charged bolt, no vanishing gun, no early release).
5. **Melee damage scale.** Settled from the reference, not measured on
   hardware: the scale s (0 standing still, 1 at run speed, 1.5
   airborne, section 13) is the damage event's scale, and the amount is
   (1 - s) x minimum + U x s with U drawn between the damage effect's
   bounds (one draw per event, taken even when s = 0). A standing or
   backing-up melee therefore deals exactly the effect's minimum (40
   for every gun, 56 shield points after the 1.4 modifier), a run-speed
   melee deals U (50 to 60) and a lunge 1.5 U - 0.5 x minimum (55 to
   70); the "no distance scaling" flag of the melee effects does not
   matter because it only gates the area-damage distance scale. To
   measure: melee a shielded target standing still and running and
   compare shield loss.
6. **Ready and reload lengths.** The tags carry ready and reload times
   (assault rifle 1.3 s and 3.4 s) that the reference ignores in favour
   of animation frame counts (0.97 s and 2.9 s). Time a draw and a
   reload on video at 60 fps.
7. **Movement penalty.** The rocket launcher's tag asks for a 25 %
   penalty "when zoomed or reloading"; the reference never applies it.
   Compare walking speed with the rocket launcher zoomed and with the
   assault rifle.
8. **Input frames for the swap hold** are counted per input poll; on
   the Xbox that is assumed to be once per tick (7 ticks = 0.233 s).
9. **Auto pickup of a second weapon** in multiplayer: this spec follows
   the reference's statement of the Xbox rule (no auto pickup into the
   second slot; the action hold adds it). Check by spawning with one
   weapon (a game type option) and walking over another.
10. **Inventory weapons updating:** heat of a holstered plasma weapon
    drains in the reference (holstered weapons stay active). Check by
    overheating, switching away and back.
11. **Melee while zoomed** is ignored in the reference. Check with the
    pistol zoomed.
