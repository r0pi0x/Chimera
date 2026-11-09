# Half-Life 2: Deathmatch: effects and sounds (H9)

Source basis: the public Source SDK 2013 multiplayer game code — the
shared bullet-firing routine and its HL2:DM fire-bullets temp entity on
both sides, the shared and HL2:DM weapon bases (sound hooks, muzzle-flash
and tracer dispatch), the HL2:DM pistol, .357, SMG1, AR2, shotgun,
crowbar, stunstick, gravity gun, RPG, crossbow, frag and S.L.A.M.
weapons, the client tracer, whizz, muzzle-flash, shell, impact, blood,
water-splash, explosion, dissolve, combine-ball and crossbow-bolt effects,
the old-style client particle helpers, the rocket trail, the explosion
entity and grenade base, the dissolve entity on both sides, the HL2 and
HL2:DM player (damage effects, step, pain and death sounds), the
recipient filters and the temp-entity prediction suppression, the material
proxies used by the shield wall, the map compiler's material-patch rule,
and the `UnlitTwoTexture` DX9 shader source — read in place in a separate
spec session. From the game's files on this box (game build 24600434,
dedicated server build 24600426, `steam.inf` PatchVersion 10889068 in
both, checked): `hl2mp`'s `scripts/game_sounds_weapons.txt`,
`scripts/game_sounds_player.txt` and the twelve weapon scripts'
`SoundData` blocks (`hl2mp/hl2mp_pak_dir.vpk`), Half-Life 2's loose
`scripts/game_sounds_manifest.txt`, `game_sounds_items.txt`,
`game_sounds.txt`, `game_sounds_physics.txt`, `game_sounds_ui.txt`,
`npc_sounds_citizen.txt`, `npc_sounds_soldier.txt`,
`npc_sounds_metropolice.txt`, `npc_sounds_combine_ball.txt`, the
`.vmt`/`.vtf` headers of the effect materials named below, the events and
attachments of the view and world weapon models, `particles/blood_impact.pcf`
(compared byte for byte with CS:S's), and the BSP lumps of the 8 stock maps
(materials, faces, models, entities), read for names and numbers only; the
client and server libraries were searched for cvar names only. Nothing was
launched.
Status: draft

## Summary

HL2:DM's effects are Half-Life 2's old-style client effects driven by a
multiplayer shot pipeline that differs from CS:S's in ways players see.
The server never draws a shot: it tells every other client "this player
fired N bullets with seed S, with or without tracers and impacts", and
each client re-traces the shot **through all players** and draws a tracer
for every bullet of a flagged shot; the shooter predicts his own. Only the
shooter's own tracers carry the whizz flag, so **near-miss sounds are
heard only for one's own bullets** (from the view-model muzzle, within 24
units of the eye; the AR2's within 72 units with a deep, half-pitch
variant). First-person muzzle flashes are short bursts of 5 additive
sprites strung along the muzzle (blue-white for the AR2), with a 0.05 s
model-only light; other players' guns show the light only. Blood is the
same `blood_impact_red_01` system as CS:S, made by the server for everyone
but the shooter and predicted (4 units off, with distance-driven wall
splats) by the shooter. Explosions are CS:S's explosion temp entity with
HL2's 140 dB `BaseExplosionEffect.Sound`. The energy ball's kill turns
the body black over 1 s, floats it weightless and removes it at 2 s, in
silence. Sounds are script entries with sound levels (no compatibility
attenuation anywhere): guns at 140 dB (sent to others up to 9000 units),
footsteps at 75 dB per player-model type at the step's computed volume,
no pain voice at all, and a death voice per model type.

This spec continues [source_deltas.md](source_deltas.md) (H0: 1.9, 1.10,
1.11, Open questions 9), [movement.md](movement.md) (H1: step timing and
volumes), [suit.md](suit.md) (H2: suit, pickup and charger sound timing),
[weapons.md](weapons.md) (H3: the shot, the tracer counter, melee),
[projectiles.md](projectiles.md) (H4: projectile behaviour and its sound
timing), [gravity_gun.md](gravity_gun.md) (H5: the gun's states and
sounds) and [player_models.md](player_models.md) (H7: ragdolls, view
models). Engine behaviour HL2:DM shares is in
[sounds.md](../cs_source/sounds.md) (scripts, channels, levels, filters,
footstep machinery, physics sounds, ambients, soundscapes),
[impact_effects.md](../cs_source/impact_effects.md) (old-style particle
machinery, surface effects, blood system, splash),
[tracers.md](../cs_source/tracers.md) (streak drawing, whizz test),
[grenades.md](../cs_source/grenades.md) 5.4 (explosion temp entity),
[view_models.md](../cs_source/view_models.md) 6–8 (events, light,
shells), [shaders.md](../cs_source/shaders.md),
[particles_and_smoke.md](../source/particles_and_smoke.md) (§5
`env_smoketrail`) and [visual_entities.md](../source/visual_entities.md)
(§1 the beam primitive); this spec states only what differs or is new.

## Units and conventions

- Source units (inches), Z up, X forward, Y left; degrees; seconds;
  server tick 0.015 s. Client effects run per rendered frame with the
  frame time clamped to 0.1 s (impact_effects.md §4), not on ticks.
- `U(a, b)` uniform float, `I(a, b)` uniform integer inclusive, fresh
  draw each time. "f, r, u" of an attachment are its forward, right and up
  axes.
- **Sprite sizes.** Old-style particle sizes ("plain sprites",
  impact_effects.md §4) are **half-widths**; a size stored in a byte is
  truncated to an integer. "Glow quads" drawn around a point (the gravity
  gun's sprites, the RPG's dot and laser glow) are given as **full
  widths**. "Halos" (stunstick, energy ball, crossbow bolt) are given as
  **half-widths**. Flat quads (FX quads) are full side lengths and use
  impact_effects.md §4's bias rule.
