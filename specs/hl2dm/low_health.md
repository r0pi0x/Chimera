# Half-Life 2: Deathmatch: low-health and damage cues for the local player

Source basis: the public Source SDK 2013 multiplayer game code (the base
player's damage path, its damage-type screen effects and explosion
hearing effect, the suit's voice queue, the HL2 player's damage
overrides, the client's screen-fade blending and the HL2 HUD's quick
info), read in place in a separate spec session; the game's
`hl2/scripts/dsp_presets.txt` and `hl2/scripts/game_sounds_ui.txt` (read
for numbers and names only); and a run of the real HL2:DM dedicated
server on this box (build 10889068) with the HL2:DM probe
(`tools/hl2dm_probe`), driving a bot with `probe_hurt` and reading
`hl2_episodic`, the bot's punch angle and every sound it made.
Status: draft

## Summary

HL2:DM tells a hurt player very little beyond the HUD. Every cue is in
this list; most are already specified elsewhere and this spec indexes
them with their numbers, says which ones we have, and specifies the
three that no spec covered (the damage view punch, the explosion ear
ringing and the drown/crush fades' fade maths). There is **no HEV suit
voice in multiplayer** (no "near death", no "seek medical attention"),
no heartbeat, no low-health colour grading or blur, and no pain voice.
Below 20 health the HEALTH panel pulses dark red every 0.8 s with red
digits; at 25 or less the left quick-info bracket by the crosshair
blinks orange-red and `common/warning.wav` plays once. That is the whole
"you are low" signal.

## Units and conventions

Source units (1 unit = 1 inch), degrees, seconds of server or client
time; server tick 1/66.67 s on stock HL2:DM. Colours are R G B A, 0–255.
Pitch is positive downward (a negative pitch punch kicks the view up).

## Constants

| Name (ours) | Value | Unit | Meaning |
|---|---|---|---|
| hud_low | 20 | hp | below (and above 0): `HealthLow` pulse (hud.md 4.3) |
| quickinfo_low | 25 | hp | at or below: quick-info blink + `HUDQuickInfo.LowHealth` (hud.md 6.2) |
| dmg_high | 25 | hp | health taken above this in one damage message: the "high" indicator (hud.md 8) |
| damage_punch | −2 | degrees pitch | punch angle **set** (not added) on every damage taken |
| ring_distance | 240 | units | blast inflictor closer than this (strictly): ear ringing |
| shock_damage | 30 | hp | blast damage (after armour) at or above this: shock muffle instead |
| dsp_ring | 32, 33, 34 | preset | ear ringing, one chosen uniformly |
| dsp_shock | 35, 36, 37 | preset | shock muffle, one chosen uniformly |
| dsp_hold | 1.6 | s | how long a 32–37 preset holds (`dsp_presets.txt` duration) |
| crush_fade | 128 0 0, alpha 128; fade 1.0 s, hold 0.1 s | | crush damage |
| drown_fade | 0 0 128, alpha 128; fade 1.0 s, hold 0.1 s | | drowning damage |

## Behavior

### 1. What the game does NOT have (checked)

- **Suit (HEV) voice.** The suit's sentence queue returns at once in
  any multiplayer game, both when a line is queued and when the queue is
  played. The damage path still works out its single-player diagnosis
  (fracture, blood loss, "health critical" below 20, "near death" below
  6, "seek medical attention" below 50, automedic/morphine after a
  first big hit from 75 or more), but nothing is ever heard. Observed:
  a bot hurt 100 → 70 → 20 → 5 by bullets and then by drowning made only
  `physics/flesh/flesh_impact_bullet*.wav` (auto channel, 0.7, level
  75) per hit and `player/pl_drown1.wav` for the drowning hit; no suit
  sound. Keep this absence.
- The suit's **Geiger counter** update still runs in multiplayer (it is
  before the multiplayer return), but only radiation sources drive it;
  no stock HL2:DM map has one. Not a health cue.
- **No pain voice** (effects_sounds.md 15.7), no heartbeat, no
  low-health post effect (no desaturation, vignette or blur), no
  low-health sound loop.
- The stunstick's red screen fade on its victim (128 0 0, alpha 128,
  0.5 s, hold 0.1 s, plus a large view kick) belongs to the stunstick's
  NPC animation-event path: a player's swing never reaches it, so in
  HL2:DM a stunstick hit gives the victim neither.

### 2. HUD cues (already specified; index)

