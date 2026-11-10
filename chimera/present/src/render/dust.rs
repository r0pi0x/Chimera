//! Dust mote simulation and drawing for `MapDust` volumes, per
//! specs/cs_source/sprites_dust.md 8-9: motes spawn at random points inside
//! the volume's brushes (10 tries in its bounds), drift (horizontal speed
//! dies out toward the wind, vertical stays), fade in and out over their
//! life and with view depth, and keep a constant size on screen (dust
//! clouds: a size in the world). Frozen volumes make their motes once, and
//! those never move, age or die.
//!
//! Drawn as a volume's quads, one per mote, made by the vertex shader
//! (dust.wesl, `DustMotes` extending the volume's unlit `StandardMaterial`)
//! from a buffer of the motes in view (position, half-width, colour; far to
//! near) and the camera's axes, rewritten each frame while the volume is
//! shown and has motes in view. Its mesh is only the quads' corners and
//! numbers, made again when the motes outgrow it: a mesh rewritten every
//! frame is freed and allocated again in Bevy's mesh slabs
//! (docs/performance.md, "Render performance").

use bevy::{
    asset::{RenderAssetUsages, embedded_asset},
    mesh::Indices,
    pbr::{ExtendedMaterial, MaterialExtension},
    prelude::*,
    reflect::TypePath,
    render::{
        render_resource::{AsBindGroup, PrimitiveTopology},
        storage::ShaderBuffer,
    },
    shader::ShaderRef,
};

use crate::map::{EntityPart, MapDust, SkyboxCamera, ViewModelCamera, monitor, water};

/// All dust shares this many motes at most (the game's particle limit).
const MAX_MOTES: usize = 2048;
/// Horizontal speed moves toward the wind (none) at this rate, m/s^2.
const WIND_PULL: f32 = 50.0 * 0.0254;

struct Mote {
    position: Vec3,
    velocity: Vec3,
    age: f32,
    life: f32,
    size: f32,
}

/// The motes a volume draws, for dust.wesl's vertex shader: the camera's
/// right (w: the motes' count), up and the quads' normal, then each mote's
/// position (w: half-width) and linear colour with its alpha.
#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
pub struct DustMotes {
    #[storage(100, read_only)]
    pub motes: Handle<ShaderBuffer>,
}

impl MaterialExtension for DustMotes {
    fn vertex_shader() -> ShaderRef {
        "embedded://chimera_present/render/dust.wesl".into()
    }
}

/// A dust volume's material: its unlit `StandardMaterial` drawing quads
/// the shader places (`DustMotes`).
pub type DustMaterial = ExtendedMaterial<StandardMaterial, DustMotes>;

pub struct DustMaterialPlugin;

impl Plugin for DustMaterialPlugin {
    fn build(&self, app: &mut App) {
        embedded_asset!(app, "dust.wesl");
        app.add_plugins(MaterialPlugin::<DustMaterial>::default());
    }
}

/// Header vec4s before the motes in a volume's buffer.
const MOTES_HEADER: usize = 3;

#[derive(Component)]
pub struct DustEmitter {
    pub dust: MapDust,
    pub mesh: Handle<Mesh>,
    /// The motes in view, for the shader (`DustMotes`): made with the
    /// volume's `DustMaterial`, which replaces its `StandardMaterial` on
    /// its first draw.
    motes_buffer: Option<Handle<ShaderBuffer>>,
    /// Quads the mesh has (0: not made yet).
    quads: usize,
    motes: Vec<Mote>,
    pending: f32,
    rng: u64,
    /// A frozen volume made its motes.
    frozen_spawned: bool,
    /// The mesh was last written with no motes in it: not written again
    /// until some show (a modified mesh is uploaded again and makes every
    /// mesh entity re-check its pipeline that frame).
    drawn_empty: bool,
}

impl DustEmitter {
    pub fn new(dust: MapDust, mesh: Handle<Mesh>, seed: u64) -> Self {
        Self {
            dust,
            mesh,
            motes_buffer: None,
            quads: 0,
            motes: Vec::new(),
            pending: 0.0,
            rng: seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1,
            frozen_spawned: false,
            drawn_empty: false,
        }
    }

    /// Uniform in 0..1 (xorshift).
    fn random(&mut self) -> f32 {
        self.rng ^= self.rng << 13;
        self.rng ^= self.rng >> 7;
        self.rng ^= self.rng << 17;
        (self.rng >> 40) as f32 / (1u64 << 24) as f32
    }

    fn between(&mut self, (a, b): (f32, f32)) -> f32 {
        a + (b - a) * self.random()
    }

