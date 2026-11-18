# Build-infra benchmark: before

Measured before the changes in `docs/plans/active/build-infra.md` on the
Windows PC, in a fresh slot with an empty `target\`.

- Machine: Ryzen 7 7800X3D (8 cores / 16 threads), 32 GB, NVMe.
- Toolchain: `cargo 1.99.1`, `rustc 1.99.1`, `rust-lld`
  (`.cargo/config.toml`).
- Slot: 5, branch `build-infra`, empty `target\`.
- Command: `scripts\build-bench.ps1 <scenario> -Label before -Jobs 8`.
- Load: another agent's build was running (16 `rustc` processes, CPU at
  100% for the first scenarios). Read the ratios, and the alternating
  leaf-full/leaf-affected rows in the after report, more than the
  absolute seconds.
- Edits are one appended comment line in the named file, restored when
  the scenario ends. `affected` selections use `--files`, so the edit is
  visible to `git diff` without staging.

| label | scenario | seconds | compiled units | jobs | exit |
|---|---|---|---|---|---|
| before | cold | 1,225.4 | 584 | 8 | 0 |
| before | noop | 0.8 | 0 | 8 | 0 |
| before | fast-tier | 120.0 | 0 | 8 | 101 |
| before | leaf-full | 41.6 | 20 | 8 | 0 |
| before | leaf-direct | 10.0 | 16 | 8 | 0 |
| before | bottom-full | 43.4 | 21 | 8 | 0 |

## Results

- **Cold is the cost.** 1,225.4 s and 584 units for the first test build
  in an empty slot, with another agent's build on the box. The earlier
  fast-tier log (fewer units, lighter load) took about 11 minutes
  including 2 min 9 s of tests; the compiled-unit count is the
  load-independent baseline. Target: 23.6 GB.
- **No-op is already correct.** 0.8 s, 0 units: `build.rs` does not
  relink everything on a worktree's HEAD/branch change.
- **Warm edits are already fast.** A `source_formats` edit rebuilds 20
  units in 41.6 s; a `chimera_core` edit rebuilds 21 units in 43.4 s.
  The "build takes forever" is the empty slot, not the steady-state
  edit.
- **Direct selection is the floor.** `-p source_formats -p repo_checks`
  builds 16 units in 10.0 s, but it does not build the dependents'
  tests; it is a lower bound, not a safe workflow.
- **The fast tier fails on Windows before any of this work.** 120.0 s
  to run 4,095 tests, exit 101: `chimera_core --lib`, `chimera_client
  --test it`, `combat_arms --test it` and `party_kit --test it` fail.
  The timings are still valid; a green tier depends on the
  `windows-fast-tier` fixes (commit `9075ba5f`).

These rows are the comparison baseline for
[build-infra-after.md](build-infra-after.md): the seeded-slot row is
against `cold`, the affected rows against `leaf-full`, and the
test-file rows against `fast-tier`.

