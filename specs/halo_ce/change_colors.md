# Halo: Combat Evolved: change colours on characters

Source basis: the disc's object, unit and actor variant tags and model
shaders as read by earlier spec sessions (the values below are the ones
recorded in [objects.md](objects.md), [characters.md](characters.md)
and [shaders.md](shaders.md); this file gathers them per character and
states the whole rule in one place), and the decompiled change-colour
behaviour those specs describe, used for meaning only. The session that
wrote this file could not open the disc image (no Halo install was
configured for it), so every value not already measured is listed in
"Open questions" with how to measure it.
Status: draft

## Summary

A Halo model can be tinted per object through four colour slots, A to
D (the object's *change colours*). A model shader names at most one
slot as its change-colour source and applies it only where its
multipurpose map's change-colour mask is set: there the base texture
is multiplied by the slot's colour, elsewhere it is left alone. The
slots are filled once, when the object is created, from (in order of
precedence) the actor variant's rank colour, the tag's own colour
permutations, and the colour the creator supplied (white for a
placement, the player's colour for a multiplayer player's unit).
**A campaign character's tint varies only slightly between
individuals of one rank** (the two bounds of each range are close
shades of one colour: orange Grunts, red Grunt majors, near-black
spec-ops, blue-grey Jackals, and so on). A model that shows many
unrelated colours, or a patchwork of tinted and untinted parts in
colours the rank does not have, is not the original's look.

## Units and conventions

Colours are linear factors 0..1 per channel as stored in the tags
(the tags' RGB values are applied to the texture as they are, with no
gamma conversion in the shader). Slots are indexed 0..3 = A..D. HSV
components are each 0..1 (hue 1 = a full turn). Tag offsets are as in
[cache_file.md](cache_file.md).

## Constants

| Name (ours) | Value | Unit | Meaning |
|---|---|---|---|
| CHANGE_COLOUR_SLOTS | 4 | slots | A, B, C, D per object |
| TAG_CHANGE_COLOURS_MAX | 4 | entries | an object tag's change colour block is read up to 4 entries (more are ignored) |
| ACTOR_CHANGE_COLOURS | 0 or 1 | entries | the actor variant's change colour block size on the disc (only slot A is ever overridden) |
| ACTOR_CC_ELEMENT | 0x20 | bytes | actor variant change colour element: lower RGB, upper RGB, 8 unused bytes |
| PLACEMENT_DEFAULT | (1, 1, 1) | rgb | a slot nobody supplied |
| PERM_HASH | 315.89313, 587.12946, 744.12415, 431.12894 | | the permutation pick's position and slot weights (objects.md) |
| PERM_BLEND_SLOT | 0.71211 | | the permutation blend factor's per-slot offset (objects.md) |

## Behaviour

### 1. The colour a slot starts with (object creation)

For each slot i in 0..3, in this order, each step overriding the
previous one where it applies:

1. **The creator's colour.** A scenario placement supplies four colours;
   on the disc they are always zero bytes and the engine treats a
   placement without colours as white (1, 1, 1) in every slot. A
   multiplayer player's unit is created with the player's colour in all
   four slots (section 4).
2. **The tag's permutations.** If the object tag has a change colour i
   and it has permutations, one permutation is picked from the object's
   creation position (a fixed pick per place, not random per spawn:
   objects.md, Change colours, step 2) and slot i becomes the HSV blend
   of that permutation's lower and upper bound at a factor that also
   depends only on the position. A tag change colour **without**
   permutations does nothing here: the slot keeps the creator's colour.
3. **The tag's functions.** A tag change colour with a "scale by"
   function input recomputes the slot every tick from its lower and
   upper bound (objects.md); "darken by" multiplies it. On the
   characters this is used only for glows (the multiplayer Spartan's
   slot A, the shield colour), never for an armour colour.
4. **The actor variant (campaign AI only).** When an actor is created
   for the unit, for each entry i of its actor variant's change colour
   block (at most 4; one on the disc), slot i becomes the HSV blend of
   the entry's lower and upper bound at a factor f drawn once from the
   game-state random generator, uniform in [0, 1). This **is** random
   per spawn, so two Grunt majors differ slightly; it replaces whatever
   steps 1 and 2 chose. A unit created without an actor (a player's
   unit, a cinematic one) skips this step.

After the slot is set it is clamped to 0..1 per channel; it does not
change again for the object's life unless step 3 applies.

### 2. The HSV blend

blend(lo, hi, f), used by steps 2 and 4:

1. Convert lo and hi to HSV (h, s, v), the usual hexcone conversion
   (v = max channel; s = (max − min)/max, 0 when max = 0; h from the
   largest channel, 0 for a grey).
2. Hue path: unless the long-way flag is set (actor variant entries
   have no flags: always the short way), when |h_hi − h_lo| > 0.5 add 1
   to the smaller hue.
3. h = h_lo + (h_hi − h_lo)·f, likewise s and v; if h ≥ 1 subtract 1.
4. Convert back to RGB.

With f = 0 the result is lo, with f = 1 hi (to rounding of the round
trip).

### 3. How a model shader applies a slot

A model shader (`soso`, shaders.md "Model") has a change-colour source
(0 none, 1 A, 2 B, 3 C, 4 D) and a multipurpose map whose **alpha**
channel is the change-colour mask on the Xbox (shaders.md: red
reflection mask, green self-illumination mask, alpha change-colour
mask; the PC release orders the channels differently, and the disc's
maps are authored for the Xbox order).

Per pixel, with c the source slot's colour (white when the source is
none, or when the shader has no multipurpose map, which then means
mask 1 for the whole surface; see Open question 4):

  tint = (1 − m)·(1, 1, 1) + m·c,  m = multipurpose alpha

