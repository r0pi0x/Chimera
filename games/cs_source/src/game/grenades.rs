//! CS:S's grenades (specs/cs_source/grenades.md): the HE grenade,
//! flashbang and smoke grenade as `weapon::grenade::Throwable` weapons with
//! the spec's numbers (their data: games/cs_source/data/weapons, the
//! `cs_source:grenade` hook: `throwable`), the HE explosion's particles (spec 5.4, built on
//! `map::particles`), the smoke cloud's 64 sprites (7.3), and the flash
//! model (6.2, a fit pending the probe's measurement), and what blasts
//! and flashes do to hearing (5.5, 6.4: the DSP presets as
//! `core::HearingEffect`s). The looks alone (`GrenadeLooksPlugin`: the
//! explosion's particles and the smoke cloud's sprites, for CS:S's grenades
//! and explosions of no game, not another game's) for apps that take
//! CS:S's guns into another game's gameplay (the party apps).
//!
//! Units: the spec's inches, converted with `UNIT`. Source's up (z) is our
//! y.

use bevy::prelude::*;

use super::weapons::HP;
use crate::{
    core::HearingEffect,
    map::{
        LightField, ParticleRng,
        particles::{
            Bounce, Fade, MapParticles, Motion, Particle, ParticleGroup, ParticleMaterials, ParticleSet,
            Particles, Ramp, Shape, WorldTrace, WorldTracer, probe_planes,
        },
    },
    weapon::{
        data::WeaponData,
        grenade::{
            Blast, BlastHearing, BlastShield, Detonated, ExplosionRule, Flash, Flight, GrenadeEffect, GrenadeKind, RagdollBlast, Shake, Smoke,
            SmokeCloud, ThrowRule, Throwable,
        },
    },
};

const UNIT: f32 = 0.0254;

pub const HEGRENADE: &str = "cs_source:weapon_hegrenade";
pub const FLASHBANG: &str = "cs_source:weapon_flashbang";
pub const SMOKEGRENADE: &str = "cs_source:weapon_smokegrenade";

/// Sound entries the grenades use.
pub const SOUNDS: &[&str] = &[
    "Default.PullPin_Grenade",
    "HEGrenade.Bounce",
    "Flashbang.Bounce",
    "SmokeGrenade.Bounce",
    "BaseGrenade.Explode",
    "BaseExplosionEffect.Sound",
    "Flashbang.Explode",
    "BaseSmokeEffect.Sound",
];

/// Particle materials the effects use (loaded with the map's).
pub use source_engine::effects::explosions::MATERIALS;

/// The view models' durations (spec weapons.md, view-model table):
/// `pullpin` 40 frames at 41 fps (0.9512 s), `throw` 24 at 30 (0.7667 s),
/// `deploy`.
pub const PIN_TIME: f32 = 39.0 / 41.0;
pub const THROW_TIME: f32 = 23.0 / 30.0;
pub const DRAW_TIME: f32 = 0.6667;
/// The pin sound: the `pullpin` event at cycle 0.6923.
pub const PIN_SOUND_AT: f32 = 0.6923 * PIN_TIME;

pub struct GrenadesPlugin;

impl Plugin for GrenadesPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<Detonated>()
            // Exploding props and env_explosions blast like the HE
            // grenade (prop_damage.md 7.5: the game's radius damage):
            // the bell with σ = R/3, `sv_legacy_grenade_damage` or not
            // (measured 2026-10-17, prop_damage.md Q3).
            .insert_resource(ExplosionRule(he_blast()))
            .init_resource::<crate::weapon::grenade::LegacyBlastDamage>();
        if !app.is_plugin_added::<GrenadeLooksPlugin>() {
            app.add_plugins(GrenadeLooksPlugin);
        }
        crate::console::resource_cvar::<crate::weapon::grenade::LegacyBlastDamage, u8>(
            app,
            "sv_legacy_grenade_damage",
            "1: grenade damage as the original Counter-Strike: Source (linear); 0: today's (a bell).",
            |l| &mut l.0,
        );
    }
}

/// What CS:S's grenades look like, alone (for another game's gameplay
/// with CS:S's guns: the party apps): the HE's explosion particles (spec
/// 5.4) and the smoke cloud's sprites (7.3), for CS:S's grenades and for
/// explosions of no game (env_explosion, exploding props), not another
/// game's (that game draws its own). `GrenadesPlugin` adds it.
pub struct GrenadeLooksPlugin;

