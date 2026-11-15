//! The shaders' looks on the disc's multiplayer maps (`game::maps::
//! materials`, shaders.md): which shader classes and features occur (a
//! survey printed by `the_shader_features_on_the_multiplayer_maps`), and
//! that each class maps to the engine's look fields the module doc says.

use std::collections::{BTreeMap, BTreeSet};

use blam_formats::{
    cache::{MapFile, class},
    shader::{PeriodicFunction, Shader},
};
use halo_ce::game::{MULTIPLAYER_MAPS, maps};

use super::maps::mount;

/// Every multiplayer map's file.
fn files() -> Option<Vec<(&'static str, MapFile)>> {
    let mount = mount()?;
    Some(
        MULTIPLAYER_MAPS
            .iter()
            .map(|name| {
                let bytes = mount.read(&maps::map_path(name)).unwrap();
                (*name, MapFile::load(&bytes).unwrap())
            })
            .collect(),
    )
}

fn animated(f: PeriodicFunction, scale: f32) -> bool {
    !matches!(f, PeriodicFunction::One | PeriodicFunction::Zero) && scale != 0.0
}

/// A shader's features, as short keys.
fn features(s: &Shader) -> Vec<String> {
    let mut out = vec![format!("class {}", s.class())];
    let some = |r: &blam_formats::cache::TagRef| !r.id.is_none();
    match s {
        Shader::Environment(e) => {
            out.push(format!("senv type {:?}", e.environment_type));
            if some(&e.self_illumination_map) {
                out.push("senv self-illumination map".into());
            }
            if some(&e.reflection_cube_map) && (e.perpendicular_brightness > 0.0 || e.parallel_brightness > 0.0) {
                out.push(format!("senv reflection {:?}", e.reflection_type));
            }
            if some(&e.bump_map) {
                out.push("senv bump map".into());
            }
            if some(&e.secondary_detail_map) {
                out.push("senv secondary detail".into());
            }
            if some(&e.micro_detail_map) {
                out.push(format!("senv micro detail {:?}", e.micro_detail_function));
            }
            if some(&e.primary_detail_map) {
                out.push(format!("senv detail {:?}", e.detail_function));
            }
            if animated(e.u_animation.function, e.u_animation.scale)
                || animated(e.v_animation.function, e.v_animation.scale)
            {
                out.push(format!(
                    "senv uv animation {:?}/{:?}",
                    e.u_animation.function, e.v_animation.function
                ));
            }
            if e.specular_flags & 4 != 0 && e.specular_brightness > 0.0 {
                out.push("senv specular lightmap".into());
            }
            if e.flags & 2 != 0 {
                out.push("senv bump is specular mask".into());
            }
        }
        Shader::Model(m) => {
            if some(&m.multipurpose_map) {
                out.push("soso multipurpose map".into());
            }
            if some(&m.reflection_cube_map) && (m.perpendicular_brightness > 0.0 || m.parallel_brightness > 0.0) {
                out.push("soso reflection".into());
            }
            if m.self_illumination_color_upper != m.self_illumination_color_lower
                || [
                    m.self_illumination_color_upper.r,
                    m.self_illumination_color_upper.g,
                    m.self_illumination_color_upper.b,
                ] != [0.0; 3]
            {
                out.push(format!("soso self-illumination {:?}", m.self_illumination_function));
            }
            out.push(format!("soso change colour source {:?}", m.change_color_source));
            if some(&m.detail_map) {
                out.push(format!("soso detail {:?} mask {:?}", m.detail_function, m.detail_mask));
            }
            if m.translucency > 0.0 {
                out.push("soso translucency".into());
            }
            let a = &m.texture_animation;
            if animated(a.u.function, a.u.scale)
                || animated(a.v.function, a.v.scale)
                || animated(a.rotation.function, a.rotation.scale)
            {
                out.push(format!(
                    "soso uv animation {:?}/{:?}/{:?}",
                    a.u.function, a.v.function, a.rotation.function
                ));
            }
        }
        Shader::TransparentChicago(c) => {
            let fns: Vec<String> = c
                .maps
                .iter()
                .take(c.maps.len().saturating_sub(1))
                .map(|m| format!("{:?}/{:?}", m.color_function, m.alpha_function))
                .collect();
            out.push(format!(
                "schi {} maps {:?} first {:?} fade {:?} [{}]",
                c.maps.len(),
                c.common.framebuffer_blend_function,
                c.common.first_map_type,
                c.common.framebuffer_fade_mode,
                fns.join(" ")
            ));
            for m in &c.maps {
                let a = &m.animation;
                if animated(a.u.function, a.u.scale)
                    || animated(a.v.function, a.v.scale)
                    || animated(a.rotation.function, a.rotation.scale)
                {
                    out.push(format!(
                        "schi map animation {:?}/{:?}/{:?}",
                        a.u.function, a.v.function, a.rotation.function
                    ));
                }
            }
        }
        Shader::TransparentGeneric(g) => {
            out.push(format!(
                "sotr {} maps {} stages {:?} first {:?} fade {:?}",
                g.maps.len(),
                g.stages.len(),
                g.common.framebuffer_blend_function,
                g.common.first_map_type,
                g.common.framebuffer_fade_mode
            ));
            for m in &g.maps {
                let a = &m.animation;
                if animated(a.u.function, a.u.scale)
                    || animated(a.v.function, a.v.scale)
                    || animated(a.rotation.function, a.rotation.scale)
                {
                    out.push(format!(
                        "sotr map animation {:?}/{:?}/{:?}",
                        a.u.function, a.v.function, a.rotation.function
                    ));
                }
            }
        }
        _ => {}
    }
    out
}

