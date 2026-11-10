//! Moving models lit as Source lights them: from the map's baked light at
//! their position (`LightField`: ambient cube and nearby lights), re-sampled
//! when they move, instead of real-time lights and shadow maps. Character
//! bodies, the weapons they hold and loose items use it; each has its own
//! material instances.
//!
//! Moving models (character bodies, held and loose weapons) lit from the baked
//! `LightField` where they are (`ProbeLit`, own `PropMaterial` instances,
//! re-sampled after moving 0.25 m), as Source lights models; no shadow maps.
//! A map's physics prop keeps the light baked into its vertices while it
//! rests (shared materials, so its draws batch) and becomes one of these
//! the moment it has moved or turned (`PropLightHome`, `light_moved_props`):
//! only static props stay as baked.
//!
//! The probe light itself is not in the materials: each lit entity has a
//! slot of the probe table (`ProbeTable`, one storage buffer every prop
//! material binds; `PropParams::probe_slot` says which), rewritten as it
//! moves. A changed material is prepared again (a uniform buffer and a bind
//! group a material, eight a body), which cost 14% of the render world with
//! six bodies moving (docs/performance.md, "Render performance").
//!
//! Known shortcuts (docs/tech-debt.md): a moved prop's skin change goes
//! back to the baked light (only `PropVariants`' shared materials know the
//! skins; no stock prop_physics changes skin).

use bevy::{
    ecs::{lifecycle::HookContext, world::DeferredWorld},
    prelude::*,
    render::storage::ShaderBuffer,
};

use super::{gpu_table::SlotTable, prop_material::PropMaterial};
use crate::map::{LightField, LightProbe};

/// The probe table's buffer (`ProbeTable`).
pub const PROBE_TABLE: Handle<ShaderBuffer> = bevy::asset::uuid_handle!("3b7c9e21-4d6a-4f0e-8b55-2a91c6d7e310");

/// vec4s a probe takes in the table: the ambient cube (6), four lights'
/// directions and colours (4 + 4), as `PropParams`' `probe_cube`,
/// `probe_light_dir` and `probe_light_color`.
pub const PROBE_STRIDE: usize = 14;

/// The probe table as a prop material binds it (`PropMaterial::probes`):
/// always `PROBE_TABLE`.
#[derive(Clone, Debug, Deref)]
pub struct ProbeTableBinding(pub Handle<ShaderBuffer>);

impl Default for ProbeTableBinding {
    fn default() -> Self {
        Self(PROBE_TABLE)
    }
}

/// Moving models' probe light, one slot each (`ProbeLit`, a view model's
/// body), read by prop.wesl at `PropParams::probe_slot`; face parts' faces
/// take slots of it too (`bodies::FaceSlot`, `PropParams::face_slot`: the
/// first four rows), rewritten every frame as heads move.
#[derive(Resource, Debug, Deref, DerefMut)]
pub struct ProbeTable(pub SlotTable);

impl Default for ProbeTable {
    fn default() -> Self {
        Self(SlotTable::new(PROBE_TABLE, PROBE_STRIDE))
    }
}

impl ProbeTable {
    /// The table, its upload each frame, and its buffer at once (so no
    /// material is ever prepared without it: a buffer too small for one
    /// probe fails the draw).
    pub fn add_to(app: &mut App) {
        app.init_resource::<ProbeTable>()
            .add_systems(PostUpdate, upload_probes.before(bevy::asset::AssetEventSystems));
        let world = app.world_mut();
        world.resource_scope(|world, mut table: Mut<ProbeTable>| {
            if let Some(mut buffers) = world.get_resource_mut::<Assets<ShaderBuffer>>() {
                table.upload(&mut buffers);
            }
        });
    }

    /// Light slot `slot` with `probe` (times `scale`).
    pub fn set_probe(&mut self, slot: u32, probe: &LightProbe, scale: f32) {
        self.0.set(slot, &probe_rows(probe, scale));
    }

    /// The probe material `m` is drawn with: its slot's, or its own.
    pub fn drawn(&self, m: &PropMaterial) -> [[f32; 4]; PROBE_STRIDE] {
        let mut rows = [[0.0; 4]; PROBE_STRIDE];
        if m.params.probe_slot != 0 {
            rows.copy_from_slice(self.get(m.params.probe_slot));
        } else {
            let p = &m.params;
            for (r, v) in rows.iter_mut().zip(p.probe_cube.iter().chain(&p.probe_light_dir).chain(&p.probe_light_color)) {
                *r = v.to_array();
            }
        }
        rows
    }
}