impl Plugin for GrenadeLooksPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<Detonated>()
            .init_resource::<Particles>()
            .init_resource::<FxRng>()
            .add_systems(FixedUpdate, explosions.after(crate::core::SimSet::Weapons))
            .add_systems(Update, draw_clouds.after(ParticleSet::Step).before(ParticleSet::Draw));
    }
}

/// Whether CS:S draws a grenade or explosion of weapon `id`: its own, and
/// ones of no game (`env_explosion`); not another game's.
pub fn draws_blast_of(id: &'static str) -> bool {
    crate::weapon::impact::weapon_game(id).is_none_or(|g| g.eq_ignore_ascii_case(super::GAME))
}

#[derive(Resource)]
struct FxRng(ParticleRng);

impl Default for FxRng {
    fn default() -> Self {
        Self(ParticleRng::new(0x0E_FFEC7))
    }
}

/// The throw (spec 3).
fn throw_rule() -> ThrowRule {
    ThrowRule {
        pitch_lift: -10.0,
        // Measured (grenades.md Q5): 100/90 looking down, 80/90 up.
        pitch_scale: 100.0 / 90.0,
        pitch_scale_up: 80.0 / 90.0,
        speed_per_deg: 6.0 * UNIT,
        speed_max: 750.0 * UNIT,
        forward: 16.0 * UNIT,
    }
}

/// The projectile (spec 4, "all CS grenades"): 0.4 x sv_gravity 800.
pub fn flight() -> Flight {
    Flight {
        half: 2.0 * UNIT,
        gravity: 0.4 * 800.0 * UNIT,
        elasticity: 0.45,
        player_elasticity: 0.3,
        elasticity_cap: 0.9,
        stop_speed: 30.0 * UNIT,
        floor_normal: 0.7,
        breakable_damage: 10.0 * HP,
        breakable_slowdown: 0.4,
        water_slowdown: 0.5,
        water_entry: 0.5,
        max_velocity: 3500.0 * UNIT,
        check_interval: 0.2,
        spin_roll: 600f32.to_radians(),
        spin_pitch: 1200f32.to_radians(),
    }
}

/// The HE blast (spec 5; measured, grenades.md "CS:S values"): 100 ×
/// exp(−d² / 2σ²), σ = 350/3 units, none past 350 (CS:S today,
/// `sv_legacy_grenade_damage 0`); with `sv_legacy_grenade_damage 1`,
/// linear from 100 to 0 at 350 (the original game's). Armour by the
/// script's ratio (H1, measured), the push capped at 75 kg x 400 units/s
/// times the template's 1.5.
pub fn he_blast() -> Blast {
    Blast {
        damage: 100.0 * HP,
        radius: 350.0 * UNIT,
        bell_sigma: Some(350.0 / 3.0 * UNIT),
        legacy: true,
        // Measured (Q4): the HE script's WeaponArmorRatio, × 0.5.
        armor_ratio: Some(1.475),
        quantum: HP,
        probe_up: 8.0 * UNIT,
        probe_down: 24.0 * UNIT,
        pull_out: 0.6 * UNIT,
        src_lift: UNIT,
        body_target: (0.7, 1.0),
        force: (100.0f32 * 300.0).min(30000.0) * 1.5 * UNIT,
        force_jitter: (0.85, 1.15),
        // Source physics' speed limit, 2000 units/s.
        max_push_speed: 2000.0 * UNIT,
        // The explosion effect's magnitude (the damage, 100) as a line of
        // 100 units at the blast, from 32 units below it (spec 5.3).
        ragdolls: Some(RagdollBlast {
            length: 100.0 * UNIT,
            min: UNIT,
            drop: 32.0 * UNIT,
        }),
        scorch: Some("scorch".into()),
        sound: Some("BaseGrenade.Explode".into()),
        hearing: Some(BlastHearing {
            shock_damage: SHOCK_DAMAGE * HP,
            shock: SHOCK_MUFFLE,
            ring_distance: EAR_RING_DISTANCE * UNIT,
            ring: EXPLOSION_RING,
        }),
        shake: Some(he_shake()),
        // Measured 2026-10-18 (grenades.md Q10): a character hidden
        // behind a crate kept 0.767 of the blast (an HE's and an
        // env_explosion's alike), behind a player 0.733; a crate over
        // its head only 0.813, over one side 0.977-0.79.
        shield: Some(BlastShield {
            prop: 0.767,
            character: 0.733,
            head: 0.8,
        }),
        through_walls: false,
        radius_rule: None,
    }
}

