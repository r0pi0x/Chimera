//! Combat Arms' map conversion (`game::world::convert`) on a small world
//! built in code (no game data): a floor and a wall in the physics BSP, a
//! drawn triangle and a start point. Checks the axes (Z mirrored, units
//! to metres), the spawn on the floor facing its yaw, and a player that
//! walks to the wall and stops there, and that the wall stops shots. The
//! real maps: `heavy::world`.

use bevy::prelude::*;
use chimera_world::{
    harness::Sim,
    map::{BlendMode, DetailMode, MapSimPlugin},
};
use combat_arms::{
    game::{movement, world},
    harness::CaSim,
};
use lithtech_formats::world::{
    BspNode, BspPolygon, GeometryPolygon, Object, Plane, Property, RenderBlock, RenderVertex, Section, Surface,
    Triangle, Value, World, WorldModel, model_flags, shader, surface_flags,
};

/// A physics BSP of axis-aligned quads (corners in file axes, wound so
/// (v1 − v0) × (v2 − v0) runs along the normal).
fn physics(quads: &[([f32; 3], [[f32; 3]; 4])]) -> WorldModel {
    let mut m = WorldModel {
        flags: model_flags::MAIN_WORLD | model_flags::PHYSICS,
        name: "PhysicsBSP".into(),
        textures: vec!["Default".into()],
        surfaces: vec![Surface {
            flags: surface_flags::SOLID,
            ..default()
        }],
        root: -1,
        ..default()
    };
    for (n, corners) in quads {
        let d = n[0] * corners[0][0] + n[1] * corners[0][1] + n[2] * corners[0][2];
        m.planes.push(Plane { normal: *n, dist: d });
        let first = m.point_indices.len() as u32;
        for c in corners {
            m.point_indices.push(m.points.len() as u32);
            m.points.push(*c);
        }
        m.polygons.push(BspPolygon {
            surface: 0,
            plane: m.planes.len() as u32 - 1,
            first,
            count: 4,
        });
        m.nodes.push(BspNode {
            polygon: m.polygons.len() as u32 - 1,
            back: -2,
            front: -1,
        });
    }
    m
}

pub(crate) fn prop(name: &str, value: Value) -> Property {
    Property {
        name: name.into(),
        flags: 0,
        len: 0,
        value,
    }
}

/// A floor at y = 0 (±1000 u) and a wall across +Z at z = 500 facing the
/// origin, one drawn triangle, and a JSF start point 66 u up facing +Z.
pub(crate) fn small_world() -> World {
    let floor = (
        [0.0, 1.0, 0.0],
        [[-1000.0, 0.0, -1000.0], [-1000.0, 0.0, 1000.0], [1000.0, 0.0, 1000.0], [1000.0, 0.0, -1000.0]],
    );
    let wall = (
        [0.0, 0.0, -1.0],
        [[-1000.0, 0.0, 500.0], [-1000.0, 400.0, 500.0], [1000.0, 400.0, 500.0], [1000.0, 0.0, 500.0]],
    );
    let vertex = |pos: [f32; 3]| RenderVertex {
        pos,
        uv0: [pos[0] / 64.0, pos[2] / 64.0],
        color: 0xFF80_8080,
        normal: [0.0, 1.0, 0.0],
        ..default()
    };
    let mut w = World {
        models: vec![physics(&[floor, wall])],
        ..default()
    };
    w.render.blocks.push(RenderBlock {
        sections: vec![Section {
            textures: ["textures\\floor.dtx".into(), String::new()],
            shader: shader::GOURAUD,
            triangle_count: 1,
            ..default()
        }],
        vertices: vec![
            vertex([0.0, 0.0, 0.0]),
            vertex([0.0, 0.0, 100.0]),
            vertex([100.0, 0.0, 0.0]),
        ],
        triangles: vec![Triangle {
            indices: [0, 1, 2],
            polygon: 0,
        }],
        ..default()
    });
    w.objects.push(Object {
        class: "GameStartPoint".into(),
        properties: vec![
            prop("Name", Value::String("GameStartPoint00".into())),
            prop("Pos", Value::Vector([0.0, 66.0, 0.0])),
            prop("Rotation", Value::Rotation([0.0, 0.3, 0.0, 0.0])),
            prop("GameType", Value::String("Team Death Match".into())),
            prop("Teams", Value::String("JSF".into())),
        ],
    });
    w
}

#[test]
fn converts_axes_spawns_and_meshes() {
    let map = world::convert(&small_world(), "small", 0.01, &|_| None);
    assert_eq!(map.name, "combat_arms:small");
    // The drawn triangle: Z mirrored, metres, winding reversed so its
    // front (+Y) stays counter-clockwise from above.
    let m = &map.meshes[0];
    assert_eq!(m.positions, [[0.0, 0.0, 0.0], [0.0, 0.0, -1.0], [1.0, 0.0, 0.0]]);
    let [a, b, c] = [0, 1, 2].map(|k| Vec3::from(m.positions[m.indices[k] as usize]));
    assert!((b - a).cross(c - a).y > 0.0);
    assert_eq!(m.uvs[1], [0.0, 100.0 / 64.0]);
    assert_eq!(m.lightmap_uvs.len(), 3);
    assert!(map.lightmap.is_some());
    assert_eq!((m.texture, map.warnings.len()), (None, 1), "no texture file: a flat colour and a warning");
    // Collision: two quads, two triangles each.
    assert_eq!(map.collision_indices.len(), 4);
    // The start point, dropped to the floor, team JSF, its yaw negated.
    assert_eq!(map.spawns.len(), 1);
    let (feet, team) = map.spawns[0];
    assert!(feet.distance(Vec3::ZERO) < 1e-4, "{feet}");
    assert_eq!(team.map(|t| t.0), Some(1));
    assert_eq!(map.spawn_yaws, [-0.3]);
}

/// Spawned at the start point (yaw 0: +Z in file axes, −Z here), a
/// player stands on the floor, walks forward and stops at the wall 500 u
/// ahead.
#[test]
fn a_player_walks_to_the_wall_and_stops() {
    let mut w = small_world();
    w.objects[0].properties[2] = prop("Rotation", Value::Rotation([0.0; 4]));
    let s = movement::movement_data().meters_per_unit;
    let map = world::convert(&w, "small", s, &|_| None);
    let (feet, yaw) = (map.spawns[0].0, map.spawn_yaws[0]);
    let mut sim = Sim::new(CaSim, MapSimPlugin::new(map));
    sim.ticks(2);
    let p = sim.spawn_character(feet + Vec3::Y, movement::ID);
    sim.intent(p).yaw = yaw;
    sim.seconds(1.0);
    let rest = sim.position(p);
    assert!(rest.y > 0.0 && rest.y < 1.5 && sim.velocity(p).length() < 0.05, "standing at {rest}");
    sim.intent(p).move_axis = Vec2::new(0.0, 1.0);
    sim.seconds(5.0);
    let stopped = sim.position(p);
    assert!(stopped.z < -400.0 * s && stopped.z > -500.0 * s, "stopped at the wall (z = -500 u): {stopped}");
    assert!(stopped.x.abs() < 0.01 && (stopped.y - rest.y).abs() < 0.05, "{stopped}");
}

