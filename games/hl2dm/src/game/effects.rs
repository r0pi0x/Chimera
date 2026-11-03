//! HL2:DM's effects (specs/hl2dm/effects_sounds.md, H9; lift S10 of
//! docs/plans/active/hl2dm.md): Source's impact effects, decals and
//! impact sounds (`source_engine::effects`) with HL2:DM's style (`STYLE`:
//! no ricochet, splashes of scale 4 to 8, each pellet's impact sound
//! played), its own tracers (`tracers`: one counter for every bullet of
//! the process, every 2nd (pistol, SMG1, AR2) or 4th (.357, shotgun)
//! bullet draws a streak, the AR2's its own: `AR2_TRACER`), the near-miss
//! sound of one's own tracers (`Bullets.DefaultNearmiss`,
//! `Bullets.GunshipNearmiss`: 24 and 72 units, at most every 0.1 s), the
//! AR2's flat flash on what its bullets hit and the stunstick's glow and
//! sparks (`gun_impacts`), and the kit's muzzle flashes (`muzzle_flashes`:
//! the five sprite strings of the spec's table, per view-model event
//! option) with their light.
//!
//! The projectiles' looks are `projectile_fx` (explosions, the rocket,
//! the ball, the bolt, the grenades' trails, the laser dot) and the ball's
//! dissolve is `dissolve`; the gravity gun's sprites and beams are left
//! (M6).
//!
//! Known shortcuts (docs/tech-debt.md): other players' shots are drawn as
//! the shooter's own are (their server-flagged tracer on every bullet of a
//! flagged shot, and the re-trace through all players, aren't: spec 1);
//! the near-miss test's listener is the shot's start, the eye, not the
//! camera; a flash sprite's size draw is one per flash, not per sprite,
//! and the AR2's core sprite is one more of the same string; the
//! third-person flash is the light alone, which needs the world weapon's
//! muzzle (`view_anim`'s events); shells (`shell.mdl` from the SMG1,
//! `shotgun_shell.mdl` from the pump) are not drawn. Why it's OK for now:
//! the spec's numbers on the shooter's own screen are all there; the rest
//! needs the real game's captures to check.

use bevy::prelude::*;
use crate::map::impact::SurfaceClassesAppExt;
use source_engine::effects::{
    ImpactStyle,
    impact_effects::{self, EffectMaterials, WaterShot},
};

use super::GAME;
use crate::{
    core::{LocalPlayer, MapWater},
    map::{
        ParticleRng, PlaySound,
        particles::{Fade, MapParticles, Motion, Particle, ParticleGroup, ParticleMaterials, Particles, Ramp, Shape, WorldTracer},
        tracer::{MuzzleCache, Tracer, TracerLook},
    },
    weapon::{Weapon, WeaponEvent, WeaponEventKind, impact::ImpactRoute},
};

const UNIT: f32 = 0.0254;

/// HL2:DM's impacts (specs/hl2dm/effects_sounds.md 6): no ricochet, a
/// bullet's splash of scale 4 to 8, every pellet's impact sound; the
/// weapon data counts damage in hundredths (health 1.0 is 100).
pub const STYLE: ImpactStyle = ImpactStyle {
    game: GAME,
    ricochet: None,
    splash: (4.0, 8.0),
    group_radius: None,
    damage_unit: 0.01,
    flesh: "flesh",
};

/// The effect materials HL2:DM's effects add to Source's (the AR2's
/// streak and flash, the stunstick's glow).
pub const MATERIALS: &[&str] = &[
    "effects/gunshiptracer",
    "effects/combinemuzzle2_nocull",
    "sprites/light_glow02_add",
];

const AR2_FLASH: &str = "effects/combinemuzzle2_nocull";
const STUNSTICK_GLOW: &str = "sprites/light_glow02_add";

/// Load the effect materials (runtime, from the install).
pub fn load_materials(loader: &mut source_engine::content::material::MaterialLoader) -> MapParticles {
    impact_effects::load_materials(
        loader,
        &[MATERIALS, super::projectile_fx::MATERIALS, source_engine::effects::explosions::MATERIALS],
    )
}

