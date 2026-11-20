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
| Rocket trail | close (2026-11-20, `launch.tf2`/`.scene`) | added (none drew): `rockettrail` on every rocket, pipes', stickies', needles' trails. The "very long trail" was `rockettrail_burst`'s sprite trails streaking by the rocket's own 1100 units/s (a lock now carries a particle's last place too, particles.md 5.1) and the copy drawn late thrown back 110 units behind the eye a frame after the shot; now held at the launch point until the drawn time reaches it, as TF2's glow at the launcher from frame 2 shows. TF2's glow sits further right; ours leaves a smoke puff at the launch point |
| Projectiles leaving the gun (pipes, stickies, needles) | close (2026-11-20) | pipes and needles show at the barrel 7 frames after the shot as TF2's (their models were hidden 0.2 s, so they appeared far down range); stickies drawn late too (were not); only the rocket's model hidden 0.2 s (`model_hidden`) |
| Grenade and Stickybomb Launchers' flashes | matches (2026-11-20) | went straight up: their muzzles' z is the barrel and the flash's core leaves along z; a per-shot flash now takes its attachment's whole orientation (`start_with_axes`, `psys::Follow::up`); none visible in either now |
| Needles on a wall | close (2026-11-20, tf2-leftovers) | TF2: no needle stays (seen from the side); a small hole mark and splinters, kept like a decal. Ours now the same: a flyer with `marks` leaves a bullet's impact (decal, debris, the surface's sound; Chimera's `projectile::FlyerMark`, sent as `NetMark`, shown when the drawn needle reaches it), on every map, the sandbox's too. TF2's mark is red at its centre head-on; ours the surface's bullet hole |
| Rocket and blast sound | close (2026-11-20) | the script data match (`Weapon_RPG.Single` CHAN_WEAPON 94 dB, `BaseExplosionEffect.Sound` CHAN_WEAPON 95 dB, volume 1, full-scale stereo `)` waves, the blast's 8 s long with its own tail); one of each plays. The distortion was our mix clipping (1.6 s at `volume` 1, a moment at 0.5): now a limiter (`audio::sound::limit_mix`) keeps the one-shot voices under 0.75 of full scale. TF2's loudness itself can't be heard here (`+volume 0`). The blast's sound now plays with its look when the drawn time reaches it (effects.md 5 step 5, 11: the temp entity's), not 0.1 s before (tf2-leftovers) |
| Sniper scope overlay, charge meter, crosshair (`scope.tf2`/`.scene`) | close (2026-11-20) | the overlay was our 512-texel stand-in stretched (blurry on a tall screen); now TF2's own quarter (`hud/scope_sniper_ul_dx80`, the plain fallback of its refracting one) over the whole screen as TF2 stretches it, no warp or tint; the generic stand-in 1024 texels with pixel-wide cross hairs. The charge meter drawn (hud.md "Measured"). The shell's crosshair hides behind a scope (`hud::ScopeUp`); TF2's HUD now keeps its crosshair there, as real TF2 draws it (hud.md "Measured", Open questions 13; tf2-leftovers). The scope's look on a 16:9 screen is unmeasured (the headless display is 4:3) |
| Projectiles' models | matches (pass 6) | none drew on any map: a map keeps the kept guns' held models by key prefix and the projectiles' keys (`tf2:rocket`, `tf2:sticky`...) matched none; now `<gun>/<part>` |
| Stickybombs | close (pass 6) | model, faint trail and the armed pulse (0.8 s, then every 0.1 s: ring, then disc) as TF2's; launch spin and jitter as the spec's (pass 7) |
| Sniper's laser dot | matches in his own view (pass 7, `dot.tf2`/`.scene`) | drawn (`weapons::dot`): size, softness and the core growing with the charge as TF2's; others see it at 6 units (the charge sent with what a character holds, `HeldGauge`), no TF2 pair (its puppet bots won't scope); TF2's scope overlay not photographed yet (the class menu covers it) |
| Water explosions | close (pass 6); under water differs (tf2-leftovers: see the log) | from above the surface both hide the burst; TF2's shows through as an orange glow from just above the surface. Under water (pass 7, `wunder`): see the log |
| Bullet impacts (wood) | matches (pass 6) | splinters, hole, puff alike; TF2's hole decal a little lighter |
| Blood on a player hit | close (pass 8) | TF2's own burst and spray now (`weapons::blood`): a red splat at the hit spraying out the far side in both; TF2's starts ~2 frames later (its snapshot wait) and its goop is larger and brighter; no decal on the body (characters take none) |
| Melee (Wrench at a wall) | still differs (pass 8) | the swing's path and timing alike; TF2's flecks and faint scratch at the view's centre, ours a puff at the right edge and no decal: at 55 units the 48-unit line misses and our box sweep's contact point is the box's corner, not on the wall ahead (where a box hit's impact point is, weapons_stock.md names none) |
| Backblast (third person) | close | from the side (pass 4) the same: an orange flame at the launcher's rear ~100 ms, then a dark grey puff; TF2's flame streakier. From the front the shoulder hides most of it |
| Bodies, idle poses (third person) | close (pass 7 of the animation: `pose.tf2` / `pose.scene`) | the "wider stances" were TF2's bots' feet yaw: bot_teleport never snaps a puppet bot's feet, and these spawn 30 degrees off a multiple of 45, so they stand 30 left of their eyes (wide) and, after the scene's teleports, 15 right (narrow); ours drawn with its feet at those offsets (`target set all feet <deg>`) stands as TF2's. The Engineer's Wrench taps flat and raised three times a loop in both (a phase-matched pair agrees). Every pose parameter 0 at a spawn, the hand and arm libraries only where a host layers them, the knees bent toward the include's knee directions under the foot locks, the Medic's real-time idle noise. Only the active weapon is drawn (TF2 and ours, the Spy with his Knife and Revolver from three sides) |
| Flame stream (air, wall); overbright 5 or 1 | close (pass 4: one stream from the nozzle, as TF2's) | overbright is 5, in linear space. The far fireball's "1.5x wider and whiter" is TF2's bloom (pass 5: with `mat_disable_bloom 1` its near-white area fell from ~8,900 to ~1,700-2,900 px; ours draws ~5,300 with no bloom; flames.md Open questions 1). Ours has bloom only at `mat_hdr_level` 1-2, a stand-in that overexposes 2fort (tried). TF2's cloud at a wall: close (pass 7: as wide at 0.5 s, ~25 % wider at 1 s); the orange tint on the right wall is gone (pass 6: the pilot's trail) |
| Detail sprites (2fort's hay) | close | drawn; kept whole at a distance (alpha raised by mip level); TF2 fades them blended where ours dithers |
| Explosion on a player / mid-air | close (pass 6: TF2 without bloom is pale in its first frame too; its streaks are longer at 33-50 ms; pass 7: measured, particles.md Open questions 7, still differs: TF2's rays are ~1 unit wide, ours radius-scaled 10-110 units, an orange haze) | the "hard-edged orange disc" was the sandbox's blast spheres (`range::blast_spheres`), which `cl_drawhud 0` didn't hide (now it does). Matched at 300 units, ours fills the far bridge with the same orange glow clipped by the near truss, the same sequence; ours a little paler yellow in the first 50 ms; TF2 shows more embers and a faint dark smudge after |
| Crits | n/a | random crits are off in our TF2 (the party's choice, docs/plans/active/tf2.md); crit tracers, trails and streams are named in `looks` but never asked for |
| View models (position, FOV, bob) | matches at rest (Scattergun, Shotgun, SMG, Revolver on our own player at fov 90; SMG at 75) | puppet bots seen in their eyes are no reference for view models: their FOV is 75 and their view model sits about 80 px right of a player's own (`viewmodels.tf2` kept for the world and the guns' look, not positions); this account's own loadout isn't stock for the Scout's secondary, the Pyro's and the Heavy's primaries |
| HDR, exposure, bloom (`mat_hdr_level` 2; specs/tf2/hdr_bloom.md) | close (pass 6, `hdr.tf2`/`hdr.scene`: bridge, wall, courtyard, bloom on and off) | the integer model (`map::HdrModel::Integer`): bloom adds 7.2-11.7 to the mean byte where TF2's adds 6.1-12.6; scenes 3-5 % brighter than TF2's in two views, 6 % darker in the courtyard (ours settles k at 0.96-1.04, TF2 reported 0.94); the far fireball's near-white area with bloom ~15,000 px against TF2's ~37,000 (its unbloomed core is smaller: the flame effect's, not the bloom's). TF2 maps now load at `mat_hdr_level` 2 by default (`SourceGame::hdr_default_level`; an explicit `mat_hdr_level` wins, -1 is "Game default") |
| View-model bob and sway (first person, own player) | close (2026-11-19, fp pass) | had none: TF2's guns registered no motion rules. Now CS:S's bob on a fixed 0.8 s cycle, no sway (`tf2::game::view::MOTION`, `view_motion::Sway::None`); walking pairs agree in amplitude and the hand's rise, within a few frames of phase |
| View-model draw, fire, reload (Scattergun, Rocket Launcher) | matches (Scattergun); close (Rocket Launcher) | the draw played at 1x: now 1.34 x, holding its last frame to its own length (`ViewAnimRules::draw_rate`); TF2 reloads a clip not full whenever attack is let go (`cl_autoreload` 1), ours only an empty one: now `[magazine] reload_when_idle`, and an attack stops a reload's start too (`shells.start_interruptible`). Rocket Launcher's reload: TF2 plays its whole 0.5 s start and a loop a round, the rocket counted 0.33 s in (measured per tick); ours now too (`shells.round_at`). The switch: ours drew no view model in the frame of a switch (the new body shown a frame late); now the new gun shows that frame (`swap_view_bodies`) |
| Camera: view punch, landing roll, eye heights, duck | matches | measured every tick through VScript on the live client: the Scattergun's kick (-2..-4, the punch spring), the landing roll 0.013 x F (a Scout's only above 580: now so, `loud_landings_only`), eyes 65/68, the duck's and jump's ticks exactly as movement.md |
| Movement (accel, jump, duck, fall) | matches | movement.md's cases 1, 5, 15, 16 exact on a real player; the first tick of a key press sends half (TF2, CS:S and HL2:DM measured alike: Source's client input), now ours for every Source movement (`core::HalfFirstPress`, `input::apply_move_keys`) |
| HUD, crosshair | todo | the scope's crosshair: see the Sniper scope row |
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

### Input timing, every Source game (input-timing, 2026-11-20)

From specs/source/input_timing.md, in Chimera's client input (so CS:S,
HL2:DM, TF2 and the other games alike; Combat Arms' and Halo's specs say
nothing otherwise, so they get the same timing, without Source's move
shares):

- **Same frame.** The local input (`write_local_intent`, the cursor grab,
  the weapon selection's and the spectator's keys, the rules' hold,
  Combat Arms' popup gate, `track_move_keys`) runs in `RunFixedMainLoop`
  before the fixed loop (`input::LocalInputSystems`: Menus, Write, Hold,
  Gate, Track), so a press reaches the tick of its own frame (spec test
  1), else the next frame's with a tick (test 2). Menus keep their
  capture: keys they take in `PreUpdate` (`CapturedKeys`, the chat's
  reset, owned keys) are gone that frame; a menu opened in `Update`
  gates from the next frame. Non-polled binds (`run_binds`: `invnext`,
  `buymenu`) still run in `Update` (Known shortcut in `input`).
- **Taps.** `input::KeyLatch`: a button press no command has taken is
  on in the next one even if let go (fire, attack2, jump, duck, reload,
  use, `lastinv`, named actions, a slot key); later ticks of the frame
  see only what is held (test 3). The click that grabs the cursor is
  taken even when let go within the frame.
- **Move shares** (`input::move_share`, `HalfFirstPress` movements):
  0.5 / 0.25 / 0.75 / 0 by the spec's table (tests 4-7), and a frame
  without a tick reads the shares away (frame loop step 3, inferred).
- Tests: chimera_client `input_timing::` (same frame, sub-tick tap,
  menus swallow keys, the shares). The schedule ratchets are unchanged
  (the input is not in their compositions; FixedPreUpdate is not
  compared).

### Leftovers (tf2-leftovers, 2026-11-20)

- **Needle marks** (effects.md "Needles on a wall"): `chimera:projectile`'s
  `marks` (Chimera, any game): a touch on anything but a character makes
  the server's `FlyerMark` (to clients as `NetMark`), held `LateDelay` on
  each drawing machine, then the surface found again there and shown as
  `WeaponEventKind::Marked`, which the Source impact path draws as a
  bullet's hit (decal by the surface, debris, sound; no ricochet). The
  Syringe Gun sets it. Tests: `impacts::a_needle_marks_the_floor_...`,
  `effects::every_client_sees_a_needles_mark`. Left (a spec question):
  which decal TF2 uses for a needle (its mark is red at the centre seen
  head-on, a bullet hole isn't), and whether a needle plays the
  surface's impact sound or the ricochet.
- **The blast's sound with its late look**: the explosion's sound plays
  where its look starts (`looks::explosions`, the blast's `sound` by
  weapon from the rules, `blast::BlastSounds`), held as long; the server
  sends no `GameSound` for it.
- **The Scattergun's reload ticks** (weapons.md 4, measured: start 14
  ticks, a loop every 34): ours alternated 33/33/34 because each loop was
  timed back to back from its due time (0.5 s is 33.3 ticks at 0.015 s).
  `ShellReload::from_tick` (`shells.from_tick`) times each part from the
  tick the last ended: 34 every time; on every TF2 one-at-a-time gun but
  the Rocket Launcher (already from the tick). Test
  `weapons::the_scattergun_loads_a_shell_every_34_ticks`.
- **The crosshair under the scope**: TF2's HUD keeps its crosshair behind
  the Sniper's scope (measured; 3.4 said hidden).
- **Confirmed** (`needles_wall.tf2`/`.scene`, a crouched Medic into the
  bridge's plank under the window, pictures 0.5 and 3 s after): both
  leave marks on the wood and no needle; TF2's one pale splintered hole
  with a red centre, ours darker bullet holes (a few, spread) and
  splinters on the floor. The decal is the question above.
- **Under water, the blast** (`water.tf2`/`.scene`, `wunder`): measured,
  not fixed. `psys_running` 0.4 s after the shot: the whole
  `ExplosionCore_MidAir_underwater` runs at about (-150, 0, -280), ~270
  units from the eye (flash, core flash r 27, smoke r 28, 64 bubbles),
  yet nothing of it shows; from 110 units at 20 degrees down it shows,
  cut along a line by the riverbed. 2fort's water fog is -100 to 400
  units (`materials/water/water_2fort.vmt`), which leaves ~45 % of an
  additive particle at 270: fog alone doesn't hide it, and the
  explosion's materials (`effects/softglow`, UnlitGeneric) don't feather.
  Read as the blast's centre lying at or under the drawn riverbed (its
  sprites hidden by the bed from above). TF2 shows it on the bed. Spec
  question: where TF2 puts a projectile's blast centre relative to the
  surface it touched (moved off it along the normal? how far?).
- **The rocket's glow at launch** (`launch.tf2`/`.scene`, frame 4):
  TF2's trail glow has already left the launcher, ~50 units ahead and
  left of it, by frame 4; ours is still held at the launch point by the
  right edge (and under water the held trail leaves one big smoke puff
  there, 150 puffs/s for the 0.1 s hold). Both follow from where the
  drawn rocket starts (effects.md Open questions 11, unanswered): not
  changed. Spec question: the drawn rocket's places in its first 0.1 s
  (the one-second-back history point used or not) and whether its trail
  emits while the drawn place hasn't moved.
- **The Stickybomb Launcher's first-person pose** (`launch` `sticky_idle`):
  differs, not fixed. TF2 shows the drum low and near with the dark
  barrel toward the middle; ours the launcher turned (the flip sight on
  top, the barrel toward the right edge), though our `sb_idle` merge puts
  the muzzle at the spec's place (viewmodel.md test 13, `heavy::guns::
  the_muzzles_...`, within 0.01 unit) and the same pipeline draws the
  Grenade Launcher as TF2 does. Next: compare each bone of the merged
  model (`vm_weapon_bone`, `vm_weapon_bone_1`, `weapon_bone_1`) at
  `sb_idle` frame 0 with the arms' (a picture of TF2's `sb_idle` alone,
  or the spec giving the gun's pose, not only its muzzle). Also fixed:
  the two heavy view-model tests indexed the animator's pose by the
  merged skeleton's bones (the Minigun's kept `barrel` is one more) and
  panicked.
- **Input a frame late** (item 7; done since: "Input timing" above): by the schedule, not only switches:
  `write_local_intent` writes the local `Intent` in `Update`, after
  `RunFixedMainLoop`, so every key (move, fire, slot) reaches the
  simulation in the next frame's tick. Not changed: moving it before the
  fixed loop reorders the menus' key capture (`weapon_menu`, popups) and
  TF2's own press-to-tick latency is unmeasured. Spec question: on the
  frame a key is pressed, does TF2's client run that command (its
  prediction showing the switch or shot that frame)?
- **The scope at 16:9**: still unmeasured (the headless display is 4:3).
- Skipped (no spec): the melee box sweep's contact point, the crit loop,
  blood decals on bodies.

### The Minigun's states, sounds and barrel; sounds loop by their WAV (tf2-minigun, 2026-11-20)

From specs/tf2/minigun.md (T16) and specs/source/sound_looping.md:

- **Looping, every game** (`audio::sound`): a game sound keeps its
  clip's own loop point (the WAV's `cue ` or `smpl` chunk, read by
  `chimera_formats::wav`) and plays on, wrapping seamlessly, until
  stopped or replaced; before, every `PlaySound` was played once with the
  loop removed, so the Minigun's 4.0 s fire loop went silent after 4 s
  and its empty click played once. `dump <game> --loops` lists which
  WAVs of an install loop by their chunks.
- **The Minigun** (`weapons::minigun`): the five states stepped as 2.1
  orders it (attack2 then attack, each with its own next time; the empty
  step; the wind-down at `idle_at`): first shot at tick 57, a released
  wind-up completes, a released spin idles 0.3 s on, firing 0.2 s after
  the last shot; dry while attack or attack2 is held; attack2 refused
  when empty; the empty auto-switch to the previous weapon after the
  1.333 s spool-down; an empty Minigun `Unselectable`.
- **One sound at a time** (`minigun::sounds`), derived on every machine
  from the gun's state (its own player's predicted, others' from
  `HeldState`, a byte in `NetHeld`: `net69`); the server sends none. The
  wind-up is cut by the state change at 0.75 s, the wind-down when the
  barrel stops (`StopSound::local`: never sent). The Minigun no longer
  uses `HeldSounds` (its sound system stops what it played when the gun
  leaves the hand, is removed or its holder dies; a put-away gun's
  wind-down plays on).
- **The barrel** (`minigun::Barrel`): per tick on every machine before
  the state step, 0.1 rad/s toward 20; the view model reads it
  (`view::spin_barrels`), and others' held models now spin too: the
  held `c_minigun`'s `barrel` part is kept apart
  (`props::load_held_turning`, `MapHeldModel::turned`, drawn as a
  `map::HeldTurn`) and turned by `view::spin_held_barrels`.
- Checked against real TF2 (`tools/tf2ref/scenes/minigun_sounds.tf2`,
  `sv_soundemitter_trace 1`, a puppet Heavy holding attack 200 ticks,
  watched in third person, markers every 5 ticks): the wind-up at 0,
  cut with the fire loop starting in ticks 50-54, the fire loop (started
  once) cut and the wind-down started in 210-214, the wind-down cut in
  410-414; ours (tests/it/minigun.rs cases 1-2): 0, 50, 211, 411. Not
  watched, the bot's wind-down was silent (its barrel unturned).
- Left: the crit loop never plays (crits off); the owner's own
  spin-release (Open question 1) and wind-down cut (Open question 2)
  unmeasured; the barrel is drawn a tick behind (between the last two
  ticks).

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

- 2026-11-20 (tf2-fp, second pass): a view-model sequence logger
  (VScript: the own view model's sequence name, cycle and playback rate,
  and the clip, every tick; console lines without spaces round
  operators: the tokenizer's 64 arguments). The Rocket Launcher's reload
  is the start sequence whole (34 ticks), then a loop per round 54 ticks
  apart with the rocket counted 22 ticks in, the finish a tick after the
  last loop: ours by `shells = { start = 0.5, insert = 0.8, round_at =
  0.33 }` (first rocket 0.84 s, was 0.92). The Scattergun's: start 14
  ticks, a loop every 34, the round at each loop's end (ours alternates
  33/34 ticks: back to back from the exact due time; left). The switch:
  the new view model's body was spawned by the drawing system and shown
  only after its first pose, a frame later: a frame with no view model
  at every switch; `swap_view_bodies` spawns it first.
- The first tick's half input is Source's: CS:S's and HL2:DM's live
  clients, started headless as tf2ref starts TF2 (their own RCON ports,
  27116 and 27117), read through the prediction system's variable watch
  (`pwatchent 1; pwatchvar m_vecVelocity`: each command's predicted
  velocity) gave 15.0 of 18.75 and 22.5 of 28.5 on the first tick. Ours
  now halves a move key's first tick on every Source movement
  (`HalfFirstPress`, filled by `source_movement`), the local player's
  keys only (bots send what they choose; movecmp's CS:S bots got whole
  commands).
- `cl_autoreload` is the player's (userinfo, archived; the Party tab's
  "Auto reload (TF2)"), default 0 by the user's choice (TF2's own built-in
  default is 1: `cl_autoreload 0` then reads `( def. "1" )` in its
  console); a character without userinfo (a bot) doesn't auto-reload.
- The Minigun's barrel spun in nothing at first: the bone merge folded the
  `c_minigun`'s `barrel` into the arms' `weapon_bone`, so there was no
  bone to turn. `map::merge_keeping` keeps named weapon bones as joints of
  their own (TF2 keeps `barrel`). Third person (others' Minigun) is the
  animation agent's (player animation / held models), not done here.
- The view model takes the screen shake again at 10 % (`ViewModelShake`,
  every game: the Source specs' rule).
- `smoothstairs`: measured on 2fort's stairs by the RED courtyard (8-unit
  steps at x 560, y 1520-1595) and a ramp (x 680, y 768-928): the client's
  eye (`getpos` every frame) against the server's feet (VScript every
  tick). The eye lags each change of the feet's height on the ground and
  closes at 150 units/s (2.25 a 0.015 s frame, 1.125 a 0.0075 s one;
  Soldier and Scout alike), up and down, on ramps too (movement.md "Eye
  smoothing over steps"). Ours: `core::StairSmoothing` (TF2's 150),
  `client::smooth_stairs`.
- The Minigun's barrel spins in first person (viewmodel.md 7: 0.1 rad/s
  a tick to 20 rad/s and back; `view::spin_barrels`,
  `ViewAnimator::bone_rolls`).

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
- 2026-11-20 (eighth pass, from be2e54457's specs): sprite trails per
  particles.md 5.1 in Chimera (`render::psys`): this step's motion x the
  trail length x min(1, age / fade-in), then clamped; none for a particle
  that didn't move; the full width the radius after the radius operators
  (no cap by the length); every renderer a system lists draws each
  particle (`SystemDef::more_renderers`: the flying embers' trail and head
  sprite); a continuous emitter whose window opens and closes inside one
  step emits nothing (the flash-up, Open questions 8). TF2's blood
  (`weapons::blood`, data/blood.toml): one burst per damage event, the
  spray chosen per viewer, `lowV_` with `violence_hblood 0`, stains per
  pellet by the gun's own damage; body decals (§8b, `render::body_decals`:
  `Impact.Flesh` per hit, none in low violence, cleared on full heal and
  respawn, not on the ragdoll; cloak, disguise and Über to come).
  The rocket's trail chosen once by the water at its first place
  (`rockettrail_underwater`). Projectiles drawn late (`interp` in
  `chimera:projectile`, `chimera_net`'s `late`: a copy drawn 0.1 s in the
  past, seeded back along its first velocity, model hidden 0.2 s) and
  their blasts and blood held as long. TF2's melee impacts on the
  attacker's machine only (`ImpactStyle::melee_own_only`).
- Pass 8 captures (`streaks`, `impacts`, `water` against real TF2, frames
  stepped at 60): the blast now shows 13 frames after the shot in both
  (ours was 7) with the flying embers' thin rays and head dots from its
  second frame, the core flash's wide star as TF2's; TF2's rocket still
  lands a little farther (Open questions 9). Under water: the launch glow
  and the long white trail show as TF2's; the underwater explosion runs
  (`psys_running`: `ExplosionCore_MidAir_underwater` with its flash and 64
  bubbles at the riverbed straight ahead) but nothing of it is visible
  from 290 units under water, where TF2 shows a bright orange glow: read
  as our water fog darkening additive particles (not measured). Blood:
  close. Wrench: the box sweep's contact point (table).
- 2026-11-20 (spec pass, tf2-spec-answers): the open spec questions
  answered. (1) Needle impacts: [effects.md §12](../../../specs/tf2/effects.md)
  (client-made needles, decal by the surface letter as a bullet's, stuck
  in the world until 6 s, RED-only ricochet sound; on players the box
  impact and the server's blood). (2) Blast centres: §13 (1 unit out
  along the touched surface's normal; pipes' 64-unit line; fused pipes
  and stickies trace down 8 above to 24 below; stuck stickies' stored
  normal; water tested at the centre; effects.md Open questions 6, 7
  answered). (3) First moments: §11.1 (history seeded back along the
  launch velocity and used from the first frame; L = 8 ticks = 0.12 s;
  model hidden 0.2 s rockets, 0.1 s pipes/stickies; the trail emits from
  arrival; Open questions 11 answered). (4) Input timing:
  [specs/source/input_timing.md](../../../specs/source/input_timing.md)
  (one command per tick; a press acts in its frame if the frame has a
  tick, else ≤ 1 tick later; taps kept). (5) Melee contact point: §10.1.
  (6) Crit loops: §14 (`Weapon_General.CritPower` follows the boost, not
  random crits; the Flame Thrower's crit loop swap). (7) Blood on bodies:
  §8b (yes: `Impact.Flesh` model decals per bullet/melee hit, cleared on
  full heal, cloak, disguise, Über). New open questions 13-15 (stuck
  needles not seen in the capture; a player box's surface letter; the
  model-decal cap).
- 2026-11-20 (ninth pass, from 735f2576b's specs, tf2-spec-impl):
  projectiles drawn late per effects.md 11.1: the interpolation delay is
  whole ticks plus one (`net::interp::interp_amount`: 0.12 s at TF2's
  tick, for everything a client interpolates, every game), the late
  copy's history seeded one second back along the launch velocity
  (`NetLaunch` sends it to clients) and used from the first frame (no
  hold at the launch point); models hidden 0.2 s (rockets) and 0.1 s
  (pipes, stickies) from the arrival. Blast centres per 13
  (`blast::Placement`): 1 unit off what the blast's own trace met, a
  pipe's 64-unit direct line, fused pipes and detonated stickies snapped
  onto a floor within 24 units below, stuck stickies' surface normal, the
  scorch on the same trace (the 16-unit scorch trace gone).
