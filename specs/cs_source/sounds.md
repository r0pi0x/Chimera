# Counter-Strike: Source: sounds

Source basis: Valve's public Source SDK 2013 only. I read the shared sound-emitter glue in the game code (script lookup, emit paths, precaching, per-map overrides), the public sound constants header (channels, sound levels, attenuation conversions, flags, file-name prefix characters), the sound-parameter text parsing, the interval parser, the recipient filters, the shared player footstep, jump, landing and swim code, the shared movement constants, the server and client physics impact, scrape and break sound code, the client bullet-impact sound code, the shared weapon sound hooks and weapon script parsing, the SDK template mod's bullet-firing effects (a sibling of CS:S's), the server ambient_generic entity, the server soundscape entities and their per-player selection, and the client soundscape player. I also read CS:S game data from the user's install: the sound script manifest and entries, the surface property scripts and the soundscape scripts, and I scanned the RIFF headers of about 2,400 extracted sound files.

Not in the SDK: the engine mixer and spatializer (distance-to-gain curve, panning, DSP, channel stealing), the sound-emitter library that parses scripts and picks waves, the physics library that computes friction energy, and all of CS:S's own game DLL. Those parts are marked **engine-side** or **CS:S-specific** and listed under Open questions, with ways to measure them.
Status: draft

## Summary

- Game code never names wave files directly. It names a **sound entry** ("Weapon_AK47.Single", "Concrete.StepLeft") from text scripts. An entry gives the channel, volume, pitch and sound level, and one or more waves, picked at random.
- Gameplay sounds come from rules on top of that. Footsteps use a timer whose period depends on speed band, ducking, ladder and water depth, alternate left and right, and take the surface under the player from the surface property scripts. Landings and jumps reuse the footstep sound at fixed volumes chosen by fall-speed tiers. Physics impacts and scrapes are picked from the surface properties of both touching materials, with the collision speed as input.
- Map ambience has two layers. ambient_generic entities are point sounds with optional pitch and volume ramps. Soundscapes are client-side scripted sets of looping and random sounds, chosen per player by the nearest visible env_soundscape and crossfaded over 3 s.

## Units and conventions

- Distances are Hammer units (1 unit ≈ 1 inch), Z up. Speeds are units/s.
- Time: seconds, except the footstep, swim and fade timers, which count milliseconds. Movement runs once per server tick: dt = 1/64 s (15.625 ms) on 64-tick servers, about 1/66 s on 66-tick servers. All tick counts below use 64 tick unless stated.
- Volume: a linear gain factor, 1.0 = full ("VOL_NORM").
- Pitch: a percentage of the recorded rate. 100 ("PITCH_NORM") is unchanged, 95 is "PITCH_LOW", 120 is "PITCH_HIGH". Valid range 0–255, integer. A pitch p plays the wave at p/100 of its rate (engine-side, assumed; this changes both pitch and duration).
- Sound level: an integer "dB" value from 0 to 255 ("SNDLVL_75dB" = 75). It is not physical dB: it only selects a distance falloff curve. 0 ("SNDLVL_NONE") means no distance falloff (heard everywhere at full volume).
- Attenuation: the older, Quake-style falloff factor. It converts to and from sound level (see Sound level and attenuation).
- Wave paths in scripts are relative to `sound/`. "weapons/ak47/ak47-1.wav" means `sound/weapons/ak47/ak47-1.wav` in the VPKs or the game folders.

## Constants

| Name (ours) | Value | Unit | Meaning |
|---|---|---|---|
| vol_norm | 1.0 | – | default entry volume |
| pitch_norm / low / high | 100 / 95 / 120 | % | named pitches |
| sndlvl_norm | 75 | dB | default entry sound level |
| sndlvl_max_normal | 255 | dB | normal levels are 0–255. 256–511 is reserved for "GoldSrc-compatible attenuation" sounds (9-bit field) |
| attn_norm | 0.8 | – | "ATTN_NORM" (= 75 dB) |
| attn_idle / static / ricochet / gunfire | 2.0 / 1.25 / 1.5 / 0.27 | – | other named attenuations |
| max_attenuation | 3.98 | – | network limit (attenuation is sent as ×64 in 8 bits) |
| nominal_clip_dist | 1000 | units | base distance in the multiplayer audibility filter |
| max_sounds | 16384 | – | precache table size (14-bit index) |
| max_delay | ±4096 | ms | largest start delay the network format carries |
| step_vel_walk_stand / run | 90 / 220 | u/s | footstep speed bands, standing (shared code) |
| step_vel_walk_duck / run | 60 / 80 | u/s | footstep speed bands, ducked or on a ladder (shared code) |
| step_period_run | 300 | ms | step timer after a "running" step on ground or in shallow water |
| step_period_walk | 400 | ms | step timer after a "walking" step on ground or in shallow water |
| step_period_ladder | 350 | ms | step timer after a ladder step |
| step_period_wade | 600 | ms | step timer after a knee-deep wading step |
| step_period_duck_add | +100 | ms | added to every step period while ducked or on a ladder |
| step_vol_duck_mult | 0.65 | – | footstep volume multiplier while ducked |
| ladder_step_vol | 0.5 | – | ladder step volume |
| wade_step_vol | 0.65 | – | knee-deep wading step volume |
| knee_frac | 0.2 | – | knee test point height = 0.2 × hull height |
| jump_step_vol | 1.0 | – | volume of the footstep played on a jump |
| land_step_timer | 400 | ms | step timer after a landing or slam sound |
| fall_punch_threshold | 350 | u/s | minimum fall speed for any landing sound (non-HL2 shared value) |
| fall_max_safe | 580 | u/s | above this: volume 1.0 and fall damage |
| fall_half_safe | 290 | u/s | above this (to 580): volume 0.85 |
| fall_min_bounce | 200 | u/s | below this (after adjustments): silent |
| land_on_floating | 200 | u/s | subtracted from fall speed when landing on a floating object |
| slam_threshold | 580 / 1160 | u/s | horizontal speed lost in one slide move for slam volume 0.85 / 1.0 |
| swim_sound_period | 1000 | ms | minimum gap between swim-stroke sounds while holding jump in water |
| impact_min_speed | 70 | u/s | physics impact sound threshold |
| impact_min_dt_server | 0.05 | s | minimum time since the pair's last collision (server) |
| impact_min_dt_client | 0.1 | s | same, for client-only physics objects (strictly greater) |
| impact_full_speed | 320 | u/s | impact volume reaches 1 at this speed |
| impact_merge_after | 4 | entries | after this many queued impact sounds in a frame, new ones merge |
| break_merge_after | 2 | entries | break sounds of the same material merge once more than 2 are queued |
| scrape_min_energy | 75 | (physics energy) | scrape callbacks below this are ignored (server/shared path) |
| scrape_energy_scale | 1/15500 | – | energy → scrape amplitude scale |
| scrape_min_volume | 1/128 | – | scrape amplitude² must exceed this to do anything |
| scrape_start_volume | 0.1 | – | script volume × amplitude² must exceed this to start a scrape |
| scrape_ramp | 0.1 | s | volume/pitch ramp time of a scrape update |
| scrape_timeout | 0.1 | s | a scrape stops if not refreshed for this long |
| scrape_mp_update | 0.5 | s | multiplayer: a running scrape is updated at most this often |
| scrape_slots_server / client | 4 / 8 | – | simultaneous scrape sounds |
| impact_group_radius | 300 | units | multi-pellet shots: an identical impact sound within this distance is skipped (SDK template mod; CS:S assumed) |
| ambient_update_rate | 5 | Hz | ambient_generic ramp/LFO update rate (0.2 s per step; first step 0.1 s after start) |
| ambient_ref_dist | 36 | units | ambient_generic radius → sound level reference distance |
| ambient_ref_level | 40 | dB | level the radius formula assigns at the radius |
| ambient_min_audible | 1.01e-3 | gain | gain treated as inaudible when finding an ambient's max audible distance |
| soundscape_fadetime | 3.0 | s | cvar: crossfade time for soundscape loops |
| soundscape_rand_radius | 36 | units | distance from the listener for "position random" soundscape sounds |
| soundscape_loop_start_vol | 0.05 | – | starting volume of a new positional soundscape loop |
| soundscape_max_positions | 8 | – | position0 … position7 on env_soundscape |
| soundscape_max_recursion | 8 | – | nested playsoundscape depth limit |
| soundscape_trace_budget | 20 | traces | per server frame (soft limit; see Soundscape selection) |
| trigger_soundscape_think | 0.2 | s | dead/spectator re-check period |

## Behavior

### 1. Sound scripts

#### Files and loading order

- `scripts/game_sounds_manifest.txt` lists the script files. Each line is either `"precache_file" "<path>"` or `"preload_file" "<path>"`. CS:S lists, in order: hostages and bots (precache), then game_sounds.txt, game_sounds_physics.txt, game_sounds_radio.txt, game_sounds_weapons.txt, game_sounds_ambient_generic.txt, game_sounds_world.txt and level_sounds_general.txt (preload).
  - **preload_file**: every wave of every entry in the file is precached at level start, whether or not anything uses it.
  - **precache_file**: an entry's waves are precached only when an entity asks for that entry by name.
  - The parsing is engine-side (sound-emitter library). The game code only shows the "preload this entry" flag and that level start precaches flagged entries. The mapping of `preload_file` to that flag is inferred from the names.
- Game files that exist but are not in CS:S's manifest (hl2's game_sounds_player.txt, game_sounds_items.txt, …) are not loaded. CS:S defines its own copies of shared entries such as "Player.Swim" and "Player.FallDamage" in game_sounds.txt.
- **Per-map overrides:** at level start (client and server) the game loads `maps/<mapname>_level_sounds.txt` if it exists. Its entries override same-named entries for that level and are cleared at level end. `<mapname>` is the map file name, lower-cased and without the extension.
- Entry names are looked up case-insensitively (engine-side; assumed from usage such as "SNDLVL_140db" and "Metalgrate"/"MetalGrate" in the data).

#### Entry format

A KeyValues block per entry:

```
"<Entry.Name>"
{
    "channel"   "CHAN_WEAPON"          // optional
    "volume"    "1.0" | "0.5, 0.6" | "VOL_NORM"
    "pitch"     "PITCH_NORM" | "95,105" | "100"
    "soundlevel" "SNDLVL_75dB" | "70,80"
    "CompatibilityAttenuation" "0.52" | "ATTN_NORM"   // alternative to soundlevel
    "wave"      "<path>"               // one wave, or:
    "rndwave"   { "wave" "<path>"  "wave" "<path>" ... }
}
```

In CS:S's loaded scripts, keys are used this often: "wave" 1366, "volume" 808, "pitch" 736, "channel" 560, "CompatibilityAttenuation" 448, "soundlevel" 350, "rndwave" 276.

Parsing rules (from the shared parsing code):

