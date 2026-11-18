# Windows affected-test verification (2026-10-30)

Compared the whole fast tier with the existing selector's output for
`chimera/client/tests/it/metrics.rs`, in one warm worktree. Sources were
unchanged: this measures repeated test execution for that change scope,
not the first rebuild after an edit. Both test builds were warmed first.
Measured revision: `85436d0c` (before the Windows metrics-test fixes
and build-infra runner merged into upstream main). Cargo used eight
build jobs. Execution order: full/affected,
affected/full, full/affected. Other sessions shared the machine.

Full command: `cargo test --features dev --no-fail-fast -- --skip heavy::`.
Affected command: first `cargo run -q -p repo_checks --bin affected --
--files chimera/client/tests/it/metrics.rs --format cargo`, then
`cargo test <selected arguments> --features dev --no-fail-fast --
--skip heavy::`. The selector returned `-p repo_checks -p chimera_client`.
The timer includes selector execution and its compilation overhead.

| Round | Full seconds | Affected seconds | Selection seconds (included) | Full failures | Affected failures |
|---|---|---|---|---|---|
| 1 | 218.76 | 18.55 | 2.96 | 9 | 0 |
| 2 | 211.55 | 12.84 | 2.33 | 9 | 0 |
| 3 | 181.94 | 15.48 | 4.70 | 8 | 1 |
| Median | 211.55 | 15.48 | | | |

Full: 3,937 tests per run, zero compilation units reported. Affected:
193 tests per run, one `repo_checks` compilation reported in each test
invocation. The median elapsed time falls 92.7%, or 196.07 seconds
(13.7x). This is a narrow-scope test-run result, not a compilation or
whole-game speedup. Foundational source edits still select dependents.

## Failures and interpretation

The full suite failed each time: core's `counts_grow_with_work`, Combat
Arms' `t36_rate_cap`, and six party-kit tests. Client metrics'
`every_tick_is_measured` failed in full rounds 1/2 and affected round 3.
Affected rounds 1/2 passed all 193 tests. Both commands used
`--no-fail-fast`, so test failures did not terminate the remaining targets.
This verifies elapsed-time savings, not a clean full-suite baseline or
proof that the selector cannot miss a dependency. Broader validation
remains necessary; selected fast tests omit heavy tests.

## Entry point

`scripts/affected-tests.ps1` exposes the existing conservative package
selector on Windows. Specify `-Files` for explicit repository-relative
paths, `-Staged` for the index, or `-Base` for committed branch changes.
The scope is required so uncommitted edits aren't silently mistaken for
the branch's committed diff. `-BuildOnly` builds the selected tests.
The wrapper runs from the repository root and propagates failures.

The wrapper originated on `build-infra` and has since merged upstream;
this PR tightens scope selection and working-directory handling. No
changes to the selection algorithm are included. Cargo uses whole-package
selection; optional `-Nextest` requests the selector's module-level
filters (not benchmarked here). `-Direct` retains the upstream opt-in
for changed packages only, omitting dependent tests; it is not the
default or the mode used in this measurement.

## Why the selector's helper was rebuilt every run

Cargo's fingerprint log (`CARGO_LOG=cargo::core::compiler::fingerprint=info`)
reported `the list of features changed` for `repo_checks` in both
directions. The selector command built `affected.exe` with the package's
default features, and the following `cargo test ... --features dev`
built it with `dev` plus the client graph's unified dependency features
(observed: `serde_json` gained `raw_value` through `workspace-hack`).
Cargo writes both configurations to the same executable path in
`target/debug`, so each command invalidated the other's fingerprint.
Adding `--features dev` to the selector removed the first mismatch, but
transitive differences remained (`serde_json` followed by `serde_core`
and `memchr`), so that alone is not a fix.

Giving selection its own target directory (`target\affected-selector`)
keeps a warm cache for the small selector build and leaves the main
`target` cache to the test/build configuration. Four paired runs,
alternating order, on warmed caches (`cargo test -p repo_checks -p
chimera_client --features dev --no-run`):

| Cache | Selector (s) | Test build (s) | Total (s) | Rebuilt in test phase |
|---|---|---|---|---|
| separate | 2.01, 1.80, 1.62, 1.82 | 1.62, 1.29, 1.30, 1.31 | 3.63, 3.08, 2.92, 3.13 | 0, 0, 0, 0 |
| shared | 2.41, 2.14, 2.06, 2.10 | 1.72, 1.59, 1.63, 1.63 | 4.13, 3.73, 3.69, 3.73 | 1, 1, 1, 1 |

Medians: separate 3.11 s, shared 3.71 s, about 0.6 s and one relink
saved per invocation. The separate cache measured 72.3 MB. This is a
small absolute saving within a 13-19 s affected run, but it removes a
guaranteed rebuild and is reproducible; it does not accelerate test
execution or the game build.
