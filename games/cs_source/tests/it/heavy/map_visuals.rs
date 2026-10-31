//! What the logic switches on real CS:S maps, headless: cs_office's
//! projector glow sprites (killed) and de_prodigy's bomb-site sprites and
//! lights; de_nuke's computer screens (Skin), cars (SetAnimation) and a
//! removed prop that stops blocking; de_prodigy's switchable light styles
//! in the lightmap; de_aztec's soundscape triggers through the logic's
//! touch code. Skipped without an install.

use avian3d::prelude::ColliderDisabled;
use bevy::prelude::*;
use crate::stack::{
    core::{LocalPlayer, MapBrushes, MovingSolid},
    games::{
        self, cs_source,
        cs_source::movement::{SourceMovementPlugin, to_engine},
    },
    harness::{CsSim, Sim},
    logic::{Logic, Value},
    map::{LightStyles, MapData, MapPlugin, PropEntity, PropLook, PropSequence, SoundscapeTouches},
    mount::config::LocalConfig,
    movement::noclip,
};

fn load(name: &str) -> Option<MapData> {
    let installed = LocalConfig::load()
        .ok()?
        .game_path_in(cs_source::GAME, cs_source::INFO.steam.as_ref())
        .is_some_and(|p| chimera_core::mount::served::usable("cs_source", p, "cstrike"));
    if !installed {
        eprintln!("skipping: no CS:S install configured");
        return None;
    }
    Some(games::load_map(&format!("cs_source:{name}")).expect("load map"))
}

fn indices(map: &MapData, name: &str) -> Vec<usize> {
    map.entities
        .iter()
        .enumerate()
        .filter(|(_, e)| e.get("targetname").is_some_and(|n| n.eq_ignore_ascii_case(name)))
        .map(|(i, _)| i)
        .collect()
}

fn sim(map: MapData) -> Sim {
    let mut sim = Sim::new(CsSim, (MapPlugin::new(map), SourceMovementPlugin));
    sim.set_tick_interval(cs_source::TICK_INTERVAL);
    sim.ticks(2);
    sim
}

fn input(sim: &mut Sim, target: &str, input: &str, value: Value) {
    let mut logic = sim.app.world_mut().resource_mut::<Logic>();
    logic.world.queue_input(target, input, value, 0.0, None);
}

/// The prop node of map entity `index`.
fn prop_node(sim: &mut Sim, index: usize) -> Entity {
    let world = sim.app.world_mut();
    world
        .query::<(Entity, &PropEntity)>()
        .iter(world)
        .find(|(_, p)| p.0 == index)
        .map(|(e, _)| e)
        .unwrap_or_else(|| panic!("no prop node for entity {index}"))
}

#[test]
fn sprites_load_with_their_entity_and_follow_the_logic() {
    let Some(map) = load("cs_office") else { return };
    let glows = indices(&map, "projectorglow");
    assert_eq!(glows.len(), 14, "cs_office's projector glows");
    // All loaded (named, "Start on"), each knowing its entity.
    for i in &glows {
        let s = map.sprites.iter().find(|s| s.entity == Some(*i));
        assert!(s.is_some_and(|s| s.start_on), "sprite of entity {i}");
    }
    let Some(prodigy) = load("de_prodigy") else { return };
    // A named sprite without "Start on" loads too, hidden at start.
    let starts_off = prodigy
        .sprites
        .iter()
        .filter(|s| s.entity.is_some() && !s.start_on)
        .count();
    eprintln!(
        "de_prodigy: {} sprites, {starts_off} start hidden",
        prodigy.sprites.len()
    );

    let mut sim = sim(map);
    let parts = |sim: &Sim| {
        let logic = sim.app.world().resource::<Logic>();
        logic.world.part_states()
    };
    assert!(glows.iter().all(|i| parts(&sim).iter().any(|p| p.0 == *i && p.2)));
    // The projector's hum stops and its glow goes when shot (here: the
    // Kill its OnHealthChanged sends).
    input(&mut sim, "projectorglow", "Kill", Value::Void);
    sim.ticks(1);
    assert!(
        glows.iter().all(|i| !parts(&sim).iter().any(|p| p.0 == *i)),
        "killed glows are gone"
    );
    crate::stack::core::restart_round(sim.app.world_mut());
    sim.ticks(1);
    assert!(
        glows.iter().all(|i| parts(&sim).iter().any(|p| p.0 == *i && p.2)),
        "back next round"
    );
}

