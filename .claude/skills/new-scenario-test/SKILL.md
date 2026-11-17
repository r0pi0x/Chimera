---
name: new-scenario-test
description: Write a headless scenario test for new or changed behaviour (where it goes, the harness Sim, SimBasePlugins, CsSim, the test kit, NetSim).
---

# Write a scenario test

Full guide: docs/OBSERVABILITY.md section 1. In short:

**Where.** The lowest crate whose public API the test needs, in that
crate's one integration binary: `<crate>/tests/it/<topic>.rs`, listed as
`mod <topic>;` in `<crate>/tests/it/main.rs`. Slow tests (real maps, long
bot rounds) go under `tests/it/heavy/`. A test naming nothing of a game
goes in Chimera (`chimera/sim/tests/it/`, `chimera/net/tests/it/`,
`chimera/client/tests/it/`, `chimera/present/tests/it/`); one naming a
game's weapon, map or rule stays in that game (`games/cs_source/tests/it/`).
Shared helpers: the game's `tests/common/`, never another copy of `fn sim(`.

**How.** `harness::Sim` (chimera_world) runs exact fixed ticks, no window:
```rust
let mut sim = Sim::new(SimBasePlugins, GreyboxMapPlugin);   // Chimera's crates
// games/cs_source: Sim::new(cs_source::harness::CsSim, ...)
let p = sim.spawn_character(Vec3::new(0.0, 1.0, 12.0), placeholder::ID);
sim.intent(p).move_axis = Vec2::Y;
sim.seconds(1.0);
assert!(sim.velocity(p).xz().length() > 4.9);
```
- Weapons without a game: `chimera_sim::testkit` (`kit_sim()`,
  `with_player`, `active`, `aim_at`; weapons `kit:pistol`, `kit:rifle`,
  `kit:shotgun`).
- Network: `NetSim::new(base, conditions, seed, clients, setup)`
  (chimera_net `harness`), maps built in code (`cs_source::harness::TestMaps`).
- Time and waiting, never `thread::sleep` (a repo check refuses it):
  `sim.advance(Duration)` / `sim.seconds(s)` jump game and real time at
  once; `sim.run_until(|w| .., max_ticks)` / `sim.expect_until("what",
  |w| .., max_ticks)` / `sim.await_message::<M>(max_ticks)` wait for a
  condition with a tick limit; `sim.run_until_idle(max_ticks)` waits for
  background work (map loads, model builds, downloads, nav meshes). UI
  animations snap in a `Sim` (`ui_motion_scale` 0). Loops on real sockets:
  `harness::let_io_run()` / `pump_until`. docs/OBSERVABILITY.md section 1,
  "Time and waiting".
- A spec's test cases each become one test; name the spec section in the
  test's doc comment.
- A cost (how much a tick does): `sim.repeatable()`, then
  `let w = sim.work(n)` (None without hardware counters: skip with a
  note) and assert on `w.total()` / `w.steady_ticks()`, printing
  `w.systems.diff_table(&before.systems, 20)` on failure. Never time it
  with `Instant`: wall time moves with the machine's load
  (docs/performance.md, "Work counts").

**Run:** `cargo test -p <crate> --features dev --test it <topic>::`.
Write the test first and watch it fail, then implement.