- **Sound entries** are given as channel, volume, pitch, level (dB, the
  script's sound level), wave count. "Send radius" is the attenuation
  filter's cut, 100·(L − 50) units for L > 50 (sounds.md §2).
- Effect names in back-quotes (`AR2Tracer`, `BloodImpact`,
  `PhyscannonImpact` …) are the names the server and client use for
  dispatched effects (they travel in the effect messages); entity classes,
  cvars, sound entries, models and materials are the game's data.
- Steam builds read: game 24600434, dedicated server 24600426.

## Constants

### Shots, tracers and whizz

| Name (ours) | Value | Unit | Meaning |
|---|---|---|---|
| tracer_every | 2 (pistol, SMG1, AR2), 4 (.357, shotgun) | bullets | weapons.md §6 (one process-wide counter per process) |
| tracer_speed_default | 5000 | in/s | `Tracer` streak (tracers.md) |
| tracer_speed_ar2 | 8000 | in/s | `AR2Tracer` streak |
| ar2_tracer_min | 128 | in | shorter shots draw no AR2 streak (default tracer: 256) |
| ar2_tracer_length | U(128, 256) | in | AR2 streak length (default 64–128) |
| ar2_tracer_half_width | U(0.5, 1.5) | in | AR2 streak half-width (default 0.75–0.9) |
| ar2_tracer_material | `effects/gunshiptracer` | – | Sprite shader, 32 × 64, render mode 8, parallel orientation |
| whiz_default | `Bullets.DefaultNearmiss`, 24 in, listener drop 24 in | – | tracers.md §5 |
| whiz_ar2 | `Bullets.GunshipNearmiss`, 72 in, no listener drop | – | AR2 tracers |
| whiz_interval | 0.1 | s | min = max, one timer per client for all kinds |
| splash_scale | U(4, 8) | – | bullet water splash scale (every HL2:DM ammo type; template 8–12) |

### Muzzle flashes (first person, view model)

| Kind (event option) | Weapons | Sprites | Spacing along f | Half-size of sprite i (i = 1..5) | Life | Material | Extra |
|---|---|---|---|---|---|---|---|
| PISTOL | pistol | 5 | 4·k·i, k = U(1, 1.25) | ⌊U(6, 8)·(8 − i)/6·k⌋ | 0.01 s | `effects/muzzleflash1..4_noz` (random each) | smoke 3 times in 4 |
| 357 | .357 | 5 | 8·k·i, k = U(1.25, 1.5) | ⌊U(6, 8)·(8 − i)/6·k⌋ | 0.01 s | same | smoke always |
| SMG1 | SMG1 | 5 | 8·k·i, k = U(1.25, 1.5) | ⌊U(6, 8)·(8 − i)/6·k⌋ | 0.025 s | same | follows the muzzle |
| SHOTGUN | shotgun (both shots) | 5 | 8·k·i, k = U(1.25, 1.5) | ⌊U(6, 8)·(8 − i)/6·k⌋ | 0.0001 s | same | one frame |
| COMBINE | AR2 (primary and ball) | 5 + 1 core | 8·k·i, k = U(2, 2.25) | ⌊U(6, 8)·(12 − i)/12·k⌋; core ⌊U(10, 16)⌋ at the muzzle | 0.025 s | `effects/combinemuzzle1..2_noz` (random each) | follows the muzzle |
| (none) | RPG, crossbow, gravity gun, frag, S.L.A.M., melee | – | – | – | – | – | – |

Flash sprites: colour (255, 255, 200 + I(0, 55)), alpha 255 → 255, roll
I(0, 360) (radians, impact_effects.md conventions), no roll speed; the
AR2 core: colour white, alpha I(64, 128) → 32. All materials are
UnlitGeneric, additive, vertex colour and alpha (`_noz` ones ignore
depth; the combine ones read gamma-space colour, write linear), textures
256 × 256 (`muzzleflash`) and 128 × 128 (`combinemuzzle`).

| Name (ours) | Value | Unit | Meaning |
|---|---|---|---|
| flash_light_radius | I(32, 64) | in | model-only light at attachment 1 (`muzzleflash_light` 1) |
| flash_light_colour | (255, 192, 64), exponent 5 | – | |
| flash_light_life | 0.05 | s | radius shrinks linearly to 0 |
| pistol_smoke | at muzzle + 8f; v = f·U(48, 64) + (0, 0, U(4, 16)); life U(0.25, 0.5); grey I(200, 255); alpha I(64, 128) → 0; size I(2, 4) → ×4; roll speed U(−0.1, 0.1) | – | `particle/particle_smokegrenade` (its own `$alpha` 0.27) |
| magnum_smoke | as pistol but v = f·U(8, 64) + (0, 0, U(4, 16)); life U(0.5, 1); size → ×8; roll speed U(−0.5, 0.5) | – | |

### Shells (first person)

| Weapon | Event | Model | Bounce sound |
|---|---|---|---|
| SMG1 | each fire sequence, cycle 0, type 0 | `models/weapons/shell.mdl` | `Bounce.Shell` |
| shotgun | pump sequence, cycle 0.312, type 2 | `models/weapons/shotgun_shell.mdl` | `Bounce.ShotgunShell` |
| pistol, .357, AR2, others | none | – | – |

Velocity f·U(150, 200) + (U(−64, 64), U(−64, 64), U(0, 64)) world axes
(f of the view model's attachment 2); spin U(−1024, 1024) deg/s on each
axis; orientation the gun's angles; life 1 + U(0, 1) s then fade; full
gravity; collides with the world. Only when the view model is within 256
units of the camera (always in first person) and `cl_ejectbrass` 1.

### Impacts and blood

| Name (ours) | Value | Unit | Meaning |
|---|---|---|---|
| ar2_impact_quad | full side U(24, 32) → 0 (bias 0.75); alpha 1 → 0 (bias 0.4); life 0.25 s; random yaw I(0, 360), no turning; at hit + 1·n facing n | – | `effects/combinemuzzle2_nocull` (additive) |
| stunstick_impact_quad | full side s → 2s, s = U(16, 32); alpha 1 → 0; life 0.1 s; at hit + 4·n | – | `sprites/light_glow02_add`, plus sparks |
| blood_server_point | the hit point | – | server's copy (player rule) |
| blood_shooter_point | hit − 4·d | – | shooter's predicted copy |
| blood_amount | min(255, 5 × damage) | – | carried, unused by the red system |
| bleed_trace | 172 | in | wall-blood traces behind the victim (impact_effects.md §10) |

### Explosions

| Source | Temp-entity origin | Scale (core drawn when ≠ 0) | Frame rate | Radius | Magnitude (probe length) | Flags | Server sound |
|---|---|---|---|---|---|---|---|
| frag | grenade after its ground probe (H4 9.6) | 7.5 | 25 | 250 | 150 | none; surface normal and material sent | `BaseGrenade.Explode` |
| SMG1 grenade | its origin | 2.0 | 15 | 250 | 100 | none | none |
| shot satchel (pop) | grenade origin | 0 (no core) | 25 | 0 | 150 | none | `BaseGrenade.Explode` |
| RPG missile | see "explosion entity" below | 5.0 | 15 | 300 | 150 | no dynamic light | none |
| satchel (detonated) | satchel + 16 Z, then the entity rule | 5.0 | 15 | 200 | 150 | no dynamic light | none |
| trip mine | mine + 8 along its normal, then the entity rule | 5.0 | 15 | 200 | 200 | no dynamic light | none |
| map `env_explosion` | entity rule | ⌊clamp(0.6·(m − 50), 10, 50)⌋/10 (flags 2048/4096 relax the clamps to 1 / 200) | 15 | `iRadiusOverride` or 2.5·m | m (`iMagnitude`) | by spawnflags | none |

### Projectile visuals

| Name (ours) | Value | Meaning |
|---|---|---|
| rpg_beam_1st | view-model attachments 2 (`laser`) → 3 (`laser_end`, 100 units further), width 2 → 2, colour (255, 0, 0), brightness U(128, 200) per frame, fade length 128 × (trace fraction of a 128-unit line), 4 segments, `effects/laser1_noz` | the laser on the gun |
| rpg_beam_3rd | world-model attachments 4 → 5, width 1, `effects/laser1` | (w_rocket_launcher has only 3 attachments: Open questions 6) |
| rpg_glow | `sprites/redglow1` glow quad, full width 8 + U(−2, 2), at the beam start | |
| laser_dot | `sprites/redglow1` glow quad, full width 16 + U(−4, 4), at (view trace hit + 4·n) | colour white, alpha 255 |
| rocket_flare | 8 plain sprites behind the missile at 3·i units (i = 1..8), half-size ⌊U(5, 6)·(12 − i)/9·k⌋, k = U(1, 2); life 0.01 s; alpha 255 → 128; `effects/muzzleflash1..4` | each frame while flying |
| rocket_smoke | one puff per 4 units travelled (≤ 50 per frame); life 0.5 + U(0.45, 0.55) s; v = random dir × U(2, 16); grey 0.65·U(0.75, 1.25) (clamped); size 8·U(0.75, 1.25) → 32·U(1, 1.25); alpha 255·clamp(0.2·U(0.75, 1.25), 0, 1) × sin(π·age/life); roll I(0, 360), roll speed U(−8, 8) decaying ×(1 − 8·dt), ≥ 0.5 | `particle/particle_smokegrenade` or `particle/particle_noisesphere` (random each) |
| smg_grenade_trail | `env_smoketrail` (particles_and_smoke.md §5): spawn rate 48/s, life 1 s, start colour (0.1, 0.1, 0.1) → black, size 12 → 48, spawn radius 4, speed 4–24, opacity 0.2, stops after 10 s; only with `smoke_trail` 1 | |
| ball_body | camera-facing quad of half-size r + sin(25·t) (r = 10), rolled by its spawn time (radians), `effects/ar2_altfire1` (27-frame animated) | not held |
| ball_flicker | halo half-size r·U(1.5, 2.5), grey U(0.2, 0.3), `effects/combinemuzzle1` | every frame |
| ball_blur | 8 halos of half-size r stepping back along the motion by min(0.5·s, 4) each (s = frame displacement, ≤ 32), grey remap(s; 4 → 0, 32 → 1)·(1 − i/12), `effects/ar2_altfire1b` | held or launched |
| ball_trail | `sprites/combineball_trail_black_1` sprite trail, additive with colour (0, 0, 0), width 10 → 0, life 0.1 s | draws nothing (Quirks) |
| ball_bounce (`cball_bounce`) | quad 160 → 0 (r·10; bias 0.75), alpha 1 → 0 (bias 0.4), 0.25 s, `effects/combinemuzzle1_nocull`; quad 32 → 64 (bias 0.75), alpha 1 → 0 (bias 0.4), 0.5 s, `effects/combinemuzzle2_nocull`; electric sparks (size 2, length 1) along n; at hit + 1·n with radius sent as 16 | |
| ball_explode | electric sparks (size 4) straight up; two rings from radius 10 to 1024: 0.2 s, width 64, colour (255, 255, 225), alpha 32, and 0.5 s, width 64, alpha 64, both fading, `sprites/lgtning`, frame rate 2, to every client | a ball out of bounces instead: one ring 128 → 384, 0.25 s, width 48, alpha 64 |
| bolt_blur | 20 halos of half-size 3 starting 24 units ahead along the motion and stepping back min(0.5·s, 4) (s = frame displacement ≤ 32); grey 0.25 for i < 5, ramping to 0.05 at i = 11, 0.05 after; `effects/muzzleflash1` | while it moves |
| bolt_stuck_glow | `sprites/light_glow02_noz`, glow mode, colour (255, 255, 255), alpha 128, scale 0.2, fading out over 3 s | only when stuck in the world |
| xbow_charger | view-model attachment 2: `sprites/light_glow02_noz` additive (255, 128, 0); load: brightness 32 and scale 0.025 over 0.5 s; ready: brightness 80 over 1.0 s, scale 0.1 over 0.5 s; fire: off | |
| xbow_load_flash | `effects/blueflare1` additive white, brightness 128, scale 0.2, fading from spawn, at view-model attachment 1; electric sparks (size 1) at attachment 1 | at the start of a load |

### Dissolve

| Name (ours) | Value | Unit | Meaning |
|---|---|---|---|
| dissolve_fade_in | 0 → 1 | s | body colour white → black |
| dissolve_model_fade | 1.9 → 2.0 | s | body alpha 255 → 0 |
| dissolve_remove | 2.0 | s | the dissolving entity is removed (first tick at or after: 134 ticks) |
| dissolve_lift | 1.02 × gravity upward | in/s² | net +12 in/s² up at gravity 600 |
| dissolve_speed_cap | 50, at most −40 per physics step | in/s | |
| dissolve_sparks | per hitbox per frame 2 tries at 1/3: `effects/spark`, v (U(−4, 4), U(−4, 4), U(16, 64)), half-size U(4, 6), life U(0.4, 0.5), alpha 255 → 0, size → 0, roll speed U(−8, 8) | – | |
| dissolve_glows | per hitbox per frame ⌊3·p⌋ (p the fade progress): `effects/combinemuzzle2`, v (U(−4, 4), U(−4, 4), U(−64, 128)), half-size U(8, 12), life 0.1, roll speed U(−2, 2) | – | |

### Gravity gun sprites and beams

| Element | First person (view model) | Third person (world model) |
|---|---|---|
| core (`sprites/orangecore1`, attachment 1) | ready: width → 14, alpha → 128 (0.2 s); holding: width → 16 (0.2 s), alpha → 255 (0.1 s) | ready: width → 8, alpha → 0 (0.2 s); holding: width → 14, alpha → 255 (0.1 s) |
| blast (`sprites/orangecore2`, attachment 1) | on a punt or launch: width 8 → 64 in 0.1 s, alpha 255 → 0 in 0.2 s | same |
| glows ×6 (`sprites/glow04_noz`, colour (255, 128, 0)) | `fork1b`, `fork1m`, `fork1t`, `fork2b`, `fork2m`, `fork2t`; every frame width U(6.4, 9.6), alpha I(24, 32) | `fork1m`, `fork1t`, `fork2m`, `fork2t`, `fork3m`, `fork3t`; same |
| end caps (`sprites/orangeflare1`) | holding only: `fork1t`, `fork2t`; every frame width U(3, 5), alpha I(200, 255) | holding only: `fork1t`, `fork2t`, `fork3t` |
| beams (`sprites/orangelight1_noz` / `orangelight1`) | holding: 2, `fork1t`/`fork2t` → attachment 1, width 0 → 4 | holding: 3, `fork1t`/`fork2t`/`fork3t` → attachment 1, width 0.5 → 2 |

Beams: white, brightness 255 while visible (0 when hidden), noise
amplitude 16, 8 segments, scroll speed 150, frame rate 30, never expire
(visual_entities.md §1). Sprite colours other than the glows' are white.

### Shield wall shader (dm_underpass)

| Name (ours) | Value | Meaning |
|---|---|---|
| shield_base | `effects/combineshield/comshieldwall` | 256 × 256, DXT1, **31 frames** |
| shield_tex2 | `effects/combineshield/comshieldwall_close` | 256 × 256, DXT1, 1 frame |
| shield_frame | clamp(0.1·d − 22, 0, 30) → frame | d = local player centre to the brush entity's centre |
| shield_scroll | texture 2 scrolls 0.1 texture widths per second at 25° | |

## Corrections to H0

1. **1.10, whizz.** "The pistol, SMG1, AR2 and .357 ammo types use 'line
   and whizz' tracers, and `Bullets.DefaultNearmiss` is defined": true of
   the data, but in HL2:DM no client ever hears a near-miss from someone
   else's bullet. The shooter's own predicted tracers carry the whizz
   flag (for every ammo type, "line" and "line and whizz" alike) and are
   tested against the shooter's own eye; tracers drawn for other players'
   shots carry no whizz flag (§3). The AR2's tracers use
   `Bullets.GunshipNearmiss` (pitch 50) within 72 units instead.
2. **1.10, "old-style surface effects apply unchanged".** Three
   differences: HL2's impact has **no ricochet sound** (CS:S's template
   plays `Bounce.Shrapnel` 30 % of the time), shotgun pellets' impact
   sounds are **not grouped** (each pellet plays its own), and bullet water
   splashes use scale U(4, 8), not U(8, 12) (§6). And other players' shots
   are re-traced through all players (§1).
3. **1.9, "citizens' [death sound] to be found".** It is
   `NPC_Citizen.die`, defined twice: `hl2mp`'s
   `scripts/game_sounds_player.txt` (3 waves, `vo/npc/<gender>01/pain07`
   to `09`) and HL2's `npc_sounds_citizen.txt` (9 waves, `pain01` to
   `09`). The `hl2mp` file is earlier in the manifest; which definition
   wins is engine-side (Open questions 2; assumed the first, 3 waves).
