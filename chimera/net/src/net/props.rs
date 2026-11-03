//! Physics props in a network game (docs/plans/active/multiplayer.md,
//! slice 3): the server simulates them; a client draws them from
//! snapshots at the render time, as it draws other players (`interp`).
//!
//! - **Server.** After each tick, each physics prop's pose (on the network's
//!   grid, `quant`) and whether it is there (shown, solid) go into its
//!   node's `NetProp` (`write_props`); a prop at rest, or shaking below the
//!   grid, sends nothing.
//! - **Client.** Its own copy of the prop (the node with the same
//!   `map::PropIndex`) stops being simulated (a kinematic body, so its
//!   own player still bumps into it) and is drawn each frame between the
//!   two snapshots around the render time (position along a line, rotation
//!   the short way), snapped on a jump (a round restart puts props back).
//!   Pushing one is the server's to simulate: the client sees the result
//!   after the delay (Source the same).
//!
//! - **Held props** (lift C6, `map::held::Held`; specs/hl2dm/gravity_gun.md
//!   14): the server simulates them as any other (its game's controller
//!   drives them) and also sends who holds one and where relative to the
//!   holder's view (`NetHold`, `write_holds`). The holder's client draws it
//!   there from its own predicted eye and view, so it follows the view at
//!   once; every other client draws the snapshots. On each client the copy
//!   is marked `Held` by its holder (target none: nothing drives it), so
//!   the holder's own movement and shots pass it as on the server.
//!
//! Physics props: the server's into `NetProp` (`write_props`) and held ones'
//! holders into `NetHold` (`write_holds`); a client's copies made kinematic and
//! `NetDrawn`, drawn between snapshots at the render tick, a held one at its
//! holder's predicted view on the holder's client (`draw_props`).
//!
//! Known shortcuts: the holder's client places the held prop at the view
//! offset the server last sent (no local controller with Source's 2-tick
//! arrival, gravity_gun.md 14; a pitch past the game's clamp or a wall's
//! push-in shows a tick late).

use avian3d::prelude::{ColliderDisabled, RigidBody};
use bevy::{app::RunFixedMainLoopSystems, prelude::*};

use super::{
    NetCharacter, NetHold, NetProp,
    interp::{Around, InterpClock, Snapshots},
    prop_flags,
};
use crate::{
    core::{Intent, LocalPlayer, MovementState, NetRole},
    map::{
        held::Held,
        LogicHidden, PhysicsProp, PropIndex, VisClusters,
        interp::{InterpSystems, NetDrawn, SNAP_SPEED},
        prop_physics::FrozenBody,
    },
};

pub(super) fn plugin(app: &mut App) {
    app.add_systems(
        FixedLast,
        (write_props, write_holds).run_if(resource_equals(NetRole::Server)),
    )
        .add_systems(
            RunFixedMainLoop,
            draw_props
                .in_set(RunFixedMainLoopSystems::AfterFixedMainLoop)
                .after(InterpSystems::Ease)
                .after(super::interp::draw_others)
                .run_if(resource_equals(NetRole::Client))
                // The props are the map's: not while another map is loaded.
                .run_if(resource_exists::<super::client::Joined>),
        );
}

/// The server: each physics prop's state into its node's `NetProp`
/// (replicated from the first one on).
#[allow(clippy::type_complexity)]
fn write_props(
    mut q: Query<
        (
            Entity,
            &PropIndex,
            &Transform,
            Has<LogicHidden>,
            Has<ColliderDisabled>,
            Option<&mut NetProp>,
        ),
        Or<(With<PhysicsProp>, With<FrozenBody>)>,
    >,
    mut commands: Commands,
) {
    for (e, index, t, hidden, no_collision, have) in &mut q {
        let mut flags = 0;
        if !hidden {
            flags |= prop_flags::VISIBLE;
        }
        if !no_collision {
            flags |= prop_flags::SOLID;
        }
        // On the network's grid: a prop shaking below it (one the physics
        // never puts to sleep) changes nothing to send.
        let now = NetProp {
            index: index.0 as u32,
            origin: t.translation.to_array(),
            rotation: t.rotation.to_array(),
            flags,
        }
        .quantized();
        match have {
            Some(mut have) => {
                have.set_if_neq(now);
            }
            None => {
                commands.entity(e).insert((bevy_replicon::prelude::Replicated, now));
            }
        }
    }
}

/// The server: held props' holders and view offsets into `NetHold`.
fn write_holds(
    held: Query<(Entity, &PropIndex, &Held, Option<&mut NetHold>)>,
    let_go: Query<Entity, (With<NetHold>, Without<Held>)>,
    owners: Query<&NetCharacter>,
    mut commands: Commands,
) {
    let mut held = held;
    for (e, index, h, have) in &mut held {
        let (Some(holder), Some((offset, rotation))) = (owners.get(h.holder).ok().and_then(|c| c.owner), h.view_offset)
        else {
            if have.is_some() {
                commands.entity(e).remove::<NetHold>();
            }
            continue;
        };
        let now = NetHold {
            index: index.0 as u32,
            holder,
            offset: offset.to_array(),
            rotation: rotation.to_array(),
        };
        match have {
            Some(mut have) => {
                have.set_if_neq(now);
            }
            None => {
                commands.entity(e).insert((bevy_replicon::prelude::Replicated, now));
            }
        }
    }
    for e in &let_go {
        commands.entity(e).remove::<NetHold>();
    }
}

