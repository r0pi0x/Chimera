# Run the ladder's per-tick update as systems

**Done 2026-10-30** (docs/plans/active/ecs-cleanup.md, step 4's first
commit: `mode::mode_is`, `ladder::refill` and `follow_lead` as systems).

## Context

A mode's per-tick work is a `fn(&mut World)` hook (`mode::Hooks::update`
and `touches`, dispatched by `mode::flow::update`/`touches`). The gun
game's ladder uses it for `refill` (ammo) and `follow_lead` (lead
events). Step 4 of [docs/plans/active/ecs-cleanup.md](../plans/active/ecs-cleanup.md)
turns these into systems with declared queries gated by a `mode_is(id)`
run condition; this brief is its first commit, the ladder only (Chimera's
own; Combat Arms' modes follow separately).

## Files

- `chimera/sim/src/minigame/ladder.rs` (`update`, `refill`, `follow_lead`)
- `chimera/sim/src/mode.rs`, `chimera/sim/src/mode/flow.rs`
  (`update`, `touches`, where they are scheduled)
- Guards: `chimera/sim/tests/it/minigame.rs`, `mode.rs`;
  `apps/party_roster/tests/it/gun_game.rs`; each game's `tests/it/schedules.rs`

## Definition of done

- [ ] `mode_is(id)` run condition (the playing mode, `mode::current_mode`).
- [ ] `refill` and `follow_lead` are systems with declared params, chained
      where `flow::update` runs, gated by `mode_is` for the ladder's mode;
      the ladder's `Hooks::update` is the no-op default.
- [ ] Same events in the same ticks: a test in
      `chimera/sim/tests/it/minigame.rs` on the test kit checks the lead
      events and the refill tick by tick.
- [ ] Schedule ratchets pass (no new ambiguous pairs) and the exclusive
      system count in `checks/tests/it/ecs.rs` drops if it counted them.

## Constraints

- **Order flag (plan O1, O4):** the hook wrote the world before the
  phases stepped in the same tick. Keep the systems chained before
  `step_phases`; flush with `ApplyDeferred` if they use `Commands`.
- Never a new system set (DESIGN.md "ECS style").

## Out of bounds

Removing `Hooks::update` itself, and Combat Arms' modes.

## Size and labels

- Size: S
- Area: area:engine, area:party
- Needs install: no
- good-first-task
