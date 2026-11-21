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
   height, crouched height and the step it walks up, the size its maps
   were built for (`chimera_core::envelope::MovementEnvelope`, metres).
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
3. The eye is kept `EYE_MARGIN` (0.1 m) under a top the rule lowered
   (`MovementEnvelope::eye_within`), standing and crouched; first person
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
Halo's `movement::body`. Tests: `apps/party_kit/tests/it/character_fit.rs`
(a TF2 player through greyboxes of training's 136 x 56 u and
graveyard's 104 x 60 u doorways on a Combat Arms map, blocked without
the envelope, the eye under the top; a Combat Arms player's body the
same on a TF2 map, its width on a CS:S map), `halo_ce` `movement_envelope`.

Note: CS:S's measured box is lower than its model (62 u standing, 45
ducked: 1.57 m and 1.14 m; its eye, 64 u, is above the box's top), so a
Combat Arms player (1.8 m, crouched 1.44 m) on a CS:S map keeps its
width but takes CS:S's heights, its eye 0.1 m under the top (1.47 m
against its own 1.49 m). That is the rule as stated; if it reads wrong
in play, the heights could fit to the map game's eye instead of its box
(the user's call). On a TF2 map nothing changes (TF2's box is bigger
every way).

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
