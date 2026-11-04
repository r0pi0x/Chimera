//! HL2:DM's muzzle flashes (specs/hl2dm/effects_sounds.md 4, Constants):
//! first-person bursts of five additive sprites strung along the muzzle,
//! per kind (`kinds`: the kit's `muzzle_flash_kinds`), named by the
//! option of a view-model fire sequence's muzzle event (`event_effect`:
//! `PISTOL MUZZLE`, `357 muzzle`, `SMG1 MUZZLE`, `COMBINE MUZZLE`,
//! `SHOTGUN MUZZLE`), with the model-only light of the spec (colour (255,
//! 192, 64) exponent 5, radius 32 to 64 units, 0.05 s). Other players' guns
//! show the light only (`no_world_sprites`).
//!
//! **The pistol's and the .357's smoke** (`muzzle_smoke`, the table's
//! `pistol_smoke` and `magnum_smoke`): with a first-person `PISTOL` flash
//! (3 times in 4) or a `357` one (always), one `particle/particle_smokegrenade`
//! puff 8 units ahead of the muzzle as the flash finds it (the view
//! model's attachment re-projected into the world view, along the
//! model's forward), drifting forward (48 to 64 units/s, the .357's 8 to
//! 64) and up (4 to 16), grey 200 to 255, alpha 64 to 128 fading out,
//! size 2 to 4 units growing 4 times (the .357's 8) over its life (0.25 to
//! 0.5 s, the .357's 0.5 to 1), slowly rolling.
//!
//! Known shortcuts: sprite i sits at the table's spacing times one random
//! factor k and has the size of its draw's mean (7), `(8 - i) / 6 * k` (the
//! AR2's `(12 - i) / 12`, its core one more sprite of 13); the table's
//! per-sprite draws are not made; the light's colour is a sixteenth of the spec's, as CS:S's
//! (it burnt walls out). Why it's OK for now: the shape and timing are the
//! spec's; the look needs the real game's captures.

use bevy::prelude::*;
use source_engine::content::material::MaterialLoader;

use crate::map::{
    MapFlashLight, MapMuzzleFlash, ParticleRng, ViewModelEvent, ViewModelEventKind,
    anim::AnimEvent,
    particles::{Fade, Motion, Particle, ParticleGroup, ParticleMaterials, ParticleSet, Particles, Ramp},
    view_model::{attachment_transform, reproject},
};

const UNIT: f32 = 0.0254;

/// The muzzle-flash event numbers of a view model's fire sequence, and
/// the named event HL2:DM's view models carry (`AE_MUZZLEFLASH`, number
/// 0; read from the install's models, heavy `view_models.rs`).
const EVENTS: &[i32] = &[5001, 5011, 5021, 5031];
const EVENT_NAME: &str = "AE_MUZZLEFLASH";

/// A flash kind's table row: its spacing step and range of k, the size
/// formula's numerator, the life, the sheets.
struct Row {
    name: &'static str,
    step: f32,
    k: (f32, f32),
    base: f32,
    life: f32,
    core: bool,
    sheets: &'static [&'static str],
}

const MUZZLE: &[&str] = &[
    "effects/muzzleflash1_noz",
    "effects/muzzleflash2_noz",
    "effects/muzzleflash3_noz",
    "effects/muzzleflash4_noz",
];
const COMBINE: &[&str] = &["effects/combinemuzzle1_noz", "effects/combinemuzzle2_noz"];

const ROWS: &[Row] = &[
    Row { name: "PISTOL", step: 4.0, k: (1.0, 1.25), base: 8.0, life: 0.01, core: false, sheets: MUZZLE },
    Row { name: "357", step: 8.0, k: (1.25, 1.5), base: 8.0, life: 0.01, core: false, sheets: MUZZLE },
    Row { name: "SMG1", step: 8.0, k: (1.25, 1.5), base: 8.0, life: 0.025, core: false, sheets: MUZZLE },
    Row { name: "SHOTGUN", step: 8.0, k: (1.25, 1.5), base: 8.0, life: 0.0001, core: false, sheets: MUZZLE },
    Row { name: "COMBINE", step: 8.0, k: (2.0, 2.25), base: 12.0, life: 0.025, core: true, sheets: COMBINE },
];

/// The sprites of a row at k = 1 (meters ahead, full size): the spec's
/// spacing `step * i` and half-size `7 * (base - i) / (base - 2)` (its
/// pistol: `(8 - i) / 6`; the AR2's `(12 - i) / 12`).
pub fn sprites(step: f32, base: f32, core: bool) -> Vec<(f32, f32)> {
    let denom = if base > 8.0 { 12.0 } else { 6.0 };
    let mut out: Vec<(f32, f32)> =
        (1..=5).map(|i| (step * i as f32 * UNIT, 2.0 * (7.0 * (base - i as f32) / denom).floor() * UNIT)).collect();
    if core {
        out.push((0.0, 2.0 * 13.0 * UNIT));
    }
    out
}

