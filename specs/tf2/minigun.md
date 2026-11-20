# Team Fortress 2: the Minigun's states, sounds and barrel spin

Source basis: Valve's Source SDK 2013 (2025 TF2 release) read in place in
a spec session: the Minigun's weapon code (states, its attack and idle
handling, its sound selection, its barrel), the shared weapon base (how
held buttons reach the attack, empty and idle handling, the idle time an
animation sets, the empty auto-switch), the TF weapon base, the game
rules' weapon choice, the player animation events, and the sound
envelope controller the Minigun plays through. TF2's
`scripts/game_sounds_weapons.txt` and the six `sound/weapons/minigun_*.wav`
files read in place with our dumper (chunk layout and lengths only), plus
a scan of the 247 TF2 WAVs whose names contain "loop" (which loop chunk
each carries). Measured on 2026-11-20 against real TF2 (client build
11191711, listen server, `host_framerate 0.015` so one frame = one
0.015 s tick, `sv_soundemitter_trace 1`, a puppet bot with the stock
Minigun mimicking our buttons, heard by our client): every sound start
and stop with its tick. The empty auto-switch's timing (2026-11-20):
the SDK's sequence-finished rule and per-tick order, the Heavy arms'
`m_spool_down` sequence data, and a per-tick measurement of the local
player's stock Minigun. Nothing copied; restated in our words.
This continues [weapons.md](weapons.md) §7 (damage, spread, rev-up
penalty, speed) and [viewmodel.md](viewmodel.md) §7 (first-person
sequences); where they disagree, this file is the newer word (the
corrections are listed below). How Source decides that a WAV loops, for
every game, is in [../source/sound_looping.md](../source/sound_looping.md).
Status: draft (checked against the SDK and measured)

## Summary

The Minigun is a five-state machine: idle, winding up, spinning (attack2),
firing, and dry (out of ammo, still trying). Exactly **one** Minigun sound
plays at a time, chosen by the state alone; on a state change the old
sound is cut at once and the new one starts from its beginning. Four of
the six WAVs loop by themselves (a cue point at sample 0: the whole file
repeats seamlessly, the game never re-triggers them); the wind-up and
wind-down are one-shots that are always cut early: the wind-up at 0.75 s
of its 0.995 s, the wind-down **when the barrel stops turning**, which is
3.0 s after a full spin but only as long as the barrel had spun up after
a short tap. The sounds are not sent by the server: every client derives
them from the gun's networked state. An empty Minigun cannot spin at all
(attack2 is refused), keeps playing the empty loop as long as any button
is held after running dry, and auto-switches away 1.155 s (77 ticks)
after the wind-down began (§3.3).

What players notice when it's wrong: the fire loop going silent after
4 s (its WAV is 4.0 s long and must loop), the empty click playing once
instead of looping, the wind-down always playing its full 3.33 s, and the
spin releasing to idle at once (it takes 0.3 s).

## Corrections to the earlier TF2 specs

