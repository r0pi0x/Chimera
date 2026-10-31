//! Drawing the particle pool (`map::particles::Particles`): each frame one
//! rebuilt mesh per material (particles outside the playable area, in the
//! 3D skybox, in a mesh of their own on the sky camera's layer)
//! (`ParticleDrawMaterial`, particle.wesl: fogged, added ones toward
//! black), camera-facing sprites, trails and flat quads, back to front.
//! Sprites of sprite-card materials keep to their screen-size limits
//! (`ParticleMaterial::screen`: capped and faded by their size on screen,
//! specs/cs_source/impact_effects.md 14); flipped particles show their
//! texture mirrored.
//!
//! Known shortcuts (docs/tech-debt.md): Particles blend in linear space (Source
//! blends in gamma), and each material is one mesh sorted against other
//! transparent things by its particles' centre; fog is the surfaces' per-pixel
//! range fog (blended toward the fog colour, added toward black). Why it's OK
//! for now: Small, fast effects; looks right in screenshots.

use std::collections::HashMap;

use bevy::{
    asset::embedded_asset, prelude::*, reflect::TypePath, render::render_resource::AsBindGroup, shader::ShaderRef,
};

use super::{dust, fog, sky::SkyCameraInfo};
use crate::map::{
    MapData, MapPart, PlayableArea, SKYBOX_LAYER, SkyboxCamera, ViewModelCamera, monitor,
    particles::{MapParticles, Particle, ParticleBlend, ParticleMaterial, Particles, Shape},
    water,
};

/// How particles are drawn (particle.wesl): texture x vertex colour,
/// blended or added, with the scene's fog (`fog::SceneFog`): blended
/// particles fog toward the fog colour, added ones toward black.
#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
#[bind_group_data(ParticleDrawKey)]
pub struct ParticleDrawMaterial {
    #[texture(0)]
    #[sampler(1)]
    pub texture: Option<Handle<Image>>,
    #[uniform(2)]
    pub fog: fog::FogUniform,
    pub blend: ParticleBlend,
}

#[repr(C)]
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct ParticleDrawKey {
    additive: bool,
}

impl From<&ParticleDrawMaterial> for ParticleDrawKey {
    fn from(m: &ParticleDrawMaterial) -> Self {
        Self {
            additive: m.blend == ParticleBlend::Additive,
        }
    }
}

impl ParticleDrawMaterial {
    /// Particles (CS:S's tracers and impact effects, Combat Arms' effect
    /// groups) are depth tested and write no depth: Bevy's compare is
    /// kept, so a wall hides them (`ParticleMaterial::no_depth` isn't
    /// supported). A test checks it, as the pipeline can't be built
    /// without a GPU.
    pub fn apply_depth(depth: &mut bevy::render::render_resource::DepthStencilState) {
        depth.depth_write_enabled = Some(false);
    }
}

impl Material for ParticleDrawMaterial {
    fn fragment_shader() -> ShaderRef {
        "embedded://chimera_present/render/particle.wesl".into()
    }

    fn alpha_mode(&self) -> AlphaMode {
        match self.blend {
            ParticleBlend::Alpha => AlphaMode::Blend,
            ParticleBlend::Additive => AlphaMode::Add,
        }
    }

    fn specialize(
        _pipeline: &bevy::pbr::MaterialPipeline,
        descriptor: &mut bevy::render::render_resource::RenderPipelineDescriptor,
        _layout: &bevy::mesh::MeshVertexBufferLayoutRef,
        key: bevy::pbr::MaterialPipelineKey<Self>,
    ) -> Result<(), bevy::render::render_resource::SpecializedMeshPipelineError> {
        descriptor.primitive.cull_mode = None;
        if let Some(depth) = descriptor.depth_stencil.as_mut() {
            Self::apply_depth(depth);
        }
        if key.bind_group_data.additive
            && let Some(fragment) = descriptor.fragment.as_mut()
        {
            fragment.shader_defs.push("ADDITIVE".into());
        }
        Ok(())
    }

    fn enable_prepass() -> bool {
        false
    }