4. **1.9, "on the body channel at the step's volume"**: and the entry's
   own volume (0.4 citizen, 0.75 Combine, 0.55 police) is ignored (§15.6).
5. **Open question 9 (HDR)** is answered for H9 by a decision, not a
   measurement: H9 specs the LDR path only (§14).

### Corrections to wave 1/2

1. **gravity_gun.md §10, launch effect** ("a beam from the muzzle to the
   point and sparks; the punter's own copy is predicted"): only the punter
   sees the beam and the sparks at the point. Every other client in the
   PVS of the point sees only a burst of sparks **at the gun's muzzle**,
   pointing at the point (§11). The effect goes to the PVS of the point,
   not the PAS.
2. **weapons.md §11, crowbar sound list**: `Weapon_Crowbar.Melee_Hit` and
   `Melee_HitWorld` are never played by a player's crowbar (only NPC
   swings use them); the stunstick's `Melee_Miss` and `Melee_HitWorld`
   are never played either (it plays `Melee_Hit` on every hit). Consistent
   with weapons.md's text; stated here because the scripts list them.
3. **weapons.md §12**: "other clients draw the shot's tracers and impacts
   … re-tracing it themselves": they re-trace **ignoring every player**
   (and view model), so on other screens the impact (decal, particles,
   sound) appears on whatever is behind the victim (§1, Quirks).
4. **player_models.md §10.3** ("dissolves the ragdoll (and mutes the death
   sound)"): confirmed; the mute works because the death sound is decided
   after the ragdoll has started dissolving in the same tick (§10).
5. **projectiles.md 5.5 / 7.3**: the energy ball's "black sprite trail"
   is drawn additively in black and so is invisible; the crossbow bolt's
   "glow sprite" is off in flight and only lights (then fades over 3 s)
   once the bolt sticks in the world. In flight the bolt shows a client
   motion blur (Constants).

## Behavior

### 1. The shot pipeline (who draws what)

For every bullet call (weapons.md §6):

- **On the server**: the bullets are traced and damage applied (blood,
  `Flesh.BulletImpact`, §7, §15.7); the tracer counter advances once per
  bullet; the server draws nothing. At the end of the call it sends one
  **fire-bullets temp entity** — shooter, shoot position (the trace start,
  the eye), the unspread aim direction, ammo type, seed (8 bits), bullet
  count, spread, "has tracers" (true if any bullet of the call got a
  tracer on the server's counter) and "has impacts" (true if any bullet
  hit something outside water) — to the PAS of the shoot position, **minus
  the shooter** (prediction suppression).
- **On each other client**: if the shooter's entity and its active weapon
  are known on that client, the bullets are regenerated from the seed
  exactly as the shooter did (weapons.md §6; odd bullets of a multi-bullet
  call are ±3 boxes), traced 56755.84 units with the "shot" mask
  **ignoring all players and view models**. For each bullet: if the shot
  has tracers, a tracer of the **receiving client's idea of the shooter's
  current weapon** (§2) from that weapon's attachment 1 to the trace end,
  with **no whizz flag**; if the shot has impacts, that weapon's impact
  effect (§6) at the trace end. Shots of a dormant or unknown shooter, or
  one without a weapon on this client, draw nothing.
- **On the shooter's client** (first prediction of the command only): the
  bullets run through the shared routine: its own tracer counter (one per
  client process; re-predicted commands are not re-drawn), tracers with
  the **whizz flag set**, impacts on what the predicted trace hit
  (players included), predicted blood (§7). Water: a bullet ending in
  water makes a splash at the entry point and no impact.
- **Not drawn by anyone**: impacts of bullets that ended under water on
  the server (the "has impacts" flag can still be true from another
  pellet; other clients' re-traces ignore water, so they draw the impact
  on the underwater surface: Quirks).

### 2. Tracers

- **Selection**: weapons.md §6 (every 2nd for pistol, SMG1, AR2; every
  4th for .357 and shotgun pellets; not when the bullet broke glass).
  Other clients: every bullet of a flagged shot (a shotgun shot whose
  server counter selected any pellet shows 7 or 12 tracers).
- **Kind**: the AR2 draws `AR2Tracer`; every other weapon the default
  `Tracer` (tracers.md §3.1).
