# Build-infra: reuse a warm slot vs seed a new one

Measured 2026-10-30 on the same Windows PC as the other reports
(Ryzen 7 7800X3D, 16 threads, 32 GB, NVMe, `rust-lld`). The box was
shared with other agents; the load is given, and the **unit counts** are
the load-independent numbers.

Both experiments use the same one-line edit: append a comment to
`source/formats/src/lib.rs`, then `cargo test --features dev --no-run`.
That is a real edit and a real link of the dependents' test binaries,
not a no-op build.

## 1. Reused fixed-path slot vs a fresh seeded slot

| run | context | seed | build | total | units |
|---|---|---|---|---|---|
| 1 | reused slot 5 (`target\` intact) | - | 90.3 s | **90.3 s** | 20 |
| 2 | reused slot 5 | - | 69.8 s | **69.8 s** | 20 |
| 3 | reused slot 5 | - | 71.1 s | **71.1 s** | 20 |
| 4 | reused slot 5 (after rebase) | - | 84.2 s | **84.2 s** | 20 |
| 5 | reused slot 5 (after rebase) | - | 62.8 s | **62.8 s** | 20 |
| 6 | reused slot 5 (after rebase) | - | 71.3 s | **71.3 s** | 20 |
| 7 | new slot 7, seeded from slot 6 | 47.9 s | 512.3 s | **560.2 s** | 25 |
| 8 | new slot 8, seeded from slot 6 | 44.3 s | 526.6 s | **570.9 s** | 25 |

The same edit is **8x faster in the reused slot** (71 s median against
560-571 s including the seed). The new slot compiles 25 workspace units
instead of 20 because every workspace crate's absolute path changed;
the copy reuses only the registry crates. That 25-unit rebuild plus 44
test links is the ~9-minute startup cost.

Later runs at load 100% (another agent building) were 118-135 s for the
full build and 32-41 s for the check below; the ratio holds, the absolute
seconds do not.

**Conclusion: keep one fixed-path slot per concurrent agent and reuse it
between tasks.** Seeding is a recovery path for an empty or trimmed slot,
not the default per task.

## 2. `cargo check --tests` + tests only for the edited crate

The same edit, in the reused slot. Instead of building all affected
test binaries:

```powershell
$selected = cargo run -q -p repo_checks --bin affected -- --files source/formats/src/lib.rs --format cargo
cargo check --tests $selected --features dev      # type-check the dependents' tests
cargo test -p source_formats                      # build and run only the edited crate
```

| run | `leaf-full` | `leaf-check` | units (full / check) |
|---|---|---|---|
| 1 | 84.2 s | 37.6 s | 20 / 20 checked |
| 2 | 62.8 s | 30.7 s | 20 / 20 checked |
| 3 | 71.3 s | 27.6 s | 20 / 20 checked |
| 4 | 135.4 s | 31.6 s | 21 / 20 checked |
| 5 | 118.1 s | 32.4 s | 20 / 20 checked |
| 6 | 127.1 s | 41.1 s | 21 / 20 checked |

The check path is **about 2.3x faster at low load (71 -> 31 s)** and
**3-4x at high load (118-135 -> 32-41 s)**. It checks 20 units and
compiles none of the downstream test binaries; the full path compiles
20 and links the affected test binaries.

It is an inner-loop tool, not a replacement for the pre-commit build:
link errors and the dependents' tests are still caught by
`cargo test --no-run` / the fast tier. The first check after a branch
change also pays to create the check artifacts; the numbers above are
warm.

## What to make the default

1. **Reuse the slot.** `slots.ps1` already picks a free slot and keeps
   its `target\`; it now also remembers the branch each slot last held
   (`slotN.last`) and prefers that slot, as `tools/agent-slot.sh` does.
   Do not raise `-Count` for every task: wait for a free warm slot, or
   release one.
2. **Seed only when there is no warm target.** A seeded slot still costs
   ~9-10 minutes; it is for a genuinely new/trimmed slot, not the
   normal handoff.
3. **Use `leaf-check` for the inner loop** on foundational crates:
   `cargo check --tests` for the affected packages, then the edited
   crate's tests. Run the full affected build before a commit/push.
4. For test-file edits, use the affected selection from the main report
   (13.5-15.4 s against 169 s).
