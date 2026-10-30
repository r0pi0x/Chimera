# Rules modules' join and arming as entity events

**Done 2026-10-30** (docs/plans/active/ecs-cleanup.md, step 8: what
landed; observers of one event have no order, so "after the ladder's"
holds for the ladder's join rung, not its arming: the reason there).

## Context

A minigame's rules module (`chimera_sim::minigame::RulesModule`) has
`joined: fn(&mut World, Entity)` and `arming: fn(&mut World, Entity)`,
called by `runtime::join` and `runtime::respawn`. Only the module can
react; a game's look or a mode's kit has to wrap the hook. Step 8 of
[docs/plans/active/ecs-cleanup.md](../plans/active/ecs-cleanup.md).

## Files

- `chimera/sim/src/minigame.rs` (`RulesModule`),
  `chimera/sim/src/minigame/runtime.rs` (the two call sites),
  `chimera/sim/src/minigame/ladder.rs` (`arm`, the join rung)
- Guards: `chimera/sim/tests/it/rung_extras.rs`, `minigame.rs`;
  `apps/party_kit/tests/it/rung_extras.rs`, `rung_characters.rs`,
  `gun_game.rs`

## Definition of done

- [x] `Joined` and `Arming` entity events, triggered with `world.trigger`
      where the hooks are called now; the ladder observes them; the two
      fields leave `RulesModule`.
- [x] A test: a second observer of `Arming` runs after the ladder's and
      before the weapons are given.
- [x] Guards pass; `docs_index` run if a `//!` doc changed.

## Constraints

**Flags O2, O3:** arming must come before the weapons are given; the
ladder's observer is the only one that gives weapons.

## Out of bounds

Combat Arms' `CaMode::left` (separate commit of the same step).

## Size and labels

- Size: S
- Area: area:engine, area:party
- Needs install: no
- good-first-task
