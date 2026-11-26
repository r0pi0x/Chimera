# Counter-Strike 1.6: weapons

Source basis: the community reimplementation of the CS 1.6 game library
(ReGameDLL_CS, open source; read in a separate spec session as reference
only, its original-game behaviour, never its optional fixes or
additions), then checked against the game: every view model's sequences
(frame counts and frame rates) read from the install's `v_*.mdl` files,
the player models' hit boxes, and the server's console variables (HLDS
build 10211). Numbers marked "(ref.)" are the reference's and were not
measured in the running game (see Open questions). Damage, armour, hit
zones and wall penetration are in [damage.md](damage.md). No code is
reproduced here.
Status: draft

## Summary

- Every gun fires hitscan bullets from the eye along the view angles
  plus the punch angle, bent by a random offset scaled by a **spread**
  that each gun computes per shot from its state (in the air, moving,
  ducked, standing) and its **accuracy** value. Automatic guns' accuracy
  worsens with the number of shots in the current burst; pistols' and
  autosnipers' accuracy depends on the time since the previous shot.
- Recoil is a **punch angle** added to the view: automatic guns use a
  "kick" with seven parameters per state (base, growth per shot and
  caps, up and sideways, and a chance to switch sideways direction);
  pistols and bolt snipers kick 2° up; autosnipers and shotguns kick by a
  random amount. The punch decays back at (10 + 0.5 × punch) °/s.
- Weapon timers are relative and counted down per user command, so
  cycle times round up to whole commands.

## Units and conventions

- Damage in hit points (players have 100); distances in units; angles in
  degrees (pitch positive down; punch pitch negative = view kicked up;
  punch yaw positive = view kicked left).
- Time in seconds. All weapon timers ("next primary attack", "next
  secondary attack", "idle", the player's "next attack") are relative:
  set to a delay when an action happens, decreased by the command's step
  (msec / 1000) at the end of each command, after the weapon has acted.
  An action is allowed when its timer is ≤ 0. "Next primary attack"
  stops at −1, the others at −0.001.
- Spread is a dimensionless offset: the bullet's direction is
  forward + x·s·right + y·s·up (not renormalised), where s is the
  spread and x, y each the sum of two uniform numbers in [−0.5, 0.5]
  (so −1..1, triangular, **not** limited to a circle). x and y come from
  the command's shared random seed (seed, seed + 1 for x; seed + 2,
  seed + 3 for y), so client and server agree.
- "Moving" means horizontal speed above a gun-specific threshold (often
  0 or 140); "in the air" means not on ground; "ducked" means the
  ducked flag (a finished duck).

## The guns

Prices in dollars; buy side by the game's rules (T: terrorists only, CT:
counter-terrorists only) (ref.). Speed is the maximum speed holding the
gun (movement.md). Penetration "count" is how many surfaces the bullet
may hit (1 = no wall penetration); the bullet type sets penetration
power and distance (damage.md).

