//! Breakables on real CS:S maps, headless (specs/source/breakables.md):
//! de_nuke's vents (func_breakable, material Metal, health 1) break from
//! one bullet or a knife hit and stop blocking shots; cs_office's windows
//! (func_breakable_surf) lose the panes that are shot, and bullets pass
//! through the holes; a round restart makes both whole again. Skipped
//! without an install.

use avian3d::prelude::*;
use bevy::{ecs::system::SystemState, prelude::*};
use crate::stack::{
    core::MovingSolid,
    games::{
        self, cs_source,
        cs_source::{
            movement::{SourceMovementPlugin, to_engine, to_source},
            weapons::{AK47, CsWeaponsPlugin, KNIFE},
        },
    },
    harness::{CsSim, Sim},
    logic::{Logic, classes::Class},
    map::{BrushPanes, MapBrushEntity, MapData, MapEntity, MapPlugin},
    mount::config::LocalConfig,
    movement::noclip,
    weapon::{Inventory, Weapon, give},
};

fn load(name: &str) -> Option<MapData> {
    let installed = LocalConfig::load()
        .ok()?
        .game_path_in(cs_source::GAME, cs_source::INFO.steam.as_ref())
        .is_some_and(|p| p.join("cstrike").is_dir());
    if !installed {
        eprintln!("skipping: no CS:S install configured");
        return None;
    }
    Some(games::load_map_lean(&format!("cs_source:{name}")).expect("load map"))
}

/// World bounds (Source units) of a brush entity.
fn bounds(e: &MapEntity) -> (Vec3, Vec3) {
    let rot = crate::stack::map::entities::entity_rotation(e.angles());
    let pts = e.hulls.iter().flat_map(|h| &h.points).map(|p| rot * *p + e.origin());
    pts.fold((Vec3::MAX, Vec3::MIN), |(lo, hi), p| (lo.min(p), hi.max(p)))
}

fn sim(map: MapData) -> Sim {
    let mut sim = Sim::new(CsSim, (MapPlugin::new(map), SourceMovementPlugin, CsWeaponsPlugin));
    sim.set_tick_interval(cs_source::TICK_INTERVAL);
    sim
}

/// A floating shooter whose eye is at `eye` (Source units) holding `weapon`,
/// drawn and looking at `target`.
fn shooter(sim: &mut Sim, eye: Vec3, target: Vec3, weapon: &str) -> Entity {
    let p = sim.spawn_character(to_engine(eye), noclip::ID);
    sim.ticks(1);
    let offset = sim.state(p).eye_offset;
    sim.app.world_mut().get_mut::<Transform>(p).unwrap().translation = to_engine(eye) - offset;
    give(sim.app.world_mut(), p, weapon).unwrap();
    sim.seconds(1.5);
    let active = sim.app.world().get::<Inventory>(p).unwrap().active.unwrap();
    assert_eq!(sim.app.world().get::<Weapon>(active).unwrap().id, weapon);
    aim(sim, p, target);
    p
}

fn aim(sim: &mut Sim, p: Entity, target: Vec3) {
    let eye = sim.position(p) + sim.state(p).eye_offset;
    let d = to_engine(target) - eye;
    let mut i = sim.intent(p);
    i.yaw = (-d.x).atan2(-d.z);
    i.pitch = d.y.atan2(d.xz().length());
}

fn attack(sim: &mut Sim, p: Entity, secondary: bool) {
    if secondary {
        sim.intent(p).secondary = true;
    } else {
        sim.intent(p).fire = true;
    }
    sim.ticks(1);
    sim.intent(p).fire = false;
    sim.intent(p).secondary = false;
    sim.ticks(2);
}

/// The logic class of map entity `index`, if it still exists.
fn class(sim: &Sim, index: usize) -> Option<Class> {
    let logic = sim.app.world().resource::<Logic>();
    let id = logic
        .world
        .ids()
        .into_iter()
        .find(|id| logic.world.get(*id).unwrap().map_index == Some(index))?;
    Some(logic.world.get(id).unwrap().class.clone())
}

fn broken(sim: &Sim, index: usize) -> bool {
    match class(sim, index) {
        None => true,
        Some(Class::Breakable(b)) => b.broken,
        Some(_) => panic!("not a breakable"),
    }
}

