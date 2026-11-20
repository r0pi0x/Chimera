# Source: when a key press becomes a user command

Source basis: the Source SDK 2013 client's input code (key button
states, how a command's buttons and move amounts are built, the extra
per-frame mouse sample, the command buffer per tick) read in place, and
the engine's frame loop as its public behaviour shows it (the engine
loop itself is not in the reference: those points are marked
*inferred*). Measured facts from specs/cs_source/movement.md ("Input: a
key's first command") and specs/tf2/movement.md (Open questions 13).
Applies to CS:S, HL2:DM and TF2 alike. Nothing copied; our words.
Status: draft

## Summary

The client builds exactly one user command per simulation tick, never
per rendered frame. Key events are handled at the start of each frame
and only change a key's state bits; the next command built reads them.
So a press is acted on in the same frame **if that frame runs a tick**,
otherwise in the next frame that does (at most one tick later). Short
taps are never lost: a "pressed since the last command" bit keeps a
press that was released before any command was built.

## Units and conventions

Seconds; tick interval T (TF2 and CS:S 0.015 s at 66.67 ticks/s);
a "frame" is one rendered client frame of any length.

## Constants

| Name (ours) | Value | Unit | Meaning |
|---|---|---|---|
| tick | 0.015 | s | one command per tick |
| first_command_share | 0.5 | – | share of a move key's amount on the first command after it goes down |
| tap_share | 0.25 | – | move key pressed and released between two commands |
| repress_share | 0.75 | – | move key released and pressed again between two commands, held at the end |
| release_share | 0 | – | move key released since the last command |

## Behavior

### Key state (per key or button)

Three bits: **down** (held now), **went down** (pressed since the last
command consumed it), **went up** (released since then). A press sets
down and went-down; a release clears down and sets went-up. Several
keys bound to the same action count as one (it goes up only when the
last one is released).

### Buttons in a command (attack, jump, duck, use, reload, …)

A button is **on** in a command when it is down **or** went down since
the last command. Building a real command then clears went-down
(went-up is not consulted for buttons). So:
- a tap shorter than a tick (press and release between two commands)
  is on in exactly one command;
- a held button is on in every command while held;
- a button released before a command and not re-pressed is off in it.
A game may force a button to need a fresh press (it stays off until it
goes up and down again; e.g. after a weapon switch).

### Move amounts (forward, back, left, right, up, down)

Each move key contributes amount × share, share by its bits since the
last command:

| went down | went up | down now | share |
|---|---|---|---|
| no | no | yes | 1 |
| no | no | no | 0 |
| yes | no | yes | 0.5 |
| no | yes | no | 0 |
| yes | yes | no | 0.25 |
| yes | yes | yes | 0.75 |

Reading the share clears both went-down and went-up. That is the
measured "first command counts half" (cs_source movement.md).

### When commands are built (the frame loop, inferred)

1. The frame starts: window and input events are processed; bound
   commands (`+attack`, `+forward`, …) run now and set the bits above.
2. The frame's time is added to an accumulator; for each whole tick in
   it, one command is built (buttons and moves from the bits, view
   angles from the current view), then predicted locally at once (the
   player's movement and weapon code run on it: the predicted shot's
   muzzle flash, sound and bullets come out in this same frame).
3. A frame that contains no whole tick builds no command; it only takes
   an extra mouse sample: the mouse's motion turns the view right away
   (the camera turns every frame), and the move keys' shares are
   evaluated into a throw-away command — which **clears the move keys'
   went-down/went-up bits** (buttons' bits are left alone).
4. The world is drawn.
5. Built commands are sent in batches at `cl_cmdrate` per second (each
   packet carries the new commands plus a few old ones as backup); the
   server runs every command in order, one tick each.

### So: does a command run on the frame its key is pressed?

- Frame rate **below** the tick rate (every frame has ≥ 1 tick): yes,
  always, in the first command of that frame; further commands of the
  same frame see the key as held (share 1, button on).
- Frame rate **above** the tick rate: in the pressing frame if it
  contains a tick, else in the first later frame that does; delay
  0 to 1 tick (≤ 15 ms), average about half a tick. Its effects show in
  the frame that built the command (prediction), not before.
- A button tap within one tick still fires (one command with it on).
- A move key pressed in a frame without a tick loses its half step
  (inferred from 3): the extra mouse sample consumes the went-down bit,
  so the first real command sees the key as held all along (share 1).
  At one tick per frame (the measurement's setup) the half step always
  shows.

## Per-tick order

1. Read buttons (down or went-down) into the command; clear went-down.
2. Read each move key's share; clear its went-down and went-up.
3. Take the view angles; stamp the command's number and the client tick.
4. Predict the command locally; queue it for sending.

## Edge cases

- Several commands in one frame (slow frames): the key's first command
  gets the press, the rest see it held.
- A press during a pause or with the console open: no command consumes
  it until input is active again.
- Bots' commands are built by the server, not by this input: no half
  step, no tap rule.

## Quirks

- The half-amount first command (keep: it is measured in all three
  games).
- The tap's 0.25 and the re-press's 0.75 shares.
- The above-tick-rate case where the extra mouse sample eats the half
  step (inferred, not measured).

## Test cases

| Setup | Input | After | Expected |
|---|---|---|---|
| 1 tick per frame | `+attack` pressed in frame k | frame k's command | attack on in it; predicted shot in frame k |
| 300 frames/s, tick every 3.33 frames | `+attack` in a frame with no tick | next frame with a tick | attack on in that command; nothing earlier |
| any | `+attack` pressed and released between two commands | next command | attack on; the one after: off |
| any | `+forward` (amount 400) pressed, held | commands 1, 2, 3 | forward 200, 400, 400 |
| any | `+forward` pressed and released between two commands | next command | forward 100 |
| any | `+forward` held, released and pressed again between two commands | next command | forward 300 |
| any | `+forward` released | next command | forward 0 |

## Open questions

1. The engine's frame loop (event pump, tick accumulator, when the
   extra mouse sample runs, prediction per command or per frame) is
   inferred from behaviour; check the above-tick-rate half-step case
   with `host_framerate`/`fps_max` 300 and the prediction watch used for
   the measurement.
2. Whether TF2's `cl_cmdrate` default is 66 or 30 changes only the
   sending, not when commands are built.
