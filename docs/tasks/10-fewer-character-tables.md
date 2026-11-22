# Fewer archetype tables for characters

## Context

Bevy never frees an archetype or table. Party characters carry ~130
components, dozens of which come and go (spawn protection, the gun's
game's movement and view parts, the body model a map change removes), so
each death and map change visits new tables, and every query matching
characters steps over the empty ones (+25-70k instructions a call with
2,000 more). Backlog "Fewer character tables" (docs/backlog.md); the
measurement exists: `apps/party_roster/tests/party/archetypes.rs`.

## Files

- the components that come and go on characters (find them with
  `chimera_census diff`, docs/performance.md "Map changes that slowed
  the tick"): spawn protection (`chimera/sim/src/minigame/`), the
  movement and view parts swapped per gun's game, the body model
  `map::unload_map` takes off
- Guards: `apps/party_roster/tests/party/archetypes.rs`, `gun_game.rs`,
  `net_gun_game.rs`; each game's `schedules.rs`

## Definition of done

- [ ] The churning components become `#[component(storage = "SparseSet")]`,
      or fewer of them come and go; each change measured.
- [ ] New tables per round trip (death, swap, map change) reported before
      and after in the PR, from `chimera_census diff` or the archetypes test.
- [ ] A ratchet in `archetypes.rs`: the tables a round trip adds can't rise.
- [ ] Fast tier of the touched crates passes; docs/performance.md gets a
      short paragraph with the numbers.

## Constraints

Sparse-set storage makes iteration over those components slower: don't
convert components read every tick by hot queries without a measurement
(`tools/perf-profile.sh`, the profile skill).

## Out of bounds

Bevy itself (no patches to the pinned crate).

## Size and labels

- Size: M
- Area: area:engine, area:party
- Needs install: no (the archetypes test runs on the greybox)