/// A probe as the shader reads it (`PROBE_STRIDE` rows): the ambient cube
/// times `scale`, then the four brightest lights' directions and colours
/// (times `scale`; unused ones zero).
pub fn probe_rows(probe: &LightProbe, scale: f32) -> [[f32; 4]; PROBE_STRIDE] {
    let mut rows = [[0.0; 4]; PROBE_STRIDE];
    for (r, v) in rows.iter_mut().zip(probe.cube) {
        *r = (v * scale).extend(0.0).to_array();
    }
    let mut lights: Vec<&(Vec3, Vec3)> = probe.lights.iter().collect();
    lights.sort_by(|a, b| b.1.max_element().total_cmp(&a.1.max_element()));
    for i in 0..4 {
        let (dir, color) = lights.get(i).map_or((Vec3::ZERO, Vec3::ZERO), |l| (l.0, l.1 * scale));
        rows[6 + i] = dir.extend(0.0).to_array();
        rows[10 + i] = color.extend(0.0).to_array();
    }
    rows
}

/// Upload the probe table when it changed.
pub fn upload_probes(table: Option<ResMut<ProbeTable>>, buffers: Option<ResMut<Assets<ShaderBuffer>>>) {
    if let (Some(mut table), Some(mut buffers)) = (table, buffers) {
        table.upload(&mut buffers);
    }
}

/// Give a lit entity's table slot back (its `ProbeLit` removed or replaced).
fn free_probe_slot(mut world: DeferredWorld, ctx: HookContext) {
    let slot = world.get::<ProbeLit>(ctx.entity).map_or(0, |l| l.slot);
    if slot != 0
        && let Some(mut table) = world.get_resource_mut::<ProbeTable>()
    {
        table.free(slot);
    }
}

/// Lit from the light field at this entity's position (plus `offset`).
#[derive(Component, Debug, Default)]
#[component(on_discard = free_probe_slot)]
pub struct ProbeLit {
    pub materials: Vec<Handle<PropMaterial>>,
    /// Added to the entity's position before sampling (e.g. a body's
    /// centre above its feet).
    pub offset: Vec3,
    /// Sample at this entity instead of the lit one (a ragdoll's drawn
    /// body sits at the origin; its root physics body is where it is).
    pub follow: Option<Entity>,
    /// Added in the entity's own axes (a prop's centre, which turns with
    /// it): where a model is lit from when its origin is on the floor.
    pub centre: Vec3,
    lit_at: Option<Vec3>,
    /// Its probe table slot (0: none yet).
    slot: u32,
}

/// A copy lights itself anew, in a slot of its own.
impl Clone for ProbeLit {
    fn clone(&self) -> Self {
        Self {
            materials: self.materials.clone(),
            offset: self.offset,
            follow: self.follow,
            centre: self.centre,
            lit_at: None,
            slot: 0,
        }
    }
}

impl ProbeLit {
    pub fn new(materials: Vec<Handle<PropMaterial>>, offset: Vec3) -> Self {
        Self {
            materials,
            offset,
            follow: None,
            centre: Vec3::ZERO,
            lit_at: None,
            slot: 0,
        }
    }

    /// Its probe table slot (0: none yet).
    pub fn slot(&self) -> u32 {
        self.slot
    }
}

/// The map's light scale for probe lighting (`MapData::look.light_scale`).
#[derive(Resource, Clone, Copy, Debug)]
pub struct ProbeLightScale(pub f32);

/// Re-sample after moving this far, meters.
const RELIGHT_DISTANCE: f32 = 0.25;

/// Turning this far (radians) also counts as moved: the baked light is
/// glued to the mesh.
const RELIGHT_TURN: f32 = 0.05;

/// A map's physics prop still wearing its baked light: where it was first
/// seen, its shared materials that take light, and its centre.
#[derive(Component, Clone, Debug)]
pub struct PropLightHome {
    pub lit: Vec<Handle<PropMaterial>>,
    pub centre: Vec3,
    home: Option<(Vec3, Quat)>,
}

impl PropLightHome {
    pub fn new(lit: Vec<Handle<PropMaterial>>, centre: Vec3) -> Self {
        Self { lit, centre, home: None }
    }
}

