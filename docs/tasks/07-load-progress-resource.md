# Map-load progress as a resource, not a static

**Done 2026-10-30** (ecs-cleanup.md, step 10).

## Context

`chimera_world::map::loading` keeps load progress in a process-wide
`static PROGRESS` (`report`, `reset`, `current`). A listen server and its
client in one process share it, as would two apps in one test. Step 10 of
[docs/plans/active/ecs-cleanup.md](../plans/active/ecs-cleanup.md).

## Files

- `chimera/world/src/map/loading.rs` (`PROGRESS`, `report`, `reset`, `current`)
- callers: `games/cs_source/src/game/bsp.rs` (reports),
  `games/cs_source/ui/src/game_menu/loading.rs` (reads); grep for
  `loading::` for others (Combat Arms' loaders)
- Guards: `checks/tests/it/determinism.rs` (counts statics),
  `apps/party_kit/tests/it/listen_server.rs`

## Definition of done

- [x] A resource the loading task writes through a handle it owns (an
      `Arc<Mutex<..>>` or channel created with the load), read by the
      loading dialog; the static is gone.
- [x] A unit test in `loading.rs` (chimera_world has no integration tests
      yet): two `App`s
      in one process load at once and each sees only its own progress.
- [x] The determinism ratchet for `chimera/world` lowered (1 to 0).

## Constraints

Loaders run on a background task: no `World` access from them.

## Out of bounds

The install memos (plan "Later" 13).

## Size and labels

- Size: S
- Area: area:engine
- Needs install: no
- good-first-task
