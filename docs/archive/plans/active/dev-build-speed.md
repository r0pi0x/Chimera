# Dev/test build artifact sharing

Scope: remove the debug-info mismatch between the default dev and test
profiles. The separate `build-infra` branch covers warm slots and affected
test selection; this change covers profile sharing.

## Plan

- Measure `cargo test -p source_formats --no-run` followed by
  `cargo build -p source_formats`, recording Cargo's JSON artifact freshness.
- Use line tables for dev as well as test, preserving opt-levels and
  incremental compilation. Add `devfull` for debugging local variables.
- Repeat the test/build transition and check that dependencies are fresh.
- Run the format crate's tests and repository checks; document measured
  results and the full-debug command.

Counts of rebuilt artifacts are the primary evidence: wall time on a
machine shared with other builds is noisy. This focused experiment does
not establish a whole-game rebuild speedup, and different feature sets
can still require different artifacts.

## Evidence

- Baseline: test build 18.61 s, then dev build 16.78 s / 21 rebuilt
  artifacts. Only four dev artifacts matched fresh test artifacts.
- With dev line tables: dev build 4.35 s / one rebuilt artifact
  (`source_formats`); all 24 dependency artifacts were fresh and matched
  paths emitted by the original test build. Sources and features unchanged.
- Added the `devfull` profile and documented it in CLAUDE.md and
  docs/performance.md. Optimization levels remain unchanged.
- Verification complete: `cargo test -p source_formats -p repo_checks`
  passed all 71 tests, including architecture and generated-doc checks.
- `cargo build -p source_formats --profile devfull` passed (19.25 s).
  JSON records confirm debug level 2 for runtime crates, level 0 for
  `rustversion`, and artifacts under `target/devfull`.
- Returning to the default build after tests and devfull rebuilt zero
  artifacts (0.63 s). All planned steps are complete.
