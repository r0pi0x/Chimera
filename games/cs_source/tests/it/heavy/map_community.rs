//! Community CS:S maps from the user's content cache (never in the repo),
//! headless: the problems the map sweep found (`mapsweep`,
//! docs/plans/active/community-maps.md) stay fixed. Each test skips when
//! its map isn't in the cache or there is no install.

use bevy::prelude::*;
use crate::stack::{
    games::{
        self, cs_source,
        cs_source::{movement::SourceMovementPlugin, weapons::CsWeaponsPlugin},
    },
    harness::{CsSim, Sim},
    map::{MapData, MapPlugin},
    mount::config::LocalConfig,
    movement::placeholder,
    weapon::{
        Inventory, Weapon,
        drop::Loose,
        equip::{MapWeapon, SpawnEquipment},
    },
};

/// The map, when the install and the cached map are there.
fn load(name: &str) -> Option<MapData> {
    let installed = LocalConfig::load()
        .ok()
        .and_then(|c| c.game_path_in(cs_source::GAME, cs_source::INFO.steam.as_ref()))
        .is_some_and(|p| chimera_core::mount::served::usable("cs_source", p, "cstrike"));
    let cached = ::cs_source::game::INFO.mount().is_some_and(|m| m.contains(&format!("maps/{name}.bsp")));
    if !installed || !cached {
        eprintln!("skipping: {name} not in the content cache (or no install)");
        return None;
    }
    Some(games::load_map(&format!("cs_source:{name}")).unwrap_or_else(|e| panic!("{name}: {e}")))
}

/// Maps that failed to load: Latin-1 text in the entity lump
/// (gg_bk_warehouse_v1, mg_wipeout2), models whose animations sit in
/// `.ani` blocks (mg_kommando: vmdl panicked), a model whose `.vtx`
/// indexes past its `.vvd` (surf_surreal: vmdl panicked).
#[test]
fn maps_that_failed_load() {
    for name in ["gg_bk_warehouse_v1", "mg_wipeout2", "mg_kommando", "surf_surreal"] {
        let Some(map) = load(name) else { continue };
        assert!(!map.meshes.is_empty(), "{name}: no world");
        assert!(!map.spawns.is_empty(), "{name}: no spawns");
    }
}

/// Models whose fixed 64-byte names aren't terminated or aren't UTF-8
/// load (`props::terminate_names`): kz_hikari_od_nh_v2's bamboo,
/// mg_kommando's sandbags, mg_creative_multigames_v8_ns's car,
/// surf_surreal's butterflies.
#[test]
fn models_with_odd_names_load() {
    for name in ["kz_hikari_od_nh_v2", "mg_kommando", "mg_creative_multigames_v8_ns", "surf_surreal"] {
        let Some(map) = load(name) else { continue };
        let bad: Vec<_> = map
            .warnings
            .iter()
            .filter(|w| w.contains("not null-terminated") || w.contains("invalid utf-8"))
            .collect();
        assert!(bad.is_empty(), "{name}: {bad:#?}");
    }
}

/// MP3 sounds packed in the map play (mg_lego_multigames_v2's music).
#[test]
fn packed_mp3_sounds_decode() {
    let Some(map) = load("mg_lego_multigames_v2") else { return };
    let (entries, raw) = cs_source::sound::ambient_messages(&map.entities);
    let mp3: Vec<String> = entries.into_iter().chain(raw).filter(|s| s.ends_with(".mp3")).collect();
    assert!(!mp3.is_empty(), "the map's music");
    for s in &mp3 {
        let entry = map.sounds.entry(s).unwrap_or_else(|| panic!("{s}: no entry"));
        let clip = &map.sounds.clips[entry.waves[0]];
        assert!(clip.rate > 0 && clip.samples.len() > 1000, "{s}: decoded");
    }
}

/// A renamed map (bhop_backport_css, packed as kz_bhop_backport_css)
/// finds its baked cubemaps under the original name.
#[test]
fn renamed_map_finds_its_cubemaps() {
    let Some(map) = load("bhop_backport_css") else { return };
    let missing: Vec<_> = map.warnings.iter().filter(|w| w.contains("/c") && w.contains("not found")).collect();
    assert!(missing.is_empty(), "{missing:#?}");
    assert!(!map.cubemap_samples.is_empty());
}

/// Textures in uncompressed formats the vtf crate doesn't decode.
#[test]
fn uncommon_texture_formats_decode() {
    for name in ["gg_simpsons_bam", "mg_n64_goldeneye_v2", "mg_creative_multigames_v8_ns"] {
        let Some(map) = load(name) else { continue };
        let bad: Vec<_> = map.warnings.iter().filter(|w| w.contains("images is not supported")).collect();
        assert!(bad.is_empty(), "{name}: {bad:#?}");
    }
}

/// mg_item_battle_v4b: its game_player_equips (no "Use Only") give a P228
/// instead of the starting weapons, and its placed weapons lie where the
/// map puts them, put back at a round restart.
#[test]
fn spawn_equipment_and_placed_weapons() {
    let Some(map) = load("mg_item_battle_v4b") else { return };
    let placed = map
        .entities
        .iter()
        .filter(|e| e.classname().starts_with("weapon_"))
        .count();
    let mut sim = Sim::new(CsSim, (MapPlugin::new(map), SourceMovementPlugin, CsWeaponsPlugin));
    sim.set_tick_interval(cs_source::TICK_INTERVAL);
    sim.ticks(2);
    let equipment = sim.app.world().resource::<SpawnEquipment>().0.clone();
    assert_eq!(equipment, Some(vec![("weapon_p228".to_string(), 1), ("weapon_p228".to_string(), 1)]));

    let p = sim.spawn_character(Vec3::new(0.0, 50.0, 0.0), placeholder::ID);
    sim.ticks(2);
    let held: Vec<&str> = {
        let world = sim.app.world();
        let inv = world.get::<Inventory>(p).expect("an inventory");
        inv.weapons.iter().filter_map(|w| world.get::<Weapon>(*w)).map(|w| w.id).collect()
    };
    assert_eq!(held, vec!["cs_source:weapon_p228"], "one P228 only");

    let lying = |sim: &mut Sim| {
        let world = sim.app.world_mut();
        world.query::<(&Loose, &MapWeapon)>().iter(world).count()
    };
    assert_eq!(lying(&mut sim), placed, "every placed weapon lies there");
    crate::stack::core::restart_round(sim.app.world_mut());
    sim.ticks(2);
    assert_eq!(lying(&mut sim), placed, "put back, not doubled");
}

/// The map, when the install and the cached map are both there.
fn cached(name: &str) -> Option<MapData> {
    let installed = LocalConfig::load()
        .ok()
        .and_then(|c| c.game_path_in(cs_source::GAME, cs_source::INFO.steam.as_ref()))
        .is_some_and(|p| chimera_core::mount::served::usable("cs_source", p, "cstrike"));
    let present = ::cs_source::game::INFO.mount().is_some_and(|m| m.contains(&format!("maps/{name}.bsp")));
    if !installed || !present {
        eprintln!("skipping: {name} not in the content cache");
        return None;
    }
    Some(games::load_map(&format!("cs_source:{name}")).expect(name))
}

