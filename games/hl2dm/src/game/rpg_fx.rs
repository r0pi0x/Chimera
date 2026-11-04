//! The RPG's laser as it is seen (specs/hl2dm/effects_sounds.md 9 and the
//! "Projectile visuals" table; projectiles.md 6.1; Open questions 6):
//!
//! - **On or off** (`Hl2LaserOn`, on the player, the server's, replicated:
//!   net part `hl2dm_rpg_fx`): on while the RPG in its hand has its dot
//!   (`weapons::rpg::Hl2LaserDot`: after the draw, with rockets or a
//!   missile out; HL2:DM has no way to turn it off). The server plays
//!   `Weapon_RPG.LaserOn` and `Weapon_RPG.LaserOff` from the player as it
//!   turns on and off (`GameSound`: every client hears them).
//! - **The dot** (`laser_dots`): a `sprites/redglow1` glow quad, full
//!   width 16 ± 4 units drawn anew every frame, 4 units off the surface at
//!   the end of the dot's line (`rpg::DotTracer`: through windows, past
//!   its owner). For the local player the line starts at the current view
//!   (it never lags the crosshair, whatever the server's dot does); for
//!   everyone else at the owner's eye along its drawn eye angles
//!   (`RenderedView`), as each machine draws that player.
//! - **The beam and its glow** (`laser_beams`, through Chimera's
//!   `map::attached`): in first person, on the view model from attachment
//!   2 (`laser`) to 3 (`laser_end`), `effects/laser1_noz`, 2 units wide at
//!   both ends, red, its brightness drawn anew in 128 to 200 every frame, 4
//!   segments, no noise; the glow `sprites/redglow1`, full width 8 ± 2, at
//!   the beam's start. Everyone else sees the same on the world model
//!   from attachment 4 to 5 (`effects/laser1`, 1 wide): `guide` and
//!   `guide_target` of the stock `w_rocket_launcher.mdl`, which has 5
//!   attachments as the install's model loads (heavy `view_models.rs`;
//!   the spec's open question 6 counted 3).
//!
//! Known shortcuts (docs/tech-debt.md): the beam's fade length (128 × the
//! fraction of a 128-unit line, visual_entities.md 1) isn't drawn: the
//! beam is full brightness to its end; the owner's client hears the
//! laser's sounds when the server's state reaches it (the spec's "shared"
//! sounds are played at once by the owner's prediction); the dot of a
//! player outside the view is drawn as for any other (no dormant owner's
//! networked point). Why it's OK for now: the beam is 100 units long and
//! mostly hidden by the gun; the sounds are a latency late.

use bevy::prelude::*;
use chimera_sim::weapon::{CHAN_WEAPON, Inventory};

use super::{
    effects::AttachedPieces,
    weapons::rpg::{DotTracer, Hl2LaserDot},
};
use crate::{
    core::{Intent, LocalPlayer, MovementState},
    map::{
        GameSound, ParticleRng, PlaySound,
        attached::{AttachPoint, AttachedFx, AttachedPiece, BeamEnd},
        interp::{RenderedView, eye_view},
        particles::{Motion, Particle, ParticleGroup, ParticleMaterials, ParticleSet, Particles},
    },
    weapon::aim_of,
};

const U: f32 = 0.0254;

/// A particle drawn in one frame only.
const ONE_FRAME: f32 = 0.0005;

/// The dot's line, units (the RPG's `dot_range`).
pub const DOT_RANGE: f32 = 56755.84;

pub const GLOW: &str = "sprites/redglow1";
pub const VIEW_BEAM: &str = "effects/laser1_noz";
pub const WORLD_BEAM: &str = "effects/laser1";

/// The materials the laser draws with (loaded with the map's).
pub const MATERIALS: &[&str] = &[GLOW, VIEW_BEAM, WORLD_BEAM];

pub const LASER_ON: &str = "Weapon_RPG.LaserOn";
pub const LASER_OFF: &str = "Weapon_RPG.LaserOff";

/// The beam's attachments: the view model's 2 and 3 (`laser`,
/// `laser_end`), the world model's 4 and 5 (Source's numbers, from 1).
pub const VIEW_FROM: usize = 1;
pub const VIEW_TO: usize = 2;
pub const WORLD_FROM: usize = 3;
pub const WORLD_TO: usize = 4;