/// Half a second of rifle fire from the start point (yaw 0: down −Z, at
/// the wall 500 u away) at a target `ahead` world units away; with `wall`
/// false the wall's two collision triangles are left out. Whether it was
/// hurt.
fn shoot(ahead: f32, wall: bool) -> bool {
    let s = movement::movement_data().meters_per_unit;
    let mut map = world::convert(&small_world(), "small", s, &|_| None);
    if !wall {
        map.collision_indices.truncate(2);
    }
    let feet = map.spawns[0].0;
    let mut sim = Sim::new(CaSim, MapSimPlugin::new(map));
    sim.ticks(2);
    let shooter = sim.spawn_character(feet + Vec3::Y, movement::ID);
    let target = sim.spawn_character(feet + Vec3::new(0.0, 1.0, -ahead * s), movement::ID);
    // Settled, the rifle drawn.
    sim.seconds(1.5);
    let health = |sim: &Sim| sim.app.world().get::<chimera_core::core::Health>(target).unwrap().current;
    let full = health(&sim);
    // At the target's chest (hull centre): its head leans to the rifle,
    // off the line of the eyes (characters.md 11).
    {
        let d = sim.position(target) - (sim.position(shooter) + sim.state(shooter).eye_offset);
        sim.intent(shooter).pitch = d.y.atan2(d.xz().length());
    }
    sim.intent(shooter).fire = true;
    sim.seconds(0.5);
    sim.intent(shooter).fire = false;
    sim.ticks(2);
    health(&sim) < full
}

/// Shots hit the map: the rifle hurts a target in front of the wall, not
/// one behind it (which it does hurt without the wall).
#[test]
fn the_wall_stops_shots() {
    assert!(shoot(300.0, true), "the target in front of the wall took no damage");
    assert!(!shoot(700.0, true), "a shot went through the wall");
    assert!(shoot(700.0, false), "without the wall the target behind it is hit");
}

/// The small world with its wall's surface `flags` (non-solid, invisible)
/// or, with `blocker`, the wall as a collision blocker instead.
fn walled(flags: u32, blocker: bool) -> World {
    let mut w = small_world();
    let m = &mut w.models[0];
    m.surfaces.push(Surface { flags, ..default() });
    m.polygons[1].surface = 1;
    if blocker {
        let wall: Vec<[f32; 3]> = m.polygon_vertices(1).collect();
        m.polygons.truncate(1);
        m.nodes.truncate(1);
        w.blockers.polygons.push(GeometryPolygon {
            plane: Plane {
                normal: [0.0, 0.0, -1.0],
                dist: -500.0,
            },
            vertices: wall,
        });
    }
    w
}

/// world_dat.md cases 9 and 10 (Q5): which polygons stop what. A wall
/// without the solid bit stops shots (rays test every polygon) but not
/// players; a solid invisible one stops both; a collision blocker stops
/// players only.
#[test]
fn t09_t10_what_collides() {
    let s = movement::movement_data().meters_per_unit;
    for (what, w, shots, players) in [
        ("non-solid", walled(0, false), true, false),
        ("invisible", walled(surface_flags::SOLID | surface_flags::INVISIBLE, false), true, true),
        ("blocker", walled(0, true), false, true),
    ] {
        let map = world::convert(&w, "small", s, &|_| None);
        let sets = |players: bool, traces: bool| {
            map.collision_sets
                .iter()
                .filter(|c| c.characters == players && c.traces == traces)
                .map(|c| c.indices.len())
                .sum::<usize>()
        };
        let main = map.collision_indices.len();
        match (shots, players) {
            (true, false) => assert_eq!((main, sets(false, true)), (2, 2), "{what}"),
            (true, true) => assert_eq!(main, 4, "{what}"),
            _ => assert_eq!((main, sets(true, false)), (2, 2), "{what}"),
        }
    }
    // Shots: the non-solid wall stops them, the blocker doesn't.
    assert!(!shoot_world(walled(0, false), 700.0), "through non-solid detail");
    assert!(shoot_world(walled(0, true), 700.0), "a blocker stopped a shot");
    // Players: through the non-solid wall, stopped by the blocker.
    assert!(walk_world(walled(0, false)) < -500.0 * s, "stopped by non-solid detail");
    assert!(walk_world(walled(0, true)) > -500.0 * s, "through a blocker");
}

/// The small world with its wall of surface type `ty` (40 Glass, 41
/// Bullet-Proof Glass), solid or not.
fn glazed(ty: u16, solid: bool) -> World {
    let mut w = small_world();
    let m = &mut w.models[0];
    m.surfaces.push(Surface {
        flags: if solid { surface_flags::SOLID } else { 0 },
        surface_type: ty,
        ..default()
    });
    m.polygons[1].surface = 1;
    w
}

/// weapon_firing.md "Penetration" on a map's glass: a pane of Glass (40),
/// solid or ray-hit only, lets a round through to the target behind it;
/// Bullet-Proof Glass (41) stops it. Each glass type is a collision set of
/// its own named with its surface (`MapCollisionSet::surface`, what the
/// round reads), and sight passes both.
#[test]
fn glass_lets_rounds_through_and_bullet_proof_glass_stops_them() {
    let s = movement::movement_data().meters_per_unit;
    for (ty, solid, name, through) in [
        (40, true, "Glass", true),
        (40, false, "Glass", true),
        (41, true, "Bullet-Proof Glass", false),
    ] {
        let map = world::convert(&glazed(ty, solid), "small", s, &|_| None);
        let set = map
            .collision_sets
            .iter()
            .find(|c| !c.indices.is_empty() && c.see_through)
            .unwrap_or_else(|| panic!("{ty}: a glass set"));
        assert_eq!((set.surface.as_deref(), set.characters), (Some(name), solid), "{ty}");
        assert_eq!(shoot_world(glazed(ty, solid), 700.0), through, "{name} (solid {solid})");
    }
}

/// `shoot` on another world.
fn shoot_world(w: World, ahead: f32) -> bool {
    let s = movement::movement_data().meters_per_unit;
    let map = world::convert(&w, "small", s, &|_| None);
    let feet = map.spawns[0].0;
    let mut sim = Sim::new(CaSim, MapSimPlugin::new(map));
    sim.ticks(2);
    let shooter = sim.spawn_character(feet + Vec3::Y, movement::ID);
    let target = sim.spawn_character(feet + Vec3::new(0.0, 1.0, -ahead * s), movement::ID);
    sim.seconds(1.5);
    let health = |sim: &Sim| sim.app.world().get::<chimera_core::core::Health>(target).unwrap().current;
    let full = health(&sim);
    // At the target's chest (hull centre): its head leans to the rifle,
    // off the line of the eyes (characters.md 11).
    {
        let d = sim.position(target) - (sim.position(shooter) + sim.state(shooter).eye_offset);
        sim.intent(shooter).pitch = d.y.atan2(d.xz().length());
    }
    sim.intent(shooter).fire = true;
    sim.seconds(0.5);
    sim.intent(shooter).fire = false;
    sim.ticks(2);
    health(&sim) < full
}

/// A player walking 6 s from the start point toward the wall: where it
/// ends (z, m).
fn walk_world(mut w: World) -> f32 {
    w.objects[0].properties[2] = prop("Rotation", Value::Rotation([0.0; 4]));
    let s = movement::movement_data().meters_per_unit;
    let map = world::convert(&w, "small", s, &|_| None);
    let (feet, yaw) = (map.spawns[0].0, map.spawn_yaws[0]);
    let mut sim = Sim::new(CaSim, MapSimPlugin::new(map));
    sim.ticks(2);
    let p = sim.spawn_character(feet + Vec3::Y, movement::ID);
    sim.intent(p).yaw = yaw;
    sim.seconds(1.0);
    sim.intent(p).move_axis = Vec2::new(0.0, 1.0);
    sim.seconds(6.0);
    sim.position(p).z
}

