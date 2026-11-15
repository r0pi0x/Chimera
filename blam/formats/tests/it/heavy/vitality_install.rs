//! The vitality and globals readers on the user's Halo disc image: the
//! Spartan's collision model against specs/halo_ce/health_shields.md ("The
//! collision model's vitality fields", materials, regions and nodes), the
//! globals' values movement.md, health_shields.md, projectiles.md and
//! multiplayer_rules.md state (run speeds, accelerations, the airborne
//! acceleration, stun, falling, the multiplayer information, the 43
//! announcer sounds, the grenades, the weapon list), a sweep of every
//! collision model of `bloodgulch`, and the same values on other
//! multiplayer maps. Each test passes with a note when there is no image.

// The spec's numbers are written as it states them, rounded.
#![allow(clippy::approx_constant, clippy::excessive_precision)]

use blam_formats::cache::{GRAVITY, MapFile, TagRef, class};
use blam_formats::collision::{MaterialType, read_model};
use blam_formats::globals::{AnnouncerEvent, Globals};
use blam_formats::object::{GrenadeType, TransitionFunction};
use blam_formats::vitality::{ModelFlags, Vitality};

use super::maps::{MULTIPLAYER, load};
use super::object_install::{near, normalized, path_of};

const SPARTAN_MODEL: &str = "characters\\cyborg\\cyborg";

fn vitality(map: &MapFile, path: &str) -> Vitality {
    Vitality::read(
        map,
        map.find(class::MODEL_COLLISION_GEOMETRY, path)
            .unwrap_or_else(|| panic!("no {path}")),
    )
    .unwrap()
}

fn globals(map: &MapFile) -> Globals {
    Globals::read(map, map.find(class::GLOBALS, "globals\\globals").unwrap()).unwrap()
}

fn p(map: &MapFile, r: &TagRef) -> Option<String> {
    path_of(map, r)
}

fn some(s: &str) -> Option<String> {
    Some(s.to_string())
}

#[test]
fn the_spartans_vitality_fields() {
    let Some(map) = load("bloodgulch") else { return };
    let v = vitality(&map, SPARTAN_MODEL);
    assert_eq!(v.flags.bits(), 0x7);
    assert!(v.flags.contains(ModelFlags::TAKES_SHIELD_DAMAGE_FOR_CHILDREN));
    assert!(v.flags.contains(ModelFlags::TAKES_BODY_DAMAGE_FOR_CHILDREN));
    assert!(v.flags.contains(ModelFlags::ALWAYS_SHIELDS_FRIENDLY_DAMAGE));
    assert_eq!(v.flags.unnamed_bits(), 0);
    assert_eq!(v.indirect_damage_material, 1);
    assert_eq!(v.indirect_material().unwrap().name, "body");
    assert_eq!(v.maximum_body_vitality, 75.0);
    assert_eq!(v.body_system_shock, 0.0);
    assert_eq!(v.friendly_damage_resistance, 0.0);
    assert_eq!(
        p(&map, &v.localized_damage_effect),
        some("characters\\cyborg\\localized damage")
    );
    assert_eq!(v.area_damage_effect_threshold, 0.0);
    assert_eq!(p(&map, &v.area_damage_effect), some("effects\\blood aoe human"));
    assert_eq!(v.body_damaged_threshold, 0.0);
    assert_eq!(p(&map, &v.body_damaged_effect), None);
    assert_eq!(p(&map, &v.body_depleted_effect), None);
    assert_eq!(v.body_destroyed_threshold, 0.0);
    assert_eq!(p(&map, &v.body_destroyed_effect), None);
    assert_eq!(v.maximum_shield_vitality, 75.0);
    assert_eq!(v.shield_material_type(), Some(MaterialType::CyborgEnergyShield));
    assert_eq!(v.shield_material_type_code, 22);
    assert_eq!(v.shield_failure_function, TransitionFunction::Linear);
    assert_eq!((v.shield_failure_threshold, v.failing_shield_leak_fraction), (0.0, 0.0));
    assert_eq!(v.minimum_stun_damage, 0.0);
    assert_eq!((v.stun_time, v.recharge_time), (6.0, 4.0));
    assert_eq!(v.shield_damaged_threshold, 0.0);
    assert_eq!(p(&map, &v.shield_damaged_effect), None);
    assert_eq!(
        p(&map, &v.shield_depleted_effect),
        some("characters\\cyborg\\cyborg shield depletion")
    );
    assert_eq!(p(&map, &v.shield_recharging_effect), None);
    assert_eq!(v.runtime_shield_recharge_velocity.to_bits(), 0x3C08_8889);
    near(v.runtime_shield_recharge_velocity, 0.008333334, "recharge velocity");
    // The shield delay is 180 ticks, the recharge 120; the stored per-tick
    // rate is 1 / (4.0 s x 30).
    assert_eq!((v.stun_ticks(), v.recharge_ticks()), (180, 120));
    assert!((v.computed_recharge_velocity() - v.runtime_shield_recharge_velocity).abs() < 1e-8);
    assert!(v.has_shield());
}

