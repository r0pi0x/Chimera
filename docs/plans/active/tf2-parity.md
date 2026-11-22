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
| Muzzle flashes, first person | close (scattergun, shotgun, SMG); Pistol (Engineer's and Scout's, stock) matches (2026-11-20, stock-recapture) | size, colour and lifetime agree frame by frame at 60 fps (the "TF2 stays bright 2x longer" was two capture errors: TF2's `wait 2` is one frame, and ours' shot ran 13 ms in one step). The flash now rides the animated muzzle as TF2's does (`attached::AttachmentMark`); TF2's view model kicks harder (the Scattergun's pump) |
| Muzzle flashes, third person | close (Scattergun, Shotgun) | the scenes compare now: ours' target fires `ahead` (TF2's `bot_forceattack`) and our camera stays while our body leaves the line of fire (`chimera_freecam 2`). The flashes' size and life agree. TF2 draws another player's flash some 7 frames before its tracers and fire gesture (seen on the Scattergun and the Shotgun); ours draws them together |
| Minigun constant flash, brass | close (first person, rechecked on the stock Minigun 2026-11-20); close (third person) | first person: the core a compact glow; the brass lives 2 s. Stock Minigun (stock-recapture): spun and firing for 0.75 s the flash covers ~5,500 px in both (TF2 5,706); its first four frames TF2 2,000 / 5,700 / 5,200 / 4,500, ours 1,400 / 4,500 / 6,200 / 8,600 (ours swells on the fourth; TF2's flickers 4,000-5,200 after). Spec ([particles.md §8.3, §8.3.1, §10.11](../../../specs/tf2/particles.md)): caps (sparks 5), the starflash and flare emit only in the first 0.5 s, smoke from 2 s, per-frame counts and sizes; by its data the flash grows to frame 3-6 and holds, particles.md Open questions 10 answered 2026-11-21: TF2 has no frame-2 peak (re-captured frame-exact: the shot frame a tracer, then ~60 % / 95 % / full, held, flickering 5,300-8,200); ours (tf2-left: first glow the second frame after the shot, full by the fourth and held, ~10,000-12,000 px changed from the frame before `+attack`) grows and holds the same way, no change needed (Next item 10: whether ours' first glow is a frame later). Third person (pass 5, from 260 units above): TF2's casings lie 40-75 units out to the gun's side (most ~58), ours ~25-55 (most ~43); from the side TF2's leave upward first. Ours' numbers (local x 100-200, drag 0.05, gravity −252) landed ~40 out from a level eject; particles.md Open questions 6 (measured 2026-11-21): TF2's leave right, square to the aim, ~23° up at 150-180 units/s from 28 right and 41 below the eye. Ours now so (tf2-spec-impl: the system's x tilted 23° up, others' from that point by the eye); the landing band not recaptured |
| Tracers | matches (SMG, Shotgun); from the front close | were a thick beam bright at the gun: the trail's texture ran backwards (TF2's `tracer1` is bright at v = 1, the head) and the radius is the full width, not the half (particles.md 5, Open questions 4 answered). From the front the geometry agrees (a streak passing ~20 units from the camera widens to tens of pixels in TF2 too); which pellets pass near is the spread's chance. Others' tracers now leave with the interpolated body (pass 4) |
| Rocket explosion on a surface | close | aligned on the blast, the same sequence (flash, fire with streaks, smoke, debris); ours a little paler in the first 50 ms. TF2's rocket took ~0.5 s to a floor 150 units ahead under host_framerate (ours 0.1 s): not understood, compared aligned on the blast |
| Rocket trail | close (2026-11-20, `launch.tf2`/`.scene`) | added (none drew): `rockettrail` on every rocket, pipes', stickies', needles' trails. The "very long trail" was `rockettrail_burst`'s sprite trails streaking by the rocket's own 1100 units/s (a lock now carries a particle's last place too, particles.md 5.1) and the copy drawn late thrown back 110 units behind the eye a frame after the shot; drawn from its seeded copy (11.1); a step's particles now spread along the control point's path, the copy's trail made at the rocket's first place (tf2-psys). TF2's big glow ~50 units ahead of the launcher at frames 2-6 still missing: ours' first step leaves a smoke puff at the launcher and its glows mid-path behind the eye (log) |
| Projectiles leaving the gun (pipes, stickies, needles) | close (2026-11-20) | pipes and needles show at the barrel 7 frames after the shot as TF2's (their models were hidden 0.2 s, so they appeared far down range); stickies drawn late too (were not); only the rocket's model hidden 0.2 s (`model_hidden`) |
| Grenade and Stickybomb Launchers' flashes | matches (2026-11-20) | went straight up: their muzzles' z is the barrel and the flash's core leaves along z; a per-shot flash now takes its attachment's whole orientation (`start_with_axes`, `psys::Follow::up`); none visible in either now |
| Needles on a wall | close (2026-11-20, tf2-leftovers) | TF2: no needle stays (seen from the side); a small hole mark and splinters, kept like a decal. Ours now the same: a flyer with `marks` leaves a bullet's impact (decal, debris, the surface's sound; Chimera's `projectile::FlyerMark`, sent as `NetMark`, shown when the drawn needle reaches it), on every map, the sandbox's too. TF2's mark is red at its centre head-on; ours the surface's bullet hole |
| Rocket and blast sound | close (2026-11-20) | the script data match (`Weapon_RPG.Single` CHAN_WEAPON 94 dB, `BaseExplosionEffect.Sound` CHAN_WEAPON 95 dB, volume 1, full-scale stereo `)` waves, the blast's 8 s long with its own tail); one of each plays. The distortion was our mix clipping (1.6 s at `volume` 1, a moment at 0.5): now a limiter (`audio::sound::limit_mix`) keeps the one-shot voices under 0.75 of full scale. TF2's loudness itself can't be heard here (`+volume 0`). The blast's sound now plays with its look when the drawn time reaches it (effects.md 5 step 5, 11: the temp entity's), not 0.1 s before (tf2-leftovers) |
| Sniper scope overlay, charge meter, crosshair (`scope.tf2`/`.scene`) | close (2026-11-20) | the overlay was our 512-texel stand-in stretched (blurry on a tall screen); now TF2's own quarter (`hud/scope_sniper_ul_dx80`, the plain fallback of its refracting one) over the whole screen as TF2 stretches it, no warp or tint; the generic stand-in 1024 texels with pixel-wide cross hairs. The charge meter drawn (hud.md "Measured"). The shell's crosshair hides behind a scope (`hud::ScopeUp`); TF2's HUD now keeps its crosshair there, as real TF2 draws it (hud.md "Measured", Open questions 13; tf2-leftovers). The scope's look on a 16:9 screen is unmeasured (the headless display is 4:3) |
| Projectiles' models | matches (pass 6) | none drew on any map: a map keeps the kept guns' held models by key prefix and the projectiles' keys (`tf2:rocket`, `tf2:sticky`...) matched none; now `<gun>/<part>` |
| Stickybombs | close (2026-11-20, stock launcher) | model, faint trail, flight and resting place (within 6 px) as the stock launcher's; the armed pulse (ring, then disc, then a frame of nothing, every 0.1 s) started the arm time (0.8 s) after each machine received the sticky (effects.md 11.3; replaces the tuned 0.87 s after it shows); launch spin (1200, 2r, 0) and air drag as the spec's (sticky-physics) |
| Sniper's laser dot | matches in his own view (pass 7, `dot.tf2`/`.scene`) | drawn (`weapons::dot`): size, softness and the core growing with the charge as TF2's; others see it at 6 units (the charge sent with what a character holds, `HeldGauge`), no TF2 pair (its puppet bots won't scope); TF2's scope overlay not photographed yet (the class menu covers it) |
| Water explosions | close (pass 6); on the riverbed seen from just above the surface close (tf2-psys: drawn before the surface, shown by its refraction); under water close (2026-11-21, tf2-left) | from above the surface both hide the burst; TF2's shows through as an orange glow from just above the surface. Under water (`wunder`, eye at z -200, 30 degrees down): ours now shows the blast as an orange glow on the riverbed from frame 23 after the shot (TF2's ~0.4 s), peaking at frames 25-27 and fading by 31, with the launch glow and the trail's smoke as TF2's (pass 9 drew nothing there; no new TF2 pair taken) |
| Bullet impacts (wood) | matches (pass 6) | splinters, hole, puff alike; TF2's hole decal a little lighter |
| Blood on a player hit | close (pass 8) | TF2's own burst and spray now (`weapons::blood`): a red splat at the hit spraying out the far side in both; TF2's starts ~2 frames later (its snapshot wait) and its goop is larger and brighter; no decal on the body (characters take none) |
| Melee (Wrench at a wall) | still differs (pass 8) | the swing's path and timing alike; TF2's flecks and faint scratch at the view's centre, ours a puff at the right edge and no decal: at 55 units the 48-unit line misses and our box sweep's contact point is the box's corner, not on the wall ahead (where a box hit's impact point is, weapons_stock.md names none) |
| Backblast (third person) | close | from the side (pass 4) the same: an orange flame at the launcher's rear ~100 ms, then a dark grey puff; TF2's flame streakier. From the front the shoulder hides most of it |
| Bodies, idle poses (third person) | close (pass 7 of the animation: `pose.tf2` / `pose.scene`) | the "wider stances" were TF2's bots' feet yaw: bot_teleport never snaps a puppet bot's feet, and these spawn 30 degrees off a multiple of 45, so they stand 30 left of their eyes (wide) and, after the scene's teleports, 15 right (narrow); ours drawn with its feet at those offsets (`target set all feet <deg>`) stands as TF2's. The Engineer's Wrench taps flat and raised three times a loop in both (a phase-matched pair agrees). Every pose parameter 0 at a spawn, the hand and arm libraries only where a host layers them, the knees bent toward the include's knee directions under the foot locks, the Medic's real-time idle noise. Only the active weapon is drawn (TF2 and ours, the Spy with his Knife and Revolver from three sides) |
| Flame stream (air, wall); overbright 5 or 1 | close (pass 4: one stream from the nozzle, as TF2's) | overbright is 5, in linear space. The far fireball's "1.5x wider and whiter" is TF2's bloom (pass 5: with `mat_disable_bloom 1` its near-white area fell from ~8,900 to ~1,700-2,900 px; ours draws ~5,300 with no bloom; flames.md Open questions 1). Ours has bloom only at `mat_hdr_level` 1-2, a stand-in that overexposes 2fort (tried). TF2's cloud at a wall: close (pass 7: as wide at 0.5 s, ~25 % wider at 1 s); the orange tint on the right wall is gone (pass 6: the pilot's trail) |
| Detail sprites (2fort's hay) | close | drawn; kept whole at a distance (alpha raised by mip level); TF2 fades them blended where ours dithers |
| Explosion on a player / mid-air | close (pass 6: TF2 without bloom is pale in its first frame too; its streaks are longer at 33-50 ms; pass 7: measured, particles.md Open questions 7, still differs: TF2's rays are ~1 unit wide, ours radius-scaled 10-110 units, an orange haze) | the "hard-edged orange disc" was the sandbox's blast spheres (`range::blast_spheres`), which `cl_drawhud 0` didn't hide (now it does). Matched at 300 units, ours fills the far bridge with the same orange glow clipped by the near truss, the same sequence; ours a little paler yellow in the first 50 ms; TF2 shows more embers and a faint dark smudge after |
| Crits | n/a | random crits are off in our TF2 (the party's choice, docs/plans/active/tf2.md); crit tracers, trails and streams are named in `looks` but never asked for |
| View models (position, FOV, bob) | matches at rest (Scattergun, Shotgun, SMG, Revolver, the Engineer's and the Scout's Pistol, the Minigun at rest, spinning up and firing, on our own player at fov 90; SMG at 75) | puppet bots seen in their eyes are no reference for view models: their FOV is 75 and their view model sits about 80 px right of a player's own (`viewmodels.tf2` kept for the world and the guns' look, not positions); this account's own loadout isn't stock for the Scout's secondary, the Pyro's and the Heavy's primaries (the Pistol and Minigun now swapped in by `stock`; the Pyro's still a bot's) |
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
   spun pose, after the animation work. **Needs a spec pass** (item 10).
5. (Done, pass 5: `targets::drive` before the selection stage.)
6. Stickies; TF2's crosshair (UGG's HUD by design). Water explosions: close (item 10).
   Crits are off (n/a).
7. (Done, pass 5: the test compared mixed-case wave names.)
8. (Done, 2026-11-21: the asset service's `players` and `weapon/` sets
   republished with the particle files, the crit glow's materials, the
   sniper dots, `crit_power.wav`, the ricochets and the Pistol's
   view-model sounds; docs/playtest-hosting.md, "Publishing the game
   files".)
9. (Done, 2026-11-21, the same republish: the view-model sounds in TF2's
   `weapon/tf_weapon_pistol_scout`, `tf_weapon_shotgun_primary` and
   `tf_weapon_grenadelauncher` and CS:S's `weapon/awp`; the TMP's
   `Weapon_TMP.Deploy` has no wave, so `weapon/tmp` is unchanged.)
10. **Built from the spec answers** (tf2-spec-impl, 2026-11-22; commit
   1cb4ace47's specs; the log's entry). Left open:
   - the Minigun flash: particles.md Open questions 10 says TF2 has no
     frame-2 peak and grows over its first three frames after the
     tracer's (60 % / 95 % / full) and holds, as §8.3.1's data; ours
     (tf2-left's `mini` capture, stepped by `host_timescale`) grows
     over three and holds the same way, but its first glow counted from
     a frame later. A `host_framerate 0.016667` capture of ours, as the
     spec's, would settle whether that frame is real (no code change);
   - the brass's flight after its start (the spec's tracks didn't refit
     the operators; ours run TF2's system as it is);
   - the flame cloud's width at a wall at 1 s (flames.md Open questions
     4: not reproduced on the points; the drawn sprites unmeasured);
   - water explosions: nothing left that a spec must answer (the under
     water blast now shows, the table's row); a fresh TF2 `wunder` pair
     would confirm its size and colour.

## Log

### Knockback "way too much" (tf2-knockback, 2026-11-22)

User report: TF2 guns knock back far too much. Our push is weapons.md
5.4's formula exactly (min(1000, D x s x 48·48·82 / V), bullets summed
per victim per shot, scale 6), but V was the victim's real box: a TF2
body fitted into a smaller map's envelope (a CS:S map's 32 x 32 x 72) or
another game's body took 2.56 x the push (a Scattergun at 512: 922
units/s, not 360; point blank: the 1000 cap). Fixed: V is TF2's hull for
the victim's stance (`blast::tf2_hull`: standing, ducked 62, own ducked
55), test `a_scattergun_pushes_any_body_as_a_tf2_player_47`.

Measured afterwards (weapons.md 5.4.1, 2026-11-22): the formula is right,
but a grounded victim's horizontal speed is cut, at his first movement
tick after a push and after that tick's friction, to 0.9 x his own max
speed (Soldier 216, spun Heavy 99); a push that lifts him (> 250 up) or
finds him airborne is kept until he lands, where the limit applies; a
Heavy takes half. Built: `SourceMovement::push_limit` (set by
`blast::shove` to `GROUND_PUSH_LIMIT`, applied in the movement's tick
with the body's own max speed, so any body on a Source movement is
capped the same; a non-Source movement (the placeholder's) is not:
known shortcut). Tests 47a-47d in games/tf2/tests/it/weapons.rs
(Scattergun at 512: 216 then friction, slide about 42; spun Heavy 99;
airborne keeps the push until landing; Minigun never past 216; rocket on
the floor 216). Open: a victim moving under his own input (weapons.md
Open questions 9).

### The spec answers built (tf2-spec-impl, 2026-11-22)

From commit 1cb4ace47 (weapons_more.md 4.4, 5.1, 7.1; particles.md Open
questions 6, 10; flames.md's edge case):

- **Grenade spin**: angular drag (9.45, 3.98, 3.98)e-7 on the whole ω
  (grenadelauncher.toml), then its precession in the body (inertia
  (0.288, 1, 1), `DragParams::inertia`), integrated as the engine does in
  1 + floor(|ω| dt / (1/12 rad)) forward-Euler sub-steps from the tick's
  starting |ω| (Chimera's `flight::precess`, also with no air). Unit
  tests 29f (4 sub-steps, −12.81°, × 1.00626), 29g (9 sub-steps, ω_x
  1189.6), the measured growth at n = 4-8; a scenario test in flight
  (weapons_more.rs). The sticky gets no inertia (its ω keeps its
  direction, as measured).
- **Sticky cap**: the launch velocity's length capped at `max_speed`
  2000 (pipebomblauncher.toml), direction kept: a full charge leaves at
  2000 (test 29e: 2000 at launch, ~1984 after the first tick; test 29's
  full charge now 1993 forward).
- **Flame Thrower at a wall**: `flame::blocked`, predicted, before the
  frame: on each emission due, the line from the eye to 40 forward, 5
  right (the point's own `origin`, any stance and pitch) against the
  world; blocked, the frame sees no attack: no point, no ammo, no burst
  (no stream, no start or loop sound); a burst blocked at an emission
  ends as a release (the end sound), and restarts on the first clear
  tick. Tests 46b (39.5 / 40.5, standing and crouched), 46c (pitch ±30°
  at 34.5 / 35.5, turned 40° at 33.5 / 34.5), 46d (flames.rs).
- **Minigun brass**: `eject_minigunbrass` leaves along the view's right
  tilted 23° up (`while_firing`'s `up`), so the system's own 100-200
  units/s along its x is TF2's measured right-and-up 150-180 at ~23°;
  on another player's Minigun it starts 28 right and 41 below the eye
  (`from_eye`), not at the held model's attachment (effects.rs test).
  The third-person landing band (TF2 40-75 out, ours ~25-55) should
  move out with the tilt; not recaptured.
- **Minigun flash**: no change (the spec: no frame-2 peak; ours grows
  and holds as §8.3.1). Recorded in the table's row and Next item 10.

### The open parity items checked against the specs (tf2-left, 2026-11-21)

Asked to build the Minigun flash's start and brass, the grenade's angular
drag and the 2000 units/s cap, flames hitting walls and explosions in
water where the specs settle them. None of the four is settled beyond
what is built, so no code changed; the questions are Next item 10.

- **Minigun flash** (capture `mini`, the stock Minigun spun 1.7 s, then
  `+attack` at `host_timescale 0.01`, a picture every 1/60 s of game
  time): ours' first frame after the shot shows only a tracer, the glow
  starts the next, grows to the fourth and holds (pixels changed from
  the frame before `+attack`: 2,300, 4,900, 10,100, 11,700, then
  9,000-12,000), as §8.3.1's data says; TF2's peak at frame 2 and
  shrink (2,000 / 5,700 / 5,200 / 4,500) is outside the system's data
  (particles.md Open questions 10). The brass is Open questions 6.
- **Grenade and sticky flight**: weapons_more.md Open questions 22 and
  23 are unanswered (ours: the grenade's angular drag a uniform 3.3e-7,
  no speed cap; sticky-physics).
- **Flames at walls**: the points' slide is flames.md 1 as built. The
  emission block while the eye-to-muzzle line meets the world
  (weapons_more.md 7.1) needs a muzzle on the server, which the spec
  doesn't place; not guessed.
- **Under water** (capture `wunder`, as `water.scene`'s second part): the
  blast on the riverbed now shows from 290 units under water (pass 9
  saw nothing): an orange glow from frame 23, brightest at 25-27, gone
  by 31 (reddish pixels 1,200 before it, 9,200 / 22,300 / 31,400 /
  36,700 / 25,900 at frames 23-27). Most likely the tf2-psys and
  psys-operators passes since (the blast's systems and their timing);
  not bisected.

### The Flame Thrower's sounds, the Grenade Launcher's drum: built (tf2-flame-drum, 2026-11-21)

From effects.md 14.2 and weapons_more.md 9.6 (the spec pass below):

- **Flame Thrower** (`weapons::flame::sounds`; data `flamethrower.toml`:
  `end_sound` `.WindDown`, `pilot_sound`, `burst_fade = 3.5`): the user's
  "keeps sounding after the release" was the start sound (3.63 s, on its
  own channel) never stopped and the release detected only once the last
  round aged out (2 ticks); now the release cuts the start and the loop
  in its tick (attack let go), plays the end on no channel (a new burst
  leaves it alone) and the pilot light (also at each draw, not after
  running dry; stopped by a burst and a holster). The fades are a new
  general message, `map::FadeSound` (a straight-line volume ramp of a
  source's playing entry; playback's `audio::sound::VolumeRamp`; sent to
  other clients as `ServerFadeSound`). Tests: `flames::*_14_2` (own
  machine, running dry, the measured volumes, another client).
- **Grenade Launcher's drum**: a general data rule, `[[anim.procedural]]`
  (`weapon::view_bones`: a view-model bone turned `per_shot` degrees per
  round about its z, smoothstep over `time`); the launcher's
  `procedural_chamber` 60 degrees over 0.14 s. First person only, never
  in reloads. Tests: `weapons_more::*_52` to `_55`.

### The Flame Thrower's sounds, the Grenade Launcher's drum (spec-tf2-flame-drum, 2026-11-21)

Spec pass from two user reports (ours' flame keeps sounding long after
the release; how the drum turns). Measured on real TF2, written as
effects.md §14.2 and weapons_more.md §9.6 (test cases there):

- **Flame Thrower** (`flame_sounds.tf2` own view, `flame_sounds_bot.tf2`
  a puppet Pyro heard by a spectator; the sound emitter's trace plus
  `snd_show` photographed per tick: each channel's volume 0-255, time
  left, loop flag). A burst starts the start sound at 1 and the loop at
  0, straight-line fades over 3.5 s of game time; the start's wave
  (3.63 s, no loop) runs out on its own. The release cuts **both** in
  its tick, whatever the burst's length, and plays `.WindDown`
  (`flame_thrower_end.wav`, 0.82 s, not `.FireEnd`); the pilot light is
  a 12.77 s wave played **once** (not looped, 0.25) at each draw and
  each release, stopped by a burst and a holster, not restarted when
  the ammo runs out. A new burst doesn't cut the end sound. Others hear
  the same 0-2 ticks later. `snd_show 1` works headless (the channel
  list drawn over the frame; tesseract reads it from the pictures).
- **Grenade Launcher's drum** (`gl_drum.tf2` / `.scene` own view at 60
  fps, `gl_drum_bot.tf2` a puppet from 35 units; the model's files; the
  server's view model every tick through VScript): six chambers round
  `procedural_chamber`'s z axis; no sequence turns it (`weapon_bone_2`
  stays on `weapon_bone_1`), no bone controller or procedural rule: the
  client's code. Own view: one chamber (60 degrees) per shot, clockwise
  seen from behind, smooth over ~8 frames from the frame after the
  shot; none while reloading; others see no turn at all. The curve's
  exact shape is weapons_more.md Open questions 24.

### The Pistol's reload unheard when served (tf2-pistol-sound, 2026-11-21)

- **Cause** (measured in Ultimate Gun Game's sandbox, offscreen, the
  install): the event path works: the Scout's arms play `ReloadScout`
  and `ClipIn` at their times, the Engineer's (`pstl_reload`) `ClipIn`
  alone, a draw its `Draw`/`Draw2`. But the Engineer's whole reload is
  `Weapon_Pistol.ClipIn`, which joined the Pistol's set with the event
  change; a store published before it holds the entry (the sound
  scripts) without its wave, so a served Engineer heard nothing and a
  served Scout half of it. Silently: playback skipped an entry with no
  wave or none at all.
- **Fix**: playback says once per entry and map why a sound asked for
  isn't heard (`audio::sound::unheard_warning`: not in the table, or no
  wave read); an owner's view-model sound event without a wave says so
  (`view_anim::unplayable_view_sound`) and, in a reload's sequence
  (`ViewModelEvent::reload`), plays the gun's world reload in its place.
  Tests: `viewmodel_sounds::a_reload_event_without_waves_plays_the_world_reload_instead`,
  `heavy::view_sounds` (the install's own view models and sound table,
  Scout and Engineer, and the Engineer without `ClipIn`'s wave). Open
  question 9 is the republish.

### CS:S and HL2:DM on the view models' sound events (css-vm-sounds, 2026-11-20)

- **CS:S**: every gun's reload, draw, bolt, shell and silencer sound is
  its view model's event 5004 (`view_anim::event_effect`), the local
  player's alone; the 24 guns' `[sounds]` lost their `reload`, `draw`,
  `shot`, `finish` and `modes` timelines and name the entries in `view`
  (what a map loads; `heavy::view_sounds` checks every sequence's events
  and times against the install, the AK-47's and AWP's against
  view_models.md 6 and mdl_v48.md). The grenades' pin pull is the
  `pullpin` events' (`pin_sounds` emptied); the C4's `c4.click` events
  stay the bomb's own timer. The AWP's three `Weapon_AWP.Bolt` events on
  `awm_fire` are heard now (the data never had them).
- **HL2:DM** (effects_sounds.md 15.3): the .357's four loader sounds and
  the AR2's rotate and push sounds are view-model events; the AR2's and
  the crossbow's generic reload sound is `[sounds] own_reload`, the
  holder's alone (`weapon::world_reload_sounds`); the pistol's, SMG1's
  and shotgun's stay timed (the spec's "shared": everyone hears them).
- **Others hear CS:S reloads** (css-others-reload, sounds.md 5.1,
  measured): the reload sequence's sound events play on every client
  for other players too, from the reloader, the first part (cycle 0) as
  the reload's start arrives, the later ones `OTHERS_RELOAD_LAG`
  (0.075 s) behind. Data-driven: `ViewAnimRules::others_reload`
  (None: owner only) marks a reload sequence's events
  (`ViewModelEvent::others`, `ViewModelPlay::reload_seq`); no new
  network traffic (the reload-start replication drives it). Draws and
  fire sequences' bolts stay owner-only; the knife's deploy is the
  server's sound (`[sounds] deploy`). Replaces "others hear nothing" below.
- **Spec gaps** (older, see above): neither spec said what others hear of a CS:S gun's
  reload then. The S.L.A.M.'s `TripMineMode` event
  (effects_sounds.md 15.3) is on a sequence our rules never play.

### The spool-down's finish, view-model sounds, the Minigun flash's fades (tf2-vm-sounds, 2026-11-20)

- **Sequence finished** (`map::anim::finished_cycle`,
  `ticks_until_finished`, `Animator::sequence_finished`; every game):
  a non-looping sequence counts as finished past 1 − fade-out × cycle
  rate × playback rate, the check before the tick's advance. The
  Minigun's switch away (empty auto-switch and by hand) waits
  `spool_down` 1.333 s with `spool_down_fade` 0.2 s: **77 ticks** after
  the wind-down, as measured (`running_dry_and_the_switch_8_9`,
  `dry_with_attack2_never_idles_10`, weapons test 36: locked at 76,
  free at 77). A new sequence's playback rate is 1 (`Animator::play`,
  `restart`, as before). The driver's idle still waits for the end.
- **View-model sounds** (viewmodel_sounds.md): Source's event 5004 is a
  `ViewModelEventKind::Sound` (`source_engine::view_anim::sound_event`,
  TF2's `view::event_effect`), played for this machine's own player
  only as the animation passes it (`view_anim::play_view_sounds`).
  Everyone else hears the gun's `[sounds] world_reload` at the reload's
  start, per shell for a one-at-a-time reload (`weapon::world_reload_sounds`,
  `map::OthersSound`: played unless by the `LocalPlayer`; the server
  sends it to every client but the shooter's). TF2's data lost its
  reload and draw timelines (the arms' events carry them, the draws'
  included; `heavy::guns` checks every stock reload's events against
  §4 and that each draw has one; the Pistol's `ClipIn` and `Draw2`, the
  Pyro's Shotgun draw and the Grenade Launcher's `DrumStart` joined
  the gun sets). The Scout hears `ReloadScout` at tick 2 and `ClipIn` at
  tick 40 (`viewmodel_sounds` tests); the Engineer's `pstl_reload` carries
  `ClipIn` alone (`heavy::guns`). CS:S
  and HL2:DM follow (css-vm-sounds, below). Left: our
  whole-clip reload isn't cut by attack (weapons_more.md Open questions
  6), so test case 3 is checked with a switch.
- **The Minigun flash** (particles.md §8.3, §8.3.1, §10.11): an
  emitter's own fade runs on its system's clock (`map::psys::OpFade`;
  strength 0 skips, a continuous rate × strength, a hard stop past the
  fade-out end), each child at its own cap. With the install's
  `muzzle_flash.pcf` every per-frame count of §8.3.1 agrees
  (`heavy::particles::the_minigun_flash_by_its_caps_and_fades_8_3_1`;
  core 2 a frame, starflash 4, 4, 4, 5, 4, 4 to 25, flare 2 to 10, sparks
  1, 3, 5 and held, nothing of the starflash or flare after 0.6 s; smoke
  from 2 s). A new system per burst (`each_minigun_burst_starts_a_new_constant_system`,
  as before). Two fixes on the way: a full step's emission counts the
  step's length, not the clock's difference (a 120/s emitter lost a
  particle to rounding), and a particle dies at its life within 1e-6
  (a 0.1 s life is exactly 6 steps at 60).
- **Spec question** (particles.md §10.11 against §8.3): the starflash's
  and flare's "operator fade oscillate" is 0.12 in the install. By
  §10.11 the clock would be taken modulo 0.12 and they would pulse every
  0.06 s for good, which contradicts §8.3 ("never again in that
  system") and §8.3.1's frames 4-6. Ours follows §8.3 (oscillation read,
  not applied: `OpFade::strength`). Which is right?

### The stock Minigun and Pistol rechecked (stock-recapture, 2026-11-20)

The comparisons that used the local player's Minigun (this account's is
the Tomislav, 424) or Pistol (the Engineer carries the Wrangler: the
old `pistol` captures showed his Shotgun) redone with the stock items
swapped in (`stock tf_weapon_minigun 15`, `stock tf_weapon_pistol 22`,
`stock tf_weapon_pistol_scout 23`), each run holding the scene lock with
class and item read every tick (VScript) and `air_density` 2 checked
(a spec probe had changed it on the same instance; the first captures,
taken in that window, were thrown away and retaken).

- **Minigun, per tick** (own player, `sv_soundemitter_trace 1`,
  `host_framerate 0.015`, VScript: `m_iWeaponState`, ammo, the view
  model's sequence): held 200 ticks: wind-up 0, fire loop 50, first shot
  57 (`m_fire` from 58), fire loop cut and wind-down 211, wind-down cut
  411: as minigun.md 4.4 and ours (`minigun::held_then_released_1_2`).
  attack2 held 150 ticks: spin loop 50, wind-down 20 ticks after the
  last held tick, cut 170 later (ours 20 and 169). A 10-tick tap: the
  wind-up completes at 50, the wind-down cut 51 later (ours 50). Dry
  (20 rounds): dry at the next due shot, released 3 ticks later, the
  wind-down at entering dry + 20 ticks (ours the same rule). The
  view model's `m_spool_down` runs 1.333 s (cycle 0.01125 a tick).
  Spec Open questions 1 and 2 answered by measurement: the spin-idle
  hold is 0.3 s from the last tick attack2 was held, and the owner's own
  wind-down is cut when the barrel stops (200 ticks after full speed),
  as ours does. Nothing changed: nothing of ours was tuned to the
  Tomislav (its numbers come from the spec).
- **Spec question (minigun.md 3.3, Open questions 3)**: the stock empty
  Minigun switched to the Shotgun 77 ticks (1.155 s) after the
  wind-down began, at `m_spool_down` cycle 0.855, twice (not at the
  sequence's end, 1.333 s, which the spec gives and ours uses: 85-93
  ticks, `running_dry_and_the_switch_8_9`). The Tomislav's 1.11 s was
  read as 1.333 x 0.6 / 0.75; stock's 1.155 doesn't fit that rule. What
  ends the hold? Ours unchanged until answered. **Answered**
  ([minigun.md §3.3](../../../specs/tf2/minigun.md)): a non-looping
  sequence counts as finished once its cycle passes 1 − fade-out
  (0.2 s) × cycle rate, 0.85 for `m_spool_down`; the playback rate is 1
  for every variant (a new sequence resets it); the check runs before
  the tick's view-model advance: 77 ticks.
- **Pistol, per tick**: a shot every 10 ticks (10/11 alternating at
  0.15 s); `cl_autoreload` starts the reload one interval after the last
  shot; the whole clip in 69 ticks on the Engineer (1.035 s) and 67 on
  the Scout (1.005 s), as ours (`reload = { scout = 1.005, engineer =
  1.035 }`). Sounds: the Scout's `Weapon_Pistol.ReloadScout` at the
  start, then `Weapon_Pistol.ClipIn` ~0.6 s in (client, view-model
  timing); the Engineer's only `ClipIn` (~0.45-0.65 s in, two runs).
  Ours plays `ReloadScout` at the start for both classes and no
  `ClipIn` (weapons_more.md 9.4 names `ClipIn` for the Engineer and only
  `ReloadScout` for the Scout, with no times): left, a spec question
  (when `ClipIn` plays, and the Scout's second sound). **Answered**
  ([viewmodel_sounds.md](../../../specs/tf2/viewmodel_sounds.md)):
  the shooter's reload sounds are the view model's animation sound
  events, per class (Scout `p_reload`: `ReloadScout` 0.033 s, `ClipIn`
  0.600 s; Engineer `pstl_reload`: `ClipIn` 0.467 s), played for the
  shooter only; others hear the script's `Weapon_Pistol.WorldReload`
  from the server at the start. Every stock reload's events are listed
  there.
- **Pictures** (`muzzle.tf2`/`.scene` now with a `pistol_scout` section;
  `barrel.tf2`/`.scene` new: the Minigun 50 and 100 ticks into attack2,
  then firing): the Engineer's and Scout's Pistols at rest and their
  flashes match in place, size and life. The Minigun at rest sits where
  TF2's does; spinning up, both view models lower the gun out of sight
  (only the fist shows, at the same place), so the barrel's turn can't
  be seen in first person in either (third person: `minigun_sounds`/
  `thirdperson`, bots, stock). The constant flash: the table's row.
- **Other tuning on non-stock items** (audit of this log): the
  Quickiebomb (rechecked: sticky-recheck), the Tomislav and the
  Engineer's Pistol (above) were the only own-player comparisons on
  non-stock items. The Pyro's flames, the Medic's needles and every
  third-person or bot-eye scene used puppet bots (stock); the Wrench
  (`impacts.tf2`) is stock; no other melee was compared. minigun.md's
  own Tomislav measurements (Open questions 2, 3) are answered above.

### Stock items in every scene; the sticky rechecked (sticky-recheck, 2026-11-20)

- **Audit** (VScript: each player's `m_hMyWeapons` item indices): the
  local player's loadout is stock for the Scattergun, Rocket Launcher,
  Grenade Launcher, Shotguns, SMG, Sniper Rifle, Revolver and Wrench;
  not stock: the Demoman's sticky slot (Quickiebomb, 1150), the Heavy's
  Minigun (Tomislav, 424; `muzzle.tf2`'s `minigun` captures were of it),
  the Engineer's secondary (the Wrangler: `muzzle.tf2`'s `use
  tf_weapon_pistol` failed, so its `pistol` captures showed the Shotgun),
  the Pyro's (Dragon's Fury), the Medic's (Crusader's Crossbow) and most
  melees. Puppet bots (every class) hold the stock items. `muzzle.tf2`
  now swaps in the stock Pistol (22) and Minigun (15) (not re-captured
  yet: the muzzle rows for them stand on the wrong guns); `pose.tf2`'s Heavy
  is `-class heavy` (`heavyweapons` gives bots a random class);
  `stickies.tf2` records the whole 1.6 s (`wait 96` gave 1.07 s, the
  pulse missing). tf2ref deletes its `stock` script once run.
- **Sticky, stock launcher** (`stickies.tf2`, `launch.tf2`; pairs in
  scratch): the launch flash (a small glow left of the sight, two frames)
  and the sticky's first showing (5 frames after the shot) match; its
  flight and resting place agree within 6 px. In `launch.tf2` (10
  degrees down) ours first shows one frame later than TF2's, then at the
  same places. Measured per tick with VScript (30 degrees down, held 0.1
  s): launch velocity 937 along the aim, 200 along the aim's up, ±3 to
  the right, as weapons_more.md 5.1.
- **Armed pulse**: stock TF2's first ring comes 26 frames (30 a second)
  after the sticky first shows, the Quickiebomb's 20 (it arms 0.2 s
  sooner); ours came at 24, tuned to effects.md's measured 0.8 s, which
  was the Quickiebomb's. Now `armed.after` 0.87. Spec question:
  effects.md "Measured" says 0.8 s after the launch; stock is ~1.03 s
  after the launch flash. **Answered** (tf2-spec-sticky, 2026-11-20):
  [effects.md §11.3](../../../specs/tf2/effects.md): the client starts
  the pulse system L (0.8 stock, 0.6 Quickiebomb) after it first
  *received* the sticky, and the system's first ring is seen ~0.1 s + a
  frame later (its emitter's timing); both captures fit. Ours should
  start it L after the arrival rather than a tuned 0.87 after the shot.
  **Done** (sticky-physics, 2026-11-20: Log).
- **Spin** (spec question, not changed): stock TF2's sticky turns at
  ~2510 degrees/s just after launch, about its own forward ~1200 and
  its left ~2200 (`GetPhysAngularVelocity` agrees: 1194, -2207), twice
  weapons_more.md 4.1's impulse (600, ±1200, 0) degrees/s, which ours
  uses. Both its linear and angular speed fall ~0.4-0.5 % a tick in the
  air (the engine's drag, weapons_more.md Open questions 7); ours has
  none. Is the impulse doubled by the physics engine (an impulse on a
  body whose inertia differs from 1), or are the units other than
  degrees/s? **Answered** (tf2-spec-sticky, 2026-11-20):
  [weapons_more.md §4.1](../../../specs/tf2/weapons_more.md): the game
  adds the same (600, r, 0) degrees/s twice, so (1200, 2r, 0) (grenades
  too); the drag, measured per tick with `air_density` 0/2/4, is §4.4:
  quadratic, per body axis, ×ρ (2.0), gravity applied first (sticky
  linear ≈ 1.43e-6 × ρ × Σ|v_i| per tick, angular ≈ 7.3e-7 × ρ ×
  Σ|ω_i|). New open: weapons_more.md Open questions 22-23 (the
  grenade's angular drag; the engine's 2000 units/s cap vs a full
  charge's 2400).
  **Done** (sticky-physics: spin and drag; the cap not modelled).
- `viewmodels.tf2` slot 1 (the puppet Demoman): the stock Stickybomb
  Launcher (item 20 active), not the Grenade Launcher; ours draws the
  same gun, ~50 px left of it (the bot's view-model offset).

### The blast that drew nothing (blast-regression, 2026-11-20)

- **Cause** (a4a41fa74, particle timing §9): a system's creation frame
  runs with zero time and its children start the frame after; but
  `Instance::finished` counted a not-yet-started child only while
  `age < delay`, so a child at delay 0 didn't count at age 0. Every
  system made only of children (`ExplosionCore_Wall`, `_MidAir`,
  `_MidAir_underwater`: rockets', pipes' and stickies' blasts) was
  dropped as finished in its creation frame by `StartedSystems::step`
  (and map `info_particle_system`s by `update_systems`). Measured, not
  pictured: `psys_running` every 1/60 s in `streaks` showed no
  `ExplosionCore_*` at all while the `Tf2Explosion` look was due and
  written (`StartParticleSystem` found, no start failure). The heavy
  particle tests stepped `Instance`s directly and never asked
  `finished`, so they didn't see it.
- **Fix** (`render::psys`, every game): a child not started yet counts
  while the parent emits and `age <= delay`. Tests
  `psys::a_started_system_of_children_only_survives_its_creation_frame_9_1`
  and `heavy::particles::a_started_blast_draws_its_core_flash_and_embers_
  within_three_frames` (all three blast systems via `StartedSystems`, the
  install's files): both fail before the fix. CS:S's HE grenade draws no
  `.pcf` system (unaffected).
- Capture (`streaks`, ref | ours, from the blast's first frame on, ours
  f14 against TF2's 15): the core flash, embers' rays and fireball draw
  again in the same sequence as TF2's.

### Particle timing, the rocket's first places (tf2-psys2, 2026-11-20)

- **Particle timing** (`render::psys`, every game; particles.md §9): a
  system's creation frame runs with zero time (initial particles only,
  no emission, children not run); a child starts the frame after with
  its points standing still; a continuous emitter's n particles of a
  step at k/n of its window (the window rule: an endless one not cut at
  its start), a burst at its start time on the point's previous place;
  each new particle where its points were at its creation time (replaces
  the 1/128 spacing and `StartParticleSystem::from`). Tests
  `psys::a_new_system_makes_only_its_initial_particles_then_spreads_
  along_the_path_9`, `continuous_counts_and_creation_times_follow_the_
  window_rule_9_2`, `a_burst_at_its_start_is_made_on_the_points_
  previous_place_9_2`.
- **`Explosion_Flashup` now draws**: §9.4 makes its 30 particles (no
  step-boundary rule any more), and under our readings of its operators
  (Alpha Fade and Decay 0 → 1 over 0.1–0.2 of life, out over 0.7–1.0;
  life U(0, 0.2); trails 3–55 wide) they are visible, where TF2 shows
  nothing. Spec question: what hides them in TF2 (the fade's times
  proportional or not, the plane constraint, the trail renderer?).
  **Answered (tf2-spec-flashup, particles.md §10.1):** nothing hides them
  in a blast (TF2 draws them too: alpha 1 before 0.1 of life, lengths ≥
  32, widths min(L, r)); the alone capture made none because the
  `info_particle_system` client skips a late effect ahead (§10.10) past
  the 0.01 s emitter. Our readings of Alpha Fade and Decay, Radius Scale
  (rational bias), Movement Basic (drag per 1/30 s) and the trail width
  are corrected in §10.2–10.7.
  Test `heavy::particles::the_blast_draws_thin_ember_rays_broad_core_
  streaks_and_flashups_thirty` checks the 30.
- **Case A or B** (effects.md Open questions 16, checked with
  `launch.tf2`'s rocket at `cl_interp 0.5` and `cl_updaterate 66`, both
  `notconnected`: set before a `map`): not A. TF2 keeps the rocket at p0
  longer than one snapshot: at the defaults until about when the drawn
  time reaches its arrival (~frame 9), at lerp 0.5 ~0.3 s then thrown
  back; observation written into the Open question. Ours (`net::late`):
  the copy comes as the first snapshot would carry it (here on the
  server's tick count, 20 a second: 0–4 ticks out), its place in whole
  units (`whole_units` in the projectile/sticky data), drawn there while
  the drawn time is before its arrival (no seeded line behind it), then
  interpolated; the trail made on it in its first frame (one puff), its
  fire and sparks from the next. Tests `late_items::a_copy_stays_...`,
  `a_late_flyers_copy_comes_with_the_next_snapshot_in_whole_units`,
  `effects::a_late_rockets_trail_is_made_on_its_copy_at_its_first_
  place_11_2`. Pair (`launch`, frames 2 and 4): the glow ahead of the
  launcher now in both, at the same place; ours a frame earlier in the
  movie numbering and its first smoke puff shows darker than TF2's under
  the glow. Matches the defaults, not lerp 0.5.
- **The crit glow** (effects.md §14.1): `particles/firstperson_weapon_fx.pcf`
  joins the looks' files (the `players` set: republish awaits the user,
  item 8); "Position on Model Random", "Movement Lock to Bone" and
  "Oscillate Scalar" now run (`render::psys`, every game; the
  oscillator a placeholder: a sine rate on radius or alpha, Open
  questions 17; **answered**: particles.md §10.9). The host model each frame (`ParticleModel` on the
  weapon, `Muzzles::glow_model`): the local player's arms view model's
  hitboxes (47 on `c_soldier_arms`, drawn in the view-model pass), else
  the held weapon's (2 on `c_rocketlauncher`), loaded as `$hitbox`
  attachments (`props::add_view_hitboxes` / `add_held_hitboxes`).
  Tests `psys::particles_go_on_the_models_boxes_by_the_bias_and_follow_
  them_14_1`, `crit_boost::others_glow_stands_on_their_held_weapons_
  hitboxes_14_1`, `heavy::guns::the_crit_glows_hosts_carry_their_
  hitboxes_14_1`. Not compared with a capture (nothing in ours grants a
  boost in the sandbox).

### Emission spread, blasts seen through water, body decals (tf2-psys, 2026-11-20)

- **The "under water" captures were from above the surface.** `wunder`
  puts both eyes at z -132 (`getpos`), just above 2fort's river: TF2
  shows the riverbed blast through the surface, ours drew nothing there.
  Measured with a new probe, `particles_drawn` (`render::particles`):
  every system of `ExplosionCore_MidAir_underwater` drew its quads
  (flash 1, core 11-12, bubbles 64...) with fog ~0 (the eye is out of
  the water), and with the eye truly under water (-182) the blast showed
  plainly (fog 0.3-0.4). So not fog and not loading: the water surface
  is opaque in the transmissive phase, refracting the main view's copy
  taken before the transparent pass, so particles under it were neither
  in the copy nor in front of the surface's depth. Now particles in a
  water volume with the eye out of the water draw before the surfaces
  (`ParticleDrawMaterial::before_water`, Bevy's alpha-mask phase, blended
  as before, no depth written), every game. Pair (`pairs3`,
  `wunder_f30`): the orange glow and bubbles on the bed through the
  surface in both; TF2's smoke ends nearer the blast. Test
  `particle_depth::particles_in_water_seen_from_above_draw_before_the_surface`.
- **Emission spread** (`render::psys`, every system): a step's new
  particles are made where their control points were at their creation
  times, the points moving straight from the last step's places
  (inferred from effects.md 11.1's nuance; particles.md names no rule:
  spec question). A started system can name where its point 0 was made
  (`StartParticleSystem::from`): TF2's trail on a late copy is made at
  the rocket's first received place (`LateCopy::first`), so its first
  step spreads from there back to the copy behind the eye; the water
  trail is chosen by that place. Tests `psys::a_moving_points_new_
  particles_spread_along_its_path`, `effects::a_late_rockets_trail_is_
  made_at_its_first_place_ahead_of_its_copy_11_1`. Pair (`wunder_f02`,
  `_f04`): still differs. TF2: a large glow (~10 units) with burst
  streaks ~50 units ahead of the launcher from frame 2; ours: a smoke
  puff at the launcher, no glow. Ours' p0 is the launch point (the
  sandbox has no snapshot wait; TF2's first snapshot puts p0 up to 50
  units out) and the first step's two fire glows are due 1/128 and
  2/128 s into the step, i.e. half-way and near the end of the path
  (behind the eye); a glow at p0 needs a particle at the window's start
  or p0 further out. Spec questions: when a continuous emitter's first
  particle is made (at its start, or one interval in), and where a
  listen host's own client first receives a projectile.
  **Answered** (tf2-spec-psys): [particles.md §9](../../../specs/tf2/particles.md)
  (the k-th of a step's n particles at the window's start + k/n of it,
  never at the start; placed at the point interpolated straight between
  its previous and current place; nothing emitted in a system's creation
  frame, children a frame later with no spread) and
  [effects.md §11.2](../../../specs/tf2/effects.md) (p0 0–4 ticks out in
  whole units; the trail made after the frame's interpolation; the
  captures fit the rocket drawn at p0 until its second snapshot, the
  fire and burst emitted there, then the jump back; Open questions 16).
- **Body decals need the mesh** (`render::body_decals`, every game): a
  decal only where the ray from the shot's start through its point (8
  units past, + 10 %) meets the body; before, a miss fell back to the
  point itself and put the decal on whatever part lay within its
  rectangle there (TF2's melee box centre in the air beside a body:
  effects.md 10.1 "no body decal"). Test `body_decals::a_contact_point_
  in_the_air_beside_the_body_leaves_no_decal`.

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
- Post-push: the run before this push (3a9704b92) failed
  `cs_source net_interp::others_are_drawn_smoothly_on_the_servers_path`:
  it still expected `cl_interp` 100 ms, where 3a9704b92 made the delay
  whole ticks plus one (109.375 ms at 64 Hz); the test now asks
  `interp_amount` (3b0a5f751: PASS). The full run on the tree with this
  change (8cd934e6f..b66beea6b): 6388 of 6390 passed; the other failure,
  `ultimate_gun_game session::a_session_with_every_game_stops_making_
  archetypes` (1015 archetypes, bound 1000), is from the body decals
  commit b66beea6b (`BodyDecalsOffRagdoll` comes and goes), left to it.

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
  **Answered**: [effects.md §11.2](../../../specs/tf2/effects.md) (see
  the tf2-psys entry).
- **The Stickybomb Launcher's first-person pose** (`sticky_pose`):
  matches (2026-11-20). Not a pose bug: the reference held this
  account's Quickiebomb Launcher (item 1150, its own model; read with
  VScript), not the stock one. With item 20 swapped in (tf2ref's new
  `stock` directive; `launch.tf2` and `stickies.tf2` now use it too, so
  their earlier sticky captures were of the Quickiebomb) TF2 and ours
  draw the same picture. Each merged bone at `sb_idle` frame 0 keeps
  the `c_` model's own place relative to `weapon_bone` (as for the
  Grenade Launcher), so the gun is drawn whole (`heavy::guns::
  the_launchers_bones_sit_where_tf2_holds_them`). (`viewmodels.tf2`'s
  `demoman_pipebomblauncher` shows the stock Stickybomb Launcher: the
  bot's active item read 20 with VScript; the 2026-11-20 recheck.) Also fixed earlier: the
  two heavy view-model tests indexed the animator's pose by the
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
- `cl_autoswitch_empty` likewise (the Party tab's "Auto switch when empty
  (TF2)"), default 0 by the user's choice: an empty Minigun let go of
  switches away (minigun.md 3.3, 77 ticks after the wind-down) only with
  it 1; off, it stays in hand. Manual switching is unaffected.
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
- Same pass, 10.1, 12, 14: the melee contact point (`Swing::box_centre`,
  `refine_box`: a box hit on a player at the box's centre, one on the world
  refined by the doubled line and the crouched player box's corners) and
  the impact ray (`ImpactStyle::melee_ray`: decal and flecks only where
  the ray from the eye through the contact, 8 units on, x1.1, reaches the
  world; the sound at its hit). Needles: not drawn late (each client's own
  in TF2), stuck in the world until 6 s from the launch (`marks_stay`,
  `projectile::StuckMark`, no trail), TF2's impact sounds skipped beyond
  1024 units (`ImpactStyle::sound_reach`), bullets' 3-in-10
  `Bounce.Shrapnel` (effects.md 7; it was off) and RED needles' alone
  (`mark_ricochet`), an enemy hit's 2 blood traces. Crit boosts
  (`weapons::crit_boost`): `CritBoosted` replicated, `Weapon_General.CritPower`
  and `critgun_weaponmodel_<team>` while boosted with a boostable weapon,
  kept across a switch, cut when it ends; the Flame Thrower's crit loop
  swap (`loop_crit_sound`). Nothing grants a boost in ours yet. The
  `critgun_weaponmodel_*` systems' particle file is unnamed in the specs
  (not loaded until it is; asset pack: that file, and
  `weapons/crit_power.wav` in `players`).
  **Answered**: `particles/firstperson_weapon_fx.pcf`, on the local
  player's arms view model / others' weapon, placed on the host's
  hitboxes ([effects.md §14.1](../../../specs/tf2/effects.md)).
- Pass 9 capture (`water`, `pairs2`): with the seeded history ours' rocket
  is drawn from behind the eye and its trail's glow passes the camera from
  frame ~8 (60 a second), TF2's glows at the launcher from frame 2: TF2's
  emitters spread a step's particles along the control point's path, so
  its first step (made at the first received place) lies in view (11.1,
  inferred). Ours puts a step's particles at the control point; starting
  the trail at the first place instead left a still smoke puff at the
  launcher, so it starts where the copy is drawn. Next: spread a step's
  emission along its control point's path in `render::psys`. The
  underwater blast is still invisible from 290 units under water (TF2: an
  orange glow with bubbles at the trail's end, where ours' trail ends
  too): the centre (1 unit off the bed) and the water effect are right
  (test `a_rocket_on_a_river_bed_shows_the_water_effect_13`); the cause
  is in drawing it under water (fog on added particles suspected,
  unmeasured).
- 2026-11-20 (psys-operators, from 16f03b19e's spec): particles.md §10
  in `render::psys` for every game. Alpha Fade and Decay and Radius
  Scale write from the initial value inside their windows and leave it
  outside (the radius by the rational bias, g(0.5) = b; the loader orders
  the fade windows: `map::psys::fade_windows`); Color Fade from the
  initial colour, eased by default; Movement Basic's drag per 1/30 s
  (`DRAG_STEPS` 30) and every operator over the whole step (a particle
  made in it moves v h); sprite trails at least their minimum (it wins
  over the maximum), no wider than long, none at age 0; the render
  alpha byte (0 not drawn); Oscillate Scalar / Vector per §10.9
  (replacing the placeholder: R h W(x), the parabolic wave, per-particle
  fixed draws). An info_particle_system's new effect is skipped ahead
  (`Instance::skip_to`, §10.10, particles_and_smoke.md 2.5) by one
  server tick (the start time isn't sent): no initial particles, a
  continuous emitter ended before never emits, a burst fires late.
  `Explosion_Flashup` draws in blasts (30 made, ~a dozen at full alpha,
  strips 32 long) and nothing when skipped. Tests `psys::*_10_1` to
  `*_10_10` (12 cases of the spec's table) and
  `heavy::particles::the_blast_draws_...` (the install's Flashup).
  Instructions to step and draw 90 frames: `ExplosionCore_Wall` 10.35M →
  10.11M, `critgun_weaponmodel_red` 4.26M → 4.58M (the oscillator now
  runs per spec). Capture (`streaks`, frames 2-10 after the blast, ref |
  ours | main before this): **neither ours draws the blast at all** in
  the 40 frames (only the held rocket glow, then the trail's smoke),
  where pass 8 showed the core flash and embers: a regression on main
  before this change (same in both builds; `BlastMeasured` comes in
  both), not yet found. Next: find why the sandbox's rocket blast
  shows no effect in `streaks.scene`. **Found and fixed** (blast-regression,
  Log).
- 2026-11-20 (sticky-physics, from 6af57cc46's specs): launch spin per
  weapons_more.md 4.1 for stickies and pipes: (600, r, 0) degrees/s added
  twice, (1200, 2r, 0), r a whole number in ±1200 from the shot's seed
  (Chimera's `weapon::flight::LaunchSpin`, the data's `spin`; pipes'
  through `chimera:projectile`). Air drag per 4.4 on both (Chimera's
  `flight::AirDrag`, the data's `drag`: after gravity each tick the
  velocity × (1 − ρ Σ c_i|v_i|), the spin × (1 − ρ Σ a_i|ω_i|), in the
  body's axes; ρ the game's `<game>:air_density` cvar, TF2's 2.0). The
  sticky's coefficients as fitted (third angular term 7.3e-7 as the spec
  assumes); the grenade's linear as fitted, its angular ours (a uniform
  3.3e-7: ~0.22 % a tick at (1200, 2200, 0), Open questions 22); no
  2000 units/s cap (Open questions 23). The armed pulse per effects.md
  11.3: on each machine the first frame the arm time L after it received
  the sticky, L = the launcher's `arm` + `sticky_arm_time` (new on the
  launcher's data, 0 stock; detonation uses it too); the looks' tuned
  `armed.after` is gone (read from the data). Tests 29b-d, the pulse's
  rows (stock 0.8, Quickiebomb 0.6). No capture made.
- 2026-11-20 (spec pass, css-spec-reload; CS:S sounds.md 5.1, Open
  question 9 closed): measured on the real CS:S client. Others **do** hear
  a CS:S gun's reload parts: the view model's reload events, same names,
  order and gaps, played client-side from the reloading player (CHAN_ITEM,
  70 dB), the first part when the reload's start arrives and the later
  ones about 0.075 s behind it. Others hear no draw sound (the knife's
  server-sent draw aside), no bolt after an AWP/Scout shot and no other
  firing event. So e177175a6's "others hear nothing" is right for draws
  and shot bolts but wrong for reloads. HL2:DM unchanged: its "owner"
  sounds (.357, AR2, crossbow reloads) reach nobody else (effects_sounds.md
  15.3).
- 2026-11-21 (needle-muzzle, playtest report "the syringes don't come
  out where the muzzle flash is"): measured, no fault. TF2 itself draws
  the shooter's needle from its shot start (16 forward, 6 right, 8 down
  of the eye), not from the view model's muzzle: in
  `tools/tf2ref/scenes/needles_fp.tf2` (the account's Medic with the
  stock gun swapped in by `stock tf_weapon_syringegun_medic 17`) and
  ours' `needles_fp.scene` the needle first shows at the drum's left
  edge, left of and below the flash, and the two games' needles sit
  within a few pixels of each other frame by frame (4:3 still, 4:3
  strafing right; ours at 16:9 the same layout). The flash is at the
  re-projected muzzle (`psys_running`: 62 forward, 15 right, 13 down of
  the eye in world terms, particles.md 6). Written into effects.md
  "Measured"; `weapons_more::a_needle_starts_at_its_eye_offset_not_at_the_muzzle_8`
  pins the start so the needle isn't "fixed" onto the muzzle. Seen in
  passing, not changed: flyers (`projectile::Flyer`) have no
  `Interpolated`, so a projectile not drawn late is drawn at its newest
  tick while the camera is eased one tick behind (up to one tick of
  flight ahead, 15 units for a needle; a moving shooter's start off by
  his tick's move). Fixed 2026-11-21: the client makes every flyer
  `Interpolated` (`client::interp`), drawn between its last two ticks
  like the world, first at its start; the late copies still sample the
  latest tick (`net::late`);
  `weapons_more::a_needle_is_drawn_between_its_ticks_from_its_start_8`.