/// surf_boreas is repacked with LZMA-compressed lumps and its version-10
/// static prop lump sets the NO_DRAW bit on 670 of its 1587 props, its
/// surf ramps among them (`models/project_tendies/ramps/*`, 100 m long
/// models). The lighting must decode (not compressed bytes read as
/// luxels) and every static prop must be placed.
#[test]
fn surf_boreas_lighting_and_ramps() {
    let Some(map) = cached("surf_boreas") else { return };
    // Lightmaps: compressed bytes read as luxels gave random exponents.
    let lm = map.lightmap.as_ref().expect("lightmap");
    let wild = lm.rgb.iter().filter(|c| c.iter().any(|v| !v.is_finite() || *v > 16.0)).count();
    assert!(
        wild * 1000 < lm.rgb.len(),
        "{wild} of {} luxels out of range: lighting read from compressed bytes?",
        lm.rgb.len()
    );
    // Cubemap names come from the (compressed) cubemap lump.
    let bad: Vec<_> = map.warnings.iter().filter(|w| w.contains("materials/maps/surf_boreas/c")).collect();
    assert!(bad.is_empty(), "{bad:#?}");
    // Static props: all of them, the ramp models included.
    let statics = map.props.iter().filter(|p| p.entity.is_none()).count();
    assert!(statics >= 1587, "{statics} static props placed");
    let ramps = map
        .props
        .iter()
        .filter(|p| {
            let (lo, hi) = map.models[p.model].bounds;
            (hi - lo).max_element() > 90.0
        })
        .count();
    assert!(ramps >= 19, "{ramps} props over 90 m (the ramps)");

    // A snow cave whose displacement folds back over itself: from inside
    // it (the "transition" soundscape, Source 10446 8619 9860), the floor
    // ahead must face the eye. Re-winding each triangle to the base face's
    // normal turned the folded ones inside out (culled: holes to the sky).
    let engine = |x: f32, y: f32, z: f32| Vec3::new(x, z, -y) * 0.0254;
    let eye = engine(10446.0, 8619.0, 9860.0);
    for dir in [Vec3::new(-0.526, -0.728, -0.44), Vec3::new(-0.876, 0.163, -0.454)] {
        let dir = Vec3::new(dir.x, dir.z, -dir.y).normalize();
        let hit = first_hit(&map, eye, dir).expect("the cave floor is drawn there");
        assert!(hit.1.dot(dir) < 0.0, "the nearest drawn triangle faces away from the eye: {hit:?}");
        assert!((7.0..13.0).contains(&hit.0), "the floor 8-12 m away, not further: {hit:?}");
    }
}

/// The nearest world triangle on a ray (distance, its front normal by
/// winding: counter-clockwise faces the viewer), skybox meshes left out.
fn first_hit(map: &MapData, o: Vec3, d: Vec3) -> Option<(f32, Vec3)> {
    let mut best: Option<(f32, Vec3)> = None;
    for m in map.meshes.iter().filter(|m| !m.skybox) {
        for t in m.indices.chunks_exact(3) {
            let [a, b, c] = [0, 1, 2].map(|k| Vec3::from_array(m.positions[t[k] as usize]));
            let (e1, e2) = (b - a, c - a);
            let h = d.cross(e2);
            let det = e1.dot(h);
            if det.abs() < 1e-9 {
                continue;
            }
            let s = o - a;
            let u = s.dot(h) / det;
            let q = s.cross(e1);
            let v = d.dot(q) / det;
            let t = e2.dot(q) / det;
            if u >= 0.0 && v >= 0.0 && u + v <= 1.0 && t > 0.0 && best.is_none_or(|(bt, _)| t < bt) {
                best = Some((t, e1.cross(e2).normalize()));
            }
        }
    }
    best
}

/// Static props compiled with per-vertex lighting (`sp_<n>.vhv` in the
/// pakfile) take it instead of the light probe, and it agrees with the
/// probe in colour (same lighting, same units, same channel order).
#[test]
fn baked_prop_vertex_light() {
    for name in ["mg_lt_galaxy_v5", "surf_nebula", "kz_ancient_ruins", "surf_demise"] {
        let Some(map) = load(name) else { continue };
        // (Detail models aren't static props: no baked light of their own.)
        let statics: Vec<_> = map.props.iter().filter(|p| p.entity.is_none() && !p.detail).collect();
        let baked: Vec<_> = statics.iter().filter(|p| p.vertex_light.is_some()).collect();
        // Per prop: the mean baked light over the mean probe light, per
        // channel; the median of each.
        let mut ratios: [Vec<f32>; 3] = Default::default();
        for p in &baked {
            let v = p.vertex_light.as_ref().unwrap();
            let (mut sum_v, mut sum_p) = (Vec3::ZERO, Vec3::ZERO);
            for m in &map.models[p.model].meshes {
                for (i, n) in m.normals.iter().enumerate() {
                    sum_v += Vec3::from(v[m.source_vertices[i] as usize]);
                    sum_p += p.lighting.as_ref().unwrap().eval(p.rotation * Vec3::from(*n));
                }
            }
            if sum_p.min_element() > 1e-3 {
                for c in 0..3 {
                    ratios[c].push(sum_v[c] / sum_p[c]);
                }
            }
        }
        let median = |v: &mut Vec<f32>| {
            v.sort_by(f32::total_cmp);
            v.get(v.len() / 2).copied().unwrap_or(0.0)
        };
        let m = ratios.each_mut().map(median);
        let unfit: Vec<_> = map.warnings.iter().filter(|w| w.contains("per-vertex")).collect();
        eprintln!(
            "{name}: {} of {} static props baked; median baked/probe light per channel {m:?}; {unfit:?}",
            baked.len(),
            statics.len(),
        );
        assert!(baked.len() * 10 >= statics.len() * 9, "{name}: most static props baked");
        assert!(unfit.is_empty(), "{name}: {unfit:?}");
        // Same hue as the probe (the files' B, G, R order), somewhat
        // darker (self-shadowing; docs/backlog.md's open question).
        let (lo, hi) = (m.iter().copied().fold(f32::MAX, f32::min), m.iter().copied().fold(0.0, f32::max));
        assert!(hi < 1.3 * lo, "{name}: channels {m:?}");
        assert!((0.3..1.5).contains(&lo), "{name}: brightness {m:?}");
    }
}

/// Animated light styles (1-31) keep their faces' share apart with their
/// pattern: bhop_myztek's flickering (style 1) and second flicker (6)
/// lights, mg_jacks_multigames_v1's candle (3) and fluorescent flicker
/// (10) next to its switchable styles.
#[test]
fn animated_light_styles_keep_their_pattern() {
    for (name, want) in [("bhop_myztek", vec![1u8, 6]), ("mg_jacks_multigames_v1", vec![1, 3, 6, 10])] {
        let Some(map) = load(name) else { continue };
        let l = map.lightmap.as_ref().expect("lightmap");
        let animated: Vec<u8> = l.styles.iter().filter(|s| !s.pattern.is_empty()).map(|s| s.style).collect();
        for style in &want {
            let s = l.styles.iter().find(|s| s.style == *style).unwrap_or_else(|| panic!("{name}: style {style}"));
            assert_eq!(s.pattern, cs_source::lightmap::style_pattern(*style, None), "{name}: style {style}");
            assert!(!s.rects.is_empty() && s.rgb.iter().any(|c| c[0] > 0.01), "{name}: style {style} lights something");
            let texels: u32 = s.rects.iter().map(|r| r[2] * r[3]).sum();
            assert_eq!(texels as usize, s.texels.len());
        }
        assert!(animated.iter().all(|s| *s < 32), "{animated:?}");
        if name == "mg_jacks_multigames_v1" {
            assert!(l.styles.iter().any(|s| s.style >= 32 && s.pattern.is_empty()), "switchable styles too");
        }
    }
}

