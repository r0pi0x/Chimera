//! Dynamic prop shadows as Source draws them (specs/cs_source/shadows_sky.md,
//! "Building one shadow" and "How a receiver is darkened"): each caster's
//! silhouette, seen along the map's shadow direction, goes into a coverage
//! texture; the shadow is clipped onto the world surfaces behind the
//! caster like a decal, and multiplies them toward the shadow colour with
//! a 5-tap blur, fading with distance and vanishing in fog. See shadow.wesl.
//!
//! Dynamic shadows as Video > Advanced's shadow detail asks (`ShadowSettings`:
//! `r_shadows`, `r_shadowrendertotexture`, `r_flashlightdepthtexture`;
//! `apply_shadow_detail` builds them from `ShadowSource` at map load, from
//! what a background load built ahead at the props' places, `ShadowsAhead`,
//! and again at once on a change): at Low Source's blob shadows (`ShadowFrame::blob`, a
//! round picture straight down under each prop), at Medium/High
//! render-to-texture prop shadows; characters' blob shadows at every level
//! (`update_character_blobs`, bodies drawn only). Render-to-texture:
//! silhouettes rasterized into a coverage atlas, clipped onto world surfaces,
//! multiplied over them; redrawn where physics props move by a quarter of a
//! shadow texel or more (`update_prop_shadows`, `moved_visibly`; only the
//! redrawn cell of the atlas image is rewritten), hidden while the logic
//! hides their prop (broken, killed). While a map settles behind a loading
//! screen (`map::MapSettling`) a shadow waits until its prop has come to
//! rest (`RESTED_SECS`) and is redrawn then, once, within a frame's work
//! (`SETTLING_REDRAW_WORK`, counted: `RedrawWork`); the loading screen
//! stays while a frame's work left some for the next (`shadows_settled`,
//! a `MapHooks::settle` hook), so the map is entered with its shadows
//! where its props lie.
//!
//! A resting caster's shadow is its clipped geometry (atlas coordinates and
//! fade per vertex). A moving one (characters' blobs; a physics prop's once
//! it moves) is projected (`Projected`): its frame goes into the frame
//! table (`ShadowFrames`, one storage buffer, the entity's `MeshTag` its
//! slot) and shadow.wesl projects it per pixel onto receiver geometry
//! around it (`region_mesh`, `REGION_MARGIN` past the frame's box), which
//! is rewritten only when the frame leaves that region: a mesh rewritten
//! every frame is freed and allocated again in Bevy's mesh slabs (11% of
//! the render world with six bots, docs/performance.md, "Render
//! performance"). Each shadow keeps one mesh asset, rewritten in place (a
//! new handle a frame left a moving prop's shadow undrawn).
//!
//! Known shortcuts (docs/tech-debt.md): Dynamic shadows (`render::shadows`, shadow
//! detail): a projected shadow's fade is worked out per pixel, a clipped
//! one's per vertex (so where a receiver triangle spans the fade's start,
//! as on a wall beside a blob, the two differ by a few levels, at most 7 of
//! 255 seen); characters cast blob shadows only, at every level (Source draws
//! players render-to-texture; theirs would need the posed silhouette each
//! frame), straight down at Low and along the map's direction at Medium/High,
//! hidden when no world surface lies within reach under them; the blob's box is
//! the collision hull axis-aligned, not turned with the character's yaw
//! (turned, it grew, shrank and slid as bots turned to watch a moving viewer);
//! shadow geometry lies 0.4 units off the surface, above impacts' depth (0.30),
//! so shadows darken bullet holes as Source's do (ours: Source draws them in
//! the decal pass); the blob picture is ours (a round falloff, Source's is a
//! fixed round texture); the 32-a-frame render-to-texture limit and its blob
//! fallback aren't modelled; `r_flashlightdepthtexture` (High) changes nothing;
//! the defaults (Medium) and CS:S's own pick per machine are unconfirmed.
//! Capture to confirm: CS:S at shadow detail Low and High beside a physics
//! barrel and a bot on de_dust2. Why it's OK for now: Reads like CS:S's at a
//! glance.

use bevy::{
    asset::{RenderAssetUsages, embedded_asset},
    image::{ImageAddressMode, ImageFilterMode, ImageSampler, ImageSamplerDescriptor},
    mesh::{Indices, PrimitiveTopology},
    prelude::*,
    reflect::TypePath,
    ecs::{lifecycle::HookContext, world::DeferredWorld},
    mesh::MeshTag,
    render::{
        render_resource::{AsBindGroup, Extent3d, TextureDimension, TextureFormat},
        storage::ShaderBuffer,
    },
    shader::ShaderRef,
};

use super::gpu_table::SlotTable;

use std::sync::Arc;

use crate::map::{
    LogicHidden, MapAlpha, MapData, MapModel, MapPart, MapProp, MapShadows, MapTexture, PhysicsProp, PropIndex, VisClusters,
    vis,
};

/// Source's inch, metres. The sizes below are Source's renderer's, in its
/// units, made metres here: shadows are a look of the map's (any game's
/// characters on it get the same in metres), never the gameplay's unit.
const SOURCE_UNIT: f32 = 0.0254;
/// Added to each projected width, units.
const BLOAT: f32 = 4.0;
const TEXELS_PER_UNIT: f32 = 2.0;
const MIN_TEX: u32 = 16;
const MAX_TEX: u32 = 256;
const ATLAS_WIDTH: u32 = 1024;
/// Most a shadow is lightened at the far end of its reach.
const FALLOFF_AMOUNT: f32 = 240.0 / 255.0;
/// Shadow geometry sits this far off the surface it darkens (units):
/// above the impacts' depth-only copies (`decal::LIFT - DEPTH_DROP`,
/// 0.30 units), which would otherwise fail its depth test, leaving bullet
/// holes unshadowed (lighter than the ground around them; playtest
/// 2026-10-11). Source draws shadows after the surface's decals,
/// multiplying them too (specs/cs_source/shadows_sky.md, per-frame
/// order 4).
pub const LIFT: f32 = 0.4;

/// One caster's projection (engine space, meters).
#[derive(Clone, Debug)]
pub struct ShadowFrame {
    pub origin: Vec3,
    pub x: Vec3,
    pub y: Vec3,
    /// Shadow direction (caster toward receiver).
    pub d: Vec3,
    pub size: Vec2,
    pub falloff_start: f32,
    pub max_dist: f32,
    /// The caster box's faces that face the light: receivers must lie
    /// behind them (n . p <= d).
    pub light_faces: Vec<(Vec3, f32)>,
}

impl ShadowFrame {
    /// The frame for a box (model-space bounds) placed at `translation`,
    /// `rotation`, casting along `d` for `distance` meters past itself.
    pub fn new(bounds: (Vec3, Vec3), translation: Vec3, rotation: Quat, d: Vec3, distance: f32) -> Self {
        let (lo, hi) = bounds;
        let s = hi - lo;
        let axes = [rotation * Vec3::X, rotation * Vec3::Y, rotation * Vec3::Z];
        let sizes = [s.x, s.y, s.z];
        // The longest of the box's axes projected across d sets y.
        let projected = (0..3).map(|i| (axes[i] - d * d.dot(axes[i])) * sizes[i]);
        let y = projected
            .max_by(|a, b| a.length_squared().total_cmp(&b.length_squared()))
            .unwrap_or(Vec3::X)
            .normalize_or(d.any_orthonormal_vector());
        let x = y.cross(d).normalize_or_zero();
        let bloat = BLOAT * SOURCE_UNIT;
        let size = Vec2::new(
            (0..3).map(|i| sizes[i] * axes[i].dot(x).abs()).sum::<f32>() + bloat,
            (0..3).map(|i| sizes[i] * axes[i].dot(y).abs()).sum::<f32>() + bloat,
        );
        let centre = translation + rotation * ((lo + hi) / 2.0);
        let radius = s.length() / 2.0;
        // Back to the box corner furthest against d.
        let m = -(0..3).map(|i| sizes[i] / 2.0 * axes[i].dot(d).abs()).sum::<f32>();
        let origin = centre + d * m;
        let falloff_start = radius - m;
        let light_faces = (0..3)
            .flat_map(|i| [axes[i], -axes[i]].map(|n| (n, n.dot(centre) + sizes[i] / 2.0)))
            .filter(|(n, _)| n.dot(d) < -1e-4)
            .collect();
        Self {
            origin,
            x,
            y,
            d,
            size,
            falloff_start,
            max_dist: distance + falloff_start,
            light_faces,
        }
    }

    /// Shadow space: (x, y, along d), meters.
    pub fn local(&self, p: Vec3) -> Vec3 {
        let r = p - self.origin;
        Vec3::new(self.x.dot(r), self.y.dot(r), self.d.dot(r))
    }

    /// 0..1 texture coordinates within the caster's cell.
    fn uv(&self, p: Vec3) -> Vec2 {
        let l = self.local(p);
        Vec2::new(l.x / self.size.x + 0.5, l.y / self.size.y + 0.5)
    }

    /// Lightening from distance along d.
    fn fade(&self, z: f32) -> f32 {
        let span = (self.max_dist - self.falloff_start).max(1e-4);
        FALLOFF_AMOUNT * ((z - self.falloff_start) / span).clamp(0.0, 1.0)
    }

    /// The box the shadow can fall in (world, axis-aligned): its rectangle
    /// swept along `d` to its reach.
    pub fn bounds(&self) -> (Vec3, Vec3) {
        let (hx, hy) = (self.size.x / 2.0, self.size.y / 2.0);
        let mut lo = Vec3::MAX;
        let mut hi = Vec3::MIN;
        for sx in [-1.0f32, 1.0] {
            for sy in [-1.0f32, 1.0] {
                for z in [0.0, self.max_dist] {
                    let c = self.origin + self.x * (sx * hx) + self.y * (sy * hy) + self.d * z;
                    lo = lo.min(c);
                    hi = hi.max(c);
                }
            }
        }
        (lo, hi)
    }