fn node(sim: &mut Sim, index: usize) -> Option<Entity> {
    let world = sim.app.world_mut();
    world
        .query::<(Entity, &MapBrushEntity)>()
        .iter(world)
        .find(|(_, n)| n.0 == index)
        .map(|(e, _)| e)
}

/// How far (Source units) a ray from `from` toward `to` gets, characters
/// left out.
fn ray(sim: &mut Sim, from: Vec3, to: Vec3, skip: Entity) -> f32 {
    let world = sim.app.world_mut();
    let mut state = SystemState::<SpatialQuery>::new(world);
    let q = state.get(world).unwrap();
    let (a, b) = (to_engine(from), to_engine(to));
    let dir = Dir3::new(b - a).unwrap();
    let filter = SpatialQueryFilter::from_excluded_entities([skip]);
    q.cast_ray(a, dir, 100.0, true, &filter)
        .map_or(f32::MAX, |h| h.distance / 0.0254)
}

/// The vents: 5 func_breakable of material 2, health 1.
fn vents(map: &MapData) -> Vec<usize> {
    map.entities
        .iter()
        .enumerate()
        .filter(|(_, e)| e.classname() == "func_breakable" && e.get("material") == Some("2"))
        .map(|(i, _)| i)
        .collect()
}

/// Eye points in front of each face of a thin brush, `dist` units out.
fn approaches(e: &MapEntity, dist: f32) -> (Vec3, [Vec3; 2]) {
    let (lo, hi) = bounds(e);
    let (size, centre) = (hi - lo, (lo + hi) / 2.0);
    let thin = if size.x < size.y { Vec3::X } else { Vec3::Y };
    let out = thin * (size.dot(thin) / 2.0 + dist);
    (centre, [centre + out, centre - out])
}

#[test]
fn nuke_vents_break_from_a_bullet_and_a_knife() {
    let Some(map) = load("de_nuke") else { return };
    let vents = vents(&map);
    assert_eq!(vents.len(), 5, "de_nuke has 5 metal vents");
    for &i in &vents {
        let e = &map.entities[i];
        assert!(e.mover && !e.hulls.is_empty(), "vent {i} has a node and volumes");
        assert_eq!(e.get("health"), Some("1"));
    }
    let entities = map.entities.clone();
    let mut sim = sim(map);
    for &i in &vents {
        assert!(!broken(&sim, i));
        assert!(node(&mut sim, i).is_some());
    }

    // Shoot the first vent from whichever side is open.
    let vent = vents[0];
    let (centre, sides) = approaches(&entities[vent], 24.0);
    let mut shot = false;
    for eye in sides {
        let p = shooter(&mut sim, eye, centre, AK47);
        let before = ray(&mut sim, eye, centre, p);
        if before > eye.distance(centre) + 8.0 {
            continue; // something else in the way, or nothing there
        }
        attack(&mut sim, p, false);
        assert!(broken(&sim, vent), "one bullet breaks the vent");
        let n = node(&mut sim, vent);
        if let Some(n) = n {
            assert!(
                sim.app.world().get::<ColliderDisabled>(n).is_some(),
                "shots pass at once"
            );
            assert!(!sim.app.world().get::<MovingSolid>(n).unwrap().solid, "players pass");
        }
        sim.ticks(8);
        assert!(class(&sim, vent).is_none(), "the vent is removed after 0.1 s");
        let n = node(&mut sim, vent).expect("its node stays for the next round");
        assert_eq!(sim.app.world().get::<Visibility>(n), Some(&Visibility::Hidden));
        assert!(
            ray(&mut sim, eye, centre, p) > before + 4.0,
            "the ray goes through the opening"
        );
        // A new round: the vent is back, whole, blocking shots again.
        crate::stack::core::restart_round(sim.app.world_mut());
        sim.ticks(2);
        assert!(!broken(&sim, vent), "whole again");
        assert_eq!(sim.app.world().get::<Visibility>(n), Some(&Visibility::Inherited));
        assert!(sim.app.world().get::<MovingSolid>(n).unwrap().solid);
        assert!((ray(&mut sim, eye, centre, p) - before).abs() < 1.0, "blocks the ray again");
        attack(&mut sim, p, false);
        assert!(broken(&sim, vent), "and breaks again");
        shot = true;
        break;
    }
    assert!(shot, "no open side to shoot vent {vent} from");

    // Knife the next one up close.
    let vent = vents[1];
    let (centre, sides) = approaches(&entities[vent], 20.0);
    let mut knifed = false;
    for eye in sides {
        let p = shooter(&mut sim, eye, centre, KNIFE);
        if ray(&mut sim, eye, centre, p) > eye.distance(centre) + 8.0 {
            continue;
        }
        attack(&mut sim, p, false);
        sim.ticks(5);
        assert!(broken(&sim, vent), "a knife slash breaks the vent");
        knifed = true;
        break;
    }
    assert!(knifed, "no open side to knife vent {vent} from");
}