- **Start**: attachment 1 of the view model when the weapon is drawn
  through it (the local player's own gun in first person), else attachment
  1 of the world model (`muzzle`; w_irifle's is 8.5 behind its bone). A
  dormant weapon gives the sent start (the eye). The own-player static
  line (tracers.md §3.2) is never used in HL2:DM (the effect names the
  weapon, not the player).
- **Default streak**: tracers.md §3.1 exactly.
- **AR2 streak**: as tracers.md §3.1 with: nothing if D < 128; L =
  U(128, 256); half-width U(0.5, 1.5); speed 8000 (no speed is ever sent);
  life (D + L)/8000; material `effects/gunshiptracer`.
- `r_drawtracers` (cheat, 1) and `r_drawtracers_firstperson` (archived, 1)
  as tracers.md §4: the first-person switch tests whether the effect's
  entity is a player that isn't drawn; the entity is always a weapon, so
  it never hides anything.

### 3. Whizz (near-miss) sounds

- Only tracers with the whizz flag: in HL2:DM, the **shooter's own
  predicted tracers** of every weapon (§1). Remote tracers never whizz.
- `Tracer`: tracers.md §5 (listener point = the main view origin, slid
  down at most 24 along the vertical; distance < 24 to the segment
  start → end; entry `Bullets.DefaultNearmiss`).
- `AR2Tracer`: listener point = the main view origin with **no** slide;
  distance < 72; entry `Bullets.GunshipNearmiss`.
- Both: not before the per-client next-whizz time; played to the local
  player only, from the world at the **tracer start** (the muzzle), with
  the shot direction, on the static channel, the entry's volume and pitch
  and the attenuation of its level (140 dB → 0.2222); next-whizz time =
  now + 0.1 s. The test runs whether or not a streak is drawn (short
  shots whizz too).
- Consequence: since the tracer starts at the view-model muzzle, a few
  units from the eye, a shooter's own tracer bullets usually pass the
  test: the shooter hears a near-miss at most every 0.1 s while firing
  (every SMG1 or AR2 tracer, every 2nd pistol shot, Open questions 1).

### 4. Muzzle flashes

- **What triggers them.** Firing increments a networked "muzzle flash"
  counter on the shooter's view model and on the weapon (pistol, .357,
  SMG1, AR2 primary and ball, shotgun both shots; also a weapon or item
  materialising on respawn, weapons.md §10). The RPG, crossbow, gravity
  gun, frag, S.L.A.M. and melee weapons don't.
- **Light**: when an animating entity sees its counter change and
  `muzzleflash_light` is 1 (default, archived), a model-only light at its
  attachment 1 (Constants; view_models.md §7's generic rule). First person:
  the view model's (`muzzle`). Others: the world weapon's attachment 1.
  Item models without attachments make none.
- **Sprites, first person only**: the fire sequences of the view models
  carry a muzzle-flash event at cycle 0 whose option names the kind and
  the attachment (`PISTOL MUZZLE`, `357 muzzle`, `SMG1 MUZZLE`, `COMBINE
  MUZZLE`, `SHOTGUN MUZZLE`); the RPG's fire sequence carries an old
  numbered event with no kind, which draws nothing. Each kind spawns the
  sprites of the Constants table along the attachment's forward axis,
  drawn before the view model (so the gun covers them) with depth testing
  off. SMG1 and COMBINE sprites live in the attachment's frame and follow
  it while alive; PISTOL, 357 and SHOTGUN sprites are placed in the world
  at creation.
- **Third person: no sprites.** The player animation model has no events,
  and the world weapon is bone-merged and plays no fire sequence, so
  other players' guns show only the light (Open questions 3).

### 5. Shells

First-person only, from view-model events (Constants table). Motion,
collision and fade as view_models.md §8's temp-entity rules, with HL2's
numbers: velocity and spin from Constants, full gravity (600), damping
0.5 per bounce. Bounce sound: for these casings it plays when the impact
|v_z| ≥ 200, otherwise 1 time in 4; volume = entry volume ×
min(1, |v_z|/350); the entry's base pitch; local player only. Other
players eject nothing.

### 6. Bullet and melee impacts

- **Surface effects**: impact_effects.md §2–§9 and §13 unchanged, with
  HL2's surface properties, except: **no ricochet sound**; each pellet's
  impact sound plays (no 300-unit grouping); `cl_new_impact_effects` 0
  and `r_drawflecks` 1 as there.
- **AR2 (`AR2Impact`)**: every AR2 bullet impact (own predicted and
  remote) first draws one flat quad (Constants) at hit + 1·n facing n,
  then the normal surface impact.
- **Water**: a bullet whose end is in water traces again with water; at
  the entry point a splash (impact_effects.md §11) with scale U(4, 8)
  (f = s/8 from 0.5 to 1). Slime always gives `slime_splash_03` (no stock
  map has slime). The splash of a server bullet goes to the PAS of the
  entry point minus the shooter; the shooter predicts his own.
- **Crowbar**: on a hit, the generic impact (decal and impact sound of the
  surface, as a bullet's, club damage); a swing whose trace starts out of
  water and ends in it makes a splash of scale 8 instead.
- **Stunstick (`StunstickImpact`)**: on a hit, a flat quad (Constants)
  plus a spark burst (impact_effects.md §8's electric sparks: count 1,
  size 2, along n, speed 64–256) at hit + 4·n, then the generic impact.
  Shared code: the swinger predicts it, the server sends it to the others.
- **Flesh**: a player's hitbox surface is `flesh`; its impact makes no
  particles; the decal and `Flesh.BulletImpact` (local) as any impact.

### 7. Blood

- **System**: `blood_impact_red_01` from `particles/blood_impact.pcf`
  (byte-identical to CS:S's file; impact_effects.md §10's table holds),
  oriented with its forward axis back along the shot. Enabled by
  `violence_hblood` 1.
- **Server copy** (every bullet or melee hit that a player takes, after
  the hitgroup scale, unless the rules refuse the damage — TDM teammates
  with friendly fire off): at the **hit point**, amount min(255, 5 ×
  damage) (unused), to the PAS of the point **minus the attacker** (the
  hit happens inside the attacker's command). Wall blood: impact_effects.md
  §10's traces, counted by the scaled damage (< 10: 1, < 25: 2, else 4).
- **Shooter's predicted copy** (bullets only; melee damage is not
  predicted): at **hit − 4·d**; wall-blood traces counted by the
  **distance** from the shooter's origin to the hit point (so beyond 25
  units always 4, σ 0.3); not for a teammate in TDM.
- So the victim and bystanders see server blood; the shooter sees his
  own predicted blood; a **melee attacker sees no blood** (Quirks).
- Crush damage (kind 1) to a player instead flashes the victim's screen red
  ((128, 0, 0), alpha 128, fade 1.0 s, hold 0.1 s); drowning blue
  ((0, 0, 128), 128); H8 owns the screen side.

### 8. Explosions

- **Temp entity**: grenades.md 5.4's client effect (flash overlay unless
  "no fireball", core when the scale is non-zero and "no fireball" is
  not set, debris unless "no particles", sound unless "no sound", water
  variant when the origin is in water, push of client ragdolls). Its
  sound is HL2's `BaseExplosionEffect.Sound` (§15.10), local to each
  receiving client. Sent to the PAS of the origin. HL2:DM's sources and
  parameters are the Constants table.
- **Explosion entity rule** (RPG, satchel, trip mine, map `env_explosion`):
  a line from origin + 8 Z to 40 below that (solid brushes and water);
  if it hits, the drawn explosion is at hit + 24·n (damage stays at the
  origin, H4); unless "no decal" (16), a `Scorch` decal on that hit.
  Spawnflags map to temp-entity flags: no fireball (4) → no fireball (and
  scale 0), no sound (64), random orientation (128), no fireball smoke
  (256), no particles (512), no dynamic lights (1024) → the matching
  flags; render mode 4 → "draw alpha", any mode other than additive (5) →
  "no additive". "No sparks" (32) clear: sparks (`spark_shower`, 0–3) are
  made only when the origin is **under water** (Quirks). Repeatable (2):
  the entity stays.
- **Explosion sparks in detail** (answered 2026-11-09). The test is the
  contents at the explosion entity's **own origin** (not the drawn point):
  water there and "no sparks" (32) clear → n = I(0, 3) spark showers (0,
  1, 2 or 3, equally likely), all created at the drawn point (hit + 24·n,
  or the origin when the down line hit nothing), each aimed along the
  line's hit normal. Out of water, or with flag 32 set, none. The flag
  and the test are the explosion entity's only; every explosion made by
  code (RPG, satchel, trip mine, exploding props, `func_breakable`)
  carries "no sparks", so only a map `env_explosion` without flag 32 can
  ever make them. Where the down line starts inside water it reports
  no usable surface; we read its normal as zero, which aims the showers
  straight down (Open questions).
- **`spark_shower`** (a server entity, never drawn itself; no model, not
  solid, zero size, not saved). At creation, with F = the forward vector
  of its angles (the normal above):
  - velocity v = U(200, 300)·F, then v_x += U(−100, 100), v_y +=
    U(−100, 100), then v_z += 200 if v_z ≥ 0, else v_z −= 200 (a surface
    normal pointing up gives 400–500 up);
  - "life" L = U(0.5, 1.5); first think 0.1 s after creation;
  - moves as a tossed object with bounce (projectiles.md's toss
    integration) under gravity 400 (scale 400 / `sv_gravity`), colliding
    with the world and solid entities (players too, mask "solid").
  - **Each think (every 0.1 s)**: one electric-spark burst at its
    position (`cs_source/impact_effects.md` §8 exactly: magnitude 1, trail
    length 1, no aim direction: 2–3 big sparks, 16–32 small sparks, the
    two flares and the 0.2 s smoke), sent as a temp entity to the PVS of
    that position; then L −= 0.1; L > 0 → think again in 0.1 s, else the
    shower is removed. So a shower makes ⌈L / 0.1⌉ bursts, 5 to 15, over
    0.5–1.5 s.
  - **On touching something** (during its move): v × 0.1 if it was
    resting on the ground, else v × 0.6; if then v_x² + v_y² < 10, L := 0
    (one more burst at the next think, then removed). After the touch the
    bounce: v reflected off the hit plane and scaled by min(0.9, the hit
    surface's elasticity × 1) (the `default` surface 0.25, so a bounce
    keeps 0.6 × 0.25 = 15 % of the speed); on a floor (normal z > 0.7) a
    speed under 30 stops it on the ground.
  - Water: on its first movement tick it counts as "entering water" (its
    remembered contents start empty), so when it is in water then it
    plays `BaseEntity.EnterWater` and halves v_z; leaving water later plays
    `BaseEntity.ExitWater`, entering again plays the enter sound and
    halves v_z again.
  - Nothing else: no light, no sound of its own, no damage.
- RPG, satchel and trip mine set "no sparks", "no dynamic lights" and
  "no smoke"; a rocket on a wall well above the floor leaves **no scorch**
  and is drawn at its own origin.
- dm_steamlab's 5 `env_explosion`s: spawnflags 50 (repeatable, no decal,
  no sparks), render mode 5, magnitude 125 (×3) or 150 (×2): scale 4.5 /
  5.0, radius 312 / 375.
- Screen shakes and hearing effects: H4.

### 9. Projectile visuals

All are client-side presentations of networked entities; the numbers are
the Constants table.

- **RPG laser**: while the RPG is guiding and not hidden (H4 6.1), the
  owner sees the beam and glow on the view model (re-projected
  attachments) and everyone else (in PVS) the same on the world model;
  the beam's brightness is re-drawn U(128, 200) every frame. The dot
  (`env_laserdot`): for the owner, drawn at the end of a trace from the
  **current view** along the current view direction (so it never lags
  the crosshair, even when the server's dot does); for others from the
  owner's eye along its animation eye angles; for a dormant owner at the
  networked dot position; always 4 units off the hit surface; full width
  16 ± 4 per frame. Sounds `Weapon_RPG.LaserOn`/`Off` (H4).
- **Missile**: `models/weapons/w_missile.mdl` after ignition; each frame
  the flare sprites and the smoke puffs; the trail stops emitting 0.1 s
  after the missile dies (puffs live out their life).
- **SMG1 grenade**: `env_smoketrail` (Constants).
- **Energy ball**: body, flicker and blur (Constants); while held in the
  gravity gun it draws its model facing the camera instead of the body
  quad. `cball_bounce` on each counted bounce (H4 5.4) to the PAS;
  explosion rings and sparks (H4 5.6) to every client.
- **Crossbow**: the view model's skin 1 (glowing bolt) from the load to
  the shot; charger glow and load flash on the view model (Constants);
  the bolt's motion blur in flight; on sticking in the world the glow
  sprite fading over 3 s, sparks unless in water (H4 7.4).
- **Frag**: H4 9.5 (red glow and `sprites/bluelaser1` trail).
- **Trip mine**: H4 8.2 (beam).

### 10. The dissolve

- **When**: a player killed by dissolve damage (the energy ball): his
  ragdoll record (player_models.md §10) is dissolved at the moment of
  death, and so is the weapon he drops (nobody can take it). Order within
  the kill: the ragdoll record is made, the dissolve attached, then the
  death sound is chosen and skipped because the ragdoll is dissolving
  (§15.8).
- **Server**: a dissolver entity follows the target; the target is moved
  to the "dissolving" collision group; at 2.0 s after the start (the
  first tick at or after: tick 134) the target is removed — the ragdoll
  disappears for everyone.
- **Client body**: drawn with colour (1 − p_in)·255 per channel and alpha
  255 until 1.9 s, then falling linearly to 0 at 2.0 s (p_in = t/1 s,
  clamped). So the body darkens to black over the first second, stays
  black, and vanishes in the last 0.1 s.
- **Float**: on the client ragdoll, every body gets an upward acceleration
  of 1.02 × gravity (it drifts up at +12 in/s² net) and any body faster
  than 50 in/s is slowed by up to 40 in/s per physics step towards 50.
- **Particles**: every frame, for every hitbox of the ragdoll, around the
  hitbox centre (offsets within ±0.375 of its two screen-facing extents)
  shifted along the camera's forward by 8 − 32·(1 − p) (p = the fade-in
  progress, 1 after the first second): the spark tries and glows of the
  Constants table. Material colours white. No sound.
- The weapon is dissolved the same way (it is not a ragdoll: no float).

### 11. Gravity gun effects

- States (gravity_gun.md §10: none, closed, ready, holding, launch) drive
  the Constants table. Sprite parameters interpolate linearly from their
  current value to the target over the stated time; the glows and end caps
  are re-randomised every frame (their per-state targets never show).
- **Closed**: end caps hidden; the rest as it was. **Ready**: core to its
  ready target, glows visible, end caps hidden, beams off; the hold sound
  fades to volume 0 and pitch 50 over 1 s. **Holding**: core, glows, end
  caps and beams on; the hold sound (if the claws are open) starts at
  volume 0, pitch 50 and ramps to volume 0.8 and pitch 100 over 0.5 s.
  **None**: everything hidden (also when the weapon leaves the PVS).
- **Launch (`PhyscannonImpact`)**: the blast sprite (all clients drawing
  the gun) and the effect, sent to the PVS of the end point (punter
  predicts his own). End point: the punted or held object's centre, or the
  end of an eye trace. The punter's client: a spark burst at the end
  point back towards the gun (16–32 colliding sparks, `effects/spark`,
  width U(1, 4), length U(0.01, 0.1), life U(0.25, 1), speed U(128, 512)
  around the direction ± 0.6, gravity 800, plus electric sparks size 1)
  and a beam from the view model's attachment 1 to the point:
  `sprites/orangelight1`, width 12 → 4, life 0.1 s, white, 16 segments,
  noise once. Other clients: only the spark burst, **at the world gun's
  attachment 1**, pointing at the end point.

### 12. Stunstick glow

