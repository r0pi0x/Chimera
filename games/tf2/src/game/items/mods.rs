//! What an item does to its holder (specs/tf2/item_attributes.md 1, 2.2,
//! 3.2, 4.4, 4.8, 4.10, 4.11): the hook `tf2:item` (`ItemParams`, the
//! weapon part `Tf2Item`) and its rules.
//!
//! - **Provided or active** (§1): an item's player numbers count while
//!   its holder carries it, or only while it is his active weapon when it
//!   has `provide on active`; some are asked of the active weapon only
//!   (`move_speed_active`, `jump`, `taken_ranged`, `taken_melee`, `drain`,
//!   `afterburn_immune`). `Factors::of` folds them over the holder's
//!   inventory (several items multiply).
//! - **Speed and jump** (4.4): the class speed rules as today, then ×
//!   every provided `move_speed`, then × the active weapon's
//!   `move_speed_active` (`movement::select_class`); the jump's (and the
//!   Scout's air jump's) speed × the active weapon's `jump`
//!   (`SourceTraits::jump_scale`).
//! - **Damage taken** (§3.2, `taken`, before the engine applies damage,
//!   the server's): bullet and fire hits × `taken_bullets` / `taken_fire`,
//!   a spun-up Heavy's hit that leaves him at half health or less ×
//!   `spun_resist`; then everything × `taken`; then the active weapon's
//!   `taken_melee` (melee) or `taken_ranged` (blast, bullet, fire);
//!   rounded to whole points, halves up.
//! - **Health** (4.8, `heals`, after the damage): a damaging hit (not a
//!   burn tick) with the item heals its holder max(3, `heal_on_hit`), a
//!   kill `heal_on_kill`, both up to his max health; `ignite` sets the
//!   victim alight as a flame hit does (the Flame Thrower's afterburn).
//! - **Max-health drain** (4.10, `drain`, `ItemDrains`): while an item
//!   with `drain` is out, its holder's max health falls by it per second
//!   (20 at once on a deploy after a refill), never below 100; holstered,
//!   it comes back as fast; his health keeps its share
//!   (`movement::class_health` takes `ItemDrains::of`).
//! - **Switching** (4.9, `holster`, predicted with the switch): the
//!   weapon switched to deploys in its `draw` × the one switched from's
//!   `holster` (`mult_switch_from_wep_deploy_time`, only when that one had
//!   been out longer than `HOLSTER_AFTER`) × 1.75 when the one switched
//!   from is a sword and the new one isn't (a sword's own `draw` has it).
//!
//! Known shortcuts:
//! - Wearables (Darwin's Danger Shield, the Cozy Camper) convert and map,
//!   but nothing puts one on a player yet (no loadouts), and regen, the
//!   scoped knockback and flinch rules and afterburn immunity aren't
//!   applied; the draw animation keeps its rate on a longer deploy.
//! - Crits are off (the party's choice), so a damage-taken factor acts on
//!   the whole hit (the spec's base part is the whole).

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use super::convert::ItemRecord;
use crate::core::{Damage, DamageKind, DamageTaken, Died, Health, SimClock};
use chimera_sim::weapon::{Inventory, Weapon};

/// `tf2:item`'s parameters: the item's player numbers (identities: 1, 0,
/// false).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct ItemParams {
    pub index: u32,
    pub provide_on_active: bool,
    pub move_speed: f32,
    pub move_speed_active: f32,
    pub jump: f32,
    pub taken: f32,
    pub taken_bullets: f32,
    pub taken_fire: f32,
    pub taken_ranged: f32,
    pub taken_melee: f32,
    pub spun_resist: f32,
    pub heal_on_hit: f32,
    pub heal_on_kill: f32,
    pub regen: f32,
    pub drain: f32,
    pub holster: f32,
    pub ignite: bool,
    pub afterburn_immune: bool,
    /// A sword (`is_a_sword`): switching from it takes ×1.75 (4.9; to it,
    /// its own `draw` has it).
    pub sword: bool,
    /// The stock weapon of ours it is built over (`super::base_in`: its
    /// view-model sequences, effects and stance are that weapon's).
    pub base: String,
    /// Its model's skin (`visuals` `skin`; none: the team's).
    pub skin: Option<i32>,
}