| Gun (data id) | Slot | Side | Price | Damage | Range modifier | Count | Bullet type | Max range | Cycle (s) | Clip | Reserve | Reload (s) | Speed |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| `glock18` | pistol | both | 400 | 25 | 0.75 | 1 | 9 mm | 8192 | 0.15 semi; 0.5 per burst | 20 | 120 | 2.2 | 250 |
| `usp` | pistol | both | 500 | 34; silenced 30 | 0.79 | 1 | .45 ACP | 4096 | 0.15 | 12 | 100 | 2.7 | 250 |
| `p228` | pistol | both | 600 | 32 | 0.8 | 1 | .357 SIG | 4096 | 0.15 | 13 | 52 | 2.7 | 250 |
| `deagle` | pistol | both | 650 | 54 | 0.81 | 2 | .50 AE | 4096 | 0.225 | 7 | 35 | 2.2 | 250 |
| `elite` | pistol | T | 800 | 36 | 0.75 | 1 | 9 mm | 8192 | 0.075 | 30 | 120 | 4.5 | 250 |
| `fiveseven` | pistol | CT | 750 | 20 | 0.885 | 1 | 5.7 mm | 4096 | 0.15 | 20 | 100 | 2.7 | 250 |
| `m3` | shotgun | both | 1700 | 9 pellets × 20 | linear to 0 at 3000 | none | buckshot | 3000 | 0.875 | 8 | 32 | 0.55 + 0.45 a shell | 230 |
| `xm1014` | shotgun | both | 3000 | 6 pellets × 20 | linear to 0 at 3048 | none | buckshot | 3048 | 0.25 | 7 | 32 | 0.55 + 0.3 a shell | 240 |
| `mac10` | SMG | T | 1400 | 29 | 0.82 | 1 | .45 ACP | 8192 | 0.07 | 30 | 100 | 3.15 | 250 |
| `tmp` | SMG | CT | 1250 | 20 | 0.85 | 1 | 9 mm | 8192 | 0.07 | 30 | 120 | 2.12 | 250 |
| `mp5navy` | SMG | both | 1500 | 26 | 0.84 | 1 | 9 mm | 8192 | 0.075 | 30 | 120 | 2.63 | 250 |
| `ump45` | SMG | both | 1700 | 30 | 0.82 | 1 | .45 ACP | 8192 | 0.1 | 25 | 100 | 3.5 | 250 |
| `p90` | SMG | both | 2350 | 21 | 0.885 | 1 | 5.7 mm | 8192 | 0.066 | 50 | 100 | 3.4 | 245 |
| `galil` | rifle | T | 2000 | 30 | 0.98 | 2 | 5.56 mm | 8192 | 0.0875 | 35 | 90 | 2.45 | 240 |
| `famas` | rifle | CT | 2250 | 30; first shot of a burst 34 | 0.96 | 2 | 5.56 mm | 8192 | 0.0825 auto; 0.55 per burst | 25 | 90 | 3.3 | 240 |
| `ak47` | rifle | T | 2500 | 36 | 0.98 | 2 | 7.62 mm | 8192 | 0.0955 | 30 | 90 | 2.45 | 221 |
| `m4a1` | rifle | CT | 3100 | 32; silenced 33 | 0.97; silenced 0.95 | 2 | 5.56 mm | 8192 | 0.0875 | 30 | 90 | 3.05 | 230 |
| `sg552` | rifle | T | 3500 | 33 | 0.955 | 2 | 5.56 mm | 8192 | 0.0825; zoomed 0.135 | 30 | 90 | 3.0 | 235 / 200 zoomed |
| `aug` | rifle | CT | 3500 | 32 | 0.96 | 2 | 5.56 mm | 8192 | 0.0825; zoomed 0.135 | 30 | 90 | 3.3 | 240 |
| `scout` | sniper | both | 2750 | 75 | 0.98 | 3 | 7.62 mm | 8192 | 1.25 | 10 | 90 | 2.0 | 260 / 220 zoomed |
| `awp` | sniper | both | 4750 | 115 | 0.99 | 3 | .338 Magnum | 8192 | 1.45 | 10 | 30 | 2.5 | 210 / 150 zoomed |
| `g3sg1` | sniper | T | 5000 | 80 | 0.98 | 3 | 7.62 mm | 8192 | 0.25 | 20 | 90 | 3.5 | 210 / 150 zoomed |
| `sg550` | sniper | CT | 4200 | 70 | 0.98 | 2 | 5.56 mm | 8192 | 0.25 | 30 | 90 | 3.35 | 210 / 150 zoomed |
| `m249` | machine gun | both | 5750 | 32 | 0.97 | 2 | 5.56 mm (box) | 8192 | 0.1 | 100 | 200 | 4.7 | 220 |
| `knife` | melee | both | (start) | slash 15, stab 65 (× 3 from behind) | | | | 48 / 32 | 0.4 / 1.1 after a hit | | | | 250 |
| `hegrenade` | grenade | both | 300 | 100 at the centre | | | | radius 350 | | | 1 | | 250 |
| `flashbang` | grenade | both | 200 | blinds | | | | radius 1500 | | | 2 | | 250 |
| `smokegrenade` | grenade | both | 300 | | | | | | | | 1 | | 250 |

Ammunition: prices per clip bought (ref.): 9 mm 20 $ for 30, .45 ACP 25 $
for 12, .357 SIG 50 $ for 13, .50 AE 40 $ for 7, 5.7 mm 50 $ for 50,
buckshot 65 $ for 8, 5.56 mm 60 $ for 30, 7.62 mm 80 $ for 30, .338 125 $
for 10. Reserve caps: 9 mm 120, .45 100, .357 52, .50 35, 5.7 100,
buckshot 32, 5.56 90 (box 200), 7.62 90, .338 30. A bought gun comes with
a full clip and no reserve. Armour: vest 650 $, vest and helmet 1000 $
(helmet alone 350 $); defuse kit 200 $; night vision 1250 $; tactical
shield 2200 $ (CT).

### Reload, draw and switch times

