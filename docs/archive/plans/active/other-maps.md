# Other stock maps (backlog 1E)

Goal: the stock CS:S maps besides de_dust2 load cleanly and look like the
game. Priorities (user, 2026-10-06): de_aztec's missing textures, cs_office
z-fighting and things not loading, lighting on those maps; then de_nuke and
the rest.

How to check:

- Load warnings for every stock map: `cargo test --features dev --test
  it all_stock_maps_warnings -- --ignored --nocapture`, or `mapsweep
  --stock` (with `--audit`: entities, I/O, doors and triggers too).
- Views: `tools/refcmp/{de_aztec,cs_office,de_nuke}.toml` (the maps'
  intermission cameras, first T/CT spawns, bomb/rescue zones four ways;
  cs_office adds hostage rooms and a window). Ours: `cargo run --features
  dev,tools --bin refcmp -- capture-ours --views tools/refcmp/<map>.toml`. The
  reference game is shared: ask the coordinating session before capturing
  it (`capture-ref`).
- `games/cs_source/tests/it/heavy/map_stock.rs` holds a test per fix, and ignored debug aids:
  `props_near` (props and their light near a point), `dark_props`
  (props whose baked light is near black from every side), `dark_textures`
  (near-black textures, mips, mod2x details), `pick` (which surface a
  `--views` pixel shows).

## Catalog

| # | Bug | Maps | Where | Cause | Status |
|---|---|---|---|---|---|
| 1 | Stone walls untextured (debug colours), 59 "Unknown variant worldtwotextureblend" errors | de_aztec | everywhere | Shader WorldTwoTextureBlend unknown to the VMT parser | Fixed: parsed as LightmappedGeneric, `$detail` blended over the base by its alpha (new detail mode 2) |
| 2 | Props black in shade (crates, barrels, bars) | all BSP v19 maps (aztec, office, assault, compound, havana, italy, cbble, chateau, piranesi, port, prodigy, tides), de_nuke, de_train | everywhere | Ambient cubes were read only from the v20 sample lumps; v19 stores one cube per leaf inside the leaf, early v20 one per leaf in lump 56 without an index | Fixed |
| 3 | Brush entities missing: glass windows, doors, func_brush/illusionary/breakable geometry; walk-through | all (office: 14 windows, door; nuke: 55 func_breakable, doors, rotating fans) | | Only the world model was converted | Fixed: drawn (rendermode 10 and disabled func_brush skipped) and solid where they spawn (doors closed, breakables whole; illusionary never) |
| 4 | World brush collection read the wrong leaves | all | | vbsp sorts its leaf list by cluster; the tree walk indexed it as stored | Fixed: walks the raw leaf lump |
| 5 | Black triangles and bow ties around lights | de_nuke (reactor, warehouse) | cam10, cam11, cam15 | `$additive` materials (light glows, beams) drawn opaque | Fixed: `MapAlpha::Add` (world and prop shaders) |
| 6 | HDR-capable skies missing ("sky ..._hdrrt: no texture") | de_nuke, de_train, de_dust, cs_militia | sky | `Sky` shader's `$basetexture` not read | Fixed |
| 7 | Static props not found (`./models/...`) | de_nuke (generator), de_train (4), cs_assault | | Paths with a `./` component | Fixed in path normalization |
| 8 | Self-lit porthole decals failed to parse | de_nuke | | DecalBaseTimesLightmapAlphaBlendSelfIllum unknown to the parser | Fixed as a translucent lit decal; self-illumination not modelled |
| 9 | "Decoding Uv88 images is not supported" | aztec, inferno, militia, chateau, piranesi, port | water | Water's `$bumpmap` is a DuDv map | Fixed: water uses `$normalmap` |
| 10 | Water surfaces black | de_aztec canals, others without a base texture | aztec cam1 | No Water shader (no base texture; refraction/reflection/fog missing) | Fixed: the Water shader (specs/cs_source/water.md, `render::water`): refraction, planar reflection, cheap cubemap pass, under-water fog |
| 11 | Decals that find no surface (assault 45, nuke 21, train 21, others ≤ 8) and overlays without geometry (train 8, assault 5) | many | | Displacements: now projected onto (compound 6→2, port 8→3, tides 5→3). Most of the rest lay on large faces whose corners were all beyond 4× the decal's reach (a quick skip by corner distance, now by the face's box). Decals whose probe (origin ∓ 5 units) hits a solid drawn brush entity go on its faces (and move with movers); overlays listing brush entity faces are placed in that entity's space | Fixed: 131 → 13 unplaced (dust2 5, tides 3, compound 2, dust 2, train 1), by props or beyond the inferred reach. The overlays left list no faces (the compiler dropped their sides) or faces away from their quad (militia's has corrupt coordinates) |
| 12 | Black blocks over building walls | cs_office (street, garage), de_nuke | office cam0, left | Additive security-light decals (`decals/offseclight`) drawn opaque | Fixed with 5 |
| 13 | View model drawn in `--views` captures | all | | No `r_drawviewmodel` equivalent; capture hides nothing | Open (client) |
| 15 | Switchable lights that start on missing (office projector, assault red lights) | cs_office, cs_assault | office h1 room | Only lightmap style 0 was read | Fixed: styles lit at map start are summed |
| 16 | 3D skybox props unlit by the sun (cooling towers teal where CS:S's are light grey) | de_nuke (and any static prop in a 3D skybox with the map above it) | nuke cam16, cam6 | A sun ray ran 500 m: past the skybox's own sky, into the map's underside | Fixed: it ends at the first sky brush (`ambient::Occluders::with_sky`); nuke cam16 0.041 -> 0.032, cam6 0.042 -> 0.036, cam2 0.025 -> 0.017; office and aztec unchanged |
| 14 | `--views` crashed (wgpu: attachments of different sizes) | all | | The view-model camera stayed on the window when `--views` retargeted the main camera | Fixed: follows the anchor camera's target |

Z-fighting: none found on cs_office, de_nuke or de_aztec. Method: each
view captured twice, the second 2 mm to the side; z-fighting flickers
between the two while everything else stays put (`magick compare -metric
AE -fuzz 19%` per pair, view model cropped out). Only alpha-tested edges
(branches, fences) differed. The most likely things the user saw before
these fixes were the black props (2) and missing windows (3); if it
persists, a view (`getpos`) of where would let the same check find it.

Also looked at (no new bugs beyond the open items): de_train, de_inferno,
cs_italy, de_dust (views in tools/refcmp/). de_train's bomb-zone views sit
under train cars and inferno's cam10 inside a wall (generated positions,
not bugs).

