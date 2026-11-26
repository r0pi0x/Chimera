# Counter-Strike 1.6: water, slime and lava

Source basis: the community reimplementation of the CS 1.6 game library
(ReGameDLL_CS: the player's per-command water handling, drowning and
time-based recovery, the shared movement's water code; its
original-game behaviour only, never its optional fixes), the community
reimplementation of the CS 1.6 client library (cs16-client: the view's
water offset, its input code), Valve's Half-Life SDK for sound
constants; read in a separate spec session as reference only. Checked
against the install: the liquid textures' palettes in the client's WADs
(`~/games/cs16/{cstrike,valve}/*.wad`, read in place), the client's
shipped `config.cfg`, the sound files present, and the engine's
console variable names. Swimming itself (speeds, water jump) is in
[movement.md](movement.md) "Water". Numbers marked "(ref.)" were not
measured in the running game. No code is reproduced here.
Status: draft

## Summary

A player is in a liquid by point contents at three heights (water
level 1-3; movement.md). The three liquids swim the same; they differ
in how fast jump lifts you (100, 80, 50) and in damage: slime and lava
hurt on every command, lava 2.5 times as hard, both enough to kill in
well under a second. With the head under for 12 s a player starts to
drown (3, 4, then 5 a second) and gets it all back, 10 every 2 s, once
breathing. Under the surface the view is fogged in the liquid's colour
and sounds get the engine's underwater effect.

## Units and conventions

As in [movement.md](movement.md). The player's per-command game logic
(before the movement of the same command) is called "the player
update" here; it runs once per user command, so per-command rates
depend on the command rate (100 a second at the shipped `fps_max`).

## Constants

| Name (ours) | Value | Unit | Meaning |
|---|---|---|---|
| air_time | 12 | s | air with the eye under before drowning |
| drown_start | 2 | | drown damage counter after breathing |
| drown_step | +1 | per hit | the counter grows before each hit |
| drown_max | 5 | per hit | the counter's cap |
| drown_interval | 1 | s | between drowning hits |
| recover_amount | 10 | health | given back per recovery tick |
| recover_interval | 2 | s | between recovery ticks |
| slime_damage | 4 × water level | per command | acid |
| lava_damage | 10 × water level | per command | burn |
| bubble_chance | 1 / 32 | per command | then a second roll against the air left |
| bubble_volume | 0.8 | | `pl_swim1`-`4` |
| splash_volume | 1.0 | | `pl_wade1`-`4` on entering or leaving |
| swim_sound_gap | 1000 | ms | between swim-up sounds |
| water_view_gap | 4 | units | `cl_waterdist`, client cvar: the eye is kept this far from the surface |
| wave_extra | 16 × the liquid entity's wave height | units | added to the gap |
| `gl_fog` | 1 | | client cvar in the shipped `config.cfg`: underwater fog on |

## Behavior

### Which liquid

Point contents (bsp.md "Contents" and the liquid brush entities):
values −3 (water) down to −14 count as liquid for the water level; −3
water, −4 slime, −5 lava, and the current values −9 to −14 report as
water (movement.md "Currents"). The liquid type of the player is the
lowest (feet) test point's contents. A `func_water` whose `skin` is −4
or −5 is slime or lava; the stock maps have only water (bsp.md).

### Damage in slime and lava

