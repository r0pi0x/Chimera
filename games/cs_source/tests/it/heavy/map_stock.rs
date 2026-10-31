//! The stock CS:S maps other than dust2 (which has its own tests), from a
//! real install, headless: they load, and what they draw resolves.
//! Skipped without an install.
//!
//! `cargo test --features dev --test it all_stock_maps_warnings -- --nocapture
//! --ignored` prints every map's load warnings (the catalog
//! in docs/plans/active/other-maps.md comes from it).

use ::cs_source::game as cs_source;
use chimera_world::map::MapData;
use chimera_core::mount::config::LocalConfig;

fn installed() -> bool {
    let ok = LocalConfig::load()
        .ok()
        .and_then(|c| c.game_path_in(cs_source::GAME, cs_source::INFO.steam.as_ref()))
        .is_some_and(|p| chimera_core::mount::served::usable("cs_source", p, "cstrike"));
    if !ok {
        eprintln!("skipping: no CS:S install configured");
    }
    ok
}

fn load(name: &str) -> Option<MapData> {
    installed().then(|| crate::load_map(&format!("cs_source:{name}")).expect(name))
}

/// Every stock map in a CS:S install.
const STOCK: &[&str] = &[
    "cs_assault",
    "cs_compound",
    "cs_havana",
    "cs_italy",
    "cs_militia",
    "cs_office",
    "de_aztec",
    "de_cbble",
    "de_chateau",
    "de_dust",
    "de_dust2",
    "de_inferno",
    "de_nuke",
    "de_piranesi",
    "de_port",
    "de_prodigy",
    "de_tides",
    "de_train",
];

/// Warnings other than the decal/overlay placement counts (tracked in
/// docs/plans/active/other-maps.md) and the AWP view model (MDL v48, not
/// read yet: docs/backlog.md section 3).
fn load_warnings(map: &MapData) -> Vec<&String> {
    map.warnings
        .iter()
        .filter(|w| {
            !w.ends_with("decals found no surface to project onto")
                && !w.ends_with("overlays produced no geometry")
                && !w.starts_with("models/weapons/v_snip_awp.mdl")
        })
        .collect()
}

/// de_aztec's walls (WorldTwoTextureBlend), de_nuke's HDR-capable sky and
/// self-lit decals, static props named `./models/...`, water normal maps:
/// everything these maps name resolves.
#[test]
fn materials_and_models_resolve() {
    for name in ["de_aztec", "cs_office", "de_nuke", "de_train", "de_inferno", "cs_italy"] {
        let Some(map) = load(name) else { return };
        let warnings = load_warnings(&map);
        assert!(warnings.is_empty(), "{name}: {warnings:#?}");
        assert!(map.sky.is_some(), "{name}: no sky");
    }
}

/// Every model a stock map's entities name reads its animations, including
/// HL2 models whose animations sit in `.ani` blocks or sections
/// (specs/cs_source/mdl_v48.md §8); the ones that do are listed.
#[test]
fn entity_models_read_their_animations() {
    if !installed() {
        return;
    }
    let path = LocalConfig::load().unwrap().game_path_in(cs_source::GAME, cs_source::INFO.steam.as_ref()).unwrap();
    let mount = cs_source::mount::open(&path).unwrap();
    let read = |p: &str| mount.read(&p.to_lowercase().replace('\\', "/")).ok();
    let mut models: Vec<String> = Vec::new();
    for name in STOCK {
        let bsp = read(&format!("maps/{name}.bsp")).expect(name);
        let text = String::from_utf8_lossy(&bsp);
        for (at, _) in text.match_indices("\"model\" \"models/") {
            let rest = &text[at + 9..];
            let Some(end) = rest.find('"') else { continue };
            let model = rest[..end].to_lowercase().replace('\\', "/");
            if model.ends_with(".mdl") && !models.contains(&model) {
                models.push(model);
            }
        }
    }
    models.sort();
    let mut external = Vec::new();
    let mut failed = Vec::new();
    for m in &models {
        let Some(bytes) = read(m) else { continue };
        let i32_at = |at: usize| i32::from_le_bytes(bytes[at..at + 4].try_into().unwrap());
        match cs_source::anim::load(&read, m) {
            Ok(set) => {
                if i32_at(352) > 1 || i32_at(4) != 44 {
                    external.push(format!(
                        "{m} (version {}, {} sequences)",
                        i32_at(4),
                        set.sequences.len()
                    ));
                }
            }
            Err(e) => failed.push(format!("{m}: {e}")),
        }
    }
    eprintln!(
        "{} entity models; version 45-48 or with .ani blocks: {external:#?}",
        models.len()
    );
    assert!(failed.is_empty(), "{failed:#?}");
}