/// A block with one lightmapped polygon: a shader-2 section (its
/// `lightmap`, `w` x `h` texels of RGB) and a shader-4 section over the
/// same three vertices, whose UV0 is the lightmap coordinate `lm`.
fn lit_block(lm: [[f32; 2]; 3], lightmap: Vec<u8>, w: u32, h: u32) -> RenderBlock {
    let pos = [[0.0, 0.0, 0.0], [100.0, 0.0, 0.0], [0.0, 0.0, 100.0]];
    let vertex = |i: usize| RenderVertex {
        pos: pos[i],
        uv0: lm[i],
        color: 0xFFFF_FFFF,
        normal: [0.0, 1.0, 0.0],
        ..default()
    };
    RenderBlock {
        sections: vec![
            Section {
                textures: ["".into(), String::new()],
                shader: shader::LIGHTMAP,
                triangle_count: 1,
                lightmap_width: w,
                lightmap_height: h,
                lightmap,
                ..default()
            },
            Section {
                textures: ["textures\\lit.dtx".into(), String::new()],
                shader: shader::LIGHTMAP_TEXTURE,
                triangle_count: 1,
                ..default()
            },
        ],
        vertices: (0..3).map(vertex).collect(),
        triangles: vec![
            Triangle {
                indices: [0, 1, 2],
                polygon: 0,
            },
            Triangle {
                indices: [0, 1, 2],
                polygon: 0,
            },
        ],
        ..default()
    }
}

/// world_dat.md case 8 (Q9): lightmap coordinates beyond 1 clamp per
/// pixel, not per vertex: the triangle is cut where u = 1 and the part
/// beyond reads the edge column; nothing is lost.
#[test]
fn t08_lightmap_clamps_per_pixel() {
    let mut w = small_world();
    w.render.blocks = vec![lit_block([[0.98, 0.5], [3.02, 0.5], [0.98, 0.6]], vec![128; 4 * 4 * 3], 4, 4)];
    let map = world::convert(&w, "small", 1.0, &|_| None);
    let m = map.meshes.iter().find(|m| m.material == "textures\\lit.dtx").expect("the lit mesh");
    // The cut: a vertex where u = 1 on the edge from u 0.98 to 3.02, at
    // x = 0.02 / 2.04 of the way (z mirrored).
    let cut = 100.0 * 0.02 / 2.04;
    assert!(
        m.positions.iter().any(|p| (p[0] - cut).abs() < 1e-3 && p[2].abs() < 1e-3),
        "{:?}",
        m.positions
    );
    // The area is all there.
    let area: f32 = m
        .indices
        .chunks(3)
        .map(|t| {
            let [a, b, c] = [0, 1, 2].map(|k| Vec3::from(m.positions[t[k] as usize]));
            (b - a).cross(c - a).length() / 2.0
        })
        .sum();
    assert!((area - 5000.0).abs() < 1e-2, "{area}");
    // Beyond the cut every vertex reads the last column: the same atlas u.
    let edge = m
        .positions
        .iter()
        .zip(&m.lightmap_uvs)
        .filter(|(p, _)| p[0] >= cut - 1e-3)
        .map(|(_, uv)| uv[0])
        .collect::<Vec<f32>>();
    assert!(edge.len() >= 3 && edge.iter().all(|u| (u - edge[0]).abs() < 1e-6), "{edge:?}");
}

/// "Lightmaps", How it is drawn: a texture-pass triangle takes the light of
/// the lightmap-pass triangle at the same place, whatever polygon index
/// either carries and whichever corner it starts at. Two triangles A and B
/// share an edge; the passes' polygon indices are crossed (as most of
/// JUNK_FLEA2's are) and A's lightmap triangle starts at another corner.
/// A's lightmap is 2 x 1 (left 200, right 100), B's one texel of 50. Before
/// the fix the shared corners took B's lightmap (by polygon and position):
/// hard-edged wedges of another surface's light.
#[test]
fn lightmap_pass_matched_by_place_not_polygon() {
    let p = [[0.0, 0.0, 0.0], [100.0, 0.0, 0.0], [0.0, 0.0, 100.0], [100.0, 0.0, 100.0]];
    // Section-own vertices (the packer never shares them between sections).
    let mut vertices = Vec::new();
    let mut tri = |corners: [(usize, [f32; 2]); 3], polygon: u32| {
        let at = vertices.len() as u32;
        for (i, uv0) in corners {
            vertices.push(RenderVertex {
                pos: p[i],
                uv0,
                color: 0xFFFF_FFFF,
                normal: [0.0, 1.0, 0.0],
                ..default()
            });
        }
        Triangle {
            indices: [at, at + 1, at + 2],
            polygon,
        }
    };
    // A = p0 p1 p2, B = p1 p3 p2 (same winding). Lightmap passes: A from p1
    // (u 0.75 at p1: the right texel; 0.25 at p0 and p2: the left).
    let triangles = vec![
        tri([(1, [0.75, 0.5]), (2, [0.25, 0.5]), (0, [0.25, 0.5])], 2),
        tri([(1, [0.5, 0.5]), (3, [0.5, 0.5]), (2, [0.5, 0.5])], 1),
        tri([(0, [0.0, 0.0]), (1, [0.0, 0.0]), (2, [0.0, 0.0])], 1),
        tri([(1, [0.0, 0.0]), (3, [0.0, 0.0]), (2, [0.0, 0.0])], 2),
    ];
    let lm_section = |lightmap: Vec<u8>, w: u32| Section {
        shader: shader::LIGHTMAP,
        triangle_count: 1,
        lightmap_width: w,
        lightmap_height: 1,
        lightmap,
        ..default()
    };
    let mut w = small_world();
    w.render.blocks = vec![RenderBlock {
        sections: vec![
            lm_section(vec![200, 200, 200, 100, 100, 100], 2),
            lm_section(vec![50; 3], 1),
            Section {
                textures: ["textures\\lit.dtx".into(), String::new()],
                shader: shader::LIGHTMAP_TEXTURE,
                triangle_count: 2,
                ..default()
            },
        ],
        vertices,
        triangles,
        ..default()
    }];
    let map = world::convert(&w, "small", 1.0, &|_| None);
    let lm = map.lightmap.as_ref().unwrap();
    let m = map.meshes.iter().find(|m| m.material == "textures\\lit.dtx").expect("the lit mesh");
    // The light a vertex reads (nearest texel) and where it is (file axes).
    let read = |v: u32| {
        let [u, t] = m.lightmap_uvs[v as usize];
        let (x, y) = ((u * lm.width as f32) as u32, (t * lm.height as f32) as u32);
        lm.rgb[(y * lm.width + x) as usize][0]
    };
    let at = |v: u32| {
        let q = m.positions[v as usize];
        p.iter().position(|c| (c[0] - q[0]).abs() < 1e-3 && (c[2] + q[2]).abs() < 1e-3).unwrap()
    };
    let light = |b: u8| world::light_value(b, 1.0);
    assert_eq!(m.indices.len(), 6);
    for t in m.indices.chunks(3) {
        let corners: Vec<usize> = t.iter().map(|&v| at(v)).collect();
        let a = corners.contains(&0);
        for &v in t {
            let want = match (a, at(v)) {
                (true, 1) => light(100),
                (true, _) => light(200),
                (false, _) => light(50),
            };
            assert!((read(v) - want).abs() < 1e-4, "triangle {corners:?}, corner {}: {} not {want}", at(v), read(v));
        }
    }
}