/// The HE grenade's view shake: a GUESS (spec 5.2: the template's base
/// grenade has none; whether CS:S shakes is Q9). Modest: 2 units at the
/// blast, none past 700 units, 30 changes a second, over 1 s.
pub fn he_shake() -> Shake {
    Shake {
        amplitude: 2.0 * UNIT,
        frequency: 30.0,
        duration: 1.0,
        radius: 700.0 * UNIT,
    }
}

// The hearing effects (spec 5.5, 6.4): the install's DSP presets (values
// from scripts/dsp_presets.txt). Each row's duration holds the preset; its
// fade time -1 means a 1 s exponential fade. We use the top of each
// preset's mix range (sounds far from the listener get the most) and
// stand in for its parts: a low-pass filter as is, a diffusor (all-pass
// smearing) as a gentle low-pass (`DIFFUSOR_CUTOFF`), a 3 kHz sine LFO as
// a ringing tone at the LFO's gain.

/// Blast damage from which the shock effect plays, hp (spec 5.5).
pub const SHOCK_DAMAGE: f32 = 30.0;
/// Closer than this (units) the ring effect plays (spec 5.5).
pub const EAR_RING_DISTANCE: f32 = 240.0;
/// Our stand-in cutoff for a diffusor, Hz (ours).
pub const DIFFUSOR_CUTOFF: f32 = 2500.0;
/// The presets' ringing LFO, Hz.
pub const RING_HZ: f32 = 3000.0;

/// Presets 32-34, "explosion ring" (identical rows): diffusor, then a
/// 1 kHz low-pass at gain 0.25; mix 0.2-0.7; 1.6 s, exponential fade.
pub const EXPLOSION_RING: HearingEffect = HearingEffect {
    hold: 1.6,
    fade: 1.0,
    exponential: true,
    mix: 0.7,
    cutoff: 1000.0,
    wet_gain: 0.25,
    ring_hz: RING_HZ,
    ring_gain: 0.0,
};

/// Presets 35-37, "shock muffle" (identical rows): diffusor, then a 3 kHz
/// sine LFO at gain 0.25; mix 0.2-0.7; 1.6 s, exponential fade.
pub const SHOCK_MUFFLE: HearingEffect = HearingEffect {
    hold: 1.6,
    fade: 1.0,
    exponential: true,
    mix: 0.7,
    cutoff: DIFFUSOR_CUTOFF,
    wet_gain: 1.0,
    ring_hz: RING_HZ,
    ring_gain: 0.25,
};

/// Presets 134-136, "flashbang muffle" long / medium / short (spec 6.4):
/// diffusor plus a 3 kHz sine LFO at gain 0.05; mix up to 0.7 / 0.4 /
/// 0.2, held 1.6 / 0.2 / 0.1 s, exponential fade.
pub const FLASH_MUFFLE: [HearingEffect; 3] = [
    flash_muffle(0.7, 1.6),
    flash_muffle(0.4, 0.2),
    flash_muffle(0.2, 0.1),
];

const fn flash_muffle(mix: f32, hold: f32) -> HearingEffect {
    HearingEffect {
        hold,
        fade: 1.0,
        exponential: true,
        mix,
        cutoff: DIFFUSOR_CUTOFF,
        wet_gain: 1.0,
        ring_hz: RING_HZ,
        ring_gain: 0.05,
    }
}

/// Which flashbang muffle a blindness gets (spec 6.4 hypothesis, Q14:
/// by its strength): the long one from 60 % of the longest blindness
/// (`FLASH_MAX_TIME`), the medium one from 30 %, else the short one.
pub fn flash_hearing(_alpha: f32, seconds: f32) -> Option<HearingEffect> {
    let s = seconds / FLASH_MAX_TIME;
    Some(if s >= 0.6 {
        FLASH_MUFFLE[0]
    } else if s >= 0.3 {
        FLASH_MUFFLE[1]
    } else {
        FLASH_MUFFLE[2]
    })
}