    fn enable_shadows() -> bool {
        false
    }
}

/// Registers the particle material. Needs rendering.
pub struct ParticleMaterialPlugin;

impl Plugin for ParticleMaterialPlugin {
    fn build(&self, app: &mut App) {
        embedded_asset!(app, "particle.wesl");
        app.add_plugins((fog::FogShaderPlugin, MaterialPlugin::<ParticleDrawMaterial>::default()));
    }
}

/// The meshes particles are drawn with: one entity per material.
#[derive(Resource)]
pub struct ParticleAssets {
    data: MapParticles,
    textures: Vec<Handle<Image>>,
    /// (Material index, in the 3D skybox) -> (mesh entity, mesh, whether
    /// it was last written empty). An empty mesh isn't written again until
    /// particles come back: a modified mesh is uploaded again, and makes
    /// every mesh entity re-check its pipeline that frame.
    meshes: HashMap<(usize, bool), (Entity, Handle<Mesh>, bool)>,
}

/// The particle meshes' resource for a map (`map::MapDrawer::insert_drawn`):
/// only with textures (drawn) and particle materials.
pub fn insert_assets(commands: &mut Commands, data: &MapData, textures: &[Handle<Image>]) {
    if !textures.is_empty() && !data.particles.materials.is_empty() {
        commands.insert_resource(ParticleAssets::new(data.particles.clone(), textures.to_vec()));
    }
}

impl ParticleAssets {
    pub fn new(data: MapParticles, textures: Vec<Handle<Image>>) -> Self {
        Self {
            data,
            textures,
            meshes: HashMap::new(),
        }
    }
}