/// Materials the VMT parser refused (the map sweep: 32 over 10 maps) read
/// the game's way: `$detailscale "[9 9 9]"`, WorldVertexTransition without
/// `$basetexture2`, an unreadable `$basetexturetransform`, a missing
/// closing brace; unknown shaders as their stand-ins (WindowImposter,
/// ShatteredGlass, `Refract_DX90`).
#[test]
fn lenient_materials() {
    let refused = [
        "Vec2OrSingle",
        "$basetexture2",
        "$basetexturetransform",
        "No valid token",
        "\"windowimposter\"",
        "\"shatteredglass\"",
        "\"lightmappedreflective\"",
        "\"refract_dx90\"",
        "Vec3OrSingle",
        "duplicate field",
        "missing field `$basetexture`",
    ];
    for name in [
        "kz_11342",
        "kz_hikari_od_nh_v2",
        "mg_kommando",
        "mg_escape_prison_beta",
        "surf_halloween_tf2",
        "surf_jive",
        "surf_threnody",
    ] {
        let Some(map) = load(name) else { continue };
        let bad: Vec<_> = map
            .warnings
            .iter()
            .filter(|w| refused.iter().any(|r| w.contains(r)))
            .collect();
        assert!(bad.is_empty(), "{name}: {bad:#?}");
        if name == "surf_threnody" {
            // Its fake skies show their cubemap.
            assert!(
                map.meshes
                    .iter()
                    .any(|m| m.material.contains("fakeskies") && m.envmap.is_some_and(|e| e.imposter)),
                "a WindowImposter surface"
            );
        }
    }
}

/// mg_swag_multigames_v1 places about 250 weapons. Every resting one swept
/// itself (swept CCD) against all it touched each tick, the world's
/// colliders included: 19 ms a tick, so frames took 340 ms (ticks piled
/// up). Resting items sweep nothing now.
#[test]
fn resting_placed_weapons_cost_no_sweeps() {
    let Some(map) = load("mg_swag_multigames_v1") else { return };
    let mut sim = Sim::new(CsSim, (MapPlugin::new(map), SourceMovementPlugin, CsWeaponsPlugin));
    sim.set_tick_interval(cs_source::TICK_INTERVAL);
    // Let them land and settle.
    sim.ticks(200);
    let placed = {
        let world = sim.app.world_mut();
        world.query::<(&Loose, &MapWeapon)>().iter(world).count()
    };
    assert!(placed > 200, "{placed} placed weapons");
    let mut worst = std::time::Duration::ZERO;
    type Diagnostics = avian3d::dynamics::solver::SolverDiagnostics;
    for _ in 0..20 {
        *sim.app.world_mut().resource_mut::<Diagnostics>() = Diagnostics::default();
        sim.ticks(1);
        worst = worst.max(sim.app.world().resource::<Diagnostics>().swept_ccd);
    }
    let moving = {
        let world = sim.app.world_mut();
        world
            .query_filtered::<&avian3d::prelude::LinearVelocity, With<MapWeapon>>()
            .iter(world)
            .filter(|v| v.length() > 0.5)
            .count()
    };
    eprintln!("swept CCD at most {worst:?} a tick; {moving} of {placed} weapons moving");
    // 55 ms before, 0.1-3.5 ms after (machine load).
    assert!(worst.as_secs_f32() < 0.02, "swept CCD took {worst:?} in a tick");
}

/// surf_demise: its ramps are translucent marble over an envmap-only
/// material (no `$basetexture`) reflecting a tinted HDR sky cubemap: black
/// albedo (specs/cs_source/shaders.md 2), not the grey stand-in (which
/// read as a magenta floor at the spawn). The cubemap is a half-float VTF
/// in an LZMA-compressed pak entry the zip reader's decoder refused.
#[test]
fn envmap_only_ramps_reflect_their_sky() {
    let Some(map) = load("surf_demise") else { return };
    let bad: Vec<_> = map.warnings.iter().filter(|w| w.contains("sky_demise_05")).collect();
    assert!(bad.is_empty(), "{bad:#?}");
    let sky: Vec<_> = map
        .models
        .iter()
        .flat_map(|m| &m.meshes)
        .filter(|m| m.material.contains("sky_demise_05"))
        .collect();
    assert!(!sky.is_empty(), "the ramps' fake-sky meshes");
    for m in sky {
        let texture = &map.textures[m.texture.expect("a black base texture")];
        assert_eq!(&texture.rgba8[..3], &[0, 0, 0], "{}: black albedo", m.material);
        assert!(m.envmap.is_some_and(|e| e.cubemap.is_some()), "{}: its sky cubemap", m.material);
    }
}

/// Material effects community maps use (specs/cs_source/shaders.md 2, 4,
/// 6 and open questions 16 and 18): detail blend modes past 0 and 1,
/// `$selfillum`, `$basetexturetransform` and its TextureScroll proxy, sky
/// faces' half-height transform.
#[test]
fn material_effects() {
    use crate::stack::map::{DetailMode, MapMesh};
    fn meshes(map: &MapData) -> impl Iterator<Item = &MapMesh> {
        map.meshes.iter().chain(map.models.iter().flat_map(|m| &m.meshes))
    }
    if let Some(map) = load("kz_ancient_ruins") {
        // A temple prop's moss: detail mode 2 (model shader: decoded).
        let moss = meshes(&map)
            .find(|m| m.material.contains("doorways_moss"))
            .expect("the doorway moss");
        let d = moss.detail.expect("its detail");
        assert_eq!(d.mode, DetailMode::Source(2));
        assert!(map.textures[d.texture].srgb, "model detail other than mod2x is sRGB-decoded");
        // Self-lit props (base alpha masks).
        assert!(meshes(&map).any(|m| m.selfillum.is_some()), "a self-illuminated surface");
        // A scrolling texture (TextureScroll on $basetexturetransform).
        assert!(
            meshes(&map).any(|m| m.base_transform.scroll != [0.0, 0.0]),
            "a scrolling base texture"
        );
    }
    // prop_ragdoll (physics_brushes.md 6) is drawn and simulated: without
    // Hammer's pose it starts from its first sequence at the entity's
    // place and angles (`map_ragdolls_fall_and_take_shots` drops it).
    if let Some(map) = load("gg_deagle7k") {
        let ragdoll = map.entities.iter().position(|e| e.classname() == "prop_ragdoll");
        let prop = map
            .props
            .iter()
            .find(|p| p.entity.is_some() && p.entity == ragdoll)
            .expect("drawn");
        assert!(map.models[prop.model].rig.is_some() && prop.ragdoll.is_some());
        let placed = cs_source::movement::to_engine(Vec3::new(-22.0, 1061.0, 29.0));
        assert!(prop.translation.distance(placed) < 1e-3, "at its origin: {}", prop.translation);
    }
    if let Some(map) = load("surf_demise") {
        if let Some(bone) = meshes(&map).find(|m| m.material.contains("bonecolor")) {
            assert_eq!(bone.detail.map(|d| d.mode), Some(DetailMode::Source(8)));
        }
    }
    if let Some(map) = load("bhop_flatzone") {
        // Half-height sky faces: "center 0 0 scale 1 2" on the sides.
        let sky = map.sky.as_ref().expect("a sky");
        assert!(sky.transforms.iter().any(|t| !t.is_identity()), "sky face transforms");
    }
}

