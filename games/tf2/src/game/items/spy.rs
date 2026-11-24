//! The Spy's kit (specs/tf2/batch3_mechanics.md 3-5): the watches' cloak
//! (the hook `tf2:cloak`, `CloakParams`, the weapon part `Tf2Cloak`), the
//! disguise (`Tf2Spy` on the player, `disguise`) and the knives' rules
//! (their `tf2:item` numbers, `super::mods::ItemParams`). Others see the
//! result through Chimera's `guise::Guise` (written here, the server's,
//! sent to every client) and its fake death (`core::Feigned`).
//!
//! - **Cloak** (`think`, predicted, before the weapon switch): a watch is
//!   carried, not drawn (drawing it puts the weapon before back). Attack2
//!   with any weapon cloaks: the Spy fades to invisible over `CLOAK_IN`
//!   (1.0 s); attack2 again, not before `CLOAK_LOCK` (0.5 s) cloaked,
//!   decloaks over `CLOAK_OUT` (2.0 s); he cannot attack from the cloak
//!   until he is fully visible. The meter (0 to 100) drains while cloaked
//!   (`drain` a second, × the carried items' `cloak_drain`: Your Eternal
//!   Reward's 1.33) and refills while not (`regen`); at 0 he decloaks.
//!   The Cloak and Dagger (`kind = "still"`) refills `regen` while cloaked
//!   and standing still and drains `drain` × (v / v_max)² moving (fitted,
//!   §3.2). Damage taken while cloaked × `CLOAK_CUT` (0.8).
//! - **Dead Ringer** (`kind = "feign"`, `taken`, the server's): attack2
//!   with a full meter arms it (again: disarms). The next damage taken
//!   feigns a death: the hit × `FEIGN_FIRST` (0.25), a death for the kill
//!   feed and a ragdoll (`core::Feigned`), the Spy invisible at once, 50
//!   off the meter, `FEIGN_TIME` (3 s) of the speed boost and of
//!   afterburn immunity; hits while the feign lasts are cut by 0.65 −
//!   t/16 (fitted), then the cloak's 0.8. Its meter drains `drain` (7.1)
//!   cloaked, refills `regen` (5).
//! - **Disguise** (`disguise`, the console's `tf2:disguise <class>
//!   [team]`, a client's `DisguiseRequest`): 2.0 s from undisguised, 0.5
//!   s when disguised (`DISGUISE_TIME`, `REDISGUISE_TIME`); then his
//!   enemies see the class's body in the team's colours, its stock
//!   primary (`classes::Class::primary`), a real player of that team's
//!   name and his health (at most the class's max). He moves at the
//!   slower of his and the class's speed (`speed_factor`, predicted;
//!   cloaked, his own). Firing ends it (`DISGUISE_WEAR_OFF`, 0.5 s of
//!   wearing off); damage and cloaking don't.
//! - **Knives** (§4): Your Eternal Reward's backstab kill disguises as the
//!   victim `YER_DELAY` (0.2 s) later (his class, team, name, full health)
//!   and a normal disguise needs a full cloak meter and empties it; the
//!   Big Earner's kill gives 30 meter and 3 s of the speed boost; the
//!   Spy-cicle takes the first fire hit in full, then melts (put away,
//!   back `melts` s later), its holder fire-immune 1 s and
//!   afterburn-immune 10 s.
//! - **The Enforcer** (batch2b_mechanics.md 16): a shot fired while
//!   disguised × `disguise_bonus` (1.2; the shot that ends the disguise
//!   too).
//!
//! The watch's meter shows on TF2's HUD as an item meter (`CLOAK`).
//!
//! Known shortcuts: the cloak has no look of its own (the spec gives
//! none): the body fades (Chimera's `BodyOpacity`), nothing else
//! (sounds, the shimmer); a disguise with no player of its team
//! to copy shows no name and the class's max health; cloak from ammo and
//! health packs (classes.md) and the watches' consume factors aren't
//! applied; the Spy-cicle's melted knife is put away on the server's tick
//! (a client predicting a draw of it is corrected); the disguise's
//! weapon switch (the disguise's other slots) isn't modelled.

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use chimera_core::guise::{Guise, Pose};
use chimera_sim::{
    status::{Condition, Conditions, SpeedBoost},
    weapon::{Inventory, NetPart, WeaponState},
};

