//! An item's weapon document (specs/tf2/item_attributes.md 2, 4): its
//! base weapon's document (data/items.toml `[bases]`, with the stock
//! script's numbers already applied: `weapons::script_numbers`), then
//! the item class's own script numbers from the pack (`ItemRecord::base`:
//! damage, interval, clip), then every attribute the table maps onto a
//! weapon number, integers rounded once at the end:
//!
//! - damage (`mult_dmg` × `mult_dmg_vs_players`: no buildings yet) on the
//!   bullets' `[damage] amount`, each swing's `damage`, the blast hooks'
//!   `damage` (the owner's own blast keeps the class's: `self_damage`,
//!   §3.3), the flame's and the needle's;
//! - clip (`mult_clipsize`), reserve (`mult_maxammo_primary`/`secondary`
//!   by slot: an ammo type of the item's own), interval
//!   (`mult_postfiredelay`: `[trigger] cycle`, the swings' refires),
//!   spread (`mult_spread_scale`);
//! - projectiles: speed (`mult_projectile_speed`, not the grenade's 200
//!   up), blast radius vs others (`mult_explosion_radius`; the owner's
//!   `self_radius` kept), the grenade's fuse (`fuse_mult`: the flyer's
//!   life, the first 14-tick check past 2.0 × f, `fuse_life`), no bounce
//!   (`grenade_no_bounce`: the first world touch keeps 10 %), shatter
//!   (`set_detonate_mode` 2), no spin, centre fire (`centerfire_projectile`:
//!   no side offset);
//! - stickies: arm (`sticky_arm_time`, added), charge time
//!   (`stickybomb_charge_rate`), charge damage
//!   (`stickybomb_charge_damage_increase`), live limit (`add_max_pipebombs`);
//! - own blasts: `rocket_jump_dmg_reduction` (`self_scale`, when the blast
//!   hurt no enemy), `no_self_blast_dmg` (`self_harm` off: still pushed);
//! - the Minigun: wind-up (`mult_minigun_spinup_time`), spun speed
//!   (`mult_player_aiming_movespeed`);
//! - swords (`is_a_sword`): swing range 72, deploy ×1.75; the swing's
//!   range and box (`melee_range_multiplier`, `melee_bounds_multiplier`);
//!   its own deploy (`mult_single_wep_deploy_time`);
//! - the flare (`tf2:flare`, batch2_mechanics.md 4): the class script's
//!   damage as its hit and its owner's own base, × the damage attributes
//!   (the owner's base kept), `blast_dmg_to_self` as `self_scale`,
//!   `mult_dmgself_push_force` on its own push, `set_weapon_mode` 1 the
//!   Detonator's, 3 the Scorch Shot's;
//! - batch 2b (batch2b_mechanics.md): a bullet gun's pellets (the class
//!   script's × `mult_bullets_per_shot`), reload time
//!   (`mult_reload_time`, `_hidden`), a whole-clip reload
//!   (`set_scattergun_no_reload_single`), an energy weapon's own clip
//!   (`energy_weapon_no_ammo`), a bat's meter (`effectbar_recharge_rate`
//!   on `tf2:throw`); the projectiles' own hits (`tf2:shot`) are their
//!   class's, untouched;
//! - batch 2b (batch2b_mechanics.md): the sniper rifles' variants on
//!   `tf2:sniper` (`mult_sniper_charge_per_sec`, `sniper_full_charge_
//!   damage_bonus`, `sniper_penetrate_players_when_charged`,
//!   `sniper_only_fire_zoomed`, `bodyshot_damage_modify`, `jarate_duration`,
//!   the Classic's class); a consumable's base table for its mode and
//!   class merged, `item_meter_charge_rate`, `lunchbox_healing_scale`,
//!   `energy_buff_dmg_taken_multiplier`, `mod_mark_attacker_for_death`,
//!   its label its name, a jar's projectile its own model, its arms its
//!   class's; a meter (`super::meters::MeterParams::of`) as `tf2:meter`;
//! - the rest that modifies the player (speed, jump, damage taken, heals,
//!   max-health drain, ignite) goes to the item's `tf2:item` hook
//!   (`super::mods::ItemParams`).
//!
//! Its looks (`looks`, the record's `model` and `visuals`): its own
//! model (`model_player`, or its class's of `model_player_per_class`) as
//! `[models] view` and `world`, its kill icon (`item_iconname`) as
//! `[hud] icon`, the sound entries `visuals` names in place of its
//! base's (`SOUNDS`: the shot, the Minigun's shot and crit shot, empty,
//! the swings' hit, world hit and miss, the crit swing), and on its
//! `tf2:item` hook its base (`super::base_in`) and skin.
//!
//! The ladder may draw it (`class` kept) when it deals damage, unless
//! data/items.toml's `gun_game` says otherwise.