/// A player's RPG laser is on (the server's, replicated).
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq, Reflect, serde::Serialize, serde::Deserialize)]
#[reflect(Component)]
pub struct Hl2LaserOn;

#[derive(Resource)]
struct LaserRng(ParticleRng);

impl Default for LaserRng {
    fn default() -> Self {
        Self(ParticleRng::new(0x1A5E_2D07))
    }
}

/// The server's: each player's laser on while the RPG in hand has its
/// dot, with its sounds as it turns on and off.
fn laser_states(
    players: Query<(Entity, &Inventory, &Transform, Has<Hl2LaserOn>)>,
    dots: Query<(), With<Hl2LaserDot>>,
    mut sounds: MessageWriter<GameSound>,
    mut commands: Commands,
) {
    for (e, inv, at, was) in &players {
        let on = inv.active.is_some_and(|a| dots.contains(a));
        if on == was {
            continue;
        }
        if on {
            commands.entity(e).insert(Hl2LaserOn);
        } else {
            commands.entity(e).remove::<Hl2LaserOn>();
        }
        sounds.write(GameSound(PlaySound {
            source: Some(e),
            channel: Some(CHAN_WEAPON),
            ..PlaySound::at(if on { LASER_ON } else { LASER_OFF }, at.translation)
        }));
    }
}

/// The dot's glow quad: 4 units off the surface at `point`, full width
/// 16 ± 4 units (`draw` in -1 to 1), white.
pub fn dot_particle(material: usize, point: Vec3, normal: Vec3, draw: f32) -> Particle {
    let width = 16.0 + 4.0 * draw;
    Particle::new(point + normal * 4.0 * U, ONE_FRAME, material, width / 2.0 * U)
}

/// Every lit laser's dot this frame (see the module docs).
#[allow(clippy::type_complexity)]
fn laser_dots(
    lasers: Query<(Entity, &Transform, &MovementState, &Intent, Option<&RenderedView>, Has<LocalPlayer>), With<Hl2LaserOn>>,
    tracer: DotTracer,
    materials: Option<Res<ParticleMaterials>>,
    mut particles: ResMut<Particles>,
    mut rng: ResMut<LaserRng>,
) {
    let Some(mat) = materials.as_ref().and_then(|m| m.0.find(GLOW)) else { return };
    let mut g = ParticleGroup::new(Motion::default());
    g.skip_first = false;
    g.capped = false;
    for (owner, at, state, intent, view, local) in &lasers {
        // The local player's current view; the others' as drawn.
        let look = if local { eye_view(None, intent, state) } else { eye_view(view, intent, state) };
        let eye = at.translation + look.eye_offset;
        let dir = aim_of(look.yaw, look.pitch) * Vec3::NEG_Z;
        if let Some((d, n, _)) = tracer.trace(owner, eye, dir, DOT_RANGE * U) {
            let draw = rng.0.float(-1.0, 1.0);
            g.particles.push(dot_particle(mat, eye + dir * d, n, draw));
        }
    }
    if !g.particles.is_empty() {
        particles.add(g);
    }
}

/// The beam's and glow's pieces on one model (`view`: the view model's),
/// with this frame's brightness (0-255) and glow width draw (-1 to 1).
pub fn beam_pieces(view: bool, beam: Option<usize>, glow: Option<usize>, brightness: f32, glow_draw: f32) -> Vec<AttachedPiece> {
    let (from, to, width) = if view { (VIEW_FROM, VIEW_TO, 2.0) } else { (WORLD_FROM, WORLD_TO, 1.0) };
    let mut out = Vec::new();
    if let Some(material) = beam {
        out.push(AttachedPiece::Beam {
            from: AttachPoint::Index(from),
            to: BeamEnd::Attachment(AttachPoint::Index(to)),
            material,
            width: [width * U; 2],
            color: Vec3::new(1.0, 0.0, 0.0),
            alpha: brightness / 255.0,
            noise: 0.0,
            segments: 4,
            scroll: 0.0,
        });
    }
    if let Some(material) = glow {
        out.push(AttachedPiece::Sprite {
            at: AttachPoint::Index(from),
            material,
            width: (8.0 + 2.0 * glow_draw) * U,
            color: Vec3::ONE,
            alpha: 1.0,
        });
    }
    out
}

