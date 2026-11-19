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
| Muzzle flashes, first person | close (scattergun, shotgun, SMG, revolver) | the flash child was 4x too big (remaps by creation time never ran: fixed). At 60 fps TF2's flash stays bright for about 50 ms, ours fades after about 25 ms (SMG, Scattergun); the Minigun's constant flash is much dimmer in ours |
| Muzzle flashes, third person | todo | |
| Minigun constant flash, brass | todo | |
| Tracers | still differs | ours a thick bright beam from the muzzle (the Shotgun's, Pistol's, Revolver's), TF2's a thin faint line: the trail's width from the radius (2) and its texture's mapping (particles.md Open questions 4) to measure |
| Rocket explosion on a surface | close | aligned on the blast, the same sequence (flash, fire with streaks, smoke, debris); ours a little paler in the first 50 ms. TF2's rocket took ~0.5 s to a floor 150 units ahead under host_framerate (ours 0.1 s): not understood, compared aligned on the blast |
| Rocket trail | close | added (none drew): `rockettrail` on every rocket, pipes', stickies', needles' trails |
| Backblast (third person) | todo | |
| Flame stream (air, wall); overbright 5 or 1 | close | overbright is 5, in linear space (we applied it to the gamma colour: x34, a white blob at a wall; now the violet cloud of TF2, its added colour within 3 of TF2's). TF2's cloud at a wall spreads wider (its points slide along the wall; ours stop) and its far fireball is about 1.5x ours |
| Crits | todo | |
| View models (position, FOV, bob) | todo | |
| HUD, crosshair | todo | |
| Lighting, materials | todo | |

## Next (for a successor)

1. Tracers: measure a single tracer's on-screen width and length in TF2
   (`muzzle.tf2`'s Minigun shows one) against ours; decide how a sprite
   trail uses the radius (width or half-width) and the default trail
   length (ours 0.1 s), then the texture's orientation on the strip.
2. Flash lifetime: TF2's SMG and Scattergun flashes stay bright ~2x
   longer at 60 fps; check the flash child's alpha remap (0.6 to 0 by
   creation time) against "Alpha Fade and Decay" and the lifetime remap.
3. The Minigun's constant muzzle (`muzzle_minigun_constant`): TF2's is a
   big bright glow at the barrel's end; ours is faint (its `_core` is a
   400-radius trail: how a trail uses the radius again).
4. Flames: TF2's cloud at a wall spreads wider (flame points slide along
   surfaces, flames.md; ours stop) and its far ball is ~1.5x ours.
5. Third person and the backblast: `thirdperson.tf2` captures TF2 (a
   puppet bot 433 units ahead firing at a free spectator); ours needs a
   matching scene: a sandbox target only shoots back once shot, so it
   needs a target that fires on its own, or our own player in third
   person with a matched camera.
6. Water and mid-air explosions, stickies, crits (`addcond`), view-model
   positions (the Flame Thrower sits a little high and left in ours; the
   Minigun's firing pose), the HUD.
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
