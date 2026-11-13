//! Other players' Source bodies on a client posed as the server had them
//! at the tick drawn. A network server poses each body once a tick from
//! the tick's values (`player_anim::SimAnimPlugin`, into
//! `map::SimAnimator`), and its hitboxes, the ones lag compensation keeps
//! and a rewind tests, come from that pose. A client's own animation of
//! another player (`player_anim::PlayerAnimPlugin`, at its frame rate, on
//! eased values, from whenever it first saw the body) drifts from it: a
//! walking HL2:DM body's run cycle was 0.1 m off at the head on average,
//! up to 0.37 m. So the server sends the pose (`AnimNet`: the main
//! sequence's and the layers' cycles, each anchored to a tick so a steady
//! one is sent once, the pose parameters and the yaw) as `NetPose<R>` for
//! game `R`'s bodies, a client keeps it by server tick
//! (`interp::Snapshots`) and, after its own animation state decided
//! (`player_anim::PlayerAnimSystems`), poses each other player's body from
//! it at its render tick (`AnimNetPlugin`), and again before its ticks,
//! which pose the hitboxes from the bodies, without what drawing adds over
//! it (HL2:DM's head look, CS:S's hand IK): the head a shooter aims at is
//! the one a rewind to that tick tests.
//!
//! Known shortcuts: a followed body snaps from one main sequence to the
//! next (the server's pose has no fades: specs/hl2dm/player_models.md 7
//! step 3); the drawn head look and hand IK, which the hitboxes leave out,
//! move the drawn head and hands by a centimetre or two from their
//! hitboxes; a cycle is sent again whenever its rate changes (an
//! accelerating body's, each tick). Why it's OK for now: the hitboxes
//! match lag compensation's to the millimetre; a value is a few dozen
//! bytes.

use std::marker::PhantomData;

use bevy::prelude::*;
use chimera_net::net::interp::{Around, HearTicks, InterpClock, Snapshots, hold_heard, remove_snapshots, write_snapshot};

use crate::{
    core::{Health, LocalPlayer, NetRole, SimClock},
    map::{DriveAnimation, SimAnimator, anim::Animator},
    player_anim::{PlayerAnimRules, PlayerAnimStates, PlayerAnimSystems, SimAnimState, normalize},
};

/// A cycle on the network (`AnimNet`): sequence `sequence` was at cycle
/// `cycle` on server tick `tick` and moves `per_tick` a tick, so a value
/// stays the same, and is not sent again, while a sequence plays on at a
/// steady rate.
#[derive(Clone, Copy, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct NetCycle {
    pub sequence: u16,
    pub tick: u64,
    pub cycle: f32,
    pub per_tick: f32,
}

impl NetCycle {
    /// The cycle at server tick `t` (fractional; not wrapped).
    pub fn at(&self, t: f64) -> f32 {
        self.cycle + (t - self.tick as f64) as f32 * self.per_tick
    }

    /// Whether this one still says cycle `cycle` of `sequence` on tick
    /// `t` (to `CYCLE_EPSILON`, round the loop when `looping`, else
    /// held at its end).
    fn says(&self, sequence: u16, cycle: f32, t: u64, looping: bool) -> bool {
        let d = if looping {
            let d = (self.at(t as f64) - cycle).rem_euclid(1.0);
            d.min(1.0 - d)
        } else {
            (self.at(t as f64).clamp(0.0, 1.0) - cycle).abs()
        };
        self.sequence == sequence && d < CYCLE_EPSILON
    }
}

/// How far a sent cycle may drift from the server's before it is sent
/// again (a cycle's fraction: a 1 s loop's 0.1 ms).
pub const CYCLE_EPSILON: f32 = 1e-4;

/// An overlay layer on the network (`AnimNet`).
#[derive(Clone, Copy, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct NetLayer {
    pub cycle: NetCycle,
    pub weight: f32,
}

/// A network server's pose of a body (`map::SimAnimator`: what its
/// hitboxes and lag compensation's kept ones are posed from) as its
/// clients are told it: the main sequence's and the layers' cycles, the
/// pose parameters (encoded, 0 to 65535) and the body's yaw (radians). A
/// client poses the body it draws from it at its render tick
/// (`AnimNet::pose`), so the hitboxes it draws, and aims at, are the ones
/// a rewind to that tick tests. The fading sequences aren't sent: the
/// server's pose has none (HL2:DM's: specs/hl2dm/player_models.md 7 step
/// 3).
#[derive(Clone, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct AnimNet {
    pub main: Option<NetCycle>,
    pub layers: Vec<NetLayer>,
    pub params: Vec<u16>,
    pub yaw: f32,
}