use super::mods::Tf2Item;
use crate::core::{Damage, DamageKind, Died, Health, Intent, SimClock, Team, Velocity};
use crate::game::classes::{Classes, Tf2Class};
use crate::game::weapons::{damage::{Tf2Gun, round_points, U}, flame::Burning, melee::BACKSTAB};

/// The hook.
pub const HOOK: &str = "tf2:cloak";
/// §3.1: seconds to invisible, back to visible, and before a decloak.
pub const CLOAK_IN: f32 = 1.0;
pub const CLOAK_OUT: f32 = 2.0;
pub const CLOAK_LOCK: f64 = 0.5;
/// Damage taken while cloaked.
pub const CLOAK_CUT: f32 = 0.8;
/// A full meter.
pub const FULL: f32 = 100.0;
/// §3.3: the feign's cost, its conditions' length, the triggering hit's
/// share, the cut's law (0.65 − t/16) while it lasts.
pub const FEIGN_COST: f32 = 50.0;
pub const FEIGN_TIME: f32 = 3.0;
pub const FEIGN_FIRST: f32 = 0.25;
pub const FEIGN_CUT_A: f32 = 0.65;
pub const FEIGN_CUT_B: f32 = 1.0 / 16.0;
/// §5: the disguise's times.
pub const DISGUISE_TIME: f64 = 2.0;
pub const REDISGUISE_TIME: f64 = 0.5;
pub const DISGUISE_WEAR_OFF: f64 = 0.5;
/// §4: Your Eternal Reward's disguise after the kill.
pub const YER_DELAY: f64 = 0.2;
/// The Spy's class.
pub const SPY: &str = "spy";
/// Chimera conditions (`status::Condition::Game`): no fire damage, no
/// afterburn.
pub const FIRE_IMMUNE: &str = "tf2:fire_immune";
pub const AFTERBURN_IMMUNE: &str = "tf2:afterburn_immune";
/// Slower than this (units/s): standing still (the Cloak and Dagger).
const STILL: f32 = 1.0;

/// Which watch.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CloakKind {
    /// The Invis Watch's.
    #[default]
    Plain,
    /// The Cloak and Dagger's.
    Still,
    /// The Dead Ringer's.
    Feign,
}

/// `tf2:cloak`'s parameters (per second, of 100).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CloakParams {
    #[serde(default)]
    pub kind: CloakKind,
    pub drain: f32,
    pub regen: f32,
}

/// The watch's state (predicted; module doc).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct CloakState {
    /// 0 to 100.
    pub meter: f32,
    /// Cloaked (the cloak's condition: from the press to the decloak's).
    pub cloaked: bool,
    /// The last change (cloak or decloak) and how hidden he was then.
    pub changed: f64,
    pub hidden_then: f32,
    /// The Dead Ringer: armed; its feign's start.
    pub armed: bool,
    pub feign: Option<f64>,
    pub prev_alt: bool,
}

impl Default for CloakState {
    fn default() -> Self {
        Self { meter: FULL, cloaked: false, changed: f64::MIN / 2.0, hidden_then: 0.0, armed: false, feign: None, prev_alt: false }
    }
}

impl CloakState {
    /// How hidden at `now` (0 seen, 1 invisible).
    pub fn hidden(&self, now: f64) -> f32 {
        let t = (now - self.changed).max(0.0) as f32;
        if self.cloaked {
            (self.hidden_then + t / CLOAK_IN).min(1.0)
        } else {
            (self.hidden_then - t / CLOAK_OUT).max(0.0)
        }
    }

    /// When he is (or will be, decloaking) fully visible.
    pub fn visible_at(&self, now: f64) -> f64 {
        if self.cloaked { now + CLOAK_OUT as f64 } else { self.changed + (self.hidden_then * CLOAK_OUT) as f64 }
    }

    fn set(&mut self, cloaked: bool, now: f64) {
        self.hidden_then = self.hidden(now);
        self.cloaked = cloaked;
        self.changed = now;
    }