/// de_train's two dust clouds (func_dustcloud, sprites_dust.md 7): world-
/// sized motes inside their brushes, off until the logic turns them on.
#[test]
fn de_train_dust_clouds_load_and_turn_on() {
    let Some(map) = load("de_train") else { return };
    let clouds: Vec<_> = map.source_fx().dust.iter().filter(|d| d.world_size).collect();
    assert_eq!(clouds.len(), 2, "de_train's dustclouda and dustcloudb");
    for c in &clouds {
        eprintln!("cloud of entity {:?}: {} to {} (engine)", c.entity, c.min, c.max);
        assert!(!c.start_on && !c.frozen && !c.brushes.is_empty());
        // SizeMin 64, SizeMax 128 units as half-widths; alpha 100.
        assert!((c.size.0 / 0.0254 - 64.0).abs() < 1e-3 && (c.size.1 / 0.0254 - 128.0).abs() < 1e-3, "{:?}", c.size);
        assert!((c.color[3] - 100.0 / 255.0).abs() < 1e-6);
    }
    let index = clouds[0].entity.expect("its entity");
    let name = map.entities[index].get("targetname").expect("named").to_string();
    let mut sim = sim(map);
    let on = |sim: &Sim| {
        let logic = sim.app.world().resource::<Logic>();
        logic.world.part_states().iter().any(|p| p.0 == index && p.2)
    };
    assert!(!on(&sim));
    input(&mut sim, &name, "TurnOn", Value::Void);
    sim.ticks(1);
    assert!(on(&sim), "{name} turned on");
}

#[test]
fn de_nuke_screens_change_skin_and_cars_play() {
    let Some(map) = load("de_nuke") else { return };
    let screens = indices(&map, "computer06");
    assert!(!screens.is_empty());
    let screen = screens[0];
    let prop = map
        .props
        .iter()
        .find(|p| p.entity == Some(screen))
        .expect("screen prop");
    let skins = map.models[prop.model].skins.len();
    assert!(skins >= 3, "the console has its screen skins ({skins})");
    let a = &map.models[prop.model].skins[0];
    let c = &map.models[prop.model].skins[2];
    assert!(
        a.iter().zip(c).any(|(a, c)| a.material != c.material),
        "skin 2 changes a material"
    );
    // The cars are animated: skeleton and sequences.
    let car = indices(&map, "car01")[0];
    let car_prop = map.props.iter().find(|p| p.entity == Some(car)).expect("car prop");
    let rig = map.models[car_prop.model].rig.as_ref().expect("the car has a rig");
    assert!(rig.animations.sequence("run").is_some() && rig.animations.sequence("hide").is_some());

    let mut sim = sim(map);
    let node = prop_node(&mut sim, screen);
    assert_eq!(sim.app.world().get::<PropLook>(node).map(|l| l.skin), Some(0));
    input(&mut sim, "computer06", "Skin", Value::Int(2));
    sim.ticks(1);
    assert_eq!(sim.app.world().get::<PropLook>(node).map(|l| l.skin), Some(2));
    let car_node = prop_node(&mut sim, car);
    let seq = sim.app.world().get::<PropSequence>(car_node).cloned().unwrap();
    assert_eq!((seq.play, seq.default.as_str()), (None, "hide"));
    input(&mut sim, "car01", "SetAnimation", Value::Str("run".into()));
    sim.ticks(1);
    let seq = sim.app.world().get::<PropSequence>(car_node).cloned().unwrap();
    assert_eq!(seq.play, Some(("run".to_string(), 1)));
}

