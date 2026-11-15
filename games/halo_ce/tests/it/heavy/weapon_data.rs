//! Halo's weapon documents (data/weapons/*.toml) against the user's disc
//! image: every number of each `halo_ce:weapon` table equals what
//! `blam_formats` reads from `bloodgulch.map` (the weapon tag, its
//! magazine and trigger, its projectile, the damage effects they name, and
//! the first-person animation graph's frame counts that time the draw,
//! reloads and melee), so the data cannot drift from the disc. Passes with
//! a note without an install.

use blam_formats::{
    animation::{AnimationGraph, FirstPersonSlot, WeaponSlot},
    cache::{MapFile, TagRef, class},
    projectile::Projectile,
    weapon::{Weapon, ref_id},
};
use halo_ce::game::{
    vitality::DamageEffects,
    weapons::{guns, params::effect_id},
};

fn bloodgulch() -> Option<MapFile> {
    let mount = super::maps::mount()?;
    let bytes = mount.read("maps/bloodgulch.map").unwrap();
    Some(MapFile::load(&bytes).unwrap())
}

fn path(map: &MapFile, r: &TagRef) -> Option<String> {
    ref_id(r).and_then(|id| map.tag(id)).map(|t| t.path.clone())
}

/// Equal to the f32 the tag holds (the data writes the spec's values with
/// enough digits to be that f32, or within a rounding of its last digit).
fn same(ours: f32, disc: f32, what: &str) {
    assert!((ours - disc).abs() <= disc.abs() * 2e-7 + 1e-9, "{what}: data {ours}, disc {disc}");
}

