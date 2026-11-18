# Team Fortress 2: faces — idle expressions, blinking, eye aim, dead faces (T11)

Source basis: the SDK's TF2 game code (Valve's Source SDK 2013 with
TF2's client and server code, read in place on the Linux box, nothing
copied): how the server picks and replays a player's idle expression
scene, how the client applies a scene's expression events and the flex
controllers' ranges, its blink and look-target code, and how the class
ragdoll takes its face; the shared choreography code for the binary
scene layout and its intensity curves. The retail client install (app
440, PatchVersion 11191711), read in place with a throwaway reader
(nothing extracted kept): the response rules
(`scripts/talker/response_rules.txt` and the per-class files it
includes), the compiled scenes in `scenes/scenes.image`, the expression
files `expressions/player/<class>/emotion/emotion.vfe`, and the flex
controllers, eyeballs and eye attachments of `models/player/<class>.mdl`.
No game was launched.
Status: draft (checked against the SDK's game code and the game's data,
2026-11-18)

## Summary

A living TF2 player never shows its model's base face (every controller
at 0) for long: the server keeps a looping **idle expression scene**
running on every living player, chosen per class and state (normal,
hurt, round lost, competitive winner, scoped Sniper, firing Minigun) by
the response rules, and every client plays it. The scene holds one
named preset of the class's expression file (`defaultFace`, `idleface`,
`madUpper`, …) at a set intensity, which pulls the preset's
controllers (the face shape plus, on most classes, the three eyelid
controllers) to the preset's values. For the Engineer that is
`defaultFace` (1.0 for most of each 30 s loop, 0.98 at its two ramp samples) and nothing else: it is what closes his mouth.
There is **no blinking** on TF2 players, the eyes look at the nearest
living player within 300 units in front, and the head does not turn.
A ragdoll copies its dead owner's face (the idle scene keeps playing to
its end) until the owner respawns.

## Units and conventions

- Controller values are in each controller's own range (most [0, 1];
  `right_CloseLid`, `left_CloseLid`, `multi_CloseLid` [−1, 1];
  `eyes_updown`, `eyes_rightleft` [−45, 45] degrees). "Base" means every
  controller at the real value 0.