and the tint multiplies the diffuse colour (base map × detail, times
lighting) before the reflection is added (shaders.md, "What it draws").
The reflection and self-illumination are not tinted. A mask of 0 shows
the base texture's own colour; a mask of 1 shows base × c.

The model draws each of its parts with that part's shader, so on one
character different parts can read different slots or none: a part
whose shader has source none is never tinted.

### 4. Multiplayer Spartans: player colour to slots

The multiplayer Spartan (`characters\cyborg_mp\cyborg_mp`) is created
with one colour in all four slots (objects.md, "Spartan multiplayer
colour"): team games red (1, 0, 0) for team 0 and blue (0, 0, 1) for
every other team; free for all, the player's profile colour from the
18-entry table. Its tag's change colours are A (the shield glow, a
scale-by function) and B (white), neither with permutations; **its
armour, visor and first-person hands read slot C**, which keeps the
player's colour. Slot D also keeps it but nothing reads it. The
campaign Spartan (`characters\cyborg\cyborg`) has a third tag change
colour, C, with one permutation (weight 1, colour (0.341, 0.404,
0.145)), so its armour is always olive green whatever it is created
with.

### 5. Per character

Slot A is the only armour slot on every Covenant character; the tag
column is the biped tag's own change colour A, the actor column the
actor variant override (section 1 step 4). Values are (r, g, b) lower
→ upper.