#[test]
fn the_spartans_materials_regions_and_nodes() {
    let Some(map) = load("bloodgulch") else { return };
    let v = vitality(&map, SPARTAN_MODEL);
    // Name, head, type, leak, shield multiplier, body multiplier.
    let rows = [
        ("head", true, 1.0),
        ("body", false, 1.0),
        ("legs", false, 0.8),
        ("shield generator", false, 1.0),
    ];
    assert_eq!(v.materials.len(), 4);
    for (i, (name, head, body)) in rows.into_iter().enumerate() {
        let m = v.material(i).unwrap();
        assert_eq!(m.name, name);
        assert_eq!(m.is_head(), head, "{name}");
        assert_eq!(m.flags.unnamed_bits(), 0, "{name}");
        assert_eq!(m.material_type(), Some(MaterialType::Cyborg), "{name}");
        assert_eq!(m.shield_leak_fraction, 0.0, "{name}");
        assert_eq!(m.shield_damage_multiplier, 1.0, "{name}");
        near(m.body_damage_multiplier, body, name);
    }
    // One region, `cyborg`: flags 0, threshold 0, no destroyed effect, one
    // permutation `__base`.
    assert_eq!(v.regions.len(), 1);
    let r = v.region(0).unwrap();
    assert_eq!(r.name, "cyborg");
    assert_eq!((r.flags.bits(), r.damage_threshold), (0, 0.0));
    assert!(r.destroyed_effect.is_null());
    let model = read_model(&map, map.find(class::MODEL_COLLISION_GEOMETRY, SPARTAN_MODEL).unwrap()).unwrap();
    assert_eq!(model.regions[0].permutations, ["__base"]);
    // Every node with geometry belongs to it.
    for (n, node) in model.nodes.iter().enumerate() {
        if !node.bsps.is_empty() && node.bsps.iter().any(|b| !b.surfaces.is_empty()) {
            assert_eq!(node.region, Some(0), "node {n} {}", node.name);
        }
    }
    // Damage parts by node: pelvis and spine 0, spine1 1, neck and head 2,
    // left clavicle and upper arm 3, left forearm and hand 4, left thigh,
    // calf and foot 6, right clavicle and upper arm 7, right forearm and hand
    // 8, right thigh, calf and foot 10; parts 5 and 9 unused.
    let want = |name: &str| -> i16 {
        match name {
            "pelvis" | "spine" => 0,
            "spine1" => 1,
            "neck" | "head" => 2,
            "l clavicle" | "l upperarm" => 3,
            "l forearm" | "l hand" => 4,
            "l thigh" | "l calf" | "l foot" => 6,
            "r clavicle" | "r upperarm" => 7,
            "r forearm" | "r hand" => 8,
            "r thigh" | "r calf" | "r foot" => 10,
            other => panic!("unexpected node {other}"),
        }
    };
    assert_eq!(v.nodes.len(), 19);
    for n in &v.nodes {
        let short = n.name.strip_prefix("bip01 ").unwrap_or_else(|| panic!("{}", n.name));
        assert_eq!(n.damage_part, want(short), "{}", n.name);
        assert!(n.damage_part != 5 && n.damage_part != 9);
    }
}