impl AnimNet {
    /// The pose `a` (a server's, at the end of tick `tick`, ticks `dt` s
    /// long) as `prev` (what was sent last) would carry it on: `prev`
    /// itself when it still says the same, else a new value.
    pub fn of(a: &Animator, tick: u64, dt: f32, prev: Option<&AnimNet>) -> AnimNet {
        let set = &*a.set;
        let cycle = |sequence: usize, cycle: f32, rate: f32| NetCycle {
            sequence: sequence.min(u16::MAX as usize) as u16,
            tick,
            cycle,
            per_tick: dt * set.cycle_rate(sequence, &a.params) * rate,
        };
        let main = a.main.map(|s| cycle(s, a.cycle, a.playback));
        let layers: Vec<NetLayer> = a
            .layers
            .iter()
            .flatten()
            .map(|l| NetLayer {
                cycle: cycle(l.sequence, l.cycle, 1.0),
                weight: l.weight,
            })
            .collect();
        let params = a.params.iter().map(|p| (p.clamp(0.0, 1.0) * 65535.0).round() as u16).collect();
        let yaw = a.yaw.unwrap_or(0.0);
        let mut next = AnimNet { main, layers, params, yaw };
        if let Some(prev) = prev {
            // Cycles still on the course sent keep their old value.
            let keep = |old: &NetCycle, new: &NetCycle| {
                let looping = set.sequences.get(new.sequence as usize).is_some_and(|q| q.looping);
                old.says(new.sequence, new.cycle, tick, looping)
            };
            if let (Some(old), Some(new)) = (&prev.main, &next.main)
                && keep(old, new)
            {
                next.main = Some(*old);
            }
            if prev.layers.len() == next.layers.len() {
                for (old, new) in prev.layers.iter().zip(&mut next.layers) {
                    if old.weight == new.weight && keep(&old.cycle, &new.cycle) {
                        new.cycle = old.cycle;
                    }
                }
            }
        }
        next
    }

    /// Pose `a` as the server had it at render tick `t` (fractional):
    /// from this value (the one at or before `t`), its pose parameters and
    /// yaw `f` of the way to `next` (the one after `t`, if any). Nothing
    /// changes for a sequence `a`'s set lacks (another model).
    pub fn pose(&self, next: Option<(&AnimNet, f32)>, t: f64, a: &mut Animator) {
        let set = a.set.clone();
        let Some(main) = self.main.filter(|m| (m.sequence as usize) < set.sequences.len()) else { return };
        let s = main.sequence as usize;
        a.main = Some(s);
        a.cycle = crate::map::anim::wrap_cycle(main.at(t), set.sequences[s].looping);
        a.playback = 1.0;
        a.fading.clear();
        a.layers = self
            .layers
            .iter()
            .filter(|l| (l.cycle.sequence as usize) < set.sequences.len())
            .map(|l| {
                Some(crate::map::anim::Layer {
                    sequence: l.cycle.sequence as usize,
                    cycle: l.cycle.at(t),
                    weight: l.weight,
                })
            })
            .collect();
        let decode = |v: u16| v as f32 / 65535.0;
        if self.params.len() == a.params.len() {
            for (i, (v, p)) in a.params.iter_mut().zip(&self.params).enumerate() {
                let x = decode(*p);
                *v = match next.filter(|(n, _)| n.params.len() == self.params.len()) {
                    Some((n, f)) => {
                        let mut d = decode(n.params[i]) - x;
                        // A wrapping parameter (`move_yaw`) the short way.
                        let wraps = set.params.get(i).is_some_and(|p| p.looping != 0.0 && (p.end - p.start).abs() >= p.looping * 0.999);
                        if wraps && d.abs() > 0.5 {
                            d -= d.signum();
                        }
                        let v = x + d * f;
                        if wraps { v.rem_euclid(1.0) } else { v.clamp(0.0, 1.0) }
                    }
                    None => x,
                };
            }
        }
        a.yaw = Some(match next {
            Some((n, f)) => self.yaw + normalize((n.yaw - self.yaw).to_degrees()).to_radians() * f,
            None => self.yaw,
        });
    }
}

/// Game `R`'s bodies' tick poses as clients are told them (`AnimNet`).
#[derive(Component, serde::Serialize, serde::Deserialize)]
#[serde(bound = "")]
pub struct NetPose<R: Send + Sync + 'static>(pub AnimNet, #[serde(skip)] PhantomData<fn() -> R>);

