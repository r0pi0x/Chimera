//! Halo's players coming into the game and back (specs/halo_ce/
//! multiplayer_rules.md 4.1 to 4.6), for Halo's own app: the rules of its
//! free-for-all Slayer (`HaloRules`, in force only where an app inserts
//! it: the party's apps have their own rules, and the scenario tests
//! place their own guns).
//!
//! - **Who and when** (`rules`, once per Halo tick): a player, seen the
//!   first time, is given what a new life holds; when it dies
//!   (`core::Died`) the respawn timer is set as 4.2 says: `P + R` (the
//!   dead player's own penalty, 0 here, plus the variant's respawn time),
//!   the growth rule on `P`, `+ S` (the suicide penalty) unless an enemy
//!   killed it, at least 90 ticks. The dead player is held (`Dead`, not
//!   solid) and the timer counts one a tick; at 0 it spawns, but after
//!   game time 3 only on the tick whose number is its slot's mod 32 (the
//!   stagger), and when no start has a rating above 0 it tries again on
//!   the next staggered tick.
//! - **Where** (`rate`, 4.4): every start of the map (a free-for-all
//!   game: all of them, teams are ignored) is rated 1, 0 within 0.25 WU of
//!   a living player, a tenth within 1 WU, 0 within 2 WU of an enemy
//!   and (d - 2) / 3 up to 5 WU; each rating is multiplied by the square
//!   root of a fresh draw and the largest product, above 0, is the place
//!   (the earlier of equal ones), the facing the start's.
//! - **What it holds** (4.6): the assault rifle in hand and the pistol,
//!   with their ammunition, and 4 frag grenades (the grenades are
//!   `weapons::grenades`', handed out on the new life by `HaloHands`).
//!   The shield and body are full by the new `Vitality`.
//!
//! The draws are Halo's recurrence (weapons.md: seed s' = 1664525 s +
//! 1013904223, the real draw (s' >> 16) / 65535), seeded by the game's
//! start, so a match replays.
//!
//! Known shortcuts: no teams (so no capture the flag starts or friends
//! bonus), no vehicles, no lives or odd man out, the variant is Slayer's
//! (respawn time 0, growth 0, suicide penalty 10 s: `HaloRules::default`);
//! "a fall or suicide soon after an enemy's damage is credited to that
//! enemy" is not tracked (a death with no attacker pays the suicide
//! penalty); the weapon set and starting equipment of the scenario are not
//! read; the "countdown for respawn" and "player respawn" sounds are not
//! played; a start the unit does not fit in is not detected (4.5).

use std::collections::{BTreeMap, BTreeSet};

use avian3d::prelude::ColliderDisabled;
use bevy::prelude::*;
use chimera_sim::weapon::{Inventory, equip::strip, give};

use super::{
    METERS_PER_UNIT,
    ticks::halo_ticks,
    weapons::{ASSAULT_RIFLE, HaloHands, PISTOL},
};
use crate::core::{Connecting, Dead, Died, Health, Intent, LocalPlayer, RespawnAt, RespawnDue, SimClock, SimSet, SpawnPoint, Spectating};

/// The least wait of any death (ticks, `MIN_RESPAWN_TICKS`).
pub const MIN_RESPAWN_TICKS: i32 = 90;

/// The game variant's respawn numbers (ticks): its respawn time, the growth
/// added to a player's penalty per death and the suicide penalty
/// (multiplayer_rules.md 4.2). Inserting it puts the rules in force.
#[derive(Resource, Clone, Copy, Debug, PartialEq, Eq)]
pub struct HaloRules {
    pub respawn_time: i32,
    pub growth: i32,
    pub suicide: i32,
}

impl Default for HaloRules {
    /// Slayer's: instant, no growth, a 10 s suicide penalty.
    fn default() -> Self {
        Self { respawn_time: 0, growth: 0, suicide: 300 }
    }
}

/// The players' waits and the rules' state.
#[derive(Resource)]
pub struct Respawns {
    /// Halo ticks since the game began (the game time of 4.1).
    pub time: u32,
    /// Each player's slot, by first sight.
    slots: BTreeMap<Entity, u32>,
    /// Ticks left before each dead player may spawn.
    pub waiting: BTreeMap<Entity, i32>,
    /// Each player's respawn penalty `P`.
    penalty: BTreeMap<Entity, i32>,
    seen: BTreeSet<Entity>,
    seed: u32,
}

impl Default for Respawns {
    fn default() -> Self {
        Self { time: 0, slots: BTreeMap::new(), waiting: BTreeMap::new(), penalty: BTreeMap::new(), seen: BTreeSet::new(), seed: 0x48414C4F }
    }
}

impl Respawns {
    /// A real draw: Halo's recurrence.
    pub fn draw(&mut self) -> f32 {
        self.seed = self.seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        (self.seed >> 16) as f32 / 65535.0
    }
}

/// The respawn timer of a death (4.2): `p` the dead player's penalty;
/// `kill`: an enemy player killed it. Returns the wait and the new penalty.
pub fn timer(rules: &HaloRules, p: i32, kill: bool) -> (i32, i32) {
    let mut wait = p + rules.respawn_time;
    let mut penalty = p;
    if rules.growth > 0 {
        penalty = (p + rules.growth).min(5 * rules.growth);
    }
    if !kill {
        wait += rules.suicide;
    }
    (wait.max(MIN_RESPAWN_TICKS), penalty)
}

/// A start's rating for a player (4.4 steps 1 and 2) among `others`, the
/// distances (WU) to the other living players, all enemies.
pub fn rate(others: &[f32]) -> f32 {
    let mut rating = 1.0;
    for &d in others {
        if d < 0.25 {
            return 0.0;
        }
        if d < 1.0 {
            rating *= 0.1;
        }
        if d < 2.0 {
            return 0.0;
        }
        if d <= 5.0 {
            rating *= (d - 2.0) / 3.0;
        }
    }
    rating
}

