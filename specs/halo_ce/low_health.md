# Halo: Combat Evolved: low-shield, low-health and damage cues for the local player

Source basis: an index over this folder's specs, whose numbers were read
from the original Xbox disc's `bloodgulch.map` (the Spartan's unit HUD
interfaces `ui\hud\cyborg` and `ui\hud\cyborg_mp`, the HUD globals
`ui\hud\default`, the collision model `characters\cyborg\cyborg`, the
unit dialogue, the damage effects) and from the engine decompilation
described in `hud.md`, used for meaning only. This session could not
open the disc image (no image path in its environment, and the local
config that names it is off-limits to spec sessions), so no new tag was
read; values not already on record are Open questions.
Status: draft

## Summary

Halo tells a hurt player mostly through **sound**: the unit HUD has a
small set of looping warning sounds keyed to the shield and body
state (shield hit, shield charging, shield low, shield empty, and a
heartbeat when the body is low), and the HUD's shield bar flashes once
it falls under a quarter. With the shield gone the health plate
flashes, and the health segments turn yellow and then red. Hits show a
red wedge at the screen edge pointing at the attacker for one second.
The Spartan has no pain voice in multiplayer. Of these we draw the
bars and their flashing; **none of the warning sounds play and the
damage arrows are not drawn**, which is most of what makes low shields
and health obvious in the original.

## Units and conventions

Halo ticks (30 per second); vitality as fractions (shield s: 0..1, up to
3 with an overshield; body b: 0..1); colours 0xAARRGGBB; HUD pixels on
the 640 × 480 canvas (`hud.md` 1).

## Constants

| Name (ours) | Value | Unit | Meaning | Where specified |
|---|---|---|---|---|
| SHIELD_FLASH_BELOW | 0.25 | fraction | the shield plate flashes while 0 ≤ s < 0.25 and alive | hud.md 6 |
| SHIELD_LOW | 0 < s < 0.25 | | `shield_low` loop | hud.md 6 |
| SHIELD_EMPTY | s = 0 | | `shield_depleted` loop | hud.md 6 |
| HEALTH_LOW | b < 0.25 | | `health_low_heart` loop | hud.md 6 |
| HEALTH_CUTOFFS | 0.67 / 0.33 | fraction | health segments blue ≥ 0.67, yellow between, red ≤ 0.33 | hud.md 6 |
| SHIELD_HIT_HOLD | 15 | ticks | the lost part of the shield bar stays marked white | hud.md 5 |
| SHIELD_DELAY | 180 | ticks | 6.0 s after a hit before recharge | health_shields.md |
| RECHARGE | 121 | ticks | empty to full (4.0 s) | health_shields.md |
| ARROW_LIFE | 1–29 | ticks | a damage arrow draws while its counter is in this range (just under 1 s) | hud.md 12 |
| ARROW_COLOUR | 0xC8FF0000 | | red, alpha 200, no fade | hud.md 12 |

## Behavior

### 1. Shield bar (specified: hud.md 4, 5, 6)

- The bar fills with s (alpha trunc(198 s) + 55 through the meter
  shader); colours 0x0000509F → 0x002896FF (blue).
