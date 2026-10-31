//! Effects that follow an entity (effects_sounds.md "The effect file",
//! placement modes; special_weapons.md Follow-up (2026-10-30) 1-5;
//! game_modes.md Corrections (2026-10-30) 4, 7), on every machine that
//! draws the entity:
//!
//! - `CaAttachedGroups` on an entity lists the effect groups it carries;
//!   `follow` makes each once on it (`fx::CaFx::attach`, where the entity
//!   is), gives them its pose every frame (its point, its frame, a
//!   character's sockets: `characters::CaPose::sockets`) so each key
//!   places itself by its placement mode, hides those seen in third
//!   person only on the local player in first person (and with the
//!   entity's visibility), and shuts them down when the entity or the
//!   listing goes (smooth: particles live out; else at once).
//! - A projectile's look in flight comes with its drawn key
//!   (`special::ProjectileLook`, `on_shown`): its `FXName` group
//!   (`FXLoop`, `FXSmoothShutdown`), its `FlySound` looping and following
//!   it (`fly_sound`, stopped when it goes: `fly_sound_stop`), its flare
//!   and smoke trail (`flight`).
//! - A character's face lists its looping groups (`ClientFX#`: the
//!   infected faces' eyes on `EYE_L`/`EYE_R`); `face_groups` lists them
//!   when the character's look changes, seen in third person only.
//!
//! - A projectile's sounds (`projectile_sounds`, `flight_sounds`; special_
//!   weapons.md Corrections (2026-10-31) 3): the fly sound loops while it
//!   moves, the activate sound loops where it comes to rest, the finish
//!   sound plays once `FinishSoundTime` before its lifetime ends; one at a
//!   time. Impact projectiles carry the look in their drawn key; devices,
//!   grenades and bolts get it as a component where the server makes
//!   them.
//!
//! Known shortcuts (docs/tech-debt.md): `PlaySound` keys of attached
//! groups play nothing (the stock projectile and eye groups have none);
//! a device's, grenade's or bolt's sounds are made only where the server
//! is (a listen server, local play), not on a network client, whose
//! drawn key doesn't carry them; the activate sound's "velocity is zero"
//! is the object's own where it has one, else whether it moved since the
//! last frame; the flare is additive (the spec says unlit only) and
//! square (`FlareScale`'s x); a character's sockets come from the posed
//! tick (`CaPose`), placed at the drawn transform, and its `Flash` socket
//! (the held launcher's) is the hand's; the body and head records'
//! `ClientFX#` lists (none in the stock data) and a host's own override
//! aren't read; spectating in first person still shows the watched
//! player's groups.

use bevy::prelude::*;

use super::{
    fx::{CaFx, ParentPose, SmokeTrail, effect_frame},
    library,
    sounds::CaSound,
};
use super::super::{
    characters::{CaPose, records::Selection},
    data::CaData,
    inventory::UNIT,
    special::{PROJECTILE_KEY, ProjectileLook, ProjectileParams},
    view_model::attached::CaLoopSound,
};
use crate::{
    core::{LocalPlayer, MovementState},
    map::{
        ShowLocalBody, SoundKey,
        loose::ShownItem,
        particles::{ParticleMaterials, Particles},
    },
};

/// The effect groups an entity carries.
#[derive(Component, Clone, Debug, Default, PartialEq)]
pub struct CaAttachedGroups(pub Vec<AttachedGroup>);

/// One group on an entity.
#[derive(Clone, Debug, PartialEq)]
pub struct AttachedGroup {
    pub group: String,
    pub looping: bool,
    pub smooth: bool,
    /// Seen in third person only: hidden on the local player while its
    /// body isn't drawn (first person).
    pub third_person: bool,
}

/// A projectile's look in flight (from its drawn key: `on_shown`).
#[derive(Component, Clone, Debug, PartialEq)]
pub struct CaFlightLook(pub ProjectileLook);

/// Keys of the projectiles' fly sounds: this bit and the entity.
pub const FLY_SOUND_KEYS: u64 = 1 << 57;

/// The fly sound key of projectile `e`.
pub fn fly_sound_key(e: Entity) -> SoundKey {
    SoundKey(FLY_SOUND_KEYS | (e.to_bits() & 0xffff_ffff_ffff))
}

