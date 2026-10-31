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
//! Known shortcuts (docs/tech-debt.md): `PlaySound` keys of attached
//! groups play nothing (the stock projectile and eye groups have none);
//! the trail key's width style and `UAdd` aren't applied (constant width,
//! the texture once from tail to head: effects_sounds.md names them
//! only), nor any fade of its sections; a projectile's `ActivateSound`,
//! `FinishSound` and `CreateSound` aren't read; smoke puffs live their
//! segment's life from their own birth; the flare is additive (the spec
//! says unlit only) and square (`FlareScale`'s x); a character's sockets
//! come from the posed tick (`CaPose`), placed at the drawn transform;
//! the body and head records' `ClientFX#` lists (none in the stock data)
//! and a host's own override aren't read; spectating in first person
//! still shows the watched player's groups.

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
    e.insert(CaFlightLook(look));
}

/// The fly sound starts with the projectile (Follow-up 2): looping, 3D
/// with `SoundRadius` (inner 7 %), volume 100, following it.
pub fn fly_sound(ev: On<Add<CaFlightLook>>, q: Query<(&CaFlightLook, Option<&Transform>)>, mut out: MessageWriter<CaLoopSound>) {
    let Ok((look, t)) = q.get(ev.event_target()) else { return };
    if look.0.fly_sound.is_empty() {
        return;
    }
    let at = t.map_or(Vec3::ZERO, |t| t.translation);
    out.write(CaLoopSound {
        key: fly_sound_key(ev.event_target()),
        sound: Some(CaSound::at(look.0.fly_sound.clone(), at, look.0.sound_radius)),
        follow: Some(ev.event_target()),
    });
}

/// ... and stops when it goes (detonation, sky, lifetime, removal).
pub fn fly_sound_stop(ev: On<Remove<CaFlightLook>>, q: Query<&CaFlightLook>, mut out: MessageWriter<CaLoopSound>) {
    if q.get(ev.event_target()).is_ok_and(|l| !l.0.fly_sound.is_empty()) {
        out.write(CaLoopSound {
            key: fly_sound_key(ev.event_target()),
            sound: None,
            follow: None,
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
) {
    let (Some(lib), Some(mut particles), Some(materials)) = (library::library(), particles, materials) else { return };
    let dt = time.delta_secs();
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
