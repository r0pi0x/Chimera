//! The seam every Halo damage source delivers through: `HaloHit`, "this
//! damage effect hit that character there" (specs/halo_ce/health_shields.md
//! "The damage event", projectiles.md behaviour 18). A weapon, projectile,
//! explosion, melee or fall writes one `HaloHit` per target with the
//! effect's id and the amount it computed (its side of the seam: falloff,
//! the random draw); `deliver_hits` applies it on the server after the
//! tick's weapons and before the engine's `core::apply_damage`, and ends
//! in the engine's `Damage` so health, `Died`, kill credit, the kill feed
//! and a gun game's ladder work as for every game.
//!
//! **The contract.**
//! - `target`: the character (or anything with health) hit.
//! - `effect`: an `EffectId` of `data/damage.toml` (`vitality::ids`: e.g.
//!   `ids::PISTOL_BULLET`); its numbers are `Res<DamageEffects>`.
//! - `amount`: A in points, already `((1 - s) lower + s U) x m`
//!   (`DamageEffect::amount(s, u)` with the hit's draw u in 0 to 1); a
//!   zero amount does nothing at all.
//! - `scale`: s (0 to 1; 1.5 for a lunge, `vitality::melee_scale`): the
//!   stun scales by it.
//! - `region`: where a direct hit landed, as the engine's hitboxes report
//!   it (`BodyRegion`); None for area hits, melee, falls and riders (the
//!   indirect material). On a Spartan the head is the head material
//!   (headshots), shins and feet the legs (body x 0.8), the rest the body;
//!   `material` overrides it with the collision model's material index
//!   when a hit knows it.
//! - `area`: area of effect (explosions, melee): no material.
//! - `epicenter`: the impact point, explosion centre, or the attacker's
//!   centre for melee (which decides "unsuspecting": a melee from behind
//!   kills); `dir`: unit direction of travel (pings).
//! - `instigator`: the owner (credit, friendly fire); None for falls.
//! - `weapon`: the weapon id for kill notices (None: the one held);
//!   `silent`: no pings; `bypass_shields`: the shield is left out.
//!
//! Use, in a weapon's system on the server:
//!
//! ```ignore
//! let fx = effects.get(ids::PISTOL_BULLET).unwrap();
//! hits.write(
//!     HaloHit::direct(target, ids::PISTOL_BULLET, fx.amount(1.0, draw), 1.0, Some(region), point, dir, Some(shooter))
//!         .weapon("halo_ce:pistol"),
//! );
//! hits.write(HaloHit::melee(target, ids::PISTOL_MELEE, fx.amount(s, draw), s, attacker_centre, aim, Some(attacker)));
//! hits.write(HaloHit::fall(&effects, player, q_f)); // movement: a hard landing
//! ```
//!
//! What it does by target (docs/DESIGN.md "Hits across games"):
//! - **A Halo character** (`Vitality`): the spec's whole rule
//!   (`Vitality::take`: shield then body with the material multipliers
//!   and the effect's material modifiers, the carry-over, emp, skips and
//!   only-shields, headshots, the overshield's charge-up, instant kills,
//!   the strict below-zero death), then stun, camouflage loss, the
//!   attacker list, a `HaloPing` and a `HaloHurt` (unzoom); `Armor` is
//!   the shield, `Health` the body (`vitality`; under the party's
//!   cross-game incoming, `cross_game`, the shield's unit is scaled and the
//!   engine's `Damage` is written divided by the character's
//!   `core::DamageScale`, so the table's incoming counts once; Halo's own
//!   hits count the table's scale too, Halo's outgoing times the body's
//!   incoming, `cross_game::own_scale`: the spec's numbers with the
//!   switch off or no table). A death's credit: the
//!   killing hit's instigator, or, when that hit was friendly or had none
//!   (a fall, a teammate), the enemy who did the most damage within the
//!   last 6 s.
//! - **Another game's character**: an unshielded Spartan's body
//!   (material type 21, cyborg: what Halo's guns are tuned against; the
//!   head counts as "head", shins and feet as "legs"), a Halo point 1/75
//!   of its health, then that character's own armour by its game's rule
//!   (`weapon::HitArmor`, ratio 1, fractions kept); a "can cause
//!   headshots" hit whose damage reaches a head's health, or a melee
//!   from behind, kills.
//! - A fall (`fall`, `lethal_fall`: falling category, no instigator) is
//!   the map's damage, not a game's: no cross-game table's scale on
//!   either kind of body (the party's rule: self and fall damage
//!   untouched).
//! - A hit that does nothing to a Halo character (an immune material:
//!   the fuel rod on a Hunter) is noted for a feedback report
//!   (`core::RecentHits`).
//! - Either way the engine's team rules decide first (`DamageRules`: a
//!   teammate's hit with friendly fire off, a protected or noclipping
//!   target do nothing), as for every game's hits; self damage counts.
//!
//! Prediction: like the other games' damage, only the server applies
//! hits (`core::authoritative`); clients see `Health`, `Armor` and the
//! replicated `Vitality`, and predict `Stun`.
//!
//! Known shortcuts:
//! - The push (instantaneous acceleration) is the weapons' and
//!   projectiles' side (projectiles.md behaviour 11): `Damage::force` is
//!   zero here.
//! - The parent chain (step 2) is the target alone: no vehicles or
//!   attached objects yet; riders' passthrough comes with vehicles.
//! - The game type's friendly fire option is not read (health_shields.md
//!   Open question 2): the engine's `mp_friendlyfire` and teams decide.
//! - Kill credit tests the attacker's team, the spec's intent, not the
//!   reference's player-index comparison (Quirks, Open question 6).