The reload time in the table (the reference's) is when the clip is
refilled and the gun can fire again (the player's "next attack" timer;
the idle timer is set to reload + 0.5). The view model's reload
animation, read from the install, is the same length within 0.03 s for
most guns and longer for some (the gun is ready before the animation
ends):

| Gun | Reload (s) | Reload animation (frames at fps = s) | Draw animation (s) |
|---|---|---|---|
| ak47 | 2.45 | 91 at 37 = 2.459 | 1.033 |
| aug | 3.3 | 133 at 40 = 3.325 | 0.886 |
| awp | 2.5 | 88 at 30 = 2.933 | 1.033 |
| deagle | 2.2 | 66 at 30 = 2.200 | 1.033 |
| elite | 4.5 | 138 at 30 = 4.600 | 1.100 |
| famas | 3.3 | 91 at 30 = 3.033 | 0.886 |
| fiveseven | 2.7 | 97 at 30 = 3.233 | 1.033 |
| g3sg1 | 3.5 | 141 at 30 = 4.700 | 1.033 |
| galil | 2.45 | 91 at 35 = 2.600 | 1.029 |
| glock18 | 2.2 | two, at random: 76 at 35 = 2.171 or 76 at 30 = 2.533 | 1.111 (two, at random) |
| m249 | 4.7 | 141 at 30 = 4.700 | 1.033 |
| m4a1 | 3.05 | 114 at 37 = 3.081 | 1.025 |
| mac10 | 3.15 | 111 at 35 = 3.171 | 0.939 |
| mp5navy | 2.63 | 101 at 38 = 2.658 | 0.886 |
| p228 | 2.7 | 96 at 35 = 2.743 | 1.033 |
| p90 | 3.4 | 136 at 40 = 3.400 | 1.033 |
| scout | 2.0 | 61 at 30 = 2.033 | 1.033 |
| sg550 | 3.35 | 107 at 28 = 3.821 | 1.033 |
| sg552 | 3.0 | 119 at 37 = 3.216 | 1.027 |
| tmp | 2.12 | 54 at 25 = 2.160 | 0.944 |
| ump45 | 3.5 | 116 at 33 = 3.515 | 1.033 |
| usp | 2.7 | 101 at 37 = 2.730 | 1.021 |
| m3 | start 0.55, a shell 0.45 | start 16 at 30 = 0.533, insert 28 at 65 = 0.431, end 34 at 38 = 0.895 | 1.033 |
| xm1014 | start 0.55, a shell 0.3 | start 21 at 30 = 0.700, insert 36 at 90 = 0.400, end 13 at 30 = 0.433 | 1.033 |

Durations are frame count ÷ frame rate (the install's numbers).

- **Draw (deploy)**: the player's "next attack" is set to 0.75 s for
  every weapon (AWP: 1.45, Scout: 1.25, which also set their own next
  primary attack to that and next secondary to 1.0). Deploying resets
  the field of view to 90 and any zoom.
- **Holster**: knife and grenades set the player's next attack to 0.5 s
  when put away (ref.).
- Silencer (M4A1, USP): secondary attack toggles it, playing the
  attach/detach animation; the gun cannot fire for 2.0 s (M4A1;
  animation 61 at 30 = 2.033 s) or 3.0 s (USP; animation 116 at 37 =
  3.135 s). The state is kept with the gun.
- Burst toggle (Glock, FAMAS): secondary attack, 0.3 s, prints a centre
  message.
- Zoom toggle: 0.3 s between toggles.

## Firing

### Common rules, per command (after movement)

1. Glock or FAMAS burst follow-up shots (below).
2. If the next primary attack timer is ≤ 0 and the player zoomed before
   the last shot (AWP, Scout), restore that zoom.
3. If the fire button is not held, forget the time of the last shot
   (the pistols' accuracy then starts fresh only when their own last-fire
   time says so; see below).
4. A reload in progress completes when the player's next attack timer
   is ≤ 0: the clip takes min(clip size − clip, reserve) rounds.
5. Secondary attack (zoom, silencer, burst toggle, knife stab, shield)
   if held and allowed (its timer ≤ 0) and the gun has one.
6. Else primary attack if held and its timer ≤ 0: not during freeze
   time, not while defusing. An empty clip clicks (next attack 0.2 s).
7. Else reload if the reload key is held, the gun has a clip, no reload
   is running, the next primary attack timer is < 0 and no burst is in
   flight (and the shield is not raised).
8. Else, with no fire button held: shot-counter recovery (below);
   pistols reset their shot counter to 0; an empty clip reloads by
   itself once the next primary attack timer is < 0; idle animations.
9. At the end of the command, all timers count down.

Shots fired by a primary attack set the gun's next primary and
secondary attack to the cycle time. Because the timer is set (not
added) and counted down per command, the real interval is the cycle
rounded **up** to a whole number of commands: at 10 ms commands the
AK-47's 0.0955 fires every 0.10 s, the M4A1's 0.0875 every 0.09 s, the
P90's 0.066 every 0.07 s.

### Automatic guns: shots counter and accuracy

Each automatic gun (rifles, SMGs, M249) keeps a **shots fired** count
for the current burst and an **accuracy** value.

- On each shot: shots += 1, then accuracy = f(shots) (table below),
  capped. The spread of this shot was computed **before** the update,
  from the accuracy left by the previous shot.
- f uses whole-number division in the original game for every gun
  except the MP5 (see Quirks): for example the AK-47's
  floor(shots³ / 200) + 0.35 is 0.35 for shots 1-5, then 1.35 (capped
  1.25) from the sixth shot.
- Recovery: when both fire buttons are up after shooting, the shots
  count is capped at 15 and, from 0.4 s after the release, decreases by
  1 every 0.0225 s down to 0. The accuracy value itself is not
  recomputed while recovering; it changes on the next shot only.
- Deploy and reload set the shots count to 0 and accuracy to the gun's
  start value.

| Gun | Start accuracy | f(shots) | Cap |
|---|---|---|---|
| ak47 | 0.2 | floor(n³/200) + 0.35 | 1.25 |
| galil | 0.2 | floor(n³/200) + 0.35 | 1.25 |
| m4a1 | 0.2 | floor(n³/220) + 0.3 | 1.0 |
| famas | 0.2 (reload sets 0) | floor(n³/215) + 0.3 | 1.0 |
| aug | 0.2 (reload sets 0) | floor(n³/215) + 0.3 | 1.0 |
| sg552 | 0.2 | floor(n³/220) + 0.3 | 1.0 |
| m249 | 0.2 | floor(n³/175) + 0.4 | 0.9 |
| mp5navy | 0 | n²/220.1 + 0.45 (not rounded) | 0.75 |
| tmp | 0.2 | floor(n³/200) + 0.55 | 1.4 |
| mac10 | 0.15 (reload sets 0) | floor(n³/200) + 0.6 | 1.65 |
| ump45 | 0 | floor(n²/210) + 0.5 | 1.0 |
| p90 | 0.2 | floor(n²/175) + 0.45 | 1.0 |

### Spread per state

a is the gun's current accuracy (before this shot's update). The first
matching row applies, top to bottom.

| Gun | In the air | Moving (2D speed > threshold) | Otherwise |
|---|---|---|---|
| ak47 | 0.04 + 0.4a | > 140: 0.04 + 0.07a | 0.0275a |
| galil | 0.04 + 0.3a | > 140: 0.04 + 0.07a | 0.0375a |
| m4a1 | 0.035 + 0.4a | > 140: 0.035 + 0.07a | silenced 0.025a; unsilenced 0.02a |
| famas | 0.03 + 0.3a | > 140: 0.03 + 0.07a | 0.02a; all three + 0.01 in automatic mode |
| aug | 0.035 + 0.4a | > 140: 0.035 + 0.07a | 0.02a |
| sg552 | 0.035 + 0.45a | > 140: 0.035 + 0.075a | 0.02a |
| m249 | 0.045 + 0.5a | > 140: 0.045 + 0.095a | 0.03a |
| mp5navy | 0.2a | (no moving case) | 0.04a |
| tmp | 0.25a | (no moving case) | 0.03a |
| mac10 | 0.375a | (no moving case) | 0.03a |
| ump45 | 0.24a | (no moving case) | 0.04a |
| p90 | 0.3a | > 170: 0.115a | 0.045a |

Ducking does not change an automatic gun's spread (only its kick).

### Pistols

Semi-automatic: a shot needs a fresh press (the shots count must be 0;
it is reset when both fire buttons are up). Accuracy, start value A0,
lowered per shot by the time since the previous shot Δt:
a = clamp(a − (T − Δt) × k, lo, A0), only when there was a previous
shot (the last-fire time is kept across releases; it is not reset by
releasing the trigger). Shooting faster than T worsens accuracy, slower
improves it back toward A0. Spread = base × (1 − a).

| Gun | A0 | T (s) | k | lo | Air | Moving (2D > 0) | Ducked | Standing |
|---|---|---|---|---|---|---|---|---|
| glock18 semi | 0.9 | 0.325 | 0.275 | 0.6 | 1.0 | 0.165 | 0.075 | 0.1 |
| glock18 burst | 0.9 | 0.325 | 0.275 | 0.6 | 1.2 | 0.185 | 0.095 | 0.3 |
| usp silenced | 0.92 | 0.3 | 0.275 | 0.6 | 1.3 | 0.25 | 0.125 | 0.15 |
| usp unsilenced | 0.92 | 0.3 | 0.275 | 0.6 | 1.2 | 0.225 | 0.08 | 0.1 |
| p228 | 0.9 | 0.325 | 0.3 | 0.6 | 1.5 | 0.255 | 0.075 | 0.15 |
| deagle | 0.9 | 0.4 | 0.35 | 0.55 | 1.5 | 0.25 | 0.115 | 0.13 |
| elite | 0.88 | 0.325 | 0.275 | 0.55 | 1.3 | 0.175 | 0.08 | 0.1 |
| fiveseven | 0.92 | 0.275 | 0.25 | 0.725 | 1.5 | 0.255 | 0.075 | 0.15 |

The spread uses the accuracy after the previous shot's update (the
update happens inside the shot, after the spread was chosen). Reload
and deploy restore A0.