/// How far a flash reaches, units (measured: blindness falls linearly to
/// 0 at 1500; none at 1178+ facing away, grenades.md "CS:S values").
pub const FLASH_RADIUS: f32 = 1500.0;
/// The longest blindness, seconds, at 0 units facing the grenade
/// (measured: 7.14 × (1 − d/1500), e.g. 6.8 s at 72, 2.3 s at 1018), and
/// how much of it is the fade (FIT).
pub const FLASH_MAX_TIME: f32 = 7.14;
pub const FLASH_MAX_FADE: f32 = 3.0;

/// The flash model (spec 6.2, measured on the probe server, grenades.md
/// "CS:S values"): the blindness lasts `FLASH_MAX_TIME` × (1 − d /
/// `FLASH_RADIUS`) × (0.7 + 0.3 × the cosine of the angle to the grenade:
/// 1 facing it, 0.7 side-on, 0.4 facing away); its whiteness (max alpha)
/// is full facing it or side-on and 200/255 facing away, at any distance
/// (between side-on and behind: our linear blend, unmeasured). Held, then
/// fading over at most `FLASH_MAX_FADE` (the overlay curve is unmeasured).
pub fn flash_model(distance: f32, facing: f32) -> Option<(f32, f32, f32)> {
    let r = distance / UNIT;
    if r >= FLASH_RADIUS {
        return None;
    }
    let near = 1.0 - r / FLASH_RADIUS;
    let facing = facing.clamp(-1.0, 1.0);
    let look = 0.7 + 0.3 * facing;
    let s = near * look;
    if s < 0.05 {
        return None;
    }
    let time = FLASH_MAX_TIME * s;
    let fade = time.min(FLASH_MAX_FADE);
    let alpha = if facing >= 0.0 { 1.0 } else { 1.0 + facing * (1.0 - 200.0 / 255.0) };
    Some((alpha, time - fade, fade))
}

/// The smoke cloud (spec 7): 240 units across in 1 s, held to 15 s, gone
/// by 20 s (measured: the cloud entity's fade start and end, grenades.md
/// "CS:S values"). Bots' sight is blocked through three quarters of its radius
/// (Q16 hypothesis: the sprites reach E/2 plus their 80-unit size).
pub fn smoke() -> Smoke {
    Smoke {
        max_radius: 240.0 * UNIT,
        expand_time: 1.0,
        fade_start: 15.0,
        fade_end: 20.0,
        // Q15 hypothesis: it pops once (nearly) still.
        pop_speed: 0.1 * UNIT,
        sight_radius: 0.75,
        sound: Some("BaseSmokeEffect.Sound".into()),
    }
}

/// The `cs_source:grenade` hook's parameters: which grenade, how many a
/// player carries (`ammo_*_max`, spec 1) and its bounce sound.
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GrenadeParams {
    pub effect: GrenadeData,
    pub max: u32,
    pub bounce_sound: String,
}

#[derive(serde::Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum GrenadeData {
    He,
    Flash,
    Smoke,
}

/// The `cs_source:grenade` hook: CS:S's throw, flight and effect on a
/// grenade weapon (the data's `draw` is also the redraw after a throw).
pub fn throwable(e: &mut EntityWorldMut, d: &'static WeaponData, p: &GrenadeParams) {
    let effect = match p.effect {
        GrenadeData::He => GrenadeEffect::Blast(he_blast()),
        GrenadeData::Flash => GrenadeEffect::Flash(Flash {
            model: flash_model,
            sound: Some("Flashbang.Explode".into()),
            hearing: Some(flash_hearing),
            // Measured 2026-10-18 (grenades.md Q12): a flash going off in
            // a smoke cloud 40 units from the victim blinded it 3.81 s
            // instead of 6.80 (three throws), at full whiteness.
            through_smoke: 0.56,
        }),
        GrenadeData::Smoke => GrenadeEffect::Smoke(smoke()),
    };
    e.insert(Throwable {
        count: 1,
        max: p.max,
        pin_time: PIN_TIME,
        // Q1: the template's release + 0.1 s (not the throw event).
        throw_delay: 0.1,
        throw_time: THROW_TIME,
        redraw_time: d.draw,
        // Q8: the template's 1.5 s, checked every 0.2 s.
        fuse: 1.5,
        throw: throw_rule(),
        flight: flight(),
        effect,
        pin_sounds: vec![(PIN_SOUND_AT, "Default.PullPin_Grenade".into())],
        bounce_sound: Some(p.bounce_sound.clone()),
        water_sounds: Some(("BaseEntity.EnterWater".into(), "BaseEntity.ExitWater".into())),
        pin: false,
        throw_at: None,
        redraw_at: None,
    });
}