    /// The feign's seconds so far, while it lasts.
    pub fn feigning(&self, now: f64) -> Option<f32> {
        self.feign.map(|f| (now - f) as f32).filter(|t| *t >= -1e-6 && *t < FEIGN_TIME - 1e-6)
    }
}

/// The weapon part: the watch.
#[derive(Component, Clone, Debug)]
pub struct Tf2Cloak {
    pub params: CloakParams,
    pub state: CloakState,
}

impl NetPart for Tf2Cloak {
    type Net = CloakState;
    fn net(&self) -> CloakState {
        self.state
    }
    fn set_net(&mut self, net: CloakState) {
        self.state = net;
    }
}

impl Tf2Cloak {
    /// The HUD meter: its label and fill.
    pub fn meter(&self) -> (&str, f32) {
        ("CLOAK", (self.state.meter / FULL).clamp(0.0, 1.0))
    }
}

/// One disguise (§5): the class (name), the team, whom it copies.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Disguise {
    pub class: String,
    pub team: u8,
    /// The copied player's name ("": none), health and max (points).
    pub name: String,
    pub health: f32,
    pub max: f32,
}

/// A Spy's disguise state (predicted, sent to his client: his speed
/// follows it), on a player the first time he disguises or a knife
/// melts, kept.
#[derive(Component, Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Tf2Spy {
    /// In effect.
    pub disguise: Option<Disguise>,
    /// Coming: in effect at its time.
    pub pending: Option<(Disguise, f64)>,
    /// When the current disguise (or the one coming) was asked for: an
    /// attack after it ends it.
    pub since: f64,
    /// The disguise wears off until then (after firing).
    pub wear_off: f64,
    /// The Spy-cicle is melted until then.
    pub melted: f64,
}

impl Tf2Spy {
    /// Disguised now (wearing off or not yet: no).
    pub fn disguised(&self) -> bool {
        self.disguise.is_some()
    }
}

/// `tf2:cloak` on a weapon: a full meter.
pub fn attach(e: &mut EntityWorldMut, p: &CloakParams) {
    e.insert(Tf2Cloak { params: p.clone(), state: CloakState::default() });
}

/// The last time weapon `w` attacked (a shot or a swing).
fn last_attack(w: Entity, guns: &Query<&Tf2Gun>, states: &Query<&WeaponState>) -> f64 {
    let shot = guns.get(w).map_or(f64::MIN, |g| g.last_shot);
    let swing = states.get(w).map_or(f64::MIN, |s| s.last_swing.iter().flatten().copied().fold(f64::MIN, f64::max));
    shot.max(swing)
}