/// Every lit laser's beam and glow, added to its `AttachedFx` (after
/// `effects::clear_attached`, with the other pieces of the frame).
fn laser_beams(
    mut lasers: Query<(Entity, Option<&mut AttachedFx>), With<Hl2LaserOn>>,
    materials: Option<Res<ParticleMaterials>>,
    mut rng: ResMut<LaserRng>,
    mut commands: Commands,
) {
    let Some(m) = materials.as_ref() else { return };
    let (view_beam, world_beam, glow) = (m.0.find(VIEW_BEAM), m.0.find(WORLD_BEAM), m.0.find(GLOW));
    for (e, fx) in &mut lasers {
        let r = &mut rng.0;
        let view = beam_pieces(true, view_beam, glow, r.float(128.0, 200.0), r.float(-1.0, 1.0));
        let world = beam_pieces(false, world_beam, glow, r.float(128.0, 200.0), r.float(-1.0, 1.0));
        match fx {
            Some(mut fx) => {
                fx.view.extend(view);
                fx.world.extend(world);
            }
            None => {
                commands.entity(e).insert(AttachedFx { view, world });
            }
        }
    }
}

pub struct Hl2RpgFxPlugin;

impl Plugin for Hl2RpgFxPlugin {
    fn build(&self, app: &mut App) {
        use chimera_net::net::parts::NetAppExt;
        app.init_resource::<LaserRng>()
            .register_type::<Hl2LaserOn>()
            .add_message::<GameSound>()
            .add_systems(
                FixedUpdate,
                laser_states.after(crate::core::SimSet::Weapons).run_if(crate::core::authoritative),
            )
            .add_systems(
                Update,
                (
                    laser_dots.after(ParticleSet::Step).before(ParticleSet::Draw),
                    laser_beams.in_set(AttachedPieces),
                ),
            )
            .net_part("hl2dm_rpg_fx", register);
    }
}

/// The protocol: each player's laser on or off, from the server.
fn register(app: &mut App) {
    use bevy_replicon::prelude::AppRuleExt;
    use chimera_net::net::ordered::OrderedAppExt;
    app.replicate::<Hl2LaserOn>().ordered::<Hl2LaserOn>();
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Test 47's sizes: 4 units off the surface, full width 12 to 20.
    #[test]
    fn the_dot_sits_off_the_surface() {
        let p = dot_particle(0, Vec3::ZERO, Vec3::Z, -1.0);
        assert!((p.position.z - 4.0 * U).abs() < 1e-6);
        assert!((p.size.start * 2.0 / U - 12.0).abs() < 1e-3);
        let p = dot_particle(0, Vec3::ZERO, Vec3::Z, 1.0);
        assert!((p.size.start * 2.0 / U - 20.0).abs() < 1e-3);
    }

    /// The table's `rpg_beam_1st`, `rpg_beam_3rd` and `rpg_glow`.
    #[test]
    fn the_beam_and_glow_by_model() {
        let view = beam_pieces(true, Some(1), Some(2), 160.0, 0.5);
        let AttachedPiece::Beam { from, to, width, color, alpha, noise, segments, .. } = &view[0] else { panic!() };
        assert_eq!((from, to), (&AttachPoint::Index(1), &BeamEnd::Attachment(AttachPoint::Index(2))));
        assert!((width[0] / U - 2.0).abs() < 1e-4 && (width[1] / U - 2.0).abs() < 1e-4);
        assert_eq!((*color, *noise, *segments), (Vec3::new(1.0, 0.0, 0.0), 0.0, 4));
        assert!((alpha - 160.0 / 255.0).abs() < 1e-6);
        let AttachedPiece::Sprite { at, width, .. } = &view[1] else { panic!() };
        assert_eq!(at, &AttachPoint::Index(1));
        assert!((width / U - 9.0).abs() < 1e-4);
        let world = beam_pieces(false, Some(1), Some(2), 128.0, 0.0);
        let AttachedPiece::Beam { from, width, .. } = &world[0] else { panic!() };
        assert_eq!(from, &AttachPoint::Index(3));
        assert!((width[0] / U - 1.0).abs() < 1e-4);
    }
}