#[test]
fn every_collision_model_reads() {
    let Some(map) = load("bloodgulch") else { return };
    let mut n = 0;
    for t in map.of_class(class::MODEL_COLLISION_GEOMETRY) {
        let v = Vitality::read(&map, t).unwrap_or_else(|e| panic!("{}: {e}", t.path));
        let m = read_model(&map, t).unwrap();
        // The vitality reader and the geometry reader agree on the blocks.
        assert_eq!(v.materials.len(), m.materials.len(), "{}", t.path);
        assert_eq!(v.regions.len(), m.regions.len(), "{}", t.path);
        assert_eq!(v.nodes.len(), m.nodes.len(), "{}", t.path);
        for (a, b) in v.materials.iter().zip(&m.materials) {
            assert_eq!(
                (&a.name, a.flags.bits(), a.material_type_code),
                (&b.name, b.flags, b.material_type_code)
            );
        }
        for (a, b) in v.regions.iter().zip(&m.regions) {
            assert_eq!((&a.name, a.flags.bits()), (&b.name, b.flags));
        }
        // References resolve to effects (or are null).
        for r in [
            &v.localized_damage_effect,
            &v.area_damage_effect,
            &v.body_damaged_effect,
            &v.body_depleted_effect,
            &v.body_destroyed_effect,
            &v.shield_damaged_effect,
            &v.shield_depleted_effect,
            &v.shield_recharging_effect,
        ] {
            if let Some(e) = map.tag(r.id) {
                assert_eq!(e.class, class::EFFECT, "{}", t.path);
            }
        }
        for r in &v.regions {
            if let Some(e) = map.tag(r.destroyed_effect.id) {
                assert_eq!(e.class, class::EFFECT, "{}", t.path);
            }
        }
        assert!(
            v.maximum_body_vitality >= 0.0 && v.maximum_shield_vitality >= 0.0,
            "{}",
            t.path
        );
        assert!(v.shield_material_type().is_some() || !v.has_shield(), "{}", t.path);
        assert!(v.indirect_damage_material >= -1);
        for m in &v.materials {
            assert!((0.0..=1.0).contains(&m.shield_leak_fraction), "{}: {}", t.path, m.name);
        }
        n += 1;
    }
    assert!(n >= 30, "{n} collision models");
    // The vehicles have no body or shield of their own in the model (flags:
    // passes area damage to children, only damaged by explosives); the
    // Spartan alone has both; a rocket ammo box has 50 body points.
    for path in [
        "vehicles\\warthog\\warthog",
        "vehicles\\ghost\\ghost_mp",
        "vehicles\\scorpion\\scorpion_mp\\scorpion_mp",
    ] {
        let v = vitality(&map, path);
        assert_eq!(v.flags.bits(), 0x28, "{path}");
        assert!(v.flags.contains(ModelFlags::PASSES_AREA_DAMAGE_TO_CHILDREN));
        assert!(v.flags.contains(ModelFlags::ONLY_DAMAGED_BY_EXPLOSIVES));
        assert_eq!(
            (v.maximum_body_vitality, v.maximum_shield_vitality),
            (0.0, 0.0),
            "{path}"
        );
    }
    let ammo = vitality(&map, "powerups\\rocket launcher ammo\\rocket launcher ammo");
    assert_eq!(ammo.maximum_body_vitality, 50.0);
    assert_eq!(
        vitality(&map, "scenery\\rocks\\boulder\\boulder").maximum_shield_vitality,
        0.0
    );
}