- First person, drawn after the view model: a halo
  (`sprites/light_glow02_add_noz`) of half-size 22 at the `sparkrear`
  attachment with grey 0.4 while swinging; when not swinging half-size 20
  and grey 0.4 × a fade from 1 to 0.1 over the 0.25 s after the last swing
  ends; plus a halo on each spark attachment with grey U(0.05, 0.5) and
  half-size U(4, 5) (× the fade when not swinging). While swinging, 1
  frame in 4 an arc from a random spark attachment to a point ±8 around
  it: `sprites/lgtning`, width U(1, 2) → 0, noise U(16, 32), 16
  segments, life 0.05 s, white.
- Third person: a halo (`sprites/light_glow02_add`) at the world model's
  attachment 1, half-size 20, grey 0.1 (22 and 0.4 "while swinging" —
  for others the stick always counts as swinging, so always 22 and 0.4,
  Quirks); plus `effects/blueflare1` half-size U(4, 6), grey U(0.6, 0.8);
  and 1 frame in 6 a tesla arc from attachment 1 (width U(1, 3), noise
  U(1, 2), life 0.01 s).
- Deploy: sparks at the world model's attachment 1 (server effect) with
  `Weapon_StunStick.Activate`; holster: `Weapon_StunStick.Deactivate`.

### 13. UnlitTwoTexture and the dm_underpass shield walls

- **Shader** (DX9 path; LDR as shaders.md §1): no lighting, no
  vertex lighting, no envmap (the materials' `$envmap` and `$envmaptint`
  are ignored). Inputs: `$basetexture` (+ `$frame`,
  `$basetexturetransform`), `$texture2` (+ `$frame2`,
  `$texture2transform`; default transform identity), `$color` and the
  entity's render colour ("modulation", gamma colours converted to linear
  unless above 1), `$alpha`, flags `$additive`, `$translucent`,
  `$vertexcolor` (stream only; not used in the colour), the cloak pass
  (unused here).
- Per pixel, in linear space (both textures are read as sRGB):

```math
c = T_1(uv_1) \cdot T_2(uv_2) \cdot m_{rgb}
```

  with uv₁, uv₂ the one texture coordinate through each transform; output
  colour c with fog (shaders.md §7), **output alpha always 1**. So
  `$alpha` and the textures' alpha never fade the surface.
- **Blending**: translucent (texture alpha in either texture, or `$alpha`
  below 1) with `$additive`: source·alpha + destination, i.e. plain
  additive since alpha is 1; opaque with `$additive`: one + one (the
  same); without `$additive`: alpha blend (opaque, alpha 1) or none.
  `$translucent` puts the surface in the translucent pass (sorted, no
  depth write, shaders.md).
- **Stock use**: 4 materials in dm_underpass's pak, each a map-compiler
  patch of `effects/combineshield/comshieldwall2` that only sets a baked
  cubemap `$envmap` (unused by this shader) and an empty `Proxies` block
  (the compiler's patch rule copies only plain values, so the original
  proxies stay, Open questions 7). They sit on the two faces of each of
  two `func_illusionary` brushes (`*2` at (−728, −1218.56, 81.27) and
  `*29` at (−388, −2331.96, 82.73), render mode 0, colour 255 255 255,
  amount 255).
- **The shield material**: `$basetexture`
  `effects/combineshield/comshieldwall` (31 frames), `$texture2`
  `…/comshieldwall_close`, `$translucent` 1, `$additive` 1, `$nodecal` 1.
  Proxies, every frame for the entity being drawn:
  - frame: d = distance from the local player's centre to the entity's
    centre; `$frame` = `$frame2` = clamp(0.1·d − 22, 0, 30), truncated:
    frame 0 within 230 units, rising one frame per 10 units, frame 30
    from 520 units. (`$frame2` beyond texture 2's single frame: Open
    questions 7.)
  - texture 2 scroll: `$texture2transform` translates by 0.1·t·(cos 25°,
    sin 25°) texture units (t in seconds, wrapped).
  - `$alpha` follows a 1.08 s sine between a Gaussian-noise value minus
    0.001·d and 0.1 above it; with the output alpha fixed at 1 it has no
    visible effect (Quirks).
- Result: an additive, unlit, slowly crawling blue-green field whose
  pattern changes with the viewer's distance.

### 14. HDR

All 8 stock maps carry LDR and HDR lighting (H0) and one
`env_tonemap_controller` each; several skies are `_hdr`. H9 specs the
LDR path only (shaders.md, `mat_hdr_level` 0): HDR tone mapping, bloom
and the `_hdr` sky textures are out of scope. refcmp captures of the stock
maps must set `mat_hdr_level 0` (Open questions 8).

### 15. Sounds

#### 15.1 Scripts

- Manifest: Half-Life 2's loose `scripts/game_sounds_manifest.txt`, 63
  files, all `precache_file`; `hl2mp`'s `scripts/game_sounds_player.txt`
  and `scripts/game_sounds_weapons.txt` take the place of HL2's files of
  the same path (search order), so HL2's loose copies are never read.
  Differences of `hl2mp`'s weapons file from HL2's: `)` (spatial stereo)
  prefixes on the 357, AR2-ball, pistol-reload and gravity-gun waves,
  `TripmineGrenade.Place`/`Activate` added, `Weapon_Physgun.On`/`Off`
  volume 0.42. `hl2mp`'s player file adds `NPC_MetroPolice.Die`,
  `NPC_CombineS.Die`, `NPC_Citizen.die`.
- No HL2:DM entry uses `CompatibilityAttenuation`: every one has a sound
  level (default 75), so only sounds.md's level path applies.
- `$gender` in a wave path expands from the emitting model's name
  (female when the name contains "female", else male: engine-side,
  assumed; the stock citizen models are `male_0N`/`female_0N`).
- Data quirk: `Weapon_Pistol.Special2`'s level is written `)SNDLVL_NORM`;
  that parses as an interval, value 0 = heard everywhere. Unused in DM.

#### 15.2 Loudness and who receives

| Level (dB) | Attenuation 20/(L − 50) | Send radius (units) | Used by |
|---|---|---|---|
| 0 | – | everywhere, no fall-off | UI (`Player.WeaponSelection*`, `Player.DenyWeaponSelection`) |
| 60 | 2.0 | 1000 | `HL2Player.SprintStart`, `SprintNoPower` |
| 70 | 1.0 | 2000 | `Grenade.Blip`, `TripmineGrenade.Place`/`Activate` |
| 75 | 0.8 | 2500 | most items, reloads, empties, footsteps, deaths of citizens |
| 80 | 0.667 | 3000 | Combine and police deaths, crossbow bolt hits |
| 90 | 0.5 | 4000 | `Missile.Ignite`, `AlyxEMP.Charge`, `NPC_CombineBall.HoldingInPhysCannon`, `Weapon_Crossbow.BoltSkewer` |
| 100 | 0.4 | 5000 | `NPC_CombineBall.Impact` |
| 105 | 0.364 | 5500 | crowbar swing, stunstick hit, gravity-gun pickup and drop |
| 140 | 0.2222 | 9000 | all gunshots, `BaseExplosionEffect.Sound`, energy ball launch and explosion, near-miss |

- The radius cut applies to sounds sent with the attenuation filter:
  weapon sounds, and named sounds emitted from an entity without an
  explicit filter (items, chargers, use, flashlight, projectiles). Death
  sounds and footsteps go to the plain PAS (no cut). Client-local sounds
  (impacts, whizz, explosion effect, view-model events) are not sent.
- Gain against distance is engine-side (sounds.md Open questions 1); the
  same curve as CS:S's level path is assumed.

#### 15.3 Weapon sounds

"Fire", "empty", "reload" etc. are the script's `SoundData` keys;
"Shared" = the shooter's client plays it on first prediction (to itself,
from the owner) and the server sends it to everyone else (attenuation
filter, prediction rules); "Owner" = only the owner's client; "Server" =
the server to all in range including the owner.