- **Defaults** when a key is absent: channel CHAN_AUTO (0), volume 1.0 with no range, pitch 100 with no range, sound level 75 with no range, delay 0.
- **channel**: if the text starts with "chan_" (case-insensitive), it is matched to CHAN_AUTO 0, CHAN_WEAPON 1, CHAN_VOICE 2, CHAN_ITEM 3, CHAN_BODY 4, CHAN_STREAM 5, CHAN_STATIC 6 or CHAN_VOICE2 7. An unknown "chan_…" name gives CHAN_AUTO. Text that doesn't start with "chan_" is read as an integer. CS:S data uses CHAN_ITEM 209, CHAN_STATIC 165, CHAN_VOICE 136, CHAN_BODY 118, CHAN_WEAPON 57 and CHAN_AUTO 6 times, plus one lower-case "chan_voice".
- **volume**: "VOL_NORM" gives 1.0. Otherwise it is an interval (below). Stored as a 16-bit float.
- **pitch**: "PITCH_NORM", "PITCH_LOW" or "PITCH_HIGH" give 100, 95 or 120. Otherwise an interval. Stored as unsigned 8-bit integers, so values are whole numbers from 0 to 255.
- **soundlevel**: text starting with "SNDLVL_" (case-insensitive) is matched to a named level: NONE 0, 20/25/30/35/40/45/50/55dB, IDLE 60, TALKING 80, 60/65dB, STATIC 66, 70dB, NORM 75, 75/80/85/90/95/100/105/110/120/130dB, GUNFIRE 140, 140/150/180dB. If the name doesn't match, the digits after "SNDLVL_" are read as an integer and accepted if 1–180 ("SNDLVL_97dB" gives 97). Otherwise the level is 75. Text not starting with "SNDLVL_" is an interval. Matching is case-insensitive, so CS:S's "SNDLVL_140db" and "SNDLVL_100DB" work.
- **Interval syntax** ("a" or "a,b"): split on the first comma. start = the C-style float value of the first part, range = (value of the second part) − start, or 0 with no comma. Parsing stops at the first non-numeric character, so trailing garbage is ignored: CS:S's dust2 soundscape has `"time" "15,40``"`, read as 15 to 40. "95, 105" gives start 95, range 10. A reversed "105,95" gives a negative range (start 105, range −10).
- **Drawing a value** from an interval (start s, range r): uniform in [s, s + r] (a float draw). Pitch and sound level are then cast to integers. For the soundscape system (which does its own draws), r = 0 returns s exactly with no random draw.
- **CompatibilityAttenuation**: present in 448 CS:S entries, including every footstep, physics and gun sound. It is handled entirely engine-side. The constants header says levels 256–511 are "GoldSrc-compatible" attenuation sounds. The likely meaning: the entry uses the older linear falloff, with level = attenuation→level conversion (+256 as a marker). This is unverified (Open question 1). Values used: 1.0 (footsteps, impacts), 0.52 (AK-47, M4A1, M249, XM1014), 0.48 (Galil, M3, SG552, FAMAS, AUG), 0.40 (G3SG1, SG550), 0.28 (AWP), 0.6 (Desert Eagle), 0.64 (MAC-10, MP5, P90, UMP45), 1.6 (TMP, Scout), "ATTN_NORM" (pistols: Glock, P228, Five-SeveN, USP, Elites).
- **wave / rndwave**: each "wave" adds one file to the entry's list. "rndwave" is a sub-block of "wave" keys, all added. One entry can have several. If an entry has no waves, nothing plays.
- **Wave name prefixes** (data characters at the start of a wave path, any of the first characters). Strip them to get the file path. Their meanings, all engine-side:
  - `*` stream from disk
  - `#` bypass room DSP (dry)
  - `)` spatialize a stereo file
  - `^` distance-variant stereo: left channel = near, right channel = far
  - `<` directional stereo: front/back mix by facing
  - `>` doppler stereo
  - `@` omnidirectional
  - `}` fast low-quality pitch shift
  - `!` names a sentence, not a file
  - `?` voice data

  In CS:S's loaded scripts, `)` starts 29 wave paths (most gunshots), `#` 8 and `^` 6.
- **Gender macro**: a wave path may contain "$gender". It is expanded from the emitting entity's model. Not used by CS:S player sounds; can be ignored.
- Other per-entry fields the parameter structure carries: a start delay in ms, and "play only to the owner". Their script key names are engine-side. CS:S data doesn't appear to use them.

#### Picking a wave and resolving parameters (per emit)

When an entry is played:

1. Look up the entry. If none exists, and the name contains ".wav" or ".mp3" or starts with "!", play it as a raw file or sentence with the caller's own channel, volume, level and pitch. Otherwise play nothing (silently).
2. Pick one wave from the entry's list at random. The random rule (uniform, or avoiding an immediate repeat) is engine-side (Open question 2).
3. Draw volume, pitch and sound level from their intervals.
4. The caller's flags may override: "change pitch" replaces the drawn pitch with the caller's, "change volume" replaces the volume. Without those flags the **script's** volume and pitch are used, not the caller's.
5. If the entry has a delay and the caller gave no start time, start time = now + delay/1000 s.
6. Emit with the resolved channel, wave, volume, level and pitch.

Several systems first resolve an entry to its wave and parameters, then emit the **resolved wave** with their own volume. Footsteps, physics impacts and scrapes do this. For these the script volume is either ignored or multiplied in explicitly, as each section below says.

#### Channels (engine-side, inferred)

Every sound is emitted on an (entity, channel) pair. A new sound on the same entity and channel replaces the one playing there. CHAN_AUTO and CHAN_STATIC always get a fresh slot. A flag exists to ask "don't overwrite the sound already on this channel", which implies overwriting is the default. Stop requests target (entity, channel, wave name). Stopping an entry that has several waves stops all of them.

#### Precaching

- A wave must be precached before it can play (engine table, 16384 entries).
- Asking to precache an entry name precaches every wave in the entry. Asking to precache a raw ".wav"/".mp3" name precaches that file. Unknown names do nothing; the engine prints a developer message once.
- On the server at level start, every entry flagged for preload has all its waves precached.
- The surface property scripts' sound names (steps, impacts, scrapes, bullet impacts, breaks) are all precached as entries at load.
- ambient_generic precaches its "message" at spawn, unless it starts with "!".

#### Sound level and attenuation

The constants header defines both conversions:

- level from attenuation: L(a) = trunc(50 + 20/a) for a ≠ 0. L(0) = 0.
- attenuation from level: a(L) = 20/(L − 50) for L > 50, else 4.0.

Examples: a = 0.8 → 75. 1.0 → 70. 0.52 → 88 (88.46). 0.28 → 121 (121.43). 0.27 → 124 (124.07). L = 75 → 0.8. 140 → 0.2222. 90 → 0.5. 60 → 2.0. L ≤ 50 → 4.0.

### 2. Spatialization

#### Who receives a sound (server, multiplayer)

- Most world sounds go to the players in the source's potentially-audible set (PAS: the map-visibility clusters the engine treats as audible from the source).
- **Attenuation filter** (weapon sounds and physics impact/break/scrape sounds use it): additionally, a player is dropped when the distance from their ear (eye) position to the source exceeds 2·1000 / a(L) = 100·(L − 50) units for L > 50, or 500 units for L ≤ 50. Examples: 75 dB → 2500 units. 88 → 3800. 100 → 5000. 140 → 9000. The filter is skipped when attenuation ≤ 0 and in single player. HLTV and replay bots are never dropped.
- Footsteps use PAS without this distance cut (see Footsteps).
- This filter only decides who is sent the sound. Loudness is computed by the engine.

#### Loudness and panning (engine-side)

The distance-to-gain curve, stereo panning, the effect of the facing and spatial-stereo prefixes, occlusion, DSP rooms and the mixer are not in the SDK. What the SDK shows:

