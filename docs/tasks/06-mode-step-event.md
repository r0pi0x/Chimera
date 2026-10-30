# Round and match lifecycle as a triggered event

**Done 2026-10-30** (docs/plans/active/ecs-cleanup.md, step 9: what
landed; a new `FlowEvent::MatchEnd` for the match's end, the
breakables observe `core::RoundRestart`, and Combat Arms'
`music::late_join` followed in the same change).

## Context

A mode's flow events (`mode::FlowEvent`: round start, end, match end...)
reach only the game's own `Hooks::event` handler. Others poll for them in
later systems: `minigame::results::record` (match end) and
`map::breakables::round_restart`. Step 9 of
[docs/plans/active/ecs-cleanup.md](../plans/active/ecs-cleanup.md).

## Files

- `chimera/sim/src/mode.rs` (`FlowEvent`, `Hooks::event`),
  `chimera/sim/src/mode/flow.rs`
- `chimera/sim/src/minigame/results.rs` (`record`),
  `chimera/world/src/map/breakables.rs` (`round_restart`)
- Guards: `chimera/sim/tests/it/mode.rs`;
  `apps/party_kit/tests/it/rotation.rs`, `results_hud.rs`,
  `party_music.rs`; `games/cs_source/tests/it/net_breakables.rs`

## Definition of done

- [x] After the game's handler runs, the flow triggers a `ModeStep` event
      carrying the `FlowEvent`; `Hooks::event` stays.
- [x] `results::record` and `breakables::round_restart` become observers
      of it (Combat Arms' `music::late_join` may follow in its own PR).
- [x] A test in `chimera/sim/tests/it/mode.rs`: an observer sees each
      flow event of a short match once, in order.
- [x] Guards pass.

## Constraints

**Flag O2:** an observer runs inside the step, where the poller ran in a
later system. Check each reads nothing the rest of the step still writes;
if it does, keep a message read by the same system instead.

## Out of bounds

The mode's own hooks and the flow's driver (`step_phases`).

## Size and labels

- Size: S-M
- Area: area:engine
- Needs install: no
