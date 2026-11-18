# Team Fortress 2: ragdolls — fade, max count, lifetime

Source basis: the SDK's TF2 game code (Valve's Source SDK 2013 with
TF2's client and server code, read in place on the Linux box, nothing
copied): how the server makes and replaces a dead player's body entity,
what the round cleanup keeps, how the client builds the physics body from
it and decides when to hide it (its per-frame think, the visibility test,
the optional forced fade), the special bodies (death animations, ice,
gold, ash, dissolve, cloaked corpse, decapitation, feign death); the
shared Source ragdoll code for the generic client ragdoll fade queue
(`g_ragdoll_maxcount` and friends) and the map's ragdoll-manager entity.
The retail client install (app 440, PatchVersion 11191711), read in
place: its client library still registers `cl_ragdoll_fade_time`,
`cl_ragdoll_forcefade`, `cl_ragdoll_physics_enable`, `g_ragdoll_maxcount`
and `g_ragdoll_fadespeed`; no shipped `.cfg` or `dxsupport*.cfg` file
sets any ragdoll variable. No game was launched.
Status: draft (checked against the SDK's game code, 2026-11-18)

## Summary

When a TF2 player dies without gibbing, the server makes one networked
**body entity** per player (sent to every client, never culled); each
client builds its own physics ragdoll from it. There is **no count cap**
on player bodies and **no visible alpha fade** by default: a client keeps
a body while it is being looked at, and **removes it instantly** the
first frame it is out of view after a timer has run out. The timer is
`cl_ragdoll_fade_time` (15 s) from creation, but every frame the body is
in view resets it to 33 % of that (4.95 s) from now — so a body glanced
at once disappears about 5 s after it was last seen, one never seen
lasts 15 s. Independently of that, the body is deleted for everyone the
moment its owner dies again, disconnects, or the round restarts.
Respawning does not remove it. Only with `cl_ragdoll_forcefade 1` does a
body visibly fade (600 alpha per second, about 0.43 s).

## Units and conventions

- Time in seconds. The client's body logic runs **once per rendered
  frame** (frame time `dt`), not per server tick; the server tick is
  1/66.67 s (15 ms).
- Alpha in integer steps 0–255 (255 opaque).
- Distances in Hammer units (u).
- "In view" has the specific meaning given under "Visibility test".
- `F` below means the current value of `cl_ragdoll_fade_time`.

## Constants

| Name (ours) | Value | Unit | Meaning |
|---|---|---|---|
| `cl_ragdoll_fade_time` (F) | 15 | s | body lifetime timer from creation; client-only, no archive/cheat flag |
| view-reset fraction | 0.33 | × F | each in-view frame sets the timer to `now + 0.33·F` (4.95 s at F = 15) |
| `cl_ragdoll_forcefade` | 0 | bool | 1 = fade bodies out visibly as soon as they are seen; client-only |
| forced fade speed | 600 | alpha/s | hard-coded for player bodies (not read from `g_ragdoll_fadespeed`) |
| `cl_ragdoll_physics_enable` | 1 | bool | 0 = no physics bodies (no ragdoll simulation) |
| visibility box half-size | 1 | u | the in-view test uses a 2×2×2 box at the body's origin |
| decapitation minimum F | > 5 | s | decapitation / head-smash bodies only when F > 5 |
| late-arrival window | 1.0 s = 67 ticks | ticks | no body is built if the owner's player entity was created on this client fewer than 67 ticks before (see Edge cases) |
| death-animation chance | 0.25 | – | fraction of eligible kills that play a death animation first |
| ice freeze-in | U(0.1, 0.75) | s | ice body plays the death pose before freezing |
| ice statue hold | U(9, 11) | s | ice body stays rigid and still, then falls as a stiff ragdoll |
| ash delay | 0.3 | s | "become ash" bodies are hidden this long after the first think |
| bombinomicon delay | 0.3 | s | bodies of a bombinomicon wearer burst into gibs after this |
| charged-plasma gib dissolve | 1.2 | s | gibs of a charged-plasma kill dissolve after this |
| cloaked corpse fade-in | 1.0 | s | cloaked-corpse invisibility goes 0 → 1 at `dt` per frame |
| dropped-wearable fade | 15 | s | hats etc. dropped from a body use their own 15 s fade (gib rules) |
| `g_ragdoll_maxcount` | 8 | bodies | generic client ragdoll queue; replicated; **not used for TF2 player bodies** |
| `g_ragdoll_important_maxcount` | 2 | bodies | generic queue, "important" bodies; replicated; not used for players |
| `g_ragdoll_fadespeed` | 600 | alpha/s | generic queue fade speed; client; not used for players |
| `g_ragdoll_lvfadespeed` | 100 | alpha/s | generic queue fade speed in low-violence mode; not used for players |

## Behavior

### 1. Body or gibs, and when the body entity is made

- On death the server decides gib vs body (gib rules are spec'd
  elsewhere; in short: `tf_playergib` 0 never, 2 always, 1 = only blast
  or half-falloff damage that is critical or leaves health ≤ −10, plus
  weapon attributes). A gibbed death still makes the body entity, flagged
  "gib": clients spawn gibs from it and hide the body at once.
- A non-gibbing blast kill multiplies the body's launch force by 2.5 on x
  and y, 2 on z.
- The body entity is made in the death handling, on the same server tick
  as the death. Exactly one **real** body handle is kept per player.
  Making a new one first **deletes the previous one** (for every client,
  instantly, however it looked at the time).
- A Dead Ringer feign death makes a separate **feign body** with its own
  per-player handle; a new feign body deletes the previous feign body but
  does not touch the real body, and a real death does not touch the feign
  body. So a player owns at most 2 bodies (1 real + 1 feign).
- The player model itself is hidden and made non-solid when the body is
  made.

### 2. When the body entity goes away (server, for everyone)

The body entity has no timer of its own. It is deleted only when:

1. its owner dies again (real body) or feigns again (feign body) —
   replaced, see 1;
2. its owner's player entity is destroyed (disconnect, kick, map change):
   both the real and the feign body are deleted;
3. the round is cleaned up (round restart, mini-round start, any full map
   reset): every entity not on the keep-list is deleted, and bodies are
   not on it;
4. the level ends.

It is **not** deleted when the owner respawns, changes class while dead,
or joins spectator. Changing team while alive kills the player first, so
that makes a new body which replaces the old one (rule 1). When the
entity is deleted the client's ragdoll goes with it immediately: no
fade.

### 3. Client life of a body (default, `cl_ragdoll_forcefade 0`)

Each client keeps its own **removal time** `R` for each body, and
evaluates it every rendered frame:

- On creation (the client receives the new body entity): `R = now + F`.
- If the body opens with a death animation (see 6), when that animation
  finishes and the physics ragdoll starts: `R = now + F` again
  (overwrites, may extend).
- Every frame, in this order:
  1. special-body handling (6), which may end the frame early;
  2. if the body is **in view**: `R = now + 0.33·F` (overwrites — this
     can make `R` *earlier* than the creation timer), and nothing else
     this frame;
  3. else if `R < now` (strict): the body is removed **instantly** —
     physics freed, not drawn, particles stopped, attached wearables
     hidden. No alpha change ever happens in this mode.

Consequences:

- Never in view: removed on the first frame with `now > t_created + F`.
- In view at least once: removed on the first frame that is out of view
  **and** later than `t_last_in_view + 0.33·F`. With F = 15 that is
  4.95 s after the last frame it was seen, even if that is well before
  15 s after death.
- In view continuously: never removed by the client; only server rules
  (2) end it.

A client-removed body's entity keeps existing (invisible) until a server
rule deletes it; it still counts as the owner's body for rule 1.

### 4. Visibility test

"In view" for this purpose means: an axis-aligned box ±1 u around the
body's current origin (the physics root) lies in a map cluster visible
from the local view (the potentially-visible set), **and** that box is
inside the local view frustum. There is no line-of-sight trace and no
distance limit: a body behind a wall but in a visible cluster and in
front of the camera counts as seen. The view is the local client's
render view, including spectator and death cams (a dead player watching
their own body keeps it alive).

### 5. Forced fade (`cl_ragdoll_forcefade 1`)

- The creation and death-animation timers are not applied (`R` stays at
  its initial value, effectively −∞).
- Each frame, in this order after the special-body handling:
  1. if the body is already **fading**: `alpha = trunc(max(alpha − 600·dt, 0))`
     (alpha is an integer, so from an integer alpha this is a drop of
     `ceil(600·dt)`), draw translucent; when alpha reaches 0, remove; end
     the frame. Fading continues whether or not the body is in view.
  2. else if in view: mark it fading (the alpha drop starts **next**
     frame) and clear its decals; end the frame.
  3. else (`R < now` always holds): remove instantly.
- So a body first seen fades from 255 to 0 in `ceil(255 / ceil(600·dt))`
  frames after the frame it was first seen (≈ 0.43 s at most frame
  rates); a body out of view on its first think vanishes at once.

### 6. Special bodies (lifetime notes only)

- **Death animation**: on certain kills (headshot, backstab, burning,
  some taunt kills…) 25 % of the time (always for ice bodies and some
  taunt kills; never in mid-air for most), the body first plays an
  animation from the player's pose, then becomes a physics ragdoll; at
  that moment `R` is reset to `now + F` (3). The view rule applies
  during the animation too.
- **Ice** (freeze attribute): the death pose plays for U(0.1, 0.75) s,
  then the body is a rigid statue for U(9, 11) s, then a stiff ragdoll.
  No timer reset at the switch; the normal rules (3) still run during the
  statue, so an unseen statue can vanish at `t_created + F`. In mid-air
  it goes rigid at once.
- **Gold**: different material, no death animation; lifetime unchanged.
- **Become ash**: hidden 0.3 s after its first think; from then on the
  think returns early every frame, so the hidden body is never "removed"
  by the client — it lingers invisibly until a server rule (2).
- **Dissolve (plasma kills)**: a dissolve effect starts on the first
  think: effect fades in over 1.0 s, the model fades out between 1.9 s
  and 2.0 s after the start. A charged-plasma kill gibs instead and its
  gibs dissolve 1.2 s later.
- **Bombinomicon**: the body bursts into gibs 0.3 s after its first
  think.
- **Cloaked corpse** (disguise-on-backstab knife): invisibility rises by
  `dt` per frame to 1 (fully cloaked after 1 s); timers unchanged.
- **Decapitation / head smash**: only when F > 5; the head is shrunk
  away and a head gib is spawned (gib rules). Lifetime unchanged. With
  F ≤ 5 the body is a normal one with its head.
- **Feign death body**: same client rules as a real body; its server
  life is rule 1/2 with the feign handle.

### 7. The generic ragdoll queue (not used for TF2 player bodies)

Source's shared code keeps a client-side "least recently made" queue for
client-only ragdoll copies (NPC-style ragdolls). TF2 player bodies never
enter it, so `g_ragdoll_maxcount` does **not** cap them. For
completeness: the cap is `g_ragdoll_maxcount` (8), or a map
`game_ragdoll_manager` entity's `MaxRagdollCount` (its
`MaxRagdollCountDX8` instead when `mat_dxlevel` < 90 and that value is
≥ 0; −1 = use the cvar). Low-violence mode sets the cap to 0 on the
client (the server ignores low violence in multiplayer). Each frame,
while over the cap, it removes the first queued body that is out of view
(else the oldest one that is not burning/dissolving); a client removal
is a fade at `g_ragdoll_fadespeed` (600 alpha/s; 100 in low violence),
a server removal a fade starting at once. "Important" bodies have their
own queue of 2.

