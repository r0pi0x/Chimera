# Parity gaps, ranked (CS:S, Combat Arms, HL2:DM)

Audit 2026-11-12, at bef5d785: where our three games differ from the
originals as `specs/` describes them, ranked by what a player of
Ultimate Gun Game notices most (every rung's guns, maps and bodies from
all three games). Parity first: match the original by default,
enhancements as opt-in toggles later. Each gap was checked in the code;
docs can be stale (see the end).

Not repeated: the CS:S inventory by area, with every measured row, is
[parity-gaps.md](parity-gaps.md); what settles each open question is in
[open-questions.md](open-questions.md). The simulation side of CS:S
(damage, falloff, recoil for all 24 guns, movement, triggers, grenades'
damage) is measured and matches; most of what's left there is client
look and sound.

Each gap: game, what differs, spec section, our code, size (S hours, M a
day or two, L more), and whether it needs a spec session (or a capture)
first, done on the Windows PC. Delete a row when it lands.

## Do next (spec is clear)

1. **HL2:DM: view model drawn at 69 degrees, not 54.** `view_model_fov`
   is fov - (default_fov - player fov) = 54 - (75 - 90); HL2:DM pins
   `default_fov` at 75 where the spec has it follow `fov_desired`, so at
   UGG's 90 every HL2 gun looks small and far. Spec:
   hl2dm/player_models.md 12.4 "FOV", cases 51-52. Code:
   `games/hl2dm/src/game/view.rs`, `chimera/world/src/map/view_model.rs`
   `view_model_fov`, `apps/party_kit` `add_view_model_fovs`. S. No spec.
2. **Combat Arms: knife and melee swings not lag compensated.** The
   server checks a swing against where victims are now; guns are already
   compensated. Online, swings at moving targets miss, and the ladder
   ends on a knife. Spec: combat_arms/inventory.md 8.3 "The attack ray".
   Code: `games/combat_arms/src/game/inventory.rs` `swing_knives` (no
   `weapon::lagcomp`; shortcut in `game/net.rs`). S-M. No spec.
3. **Combat Arms: others' muzzle flash, tracer start, casings and shot
   sound come from the hand, not the barrel** (25-39 units short). Spec:
   combat_arms/effects_sounds.md, third-person effect points (`Flash`,
   `Breach` sockets). Code: `games/combat_arms/src/game/effects/mod.rs`
   (`hand` vs `flash`); `CaPose::flash_point` exists, used by launchers
   only; `Breach` not posed. S. No spec.
4. **HL2:DM: pickups and chargers dead in UGG.** `party_sim` adds no
   `PickupsPlugin`: no kits, batteries, ammo or crates on dm_ maps, and
   the 20 drawn wall chargers do nothing on `+use`. Its resources are
   global, so gate them to HL2:DM maps. Spec: hl2dm/maps_entities.md 1-4;
   suit.md "Pickups", "Wall chargers". Code:
   `games/hl2dm/src/game/mod.rs` `party_sim`, `apps/party_kit`
   `PartyPartsPlugin`; `apps/ultimate_gun_game/tests/it/pickups.rs`. M.
   No spec; the user decides whether UGG wants pickups (parity says yes).
5. **HL2:DM: items and ammo crates invisible, in every app.** Pickups
   spawn as mesh-less 16-unit boxes (the data's `model` is never read);
   crates are invisible solid blocks. Spec: hl2dm/maps_entities.md 1, 3
   (touch box: bounds grown 24 sideways, 12 up). Code:
   `games/hl2dm/src/game/pickups.rs`, `ammo_crates.rs`. M. No spec.
6. **Ladders don't cross into HL2:DM's movement.** An HL2 rung can't
   climb CS:S ladder brushes or Combat Arms ladder zones
   (`contents_ladders: false`); a CS:S rung can't use
   `func_useableladder`. Crossing was fixed for CS:S and Combat Arms on
   2026-10-27, before HL2:DM came. Spec: silent on mixing (DESIGN.md
   rules apply). Code: `games/hl2dm/src/game/movement.rs`,
   `source/engine/src/source_movement/mod.rs`. M. No spec; a design call.
7. **HL2:DM: prop physics rules missing in UGG.** Without
   `Hl2PhysicsPlugin`, dm_ maps' `prop_physics_multiplayer` (92 on
   dm_underpass) follow CS:S's walk-through rule, hit players at impact
   scale 1 not 4, and barrels use CS:S's blast. Gate per map's game.
   Spec: hl2dm/source_deltas.md 1.5; gravity_gun.md Corrections 1-2, 9.
   Code: `party_sim`, `games/hl2dm/src/game/physics.rs`. M. No spec.
