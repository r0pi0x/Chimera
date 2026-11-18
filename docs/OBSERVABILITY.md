# Observability & Testing Cheat Sheet

> Full reference & historical examples: [docs/archive/observability-full.md](archive/observability-full.md).

## 1. Scenario Tests (`harness::Sim`)

Run headless simulations in `<crate>/tests/it/`:
- Engine crates: `Sim::new(SimBasePlugins, GreyboxMapPlugin)`
- CS:S: `CsSim` (greybox or `heavy::` with real map assets)
- Use `sim.tick(n)` / `sim.repeatable()` / `sim.work(n)` for deterministic counts.

## 2. Screenshots & Visual Checks

Tool runs never overwrite `config.cfg`:
```bash
cargo run --features dev -- --screenshot /tmp/a.png --frames 90 --spawn 0,1,12 --look 30,-10
cargo run --features dev -- --views /tmp/views.toml --frames 90
```
- Compare against reference screenshots with `refcmp` / `refcmp report` / `refcmp hudcmp`.

## 3. Remote Protocol & Console (`--features dev`)

Dev builds serve Bevy's Remote Protocol (JSON-RPC) on a free port logged at startup (`BRP listening on 127.0.0.1:<port>`):
- Query entities/components or inject console commands over JSON-RPC.
- For multi-agent worktrees on Linux, use `tools/agent-slot.sh acquire <branch>` and `source tools/agent-env.sh && agent_run cargo ...`.

## 4. Performance (`WorkCounts` & `perfgate`)

Always judge performance by deterministic instruction/work counts (`sim.repeatable()`, `sim.work(n)`, `perfgate`, `frame_bench`), never wall-clock time. See [docs/archive/performance-log.md](archive/performance-log.md) for baselines.