#[test]
fn office_windows_lose_shot_panes() {
    let Some(map) = load("cs_office") else { return };
    let windows: Vec<usize> = map
        .entities
        .iter()
        .enumerate()
        .filter(|(_, e)| e.classname() == "func_breakable_surf")
        .map(|(i, _)| i)
        .collect();
    assert_eq!(windows.len(), 14, "cs_office has 14 windows");
    let entities = map.entities.clone();
    let mut sim = sim(map);
    let mut done = false;
    for &win in &windows {
        let (centre, sides) = approaches(&entities[win], 24.0);
        for eye in sides {
            let p = shooter(&mut sim, eye, centre, AK47);
            let before = ray(&mut sim, eye, centre, p);
            if before > eye.distance(centre) + 8.0 {
                continue;
            }
            attack(&mut sim, p, false);
            let Some(Class::Breakable(b)) = class(&sim, win) else {
                panic!("window gone")
            };
            let w = b.window.as_ref().unwrap();
            assert!(w.window_broken, "the first shot breaks the window");
            assert!(w.broken_count() >= 1, "and its pane");
            let n = node(&mut sim, win).unwrap();
            sim.ticks(1);
            assert!(
                !sim.app.world().get::<MovingSolid>(n).unwrap().solid,
                "players walk through"
            );
            assert!(sim.app.world().get::<BrushPanes>(n).is_some(), "drawn as panes");
            assert!(
                ray(&mut sim, eye, centre, p) > before + 4.0,
                "a second bullet along the same line passes the hole"
            );
            // A new round: the window is whole again.
            crate::stack::core::restart_round(sim.app.world_mut());
            sim.ticks(2);
            let Some(Class::Breakable(b)) = class(&sim, win) else {
                panic!("window not re-created")
            };
            assert!(!b.window.as_ref().unwrap().window_broken);
            assert!(sim.app.world().get::<BrushPanes>(n).is_none(), "drawn whole");
            assert!(sim.app.world().get::<MovingSolid>(n).unwrap().solid);
            assert!((ray(&mut sim, eye, centre, p) - before).abs() < 1.0, "blocks the ray again");
            done = true;
            break;
        }
        if done {
            break;
        }
    }
    assert!(done, "no window could be shot");
}

/// A shot cs_office window takes the spec's "GlassBreak" decal (not the
/// glass surface's own impact decal), on the window's node; the install's
/// decal script has both breakable glass groups.
#[test]
fn office_window_shots_leave_glass_break_decals() {
    use crate::stack::map::{DecalGroup, PlaceDecal};
    let Some(map) = load("cs_office") else { return };
    for group in ["glassbreak", "bulletproof"] {
        let g = map.kit.decals.groups.get(group);
        assert!(g.is_some_and(|v| !v.is_empty()), "decals_subrect.txt has {group}");
    }
    let windows: Vec<usize> = map
        .entities
        .iter()
        .enumerate()
        .filter(|(_, e)| e.classname() == "func_breakable_surf")
        .map(|(i, _)| i)
        .collect();
    let entities = map.entities.clone();
    let mut sim = sim(map);
    for &win in &windows {
        let (centre, sides) = approaches(&entities[win], 24.0);
        for eye in sides {
            let p = shooter(&mut sim, eye, centre, AK47);
            if ray(&mut sim, eye, centre, p) > eye.distance(centre) + 8.0 {
                continue;
            }
            let n = node(&mut sim, win).unwrap();
            let mut cursor = sim
                .app
                .world()
                .resource::<bevy::ecs::message::Messages<PlaceDecal>>()
                .get_cursor_current();
            let mut decals: Vec<PlaceDecal> = Vec::new();
            sim.intent(p).fire = true;
            for _ in 0..3 {
                sim.ticks(1);
                sim.intent(p).fire = false;
                let messages = sim.app.world().resource::<bevy::ecs::message::Messages<PlaceDecal>>();
                decals.extend(cursor.read(messages).cloned());
            }
            let on_window: Vec<&PlaceDecal> = decals
                .iter()
                .filter(|d| {
                    d.target.is_some_and(|t| {
                        t == n || sim.app.world().get::<ChildOf>(t).is_some_and(|c| c.parent() == n)
                    })
                })
                .collect();
            assert!(!on_window.is_empty(), "the shot marks the window: {decals:?}");
            assert!(
                on_window
                    .iter()
                    .all(|d| matches!(&d.group, DecalGroup::Named(g) if g == "GlassBreak")),
                "{on_window:?}"
            );
            return;
        }
    }
    panic!("no window could be shot");
}

