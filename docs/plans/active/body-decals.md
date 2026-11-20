# Decals on character bodies (model decals)

Goal: every bullet, pellet and melee hit on a character puts the hit
surface's decal on the drawn body, following its animated skin, for every
game that does so; data-driven, capped per body. Specs:
[specs/tf2/effects.md](../../../specs/tf2/effects.md) §8b (TF2: `Impact.Flesh`,
the rules below), [specs/cs_source/weapons.md](../../../specs/cs_source/weapons.md)
(the bullet's impact: "studio decals" on players, by the hitbox surface's letter) and
ragdolls.md 2.4 step 9 (CS:S: the model instance, decals and all, moves to
the ragdoll), [specs/hl2dm/effects_sounds.md](../../../specs/hl2dm/effects_sounds.md) 6
("Flesh": the decal as any impact) and player_models.md 10.2 (same
ragdoll hand-over); [specs/combat_arms/effects_sounds.md](../../../specs/combat_arms/effects_sounds.md)
case 20: Combat Arms puts none on characters. Halo: no spec, none.

## Design

- **Sim side (source/engine `impacts::impact_decals`)**: a shot,
  continued shot or swing whose hit is a character writes
  `map::PlaceBodyDecal` (target, shooter, ray start, hit point,
  direction, group) instead of nothing. The group is the one a bullet on
  the game's flesh surface takes (`ImpactStyle::flesh` → its game
  material letter → `DecalGroup::GameMaterial`, from the install's
  `scripts/decals_subrect.txt`: TF2's F is `Impact.Flesh`); a melee
  style's own decal (CS:S's slash) as on the world. Whether a game decals
  bodies is its style's `body_decals` (`BodyDecals::Never/Always/
  NotLowViolence`): CS:S and HL2:DM `Always`, TF2 `NotLowViolence`
  (`violence_hblood 0` drops it, §8b). No new fixed-tick system (the
  schedule ratchet stays).
- **Engine rules (chimera_world `map::decal`, chimera_present
  `render::body_decals`)**: `NoBodyDecals` on a character: none placed
  while it is there, and its arrival clears them (TF2's cloak, disguise
  and Über will insert it; disguise changes write `ClearBodyDecals`);
  `ClearBodyDecals(character)`; every game clears on respawn (`Dead`
  removed); `BodyDecalsOffRagdoll` on a character: its ragdoll does not
  keep them (TF2, inferred in §8b; CS:S/HL2:DM keep them as the body moves
  to the ragdoll, which ours does already). Not on a teammate with
  friendly fire off, nor on the shooter. `r_drawmodeldecals` (0 hides
  them) and `r_maxmodeldecal` (cap per body, default **50**: §8b's
  "believed 50", Open questions 15), oldest replaced.
- **TF2 (games/tf2)**: heal to full (health rose and ≥ max) writes
  `ClearBodyDecals`; TF2 characters get `BodyDecalsOffRagdoll`.

## Placement and drawing (performance)

Chosen: **per-vertex decal geometry in bind space, skinned by the body's
own joints** (Source's studio-decal approach). On a hit, the body's shown
parts' vertices (from `CharacterModels`, the main world keeps no mesh
data) are skinned once on the CPU with the drawn joints (one pass per body
per frame, however many pellets hit it); the ray from the shot's start
through the hit point picks the nearest front-facing triangle, and the
triangles within the decal's reach of that depth, facing the shot, are
clipped to the decal's rectangle in the plane across the shot. Each
clipped vertex keeps its barycentric place in its source triangle, so its
bind-pose position, normal and joint weights (the three corners' blended,
top four) come from the bind mesh. A body's decals (≤ cap) are one mesh
per decal material, a `SkinnedMesh` child of the body with the body's
joints and inverse bind poses: the GPU skins them with the body, they go
with the body to a ragdoll, and they cost one draw per decalled body and
material and nothing per frame on the CPU (no per-frame reprojection).
Rejected: re-projecting every frame (CPU per frame per decal), screen-space
or deferred decals (don't follow the skin, smear across limbs), painting
into a per-body texture (unique UVs and a texture per body).

Cost on a hit: skinning ≈ the body's vertex count × its weights (a few
thousand vertices) and a pass over its triangles; rebuilding the body's
decal mesh (≤ 50 decals). Nothing per tick without hits; perf tests
unaffected.

## Steps

1. [x] Plan (this file).
2. [x] Messages and components (chimera_world), style field, impacts
   writing `PlaceBodyDecal` (source/engine); TF2/CS:S/HL2:DM styles.
3. [x] `render::body_decals`: projection, skinned mesh, cap, clears;
   cvars.
4. [x] TF2 rules (heal to full, ragdoll).
5. [x] Tests: impacts write/skip (TF2 low violence, CS:S), the present
   rules (cap, clear, `NoBodyDecals`, respawn, teammates), TF2 heal.
6. [ ] Screenshot of a decalled body; docs (backlog, parity-gaps,
   tf2-parity, tech-debt via `//!`), docs_index.

## Known shortcuts (expected)

- Decals sit on LOD 0's surface, lifted along the normal; a coarser level
  may hide or float them a little.
- Face flexes don't move decals on the face.
- HL2:DM police's `no_decal` hitboxes still take them (per-hitbox
  surfaces aren't carried).
- TF2 needles' `Impact.Concrete` on bodies (§12) not done.