impl Default for ItemParams {
    fn default() -> Self {
        Self {
            index: 0,
            provide_on_active: false,
            move_speed: 1.0,
            move_speed_active: 1.0,
            jump: 1.0,
            taken: 1.0,
            taken_bullets: 1.0,
            taken_fire: 1.0,
            taken_ranged: 1.0,
            taken_melee: 1.0,
            spun_resist: 1.0,
            heal_on_hit: 0.0,
            heal_on_kill: 0.0,
            regen: 0.0,
            drain: 0.0,
            holster: 1.0,
            ignite: false,
            afterburn_immune: false,
            sword: false,
            base: String::new(),
            skin: None,
        }
    }
}

impl ItemParams {
    /// The record's player numbers (the converter table, §2.2).
    pub fn of(r: &ItemRecord) -> Self {
        let a = |c: &str| r.attr(c, 1.0) as f32;
        let add = |c: &str| r.attr(c, 0.0) as f32;
        Self {
            index: r.index,
            provide_on_active: r.flag("provide_on_active"),
            move_speed: a("mult_player_movespeed"),
            move_speed_active: a("mult_player_movespeed_active"),
            jump: a("mod_jump_height_from_weapon"),
            taken: a("mult_dmgtaken"),
            taken_bullets: a("mult_dmgtaken_from_bullets"),
            taken_fire: a("mult_dmgtaken_from_fire"),
            taken_ranged: a("dmg_from_ranged"),
            taken_melee: a("dmg_from_melee"),
            spun_resist: a("spunup_damage_resistance"),
            heal_on_hit: add("add_onhit_addhealth"),
            heal_on_kill: add("heal_on_kill"),
            regen: add("add_health_regen"),
            drain: add("mod_maxhealth_drain_rate"),
            holster: a("mult_switch_from_wep_deploy_time"),
            ignite: r.flag("set_dmgtype_ignite"),
            afterburn_immune: r.flag("afterburn_immunity"),
            sword: r.flag("is_a_sword"),
            base: String::new(),
            skin: r.visuals.skin.and_then(|s| i32::try_from(s).ok()),
        }
    }
}

/// An item weapon's part: its player numbers.
#[derive(Component, Clone, Debug, PartialEq)]
pub struct Tf2Item(pub ItemParams);

/// A holder's folded player numbers (module doc).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Factors {
    pub speed: f32,
    pub jump: f32,
    pub taken: f32,
    pub taken_bullets: f32,
    pub taken_fire: f32,
    pub taken_ranged: f32,
    pub taken_melee: f32,
    pub spun_resist: f32,
}

impl Default for Factors {
    fn default() -> Self {
        Self { speed: 1.0, jump: 1.0, taken: 1.0, taken_bullets: 1.0, taken_fire: 1.0, taken_ranged: 1.0, taken_melee: 1.0, spun_resist: 1.0 }
    }
}

impl Factors {
    /// The numbers of `inv`'s items (`items` reads a weapon's).
    pub fn of(inv: &Inventory, items: &impl Fn(Entity) -> Option<ItemParams>) -> Factors {
        let mut f = Factors::default();
        for &w in &inv.weapons {
            let Some(p) = items(w) else { continue };
            let active = inv.active == Some(w);
            if active || !p.provide_on_active {
                f.speed *= p.move_speed;
                f.taken *= p.taken;
                f.taken_bullets *= p.taken_bullets;
                f.taken_fire *= p.taken_fire;
                f.spun_resist *= p.spun_resist;
            }
            if active {
                f.speed *= p.move_speed_active;
                f.jump *= p.jump;
                f.taken_ranged *= p.taken_ranged;
                f.taken_melee *= p.taken_melee;
            }
        }
        f
    }

