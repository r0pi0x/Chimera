# Counter-Strike 1.6: shell ejection

Source basis: the community reimplementation of the CS 1.6 client
library (cs16-client: its weapon fire events and its handler of the
server's shell message), the community reimplementation of the CS 1.6
game library (ReGameDLL_CS: which weapons eject late and the server's
shell message), Valve's Half-Life SDK for the temporary-model message's
layout and bounce sound types; all read in a separate spec session as
reference only. Checked against the install: the shell models and
bounce sounds present in `~/games/cs16`. Numbers are the references'
(not measured in the running game). No code is reproduced here.
Status: draft

## Summary

Each shot of a gun throws one spent case as a short-lived bouncing
model, made on each client (no network entity). Most guns throw it from
the fire event, on the shot itself; the pump shotgun and the two bolt
sniper rifles throw it later, from the server, when the pump or bolt
moves. The case starts beside the gun, flies out sideways, up and
forward with random speeds, and clinks on each bounce.

## Units and conventions

Units and angles as in [movement.md](movement.md). Vectors **f**, **r**,
**u** are forward, right and up of the shooter's view angles plus the
current punch angle (the event carries both). "Eye" is the shooter's
origin + (0, 0, view height): the local player's current view height;
another player's 17, or 12 when the event says ducked. U(a, b) is a
uniform random number.

## Constants

| Name (ours) | Value | Unit | Meaning |
|---|---|---|---|
| shell_life | 2.5 | s | a case's lifetime |
| side_speed | U(50, 70) | units/s | sideways speed (event shells) |
| up_speed | U(75, 175) | units/s | upward speed (event shells) |
| forward_speed | U(25, 250) | units/s | forward speed (event shells) |
| carry_velocity | 0.5 | × | share of the shooter's velocity a case keeps |
| lead_velocity | 0.1 | s | the start point is moved by the shooter's velocity × this |
| third_person_offset | 20, −12, 4 | units | forward, up, right from the eye, other players' guns |
| late_up | U(100, 150) | units/s | late shells |
| late_side | U(50, 70) | units/s | late shells |
| late_forward | 25 | units/s | late shells |
| late_offset | 16 forward, −9 up | units | late shells' start from the eye |

## Behavior

### Models and bounce sounds

| Model | Guns | Bounce sound type |
|---|---|---|
| `models/pshell.mdl` (cstrike) | Glock, USP, P228, Deagle, Five-SeveN, Elites, TMP, UMP45 | brass |
| `models/rshell.mdl` (cstrike) | AK-47, M4A1, AUG, SG552, Galil, FAMAS, SG550, G3SG1, M249, P90, MP5, MAC-10 | brass |
| `models/rshell_big.mdl` (cstrike) | Scout, AWP (late) | brass |
| `models/shotgunshell.mdl` (valve) | XM1014 | brass |
| `models/shotgunshell.mdl` | M3 (late) | shotgun |

- Brass: one of `sound/player/pl_shell1`-`3.wav` per bounce
  (`pl_shell1` from cstrike, 2 and 3 from valve by the chain).
- Shotgun: one of `sound/weapons/sshell1`-`3.wav` (valve).
- Which file of the three, how loud, and how many bounces sound are the
  engine's temporary-model rules (Open questions); the client only names
  the type.

### Event shells (on the shot)

Every gun but the knife, the grenades, the M3, Scout and AWP throws one
case per bullet event: each shot, each round of a Glock or FAMAS burst,
one per XM1014 blast (not per pellet). The client that receives the
shot's event (every client the event reaches, and the shooter's own
predicting client) makes it:

- Start = eye + shooter velocity × 0.1 + F·**f** + Up·**u** + R·**r**, with
  (F, Up, R) from the table below.
- Velocity = shooter velocity × 0.5 + s · U(50, 70) · **r** +
  U(75, 175) · **u** + U(25, 250) · **f**, where the sideways sign s is
  −1 when R > 0 and +1 when R ≤ 0 (the reference's rule; see Open
  questions 1). One left-handed case (below) reverses s.
- Initial yaw: the shooter's yaw. Life 2.5 s. Model and sound type from
  the table above.

First-person offsets (the shooter's own client; `cl_righthand` 1, the
default, uses R as listed; `cl_righthand` 0 uses −R and the F listed in
the left column where it differs):

| Gun | F | Up | R (right-handed) | Left-handed F if different |
|---|---|---|---|---|
| AK-47, Galil, M4A1, SG552 | 20 | −8 | 10 | |
| SG550, G3SG1 | 17 | −8 | 10 | 20 |
| AUG, FAMAS | 17 | −8 | 14 | |
| M249 | 20 | −10 | 13 | |
| MP5 | 36 | −10 | 11 | |
| UMP45 | 34 | −10 | 11 | |
| TMP | 32 | −6 | 11 | |
| MAC-10 | 32 | −9 | 11 | |
| P90 | 35 | −16 | 22 | |
| XM1014 | 22 | −9 | 11 | |
| Glock, USP, P228, Five-SeveN | 36 | −14 | 14 | |
| Deagle | 35 | −11 | 16 | |
| Elites | 35 | −11 | −16 for the right gun's shot, +16 for the left gun's | (no handedness switch) |

