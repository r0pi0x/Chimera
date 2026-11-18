# Test architecture: an agent can verify without a human

Goal: the suite is a verification instrument an agent can use on its own.
A change is checked at the right layer, with deterministic data, and a
failure leaves evidence (state, pixels, logs), not a request to play the
game. "Done" means:

- every layer below has a harness and a home, and a change runs the layers
  it can affect;
- presentation (what is drawn and heard) is verified headlessly: state
  first, pixels where state cannot prove it;
- tests build their data through typed kits, so no test depends on a game
  install, a checkout's line endings or the wall clock;
- a red test tells the agent what broke and where to look;
- the fast tier stays fast and the heavy tier stays off the normal path.

The current Windows failures are a separate session's job (LF checkouts,
the metrics clock). Treat them as a prerequisite, not as this plan.

## Where the architecture is today

The suite is organised by **crate** (what code it exercises) and **cost**
(`heavy::` or not). That was right when the games were the code. It has no
axis for **what kind of verification** a test does, and three layers are
missing entirely:

| Layer | Harness | Today |
|---|---|---|
| Unit | none | good |
| Contract (engine without a game) | `chimera_sim::testkit`, `Sim` | started (T2) |
| Scenario (a game's behavior) | `Sim`, `NetSim`, game `common` | strong: 2,814 tests, 106k lines in the two games |
| **Presentation (state)** | **scene audit** | **missing** |
| **Presentation (pixels/audio)** | **headless render harness** | **missing** |
| Content (loaders, installs) | fixtures, `heavy::` | good but install-bound |
| **Composition (an app's plugin set boots)** | **app smoke** | partial (`startup.rs` per app) |
| Soak/perf | `NetSim`, `perfgate` | good |

Two consequences:

- **The renderer has 2.4k lines and 42 tests, and no test starts a
  renderer.** Every presentation test checks ECS/asset state. A missing
  material, a bad blend or a view model on the wrong layer passes the whole
  suite and is found by eye.
- **Test data is patched as text.** Tests do `include_str!` +
  `.replace("...\n...")`; 59 `fn sim(`, 26 `fn load(`, 12 `fn feet(` and 9
  `fn active_id(` are copies. The encoding of a checkout and the exact
  text of a fixture are part of too many tests.

## The architecture

Five additions. Each is a layer with an interface, so tests use it instead
of hand-rolling.

### 1. `chimera_present::testkit` (feature `testkit`, dev-dependencies only)

The presentation's missing harness. Same pattern as `chimera_sim::testkit`:
the engine's crate owns the kit, every crate above enables it in dev-deps.

- `TestScene`: a deterministic scene built in code: a camera, a sun, a
  floor, a lit box, an unlit box, two translucent quads at one depth, a
  generated 2x2 texture, and a clear/sky colour. Values are round and
  documented in the module, so a probe's expectations are readable. It
  reuses `greybox` blocks where a real map matters.
- `SceneAudit`: `assert_scene_sane(world, camera)`:
  - every `Mesh3d`/material handle resolves in its `Assets`;
  - transforms and AABBs are finite and non-empty;
  - the camera sees a non-zero number of meshes and has a non-zero target;
  - the local player's view model, if any, is in front of the eye and in
    the near/far range;
  - every camera's target size is non-zero.
  This needs no GPU and catches "a missing presentation plugin draws
  nothing and fails no test" (architecture-review A4/A5).
- `RenderSim`: the smallest headless render app:
  - `DefaultPlugins` without `WinitPlugin`, with
    `WindowPlugin { primary_window: None, exit_condition: DontExit }` and
    no audio device;
  - a 3D camera into an `Image` (320x180 default);
  - `frames(n)`, `shot()`; capture through `Screenshot::image` +
    `ScreenshotCaptured` (pixels in memory, no file I/O, no new crate);
  - `Skip` when no adapter is found, or `CHIMERA_SKIP_RENDER=1`
    (`WGPU_FORCE_FALLBACK_ADAPTER=1` for software rendering);
  - a `Drop` guard: if the test is panicking, write the last frame and the
    scene audit to `target/test-artifacts/<crate>/<test>/`. Every visual
    failure leaves a PNG for the agent to open.
- `Shot`: pixels plus small readings (`mean_luma`, `distinct_colours`,
  `region(x,y,w,h)`, `diff`). Deliberately not a golden-image library.

### 2. `fixture`: typed test data, not text surgery

- `fixture!(path)` returns the file with `\r\n` normalised to `\n`; or
  better, the kit provides builders for the documents tests edit today
  (`WeaponDoc`, `MinigameDoc`) and parses them, so a test states the field
  it changes.
- Convert the 8 files that do line surgery on included data (party_kit's
  `identity`, `net_party`, `rotation`, `rung_characters`, `rung_extras`;
  `chimera_sim`'s `minigame`, `rung_extras`; combat_arms' `inventory`).
- A `repo_checks` rule then refuses new line-ending-sensitive replacements
  in tests (reviewed allowlist), so the class cannot come back.
- This is also the base for a `TestMap`/`TestWeapon`/`TestScene` content
  kit: tests request what they need, not a copy of a game's file.

### 3. Composition smokes

Each app already has a `tests/it` (some have `startup.rs`). Make it
uniform: build the app's real plugin set headless, run N fixed ticks, and
assert the world is non-empty, the map (if any) is loaded, scheduled
systems ran and nothing panicked. A shared `testkit` helper keeps this to
one function per app. This is the automated version of "it boots".

### 4. Meta-tests: the suite checks itself

Extend `repo_checks/checks/tests/it/architecture.rs` (grep-level, seconds):

- `#[ignore]` is a reviewed list, not a growing habit (16 today);
- no `Instant::now`/`SystemTime::now`/`thread::sleep` for simulated
  behavior (allowlist: real sockets, perf, metrics);
- no `include_str!` + `\n`-replacement outside the `fixture` helper;
- every `tests/it/main.rs` registers each file (exists);
- every app has a composition smoke test;
- every `chimera_present` test group that draws has a `SceneAudit` case.

The checks fail with the fix in the message, as the existing ones do.

### 5. Selection and evidence

- `affected` stays the source of truth for crates. Add a **layer** on top:
  - changes in `*/render/*`, `*/ui/*`, `map/spawn*` also run the
    presentation probes (state audit always; pixels when a GPU is there);
  - app composition paths run the smoke;
  - heavy stays opt-in.
- nextest groups: `render` (1 thread, GPU memory), `heavy` (5),
  default (8). Timeouts per group as today.
- `tools/verify.ps1`: one command that picks the layers, builds the tests,
  runs them, writes `target/verify.log`, and prints `VERIFY PASS|FAIL`
  plus the artifacts directory. This is the agent's gate on Windows; the
  same selection is available to the bash hooks.
- Failure output points at the artifact, not just an assertion: scene
  dump for the audit, PNG for a probe, target log for a test.

## Steps

### Step 1: presentation state (no GPU, highest value per line)

- [ ] Add `chimera_present::testkit` with `TestScene` and `SceneAudit`.
- [ ] One audit test per composition: engine base + presentation, CS:S
      greybox, Combat Arms greybox, the party kit's gun game, and each
      app's startup scene.
- [ ] Verify by removing one `PresentationPlugins` entry (e.g. the prop
      material): the audit names the missing plugin; revert.

### Step 2: presentation pixels (the eyes)

- [ ] Add `RenderSim` and `Shot`; the `Drop` artifact guard; the skip path.
- [ ] Four probes on `TestScene`:
      1. an unlit cube at a known screen position covers expected pixels;
         the same cube behind the camera covers none (transform + culling);
      2. two translucent quads at one depth composite in entity order
         (blend order, `render::blend_order`'s bug class);
      3. clear colour, a world face and the sky keep their expected
         colours within tolerance (materials, tonemapping, missing
         textures);
      4. on the greybox with `MapPlugin`, the frame is not black and has
         more than N distinct colours; a view model drawn on its layer
         appears over the world and not when hidden.
- [ ] Keep the module under ~10 s in `chimera_present`'s binary. If shader
      compilation makes that false, run it only from `verify` for render
      paths; do not make every edit pay.
- [ ] Verify each by introducing the bug on purpose (black clear, dropped
      material, swapped blend, hidden view-model layer) and watching the
      matching probe fail; revert.

### Step 3: content and data

- [ ] Add `fixture` + typed builders; convert the 8 line-surgery files.
- [ ] Add the `repo_checks` rule for new line-ending-sensitive surgery.
- [ ] Move the duplicated `sim()`/`load()`/`feet()`/`active_id()` helpers
      into the per-game `common` and `chimera_sim::testkit` (T4), as files
      are touched; no big-bang sweep.

### Step 4: composition and hygiene

- [ ] One `testkit::app_smoke` helper; use it in each app's `tests/it`.
- [ ] Add the meta-tests (`#[ignore]` list, clocks, surgery, smoke
      presence, presentation coverage).
- [ ] Add the nextest `render` group and the `verify.ps1` layer selection.

### Later (not this plan's first slice)

- UI pixels: one `RenderSim` probe per game's HUD/menu (after the engine
  probes prove the harness is cheap).
- Audio by measurement: render the mixer to PCM for muffle, falloff and
  looping instead of listening.
- Property/fuzz tests for `source_formats` and `lithtech_formats` (pure
  readers, nightly job, not the fast tier).
- Heavy tier: the converted-map cache (architecture-review P4b step 2).
- CI (T3) when the user wants it; the audit and probes are exactly the
  install-free, display-free part a runner can do.

## Overhead budget

- Fast tier: +1-2 s for the state audits; the pixel probes run only where
  a GPU exists and stay under ~10 s; the heavy tier does not change.
- One `testkit` module in `chimera_present`, one small additions module in
  `chimera_sim`, one `verify.ps1`, a handful of meta-tests. No new crate,
  no new runtime dependency, no golden-image set.
- The content builders replace text surgery rather than add to it.

## Why this is better than adding tests one by one

- The missing layer is added once; every future visual fix has a place to
  land (state audit or probe) instead of a screenshot request.
- Data comes from builders, so a test cannot be green on Linux and red on
  Windows because of an encoding.
- Meta-tests keep the suite honest as parallel agents add tests: ignored
  tests, clock-dependent tests and text surgery cannot accumulate.
- The layer selection means an agent runs the smallest meaningful set and
  still gets pixels when the change can affect them.

## Progress

2026-10-29: architecture written after surveying the suite: 2,814 game
integration tests and 106k test lines against 42 presentation tests and no
renderer; 59 `fn sim(` copies; 16 ignored tests; 36 "confirm in a
playtest" items.

## Decision log

2026-10-29: presentation gets two levels, state and pixels, in that order.
State is cheap, deterministic and catches most "nothing drawn" bugs; pixels
are reserved for what only the render graph can prove.

2026-10-29: the render kit lives in `chimera_present` behind a `testkit`
feature, as `chimera_sim` does, rather than in a new crate or in each
game's tests. One harness, no layering exception.

2026-10-29: no committed golden images. Goldens of real maps are another
game's content, which the asset rule forbids, and would need a GPU
everywhere. Probes assert invariants on synthetic scenes.

2026-10-29: `fixture`/builders replace `include_str!` + `.replace("\n")`,
and a meta-test enforces it. This is the structural fix for the class of
failures the LF branch is cleaning up.

## Open questions

- Is a headless render probe acceptable in the fast tier (GPU, ~10 s), or
  should it be a separate `render` group the verify command runs only for
  render/UI changes? Recommendation: the latter, with the state audit
  always in the fast tier.
- Should `RenderSim` live in `chimera_present` or in a new
  `chimera/testkit` crate? The feature keeps the crate count down; a crate
  would keep test-only code out of the published API.
- Do we want a generated gap report (spec test cases and playtest items
  vs tests) or only the meta-tests? A report is useful; a failing check on
  day one is not.