- **Glock burst**: in burst mode every press (or every 0.5 s while
  held: burst mode does not need a fresh press) fires a burst of three.
  The first shot is the normal shot above. The other two are fired on
  the **next two commands** (one per command, whatever the time) with a
  fixed spread 0.05, damage 25, range modifier 0.9, no punch. A burst
  stops early when the clip is empty.
- **Elites**: shots alternate between the left and right gun, starting
  with the left when the clip count is even on deploy; the left gun's
  bullet starts 5 units to the left of the eye, the right gun's 5 to the
  right.

### Snipers

- **AWP**: spread: in the air 0.85; 2D speed > 140: 0.25; > 10: 0.1;
  ducked 0; standing 0.001; + 0.08 when not zoomed. Zoom cycles 90 →
  40 → 10 → 90 field of view. Firing while zoomed drops to 90 at once
  and returns to the previous zoom when the cycle (1.45 s) has passed.
  Punch 2° up.
- **Scout**: spread: air 0.2; 2D speed > 170: 0.075; ducked 0; standing
  0.007; + 0.025 not zoomed. Zoom 90 → 40 → 15 → 90; same drop and
  return after shots. Punch 2° up.
- **G3SG1**: before the shot, accuracy = 0.55 + 0.3 × Δt since its
  previous shot, capped at 0.98 (first shot ever: 0.98). Spread =
  (1 − accuracy) × base, base: air 0.45; 2D speed > 0: 0.15; ducked
  0.035; standing 0.055; + 0.025 to the base when not zoomed. Zoom
  90 → 40 → 15 → 90 (no drop after a shot). Punch: pitch kicked up by
  U(0.75, 1.75) + 0.25 × (current punch pitch), i.e. less when already
  kicked; yaw by U(−0.75, 0.75); both from the shared seed (seed + 4,
  seed + 5).
