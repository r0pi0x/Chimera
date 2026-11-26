# Counter-Strike 1.6: footsteps

Source basis: the community reimplementation of the CS 1.6 game library
(ReGameDLL_CS, open source; its shared player movement, read in a
separate spec session as reference only, its original-game behaviour,
never its optional fixes or additions), the community reimplementation
of the CS 1.6 client library (cs16-client: its copy of the shared
movement and its input code), Valve's Half-Life SDK for the sound
constants; then checked against the install: the client's
`cstrike/sound/materials.txt` and `sound/player/` files
(`~/games/cs16`), and `mp_footsteps` read from the HLDS (build 10211,
`mod cstrike`, no server config). Numbers marked "(ref.)" were not
measured in the running game. No code is reproduced here.
Status: draft

## Summary

Footsteps are part of the shared movement: on each command a step timer
counts down; when it runs out and the player is on the ground or on a
ladder moving faster than 150 units/s (3D), a step sound for the surface
under the feet plays and the timer is set again by the surface. Walking
(the walk key caps the speed at 0.52 of the maximum, 130 with a 250 gun)
and ducked movement (0.333 of it) are below 150, so they are silent.
Jumps and hard landings play a step too.

## Units and conventions

Units, time and commands as in [movement.md](movement.md) (10 ms
commands at the shipped 100 frames a second). Volumes are 0-1.
Attenuation is GoldSrc's sound attenuation number (0 = heard
everywhere; larger falls off sooner).

## Constants

| Name (ours) | Value | Unit | Meaning |
|---|---|---|---|
| step_min_speed | 150 | units/s | at or below this (3D speed) no step; the timer is set to 400 instead |
| step_idle_timer | 400 | ms | timer set on each check while too slow |
| step_interval | 300 | ms | timer after a step on any surface, and when sloshing |
| step_interval_wade | 600 | ms | timer after a wading step (knee in water) |
| step_interval_ladder | 350 + 100 | ms | ladder steps (the +100 is the ducked-or-ladder extra) |
| step_extra_ducked | 100 | ms | added when ducked or on a ladder |
| unduck_step_credit | 100 | ms | taken off the timer when the player stands up |
| step_volume | see table | | per surface |
| step_volume_ducked | × 0.35 | | ducked steps (see Behavior) |
| step_attenuation | 0.8 | | the normal attenuation (Half-Life SDK value) |
| texture_probe | 64 | units | the surface is the texture 64 units straight below the origin |
| name_cut | 16 | characters | texture names are compared on their first 16 characters (ref.) |
| table_max | 1024 | entries | `materials.txt` entries read (ref.; the file has 701) |
| `mp_footsteps` | 1 | | server cvar (HLDS default); 0 silences all steps |

## Behavior

### The surface type

The surface under the player is found on each step (and on each jump
and landing), not cached between them:

1. Take the texture of the face hit by a line traced from the player's
   origin straight down 64 units, against the entity the player stands
   on (the world or a brush entity). Standing, the line reaches 28 units
   below the feet; ducked, 46. Nothing hit: concrete.
2. Normalise the name: if it starts with `-` or `+`, drop that character
   and the one after it (the tiling or animation frame); then, if it
   starts with `{`, `!`, `~` or a space, drop that one character.
3. Look it up in `sound/materials.txt` (the chain's first: `cstrike`'s).
   CS 1.6 differs from Half-Life's rule in
   [../goldsrc/mount.md](../goldsrc/mount.md) (12 characters, 512
   entries, first match) (ref.):
   - Every entry is read (up to 1024). A line counts when its first
     non-space character is a letter (the type, upper-cased) followed by
     whitespace, a name, and whitespace after the name (the line's own
     line break counts; a last line with no line break is dropped). Lines
     starting with `/` or any non-letter are skipped. Names are kept to
     16 characters.
   - The entries are sorted by name ignoring case and searched by
     halving; a name matches when its first 16 characters equal an
     entry's, ignoring case (whole names in practice: WAD texture names
     are at most 15 characters). No match: concrete (`C`).
   - Entries written with a prefix (`-0cstrike_me1pl`,
     `{cstrike_oe4ven`, 56 such in the install) never match, because the
     texture's name is stripped first and the entry's is not.
   - One name is listed twice with different types: `GENERIC015V` as M
     and as G. The sorted search lands on the M entry: metal.
4. Map the type letter to a step sound set:

