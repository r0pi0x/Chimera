# Day of Defeat: Source: helmets, body groups and skins

Source basis: the twelve player models, the two dropped-helmet models
and the twelve class scripts in the dedicated server's content (read in
place: the models' body part, model, mesh, texture and skin tables; the
scripts' fields), and the dedicated server (build 11003710) measured
through `tools/dods_probe` (2026-11-24): every player's `m_nBody` and
`m_nSkin` read per class and team, and logged every tick around hits.
No game code was read.
Status: draft (data read, measured)

## Summary

A DoD:S soldier's look is his class's model: the team is in the model
(american_* or german_*), not in a skin, and every model has one skin.
The only switch is the helmet: each body has a helmet group with two
choices, the helmet on or nothing, and a soldier loses his helmet when a
bullet hits his head, lethal or not, until he respawns.

## Units and conventions

`m_nBody` is the usual Source body-group number: the sum over body parts
of `choice × base`, where each part's base is the product of the choice
counts of the parts before it.

## Constants

| Name (ours) | Value | Meaning |
|---|---|---|
| helmet on | body 0 | the helmet group's first choice (the class scripts' `helmetgroup` 0) |
| helmet off | body 1 | the helmet group's second choice, an empty model (the class scripts' `hairgroup` 1) |
| skin | 0 | every player model has one skin family |

## The models (data)

Every one of the twelve `models/player/<team>_<class>.mdl` (MDL v44) has
the same layout:

| Body part | Base | Choices |
|---|---|---|
| `body` | 1 | 0: the class's body (`<team>_<class>_reference`), 2 meshes: the team's `*_body` and `*_gear` materials |
| `helmet` | 1 | 0: the team's helmet (`<team>_helmet_reference`), 1 mesh on the team's `*_body` material; 1: empty (no meshes) |

- So `m_nBody` 0 draws the helmet, 1 draws none. There is no hair or
  head-variant model: "hair" is what shows with the helmet choice empty
  (the head under it is part of the body mesh).
- Textures: the American models use `american_body` and `american_gear`
  (from `materials/models/player/american/`), the German ones
  `german_body` and `german_gear` (`.../german/`). One skin family each.
  `american_sniper` also lists a third texture, `american_soldier`, that
  no mesh of it uses (it doesn't exist in the content; harmless).
- `dod_player_shared.mdl` (v48) holds the animations only: no meshes,
  textures or body parts.
- Dropped helmets: `models/helmets/helmet_american.mdl` and
  `helmet_german.mdl`, one body part, one mesh on the team's `*_body`
  material, one skin. The class scripts name them by `drophelmet`
  (`HELMET_ALLIES` for every US class, `HELMET_AXIS` for every Axis
  class).
- `materials/models/helmets/` (`axis_hat`, `axis_helmet`, `helmet_us`)
  is used by none of these models.

## Which class uses what (data)

Every class script (`scripts/playerclass_{us,axis}_*.txt`) gives
`helmetgroup` 0 and `hairgroup` 1; they differ only in `playermodel` and
`drophelmet`:

| Class | Model | drophelmet |
|---|---|---|
| US Rifleman, Assault, Support, Sniper, MG, Rocket | american_rifleman, _assault, _support, _sniper, _mg, _rocket | HELMET_ALLIES |
| Axis Rifleman, Assault, Support, Sniper, MG, Rocket | german_rifleman, _assault, _support, _sniper, _mg, _rocket | HELMET_AXIS |

No class has a cap, a second skin or a different helmet; the sniper
classes wear the same helmet as the rest.

## Behavior (measured)

### On spawn

- Every class of both teams spawns with `m_nBody` 0 (helmet on) and
  `m_nSkin` 0 (all twelve classes, each spawned by the class
  command and a respawn).
- A respawn puts the helmet back (body 0 after every respawn, also
  after dying helmetless).

### Losing the helmet

- A bullet that hits the head hit group (`hitgroup` 1) sets `m_nBody`
  to 1 on the same command, **every time** (8 of 8 head hits, the body
  reset to 0 before each), whether the hit kills or not.
- Hits elsewhere leave it on (chest hits, lethal or not: body stays 0).
- Once off it stays off for the rest of that life; further head hits
  change nothing.
- On a lethal head hit the body is 1 when the player dies, so the
  ragdoll is helmetless. The server's ragdoll entity carries no body or
  skin of its own (only the player it came from, the force and the
  velocity): the client builds it from the player.
- The flying helmet (`drophelmet`'s model) is not a server entity (no
  entity was created for it): a client effect, unmeasured here.

## Per-tick order

1. The bullet's trace hits the victim's head hitbox (hit group 1).
2. The same command: damage is dealt (×2.5 for the head, damage.md) and
   the victim's body is set to 1.

## Edge cases

- Other damage that isn't a traced bullet to the head (explosions,
  melee to the head, falling): unmeasured; only bullets were tried.
- The Allies' helmet loss was not measured (the victim was an Axis
  rifleman); the class scripts give both teams the same groups.

## Test cases

| Setup | Input | After | Expected |
|---|---|---|---|
| any class, either team | spawn | – | body 0 (helmet), skin 0 |
| Axis rifleman, 100000 health | Garand shot to the head | same command | body 1, health −200 |
| helmetless | another head hit | – | body stays 1 |
| Axis rifleman, 150 health | Garand shot to the head | – | dies with body 1 |
| Axis rifleman, 10 health | Garand shot to the chest | – | dies with body 0 |
| died helmetless | respawn | – | body 0 |
| US class model | – | – | body part `helmet` with 2 choices, the second empty; skin families 1 |

## Open questions

1. The dropped helmet's flight (client-side): speed, spin, lifetime; does
   it drop on a non-head death?
2. Head hits by melee or explosions.
3. Whether the client hides the helmet on its own (first person) or
   draws anything for the hair beyond the body mesh's head.