/// Rebuild the particle meshes for the camera.
#[allow(clippy::type_complexity)]
pub fn draw_particles(
    particles: Res<Particles>,
    assets: Option<ResMut<ParticleAssets>>,
    cameras: Query<
        (&GlobalTransform, Option<&Projection>),
        (
            With<Camera3d>,
            Without<monitor::ScreenCamera>,
            Without<SkyboxCamera>,
            Without<ViewModelCamera>,
            Without<water::WaterReflectionCamera>,
        ),
    >,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ParticleDrawMaterial>>,
    fog: Option<Res<fog::SceneFog>>,
    area: Option<Res<PlayableArea>>,
    sky: Option<Res<SkyCameraInfo>>,
    mut transforms: Query<&mut Transform>,
    mut commands: Commands,
) {
    let Some(mut assets) = assets else { return };
    let Some((eye, projection)) = cameras.iter().next() else {
        return;
    };
    // Sprite cards' screen sizes are taken against the view's height.
    let tan_half_fov = match projection {
        Some(Projection::Perspective(p)) => (p.fov / 2.0).tan(),
        _ => 1.0,
    };
    let (from, forward, right, up) = (
        eye.translation(),
        eye.forward().as_vec3(),
        eye.right().as_vec3(),
        eye.up().as_vec3(),
    );
    // Particles outside the playable area are in the 3D skybox (a tesla,
    // a fire out there): the sky camera draws them where they are, from
    // its own eye (the player's scaled into the skybox).
    let in_sky = |p: Vec3| {
        area.as_ref()
            .is_some_and(|a| p.cmplt(a.0.0).any() || p.cmpgt(a.0.1).any())
    };
    let sky_eye = sky.as_ref().map(|s| s.0.origin + from / s.0.scale);
    // Per material and layer: (depth, particle, near fade).
    let mut buckets: HashMap<(usize, bool), Vec<(f32, &Particle, f32)>> = HashMap::new();
    for g in &particles.groups {
        for p in &g.particles {
            let skybox = sky_eye.is_some() && in_sky(p.position);
            let eye = if skybox { sky_eye.unwrap_or(from) } else { from };
            let depth = (p.position - eye).dot(forward);
            let near = g.motion.draw_factor(depth, p);
            buckets.entry((p.material, skybox)).or_default().push((depth, p, near));
        }
    }
    let mut used: Vec<(usize, bool)> = assets.meshes.keys().chain(buckets.keys()).copied().collect();
    used.sort_unstable();
    used.dedup();
    for key in used {
        let (material, skybox) = key;
        let from = if skybox { sky_eye.unwrap_or(from) } else { from };
        let Some(def) = assets.data.materials.get(material).cloned() else {
            continue;
        };
        let list = buckets.remove(&key).unwrap_or_default();
        if list.is_empty() && !assets.meshes.contains_key(&key) {
            continue;
        }
        let (entity, handle, was_empty) = match assets.meshes.get(&key) {
            Some(m) => m.clone(),
            None => {
                let handle = meshes.add(dust::empty_mesh());
                let texture = def.texture.and_then(|t| assets.textures.get(t).cloned());
                let entity = commands
                    .spawn((
                        Name::new(format!("Particles {}", def.name)),
                        MapPart,
                        Mesh3d(handle.clone()),
                        MeshMaterial3d(materials.add(ParticleDrawMaterial {
                            texture,
                            fog: fog.as_ref().map_or_else(Default::default, |f| f.0),
                            blend: def.blend,
                        })),
                        // The mesh moves with its particles.
                        bevy::camera::visibility::NoFrustumCulling,
                        bevy::light::NotShadowCaster,
                        Transform::default(),
                    ))
                    .id();
                if skybox {
                    commands
                        .entity(entity)
                        .insert(bevy::camera::visibility::RenderLayers::layer(SKYBOX_LAYER));
                }
                assets.meshes.insert(key, (entity, handle.clone(), false));
                (entity, handle, false)
            }
        };
        let mut list = list;
        // Far to near (depth rounded to whole game units, as the game sorts).
        list.sort_by(|a, b| (b.0 / 0.0254).round().total_cmp(&(a.0 / 0.0254).round()));
        // Sorted against other transparent things by its particles' centre.
        let centre = if list.is_empty() {
            Vec3::ZERO
        } else {
            list.iter().map(|(_, p, _)| p.position).sum::<Vec3>() / list.len() as f32
        };
        let mut b = QuadBuilder::default();
        for (depth, p, near) in list {
            let alpha = if def.vertex_alpha {
                p.current_alpha() * near
            } else {
                near.min(1.0)
            };
            if def.vertex_alpha && alpha < 0.001 {
                continue;
            }
            let color = if def.vertex_color {
                let c = p.current_color();
                Color::srgb(c.x, c.y, c.z).to_linear()
            } else {
                LinearRgba::WHITE
            };
            let rgba = [color.red, color.green, color.blue, alpha];
            let mut rect = frame_rect(&def, p);
            if p.flip {
                rect = [rect[2], rect[1], rect[0], rect[3]];
            }
            let size = p.current_size();
            let at = p.position - centre;
            match p.shape {
                Shape::Sprite => {
                    let (size, fade) = def.screen.apply(size, depth, tan_half_fov);
                    if fade < 0.001 {
                        continue;
                    }
                    let rgba = [rgba[0], rgba[1], rgba[2], rgba[3] * fade];
                    let (s, c) = p.roll.sin_cos();
                    let (r, u) = ((right * c + up * s) * size, (up * c - right * s) * size);
                    b.quad([at - r - u, at - r + u, at + r + u, at + r - u], rect, rgba);
                }
                Shape::Trail { width, length } => {
                    let life = if p.life > 0.0 { p.age / p.life } else { 1.0 };
                    let along = p.velocity * length * (1.0 - life).max(0.01);
                    let w = width.min(along.length()) / 2.0;
                    let side = along.cross(p.position - from).normalize_or_zero() * w;
                    // Texture V runs along the strip.
                    b.quad([at - side, at - side + along, at + side + along, at + side], rect, rgba);
                }
                Shape::Streak {
                    start,
                    dir,
                    distance,
                    length,
                    width,
                    speed,
                } => {
                    let head = (speed * p.age).clamp(0.0, distance);
                    let tail = (head - length).clamp(0.0, distance);
                    if head <= tail {
                        continue;
                    }
                    let (h, t) = (start + dir * head, start + dir * tail);
                    let side = (h - t).cross(h - from).normalize_or_zero();
                    let (h, t) = (h - centre, t - centre);
                    // Outline: twice as wide at 64/255 of the core (additive).
                    let dim = 64.0 / 255.0;
                    let outline = [rgba[0] * dim, rgba[1] * dim, rgba[2] * dim, rgba[3]];
                    let w2 = side * width * 2.0;
                    b.quad([t - w2, h - w2, h + w2, t + w2], rect, outline);
                    let w = side * width;
                    b.quad([t - w, h - w, h + w, t + w], rect, rgba);
                }
                Shape::Quad { corners, rect } => {
                    b.quad(corners.map(|c| c - centre), rect, rgba);
                }
                Shape::Ribbon { to, width, u } => {
                    let to_c = to - centre;
                    let along = to - p.position;
                    let side = along.cross(p.position - from).normalize_or_zero();
                    let (w0, w1) = (side * width[0] / 2.0, side * width[1] / 2.0);
                    // U along the strip, V across.
                    let r = [rect[0] + (rect[2] - rect[0]) * u.0, rect[1], rect[0] + (rect[2] - rect[0]) * u.1, rect[3]];
                    b.quad([at - w0, at + w0, to_c + w1, to_c - w1], r, rgba);
                }
                Shape::Tumble { angles, .. } => {
                    // One corner on the particle; the face across the side
                    // and up axes.
                    let (_, side, up) = crate::map::particles::tumble_axes(angles);
                    let (r, u) = (-side * size, up * size);
                    b.quad([at, at + u, at + u + r, at + r], rect, rgba);
                }
                Shape::Flat { normal, yaw, .. } => {
                    let n = normal.normalize_or_zero();
                    let t = n.any_orthonormal_vector();
                    let t = Quat::from_axis_angle(n, yaw.to_radians()) * t;
                    let (r, u) = (t * size / 2.0, n.cross(t) * size / 2.0);
                    b.quad([at - r - u, at - r + u, at + r + u, at + r - u], rect, rgba);
                }
            }
        }
        let empty = b.positions.is_empty();
        if empty && was_empty {
            continue;
        }
        if let Some(m) = assets.meshes.get_mut(&key) {
            m.2 = empty;
        }
        if let Ok(mut t) = transforms.get_mut(entity) {
            t.translation = centre;
        }
        if let Some(mut mesh) = meshes.get_mut(&handle) {
            b.write(&mut mesh, -forward);
        }
    }
}

