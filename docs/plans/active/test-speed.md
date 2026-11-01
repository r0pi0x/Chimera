# Test cycle speed

Goal: cut the time from "change made" to "verified" for the loops an agent
actually runs, without giving up the check. Targets:

| Loop | Now | Target |
|---|---|---|
| One crate's tests (leaf edit) | 2-15 s build + run (docs) | keep |
| Fast tier, Windows warm | **~130 s wall** (119.6 s tests, 46 binaries) | **≤ 75 s** |
| Commit gate (affected build + direct fast tier) | **~226 s** (docs, 2026-10-27) | **≤ 60 s** |
| Full suite incl. heavy | **~1,757 s** (docs) | **≤ 1,200 s** (slots + profiles), less with a map cache |
| Cold slot build | 30-60 min | not this plan (build session) |

The fast tier is not build-bound on Windows: `fast-tier2.log` has no
compiles, 119.6 s of test time and ~10 s of cargo overhead. The time is
**tests running one binary after another**, and one binary is half of it:
`combat_arms it` 61.9 s (1,509 tests), `party_kit it` 19.3 s, `cs_source it`
12.8 s. The full suite is different: ~80% is heavy tests, a third of that
is converting maps.

## Where the time goes

- **Fast tier**: 46 test binaries run sequentially by `cargo test`. The
  biggest is 62 s; the other 45 add ~58 s. `cargo nextest` is not
  installed on Windows.
- **Commit gate**: the hook builds the test binaries of *every affected
  crate* (20 crates in the measured example, 226 s) before running only
  the changed crates' fast tier. The dependents' build is the cost.
- **Heavy tier**: ~5,500 s of tests over 5 slots (`nextest.toml`), memory
  being the stated limit (median 1.1 GB, some 3-6 GB). Map conversion is
  1,157 s of 3,468 s measured heavy time; `map_de_dust2` alone loads the
  same map in 67 separate test processes. The soak and capacity tests hold
  slots for 3-4 minutes by themselves.
- **Cold build**: dependencies dominate; already being worked on in the
  build slots/sccache/mold session. This plan does not duplicate it.

## Benchmarking

Every step below is judged by the same scenarios and the same script, so a
win is a number, not a feeling. This is as much part of the plan as the
optimizations.

### Harness