#[test]
fn a_removed_prop_stops_blocking() {
    let Some(map) = load("de_nuke") else { return };
    let mut sim = sim(map);
    // A solid prop entity carries its own brushes (not the map's).
    let world = sim.app.world_mut();
    let (node, index, brushes) = world
        .query::<(Entity, &PropEntity, &MovingSolid)>()
        .iter(world)
        .find(|(_, _, m)| m.solid && !m.brushes.is_empty())
        .map(|(e, p, m)| (e, p.0, m.brushes.clone()))
        .expect("a solid prop entity");
    let in_map = world
        .resource::<MapBrushes>()
        .0
        .iter()
        .any(|b| b.min == brushes[0].min && b.max == brushes[0].max);
    assert!(!in_map, "its brushes are its own");
    let id = {
        let logic = world.resource::<Logic>();
        logic
            .world
            .ids()
            .into_iter()
            .find(|id| logic.world.get(*id).unwrap().map_index == Some(index))
            .unwrap()
    };
    world.resource_mut::<Logic>().world.kill(id);
    sim.ticks(1);
    let solid = sim.app.world().get::<MovingSolid>(node).unwrap().solid;
    assert!(!solid, "removed: movement passes it");
    let shot = sim.app.world().get::<ColliderDisabled>(node).is_some()
        || sim
            .app
            .world()
            .get::<Children>(node)
            .is_some_and(|c| c.iter().any(|c| sim.app.world().get::<ColliderDisabled>(c).is_some()));
    assert!(shot, "and shots pass it");
    crate::stack::core::restart_round(sim.app.world_mut());
    sim.ticks(1);
    assert!(
        sim.app.world().get::<MovingSolid>(node).unwrap().solid,
        "back next round"
    );
}

#[test]
fn de_prodigy_lights_switch_their_lightmap_style() {
    let Some(map) = load("de_prodigy") else { return };
    let l = map.lightmap.as_ref().expect("lightmap");
    let styles: Vec<(u8, bool, usize)> = l.styles.iter().map(|s| (s.style, s.on, s.texels.len())).collect();
    eprintln!("de_prodigy switchable styles: {styles:?}");
    let s32 = l
        .styles
        .iter()
        .find(|s| s.style == 32)
        .expect("style 32 (bomb site B's lights)");
    assert!(s32.on && !s32.texels.is_empty());
    // As at map start: unchanged; style 32 off: darker where it lit.
    let (same, _) = l.relit(&|s| s.start());
    assert_eq!(same, l.rgb);
    let (off, _) = l.relit(&|s| if s.style == 32 { 0.0 } else { s.start() });
    let t = s32
        .rgb
        .iter()
        .position(|v| v[0] + v[1] + v[2] > 0.05)
        .expect("style 32 lights something");
    let at = s32.texels[t] as usize;
    let sum = |v: [f32; 3]| v[0] + v[1] + v[2];
    assert!(sum(off[at]) < sum(l.rgb[at]) - 0.04, "{:?} -> {:?}", l.rgb[at], off[at]);

    let mut sim = sim(map);
    let lit = |sim: &Sim, s: u8| {
        sim.app
            .world()
            .get_resource::<LightStyles>()
            .and_then(|l| l.styles.iter().find(|x| x.0 == s).map(|x| x.1))
    };
    assert_eq!(lit(&sim, 32), Some(true));
    input(&mut sim, "bombeffectsblights", "TurnOff", Value::Void);
    sim.ticks(1);
    assert_eq!(lit(&sim, 32), Some(false));
}