/// The texture rectangle a particle shows now.
fn frame_rect(def: &ParticleMaterial, p: &Particle) -> [f32; 4] {
    let Some(seq) = def.sequences.get(p.sequence as usize).filter(|s| !s.is_empty()) else {
        return [0.0, 0.0, 1.0, 1.0];
    };
    let frame = (p.age * p.fps).max(0.0) as usize;
    let frame = if p.frame_loop {
        frame % seq.len()
    } else {
        frame.min(seq.len() - 1)
    };
    seq[frame]
}

#[derive(Default)]
struct QuadBuilder {
    positions: Vec<[f32; 3]>,
    uvs: Vec<[f32; 2]>,
    colors: Vec<[f32; 4]>,
    indices: Vec<u32>,
}

impl QuadBuilder {
    /// Corners: bottom-left, top-left, top-right, bottom-right.
    fn quad(&mut self, corners: [Vec3; 4], [u0, v0, u1, v1]: [f32; 4], rgba: [f32; 4]) {
        let base = self.positions.len() as u32;
        for (p, uv) in corners.iter().zip([[u0, v1], [u0, v0], [u1, v0], [u1, v1]]) {
            self.positions.push(p.to_array());
            self.uvs.push(uv);
            self.colors.push(rgba);
        }
        self.indices
            .extend([base, base + 1, base + 2, base, base + 2, base + 3]);
    }

    fn write(self, mesh: &mut Mesh, normal: Vec3) {
        dust::write_dynamic_mesh(mesh, normal, self.positions, self.uvs, self.colors, self.indices);
    }
}