    /// The frame as shadow.wesl's projected path reads it (`ShadowFrames`,
    /// `FRAME_STRIDE` rows): origin and fade start, x and width, y and
    /// height, d and reach, the atlas cell `rect`, up to three of the
    /// caster's light-facing planes (unused: always passed), the lift and
    /// the fade's amount.
    pub fn gpu(&self, rect: (Vec2, Vec2)) -> [[f32; 4]; FRAME_STRIDE] {
        let mut rows = [[0.0; 4]; FRAME_STRIDE];
        rows[0] = self.origin.extend(self.falloff_start).to_array();
        rows[1] = self.x.extend(self.size.x).to_array();
        rows[2] = self.y.extend(self.size.y).to_array();
        rows[3] = self.d.extend(self.max_dist).to_array();
        rows[4] = [rect.0.x, rect.0.y, rect.1.x, rect.1.y];
        for i in 0..3 {
            rows[5 + i] = match self.light_faces.get(i) {
                Some((n, d)) => n.extend(*d).to_array(),
                None => [0.0, 0.0, 0.0, 1.0],
            };
        }
        rows[8] = [LIFT * SOURCE_UNIT, FALLOFF_AMOUNT, 0.0, 0.0];
        rows
    }
}

/// Cell size for a caster: 2 texels per unit of its largest dimension,
/// rounded up to a power of two, 16..256.
pub fn cell_size(bounds: (Vec3, Vec3)) -> u32 {
    let units = (bounds.1 - bounds.0).max_element() / SOURCE_UNIT;
    ((TEXELS_PER_UNIT * units).ceil().max(1.0) as u32)
        .next_power_of_two()
        .clamp(MIN_TEX, MAX_TEX)
}

/// The caster's silhouette as coverage (0..1) in an `n` x `n` cell: its
/// triangles drawn orthographically along d, both faces, alpha-tested and
/// translucent parts by their texture alpha, overlaps saturating.
pub fn silhouette(
    model: &MapModel,
    textures: &[MapTexture],
    frame: &ShadowFrame,
    translation: Vec3,
    rotation: Quat,
    n: u32,
) -> Vec<f32> {
    silhouette_counted(model, textures, frame, translation, rotation, n).0
}

/// `silhouette`, and the work it took (`RedrawWork`): the triangles it
/// drew and the texels it tested them against.
fn silhouette_counted(
    model: &MapModel,
    textures: &[MapTexture],
    frame: &ShadowFrame,
    translation: Vec3,
    rotation: Quat,
    n: u32,
) -> (Vec<f32>, RedrawWork) {
    let mut out = vec![0.0f32; (n * n) as usize];
    let mut work = RedrawWork::default();
    let scale = n as f32;
    for m in &model.meshes {
        // Additive glows and beams cast no shadow.
        if m.alpha == MapAlpha::Add {
            continue;
        }
        let texture = (m.alpha != MapAlpha::Opaque)
            .then_some(m.texture)
            .flatten()
            .map(|t| &textures[t]);
        for tri in m.indices.as_chunks::<3>().0 {
            let pts = tri.map(|i| {
                let p = translation + rotation * Vec3::from(m.positions[i as usize]);
                frame.uv(p) * scale
            });
            let uvs = tri.map(|i| Vec2::from(m.uvs.get(i as usize).copied().unwrap_or([0.0, 0.0])));
            work.triangles += 1;
            let area = (pts[1] - pts[0]).perp_dot(pts[2] - pts[0]);
            if area.abs() < 1e-9 {
                continue;
            }
            let lo = pts[0].min(pts[1]).min(pts[2]).floor().max(Vec2::ZERO);
            let hi = pts[0].max(pts[1]).max(pts[2]).ceil().min(Vec2::splat(scale - 1.0));
            // Wholly outside the cell (a caster bigger than its box: some
            // community props), or not a number.
            if !(lo.x <= hi.x && lo.y <= hi.y) {
                continue;
            }
            work.texels += ((hi.x.max(lo.x) as u32 - lo.x as u32 + 1) * (hi.y.max(lo.y) as u32 - lo.y as u32 + 1)) as u64;
            for py in lo.y as u32..=hi.y.max(lo.y) as u32 {
                for px in lo.x as u32..=hi.x.max(lo.x) as u32 {
                    let c = Vec2::new(px as f32 + 0.5, py as f32 + 0.5);
                    let w0 = (pts[1] - c).perp_dot(pts[2] - c) / area;
                    let w1 = (pts[2] - c).perp_dot(pts[0] - c) / area;
                    let w2 = 1.0 - w0 - w1;
                    if w0 < 0.0 || w1 < 0.0 || w2 < 0.0 {
                        continue;
                    }
                    let alpha = texture.map_or(1.0, |t| {
                        let uv = uvs[0] * w0 + uvs[1] * w1 + uvs[2] * w2;
                        let tx = (uv.x.rem_euclid(1.0) * t.width as f32) as u32 % t.width.max(1);
                        let ty = (uv.y.rem_euclid(1.0) * t.height as f32) as u32 % t.height.max(1);
                        // A texture without texels (its blocks only) reads opaque.
                        t.rgba8.get(((ty * t.width + tx) * 4 + 3) as usize).map_or(1.0, |a| *a as f32 / 255.0)
                    });
                    let cell = &mut out[(py * n + px) as usize];
                    *cell = (*cell + alpha).min(1.0);
                }
            }
        }
    }
    (out, work)
}

/// What a shadow's redraw took, counted (the same on every machine, unlike
/// its time): the caster's triangles and the atlas texels they were tested
/// against (`silhouette`), and the receiver triangles a new region looked
/// at (`region_mesh`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RedrawWork {
    pub triangles: u64,
    pub texels: u64,
    pub receivers: u64,
}

impl RedrawWork {
    /// In units of a tested texel: about 20 instructions on an optimized
    /// build (docs/performance.md, "A swap's prop meshes and shadows"); a
    /// triangle's own work is put at a few texels' (`TRIANGLE_WORK`,
    /// `RECEIVER_WORK`: the texels are nearly all of a silhouette).
    pub fn units(&self) -> u64 {
        self.texels + TRIANGLE_WORK * self.triangles + RECEIVER_WORK * self.receivers
    }
}

/// A caster triangle projected into its cell, in tested texels (`RedrawWork::units`).
const TRIANGLE_WORK: u64 = 8;

/// A receiver triangle looked at for a region's mesh, in tested texels.
const RECEIVER_WORK: u64 = 8;

/// World triangles, bucketed on a horizontal grid for shadow-box queries.
pub struct Receivers {
    tris: Vec<[Vec3; 3]>,
    cell: f32,
    grid: std::collections::HashMap<(i32, i32), Vec<u32>>,
}

impl Receivers {
    const CELL: f32 = 4.0;

    /// The world's surfaces (not the 3D skybox, decals or overlays).
    pub fn new(data: &MapData) -> Self {
        let mut tris = Vec::new();
        for m in data
            .meshes
            .iter()
            .filter(|m| !m.skybox && m.entity.is_none() && !m.material.starts_with("decal:"))
        {
            for t in m.indices.as_chunks::<3>().0 {
                tris.push(t.map(|i| Vec3::from(m.positions[i as usize])));
            }
        }
        let mut grid: std::collections::HashMap<(i32, i32), Vec<u32>> = Default::default();
        for (i, t) in tris.iter().enumerate() {
            let lo = t[0].min(t[1]).min(t[2]);
            let hi = t[0].max(t[1]).max(t[2]);
            for gx in (lo.x / Self::CELL).floor() as i32..=(hi.x / Self::CELL).floor() as i32 {
                for gz in (lo.z / Self::CELL).floor() as i32..=(hi.z / Self::CELL).floor() as i32 {
                    grid.entry((gx, gz)).or_default().push(i as u32);
                }
            }
        }
        Self {
            tris,
            cell: Self::CELL,
            grid,
        }
    }

    fn near(&self, lo: Vec3, hi: Vec3) -> impl Iterator<Item = &[Vec3; 3]> {
        let mut ids: Vec<u32> = Vec::new();
        for gx in (lo.x / self.cell).floor() as i32..=(hi.x / self.cell).floor() as i32 {
            for gz in (lo.z / self.cell).floor() as i32..=(hi.z / self.cell).floor() as i32 {
                if let Some(v) = self.grid.get(&(gx, gz)) {
                    ids.extend(v);
                }
            }
        }
        ids.sort_unstable();
        ids.dedup();
        ids.into_iter().map(move |i| &self.tris[i as usize])
    }
}

/// Clip a convex polygon to n . p <= d.
fn clip(poly: Vec<Vec3>, n: Vec3, d: f32) -> Vec<Vec3> {
    let mut out = Vec::with_capacity(poly.len() + 2);
    for i in 0..poly.len() {
        let (a, b) = (poly[i], poly[(i + 1) % poly.len()]);
        let (da, db) = (n.dot(a) - d, n.dot(b) - d);
        if da <= 0.0 {
            out.push(a);
        }
        if (da < 0.0) != (db < 0.0) && (da - db).abs() > 1e-12 {
            out.push(a + (b - a) * (da / (da - db)));
        }
    }
    out
}

/// The shadow as geometry on the world: receiver triangles facing the
/// light, behind the caster, within its box and reach; atlas coordinates
/// in UV0 (cell `rect`: origin and size in 0..1) and the fade in the vertex
/// colour's red channel.
pub fn shadow_mesh(frame: &ShadowFrame, receivers: &Receivers, rect: (Vec2, Vec2)) -> Option<Mesh> {
    let (hx, hy) = (frame.size.x / 2.0, frame.size.y / 2.0);
    let corners = [-1.0f32, 1.0].into_iter().flat_map(|sx| {
        [-1.0f32, 1.0].into_iter().flat_map(move |sy| {
            [0.0, frame.max_dist].map(|z| frame.origin + frame.x * (sx * hx) + frame.y * (sy * hy) + frame.d * z)
        })
    });
    let (lo, hi) = corners.fold((Vec3::MAX, Vec3::MIN), |(a, b), c| (a.min(c), b.max(c)));
    let mut planes = vec![
        (frame.x, frame.x.dot(frame.origin) + hx),
        (-frame.x, -frame.x.dot(frame.origin) + hx),
        (frame.y, frame.y.dot(frame.origin) + hy),
        (-frame.y, -frame.y.dot(frame.origin) + hy),
        (-frame.d, -frame.d.dot(frame.origin)),
        (frame.d, frame.d.dot(frame.origin) + frame.max_dist),
    ];
    planes.extend(frame.light_faces.iter().copied());
    let (mut positions, mut uvs, mut colors, mut indices) = (Vec::new(), Vec::new(), Vec::new(), Vec::new());
    for tri in receivers.near(lo, hi) {
        let n = (tri[1] - tri[0]).cross(tri[2] - tri[0]).normalize_or_zero();
        if n.dot(frame.d) >= 0.0 {
            continue;
        }
        if tri[0].min(tri[1]).min(tri[2]).cmpgt(hi).any() || tri[0].max(tri[1]).max(tri[2]).cmplt(lo).any() {
            continue;
        }
        let mut poly = tri.to_vec();
        for (pn, pd) in &planes {
            poly = clip(poly, *pn, *pd);
            if poly.len() < 3 {
                break;
            }
        }
        if poly.len() < 3 {
            continue;
        }
        let base = positions.len() as u32;
        for p in &poly {
            let local = frame.local(*p);
            let uv = Vec2::new(local.x / frame.size.x + 0.5, local.y / frame.size.y + 0.5);
            positions.push((*p + n * (LIFT * SOURCE_UNIT)).to_array());
            uvs.push((rect.0 + uv * rect.1).to_array());
            colors.push([frame.fade(local.z), 0.0, 0.0, 1.0]);
        }
        for i in 1..poly.len() as u32 - 1 {
            indices.extend([base, base + i, base + i + 1]);
        }
    }
    if indices.is_empty() {
        return None;
    }
    let normals = vec![[0.0, 1.0, 0.0]; positions.len()];
    let mut mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::RENDER_WORLD);
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
    mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, colors);
    mesh.insert_indices(Indices::U32(indices));
    Some(mesh)
}