/// The start a player takes (4.4): each of `ratings` multiplied by the
/// square root of a draw, the largest product above 0 (the earlier of
/// equals); None when there is none.
pub fn choose(ratings: &[f32], draw: &mut impl FnMut() -> f32) -> Option<usize> {
    let mut best: Option<(usize, f32)> = None;
    for (i, r) in ratings.iter().enumerate() {
        let product = r * draw().sqrt();
        if product > 0.0 && best.is_none_or(|(_, b)| product > b) {
            best = Some((i, product));
        }
    }
    best.map(|(i, _)| i)
}

/// Whether a player of `slot` may spawn at game time `time` (the stagger of
/// 4.1: after game time 3 only on a tick equal to its slot mod 32).
pub fn staggered(time: u32, slot: u32) -> bool {
    time <= 3 || time % 32 == slot % 32
}

/// The starting loadout: the assault rifle in hand, then the pistol.
fn loadout(world: &mut World, e: Entity) {
    strip(world, e);
    let rifle = give(world, e, ASSAULT_RIFLE);
    give(world, e, PISTOL);
    // The rifle is the one in hand (4.6), the pistol holstered.
    if let (Some(rifle), Some(mut inv)) = (rifle, world.get_mut::<Inventory>(e)) {
        inv.active = Some(rifle);
        inv.wanted = None;
    }
}

type Players<'w, 's> = Query<
    'w,
    's,
    (Entity, &'static Inventory, Has<Dead>, Has<LocalPlayer>),
    (With<HaloHands>, With<Intent>, Without<Connecting>, Without<Spectating>),
>;

/// The rules' tick (module doc): what it decides, it queues.
#[allow(clippy::too_many_arguments)]
pub fn rules(
    rules: Res<HaloRules>,
    clock: Res<SimClock>,
    mut state: ResMut<Respawns>,
    mut died: MessageReader<Died>,
    mut due: ResMut<RespawnDue>,
    players: Players,
    living: Query<(Entity, &Transform, &Health), (With<Intent>, Without<Dead>, Without<Connecting>, Without<Spectating>)>,
    starts: Query<(&Transform, &SpawnPoint)>,
    mut commands: Commands,
) {
    let n = halo_ticks(&clock);
    let state = &mut *state;
    // A player seen the first time: its slot and a new life's loadout.
    for (e, inv, dead, _) in &players {
        if state.seen.insert(e) {
            let slot = state.slots.len() as u32;
            state.slots.insert(e, slot);
            if inv.weapons.is_empty() && !dead {
                commands.queue(move |w: &mut World| loadout(w, e));
            }
        }
    }
    // Deaths.
    let now = clock.now;
    for d in died.read() {
        let victim = d.entity;
        let Ok((_, _, _, local)) = players.get(victim) else { continue };
        if state.waiting.contains_key(&victim) {
            continue;
        }
        let attacker = d.attacker;
        let kill = attacker.is_some_and(|a| a != victim && state.slots.contains_key(&a));
        let p = state.penalty.get(&victim).copied().unwrap_or(0);
        let (wait, penalty) = timer(&rules, p, kill);
        state.penalty.insert(victim, penalty);
        if kill && rules.growth > 0 && let Some(a) = attacker {
            let k = state.penalty.entry(a).or_insert(0);
            *k = (*k - rules.growth).max(0);
        }
        state.waiting.insert(victim, wait);
        commands.entity(victim).insert((Dead { since: now }, ColliderDisabled));
        if local {
            due.0 = Some(RespawnAt { at: now + f64::from(wait) / 30.0, delay: f64::from(wait) / 30.0 });
        }
    }
    // Ticks of waiting.
    let spots: Vec<Transform> = starts.iter().map(|(t, _)| *t).collect();
    for _ in 0..n {
        state.time += 1;
        let waiting: Vec<Entity> = state.waiting.keys().copied().collect();
        for e in waiting {
            let Ok((_, _, _, local)) = players.get(e) else {
                state.waiting.remove(&e);
                continue;
            };
            let left = state.waiting.get_mut(&e).expect("listed");
            if *left > 0 {
                *left -= 1;
            }
            if *left > 0 {
                continue;
            }
            let slot = state.slots.get(&e).copied().unwrap_or(0);
            if !staggered(state.time, slot) {
                continue;
            }
            // Rate every start against the living.
            let others: Vec<Vec3> = living.iter().filter(|(o, _, h)| *o != e && h.current > 0.0).map(|(_, t, _)| t.translation).collect();
            let ratings: Vec<f32> = spots
                .iter()
                .map(|s| rate(&others.iter().map(|p| p.distance(s.translation) / METERS_PER_UNIT).collect::<Vec<_>>()))
                .collect();
            let pick = {
                let mut draw = || state.draw();
                choose(&ratings, &mut draw)
            };
            let Some(i) = pick else {
                // No usable start: the timer is 1, the test repeats.
                state.waiting.insert(e, 1);
                continue;
            };
            state.waiting.remove(&e);
            let at = spots[i];
            commands.queue(move |w: &mut World| {
                chimera_sim::spawning::revive(w, e, Some(at));
                loadout(w, e);
            });
            if local {
                due.0 = None;
            }
        }
    }
}

/// Halo's respawn rules: `rules` each tick, where `HaloRules` is inserted.
pub struct HaloRespawnPlugin;

impl Plugin for HaloRespawnPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Respawns>()
            .add_systems(FixedUpdate, rules.in_set(SimSet::Rules).run_if(crate::core::authoritative).run_if(resource_exists::<HaloRules>));
    }
}