/// WorldTwoTextureBlend's walls use the 2x grime mask (detail mode 4,
/// specs/cs_source/shaders_two_texture_blend.md): the stone is the detail.
#[test]
fn aztec_walls_blend_two_textures() {
    let Some(map) = load("de_aztec") else { return };
    let wall = map
        .meshes
        .iter()
        .find(|m| m.material == "de_aztec/stn01grm01_s1")
        .expect("an aztec stone wall");
    assert!(wall.texture.is_some());
    let detail = wall.detail.expect("the stonework as the detail layer");
    assert_eq!(detail.mode, chimera_world::map::DetailMode::TwoTextureMask);
    assert_eq!(detail.scale, [4.0, 4.0]);
    // The canals (Water, no base texture) draw their fog colour and
    // reflect the baked cubemap.
    let water: Vec<_> = map
        .meshes
        .iter()
        .filter(|m| m.material.contains("aztecwater"))
        .collect();
    assert!(!water.is_empty());
    for m in water {
        assert!(m.texture.is_some() && m.envmap.is_some(), "{}", m.material);
    }
}

/// Older maps (BSP v19 like de_aztec and cs_office; early v20 like de_nuke)
/// store one ambient cube per leaf rather than sample lists: props there
/// must still get ambient light, not just the sun.
#[test]
fn old_maps_light_props_with_ambient_cubes() {
    for name in ["de_aztec", "cs_office", "de_nuke", "de_train"] {
        let Some(map) = load(name) else { return };
        let props: Vec<_> = map.props.iter().filter(|p| !p.skybox).collect();
        let lit = props
            .iter()
            .filter(|p| {
                p.lighting
                    .as_ref()
                    .is_some_and(|l| l.cube.iter().any(|c| c.max_element() > 0.0))
            })
            .count();
        assert!(
            lit as f32 > 0.9 * props.len() as f32,
            "{name}: {lit} of {} props have ambient light",
            props.len()
        );
    }
}

/// cs_militia's props (specs/cs_source/shaders.md 2 and 4): the ammo can
/// and the cable spool reflect `env_cubemap` through the normal map's alpha
/// (`$normalmapalphaenvmapmask`), so their meshes carry the normal map the
/// renderer masks with (without it they were mirrors). Its crates by the
/// shed are lit each from one point (the documented reading).
#[test]
fn militia_props_mask_reflections_and_light_from_one_point() {
    use crate::stack::map::EnvmapMask;
    let Some(map) = load("cs_militia") else { return };
    let src = |x: f32, y: f32, z: f32| bevy::math::Vec3::new(x, z, -y) * 0.0254;
    let prop_at = |p: bevy::math::Vec3| {
        map.props
            .iter()
            .min_by(|a, b| a.translation.distance(p).total_cmp(&b.translation.distance(p)))
            .unwrap()
    };
    for (at, material) in [
        (src(170.0, 633.0, 19.2), "ammo_can_01"),
        (src(1786.0, 511.0, -150.2), "spool"),
    ] {
        let prop = prop_at(at);
        let m = &map.models[prop.model].meshes[0];
        assert_eq!(m.material, material);
        let env = m.envmap.expect("reflects");
        assert_eq!(env.mask, EnvmapMask::NormalAlpha, "{material}");
        assert!(m.normal_map.is_some(), "{material}: the mask's normal map");
    }
    // The crates by the shed: no `.vhv` in the map, so each is lit from its
    // centre (docs/tech-debt.md "Prop light probes"): the one against the
    // shed's wall is in its shadow, the one beside it in the sun.
    assert!(map.props.iter().all(|p| p.vertex_light.is_none()));
    let sun = |x: f32, y: f32| {
        let p = prop_at(src(x, y, -161.0));
        assert_eq!(map.models[p.model].meshes[0].material, "crate_extrasmallmil");
        p.lighting.as_ref().expect("probe").lights.iter().any(|(_, c)| c.max_element() > 1.0)
    };
    assert!(!sun(1353.5, 759.8), "by the shed");
    assert!(sun(1402.5, 757.8), "outer");
}