/// How far a moving shadow's receiver geometry reaches past its frame's
/// box, metres (`region_mesh`): rebuilt once the frame leaves it, so a
/// running character's blob gets new geometry every 10-20 frames, not
/// every frame.
pub const REGION_MARGIN: f32 = 1.0;

/// The receiver geometry for a moving shadow (drawn by shadow.wesl's
/// projected path, its frame from `ShadowFrames`): every world triangle
/// facing against `d` clipped to the box `lo..hi`, lifted off its surface
/// by `LIFT` along its normal, the normal in each vertex (the shader takes
/// the lift back off before projecting). None when nothing is there.
pub fn region_mesh(receivers: &Receivers, d: Vec3, lo: Vec3, hi: Vec3) -> Option<Mesh> {
    region_mesh_counted(receivers, d, lo, hi).0
}

/// `region_mesh`, and how many receiver triangles it looked at.
fn region_mesh_counted(receivers: &Receivers, d: Vec3, lo: Vec3, hi: Vec3) -> (Option<Mesh>, u64) {
    let mut seen = 0;
    let planes = [
        (Vec3::X, hi.x),
        (Vec3::NEG_X, -lo.x),
        (Vec3::Y, hi.y),
        (Vec3::NEG_Y, -lo.y),
        (Vec3::Z, hi.z),
        (Vec3::NEG_Z, -lo.z),
    ];
    let (mut positions, mut normals, mut indices) = (Vec::new(), Vec::new(), Vec::new());
    for tri in receivers.near(lo, hi) {
        seen += 1;
        let n = (tri[1] - tri[0]).cross(tri[2] - tri[0]).normalize_or_zero();
        if n.dot(d) >= 0.0 {
            continue;
        }
        if tri[0].min(tri[1]).min(tri[2]).cmpgt(hi).any() || tri[0].max(tri[1]).max(tri[2]).cmplt(lo).any() {
            continue;
        }
        let mut poly = tri.to_vec();
        for (pn, pd) in &planes {
            poly = clip(poly, *pn, *pd);
            if poly.len() < 3 {
                break;
            }
        }
        if poly.len() < 3 {
            continue;
        }
        let base = positions.len() as u32;
        for p in &poly {
            positions.push((*p + n * (LIFT * SOURCE_UNIT)).to_array());
            normals.push(n.to_array());
        }
        for i in 1..poly.len() as u32 - 1 {
            indices.extend([base, base + i, base + i + 1]);
        }
    }
    if indices.is_empty() {
        return (None, seen);
    }
    let mut mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::RENDER_WORLD);
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
    mesh.insert_indices(Indices::U32(indices));
    (Some(mesh), seen)
}

/// A frame's region: its box grown by `REGION_MARGIN`.
fn region_of(frame: &ShadowFrame) -> (Vec3, Vec3) {
    let (lo, hi) = frame.bounds();
    (lo - Vec3::splat(REGION_MARGIN), hi + Vec3::splat(REGION_MARGIN))
}

/// The shadow-frame table's buffer (`ShadowFrames`).
pub const SHADOW_FRAMES: Handle<ShaderBuffer> = bevy::asset::uuid_handle!("8e2f4a63-1b9d-4c7e-a05f-6d3c2e91b4a8");

/// vec4s a frame takes in the table (`ShadowFrame::gpu`).
pub const FRAME_STRIDE: usize = 9;

/// The frame table as a shadow material binds it: always `SHADOW_FRAMES`.
#[derive(Clone, Debug, Deref)]
pub struct ShadowFramesBinding(pub Handle<ShaderBuffer>);

impl Default for ShadowFramesBinding {
    fn default() -> Self {
        Self(SHADOW_FRAMES)
    }
}

/// Moving shadows' frames (characters' blobs, physics props' shadows once
/// they move), one slot each (`Projected`, the entity's `MeshTag`): the
/// shader projects the atlas onto receiver geometry kept while the frame
/// stays in its region (`region_mesh`), so a moving shadow rewrites a
/// table row a frame instead of a mesh (a rewritten mesh is freed and
/// allocated again in Bevy's mesh slabs: docs/performance.md, "Render
/// performance").
#[derive(Resource, Debug, Deref, DerefMut)]
pub struct ShadowFrames(pub SlotTable);

impl Default for ShadowFrames {
    fn default() -> Self {
        Self(SlotTable::new(SHADOW_FRAMES, FRAME_STRIDE))
    }
}

/// Upload the frame table when it changed.
pub fn upload_frames(table: Option<ResMut<ShadowFrames>>, buffers: Option<ResMut<Assets<ShaderBuffer>>>) {
    if let (Some(mut table), Some(mut buffers)) = (table, buffers) {
        table.upload(&mut buffers);
    }
}

/// A shadow drawn by projection: its `ShadowFrames` slot (also its
/// `MeshTag`) and the region its receiver geometry covers.
#[derive(Component, Debug)]
#[component(on_discard = free_frame_slot)]
pub struct Projected {
    pub slot: u32,
    pub region: (Vec3, Vec3),
}

impl Projected {
    /// Whether the region still holds `frame`'s box.
    pub fn holds(&self, frame: &ShadowFrame) -> bool {
        let (lo, hi) = frame.bounds();
        lo.cmpge(self.region.0).all() && hi.cmple(self.region.1).all()
    }
}

fn free_frame_slot(mut world: DeferredWorld, ctx: HookContext) {
    let slot = world.get::<Projected>(ctx.entity).map_or(0, |p| p.slot);
    if slot != 0
        && let Some(mut table) = world.get_resource_mut::<ShadowFrames>()
    {
        table.free(slot);
    }
}

/// Coverage cells packed into one atlas (shelves of equal-height cells).
pub struct Atlas {
    pub width: u32,
    pub height: u32,
    pub coverage: Vec<f32>,
}

impl Atlas {
    /// Pack cells (size, coverage); returns each cell's (origin, size) in
    /// 0..1 atlas coordinates, inset half a texel against bleeding.
    pub fn pack(cells: &[(u32, Vec<f32>)]) -> (Self, Vec<(UVec2, (Vec2, Vec2))>) {
        let mut order: Vec<usize> = (0..cells.len()).collect();
        order.sort_by_key(|&i| std::cmp::Reverse(cells[i].0));
        let mut at = vec![UVec2::ZERO; cells.len()];
        let (mut x, mut y, mut shelf) = (0u32, 0u32, 0u32);
        for &i in &order {
            let n = cells[i].0;
            if x + n > ATLAS_WIDTH {
                x = 0;
                y += shelf;
                shelf = 0;
            }
            at[i] = UVec2::new(x, y);
            x += n;
            shelf = shelf.max(n);
        }
        let height = (y + shelf).max(1);
        let mut coverage = vec![0.0; (ATLAS_WIDTH * height) as usize];
        let mut rects = Vec::with_capacity(cells.len());
        for (i, (n, cov)) in cells.iter().enumerate() {
            for cy in 0..*n {
                for cx in 0..*n {
                    coverage[((at[i].y + cy) * ATLAS_WIDTH + at[i].x + cx) as usize] = cov[(cy * n + cx) as usize];
                }
            }
            let size = Vec2::new(ATLAS_WIDTH as f32, height as f32);
            let origin = (at[i].as_vec2() + 0.5) / size;
            let extent = Vec2::splat(*n as f32 - 1.0) / size;
            rects.push((at[i], (origin, extent)));
        }
        (
            Self {
                width: ATLAS_WIDTH,
                height,
                coverage,
            },
            rects,
        )
    }

    /// Replace one cell's coverage.
    pub fn write(&mut self, at: UVec2, n: u32, coverage: &[f32]) {
        for cy in 0..n {
            for cx in 0..n {
                self.coverage[((at.y + cy) * self.width + at.x + cx) as usize] = coverage[(cy * n + cx) as usize];
            }
        }
    }

    pub fn bytes(&self) -> Vec<u8> {
        self.coverage.iter().map(|c| texel_byte(*c)).collect()
    }

    /// Copy one cell's coverage into the atlas image's bytes (`bytes`'
    /// layout), leaving the rest alone: converting the whole atlas after
    /// each moved prop cost milliseconds a frame on cs_office.
    pub fn write_bytes(&self, bytes: &mut [u8], at: UVec2, n: u32) {
        for y in at.y..(at.y + n).min(self.height) {
            let row = (y * self.width) as usize;
            let (from, to) = (row + at.x as usize, row + (at.x + n).min(self.width) as usize);
            if to > bytes.len() {
                return;
            }
            for (b, c) in bytes[from..to].iter_mut().zip(&self.coverage[from..to]) {
                *b = texel_byte(*c);
            }
        }
    }

    pub fn image(&self) -> Image {
        let data = self.bytes();
        let mut image = Image::new(
            Extent3d {
                width: self.width,
                height: self.height,
                depth_or_array_layers: 1,
            },
            TextureDimension::D2,
            data,
            TextureFormat::R8Unorm,
            // Kept in the main world too: moved props redraw their cells
            // in place (`write_bytes`).
            RenderAssetUsages::default(),
        );
        image.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor {
            address_mode_u: ImageAddressMode::ClampToEdge,
            address_mode_v: ImageAddressMode::ClampToEdge,
            mag_filter: ImageFilterMode::Linear,
            min_filter: ImageFilterMode::Linear,
            ..default()
        });
        image
    }
}

