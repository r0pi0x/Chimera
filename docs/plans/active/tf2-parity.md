# Plan: TF2 at parity with real TF2, by side-by-side captures

Started 2026-11-19 (overnight). Goal: our TF2 (in Ultimate Gun Game; TF2
has no app of its own yet, docs/plans/active/tf2.md 7) looks and feels like
the real game, judged by matched screenshots of real TF2 and of ours.
Effects first (muzzle flashes, tracers, explosions, trails, flames, crits),
then whatever else a player notices (view models, HUD, crosshair, lighting,
player models). Gameplay numbers stay as the specs say; a capture that
contradicts a spec goes into that spec's Open questions.

## How (the tool)

`tools/tf2ref/tf2ref.py` (docs/OBSERVABILITY.md 6, "TF2 as the
reference"): real TF2 runs on this box with no display (SDL's offscreen
driver and our Vulkan layer `headless_vk.c`; Steam on a rootless Xwayland
over `nullcomp.c` when no X display answers), as a listen server on 2fort,
driven over RCON (port 27115). Matched scene pairs live in
`tools/tf2ref/scenes/`: `<x>.tf2` for real TF2 (`tf2ref.py scene`), `<x>.scene`
for ours (`ultimate_gun_game --offscreen 1024x768 +sandbox tf2:ctf_2fort
--scene ...`); both step effects at `host_timescale 0.01` and photograph
at game-time offsets (`gsleep` there, `gwait` here). `tf2ref.py pair`
writes reference | ours | difference per picture. Frames are 1024x768
(the offscreen display's size), fov_desired 90, viewmodel_fov 54.
Captured imagery stays in scratch, never in the repo.

## Progress (effect, status, remaining difference)

| Effect | Status | Remaining difference |
|---|---|---|
| Tool: headless TF2, scenes, pairs | done | – |
| Camera match (2fort bridge, setpos/setang) | matches | – |
| Muzzle flashes, first person | close (scattergun, shotgun, SMG) | size, colour and lifetime agree frame by frame at 60 fps (the "TF2 stays bright 2x longer" was two capture errors: TF2's `wait 2` is one frame, and ours' shot ran 13 ms in one step). The flash now rides the animated muzzle as TF2's does (`attached::AttachmentMark`); TF2's view model kicks harder (the Scattergun's pump) |
| Muzzle flashes, third person | close (Scattergun, Shotgun) | the scenes compare now: ours' target fires `ahead` (TF2's `bot_forceattack`) and our camera stays while our body leaves the line of fire (`chimera_freecam 2`). The flashes' size and life agree. TF2 draws another player's flash some 7 frames before its tracers and fire gesture (seen on the Scattergun and the Shotgun); ours draws them together |
| Minigun constant flash, brass | close (first person); close (third person) | first person: the core a compact glow; the brass lives 2 s. Third person (pass 5, from 260 units above): TF2's casings lie 40-75 units out to the gun's side (most ~58), ours ~25-55 (most ~43); from the side TF2's leave upward first. Ours' numbers (local x 100-200, drag 0.05, gravity −252) land ~40 out from a level eject: the difference reads as the eject attachment's tilt under TF2's spun pose (particles.md Open questions 6; the animation agent's pose) |
| Tracers | matches (SMG, Shotgun); from the front close | were a thick beam bright at the gun: the trail's texture ran backwards (TF2's `tracer1` is bright at v = 1, the head) and the radius is the full width, not the half (particles.md 5, Open questions 4 answered). From the front the geometry agrees (a streak passing ~20 units from the camera widens to tens of pixels in TF2 too); which pellets pass near is the spread's chance. Others' tracers now leave with the interpolated body (pass 4) |
| Rocket explosion on a surface | close | aligned on the blast, the same sequence (flash, fire with streaks, smoke, debris); ours a little paler in the first 50 ms. TF2's rocket took ~0.5 s to a floor 150 units ahead under host_framerate (ours 0.1 s): not understood, compared aligned on the blast |
| Rocket trail | close | added (none drew): `rockettrail` on every rocket, pipes', stickies', needles' trails |
| Projectiles' models | matches (pass 6) | none drew on any map: a map keeps the kept guns' held models by key prefix and the projectiles' keys (`tf2:rocket`, `tf2:sticky`...) matched none; now `<gun>/<part>` |
| Stickybombs | close (pass 6) | model, faint trail and the armed pulse (0.8 s, then every 0.1 s: ring, then disc) as TF2's; launch spin and jitter as the spec's (pass 7) |
| Sniper's laser dot | matches in his own view (pass 7, `dot.tf2`/`.scene`) | drawn (`weapons::dot`): size, softness and the core growing with the charge as TF2's; others see it at 6 units (the charge sent with what a character holds, `HeldGauge`), no TF2 pair (its puppet bots won't scope); TF2's scope overlay not photographed yet (the class menu covers it) |
| Water explosions | close (pass 6) | from above the surface both hide the burst; TF2's shows through as an orange glow from just above the surface. Under water (pass 7, `wunder`): see the log |
| Bullet impacts (wood) | matches (pass 6) | splinters, hole, puff alike; TF2's hole decal a little lighter |
| Blood on a player hit | still differs (pass 7) | TF2: a dark red puff ~20 px at 180 units and a red splat on the body; ours a speck (effects.md Open questions 8: no spec of TF2's blood) |
| Melee (Wrench at a wall) | close (pass 7) | the swing's path and timing alike; TF2 leaves a faint scratch, ours nothing visible |
| Backblast (third person) | close | from the side (pass 4) the same: an orange flame at the launcher's rear ~100 ms, then a dark grey puff; TF2's flame streakier. From the front the shoulder hides most of it |
| Bodies, idle poses (third person) | close (pass 7 of the animation: `pose.tf2` / `pose.scene`) | the "wider stances" were TF2's bots' feet yaw: bot_teleport never snaps a puppet bot's feet, and these spawn 30 degrees off a multiple of 45, so they stand 30 left of their eyes (wide) and, after the scene's teleports, 15 right (narrow); ours drawn with its feet at those offsets (`target set all feet <deg>`) stands as TF2's. The Engineer's Wrench taps flat and raised three times a loop in both (a phase-matched pair agrees). Every pose parameter 0 at a spawn, the hand and arm libraries only where a host layers them, the knees bent toward the include's knee directions under the foot locks, the Medic's real-time idle noise. Only the active weapon is drawn (TF2 and ours, the Spy with his Knife and Revolver from three sides) |
| Flame stream (air, wall); overbright 5 or 1 | close (pass 4: one stream from the nozzle, as TF2's) | overbright is 5, in linear space. The far fireball's "1.5x wider and whiter" is TF2's bloom (pass 5: with `mat_disable_bloom 1` its near-white area fell from ~8,900 to ~1,700-2,900 px; ours draws ~5,300 with no bloom; flames.md Open questions 1). Ours has bloom only at `mat_hdr_level` 1-2, a stand-in that overexposes 2fort (tried). TF2's cloud at a wall: close (pass 7: as wide at 0.5 s, ~25 % wider at 1 s); the orange tint on the right wall is gone (pass 6: the pilot's trail) |
| Detail sprites (2fort's hay) | close | drawn; kept whole at a distance (alpha raised by mip level); TF2 fades them blended where ours dithers |
| Explosion on a player / mid-air | close (pass 6: TF2 without bloom is pale in its first frame too; its streaks are longer at 33-50 ms; pass 7: measured, particles.md Open questions 7, still differs: TF2's rays are ~1 unit wide, ours radius-scaled 10-110 units, an orange haze) | the "hard-edged orange disc" was the sandbox's blast spheres (`range::blast_spheres`), which `cl_drawhud 0` didn't hide (now it does). Matched at 300 units, ours fills the far bridge with the same orange glow clipped by the near truss, the same sequence; ours a little paler yellow in the first 50 ms; TF2 shows more embers and a faint dark smudge after |
| Crits | n/a | random crits are off in our TF2 (the party's choice, docs/plans/active/tf2.md); crit tracers, trails and streams are named in `looks` but never asked for |
| View models (position, FOV, bob) | matches at rest (Scattergun, Shotgun, SMG, Revolver on our own player at fov 90; SMG at 75) | puppet bots seen in their eyes are no reference for view models: their FOV is 75 and their view model sits about 80 px right of a player's own (`viewmodels.tf2` kept for the world and the guns' look, not positions); this account's own loadout isn't stock for the Scout's secondary, the Pyro's and the Heavy's primaries |
| HDR, exposure, bloom (`mat_hdr_level` 2; specs/tf2/hdr_bloom.md) | close (pass 6, `hdr.tf2`/`hdr.scene`: bridge, wall, courtyard, bloom on and off) | the integer model (`map::HdrModel::Integer`): bloom adds 7.2-11.7 to the mean byte where TF2's adds 6.1-12.6; scenes 3-5 % brighter than TF2's in two views, 6 % darker in the courtyard (ours settles k at 0.96-1.04, TF2 reported 0.94); the far fireball's near-white area with bloom ~15,000 px against TF2's ~37,000 (its unbloomed core is smaller: the flame effect's, not the bloom's). TF2 maps now load at `mat_hdr_level` 2 by default (`SourceGame::hdr_default_level`; an explicit `mat_hdr_level` wins, -1 is "Game default") |
| View-model bob and sway (first person, own player) | close (2026-11-19, fp pass) | had none: TF2's guns registered no motion rules. Now CS:S's bob on a fixed 0.8 s cycle, no sway (`tf2::game::view::MOTION`, `view_motion::Sway::None`); walking pairs agree in amplitude and the hand's rise, within a few frames of phase |
| View-model draw, fire, reload (Scattergun, Rocket Launcher) | matches (Scattergun); close (Rocket Launcher) | the draw played at 1x: now 1.34 x, holding its last frame to its own length (`ViewAnimRules::draw_rate`); TF2 reloads a clip not full whenever attack is let go (`cl_autoreload` 1), ours only an empty one: now `[magazine] reload_when_idle`, and an attack stops a reload's start too (`shells.start_interruptible`). The Rocket Launcher's reload rises into its pose over ~0.25 s in TF2, ours jumps in (viewmodel.md Open questions 4). Our switch shows the old gun one frame longer |
| Camera: view punch, landing roll, eye heights, duck | matches | measured every tick through VScript on the live client: the Scattergun's kick (-2..-4, the punch spring), the landing roll 0.013 x F (a Scout's only above 580: now so, `loud_landings_only`), eyes 65/68, the duck's and jump's ticks exactly as movement.md |
| Movement (accel, jump, duck, fall) | matches | movement.md's cases 1, 5, 15, 16 exact on a real player, but the first tick of a key press: half input (33.75 for the Scout; movement.md Open questions 13, not changed) |
| HUD, crosshair | todo | |
| Lighting, materials | todo | |

## Next (for a successor)

1. The spun Heavy and player animation: another agent's (specs/tf2/
   player_animation.md, install.md Open question 6 answered; commit
   3bdaaae1d and on). Leave `heavy::bodies`/player animation to it.
   Its second pass (2026-11-19): the 2026-11-15 main-activity rules
   (airwalk only rising above 300 units/s, the jump's start cut at 0.5 s,
   restarts on landing and entering water, the land gesture only after a
   jump or airwalk); the aim cells' IK rule holding the Heavy's left hand
   on the Minigun (its on-disk layout in 4a, general in `map::anim`:
   `Animator::ik_rules`); the spin layers' bone weights are 0.5-1 as the
   file has them and the spec's spin timelines were recomputed with them
   (6.6 degrees at 0.2 s into the wind-down, not 9: the first reading
   took every weight as 1). Side-by-side with TF2's bots
   (`anim.tf2` / `anim.scene`, 2026-11-19): the Heavy firing matches
   (Minigun level and low, forward); TF2's spun-only and idle Heavy look as
   the spec has them, but ours' "idle" target fired anyway (item 5 below, fixed in pass 5),
   so no idle pair yet. The other classes' upper bodies match; TF2's legs
   look wider apart, most likely its bots' feet yaw (still standing, the
   feet keep their own goal up to 45 degrees off the eyes) against ours'
   feet on the eyes, not the stances (frame 0 matches, test 35). The
   Demoman's Grenade Launcher points forward in TF2 but across his body in
   ours although the animation's `weapon_bone` points forward (+Z along
   the model's +X): the held `c_` model's merge (7). Fixed (tf2-held,
   2026-11-19): the Demoman's player animations pose `weapon_bone_1` per
   kind, and ours played the Grenade Launcher with his primary set (the
   Stickybomb Launcher's), turning its barrel (merged onto
   `weapon_bone_1`) 94 degrees across him; the items' animation slots
   (weapons_more.md 1: the Grenade Launcher a secondary, the Stickybomb
   Launcher a primary) now pick the set (`Kind::of`). Also: parts of 11
   held weapons were merged onto the arms' own `vm_weapon_bone…` bones,
   which no player model has, and never drawn in third person (the
   Shovel's blade, the Knife's, the Shotgun's pump, the launchers' and
   rifles' parts); held models now merge only onto bones bodies have
   (`maps::is_view_bone`), and a body lacking a part's bone holds the
   model whole (`render::bodies::attach_held`: the Revolver's
   `weapon_bone_4` on the Spy). `heavy::player_anim` checks every
   weapon's merged bones against its class's pose standing, crouched,
   running and in the air. Pairs of all nine classes' slots and of
   crouching and jumping: `anim2.tf2` / `anim2.scene` (targets' `jump`,
   `crouch`, `run <dir>`; TF2's puppet bots can't be made to run over
   RCON, so the run has no pair). The Wrench "upright in TF2, flat in
   ours" was the idle's phase and the "wider stances" TF2's bots' feet
   yaw (2026-11-20, `pose.tf2` / `pose.scene`: both now pair; Log). Still
   different: the TF2 bots' jump starts later after `bot_jump` than ours' after
   `jump`, so the 0.15 / 0.4 s pictures show different phases (a movie
   capture would align them).
2. (Done, pass 5: the disc was the sandbox's blast spheres.) The
   mid-air explosion's first 50 ms: ours paler yellow than TF2's orange.
3. Flames: the far fireball's extra width is TF2's bloom (pass 5): TF2's
   bloom is now ours too at `mat_hdr_level` 2 (pass 6, the integer HDR
   model; captures run `+mat_colorcorrection 0`: TF2 maps load at 2 by
   default). Ours'
   unbloomed fireball core is smaller than TF2's, so it blooms less. The
   cloud at a wall spreads wider in TF2 (open). TF2's maps load at
   `mat_hdr_level` 2 by default (TF2's own default; pass 7): the cvar
   defaults to -1, each map's game's own level (CS:S and HL2:DM 0, so
   their look is unchanged), and a level the player sets wins for every
   game, in Ultimate Gun Game too.
4. The Minigun's brass: TF2's land ~15 units farther out and leave
   upward (particles.md Open questions 6): the eject attachment under the
   spun pose, after the animation work.
5. (Done, pass 5: `targets::drive` before the selection stage.)
6. Water explosions, stickies; TF2's crosshair (UGG's HUD by design).
   Crits are off (n/a).
7. (Done, pass 5: the test compared mixed-case wave names.)
8. The asset service's `players` set needs republishing (by the user)
   to carry rockettrail.pcf, stickybomb.pcf and nailtrails.pcf.

## Log

### First person and movement (tf2-fp, 2026-11-19)

Tools (scratch, not in the repo): a per-tick logger of the listen
server's own player over RCON through VScript (`script` lines, no `;`
inside: the console splits on it; strings built from `tochar()`; lines
under 512 characters): origin, velocity, eye height, flags, punch angle,
eye angles, the active weapon's `m_iClip1`. Movies of the own player
(`movie`, every 4th frame kept) against ours stepped at `host_timescale
0.1` with a shot every 4/60 s (`gwait`).

- Bob: TF2's guns had no view-model motion at all; now `MOTION`
  (viewmodel.md 3-4). Turns show no sway (spec; a frame-to-frame jitter
  under `setang` steps is the capture's).
- Draw at 1.34 x and its hold (`DRAW_RATE`); the idle 1.0 s after a
  1.0 s draw (test).
- `cl_autoreload` (weapons.md 4, steps 3-4): `ReloadWhenIdle` and
  `ShellReload::start_interruptible` on every TF2 clip gun. Real TF2's
  Scattergun reload starts ~0.7 s after one shot, ours now the same tick.
- Landings: TF2's full volume above 580 (not the 650 of its damage) and
  the Scout's sound and roll only then (`fall_loud`,
  `loud_landings_only`, classes.toml).
- Measured and matching, nothing changed: the jump (stored vz 271, 259,
  ..., apex 50.04), the duck's 14 ticks both ways, the eyes, the
  Scattergun's punch and spring, the landing roll 0.013 x F.
- Left: the Rocket Launcher's reload entry (above); the first tick's half
  input (movement.md Open questions 13); the Minigun's barrel spin in
  first person (viewmodel.md 7; needs a procedural bone in the view
  model); view-model pairs of the bots' guns (Pyro, Heavy primaries,
  the Scout's Pistol) and of the melee swings; TF2's `smoothstairs` (the
  eye smoothed over steps: no spec numbers; measurable with the
  logger); the view's shake at 10 % on the view model; `cl_autoreload`
  as a player option (always on).

- 2026-11-20 (tf2-pose): specs/tf2/player_animation.md's second-pass
  corrections implemented: the include's knee directions merged onto the
  class models' foot chains and the foot locks' knees bent toward them
  (4a, the spec's reach clamps); every pose parameter at 0 on a spawn and
  a new model, an item's `player_poseparam` through the
  `tf2:player_poseparam` weapon hook (none stock), the Engineer's pistol
  on `r_handposes_engineer` (4b); the feet's goal snaps only when the
  state is made and on `setang_exact` / `target set <n> feet [deg]`
  (`map::anim::SnapFeet`), never on a death, respawn or teleport (4c 5);
  real-time autolayers (the Medic's noise); the held model swapped in the
  frame of a switch (7a). Test cases 42-60 as tests (49-52, 56, 60 on the
  install). TF2 pairs in `pose.tf2` / `pose.scene` answered open
  questions 12 and 14 and part of 15 (the spec). Also: a Combat Arms
  sandbox target stayed crouched after `nocrouch` (its headroom check met
  the floor it rested on); the check starts a skin above the feet now.

- Pass 6 (HDR): specs/tf2/hdr_bloom.md implemented as Chimera's integer
  HDR model, chosen per game by `SourceGame::hdr_model` (TF2: integer;
  CS:S and HL2:DM keep the stand-in). Matched in `hdr.tf2`/`hdr.scene`
  (TF2 and ours, bloom on and off) and `flames.tf2` with and without
  `mat_disable_bloom`; ours' exposure state in the console with
  `mat_hdr_tonemapscale`. Cost at 1024x768: the meter and bloom passes
  0.1-0.3 ms GPU (`source_hdr` in `chimera_perf`), the render world
  +0.26M instructions a frame for the bloom passes and none measurable
  for the meter (frame logs, last 500 frames: 20.57M without bloom,
  20.83M with, 20.80M at level 1 without the meter).

- 2026-11-19: this box's Wayland socket was gone (the compositor still
  ran, its socket unlinked), so no window could open and Steam failed
  ("Unable to open X11 display"). Built the headless route instead
  (above); it needs no compositor at all. `cl_drawhud 0` added to ours
  (every HUD, ours, the sandbox's and the crosshair hidden) and the scene
  directive `gwait <game seconds>`.
- 2026-11-19: TF2 under `host_timescale` misleads: flashes vanished in
  15 ms of game time; under `host_framerate 0.002` they last about 80 ms
  as the data says. TF2 scenes step with `host_framerate` and bursts of
  `snap`s (alias chains: RCON drops long packets, an exec'd cfg ran its
  waits in one frame). This account's Scout carries the Flying Guillotine:
  the pistol is photographed on the Engineer.
- Muzzle flashes: "remap initial scalar" / "remap scalar to vector" were
  not loaded, so `muzzle_scattergun_flash` (the child of the Scattergun's,
  Shotgun's, SMG's flashes) drew at its base radius 32, not 8 to 0.5:
  a screen-filling glow. Loaded and run now (with "Remap Scalar" and
  "Trail Length Random"), particles made at their own moments in a step.
  Real and ours now agree in size, place and colour for the Scattergun,
  Shotgun, SMG and Revolver.
- Particle drag: Source's "Movement Basic" drag is per frame. The same
  TF2 rocket blast at 250 and at 62.5 frames a second differed (the
  slower rate's fire bigger and longer). Ours took it off per step too,
  so our slowed captures (tiny steps) stopped every particle dead; now
  per 1/60 s at any step (`DRAG_STEPS`), and TF2 is captured at 60
  frames a second (`host_framerate 0.016667`) to match.
- Trails: nothing drew a flyer's trail (the rocket's smoke and fire, the
  pipes', stickies', needles'). `[hooks."tf2:looks"] trail` now; the
  particle files rockettrail.pcf, stickybomb.pcf and nailtrails.pcf join
  the looks' (the asset service's `players` set needs a republish to
  carry them to players without the game).
- Flames: this account's Pyro carries the Dragon's Fury, so TF2's stream
  is a puppet bot's (`bot -class pyro`, `bot_teleport`,
  `bot_forceattack 1`) watched in its eyes (`spec_mode 4`), in real time:
  under `host_framerate` TF2 drew no stream. Overbright 5 holds
  (particles.md Open questions 3, answered) but multiplies the linear
  colour: `psys::overbright`.
- 2026-11-19 (second pass): `tf2ref.py up` worked end to end (nullcomp,
  our Xwayland, Steam, TF2), but TF2's frame came up 1024x720: a wider
  FOV than ours; it now asks for 1024x768. Two timing errors in the first
  pass's captures: on this listen server `wait 2` is one rendered frame
  (a falling player's `cl_showpos` speed rose 12 units/s per recorded
  frame: 15 ms, one tick), so the "a" and "b" bursts were the same frames
  and snaps were dropped; and ours ran `+attack` in the frame
  `host_timescale` changed, so its effects' first 13 ms ran in one step
  (`psys_running`, new: the tracer was 79 units out when `wait_for shot`
  held). TF2 now records with `startmovie` (`movie`), ours waits 3 frames
  first. With both fixed the muzzle flashes agree in lifetime.
- Tracers: TF2's are thin, brightest at the bullet, fading back to the
  gun; ours were twice as wide and bright at the gun. The strip's v now
  runs tail 0 to head 1 and its width is the radius, at most its length
  (the Minigun core's 400 would otherwise be a wide bar).
- Puppet bots watched in their eyes (`spec_mode 4`, the only way to see
  stock guns this account doesn't carry) show the world at FOV 75 (a
  bot's `fov_desired`; own player at 75 matched it exactly) and their
  view model about 80 px right of a player's own at the same FOV. The
  first pass's flame comparisons ran ours at 90 against them: the
  "TF2's far fireball 1.5x ours" and "the Flame Thrower sits high and
  left" were partly that. `flames.scene` and `viewmodels.scene` run ours
  at 75 now; at 75 TF2's far fireball is still about twice ours across,
  and its cloud at a wall wider (open). `tf_debug_flamethrower 1` draws
  TF2's flame points' boxes (they reach the bridge's far end).
- `cl_drawhud 0` now hides the sandbox range's look panel too (it showed
  over the third-person captures).
- 2026-11-19 (third pass): `target add ... ahead` / `target set <n>
  ahead|noahead` (`Target::fires_ahead`): the trigger held, level along
  its line. TF2's bots in `thirdperson.tf2` now spawn at once
  (`mp_disable_respawn_times 1`: a bot teleported before its respawn wave
  wasn't moved, so a class was missing from a run, a different one each
  time), carry their own names, fire no random crits (one Engineer shot
  was a crit: violet tracers), and the Minigun and Flame Thrower are
  forced to fire inside the stepped movie (forced before it, the bot
  vanished). First-person systems (the flash, the Minigun's constant
  systems and brass, the flame stream) of this machine's own player now
  follow an `AttachmentMark`, placed each frame where the view model's
  attachment appears in the world view: the flash rides the gun's kick.
- Third person, pass 3: a server's own animation state of each TF2 body
  (`SimAnimState`, whose pose clients draw others with) now gets the
  class too (`follow_class`): a server posed a Soldier's, Pyro's or
  Heavy's Shotgun as a primary for everyone else. A held gun's systems
  turn with the owner's aim (`Muzzles::view_turn`: a character's
  `Transform` never turns). Others' Minigun constant systems and brass
  start while their rounds arrive (`REMOTE_FIRING_HOLD`; a client is
  sent only its own player's spin), and a bot's in the same world from
  its shots; without a drawn gun the constant flash starts ahead of the
  eye.
- The "flames go up" seen in ours' third person is the stream's end: the
  points stop about 286 units out (2450 units/s under drag 8.5/s for
  0.6 s, flames.md) and the sprites there rise; not an obstacle (checked
  against 2fort's brushes and by moving our body).
- 2026-11-19 (fourth pass): tracers from the front are the same geometry
  in both (a streak passing within ~20 units of the camera widens to tens
  of pixels in TF2 too); TF2's fan downward because the stream starts at
  the third-person muzzle, below the bot's eye (65-68, checked with
  `spec_mode 4` + `getpos`), and ends far behind the camera. Flames: seen
  from the side (`side.tf2`-style camera outside the bridge, 330 units
  off) TF2 draws one continuous stream from near the nozzle; ours drew
  puffs. Two causes: others' rounds reach a client in batches, so their
  points were made together in one place (now played out one `emit`
  apart, `flame::RemoteRounds`), and the sprites sat on their points
  (now they move with them and close the gap at the catch-up value in
  units/s, flames.md Open questions 1, measured). `psys_running all`
  prints every particle with its control point, and the tag's points.
- Backblast, side view (camera 100 units beside the Soldier): TF2's and
  ours agree (an orange flame at the launcher's rear for ~100 ms, then a
  dark grey puff that drifts and fades); TF2's flame is streakier. Close.
- The hay by the bridge's far barrel: ours drew it (detail sprites) but
  as thin aliased strands and, farther off, a few dots: the mips average
  the blades' alpha below the 0.5 test. The alpha is now raised by the
  mip level (`render::detail::tested_alpha`); TF2 fades it the same way
  between 800 and 1200 units (cl_detaildist 1200, cl_detailfade 400:
  gone at ~1180, measured along the bridge) but blended, ours dithered.
- Brass: "collision via traces" was parsed and dropped, so the Minigun's
  casings (bounce 0, slide 0, radius scale 1.5) fell through the floor.
  `Op::Collide` now runs for started systems (a ray per step against the
  solid world, characters left out): the casings land and lie. TF2's fly
  farther out to the Heavy's side and fewer lie by his feet (open).
- Others' flash before their tracers: confirmed (`thirdperson.tf2`, the
  Scattergun and Shotgun: the flash at frame 2, the tracers at 9-10, with
  the fire gesture). Read as TF2's interpolation: the flash comes with
  the shot, the tracers when the body drawn 0.1 s in the past fires. Ours
  now holds another player's tracers (`NetDrawn`) for the interpolation
  delay (`looks::tracer_hold`, `InterpClock::delay`). The sandbox's
  targets live in the client's own world (not interpolated, delay 0), so
  its pictures still show them together, as its bodies fire.
- 2026-11-20 (fifth pass): the mid-air explosion's "hard-edged orange
  disc" was no particle at all: with `Explosions_MA_Flash_1` left out
  (a scratch switch) it stayed, a uniform translucent dome round the
  blast: the sandbox's blast spheres (`range::blast_spheres`), which
  `cl_drawhud 0` didn't hide (the labels it did). Hidden now
  (`range::spheres_shown`). Found with `psys_running all` (now a row per
  started child, with its material and renderer) and `psys_def <name>`.
  At 300 units both games' glows fill the far bridge, clipped by the near
  truss. `air.tf2` had put TF2's Heavy 200 units off and ours 300.
- A target set `noahead` kept its Minigun spun: the selection stage (where
  the spin reads the trigger) was ordered only after the brain, not after
  `targets::drive`; now it is.
- `heavy::pack::the_class_sets_load_only_the_offered_classes_bodies` with
  the client install: the test compared mixed-case wave names
  (`vo/scout_Award12.mp3`, the client's scripts) with the sets' lower-case
  paths; the sets were right. It compares lower case now.
- Flames (pass 5): TF2 with and without `mat_disable_bloom 1`: the far
  fireball's white core and much of its width are bloom; ours (no bloom
  at the default `mat_hdr_level 0`) sits between TF2's two. Ours at
  `mat_hdr_level 2` washed 2fort out white: the HDR path's exposure is a
  stand-in. Measurement in flames.md Open questions 1.
- Brass (pass 5, `mg_top` scenes, camera 260 units above the Heavy):
  TF2's casings 40-75 units to the side, ours 25-55; particles.md Open
  questions 6.
- Scenes added: `air.tf2`/`air.scene` (the mid-air burst at 300 units, matched) and `brass_top.tf2`/`brass_top.scene` (the Minigun brass from above).
- 2026-11-19 (tf2-held): the Demoman's Grenade Launcher across his body
  was the kind, not the merge: his primary set poses `weapon_bone_1` for
  the Stickybomb Launcher. The bodies' own bones are what held models
  merge onto now (no `vm_` bones; whole on a body lacking one). Sandbox
  targets jump, crouch and run (`target set <n> jump|crouch|run <dir>`)
  for animation pairs; `anim2.tf2` / `anim2.scene` pair every class's
  three slots and its crouch and jump.
- 2026-11-20 (sixth pass): TF2's projectiles were invisible on every map:
  `GameKit::stay` keeps held models whose key starts with a kept gun's,
  and `tf2:rocket`/`tf2:grenade`/`tf2:sticky`/`tf2:syringe` started with
  none; only their trails showed. Keys are now `<gun>/<part>` (converter
  3). A shown item whose model is built late now waits for it
  (`loose::attach_shown`, `LateWanted::shown`) instead of drawing nothing
  for good.
- Stickies (`stickies.tf2`/`.scene`): the armed pulse (`stickybomb_pulse_<team>`,
  `[hooks."tf2:looks"] armed`) comes at 0.8 s and every 0.1 s, as TF2's.
  With the model drawn ours' trail glow is as faint as TF2's.
- The Flame Thrower's orange wash on the right wall was `new_flame_pilot`:
  a locked trail with no movement operator drew its 800-1200 start speed
  as a 100-unit streak back past the eye; a trail of an unmoved system now
  follows its motion (`render::psys`). TF2 shows none while still.
- Water (`water.tf2`/`.scene`, 2fort's river, surface near z = -140) and
  impacts (`impacts.tf2`/`.scene`: SMG on wood, blood, the Wrench):
  close; the blood pair missed (TF2's bot stood off the crosshair).
- Explosion colour: TF2 with `mat_disable_bloom 1` is pale in the first
  frame too; the rest of the saturation is the HDR/bloom (the HDR spec's
  work). Its long yellow streaks at 33-50 ms are longer than ours: the
  trail renderer's length fade-in (age-based in ours) is a guess
  (particles.md Open questions).
- Not in our game: the Medi Gun (no beam), the Engineer's buildings. The
  Sniper's dot (weapons_more.md 3.1) isn't drawn yet: others' scope state
  isn't sent to other clients; it needs a replicated dot and its sprite
  (`effects/sniperdot_<team>`).
- 2026-11-20 (seventh pass): the Sniper's laser dot (weapons_more.md
  3.1): `[hooks."tf2:looks".dot]` on the rifle, drawn each frame
  (`weapons::dot::laser_dots`) where a scoped holder's aim meets the
  world or a character, 4 units back; 2 units and at most 384 out in his
  own view, 6 for others. Others' scope was already sent (`NetHeld::mode`);
  the charge now goes too (`weapon::HeldGauge`, a byte in `NetHeld`),
  the dot's core scaled by it. In his own view TF2's and ours match in
  size, softness and the core's growth (0.5, 1.5, 4 s); the core is
  red-orange in TF2 too (weapons_more.md Open questions 21). Sandbox
  targets press attack2 once (`target set <n> alt`) and look down
  (`pitch <deg>`).
- Explosion streaks (`streaks.tf2`/`.scene`, every frame): TF2's rays are
  ~25-50 units at 17 ms, ~100 at 33 ms, 150+ at 50 ms and ~1 unit wide;
  ours draw no rays but a haze, the trails 10-110 units wide (their
  radius after the 3-11x radius operators). A fade-in by life fraction
  and the unscaled radius were tried (no rays either) and dropped; the
  rule stays open (particles.md Open questions 7). TF2's rocket still
  meets the floor farther away and later (13 frames, ours 7).
- Blood (`impacts.tf2` re-aimed 10 degrees down, the bot renamed: a
  kicked `HitHeavy`'s name lingered and the teleport missed): TF2's puff
  is ~20 px and leaves a splat; ours a speck (effects.md Open
  questions 8). Wrench: close.
- Stickies spin at launch (600, ±1200, 0 degrees/s about their own axes)
  and jitter ±10 units/s right and up, from the shot's seed.
- Every `.tf2` scene sets `mat_colorcorrection 0`.
- Under water (`water.tf2`/`.scene`, `wunder`: the eye under 2fort's
  river, 30 degrees down): still differs. TF2 shows a large orange glow
  round the launcher for ~6 frames after the shot, a long white trail
  from the gun to the rocket, and the blast as an orange glow on the
  riverbed ~0.4 s later; ours a small glow on the rocket, its trail one
  cluster of puffs, and the blast only as a faint orange tint near the
  surface. Next: which systems TF2 starts under water (its rocket's trail
  and the underwater explosion), and where ours' rocket ends.
- Flames at the wall (`flames.tf2`/`.scene`, 0.5 and 1.0 s): close now.
  The violet cloud spans about the same width in both at 0.5 s; at 1.0 s
  TF2's is ~25 % wider and sits a little higher. No change made.
- 2026-11-20 (spec pass, tf2-fx-spec2): the streaks are `Explosion_FlyingEmbers`
  (each blast child started alone in TF2: the core flash is TF2's wide orange
  star too, the flash-up draws nothing); trail rule settled (particles.md
  5.1). TF2's late blast is the client's interpolation (blast at frame 15 with
  lerp 0.1, 11 with lerp 0.03; effects.md 11), which also makes the "launch
  glow" (the trail's fire passing the camera). Blood (effects.md 8), under
  water (9) and melee impacts (10) specified.