/// mg_item_battle_v4b's items: placed knives with entities parented to
/// them (a car, a jetpack...). Walking onto one picks it up, its map
/// entity fires OnPlayerPickup with the player as activator, and what is
/// parented to it follows the player: props ride the knife's anchor node
/// (at the middle of the player's box, 31 units above the feet standing,
/// turned to its yaw), keeping their placement. CS:S's captures of the car
/// knife (first and third person) put the car there: on the floor, roof
/// at the carrier's shoulders; at the feet it sank to the carrier's knees.
#[test]
fn item_children_follow_the_player() {
    use crate::stack::map::{
        PropEntity,
        entities::{MapAnchor, anchor_entities, parent_name},
    };
    let Some(map) = load("mg_item_battle_v4b") else { return };
    let anchors = anchor_entities(&map.entities);
    let items: Vec<usize> = (0..map.entities.len()).filter(|i| anchors[*i]).collect();
    assert!(!items.is_empty(), "placed weapons with entities parented to them");
    let item = items[0];
    let name = map.entities[item].get("targetname").unwrap().to_string();
    let children: Vec<usize> = map
        .entities
        .iter()
        .enumerate()
        .filter(|(_, e)| e.get("parentname").is_some_and(|p| parent_name(p).eq_ignore_ascii_case(&name)))
        .map(|(i, _)| i)
        .collect();
    assert!(!children.is_empty());
    let mut sim = Sim::new(CsSim, (MapPlugin::new(map), SourceMovementPlugin, CsWeaponsPlugin));
    sim.set_tick_interval(cs_source::TICK_INTERVAL);
    sim.ticks(2);
    sim.app.world_mut().resource_mut::<crate::stack::logic::Logic>().world.record = true;
    let lying = |sim: &mut Sim| {
        let world = sim.app.world_mut();
        world
            .query::<(&Loose, &MapWeapon, &Transform)>()
            .iter(world)
            .find(|(_, m, _)| m.0 == item)
            .map(|(l, _, t)| (l.weapon, t.translation))
    };
    let (weapon, at) = lying(&mut sim).expect("the item lies where the map put it");
    let node = |sim: &mut Sim, index: usize| {
        let world = sim.app.world_mut();
        world
            .query::<(&PropEntity, &GlobalTransform)>()
            .iter(world)
            .find(|(p, _)| p.0 == index)
            .map(|(_, g)| g.translation())
    };
    let anchor = |sim: &mut Sim| {
        let world = sim.app.world_mut();
        world
            .query::<(&MapAnchor, &GlobalTransform)>()
            .iter(world)
            .find(|(a, _)| a.0 == item)
            .map(|(_, g)| g.translation())
            .expect("the item's anchor node")
    };
    let props: Vec<(usize, f32)> = children
        .iter()
        .filter_map(|c| Some((*c, node(&mut sim, *c)?.distance(anchor(&mut sim)))))
        .collect();
    // Walk onto it: picked up.
    let p = sim.spawn_character(at + Vec3::Y * 0.1, crate::stack::games::cs_source::movement::ID);
    sim.ticks(10);
    let owner = sim.app.world().get::<Weapon>(weapon).and_then(|w| w.owner);
    assert_eq!(owner, Some(p), "picked up");
    {
        let logic = sim.app.world().resource::<crate::stack::logic::Logic>();
        let id = logic.world.by_map_index(item).expect("the item's entity");
        assert!(
            logic.world.fired.iter().any(|(_, e, o)| *e == id && o == "OnPlayerPickup"),
            "OnPlayerPickup"
        );
        for line in logic.world.log.iter().filter(|l| l.contains("unhandled")).take(20) {
            eprintln!("logic: {line}");
        }
    }
    // Carried away: the anchor follows the player, the props keep their
    // distance from it.
    let away = at + Vec3::new(8.0, 0.0, 3.0);
    sim.app.world_mut().get_mut::<Transform>(p).unwrap().translation = away;
    sim.seconds(1.0);
    let a = anchor(&mut sim);
    assert!(a.xz().distance(sim.position(p).xz()) < 0.05, "anchor {a} at the player {}", sim.position(p));
    // Standing on the floor (what rides along doesn't lift its carrier),
    // the anchor 31 units above the feet.
    let state = sim.state(p);
    assert!(state.on_ground, "the carrier stands");
    let feet = sim.position(p).y + state.hull_min.y;
    let above = (a.y - feet) / cs_source::bsp::METERS_PER_UNIT;
    assert!((above - 31.0).abs() < 0.1, "anchor {above} units above the feet");
    for (c, d) in props {
        let now = node(&mut sim, c).unwrap();
        assert!((now.distance(a) - d).abs() < 0.05, "prop {c}: {d} from the anchor, now {}", now.distance(a));
    }
}

/// mg_creative_multigames_v8_ns's minigame rooms are walled off from each
/// other by sky faces. From the playtest's spot (`setpos -6332.74 -3047.50
/// -9638.97`) other rooms' placed weapons and water showed through the
/// sky: they weren't culled by the map's visibility as the world is (and
/// as Source culls every entity). Now loose items, moving props and water
/// surfaces (in chunks) are drawn exactly when one of their clusters is
/// potentially visible.
#[test]
fn other_rooms_items_and_water_are_culled_on_creative_multigames() {
    let Some(map) = load("mg_creative_multigames_v8_ns") else { return };
    assert!(!map.sky_surfaces.is_empty() && map.sky_surfaces.len() % 3 == 0, "sky faces kept");
    let v = map.visibility.clone().expect("visibility");
    let eye = Vec3::new(-6332.74, -9638.97 + 64.0, 3047.50) * cs_source::bsp::METERS_PER_UNIT;
    let cluster = v.cluster_at(eye).expect("the spot is in a cluster");
    let mut sim = Sim::new(CsSim, (
        |app: &mut App| {
            app.init_asset::<Image>()
                .init_asset::<StandardMaterial>()
                .init_asset::<crate::stack::render::world_material::WorldMaterial>()
                .init_asset::<crate::stack::render::prop_material::PropMaterial>()
                .init_asset::<crate::stack::render::sprite_material::SpriteMaterial>()
                .init_asset::<crate::stack::render::water::WaterMaterial>()
                .init_asset::<bevy::mesh::skinning::SkinnedMeshInverseBindposes>();
        },
        MapPlugin::new(map),
        SourceMovementPlugin,
        CsWeaponsPlugin,
    ));
    sim.set_tick_interval(cs_source::TICK_INTERVAL);
    let view = sim.app.world_mut().spawn((Camera3d::default(), Transform::from_translation(eye))).id();
    sim.ticks(1);
    // The map's own cameras (the water's reflection, the sky's) are
    // switched by systems that need rendering: off here.
    let world = sim.app.world_mut();
    let mut cams = world.query::<(Entity, &mut Camera)>();
    for (e, mut c) in cams.iter_mut(world) {
        c.is_active = e == view;
    }
    // Placed weapons settle (they fall onto what they rest on).
    sim.seconds(3.0);
    let world = sim.app.world_mut();
    assert_eq!(world.resource::<crate::stack::render::vis::VisStats>().cluster, Some(cluster));
    let check = |what: &str, parts: Vec<(Box<[u32]>, bool)>| {
        let hidden = parts.iter().filter(|(_, on)| !on).count();
        for (clusters, on) in &parts {
            assert!(!clusters.is_empty(), "{what}: untagged");
            assert_eq!(*on, v.sees_any(cluster, clusters), "{what}: clusters {clusters:?}");
        }
        eprintln!("{what}: {} drawn, {hidden} culled", parts.len() - hidden);
        assert!(hidden > 0, "{what}: other rooms' culled");
    };
    let mut q = world.query_filtered::<&crate::stack::map::VisClusters, With<crate::stack::map::loose::LooseItem>>();
    let items = q.iter(world).map(|c| (c.clusters.clone(), c.potentially_visible)).collect();
    check("placed weapons", items);
    // Drawn surfaces only: fog-only water (another shader's water brush,
    // drawn as the world) spawns a bare `WaterSurface` with its bounds,
    // for the eye's fog volume, which draws nothing to cull.
    let mut q = world.query::<(
        &crate::stack::map::water::WaterSurface,
        Option<&crate::stack::map::VisClusters>,
        Has<MeshMaterial3d<crate::stack::render::water::WaterMaterial>>,
    )>();
    let water = q
        .iter(world)
        .filter(|(s, _, drawn)| !s.skybox && *drawn)
        .map(|(_, c, _)| c.map_or((Box::default(), true), |c| (c.clusters.clone(), c.potentially_visible)))
        .collect();
    check("water surfaces", water);
}