## Per-tick order

Server, on the death tick: decide gib vs body → delete the owner's
previous body (if any) → make the new body entity → hide the player.
Round cleanup and disconnect delete bodies on the tick they happen.

Client, per body, per rendered frame:

1. if the body entity was deleted by the server: the ragdoll is gone
   (no further steps);
2. special-body handling (6): dissolve/ash/bombinomicon/charged-plasma
   timers, cloak fade-in; may end the frame;
3. forced-fade alpha step if fading (5.1);
4. in-view test (4): if in view, reset `R` (default) or start fading
   (forced) and end the frame;
5. if `R < now`: remove instantly.

## Edge cases

- **Late arrival**: when a client receives a new body entity and the
  owner's player entity was created on that client fewer than 67 ticks
  (1 s) earlier — or, if the owner is unknown, the local player's entity
  was — no ragdoll or gibs are built at all (this suppresses duplicates
  after a full update; it also means a player who just came into your
  potentially-visible set and dies within 1 s leaves no body for you).
  Demo playback after skipping suppresses bodies made during the skip.
- **Frame-rate dependence**: removal happens on frame boundaries, so the
  removal time is the first frame time past `R`; the forced fade's
  per-frame truncation makes it slightly faster than 255/600 s when
  `600·dt` is not an integer.