    /// Hit `d` (points) of `kind` on a holder with these numbers: `spun`
    /// a spun-up Heavy, `health` and `max` his.
    pub fn taken_points(&self, points: f32, kind: DamageKind, spun: bool, health: f32, max: f32) -> f32 {
        let mut x = points;
        match kind {
            DamageKind::Bullet => x *= self.taken_bullets,
            DamageKind::Burn => x *= self.taken_fire,
            _ => {}
        }
        if spun && health - x <= max * 0.5 {
            x *= self.spun_resist;
        }
        x *= self.taken;
        match kind {
            DamageKind::Melee => x *= self.taken_melee,
            DamageKind::Bullet | DamageKind::Burn => x *= self.taken_ranged,
            k if k.is_blast() => x *= self.taken_ranged,
            _ => {}
        }
        x
    }
}

/// Every hit on a holder of items, scaled by their numbers (module doc).
pub fn taken(
    mut damage: MessageMutator<Damage>,
    holders: Query<(&Inventory, &Health)>,
    items: Query<&Tf2Item>,
    miniguns: Query<&crate::game::weapons::minigun::Tf2Minigun>,
) {
    for d in damage.read() {
        let Ok((inv, h)) = holders.get(d.target) else { continue };
        let f = Factors::of(inv, &|w| items.get(w).ok().map(|i| i.0.clone()));
        if f == Factors::default() {
            continue;
        }
        let spun = inv
            .active
            .and_then(|w| miniguns.get(w).ok())
            .is_some_and(|m| m.state != crate::game::weapons::minigun::State::Idle);
        let points = f.taken_points(d.amount * 100.0, d.kind, spun, h.current * 100.0, h.max * 100.0);
        d.amount = crate::game::weapons::damage::round_points(points / 100.0);
    }
}

/// The least an on-hit heal gives (4.8).
pub const ON_HIT_MIN: f32 = 3.0;

/// Heals on hit and kill, and ignition, from the hits that landed.
#[allow(clippy::too_many_arguments)]
pub fn heals(
    mut taken: MessageReader<DamageTaken>,
    mut died: MessageReader<Died>,
    mut health: Query<&mut Health>,
    holders: Query<&Inventory>,
    weapons: Query<(&Weapon, &Tf2Item)>,
    clock: Res<SimClock>,
    mut flame: Local<Option<crate::game::weapons::flame::FlameParams>>,
    mut commands: Commands,
) {
    // The item that dealt `d`: the attacker's active weapon, when it is
    // the one named (or none is).
    let item = |d: &Damage| -> Option<(ItemParams, &'static str)> {
        let a = d.attacker.filter(|a| *a != d.target)?;
        let w = holders.get(a).ok()?.active?;
        let (weapon, item) = weapons.get(w).ok()?;
        d.weapon.is_none_or(|n| n == weapon.id).then(|| (item.0.clone(), weapon.id))
    };
    let mut gains: Vec<(Entity, f32)> = Vec::new();
    for t in taken.read() {
        let d = &t.damage;
        if d.kind == DamageKind::Burn || d.amount <= 0.0 {
            continue;
        }
        let Some((p, weapon)) = item(d) else { continue };
        let a = d.attacker.expect("an attacker");
        if p.heal_on_hit > 0.0 {
            gains.push((a, p.heal_on_hit.max(ON_HIT_MIN)));
        }
        if p.ignite && health.get(d.target).is_ok_and(|h| h.current > 0.0) {
            let burn = crate::game::weapons::flame::ignited(clock.now, a, weapon, flame.get_or_insert_with(crate::game::weapons::flame::stock_params));
            commands.entity(d.target).insert(burn);
        }
    }
    for k in died.read() {
        if let Some((p, _)) = item(&k.damage).filter(|(p, _)| p.heal_on_kill > 0.0) {
            gains.push((k.damage.attacker.expect("an attacker"), p.heal_on_kill));
        }
    }
    for (e, points) in gains {
        if let Ok(mut h) = health.get_mut(e)
            && h.current > 0.0
            && h.current < h.max
        {
            h.current = (h.current + points / 100.0).min(h.max);
        }
    }
}