- **SG550**: spread from the accuracy left by its previous shot: air
  0.45 (1 − a); 2D speed > 0: 0.15 (no accuracy term); ducked 0.04
  (1 − a); standing 0.05 (1 − a); then + 0.025 when not zoomed (added
  after the accuracy scaling). Then accuracy = 0.65 + 0.35 × Δt, capped
  at 0.98 (no update on the first shot ever; the start value is 0).
  Zoom as the G3SG1. Punch: pitch up by U(0.75, 1.25) + 0.25 × punch
  pitch, yaw U(−0.75, 0.75).
- Zoomed weapons' speed changes with the zoom (table above). Reloading
  a zoomed sniper unzooms it.
- Zoom fields of view: AUG and SG552 55 (one step, toggles back to 90).

### Shotguns

- Pellets: 9 (M3) or 6 (XM1014), each with its own random direction from
  the server's own random numbers (not the shared seed): x and y as two
  sums of uniform numbers, redrawn until x² + y² ≤ 1; direction =
  forward + x · 0.0675 · right + y · 0.0675 · up (XM1014: 0.0725).
- Each pellet that hits does int(20 × (1 − distance / range)) (range
  3000 / 3048), no penetration, no range modifier; the hits on the same
  target in one shot are summed into one damage event (damage.md).
- Punch: whole degrees up, from the shared seed: M3 4-6 on ground, 8-11
  in the air; XM1014 3-5 and 7-10.
- An empty magazine on a primary attack starts a reload; if still
  empty, next primary attack 1 s.
- Shell reload: a start phase (start animation; next attack, next
  primary and secondary 0.55 s), then repeatedly: when the idle timer
  has run out, play the insert animation and set the idle timer to the
  shell delay (0.45 / 0.3); when it runs out, add one shell (from the
  reserve) and repeat. When the clip is full or the reserve is empty,
  the end ("pump") animation plays (idle 1.5 s). The player may fire
  between shells (that ends the reload).

### Burst and automatic FAMAS

- Automatic mode: + 0.01 spread, cycle 0.0825.
- Burst mode: each press fires a burst of three; the first shot uses
  damage 34, the gun's normal spread and kick; then two follow-ups at
  0.05 s and a further 0.1 s, each with the first shot's spread, damage
  30, range modifier 0.96, count 2, no kick, not counted as shots. The
  cycle after a burst is 0.55 s.

### Recoil: the kick function

Automatic guns call a kick with seven parameters per state: up base
U_b, sideways base L_b, up growth U_m, sideways growth L_m, up cap
U_max, sideways cap L_max, direction-change odds D. With n the shots
count after this shot:

- n = 1: up = U_b, side = L_b; otherwise up = U_b + n·U_m,
  side = L_b + n·L_m.
- punch pitch −= up, then not below −U_max.
- The gun's sideways direction flag (starts at 0) picks the side:
  1: punch yaw += side, capped at +L_max; 0: punch yaw −= side, capped
  at −L_max.
- Then with probability 1 / (D + 1) the flag flips (the server's own
  random numbers).