Other players' guns (third person, any gun above): F 20, Up −12, R 4.

### Late shells (pump and bolt)

| Gun | Delay after the shot | Model | Sound type |
|---|---|---|---|
| M3 | 0.45 s | `shotgunshell` | shotgun |
| Scout | 0.56 s | `rshell_big` | brass |
| AWP | 0.55 s | `rshell_big` | brass |

- The shot sets a per-player eject time; the server checks it in the
  held weapon's per-command processing and, once the time has passed,
  clears it and throws the case. It uses the vectors of the view angles
  plus punch at that moment (not at the shot).
- Start = eye + 16·**f** − 9·**u**.
- Velocity = shooter velocity (whole) + U(50, 70)·**r** + U(100, 150)·**u**
  + 25·**f**.
- Sent as the game's shell message to every client whose potentially
  visible set holds the start point, with the model, the sound type, the
  life (25 tenths: 2.5 s) and the shooter's player number.
- Each receiving client moves the start 9 units sideways and, for the
  local right-handed shooter, adds a sideways push of 120 units/s
  (Open questions 2 on the direction).

## Per-tick order

Event shells appear when the client plays the shot's event: on the
shooter's client in the command that fires (predicted), on others' when
the server's event arrives. Late shells appear in the server command at
or after the eject time, after the movement (with the weapon's
processing, weapons.md "Common rules").

## Edge cases

- Switching weapons before a late shell's time: the eject time belongs
  to the player, so the next weapon's processing throws it with that
  weapon's late-shell model; only the M3, XM1014, Glock, SG550, G3SG1,
  Scout and AWP name one (ref.); for the others no model is set and
  nothing is seen.
- Dying before the late time: no processing, no case (ref.).
- Silenced M4A1 and USP throw cases as unsilenced.
- Empty-chamber or dry fire: no event, no case.
- The case collides with the world and bounces (engine).

## Quirks

- The XM1014's case is the shotgun shell model but clinks as brass.
- The MP5 and MAC-10 throw rifle cases (`rshell`) on the client though
  the server precaches the pistol case for them; what players see is the
  client's. Keep it.
- Late shells use the view at the eject time: looking around during the
  pump throws the case in the new direction.

## Test cases

Shooter at the origin, standing (view height 17), still, yaw 0, pitch 0,
no punch: **f** = (1, 0, 0), **r** = (0, −1, 0), **u** = (0, 0, 1).

| Setup | Input | Expected |
|---|---|---|
| local AK-47, right-handed | one shot | one `rshell`, start (20, −10, 9); velocity x in [25, 250], y in [50, 70] (s = −1 along **r**), z in [75, 175]; gone after 2.5 s |
| local AK-47, left-handed | one shot | start (20, 10, 9); y velocity in [−70, −50] |
| another player's AK-47 | one shot seen | start (20, −4, 5) |
| local Deagle | one shot | `pshell`, start (35, −16, 6) |
| local Elites | right gun fires | start (35, 16, 6), y velocity in [−70, −50] |
| local Glock, burst | one burst | three `pshell` cases, one per round |
| XM1014 | one blast | one `shotgunshell`, brass clinks |
| M3 | one shot | no case at the shot; one `shotgunshell` 0.45 s later at (16, 0, 8), velocity x 25, y in [−70, −50], z in [100, 150]; shotgun clinks |
| AWP | one shot | one `rshell_big` 0.55 s later |
| shooter running at 250 along x | AK-47 shot | start x 20 + 25 = 45; velocity x gains 125 |

## Open questions

1. The sideways direction of event shells: the client reference flips
   it against the side of the start offset (a right-handed case starts
   10 units right of the eye and flies left), where Half-Life's SDK
   throws its cases to the right. In the game, fire an AK-47 standing
   still and see which way the cases fly; if right, use s = +1 for
   R > 0.
2. The late shells' client-side sideways offset and push: the reference
   computes the sideways direction from the yaw in a way that does not
   follow the yaw's sine and cosine; check in the game which side an
   AWP's case leaves on and implement 9 units and 120 units/s to that
   side (left of the view for others is the guess).
3. The engine's temporary-model physics: gravity (assumed the world's
   `sv_gravity`), bounce damping, spin, the random pick among the three
   bounce sounds, their volume and attenuation, how many bounces sound,
   and the fade before the 2.5 s end. Measure in the client by
   recording a demo of shells falling on flat ground.
4. Whether the reference client reads the shell message's life (25) as
   tenths (2.5 s, the message's documented unit) or as seconds; 2.5 s is
   assumed.