/// One polygon's lightmap on two pages (WAREHOUSE's wall at z = 588: the
/// upper triangle's on one shader-2 section, the lower one's on another).
/// The texture pass's two triangles share their block vertices; a corner
/// they share must not be shared in the drawn mesh, or one triangle reads
/// the other's page there and interpolates across the atlas (before the
/// fix: horizontal streaks over the wall).
#[test]
fn one_polygon_lightmapped_on_two_pages() {
    let p = [[0.0, 0.0, 0.0], [100.0, 0.0, 0.0], [0.0, 0.0, 100.0], [100.0, 0.0, 100.0]];
    let vertex = |i: usize, uv0: [f32; 2]| RenderVertex {
        pos: p[i],
        uv0,
        color: 0xFFFF_FFFF,
        normal: [0.0, 1.0, 0.0],
        ..default()
    };
    // Lightmap pass: A = p0 p1 p2 on page 1 (200), B = p1 p3 p2 on page 2
    // (50), each its own vertices; texture pass: both over the shared
    // vertices 6..9, one polygon.
    let mut vertices = vec![
        vertex(0, [0.5, 0.5]),
        vertex(1, [0.5, 0.5]),
        vertex(2, [0.5, 0.5]),
        vertex(1, [0.5, 0.5]),
        vertex(3, [0.5, 0.5]),
        vertex(2, [0.5, 0.5]),
    ];
    vertices.extend((0..4).map(|i| vertex(i, [0.0, 0.0])));
    let lm_section = |b: u8| Section {
        shader: shader::LIGHTMAP,
        triangle_count: 1,
        lightmap_width: 1,
        lightmap_height: 1,
        lightmap: vec![b; 3],
        ..default()
    };
    let mut w = small_world();
    w.render.blocks = vec![RenderBlock {
        sections: vec![
            lm_section(200),
            lm_section(50),
            Section {
                textures: ["textures\\lit.dtx".into(), String::new()],
                shader: shader::LIGHTMAP_TEXTURE,
                triangle_count: 2,
                ..default()
            },
        ],
        vertices,
        triangles: vec![
            Triangle {
                indices: [0, 1, 2],
                polygon: 1,
            },
            Triangle {
                indices: [3, 4, 5],
                polygon: 1,
            },
            Triangle {
                indices: [6, 7, 8],
                polygon: 7,
            },
            Triangle {
                indices: [7, 9, 8],
                polygon: 7,
            },
        ],
        ..default()
    }];
    let map = world::convert(&w, "small", 1.0, &|_| None);
    let lm = map.lightmap.as_ref().unwrap();
    let m = map.meshes.iter().find(|m| m.material == "textures\\lit.dtx").expect("the lit mesh");
    let read = |v: u32| {
        let [u, t] = m.lightmap_uvs[v as usize];
        let (x, y) = ((u * lm.width as f32) as u32, (t * lm.height as f32) as u32);
        lm.rgb[(y * lm.width + x) as usize][0]
    };
    let has_p0 = |t: &[u32]| {
        t.iter().any(|&v| {
            let q = m.positions[v as usize];
            q[0].abs() < 1e-3 && q[2].abs() < 1e-3
        })
    };
    assert_eq!(m.indices.len(), 6);
    for t in m.indices.chunks(3) {
        let want = world::light_value(if has_p0(t) { 200 } else { 50 }, 1.0);
        for &v in t {
            assert!((read(v) - want).abs() < 1e-4, "corner {:?}: {} not {want}", m.positions[v as usize], read(v));
        }
    }
}

/// world_dat.md case 11 (Q12): a light group adds ⌊colour × x⌋ to each
/// channel, at most 255; case 15 (Q7): the drawn light is twice the
/// lightmap (`Saturate`) times the lightmap factor on both passes.
#[test]
fn t11_t15_light_group_and_lighting_multiply() {
    let on = [1.0, 0.968_627_5, 0.901_960_8];
    assert_eq!(on.map(|c| world::add_light(100, c, 100)), [200, 196, 190]);
    assert_eq!(world::add_light(100, 128.0 / 255.0, 100), 150);
    assert_eq!(world::add_light(200, 1.0, 100), 255);
    // Texture 0.8 under lightmap 0.5: 0.8 (Saturate 1), in gamma space.
    let shown = |tex: f32, lm: u8, t: f32| {
        let lin = Color::srgb(tex, tex, tex).to_linear().red * world::light_value(lm, t);
        Color::LinearRgba(LinearRgba::rgb(lin, lin, lin)).to_srgba().red
    };
    assert!((shown(0.8, 128, 1.0) - 0.8).abs() < 0.01, "{}", shown(0.8, 128, 1.0));
    assert!((shown(0.8, 64, 1.0) - 0.4).abs() < 0.015, "{}", shown(0.8, 64, 1.0));
    assert!((shown(0.8, 128, 128.0 / 255.0) - 0.2016).abs() < 0.015, "{}", shown(0.8, 128, 128.0 / 255.0));
}

/// A volume's world model is drawn only when its object says `Visible`:
/// SHORTFUSE's `Filter` volumes store `Visible` as the real 0 (not a
/// bool), and read as bool-only they were drawn as flat slabs over the
/// doors beside them. A bool false, a real 0 and a long 0 all hide it;
/// a real 1 and no property show it.
#[test]
fn a_volume_with_visible_zero_is_not_drawn() {
    let drawn = |visible: Option<Value>| {
        let mut w = small_world();
        let mut m = physics(&[([0.0, 1.0, 0.0], [[0.0, 100.0, 0.0], [0.0, 100.0, 100.0], [100.0, 100.0, 100.0], [100.0, 100.0, 0.0]])]);
        m.flags = 0;
        m.name = "Room".into();
        w.models.push(m);
        let vertex = |pos: [f32; 3]| RenderVertex { pos, color: 0xFF80_8080, normal: [0.0, 1.0, 0.0], ..default() };
        w.render.models.push((
            "Room".into(),
            lithtech_formats::world::RenderWorld {
                blocks: vec![RenderBlock {
                    sections: vec![Section {
                        textures: ["textures\\flat.dtx".into(), String::new()],
                        shader: shader::GOURAUD,
                        triangle_count: 1,
                        ..default()
                    }],
                    vertices: vec![vertex([0.0, 100.0, 0.0]), vertex([0.0, 100.0, 100.0]), vertex([100.0, 100.0, 0.0])],
                    triangles: vec![Triangle { indices: [0, 1, 2], polygon: 0 }],
                    ..default()
                }],
                models: Vec::new(),
            },
        ));
        let mut properties = vec![prop("Name", Value::String("Room".into()))];
        properties.extend(visible.map(|v| prop("Visible", v)));
        w.objects.push(Object { class: "Filter".into(), properties });
        let map = world::convert(&w, "small", 0.01, &|_| None);
        map.meshes.iter().filter(|m| m.material.contains("flat")).count()
    };
    assert_eq!(drawn(None), 1, "no property: drawn");
    assert_eq!(drawn(Some(Value::Real(1.0))), 1);
    assert_eq!(drawn(Some(Value::Bool(false))), 0);
    assert_eq!(drawn(Some(Value::Real(0.0))), 0, "SHORTFUSE's Filter volumes");
    assert_eq!(drawn(Some(Value::Long(0.0))), 0);
}

/// world_dat.md case 12 (Q12) on a built world: a group whose
/// `LightGroup` object starts off adds nothing; one that starts on adds
/// its `StartColor`; without an object, the stored colour.
#[test]
fn t12_light_group_start_state() {
    let mut w = small_world();
    w.objects.push(Object {
        class: "LightGroup".into(),
        properties: vec![
            prop("Name", Value::String("Off".into())),
            prop("StartOn", Value::Bool(false)),
            prop("StartColor", Value::Color([255.0; 3])),
        ],
    });
    w.objects.push(Object {
        class: "LightGroup".into(),
        properties: vec![prop("Name", Value::String("On".into())), prop("StartColor", Value::Color([128.0; 3]))],
    });
    let colours = world::light_group_colours(&w);
    assert_eq!(colours("Off", [1.0; 3]), [0.0; 3]);
    assert_eq!(colours("On", [1.0; 3]), [128.0 / 255.0; 3]);
    assert_eq!(colours("Elsewhere", [0.5, 0.25, 1.0]), [0.5, 0.25, 1.0]);
}