The state is chosen in the gun's order. Rifles (AK-47, M4A1, Galil,
FAMAS, AUG, SG552): moving (2D speed > 0) first, then in the air, then
ducked, then standing. SMGs and the M249: in the air first, then moving
(2D speed > 0), ducked, standing.

| Gun | State | U_b | L_b | U_m | L_m | U_max | L_max | D |
|---|---|---|---|---|---|---|---|---|
| ak47 | moving | 1.5 | 0.45 | 0.225 | 0.05 | 6.5 | 2.5 | 7 |
| ak47 | air | 2.0 | 1.0 | 0.5 | 0.35 | 9.0 | 6.0 | 5 |
| ak47 | ducked | 0.9 | 0.35 | 0.15 | 0.025 | 5.5 | 1.5 | 9 |
| ak47 | standing | 1.0 | 0.375 | 0.175 | 0.0375 | 5.75 | 1.75 | 8 |
| m4a1, galil | moving | 1.0 | 0.45 | 0.28 | 0.045 | 3.75 | 3.0 | 7 |
| m4a1, galil | air | 1.2 | 0.5 | 0.23 | 0.15 | 5.5 | 3.5 | 6 |
| m4a1, galil | ducked | 0.6 | 0.3 | 0.2 | 0.0125 | 3.25 | 2.0 | 7 |
| m4a1, galil | standing | 0.65 | 0.35 | 0.25 | 0.015 | 3.5 | 2.25 | 7 |
| famas, aug | moving | 1.0 | 0.45 | 0.275 | 0.05 | 4.0 | 2.5 | 7 |
| famas, aug | air | 1.25 | 0.45 | 0.22 | 0.18 | 5.5 | 4.0 | 5 |
| famas, aug | ducked | 0.575 | 0.325 | 0.2 | 0.011 | 3.25 | 2.0 | 8 |
| famas, aug | standing | 0.625 | 0.375 | 0.25 | 0.0125 | 3.5 | 2.25 | 8 |
| sg552 | moving | 1.0 | 0.45 | 0.28 | 0.04 | 4.25 | 2.5 | 7 |
| sg552 | air | 1.25 | 0.45 | 0.22 | 0.18 | 6.0 | 4.0 | 5 |
| sg552 | ducked | 0.6 | 0.35 | 0.2 | 0.0125 | 3.7 | 2.0 | 10 |
| sg552 | standing | 0.625 | 0.375 | 0.25 | 0.0125 | 4.0 | 2.25 | 9 |
| m249 | air | 1.8 | 0.65 | 0.45 | 0.125 | 5.0 | 3.5 | 8 |
| m249 | moving | 1.1 | 0.5 | 0.3 | 0.06 | 4.0 | 3.0 | 8 |
| m249 | ducked | 0.75 | 0.325 | 0.25 | 0.025 | 3.5 | 2.5 | 9 |
| m249 | standing | 0.8 | 0.35 | 0.3 | 0.03 | 3.75 | 3.0 | 9 |
| mp5navy | air | 0.9 | 0.475 | 0.35 | 0.0425 | 5.0 | 3.0 | 6 |
| mp5navy | moving | 0.5 | 0.275 | 0.2 | 0.03 | 3.0 | 2.0 | 10 |
| mp5navy | ducked | 0.225 | 0.15 | 0.1 | 0.015 | 2.0 | 1.0 | 10 |
| mp5navy | standing | 0.25 | 0.175 | 0.125 | 0.02 | 2.25 | 1.25 | 10 |
| tmp | air | 1.1 | 0.5 | 0.35 | 0.045 | 4.5 | 3.5 | 6 |
| tmp | moving | 0.8 | 0.4 | 0.2 | 0.03 | 3.0 | 2.5 | 7 |
| tmp | ducked | 0.7 | 0.35 | 0.125 | 0.025 | 2.5 | 2.0 | 10 |
| tmp | standing | 0.725 | 0.375 | 0.15 | 0.025 | 2.75 | 2.25 | 9 |
| mac10 | air | 1.3 | 0.55 | 0.4 | 0.05 | 4.75 | 3.75 | 5 |
| mac10 | moving | 0.9 | 0.45 | 0.25 | 0.035 | 3.5 | 2.75 | 7 |
| mac10 | ducked | 0.75 | 0.4 | 0.175 | 0.03 | 2.75 | 2.5 | 10 |
| mac10 | standing | 0.775 | 0.425 | 0.2 | 0.03 | 3.0 | 2.75 | 9 |
| ump45 | air | 0.125 | 0.65 | 0.55 | 0.0475 | 5.5 | 4.0 | 10 |
| ump45 | moving | 0.55 | 0.3 | 0.225 | 0.03 | 3.5 | 2.5 | 10 |
| ump45 | ducked | 0.25 | 0.175 | 0.125 | 0.02 | 2.25 | 1.25 | 10 |
| ump45 | standing | 0.275 | 0.2 | 0.15 | 0.0225 | 2.5 | 1.5 | 10 |
| p90 | air | 0.9 | 0.45 | 0.35 | 0.04 | 5.25 | 3.5 | 4 |
| p90 | moving | 0.45 | 0.3 | 0.2 | 0.0275 | 4.0 | 2.25 | 7 |
| p90 | ducked | 0.275 | 0.2 | 0.125 | 0.02 | 3.0 | 1.0 | 9 |
| p90 | standing | 0.3 | 0.225 | 0.125 | 0.02 | 3.25 | 1.25 | 8 |