/// de_piranesi's wooden func_breakables name propdata templates: number 1
/// (Wooden.Tiny: health 6 over the map's 30, Wooden.Base's multipliers
/// and WoodChunks) from the install's propdata.txt, which the logic's
/// breakable takes.
#[test]
fn piranesi_breakables_take_their_propdata_template() {
    use crate::stack::map::entities::{PROP_DAMAGE_KEY, PROP_GIBS_KEY, PROP_HEALTH_KEY};
    let Some(map) = load("de_piranesi") else { return };
    let tiny: Vec<usize> = map
        .entities
        .iter()
        .enumerate()
        .filter(|(_, e)| e.classname() == "func_breakable" && e.get("propdata") == Some("1"))
        .map(|(i, _)| i)
        .collect();
    assert!(!tiny.is_empty(), "de_piranesi has Wooden.Tiny breakables");
    let e = &map.entities[tiny[0]];
    assert_eq!(e.get("health"), Some("30"), "the map's own health");
    assert_eq!(e.get(PROP_HEALTH_KEY), Some("6"));
    assert_eq!(e.get(PROP_DAMAGE_KEY), Some("0.75 2 1.5"));
    assert_eq!(e.get(PROP_GIBS_KEY), Some("WoodChunks"));
    assert!(map.gibs.iter().any(|g| g.name == "WoodChunks" && !g.models.is_empty()));
    let sim = sim(map);
    let Some(Class::Breakable(b)) = class(&sim, tiny[0]) else {
        panic!("not a breakable")
    };
    assert_eq!((b.health, b.dmg), (6, [0.75, 2.0, 1.5]));
    assert!(b.damageable);
}

/// cs_office's windows load their broken looks: the face again in its
/// `$crackmaterial` (glass/offwndwb_break), and the glass edge pieces.
#[test]
fn office_windows_have_their_broken_looks() {
    use crate::stack::map::PaneLook;
    let Some(map) = load("cs_office") else { return };
    let windows: Vec<usize> = map
        .entities
        .iter()
        .enumerate()
        .filter(|(_, e)| e.classname() == "func_breakable_surf")
        .map(|(i, _)| i)
        .collect();
    for &win in &windows {
        let looks: Vec<_> = map.meshes.iter().filter(|m| m.entity == Some(win)).collect();
        let cracked: Vec<_> = looks.iter().filter(|m| m.pane_look == PaneLook::Cracked).collect();
        assert!(!cracked.is_empty(), "window {win} has a cracked look");
        assert!(cracked.iter().all(|m| m.texture.is_some() && m.material.contains("break")));
        let edges = looks.iter().filter(|m| matches!(m.pane_look, PaneLook::Edge(_))).count();
        assert_eq!(edges % 12, 0, "window {win}: the 12 glass edge materials per face");
        assert!(edges > 0);
    }
    // The falling pane piece's three bodies.
    let pieces = map
        .gibs
        .iter()
        .find(|g| g.name == crate::stack::map::breakables::PANE_PIECES)
        .expect("pane pieces");
    assert_eq!(pieces.models.len(), 3);
    assert!(pieces.models.iter().all(|m| !m.meshes.is_empty()));
}

