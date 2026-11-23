# Characters fit the map's game

The user's design (2026-11-21): on a map, a character's movement body
fits the body the map was built for, so a big character from one game
gets through another game's doorways. The cause it fixes: TF2's player
box is 1.22 m wide (66 Combat Arms units), Combat Arms' hull 0.555 m and
CS:S's 0.81 m, and 72 of the 407 doorways measured on Combat Arms' maps
are narrower than a TF2 box (specs/combat_arms/movement.md, "World scale
(metres per unit)", "Door sizes").

## The rule (done)

1. Each game declares its player envelope: standing width, standing
   height, crouched height, the step it walks up and its eye heights
   (standing, crouched), the size its maps were built for (`chimera_core::envelope::MovementEnvelope`, metres).
   It comes from the game's own movement data, the same numbers its
   movement collides with: Source's config (`SourceMovementConfig::
   envelope`; CS:S, HL2:DM, TF2), Combat Arms' `data/movement.toml`
   (`movement::envelope`), Halo's table (`movement::envelope`). A map
   carries its game's (`core::MapGame`).
2. On a map every character's movement body is fitted to the map's
   envelope per axis, shrink only: width, standing and crouched height
   each the smaller of its own and the map's; a smaller character keeps
   its own body. The step is the higher of the two. Each shape fits its
   own way: a box per axis (`MovementEnvelope::fit_half_widths`: Source's
   box, Combat Arms' hull), a round body by its radius (Halo's pill). A
   sphere or capsule body (none yet) would take `radius` the same way.
3. Under a top the rule lowered, the eye goes `EYE_MARGIN` (0.1 m) under
   that top or to the map's own character's eye, whichever is higher,
   and never above the character's own (`MovementEnvelope::fitted_eye`),
   standing and crouched: a character sees no lower than the map's own
   players do unless its own eye is lower. First person
   and the third-person pivot follow `MovementState::eye_offset`, so no
   camera goes into a ceiling the body fits under.
4. The model, hit boxes and look stay full size: only what the character
   walks through changes.
5. Server and client prediction agree: the fitted body is derived from
   the loaded map and each game's data on both sides; nothing new goes
   on the network.

Where: `chimera/core/src/envelope.rs` (the rule and helpers);
`SourceMovementConfig::fitted` (every Source game; applied after a
character's traits, TF2's classes); Combat Arms' `movement::body`;
Halo's `movement::body`. Tests: `apps/party_roster/tests/it/character_fit.rs`
(a TF2 player through greyboxes of training's 136 x 56 u and
graveyard's 104 x 60 u doorways on a Combat Arms map, blocked without
the envelope, the eye under the top; a Combat Arms player's body the
same on a TF2 map, its width on a CS:S map), `halo_ce` `movement_envelope`.

Heights are the map game's movement boxes, eyes its own (fixed
2026-11-21): CS:S's 62 u standing and 45 u ducked
(specs/cs_source/movement.md, "Hulls, eyes and ducking") are the
collision box CS:S moves with, read every tick by the probe; the 8.5 u
air-duck lift is half their difference, and our movement collides with
them tick-exact. CS:S's eye (64 u, 47 ducked) is 2 u over that box, so
its maps' clearance is the box and its players see from above it. The
first version kept a lowered eye 0.1 m under the top, which put a TF2
player's eye on CS:S's maps at 58 u, under CS:S's own 64; now the eye
floor is the map game's own eye. Each game, fitted to another's map
(own box standing / crouched, own eye; metres):

| Game | Height | Crouched | Eye | Crouched eye | On CS:S maps |
|---|---|---|---|---|---|
| CS:S (62/45 u, eye 64/47) | 1.575 | 1.143 | 1.626 | 1.194 | own |
| HL2:DM (72/36 u, eye 64/28) | 1.829 | 0.914 | 1.626 | 0.711 | box 1.575, eye 1.626 (was 1.475) |
| TF2 (82/62 u, eye 68/45; classes' eyes over that) | 2.083 | 1.575 | 1.727 | 1.143 | box 1.575/1.143, eye 1.626 (was 1.475), crouched its own 1.143 (was 1.043) |
| Combat Arms (100/80 u at 0.0185) | 1.850 | 1.480 | 1.536 | 1.110 | box 1.575/1.143, eyes its own (were 1.475/1.043) |
| Halo (0.7/0.5 WU) | 2.134 | 1.524 | 1.890 | 1.067 | box 1.575/1.143, eye 1.626 (was 1.475), crouched its own |

The envelopes' sizes did not change; only a lowered body's eye. Each
game's own maps stay the identity, and a smaller game's character keeps
its own body. Tests: `character_fit`
(`a_combat_arms_player_keeps_its_body_on_a_tf2_map_and_its_width_on_cs_sources`:
its own eyes on CS:S's maps; `bigger_source_players_on_a_cs_source_map_see_from_its_own_eye`:
TF2 and HL2:DM at CS:S's 64 u), `chimera_core` `envelope` unit tests.

## Fixed from reports (2026-11-22)

- A Source box on another game's map collided with Combat Arms'
  traces-only surfaces (non-solid detail): a ramp of it filled a
  coldseed doorway. Source's movement now meets what every body meets
  (`core::CHARACTER_FILTER`), as Combat Arms' and Halo's do.
- A fitted Halo pill could not walk up Source stairs (dm_underpass, 12-16 u
  risers): its step-up landed the rounded bottom on the stair's nosing, a
  contact too steep to stand on, and it slid back. A landing on the edge of
  a walkable top now stands on that top (`mover::step_up`,
  `Space::ledge`).

Tests: `apps/party_roster/tests/it/heavy/report_spots.rs`.

## Later (not built)

- Host option "Fit characters to the map": collision only (today's
  behaviour, the default) or whole character: a uniform scale of the
  model, hit boxes, eye and box to the map's envelope (the largest
  scale at which the box fits), so a Spartan shrinks to the map's size
  as a whole. A server setting, sent to clients with the rules so
  prediction agrees.
- A per-character data opt-in to whole-character scaling (a character
  whose look depends on its size relative to the level), whatever the
  host option says.
- A sphere or capsule movement body fitting its radius (the hook is
  `MovementEnvelope::radius`; Halo's pill already does).
