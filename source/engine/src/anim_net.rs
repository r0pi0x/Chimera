//! Other players' Source bodies on a client posed as the server had them
//! at the tick drawn. A network server poses each body once a tick from
//! the tick's values (`player_anim::SimAnimPlugin`, into
//! `map::SimAnimator`), and its hitboxes, the ones lag compensation keeps
//! and a rewind tests, come from that pose. A client's own animation of
//! another player (`player_anim::PlayerAnimPlugin`, at its frame rate, on
//! eased values, from whenever it first saw the body) drifts from it: a
//! walking HL2:DM body's run cycle was 0.1 m off at the head on average,
//! up to 0.37 m. So the server sends the pose for game `R`'s bodies in
//! two parts, each sent only when it changes (replicon sends a changed
//! component whole): `NetPose<R>` (`AnimNet`: the main sequence's and the
//! layers' cycles, each anchored to a tick so a steady one is sent once,
//! a layer locked to the main cycle sending none of its own, and the main
//! sequences fading out after a change, with the tick each was replaced
//! on) and `NetAim<R>` (`AimNet`: the pose parameters in 4095ths of
//! their range, the yaw in 16384ths of a turn: 0.044 and 0.022 degree
//! steps, sent again once one moved 8 or 11 steps). Every frame a client
//! carries the cycles on from their ticks to its render tick, weighs the
//! fades and lerps the layers' weights between the values around it, and
//! ramps the aim between the values it received (`AIM_RAMP_TICKS`), so a
//! body drawn above the tick rate moves without steps. A client keeps both by server tick
//! (`interp::Snapshots`) and, after its own animation state decided
//! (`player_anim::PlayerAnimSystems`), poses each other player's body from
//! them at its render tick (`AnimNetPlugin`), cross-fades included, and
//! again before its ticks, which pose the hitboxes from the bodies,
//! without what drawing adds over it (HL2:DM's head look, CS:S's hand
//! IK): the head a shooter aims at is the one a rewind to that tick
//! tests. In the CS:S network soak (twelve characters) the two cost each
//! client 2.7 and 1.3 KB/s (one 59-byte part, sent nearly every update:
//! 10.4 KB/s); a main sequence change and its fade add 7 bytes to the
//! value sent with it, and nothing is sent when a fade ends.
//!
//! Known shortcuts: the drawn head look and hand IK, which the hitboxes
//! leave out, move the drawn head and hands by a centimetre or two from
//! their hitboxes; a cycle is sent again once its rate change moved it
//! `CYCLE_EPSILON` (an accelerating body's, every few ticks); the aim
//! drawn trails the server's by up to `PARAM_SEND_STEPS` steps (0.35
//! degrees), and a sudden turn after a still spell ramps in over
//! `AIM_RAMP_TICKS`. Why it's OK for now: the hitboxes match lag
//! compensation's to the millimetre in the head matrix; the soak's
//! clients take 13.9-16.4 KB/s against their 18.

use std::marker::PhantomData;

use bevy::prelude::*;
use chimera_net::net::interp::{Around, HearTicks, InterpClock, Snapshots, hold_heard, remove_snapshots, write_snapshot};

use crate::{
    core::{Health, LocalPlayer, NetRole, SimClock},
    map::{DriveAnimation, SimAnimator, anim::Animator},
    player_anim::{PlayerAnimRules, PlayerAnimStates, PlayerAnimSystems, SimAnimState, normalize},
};

/// A cycle on the network (`AnimNet`): sequence `sequence` was at cycle
/// `cycle` (in 65535ths) on server tick `tick` (its low 16 bits: the
/// one nearest the tick it is read at; a value is sent again before it
/// is `MAX_AGE` ticks old) and moves `per_tick` a tick (a half float:
/// 0.05 % off, which `CYCLE_EPSILON` takes seconds to reach), so a value
/// stays the same, and is not sent again, while a sequence plays on at a
/// steady rate. Two bytes each but the sequence.
#[derive(Clone, Copy, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct NetCycle {
    pub sequence: u16,
    pub tick: [u8; 2],
    pub cycle: [u8; 2],
    pub per_tick: [u8; 2],
}