/// Brush entities draw and collide where they stand: cs_office's first
/// window (func_breakable_surf, model *1, Source x -628..-508 at y -344..-340,
/// z -148..-52) is a breakable, so it has its own node: drawn there and
/// solid through its volumes (origin 0 0 0); its sliding door (func_door
/// *7, stored around its origin 584 -1872 -252) is a mover: drawn and solid
/// through its own node, so its meshes and volumes are local to it.
#[test]
fn brush_entities_draw_and_collide() {
    use bevy::math::Vec3;
    let Some(map) = load("cs_office") else { return };
    let src = |x: f32, y: f32, z: f32| Vec3::new(x, z, -y) * 0.0254;
    let at = src(-568.0, -342.0, -100.0);
    let (window, w) = map
        .entities
        .iter()
        .enumerate()
        .find(|(_, e)| e.get("model") == Some("*1"))
        .expect("cs_office's first window");
    assert_eq!(w.classname(), "func_breakable_surf");
    assert!(w.mover, "the window has its own node");
    let drawn = map
        .meshes
        .iter()
        .filter(|m| m.entity == Some(window))
        .flat_map(|m| m.positions.iter())
        .any(|p| Vec3::from(*p).distance(at) < 100.0 * 0.0254);
    assert!(drawn, "window not drawn");
    let (lo, hi) = w
        .hulls
        .iter()
        .flat_map(|h| &h.points)
        .fold((Vec3::MAX, Vec3::MIN), |(lo, hi), q| (lo.min(*q), hi.max(*q)));
    let p = Vec3::new(-568.0, -342.0, -100.0) - w.origin();
    let solid = (lo - Vec3::splat(2.0)).cmple(p).all() && (hi + Vec3::splat(2.0)).cmpge(p).all();
    assert!(solid, "window not solid");

    let (index, door) = map
        .entities
        .iter()
        .enumerate()
        .find(|(_, e)| e.get("model") == Some("*7"))
        .expect("cs_office's door");
    assert_eq!(door.classname(), "func_door");
    assert!(door.mover, "the door moves");
    assert_eq!(door.origin(), Vec3::new(584.0, -1872.0, -252.0));
    assert!(!door.hulls.is_empty(), "door has volumes");
    // Local: around the origin, within the door's size.
    let far = door.hulls.iter().flat_map(|h| &h.points).map(|p| p.length()).fold(0.0, f32::max);
    assert!(far < 200.0, "door volume reaches {far} units from its origin");
    assert!(map.meshes.iter().any(|m| m.entity == Some(index)), "door drawn on its node");
}

/// de_nuke's glass and vents (window contents) are see-through, so a
/// flash's line passes them (grenades.md Q12, measured 2026-10-18): its
/// breakable windows and metal vents (func_breakable_surf *65, func_breakable
/// *45 and *57), not its doors (func_door_rotating *49).
#[test]
fn glass_and_vents_are_see_through() {
    let Some(map) = load("de_nuke") else { return };
    let index = |model: &str| {
        map.entities
            .iter()
            .position(|e| e.get("model") == Some(model))
            .unwrap_or_else(|| panic!("de_nuke's {model}"))
    };
    for model in ["*65", "*45", "*57"] {
        assert!(map.see_through_entities.contains(&index(model)), "{model}");
    }
    assert!(!map.see_through_entities.contains(&index("*49")), "the door");
    assert!(!map.see_through_hulls.is_empty(), "the world's own glass");
    assert!(map.see_through_hulls.len() < map.collision_hulls.len() / 10);
}

