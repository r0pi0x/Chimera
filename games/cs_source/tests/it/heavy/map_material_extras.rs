//! Material keys past the base look, on real maps (skipped without an
//! install or the community map): `$surfaceprop2` on blended
//! displacements (cs_militia's ground: grass where its second texture
//! shows), AnimatedTexture proxies (de_nuke's pool, cs_office's projector
//! slide), LightmappedGeneric `$color` (a community map's DirectX 7
//! water stand-in; no stock map's world material sets it) and sprite
//! cards' screen-size limits (the blood and fire particles).

use bevy::math::Vec3;

use crate::stack::{
    games::{self, cs_source},
    map::{MapData, sound::SurfaceGrid},
    mount::config::{LocalConfig, content_dir},
};

fn load(name: &str) -> Option<MapData> {
    LocalConfig::load().ok()?.game_path_in(cs_source::GAME, cs_source::INFO.steam.as_ref())?;
    // The install's maps, or the asset store's standing in for it.
    let stock = ::cs_source::game::INFO.mount().is_some_and(|m| m.contains(&format!("maps/{name}.bsp")));
    let cached = content_dir(cs_source::GAME).is_some_and(|d| d.join("maps").join(format!("{name}.bsp")).is_file());
    if !stock && !cached {
        eprintln!("skipping {name}: not installed");
        return None;
    }
    Some(games::load_map(&format!("cs_source:{name}")).unwrap_or_else(|e| panic!("{name}: {e}")))
}

/// cs_militia's blended ground: a triangle whose corners show the second
/// texture takes `$surfaceprop2`, footsteps (`below`) and impacts
/// (`nearest`) alike; one showing the first keeps `$surfaceprop`.
#[test]
fn militia_ground_takes_its_second_surface() {
    let Some(map) = load("cs_militia") else { return };
    let grid = SurfaceGrid::new(&map);
    let blended: Vec<_> = map
        .meshes
        .iter()
        .filter(|m| m.surface2.is_some() && m.entity.is_none() && !m.skybox)
        .collect();
    let names: Vec<_> = blended.iter().map(|m| m.material.as_str()).collect();
    assert!(
        names.contains(&"nature/blendmilground004_2"),
        "militia's dirt/grass blend: {names:?}"
    );
    let ground = blended
        .iter()
        .find(|m| m.material == "nature/blendmilground004_2")
        .unwrap();
    assert_eq!(ground.surface.as_deref(), Some("dirt"));
    assert_eq!(ground.surface2.as_deref(), Some("grass"));
    assert_eq!(ground.blend_weights.len(), ground.positions.len());
    // Upward triangles fully one texture or the other, and what the grid
    // says right above them (another surface may lie over a few).
    let (mut first, mut second) = ((0, 0), (0, 0));
    for t in ground.indices.as_chunks::<3>().0 {
        let p = t.map(|i| Vec3::from(ground.positions[i as usize]));
        let w = t.map(|i| ground.blend_weights[i as usize]);
        if (p[1] - p[0]).cross(p[2] - p[0]).normalize_or_zero().y < 0.7 {
            continue;
        }
        let centre = (p[0] + p[1] + p[2]) / 3.0;
        let step = grid.below(centre + Vec3::Y * 0.05, 0.2);
        let hit = grid.nearest(centre, 0.01);
        if w.iter().all(|w| *w >= 0.95) {
            second.0 += 1;
            second.1 += usize::from(step == Some("grass") && hit == Some("grass"));
        } else if w.iter().all(|w| *w <= 0.05) {
            first.0 += 1;
            first.1 += usize::from(step == Some("dirt") && hit == Some("dirt"));
        }
    }
    eprintln!("dirt triangles {first:?}, grass triangles {second:?} (count, matched)");
    assert!(first.0 > 20 && second.0 > 20, "{first:?} {second:?}");
    assert!(first.1 * 10 >= first.0 * 9, "dirt: {first:?}");
    assert!(second.1 * 10 >= second.0 * 9, "grass: {second:?}");
}

