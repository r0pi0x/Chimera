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
| Minigun constant flash, brass | close (first person); differs (third person) | first person: the core is a compact glow (a trail no wider than long), dimmer and nearer the barrel than the account's own Minigun's (not stock); the brass lives 2 s. Third person: others' (and bots') constant systems now run while their rounds come and turn with their aim (they never started for another player's gun, and pointed along the world's -Z); but TF2's spun Heavy lowers the gun forward (the deployed set) where ours keeps the idle pose, so ours' glow sits at the raised barrel by the face; ours' brass falls through the bridge's floor |
| Tracers | matches (SMG, Shotgun) | were a thick beam bright at the gun: the trail's texture ran backwards (TF2's `tracer1` is bright at v = 1, the head) and the radius is the full width, not the half (particles.md 5, Open questions 4 answered) |
| Rocket explosion on a surface | close | aligned on the blast, the same sequence (flash, fire with streaks, smoke, debris); ours a little paler in the first 50 ms. TF2's rocket took ~0.5 s to a floor 150 units ahead under host_framerate (ours 0.1 s): not understood, compared aligned on the blast |
| Rocket trail | close | added (none drew): `rockettrail` on every rocket, pipes', stickies', needles' trails |
| Backblast (third person) | close | the flame at the shoulder in the first 150 ms agrees; at 230 ms TF2's has turned to a dark smoke ring where ours is a bright ball with a detached flame to its left |
| Bodies, idle poses (third person) | close | Pyro, Scout, Engineer, Soldier stand as TF2's; TF2's stances are a little wider. The Heavy's idle (gun raised) matches TF2's only with the class model's own 1-frame aim-matrix stubs: taking the include's same-named grids instead (tried) lowered it to TF2's firing pose, and the deployed set (tried, `ACT_MP_DEPLOYED_IDLE` and its PREFIRE/POSTFIRE) then raised the gun straight up while firing. Which of a player model's local stub and its include's sequence TF2 uses, per lookup, is install.md Open question 6: needs a spec session (the deployed attempt is kept in scratch, `player_anim_spun.rs.txt`) |
| Flame stream (air, wall); overbright 5 or 1 | close | overbright is 5, in linear space (we applied it to the gamma colour: x34, a white blob at a wall; now the violet cloud of TF2, its added colour within 3 of TF2's). Re-taken at a bot's FOV 75 (the first pass ran ours at 90): TF2's cloud at a wall spreads wider (its points slide along the wall; ours stop) and its far fireball is about 2x ours across; ours tints the right wall orange |
| Crits | todo | |
| View models (position, FOV, bob) | matches at rest (Scattergun, Shotgun, SMG, Revolver on our own player at fov 90; SMG at 75) | puppet bots seen in their eyes are no reference for view models: their FOV is 75 and their view model sits about 80 px right of a player's own (`viewmodels.tf2` kept for the world and the guns' look, not positions); this account's own loadout isn't stock for the Scout's secondary, the Pyro's and the Heavy's primaries |
| HUD, crosshair | todo | |
| Lighting, materials | todo | |

## Next (for a successor)

1. The spun Heavy (third person): done 2026-11-19 from the corrected
   spec (player_animation.md 1a, 2, 5a, 6): the class models' stubs are
   0x800 forward declarations the include's sequences replace (the old
   "1-frame, no activity" test), the autolayers' spline and pose ramps,
   the feet's IK locks, the kind tables with the deployed exception, the
   PREFIRE/POSTFIRE gestures in one attack slot; the decoded poses match
   the spec's (raised 34 degrees at rest, level spun, aim and spin
   timelines within 2-3 degrees; `heavy::player_anim`). Left: the left
   hand's IK rule onto the Minigun (4a), the 2026-11-15 main-activity
   rules (airwalk above 300 units/s, the 0.5 s jump start), and a
   side-by-side with TF2's bots once tools/tf2ref is free.
2. Tracers seen from the front: TF2's are short streaks; ours, passing
   close to the camera, become wide glowing cones. Record 30 frames
   (`thirdperson.tf2` does now) and compare past 150 ms; a far shooter's
   streaks also come out up where TF2's go down (its bullets level from
   a higher eye than our camera? check each class's eye height).
3. Flames (`flames.tf2` against `flames.scene`, FOV 75): TF2's far
   fireball is about twice ours across; its cloud at a wall spreads
   wider. The stream's points stop about 286 units out (2450 units/s
   under drag 8.5/s, life 0.6 s), and ours' sprites there rise more than
   TF2's (seen from the front: TF2's fireball sits at the Pyro's chest
   height, ours above his head, rising to the roof). The orange haze to
   the right of the barrel while firing (first person) is not the muzzle
   light (`muzzleflash_light 0` leaves it): look at the stream's newest
   sprites near the camera (the core's size fade 0.3 to 0.5 x l).
4. The backblast: at 230 ms TF2's is a dark smoke ring, ours a bright
   ball with a detached flame beside it.
5. The hay tuft beside the barrel at the bridge's far end is missing in
   ours (small yellow dots there): a detail sprite or prop.
6. The Minigun's brass falls through the bridge's floor in ours (TF2's
   lands): the casings' world collision.
7. TF2 draws another player's flash some 7 frames before its tracers and
   fire gesture (Scattergun, Shotgun); ours draws them together.
8. Water and mid-air explosions, stickies, crits (`addcond`), the HUD and
   crosshair (UGG's own HUD by design; TF2's crosshair to compare).
9. The asset service's `players` set needs republishing (by the user)
   to carry rockettrail.pcf, stickybomb.pcf and nailtrails.pcf.

## Log

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
