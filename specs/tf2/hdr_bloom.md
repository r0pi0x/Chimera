# Team Fortress 2: HDR, auto exposure, tone mapping and bloom

Source basis: Valve's Source SDK 2013 (2025 release) read in place in a
spec session: the client's post-processing (bloom amount, exposure range,
the luminance histogram and its target, the tone-map goal smoothing, the
order of the passes), the tonemap controller entity on both sides, the
client's level and respawn hooks, the screen-space shaders (downsample,
the two blur passes, the bloom add, the combined post pass, the luminance
compare) and their parameter set-up, and the shared pixel-shader output
helper. The engine's material system and shader API are **not** in the
SDK (binaries): how the current tone-map scale follows its goal, which
HDR mode each `mat_hdr_level` selects and the render targets' formats
are marked *inferred* or left in Open questions. Data: TF2's `dev/`
post-process materials, sky materials and every map's entity lump read
in place (values only); console variable values and help texts read from
the running game (tools/tf2ref, 2026-11-20); pixel values from the
pass-5 bloom on/off frames (docs/plans/active/tf2-parity.md). Nothing
copied; restated in our words.
Status: draft

## Summary

TF2 on PC renders "integer HDR": the scene is drawn into an ordinary
8-bit sRGB frame buffer, and every lit shader multiplies its linear
output by a **tone-map scale** before the write, so the frame clamps at
1.0 like LDR. The scale is driven by a histogram of the frame's
luminance that aims to put the brightest 2 % of the centre's pixels at
60 % luminance, and each map clamps it to a narrow range (2fort:
0.8–1.1, so 2fort is drawn at about scale 1). Bloom is computed **from
the clamped, gamma-encoded frame**: a quarter-size copy, each pixel
weighted by its own brightness (c^2.2 × luminance, a soft threshold),
blurred by a 25-texel Gaussian-like kernel, scaled by the map's bloom
scale (2fort 0.5) and **added in gamma space**, clamped to 1. There is
no tone-map curve: the only "curve" is the linear scale and the 8-bit
clamp. Bright areas (gamma over about 0.8) bloom to white; dark ones
are nearly untouched.

## Units and conventions

- Colours: c is a frame-buffer value in 0..1 as stored (gamma encoded,
  byte / 255). "Linear" means sRGB-decoded. Luminance for metering uses
  linear values; the bloom's weighting uses gamma values.
- W × H: the back buffer size in pixels. The bloom works at W/4 × H/4
  ("quarter texels").
- Time base: **per rendered frame**, not per second, for the bloom
  amount's smoothing and the histogram; the reference runs at 60 fps
  (fps_max 60 in the scenes) unless stated.
- Tone-map scale k: a linear multiplier; k = 1 means "as authored".

## Constants

