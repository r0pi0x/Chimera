# Halo: Combat Evolved: the globals tag

Source basis: the game's use of its one globals tag (`matg`, path
`globals\globals`: player spawning and movement, grenades, falling
damage, the multiplayer game engine, the cheats, breakable surfaces and
vehicle ground physics, the first-person weapon view, player control),
read in a decompilation of an Xbox build, and every layout and number
checked against the Xbox disc's own map files: all 24 maps were parsed
with the layout below (13 multiplayer maps, 10 campaign maps, `ui`);
the 13 multiplayer maps are identical in every block, the campaign maps
differ only in the three blocks that are empty there.
Status: draft

## Summary

The globals tag is the one tag every map carries that holds game-wide
numbers instead of an object's: the player's walking, running and
airborne speeds, the stun and first-person idle numbers, the
falling-damage bounds, the grenade types, the multiplayer information
(the flag, the player's biped, the vehicles, the ball, the announcer's
43 sounds), the multiplayer weapon list, the table of 32 materials
(melee hit sound, vehicle ground friction scale, breakable glass), the
difficulty table, and the references to the interface's fonts, colour
tables and bitmaps. This spec is the owner of its **layout**; the
behaviour each number drives is in `movement.md`, `health_shields.md`,
`projectiles.md`, `weapons.md`, `animations.md` and
`multiplayer_rules.md`, which this spec points to instead of repeating.

## Units and conventions

All as `cache_file.md` defines: world units (WU, 3.048 m), +Z up, the
30 Hz tick, little-endian, a block field is 12 bytes (count, address,
unused), a tag reference is 16 bytes (class code, path address, path
length, tag id; on the disc the path length is 0 and a null reference has
tag id 0xFFFFFFFF), tags are stored as one main struct followed by its
blocks' elements. Offsets below are from the start of the element or the
main struct; "unused" ranges are zero on every map (checked).

- **Speeds** in the player information are WU **per second** (the engine
  divides them by 30 per tick); **accelerations** are WU/s gained per
  tick (so 0.32 is 0.0106667 WU/tick per tick). Times are seconds unless
  a field says ticks. Angles are radians unless marked.