/// world_dat.md case 17 (Q17): a section with a shader code other than
/// 1, 2, 4, 8 or 9 is kept and never drawn; case 20 (Q15): no file hash,
/// so the network handshake compares none.
#[test]
fn t17_t20_unknown_shader_and_no_hash() {
    let mut w = small_world();
    w.render.blocks[0].sections[0].shader = 3;
    let map = world::convert(&w, "small", 0.01, &|_| None);
    assert!(map.meshes.is_empty(), "not drawn");
    assert_eq!(map.file_hash, None);
}

/// world_dat.md case 22 (Q14): a `WorldModel` that is ray-hit but not
/// solid stops traces, not players; one that is neither is drawn only;
/// a solid one blocks both.
#[test]
fn t22_world_model_flags() {
    let s = 0.01;
    let with = |solid: bool, rayhit: bool| {
        let mut w = small_world();
        let mut door = physics(&[(
            [0.0, 0.0, -1.0],
            [[-50.0, 0.0, 300.0], [-50.0, 200.0, 300.0], [50.0, 200.0, 300.0], [50.0, 0.0, 300.0]],
        )]);
        door.flags = 0;
        door.name = "Door".into();
        w.models.push(door);
        w.objects.push(Object {
            class: "WorldModel".into(),
            properties: vec![
                prop("Name", Value::String("Door".into())),
                prop("Solid", Value::Bool(solid)),
                prop("Rayhit", Value::Bool(rayhit)),
            ],
        });
        let map = world::convert(&w, "small", s, &|_| None);
        let traces_only: usize = map
            .collision_sets
            .iter()
            .filter(|c| c.traces && !c.characters)
            .map(|c| c.indices.len())
            .sum();
        (map.collision_indices.len(), traces_only)
    };
    assert_eq!(with(true, true), (6, 0));
    assert_eq!(with(false, true), (4, 2));
    assert_eq!(with(false, false), (4, 0));
}

/// world_dat.md "Moving world models and volumes" (Q14) and grenades.md
/// section 7 (Q19): a `Water` volume reaches the game as a liquid
/// `CaVolume` (its `Viscosity`, `Current`); an `EndlessFall` volume kills
/// a player inside.
#[test]
fn volumes_from_the_map() {
    use combat_arms::game::volumes::CaVolume;
    let s = movement::movement_data().meters_per_unit;
    let volume = |class: &str, name: &str, props: Vec<Property>| {
        let mut m = physics(&[
            ([0.0, 1.0, 0.0], [[-100.0, 100.0, -100.0], [-100.0, 100.0, 100.0], [100.0, 100.0, 100.0], [100.0, 100.0, -100.0]]),
            ([0.0, -1.0, 0.0], [[-100.0, 0.0, -100.0], [100.0, 0.0, -100.0], [100.0, 0.0, 100.0], [-100.0, 0.0, 100.0]]),
            ([1.0, 0.0, 0.0], [[100.0, 0.0, -100.0], [100.0, 100.0, -100.0], [100.0, 100.0, 100.0], [100.0, 0.0, 100.0]]),
            ([-1.0, 0.0, 0.0], [[-100.0, 0.0, -100.0], [-100.0, 0.0, 100.0], [-100.0, 100.0, 100.0], [-100.0, 100.0, -100.0]]),
            ([0.0, 0.0, 1.0], [[-100.0, 0.0, 100.0], [100.0, 0.0, 100.0], [100.0, 100.0, 100.0], [-100.0, 100.0, 100.0]]),
            ([0.0, 0.0, -1.0], [[-100.0, 0.0, -100.0], [-100.0, 100.0, -100.0], [100.0, 100.0, -100.0], [100.0, 0.0, -100.0]]),
        ]);
        m.flags = 0;
        m.name = name.into();
        let mut properties = vec![prop("Name", Value::String(name.into()))];
        properties.extend(props);
        (m, Object { class: class.into(), properties })
    };
    let mut w = small_world();
    let (m, o) = volume(
        "Water",
        "Pond",
        vec![prop("Viscosity", Value::Real(0.1)), prop("Current", Value::Vector([0.0, 1500.0, 0.0]))],
    );
    w.models.push(m);
    w.objects.push(o);
    let map = world::convert(&w, "small", s, &|_| None);
    assert_eq!(map.collision_indices.len(), 4, "a volume doesn't collide");
    let mut sim = Sim::new(CaSim, MapSimPlugin::new(map));
    sim.ticks(3);
    let world = sim.app.world_mut();
    let vols: Vec<CaVolume> = world.query::<&CaVolume>().iter(world).cloned().collect();
    assert_eq!(vols.len(), 1);
    let v = &vols[0];
    assert!(v.liquid && (v.viscosity - 0.1).abs() < 1e-6 && v.current == Vec3::new(0.0, 1500.0, 0.0), "{v:?}");
    assert!(v.contains(Vec3::new(0.0, 50.0 * s, 0.0)) && !v.contains(Vec3::new(0.0, 150.0 * s, 0.0)));
    assert!(v.contains(Vec3::new(90.0 * s, 50.0 * s, -90.0 * s)) && !v.contains(Vec3::new(110.0 * s, 50.0 * s, 0.0)));

    // A pit that kills.
    let mut w = small_world();
    let (m, o) = volume("EndlessFall", "Pit", vec![]);
    w.models.push(m);
    w.objects.push(o);
    let map = world::convert(&w, "small", s, &|_| None);
    let feet = map.spawns[0].0;
    let mut sim = Sim::new(CaSim, MapSimPlugin::new(map));
    sim.ticks(2);
    let p = sim.spawn_character(feet + Vec3::Y, movement::ID);
    sim.seconds(1.0);
    let health = sim.app.world().get::<chimera_core::core::Health>(p).unwrap().current;
    assert!(health <= 0.0, "killed: {health}");
}

/// Glass (surface types 40 and 41, `world::GLASS_SURFACE_TYPES`) is
/// collision of its own marked Chimera's `SeeThrough`, solid to players as
/// the rest of the wall is (bots see through it, bots.md 5.2).
#[test]
fn glass_is_see_through() {
    let s = movement::movement_data().meters_per_unit;
    let mut w = small_world();
    let m = &mut w.models[0];
    m.surfaces.push(Surface {
        flags: surface_flags::SOLID,
        surface_type: 40,
        ..default()
    });
    m.polygons[1].surface = 1;
    let map = world::convert(&w, "small", s, &|_| None);
    assert_eq!(map.collision_indices.len(), 2, "the floor");
    let glass: Vec<_> = map.collision_sets.iter().filter(|c| c.see_through && !c.indices.is_empty()).collect();
    assert_eq!(glass.len(), 1);
    assert!(glass[0].characters && glass[0].traces && glass[0].indices.len() == 2, "{:?}", glass[0]);
    let mut sim = Sim::new(CaSim, MapSimPlugin::new(map));
    sim.ticks(2);
    let world = sim.app.world_mut();
    let marked = world
        .query_filtered::<(&Name, Option<&chimera_world::map::PropSurface>), With<chimera_world::map::SeeThrough>>()
        .iter(world)
        .map(|(n, s)| (n.to_string(), s.map(|s| s.0.clone())))
        .collect::<Vec<_>>();
    // Its collider names its surface (what a round reads there).
    assert_eq!(marked, [("Map collision (glass (Glass))".to_string(), Some("Glass".to_string()))]);
    assert!(walk_world(w) > -500.0 * s, "players stop at the glass");
}