    /// A spawn point (8): up to 10 tries at random in the bounds, the
    /// first inside one of the brushes (any point without brushes).
    fn spawn_point(&mut self) -> Option<Vec3> {
        let (min, max) = (self.dust.min, self.dust.max);
        for _ in 0..10 {
            let p = Vec3::new(
                min.x + (max.x - min.x) * self.random(),
                min.y + (max.y - min.y) * self.random(),
                min.z + (max.z - min.z) * self.random(),
            );
            if self.dust.brushes.is_empty() || self.dust.brushes.iter().any(|b| b.iter().all(|(n, d)| n.dot(p) <= *d)) {
                return Some(p);
            }
        }
        None
    }
}

/// A mote's half-width (9): screen-constant (size / 10000 x depth), or a
/// dust cloud's size in the world.
fn half_width(dust: &MapDust, size: f32, depth: f32) -> f32 {
    if dust.world_size { size } else { size / 10000.0 * depth }
}

/// A mote's alpha before the volume's own (9): its life's fade in and out
/// (frozen motes: 1) times the fade with view depth; None past DistMax.
fn mote_alpha(dust: &MapDust, age: f32, life: f32, depth: f32) -> Option<f32> {
    if depth <= 0.0 || depth >= dust.fade_distance {
        return None;
    }
    let life_alpha = if dust.frozen {
        1.0
    } else {
        0.5 - 0.5 * (std::f32::consts::TAU * age / life).cos()
    };
    Some(life_alpha * (1.0 - depth / dust.fade_distance))
}

/// A mesh for an emitter to fill each frame: one invisible triangle, since
/// Bevy's mesh allocator logs a use-after-free for meshes with no vertices
/// (it skips allocating them but still copies their data).
pub fn empty_mesh() -> Mesh {
    let mut mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default());
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, vec![[0.0f32; 3]; 3]);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, vec![[0.0f32, 1.0, 0.0]; 3]);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, vec![[0.0f32; 2]; 3]);
    mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, vec![[0.0f32; 4]; 3]);
    mesh.insert_indices(Indices::U32(vec![0, 1, 2]));
    mesh
}

/// Write an emitter's quads into its mesh (dust motes, particles).
///
/// Bevy's mesh allocator frees a modified mesh and allocates it again in
/// the GPU buffer (slab) it shares with other meshes of its layout; a mesh
/// whose size changed every frame made that cost 3.5-4.5 ms of the render
/// world per frame on de_dust2 for one dust volume
/// (`allocate_and_free_meshes`; presumably the new size not fitting the
/// hole the old one left, so the slab, which holds the whole map's
/// indices, is reallocated and copied; docs/performance.md). So the
/// buffers are padded to a power of two (at least 64 vertices) with
/// degenerate triangles: the size changes rarely. An empty mesh gets the
/// padding too (Bevy's allocator logs a use-after-free for meshes without
/// vertices).
pub fn write_dynamic_mesh(
    mesh: &mut Mesh,
    normal: Vec3,
    mut positions: Vec<[f32; 3]>,
    mut uvs: Vec<[f32; 2]>,
    mut colors: Vec<[f32; 4]>,
    mut indices: Vec<u32>,
) {
    let vertices = positions.len().max(1).next_power_of_two().max(MIN_DYNAMIC_VERTICES);
    // Six indices per quad of four vertices.
    let index_count = (vertices / 2 * 3).max(indices.len().next_power_of_two());
    // Padding sits on a real vertex (bounds stay put), fully transparent.
    let at = positions.first().copied().unwrap_or([0.0; 3]);
    positions.resize(vertices, at);
    uvs.resize(vertices, [0.0; 2]);
    colors.resize(vertices, [0.0; 4]);
    indices.resize(index_count, 0);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, vec![normal.to_array(); vertices]);
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
    mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, colors);
    mesh.insert_indices(Indices::U32(indices));
}

/// `quads` quads for dust.wesl to place: each corner's coordinates in UV0
/// (and so which corner: u for left or right, v for top or bottom) and its
/// quad's number in UV1's x; positions are the shader's.
pub fn mote_quads(quads: usize) -> Mesh {
    let mut uvs = Vec::with_capacity(quads * 4);
    let mut numbers = Vec::with_capacity(quads * 4);
    let mut indices = Vec::with_capacity(quads * 6);
    for q in 0..quads {
        let base = (q * 4) as u32;
        for uv in [[0.0, 1.0], [0.0, 0.0], [1.0, 0.0], [1.0, 1.0]] {
            uvs.push(uv);
            numbers.push([q as f32, 0.0]);
        }
        indices.extend([base, base + 1, base + 2, base, base + 2, base + 3]);
    }
    let n = quads * 4;
    let mut mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::RENDER_WORLD);
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, vec![[0.0f32; 3]; n]);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, vec![[0.0f32, 1.0, 0.0]; n]);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_1, numbers);
    // Only so the material reads the vertex colour (the shader's).
    mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, vec![[0.0f32; 4]; n]);
    mesh.insert_indices(Indices::U32(indices));
    mesh
}