| Cue | Where | Trigger | Look/sound |
|---|---|---|---|
| HEALTH digits flash + glow on any damage | hud.md 4.3 `HealthDamageTaken` | health damage > 0 in a damage message | digits → `BrightFg` 255 220 0 then back over ~1 s; glow 3 → 0 over 2 s |
| HEALTH low pulse | hud.md 4.3 `HealthLow`, `HealthPulse`, `HealthLoop` | shown health changes to 1–19 | box flashes `DamagedBg`, then pulses 100 0 0 80 every 0.8 s; digits settle on `DamagedFg` 180 0 0 230 (red) and stay red until health changes to ≥ 20 |
| SUIT panel | hud.md 4.4 | armour falls / reaches 0 | flash on loss; fades out over 0.4 s at 0 (no red: quirk) |
| Quick-info health bracket | hud.md 6.2 | health changes to ≤ 25 from above | left bracket blinks in `Caution` 255 48 0 for about 1.3 s, then the bar stays `Caution` (pulsing alpha) while ≤ 25; `HUDQuickInfo.LowHealth` once per crossing |
| Damage direction | hud.md 8 | each damage message with a non-world inflictor | orange→red side band left/right (both for behind) fading 0.8 s; heavy hit (> 25) a half-screen gradient or full-screen 255 88 0 → red fading over 2.8 s; poison, burn, radiation/acid, drown variants |
| Death | hud.md 8 `HudPlayerDeath` | health ≤ 0 | full screen → 255 0 0 96 by 0.3 s, then → opaque red over 4 s |