/// The default tracer's streak (specs/cs_source/tracers.md 3.1): 5000
/// in/s, 64 to 128 long, 0.75 to 0.9 half-width, nothing under 256.
pub const TRACER: TracerLook = TracerLook {
    material: "effects/spark",
    speed: 5000.0 * UNIT,
    length: (64.0 * UNIT, 128.0 * UNIT),
    half_width: (0.75 * UNIT, 0.9 * UNIT),
    min_distance: 256.0 * UNIT,
};

/// The AR2's streak (spec 2): 8000 in/s, 128 to 256 long, 0.5 to 1.5
/// half-width, nothing under 128.
pub const AR2_TRACER: TracerLook = TracerLook {
    material: "effects/gunshiptracer",
    speed: 8000.0 * UNIT,
    length: (128.0 * UNIT, 256.0 * UNIT),
    half_width: (0.5 * UNIT, 1.5 * UNIT),
    min_distance: 128.0 * UNIT,
};

/// Bullets per tracer by weapon (weapons.md 6): every 4th for the .357 and
/// the shotgun's pellets, every 2nd for the rest.
pub fn tracer_every(weapon: &str) -> u64 {
    if weapon == super::weapons::MAGNUM || weapon == super::weapons::SHOTGUN {
        4
    } else {
        2
    }
}

/// Whether bullet number `count` (the process's, counting from 1 as the
/// counter advances before it is tested: spec test 18) of `weapon` draws a
/// tracer.
pub fn draws_tracer(count: u64, weapon: &str) -> bool {
    count.is_multiple_of(tracer_every(weapon))
}

/// A near-miss sound (spec 3): its entry, how close the shot must pass
/// (units) and how far the listener may slide down to meet it (units).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Whizz {
    pub entry: &'static str,
    pub range: f32,
    pub drop: f32,
}

/// The default tracer's near-miss: within 24 units, the listener slid
/// down by up to 24.
pub const WHIZZ: Whizz = Whizz { entry: "Bullets.DefaultNearmiss", range: 24.0, drop: 24.0 };
/// The AR2's: within 72, no slide, the deep variant.
pub const AR2_WHIZZ: Whizz = Whizz { entry: "Bullets.GunshipNearmiss", range: 72.0, drop: 0.0 };
/// Seconds between near-miss sounds.
pub const WHIZZ_INTERVAL: f64 = 0.1;

/// The distance from `listener`, slid straight down by at most `drop` (all
/// in meters, engine space, y up), to the segment `a` to `b`: the nearest
/// it comes (spec 3).
pub fn whizz_distance(listener: Vec3, a: Vec3, b: Vec3, drop: f32) -> f32 {
    let at = |t: f32| {
        let p = listener - Vec3::Y * t;
        let ab = b - a;
        let len2 = ab.length_squared();
        let u = if len2 > 0.0 { ((p - a).dot(ab) / len2).clamp(0.0, 1.0) } else { 0.0 };
        p.distance(a + ab * u)
    };
    // Convex in the slide: ternary search.
    let (mut lo, mut hi) = (0.0f32, drop.max(0.0));
    for _ in 0..40 {
        let (m1, m2) = (lo + (hi - lo) / 3.0, hi - (hi - lo) / 3.0);
        if at(m1) <= at(m2) {
            hi = m2;
        } else {
            lo = m1;
        }
    }
    at(lo).min(at(0.0))
}