impl<R: Send + Sync + 'static> Clone for NetPose<R> {
    fn clone(&self) -> Self {
        Self(self.0.clone(), PhantomData)
    }
}

impl<R: Send + Sync + 'static> PartialEq for NetPose<R> {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}

impl<R: Send + Sync + 'static> std::fmt::Debug for NetPose<R> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

impl<R: Send + Sync + 'static> NetPose<R> {
    pub fn new(pose: AnimNet) -> Self {
        Self(pose, PhantomData)
    }
}

/// Where a client poses others' bodies from `NetPose`: in
/// `PlayerAnimSystems`, after the games' own animation states.
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub struct FollowServerPose;

/// Game `R`'s bodies' tick poses sent by a network server (`NetPose<R>`,
/// the network part named `name`) and followed by its clients.
pub struct AnimNetPlugin<R> {
    name: &'static str,
    game: PhantomData<fn() -> R>,
}

impl<R> AnimNetPlugin<R> {
    /// The plugin with its network part's name (unique among the parts).
    pub fn new(name: &'static str) -> Self {
        Self { name, game: PhantomData }
    }
}

impl<R: PlayerAnimRules> Plugin for AnimNetPlugin<R> {
    fn build(&self, app: &mut App) {
        use chimera_net::net::parts::NetAppExt;
        use bevy::app::RunFixedMainLoopSystems;
        use bevy_replicon::prelude::ServerSystems;
        app.configure_sets(Update, FollowServerPose.in_set(DriveAnimation).in_set(PlayerAnimSystems).after(PlayerAnimStates))
            // Out of the fixed schedules (their systems' order is a ratchet,
            // cs_source schedules.rs): once a frame, before replication
            // sends; the value carries its tick.
            .add_systems(PostUpdate, send::<R>.before(ServerSystems::Send).run_if(resource_equals(NetRole::Server)))
            .add_systems(Update, follow::<R>.in_set(FollowServerPose).run_if(resource_equals(NetRole::Client)))
            // Again before the frame's ticks, which pose the hitboxes from
            // the bodies (`map::ragdoll::record_poses`): without what the
            // last frame's drawing added over the server's pose (HL2:DM's
            // head turned toward a near player or a point ahead, CS:S's
            // hands locked on the weapon), at its render tick, the one the
            // ticks' shots are traced at (`predict`'s view tick).
            .add_systems(
                RunFixedMainLoop,
                follow_tick::<R>
                    .in_set(RunFixedMainLoopSystems::BeforeFixedMainLoop)
                    .run_if(resource_equals(NetRole::Client)),
            )
            .net_part(self.name, register::<R>);
    }
}

fn register<R: PlayerAnimRules>(app: &mut App) {
    use bevy_replicon::prelude::*;
    use chimera_net::net::ordered::OrderedAppExt;
    app.replicate::<NetPose<R>>()
        .set_receive_fns::<NetPose<R>>(write_snapshot::<NetPose<R>>, remove_snapshots::<NetPose<R>>)
        .ordered_with::<NetPose<R>>(write_snapshot::<NetPose<R>>, remove_snapshots::<NetPose<R>>)
        .add_systems(PreUpdate, hold_heard::<NetPose<R>>.after(HearTicks).run_if(resource_equals(NetRole::Client)));
}

/// Each of game `R`'s bodies' tick pose (`SimAnimator`) as `NetPose<R>`,
/// changed only when it says something new; gone from a body no longer
/// the game's.
#[allow(clippy::type_complexity)]
fn send<R: PlayerAnimRules>(
    clock: Res<SimClock>,
    mut bodies: Query<(Entity, Option<&SimAnimator>, Option<&mut NetPose<R>>, Has<SimAnimState<R>>)>,
    mut commands: Commands,
) {
    let dt = clock.dt();
    for (e, sim, net, ours) in &mut bodies {
        let Some(sim) = sim.filter(|_| ours) else {
            if net.is_some() {
                commands.entity(e).remove::<NetPose<R>>();
            }
            continue;
        };
        let next = AnimNet::of(&sim.0, clock.tick, dt, net.as_deref().map(|n| &n.0));
        match net {
            Some(mut n) => {
                if n.0 != next {
                    n.0 = next;
                }
            }
            None => {
                commands.entity(e).insert(NetPose::<R>::new(next));
            }
        }
    }
}

