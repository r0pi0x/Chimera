# Build-infra benchmark: after

Measured 2026-10-30 on the same machine and with the same harness as
[build-infra-before.md](build-infra-before.md), in slot 6
(`build-infra-seed`) after the first round of changes. The boxes were
shared with other agents' builds; the 1-minute load is given where it
was taken, and the **compiled-unit column is the load-independent
number** to trust.

What changed before these runs:

- `slots.ps1` discovers every `slotN` on disk (not just `1..-Count`)
  and has `seed slotN`, which copies a warm `target\` into a new slot.
- `scripts/affected-tests.ps1` asks `repo_checks affected` which
  packages a change touches, instead of building/running the whole
  workspace.
- `scripts/build-bench.ps1` is the shared harness for both reports.

| label | scenario | seconds | compiled units | jobs | exit |
|---|---|---|---|---|---|
| after | seeded | 559.9 | 25 | 8 | 0 |
| after | leaf-full | 70.0 | 20 | 8 | 0 |
| after | leaf-affected | 58.7 | 21 | 8 | 0 |
| after | leaf-full | 55.8 | 20 | 8 | 0 |
| after | leaf-affected | 50.0 | 21 | 8 | 0 |
| after | test-file-affected | 153.1 | 7 | 8 | 101 |
| after | test-file-affected | 13.5 | 2 | 8 | 101 |
| after | fast-tier | 248.9 | 19 | 8 | 101 |
| after | fast-tier | 168.9 | 0 | 8 | 101 |
| after | test-file-affected | 15.4 | 2 | 8 | 0 |
| after | bottom-affected | 60.8 | 19 | 8 | 0 |
| after | noop | 5.9 | 0 | 8 | 0 |

`seeded` is the same `cargo test --features dev --no-run` as `cold`, but
slot 6's `target\` was first copied from slot 5 (30.1 GB in 45.1 s).

## New session: cold vs seeded

| | cold (before) | seeded (after) | change |
|---|---|---|---|
| compile/link | 1,225.4 s | 559.9 s | -54.3% |
| seed copy | - | 45.1 s | |
| total to a usable target | 1,225.4 s | 605.0 s | **-50.6%** |
| units compiled | 584 | 25 | **-95.7%** |

The unit count is the reliable half: seeding reused every registry
crate and compiled only 25 workspace units (their absolute paths
differ, which is exactly why slots exist). The wall times were taken
under different other-agent load, so read the unit count as the cause
and the ~2x as the measured effect on this box, not a controlled
quiet-box ratio.

## Warm source edit: full workspace vs affected

A one-line edit in `source/formats/src/lib.rs`, alternating on the same
warm target:

| round | full workspace | affected | units (full/affected) |
|---|---|---|---|
| 1 | 70.0 s | 58.7 s | 20 / 21 |
| 2 | 55.8 s | 50.0 s | 20 / 21 |
| median | 62.9 s | 54.4 s | |

Affected selection saves **about 13%**, not the predicted 50-60%.
`source_formats` is foundational: every dependent's test binary really
does link it, so `affected` selects them all. `leaf-direct` (before:
10.0 s) is the unsafe lower bound: it builds only `source_formats` and
`repo_checks`, not the dependents' tests.

## Test-file edit: full fast tier vs affected

A one-line edit in `chimera/client/tests/it/metrics.rs`:

| | seconds | what it did |
|---|---|---|
| full fast tier (before, quiet) | 120.0 | ran all 4,095 fast tests |
| full fast tier (after, load 77%) | 168.9 | ran all 4,095 fast tests, 0 units |
| affected, first run (after) | 153.1 | built the test binary, then 162 tests |
| affected, warm run (after) | 13.5-15.4 | 2 units, then 162 tests + repo_checks |

The steady-state edit loop is **11x faster** (168.9 s -> 15.4 s); the
first edit after a build pays the test binary's link (153.1 s, still
38% under the full run). This is the biggest everyday win measured.

## Bottom edit: no affected benefit

A one-line edit in `chimera/core/src/lib.rs`:

| | seconds | units |
|---|---|---|
| full workspace (before) | 43.4 | 21 |
| affected (after) | 60.8 | 19 |

No gain: a bottom-crate edit selects nearly every dependent, so
`affected` has nothing to narrow. The difference here is load noise,
not a regression. This is the 38% of commits the plan expected to need
the line-tables/structural work instead.

## No-op

0 units both times (0.8 s before, 5.9 s after under load): no relink
regression from the scripts or the slot changes.

## Failures are pre-existing

The fast tier exits 101 on `main` on Windows: `chimera_core --lib` and
`party_kit`/`combat_arms` fail, and `chimera_client`'s metrics test is
flaky (it failed in two runs and passed in one). These are the Windows
clock/`GetThreadTimes` issues the `windows-fast-tier` branch
(`9075ba5f`) started fixing; the benchmark times include them. A green
fast tier is a prerequisite for trusting the exit column, not for the
timings.

## What this changes in the plan

- Seeding and staying in one warm slot are the biggest first-build win
  (584 -> 25 units, ~2x wall here).
- `affected` is a **test-run** win (11x on a test-file edit), not a
  build-time win for foundational or bottom crates.
- Warm source/bottom edits are already ~40-60 s; the 20-minute cold
  slot is the actual pain.
- Not measured this round: sccache (not installed), Defender
  exclusions (no administrator), nextest (not installed), and a
  line-tables dev profile (the test profile already uses line tables).
  These stay as candidates in the plan rather than claimed gains.