- **Hit mark.** On a loss, the lost stretch of the bar is painted white
  (0x00FFFFFF at the bar's max alpha) for 15 ticks from the first loss
  of the episode, then disappears.
- **Low flash.** While alive and s < 0.25 the shield **plate**
  (background, 0xDCAAD5FF) flashes toward 0xFFFF552B (orange-red):
  period 1.35 s, three pulses of 0.225 s spaced 0.225 s apart: the
  plate pulses continuously (1.35 = 3 × 0.45). It starts its phase when
  the condition starts.
- Recharge has no animation: the bar grows, with the charge loop below.
- Overshield stages draw red (1–2) and green (2–3) bars over the blue.

### 2. Health segments (specified: hud.md 6)

- Eight segments (value scale 8; fill alpha max(31, 30·⌊8b⌋ + 15)).
- Colour by b: ≥ 0.67 blue 0x2896FF, ≤ 0.33 red 0xFF0000, between
  yellow 0xFFDF00, solid.
- **Shield-down flash.** While the unit's shield is marked depleted
  (broken and not yet recharging, `health_shields.md`), the health
  **plate** flashes toward 0xFFFF552B: period 1.0 s, one pulse of
  1.0 s (a continuous half-sine glow, once a second). The segments
  keep their colour by b (hud.md 6: the plate flashes, not the meter).

### 3. Warning sounds (specified: hud.md 6 "Warning sounds"; tags on the disc)

A state word is rebuilt every frame from the local unit:

| Bit | Condition | Element on the Spartan's HUD | Kind |
|---|---|---|---|
| 0 | the shield is recharging | `sound\sfx\ui\shield_charge` | looping (`lsnd`): plays while charging |
| 1 | the shield fell (the 15-tick hit hold of section 1 is running) | `sound\sfx\ui\shield_hit` | one-shot (`snd!`), latched: once per hit episode |
| 2 | 0 < s < 0.25 | `sound\sfx\ui\shield_low` | looping: the low-shield alarm |
| 3 | s = 0 | `sound\sfx\ui\shield_depleted` | looping: the empty-shield alarm |
| 4 | b < 0.25 | `sound\sfx\ui\health_low_heart` | looping: a heartbeat |
| 5–7 | dead / body fell < 0.1875 / ≥ 0.1875 since last frame | (no element on the Spartan) | – |

Rule each frame: an element whose bits meet the word starts (a loop if
not already playing; a one-shot only if its latch is clear, then
latched); an element whose bits do not meet it stops its loop and
clears its latch. So, a typical fight: a hit plays `shield_hit`;
under a quarter the `shield_low` alarm loops; at zero it switches to
`shield_depleted` (the low loop stops because bit 2 needs s > 0); with
the body under a quarter the heartbeat loops on top; six seconds after
the last hit the charge loop runs while the bar refills and the alarms
stop as s passes 0 and then 0.25. All are unspatialised interface
sounds at scale 0. No sound plays while dead (and the memory clears),
or when the HUD is hidden by script/cinematic; the shield bits need a
game type with shields.

### 4. Damage direction arrows (specified: hud.md 12, globals hud.md "HUD globals")

On damage from a source object the matching edge counters (top, left,
bottom, right) restart; each draws the `hud_damage_arrows` wedge
(231 × 40; split screen 165 × 29) centred 10 pixels in from that edge,
pointing outward, red 0xC8FF0000, for counter values 1–29 (about 1 s),
no fade. Up/down when |up| > 0.5 of the direction; left/right by the
reference's literal angle test (hud.md Open question 6: a hole in
front, everything behind goes left or right).

### 5. Screen flash, shake and impulse from the hit (specified in part: effects.md "Damage effect presentation")

The damage effect (`jpt!`) of whatever hit the player carries a screen
flash (type, priority, duration, fade function, maximum intensity,
colour), a temporary and a permanent camera impulse and a shake; the
rules for applying them per local player are in effects.md. The
**victim-side values per weapon** (bullets, plasma, needles, melee,
grenade and rocket blasts, falling) are not on record (Open question
1). On record: the frag grenade's shock wave shakes nearby players (1.5
s, late falloff, 0.1 WU translation, wobble 0.4); the frag and tank
explosions turn the aim by 0.0698 rad (permanent impulse); the
overshield pickup flashes the screen white for 2 s at half intensity;
the overcharged plasma pistol's firing flashes its *shooter* green.

### 6. Seen by others (not a self-cue, listed for completeness)

- The shield breaking plays `characters\cyborg\cyborg shield depletion`
  on the biped (particles only: an expanding flare and sparks;
  effects.md).
- The shield's glow over the body when hit (the model's shield shader,
  driven by recent shield damage; objects.md, health_shields.md).

### 7. What Halo does not have

No pain, scream or "maimed" voice for the Spartan in multiplayer
(sounds.md "unit dialogue"); no red vignette, blur or desaturation for
low health; no screen flash from ordinary damage unless the damage
effect itself carries one (Open question 1).