fn texel_byte(c: f32) -> u8 {
    (c.clamp(0.0, 1.0) * 255.0).round() as u8
}

/// One caster's place in the atlas.
#[derive(Clone, Copy, Debug)]
pub struct Cell {
    /// Index in `MapData::props`.
    pub prop: usize,
    pub size: u32,
    pub at: UVec2,
    pub rect: (Vec2, Vec2),
}

/// Everything needed to draw the map's prop shadows.
pub struct BuiltShadows {
    pub atlas: Atlas,
    pub cells: Vec<Cell>,
    /// Per caster prop: its index in `MapData::props` and its shadow mesh.
    pub meshes: Vec<(usize, Mesh)>,
    pub receivers: Receivers,
}

/// Build every caster's shadow (props marked `casts_shadow`, outside the
/// 3D skybox) where the map placed it.
pub fn build(data: &MapData, settings: &MapShadows) -> BuiltShadows {
    let receivers = Receivers::new(data);
    let (atlas, cells, meshes) = build_posed(data, settings, &receivers, &|i| {
        (data.props[i].translation, data.props[i].rotation)
    });
    BuiltShadows {
        atlas,
        cells,
        meshes,
        receivers,
    }
}

/// The map's casters (props marked `casts_shadow`, outside the 3D skybox).
pub fn casters(data: &MapData) -> impl Iterator<Item = (usize, &MapProp)> {
    data.props.iter().enumerate().filter(|(_, p)| p.casts_shadow && !p.skybox)
}

/// Drops what the shadows never read from the data they keep for the map's
/// life (`ShadowSource`; the drawing was made from it at spawn): the
/// gameplay's kit, the extensions, the world's meshes, lightmap, cubemaps,
/// collision and entities, and the texels of every texture they don't
/// sample (`shed_unread_textures`). What stays is what `build_posed` and
/// the casters' silhouettes read: props, models, textures, visibility.
pub fn shed_unread(data: &mut MapData) -> Shed {
    use std::mem::take;
    let parts: Box<dyn std::any::Any + Send> = Box::new((
        take(&mut data.kit),
        take(&mut data.extras),
        take(&mut data.meshes),
        take(&mut data.lightmap),
        take(&mut data.cubemaps),
        (
            take(&mut data.collision_positions),
            take(&mut data.collision_indices),
            take(&mut data.collision_sets),
            take(&mut data.collision_hulls),
            take(&mut data.collision_brushes),
        ),
        take(&mut data.brush_tree),
        take(&mut data.entities),
        take(&mut data.sky_surfaces),
    ));
    let texels = shed_unread_textures(data);
    Shed(parts, texels)
}

/// What `shed_unread` took out of the data, for its caller to free where
/// it costs no frame (`map::drop_later`: a map's meshes, collision and
/// texels are many allocations).
pub struct Shed(#[allow(dead_code)] Box<dyn std::any::Any + Send>, #[allow(dead_code)] Vec<Box<dyn std::any::Any + Send>>);

/// Drops the texels of every texture the shadows never read: a caster's
/// silhouette samples the alpha of its alpha-tested and translucent
/// meshes' textures (`silhouette`), nothing else of `data.textures` is
/// read after the map is spawned (the images were made from it). The
/// shadows keep `data` for the map's life (`ShadowSource`), so this is
/// what keeps the decoded textures from being resident twice. Returns
/// what it took, to be freed by the caller.
pub fn shed_unread_textures(data: &mut MapData) -> Vec<Box<dyn std::any::Any + Send>> {
    let mut read = vec![false; data.textures.len()];
    for (_, prop) in casters(data) {
        for m in &data.models[prop.model].meshes {
            if m.alpha != MapAlpha::Opaque
                && m.alpha != MapAlpha::Add
                && let Some(t) = m.texture
                && let Some(r) = read.get_mut(t)
            {
                *r = true;
            }
        }
    }
    let mut taken: Vec<Box<dyn std::any::Any + Send>> = Vec::with_capacity(data.textures.len());
    for (t, read) in data.textures.iter_mut().zip(read) {
        let mips = std::mem::take(&mut t.mips);
        let bc = t.bc.take();
        let rgba8 = if read { Vec::new() } else { std::mem::take(&mut t.rgba8) };
        taken.push(Box::new((mips, bc, rgba8)));
    }
    taken
}

/// `build` with each caster where `pose` says it is now (translation,
/// rotation; physics props move) onto `receivers`.
#[allow(clippy::type_complexity)]
pub fn build_posed(
    data: &MapData,
    settings: &MapShadows,
    receivers: &Receivers,
    pose: &dyn Fn(usize) -> (Vec3, Quat),
) -> (Atlas, Vec<Cell>, Vec<(usize, Mesh)>) {
    let casters: Vec<(usize, &MapProp, ShadowFrame, (Vec3, Quat))> = casters(data)
        .map(|(i, p)| {
            let model = &data.models[p.model];
            let at = pose(i);
            let frame = ShadowFrame::new(model.bounds, at.0, at.1, settings.direction, settings.distance);
            (i, p, frame, at)
        })
        .collect();
    let cells: Vec<(u32, Vec<f32>)> = casters
        .iter()
        .map(|(_, p, frame, at)| {
            let model = &data.models[p.model];
            let n = cell_size(model.bounds);
            (n, silhouette(model, &data.textures, frame, at.0, at.1, n))
        })
        .collect();
    let (atlas, placed) = Atlas::pack(&cells);
    let cells: Vec<Cell> = casters
        .iter()
        .zip(&placed)
        .zip(&cells)
        .map(|(((i, ..), (at, rect)), (size, _))| Cell {
            prop: *i,
            size: *size,
            at: *at,
            rect: *rect,
        })
        .collect();
    let meshes = casters
        .iter()
        .zip(&cells)
        .filter_map(|((i, _, frame, _), cell)| shadow_mesh(frame, receivers, cell.rect).map(|m| (*i, m)))
        .collect();
    (atlas, cells, meshes)
}

// ---------------------------------------------------------------------------
// Shadow detail and blob shadows.

/// Video > Advanced's shadow detail, CS:S's cvars: `r_shadows` (0: no
/// dynamic shadows), `r_shadowrendertotexture` (0: every shadow a blob,
/// Low; 1: render-to-texture, Medium and High) and
/// `r_flashlightdepthtexture` (High; CS:S has no flashlights, so as
/// Medium here). Defaults: shadows on, render-to-texture, no flashlight
/// depth (the dialog's Medium; CS:S's own pick for the machine is to
/// confirm).
#[derive(Resource, Clone, Copy, Debug, PartialEq)]
pub struct ShadowSettings {
    pub shadows: u8,
    pub render_to_texture: u8,
    pub flashlight_depth: u8,
}

impl Default for ShadowSettings {
    fn default() -> Self {
        Self {
            shadows: 1,
            render_to_texture: 1,
            flashlight_depth: 0,
        }
    }
}

/// How dynamic shadows are drawn.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShadowMode {
    Off,
    /// Source's blob shadows: a round dark patch under each caster,
    /// straight down.
    Blob,
    /// Render-to-texture: each caster's silhouette along the map's
    /// direction (characters: blobs along it, `character_frame`).
    Texture,
}

impl ShadowSettings {
    pub fn mode(&self) -> ShadowMode {
        if self.shadows == 0 {
            ShadowMode::Off
        } else if self.render_to_texture == 0 {
            ShadowMode::Blob
        } else {
            ShadowMode::Texture
        }
    }
}

/// Blob shadows' padding, and their least size, units (spec D:
/// blob_bloat).
const BLOB_BLOAT: f32 = 10.0;
/// A blob's origin is truncated to this grid, units (spec D).
const BLOB_SNAP: f32 = 0.5;
/// The blob picture's size, texels.
pub const BLOB_TEX: u32 = 32;
/// Straight down, engine space.
pub const DOWN: Vec3 = Vec3::NEG_Y;

impl ShadowFrame {
    /// A blob shadow's frame (spec D, "Blob shadows"): the box's widths
    /// across `d` plus `BLOB_BLOAT` (at least that), its origin backed up
    /// twice as far as a render-to-texture shadow's and snapped to
    /// `BLOB_SNAP`.
    pub fn blob(bounds: (Vec3, Vec3), translation: Vec3, rotation: Quat, d: Vec3, distance: f32) -> Self {
        let mut f = Self::new(bounds, translation, rotation, d, distance);
        let blob = BLOB_BLOAT * SOURCE_UNIT;
        f.size = (f.size - Vec2::splat(BLOAT * SOURCE_UNIT) + Vec2::splat(blob)).max(Vec2::splat(blob));
        let centre = translation + rotation * ((bounds.0 + bounds.1) / 2.0);
        // `new` backed up by m (negative along d): once more.
        let m = (f.origin - centre).dot(d);
        f.origin += d * m;
        f.falloff_start -= m;
        f.max_dist = distance + f.falloff_start;
        let snap = BLOB_SNAP * SOURCE_UNIT;
        f.origin = (f.origin / snap).trunc() * snap;
        f
    }
}

/// The blob picture's coverage, `n` x `n`: full in the middle, fading to
/// nothing at the circle's edge (Source's is a fixed round texture; this
/// falloff is ours).
pub fn blob_coverage(n: u32) -> Vec<f32> {
    let n = n.max(2);
    (0..n * n)
        .map(|i| {
            let (x, y) = ((i % n) as f32 + 0.5, (i / n) as f32 + 0.5);
            let r = Vec2::new(x / n as f32 * 2.0 - 1.0, y / n as f32 * 2.0 - 1.0).length();
            let t = ((1.0 - r) / 0.6).clamp(0.0, 1.0);
            t * t * (3.0 - 2.0 * t)
        })
        .collect()
}

/// The blob picture as an atlas of one cell (its rect: the whole).
pub fn blob_atlas() -> Atlas {
    Atlas {
        width: BLOB_TEX,
        height: BLOB_TEX,
        coverage: blob_coverage(BLOB_TEX),
    }
}

/// The whole of the blob picture, as a cell's rect.
pub const BLOB_RECT: (Vec2, Vec2) = (Vec2::ZERO, Vec2::ONE);