| Character, rank | Biped tag | Tag's slot A | Actor variant slot A | Look |
|---|---|---|---|---|
| Grunt minor | `grunt` | orange (0.863, 0.502, 0.173) → (0.824, 0.443, 0.157) | none (keeps the tag's) | orange |
| Grunt major | `grunt` | as minor | (0.824, 0.075, 0.075) → (0.643, 0.090, 0.090); the needler major's upper bound (0.600, 0.082, 0.082) | red |
| Grunt spec-ops | `grunt specops` | as minor (to be confirmed: Open question 1) | (0.071, 0.071, 0.071) → (0, 0, 0) | near black |
| Jackal minor | `jackal` | blue grey (0.259, 0.373, 0.490) → (0.267, 0.353, 0.482) | the same as the tag's | blue grey |
| Jackal major | `jackal major` | as minor | pink (0.675, 0.459, 0.459) → (0.710, 0.435, 0.443) | pink / mauve |
| Elite minor | `elite` | no permutations: keeps the creator's colour | (0.314, 0.302, 0.533) (one value) | blue violet, blue sheen |
| Elite major | `elite` | as minor | (0.251, 0.082, 0.086) | dark red, red sheen |
| Elite commander | `elite special` | one permutation, grey (0.565 … 0.678) | (0.294, 0.239, 0.055) | dark gold, gold sheen |
| Elite spec-ops | `elite special` | as commander | (0, 0, 0) | black, grey sheen |
| Elite stealth | `elite special` | as commander | none | the tag's grey (seen through camouflage) |
| Hunter | `hunter` | no change colours | none | untinted |
| Marines, crewmen, Flood | (see characters.md) | not recorded (Open question 2) | crewmen have one entry (values not recorded) | |

The Elite's rank is also its forced shader permutation (the reflection
cube map's colour: characters.md, "The Elite's colours"); the other
characters have none.

**Where the Grunt minor's orange comes from** depends on whether the
`grunt` tag's slot A bounds are its base range or a single permutation
(Open question 1). If they are a permutation (as on `elite special`),
every Grunt without an actor override is orange whatever colour it is
created with, a player-worn Grunt included. If they are the base range
without permutations (as on `elite`), a Grunt created without an actor
override takes its creator's colour: white for a scenario placement,
the player's colour for a player. The campaign never shows the latter
case, because every campaign Grunt has an actor.

### 6. Playable campaign characters (ours)

The original never lets a player wear a campaign character, so a rule
for it is ours. For parity with how the characters look in the
campaign:

- By default a playable Grunt, Jackal or Elite wears its rank's colour
  from the table in section 5 (the lower bound, or a blend at a random
  factor per spawn, like an actor). It does **not** take the
  free-for-all profile colours on the armour: those 18 colours (pink,
  cyan, lime, and so on) never appear on a campaign character and make
  the body look painted.
- A team tint, when wanted, is an opt-in: for the Elite, colour A by
  team as characters.md says; for the others, pick a rank per team
  (Grunt: minor orange vs major red; Jackal: minor blue grey vs major
  pink) instead of overwriting slot A with pure red or blue.
- Parts whose shaders read no slot, or read a slot the character's
  rank does not set, keep the colour the original would give them
  (white or the tag's permutation colour), never the player's colour.

## Per-tick order

Slots are set at creation in the order of section 1 (creator, tag
permutations, tag functions once, actor variant when the actor is
attached). Each tick thereafter, only slots with a "scale by" function
are recomputed, after the object's function update (objects.md,
Per-tick order). Rendering reads the values the last tick stored.

## Edge cases

- A shader whose source names a slot the tag has no change colour for
  reads the creator's colour (white for a placement): such a part is
  untinted in the campaign and takes the player's colour on a
  multiplayer unit.
- The actor variant override happens when the actor attaches, which is
  at creation for every disc encounter; a unit that gets an actor later
  (none on the disc) would change colour then.
- An actor variant entry whose two bounds are equal (Elite minor) gives
  the same colour on every spawn.
- Black spec-ops colours with a fully set mask make the tinted parts
  black apart from their reflection: that is the original's look, not
  a missing texture.

## Quirks

- The tag permutation pick depends on the creation position, so a
  given placement always gets the same variation; respawning at the
  same point repeats it.
- The actor override ignores the tag's permutations entirely, so two
  biped tags that differ only in their tag colour (none on the disc)
  look the same under one actor variant.

## Test cases

| Setup | Input | After | Expected |
|---|---|---|---|
| HSV blend | lo = (0.824, 0.075, 0.075), hi = (0.643, 0.090, 0.090), f = 0 | blend | (0.824, 0.075, 0.075) within 1e-4 |
| HSV blend | same, f = 1 | blend | (0.643, 0.090, 0.090) within 1e-4 |
| HSV blend, hue short way | lo pure red (1, 0, 0) h = 0, hi (1, 0, 0.2) h ≈ 0.967, f = 0.5 | blend | hue ≈ 0.983 (not 0.483): a red-magenta, never green |
| Actor override | Grunt major spawned with any random factor | creation | slot A's red channel in [0.643, 0.824], green and blue in [0.075, 0.090] |
| Actor override replaces tag | Grunt created by a Grunt-major actor at any position | creation | slot A comes from the actor entry, not the tag's orange |
| No actor entry | Grunt minor actor | creation | slot A is the tag's (orange range), independent of the random generator |
| Spec-ops | Grunt spec-ops | creation | slot A every channel in [0, 0.071] |
| Elite minor | any spawn | creation | slot A = (0.314, 0.302, 0.533) exactly; forced permutation 0 |
| Shader mask | model shader source A, multipurpose alpha 0 at a pixel, slot A red | draw | the pixel's diffuse is the base texture's (untinted) |
| Shader mask | same, alpha 1 | draw | diffuse = base × (red) |
| Shader no source | source none, any slot values | draw | untinted everywhere |
| MP Spartan FFA | player colour index 7 (FF56B9) | spawn | slots A..D created (1, 0.337, 0.725); A then follows its shield function; B, C and D keep (1, 0.337, 0.725) (B has neither permutations nor a function); the armour (source C) shows (1, 0.337, 0.725) |
| MP Spartan team | team 1 | spawn | armour (slot C) (0, 0, 1) |
| Campaign Spartan | created with any colour | creation | slot C = (0.341, 0.404, 0.145) |
| Playable Grunt, FFA (ours) | player with profile colour cyan | spawn | armour orange (rank colour), not cyan |

## Open questions

1. **Grunt tag's slot A: base range or permutation?** characters.md
   records the `grunt` tag's A as an orange range but not whether it is
   the base bounds or a permutation. Measure: read the `grunt` and
   `grunt specops` bipeds' change colour blocks (count, each entry's
   permutation count and weights). Same for `jackal` and `jackal
   major`.
