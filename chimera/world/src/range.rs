//! The range: a firing range built in code (map id `range`, also
//! `chimera:range`), the sandbox's map (Ultimate Gun Game's `+sandbox`).
//! Made so things are easy to see: a long, flat, light grey floor with a
//! grid, blue distance lines at 5, 10, 20, 40, 100 and 200 m from a
//! firing line where players spawn (each with its number on the floor and
//! on boards on both side walls), side walls with a few cover crates by
//! them (the middle lane clear for targets to strafe and walk), a shelf
//! for bottles and cans, a target board on a stand, a raised platform with
//! a ramp for long shots, a backstop past 200 m, a plain gradient sky and
//! an even, bright light. Our own content: plain shapes and colours.
//!
//! Floor labels: each distance has two, one reading from the firing line and
//! its half turn reading from the board's side (`floor_labels`).
//!
//! The layout is data, `data/range.toml` (compiled in, `LAYOUT_TOML`,
//! read by `data`): dimensions, distances, cover, the shelf, the board,
//! the platform, colours and light. Game-neutral like the greybox, whose
//! slot it shares (`map::code_maps::CodeMap` picks which code-built map
//! `greybox::respawn` and `GreyboxMapPlugin` spawn; `RangeMapPlugin` is the
//! range's for tests: `Sim::new(SimBasePlugins, RangeMapPlugin)`): its
//! parts carry `RangePart`, and `greybox::unload` takes either out.
//! Collision always (static boxes; plain floors, walls and boxes every
//! game's movement walks); meshes only when rendering (the `render`
//! feature and the assets): the markings (grid, lines, numbers, seams,
//! boards, rings) are one mesh with vertex colours and no collider.
//!
//! Bodies, held and view models: a code-built map has no game's kit of its
//! own; an app whose maps list `map::sources::CODE_MAP_KIT` loads
//! `chimera:range` beside it, an empty map its `MapSources` dresses as any
//! other game's (the party's: every game's bodies), so targets and bots
//! are drawn here (the greybox likewise).
//!
//! Players spawn at the firing line: its spawn points, also for a
//! minigame that spawns anywhere on the floor (`map::code_maps::
//! SpawnOnPoints`).
//!
//! `RangeLayout` (a resource while the range is loaded) says where things
//! are in engine space: the firing line's middle, the direction down
//! range, the distances, the spawns, the platform's top, the props' spots
//! and the board, for what places things on it (the sandbox's targets).
//! The range runs down -Z from the firing line at z = 0; +X is to the
//! right seen from there.
//!
//! Known shortcuts: the numbers are drawn with straight strokes (a
//! seven-segment style and an M), not a font; the props' spots are only
//! places (nothing stands there until a game's props are put on them).

use avian3d::prelude::*;
use bevy::prelude::*;
use serde::Deserialize;

pub use crate::map::code_maps::RANGE;
use crate::{
    core::{MapBrushes, MapWater, SpawnPoint},
    map::code_maps::{CodeMap, SpawnOnPoints},
};

/// The range's layout (module doc).
pub const LAYOUT_TOML: &str = include_str!("../data/range.toml");

/// The range for a test or tool (`GreyboxMapPlugin` with `CodeMap(RANGE)`
/// does the same in the apps).
pub struct RangeMapPlugin;

impl Plugin for RangeMapPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(CodeMap(RANGE)).add_systems(Startup, spawn);
    }
}

/// Everything the range spawned, so it can be removed (`unload`).
#[derive(Component)]
pub struct RangePart;

/// Remove the range (`greybox::unload` calls it).
pub fn unload(world: &mut World) {
    let parts: Vec<Entity> = world.query_filtered::<Entity, With<RangePart>>().iter(world).collect();
    for e in parts {
        if let Ok(e) = world.get_entity_mut(e) {
            e.despawn();
        }
    }
    world.remove_resource::<RangeLayout>();
    world.remove_resource::<SpawnOnPoints>();
}