8. **HL2:DM: SMG1 and AR2 alt fire empty in UGG.** Rungs seem to give
   no `smg1_grenade`/`ar2altfire` ammo and there are no pickups, so
   attack2 clicks (traced to `give_item`, not to the end). Code:
   `chimera/sim/src/minigame/`, `chimera/sim/src/weapon/equip.rs`. S. No
   spec; the user's call on how much.
9. **Combat Arms: first-person hands always the default character's.**
   UGG wears a random body per rung; sleeves, sex and gloves never match.
   Spec: combat_arms/view_models.md 2. Code: `game/view_model/on_demand.rs`,
   `view_model.rs` `add_view_models` (always `HandProfile { character:
   true, .. }`; the cache needs a key per hand). M. No spec.
10. **HL2:DM: blasts have no hearing effects** (ringing within 240,
    shock muffle). `hl2_blast` sets `hearing: None`; CS:S has the
    pieces. Spec: hl2dm/projectiles.md 2.2, case 63. Code:
    `games/hl2dm/src/game/weapons/launch.rs`. S. No spec.
11. **Combat Arms: launcher rockets and hidden pieces always drawn.**
    The view-model machine tracks `Pieces` but nothing draws from it; an
    RPG-7 keeps its rocket after firing. Spec: combat_arms/view_models.md
    3 "Hidden pieces", "Projectile piece", case 37. Code:
    `games/combat_arms/src/game/view_model/machine.rs`; data lacks
    `ProjectilePiece`/`HiddenPiece#`. S-M. No spec.
14. **Combat Arms: no per-shot back push** (`BaseCamBackJitter`,
    recovering at 5 x `BackJitterRecover`/s); read and unused, default
    0.5. Spec: combat_arms/weapon_firing.md "Recoil (view kick)". Code:
    `games/combat_arms/src/game/weapons.rs` `back_jitter`. S. No spec.

## Later (spec is clear, less often seen)

20. **HL2:DM / UGG: flashlight and suit zoom inconsistent across
    rungs.** `mp_flashlight` 0 (parity) so F does nothing on an HL2
    rung; `+zoom` unbound in the party bundle. Spec: hl2dm/suit.md
    "Flashlight" (Q7), "Suit zoom". Code: `games/hl2dm/src/game/suit.rs`,
    `mod.rs` `BUNDLE.binds`. S. No spec; the user's call.