2. **Marines, crewmen and the Flood.** Their biped tags' change
   colours (count, bounds, permutations, scale/darken inputs), their
   actor variants' entries (the crewmen's values), and which slot each
   of their model shaders reads. The Marine's skin tones and the Flood's
   colours may come from slots B to D or from model permutations alone.
3. **Which Grunt, Jackal, Hunter, Marine and Flood shaders read which
   slot.** For every model shader of each character model: the change
   colour source and the multipurpose map's alpha coverage (mean alpha,
   share of texels at or above 0.5, share at 0). Only the Elite's is
   measured (source A, alpha 255 everywhere). A Grunt whose armour mask
   covers only the armour plates and backpack would show orange plates
   over an untinted grey-brown body; that is the expected two-tone
   look, not a bug.
4. **A model shader with a change-colour source but no multipurpose
   map**: whether it tints the whole surface (mask 1) or nothing
   (mask 0). shaders.md's survey suggests 57 disc tags set a source;
   count how many of them lack a multipurpose map.
5. **The actor variant's random factor**: whether one factor is drawn
   per entry or one per actor, and whether it is drawn from the
   deterministic game-state generator (it is said to be; the draw's
   order relative to other creation draws matters only for replays).
6. **A live capture** of a Grunt minor, major and spec-ops side by side
   in the original (e.g. on c40 or d20) to confirm the look and the
   mask coverage, and of our build's campaign Grunt for comparison.

How to measure 1 to 4: a scratch reader over the ten campaign maps
using our `blam_formats` crate (object base change colours, the actor
variant's block at 0x22C, every model shader's source and its
multipurpose bitmap's alpha), run where the disc image is configured.