#[test]
fn de_aztec_soundscapes_follow_the_touch_code() {
    let Some(map) = load("de_aztec") else { return };
    let zones = &map.sounds.soundscape_zones;
    assert!(!zones.is_empty() && zones.iter().all(|z| z.entity.is_some()));
    let index = zones[0].entity.unwrap();
    let e = map.entities[index].clone();
    let pts: Vec<Vec3> = e.hulls.iter().flat_map(|h| h.points.clone()).collect();
    let (lo, hi) = pts
        .iter()
        .fold((Vec3::MAX, Vec3::MIN), |(a, b), p| (a.min(*p), b.max(*p)));
    let centre = e.origin() + (lo + hi) / 2.0;
    let mut sim = sim(map);
    let p = sim.spawn_character(to_engine(centre), noclip::ID);
    sim.app.world_mut().entity_mut(p).insert(LocalPlayer);
    sim.ticks(2);
    let touches = sim.app.world().resource::<SoundscapeTouches>().clone();
    assert!(
        touches.0.as_ref().is_some_and(|t| t.contains(&index)),
        "inside trigger {index}: {touches:?}"
    );
}

/// Detail props (the BSP's detail lump): cs_militia's grass, 3025 "cross"
/// sprites (two blades each) and 8819 "tri" ones (three), fading out over
/// its env_detail_controller's 800-1024 units; de_inferno's detail bushes
/// are models, drawn as non-solid props with its controller's fade.
#[test]
fn detail_props_load() {
    let Some(map) = load("cs_militia") else { return };
    let d = map
        .source_fx().detail_props
        .as_ref()
        .expect("cs_militia detail sprites");
    assert_eq!(d.quads.len(), 3025 * 2 + 8819 * 3);
    assert_eq!(d.fade, Some((800.0 * 0.0254, 1024.0 * 0.0254)));
    assert!(d.quads.iter().all(|q| q.billboard.is_none()), "shapes are fixed blades");
    assert!(d.quads.iter().any(|q| q.sway > 0.0));
    // Lit: the sprites take their baked light, not black.
    assert!(d.quads.iter().filter(|q| q.light.iter().any(|c| *c > 0.05)).count() * 2 > d.quads.len());
    assert!(!crate::stack::render::detail::cell_meshes(d).is_empty());
    let Some(map) = load("de_inferno") else { return };
    assert!(map.source_fx().detail_props.is_none(), "models only");
    let fade = Some((1024.0 * 0.0254, 1536.0 * 0.0254));
    let bushes = map
        .props
        .iter()
        .filter(|p| p.detail && p.fade == fade && p.solid == crate::stack::map::PropSolid::None)
        .count();
    assert_eq!(bushes, 53, "its 53 detail bushes");
}

/// Slime splashes (specs/cs_source/impact_effects.md 11): the three
/// `slime_splash_0x` systems build from the install's water_impact.pcf for
/// a map with no info_particle_system (de_aztec: its loader reads only
/// that file), each with something to emit and draw.
#[test]
fn slime_splash_systems_load_from_the_install() {
    use cs_source::impact_effects::{SLIME_SPLASH_FILE, SLIME_SPLASHES};
    let Some(install) = LocalConfig::load().ok().and_then(|c| c.game_path_in(cs_source::GAME, cs_source::INFO.steam.as_ref())) else {
        eprintln!("skipping: no CS:S install configured");
        return;
    };
    if !chimera_core::mount::served::usable("cs_source", &install, "cstrike") {
        eprintln!("skipping: no CS:S install configured");
        return;
    }
    let mount = cs_source::mount::open(&install).expect("mount");
    let bytes = mount.read("maps/de_aztec.bsp").expect("de_aztec");
    let bsp = vbsp::Bsp::read(&bytes).expect("bsp");
    let mut materials = cs_source::material::MaterialLoader::new(&bsp, &mount, &[]);
    let mut data = MapData::default();
    cs_source::pcf::add_particle_systems(
        &bsp,
        &mut materials,
        &mut data,
        "de_aztec",
        &SLIME_SPLASHES,
        &[SLIME_SPLASH_FILE],
    );
    let ps = &data.particle_systems;
    assert!(ps.placed.is_empty());
    for name in SLIME_SPLASHES {
        let i = ps.find(name).unwrap_or_else(|| panic!("{name}: {:?}", data.warnings));
        // The system or its children emit and draw.
        let mut todo = vec![i];
        let (mut emits, mut draws) = (false, false);
        while let Some(i) = todo.pop() {
            let d = &ps.defs[i];
            emits |= !d.emitters.is_empty() || d.initial_particles > 0;
            draws |= d.material.is_some() && d.renderer != crate::stack::map::psys::Render::None;
            todo.extend(d.children.iter().map(|c| c.0));
        }
        eprintln!("{name}: emits {emits}, draws {draws}");
        assert!(emits && draws, "{name}");
    }
}

