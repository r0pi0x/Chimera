# Halo: Combat Evolved: the campaign's characters as playable bodies

Source basis: the game's own map files from the original Xbox disc (all
ten campaign maps, a10 to d40, read whole with throwaway scripts: every
biped tag, its model, collision model, animation graph and dialogue,
every actor variant tag, every scenario's actor palette and encounters,
every weapon and damage effect; Blood Gulch for the multiplayer
weapons' damage effects and the Spartan's comparison); and a
decompilation of an Xbox build of the game's engine (unit, biped,
dialogue, actor and damage code), used for meaning only. The
decompilation describes a different Xbox build (PAL 01.01.14.2342)
from the disc's (NTSC 01.10.12.2276); where they could differ the disc
wins. Conventions (tag index, blocks, references, units, axes, the
tick) are in [cache_file.md](cache_file.md); the object, unit and
biped layers in [objects.md](objects.md); graphs in
[animations.md](animations.md); vitality and damage in
[health_shields.md](health_shields.md); dialogue playback in
[sounds.md](sounds.md); the player's movement in
[movement.md](movement.md). None of that is repeated here.
Status: draft

Sections: Summary; Units and conventions; Constants; 1. The roster;
2. Which weapons each may hold; 3. The body; 4. Movement; 5. Vitality;
6. Voice; 7. First person; 8. Each character as a unit to fetch;
Per-tick order; Edge cases; Quirks; Test cases; Confidence per
section; Open questions.

---

## Summary

- The ten campaign maps hold 42 distinct biped tags. Eighteen are
  walking combatants (counting rank and weapon variants and the Flood's
  carrier and infection forms); the rest are the Spartans, story
  bipeds, wounded props, flying units or cinematic copies. The sensible
  playable bodies are the **Elite**, **Grunt**, **Jackal**, **Hunter**,
  **Marine** (and crewman), and the **Flood combat forms** (Elite and
  human). The carrier and infection forms, the Sentinel, the Monitor
  and the Engineer are not humanoid shooters and are listed for
  completeness.
- **The Spartan alone uses "player physics".** Every other character
  moves by its animations: each tick its velocity is the current
  movement animation's root motion for that frame, chosen from four
  directions by the dominant stick axis, with no acceleration limit and
  no air control. That is what makes a Grunt lumber and wobble: its
  standing walk cycle moves it between 0.054 and 0.069 WU per tick
  inside one 16-frame cycle. Motion starts and stops in one tick, one
  tick after the stick, at full speed whatever the stick's size. A
  Hunter and the Flood combat Elite have no backward or sideways walk
  at all: pushed that way they stop dead in their default pose. Every
  character can jump (the Flood combat forms 3.5 WU high) and, with
  most weapons, crouch.
- **What a character may hold is decided by its animation graph**: a
  weapon whose label (`ar`, `hp`, `pr` ...) is not a weapon type of the
  graph's `stand` seat cannot be selected. No character's graph covers
  more than eight of the ten multiplayer weapon labels with animations.
  The Elite, Grunt and Jackal hold only Covenant weapons (the Elite
  also the human pistol, without fire or reload animations); the
  Marine only human guns plus the needler and an unanimated plasma
  rifle; the Flood both, with fire overlays but no reloads.
- **Vitality is the collision model's, overridden per rank by the
  campaign's actor variants.** The Elite has 100 body and 100 shield
  (shield waits 12 s, recharges in 10 s); the Grunt 40 body, no shield;
  the Jackal 60 body and a 100-point shield that only protects the arm
  shield itself; the Hunter 200 body, no shield, and a skin flagged as
  head, so a pistol or sniper shot to the exposed skin kills it
  outright under multiplayer rules.
- **Aim is rate-limited for every character but the Spartan**: the
  unit tag's aiming velocity limits (2π rad/s for most, π rad/s for the
  Hunter) apply to player-controlled units too, and the aim stays
  within 90° of the body, whose feet follow the view at their own
  turning rate.
- **Voice**: each character has a dialogue tag (Grunt 65 filled
  vocalizations, 459 lines; Elite 53, 271; Jackal 63, 577; Hunter 8,
  90). A player-worn body speaks only what the damage code triggers:
  pain, death, scream and maimed lines. Kill boasts, taunts, grenade
  warnings and the rest are spoken only by AI.
- **No character has first-person animations**; the fuel rod gun and
  the energy sword have no first-person model at all.
- **Each character is a closed set of tags**: from 31 tags and 0.94 MB
  (infection form) to 885 tags and 59.6 MB (armored Marine, whose six
  voice sets are 57 MB of sound). The smallest campaign map holding the
  Flood is c20 (77.0 MB); for the Covenant d20 (131.0 MB); for the
  Marines a30 (152.6 MB).

## Units and conventions

- Distances in world units (1 WU = 3.048 m), heights above the biped's
  origin (its feet), +X forward, +Y left, +Z up.
- Time: 30 ticks per second. Animation frames are one per tick (every
  graph here is 30 Hz: no animation carries the 25 Hz flag).
- Speeds: WU per tick (WU/t), with WU/s = 30 × WU/t and m/s = WU/s ×
  3.048.
- Angles in radians, with degrees in parentheses.
- Vitality in points as `health_shields.md` defines them.
- "Character" means a biped tag other than the Spartan's. "Player-worn"
  means a character biped controlled by a player instead of an actor.

## Constants

| Name (ours) | Value | Unit | Meaning |
|---|---|---|---|
| CAMPAIGN_MAPS | a10, a30, a50, b30, b40, c10, c20, c40, d20, d40 | | the maps read |
| BIPED_TAGS | 42 | tags | distinct biped paths over the ten maps |
| ACTOR_VARIANT_SIZE | 0x238 | bytes | the actor variant (`actv`) main struct |
| DIALOGUE_SLOTS | 209 (+47 unused) | references | vocalization slots of a dialogue tag (`udlg`, 0x1010 bytes) |
| SPARTAN_RUN | 0.075 | WU/t | for comparison (`movement.md`) |
| DIALOGUE_DISTANCE | 3 to 20 | WU | unit dialog class default distances (every character line uses them) |

### Where the numbers live in tags this spec adds

The actor variant (`actv`) is not described elsewhere. The fields this
spec uses, at offsets from the start of its main struct (checked on
every actor variant of the ten maps: the references resolve to tags of
the stated class and the vitality values are plausible):

| Offset | Type | Field (tag name) | Meaning |
|---|---|---|---|
| 0x00 | u32 | flags | |
| 0x04 | ref `actr` | actor | the behaviour definition (AI only) |
| 0x14 | ref `unit` | unit | the biped it drives |
| 0x24 | ref `actv` | major upgrade | the variant a squad's "major upgrade" swaps in |
| 0x4C | i16 | movement type | AI movement style (AI only) |
| 0x50 | f32 | initial crouch chance | AI only |
| 0x64 | ref `weap` | weapon | the weapon the actor is created with |
| 0x180 | i16 | grenade type | 0 human fragmentation, 1 Covenant plasma |
| 0x1C0 | ref `eqip` | equipment | dropped on death (none on the disc's characters) |
| 0x1D0, 0x1D2 | i16 × 2 | grenades lower, upper | grenades carried |
| 0x200 | f32 | maximum body vitality | points; when this or the next is above 0 both replace the collision model's maxima |
| 0x204 | f32 | maximum shield vitality | points |
| 0x208 | f32 | shield sapping radius | 0 on every disc variant |
| 0x20C | i16 | forced shader permutation index | when not 0, every shader of the model draws this permutation (the rank sheen of Elites: section 1, The Elite's colours) |
| 0x22C | block | change colors | rank colours (count 0 or 1 on the disc) |

The scenario's actor palette is the block at 0x420 of the scenario's
main struct (elements of 16 bytes: one `actv` reference); its
encounters the block at 0x42C (0xB0 bytes each), whose squads block is
at 0x80 of an encounter (0xE8 bytes each). A squad has its actor
palette index at 0x20 (i16), its normal-difficulty count at 0x7C and
its legendary count at 0x7E, and a starting locations block at 0xD0
(0x1C bytes each) whose element has its own actor palette index at
0x18 (−1: the squad's).

The dialogue tag (`udlg`) is a 0x10-byte header and then 256 tag
references of 16 bytes (`snd!`): slots 0 to 208 are the vocalizations
named in section 6, 209 to 255 unused. An empty slot holds either a
null reference or all zero bytes (both mean "no line").

## 1. The roster

### Every biped on the campaign maps

"Combat graph" means its animation graph has a `stand` seat with
movement animations. "Cand." is our verdict on it as a playable body
(section "Playable candidates").

| Biped tag | Maps | What it is | Combat graph | Cand. |
|---|---|---|---|---|
| `characters\elite\elite` | a10 a30 a50 b30 b40 c40 d20 | Elite minor and major | yes | **yes** |
| `characters\elite\elite special` | a50 b30 b40 c40 d20 d40 | Elite commander, spec-ops, stealth | yes (same graph) | **yes** |
| `characters\elite\elite special cinematic` | d40 | cinematic copy | yes | no (story) |
| `characters\grunt\grunt` | a10 a30 a50 b30 b40 c10 c40 d20 d40 | Grunt minor and major | yes | **yes** |
| `characters\grunt\grunt specops` | c40 d20 d40 | spec-ops Grunt | yes (same graph) | **yes** |
| `characters\jackal\jackal` | a30 a50 b30 b40 c10 c40 d20 | Jackal minor | yes | **yes** |
| `characters\jackal\jackal major` | a30 a50 b30 b40 c10 c40 d20 | Jackal major | yes (same graph) | **yes** |
| `characters\hunter\hunter` | a50 b30 b40 c40 d20 d40 | Hunter | yes (forward only) | **yes** |
| `characters\marine\marine` | a30 a50 c10 | Marine (no armour) | yes | **yes** |
| `characters\marine\marine unarmed` | a50 c10 | unarmed Marine | yes | variant |
| `characters\marine\marine_plasma_rifle` | a50 | Marine starting with a plasma rifle | yes | variant |
| `characters\marine_armored\marine_armored` | a10 a30 a50 b30 b40 c10 | armoured Marine | yes | **yes** |
| `characters\marine_armored\marine_armored_sniper` | a30 b40 | armoured Marine, sniper | yes | variant |
| `characters\marine_armored\marine_armored_unarmed` | a10 a30 | unarmed armoured Marine | yes | variant |
| `characters\crewman\crewman` | a10 a30 | ship's crewman | yes (Marine graph) | yes (weak) |
| `characters\floodcombat elite\floodcombat elite` | c10 c20 c40 d20 d40 | Flood combat form (Elite) | forward only | **yes** |
| `characters\floodcombat_human\floodcombat_human` | c10 c20 c40 d20 d40 | Flood combat form (human) | forward, back | **yes** |
| `characters\floodcarrier\floodcarrier` | c20 c40 d20 d40 | Flood carrier form | forward only, unarmed | no (see below) |
| `characters\flood_infection\flood_infection` | c10 c20 c40 d20 d40 | Flood infection form | forward only, unarmed | no |
| `characters\flood_infection\flood_infection nopop` | c10 | infection form that does not pop | forward only | no |
| `characters\sentinel\sentinel` | c10 c20 c40 d40 | Sentinel (a flying biped) | no (flying, fixed gun) | no |
| `characters\monitor\monitor` | c10 c20 c40 d40 | 343 Guilty Spark (flying, invulnerable) | no | no (story) |
| `characters\monitor\monitor_no_light` | d40 | copy without its light | no | no (story) |
| `characters\engineer\engineer` | b30 | Engineer (flying) | no | no |
| `characters\captain\captain` | a10 a50 c10 d40 | Captain Keyes, ship uniform | idle, walk only | no (story) |
| `characters\captain_ingame\captain_ingame` | a50 | Keyes in the brig, Marine graph | yes | no (story) |
| `characters\captain_ingame\fatigues\captain_fatigues` | c10 | Keyes in fatigues | yes | no (story) |
| `characters\captain_ingame\fatigues\captain_fatigues_unarmed` | c10 | the same, unarmed | yes | no (story) |
| `characters\cortana\cortana` | a10 d40 | hologram | no | no (story) |
| `characters\cortana\halo_enhanced\halo_enhanced` | b40 c40 | hologram | no | no (story) |
| `cinematics\johnson\johnson` | d40 | Sergeant Johnson, cinematic | no | no (story) |
| `characters\flood_captain\flood_captain` | d20 | Keyes absorbed by the Flood | no | no (story) |
| `characters\pilot\pilot` | a10 a30 | pilot, cinematic | no | no (story) |
| `characters\marine_suicidal\marine_suicidal` | c10 | the Marine who shoots himself | no | no (story) |
| `characters\marine_wounded\prone_1\prone_1` | a10 | wounded prop | no | no |
| `characters\marine_wounded\sitting_1\sitting_1` | a10 a30 | wounded prop | no | no |
| `characters\marine_wounded\sitting_2\sitting_2` | a10 | wounded prop | no | no |
| `characters\cyborg\cyborg` | all ten | the campaign Spartan | yes | (the Spartan) |
| `characters\cyborg\cyborg_unarmed` | all ten | unarmed Spartan | yes | no |
| `characters\cyborg\cyborg_cinematic` | a10 | cinematic Spartan | no | no |
| `vehicles\c_dropship\cd_gun\cd_gun` | a30 a50 b30 b40 c40 d20 d40 | the dropship's turret | no | no |
| `levels\d40\devices\invisible_collision_object\invisible damage detector` | d40 | an invisible trigger | no | no |

Counts per map (distinct biped tags): a10 14, a30 14, a50 15, b30 11,
b40 12, c10 18, c20 8, c40 17, d20 15, d40 19. Actor variant tags per
map: a10 16, a30 21, a50 24, b30 22, b40 31, c10 37, c20 24, c40 47,
d20 40, d40 35.

Three bipeds (`pilot`, `engineer`, the invisible damage detector) hold
a value outside the defined range in the unit's motion sensor blip
size field (0x298: 30062); a reader must not refuse them for it.

### How variants differ

A character's ranks and colours are not separate bipeds, mostly. They
differ in four ways:

1. **A different biped tag** with the same graph: `elite` and `elite
   special` (different model and collision model: the special one has
   one region and different shield timing, section 5); `grunt` and
   `grunt specops` (identical model, collision, graph; unit fields
   differ: hard ping threshold 0.3 against 0.25); `jackal` and `jackal
   major` (identical model, collision and graph; unit flags 0 against
   0x40000); the Marine bipeds (different starting weapon, model or
   armour).
2. **The actor variant's forced shader permutation** (0x20C): the
   Elite's rank sheen. Minor 0, major 1, commander 2, spec-ops 3,
   stealth 0 (the stealth Elite's look is its active camouflage, not a
   permutation). See "The Elite's colours" below.
3. **The actor variant's change colour** (0x22C, elements of 0x20
   bytes: a lower and an upper RGB bound, then 8 unused bytes; one
   entry where present): Grunt majors and spec-ops, Jackal minors and
   majors, the Elites, the crewmen (minors of Grunts have none). When
   the actor is created, slot i of the unit's change colours (for each
   entry i, at most 4) becomes the HSV blend of the two bounds at a
   random factor drawn from the game-state random generator; it
   replaces whatever the biped tag's own change colour chose.
4. **The actor variant's vitality** (0x200, 0x204): section 5.

### The Elite's colours

An Elite's rank look is two things layered on the same grey-blue
armour textures:

1. **Change colour A tints the whole armour.** Every armour shader of
   both Elite models (`elite torso`, `elite arms`, `elite legs`, `elite
   head`, `elite heada`, `elite legsa`; `elite special body`) is a model
   shader with change colour source A, and their multipurpose maps'
   alpha (the change-colour mask on the Xbox, `shaders.md`) is fully
   opaque everywhere (measured), so the diffuse colour is the base map
   times colour A all over. (The teeth and the transparent inset
   lights are not tinted.)
2. **The shader permutation picks the reflection's colour.** A
   permutation index p makes every bitmap the shader reads use its
   image p modulo that bitmap's image count. The Elite's base, detail
   and multipurpose bitmaps have one image each, so only the
   reflection cube map changes: `characters\elite\bitmaps\cubemaps`
   holds four cube maps whose average colours are, by index, 0 blue
   (9, 59, 142 of 255), 1 maroon red (84, 16, 36), 2 orange gold (156,
   99, 22), 3 grey violet (81, 77, 98). The reflection is masked by the
   multipurpose map's red channel, so it is a sheen over the tinted
   armour. Index 0 is "not forced": the model's own shader references
   (all permutation 0) are used.

What each rank wears (forced permutation; colour A from the actor
variant):

| Rank (actor variants) | Biped | Permutation, sheen | Colour A |
|---|---|---|---|
| minor (plasma rifle, needler, ranged) | `elite` | 0, blue | (0.314, 0.302, 0.533), blue violet |
| major (plasma rifle, needler) | `elite` | 1, red | (0.251, 0.082, 0.086), dark red |
| commander (plasma rifle, energy sword) | `elite special` | 2, gold | (0.294, 0.239, 0.055), dark gold |
| spec-ops (plasma rifle, needler) | `elite special` | 3, grey | (0, 0, 0), black |
| stealth, stealth major | `elite special` | 0, blue | none: the tag's own (below) |

A unit created without an actor gets no forced permutation (0, the
blue sheen) and the change colours of its placement and its tag
(`objects.md`, Change colours). The `elite` tag's change colour A has
no permutations, so it **keeps the colour the unit was created with**:
a player-worn `elite` created with the player's colour in all four
slots, as a multiplayer Spartan is, wears the player's colour all over
(team red or blue, or the free-for-all colour), which is a usable team
tint with no change. The `elite special` tag's A has one permutation
(grey, 0.565 to 0.678), so without an actor it is always grey whatever
it is created with. For a rank look on a player, set the forced
permutation and colour A from the table (ours: an implementation may
pair the red team with the major's permutation 1 and the blue team with
0 while tinting A by the team colour).

The other characters' colours work the same way through colour A
(no forced permutation): the Grunt tag's A is orange ((0.863, 0.502,
0.173) to (0.824, 0.443, 0.157)), overridden red for majors ((0.824,
0.075, 0.075) to (0.643, 0.090, 0.090); needler majors' upper bound
(0.600, 0.082, 0.082)) and near black for spec-ops ((0.071, 0.071,
0.071) to black); the Jackal tag's A is blue grey ((0.259, 0.373,
0.490) to (0.267, 0.353, 0.482)), the minor's actor colour the same,
the major's pink ((0.675, 0.459, 0.459) to (0.710, 0.435, 0.443)); the
Hunter has no change colours at all.