use toml::{Table, Value};

use super::convert::{ItemRecord, constants};
use super::mods::ItemParams;

/// The grenade's fuse checks: every 14 ticks (0.21 s, weapons_more.md 6;
/// item_attributes.md 4.2).
pub const FUSE_CHECK: f64 = 14.0 * 0.015;

/// What the first world touch of a no-bounce grenade keeps (4.5).
pub const NO_BOUNCE_KEEP: f64 = 0.1;

/// A grenade fused at `fuse` s goes off at the first check past it.
pub fn fuse_life(fuse: f64) -> f64 {
    let checks = (fuse / FUSE_CHECK - 1e-9).ceil().max(1.0);
    ((checks * FUSE_CHECK) * 1e6).round() / 1e6
}

fn table<'a>(doc: &'a mut Table, key: &str) -> &'a mut Table {
    let v = doc.entry(key.to_string()).or_insert_with(|| Value::Table(Table::new()));
    if !v.is_table() {
        *v = Value::Table(Table::new());
    }
    v.as_table_mut().expect("a table")
}

fn num(v: &Value) -> Option<f64> {
    v.as_float().or_else(|| v.as_integer().map(|i| i as f64))
}

/// Scale number `key` of `t` by `f` (absent: nothing).
fn scale(t: &mut Table, key: &str, f: f64) {
    if f == 1.0 {
        return;
    }
    if let Some(x) = t.get(key).and_then(num) {
        t.insert(key.into(), Value::Float(x * f));
    }
}

fn hook<'a>(doc: &'a mut Table, name: &str) -> Option<&'a mut Table> {
    doc.get_mut("hooks")?.as_table_mut()?.get_mut(name)?.as_table_mut()
}

/// The blast hooks a weapon may carry.
const BLASTS: &[&str] = &[crate::game::weapons::ROCKET_HOOK, crate::game::weapons::GRENADE_HOOK, crate::game::weapons::STICKY_HOOK];

/// The ammo types' maxima (data/weapons/ammo.toml).
fn ammo_max(ammo: &str) -> Option<i64> {
    let doc: Table = crate::game::weapons::DATA.iter().find(|(n, _)| *n == "ammo.toml")?.1.text().parse().ok()?;
    doc.get("ammo")?.get(ammo)?.get("max")?.as_integer()
}