/// The bullets' tracers and the shooter's own near-miss sounds.
#[allow(clippy::too_many_arguments)]
fn tracers(
    mut events: MessageReader<WeaponEvent>,
    weapons: Query<&Weapon>,
    local: Query<(), With<LocalPlayer>>,
    route: ImpactRoute,
    muzzles: Option<Res<MuzzleCache>>,
    time: Res<Time>,
    mut out: MessageWriter<Tracer>,
    mut play: MessageWriter<PlaySound>,
    mut count: Local<u64>,
    mut next_whizz: Local<f64>,
) {
    let now = time.elapsed_secs_f64();
    for e in events.read().filter(|e| e.shown()) {
        // HL2:DM's guns' tracers, and other games' guns' where HL2:DM
        // draws their impacts.
        if route.gun_game(e.weapon) != Some(GAME) && !route.draws(GAME, e) {
            continue;
        }
        let WeaponEventKind::Shot { from, to, .. } = e.kind else { continue };
        let id = weapons.get(e.weapon).map_or("", |w| w.id);
        let ar2 = id == super::weapons::AR2;
        *count += 1;
        if draws_tracer(*count, id) {
            out.write(Tracer {
                owner: e.owner,
                from,
                to,
                look: if ar2 { AR2_TRACER } else { TRACER },
            });
            // Only the shooter's own tracers whizz, whether or not a
            // streak is drawn for a short shot.
            if local.contains(e.owner) && now >= *next_whizz {
                let whizz = if ar2 { AR2_WHIZZ } else { WHIZZ };
                let start = muzzles
                    .as_ref()
                    .and_then(|m| m.0.get(&e.owner))
                    .filter(|(_, at)| now - at < 0.25)
                    .map_or(from, |(p, _)| *p);
                if whizz_distance(from, start, to, whizz.drop * UNIT) < whizz.range * UNIT {
                    *next_whizz = now + WHIZZ_INTERVAL;
                    play.write(PlaySound::at(whizz.entry, start));
                }
            }
        }
    }
}

#[derive(Resource)]
struct EffectRng(ParticleRng);

impl Default for EffectRng {
    fn default() -> Self {
        Self(ParticleRng::new(0x2_7F4D))
    }
}

/// A flat quad facing `normal` at `at`: side `from` to `to` meters over
/// `life` seconds (the size's bias `size_bias`), alpha 1 to 0 (bias
/// `alpha_bias`), turned `yaw` degrees.
#[allow(clippy::too_many_arguments)]
pub fn flat_quad(
    material: usize,
    at: Vec3,
    normal: Vec3,
    yaw: f32,
    size: (f32, f32),
    size_bias: f32,
    alpha_bias: f32,
    life: f32,
) -> ParticleGroup {
    let mut g = ParticleGroup::new(Motion::default());
    g.skip_first = false;
    g.particles.push(Particle {
        fade: Fade::Ramp(Ramp::linear(1.0, 0.0).with_bias(alpha_bias)),
        size: Ramp::linear(size.0, size.1).with_bias(size_bias),
        shape: Shape::Flat { normal, yaw, yaw_speed: 0.0 },
        ..Particle::new(at, life, material, size.0)
    });
    g
}

/// The AR2's flash on what its bullets hit, and the stunstick's glow and
/// sparks where it hits (spec 6): before the surface's own impact
/// (`source_engine::effects::impact_effects`).
#[allow(clippy::too_many_arguments)]
fn gun_impacts(
    mut events: MessageReader<WeaponEvent>,
    weapons: Query<&Weapon>,
    route: ImpactRoute,
    water: Option<Res<MapWater>>,
    materials: Option<Res<ParticleMaterials>>,
    mut particles: ResMut<Particles>,
    mut rng: ResMut<EffectRng>,
    world: WorldTracer,
) {
    let Some(materials) = materials else {
        events.clear();
        return;
    };
    let rng = &mut rng.0;
    for e in events.read().filter(|e| e.shown() && route.draws(GAME, e)) {
        let id = weapons.get(e.weapon).map_or("", |w| w.id);
        match &e.kind {
            WeaponEventKind::Shot { from, to, hit: Some(_), normal: Some(n) }
            | WeaponEventKind::ShotContinued { from, to, hit: Some(_), normal: Some(n) }
                if id == super::weapons::AR2 =>
            {
                // A bullet that ends in water makes a splash instead.
                if water.as_ref().is_some_and(|w| impact_effects::water_shot(*from, *to, &w.0) != WaterShot::Dry) {
                    continue;
                }
                let Some(quad) = materials.0.find(AR2_FLASH) else { continue };
                let side = rng.float(24.0, 32.0) * UNIT;
                let yaw = rng.int(0, 360) as f32;
                particles.add(flat_quad(quad, *to + *n * UNIT, *n, yaw, (side, 0.0), 0.75, 0.4, 0.25));
            }
            WeaponEventKind::Swing { at: Some((point, n, _)), .. } if id == super::weapons::STUNSTICK => {
                let Some(glow) = materials.0.find(STUNSTICK_GLOW) else { continue };
                let side = rng.float(16.0, 32.0) * UNIT;
                let yaw = rng.int(0, 360) as f32;
                let at = *point + *n * 4.0 * UNIT;
                particles.add(flat_quad(glow, at, *n, yaw, (side, side * 2.0), 0.5, 0.5, 0.1));
                let mats = EffectMaterials::new(&materials.0);
                for g in impact_effects::electric_sparks(rng, &mats, *point, *n, &world) {
                    particles.add(g);
                }
            }
            _ => {}
        }
    }
}