type Followed<'w, 's, R> =
    Query<'w, 's, (&'static mut Animator, &'static NetPose<R>, Option<&'static Snapshots<NetPose<R>>>, Option<&'static Health>), Without<LocalPlayer>>;

/// Another player's body of game `R` on a client, drawn: posed as the
/// server had it at the render tick (the values around it).
fn follow<R: PlayerAnimRules>(clock: Option<Res<InterpClock>>, mut bodies: Followed<R>) {
    pose_followed(clock.as_deref(), &mut bodies, false);
}

/// The same before a client's ticks, for the hitboxes: the hands' IK
/// goals (the drawing's) left out.
fn follow_tick<R: PlayerAnimRules>(clock: Option<Res<InterpClock>>, mut bodies: Followed<R>) {
    pose_followed(clock.as_deref(), &mut bodies, true);
}

fn pose_followed<R: PlayerAnimRules>(clock: Option<&InterpClock>, bodies: &mut Followed<R>, no_ik: bool) {
    let Some(clock) = clock.filter(|c| c.running) else { return };
    let t = clock.render_tick;
    for (mut a, newest, kept, health) in bodies.iter_mut() {
        if health.is_some_and(|h| h.current <= 0.0) {
            continue;
        }
        match kept.map(|k| k.around(t)) {
            Some(Around::Between((_, x), (_, y), f)) => x.0.pose(Some((&y.0, f)), t, &mut a),
            Some(Around::After((_, x))) => x.0.pose(None, t, &mut a),
            _ => newest.0.pose(None, t, &mut a),
        }
        if no_ik {
            a.ik.clear();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use crate::map::anim::{AnimSet, Animation, Layer, PoseParam, Sequence};

    /// A looping 31-frame, 30 fps sequence (1 s a cycle), a one-shot
    /// gesture and a wrapping yaw parameter.
    fn set() -> Arc<AnimSet> {
        let anim = |frames| Animation {
            fps: 30.0,
            frames,
            ..default()
        };
        let seq = |anim, looping| Sequence {
            looping,
            grid: (1, 1),
            anims: vec![anim],
            ..default()
        };
        Arc::new(AnimSet {
            animations: vec![anim(31), anim(16)],
            sequences: vec![seq(0, true), seq(1, false)],
            params: vec![PoseParam {
                name: "move_yaw".into(),
                start: -180.0,
                end: 180.0,
                looping: 360.0,
            }],
            ..default()
        })
    }

    /// A steadily playing sequence is sent once; a client posing from it
    /// at any later tick has the server's cycle, and the parameters and
    /// yaw between two values.
    #[test]
    fn a_steady_cycle_is_sent_once_and_posed_at_any_tick() {
        let dt = 1.0 / 64.0;
        let mut server = Animator::new(set());
        server.play(0, 0.0);
        server.playback = 1.5;
        let mut sent: Option<AnimNet> = None;
        let mut values = 0;
        for tick in 0..200u64 {
            server.advance(dt, tick as f64 * dt as f64);
            let next = AnimNet::of(&server, tick, dt, sent.as_ref());
            if sent.as_ref() != Some(&next) {
                values += 1;
                sent = Some(next);
            }
            let mut client = Animator::new(set());
            sent.as_ref().unwrap().pose(None, tick as f64, &mut client);
            assert!((client.cycle - server.cycle).abs() < 2.0 * CYCLE_EPSILON, "tick {tick}: {} {}", client.cycle, server.cycle);
        }
        assert_eq!(values, 1, "a steady cycle is one value");
        // A gesture plays over it: a value with it, the main cycle kept.
        server.layers = vec![Some(Layer {
            sequence: 1,
            cycle: 0.0,
            weight: 1.0,
        })];
        let with = AnimNet::of(&server, 199, dt, sent.as_ref());
        assert_eq!(with.main, sent.as_ref().unwrap().main);
        assert_eq!(with.layers.len(), 1);
        // The yaw parameter the short way round, the yaw likewise.
        let mut a = with.clone();
        a.params = vec![(0.95 * 65535.0) as u16];
        a.yaw = 3.0;
        let mut b = with.clone();
        b.params = vec![(0.05 * 65535.0) as u16];
        b.yaw = -3.0;
        let mut client = Animator::new(set());
        a.pose(Some((&b, 0.5)), 200.0, &mut client);
        assert!(client.params[0] < 0.01 || client.params[0] > 0.99, "{}", client.params[0]);
        let yaw = client.yaw.unwrap();
        assert!((yaw.abs() - std::f32::consts::PI).abs() < 0.01, "{yaw}");
    }
}