/// The cloak and the disguise each tick (module doc), predicted, before
/// the weapon switch.
#[allow(clippy::type_complexity)]
pub fn think(
    mut owners: Query<
        (Entity, &mut Intent, &mut Inventory, Option<&Health>, Option<&Velocity>, Option<&Tf2Class>, Option<&mut Tf2Spy>),
        Without<crate::map::interp::NetDrawn>,
    >,
    mut cloaks: Query<&mut Tf2Cloak>,
    items: Query<&Tf2Item>,
    guns: Query<&Tf2Gun>,
    states: Query<&WeaponState>,
    table: Option<Res<Classes>>,
    clock: Res<SimClock>,
) {
    let (now, dt) = (clock.now, clock.dt());
    for (_, mut intent, mut inv, health, velocity, class, spy) in &mut owners {
        let watch = inv.weapons.iter().copied().find(|w| cloaks.contains(*w));
        if watch.is_none() && spy.is_none() {
            continue;
        }
        let dead = health.is_some_and(|h| h.current <= 0.0);
        // The disguise.
        if let Some(mut spy) = spy {
            let mut s = spy.clone();
            if dead {
                s = Tf2Spy { melted: s.melted, ..default() };
            } else {
                if let Some((d, at)) = s.pending.clone()
                    && now + 1e-6 >= at
                {
                    s.disguise = Some(d);
                    s.pending = None;
                }
                let attacked = inv.weapons.iter().map(|w| last_attack(*w, &guns, &states)).fold(f64::MIN, f64::max);
                if (s.disguise.is_some() || s.pending.is_some()) && attacked > s.since + 1e-6 {
                    s.disguise = None;
                    s.pending = None;
                    s.wear_off = now + DISGUISE_WEAR_OFF;
                }
                // The melted knife stays away.
                if now < s.melted {
                    let melted = |w: Entity| items.get(w).is_ok_and(|i| i.0.melts > 0.0);
                    if inv.wanted.is_some_and(melted) {
                        inv.wanted = None;
                    }
                    if inv.active.is_some_and(melted) && inv.wanted.is_none() {
                        inv.wanted = inv.weapons.iter().copied().find(|w| !melted(*w) && Some(*w) != watch);
                    }
                }
            }
            if *spy != s {
                *spy = s;
            }
        }
        let Some(w) = watch else { continue };
        // A watch isn't drawn: the one before is.
        if inv.wanted == Some(w) {
            inv.wanted = if inv.active.is_some() { None } else { inv.weapons.iter().copied().find(|x| *x != w) };
        }
        if inv.active == Some(w) && inv.wanted.is_none() {
            inv.wanted = inv.last.filter(|l| *l != w && inv.weapons.contains(l)).or_else(|| inv.weapons.iter().copied().find(|x| *x != w));
        }
        let drain_mult: f32 = inv.weapons.iter().filter_map(|x| items.get(*x).ok()).map(|i| i.0.cloak_drain).product();
        let Ok(mut cloak) = cloaks.get_mut(w) else { continue };
        let p = cloak.params.clone();
        let mut s = cloak.state;
        if dead {
            s = CloakState { prev_alt: s.prev_alt, ..default() };
            if cloak.state != s {
                cloak.state = s;
            }
            continue;
        }
        let alt = intent.secondary;
        let press = alt && !s.prev_alt;
        s.prev_alt = alt;
        // The meter.
        if s.cloaked {
            let loss = match p.kind {
                CloakKind::Still => {
                    let v = velocity.map_or(0.0, |v| v.0.with_y(0.0).length() / U);
                    let top = table.as_deref().zip(class).map_or(320.0, |(t, c)| c.class(t).speed).max(1.0);
                    if v < STILL { -p.regen } else { p.drain * (v / top).powi(2) }
                }
                _ => p.drain * drain_mult,
            };
            s.meter = (s.meter - loss * dt).clamp(0.0, FULL);
            if s.meter <= 0.0 {
                s.set(false, now);
            }
        } else if !s.armed {
            s.meter = (s.meter + p.regen * dt).min(FULL);
        }
        if press {
            if s.cloaked {
                if now + 1e-6 >= s.changed + CLOAK_LOCK {
                    s.set(false, now);
                }
            } else if p.kind == CloakKind::Feign {
                if s.armed {
                    s.armed = false;
                } else if s.meter >= FULL - 1e-3 {
                    s.armed = true;
                }
            } else if s.meter > 0.0 {
                s.set(true, now);
            }
        }
        // No attacks from the cloak until fully visible.
        if s.cloaked || s.hidden(now) > 0.0 {
            let at = s.visible_at(now);
            if inv.next_attack < at {
                inv.next_attack = at;
            }
            intent.fire = false;
        }
        if s.feign.is_some() && s.feigning(now).is_none() && !s.cloaked {
            s.feign = None;
        }
        if cloak.state != s {
            cloak.state = s;
        }
    }
}

/// The factor a Spy's disguise puts on his max speed (§5: the slower of
/// his class's `base` and the disguise's class's; cloaked, none).
pub fn speed_factor(inv: &Inventory, spy: Option<&Tf2Spy>, cloaks: &Query<&Tf2Cloak>, table: &Classes, base: f32) -> f32 {
    let Some(d) = spy.and_then(|s| s.disguise.as_ref()) else { return 1.0 };
    if inv.weapons.iter().any(|w| cloaks.get(*w).is_ok_and(|c| c.state.cloaked)) {
        return 1.0;
    }
    let Some(c) = table.get(&d.class) else { return 1.0 };
    if base <= 0.0 { 1.0 } else { (c.speed / base).min(1.0) }
}