/// A physics prop that has moved or turned is lit where it is, as any
/// moving model: its meshes take own materials with the light field's probe
/// (shaders.md 4; `relight` keeps it up to date). Without a light field it
/// keeps what it has.
pub fn light_moved_props(
    field: Option<Res<LightField>>,
    scale: Option<Res<ProbeLightScale>>,
    mut props: Query<(Entity, &mut PropLightHome, &GlobalTransform, &Children)>,
    mut meshes: Query<&mut MeshMaterial3d<PropMaterial>>,
    materials: Option<ResMut<Assets<PropMaterial>>>,
    mut commands: Commands,
) {
    let (Some(field), Some(mut materials)) = (field, materials) else { return };
    let scale = scale.map_or(1.0, |s| s.0);
    for (e, mut home, at, children) in &mut props {
        let (pos, rot) = (at.translation(), at.rotation());
        let Some((was_at, was_turned)) = home.home else {
            home.home = Some((pos, rot));
            continue;
        };
        if was_at.distance(pos) < RELIGHT_DISTANCE && was_turned.angle_between(rot) < RELIGHT_TURN {
            continue;
        }
        let probe = (field.0.0)(pos + rot * home.centre);
        let mut own = Vec::new();
        for child in children {
            let Ok(mut material) = meshes.get_mut(*child) else { continue };
            if !home.lit.contains(&material.0) {
                continue;
            }
            let handle = instance(&mut materials, None, &material.0);
            if let Some(mut m) = materials.get_mut(&handle) {
                m.params.set_probe(&probe, scale);
            }
            material.0 = handle.clone();
            own.push(handle);
        }
        let mut lit = ProbeLit::new(own, Vec3::ZERO);
        lit.centre = home.centre;
        commands.entity(e).remove::<PropLightHome>().insert(lit);
    }
}

/// A copy of a shared material for one model instance (from the store, or
/// held dormant: a kit's, `prop_material::DormantProps`).
pub fn instance(
    materials: &mut Assets<PropMaterial>,
    dormant: Option<&super::prop_material::DormantProps>,
    shared: &Handle<PropMaterial>,
) -> Handle<PropMaterial> {
    match super::prop_material::prop(materials, dormant, shared).cloned() {
        Some(m) => materials.add(m),
        None => shared.clone(),
    }
}

/// Light each `ProbeLit` where it is, once it has moved `RELIGHT_DISTANCE`:
/// into its probe table slot (its materials point there once, when it gets
/// the slot), or without the table (headless apps that add none) into its
/// materials as before.
pub fn relight(
    field: Option<Res<LightField>>,
    scale: Option<Res<ProbeLightScale>>,
    mut lit: Query<(&mut ProbeLit, &GlobalTransform)>,
    places: Query<&GlobalTransform>,
    materials: Option<ResMut<Assets<PropMaterial>>>,
    mut table: Option<ResMut<ProbeTable>>,
) {
    let (Some(field), Some(mut materials)) = (field, materials) else { return };
    let scale = scale.map_or(1.0, |s| s.0);
    for (mut l, at) in &mut lit {
        let at = l.follow.and_then(|e| places.get(e).ok()).unwrap_or(at);
        let p = at.translation() + at.rotation() * l.centre + l.offset;
        if l.lit_at.is_some_and(|q| q.distance(p) < RELIGHT_DISTANCE) {
            continue;
        }
        let probe = (field.0.0)(p);
        match table.as_mut() {
            Some(table) => {
                if l.slot == 0 {
                    l.slot = table.alloc();
                }
                let slot = l.slot;
                table.set_probe(slot, &probe, scale);
                for h in &l.materials {
                    // Read first: `get_mut` marks the asset modified, which
                    // makes the render world prepare it again.
                    if materials.get(h).is_some_and(|m| m.params.probe > 0.5 && m.params.probe_slot != slot)
                        && let Some(mut m) = materials.get_mut(h)
                    {
                        m.params.probe_slot = slot;
                    }
                }
            }
            None => {
                for h in &l.materials {
                    if materials.get(h).is_some_and(|m| m.params.probe > 0.5)
                        && let Some(mut m) = materials.get_mut(h)
                    {
                        m.params.set_probe(&probe, scale);
                    }
                }
            }
        }
        l.lit_at = Some(p);
    }
}