/// cs_militia's broken windows (their `$crackmaterial` a cubemap patch of
/// the ShatteredGlass glass/glasswindowbreak070b) keep most of their
/// brightness in the house's shade: the cracked face and its edge pieces
/// are lit only `SHATTERED_GLASS_UNLIT_FACTOR` of the way toward full
/// brightness; the whole face (LightmappedGeneric) is lit.
#[test]
fn militia_cracked_glass_is_partly_unlit() {
    use crate::stack::map::PaneLook;
    use cs_source::material::SHATTERED_GLASS_UNLIT_FACTOR;
    let Some(map) = load("cs_militia") else { return };
    let windows: Vec<usize> = map
        .entities
        .iter()
        .enumerate()
        .filter(|(_, e)| e.classname() == "func_breakable_surf")
        .map(|(i, _)| i)
        .collect();
    assert!(!windows.is_empty());
    let looks: Vec<_> = map
        .meshes
        .iter()
        .filter(|m| m.entity.is_some_and(|e| windows.contains(&e)))
        .collect();
    let broken: Vec<_> = looks.iter().filter(|m| m.pane_look != PaneLook::Whole).collect();
    assert!(broken.iter().any(|m| m.material.contains("glasswindowbreak070b")));
    for m in &broken {
        assert_eq!(m.unlit_factor, SHATTERED_GLASS_UNLIT_FACTOR, "{}", m.material);
        assert!(!m.unlit);
    }
    for m in looks.iter().filter(|m| m.pane_look == PaneLook::Whole) {
        assert_eq!(m.unlit_factor, 0.0, "{}", m.material);
    }
}

/// The logic id of map entity `index`.
fn id_of(sim: &Sim, index: usize) -> crate::stack::logic::EntId {
    let logic = sim.app.world().resource::<Logic>();
    logic
        .world
        .ids()
        .into_iter()
        .find(|id| logic.world.get(*id).unwrap().map_index == Some(index))
        .expect("entity")
}

/// A cs_office window carved into lone panes: they collapse, about half
/// dropping a pane piece, a physics body that falls and shatters on the
/// floor (gone well before its 5 s life).
#[test]
fn office_window_panes_fall_and_shatter() {
    use crate::stack::map::breakables::PanePiece;
    let Some(map) = load("cs_office") else { return };
    let win = map
        .entities
        .iter()
        .position(|e| e.classname() == "func_breakable_surf")
        .unwrap();
    let mut sim = sim(map);
    sim.ticks(2);
    let id = id_of(&sim, win);
    let (cols, rows) = {
        let logic = sim.app.world().resource::<Logic>();
        let w = logic.world.window(id).unwrap();
        (w.cols, w.rows)
    };
    // Shatter every pane but every third one both ways (below the top
    // row, which its edge holds up), pane by pane.
    {
        let mut logic = sim.app.world_mut().resource_mut::<Logic>();
        for r in 0..rows {
            for c in 0..cols {
                if r % 3 == 1 && c % 3 == 1 && r + 1 < rows {
                    continue;
                }
                let at = Vec3::new((c as f32 + 0.5) / cols as f32, (r as f32 + 0.5) / rows as f32, 1.0);
                logic.world.deliver(
                    crate::stack::logic::Who::Ent(id),
                    "Shatter",
                    crate::stack::logic::Value::Vector(at),
                    None,
                    None,
                );
            }
        }
    }
    let mut most = 0;
    for _ in 0..20 {
        sim.ticks(1);
        let world = sim.app.world_mut();
        most = most.max(world.query::<&PanePiece>().iter(world).count());
    }
    let logic = sim.app.world().resource::<Logic>();
    let w = logic.world.window(id).unwrap();
    assert_eq!(w.broken_count(), cols * rows, "the lone panes fell");
    assert!(most > 0, "some dropped a piece");
    sim.seconds(1.0);
    let world = sim.app.world_mut();
    assert_eq!(world.query::<&PanePiece>().iter(world).count(), 0, "shattered on landing");
}