/// The AI objects (volumes, patrol nodes) pass through as map entities,
/// every property as text, vectors in entity axes; other objects don't.
#[test]
fn ai_objects_pass_through() {
    use combat_arms::game::bots::map_ai::MapAi;
    let s = movement::movement_data().meters_per_unit;
    let mut w = small_world();
    w.objects.push(Object {
        class: "AIVolume".into(),
        properties: vec![
            prop("Name", Value::String("AIVolume01".into())),
            prop("Pos", Value::Vector([0.0, 32.0, 300.0])),
            prop("Dims", Value::Vector([200.0, 32.0, 100.0])),
            prop("PreferredPath", Value::Bool(false)),
        ],
    });
    for (name, z, next) in [("AINodePatrol_01", 250.0, "AINodePatrol_02"), ("AINodePatrol_02", 350.0, "")] {
        w.objects.push(Object {
            class: "AINodePatrol".into(),
            properties: vec![
                prop("Name", Value::String(name.into())),
                prop("Pos", Value::Vector([0.0, 16.0, z])),
                prop("Next", Value::String(next.into())),
                prop("StartDisabled", Value::Bool(false)),
            ],
        });
    }
    let map = world::convert(&w, "small", s, &|_| None);
    let classes: Vec<&str> = map.entities.iter().map(|e| e.classname()).collect();
    // The start point goes through too, for the game modes (`rules::objects`),
    // its feet on the floor and its yaw added.
    // The world file's offset goes through last, for the radar (`radar::WORLD_INFO`).
    assert_eq!(classes, ["GameStartPoint", "AIVolume", "AINodePatrol", "AINodePatrol", "WorldInfo"]);
    let start = &map.entities[0];
    assert!(start.get(world::FEET_KEY).is_some() && start.get(world::YAW_KEY).is_some());
    let objects = combat_arms::game::rules::objects::read(&map.entities, map.entity_scale);
    assert_eq!(objects.starts.len(), 1);
    let feet = objects.starts[0].at.translation - Vec3::Y * chimera_world::map::SPAWN_LIFT;
    assert!(feet.abs_diff_eq(map.spawns[0].0, 1e-4), "{feet} vs {}", map.spawns[0].0);
    let v = &map.entities[1];
    assert_eq!(v.get("targetname"), Some("AIVolume01"));
    assert_eq!(v.get("origin"), Some("0 300 32"));
    assert_eq!(v.get("PreferredPath"), Some("0"));
    assert_eq!(map.entity_scale, s);
    let ai = MapAi::read(&map.entities, map.entity_scale);
    assert_eq!(ai.volumes.len(), 1);
    // Z mirrored: the file's z 200..400 is engine z -400..-200 (u).
    assert!(ai.volumes[0].min.abs_diff_eq(Vec3::new(-200.0, 0.0, -400.0) * s, 1e-5), "{:?}", ai.volumes[0]);
    assert!(ai.volumes[0].max.abs_diff_eq(Vec3::new(200.0, 64.0, -200.0) * s, 1e-5), "{:?}", ai.volumes[0]);
    assert_eq!(ai.patrol.len(), 2);
    assert_eq!(ai.patrol[0].next, ["AINodePatrol_02"]);
    assert!(ai.patrol[1].pos.abs_diff_eq(Vec3::new(0.0, 16.0, -350.0) * s, 1e-5));
}

/// A 4 x 4 BGRA texture file whose texels are all `bgra`, with texture
/// flags `flags`, a command string and the detail fields (scale stored
/// minus 1, rotation in degrees): textures_dtx.md's header.
fn dtx_file(bgra: [u8; 4], flags: u32, command: &str, scale: f32, rotation: i16) -> Vec<u8> {
    let mut b = vec![0u8; 164];
    b[4..6].copy_from_slice(&4u16.to_le_bytes());
    b[6..8].copy_from_slice(&4u16.to_le_bytes());
    b[8..12].copy_from_slice(&(-5i32).to_le_bytes());
    b[0x0C..0x0E].copy_from_slice(&1u16.to_le_bytes());
    b[0x10..0x14].copy_from_slice(&(flags | 0x08).to_le_bytes());
    b[0x19] = 1;
    b[0x1A] = 3;
    b[0x1E..0x22].copy_from_slice(&(scale - 1.0).to_le_bytes());
    b[0x22..0x24].copy_from_slice(&rotation.to_le_bytes());
    b[0x24..0x24 + command.len()].copy_from_slice(command.as_bytes());
    b.extend((0..16).flat_map(|_| bgra));
    b
}