/// The pose character `e` with `guise` on `team` shows this machine's
/// player (`viewer`: him and his team; None: a server, which draws
/// everyone as he is): his disguise to an enemy (Chimera's
/// `Guise::shown_to`), never to himself.
pub fn shown_pose<'a>(guise: Option<&'a Guise>, team: Option<&Team>, e: Entity, viewer: Option<(Entity, Option<&Team>)>) -> Option<&'a Pose> {
    let (me, mine) = viewer?;
    if me == e {
        return None;
    }
    guise?.shown_to(team, mine)
}

/// Whether `e` is a Spy (his gun's class).
fn is_spy(world: &World, e: Entity) -> bool {
    let (Some(c), Some(t)) = (world.get::<Tf2Class>(e), world.get_resource::<Classes>()) else { return false };
    c.class(t).name == SPY
}

/// What the server calls character `e` (its network name, else its
/// `Name`).
fn name_of(world: &World, e: Entity) -> String {
    if let Some(n) = world.get::<chimera_net::net::NetCharacter>(e) {
        return n.name.clone();
    }
    world.get::<Name>(e).map_or_else(String::new, |n| n.to_string())
}

/// The disguise as `class` of `team`, copying a living player of that
/// team (one of the class first; never `spy` himself).
fn disguise_of(world: &mut World, spy: Entity, class: &str, team: u8) -> Disguise {
    let max = world.get_resource::<Classes>().and_then(|t| t.get(class)).map_or(125.0, |c| c.health);
    let mut q = world.query::<(Entity, &Health, Option<&Team>, Option<&Tf2Class>)>();
    let table = world.get_resource::<Classes>().cloned();
    let mut best: Option<(bool, Entity, f32)> = None;
    for (e, h, t, c) in q.iter(world) {
        if e == spy || h.current <= 0.0 || t.map_or(0, |t| t.0) != team {
            continue;
        }
        let same = table.as_ref().zip(c).is_some_and(|(tb, c)| c.class(tb).name == class);
        if best.is_none_or(|(s, ..)| same && !s) {
            best = Some((same, e, h.current * 100.0));
        }
    }
    match best {
        Some((_, e, h)) => Disguise { class: class.into(), team, name: name_of(world, e), health: h.min(max).round(), max },
        None => Disguise { class: class.into(), team, name: String::new(), health: max, max },
    }
}

/// `e`'s watch, if he carries one.
fn watch_of(world: &World, e: Entity) -> Option<Entity> {
    world.get::<Inventory>(e)?.weapons.iter().copied().find(|w| world.get::<Tf2Cloak>(*w).is_some())
}

/// `e`'s `Tf2Spy`, put on with a `Guise` the first time.
fn spy_mut(world: &mut World, e: Entity) -> Mut<'_, Tf2Spy> {
    if world.get::<Tf2Spy>(e).is_none() {
        let guise = world.get::<Guise>(e).cloned().unwrap_or_default();
        world.entity_mut(e).insert((Tf2Spy::default(), guise));
    }
    world.get_mut::<Tf2Spy>(e).expect("put on")
}

/// Spy `e` starts disguising as `class` of `team` (None: the other side,
/// 1 and 2; else his own); §5, Your Eternal Reward's rule (§4). The
/// server's.
pub fn disguise(world: &mut World, e: Entity, class: &str, team: Option<u8>) -> Result<String, String> {
    if !is_spy(world, e) {
        let class = world.get::<Tf2Class>(e).map(|c| c.0);
        return Err(format!("only a Spy disguises (class {class:?})"));
    }
    if world.get_resource::<Classes>().and_then(|t| t.get(class)).is_none() {
        return Err(format!("no class {class}"));
    }
    if world.get::<Health>(e).is_none_or(|h| h.current <= 0.0) {
        return Err("dead".into());
    }
    let own = world.get::<Team>(e).map_or(0, |t| t.0);
    let team = team.unwrap_or(match own {
        1 => 2,
        2 => 1,
        t => t,
    });
    // Your Eternal Reward: a full cloak, emptied.
    let needs = world
        .get::<Inventory>(e)
        .is_some_and(|inv| inv.weapons.iter().any(|w| world.get::<Tf2Item>(*w).is_some_and(|i| i.0.disguise_needs_cloak)));
    if needs {
        let w = watch_of(world, e).ok_or("Your Eternal Reward: a full cloak meter needed")?;
        let mut c = world.get_mut::<Tf2Cloak>(w).expect("a watch");
        if c.state.meter < FULL - 1e-3 {
            return Err("Your Eternal Reward: a full cloak meter needed".into());
        }
        c.state.meter = 0.0;
    }
    let d = disguise_of(world, e, class, team);
    let now = world.resource::<SimClock>().now;
    let mut spy = spy_mut(world, e);
    let wait = if spy.disguised() { REDISGUISE_TIME } else { DISGUISE_TIME };
    let said = format!("disguising as {class} of team {team} ({:.1} s)", wait);
    spy.pending = Some((d, now + wait));
    spy.since = now;
    Ok(said)
}