/// An explosion next to a cs_office window shatters every pane, and one
/// next to a de_nuke vent breaks it (blast damage reaches breakables).
#[test]
fn explosions_break_windows_and_vents() {
    let Some(map) = load("cs_office") else { return };
    let win = map
        .entities
        .iter()
        .position(|e| e.classname() == "func_breakable_surf")
        .unwrap();
    let (centre, sides) = approaches(&map.entities[win], 40.0);
    let mut office = sim(map);
    office.ticks(2);
    office.app.world_mut().write_message(crate::stack::core::Explosion {
        origin: to_engine(sides[0]),
        damage: 1.0,
        radius: 350.0 * 0.0254,
        attacker: None,
        inflictor: None,
        sound: None,
        weapon: None,
        round_scorch: false,
        through_walls: false,
        bell_sigma: None,
        armor_ratio: None,
    });
    office.ticks(3);
    let id = id_of(&office, win);
    let logic = office.app.world().resource::<Logic>();
    let w = logic.world.window(id).unwrap();
    assert!(w.window_broken, "the blast breaks the window ({centre})");
    assert_eq!(w.broken_count(), w.cols * w.rows, "and every pane");

    let Some(map) = load("de_nuke") else { return };
    let vent = vents(&map)[0];
    let (_, sides) = approaches(&map.entities[vent], 24.0);
    let mut nuke = sim(map);
    nuke.ticks(2);
    for side in sides {
        nuke.app.world_mut().write_message(crate::stack::core::Explosion {
            origin: to_engine(side),
            damage: 1.0,
            radius: 350.0 * 0.0254,
            attacker: None,
            inflictor: None,
            sound: None,
            weapon: None,
            round_scorch: false,
            through_walls: false,
            bell_sigma: None,
            armor_ratio: None,
        });
    }
    nuke.ticks(3);
    assert!(broken(&nuke, vent), "the blast breaks the vent");
}

/// How far (Source units, along the approach) a crouched player starting
/// `dist` units out on each open side of a vent walks toward its centre
/// in a second; the side's start and the distance.
fn walk_into(sim: &mut Sim, e: &MapEntity, dist: f32) -> Vec<(Vec3, f32)> {
    let (lo, _) = bounds(e);
    let (centre, sides) = approaches(e, dist);
    let mut out = Vec::new();
    for side in sides {
        let feet = Vec3::new(side.x, side.y, lo.z + 1.0);
        let p = sim.spawn_character(to_engine(feet + Vec3::Z * 36.0), cs_source::movement::ID);
        sim.intent(p).crouch = true;
        sim.ticks(40);
        let start = to_source(sim.position(p));
        if start.truncate().distance(side.truncate()) > 4.0 || start.z < lo.z - 8.0 {
            // No floor there (outside the duct), or pushed out: not an
            // approach.
            sim.app.world_mut().despawn(p);
            continue;
        }
        let d = (centre - side).truncate().normalize();
        {
            // Intent yaw 0 looks along Source +Y (yaw 90).
            let mut i = sim.intent(p);
            i.yaw = d.y.atan2(d.x) - std::f32::consts::FRAC_PI_2;
            i.move_axis = Vec2::Y;
        }
        sim.seconds(1.0);
        let end = to_source(sim.position(p));
        out.push((side, (end - start).truncate().dot(d)));
        sim.app.world_mut().despawn(p);
    }
    out
}

/// How far (Source units) a small box swept from each side of a vent
/// toward the other gets, as physics-query movement (and shots) see it.
fn sweep_through(sim: &mut Sim, e: &MapEntity) -> Vec<f32> {
    let (centre, sides) = approaches(e, 32.0);
    let world = sim.app.world_mut();
    let mut state = SystemState::<SpatialQuery>::new(world);
    let q = state.get(world).unwrap();
    let filter = SpatialQueryFilter::default().with_mask(crate::stack::core::CHARACTER_FILTER);
    sides
        .iter()
        .map(|side| {
            let (a, b) = (to_engine(*side), to_engine(centre));
            let dir = Dir3::new(b - a).unwrap();
            let config = ShapeCastConfig::from_max_distance(2.0 * (b - a).length());
            q.cast_shape(&Collider::cuboid(0.1, 0.1, 0.1), a, Quat::IDENTITY, dir, &config, &filter)
                .map_or(f32::MAX, |h| h.distance / 0.0254)
        })
        .collect()
}