/// Where the range's things are, engine space (module doc): inserted
/// with the range, removed with it (`greybox::unload`).
#[derive(Resource, Clone, Debug, PartialEq)]
pub struct RangeLayout {
    /// The firing line's middle, on the floor.
    pub firing_line: Vec3,
    /// Down range: flat, unit length (-Z).
    pub dir: Vec3,
    /// To the right seen from the firing line: flat, unit length (+X).
    pub right: Vec3,
    /// The distance lines, metres from the firing line along `dir`.
    pub distances: Vec<f32>,
    /// Half the floor's width (axis to a side wall's face), m.
    pub half_width: f32,
    /// Firing line to the backstop's face, m.
    pub length: f32,
    /// The spawn points (feet, 1 m up as every map's).
    pub spawns: Vec<Vec3>,
    /// The middle of the platform's top.
    pub platform_top: Vec3,
    /// Spots on the shelf's top for bottles and cans.
    pub prop_spots: Vec<Vec3>,
    /// The middle of the target board's face.
    pub board: Vec3,
    /// The board's face: width and height (m) and the way it faces (flat,
    /// unit: back toward the firing line). A shot is scored on this face,
    /// the painted board's whole size.
    pub board_size: Vec2,
    pub board_normal: Vec3,
    /// The shelf: its middle (on the floor), size (across, up, along) and
    /// the height of its top (m).
    pub shelf: Vec3,
    pub shelf_size: Vec3,
    pub shelf_top: f32,
    /// The platform: the middle of its footprint on the floor, its size
    /// (across, up, along), its ramp's width and slope (degrees).
    pub platform: Vec3,
    pub platform_size: Vec3,
    pub ramp_width: f32,
    pub ramp_degrees: f32,
    /// The cover crates (middle on the floor, size), in the order of the
    /// data's.
    pub cover: Vec<LayoutBox>,
    /// Half the clear lane's width (m): axis to the nearest crate's inner
    /// face (the half width where there are none).
    pub lane_half: f32,
    /// Half the distance stripes' length across the lane (m).
    pub stripe_half: f32,
}

/// A box of the layout: its middle on the floor and its size (across,
/// up, along; the range's frame, as the data's).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LayoutBox {
    pub centre: Vec3,
    pub size: Vec3,
}

impl LayoutBox {
    /// The box's extent across the range: (left, right) of the axis.
    pub fn across(&self) -> (f32, f32) {
        (self.centre.x - self.size.x / 2.0, self.centre.x + self.size.x / 2.0)
    }
}

impl RangeLayout {
    /// The point `along` m down range and `side` m to the right of the
    /// axis, on the floor.
    pub fn point(&self, along: f32, side: f32) -> Vec3 {
        self.firing_line + self.dir * along + self.right * side
    }

    /// The point on the axis `distance` m down range, on the floor.
    pub fn at_distance(&self, distance: f32) -> Vec3 {
        self.point(distance, 0.0)
    }

    pub fn of(d: &RangeData) -> Self {
        let p = &d.platform;
        let s = &d.shelf;
        let spots = s.spots.max(1);
        let cover: Vec<LayoutBox> = d
            .cover
            .iter()
            .map(|c| LayoutBox { centre: at(c.at[0], c.at[1], 0.0), size: Vec3::from(c.size) })
            .collect();
        let half_width = d.floor.width / 2.0;
        // The nearest a crate's face comes to the axis (a crate across it
        // closes the lane: zero).
        let lane_half = cover
            .iter()
            .map(|c| {
                let (l, r) = c.across();
                if l <= 0.0 && r >= 0.0 { 0.0 } else { l.abs().min(r.abs()) }
            })
            .fold(half_width, f32::min);
        Self {
            firing_line: Vec3::ZERO,
            dir: Vec3::NEG_Z,
            right: Vec3::X,
            distances: d.distances.at.clone(),
            half_width: d.floor.width / 2.0,
            length: d.floor.length,
            spawns: d.firing_line.spawns.iter().map(|[x, a]| at(*x, *a, 1.0)).collect(),
            platform_top: at(p.at[0], p.at[1], p.size[1]),
            prop_spots: (0..spots)
                .map(|i| {
                    let x = s.at[0] - s.size[0] / 2.0 + (i as f32 + 0.5) * s.size[0] / spots as f32;
                    at(x, s.at[1], s.size[1])
                })
                .collect(),
            board: at(d.board.at[0], d.board.at[1] - d.board.size[2] / 2.0, d.board.height),
            board_size: Vec2::new(d.board.size[0], d.board.size[1]),
            board_normal: Vec3::Z,
            shelf: at(s.at[0], s.at[1], 0.0),
            shelf_size: Vec3::from(s.size),
            shelf_top: s.size[1],
            platform: at(p.at[0], p.at[1], 0.0),
            platform_size: Vec3::from(p.size),
            ramp_width: p.ramp_width,
            ramp_degrees: p.ramp_degrees,
            cover,
            lane_half,
            stripe_half: d.distances.half,
        }
    }
}

/// `data/range.toml` (module doc).
#[derive(Deserialize, Clone, Debug)]
pub struct RangeData {
    pub floor: FloorData,
    pub walls: WallsData,
    pub firing_line: FiringLineData,
    pub distances: DistancesData,
    #[serde(default)]
    pub cover: Vec<CoverData>,
    pub shelf: ShelfData,
    pub board: BoardData,
    pub platform: PlatformData,
    pub light: LightData,
}

#[derive(Deserialize, Clone, Debug)]
pub struct FloorData {
    pub width: f32,
    pub behind: f32,
    pub length: f32,
    pub grid: f32,
    pub grid_line: f32,
    pub color: String,
    pub grid_color: String,
}

#[derive(Deserialize, Clone, Debug)]
pub struct WallsData {
    pub height: f32,
    pub thickness: f32,
    pub backstop_height: f32,
    pub panel: f32,
    pub color: String,
    pub seam_color: String,
    pub cover_color: String,
}

