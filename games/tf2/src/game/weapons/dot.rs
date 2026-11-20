//! The Sniper Rifle's laser dot (specs/tf2/weapons_more.md 3.1, "The
//! dot"), from the gun's data (`[hooks."tf2:looks".dot]`, `DotLook`):
//! while a character's held gun with a dot is scoped (`AltModes` mode 1
//! and up: the owner's own, a sandbox target's, or another player's as
//! the server sends it, `NetHeld`), a dot is drawn where its aim from the
//! eye ends, stopped by the world and by characters other than the
//! holder, pulled `back` units toward it. Others see it at `size` at any
//! distance; the holder's own first-person view at `own_size` with the
//! trace cut at `own_reach` (it floats there when nothing is nearer); not
//! in the holder's own third-person camera. Two sprites each frame (a
//! one-frame particle group, `ParticleSet`): the team's `<sprite>_red` /
//! `_blue` at full size and `alpha`, and `core` (white) scaled by the
//! charge others see (`HeldGauge`, at least `core_min`). The materials
//! load with TF2's looks (`dot_materials`, `maps::add_impact_looks`).
//!
//! Known shortcuts: a character's box (its collider) stops the trace, not
//! its hitboxes; nothing else lets the trace through (the spec's friendly
//! combat items: none exist in our TF2); the sprite's size is read as its
//! full width (weapons_more.md Open questions: measured against TF2's in
//! `dot.tf2`).

use avian3d::prelude::*;
use bevy::prelude::*;
use serde::Deserialize;

use chimera_core::core::{Dead, LocalPlayer, MovementState, SOLID_LAYERS, Team};
use chimera_sim::weapon::{AltModes, HeldGauge, Inventory, Weapon};

use super::{damage::U, looks::GunLooks};
use crate::map::{
    ShowLocalBody,
    particles::{Motion, Particle, ParticleGroup, ParticleMaterials, Particles},
};

/// A gun's laser dot (sizes and distances in units).
#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct DotLook {
    /// The team's sprite before `_red` / `_blue`.
    pub sprite: String,
    /// The white core's sprite.
    pub core: String,
    /// Full width as others see it, and in the holder's own view.
    pub size: f32,
    pub own_size: f32,
    /// The own view's trace length.
    pub own_reach: f32,
    /// Pulled back toward the holder from where the trace ends.
    pub back: f32,
    /// The team sprite's alpha.
    pub alpha: f32,
    /// The core's smallest scale (an empty charge).
    pub core_min: f32,
}

impl DotLook {
    /// The team sprite's name (`blu`: BLU's).
    pub fn sprite_for(&self, blu: bool) -> String {
        format!("{}_{}", self.sprite, if blu { "blue" } else { "red" })
    }

    /// Every material it draws with.
    pub fn materials(&self) -> [String; 3] {
        [self.sprite_for(false), self.sprite_for(true), self.core.clone()]
    }

    /// The core's scale at charge fraction `gauge`.
    pub fn core_scale(&self, gauge: f32) -> f32 {
        gauge.clamp(0.0, 1.0).max(self.core_min)
    }

    /// Where the dot sits on a trace of `reach` units from `eye` along
    /// `dir` that stopped `hit` units out (None: nothing met), in units
    /// from the eye.
    pub fn distance(&self, hit: Option<f32>, reach: f32) -> f32 {
        (hit.unwrap_or(reach).min(reach) - self.back).max(0.0)
    }
}

/// The dots' materials, every gun's (loaded with TF2's looks).
pub fn dot_materials() -> Vec<String> {
    let looks = GunLooks::default();
    let mut out: Vec<String> = looks.dots().flat_map(|d| d.materials()).collect();
    out.dedup();
    out
}

/// A dot's trace: far enough for any map (the rifle's range).
const REACH: f32 = 56754.0;

