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
//! Known shortcuts (docs/tech-debt.md): a moved prop's skin change goes
//! back to the baked light (only `PropVariants`' shared materials know the
//! skins; no stock prop_physics changes skin).

use bevy::prelude::*;

use super::prop_material::PropMaterial;
use crate::map::LightField;

/// Lit from the light field at this entity's position (plus `offset`).
#[derive(Component, Clone, Debug, Default)]
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
}

impl ProbeLit {
    pub fn new(materials: Vec<Handle<PropMaterial>>, offset: Vec3) -> Self {
        Self {
            materials,
            offset,
            follow: None,
            centre: Vec3::ZERO,
            lit_at: None,
        }
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
            let handle = instance(&mut materials, &material.0);
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

/// A copy of a shared material for one model instance.
pub fn instance(materials: &mut Assets<PropMaterial>, shared: &Handle<PropMaterial>) -> Handle<PropMaterial> {
    match materials.get(shared).cloned() {
        Some(m) => materials.add(m),
        None => shared.clone(),
    }
}

pub fn relight(
    field: Option<Res<LightField>>,
    scale: Option<Res<ProbeLightScale>>,
    mut lit: Query<(&mut ProbeLit, &GlobalTransform)>,
    places: Query<&GlobalTransform>,
    materials: Option<ResMut<Assets<PropMaterial>>>,
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
        for h in &l.materials {
            // Read first: `get_mut` marks the asset modified, which makes
            // the render world prepare it again (docs/performance.md,
            // "Lucker Party": Combat Arms' bodies aren't probe lit).
            if materials.get(h).is_some_and(|m| m.params.probe > 0.5)
                && let Some(mut m) = materials.get_mut(h)
            {
                m.params.set_probe(&probe, scale);
            }
        }
        l.lit_at = Some(p);
    }
}