/// A client asks to disguise (`tf2:disguise`).
#[derive(Message, Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct DisguiseRequest {
    pub class: String,
    pub team: Option<u8>,
}

/// Damage on and by Spies (module doc), before the damage lands: the
/// Enforcer's bonus, fire immunity, the Dead Ringer's feign and its cut,
/// the cloak's cut, the Spy-cicle's melt.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub fn taken(
    mut damage: MessageMutator<Damage>,
    holders: Query<(&Inventory, Option<&Tf2Spy>)>,
    items: Query<&Tf2Item>,
    mut cloaks: Query<&mut Tf2Cloak>,
    mut conditions: ResMut<Conditions>,
    boosts: Query<&SpeedBoost>,
    mut feigned: MessageWriter<crate::core::Feigned>,
    clock: Res<SimClock>,
    mut commands: Commands,
) {
    let now = clock.now;
    for d in damage.read() {
        if d.amount <= 0.0 {
            continue;
        }
        // The Enforcer: a shot while disguised.
        if let Some(a) = d.attacker.filter(|a| *a != d.target)
            && let Ok((inv, Some(spy))) = holders.get(a)
            && spy.disguised()
            && let Some(bonus) = inv.active.and_then(|w| items.get(w).ok()).map(|i| i.0.disguise_bonus).filter(|b| *b != 1.0)
        {
            d.amount = round_points(d.amount * bonus);
        }
        let v = d.target;
        let fire = d.kind == DamageKind::Burn;
        if fire && conditions.has(v, Condition::Game(FIRE_IMMUNE), now) {
            d.amount = 0.0;
            continue;
        }
        let Ok((inv, spy)) = holders.get(v) else { continue };
        // The Spy-cicle: this fire hit in full, then it melts.
        if fire
            && spy.is_none_or(|s| now >= s.melted)
            && let Some(p) = inv.weapons.iter().filter_map(|w| items.get(*w).ok()).map(|i| &i.0).find(|p| p.melts > 0.0)
        {
            conditions.give(v, Condition::Game(FIRE_IMMUNE), now + p.melt_fire_immune as f64, Some(v));
            conditions.give(v, Condition::Game(AFTERBURN_IMMUNE), now + p.melt_afterburn_immune as f64, Some(v));
            let until = now + p.melts as f64;
            commands.queue(move |w: &mut World| {
                if w.get_entity(v).is_ok() {
                    spy_mut(w, v).melted = until;
                }
            });
        }
        let Some(watch) = inv.weapons.iter().copied().find(|w| cloaks.contains(*w)) else { continue };
        let mut c = cloaks.get_mut(watch).expect("a watch");
        let mut s = c.state;
        if s.armed && !s.cloaked && c.params.kind == CloakKind::Feign {
            // The feign (§3.3).
            d.amount = round_points(d.amount * FEIGN_FIRST);
            s.armed = false;
            s.cloaked = true;
            s.changed = now;
            s.hidden_then = 1.0;
            s.feign = Some(now);
            s.meter = (s.meter - FEIGN_COST).max(0.0);
            feigned.write(crate::core::Feigned(Died { entity: v, attacker: d.attacker, damage: d.clone() }));
            let until = now + FEIGN_TIME as f64;
            conditions.give(v, Condition::Game(AFTERBURN_IMMUNE), until, Some(v));
            let b = SpeedBoost::longer(boosts.get(v).ok(), super::effects::boost(now, FEIGN_TIME));
            commands.entity(v).insert(b);
            commands.queue(move |w: &mut World| {
                if w.get::<Guise>(v).is_none() && w.get_entity(v).is_ok() {
                    w.entity_mut(v).insert(Guise::default());
                }
            });
        } else if s.cloaked {
            let cut = match s.feigning(now) {
                Some(t) => 1.0 - (FEIGN_CUT_A - FEIGN_CUT_B * t),
                None => CLOAK_CUT,
            };
            d.amount = round_points(d.amount * cut);
        }
        if c.state != s {
            c.state = s;
        }
    }
}