// ---------------------------------------------------------------------------
// The HE explosion (spec 5.4)

pub use source_engine::effects::explosions::{FxMaterials, blast_direction, explosion};
use source_engine::effects::explosions::light_at;

fn explosions(
    mut detonated: MessageReader<Detonated>,
    materials: Option<Res<ParticleMaterials>>,
    light: Option<Res<LightField>>,
    mut particles: ResMut<Particles>,
    mut rng: ResMut<FxRng>,
    world: WorldTracer,
) {
    let Some(materials) = materials else {
        detonated.clear();
        return;
    };
    let mats = FxMaterials::new(&materials.0);
    for d in detonated.read() {
        if d.kind != GrenadeKind::Blast || !draws_blast_of(d.weapon) {
            continue;
        }
        let a = blast_direction(d.at, &world);
        let l = light_at(light.as_deref(), d.at + a * 32.0 * UNIT);
        for g in explosion(&mut rng.0, &mats, d.at, a, l, &world) {
            particles.add(g);
        }
    }
}

// ---------------------------------------------------------------------------
// The smoke cloud (spec 7.3)

/// Grid offsets per axis, units.
const GRID: [f32; 4] = [-120.0, -40.0, 40.0, 120.0];
const SPRITE_SIZE: f32 = 80.0;

/// One sprite of a cloud: its grid slot, grey, roll, and a swap in
/// progress with another sprite (index, start, duration).
#[derive(Clone, Debug)]
struct CloudSprite {
    slot: usize,
    grey: f32,
    roll: f32,
    roll_speed: f32,
    swap: Option<(usize, f64, f32)>,
}

/// A cloud's sprites and each grid slot's light, made when it is first
/// drawn.
#[derive(Component, Clone, Debug)]
pub struct CloudSprites {
    sprites: Vec<CloudSprite>,
    light: Vec<Vec3>,
    last: f64,
}

fn slot_offset(slot: usize) -> Vec3 {
    Vec3::new(GRID[slot % 4], GRID[(slot / 4) % 4], GRID[slot / 16]) * UNIT
}

/// A sprite's alpha at distance `l` from the centre of a cloud of radius
/// `e` (spec 7.3), before the cloud's alpha: none beyond the edge, full in
/// the inner 70 %.
pub fn sprite_alpha(l: f32, e: f32) -> f32 {
    if e <= 0.0 || l > e {
        return 0.0;
    }
    let a = 1.0 - l / e;
    if a > 0.3 { 1.0 } else { a / 0.3 }
}

