//! Combat Arms' readers for the gun game's power curve
//! (`chimera_sim::minigame::power`, `PowerAppExt::add_power_reader`): what
//! its hooks' numbers mean for a gun's stats at a distance, so its guns
//! score on the same terms as any game's.
//!
//! - `combat_arms:damage`: the ray's damage (`InstDamage`, the document's
//!   damage units) times the range band at the distance (`band_scale`'s
//!   rule: the first band whose edge reaches it, beyond the last 1), and
//!   `fireDelay` as the least time between shots.
//! - `combat_arms:spread`: the cone, `DynamicPerturbFactor` times the
//!   minimum (the first shot) and the minimum plus the curve's `firing`
//!   share of the way to the maximum (the follow-ups), in thousandths of
//!   the distance (a square's half side, read as a disc's radius); a sniper's or marksman rifle's zoomed cone, as it's fired.
//! - `combat_arms:inventory`: the fire animation's rate (1 +
//!   `FireAniRate`) dividing the cycle.
//! - `combat_arms:projectile`: an impact projectile's area damage (a
//!   direct hit's), or its lasting sphere's burn per second over the
//!   cycle; its cone (`perturb`).
//! - `combat_arms:bolt`: a crossbow bolt's damage table (as
//!   `combat_arms:damage`) and its cone (`perturb`).
//! - `combat_arms:flame`: the pre-fire before the first flame.
//! - `combat_arms:device`: a turret's gun (`turret.gun`, read as
//!   `combat_arms:damage`), as the weapon's damage: the target at the
//!   distance, but no farther than the turret's sensor (`search_radius`:
//!   it fights what walks into it; the gun's range caps it too), its
//!   `perturb` a fixed offset at any range (a spread of perturb /
//!   distance), one shot per `fire_delay` over the curve's
//!   `device_uptime` (the share of the time it fires at the player
//!   counted), and the placing (`prefire` + `key`) and `setup` as the
//!   latency. Its `ammo` caps nothing (a kill takes fewer rounds).

use chimera_core::game_parts::GunClass;
use chimera_sim::minigame::power::{GunStats, PowerAppExt, ReadAt};

fn num(v: &toml::Value, key: &str) -> Option<f32> {
    let x = v.get(key)?;
    x.as_float().map(|f| f as f32).or_else(|| x.as_integer().map(|i| i as f32))
}

fn nums(v: &toml::Value, key: &str) -> Vec<f32> {
    v.get(key)
        .and_then(|a| a.as_array())
        .map(|a| a.iter().filter_map(|x| x.as_float().map(|f| f as f32).or_else(|| x.as_integer().map(|i| i as f32))).collect())
        .unwrap_or_default()
}

fn damage(p: &toml::Value, at: &ReadAt, s: &mut GunStats) {
    let Some(base) = num(p, "damage") else { return };
    let edges = nums(p, "effect_range");
    let scales = nums(p, "range_damage");
    let band = if edges.len() >= 3 && scales.len() >= 3 {
        edges.iter().zip(&scales).find(|(e, _)| at.distance <= e.trunc() + 1e-3).map_or(1.0, |(_, m)| *m)
    } else {
        1.0
    };
    s.damage = base * at.data.units.damage * band;
    if let Some(ms) = num(p, "fire_delay") {
        s.interval = s.interval.max(ms / 1000.0);
    }
}

/// A cone's first and follow-up spread (`firing` into it), per unit of
/// distance.
fn cone(s: &mut GunStats, min: f32, max: f32, factor: f32, firing: f32) {
    s.spread = factor * min / 1000.0;
    s.follow = Some(factor * (min + (max - min).max(0.0) * firing) / 1000.0);
}

fn spread(p: &toml::Value, at: &ReadAt, s: &mut GunStats) {
    // A sniper or marksman rifle is fired zoomed (its unzoomed cone is a
    // fixed wide one: weapon_classes.md 6).
    let scoped = matches!(at.data.class, Some(GunClass::Sniper | GunClass::Marksman));
    let Some(c) = p.get(if scoped && p.get("zoomed_cone").is_some() { "zoomed_cone" } else { "cone" }) else { return };
    let (Some(min), Some(max)) = (num(c, "min"), num(c, "max")) else { return };
    cone(s, min, max, num(p, "dynamic_factor").unwrap_or(1.0), at.firing);
}

fn inventory(p: &toml::Value, _: &ReadAt, s: &mut GunStats) {
    let rate = super::super::inventory::played_rate(1.0 + num(p, "fire_rate").unwrap_or(0.0));
    if rate > 0.0 {
        s.interval /= rate;
    }
}

fn projectile(p: &toml::Value, at: &ReadAt, s: &mut GunStats) {
    let unit = at.data.units.damage;
    if let Some(area) = num(p, "area_damage").filter(|a| *a > 0.0) {
        s.damage = area * unit;
    } else if let Some(rate) = p.get("over_time").and_then(|o| num(o, "rate")) {
        s.damage = rate * unit * s.interval;
    }
    let perturb = nums(p, "perturb");
    if let [min, max, ..] = perturb[..] {
        cone(s, min, max, num(p, "dynamic_perturb").unwrap_or(1.0), at.firing);
    }
}

fn bolt(p: &toml::Value, at: &ReadAt, s: &mut GunStats) {
    if let Some(d) = p.get("damage") {
        damage(d, at, s);
    }
    if let [min, max, ..] = nums(p, "perturb")[..] {
        cone(s, min, max, num(p, "dynamic_perturb").unwrap_or(1.0), at.firing);
    }
}

fn flame(p: &toml::Value, _: &ReadAt, s: &mut GunStats) {
    s.latency += num(p, "prefire").unwrap_or(0.0);
}

fn device(p: &toml::Value, at: &ReadAt, s: &mut GunStats) {
    let Some(t) = p.get("turret") else { return };
    let Some(gun) = t.get("gun") else { return };
    let reach = [num(t, "search_radius"), num(t, "range")].into_iter().flatten().filter(|r| *r > 0.0).fold(f32::INFINITY, f32::min);
    let distance = at.distance.min(reach).max(1e-3);
    let near = ReadAt { distance, ..*at };
    s.interval = 1.0;
    damage(gun, &near, s);
    s.pellets = 1;
    s.follow = None;
    s.spread = num(t, "perturb").unwrap_or(0.0) / distance;
    if let Some(delay) = num(t, "fire_delay").filter(|d| *d > 0.0) {
        s.interval = delay / at.uptime.clamp(1e-3, 1.0);
    }
    s.latency = num(p, "prefire").unwrap_or(0.0) + num(p, "key").unwrap_or(0.0) + num(t, "setup").unwrap_or(0.0);
    s.clip = 0;
    s.reload = 0.0;
}

/// Combat Arms' readers registered.
pub fn plugin(app: &mut bevy::prelude::App) {
    // In this order: the rate before the damage's least interval and the
    // burn's per-shot share.
    app.add_power_reader(super::super::inventory::ITEM_HOOK, inventory)
        .add_power_reader(super::DAMAGE, damage)
        .add_power_reader(super::SPREAD, spread)
        .add_power_reader(super::super::special::data::PROJECTILE, projectile)
        .add_power_reader(super::super::special::data::BOLT, bolt)
        .add_power_reader(super::super::special::data::FLAME, flame)
        .add_power_reader(super::super::special::data::DEVICE, device);
}