`HUDQuickInfo.LowHealth` (Half-Life 2's `scripts/game_sounds_ui.txt`):
item channel, volume 0.8, normal pitch, sound level none (unspatialised,
2D), wave `common/warning.wav`. Played locally from the client, only to
the hurt player.

### 3. View punch on damage (new)

Every time the player actually takes damage (after armour, god mode and
friendly-fire refusals; any amount, any type), the server **sets** the
pitch of the punch angle to −2° (the view kicks up 2°), leaving yaw and
roll as they were. It replaces whatever punch was there (a weapon's
recoil punch is overwritten, not added to). HL2:DM is built without
the episodic option (`hl2_episodic` reads 0 on the server; checked),
so the episodic variant (−10° or −5…−7° from an unseen attacker) never
applies. The punch then decays by the normal punch spring
(movement.md, weapons.md); with nothing else acting, the probe read
−1.89° to −1.96° about one tick after the hit and +0.14° to +0.17°
0.3 s later (the spring overshoots slightly before settling).

Several hits in one tick: each sets −2 again (no build-up). Fall damage
sets it as well (the fall's own landing punch is movement.md's).

### 4. Explosion hearing effect: ear ringing and shock (new)

After a blast-type hit that the player took (any explosion: frag, RPG,
SMG grenade, SLAM, barrels, the combine ball's blast if it carries the
blast bit), with D = the damage left after armour (a float, before
the conversion to whole health) and r = distance from the player's
origin (feet) to the inflictor's origin (9999 if there is no inflictor):

1. shock = D ≥ 30; ring = r < 240.
2. Neither: nothing.
3. Otherwise the server tells **only that player's** client to switch
   its player DSP preset to a uniformly random one of 35–37 if shock,
   else of 32–34.
4. The preset holds 1.6 s, then fades back exponentially to the
   previous preset (`dsp_presets.txt`: duration 1.6, fade −1 =
   exponential; mix range 0.2–0.7).
5. At spawn the player's DSP preset is set to 0 (none) again.

The presets (game data, `hl2/scripts/dsp_presets.txt`, "system presets"):

| Preset | Name in the file | Processing |
|---|---|---|
| 32, 33, 34 | explosion ring 1–3 (identical) | a diffusor (size 1.0, 3 delays, feedback 0.15) into a low-pass at 1000 Hz, gain 0.25: everything muffled and smeared |
| 35, 36, 37 | shock muffle 1–3 (identical) | a diffusor (size 1.0, 2 delays, feedback 0.15) and a sine LFO (rate 3000, gain 0.25): a warbling, ringing tone over muffled sound |

There is no "ringing" sample; the ring is the DSP. Same base-player
rule as CS:S (specs/cs_source/grenades.md "base-player hearing", tests
G23/G24); TF2 uses the same rule and presets from its own player
damage path. In HL2 single player a mortar shell skips the ringing
(screen shake instead): no mortars exist in HL2:DM.

### 5. Damage-type screen fades (fade maths; values already in effects_sounds.md 7)

On a hit with the crush bit the victim's screen fades from 128 0 0 at
alpha 128; with the drown bit (and no crush bit), from 0 0 128 at alpha
128. Plasma damage (blue modulate fade) has no source in HL2:DM.
Each fade, from the moment the client receives it (time t₀):

- hold h = 0.1 s, fade d = 1.0 s, peak alpha A = 128;
- alpha(t) = clamp(A · (t₀ + h + d − t) / d, 0, A): A until t₀ + h,
  then linearly to 0 at t₀ + h + d; removed after that.
- Several fades at once: the drawn colour is the **average** of their
  colours, the alpha the **largest** of their alphas; drawn as a
  normal alpha blend over the 3D view (not over the HUD).
- Durations travel in 1/512 s steps (the client rounds down).

### 6. CS:S and TF2 (where shared)

The punch, the hearing effect and the fade maths are the shared base
player's and client's: CS:S and TF2 differ only where their own player
code overrides them (CS:S's in specs/cs_source; TF2 sets punch through
its own helper and has its own damage-type effects; neither is
specified here).

## Per-tick order (server, one damage event on the player)

1. Refusals (dead, god mode, team rules, spawn protection vs SLAM).
2. Armour absorbs (suit.md); D = remaining damage.
3. Health reduced (whole part); last-damage bookkeeping; damage message
   queued for the client (hud.md 4.7).
4. Damage-type screen fade (crush, else drown).
5. Suit diagnosis (silent in multiplayer).
6. Punch pitch := −2.
7. Blast bit: hearing effect (section 4).
8. Damage sounds (effects_sounds.md 15.7).

Client: the damage message drives the HUD (hud.md 4, 6, 8) next frame;
the fade and DSP messages arrive in the same snapshot.

## Edge cases

- Lethal hit: the punch and ringing still apply (they come before death
  handling); the HUD shows `HudPlayerDeath`, not a damage flash.
- Quick-info warning compares with the last *seen* health: health
  regained above 25 and lost again re-plays the sound; spawn resets the
  memory to 100.

## Quirks

- The suit computes its diagnosis every hit and throws it away (keep:
  silent).
- Punch is set, not added: a hit during heavy recoil *reduces* the kick
  (keep).

## Test cases

| Setup | Input | After | Expected |
|---|---|---|---|
| health 100, armour 0 | 30 bullet damage | 1 tick | punch pitch ≈ −2 (−1.9 after one decay step); no suit sound; one `Flesh.BulletImpact` |
| punch pitch −6 from recoil | 1 damage | same tick | punch pitch −2 |
| health 70 → 20 → 5 | bullet hits | – | no sound other than flesh impacts; no `!HEV_*` line ever |
| blast, D 35, inflictor 500 units away | – | – | DSP preset ∈ {35, 36, 37} for that player only |
| blast, D 20, inflictor 200 units away | – | – | DSP preset ∈ {32, 33, 34} |
| blast, D 20, inflictor 250 units away | – | – | DSP unchanged |
| any preset 32–37 set | wait | 1.6 s | preset fading back (exponential) |
| DSP 34 active | respawn | – | DSP 0 |
| crush hit at t₀ | – | t₀+0.05 / t₀+0.6 / t₀+1.2 s | alpha 128 / 64 / gone, colour 128 0 0 |
| crush fade and drown fade both at t₀ | – | t₀ + 0.05 s | colour 64 0 64, alpha 128 |
| health 30 | 10 damage | – | quick-info blink + `common/warning.wav` once (0.8, 2D); HEALTH not yet low-pulsing (20 is not below 20) |
| health 20 | 5 damage | – | `HealthLow`: dark red pulse every 0.8 s, red digits |

## Implemented in Chimera? (checked 2026-11-21 against games/hl2dm)

| Cue | Implemented |
|---|---|
| HEALTH/SUIT panel flashes, low pulse, red digits | yes (`games/hl2dm/src/ui/numeric.rs`, driven by the install's animation script) |
| Quick-info low-health blink and `HUDQuickInfo.LowHealth` | yes (`ui/quick.rs`, sound through `ui/systems.rs`) |
| Damage direction bands and full-screen flashes | partly: present (`ui/damage.rs`), but the side shapes are straight bands drawn alpha-blended, not the additive trapezoids |
| Death red fade | yes (`ui/damage.rs`) |
| Crush and drown screen fades | yes (`game/projectile_fx.rs`, Chimera's `ScreenFades`) |
| Damage view punch −2° | **no** |
| Explosion ear ringing / shock DSP | **no** for HL2:DM (CS:S has the DSP presets in `games/cs_source/src/game/grenades.rs`; not shared with HL2:DM) |
| Suit voice, heartbeat, low-health post effect | correctly absent |

Note for the party apps: the HL2:DM HUD (and so every HUD cue above)
shows only while an HL2:DM gun is the HUD's owner; a player holding
another game's gun gets that game's HUD and none of these cues. The
punch and the ringing belong to the player, not the HUD, so they would
apply whatever is in hand if implemented on the player.

## Open questions

1. HL2:DM's own `hl2mp` VPK may override `game_sounds_ui.txt`; only
   Half-Life 2's loose copy was read for `HUDQuickInfo.LowHealth`.
   Check with a VPK listing.
2. The DSP presets' exact audio (diffusor, LFO "rate 3000") was not
   listened to: a muted run cannot. Confirm by ear on the Windows PC or
   by recording the client with `dsp_player` echoed (`snd_show`), as
   CS:S's Q11.
3. Whether the quick-info sound still plays with `hud_quickinfo 0`
   (the health check sits in the element's paint; not checked).
4. Client-side confirmation of the fades and punch was done on the
   server only (punch angle read through the probe); a client capture
   (`HudTakeDamage*` frames and the −2° kick) would close it.