| Weapon | Action | Entry | Channel, volume, pitch, level, waves | Who |
|---|---|---|---|---|
| pistol | fire | `Weapon_Pistol.Single` | weapon, 0.55, 98–102, 140, 1 | shared |
| pistol | empty | `Weapon_Pistol.Empty` | weapon, 0.7, 100, 75, 1 | shared |
| pistol | reload | `Weapon_Pistol.Reload` | item, 0.7, 100, 75, 1 | shared (the owner's copy twice on one channel: one heard) |
| .357 | fire | `Weapon_357.Single` | weapon, 0.93, 88–93, 140, 2 | shared |
| .357 | empty | `Weapon_Pistol.Empty` | as pistol | shared |
| .357 | reload | view-model events: `Weapon_357.OpenLoader` (cycle 0.255), `RemoveLoader` (0.355), `ReplaceLoader` (0.609), `Spin` (0.836) | item, 0.7, 100, 75, 1 each | owner |
| SMG1 | fire | `Weapon_SMG1.Single` | weapon, 0.55, 95–105, 140, 1 | shared, once per bullet (stamped with its scheduled time) |
| SMG1 | grenade | `Weapon_SMG1.Double` | weapon, 0.61, 100, 140, 1 | shared |
| SMG1 | empty | `Weapon_SMG1.Empty` | weapon, 0.7, 100, 75, 1 | shared |
| SMG1 | reload | `Weapon_SMG1.Reload` | item, 0.7, 100, 75, 1 | shared |
| AR2 | fire | `Weapon_AR2.Single` | weapon, 0.8, 85–95, 140, 1 | shared, per bullet |
| AR2 | ball charge | `Weapon_CombineGuard.Special1` | weapon, 0.7, 100, 75, 1 | shared |
| AR2 | ball launch | `Weapon_IRifle.Single` | weapon, 0.55, 100, 140, 1 | shared |
| AR2 | empty | `Weapon_IRifle.Empty` | weapon, 0.7, 100, 75, 1 | shared |
| AR2 | reload | `Weapon_AR2.Reload`; view-model `Weapon_AR2.Reload_Rotate` (0.021), `Reload_Push` (0.404) | item, 0.7 / 0.9 / 0.9, 100, 75 | owner |
| shotgun | fire / double | `Weapon_Shotgun.Single` / `Double` | weapon, 0.86 / 1.0, 98–101 / 90–95, 140, 1 | shared |
| shotgun | empty | `Weapon_Shotgun.Empty` | weapon, 0.7, 95–100, 75, 1 | shared |
| shotgun | each shell | `Weapon_Shotgun.Reload` | item, 0.7, 100, 75, 3 | shared |
| shotgun | pump | `Weapon_Shotgun.Special1` | item, 0.7, 100, 75, 1 | shared |
| crowbar | swing (hit or miss) | `Weapon_Crowbar.Single` | weapon, 0.55, 95–100, 105, 1 | shared |
| stunstick | swing | `Weapon_StunStick.Swing` | weapon, 0.7, 95–102, 75, 2 | shared |
| stunstick | any hit | `Weapon_StunStick.Melee_Hit` | weapon, 0.7, 98–102, 105, 2 | shared |
| stunstick | deploy / holster | `Weapon_StunStick.Activate` / `Deactivate` | weapon, 0.7, 98–102, 75, 3 | server |
| gravity gun | punt / launch | `Weapon_PhysCannon.Launch` | weapon, 0.27, 110–120, 140, 3 | shared |
| gravity gun | dry fire | `Weapon_PhysCannon.DryFire` | weapon, 0.22, 100, 75, 1 | shared |
| gravity gun | pick up / drop | `Weapon_PhysCannon.Pickup` / `Drop` | weapon, 0.22, 100, 105, 1 | shared |
| gravity gun | claws open / close | `Weapon_PhysCannon.OpenClaws` / `CloseClaws` | voice, 0.2, 100, 75, 1 | shared |
| gravity gun | too heavy | `Weapon_PhysCannon.TooHeavy` | weapon, 0.4, 100, 75, 1 | shared |
| gravity gun | holding loop | `Weapon_PhysCannon.HoldSound` | static, ramps (§11), 75, 1 | owner's and each client's own copy |
| frag | throw / lob | `common/null.wav` (raw file) | silent | – |
| frag | roll | `WeaponFrag.Roll` | voice, 0.7, 100, 75, 1 | shared |
| RPG | fire | `Weapon_RPG.Single` | weapon, 0.55, 100, 140, 1 | shared |
| RPG | laser on / off | `Weapon_RPG.LaserOn` / `LaserOff` | weapon, 0.7, 100, 75, 1 | shared |
| RPG | empty | `Weapon_SMG1.Empty` | as SMG1 | shared |
| crossbow | fire | `Weapon_Crossbow.Single` + `Weapon_Crossbow.BoltFly` | weapon 0.61 93–108 **75**; voice 0.6 90–110 75 | shared |
| crossbow | reload | `Weapon_Crossbow.Reload` | item, 0.7, 93–108, 75, 1 | owner |
| crossbow | load | `Weapon_Crossbow.BoltElectrify` | weapon, 0.6, 93–108, 75, 2 | shared |
| S.L.A.M. | (no `SoundData`) | `Weapon_SLAM.SatchelThrow`, `TripMineMode` (view-model event, cycle 0.75 of the mode switch), `SatchelDetonate` | voice, 0.7, 100, 75, 1 | H4 |

The `_npc`/`NPC_*` keys and `Burst`, `Special1/2` of the pistol and SMG1
are never played by players.

#### 15.4 Pickups, items, suit, chargers

| Event | Entry | Channel, volume, pitch, level, waves | Who |
|---|---|---|---|
| ammo added to the reserve (ammo item, a weapon taken for ammo, or a new weapon whose default ammo exceeds its clip; once per ammo type that gains ≥ 1) | `BaseCombatCharacter.AmmoPickup` | item, 1, 100, 75, 1 | server |
| weapon taken without ammo gain | none (`HL2Player.PickupWeapon` is HL2's single-player path) | – | – |
| health kit / vial | `HealthKit.Touch` / `HealthVial.Touch` | item, 1, 100, 75, 1 (both `items/smallmedkit1.wav`) | server |
| battery | `ItemBattery.Touch` | item, 1, 100, 75, 1 | server |
| item or weapon reappears / is moved back | `AlyxEMP.Charge` | weapon, 0.65, 100–120, 90, 1 | server |
| `+use` found / nothing | `HL2Player.Use` / `HL2Player.UseDeny` | item, 0.4, 100, 75, 1 | server |
| flashlight on / off | `HL2Player.FlashLightOn` / `Off` | weapon, 0.8, 100, 75, 1 (same wave) | server |
| sprint start / no power | `HL2Player.SprintStart` / `SprintNoPower` | voice, 0.9 / 0.8, 100, 60, 1 | shared (predicted) |
| suit charger start / loop / deny | `SuitRecharge.Start` / `ChargingLoop` / `Deny` | item / static / item, 0.75, 100, 75, 1 | server |
| health charger start / loop / deny / refill | `WallHealth.Start` / `LoopingContinueCharge` / `Deny` / `Recharge` | item / static / item / item, 0.7, 100, 75, 1 | server |
| ammo crate (none placed in stock maps) | `AmmoCrate.Open` / `Close` | static, 0.45, 75 | server |
| weapon menu | `Player.WeaponSelectionOpen`, `…MoveSlot`, `Player.WeaponSelected`, `Player.DenyWeaponSelection` | item, 0.32, 100, 0 (no fall-off) | owner (UI) |

Timing of all of these: suit.md, weapons.md §8 and §10, movement.md.

#### 15.5 Who makes which sound (summary)

- **Predicted on the actor's client, sent by the server to the others**:
  weapon `SoundData` sounds from shared code, sprint sounds, footsteps of
  the local player (the server sends steps only to listeners outside the
  walker's PVS; those inside make other players' steps themselves,
  movement.md).
- **Owner's client only**: view-model sound events, the reload sounds of
  weapons whose reload is the generic one (AR2, .357, crossbow), the
  gravity gun's hold loop (each client runs it for the guns it draws),
  whizz, impact sounds and splashes of one's own shots.
- **Each receiving client for itself**: impact sounds of others' shots
  (§1), the explosion effect's `BaseExplosionEffect.Sound`, shell
  bounces, UI.
- **Server to all (including the actor)**: pickups, chargers, use,
  flashlight, damage sounds (§15.7), death (§15.8), drown, fall damage,
  projectile and grenade sounds (§15.10).

#### 15.6 Footsteps

- Timing, volume and silence: movement.md "Step sounds" (H1).
- **Model type** (set whenever the player's model is set, by its path,
  case-insensitive, first match): contains `models/human` → citizen;
  else contains `police` → metro police; else contains `combine` →
  Combine; a path matching none keeps the previous type (initially
  citizen).

| Type | Left entry | Right entry | Waves | Script volume (ignored) | Pitch | Level |
|---|---|---|---|---|---|---|
| citizen (male and female) | `NPC_Citizen.RunFootstepLeft` | `NPC_Citizen.RunFootstepRight` | `npc/footsteps/hardboot_generic1/3/5` · `2/4/6` | 0.4 | 95–105 | 75 |
| Combine (3 models) | `NPC_CombineS.RunFootstepLeft` | `…Right` | `npc/combine_soldier/gear1/3/5` · `2/4/6` | 0.75 | 95–105 | 75 |
| metro police | `NPC_MetroPolice.RunFootstepLeft` | `…Right` | `npc/metropolice/gear1/3/5` · `2/4/6` | 0.55 | 95–105 | 75 |

- Played from the player on the **body** channel at the step's computed
  volume (0.2, 0.5, …; H1), the entry's level and a pitch drawn from its
  range; a uniform wave pick (sounds.md). All six entries are precached
  for every player.
- No surface, ladder or water step entries (`Ladder.Step*` only for the
  ladder mount, H1); the walk entries (`…FootstepLeft`, tile and concrete
  waves) are never used.

#### 15.7 Pain (damage sounds)

There is **no pain voice** in HL2:DM (the suit's voice lines are
disabled in multiplayer). Damage sounds, each emitted by the server from
the victim to everyone in range, after the damage is applied (only if
the damage was taken):

| Damage kind | Entry | Channel, volume, pitch, level, waves |
|---|---|---|
| bullet (any hit, every bullet) | `Flesh.BulletImpact` | auto, 0.7, 100, 75, 5 (`physics/flesh/flesh_impact_bullet1–5`) |
| burn | `HL2Player.BurnPain` | voice, 0.5, 100, 75, 3 |
| plasma | `Player.PlasmaDamage` | body, 0.5, 100, 75, 1 |
| sonic | `Player.SonicDamage` | body, 0.7, 100, 75, 3 |
| drowning | `Player.DrownStart` / `DrownContinue` | voice, 1.0, 90–110, 75, 1 / 3 (suit.md) |
| fall | `Player.FallDamage` | auto, 0.75, 90–110, 75, 2 (movement.md) |
| club (melee), blast, crush, dissolve | none of their own (the weapon's hit sound, the explosion, the physics impact) | – |

The shooter additionally hears his own client's impact sound on the
victim's flesh (`Flesh.BulletImpact` again, local), so a hit is heard
twice by the shooter; other clients hear the impact sound of the surface
behind the victim (§1).

#### 15.8 Death

- At death, after the ragdoll is made: if the ragdoll is dissolving,
  nothing. Otherwise the entry `<type>.Die` (`NPC_Citizen.die`,
  `NPC_CombineS.Die`, `NPC_MetroPolice.Die`), resolved with the dead
  player's model (gender), is emitted from the player entity at its
  origin on the entry's channel with its volume, level and pitch, to the
  **PAS of the origin** (no distance cut). A fall death plays the same
  (not `Player.FallGib`); `Player.Death` is never used.

| Type | Entry | Channel, volume, pitch, level | Waves |
|---|---|---|---|
| citizen | `NPC_Citizen.die` | voice, 1.0, 100, 75 | `*vo/npc/male01/pain07–09` or `female01` (streamed; Open questions 2) |
| Combine | `NPC_CombineS.Die` | voice, 0.5, 100, 80 | `npc/combine_soldier/die1–3` |
| metro police | `NPC_MetroPolice.Die` | voice, 0.5, 100, 80 | `npc/metropolice/die1–4` |

#### 15.9 Bullets, impacts and physics

| Entry | Channel, volume, pitch, level, waves | Use |
|---|---|---|
| `Bullets.DefaultNearmiss` | static, 0.7, 100, 140, 8 (`>weapons/fx/nearmiss/bulletLtoR03, 04, 06, 07, …`) | §3 |
| `Bullets.GunshipNearmiss` | static, 0.7, **50**, 140, 6 | §3, AR2 |
| `<surface>.BulletImpact` | per surface (e.g. `Concrete.BulletImpact` 0.7, 75, 4 waves; `Default.BulletImpact` 0.7, 75, 3) | §6 |
| `Physics.WaterSplash` | voice, 0.8–1.0, 85–115, 75, 3 (code asks volume 1) | splashes |
| `Underwater.BulletImpact` | auto, 0.9, 95–105, 95, 3 (`#` dry) | only with one player slot (never in DM) |
| `Bounce.Shell` / `Bounce.ShotgunShell` | static, 0.5, 100, 75, 3 | §5 |

Physics impacts, scrapes and breaks: sounds.md §4 with HL2's surfaces.

#### 15.10 Projectiles and explosions

| Entry | Channel, volume, pitch, level, waves | Use |
|---|---|---|
| `BaseExplosionEffect.Sound` | static, 1.0, 100, 140, 3 (`^weapons/explode3–5`) | every explosion temp entity, client-local |
| `WaterExplosionEffect.Sound` | static, 1.0, 80–130, 140, 2 | under-water explosions (plus `Physics.WaterSplash`) |
| `BaseGrenade.Explode` | voice, 0.45, 100, 75, 3 (`weapons/debris1–3`) | frag, shot satchel (server) |
| `Grenade.Blip` | weapon, 0.95, 100, 70, 1 | frag (H4) |
| `Missile.Ignite` | weapon, 1.0, 100, 90, 1 (`weapons/rpg/rocket1.wav`) | ignition to removal (stopped on removal) |
| `NPC_CombineBall.Launch` | static, 1.0, 95–105, 140, 2 | |
| `NPC_CombineBall.Impact` | static, 1.0, 95–105, 100, 2 | each bounce |
| `NPC_CombineBall.Explosion` | static, 0.75, 95–105, 140, 1 | |
| `NPC_CombineBall.HoldingInPhysCannon` | static, 0.9, 95–105, 90, 1 | held ball (H5) |
| `Weapon_Crossbow.BoltHitBody` | body, 0.7, 93–108, 80, 2 | |
| `Weapon_Crossbow.BoltHitWorld` | body, 0.95–1.0, 110–130, 80, 1 | |
| `Weapon_Crossbow.BoltSkewer` | static, 0.9, 90–110, 90, 1 | client, pinning (H4 7.5) |
| `TripmineGrenade.Place` / `Activate` | weapon, 0.95, 100, 70, 1 | |
| `Weapon_SLAM.SatchelThrow` / `TripMineMode` / `SatchelDetonate` | voice, 0.7, 100, 75, 1 | |

## Per-tick order

Server, one user command (weapons.md's order), effects and sounds only:

1. Movement: sprint sound (prediction rules), step sounds (H1).
2. Weapon frame: weapon sounds (others only), muzzle-flash counters on
   view model and weapon; for each bullet: trace, damage → blood temp
   entity (minus the shooter), `Flesh.BulletImpact` from the victim (all),
   kill → ragdoll record, dissolve (if dissolve damage), death sound; tracer
   counter; at the end of the call the fire-bullets temp entity (minus the
   shooter).
3. Entity thinks after the commands: projectiles (their sounds and
   explosion temp entities go to everyone, not suppressed), dissolvers
   (removal at 2.0 s).

Client, one frame:

1. Network: temp entities (fire-bullets → tracers and impacts; blood;
   explosions; effects), entity updates (muzzle-flash counters → light).
2. Prediction of new commands (first time only): own weapon sounds,
   tracers (with whizz test), impacts, blood, muzzle-flash events from
   the view model's fire sequence.
3. Client thinks: gravity gun sprite targets, stunstick arcs, dissolve
   colour.
4. Particle simulation (old-style, dt ≤ 0.1), then drawing: world,
   translucents (shield wall, sprites, beams, halos), view model, then the
   effects drawn after it (RPG laser, gravity gun sprites in first
   person, stunstick halos).

## Edge cases

- **Shooter switches weapon** before a remote client gets the fire-bullets
  message: that client draws the tracer kind and impact of the weapon it
  now thinks is active (an AR2 burst can show default tracers and no
  `AR2Impact`, or the reverse).
- **Dormant shooter** (outside the receiver's PVS but inside the PAS of
  the shot): no tracers, no impacts on that client.
- **Point-blank remote shots**: the remote re-trace passes through the
  victim; the impact lands on the wall behind it, possibly far away.
- **Shotgun on a remote client**: any 7 consecutive counter values hold a
  multiple of 4, so every shotgun shot is flagged and other players see
  7 or 12 tracers per shot, while the shooter sees 1–3 of his own.
- **Bullet into water from a remote shooter**: the server's splash
  arrives (PAS of the entry point), and the remote re-trace (no water)
  also draws an impact on the floor under the water if "has impacts".
- **Melee kill**: no blood for the attacker (server blood suppressed to
  him, no predicted blood), blood for everyone else.
- **Explosion entity near a ceiling or wall only**: the downward 40-unit
  probe misses: no scorch, drawn at the origin.
- **Dissolve of a ragdoll whose player respawns**: the ragdoll keeps
  dissolving and is removed at 2.0 s regardless.
- **Two whizzes 0.05 s apart** (two tracers in one frame burst): one
  sound.
- **Gravity gun leaves the PVS**: all its sprites and beams turn off at
  once.
- **First-person RPG laser**: the dot is where the local view points this
  frame, not where the server's dot is; others see the server-side aim
  (via the owner's eye angles).

## Quirks

- **Near-misses only for your own bullets** (§3): keep (parity).
- **Remote impacts ignore players** (§1): keep; it is visible.
- **Predicted blood offset and distance-driven wall blood** for the
  shooter (§7): keep.
- **Melee attacker sees no blood**: keep.
- **No third-person muzzle flash sprites** (§4): keep (Open questions 3).
- **Energy ball trail invisible** (additive black): keep (draw nothing).
- **Explosion-entity sparks only under water** (inverted test): keep; no
  stock map hits it (steamlab's set "no sparks").
- **Shield wall `$alpha` proxies without effect** (§13): keep.
- **Stunstick third person always "swinging"**: the swing test for a gun
  not drawn through a view model always answers yes, so other players'
  stunsticks always glow at the bright setting and throw arcs: keep.
- **Shotgun flash one frame** (life 0.0001 s): keep.
- **Own tracers' counter** advances on the shooter's client independently
  of the server's: the shooter's own tracer pattern may differ from what
  others see.

## Test cases

Implementer checks, no game needed (tick 0.015; frame time as stated;
random draws fixed at the stated values):

| # | Setup | Input | After | Expected |
|---|---|---|---|---|
| 1 | player model `models/humans/group03/female_04.mdl` | step | – | `NPC_Citizen.RunFootstepLeft` (first step), body channel |
| 2 | model `models/police.mdl`; then `models/combine_super_soldier.mdl` | step after each change | – | `NPC_MetroPolice.RunFootstep…`, then `NPC_CombineS.RunFootstep…` |
| 3 | model path `models/Humans/Group03/male_01.mdl` (capitals) | step | – | citizen (case-insensitive) |
| 4 | citizen running 190 on concrete (H1 test 55) | step | – | volume 0.2 (not the entry's 0.4), level 75, pitch in 95–105, one of `hardboot_generic1/3/5` |
| 5 | Combine sprinting 320 | step | – | volume 0.5, entry volume 0.75 ignored |
| 6 | fully ducked, moving | – | – | no step entry ever (H1) |
| 7 | female citizen killed by a pistol | death | – | `NPC_Citizen.die`, one of `vo/npc/female01/pain07–09`, voice, volume 1, 75 dB, from the origin, PAS (no 2500-unit cut) |
| 8 | Combine killed | death | – | `NPC_CombineS.Die`, volume 0.5, 80 dB |
| 9 | player killed by the energy ball at tick t | – | – | no death sound; ragdoll dissolving; removed at tick t + 134 |
| 10 | dissolve started at time 0 | client body colour | 0.5 s / 1.5 s / 1.95 s / 2.0 s | rgb 127, alpha 255 / rgb 0, alpha 255 / rgb 0, alpha 127 / removed |
| 11 | dissolving ragdoll body at rest, gravity 600 | 1 s | – | upward speed ≈ +12 in/s (net +0.02 g) |
| 12 | dissolving body moving 130 in/s | one physics step | – | 90 in/s; next step 50; stays ≤ 50 |
| 13 | pistol body hit (8 damage, generic hitgroup) on a player | server | – | blood at the hit point, amount 40, 1 wall trace σ 0.1, to PAS minus the shooter; `Flesh.BulletImpact` from the victim to all within 2500 |
| 14 | as 13, head (×3 = 24) | server | – | amount 120, 2 wall traces σ 0.2 |
| 15 | shooter 300 units from the victim | shooter's client | – | blood at hit − 4·d, 4 wall traces σ 0.3 (distance, not damage) |
| 16 | crowbar hit on a player | – | – | attacker: decal and impact sound only, no blood; others: blood |
| 17 | TDM, friendly fire off, shot at a teammate | – | – | no blood anywhere, no `Flesh.BulletImpact` |
| 18 | server tracer counter at 1; SMG1 fires 2 bullets in one tick (catch-up) | – | – | bullet 2 gets the tracer (counter 2 mod 2 = 0) → "has tracers"; remote clients draw 2 tracers |
| 19 | counter at 1; shotgun 7 pellets (every 4th) | – | – | pellet 4 selected (counter 4) → remote clients draw 7 tracers |
| 20 | any counter value c; shotgun 7 or 12 pellets | – | – | c … c + 6 always holds a multiple of 4 → **every** shotgun shot is flagged: remote clients always draw 7 (12) tracers; the shooter's own client draws 1–2 (3) |
| 21 | AR2 tracer, D = 1000, L = 200 | – | – | speed 8000, life 0.15 s; head at τ = 0.1: 800, tail 600 |
| 22 | AR2 tracer, D = 120 | – | – | no streak; whizz test still runs |
| 23 | default tracer, own shot, eye (0, 0, 0), muzzle (20, −6, −5), hit (1000, 0, 0) | own client | – | closest point is the muzzle, distance ≈ 21.5 (≈ 20.9 after the listener slide) < 24 → `Bullets.DefaultNearmiss` at the muzzle |
| 24 | remote SMG1 tracer passing 5 units from the local eye | – | – | no whizz (no flag) |
| 25 | own AR2 tracer passing 60 units from the eye, below it | own client | – | `Bullets.GunshipNearmiss` (72 > 60), pitch 50 |
| 26 | own default tracer passing 30 units beside the eye, level | own client | – | no whizz (30 ≥ 24; slide doesn't help sideways) |
| 27 | two own tracers 0.05 s apart, both within range | – | – | one whizz; a third 0.1 s after the first: whizz |
| 28 | SMG1 flash, k = 1.25, all U(6, 8) draws 6 | first person | – | sprites at 10, 20, 30, 40, 50 along f; half-sizes 8, 7, 6, 5, 3; life 0.025 s |
| 29 | AR2 flash, k = 2, U draws 6, core draw 10 | first person | – | sprites at 16, 32, 48, 64, 80; half-sizes 11, 10, 9, 8, 7; core 10 at the muzzle |
| 30 | pistol flash, k = 1, draws 6 | first person | – | sprites at 4, 8, 12, 16, 20; half-sizes 7, 6, 5, 4, 3; life 0.01 s; smoke 3 times in 4 |
| 31 | another player fires the SMG1 | local client | – | no flash sprites; a light at the world gun's attachment 1, radius 32–64, (255, 192, 64), 0.05 s |
| 32 | RPG or crossbow fired | first person | – | no flash sprites, no light |
| 33 | SMG1 shot, first person | – | – | one `shell.mdl` from the view model's attachment 2 at 150–200 along its f (+ jitter), life 1–2 s |
| 34 | shell landing with |v_z| 100 | bounce | – | sound with probability 1/4; volume 0.5 × 100/350 = 0.143 |
| 35 | AR2 bullet hits concrete | own client | – | `AR2Impact` quad at hit + n, start side 24–32, at 0.125 s side 0.25 × start, alpha 0.6; then concrete flecks and dust (impact_effects.md §5), `Concrete.BulletImpact`; no ricochet |
| 36 | shotgun 12 pellets on one concrete wall | own client | – | 12 impact sounds (no grouping) |
| 37 | bullet entering water | own client | – | splash with s in [4, 8], f = s/8; no impact |
| 38 | stunstick hit on a wall | – | – | glow quad side s → 2s (s 16–32), 0.1 s, at hit + 4n; sparks; `Weapon_StunStick.Melee_Hit` |
| 39 | frag explodes on a floor | – | – | temp entity scale 7.5, 25 fps, radius 250, probe length 150; `BaseGrenade.Explode` from the server; `BaseExplosionEffect.Sound` on each client in the PAS |
| 40 | shot satchel pops | – | – | scale 0: flash overlay, debris, sound; no fireballs or smoke puffs |
| 41 | RPG missile hits a wall at (0, 0, 100), floor at z = 0 | – | – | probe from z 108 to 68 misses: no scorch; drawn at the missile origin; scale 5.0, radius 300 |
| 42 | RPG missile hits the floor at z = 0 (origin z 0.5) | – | – | probe hits at z 0: drawn at z 24; `Scorch` on the floor |
| 43 | `env_explosion` magnitude 125, no radius override, spawnflags 50 | Explode | – | scale 4.5, radius 312, no decal, no sparks, entity stays |
| 44 | `env_explosion` magnitude 30 / 300 | Explode | – | scale 1.0 / 5.0 (clamped 10 / 50 before ÷ 10) |
| 45 | missile at 1500 u/s, frame 1/60 s | one frame | – | 6 smoke puffs (25 units / 4), flare: 8 sprites at 3, 6, …, 24 behind |
| 46 | rocket puff alpha draw 0.2 exactly | age = half its life | – | alpha 51 × sin(π/2) = 51 |
| 47 | laser dot, view trace hits a wall 500 ahead with n facing the player | local owner | – | glow quad at hit + 4n, full width 12–20, follows the view this frame |
| 48 | gravity gun, first person, holding from ready (core 14) | 0.1 s | – | core width 15, alpha 255 (0.1 s); 2 beams width 0 → 4; end caps at `fork1t`, `fork2t` |
| 49 | gravity gun punt | 0.05 s / 0.2 s | – | blast width 36 / 64; alpha 191.25 / 0 |
| 50 | another player punts | local client in PVS of the point | – | spark burst at that player's gun, no beam |
| 51 | dm_underpass, local player 300 units from the shield's centre | draw | – | base frame 8 (0.1·300 − 22); additive; colour = T1 × T2 (linear); output alpha 1 |
| 52 | same at 200 / 600 units | draw | – | frame 0 / 30 |
| 53 | shield, t = 10 s | draw | – | texture 2 offset (0.9063, 0.4226) texture units |
| 54 | UnlitTwoTexture with `$alpha` 0.5 | draw | – | same colour as `$alpha` 1 (alpha ignored) |
| 55 | send radius | 140 / 105 / 75 / 60 dB sound from a player | listener 8000 / 5000 / 2600 / 900 units away | received / received / not received / received |
| 56 | weapon fire on the shooter's client | – | – | the client plays the fire entry once; the server sends it to others in range, never to the shooter |
| 57 | AR2 reload by another player | local client | – | nothing heard (owner-only) |
| 58 | shotgun reload by another player | – | – | `Weapon_Shotgun.Reload` per shell |
| 59 | `Weapon_Pistol.Special2` resolved | – | – | level 0 (everywhere) |
| 60 | `NPC_Citizen.die` lookup (two definitions) | – | – | 3-wave `hl2mp` definition (assumed, Open questions 2) |
| 61 | picking up a new `weapon_crowbar`, `weapon_smg1` or `weapon_357` (default ammo equals the clip size: nothing goes to the reserve) | – | – | no pickup sound |
| 62 | picking up a new `weapon_frag` (no clips, 1 grenade, reserve below 5) / taking a placed SMG1 as an owner (+45 reserve) | – | – | one `BaseCombatCharacter.AmmoPickup` each |
| 63 | map `env_explosion` spawnflags 2 (no "no sparks"), origin 10 units under a water surface, nothing else below | Explode | – | 0–3 `spark_shower`s at the water hit + 24·(0, 0, 1); each starts at 400–500 u/s up plus ±100 sideways and makes 5–15 electric-spark bursts 0.1 s apart |
| 64 | the same with spawnflags 34 (no sparks), or the origin out of water | Explode | – | no `spark_shower` |
| 65 | a `spark_shower` with L = 0.53 that never touches anything | – | – | 6 bursts at 0.1, 0.2 … 0.6 s, removed at 0.6 s |
| 66 | a `spark_shower` falling onto a default-surface floor at (100, 0, −300) | touch | – | after the touch × 0.6 and the bounce × 0.25: (15, 0, 45); still above 10 horizontally squared, keeps bursting |

Checks against this install or the probe server (game build 24600434,
dedicated server build 24600426):

| # | How | Expected |
|---|---|---|
| P1 (install) | list the `SoundData` of the 12 weapon scripts in `hl2mp_pak_dir.vpk` | as §15.3 (S.L.A.M. empty; frag `common/null.wav` twice) |
| P2 (install) | resolve every entry of §15.3–§15.10 through the manifest (heavy `sound_stock.rs`) | all found, values as tabled; `Weapon_Pistol.Special2` level 0 |
| P3 (install) | dm_underpass's materials via the pak and VPKs | exactly 4 UnlitTwoTexture materials, all patches of `effects/combineshield/comshieldwall2`, on `func_illusionary` `*2` and `*29` (2 faces each) |
| P4 (install) | headers of `comshieldwall.vtf` / `comshieldwall_close.vtf` | 256 × 256 DXT1, 31 / 1 frames |
| P5 (install) | `particles/blood_impact.pcf` in HL2's misc VPK vs CS:S's | identical (134819 bytes) |
| P6 (install) | view-model events (`v_pistol`, `v_357`, `v_smg1`, `v_irifle`, `v_shotgun`, `v_rpg`) | as §4/§5/§15.3 (muzzle events at cycle 0; shotgun pump shell at 0.312; 357 loader sounds at 0.255/0.355/0.609/0.836; AR2 reload sounds at 0.021/0.404; RPG one numbered event with empty option) |
| P7 (probe) | `sv_soundemitter_trace 1` on the probe server; a bot shoots another with the pistol and SMG1 | per hit `Flesh.BulletImpact` from the victim; per kill `<type>.Die`; no shot sound reaches the shooter's own client from the server |
| P8 (probe) | energy-ball kill of a bot | no `.Die` emitted; the ragdoll entity removed ~134 ticks after the kill |
| P9 (probe) | bots of each model type running | step entries per §15.6, volume 0.2 running / 0.5 sprinting |
| P10 (probe) | frag and RPG explosions with the sound trace | `BaseGrenade.Explode` for the frag only; no server explosion sound for the RPG (the 140 dB boom is client-side) |
| P11 (probe) | cvars over RCON / client console: `muzzleflash_light`, `cl_ejectbrass`, `r_drawtracers`, `r_drawtracers_firstperson`, `cl_new_impact_effects`, `r_drawflecks`, `violence_hblood`, `cl_show_splashes`, `sv_footsteps`, `smoke_trail` | 1, 1, 1, 1, 0, 1, 1, 1, 1, 1 (names found in the shipped libraries) |
| P12 (install) | dm_steamlab's `env_explosion`s | 5, spawnflags 50, magnitude 125 ×3 and 150 ×2, render mode 5 |

## Open questions

1. **Own-bullet whizz.** Does a shooter in HL2:DM hear
   `Bullets.DefaultNearmiss` while firing (every SMG1 tracer at most every
   0.1 s), and `Bullets.GunshipNearmiss` with the AR2? The code says yes
   for any view-model muzzle within 24 (72) units of the shot line.
   Check on a listen client with `snd_show 1` while firing at a far wall;
   also confirm that standing next to a bot's line of fire gives no
   whizz.
2. **Duplicate `NPC_Citizen.die`.** Which definition wins (the first
   loaded, `hl2mp`'s 3 waves, or the last, HL2's 9)? Kill a citizen bot
   30 times with `sv_soundemitter_trace 1` and see whether `pain01–06`
   ever play. Also confirm the `$gender` rule (female models →
   `female01`).
3. **Third-person muzzle flash.** The code gives other players' guns only
   the model light. Confirm on a listen client with `host_timescale 0.1`
   watching a bot fire; if sprites appear, find their source (the world
   models' `AE_NPC_MUZZLEFLASH` events).
4. **Shell fade.** The fade-out length of HL2's shell temp entities after
   their 1–2 s life (view_models.md §8 has CS:S's 2 s rule for a
   different spawner); measure frames.
5. **Gain curve.** sounds.md Open questions 1 (level → gain against
   distance) applies unchanged; HL2:DM has no compatibility-attenuation
   entries to measure separately.
6. **RPG laser in third person.** The world model
   `w_rocket_launcher.mdl` has 3 attachments, but the third-person laser
   uses attachments 4 and 5: where do other players see the beam and glow
   (the entity origin, nothing)? Observe a bot guiding a missile.
7. **Shield wall patches and frames.** Confirm the patched materials keep
   the include's proxies (the map compiler copies only plain values, the
   engine's own patch loader is not in the SDK), and what `$frame2` beyond
   texture 2's single frame draws (frame 0 assumed). Compare a capture at
   200 and 600 units from a shield in dm_underpass.
8. **HDR default.** Does the game start with `mat_hdr_level 2` on this
   hardware, and do the stock maps look different enough in HDR (tone
   mapping by `env_tonemap_controller`) that refcmp must always force 0?
   H9 assumes captures at 0.
9. **Remote impacts through players.** Confirm on a listen client: a bot
   shooting a second bot point-blank shows the impact decal on the wall
   behind the victim, not on the victim.
10. **Melee blood for the attacker.** Confirm that a crowbar hit shows no
    blood on the attacker's own screen.
11. **Flesh sound twice.** Confirm the shooter hears two flesh impacts
    per hit (server + own client), and whether the engine merges them.
12. **Explosion sparks when the down line starts in water.** An
    `env_explosion` deeper than 8 units under water starts its 8-up /
    40-down line inside water; the engine's answer there (hit or no hit,
    which normal) is not in the SDK. Read as "no usable normal" (showers
    straight down at the origin + 8 + 24·0). Check: an `env_explosion`
    without flag 32 at the bottom of a deep pool; watch which way the
    sparks fly.