/// mg_jacks_multigames_v1 (a playtest, `setpos -12146.96 15343.41
/// -927.97; setang -29.57 184.04 0`): the flood's water (a
/// func_water_analog, drawn under its mover's node) showed through the
/// sky from another room: moving water had no clusters, so it was never
/// culled. Now it is tagged by its moved bounds, as brush entities are.
#[test]
fn moving_water_is_culled_on_jacks() {
    let Some(map) = load("mg_jacks_multigames_v1") else { return };
    let v = map.visibility.clone().expect("visibility");
    let eye = Vec3::new(-12146.96, -927.97 + 64.0, -15343.41) * cs_source::bsp::METERS_PER_UNIT;
    let cluster = v.cluster_at(eye).expect("the spot is in a cluster");
    let mut sim = Sim::new(CsSim, (
        |app: &mut App| {
            app.init_asset::<Image>()
                .init_asset::<StandardMaterial>()
                .init_asset::<crate::stack::render::world_material::WorldMaterial>()
                .init_asset::<crate::stack::render::prop_material::PropMaterial>()
                .init_asset::<crate::stack::render::sprite_material::SpriteMaterial>()
                .init_asset::<crate::stack::render::water::WaterMaterial>()
                .init_asset::<bevy::mesh::skinning::SkinnedMeshInverseBindposes>();
        },
        MapPlugin::new(map),
        SourceMovementPlugin,
        CsWeaponsPlugin,
    ));
    sim.set_tick_interval(cs_source::TICK_INTERVAL);
    let view = sim.app.world_mut().spawn((Camera3d::default(), Transform::from_translation(eye))).id();
    sim.ticks(1);
    let world = sim.app.world_mut();
    let mut cams = world.query::<(Entity, &mut Camera)>();
    for (e, mut c) in cams.iter_mut(world) {
        c.is_active = e == view;
    }
    sim.ticks(10);
    let world = sim.app.world_mut();
    assert_eq!(world.resource::<crate::stack::render::vis::VisStats>().cluster, Some(cluster));
    let mut q = world.query::<(
        &crate::stack::map::water::WaterSurface,
        Option<&crate::stack::map::VisClusters>,
        Has<crate::stack::map::water::MovingWaterSurface>,
    )>();
    let mut moving = 0;
    for (s, c, moves) in q.iter(world).filter(|(s, ..)| !s.skybox) {
        let c = c.unwrap_or_else(|| panic!("{:?}..{:?} untagged (moving: {moves})", s.min, s.max));
        assert!(!c.clusters.is_empty(), "{:?}..{:?}: no clusters", s.min, s.max);
        assert_eq!(c.potentially_visible, v.sees_any(cluster, &c.clusters));
        if moves {
            moving += 1;
            assert!(!c.potentially_visible, "the flood, another room's, is culled here");
        }
    }
    assert!(moving > 0, "the flood's surfaces");
}

/// The 2D sky's bottom face (`dn`) joins the side faces: on community
/// skies whose bottom is a picture (stock CS:S bottoms are one colour),
/// the orientation `cs_source::sky::FACES` gives it has the smallest seam
/// of the eight (`map::sky_seam_error`; the top face's fitted one wins
/// the same way on stock skies: `map_stock::sky_top_and_bottom_join_the_sides`).
#[test]
fn sky_bottom_joins_the_sides() {
    let names = [
        "gg_lego_spacetower2",
        "kz_hikari_od_nh_v2",
        "kz_bhop_izanami",
        "surf_happyhands",
        "surf_stickybutt_alpha",
        "gg_dev_moment_v1",
        "gg_desert_paintball",
        "gg_mario_vs_wario",
        "gg_fusion_trx",
        "mg_kommando",
    ];
    let dn = cs_source::sky::FACES.iter().find(|f| f.0 == "dn").unwrap();
    let mut decided = 0;
    for name in names {
        let Some(map) = load(name) else { continue };
        let Some(sky) = map.sky.as_ref() else { continue };
        let errors: Vec<f32> = (0..8u8)
            .map(|turns| {
                let mut s = sky.clone();
                s.faces[dn.1].1 = turns;
                crate::stack::map::sky_seam_error(&s, &map.textures, dn.1)
            })
            .collect();
        eprintln!("{name}: {errors:?}");
        // One-colour bottoms join every way alike.
        let (lo, hi) = errors.iter().fold((f32::MAX, f32::MIN), |(a, b), e| (a.min(*e), b.max(*e)));
        if hi - lo < 2.0 {
            continue;
        }
        let best = (0..8).min_by(|a, b| errors[*a].total_cmp(&errors[*b])).unwrap();
        assert_eq!(best, dn.2 as usize, "{name}: {errors:?}");
        decided += 1;
    }
    eprintln!("{decided} skies decide the bottom face");
}

/// Visual entities the logic audit found undrawn (community-maps.md, "Map
/// logic audit, round 2"): gg_future's spinning laser trails
/// (env_spritetrail), gg_nukkon_hdr's cooling-tower smoke
/// (env_smokestack), the `.pcf` effects of info_particle_system from the
/// install (surf_hellenic's env_fire_large) and from the map's own pak
/// (mg_crazykart_v1_1's kart effects), and surf_demise's faint
/// func_illusionary glass (rendermode 1, renderamt 20) drawn blended.
#[test]
fn visual_entities_load() {
    use crate::stack::map::MapAlpha;
    // Round 3: color correction tables, embers, a muzzle flash.
    for name in ["mg_lt_galaxy_v5", "surf_demise"] {
        if let Some(map) = load(name) {
            assert_eq!(map.color_corrections.len(), 1, "{name}: its .raw table from the pak");
            let cc = &map.color_corrections[0];
            assert!(cc.start_on && cc.max_falloff < 0.0 && cc.max_weight == 1.0, "{name}: everywhere");
            assert_eq!(cc.lut.len(), 32 * 32 * 32 * 3);
        }
    }
    if let Some(map) = load("mg_jacks_multigames_v1") {
        assert_eq!(map.source_fx().embers.len(), 6);
        assert_eq!(map.source_fx().embers.iter().filter(|e| e.start_on).count(), 3, "flag 1 starts on");
    }
    if let Some(map) = load("surf_stickybutt_alpha") {
        assert_eq!(map.source_fx().embers.len(), 1);
        let e = &map.source_fx().embers[0];
        eprintln!("stickybutt embers {:?}..{:?}", e.min, e.max);
        // Pitch 90: down.
        assert!(e.velocity.y < 0.0 && e.velocity.x.abs() < 1e-4, "{}", e.velocity);
    }
    if let Some(map) = load("mg_kommando") {
        assert_eq!(map.muzzle_flashes.len(), 1);
    }
    if let Some(map) = load("gg_future") {
        assert_eq!(map.trails.len(), 4);
        assert!(map.trails.iter().all(|t| t.life == 10.0 && t.entity.is_some()));
    }
    if let Some(map) = load("gg_nukkon_hdr") {
        assert_eq!(map.source_fx().smokestacks.len(), 4);
        assert!(
            map.source_fx().smokestacks
                .iter()
                .all(|s| s.life().is_some_and(|l| (l - 10.0).abs() < 1e-3))
        );
    }
    if let Some(map) = load("surf_hellenic") {
        let ps = &map.particle_systems;
        assert_eq!(ps.placed.len(), 160);
        let fire = ps
            .find("env_fire_large")
            .expect("env_fire_large from the install's fire_01.pcf");
        assert!(!ps.defs[fire].children.is_empty(), "its flames and embers");
        assert!(ps.defs[fire].material.is_some());
        assert!(
            !map.warnings.iter().any(|w| w.contains("particle")),
            "{:?}",
            map.warnings
        );
    }
    if let Some(map) = load("mg_crazykart_v1_1") {
        let ps = &map.particle_systems;
        assert_eq!(ps.placed.len(), 40);
        assert!(ps.find("kart_boost").is_some(), "from the map's packed crazykart.pcf");
        assert!(
            !map.warnings.iter().any(|w| w.contains("particle")),
            "{:?}",
            map.warnings
        );
    }
    if let Some(map) = load("surf_demise") {
        let faint = map
            .meshes
            .iter()
            .filter(|m| matches!(m.render, Some((MapAlpha::Blend, a)) if (a - 20.0 / 255.0).abs() < 1e-4))
            .count();
        assert!(faint > 0, "the func_illusionary glass is drawn faint");
        assert!(
            map.meshes
                .iter()
                .filter(|m| m.render.is_some())
                .all(|m| m.alpha == MapAlpha::Blend)
        );
    }
}