use bevy::{ecs::system::ParamSet, prelude::*};
use chimera_sim::weapon::{Armor, HitArmor};

use crate::core::{BodyRegion, Damage, DamageRules, Health, Hitgroup, Intent, RegionHit, SimClock, Team};

use super::vitality::{
    self, DamageEffects, EffectId, Event, HaloPing, Mirrored, Outcome, Spartan, Stun, Vitality,
    data::{Category, SideEffect, flags},
};

/// The hit's flags.
pub mod hit_flags {
    /// Area of effect (and the player's melee): no material.
    pub const AREA: u8 = 1;
    /// No pings (scripts).
    pub const SILENT: u8 = 2;
    /// The shield is left out (AI riders).
    pub const BYPASS_SHIELDS: u8 = 4;
}

/// One damage event on one target (the module doc's contract).
#[derive(Message, Clone, Debug, PartialEq)]
pub struct HaloHit {
    pub target: Entity,
    pub effect: EffectId,
    pub amount: f32,
    pub scale: f32,
    pub region: Option<BodyRegion>,
    pub material: Option<usize>,
    pub flags: u8,
    pub epicenter: Vec3,
    pub dir: Vec3,
    pub instigator: Option<Entity>,
    pub weapon: Option<&'static str>,
}

impl HaloHit {
    /// A direct hit (a bullet, a bolt, a stuck needle's or grenade's
    /// attached damage with `region` None) at `point`.
    #[allow(clippy::too_many_arguments)]
    pub fn direct(
        target: Entity,
        effect: EffectId,
        amount: f32,
        scale: f32,
        region: Option<BodyRegion>,
        point: Vec3,
        dir: Vec3,
        instigator: Option<Entity>,
    ) -> Self {
        Self {
            target,
            effect,
            amount,
            scale,
            region,
            material: None,
            flags: 0,
            epicenter: point,
            dir,
            instigator,
            weapon: None,
        }
    }

    /// An area hit from `epicenter` (an explosion; `dir` from it to the
    /// target's centre).
    pub fn area(target: Entity, effect: EffectId, amount: f32, scale: f32, epicenter: Vec3, dir: Vec3, instigator: Option<Entity>) -> Self {
        Self {
            flags: hit_flags::AREA,
            ..Self::direct(target, effect, amount, scale, None, epicenter, dir, instigator)
        }
    }

    /// A melee on a biped: an area hit from the attacker's centre along
    /// its aim (so never a headshot; from behind, a kill).
    pub fn melee(target: Entity, effect: EffectId, amount: f32, scale: f32, attacker_centre: Vec3, aim: Vec3, attacker: Option<Entity>) -> Self {
        Self::area(target, effect, amount, scale, attacker_centre, aim, attacker)
    }

    /// A landing's falling damage at scale `q_f` (movement.md "Landing
    /// and falling"): `globals\falling`, no instigator.
    pub fn fall(effects: &DamageEffects, target: Entity, q_f: f32) -> Self {
        let amount = effects.get(vitality::ids::FALLING).map_or(0.0, |fx| fx.amount(q_f, 0.0));
        Self::area(target, vitality::ids::FALLING, amount, q_f, Vec3::ZERO, Vec3::ZERO, None)
    }