/// A character's blob shadow frame: its collision box (`hull`, relative to
/// its origin, metres, whatever its game; Source's player box when it
/// has none), axis-aligned as
/// the box itself is, cast straight down (Low) or along the map's
/// direction (Medium, High: our characters have no render-to-texture
/// shadow; Source falls back to blobs that way past its 32 a frame).
/// Not turned to the character's yaw: the box's projected widths and
/// back corner change with it, so the blob grew, shrank and slid in
/// steps as bots turned to watch a moving viewer (playtest 2026-10-11).
pub fn character_frame(hull: (Vec3, Vec3), at: Vec3, mode: ShadowMode, settings: &MapShadows) -> ShadowFrame {
    let hull = if (hull.1 - hull.0).length_squared() > 0.0 {
        hull
    } else {
        (
            Vec3::new(-16.0, 0.0, -16.0) * SOURCE_UNIT,
            Vec3::new(16.0, 72.0, 16.0) * SOURCE_UNIT,
        )
    };
    let d = if mode == ShadowMode::Blob { DOWN } else { settings.direction };
    ShadowFrame::blob(hull, at, Quat::IDENTITY, d, settings.distance)
}

/// Whether a caster with these model bounds (meters) and an `n` x `n`
/// atlas cell moved far enough from `from` to `to` (translation, rotation)
/// for its shadow to change: some point of its box by more than a quarter
/// of one of its shadow's texels (at most its box plus `BLOAT` across `n`;
/// the receiver blurs over several).
/// The shadow is cast along the map's fixed direction, so only the
/// caster's own movement shifts it.
pub fn moved_visibly(bounds: (Vec3, Vec3), n: u32, from: (Vec3, Quat), to: (Vec3, Quat)) -> bool {
    let (lo, hi) = bounds;
    let texel = ((hi - lo).max_element().max(0.0) + BLOAT * SOURCE_UNIT) / n.max(1) as f32;
    let radius = lo.length().max(hi.length());
    let turn = from.1.angle_between(to.1);
    from.0.distance(to.0) + turn * radius > texel / 4.0
}

/// A caster's silhouette for where its prop is now, into its atlas cell;
/// its frame.
pub fn redraw_silhouette(
    data: &MapData,
    settings: &MapShadows,
    atlas: &mut Atlas,
    cell: &Cell,
    translation: Vec3,
    rotation: Quat,
) -> ShadowFrame {
    redraw_silhouette_counted(data, settings, atlas, cell, translation, rotation).0
}

/// `redraw_silhouette`, and the work it took.
fn redraw_silhouette_counted(
    data: &MapData,
    settings: &MapShadows,
    atlas: &mut Atlas,
    cell: &Cell,
    translation: Vec3,
    rotation: Quat,
) -> (ShadowFrame, RedrawWork) {
    let model = &data.models[data.props[cell.prop].model];
    let frame = ShadowFrame::new(
        model.bounds,
        translation,
        rotation,
        settings.direction,
        settings.distance,
    );
    let (coverage, work) = silhouette_counted(model, &data.textures, &frame, translation, rotation, cell.size);
    atlas.write(cell.at, cell.size, &coverage);
    (frame, work)
}

#[derive(Clone, Copy, Debug, Default, bevy::render::render_resource::ShaderType)]
pub struct ShadowParams {
    /// Shadow colour, linear.
    pub color: Vec4,
    /// One atlas texel (u, v).
    pub texel: Vec2,
    /// Range fog colour (w = 1 when on) and start, end, max density.
    pub fog_color: Vec4,
    pub fog_range: Vec4,
}

#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
pub struct ShadowMaterial {
    #[uniform(0)]
    pub params: ShadowParams,
    #[texture(1)]
    #[sampler(2)]
    pub atlas: Handle<Image>,
    /// Moving shadows' frames (`ShadowFrames`), read by the projected path.
    #[storage(3, read_only)]
    pub frames: ShadowFramesBinding,
}

impl Material for ShadowMaterial {
    fn fragment_shader() -> ShaderRef {
        "embedded://chimera_present/render/shadow.wesl".into()
    }

    fn alpha_mode(&self) -> AlphaMode {
        AlphaMode::Multiply
    }

    /// Sort before every other see-through surface: Source draws shadows
    /// with the world, its decals and overlays already on it (ours draw
    /// before every other blended item, `blend_order`) and before anything
    /// translucent, so smoke, particles and windows cover a shadow behind
    /// them. (For custom materials this only moves the sort key, not the
    /// depth.)
    fn depth_bias(&self) -> f32 {
        -1.0e6
    }
}

pub struct ShadowMaterialPlugin;

impl Plugin for ShadowMaterialPlugin {
    fn build(&self, app: &mut App) {
        embedded_asset!(app, "shadow.wesl");
        app.add_plugins(MaterialPlugin::<ShadowMaterial>::default())
            .init_resource::<ShadowFrames>()
            .add_systems(PostUpdate, upload_frames.before(bevy::asset::AssetEventSystems));
        // The buffer at once: no material is prepared without it.
        app.world_mut().resource_scope(|world, mut table: Mut<ShadowFrames>| {
            if let Some(mut buffers) = world.get_resource_mut::<Assets<ShaderBuffer>>() {
                table.upload(&mut buffers);
            }
        });
    }
}

/// The shadow colour as the receiver shader uses it: sRGB byte to linear
/// with the 2.2 curve (values from 0.95 up count as 1).
pub fn shadow_color(color: [u8; 3]) -> Vec4 {
    let c = color.map(|v| {
        let x = v as f32 / 255.0;
        if x >= 0.95 { 1.0 } else { x.powf(2.2) }
    });
    Vec4::new(c[0], c[1], c[2], 1.0)
}

/// What the map's dynamic shadows are made from (its props and
/// `shadow_control`, the world surfaces they fall on, the static parts'
/// root): kept so shadow detail can rebuild them at once
/// (`apply_shadow_detail`).
#[derive(Resource)]
pub struct ShadowSource {
    pub data: std::sync::Arc<MapData>,
    pub settings: MapShadows,
    pub receivers: std::sync::Arc<Receivers>,
    pub root: Entity,
    pub fog_color: Vec4,
    pub fog_range: Vec4,
    /// Tag shadows with the PVS clusters they touch (`vis`).
    pub tag: bool,
    /// The mode built; None: not yet.
    pub built: Option<ShadowMode>,
    /// The blob picture's material (characters' blobs; props' at Low).
    pub blob_material: Option<Handle<ShadowMaterial>>,
    /// The props' render-to-texture shadows built on the loading thread, at
    /// the props' places in the map (`ShadowsAhead`): the first build takes
    /// them when it is for that mode.
    pub ahead: Option<ShadowsBuilt>,
}

/// Render-to-texture prop shadows built: the mode, the atlas, its image,
/// each caster's cell and each shadow's mesh.
pub type ShadowsBuilt = (ShadowMode, Atlas, Image, Vec<Cell>, Vec<(usize, Mesh)>);

/// A map's shadow work done where it loads (`render::prepare_images`, off
/// the main thread): the receivers, and at render-to-texture the props'
/// shadows as `build_posed` makes them with each prop where the map puts
/// it (as the first build after the spawn does; physics props that moved
/// since are redrawn by `update_prop_shadows`).
pub struct ShadowsAhead {
    pub receivers: std::sync::Arc<Receivers>,
    pub built: Option<ShadowsBuilt>,
}

impl ShadowsAhead {
    /// For `data` at shadow mode `mode`; none for a map without shadows.
    pub fn build(data: &MapData, mode: ShadowMode) -> Option<Self> {
        let settings = data.shadows.as_ref()?;
        let receivers = std::sync::Arc::new(Receivers::new(data));
        let built = (mode == ShadowMode::Texture).then(|| {
            let (atlas, cells, built) = build_posed(data, settings, &receivers, &|i| home(data, i));
            let image = atlas.image();
            (mode, atlas, image, cells, built)
        });
        Some(Self { receivers, built })
    }
}

/// Prop `i` where the map puts it.
fn home(data: &MapData, i: usize) -> (Vec3, Quat) {
    (data.props[i].translation, data.props[i].rotation)
}

/// Physics props' shadows redrawn a frame (`update_prop_shadows`; a redraw is
/// about 0.8 ms, its silhouette over its triangles).
const REDRAWS_PER_FRAME: usize = 6;

/// The redraws' work a frame while the map settles behind a loading screen
/// (`map::MapSettling`), in `RedrawWork::units`: a frame redraws until it
/// has spent this, at least one shadow. Nobody sees them there, and six a
/// frame were half of dm_lockdown's settling frames (docs/performance.md,
/// "A swap's prop meshes and shadows").
pub const SETTLING_REDRAW_WORK: u64 = 400_000;

/// Behind a loading screen a prop's shadow waits until the prop has been
/// this long without moving visibly (`moved_visibly`), seconds: one still
/// falling would be redrawn again and again.
pub const RESTED_SECS: f64 = 0.1;

/// A prop whose shadow waits to be redrawn (`ShadowState::waiting`).
#[derive(Clone, Copy, Debug)]
pub struct Waiting {
    /// Where the prop is now.
    pub pose: (Vec3, Quat),
    /// Where it was when it last moved visibly, and when (the frame
    /// clock's seconds).
    anchor: (Vec3, Quat),
    moved_at: f64,
}