/// After the damage (the server's): no afterburn on the immune; the
/// knives' kills (Your Eternal Reward's disguise, the Big Earner's cloak
/// and boost).
#[allow(clippy::type_complexity)]
pub fn after(
    mut died: MessageReader<Died>,
    burning: Query<Entity, With<Burning>>,
    conditions: Res<Conditions>,
    holders: Query<&Inventory>,
    items: Query<&Tf2Item>,
    who: Query<(Option<&Tf2Class>, Option<&Team>, &Health)>,
    mut cloaks: Query<&mut Tf2Cloak>,
    boosts: Query<&SpeedBoost>,
    table: Option<Res<Classes>>,
    clock: Res<SimClock>,
    mut commands: Commands,
) {
    let now = clock.now;
    for e in &burning {
        if conditions.has(e, Condition::Game(AFTERBURN_IMMUNE), now) {
            commands.entity(e).remove::<Burning>();
        }
    }
    for k in died.read() {
        let Some(a) = k.damage.attacker.filter(|a| *a != k.entity) else { continue };
        let Ok(inv) = holders.get(a) else { continue };
        let Some(p) = inv.active.and_then(|w| items.get(w).ok()).map(|i| i.0.clone()) else { continue };
        if p.cloak_on_kill > 0.0
            && let Some(w) = inv.weapons.iter().copied().find(|w| cloaks.contains(*w))
            && let Ok(mut c) = cloaks.get_mut(w)
        {
            c.state.meter = (c.state.meter + p.cloak_on_kill).min(FULL);
        }
        if p.boost_on_kill > 0.0 {
            let b = SpeedBoost::longer(boosts.get(a).ok(), super::effects::boost(now, p.boost_on_kill));
            commands.entity(a).insert(b);
        }
        if p.disguise_on_backstab
            && k.damage.weapon == Some(BACKSTAB)
            && let Ok((class, team, h)) = who.get(k.entity)
            && let Some(t) = table.as_deref()
        {
            let class = class.map_or(crate::game::classes::DEFAULT, |c| c.class(t).name.as_str()).to_string();
            let team = team.map_or(0, |t| t.0);
            let victim = k.entity;
            let max = h.max * 100.0;
            commands.queue(move |w: &mut World| {
                if w.get_entity(a).is_err() {
                    return;
                }
                let name = name_of(w, victim);
                let d = Disguise { class, team, name, health: max, max };
                let mut spy = spy_mut(w, a);
                spy.pending = Some((d, now + YER_DELAY));
                spy.since = now;
            });
        }
    }
}

/// What others see of each Spy (`Guise`, the server's): how hidden his
/// watch has him, whom he poses as.
#[allow(clippy::type_complexity)]
pub fn show(
    mut spies: Query<(&Inventory, Option<&Tf2Spy>, &mut Guise)>,
    cloaks: Query<&Tf2Cloak>,
    table: Option<Res<Classes>>,
    clock: Res<SimClock>,
    mut commands: Commands,
    unshown: Query<(Entity, &Inventory), Without<Guise>>,
) {
    let now = clock.now;
    // A watch's first cloak puts the `Guise` on.
    for (e, inv) in &unshown {
        if inv.weapons.iter().any(|w| cloaks.get(*w).is_ok_and(|c| c.state.cloaked)) {
            commands.entity(e).insert(Guise::default());
        }
    }
    for (inv, spy, mut guise) in &mut spies {
        let hidden = inv.weapons.iter().filter_map(|w| cloaks.get(*w).ok()).map(|c| c.state.hidden(now)).fold(0.0, f32::max);
        let pose = spy.and_then(|s| s.disguise.as_ref()).map(|d| Pose {
            name: d.name.clone(),
            team: d.team,
            class: d.class.clone(),
            weapon: table.as_deref().and_then(|t| t.get(&d.class)?.primary.clone()),
            health: d.health / 100.0,
            max: d.max / 100.0,
        });
        let want = Guise { hidden: Guise::level(hidden), pose };
        if *guise != want {
            *guise = want;
        }
    }
}

