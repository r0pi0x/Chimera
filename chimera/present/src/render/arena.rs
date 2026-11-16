//! The placeholder look of a match's slice (docs/plans/active/arena-
//! slices.md, section 8's first cut; the owner's decision 9): where the
//! wall of the loaded map's play area (`map::play_area::PlayArea`) stands,
//! drawn on every machine from the play area alone. A translucent curtain
//! along each edge, from the floor up `CURTAIN_HEIGHT`, its colour fading
//! to nothing at the top, and a line at its foot, both unlit, plain alpha
//! blending, in the plan's accent blue: one mesh each (`ArenaLook`).
//!
//! The floor under the wall: each edge sampled every `SAMPLE` m, a ray
//! down from `RAY_ABOVE` over the slice's seed (the world's solid
//! collision, the wall left out), the last floor found kept where a ray
//! finds none. Built once the seed's own ray finds the floor (the map's
//! collision in the physics world: a few frames after the spawn), again
//! when the slice changes, gone with it.
//!
//! Known shortcuts: one floor per sample (a stacked map's upper floor
//! along the wall gets no line); no glow, dashes, dimming or theme tokens
//! (A7, A8); the colours are constants here.

use avian3d::prelude::*;
use bevy::{
    asset::RenderAssetUsages,
    ecs::system::SystemState,
    mesh::{Indices, PrimitiveTopology},
    prelude::*,
};

use crate::map::{
    MapPart,
    play_area::{ArenaWall, PlayArea},
};

/// How tall the curtain is over the floor (m).
pub const CURTAIN_HEIGHT: f32 = 4.0;
/// The foot line's width (m) and its height over the floor.
pub const LINE_WIDTH: f32 = 0.15;
const LINE_LIFT: f32 = 0.04;
/// Samples along an edge (m).
pub const SAMPLE: f32 = 0.5;
/// Where the floor rays start over the seed (m), and how far they reach.
const RAY_ABOVE: f32 = 40.0;
const RAY_LENGTH: f32 = 200.0;
/// The accent (#3B7BFF); the curtain's alpha at its foot.
const ACCENT: [f32; 3] = [0.231, 0.482, 1.0];
const CURTAIN_ALPHA: f32 = 0.22;
const LINE_ALPHA: f32 = 0.9;

/// The look's meshes, for the slice `id`.
#[derive(Component, Clone, Debug)]
pub struct ArenaLook {
    pub id: String,
}

pub fn plugin(app: &mut App) {
    app.add_systems(Update, draw_play_area);
}

/// The look follows the play area (module doc).
pub fn draw_play_area(world: &mut World) {
    let want = world.get_resource::<PlayArea>().map(|a| a.slice.key());
    let have: Vec<(Entity, String)> = world
        .query::<(Entity, &ArenaLook)>()
        .iter(world)
        .map(|(e, l)| (e, l.id.clone()))
        .collect();
    if have.first().map(|h| &h.1) == want.as_ref() {
        return;
    }
    for (e, _) in have {
        world.despawn(e);
    }
    let Some(area) = world.get_resource::<PlayArea>().cloned() else { return };
    // Nothing to draw with (a headless app).
    if !world.contains_resource::<Assets<Mesh>>() || !world.contains_resource::<Assets<StandardMaterial>>() {
        return;
    }
    let walls: Vec<Entity> = world.query_filtered::<Entity, With<ArenaWall>>().iter(world).collect();
    let mut state = SystemState::<SpatialQuery>::new(world);
    let Ok(q) = state.get(world) else { return };
    let filter = SpatialQueryFilter::from_mask(crate::core::SOLID_LAYERS).with_excluded_entities(walls);
    let top = area.slice.seed.y + RAY_ABOVE;
    let floor = |x: f32, z: f32| {
        q.cast_ray(Vec3::new(x, top, z), Dir3::NEG_Y, RAY_LENGTH, true, &filter)
            .map(|h| top - h.distance)
    };
    // Not in the physics world yet: next frame.
    if floor(area.slice.seed.x, area.slice.seed.z).is_none() {
        return;
    }
    let mut last = area.slice.seed.y;
    // Per edge, its samples: (point, floor, the edge's direction).
    let mut rows: Vec<Vec<(Vec2, f32, Vec2)>> = Vec::new();
    for (a, b) in area.polygon.edges() {
        let dir = (b - a).normalize_or_zero();
        let n = ((a.distance(b) / SAMPLE).ceil() as usize).max(1);
        let mut row = Vec::with_capacity(n + 1);
        for i in 0..=n {
            let p = a.lerp(b, i as f32 / n as f32);
            if let Some(y) = floor(p.x, p.y) {
                last = y;
            }
            row.push((p, last, dir));
        }
        rows.push(row);
    }
    let curtain = strip_mesh(&rows, |p, y, _| {
        (Vec3::new(p.x, y + LINE_LIFT, p.y), Vec3::new(p.x, y + CURTAIN_HEIGHT, p.y), CURTAIN_ALPHA, 0.0)
    });
    let line = strip_mesh(&rows, |p, y, dir| {
        let side = Vec2::new(-dir.y, dir.x) * (LINE_WIDTH / 2.0);
        let (l, r) = (p - side, p + side);
        (Vec3::new(l.x, y + LINE_LIFT, l.y), Vec3::new(r.x, y + LINE_LIFT, r.y), LINE_ALPHA, LINE_ALPHA)
    });
    let id = area.slice.key();
    for (name, mesh) in [("Arena curtain", curtain), ("Arena foot line", line)] {
        let mesh = world.resource_mut::<Assets<Mesh>>().add(mesh);
        let material = world.resource_mut::<Assets<StandardMaterial>>().add(StandardMaterial {
            base_color: Color::WHITE,
            unlit: true,
            alpha_mode: AlphaMode::Blend,
            double_sided: true,
            cull_mode: None,
            fog_enabled: true,
            ..default()
        });
        world.spawn((
            Name::new(name),
            ArenaLook { id: id.clone() },
            MapPart,
            Mesh3d(mesh),
            MeshMaterial3d(material),
            bevy::light::NotShadowCaster,
            bevy::light::NotShadowReceiver,
            Transform::default(),
        ));
    }
    info!("arena {id}: its wall drawn ({} edges)", rows.len());
}

/// A strip per edge from its samples: each gives two vertices (`pair`:
/// the first, the second, and their alphas), quads between neighbours.
fn strip_mesh(rows: &[Vec<(Vec2, f32, Vec2)>], pair: impl Fn(Vec2, f32, Vec2) -> (Vec3, Vec3, f32, f32)) -> Mesh {
    let mut positions: Vec<[f32; 3]> = Vec::new();
    let mut colors: Vec<[f32; 4]> = Vec::new();
    let mut indices: Vec<u32> = Vec::new();
    let [r, g, b] = ACCENT;
    for row in rows {
        let base = positions.len() as u32;
        for &(p, y, dir) in row {
            let (lo, hi, a0, a1) = pair(p, y, dir);
            positions.push(lo.to_array());
            positions.push(hi.to_array());
            colors.push([r, g, b, a0]);
            colors.push([r, g, b, a1]);
        }
        for i in 0..row.len().saturating_sub(1) as u32 {
            let (a, b, c, d) = (base + 2 * i, base + 2 * i + 1, base + 2 * i + 2, base + 2 * i + 3);
            indices.extend_from_slice(&[a, c, b, b, c, d]);
        }
    }
    let normals = vec![[0.0, 1.0, 0.0]; positions.len()];
    Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::RENDER_WORLD)
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
        .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, normals)
        .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, colors)
        .with_inserted_indices(Indices::U32(indices))
}