/// A Combat Arms impact projectile drawn: its look in flight from its
/// key (special_weapons.md Follow-up 1-3).
pub fn on_shown(ev: On<Insert<ShownItem>>, items: Query<&ShownItem>, mut commands: Commands) {
    let Ok(item) = items.get(ev.event_target()) else { return };
    if !item.0.starts_with(PROJECTILE_KEY) && !ProjectileParams::is_modelless(&item.0) {
        return;
    }
    let look = ProjectileLook::parse(&item.0);
    if look.is_empty() {
        return;
    }
    let mut e = commands.entity(ev.event_target());
    if !look.fx.is_empty() {
        e.insert(CaAttachedGroups(vec![AttachedGroup {
            group: look.fx.clone(),
            looping: look.fx_loop,
            smooth: look.fx_smooth,
            third_person: false,
        }]));
    }
    if let Some(kind) = look.smoke_trail {
        e.insert(SmokeTrail { kind, ..default() });
    }
    e.insert((CaFlightSounds::default(), CaFlightLook(look)));
}

/// Which of a projectile's sounds plays now (special_weapons.md
/// Corrections (2026-10-31) 3: one at a time, starting one stops the
/// current one).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum FlightSound {
    #[default]
    None,
    Fly,
    Activate,
    Finish,
}

/// A projectile effect's sound state on this client: what plays, how
/// long the effect has existed (T0 = when this client made it), where it
/// was last frame.
#[derive(Component, Clone, Debug, Default, PartialEq)]
pub struct CaFlightSounds {
    pub current: FlightSound,
    pub age: f32,
    pub last: Option<Vec3>,
}

/// One client update of a projectile effect's sounds (Corrections
/// (2026-10-31) 3), in the game's order: once the finish sound has
/// started nothing changes; the fly sound plays while the projectile
/// moves (and follows it: `CaLoopSound::follow`); the activate sound
/// starts, looping where the projectile then is, when it is at rest; the
/// finish sound starts once, `finish_time` s before `lifetime` is over.
/// Each sound is enabled by its name being non-empty. The sounds started
/// this update, in order; the last one plays.
pub fn flight_sounds(state: &mut FlightSound, look: &ProjectileLook, age: f32, at_rest: bool) -> Vec<FlightSound> {
    let mut started = Vec::new();
    if *state == FlightSound::Finish {
        return started;
    }
    if !look.fly_sound.is_empty() && !at_rest && *state != FlightSound::Fly {
        *state = FlightSound::Fly;
        started.push(*state);
    }
    if !look.activate_sound.is_empty() && *state != FlightSound::Activate && at_rest {
        *state = FlightSound::Activate;
        started.push(*state);
    }
    if !look.finish_sound.is_empty() && look.lifetime > 0.0 && age > look.lifetime - look.finish_time {
        *state = FlightSound::Finish;
        started.push(*state);
    }
    started
}

/// A projectile's sounds each client update (`flight_sounds`), 3D at
/// `SoundRadius`, volume 100: the fly sound loops and follows it, the
/// activate sound loops where it came to rest (not moved after), the
/// finish sound plays once where it is then.
pub fn projectile_sounds(
    time: Res<Time>,
    mut q: Query<(
        Entity,
        &CaFlightLook,
        &mut CaFlightSounds,
        &Transform,
        Option<&crate::game::special::CaObject>,
        Option<&crate::game::grenades::CaGrenade>,
    )>,
    mut out: MessageWriter<CaLoopSound>,
) {
    let dt = time.delta_secs();
    for (e, look, mut state, t, object, grenade) in &mut q {
        let at = t.translation;
        // The object's velocity where it has one, else whether it moved.
        let at_rest = match (object, grenade) {
            (Some(o), _) => o.velocity == Vec3::ZERO,
            (_, Some(g)) => g.velocity == Vec3::ZERO,
            _ => state.last.is_some_and(|l| l == at),
        };
        state.last = Some(at);
        let age = state.age;
        state.age += dt;
        let look = &look.0;
        if !look.has_sounds() {
            continue;
        }
        let mut now = state.current;
        for which in flight_sounds(&mut now, look, age, at_rest) {
            let (file, once, follow) = match which {
                FlightSound::Fly => (&look.fly_sound, false, Some(e)),
                FlightSound::Activate => (&look.activate_sound, false, None),
                _ => (&look.finish_sound, true, None),
            };
            out.write(CaLoopSound {
                key: fly_sound_key(e),
                sound: Some(CaSound::at(file.clone(), at, look.sound_radius)),
                follow,
                once,
            });
        }
        state.current = now;
    }
}

/// ... and stops when it goes (detonation, sky, lifetime, removal).
pub fn fly_sound_stop(ev: On<Remove<CaFlightLook>>, q: Query<&CaFlightLook>, mut out: MessageWriter<CaLoopSound>) {
    if q.get(ev.event_target()).is_ok_and(|l| l.0.has_sounds()) {
        out.write(CaLoopSound {
            key: fly_sound_key(ev.event_target()),
            sound: None,
            follow: None,
            once: false,
        });
    }
}

