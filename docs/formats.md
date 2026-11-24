# Converted formats and their versions

Generated from the sources by `cargo run -q -p repo_checks --bin docs_index`
(`repo_checks::formats`): do not edit. The architecture test
`generated_docs_are_fresh` fails when a number or a hashed path below
changes without this file. The policy (when to bump a number, what a
converter hashes, how long the asset service keeps a version):
[plans/active/converted-cache.md](plans/active/converted-cache.md),
"Converter versions" and "Served entries".

## Numbered by hand

| Format | Version | Where |
|---|---|---|
| Asset manifest (`chimera-manifest`) | 1 | `chimera/assets/src/manifest.rs` `FORMAT` |
| Converted-map cache entry (`CHIMAP`, also a served entry) | 3 | `chimera/assets/src/map_cache.rs` `MAGIC` |
| Served entry's recipe (`chimera-recipe`) | 1 | `chimera/assets/src/map_cache/served.rs` `RECIPE_FORMAT` |
| Map format's bump (with the hash of its sources, below) | 1 | `chimera/world/src/map/cache.rs` `FORMAT_BUMP` |
| Halo's campaign character units (`halo_ce:character_unit`) | 2 | `games/halo_ce/src/game/campaign/unit.rs` `VERSION` |
| TF2's item pack (`tf2:item_stats`) | 3 | `games/tf2/src/game/items/mod.rs` `VERSION` |

## Made by build scripts

A converter's version is `v<number>-<hash>`: the number bumped by hand and
the hash of the paths it lists and of the parsing crates' versions in
`Cargo.lock`. The map format's is the hash of its paths and `FORMAT_BUMP`.

| Build script | Variable | Number | Hashed paths | Parsing crates |
|---|---|---|---|---|
| `chimera/world/build.rs` | `CHIMERA_MAP_SOURCES` | - | `src/map`, `../core/src/core.rs`, `../vfs/src/lib.rs` | - |
| `games/combat_arms/build.rs` | `CHIMERA_CONVERTER_SOURCES` | 1 | `src/game/world.rs`, `src/game/model.rs`, `src/game/props.rs`, `src/game/render_style.rs`, `src/game/volumes.rs`, `src/game/characters.rs`, `src/game/characters/kept.rs`, `src/game/characters/models.rs`, `src/game/characters/records.rs`, `src/game/characters/skeleton.rs`, `src/game/characters/families.rs`, `src/game/effects`, `src/game/objects/mod.rs`, `src/game/objects/geometry.rs`, `src/game/objects/lights.rs`, `src/game/objects/markers.rs`, `src/game/objects/sound.rs`, `src/game/view_model.rs`, `src/game/view_model/hands.rs`, `src/game/view_model/parts.rs`, `src/game/view_model/attached.rs`, `src/game/data.rs`, `src/game/data/attributes.rs`, `src/game/data/store.rs`, `src/game/data/families.rs`, `data/effects.toml`, `data/render_styles.toml`, `data/hands.toml`, `data/surfaces.toml`, `data/debris.toml`, `data/npc_bodies.toml`, `../../lithtech/formats/src`, `../../chimera/formats/src` | `aes`, `glam`, `symphonia*`, `postcard` |
| `games/cs_source/build.rs` | `CHIMERA_CONVERTER_SOURCES` | 1 | `src/game/bsp.rs`, `src/game/hud.rs`, `src/game/sound.rs`, `src/game/radio.rs`, `src/game/impact_effects.rs`, `src/game/objectives.rs`, `../../source/engine/src/content`, `../../source/engine/src/hud/look.rs`, `../../source/engine/src/effects`, `../../source/formats/src`, `../../chimera/formats/src` | `vtf`, `vmdl`, `vbsp`, `vmt-parser`, `image`, `half`, `lzma-rs`, `bzip2`, `glam`, `symphonia*`, `postcard` |
| `games/dods/build.rs` | `CHIMERA_CONVERTER_SOURCES` | 2 | `src/game/maps.rs`, `src/game/bodies.rs`, `src/game/weapons`, `data/weapons`, `../../source/engine/src/content`, `../../source/engine/src/hud/look.rs`, `../../source/engine/src/effects`, `../../source/formats/src`, `../../chimera/formats/src` | `vtf`, `vmdl`, `vbsp`, `vmt-parser`, `image`, `half`, `lzma-rs`, `bzip2`, `glam`, `symphonia*`, `postcard` |
| `games/halo_ce/build.rs` | `CHIMERA_CONVERTER_SOURCES` | 1 | `src/game/maps.rs`, `src/game/maps`, `src/game/body`, `src/game/view`, `src/game/fx`, `src/game/audio`, `../../blam/formats/src` | `flate2`, `glam`, `postcard` |
| `games/hl2dm/build.rs` | `CHIMERA_CONVERTER_SOURCES` | 1 | `src/game/maps.rs`, `src/game/bodies.rs`, `src/game/hud.rs`, `src/game/flashes.rs`, `src/game/shells.rs`, `src/game/sounds.rs`, `src/game/effects.rs`, `data/player_models.toml`, `../../source/engine/src/content`, `../../source/engine/src/hud/look.rs`, `../../source/engine/src/effects`, `../../source/formats/src`, `../../chimera/formats/src` | `vtf`, `vmdl`, `vbsp`, `vmt-parser`, `image`, `half`, `lzma-rs`, `bzip2`, `glam`, `symphonia*`, `postcard` |
| `games/tf2/build.rs` | `CHIMERA_CONVERTER_SOURCES` | 4 | `src/game/maps.rs`, `src/game/bodies.rs`, `src/game/view.rs`, `src/game/hud.rs`, `src/game/sets.rs`, `../../source/engine/src/content`, `../../source/engine/src/hud/look.rs`, `../../source/engine/src/effects`, `../../source/formats/src`, `../../chimera/formats/src` | `vtf`, `vmdl`, `vbsp`, `vmt-parser`, `image`, `half`, `lzma-rs`, `bzip2`, `glam`, `symphonia*`, `postcard` |