#[derive(Deserialize, Clone, Debug)]
pub struct FiringLineData {
    pub width: f32,
    pub color: String,
    pub spawns: Vec<[f32; 2]>,
    pub axis_width: f32,
}

#[derive(Deserialize, Clone, Debug)]
pub struct DistancesData {
    pub at: Vec<f32>,
    pub half: f32,
    pub width: f32,
    pub width_per_m: f32,
    pub color: String,
    pub floor_label: f32,
    pub floor_label_per_m: f32,
    pub floor_label_stretch: f32,
    pub label_color: String,
    pub sign: f32,
    pub sign_per_m: f32,
    pub sign_max: f32,
    pub sign_color: String,
    pub sign_digits: String,
}

#[derive(Deserialize, Clone, Debug)]
pub struct CoverData {
    pub at: [f32; 2],
    pub size: [f32; 3],
}

#[derive(Deserialize, Clone, Debug)]
pub struct ShelfData {
    pub at: [f32; 2],
    pub size: [f32; 3],
    pub spots: usize,
    pub color: String,
}

#[derive(Deserialize, Clone, Debug)]
pub struct BoardData {
    pub at: [f32; 2],
    pub size: [f32; 3],
    pub height: f32,
    pub rings: Vec<f32>,
    pub ring_colors: Vec<String>,
    pub stand_color: String,
}

#[derive(Deserialize, Clone, Debug)]
pub struct PlatformData {
    pub at: [f32; 2],
    pub size: [f32; 3],
    pub ramp_width: f32,
    pub ramp_degrees: f32,
    pub color: String,
}

#[derive(Deserialize, Clone, Debug)]
pub struct LightData {
    pub sun: f32,
    pub ambient: f32,
    pub sky_top: String,
    pub sky_horizon: String,
}

/// The layout (read at each spawn: a small file; a unit test reads it in
/// every build).
pub fn data() -> RangeData {
    toml::from_str(LAYOUT_TOML).expect("chimera_world's data/range.toml")
}

/// Engine space for (`side`, `along`) at height `y`.
fn at(side: f32, along: f32, y: f32) -> Vec3 {
    Vec3::new(side, y, -along)
}

fn color(hex: &str) -> Color {
    Color::Srgba(Srgba::hex(hex).unwrap_or(Srgba::WHITE))
}

/// A static box of the range.
struct Block {
    name: String,
    size: Vec3,
    transform: Transform,
    #[cfg_attr(not(feature = "render"), allow(dead_code))]
    color: Color,
}

fn blocks(d: &RangeData) -> Vec<Block> {
    let mut out = Vec::new();
    let mut block = |name: &str, size: Vec3, transform: Transform, c: &str| {
        out.push(Block {
            name: name.to_string(),
            size,
            transform,
            color: color(c),
        });
    };
    let f = &d.floor;
    let w = &d.walls;
    let (half, t) = (f.width / 2.0, w.thickness);
    let total = f.behind + f.length;
    let z_mid = (f.behind - f.length) / 2.0;
    block(
        "Range floor",
        Vec3::new(f.width + 2.0 * t, 1.0, total + 2.0 * t),
        Transform::from_xyz(0.0, -0.5, z_mid),
        &f.color,
    );
    for (name, size, pos, h) in [
        ("Range wall W", Vec3::new(t, w.height, total), Vec3::new(-half - t / 2.0, 0.0, z_mid), w.height),
        ("Range wall E", Vec3::new(t, w.height, total), Vec3::new(half + t / 2.0, 0.0, z_mid), w.height),
        (
            "Range wall S",
            Vec3::new(f.width + 2.0 * t, w.height, t),
            Vec3::new(0.0, 0.0, f.behind + t / 2.0),
            w.height,
        ),
        (
            "Range backstop",
            Vec3::new(f.width + 2.0 * t, w.backstop_height, t),
            Vec3::new(0.0, 0.0, -f.length - t / 2.0),
            w.backstop_height,
        ),
    ] {
        block(name, size, Transform::from_translation(pos + Vec3::Y * h / 2.0), &w.color);
    }
    for (i, c) in d.cover.iter().enumerate() {
        let size = Vec3::from(c.size);
        block(
            &format!("Range cover {i}"),
            size,
            Transform::from_translation(at(c.at[0], c.at[1], size.y / 2.0)),
            &w.cover_color,
        );
    }
    // The shelf: a bench whose top holds the props' spots.
    let s = &d.shelf;
    block(
        "Range shelf",
        Vec3::from(s.size),
        Transform::from_translation(at(s.at[0], s.at[1], s.size[1] / 2.0)),
        &s.color,
    );
    // The target board on two posts.
    let b = &d.board;
    let board = Vec3::from(b.size);
    block(
        "Range target board",
        board,
        Transform::from_translation(at(b.at[0], b.at[1], b.height)),
        b.ring_colors.first().map_or("#f2f2ee", String::as_str),
    );
    let post_h = b.height - board.y / 2.0;
    for (i, dx) in [-1.0, 1.0].into_iter().enumerate() {
        block(
            &format!("Range target post {i}"),
            Vec3::new(0.06, post_h, 0.06),
            Transform::from_translation(at(b.at[0] + dx * (board.x / 2.0 - 0.15), b.at[1] + 0.06, post_h / 2.0)),
            &b.stand_color,
        );
    }
    // The platform and its ramp, rising toward the platform's back.
    let p = &d.platform;
    let size = Vec3::from(p.size);
    block(
        "Range platform",
        size,
        Transform::from_translation(at(p.at[0], p.at[1], size.y / 2.0)),
        &p.color,
    );
    let angle = p.ramp_degrees.to_radians();
    let length = size.y / angle.sin();
    let back = -(p.at[1] - size.z / 2.0);
    block(
        "Range platform ramp",
        Vec3::new(p.ramp_width, 0.2, length),
        Transform::from_xyz(p.at[0], length / 2.0 * angle.sin(), back + length / 2.0 * angle.cos())
            .with_rotation(Quat::from_rotation_x(angle)),
        &p.color,
    );
    out
}