## Per-tick order

Damage is applied in the update of whatever caused it
(health_shields.md); the HUD's shield bookkeeping (hud.md 5) then runs
once per tick per local player; the HUD draws and rebuilds the sound
state word every frame (hud.md "Per-tick order").

## Edge cases

- Overshield draining above 1.0 never marks a hit (the HUD's memory is
  lowered with it) and plays no `shield_hit`.
- s exactly 0.25: no low flash, no low alarm (strict).
- Body exactly 0.25: no heartbeat (strict).
- A player in a vehicle: the plate flash start follows only the unit
  the player controls (hud.md 6).

## Test cases

| Setup | Input | After | Expected |
|---|---|---|---|
| s 1.0, b 1.0 | hit to s 0.6 | same frame | `shield_hit` once; bar's 0.6–1.0 stretch white for 15 ticks; no loop |
| s 0.6 | hit to s 0.2 | – | `shield_hit` (new episode), `shield_low` loop starts; shield plate flashing (1.35 s cycle) |
| s 0.2, `shield_low` looping | hit to s 0 | – | `shield_low` stops, `shield_depleted` loop; health plate flashing once a second |
| s 0, b 0.3 | hit to b 0.2 | – | heartbeat loop added; segments red (0.2 ≤ 0.33) |
| s 0, b 0.2, no further hits | 180 ticks | – | `shield_charge` loop starts; `shield_depleted` stops when s > 0; `shield_low` loops until s ≥ 0.25 then stops; heartbeat continues (body does not regenerate in Halo CE) |
| alive, any loop playing | die | – | every loop stops; nothing plays while dead |
| hit from the left | – | ticks 1–29 / 30 | left arrow at (58, 240) / gone |

## Implemented in Chimera? (checked 2026-11-21 against games/halo_ce)

| Cue | Implemented |
|---|---|
| Shield bar, hit mark (15-tick white), overshield stages | yes (`games/halo_ce/src/ui/hud/rules.rs`, `draw.rs`) |
| Shield plate flash below 0.25 | yes (`draw.rs`) |
| Health segments' colours and the plate flash while the shield is depleted | yes (`draw.rs`, `nodes.rs` reading the vitality's depleted flag) |
| Warning sounds (`shield_hit`, `shield_charge`, `shield_low`, `shield_depleted`, `health_low_heart`) | **no** ("the warning sounds do not play", `ui/hud/draw.rs` Known shortcuts) |
| Damage direction arrows | **no** ("damage arrows ... are not drawn", same) |
| Victim screen flash / camera impulse / shake from a hit's damage effect | **no** (only the shooter's firing kick shake is read, `game/fx/load.rs`) |
| Shield depletion effect, shield glow on the body | no (seen by others only) |

In the party apps Halo's HUD (and so the bars and flashes) shows only
while a Halo gun is the HUD's owner; the sounds and arrows, once
implemented, are the unit's state and could play whatever HUD shows.

## Open questions

1. **Victim-side damage effects.** The screen flash (type, colour,
   duration, intensity), impulse and shake of each multiplayer weapon's
   hit damage effect (assault rifle, pistol, shotgun, sniper, rocket and
   its explosion, plasma pistol/rifle bolts and overcharge, needler and
   supercombine, frag and plasma grenade explosions, melee, falling,
   Warthog/Scorpion/Banshee/Ghost weapons). Read every `jpt!` on
   `bloodgulch.map` at offsets 0x024–0x05C and 0x098–0x0F4
   (projectiles.md layout) and list them; effects.md says they are "in
   the tags" but does not list them.
2. **The five warning sound tags.** Each one's class, permutations,
   gain, and for the loops their tracks (start, loop, end), so the
   heartbeat's and alarms' rhythm is known. Read on the disc (sounds.md
   layout).
3. Whether the HUD's "body fell" bits 6 and 7 have elements in any other
   unit HUD (none on the Spartan's).
4. Whether a networked client in the original sees the same alarms
   (they follow the local unit's replicated shield and body values;
   believed yes).