/// The dots drawn this frame (module doc).
#[allow(clippy::type_complexity)]
pub fn laser_dots(
    looks: Res<GunLooks>,
    holders: Query<(Entity, &Transform, &Inventory, Option<&MovementState>, Option<&chimera_core::core::Intent>, Option<&Team>, Has<LocalPlayer>), Without<Dead>>,
    weapons: Query<(&Weapon, &AltModes, Option<&HeldGauge>)>,
    materials: Option<Res<ParticleMaterials>>,
    third_person: Option<Res<ShowLocalBody>>,
    spatial: SpatialQuery,
    colliders: Query<&ColliderOf>,
    mut particles: ResMut<Particles>,
) {
    let Some(materials) = materials else { return };
    for (owner, at, inv, state, intent, team, local) in &holders {
        let Some((weapon, modes, gauge)) = inv.active.and_then(|w| weapons.get(w).ok()) else { continue };
        if modes.current == 0 {
            continue;
        }
        let Some(dot) = looks.of(weapon.id).and_then(|l| l.dot.as_ref()) else { continue };
        if local && third_person.as_deref().is_some_and(|t| t.0) {
            continue;
        }
        let blu = super::super::bodies::skin_of(team.copied()) == 1;
        let (Some(sprite), Some(core)) = (materials.0.find(&dot.sprite_for(blu)), materials.0.find(&dot.core)) else { continue };
        let aim = intent.map_or(at.rotation, |i| chimera_sim::weapon::aim_of(i.yaw, i.pitch));
        let eye = at.translation + state.map_or(Vec3::ZERO, |s| s.eye_offset);
        let dir = aim * Vec3::NEG_Z;
        let Ok(dir3) = Dir3::new(dir) else { continue };
        let (reach, size) = if local { (dot.own_reach, dot.own_size) } else { (REACH, dot.size) };
        let filter = SpatialQueryFilter::default().with_mask(SOLID_LAYERS);
        let hit = spatial
            .cast_ray_predicate(eye, dir3, reach * U, true, &filter, &|e| colliders.get(e).map_or(e, |c| c.body) != owner)
            .map(|h| h.distance / U);
        let p = eye + dir * dot.distance(hit, reach) * U;
        let half = size * U / 2.0;
        let mut team_dot = Particle::new(p, 1.0, sprite, half);
        team_dot.alpha = dot.alpha;
        let core_dot = Particle::new(p, 1.0, core, half * dot.core_scale(gauge.map_or(0.0, |g| g.0)));
        let mut group = ParticleGroup::new(Motion::default());
        group.particles = vec![team_dot, core_dot];
        group.capped = false;
        group.kill_after = Some(0.0);
        particles.add(group);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rifle() -> DotLook {
        GunLooks::default().of(super::super::SNIPER_RIFLE).and_then(|l| l.dot.clone()).expect("the rifle's dot")
    }

    /// weapons_more.md 3.1: 6 for others, 2 in the own view within 384
    /// units, 4 units back from the trace's end, the team sprite at half
    /// alpha and a core scaled by c / 150, at least 0.1.
    #[test]
    fn the_rifles_dot_is_the_specs() {
        let d = rifle();
        assert_eq!((d.size, d.own_size, d.own_reach, d.back, d.alpha), (6.0, 2.0, 384.0, 4.0, 0.5));
        assert_eq!(d.sprite_for(true), "effects/sniperdot_blue");
        assert_eq!(d.core_scale(0.0), 0.1);
        assert_eq!(d.core_scale(0.5), 0.5);
        assert_eq!(d.core_scale(2.0), 1.0);
        assert_eq!(d.distance(Some(200.0), 384.0), 196.0);
        assert_eq!(d.distance(None, 384.0), 380.0, "the own view's dot floats at the trace's end");
        assert_eq!(d.distance(Some(1000.0), 384.0), 380.0);
        assert!(dot_materials().contains(&"effects/sniperdot_red".to_string()));
    }
}
