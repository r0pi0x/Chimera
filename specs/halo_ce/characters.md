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
  inside one 16-frame cycle. A Hunter and the Flood combat Elite have
  no backward or sideways walk at all.
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
  Hunter) apply to player-controlled units too.
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
| 0x20C | i16 | forced shader permutation index | when not 0, every shader of the model draws this permutation (the rank colour of Elites) |
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
   Elite's rank colour. Minor 0, major 1, commander 2, spec-ops 3,
   stealth 0 (the stealth Elite's look is its active camouflage, not a
   permutation). The colour each permutation index gives was not
   checked (Open questions).
3. **The actor variant's change colour** (0x22C, one entry): Grunt
   majors and spec-ops, Jackal minors and majors, the Elites, the
   crewmen (minors of Grunts have none).
4. **The actor variant's vitality** (0x200, 0x204): section 5.

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
   covenant` on the fuel rod gun only); which IK points each character
   graph's weapon classes hold was not read (Open questions).
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
   Its region is flagged "disappears when shield is off": while the
   shield vitality is 0 the region draws its second permutation (the
   shield gone), and it returns when the shield recharges.

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
   `stand`, or `crouch` while crouched.
3. **Velocity from root motion.** Each tick, if the unit is not
   airborne and not slipping, its horizontal velocity is set to the
   current frame's root motion (the animation's frame info dx forward,
   dy left, in WU per tick, in the body's facing frame), with no
   acceleration limit. The throttle's magnitude does not matter: any
   push past the dead zone gives the animation's full speed. If the
   frame info carries a yaw (turn animations) the body rotates by it.
4. **A missing direction does not move.** If the graph has no
   animation for the wanted state, the unit cannot enter it and stays
   in its current state (Open questions: the exact fallback); in
   practice a Hunter standing still and pushed back stays idle, and one
   walking forward and pushed back keeps walking forward until the
   stick returns. The Flood combat Elite and the carrier have only
   move-front in every seat; the Flood human has front and back.
5. **The infection-form speed boost.** Bipeds with the "random speed
   increase" flag (bit 11; none of the candidates) scale root motion by
   1 + d × (k mod 137) × 0.00729927 where k is the object's index and d
   the difficulty's infection-form value; not for player-worn bodies we
   offer.
6. **Facing.** While moving the body yaw turns toward the aim yaw by at
   most the biped's moving turning speed / 30 per tick (`movement.md`
   section 1); motion is along the body's facing, so a slow-turning
   body walks where it faces, not where the player aims. Idle, the
   turn-left and turn-right animations (90° over 14 frames on every
   character) turn the body once the aim is more than the stationary
   turning threshold away.
7. **Aim lag.** The unit's aiming velocity and acceleration limits
   (`objects.md`, unit layer 0x264 and 0x268) move the unit's aim
   toward the wanted aim by at most those rates **for every unit,
   player-controlled included**. The Spartan's are 0 (instant). The
   looking limits (0x270, 0x274) do the same to the head's look.
8. **Jumping.** Any biped jumps when grounded and not hard-landing:
   the vertical velocity becomes at least the biped's jump velocity
   (scaled for a player by 1 − body stun × 0.8). Gravity is
   `movement.md`'s.
9. **No air control.** Only the player-physics branch sets an airborne
   acceleration; a character in the air keeps its velocity until it
   lands.
10. **Landing.** The landing velocities and times of the biped tag
    (`movement.md` section 6) apply as written; while hard-landing
    (`landing` state 1) the unit does not move.
11. **Crouching** works as `movement.md` section 4 with the biped's
    own heights and transition time (0.2 s on all).
12. **Walking slopes** use the biped's own maximum slope and up- and
    downhill scales: 60° for Covenant and Flood (uphill 30° to 60°,
    scale 0.5; downhill 15° to 45°, scale 2), 45° for humans (the
    Spartan's values).

### The numbers

Speeds are the cycle's mean root motion; the per-frame range is given
where it varies by more than 5 %. Standing class is the one the
character's default weapon uses.

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
| Flood combat Elite | 0.0625, 1.875, 5.715 | none | none | 0.0181 (0.014 to 0.022) front only | 24; crouch 36 |
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
   the shield but never hurts the body, even after the shield is down.
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
3. The biped picks its wanted movement state from the throttle
   (section 4 rule 2) and, if not airborne or slipping, sets its
   horizontal velocity to the current animation frame's root motion
   (rule 3), applying the frame's yaw to the body.
4. The physics move (`movement.md` section 5) runs with an unlimited
   acceleration and no airborne acceleration.
5. The animation advances one frame (`animations.md`, Playback).
6. Damage, shield recharge, dialogue (`health_shields.md`,
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
- **The Hunter pushed backward while standing** does not move; crouched
  it walks in all four directions at a quarter speed.
- **The Grunt's crouched back-step** is the standing back-step
  animation (0.0325 WU/t): crouched, a Grunt backs away faster than it
  walks forward (0.0087).
- **A destroyed right arm** on a Flood combat form drops its weapon and
  inhibits firing; a player-worn Flood body loses its gun for the rest
  of its life.
- **The Jackal's shield at zero** still blocks bodily harm on hits to
  the `shield` material (its body multiplier is 0), while the region
  is drawn in its `~shield_off` permutation; see Open questions for
  whether its collision geometry remains.
- **Infection forms** have body 80 in the collision model but every
  actor variant sets 0.01: as placed in the game they die to anything.
- **The Monitor** has 10¹⁰ shield points and no body: unkillable.

## Quirks

- **Animation-driven speeds are not the globals'**: the Elite runs
  faster than the Spartan (2.595 WU/s against 2.25) and the Flood
  flee 60 % faster; faithful player-worn bodies will feel unfair.
  This is intended by the brief (tune later).
- **Throttle magnitude is ignored** by animation-driven bodies: a half
  push walks at full speed.
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
   0.06, 0.07, 0.06 and no move-back: throttle (1, 0) → velocities
   0.05, 0.06, 0.07, 0.06, 0.05 ... WU/t forward; throttle (0.3, 0) →
   the same; throttle (0.5, 0.6) → left (no animation: no motion);
   throttle (−1, 0) → back (no animation: no motion).
4. **No air control.** That biped airborne with horizontal velocity
   (0.05, 0) and throttle (0, 1) keeps (0.05, 0) until it lands.
5. **Aim limit.** A unit with aiming velocity maximum 2π rad/s and
   acceleration maximum 4π rad/s², aim at rest, wanted aim 90° away:
   the per-tick limits are 0.20944 rad/t and 0.013963 rad/t²; the aim
   speeds up by at most the acceleration limit per tick, never exceeds
   the velocity limit, and arrives after at least 15 ticks (√(2 ×
   1.5708 / 0.013963) = 15.0, accelerating all the way), never in one
   tick as the Spartan's does.
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
   pistol move-front` (first permutation) 0.0628 over 16 frames, per
   frame between 0.054 and 0.069; Jackal 0.0458 over 24; Hunter `stand
   unarmed move-front` 0.0773 over 26 and no stand move-back; Marine
   `stand rifle move-front` 0.0700 over 20. Biped flags: Spartans
   0x202, every character 0x200 or 0.
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

