# Heavy tier: one more slot, then a converted-map cache

## Context

The `heavy::` tests are ~80% of the full suite's test time. Measured on the
Linux dev box (docs/performance.md, "The heavy tier", 2026-10-14) with
nextest at 5 heavy slots: **896-1,009 s wall**; the heavy tests' own times
sum to **3,035-3,457 s**, so the wall time is roughly that sum divided by
how many slots can run at once. Map conversion is about **a third** of it
(1,157 s of 3,468 s): 64 de_dust2 tests each convert the same map (161 s),
`map_community` 203 of 235 s, `map_stock` 62 of 68 s.

Two levers are open, in this order:

1. **Slots** - `.config/nextest.toml` caps the `heavy` group at
   `max-threads = 5`. Memory, not CPU, was the original limit (median
   peak 1.1 GB per heavy test, a quarter over 1.7 GB, the soak 6.6 GB).
2. **Converted-map cache** - the open step 2 of P4b
   (docs/plans/active/architecture-review.md); the asset service plan
   (docs/plans/active/asset-service.md, option B) sketches it. No cache
   exists yet.

This brief is for a machine with **Counter-Strike: Source installed** (and
Combat Arms for its heavy tests). On a machine without them every heavy
test skips and any timing is meaningless: here 579 tests "passed" in
14.7 s with
`skipping: no CS:S install configured (or asset store serving it)`.

## Why this should help, where, and how much

These are **estimates from the 2026-10-14 numbers**, not new measurements.
The first step of the task is to replace them with real ones.

| Lever | Where | Expected effect | Basis |
|---|---|---|---|
| 6 heavy slots instead of 5 | `.config/nextest.toml`, `[test-groups] heavy` | 5-15% off heavy wall (ceiling ~17%: 3,035 s / 5 vs / 6) | Wall is memory-bound and roughly sum/slots; the soak still holds 3 slots, and CPU/IO cap the gain below the arithmetic |
| Converted-map cache, warm | map loader / `chimera_assets` store | 15-25% off heavy wall | Conversion is ~1/3 of heavy time; cache hits avoid most of it, cold runs are unchanged |
| Both | - | ~20-35% off 896-1,009 s, i.e. ~650-800 s | The two compose; the cache also shortens the single longest map tests, not just the total sum |

The slot change is cheap and reversible: one line, one benchmark. The cache
is the larger, longer job and needs its own design (storage, invalidation
by converter version and source hashes, cold-start behaviour) before code.

## Files

- `.config/nextest.toml`: `[test-groups] heavy` and the per-test
  `threads-required` overrides.
- `scripts/testbench.ps1 -Scenario heavy`: wall, CPU and peak memory of the
  whole process tree, plus nextest's JUnit slowest-tests list.
- Map load and conversion: `games/cs_source/src/bsp/` (the `LoadTimer`
  numbers above came from `load_level_bytes`) and the store in
  `chimera/assets/`; `docs/plans/active/asset-service.md`.
- Heavy tests: `games/cs_source/tests/it/heavy/` and
  `games/combat_arms/tests/it/heavy/`.
- Results belong in docs/performance.md, "The heavy tier".

## Definition of done

- [ ] Baseline on an install machine, warm build, no other agent building:
      `.\scripts\testbench.ps1 -Scenario heavy -Repetitions 1`, three runs;
      report median wall, CPU, peak committed memory, passed/failed count,
      slowest tests and the load.
- [ ] Repeat with `heavy = { max-threads = 6 }` (keep total
      `test-threads = 8`). Keep it only if the median improves beyond the
      run-to-run spread (target > 10%) and at least 4 GB stays available
      (the criterion of the 2026-10-14 run); otherwise revert and record
      the numbers.
- [ ] Write the cache design before implementing it: what is cached,
      where it lives (per-user store, never the repo), the invalidation
      key, and which tests use it.
- [ ] Implement the cache separately and measure it cold and warm: heavy
      wall and per-map load time before/after.
- [ ] Tests still skip cleanly without an install; `cargo test -p
      repo_checks` passes.

## Constraints

- Never commit game assets, extracted files or keys; never read
  `chimera.local.toml` (AGENTS.md rules 1-2).
- Do not "speed up" heavy tests by skipping checks, lowering budgets or
  removing maps; the suite is the coverage.
- Measure on a quiet box: heavy wall time swings 2-4x with load. Record
  load and CPU time, not only wall.
- The cache is per user and outside the repository; two checkouts share it.
- Identity, UTC dates, LF text, no machine paths (AGENTS.md rules 3-4).

## Out of bounds

- Changing what the heavy tests assert, or moving them out of `heavy::`.
- Splitting the network soak (it is one run of map changes, not
  splittable per the measurement).
- The fast tier and its build settings (build-infra.md).

## Size and labels

- Size: S for the slot change; L for the cache (split design and
  implementation)
- Area: area:tools, area:cs_source
- Needs install: cs_source (combat_arms for its heavy tests)