| Type letter (materials.txt) | Step set | Volume | Timer after the step |
|---|---|---|---|
| `C` concrete, and any letter not below (`W` wood, `P` computer, `Y` glass, `X` grass, `F` flesh) | concrete | 0.5 | 300 |
| `M` metal | metal | 0.5 | 300 |
| `D` dirt | dirt | 0.55 | 300 |
| `V` vent | vent | 0.7 | 300 |
| `G` grate | grate | 0.5 | 300 |
| `T` tile | tile | 0.5 | 300 |
| `S` slosh | slosh | 0.5 | 300 |
| `N` snow | snow | 0.5 | 300 |

Wood, computer, glass and grass have no step sounds of their own in
CS 1.6: they step as concrete (their letters matter for bullet impacts
only).

### What overrides the surface

Checked in this order, the first that holds wins:

1. **On a ladder** (move type fly, movement.md "Ladders"): ladder set,
   volume 0.35, timer 350 (+100 below: 450).
2. **Knee in water**: the point at origin − 0.3 × box height (standing
   72: 21.6 below the origin, 14.4 above the feet; ducked 36: 10.8 below)
   is water (contents −3 only; slime and lava do not count): wade set,
   volume 0.65, timer 600.
3. **Feet in water**: the point at origin − 0.5 × box height (the box's
   bottom) is water: slosh set, volume 0.5, timer 300.
4. Otherwise the surface's row above.

### When a step plays

On each command, after the duck handling and before the ladder or walk
movement (movement.md "Per-tick order" step 8):

1. The step timer was already counted down by the command's `msec`
   (floored at 0) at the start of the command.
2. If the timer is above 0, or the player is frozen: nothing.
3. Speed = the length of the full velocity (x, y and z), as left by the
   previous command. If speed ≤ 150: timer = 400, nothing plays.
4. If neither on the ground nor on a ladder: nothing (the timer stays at
   0, so a step plays on the first ground command above 150).
5. Find the set, volume and timer (above).
6. If the player is ducked or on a ladder: timer += 100. If ducked
   (fully, the ducked flag) and the duck timer is below 950 ms: volume
   × 0.35. (The duck timer is set to 1000 when the duck key goes down and
   counts down, so this is nearly always true once ducked.)
7. Play the step (below).

Standing up from a duck takes 100 off the step timer (floored at 0).

At 10 ms commands a running player on concrete steps every 30 commands
(0.3 s); on a ladder every 45 (0.45 s); wading every 60 (0.6 s).

### Jumping and landing