/// `$additive` materials (de_nuke's light glows) add to what's behind them
/// instead of drawing their black background.
#[test]
fn additive_materials_add() {
    let Some(map) = load("de_nuke") else { return };
    let additive = |m: &chimera_world::map::MapMesh| m.alpha == chimera_world::map::MapAlpha::Add;
    assert!(map.meshes.iter().any(additive), "no additive world surfaces");
    assert!(
        map.models.iter().any(|m| m.meshes.iter().any(additive)),
        "no additive prop surfaces"
    );
}

/// Water surfaces get the Water shader's look (specs/cs_source/water.md
/// section 10): de_aztec refracts with the cheap cubemap pass and the
/// map's LOD distances; de_port reflects with three-layer normals and the
/// `srgb?` fog colour; both have a bottom material and animated normals.
#[test]
fn water_materials() {
    let Some(aztec) = load("de_aztec") else { return };
    let units = |u: f32| u * cs_source::bsp::METERS_PER_UNIT;
    let top = aztec
        .meshes
        .iter()
        .filter_map(|m| m.water)
        .map(|i| &aztec.water_materials[i])
        .find(|w| w.name.contains("aztecwater"))
        .expect("aztec water");
    assert!(top.refract && !top.reflect && top.above_water && top.envmap.is_some());
    assert!((top.refract_amount - 0.2).abs() < 1e-6);
    assert!((top.cheap_start - units(500.0)).abs() < 1e-4 && (top.cheap_end - units(2000.0)).abs() < 1e-4);
    assert_eq!(top.fog_color, [0.15, 0.1, 0.0]);
    assert_eq!(top.normal_frames.len(), 29);
    assert_eq!(top.frame_rate, 16.0);
    let bottom = top.bottom.as_deref().expect("bottom material");
    assert!(!bottom.above_water && bottom.envmap.is_none());

    let Some(port) = load("de_port") else { return };
    let water = port
        .meshes
        .iter()
        .filter_map(|m| m.water)
        .map(|i| &port.water_materials[i])
        .find(|w| w.name.contains("water_wasteland002b"))
        .expect("port water");
    assert!(water.reflect && water.refract && !water.reflect_entities);
    assert!(water.scroll1.x != 0.0);
    assert!((water.refract_amount - 5.0).abs() < 1e-6);
    let b = |v: f32| v / 255.0;
    assert_eq!(water.fog_color, [b(21.0), b(48.0), b(52.0)]);
    assert!((water.fog_start - units(100.0)).abs() < 1e-4 && (water.fog_end - units(500.0)).abs() < 1e-4);
    let below = water.bottom.as_deref().expect("port bottom material");
    assert!(below.underwater_overlay.is_some() && (below.fog_start - units(-256.0)).abs() < 1e-4);

    // The spec's table (section 10): de_chateau reflects entities, no
    // cubemap pass; cs_militia refracts undistorted.
    let water_of = |map: &MapData, name: &str| {
        map.meshes
            .iter()
            .filter_map(|m| m.water)
            .map(|i| map.water_materials[i].clone())
            .find(|w| w.name.contains(name))
    };
    let Some(chateau) = load("de_chateau") else { return };
    let pretty = water_of(&chateau, "water_pretty1").expect("chateau water");
    assert!(pretty.reflect && pretty.refract && pretty.reflect_entities);
    assert_eq!((pretty.refract_amount, pretty.reflect_amount), (1.0, 0.3));
    assert_eq!(pretty.fog_color, [0.05, 0.05, 0.0]);
    let Some(militia) = load("cs_militia") else { return };
    let m = water_of(&militia, "militiawater").expect("militia water");
    assert!(m.refract && !m.reflect && m.envmap.is_some());
    assert_eq!(m.refract_amount, 0.0);
    assert!((m.cheap_start - units(512.0)).abs() < 1e-4 && (m.cheap_end - units(960.0)).abs() < 1e-4);
    // de_nuke's pool isn't the Water shader: a fog volume only, drawn as
    // the world (spec section 10: {150 200 200}, 64 / 512).
    let Some(nuke) = load("de_nuke") else { return };
    let pool = nuke
        .water_materials
        .iter()
        .find(|w| w.name.contains("nukwater_movingplane") && !w.name.contains("beneath"))
        .expect("nuke pool");
    assert!(pool.fog_only && pool.above_water && !pool.refract && !pool.reflect);
    assert_eq!(pool.fog_color, [b(150.0), b(200.0), b(200.0)]);
    assert!((pool.fog_start - units(64.0)).abs() < 1e-4 && (pool.fog_end - units(512.0)).abs() < 1e-4);
    assert!(
        port.water_materials.iter().chain(&aztec.water_materials).all(|m| !m.fog_only),
        "Water-shader water stays drawn as water"
    );
}

