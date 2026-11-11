---
name: screenshot-and-compare
description: Check a rendering or HUD change visually - before/after screenshots of the same frozen scene, pixel diffs, and refcmp comparisons against the real game.
---

# Screenshot and compare

Images go to a scratch folder, never the repo. Needs CS:S installed.

**Before/after of one scene** (rendering refactors, HUD changes):
1. Build the old commit and the new one (two worktrees, or stash).
2. Same command for both, game time frozen so nothing moves:
   ```
   cargo run --features dev -- --map cs_source:de_dust2 --window 1280x720 \
       --spawn 0,75,0 --look 30,-10 --movement chimera:noclip \
       +host_timescale 0 --screenshot $OUT/before.png --frames 120
   ```
3. Compare: `magick compare -metric AE $OUT/before.png $OUT/after.png $OUT/diff.png`
   (pixel count; 0 for a pure refactor). Look at `diff.png` and crops
   (`magick <img> -crop WxH+X+Y`).
4. Live scenes over the remote console (`setpos`, `setang`, `give`,
   `screenshot <file>`): docs/OBSERVABILITY.md sections 2 and 3b.
5. Party apps (Ultimate Gun Game, Lucker Party) draw a ladder's bodies,
   attachments and melee at random per launch: set
   `CHIMERA_RUNG_SEED=<n>` for both runs (docs/OBSERVABILITY.md section 2).

**Against our own captures, many views** (no reference needed):
```
cargo build --features dev
cargo run --features dev,tools --bin refcmp -- capture-ours --out $OUT --only a_sign
cargo run --features dev,tools --bin refcmp -- ordercheck --views tools/refcmp/<map>.toml --out $OUT
```
**Against the real CS:S** (reference captures from a machine running CS:S):
`refcmp` / `refcmp report` / `refcmp hudcmp`, docs/OBSERVABILITY.md
section 6. refcmp builds nothing: run `cargo build --features dev` first.