/// A client: its props where the server had them at the render time.
#[allow(clippy::type_complexity)]
fn draw_props(
    clock: Res<InterpClock>,
    command_clock: Res<super::predict::CommandClock>,
    fixed: Res<Time<Fixed>>,
    mut proxies: Query<&mut Snapshots<NetProp>>,
    mut props: Query<
        (
            Entity,
            &PropIndex,
            &mut Transform,
            Option<&RigidBody>,
            Has<NetDrawn>,
            Has<ColliderDisabled>,
            Option<&VisClusters>,
            Option<&Visibility>,
        ),
        Or<(With<PhysicsProp>, With<FrozenBody>)>,
    >,
    (holds, characters, me, local, mut marked): (
        Query<&NetHold>,
        Query<(Entity, &NetCharacter)>,
        Option<Res<super::client::LocalClientId>>,
        Query<(&Transform, &MovementState, &Intent), (With<LocalPlayer>, Without<PropIndex>)>,
        Query<(Entity, &PropIndex, &mut Held)>,
    ),
    mut commands: Commands,
) {
    if !clock.running || proxies.is_empty() {
        return;
    }
    // Held props: by index, the holder's character here and the offset.
    let holder_of = |id: u64| characters.iter().find(|(_, c)| c.owner == Some(id)).map(|(e, _)| e);
    let holds: std::collections::BTreeMap<u32, (&NetHold, Option<Entity>)> =
        holds.iter().map(|h| (h.index, (h, holder_of(h.holder)))).collect();
    for (e, index, mut h) in &mut marked {
        match holds.get(&(index.0 as u32)).and_then(|(_, by)| *by) {
            Some(by) if h.holder != by => h.holder = by,
            Some(_) => {}
            None => {
                commands.entity(e).remove::<Held>();
            }
        }
    }
    let my_view = local.single().ok().map(|(t, s, i)| {
        (t.translation + s.eye_offset, Quat::from_euler(EulerRot::YXZ, i.yaw, i.pitch, 0.0))
    });
    let at = clock.render_tick;
    let step = super::interp::tick_step(&command_clock, &fixed) as f32;
    let mut by_index: std::collections::HashMap<u32, (Transform, u16)> = std::collections::HashMap::new();
    for mut buf in &mut proxies {
        buf.prune(at);
        let pose = match buf.around(at) {
            Around::Empty => continue,
            Around::Before((_, p)) | Around::After((_, p)) => (pose_of(p), p.index, p.flags),
            Around::Between((ta, a), (tb, b), f) => {
                let (pa, pb) = (pose_of(a), pose_of(b));
                let far = SNAP_SPEED * step * (tb - ta) as f32;
                let t = if pa.translation.distance(pb.translation) > far {
                    pa
                } else {
                    Transform {
                        translation: pa.translation.lerp(pb.translation, f),
                        rotation: pa.rotation.slerp(pb.rotation, f),
                        scale: pa.scale,
                    }
                };
                (t, a.index, if f >= 1.0 { b.flags } else { a.flags })
            }
        };
        by_index.insert(pose.1, (pose.0, pose.2));
    }
    for (e, index, mut t, body, drawn, no_collision, vis, visibility) in &mut props {
        let Some((pose, flags)) = by_index.get(&(index.0 as u32)) else {
            continue;
        };
        if !drawn {
            // The server simulates it: here it only follows.
            commands.entity(e).insert(NetDrawn);
            if body.is_some_and(|b| b.is_dynamic()) {
                commands.entity(e).insert(RigidBody::Kinematic);
            }
        }
        let mut pose = Transform {
            scale: t.scale,
            ..*pose
        };
        if let Some((h, by)) = holds.get(&(index.0 as u32)) {
            if let Some(by) = by
                && !marked.contains(e)
            {
                commands.entity(e).insert(Held {
                    holder: *by,
                    target: None,
                    view_offset: None,
                    arrive: 0.0,
                    max_speed: 0.0,
                    max_angular: 0.0,
                });
            }
            // The holder's own: at its predicted view now.
            if me.as_deref().is_some_and(|m| m.0 == h.holder)
                && let Some((eye, view)) = my_view
            {
                pose.translation = eye + view * Vec3::from_array(h.offset);
                pose.rotation = (view * Quat::from_array(h.rotation)).normalize();
            }
        }
        if *t != pose {
            *t = pose;
        }
        let solid = flags & prop_flags::SOLID != 0;
        if solid == no_collision {
            if solid {
                commands.entity(e).remove::<ColliderDisabled>();
            } else {
                commands.entity(e).insert(ColliderDisabled);
            }
        }
        let shown = flags & prop_flags::VISIBLE != 0;
        let culled = vis.is_some_and(|v| !v.potentially_visible);
        let want = if !shown || culled { Visibility::Hidden } else { Visibility::Inherited };
        if visibility != Some(&want) {
            commands.entity(e).insert(want);
            if shown {
                commands.entity(e).remove::<LogicHidden>();
            } else {
                commands.entity(e).insert(LogicHidden);
            }
        }
    }
}

fn pose_of(p: &NetProp) -> Transform {
    Transform {
        translation: Vec3::from_array(p.origin),
        rotation: Quat::from_array(p.rotation).normalize(),
        scale: Vec3::ONE,
    }
}