| Name (ours) | Value | Unit | Meaning |
|---|---|---|---|
| `mat_hdr_level` | 2 (archived) | – | 0 no HDR, 1 LDR + bloom on HDR maps, 2 full HDR on HDR maps (the game's own help text). TF2's presets for DX 9.0b and above set 2; the reference install runs 2 at `mat_dxlevel` 100 and reports "HDR Enabled" on 2fort |
| `mat_disable_bloom` | 0 | – | 1 turns bloom off (client, not a cheat) |
| `mat_force_bloom` | 0 | – | cheat: bloom even where it would be off |
| `mat_bloomscale` | 1 | – | bloom scale when the map sets none |
| `mat_bloomamount_rate` | 0.05 | per frame | smoothing of the bloom amount |
| `mat_non_hdr_bloom_scalefactor` | 0.3 | – | bloom multiplier when the frame is not HDR (`mat_hdr_level` 1) |
| `mat_bloom_scalefactor_scalar` | 1 | – | final bloom multiplier |
| `r_bloomtintr/g/b` | 0.3 / 0.59 / 0.11 | – | weights of the bloom's luminance |
| `r_bloomtintexponent` | 2.2 | – | the bloom's per-channel exponent |
| `mat_autoexposure_min` | 0.5 | scale | exposure floor when the map sets none |
| `mat_autoexposure_max` | 2 | scale | exposure ceiling when the map sets none |
| `mat_tonemap_algorithm` | 1 | – | TF2 uses the "new" (percentile) algorithm (§3) |
| `mat_tonemap_percent_target` | 60 | % | where the bright pixels are put |
| `mat_tonemap_percent_bright_pixels` | 2 | % | the share of pixels counted as "bright" |
| `mat_tonemap_min_avglum` | 3 | % | the floor on the median's luminance |
| `mat_exposure_center_region_x / _y` | 0.9 / 0.85 | fraction | the metered region, centred |
| `mat_dynamic_tonemapping` | 1 | – | cheat: 0 freezes the exposure |
| `mat_hdr_manual_tonemap_rate` | 1 | – | adaptation rate (engine side, §4; set by SetTonemapRate) |
| `mat_accelerate_adjust_exposure_down` | 3 | – | engine side, §4 |
| `mat_hdr_uncapexposure` | 0 | – | cheat: 1 makes the range 0..20 |
| `mat_force_tonemap_scale` | 0 | scale | cheat: > 0 pins the scale |
| `mat_colorcorrection` | 0 on the reference install | – | colour correction (§6) |
| histogram bins | 16 (+1 pixel-count bin) | – | §3 |
| goal history | 10 | frames | §3.4 |
| blur taps | 13 | – | §5.3 |

## Behavior

### 1. Which path runs

Per map load, from `mat_hdr_level` (L) and whether the map has HDR
lighting (the BSP's HDR lighting lump; every TF2 map but cp_cloak,
install.md):

| L | map has HDR | frame | exposure | bloom amount |
|---|---|---|---|---|
| 0 | any | LDR (LDR lightmaps, k = 1) | none | 0 (off) |
| 1 | yes | LDR (k = 1) *inferred* | none | S × 0.3 |
| 1 | no | LDR | none | 0 |
| 2 | yes | integer HDR (HDR lightmaps, HDR sky) | §3 | S |
| 2 | no | LDR *inferred* (Open questions 2) | none *inferred* | 0 |

S is the map's bloom scale (§2), smoothed (§5.1). `mat_disable_bloom 1`
sets the bloom amount to 0 in every row; `mat_force_bloom 1` turns it on
in rows that have a frame to bloom (HDR lighting not required). Bloom is
also off under `mat_fullbright 1`. A bloom amount of 0 skips the bloom
passes entirely (the frame is exactly the no-bloom frame).

In integer HDR (L = 2), the world, models, sprites, SpriteCard particles
and the sky multiply their linear colour by k just before the sRGB
write (specs/cs_source/shaders.md §1 already says so for the world);
HUD and VGUI are drawn after the post-processing with k reset to 1.

### 2. The map's settings: `env_tonemap_controller`

A point entity, always sent to every client. It has no keyvalues of
its own (TF2 maps give it only a targetname); the map drives it with
inputs, normally from a `logic_auto`'s OnMapSpawn:

| Input | Argument | Effect |
|---|---|---|
| SetAutoExposureMin | float | exposure floor = value (used only if > 0) |
| SetAutoExposureMax | float | exposure ceiling = value (used only if > 0) |
| UseDefaultAutoExposure | – | back to the console variables' range |
| SetBloomScale | float | S = value (also sets its "minimum" to value; the minimum is unused) |
| UseDefaultBloomScale | – | S = `mat_bloomscale` |
| SetBloomScaleRange | "max min" | does nothing (Quirks 4) |
| SetTonemapRate | float | sets `mat_hdr_manual_tonemap_rate` (§4) |
| SetTonemapScale | float | sets the tone-map goal variable; overwritten next frame by auto exposure (Quirks 5) |
| BlendTonemapScale | "target seconds" | the same, ramped every 0.1 s; same caveat |

- Range resolution each frame: min = custom min if set and > 0, else
  `mat_autoexposure_min`; max likewise; under `mat_hdr_uncapexposure 1`
  min 0, max 20; then if min > max, max = min.
- The client uses the last controller whose data changed; when that
  entity goes away, the custom flags clear.
- **Per-map values** (every installed map's entity lump, 2026-11-20): 235
  of 238 maps have a controller and all of those set min, max and bloom
  scale at map spawn. Most common: min 0.5 (69 maps), 0.7 (68), 0.8
  (33); max 1.1 (67), 0.8 (51), 1.0 (57); bloom 0.3 (58), 0.5 (54), 0.6
  (31), 0.4 (22). Two set a rate (koth_overcast_final 0.1,
  vsh_distillery 0.35). No controller: cp_cloak (LDR map),
  cp_junction_final, ctf_snowfall_final; those use 0.5–2 and S = 1.

| Map | min | max | bloom S |
|---|---|---|---|
| ctf_2fort | 0.8 | 1.1 | 0.5 |
| cp_badlands | 0.7 | 1.1 | 0.4 |
| cp_dustbowl | 0.7 | 1.1 | 0.3 |
| cp_granary | 0.6 | 1.0 | 0.3 |
| cp_well, cp_gravelpit, arena_well | 0.8 | 1.1 | 0.5 |
| cp_process_final | 0.5 | 0.7 | 0.5 |
| pl_badwater, pl_goldrush | 0.7 | 1.1 | 0.3 |
| pl_upward, cp_5gorge | 0.5 | 0.8 | 0.5 |
| koth_viaduct | 0.5 | 0.8 | 0.6 |
| ctf_turbine | 0.8 | 1.1 | 0.9 |
| tc_hydro | 0.7 | 0.9 | 0.1 |

### 3. Auto exposure (L = 2): the goal

Runs once per rendered frame, after the 3D view (world, effects, view
model) is complete and before bloom, on a copy of that frame (no HUD).

#### 3.1 Metering region and luminance

- Region: the centred rectangle 90 % of the width by 85 % of the height
  (5 % skipped left and right, 7.5 % top and bottom).
- Per pixel: decode the stored value to linear (sRGB), Y = 0.2125 R +
  0.7154 G + 0.0721 B. Values are in 0..1 because the frame is 8-bit.

#### 3.2 Histogram

- 16 bins over 0..1 with edges e_b = (b / 16)^1.5, b = 0..16 (finer at
  the dark end): 0, 0.0156, 0.0442, 0.0812, 0.125, 0.1747, 0.2296,
  0.2894, 0.3536, 0.4219, 0.4941, 0.5701, 0.6495, 0.7324, 0.8185,
  0.9077, 1. A pixel counts in bin b when e_b ≤ Y ≤ e_(b+1) (both ends
  inclusive; the top bin also takes anything above 1).
- A 17th "bin" counts every pixel of the region (all luminances), only
  to scale counts; it is not used by the algorithm below.
- **Staggered refresh:** each frame **one** bin is re-counted (GPU
  occlusion query), and a count is read back at the earliest 3 frames
  after it was issued. At start the bins are issued in order 0..16, one
  per frame; afterwards each frame re-issues the bin whose last count is
  the oldest. So a bin's count is about 17–20 frames old and bins come
  from different frames. Until every bin has a count the goal is held
  (§3.3, "no data").

#### 3.3 Target scale

With N = the sum of the 16 bins' counts:

1. **Bright point P(p)**, the luminance above which a fraction p of the
   counted pixels lie: walk the bins from the brightest down,
   accumulating each bin's share n_b / N, until a bin holds the
   remaining share needed; inside that bin interpolate linearly from its
   **top** edge down: P = e_(b+1) − (e_(b+1) − e_b) × (needed / share_b).
   If any bin walked has no count yet, or N = 0, there is no data.
2. P_bright = P(0.02), with the **sticky rule**: if the bin found is the
   one containing the target 0.60 (bin 11, 0.5701–0.6495), P_bright =
   0.60 exactly. No data → P_bright = 0.60.
3. r1 = 0.60 / max(P_bright, 0.0001).
4. P_median = P(0.50) (no sticky rule). If P_median > 0:
   r2 = 0.03 / P_median, and r = max(r1, r2) (the median is never left
   below 3 % luminance). Else r = r1.
5. Target = max(0.001, r × k_now), where k_now is the scale the frame
   was drawn with (the metered frame already includes it).
6. Clamp to the range of §2: t = clamp(target, min, max), then t =
   max(t, 0.001).

So the scale holds while the 98th-percentile luminance sits in
0.570–0.650 (gamma about 0.78–0.83, bytes 198–211), rises when the
scene's brightest 2 % are darker and falls when they are brighter.

#### 3.4 Goal smoothing

Each frame's t is pushed into a 10-entry history (oldest first). Until
the history is full the goal is t itself. Once full:

- goal = Σ w_i h_i / Σ w_i with w_i = |i − 5| / 5 for i = 0..9 (oldest
  i = 0): weights 1, 0.8, 0.6, 0.4, 0.2, 0, 0.2, 0.4, 0.6, 0.8 (sum 5).
  See Quirks 1.
- goal = clamp(goal, min, max).

The history is emptied and the current scale set to exactly 1.0 at
level start, when the local player respawns, and when the spectated
target changes.

### 4. The current scale follows the goal (engine side)

The scale drawn with, k, moves towards the goal each frame inside the
engine's material system, which is not in the SDK. What is known: its
speed is set by `mat_hdr_manual_tonemap_rate` (1, maps may set it with
SetTonemapRate: 0.1 and 0.35 on two maps), and
`mat_accelerate_adjust_exposure_down` (3) is the factor by which
darkening (k going down) is faster. Its exact law is Open question 1.
Provisional, until measured: k moves exponentially towards the goal
with a time constant of 1 s / rate when brightening and 1/3 of that
when darkening. For 2fort the clamp (0.8–1.1) bounds any error to
±20 %; at the reference views the game reported goals of 0.943–0.944.

### 5. Bloom

#### 5.1 Bloom amount

Each frame while bloom is on (§1): A ← 0.05 × S + 0.95 × A (per frame),
where A starts at 1.0 **once per game launch** (not per map) and is
not updated while bloom is off. Then the frame's bloom amount is
B = A × (0.3 if the frame is not HDR) × 1. On 2fort at L = 2, B → 0.5
(within 1 % after 90 frames from a start at 1.0).

#### 5.2 Downsample with the soft threshold

Source: the finished 3D frame (after the view model, before the HUD), as
stored bytes c (gamma), **not** decoded. Destination: a W/4 × H/4
8-bit target.

- Shape, per sample: ℓ = 0.3 c_r + 0.59 c_g + 0.11 c_b; s = c^2.2 × ℓ
  (per channel). So a grey g gives g^3.2: white 1, 0.9 → 0.714,
  0.8 → 0.490, 0.6 → 0.195, 0.5 → 0.109, 0.25 → 0.012.
- Each quarter texel takes four bilinear samples, each the average of a
  2 × 2 block of the source, together covering the 4 × 4 block under the
  texel; **the shape is applied to each 2 × 2 average**, then the four
  results are averaged. (Within about a pixel of the block; the exact
  sub-pixel alignment is not significant.)
- Written as 8-bit (each channel clamped to 0..1 and rounded).

#### 5.3 Blur (horizontal, then vertical)

Both passes are the same 13-tap filter on the quarter target, with
bilinear filtering, written back as 8-bit:

| Tap offset (quarter texels) | 0 | ±1.3366 | ±3.4295 | ±5.4264 | ±7.4359 | ±9.4436 | ±11.4401 |
|---|---|---|---|---|---|---|---|
| Weight (each tap) | 0.2013 | 0.2185 | 0.0821 | 0.0461 | 0.0262 | 0.0162 | 0.0102 |

The weights sum to 0.9999. With bilinear sampling the horizontal pass
is the discrete kernel (texel offset: weight, symmetric): 0: 0.2013,
1: 0.1450, 2: 0.0735, 3: 0.0468, 4: 0.0353, 5: 0.0264, 6: 0.0197,
7: 0.0148, 8: 0.0114, 9: 0.0090, 10: 0.0072, 11: 0.0057, 12: 0.0045
(σ ≈ 3.75 quarter texels ≈ 15 screen pixels; reach ±12 quarter texels
≈ ±48 pixels).

- Horizontal pass: offsets as listed, result × 1.
- Vertical pass: the offsets are scaled by **H / W** (Quirks 2: the
  step is computed from the target's width), i.e. × 0.75 at 1024 × 768,
  × 0.5625 at 16:9; result **× B** (the bloom amount), then written as
  8-bit (clamped to 1).

#### 5.4 Adding bloom

The quarter-size bloom image is stretched over the screen with
bilinear filtering and **added to the stored frame bytes in gamma
space** (additive blend, no sRGB conversion on read or write): out =
min(1, c + bloom). With colour correction on, the same sum is formed in
the combined pass and fed to the correction (§6). Nothing else in the
frame changes: there is no exposure applied to the bloom, and no
threshold besides the shape of §5.2.

Consequences for uniform areas (bloom ≈ B × shape(c) once the area is
much wider than the kernel), at 2fort's B = 0.5, in bytes: grey 64 →
66, 128 → 142, 204 → 255, (157, 145, 122) → (182, 166, 136), the
flame's (255, 230, 180) → (255, 255, 234). Small bright details
(narrower than about 25 screen pixels) bloom less than this: their
shape value is spread over the kernel.

### 6. Colour correction and the rest of the post chain

Order after the 3D view: copy the frame → exposure metering (§3) →
bloom image (§5.2–5.3) → one pass that adds bloom and applies colour
correction (and software AA, off on PC: `mat_software_aa_strength` 0)
→ Pyro vision effects (if any) → HUD. Colour correction is on only with
`mat_colorcorrection 1` and a `color_correction` entity with weight;
2fort has one (`scripts/2fort_global.raw`, everywhere). TF2's DX-level
presets set `mat_colorcorrection` 1 at DX 9.0 and above, but the
reference install (tools/tf2ref) runs with it 0, so the reference
captures have no colour correction; with it 0 the bloom is added by its
own additive pass, as §5.4.

### 7. Particles' overbright and HDR

- SpriteCard and UnlitGeneric sprites multiply their linear output by k
  (integer HDR), then the output is written to the 8-bit sRGB target:
  **each fragment's colour is clamped to 0..1 before blending**, and the
  blended result clamps again. So `$overbrightfactor` 5 or `$addself`
  can make one layer at most white; brightness beyond 1 survives only
  as "several layers saturating". There is no float frame buffer on this
  path. *Inferred* for the fragment clamp: an 8-bit (unsigned
  normalised) target clamps blend inputs (D3D9 and Vulkan rules).
- Bloom then sees only the clamped bytes: a saturated flame core blooms
  exactly as much as a white wall would.
- At 2fort's k ≈ 0.94 a particle is drawn about 6 % darker than at
  k = 1 (linear), before the clamp.

## Per-frame order

1. Draw the 3D view: sky, world, models, translucents and particles,
   view model, each fragment × k (integer HDR) and clamped to 0..1 on
   the 8-bit sRGB write.
2. Copy the frame. If L = 2 on an HDR map: one histogram bin issued,
   one older bin read (§3.2); target t (§3.3); goal (§3.4).
3. Bloom amount A, B (§5.1). If B > 0: downsample (§5.2), blur
   horizontal, blur vertical × B (§5.3).
4. Add bloom (and colour correction) to the frame (§5.4, §6).
5. HUD and VGUI at k = 1.
6. Engine: k moves towards the goal for the next frame (§4).

## Edge cases

- Bloom on an LDR-only map (cp_cloak): none, at any `mat_hdr_level`
  (unless `mat_force_bloom`).
- `mat_hdr_level` changes take effect on the next map load (the
  lighting lump and the HDR mode are chosen then; *inferred*, as Chimera
  already does).
- Under water the world is drawn into reflection and refraction targets
  at k × 0.25 (an integer-HDR detail of the water path; out of scope).
- Very dark scenes: the median rule (§3.3 step 4) brightens until the
  median reaches 3 % luminance, then the range clamps it.
- A frame where the brightest 2 % fall in bin 11 holds k exactly; the
  sticky band is 0.570–0.650.

## Quirks

1. **Goal history weights**: the smoothing weighs the oldest and newest
   entries most and the middle one (5 frames old) not at all. Keep it:
   it is what the game does.
2. **Vertical blur step**: computed from the quarter target's width, so
   the vertical blur is narrower than the horizontal by H/W (bloom
   halos are wider than tall). Keep.
3. **Bloom amount smoothing is per frame and per launch**: frame-rate
   dependent, starts at 1.0 when the game starts and carries between
   maps, so the first second after loading 2fort fresh blooms at up to
   twice its settled strength. Keep (or settle it at map load if a test
   needs a fixed value).
4. **SetBloomScaleRange** never takes effect (its argument parsing
   fails and it only warns). No TF2 map uses it.
5. **SetTonemapScale / BlendTonemapScale** write the goal variable on
   the server's side; on clients the auto exposure overwrites the goal
   every frame, so they have no visible effect while
   `mat_dynamic_tonemapping` is 1 (*inferred* for clients of a dedicated
   server, where the server's variable is not the client's).
6. **The view model is metered and bloomed** (it is part of the 3D frame
   before post-processing); the HUD is not.
7. The bloom image's sampling is offset by half a quarter texel
   vertically (about 2 screen pixels); negligible.

## Test cases

| Setup | Input | Expected |
|---|---|---|
| bloom shape | grey c = 0.8 | ℓ = 0.8, s = 0.8^3.2 = 0.490 |
| bloom shape | c = (157, 145, 122)/255 | ℓ = 0.573, s = (0.197, 0.166, 0.113) |
| uniform frame, 2fort B = 0.5 | bytes 128 everywhere | bloom 14 → 142 |
| uniform frame, B = 0.5 | 204 | bloom 62 → 255 (clamped) |
| uniform frame, B = 0.5 | (157, 145, 122) | (182, 166, 136) |
| uniform frame, B = 0.5 | (255, 230, 180) | (255, 255, 234) |
| uniform frame, B = 0.15 (L = 1 on 2fort) | 128 | 132 |
| uniform frame, B = 0.15 | 204 | 223 |
| uniform frame, B = 1 (no controller) | 128 | 156 |
| `mat_disable_bloom 1` | any frame | unchanged bytes |
| one lit quarter texel (value 1) | horizontal blur | 0.2013 at 0, 0.1450 at ±1, 0.0735 at ±2, …, 0.0045 at ±12 |
| vertical blur at 1024 × 768 | offset scale | 0.75 (1.0025, 2.5721, 4.0698, 5.5769, 7.0827, 8.5801 quarter texels) |
| bloom amount, S = 0.5, A = 1 at start | 1 frame / 14 frames / 90 frames | 0.975 / 0.744 / 0.505 |
| bins | edges | bin 11 = 0.5701–0.6495 |
| exposure, 2fort range, k_now = 1 | P_bright = 0.8, P_median = 0.2 | r1 = 0.75, r2 = 0.15 → 0.75 → clamped 0.8 |
| exposure, 2fort range, k_now = 1 | P_bright = 0.5, P_median = 0.15 | r1 = 1.2 → clamped 1.1 |
| exposure, any range, k_now = 0.9 | P_bright in bin 11 | target = 0.9 (held) |
| exposure, range 0.5–2, k_now = 1 | P_bright = 0.1, P_median = 0.005 | r1 = 6, r2 = 6 → 6 → clamped 2 |
| goal smoothing | history 1 × 9 then 0.8 (newest) | (4.2 + 0.64) / 5 = 0.968 |
| goal smoothing | history 0.8 then 1 × 9 | (0.8 + 4.0) / 5 = 0.96 |
| P(p) interpolation | all pixels in bin 15 (0.9077–1) | P(0.02) = 1 − 0.0923 × 0.02 = 0.9982 |
| 2fort, measured (pass 5, 1024 × 768, FOV 75, bridge) | wall region bytes (157, 145, 122) without bloom | with bloom (188, 174, 141); the uniform-area model gives (182, 166, 136): the brighter neighbourhood adds the rest |
| 2fort, measured | floor (145, 124, 102) | with bloom (159, 133, 107); model (163, 137, 110) |
| 2fort, measured | dark (63, 55, 46) | unchanged; model +1 |
| 2fort flame fireball, measured | near-white pixels (mean > 220, red > 200) | ~8,300–9,300 with bloom, ~1,700–2,900 without (flames.md Open questions 1) |

## Mapping to our renderer (implementation notes)

- Our HDR stand-in (`client::hdr`, Bevy's bloom and auto exposure on a
  float target) does none of the above; its exposure target (average
  0.5) is what washes 2fort out. To match TF2 on 2fort: draw at scale
  k (0.8–1.1, about 0.94 in the reference views), clamp to 1 and encode
  sRGB as at L = 0, then run §5 on the encoded bytes.
- To compare with the reference captures: TF2 there runs L = 2, bloom
  on, colour correction off (§6).

## Open questions

1. **How k follows its goal** (§4): the law (exponential, linear, per
   frame or per second), the meaning of rate 1, and how the ×3 for
   darkening applies. Check: in tools/tf2ref with `fps_max 60`, look
   from a dark interior (2fort's basement) out to the courtyard with
   `mat_autoexposure_min 0.5; mat_autoexposure_max 2` set through
   UseDefaultAutoExposure (or on a map without a controller), record a
   movie, and fit the mean brightness of a fixed patch over time; repeat
   with `mat_hdr_manual_tonemap_rate 0.5`.
2. **L = 2 on an LDR-only map and L = 1 on an HDR map**: which lightmaps
   and HDR mode the engine picks (inferred: LDR lightmaps and k = 1 in
   both). Check: `mat_hdr_enabled` and `mat_hdr_tonemapscale` on cp_cloak
   at L = 2, and on 2fort at L = 1.
3. **Render-target formats**: the quarter targets and the back buffer
   are taken as 8-bit unsigned normalised (consistent with every shader
   above and with the measured clamp); confirm with `mat_dump_rts 1`.
4. The bloom amount's state across map changes (§5.1, Quirks 3) is read
   from a function-local value that is initialised once; confirm by
   loading 2fort after a map with S = 1 and comparing the first frames.
5. Exact sub-pixel placement of the downsample taps and the
   bloom-to-screen stretch (§5.2, Quirks 7); a single-pixel white dot in
   a black frame captured with bloom on would settle it.