/// de_nuke's pool (its bump map `dev/water_normal`, 29 frames at 21 a
/// second, kept through the map's cubemap patch; its base
/// `dev/water_moving` has one frame, so the base proxy changes nothing)
/// and cs_office's projector slide (`decals/slide`, 7 slides, a new one
/// every 10 s): their meshes bind the first frame of an animation the
/// renderer runs.
#[test]
fn animated_textures_on_stock_maps() {
    for (map, material, base_rate, bump_rate) in [
        ("de_nuke", "maps/de_nuke/de_nuke/nukwater_movingplane_640_-976_-734", None, Some((21.0, 29))),
        ("cs_office", "plaster/slide", Some((0.1, 7)), None),
    ] {
        let Some(data) = load(map) else { continue };
        let mesh = data
            .meshes
            .iter()
            .find(|m| m.material == material)
            .unwrap_or_else(|| panic!("{map}: no {material}"));
        let anim_of = |t: Option<usize>| {
            data.texture_anims
                .iter()
                .find(|a| t.is_some_and(|t| a.frames[0] == t))
        };
        let size = |i: usize| (data.textures[i].width, data.textures[i].height);
        for (what, texture, want, srgb) in [
            ("base", mesh.texture, base_rate, true),
            ("bump", mesh.normal_map, bump_rate, false),
        ] {
            let anim = anim_of(texture);
            let Some((rate, frames)) = want else {
                assert!(anim.is_none(), "{map}: {what} animated");
                continue;
            };
            let anim = anim.unwrap_or_else(|| panic!("{map}: {material}'s {what} isn't animated"));
            assert_eq!((anim.rate, anim.frames.len()), (rate, frames), "{map} {what}");
            assert!(anim.frames.iter().all(|&f| size(f) == size(anim.frames[0])), "{map}");
            assert_eq!(data.textures[anim.frames[0]].srgb, srgb, "{map} {what}");
        }
    }
}

/// A community map's LightmappedGeneric `$color` (bhop_backport_css's
/// `nature/water_dx70`, `{160 187 201}`): the mesh's tint is it raw, bytes
/// / 255 (the world shader multiplies its lighting by it, shaders.md 2);
/// stock world materials set none, and the two-texture-blend walls never
/// take one.
#[test]
fn lightmapped_color_is_raw() {
    if let Some(data) = load("bhop_backport_css") {
        let mesh = data
            .meshes
            .iter()
            .find(|m| m.material == "nature/water_dx70")
            .expect("nature/water_dx70");
        let t = mesh.tint.expect("tinted");
        for (got, byte) in t.iter().zip([160.0, 187.0, 201.0]) {
            assert!((got - byte / 255.0).abs() < 1e-5, "{t:?}");
        }
        assert!(!mesh.unlit);
    }
    if let Some(data) = load("de_aztec") {
        let tinted: Vec<_> = data
            .meshes
            .iter()
            .filter(|m| m.tint.is_some() && !m.unlit && m.entity.is_none() && m.source_vertices.is_empty())
            .map(|m| m.material.as_str())
            .collect();
        assert!(tinted.is_empty(), "{tinted:?}");
    }
}

/// Sprite cards' screen-size limits read from the install's materials
/// (specs/cs_source/impact_effects.md 14): the blood mist's `$maxsize`,
/// the blood smoke's and the fire smoke's fades; commented-out keys
/// (the goop's fades) and UnlitGeneric materials (the smoke grenade's)
/// set none.
#[test]
fn sprite_cards_read_their_screen_size_limits() {
    let Some(data) = load("de_dust2") else { return };
    let screen = |name: &str| {
        data.particles
            .materials
            .iter()
            .find(|m| m.name == name)
            .unwrap_or_else(|| panic!("{name}"))
            .screen
    };
    let core = screen("particle/blood_core");
    assert_eq!((core.max, core.fade_start, core.fade_end), (0.25, 0.0, 0.0));
    let goop = screen("particle/antlion_goop3/antlion_goop3");
    assert_eq!((goop.max, goop.fade_start, goop.fade_end), (0.35, 0.0, 0.0));
    let smoke = screen("particle/smoke1/smoke1_nearcull2");
    assert_eq!((smoke.max, smoke.fade_start, smoke.fade_end), (0.0, 0.075, 0.1));
    let fire = screen("particle/vistasmokev1/vistasmokev1");
    assert_eq!((fire.fade_start, fire.fade_end), (1.0, 1.4));
    assert!(screen("particle/particle_smokegrenade").is_none());
}