- **Changing F at run time** affects only later timer writes; a body
  already timed keeps its `R` until its next reset.
- **F ≤ 0**: unseen bodies vanish on the first frame; seen ones on the
  first frame after leaving view.
- `cl_ragdoll_physics_enable 0`: bodies are not simulated (they stay
  where the model was); timers are unchanged.

## Quirks

- **Looking shortens life**: the in-view reset overwrites rather than
  extends, so glancing at a fresh body cuts its 15 s life to ~4.95 s
  after the glance. Keep (it is how the game behaves).
- **No visible fade by default**: bodies pop out of existence, but only
  while out of view, so players rarely see it. Keep.
- **Bodies in view live forever**: staring at a body (or standing where
  its cluster is visible with it on screen) keeps it until the owner dies
  again or the round restarts. Keep.
- **Ash bodies never get removed** client-side, only hidden. Harmless.
- `g_ragdoll_maxcount` looks like it should limit bodies in TF2 but does
  not. An implementation should not add a cap for parity.

## Test cases

All times in seconds; the client frame clock ticks at `dt = 1/64` from
t = 0 (frames at k/64), the body is created at t = 0 and its first
evaluation is the frame at t = 0, F = 15, forcefade 0 unless stated.
"Present" means the body exists and is drawn.

| # | Setup | Input | After | Expected |
|---|---|---|---|---|
| T1 | one body, never in view | — | t = 15.0 | present (15.0 < 15.0 is false), alpha 255 |
| T1b | same | — | t = 15.015625 | removed (first frame with R < now) |
| T2 | in view every frame for t ≤ 2.0, then never | — | t = 6.9375 | present (R = 6.95) |
| T2b | same | — | t = 6.953125 | removed |
| T3 | in view for t ≤ 10.0, then never | — | t = 14.953125 | removed (R = 14.95); present at 14.9375 |
| T4 | in view every frame | — | t = 60 | present, alpha 255 |
| T5 | owner respawns at t = 3, body never in view | — | t = 14 | present; removed at 15.015625 (respawn changes nothing) |
| T6 | body present, in view | owner dies again at t = 30 | t = 30 | old body removed at once; exactly one body for this owner (the new one, R = 45) |
| T7 | 3 owners' bodies present | round restart at t = 5 | t = 5 | 0 bodies |
| T8 | owner has a real and a feign body | owner disconnects at t = 5 | t = 5 | both removed; other owners' bodies unaffected |
| T9 | 24 owners each die once at t ∈ [0, 2], all bodies in view | — | t = 3 | 24 bodies present (no cap; `g_ragdoll_maxcount` 8 does not apply) |
| T10 | one owner dies at t = 0, 5, 10 | — | t = 11 | 1 body for that owner (made at t = 10) |
| T11 | Spy feigns at t = 0, really dies at t = 3 | both in view | t = 4 | 2 bodies (feign + real) |
| T11b | same, feigns again at t = 20 | — | t = 20 | first feign body removed; 2 bodies (new feign + real) |
| T12 | owner alive, switches team at t = 10 | — | t = 10 | old body replaced by a new one (team switch kills) |
| T13 | death animation of length 2.0 s, never in view | — | t = 17.0 / 17.015625 | present / removed (R reset to 2.0 + 15) |
| T14 | forcefade 1, body in view at t = 0 | — | per frame | alpha 255 at t = 0; 245 at 1/64 (255 − 9.375 truncated); then −10 per frame; 5 at 25/64 |
| T14b | same | — | t = 26/64 = 0.40625 | alpha 0, removed |
| T15 | forcefade 1, in view at t = 0, out of view from t = 0.1 | — | t = 0.40625 | removed (fade continues out of view) |
| T16 | forcefade 1, never in view | — | t = 0 | removed on the first evaluation |
| T17 | forcefade 1, `dt = 1/128`, in view at t = 0 | — | t = 51/128 = 0.3984375 | removed (step ceil(4.6875) = 5, 51 frames); alpha 5 at 50/128 |
| T18 | F = 5, never in view | — | t = 5.015625 | removed; a decapitation kill shows no decapitation (needs F > 5) |
| T19 | F = 5, in view for t ≤ 1.0 | — | t = 2.65625 | removed (R = 1 + 1.65 = 2.65) |
| T20 | owner's player entity created on this client at tick N | body arrives at tick N + 66 / N + 67 | — | no ragdoll built / ragdoll built |
| T21 | ice body, freeze-in 0.5, hold 10, never in view | — | t = 15.015625 | removed (removal timer not reset by the statue) |

## Open questions

- Whether the body's first evaluation happens on the frame it is
  received or on the following frame (the tests assume the same frame);
  only matters for T16 and frame-exact T1b-style checks.
- The retail defaults of `cl_ragdoll_fade_time` (15) and
  `cl_ragdoll_forcefade` (0) are from the SDK; the retail library has the
  names but its defaults were not read. Check with `help
  cl_ragdoll_fade_time` in the live game.
- The exact set of kills eligible for a death animation and their
  animation lengths (per class model) is not listed here; the lifetime
  rule only needs "R reset when the animation ends".
- What happens if the client timer removes a body while its death
  animation is still playing (the animation's end may build physics on a
  removed body); not worked out.
- Plasma-dissolve bodies: whether the dissolve effect itself deletes the
  body after 2 s or only hides it was not followed into the dissolve
  effect code.
- Float rounding: `600·dt` for `dt = 1/60` lands within float error of
  10, so the step may come out 10 or 11 in practice; the tests use
  binary-exact frame times to avoid this.