/// Infodecals find their surfaces: displacement terrain too (cs_compound,
/// de_port), large faces whose corners are all far from the decal (most of
/// cs_assault's 45, de_nuke's and de_train's 21 were unplaced before), and
/// brush entities the decal's probe hits (overlays_decals.md). What's left
/// sits by props (dust2's window frames), which the probe ignores.
#[test]
fn decals_find_their_surfaces() {
    for (name, most) in [
        ("cs_assault", 0),
        ("cs_compound", 2),
        ("de_dust", 2),
        ("de_dust2", 5),
        ("de_nuke", 0),
        ("de_port", 0),
        ("de_tides", 3),
        ("de_train", 1),
    ] {
        let Some(map) = load(name) else { return };
        let unplaced: usize = map
            .warnings
            .iter()
            .find_map(|w| w.strip_suffix(" decals found no surface to project onto")?.parse().ok())
            .unwrap_or(0);
        assert!(unplaced <= most, "{name}: {unplaced} decals unplaced");
    }
}

/// Switchable lights that start on (cs_office's projector: style 32, no
/// "starts dark" flag) add their own lightmap style to the faces they light.
#[test]
fn switchable_lights_that_start_on_are_baked_in() {
    use cs_source::lightmap;
    if !installed() {
        return;
    }
    let install = LocalConfig::load().unwrap().game_path_in(cs_source::GAME, cs_source::INFO.steam.as_ref()).unwrap();
    let mount = cs_source::mount::open(&install).unwrap();
    let bytes = mount.read("maps/cs_office.bsp").unwrap();
    let bsp = vbsp::Bsp::read(&bytes).unwrap();
    let lump = lightmap::lighting_lump(&bytes, false);
    let (mut faces, mut brighter) = (0, 0);
    for face in bsp.models().next().unwrap().faces() {
        if !face.styles.contains(&32) {
            continue;
        }
        faces += 1;
        let bumped = face.texture().flags.contains(vbsp::TextureFlags::BUMPLIGHT);
        let total = |s: &lightmap::FaceSamples| s.rgb.iter().map(|c| c[0] + c[1] + c[2]).sum::<f32>();
        let base = lightmap::face_samples(lump, &face).map_or(0.0, |s| total(&s));
        let on = lightmap::face_samples_lit(lump, &face, bumped, &|_| false)
            .0
            .map_or(0.0, |s| total(&s));
        let off = lightmap::face_samples_lit(lump, &face, bumped, &|_| true)
            .0
            .map_or(0.0, |s| total(&s));
        assert!(
            (off - base).abs() <= 1e-3 * base.max(1.0),
            "style 0 alone when the light is off"
        );
        if on > base * 1.01 {
            brighter += 1;
        }
    }
    assert!(faces > 0, "the projector lights some faces");
    assert!(
        brighter * 2 > faces,
        "{brighter} of {faces} faces brighter with the projector on"
    );
}