/// The Bread Bite's max-health floor and deploy cost (points; 4.10).
pub const DRAIN_FLOOR: f32 = 100.0;
pub const DRAIN_DEPLOY: f32 = 20.0;

/// Each holder's max health drained by an item (points), and whether the
/// draining item was out last tick: a resource, not a part (no archetype
/// per holder).
#[derive(Resource, Clone, Debug, Default)]
pub struct ItemDrains(pub bevy::platform::collections::HashMap<Entity, (f32, bool)>);

impl ItemDrains {
    /// What `e`'s max health is lowered by (points).
    pub fn of(&self, e: Entity) -> f32 {
        self.0.get(&e).map_or(0.0, |d| d.0)
    }
}

/// The drain's tick (module doc), before `movement::class_health`.
pub fn drain(
    holders: Query<(Entity, &Inventory, &Health, &crate::game::classes::Tf2Class)>,
    items: Query<&Tf2Item>,
    table: Res<crate::game::classes::Classes>,
    time: Res<Time<Fixed>>,
    mut drains: ResMut<ItemDrains>,
) {
    let dt = time.delta_secs();
    drains.0.retain(|e, _| holders.contains(*e));
    for (e, inv, h, class) in &holders {
        let rate = inv.active.and_then(|w| items.get(w).ok()).map_or(0.0, |i| i.0.drain);
        let entry = drains.0.get(&e).copied();
        if rate <= 0.0 && entry.is_none() {
            continue;
        }
        let (mut drained, was_out) = entry.unwrap_or((0.0, false));
        let room = (class.class(&table).health - DRAIN_FLOOR).max(0.0);
        if h.current <= 0.0 {
            drained = 0.0;
        } else if rate > 0.0 {
            if !was_out && drained <= 0.0 {
                drained += DRAIN_DEPLOY;
            }
            drained += rate * dt;
        } else {
            drained -= drains_back(entry) * dt;
        }
        let drained = drained.clamp(0.0, room);
        if drained <= 0.0 && rate <= 0.0 {
            drains.0.remove(&e);
        } else {
            drains.0.insert(e, (drained, rate > 0.0));
        }
    }
}

/// The refill's rate: the drain's (10/s, the only one there is).
fn drains_back(_: Option<(f32, bool)>) -> f32 {
    10.0
}

/// How long a weapon must have been out for its holster multiplier to
/// count (4.9).
pub const HOLSTER_AFTER: f64 = 0.5;

/// The sword's switch factor, either side (4.9).
pub const SWORD_SWITCH: f32 = 1.75;

/// When a holder's weapon in hand was deployed (sim time).
#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub struct DeployedAt(pub f64);

/// The deploy's factor for switching from `from` (out for `out` s) to
/// `to` (module doc, "Switching").
pub fn switch_factor(from: Option<&ItemParams>, out: f64, to: Option<&ItemParams>) -> f32 {
    let Some(from) = from else { return 1.0 };
    let mut f = 1.0;
    if out > HOLSTER_AFTER {
        f *= from.holster;
    }
    if from.sword && !to.is_some_and(|t| t.sword) {
        f *= SWORD_SWITCH;
    }
    f
}