/// The kit's flashes, by kind name.
pub fn kinds(materials: &mut MaterialLoader) -> Vec<(String, MapMuzzleFlash)> {
    let exp = 2f32.powi(5) / 255.0 / 16.0;
    ROWS.iter()
        .map(|r| {
            let textures: Vec<usize> = r.sheets.iter().filter_map(|s| materials.resolve(s).texture).collect();
            let flash = MapMuzzleFlash {
                texture: textures.first().copied(),
                alt_textures: textures.iter().skip(1).copied().collect(),
                sprites: sprites(r.step, r.base, r.core),
                scale: r.k,
                color: [1.0, 1.0, 0.9],
                life: r.life,
                light: Some(MapFlashLight {
                    color: Vec3::new(255.0, 192.0, 64.0) * exp,
                    radius: (32.0 * UNIT, 64.0 * UNIT),
                    life: 0.05,
                }),
                no_world_sprites: true,
            };
            (r.name.to_string(), flash)
        })
        .collect()
}

/// What a view-model event asks for: a muzzle flash of the kind its option
/// names (`SMG1 MUZZLE`), else HL2's numbered brass event
/// (`shells::event_effect`), else Source's shared reading.
pub fn event_effect(e: &AnimEvent) -> Option<ViewModelEventKind> {
    if EVENTS.contains(&e.event) || e.name.eq_ignore_ascii_case(EVENT_NAME) {
        let kind = e.options.split_whitespace().next()?.to_ascii_uppercase();
        return ROWS.iter().find(|r| r.name == kind).map(|r| ViewModelEventKind::MuzzleFlashOf {
            attachment: 0,
            kind: r.name.to_string(),
        });
    }
    super::shells::event_effect(e).or_else(|| source_engine::view_anim::event_effect(e))
}

/// The smoke puff's material.
pub const SMOKE: &str = "particle/particle_smokegrenade";

/// Which smoke a flash kind makes (the table's "Extra"): the pistol's 3
/// times in 4, the .357's always.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Smoke {
    Pistol,
    Magnum,
}

impl Smoke {
    pub fn of(kind: &str) -> Option<Self> {
        match kind.to_ascii_uppercase().as_str() {
            "PISTOL" => Some(Self::Pistol),
            "357" => Some(Self::Magnum),
            _ => None,
        }
    }

    /// Whether a flash makes its puff, for a draw `roll` in 0 to 1.
    pub fn makes(self, roll: f32) -> bool {
        self == Self::Magnum || roll < 0.75
    }
}

/// One smoke puff for a flash at `muzzle` (engine space) whose forward is
/// `f` (the table's `pistol_smoke`, `magnum_smoke`).
pub fn smoke_puff(rng: &mut ParticleRng, smoke: Smoke, material: usize, muzzle: Vec3, f: Vec3) -> Particle {
    let (speed, life, grow, spin) = match smoke {
        Smoke::Pistol => ((48.0, 64.0), (0.25, 0.5), 4.0, 0.1),
        Smoke::Magnum => ((8.0, 64.0), (0.5, 1.0), 8.0, 0.5),
    };
    let velocity = f * rng.float(speed.0, speed.1) * UNIT + Vec3::Y * rng.float(4.0, 16.0) * UNIT;
    let life = rng.float(life.0, life.1);
    let grey = rng.int(200, 255) as f32 / 255.0;
    let alpha = rng.int(64, 128) as f32 / 255.0;
    let size = rng.int(2, 4) as f32 * UNIT;
    Particle {
        velocity,
        color: Vec3::splat(grey),
        alpha,
        fade: Fade::Ramp(Ramp::linear(1.0, 0.0)),
        size: Ramp::linear(size, size * grow),
        roll_speed: rng.float(-spin, spin),
        ..Particle::new(muzzle + f * 8.0 * UNIT, life, material, size)
    }
}

#[derive(Resource)]
struct SmokeRng(ParticleRng);

impl Default for SmokeRng {
    fn default() -> Self {
        Self(ParticleRng::new(0x5_340C))
    }
}