/// Debug aid: `MAP=cs_office cargo test ... --ignored dark_textures --
/// --nocapture` lists world and prop materials whose base texture is
/// nearly black, or missing.
#[test]
#[ignore = "debug aid"]
fn dark_textures() {
    let Ok(name) = std::env::var("MAP") else { return };
    let Some(map) = load(&name) else { return };
    for m in map.meshes.iter().chain(map.models.iter().flat_map(|m| m.meshes.iter())) {
        let Some(t) = m.texture.map(|i| &map.textures[i]) else {
            if m.alpha != chimera_world::map::MapAlpha::Add {
                println!("{} untextured", m.material);
            }
            continue;
        };
        let n = (t.rgba8.len() / 4).max(1) as f32;
        let mean = t
            .rgba8
            .chunks(4)
            .map(|p| p[0] as f32 + p[1] as f32 + p[2] as f32)
            .sum::<f32>()
            / (3.0 * 255.0 * n);
        if mean < 0.05 {
            println!("{} {} {mean:.3} {}x{}", m.material, t.name, t.width, t.height);
        }
        if let Some(d) = m.detail {
            let t = &map.textures[d.texture];
            let n = (t.rgba8.len() / 4).max(1) as f32;
            let dm = t
                .rgba8
                .chunks(4)
                .map(|p| p[0] as f32 + p[1] as f32 + p[2] as f32)
                .sum::<f32>()
                / (3.0 * 255.0 * n);
            if d.mode == chimera_world::map::DetailMode::Source(0) && dm < 0.3 {
                println!("{} detail {} mod2x mean {dm:.3}", m.material, t.name);
            }
        }
    }
    // Shipped mips much darker than the full-size image (distant surfaces
    // turn black).
    let mean = |px: &[u8]| {
        let n = (px.len() / 4).max(1) as f32;
        px.chunks(4)
            .map(|p| p[0] as f32 + p[1] as f32 + p[2] as f32)
            .sum::<f32>()
            / (3.0 * 255.0 * n)
    };
    for t in &map.textures {
        let full = mean(&t.rgba8);
        for (level, mip) in t.mips.iter().enumerate() {
            let m = mean(mip);
            if (m - full).abs() > 0.25 * full.max(0.05) {
                println!("{} mip {} mean {m:.3} vs {full:.3}", t.name, level + 1);
                break;
            }
        }
    }
}

/// Debug aid: which world mesh a `--views` pixel shows. `MAP=cs_office
/// EYE=x,y,z LOOK=yaw,pitch PX=250,330 cargo test ... --ignored pick --
/// --nocapture` (engine space and angles as in views.json; 1280x720).
#[test]
#[ignore = "debug aid"]
fn pick() {
    use bevy::math::{Quat, Vec3};
    let (Ok(name), Ok(eye), Ok(look), Ok(px)) = (
        std::env::var("MAP"),
        std::env::var("EYE"),
        std::env::var("LOOK"),
        std::env::var("PX"),
    ) else {
        return;
    };
    let Some(map) = load(&name) else { return };
    let nums = |s: &str| s.split(',').map(|v| v.parse::<f32>().unwrap()).collect::<Vec<_>>();
    let (e, l, p) = (nums(&eye), nums(&look), nums(&px));
    let eye = Vec3::new(e[0], e[1], e[2]);
    // 74 degrees vertical (CS:S's 90 at 4:3), 16:9.
    let ty = (73.74f32 / 2.0).to_radians().tan();
    let (x, y) = ((p[0] + 0.5) / 640.0 - 1.0, 1.0 - (p[1] + 0.5) / 360.0);
    let local = Vec3::new(x * ty * 16.0 / 9.0, y * ty, -1.0).normalize();
    let rot = Quat::from_rotation_y(l[0].to_radians()) * Quat::from_rotation_x(l[1].to_radians());
    let dir = rot * local;
    let mut hits = Vec::new();
    for m in &map.meshes {
        for tri in m.indices.chunks(3) {
            let [a, b, c] = [0, 1, 2].map(|k| Vec3::from(m.positions[tri[k] as usize]));
            let (e1, e2) = (b - a, c - a);
            let h = dir.cross(e2);
            let det = e1.dot(h);
            if det.abs() < 1e-9 {
                continue;
            }
            let s = eye - a;
            let u = s.dot(h) / det;
            let q = s.cross(e1);
            let v = dir.dot(q) / det;
            let t = e2.dot(q) / det;
            if u >= 0.0 && v >= 0.0 && u + v <= 1.0 && t > 0.0 {
                hits.push((
                    t,
                    m.material.clone(),
                    m.skybox,
                    m.alpha,
                    m.texture.map(|i| map.textures[i].name.clone()),
                ));
            }
        }
    }
    hits.sort_by(|a, b| a.0.total_cmp(&b.0));
    for h in hits.iter().take(5) {
        println!("{h:?}");
    }
}