/// The item's document: `base` (its base weapon's, parsed) with `r`'s
/// numbers, named `id`, called `name`; `gun_game` the data's override.
pub fn item_doc(mut doc: Table, id: &str, name: &str, base_id: &str, r: &ItemRecord, gun_game: Option<bool>) -> Table {
    let a = |c: &str| r.attr(c, 1.0);
    let add = |c: &str| r.attr(c, 0.0);
    doc.insert("id".into(), Value::String(id.into()));
    doc.remove("abstract");
    table(&mut doc, "hud").insert("name".into(), Value::String(name.into()));
    let melee = doc.get("melee").is_some_and(Value::is_table);
    // The game ignores the Sniper Rifle's script damage and the flame's
    // damage and interval (`script_numbers::apply_script`).
    let sniper = hook(&mut doc, crate::game::weapons::SNIPER_HOOK).is_some();
    let flame = hook(&mut doc, crate::game::weapons::FLAME_HOOK).is_some();

    // The item class's own script (its base numbers).
    let base = |k: &str| r.base.get(k).copied().filter(|x| *x > 0.0);
    if melee {
        let swings: Vec<String> = table(&mut doc, "melee").keys().cloned().collect();
        for s in swings {
            let t = table(table(&mut doc, "melee"), &s);
            if let Some(d) = base("damage") {
                t.insert("damage".into(), Value::Float(d));
            }
            if let Some(i) = base("interval") {
                for k in ["hit_refire", "miss_refire", "miss_refire_other"] {
                    t.insert(k.into(), Value::Float(i));
                }
            }
        }
    } else {
        if let Some(d) = base("damage").filter(|_| !sniper && !flame) {
            let blast = BLASTS.iter().find(|h| hook(&mut doc, h).is_some()).copied();
            if let Some(t) = hook(&mut doc, crate::game::weapons::FLARE_HOOK) {
                // The flare: its hit and its owner's own base (4.3).
                t.insert("damage".into(), Value::Float(d.trunc()));
                t.insert("self_damage".into(), Value::Float(d.trunc()));
            } else {
            match blast {
                Some(h) => {
                    let t = hook(&mut doc, h).expect("the hook");
                    t.insert("damage".into(), Value::Float(d.trunc()));
                    t.insert("self_damage".into(), Value::Float(d.trunc()));
                }
                None => {
                    if let Some(t) = doc.get_mut("damage").and_then(Value::as_table_mut) {
                        t.insert("amount".into(), Value::Float(d.trunc()));
                    }
                }
            }
            }
        }
        if let Some(i) = base("interval").filter(|_| !flame)
            && let Some(t) = doc.get_mut("trigger").and_then(Value::as_table_mut)
        {
            t.insert("cycle".into(), Value::Float(i));
        }
        // An energy weapon's cell is its clip (batch2b_mechanics.md 6: 4
        // shots of 5), not the script's.
        if let Some(c) = base("clip").filter(|_| !r.flag("energy_weapon_no_ammo"))
            && let Some(t) = doc.get_mut("magazine").and_then(Value::as_table_mut)
            && t.contains_key("clip")
        {
            t.insert("clip".into(), Value::Integer(c as i64));
        }
    }

    // Damage: every path but the owner's own blast.
    let dmg = a("mult_dmg") * a("mult_dmg_vs_players");
    if let Some(t) = doc.get_mut("damage").and_then(Value::as_table_mut) {
        scale(t, "amount", dmg);
    }
    let mut swing_damage: Option<f64> = None;
    if melee {
        let swings: Vec<String> = table(&mut doc, "melee").keys().cloned().collect();
        for s in swings {
            let t = table(table(&mut doc, "melee"), &s);
            scale(t, "damage", dmg);
            if s == "primary" || swing_damage.is_none() {
                swing_damage = t.get("damage").and_then(num);
            }
            // Whole points, halves up (§3.2 step 10): a swing's damage has
            // nothing after it, and the frame's quantum truncates.
            if let Some(d) = t.get("damage").and_then(num) {
                t.insert("damage".into(), Value::Float((d + 1e-6).round()));
            }
            for k in ["hit_refire", "miss_refire", "miss_refire_other"] {
                scale(t, k, a("mult_postfiredelay"));
            }
            if r.flag("is_a_sword") {
                t.insert("range".into(), Value::Float(constants::SWORD_RANGE));
            }
            // Reach and box (batch2_mechanics.md 5.4).
            scale(t, "range", a("melee_range_multiplier"));
            scale(t, "hull", a("melee_bounds_multiplier"));
        }
    }
    for h in BLASTS {
        if let Some(t) = hook(&mut doc, h) {
            if !t.contains_key("self_damage")
                && let Some(d) = t.get("damage").cloned()
            {
                t.insert("self_damage".into(), d);
            }
            if !t.contains_key("self_radius")
                && let Some(d) = t.get("radius").cloned()
            {
                t.insert("self_radius".into(), d);
            }
            scale(t, "damage", dmg);
            scale(t, "radius", a("mult_explosion_radius"));
            let reduction = a("rocket_jump_dmg_reduction");
            if reduction != 1.0 {
                t.insert("self_scale".into(), Value::Float(reduction));
            }
            if r.flag("no_self_blast_dmg") {
                t.insert("self_harm".into(), Value::Boolean(false));
            }
        }
    }
    for h in [crate::game::weapons::FLAME_HOOK, crate::game::weapons::NEEDLE_HOOK] {
        if let Some(t) = hook(&mut doc, h) {
            scale(t, "damage", dmg);
        }
    }
    // The flare (batch2_mechanics.md 4): its damage (the owner's own base
    // kept), `blast dmg to self`, its own push, its mode.
    if let Some(t) = hook(&mut doc, crate::game::weapons::FLARE_HOOK) {
        scale(t, "damage", dmg);
        if a("blast_dmg_to_self") != 1.0 {
            t.insert("self_scale".into(), Value::Float(a("blast_dmg_to_self")));
        }
        scale(t, "push_self", a("mult_dmgself_push_force"));
        let mode = match add("set_weapon_mode") as i64 {
            1 => Some("detonator"),
            3 => Some("scorch"),
            _ => None,
        };
        if let Some(m) = mode {
            t.insert("mode".into(), Value::String(m.into()));
        }
    }

    // Clip, reserve, interval, spread.
    if let Some(m) = doc.get_mut("magazine").and_then(Value::as_table_mut)
        && let Some(c) = m.get("clip").and_then(num)
        && a("mult_clipsize") != 1.0
    {
        m.insert("clip".into(), Value::Integer((c * a("mult_clipsize")).round().max(1.0) as i64));
    }
    let slot = doc.get("slot").and_then(Value::as_integer).unwrap_or(0);
    let reserve = if slot == 0 { a("mult_maxammo_primary") } else { a("mult_maxammo_secondary") };
    if reserve != 1.0
        && let Some(m) = doc.get_mut("magazine").and_then(Value::as_table_mut)
        && let Some(ammo) = m.get("ammo").and_then(Value::as_str).map(str::to_string)
        && let Some(max) = ammo_max(&ammo)
    {
        let own = format!("{ammo}@{}", r.index);
        m.insert("ammo".into(), Value::String(own.clone()));
        let mut t = Table::new();
        t.insert("max".into(), Value::Integer((max as f64 * reserve).round() as i64));
        table(&mut doc, "ammo").insert(own, Value::Table(t));
    }
    if let Some(t) = doc.get_mut("trigger").and_then(Value::as_table_mut) {
        scale(t, "cycle", a("mult_postfiredelay"));
    }
    // A bullet gun's pellets: the class script's × `mult_bullets_per_shot`
    // (the Shortstop's 4, the Force-a-Nature's 12; a projectile's rounds
    // stay one).
    if doc.get("damage").is_some_and(Value::is_table)
        && let Some(h) = doc.get_mut("hitscan").and_then(Value::as_table_mut)
    {
        let pellets = base("bullets").or_else(|| h.get("pellets").and_then(num));
        if let Some(n) = pellets {
            let n = (n * a("mult_bullets_per_shot")).round().max(1.0);
            h.insert("pellets".into(), Value::Integer(n as i64));
        }
    }
    if let Some(s) = doc.get_mut("hitscan").and_then(|h| h.get_mut("spread")).and_then(Value::as_table_mut) {
        scale(s, "template", a("mult_spread_scale"));
    }
    // Reloads: their time (`mult_reload_time`, `mult_reload_time_hidden`),
    // the whole clip at once (`set_scattergun_no_reload_single`).
    if let Some(m) = doc.get_mut("magazine").and_then(Value::as_table_mut) {
        let f = a("mult_reload_time") * a("mult_reload_time_hidden");
        scale(m, "reload", f);
        if let Some(sh) = m.get_mut("shells").and_then(Value::as_table_mut) {
            scale(sh, "start", f);
            scale(sh, "insert", f);
        }
        if r.flag("set_scattergun_no_reload_single") {
            m.remove("shells");
        }
    }
    // A bat's meter (`effectbar_recharge_rate`, batch2b_mechanics.md 4).
    if let Some(t) = hook(&mut doc, crate::game::weapons::shot::THROW_HOOK) {
        scale(t, "recharge", a("effectbar_recharge_rate"));
    }

    // Projectiles.
    if let Some(t) = hook(&mut doc, crate::game::weapons::PROJECTILE_HOOK) {
        scale(t, "speed", a("mult_projectile_speed"));
        if a("fuse_mult") != 1.0 && t.contains_key("life") {
            t.insert("life".into(), Value::Float(fuse_life(constants::GRENADE_FUSE * a("fuse_mult"))));
        }
        if r.flag("grenade_no_spin") {
            t.remove("spin");
        }
        if r.flag("centerfire_projectile") {
            for k in ["offset", "offset_crouched"] {
                if let Some(Value::Array(o)) = t.get_mut(k)
                    && o.len() == 3
                {
                    o[1] = Value::Float(0.0);
                }
            }
        }
    }
    if let Some(t) = hook(&mut doc, crate::game::weapons::GRENADE_HOOK) {
        if r.flag("grenade_no_bounce") {
            t.insert("first_touch_keep".into(), Value::Float(NO_BOUNCE_KEEP));
        }
        if add("set_detonate_mode") == 2.0 {
            t.insert("shatter".into(), Value::Boolean(true));
        }
    }
    if let Some(t) = hook(&mut doc, crate::game::weapons::STICKY_HOOK) {
        if add("sticky_arm_time") != 0.0 {
            let old = t.get("sticky_arm_time").and_then(num).unwrap_or(0.0);
            t.insert("sticky_arm_time".into(), Value::Float(old + add("sticky_arm_time")));
        }
        scale(t, "charge_time", a("stickybomb_charge_rate"));
        let f = a("stickybomb_charge_damage_increase");
        if f != 1.0 {
            t.insert("charge_damage".into(), Value::Float(f));
        }
        if add("add_max_pipebombs") != 0.0
            && let Some(m) = t.get("max").and_then(num)
        {
            t.insert("max".into(), Value::Integer((m + add("add_max_pipebombs")).round().max(1.0) as i64));
        }
    }
    if let Some(t) = hook(&mut doc, crate::game::weapons::MINIGUN_HOOK) {
        scale(t, "wind_up", a("mult_minigun_spinup_time"));
        scale(t, "spun_speed", a("mult_player_aiming_movespeed"));
    }
    if r.flag("is_a_sword") {
        let d = doc.get("draw").and_then(num).unwrap_or(constants::SWITCH);
        doc.insert("draw".into(), Value::Float(d * constants::SWORD_SWITCH));
    }
    // Its own deploy (`mult_single_wep_deploy_time`: the Atomizer's 1.5).
    if a("mult_single_wep_deploy_time") != 1.0 {
        let d = doc.get("draw").and_then(num).unwrap_or(constants::SWITCH);
        doc.insert("draw".into(), Value::Float(d * a("mult_single_wep_deploy_time")));
    }

    // The sniper rifle variants (batch2b_mechanics.md 15).
    if let Some(t) = hook(&mut doc, crate::game::weapons::SNIPER_HOOK) {
        let set = |t: &mut Table, k: &str, v: Value| {
            t.insert(k.into(), v);
        };
        if a("mult_sniper_charge_per_sec") != 1.0 {
            set(t, "charge_scale", Value::Float(a("mult_sniper_charge_per_sec")));
        }
        if a("sniper_full_charge_damage_bonus") != 1.0 {
            set(t, "full_bonus", Value::Float(a("sniper_full_charge_damage_bonus")));
        }
        if r.flag("sniper_penetrate_players_when_charged") {
            set(t, "pierce_full", Value::Boolean(true));
        }
        if r.flag("sniper_only_fire_zoomed") {
            set(t, "only_scoped", Value::Boolean(true));
        }
        if r.class == "tf_weapon_sniperrifle_classic" {
            set(t, "classic", Value::Boolean(true));
        }
        if a("bodyshot_damage_modify") != 1.0 {
            set(t, "body_scale", Value::Float(a("bodyshot_damage_modify")));
        }
        if add("jarate_duration") > 0.0 {
            set(t, "head_mini", Value::Boolean(true));
            set(t, "jarate", Value::Float(add("jarate_duration")));
        }
    }
    // The consumables (batch2b_mechanics.md 11-13, `super::consumables`):
    // the base's table for the item's mode and class merged, then its
    // attributes; a thrown jar shows the item's own model; the arms are
    // its class's.
    if let Some(t) = hook(&mut doc, super::consumables::HOOK) {
        let merge = |t: &mut Table, from: &str, key: &str| {
            let picked = t.get(from).and_then(|m| m.get(key)).and_then(Value::as_table).cloned();
            for (k, v) in picked.into_iter().flatten() {
                t.insert(k, v);
            }
        };
        merge(t, "modes", &format!("{}", add("set_weapon_mode") as i64));
        merge(t, "by_class", &r.class);
        t.remove("modes");
        t.remove("by_class");
        if add("item_meter_charge_rate") > 0.0 {
            t.insert("recharge".into(), Value::Float(add("item_meter_charge_rate")));
        }
        if a("lunchbox_healing_scale") != 1.0 {
            let h = t.get("heal").and_then(num).unwrap_or(0.0);
            t.insert("heal".into(), Value::Float((h * a("lunchbox_healing_scale")).round()));
        }
        if a("energy_buff_dmg_taken_multiplier") != 1.0 {
            t.insert("taken_buffed".into(), Value::Float(a("energy_buff_dmg_taken_multiplier")));
        }
        if add("mod_mark_attacker_for_death") > 0.0 {
            t.insert("mark_attacker".into(), Value::Float(add("mod_mark_attacker_for_death")));
        }
        t.insert("label".into(), Value::String(name.to_uppercase()));
    }
    if hook(&mut doc, super::consumables::HOOK).is_some() {
        if let Some(p) = hook(&mut doc, crate::game::weapons::PROJECTILE_HOOK) {
            p.insert("shown".into(), Value::String(thrown_key(id)));
        }
        if let [class] = r.classes.as_slice()
            && let Some(g) = hook(&mut doc, crate::game::weapons::GUN_HOOK)
        {
            g.insert("arms".into(), Value::String(if class == "demoman" { "demo".into() } else { class.clone() }));
        }
    }
    // The item meters (batch2b_mechanics.md 14, `super::meters`).
    if let Some(m) = super::meters::MeterParams::of(r)
        && let Ok(Value::Table(t)) = Value::try_from(&m)
    {
        table(&mut doc, "hooks").insert(super::meters::HOOK.into(), Value::Table(t));
    }

    looks(&mut doc, r);

    // The player's numbers, its base and skin.
    let mut params = ItemParams::of(r);
    params.base = base_id.to_string();
    // A swing's damage before rounding (the Axtinguisher's and Equalizer's
    // laws, the Boston Basher's half).
    if let Some(d) = swing_damage {
        params.base_damage = d as f32;
    }
    let t: Table = Value::try_from(&params).ok().and_then(|v| v.as_table().cloned()).unwrap_or_default();
    table(&mut doc, "hooks").insert(super::ITEM_HOOK.into(), Value::Table(t));

    // The ladder: a damaging gun of a ladder class.
    let deals = dmg > 0.0;
    if !gun_game.unwrap_or(deals) {
        doc.remove("class");
    }
    doc
}