#[test]
fn collision_models_are_identical_on_other_maps() {
    let Some(base) = load("bloodgulch") else { return };
    for name in ["hangemhigh", "damnation"] {
        let Some(other) = load(name) else { return };
        let mut n = 0;
        for t in base.of_class(class::MODEL_COLLISION_GEOMETRY) {
            let Some(o) = other.find(class::MODEL_COLLISION_GEOMETRY, &t.path) else {
                continue;
            };
            assert_eq!(
                normalized(&base, &Vitality::read(&base, t).unwrap()),
                normalized(&other, &Vitality::read(&other, o).unwrap()),
                "{name}: {}",
                t.path
            );
            n += 1;
        }
        // The Spartan's model is on every map.
        assert!(n >= 10, "{name}: {n} models compared");
        assert!(other.find(class::MODEL_COLLISION_GEOMETRY, SPARTAN_MODEL).is_some());
    }
}

#[test]
fn the_globals_values_of_movement_and_health() {
    let Some(map) = load("bloodgulch") else { return };
    let g = globals(&map);
    let pl = &g.player;
    assert_eq!(p(&map, &pl.unit), some("characters\\cyborg\\cyborg"));
    // movement.md's constants table.
    near(pl.walking_speed, 0.512, "walking speed");
    near(pl.double_speed_multiplier, 1.25, "double speed");
    near(pl.run_forward_speed, 2.25, "run forward");
    near(pl.run_backward_speed, 2.0, "run backward");
    near(pl.run_sideways_speed, 2.0, "run sideways");
    near(pl.run_acceleration, 0.32, "run acceleration");
    near(pl.sneak_forward_speed, 0.9, "sneak forward");
    near(pl.sneak_backward_speed, 0.65, "sneak backward");
    near(pl.sneak_sideways_speed, 0.6, "sneak sideways");
    near(pl.sneak_acceleration, 0.16, "sneak acceleration");
    near(pl.airborne_acceleration, 0.035, "airborne acceleration");
    near(pl.stun_movement_penalty, 0.8, "stun movement");
    near(pl.stun_turning_penalty, 0.8, "stun turning");
    near(pl.stun_jumping_penalty, 1.0, "stun jumping");
    assert_eq!(pl.minimum_stun_time, 0.0);
    near(pl.maximum_stun_time, 1.25, "maximum stun time");
    assert_eq!(pl.maximum_stun_ticks(), 37);
    // globals.md: the first-person idle numbers on the disc (not the synthetic 8, 12, 0.5).
    near(pl.first_person_idle_time.lower, 10.0, "idle lower");
    near(pl.first_person_idle_time.upper, 15.0, "idle upper");
    near(pl.first_person_idle_skip, 0.25, "idle skip");
    assert_eq!(pl.first_person_idle_ticks(), (300, 450));
    // The grenade origin (0, 0.05, 0): 0.05 to the left of the camera.
    near(pl.grenade_origin[0], 0.0, "grenade origin x");
    near(pl.grenade_origin[1], 0.05, "grenade origin y");
    near(pl.grenade_origin[2], 0.0, "grenade origin z");
    // Per tick (the spec's WU/t): run forward 0.075, acceleration 0.0106667,
    // airborne 0.00116667.
    near(pl.run_forward_speed / 30.0, 0.075, "run forward per tick");
    near(pl.run_acceleration / 30.0, 0.0106667, "run acceleration per tick");
    near(pl.airborne_acceleration / 30.0, 0.00116667, "airborne per tick");

    // Falling damage: 3.0 to 6.0 harmful, 16.5 maximum, the precomputed
    // velocities sqrt(2 g d) to 7 digits, the two damage effects.
    let f = &g.falling;
    assert_eq!(
        (f.harmful_falling_distance.lower, f.harmful_falling_distance.upper),
        (3.0, 6.0)
    );
    assert_eq!(f.maximum_falling_distance, 16.5);
    near(f.harmful_falling_velocity.lower, 0.14625688, "fall velocity min");
    near(f.harmful_falling_velocity.upper, 0.20683846, "fall velocity max");
    near(f.maximum_falling_velocity, 0.3430028, "fall velocity kill");
    for (v, d) in [
        (f.harmful_falling_velocity.lower, 3.0f32),
        (f.harmful_falling_velocity.upper, 6.0),
        (f.maximum_falling_velocity, 16.5),
    ] {
        assert!((v - (2.0 * GRAVITY * d).sqrt()).abs() < 1e-6, "sqrt(2 g {d})");
    }
    assert_eq!(p(&map, &f.falling_damage), some("globals\\falling"));
    assert_eq!(p(&map, &f.distance_damage), some("globals\\distance"));
    assert_eq!(
        p(&map, &f.vehicle_hit_environment_damage),
        some("globals\\vehicle_hit_environment")
    );
    assert_eq!(
        p(&map, &f.vehicle_killed_unit_damage),
        some("globals\\vehicle_killed_unit")
    );
    assert_eq!(p(&map, &f.vehicle_collision_damage), some("globals\\vehicle_collision"));
    assert_eq!(p(&map, &f.flaming_death_damage), some("globals\\flaming_death"));
    assert_eq!(g.difficulty_count, 1);
}