/// Detail sprites on community maps: kz_ancient_ruins' detail lump is
/// LZMA-compressed and its sprite sheet packed in the map; its
/// env_detail_controller fades them out over 1648-2048 units (a negative
/// start is the end less cl_detailfade's 400). mg_kommando's sprites face
/// the view, turning about the vertical only.
#[test]
fn detail_sprites_on_community_maps() {
    if let Some(map) = load("kz_ancient_ruins") {
        let d = map.source_fx().detail_props.as_ref().expect("kz_ancient_ruins detail sprites");
        assert!(!d.quads.is_empty());
        assert_eq!(d.fade, Some((1648.0 * 0.0254, 2048.0 * 0.0254)));
        assert!(map.textures[d.texture].width > 1, "its packed sheet");
    }
    if let Some(map) = load("mg_kommando") {
        let d = map.source_fx().detail_props.as_ref().expect("mg_kommando detail sprites");
        assert_eq!(d.quads.len(), 846);
        assert!(d.quads.iter().all(|q| q.billboard.is_some_and(|b| b.vertical)));
        assert_eq!(d.fade, None, "no controller: cl_detaildist and cl_detailfade");
    }
}

/// Map-placed ragdolls are simulated (physics_brushes.md 6): gg_deagle7k's
/// corpse (debris, no Hammer pose) starts from its first sequence at the
/// entity's angles, falls and settles on the floor, its drawn node with
/// it; a shot through a part pushes that part; a round restart puts it
/// back where it was placed.
#[test]
fn map_ragdolls_fall_and_take_shots() {
    use crate::stack::map::{
        RagdollShot,
        placed_ragdoll::{PlacedRagdoll, SF_DEBRIS},
    };
    // The other maps' ragdolls (mattresses, a zombie, bloody humans,
    // corpses) have theirs too.
    for (name, n) in [
        ("mg_boatrace_scramble", 4),
        ("mg_escape_prison_beta", 1),
        ("mg_jacks_multigames_v1", 2),
        ("mg_n64_goldeneye_v2", 1),
        ("surf_happyhands", 1),
    ] {
        if let Some(map) = load(name) {
            assert_eq!(map.props.iter().filter(|p| p.ragdoll.is_some()).count(), n, "{name}");
        }
    }
    let Some(map) = load("gg_deagle7k") else { return };
    let placed = map.props.iter().filter(|p| p.ragdoll.is_some()).count();
    assert_eq!(placed, 1, "the corpse has a ragdoll .phy");
    let mut sim = Sim::new(CsSim, MapPlugin::new(map));
    sim.set_tick_interval(cs_source::TICK_INTERVAL);
    sim.ticks(2);
    let parts = |sim: &mut Sim| -> Vec<(Entity, Vec3)> {
        let world = sim.app.world_mut();
        let sims: Vec<Vec<Entity>> = world
            .query::<&crate::stack::map::Ragdoll>()
            .iter(world)
            .filter(|r| r.placed)
            .map(|r| r.bodies.clone())
            .collect();
        assert_eq!(sims.len(), 1, "one placed ragdoll simulated");
        sims[0]
            .iter()
            .map(|b| (*b, world.get::<Transform>(*b).unwrap().translation))
            .collect()
    };
    let flags = {
        let world = sim.app.world_mut();
        world.query::<&PlacedRagdoll>().single(world).unwrap().flags
    };
    assert!(flags & SF_DEBRIS != 0);
    let start = parts(&mut sim);
    assert!(start.len() > 5, "{} bodies", start.len());
    sim.seconds(4.0);
    let rest = parts(&mut sim);
    let fell = start[0].1.y - rest[0].1.y;
    assert!(fell > 0.05, "the pelvis fell {fell} m");
    // At rest on the floor (not through it): within about a metre of
    // where it was placed.
    let floor_ok = rest.iter().all(|(_, p)| (p.y - start[0].1.y).abs() < 1.5);
    assert!(floor_ok, "{rest:?}");
    let node_y = {
        let world = sim.app.world_mut();
        world
            .query_filtered::<&GlobalTransform, With<PlacedRagdoll>>()
            .single(world)
            .unwrap()
            .translation()
            .y
    };
    assert!((node_y - rest[0].1.y).abs() < 0.5, "the drawn node follows the pelvis");
    // A shot through the highest part pushes it.
    let (part, at) = *rest.iter().max_by(|a, b| a.1.y.total_cmp(&b.1.y)).unwrap();
    sim.app.world_mut().write_message(RagdollShot::bullet(
        at + Vec3::new(-2.0, 0.0, 0.0),
        at + Vec3::new(2.0, 0.0, 0.0),
    ));
    sim.ticks(3);
    let v = sim.app.world().get::<avian3d::prelude::LinearVelocity>(part).unwrap().0;
    assert!(v.x > 0.1, "the shot part moves along the shot: {v}");
    // A round restart: made again where it was placed.
    crate::stack::core::restart_round(sim.app.world_mut());
    sim.ticks(3);
    let again = parts(&mut sim);
    assert!(
        again[0].1.distance(start[0].1) < 0.2,
        "back at {} (placed {})",
        again[0].1,
        start[0].1
    );
}