Other guns: USP, P228, Deagle, Elites, Five-SeveN, AWP, Scout: punch
pitch −2 per shot (no cap, no yaw). Glock: no punch. G3SG1, SG550 and
the shotguns: above.

The punch decays per command: its length L becomes
max(L − (10 + 0.5·L)·dt, 0) (movement.md). The shot's direction uses
view angles + punch at the moment of firing, so the punch is the
recoil pattern itself.

## Knife

- **Slash** (primary): a line from the eye 48 units along the view
  (view angles without punch). If it misses, a sweep of a ducked-size
  box (±16, ±16, ±18) along the same line; if that hits the world or a
  brush entity, the hit point is refined to the nearest hit among lines
  from the eye to the 8 corners of a ducked-size box around the line's
  end point doubled. Damage 15 (see Quirks), hit-zone multipliers apply.
  On a hit: next primary 0.4 s, next secondary 0.5 s; on a miss: 0.35 s /
  0.5 s. The two slash animations alternate.
- **Stab** (secondary): the same trace with 32 units; damage 65; on a
  player, if the attacker's flat view direction and the victim's flat
  facing direction have a dot product above 0.8 (the attacker is behind,
  facing the same way), × 3 = 195. On a hit: next primary and secondary
  1.1 s; on a miss 1.0 s.
- The knife's hits are bullet-type damage from the attacker: hit zones,
  armour (× 1.7 ratio, damage.md) and team rules apply. Head stab
  65 × 4 = 260.
- Animations (install): slash `midslash1`/`midslash2` 66 frames at 55 =
  1.2 s; `stab` 47 at 35 = 1.343 s; `stab_miss` 61 at 40 = 1.525 s;
  `draw` 46 at 45 = 1.022 s.

## Grenades

### Throwing (all three)

- Primary press (with ammo, not already pulled, shield lowered): pull
  the pin (pull-pin animation, 41 frames at 41 = 1.0 s); the throw
  cannot happen before 0.5 s.
