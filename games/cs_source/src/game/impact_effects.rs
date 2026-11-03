//! CS:S's impact effects: Source's (`source_engine::effects::impact_effects`,
//! lift S10 of docs/plans/active/hl2dm.md, re-exported here at its old
//! path; the spec's numbers, section by section, are documented there) with
//! CS:S's own style (`STYLE`: the 30 % ricochet, splashes of scale 8 to 12,
//! pellets' sounds grouped), its effect materials (grenades', fire's) and
//! its tracers (one every fourth bullet, the spark streak of spec
//! tracers.md).

pub use source_engine::effects::impact_effects::*;

use bevy::prelude::*;
use source_engine::effects::{ImpactStyle, impact_effects as engine};

use crate::map::{
    particles::MapParticles,
    tracer::Tracer,
};
use crate::weapon::{WeaponEvent, WeaponEventKind, impact::ImpactRoute};

/// CS:S's impacts (specs/cs_source/impact_effects.md): `Bounce.Shrapnel`
/// on 3 in 10 bullet impacts, splashes of scale 8 to 12, one impact sound
/// per 300 units for a shot's pellets; its weapon data counts damage in
/// hundredths.
pub const STYLE: ImpactStyle = ImpactStyle {
    game: super::GAME,
    ricochet: Some((RICOCHET, RICOCHET_CHANCE)),
    splash: (8.0, 12.0),
    group_radius: Some(300.0),
    damage_unit: super::weapons::HP,
    flesh: "flesh",
};

/// Load the effect materials (runtime, from the install): Source's with
/// CS:S's grenades' and fire's.
pub fn load_materials(loader: &mut super::material::MaterialLoader) -> MapParticles {
    engine::load_materials(loader, &[super::grenades::MATERIALS, super::fire::MATERIALS])
}

/// Bullets per tracer: one global count over every bullet (spec
/// tracers.md 1; CS:S's N is open question 1, 4 is the shared default).
pub const TRACER_EVERY: u64 = 4;

/// The tracer's look (spec tracers.md constants), in meters.
pub const TRACER_LOOK: crate::map::tracer::TracerLook = crate::map::tracer::TracerLook {
    material: "effects/spark",
    speed: 5000.0 * super::bsp::METERS_PER_UNIT,
    length: (64.0 * super::bsp::METERS_PER_UNIT, 128.0 * super::bsp::METERS_PER_UNIT),
    half_width: (0.75 * super::bsp::METERS_PER_UNIT, 0.9 * super::bsp::METERS_PER_UNIT),
    min_distance: 256.0 * super::bsp::METERS_PER_UNIT,
};

/// Whether the bullet numbered `count` (from 0) draws a tracer.
pub fn draws_tracer(count: u64) -> bool {
    count.is_multiple_of(TRACER_EVERY)
}

/// One tracer every `TRACER_EVERY` bullets, from the gun to where the
/// bullet stopped.
fn tracers(
    mut events: MessageReader<WeaponEvent>,
    mut out: MessageWriter<crate::map::tracer::Tracer>,
    mut count: Local<u64>,
    route: ImpactRoute,
) {
    // CS:S's guns' tracers, and other games' guns' where CS:S draws their
    // impacts.
    let ours = |e: &WeaponEvent| route.gun_game(e.weapon) == Some(super::GAME) || route.draws(super::GAME, e);
    for e in events.read().filter(|e| e.shown() && ours(e)) {
        let WeaponEventKind::Shot { from, to, .. } = e.kind else { continue };
        if draws_tracer(*count) {
            out.write(crate::map::tracer::Tracer {
                owner: e.owner,
                from,
                to,
                look: TRACER_LOOK,
            });
        }
        *count += 1;
    }
}

/// CS:S's impact effects: Source's with `STYLE`, and its tracers.
pub struct ImpactEffectsPlugin;

impl Plugin for ImpactEffectsPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(engine::ImpactEffectsPlugin(STYLE))
            .add_message::<Tracer>()
            .add_systems(FixedUpdate, tracers.after(crate::core::SimSet::Weapons));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_fourth_bullet_draws_a_tracer() {
        let drawn: Vec<u64> = (0..10).filter(|n| draws_tracer(*n)).collect();
        assert_eq!(drawn, [0, 4, 8]);
        // 256 in minimum, 5000 in/s.
        assert!((TRACER_LOOK.min_distance - 6.5024).abs() < 1e-4);
        assert!((TRACER_LOOK.speed - 127.0).abs() < 1e-3);
    }
}