/// The model key of item `id`'s thrown projectile (a jar: its own model,
/// `maps::add_guns_with`).
pub fn thrown_key(id: &str) -> String {
    format!("{id}/thrown")
}

/// Where each `visuals` sound kind goes: (kind, place). A place is
/// `sounds.<key>` (the document's `[sounds]`, where it has the key),
/// `hook.<hook>.<key>` (a hook it has), or `swings.<key>` (every swing
/// of `[melee]`).
pub const SOUNDS: &[(&str, &str)] = &[
    ("single_shot", "sounds.fire"),
    ("empty", "sounds.empty"),
    ("single_shot", "hook.tf2:minigun.fire_sound"),
    ("burst", "hook.tf2:minigun.fire_crit_sound"),
    ("empty", "hook.tf2:minigun.empty_sound"),
    ("melee_miss", "swings.sound_swing"),
    ("melee_hit", "swings.sound_hit"),
    ("melee_hit_world", "swings.sound_hit_world"),
    ("burst", "hook.tf2:melee.crit_swing"),
];

/// The record's looks onto `doc` (module doc).
fn looks(doc: &mut Table, r: &ItemRecord) {
    let v = &r.visuals;
    let arms = [crate::game::weapons::GUN_HOOK, crate::game::weapons::MELEE_HOOK]
        .iter()
        .find_map(|h| hook(doc, h)?.get("arms")?.as_str().map(str::to_string));
    let model = arms
        .as_ref()
        .and_then(|a| v.per_class.get(a))
        .or(r.model.as_ref())
        .or_else(|| v.per_class.values().next())
        .cloned();
    if let Some(m) = model {
        let t = table(doc, "models");
        t.insert("view".into(), Value::String(m.clone()));
        t.insert("world".into(), Value::String(m));
    }
    if let Some(icon) = &v.icon {
        table(doc, "hud").insert("icon".into(), Value::String(icon.clone()));
    }
    let own = |n: &str| Value::String(crate::game::maps::own_sound(n));
    for (kind, place) in SOUNDS {
        let Some(name) = v.sounds.get(*kind) else { continue };
        if let Some(key) = place.strip_prefix("sounds.") {
            if let Some(t) = doc.get_mut("sounds").and_then(Value::as_table_mut)
                && t.contains_key(key)
            {
                t.insert(key.into(), own(name));
            }
        } else if let Some(rest) = place.strip_prefix("hook.") {
            let (h, key) = rest.rsplit_once('.').expect("hook.<hook>.<key>");
            if let Some(t) = hook(doc, h)
                && t.contains_key(key)
            {
                t.insert(key.into(), own(name));
            }
        } else if let Some(key) = place.strip_prefix("swings.")
            && let Some(m) = doc.get_mut("melee").and_then(Value::as_table_mut)
        {
            for (_, swing) in m.iter_mut() {
                if let Some(t) = swing.as_table_mut()
                    && t.contains_key(key)
                {
                    t.insert(key.into(), own(name));
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_fuse_goes_off_at_the_first_check_past_it() {
        assert!((fuse_life(2.0) - 2.1).abs() < 1e-9, "stock: the tenth check");
        assert!((fuse_life(1.4) - 1.47).abs() < 1e-9, "x0.7: the seventh");
        assert!((fuse_life(0.21) - 0.21).abs() < 1e-9);
    }
}