In each player update, when the water level (as of the previous
command's movement) is above 0:

- Lava: damage 10 × water level, type burn (ref.: the throttle the code
  has for lava is never armed, so it hits on every command).
- Slime: damage 4 × water level, type acid, every command.
- Both go through armour like other non-bullet damage (damage.md:
  ratio 0.5, the zone of the player's last hit).
- At 10 ms commands, from 100 health, no armour: lava at level 1 kills
  in 10 commands (0.1 s), at level 3 in 4; slime at level 1 in 25
  (0.25 s), at level 3 in 9.

Movement in slime and lava is water's (friction, the 0.8 wish factor,
the 60 sink, the water jump); only the jump-held lift differs
(movement.md "Water").

### Air and drowning

Per player update:

- **Eye not under** (water level below 3), in any case: the air timer is
  reset to now + 12 s and the drown counter to 2. Just before that, if
  the player was under: if the air had run out, gasp with
  `pl_wade1.wav`; else if the player had been under for more than 3 s
  (less than 9 s of air left), gasp with `pl_wade2.wav`; both on the
  voice channel, volume 1, attenuation 0.8. If drowning damage is still
  owed, the recovery runs (below).
- **Eye under** (level 3): the recovery stops. Once now is past the
  air timer and past the next-pain time: the counter += 1 (capped at
  5), the player takes that much drowning damage (armour never absorbs
  it), the next-pain time = now + 1 s, and the damage is added to the
  owed total.
- So from the moment the eye goes under: hits of 3 at 12 s, 4 at 13 s,
  5 at 14 s and 5 every second after. From 100 health: dead at the
  21st hit, 32 s after going under.

Recovery: once breathing, a time-based tick runs at most every 2 s
(the shared time-based damage clock: the first tick may come at once if
none ran in the last 2 s): each gives back min(owed − given, 10) health
(not above the maximum, 100), until all is given back. Going under
again pauses it.

### Bubbles

Each player update with water level above 0: with chance 1/32, roll
a whole number 0-11; if it is at least the whole seconds of air left
(air timer − now, truncated), play one of `pl_swim1`-`pl_swim4.wav`
(body channel, volume 0.8, attenuation 0.8). With the eye out of the
liquid the air left is 12, so this is only heard under the surface, and
more often as the air runs out (about once in 32 × 12 / (12 − left)
commands).

### Splashes and wading sounds

- Entering or leaving: at the end of a walking command's movement, if
  the water level went from 0 to above 0, or from above 0 to 0, play one
  of `pl_wade1`-`pl_wade4.wav` at random (body channel, volume 1,
  attenuation 0.8). Any liquid.
- Swimming up: holding jump at water level 2 or more plays one of
  `pl_wade1`-`4` at volume 1, at most once every 1000 ms (a swim-sound
  timer counted down by `msec`).
- Wading and sloshing steps: [footsteps.md](footsteps.md) (water only,
  not slime or lava).
- A landing in water plays its step at volume 0.5 and does no fall
  damage (movement.md).

### The view in a liquid

- **Eye height near the surface** (client): at water level 2 or 3 the
  client moves the drawn eye so it is never within the gap (4 units +
  16 × the liquid entity's wave height, in the hardware renderer) of the
  surface: at level 2 (eye out) it scans up from gap below the eye, one
  unit at a time for at most gap steps, to the first non-liquid point,
  and puts the eye gap above it if that is higher; at level 3 (eye
  under) it scans down from gap above the eye to the first liquid point
  and keeps the eye gap below it. The view model moves with the eye.
  This only shifts the drawn view, not the movement.
- **Fog** (engine, hardware renderer, `gl_fog` 1): while the drawn eye
  is inside a liquid volume the world is fogged. The fog's colour is
  the liquid surface texture's palette entry 3 (the fourth colour) and
  its thickness the red value of palette entry 4 (the fifth). The
  install's liquid textures are authored that way: e.g. `liquids.wad`'s
  `!BWATER1`/`2`/`3` share colour (37, 40, 52) with thickness 255, 128,
  32; `!GWATER1` (24, 38, 8) with 255; `halflife.wad`'s `!WATERBLUE`
  (61, 98, 110) with 57. Applies to water, slime and lava alike. The
  fog's curve (how thickness maps to distance) is the engine's (Open
  questions 1).
- **Sound**: the engine switches its room effect to the `waterroom_type`
  preset (an engine cvar) while the listener is under (Open questions
  2).

## Per-tick order

One command (movement.md "Per-tick order" for the movement steps):

1. Player update: the water handling above (gasp, air, drowning,
   bubbles, slime or lava damage), using the water level from the end
   of the previous command.
2. The movement: water level found again, swimming, then the splash
   check at its end.
3. The time-based damage clock (recovery ticks) runs in the same player
   update, after the water handling.

## Edge cases

- Dead players: the reference's original code skips the water handling
  only when health is below 0 (a dead player at exactly 0 would still be
  handled); harmless, nothing to see.
- Noclip and frozen-in-place (no move type) players skip it.
- Surfacing and diving every second: the air resets on each surfacing
  command, so a player bobbing never drowns.
- The gasp needs more than 3 s under; a short dive is silent.
- Slime or lava damage uses the level from the previous command's
  movement: the first hit comes in the command after the movement that
  took the player in.

## Quirks

- Lava's throttle is dead code: lava hurts on every command, so its
  damage per second follows the command rate. Keep it per command.
- Slime also hurts every command (its own throttle time is set but
  never read).
- The gasp sounds reuse the wading files.

## Test cases

At 10 ms commands, 100 health, no armour.

| Setup | Input | After | Expected |
|---|---|---|---|
| eye under water | stay | 11.99 s | no damage |
| eye under water | stay | 12 s + 1 command | 3 damage (drowning) |
| eye under water | stay | 14 s + 1 command | total 12 (3 + 4 + 5) |
| eye under water | stay | 32 s + 1 command | dead (21 hits: 3 + 4 + 19 × 5 = 102) |
| drowned 12, surfaced | breathe | first recovery tick | +10 health |
| drowned 12, surfaced | breathe | next tick, 2 s later | +2 health; nothing owed |
| eye under 5 s | surface | | `pl_wade2` on the voice channel |
| eye under 13 s | surface | | `pl_wade1` |
| eye under 2 s | surface | | no gasp |
| armour 100, drowning hit | | | health −3, armour unchanged |
| standing in lava to the knees (level 1) | | 10 commands | dead |
| lava, level 3 | | 4 commands | dead (10 × 3 = 30 a command) |
| slime, level 1 | | 1 command | 4 damage |
| slime, level 2 | | 13 commands | dead (8 a command: 104) |
| walk from dry ground into water | | the command the feet go in | one `pl_wade1`-`4` at volume 1 |
| swimming at level 3 | hold jump | 200 commands | 2 swim-up sounds (commands 1 and 101) |
| eye under, 6 s of air left | stay | per command | bubble chance 1/32 × 6/12 = 1/64 |

## Open questions

1. The fog's curve: which formula turns the palette's thickness value
   into fog (exponential or linear, and the scale), and whether the
   fog's start is at the eye. Check in the client: screenshots under
   `!BWATER1` and `!BWATER3` water at known distances to a wall.
2. `waterroom_type`'s default value and what the preset does; read it
   from the client's console.
3. Whether a colour tint is drawn over the screen in liquids besides
   the fog (the software renderer's colour shift; slime and lava
   colours) in the hardware renderer.
4. The liquid texture whose palette drives the fog when the eye is in a
   `func_water`: assumed the entity's surface texture; for a world
   liquid leaf, the texture of the leaf's surface faces.