- The engine answers "gain at distance d for level L" (the game uses it to find an ambient sound's max audible distance). The function itself is engine-side.
- The ambient_generic radius rule (below) assumes a sound of level L is perceived at L − 20·log10(d/36) "dB": 20 dB less per tenfold distance, from a 36-unit reference. It also states a 60 dB reference. Hypothesis H1, unverified: gain(d) = 10^((L − 60)/20) · 36/d, i.e. inverse-distance falloff, gain 1 at the distance where that is 1, with some clamp near the source and a cutoff far away. Under H1, at the ambient radius r the gain is 0.1.
- Hypothesis H2 for "CompatibilityAttenuation" entries (GoldSrc/Quake lineage, unverified for Source): linear falloff, gain(d) = max(0, 1 − d·a/1000), so a = 1.0 is silent beyond 1000 units and a = 0.52 beyond 1923 units. Quake-lineage stereo: per ear, gain × (1 ± dot(listener right, direction to source)).
- Level 0 ("SNDLVL_NONE", or the ambient "play everywhere" flag) plays at full volume everywhere, without distance falloff.
- Ambient sounds started without a position (soundscape ambients, client-side ambient emits) are not spatialized.
- Listener position: the player's eye position.
- "dsp" in soundscapes selects a room preset index 0–28 (names listed in the soundscape manifest's comments: 0 Normal off, 1 Generic, 2–4 Metal S/M/L, 5–7 Tunnel S/M/L, 8–10 Chamber S/M/L, 11–13 Bright S/M/L, 14–16 Water 1–3, 17–19 Concrete S/M/L, 20–22 Big 1–3, 23–25 Cavern S/M/L, 26–28 Weirdo 1–3). The presets are data in `scripts/dsp_presets.txt` (section 7); the processing is engine-side.

See Open question 1 for how to measure the real curves.

### 3. Player movement sounds

All of this runs inside the player movement tick (see specs/cs_source/movement.md), on the server and in the client's prediction, and for other players on the client (see Who hears footsteps).

#### Surface under the feet

- When movement finds ground, it records the ground trace's surface property. That surface's step sounds and game material are used.
- When the player leaves the ground, the last ground surface stays recorded. The jump sound uses the surface jumped from.
- Landing happens after the new ground is recorded, so the landing sound uses the surface landed on.
- For other players drawn on a client, the surface comes from the client's own trace: the player box swept 64 units straight down against world brushes only. No hit within 64 units: no surface, so no footstep.
- On a ladder the surface is always the "ladder" surface property, whatever the ladder is made of.
- Knee-deep water uses the "wade" surface property. Shallow water uses "water".

Surface properties (`scripts/surfaceproperties_manifest.txt` lists surfaceproperties.txt then surfaceproperties_cs.txt):

- Each block names a surface. "default" values copy into every surface unless overridden. "base" (must be the first key) inherits from another surface.
- Sound keys: "stepleft", "stepright", "impactsoft", "impacthard", "scrapesmooth", "scraperough", "bulletimpact", "break" ("strain" and "rolling" are unused here).
- Audio keys: "audioreflectivity", "audiohardnessfactor", "audioroughnessfactor", "impactHardThreshold", "scrapeRoughThreshold", "audioHardMinVelocity".
- Game keys: "gamematerial" (one letter), "jumpfactor", "maxspeedfactor", "climbable".
- "default" sets hardness 1.0, roughness 1.0, impactHardThreshold 0.5, scrapeRoughThreshold 0.5, gamematerial C, and the Default.* sounds.
- CS:S's file adds or overrides "gravel" (friction only), "snow", "metalgrate", "metalvehicle", "brass_bell_*" (bullet impacts) and "hay".
- A later file redefining a surface overrides it. Whether a partial redefinition (CS:S "gravel" gives only friction) merges with or replaces the earlier block is engine-side (Open question 6).

#### When a footstep is checked

Once per movement tick, after ground and water state are categorized and before ducking or moving (so it sees the start-of-tick velocity v, ground state and water level):

1. If the step timer T > 0: T ← max(0, T − 1000·dt). If T is still > 0, stop.
2. Stop if the player is frozen or at controls, in noclip, or an observer.
3. Stop if sv_footsteps is 0.
4. Let speed = |v| (3D) and ground speed = |v_xy|. Pick the speed bands (v_walk, v_run):
   - ducked, or on a ladder: (60, 80)
   - otherwise: (90, 220)
5. A step is allowed only if speed ≥ v_walk, and either the player is on a ladder, or is on the ground with ground speed > 0.0001.
6. "Walking" = speed < v_run.
7. Pick the case, in this order:
   - **Ladder**: surface "ladder", volume 0.5, T = 350.
   - **Knee in water** (CS:S rule: the point at height origin.z + 0.2·hull height is inside water): uses a counter **shared by all players** (Quirks). If the counter is 0, set it to 1 and stop without a sound and without setting T, so the next tick checks again. Otherwise increment it, and if it was 3, reset it to 0. Then surface "wade", volume 0.65, T = 600.
   - **Feet in water** (water level 1, knee dry): surface "water", volume 0.2 walking / 0.5 running, T = 400 walking / 300 running.
   - **Ground**: if there is no surface, stop. T = 400 walking / 300 running. Volume by the surface's game material:

     | game material | walking | running |
     |---|---|---|
     | D (dirt) | 0.25 | 0.55 |
     | V (vent) | 0.4 | 0.7 |
     | everything else (C concrete, M metal, G grate, T tile, S slosh, W wood, N sand, Y glass, …) | 0.2 | 0.5 |
8. If ducked or on a ladder: T += 100.
9. If ducked: volume × 0.65.
10. Play the footstep (below) at the player's origin (bottom centre of the box).

The duck +100 applies to every case. A ducked ladder step is 0.5 × 0.65 = 0.325 with T = 450. A ducked wade is T = 700.

#### Playing a footstep (also used by jump and landing)

1. Multiplayer with sv_footsteps 0: no sound.
2. Client prediction: only the first time a command is predicted (re-predictions are silent).
3. If there is no surface, no sound.
4. The player keeps a step side bit, starting at 0. Side 1 uses the surface's "stepleft" entry, side 0 its "stepright". So the first step after spawn is a right step. No entry for that side: no sound, and the side does not flip.
5. Flip the side bit.
6. Resolve the entry (random wave, script pitch and level). CS:S has no player-specific step override in the SDK; see Open questions.
   - Cache: if the entry has exactly one wave, its resolved parameters are cached per side and reused for later steps on that same entry. Any random pitch is then drawn once and frozen.
7. Emit the **resolved wave** on CHAN_BODY from the player entity, at the origin. Volume = the computed footstep volume; the script's volume is **ignored**. Level and pitch come from the script.
8. Recipients: everyone in the PAS of the origin. On a multiplayer server, everyone in the origin's PVS is removed, because those clients make the sound themselves (see next).

#### Who hears footsteps (multiplayer)

- The **server** sends the footstep only to players who can't see the stepper's position (in PAS, not in PVS).
- Each **client** plays its own footsteps through prediction (first prediction only).
- Each client plays every **other** visible player's footsteps itself, once per rendered frame. It runs the same check with the render frame time instead of dt, the player's estimated velocity, and the client-side ground trace above. So remote footstep cadence depends on frame rate, quantized to frames instead of ticks.

#### Cadence (what the timer gives)

Because the decrement happens before the check, after a step with period P the next step comes n = ceil(P / (1000·dt)) ticks later, if the conditions still hold. A period that is an exact multiple of the tick plays on exactly that tick.

| case | P (ms) | ticks at 64 | seconds | ticks at 66 |
|---|---|---|---|---|
| running, ground/shallow water | 300 | 20 | 0.3125 | 20 |
| walking, ground/shallow water | 400 | 26 | 0.40625 | 27 |
| ducked running | 400 | 26 | 0.40625 | 27 |
| ducked walking | 500 | 32 | 0.5 | 33 |
| ladder | 450 | 29 | 0.453125 | 30 |
| wade (knee) | 600 | 39 | 0.609375 | 40 |
| wade, ducked | 700 | 45 | 0.703125 | 47 |

The wading skip adds one extra tick to every fourth wade step (see Quirks). From standstill with T = 0, the first step plays on the first tick whose start-of-tick speed reaches v_walk.

#### Silence rules (shared code)

- Speed below v_walk: no step. Standing: < 90. Ducked or on a ladder: < 60.
- In the air, including swimming at water level ≥ 2 while off the ground: no step (needs ground or ladder).
- On the ground with zero horizontal speed (pure vertical motion): no step.
- sv_footsteps 0: no steps at all.
- In shared code, holding walk (+speed) only matters through speed. CS:S walking at 130 u/s (knife) would be ≥ 90 and audible, but in CS:S walking is known to be silent, and crouch-moving likely is too. So **CS:S overrides these bands or the rule** (Open question 3).

#### Jump

When a jump starts (the jump impulse is about to be applied, the player has just left the ground): play a footstep at volume 1.0 on the current (jumped-from) surface. This flips the step side. The step timer is not changed. CS:S has its own jump code, so whether it keeps this is Open question 4.

#### Landing

After each walking-mode move, if the player is on the ground and the stored fall speed f > 0 (f = downward speed recorded at the start of the tick while airborne):

1. If the player is alive and f ≥ 350:
   - Start with volume = 0.5.
   - If the water level > 0: keep 0.5. Skip steps 2–4 (no adjustments, no damage).
   - Otherwise:
     2. If the ground is a floating object: f ← f − 200.
     3. If the ground entity moves downward with speed vz < 0: f ← max(0.1, f + vz).
     4. Tiers on the adjusted f:
        - f > 580: volume 1.0, and fall damage is applied. If damage is actually taken, the server also plays "Player.FallDamage" at the player (PAS).
        - else if f > 290: volume 0.85.
        - else if f < 200: volume 0.
        - else: 0.5.
   - If volume > 0: step timer T = 400, then play a footstep at that volume on the landed-on surface.
2. Clear the fall speed (always, even below 350).

On solid, non-moving ground any audible landing is at least 350 and so at volume 0.85 or 1.0. The 0.5 and silent tiers only happen on floating or descending ground, or with feet in water.

#### Wall slam

After every slide move: loss = |v_xy before the move| − |v_xy after|. If loss > 1160: volume 1.0. Else if loss > 580: volume 0.85. Otherwise nothing. Same effect as a landing: T = 400 and a footstep at that volume on the current recorded surface.

#### Water: entry, exit and swimming

- **Entering or leaving water:** at the end of a walking/swimming-mode tick, if the water level went from 0 to non-zero or from non-zero to 0 since the start of the tick, play "Player.Swim" at the player's origin. The server also makes a splash effect. Ladder-mode ticks don't check this.
- **Swim strokes:** while the water level is ≥ 2 and jump is held, every tick checks the swim timer S. If S ≤ 0: S = 1000 ms and play "Player.Swim". S counts down by 1000·dt every tick (with the other movement timers, at the start of the tick). At 64 tick, holding jump in deep water gives a stroke every 64 ticks = 1.000 s, the first one immediately.
- How "Player.Swim" is played (both cases):
  - Server: emitted by the script entry (script volume and pitch) from the player entity, to the PAS of the origin.
  - Predicting client: plays it too, to the local player only, first prediction only.
  - Whether the owner hears it twice is Open question 5.
- CS:S's "Player.Swim" (game_sounds.txt): VOL_NORM, CompatibilityAttenuation 1.0, PITCH_NORM, rndwave of the four player/footsteps/slosh1–4.wav.
- Wading and shallow-water steps: see Footsteps (surfaces "wade" and "water"). CS:S data: "Wade.Step*" (wade1–4.wav, script volume 0.75, ignored) and "Water.Step*" (slosh1–4.wav).

#### Ladder

Ladder steps are footsteps with the "ladder" surface ("Ladder.StepLeft"/"Ladder.StepRight": player/footsteps/ladder1–4.wav), volume 0.5, period 450 ms, bands (60, 80). At the shared climb speed of 200 u/s the step is "running", which changes only the walking flag here: ladder volume and period don't depend on it.

### 4. Impact and physics sounds

#### Bullet impacts (client-side)

- Each client computes its own bullet impacts from the shot event (no server sound).
- Surface: from the client's own trace of the shot. If that trace hit a different entity than the server reported, or the server gave no surface, the client trace's surface is used. Otherwise the server's surface is used.
- Position: the client trace's end point if it hit, else the server's position.
- Play the surface's "bulletimpact" entry (script volume, pitch and level) to the local player only, at that position. No entry: nothing.
- **Multi-pellet grouping** (SDK template mod, which mirrors CS:S's shot code; assumed for CS:S): while one shot's pellets are processed, an impact sound whose entry name equals one already played within 300 units in this shot is skipped. The list resets after each shot.
- Examples (game_sounds_physics.txt):
  - "Concrete.BulletImpact": volume 0.7, CompatibilityAttenuation 1.0, rndwave physics/concrete/concrete_impact_bullet1–4.wav.
  - "Default.BulletImpact": 0.7, plastic box impacts.
  - "Water.BulletImpact": player/pl_wade2.wav.
  - CS:S's own surfaces "brass_bell_large/medium/small/smallest" use "BrassBell.C/D/E/F".
- Ricochets, penetration and flesh hits are CS:S-specific (Open question 7).

#### Physics impacts

- **Trigger** (server physics objects):
  - Collision speed s ≥ 70 and at least 0.05 s since that pair's previous contact.
  - At least one of the two entities is a physics-simulated object.
  - Neither surface has game material X (wade, ladder, default_silent and the like are silent).
  - An object hitting another part of itself (ragdoll limbs) is skipped if less than 0.5 s since that pair's previous contact. Such sounds use CHAN_BODY; all others use CHAN_STATIC.
- **Client-only physics objects**: a sound for each of the two objects that is not static, when the time since the last contact > 0.1 s and s > 70. CHAN_STATIC, from the world.
- **Volume:** v = min(1, (s/320)²). Examples: s = 70 gives 0.0478515625. 160 gives 0.25. 226.27 gives 0.5. 320 and above gives 1.
- **Queueing per physics frame:**
  1. Each impact is queued under the surface of the object making the sound, with impact speed s + 0.0001.
  2. Search the queue from its end for an entry with the same surface. Once the queue holds more than 4 entries, any entry matches, i.e. the last one.
  3. On a match: if the new v is greater than the entry's accumulated volume, replace the entry's position (object position), channel and hit surface with the new impact's. Then add v to the accumulated volume and keep the larger impact speed.
  4. With no match, append a new entry.
- **Playing** at the end of the physics frame, last entry first. For each entry with own surface P and hit surface H:
  1. P must have "impacthard". If not, skip the entry.
  2. Choose "impactsoft" instead, if P has one and either H's hardness factor < P's impactHardThreshold, or P's audioHardMinVelocity > 0 and the impact speed is below it.
  3. Resolve the entry. If the lookup fails, stop playing the rest of the queue (Quirks).
  4. Clamp the accumulated volume to ≤ 1.
  5. Emit the resolved wave from the world (not the entity) at the entry position, volume = script volume × v, script level and pitch. Recipients: the attenuation filter at the script level.
- Hardness examples: player 0.0 (props hitting players sound soft), dirt 0.25, mud 0.0, water 0.0, glass 1.0, default 1.0. With the default threshold of 0.5, anything hitting dirt, mud, water or a player plays its soft sound.
- Example: a "Wood_Crate" prop hits concrete at 200 u/s. v = (200/320)² = 0.390625. Concrete's hardness is 1.0 ≥ 0.5, and the crate surface has no min velocity, so the hard sound plays. Its volume is that entry's script volume × 0.390625.

#### Scrapes (looping friction sounds)

- **Input:** the physics engine reports friction energy E for an object sliding on a surface. The units are engine-side.
  - The server ignores any report while that object's scrape already plays and was updated less than 0.5 s ago; it only refreshes the keep-alive time.
  - The client ignores E < 0.05 and applies the same 0.5 s rule only in multiplayer.
- **Rule** (shared):
  1. Ignore if E < 75, or either surface has game material X.
  2. Amplitude e = E/15500. volume = e², clamped to [0, 1].
  3. Choose the own surface's "scraperough", unless it has a "scrapesmooth" and the other surface's roughness factor < own scrapeRoughThreshold.
  4. If volume ≤ 1/128: nothing (no refresh either).
  5. Find the object's scrape slot (server 4 slots, client 8). No slot and none free: nothing.
  6. **Starting:** only if script volume × volume > 0.1. Then create a looping sound on CHAN_BODY from the entity, at the script level, and play it at script volume × volume and script pitch.
  7. **Updating:** ramp the volume to script volume × volume over 0.1 s, and the pitch to pitchlow + volume × (pitchhigh − pitchlow) over 0.1 s, where pitchlow/pitchhigh are the ends of the script pitch range.
  8. Record the update time.
- **Stop:** every frame, a scrape not refreshed in the last 0.1 s is stopped and its slot freed.
- Thresholds in E: anything happens only above E = 15500·√(1/128) ≈ 1370. With "Default.ScrapeRough" (script volume 0.5), a new scrape starts only when e² > 0.2, i.e. E > 6932.
- Whether scrape sounds need a looping wave (cue chunk), and how E relates to mass and speed, are engine-side (Open question 8).

#### Break sounds

- When a breakable prop or surface breaks, its material's "break" entry (e.g. "Wood_Crate.Break", "Glass.Break") is queued at the break position.
- If 3 or more breaks are already queued, a new one with the same material is merged: the stored position becomes the midpoint of the old and new positions.
- Played at the end of the physics frame from the world on CHAN_STATIC: script volume, level and pitch, attenuation filter. A failed lookup stops the rest of the queue.

### 5. Weapon sound hooks (shared rules)

- **Weapon script.** The weapon's script has a "SoundData" block. Its keys name sound entries for these categories, in index order:
  - empty
  - single_shot, single_shot_npc, double_shot, double_shot_npc, burst
  - reload, reload_npc
  - melee_miss, melee_hit, melee_hit_world
  - special1, special2, special3
  - taunt, deploy

  Missing or empty keys mean no sound.
- CS:S weapon scripts are `scripts/weapon_*.ctx` (encrypted). This spec did not read them. Their entry names follow the pattern in game_sounds_weapons.txt: "Weapon_<Name>.Single", "Weapon_<Name>.Deploy", "Default.ClipEmpty_Rifle", "Default.ClipEmpty_Pistol", "Default.Zoom".
- **Playing a category:** resolve the entry.
  - If it is flagged "play only to owner", play it only to the owning player, from the owner.
  - Otherwise play it from the owner to the attenuation filter at the entry's level. With no owner (thrown items), play it from the weapon itself.
  - Predicted weapons use prediction rules: the shooter's client plays it locally once, and the server doesn't send it back to them.
  - The volume, pitch and channel are the script's.
- **When each category plays** (shared base weapon; CS:S overrides most of this, Open question 9):
  - **deploy**: when the weapon is drawn.
  - **empty**:
    - Attack pressed with an empty clip (or no ammo, for a weapon without clips): played if at least 0.5 s since the last empty sound, then the next allowed time = now + 0.5 s. Holding attack on empty plays it once, then tries to reload or switch.
    - Primary attack under water (eye level, water level 3) with a weapon that can't fire under water: plays every time, and the next attack is delayed 0.2 s.
    - Secondary attack without secondary ammo: same 0.5 s gating.
  - **single_shot**: once per shot, when the shot fires. Auto-firing weapons that fire several shots in one tick get one sound per shot, each stamped with its scheduled fire time.
  - **reload**: when a reload starts (client side only), and stopped if the reload is aborted.
  - **burst, double_shot, melee_*, special*, taunt**: only when game code calls them.
- **Model animation sound events:** animation event 5004 (and its modern form) carries an entry name. It plays that entry to the local player only, from the model's first attachment, or its origin if it has none. CS:S's reload part sounds ("Weapon_AK47.Clipout", "Weapon_AK47.Clipin", "Weapon_AK47.BoltPull": CHAN_ITEM, CompatibilityAttenuation 1.0, pitch 95–105) have no SoundData category, so they are most likely fired this way from the view model animations (Open question 9).
- **Data example** "Weapon_AK47.Single": CHAN_WEAPON, volume 1.0, CompatibilityAttenuation 0.52, PITCH_NORM, wave ")weapons/ak47/ak47-1.wav". Because all CS:S guns are on CHAN_WEAPON from the owner, a new shot cuts off the previous shot's tail on the same player (engine channel rule, inferred).

### 6. Map sounds

#### ambient_generic

**Keyvalues:**

- "message": entry name, raw wave, or "!sentence".
- "radius": audible radius in units.
- "SourceEntityName": play from another entity.
- "health": volume 0–10.
- "pitch": running pitch 0–255.
- "pitchstart": 0–255.
- "spinup", "spindown": 0–100.
- "volstart": 0–10.
- "fadein", "fadeout": legacy 0–100 rates.
- "fadeinsecs", "fadeoutsecs": 0–100 seconds.
- "lfotype": 0 off, 1 square, 2 triangle, 3 random; > 4 becomes triangle.
- "lforate": 0–1000.
- "lfomodpitch", "lfomodvol": 0–100.
- "cspinup": 0–100.
- "preset": 1–27.
- spawnflags: 1 play everywhere, 16 start silent, 32 not looping.

**Spawn:**

- Sound level from radius r: if r > 0 and the play-everywhere flag is off, L = trunc(40 + 20·log10(r/36)). Otherwise L = 0 (no falloff).
  - r = 36 → 40. 360 → 60. 1250 → 70 (70.81). 3600 → 80. 10000 → 88 (88.87).
  - Below about 0.57 units L goes negative; not guarded.
- An empty "message" removes the entity.
- A "message" starting with "vo" and ending ".wav", where that file doesn't exist but the ".mp3" does, is switched to the .mp3.
- "Looping" = the not-looping flag (32) is clear. This only changes the entity's on/off bookkeeping. Whether the audio itself loops comes from the wave file (cue/loop chunk; engine-side, see Mapping).
- **Max audible distance** (only used to decide whether to network a separate source entity to a client):
  - If L = 0 or r = 0: unlimited.
  - Else if the engine gain at r is ≤ 1.01e-3: r.
  - Else double the distance from 2r until the gain is ≤ 1.01e-3, or until the distance passes 100000, which means unlimited. Then bisect 4 times between the last two distances and keep the upper bound.

**Modulation parameters** (set at spawn and on every start). Volumes and pitches are integers; "frac" values are fixed-point ×256.

- vol_run = clamp(health × 10, 0, 100).
- If preset p is 1–27, the whole parameter set is replaced by preset row p, so health is ignored (Quirks). The rows are fixed tables of pitch run/start, spinup/down, vol run/start, fade in/out, LFO type/rate/mod pitch/mod vol and cspinup, 27 rows from the GoldSrc era. Implement only if a CS:S map uses presets (Open question 10).
- **Key conversions**:
  - spinup, spindown, fadein, fadeout (legacy) s in 1–100 → rate = (101 − s)·64 per step. Pitch or volume moves (101 − s)/4 units per step.
  - fadeinsecs, fadeoutsecs t in 1–100 → rate = floor(25600 / (5t)). Volume moves 100/(5t) units per step, so 0 → 100 takes t seconds.
  - volstart × 10.
  - lforate × 256.
- **On start**:
  - With a fade-in, vol = vol_start, else vol = vol_run.
  - With a spin-up, pitch = pitch_start, else pitch = pitch_run.
  - Pitch 0 becomes 100.
  - cspinup n > 0: pitch_run = pitch_start + floor((255 − pitch_start)/n), clamped to ≤ 255.
  - If any spin or pitch LFO is set and pitch = 100, use 101.
- **Emitted values**: volume = vol × 0.01, at L, from the source entity's position.

**Ramps** (every 0.2 s while any ramp or LFO is active; the first step comes 0.1 s after a start or toggle):

- **Pitch**:
  - frac += spinup (or −= spindown). pitch = frac/256.
  - pitch > pitch_run: clamp, spin-up done.
  - pitch < pitch_start: stop the sound, end ramping.
  - Clamp to 1–255.
- **Volume**:
  - frac += fadein (or −= fadeout). vol = frac/256.
  - vol > vol_run: clamp, fade-in done.
  - vol < vol_start: set to vol_start, stop the sound, end ramping.
  - Clamp to 1–100.
- **LFO** (pos 0–255, bouncing):
  - phase += rate; pos = phase/256. Bounce at 0 and 255 by flipping the sign of the rate.
  - mult: square = 255 when pos < 128, else 0. Triangle = pos. Random = a new uniform integer 0–255 each time pos hits 255.
  - Pitch += (mult − 128)·mod_pitch/100, clamped 1–255.
  - Volume += (mult − 128)·mod_vol/100, clamped 0–100.
- If pitch or volume changed this step, send a change (pitch 100 is sent as 101).

**Inputs:**

- **PlaySound**: if not active, stop any current instance, then toggle on.
- **StopSound**: if active, toggle off.
- **ToggleSound**.
- **Pitch** x: pitch = clamp(int(x), 0, 255). The SDK uses a fast float-to-int conversion whose rounding mode isn't stated; assume round-to-nearest. Sent immediately as a change.
- **Volume** x: vol = clamp(round(10x), 0, 100). Sent immediately.
- **FadeIn** t / **FadeOut** t: cancel the other fade, clamp t to 0–100 s, rate as fadeinsecs, start ramping in 0.1 s.

**Toggle on:**

- If not looping, first stop the sound (each trigger restarts a one-shot); if looping, mark it active.
- Re-initialize the modulation and start the sound with no change flags. So the script entry's own volume and pitch apply (Quirks).

**Toggle off:**

- With cspinup set, a toggle-off doesn't stop the sound. It raises pitch_run by one more step, up to cspinup steps.
- Otherwise mark inactive, and stop at once, or spin down / fade out first if those are set.

**Map start:**

- Active (looping, not start-silent) sounds start on activation with the "spawning" flag, plus change-pitch and change-volume flags so the entity's pitch and volume apply. They go into the signon data, so late joiners hear them.
- This happens only if vol > 0.
- On a round restart the spawning flag is not used.

**Level used**: the radius-derived L is passed to the emitter, but for script-entry names the shared emitter forwards the **entry's** level instead (Open question 11).

#### Soundscapes (client-side playback, server-side selection)

**Script files:** `scripts/soundscapes_manifest.txt` lists `"file" "<path>"` lines. CS:S lists 16 map files, plus soundscapes_general.txt. If `scripts/soundscapes_<mapname>.txt` exists and isn't listed, it is loaded too. Every top-level block with children is a named soundscape. Name lookup is case-insensitive, and the later definition wins.

**Commands inside a soundscape**, processed in file order:

- **"dsp" n**: set the room DSP preset n. Only at the top level.
- **"dsp_player" n**: set a player-only DSP. Top level only.
- **"dsp_volume" x**: set the dsp_volume cvar. Top level only. If no top-level dsp_volume is given, the cvar is reverted.
- **"soundmixer" name**: set snd_soundmixer. Top level only; reverted when absent.
- **"playlooping" { … }**: a looping sound. Keys:
  - "volume": interval, drawn once, × master volume. Default 0, and volume 0 means the sound is not added.
  - "pitch": interval drawn once, default 100.
  - "wave": a file path.
  - "position" n: index into the entity's positions, offset by the parent's starting position.
  - "attenuation": interval drawn once, converted to a level.
  - "soundlevel": a "SNDLVL_" name or an interval. Default level 75.
  - "suppress_on_restore": skip it when loading a save.
- **"playrandom" { … }**: random one-shots. Keys:
  - "time": interval in seconds between plays.
  - "volume", "pitch": intervals, each drawn per play. Both default to 0, so a block without "volume" is silent.
  - "soundlevel" or "attenuation": interval drawn per play. With neither, the level is 0 (no falloff).
  - "rndwave" { "wave" … }: the files.
  - "position": n, or "random".
  - "suppress_on_restore".
- **"playsoundscape" { … }**: include another soundscape by "name". Keys:
  - "volume": multiplies the master volume.
  - "position" n: shifts the starting position by n.
  - "positionoverride" n: forces every sound inside to that position, unless an outer override exists.
  - "ambientpositionoverride" n: forces only the sounds with no position.
  - A "soundlevel" key here is unsupported.
  - Nested soundscapes never apply DSP, mixer or dsp_volume. Nesting stops past depth 8.

**Positions:**

- Positions 0–7 come from env_soundscape's "position0"–"position7" keys. Each names an entity whose origin at selection time becomes that position.
- A positional sound whose index > 31, or whose position wasn't provided, is dropped.
- With no position and no override, a sound is **ambient**: unspatialized, heard everywhere.
- "position random": each play is placed 36 units from the listener's eye, in a uniformly random direction in the plane spanned by the view's right and forward vectors: eye + 36·(cos θ·right + sin θ·forward), with θ uniform in [−180°, 180°].

**Starting a soundscape** (when the client's selected soundscape or its entity changes):

1. Every playing loop gets target volume 0. Starting no soundscape at all cuts them to 0 at once.
2. Bump a generation counter. Clear the random-sound list. The next random check is now.
3. Process the commands.
4. **Adding a loop:** look for an existing loop from an older generation with the same wave (case-insensitive) and the same pitch.
   - Both ambient: reuse it.
   - Both positional at the same position (within 0.1 units): reuse it.
   - Both positional at different positions: stop it and restart at the new position.
   - Otherwise start a new one. Ambient loops start at volume 0, positional loops at 0.05, on CHAN_STATIC from the world, to the local player.
   - The loop's target becomes the new volume.
5. **Adding a random sound:** first play at now + 0.5 × (a draw from "time").

**Every client frame:**

- **Loops:** move each loop's current volume toward its target by frametime / soundscape_fadetime (3.0 s by default). A full 0 → 1 change takes 3 s, 0 → 0.45 takes 1.35 s. A loop that reaches target 0 at volume 0 is stopped and removed. Changes are sent as volume updates.
- **Random sounds:** checked only when the time reaches the earliest scheduled play. Each due sound:
  - Picks a uniform random wave from its list.
  - Plays at master volume × a volume draw, a pitch draw (cast to int), and a level draw (int), at its position (re-drawn if random). Ambient ones play unspatialized.
  - Is rescheduled at now + a draw from "time".

**Selection on the server** (per player):

- Each env_soundscape (keyvalues: "soundscape" name, "radius" with −1 = unlimited, "position0"–"position7", "StartDisabled"; inputs Enable, Disable, ToggleEnabled; output OnPlay) is tested against a player's ear position:
  - **In range**: radius = −1, or radius > distance from the entity's position to the ear.
  - **Visible**: a line between them, against world brushes and water only, is clear and doesn't start in solid.
- **Rules:**
  - The current soundscape is re-checked first. Being out of range or out of sight only marks it "not in range"; it stays current. It is replaced only when another soundscape qualifies.
  - Another enabled soundscape becomes current if it is in range and visible, and either the current one is not in range, or the candidate is closer than the current one.
  - A disabled current soundscape is dropped, and the player keeps hearing the old one until another qualifies.
  - Candidates come only from those listed for the player's visibility cluster, in list order.
  - On becoming current, the soundscape sends the player its index and its resolved positions, and fires OnPlay. A soundscape keeps playing when the player leaves its radius, until another one takes over.
- **Load balancing:** each server frame visits players round-robin. It stops after more than max(1, maxplayers/2) players, or once more than 20 traces have been made, but always finishes the player it started. So with many players, a player's soundscape may update only every few frames.
- **env_soundscape_proxy**: "MainSoundscapeName" names an env_soundscape whose soundscape and positions it copies. Its own position is used for range tests.
- **env_soundscape_triggerable + trigger_soundscape**:
  - The trigger ("soundscape" names the triggerable) applies it to players who start touching. Each player keeps a most-recent-first list.
  - On end-touch, the next one in the list applies. If none is left, the player's soundscape entity is cleared to "none" (index 0), which the client treats as no new soundscape.
  - Dead or spectating players are handled by a 0.2 s check of whether they intersect the trigger.
  - The triggerable doesn't take part in the distance/visibility selection.

### 7. Room DSP presets (dsp)

Source basis for this section: CS:S's `scripts/dsp_presets.txt` (data, read from the install; the engine reads it at start and `dsp_reload` re-reads it) and the engine's console variable names. The processors' signal algorithms are engine-side and not in the SDK; where this section says what a processor *does*, it is the file's own parameter descriptions restated, and the DSP maths itself is a hypothesis (Q16). Implementation should parse the file at run time from the install; the tables here are for tests and for when it is absent.

#### File format

- Text, `//` comments to end of line. Each preset is a brace block: `{ <index> <chain type> <mix min> <mix max> <duration> <fade> <db min> <db mixdrop>` followed by one brace block per processor, then `}`. Parentheses (braces) must balance; the processor count is the number of inner blocks.
- Some header fields may be missing: test, speaker and empty presets give only index, chain type and the two mix values (then `0 0 0 0` or nothing); treat missing numbers as 0.
- Indices are unique and contiguous from 0 (the file defines 0 to 139 and lists further names only as comments). Order in the file does not matter.
- **Chain types** (a word or its number): SIMPLE 0 and LINEAR 1 run the processors in series in order; PARALLEL2 5 sums P0 and P1 run side by side; PARALLEL4 6 sums the series P0→P1 and P2→P3; PARALLEL5 7 is PARALLEL4 followed by P4; FEEDBACK 8, FEEDBACK3 9, FEEDBACK4 10 feed a later processor's output back to the input sum; MOD 11, MOD2 12, MOD3 13 let P0 modulate the others (marked untested in the file). CS:S's presets 0–136 are all LINEAR; 137–139 are PARALLEL4.
- **Processor blocks**: the first token is the type (word or number): NULL 0 (pass-through; a block of zeros is a NULL), DLY 1 (delay with feedback), RVA 2 (reverb: parallel delays), FLT 3 (filter), CRS 4 (chorus), PTC 5 (pitch shift, marked not usable), ENV 6 (envelope), LFO 7 (oscillator), EFO 8 (envelope follower / gate), MDY 9 (modulated delay), DFR 10 (diffusor: series all-pass delays), AMP 11 (amplifier with distortion). Up to 16 numeric parameters follow; words stand for numbers: filter type LP 0 / HP 1 / BP 2; quality LO 0 / MED 1 / HI 2 / VHI 3; delay type PLAIN 0 / ALLPASS 1 / LOWPASS 2 / DLINEAR 3 / FLINEAR 4 / LOWPASS_4TAP 5 / PLAIN_4TAP 6 (a bare number is also accepted, e.g. preset 29); LFO wave SIN 0 / TRI 1 / SQR 2 / SAW 3 / RND 4 / LOG_IN 5 / LOG_OUT 6 / LIN_IN 7 / LIN_OUT 8.

#### Header fields

- **mix min, mix max** (0–1): how much processed ("wet") signal a sound gets. The file says the mix depends on the source's distance from the listener: near sounds get the minimum, far ones the maximum, so small rooms use a larger minimum. *Inferred:* linear in distance between the engine cvars `dsp_dist_min` and `dsp_dist_max` (values: Q16).
- **duration** (s): 0 = the preset stays until replaced; > 0 = it lasts that long and then the previous preset returns.
- **fade** (s): how the timed preset fades out; negative = an exponential fade.
- **db min, db mixdrop** (room presets only): a new sound whose level is below `db min` dB gets its mix min and max reduced by `db mixdrop` (a fraction), so quiet sounds outdoors stay drier.

#### Processor parameters (in file order)

| Type | Parameters |
|---|---|
| DLY | delay type; delay (ms; −1 = from room size × 2, auto presets only); feedback 0–1; output gain 0–10; filter type; cutoff Hz; band width Hz (band-pass: passes cutoff to cutoff + width); quality (number of filter stages); three tap values (4-tap types) |
| RVA | max size (ms); min size (ms); number of delays (parallel); feedback scale 0–1; output gain; lowpass cutoff in the feedback loops (Hz, 0 = none); filters inside the delays (1) or on the output only (0); modulation depth (ms, 0 = static delays); modulation rate (delay passes between changes); width, depth, height (ms, used instead of size when non-zero; −1 = from room size); reflectivity for width/depth/height (used as feedback instead of the decay; negative = also add a predelay); tap scale (> 0: taps at D·(1 − n·tap), n = 0..3) |
| DFR | size scale 0–1 (each of its delays is 13–41 ms × scale); number of series delays 0–4; feedback scale; optional gain |
| FLT | type; cutoff Hz (−3 dB point); band width Hz; quality (number of series filters); output gain |
| AMP | gain 0–10; distortion threshold 0–1 (1 = clean); distortion mix 0–1; distortion feedback; then optional self-modulation: rate Hz (0 = none), depth 0–1, glide ms, random (1) or alternating (0) |
| LFO | wave; rate Hz; one-shot (1) or looping (0); output gain 0–1 |
| MDY | delay type; delay ms; feedback; gain; filter type; cutoff; band width; quality; modulation rate Hz; modulation depth 0–1; glide ms; wet/dry mix |
| EFO | gate threshold dB (0 = no gate); attack ms; decay ms; exponential (1) |
| CRS | wave; rate Hz; depth 0–1; mix |
| PTC | pitch ratio; slice ms; crossfade ms |
| ENV | type; attack peak, decay target, sustain levels; attack, decay, sustain, release ms; exponential |

#### The presets CS:S uses

Every preset 0–40 has mix 0.2–0.7, db min 80, mixdrop 0.5 and (except where noted) duration 0, fade 0. "→" means in series.

| # | Name (ours) | Chain |
|---|---|---|
| 0 | off | nothing (pass-through) |
| 1 | automatic | nothing in the file; selecting it turns on the engine's automatic room DSP (`dsp_automatic`, the `adsp_*` cvars), which builds a room from presets 100–133 (Q16) |
| 2 | metal small | RVA 80/30 ms, 4 delays, feedback 0.85, gain 1.1, lowpass 4000, filters in delays |
| 3 | metal medium | RVA 80/30, 4, 0.9, 1.4, 4000, in delays |
| 4 | metal large | DFR 1.0 × 3, fb 0.1483 → RVA 100/30, 4, 0.95, 1.8, 4000, in delays |
| 5 | tunnel small | RVA 50/8, 2, 0.92, 1.1, 6000, in delays |
| 6 | tunnel medium | DFR 1.0 × 2, 0.15 → RVA 100/15, 2, 0.92, 1.1, 5000, in delays |
| 7 | tunnel large | DFR 1.0 × 3, 0.15 → RVA 120/25, 2, 0.95, 1.1, 4000, in delays |
| 8 | chamber small | DFR 1.0 × 2, 0.15 → RVA 50/20, 6, 0.9, 1.4, 5000, in delays, modulation 4 ms at rate 3.48 |
| 9 | chamber medium | as 8 with cutoff 6000 |
| 10 | chamber large | as 9 with 9 delays |
| 11 | bright small | RVA 50/20, 3, 0.9, 1.0, 5000, in delays |
| 12 | bright medium | RVA 50/20, 5, 0.9, 1.0, 5000, in delays |
| 13 | bright large | RVA 50/20, 6, 0.9, 1.0, 6000, output filter only |
| 14 | water small | DFR 1.0 × 3, 0.15 → AMP gain 1, clean, self-modulation 10 Hz depth 0.6 glide 80 ms alternating → RVA 82/59, 2, 0.4, 2.0, 1800, output filter, modulation 10 ms rate 3.0 |
| 15 | water medium | DFR 1.0 × 2, 0.15 → RVA 50/20, 5, 0.9, 1.4, 1000, output filter, modulation 4 / 3.48 |
| 16 | water large | DFR 1.0 × 2, 0.15 → RVA 50/20, 7, 0.9, 1.0, 1000, output filter, 4 / 3.48 → MDY plain 500 ms, fb 0.4, gain 1, no filter, rate 2 Hz, depth 0.01, glide 15 ms, mix 1 |
| 17 | concrete small | DFR 1.0 × 2, 0.15 → RVA 50/20, 6, 0.9, 1.4, 4000, in delays, 4 / 3.48 |
| 18 | concrete medium | as 17 with 7 delays, cutoff 3500 |
| 19 | concrete large | as 17 with 8 delays, cutoff 3000 |
| 20 | outside small | DFR 1.0 × 2, 0.15 → DLY lowpass-in-loop 300 ms, fb 0.5, gain 0.84, LP 2000, quality LO |
| 21 | outside medium | as 20 with 400 ms, LP 1500 |
| 22 | outside large | as 20 with 750 ms, LP 1000 |
| 23 | cavern small | DLY lowpass 150 ms, fb 0.5, gain 0.84, LP 3000 → RVA 50/20, "1.3" delays (as written; *inferred* truncated to 1), 0.9, 1.0, 1500, in delays, 4 / 3.48 |
| 24 | cavern medium | DLY lowpass 200 ms, fb 0.7, gain 0.6, LP 3000 → RVA 50/20, 7, 0.9, 1.0, 1500, in delays, 4 / 3.48 |
| 25 | cavern large | DFR 1.0 × 2, 0.15 → DLY lowpass 300 ms, 0.7, 0.6, LP 3000 → RVA 50/20, 9, 0.9, 1.0, 1500, in delays, 4 / 3.48 |
| 26–29 | weird 1–4 | DLY lowpass 400 ms, fb 0.5, gain 0.6, LP 1500 → DFR 1.0 × 2, 0.15 (all four identical) |
| 30 | facing away | FLT lowpass 3000, quality MED, gain 1 |
| 31 | facing away, 80 ms | FLT lowpass 1000, MED, 1 → DLY linear 80 ms, no feedback, gain 1 |
| 32–34 | explosion ring 1–3 | duration 1.6, exponential fade: DFR 1.0 × 3, 0.15 → FLT lowpass 1000, MED, gain 0.25 |
| 35–37 | shock muffle 1–3 | duration 1.6, exponential fade: DFR 1.0 × 2, 0.15 → LFO sine 3000 Hz, looping, gain 0.25 |
| 38 | distorted speaker | AMP gain 100, threshold 0.005, full distortion → FLT highpass 1000 LO → FLT lowpass 2000 LO → FLT lowpass 2000 LO gain 0.8 |
| 39 | pre-fire wobble | duration 1.0, fade −0.2: LFO square 50 Hz, gain 0.1 |
| 40 | player wall delay | MDY linear 100 ms, no feedback, gain 1, glide 50 ms, mix 1 |
| 41–43 | reserved | empty |
| 44–49 | test slots | pitch ±10 %, gates, a chorus, empty (not used by the game) |
| 50–54 | speaker slots | empty |
| 55–59 | speakers | two AMPs (gain 10, threshold 0.01, mix 0.65; then gain 0.65) → band-pass filters around 2500 Hz (57 adds a 180 ms echo) |
| 60–99 | slots for the automatic DSP | empty |
| 100–133 | automatic DSP templates | pairs "min / max" for rooms, ducts, halls, tunnels, streets, alleys, courtyards and open spaces, each empty/diffuse; mix ranges 0.2–0.9; db min 60–95 |
| 134 | flashbang muffle long | mix 0.2–0.7, duration 1.6, exponential fade: DFR 1.0 × 2, 0.15 → LFO sine 3000 Hz gain 0.05 |
| 135 | flashbang muffle medium | as 134 with mix 0.2–0.4, duration 0.2 |
| 136 | flashbang muffle short | as 134 with mix 0.2–0.2, duration 0.1 |
| 137 | HE shock start | PARALLEL4, duration 1, exponential fade: (DFR 1.0 × 3, 0.15 → FLT lowpass 4000 HI gain 0.5) + (DFR 1.0 × 2, 0.15 → LFO sine 3000 gain 0.01) |
| 138 | HE shock middle | as 137 with duration 1.5, lowpass 2000, LFO gain 0.02 |
| 139 | HE shock long | as 137 with duration 3, lowpass 1000, LFO gain 0.03 |

Names: the soundscape manifest's comments name 0–28 differently in places (1 "Generic", 20–22 "Big 1–3", 26–28 "Weirdo 1–3"; see section 2); the numbers are what matter.

Use in CS:S's stock soundscapes (`"dsp"` values over all `soundscapes_*.txt`): 6 ×98, 1 ×23, 0 ×22, 9 ×6, 18 ×6, 2 ×2, 4 ×2, 13 ×2, and 3, 5, 15, 17, 19, 20 once each. Grenade effects use 32–37 and 134–139 (grenades.md).

#### Applying a preset (hypothesis, Q16)

- One room preset is active per listener (`dsp_room`, set by the soundscape's "dsp" or the cvar). A player preset (`dsp_player`) can sit on top (grenade effects); timed presets revert after their duration with their fade.
- Each playing sound is split into dry and wet. Wet amount = the preset's mix, interpolated by distance between mix min and mix max (reduced by `db mixdrop` for room presets when the sound's level is below `db min`). Sounds whose wave name starts with `#` get no wet part. Sounds with sound level 0 (`SNDLVL_NONE`) and unspatialized sounds: *inferred* dry.
- The wet sum of all sounds runs through the preset's processor chain once (a shared room reverb, not per sound), and is added to the dry mix scaled by `dsp_volume`.
- A first implementation can approximate RVA as a bank of N parallel feedback comb filters with delays spread between min and max size (ms), feedback = the feedback scale, a one-pole lowpass at the cutoff in each loop, then the output gain; DFR as N series all-pass filters of 13–41 ms × size with the given feedback; DLY as one feedback comb (lowpass in the loop for LOWPASS); LFO as amplitude modulation (ring) by a sine at the rate, mixed at the gain; FLT as cascaded biquads (quality + 1 stages, *inferred*); AMP as gain then soft clipping above the threshold mixed by distortion mix.

### 8. Distance-variant waves (`^`)

- CS:S marks six waves with `^`: `weapons/hegrenade/explode3.wav`, `explode4.wav`, `explode5.wav` (HE), `weapons/flashbang/flashbang_explode1.wav`, `flashbang_explode2.wav`, and `weapons/smokegrenade/sg_explode.wav`.
- The header defines the prefix as "a stereo file whose left channel is the near version and right channel the far version". Checked on the files: the HE and flashbang waves are stereo (HE 44.1 kHz, about 2.9 s; flashbang 22.05 kHz, about 2.2 s); their right channel is quieter and shorter (HE: overall level about 57 % of the left; the right falls to about half within 0.2 s and below 5 % of its peak by 1.1 s, while the left stays near full for about 0.6 s) and, for the flashbang, much duller (far less high-frequency energy). The smoke wave is **mono**, so the prefix changes nothing for it: play it as mono.
- **The blend curve is engine-side and not in the reference.** Hypothesis H3 (low confidence): the near weight goes linearly from 1 at the listener distance `dsp_dist_min` to 0 at `dsp_dist_max` (the same distance range the DSP mix uses), far weight = 1 − near; output = near·L + far·R as a mono source, then spatialized normally. Hypothesis H4: the blend uses the sound's attenuation (the fraction of its audible radius), e.g. near weight = the distance gain. Q17 says how to tell them apart.

## Per-tick order

Within one player movement tick (sound-relevant steps only; see movement.md for the rest):

1. Count down the movement timers, including the swim timer S (−1000·dt ms each).
2. Categorize position (ground, water level). Record the start water level. If airborne, record fall speed = −v_z.
3. **Footstep check** (timer, bands, cases; may play a step).
4. Ducking, then ladder detection.
5. Ladder mode: ladder move (no water-transition sound). Otherwise walking/swimming mode:
   - In water at level ≥ 2 with jump held: **swim stroke** check. On the ground with jump pressed: **jump step** sound.
   - Slide move(s): **slam** check after each.
   - Re-categorize. **Landing** check (tiers, step timer 400, landing step).
   - If the water level crossed 0 during the tick: **"Player.Swim"** (+ server splash).
6. Physics simulation (server frame): impacts and breaks are queued during the simulation, scrapes are started or updated as friction is reported. After the simulation, stale scrapes (> 0.1 s) are stopped, then queued impacts and breaks play.
7. Server frame end: soundscape selection for the next players in the round-robin.
8. Client frame: other players' footstep checks (frame time), soundscape fades and random sounds.

## Edge cases

- **Step side flips even when the timer case returns without sound?** No. The side flips only inside "play a footstep", after the entry exists. The wading skip tick and missing surfaces don't flip it. Jump and landing sounds do.
- **Surface with no stepleft/stepright:** every surface inherits Default.Step* from "default", so this only happens with a deliberately cleared key.
- **Ladder while ducked:** volume 0.5 × 0.65 = 0.325, period 450 ms, bands (60, 80).
- **Speed vs ground speed:** the band test uses 3D speed, but motion must also be horizontal (> 0.0001). Walking up a slope counts the vertical part in the speed.
- **Fall speed is from the start of the landing tick**, not the impact speed after gravity on that tick.
- **Landing in shallow water** (level 1) always uses volume 0.5 and skips the damage tiers. The landing sound uses the recorded ground surface (the floor under the water), not "water". Only the timed footsteps switch to the water surfaces.
- **ambient_generic with a fade-in at map start:** the starting volume is vol_start (often 0). A sound with volume 0 is not sent at activation, but ramping still starts 0.1 s later and sends "change volume" updates for a sound that was never started. What the engine does with those is engine-side; treat it as starting the sound at the ramped volume (Open question 11).
- **Physics impact lookup failure** stops the remaining queued impacts of that frame. Same for break sounds.
- **Entry vs raw file:** anything containing ".wav" or ".mp3" anywhere in the name is treated as a raw file, not a script name.
- **ambient_generic with "!" sentences:** sentence lookup; not precached as an entry.
- **Soundscape positional loops at changed positions** are restarted, not moved.
- **Soundscape random sound with no "soundlevel"** plays positional sounds at level 0 (no falloff). CS:S scripts set "SNDLVL_140db" on "position random" sounds, which under any curve is near-full volume at 36 units.
- **Soundscape "volume" omitted** in playlooping or playrandom: silent (default 0).
- **Pitch outside 0–255 in scripts:** stored in 8 bits. Behaviour for > 255 is undefined; CS:S data stays within range.

## Quirks

- **Script volume ignored for footsteps** (keep): footstep loudness comes only from the movement rules (0.2/0.5/…), not the step entries' "volume". CS:S's Wade entries say 0.75, but the actual wade steps use 0.65.
- **Wading skip uses one counter for everyone** (keep for single-player parity, harmless otherwise): one of every four wade checks is skipped for a tick, and the counter is shared between all players (and between the client's prediction and its rendering of others). Effect: a one-tick (or one-frame) delay on some wade steps.
- **Single-wave step entries freeze their random pitch** (keep): their resolved parameters are cached per foot.
- **Landing 0.5 and 0 tiers are unreachable on solid ground** (keep): the 350 gate is above both thresholds.
- **ambient_generic preset ignores "health"** (keep if presets are implemented).
- **ambient_generic toggled on by input uses the script entry's volume and pitch**, while the map-start play uses the entity's (keep): the first start sends change flags, later starts don't.
- **Physics impact speed on client-only objects is passed squared** to the soft/hard velocity test: the client compares s² against audioHardMinVelocity. Keep for client-side debris if we simulate it.
- **Scrape smooth/rough handle mix-up**: the smooth choice reuses the rough sound's cached lookup handle. If the handle is already valid, the smooth scrape may resolve to the rough entry. Unverified in practice (Open question 8); an implementation can ignore it.

## Test cases

64 tick (dt = 0.015625 s) unless stated.

| Setup | Input | After | Expected |
|---|---|---|---|
| Parse "pitch" "95, 105" | – | – | start 95, range 10; draws uniform in [95, 105] then truncated to an integer |
| Parse "time" "15,40``" | – | – | start 15, range 25 |
| Parse "pitch" "PITCH_LOW" | – | – | 95, range 0 |
| Parse "soundlevel" "SNDLVL_97dB" | – | – | 97 |
| Parse "soundlevel" "SNDLVL_140db" | – | – | 140 |
| Parse "soundlevel" "SNDLVL_200dB" | – | – | 75 (out of 1–180, unknown name) |
| Parse "channel" "chan_voice" | – | – | 2 |
| Parse "channel" "6" | – | – | 6 |
| Parse "channel" "CHAN_FOO" | – | – | 0 |
| attenuation → level | 0.8 / 1.0 / 0.52 / 0.28 / 0 | – | 75 / 70 / 88 / 121 / 0 |
| level → attenuation | 75 / 140 / 60 / 50 / 40 | – | 0.8 / 0.22222 / 2.0 / 4.0 / 4.0 |
| MP attenuation filter cut-off | levels 75 / 88 / 140 / 45 | – | listeners beyond 2500 / 3800 / 9000 / 500 units are not sent the sound |
| ambient radius → level | r = 36 / 360 / 1250 / 3600 / 10000; play-everywhere flag | spawn | 40 / 60 / 70 / 80 / 88; 0 |
| Standing on concrete (gamematerial C), on ground, step timer 0, start-of-tick speed 250 | hold forward | this tick | right step ("Concrete.StepRight"), volume 0.5, timer 300 |
| Same, keep speed 250 | – | 20 ticks later | left step, volume 0.5; next at +20 ticks |
| Same at 66 tick | – | – | steps every 20 ticks (0.30303 s) |
| Speed 200 on concrete | – | – | volume 0.2, steps every 26 ticks (0.40625 s); 27 ticks at 66 tick |
| Speed 89.9 standing | – | any | no step |
| Speed 90.0 standing | timer 0 | this tick | step, volume 0.2 (walking), timer 400 |
| Speed 219.99 / 220 | – | – | walking (0.2, 400 ms) / running (0.5, 300 ms) |
| Ducked, speed 85, concrete | timer 0 | this tick | volume 0.5 × 0.65 = 0.325, timer 400 → 26 ticks |
| Ducked, speed 70, concrete | timer 0 | this tick | volume 0.2 × 0.65 = 0.13, timer 500 → 32 ticks (0.5 s) |
| Ducked, speed 59 | – | – | no step |
| Dirt (D), speed 250 / 150 | timer 0 | – | 0.55 / 0.25 |
| Vent (V), speed 250 / 150 | timer 0 | – | 0.7 / 0.4 |
| On ladder, climbing 200, not ducked | timer 0 | this tick | "Ladder.Step*", volume 0.5, timer 450 → 29 ticks |
| Feet in water (level 1), knee dry, speed 250 | timer 0 | – | "Water.Step*", 0.5, timer 300 |
| Knee in water, speed 250, shared wade counter 0 | timer 0 | tick 1 | no sound, counter 1, timer stays 0 |
| Same | – | tick 2 | "Wade.Step*", volume 0.65, timer 600; counter 2 |
| Same, keep moving | – | – | later wade steps at +39, +39, +40, +39, +39, +40, … ticks (every third gap includes the skip tick) |
| sv_footsteps 0 | any movement | – | no step, jump or landing step sounds (MP) |
| Airborne at any speed | – | – | no timed step |
| Jump from concrete | press jump | jump tick | step at volume 1.0 on concrete, side flips, timer unchanged |
| Land on concrete, fall speed 349 | – | landing tick | no sound; fall speed cleared |
| Land on concrete, fall speed 350 | – | landing tick | step at 0.85, timer 400 |
| Land on concrete, fall speed 600 | – | landing tick | step at 1.0; fall damage; "Player.FallDamage" if damage > 0 |
| Land on a floating object, fall speed 400 | – | landing tick | adjusted 200 → volume 0.5 |
| Land on a floating object, fall speed 380 | – | landing tick | adjusted 180 → volume 0 (silent), timer unchanged |
| Land with feet in water (level 1), fall speed 700 | – | landing tick | volume 0.5, no damage |
| Slide move loses 600 u/s horizontally | – | that move | step at 0.85, timer 400 |
| Slide move loses 1200 u/s | – | that move | step at 1.0 |
| Water level 0 → 1 during a tick | – | end of tick | "Player.Swim" once |
| Water level ≥ 2, holding jump, swim timer 0 | hold jump | tick 0, 64, 128 … | "Player.Swim" on each |
| Physics impact s = 69.9 | – | – | none |
| Physics impact s = 70, ≥ 0.05 s since last | – | – | queued, v = 0.0478515625 |
| Physics impact s = 160 | – | – | v = 0.25 |
| Physics impact s = 400 | – | – | v = 1.0 |
| Wood crate (hardness threshold 0.5) hits player (hardness 0) at 300 | – | – | soft entry |
| Two impacts in one frame, same material, v 0.3 then 0.5 | – | frame end | one sound, volume = script × min(1, 0.8), at the second impact's position |
| Six impacts of six different materials in one frame | – | frame end | 5 sounds; the 6th merged into the 5th queued entry |
| Scrape E = 1000 | – | – | nothing (e² = 0.00416 < 1/128) |
| Scrape E = 7750, rough entry with script volume 0.5 | no scrape playing | – | e² = 0.25; 0.5 × 0.25 = 0.125 > 0.1 → starts at volume 0.125 |
| Scrape playing, no friction reports | – | 0.1 s later | stopped |
| Soundscape loop "volume" ".45" just selected | – | 0.5 s / 1.35 s | volume 0.05 + 0.5/3 = 0.2167 / 0.45 (positional); 0.1667 / 0.45 (ambient, starts at 0) |
| playrandom "time" "10,20" selected at t = 0 | – | – | first play in [5, 10] s; then every [10, 20] s |
| env_soundscape radius 128, player ear 100 units away, clear line, no current | – | next update | becomes current; OnPlay fires |
| Player moves to 200 units away | – | update | stays current (not in range), until another one qualifies |
| ambient_generic fadeinsecs 2 (rate 2560), health 10, volstart 0 | PlaySound at t = 0 | steps at t = 0.1, 0.3, … | starts at volume 0; after step k volume = 0.1k; 1.0 at step 10 (t = 1.9 s); step 11 ends the fade |
| ambient_generic Volume input 5.5 / 0.55 / 12 | – | – | emitted volume 0.55 / 0.06 (round(5.5) = 6) / 1.0 (clamped) |
| ambient_generic Pitch input 300 / −5 | – | – | pitch 255 / 0 |
| ambient_generic spinup 1, pitchstart 50, pitch 100 | PlaySound | steps | pitch 50 at start, +25 per 0.2 s step: 75, 100; the third step clamps at 100 and ends the spin-up |

## Mapping to our engine

What an implementation needs:

- **Decoders:**
  - **PCM WAV**: format tag 1, 8-bit unsigned and 16-bit signed, mono and stereo, at 11025, 22050 and 44100 Hz. Of 1,907 scanned files from CS:S's and HL2's weapons/player/ambient/physics/radio/items/buttons/doors folders: 1,631 are 16-bit, 270 8-bit; 1,709 mono, 198 stereo; 899 at 22050 Hz, 712 at 44100, 296 at 11025. Bot/hostage voice files (534 scanned) are mostly 8-bit 22050 Hz mono.
  - **Microsoft ADPCM WAV**: format tag 2, 4-bit. Rare, but CS:S's own `sound/ambient/weather/drip_loop1.wav` uses it, plus a few HL2 files.
  - **MP3**: CS:S's own VPK has no MP3s. HL2's shared sound VPK has 53–54 (music and a few ambients), and maps may reference them. Low priority.
  - No WAV in the scan used MP3-in-WAV (tag 0x55). Re-run the scan over all of `sound/` before relying on that.
  - Check what the pinned Bevy 0.19.1 audio backend decodes (8-bit PCM, MS-ADPCM, MP3 feature flags). Decode ourselves where it doesn't; MS-ADPCM is small.
- **Looping:** looping ambient waves carry a RIFF "cue " chunk (one cue point), some a "smpl" loop chunk. Engine rule, unverified: a wave with a cue point loops from that sample offset to the end; one without plays once. ambient_generic's flag doesn't change this. Our player needs per-file loop points read from these chunks.
- **Other RIFF chunks** to skip: LIST, fact, PAD, DISP, ID3x, VDAT (lip-sync data on voice files), bext/minf/elmo/regn/ovwf/umid. Respect chunk padding (odd sizes are padded by one byte).
- **Path rules:** strip leading prefix characters (`* # ) ^ < > @ } ! ?`) before resolving `sound/<path>` through the VPK/folder search path. Treat the prefix meanings as optional features: for a first version, `)` and `^` can play as plain stereo.
- **KeyValues parser** for scripts, tolerant of: `//` comments; duplicate keys (multiple "wave", "file", "playlooping"); case-insensitive keys and names; unknown keys (ignored); trailing garbage in numbers; the misspelled manifest root ("soundscaples_manifest" in CS:S data; the root name is ignored).
- **Data loaded at startup:** the game_sounds manifest (CS:S list above) plus per-map `maps/<map>_level_sounds.txt`, the surface property manifest (two files), the soundscape manifest plus the per-map file.
- **Entry resolution:** random wave, interval draws, the raw-file fallback, channels with replace-on-same-channel.
- **Pitch** as playback-rate change (rate × p/100). Volume as linear gain.
- **Distance model:** until measured, use H1 for sound levels and H2 for CompatibilityAttenuation entries, behind one function so measured curves can replace them. Keep "level 0 = everywhere" and "ambient = unspatialized".
- **Gameplay hooks:**
  - The movement tick exposes start-of-tick velocity, ground state, recorded surface, duck/ladder/water state and the step side, step timer and swim timer as player state.
  - The landing and slam results feed the same footstep player.
  - The physics layer (avian3d) reports collision speed and time-since-last-contact per pair, for impact sounds and a per-frame queue, and some friction measure for scrapes. avian3d's contact data differs from vphysics' energy, so the scrape thresholds need re-tuning (Open question 8).
- **Map entities:** ambient_generic and env_soundscape(_proxy/_triggerable) + trigger_soundscape from the BSP entity lump. Soundscape selection runs per local player (we can do it every frame without the trace budget).
- **Not needed for a first slice:** sentences ("!"), gender macros, DSP presets (no-op, or a simple reverb per preset later), close captions, sound mixers.

## CS:S values (measured)

### Footsteps (2026-10-14, probe server)

The probe plugin's sound hook (`S` rows of `mashup_run`) on the bot running
straight on de_dust2's sand (-2000, -872) with the knife (max speed 250),
steady speeds set by forwardmove:
- No step below 150: 100, 130 (walking, +speed), 140, 145 and ducking (85)
  are silent; 150 steps.
- 150 to under 220 (150, 160, 200, 210): a step every 27 ticks (0.4 s) at
  volume 0.25 on sand; 220 and up (220, 225, 230, 240, 245, 250): every 20
  ticks (0.3 s) at 0.55. The band is picked at each step by the speed then.
- Channel 4 (CHAN_BODY), level 326 (as the scripts' 75 dB).
- A jump's takeoff plays a step at volume 1.0, standing, walking (130) or
  running; a duck + jump pressed together from standing played none.

### Hurt and death sounds (2026-10-14, probe server)

`tools/css_probe/hitmeas.py`'s shooter firing one USP or AK-47 shot at the
probe bot, `sv_soundemitter_trace 1`; the server's emitted entries per hit
(the client may add its own; not measured):
- Head (no helmet, armour or not): `Player.DamageHeadShot`; when the hit
  kills, `Player.DamageHeadShot` and then `Player.DeathHeadShot`.
- Chest/stomach with armour (the vest takes part): `Player.DamageKevlar`
  only, no `Flesh.BulletImpact`.
- Chest/stomach without armour, and legs with or without armour:
  `Flesh.BulletImpact` only.
- A lethal non-head hit: its hit sound above, then `Player.Death`.
- A helmeted head: `Player.DamageHelmet` only (USP, Glock, AK-47); a killing
  helmeted headshot not caught.
- The knife (measured 2026-10-17, `tools/css_probe/knifearmor.py`, the
  mashup_props sound hook; slash and stab from 40 units, aimed at the
  chest and at the head): its hits are bullet damage with hitgroup generic,
  so they sound as a bullet in the chest: with armour `player/kevlar1-5`
  (`Player.DamageKevlar`, CHAN_BODY, from the victim) only, aimed at the
  head too; without armour `physics/flesh/flesh_impact_bullet*`
  (`Flesh.BulletImpact`, from the victim, volume 0.69). The knife's own
  hit sound (`knife_hit1-4` for a slash, `knife_stab` for a stab) plays
  besides, about 7 ticks later in the log (the weapon's sound).

## Open questions

1. **Distance falloff (engine-side).** What is gain(d) for a sound level L, and for "CompatibilityAttenuation" entries? H1 and H2 above are hypotheses.
   - Measure on a dedicated server: the engine's sound interface for server plugins exposes the distance-gain query used by ambient_generic. A small server plugin can print gain for L ∈ {60, 70, 75, 80, 88, 100, 140} and also L + 256 (the compatibility range) at d = 0, 36, 100, 250, 500, 1000, 2000, 4000.
   - Cross-check by ear: record loopback audio of `playgamesound Weapon_AK47.Single` emitted by a bot at known distances.
   - Also: the stereo pan law, and whether sound passes through walls without occlusion.
2. **Answered (2026-10-18 for other entries: eight 30 kg crates dropped 14 times each on de_dust2, `mashup_sndlog`):** the hard-impact entry's three waves (`wood_panel_impact_hard1`, `wood_crate_impact_hard4`, `hard5`) came 29, 22 and 29 times in a run of 80 with repeats (the panel wave four times in a row): a uniform pick, repeats allowed, as ours. The impacts are sent from the world (entity 0), not the prop. **Partly answered (2026-10-16, probe server, a bot running back and
   forth on dust2's sand, 109 footsteps logged):** footsteps alternate
   left and right entries (sand1/sand3 then sand2/sand4, every step), and
   within a foot's two waves the pick is random with repeats (the right
   foot: 2, 4, 2, 4, 4, 4, 4, 2, ...); counts 30/24/24/31; never the same
   wave twice in a row only because the feet alternate. As ours
   (`movement` `step_left`, a uniform pick). Other entries' picks
   unmeasured. Was: **Wave choice.** Is the rndwave pick uniform, or does it avoid repeating the previous wave? Is the "count" (number of waves) used anywhere else?
   - Measure: `sv_soundemitter_trace 1` (replicated cvar) prints every emit as "'<entry>' emitted as '<wave>'". Fire 200 shots of a 3-wave entry and look at the repeat statistics.
3. **Answered (2026-10-14, see "Footsteps" under CS:S values: no step below 150, 0.4 s at 0.25 under 220, 0.3 s at 0.55 above; the bands follow speed, so a weapon matters only through its speed). Feet in water measured 2026-10-18 (de_aztec's shallows, water 10 units deep, a running bot): `slosh` steps at 0.5 every 20 ticks (the "water" surface, as 7 above); entering or leaving the water plays `slosh` at 1.0 (Player.Swim) and the tick after a `player/footsteps/wadeN.wav` at 0.2. Still open: knee-deep wading (on inferno's pool the probe bot doesn't move; 2026-10-18: aztec's water is 15-16 deep over a strip (e.g. (256, 864), (704, 576); the knee point is 14.4 up), but the probe bot placed in it, or on the bank at (704, 466) running in, stayed frozen in place for 150 ticks while it walked normally on dry ground; only the entering `wadeN.wav` at 0.2 was heard).** Was: **CS:S footstep rules.** CS:S overrides the shared bands or rule: walking (+speed) is silent, and probably crouch-walking. What are the CS:S speed bands, volumes and periods? Is there a per-weapon effect (max speed 210–250)? Does CS:S keep the "knee in water" wading?
   - Measure on a dedicated server with sv_soundemitter_trace 1 and the movecmp harness: drive a bot or second client at fixed speeds (walk, run, duck, duck-walk, each weapon class), and log emit ticks and entry names. Volume isn't printed. Compare loudness by recording, or test whether the emit happens at all.
4. **Answered ("Footsteps": a jump's takeoff plays a step at volume 1.0; a duck and jump pressed together from standing played none).** Was: **CS:S jump sound.** CS:S's own jump code: does it still play a footstep at volume 1.0 on takeoff? (Same measurement.)
5. **"Player.Swim" doubling.** The server sends it to the whole PAS without prediction rules, and the predicting client also plays it locally. Does the owner hear it twice? (Trace on the client, with a listen client vs. dedicated.)
6. **Answered (2026-10-18, probe server, a bot running on de_train's gravel displacement at (-1400, 1400)):** gravel keeps its sounds: the steps played `player/footsteps/gravel1.wav` and `gravel2.wav` (volume 0.5 running, every 20 ticks), so the later `gravel` entry overrides only its friction. Was: **Surface property overrides.** When surfaceproperties_cs.txt redefines "gravel" with only "friction", does gravel keep its other properties (base/default and earlier sounds), or is it replaced?
   - Measure: walk on a gravel surface in a CS:S map with sv_soundemitter_trace 1 and see which step entry plays ("Gravel.StepLeft" exists in the physics sounds).
7. **CS:S bullet sounds.** Ricochet and "whizz-by" sounds, penetration exit impacts, flesh/headshot sounds ("Player.DamageHelmet", "Player.DamageKevlar", …) and the shot-sound path are CS:S-specific. Is the shot played client-side from the fire-bullets event as in the SDK template? Is impact grouping (300 units) used?
   - Measure with sv_soundemitter_trace on both sides while firing shotguns at walls, bodies, armored bodies and helmets.
8. **Partly measured (2026-10-18, `mashup_sndlog`, a 30 kg wood crate shoved along de_dust2's concrete at 30-400 units/s):** no scrape at 30, 40 or 50 units/s; `wood_crate_scrape_rough_loop1.wav` from 60 (volume 0.05 at 60, 0.12 at 70, 0.20 at 80, 0.25 at 90 to 400, but 0.06 once at 120: the start's volume scatters), started once (two sends on one tick) and stopped with a volume-0 send when the crate stops (11-32 ticks later); a long slide (400) fades it by about 0.025 a tick before the stop. The energy formula is still ours. Was: **Scrape energy.** The unit and formula of the physics engine's friction energy (likely mass × speed²-like). Do scrape waves need cue points to loop? Does the smooth/rough handle quirk show up in practice? Needed to retune for avian3d.
   - Implementation stand-in (docs/tech-debt.md): E = friction × normal force (N) × sliding speed (m/s) × 0.015 s / 0.0254² (friction work per tick in kg·in²/s²). To check it: with `sv_soundemitter_trace 1`, shove a known prop (e.g. a 30 kg wooden crate) across concrete at a few known speeds (`ent_fire`/`phys_pushscale` or a scripted push) and log whether and when `*.ScrapeRough`/`*.ScrapeSmooth` start and stop; a SourceMod hook on the friction callback, if reachable, would print E directly. Also note which scrape waves carry a cue/smpl chunk.
   - Which object sounds when two moving objects slide on each other (both, or one), and whether ragdolls scrape at all in CS:S (client-side ragdolls).
9. **Partly answered (2026-10-18, probe server, `mashup_sndlog` while a bot drew, fired, emptied and reloaded an AK-47):** the server emits none of the gun's own sounds (no deploy, shot, reload part or zoom sound reaches the server's sound hook: they are client-side); it emits the knife's draw (`weapons/knife/knife_deploy1.wav`, channel 1), `items/itempickup.wav` on receiving a weapon and the dry-fire click (`weapons/ClipEmpty_Rifle.wav`, channel 1) once per press of an empty gun (held 0.5 s and 6 s, one click each, the first 6 ticks after the last round). Their client-side timing needs the client. Was: **CS:S weapon sound timing.** Which SoundData keys CS:S weapons fill (the .ctx scripts weren't read). Are reload part sounds (Clipout/Clipin/BoltPull/Slideback) played by view-model animation events? Empty-click and zoom timing, deploy sound, knife hit/miss rules, and the burst modes (FAMAS/Glock).
   - Measure with sv_soundemitter_trace while doing each action, and note the tick of each emit relative to the button press.
10. **Answered (2026-10-18, `tools/css_probe/entsweep.py` over the 20 stock and 89 cached community maps, 805 ambient_generics):** no stock map uses a preset (all 0 or absent); two community ambient_generics do (7 and 1). Every one writes radius, health and pitch: radius 1250 the most common (34 of 64 stock), health 10, pitch and pitchstart 100. Was: **ambient_generic presets.** Do any CS:S maps use "preset"? If so, transcribe the 27-row preset table into a later spec revision. Also the Hammer defaults for an unset radius, health and pitch (the FGD isn't in the SDK checkout; commonly radius 1250, health 10, pitch 100). Check the defaults against a map's entity lump, where Hammer writes them explicitly.
11. **ambient_generic sound level.** For a script-entry "message", is the radius-derived level used, or the entry's own level? The shared emitter forwards the entry's. For a raw ".wav" message, what level does the emitter return? Without a script entry the lookup may fail outright, unless the emitter fakes one.
    - Measure: two ambient_generics in a test map with the same wave and radii 100 and 5000; compare audibility at 1000 units.
12. **Channels and voice limits (engine-side).** Exact channel stealing when all mixing channels are busy, the maximum simultaneous sounds, and whether CHAN_STATIC sounds are ever stolen.
13. **Footstep cadence of remote players** is frame-rate based on the client. Is this what CS:S players hear, or does CS:S network footsteps differently?
14. **dsp_player and soundmixer.** What does a soundscape's "dsp_player" preset do to what the player hears (the same preset table as the room DSP, applied to every sound?), and what mixers do "soundmixer" names select (scripts/soundmixers.txt)? Which stock CS:S soundscapes set them? Listen in a map whose soundscape sets dsp_player, with `dsp_player` toggled from the console, and read `snd_soundmixer` while standing in each soundscape.
15. **OnPlay tried (2026-10-18, probe server):** a bot standing inside a spawned env_soundscape (radius 128, `Dust.Outside`) whose OnPlay told a point_servercommand to `say` fired nothing in 5 s (the servercommand said its line when sent the input directly): soundscapes are worked out for human clients only, so this needs one (class 2). **env_soundscape_proxy radius and OnPlay.** Does a proxy range with its own "radius" or its main soundscape's? Does env_soundscape_triggerable fire OnPlay when a trigger_soundscape applies it? Check with a test map: a proxy with a small radius next to a main with a large one; a triggerable with an OnPlay output.
16. **DSP processing (engine-side).** The values of `dsp_dist_min`, `dsp_dist_max`, `dsp_mix_min`, `dsp_mix_max`, `dsp_db_min`, `dsp_db_mixdrop`, `dsp_volume`, `dsp_automatic` and the `adsp_*` cvars (`help` in the client console); whether one shared wet chain serves all sounds; what automatic DSP (preset 1, the stock maps' second most used value) builds. Check by ear and by recording: loopback-record a fixed sound (`play weapons/ak47/ak47-1.wav` at a wall) with `dsp_room` 0, 2, 6, 9, 18, 20, 22 set from the console, and compare decay times and spectra with a model of section 7; for preset 1, record the same spot on a stock map with `dsp_automatic` 0 and 1.
17. **`^` near/far blend.** Record loopback audio of an HE grenade exploding at known distances from a still listener (bot throws, or `ent_create hegrenade_projectile` with a fuse, at 100, 250, 500, 1000, 1500, 2000, 3000 units on open ground), and fit the near/far weights by matching each recording to a·L + b·R of the wave (least squares on the first 1.5 s). Read `dsp_dist_min`/`dsp_dist_max` first (Q16) to test H3.