/// `tint_perc` (impact_effects.md 14): `slime_splash_03` (water_impact.pcf;
/// its parts 0.45 and 0.3 toward the light) takes the colour of the light
/// where it starts, sampled from de_aztec's light field: started at a
/// spawn, each particle sits its part's tint of the way from its unlit
/// colour toward that light (gamma space); a system with no tint would
/// not change.
#[test]
fn slime_splash_colours_take_the_light_where_they_start() {
    use crate::stack::{
        map::{ParticleRng, psys::Init},
        render::psys::{ControlPoint, Instance, point_light},
    };
    use cs_source::impact_effects::{SLIME_SPLASH_FILE, SLIME_SPLASHES};
    let Some(map) = load("de_aztec") else { return };
    let install = LocalConfig::load()
        .ok()
        .and_then(|c| c.game_path_in(cs_source::GAME, cs_source::INFO.steam.as_ref()))
        .expect("install");
    let mount = cs_source::mount::open(&install).expect("mount");
    let bytes = mount.read("maps/de_aztec.bsp").expect("de_aztec");
    let bsp = vbsp::Bsp::read(&bytes).expect("bsp");
    let mut materials = cs_source::material::MaterialLoader::new(&bsp, &mount, &[]);
    let mut data = MapData::default();
    cs_source::pcf::add_particle_systems(
        &bsp,
        &mut materials,
        &mut data,
        "de_aztec",
        &SLIME_SPLASHES,
        &[SLIME_SPLASH_FILE],
    );
    let ps = &data.particle_systems;
    let system = ps.find("slime_splash_03").expect("slime_splash_03");
    // The tints of it and its children, as read from the file.
    let mut tints = Vec::new();
    let mut todo = vec![system];
    while let Some(i) = todo.pop() {
        for init in &ps.defs[i].initializers {
            if let Init::Color { tint, .. } = init {
                tints.push(*tint);
            }
        }
        todo.extend(ps.defs[i].children.iter().map(|c| c.0));
    }
    eprintln!("tints {tints:?}");
    assert!(tints.iter().any(|t| (t - 0.45).abs() < 1e-4), "{tints:?}");
    // The light 32 units above a spawn.
    let field = map.light_field.as_ref().expect("de_aztec's light field");
    let at = map.spawns[0].0 + Vec3::Y * 32.0 * cs_source::bsp::METERS_PER_UNIT;
    let light = point_light(&(field.0)(at));
    eprintln!("light at {at}: {light}");
    assert!(light.max_element() > 0.05 && light.min_element() < 1.0, "{light}");
    let run = |light: Vec3| {
        let cps = [ControlPoint { light, ..default() }];
        let mut i = Instance::new(&ps.defs, system);
        let mut rng = ParticleRng::new(7);
        for _ in 0..5 {
            i.step(&ps.defs, &cps, 0.02, &mut rng, &|_: usize, _: u32| 1);
        }
        i.colors()
    };
    let (lit, dark) = (run(light), run(Vec3::ZERO));
    assert!(!lit.is_empty());
    assert_eq!(lit.len(), dark.len());
    let gamma = light.powf(1.0 / 2.2);
    let mut tinted = 0;
    for (l, d) in lit.iter().zip(&dark) {
        // l = c + t (g - c), d = c - t c: l - d = t g for the part's t.
        let diff = *l - *d;
        let t = tints
            .iter()
            .find(|t| diff.abs_diff_eq(gamma * **t, 1e-4))
            .unwrap_or_else(|| panic!("{l} vs {d}: not a part's tint toward {gamma}"));
        if *t > 0.0 {
            tinted += 1;
        }
    }
    assert!(tinted > 0, "some particles tinted");
}