- Times in seconds, scene-local (0 = the scene's start) unless said
  event-local (0 = the event's start).
- Intensities in [0, 1]; the binary scenes store them as a byte n,
  meaning n/255 (so 0.98 is 250/255, 0.733 is 187/255, 0.514 is 131/255).
- Distances in Hammer units (u).

## Constants

| Name (ours) | Value | Unit | Meaning |
|---|---|---|---|
| idle concept | `TLK_PLAYER_EXPRESSION` | – | response concept the server asks for an idle face |
| fire concept | `TLK_FIREWEAPON` | – | response concept for the firing face |
| retry when no rule | U(30, 40) | s | next attempt when no idle rule matches (the Pyro, always) |
| low-health fraction | < 0.25 | health / max health | `LowHealthContext` criterion (strict) |
| low-health boundary | floor(0.25 × max health) | HP | damage taking health from above to at or below it restarts the idle face |
| fire face spacing | 5 | s | minimum gap between firing faces (or the last one's length, if longer) |
| fire face slots | 4 | – | firing scenes kept at once (the oldest is stopped) |
| look radius | 300 | u | other players farther than this (origin to origin, 3D) are not looked at |
| forward look distance | 512 | u | look point when no player qualifies |
| eye cone | cos 30° = 0.866 | – | eye deflection limit; the class models leave it unset, so the default applies |
| look-at cvar | `tf_clientsideeye_lookats` 1 | – | 0: eyes always look at the forward point |

## Behavior

### 1. The faces

| Class | Controllers | Eyeballs | `eyes` attachment | `blink` | Lids (`*_CloseLid`) | Idle preset's file |
|---|---|---|---|---|---|---|
| Scout | 42 | 2 | yes | yes | yes | `player/scout/emotion/emotion` |
| Soldier | 42 | 2 | yes | yes | yes | `player/soldier/emotion/emotion` |
| Pyro | 1 (`dummyBlend`) | 0 | yes | no | no | none |
| Demoman | 41 | **1** | yes | yes | yes | `player/demo/emotion/emotion` |
| Heavy | 46 | 2 | yes | yes | yes | `player/heavy/emotion/emotion` |
| Engineer | 41 | **0** | **no** | **no** | **no** | `player/engineer/emotion/emotion` |
| Medic | 49 | 2 | yes | yes | yes | `player/medic/emotion/emotion` |
| Sniper | 53 | 2 | yes | yes | yes | `player/sniper/emotion/emotion` |
| Spy | 44 | 2 | yes | yes | yes | `player/spy/emotion/emotion` |

The expression file named in a scene resolves to
`expressions/<name>.vfe`. The `hwm` copies under `expressions/player/hwm/`
are for the high-detail models, not the in-game players.

### 2. Base values

- When a player gets its model and at every spawn, the client stops any
  scene events on it and sets every controller to the real value 0
  (stored normalized as (0 − min)/(max − min): 0 for [0, 1] controllers,
  0.5 for the symmetric ones). TF2 players do **not** network their
  controller values, blink toggle or look target; each client keeps its
  own.
- These base values are only the starting point of each frame (§4); the
  scene then pulls the named controllers away from them.

### 3. Choosing the idle face (server)

Every server think of a living player, when all of these hold:

- the player is alive and not standing ready for a partner taunt;
- no firing face is due to block it (the firing-face timer, §7, has run
  out);
- the idle timer is not switched off (§3.3) and has run out;

the server stops the current idle scene and all firing scenes, forgets
which idle scene it was, and asks the response rules for
`TLK_PLAYER_EXPRESSION`:

- **A rule matched**: if the chosen scene is the one already playing,
  nothing happens (cannot occur on this path, since it was just
  forgotten); otherwise the scene starts on this player as a networked
  multiplayer scene (sent to every client) and the idle timer is set to
  now + the scene's length. When it runs out the same choice is made
  again, so the idle face loops by restarting.
- **No rule matched**: no scene; next attempt in U(30, 40) s.

The idle timer is also set to "now" (an immediate re-pick on the next
think) at spawn, and whenever damage takes the player's health from
above floor(0.25 × max health) to at or below it ("look worried").
Healing back above a quarter does **not** re-pick: the hurt face lasts
until its scene ends.

#### 3.1 Rule choice

Each rule scores the sum of its criteria's weights (all 1 here) when all
its required criteria match; the highest score wins and ties are broken
uniformly at random, at each pick. The criteria used:

| Criterion | Meaning |
|---|---|
| `IsScout` … `IsEngineer` | the player's real class (the expression ask carries no disguise) |
| `LowHealthContext` | health / max health < 0.25 |
| `GameRulesInWinState` + `PlayerOnLosingTeam` | the round is over (win state) and this player's team lost |
| `IsCompWinner` | the player has the competitive-winner condition |
| `DeployedContext` + `WeaponIsSniperrifle` (or `…Classic…`) | a Sniper zoomed with a sniper rifle |
| `IsFiringMinigun` | the Minigun has been firing for more than 0 s |

#### 3.2 The rules (scenes under `scenes/player/<class>/low/`)

| Class | Normal (score 2) | Hurt (3) | Round lost (4) | Comp winner (3) | Other |
|---|---|---|---|---|---|
| Scout | `idleloop01` | `idleloopHurt01` | `idleloopHurt01` | `comp_winner_idle_face` | – |
| Soldier | `idleloop01` | `idleloopHurt01` | `idleloopHurt01` | `comp_winner_idle_face` | – |
| Pyro | none | none | none | none | – |
| Demoman | `idleloop01` | `idleloopHurt01` | `idleloopHurt01` | `comp_winner_idle_face` | – |
| Heavy | `idleloop01` | `idleloopHurt01` | `idleloopHurt01` | `comp_winner_idle_face` | firing Minigun (3): `attackMinigun02` |
| Engineer | `idleloop01` | `idleloopHurt01` | `idleloopHurt01` | `comp_winner_idle_face` | – |
| Medic | `idleloop01` | `idleloopHurt01` | `idleloopHurt01` | `comp_winner_idle_face` | – |
| Sniper | `idleloop01` | `idleloopHurt01` | `idleloopHurt01` | `comp_winner_idle_face` | scoped (4): `idleloopDeployed01` |
| Spy | `idleloop01` | none | none | `comp_winner_idle_face` | – |

(All with `.vcd`.) A few more rules answer the same concept with voice
lines (a scoped Sniper with an enemy under the crosshair, Halloween
costumes and events); they are speech, not the resting face, and are
out of scope here.

#### 3.3 The Minigun switch

When the Heavy starts winding up the Minigun, the idle timer is
switched off. Nothing switches it back on except the spawn reset and
the low-health restart above. So after the Heavy's first wind-up in a
life, the running idle scene plays to its end and then **no idle face
returns** (base face) until he respawns or drops below a quarter of his
health. A low-health restart while still firing scores 3 for both
"hurt" and "firing Minigun": a coin flip.

### 4. How a scene drives the controllers (each client, each frame)

The client plays the networked scene itself, for the player it was
started on (the scene's actor name does not matter; the
`comp_winner_idle_face` scenes all name their actor "demoman"). Per
frame, per player:

1. Every controller starts at its base (stored) value, converted to its
   real range.
2. For each active **expression event** of the playing scenes (the idle
   scene and any firing or other scene), in turn: look up the event's
   expression file and its named preset (case-insensitive). If either
   is missing, the event does nothing. Otherwise, with the event's
   intensity I at the current scene time, for each controller the
   preset lists with weight w and influence f (f = 1 in every preset
   used here):

```math
s = \operatorname{clamp}(I \cdot f,\ 0,\ 1), \qquad v \leftarrow v\,(1 - s) + w\,s
```

   A preset entry whose controller the model lacks is ignored; model
   controllers the preset does not list keep their value. Weights are
   in the controller's real range (lid weights are negative).
3. Then speech visemes are added (out of scope), and the result goes
   through the model's flex rules (as HL2:DM, player_models.md §11.1).

Starting from base 0, a preset's controller therefore ends at w·I, and
everything else stays 0.

#### 4.1 Intensity of an event

I(t) = the scene's global ramp at scene time t (all these scenes have
none: 1) × the event's own ramp at event-local time τ = t − start:

- No ramp samples: I = 1 for the whole event.
- Otherwise I is a Catmull-Rom curve through the samples (τ_k, i_k),
  completed by two **edge points**: (0, 0) before the first sample and
  (event length, 0) after the last (the ramps carry no edge settings in
  the binary scenes; their default is value 0, default curve). The
  binary ramps store no per-sample curve type either, so every span uses
  the event's default curve: Catmull-Rom with "normalized x" on both
  sides. For τ in the span from P1 to P2 (consecutive points, edge points
  included), with Δ = P2.τ − P1.τ and u = (τ − P1.τ)/Δ, the outer
  neighbours P0 (before P1) and P3 (after P2) are chosen so:
  - **A neighbour that would lie beyond an edge** (P0 when P1 is the first
    sample or the start edge; P3 when P2 is the last sample or the end
    edge) is the edge point (value 0) **moved to the time of the span's
    own endpoint** (P1's or P2's). It is then not slid (its time distance
    is 0), so it enters the formula as y = 0.
  - **A neighbour that is a real sample** is slid along the straight line
    from the span's endpoint through it, by time: P0' = P1 + (P0 − P1)·Δ/(P1.τ − P0.τ),
    P3' = P2 + (P3 − P2)·Δ/(P3.τ − P2.τ). Both coordinates move by the
    same factor, so the slid point lies exactly Δ in time from the
    endpoint; it is a time ratio, not a 2D length.

  Then, with y the points' intensities,

```math
I = \tfrac12\left(2y_1 + (y_2 - y_0')u + (2y_0' - 5y_1 + 4y_2 - y_3')u^2 + (-y_0' + 3y_1 - 3y_2 + y_3')u^3\right)
```

  clamped to [0, 1].
- Consequence for the common two-sample ramp (v at a, v at b, edges 0
  at 0 and at the length L): on the middle span both neighbours are edge
  points with y = 0, so

```math
I(u) = v\left(1 + \tfrac12 u(1-u)\right), \quad u = \frac{\tau - a}{b - a}
```

  It is **not flat**: it bulges to 1.125·v at the middle (u = ½) and is v
  only at the two samples; above 1 it clamps to 1. The first span (0 to
  a) rises from 0 to v, the last (b to L) falls from v to 0.
- An event only acts between its start and end time.

So every idle scene fades its face in from 0 over its first fraction of
a second, bulges gently through the middle, and fades it out to 0 at
its end; the re-pick (§3) then starts the next copy from 0 again.

### 5. Resting values (normal idle, alive, not speaking, not firing)

The values are w·I, with I from §4.1: "at the samples" is v (the
sample value), "mid-scene" is the bulge at u = ½ (1.125·v, at most 1).

| Class | Scene length | Preset (event span) | Ramp samples (event time: value) | Controllers (all others 0) |
|---|---|---|---|---|
| Engineer | 30.0 | `defaultFace` (0–30.0) | 0.194: 0.980, 29.680: 0.980 | `defaultFace` 0.980 at the samples; clamped to **1.0** from about 1.45 s to 28.42 s (u from 0.0426 to 0.957) |
| Soldier | 15.0 | `defaultFace` (0–15.0) | 0.253: 0.733, 14.771: 0.733 | preset `defaultFace` 1, lids −0.392 each, `multi_CloseLid` −0.656, times I: I = 0.733 at the samples, 0.825 mid-scene (7.5 s): `defaultFace` 0.825, lids −0.323, multi −0.541 |
| Demoman | 15.0 | `defaultFace` (0–15.0) | 0.306: 1, 14.473: 1 | `defaultFace` 1 (clamped) from 0.306 to 14.473 |
| Heavy | 15.0 | `idleface` (0–15.0) | 0.335: 1, 14.501: 0.992, 15.0: 0 | I ≥ 1, clamped to 1, from 0.335 to just before 14.501 (the slid 15.0 sample makes the middle span bulge strongly): `idleface` 1, lids −0.58 each, `multi_CloseLid` +0.62 |
| Sniper | 15.0 | `defaultFace` (0–15.0) | 0.338: 0.514, 14.556: 0.514 | preset as the Soldier's; I = 0.514 at the samples, 0.578 mid-scene: `defaultFace` 0.578, lids −0.227, multi −0.379 |
| Spy | 15.0 | `defaultFace` (0–15.0) | 0.306: 1, 14.473: 1 | from 0.306 to 14.473 (clamped at 1): `defaultFace` 1, lids −0.962 each, `multi_CloseLid` −0.656 |
| Scout | 16.627 | `madUpper` (0.013–16.627) | varies, below | `madUpper` I, lids −0.5·I each, `multi_CloseLid` −0.46·I |
| Medic | 15.0 | `default` (0–15.0) | varies, below | `madUpper` I (the preset also lists `AE`, `UH` at 0) |
| Pyro | – | none | – | everything 0 (one dummy controller) |

The two varying ramps (event-local time: sample value; the curve of
§4.1 runs through them and overshoots between them):

- Scout: 0.62: 0.54, 1.99: 0.55, 2.45: 0.27, 3.43: 0.24, 3.89: 0.49,
  4.30: 0.35, 4.84: 0.27, 7.15: 0.27, 7.79: 0.48, 8.30: 0.13, 8.95: 0.13,
  10.68: 0.12, 11.21: 0.42, 11.73: 0.18, 13.08: 0.07, 13.72: 0.10,
  14.06: 0.37, 14.55: 0.15, 15.62: 0.13 (event length 16.614).
- Medic: 0.29: 0.33, 1.97: 0.31, 2.40: 0.62, 3.10: 0.38, 5.41: 0.27,
  5.93: 0.57, 6.67: 0.39, 7.56: 0.42, 10.74: 0.29, 11.32: 0.57,
  12.54: 0.37, 14.68: 0.33 (event length 15.0).

The Scout's and the Medic's idle faces are upper-face only: their mouths
stay at base. The Engineer's lower face is the only thing his idle sets.

### 6. Other idle states

Values are the presets' non-zero weights; multiply by the intensity.
When one scene has several expression events active at once they blend
in turn by §4 step 2 (order: Open questions 2).

| Class | Hurt / round-lost scene (length) | Events: preset (span) intensity ≈ | Preset weights |
|---|---|---|---|
| Scout | `idleloopHurt01` (16.627) | `painBigUpper` (0.013–16.627) 0.73–1.0; `painBig` (same span) 0.06–0.44 | `painBigUpper`: lids −0.134 each, `painBigUpper` 1; `painBig`: lids −0.134 each, `painBig` 1 |
| Soldier | `idleloopHurt01` (15.0) | `painSmall` (0–15.0) 0.74–1.0; `painBig` (0.857–6.060, 7.369–13.548) 0.15–0.35 | `painSmall`: lids −0.422 each, multi −0.326, `painSmall` 1; `painBig`: lids −0.134 each, `painBig` 1 |
| Demoman | `idleloopHurt01` (14.569) | `pain` (0.028–14.569) 0.66–1.0 | lids −0.57 each, `pain` 1 |
| Heavy | `idleloopHurt01` (15.056) | `upset1` (0.028–15.056) ≈1.0; `upset2` (0.806–3.472) 0.65 and (7.181–11.125) ≈0.84; `painBig` (2.958–4.528) 0.33 and (10.528–12.597) ≈0.45 | `upset1`: lids −1 each, multi −0.444, `upset1` 1; `upset2`: all three lids −1, `upset2` 1; `painbig` (matched case-insensitively): lids −0.72 each, `painbig` 1 |
| Engineer | `idleloopHurt01` (9.196) | `painSmall` (−0.021–9.196) 0.42–1.0; `painBig` (3.061–4.712) 0.53 | `painSmall` 1; `painBig` 1 |
| Medic | `idleloopHurt01` (15.0) | `pain` (0–15.0) 1.0 | `painBigUpper` 1 (lids listed at 0) |
| Sniper | `idleloopHurt01` (15.0) | `painSmall` (0–15.0) 0.83–1.0; `painBig` (1.542–3.931, 7.972–11.444) 0.18–0.29 | as the Soldier's presets of the same names |
| Spy | none: the normal idle | – | – |

- **Competitive winner** (`comp_winner_idle_face`, about 29.1–29.5 s, a
  loop marker at 29.067): one expression at intensity 1 for the whole
  scene, no ramp: Scout, Soldier, Engineer, Spy `happySmall`; Demoman
  `drunkHappy`; Heavy `happysmall02` (with a ramp, 0.545–0.549); Medic
  `happy`; Sniper `Cocky`.
- **Scoped Sniper** (`idleloopDeployed01`, 12.0 s): its one event asks
  for a preset `Squint`, which the Sniper's expression file does not
  have (the model has a `Squint` controller, the file does not). The
  event does nothing: **a scoped Sniper shows the base face** for the
  scene's 12 s, then the choice repeats.
- **Firing Minigun** (`attackMinigun02`, 19.801 s): `mad` (0–13.153,
  ≈0.84–1.0), `happysmall02` (10.829–19.801, ≈0.8–1.0), `happyBig`
  (9.653–13.194, 0–0.27).

### 7. Firing faces (for completeness)

On each shot (any class but the Pyro, whose rules give none) the server
asks `TLK_FIREWEAPON`, at most once per max(5 s, the last firing
scene's length): `scenes/player/<class>/low/attack01.vcd` (the
Engineer's: `mad` from 0.01 to 1.99 s, peaking at 1.0). It plays **on
top of** the idle scene, which keeps running; while one is due the idle
re-pick waits. Several melee and voice rules also answer this concept;
out of scope.

### 8. Blinking

TF2 players **do not blink**. The client's blink timer for players is
disabled, and the blink toggle is not networked for them. The `blink`
controller that seven classes carry therefore stays 0 (its scene
tracks, if any, aside); eyelids move only through the `*_CloseLid`
weights of §5–§6. The engine's blink shape (`blink_duration`, HL2:DM
player_models.md §11.2) never triggers.

### 9. Eye aim and head (clients)

Each client frame, for each player:

1. **Look target**: if `tf_clientsideeye_lookats` is 1, go through the
   other players in index order (the player itself excluded; the local
   player included as a target for others); skip dead ones, those whose
   origin is more than 300 u away (3D), and those behind (dot of this
   player's forward, from its entity angles as in HL2:DM §6.4, with the
   unit direction < 0). The first that remains gives the target: its eye
   position. Otherwise the target is origin + 512·forward.
2. The eyes aim at it exactly as HL2:DM §11.3: through the `eyes`
   attachment, x at least 6, the 30° cone (the class models leave the
   limit unset, so cos 30° = 0.866 applies), `eyes_updown` /
   `eyes_rightleft` as deflection (0 at rest: no scene here moves them).
   The Engineer has no `eyes` attachment and no eyeballs, the Pyro no
   eyeballs: nothing to aim. The Demoman aims his one eye.
3. **No head turning and no blink step**: unlike HL2:DM §6.4 steps 3–5,
   TF2 sets no head pose and toggles no blink (the class models have no
   head pose parameters anyway, player_animation.md "Pose parameters").

Differences from HL2:DM: radius 300 instead of 128; dead players are
not looked at; a cvar can turn it off; no head yaw/pitch; no blink.

### 10. Dead players and ragdolls

- Death plays no face: no rule answers the death concept, and death does
  not stop the idle scene. The dead player's idle scene plays on to its
  end (up to 30 s for the Engineer, 15–16.6 s for the others; the
  hurt scene if the killing blow crossed a quarter health and the
  re-pick happened in time) and is not restarted (re-picks need a living
  player). After that the dead player's face is base.
- The class ragdoll's face: **while its owner exists and is dead**, the
  ragdoll draws the owner's current face, computed exactly as for the
  living player (§4, the still-running scene included) and the owner's
  eye target. **When the owner is alive again, or gone**, the ragdoll
  uses its own controllers, which no scene drives: in practice the base
  face (Open questions 3).
- Feign death (Dead Ringer): the player stays alive, so his idle face
  keeps cycling; the fake ragdoll mirrors only while the owner counts as
  dead, i.e. it shows its own (base) face (Open questions 4).

### 11. Every class alike, and the Pyro

The mechanism is the same for all nine classes; only the scenes and
presets of §5–§6 differ, and the Spy has no hurt face. The Pyro has no
idle rule at all: every attempt fails and is retried every U(30, 40) s,
his single `dummyBlend` controller stays 0, and the masked head shows
its base mesh. He has no eyeballs, so the look target has no visible
effect.

### 12. The data, for loading it at run time

- **`scenes/scenes.image`** (in `tf2_misc_dir.vpk`): little-endian.
  Header: magic `VSIF`, version (2 or 3), scene count, string count,
  offset of the entry table. After the header, one 32-bit offset per
  string into a NUL-terminated string pool. Each entry (16 bytes): the
  CRC-32 of the scene's path normalized to lower case with backslashes
  (e.g. `scenes\player\engineer\low\idleloop01.vcd`), data offset, data
  length, summary offset. Data that starts with `LZMA` is compressed:
  then uncompressed size (32-bit), compressed size (32-bit), the 5
  LZMA property bytes, and the raw LZMA stream.
- **A binary scene**: magic `bvcd`, version byte (4 here), a 32-bit CRC
  to skip; then the scene-level events (count byte, events), the actors
  (count byte; each: name, channel count byte; each channel: name, event
  count byte, events, active byte; actor active byte), the scene ramp
  (count byte; each sample a 32-bit float time and an intensity byte),
  and an "ignore phonemes" byte. Names and parameters are 16-bit indices
  into the image's string pool.
- **An event**: type byte (2 = expression, 5 = speak, 6 = gesture,
  10 = flex animation, 12 = loop, …), name, start and end (floats), three
  parameters (for an expression: the expression file name, the preset
  name, unused), its ramp (as the scene ramp), a flags byte (bit 3 =
  active), a float, then three tag lists (relative and timing tags: name
  plus a byte each; then two absolute-tag lists: name plus a 16-bit
  value each), a float more for gestures only, an optional tag (a byte;
  if 1, two names), its flex animation tracks (count byte; each: name,
  flags byte (bit 0 active, bit 1 combo), min and max floats, a 16-bit
  sample count with (float time, intensity byte, 16-bit curve type)
  each, and for combo tracks a second such list), a loop-count byte for
  loop events, and for speak events a caption-type byte, a name and a
  flags byte.
- **A `.vfe` expression file**: magic `EFV\0`, version, a 64-byte name,
  length, preset count, preset table offset, name offset, index count,
  index table offset, key count, key-name offset table offset, key
  mapping offset. Each preset (24 bytes): name offset (relative to the
  preset), two unused fields, weight count, an index, unused, weight
  array offset (relative to the preset). Each weight (12 bytes): key
  index (into the file's key names = controller names), weight (float),
  influence (float).

## Per-frame order

Per player, on each client frame:

1. Look target (§9 step 1).
2. Controllers to base, real range (§2, §4 step 1); flex-animation
   tracks of running scenes, if any, write here.
3. Expression events of running scenes blend in (§4 step 2).
4. Visemes from speech (out of scope).
5. Flex rules; eye aim (§9 step 2).

On the server, each think: the idle re-pick check (§3).

## Edge cases

- Health exactly a quarter of max (possible when max health is a
  multiple of 4, e.g. Soldier 200, Heavy 300): damage to exactly 50 HP
  (Soldier) restarts the idle face, but 50/200 = 0.25 is not < 0.25, so
  the normal idle is picked again. Below that, hurt.
- The idle face restarts on every idle scene end with a fade to 0 and a
  fade back in: for about 0.3–0.5 s every 15 s (30 s for the Engineer)
  the face passes near base (the Engineer's mouth hangs open briefly).
- Respawn: the client resets the controllers and drops scene events; the
  server re-picks on the first think, so the new face fades in within
  the first fraction of a second after the scene arrives.
- Taunting does not stop the idle scene; the taunt's own scene blends
  on top (out of scope).
- A disguised Spy: the idle choice uses his real class (Spy scenes and
  the Spy's presets), applied by controller name to whatever model he
  is drawn with (Open questions 5).

## Quirks

- The Engineer's model at base has a slack, open mouth; it is the
  `defaultFace` controller (about 1.0), set by the idle scene, that closes
  it. Rendering the base mesh is wrong for every class but the Pyro.
- A scoped Sniper's idle scene names a preset his file lacks, so scoped
  Snipers show the base face (§6).
- After a Heavy's first Minigun wind-up the idle face stops cycling for
  the rest of that life unless he drops below a quarter health (§3.3).
- Hurt faces outlast healing (no re-pick on the way up).
- The Spy has no hurt and no round-lost face.

## Test cases

1. Living Engineer, full health, idle: `defaultFace` is 0.980 at 0.194 s
   and at 29.68 s into his scene, exactly 1.0 (clamped) from about 1.45 s
   to 28.42 s, between 0 and 0.98 in the first 0.194 s and the last
   0.32 s, and 0 at 30.0 s; every other controller stays 0; the next
   scene starts and it climbs back.
1b. Ramp curve, two equal samples v at a and b, edges 0: at u = 0.25 the
   intensity is v·1.09375, at u = 0.5 v·1.125 (both clamped to 1).
2. Living Spy, idle, mid-scene: `defaultFace` 1, `right_CloseLid` =
   `left_CloseLid` = −0.962, `multi_CloseLid` = −0.656.
3. Soldier, 7.5 s into his scene: `defaultFace` 0.825, lids −0.323,
   multi −0.541; at 0.253 s: 0.733, −0.287, −0.481.
4. Heavy, mid-scene: `idleface` 1, lids −0.58, `multi_CloseLid` +0.62.
5. Sniper scoped with a sniper rifle at the re-pick: no controller
   changes from base during the 12 s scene.
6. Pyro: no idle scene ever; `dummyBlend` 0; retries spaced 30–40 s.
7. Engineer damaged from 125 to 30 HP (boundary 31): the idle face is
   replaced at once by `idleloopHurt01` (`painSmall` ≈0.85 shortly
   after start). Healed to 125: `painSmall` continues until the 9.196 s
   scene ends, then `defaultFace` returns.
8. Soldier damaged from 120 to exactly 50: the face restarts with the
   normal idle (not hurt).
9. Blink: over 60 s of any idle player, `blink` stays 0.
10. Eye target: two players 250 u apart facing each other → each looks
    at the other's eye position; at 350 u → each looks 512 u ahead; a
    dead player at 100 u in front is ignored.
11. Kill an Engineer 5 s into his idle scene: his ragdoll keeps
    `defaultFace` at 1.0 until about 28.4 s into the scene (0.98 at
    29.68 s), fades to 0 at
    30 s and stays at base; when he respawns, the ragdoll shows base.
12. Heavy winds up the Minigun 3 s into a 15 s idle scene and keeps
    full health: from 15 s on his face is base until he respawns.

## Open questions

1. The exact delay between an idle scene's end and the next one's start
   on a client (one server think plus the network delay for the new
   scene entity) was not measured; the dip of the Engineer's face
   between loops depends on it.
2. When several expression events of one or several scenes are active,
   the order in which they blend (§4 step 2) was not settled: assumed in
   the order they started (the idle scene's first, a later firing scene
   on top).
3. The ragdoll's own controller values after its owner respawns: for a
   remote player the ragdoll copies the player's client-side history
   (base, real 0); for the local player's own ragdoll the stored values
   may be all-zero normalized, which would decode to each range's
   minimum (lids −1, eyes −45). Not settled; base is the safe choice.
4. Which face a Dead Ringer's fake ragdoll shows was inferred (owner
   alive → its own controllers), not checked in a game.
5. A disguised Spy drawn with another class's model: the Spy presets
   are applied by controller name to that model (so `defaultFace` and
   the lids carry over where the names match); not checked in a game.
6. The dead player's eye target while dead (it keeps being computed from
   the dead player's origin and angles, which follow the spectator
   state) was not followed through; a ragdoll's eyes may look somewhere
   odd while mirroring.