21. **HL2:DM: `func_clip_vphysics` lets frags, items and weapons
    through** (they're on `ITEM_LAYER`). Spec: hl2dm/maps_entities.md 11,
    case 40. Code: `chimera/world/src/map/physics_clip.rs`. S. No spec.
23. **HL2:DM: held objects don't slide along walls or ramp their
    spin** (gravity gun controller). Spec: hl2dm/gravity_gun.md 5. Code:
    `chimera/world/src/map/held.rs` `drive`. M. No spec. Not on UGG
    ladders yet.
24. **HL2:DM: dm_underpass's shield walls drawn plain**
    (`UnlitTwoTexture`). Spec: hl2dm/effects_sounds.md 13, cases 51-54.
    Code: `source/engine/src/content/material.rs`. M. Q7 (proxies) needs
    a capture.

## Needs a spec or a capture first (Windows PC)

25. **CS:S: gunshot loudness by distance is a guess** (`distance_gain`
    on the spec's hypotheses H1/H2: either 140 dB carries ~360k units or
    the AK goes silent past 1923). Every fight is placed by ear. Spec:
    cs_source/sounds.md "Loudness and panning", Q1. Code:
    `chimera/world/src/map/sound.rs` `distance_gain`. S once measured
    (probe's `mashup_gain`, open-questions.md).
26. **CS:S: the dynamic crosshair follows spread; CS:S's grows per shot
    from `CrosshairMinDistance`/`DeltaDistance`.** Spec lists the keys
    only. Spec: cs_source/weapons.md 1 "Weapon script data". Code:
    `chimera/client/src/client/hud.rs` crosshair gap. M. Spec: the rule
    and the per-weapon values.
27. **CS:S: view-model muzzle flash look is ours** (material, size,
    life, `MuzzleFlashScale`). Spec: cs_source/view_models.md 7, Q5.
    Code: `chimera/present/src/render/view_model.rs`. M. Capture C15.
28. **CS:S: bullet impacts are the SDK template's effects** (counts,
    colours, timings, exit marks). Spec: cs_source/impact_effects.md
    2-8, Q1, Q3, Q5, Q7. Code: `source/engine/src/effects/impact_effects.rs`.
    M. Client captures.
29. **CS:S: HE explosions play near and far channels into separate
    ears** (the `^` prefix stripped). Spec: cs_source/sounds.md 1 "Wave
    name prefixes"; grenades.md 5.4. Code:
    `source/engine/src/content/sound.rs` `WAVE_PREFIXES`,
    `chimera/present/src/audio/live_sound.rs`. S (downmix now, the blend
    curve once specified).
30. **CS:S: others' feet slide** (template ground speeds 100/250/85 vs
    the models' ~107/150/67; no death layer). Spec:
    cs_source/animation.md 12, open questions. Code:
    `games/cs_source/src/game/player_anim.rs`. M. Capture C14.
31. **Combat Arms: scopes silent, no focus blur** (`FocusTex`). Spec:
    combat_arms/weapon_classes.md zoom; attachments.md (zoom sounds not
    named). Code: `game/weapons/zoom.rs`, `ui/scope.rs`. S-M. Spec: the
    zoom sound files.
32. **Combat Arms: no burn or poison screen effects.** Spec:
    combat_arms/effects_sounds.md Q26. Code: missing
    (`game/volumes.rs` notes it). M. Spec: the damage-effect data.
33. **CS:S: tracer every 4th bullet, and the damage indicator's side
    rule, are ours.** Spec: cs_source/tracers.md 1, Q1-Q2; damage
    indicator unspecified (parity-gaps.md, capture C8). Code:
    `games/cs_source/src/game/impact_effects.rs` `TRACER_EVERY`,
    `games/cs_source/src/ui/hud_sprites.rs`. S each. Captures.
34. **CS:S: flashbang and HE feedback curves approximated** (flash
    hold-and-fade, DSP presets 134-136, ring, shake). Spec:
    cs_source/grenades.md 5.4-5.5, 6.3-6.4, Q9, Q11, Q13-Q14. Code:
    `chimera/sim/src/weapon/grenade.rs`. M. Client captures.

Lower, not ranked: CS:S silenced USP/M4A1 numbers (a human on the
probe), penetration fits (weapons.md M13), scope overlay and zoomed
sensitivity, view-model lighting and bob, room reverb presets, spread
generator (Q1, Q12); Combat Arms crossbow alt fire, kill call-out voice
files, hit-sphere priorities (Q16), multi-kill reset on death
(kill_streaks.md Q2), M92FS post-fire 0.15; HL2:DM crossbow bolt
sounds, blood per viewer (effects_sounds.md 7, Q10), bolts through
grates and glass, damageable projectiles, the combine camera.

## Counts

Do next 17, later 7, needs a spec or capture 10.

## Stale `//!` docs found on the way

`games/cs_source/src/game/weapons.rs` (recoil measured for a few guns:
all 24 are), `chimera/sim/src/weapon/random.rs` (no lag compensation:
there is), `games/combat_arms/src/game/weapons.rs` (view-model jiggle not
done: it is), `games/combat_arms/src/game/special/mod.rs` (launchers
without view models), `games/hl2dm/src/game/pickups.rs` (respawn light),
`launch.rs` (spawn grace), `frag.rs` (gravity-gun catch), `rungs.rs`
(bodies), `physcannon.rs` (claims a shake at the ball's end),
`source/logic/src/logic/community.rs` (`env_shake` is networked).