1. **Missing movement animation.** The exact behaviour when the wanted
   state has no animation (stay in the current state, or idle?) was not
   traced through the state machine; test with a Hunter.
2. **Firing origin of the Hunter's cannon** (invisible weapon attached
   at the Hunter's origin): does the projectile leave from the
   weapon's `primary trigger` (the weapon has no model, so no marker)
   or from the unit's own `primary trigger` marker? Not determined.
3. **Weapons without first-person models** (fuel rod, Hunter cannon,
   energy sword): what the game does for a player holding them (the
   Xbox campaign never lets the player pick them up) is unknown; ready,
   reload and melee timing come from first-person animations that do
   not exist.
4. **Elite rank colours**: which colour each forced shader permutation
   (0 to 3) shows, and the change colours of the actor variants, were
   not read.
5. **Jackal shield collision when down**: whether the collision
   model's `shield` region stops blocking hits while its render
   permutation is `~shield_off` (does the collision model's region have
   a matching permutation?) was not checked.
6. **Melee damage of a player-worn body**: the player's melee uses the
   held weapon's melee damage effect; whether a character's own melee
   damage effect (Elite `characters\elite\elite melee`, 30 to 35;
   Hunter 35 to 45 with acceleration 9; Flood 12 to 16 and 20 to 30)
   should replace it is a design choice, not the game's.
7. **Splitting voices**: the Marine unit is 59.6 MB because it carries
   six voices; whether a "Marine" download should carry one voice is a
   product decision.
8. **Mean speeds** hide per-frame variation; a faithful port must play
   the per-frame root motion, not the mean. The Grunt's move-front has
   permutations (`%0` names a chain); which permutation plays when was
   not measured here (`animations.md`, permutations).
9. **Left-hand IK**: the weapon classes' IK point blocks of the
   character graphs were not read; whether a character's off hand is
   pinned to the weapon (as the Spartan's is) is unknown.
10. **Severe deaths**: for units without an actor the "severe" test
   practically never holds (`sounds.md`), so player-worn bodies almost
   always play death quiet; whether to prefer death violent for
   explosive kills is ours to decide.