#[allow(clippy::type_complexity)]
fn draw_clouds(
    mut clouds: Query<(Entity, &SmokeCloud, Option<&mut CloudSprites>)>,
    materials: Option<Res<ParticleMaterials>>,
    light: Option<Res<LightField>>,
    mut particles: ResMut<Particles>,
    mut rng: ResMut<FxRng>,
    time: Res<Time>,
    mut commands: Commands,
) {
    let Some(materials) = materials else { return };
    let Some(mat) = FxMaterials::new(&materials.0).cloud else {
        return;
    };
    let now = time.elapsed_secs_f64();
    let rng = &mut rng.0;
    for (e, cloud, sprites) in &mut clouds {
        if cloud.weapon.is_some_and(|w| !draws_blast_of(w)) {
            continue;
        }
        let Some(mut s) = sprites else {
            let light = (0..64)
                .map(|i| light_at(light.as_deref(), cloud.centre + slot_offset(i)))
                .collect();
            let sprites = (0..64)
                .map(|slot| CloudSprite {
                    slot,
                    grey: rng.int(0, 255) as f32 / 255.1,
                    roll: rng.float(-6.0, 6.0),
                    roll_speed: rng.float(-0.1, 0.1),
                    swap: None,
                })
                .collect();
            commands.entity(e).insert(CloudSprites {
                sprites,
                light,
                last: now,
            });
            continue;
        };
        let dt = (now - s.last).max(0.0) as f32;
        s.last = now;
        churn(&mut s, rng, now);
        let t = (now - cloud.started) as f32;
        let radius = cloud.radius(t);
        let alpha = cloud.alpha(t);
        let mut g = ParticleGroup::new(Motion {
            near_fade: Some((0.0, 10.0 * UNIT)),
            ..default()
        });
        g.capped = false;
        g.kill_after = Some(0.0);
        let slots: Vec<usize> = s.sprites.iter().map(|x| x.slot).collect();
        for i in 0..s.sprites.len() {
            let sp = s.sprites[i].clone();
            let (mut q, mut l) = (slot_offset(sp.slot), s.light[sp.slot]);
            if let Some((other, start, dur)) = sp.swap {
                let w = 0.5 + 0.5 * (std::f32::consts::PI * ((now - start) as f32 / dur).clamp(0.0, 1.0)).cos();
                q = q * w + slot_offset(slots[other]) * (1.0 - w);
                l = l * w + s.light[slots[other]] * (1.0 - w);
            }
            let dist = q.length();
            let a = sprite_alpha(dist, radius) * alpha;
            if a <= 0.0 {
                continue;
            }
            let pos = if dist <= radius / 2.0 {
                cloud.centre + q
            } else {
                cloud.centre + q * (radius / 2.0) / dist
            };
            let c = (0.5 + 0.1 * sp.grey) * l;
            s.sprites[i].roll += sp.roll_speed * dt;
            g.particles.push(Particle {
                color: (c + Vec3::splat(0.5)) / 2.0,
                alpha: a,
                roll: s.sprites[i].roll,
                ..Particle::new(pos, 1.0, mat, SPRITE_SIZE * UNIT)
            });
        }
        particles.add(g);
    }
}