/// Moving water (func_water_analog): mg_jacks_multigames_v1's flood
/// ("water": up 635 units at 11 units/s on Open) rises, its swim volume
/// with it; water parented to a mover (mg_3k_smash_lego_copter's "water"
/// on the spinning "midrot") goes where its parent takes it.
#[test]
fn moving_water_follows_its_entity() {
    use crate::stack::{
        core::MapWater,
        logic::{Logic, Value},
        map::{entities::MapBrushEntity, water::WaterMovers},
    };
    let unit = cs_source::bsp::METERS_PER_UNIT;
    if let Some(map) = load("mg_jacks_multigames_v1") {
        let flood = map
            .entities
            .iter()
            .position(|e| {
                e.get("targetname") == Some("water") && e.classname().eq_ignore_ascii_case("func_water_analog")
            })
            .expect("the flood");
        let mut sim = Sim::new(CsSim, MapPlugin::new(map));
        sim.set_tick_interval(cs_source::TICK_INTERVAL);
        sim.ticks(2);
        let volume = sim
            .app
            .world()
            .resource::<WaterMovers>()
            .0
            .iter()
            .find(|m| m.1 == flood)
            .map(|m| m.0)
            .expect("the flood's volume moves with it");
        let top = |sim: &Sim| sim.app.world().resource::<MapWater>().0[volume].brush.max.y;
        let before = top(&sim);
        sim.app
            .world_mut()
            .resource_mut::<Logic>()
            .world
            .queue_input("water", "Open", Value::Void, 0.0, None);
        sim.seconds(5.0);
        let risen = (top(&sim) - before) / unit;
        assert!((risen - 55.0).abs() < 4.0, "5 s at 11 units/s: rose {risen} units");
        let node = {
            let world = sim.app.world_mut();
            world
                .query::<(&MapBrushEntity, &GlobalTransform)>()
                .iter(world)
                .find(|(b, _)| b.0 == flood)
                .map(|(_, g)| g.translation())
                .expect("the flood's node")
        };
        let placed = -273.6 * unit;
        assert!(
            ((node.y - placed) / unit - risen).abs() < 2.0,
            "its node rose with it: {}",
            node.y / unit
        );
    }
    if let Some(map) = load("mg_3k_smash_lego_copter") {
        let water = map
            .entities
            .iter()
            .position(|e| {
                e.get("targetname") == Some("water") && e.classname().eq_ignore_ascii_case("func_water_analog")
            })
            .expect("the parented water");
        let mut sim = Sim::new(CsSim, MapPlugin::new(map));
        sim.set_tick_interval(cs_source::TICK_INTERVAL);
        sim.ticks(2);
        let movers = sim.app.world().resource::<WaterMovers>().0.clone();
        let volume = movers
            .iter()
            .find(|m| m.1 == water)
            .map(|m| m.0)
            .expect("its volume moves with it");
        let at = |sim: &Sim| {
            let b = &sim.app.world().resource::<MapWater>().0[volume].brush;
            (b.min + b.max) / 2.0
        };
        let start = at(&sim);
        // Its parent spins up when the game starts: start it.
        sim.app
            .world_mut()
            .resource_mut::<Logic>()
            .world
            .queue_input("midrot", "Start", Value::Void, 0.0, None);
        sim.seconds(3.0);
        let node = {
            let world = sim.app.world_mut();
            world
                .query::<(&MapBrushEntity, &GlobalTransform)>()
                .iter(world)
                .find(|(b, _)| b.0 == water)
                .map(|(_, g)| g.compute_transform())
                .expect("its node")
        };
        let (home_brush, home) = movers
            .iter()
            .find(|m| m.1 == water)
            .map(|m| (m.2.clone(), m.3))
            .unwrap();
        let placed = (home_brush.min + home_brush.max) / 2.0;
        let moved = node.translation.distance(home.translation) + node.rotation.angle_between(home.rotation);
        if moved > 1e-3 {
            assert!(at(&sim).distance(start) > 1e-3, "the volume moved with its node");
        }
        // Wherever the node is, the volume sits where the node puts it.
        let expected = node.transform_point(home.to_matrix().inverse().transform_point3(placed));
        assert!(
            at(&sim).distance(expected) < 0.05,
            "volume at {} vs {expected}",
            at(&sim)
        );
    }
}

/// Gungame arenas' weapons as CS:S hands them out (the weapon ladder
/// itself is a server plugin's, not the map's): gg_iceworld_l33t's placed
/// weapons are picked up by walking onto them into an empty slot only;
/// gg_usp_deagle's game_player_equip is the whole kit; gg_deagle7k's
/// player_weaponstrip, fired at map spawn with no player, takes nothing.
#[test]
fn gungame_weapons_pick_up_equip_and_strip() {
    fn held(sim: &Sim, p: Entity) -> Vec<String> {
        let world = sim.app.world();
        let inv = world.get::<Inventory>(p).expect("an inventory");
        let mut ids: Vec<String> = inv
            .weapons
            .iter()
            .filter_map(|w| world.get::<Weapon>(*w))
            .map(|w| w.id.trim_start_matches("cs_source:").to_string())
            .collect();
        ids.sort();
        ids
    }
    fn lying(sim: &mut Sim, class: &str, entities: &[crate::stack::map::MapEntity]) -> Vec<(Entity, Vec3)> {
        let world = sim.app.world_mut();
        world
            .query::<(&Loose, &MapWeapon, &Transform)>()
            .iter(world)
            .filter(|(_, m, _)| entities[m.0].classname().eq_ignore_ascii_case(class))
            .map(|(l, _, t)| (l.weapon, t.translation))
            .collect()
    }
    let new_sim = |map: MapData| {
        let mut sim = Sim::new(CsSim, (MapPlugin::new(map), SourceMovementPlugin, CsWeaponsPlugin));
        sim.set_tick_interval(cs_source::TICK_INTERVAL);
        sim.ticks(2);
        sim
    };
    if let Some(map) = load("gg_iceworld_l33t") {
        let entities = map.entities.clone();
        let mut sim = new_sim(map);
        let (_, at) = lying(&mut sim, "weapon_ak47", &entities)[0];
        let p = sim.spawn_character(at + Vec3::Y * 0.1, cs_source::movement::ID);
        sim.ticks(10);
        // The spawn kit has no rifle (knife and pistol).
        let with_ak = held(&sim, p);
        assert_eq!(
            with_ak,
            vec!["weapon_ak47", "weapon_knife", "weapon_usp"],
            "walked onto with the rifle slot empty: picked up"
        );
        // Another rifle: the slot is full, so it stays (one weapon per slot).
        let m4s = lying(&mut sim, "weapon_m4a1", &entities).len();
        let (_, at) = lying(&mut sim, "weapon_m4a1", &entities)[0];
        sim.app.world_mut().get_mut::<Transform>(p).unwrap().translation = at + Vec3::Y * 0.1;
        sim.ticks(70);
        assert_eq!(held(&sim, p), with_ak, "the M4 stays");
        assert_eq!(lying(&mut sim, "weapon_m4a1", &entities).len(), m4s);
        // A grenade: its own slot.
        let (_, at) = lying(&mut sim, "weapon_hegrenade", &entities)[0];
        sim.app.world_mut().get_mut::<Transform>(p).unwrap().translation = at + Vec3::Y * 0.1;
        sim.ticks(70);
        assert!(held(&sim, p).contains(&"weapon_hegrenade".to_string()), "the grenade");
        // A round restart puts the carried placed weapons back where the
        // map put them; the player keeps its own.
        let placed = entities.iter().filter(|e| e.classname().starts_with("weapon_")).count();
        crate::stack::core::restart_round(sim.app.world_mut());
        sim.ticks(2);
        let world = sim.app.world_mut();
        let lying_now = world.query::<(&Loose, &MapWeapon)>().iter(world).count();
        assert_eq!(lying_now, placed, "every placed weapon lies there again");
    }
    if let Some(map) = load("gg_usp_deagle") {
        let mut sim = new_sim(map);
        let p = sim.spawn_character(Vec3::new(0.0, 50.0, 0.0), placeholder::ID);
        sim.ticks(2);
        assert_eq!(held(&sim, p), vec!["weapon_knife", "weapon_usp"], "the equip is the whole kit");
    }
    if let Some(map) = load("gg_deagle7k") {
        let mut sim = new_sim(map);
        let p = sim.spawn_character(Vec3::new(0.0, 50.0, 0.0), placeholder::ID);
        sim.ticks(2);
        let kit = held(&sim, p);
        assert!(!kit.is_empty());
        // OnMapSpawn's StripWeaponsAndSuit (no activator: no player).
        sim.app.world_mut().resource_mut::<crate::stack::logic::Logic>().world.queue_input(
            "strip",
            "StripWeaponsAndSuit",
            crate::stack::logic::Value::Void,
            0.0,
            None,
        );
        sim.ticks(5);
        assert_eq!(held(&sim, p), kit, "a strip with no player to strip takes nothing");
    }
}

