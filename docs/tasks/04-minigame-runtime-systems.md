# Minigame runtime's per-tick work as normal systems

**Done 2026-10-30** (docs/plans/active/ecs-cleanup.md, step 6: what
landed; `join` stayed a system, the reason there).

## Context

Every minigame runs on `chimera_sim::minigame::runtime`. Its per-tick
`end_protection`, `respawn_dead` and `hold_the_end` take the whole world;
`join` polls for new players. Step 6 of
[docs/plans/active/ecs-cleanup.md](../plans/active/ecs-cleanup.md).

## Files

- `chimera/sim/src/minigame/runtime.rs` (`respawn_dead`, `end_protection`,
  `hold_the_end`, `join`, `number_players`)
- Guards: `chimera/sim/tests/it/minigame.rs`;
  `apps/party_roster/tests/it/gun_game.rs`, `protection_look.rs`,
  `swap_effect.rs`, `rotation.rs`, `net_gun_game.rs`

## Definition of done

- [x] The three are systems with declared queries (or a one-shot system
      only for the part that must spawn); `join` is an observer of the
      joining moment (`On<Add<Team>>` already numbers players).
- [x] Spawn protection still ends on the first attack in the same tick and
      before that tick's damage (a test fires on the protection's last
      tick).
- [x] Guards and schedule ratchets pass; exclusive ratchet lowered.

## Constraints

**Order flag O1:** `end_protection` removes components the same tick's
damage reads. Chain it before damage and apply its `Commands` at a sync
point damage follows.

## Out of bounds

The HUD's `status` hook (plan step 7) and the rules modules' hooks.

## Size and labels

- Size: S-M
- Area: area:engine, area:party
- Needs install: no