- [ ] `scripts/testbench.ps1` (Windows; `tools/testbench.sh` later for the
      Linux box) runs a named scenario and records:
      - wall time, user + system CPU time, peak RSS;
      - build time separately from run time;
      - test count and per-test times (from nextest's JUnit output; `cargo
        test` gives totals only, its per-test report needs nightly);
      - the top tests by time and the machine's load/RAM.
- [ ] `-Repetitions 3` (median and min; first run a warm-up), `-Load`,
      `-Update` (write the baseline) and the default compare mode.
- [ ] Write `target/testbench/<date>-<scenario>.json` (git-ignored) and a
      markdown row. Never run it in the fast tier; it is a tool, like
      `perfgate`.
- [ ] Add a `bench` nextest profile with JUnit output
      (`target/testbench/junit.xml`) and a `heavy` profile variant for slot
      experiments.

### Scenarios

| Name | What it runs | Question |
|---|---|---|
| `leaf` | touch a leaf file, `cargo test -p <crate> --features dev --no-run`, then one test | edit loop |
| `crate` | `cargo test -p <crate> --features dev` | one crate's tests |
| `fast-cargo` | `cargo test --features dev -- --skip heavy::` | fast tier, runner A |
| `fast-nextest` | `cargo nextest run --features dev -E '!test(/^heavy::/)'` | fast tier, runner B |
| `commit` | the pre-commit hook on a real staged change | commit gate |
| `affected` | `affected --base main` selection plus build/run | a typical change |
| `full-cargo` | `cargo test --features dev` | full suite, runner A |
| `full-nextest` | `cargo nextest run --features dev` | full suite, runner B |
| `heavy` | `-E 'test(/^heavy::/)'`, at 5/6/8 slots | slot tuning |
| `map-load` | an ignored microbench: lean, full, serialized cache | map cache |
| `sim-setup` | an ignored microbench: build N `Sim`s | per-test overhead |

### Protocol

- Same commit, same target dir, same features; warm build unless measuring
  cold. Record the machine's load; never trust a run taken beside another
  agent's build.
- Three runs, median and minimum; interleave A/B when comparing (the box is
  shared and wall time swings 2-4x).
- Record the test count with every number, so a "faster" run that skipped
  tests is visible.
- Put the result in `docs/performance.md` ("Test cycle") with the date,
  machine and load. Raw logs stay in `target/testbench/`.
- A change claims a win only if the median improves outside the run-to-run
  spread; CPU time and instructions are steadier than wall time.

### Baselines and gate

- `tools/testbench/baseline.json`: per machine and scenario, the last
  accepted median wall, CPU, peak RSS and test count.
- Default compare prints deltas and warns above 10%; it fails on a 2x
  regression or a change in test count. A hard gate on wall time would be
  noise on this box, so the failure threshold is deliberately loose.
- Update the baseline only in the commit that explains the change; the
  `docs/performance.md` row and the JSON move together.
- Re-run the heavy slot experiment (5/6/8) after any change to load
  options, the map cache or `threads-required`; the memory profile is what
  decides the slot count, not the time alone.

### First numbers to fill

| Scenario | Now | Target | Notes |
|---|---|---|---|
| fast-cargo (Windows, warm) | 106.8 s wall / 985.8 s CPU / 1,552 MB | ≤ 75 s | 3,956 passed, 1 failed (CA `t36_rate_cap`) |
| fast-nextest (Windows, warm, `fast` = 8) | 117.4 s wall / 950.1 s CPU / 865 MB | ≤ 75 s | 3,956 passed, 1 failed; slower than cargo, lower peak memory |
| commit | 226 s (docs) | ≤ 60 s | check measured 2.7x on `source_tools`; multi-crate pending |
| full-nextest | 1,757 s (docs) | ≤ 1,200 s | heavy ~80% |
| heavy at 5 slots | ~1,100 s floor | measure 6/8 | 32 GB, 16 threads; slots out of scope for now |
| map-load de_dust2 | 161 s / 67 loads (docs) | 1 load or cache | one-load cache compiles; runtime test skipped (no install here) |

## Steps, in order of time saved per effort

### 1. Measure the loop (half a session, before changing anything)

- [ ] Build `scripts/testbench.ps1` from the section above and run the
      `leaf`, `crate`, `fast-cargo`, `fast-nextest`, `commit` and `full`
      scenarios on the current commit. Record them as the baseline.
- [ ] Install `cargo-nextest` on Windows (needed for per-test times and the
      full-suite comparison). Use its slow-test report rather than
      guessing.
- [ ] Run the `heavy` scenario at 5 slots and record peak RSS; this is the
      datum for step 5.
- [ ] Put the table in `docs/performance.md` ("Test cycle") with date,
      machine and load. No optimization without this baseline.

Deliverable: a table of where the seconds are, per loop, and a baseline
file the later steps compare against.

### 2. Parallelise binaries (fast tier)

`cargo test` runs the 46 binaries one after another. `nextest` runs test
processes across binaries at once.

- [ ] Compare, warm, same commit: `cargo test --features dev -- --skip
      heavy::` vs `cargo nextest run --features dev -E '!test(/^heavy::/)'`
      (about 1,500 process spawns on Windows; measure, do not assume it is
      faster for tiny tests).
- [ ] Tune `test-threads` for the fast tier (8 today; the box has 16
      logical processors and the tests are light). Keep heavy at 5 until
      step 5 measures memory.
- [ ] Keep `cargo test` as the fallback when nextest is absent.

Expected: fast tier 130 s → 75-90 s. If nextest is slower for the fast
tier, keep `cargo test` there and use nextest for heavy only.

### 3. Stop building every dependent at commit (biggest recurring win)

The hook's 226 s is mostly `cargo test --no-run` for 20 crates whose tests
it does not run. The dependents run after the push anyway
(`tools/post-push-tests.sh`).

- [x] Change the commit gate to:
      - build and run the **changed crates'** tests (as now);
      - for dependents, `cargo check --all-targets` (catches compile breaks
        without codegen or linking), or defer them to post-push if check
        is still slow;
      - always `repo_checks`.
      Done 2026-10-30; `CHIMERA_COMMIT_DEPENDENTS=build` restores the old
      full build. The check catches a dependent that no longer compiles;
      a link-only break waits for post-push, which is the minimal
      downside.
- [ ] Measure both variants on 3-4 real changes. Keep the one that stays
      under 60 s and still catches a dependent that no longer compiles.
- [x] Keep the strict version as the opt-in (`CHIMERA_COMMIT_DEPENDENTS=build`;
      `CHIMERA_COMMIT_TESTS=affected` still runs the old fast tier).

Expected: 226 s → 30-60 s per commit. The risk is a dependent break found
after commit instead of before; a push is cheap and post-push already
tests it.

### 4. Hunt the slow fast-tier tests

Step 1 names them. Attack in this order:

- [ ] `combat_arms it` (62 s): find the top 10; look for `sim.seconds()`
      loops that run seconds of simulation for a one-value assertion,
      tests that build several worlds, and tests that start a server or
      load install data without being `heavy::`.
- [ ] `party_kit it` (19 s): the same; its app compositions are heavier.
- [ ] Move tests that are correct but slow and do not need install data
      into a `slow::` module that runs after push (like `heavy::` but
      without an install). Do not delete coverage; move it.
- [ ] Add a guard: a fast-tier test that takes over ~2 s prints a warning
      (nextest slow-timeout already gives the data).

Expected: 20-30 s off the fast tier; CA from 62 s to 35-40 s.

### 5. Heavy tier

The heavy tests sum to ~5,500 s over 5 slots, so the wall time is
~1,100 s; ~80% of the full suite. A third of that is converting maps, and
one file shows the repeat cost: `map_de_dust2` has 67 tests and each loads
de_dust2 (161 s of its 283 s, measured 2026-10-14). Options, cheapest
first:

- [ ] **Slots.** Re-measure peak RSS per heavy test on the Windows box
      (32 GB, 16 threads) and raise the heavy group from 5 to 6-8 if it
      fits. The old 1.1 GB median and 3-6 GB peaks predate the lean load
      path. Expected: several minutes off the full suite; cheapest change
      here.
- [ ] **Test-load profiles.** Make the common map helper lean by default
      (no texture decode) and use the full load only in the tests that
      read texels or draw. Add options for what a test never reads
      (lightmaps, cubemaps, nav) so a map load pays only for the data the
      test asserts. Many `map_de_dust2` tests need geometry, entities or
      collision, not textures.
- [x] **One load per file (prototype).** `map_de_dust2` converts de_dust2
      once per process (`OnceLock<Option<Arc<MapData>>>`) and shares it
      through `MapPlugin::from_arc` (a new general `MapPlugin` constructor
      for an already-converted map); `cargo test` runs the binary in one
      process, so its 67 tests load once. Nextest runs one test per process
      and is unchanged. No global cache: one map retained, not every heavy
      map. Measurement pending the warm build.
- [x] **Converted-map cache (P4b step 2).** Built 2026-10-31 per user
      (`~/.cache/chimera/maps`, not `target/`), keyed by the converter's
      sources and checked against every file the conversion read
      (asset-service.md section 10): de_dust2 7-8x faster warm,
      cs_source's heavy tests -15% wall, -21% summed (performance.md,
      "The converted-map cache"). No test bypasses it: a hit is the
      converted map (compared by hash in `heavy::map_cache`).
- [ ] **Long tests as a profile.** The soak (178-266 s, 3 slots) and the
      capacity test (3 slots) are the longest. Put them in a `soak`
      profile/nightly run and keep the default full suite to the rest;
      document that "full" means "everything but the soak" unless asked.
      Frees several minutes of slot time.
- [ ] **Split the loops.** Tests that still loop over maps
      (`map_flows`' jacks test, 54-104 s) become one test per map, as
      `map_ladders`/`map_surf` already are, so the heavy slots can overlap
      them. Long course tests stay whole (their state is shared) unless
      their checks are independent enough for threads, as `map_de_nuke`'s
      ladders do.
- [ ] **Windows specifics.** Measure nextest's process-per-test cost on
      the fast tier before adopting it there (3,900 tiny tests vs
      Linux). Exclude the repo, `target/` and the game installs from
      Defender's real-time scan on this PC; 1,500-3,900 process starts and
      constant asset reads are what it slows most. This is machine setup,
      not a repo change.


### 6. Build side (coordinate, don't repeat)

- The warm-slot/sccache/mold/line-tables work and the LF/metrics branches
  are other sessions. This plan only measures their effect and avoids
  adding test time on top.
- For an edit loop, `cargo test -p <crate>` (not the workspace) is the
  command; the build session's slot setup makes it warm.

## Guardrails

- Every optimization must keep "a broken change still fails a test". A
  moved test goes to `slow::`/`heavy::`, not away.
- Do not add the render/visual probes (the coverage plan,
  `test-architecture.md`) to the default fast tier. They run only for
  render/UI paths, and only where a GPU exists.
- One test binary per crate stays; the win is parallelism, not more
  binaries.

## Expected result

- Commit loop: ~226 s → under a minute.
- Fast tier: ~2 min → about one.
- Full suite: ~30 min → ~18-20 min from slot tuning, the soak profile and
  splitting loops; a map cache targets ~15 min by removing the repeated
  conversions.
- No loss of verification; the heavy and render checks stay available when
  the change can affect them.

## Progress

2026-10-30: plan written. Baseline from `fast-tier2.log` (Windows fast
tier: 129 s wall, 119.6 s tests, 46 binaries, CA 61.9 s) and
`docs/performance.md` ("Test cycle": commit gate 226 s, full suite
1,757 s, heavy ~80%, map conversion 1/3 of heavy). The full-suite options
were added after finding that `map_de_dust2` loads de_dust2 once per test
(67 loads) and that the soak/capacity tests hold 3 slots each. A
benchmarking harness and protocol were added before any optimization:
wall, CPU, peak RSS, per-test times, three runs, a JSON baseline and a
loose regression gate.

2026-10-30: harness written: `scripts/testbench.ps1` (job-object wall/CPU/
peak RSS, cargo/nextest scenarios, JSON baselines, compare/update) and the
`bench` nextest profile for JUnit per-test times. Validated on
`source_formats`: 14.0 s wall / 29.0 s CPU / 746 MB on a cold first run at
load 53, then 0.5 s warm (40 tests). The provisional baseline was removed:
a cold or loaded run is not a test-cycle baseline; the first real baseline
waits for a quiet box and `-Warmup`.

2026-10-30: the commit gate now checks the dependents (`cargo check
--all-targets`) instead of building their test binaries; the changed crates'
fast tier still builds and runs. `CHIMERA_COMMIT_DEPENDENTS=build` is the
old behaviour. Syntax checked with Git Bash; the wall-time measurement is
pending (load 46-60 from another slot's build).

2026-10-30: every workspace member the hooks can select now has an empty
`dev` feature (`chimera_vfs`, `source_formats`, `lithtech_formats`,
`source_tools`; `workspace-hack` excepted, generated). Without it,
`cargo test -p <pure crate> --features dev` fails and the hook could never
check those crates; `repo_checks` now guards the rule (and passes).
The dependent check uses `--all-targets`, so a bin or example that no
longer compiles is caught too.

2026-10-30: the dependent check measured on `source_tools` (tiny crate,
same load): `cargo test --no-run` 4.44 s vs `cargo check --all-targets`
1.64 s, 2.7x faster. The multi-crate commit number is still pending, but
the direction is confirmed.

2026-10-30: `map_de_dust2` now converts de_dust2 once per process and
shares it (`OnceLock<Arc<MapData>>`, `MapPlugin::from_arc`); its 67 tests
load one map under `cargo test` instead of 67. Deliberately one map, not a
global cache, so the process does not retain every heavy map. Nextest is
unchanged. Compile/test pending the warm build.

2026-10-30: `cargo-nextest 0.9.150` installed on Windows from the prebuilt
zip (the `get.nexte.st` redirect + GitHub download works with `curl`).
Added a `fast` profile (12 test threads, inherits `bench`'s JUnit output);
the testbench's `fast-nextest` scenario uses it. The fast-tier comparison
runs as soon as the warm build finishes; heavy slots stay out of scope per
the user's call.

2026-10-30: after rebasing onto upstream/main (melee/projectile), the fast
tier on Windows: `fast-cargo` 106.8 s wall / 985.8 s CPU / 1,552 MB peak,
3,956 passed and 1 failed; `fast-nextest --profile fast` (8 threads)
117.4 s wall / 950.1 s CPU / 865 MB, 3,956 passed and 1 failed. Cargo is
faster; nextest uses less peak memory. The earlier 89.3 s nextest result
was at 12 threads before upstream; after it, 12 threads peaked at 39 GB
(swap), so `fast` is back to 8. The one failure is
`combat_arms::inventory::t36_rate_cap`. The dust2 cache compiles and the
affected tests pass; the heavy dust2 file skipped all 67 tests because
this clone has no CS:S install configured, so its runtime gain is still
unmeasured.

## Decision log

2026-10-30: the first target is the commit gate, not the fast tier. It is
the largest recurring cost (226 s) and the fix is a hook policy plus a
`cargo check`, not new infrastructure.

2026-10-30: optimizations stay general and reversible. The commit gate
still type-checks every affected crate's tests, so a compile break is
caught before commit; only a link-only break can reach the push, where the
post-push run catches it. `CHIMERA_COMMIT_DEPENDENTS=build` restores the
old behaviour. No test coverage is moved or skipped, and no heavy slot or
memory limit is raised without a measurement.

2026-10-30: measure nextest on the fast tier before adopting it. It is 2x
faster for the full suite (heavy tests contend in one process), but 3,900
fast tests in separate processes may not beat `cargo test`.

2026-10-30: keep `cargo test` for the fast tier and `nextest` for the full
and heavy tiers. The 12-thread nextest win (89 s) did not survive the
upstream melee/projectile merge: at 8 threads it is 117 s vs cargo's
107 s, and 12 threads peaked at 39 GB. `fast` stays opt-in (JUnit and
lower memory), not the default; the pre-commit hook stays on cargo until a
real commit is measured on Linux.

2026-10-30: the map cache is the heavy-tier lever; slot tuning comes first
because it is a config change and a measurement, not a new cache.

2026-10-30: benchmarks record wall, CPU and peak RSS, not wall alone. Wall
time on this box swings 2-4x with other agents' builds; CPU time and
instructions are steadier. Three runs, median and minimum; the gate is
loose (warn 10%, fail 2x) so it catches a collapse without failing on
noise.

2026-10-30: per-test times come from nextest's JUnit output; `cargo test`
is measured at process level. The harness is a tool, never part of the
fast tier, so benchmarking cannot become test overhead.

## Open questions

- Is the pain the edit loop (fast tier), the commit gate, or the full
  suite? The plan starts at the commit gate because it is the largest
  measured per-commit cost; if the agent rarely commits, start at step 2.
- Does the Windows box run heavy tests at all today, or only Linux? If only
  Linux, heavy tuning belongs there and the Windows plan stops after step
  4.