/// The oldest a sent cycle's tick may be (`NetCycle::tick`'s 16 bits).
pub const MAX_AGE: u64 = 30_000;

impl NetCycle {
    fn new(sequence: usize, tick: u64, cycle: f32, per_tick: f32) -> Self {
        Self {
            sequence: sequence.min(u16::MAX as usize) as u16,
            tick: (tick as u16).to_le_bytes(),
            cycle: ((cycle.clamp(0.0, 1.0) * 65535.0).round() as u16).to_le_bytes(),
            per_tick: half::f16::from_f32(per_tick).to_le_bytes(),
        }
    }

    /// Its cycles a tick.
    pub fn per_tick(&self) -> f32 {
        half::f16::from_le_bytes(self.per_tick).to_f32()
    }

    /// Its tick in full, the one nearest server tick `t`.
    pub fn tick_near(&self, t: f64) -> f64 {
        let low = u16::from_le_bytes(self.tick) as i64;
        let base = t.floor() as i64;
        let d = (low - base).rem_euclid(65536);
        (base + if d >= 32768 { d - 65536 } else { d }) as f64
    }

    /// The cycle at server tick `t` (fractional; not wrapped).
    pub fn at(&self, t: f64) -> f32 {
        u16::from_le_bytes(self.cycle) as f32 / 65535.0 + (t - self.tick_near(t)) as f32 * self.per_tick()
    }

    /// Whether this one still says cycle `cycle` of `sequence` on tick
    /// `t` (to `CYCLE_EPSILON`, round the loop when `looping`, else
    /// held at its end), and isn't too old to say it.
    fn says(&self, sequence: u16, cycle: f32, t: u64, looping: bool) -> bool {
        if t as f64 - self.tick_near(t as f64) >= MAX_AGE as f64 {
            return false;
        }
        let d = if looping {
            let d = (self.at(t as f64) - cycle).rem_euclid(1.0);
            d.min(1.0 - d)
        } else {
            (self.at(t as f64).clamp(0.0, 1.0) - cycle.clamp(0.0, 1.0)).abs()
        };
        self.sequence == sequence && d < CYCLE_EPSILON
    }
}

/// How far a sent cycle may drift from the server's before it is sent
/// again (a cycle's fraction: a 1 s loop's 5 ms, a running body's feet
/// a centimetre at most, its head a millimetre).
pub const CYCLE_EPSILON: f32 = 5e-3;

/// An overlay layer on the network (`AnimNet`): its own cycle, or none
/// when it plays at the main sequence's (CS:S's upper-body layers, locked
/// to the legs), and its weight in `WEIGHT_STEPS`ths.
#[derive(Clone, Copy, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct NetLayer {
    pub sequence: u16,
    pub cycle: Option<NetCycle>,
    pub weight: u8,
}

/// A network server's pose of a body (`map::SimAnimator`: what its
/// hitboxes and lag compensation's kept ones are posed from) as its
/// clients are told it, in two parts sent apart, so a steady one isn't
/// sent again when the other changes: the main sequence's and the
/// layers' cycles (`AnimNet`, `NetPose`) and the aim (`AimNet`,
/// `NetAim`: the pose parameters and the body's yaw). A client poses the
/// body it draws from them at its render tick (`pose`), so the hitboxes
/// it draws, and aims at, are the ones a rewind to that tick tests. The
/// fading sequences aren't sent: the server's pose has none (HL2:DM's:
/// specs/hl2dm/player_models.md 7 step 3).
#[derive(Clone, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct AnimNet {
    pub main: Option<NetCycle>,
    pub layers: Vec<NetLayer>,
    /// The main sequences fading out, the most recently replaced first
    /// (`map::anim::Fading`): sent when the main sequence changes.
    pub fading: Vec<NetFade>,
}