- The throw happens when the fire button is released and the 0.5 s has
  passed. Throw pitch p' from the view pitch p (+ punch): looking up
  (p < 0): p' = −10 + p × 80/90; else p' = −10 + p × 100/90. Speed =
  min((90 − p') × 6, 750). Direction from (p', yaw). Start: eye + 16
  along that direction. Velocity = direction × speed + the player's
  velocity. Then next primary 0.5 s; the last grenade's weapon is
  removed after the throw animation.
- Looking level: thrown 10° up at 600 units/s.
- Fuse: 1.5 s from the throw, checked by a think every 0.1 s.
- Grenades bounce (the engine's bouncing movement) with gravity 0.55
  (HE) or 0.5 (flash, smoke) × `sv_gravity`; each touch while on the
  ground × 0.8 speed; after 10 bounces in the air they stop; in water
  their speed halves every 0.1 s.

### HE grenade

At the fuse: a trace 40 units down from 8 above the grenade; if it hits,
the explosion point is pulled out from the surface along its normal by
(100 − 24) × 0.6 = 45.6 units. Damage 100 at the point, radius 350:
every damageable entity whose origin (a player's box centre) is within
350 takes 100 − distance × 100/350, **walls do not block it**. Not into
or out of water (both sides must agree). Explosion damage to players
uses the explosion armour rule (damage.md). Thrower and team: with
friendly fire off, teammates are not hurt, the thrower is.

### Flashbang

At the fuse (same pull-out, 6.6 units): every living player within 1500
whose body point (box centre + eye offset × U(0.5, 1.1)) can be seen
from the grenade + 1 unit up, and who can see back to it, is blinded
(same water rule). With d the distance to the point, strength
s = 4 − d × 4/1500 (≥ 0). If the grenade is in front of the player's
view (dot of the view direction with the direction from the player's
ears to the grenade ≥ 0): full white (alpha 255), fade over 3s seconds
after holding for s/1.5 seconds; if behind: alpha 200, fade 1.75s, hold
s/3.5. A new flash on a player still blinded keeps the longer fade and
the stronger alpha and adds the remaining hold to a facing flash's hold.
Teammates and the thrower are flashed too. No damage. At 0 distance,
facing: hold 2.67 s, fade 12 s.

### Smoke grenade

Detonates at the fuse only once it is resting on the ground (checked
every 0.1 s after). Then the grenade hops (random velocity ±175, ±175,
250-350 up), and the cloud is emitted by the client at the detonation
point: a puff every 1 s, 21 times (about 20 s of emission). A second
smoke within 250 units of an already detonated one makes a lighter
cloud. The cloud's look is the client's (open question).

## Models and sounds

Each gun uses `models/v_<id>.mdl` (view), `models/p_<id>.mdl` (in the
hands of others), `models/w_<id>.mdl` (dropped), with these exceptions:
the MP5 is `v_mp5`/`p_mp5`/`w_mp5`; with a shield, pistols, knife and
grenades use `models/shield/v_shield_<id>.mdl`. Fire sounds are
`sound/weapons/<id>-1.wav` (and `-2`), e.g. `ak47-1.wav`; the knife's are
`knife_slash1.wav`, `knife_slash2.wav`, `knife_hit1..4.wav`,
`knife_hitwall1.wav`, `knife_stab.wav`. Shell models: `rshell` (rifle
shells), `pshell` (pistol), `shotgunshell`, `rshell_big`; which gun
throws which, when and how: [shells.md](shells.md).

## Test cases

| Setup | Input | After | Expected |
|---|---|---|---|
| AK-47 deployed, standing still on ground | hold fire | shot 1 | spread 0.0275 × 0.2 = 0.0055; accuracy afterwards 0.35 |
| AK-47 | hold fire | shots 2-6 | spread 0.0275 × 0.35 = 0.009625 each; accuracy after shot 6: 1.25 |
| AK-47 | hold fire | shot 7 on | spread 0.034375 |
| AK-47, 10 ms commands | hold fire | 1 s | 11 shots (one every 10 commands, starting at 0) |
| AK-47, standing | first shot | | punch pitch −1.0, yaw −0.375 (direction flag 0) |
| AK-47, standing | second shot (punch not decayed) | | pitch −1.0 − (1.0 + 2 × 0.175) = −2.35 |
| M4A1 silenced | toggle | | 2.0 s before the next shot |
| Deagle | two shots 0.225 s apart | second shot | accuracy 0.9 − (0.4 − 0.225) × 0.35 = 0.83875; spread standing 0.13 × (1 − 0.9) = 0.013 for that shot (it used the first shot's value), 0.021 for the third |
| Glock burst, 10 ms commands | press fire once | | 3 bullets on 3 consecutive commands |
| AWP unzoomed, standing still | fire | | spread 0.081 |
| AWP zoomed, ducked, still | fire | | spread 0; view to 90; back to the zoom after 1.45 s |
| HE at 175 units from an unarmoured player | | | 50 damage |
| HE at 350 or more | | | none |
| Flash at 0 units, facing | | | white 255, hold 2.67 s, fade 12 s |
| Knife stab from behind, no armour, chest | | | 195 |
| Knife slash, chest, no armour | | | 15 |
| M3 point blank (distance 0) all 9 pellets in the chest | | | 180 (one damage event) |
| Grenade thrown looking level | | | speed 600 + player velocity, 10° above the horizon |
| reload AK-47 with 10 in the clip, 90 reserve | press reload | 2.45 s | clip 30, reserve 70 |

## Quirks

- **Whole-number accuracy division**: the original game divides the
  shots count's power by a whole number, so most automatic guns' accuracy
  is a step function (AK-47 0.35 for shots 1-5, then capped). The
  reimplementation's optional fix makes it continuous; parity keeps the
  steps. To verify (Open questions).
- **Stale accuracy**: after a spray the accuracy value stays at its
  capped value until the next shot, so the first shot of the next burst
  still uses the spray's accuracy (the shots count has recovered, so
  the second shot is accurate again). Keep.
- The knife's "faster first slash" damage (20) never applies: the test
  for it compares against the attack timer just set. Slash is always 15.
- Rifles check "moving" before "in the air" for the kick: a moving
  player in the air uses the moving kick.
- The Glock's burst follow-ups are one per command: their spacing is
  the frame time.
- Cycle times round up to whole commands.

## Open questions

1. Measured in the running game: none of the firing numbers yet. A
   client is needed (installed, but it needs a running Steam client and
   32-bit libraries missing here); then: shot intervals at 100 fps,
   spray patterns from decals at a wall (checks the whole-number
   accuracy and the kick tables), reload and draw times by ear and HUD.
2. Buy sides and ammunition prices are from the reference's tables and
   common knowledge of the game, not checked in the buy menu.
3. The FAMAS burst follow-ups' exact times (0.05 s then 0.1 s after the
   first) depend on the server's clock checks; confirm by recording.
4. The engine's bounce for grenades (how much speed a bounce keeps, the
   role of the grenade's friction 0.7/0.8) is engine behaviour; measure
   by throwing at a wall.
5. The smoke cloud's size and timing are the client's; capture it.