/// Spawn the range (`greybox::respawn` with `CodeMap(RANGE)`).
pub fn spawn(world: &mut World) {
    let d = &data();
    world.insert_resource(GlobalAmbientLight {
        brightness: d.light.ambient,
        ..default()
    });
    for b in blocks(d) {
        #[cfg_attr(not(feature = "render"), allow(unused_variables))]
        let e = world
            .spawn((
                Name::new(b.name),
                RangePart,
                b.transform,
                RigidBody::Static,
                Collider::cuboid(b.size.x, b.size.y, b.size.z),
            ))
            .id();
        #[cfg(feature = "render")]
        if let Some(look) = draw::look(
            world,
            Cuboid::from_size(b.size).into(),
            StandardMaterial {
                base_color: b.color,
                perceptual_roughness: 0.9,
                ..default()
            },
        ) {
            world.entity_mut(e).insert(look);
        }
    }
    #[cfg(feature = "render")]
    draw::markings_and_sky(world, d);
    world.insert_resource(MapBrushes(Vec::new()));
    world.insert_resource(MapWater(Vec::new()));
    world.spawn((
        Name::new("Range sun"),
        RangePart,
        DirectionalLight {
            illuminance: d.light.sun,
            shadow_maps_enabled: true,
            ..default()
        },
        // High, from behind the firing line: the faces toward it lit.
        Transform::default().looking_at(Vec3::new(0.35, -1.0, -0.45), Vec3::Y),
    ));
    let layout = RangeLayout::of(d);
    for (i, pos) in layout.spawns.iter().enumerate() {
        world.spawn((
            Name::new(format!("Range spawn {i}")),
            RangePart,
            SpawnPoint::default(),
            Transform::from_translation(*pos),
        ));
    }
    // Players spawn at the firing line facing down range, whatever the
    // minigame's spawning.
    world.insert_resource(SpawnOnPoints { facing: Some(layout.dir) });
    world.insert_resource(layout);
}

/// A floor label's place: its bottom-left corner, the way its text runs
/// (`right`, unit), the way its up runs (`up`, stretched down or up range
/// by `distances.floor_label_stretch`) and its character height.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FloorLabel {
    pub origin: Vec3,
    pub right: Vec3,
    pub up: Vec3,
    pub height: f32,
}

/// The floor labels of the `dist` m line, beside the stripe's two ends:
/// the right one reads from the firing line (up the text is down range),
/// the left one is that label turned half a turn about the stripe's
/// middle, so it reads from the board's side, looking back up range
/// (each label lies where its reader sees it at the stripe's right end).
///
/// The height grows with the distance but stops where the text would run
/// past the side wall (`floor_half`, the floor's half width): at 100 and
/// 200 m it would otherwise lie under the wall boards and outside the floor.
pub fn floor_labels(ds: &DistancesData, floor_half: f32, dist: f32) -> [FloorLabel; 2] {
    let wide = text_width(&format!("{} M", number(dist)));
    let room = (floor_half - ds.half - 0.5).max(0.1) / (wide + 0.4);
    let h = (ds.floor_label + ds.floor_label_per_m * dist).min(room);
    let deep = h * ds.floor_label_stretch;
    let up = Vec3::NEG_Z * ds.floor_label_stretch;
    let origin = at(ds.half + 0.4 * h, dist - deep / 2.0, 0.0);
    // The half turn about the stripe's middle (x = 0, z = -dist).
    let turned = Vec3::new(-origin.x, origin.y, -2.0 * dist - origin.z);
    [
        FloorLabel { origin, right: Vec3::X, up, height: h },
        FloorLabel { origin: turned, right: Vec3::NEG_X, up: -up, height: h },
    ]
}