/// A main sequence fading out after it was replaced (`AnimNet`): its
/// cycle, carried on as the main one's from the tick it was replaced on
/// (`NetCycle::tick`).
/// Its fade time is its own fade-out against the fade-in of what replaced
/// it (the next newer one, or the main sequence: specs/cs_source/
/// animation.md 9, 0.2 s for the players' sequences), its weight
/// 3u² − 2u³ of what remains of it, so a client works both out.
#[derive(Clone, Copy, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct NetFade {
    pub cycle: NetCycle,
}

impl NetFade {
    /// Its weight at server tick `t` (ticks `dt` s long) with fade time
    /// `fade` s.
    fn weight(&self, t: f64, dt: f32, fade: f32) -> f32 {
        if fade <= 0.0 {
            return 0.0;
        }
        let u = (1.0 - (t - self.cycle.tick_near(t)) as f32 * dt / fade).clamp(0.0, 1.0);
        3.0 * u * u - 2.0 * u * u * u
    }
}

/// The fade time of the `k`th fading sequence of `cycles` (`NetFade`).
fn fade_of(set: &crate::map::anim::AnimSet, cycles: &AnimNet, k: usize) -> f32 {
    let newer = if k == 0 { cycles.main.map(|m| m.sequence) } else { Some(cycles.fading[k - 1].cycle.sequence) };
    let (Some(old), Some(new)) = (set.sequences.get(cycles.fading[k].cycle.sequence as usize), newer.and_then(|n| set.sequences.get(n as usize))) else {
        return 0.0;
    };
    old.fade_out.min(new.fade_in)
}

/// The aim part of a server's pose (`AnimNet`): the pose parameters
/// (encoded, in `PARAM_STEPS`ths of their range) and the body's yaw (in
/// `YAW_STEPS`ths of a turn); each two bytes as sent (postcard's
/// variable-length integers up to 16383).
#[derive(Clone, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct AimNet {
    pub params: Vec<u16>,
    pub yaw: u16,
}

/// A pose parameter's steps over its range: CS:S's `body_pitch` and
/// `body_yaw` and HL2:DM's `aim_pitch` (180 degrees) in 0.044 degree
/// steps, `move_yaw` (360) in 0.09: a head 0.6 m up the spine moves 0.5
/// mm a step, so a slow turn shows no stepping and the hitboxes no
/// offset worth a hit.
pub const PARAM_STEPS: f32 = 4095.0;
/// A layer's weight steps (1.6 %: a blend's step moves no bone visibly;
/// CS:S's running upper body, its weight following the speed, isn't
/// sent again for a step's jitter).
pub const WEIGHT_STEPS: f32 = 63.0;
/// The aim is sent again when a pose parameter moved this many steps
/// (`PARAM_STEPS`: 0.35 degrees of a 180 degree range) or the yaw
/// `YAW_SEND_STEPS` (0.24 degrees) since the last value: a walking
/// body's speed jitter in CS:S's `move_x`/`move_y` isn't sent at all. A
/// client draws the aim between the values it receives (no holds in
/// between), so the motion is a ramp, never a step.
pub const PARAM_SEND_STEPS: u16 = 8;
pub const YAW_SEND_STEPS: u16 = 11;
/// A client ramps from one aim value to the next over at most this many
/// ticks before the next one's tick (a change after a still spell isn't
/// spread over the whole spell).
pub const AIM_RAMP_TICKS: f64 = 6.0;
/// The body's yaw steps over a turn: 0.022 degrees.
pub const YAW_STEPS: f32 = 16384.0;