/// mg_n64_goldeneye_v2's strike-generator env_beams (visual_entities.md
/// 4.2): a weapon button's StrikeOnce, 20 s later, strikes all 14 between
/// their info_targets (the map draws each for its 2 s life:
/// `render::emitters::StrikeEmitter`) and kills a player on a line (900).
#[test]
fn strike_beams_strike_and_kill() {
    use bevy::ecs::message::Messages;
    use crate::stack::{
        core::Health,
        logic::{Logic, Value},
        map::BeamStrike,
    };
    let Some(map) = load("mg_n64_goldeneye_v2") else { return };
    assert_eq!(map.source_fx().strikes.len(), 14, "its strike generators' looks");
    assert!(map.source_fx().strikes.iter().all(|s| s.noise == 20.0 && (s.width / 0.0254 - 50.0).abs() < 1e-3));
    let mut sim = Sim::new(CsSim, (MapPlugin::new(map), SourceMovementPlugin, CsWeaponsPlugin));
    sim.set_tick_interval(cs_source::TICK_INTERVAL);
    sim.ticks(2);
    // On the line from t1 (-18 -32 2994) to t2 (94 208 2900), halfway.
    let p = sim.spawn_character(
        cs_source::movement::to_engine(Vec3::new(38.0, 88.0, 2947.0)),
        cs_source::movement::ID,
    );
    sim.ticks(2);
    sim.app
        .world_mut()
        .resource_mut::<Logic>()
        .world
        .queue_input("beam", "StrikeOnce", Value::Void, 0.0, None);
    let mut strikes: Vec<BeamStrike> = Vec::new();
    for _ in 0..3 {
        sim.ticks(1);
        let messages = sim.app.world().resource::<Messages<BeamStrike>>();
        strikes.extend(messages.iter_current_update_messages().copied());
    }
    strikes.dedup();
    assert_eq!(strikes.len(), 14, "every beam struck");
    assert!(strikes.iter().all(|s| s.life == 2.0 && s.start.distance(s.end) > 1.0));
    let health = sim.app.world().get::<Health>(p).map_or(0.0, |h| h.current);
    assert!(health <= 0.0, "the beam through the player kills: {health}");
}

/// Sprites in the blended (1, 2, 4) and normal (0) render modes are drawn
/// (sprites_dust.md 3): surf_sacrifice's name labels (texture mode),
/// mg_swag_multigames_v1's buy zone signs (normal).
#[test]
fn blended_and_normal_sprites_load() {
    use crate::stack::map::SpriteBlend;
    if let Some(map) = load("surf_sacrifice") {
        let labels = map.sprites.iter().filter(|s| s.blend == SpriteBlend::Blend).count();
        assert!(labels >= 14, "{labels}");
        assert!(!map.warnings.iter().any(|w| w.contains("render modes")), "{:?}", map.warnings);
    }
    if let Some(map) = load("mg_swag_multigames_v1") {
        let signs = map.sprites.iter().filter(|s| s.blend == SpriteBlend::Opaque).count();
        assert_eq!(signs, 7);
    }
}

/// DecalModulate decals and overlays (blood, dirt, scratches: the surface
/// times 2 x the texel) multiply what they lie on. They had no texture
/// (the VMT parser's accessor skips the shader), so infodecals were never
/// placed and overlays drew untextured; now they are blended modulate
/// decals (`MapMesh::modulate`). gg_bk_warehouse_v1's infodecals use the
/// stock `decals/blood*`.
#[test]
fn modulate_decals_multiply_the_surface() {
    let Some(map) = load("gg_bk_warehouse_v1") else { return };
    let blood: Vec<_> = map
        .meshes
        .iter()
        .filter(|m| m.material.starts_with("decal:decals/blood"))
        .collect();
    assert!(blood.len() >= 8, "{} blood decals placed (they had no texture)", blood.len());
    for m in &blood {
        assert!(m.modulate, "{}: not modulate", m.material);
        assert_eq!(m.alpha, crate::stack::map::MapAlpha::Blend, "{}", m.material);
    }
    // Ordinary decals stay what they were.
    assert!(
        map.meshes
            .iter()
            .filter(|m| m.material.starts_with("decal:") && !m.material.contains("blood"))
            .all(|m| !m.modulate)
    );
}

/// A map without `func_buyzone` (mg_jacks_multigames_v1; buying anywhere
/// in the playtest of 2026-10-11): buy zones around each team's own
/// spawn points (`SPAWN_BUY_ZONE_RADIUS`, ours: the size is
/// undocumented), none out in the games; no `info_map_parameters`, so
/// both teams may buy. (`mp_buytime` closes buying as on every map:
/// `rules::rounds`.)
#[test]
fn spawn_buy_zones_without_func_buyzone() {
    use crate::stack::{
        core::Team,
        map::MapEntities,
        weapon::economy::{Money, SPAWN_BUY_ZONE_RADIUS, cannot_buy_here, in_buy_zone},
    };
    let Some(map) = load("mg_jacks_multigames_v1") else { return };
    assert!(!map.entities.iter().any(|e| e.classname().eq_ignore_ascii_case("func_buyzone")));
    let mut w = World::new();
    w.insert_resource(MapEntities {
        entities: std::sync::Arc::new(map.entities.clone()),
        scale: map.entity_scale,
    });
    let mut check = |at: Vec3, team: u8| {
        let e = w.spawn((Transform::from_translation(at), Team(team), Money(800))).id();
        let r = (in_buy_zone(&w, e), cannot_buy_here(&w, e));
        w.despawn(e);
        r
    };
    let spawns: Vec<(Vec3, u8)> = map.spawns.iter().filter_map(|(p, t)| Some((*p, (*t)?.0))).collect();
    assert!(spawns.iter().any(|s| s.1 == 1) && spawns.iter().any(|s| s.1 == 2), "both teams spawn");
    let reach = SPAWN_BUY_ZONE_RADIUS * map.entity_scale;
    let near = |a: Vec3, b: Vec3, k: f32| (a.x - b.x).abs() <= k * reach && (a.z - b.z).abs() <= k * reach;
    for (at, team) in &spawns {
        assert_eq!(check(*at, *team), (true, None), "team {team} at its spawn {at}");
        // The other team's spawns are elsewhere: not its zone (unless one
        // of its own is near).
        if !spawns.iter().any(|(p, t)| *t == 3 - team && near(*p, *at, 1.0)) {
            assert!(!check(*at, 3 - team).0, "team {} at the other's spawn {at}", 3 - team);
        }
    }
    // Out in the map, past every spawn's reach.
    let far = map
        .meshes
        .iter()
        .flat_map(|m| m.positions.iter().map(|p| Vec3::from(*p)))
        .find(|p| spawns.iter().all(|(s, _)| !near(*p, *s, 2.0)))
        .expect("somewhere far from the spawns");
    assert!(!check(far, 1).0 && !check(far, 2).0, "no buying at {far}");
}