/// world_dat.md "Second textures" (Q13) on a built world: a dual-texture
/// section draws texture 1 at UV1 by the vertex alpha (added when texture
/// 1 is fullbright); a base texture's `detailtex` link lays the detail
/// texture over it at the base's scale and rotation, an `envmapalpha`
/// link a flat environment map weighted by the base alpha; a link that
/// doesn't resolve (textures_dtx.md case 22) leaves the surface without.
#[test]
fn t13_second_layers() {
    let vertex = |pos: [f32; 3], alpha: u8| RenderVertex {
        pos,
        uv0: [pos[0] / 64.0, pos[2] / 64.0],
        uv1: [pos[0] / 16.0 + 0.5, pos[2] / 16.0],
        color: (u32::from(alpha) << 24) | 0x80_8080,
        normal: [0.0, 1.0, 0.0],
        ..default()
    };
    let section = |t0: &str, t1: &str, shader| Section {
        textures: [t0.into(), t1.into()],
        shader,
        triangle_count: 1,
        ..default()
    };
    let tri = |k: u32| Triangle {
        indices: [3 * k, 3 * k + 1, 3 * k + 2],
        polygon: k,
    };
    let corners = |x: f32, alpha: [u8; 3]| {
        [[x, 0.0, 0.0], [x, 0.0, 100.0], [x + 100.0, 0.0, 0.0]]
            .into_iter()
            .zip(alpha)
            .map(|(p, a)| vertex(p, a))
            .collect::<Vec<_>>()
    };
    let mut w = small_world();
    w.render.blocks = vec![RenderBlock {
        sections: vec![
            section("textures\\a.dtx", "textures\\b.dtx", shader::DUAL_TEXTURE),
            section("textures\\a.dtx", "textures\\glow.dtx", shader::DUAL_TEXTURE),
            section("textures\\detailed.dtx", "", shader::GOURAUD),
            section("textures\\shiny.dtx", "", shader::GOURAUD),
            section("textures\\lost.dtx", "", shader::GOURAUD),
        ],
        vertices: [
            corners(0.0, [64, 255, 0]),
            corners(200.0, [128; 3]),
            corners(400.0, [255; 3]),
            corners(600.0, [255; 3]),
            corners(800.0, [255; 3]),
        ]
        .concat(),
        triangles: (0..5).map(tri).collect(),
        ..default()
    }];
    let file = |path: &str| -> Option<Vec<u8>> {
        Some(match path {
            "textures/a.dtx" => dtx_file([0, 0, 255, 255], 0, "", 1.0, 0),
            "textures/b.dtx" => dtx_file([255, 0, 0, 255], 0, "", 1.0, 0),
            "textures/glow.dtx" => dtx_file([255, 0, 0, 255], 1, "", 1.0, 0),
            "textures/detailed.dtx" => dtx_file([128; 4], 0, "detailtex textures\\Grit.dtx", 4.0, 90),
            "textures/grit.dtx" => dtx_file([100; 4], 0, "", 1.0, 0),
            "textures/shiny.dtx" => dtx_file([128; 4], 0, "envmapalpha textures\\Env.dtx", 1.0, 0),
            "textures/env.dtx" => dtx_file([200; 4], 0, "", 1.0, 0),
            "textures/lost.dtx" => dtx_file([128; 4], 0, "detailtex textures\\nowhere.dtx", 1.0, 0),
            _ => return None,
        })
    };
    let map = world::convert(&w, "small", 0.01, &file);
    let mesh = |name: &str, second: Option<&str>| {
        map.meshes
            .iter()
            .find(|m| {
                m.material == name && m.blend.and_then(|b| b.texture).map(|t| map.textures[t].name.as_str()) == second
            })
            .unwrap_or_else(|| panic!("{name} with {second:?}"))
    };
    // Dual texture: texture 1 at UV1, weight 1 − vertex alpha.
    let m = mesh("textures\\a.dtx", Some("textures\\b.dtx"));
    assert_eq!(m.blend.unwrap().mode, BlendMode::BaseAlpha);
    assert_eq!(m.uvs2.len(), m.positions.len());
    for (uv, uv2) in m.uvs.iter().zip(&m.uvs2) {
        // uv0 = x / 64, uv1 = x / 16 + 0.5 (file x; z likewise).
        assert!((uv2[0] - (uv[0] * 4.0 + 0.5)).abs() < 1e-5 && (uv2[1] - uv[1] * 4.0).abs() < 1e-5);
    }
    let mut weights = m.blend_weights.clone();
    weights.sort_by(f32::total_cmp);
    assert_eq!(weights, [0.0, 1.0 - 64.0 / 255.0, 1.0]);
    // A fullbright texture 1 is added.
    let m = mesh("textures\\a.dtx", Some("textures\\glow.dtx"));
    assert_eq!(m.blend.unwrap().mode, BlendMode::BaseAlphaAdd);
    assert!(m.blend_weights.iter().all(|w| (w - (1.0 - 128.0 / 255.0)).abs() < 1e-6));
    // Detail: the signed add at the base's scale 4 and rotation 90°.
    let d = mesh("textures\\detailed.dtx", None).detail.expect("a detail texture");
    assert_eq!((d.mode, d.scale), (DetailMode::SignedAdd, [4.0; 2]));
    assert!((d.rotation - std::f32::consts::FRAC_PI_2).abs() < 1e-6);
    assert_eq!(map.textures[d.texture].name, "textures\\Grit.dtx");
    // envmapalpha: a flat environment map weighted by the base alpha.
    let e = mesh("textures\\shiny.dtx", None).flat_envmap.expect("an environment map");
    assert!(e.by_base_alpha && e.signed_add && e.scale == 1.0);
    assert_eq!(map.textures[e.texture].name, "textures\\Env.dtx");
    // A link that doesn't resolve: nothing over the base.
    let m = mesh("textures\\lost.dtx", None);
    assert!(m.detail.is_none() && m.flat_envmap.is_none() && m.uvs2.is_empty());
}

/// world_dat.md case 16 and textures_dtx.md "Detail textures": the
/// second layers' arithmetic (gamma-space colours) the world shader runs,
/// on its CPU mirror (`chimera_world::map::material_fx`).
#[test]
fn t16_second_layer_maths() {
    use chimera_world::map::material_fx::{detail_uv, dual_texture, flat_envmap_uv, signed_add};
    let close = |a: Vec4, b: Vec4| (a - b).abs().max_element() < 1e-6;
    // Vertex alpha 0.25, texture 0 red with alpha 1, texture 1 blue.
    let base = dual_texture(Vec4::new(1.0, 0.0, 0.0, 1.0), Vec3::new(0.0, 0.0, 1.0), 0.25, false);
    assert!(close(base, Vec4::new(0.25, 0.0, 0.75, 0.25)), "{base}");
    // Shader 8 with vertex colour 0.5: tex x colour x 2 (Saturate).
    assert!(close((base.truncate() * 0.5 * 2.0).extend(0.25), base));
    // Texture 1 fullbright: added.
    let added = dual_texture(Vec4::new(1.0, 0.0, 0.0, 1.0), Vec3::new(0.0, 0.0, 1.0), 0.25, true);
    assert!(close(added, Vec4::new(1.0, 0.0, 0.25, 0.25)), "{added}");
    // The weight takes texture 0's alpha too.
    let half = dual_texture(Vec4::new(1.0, 0.0, 0.0, 0.5), Vec3::new(0.0, 0.0, 1.0), 1.0, false);
    assert!(close(half, Vec4::new(0.5, 0.0, 0.5, 0.5)), "{half}");
    // Detail: clamp(base + detail − 0.5); grey 0.5 leaves the base.
    let same = signed_add(Vec3::new(0.3, 0.8, 0.1), Vec3::splat(0.5));
    assert!((same - Vec3::new(0.3, 0.8, 0.1)).abs().max_element() < 1e-6, "{same}");
    let d = signed_add(Vec3::new(0.3, 0.8, 0.1), Vec3::new(0.7, 0.9, 0.2));
    assert!((d - Vec3::new(0.5, 1.0, 0.0)).abs().max_element() < 1e-6, "{d}");
    // Detail coordinates: σ R(θ) (u, v); scale 8 (stored 7), rotation 0.
    assert_eq!(detail_uv(Vec2::new(0.25, 0.5), 8.0, 0.0), Vec2::new(2.0, 4.0));
    let r = detail_uv(Vec2::new(1.0, 0.0), 2.0, 90f32.to_radians());
    assert!((r - Vec2::new(0.0, 2.0)).length() < 1e-6, "{r}");
    // Flat environment map: a reflection straight back reads the centre.
    assert_eq!(flat_envmap_uv(Vec3::Z, 1.0), Vec2::splat(0.5));
    assert_eq!(flat_envmap_uv(Vec3::X, 1.0), Vec2::new(0.0, 0.5));
}

/// game_modes.md S4: a map's spawn family is the team modes' when it has
/// a qualifying point on a team; else the family with the most
/// qualifying unlocked points (a Hunted map's `HungerGame`, whose one
/// `Team Death Match` point has no team), the first on a tie; none
/// without a qualifying point.
#[test]
fn map_spawn_family() {
    use combat_arms::game::world::map_family;
    let tdm = "Team Death Match";
    // (GameType, unlocked, Spawn is 1, on a team)
    let team_map = [(tdm, true, true, true), ("FreeForAll", true, true, false), ("FreeForAll", true, true, false)];
    assert_eq!(map_family(team_map.iter().copied()), Some(tdm));
    let hunted = [
        ("HungerGame", false, true, false),
        ("HungerGame", true, true, false),
        ("HungerGame", true, true, false),
        (tdm, true, true, false),
    ];
    assert_eq!(map_family(hunted.iter().copied()), Some("HungerGame"));
    let tie = [("Doobic", true, true, true), ("Capturetheflag", true, true, true)];
    assert_eq!(map_family(tie.iter().copied()), Some("Doobic"));
    let none = [(tdm, true, false, true), ("HungerGame", false, true, false)];
    assert_eq!(map_family(none.iter().copied()), None);
}