/// The console's `tf2:disguise`, a client's request on the server.
fn receive_requests(
    mut requests: MessageReader<bevy_replicon::prelude::FromClient<DisguiseRequest>>,
    players: Query<&chimera_net::net::server::Player>,
    mut commands: Commands,
) {
    for r in requests.read() {
        let Some(client) = r.client_id.entity() else { continue };
        let Ok(p) = players.get(client) else { continue };
        let (character, m) = (p.character, r.message.clone());
        commands.queue(move |w: &mut World| {
            if w.get_entity(character).is_ok()
                && let Err(why) = disguise(w, character, &m.class, m.team)
            {
                debug!("disguise refused: {why}");
            }
        });
    }
}

fn register(app: &mut App) {
    use bevy_replicon::prelude::*;
    app.add_client_message::<DisguiseRequest>(Channel::Ordered).add_systems(
        PreUpdate,
        receive_requests.after(ServerSystems::Receive).run_if(in_state(ServerState::Running)),
    );
}

/// The hook, the parts, the systems, the command (`mods::add` calls it).
pub fn add(app: &mut App) {
    use chimera_net::net::parts::NetAppExt;
    use chimera_sim::weapon::{CarriedAppExt, SelectWeapons, data::WeaponDataAppExt};
    use crate::console::ConsoleAppExt;
    app.register_weapon_hook::<CloakParams>(HOOK, |e, _, p| attach(e, p))
        .predicted_part::<Tf2Cloak>()
        .net_part("tf2_spy", register)
        .add_systems(crate::core::Predict::Select, think.before(SelectWeapons))
        .add_systems(
            FixedUpdate,
            (
                taken.after(super::consumables::taken).before(crate::core::apply_damage),
                (after, show).chain().after(crate::core::apply_damage),
            )
                .run_if(crate::core::authoritative),
        );
    crate::core::PredictedAppExt::predicted_net::<Tf2Spy>(app);
    app.console_command(
        "tf2:disguise",
        "tf2:disguise <class> [team] [targets]: a Spy disguises as that class (scout ... engineer) of that team (1 RED, \
         2 BLU; default the other side); `targets`: every practice target that is a Spy instead (the server's).",
        |w, a| {
            let class = a.first().map(|s| s.to_ascii_lowercase()).ok_or("tf2:disguise <class> [team]")?;
            let team = a.get(1).filter(|t| !t.eq_ignore_ascii_case("targets")).and_then(|t| match t.to_ascii_lowercase().as_str() {
                "red" => Some(1),
                "blu" | "blue" => Some(2),
                n => n.parse().ok(),
            });
            if a.iter().any(|x| x.eq_ignore_ascii_case("targets")) {
                let targets: Vec<Entity> =
                    w.query_filtered::<Entity, With<chimera_sim::minigame::targets::Target>>().iter(w).collect();
                let done = targets.into_iter().filter(|e| disguise(w, *e, &class, team).is_ok()).count();
                return Ok(Some(format!("{done} targets disguising")));
            }
            let me = w
                .query_filtered::<Entity, With<crate::core::LocalPlayer>>()
                .iter(w)
                .next()
                .ok_or("no local player")?;
            if w.get_resource::<crate::core::NetRole>().is_some_and(|r| *r == crate::core::NetRole::Client) {
                w.write_message(DisguiseRequest { class, team });
                return Ok(Some("disguise requested".into()));
            }
            disguise(w, me, &class, team).map(Some)
        },
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_cloak_fades_in_one_second_and_out_in_two() {
        let mut s = CloakState::default();
        s.set(true, 10.0);
        assert!((s.hidden(10.5) - 0.5).abs() < 1e-6);
        assert_eq!(s.hidden(11.0), 1.0);
        s.set(false, 11.0);
        assert!((s.hidden(12.0) - 0.5).abs() < 1e-6);
        assert_eq!(s.visible_at(12.0), 13.0);
    }
}