impl AnimNet {
    /// The cycles of pose `a` (a server's, at the end of tick `tick`, at
    /// time `now`, ticks `dt` s long) as `prev` (what was sent last) would
    /// carry them on: `prev`'s where they still say the same, `prev` itself
    /// when all that changed is a fade that ended.
    pub fn of(a: &Animator, tick: u64, now: f64, dt: f32, prev: Option<&AnimNet>) -> AnimNet {
        let set = &*a.set;
        let cycle = |sequence: usize, cycle: f32, rate: f32| NetCycle::new(sequence, tick, cycle, dt * set.cycle_rate(sequence, &a.params) * rate);
        let main = a.main.map(|s| cycle(s, a.cycle, a.playback));
        let layers: Vec<NetLayer> = a
            .layers
            .iter()
            .flatten()
            .map(|l| NetLayer {
                sequence: l.sequence.min(u16::MAX as usize) as u16,
                cycle: (a.main.is_none() || l.cycle != a.cycle).then(|| cycle(l.sequence, l.cycle, 1.0)),
                weight: (l.weight.clamp(0.0, 1.0) * WEIGHT_STEPS).round() as u8,
            })
            .collect();
        let fading = a
            .fading
            .iter()
            .map(|f| {
                // Its cycle as it was on the tick it was replaced on.
                let ago = ((now - f.stopped) / dt.max(1e-6) as f64).round().max(0.0);
                let per_tick = dt * set.cycle_rate(f.sequence, &a.params) * f.playback;
                let looping = set.sequences.get(f.sequence).is_some_and(|q| q.looping);
                let then = f.cycle - ago as f32 * per_tick;
                let then = if looping { then.rem_euclid(1.0) } else { then.max(0.0) };
                NetFade {
                    cycle: NetCycle::new(f.sequence, (tick as f64 - ago).max(0.0) as u64, then, per_tick),
                }
            })
            .collect();
        let mut next = AnimNet { main, layers, fading };
        if let Some(prev) = prev {
            // Cycles still on the course sent keep their old value.
            let keep = |old: &NetCycle, new: &NetCycle| {
                let looping = set.sequences.get(new.sequence as usize).is_some_and(|q| q.looping);
                // The quantized value said again, against the exact one.
                old.says(new.sequence, new.at(tick as f64), tick, looping)
            };
            if let (Some(old), Some(new)) = (&prev.main, &next.main)
                && keep(old, new)
            {
                next.main = Some(*old);
            }
            if prev.layers.len() == next.layers.len() {
                for (old, new) in prev.layers.iter().zip(&mut next.layers) {
                    if let (Some(o), Some(n)) = (&old.cycle, &new.cycle)
                        && keep(o, n)
                    {
                        new.cycle = Some(*o);
                    }
                }
            }
            for (old, new) in prev.fading.iter().zip(&mut next.fading) {
                if old.cycle.tick == new.cycle.tick && keep(&old.cycle, &new.cycle) {
                    new.cycle = old.cycle;
                }
            }
            // A fade that ended on the server ends on its clients too (its
            // weight is 0 from the same tick): not sent again for it.
            let mut ended = prev.clone();
            ended.fading.truncate(next.fading.len());
            if prev.fading.len() > next.fading.len() && ended == next && prev.fading[next.fading.len()..].iter().enumerate().all(|(k, f)| {
                f.weight(tick as f64, dt, fade_of(set, prev, next.fading.len() + k)) <= 0.0
            }) {
                return prev.clone();
            }
        }
        next
    }
}

impl AimNet {
    /// The aim of pose `a`.
    pub fn of(a: &Animator) -> AimNet {
        let turn = std::f32::consts::TAU;
        AimNet {
            params: a.params.iter().map(|p| (p.clamp(0.0, 1.0) * PARAM_STEPS).round() as u16).collect(),
            yaw: ((a.yaw.unwrap_or(0.0).rem_euclid(turn) / turn * YAW_STEPS).round() as u32 % YAW_STEPS as u32) as u16,
        }
    }

    /// Whether `next` is far enough from this to be sent
    /// (`PARAM_SEND_STEPS`, `YAW_SEND_STEPS`).
    pub fn moved(&self, next: &AimNet) -> bool {
        let yaw = (next.yaw as i32 - self.yaw as i32).rem_euclid(YAW_STEPS as i32);
        self.params.len() != next.params.len()
            || self.params.iter().zip(&next.params).any(|(a, b)| a.abs_diff(*b) >= PARAM_SEND_STEPS)
            || yaw.min(YAW_STEPS as i32 - yaw) >= YAW_SEND_STEPS as i32
    }

