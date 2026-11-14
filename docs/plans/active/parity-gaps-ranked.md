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

Decided (user, 2026-11-12): HL2:DM's pickups and chargers stay off in
Ultimate Gun Game (was item 4): a party's HL2:DM map places none of its
items and wall chargers, on the server and the client (`KitLoad::
pickups`); its ammo crates stay as inert solid models (the spec makes a
crate a static solid model that blocks players and bullets: level
geometry). docs/DESIGN.md, "Gun game rules".

Decided (user, 2026-11-12): an HL2:DM rung gives 2 alt-fire rounds (was
item 8). The cause, traced: each life the ladder gives a new weapon
(`runtime::respawn` strips and gives; a rung swap gives too), and its
hooks build `Hl2Rounds` from the data's `count`, 0 (HL2:DM spawns with
none: its rounds come from boxes and crates, `ammo::give`); the ladder's
`refill_ammo` tops up only `Magazine` reserves and thrown grenades, and a
party has no pickups, so attack2 clicked empty from the first press. Now
the rules' weapons are `minigame::LoadoutWeapon`s and the SMG1's and
AR2's hooks say `loadout = 2`.

Decided (user, 2026-11-12): ladders cross every game (was item 6): each
map's ladders are published in one form (`core::MapLadders`,
`LadderZone`) and each movement climbs them by its own rules; each game's
own maps unchanged. docs/DESIGN.md, "Ladders across games".

## Do next (spec is clear)

Nothing: every row that was here has landed (view model FOV, melee lag
compensation, Combat Arms' Flash/Breach sockets, hands, launcher pieces
and back push, HL2:DM's items, crates, prop rules and blast hearing).

## Later (spec is clear, less often seen)

20. **HL2:DM / UGG: flashlight and suit zoom inconsistent across
    rungs.** `mp_flashlight` 0 (parity) so F does nothing on an HL2
    rung; `+zoom` unbound in the party bundle. Spec: hl2dm/suit.md
    "Flashlight" (Q7), "Suit zoom". Code: `games/hl2dm/src/game/suit.rs`,
    `mod.rs` `BUNDLE.binds`. S. No spec; the user's call.
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
29. **CS:S: HE explosions' `^` near/far blend is a mono downmix.** The
    two channels (left near, right far) now play as their average, not
    blended by the listener's distance. Spec: cs_source/sounds.md 1 "Wave
    name prefixes" (gives no curve); grenades.md 5.4. Code:
    `source/engine/src/content/sound.rs` `wave_distant` (Known shortcuts).
    S once measured. Spec: the near/far curve (a capture).
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

Do next 0, later 2, needs a spec or capture 10.