#[test]
fn the_multiplayer_information_and_grenades() {
    let Some(map) = load("bloodgulch") else { return };
    let g = globals(&map);
    let m = &g.multiplayer;
    assert_eq!(p(&map, &m.flag), some("weapons\\flag\\flag"));
    assert_eq!(p(&map, &m.unit), some("characters\\cyborg_mp\\cyborg_mp"));
    assert_eq!(
        m.vehicles.iter().map(|r| p(&map, r)).collect::<Vec<_>>(),
        [
            some("vehicles\\warthog\\warthog"),
            some("vehicles\\ghost\\ghost_mp"),
            some("vehicles\\scorpion\\scorpion_mp")
        ]
    );
    assert_eq!(p(&map, &m.hill_shader), some("scenery\\hilltop\\shaders\\hilltop"));
    assert_eq!(p(&map, &m.flag_shader), some("weapons\\flag\\shaders\\flag_blue"));
    assert_eq!(p(&map, &m.ball), some("weapons\\ball\\ball"));

    // The 43 announcer sounds, in the spec's order.
    let sounds = [
        "dialog\\multiplayer1\\play_ball",
        "dialog\\multiplayer1\\game_over",
        "dialog\\multiplayer1\\one_minute_to_win",
        "dialog\\multiplayer1\\30_seconds_to_win",
        "dialog\\multiplayer1\\red_team_minute_to_win",
        "dialog\\multiplayer1\\red_team_30_to_win",
        "dialog\\multiplayer1\\blue_team_minute_to_win",
        "dialog\\multiplayer1\\blue_team_30_to_win",
        "dialog\\multiplayer1\\blue_team_has_the_flag",
        "dialog\\multiplayer1\\blue_team_flag_returned",
        "dialog\\multiplayer1\\blue_team_score",
        "dialog\\multiplayer1\\red_team_has_the_flag",
        "dialog\\multiplayer1\\red_team_flag_returned",
        "dialog\\multiplayer1\\red_team_score",
        "dialog\\multiplayer1\\double_kill",
        "dialog\\multiplayer1\\triple_kill",
        "dialog\\multiplayer1\\killtacular",
        "dialog\\multiplayer1\\running_riot",
        "dialog\\multiplayer1\\killing_spree",
        "dialog\\multiplayer1\\oddball",
        "dialog\\multiplayer1\\race",
        "dialog\\multiplayer1\\slayer",
        "dialog\\multiplayer1\\capture_the_flag",
        "dialog\\multiplayer1\\warthog",
        "dialog\\multiplayer1\\ghost",
        "dialog\\multiplayer1\\scorpion",
        "sfx\\ui\\countdown_timer",
        "sfx\\ui\\teleporter_activate",
        "sfx\\ui\\flag_failure",
        "sfx\\ui\\countdown_for_respawn",
        "sfx\\ui\\hill_move",
        "sfx\\ui\\player_respawn",
        "dialog\\multiplayer1\\team_king_of_the_hill",
        "dialog\\multiplayer1\\team_oddball",
        "dialog\\multiplayer1\\team_race",
        "dialog\\multiplayer1\\team_slayer",
        "dialog\\multiplayer1\\king_of_the_hill",
        "dialog\\multiplayer1\\blue_team_ctf",
        "dialog\\multiplayer1\\red_team_ctf",
        "dialog\\multiplayer1\\hill_contested",
        "dialog\\multiplayer1\\hill_controlled",
        "dialog\\multiplayer1\\hill_occupied",
        "sfx\\ui\\countdown_timer_end",
    ];
    assert_eq!(m.sounds.len(), 43);
    for (event, path) in AnnouncerEvent::ALL.iter().zip(sounds) {
        let r = m.sound(*event).unwrap();
        let t = map.tag(r.id).unwrap_or_else(|| panic!("{event:?}"));
        assert_eq!(t.class, class::SOUND, "{event:?}");
        assert_eq!(t.path, format!("sound\\{path}"), "{event:?}");
    }
    assert_eq!(
        m.sound(AnnouncerEvent::Race)
            .map(|r| map.tag(r.id).unwrap().path.as_str()),
        Some("sound\\dialog\\multiplayer1\\race")
    );

    // The grenades: maximum count 4, mp spawn default 0, the throwing
    // effects, HUD interfaces, items and projectiles.
    assert_eq!(g.grenades.len(), 2);
    let frag = g.grenade(GrenadeType::Frag).unwrap();
    let plasma = g.grenade(GrenadeType::Plasma).unwrap();
    for gr in [frag, plasma] {
        assert_eq!((gr.maximum_count, gr.mp_spawn_default), (4, 0));
    }
    assert_eq!(
        p(&map, &frag.throwing_effect),
        some("weapons\\frag grenade\\effects\\throw")
    );
    assert_eq!(
        p(&map, &plasma.throwing_effect),
        some("weapons\\plasma grenade\\effects\\throw")
    );
    assert_eq!(p(&map, &frag.hud_interface), some("ui\\hud\\default"));
    assert_eq!(p(&map, &plasma.hud_interface), some("ui\\hud\\plasma_flare"));
    assert_eq!(p(&map, &frag.equipment), some("weapons\\frag grenade\\frag grenade"));
    assert_eq!(
        p(&map, &plasma.equipment),
        some("weapons\\plasma grenade\\plasma grenade")
    );
    assert_eq!(p(&map, &frag.projectile), some("weapons\\frag grenade\\frag grenade"));
    assert_eq!(
        p(&map, &plasma.projectile),
        some("weapons\\plasma grenade\\plasma grenade")
    );
    assert_eq!(map.tag(frag.projectile.id).unwrap().class, class::PROJECTILE);
    assert_eq!(map.tag(frag.equipment.id).unwrap().class, class::EQUIPMENT);

    // The multiplayer weapon list: 14 tags in the order of the spec's 5.3.
    let order = [
        "assault rifle",
        "flamethrower",
        "gravity rifle",
        "needler",
        "pistol",
        "plasma pistol",
        "plasma rifle",
        "rocket launcher",
        "shotgun",
        "sniper rifle",
        "ball",
        "flag",
        "frag grenade",
        "plasma grenade",
    ];
    assert_eq!(g.weapon_list.len(), 14);
    for (r, name) in g.weapon_list.iter().zip(order) {
        assert_eq!(p(&map, r), Some(format!("weapons\\{name}\\{name}")), "{name}");
    }
}

#[test]
fn the_globals_are_identical_on_every_multiplayer_map() {
    let Some(base) = load("bloodgulch") else { return };
    let want = normalized(&base, &globals(&base));
    for name in MULTIPLAYER {
        let Some(other) = load(name) else { return };
        assert_eq!(normalized(&other, &globals(&other)), want, "{name}");
    }
}