/// The strokes of a character in a cell 0.6 wide and 1 tall (x right, y
/// up): digits seven-segment style, `M`; others none.
fn strokes(c: char) -> Vec<(Vec2, Vec2)> {
    let p = |x: f32, y: f32| Vec2::new(x, y);
    let seg = |s: char| match s {
        'a' => (p(0.0, 1.0), p(0.6, 1.0)),
        'b' => (p(0.6, 1.0), p(0.6, 0.5)),
        'c' => (p(0.6, 0.5), p(0.6, 0.0)),
        'd' => (p(0.0, 0.0), p(0.6, 0.0)),
        'e' => (p(0.0, 0.0), p(0.0, 0.5)),
        'f' => (p(0.0, 0.5), p(0.0, 1.0)),
        _ => (p(0.0, 0.5), p(0.6, 0.5)),
    };
    let segments = match c {
        '0' => "abcdef",
        '1' => "bc",
        '2' => "abged",
        '3' => "abgcd",
        '4' => "fgbc",
        '5' => "afgcd",
        '6' => "afgedc",
        '7' => "abc",
        '8' => "abcdefg",
        '9' => "abcdfg",
        'M' => {
            return vec![
                (p(0.0, 0.0), p(0.0, 1.0)),
                (p(0.0, 1.0), p(0.3, 0.45)),
                (p(0.3, 0.45), p(0.6, 1.0)),
                (p(0.6, 1.0), p(0.6, 0.0)),
            ];
        }
        _ => "",
    };
    segments.chars().map(seg).collect()
}

/// A text's width in character heights (`strokes`' cells, a quarter
/// height between them, a space 0.4).
fn text_width(text: &str) -> f32 {
    let n = text.chars().count();
    text.chars().map(|c| if c == ' ' { 0.4 } else { 0.6 }).sum::<f32>() + 0.25 * n.saturating_sub(1) as f32
}

/// A distance as its label's number (`40`, `7.5`).
fn number(d: f32) -> String {
    if d.fract() == 0.0 { format!("{}", d as i64) } else { format!("{d}") }
}

#[cfg(feature = "render")]
mod draw {
    use bevy::{
        asset::RenderAssetUsages,
        mesh::{Indices, VertexAttributeValues},
        render::render_resource::PrimitiveTopology,
    };

    use super::*;

    /// A mesh and a material added to the world's assets (none without
    /// them: a headless app).
    pub(super) fn look(world: &mut World, mesh: Mesh, material: StandardMaterial) -> Option<(Mesh3d, MeshMaterial3d<StandardMaterial>)> {
        if !world.contains_resource::<Assets<Mesh>>() || !world.contains_resource::<Assets<StandardMaterial>>() {
            return None;
        }
        let mesh = world.resource_mut::<Assets<Mesh>>().add(mesh);
        let material = world.resource_mut::<Assets<StandardMaterial>>().add(material);
        Some((Mesh3d(mesh), MeshMaterial3d(material)))
    }

    /// Triangles with a colour per vertex.
    #[derive(Default)]
    struct Batch {
        pos: Vec<[f32; 3]>,
        normal: Vec<[f32; 3]>,
        color: Vec<[f32; 4]>,
        index: Vec<u32>,
    }

    impl Batch {
        /// A rectangle: `u`, `v` its half sides, facing `normal`.
        fn quad(&mut self, center: Vec3, u: Vec3, v: Vec3, normal: Vec3, c: Color) {
            let (u, v) = if u.cross(v).dot(normal) >= 0.0 { (u, v) } else { (v, u) };
            let base = self.pos.len() as u32;
            let c = c.to_linear().to_f32_array();
            for p in [center - u - v, center + u - v, center + u + v, center - u + v] {
                self.pos.push(p.to_array());
                self.normal.push(normal.to_array());
                self.color.push(c);
            }
            self.index.extend([base, base + 1, base + 2, base, base + 2, base + 3]);
        }

        /// A rectangle on the floor, `y` up: `size` across (x) and along
        /// (z).
        fn floor(&mut self, center: Vec3, size: Vec2, c: Color) {
            self.quad(center, Vec3::X * size.x / 2.0, Vec3::Z * size.y / 2.0, Vec3::Y, c);
        }

        /// An axis-aligned box.
        fn cuboid(&mut self, center: Vec3, size: Vec3, c: Color) {
            let h = size / 2.0;
            for (n, u, v) in [
                (Vec3::X, Vec3::Y * h.y, Vec3::Z * h.z),
                (Vec3::NEG_X, Vec3::Y * h.y, Vec3::Z * h.z),
                (Vec3::Y, Vec3::X * h.x, Vec3::Z * h.z),
                (Vec3::NEG_Y, Vec3::X * h.x, Vec3::Z * h.z),
                (Vec3::Z, Vec3::X * h.x, Vec3::Y * h.y),
                (Vec3::NEG_Z, Vec3::X * h.x, Vec3::Y * h.y),
            ] {
                self.quad(center + n * h.dot(n.abs()), u, v, n, c);
            }
        }