/// Neighbouring sprites trade places over 10-20 s (spec 7.3, churn).
fn churn(s: &mut CloudSprites, rng: &mut ParticleRng, now: f64) {
    // Finished swaps exchange their slots.
    for i in 0..s.sprites.len() {
        if let Some((j, start, dur)) = s.sprites[i].swap
            && now - start >= dur as f64
            && i < j
        {
            let (a, b) = (s.sprites[i].slot, s.sprites[j].slot);
            s.sprites[i].slot = b;
            s.sprites[j].slot = a;
            s.sprites[i].swap = None;
            s.sprites[j].swap = None;
        }
    }
    let by_slot: Vec<usize> = {
        let mut v = vec![0; 64];
        for (i, sp) in s.sprites.iter().enumerate() {
            v[sp.slot] = i;
        }
        v
    };
    for i in 0..s.sprites.len() {
        if s.sprites[i].swap.is_some() {
            continue;
        }
        let slot = s.sprites[i].slot;
        let (x, y, z) = ((slot % 4) as i32, ((slot / 4) % 4) as i32, (slot / 16) as i32);
        let start = [rng.int(0, 2), rng.int(0, 2), rng.int(0, 2)];
        'search: for dx in 0..3 {
            for dy in 0..3 {
                for dz in 0..3 {
                    let o = [
                        (dx + start[0]) % 3 - 1,
                        (dy + start[1]) % 3 - 1,
                        (dz + start[2]) % 3 - 1,
                    ];
                    if o == [0, 0, 0] {
                        continue;
                    }
                    let (nx, ny, nz) = (x + o[0], y + o[1], z + o[2]);
                    if !(0..4).contains(&nx) || !(0..4).contains(&ny) || !(0..4).contains(&nz) {
                        continue;
                    }
                    let j = by_slot[(nx + ny * 4 + nz * 16) as usize];
                    if s.sprites[j].swap.is_some() {
                        continue;
                    }
                    let dur = rng.float(10.0, 20.0);
                    s.sprites[i].swap = Some((j, now, dur));
                    s.sprites[j].swap = Some((i, now, dur));
                    break 'search;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sprite_alphas_g27() {
        let e = 240.0;
        let corner = Vec3::splat(120.0).length();
        let edge = Vec3::new(120.0, 120.0, 40.0).length();
        let face = Vec3::new(120.0, 40.0, 40.0).length();
        let inner = Vec3::splat(40.0).length();
        assert!((sprite_alpha(corner, e) - 0.447).abs() < 1e-3);
        assert!((sprite_alpha(edge, e) - 0.912).abs() < 1e-3);
        assert_eq!(sprite_alpha(face, e), 1.0);
        assert_eq!(sprite_alpha(inner, e), 1.0);
    }

    #[test]
    fn flash_fit_is_stronger_near_and_facing() {
        let full = flash_model(100.0 * UNIT, 1.0).unwrap();
        assert_eq!(full.0, 1.0);
        let behind = flash_model(100.0 * UNIT, -1.0).unwrap();
        assert!(behind.1 + behind.2 < full.1 + full.2);
        let far = flash_model(1200.0 * UNIT, 1.0).unwrap();
        assert!(far.1 + far.2 < full.1 + full.2);
        assert!(flash_model(1600.0 * UNIT, 1.0).is_none());
    }

    /// A flash at a given distance and angle: how long the white and its
    /// after-image last (the blindness) and which ringing follows.
    #[test]
    fn flash_at_distance_and_angle_gives_after_image_and_ring_times() {
        let hearing = |d: f32, k: f32| {
            let (alpha, hold, fade) = flash_model(d * UNIT, k).unwrap();
            (hold + fade, flash_hearing(alpha, hold + fade).unwrap())
        };
        // The probe's numbers (grenades.md "CS:S values"): facing it, 6.8 s
        // at 72 units, 4.78 at 496, 2.3 at 1018; facing away 0.4 of that.
        for (d, k, want) in [(72.0, 1.0, 6.8), (496.0, 1.0, 4.78), (1018.0, 1.0, 2.29), (173.0, -1.0, 2.53)] {
            let (t, _) = hearing(d, k);
            assert!((t - want).abs() < 0.03, "{d} {k}: {t}");
        }
        // Whiteness by facing only: full facing it or side-on, 200 behind.
        assert_eq!(flash_model(1000.0 * UNIT, 1.0).unwrap().0, 1.0);
        assert_eq!(flash_model(100.0 * UNIT, 0.0).unwrap().0, 1.0);
        assert!((flash_model(100.0 * UNIT, -1.0).unwrap().0 * 255.0 - 200.0).abs() < 1e-3);
        // Hearing by the blindness's share of the longest: close and facing
        // it the long muffle (1.6 s held, then a 1 s fade), facing away or
        // halfway out the medium one, far the short one.
        let (_, h) = hearing(100.0, 1.0);
        assert_eq!(h, FLASH_MUFFLE[0]);
        assert!((h.length() - 2.6).abs() < 1e-5);
        assert_eq!(hearing(100.0, -1.0).1, FLASH_MUFFLE[1]);
        assert_eq!(hearing(750.0, 1.0).1, FLASH_MUFFLE[1]);
        assert_eq!(hearing(1200.0, 1.0).1, FLASH_MUFFLE[2]);
    }

    #[test]
    fn he_hearing_by_damage_then_distance() {
        let h = he_blast().hearing.unwrap();
        assert_eq!(h.effect(30.0 * HP, 300.0 * UNIT), Some(SHOCK_MUFFLE));
        assert_eq!(h.effect(29.0 * HP, 200.0 * UNIT), Some(EXPLOSION_RING));
        assert_eq!(h.effect(20.0 * HP, 250.0 * UNIT), None);
        // The shock rings; the explosion ring only muffles (a low-pass).
        assert!(SHOCK_MUFFLE.ring_gain > 0.0 && EXPLOSION_RING.ring_gain == 0.0);
    }

    #[test]
    fn churn_swaps_neighbours_and_keeps_every_slot() {
        let mut s = CloudSprites {
            sprites: (0..64)
                .map(|slot| CloudSprite {
                    slot,
                    grey: 0.0,
                    roll: 0.0,
                    roll_speed: 0.0,
                    swap: None,
                })
                .collect(),
            light: vec![Vec3::ONE; 64],
            last: 0.0,
        };
        let mut rng = ParticleRng::new(3);
        churn(&mut s, &mut rng, 0.0);
        assert!(s.sprites.iter().filter(|x| x.swap.is_some()).count() > 32);
        churn(&mut s, &mut rng, 30.0);
        let mut slots: Vec<usize> = s.sprites.iter().map(|x| x.slot).collect();
        slots.sort();
        assert_eq!(slots, (0..64).collect::<Vec<_>>());
    }
}