/// Each deploy longer by the weapon switched from (module doc,
/// "Switching"), after the switch.
pub fn holster(
    mut events: MessageReader<chimera_sim::weapon::WeaponEvent>,
    mut holders: Query<(&mut Inventory, Option<&mut DeployedAt>)>,
    mut weapons: Query<(&Weapon, &mut chimera_sim::weapon::WeaponState, Option<&Tf2Item>)>,
    clock: Res<SimClock>,
    mut commands: Commands,
) {
    let now = clock.now;
    for e in events.read() {
        if !matches!(e.kind, chimera_sim::weapon::WeaponEventKind::Deployed) {
            continue;
        }
        let Ok((mut inv, at)) = holders.get_mut(e.owner) else { continue };
        let out = at.as_ref().map_or(0.0, |a| now - a.0);
        match at {
            Some(mut a) => a.0 = now,
            None => {
                commands.entity(e.owner).insert(DeployedAt(now));
            }
        }
        let from = inv.last.and_then(|w| weapons.get(w).ok()).and_then(|(_, _, i)| i.map(|i| i.0.clone()));
        let Ok((w, mut st, to)) = weapons.get_mut(e.weapon) else { continue };
        let f = switch_factor(from.as_ref(), out, to.map(|t| &t.0));
        if f == 1.0 {
            continue;
        }
        let ready = now + (w.draw_time * f) as f64;
        inv.next_attack = inv.next_attack.max(ready);
        st.next_primary = st.next_primary.max(ready);
        st.next_secondary = st.next_secondary.max(ready);
    }
}

/// `tf2:item`'s hook and rules.
pub fn add(app: &mut App) {
    use chimera_sim::weapon::data::WeaponDataAppExt;
    app.register_weapon_hook::<ItemParams>(super::ITEM_HOOK, |e, _, p| {
        e.insert(Tf2Item(p.clone()));
    })
    .init_resource::<ItemDrains>()
    .add_systems(
        FixedUpdate,
        (
            taken.after(crate::core::SimSet::Weapons).before(crate::core::apply_damage),
            heals.after(crate::core::apply_damage),
            drain.after(crate::core::SimSet::Movement).before(crate::game::movement::class_health),
        )
            .run_if(crate::core::authoritative),
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_switch_factor_follows_4_9() {
        let fists = ItemParams { holster: 2.0, ..default() };
        let sword = ItemParams { sword: true, ..default() };
        assert_eq!(switch_factor(Some(&fists), 0.6, None), 2.0, "Fists of Steel: 1.0 s");
        assert_eq!(switch_factor(Some(&fists), 0.4, None), 1.0, "out 0.5 s or less: not counted");
        assert_eq!(switch_factor(Some(&sword), 0.1, None), 1.75, "from a sword, always");
        assert_eq!(switch_factor(Some(&sword), 0.1, Some(&sword)), 1.0, "sword to sword: its own draw has it");
        assert_eq!(switch_factor(None, 9.0, Some(&fists)), 1.0);
    }

    #[test]
    fn damage_taken_folds_in_the_spec_order() {
        let f = Factors { taken: 1.3, ..default() };
        assert!((f.taken_points(50.0, DamageKind::Generic, false, 300.0, 300.0) - 65.0).abs() < 1e-4);
        let steel = Factors { taken_ranged: 0.6, taken_melee: 2.0, ..default() };
        assert_eq!(steel.taken_points(50.0, DamageKind::Melee, false, 300.0, 300.0), 100.0);
        assert!((steel.taken_points(50.0, DamageKind::Bullet, false, 300.0, 300.0) - 30.0).abs() < 1e-4);
        // Brass Beast spun: x0.8 only when the hit leaves him at half or less.
        let beast = Factors { spun_resist: 0.8, ..default() };
        assert_eq!(beast.taken_points(100.0, DamageKind::Bullet, true, 180.0, 300.0), 80.0);
        assert_eq!(beast.taken_points(100.0, DamageKind::Bullet, true, 300.0, 300.0), 100.0);
        assert_eq!(beast.taken_points(100.0, DamageKind::Bullet, false, 180.0, 300.0), 100.0);
    }
}