/// Debug aid: `MAP=de_aztec AT=-280,-1512,-160 R=600 cargo test ...
/// --ignored props_near -- --nocapture` lists props near a Source point
/// (Source coordinates): their materials, reflections and lighting.
#[test]
#[ignore = "debug aid"]
fn props_near() {
    let (Ok(name), Ok(at)) = (std::env::var("MAP"), std::env::var("AT")) else {
        return;
    };
    let Some(map) = load(&name) else { return };
    let a: Vec<f32> = at.split(',').map(|s| s.parse().unwrap()).collect();
    let p = bevy::math::Vec3::new(a[0], a[2], -a[1]) * 0.0254;
    let r: f32 = std::env::var("R").ok().and_then(|s| s.parse().ok()).unwrap_or(500.0) * 0.0254;
    for prop in map.props.iter().filter(|q| q.translation.distance(p) < r) {
        let m = &map.models[prop.model];
        // Each mesh: material, textured, reflection (cubemap, mask, tint),
        // normal map.
        let mats: Vec<_> = m
            .meshes
            .iter()
            .map(|x| {
                let env = x.envmap.map(|e| (e.cubemap, e.mask, e.tint));
                (x.material.as_str(), x.texture.is_some(), env, x.normal_map.is_some())
            })
            .collect();
        let cube = prop.lighting.as_ref().map(|l| (l.cube, l.lights.len()));
        let source = bevy::math::Vec3::new(prop.translation.x, -prop.translation.z, prop.translation.y) / 0.0254;
        println!(
            "{source:?} {mats:?} light {cube:?} baked per vertex {}",
            prop.vertex_light.is_some()
        );
    }
}

/// de_nuke's cooling towers (static props in the 3D skybox, no baked
/// per-vertex light) take the sun: its ray ends at the skybox's own sky,
/// not at the map far above (they were teal, lit by the sky's blue ambient
/// alone, where CS:S shows them light grey: refcmp `cam16`).
#[test]
fn skybox_props_see_the_sun() {
    let Some(map) = load("de_nuke") else { return };
    let towers: Vec<_> = map
        .props
        .iter()
        .filter(|p| p.skybox && map.models[p.model].meshes.iter().any(|m| m.material.contains("coolingtower")))
        .collect();
    assert_eq!(towers.len(), 2);
    for t in towers {
        let lights = t.lighting.as_ref().map_or(0, |l| l.lights.len());
        assert!(lights >= 1, "a cooling tower without the sun: {:?}", t.lighting);
    }
}

/// Debug aid: `cargo test ... --ignored dark_props -- --nocapture` lists,
/// per stock map (or `MAP`), the props whose baked light is nearly black
/// from every side (a lighting origin in solid, or a missing sample):
/// Source position, first material, brightest side.
#[test]
#[ignore = "debug aid"]
fn dark_props() {
    let names: Vec<String> = match std::env::var("MAP") {
        Ok(m) => vec![m],
        Err(_) => STOCK.iter().map(|s| s.to_string()).collect(),
    };
    for name in names {
        let Some(map) = load(&name) else { return };
        let mut dark = 0;
        for prop in map.props.iter().filter(|p| !p.skybox && p.vertex_light.is_none()) {
            let Some(l) = &prop.lighting else { continue };
            let axes = [bevy::math::Vec3::X, bevy::math::Vec3::NEG_X, bevy::math::Vec3::Y, bevy::math::Vec3::NEG_Y, bevy::math::Vec3::Z, bevy::math::Vec3::NEG_Z];
            let max = axes.iter().map(|n| l.eval(*n).max_element()).fold(0.0, f32::max);
            if max < 0.02 {
                dark += 1;
                let t = prop.translation / 0.0254;
                let mat = map.models[prop.model].meshes.first().map_or("?", |m| m.material.as_str());
                println!("{name} setpos {:.0} {:.0} {:.0}  {mat}  max {max:.4}", t.x, -t.z, t.y);
            }
        }
        println!("== {name}: {dark} dark props of {}", map.props.len());
    }
}