/// The puffs of this frame's first-person pistol and .357 flashes (see
/// the module docs): at the flash's point, as `render::view_model` finds
/// it (the attachment re-projected, the model's forward).
fn muzzle_smoke(
    mut events: MessageReader<ViewModelEvent>,
    view: super::physcannon_fx::FirstPersonView,
    materials: Option<Res<ParticleMaterials>>,
    time: Res<Time>,
    mut particles: ResMut<Particles>,
    mut rng: ResMut<SmokeRng>,
) {
    let Some(mat) = materials.as_ref().and_then(|m| m.0.find(SMOKE)) else {
        events.clear();
        return;
    };
    let now = time.elapsed_secs_f64();
    let mut g = ParticleGroup::new(Motion::default());
    for ev in events.read() {
        let ViewModelEventKind::MuzzleFlashOf { attachment, kind } = &ev.kind else { continue };
        let Some(smoke) = Smoke::of(kind) else { continue };
        let Some(fp) = view.first(ev.owner, now) else { continue };
        let Some(m) = attachment_transform(fp.model, &fp.pose, *attachment, &fp.placed) else { continue };
        let muzzle = m.transform_point3(Vec3::ZERO);
        let forward = (fp.placed.to_matrix() * fp.model.root.to_matrix()).transform_vector3(Vec3::X);
        let project = |p: Vec3| fp.eye.transform_point(fp.fovs.map_or(p, |(w, v)| reproject(p, w, v)));
        let (at, ahead) = (project(muzzle), project(muzzle + forward));
        let f = (ahead - at).normalize_or_zero();
        if smoke.makes(rng.0.float(0.0, 1.0)) {
            g.particles.push(smoke_puff(&mut rng.0, smoke, mat, at, f));
        }
    }
    if !g.particles.is_empty() {
        particles.add(g);
    }
}

/// The pistol's and .357's smoke (`Hl2EffectsPlugin`).
pub fn plugin(app: &mut App) {
    app.init_resource::<SmokeRng>()
        .add_message::<ViewModelEvent>()
        .add_systems(Update, muzzle_smoke.after(ParticleSet::Step).before(ParticleSet::Draw));
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Spec test 28 at k = 1 and mean draws: the SMG1's sprites sit every 8.
    #[test]
    fn spacing_and_sizes() {
        let s = sprites(4.0, 8.0, false);
        assert_eq!(s.len(), 5);
        assert!((s[0].0 - 4.0 * UNIT).abs() < 1e-6 && (s[4].0 - 20.0 * UNIT).abs() < 1e-6);
        // Half-sizes 8, 7, 5, 4, 2 from the mean draw 7 (the spec's table
        // with every draw 6 gives 7, 6, 5, 4, 3).
        assert!(s.windows(2).all(|w| w[0].1 >= w[1].1));
        let ar2 = sprites(8.0, 12.0, true);
        assert_eq!(ar2.len(), 6);
        assert!((ar2[5].1 - 26.0 * UNIT).abs() < 1e-6);
    }

    /// The table's smoke: the pistol's 3 times in 4, the .357's always,
    /// 8 units ahead, drifting forward and up, growing 4 or 8 times.
    #[test]
    fn the_pistol_and_magnum_smoke() {
        assert_eq!(Smoke::of("PISTOL"), Some(Smoke::Pistol));
        assert_eq!(Smoke::of("357"), Some(Smoke::Magnum));
        assert_eq!(Smoke::of("SMG1"), None);
        assert!(Smoke::Pistol.makes(0.7) && !Smoke::Pistol.makes(0.8) && Smoke::Magnum.makes(0.99));
        let mut rng = ParticleRng::new(3);
        for _ in 0..50 {
            let p = smoke_puff(&mut rng, Smoke::Pistol, 0, Vec3::ZERO, Vec3::X);
            assert!((p.position.x / UNIT - 8.0).abs() < 1e-3);
            let (vx, vy) = (p.velocity.x / UNIT, p.velocity.y / UNIT);
            assert!((48.0..=64.0).contains(&vx) && (4.0..=16.0).contains(&vy), "{vx} {vy}");
            assert!((0.25..=0.5).contains(&p.life));
            let s = p.size.start / UNIT;
            assert!((2.0..=4.0).contains(&s) && (p.size.end / p.size.start - 4.0).abs() < 1e-4);
            assert!((64.0..=128.0).contains(&(p.alpha * 255.0).round()));
            assert!(p.roll_speed.abs() <= 0.1);
            let m = smoke_puff(&mut rng, Smoke::Magnum, 0, Vec3::ZERO, Vec3::X);
            assert!((0.5..=1.0).contains(&m.life) && (m.size.end / m.size.start - 8.0).abs() < 1e-4);
            assert!((8.0..=64.0).contains(&(m.velocity.x / UNIT)));
        }
    }

    #[test]
    fn the_option_names_the_kind() {
        let ev = |opt: &str| AnimEvent { event: 5001, options: opt.into(), ..Default::default() };
        assert!(matches!(event_effect(&ev("PISTOL MUZZLE")), Some(ViewModelEventKind::MuzzleFlashOf { kind, .. }) if kind == "PISTOL"));
        assert!(matches!(event_effect(&ev("357 muzzle")), Some(ViewModelEventKind::MuzzleFlashOf { kind, .. }) if kind == "357"));
        assert!(event_effect(&ev("")).is_none());
        // As the install's models name it.
        let named = AnimEvent { name: "AE_MUZZLEFLASH".into(), options: "SMG1 MUZZLE".into(), ..Default::default() };
        assert!(matches!(event_effect(&named), Some(ViewModelEventKind::MuzzleFlashOf { kind, .. }) if kind == "SMG1"));
    }
}