/// Build the map's prop shadows as shadow detail asks: render-to-texture
/// (Medium, High), blobs straight down (Low) or none (`r_shadows 0`);
/// again, at once, when it changes (physics props where they are now,
/// those the logic hid hidden).
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub fn apply_shadow_detail(
    mut commands: Commands,
    source: Option<ResMut<ShadowSource>>,
    detail: Option<Res<ShadowSettings>>,
    old: Query<Entity, Or<(With<PropShadow>, With<CharacterBlob>)>>,
    moved: Query<(&PropIndex, &Transform), With<PhysicsProp>>,
    hidden: Query<&PropIndex, With<LogicHidden>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut images: ResMut<Assets<Image>>,
    mut materials: Option<ResMut<Assets<ShadowMaterial>>>,
) {
    let (Some(mut source), Some(materials)) = (source, materials.as_mut()) else {
        return;
    };
    let mode = detail.map_or(ShadowMode::Texture, |d| d.mode());
    if source.built == Some(mode) {
        return;
    }
    for e in &old {
        commands.entity(e).despawn();
    }
    commands.remove_resource::<ShadowState>();
    source.built = Some(mode);
    source.blob_material = None;
    if mode == ShadowMode::Off {
        info!("prop shadows: off (r_shadows 0)");
        return;
    }
    // Built ahead at the props' homes (the map's first build): drawn as
    // they were there, those that moved since redrawn as any moved prop.
    let ahead = source.ahead.take().filter(|a| a.0 == mode);
    let poses: std::collections::HashMap<usize, (Vec3, Quat)> = match ahead {
        Some(_) => default(),
        None => moved.iter().map(|(i, t)| (i.0, (t.translation, t.rotation))).collect(),
    };
    let hidden: std::collections::HashSet<usize> = hidden.iter().map(|i| i.0).collect();
    let data = source.data.clone();
    let pose = |i: usize| poses.get(&i).copied().unwrap_or((data.props[i].translation, data.props[i].rotation));
    let (color, fog_color, fog_range) = (shadow_color(source.settings.color), source.fog_color, source.fog_range);
    let params = move |texel: Vec2| ShadowParams {
        color,
        texel,
        fog_color,
        fog_range,
    };
    // The blob picture (characters' blobs in every mode).
    let blob = blob_atlas();
    let blob_image = images.add(blob.image());
    let blob_material = materials.add(ShadowMaterial {
        params: params(Vec2::splat(1.0 / BLOB_TEX as f32)),
        atlas: blob_image.clone(),
        frames: default(),
    });
    source.blob_material = Some(blob_material.clone());
    let (atlas, cells, built, material, image) = if mode == ShadowMode::Texture {
        let (atlas, image, cells, built) = match ahead {
            Some((_, atlas, image, cells, built)) => (atlas, image, cells, built),
            None => {
                let (atlas, cells, built) = build_posed(&data, &source.settings, &source.receivers, &pose);
                let image = atlas.image();
                (atlas, image, cells, built)
            }
        };
        let image = images.add(image);
        let material = materials.add(ShadowMaterial {
            params: params(Vec2::new(1.0 / atlas.width as f32, 1.0 / atlas.height as f32)),
            atlas: image.clone(),
            frames: default(),
        });
        (atlas, cells, built, material, image)
    } else {
        let mut cells = Vec::new();
        let mut built = Vec::new();
        for (i, p) in casters(&data) {
            let at = pose(i);
            cells.push(Cell {
                prop: i,
                size: BLOB_TEX,
                at: UVec2::ZERO,
                rect: BLOB_RECT,
            });
            let frame = ShadowFrame::blob(data.models[p.model].bounds, at.0, at.1, DOWN, source.settings.distance);
            if let Some(mesh) = shadow_mesh(&frame, &source.receivers, BLOB_RECT) {
                built.push((i, mesh));
            }
        }
        (blob, cells, built, blob_material, blob_image)
    };
    info!("prop shadows ({mode:?}): {} casters reach the world", built.len());
    let visibility = data.visibility.as_deref().filter(|_| source.tag);
    let mut entities = std::collections::HashMap::new();
    for (prop, mesh) in built {
        // Physics props' shadows move with them: their clusters follow
        // the rebuilt mesh (`update_prop_shadows`).
        let clusters = match (visibility, bevy::camera::primitives::MeshAabb::get_aabb(&mesh)) {
            (Some(v), Some(aabb)) => vis::box_clusters(v, Vec3::from(aabb.min()), Vec3::from(aabb.max())),
            _ => Vec::new(),
        };
        let mut e = commands.spawn((
            Name::new(format!("Shadow of prop {prop}")),
            crate::map::MapPart,
            PropShadow { prop },
            Mesh3d(meshes.add(mesh)),
            MeshMaterial3d(material.clone()),
            bevy::light::NotShadowCaster,
            Transform::default(),
            ChildOf(source.root),
        ));
        if !clusters.is_empty() {
            e.insert(VisClusters::new(clusters));
        }
        if hidden.contains(&prop) {
            e.insert((LogicHidden, Visibility::Hidden));
        }
        entities.insert(prop, e.id());
    }
    commands.insert_resource(ShadowState {
        data: data.clone(),
        settings: source.settings.clone(),
        receivers: source.receivers.clone(),
        blob: mode == ShadowMode::Blob,
        atlas,
        atlas_image: image,
        material,
        built: cells.iter().map(|c| (c.prop, pose(c.prop))).collect(),
        cells: cells.into_iter().map(|c| (c.prop, c)).collect(),
        entities,
        root: source.root,
        waiting: default(),
        last: default(),
        owed: 0,
    });
}

/// A character's blob shadow: whose, and where it was drawn for.
#[derive(Component, Debug)]
pub struct CharacterBlob {
    pub owner: Entity,
    drawn: Vec3,
}

/// Characters' blob shadows (Source's blob for players: our characters
/// have no render-to-texture shadow; straight down at Low, along the
/// map's direction at Medium and High): one per living character whose
/// body is drawn (not your own in first person, not one watched through
/// its eyes), projected (`Projected`): its frame rewritten in
/// `ShadowFrames` when it moves, its receiver geometry only when the frame
/// leaves the region it covers.
#[allow(clippy::type_complexity)]
pub fn update_character_blobs(
    mut commands: Commands,
    source: Option<Res<ShadowSource>>,
    detail: Option<Res<ShadowSettings>>,
    characters: Query<
        (
            Entity,
            &GlobalTransform,
            Option<&crate::core::MovementState>,
            Option<&crate::core::Health>,
            Option<&Children>,
        ),
        (With<crate::core::Intent>, Without<crate::core::Spectating>, Without<crate::map::ragdoll::Ragdolled>),
    >,
    bodies: Query<&Visibility, (With<crate::render::bodies::CharacterBody>, Without<CharacterBlob>)>,
    mut blobs: Query<
        (Entity, &mut CharacterBlob, &Mesh3d, &mut Visibility, &mut Projected),
        Without<crate::render::bodies::CharacterBody>,
    >,
    mut meshes: ResMut<Assets<Mesh>>,
    frames: Option<ResMut<ShadowFrames>>,
) {
    let mode = detail.map_or(ShadowMode::Texture, |d| d.mode());
    let material = source.as_ref().and_then(|s| s.blob_material.clone());
    let (Some(source), Some(material), Some(mut frames), false) = (source, material, frames, mode == ShadowMode::Off)
    else {
        for (e, ..) in &blobs {
            commands.entity(e).despawn();
        }
        return;
    };
    let mut have: std::collections::HashMap<Entity, Entity> = blobs.iter().map(|(e, b, ..)| (b.owner, e)).collect();
    for (owner, at, movement, health, children) in &characters {
        let alive = health.is_none_or(|h| h.current > 0.0);
        let drawn = children
            .into_iter()
            .flatten()
            .any(|c| bodies.get(*c).is_ok_and(|v| *v != Visibility::Hidden));
        if !alive || !drawn {
            continue;
        }
        let pose = at.translation();
        let blob = have.remove(&owner);
        if let Some(Ok((_, b, ..))) = blob.map(|b| blobs.get(b))
            && b.drawn.distance(pose) < 0.005
        {
            continue;
        }
        let hull = movement.map_or((Vec3::ZERO, Vec3::ZERO), |m| (m.hull_min, m.hull_max));
        let frame = character_frame(hull, pose, mode, &source.settings);
        match blob.and_then(|b| blobs.get_mut(b).ok()) {
            Some((_, mut b, handle, mut vis, mut projected)) => {
                b.drawn = pose;
                frames.set(projected.slot, &frame.gpu(BLOB_RECT));
                if !projected.holds(&frame) {
                    let region = region_of(&frame);
                    projected.region = region;
                    match region_mesh(&source.receivers, frame.d, region.0, region.1) {
                        Some(mesh) => {
                            vis.set_if_neq(Visibility::Inherited);
                            let _ = meshes.insert(handle.id(), mesh);
                        }
                        // Nothing under it within reach (jumping, falling).
                        None => {
                            vis.set_if_neq(Visibility::Hidden);
                        }
                    }
                }
            }
            None => {
                let region = region_of(&frame);
                let Some(mesh) = region_mesh(&source.receivers, frame.d, region.0, region.1) else {
                    continue;
                };
                let slot = frames.alloc();
                frames.set(slot, &frame.gpu(BLOB_RECT));
                commands.spawn((
                    Name::new("Character blob shadow"),
                    crate::map::MapPart,
                    CharacterBlob { owner, drawn: pose },
                    Projected { slot, region },
                    MeshTag::new(slot),
                    Mesh3d(meshes.add(mesh)),
                    MeshMaterial3d(material.clone()),
                    bevy::light::NotShadowCaster,
                    Transform::default(),
                    Visibility::Inherited,
                    ChildOf(source.root),
                ));
            }
        }
    }
    // Characters gone, dead or not drawn.
    for (_, blob) in have {
        commands.entity(blob).despawn();
    }
}

/// What it takes to redraw prop shadows when props move (built by
/// `apply_shadow_detail`): render-to-texture, or blobs (`blob`:
/// one picture, only the meshes move).
#[derive(Resource)]
pub struct ShadowState {
    pub data: Arc<MapData>,
    pub settings: MapShadows,
    pub receivers: Arc<Receivers>,
    pub blob: bool,
    pub atlas: Atlas,
    pub atlas_image: Handle<Image>,
    pub material: Handle<ShadowMaterial>,
    pub cells: std::collections::HashMap<usize, Cell>,
    pub entities: std::collections::HashMap<usize, Entity>,
    pub root: Entity,
    /// Where each caster was when its shadow was last drawn.
    pub built: std::collections::HashMap<usize, (Vec3, Quat)>,
    /// The props that moved and whose shadows wait for their redraw
    /// (`update_prop_shadows`), each at its latest pose.
    pub waiting: std::collections::BTreeMap<usize, Waiting>,
    /// The last frame's redraws: how many, and their work.
    pub last: (usize, RedrawWork),
    /// The props at rest whose redraws the last frame's work left for the
    /// next (behind a loading screen; `shadows_settled`).
    pub owed: usize,
}

impl ShadowState {
    /// Whether prop `index`'s shadow is drawn for where the prop is now
    /// (`pose`), near enough (`moved_visibly`).
    fn drawn_for(&self, index: usize, cell: &Cell, pose: (Vec3, Quat)) -> bool {
        let model = &self.data.models[self.data.props[index].model];
        self.built.get(&index).is_some_and(|&from| !moved_visibly(model.bounds, cell.size, from, pose))
    }