#[test]
fn nuke_vents_block_players_again_after_a_restart() {
    let Some(map) = load("de_nuke") else { return };
    let vents = vents(&map);
    let entities = map.entities.clone();
    let mut sim = sim(map);
    sim.ticks(3);
    let blocked = |walked: &[(Vec3, f32)]| walked.iter().all(|(_, w)| *w < 20.0);
    let swept: Vec<Vec<f32>> = vents.iter().map(|&i| sweep_through(&mut sim, &entities[i])).collect();
    for &i in &vents {
        let w = walk_into(&mut sim, &entities[i], 32.0);
        eprintln!("vent {i}: whole {w:?}");
        assert!(!w.is_empty(), "vent {i} has an approach");
        assert!(blocked(&w), "vent {i} blocks when whole: {w:?}");
    }
    sim.app.world_mut().resource_mut::<Logic>().world.queue_input(
        "func_breakable",
        "Break",
        crate::stack::logic::Value::Void,
        0.0,
        None,
    );
    sim.seconds(1.0);
    for &i in &vents {
        assert!(broken(&sim, i));
        let w = walk_into(&mut sim, &entities[i], 32.0);
        eprintln!("vent {i}: broken {w:?}");
        assert!(w.iter().any(|(_, w)| *w > 30.0), "vent {i} lets players through: {w:?}");
    }
    for round in 0..2 {
        crate::stack::core::restart_round(sim.app.world_mut());
        sim.ticks(3);
        for &i in &vents {
            assert!(!broken(&sim, i));
            let w = walk_into(&mut sim, &entities[i], 32.0);
            eprintln!("vent {i}: restored (round {round}) {w:?}");
            assert!(blocked(&w), "vent {i} blocks again after restart {round}: {w:?}");
        }
        for (k, &i) in vents.iter().enumerate() {
            let now = sweep_through(&mut sim, &entities[i]);
            eprintln!("vent {i}: swept {:?} whole, {now:?} restored", swept[k]);
            for (a, b) in swept[k].iter().zip(&now) {
                assert!((a - b).abs() < 1.0, "vent {i} collides again after restart {round}: {a} vs {b}");
            }
        }
    }
}

/// cs_havana's monitor: its screen (`breen_monitor_1_breakable_1`, a
/// func_breakable parented to the prop_physics_multiplayer
/// `breen_monitor_1_body_1`) stays on the monitor when it is knocked
/// flying and tumbles, and shots still hit it.
#[test]
fn havana_monitor_screen_rides_the_monitor() {
    let Some(map) = load("cs_havana") else { return };
    let find = |name: &str| {
        map.entities
            .iter()
            .position(|e| e.get("targetname") == Some(name))
            .unwrap_or_else(|| panic!("{name}"))
    };
    let (body, screen) = (find("breen_monitor_1_body_1"), find("breen_monitor_1_breakable_1"));
    let mut sim = sim(map);
    sim.seconds(1.0);
    let prop = {
        let world = sim.app.world_mut();
        world
            .query::<(Entity, &crate::stack::map::PropEntity)>()
            .iter(world)
            .find(|(_, p)| p.0 == body)
            .map(|(e, _)| e)
            .expect("monitor node")
    };
    let screen_node = node(&mut sim, screen).expect("screen node");
    // The screen's pose in the monitor's frame.
    let local = |sim: &Sim| {
        let w = sim.app.world();
        let p = w.get::<GlobalTransform>(prop).unwrap().compute_transform();
        let s = w.get::<Transform>(screen_node).unwrap();
        (
            p.rotation.inverse() * (s.translation - p.translation) / 0.0254,
            p.rotation.inverse() * s.rotation,
            to_source(p.translation),
        )
    };
    let (offset, turn, start) = local(&sim);
    {
        let world = sim.app.world_mut();
        world.entity_mut(prop).remove::<Sleeping>();
        world.get_mut::<LinearVelocity>(prop).unwrap().0 = to_engine(Vec3::new(0.0, 0.0, 300.0));
        world.get_mut::<AngularVelocity>(prop).unwrap().0 = Vec3::new(4.0, 6.0, 0.0);
    }
    for _ in 0..30 {
        sim.ticks(2);
        let (o, t, _) = local(&sim);
        assert!(o.distance(offset) < 0.5, "screen off the monitor: {o} (was {offset})");
        assert!(t.angle_between(turn) < 0.01, "screen turned apart from the monitor");
    }
    let (_, _, end) = local(&sim);
    assert!(end.distance(start) > 4.0, "the monitor moved: {start} -> {end}");
    assert!(!sim.app.world().entity(screen_node).contains::<ColliderDisabled>(), "shootable");
    assert!(!broken(&sim, screen));
}
