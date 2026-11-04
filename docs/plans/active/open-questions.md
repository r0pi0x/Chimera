# Open questions: what can answer each

Started 2026-10-17. Every open question in the specs (`specs/cs_source/*.md`,
`specs/source/*.md`, their "Open questions" sections) and every "differs" or
"missing" row of [parity-gaps.md](parity-gaps.md), sorted by what would
answer it. Questions already marked answered in their spec are left out;
partly answered ones list only what is left.

Classes:

1. **Probe server**: answerable now on the CS:S dedicated server
   (`tools/css_probe`: server state, events, damage, entities, the sounds
   the server sends, movement), or offline from the server install's files
   and the maps with our own readers.
2. **CS:S client**: needs the game's client (screenshots, audio, RenderDoc,
   client demos): the Windows sittings in [windows-captures.md](windows-captures.md).
3. **Spec session**: needs a spec session on the reference material (or a
   design decision with no game behaviour to measure).
4. **Second human**: needs a second human player (bots behave differently).
5. **Answered, not marked**: the answer was already measured elsewhere;
   the spec or row is updated in the same commit as this list.

Counts (2026-10-18, 296 rows; a row grouping several questions counts
each, a row naming two classes counts in both): class 1: 137, class 2:
173, class 3: 56, class 4: 7, class 5: 25 (24 spec questions or parts and
one parity row, fixed with this list). Part B below works through the
class-1 items by player impact.

Recount after Part B's second pass (2026-10-17, same rules; the
physics_constraints row is split in two, so class 1 counts 138): class 1:
138, of which 31 answered in Part B and 107 left (a row with a part left
counts as left); classes 2-5 unchanged.

Third pass (2026-10-18, physics props, constraints, sounds, recoil,
templates, ropes; the physics_constraints row split again, its Q10 apart):
34 class-1 items settled, 27 answered on the probe or by sweeping the
maps' entity lumps (`tools/css_probe/entsweep.py`) and 7 found to need
something else (moved to classes 2-4: a dangling rope's look, the
silencer's recoil, the template suffix and templated logic_auto, the
constraint tie order, a human's airborne inaccuracy, nav_analyze on a
listen server); about 73 class-1 items left.

Fourth pass (2026-10-18, blasts, the bomb, spectating, doors, flashes,
ladders; `tools/css_probe/blastmeas.py`, `chordmeas.py`): 10 class-1
items settled (grenades Q6 and Q10, objectives Q1 and Q3, spectating's
mode 3 and first target, doors_buttons Q6, physics_brushes Q4, fire Q6,
and entity_io Q9 moved to class 3: no map has a multi-line game_text),
5 more answered in part (grenades Q12's smoke, movement Q16's ladder
jump-off, weapons Q7's double-count edge, physics_brushes Q1's mass
reading, grenades Q17's `drop`, which met the bot AI's knife); about 63
class-1 items left. Two plugin commands that unblock sounds Q1 and the
thrower's side of grenades Q12 are written but not built
(`tools/css_probe/README.md`).