        /// `text` from `origin` (its bottom left), `right` and `up` the
        /// plane's directions (`right` unit; a longer `up` stretches the
        /// characters that way), characters `height` tall.
        fn text(&mut self, text: &str, origin: Vec3, right: Vec3, up: Vec3, height: f32, c: Color) {
            let normal = right.cross(up).normalize();
            let thick = 0.13 * height;
            let mut x = 0.0;
            for ch in text.chars() {
                for (a, b) in strokes(ch) {
                    let (a, b) = (a * height, b * height);
                    let dir = (b - a).normalize_or_zero();
                    let mid = (a + b) / 2.0 + Vec2::X * x;
                    let half = (b - a).length() / 2.0 + thick / 2.0;
                    let u = (right * dir.x + up * dir.y) * half;
                    let v = (right * -dir.y + up * dir.x) * thick / 2.0;
                    self.quad(origin + right * mid.x + up * mid.y, u, v, normal, c);
                }
                x += if ch == ' ' { 0.4 } else { 0.6 } * height + 0.25 * height;
            }
        }

        /// A ring from `r0` to `r1` (a disc when `r0` is 0) in the plane
        /// of `right` and `up`, facing their cross product.
        fn ring(&mut self, center: Vec3, right: Vec3, up: Vec3, r0: f32, r1: f32, c: Color) {
            const N: usize = 48;
            let normal = right.cross(up).normalize();
            let point = |r: f32, k: usize| {
                let a = k as f32 / N as f32 * std::f32::consts::TAU;
                center + (right * a.cos() + up * a.sin()) * r
            };
            let c = c.to_linear().to_f32_array();
            for k in 0..N {
                let base = self.pos.len() as u32;
                for p in [point(r0, k), point(r1, k), point(r1, k + 1), point(r0, k + 1)] {
                    self.pos.push(p.to_array());
                    self.normal.push(normal.to_array());
                    self.color.push(c);
                }
                self.index.extend([base, base + 1, base + 2, base, base + 2, base + 3]);
            }
        }