    /// A too-fast fall's tick: `globals\distance`, 105 points.
    pub fn lethal_fall(effects: &DamageEffects, target: Entity) -> Self {
        let amount = effects.get(vitality::ids::DISTANCE).map_or(0.0, |fx| fx.amount(1.0, 0.0));
        Self::area(target, vitality::ids::DISTANCE, amount, 1.0, Vec3::ZERO, Vec3::ZERO, None)
    }

    /// The weapon id for kill notices.
    pub fn weapon(mut self, weapon: &'static str) -> Self {
        self.weapon = Some(weapon);
        self
    }

    /// The collision model's material index hit.
    pub fn material(mut self, index: usize) -> Self {
        self.material = Some(index);
        self
    }

    /// Add `flags` (`hit_flags`).
    pub fn with(mut self, flags: u8) -> Self {
        self.flags |= flags;
        self
    }

    pub fn is(&self, flag: u8) -> bool {
        self.flags & flag != 0
    }
}

/// What a hit did to a Halo character (after `deliver_hits`): for
/// reactions that are the weapons' or the HUD's (a scoped player drops
/// out of zoom when `shield_taken` or `body_amount` is above 0).
#[derive(Message, Clone, Copy, Debug, PartialEq)]
pub struct HaloHurt {
    pub target: Entity,
    pub instigator: Option<Entity>,
    pub effect: EffectId,
    pub outcome: Outcome,
    /// The event's scale and the direction the damage travelled (its
    /// victim's presentation, `fx::victim`).
    pub scale: f32,
    pub dir: Vec3,
}

/// One remembered attacker (health_shields.md "Death", credit).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Attacker {
    pub entity: Entity,
    pub damage: f32,
    pub tick: u64,
}

/// The attackers a Halo character remembers for kill credit (4), server
/// side.
#[derive(Component, Clone, Debug, Default, PartialEq)]
#[component(storage = "SparseSet")]
pub struct Attackers(pub Vec<Attacker>);

impl Attackers {
    /// Record `damage` by `entity` at `tick`: added to its entry, or a new
    /// one; a full list replaces its oldest entry but the biggest.
    pub fn record(&mut self, entity: Entity, damage: f32, tick: u64, cap: usize) {
        if let Some(a) = self.0.iter_mut().find(|a| a.entity == entity) {
            a.damage += damage;
            a.tick = tick;
            return;
        }
        let new = Attacker { entity, damage, tick };
        if self.0.len() < cap {
            self.0.push(new);
            return;
        }
        let biggest = (0..self.0.len()).max_by(|a, b| self.0[*a].damage.total_cmp(&self.0[*b].damage));
        if let Some(oldest) = (0..self.0.len()).filter(|i| Some(*i) != biggest).min_by_key(|i| self.0[*i].tick) {
            self.0[oldest] = new;
        }
    }

    /// The attacker with the most damage among those within `window`
    /// ticks of `now` that `is_enemy` accepts.
    pub fn best(&self, now: u64, window: u64, is_enemy: impl Fn(Entity) -> bool) -> Option<Entity> {
        self.0
            .iter()
            .filter(|a| now.saturating_sub(a.tick) <= window && is_enemy(a.entity))
            .max_by(|a, b| a.damage.total_cmp(&b.damage))
            .map(|a| a.entity)
    }
}

/// Damage takes and the hit's flags, read before a hit counts.
pub struct DamagePlugin;

impl Plugin for DamagePlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<HaloHit>()
            .add_message::<HaloHurt>()
            .add_message::<Damage>()
            .add_message::<RegionHit>()
            .add_systems(
                FixedUpdate,
                deliver_hits
                    .after(crate::core::SimSet::Weapons)
                    .before(crate::core::apply_damage)
                    .run_if(crate::core::authoritative),
            );
    }
}

/// What a killing hit takes beyond what is left (normalized health).
const LETHAL_MARGIN: f32 = 1e-3;

/// A character's 3D looking direction (engine space).
fn look_of(intent: Option<&Intent>) -> Vec3 {
    intent.map_or(Vec3::NEG_Z, |i| chimera_sim::weapon::aim_of(i.yaw, i.pitch) * Vec3::NEG_Z)
}

/// A Halo character hit by `deliver_hits`.
type HaloTarget<'a> = (
    &'a mut Vitality,
    &'a Health,
    Option<&'a mut Armor>,
    Option<&'a mut Stun>,
    Option<&'a mut Attackers>,
    Option<&'a mut Mirrored>,
    &'a Transform,
    Option<&'a Intent>,
    Option<&'a crate::core::DamageScale>,
    Option<&'a super::campaign::BodyKind>,
);

