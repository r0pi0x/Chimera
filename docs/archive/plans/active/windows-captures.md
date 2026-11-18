# Captures on the Windows PC (CS:S client)

What's left of [parity-gaps.md](parity-gaps.md) "Captures wanted" needs the
real CS:S client: screenshots, the client's own sounds, a human player.
Everything server-side is measured already (probe server, SourceTV demos).
This is a checklist for one or two sittings; each item names its capture
in parity-gaps.md, where the full description is.

Keep the files out of the repo (they are game output): put them in one
folder, e.g. `Documents\lp-captures\<date>\`, and send the folder (or a zip)
to the coordinator session. Name each file after its item (`C7_bomb_1s.png`).

Setup once: CS:S, windowed 1280x720 (`-windowed -w 1280 -h 720`), default
settings otherwise. Console on (`con_enable 1`). Screenshots with `jpeg` or
F5 land in `cstrike\screenshots\`.

## Sitting 1: a listen server with bots (about 45 min)

Create Server, de_dust2, 10 bots, `sv_cheats 1`, `mp_roundtime 2`.

1. **A POV demo for `demosim`** (most useful single item): `cl_cmdrate 66;
   cl_updaterate 66; record pov1`, play two rounds normally (walk, run, jump,
   crouch, shoot every gun you can, throw grenades), then `stop`. The file is
   `cstrike\pov1.dem`. A second one on a surf or bhop map if you play those.
2. **C7 round win panel** and **C12 win panel contents**: screenshots 1 s and
   4 s after each round end (elimination, bomb, defuse, time), with the
   scoreboard (Tab) once per round.
3. **C8 damage indicator**: `bot_stop 1`, get shot from left, right, front,
   behind (`setang` to turn); screenshots 0.1, 0.5 and 1 s after each hit.
4. **C3 weapon selection sounds**: record the game's audio (OBS or similar)
   with `hud_fastswitch 0`: press 2, 2 again, 4 with no grenade, the wheel
   twice, then attack; the same with `hud_fastswitch 1`.
5. **C4 autobuy and rebuy**: as described in parity-gaps.md C4, plus the
   console text each prints (copy the console or screenshot it).
6. **C11 team balance**: `mp_autoteambalance 1; mp_limitteams 0`, put four
   bots on T and yourself plus one bot on CT, play a round: who moves, the
   message, when.
7. **C15 muzzle flash**: in the lower tunnel, `host_timescale 0.1`,
   screenshots of the first two frames of an AK-47, USP, M3 and AWP shot.
8. **C23 sprays** and **C24 night vision**: as in parity-gaps.md.
9. **Empty gun with no reserve**: empty a pistol's last clip with no spare
   ammo (`give weapon_deagle`, fire everything, then drop to 0 reserve
   with `buy` disabled or after the buy time): does CS:S switch to another
   weapon by itself after the last shot, or stay on the empty gun
   clicking? The probe bot switched to its knife, but that may be its AI;
   ours stays on the empty gun.
10. **C25 votes** (needs a second human player on the listen server):
   `callvote` alone, a kick vote and a changelevel vote, F1 during a
   vote, a refused call; screenshots of each panel, as in parity-gaps.md
   C25.
11. **C26 automatic hints**: on a fresh profile with `cl_autohelp 1`
    (rename `cfg\config.cfg` first, put it back after), which hints
    appear when, as described in parity-gaps.md C26 (a video of the
    first two rounds is easiest; note each hint's time).
12. **C29 client props**: on cs_italy, knife and shoot a small cardboard
    box and a mug or bottle (count the hits each takes), with the game's
    audio recorded (any sound when a bottle breaks, beyond the bullet's?);
    on de_inferno, shoot a window (break sound or none?); then break a
    bottle and `mp_restartgame 1` (is it back?). As in parity-gaps.md
    C29.
13. **C30 refused hits**: with `mp_friendlyfire 0`, shoot and knife a
    teammate bot (audio recorded, screenshots 0.1 s after: blood, a
    flesh impact?); then `god` with kevlar and get shot by an enemy bot
    (armour on the HUD, sounds, flinch, slowdown, blood). As in
    parity-gaps.md C30.
14. **C31 HE ear-ringing**: with `mp_friendlyfire 0`, a teammate bot's
    HE at your feet (audio recorded, `dsp_player` in the console right
    after); again in `noclip`; once with `mp_friendlyfire 1`. As in
    parity-gaps.md C31.
15. **C32 planting the bomb**: a screen recording (or screenshots every
    0.25 s) of a plant on de_dust2 A, first person, from the press to the
    bomb on the ground, with `cl_c4progressbar 1` and again with 0; then a
    defuse with each; then a bot planting seen from beside it. As in
    parity-gaps.md C32.

## Sitting 2: sound (about 20 min)

Record the game's audio (OBS, 48 kHz) for:

1. **C5 under water**: in de_nuke's deep water (de_aztec's river is too
   shallow to go under), head under, a bot firing an AK-47 at about 500
   units; then the same with the head above.
2. **The planted bomb's beeps** (objectives.md Q4: the server sends none,
   the client makes them): plant on de_dust2 B (`mp_c4timer 45`), stand
   next to it and record until the explosion; the beep times give the
   schedule.
3. **Sound distance falloff** (parity-gaps "Distance falloff curves", the
   last high-impact gap): `bot_stop 1`, a bot firing an AK-47 from 250, 500,
   1000, 2000 and 3000 units in a straight open line (de_dust2 long A), you
   standing still facing it; say the distance into the mic or note the times.

## Sitting 3: refcmp references (automatic, about 30 min)

`scripts\playtest.ps1` builds the game; refcmp drives CS:S by itself
(docs/OBSERVABILITY.md, refcmp). Run the capture modes listed in C10, C17,
C18, C19, C20, C21, C22 and C27 (each names its views and settings). The output
folder is printed at the end: send it.

## Sitting 4: the other stock maps (mostly automatic, about 40 min)

Reference views exist only for de_dust2, de_aztec, cs_office and de_nuke;
the other stock maps (docs/plans/active/other-maps.md) were checked by
eye only. refcmp drives CS:S by itself, as in sitting 3:

1. `refcmp capture-ref --views tools/refcmp/<map>.toml` for de_train,
   de_inferno, de_dust, cs_assault, cs_compound, de_cbble, de_chateau,
   de_prodigy and de_tides (views from their cameras, spawns and
   bomb/rescue zones). Send the `refcmp/<map>/ref/` folders.
2. cs_italy, de_port, cs_havana and de_piranesi have no
   `point_viewcontrol` for refcmp to move, so take their views by hand:
   `sv_cheats 1; noclip; cl_drawhud 0; r_drawviewmodel 0`, then for each
   `[[view]]` in `tools/refcmp/<map>.toml` `setpos x y z-64` (the file
   holds eye positions, setpos the feet) and `setang pitch yaw 0`, wait a
   second, `jpeg refcmp_<name> 95`.
3. **cs_compound's fog colour by view direction** (its
   `env_fog_controller` blends two colours, `fogblend 1`: warm yellow
   toward yaw 108, grey away; ours draws the first only): from the CT
   spawn, `setang 0 108 0` and `setang 0 288 0`, a screenshot each of the
   far skyline. Says whether the blend shows at all at its distances
   (fog from 3000 units) and which colour lies which way.
4. **de_prodigy's vent fans** (`fan.mdl` lit from an
   `info_lighting_relative`, which ours ignores): `setpos 2384 -1511 -24;
   setang 89 0 0` (looking down at the fan from 116 units above), a
   screenshot: are the blades lit like the vent around them, or darker?

## Sitting 5: Combat Arms (a room with a friend or bots, about 40 min)

Live checks the Wave 1 specs could not settle from the reference (each
names its spec and open question in `specs/combat_arms/`). Combat Arms'
own client, any map unless named; a screen recording of each is easiest.

1. **Scale** (movement.md Q3): stand next to a standard door or crate of
   known real size; note its height against the eye (83 units) from a
   screenshot side on. Says whether bodies or the HUD's 40 units a metre
   are to scale.
2. **Teleporters** (movement.md Q16): on a map with a teleporter, use
   one that drops you a long way: any fall damage on landing?
3. **Second victim** (weapon_firing.md Q5): two players or bots standing
   in line, one M4A1 round at the first's chest from about 2800 units:
   does the second take damage (about 17 if unarmoured)?
4. **Turn and spread** (weapon_firing.md Q13): stand still with the M4A1,
   flick the mouse 30 degrees back and forth for a few seconds: does the
   crosshair ever shrink faster than standing still without turning?
   Then the same while strafing.
5. **Pickup box** (inventory.md Q13): drop an M16A3 on flat ground and
   aim at it from the side and from its end, moving the aim off it a
   little at a time: at which offsets from its centre does the pickup
   prompt appear and vanish (a screenshot at each edge)?
6. **Spent grenade in the list** (inventory.md Q14): throw your last
   grenade, then press 4 or another class key: is the grenade's entry in
   the weapon list greyed, hidden or shown with 0?
7. **Pickup switch** (inventory.md Q6): carry a pistol, hold the rifle,
   pick up a pistol of the same kind dropped by someone: does the pistol
   come out?
8. **Drop spot** (inventory.md Q17): stand still facing one way on flat
   ground and die (or swap weapons by picking one up), several times:
   where does the item lie relative to where you stood (a screenshot
   from above, `getpos` if the console allows)?
9. **Free-for-all teams** (hud_crosshair.md Q15): in a free-for-all room,
   hit another player: does the crosshair turn red? Do players you are
   not aiming at show name tags?
10. **Name tag height** (hud_crosshair.md Q10): a teammate on flat ground
    next to a wall mark of known height at the same distance: a
    screenshot to compare the tag's height with the mark.
11. **Server frame rate** (grenades.md Q6): drop a frag from a known
    height onto flat ground (look straight down, release): count the
    frames between bounces in a recording, or read the server's frame time
    from a packet capture. We use the shipped server's 20 Hz.
12. **Frag through a body** (grenades.md Q7): throw a frag at a standing
    teammate's chest from about 100 units: where does it land compared
    with the same throw past them (1 to 3 factors of 0.65 expected)? We
    count the character's box and its hit-box object's (taken as the hit
    spheres' bounds), each step inside.
13. **End-of-round phase** (grenades.md Q20): time from a round's result
    to the respawn in Elimination or Search & Destroy (the shipped
    configuration says 3 s); a frag thrown just before the result: does it
    hurt players after the respawn?
14. **Sky edge** (world_dat.md Q11): look at the sky over a low roof edge
    on WAREHOUSE and SNOWSTORM: a screenshot. We draw the sky world model
    over the whole screen behind the world; the game only inside the sky
    portals' screen rectangle.
15. **Fullbright texture in the dark** (world_dat.md Q13): find a surface
    whose texture has the fullbright flag (textures_dtx.md case 5's,
    TEXTURES\FLOOR\GB_CELLING_FB005.DTX, or any glowing sign) in a dark
    corner: do its bright parts stay at full brightness? We draw it lit.
16. **Group 5 textures** (textures_dtx.md Q4): an effect that shows one of
    `FX.rez`'s 28 group-5 textures at a known size (a screenshot close up):
    is it its file's level 0 or one level down? We load level 0.
17. **Server reload** (characters.md Q2): reload an M4A1 and fire at
    1.6 s and at 1.9 s after the key (a second client counting shots):
    both should register (the server's third-person timer, 1555 ms, ends
    before the client's 2300 ms refill). We end the server's reload pose
    on that timer.
18. **Emote legs** (characters.md Q20): in Hunted start `Insult_Dance`
    standing still and watch from a second client: do the legs dance for
    the whole animation, or do they snap back to standing? We hold both.
19. **Plant time on Steam** (game_modes.md Q4): time from pressing fire
    with the bomb on a site to the "planted" text (3.9 s expected from
    the older XK-2 files).
20. **Protected enemy puff** (game_modes.md Q25): shoot a freshly
    respawned enemy standing in front of a wall: is a surface puff drawn
    on the enemy? We draw nothing.
21. **Ties on the scoreboard** (game_modes.md Q31): screenshots of the
    scoreboard and the round result screen in each mode with two players
    tied on kills.
22. **Late joiners** (game_modes.md Q22): in an Elimination Pro room two
    rounds from the end, try to join from a second account.
23. **Bot fire with an M-class gun** (bots.md Q3): give a bot an
    M60-class weapon of `AniType` 8 (or watch one carry it): does it fire
    at all?
24. **Stuck bots** (bots.md Q11): on Short Fuse, block a narrow corridor
    with a crate-sized prop or a teammate and time how long a patrolling
    bot pushes against it before turning. We plan again and jump after
    1.5 s.
25. **Catch breath** (bots.md Q20): let a bot chase you out of sight for
    its endurance time (12 s on Easy) and time its pause.
26. **Beyond the outer radius** (effects_sounds.md Q1): stand 2100 units
    from a firing M4A1 on an open map: silent?
27. **Light through a thin wall** (effects_sounds.md Q4): a muzzle flash
    behind a thin wall at medium detail: is the wall's far face lit?
28. **Unseen footsteps** (effects_sounds.md Q8): stand behind a wall while
    an enemy runs on the other side: are its steps heard?
29. **Steam's mission values** (missions.md Q1): Ghost Town Search &
    Destroy's round timer at the start of round 1 (the Steam mission file
    reads 120 s, the older client's 180 s), and Quarantine's points
    (the Steam file: kill 2, infect 1, humans' win 5, infected's 2).
30. **A 4-unit step** (effects_sounds.md case 56): walk up a 4-unit step
    while recording at 144 fps: does the camera take about 21 ms
    (movement.md's snap within the last unit) or 29 ms?

## What each sitting unlocks

- Sitting 1: movement and weapon checks against real human play (demosim),
  the HUD panels' look and timing, autobuy/rebuy rules, team balance, muzzle
  flash, sprays and night vision values.
- Sitting 2: the sound falloff curve (the last high-impact parity gap) and
  the underwater muffle.
- Sitting 3: HDR, 3D skybox fog, ropes, beams, sky, LOD, detail props and
  what cs_assault's PVS hides past sky brushes.
- Sitting 4: side-by-side checks of the 13 stock maps never compared
  (refcmp `report`), cs_compound's fog blend, prodigy's fan lighting.
- Sitting 5: Combat Arms' scale, teleport falls, two-victim rounds, the
  spread's turn term, pickup boxes and drop spots, the spent grenade's
  list entry, free-for-all team ids and name tag height; the server's
  frame rate, grenades through bodies, the end-of-round phase, the sky's
  edge, fullbright textures and group-5 mip skipping; the Wave 3
  follow-ups' live checks (server reload, emote legs, plant time, the
  protected enemy's puff, scoreboard ties, late joiners, bots' fire,
  stuck bots and catch breath, sounds beyond R, lights and unseen steps,
  Steam's mission values, a 4-unit step).