    /// The waiting props that have come to rest (`RESTED_SECS` at `now`)
    /// away from where their shadows are drawn.
    pub fn resting(&self, now: f64) -> usize {
        self.waiting
            .iter()
            .filter(|(index, w)| {
                now - w.moved_at >= RESTED_SECS
                    && self.cells.get(index).is_some_and(|cell| !self.drawn_for(**index, cell, w.pose))
            })
            .count()
    }
}

/// `map::MapHooks::settle`: a map behind its loading screen has its props'
/// shadows where the props came to rest, none left over by a frame's work
/// (those still moving, or come to rest this frame, are redrawn in play,
/// as ever).
pub fn shadows_settled(world: &mut World) -> bool {
    world.get_resource::<ShadowState>().is_none_or(|s| s.owed == 0)
}

/// Redraw the shadows of physics props that moved (silhouette and mesh);
/// a prop the logic hid (broken, killed) hides its shadow until it shows
/// again (a round restart). Props that only jitter in place (resting
/// physics props that never fall asleep: seven on cs_office, every frame)
/// keep their shadow until they have moved by a quarter of a shadow texel
/// (`moved_visibly`), and a redrawn cell updates only its part of
/// the atlas image: with seven props rebuilt and the whole atlas converted
/// every frame this cost 16 ms a frame there. At most `REDRAWS_PER_FRAME` a
/// frame, the others waiting at their latest pose (a map's props settling
/// in its first seconds). While the map settles behind a loading screen
/// (`map::MapSettling`) only props that have come to rest are redrawn
/// (`RESTED_SECS`), `SETTLING_REDRAW_WORK` a frame; the loading screen
/// stays while that left some for the next frame (`shadows_settled`), so
/// the map is entered with the same shadows as before.
#[allow(clippy::type_complexity)]
pub fn update_prop_shadows(
    mut commands: Commands,
    state: Option<ResMut<ShadowState>>,
    moved: Query<(&PropIndex, &Transform), (With<PhysicsProp>, Changed<Transform>)>,
    hidden: Query<&PropIndex, Added<LogicHidden>>,
    mut shown: RemovedComponents<LogicHidden>,
    props: Query<&PropIndex, Without<LogicHidden>>,
    mut shadow_vis: Query<Option<&mut VisClusters>, With<PropShadow>>,
    vis: Option<Res<vis::ActiveVisibility>>,
    shadow_meshes: Query<&Mesh3d, With<PropShadow>>,
    mut projected: Query<&mut Projected, With<PropShadow>>,
    mut images: ResMut<Assets<Image>>,
    mut meshes: ResMut<Assets<Mesh>>,
    frames: Option<ResMut<ShadowFrames>>,
    settling: Option<Res<crate::map::MapSettling>>,
    time: Res<Time>,
    mut settled: Local<(usize, u64)>,
) {
    let (Some(mut state), Some(mut frames)) = (state, frames) else {
        shown.clear();
        return;
    };
    let state = &mut *state;
    for index in &hidden {
        if let Some(e) = state.entities.get(&index.0) {
            commands.entity(*e).insert((LogicHidden, Visibility::Hidden));
        }
    }
    for node in shown.read() {
        let Ok(index) = props.get(node) else { continue };
        if let Some(e) = state.entities.get(&index.0).copied() {
            let on = shadow_vis.get(e).ok().flatten().is_none_or(|v| v.potentially_visible);
            commands.entity(e).remove::<LogicHidden>().insert(if on {
                Visibility::Inherited
            } else {
                Visibility::Hidden
            });
        }
    }
    let mut redrawn: Vec<Cell> = Vec::new();
    // A map's physics props settling in its first seconds would all redraw
    // at once: a frame's share, each at its latest pose; the rest wait.
    let now = time.elapsed_secs_f64();
    for (index, t) in &moved {
        let Some(cell) = state.cells.get(&index.0) else { continue };
        let pose = (t.translation, t.rotation);
        let bounds = state.data.models[state.data.props[index.0].model].bounds;
        match state.waiting.entry(index.0) {
            std::collections::btree_map::Entry::Vacant(e) => {
                e.insert(Waiting {
                    pose,
                    anchor: pose,
                    moved_at: now,
                });
            }
            std::collections::btree_map::Entry::Occupied(mut e) => {
                let w = e.get_mut();
                w.pose = pose;
                if moved_visibly(bounds, cell.size, w.anchor, pose) {
                    (w.anchor, w.moved_at) = (pose, now);
                }
            }
        }
    }
    let settling = settling.is_some();
    let (mut redraws, mut spent, mut owed) = (0, RedrawWork::default(), 0);
    let waiting: Vec<(usize, Waiting)> = state.waiting.iter().map(|(i, w)| (*i, *w)).collect();
    for (index, w) in waiting {
        if !settling && redraws == REDRAWS_PER_FRAME {
            break;
        }
        let (translation, rotation) = w.pose;
        let Some(cell) = state.cells.get(&index).copied() else {
            state.waiting.remove(&index);
            continue;
        };
        if state.drawn_for(index, &cell, w.pose) {
            state.waiting.remove(&index);
            continue;
        }
        // Behind the loading screen: once it rests, within the frame's work.
        if settling && now - w.moved_at < RESTED_SECS {
            continue;
        }
        if settling && redraws > 0 && spent.units() >= SETTLING_REDRAW_WORK {
            owed += 1;
            continue;
        }
        state.waiting.remove(&index);
        redraws += 1;
        let model = &state.data.models[state.data.props[index].model];
        state.built.insert(index, (translation, rotation));
        let (frame, rect) = if state.blob {
            let frame = ShadowFrame::blob(
                model.bounds,
                translation,
                rotation,
                DOWN,
                state.settings.distance,
            );
            (frame, BLOB_RECT)
        } else {
            redrawn.push(cell);
            let (frame, work) = redraw_silhouette_counted(
                &state.data,
                &state.settings,
                &mut state.atlas,
                &cell,
                translation,
                rotation,
            );
            spent.triangles += work.triangles;
            spent.texels += work.texels;
            (frame, cell.rect)
        };
        let existing = state.entities.get(&index).copied();
        // A projected shadow whose region still holds the frame: only its
        // table row changes.
        if let Some(e) = existing
            && let Ok(projected) = projected.get(e)
            && projected.holds(&frame)
        {
            frames.set(projected.slot, &frame.gpu(rect));
            continue;
        }
        let region = region_of(&frame);
        let (mesh, seen) = region_mesh_counted(&state.receivers, frame.d, region.0, region.1);
        spent.receivers += seen;
        match (mesh, existing) {
            (Some(mesh), Some(e)) => {
                // Projected from now on (a resting prop's shadow is its
                // clipped mesh): its slot, the frame, the region's mesh.
                let slot = match projected.get(e) {
                    Ok(p) => p.slot,
                    Err(_) => {
                        let slot = frames.alloc();
                        commands.entity(e).insert((Projected { slot, region }, MeshTag::new(slot)));
                        slot
                    }
                };
                if let Ok(mut p) = projected.get_mut(e) {
                    p.region = region;
                }
                frames.set(slot, &frame.gpu(rect));
                // Hidden where the camera can't see where it now lies.
                if let Some(v) = vis.as_ref()
                    && let Ok(Some(mut tag)) = shadow_vis.get_mut(e)
                {
                    let clusters = vis::box_clusters(&v.0, region.0, region.1);
                    if !clusters.is_empty() && *tag.clusters != clusters[..] {
                        tag.clusters = clusters.into_boxed_slice();
                    }
                }
                match shadow_meshes.get(e) {
                    // The same mesh asset, rewritten: a new handle left the
                    // shadow undrawn while its prop moved (Bevy
                    // re-specializes an entity whose `Mesh3d` changed and
                    // skipped it while the new mesh wasn't prepared yet).
                    // (The handle keeps the asset alive: the id stays valid.)
                    Ok(handle) => {
                        let _ = meshes.insert(handle.id(), mesh);
                    }
                    Err(_) => {
                        commands.entity(e).insert(Mesh3d(meshes.add(mesh)));
                    }
                }
            }
            (Some(mesh), None) => {
                let slot = frames.alloc();
                frames.set(slot, &frame.gpu(rect));
                let e = commands
                    .spawn((
                        Name::new(format!("Shadow of prop {}", index)),
                        MapPart,
                        PropShadow { prop: index },
                        Projected { slot, region },
                        MeshTag::new(slot),
                        Mesh3d(meshes.add(mesh)),
                        MeshMaterial3d(state.material.clone()),
                        bevy::light::NotShadowCaster,
                        Transform::default(),
                        ChildOf(state.root),
                    ))
                    .id();
                state.entities.insert(index, e);
            }
            (None, Some(e)) => {
                commands.entity(e).despawn();
                state.entities.remove(&index);
            }
            (None, None) => {}
        }
    }
    state.last = (redraws, spent);
    state.owed = owed;
    // What a map's settling redrew, once it is entered.
    if settling {
        *settled = (settled.0 + redraws, settled.1 + spent.units());
    } else if settled.0 > 0 {
        info!(
            "prop shadows: {} redrawn behind the loading screen ({} of work), {} waiting",
            settled.0,
            settled.1,
            state.waiting.len()
        );
        *settled = (0, 0);
    }
    if !redrawn.is_empty()
        && let Some(mut image) = images.get_mut(&state.atlas_image)
    {
        match image.data.as_mut() {
            Some(bytes) => {
                for cell in &redrawn {
                    state.atlas.write_bytes(bytes, cell.at, cell.size);
                }
            }
            None => image.data = Some(state.atlas.bytes()),
        }
    }
}

/// A prop's dynamic shadow on the world (index into `MapData::props`).
#[derive(Component, Debug)]
pub struct PropShadow {
    pub prop: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn jitter_keeps_the_shadow() {
        // A 32-unit crate: a 64-texel cell of about half a unit (1.3 cm)
        // each.
        let half = Vec3::splat(16.0 * SOURCE_UNIT);
        let bounds = (-half, half);
        let n = cell_size(bounds);
        let at = (Vec3::new(1.0, 0.5, 2.0), Quat::from_rotation_y(0.4));
        let nudged = |dt: Vec3, turn: f32| (at.0 + dt, at.1 * Quat::from_rotation_x(turn));
        // Settling jitter seen on cs_office: 0.1 mm, 0.01 degrees.
        assert!(!moved_visibly(bounds, n, at, nudged(Vec3::splat(1e-4), 2e-4)));
        // A centimetre or a degree is a different shadow.
        assert!(moved_visibly(bounds, n, at, nudged(Vec3::X * 0.01, 0.0)));
        assert!(moved_visibly(bounds, n, at, nudged(Vec3::ZERO, 1f32.to_radians())));
    }