pub struct Hl2EffectsPlugin;

impl Plugin for Hl2EffectsPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((
            source_engine::effects::impact_effects::ImpactEffectsPlugin(STYLE),
            source_engine::effects::impacts::ImpactSoundsPlugin(STYLE),
        ))
        .init_resource::<EffectRng>()
        .register_surface_classes(source_engine::effects::impacts::surfaces(GAME))
        .init_resource::<MuzzleCache>()
        .add_message::<Tracer>()
        .add_message::<PlaySound>()
        .add_plugins((super::projectile_fx::Hl2ProjectileFxPlugin, super::dissolve::Hl2DissolvePlugin))
        .add_systems(FixedUpdate, (tracers, gun_impacts).after(crate::core::SimSet::Weapons));
    }
}

/// HL2:DM's impacts and its tracers for an app of several games.
pub fn add_effects(app: &mut App) {
    if !app.is_plugin_added::<Hl2EffectsPlugin>() {
        app.add_plugins(Hl2EffectsPlugin);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tracer_cadence_by_weapon() {
        // The counter advances first: the 2nd bullet of the SMG1 draws,
        // the 4th of the shotgun (spec tests 18, 19).
        assert!(!draws_tracer(1, super::super::weapons::SMG1) && draws_tracer(2, super::super::weapons::SMG1));
        assert!((1..4).all(|c| !draws_tracer(c, super::super::weapons::SHOTGUN)) && draws_tracer(4, super::super::weapons::SHOTGUN));
        assert!(draws_tracer(4, super::super::weapons::MAGNUM));
    }

    #[test]
    fn the_ar2_streak() {
        // Spec test 21: 8000 in/s; D = 1000, L = 200: 0.15 s of life.
        assert!((AR2_TRACER.speed - 8000.0 * UNIT).abs() < 1e-3);
        assert!((AR2_TRACER.min_distance - 128.0 * UNIT).abs() < 1e-4);
        assert!(((1000.0 + 200.0) / 8000.0 - 0.15f32).abs() < 1e-6);
    }

    /// Spec test 23: the eye at the origin, the muzzle at (20, -6, -5),
    /// the hit at (1000, 0, 0) (Source's axes: x forward, y left, z up).
    #[test]
    fn a_nearby_muzzle_whizzes() {
        let s = |x: f32, y: f32, z: f32| Vec3::new(y, z, -x) * UNIT;
        let (eye, muzzle, hit) = (s(0.0, 0.0, 0.0), s(20.0, -6.0, -5.0), s(1000.0, 0.0, 0.0));
        let plain = whizz_distance(eye, muzzle, hit, 0.0) / UNIT;
        assert!((plain - 21.47).abs() < 0.05, "{plain}");
        let slid = whizz_distance(eye, muzzle, hit, 24.0 * UNIT) / UNIT;
        assert!((slid - 20.88).abs() < 0.05, "{slid}");
        assert!(slid < WHIZZ.range);
    }

    /// Spec tests 25 and 26.
    #[test]
    fn the_ar2_reaches_further_and_a_side_pass_is_too_far() {
        let s = |x: f32, y: f32, z: f32| Vec3::new(y, z, -x) * UNIT;
        // 60 units below the eye: no slide for the AR2, 72 reaches.
        let below = whizz_distance(s(0.0, 0.0, 0.0), s(0.0, 0.0, -60.0), s(1000.0, 0.0, -60.0), 0.0) / UNIT;
        assert!(below < AR2_WHIZZ.range && below > WHIZZ.range, "{below}");
        // 30 beside it, level: the slide doesn't help sideways.
        let beside = whizz_distance(s(0.0, 0.0, 0.0), s(0.0, 30.0, 0.0), s(1000.0, 30.0, 0.0), WHIZZ.drop * UNIT) / UNIT;
        assert!(beside >= WHIZZ.range, "{beside}");
    }
}