#[test]
#[ignore = "slow: loads every stock map; run by hand for the catalog"]
fn all_stock_maps_warnings() {
    if !installed() {
        return;
    }
    for name in STOCK {
        let map = crate::load_map(&format!("cs_source:{name}")).expect(name);
        println!("== {name}: {} warnings", map.warnings.len());
        for w in &map.warnings {
            println!("  {w}");
        }
    }
}

/// A community map in the user's content cache (never in the repo), when
/// present: its packed materials resolve whatever case the pack stores
/// their names in (`legomg/Shotgun.vmt` for `legomg/shotgun`).
#[test]
fn packed_materials_resolve_case_insensitively() {
    let name = "mg_lego_multigames_v2";
    let present = chimera_core::mount::config::content_dir(cs_source::GAME)
        .is_some_and(|d| d.join("maps").join(format!("{name}.bsp")).is_file());
    if !installed() || !present {
        eprintln!("skipping: {name} not in the content cache");
        return;
    }
    let map = load(name).unwrap();
    let missing: Vec<_> = map.warnings.iter().filter(|w| w.contains("not found")).collect();
    assert!(missing.is_empty(), "{missing:#?}");
}

/// Trigger-like brush entities draw nothing (`bsp::draws_brushes`):
/// cs_militia's two `func_footstep_control` volumes by the Deck ladder and
/// its 3D skybox `env_embers` are `tools/toolstrigger` brushes compiled
/// without the trigger surface flag, and drew as yellow walls. The only
/// tool material CS:S draws is `tools/toolsblack`.
#[test]
fn tool_textured_volumes_are_not_drawn() {
    let Some(map) = load("cs_militia") else { return };
    let tools: Vec<_> = map
        .meshes
        .iter()
        .filter(|m| m.material.starts_with("tools/") && !m.material.starts_with("tools/toolsblack"))
        .map(|m| (m.material.as_str(), m.skybox, m.entity, m.indices.len() / 3))
        .collect();
    assert!(tools.is_empty(), "drawn tool materials: {tools:?}");
    let footsteps = map
        .entities
        .iter()
        .filter(|e| e.classname() == "func_footstep_control")
        .count();
    assert_eq!(footsteps, 2, "cs_militia's footstep volumes still load (their sounds)");
    for class in ["func_footstep_control", "env_embers", "env_bubbles", "trigger_multiple", "func_buyzone"] {
        assert!(!cs_source::bsp::draws_brushes(class), "{class}");
    }
    for class in ["func_wall", "func_illusionary", "func_brush", "func_door", "func_breakable", "func_lod"] {
        assert!(cs_source::bsp::draws_brushes(class), "{class}");
    }
}

/// The seam `map::sky_seam_error` measures for each of the eight ways to
/// lay a sky's cube layer.
fn seam_errors(map: &MapData, layer: usize) -> Vec<f32> {
    let sky = map.sky.as_ref().expect("sky");
    (0..8u8)
        .map(|turns| {
            let mut s = sky.clone();
            s.faces[layer].1 = turns;
            chimera_world::map::sky_seam_error(&s, &map.textures, layer)
        })
        .collect()
}

/// The 2D sky's top face as `refcmp skyconv` fitted it to CS:S joins the
/// (fitted) side faces best of the eight ways to lay it, on every stock
/// sky: the measure `map_community::sky_bottom_joins_the_sides` decides
/// the bottom face's orientation with. (Stock bottoms are one colour.)
#[test]
fn sky_top_joins_the_sides() {
    let up = cs_source::sky::FACES.iter().find(|f| f.0 == "up").unwrap();
    for name in ["de_dust2", "cs_italy", "de_aztec", "cs_office", "de_inferno", "de_nuke", "cs_militia"] {
        let Some(map) = load(name) else { return };
        let errors = seam_errors(&map, up.1);
        let best = (0..8).min_by(|a, b| errors[*a].total_cmp(&errors[*b])).unwrap();
        assert_eq!(best, up.2 as usize, "{name}: {errors:?}");
        assert!(errors[best] * 3.0 < errors.iter().filter(|e| **e != errors[best]).fold(f32::MAX, |a, e| a.min(*e)));
    }
}