    #[test]
    fn redrawing_a_cell_matches_the_whole_atlas() {
        let cells = [(16, vec![0.25; 256]), (32, vec![0.5; 1024]), (16, vec![1.0; 256])];
        let (mut atlas, placed) = Atlas::pack(&cells);
        let mut bytes = atlas.bytes();
        let (at, n) = (placed[2].0, 16);
        atlas.write(at, n, &[0.75; 256]);
        atlas.write_bytes(&mut bytes, at, n);
        assert_eq!(bytes, atlas.bytes());
    }

    /// Blob shadows (spec D): round, at least 10 units across, under the
    /// caster straight down, reaching past it twice as far back.
    #[test]
    fn blobs_are_round_and_sized_as_source_s() {
        let c = blob_coverage(BLOB_TEX);
        let at = |x: u32, y: u32| c[(y * BLOB_TEX + x) as usize];
        assert!(at(16, 16) > 0.99 && at(0, 0) == 0.0 && at(0, 16) < 0.05, "full in the middle, none at the edge");
        assert!((at(8, 16) - at(16, 8)).abs() < 1e-6, "round");
        // A 2-unit pebble: the least size; a player box: its width plus 10.
        let u = SOURCE_UNIT;
        let pebble = ShadowFrame::blob((Vec3::splat(-u), Vec3::splat(u)), Vec3::ZERO, Quat::IDENTITY, DOWN, 1.0);
        assert!((pebble.size - Vec2::splat(12.0 * u)).abs().max_element() < 1e-4, "{}", pebble.size / u);
        let hull = (Vec3::new(-16.0, 0.0, -16.0) * u, Vec3::new(16.0, 72.0, 16.0) * u);
        let player = ShadowFrame::blob(hull, Vec3::ZERO, Quat::IDENTITY, DOWN, 1.0);
        assert!((player.size - Vec2::splat(42.0 * u)).abs().max_element() < 1e-4, "{}", player.size / u);
        let rtt = ShadowFrame::new(hull, Vec3::ZERO, Quat::IDENTITY, DOWN, 1.0);
        let centre = Vec3::new(0.0, 36.0 * u, 0.0);
        assert!(((player.origin - centre).dot(DOWN) - 2.0 * (rtt.origin - centre).dot(DOWN)).abs() < BLOB_SNAP * u);
    }

    /// Shadows darken bullet holes as the ground around them (Source
    /// draws shadows after the surface's decals): the shadow lies above
    /// the depth the impacts write, or it fails the depth test there.
    #[test]
    fn shadows_draw_over_impact_decals() {
        let u = SOURCE_UNIT;
        let depth_copy = crate::map::decal::LIFT - crate::map::decal::DEPTH_DROP;
        assert!(LIFT * u > depth_copy + 0.02 * u, "shadow {LIFT} units, impacts' depth {} units", depth_copy / u);
        // And on a real floor: the shadow's vertices sit that high.
        let data = floor_map();
        let receivers = Receivers::new(&data);
        let s = data.shadows.clone().unwrap();
        let frame = character_frame((Vec3::ZERO, Vec3::ZERO), Vec3::new(0.0, 0.0, 0.0), ShadowMode::Texture, &s);
        let mesh = shadow_mesh(&frame, &receivers, BLOB_RECT).expect("a shadow on the floor");
        let Some(bevy::mesh::VertexAttributeValues::Float32x3(p)) = mesh.attribute(Mesh::ATTRIBUTE_POSITION) else {
            panic!("positions")
        };
        assert!(p.iter().all(|v| v[1] > depth_copy), "under the impacts' depth");
    }

    /// A tiny map: a floor and the shadow settings.
    fn floor_map() -> MapData {
        MapData {
            name: "test:blobs".into(),
            shadows: Some(MapShadows {
                direction: Vec3::new(0.3, -1.0, 0.0).normalize(),
                color: [100, 100, 100],
                distance: 2.0,
            }),
            meshes: vec![crate::map::MapMesh {
                material: "test/floor".into(),
                positions: vec![[-10.0, 0.0, -10.0], [-10.0, 0.0, 10.0], [10.0, 0.0, 10.0], [10.0, 0.0, -10.0]],
                normals: vec![[0.0, 1.0, 0.0]; 4],
                uvs: vec![[0.0, 0.0]; 4],
                lightmap_uvs: vec![[0.0, 0.0]; 4],
                indices: vec![0, 1, 2, 0, 2, 3],
                color: [200, 180, 150],
                ..default()
            }],
            ..default()
        }
    }

    /// Characters whose body is drawn get a blob under them (straight down
    /// at Low, along the map's direction at Medium); none once the body is
    /// hidden (your own in first person) or `r_shadows 0`.
    #[test]
    fn drawn_characters_get_blob_shadows() {
        // Just the shadow systems over the map's floor (no simulation).
        let mut app = App::new();
        app.add_plugins((
            MinimalPlugins,
            TransformPlugin,
            AssetPlugin::default(),
            bevy::mesh::MeshPlugin,
        ))
        .init_asset::<Image>()
        .init_asset::<ShadowMaterial>()
        .init_resource::<ShadowFrames>()
        .add_systems(Update, (apply_shadow_detail, update_character_blobs).chain());
        let data = std::sync::Arc::new(floor_map());
        let receivers = std::sync::Arc::new(Receivers::new(&data));
        let root = app.world_mut().spawn((Transform::default(), Visibility::default())).id();
        app.insert_resource(ShadowSource {
            receivers: receivers.clone(),
            settings: data.shadows.clone().unwrap(),
            data,
            root,
            fog_color: Vec4::ZERO,
            fog_range: Vec4::ZERO,
            tag: false,
            built: None,
            blob_material: None,
            ahead: None,
        });
        struct Sim {
            app: App,
        }
        impl Sim {
            fn ticks(&mut self, n: usize) {
                for _ in 0..n {
                    self.app.update();
                }
            }
        }
        let mut sim = Sim { app };
        let at = Vec3::new(2.0, 0.05, 1.0);
        let character = sim
            .app
            .world_mut()
            .spawn((
                crate::core::Intent::default(),
                crate::core::Health::default(),
                Transform::from_translation(at),
            ))
            .id();
        let body = sim
            .app
            .world_mut()
            .spawn((
                crate::render::bodies::CharacterBody {
                    held_parts: Vec::new(),
                    skeleton: 0,
                    model: 0,
                    skin: 0,
                    body: 0,
                    joints: Vec::new(),
                    held: None,
                },
                Visibility::Inherited,
                ChildOf(character),
            ))
            .id();
        // The blob as drawn: its frame from the table, clipped onto the
        // floor as the shader projects it (`shadow_mesh`).
        let drawn = move |world: &mut World, slot: u32| {
            let rows = world.resource::<ShadowFrames>().get(slot).to_vec();
            let v = |i: usize| Vec3::new(rows[i][0], rows[i][1], rows[i][2]);
            let frame = ShadowFrame {
                origin: v(0),
                x: v(1),
                y: v(2),
                d: v(3),
                size: Vec2::new(rows[1][3], rows[2][3]),
                falloff_start: rows[0][3],
                max_dist: rows[3][3],
                light_faces: (5..8).map(|i| (v(i), rows[i][3])).filter(|(n, _)| *n != Vec3::ZERO).collect(),
            };
            let mesh = shadow_mesh(&frame, &receivers, BLOB_RECT).expect("the blob on the floor");
            let aabb = bevy::camera::primitives::MeshAabb::get_aabb(&mesh).unwrap();
            (Vec3::from(aabb.center), Vec3::from(aabb.half_extents))
        };
        let blob_centre = |sim: &mut Sim| {
            let world = sim.app.world_mut();
            let found: Vec<u32> = world
                .query::<(&CharacterBlob, &Projected, &Visibility)>()
                .iter(world)
                .filter(|(_, _, v)| **v != Visibility::Hidden)
                .map(|(_, p, _)| p.slot)
                .collect();
            found.first().map(|&slot| drawn(world, slot).0)
        };
        let set = |sim: &mut Sim, shadows: u8, rtt: u8| {
            sim.app.insert_resource(ShadowSettings {
                shadows,
                render_to_texture: rtt,
                flashlight_depth: 0,
            });
            sim.ticks(3);
        };
        set(&mut sim, 1, 0);
        let low = blob_centre(&mut sim).expect("a blob at Low");
        assert!(low.xz().distance(at.xz()) < 0.05, "straight down: {low}");
        set(&mut sim, 1, 1);
        let medium = blob_centre(&mut sim).expect("a blob at Medium");
        assert!(medium.x > at.x + 0.05, "along the map's direction: {medium}");
        // A bot standing still, turning to watch a viewer walking round
        // it: one shadow throughout (it grew, shrank and slid in steps).
        let blob_box = |sim: &mut Sim| {
            let world = sim.app.world_mut();
            let slot = world.query::<(&CharacterBlob, &Projected)>().single(world).unwrap().1.slot;
            drawn(world, slot)
        };
        let first = blob_box(&mut sim);
        for step in 1..=24 {
            let yaw = step as f32 * 15f32.to_radians();
            let mut e = sim.app.world_mut().entity_mut(character);
            e.get_mut::<crate::core::Intent>().unwrap().yaw = yaw;
            e.insert(Transform::from_translation(at).with_rotation(Quat::from_rotation_y(yaw)));
            sim.ticks(1);
            let now = blob_box(&mut sim);
            assert!(
                now.0.abs_diff_eq(first.0, 1e-5) && now.1.abs_diff_eq(first.1, 1e-5),
                "yaw {step}x15: {now:?} vs {first:?}"
            );
        }
        sim.app.world_mut().entity_mut(body).insert(Visibility::Hidden);
        sim.ticks(2);
        assert!(blob_centre(&mut sim).is_none(), "no body drawn, no blob");
        sim.app.world_mut().entity_mut(body).insert(Visibility::Inherited);
        set(&mut sim, 0, 1);
        assert!(blob_centre(&mut sim).is_none(), "r_shadows 0");
    }
}
