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
| Muzzle flashes, first person | close (scattergun, shotgun, SMG) | size, colour and lifetime agree frame by frame at 60 fps (the "TF2 stays bright 2x longer" was two capture errors: TF2's `wait 2` is one frame, and ours' shot ran 13 ms in one step). Ours' flash stays where it started while the gun kicks; TF2's rides the muzzle ("Movement Lock to Control Point" on the animated attachment), and TF2's view model kicks harder (the Scattergun's pump) |
| Muzzle flashes, third person | todo | `thirdperson.tf2` (bot 433 units ahead, free spectator via `spec_mode 6` then `setpos`) and `thirdperson.scene` (a sandbox target, `target add <gun> 11 still protect`) frame the same; but ours' target aims at us and hits us (blood, flames on the camera) where TF2's bot fires straight ahead, and its Minigun points at the camera: the target needs a "fire ahead" mode before the pictures compare. Ours' brass shows (golden casings at its feet) |
| Minigun constant flash, brass | close | the core is a compact glow now (a trail no wider than long); ours is dimmer and nearer the barrel than the account's own Minigun's (not stock); the brass lived one frame (lifetime from a sheet the casing has none of): now 2 s |
| Tracers | matches (SMG, Shotgun) | were a thick beam bright at the gun: the trail's texture ran backwards (TF2's `tracer1` is bright at v = 1, the head) and the radius is the full width, not the half (particles.md 5, Open questions 4 answered) |
| Rocket explosion on a surface | close | aligned on the blast, the same sequence (flash, fire with streaks, smoke, debris); ours a little paler in the first 50 ms. TF2's rocket took ~0.5 s to a floor 150 units ahead under host_framerate (ours 0.1 s): not understood, compared aligned on the blast |
| Rocket trail | close | added (none drew): `rockettrail` on every rocket, pipes', stickies', needles' trails |
| Backblast (third person) | todo | |
| Flame stream (air, wall); overbright 5 or 1 | close | overbright is 5, in linear space (we applied it to the gamma colour: x34, a white blob at a wall; now the violet cloud of TF2, its added colour within 3 of TF2's). Re-taken at a bot's FOV 75 (the first pass ran ours at 90): TF2's cloud at a wall spreads wider (its points slide along the wall; ours stop) and its far fireball is about 2x ours across; ours tints the right wall orange |
| Crits | todo | |
| View models (position, FOV, bob) | matches at rest (Scattergun, Shotgun, SMG, Revolver on our own player at fov 90; SMG at 75) | puppet bots seen in their eyes are no reference for view models: their FOV is 75 and their view model sits about 80 px right of a player's own (`viewmodels.tf2` kept for the world and the guns' look, not positions); this account's own loadout isn't stock for the Scout's secondary, the Pyro's and the Heavy's primaries |
| HUD, crosshair | todo | |
| Lighting, materials | todo | |

## Next (for a successor)

1. The Minigun's constant glow is dimmer and sits nearer the barrel
   than the account's own (non-stock) Minigun's in TF2 (black barrels and
   a low firing pose were the bot's FOV and the account's other Minigun,
   not ours; the brass now flies).
2. Third person: give sandbox targets a "fire straight ahead, don't aim"
   mode (like TF2's `bot_forceattack` on a puppet) so `thirdperson.scene`
   compares; TF2's bots then the reference for bodies, poses, the
   Rocket Launcher's backblast, the Minigun's glow and brass, the flames
   seen from the front. In TF2 the Scout and Engineer bots sometimes
   didn't appear (the old P1 not yet gone): give each class its own name.
3. First-person flashes ride the animated muzzle in TF2 (each frame's
   attachment); ours follow the eye from where the shot was.
4. Flames (`flames.tf2` against `flames.scene`, both at FOV 75 now): at
   1 s TF2's far fireball is about twice ours across and brighter; its
   cloud at a wall spreads wider (flame points slide along surfaces,
   flames.md; ours stop). Ours also tints the bridge's right wall orange
   while firing where TF2 doesn't: find what draws it.
5. The hay tufts on the bridge's far end draw as orange spikes in ours
   (soft yellow tufts in TF2): a detail sprite or material.
6. Water and mid-air explosions, stickies, crits (`addcond`), the HUD and
   crosshair (UGG's own HUD by design; TF2's crosshair to compare).
7. The asset service's `players` set needs republishing (by the user)
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