Survey 2026-10-18 (cs_office, de_aztec, de_nuke and all the others but
de_dust2 and cs_militia): `mapsweep --stock --audit` (no load failures; no
door, button or mover the scripted player found stuck; warnings only the
decal/overlay counts above), our views of every map
(`tools/refcmp/<map>.toml`, six added: cbble, chateau, piranesi,
prodigy, tides, havana), views of each map's brush entities (doors,
breakables, func_brush/illusionary/wall, generated from their bounds),
an audit of the shaders and VMT parameters the maps' world and prop
materials use against what the material loader reads (nothing used that
matters left unread: proxies are AnimatedTexture, TextureScroll, WaterLOD
and two ToggleTexture on de_cbble), `dark_props` and refcmp on the three
maps with references. Found and fixed: 16. Looked at and left:

- de_train's 8 and cs_assault's 5 overlays without geometry list no
  faces at all in the BSP (the compiler dropped them): CS:S draws nothing
  for them either.
- de_cbble's four `ambient_generic`s name `ambience/sheep.wav` (and cow,
  crow, sparrow), files the game doesn't have (it ships
  `ambient/sheep.wav`): silent in CS:S too unless the engine maps the
  folder; worth a listen (sound, not this plan).
- cs_havana: `breen_monitor_1_breakable_1` (a func_breakable) is parented
  to a physics prop, and three of the map's outputs name entities it
  doesn't have (`breen_mic_1`, `..._screen_1`, `..._glass_rl_1`): the
  map's own leftovers. The brush follows its physics parent (2026-10-18:
  place and turn, its collider off for physics bodies, still shot;
  parity-gaps "Brushes parented to physics props").
- de_prodigy's five vent fans name `info_lighting_relative`s as their
  lighting origin (`lightingoriginhack`), which nothing reads; ours light
  them from their own origin and they look lit (windows-captures sitting
  4 item 4 to compare).
- `fogblend` (two fog colours by view direction) is set on cs_compound
  and de_port, not only mg_kommando as backlog section 0 says; unread.
  At their fog distances (from 3000 and 2048 units) it would show only
  on the far skyline (windows-captures sitting 4 item 3). No spec gives
  the blend rule (2026-10-18): written up as specs/cs_source/shaders.md
  open question 21 (curve, colour space, `fogdir` vs `use_angles`,
  per view or per pixel), not guessed.
- Dark props (`dark_props`): de_port's tunnel pipes, de_nuke's warehouse
  floodlights, de_tides' shelves are near black from every side, in
  places that are dark in our lightmaps too; no reference to tell.
- The generated brush-entity views that look broken (de_prodigy rails,
  cs_havana and de_chateau white or black planes) are cameras inside a
  floor or outside the map, not bugs.

## Next

- Reference captures done 2026-10-06 (refcmp `all`): after implementing
  WorldTwoTextureBlend from its spec, de_aztec's 13 views match in
  brightness to within 0.01 (mean abs diff 0.025-0.064; before: luma up to
  +0.09, diff up to 0.18). cs_office: diff 0.010-0.041 except cam0
  (0.093; ours a little darker, 0.584 -> 0.522), ours generally ~3%
  darker. de_nuke: 0.007-0.036 except cam6/cam7/cam13 (~0.05). Next: look
  at office cam0 and nuke cam6/7/13 side by side.
- Other stock maps' views (de_train, de_inferno, cs_italy, de_dust, ...).