/// The groups a look wears: its face's `ClientFX#` list (game_modes.md
/// Corrections (2026-10-30) 4, 7), looping, third person only; the face
/// a selection names, else its body's default.
pub fn worn_groups(data: &CaData, look: &Selection) -> Vec<AttachedGroup> {
    data.characters()
        .worn_face(look)
        .map_or(&[][..], |f| &f.client_fx[..])
        .iter()
        .filter(|g| !g.is_empty())
        .map(|g| AttachedGroup {
            group: g.clone(),
            looping: true,
            smooth: false,
            third_person: true,
        })
        .collect()
}

/// A character's look changed: the groups its face lists (made at
/// infection with the infected face, gone when the face changes).
pub fn face_groups(
    ev: On<Insert<Selection>>,
    looks: Query<(&Selection, Option<&CaAttachedGroups>)>,
    data: Option<Res<CaData>>,
    mut commands: Commands,
) {
    let (Ok((look, have)), Some(data)) = (looks.get(ev.event_target()), data) else { return };
    let want = worn_groups(&data, look);
    match (want.is_empty(), have) {
        (true, Some(_)) => {
            commands.entity(ev.event_target()).remove::<CaAttachedGroups>();
        }
        (false, h) if h.map(|h| &h.0) != Some(&want) => {
            commands.entity(ev.event_target()).insert(CaAttachedGroups(want));
        }
        _ => {}
    }
}

/// An entity's pose for its groups: a character's point, its body's frame
/// and its sockets; anything else its transform's point and facing.
pub fn pose_of(t: &Transform, pose: Option<(&CaPose, &MovementState)>) -> ParentPose {
    match pose {
        Some((p, state)) => ParentPose {
            at: t.translation,
            frame: effect_frame(Quat::from_rotation_y(p.body_yaw) * Vec3::NEG_Z),
            sockets: p.sockets().map(|(n, at)| (n.to_string(), p.world(t, state, UNIT, at))).collect(),
        },
        None => ParentPose {
            at: t.translation,
            frame: effect_frame(t.rotation * Vec3::NEG_Z),
            sockets: Vec::new(),
        },
    }
}

/// Keep the groups on their entities: make the listed ones, give them
/// their entity's pose and visibility, shut down the rest.
#[allow(clippy::type_complexity)]
pub fn follow(
    parents: Query<(
        Entity,
        &CaAttachedGroups,
        &Transform,
        Option<&CaPose>,
        Option<&MovementState>,
        Option<&Visibility>,
        Has<LocalPlayer>,
    )>,
    show_local: Option<Res<ShowLocalBody>>,
    mut fx: ResMut<CaFx>,
    mut particles: Option<ResMut<Particles>>,
) {
    fx.shut_down(particles.as_deref_mut(), |e, g| {
        parents
            .get(e)
            .is_ok_and(|p| p.1.0.iter().any(|a| a.group.eq_ignore_ascii_case(g)))
    });
    let Some(lib) = library::library() else { return };
    let show_local = show_local.is_some_and(|s| s.0);
    for (e, groups, t, pose, state, vis, local) in &parents {
        let p = pose_of(t, pose.zip(state));
        let hidden_any = vis == Some(&Visibility::Hidden);
        let first_person = local && !show_local;
        for g in &groups.0 {
            if fx.attached(e).any(|i| i.group.eq_ignore_ascii_case(&g.group)) {
                continue;
            }
            fx.attach(&lib, &g.group, e, g.looping, g.smooth, p.clone());
        }
        // Each group's visibility: third-person ones hide in first person.
        for g in &groups.0 {
            let hidden = hidden_any || (g.third_person && first_person);
            fx.set_pose_of(e, &g.group, &p, hidden);
        }
    }
}

/// Projectiles' flares and smoke trails this frame (Follow-up 3).
pub fn flight(
    time: Res<Time>,
    mut projectiles: Query<(&CaFlightLook, &Transform, Option<&mut SmokeTrail>)>,
    mut fx: ResMut<CaFx>,
    particles: Option<ResMut<Particles>>,
    materials: Option<Res<ParticleMaterials>>,
    window: Query<&Window, With<bevy::window::PrimaryWindow>>,
) {
    let (Some(lib), Some(mut particles), Some(materials)) = (library::library(), particles, materials) else { return };
    let dt = time.delta_secs();
    if let Ok(w) = window.single() {
        fx.screen_width = w.resolution.physical_width() as f32;
    }
    for (look, t, trail) in &mut projectiles {
        if let Some(f) = &look.0.flare {
            let at = t.translation + Vec3::Y * f.y_offset * fx.unit;
            fx.flare(&lib, &f.sprite, f.scale[0], at, &mut particles, &materials.0);
        }
        if let Some(mut trail) = trail {
            fx.smoke(&lib, &mut trail, t.translation, dt, &mut particles, &materials.0);
        }
    }
}