Fifth pass (2026-10-18, flashes, triggers, movers, restarts, fires,
lasers, conveyors, ragdolls, the bomb's timing;
`tools/css_probe/flashmeas.py`): 18 class-1 items settled
(fall_damage's damagemodel 1, triggers Q3 and Q8, doors_buttons Q3, Q4,
Q5 and Q7, objectives Q13, entity_io Q1 and Q10, fire Q9, visual_entities Q3, movement Q18,
physics_brushes Q6 and Q7, particles_and_smoke Q6, and fire Q4 moved to
class 5: entity_io Q1 had it), 5 more answered in part (grenades Q12 all
but the thrower's side, grenades Q17's `drop`, movement Q15's conveyor, physics_brushes Q3's force,
objectives Q14's defuse and blast timing); about 45 class-1 items left. The two plugin commands above are still not
built, so sounds Q1 and the thrower's side of grenades Q12 wait.

Sixth pass (2026-10-18, the bomb's think, penetration, hostages, the
log, thrusters; `tools/css_probe/bombtime.py`, `thrustmeas.py`,
`primedeath.py`): 6 class-1 items settled (objectives Q14, Q17,
grenades Q17, physics_brushes Q1, and Q3 moved on to the turn rate; the
bomb's two measured-but-unchanged timings now implemented), 4 more
answered in part (weapons Q7's tile and glass scales and buckshot,
objectives Q10's give-up distance, physics_brushes Q3's arm, sounds Q3
tried: the probe bot freezes in aztec's knee-deep water); the players'
shove of a solid map ragdoll was tried and reverted (it launched the
player). Then fire Q5 (61 fires on seven maps, `firedrop.py`), shaders
Q3 (4 bump pages), entity_io Q5 (what maps send) settled, sounds Q15's
OnPlay moved to class 2 (bots get no soundscape). About 35 class-1
items left. The plugin commands are still not built, so sounds Q1 and
grenades Q12's thrower wait.

Added since (2026-10-18): shaders Q21, the fog colour by view direction
(classes 3 and 2; one row in the specs table and one in the parity list);
objectives Q17, game events' fields and the log's other lines (class 1).

## Specs: specs/cs_source

| Spec | Question | Class | What would answer it |
|---|---|---|---|
| animation | ground speeds, walk/run threshold, max body yaw, playback scale | 2 | client demo of a bot walking/running (capture C14) |
| animation | reload sequence choice and timing by movement | 2 | client demo, frame-stepped |
| animation | grenade movement shoot layers; ELITES left/right rule | 2 | client demo |
| animation | jump sequence restart and air blending | 2 | client demo |
| animation | death sequences and when the ragdoll takes over | 2 | client demo |
| animation | weapon `PlayerAnimationExtension` per weapon | 3 | read the decrypted scripts in a spec session |
| animation | do the sequences' footstep events play anything | 2 | client audio while a bot walks (the server's steps follow movement, measured) |
| animation | IK rules and locks | 3 | spec session (out of scope so far) |
| animation | lag compensation's use of the layers | 3 | spec session (hitbox/lag-compensation spec) |
| fall_damage | `damagemodel 1` doubling | 1 | done (Part B, 2026-10-18: 5, 10, 20, 25, ... as ours) |
| fall_damage | armour against trigger_hurt (drown type, others) | 1 | drown type done (Part B); other types done (Part B, triggers Q4) |
| fall_damage | half-second timer per trigger or per toucher | 1 | two bots entering de_port's at different ticks (done, Part B) |
| fall_damage | `Player.FallDamage` on hurting landings only | 1 | sound hook on drops with and without damage (done, Part B) |
| fall_damage | landing view roll and its decay, HUD indicators, `HealthTookDamage` | 2 | client screenshots after a drop |
| grenades | Q2 redraw: the same grenade again or the best weapon | 1 → 2 | tried in Part B: the bot AI picks its own knife after a throw, so a human capture decides |
| grenades | Q3 switch with the pin pulled | 1 | a slot pick while attack is held (done, Part B) |
| grenades | Q6 spawn inside a wall | 1 | done (Part B, 2026-10-18) |
| grenades | Q7 bounce sounds per contact | 1 | sound hook while a grenade bounces (done, Part B) |
| grenades | Q9 HE visuals (fireball, shake, light) | 2 | client screenshots at `host_timescale 0.1` |
| grenades | Q10 crouched victims, walls and props blocking | 1 | done (Part B, 2026-10-18, `blastmeas.py`) |
| grenades | Q11 ear-ringing presets and who gets them | 2 | capture C31 (the server sends nothing, measured) |
| grenades | Q12 left: the thrower's own blindness (smoke, players, props, walls, glass and grates done, Part B 2026-10-18) | 1 | `mashup_netprop` (added to mashup_probe.sp 2026-10-18, reads `m_flFlashDuration` once the plugin is rebuilt) |
| grenades | Q13 flash overlay curve | 2 | client screenshots every 0.1 s |
| grenades | Q14 flash DSP presets | 2 | client console/audio |
| grenades | Q15 left: smoke on a moving lift, in-flight trail | 1, 2 | lift: probe (de_nuke has none moving; skip); trail: client |
| grenades | Q16 smoke against bots; target ID through smoke | 1, 2 | bots (parked); target ID: client |
| grenades | Q17 left: dying primed while moving | 1 | done (sixth pass, 2026-10-18: the runner's velocity, as ours) |
| impact_effects | Q1 CS:S's own impact callback (counts, colours, timings) | 2 | client screenshots at `host_timescale 0.1` |
| impact_effects | Q2 ricochet rate; no effect on refused teammates | 2 | client audio count; C30 |
| impact_effects | Q3 exit impact after penetration | 2 | client screenshot of a door's far side |
| impact_effects | Q4 particle throttle | 2 | client, a smoke-heavy scene |
| impact_effects | Q5 surface colour source | 2 | client screenshots of red and white walls |
| impact_effects | Q6 blood position and who makes it | 2 | client, `cl_predict` 0/1 |
| impact_effects | Q7 particle operator semantics | 2 | client recording at `host_timescale 0.05` |
| impact_effects | Q8 splash size per ammo; underwater impacts | 2 | client screenshots |
| impact_effects | Q9 surfaces without a decal | 2 | client: shoot a ladder |
| impact_effects | Q10 wood fleck fade | 2 | client at `host_timescale 0.2` |
| impact_effects | Q11 left: knife water check and client prediction (its damage type is measured: bullet) | 2 | client |
| impact_effects | Q12 `cl_new_impact_effects` | 2 | client console |
| impact_effects | Q13 effect detail setting | 2 | client video settings |
| mdl_v48 | zero-frame entries, v44 leftovers, `.ani` header | 3 | nothing reads them; spec session if ever needed |
| mdl_v48 | AWP poses against the game | 2 | view-model screenshot pair |
| movement | Q15 left: water currents (a conveyor's base velocity done, Part B 2026-10-18) | 1 | no stock map has a current; a spawned water brush with a current texture can't be made on the probe: a test map |
| movement | Q16 left: water jump 256/50 | 1 | a movecmp scenario at a water edge (the ladder jump-off's 270 done, Part B 2026-10-18) |
| movement | Q18 a box exactly touching a floor | 1 | done (Part B, 2026-10-18: start solid in CS:S, ours stands; not changed) |
| movement | Q19 displacement contact normals elsewhere | 1 | movecmp on more dust2 strip edges |
| nav | Q1 the per-area CS byte | 2 | tried 2026-10-18: `nav_analyze` does nothing on the dedicated server (no listen host) and `nav_save` wrote de_dust2's mesh back byte for byte; needs a listen server |
| nav | Q2-Q4 bot path cost, attribute handling, follower | 3 | bots are parked; a behaviour spec |
| nav | Q5 nearest-area distance | 1 | a bot placed off-mesh above a ledge (bots parked) |
| nav | Q6 stairs flags at load | 1 | `nav_edit` on dust2's stairs areas |
| nav | Q7 approach record fields (v9) | 3 | spec session |
| nav | Q8 `earliest_occupy` effect | 1 | bot round starts (bots parked) |
| objectives | Q1 left: leaving the zone by teleport while planting | 1 | done (Part B, 2026-10-18) |
| objectives | Q3 radii and walls | 1 | done (Part B, 2026-10-18) |
| objectives | Q4 beep schedule and loudness; LED place; debris sounds | 2 | the server emits no beep (measured): client audio, sitting 2 |
| objectives | Q5 money for plant, defuse, round-end reasons, hostages | 1 | `mashup_state` money around each event (done, Part B; the draw, Ts after a rescue and "led" too) |
| objectives | Q6 left: who gets the bomb with humans; defuse announcers | 4, 2 | a human on T for 50 restarts; client audio |
| objectives | Q7 left: the planted bomb on a moving platform | 1 | no stock map; skip |
| objectives | Q10 left: hostages at corners and jumps; give-up rule | 1 | give-up distance done (sixth pass: 2000 units); corners, jumps and an out-of-sight time: `mashup_htrack` with longer leads |
| objectives | Q12 HUD and radar | 2 | client screenshots |
| objectives | Q13 `info_bomb_target` radius | 1 | done (Part B, 2026-10-18: no such class in CS:S) |
| objectives | Q14 left: same-tick defuse and blast; the kit's 5 s (the plant's 200 ticks, the defuse's and the blast's 0.11 s late done, Part B 2026-10-18) | 1 | done (sixth pass: the bomb's think every 8 ticks; the kit's 5 s; a shared think blows) |
| objectives | Q16 the bar while planting | 2 | client screenshots |
| objectives | Q17 events' fields and the log's other lines (kills, chat, buys, connects) | 1 | done (sixth pass: the lines from a round's log, the fields from the install's event files); left: `site`'s value, `(headshot)` |
| overlays_decals | Q1-Q9 (decal orientation, reach, size, displacements/props, overlay order, overlays vs decals, corner mapping, deathmatch flag, fade) | 2 | client screenshots, one each (9 items) |
| physics_props | Q1 inertia tensor | 5 | measured (Part B, 2026-10-18) |
| physics_props | Q2 rotational inertia limit | 3 | inside vphysics |
| physics_props | Q3 damping law | 5 | measured (Part B, 2026-10-18) |
| physics_props | Q4 friction and elasticity combination | 5 | measured (Part B, 2026-10-18) |
| physics_props | Q5 sleep thresholds | 5 | measured (Part B, 2026-10-18) |
| physics_props | Q6 air drag and density | 5 | measured (Part B, 2026-10-17 and 2026-10-18) |
| physics_props | Q7 player push limits | 5, 3 | mass limit measured (Part B, 2026-10-18); the 50 in/s speed limit's meaning: inside vphysics |
| physics_props | Q8 hull shrink | 5 | measured (Part B, 2026-10-18) |
| physics_props | Q9 static prop solid types 2 and 6 | 5 | measured (Part B, 2026-10-18) |
| physics_props | Q10 SOLID_VPHYSICS without `.phy` | 5 | measured (Part B, 2026-10-18) |
| physics_props | Q12 player push call sites (< 75 vs > 75 in/s) | 5, 4 | the server push's rate measured (Part B, 2026-10-18); slower than 75: a human (a bot can't start moving under 75) |
| physics_props | Q13 `sv_turbophysics` default | 1 | `cvarlist` (done, Part B: 0) |
| physics_props | Q14 drag axis areas | 3 | units unknown; spec session |
| physics_props | Q15 model type 1 solids | 3 | none shipped |
| ragdolls | Q1 client-side ragdolls, per-client results | 4 | two clients watching one kill |
| ragdolls | Q2 left: does a player's death push use the ammo impulse | 2 | capture C13 |
| ragdolls | Q3 death pose choice | 2 | client demo |
| ragdolls | Q4 bone_dt | 2 | client demo |
| ragdolls | Q5 joint limit convention | 3 | inside vphysics |
| ragdolls | Q6 `cl_ragdoll_physics_enable` | 2 | client |
| ragdolls | Q7 collision with server props and movers | 2 | client |
| ragdolls | Q8 water | 2 | client |
| ragdolls | Q9 removal at restart and disconnect | 2 | client |
| ragdolls | Q10 bone derivative helper | 3 | spec session |
| ragdolls | Q11 physics position interpolation | 3 | spec session |
| ragdolls | Q12 damping, sleep, inertia, friction | 1 | as physics_props Q1-Q5 |
| ropes | strip builder (side vector, U edge, alpha, culling) | 2 | client screenshot edge-on |
| ropes | width = full width | 2 | client screenshot |
| ropes | engine point lighting | 2 | client screenshot |
| ropes | Hammer defaults in the FGD | 3 | the FGD |
| ropes | `_back` rope materials; `cable.vmt` blend | 5 | listed (Part B, 2026-10-18) |
| ropes | TextureScale network rounding | 3 | spec session |
| ropes | dangling rope's free end | 2 | three found on cs_compound (Part B); where the free end starts: client screenshot |
| ropes | CS:S-specific rope code; the 10-node cable's 11 px | 2 | client screenshots |
| shaders | Q1 lightmap scale constant (test map) | 2 | client test map |
| shaders | Q2 left: bump redistribution with coloured light; page borders | 2 | RenderDoc |
| shaders | Q3 linear light and envmap scale | 2 | RenderDoc |
| shaders | Q4 alpha-test reference (0.5 consistent) | 2 | RenderDoc of a ramp texture |
| shaders | Q5 brush tangents | 3 | engine code |
| shaders | Q6 `mat_fullbright` | 2 | client screenshot |
| shaders | Q7 model modulation parameter | 3 | engine code |
| shaders | Q8 translucent/additive blend states | 3 | engine code |
| shaders | Q9 decals on bump-lit brushes | 2 | client screenshot pair |
| shaders | Q10 bump weight sum 0 | 2 | client |
| shaders | Q11 vertex-lit inputs | 2 | capture C28, RenderDoc |
| shaders | Q12 pixel-fog depth | 3 | engine code |
| shaders | Q13 revision skew | 3 | spec session |
| shaders_phong | Q1 missing parameters (defined-and-zero or the stated defaults; implemented with the defaults) | 2 | a test VMT with `$rimlight 1` and no `$rimlightboost` under a bright ambient, against `$rimlightboost 1` |
| shaders_phong | Q2 directional lights packed as far points; Q3 which 4 lights and their scale | 2 | RenderDoc capture of a model outdoors (and shaders Q11) |
| shaders_phong | Q4 flat normal's alpha; Q5 `$bumptransform`; Q6 lightwarp's row | 3 | shader/engine source (no stock content uses them) |
| shaders_phong | Q7 flashlight pass on phong models | 3 | spec session on the flashlight include |
| shaders_phong | Q8 revision skew | 3 | spec session |
| shaders_phong | Q9 measurement: Combine soldier and SMG1 view model | 2 | refcmp of HL2:DM's wash room on dm_lockdown |
| shaders | Q15 WindowImposter, ShatteredGlass, LightmappedReflective | 3 | shader source |
| shaders | Q16 texture transforms | 3 | material-system source |
| shaders | Q17 `$emissiveblend*` | 3 | shader source |
| shaders | Q18 `$selfillum` details | 3 | shader source |
| shaders | Q19 detail modes 10, 11 | 3 | shader source |
| shaders | Q20 displacement texture coordinates (measured) | 3 | confirm from the compiler/engine source |
| shaders | Q21 fog colour by view direction (`fogblend`, `fogcolor2`, `fogdir`/`use_angles`): weight curve, colour space, direction source, per view or pixel, 3D skybox | 3, 2 | the client's fog set-up in a spec session; windows-captures sitting 4 item 3 shows which colour lies which way on cs_compound |
| shaders_two_texture_blend | Q1, Q2 the ×2 factor, sampler sRGB | 2 | RenderDoc of a de_aztec wall |
| shaders_two_texture_blend | Q3 bump pages on de_aztec | 1 | done (sixth pass: 4 pages, SURF_BUMPLIGHT, as our loader) |
| shaders_two_texture_blend | Q4 mode 0 with detail and bump | 3 | unused by stock content |
| shaders_two_texture_blend | Q5 refcmp after implementing | 2 | refcmp views |
| shaders_two_texture_blend | Q6 2-vector `$detailscale` | 3 | material parser source |
| shadows_sky | Q1 shadow clipper; Q2 static props as casters; Q3 player shadows; Q5 2D sky order; Q6 shadow detail; Q7 blend space | 2 | RenderDoc/screenshots (6 items) |
| shadows_sky | Q4 the no-shadow_control ambient colour | 3 | engine code |
| sounds | Q1 distance falloff, pan law, occlusion | 1, 2 | the falloff: `mashup_gain` (added to mashup_probe.sp 2026-10-18, through SourceMod's `GetDistGainFromSoundLevel`), once the plugin is rebuilt; pan law and occlusion: client recordings |
| sounds | Q2 left: other entries' wave picks | 5 | measured (Part B, 2026-10-18) |
| sounds | Q3 left: wading in knee-deep water | 1 | feet-deep measured (Part B, 2026-10-18); knee-deep: aztec has a 15-16-deep strip, but the probe bot freezes in it (sixth pass); another map or spot |
| sounds | Q5 `Player.Swim` doubled for the owner | 2 | client trace |
| sounds | Q6 gravel surface override | 5 | measured (Part B, 2026-10-18) |
| sounds | Q7 left: shot sound path, ricochets, whizz | 2 | client trace |
| sounds | Q8 scrape energy | 5, 3 | thresholds and volumes measured (Part B, 2026-10-18); the energy formula: inside vphysics |
| sounds | Q9 weapon sound timing (reload parts, empty click, zoom, deploy) | 5, 2 | server half measured (Part B, 2026-10-18); the client-side sounds' timing: client trace |
| sounds | Q10 ambient_generic presets and Hammer defaults | 5 | swept (Part B, 2026-10-18) |
| sounds | Q11 ambient_generic level for a radius | 2 | client audibility test |
| sounds | Q12 channel stealing and voice limits | 2 | engine |
| sounds | Q13 remote footstep cadence | 2 | client |
| sounds | Q14 `dsp_player`, `soundmixer` | 2 | client |
| sounds | Q15 soundscape proxy radius; triggerable OnPlay | 2 | OnPlay: a bot standing in a spawned env_soundscape (radius 128) fired no OnPlay (sixth pass; its output works: soundscapes are for human clients); what plays: client |
| spectating | freeze cam timing for humans | 4 | a human killed by a bot (client demo) |
| spectating | a human without the freeze cam waiting 6.5 s | 4 | a human, `cl_disablefreezecam 1` |
| spectating | mode 3 after the last teammate died | 1 | done (Part B, 2026-10-18) |
| spectating | first target: lowest index or next after own | 1 | done (Part B, 2026-10-18) |
| spectating | 6.5 s with the freeze cam off server-wide; bots' mode 4 vs a human's last mode | 4 | a human client |
| sprites_dust | Q1 scale rounding; Q2 undefined origin; Q3 vertex colours and `$color`; Q5 pixel diameter; Q7 Linux random range; Q8 proxy state; Q10 additive fog; Q11 global blend; Q12 HDR | 2 | client screenshots (9 items) |
| sprites_dust | Q6 network bit masking; Q9 dust brush contents | 3 | engine code |
| tracers | Q1-Q5 (frequency, own path, look, whizz, dormant shooters) | 2 | client recordings (5 items) |
| view_models | Q1-Q7 (bob, bob state, sway, mirroring, muzzle flash, brass, lighting point) | 2 | client screenshots, capture C15 (7 items) |
| view_models | Q8 sequence cross-fades | 3 | animation spec |
| water | Q1-Q12 (expensive water, reflect texture, target size, fog range, cheap pass, time base, fog material, patched proxies, `srgb?` fog colour, stale refraction, tangents, which volume) | 2 | client screenshots and RenderDoc (12 items) |
| weapons | Q1 random generator | 3 | the generator body (spec session) |
| weapons | Q2 left: the airborne inaccuracy target | 4 | the bot's jumps duck it (Part B, 2026-10-18): a human's plain jump |
| weapons | Q3 left: silenced modes | 4 | scoped and burst measured (Part B, 2026-10-18); the bot's AI takes the silencer off: a human |
| weapons | Q6 hitbox reach outside the hull | 1 | M12 shots past the hull (done, Part B) |
| weapons | Q7 left: penetration budget, player cost, other materials and ammo distances | 1 | sand's, metal's, tile's, computer's, grate's, plastic's and glass's damage factors measured (Part B, 2026-10-18); tile's (0.65) and glass's (1.0) scales and buckshot (nothing) done (sixth pass); grate's and plastic's scales; the budget |
| weapons | Q11 left: shotgun reload (M9) | 1 | clip per tick during M3/XM1014 reloads (done, Part B) |
| weapons | Q12 left: pellet pattern, line or box traces | 3 | the shared offset measured (Part B, 2026-10-18); the pattern inside the spread is the generator's (Q1) |
| weapons | Q13 left: which weapon drops at death, dropped weapons' lifetime | 1 | death drops done (Part B); lifetime done (Part B) |

## Specs: specs/source

| Spec | Question | Class | What would answer it |
|---|---|---|---|
| breakables | Q6 gib gravity | 2 | client |
| breakables | Q7 de_nuke vent identification | 1 | our screenshot at those origins (no CS:S needed) |
| breakables | Q8 glass pane render details | 3 | client glass code |
| breakables | Q9 gib bounce sounds | 2 | client trace (gibs are client-side) |
| doors_buttons | Q1 CS:S use search distances | 1 | a spawned func_button, bot +use at 79/81 (done, Part B) |
| doors_buttons | Q2 use deny sound | 2 | client audio |
| doors_buttons | Q3 use trace through grates and windows | 1 | done (Part B, 2026-10-18: both stop it) |
| doors_buttons | Q4 ride timing on elevators | 1 | done (Part B, 2026-10-18) |
| doors_buttons | Q5 func_tracktrain OnNextPoint per tick | 1 | done (Part B, 2026-10-18: every tick) |
| doors_buttons | Q6 door move sound timing | 1 | done (Part B, 2026-10-18) |
| doors_buttons | Q7 rotating door on its plane | 1 | done (Part B, 2026-10-18: opens to +distance, as ours) |
| doors_buttons | Q9 touch-open door under a player | 1 | spawned func_door flag 1024 (done, Part B) |
| entity_io | Q1 left: classes absent from stock maps | 1 | done (Part B, 2026-10-18: none kept) |
| entity_io | Q3 delay at an exact tick multiple | 1 | relays with 0.015/0.03/0.3 delays (done, Part B) |
| entity_io | Q4 Kill then input the same tick | 1 | a relay pair (done, Part B) |
| entity_io | Q5 `sv_allow_point_servercommand` | 1 | `cvarlist sv_allow` (done, Part B: "always"); what maps send done (sixth pass: `say`, `sm_`/`ma_`, sv_/mp_/phys_ settings; ours runs them all but `rcon`) |
| entity_io | Q6 `cl_restrict_server_commands` | 2 | client console |
| entity_io | Q7, Q8 game_text scan-out and channels | 2 | client screenshots |
| entity_io | Q9 game_text newline | 1 → 3 | swept 2026-10-18: no map has a multi-line one (3 game_texts in 89 community maps); a test map |
| entity_io | Q10 tracktrain and I/O `Use` | 1 | done (Part B, 2026-10-18: ignored) |
| fire | Q1 client fire visuals | 2 | client |
| fire | Q2 burn damage to players (armour, score) | 1 | armour and score done (Part B) |
| fire | Q3 player ignition | 1 | `Ignite` input on a bot (done, Part B; the gas can left) |
| fire | Q4 fires at a round restart | 5 | env_fire is re-created (entity_io Q1, 2026-10-15); marked 2026-10-18 |
| fire | Q5 floors on de_dust and de_train | 1 | done (sixth pass: 61 fires on seven maps; displacements and static props stop the drop, inside a static prop it stays) |
| fire | Q6 burning gas can's radius damage | 1 | done (Part B, 2026-10-18) |
| fire | Q7 burning-prop light | 2 | client |
| fire | Q9 buried fires never damage | 1 | done (Part B, 2026-10-18; de_chateau not tried) |
| fire | Q10 particle operators, colour fade | 2 | client |
| fire | Q11 fire sound level | 2 | client |
| game_entities | Q1 lagged movement (player_speedmod) | 1 | spawned player_speedmod, movement probe (done, Part B) |
| game_entities | Q2 speedmod without an activator | 1 | spawned (done, Part B) |
| game_entities | Q3 disabled buttons after respawn (HUD: client) | 1, 2 | spawned speedmod (done, Part B); how the client draws the HUD: client |
| game_entities | Q4 game_ui "on train" flag | 1 | spawned game_ui (done, Part B) |
| game_entities | Q5 key-hint panel | 2 | client |
| game_entities | Q6 env_fade modulate | 2 | client |
| game_entities | Q7 left: env_explosion against armour and friendly fire (the falloff is measured) | 1 | armour done (Part B); friendly fire: an env_explosion has no team (moot) |
| game_entities | Q8 game_score and team score | 1 | spawned game_score, team score netprop (done, Part B) |
| game_entities | Q9 conveyor and physics props | 1 | spawned func_conveyor (done, Part B) |
| game_entities | Q10 fades at round restart | 2 | client |
| particles_and_smoke | Q1, Q2, Q3, Q5, Q7, Q9, Q10, Q11 (filters, manifests, redefinition, kill, light, skybox, emittime 0, quantisation) | 2 | client test maps (8 items) |
| particles_and_smoke | Q4 operator semantics | 3 | its own spec |
| particles_and_smoke | Q6 unknown inputs logged | 1 | done (Part B, 2026-10-18) |
| particles_and_smoke | Q8 env_smokestack Hammer defaults | 3 | the FGD |
| physics_brushes | Q1 physbox mass shrunk or not | 1, 3 | done (sixth pass: the game uses the compiler's stored mass, 72.46 in lump 29, as our loader) |
| physics_brushes | Q2 inertia and damping laws | 1 | as physics_props Q1, Q3 |
| physics_brushes | Q3 left: an offset thruster's turn (a = F/m done, Part B 2026-10-18) | 1 | the arm from the centre of mass done (sixth pass); the rate: needs angles (`mashup_netprop` or a plugin angle read) |
| physics_brushes | Q4 +use pickup of a physbox | 1 | done for props (Part B, 2026-10-18: no pickup) |
| physics_brushes | Q5 physbox_multiplayer on the client | 2 | client |
| physics_brushes | Q6 players and a map prop_ragdoll | 1 | done (Part B, 2026-10-18) |
| physics_brushes | Q7 prop_ragdoll without a ragdoll | 1 | done (Part B, 2026-10-18: it crashes the server) |
| physics_brushes | Q8 thin-rod check on rotated brushes | 1 | spawned |
| physics_constraints | Q1, Q5 (break limit at rest, deleted body; 2 items) | 1 | done (Part B; Q1's moving loads and torque left) |
| physics_constraints | Q1 left: torque at rest | 5 | measured (Part B, 2026-10-18); moving loads still class 1 |
| physics_constraints | Q2-Q4, Q6-Q9 (motor units, hinge friction, spring law, world fallback, equal limits, pulley, never-created joint; 7 items) | 5 | measured (Part B, 2026-10-18) |
| physics_constraints | Q10 tie order | 3 | map load order: a test map |
| prop_damage | Q4, Q7, Q8 and the client halves of Q2, Q9 | 2 | capture C29 |
| prop_damage | Q6 left: why the landing crush only at 1450-1500 in/s | 1 | more landing speeds |
| triggers | Q1 face contact | 1 | spawned trigger, bot placed at −16.0/−15.9 (done, Part B) |
| triggers | Q2 swept boxes for carried players | 1 | spawned fast func_door through a thin trigger |
| triggers | Q3 touch order of overlapping triggers | 1 | done (Part B, 2026-10-18) |
| triggers | Q4 trigger_hurt armour, fractions, knockback | 1 | spawned trigger_hurt, armoured bot (done, Part B) |
| triggers | Q5 gravity reset on respawn | 1 | spawned trigger_gravity (done, Part B) |
| triggers | Q6 teleport flags 32 and 4096 | 1 | spawned trigger_teleport with a bot (done, Part B) |
| triggers | Q7 trigger_push base velocity | 1 | spawned trigger_push, movement probe (done, Part B) |
| triggers | Q8 first hurt pass timing | 1 | done (Part B, 2026-10-18: 4 ticks after the touch) |
| triggers | Q9 AddOutput basevelocity | 1 | spawned trigger_multiple with that output (done, Part B) |
| viewcontrol_and_templates | Q1 HUD and view model under a camera | 2 | client |
| viewcontrol_and_templates | Q2 view reset with two players | 4 | two clients |
| viewcontrol_and_templates | Q3 camera path | 5 | measured (Part B, 2026-10-18) |
| viewcontrol_and_templates | Q4 ForceSpawnAtEntityOrigin | 5 | measured (Part B, 2026-10-18) |
| viewcontrol_and_templates | Q5 output separator in community maps | 5 | swept (Part B, 2026-10-18) |
| viewcontrol_and_templates | Q6 instance counter start | 3 | every stock template keeps names (flag 2): a test map |
| viewcontrol_and_templates | Q7 templated logic_auto | 3 | templates can't be spawned at run time and no stock one holds a logic_auto: a test map |
| visual_entities | Q1, Q2, Q4-Q10 (beam width, dlight falloff, overrides, pixel visibility, trail repeat, steam roll, spark wrap, spotlight parent, scroll quirk) | 2 | client screenshots (9 items) |
| visual_entities | Q3 env_laser damage against armour | 1 | done (Part B, 2026-10-18) |

## parity-gaps.md rows marked differs or missing

| Row | Class | What would answer it |
|---|---|---|
| Surf ramps (rampbug patched out) | 3 | our choice |
| Base velocity (conveyors, currents) | 1 | movement Q15 |
| Camera under a low ceiling | 3 | other work (mg_jacks) |
| Tick rate 66.67 vs community 100 | 3 | a choice (CS:S servers always run 0.015 s) |
| Spread generator | 3 | weapons Q1 |
| Other automatics' recoil, scoped/silenced/burst | 1 | weapons Q3 |
| Penetration materials | 1 | weapons Q7 |
| Unmeasured gun values | 1 | M13 for long reaches (buckshot done, sixth pass) |
| Grenade redraw, pin rules | 1, 2 | Q3 done (Part B, as ours); Q2 needs a human |
| `cl_autowepswitch` while firing | 2 | a human client's userinfo |
| Knife idle timing | 2 | client view model |
| Flashlight | 1, 2 | `mp_flashlight` default is measured (0, C9); the look: client |
| Distance falloff curves | 1, 2 | sounds Q1 |
| Room DSP presets | 2 | client |
| `dsp_player`, `soundmixer` | 2 | sounds Q14 |
| Wave choice | 1 | sounds Q2 |
| ambient_generic levels | 2 | sounds Q11 |
| Scrapes | 1 | sounds Q8 |
| Drowning sounds and reset | 2 | client (the server plays none, measured) |
| Spray, night vision, flashlight sounds | 2 | C24 |
| Radar look | 2 | client |
| Scoreboard spacing, dead rows | 2 | client |
| Hint panel messages | 2 | C26 |
| Console look | 3 | a choice |
| `con_enable`, `volume` defaults | 3 | a choice |
| Options apply at once | 3 | a choice |
| `impulse 100` | 1, 2 | as the flashlight |
| `impulse 201`, Import Spray | 2 | client dialog |
| `record`, `stop`, `demoui` | 3 | ours to build |
| `commandmenu`, `showbriefing`, `cheer` | 2 | client |
| `cl_minmodels` | 3 | ours to build |
| Stats, achievements dialogs | 3 | ours to build |
| Impact effects beyond the shooter's PAS | 3 | network relevance (ours) |
| Knife slash mark | 2 | client screenshot |
| HDR | 2 | C17 |
| Player shadows | 2 | RenderDoc |
| World reflections | 3 | per-vertex tangents (shaders Q5) |
| Water refraction | 2 | water spec questions |
| Particles blend in linear space | 2 | client |
| Glow occlusion by rays | 3 | engine |
| LOD switch metric | 2 | C21 |
| Detail props layout | 2 | C22 |
| Player animation | 2 | C14 |
| Ragdoll push per weapon | 2 | C13 |
| Muzzle flash, view-model lighting | 2 | C15 |
| Prop light probes | 2, 3 | C28; the weighting is engine code |
| 3D skybox haze, sky down face | 2 | C10, C20 |
| 3D skybox fog distances | 2 | C10 |
| Fog colour by view direction (`fogblend`) | 3, 2 | shaders Q21 |
| Blood decals on characters | 3 | a spec, then a client capture |
| Motion blur option | 3 | a spec |
| Breakables' damage types, broken-window drawing | 3 | breakables Q8 |
| Pushers | 1 | triggers Q2 (doors_buttons Q4 done, Part B 2026-10-18) |
| Physics shadow controller | 1 | physics_props Q7, Q12 |
| Prop damage left-outs | 1, 2 | prop_damage Q6; C29 |
| Steam heat-wave jets | 3 | no stock map; a spec |
| Community-map entity readings | 1 | the triggers and entity_io questions |
| Water and physics (buoyancy) | 1 | props floating in de_nuke's pool |
| LOD and fading of props | 2 | client |
| Model doors beyond the spec | 1 | doors_buttons questions |
| Bomb and hostage details | 1 | objectives Q5, Q10, Q14 |
| Draw rules on one-sided maps | 5 | measured (objectives.md Q15: a draw at once, as ours) |
| Movers mispredicted | 3 | ours (network) |
| `cl_cmdrate` | 3 | a choice |
| Demos | 3 | ours to build |
| Votes | 4 | C25 |

## Answered but not marked (class 5, fixed with this list)

- grenades.md Q1, Q4, Q5, Q8, Q10 (measured in "CS:S values"; the open
  questions now say so), and the measured parts of Q12, Q15, Q17.
- objectives.md Q3 (the blast, measured 2026-10-15) and Q4's server half
  (the server emits no beep).
- weapons.md Q12 (one `bullet_impact` per pellet, M16) and Q13 (the drop,
  pickup delay and reach, M20) in part.
- sounds.md Q3 and Q4 (the footstep bands and the jump step, "Footsteps").
- physics_props.md Q11 (ammo impulses, weapons.md Q8).
- movement.md Q6 (the air-duck shift: 8.5 units, item 5), Q9 (no delay),
  Q13 and Q14 (ladder climbing matches tick-exact in the movecmp fuzz),
  Q16 in part (the same fuzz), Q17 (CS:S servers ignore `-tickrate`:
  always 0.015 s ticks).
- entity_io.md Q2 (OnNewGame fires at a dedicated server's map load:
  measured with Q1).
- fire.md Q8 (CS:S's tick is always 0.015 s).
- sprites_dust.md Q4 (de_dust2's glows render in game: the spec says so).
- parity-gaps.md: "Draw rules on one-sided maps" (objectives.md Q15).

## Part B: class-1 items measured (2026-10-17)

In order of player impact. Method and numbers are in each spec, marked
measured.

| Question | CS:S answer | Ours before | Change |
|---|---|---|---|
| objectives Q5: when the round's bonuses are paid | as the next round starts (5 s after the end, a tick before the respawn) | at the round's end | paid at the next round's start (`RoundState::payouts`) |
| objectives Q5: planter's and defuser's reward | none | planter +300 | none |
| objectives Q5: bomb defused | CTs +3250 | +3500 | 3250 (`win_defused`) |
| objectives Q5: time-out losers still alive | nothing (Ts on Target_Saved, CTs on Hostages_Not_Rescued; the dead get the loss bonus) | the loss bonus | nothing |
| objectives Q5: hostage map wins | elimination 3000, all rescued 2500, not rescued 3250; plus 150 a living hostage, and for CT winners 100 a led one and 850 a rescued one | 3250 / 3500 / 3250 flat | as CS:S (`hostage_*_bonus`) |
| objectives Q5: hostage touch and rescue | +150 to the first leader of each hostage, +1000 to the rescuer | the same | none |
| objectives Q5: hurting and killing a hostage | −20 a whole point; the killing hit −40 a point −500 | −1500 a kill, nothing for a hurt | as CS:S |
| grenades Q3: switching with the pin pulled | allowed at once, nothing thrown, grenade kept | the same | none |
| grenades Q17 / weapons Q13: what the dead drop | the best gun whatever was held, and one grenade: HE, else flashbang, else smoke | the weapon in hand (or the lowest slot) | as CS:S (`drop::DeathDrop`, data `death_drop`) |
| weapons M9: shotgun reload | 0.5 s start, 0.5 s a shell (68 ticks to the first, 35 each after), a shot stops it | 0.45 s a shell (64, then 31) | 0.5 s a shell |
| fall_damage Q2: armour against de_port's trigger_hurt | takes nothing (health 25 a bite, armour 100 kept) | the same | none |
| fall_damage Q3: trigger_hurt timer | one per trigger; a newcomer waits for its next bite | the same | none |
| fall_damage Q4: `Player.FallDamage` | on every landing that takes health (with a step at 1.0), none otherwise (a step at 0.85) | the same | none |
| entity_io Q5: `sv_allow_point_servercommand` | exists, default "always"; a spawned one's `say` ran | our allowlist (a security choice) | none |
| physics_props Q13, Q6: `sv_turbophysics`, `air_density` | 0; 2.00 | not read | none |
| grenades Q7: bounce sounds | one a contact (the smoke's twice on the tick), the stopping contact too, none at rest | one a contact | none |
| grenades Q2: redraw after a throw | inconclusive: the bot ends up with its knife, but its AI picks the knife on its own | redraw of the same grenade | none (needs a human) |
| objectives Q5: a draw's pay | nothing to anyone (a lone CT's draws) | the loss bonus to both sides | nothing; the streaks left as they were |
| objectives Q5: an elimination on a map without objectives | 3000 (de_dust2 with its bomb targets removed), as a hostage map's | 3250 | 3000 (`win_elimination`, any map without bomb targets) |
| objectives Q5: time running out on a map without objectives | nothing: the round goes on until a side is dead | the defenders win (a draw without them) | nothing (`chimera_untimed_win 1` keeps ours) |
| objectives Q5: Ts winning on time after a rescue | 3250 + 150 a hostage left; nothing for the rescued one | the same | none (test) |
| objectives Q5: "led" for the 100 | led at some time this round (a hostage led and let go still pays) | the same | none (test) |
| objectives 7: both teams' last players dying together | not a draw: deaths count one at a time, the side whose last player dies first loses (`sm_slay @all`: Ts won) | a draw on the same tick | none (noted in the spec; one tick's order decides in CS:S) |
| weapons Q6 / M12: hitboxes outside the hull | hit (arms and feet up to 17 units out, 4 past the hull); the hull alone stops nothing | the same | none (test) |
| weapons Q13: dropped weapons' lifetime | kept until the round restarts, no count limit (203 at once) | the oldest went past 32 | kept (a 1024 safety net) |
| triggers Q1: a box flush with a trigger's face | touches (0.01 off doesn't) | no touch | touches (`world::box_touches`) |
| triggers Q4: trigger_hurt against armour | armour takes its part of every type but fall and drown (ratio 1: health half, armour half the rest, truncated) | none taken | as CS:S (`core::MapDamage`, `weapon::map_damage_armor`) |
| triggers Q4: trigger_hurt fractions | whole points a pass, nothing carried (2.5 takes 2 each time) | the fraction kept in health | whole points |
| triggers Q4: trigger_hurt knockback | none (nodmgforce 0 or 1) | none | none |
| triggers Q5: trigger_gravity after a respawn | kept (also after `mp_restartgame`) | kept | none (test) |
| triggers Q6: flags 32 and 4096 | 32: no player; 4096: no bots | 32 as CS:S; 4096 ignored | 4096 leaves bots out |
| triggers Q7: trigger_push | +4.5 a tick at 300, one more tick out, then 302.25 less friction | the same | none (test tightened to the tick) |
| triggers Q9: `AddOutput basevelocity` | the next move turns it into velocity × 1.0075, as our reading | the same | none |
| fall_damage: trigger_hurt drown damage given back | no | no | none |
| game_entities Q1: player_speedmod's lagged movement | scales the movement clock (7.5 units a tick at 250 with 2; velocity unscaled) | the same | none (test) |
| game_entities Q2: speedmod with no activator | nobody | nobody | none (test) |
| game_entities Q3: a respawn after a speedmod | clock back to 1, HUD back, taken buttons stay | clock back, HUD stayed hidden, buttons stay | the HUD comes back |
| game_entities Q4: game_ui without "Freeze Player" | the player walks | the same | none (test) |
| game_entities Q7: env_explosion against armour | armour ratio 1 (health half, armour half the rest) | the HE's ratio (1.475) | ratio 1 for map explosions |
| game_entities Q8: game_score team points | added to the team's score, kept over rounds | added to rounds won, kept | none (test) |
| game_entities Q9: a conveyor under a physics prop | the prop stays | the same | none |
| entity_io Q3: a delay of exactly n ticks | n ticks (0.015, 0.03, 0.3: 1, 2, 20; 0.01: 1) | the same | none (test) |
| entity_io Q4: Kill then an input in one tick | the input is lost; Trigger then Kill fires | the input still delivered | a killed entity refuses inputs |
| fire Q2: burns against armour | armour takes its part (ratio 1) | none taken | as CS:S (`core::MapDamage`) |
| fire Q2: a death by fire (also a fall, `kill`) | "suicide with world": frags −1 | no change to kills | −1 kill |
| fire Q3: `Ignite` on a player | accepted: the bot burned about 5 a second (1 every 13 ticks) until dead; `Extinguish` refused | an entity flame on the player | none (rate unchecked) |
| physics_constraints Q1: a ball socket's force limit under a hanging body | breaks below about 13.8 lb a kg (forcelimit / 2.2 ≈ 6.3 × the mass) | below 2.2 lb a kg (1 × the mass) | 1/6.3 of the weight limit |
| physics_constraints Q5: the body of a constraint broken | the constraint's OnBreak, the tick after | the joint dropped silently | OnBreak, the constraint goes |
| doors_buttons Q9: a touch-open door under a landing player | opens every time | the same | none (test) |
| doors_buttons Q1: +use reach | the shared search; a button face square ahead is used out to 87.0 units (the 10° box probes, reach counted at the box's centre) | the box probes' reach counted at the button's nearest point: 80 | counted at the box's centre (logic and the bomb/hostage search) |
| physics_props Q1: inertia | the `.phy`'s stored inertia per kg × mass (`physics_debug_entity`: crate 0.56, 0.33, 0.62; 70-80% of the solid's) | avian's, from the hull as a solid | the stored inertia (`prop_physics::StoredInertia`) |
| physics_props Q4: friction | the two surfaces' product (crate on concrete: 0.086 g with ice's 0.1 up to 0.61 g with 0.8) | averaged with the world's 0.5 | multiplied; the world 0.8 (`WORLD_FRICTION`) |
| physics_props Q4: rebound | 0.13-0.15 of the landing speed for elasticities to 0.3 on concrete, 0.28 at 0.99, 0.34 at 2.0 | averaged elasticity (0.05) | `rebound`, a fit |
| physics_props Q6: air drag | ½ · 2.0 · face area / mass × v² per axis (a 30 kg crate at 1000 units/s: about 950 units/s²) | none | as CS:S (`AirDrag`) |
| physics_props Q3: damping | speed falls about d a second (`v·(1 − d·dt)` fits) | avian's `v/(1 + d·dt)` | none (props ship 0) |
| physics_props Q5: sleep | 0.4-0.55 s after the speed falls under about 1 unit/s | 1 s still | 0.5 s (`SETTLE_TIME`) |
| physics_props Q7: push limit | 340 kg pushed (11 units/s), 360 not | 350 | none |
| physics_props Q8: rest height | 0.41-0.45 units above the floor (0.38 on another prop) | touching | none: a 0.42-unit collision margin (avian `CollisionMargin`) was tried and reverted: placed props on de_dust2 and stacked ones on de_port moved and fell |
| physics_props Q10, Q9: prop_dynamic solids | no `.phy`: traces pass; solid 2: its box, turned with it | the visible mesh; solid 2 as 6 | as CS:S |
| physics_props Q9: static props' solid 2 | the box along the world's axes around the turned bounds (de_dust's windows at yaw 120) | the bounds turned with the prop | the world box (`PropSolid::WorldBox`) |
| physics_props Q12: the players' push | a 1000 kg·units/s kick every 7 ticks (0.105 s) | every tick | every 0.1 s (`pushaway::PUSH_INTERVAL`) |
| physics_constraints Q1: torque limit | breaks nothing at rest (a weld 40 units off a 30 kg crate held at torquelimit 1); a weld's force limit as a ball socket's | the torque as a weight × units | no torque breaking |
| weapons Q7: sand's damage factor | 0.30 (AWP, AK-47, M4A1 through 11.4 and 14.4 units) | 0.5 (a guess) | 0.3 |
| weapons Q7: other materials' damage | tile 0.30, computer 0.45, grate 0.99, plastic 0.50, glass 0.50 | 0.5 for all | as CS:S |
| weapons Q12: pellets' offset | one inaccuracy offset a shot, each pellet within the 0.04 spread of it (M3 in the air: centre up to 0.28 off, pellets ≤ 0.037 from it) | the same | none |
| weapons Q7: computer's thickness scale | 0.377-0.44 (M4A1 passes a 13.2-unit TV, Deagle doesn't) | 1.0 | 0.4 |
| weapons Q7: metal's damage factor | 0.30 through an oil drum's 13.6-22.4-unit chords; 0.09 through 25-28 (the entity counted twice) | 0.5 (a guess) | 0.3, and an entity passed for 24 units or more counts twice (`PassMaterials::entity_twice_from`) |
| sounds Q3: feet in water | slosh steps at 0.5 every 0.3 s running; entering or leaving: slosh at 1.0, then a wade wave at 0.2 | slosh steps; Player.Swim at entry and exit | none (the wade wave noted) |
| sounds Q2: other entries' waves | uniform, repeats allowed (a crate's 3 hard-impact waves 29/22/29 of 80) | uniform | none |
| physics_constraints Q2: motors | hinge: the value in deg/s, clockwise about the axis, at once; slider: 5 × the value in units/s; 0: off, it coasts | ignored | as CS:S (logic controls: `ControlKind::Slide`) |
| physics_constraints Q3: hinge friction | no effect measured (0, 1, 10) | ignored | none |
| physics_constraints Q4: phys_spring | `constant` × stretch, kg·units/s² a unit (period 2π√(m/k)) | not made | none (missing; no stock map) |
| physics_constraints Q6: attach typo | held to the world | the same | none |
| physics_constraints Q7: equal ragdoll limits | locked | a free ball joint | welded when every axis is |
| physics_constraints Q8: pulley | second body moves 1/gearratio as far | not made | none (missing; no stock map) |
| physics_constraints Q9: Break on a never-made joint | OnBreak, removed | the same | none |
| weapons Q3: scoped and burst recoil | scoped AWP/scout 2.0 up, SG550/G3SG1 random as unscoped; a FAMAS burst kicks on its first round only | burst rounds 2-3 kicked | first round only |
| sounds Q9: server weapon sounds | the server emits none of a gun's own sounds; the knife's draw, the pickup, and one dry-fire click per press | one click per press | none |
| sounds Q10: ambient_generic presets | none on stock maps, 2 on community maps; radius, health, pitch always written | presets read | none |
| viewcontrol Q4: ForceSpawnAtEntityOrigin | accepted by env_entity_maker (de_train's `pt_can`) | accepted | none |
| viewcontrol Q5: output separator | commas in every map | commas | none |
| viewcontrol Q3: camera path | the moveto corner skipped; speed approaches by 3% a tick (frozen); unfrozen crawl 3 units/s | as the spec | none |
| ropes: `_back` materials, `cable.vmt` | six `_back` materials; cable.vmt opaque, cable_back translucent | as the spec | none |
| sounds Q6: gravel steps | gravel keeps its step sounds (the cs file overrides friction only) | the same (keys merged) | none |
| sounds Q8: scrapes | none under 60 units/s (30 kg crate on concrete), volume 0.05-0.25 (scattered), a volume-0 stop | our energy rule | none (noted) |
| weapons Q2: airborne target | the probe bot's jump ducks it (45-unit hull, no duck button) | the crouched target | none (a human's jump needed) |
| grenades Q10: crouched victims | aimed at 0.7-1.0 of the crouched eye height | the same (the eye offset) | none |
| grenades Q10: a static prop between | stops the blast whole, as the world | let it through (props weren't traced) | stops it; flashes too (`grenade::is_world`) |
| grenades Q10: a prop entity or a player between | shields part: 0.767 (crate) and 0.733 (player) left with the victim covered, 0.81-0.98 partly covered; HE and env_explosion alike | nothing | `BlastShield` (three lines, a fit) |
| objectives Q3: walls between bomb and victim | nothing stops or weakens the bomb's blast (the bell's whole value behind a 200-unit stone wall and a static prop) | walls stopped it | through walls (`BombRules::through_walls`) |
| grenades Q6: thrown flush against a wall | starts where its box meets the wall on the way from the eye (4 units off a 45° panel) and bounces back | 16 units ahead whatever: inside or past a thin wall | a box trace from the eye (`throw_frame`) |
| spectating: mode 3 | nobody left to watch (`mp_forcecamera 1`) gives the fixed view 3 s after the last teammate's death, round end or not; the camera at that player's eye | flying free | the camera held (`SpecMode::Fixed`, "Fixed View") |
| spectating: first target | the next entity index after the watcher's own (4 watched 5, not 1); after the watched one dies, the next after it | by name | none (noted: bots join in name order) |
| doors_buttons Q6: door move sound | starts on the activation tick; stopped on arrival (open and closed) as the arrival sound plays | a one-shot that played out | stopped on arrival (`movers::move_sound`) |
| objectives Q1: teleported while arming | out of the zone: no bomb, C4 kept; within it: planted where the planter stands | the same | none |
| physics_brushes Q4: +use on a light prop | nothing: no pickup | no pickup | none |
| movement Q16: jumping off a ladder | 270 units/s away from it on the jump tick, whatever key is held | the same, tick-exact (six `ladderjump_*` movecmp runs on de_nuke) | none |
| grenades Q12: a flash in smoke | blinds 0.56 as long (3.81 s for 6.80), as white | smoke ignored | × 0.56 when the line crosses a cloud (`Flash::through_smoke`, a fit) |
| fire Q6: a burning gas can next to a crouched player | hurts nobody but itself (1 + 3.728 every 13 ticks) | the same | none |
| weapons Q7: where a prop counts twice | from a chord of 24.0 units (23.7 once, 24.4 twice; 24.0 both ways) | (the 24 guessed) | none (confirmed) |
| objectives Q3: the bomb on a map with `bombradius` 400 | 400 at the bomb, σ = 400 × 3.5 / 3 (de_nuke: 322.41 at 306.5 units, the fit 322.4) | the same | none |
| grenades Q12: a player or a prop between a flash and the eye | stops it whole (the line against the player's box: 15 units off its centre stopped, 17 didn't; a crate too) | only the world and static props stopped it | stopped by players and props (`grenade::flash_blocked`) |
| grenades Q12: glass or a grate between | lets it through whole (de_nuke's windows, a door's pane, window-contents vents, a grate catwalk); solid brushes and doors stop it | glass and grates stopped it | glass and grates pass (`map::SeeThrough` from window/grate contents) |
| triggers Q3: overlapping triggers | of two teleports the lower entity index wins; a push, a gravity or a trigger_multiple sharing a teleport's volume still acts | the higher index won | touched highest index first |
| triggers Q8: a trigger_hurt's first pass | 4 ticks after the touch, then every 33 | the next tick | 4 ticks (`HURT_FIRST_TICKS`) |
| fall_damage: `damagemodel` 1 | 5, 10, 20, 25, 25 ... (damage 10, cap 50) | the same | none |
| visual_entities Q3: env_laser against armour | armour's ratio-1 part, whole points (10.5 a hit: 10, or 5 and 3 armour) | no armour, the fraction kept | as CS:S (`Effect::Hurt`) |
| entity_io Q10: I/O Use on a func_tracktrain | ignored (stopped or moving, any activator) | toggled it | ignored |
| doors_buttons Q4: riding a lift | carried on the same tick, 1.5 units a tick at 100, never a tick behind | the same | none (test tightened) |
| doors_buttons Q5: OnNextPoint | every tick the train moves | the same | none |
| doors_buttons Q3: +use through glass or a grate | blocked | blocked | none |
| entity_io Q1: absent classes at a restart | none kept (every one tried, e.g. trigger_teleport, func_tracktrain, logic_auto, game_text, filters) | none kept | none |
| fire Q9: buried fires | inside a brush: no damage; inside a static prop: damages | the same | none |
| movement Q18: feet exactly on a floor | start solid: held 17-33 ticks, then 1/32 up on the ground; 0.01 above stands at once | stands at once | none (noted; a known difference) |
| physics_brushes Q6: running into a map prop_ragdoll | solid (group none): stopped, then shoves it as a prop; debris (flag 4): runs through | the same, but no shove | none (noted) |
| physics_brushes Q7: prop_ragdoll on a model without a ragdoll | crashes the dedicated server | lies still | none |
| movement Q15: a func_conveyor | carried 1.5 a tick at 100 from the first tick; stepping off: 100 × 1.0075 less friction | the same | none |
| objectives Q14: defuse and blast timing | done 10.11 s after the defuse starts, the blast 45.105 s after the plant: the bomb thinks every 8 ticks from 7 after the plant and both wait for a think (sixth pass) | 10.0, 45.0 on the tick due | on the bomb's think (`BombRules::think`) |
| grenades Q17: `drop` with a grenade in hand | nothing: it stays in hand | dropped it | kept (`kept_on_drop`, `drop::KeptOnDrop`) |
| particles_and_smoke Q6: an unknown input | "unhandled input" in the console, nothing done | the same | none |
| doors_buttons Q7: a user on a rotating door's plane | opens to +distance | the same | none (test) |
| objectives Q13: `info_bomb_target`, `info_hostage_rescue` | no such entity classes: a map's do nothing | 128- and 256-unit spheres | ignored |
| physics_brushes Q3: phys_thruster force | kg·units/s², a = F/m (400000 on 400 kg: 199 units/s² up against gravity) | the same | none (test comment) |
| objectives Q14: a kit's defuse | 5 s to the next think (339 server ticks twice) | 5 s | on the think |
| objectives Q14: defuse and blast due on one think | it blows | the defuse won | the explosion first |
| objectives Q10: hostage give-up distance | 2000 units (1980 kept, 2060 given up at once) | 1500 | 2000 |
| objectives Q17: log lines | `"A" killed "B" with "usp"`, `"B" committed suicide with "world"`, `Begin_Bomb_Defuse_With_Kit`/`Without_Kit`, attacks (logdetail 3), say, team, connect lines; no purchases | the bomb's and rounds' triggers only | kill, suicide and defuse-begin lines; `bomb_planted` posx/posy |
| weapons Q7: tile and glass thickness | tile 0.642-0.676 (Glock, Deagle, AK-47 through inferno's big pot), glass (pottery) 0.987-1.0 | 1.0 both | tile 0.65 |
| weapons M13: buckshot | passes nothing (wood from 2.6 units, a player) | power 15 (30 of wood, a player) | power 0 |
| grenades Q17: dying primed while running | the live grenade keeps the runner's 250 | the same | none (test) |
| physics_brushes Q1: physbox mass | the compiler's stored mass (72.46, lump 29) | the same | none |
| physics_brushes Q3: an offset thruster's arm | from the centre of mass (level with an oil drum's centre: a spin; at its base: a tip) | from the origin | from the centre of mass |
| fire Q5: where started fires land | on displacements and static props as on brushes (de_train's on the flat cars); inside a static prop: no drop | the same, but a fire inside a static prop dropped out of it | stays (`Collision::inside_prop`, `map::MapPropBrushStart`) |
| physics_brushes Q6: shoving a solid map ragdoll | shoved | not shoved | tried (the shadow on its parts) and reverted: it launched the player over a lying corpse |