/// A mount of loose paths, for the map lister.
struct Fake(Vec<&'static str>);

impl chimera_core::mount::FileSource for Fake {
    fn name(&self) -> String {
        "fake".into()
    }
    fn entries(&self) -> Vec<chimera_core::mount::Entry> {
        self.0
            .iter()
            .map(|p| chimera_core::mount::Entry { path: p.to_string(), size: 1 })
            .collect()
    }
    fn read(&self, _: &str) -> Option<std::io::Result<Vec<u8>>> {
        None
    }
}

#[test]
fn the_game_lists_its_maps_for_the_console() {
    let mut mount = chimera_core::mount::Mount::default();
    mount.push(Fake(vec![
        "worlds2/warehouse.dat",
        "worlds/junk_flea.dat",
        "worlds/sub/deep.dat",
        "maps/de_dust2.bsp",
        "worlds2/readme.txt",
    ]));
    assert_eq!((combat_arms::game::INFO.map_names)(&mount), ["junk_flea", "warehouse"]);
    assert_eq!((combat_arms::game::INFO.map_file)("warehouse"), None, "by id and hash only: no file clients download");
}

/// `small_world` plus a lightmapped triangle (a texture pass and its
/// 2 x 1 lightmap pass) and light group `G` over both blocks: on the
/// vertex-lit triangle's first and third corners and on the lightmap's
/// texels; `LightGroup` object `G` starting on or not.
pub(crate) fn light_group_world(start_on: bool) -> World {
    use lithtech_formats::world::{BlockLightGroup, SubLightmap};
    let mut w = small_world();
    w.render.blocks[0].light_groups.push(BlockLightGroup {
        name: "G".into(),
        color: Vec3::new(1.0, 0.9, 0.8).into(),
        intensities: vec![100, 0, 50],
        sections: vec![vec![]],
    });
    let vertex = |pos: [f32; 3], uv: [f32; 2]| RenderVertex {
        pos,
        uv0: uv,
        color: 0xFF40_4040,
        normal: [0.0, 1.0, 0.0],
        ..default()
    };
    let corners = [[0.0, 0.0, 200.0], [0.0, 0.0, 300.0], [100.0, 0.0, 200.0]];
    let uvs = [[0.0, 0.0], [0.0, 1.0], [1.0, 0.0]];
    w.render.blocks.push(RenderBlock {
        sections: vec![
            Section {
                textures: ["textures\\floor.dtx".into(), String::new()],
                shader: shader::LIGHTMAP_TEXTURE,
                triangle_count: 1,
                ..default()
            },
            Section {
                shader: shader::LIGHTMAP,
                triangle_count: 1,
                lightmap_width: 2,
                lightmap_height: 1,
                lightmap: vec![60, 60, 60, 200, 100, 50],
                ..default()
            },
        ],
        vertices: (0..3).map(|k| vertex(corners[k], uvs[k])).chain((0..3).map(|k| vertex(corners[k], uvs[k]))).collect(),
        triangles: vec![
            Triangle {
                indices: [0, 1, 2],
                polygon: 0,
            },
            Triangle {
                indices: [3, 4, 5],
                polygon: 0,
            },
        ],
        light_groups: vec![BlockLightGroup {
            name: "G".into(),
            color: Vec3::ONE.into(),
            intensities: vec![0; 6],
            sections: vec![
                vec![],
                vec![SubLightmap {
                    left: 0,
                    top: 0,
                    width: 2,
                    height: 1,
                    intensities: vec![120, 255],
                }],
            ],
        }],
        ..default()
    });
    w.objects.push(Object {
        class: "LightGroup".into(),
        properties: vec![
            prop("Name", Value::String("G".into())),
            prop("StartOn", Value::Bool(start_on)),
            prop("StartColor", Value::Color([255.0; 3])),
        ],
    });
    w
}

/// world_dat.md "Light groups" (switching): a group a `LightGroup`
/// object names becomes a light style of the converted map, off or on as
/// the object starts, whose texels add exactly what switching it adds:
/// the map converted with the group off and relit with it on is the map
/// converted with it on (lightmap texels with their border copies,
/// vertex-lit corners), and the other way round; its object carries the
/// style number.
#[test]
fn light_group_switches_as_a_style() {
    use combat_arms::game::objects::runtime::keys;
    let off = world::convert(&light_group_world(false), "lg", 0.01, &|_| None);
    let on = world::convert(&light_group_world(true), "lg", 0.01, &|_| None);
    let (lo, ln) = (off.lightmap.as_ref().unwrap(), on.lightmap.as_ref().unwrap());
    assert_eq!((lo.width, lo.height), (ln.width, ln.height), "the same atlas");
    assert_eq!(lo.styles.len(), 1);
    let s = &lo.styles[0];
    assert_eq!((s.style, s.on), (32, false));
    assert_eq!(ln.styles[0].on, true);
    // 2 x 1 lightmap texels plus their borders (4 x 3), and the vertex-lit
    // triangle's 2 x 2 (4 x 4 with borders; its second corner adds
    // nothing, the fourth texel is the blend).
    assert!(s.texels.len() >= 12, "{}", s.texels.len());
    let (relit_on, _) = lo.relit(&|_| 1.0);
    let (relit_off, _) = ln.relit(&|_| 0.0);
    // (Relighting clamps at 0: a vertex-lit rectangle's fourth texel, the
    // corners' extrapolated blend, can be below.)
    let close = |a: &[[f32; 3]], b: &[[f32; 3]]| {
        a.iter().zip(b).all(|(x, y)| (0..3).all(|k| (x[k].max(0.0) - y[k].max(0.0)).abs() < 1e-5))
    };
    assert!(close(&relit_on, &ln.rgb), "off + switched on = converted on");
    assert!(close(&relit_off, &lo.rgb), "on + switched off = converted off");
    assert!(!close(&lo.rgb, &ln.rgb), "and it shows");
    let group = off.entities.iter().find(|e| e.classname() == "LightGroup").expect("the object");
    assert_eq!(group.get(keys::LIGHT_STYLE), Some("32"));
}

/// The object's messages switch the style (world_dat.md "Light groups":
/// `ON`, `OFF`, `TOGGLE`, `COLOR` changing the colour without switching
/// on; a colour black at 8 bits counts as off): `objects::lights::wanted`,
/// and in play `LightStyles`, which the map relights from.
#[test]
fn light_group_messages_switch_the_style() {
    use chimera_world::map::LightStyles;
    use combat_arms::game::objects::{CaObjectMessage, CaObjects, lights};
    let map = world::convert(&light_group_world(false), "lg", 0.01, &|_| None);
    let mut sim = Sim::new(CaSim, MapSimPlugin::new(map));
    sim.ticks(3);
    let wanted = |sim: &Sim| lights::wanted(&sim.app.world().resource::<CaObjects>().world);
    assert_eq!(wanted(&sim), [(32, false)]);
    let mut send = |sim: &mut Sim, m: &str| {
        sim.app.world_mut().write_message(CaObjectMessage {
            target: "G".into(),
            message: m.into(),
        });
        sim.ticks(4);
    };
    send(&mut sim, "on");
    assert_eq!(wanted(&sim), [(32, true)]);
    assert_eq!(sim.app.world().resource::<LightStyles>().styles, [(32, true)], "the map relights");
    send(&mut sim, "toggle");
    assert_eq!(wanted(&sim), [(32, false)]);
    send(&mut sim, "color 0 0 0");
    send(&mut sim, "toggle");
    assert_eq!(wanted(&sim), [(32, false)], "on, but black");
    send(&mut sim, "color -1 0.9 -1");
    assert_eq!(wanted(&sim), [(32, false)], "0.9/255 truncates to 0");
    send(&mut sim, "color 255");
    assert_eq!(wanted(&sim), [(32, true)]);
    send(&mut sim, "off");
    assert_eq!(sim.app.world().resource::<LightStyles>().styles, [(32, false)]);
}