    fn yaw(&self) -> f32 {
        self.yaw as f32 / YAW_STEPS * std::f32::consts::TAU
    }
}

/// Pose `a` as the server had it at render tick `t` (fractional; ticks
/// `dt` s long), every frame: the cycles from `cycles` (the value at or
/// before `t`) carried on to `t` from their ticks, the fades' weights at
/// `t`, the layers' weights and the pose parameters and yaw from them and
/// `aim`, `f` of the way to the next value when there is one (`next`, in
/// each). Nothing changes for a sequence `a`'s set lacks (another model).
pub fn pose(
    (cycles, next_cycles): (&AnimNet, Option<(&AnimNet, f32)>),
    aim: Option<(&AimNet, Option<(&AimNet, f32)>)>,
    t: f64,
    dt: f32,
    a: &mut Animator,
) {
    let set = a.set.clone();
    let Some(main) = cycles.main.filter(|m| (m.sequence as usize) < set.sequences.len()) else { return };
    let s = main.sequence as usize;
    a.main = Some(s);
    a.cycle = crate::map::anim::wrap_cycle(main.at(t), set.sequences[s].looping);
    a.playback = 1.0;
    a.fading = (0..cycles.fading.len())
        .filter_map(|k| {
            let f = &cycles.fading[k];
            let seq = set.sequences.get(f.cycle.sequence as usize)?;
            let fade = fade_of(&set, cycles, k);
            let weight = f.weight(t, dt, fade);
            (weight > 0.0).then(|| crate::map::anim::Fading {
                sequence: f.cycle.sequence as usize,
                cycle: crate::map::anim::wrap_cycle(f.cycle.at(t), seq.looping),
                playback: 1.0,
                stopped: 0.0,
                fade,
                held: Some(weight),
            })
        })
        .collect();
    let main_cycle = a.cycle;
    a.layers = cycles
        .layers
        .iter()
        .enumerate()
        .filter(|(_, l)| (l.sequence as usize) < set.sequences.len())
        .map(|(k, l)| {
            let w = l.weight as f32 / WEIGHT_STEPS;
            // A weight blending (CS:S's upper body between two layers)
            // between the values around `t`.
            let weight = match next_cycles.and_then(|(n, f)| n.layers.get(k).filter(|m| m.sequence == l.sequence).map(|m| (m, f))) {
                Some((m, f)) => w + (m.weight as f32 / WEIGHT_STEPS - w) * f,
                None => w,
            };
            Some(crate::map::anim::Layer {
                sequence: l.sequence as usize,
                cycle: l.cycle.map_or(main_cycle, |c| c.at(t)),
                weight,
            })
        })
        .collect();
    let Some((aim, next)) = aim else { return };
    let decode = |v: u16| v as f32 / PARAM_STEPS;
    if aim.params.len() == a.params.len() {
        for (i, (v, p)) in a.params.iter_mut().zip(&aim.params).enumerate() {
            let x = decode(*p);
            *v = match next.filter(|(n, _)| n.params.len() == aim.params.len()) {
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
        Some((n, f)) => aim.yaw() + normalize((n.yaw() - aim.yaw()).to_degrees()).to_radians() * f,
        None => aim.yaw(),
    });
}

/// Game `R`'s bodies' cycles as clients are told them (`AnimNet`).
pub type NetPose<R> = Net<AnimNet, R>;
/// Game `R`'s bodies' aim as clients are told it (`AimNet`).
pub type NetAim<R> = Net<AimNet, R>;

/// A part of a server's pose (`T`) of game `R`'s body, replicated.
#[derive(Component, serde::Serialize, serde::Deserialize)]
#[serde(bound(serialize = "T: serde::Serialize", deserialize = "T: serde::Deserialize<'de>"))]
pub struct Net<T: Send + Sync + 'static, R: Send + Sync + 'static>(pub T, #[serde(skip)] PhantomData<fn() -> R>);

impl<T: Clone + Send + Sync + 'static, R: Send + Sync + 'static> Clone for Net<T, R> {
    fn clone(&self) -> Self {
        Self(self.0.clone(), PhantomData)
    }
}

impl<T: PartialEq + Send + Sync + 'static, R: Send + Sync + 'static> PartialEq for Net<T, R> {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}

impl<T: std::fmt::Debug + Send + Sync + 'static, R: Send + Sync + 'static> std::fmt::Debug for Net<T, R> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

impl<T: Send + Sync + 'static, R: Send + Sync + 'static> Net<T, R> {
    pub fn new(value: T) -> Self {
        Self(value, PhantomData)
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
    replicate::<AnimNet, R>(app, true);
    // The aim's values only (`AIM_RAMP_TICKS`): no copies at the ticks it
    // was heard unchanged.
    replicate::<AimNet, R>(app, false);
}

fn replicate<T, R: PlayerAnimRules>(app: &mut App, held: bool)
where
    T: serde::Serialize + serde::de::DeserializeOwned + Clone + Send + Sync + 'static,
{
    use bevy_replicon::prelude::*;
    use chimera_net::net::ordered::OrderedAppExt;
    app.replicate::<Net<T, R>>()
        .set_receive_fns::<Net<T, R>>(write_snapshot::<Net<T, R>>, remove_snapshots::<Net<T, R>>)
        .ordered_with::<Net<T, R>>(write_snapshot::<Net<T, R>>, remove_snapshots::<Net<T, R>>);
    if held {
        app.add_systems(PreUpdate, hold_heard::<Net<T, R>>.after(HearTicks).run_if(resource_equals(NetRole::Client)));
    }
}

/// Each of game `R`'s bodies' tick pose (`SimAnimator`) as `NetPose<R>`
/// and `NetAim<R>`, each changed only when it says something new; gone
/// from a body no longer the game's.
#[allow(clippy::type_complexity)]
fn send<R: PlayerAnimRules>(
    clock: Res<SimClock>,
    mut bodies: Query<(Entity, Option<&SimAnimator>, Option<&mut NetPose<R>>, Option<&mut NetAim<R>>, Has<SimAnimState<R>>)>,
    mut commands: Commands,
) {
    let dt = clock.dt();
    for (e, sim, pose, aim, ours) in &mut bodies {
        let Some(sim) = sim.filter(|_| ours) else {
            if pose.is_some() || aim.is_some() {
                commands.entity(e).remove::<(NetPose<R>, NetAim<R>)>();
            }
            continue;
        };
        let next = AnimNet::of(&sim.0, clock.tick, clock.now, dt, pose.as_deref().map(|n| &n.0));
        let next_aim = AimNet::of(&sim.0);
        match (pose, aim) {
            (Some(mut p), Some(mut a)) => {
                if p.0 != next {
                    p.0 = next;
                }
                if a.0.moved(&next_aim) {
                    a.0 = next_aim;
                }
            }
            _ => {
                commands.entity(e).insert((NetPose::<R>::new(next), NetAim::<R>::new(next_aim)));
            }
        }
    }
}

type Followed<'w, 's, R> = Query<
    'w,
    's,
    (
        &'static mut Animator,
        (&'static NetPose<R>, Option<&'static Snapshots<NetPose<R>>>),
        (Option<&'static NetAim<R>>, Option<&'static Snapshots<NetAim<R>>>),
        Option<&'static Health>,
    ),
    Without<LocalPlayer>,
>;

/// Another player's body of game `R` on a client, drawn: posed as the
/// server had it at the render tick (the values around it).
fn follow<R: PlayerAnimRules>(clock: Option<Res<InterpClock>>, fixed: Res<Time<Fixed>>, mut bodies: Followed<R>) {
    pose_followed(clock.as_deref(), fixed.timestep().as_secs_f32(), &mut bodies, false);
}

/// The same before a client's ticks, for the hitboxes: the hands' IK
/// goals (the drawing's) left out.
fn follow_tick<R: PlayerAnimRules>(clock: Option<Res<InterpClock>>, fixed: Res<Time<Fixed>>, mut bodies: Followed<R>) {
    pose_followed(clock.as_deref(), fixed.timestep().as_secs_f32(), &mut bodies, true);
}

fn pose_followed<R: PlayerAnimRules>(clock: Option<&InterpClock>, dt: f32, bodies: &mut Followed<R>, no_ik: bool) {
    let Some(clock) = clock.filter(|c| c.running) else { return };
    let t = clock.render_tick;
    for (mut a, (pose_newest, pose_kept), (aim_newest, aim_kept), health) in bodies.iter_mut() {
        if health.is_some_and(|h| h.current <= 0.0) {
            continue;
        }
        // The cycles: the value at or before the render tick, carried on
        // (the next one for the layers' weights).
        let cycles = match pose_kept.map(|k| k.around(t)) {
            Some(Around::Between((_, x), (_, y), f)) => (&x.0, Some((&y.0, f))),
            Some(Around::After((_, x))) => (&x.0, None),
            _ => (&pose_newest.0, None),
        };
        // The aim: between the values around it, ramping into the next
        // over its last `AIM_RAMP_TICKS`.
        let aim = aim_newest.map(|newest| match aim_kept.map(|k| k.around(t)) {
            Some(Around::Between((ta, x), (tb, y), _)) => {
                let start = (*ta as f64).max(*tb as f64 - AIM_RAMP_TICKS);
                let f = ((t - start) / (*tb as f64 - start).max(1e-6)).clamp(0.0, 1.0) as f32;
                (&x.0, Some((&y.0, f)))
            }
            Some(Around::After((_, x))) => (&x.0, None),
            _ => (&newest.0, None),
        });
        pose(cycles, aim, t, dt, &mut a);
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
            fade_in: 0.2,
            fade_out: 0.2,
            ..default()
        };
        Arc::new(AnimSet {
            animations: vec![anim(31), anim(16), anim(21)],
            sequences: vec![seq(0, true), seq(1, false), seq(2, true)],
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
            let next = AnimNet::of(&server, tick, tick as f64 * dt as f64, dt, sent.as_ref());
            if sent.as_ref() != Some(&next) {
                values += 1;
                sent = Some(next);
            }
            let mut client = Animator::new(set());
            pose((sent.as_ref().unwrap(), None), None, tick as f64, dt, &mut client);
            assert!((client.cycle - server.cycle).abs() < 2.0 * CYCLE_EPSILON, "tick {tick}: {} {}", client.cycle, server.cycle);
        }
        assert_eq!(values, 1, "a steady cycle is one value");
        // A gesture plays over it: a value with it, the main cycle kept.
        server.layers = vec![Some(Layer {
            sequence: 1,
            cycle: 0.0,
            weight: 1.0,
        })];
        let with = AnimNet::of(&server, 199, 199.0 * dt as f64, dt, sent.as_ref());
        assert_eq!(with.main, sent.as_ref().unwrap().main);
        assert_eq!(with.layers.len(), 1);
        assert!(with.layers[0].cycle.is_some(), "its own cycle");
        // A layer at the main sequence's cycle (CS:S's upper body) sends
        // none of its own: steady with it.
        server.layers = vec![Some(Layer {
            sequence: 1,
            cycle: server.cycle,
            weight: 0.5,
        })];
        let locked = AnimNet::of(&server, 199, 199.0 * dt as f64, dt, sent.as_ref());
        assert_eq!(locked.layers[0].cycle, None);
        let mut client = Animator::new(set());
        pose((&locked, None), None, 230.0, dt, &mut client);
        assert_eq!(client.layers[0].unwrap().cycle, client.cycle);
        // The yaw parameter the short way round, the yaw likewise.
        let a = AimNet {
            params: vec![(0.95 * PARAM_STEPS) as u16],
            yaw: (3.0 / std::f32::consts::TAU * YAW_STEPS) as u16,
        };
        let b = AimNet {
            params: vec![(0.05 * PARAM_STEPS) as u16],
            yaw: ((std::f32::consts::TAU - 3.0) / std::f32::consts::TAU * YAW_STEPS) as u16,
        };
        let mut client = Animator::new(set());
        pose((&with, None), Some((&a, Some((&b, 0.5)))), 200.0, dt, &mut client);
        assert!(client.params[0] < 0.01 || client.params[0] > 0.99, "{}", client.params[0]);
        let yaw = client.yaw.unwrap().rem_euclid(std::f32::consts::TAU);
        assert!((yaw - std::f32::consts::PI).abs() < 0.01, "{yaw}");
    }

    /// A followed body's main sequence change is the server's fade: the
    /// outgoing sequence's weight and cycle on the client at every tick of
    /// it as on the server (no snap), sent once with the change and not
    /// again when it ends.
    #[test]
    fn a_sequence_change_is_followed_as_a_fade() {
        let dt = 1.0 / 64.0;
        let mut server = Animator::new(set());
        server.play(0, 0.0);
        let mut sent: Option<AnimNet> = None;
        let (mut values, mut steady, mut changed) = (0, 0, 0);
        let mut faded = 0;
        for tick in 0..100u64 {
            let now = tick as f64 * dt as f64;
            if tick == 40 {
                server.play(2, now);
            }
            server.advance(dt, now);
            let next = AnimNet::of(&server, tick, now, dt, sent.as_ref());
            if sent.as_ref() != Some(&next) {
                values += 1;
                let size = |v: &AnimNet| postcard::to_allocvec(v).unwrap().len();
                if tick == 40 {
                    (steady, changed) = (size(sent.as_ref().unwrap()), size(&next));
                }
                sent = Some(next);
            }
            let mut client = Animator::new(set());
            pose((sent.as_ref().unwrap(), None), None, tick as f64, dt, &mut client);
            assert_eq!(client.main, server.main);
            assert_eq!(client.fading.len(), server.fading.len(), "tick {tick}");
            for (c, s) in client.fading.iter().zip(&server.fading) {
                assert_eq!(c.sequence, s.sequence);
                assert!((c.weight(0.0) - s.weight(now)).abs() < 1e-4, "tick {tick}: {} {}", c.weight(0.0), s.weight(now));
                assert!((c.cycle - s.cycle).abs() < 2.0 * CYCLE_EPSILON, "tick {tick}: {} {}", c.cycle, s.cycle);
                if s.weight(now) > 0.05 && s.weight(now) < 0.95 {
                    faded += 1;
                }
            }
        }
        assert!(faded >= 8, "a fade over the 0.2 s, not a snap ({faded} ticks part way)");
        assert_eq!(values, 2, "the first value and the change, none when the fade ends");
        eprintln!("a sequence change: {steady} B steady, {changed} B with the change and its fade (+{} B)", changed - steady);
    }

    /// A cycle's 16-bit tick is read as the one nearest the reading tick,
    /// across the wrap; one `MAX_AGE` old is sent again.
    #[test]
    fn a_cycles_tick_is_the_nearest() {
        let c = NetCycle::new(0, 65_530, 0.25, 0.01);
        assert_eq!(c.tick_near(65_540.5), 65_530.0);
        assert!((c.at(65_540.0) - 0.35).abs() < 1e-3, "{}", c.at(65_540.0));
        let d = NetCycle::new(0, 131_080, 0.5, 0.0);
        assert_eq!(d.tick_near(131_070.0), 131_080.0);
        assert!(d.says(0, 0.5, 131_080 + MAX_AGE - 1, true));
        assert!(!d.says(0, 0.5, 131_080 + MAX_AGE, true));
    }
}