### Model permutations

The model's own permutations add variety independent of rank: the
Elite's `arms`, `head`, `legs` regions (`base00` or an armoured
permutation), the Grunt's `perm head and back` (`base00`,
`shellback`), the Jackal's `head` (`armored_head`, `base00`), the
Marine's eleven heads and two sleeves, the crewman's six heads. The
variant number of the object picks them as `models.md` describes.

### Playable candidates

| Character | Biped (default) | Verdict |
|---|---|---|
| Elite | `characters\elite\elite special` (one region, the commander, spec-ops and stealth body) or `characters\elite\elite` (four regions) | yes: full four-direction walk, pistol-class weapons, melee and grenade animations |
| Grunt | `characters\grunt\grunt` | yes: full walk, Covenant pistols and the fuel rod |
| Jackal | `characters\jackal\jackal` | yes: full walk; no grenade throw and no melee animation |
| Hunter | `characters\hunter\hunter` | yes, with care: walks forward only while standing (all four directions crouched), holds only its own arm cannon |
| Marine | `characters\marine_armored\marine_armored` (the campaign's common Marine) or `characters\marine\marine` | yes: full walk, human rifles and pistol; no melee animation |
| Crewman | `characters\crewman\crewman` | possible: Marine graph, 25 body points |
| Flood combat Elite | `characters\floodcombat elite\floodcombat elite` | yes, with care: forward only, every weapon fires, no reloads, a leap melee |
| Flood combat human | `characters\floodcombat_human\floodcombat_human` | yes, with care: forward and back only |
| Flood carrier | `characters\floodcarrier\floodcarrier` | no: unarmed, forward only, its melee is fatal to itself (it bursts) |
| Flood infection | `characters\flood_infection\flood_infection` | no: unarmed, 0.27 WU tall, spherical body, no dialogue |
| Sentinel, Monitor, Engineer | | no: flying bipeds, no hands; the Monitor's shield is 10¹⁰ points |
| Keyes, Cortana, Johnson, pilots, wounded | | no: story bipeds |

## 2. Which weapons each may hold

### Rules

1. **Selection.** A unit's weapon animation set is chosen by the seat
   label and the held weapon's label as `animations.md` ("Seat, weapon
   class and weapon type") states: the engine finds, in the seat whose
   label matches, the weapon type whose label equals the weapon's; the
   last match in block order wins; **no match leaves the previous
   selection unchanged** (the unit keeps animating as it did with the
   previous weapon). For a player-worn character the seat is `stand`,
   or `crouch` while crouched.
2. **Eligibility (ours, faithful to the data).** A character *can hold*
   a weapon when its graph's `stand` seat has a weapon type with the
   weapon's label. It holds it *fully animated* when, in addition, that
   type's class has the slots idle, move-front and aim-still, and the
   type has fire-1. Otherwise the selection works but the named
   animations are missing (an empty slot plays nothing: the unit simply
   does not visibly fire, reload or melee). A weapon whose label no
   `stand` type carries is *never* held.
3. **Labels** (the weapon tag's label at 0x30C of its main struct, read
   on the disc): assault rifle `ar`, pistol `hp`, plasma rifle `pr`,
   plasma pistol `pp`, needler `ne`, shotgun `sg`, sniper rifle `sr`,
   rocket launcher `rl`, flamethrower `ft`, both fuel rod guns (the
   Grunt's `weapons\fuel rod gun\fuel rod` and the Hunter's
   `weapons\fuel rod gun\hunter fuel rod`) `fr`, energy sword `fb`, the
   Sentinel's beam `ar`. Grenades are not weapons: throwing one uses
   the class's throw-grenade slot.
4. **Where it attaches.** On selection the weapon is attached to the
   unit at the marker named by the weapon class's hand marker (on the
   unit's model) by the weapon's grip marker (empty on every character
   class: the weapon's own origin). The hand markers: Elite `right hand
   elite`, Grunt `right hand`, Jackal `left hand jackal` (Jackals are
   left-handed; the shield is on the right), Marine and crewman `right
   hand`, Flood `right hand`, Hunter none (empty: its weapon has no
   model, rule 6). An empty hand marker attaches at the unit's origin.
   The weapons carry left-hand markers for the humans (`left hand
   cyborg`, `left hand marine`) and for Covenant hands (`left hand
   covenant` on the fuel rod gun only).
   **The off hand is free on every Covenant and Flood character.** A
   unit pins a hand only through its weapon class's IK points
   (`animations.md`, Inverse kinematics), and on the disc the classes a
   held weapon can select have none for the Elite (pistol, sword), the
   Grunt (pistol, `missle`), the Jackal (pistol), the Hunter (unarmed)
   or either Flood combat form, standing or crouched: their off hands
   move only as the animations move them (the Jackal's off hand is its
   shield arm). The Elite's `rifle` class does pin its `left hand
   elite` marker to the weapon's `left hand elite` marker, but that
   class has no movement animations and selects no weapon a player
   holds standing. The Marine's `rifle` class pins its `left hand` to
   the weapon's `left hand marine` (as the Spartan's pins `left hand
   cyborg`); its `pistol` class has one IK point whose two marker names
   are empty, which pins nothing.
5. **Melee and grenades by a player.** The player's melee action uses
   the weapon type's melee slot (`animations.md`, Actions); only the
   Spartan's graph fills it. A character's own melee animation is the
   weapon class's melee slot, used by the AI's melee; a unit whose
   class lacks it cannot start that melee. The gameplay timing of a
   player's melee is the first-person animation's (`weapons.md`), which
   a player-worn character keeps (section 7), so the damage still
   happens; only the third-person swing is missing or comes from the
   class slot if we choose to play it.
6. **The Hunter's arm cannon** is an ordinary weapon tag, `weapons\fuel
   rod gun\hunter fuel rod` (label `fr`), with **no render model and
   no first-person model**: it is invisible and the cannon seen is part
   of the Hunter's own model. Its projectiles leave from the weapon's
   origin, which sits at the Hunter's origin (empty hand marker); the
   Hunter's model has a `primary trigger` marker on its right forearm
   (−0.011, −0.336, 0.270 in the bind pose) carrying two light
   attachments. Whether the engine fires from the unit's marker rather
   than the invisible weapon's is an open question. The Hunter's
   `stand` seat has one class, `unarmed`, with one type, `fr`.
7. **The Jackal's shield is not an object.** It is the region `shield`
   of the Jackal's own model (permutations `base00` and `~shield_off`)
   and the material `shield` (type 16, jackal energy shield) of its
   collision model, on the node `frame shield` (damage part 8, right
   hand). The unit's shield vitality *is* this arm shield (section 5).
   Its region is flagged "disappears when shield is off": when the
   shield is depleted the object's permutation for that region becomes
   1, and when its recharge begins (for the Jackal, the same tick it is
   full again) it returns to 0. The one per-region permutation index
   chooses both the drawn permutation (the render model's region 3,
   `~shield_off`: the disc not drawn) **and the collision BSP of
   every collision node of that region** (the collision model's region
   3 has the same two permutations, `base00` and `~shield_off`; the two
   models list their regions in the same order, so the index is the
   same). The node `frame shield` has two BSPs: `base00`, 54 surfaces of
   materials `armored` and `shield` spanning 0.10 × 0.43 × 0.50 WU (the
   energy disc), and `~shield_off`, 6 surfaces of material `armored`
   only, 0.04 × 0.06 × 0.09 WU (the emitter on the forearm). So **while
   the shield is down the disc stops blocking anything**: shots pass
   where it was, and only the small emitter remains, whose `armored`
   material leaks every hit to the body (section 5).

### What each graph animates (stand seat; crouch where it differs)

Class slots are the weapon class's (`animations.md` slot names); type
slots the weapon type's. "Missing" lists the slots an implementer will
look for and not find.

| Character (graph) | Class | Types (type slots) | Class slots present | Missing |
|---|---|---|---|---|
| Spartan (`characters\cyborg\cyborg`) | rifle | ar (reload-1, fire-1, fire-2, melee), sg (reload-1, chamber-1, fire-1, melee), sr (reload-1, melee), pr (fire-1, melee), pb, f (melee), ne (overridden by pistol's) | idle, turns, 4 moves, airborne, lands, throw-grenade, ready, aim-still, aim-move, 4 stunned | |
| | pistol | hp (reload-1, fire-1, melee), pp (fire-1, melee), ne (reload-1, melee), b (melee) | as rifle without ready | |
| | missile | rl (reload-1, fire-1, melee) | as rifle | |
| | support | ft (none) | airborne, lands, throw-grenade only | idle, moves, aim: the flamethrower is not animated in third person |
| Elite (`characters\elite\elite`, both Elite bipeds) | pistol | pr, pp, ne (reload-1, fire-1 each); hp (none) | idle, turns, 3 dives, 4 moves, airborne, lands, throw-grenade, aim-still, aim-move, surprise ×2, berserk, evade ×2, signal-attack, warn, melee, melee-airborne | hp: no fire, no reload |
| | rifle | pb, `c needler` (none) | dives, airborne, lands, throw-grenade, surprise, berserk, evades, signal, warn, melee | idle, moves, aim: unusable |
| | sword | fb (none) | as pistol | fire and reload (the sword has none) |
| | (crouch only) | ar under pistol and rifle, no slots | | the Elite has no standing `ar` type: an assault rifle is never selected standing |
| Grunt (`characters\grunt\grunt`, both) | pistol | pr, ne, pp (reload-1, fire-1 each) | idle, turns, 3 dives, 4 moves, airborne, lands, throw-grenade, aim-still, aim-move, surprise ×2, evade ×2 | melee |
| | missle (sic) | fr (none) | as pistol without aim-move | fire, reload, melee |
| Jackal (`characters\jackal\jackal`, both) | pistol | pp, pr, ne (reload-1, fire-1 each) | idle, turns, 3 dives, 4 moves, airborne, lands, aim-still, aim-move, surprise ×2, evade ×2, signal-attack, warn, celebrate | throw-grenade, melee |
| | rifle | (no types in stand) | | |
| Hunter (`characters\hunter\hunter`) | unarmed | fr (fire-1) | idle, turns, 3 dives, move-front, airborne, lands, aim-still, aim-move, melee | move-back, move-left, move-right (present crouched), throw-grenade, reload |
| Marine, crewman, Keyes in-game (`characters\marine\marine`) | rifle | ar (reload-1, fire-1, fire-2), sg (reload-1, fire-1), ne (fire-1), sr, pr (none) | idle, turns, 3 dives, 4 moves, airborne, lands, throw-grenade, aim-still, aim-move, berserk, evades, signal-move, signal-attack, warn, 4 stunned, celebrate, panic | melee; sr and pr fire and reload |
| | pistol | hp (reload-1, fire-1) | as rifle without stunned | melee |
| | unarmed | (empty label) | idle, turns, dives, 4 moves, airborne, lands, throw-grenade, aim-still | |
| Flood combat Elite (`characters\floodcombat elite\floodcombat elite`) | pistol | ar, pr, hp, ne, pp, rl, sg (fire-1 each); ft (none) | idle, turns, move-front, airborne, lands, aim-still, aim-move, surprise-front, melee, melee-airborne, resurrect ×2, leap-start, leap-airborne, leap-melee | move-back/left/right, throw-grenade, every reload |
| | unarmed | (empty label) | as pistol without aim-move | |
| Flood combat human (`characters\floodcombat_human\floodcombat_human`) | pistol | ar, hp, rl, pp, pr, ne, sg (fire-1 each); ft (none) | as the Flood Elite plus move-back | move-left/right, throw-grenade, reloads |
| Flood carrier | unarmed | (empty label) | idle, turns, move-front, airborne, lands, aim-still, melee | |
| Flood infection | unarmed | (empty label) | idle, move-front, airborne, lands, aim-still, melee, melee-continuous, feeding | |

Every character graph also has the damage table's ping and death
animations (section 5), seats for vehicles and dropships, and the
`alert`, `asleep`, `flee` and `flaming` seats the AI uses.

### What the campaign gives them

From every actor variant tag (its weapon at 0x64) and every scenario's
starting locations (each location names its actor variant, or its
squad's). "Placements" counts starting locations over the ten maps
naming the variant; it is an upper bound of the actors placed (a
squad's count may be lower than its locations) and ignores scripted
weapon swaps.

| Character | Actor variant (vitality body / shield; grenades) | Weapon | Placements |
|---|---|---|---|
| Elite (`elite`) | elite minor plasma rifle (100 / 100; plasma 1 to 2) | plasma rifle | 434 |
| | elite minor plasma rifle ranged (100 / 100) | plasma rifle | 8 |
| | elite minor needler (100 / 100; plasma 1 to 2) | needler | 124 |
| | elite major plasma rifle (100 / 150; plasma 2 to 3) | plasma rifle | 8 |
| | elite major needler (100 / 150) | needler | 5 |
| Elite (`elite special`) | elite commander plasma rifle (100 / 200) | plasma rifle | 10 |
| | elite commander energy sword (100 / 300; no grenades) | energy sword | 12 |
| | elite specops plasma rifle (125 / 200) | plasma rifle | 50 |
| | elite specops needler (125 / 200) | needler | 8 |
| | stealth elite plasma rifle (100 / 0) | plasma rifle | 45 |
| | stealth elite major plasma rifle (100 / 0) | plasma rifle | 4 |
| | stealth elite major energy sword (100 / 0) | energy sword | 18 |
| Grunt (`grunt`) | grunt minor plasma pistol (30; plasma 1) | plasma pistol | 971 |
| | grunt minor needler (45; plasma 1) | needler | 603 |
| | grunt major plasma pistol (60; plasma 1 to 2) | plasma pistol | 79 |
| | grunt major needler (75; plasma 1 to 2) | needler | 72 |
| | grunt flee minor / major (30 to 75) | plasma pistol, needler | 85 |
| Grunt (`grunt specops`) | grunt specops fuel rod (75) | fuel rod gun | 44 |
| | grunt specops fuel rod airdef (75) | fuel rod gun | 10 |
| | grunt specops needler (75) | needler | 59 |
| Jackal | jackal minor plasma pistol (50 / 200; none) | plasma pistol | 705 |
| Jackal (`jackal major`) | jackal major plasma pistol (75 / 250) | plasma pistol | 25 |
| Hunter | hunter (225), hunter major (350) | hunter fuel rod | 60 |
| Marine (`marine`) | marine assault rifle (12 / 24; frag 1 to 2), major (24 / 24) | assault rifle | 40 |
| | marine plasma rifle (12 / 24) | plasma rifle | 228 |
| | marine needler (12 / 24) | needler | 1 |
| | marine shotgun (60 / 45) | shotgun | 8 |
| Marine (`marine_armored`) | marine_armored assault rifle (60 / 45), major (75 / 45) | assault rifle | 135 |
| | marine_armored plasma rifle (60 / 45), major (75 / 60) | plasma rifle | 17 |
| | marine_armored needler (60 / 45) | needler | 6 |
| | marine_armored shotgun major (75 / 60) | shotgun | (c10, scripted) |
| | marine_armored sniper rifle (60 / 45), major (75 / 60), on `marine_armored_sniper` | sniper rifle | 4 |
| Crewman | crewman pistol, security pistol (25) | pistol | 20 |
| | crewman command, engineer, pilot, science, tech (25) | none | 29 |
| Flood combat Elite | assault rifle (120) | assault rifle | 185 |
| | plasma rifle (120) | plasma rifle | 114 |
| | shotgun (120) | shotgun | 103 |
| | plasma pistol (120) | plasma pistol | 81 |
| | needler (120) | needler | 67 |
| | pistol (120) | pistol | 65 |
| | rocket launcher (150) | rocket launcher | 24 |
| | flame thrower (100) | flamethrower | (c20, d20) |
| | sniper rifle (300) | sniper rifle | (c40) |
| | unarmed (100), stealth unarmed (200) | none | 380 |
| Flood combat human | plasma pistol, assault rifle, pistol, shotgun, plasma rifle, needler (100 each) | as named | 109, 88, 85, 85, 84, 76 |
| | rocket launcher (100) | rocket launcher | 15 |
| | flame thrower (100) | flamethrower | (c20, d20) |
| | unarmed (100) | none | 98 |
| Flood carrier | floodcarrier (50) | none | 384 |
| Flood infection | flood_infection (0.01), nopop (0.01) | none | 3856 |
| Sentinel | sentinel (80), major (120), defensive (80), shielded (60 / 60), shielded major (80 / 80) | `characters\sentinel\sentinel` | 162 |

Grenade types: every Covenant variant and every Flood Elite variant
carries plasma grenades; Marines, crewmen and Flood humans frag
grenades (Flood forms never throw: no throw-grenade animation).

### Character by multiplayer weapon

"anim" = can hold, fully animated (rule 2); "part" = can hold, with the
named overlays missing; "game" = the campaign gives it this weapon;
"never" = no `stand` type with the label.

| Character | AR | Pistol | Plasma rifle | Plasma pistol | Needler | Shotgun | Sniper | Rocket | Flamethrower | Fuel rod (`fr`) | Frag | Plasma grenade |
|---|---|---|---|---|---|---|---|---|---|---|---|---|
| Spartan | anim | anim | anim | anim | part (no fire-1) | anim | part (no fire-1) | anim | part (no idle, move or aim) | never | animated throw | animated throw |
| Elite | never | part (no fire, reload) | anim, game | anim | anim, game | never | never | never | never | never | animated throw | animated throw, game |
| Grunt | never | never | anim | anim, game | anim, game | never | never | never | never | part (no fire, reload), game | animated throw | animated throw, game |
| Jackal | never | never | anim | anim, game | anim | never | never | never | never | never | no animation | no animation |
| Hunter | never | never | never | never | never | never | never | never | never | anim (own cannon only), game | no | no |
| Marine | anim, game | anim | part (no fire, reload), game | never | anim (no reload), game | anim, game | part (no fire, reload), game | never | never | never | animated throw, game | animated throw |
| Flood combat Elite | anim (no reload), game | anim (no reload), game | anim, game | anim, game | anim (no reload), game | anim (no reload), game | never | anim (no reload), game | part (no fire), game | never | no animation | no animation (carried) |
| Flood combat human | anim (no reload), game | anim (no reload), game | anim, game | anim, game | anim (no reload), game | anim (no reload), game | never | anim (no reload), game | part (no fire), game | never | no animation | no animation |

The flood forms' sniper rifle in c40 has no `sr` type in either flood
graph: the actor keeps its previous (or unarmed) animation set while
holding it.

## 3. The body

### Dimensions and the pill

Model height and width are the highest-detail geometry's bounding box
in the bind pose (permutation 0 of every region; the bind pose is the
modelling pose, arms out, so widths include the arms). The head height
is the `head` marker's in the bind pose. Pill heights, radius and
camera heights are the biped tag's (`objects.md`, The biped).

| Character | Model top | Head marker | Width (Y) | Standing pill | Crouching pill | Radius | Camera stand / crouch | Acceleration scale |
|---|---|---|---|---|---|---|---|---|
| Spartan (MP) | 0.704 (2.146 m) | 0.649 (1.978 m) | 0.30 | 0.7 (2.134 m) | 0.5 | 0.2 | 0.62 / 0.35 | 0.7 |
| Elite | 0.786 (2.396 m) | 0.664 (2.024 m) | 0.39 | 0.8 (2.438 m) | 0.5 | 0.2 | 0.6 / 0.3 | 0.6 |
| Elite special | 0.821 (2.502 m) | 0.664 | 0.41 | 0.8 | 0.5 | 0.2 | 0.6 / 0.3 | 0.6 |
| Grunt | 0.584 (1.780 m) | 0.449 (1.369 m) | 0.60 | 0.55 (1.676 m) | 0.45 | 0.2 | **0.28 / 0.28** | 0.9 |
| Jackal | 0.640 (1.951 m) | 0.582 (1.774 m) | 0.79 | 0.6 (1.829 m) | 0.45 | 0.18 | 0.6 / 0.3 | 1.0 |
| Hunter | 0.969 (2.954 m) | 0.904 (2.755 m) | 0.79 | 0.8 (2.438 m) | **0.8** | 0.24 | 0.6 / 0.3 | 0.5 |
| Marine (`marine`) | 0.634 (1.932 m) | 0.593 (1.807 m) | 0.66 | 0.64 (1.951 m) | 0.5 | 0.18 | 0.62 / 0.44 | 1.0 |
| Marine (`marine_armored`) | 0.647 (1.971 m) | 0.593 | 0.66 | 0.64 | 0.5 | 0.18 | 0.62 / 0.44 | 1.0 |
| Crewman | 0.634 | 0.593 | 0.67 | 0.64 | 0.4 | 0.18 | 0.62 / 0.4 | 1.0 |
| Flood combat Elite | 0.775 (2.362 m) | 0.625 (1.905 m) | 0.53 | 0.7 | **0.8** | 0.2 | 0.6 / 0.6 | 0.9 |
| Flood combat human | 0.659 (2.009 m) | 0.532 (1.622 m) | 0.70 | 0.6 | 0.45 | 0.18 | 0.6 / 0.45 | 1.0 |
| Flood carrier | 0.669 (2.039 m) | 0.432 | 0.38 | 0.7 | 0.5 | 0.2 | 0.6 / 0.3 | 1.2 |
| Flood infection | 0.274 (0.835 m) | 0.139 | 0.17 | 0 (spherical) | 0 | 0.12 | 0.15 / 0.15 | 1.0 |

Notes:

1. Bipeds have **no physics tag and no mass** on the disc; the only
   mass-like number is the object's acceleration scale (`objects.md`),
   which scales pushes from damage.
2. The Grunt's camera height (0.28 WU, 0.85 m) is below its head (0.45
   WU): a faithful first-person Grunt sees from its chest. The Hunter's
   and the Flood Elite's crouching pill is as tall as (or taller than)
   the standing one: crouching does not lower them.
3. The pill is a player's: `movement.md` (The pill) gives a
   player-controlled biped the segment between two spheres; any biped
   not player-controlled moves as one sphere of the radius. A
   player-worn character therefore gets its own heights and radius in
   the same pill.
4. The infection form's biped flags (0x5F9) make it spherical (no
   pill), pass through other bipeds, climb any surface and ignore
   falling damage; its origin offset is (0, 0, −0.12).

### Hit boxes: collision models

Hits are traced against the collision model per node (`collision.md`);
the material of the surface hit gives the head flag, the shield leak
and the shield and body multipliers (`health_shields.md`). Region
flags are `health_shields.md`'s region flags.

| Character (collision model) | Materials: name (type; head; leak; shield ×; body ×) | Regions (flags; threshold; destroyed effect) | Indirect material |
|---|---|---|---|
| Spartan (`characters\cyborg\cyborg`) | head (21; yes; 0; 1; 1), body (21; 0; 1; 1), legs (21; 0; 1; 0.8), shield generator (21; 0; 1; 1) | cyborg | body |
| Elite (`characters\elite\elite`) | head (14 elite; yes; 0; 1; 1), arms (14; 0; 1; **0.5**), body (14; 0; 1; 1), legs (14; 0; 1; 0.8) | arms, head, legs, torso (none destroyable) | body |
| Elite special (`characters\elite\elite_special\elite_special`) | head (14; yes; 0; 1; 1), body (14; 0; 1; 1), legs (14; 0; 1; 0.8) | elite | body |
| Grunt (`characters\grunt\grunt`) | head (11 grunt; yes; 0; 0; 1), body (11; 0; 0; **0.8**), armor (11; 0; 0; 1) | perm arms and legs, perm head and back | armor |
| Jackal (`characters\jackal\jackal`) | unarmored (15 jackal; **leak 1**; 0; 1), head (15; yes; leak 1; 0; 1), armored (15; leak 1; 0; 1), shield (16 jackal energy shield; leak 0; 1; **0**) | arms, head, legs, **shield** (0x10: disappears when shield off), torso | unarmored |
| Hunter (`characters\hunter\hunter`) | body (12 hunter armor; 0; 0; 1), shield (32 hunter shield; 0; 0; **0**), skin (13 hunter skin; **head**; 0; 0; 1), spikes (12; 0; 0; **0**) | one region | body |
| Marine, both (`characters\marine_armored\marine_armored`) | head (23 human; yes; **leak 0.2**; 1; 1), body (24 armored human; 0.2; 1; 1), legs (24; 0.2; 0.8; 0.8) | arms, head, legs, torso | body |
| Crewman (`characters\crewman\crewman`) | head (23; yes; 0; 0; 1), body (23; 0; 0; 1), legs (23; 0; 0; 0.8) | arms, **head (0x2 forces object to die; 0.5)**, legs, torso | body |
| Flood combat Elite | bulbous (19 flood combat form; no head), skin (20 flood carrier form), tendrils (20); all leak 0, ×1 | head (0x200 head-maimed scream; 0.1; skin explode), left arm (0x20 inhibits melee; 0.13), legs (0x80 inhibits walking; 0.13), right arm (0x140 inhibits weapon attack and forces drop weapon; 0.13), torso | bulbous |
| Flood combat human | head (19; **no head flag**), body (19), legs (20) | head (0x200; 0.1; h-head explode), left_arm (0x20; 0.12), legs (0x80; 0), right_arm (0x140; 0.12), torso (0; 0.2) | body |
| Flood carrier | floodcarrier skin (20; 0; 0; 1), floodcarrier bulbous (19; 0; 0; 1) | one | bulbous |
| Flood infection | body (19) | one | body |

Each collision node's damage part (for the ping and death animation
choice) follows the Spartan's pattern on every humanoid (pelvis and
spine 0, spine1 1, neck and head 2, arms 3/4 and 7/8, legs 6 and 10);
the Jackal's `frame shield` is part 8, the Hunter's spikes part 1.

## 4. Movement

### How a character moves (rules)

1. **Which model applies.** Bit 1 of the biped flags ("uses player
   physics") selects `movement.md`'s model. Only the Spartans have it
   (biped flags 0x202). Every character on the disc has 0x200 (limp
   body only) or 0, so **a player-worn character does not use the
   globals' run speeds at all**.
2. **Animation state.** As in `movement.md` section 9: with a non-zero
   throttle (after its 0.1 dead zone) the wanted state is move front,
   back, left or right by the dominant axis (|t_x| ≥ |t_y|: front if
   t_x ≥ 0, else back; otherwise left if t_y ≥ 0, else right); the
   stunned versions when the body stun is above 0.2 **or** the unit's
   stunned movement threshold is positive and its recent body damage
   exceeds it (Marines 0.1, others 0). Zero throttle: idle. The seat is
   `stand`, or `crouch` while crouched. There is no diagonal: a stick
   at (0.7, 0.7) is "front", and the body moves straight ahead.
3. **Velocity from root motion.** Each tick, if the unit is not
   airborne and not slipping, its velocity is set from the current
   frame's root motion with no acceleration limit; the details (which
   frame, which frame of reference, the loop point, permutations,
   blends, partial stick) are in "Root motion, tick by tick" below.
4. **A missing animation stops the body.** If the graph has no
   animation for the wanted state, the state is entered anyway with no
   animation at all: no root motion (the body stops on the next tick)
   and the model's default pose. See "A missing animation" below; this
   corrects an earlier draft that had the body keep its old state.
5. **The infection-form speed boost.** Bipeds with the "random speed
   increase" flag (bit 11; none of the candidates) scale root motion by
   1 + d × (k mod 137) × 0.00729927 where k is the object's index and d
   the difficulty's infection-form value; not for player-worn bodies we
   offer.
6. **Facing.** While in a move state the body yaw turns toward the aim
   yaw by at most the biped's moving turning speed / 30 per tick
   (`movement.md` section 1); motion is along the body's facing, so a
   slow-turning body walks where it faces, not where the player aims.
   Idle, the turn-left and turn-right animations (90° over 14 frames on
   every character) turn the body once the aim is more than the
   stationary turning threshold away. Details and per-character rates:
   "Turning and aiming" below.
7. **Aim lag.** The unit's aiming velocity and acceleration limits
   (`objects.md`, unit layer 0x264 and 0x268) move the unit's aim
   toward the wanted aim by at most those rates **for every unit,
   player-controlled included**, and the aim is held inside the held
   weapon class's aiming bounds about the body. The Spartan's limits
   are 0 (instant). The looking limits (0x270, 0x274) do the same to
   the head's look. "Turning and aiming" below has the exact motion.
8. **Jumping.** Any character jumps when its player presses jump,
   exactly as the Spartan does (`movement.md` section 3: grounded, not
   hard-landing, the 6-tick jump timer, the velocity's upward part
   raised to the biped's jump velocity, scaled for a player by 1 − body
   stun × 0.8). No graph has a jump animation; see "Jumping and
   crouching" below for heights and landings.
9. **No air control.** Only the player-physics branch sets an airborne
   acceleration; a character in the air keeps its horizontal velocity
   until it lands. Gravity is `movement.md`'s.
10. **Landing.** The landing velocities and times of the biped tag
    (`movement.md` section 6) apply as written, but the effect differs
    from the Spartan's: during a **soft** landing the body moves by the
    landing animation's root motion, which is nearly zero, so it stands
    still for the recovery time; during a **hard** landing the wanted
    motion is zero and the velocity brakes at 0.0053333 WU/t² (it
    slides to a stop, as the Spartan does). "Meeting the world" below
    has the numbers.
11. **Crouching** works as `movement.md` section 4 with the biped's
    own heights and transition time (0.2 s on all), provided the
    `crouch` seat has a weapon type for the held weapon; otherwise the
    body cannot crouch at all ("Jumping and crouching" below).
12. **Walking slopes** use the biped's own maximum slope and up- and
    downhill scales: 60° for the Covenant and the Flood combat Elite
    (uphill 30° to 60°, scale 0.5; downhill 15° to 45°, scale 2), 45°
    for the Marines, the crewman and the Flood combat human (uphill 20°
    to 45°, scale 0.65; downhill 20° to 45°, scale 1.25: the Spartan's
    values). An earlier draft put the Flood human with the Covenant; the
    disc gives it the human values.

### Root motion, tick by tick

This answers how a moving character's displacement per tick comes from
its animation. Everything here is the engine's own behaviour for any
biped without the player physics flag; nothing in it depends on
whether a player or an actor drives the body.

**Which animation.** The wanted state of rule 2 picks a slot of the
current weapon class (`animations.md`, State to animation): move-front,
move-back, move-left or move-right (the stunned-* slots for the stunned
states). The class is the one the held weapon selected in the current
seat (`stand` or `crouch`). A player-worn body is never in `alert`,
`flee` or `flaming` (a player's unit always asks for the combat stance),
so the AI's slow patrol walk and flee run are never used.

**Permutations.** A slot names the first animation of a chain; the
chain's animations carry cumulative weights rising to 1. On entering a
state, and again every time a looping animation runs past its last
frame, a uniform random number r in [0, 1) is drawn from the game's
deterministic random generator (the one that affects game state, so
every machine of a network game draws the same) and the first animation
of the chain whose cumulative weight is at least r plays. On the disc
**no movement slot of the Elite, Grunt, Jackal, Hunter or Marine has
more than one permutation** (the Grunt's move-front is named `%0` but
is alone in its chain, weight 1). The only movement chain with two is
the Flood combat Elite's standing move-front (24 frames at 0.0625 WU/t
and 22 frames at 0.0800 WU/t, cumulative weights 0.5 and 1): its pace
is re-chosen every loop, giving an average of 3.26 WU per 46 frames,
0.0709 WU/t (2.126 WU/s) over a long run. Idle, ping and death slots
have permutations elsewhere; they do not move the body.

**Which frame, and the one-tick lag.** In each tick the body first
decides its wanted state, then moves using the frame the current
animation is showing (the frame shown since the previous tick), and
only then does the animation advance by one frame, after which a new
state (if wanted) starts at its frame 0. So:

- pushing the stick from idle on tick T: tick T moves by the idle
  animation's frame (zero) and enters the move state; tick T + 1 moves
  by the move animation's frame 0, tick T + 2 by frame 1, and so on;
- releasing on tick R: tick R still moves by the current move frame and
  enters idle; tick R + 1 moves by the idle frame, which is zero or
  nearly (the idle animations here move at most 0.0004 WU in a frame
  and nothing over a loop);
- changing direction (front to left) on tick C: tick C moves by the
  front frame, tick C + 1 by frame 0 of the left animation. A new state
  always starts its animation at frame 0; the phase is not carried over.

There is no acceleration and no braking: speed goes from zero to the
frame's full step in one tick and back to zero in one tick, one tick
after the input. (The one exception is slipping, below.)

**The loop point.** Every movement animation here has loop frame 0, so
after its last frame N − 1 it restarts at frame 0 with a fresh
permutation draw, without any blend. The displacement over one loop is
exactly the sum of the N frame steps.

**Frame of reference.** A frame's (dx, dy) is a step in WU in the
body's own frame at the start of the tick: dx along the body's facing
f = (f_x, f_y) (horizontal, unit length), dy to its left, l = (−f_y,
f_x). The horizontal wish is H = dx·f + dy·l. On the ground it is
lifted onto the ground plane exactly as the Spartan's wish is
(`movement.md` section 2, "Direction on the ground plane"), its length
is |(dx, dy)| (dz is 0: every movement animation here has frame info
type dx dy or dx dy dyaw), then the biped's slope scale applies and the
velocity is set to that vector at once (acceleration limit infinite),
minus the 1/128 ground push of `movement.md`. The facing is the body's,
never the aim's: a strafe animation moves the body sideways relative to
where its legs point. A frame's dyaw (turn animations only) turns the
facing about the up axis in the same tick; the step of that tick still
uses the facing from before the turn.

**Blends do not move anything.** The 6-tick pose blend that starts on
every state change (and the 1-tick idle/turn and 2-tick landing
blends, `animations.md`) mixes poses only. Motion always comes from
the one current animation's frame; the animation being blended out
contributes nothing.

**Partial stick and walking.** The throttle's size is used only to
pass the 0.1 dead zone and to choose the direction. Playback is one
frame per tick whatever the stick, and the step is the frame's full
step: half a push and a full push move at the same speed. A player has
no walk: the only walking animations belong to the AI's `alert` seat.
The one thing a small push changes is sound: footsteps are made only
while the throttle's magnitude is above 0.5 (`animations.md`, Bipeds,
Footsteps), so a push between 0.1 and 0.5 moves at full speed in
silence. (Whether to scale our player-worn bodies' speed by the stick
is ours to decide; the game does not.)

**What a moving body does in the air.** For the first 3 airborne
ticks the move state and its animation continue (the legs keep
walking) but the velocity is not set from them; from the 4th airborne
tick the state is airborne (`animations.md`, Bipeds). The velocity in
the air is whatever it was when the body left the ground.

### Per-direction numbers (measured)

Every frame's step of every movement animation of the three main
characters was summed from the disc (on d20; the tags are the same
build on every map holding them, section 8). "Loop" is the net displacement over one loop,
"mean" the loop divided by its frames (the speed a long run averages),
"step" the smallest and largest per-frame step length |(dx, dy)|. Sway
is the sideways wobble inside the loop; every loop's sideways sum is
zero except the Grunt's standing move-front (−0.0018 WU per loop, a
drift of 0.1° to the right).

| Character, seat, class | Direction | Frames | Loop (WU) | Mean (WU/t; WU/s) | Step min to max (WU/t) | Notes |
|---|---|---|---|---|---|---|
| Elite, stand, pistol (plasma rifle, plasma pistol, needler, human pistol) | front | 26 | 2.2500 | 0.086538; 2.596 | 0.08654 to 0.08665 | forward part constant, sway ±0.0044 |
| | back | 26 | 1.9000 | 0.073077; 2.192 | 0.07308 to 0.07316 | |
| | left, right | 26 | 1.9000 | 0.073077; 2.192 | 0.07308 constant | |
| Elite, stand, sword | front | 26 | 2.5951 | 0.099812; 2.994 | 0.09981 to 0.09991 | back, left, right as pistol |
| Elite, crouch, pistol and sword | front | 30 | 1.0000 | 0.033333; 1.000 | 0.03333 to 0.03354 | |
| | back | 30 | 0.6500 | 0.021667; 0.650 | 0.02167 to 0.02206 | |
| | left | 30 | 0.6000 | 0.020000; 0.600 | 0.01843 to 0.02335 | |
| | right | 30 | 0.6000 | 0.020000; 0.600 | 0.01984 to 0.02041 | |
| Grunt, stand, pistol | front | 16 | 1.0046 | 0.062790; 1.884 | 0.05416 to 0.06921 | the lumber: two surges per loop, sway ±0.012 |
| | back | 20 | 0.6500 | 0.032500; 0.975 | 0.02790 to 0.03532 | |
| | left, right | 20 | 0.8000 | 0.040000; 1.200 | 0.00503 to 0.05483 | a hop: 15 fast frames, then nearly stops for 4 |
| Grunt, stand, fuel rod (`missle`) | front | 16 | 1.0000 | 0.062500; 1.875 | 0.06241 to 0.06274 | |
| | back | 20 | 0.6500 | 0.032500; 0.975 | 0.03138 to 0.03444 | |
| | left | 23 | 0.2676 | 0.011635; 0.349 | 0.00493 to 0.01413 | |
| | right | 24 | 0.2650 | 0.011040; 0.331 | 0.00339 to 0.01361 | |
| Grunt, crouch, pistol | front | 46 | 0.4000 | 0.008696; 0.261 | 0.00283 to 0.01180 | |
| | back | 20 | 0.6500 | 0.032500; 0.975 | 0.02790 to 0.03532 | the standing back-step |
| | left | 25 | 0.2775 | 0.011099; 0.333 | 0.00422 to 0.01588 | |
| | right | 26 | 0.2900 | 0.011153; 0.335 | 0.00359 to 0.01478 | |
| Jackal, stand, pistol | front | 24 | 1.1000 | 0.045833; 1.375 | 0.04584 to 0.04629 | sway ±0.0063 |
| | back | 24 | 0.9000 | 0.037500; 1.125 | 0.03750 to 0.03778 | |
| | left, right | 24 | 0.8000 | 0.033333; 1.000 | 0.03333 constant | |
| Jackal, crouch, pistol | front | 36 | 0.5000 | 0.013889; 0.417 | 0.01388 to 0.01408 | |
| | back | 42 | 0.5000 | 0.011905; 0.357 | 0.01190 to 0.01210 | |
| | left | 30 | 0.4000 | 0.013333; 0.400 | 0.01283 to 0.01355 | |
| | right | 30 | 0.4000 | 0.013333; 0.400 | 0.01302 to 0.01345 | |
| Hunter, stand, unarmed | front | 26 | 2.0100 | 0.077308; 2.319 | 0.07731 to 0.07770 | no back, left or right |
| Hunter, crouch, unarmed | front, back | 22 | 0.4150 | 0.018865; 0.566 | 0.0099 to 0.0307 | |
| | left, right | 23 | 0.3846 | 0.016723; 0.502 | 0.0007 to 0.0236 | |

The idle turn animations of all four move the body slightly while
turning (Elite 0.0016 WU per tick, Grunt 0.0006, Jackal and Hunter
below 0.0001) and turn it by 0.1121997 rad (6.43°) per frame, 90° in
14 frames.

### A missing animation

When the wanted state's slot is empty in the current class (a Hunter
standing and pushed back, left or right; a Flood combat Elite pushed
anywhere but forward; a Covenant or Flood character in a stunned
state, below), the
state is still entered; only melee, airborne melee, continuous melee,
throw-grenade, leap-start and leap-melee refuse to start without an
animation. Entering it:

- clears the body's base animation: from the next tick the root motion
  is zero, so the body **stops**, whatever it was doing (a Hunter
  walking forward and then pushed back stops within one tick; it does
  not keep walking forward);
- starts the usual 6-tick pose blend toward the model's default node
  pose (its modelling pose, arms out), which the body then holds; the
  aim and look overlays (aim-move, since the state is a move state)
  and any firing overlay still play on top of it;
- makes no footsteps; the state stays until the wanted state changes,
  and since it is a move state the body keeps turning toward the aim
  at its moving rate while standing still.

This is the engine's rule for AI and players alike. The AI never asks a
Hunter or a Flood Elite to walk backward or sideways, so the campaign
never shows it; a player can. Which characters lack which (stand / crouch,
default class):

| Character | Stand | Crouch |
|---|---|---|
| Elite, Grunt, Jackal, Marine (rifle), Spartan | all four | all four (the Grunt's crouched back is the standing back animation) |
| Hunter | front only | all four |
| Flood combat Elite | front only | front only |
| Flood combat human | front, back | front only |
| Flood carrier, infection form | front only | front only |
| Marine, crewman (pistol, unarmed) | all four | none |
| every character but the Marine and the Spartan | no stunned moves | no stunned moves |

**The stunned states.** Body stun is given only to a player's unit in a
multiplayer game (`health_shields.md`, Stun), so the AI never enters
the stunned moving states. A player-worn Elite, Grunt, Jackal, Hunter
or Flood hit by a stunning damage effect (the plasma rifle's and
plasma pistol's bolts: stun 1 for 4 ticks, stacking to 37) while the
stick is pushed enters a stunned move state with no animation: it
stops dead in its default pose for the stun's duration, and it does
not turn either (the moving turn runs only in the plain move states).
A faithful port reproduces a freeze the original never showed; our
choice (Open questions) is likely to play the plain move animation
instead, with the step scaled by (1 − 0.8 s_b) as the Spartan's speed
is.

### Meeting the world

**Ground and the ground test** are `movement.md` section 5 with the
character's own pill (`movement.md`, The pill: a player-controlled
character gets the segment between two spheres of its own radius and
heights; an AI-driven one is a single sphere) and its own minimum
ground normal: cos 60° = 0.5 for the Covenant and the Flood Elite, cos
45° = 0.7071 for the humans and the Flood human. Ground steeper than
that is not ground: the body is airborne there and slides down it under
gravity with no control.

**Slopes.** The step's length is scaled by the travel direction's
climb k (the sine of the path's slope) as `movement.md` section 2
describes, with the character's own values. For the Covenant and the
Flood Elite: downhill, ×1 down to 15°, rising linearly to ×2 at 45° and
steeper; uphill, ×1 up to 30°, falling linearly to ×0.5 at 60°. So an
Elite running straight down a 45° slope covers 0.1731 WU per tick along
the slope (0.1224 horizontally). Crossing a slope sideways is level
travel and keeps the full step.

**Steps.** There is no step-up code (`movement.md` section 5, Steps).
An edge is ground when the contact normal of the rounded edge has z
at least the minimum normal, that is for an edge no higher than
r·(1 − cos θ_max) above the sole: 0.100 WU for the Elite and the Grunt,
0.090 for the Jackal, 0.120 for the Hunter (Covenant 60°), 0.053 for
the Marine and the Flood human (45°); such an edge is walked up like a
ramp at the full step. Higher edges, up to the bottom sphere's centre
(the radius: 0.2, 0.2, 0.18, 0.24 WU), can be climbed only by the
clip's upward slide, as for the Spartan; because a character's ground
velocity is reset to the root motion on every grounded tick, the
heights it carries over at speed were not simulated (Open questions).

**Walking off a ledge.** The body becomes airborne on the first tick
whose sweep meets no floor and keeps the velocity of its last ground
tick (the root-motion step, plus the −1/128 WU ground push on z):
without air control it continues in a straight line at its walking
speed and falls on `movement.md`'s gravity, 0.0035651792 WU/t². The
legs keep walking for 3 ticks, then the airborne animation plays
(Elite 63 frames looping from frame 30; Grunt 45; Jackal 62; Hunter 70;
the root motion of these is not used). There is no terminal speed;
below a vertical speed of −0.3430028 WU/t the falling-damage rule
deals 105 points every tick in the air, and the impact deals falling
damage from 0.14625688 WU/t up, both as `movement.md` section 6 says
and with no exemption for characters (only the infection form's biped
flags are immune).

**Landing.** The character's own landing thresholds (section "The
numbers", second table) choose soft or hard by the impact speed λ as in
`movement.md` section 6. Then:

- soft: the recovery ticks R = trunc(soft time × 30 × q) (q =
  (λ − v_soft)/(v_hard − v_soft)); the land-soft animation plays and
  its root motion moves the body, which on the disc is nearly nothing
  (Elite 0.0009 WU over 37 frames, Hunter −0.011 over 21), so the
  character stands still for R ticks, then resumes the move animation
  from frame 0. The Grunt's and the Jackal's land-soft slots hold their
  land-hard animations (Grunt 0.141 WU over 49 frames, at most 0.020
  WU in a frame);
- hard: R = trunc(hard time × 30 × q) with q = clamp(λ/(v_max −
  v_hard), 0, 1) (the quirk of `movement.md`); the wanted motion is
  zero, the velocity brakes at 0.0053333 WU/t² (an Elite at full run
  stops in 17 ticks), and no jump is possible.

The Grunt's thresholds are low (soft from 0.5 WU/s = 0.01667 WU/t, hard
from 1 WU/s = 0.03333 WU/t): any fall of more than 8 ticks (a drop of
about 0.16 WU, 0.5 m) is a hard landing for it, and stepping off a
kerb a soft one. For the Elite, Jackal and Hunter a hard landing needs
3 WU/s (a drop of about 1.36 WU).

**Pushes.** A damage effect's push reaches a living character halved,
as for any biped (`movement.md` section 8): its velocity gets half the
push, it becomes airborne and slipping, and **its body turns to face
the push direction**. Pushes are scaled by the object's acceleration
scale first (`health_shields.md`; Elite 0.6, Grunt 0.9, Jackal 1.0,
Hunter 0.5, Marine 1.0, Flood Elite 0.9). In the air it keeps the
pushed velocity. On landing it stays slipping: the velocity moves
toward the root-motion wish by at most 0.0053333 WU/t² per tick, with
gravity still applied on the ground, until a tick in which the wish is
within one such step; from then the root motion sets the velocity
again. Because a character moves along its body and the push turned
the body, after a push it walks in the push's direction until its
moving turn brings the body back to the aim (half a second for a
Grunt turned 90°).

**Other bodies.** The mover meets other units through their collision
models (`movement.md` section 5, Moving and other objects); a living
character collides with every living biped except where a biped is
flagged to pass through bipeds (only the infection form). Bumping into
another unit has no effect on a player's body in a game (the reactions
to a bump are the AI's).

### Jumping and crouching

**Jumping.** Every character can jump: the jump control reaches any
unit, and the jump itself is not tied to the player physics flag. It
replaces the upward part of the velocity by the biped's jump velocity
(× (1 − 0.8 s_b) for a player under body stun) and keeps the
horizontal part, which for a character is the root-motion step of the
take-off tick; there is no jump animation (the move or idle animation
continues for 3 ticks, then the airborne one). On flat ground (single
precision, `movement.md` section 3's flight):

| Character | Jump velocity (WU/t) | Apex above the take-off (WU) | Flight (ticks) | Impact λ (WU/t) | Landing | Recovery |
|---|---|---|---|---|---|---|
| Elite, Jackal, Hunter | 0.07 | 0.6526 (1.99 m) | 39 | 0.0690 | soft | 9 ticks standing still |
| Grunt | 0.07 | 0.6526 | 39 | 0.0690 | **hard** (above 1 WU/s) | 24 ticks, braking |
| Marine, crewman | 0.07 | 0.6526 | 39 | 0.0690 | soft | 2 ticks |
| Flood combat Elite and human | 0.16 | 3.5105 (10.70 m) | 89 | 0.1573 | hard | 45 ticks; falling damage at scale 0.182 (36.4 points before material modifiers) |
| Flood infection form | 0.05 | 0.3257 | 28 | 0.0498 | soft | (immune to falling damage) |

A running Elite therefore jumps about 3.37 WU far (0.0865 × 39), a
Grunt about 2.45 (its take-off step varies from 0.054 to 0.069 with the
frame), a Jackal 1.79, a Hunter 3.02. In the campaign only the Flood's leaps
(aimed by the actor, which may change the jump's direction) and the
infection forms use the jump; a player-worn Flood's straight-up 3.5 WU
jump is faithful but surely unwanted (Open questions).

**Crouching.** Every candidate has a `crouch` seat with its own
movement animations, so every one can crouch, with the speeds of the
per-direction table, the crouch pill and camera of section 3 and the
0.2 s (6-tick) transition. Three exceptions:

- the **Hunter**'s and the **Flood combat Elite**'s crouching
  collision height equals or exceeds the standing one (0.8): crouching
  only swaps to the slow animations (the Hunter's 4-way crouch walk at
  a quarter speed);
- the crouch seat must hold a weapon type for the held weapon, or the
  seat change fails and the body stays standing (its crouch fraction
  never moves): the **Elite holding the human pistol** (`hp` is in its
  `stand` pistol class but not its `crouch` one) and the **Grunt
  holding a fuel rod** (its `crouch` seat has no `fr` type) cannot
  crouch;
- the Marine and the crewman with the pistol or unarmed have no crouch
  movement animations: crouched, any push is a missing animation (they
  stop in their default pose).

### Turning and aiming

The player's view direction is the unit's wanted aim, wanted look and
wanted body facing all at once. Three things then follow it at their
own pace.

**The aim (where shots go).** Each tick the aim moves toward the
wanted aim on two axes, yaw and pitch, measured in the body's frame.
For each axis the game plans the fastest motion that arrives at rest:
accelerate at the acceleration limit, coast at the velocity limit if
it is reached, decelerate at the acceleration limit, starting from the
current angular velocity; the shorter axis is slowed so both finish
together. The plan is rebuilt every tick and followed for one tick;
when it would finish within the tick the aim is set to the target and
its velocity to zero, and an axis within 0.001 rad of the target with
a velocity below 0.001 counts as there. The wanted aim is first held
inside the current weapon class's aiming bounds about the body, so the
aim (and every shot) can never point further from the body's facing
than those bounds even while the view does.

**The body.** In a move state the body's facing turns toward the
view's yaw at the biped's moving turning speed (a constant rate,
snapping when it would overshoot). Idle, it does not turn until the
view is more than the stationary turning threshold away; then a
turn-left or turn-right animation plays once (14 frames, 90°), and
plays again if the view is still beyond the threshold; when the view
is more than about 154° behind the turn keeps its current side. A
push of the stick interrupts a turn animation at once. In every other
state (airborne, soft or hard landing, the stunned moves, throwing a
grenade) the body does not turn; a plain move state entered without
an animation still turns. The torso twist between the legs and the aim
is purely the aim-still or aim-move overlay (`animations.md`, Aiming
and looking screens): the upper body twists toward the aim within the
aiming bounds and the feet follow only by the turning just described.

| Character | Aim speed limit | Aim acceleration limit | 90° aim flick, rest to rest | Aim bounds about the body (yaw each side; down; up) | Body turn while moving | Idle turn threshold | Idle turn |
|---|---|---|---|---|---|---|---|
| Spartan | none | none | instant | ±90°; 90°; 90° | 337.7°/s | 60° (MP), 80° (campaign) | 7 frames per 90° |
| Elite | 360°/s (2π rad/s) | 720°/s² | 22 ticks (0.73 s; peak 255°/s) | ±90°; 90°; 90° (all its classes) | 337.7°/s (11.3° per tick) | 60° | 14 frames per 90° (193°/s) |
| Grunt | 360°/s | 720°/s² | 22 ticks | ±90°; 90°; 90° | 180°/s (6° per tick) | 60° | as the Elite |
| Jackal | 540°/s (3π) | 2160°/s² (12π) | 13 ticks (0.43 s) | ±90°; 90° (crouched 60°); 90° | 180°/s | 80° | as the Elite |
| Hunter | 180°/s (π) | 540°/s² (3π) | 25 ticks (0.83 s; reaches 180°/s) | ±90°; **45°**; 90° | 180°/s | 60° | as the Elite |
| Marine (rifle) | 360°/s | 720°/s² | 22 ticks | ±90°; 60°; 60° | 300°/s | 90° | as the Elite |

(Flick times are those of the plan: a triangle of 2√(θ/a), or θ/v +
v/a when the velocity limit is reached, rounded up to whole ticks.)

So a player-worn Grunt that swings its view 180° while running fires
at most 90° off its body until the body has turned (15 ticks for the
first 90°), and a Hunter cannot aim more than 45° below level: it
cannot shoot a Grunt at its feet.

The head's look follows the same planner with the looking limits (Elite
720°/s and 1440°/s², Grunt the same, Jackal 2880°/s and 11520°/s²,
Hunter 540°/s and 1080°/s²) inside the seat's looking bounds; it moves
the head overlay only.

### The numbers

Speeds are the cycle's mean root motion; the per-frame range is given
where it varies by more than 5 %. Standing class is the one the
character's default weapon uses. The Elite's, Grunt's, Jackal's and
Hunter's frame-by-frame figures are in "Per-direction numbers" above.

| Character (stand class) | Front | Back | Left / right | Crouch front / back / side | Cycle (frames) |
|---|---|---|---|---|---|
| Spartan (player physics) | 0.075 WU/t, 2.25 WU/s, 6.858 m/s | 0.0667, 2.0, 6.096 | 0.0667, 2.0, 6.096 | 0.03 / 0.0217 / 0.02 | (globals) |
| Elite (pistol) | 0.0865, 2.595, 7.910 m/s | 0.0731, 2.193, 6.684 | 0.0731, 2.193, 6.684 | 0.0333 / 0.0217 / 0.0200 | 26 |
| Elite (sword) | 0.0998, 2.994, 9.126 | 0.0731 | 0.0731 | as pistol | 26 |
| Grunt (pistol) | 0.0628 (0.054 to 0.069), 1.884, 5.742 | 0.0325 (0.028 to 0.035), 0.975, 2.972 | 0.0400 (0.005 to 0.055), 1.2, 3.658 | 0.0087 (0.003 to 0.012) / 0.0325 (the standing back-step) / 0.0111 | 16 / 20 / 20 |
| Grunt (fuel rod, `missle`) | 0.0625, 1.875, 5.715 | 0.0325 | 0.0116 / 0.0110 (0.003 to 0.014) | | 16 / 20 / 23, 24 |
| Jackal (pistol) | 0.0458, 1.374, 4.188 | 0.0375, 1.125, 3.429 | 0.0333, 0.999, 3.045 | 0.0139 / 0.0119 / 0.0133 | 24 |
| Hunter (unarmed) | 0.0773, 2.319, 7.068 | none | none | 0.0189 (0.010 to 0.031) / 0.0189 / 0.0167 | 26; crouch 22, 23 |
| Marine (rifle) | 0.0700, 2.1, 6.401 | 0.0500, 1.5, 4.572 | 0.0583 (0.054 to 0.063), 1.749, 5.331 | 0.0326 / 0.0326 / 0.0175 left, 0.0230 right | 20; crouch 24 |
| Marine, crewman (pistol, unarmed) | 0.0750, 2.25, 6.858 | 0.0500 | 0.0452, 1.356, 4.133 | (no crouch moves in these classes) | 20 |
| Flood combat Elite | two permutations re-drawn each loop: 0.0625 (24 frames) or 0.0800 (22 frames); long-run mean 0.0709, 2.126 WU/s, 6.480 m/s | none | none | 0.0181 (0.014 to 0.022) front only | 24 or 22; crouch 36 |
| Flood combat human | 0.0614, 1.842, 5.614 | 0.0614 | none | 0.0083 (0.0003 to 0.015) front only | 30; crouch 65 |
| Flood carrier | 0.0333, 0.999, 3.045 | none | none | 0.0103 | 24 |
| Flood infection | 0.0431, 1.293, 3.941 | none | none | 0.0040 | 27 |

Moving, the Elite outruns the Spartan by 15 % forward; the Grunt runs
at 84 % forward and 49 % backward of the Spartan; the Jackal at 61 %;
the Hunter at 103 % but only forward.

| Character | Moving turning speed | Stationary threshold | Aim velocity / acceleration limit | Look velocity / acceleration | Jump (WU/t) | Soft / hard landing min, hard max (WU/s); hard time |
|---|---|---|---|---|---|---|
| Spartan (MP) | 5.894 rad/s (337.7°/s) | 60° | 0 / 0 (instant) | 0 / 0 | 0.07 | 1.5 / 5 / 8; 1.0 s |
| Elite | 5.894 | 60° | 2π / 4π | 4π / 8π | 0.07 | 1 / 3 / 6; 2.0 s |
| Grunt | **π (180°/s)** | 60° | 2π / 4π | 4π / 8π | 0.07 | **0.5 / 1** / 6; 2.0 s |
| Jackal | π | 80° | 3π / 12π | 16π / 64π | 0.07 | 1 / 3 / 6; 2.0 s |
| Hunter | π | 60° | **π / 3π** | 3π / 6π | 0.07 | 1 / 3 / 6; 2.0 s |
| Marine, crewman | 5.236 (300°/s) | 90° | 2π / 4π | 4π / 8π | 0.07 | 1.5 / 5 / 8; 2.0 s |
| Flood combat (both) | 5.894 | 60° Elite, 90° human | 4π / 8π | 4π / 8π | **0.16** | 1 / 3 / 6; 1.5 s |
| Flood carrier | 5.894 | 60° | 2.22π / 6.67π | | 0.07 | 1 / 3 / 6; 1.5 s |
| Flood infection | 5.894 | 90° | 4π / 8π | | 0.05 | 0.5 / 1.5 / 4; 0.7 s |

(Rates in rad/s and rad/s²; the casual aiming modifier, 0.7 to 0.8,
applies only to the AI's "casual" aim.)

### What comes from behaviour, not the body

An AI-driven character's motion also contains things the player would
not have, because the actor (the AI) decides them:

- **Which seat**: the AI walks in `alert` (Elite 0.0191 WU/t, Grunt
  0.0153, Jackal 0.0154, Marine 0.0141 rifle and 0.0225 pistol: the
  slow patrol walk), runs in `stand`, flees in `flee` (Grunt 0.0625,
  Jackal 0.0611, Marine 0.0824, Flood Elite 0.12, Flood human 0.1085:
  faster than any standing walk) and burns in `flaming`. A player-worn
  body is always in `stand` or `crouch`.
- **Dives, evades, surprise and berserk** animations (with their own
  root motion) are started by the AI only.
- **The Flood's leap** (leap-start, leap-airborne, leap-melee) is
  aimed by the actor; a player has no input for it.
- **Crouch timing, cover and when to run**: the actor variant's
  movement switching fields (0x4C onward).
- **Major upgrades, vitality overrides and rank colours** come from the
  actor variant (section 5), not the biped.
- **The aim's casual modifier** (slower aim when not in combat).

## 5. Vitality

### The collision models' numbers (the body's own)

| Character | Body max | Shield max (type) | Shield delay (stun time) | Recharge (empty to full) | Notes |
|---|---|---|---|---|---|
| Spartan | 75 | 75 (22) | 6.0 s (180 ticks) | 4.0 s | `health_shields.md` |
| Elite (`elite`) | 100 | 100 (30 elite energy shield) | **12.0 s** (360 ticks) | 10.0 s (0.0033333334 per tick) | |
| Elite special | 100 | 100 (30) | 8.0 s (240 ticks) | 10.0 s | |
| Grunt | 40 | none | | | |
| Jackal | 60 | 100 (16, the arm shield) | 15.0 s (450 ticks) | **0 s: the recharge velocity is 1, full in one tick** | body never shielded (leak 1) |
| Hunter | 200 | none | | | |
| Marine (both) | 50 | 50 (24 armored human), **failing** | 8.0 s | 1.0 s (0.033333335 per tick) | failure threshold 1, linear, failing leak 0.2 |
| Crewman | 25 | none | | | head region kills at half body |
| Flood combat Elite | 80 | none (max 0) | | | |
| Flood combat human | 80 | none | | | |
| Flood carrier | 40 | none | | | |
| Flood infection | 80 | none | | | the actor variant sets 0.01 |
| Sentinel | 80 | 80 (30) | 12 s | 10 s | |
| Monitor | 0 | 10 000 000 000 (26) | | | effectively invulnerable |

### The campaign's overrides

When an actor is created from an actor variant whose maximum body or
shield vitality (0x200, 0x204) is above 0, both maxima of the unit are
replaced by the variant's (a 0 then means none). The campaign's values
are in section 2's table; the difficulty's enemy vitality, shield and
recharge rows scale them further in the campaign (`health_shields.md`:
1.0 at normal, the only level multiplayer uses). A player-worn body
created without an actor keeps the collision model's maxima. The
shield delay and recharge rate always come from the collision model.

### What matters about each

1. **Headshots.** A damage effect that "can cause headshots" (pistol,
   sniper rifle) kills outright when any body damage lands on a
   material with the head flag (in multiplayer: any target; in the
   campaign: any target but a player). Heads by character: Elite and
   Elite special `head`; Grunt `head`; Jackal `head`; Hunter **`skin`
   (the exposed orange flesh of the back and belly)**; Marine and
   crewman `head`; Spartan `head`. The Flood forms have **no** head
   material: no headshots. So with the pistol or the sniper rifle:
   - a Grunt, a Jackal and a crewman die to one head shot (no shield,
     or a shield that does not cover the head);
   - a Marine dies to one head shot **even at full shield**, because
     its materials leak 20 % of every hit through the shield, so the
     body damage is never zero;
   - a Hunter dies to one shot on the skin;
   - an Elite (and the Spartan) only once its shield is down (the
     sniper's overflow does it in one shot, section "Hits to kill").
   The plasma rifle's "multiplayer headshot" doubles body damage on a
   head material in a multiplayer game.
2. **The Jackal's shield.** Every material but `shield` leaks 100 % to
   the body with shield multiplier 0, so a hit anywhere on the Jackal
   ignores the shield vitality entirely. A hit on the `shield` material
   goes to the shield (multiplier 1, body multiplier 0): it can deplete
   the shield but never hurts the body. Once the shield is down the
   `shield` material is no longer there to hit: the disc's collision
   geometry is swapped for the emitter's (section 2, rule 7), so shots
   aimed at the disc pass through to whatever is behind, the Jackal's
   body included.
   Area damage uses the indirect material (`unarmored`): explosions
   ignore the shield. The weapons that hurt the arm shield are those
   whose modifier for type 16 is not 0 (plasma rifle 1, plasma pistol
   1, charged plasma pistol 1, plasma grenade 1.5); the bullet weapons'
   is 0 (and `projectiles.md` reflects their bullets off it). After 15
   s without shield damage the shield returns at once.
3. **The Hunter.** Its armour (type 12) takes reduced damage from
   nearly everything (assault rifle 0.5, pistol 0.2, plasma 0.5,
   shotgun 0.2, frag 0.25, fuel rod 0); the shield arm and spikes have
   body multiplier 0; the skin takes full damage and is a head. Its
   unit is "never unsuspecting": a melee from behind does not kill it
   outright. The fuel rod's damage modifier for both Hunter types is 0:
   Hunters are immune to fuel rods.
4. **The Marine's failing shield.** With failure threshold 1 the
   shield is always failing: shield damage is multiplied by 0.8·σ + 0.2
   (σ the shield fraction) after the 20 % leak, the rest going to the
   body. The shield therefore never reaches exactly zero from small
   hits; the Marine usually dies with some shield left.
5. **The Flood's limbs.** Body hits on a Flood combat form's region
   accumulate as `health_shields.md` describes; past 10 % (head) or 12
   to 13 % (arms, legs) of the body the region is destroyed, plays its
   explosion effect and applies its flags: losing the right arm drops
   the weapon and stops firing; the left arm stops melee; the legs stop
   walking; the head makes the maimed scream. The body vitality is
   unaffected by the loss.
6. **The crewman's head region** is flagged "forces object to die" at
   0.5: half its body points of damage on the head kills it.
7. **Stun.** Pings and body stun are `health_shields.md`'s, with each
   unit's soft and hard ping thresholds: Elite hard ping 0.3 (0.5 s),
   Grunt 0.25 (0.2 s), spec-ops Grunt 0.3, Jackal 0.2, Hunter 0.25
   (0.25 s) and "does not reorient during pings", Marine 0 (soft ping
   0.3 s), Flood combat 0.13 (Elite) / 0.03 (human) with "resists
   pings". Hard death thresholds: Elite 0.3, Grunt 0.4, Jackal 0.4,
   Hunter 0.2, Marine 0.4, Flood 0.26.
8. **Units that never become unsuspecting** (bit 16 of the unit
   flags): Hunter, Flood combat forms, carrier. A melee from behind
   does not kill them outright.
9. **The carrier** has "melee attack is fatal": starting its melee
   kills it (it bursts and its body destroyed damage hurts those near).

### Death

At zero (`health_shields.md`, Death) the unit plays the damage table's
kill animation for the hit's direction and body part. The animations
the graphs actually hold:

| Character | Hard kill (h-kill) | Soft kill (s-kill) | Ping animations | Airborne and landing dead |
|---|---|---|---|---|
| Spartan | front, back, left, right gut; front, left, right head | front, back, left, right gut; front chest | 1 | yes |
| Elite (both) | front, back gut | front, back, left, right gut | 18 | yes |
| Grunt (both) | front, back gut | front, left, right gut | 13 | yes |
| Jackal (both) | front, back gut | front, back, left, right gut | 15 | yes |
| Hunter | front, back gut | front, left, right gut | 10 | yes |
| Marine, crewman | as the Spartan | as the Spartan | 11 | yes |
| Flood combat Elite | front, back gut | front, back, left, right gut | 11 | yes |
| Flood combat human | front, back gut | none | 11 | yes |
| Flood carrier | front, back gut | front, back gut | 1 | yes |
| Flood infection | front gut | none | 1 | yes |

The Flood combat forms also have resurrect-front and resurrect-back:
the AI raises dead ones (Flood "resurrection"; AI only). Every
humanoid character has the limp-body flag (biped flags bit 9) like the
Spartan.

### Hits to kill

Against a full-health character of the collision model's maxima (the
body's own numbers, not a campaign rank), multiplayer rules (headshots
for everyone, normal difficulty, no health option), point blank (scale
1), explosions centred on the target (scale 1, indirect material),
computed by a scratch simulation of `health_shields.md`'s rules in
single precision with the multiplayer weapons' damage effects as
Blood Gulch holds them. The simulation reproduces every row of
`health_shields.md`'s Spartan table (first row). "Body" is the
character's main body material (`body`, or for the Hunter its armour);
"head" its head material. For random amounts the range over always
lowest and always highest draws.

| Character | AR | Pistol | Plasma rifle | Plasma pistol | Charged PP | Needles | Shotgun pellets | Sniper | Rocket | Flame hits | Fuel rod | Frag | Plasma grenade | Melee (standing) |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| Spartan | 16 / 16 | 5 / 3 | 14-15 / 8-9 | 13-16 | 3 | 16 | 9-13 | 2 / 1 | 1 | 16 | 3 | 2 | 2 | 3 |
| Elite | 25 / 25 | 9 / 5 | 11-12 / 7-8 | 8-9 | 3 | 20 | 9-12 | 1 / 1 | 1 | 35 | 3 | 2 | 1 | 6 |
| Grunt | 6 / 5 | 3 / 1 | 4-5 / 2 | 3-4 / 3 | 1 | 5 | 3 / 2-3 | 1 / 1 | 1 | 5 | 1 | 1 | 1 | 2 |
| Jackal (body) | 6 / 6 | 3 / 1 | 5 / 3 | 3-4 | 1 | 6 | 3-4 | 1 / 1 | 1 | 6 | 1 | 1 | 1 | 2 |
| Hunter (armour / skin) | 41 / 41 | 41 / **1** | 29-34 / 15-17 | 20-25 | 6 | 41 | 41-56 / 9-12 | 4 / **1** | 1 | 101 | **never** | 7 | 7 | 6 |
| Marine | 10 / 10 | 4 / **1** | 7-9 / 5-6 | 5-7 | 5 | 10 | 4-6 | 1 / 1 | 1 | 10 | 2 | 1 | 1 | 3 |
| Crewman | 3 / 3 | 2 / 1 | 2-3 / 1-2 | 2 | 2 | 3 | 2 | 1 / 1 | 1 | 3 | 1 | 1 | 1 | 1 |
| Flood combat (either) | 13 | 3 | 6-7 | 5-6 | 2 | 7 | 3 | 16 | 1 | 5 | 1 | 1 | 1 | 10 |
| Flood carrier | 7 | 2 | 3-4 | 3 | 1 | 4 | 2 | 8 | 1 | 3 | 1 | 1 | 1 | 6 |

(One number: body and head the same, or no head. Shotgun: pellets; 15
per shell, so every character but the Hunter dies to one shell at
point blank. Needles: detonations of 10 with no material. Jackal
shots on the arm shield: plasma rifle 8, plasma pistol 6, charged
plasma pistol 1 to deplete it, never any body damage. The Marine's
shield never breaks first: the failing shield is still above 0 at
death for every weapon but the sniper rifle.)

The modifiers that make these: the elite energy shield (type 30) takes
×0.7 from the assault rifle, ×2 from plasma, ×2 from the sniper
rifle, ×4 from the plasma grenade and the needler's supercombine, ×1
from the multiplayer pistol (the campaign's pistol has ×0.8: the only
multiplayer damage effect that differs between Blood Gulch and the
campaign maps); the elite body takes ×0.4 from flame; the Flood forms
take ×0.05 from the sniper rifle, ×1.2 from needles, ×1.5 to ×1.75
from pistol and shotgun, ×2 to ×4 from explosions and flame.

## 6. Voice

### The dialogue tags

A unit's dialogue is chosen per unit from its biped's dialogue
variants block (`sounds.md`, Unit dialogue): a unit created with
variant number 0 takes the next of the variants numbered below 100 in
turn (a game-wide counter cycles through them); numbers 100 and up
(the Sergeants) are only for units placed with that variant number.

| Character | Dialogue tag | Filled slots | Lines (permutations) |
|---|---|---|---|
| Spartan | `sound\dialog\chief\chief` | 5 (deaths only) | 71 |
| Elite (both) | `sound\dialog\elite\conditional\elite` | 53 | 271 |
| Grunt (both) | `sound\dialog\grunt\conditional\grunt` | 65 | 459 |
| Jackal (both) | `sound\dialog\jackal\jackal` | 63 | 577 |
| Hunter | `sound\dialog\hunter\hunter` | 8 | 90 |
| Marine (every Marine biped) | variants 0: `marines\bisenti`, `marines\fitzgerald`, `marines\aussie`, `marines\mendoza` (each under `sound\dialog\...\conditional\`); 100: `sargeant`; 101: `sarge2` | 125 to 128 each | 521, 667, 534, 627; 694; 924 |
| Crewman | `sound\dialog\crewman\conditional\crewman` | 40 (no pain or death lines) | 112 |
| Flood combat (both) | `sound\dialog\flood\flood` | 18 | 136 |
| Flood carrier, infection | none | | |
| Keyes in-game | `sound\dialog\captain\captain` | 8 | 30 |
| Monitor | `sound\dialog\monitor\monitor` | 6 | 50 (distances 15 to 30) |

Every line is sound class "unit dialog" with zero distances in the
tag, so the class defaults apply: full volume within 3 WU, silent
beyond 20 WU (`sounds.md`, Distance), gain modifier as the sound tag
says.

The slots each fills, with their line counts:

- **Elite**: idle noncombat 4; pain body minor 16, major 14, falling
  10; scream pain 11; death quiet 10, violent 13, falling 10, agonizing
  13, instant 17, flying 8; damaged friend 3, enemy 2, enemy cm 3; hurt
  friend 3, friend re 3, enemy 6, enemy grenade 11, enemy melee 2;
  killed friend 3, enemy 8, enemy cm 3, enemy player 7, enemy player cm
  3; friend died 3, killed by friend 3, by enemy player 2, by flood 2;
  new combat alone 3, new enemy recent combat 2, old enemy sighted 3,
  dead friend found 3; grenade throwing 2, startle 2, sighted 2,
  danger enemy 3, danger friend 3; new combat group re 3, nearby re 3;
  alert friend 3, re 2; search query 3, re 3, report 3; uncover start
  5, re 3; cover 5; shooting 2; **taunt 9**; flee re 3; berserk 2;
  melee attack 3; dive 3.
- **Grunt**: idle noncombat 8, combat 8, flee 5; pain body minor 12,
  major 6, falling 2; scream fear 6, pain 6; death quiet 6, violent 4,
  falling 3, agonizing 4, instant 5, flying 3; damaged friend 4, enemy
  10, enemy cm 4; hurt friend 3, re 4, enemy 7, needler 3, plasma 3,
  grenade 6, **melee 20**; killed friend 4, enemy 10, enemy cm 8, enemy
  player 14, player cm 6, enemy grenade 15, mountedweapon 5; friend
  died 6, killed by friend 6, by enemy player 7, by flood 6, by
  sentinel 4; new combat alone 9, recent 5, old enemy sighted 10, dead
  friend found 6; grenade throwing 7, startle 4, sighted 2, danger
  enemy 7, self 5, friend 6; group re 5, nearby re 5; alert friend 6,
  re 2; search query 5, re 6, report 6; uncover start 11, re 4;
  **retreat 24**; cover 4; shooting 10; **taunt 9**; **flee 24**, re 10,
  leader died 4; surprise 9; dive 7; **celebration 14**.
- **Jackal**: as the Grunt's set without idle combat, idle flee, hurt
  enemy plasma's neighbours, with: idle noncombat 12; pain minor 7,
  major 4, falling 2; scream fear 7, pain 7; deaths 5, 4, 3, 4, 5, 3;
  damaged friend 9, enemy 20, cm 9; hurt friend 9, re 9, enemy 12,
  needler 9, plasma 7, grenade 7, melee 13; killed friend 9, enemy 18,
  cm 18, enemy player 21, player cm 21, grenade 10, mountedweapon 6;
  friend died 10, by friend 8, by enemy player 13, by flood 12, by
  sentinel 10; new combat 7, recent 10, old enemy 15, dead friend 10;
  grenades 8, 7, 4, 7, 6, 8; group re 13, nearby re 7; alert friend 8,
  re 4; search query 8, re 7, report 7; uncover 8, re 7; retreat 17;
  cover 9; shooting 20; taunt 8; flee 17, re 4, leader died 8; surprise
  3; dive 8; celebration 9.
- **Hunter**: pain body minor 12, major 13; death violent 6, falling
  10, instant 19; killed enemy 9; melee attack 10; dive 11.
- **Flood**: idle noncombat 5, combat 15; pain body minor 6, major 6,
  falling 6; maimed limb 6, head 6; death quiet, violent, falling,
  agonizing, instant, flying 8 each; berserk 5; melee attack 10; dive
  10; leap attack 5; resurrection 8.
- **Marines**: every slot from pain body minor to rout re except a few
  (no pain shield on four of the six; idle and taunt slots empty): the
  full squad-chatter set.

Empty slots fall back as `sounds.md`'s table says (so an Elite's
missing pain shield plays pain body minor, a Hunter's missing death
quiet plays death violent).

### What a player-worn character says

Only three things in the game speak for a unit that has no actor:

1. **Damage** (every non-silent hit that does body or shield damage, or
   kills): the pain and death lines exactly as `sounds.md` (Death line
   choice, Pain line choice) describes. A unit without an actor is
   "severe" only when its recent body damage exceeds the killing hit's
   plus 0.2; otherwise its death line is death quiet (falling: death
   falling; flame: death agonizing; a damage effect with instantaneous
   acceleration 2 or more: death flying). A violent or instant death
   also alerts nearby AI (no AI in our game).
2. **Region loss**: when a region is destroyed (the Flood's limbs, the
   crewman's head) the unit screams maimed limb, or maimed head for a
   region flagged "causes head-maimed scream".
3. **A dangerous fall**: scream fear when the biped falls fast enough
   to be hurt.

(The Spartan's dialogue fills only deaths, so in multiplayer a hit
Spartan is silent. A player-worn Grunt, Elite, Jackal, Hunter, Marine
or Flood **does** cry out when hit: their tags fill the pain slots.)

Everything else is spoken by the AI's communication system, which
picks a vocalization from game events and the actor's state: idle
chatter, kills and boasts (killed enemy, killed enemy player and the
weapon-specific ones), taunts, celebrations, grenade throws and
warnings, friend-died reactions, search, cover, retreat and flee
calls, berserk, surprise and dive shouts. A player-worn body never
says these in the game. If we want "chatter", the lines that answer to
events a player causes are, by slot: killed enemy (or killed enemy
player) after a kill, the weapon-specific killed enemy lines (bullet,
needler, plasma, sniper, grenade, explosion, melee, flame, shotgun),
killing spree, grenade throwing on a throw, melee attack on a melee,
taunt and celebration (a "taunt" button), hurt enemy and its weapon
variants after damaging someone, shooting while firing. That mapping
is ours, not the game's.

### Playing a line

As `sounds.md` (Playing a line): an impulse sound on the unit's `head`
marker, priorities pain 2, involuntary 7, death 10, a 7-tick pause
after each damage line, the minor-pain limit of 3 lines and the
one-major-pain-per-life rule. Lines are positional: others hear them
within 20 WU.

## 7. First person

1. **No character graph has first-person animations**: the first
   person weapons block of every character graph on the ten maps is
   empty (count 0), as is the Spartan's third-person graph's.
2. **First person is not part of the character.** The first-person
   arms model is the globals' (`characters\cyborg\fp\fp`, 37 nodes,
   `globals.md` First person interface) and each weapon names its own
   first-person model and graph (`weapons.md`, at 0x154 and 0x164 of
   the weapon part); the graph poses the arms and the gun together
   (`animations.md`, First person). So the fallback "the Spartan's
   first-person hands" is simply the game's normal first-person view
   with any character: nothing in the character changes it.
3. **Weapons with no first-person view**: `weapons\fuel rod gun\fuel
   rod`, `weapons\fuel rod gun\hunter fuel rod`, `weapons\energy
   sword\energy sword` and every vehicle gun have no first-person model
   and no first-person graph. A player-worn Grunt with a fuel rod, or a
   Hunter, has nothing to draw in first person and no first-person
   animation to time its ready, reload or melee (Open questions). The
   flamethrower has both (`weapons\flamethrower\fp\fp`).
4. The first-person camera height is the biped's standing or
   crouching camera height (section 3), which for the Grunt is below
   its head.

## 8. Each character as a unit to fetch

### The closure

A character's unit is every tag reachable from its biped tag through
tag references, recursively, **excluding** actor tags (`actv`,
`actr`: AI only), weapons (`weap`: each is its own unit), and HUD
interfaces (`unhi`, `wphi`, `grhi`: the HUD is the Spartan's). It
includes the model, collision model, animation graph, shaders,
bitmaps, the dialogue and every sound it names, footsteps and the
material effects they reach, effects, particles, decals, lights,
garbage pieces, the melee damage effect and attachments.

The size is the tag data plus the raw data:

- **Tag data**: each tag's bytes in the map's tag data, from its main
  struct's address to the next tag's (tags and their blocks are laid
  out contiguously in tag index order; the model vertex and index
  buffers come before the first tag and are not in any tag's span);
- **Raw data**: bitmaps' pixel data (the sum of each image's pixel
  size), sounds' samples (each permutation's sample size), and models'
  geometry (32 bytes per compressed vertex plus 2 per strip index).

Measured on every map holding the biped; the numbers are identical on
every map (the tags are byte-for-byte the same build).

| Character (biped) | Tags | Tag data | Raw data | Total bytes | Of which sound | Smallest map holding it (file size) |
|---|---|---|---|---|---|---|
| Spartan, campaign (`characters\cyborg\cyborg`) | 137 | 984 752 | 3 387 948 | **4 372 700** | | c20 (76 980 224) |
| Spartan, multiplayer (`characters\cyborg_mp\cyborg_mp`, Blood Gulch) | 136 | 984 724 | 3 385 284 | 4 370 008 | | bloodgulch |
| Elite (`characters\elite\elite`) | 161 | 1 013 828 | 5 668 994 | **6 682 822** | 4 503 360 (105 sounds) | d20 (131 012 608) |
| Elite special (`characters\elite\elite special`) | 164 | 1 012 004 | 5 148 458 | **6 160 462** | 4 526 444 (106) | d20 |
| Grunt (`characters\grunt\grunt`) | 169 | 616 652 | 11 536 976 | **12 153 628** | 10 662 140 (120) | d20 (131 012 608) |
| Grunt spec-ops (`characters\grunt\grunt specops`) | 169 | 616 652 | 11 536 976 | 12 153 628 | 10 662 140 | d20 |
| Jackal (`characters\jackal\jackal`, and `jackal major`) | 173 | 730 168 | 7 816 054 | **8 546 222** | 6 959 968 (107) | d20 |
| Hunter (`characters\hunter\hunter`) | 128 | 617 588 | 3 972 938 | **4 590 526** | 3 348 884 (62) | d20 |
| Marine (`characters\marine\marine`) | 875 | 1 911 076 | 57 221 794 | **59 132 870** | 57 005 956 (791) | a30 (152 610 816) |
| Marine armoured (`characters\marine_armored\marine_armored`) | 885 | 1 911 904 | 57 685 810 | **59 597 714** | 57 005 956 (791) | a30 |
| Marine armoured sniper | 645 | 1 623 628 | 36 436 450 | 38 060 078 | 35 476 592 (553; 4 voices) | a30 |
| Marine unarmed (the plasma-rifle Marine: 64 bytes less) | 814 | 1 853 448 | 55 699 302 | 57 552 750 | 55 492 572 (741) | c10 (166 926 336); the plasma-rifle Marine only on a50 (176 224 256) |
| Crewman (`characters\crewman\crewman`) | 188 | 1 125 952 | 4 491 670 | **5 617 622** | 4 140 364 (119) | a30 |
| Flood combat Elite | 97 | 695 776 | 2 360 116 | **3 055 892** | 1 160 780 (28) | c20 (76 980 224) |
| Flood combat human | 90 | 598 956 | 2 163 756 | **2 762 712** | 1 324 856 (25) | c20 |
| Flood carrier | 95 | 243 356 | 2 530 412 | 2 773 768 | 1 475 208 (19) | c20 |
| Flood infection | 31 | 91 544 | 852 192 | 943 736 | 618 644 (4) | c20 |
| Sentinel | 65 | 153 568 | 1 258 772 | 1 412 340 | | c20 |
| Engineer | 53 | 269 116 | 706 846 | 975 962 | | b30 |
| Monitor | 36 | 33 400 | 1 724 788 | 1 758 188 | | c20 |
| Keyes (`characters\captain\captain`) | 40 | 198 520 | 386 596 | 585 116 | | c10 |

(No Covenant character is on c20; d20 is the smallest map holding
every Covenant candidate: Elite, Elite special, Grunt, Grunt
spec-ops, Jackal, Jackal major, Hunter. c20 holds every Flood form;
a30 the Marine, the armoured Marine, the armoured sniper and the
crewman. Map file sizes are the compressed files on the disc.)

By class, the Elite's unit: animation graph 1 (854 452 bytes), biped 1
(1 972), bitmaps 23 (878 100), collision model 1 (22 792), decals 1,
effects 5, footsteps 1, damage effect 1, model 1 (398 118), particles
7, point physics 4, sounds 105 (4 503 360), sound scenery 7, sound
environment 1, the dialogue 1 (4 112). The Marine's six voices are 57
MB of its 59.6 MB: a Marine unit with one voice (variant 0's first,
Bisenti) would be roughly 1.9 MB of tag data plus the shared lines;
splitting voices into their own units is the obvious economy (Open
questions).

### The weapons characters need beyond the multiplayer set

Measured the same way (HUD interfaces excluded):

| Weapon | Tags | Total bytes | First-person model |
|---|---|---|---|
| `weapons\fuel rod gun\fuel rod` (Grunt) | 96 | 1 224 992 | none |
| `weapons\fuel rod gun\hunter fuel rod` | 81 | 1 016 094 | none (no world model either) |
| `weapons\energy sword\energy sword` | 34 | 212 018 | none |
| `weapons\flamethrower\flamethrower` | 67 | 1 762 942 | yes |
| `weapons\plasma pistol\plasma pistol` (for scale) | 270 | 3 110 788 | yes |

A character unit plus its default weapon is the download for "play as
X": the Hunter needs its 1.0 MB cannon, the spec-ops Grunt's fuel rod
1.2 MB; the others hold weapons the multiplayer set already has.

## Per-tick order

For a player-worn character, `movement.md`'s per-tick order with these
replacements, in place:

1. Input as `movement.md` section 1 (the throttle, dead zone, crouch).
2. The unit's aim moves toward the wanted aim within the unit's
   aiming limits (section 4 rule 7), then its look within the looking
   limits.
3. The body turns: in a plain move state toward the view's yaw at the
   moving turning speed; idle, it asks for a turn animation when the
   view is beyond the stationary threshold (section 4, Turning and
   aiming).
4. The biped picks its wanted movement state from the throttle
   (section 4 rule 2; a push overrides a wanted turn) and builds its
   wish from the frame the current animation is showing (section 4,
   Root motion, tick by tick): zero during a hard landing; the frame's
   (dx, dy) in the body's frame otherwise, the frame's yaw applied to
   the body. If not airborne or slipping the acceleration limit is
   infinite (the velocity becomes the wish); slipping, 0.0053333 WU/t²;
   airborne, 0 (no air control). The crouch fraction moves.
5. The physics move (`movement.md` section 5) runs; then the landing
   check, the jump (section 4, Jumping and crouching), falling damage.
6. Airborne (after 3 ticks), landing or dead states override the
   wanted state.
7. The animation advances one frame (`animations.md`, Playback); if the
   wanted state differs (and the current one may be interrupted) the
   new state starts at frame 0 with a permutation drawn now, or with
   no animation if its slot is empty (section 4, A missing animation).
8. Damage, shield recharge, dialogue (`health_shields.md`,
   `sounds.md`) as for any unit.

## Edge cases

- **A weapon the graph cannot select** (an Elite picking up an assault
  rifle while standing): the unit keeps its previous weapon animation
  set; the new weapon is attached to that class's hand marker and fires
  normally. Our eligibility rule forbids the pickup instead.
- **Crouched selection differs**: the Elite's `crouch` seat has an `ar`
  type (with no slots), the `stand` seat has none; selecting while
  crouched succeeds and leaves an empty animation set. We treat
  `stand` as the authority.
- **The Hunter pushed backward while standing** stops (even if it was
  walking forward) and blends into its default pose; crouched it walks
  in all four directions at a quarter speed (section 4, A missing
  animation).
- **A crouch the seat cannot take** (Elite with the human pistol, Grunt
  with the fuel rod) leaves the body standing with the standing
  animations and the standing pill (section 4, Jumping and crouching).
- **Body stun on a player-worn Covenant or Flood body** in multiplayer
  freezes it in its default pose while the stick is pushed (section 4,
  A missing animation).
- **The Grunt's crouched back-step** is the standing back-step
  animation (0.0325 WU/t): crouched, a Grunt backs away faster than it
  walks forward (0.0087).
- **A destroyed right arm** on a Flood combat form drops its weapon and
  inhibits firing; a player-worn Flood body loses its gun for the rest
  of its life.
- **The Jackal's shield at zero** has no collision disc: its region's
  collision BSP is the emitter's (section 2, rule 7), so nothing is
  blocked until the shield returns 15 s after the last shield damage.
- **Infection forms** have body 80 in the collision model but every
  actor variant sets 0.01: as placed in the game they die to anything.
- **The Monitor** has 10¹⁰ shield points and no body: unkillable.

## Quirks

- **Animation-driven speeds are not the globals'**: the Elite runs
  faster than the Spartan (2.595 WU/s against 2.25) and the Flood
  flee 60 % faster; faithful player-worn bodies will feel unfair.
  This is intended by the brief (tune later).
- **Throttle magnitude is ignored** by animation-driven bodies: a half
  push walks at full speed (silently below half a push), and a
  diagonal push walks straight along the dominant axis.
- **Start and stop are instant, one tick late**: no ramp, no slide
  (section 4, Root motion, tick by tick).
- **A soft landing stops a character** for up to 18 ticks (the
  Spartan keeps running through his); **a Grunt hard-lands from its
  own jump** and stands braking for 24 ticks.
- **A player-worn Flood combat form jumps 3.5 WU** (0.16 WU/t, the
  leap's speed) and takes falling damage from its own jump.
- **A missing animation is a frozen default pose**: a Hunter pushed
  backward, a Flood Elite pushed sideways, a stunned Covenant body.
  Faithful, never seen in the campaign; likely to be replaced (Open
  questions).
- **A push turns the body** to face away from the blast, and the body
  then walks that way until it turns back.
- **The Hunter cannot aim more than 45° below level**, and every
  character's shots stay within 90° of its body's facing.
- **Aim lag for player-worn bodies**: a player Grunt's aim turns at
  most 360°/s and accelerates at 720°/s²; a Hunter's at 180°/s. Bullets
  of a unit without "fires from camera" (only the Spartan has it) leave
  along the unit's aim, so the lag reaches the shots.
- **The Grunt's camera is at its chest** (0.28 WU).
- **The Jackal's shield recharges instantly** after its 15-second wait
  (recharge time 0 gives a per-tick velocity of 1).
- **The Marine dies to one pistol head shot at full shield** (the 20 %
  leak).
- **The campaign pistol differs from the multiplayer pistol** only in
  its elite-shield modifier (0.8 against 1.0).
- The Grunt's fuel-rod class is spelled `missle` in its graph; labels
  are compared by weapon type, never by class, so it does not matter.

## Test cases

### (a) Synthetic

1. **Eligibility from a graph.** A graph with a `stand` seat holding
   class `pistol` (slots idle, move-front, aim-still) with types `pr`
   (fire-1, reload-1) and `hp` (no slots), class `rifle` (no idle) with
   type `ar`, and a `crouch` seat with class `rifle` holding type `sg`:
   - `pr` → can hold, fully animated;
   - `hp` → can hold, missing fire-1 and reload-1;
   - `ar` → can hold, missing idle, move and aim (class lacks them);
   - `sg` → never (only crouched);
   - `rl` → never;
   - an empty label (`unarmed`) → never (no type with an empty label).
2. **Selection keeps the previous set.** Same graph, unit holding `pr`
   (stand, pistol, pr); switch to `rl`: the selection stays (stand,
   pistol, pr).
3. **Movement state and root motion.** A biped without the player
   physics flag, `stand` seat with move-front of 4 frames dx = 0.05,
   0.06, 0.07, 0.06 and no move-back, an idle with zero frame info,
   standing still on flat ground facing +X: throttle (1, 0) from tick 1
   → velocity 0 on tick 1, then 0.05, 0.06, 0.07, 0.06, 0.05 ... WU/t
   forward from tick 2; throttle (0.3, 0) → the same; throttle (0.7,
   0.7) → the same (front wins ties); throttle (0.5, 0.6) → left (no
   animation: no motion, default pose); throttle (−1, 0) → back (the
   same). Walking forward, then (−1, 0) on tick C: tick C still moves
   by the current front frame, tick C + 1 and after by 0 (it does not
   keep walking forward). Releasing on tick R: tick R moves by the
   current frame, tick R + 1 by 0.
4. **No air control.** That biped airborne with horizontal velocity
   (0.05, 0) and throttle (0, 1) keeps (0.05, 0) until it lands.
5. **Aim limit.** A unit with aiming velocity maximum 2π rad/s and
   acceleration maximum 4π rad/s², aim at rest, wanted aim 90° away in
   yaw (inside ±90° bounds): the per-tick limits are 0.20944 rad/t and
   0.0139626 rad/t²; the aim accelerates for 10.6 ticks to a peak of
   0.148 rad/t (below the velocity limit), decelerates as long, and is
   set exactly on the target on tick 22 with zero velocity, never in
   one tick as the Spartan's is. (An earlier draft had it accelerate
   all the way and arrive after 15 ticks; the game plans to arrive at
   rest.) With limits π and 3π (the Hunter) the aim reaches the
   velocity limit 0.10472 rad/t after 10 ticks, coasts 5, decelerates
   10: about 25 ticks. A wanted aim 120° from the body's facing is held
   at the 90° bound until the body turns.
6. **Hits-to-kill rows** (single precision, `health_shields.md` rules,
   multiplayer headshots):
   - Grunt (body 40, no shield; materials head ×1 head, body ×0.8,
     armor ×1): pistol (25, headshots) on body: 25 × 0.8 / 40 = 0.5 →
     β 0.5, 0.0, −0.5: **3**; on head: **1**. Assault rifle (10) on
     body: x = 0.2 per hit; in f32 β runs 0.8, 0.6, 0.40000004,
     0.20000003, 2.9802322e-08 (not below 0: alive), −0.19999997:
     **6**; on head (×1): 0.25 per hit: 0.75, 0.5, 0.25, 0, −0.25:
     **5**.
   - Elite (shield 100 type 30, body 100): sniper (101, modifier 2 on
     the shield): shield points 202 > 100, excess 102 goes to the body
     → β = 1 − 1.02 < 0: **1**. Pistol (25, multiplayer modifier 1) on
     body: shield 75, 50, 25, 0 (the fourth leaves δ = σ, no excess),
     then body 0.75, 0.5, 0.25, 0, −0.25: **9**; on head the fifth
     shot kills (**5**).
   - Jackal (shield 100 on the `shield` material only, leak 1
     elsewhere): plasma pistol (20, modifier 1 for type 16) on the
     `shield` material: 5 hits deplete 100, the body stays 1.0 forever;
     the same on `unarmored` (body 60): x = 0.33333334 per hit, β
     0.6666666, 0.33333328, −5.9604645e-08: **3** (with the lowest
     draw, 16, it takes 4).
   - Marine (shield 50 failing, threshold 1, failing leak 0.2; head
     leak 0.2): pistol on head at full shield: D_s = 0.8 × 25 × (0.8 ×
     1 + 0.2) = 20, T = 5 > 0 → headshot → **1**.
   - Hunter (body 200): pistol on skin (head): **1**; on armour
     (×0.2): 5/200 per hit → **41**; fuel rod: modifier 0 → never.
7. **Dialogue for a player-worn unit.** A unit without an actor whose
   dialogue fills pain body minor only: three non-lethal body hits 31
   ticks apart → three minor lines; a fourth within the count's decay
   → none (`sounds.md`).
8. **Permutation draw.** A move slot whose chain is A (cumulative
   weight 0.5, 24 frames of 0.0625) then B (1.0, 22 frames of 0.08):
   with random draws 0.3 then 0.7, the first loop is A (24 ticks of
   0.0625), the second B (22 of 0.08); a draw of exactly 0.5 picks A.
   Draws happen when the state is entered and each time the animation
   passes its last frame, never in between.
9. **Landings.** An Elite (soft 1 WU/s, hard 3 WU/s, soft time 0.6 s)
   holding forward lands from a flat-ground jump at λ = 0.069042 WU/t:
   soft, q = 0.5356, 9 recovery ticks in which the body moves only by
   the land-soft animation's root motion (take it as 0); then the move
   animation restarts at frame 0. A Grunt (soft 0.5, hard 1, hard time
   2 s) doing the same lands hard: q = 0.4143, 24 ticks, its velocity
   braking by 0.0053333 WU/t per tick from the take-off step.
10. **Crouch refused.** A graph whose `crouch` seat has no type for
    the held weapon's label: crouch held → the seat stays `stand`, the
    crouch fraction stays 0, the pill keeps its standing height.
11. **A push turns the body.** A living character facing +X with an
    incoming push of (0, 0.1, 0.045) WU/t: its velocity gains (0,
    0.05, 0.0225), it becomes airborne and slipping, and its facing
    becomes +Y (the push's direction made horizontal).
12. **Shield region swap.** A unit whose collision region r is flagged
    "missing when shield is zero" and has two permutations, with a node
    of region r holding two BSPs: shield above 0 → rays test BSP 0;
    shield depleted → BSP 1 (and the drawn permutation 1); recharge
    begins → BSP 0 again.
13. **Forced shader permutation.** A shader whose cube map bitmap has 4
    images and whose other bitmaps have 1: forced permutation 0 → the
    model's own reference (image 0 everywhere); 2 → cube image 2,
    other bitmaps image 0; 5 → cube image 1 (5 mod 4).

### (b) Against the disc

1. **Roster**: 42 distinct biped tags over the ten campaign maps;
   per map: a10 14, a30 14, a50 15, b30 11, b40 12, c10 18, c20 8, c40
   17, d20 15, d40 19. Actor variants per map: a10 16, a30 21, a50 24,
   b30 22, b40 31, c10 37, c20 24, c40 47, d20 40, d40 35.
2. **Heights and pills** as section 3's table: e.g. `characters\elite\
   elite` standing collision height 0.8, crouching 0.5, radius 0.2,
   camera 0.6 / 0.3, model top 0.786 (±0.001), `head` marker z 0.664;
   `characters\grunt\grunt` 0.55, 0.45, 0.2, 0.28 / 0.28, top 0.584,
   head 0.449; `characters\hunter\hunter` 0.8, 0.8, 0.24, top 0.969.
3. **Speeds**: mean dx per frame of `stand pistol move-front` in
   `characters\elite\elite` 0.0865 over 26 frames; Grunt's `stand
   pistol move-front%0` (the only animation of its chain, cumulative
   weight 1) 0.0628 over 16 frames, per frame between 0.054 and 0.069;
   Jackal 0.0458 over 24; Hunter `stand unarmed move-front` 0.0773 over
   26 and no stand move-back; Marine `stand rifle move-front` 0.0700
   over 20; the Flood combat Elite's `stand pistol move-front%0` and
   `%1` (24 frames, sum 1.5000; 22 frames, sum 1.7600; cumulative 0.5
   and 1). Every row of section 4's per-direction table (frames, loop
   sum, step range). Biped flags: Spartans 0x202, every character 0x200
   or 0.
4. **Turning and aim**: moving turning speed Elite 5.893977, Grunt
   3.1415927, Hunter 3.1415927, Marine 5.2359877; aiming velocity
   maximum Grunt 6.2831855, Jackal 9.424778, Hunter 3.1415927, Spartan
   0.
5. **Vitality**: collision models Elite 100 / 100, stun 12 s, recharge
   10 s, runtime recharge velocity 0.0033333334; Elite special stun 8
   s; Grunt 40 / 0; Jackal 60 / 100, stun 15, recharge 0, velocity 1;
   Hunter 200 / 0; Marine 50 / 50, failure threshold 1, linear, failing
   leak 0.2. Actor variants: elite minor plasma rifle 100 / 100, elite
   major 100 / 150, commander energy sword 100 / 300, specops 125 /
   200, grunt minor plasma pistol 30, jackal minor 50 / 200, hunter 225,
   hunter major 350, flood_infection 0.01.
6. **Animation labels**: the `stand` types listed in section 2's graph
   table, e.g. Elite pistol class {pr, hp, pp, ne}, sword {fb}; Grunt
   pistol {pr, ne, pp}, missle {fr}; Jackal pistol {pp, pr, ne}; Hunter
   unarmed {fr}; Marine rifle {ar, sg, sr, pr, ne}, pistol {hp}; Flood
   Elite pistol {ar, pr, hp, ne, pp, rl, ft, sg}. First person weapons
   block count 0 in every character graph. Weapon labels: as rule 3.
7. **Dialogue**: filled slots and permutation totals of section 6's
   table (Elite 53 / 271, Grunt 65 / 459, Jackal 63 / 577, Hunter 8 /
   90, Flood 18 / 136, Spartan 5 / 71); every line class "unit dialog"
   with distances 0 except the Monitor's (15, 30).
8. **Unit sizes**: the closure counts and totals of section 8's table,
   identical on every map holding the biped (e.g. Elite 161 tags, 6 682
   822 bytes on a10, a30, a50, b30, b40, c40 and d20).
9. **Damage modifiers**: Blood Gulch's `weapons\pistol\bullet` has
   modifier 1.0 for type 30, the campaign maps' 0.8; every other
   multiplayer weapon damage effect is identical on Blood Gulch and the
   campaign maps.
10. **IK points** (stand and crouch seats): none in any weapon class of
    the Grunt, Jackal, Hunter and both Flood combat graphs, and none in
    the Elite's pistol and sword classes; the Elite's rifle class has
    `left hand elite` → `left hand elite`; the Marine's rifle class
    `left hand` → `left hand marine`, its pistol class one point with
    two empty names.
11. **Aiming bounds** (each side, frame count × per-frame): every
    class named in item 10 that selects a weapon ±90° yaw (1 × 1.5708);
    pitch ±90° except the Hunter's
    unarmed class (down 1 × 0.7853982, up 1 × 1.5708), the Jackal's
    crouch pistol class (down 1 × 1.0471976) and the Marine's stand
    rifle class (down and up 1 × 1.0471976).
12. **Jackal shield geometry**: collision model `characters\jackal\
    jackal` region 3 `shield`, flags 0x10, permutations `base00` and
    `~shield_off`; node `frame shield` (region 3) BSPs of 54 surfaces
    (materials 2 and 3) and 6 surfaces (material 2 only); the render
    model's region 3 is `shield` with the same two permutations.
13. **Elite colours**: `characters\elite\bitmaps\cubemaps` has 4 cube
    images, mean colours about (9, 59, 142), (84, 16, 36), (156, 99,
    22), (81, 77, 98); the armour shaders' change colour source is A;
    the armour multipurpose maps' alpha is 255 everywhere; actor
    variant colours and forced permutations as in section 1; the
    `elite` tag's change colour A has no permutations, the `elite
    special` tag's one (grey 0.678 to 0.565).
14. **Landing, jump and slope fields**: jump velocity 0.07 (Elite,
    Grunt, Jackal, Hunter, Marine), 0.16 (both Flood combat forms);
    minimum soft / hard and maximum hard landing velocities and the
    landing times of section 4's second table; maximum slope 60° (cos
    0.49999997) for the Elite, Grunt, Jackal, Hunter and Flood combat
    Elite, 45° (0.70710677) for the Marine and the Flood combat human.

## Confidence per section

| Section | Confidence |
|---|---|
| Roster, maps, variants | checked against the disc (every tag on all ten maps) |
| Actor variant layout | offsets from the reference's definition, checked on the disc (references resolve, values consistent with the rank names) |
| Scenario palette and encounter offsets | from the reference, checked on the disc (palette indices resolve; counts plausible); placements are starting locations, not exact actor counts |
| Weapon selection rule | reference (`animations.md`), labels and slots checked on the disc |
| Eligibility rule | ours, from the data |
| Weapon attachment, melee by a player | reference only |
| Body dimensions, pills, hit materials | checked against the disc (bounds in the bind pose: inferred as "height") |
| Movement by root motion, aim limits, jumping, no air control | reference only (the numbers checked on the disc) |
| Root motion tick by tick, permutations, missing animations, stunned states | reference (the engine's unit and biped code traced for these cases); chains and weights checked on the disc; not observed in a running game |
| Landings, pushes, slopes, steps for characters | reference rules with the disc's per-biped values; jump and landing figures computed in single precision; step heights static bounds only |
| Turning and aiming | reference (the aim planner and the clip to the aiming bounds); limits and bounds checked on the disc; flick times computed from the plan's continuous profile |
| Left-hand IK | checked on the disc (IK point blocks read) with the reference's rule |
| Elite colours | reference (how a permutation picks a bitmap image; how actor colours apply) and disc (bitmap images, shader sources, mean colours of the decoded images); the look itself not compared with the game |
| Jackal shield when down | reference (one permutation index drives drawing and collision) and disc (the two BSPs measured) |
| Speeds and cycles | checked on the disc (frame info summed) |
| Vitality numbers | checked on the disc |
| Actor variant override rule | reference only |
| Jackal, Hunter, Marine mechanics | follow from the disc's materials and `health_shields.md`'s rules (rules: reference) |
| Hits to kill | computed by a scratch f32 simulation of `health_shields.md`, which reproduces its Spartan table; not observed in a running game |
| Dialogue contents | checked on the disc |
| Who speaks what | reference only |
| First person | checked on the disc (blocks empty, references null) |
| Unit closures and sizes | measured on the disc; the closure definition is ours; tag spans assume contiguous layout (the spans of all tags sum to 6.1 to 11.5 MB per map against 9 to 17 MB of tag space, the rest being the index, names and model buffers) |

## Open questions

1. **Missing movement animation.** Answered: section 4, "A missing
   animation" (the state is entered with no animation; the body stops
   and takes its default pose). What remains is ours: whether a
   player-worn Hunter or Flood should instead play another direction's
   animation, or none and keep still in the idle pose. Checkable in
   the game only by possessing a Hunter (a debug possession of a unit
   by bumping it exists in the engine).
2. **Firing origin of the Hunter's cannon** (invisible weapon attached
   at the Hunter's origin): does the projectile leave from the
   weapon's `primary trigger` (the weapon has no model, so no marker)
   or from the unit's own `primary trigger` marker? Not determined.
3. **Weapons without first-person models** (fuel rod, Hunter cannon,
   energy sword): what the game does for a player holding them (the
   Xbox campaign never lets the player pick them up) is unknown; ready,
   reload and melee timing come from first-person animations that do
   not exist.
4. **Elite rank colours.** Answered: section 1, "The Elite's
   colours" (permutation 0 to 3 = blue, red, gold, grey reflection;
   colour A tints the armour; the player's colour reaches a plain
   `elite`).
5. **Jackal shield collision when down.** Answered: section 2, rule 7
   (the disc's collision geometry is swapped for the emitter's; nothing
   is blocked).
6. **Melee damage of a player-worn body**: the player's melee uses the
   held weapon's melee damage effect; whether a character's own melee
   damage effect (Elite `characters\elite\elite melee`, 30 to 35;
   Hunter 35 to 45 with acceleration 9; Flood 12 to 16 and 20 to 30)
   should replace it is a design choice, not the game's.
7. **Splitting voices**: the Marine unit is 59.6 MB because it carries
   six voices; whether a "Marine" download should carry one voice is a
   product decision.
8. **Per-frame motion and permutations.** Answered: section 4, "Root
   motion, tick by tick" and "Per-direction numbers" (the Grunt's
   move-front has one permutation; only the Flood combat Elite's
   standing move-front has two).
9. **Left-hand IK.** Answered: section 2, rule 4 (no Covenant or Flood
   class a held weapon selects pins the off hand).
10. **Severe deaths**: for units without an actor the "severe" test
   practically never holds (`sounds.md`), so player-worn bodies almost
   always play death quiet; whether to prefer death violent for
   explosive kills is ours to decide.
11. **Body stun on player-worn characters** (section 4, A missing
   animation): the faithful result is a freeze in the default pose
   while stunned and pushing. Ours to decide: likely play the plain move
   animation with its step scaled by (1 − 0.8 s_b), and let the body
   turn.
12. **Step heights at speed.** The static bound (an edge no higher
   than r·(1 − cos θ_max) is ground) is exact; how much higher a
   character climbs at its walking speed, with its velocity reset to
   the root motion every grounded tick, was not simulated. A
   simulation with `movement.md`'s mover and the per-direction table's
   steps would answer it; the live game could check an Elite against
   a 0.15 WU kerb.
13. **Flood jumps and Grunt landings for players.** A player-worn
   Flood jumps 3.5 WU and hurts itself landing; a Grunt is stopped for
   24 ticks after each of its own jumps. Faithful; whether to keep them
   (or, for the Flood, aim the jump forward as the leap does) is ours.
14. **Player-worn walking speed and stick.** The game ignores the
   stick's magnitude and gives a player no walk; offering a walk (the
   AI's `alert` animations, Elite 0.0191 WU/t) or scaling by the stick
   is ours to decide.
15. **Hunter aim floor.** The Hunter's aim (and so its shots) cannot go
   below 45° under level; a player will notice. Faithful; ours to keep
   or widen.