/// A survey of the shader features on the 13 maps (each shader tag once,
/// by path): printed, for the module doc's counts.
#[test]
fn the_shader_features_on_the_multiplayer_maps() {
    let Some(files) = files() else { return };
    let mut seen = BTreeSet::new();
    let mut counts: BTreeMap<String, usize> = BTreeMap::new();
    for (name, file) in &files {
        let mut per_map: BTreeMap<String, usize> = BTreeMap::new();
        for tag in file.of_kind(class::SHADER) {
            let Ok(s) = Shader::read(file, tag) else { continue };
            *per_map.entry(format!("{}", s.class())).or_default() += 1;
            if !seen.insert(tag.path.clone()) {
                continue;
            }
            for f in features(&s) {
                *counts.entry(f).or_default() += 1;
            }
        }
        let bsp = blam_formats::structure_bsp::StructureBsp::read(file, 0).unwrap();
        let outdoor = bsp.clusters.iter().filter(|c| c.sky.is_some()).count();
        eprintln!(
            "{name}: {per_map:?}; clusters {} ({outdoor} show a sky), fog planes {}, fog regions {}, lens flares {} at {} markers",
            bsp.clusters.len(),
            bsp.fog_planes.len(),
            bsp.fog_regions.len(),
            bsp.lens_flares.len(),
            bsp.lens_flare_markers.len()
        );
    }
    eprintln!("{} distinct shaders", seen.len());
    for (k, n) in &counts {
        eprintln!("{n:5}  {k}");
    }
    assert!(seen.len() > 100);
}

/// Each map's meshes with the looks' features (`materials` module doc):
/// reflections on cube maps the map holds, self-illumination, scrolls and
/// details; printed per map, and on the maps whose shaders carry them.
#[test]
fn the_looks_draw_the_shaders_features_on_every_map() {
    let Some(mount) = mount() else { return };
    let mut totals = [0usize; 5];
    for &name in halo_ce::game::MULTIPLAYER_MAPS {
        let map = super::maps::load(&mount, name, true);
        let meshes: Vec<&chimera_world::map::MapMesh> = map
            .meshes
            .iter()
            .chain(map.models.iter().flat_map(|m| &m.meshes))
            .collect();
        let count = |f: &dyn Fn(&chimera_world::map::MapMesh) -> bool| meshes.iter().filter(|m| f(m)).count();
        let reflect = count(&|m| m.envmap.is_some());
        let lit_mask = count(&|m| m.envmap.is_some_and(|e| e.light_mask.is_some()));
        let glow = count(&|m| m.selfillum.is_some());
        let scroll = count(&|m| m.base_transform.scroll != [0.0, 0.0]);
        let detail = count(&|m| m.detail.is_some());
        eprintln!(
            "{name}: {} meshes, {} cube maps; reflect {reflect} (lightmap-masked {lit_mask}), self-illuminated {glow}, scrolling {scroll}, detailed {detail}",
            meshes.len(),
            map.cubemaps.len()
        );
        for m in &meshes {
            if let Some(e) = m.envmap {
                let c = &map.cubemaps[e.cubemap.expect("a reflection names its cube map")];
                assert!(c.size > 0 && c.faces.iter().all(|f| !f.is_empty()), "{}", c.name);
                assert!(
                    matches!(e.mask, chimera_world::map::EnvmapMask::Texture(t) if t < map.textures.len())
                        || e.mask == chimera_world::map::EnvmapMask::None
                );
            }
            if let Some(s) = m.selfillum {
                assert!(s.tint.iter().any(|c| *c > 0.0), "{}", m.material);
            }
        }
        for (k, n) in [reflect, glow, scroll, detail, map.cubemaps.len()]
            .into_iter()
            .enumerate()
        {
            totals[k] += n;
        }
        if name == "hangemhigh" || name == "damnation" || name == "chillout" {
            assert!(reflect > 0 && lit_mask > 0, "{name}: level reflections");
        }
        if name == "damnation" || name == "sidewinder" || name == "prisoner" {
            assert!(glow > 0, "{name}: self-illumination");
        }
    }
    eprintln!(
        "totals: reflect {}, self-illuminated {}, scrolling {}, detailed {}, cube maps {}",
        totals[0], totals[1], totals[2], totals[3], totals[4]
    );
    assert!(totals[2] > 0, "slides scroll somewhere");
}

/// The Spartan's armour takes the player's colour where its multipurpose
/// map's alpha says (`MapMesh::tint_mask`; shaders.md, "Multipurpose map
/// channels on the Xbox"): the armour meshes carry a mask. (On the disc
/// the armour's multipurpose alpha is 1 throughout: the whole armour
/// base takes the colour.)
#[test]
fn the_spartans_armour_colour_is_masked() {
    let Some(files) = files() else { return };
    let (_, file) = files.iter().find(|(n, _)| *n == "bloodgulch").unwrap();
    let spartan = halo_ce::game::body::load::spartan(file, true).unwrap();
    let masked: Vec<_> = spartan
        .character
        .model
        .meshes
        .iter()
        .filter_map(|m| m.tint_mask.map(|t| (m, t)))
        .collect();
    assert!(!masked.is_empty(), "armour meshes carry the change-colour mask");
    for (m, t) in masked {
        let tex = &spartan.textures[t];
        let texels = tex.rgba8.len() / 4;
        let mean = tex.rgba8.chunks_exact(4).map(|p| p[0] as f32 / 255.0).sum::<f32>() / texels as f32;
        eprintln!("{}: mask {} mean {mean:.3}", m.material, tex.name);
        assert!(mean > 0.02, "{}: {mean}", m.material);
    }
}