/// Another game's body hit by `deliver_hits`.
type OtherTarget<'a> = (Option<&'a Health>, Option<&'a Transform>, Option<&'a Intent>);

/// Apply the tick's `HaloHit`s (the module doc).
#[allow(clippy::too_many_arguments)]
pub fn deliver_hits(
    mut hits: MessageReader<HaloHit>,
    (spartan, tables): (Res<Spartan>, Option<Res<super::campaign::BodyTables>>),
    effects: Res<DamageEffects>,
    mut set: ParamSet<(Query<HaloTarget>, HitArmor)>,
    others: Query<OtherTarget, Without<Vitality>>,
    teams: Query<&Team>,
    rules: DamageRules,
    clock: Res<SimClock>,
    balance: Option<Res<chimera_sim::weapon::balance::Balance>>,
    mut out: (
        MessageWriter<Damage>,
        MessageWriter<RegionHit>,
        MessageWriter<HaloPing>,
        MessageWriter<HaloHurt>,
        Option<ResMut<crate::core::RecentHits>>,
    ),
) {
    let team = |e: Entity| teams.get(e).map_or(0, |t| t.0);
    for hit in hits.read() {
        let Some(fx) = effects.get(hit.effect) else {
            warn!("halo_ce: a hit with unknown damage effect {:?}", hit.effect);
            continue;
        };
        if hit.amount <= 0.0 {
            continue;
        }
        let region = if hit.is(hit_flags::AREA) { None } else { hit.region };
        let mut damage = Damage {
            target: hit.target,
            attacker: hit.instigator,
            amount: 0.0,
            point: hit.epicenter,
            dir: hit.dir,
            hitgroup: region.map_or(Hitgroup::Generic, |r| r.hitgroup()),
            kind: fx.category.kind(),
            weapon: hit.weapon,
            force: Vec3::ZERO,
        };
        if !rules.lands(&damage) {
            continue;
        }
        // A fall (no instigator): the map's damage, not a game's.
        let environment = hit.instigator.is_none() && fx.category == Category::Falling;
        let friendly = hit
            .instigator
            .is_some_and(|i| i == hit.target || (team(i) != 0 && team(i) == team(hit.target)));
        let mut halo = set.p0();
        if let Ok((mut v, health, mut armor, stun, attackers, mirrored, transform, intent, scale, kind)) = halo.get_mut(hit.target) {
            // The body's own table (`campaign::vitality_of`).
            let spartan = super::campaign::vitality_of(&spartan, tables.as_deref(), kind);
            let look = look_of(intent);
            // Another game's damage counts this on it (the party's
            // balance, `cross_game`); Halo's own counts the table's too:
            // Halo's outgoing times this body's incoming, the same
            // `Balance::pair` as every game's.
            let scale = vitality::scale_of(scale);
            // A fall is the map's, not a game's: no table's scale (the
            // party's rule: self and fall damage untouched).
            let own = if environment { 1.0 } else { super::cross_game::own_scale(balance.as_deref(), kind) };
            let mirrored = mirrored.filter(|m| m.primed);
            if let Some(m) = mirrored.as_deref() {
                vitality::take_foreign(spartan, &mut v, health, armor.as_deref(), m, scale);
            }
            let was_dead = v.dead;
            let before = vitality::health_of(v.body);
            let event = Event {
                effect: fx,
                amount: hit.amount,
                scale: hit.scale,
                material: if hit.is(hit_flags::AREA) {
                    None
                } else {
                    hit.material.or(region.map(|r| spartan.material_of(Some(r))))
                },
                friendly,
                unsuspecting: vitality::unsuspecting(transform.translation, hit.epicenter, look),
                bypass_shields: hit.is(hit_flags::BYPASS_SHIELDS),
                game_multiplier: own,
            };
            let outcome = v.take(spartan, &event);
            // For a report: a hit that took nothing (an immune body: a
            // material modifier of 0).
            if outcome.shield_taken == 0.0 && outcome.body_fraction == 0.0 && !outcome.died {
                if let Some(r) = out.4.as_deref_mut() {
                    r.note(
                        clock.tick,
                        format!(
                            "{} -> {} ({}): {} {:?} did nothing (amount {:.1}, scale {:.2})",
                            crate::core::RecentHits::who(hit.instigator),
                            crate::core::RecentHits::who(Some(hit.target)),
                            super::cross_game::body_id(kind),
                            hit.weapon.unwrap_or("-"),
                            hit.effect,
                            hit.amount,
                            own
                        ),
                    );
                }
            }
            if !outcome.landed {
                continue;
            }
            if let Some(mut stun) = stun {
                stun.hit(spartan, fx, hit.scale);
            }
            // The attacker list and the kill's credit.
            let mut attacker = hit.instigator;
            if let Some(mut list) = attackers {
                if let Some(i) = hit.instigator {
                    list.record(i, outcome.shield_taken + outcome.body_amount, clock.tick, spartan.constants.attackers);
                }
                if outcome.died && (hit.instigator.is_none() || friendly) {
                    let victim = team(hit.target);
                    let enemy = |a: Entity| a != hit.target && (victim == 0 || team(a) != victim);
                    if let Some(credit) = list.best(clock.tick, spartan.constants.credit_window, enemy) {
                        attacker = Some(credit);
                    }
                }
            }
            if outcome.died {
                v.die();
            }
            // The engine's view: the shield is the armour, the body the
            // health; the death is a hit that takes all that is left.
            let after = if v.dead { 0.0 } else { vitality::health_of(v.body) };
            damage.attacker = attacker;
            damage.amount = if v.dead && !was_dead {
                // More than is left, whatever this tick's other hits did.
                before.max(health.current) - v.body.min(0.0) + LETHAL_MARGIN
            } else {
                (before - after).max(0.0)
            } / scale;
            let shield = super::cross_game::shield_unit(v.shield, scale);
            if let Some(a) = armor.as_deref_mut() {
                a.amount = shield;
            }
            if let Some(mut m) = mirrored {
                m.shield = shield;
                m.health = if v.dead { 0.0 } else { after };
            }
            // Pings, for animation.
            let ping = !hit.is(hit_flags::SILENT) && !fx.has(flags::DOES_NOT_PING_UNITS) && (!was_dead || outcome.died);
            if ping {
                let unit = &spartan.unit;
                let soft = v.current_body_damage > unit.soft_ping_threshold || v.current_shield_damage > unit.soft_ping_threshold;
                let hard = v.current_body_damage > unit.hard_ping_threshold || outcome.hard_forced;
                out.2.write(HaloPing {
                    target: hit.target,
                    soft: soft || hard,
                    hard,
                    kill: outcome.died,
                    direction: vitality::ping_direction(spartan, hit.dir, look),
                });
            }
            out.3.write(HaloHurt {
                target: hit.target,
                instigator: hit.instigator,
                effect: hit.effect,
                outcome,
                scale: hit.scale,
                dir: hit.dir,
            });
        } else {
            // Another game's body: an unshielded Spartan's, then its own
            // armour.
            if fx.has(flags::ONLY_HURTS_SHIELDS) {
                continue;
            }
            let material = spartan.material_of(region);
            let m = &spartan.model.materials[material.min(spartan.model.materials.len() - 1)];
            let a = m.body * hit.amount;
            let mut x = a * (1.0 / spartan.model.maximum_body) * fx.modifier(CROSS_GAME_MATERIAL);
            if m.head && a > 0.0 && fx.has(flags::CAN_CAUSE_MULTIPLAYER_HEADSHOTS) {
                x *= 2.0;
            }
            let at = region.unwrap_or(BodyRegion::Generic);
            damage.amount = x;
            // Halo's damage, whatever its weapon: the party's cross-game
            // table's scale of Halo's on this body (`weapon::balance`).
            // A fall: the map's, unscaled (`apply_environment`).
            if environment {
                set.p1().apply_environment(&mut damage, x, Some(1.0), 0.0);
            } else {
                set.p1().apply_region_of(Some(super::GAME), &mut damage, at, x, Some(1.0), 0.0);
            }
            let (health, transform, intent) = others.get(hit.target).unwrap_or((None, None, None));
            let instant = fx.side_effect == SideEffect::LethalToTheUnsuspecting
                && transform.is_some_and(|t| vitality::unsuspecting(t.translation, hit.epicenter, look_of(intent)));
            let headshot = m.head && fx.has(flags::CAN_CAUSE_HEADSHOTS) && damage.amount > 0.0;
            if headshot || instant {
                damage.amount = health.map_or(damage.amount, |h| h.current.max(damage.amount) + LETHAL_MARGIN);
            }
        }
        if let Some(r) = region {
            out.1.write(RegionHit { target: hit.target, region: r });
        }
        out.0.write(damage);
    }
}

/// The material type another game's body counts as for a Halo damage
/// effect's modifier: 21, cyborg (the Spartan's body).
pub const CROSS_GAME_MATERIAL: usize = 21;