- weapons.md §7.1 "Stopping … at once when it was only spun" -> 0.3 s
  after attack2 is let go (the spin-idle animation's length; §2.4).
  Same for its test 35.
- weapons.md §7.1 "Dry: … while attack is held" -> while attack **or
  attack2** is held; with ammo 0, attack2 alone never starts a spin
  (§3).
- weapons.md §8.3 "the minigun loops on the static channel" -> the fire,
  crit, spin and empty loops are on the static channel; the wind-up and
  wind-down are on the weapon channel; spin and empty are 75 dB, the rest
  94 dB (§4.1).
- weapons.md §8.3 / §7.1 sound of idle: "wind-down sound while the barrel
  still turns" stays right; added: it is cut the moment the barrel stops
  (§4.3).
- viewmodel.md edge case "Wind-up interrupted: the wind-down follows at
  once" -> the wind-up always completes (0.75 s), then the wind-down
  starts (§2.3; measured).

## Units and conventions

- Time in seconds; server tick 0.015 s (66.67 Hz). Timers are absolute
  times; something due at T happens on the first tick with time ≥ T.
  "Tick n" counts from the tick a button is pressed (tick 0).
- A = attack (primary, mouse 1) held, B = attack2 (mouse 2) held, a =
  the Heavy's Minigun ammo (one pool, 200 at most, no clip).
- Angles in radians; the barrel's spin speed in rad/s.
- Sound level in dB as the sound script gives it (75 dB = "normal").

## Constants

| Name (ours) | Value | Unit | Meaning |
|---|---|---|---|
| wind_up | 0.75 | s | press in idle to spun/firing (50 ticks) |
| fire_step | 0.1 | s | from entering firing (or spinning) to the next attack time; the first shot of a held press is at tick 57 (0.855 s) |
| fire_interval | 0.105 | s | between shots (7 ticks) |
| fire_idle_delay | 0.2 | s | after the latest shot before a released gun winds down |
| spin_idle_anim | 0.3 | s | the Heavy's `m_spool_idle` (a 10-frame loop at 30 fps, 0.300 s long); a released spin or dry gun winds down this long after the last time it was re-sent |
| spool_down_anim | 1.333 | s | the Heavy's `m_spool_down` (41 frames at 30 fps, not looping, fade-out time 0.2 s) |
| spool_down_done | 0.85 | cycle | the sequence counts as finished once its cycle passes 1 − 0.2 s × 0.75/s = 0.85 (1.133 s at rate 1); the empty auto-switch and any switch away wait for it (§3.3) |
| barrel_max_speed | 20 | rad/s | target speed while winding up, spinning, firing, dry |
| barrel_step | 0.1 | rad/s per update | change of the barrel's speed per update (stock) |
| barrel_step_fast | 0.5 | rad/s per update | the same for items with the "can holster while spinning" attribute (none stock) |
| wav_wind_up | 0.9951 | s | `minigun_wind_up.wav`, 43 884 frames, one-shot |
| wav_wind_down | 3.3300 | s | `minigun_wind_down.wav`, 146 851 frames, one-shot |
| wav_fire | 4.0009 | s | `minigun_shoot.wav`, 176 441 frames, loops from sample 0 |
| wav_fire_crit | 4.0009 | s | `minigun_shoot_crit.wav`, 176 441 frames, loops from sample 0 |
| wav_spin | 0.6038 | s | `minigun_spin.wav`, 26 626 frames, loops from sample 0 |
| wav_empty | 0.6023 | s | `minigun_empty.wav`, 26 560 frames, loops from sample 0 |

All six WAVs are 16-bit PCM stereo at 44 100 Hz. The four loops carry a
`cue ` chunk with one cue point at sample offset 0 (and a label "01");
the wind-up and wind-down have no cue and no `smpl` chunk.

## Behavior

### 1. Inputs and timers

The gun keeps:

- its **state**: idle, winding up, spinning, firing, dry;
- a **mode**, primary or secondary, rewritten whenever the state step (§2.2)
  runs: primary if A is held, else secondary if B is held, else unchanged;
- **next_attack**: one time shared by both buttons (every place that sets
  it sets both buttons' times, except that a shot pushes only the primary
  one; see §2.2 "both held");
- **idle_at**: the earliest time a released gun may wind down. Any
  first-person animation the gun sends sets it to now + that sequence's
  length, and the Minigun then overrides it in the places listed below.

### 2. The state machine

#### 2.1 One tick (server, and the owner's client in prediction)

Only while the Minigun is the active, deployed weapon (during the 0.5 s
draw nothing below runs).

1. **Attack2 held.** If B is held:
   - with a = 0: nothing at all (no state step, no sound; the generic
     "empty" click is suppressed for this gun);
   - else, if now ≥ next_attack: run the state step (§2.2).
2. **Attack held.** Then, if A is held and now ≥ next_attack (as it stands
   after step 1):
   - with a = 0: the **empty step**: if the state is firing or spinning,
     it becomes dry (and the spin-idle animation is sent, so idle_at =
     now + 0.3); if the mode is secondary it then becomes spinning
     instead. In idle, winding up or dry: nothing.
   - else: run the state step (§2.2).
3. **No button.** If neither A nor B is held:
   - first the empty auto-switch check (§3.3);
   - then, if now ≥ idle_at and the state is not idle: **wind down**:
     state idle, the post-fire animation (`m_spool_down`), idle_at =
     now + 2.0 (for the view model's idle; weapons.md), the barrel's target
     speed 0, the wind-down sound (§4), speed and jump back at once
     (weapons.md §7.1).

#### 2.2 The state step

The mode is set first (above). Then by state:

| State | What the step does |
|---|---|
| idle | **Wind up**: state winding up; the pre-fire animation (`m_spool_up`, rate 0.75 / wind-up = 1); next_attack and idle_at = now + 0.75; the wind-up sound; the barrel's target 20; the Heavy slowed and unable to jump (weapons.md §7.1); the wind-up start time is recorded if not already set (for the rev-up penalty). |
| winding up | Only once now ≥ next_attack (the step doesn't run earlier anyway): state **spinning** if the mode is secondary, else **firing** (no ammo check here). next_attack and idle_at = now + 0.1. |
| firing | If the mode is secondary: state **spinning**, next_attack and idle_at = now + 0.1. Else if a = 0: state **dry** (no animation here). Else: **fire** if due (the shot itself checks its own next time: one shot, then next = now + 0.105, weapons.md §7.2); then idle_at = now + 0.2. |
| spinning | If the mode is primary: state **firing** if a > 0, else **dry**. In every case the spin-idle animation is re-sent (idle_at = now + 0.3; the animation restarts every tick, viewmodel.md quirk). |
| dry | Clears the wind-up and firing start times (the rev-up penalty starts over). If a > 0: state **firing**. Else if the mode is secondary: state **spinning**. The spin-idle animation is re-sent (idle_at = now + 0.3). |

Both held: step 1 runs first, so with A and B held the gun is in primary
mode and fires (A wins). Since a shot pushes only the primary next time,
step 1 then runs the firing step every tick; that changes nothing
audible (the shot still comes every 0.105 s; idle_at keeps moving to
now + 0.2).

#### 2.3 The transitions, by buttons and ammo

Ammo a > 0:

| From | Buttons | Result | When |
|---|---|---|---|
| idle | A or B (or both) pressed | winding up | that tick |
| winding up | A held at 0.75 s | firing; first shot 0.1 s later (tick 57) | tick 50 |
| winding up | B held, A not, at 0.75 s | spinning | tick 50 |
| winding up | released before 0.75 s, none held at 0.75 s | **the wind-up completes**, then idle (wind-down) | tick 50 (measured: a 0.3 s tap still winds up 0.75 s, then winds down) |
| winding up | released, then pressed again before 0.75 s | as if held: decided by the buttons held at 0.75 s | tick 50 |
| firing | A released, B held | spinning | the first tick without A (measured: 1 tick) |
| firing | A released, nothing held | idle (wind-down) at max(now, last shot + 0.2) | 0.095 to 0.2 s after the release (measured: 13 ticks after it, with about 2 ticks of input delay) |
| firing | B pressed too | stays firing | – |
| spinning | A pressed (B still held) | firing, and the first shot **the same tick** (the spin had long passed its next time) | the press tick (measured: the fire loop replaced the spin within the 3-tick input delay) |
| spinning | B released | idle (wind-down) **0.3 s later** (the last re-sent spin-idle animation) | 20 ticks after the last tick B was held (measured 0.33 s at the observing client) |
| idle (after a wind-down) | A or B pressed | winding up at once, a full 0.75 s again; the barrel keeps its current speed and accelerates from there | the press tick (measured) |

Ammo running out and a = 0 (§3 has the details):

| From | Buttons | Result | When |
|---|---|---|---|
| firing | A held (B held or not) | dry | the due time of the shot after the last one (last shot + 0.105) |
| dry | A held | stays dry | – |
| dry | A released, B held | **stays dry** (attack2 is refused with no ammo, so nothing else runs and the gun never idles) | – |
| dry | everything released | idle (wind-down) at max(now, entering dry + 0.3) | – |
| idle | A or B pressed | **nothing** (no wind-up, no sound, no slow-down) | – |
| idle | nothing held | auto-switch away once the spool-down animation counts as finished (§3.3) | 77 ticks (1.155 s) after the wind-down began |
| dry | ammo picked up, A held | firing at once (no shot that tick), shots from the next tick; the rev-up penalty starts over | – |
| dry | ammo picked up, only B held | firing for one tick (mode secondary), then spinning | 1 tick |

#### 2.4 Release timing in one line each

- From winding up: at the wind-up's end (press + 0.75 s), never earlier.
- From firing: 0.2 s after the latest shot.
- From spinning: 0.3 s after the last tick attack2 was held.
- From dry: 0.3 s after entering dry, or at once if that has passed.

### 3. Out of ammo

#### 3.1 Running dry

Held fire with 200 ammo: 200 shots at ticks 57 + 7k (k = 0 … 199), the
last at tick 1450 (21.75 s). At tick 1457 (the next due shot) the gun
goes dry: the fire loop is cut and the **empty loop**
(`Weapon_Minigun.ClipEmpty`, 0.60 s, looping) starts and keeps looping
without a gap for as long as the gun stays dry (measured: one start, no
re-trigger, 10 s and 3.4 s of dry in two runs). The barrel keeps its
full speed (its target stays 20) and the Heavy stays slowed and unable
to jump. No shots, no muzzle flash or brass (they are tied to firing).

#### 3.2 What the buttons do while empty

- Holding attack, attack2 or both after running dry: the gun stays dry,
  the empty loop plays. Letting go of attack while attack2 stays held
  does **not** idle the gun (measured: the empty loop went on until
  attack2 was released, then the wind-down began the same tick).
- attack2 with no ammo is refused before the Minigun sees it: an empty
  Minigun **cannot spin up** with attack2, and attack from idle does
  nothing either (the empty step only acts on firing/spinning). No
  sound plays for these presses.
- The generic once-per-0.5 s empty click that other weapons make is
  suppressed for the Minigun: its only empty sound is the loop.

#### 3.3 Auto-switch

Yes, the empty Minigun switches away by itself. On every tick with no
button held, before the wind-down check: if the gun has no ammo and both
its next times have passed, the game tries to switch to the player's
**previous weapon** if that has ammo, otherwise to the best weapon by the
scripts' `weight` (the melee always qualifies). The switch is refused
while the Minigun can't be put away: while it isn't idle, and after the
wind-down while the first-person spool-down sequence is not yet
**finished** in the engine's sense (below). Holding any button postpones
the switch.

**When the spool-down counts as finished** (2026-11-20, from the SDK's
animation code, the arms' sequence data and a per-tick measurement):

- A non-looping sequence is flagged finished as soon as an animation
  advance takes its cycle **past 1 − fade_out × cycle_rate × playback
  rate**, not at cycle 1. fade_out is the sequence's own cross-fade-out
  time (0.2 s for `m_spool_down`, the compiler's default), cycle_rate is
  1 / its length (1 / 1.333 = 0.75 per second). For `m_spool_down` at
  rate 1: 1 − 0.2 × 0.75 = **0.85**, reached after 0.85 / 0.75 =
  **1.133 s** (its length minus 0.2 s). (The flag is also set when the
  cycle reaches 1.)
- The playback rate is **1**. Every new view-model sequence resets the
  playback rate to 1 when it starts; the wind-up's rate (0.75 / wind-up
  time) is set after the wind-up sequence is sent and so applies to
  `m_spool_up` only, and the wind-down's `m_spool_down` starts at rate
  1 for every Minigun variant. (This replaces the earlier reading that
  the wind-up's rate is kept.)
- Per tick: the weapon's think (with the switch check) runs **before**
  the view model's animation advance in the same tick. The wind-down
  tick sends `m_spool_down` at cycle 0; each later tick advances it by
  0.015 / 1.333 = 0.01125. After 76 advances the cycle is 0.855 > 0.85
  and the flag is set; the next tick's check sees it and switches.
- So the empty auto-switch (and a manual switch away) comes **77 ticks
  = 1.155 s after the wind-down tick**, and the new weapon's draw starts
  then. In general: ticks = ceil((L − f) / 0.015) + 1, with L the
  spool-down's length and f its fade-out time, at rate 1.

Measured (2026-11-20, the local player's stock Minigun, `host_framerate
0.015`, the view model's sequence and cycle read every tick): the switch
to the Shotgun came 77 ticks after the wind-down began, at
`m_spool_down` cycle 0.855, in both runs. The earlier 1.11 s with this
account's non-stock variant (0.6 s wind-up) is not explained by a scaled
rate (there is none at the wind-down); by the rule above it too should
be 77 ticks; it was measured from an observing client, not per tick. An empty Minigun cannot be switched back to by hand (a
weapon with no ammo of its type is refused) until ammo arrives.

### 4. Sounds

#### 4.1 The sound entries (stock; the script's sound roles)

| State | Script role | Sound script entry | WAV | Channel | Level | Volume | Loops |
|---|---|---|---|---|---|---|---|
| winding up | `special1` | `Weapon_Minigun.WindUp` | `)weapons/minigun_wind_up.wav` | weapon | 94 dB | 1.0 | no (0.995 s; cut at 0.75 s) |
| firing | `double_shot` | `Weapon_Minigun.Fire` | `)weapons/minigun_shoot.wav` | static | 94 dB | 1.0 | yes, cue at 0 (4.0 s) |
| firing, current shot a crit | `burst` | `Weapon_Minigun.FireCrit` | `)weapons/minigun_shoot_crit.wav` | static | 94 dB | 1.0 | yes, cue at 0 (4.0 s) |
| spinning | `special3` | `Weapon_Minigun.Spin` | `)weapons/minigun_spin.wav` | static | 75 dB | 1.0 | yes, cue at 0 (0.60 s) |
| dry | `empty` | `Weapon_Minigun.ClipEmpty` | `)weapons/minigun_empty.wav` | static | 75 dB | 1.0 | yes, cue at 0 (0.60 s) |
| idle, barrel still turning | `special2` | `Weapon_Minigun.WindDown` | `)weapons/minigun_wind_down.wav` | weapon | 94 dB | 1.0 | no (3.33 s; cut when the barrel stops) |
| idle, barrel stopped | – | – | – | – | – | – | silence |
| (draw) | – | `Weapon_Minigun.Draw` | `weapons/draw_minigun_heavy.wav` | auto | 74 dB | 0.5 | no |

Pitch 100 for the stock gun. (Items that change the fire rate shift the
pitch of all six: 80 at 1.5 × the stock interval to 120 at 0.5 ×, linear,
clamped; not stock.) The `)` prefix marks spatial stereo: the stereo
file is positioned in the world (cs_source/sounds.md path rules). The
item schema may replace these entries per item and team (the stock gun
doesn't). No per-shot sound exists: the shot plays nothing of its own.

#### 4.2 How the one sound is managed

- Each Minigun owns **one** current sound. The wanted sound is a function
  of the state (table above). Whenever the wanted sound differs from the
  current one, the current one is **stopped at once** (no fade) and the
  wanted one **started from its beginning** at full volume (no fade in:
  the start sets volume 1 immediately; the 0.1 s volume ramp that follows
  goes from 1 to 1). When the wanted sound equals the current one, nothing
  happens: a loop keeps looping, a one-shot keeps playing or stays
  finished.
- So channels don't decide replacement here: the gun stops its own
  previous sound by name. The channels still matter for anything else
  emitted on the same entity (nothing stock is).
- A loop is never re-triggered by the game: it repeats because its WAV
  loops (§7). Every state change restarts the new sound from sample 0, so
  firing → spinning → firing restarts `minigun_shoot.wav` from its start;
  a crit flag flipping on or off swaps Fire and FireCrit, each from its
  start.
- When the wanted sound is re-evaluated: on the owner's client at the
  predicted wind-up and wind-down; on every client (the owner's too) each
  time a network update for the gun arrives; and when the barrel's speed
  reaches 0 (which turns the wind-down off). So spinning, firing and dry
  sounds follow the server's state as received, also for the owner. Ours
  may re-evaluate every tick from the predicted state (no audible
  difference beyond the latency).
- The sound stops at once when the gun is removed (death, disconnect)
  or reset (respawn).

#### 4.3 The wind-down's length

The wind-down plays while the gun is idle **and** the barrel is still
turning, and is cut the moment the barrel's speed reaches 0 (§5). The
barrel decelerates 0.1 rad/s per update, so the wind-down lasts as many
updates as the barrel needs to stop from its speed at the release:

```math
t_{wind\,down} = \min\!\left(\frac{v_{release}}{0.1},\ 200\right)\cdot 0.015\ \text{s}
\quad(\le 3.0\ \text{s} < 3.33\ \text{s, the WAV})
```

- After a spin of 3 s or more: v = 20, cut after 3.0 s (measured 2.985 s).
- After a tap (wind-up only, 0.75 s): v = 5, cut after 0.75 s (measured
  0.63 s on a remote client, whose barrel had started a few frames late).
- After attack2 held for 2.25 s: wind-down 0.3 s after the release at
  v ≈ 17 → cut about 2.5 s after that (measured 2.475 s).
- If the barrel isn't being updated the cut never comes and the WAV
  plays its full 3.33 s: measured after the empty auto-switch (the gun
  put away). Ours: always update the barrel per tick while the weapon
  exists, holstered or not, and play the wind-down to its WAV end only
  if the gun is put away (§Edge cases).

#### 4.4 Timeline of a held press (t = 0 at the press, stock, 200 ammo)

| t (s) | Tick | Sound |
|---|---|---|
| 0 | 0 | wind-up starts |
| 0.75 | 50 | wind-up cut (0.245 s of it never heard), fire loop starts |
| 0.855 | 57 | first shot (no sound of its own) |
| release at 3.0 | 200 | fire loop continues |
| 3.155 | 211 (first tick ≥ last shot at 2.955 + 0.2) | fire loop cut, wind-down starts |
| 6.165 | 411 | wind-down cut (barrel stopped) |

### 5. The barrel spin

- **What it drives**: the bone named `barrel` of the `c_minigun` model
  (bone 1, child of `weapon_bone`; its local z axis runs along the
  barrels). Not a pose parameter: `c_minigun` has none, and its own
  sequences (`idle`, `spool_up`, `fire_loop`, `spool_down`, all 0.967 s)
  don't turn it. The same angle is used in first person (the `c_` model
  bone-merged onto the arms) and third person.
- **Target speed**: 20 rad/s from the moment the wind-up begins (stays 20
  through spinning, firing and dry); 0 from the wind-down. (Also set to 0
  whenever the idle sound is chosen while the barrel still turns.)
- **Speed update**: the current speed v moves toward the target by 0.1
  rad/s per update, never past it (0.5 for "can holster while spinning"
  items; none stock).
- **Angle update**: after the speed, angle += v × dt, dt = that update's
  frame time (0.015 per tick). The update comes **before** the gun's
  state step in the tick (the original runs it in the weapon's pre-frame),
  so the tick that starts a wind-up or wind-down changes the target for
  the next tick's update. Radians, unbounded (wrap it for float
  precision; only the angle mod 2π matters).
- **Applying it**: the bone's local rotation about its z axis is replaced
  by the angle. In the world model its rest rotation is zero, so the
  whole local rotation becomes (roll 0, pitch 0, z = angle); in the view
  model the animated x and y Euler components are kept and only z is
  replaced. Positive angles turn counter-clockwise about +z.
- **Update rate in the original**: the owner's gun updates once per new
  predicted command (= per tick) plus once per bone setup of the gun's
  own model; other players' guns update once per bone setup, i.e. per
  rendered frame while drawn, with the frame time as dt. So the original
  is frame-rate dependent (a remote Minigun at 144 fps stops in 1.4 s).
  **Ours: one update per tick for every Minigun** (66.67 Hz), which gives
  the figures below and matches the measurements (taken at 66.67 Hz).
- **For other players' guns** the target is driven by the Heavy's
  animation events the client receives (wind-up start → 20, fire →
  20, wind-down → 0), not by the networked state (Quirks: crouched).

Per-tick numbers (wind-up at tick 0, so the updates that accelerate are at ticks 1, 2, …):

| After n updates | Speed (rad/s) | Angle (rad) |
|---|---|---|
| 1 | 0.1 | 0.0015 |
| 50 (end of the wind-up) | 5.0 | 1.9125 |
| 57 (first shot) | 5.7 | 2.4795 |
| 100 | 10.0 | 7.575 |
| 200 (full speed, 3.0 s) | 20.0 | 30.15 |
| then | 20.0 | + 0.3 per tick (3.18 turns/s, 191 rpm) |

The angle after n updates of spin-up from rest is 0.0015 · n(n+1)/2.
A wind-down from full speed takes 200 updates (3.0 s) and turns the
barrel a further 29.85 rad (4.75 turns).

### 6. What the other players hear

- The server sends **no Minigun sound messages**. The gun's state (one of
  the five) and its "current shot is a crit" flag are part of the
  weapon's networked data, and the Minigun is sent to **every client
  always**, whatever the distance or visibility. Each client (the owner's
  too) runs §4's selection for every Minigun it knows and plays the sound
  for itself only, from the weapon entity's position (the owner's hand,
  i.e. the player), with the levels of §4.1: the 94 dB fire and wind
  sounds carry far, the 75 dB spin and empty loops much less.
- Others therefore hear a state change when the update arrives (about
  half the round-trip later; no interpolation delay: sounds follow the
  data on arrival). Measured on a listen server: the bot's wind-up was
  heard 2–3 ticks after its press, every later change within a tick of
  the server's.
- A player joining while a Minigun fires starts the matching loop on
  his first update; a wind-down then is silent for him (his copy of the
  barrel isn't turning).
- The owner hears the same sounds, started by his own prediction for the
  wind-up and wind-down (no latency) and by the server's updates for the
  rest.
- Ours: drive the sound from the replicated state (and crit flag) on
  every client, local prediction for the owner allowed; never send
  Minigun sounds as one-shot sound events.

### 7. Looping (all games)

Source decides that a sound loops **only from the WAV file itself**: a
`cue ` chunk's first cue point, or a `smpl` chunk's first loop, gives
the loop start; the loop runs to the end of the sample data and repeats
until the sound is stopped. Nothing in the sound script, the channel or
the calling code makes a sound loop, and a looping sound is never
re-triggered by the game. Full rule, the data scan and what to fix:
[../source/sound_looping.md](../source/sound_looping.md). For the
Minigun: Fire, FireCrit, Spin and ClipEmpty loop from sample 0 (the whole
file); WindUp and WindDown don't loop. Playing the four loops as
one-shots is why the fire sound went silent after 4.0 s and the empty
sound played once (0.6 s).

## Per-tick order

1. Barrel: speed toward the target by 0.1, then angle += speed × 0.015;
   if the speed has just reached 0, re-evaluate the sound (ends the
   wind-down).
2. Read buttons (attack, attack2) and ammo.
3. Attack2 step (§2.1.1); then attack step (§2.1.2) with next_attack as
   the attack2 step left it; or, with no button, the auto-switch check and then the
   wind-down check (§2.1.3).
4. Owner's client, predicted: if the step began a wind-up or a wind-down,
   re-evaluate the sound now.
5. Network: the state and crit flag go out; each receiving client
   re-evaluates its sound for that gun.

## Edge cases

- **Both buttons pressed in idle**: one wind-up; at 0.75 s firing (attack
  wins).
- **Attack2 added during a wind-up begun with attack, attack released**:
  at 0.75 s the gun spins (the mode is decided by what is held then).
- **Switching away / holster while not idle**: refused (stock). If a
  forced switch puts the gun away (round end, death), it winds down
  first; the barrel then isn't drawn, and the original lets the wind-down
  WAV run to its end (3.33 s). Ours may keep that for a put-away gun.
- **Death**: the gun's sound stops at once with it.
- **Barrel speed not 0 at a new wind-up**: the wind-up sound replaces the
  wind-down at once; the barrel accelerates from its current speed.
- **Ammo given while dry and a button held**: §2.3 (one tick of firing
  state even with only attack2: a one-tick blip of the fire loop is
  possible for observers; harmless, ours may skip it).
- **Ammo exactly 0 at the start and attack2**: nothing, ever, until ammo
  arrives (§3.2).
- **Crit boost (crit flag on for every shot)**: the crit loop plays from
  the first shot, continuously.

## Quirks

- **The wind-up WAV is cut at 0.75 s** of its 0.995 s, every time
  (including a tap: the tap's wind-down replaces it at 0.75 s). Keep.
- **The wind-down is cut by the barrel, not by its WAV**, so a tap's
  wind-down is short (0.75 s) and a full one 3.0 s of the 3.33 s file.
  Keep (per tick).
- **The spin releases 0.3 s late** because the spin-idle animation,
  re-sent every tick while spinning, keeps pushing the idle time. Keep
  the 0.3 s.
- **Dry with attack2 held never idles**, even with attack released. Keep.
- **An empty Minigun can't spin with attack2.** Keep.
- **Frame-rate dependent barrel** (original): per update, not per second.
  Use per tick.
- **Crouched or swimming Heavies' barrels don't spin up for observers**
  in the original: the observers' barrel target follows only the
  standing wind-up/fire/wind-down animation events, and crouched or
  swimming ones use other activities. Their wind-down is then silent for
  observers (no turning barrel). Not measured; don't keep (drive the
  barrel by state for everyone).
- The owner's own wind-down (this account's Minigun variant, as owner)
  was not cut early in the measurement: it ran until the next wind-up
  replaced it. Open question 2.

## Test cases

Stock gun, 200 ammo unless stated, t = 0 at the first press, tick 0.015 s.
"Sound X at t" = X starts at t; "cut" = stopped at once.

| # | Setup | Input | Expected |
|---|---|---|---|
| 1 | idle | hold A | wind-up at 0; at 0.75 (tick 50) wind-up cut, fire loop starts; shots at ticks 57, 64, 71, … |
| 2 | as 1 | release A at 3.0 (tick 200) | last shot tick 197; fire loop cut and wind-down at tick 211 (3.165); wind-down cut at tick 411 (6.165) |
| 3 | idle | A from 0 to 0.3 | wind-up 0 → 0.75 (not cut at the release); wind-down from tick 50 (0.75) to tick 100 (1.5) (barrel 5 rad/s → 0); state idle from 0.75; speed 230 and jump from 0.75 |
| 4 | idle | hold B for ticks 0 … 149 (released at 2.25) | wind-up 0; spin loop from 0.75 (tick 50); the spin loop goes on to tick 169 (2.535), then wind-down; barrel at 16.9 rad/s → wind-down cut at tick 338 (5.07) |
| 5 | as 4, spun | press A at 1.5 (B held) | fire loop replaces spin at tick 100; first shot tick 100 |
| 6 | as 5 | release A at 3.0, keep B | spin loop replaces fire at tick 200 (the first tick without A) |
| 7 | as 2 | press A again while the wind-down plays | that tick: wind-down cut, wind-up starts; fire loop 50 ticks later, first shot 57 ticks later (no shortened wind-up; measured) |
| 8 | firing | hold A for 30 s | 200 shots (ticks 57 … 1450); at tick 1457 (21.855) fire loop cut, empty loop starts and loops without a gap to 30 s; no new sound starts in between |
| 9 | as 8 | release A at 30 | empty loop cut and wind-down at once (tick 2000); at tick 2077 (30 + 1.155 s; `m_spool_down` cycle 0.855) switch to the previous weapon (its draw sound) |
| 10 | idle | hold A and B, 200 ammo | fires from 0.855; dry at 21.855 with the empty loop; release A at 24: still dry, empty loop; release B at 25.5: wind-down that tick; switch 77 ticks (1.155 s) later |
| 11 | idle, ammo 0 (before the switch) | press B or A, hold 1 s | nothing: no wind-up, no sound, speed 230, jump works; no switch while held; switch after release |
| 12 | dry, A held | ammo pack gives 100 | next tick: firing (fire loop replaces empty); shots from the tick after; damage penalty from τ = 0 (weapons.md §7.2) |
| 13 | firing | the crit flag turns on at 5.0 for 2 s | FireCrit replaces Fire at 5.0 from its start; Fire replaces FireCrit at 7.0 from its start |
| 14 | firing | hold A for 10 s | `minigun_shoot.wav` started once at 0.75 and heard continuously until the release (loop from sample 0; never silent after 4.75) |
| 15 | barrel | wind-up at tick 0 | speed 0.1·n, angle 0.0015·n(n+1)/2 after n ticks (n ≤ 200); 5.0 and 1.9125 at tick 50; 20 at tick 200 |
| 16 | barrel at 20 | wind-down at tick 0 | speed 20 − 0.1·n; 0 at tick 200 (3.0 s); the wind-down sound cut at that tick |
| 17 | spinning (B) | release B | state idle 20 ticks (0.3 s) after the last held tick; the Heavy can jump from then |
| 18 | two clients, Heavy on client 1 fires | listen on client 2 | client 2 receives no sound event; it starts and stops the same sounds as client 1 from the gun's replicated state, within the update latency |

## Open questions

1. The spin-idle hold is 0.30 s in the data (`m_spool_idle` 0.300 s);
   measured 0.33–0.345 s at the observing client, which includes 1–2
   ticks of mimic/update delay. Check with the owner's own stock gun.
2. The owner's own wind-down (measured with this account's non-stock
   Minigun) was never cut when the barrel stopped, while the observed
   bot's stock one was (2.985 s). Code says it should be cut for the
   owner too. Check with a stock Minigun in the owner's hands (needs a
   loadout with the stock gun); until then use the barrel cut for all.
3. ~~The auto-switch delay with a stock Minigun.~~ Answered
   (2026-11-20, §3.3): 77 ticks (1.155 s) after the wind-down, when
   `m_spool_down`'s cycle passes 0.85 (its length minus its 0.2 s
   fade-out), at playback rate 1 for every variant.
4. Crit loop switching was not measured (the reference server has random
   crits off). Per code: Fire ↔ FireCrit follow the networked crit flag.
5. The crouched/swimming observer quirk (barrel and wind-down) is from
   the code only.