        fn mesh(self) -> Mesh {
            let n = self.pos.len();
            Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default())
                .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, self.pos)
                .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, self.normal)
                .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, vec![[0.0f32, 0.0]; n])
                .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, self.color)
                .with_inserted_indices(Indices::U32(self.index))
        }
    }

    /// Lifts off the surface under a marking (m): the grid, then the
    /// lines, then the numbers.
    const GRID_Y: f32 = 0.004;
    const LINE_Y: f32 = 0.008;
    const LABEL_Y: f32 = 0.012;
    const OFF_WALL: f32 = 0.006;

    /// The floor's grid and lines, the distances' numbers and boards, the
    /// walls' seams and the board's rings (one mesh), and the sky.
    pub(super) fn markings_and_sky(world: &mut World, d: &RangeData) {
        let mut m = Batch::default();
        let f = &d.floor;
        let half = f.width / 2.0;
        let (near, far) = (-f.behind, f.length);
        let grid = color(&f.grid_color);
        // The grid: lines across, then along.
        let mut a = (near / f.grid).ceil() * f.grid;
        while a <= far {
            m.floor(at(0.0, a, GRID_Y), Vec2::new(f.width, f.grid_line), grid);
            a += f.grid;
        }
        let mut x = -half + f.grid;
        while x < half - 0.01 {
            m.floor(at(x, (near + far) / 2.0, GRID_Y), Vec2::new(f.grid_line, far - near), grid);
            x += f.grid;
        }
        // The firing line and the axis.
        let fl = &d.firing_line;
        m.floor(at(0.0, 0.0, LINE_Y), Vec2::new(f.width, fl.width), color(&fl.color));
        let last = d.distances.at.iter().copied().fold(0.0, f32::max);
        m.floor(at(0.0, last / 2.0, LINE_Y), Vec2::new(fl.axis_width, last), color(&fl.color));
        // The distances: a stripe, its number on the floor at the stripe's
        // right end, and a board on each side wall.
        let ds = &d.distances;
        let (stripe, label) = (color(&ds.color), color(&ds.label_color));
        let (board, digits) = (color(&ds.sign_color), color(&ds.sign_digits));
        for &dist in &ds.at {
            let width = ds.width + ds.width_per_m * dist;
            m.floor(at(0.0, dist, LINE_Y), Vec2::new(2.0 * ds.half, width), stripe);
            let text = format!("{} M", number(dist));
            // One reads from the firing line (up the text is down range,
            // stretched that way as road markings against the slant), one
            // from the board's side (`floor_labels`).
            for l in floor_labels(ds, d.floor.width / 2.0, dist) {
                m.text(&text, l.origin + Vec3::Y * LABEL_Y, l.right, l.up, l.height, label);
            }
            let h = (ds.sign + ds.sign_per_m * dist).min(ds.sign_max);
            let n = number(dist);
            let (bw, bh, depth) = (text_width(&n) * h + 1.0 * h, 1.7 * h, 0.1);
            let bottom = 0.4;
            for s in [-1.0f32, 1.0] {
                let cx = s * (half - bw / 2.0);
                let center = Vec3::new(cx, bottom + bh / 2.0, -dist);
                m.cuboid(center, Vec3::new(bw, bh, depth), board);
                let face = -dist + depth / 2.0 + OFF_WALL;
                // A blue bar along the top, the number under it.
                m.quad(
                    Vec3::new(cx, bottom + bh - 0.1 * h, face),
                    Vec3::X * (bw / 2.0),
                    Vec3::Y * 0.1 * h,
                    Vec3::Z,
                    stripe,
                );
                let left = cx - text_width(&n) * h / 2.0;
                m.text(&n, Vec3::new(left, bottom + 0.3 * h, face), Vec3::X, Vec3::Y, h, digits);
            }
        }
        // Seams on the walls' faces.
        let w = &d.walls;
        let seam = color(&w.seam_color);
        let mut a = (near / w.panel).ceil() * w.panel;
        while a <= far {
            for s in [-1.0f32, 1.0] {
                m.quad(
                    Vec3::new(s * (half - OFF_WALL), w.height / 2.0, -a),
                    Vec3::Z * 0.03,
                    Vec3::Y * w.height / 2.0,
                    Vec3::X * -s,
                    seam,
                );
            }
            a += w.panel;
        }
        let mut x = -half + w.panel;
        while x < half - 0.01 {
            m.quad(
                Vec3::new(x, w.backstop_height / 2.0, -far + OFF_WALL),
                Vec3::X * 0.03,
                Vec3::Y * w.backstop_height / 2.0,
                Vec3::Z,
                seam,
            );
            x += w.panel;
        }
        // The board's rings, outermost first, each down to the next.
        let b = &d.board;
        let face = at(b.at[0], b.at[1], b.height) + Vec3::Z * (b.size[2] / 2.0 + OFF_WALL);
        for (i, r) in b.rings.iter().enumerate() {
            let inner = b.rings.get(i + 1).copied().unwrap_or(0.0);
            let c = b.ring_colors.get(i).map_or(Color::WHITE, |c| color(c));
            m.ring(face, Vec3::X, Vec3::Y, inner, *r, c);
        }
        if let Some(look) = look(
            world,
            m.mesh(),
            StandardMaterial {
                perceptual_roughness: 0.9,
                ..default()
            },
        ) {
            world.spawn((Name::new("Range markings"), RangePart, Transform::default(), look));
        }
        sky(world, d);
    }

    /// A dome of the sky's gradient around the range, unlit, casting no
    /// shadow.
    fn sky(world: &mut World, d: &RangeData) {
        let radius = 700.0;
        let mut mesh = Sphere::new(radius).mesh().uv(48, 24);
        let (top, horizon) = (color(&d.light.sky_top).to_linear(), color(&d.light.sky_horizon).to_linear());
        if let Some(VertexAttributeValues::Float32x3(pos)) = mesh.attribute(Mesh::ATTRIBUTE_POSITION) {
            let colors: Vec<[f32; 4]> = pos
                .iter()
                .map(|p| {
                    let t = (p[1] / radius).clamp(0.0, 1.0).powf(0.6);
                    horizon.mix(&top, t).to_f32_array()
                })
                .collect();
            mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, colors);
        }
        if let Some(look) = look(
            world,
            mesh,
            StandardMaterial {
                unlit: true,
                fog_enabled: false,
                cull_mode: None,
                ..default()
            },
        ) {
            world.spawn((
                Name::new("Range sky"),
                RangePart,
                Transform::from_translation(at(0.0, d.floor.length / 2.0, 0.0)),
                look,
                bevy::light::NotShadowCaster,
                bevy::light::NotShadowReceiver,
            ));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_layout_reads_and_holds_its_promises() {
        let d = &data();
        assert_eq!(d.distances.at, [5.0, 10.0, 20.0, 40.0, 100.0, 200.0]);
        assert!(d.floor.length > 200.0, "the backstop is past 200 m");
        let l = RangeLayout::of(d);
        assert!(l.spawns.iter().all(|s| s.z > 0.0 && s.z < 3.0), "spawns just behind the firing line");
        assert!(l.prop_spots.len() >= 2);
        assert_eq!(l.at_distance(40.0), Vec3::new(0.0, 0.0, -40.0));
    }

    #[test]
    fn the_layout_carries_the_datas_dimensions() {
        let mut d = data();
        let l = RangeLayout::of(&d);
        assert_eq!(l.board_size, Vec2::new(1.2, 1.2));
        assert_eq!(l.board_normal, Vec3::Z, "the board faces the firing line");
        assert_eq!(l.shelf_size, Vec3::new(4.0, 1.0, 0.8));
        assert_eq!(l.shelf_top, 1.0);
        assert_eq!(l.shelf, Vec3::new(-11.0, 0.0, -12.0));
        assert_eq!((l.platform_size, l.ramp_width, l.ramp_degrees), (Vec3::new(6.0, 3.0, 6.0), 4.0, 20.0));
        assert_eq!(l.platform_top, Vec3::new(14.0, 3.0, 4.0));
        assert_eq!(l.stripe_half, 6.0);
        assert_eq!(l.cover.len(), d.cover.len());
        assert_eq!(l.cover[0], LayoutBox { centre: Vec3::new(-15.0, 0.0, -14.0), size: Vec3::new(3.0, 1.2, 2.0) });
        // The clear lane: the nearest crate face to the axis (the crate at
        // x = 15 / 2.4 m wide... its face at 14.0 for the 4 m crate).
        let nearest = d.cover.iter().map(|c| c.at[0].abs() - c.size[0] / 2.0).fold(f32::MAX, f32::min);
        assert!((l.lane_half - nearest).abs() < 1e-5 && l.lane_half > 5.0, "{}", l.lane_half);
        // Changing the data changes the layout: nothing is assumed.
        d.board.size = [2.0, 1.5, 0.05];
        d.shelf.size = [3.0, 1.4, 1.0];
        d.platform.size = [8.0, 2.0, 5.0];
        d.distances.half = 4.0;
        d.cover.push(CoverData { at: [-2.0, 40.0], size: [2.0, 1.0, 1.0] });
        let l = RangeLayout::of(&d);
        assert_eq!(l.board_size, Vec2::new(2.0, 1.5));
        assert_eq!((l.shelf_size.y, l.shelf_top), (1.4, 1.4));
        assert_eq!(l.platform_top.y, 2.0);
        assert_eq!(l.stripe_half, 4.0);
        assert!((l.lane_half - 1.0).abs() < 1e-5, "a crate 1 m off the axis narrows the lane: {}", l.lane_half);
        d.cover.push(CoverData { at: [0.0, 60.0], size: [2.0, 1.0, 1.0] });
        assert_eq!(RangeLayout::of(&d).lane_half, 0.0, "a crate across the axis closes the lane");
    }

    #[test]
    fn floor_labels_read_from_the_firing_line_and_from_the_boards_side() {
        let d = data();
        for &dist in &d.distances.at {
            let [near, far] = floor_labels(&d.distances, d.floor.width / 2.0, dist);
            // From the firing line (looking down range, -Z; its right is
            // +X): the text runs right, its up goes away.
            assert_eq!(near.right, Vec3::X);
            assert!(near.up.dot(Vec3::NEG_Z) > 0.0);
            // From the board's side (looking back, +Z; its right is -X).
            assert_eq!(far.right, Vec3::NEG_X);
            assert!(far.up.dot(Vec3::Z) > 0.0);
            // Both lie on the floor facing up, either side of the stripe's
            // ends, the turned one the other's half turn about its middle.
            assert_eq!(near.right.cross(near.up).normalize(), Vec3::Y);
            assert_eq!(far.right.cross(far.up).normalize(), Vec3::Y);
            assert!(near.origin.x > d.distances.half && far.origin.x < -d.distances.half);
            let w = text_width(&format!("{} M", number(dist))) * near.height;
            // Within the floor (the wall's face at the half width).
            assert!(near.origin.x + w <= d.floor.width / 2.0 + 1e-3, "{dist}: label past the wall");
            let (a, b) = (near.origin + near.right * w + near.up * near.height, far.origin + far.right * w + far.up * far.height);
            assert!((far.origin.x + near.origin.x).abs() < 1e-4 && (far.origin.z + near.origin.z + 2.0 * dist).abs() < 1e-3, "{dist}");
            assert!((a.x + b.x).abs() < 1e-4 && (a.z + b.z + 2.0 * dist).abs() < 1e-3, "{dist}");
        }
    }

    #[test]
    fn map_names_read_as_code_maps() {
        use crate::map::code_maps::{GREYBOX, code_map};
        assert_eq!(code_map(""), Some(GREYBOX));
        assert_eq!(code_map("mashup:greybox"), Some(GREYBOX));
        assert_eq!(code_map("chimera:Range"), Some(RANGE));
        assert_eq!(code_map("range"), Some(RANGE));
        assert_eq!(code_map("cs_source:range"), None);
        assert_eq!(code_map("de_dust2"), None);
    }

    #[test]
    fn digits_and_m_have_strokes() {
        for c in "0123456789M".chars() {
            assert!(!strokes(c).is_empty(), "{c}");
        }
        assert_eq!(number(40.0), "40");
        assert!((text_width("200") - 2.3).abs() < 1e-5);
    }
}