#[test]
fn every_weapon_number_is_the_disc_s() {
    let Some(map) = bloodgulch() else { return };
    let effects = DamageEffects::load();
    for (id, g) in guns() {
        let w = Weapon::read(&map, map.find(class::WEAPON, &g.tag).unwrap_or_else(|| panic!("{id}: {}", g.tag))).unwrap();
        let o = &w.own;
        assert_eq!(o.flags.bits(), g.flags, "{id}: flags");
        assert_eq!(o.weapon_type as u16, g.weapon_type, "{id}: type");
        assert_eq!(o.magnification_levels, g.zoom_levels, "{id}: zoom levels");
        if let Some([m0, m1]) = g.magnification {
            same(m0, o.magnification_range.lower, &format!("{id}: magnification"));
            same(m1, o.magnification_range.upper, &format!("{id}: magnification"));
        }
        let h = &g.heat;
        for (ours, disc, what) in [
            (h.recovery_threshold, o.heat_recovery_threshold, "heat recovery"),
            (h.overheated_threshold, o.overheated_threshold, "overheated"),
            (h.detonation_threshold, o.heat_detonation_threshold, "detonation threshold"),
            (h.detonation_fraction, o.heat_detonation_fraction, "detonation fraction"),
            (h.loss_per_second, o.heat_loss_per_second, "heat loss"),
            (h.illumination, o.heat_illumination, "heat illumination"),
            (g.age.heat_recovery_penalty, o.age_heat_recovery_penalty, "age heat penalty"),
            (g.age.rate_of_fire_penalty, o.age_rate_of_fire_penalty, "age rof penalty"),
            (g.age.misfire_start, o.age_misfire_start, "misfire start"),
            (g.age.misfire_chance, o.age_misfire_chance, "misfire chance"),
            (g.aim_assist.autoaim_angle, o.autoaim_angle, "autoaim angle"),
            (g.aim_assist.autoaim_range, o.autoaim_range, "autoaim range"),
            (g.aim_assist.magnetism_angle, o.magnetism_angle, "magnetism angle"),
            (g.aim_assist.magnetism_range, o.magnetism_range, "magnetism range"),
            (g.aim_assist.deviation_angle, o.deviation_angle, "deviation"),
            (g.camo[0], o.active_camo_ding, "camo ding"),
            (g.camo[1], o.active_camo_regrowth_rate, "camo regrowth"),
        ] {
            same(ours, disc, &format!("{id}: {what}"));
        }
        assert_eq!(path(&map, &o.melee_damage), effect_id(&g.melee_damage).and_then(|e| effects.get(e)).map(|e| e.tag.clone()), "{id}: melee");
        // The magazine.
        let m = g.magazine.unwrap();
        let dm = &o.magazines[0];
        assert_eq!(
            (dm.flags.bits(), dm.rounds_recharged, dm.rounds_total_initial, dm.rounds_total_maximum, dm.rounds_loaded_maximum, dm.rounds_reloaded),
            (m.flags, m.rounds_recharged, m.rounds_total_initial, m.rounds_total_maximum, m.rounds_loaded_maximum, m.rounds_reloaded),
            "{id}: magazine"
        );
        same(m.chamber_time, dm.chamber_time, &format!("{id}: chamber time"));
        // The trigger.
        let t = &g.trigger;
        let dt = &o.triggers[0];
        assert_eq!(o.triggers.len(), 1, "{id}: one trigger");
        assert_eq!(dt.flags.bits(), t.flags, "{id}: trigger flags");
        assert_eq!(
            (dt.magazine, dt.rounds_per_shot, dt.minimum_rounds_loaded, dt.rounds_between_tracers, dt.projectiles_per_shot),
            (t.magazine, t.rounds_per_shot, t.minimum_rounds_loaded, t.rounds_between_tracers, t.projectiles_per_shot),
            "{id}: trigger counts"
        );
        assert_eq!(dt.distribution_function as u16, t.distribution, "{id}: distribution");
        assert_eq!(dt.firing_effects.len(), t.firing_effects as usize, "{id}: firing effects");
        assert!(dt.firing_effects.iter().all(|e| (e.shot_count.lower, e.shot_count.upper) == (1, 1)), "{id}: shot counts 1 - 1");
        for (ours, disc, what) in [
            (t.rounds_per_second[0], dt.rounds_per_second.lower, "rounds per second"),
            (t.rounds_per_second[1], dt.rounds_per_second.upper, "rounds per second"),
            (t.rate_of_fire_acceleration, dt.runtime_rate_of_fire_acceleration, "rof acceleration"),
            (t.rate_of_fire_deceleration, dt.runtime_rate_of_fire_deceleration, "rof deceleration"),
            (t.error_acceleration, dt.runtime_error_acceleration, "error acceleration"),
            (t.error_deceleration, dt.runtime_error_deceleration, "error deceleration"),
            (t.minimum_error, dt.minimum_error, "minimum error"),
            (t.error_angle[0], dt.error_angle.lower, "error angle"),
            (t.error_angle[1], dt.error_angle.upper, "error angle"),
            (t.distribution_angle, if t.distribution == 1 { dt.distribution_angle } else { 0.0 }, "distribution angle"),
            (t.first_person_offset[0], dt.first_person_offset[0], "offset"),
            (t.first_person_offset[1], dt.first_person_offset[1], "offset"),
            (t.first_person_offset[2], dt.first_person_offset[2], "offset"),
            (t.heat_per_round, dt.heat_generated_per_round, "heat per round"),
            (t.age_per_round, dt.age_generated_per_round, "age per round"),
        ] {
            same(ours, disc, &format!("{id}: {what}"));
        }
        assert_eq!(path(&map, &dt.projectile).as_deref(), Some(t.projectile.as_str()), "{id}: projectile");
        // The projectile.
        let p = &g.projectile;
        assert_eq!(p.tag, t.projectile);
        let dp = Projectile::read(&map, map.find(class::PROJECTILE, &p.tag).unwrap()).unwrap();
        let own = &dp.own;
        for (ours, disc, what) in [
            (p.initial_velocity, own.initial_velocity, "initial velocity"),
            (p.final_velocity, own.final_velocity, "final velocity"),
            (p.air_damage_range[0], own.air_damage_range.lower, "air range"),
            (p.air_damage_range[1], own.air_damage_range.upper, "air range"),
            (p.water_damage_range[0], own.water_damage_range.lower, "water range"),
            (p.water_damage_range[1], own.water_damage_range.upper, "water range"),
            (p.maximum_range, own.maximum_range, "maximum range"),
            (p.air_gravity_scale, own.air_gravity_scale, "gravity"),
            (p.water_gravity_scale, own.water_gravity_scale, "water gravity"),
            (p.collision_radius, own.collision_radius, "collision radius"),
            (p.minimum_velocity, own.minimum_velocity, "minimum velocity"),
            (p.timer[0], own.detonation_timer.lower, "timer"),
            (p.timer[1], own.detonation_timer.upper, "timer"),
        ] {
            assert!((ours - disc).abs() <= disc.abs() * 2e-5 + 1e-6, "{id}: {what}: data {ours}, disc {disc}");
        }
        assert_eq!(
            path(&map, &own.impact_damage),
            effect_id(&p.impact_damage).and_then(|e| effects.get(e)).map(|e| e.tag.clone()),
            "{id}: impact damage"
        );
        // The first-person animations.
        let fp = path(&map, &o.first_person_animations).unwrap();
        let graph = AnimationGraph::read(&map, map.find(class::MODEL_ANIMATIONS, &fp).unwrap()).unwrap();
        let slots = &graph.first_person_weapons[0].animations;
        let frames = |s: FirstPersonSlot| slots.get(s).map(|i| graph.animations[i].frame_count as i32);
        let a = &g.animations;
        assert_eq!(frames(FirstPersonSlot::Ready), Some(a.ready), "{id}: ready");
        assert_eq!(frames(FirstPersonSlot::ReloadEmpty), a.reload_empty, "{id}: reload-empty");
        assert_eq!(frames(FirstPersonSlot::Melee), Some(a.melee), "{id}: melee");
        let key = slots.get(FirstPersonSlot::Melee).map(|i| graph.animations[i].key_frame as i32);
        assert_eq!(key, Some(a.melee_key), "{id}: melee key frame");
        if let Some(enter) = a.enter {
            assert_eq!(frames(FirstPersonSlot::Enter), Some(enter), "{id}: enter");
        }
        // The weapon's own graph: its primary recoil (`fire-1`) and
        // chamber (`chamber-1`) animations, or none.
        let world = path(&map, &w.object.animation_graph)
            .map(|p| AnimationGraph::read(&map, map.find(class::MODEL_ANIMATIONS, &p).unwrap()).unwrap());
        let own = |s: WeaponSlot| {
            let g = world.as_ref()?;
            g.weapons.first()?.animations.get(s).map(|i| g.animations[i].frame_count as i32)
        };
        assert_eq!(own(WeaponSlot::Fire1), a.world_recoil, "{id}: fire-1");
        assert_eq!(own(WeaponSlot::Chamber1), a.world_chamber, "{id}: chamber-1");
    }
}
