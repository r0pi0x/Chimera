# Plan: Halo: Combat Evolved parity checklist (milestone M5)

Written 2026-11-15 for [halo-ce.md](halo-ce.md), M5 ("feel and parity
pass"): a checklist for a player with a controller and a capture
device, and a work list for the agent who turns results into fixes.

## 1. What this is

- Compared: the real game (NTSC Xbox disc, build 01.10.12.2276, on the
  console or an emulator) against `specs/halo_ce/` and the port built
  from it (`games/halo_ce`).
- The specs' numbers come from the disc; their rules come from a
  reference that describes another build (PAL, 01.01.14.2342); their
  timelines are simulations, not measurements (each spec's Confidence).
- The rule: the real game wins. A difference is fixed in the spec
  first, by a spec writer, then in code; code is never tuned alone.
- Units: WU = world unit = 3.048 m; a tick = 1/30 s = 2 captured frames
  at 60 fps. "Spec" quotes `specs/halo_ce/<file>`; "derived" means the
  arithmetic is ours, from spec numbers.

## 2. How to capture

**Video.** 60 frames (or 60 fields, one picture per field) a second,
with sound, constant frame rate, no filters. The game is assumed to
draw 30 pictures a second (effects.md Open questions 1), so a tick
shows as two identical frames; if every frame differs, note it. One
tick (2 frames) is the resolution: measure long spans and divide.

**Counting frames.** `ffmpeg -i clip.mp4 -fps_mode passthrough
f_%06d.png` (older ffmpeg: `-vsync 0`) numbers every frame; or step a
player frame by frame. Ticks = frames / 2. Put the frame numbers and
what each shows in the clip's notes.

**A clock in the picture.** The motion tracker's sweep repeats every
63 ticks = 126 frames (hud.md Constants). Keep the tracker in view in
timing clips and count one sweep: 126 means the capture is true and
the game ticks at 30 Hz. That period is itself unmeasured, so check it
once against the capture device's own timecode. The shield bar is a
second clock (SHD-02).

**Reference objects.** A second controller gives a second player: 0.7
WU tall standing, 0.5 crouched, 0.4 wide (movement.md Constants). It
also means split screen, which changes the HUD and view (hud.md 15):
take the LOK items with one controller. For distances name landmarks (a
base wall, a rock, a doorway) in the notes; a later agent measures the
span in WU from the map's collision data. One known span: Blood
Gulch's flag spots are 97.6 WU apart in a straight line (derived from
scenario.md, Blood Gulch in detail). Repeat each item three times and
take the median.

**Game settings** (the disc's setup menu, multiplayer_rules.md 1.5 and
2). The disc has no time limit: a game ends by score, lives or players
leaving (1.6). Start from stock Slayer (radar and friend indicators,
respawn instant, suicide penalty 10 s, lives infinite, health 100%,
weapon set normal) or Slayer Pro (adds generic starting equipment:
assault rifle 60+180, pistol 12+48, 4 frag grenades). Set slayer kills
to win to 50, the largest choice, and stay under it. Change only what
an item says: starting equipment (custom or generic), weapon set (6
rocket launchers, 4 sniper), infinite grenades, shields, team play,
respawn time (instant, 5, 10, 15 s), vehicle set (none; stock Slayer's
set 2 puts a Warthog on Blood Gulch).

**Maps.** Hang 'Em High: small, no teleporters, no vehicles; assault
rifle, plasma rifle, rocket launcher, shotgun, sniper, needler, pistol,
powerups. Blood Gulch: long spans, sky, fog, 2 teleporter pairs,
vehicles. Chill Out: 3 teleporter pairs. Boarding Action: 40 ladder
sheets (Prisoner, Sidewinder, Wizard 4 each). Carousel: the one
flamethrower. Damnation, Putput: plasma pistols (11.3 of
multiplayer_rules.md lists every map's items).

**Naming.** `<ID>_t<n>.mp4` (for example `MOV-01_t2.mp4`) plus
`<ID>_t<n>.txt`: map, game settings, landmarks, frame numbers, buttons.
A clip may serve several items: name it by the first, list the others.

**Our side.** `halo_ce --map halo_ce:<map> --offscreen 1280x720 --dump
<dir> --dump-every 1` writes frames and `audio.wav`;
`tools/dump_to_mp4.py <dir>` makes a clip (docs/OBSERVABILITY.md 2).
`refcmp` and `movecmp` drive Counter-Strike over RCON and cannot read
an Xbox; `tools/frame_cut.py` ranks slow frames of our own runs and
does not help with captures. If needed, the later agent adds: a script
from frame numbers to ticks and a results table; our camera placed on
a named landmark.

**Agreement.** Tick counts within 1 tick (2 frames), or 3% over 100
ticks. Speeds and lengths within 3%. Counts exact. Colours: describe.
Anything else is a difference.

## 3. The checklist

Each item: Do (for a player), Read (from the capture), Spec (the
prediction, with source), Why (what it settles).

### Movement (MOV)

MOV-01 Top speeds. Do: on a level straight span run forward, backward,
  sideways, and diagonally (forward + right), full stick, from
  standing; three runs each. Read: frames from first movement to the
  far landmark. Spec: 2.25, 2.0, 2.0 WU/s; diagonal 2.129 WU/s
  (movement.md Constants, Ground speed): ratios 1 : 0.889 : 0.889 :
  0.946. Why: speed units (reference only).
MOV-02 Start, stop, reverse. Do: cross the span from standing and from
  a running start; run and release the stick; run and push fully
  back. Read: frame differences; where you stop against a floor seam.
  Spec: standing start trails by 0.2263 WU = 3 ticks = 6 frames; stops
  on tick 8 (16 frames), 0.2263 WU on; reversal moves back from tick
  8, full speed on tick 14 (movement.md Ground speed). Why: linear
  ramp 0.32 WU/s per tick, no friction.
MOV-03 Jump. Do: stand beside the second player; jump straight up;
  then hold jump for 10 s. Read: frames take-off to landing; feet
  against their body; frames between take-offs. Spec: apex 0.6526 WU
  (93% of a body) on tick 19, landing tick 39 = 78 frames; held: jumps
  on ticks 0, 44, 88 (Jumping and air control). Why: gravity, jump
  speed, 6-tick recovery.
MOV-04 Running jump, air control. Do: run and jump holding forward;
  run, jump and release the stick; standing jump holding forward.
  Read: air time, span. Spec: 39 ticks, 2.925 WU (8.9 m); released:
  lands at 0.0295 WU/t, about 2.0 WU flown (derived); standing jump
  forward 0.91 WU (Jumping; Open questions, Feel check, Air braking).
  Why: floaty jump; air acceleration 1/9 of ground.
MOV-05 Crouch. Do: crouch and stand on level ground; cross the span
  crouched; then crouch with the stick pushed to the stop, then
  slightly relaxed. Read: frames button to view settled; crossing
  time against MOV-01; crouched or not. Spec: eye 0.62 to 0.35 WU in
  6 ticks (12 frames); crouched forward 0.9 WU/s (0.4 of run); a stick
  at 0.98 or more cancels the crouch, 0.97 crouches at 0.0291 WU/t
  (Crouch; Open questions, Crouch latency, Controller crouch rule).
  Why: crouch speed, latency, input rule.
MOV-06 Slopes. Do: run up and down one smooth slope of 30 deg or
  more, and a gentle one under 20 deg. Read: times against MOV-01.
  Spec: under 20 deg full speed; 30 deg up 0.849, down 1.108 of run
  speed; 40 deg up 0.712, down 1.206; above 45 deg you slide (Slopes).
  Why: slope scales 0.65 and 1.25. Needs the slope's angle from
  collision data.
MOV-07 Small lips. Do: run into curbs and ledges of small heights.
  Read: climbed, hopped over, or stopped. Spec: 0.10 and 0.12 WU
  climbed; 0.13 and 0.15 WU: a hop up to 0.046 WU, then blocked; limit
  0.1231 WU (Steps and edges). Why: settles the capsule-cast shortcut.
  Needs a list of lips (spot, height); no prediction until then.
MOV-08 Landing and falls. Do: drop from ledges of known height, with
  full shield and health; try to move at once. Read: frames you
  cannot move; damage. Spec: soft landings 5 ticks at 1 WU, 10 at 2,
  14 at 3, 15 at 3.1; any hard landing (over 3 WU) 30 ticks = 60
  frames; damage 0 at 3.0 WU, 18.3 points at 3.1, 61.0 at 4.0, 112.2
  at 5.0, 155 (dead) at 6.5 or more (movement.md Landing and falling;
  Open questions, Hard landing length, Falling damage). Why: hard
  landing's fixed 1 s; lethal height. Heights from collision data.
MOV-09 Teleporter. Do: on Blood Gulch or Chill Out run at full speed
  into a teleporter; then walk in. Read: exit direction and speed in
  the first frames. Spec: world velocity is kept, not turned with the
  new facing (Quirks; Open questions, Teleporter velocity). Why:
  reference-only rule. Hang 'Em High has none.
MOV-10 Ladders. Do: on Boarding Action climb looking up, then level,
  then down; descend. Read: ticks per 2.5 WU sheet. Spec: up 1.4625
  WU/s (1.7 s a sheet, full speed in 5 ticks), down 2.81 WU/s; any
  upward pitch climbs equally, level aim does not move (Climbable
  surfaces). Why: reference-only rules; mounting from the floor.
MOV-11 Standing on a player. Do: jump onto the second player standing
  still. Read: do you stand, how high. Spec: players stand on each
  other's collision models (Open questions, Standing on players).
MOV-12 Turn rate. Do: hold the stick fully right for a turn. Read:
  frames for 360 deg. Spec: no prediction; default look rates 120 deg/s
  yaw, 60 pitch, inferred (globals.md Open questions 5).

### Shields and health (SHD)

SHD-01 Recharge delay. Do: take one assault rifle bullet from the
  second player, then stay out of fire. Read: frames from the hit to
  the bar's first rise (or the charge sound). Spec: 180 ticks = 6.0 s
  = 360 frames, plus at most 1 tick (health_shields.md Open questions
  1). Why: 6.0 s against the common "about 5".
SHD-02 Recharge length. Do: have the shield emptied (a sniper body
  shot), then wait. Read: frames empty to full; hit to full. Spec:
  120 steps, full on the 121st tick (242 frames); from the hit 300
  ticks = 10.0 s (Test cases). Why: rate, linearity.
SHD-03 What resets the delay. Do: hit as SHD-01, again 4 s later; hit
  once during recharge; melee, grenade near miss. Read: frames from
  the last hit to the rise. Spec: each hit restarts the 180 ticks
  (second hit on tick 250: rise on 430). Why: which events count.
SHD-04 Overshield. Do: pick one up (Hang 'Em High's camo-or-overshield
  item, 180 s, a 50:50 draw); let the partner fire in the first
  second. Read: frames to full; colours; time back to normal. Spec:
  3.0 after 61 ticks (2.03 s); bar blue, red 1 to 2, green 2 to 3;
  invulnerable while charging; the extra drains over 2700 ticks = 90
  s (health_shields.md Test cases; hud.md 6, Open questions 16).
SHD-05 Camouflage. Do: pick it up; stand; fire pistol, assault rifle,
  shotgun in turn; be shot. Read: frames to full fade; seconds it
  lasts; frames to refade after a shot. Spec: fade 121 ticks (4.0 s);
  lasts 45 s; a shot costs 0.65 (pistol), 0.25 (rifle), 1.0
  (shotgun), regrowing 0.01, 0.04, 0.036 a tick: 65, 6, 28 ticks
  (derived; weapons.md, other fields); a bullet hit leaves 0.75; the
  tracker still shows you (multiplayer_rules.md 7).
SHD-06 Health. Do: lose health, take a health pack; touch one at full
  health. Spec: body never regenerates; a pack restores it in one
  step, shield unchanged; not taken at full (health_shields.md Test
  cases).
SHD-07 Credit for a fall. Do: shoot the partner once, let them jump
  off a cliff within 6 s; then with no hit. Read: whose score moves.
  Spec: hit within 6 s: the shooter scores; else a suicide, -1
  (health_shields.md Open questions 6; multiplayer_rules.md Test
  cases).

### Weapons (WPN)

WPN-01 Rate of fire. Do: hold the trigger on a wall for 5 s with each
  weapon (tap fast for the sniper rifle and plasma pistol). Read:
  shots, frames between the first six. Spec (weapons.md Test cases):

| Weapon | Spec |
|---|---|
| assault rifle | ticks 0, 2, 4, 6, 8, 10; 15 in 1 s |
| pistol | ticks 0, 9, 18, 27; 4 in 1 s; 12 in 5 s |
| shotgun | 30 ticks apart, 1 a second |
| sniper rifle | tapped: ticks 0, 16, 32, 48 |
| plasma rifle | ticks 0, 5, 10, 14, 18, 22, then every 4 |
| plasma pistol | tapped: ticks 1, 3, ..., 17 (9 shots) |
| needler | spacing 10 falling to 3 ticks over 0.5 s; no timeline |
| rocket launcher | 60 ticks apart; magazine 2 |
| flamethrower | 2 ticks (15/s) after a 0.2 s ramp; no timeline |

WPN-02 Plasma rifle's real rate. Do: fresh plasma rifle, hold 4 s.
  Read: bolts in the third second. Spec: 7.5/s (every 4 ticks); the
  tag's 10/s is never reached (Quirks 1; Open questions 1). Why: the
  weapon's biggest doubt.
WPN-03 Overheat. Do: hold the plasma rifle until it locks, keep
  holding. Read: shots; frames fire to lock; lock to firing. Spec: 25
  shots; locks on tick 99 (3.3 s); fires again on tick 175: 152
  frames locked (Test cases).
WPN-04 Plasma pistol. Do: tap fast; hold 0.3 s; hold 1 s; hold 60 s;
  release each. Read: bolts, frames release to bolt, lock-out. Spec:
  tapping: 9 shots, locked on tick 18, ready on 52; a hold under 18
  ticks fires a normal bolt on release; 18 or more fires the charged
  bolt on release, locks the next tick, frees 35 ticks after; 60 s:
  nothing happens, release fires (Test cases; Open questions 4).
WPN-05 Reload. Do: empty each magazine, release the trigger; once
  part-empty. Read: frames from the last shot to the HUD's new
  ammunition and to the next shot. Spec (rounds in / can fire):
  assault rifle 87 / 89; pistol 70 / 72; sniper 97 / 99; needler 70 /
  72; rocket 125 / 127; flamethrower 104 / 106; part-empty takes as
  long (weapons.md 11). Why: tag time 3.4 s against animation 2.9 s.
WPN-06 Cut short. Do: start a reload and melee at about 30%, again
  at 70%; empty the shotgun and reload it, once cut by the trigger.
  Read: ammunition; frames per shell. Spec: over half done completes
  at once, under half loses it; shotgun: first shell tick 15, then
  every 11 ticks (22 frames), 12th on 136; no reload until 22 ticks
  after a shot (Quirks 4; Test cases).
WPN-07 Draw. Do: switch weapons and fire at once. Read: frames switch
  to first shot. Spec: assault rifle 28, pistol 34, shotgun 22, plasma
  rifle 28, sniper 28, needler 22, rocket 21, flamethrower 29 ticks;
  plasma pistol: no prediction (weapons.md 2).
WPN-08 Melee timing. Do: with each gun melee the partner or a wall.
  Read: frames press to melee sound, and to the next melee. Spec: hit
  tick / next: assault rifle 4 / 29; pistol 5 / 35; shotgun 5 / 28;
  plasma rifle 7 / 28; sniper 7 / 28; plasma pistol 5 / 26; needler
  7 / 37; rocket 5 / 40; flamethrower 9 / 40 (weapons.md 13).
WPN-09 Spread. Do: from one spot fire at a wall: assault rifle 40
  held, pistol held, shotgun, plasma rifle, needler. Read: cluster
  width in pixels; degrees = pixels / picture width x horizontal field
  of view (LOK-01). Spec: half-angle limit, deg: assault rifle 2.0 +
  0.5 a shot to 6.5 (shot 10); pistol 0.2 to 2.0 over 15 ticks;
  shotgun 10 for each of 15 pellets; plasma rifle 0.5 to 5.0 over 75
  ticks; needler 2 to 4; sniper 0.5, zero zoomed (weapons.md Test
  cases, Multiplayer weapon numbers). Why: error model.
WPN-10 Zoom. Do: zoom the pistol; zoom the sniper rifle three times;
  fire zoomed; melee zoomed. Read: size of a wall feature at each
  step; HUD labels. Spec: pistol 2x: vertical field 48.11 to 22.73
  deg, features grow 2.22 times (derived); sniper 2x, 8x (5.58 deg,
  9.2 times, derived), out; label "10x"; pistol equally accurate
  zoomed; melee ignored zoomed (weapons.md 12; Quirks 6, 8).
WPN-11 Pickup. Do: stand over a second weapon, tap, then hold the
  action button; walk over the same weapon. Read: frames to swap.
  Spec: a hold of 7 ticks (14 frames) swaps; walking over adds only
  ammunition (weapons.md Open questions 8, 9; multiplayer_rules.md
  Open questions 12).
WPN-12 Rocket line. Do: aim a rocket at a wall mark from near and far.
  Read: impact against the crosshair. Spec: leaves 0.1 WU (0.3 m)
  right of the camera line, flies parallel: a constant offset, no aim
  bend (Quirks 10).
WPN-13 Armed walking, holstered heat. Do: walk the MOV-01 span with
  the rocket launcher (zoomed too) and the assault rifle; overheat a
  plasma rifle, switch away 5 s, back. Spec: no speed penalty (Open
  questions 7); holstered plasma weapons keep cooling (10).

### Damage (DMG)

Slayer, second controller, full shield and health, point-blank and at a
span you can pace twice (assault rifle, pistol, sniper bullets lose no
damage; range 40, 40, 1000 WU). Read: shots to break the shield (bar
empty) and to kill. Head counts only once the shield is gone.
Spec: health_shields.md, Shots to kill, unless said.

DMG-01 Pistol: body 3 break / 5 kill; legs 3 / 6; head 3, third kills
  (Quirks 1: needs 32-bit rounding).
DMG-02 Assault rifle: 8 break, 16 kill, head the same.
DMG-03 Plasma rifle: body 3 to 4 / 14 to 15; head 8 to 9 kill; a bolt
  does 11.7 to 13.5 at 5 WU, 10.8 to 11.7 at 20 WU (projectiles.md b).
DMG-04 Plasma pistol: uncharged 7 to 8 / 13 to 16; charged bolt: shield
  gone, body untouched, then one pistol headshot kills (or the third
  body shot).
DMG-05 Shotgun: one shell at point-blank kills (11 to 13 of 15
  pellets); full damage to 14 WU, 8 points a pellet at 20 WU, gone
  at 40 (projectiles.md b).
DMG-06 Sniper rifle: body 1 break / 2 kill; head 1 kills; legs leave
  body 0.7227.
DMG-07 Needler: 8 needles break, 16 kill; 7 stuck needles supercombine
  but a full Spartan survives (body 0.067; Open questions 3).
DMG-08 Rocket: dead from full, direct or at the feet; 300 to 330 at
  0.5 WU, 153 to 163 at 1.5 WU, none at 2.0 WU (projectiles.md d).
DMG-09 Frag grenade: stand on it, then at 1, 1.5, 2, 2.5 WU (1.4, 2.1,
  2.9, 3.6 body heights): 120 within 1.5 WU leaves body 0.4, a second
  kills; 100 at 2.0; none past 2.5 (projectiles.md d; Open questions 7).
DMG-10 Plasma grenade: stuck to a player: 120 attached then 120 area:
  dead; unstuck at the centre: body 0.4; none at 2.0 WU.
DMG-11 Melee: front standing 2 break / 3 kill; running 1 to 2 / 2;
  lunging 1 / 2; from behind kills at once; backing away equals
  standing (Open questions 9).
DMG-12 Rocket and grenade jumps. Do: rocket at your feet with an
  overshield, then without; frag at your feet. Spec: shield 3.0: left
  at 0, body 0, alive (301 points kills); push 0.07 WU/t tilted up =
  0.69 WU straight up (derived); frag push 0.047 = 0.31 WU
  (projectiles.md d).
DMG-13 Friendly fire. Do: Team Slayer; shoot, melee behind, grenade a
  teammate. Spec: no option on the disc; full damage (multiplayer_
  rules.md 1.6; Open questions 13).

### Projectiles (PRJ)

PRJ-01 Plasma rifle bolt. Do: fire at a far wall across a named span.
  Read: frames muzzle to impact. Spec: 1.65 WU/tick falling to 0.83;
  30 WU on tick 24 (0.8 s, 48 frames); vanishes at 30.8 WU; damage
  scale 0.87 at 5 WU, 0.41 at 20 WU (projectiles.md a, b).
PRJ-02 Needle. Do: fire across open ground at a standing, then a
  strafing player. Read: frames to arrive; bend. Spec: 0.133 WU/tick (4
  WU/s), gone at 20 WU on tick 150 (5.0 s); against a crossing
  player it turns 1.5 deg a tick: 13.5 deg on tick 20, 31.5 on 40
  (Test cases e); stuck needles burst 23 ticks (0.77 s) later. Why:
  homing strength (Open questions 2).
PRJ-03 Rocket. Do: fire base to base on Blood Gulch (97.6 WU, derived).
  Read: frames to the burst. Spec: 0.4 WU/tick falling to 0.33; 100.1
  WU on tick 273 (9.1 s); with no target it bursts at 128 WU on tick
  358.
PRJ-04 Frag fuse. Do: throw level on flat ground; drop one at your
  feet. Read: frames throw to burst. Spec: level throw: bounce tick 19
  at 6.2 WU, rest tick 34, burst tick 49 (1.63 s); dropped: tick 45
  (1.5 s); the fuse runs from rest (Test cases c; Quirks 2). Release
  delay after the button: no prediction.
PRJ-05 Plasma grenade. Do: stick one to the partner; throw one at the
  floor. Spec: stuck: bursts on the 61st tick (2.03 s); on the floor
  rests after two bounces, bursts about 61 ticks later; sticks to a
  Warthog (Test cases c).
PRJ-06 Bounce. Do: drop a frag on stone from a ledge; throw one down
  a slope. Spec: restitution 0.3 or 0.2 by the axis reading (Open
  questions 1); a slope keeps it sliding (5).

### Multiplayer rules (MPR)

MPR-01 Respawn. Do: die many times, Slayer (instant) and Team Slayer.
  Read: frames death to new body. Spec: 90 ticks minimum plus 0 to 31
  of stagger (3.0 to 4.0 s); Team Slayer 300 plus stagger; suicide
  adds 300 (multiplayer_rules.md 4.2; Open questions 1, 10).
MPR-02 Spawn choice. Do: the partner stands still on a start (Blood
  Gulch has 16) while you die 20 times. Read: ever within about 2 WU
  of them, or in their view. Spec: never within 2 WU of an enemy;
  nothing tests line of sight (4.4).
MPR-03 Item clock. Do: from the first second on Hang 'Em High take
  the assault rifle at (27.99, -2.55) and the rocket launcher at
  (15.34, -10.06); note when each is back. Spec: assault rifle every
  30 s of game clock (900 ticks) taken or not; rocket 120 s; powerups
  60 s and 180 s; an item nobody holds is deleted 30 s after last
  held (5.1; Test cases). Why: a clock, not a delay.
MPR-04 Dropped weapon. Do: drop one, wait. Spec: gone after 900 ticks
  (30 s).
MPR-05 Duplicate weapon. Do: spawn with the assault rifle, walk onto a
  second; pistol too. Read: ammunition. Spec: spare ammunition adds
  (max 600, pistol 120): 100 + 180 = 280 and the ground weapon goes;
  over the cap it stays with the remainder (Test cases).
MPR-06 Starting equipment. Do: start stock Slayer on Blood Gulch; Slayer
  Pro (2, 6, 12 players if you can); Rockets; Snipers. Spec: stock
  Slayer on Blood Gulch: one plasma pistol, no grenades; Slayer Pro:
  assault rifle, pistol, 4 frag (2 with 6 players, 1 with 12);
  Rockets: one rocket launcher 2+2 and 4 frag; Snipers: sniper 4+8
  and pistol (4.6; Test cases). Why: the odd stock case.
MPR-07 Score. Do: suicide; two kills within 4 s; 5 kills unkilled.
  Spec: kill +1, suicide -1; double kill inside 120 ticks; "killing
  spree" at 5 (Scoring and multi-kills).

### Look (LOK)

LOK-01 Field of view. Do: one controller; from a named spot face a long
  straight wall so two named features meet the screen's edges. Read:
  both features. Spec: vertical 48.11 deg; horizontal 61.52 deg at
  4:3 (not 70), 76.87 at 16:9 (weapons.md 12). Needs the angle
  between the features from the map data.
LOK-02 Gun on screen. Do: each gun idle at a wall; fire. Read: stills
  at our size. Spec: no prediction; compare position, size, flash
  length (two frames, effects.md Open questions 1).
LOK-03 Sky. Do: Blood Gulch, horizon and sky from both bases. Spec: sun
  yaw 10, pitch 40 deg, power 1.8 (scenario.md sky table); the ring:
  no prediction, compare stills.
LOK-04 Fog. Do: a dark object near and far, outdoors and in. Read:
  pixel colours. Spec: factor (depth - start) / (opaque - start) x
  density: Blood Gulch outdoor (1.0, 0.902, 0.769), 0.4, 3, 100 WU;
  Hang 'Em High (0.533, 0.643, 0.710), 0.2, 0, 32; indoor (0.196,
  0.173, 0.329), 1, 1, 120 (scenario.md, How a renderer uses the sky).
LOK-05 Light. Do: eight still spots per map. Spec: no prediction; our
  look lacks bump, glow, specular, reflections (section 4).
LOK-06 HUD. Do: stills: full, empty shield, health under a third,
  overshield. Read: pixel boxes, colours. Spec: 640 x 480 canvas;
  tracker window (48, 316) to (176, 444), centre (89, 403); health
  blue above 0.67, yellow, red below 0.33; shield flashes below 0.25
  (hud.md 6, 11).
LOK-07 Crosshair. Do: aim at an enemy, at a teammate, firing. Spec:
  blue; red on an enemy within half the autoaim range and angle;
  never for friends; no growth with spread (hud.md 8; Open
  questions 11).
LOK-08 Motion tracker. Do: the partner walks, crouch-walks, stands,
  fires at about 5, 10, 20, 25 WU. Spec: shown if moving (0.424
  WU/s), firing or throwing, within 20 WU level; enemy red, friend
  yellow, self orange; list refreshed every 15 ticks (hud.md 11;
  multiplayer_rules.md 7; Open questions 5, 10).
LOK-09 Arrows. Do: take hits from front, sides, back. Spec: none for
  the front; behind goes left or right (hud.md 12; Open questions 6).
LOK-10 Scope. Do: zoom the sniper at a bright wall. Spec: no
  prediction (hud.md Open questions 15).

### Sound (SND)

SND-01 Gunshot carry. Do: the partner fires at one end of Blood Gulch,
  you at the other (97.6 WU), then walk closer. Read: where it
  becomes audible. Spec: full to 4 WU, then 4/d (-6 dB a doubling),
  cut off past 70 WU with a 2 s fade: silent at 97.6; an explosion
  carries to 120 WU, 0.08 of full at 97.6 (derived; sounds.md Distance).
SND-02 Footsteps. Do: the partner runs straight away from beside you.
  Read: when the steps fade. Spec: full to 0.9 WU, then 0.9/d, cut
  off past 10 WU: the fade starts 4.4 s on (10 / 2.25, derived), ends
  2 s later (sounds.md class table).
SND-03 Handling sounds. Do: reload, switch near the partner at 5 and 12
  WU. Spec: ready and reload sounds cut off past 9 WU.
SND-04 Warnings. Do: take hits down to empty shield, then low health.
  Spec: hit one-shot; loop while recharging; low loop under 0.25;
  depleted loop at 0; heartbeat under 0.25 body (hud.md 6; Open
  questions 7). Played since 2026-11-21 (`game::audio::warnings`, from
  the unit HUD interface's own sound elements; whatever gun is in hand).

### Vehicles (VEH, milestone M6)

VEH-01 Warthog. Do: Blood Gulch flat, full stick from rest for 10 s;
  stick at 10% and 50%. Read: distance by frames. Spec: top 7.01 WU/s
  (21.4 m/s); 0.46 WU at 1 s, 2.66 at 2 s, 20.1 at 5 s, 55.1 at 10 s;
  stick 10% and 50%: about 0.09 and 0.234 WU/t (vehicles.md Test
  cases; Open questions 1, 2). Why: model unvalidated.
VEH-02 Stopping and reverse. Do: from top speed release, then brake;
  hold reverse 10 s. Spec: release stops in 86 ticks, 6.7 WU on; brake
  50 ticks, 6.0 WU; reverse 2.75 WU/s after 600 ticks.
VEH-03 Turning. Do: at top speed full lock with the camera aside; a
  small camera offset. Read: circle radius, heading after 10, 30
  ticks. Spec: full lock: path radius 0.44 WU, a spin; camera 10 deg
  left: heading 5.9, 14, settles 10 deg; the front steers toward the
  camera (Test cases; Open questions 3).
VEH-04 Ghost. Do: full stick forward, back, strafe; turn to the camera.
  Spec: 6.73 WU/s every direction, top by tick 60; 90 deg turn in
  about 40 ticks; stops in 43 ticks.
VEH-05 Scorpion. Do: full stick; camera aside; camera aside at rest.
  Spec: 4.15 WU/s (12.7 m/s), reverse 3.6; turns 133 deg/s at speed
  (radius 1.39 WU); no turn at rest (Open questions 6).
VEH-06 Enter, exit, flip. Do: enter and leave a Warthog and Ghost; tip
  a Warthog on its side with riders. Spec: seat offered within 1 WU
  and slower than 0.1 WU/t; riders thrown out when it lies on its
  side (up.z under 0.2); animation lengths: no prediction (vehicles.md
  Open questions 7, 8).
VEH-07 Indestructible. Do: empty rocket launchers into a parked
  Warthog and Scorpion. Spec: never explode; riders take 18% of hits.

## 4. Known differences already on file

Gaps you will see that a capture cannot settle (the "Known shortcuts"
in the `//!` docs of `games/halo_ce/src`). Our build does not yet:

- **Movement:** use the spec's sweep (a capsule cast: lip hops and
  blocked rows differ in detail); slip, elevators; the edge hold on
  ladders.
- **Shields:** read the game type's health, shields, invisibility;
  stun other games' hits; count assists; powerup pickups may not be
  wired (check before SHD-04, SHD-05).
- **Damage:** read friendly fire (the engine's rule decides); walk
  vehicle riders.
- **Weapons:** aim assist, magnetism; water; fire from the gun's marker
  (shots start at the eye); change grenade kind; bind the throw;
  items falling; weapon sets beyond 0; same-weapon swaps; death drops;
  cool a dropped plasma rifle. Others' grenades and guided needles
  are not shown on other machines.
- **Body, view:** death and ping animations; left-hand IK; pistol slide
  and pump; the rifle's ammunition counter (static); light, charge,
  throw overlays; first-person animation sounds.
- **Effects:** full-body shield glow; lens flares; particle collision
  and material effects.
- **Maps:** culling; bump, glow, specular, reflections; moving textures;
  detail in gamma; indoor and outdoor fog easing (one fog); the sky
  drawn by the engine's sky camera; per-spot scenery light, shadows;
  level of detail; breakable glass (solid); teleporter message, sound,
  shimmer.
- **HUD:** split screen; scope effects, night vision; flashlight meter,
  waypoints, friend markers, messages, grenade counts; the sweep's look is ours; tracker ignores `hud_radar`.
- **Sound:** Doppler, cones, obstruction, reverb (console hardware).

## 5. Recording results

Copy this table once per session; one row per item and take.

| ID | Measured | Predicted | Difference | Clip |
|---|---|---|---|---|
| MOV-01 | | 2.25 WU/s forward | | MOV-01_t1.mp4 |

Finished results go at the end of this file as "## Results, <date>":
the filled table, then one line per difference naming what the spec
writer must check. Spec corrections come first, then code with a test
that encodes the measured number; the plan's M5 row and Open questions
follow.

## 6. Low-shield and low-health cues (2026-11-21, specs/halo_ce/low_health.md)

Done: the five unit-HUD warning sounds (`game::audio::warnings`: the
unit HUD interface's sound elements compiled with the audio kit,
`AudioLibrary::warnings`; a state word from the local `Vitality`; loops
as unspatialized `LoopRequest`s at scale 0, the hit as a latched
one-shot; none while dead); the damage direction arrows (`ui::hud::draw`
`arrows`: the globals' `hud_damage_arrows` picture turned to the side,
red 0xC8FF0000, counters 1 to 29 ticks, from `DamageTaken`'s direction in
the camera's frame, `rules::arrow_sides`). The sounds are the unit's
state (any gun in hand); the arrows are the HUD's (Halo's gun in hand).

Done since (2026-11-22): the victim's screen flash, temporary and
permanent camera impulse and shake from the hit's damage effect
(`game::fx::victim`): every `jpt!` on the donor map is read into a
`DamageLook` at conversion (`HaloFx::damage`, by tag path; numbers from
the tags, none in the repo); the server's `HaloHurt` (now with the event's
scale and direction) reaches the victim as `HaloVictim`, locally or as
`HaloVictimNet` to that client only (effect index in `DamageEffects`, a
flags byte, scale, direction); the spec's intensity, truncation,
replacement, fade, falloff and wobble rules give `VictimView`, which
`halo_ce_ui::victim_view` draws as `ScreenPost` light and
`ViewEffects` rotation/offset; the permanent impulse turns the local
`Intent`. The arrows honour "indicators always point down" (matched to
their `DamageTaken` by frame). The overshield's drain above 1.0 no longer
counts as a fall for `shield_hit` (`ShieldMark` follows it, up to four
ticks' drain at once).

Still unspecified (spec questions): how the renderer blends each flash
type with the frame (ours: lighten/max add, darken/min multiply, tint
mixes, invert turns over; the colour's alpha unused); the per-weapon
victim-side values (low_health.md Open question 1; ours come from the
tags, unchecked against the original); whether the impulse's direction
is applied in the pitched camera frame (ours) or the yaw's; rumble is not
played. Not checked on screen here (no Halo data in the agent's slot).

## Campaign bodies' rank colours (2026-11-22)

Done: a playable Grunt, Jackal or Elite wears its rank's slot A colour
(`BodyDef::rank` / `ranks` in games/halo_ce/data/campaign.toml, values from
specs/halo_ce/change_colors.md section 5), a shade of `RANK_STEPS` drawn once
per body (`body::RankStep`), never the party's colour; Spartans keep it. The
model shader's mask formula (`prop.wesl`: `mix(1, colour, mask)` times the
diffuse before reflection) already matches section 3.

Open: the ranks are data copied from the spec, not read from the actor
variant tags (the character units' closure leaves `actv` out; reading them
needs a converter change and a `VERSION` bump). A rank per team (Grunt minor
orange vs major red) is not built: skins hold only the worn rank's shades.
The shade is drawn per body, not per respawn. The spec's open questions
(Grunt tag permutations, which shaders read which slot, mask coverage) stay
open: no Halo install was available to measure them.