- A jump from the ground (movement.md "Jumping") finds the surface and,
  if the 3D speed at take-off (before the jump's vertical speed is set)
  is at least 150, plays its step at volume 1.0. The step timer is not
  changed. Wading, sloshing and ladders do not apply: the surface's own
  set is used even in shallow water.
- A landing (movement.md "Fall damage and landing") with a volume above
  0 plays the surface's step at that volume (1, 0.85, or 0.5 in water)
  and sets the timer to 300.
- The water jump plays no step.

### Playing a step

- Each step flips the foot (left/right), even when the sound is then
  not played (`mp_footsteps` 0, or a repeated prediction of the same
  command).
- The file: the set's files are numbered 1-4; the right foot picks 1 or
  3, the left foot 2 or 4, each at even odds.
  - Tile: one step in five (at random) plays `pl_tile5` instead.
  - Wade: one wading step in four is skipped: a counter goes 0, 1, 2, 3,
    0, ...; the step with the counter at 0 is silent. The counter is
    shared by every player the movement code runs for (one per server,
    one per client), not per player.
- `mp_footsteps` 0 (multiplayer): no step sounds at all.
- The sound comes from the player entity on its body channel (a new
  body-channel sound from the same player cuts the previous one), at
  the volume above, attenuation 0.8, normal pitch.

| Set | Files (`sound/player/`) |
|---|---|
| concrete | `pl_step1`-`pl_step4` |
| metal | `pl_metal1`-`pl_metal4` |
| dirt | `pl_dirt1`-`pl_dirt4` |
| vent | `pl_duct1`-`pl_duct4` |
| grate | `pl_grate1`-`pl_grate4` |
| tile | `pl_tile1`-`pl_tile4`, and `pl_tile5` |
| slosh | `pl_slosh1`-`pl_slosh4` |
| wade | `pl_wade1`-`pl_wade4` |
| ladder | `pl_ladder1`-`pl_ladder4` |
| snow | `pl_snow1`-`pl_snow4` (`pl_snow5`, `pl_snow6` are in the install, unused) |

All are `.wav` in `cstrike/sound/player/` (the snow set only there; the
others also in `valve/`, overridden by the chain).

### Who hears them

- The movement runs on the server for every player and on each client
  for its own player (prediction). A step is emitted only on a command's
  first run on that side (not on the client's re-predictions).
- Others hear the server's copy, from the player's position, at
  attenuation 0.8. The moving player hears its own predicted copy, so
  its own steps are not delayed by the network. That the server leaves
  the mover out of its copy's recipients is the engine's and is not in
  the references (Open questions).
- Distance: GoldSrc's sound falls off linearly with distance × the
  attenuation / 1000 (the engine's, Quake lineage, not in the
  references): at 0.8 a step is silent beyond 1250 units and at half
  volume at 625 (Open questions).

## Per-tick order

Within one command (movement.md "Per-tick order"): the timers count
down (step 2), the duck handling (step 7, standing up credits 100 ms),
then the footstep check (step 8), then the move. A jump's step plays in
the jump (step 13), a landing's at the end (step 19).

## Edge cases

- The speed tested is the previous command's velocity (the check runs
  before this command's move), including its vertical part: running
  down a slope or stairs counts the vertical speed too.
- On a ladder the speed is usually the climb speed (200, capped by the
  maximum speed): audible. Ducked on a ladder the climb is 200 × 0.333 =
  66.6: silent.
- From a standstill, holding forward: the first check sets the timer
  to 400 (speed 0); it next runs out 40 commands later, by when the
  speed is above 150, so the first step plays 0.4 s after the key goes
  down; then every 0.3 s.
- In the air (falling, jumping): no steps; the timer stays at 0 and a
  step plays on the first ground command if fast enough, in addition to
  any landing step that command.
- Swimming (water level 2 and up) is never on the ground: no steps; the
  swim sounds are in [water.md](water.md).
- Slime and lava are not water for the wade and slosh tests: a player
  wading in slime steps on the bottom's texture.

## Quirks

- Walking with the walk key is silent only because 0.52 of every
  stock maximum speed (at most 260) is below 150; nothing else silences
  it.
- The wade skip counter is shared by all players.
- The `GENERIC015V` double entry resolves to metal by the sort order;
  keep it.
- Entries with a prefix in `materials.txt` are dead.

## Test cases

At 10 ms commands, `mp_footsteps` 1, standing hull unless said.

| Setup | Input | After | Expected |
|---|---|---|---|
| standing on `generic015v` | look up the type | | M (metal), volume 0.5 |
| texture `-0cstrike_me1pl` | look up | | name `cstrike_me1pl`; not listed without the prefix: C |
| texture `{pi_treeb` | look up | | name `pi_treeb` |
| a texture in no entry | look up | | C: concrete set, 0.5 |
| floor textured `W` (wood) | run at 250 | | `pl_step1`-`4` at 0.5 |
| floor textured `N` | run at 250 | | `pl_snow1`-`4` at 0.5 |
| standing still on concrete | hold forward (250 gun) | 40 commands | the first step on command 41 (0.4 s), then every 30 commands |
| running at 250 on concrete | keep running | 1 s | 3 or 4 steps (every 0.3 s), feet alternating: files 1/3 then 2/4 |
| running on a vent | | | volume 0.7, every 0.3 s |
| walking with the walk key, 250 gun | hold forward | any | speed 130: no step; the timer held at 400 |
| ducked, moving at 83.3 | | any | no step |
| sliding ducked at 200 (after a landing) | | | steps every 0.4 s at 0.5 × 0.35 = 0.175 |
| climbing a ladder at 200 | hold forward | | `pl_ladder1`-`4` at 0.35 every 0.45 s |
| climbing ducked at 66.6 | | | no step |
| knee-deep water, running at 250 | | 2.4 s | 4 checks at 0.6 s; 3 wade sounds at 0.65 (one in four skipped) |
| feet in water only (water 10 units deep) | run at 250 | | `pl_slosh1`-`4` at 0.5 every 0.3 s |
| knee-deep slime | run at 250 | | the bottom texture's steps |
| running at 250 | jump | take-off | the surface's step at volume 1.0 |
| standing still | jump | take-off | no step (speed 0) |
| landing at 300 units/s on metal | | | `pl_metal*` at 0.85; step timer 300 |
| `mp_footsteps 0` | run | | silent; the foot still alternates |

## Open questions

1. Whether the server sends a player's own steps to that player too
   (heard twice, once predicted) or leaves the mover out; in the client,
   run on a server with `cl_lw 0` and listen.
2. The engine's distance rule (linear to 0 at 1000 / attenuation) and
   stereo spread for GoldSrc: measure with two clients at known
   distances, or record a demo with the client.
3. Whether the CS 1.6 binary really reads 1024 entries and compares 16
   characters (the reference's numbers; Half-Life's SDK has 512 and 12):
   walk on `cudumpsterc` (M, the file's 691st entry): metal steps confirm
   the 1024 limit, concrete steps mean 512.