/// Smallest vertex count `write_dynamic_mesh` writes.
pub const MIN_DYNAMIC_VERTICES: usize = 64;

#[allow(clippy::type_complexity)]
pub fn update_dust(
    time: Res<Time>,
    cameras: Query<
        &GlobalTransform,
        (
            With<Camera3d>,
            Without<monitor::ScreenCamera>,
            Without<SkyboxCamera>,
            Without<ViewModelCamera>,
            Without<water::WaterReflectionCamera>,
        ),
    >,
    mut emitters: Query<(
        Entity,
        &mut DustEmitter,
        Option<&EntityPart>,
        Option<&Visibility>,
        Option<&MeshMaterial3d<StandardMaterial>>,
    )>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut gpu: (
        Option<ResMut<Assets<ShaderBuffer>>>,
        Option<ResMut<Assets<DustMaterial>>>,
        Option<Res<Assets<StandardMaterial>>>,
    ),
    mut commands: Commands,
) {
    let dt = time.delta_secs();
    let Some(eye) = cameras.iter().next() else { return };
    let (from, forward, right, up) = (
        eye.translation(),
        eye.forward().as_vec3(),
        eye.right().as_vec3(),
        eye.up().as_vec3(),
    );
    let mut total: usize = emitters.iter().map(|(_, e, ..)| e.motes.len()).sum();
    for (entity, mut e, part, visibility, standard) in &mut emitters {
        // Turned off (TurnOff): no new motes, the others live out their
        // life; removed (Kill): gone at once.
        if part.is_some_and(|p| !p.exists) {
            e.motes.clear();
        }
        let spawning = part.is_none_or(|p| p.on && p.exists);
        // Spawn (frame time for spawning is clamped to 0.1 s). Frozen:
        // `rate` tries once, when first switched on.
        if spawning && e.dust.frozen {
            if !e.frozen_spawned {
                e.frozen_spawned = true;
                e.pending = e.dust.rate.max(0.0).floor();
            }
        } else if spawning {
            e.pending += e.dust.rate * dt.min(0.1);
        } else {
            e.pending = 0.0;
        }
        while e.pending >= 1.0 {
            e.pending -= 1.0;
            if total >= MAX_MOTES {
                continue;
            }
            let Some(position) = e.spawn_point() else { continue };
            let speed = e.dust.speed;
            let velocity = Vec3::new(
                (e.random() * 2.0 - 1.0) * speed,
                (e.random() * 2.0 - 1.0) * speed,
                (e.random() * 2.0 - 1.0) * speed,
            );
            let (life_range, size_range) = (e.dust.life, e.dust.size);
            let life = e.between(life_range).max(0.01);
            let size = e.between(size_range);
            e.motes.push(Mote {
                position,
                velocity,
                age: 0.0,
                life,
                size,
            });
            total += 1;
        }
        // Move and age (frozen motes do neither).
        let pull = WIND_PULL * dt;
        let frozen = e.dust.frozen;
        e.motes.retain_mut(|m| {
            if frozen {
                return true;
            }
            let horizontal = Vec2::new(m.velocity.x, m.velocity.z);
            let slowed = horizontal.clamp_length_max((horizontal.length() - pull).max(0.0));
            m.velocity.x = slowed.x;
            m.velocity.z = slowed.y;
            m.position += m.velocity * dt;
            m.age += dt;
            m.age < m.life
        });
        // Hidden (culled by `vis`, or switched off): the mesh waits.
        if visibility == Some(&Visibility::Hidden) {
            continue;
        }
        // Draw: camera-facing quads, far to near.
        let mut visible: Vec<(f32, f32, &Mote)> = e
            .motes
            .iter()
            .filter_map(|m| {
                let depth = (m.position - from).dot(forward);
                Some((depth, mote_alpha(&e.dust, m.age, m.life, depth)?, m))
            })
            .collect();
        visible.sort_by(|a, b| b.0.total_cmp(&a.0));
        let color = Color::srgb(e.dust.color[0], e.dust.color[1], e.dust.color[2]).to_linear();
        // The motes for the shader (`DustMotes`): the camera's axes, then
        // each mote far to near.
        let count = visible.len();
        let mut data = Vec::with_capacity(MOTES_HEADER + 2 * count);
        data.push(right.extend(count as f32).to_array());
        data.push(up.extend(0.0).to_array());
        data.push((-forward).extend(0.0).to_array());
        for (depth, alpha, m) in visible {
            let half = half_width(&e.dust, m.size, depth);
            data.push(m.position.extend(half).to_array());
            data.push([color.red, color.green, color.blue, alpha * e.dust.color[3]]);
        }
        if count == 0 && e.drawn_empty {
            continue;
        }
        e.drawn_empty = count == 0;
        let (Some(buffers), Some(dust_materials)) = (gpu.0.as_mut(), gpu.1.as_mut()) else {
            continue;
        };
        let quads = count.max(1).next_power_of_two().max(MIN_DYNAMIC_VERTICES / 4);
        // Its mesh only grows (and so is rarely made again).
        if quads > e.quads {
            e.quads = quads;
            let _ = meshes.insert(e.mesh.id(), mote_quads(quads));
        }
        data.resize(MOTES_HEADER + 2 * e.quads, [0.0; 4]);
        let motes = ShaderBuffer::new(data, RenderAssetUsages::RENDER_WORLD);
        match e.motes_buffer.clone() {
            Some(h) => {
                let _ = buffers.insert(h.id(), motes);
            }
            None => {
                // The first draw: its material becomes the same unlit one
                // with its quads placed by the shader.
                let h = buffers.add(motes);
                e.motes_buffer = Some(h.clone());
                let base = standard
                    .and_then(|m| gpu.2.as_ref().and_then(|s| s.get(&m.0)).cloned())
                    .unwrap_or_default();
                let material = dust_materials.add(DustMaterial {
                    base,
                    extension: DustMotes { motes: h },
                });
                commands
                    .entity(entity)
                    .remove::<MeshMaterial3d<StandardMaterial>>()
                    .insert(MeshMaterial3d(material));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dust() -> MapDust {
        MapDust {
            min: Vec3::ZERO,
            max: Vec3::splat(2.0),
            brushes: Vec::new(),
            rate: 10.0,
            size: (64.0 * 0.0254, 128.0 * 0.0254),
            world_size: true,
            frozen: false,
            speed: 0.1,
            life: (3.0, 5.0),
            fade_distance: 10.0,
            color: [1.0; 4],
            texture: None,
            entity: None,
            start_on: true,
        }
    }

    #[test]
    fn clouds_are_sized_in_the_world_and_motes_on_screen() {
        let mut d = dust();
        // A cloud's mote: its size at any depth.
        assert_eq!(half_width(&d, 1.0, 5.0), 1.0);
        // A mote's: size / 10000 x depth (constant on screen).
        d.world_size = false;
        assert!((half_width(&d, 20.0, 5.0) - 0.01).abs() < 1e-6);
        assert!((half_width(&d, 20.0, 10.0) - 0.02).abs() < 1e-6);
    }

    #[test]
    fn alpha_fades_with_life_and_depth_frozen_ones_by_depth_only() {
        let mut d = dust();
        // Mid-life at half DistMax: 1 x 0.5; at birth 0; past DistMax none.
        assert!((mote_alpha(&d, 2.0, 4.0, 5.0).unwrap() - 0.5).abs() < 1e-6);
        assert!(mote_alpha(&d, 0.0, 4.0, 5.0).unwrap().abs() < 1e-6);
        assert_eq!(mote_alpha(&d, 2.0, 4.0, 10.5), None);
        assert_eq!(mote_alpha(&d, 2.0, 4.0, -1.0), None);
        d.frozen = true;
        assert!((mote_alpha(&d, 0.0, 4.0, 5.0).unwrap() - 0.5).abs() < 1e-6);
    }

    #[test]
    fn motes_start_inside_the_brushes() {
        let mut d = dust();
        // One brush: the bounds' lower x quarter (x <= 0.5 of 0..2).
        d.brushes = vec![vec![(Vec3::X, 0.5)]];
        let mut e = DustEmitter::new(d, Handle::default(), 7);
        let mut found = 0;
        for _ in 0..200 {
            if let Some(p) = e.spawn_point() {
                assert!(p.x <= 0.5, "{p}");
                found += 1;
            }
        }
        // Ten tries at a quarter of the box nearly always find it.
        assert!(found > 180, "{found}");
        // No brushes: anywhere in the bounds.
        e.dust.brushes.clear();
        assert!(e.spawn_point().is_some());
    }

    /// Drawn motes go to the volume's buffer each frame, its quads placed
    /// by the shader: its material becomes a `DustMaterial` once, and its
    /// mesh is made again only when the motes outgrow it, not every frame
    /// (docs/performance.md, "Render performance").
    #[test]
    fn motes_go_to_a_buffer_not_a_new_mesh_each_frame() {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default()));
        app.init_asset::<Mesh>()
            .init_asset::<Image>()
            .init_asset::<ShaderBuffer>()
            .init_asset::<StandardMaterial>()
            .init_asset::<DustMaterial>();
        #[derive(Resource, Default)]
        struct Rewrites(usize);
        app.init_resource::<Rewrites>().add_systems(Update, update_dust).add_systems(
            Last,
            |mut events: MessageReader<AssetEvent<Mesh>>, mut n: ResMut<Rewrites>| {
                n.0 += events.read().filter(|e| matches!(e, AssetEvent::Modified { .. })).count();
            },
        );
        app.world_mut().spawn((Camera3d::default(), GlobalTransform::default()));
        let mut d = dust();
        // In front of the camera (it looks down -Z), plenty of motes.
        d.min = Vec3::new(-1.0, -1.0, -5.0);
        d.max = Vec3::new(1.0, 1.0, -3.0);
        d.rate = 200.0;
        d.life = (100.0, 100.0);
        let w = app.world_mut();
        let mesh = w.resource_mut::<Assets<Mesh>>().add(empty_mesh());
        let standard = w.resource_mut::<Assets<StandardMaterial>>().add(StandardMaterial {
            unlit: true,
            alpha_mode: AlphaMode::Blend,
            ..default()
        });
        let e = w.spawn((DustEmitter::new(d, mesh, 5), Mesh3d(Handle::default()), MeshMaterial3d(standard))).id();
        for _ in 0..30 {
            std::thread::sleep(std::time::Duration::from_millis(2));
            app.update();
        }
        let w = app.world();
        assert!(w.get::<MeshMaterial3d<StandardMaterial>>(e).is_none(), "the plain material replaced");
        let material = w.get::<MeshMaterial3d<DustMaterial>>(e).expect("a dust material");
        let dm = w.resource::<Assets<DustMaterial>>().get(&material.0).unwrap();
        assert!(dm.base.unlit && dm.base.alpha_mode == AlphaMode::Blend, "the volume's own look");
        let buffer = w.resource::<Assets<ShaderBuffer>>().get(&dm.extension.motes).expect("its motes");
        let rows = buffer.cast_slice::<[f32; 4]>().expect("written");
        let count = rows[0][3] as usize;
        assert!(count > 0 && count <= w.get::<DustEmitter>(e).unwrap().motes.len(), "{count} motes in view");
        assert_eq!(rows.len(), MOTES_HEADER + 2 * w.get::<DustEmitter>(e).unwrap().quads, "padded to the mesh's quads");
        let quads = w.resource::<Assets<Mesh>>().get(&w.get::<DustEmitter>(e).unwrap().mesh).unwrap();
        assert!(quads.attribute(Mesh::ATTRIBUTE_UV_1).is_some(), "the quads' numbers");
        let rewrites = w.resource::<Rewrites>().0;
        assert!(rewrites <= 3, "the quads' mesh made again only as the motes grew: {rewrites} in 30 frames");
    }

    #[test]
    fn frozen_volumes_spawn_once_and_hold() {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default()));
        app.init_asset::<Mesh>();
        app.add_systems(Update, update_dust);
        app.world_mut().spawn((Camera3d::default(), GlobalTransform::default()));
        let mut d = dust();
        d.frozen = true;
        d.min = Vec3::new(-1.0, -1.0, -5.0);
        d.max = Vec3::new(1.0, 1.0, -3.0);
        let emitter = app.world_mut().spawn(DustEmitter::new(d, Handle::default(), 3)).id();
        let motes = |app: &App| {
            app.world()
                .get::<DustEmitter>(emitter)
                .unwrap()
                .motes
                .iter()
                .map(|m| (m.position, m.age))
                .collect::<Vec<_>>()
        };
        app.update();
        let first = motes(&app);
        assert_eq!(first.len(), 10, "SpawnRate motes at once");
        for _ in 0..5 {
            std::thread::sleep(std::time::Duration::from_millis(5));
            app.update();
        }
        assert_eq!(motes(&app), first, "no new motes; none moved or aged");
    }
}
