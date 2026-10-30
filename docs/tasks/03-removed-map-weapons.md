# Remove map-killed weapons without polling the world

**Done 2026-10-30** (docs/plans/active/ecs-cleanup.md, step 5: what
landed; a normal system comparing the list in place, the test
`chimera_sim` `equipment::weapons_the_logic_removed_leave_in_that_tick_
loose_or_carried_and_only_once`).

## Context

When map logic kills a placed weapon (`map::entities::RemovedWeapons`),
`weapon::equip::remove_killed_weapons` takes it out of the world: lying
loose, it goes; carried, its carrier loses it. Today it is an exclusive
system that clones and compares the whole list every tick. Step 5 of
[docs/plans/active/ecs-cleanup.md](../plans/active/ecs-cleanup.md)
(death and spawn pollers) asks for a reaction instead.

## Files

- `chimera/sim/src/weapon/equip.rs` (`remove_killed_weapons`),
  `chimera/sim/src/weapon/mod.rs` (where it is scheduled)
- `chimera/world/src/map/entities.rs` (`RemovedWeapons`, who fills it),
  `source/logic/src/logic/bridge.rs`
- Guards: `games/cs_source/tests/it/heavy/map_flows.rs` (needs CS:S;
  skips without it), `chimera/sim/tests/it/weapon_frame.rs`

## Definition of done

- [x] Removal reacts to a message or an observer written where a weapon
      is removed, or runs as a normal system gated by
      `resource_changed::<RemovedWeapons>`; no per-tick clone of the list.
- [x] Round restarts that re-place weapons still don't remove them again.
- [x] A new test in `chimera/sim/tests/it/` on the greybox: a loose map
      weapon and a carried one, both removed by id; a third untouched.
- [x] `checks/tests/it/ecs.rs`' exclusive-system ratchet for chimera_sim is
      lowered by one.

## Constraints

Same tick as today: the weapons vanish in the tick the logic removed them.

## Out of bounds

The other pollers of step 5 (`give_starting_weapons`, Combat Arms').

## Size and labels

- Size: S
- Area: area:engine
- Needs install: no
- good-first-task