- Element sizes were taken from the structure sizes the reference
  asserts and confirmed on the disc by how the blocks pack (each block's
  elements start where the previous block's end, children after their
  parent's whole block): the sums match exactly for every block below
  except where noted.

## The main struct (0x1AC bytes)

| Offset | Size | Type | Field | Meaning |
|---|---|---|---|---|
| 0x000 | 4 | flags32 | flags | 0 on every map; no code reads it |
| 0x004 | 244 | | unused | 61 words, zero |
| 0x0F8 | 12 | block | sounds | element 0x10 |
| 0x104 | 12 | block | camera | element 0x10 |
| 0x110 | 12 | block | player control | element 0x80 (plus a child block) |
| 0x11C | 12 | block | difficulty | element 0x284 |
| 0x128 | 12 | block | grenades | element 0x44 |
| 0x134 | 12 | block | rasterizer data | element 0x1AC |
| 0x140 | 12 | block | interface bitmaps | element 0x130 |
| 0x14C | 12 | block | weapon list | element 0x10 |
| 0x158 | 12 | block | cheat powerups | element 0x10 |
| 0x164 | 12 | block | multiplayer information | element 0xA0 |
| 0x170 | 12 | block | player information | element 0xF4 |
| 0x17C | 12 | block | first person interface | element 0xC0 |
| 0x188 | 12 | block | falling damage | element 0x98 |
| 0x194 | 12 | block | materials | element 0x374 (plus child blocks) |
| 0x1A0 | 12 | block | playlist | element 0x94 |

Counts on the disc:

| Block | 13 multiplayer maps | 10 campaign maps | `ui` |
|---|---|---|---|
| sounds | 2 | 2 | 2 |
| camera | 1 | 1 | 1 |
| player control | 1 | 1 | 1 |
| difficulty | 1 | 1 | 1 |
| grenades | 2 | 2 | 2 |
| rasterizer data | 1 | 1 | 1 |
| interface bitmaps | 1 | 1 | 1 |
| weapon list | 14 | 0 | 0 |
| cheat powerups | 5 | 0 | 0 |
| multiplayer information | 1 | 0 | 0 |
| player information | 1 | 1 | 1 |
| first person interface | 1 | 1 | 1 |
| falling damage | 1 | 1 | 0 |
| materials | 32 | 32 | 0 |
| playlist | 1 | 1 | 1 |

The block data is laid out in the order above, the children of a block
(the look function, the multiplayer vehicles and sounds, the breakable
surface particle effects) following the whole parent block. The main
struct is 0x1AC bytes: the first block's address on the disc is the tag's
address plus 0x1AC. A reader that finds a count of 0 for the weapon list,
multiplayer information, falling damage or materials must not read
element 0 (campaign maps, `ui`).

Every tag the blocks name resolves to a tag of the expected class on all
24 maps (checked); tags the globals name are in every map because they
are roots of its tag closure (`cache_file.md`).

## Player information (block 0x170, element 0xF4)

One element. The numbers that drive `movement.md`, the stun rule of
`health_shields.md`, the first-person view of `animations.md` and the
grenade release of `projectiles.md`. Disc values are identical on all
24 maps.

| Offset | Size | Type | Field | Unit | Disc | Meaning |
|---|---|---|---|---|---|---|
| 0x00 | 16 | tag ref `bipd` | player unit | | `characters\cyborg\cyborg` | the biped a player gets when no game engine is running (the campaign Spartan); in a multiplayer game the biped is the multiplayer information's unit (0x10 there) |
| 0x10 | 28 | | unused | | zero | 7 words |
| 0x2C | 4 | f32 | walking speed | WU/s | 0.512 | only used in the "alert" stance, never reached by a player in a match (`movement.md`); for a cinematic or a biped flagged "uses old player physics" the walking, run forward, backward, sideways speeds and run acceleration are replaced by 0.512, 2.25, 2.0, 2.0 and 0.32, the disc's own values |
| 0x30 | 4 | f32 | double speed multiplier | factor | 1.25 | a debug option only |
| 0x34 | 4 | f32 | run forward speed | WU/s | 2.25 | 0.075 WU/tick; also the melee's speed scale (`health_shields.md`) |
| 0x38 | 4 | f32 | run backward speed | WU/s | 2.0 | 0.0666667 WU/tick |
| 0x3C | 4 | f32 | run sideways speed | WU/s | 2.0 | |
| 0x40 | 4 | f32 | run acceleration | WU/s per tick | 0.32 | 0.0106667 WU/tick per tick: 8 ticks from rest to full run |
| 0x44 | 4 | f32 | sneak forward speed | WU/s | 0.9 | |
| 0x48 | 4 | f32 | sneak backward speed | WU/s | 0.65 | |
| 0x4C | 4 | f32 | sneak sideways speed | WU/s | 0.6 | |
| 0x50 | 4 | f32 | sneak acceleration | WU/s per tick | 0.16 | |
| 0x54 | 4 | f32 | airborne acceleration | WU/s per tick | 0.035 | 0.00116667 WU/tick per tick; steers the horizontal velocity in the air |
| 0x58 | 4 | f32 | multiplayer only speed multiplier | factor | 0 | read by nothing in the game (`movement.md`) |
| 0x5C | 12 | | unused | | zero | 3 words |
| 0x68 | 12 | 3 x f32 | grenade origin | WU | (0, 0.05, 0) | where a released grenade starts relative to the camera, in the aim's frame: the three offsets along the aim, to its left and above it; so 0.05 WU to the left (`projectiles.md`) |
| 0x74 | 12 | | unused | | zero | 3 words |
| 0x80 | 4 | f32 | stun movement penalty | factor | 0.8 | `movement.md`, `health_shields.md` |
| 0x84 | 4 | f32 | stun turning penalty | factor | 0.8 | scales the player's jump (`movement.md`, "Damage stun"); it does not scale the turn rate, despite its name |
| 0x88 | 4 | f32 | stun jumping penalty | factor | 1.0 | not read (`movement.md`) |
| 0x8C | 4 | f32 | minimum stun time | s | 0 | the body stun's floor, times 30 and truncated to ticks (0) |
| 0x90 | 4 | f32 | maximum stun time | s | 1.25 | its ceiling: 37 ticks (1.25 x 30 = 37.5 truncated) |
| 0x94 | 8 | | unused | | zero | 2 words |
| 0x9C | 4 | f32 | first person idle time lower bound | s | 10 | `animations.md`: the wait before the first-person weapon may pose, drawn uniformly between the two bounds, times 30 |
| 0xA0 | 4 | f32 | first person idle time upper bound | s | 15 | (the disc's values are 10 s, 15 s and 0.25 on all 24 maps; 240 and 360 ticks) |
| 0xA4 | 4 | f32 | first person idle skip fraction | fraction (0 to 1) | 0.25 | the chance a draw ends the wait without posing |
| 0xA8 | 16 | | unused | | zero | 4 words |
| 0xB8 | 16 | tag ref `effe` | coop respawn effect | | `effects\coop teleport` | played at a player who respawns near a partner in co-op; no multiplayer use |
| 0xC8 | 44 | | unused | | zero | 11 words; the element ends at 0xF4 |

Unit checks against the other specs: every speed and acceleration value
equals the one `movement.md` lists as read from `matg`; the stun
penalties, times and the run forward speed are those `health_shields.md`
lists; the idle bounds are in seconds because the animation update
multiplies them by 30; the grenade origin is the (0, 0.05, 0) that
`projectiles.md` states.

## Falling damage (block 0x188, element 0x98)

One element, absent from `ui`. `movement.md` owns the behaviour (the
speed at impact decides the damage), `health_shields.md` the damage
formula, `projectiles.md` the damage effects.

| Offset | Size | Type | Field | Unit | Disc | Meaning |
|---|---|---|---|---|---|---|
| 0x00 | 8 | | unused | | zero | 2 words |
| 0x08 | 4 | f32 | harmful falling distance, lower bound | WU | 3.0 | a fall shorter than this does no damage |
| 0x0C | 4 | f32 | harmful falling distance, upper bound | WU | 6.0 | at this fall the damage effect is applied at full scale |
| 0x10 | 16 | tag ref `jpt!` | falling damage | | `globals\falling` | |
| 0x20 | 8 | | unused | | zero | 2 words |
| 0x28 | 4 | f32 | maximum falling distance | WU | 16.5 | at this fall the biped dies in the air (lethal) |
| 0x2C | 16 | tag ref `jpt!` | maximum distance damage | | `globals\distance` | the lethal fall |
| 0x3C | 16 | tag ref `jpt!` | vehicle hit environment damage | | `globals\vehicle_hit_environment` | all zero damage on the disc |
| 0x4C | 16 | tag ref `jpt!` | vehicle killed unit damage | | `globals\vehicle_killed_unit` | |
| 0x5C | 16 | tag ref `jpt!` | vehicle collision damage | | `globals\vehicle_collision` | 1000 points (`projectiles.md`) |
| 0x6C | 16 | tag ref `jpt!` | flaming death damage | | `globals\flaming_death` | 5000 points |
| 0x7C | 16 | | unused | | zero | 4 words |
| 0x8C | 4 | f32 | runtime: maximum falling velocity | WU/tick | 0.3430028 | the build tool precomputes it as sqrt(2 g d) with d the maximum distance and g = 0.0035651792 WU/tick^2 |
| 0x90 | 4 | f32 | runtime: minimum damage velocity | WU/tick | 0.14625688 | the same for the lower bound |
| 0x94 | 4 | f32 | runtime: maximum damage velocity | WU/tick | 0.20683846 | the same for the upper bound |

The three runtime values equal sqrt(2 g d) for d = 16.5, 3.0 and 6.0 to
under 2e-7 on every map; the engine reads them, not the distances.

There is no gravity field anywhere in the globals: the 61 words at
0x004 are zero and no block holds a gravity scale; gravity is the
engine's constant (`cache_file.md`, `movement.md`).

## Grenades (block 0x128, element 0x44)

Two elements: type 0 frag, type 1 plasma (the order the unit's grenade
type, the HUD and `projectiles.md` use).

| Offset | Size | Type | Field | Unit | Meaning |
|---|---|---|---|---|---|
| 0x00 | 2 | i16 | maximum count | grenades | the most a unit carries of this type |
| 0x02 | 2 | i16 | mp spawn default | grenades | the count a multiplayer spawn starts with unless the game engine says otherwise (`multiplayer_rules.md` section 4.6: frag 4 and plasma 0 there come from the game type, not from here) |
| 0x04 | 16 | tag ref `effe` | throwing effect | | played when the grenade leaves the hand |
| 0x14 | 16 | tag ref `grhi` | hud interface | | the grenade HUD's definition |
| 0x24 | 16 | tag ref `eqip` | item | | the pickup |
| 0x34 | 16 | tag ref `proj` | projectile | | the thrown projectile |

| Element | Max count | MP spawn default | Throwing effect | HUD interface | Item | Projectile |
|---|---|---|---|---|---|---|
| 0 frag | 4 | 0 | `weapons\frag grenade\effects\throw` | `ui\hud\default` | `weapons\frag grenade\frag grenade` | `weapons\frag grenade\frag grenade` |
| 1 plasma | 4 | 0 | `weapons\plasma grenade\effects\throw` | `ui\hud\plasma_flare` | `weapons\plasma grenade\plasma grenade` | `weapons\plasma grenade\plasma grenade` |

(The reference never reads the `mp spawn default` of type 1; it is 0
for both on the disc.)

## Multiplayer information (block 0x164, element 0xA0)

One element on multiplayer maps; none on campaign maps and `ui`.
Behaviour: `multiplayer_rules.md` (sections 5, 6, 9 and 11).

| Offset | Size | Type | Field | Meaning |
|---|---|---|---|---|
| 0x00 | 16 | tag ref `weap` | flag | the capture-the-flag weapon `weapons\flag\flag` |
| 0x10 | 16 | tag ref `bipd` | unit | the player's biped in a match: `characters\cyborg_mp\cyborg_mp` |
| 0x20 | 12 | block | vehicles | element 0x10: 3 tag references `vehi`, in the order `vehicles\warthog\warthog`, `vehicles\ghost\ghost_mp`, `vehicles\scorpion\scorpion_mp` (the vehicle sets of the game types pick among them) |
| 0x2C | 16 | tag ref `sotr` | hill shader | `scenery\hilltop\shaders\hilltop` |
| 0x3C | 16 | tag ref `sotr` | flag shader | `weapons\flag\shaders\flag_blue` |
| 0x4C | 16 | tag ref `weap` | ball | `weapons\ball\ball` |
| 0x5C | 12 | block | sounds | element 0x10: 43 tag references `snd!` (the announcer's lines, below) |
| 0x68 | 56 | | unused | zero |

The vehicles' child block starts right after the element (0x30 bytes),
the sounds' after it (0x2B0 bytes), so the element and its two children
fill 0x380 bytes.

### The 43 sounds

Every path begins `sound\`; 0 to 25, 32 to 41 are dialogue lines in
`dialog\multiplayer1\`, 26 to 31 and 42 are user-interface sounds in
`sfx\ui\`. When and how each plays (queue rules, lengths) is
`multiplayer_rules.md` section 9, whose table is in this order (checked
identical to the disc, all 43).

| # | Sound | # | Sound |
|---|---|---|---|
| 0 | play_ball | 22 | capture_the_flag |
| 1 | game_over | 23 | warthog |
| 2 | one_minute_to_win | 24 | ghost |
| 3 | 30_seconds_to_win | 25 | scorpion |
| 4 | red_team_minute_to_win | 26 | (ui) countdown_timer |
| 5 | red_team_30_to_win | 27 | (ui) teleporter_activate |
| 6 | blue_team_minute_to_win | 28 | (ui) flag_failure |
| 7 | blue_team_30_to_win | 29 | (ui) countdown_for_respawn |
| 8 | blue_team_has_the_flag | 30 | (ui) hill_move |
| 9 | blue_team_flag_returned | 31 | (ui) player_respawn |
| 10 | blue_team_score | 32 | team_king_of_the_hill |
| 11 | red_team_has_the_flag | 33 | team_oddball |
| 12 | red_team_flag_returned | 34 | team_race |
| 13 | red_team_score | 35 | team_slayer |
| 14 | double_kill | 36 | king_of_the_hill |
| 15 | triple_kill | 37 | blue_team_ctf |
| 16 | killtacular | 38 | red_team_ctf |
| 17 | running_riot | 39 | hill_contested |
| 18 | killing_spree | 40 | hill_controlled |
| 19 | oddball | 41 | hill_occupied |
| 20 | race | 42 | (ui) countdown_timer_end |
| 21 | slayer | | |

## Weapon list (block 0x14C) and cheat powerups (block 0x158)

Both are blocks of 16-byte tag references; empty on campaign maps and
`ui`.

- **Weapon list**, 14 entries on multiplayer maps, in the order of the
  table below. The multiplayer game engine addresses it by these
  positions (a game type's weapon set names positions in it;
  `multiplayer_rules.md` 5.3). The weapons cheat uses the same list: it
  creates every entry in an arc in front of the player (1.5 WU from his
  origin, 0.8 WU above it); with an empty list it uses every weapon tag
  of the map instead.
- **Cheat powerups**, 5 `eqip` entries, in this order:
  `powerups\active camouflage`, `powerups\double speed`,
  `powerups\full-spectrum vision`, `powerups\over shield`,
  `powerups\health pack`. The powerups cheat creates all five the same way.

The weapon list, in order:

| # | Class | Tag |
|---|---|---|
| 0 | `weap` | `weapons\assault rifle\assault rifle` |
| 1 | `weap` | `weapons\flamethrower\flamethrower` |
| 2 | `weap` | `weapons\gravity rifle\gravity rifle` |
| 3 | `weap` | `weapons\needler\needler` |
| 4 | `weap` | `weapons\pistol\pistol` |
| 5 | `weap` | `weapons\plasma pistol\plasma pistol` |
| 6 | `weap` | `weapons\plasma rifle\plasma rifle` |
| 7 | `weap` | `weapons\rocket launcher\rocket launcher` |
| 8 | `weap` | `weapons\shotgun\shotgun` |
| 9 | `weap` | `weapons\sniper rifle\sniper rifle` |
| 10 | `weap` | `weapons\ball\ball` |
| 11 | `weap` | `weapons\flag\flag` |
| 12 | `eqip` | `weapons\frag grenade\frag grenade` |
| 13 | `eqip` | `weapons\plasma grenade\plasma grenade` |

## Materials (block 0x194, element 0x374)

32 elements on every map except `ui` (0): one per material type 0 to 31
in the order of `collision.md` (0 dirt, 1 sand, 2 stone, 3 snow, 4 wood,
5 hollow metal, 6 thin metal, 7 thick metal, 8 rubber, 9 glass, 10 force
field, 11 grunt, 12 hunter armor, 13 hunter skin, 14 elite, 15 jackal,
16 jackal energy shield, 17 engineer, 18 engineer force field, 19 flood
combat form, 20 flood carrier form, 21 cyborg, 22 cyborg energy shield,
23 armored human, 24 human, 25 sentinel, 26 monitor, 27 plastic, 28
water, 29 leaves, 30 elite energy shield, 31 ice). **Type 32 (hunter
shield) has no element**: a type with no element gets the engine's
built-in default (all zero, melee hit sound none), the same rule as the
projectile response tables of `projectiles.md`. The block is read by
index = material type of the surface or of the damage hit.

| Offset | Size | Type | Field | Unit | Meaning |
|---|---|---|---|---|---|
| 0x000 | 4 | flags32 | flags | | 0 on every element; not read |
| 0x004 | 96 | | unused | | zero (24 words) |
| 0x064 | 4 | flags32 | biped flags | | 0 everywhere; not read |
| 0x068 | 4 | f32 | biped maximum acceleration | | 0 everywhere; not read |
| 0x06C | 4 | f32 | biped slip angle | | 0 everywhere; not read |
| 0x070 | 4 | f32 | biped slow angle | | 0 everywhere; not read |
| 0x074 | 32 | | unused | | zero |
| 0x094 | 4 | f32 | ground friction scale | factor | multiplies a vehicle physics object's ground friction while its wheel contact point is on this material; 0 = no scaling; applied only to objects whose physics mass is at most 7500 |
| 0x098 | 4 | f32 | ground friction normal k1 scale | factor | scales the physics tag's normal k1 on this material; 0 = none |
| 0x09C | 4 | f32 | ground friction normal k0 scale | factor | the same for k0 |
| 0x0A0 | 4 | f32 | ground depth scale | factor | the same for the ground depth |
| 0x0A4 | 4 | f32 | ground damp fraction scale | factor | the same for the damp fraction |
| 0x0A8 | 76 | | unused | | zero |
| 0x0F4 | 480 | | unused | | zero (120 words) |
| 0x2D4 | 4 | f32 | breakable surface: maximum vitality | points | 0 = the material cannot break; else the damage points that break one of its breakable surfaces |
| 0x2D8 | 8 | | unused | | zero |
| 0x2E0 | 4 | flags32 | breakable surface: flags | | 0 everywhere |
| 0x2E4 | 16 | tag ref `effe` | breakable surface: effect | | null on every element |
| 0x2F4 | 16 | tag ref `snd!` | breakable surface: sound | | played when it breaks (glass only) |
| 0x304 | 24 | | unused | | zero |
| 0x31C | 12 | block | breakable surface: particle effects | | element 0x80 (below); the disc has 1 on glass, 0 elsewhere |
| 0x328 | 60 | | unused | | zero (15 words) |
| 0x364 | 16 | tag ref `snd!` | melee hit sound | | played, from the attacking unit, when its melee hits something of this material (`weapons.md`) |

The "physics" ground fields are the vehicle ground-contact model's:
`ground friction scale` and its four siblings multiply, for a mass
point whose ground material is this one, the values the vehicle's
physics tag (`phys`) gives for friction, normal force constants, ground
depth and damping; each applies only when it is above 0. That is where
**vehicle terrain physics per material** come from. Footstep sounds and
dust do not come from here: they come from the biped's footsteps tag
(`foot`, `globals\cyborg`; `effects.md`), indexed by the same material
types.

Disc values (identical everywhere):

- The only non-zero physics field is **ice (31): ground friction scale
  0.125**, so a light vehicle on ice (none of the multiplayer maps has
  ice terrain) has an eighth of its normal friction.
- **Glass (9)** is the only breakable material: maximum vitality 10,
  sound `sound\sfx\impulse\glass\glass_medium`, one particle effect.
- Melee hit sounds, all under `sound\sfx\impulse\melee\`: dirt and sand
  `melee_dirt`; stone `melee_concrete`; snow `melee_snow`; wood
  `melee_wood`; hollow and thin metal `melee_grating`; thick metal
  `melee_metal`; rubber and plastic `melee_rubber`; glass and ice
  `melee_glass`; hunter armor, armored human, sentinel and monitor
  `melee_armor`; the organic types (grunt, hunter skin, elite, jackal,
  engineer, flood forms, cyborg, cyborg energy shield, human)
  `melee_impact_fleshy`; water `melee_water`; leaves `melee_grass`; and
  under `sound\sfx\weapons\shield fx\` the sound `hits` for force field,
  jackal shield, engineer force field and elite energy shield.

**Breakable surfaces** (reference rule, glass on `prisoner`): a breakable
surface of a level starts with vitality 1.0. A hit on it (a projectile
or a damage effect that reaches it; area damage too) takes a damage
number of the effect's range (lower to upper bound, the effect's minimum
and scale rule as in `health_shields.md`), multiplied by the damage
effect's modifier for the material type of the surface, divided by the
material's maximum vitality, and subtracted from the surface's vitality;
at 0 or below the surface breaks (its collision stops, `collision.md`)
and the material's sound and particle effects play. A material whose
maximum vitality is 0 never takes this damage.

### Breakable surface particle effect (element 0x80)

| Offset | Size | Type | Field | Unit | Glass | Meaning |
|---|---|---|---|---|---|---|
| 0x00 | 16 | tag ref `part` | particle | | `effects\particles\solid\glass breakable` | |
| 0x10 | 4 | flags32 | flags | | 0 | bit 0 interpolate colour in HSV, bit 1 along the farthest hue path |
| 0x14 | 4 | f32 | density | per unit area | 0.175 | shards per area of broken surface (unit not traced) |
| 0x18 | 4 | f32 | velocity scale lower bound | factor | 0.4 | |
| 0x1C | 4 | f32 | velocity scale upper bound | factor | 0.6 | |
| 0x20 | 4 | f32 | velocity cone angle | rad | 0 | |
| 0x24 | 4 | f32 | angular velocity lower bound | rad/s | 1.0471976 (60 degrees) | |
| 0x28 | 4 | f32 | angular velocity upper bound | rad/s | 3.4906585 (200 degrees) | |
| 0x2C | 8 | | unused | | zero | |
| 0x34 | 4 | f32 | radius lower bound | WU | 0.05 | |
| 0x38 | 4 | f32 | radius upper bound | WU | 0.2 | |
| 0x3C | 8 | | unused | | zero | |
| 0x44 | 16 | 4 x f32 | tint lower bound (a, r, g, b) | | (1, 1, 1, 1) | |
| 0x54 | 16 | 4 x f32 | tint upper bound | | (1, 1, 1, 1) | |
| 0x64 | 28 | | unused | | zero | 7 words |

## The other blocks (in brief)

- **Sounds (0x0F8, element 0x10, 2 elements):** tag references `snd!`:
  `sound\sfx\impulse\coolant\enter_water` and `...\exit_water` (the
  sounds of a unit or object crossing a water surface; their use was not
  traced).
- **Camera (0x104, element 0x10, 1 element):** a tag reference `trak`:
  `globals\default unit camera track` (the camera track a unit uses when
  its own has none).
- **Player control (0x110, element 0x80, 1 element, plus the look
  function block):** the table below.
- **Difficulty (0x11C, element 0x284, 1 element):** 35 rows of 4 f32
  (easy, normal, heroic, legendary), then 21 unused zero words. Rows 0 to
  7 are the damage, vitality, shield and recharge multipliers of
  enemies (0 to 3) and friends (4 to 7): normal 1.0 in all of them
  (easy 0.3, 0.6, 0.6, 0.5 and 0.4, 0.8, 0.8, 0.5; heroic 1.4, 1.2, 1.2,
  1.5 and 1.4, 1.2, 1.2, 1.5; legendary 1.8, 1.4, 1.4, 2.0 and 1.8,
  1.4, 1.4, 2.0); a multiplayer game reads the normal column
  (`health_shields.md`). Row 19 is the guidance rate scale against
  players (0.5, 0.5, 0.75, 1.0: `projectiles.md`). The other rows hold
  the AI's numbers (infection forms, rate of fire, projectile and burst
  error, new target delay, burst separation, target tracking and leading,
  overcharge chance, special fire delay, melee delay, grenade chance
  and timer scales, and three upgrade rows 28 to 30); rows 9, 22, 25 to 27
  and 31 to 34 are zero. None matters to a multiplayer match.
- **Rasterizer data (0x134, element 0x1AC, 1 element):** 7 `bitm`
  references (0x00: distance attenuation, vector normalization,
  atmospheric fog density, planar fog density, linear corner fade, active
  camouflage distortion, glow: `rasterizer\...`), 60 unused bytes (0x70),
  3 default textures (0xAC: `rasterizer\default 2d`, `default 3d`,
  `default cube map`), 4 test bitmaps (0xDC:
  `ui\shell\bitmaps\postgame_carnage_report`,
  `characters\jackal\bitmaps\shield noise`, `rasterizer\video noise`,
  `rasterizer\reference bump`), the video screen effect's scanline mask (0x11C, `rasterizer\video mask`) and noise
  map (0x12C, `rasterizer\video noise`), 52 unused bytes (0x13C), then at
  0x170 u16 active camouflage flags (0) and 2 bytes padding, f32
  refraction amount 11.0 (0x174), f32 distance falloff 22.0 (0x178),
  an RGB tint (0.8431, 0.8392, 0.9137) (0x17C), the hyper-stealth
  refraction amount 9.0 (0x188), falloff 9.0 (0x18C) and tint (1, 1, 1)
  (0x190), and a null `bitm` reference (0x19C: the PC's 2D attenuation).
  `shaders.md` and `effects.md` use these.
- **Interface bitmaps (0x140, element 0x130, 1 element):** 16 tag
  references (0x10 each) then 48 unused bytes (0x100). In order:
  0 font `ui\interstate` (the system font), 1 font
  `old tags\internal system plain` (terminal), 2 to 5 colour tables `colo`
  (`old tags\internal screen`, `hud`, `editor`, `dialog`), 6 HUD
  globals `hudg` `ui\hud\default` (`multiplayer_rules.md` 11.1 reads its
  motion sensor numbers), 7 and 8 the motion sensor's sweep bitmap and
  mask (`ui\hud\bitmaps\hud_sweeper`, `hud_sweeper_mask`), 9 the
  multiplayer HUD bitmap `hud_multiplayer`, 10 the localisation string
  list `str#` `old tags\internal string localization`, 11 the HUD number
  tag `hud#` `ui\hud\counter`, 12 the motion blip bitmap
  `hud_sensor_blip`, 13 `hud_sensor_blip_custom`, 14 `hud_msg_icons_sm`
  and 15 `characters\jackal\bitmaps\shield noise`.
- **First person interface (0x17C, element 0xC0, 1 element):** 0x00 `mode`
  the arms model `characters\cyborg\fp\fp` (37 nodes, `animations.md`),
  0x10 `bitm` `ui\hud\bitmaps\hud_health_base`, 0x20 `metr` the shield
  meter `ui\hud\cyborg shield`, 0x30 its origin as two i16 (x 64, y 16),
  0x34 `metr` the body meter `ui\hud\cyborg body`, 0x44 its origin (64,
  16), 0x48 `effe` the night vision on effect
  `sound\sfx\weapons\sniper rifle\night_vision_on`, 0x58 `effe` the off
  effect `...\night_vision_off`, 0x68 88 unused bytes.
- **Playlist (0x1A0, element 0x94, 1 element):** a leftover playlist
  entry, the same on all 24 maps: a 32-byte string
  `levels\\test\\carousel\\carouse` (written with doubled
  backslashes and cut at 31 characters), a 32-byte string `slayer`, 4 zero bytes at 0x40, then
  i32 10 (0x44), 1 (0x48), 16 (0x4C), 100 (0x50) and zeros to 0x94. Nothing
  in the game reads it that was found; ignore it.

### Player control (element 0x80)

| Offset | Size | Type | Field | Disc |
|---|---|---|---|---|
| 0x00 | 4 | f32 | magnetism friction | 0.6 (`weapons.md`) |
| 0x04 | 4 | f32 | magnetism adhesion | 0.7 |
| 0x08 | 4 | f32 | magnetism inconsequential target scale | 0.5 |
| 0x0C | 52 | | unused | zero (13 words) |
| 0x40 | 4 | f32 | look acceleration time | 1.0 s |
| 0x44 | 4 | f32 | look acceleration scale | 3.0 |
| 0x48 | 4 | f32 | look pegging threshold | 0.85 (fraction of full stick) |
| 0x4C | 4 | f32 | look default pitch rate | 60 (degrees per second, inferred from the size) |
| 0x50 | 4 | f32 | look default yaw rate | 120 |
| 0x54 | 4 | f32 | look autolevel scale | 0.5 |
| 0x58 | 20 | | unused | zero |
| 0x6C | 2 | i16 | minimum weapon swap ticks | 7 (`weapons.md`: input frames the action button is held) |
| 0x6E | 2 | i16 | minimum autolevel enabled ticks | 15 |
| 0x70 | 4 | f32 | minimum vehicle flipping angle | 0.7853982 rad (45 degrees) |
| 0x74 | 12 | block | look function | 6 elements of one f32: 0, 0.05, 0.1, 0.25, 0.58, 1.0 |

The look function is the piecewise-linear curve from stick deflection
(0 to 1) to the fraction of the turn rate.

## Test cases

### Synthetic player information element (unit test)

A 0xF4-byte element, every byte zero except the words below (all values
are exact binary fractions, chosen so each field has a distinct value and
a reader that is off by one word fails). **These are made-up test values,
not the disc's** (the idle bounds here are 8 s, 12 s and 0.5; the disc's
10 s, 15 s and 0.25 are in the field table above and in "Disc-backed"
below); a test that reads a map must expect the disc's numbers:

| Offset | Word | Value | Field |
|---|---|---|---|
| 0x00 | 0x62697064, 0, 0, 0xFFFFFFFF | `bipd`, null | player unit |
| 0x2C | 0x3F000000 | 0.5 | walking speed |
| 0x30 | 0x3FC00000 | 1.5 | double speed multiplier |
| 0x34 | 0x40200000 | 2.5 | run forward speed |
| 0x38 | 0x40100000 | 2.25 | run backward speed |
| 0x3C | 0x40080000 | 2.125 | run sideways speed |
| 0x40 | 0x3EC00000 | 0.375 | run acceleration |
| 0x44 | 0x3F600000 | 0.875 | sneak forward speed |
| 0x48 | 0x3F200000 | 0.625 | sneak backward speed |
| 0x4C | 0x3F100000 | 0.5625 | sneak sideways speed |
| 0x50 | 0x3E400000 | 0.1875 | sneak acceleration |
| 0x54 | 0x3D000000 | 0.03125 | airborne acceleration |
| 0x58 | 0x3E800000 | 0.25 | multiplayer only speed multiplier |
| 0x68 | 0x3E000000 | 0.125 | grenade origin i |
| 0x6C | 0x3D800000 | 0.0625 | grenade origin j |
| 0x70 | 0x3D000000 | 0.03125 | grenade origin k |
| 0x80 | 0x3F400000 | 0.75 | stun movement penalty |
| 0x84 | 0x3F200000 | 0.625 | stun turning penalty |
| 0x88 | 0x3F600000 | 0.875 | stun jumping penalty |
| 0x8C | 0x3F000000 | 0.5 | minimum stun time |
| 0x90 | 0x40000000 | 2.0 | maximum stun time |
| 0x9C | 0x41000000 | 8.0 | first person idle time lower bound |
| 0xA0 | 0x41400000 | 12.0 | first person idle time upper bound |
| 0xA4 | 0x3F000000 | 0.5 | first person idle skip fraction |
| 0xB8 | 0x65666665, 0, 0, 0xFFFFFFFF | `effe`, null | coop respawn effect |

Expected:

| Check | Expected |
|---|---|
| each field above | the value in the table (bit-exact) |
| unused ranges 0x10-0x2B, 0x5C-0x67, 0x74-0x7F, 0x94-0x9B, 0xA8-0xB7, 0xC8-0xF3 | all zero; a reader must not touch them |
| tag references | class `bipd` and `effe`, null (tag id 0xFFFFFFFF) |
| per-tick forms (speed / 30) | run forward 0.0833333, run backward 0.075, run sideways 0.0708333, walking 0.0166667 WU/tick; run acceleration 0.0125 WU/tick per tick |
| stun time in ticks (seconds x 30, truncated) | minimum 15, maximum 60 |
| idle wait bounds in ticks | 240 and 360 |
| element size | 0xF4; the next field after the element starts there |
| an element of 0xF3 bytes or fewer | rejected |

### Synthetic falling damage element (unit test)

0x98 bytes, zero except: 0x08 = 3.0 (0x40400000), 0x0C = 6.0 (0x40C00000),
0x28 = 16.5 (0x41840000), 0x8C = 0.3430028 (0x3EAF9E10), 0x90 =
0.14625688 (0x3E15C45D), 0x94 = 0.20683846 (0x3E53CD76). Expected: the
bounds read as 3.0, 6.0 and 16.5 WU; each runtime velocity equals
sqrt(2 x 0.0035651792 x d) within 2e-7 for d = 16.5, 3.0 and 6.0; every
range other than those words and the tag references is zero.

### Synthetic grenade element (unit test)

0x44 bytes: i16 4 at 0x00, i16 0 at 0x02, then four null tag references
of classes `effe`, `grhi`, `eqip`, `proj` at 0x04, 0x14, 0x24 and 0x34.
Expected: maximum count 4, mp spawn default 0, four references of those
classes at those offsets, element size 0x44; a block of two such
elements has its second at +0x44.

### Disc-backed (slow tests)

| Map | Check | Expected |
|---|---|---|
| all 24 | main struct | flags 0 and 61 zero words; block counts as the table of counts |
| all 13 multiplayer | block contents | identical on all 13 (same references, same reals) |
| all 24 | padding | every unused range listed in this spec is zero |
| all 24 | tag references | every non-null reference resolves to a tag of the class stated |
| bloodgulch | player information | the disc column of its table: walking 0.512, double 1.25, run 2.25 / 2.0 / 2.0, run acceleration 0.32, sneak 0.9 / 0.65 / 0.6, sneak acceleration 0.16, airborne 0.035, multiplayer speed multiplier 0, grenade origin (0, 0.05, 0), stun 0.8 / 0.8 / 1.0, stun times 0 and 1.25, idle 10, 15, 0.25, player unit `characters\cyborg\cyborg`, coop effect `effects\coop teleport` |
| bloodgulch | falling damage | 3.0, 6.0, 16.5; `globals\falling`, `globals\distance`; runtime 0.3430028, 0.14625688, 0.20683846 |
| bloodgulch | grenades | 2 elements, maximum count 4 and 4, mp spawn default 0 and 0, the references of the grenade table |
| bloodgulch | multiplayer information | flag `weapons\flag\flag`, unit `characters\cyborg_mp\cyborg_mp`, 3 vehicles in the stated order, 43 sounds in the stated order, hill shader, flag shader, ball |
| bloodgulch | weapon list, cheat powerups | 14 and 5 entries in the stated order |
| bloodgulch | materials | 32 elements; ice ground friction scale 0.125, everything else in the physics fields 0; glass maximum vitality 10, breakable sound `glass_medium`, 1 particle effect (density 0.175, radius 0.05 to 0.2); melee hit sound per type as listed |
| a10 | blocks | weapon list, cheat powerups and multiplayer information empty; player information, falling damage and materials equal bloodgulch's |
| ui | blocks | falling damage and materials empty; player information equal except the player unit reference (null) |

## Confidence per section

| Section | Confidence |
|---|---|
| Main struct, block offsets, counts | checked against the disc (all 24 maps) |
| Element sizes | structure sizes from the reference, confirmed on the disc by block spacing and by all-map parse with no overlap and zero padding |
| Player information fields, units | offsets and values checked against the disc; units of speeds and accelerations (per second, divided by 30) from the reference and `movement.md`; the stun ticks and idle bounds from the reference |
| Falling damage | checked against the disc (the runtime velocities equal sqrt(2 g d) to 2e-7); roles of the damage references from the reference |
| Grenades | checked against the disc; use from the reference |
| Multiplayer information, 43 sounds, vehicles | checked against the disc; use is `multiplayer_rules.md`'s |
| Weapon list, cheat powerups | checked against the disc; the cheat rings from the reference |
| Materials: layout and values | checked against the disc (all 32 elements, all maps); meanings of the physics scales and the breakable rule from the reference only |
| Breakable surface particle effect fields | layout checked against the disc (glass); the units of density and the angular velocities inferred |
| Player control, difficulty, rasterizer, interface bitmaps, first person interface, camera, sounds | layout and values checked against the disc; meaning from the reference or inferred |
| Playlist | layout from the disc only (size from block spacing); meaning unknown |

## Open questions

1. **Unread fields.** The material flags, the biped flags, maximum
   acceleration, slip and slow angles, the breakable surface flags and
   effect, the globals flags and the player information's stun jumping
   penalty and multiplayer only speed multiplier are zero or unread in
   the reference. Whether the Xbox executable reads any of them can be
   checked in the executable's disassembly or by changing one in a
   test map.
2. **Water enter and exit sounds** (the globals' sounds block): which
   event plays them and with what falloff was not traced.
3. **Playlist block.** Only its shape is known; whether the menus read
   it (it looks like a leftover default playlist of the build tool) is
   open: a reader should skip it.
4. **Units of the breakable particle effect.** Density per unit area and
   the exact scale of the velocity fields were not traced; the glass
   shards are cosmetic (`effects.md`).
5. **Default look rates.** 60 and 120 are read as degrees per second from
   their size; confirm against a controller's full-stick turn speed.
6. **Material type 32.** A hunter shield hit uses the built-in default
   (no element); no multiplayer map has one, so it is untestable there.
