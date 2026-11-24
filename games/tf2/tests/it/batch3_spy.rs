//! TF2's batch 3, the Spy (specs/tf2/batch3_mechanics.md 3-5, its test
//! cases; batch2b_mechanics.md 16's Enforcer): a made-up item schema in
//! items_game's shape (nothing copied from the game) through the
//! converter, each item's own numbers from data/items.toml's `hooks`,
//! played on the headless harness: the watches (cloak, the Cloak and
//! Dagger, the Dead Ringer's feign and cuts), the disguise, the knives
//! (Your Eternal Reward, Big Earner, Spy-cicle), the Enforcer; and on the
//! network, what an enemy's client sees of a disguised and of a cloaked
//! Spy.

use bevy::prelude::*;
use chimera_core::{
    core::{Damage, DamageKind, Feigned, Health, Hitgroup, SimClock, Team},
    guise::Guise,
};
use chimera_sim::{
    status::{Condition, Conditions},
    testkit,
    weapon::{Inventory, give},
};
use chimera_world::{greybox::{self, GreyboxMapPlugin}, harness::Sim};
use source_engine::source_movement::SourceTraits;
use tf2::{
    game::{
        TICK_INTERVAL,
        items::{ItemPack, convert, spy::{self, Tf2Cloak, Tf2Spy}},
        weapons::{KNIFE, MINIGUN, REVOLVER, ROCKET_LAUNCHER, script_numbers::GunNumbers},
    },
    harness::Tf2Sim,
};

use super::weapons::{health_points, hold_still, holding, no_spread, run};

const U: f32 = 0.0254;

const DEFS: &[(&str, &str)] = &[("add_maxhealth", "value_is_additive"), ("mult_postfiredelay", "value_is_inverted_percentage")];

/// (index, name, class, slot, attributes).
#[allow(clippy::type_complexity)]
const ITEMS: &[(u32, &str, &str, &str, &[(&str, &str)])] = &[
    (30, "Invisibility Watch", "tf_weapon_invis", "pda2", &[]),
    (60, "The Cloak and Dagger", "tf_weapon_invis", "pda2", &[]),
    (59, "The Dead Ringer", "tf_weapon_invis", "pda2", &[]),
    (225, "Your Eternal Reward", "tf_weapon_knife", "melee", &[]),
    (461, "The Big Earner", "tf_weapon_knife", "melee", &[("add_maxhealth", "-25")]),
    (649, "The Spy-cicle", "tf_weapon_knife", "melee", &[]),
    (460, "The Enforcer", "tf_weapon_revolver", "secondary", &[("mult_postfiredelay", "1.2")]),
];

fn schema() -> String {
    let mut s = String::from("\"items_game\"\n{\n\"items\"\n{\n");
    for (index, name, class, slot, attrs) in ITEMS {
        s += &format!("\"{index}\" {{ \"name\" \"{name}\" \"item_class\" \"{class}\" \"item_slot\" \"{slot}\" \"used_by_classes\" {{ \"spy\" \"1\" }} \"attributes\" {{\n");
        for (a, v) in *attrs {
            s += &format!("\"{a}\" {{ \"value\" \"{v}\" }}\n");
        }
        s += "} }\n";
    }
    s += "}\n\"attributes\"\n{\n";
    for (i, (class, format)) in DEFS.iter().enumerate() {
        s += &format!("\"{i}\" {{ \"name\" \"{class}\" \"attribute_class\" \"{class}\" \"description_format\" \"{format}\" }}\n");
    }
    s + "}\n}\n"
}

fn pack() -> ItemPack {
    let mut p = convert::convert(&schema(), &|_| None).pack;
    for r in &mut p.items {
        if r.class == "tf_weapon_revolver" {
            r.base.insert("damage".into(), 40.0);
            r.base.insert("interval".into(), 0.5);
        }
    }
    ItemPack::parse(&p.to_text()).unwrap()
}

#[derive(Clone)]
struct WithItems;

impl Plugin for WithItems {
    fn build(&self, app: &mut App) {
        app.insert_resource(GunNumbers::builtin().with_pack(&pack()));
        Plugin::build(&Tf2Sim, app);
    }
}

fn sim() -> Sim {
    let mut sim = Sim::new(WithItems, GreyboxMapPlugin);
    sim.set_tick_interval(TICK_INTERVAL);
    sim
}

fn now(sim: &Sim) -> f64 {
    sim.app.world().resource::<SimClock>().now
}

fn speed(sim: &Sim, p: Entity) -> f32 {
    sim.app.world().get::<SourceTraits>(p).unwrap().max_speed.unwrap()
}

fn set_health(sim: &mut Sim, e: Entity, points: f32) {
    sim.app.world_mut().get_mut::<Health>(e).unwrap().current = points / 100.0;
}

fn carry(sim: &mut Sim, p: Entity, id: &str) -> Entity {
    give(sim.app.world_mut(), p, id).unwrap()
}

/// What `amount` points of `kind` on `target` take off him.
fn hit(sim: &mut Sim, attacker: Option<Entity>, target: Entity, amount: f32, kind: DamageKind) -> f32 {
    let before = health_points(sim, target);
    sim.app.world_mut().resource_mut::<Messages<Damage>>().write(Damage {
        target,
        attacker,
        amount: amount / 100.0,
        point: Vec3::ZERO,
        dir: Vec3::NEG_Z,
        hitgroup: Hitgroup::Generic,
        kind,
        weapon: None,
        force: Vec3::ZERO,
    });
    sim.ticks(1);
    (before - health_points(sim, target)).round()
}

fn press_alt(sim: &mut Sim, p: Entity) {
    run(sim, p, 2, |s, k| s.intent(p).secondary = k == 0);
}

fn cloak(sim: &Sim, w: Entity) -> spy::CloakState {
    sim.app.world().get::<Tf2Cloak>(w).unwrap().state
}

fn hidden(sim: &Sim, p: Entity) -> u8 {
    sim.app.world().get::<Guise>(p).map_or(0, |g| g.hidden)
}

fn spy_state(sim: &Sim, p: Entity) -> Tf2Spy {
    sim.app.world().get::<Tf2Spy>(p).cloned().unwrap_or_default()
}

/// A Spy (his revolver drawn) carrying `watch` (if any), on RED.
fn a_spy(sim: &mut Sim, watch: Option<&str>) -> (Entity, Entity, Option<Entity>) {
    let (p, gun) = holding(sim, greybox::SPAWNS[0], REVOLVER);
    sim.app.world_mut().entity_mut(p).insert(Team(1));
    let w = watch.map(|id| carry(sim, p, id));
    sim.ticks(2);
    (p, gun, w)
}

fn set_meter(sim: &mut Sim, w: Entity, m: f32) {
    sim.app.world_mut().get_mut::<Tf2Cloak>(w).unwrap().state.meter = m;
}

// ---- 3.1 Cloaking ----

#[test]
fn the_watch_cloaks_in_1_s_and_decloaks_in_2_s_with_no_attacks_until_visible() {
    let mut sim = sim();
    let (p, _, w) = a_spy(&mut sim, Some("tf2:invis_watch"));
    let w = w.unwrap();
    assert_eq!(sim.app.world().get::<Inventory>(p).unwrap().active.map(|a| a != w), Some(true), "the watch isn't drawn");
    press_alt(&mut sim, p);
    assert!(cloak(&sim, w).cloaked);
    sim.seconds(0.5);
    let half = hidden(&sim, p);
    assert!((100..=170).contains(&half), "fading in: {half}");
    sim.seconds(0.55);
    assert_eq!(hidden(&sim, p), 255, "fully invisible 1.0 s in");
    assert!((speed(&sim, p) - 320.0).abs() < 0.01, "his own speed cloaked");
    press_alt(&mut sim, p);
    assert!(!cloak(&sim, w).cloaked);
    sim.seconds(1.0);
    assert!(hidden(&sim, p) > 100, "decloaking over 2 s");
    assert!(sim.app.world().get::<Inventory>(p).unwrap().next_attack > now(&sim), "no attacks yet");
    sim.seconds(1.05);
    assert_eq!(hidden(&sim, p), 0, "fully visible 2.0 s after");
    assert!(sim.app.world().get::<Inventory>(p).unwrap().next_attack <= now(&sim) + 1e-6, "can attack");
}

#[test]
fn an_invis_watch_drains_in_10_s_and_forces_the_decloak() {
    let mut sim = sim();
    let (p, _, w) = a_spy(&mut sim, Some("tf2:invis_watch"));
    let w = w.unwrap();
    press_alt(&mut sim, p);
    sim.seconds(5.0);
    let m = cloak(&sim, w).meter;
    assert!((m - 50.0).abs() < 1.0, "10 a second: {m}");
    sim.seconds(5.1);
    assert!(!cloak(&sim, w).cloaked, "decloaked at 0");
    assert!(cloak(&sim, w).meter < 1.0);
    sim.seconds(3.0);
    let m = cloak(&sim, w).meter;
    assert!((m - 10.0).abs() < 1.0, "refills 3.33 a second: {m}");
}

#[test]
fn the_cloak_and_dagger_refills_standing_still_and_drains_moving() {
    let mut sim = sim();
    let (p, _, w) = a_spy(&mut sim, Some("tf2:cloak_and_dagger"));
    let w = w.unwrap();
    set_meter(&mut sim, w, 50.0);
    press_alt(&mut sim, p);
    let m0 = cloak(&sim, w).meter;
    sim.seconds(1.0);
    let still = cloak(&sim, w).meter - m0;
    assert!((still - 6.6).abs() < 0.3, "+6.6 a second standing still: {still}");
    // Running at 320: −15 a second.
    run(&mut sim, p, 60, |s, _| s.intent(p).move_axis = Vec2::Y);
    let m1 = cloak(&sim, w).meter;
    run(&mut sim, p, 67, |s, _| s.intent(p).move_axis = Vec2::Y);
    let moving = m1 - cloak(&sim, w).meter;
    assert!((moving - 15.0).abs() < 1.5, "−15 a second at full speed: {moving}");
}

#[test]
fn damage_taken_while_cloaked_is_cut_to_80_percent() {
    let mut sim = sim();
    let (p, _, _) = a_spy(&mut sim, Some("tf2:invis_watch"));
    press_alt(&mut sim, p);
    set_health(&mut sim, p, 125.0);
    assert_eq!(hit(&mut sim, None, p, 100.0, DamageKind::Bullet), 80.0);
    set_health(&mut sim, p, 125.0);
    assert_eq!(hit(&mut sim, None, p, 100.0, DamageKind::Blast), 80.0);
}

// ---- 3.3 The Dead Ringer ----

#[test]
fn the_dead_ringer_arms_only_full_and_feigns_the_next_hit() {
    let mut sim = sim();
    let (p, _, w) = a_spy(&mut sim, Some("tf2:dead_ringer"));
    let w = w.unwrap();
    set_meter(&mut sim, w, 60.0);
    press_alt(&mut sim, p);
    assert!(!cloak(&sim, w).armed, "60: not armed");
    set_meter(&mut sim, w, 100.0);
    press_alt(&mut sim, p);
    assert!(cloak(&sim, w).armed);
    set_health(&mut sim, p, 125.0);
    let attacker = sim.spawn_character(greybox::SPAWNS[1], tf2::game::movement::ID);
    let mut feigns = sim.app.world().resource::<Messages<Feigned>>().get_cursor_current();
    assert_eq!(hit(&mut sim, Some(attacker), p, 100.0, DamageKind::Bullet), 25.0, "the triggering hit x 0.25");
    let f: Vec<_> = feigns.read(sim.app.world().resource::<Messages<Feigned>>()).cloned().collect();
    assert_eq!(f.len(), 1, "a fake death");
    assert_eq!((f[0].0.entity, f[0].0.attacker), (p, Some(attacker)));
    assert!(health_points(&sim, p) > 0.0, "alive");
    assert_eq!(hidden(&sim, p), 255, "invisible at once");
    assert!((cloak(&sim, w).meter - 50.0).abs() < 0.5, "50 off: {}", cloak(&sim, w).meter);
    sim.ticks(2);
    assert!((speed(&sim, p) - 425.0).abs() < 0.5, "the speed boost: {}", speed(&sim, p));
    // 1.2 s after: cut by 0.65 − 1.2/16.
    // The hit lands on the next tick: 1.21 s after the trigger.
    let f = cloak(&sim, w).feign.expect("feigning");
    let n = ((f + 1.21 - TICK_INTERVAL - now(&sim)) / TICK_INTERVAL).round() as u64;
    sim.ticks(n);
    set_health(&mut sim, p, 125.0);
    assert_eq!(hit(&mut sim, None, p, 100.0, DamageKind::Bullet), 43.0);
    sim.seconds(2.0);
    assert!(speed(&sim, p) < 330.0, "the boost over after 3 s");
    assert!(cloak(&sim, w).cloaked, "still cloaked");
    set_health(&mut sim, p, 125.0);
    assert_eq!(hit(&mut sim, None, p, 100.0, DamageKind::Bullet), 80.0, "the feign over: the cloak's cut");
}

// ---- 4. Knives ----

#[test]
fn your_eternal_rewards_backstab_kill_disguises_as_the_victim() {
    let mut sim = sim();
    let from = greybox::SPAWNS[0];
    let (p, knife) = holding(&mut sim, from, "tf2:your_eternal_reward");
    let w = carry(&mut sim, p, "tf2:invis_watch");
    sim.app.world_mut().entity_mut(p).insert(Team(1));
    let (t, _) = holding(&mut sim, from - Vec3::Z * 40.0 * U, ROCKET_LAUNCHER);
    sim.app.world_mut().entity_mut(t).insert((Team(2), Name::new("Sollie")));
    hold_still(&mut sim, t);
    set_health(&mut sim, t, 90.0);
    assert_eq!(sim.app.world().get::<Inventory>(p).unwrap().active, Some(knife));
    let at = sim.position(t);
    testkit::aim_at(&mut sim, p, at);
    let yaw = sim.intent(p).yaw;
    run(&mut sim, p, 20, |s, k| {
        s.intent(t).yaw = yaw;
        s.intent(p).fire = k == 0
    });
    assert!(health_points(&sim, t) <= 0.0, "a backstab kill");
    sim.seconds(0.3);
    let d = spy_state(&sim, p).disguise.expect("disguised as the victim");
    assert_eq!((d.class.as_str(), d.team, d.name.as_str(), d.health), ("soldier", 2, "Sollie", 200.0));
    sim.ticks(2);
    assert!((speed(&sim, p) - 240.0).abs() < 0.01, "the Soldier's speed: {}", speed(&sim, p));
    // A normal disguise needs a full meter and empties it.
    set_meter(&mut sim, w, 60.0);
    assert!(spy::disguise(sim.app.world_mut(), p, "heavy", Some(2)).is_err(), "60: refused");
    set_meter(&mut sim, w, 100.0);
    assert!(spy::disguise(sim.app.world_mut(), p, "heavy", Some(2)).is_ok());
    assert!(cloak(&sim, w).meter < 1.0, "emptied");
    // Its drain x 1.33.
    set_meter(&mut sim, w, 100.0);
    press_alt(&mut sim, p);
    let m0 = cloak(&sim, w).meter;
    sim.seconds(1.0);
    let lost = m0 - cloak(&sim, w).meter;
    assert!((lost - 13.3).abs() < 0.5, "13.3 a second: {lost}");
}

#[test]
fn the_big_earners_kill_gives_30_cloak_and_a_3_s_boost() {
    let mut sim = sim();
    let (p, _) = holding(&mut sim, greybox::SPAWNS[0], "tf2:big_earner");
    let w = carry(&mut sim, p, "tf2:invis_watch");
    sim.ticks(2);
    assert_eq!(sim.app.world().get::<Health>(p).unwrap().max * 100.0, 100.0, "max health 100");
    let t = sim.spawn_character(greybox::SPAWNS[1], tf2::game::movement::ID);
    sim.seconds(0.5);
    set_health(&mut sim, t, 10.0);
    set_meter(&mut sim, w, 20.0);
    hit(&mut sim, Some(p), t, 50.0, DamageKind::Melee);
    assert!(health_points(&sim, t) <= 0.0);
    let m = cloak(&sim, w).meter;
    assert!((m - 50.0).abs() < 0.5, "20 + 30: {m}");
    sim.ticks(2);
    assert!((speed(&sim, p) - 425.0).abs() < 0.5, "{}", speed(&sim, p));
    sim.seconds(3.1);
    assert!((speed(&sim, p) - 320.0).abs() < 0.5, "3 s: {}", speed(&sim, p));
}

#[test]
fn the_spy_cicle_takes_the_first_fire_hit_then_melts() {
    let mut sim = sim();
    let (p, knife) = holding(&mut sim, greybox::SPAWNS[0], "tf2:spy_cicle");
    let gun = carry(&mut sim, p, REVOLVER);
    set_health(&mut sim, p, 125.0);
    assert_eq!(hit(&mut sim, None, p, 10.0, DamageKind::Burn), 10.0, "the first fire hit in full");
    let has = |sim: &Sim, c: &'static str| sim.app.world().resource::<Conditions>().has(p, Condition::Game(c), now(sim));
    assert!(has(&sim, spy::FIRE_IMMUNE) && has(&sim, spy::AFTERBURN_IMMUNE));
    assert_eq!(hit(&mut sim, None, p, 10.0, DamageKind::Burn), 0.0, "fire-immune");
    sim.seconds(1.0);
    assert_eq!(sim.app.world().get::<Inventory>(p).unwrap().active, Some(gun), "melted: put away");
    assert!(!has(&sim, spy::FIRE_IMMUNE), "1 s");
    assert!(has(&sim, spy::AFTERBURN_IMMUNE), "10 s");
    sim.app.world_mut().get_mut::<Inventory>(p).unwrap().wanted = Some(knife);
    sim.seconds(1.0);
    assert_eq!(sim.app.world().get::<Inventory>(p).unwrap().active, Some(gun), "no knife before 15 s");
    sim.seconds(13.5);
    sim.app.world_mut().get_mut::<Inventory>(p).unwrap().wanted = Some(knife);
    sim.seconds(1.0);
    assert_eq!(sim.app.world().get::<Inventory>(p).unwrap().active, Some(knife), "back after 15 s");
}

// ---- 5. Disguise ----

#[test]
fn a_disguise_takes_2_s_then_0_5_s_slows_to_the_class_and_ends_on_firing() {
    let mut sim = sim();
    let (p, gun, _) = a_spy(&mut sim, Some("tf2:invis_watch"));
    let (heavy, _) = holding(&mut sim, greybox::SPAWNS[1], MINIGUN);
    sim.app.world_mut().entity_mut(heavy).insert((Team(2), Name::new("Hoovy")));
    set_health(&mut sim, heavy, 150.0);
    sim.ticks(2);
    spy::disguise(sim.app.world_mut(), p, "heavy", None).unwrap();
    sim.seconds(1.9);
    assert!(spy_state(&sim, p).disguise.is_none(), "not before 2 s");
    sim.seconds(0.15);
    let d = spy_state(&sim, p).disguise.expect("disguised");
    assert_eq!((d.class.as_str(), d.team, d.name.as_str(), d.health), ("heavy", 2, "Hoovy", 150.0));
    let pose = sim.app.world().get::<Guise>(p).unwrap().pose.clone().expect("posed");
    assert_eq!(pose.weapon.as_deref(), Some(MINIGUN), "the class's stock primary");
    assert!((speed(&sim, p) - 230.0).abs() < 0.01, "the Heavy's speed: {}", speed(&sim, p));
    // Damage and the cloak keep it; cloaked, his own speed.
    hit(&mut sim, None, p, 20.0, DamageKind::Bullet);
    press_alt(&mut sim, p);
    sim.ticks(2);
    assert!(spy_state(&sim, p).disguised());
    assert!((speed(&sim, p) - 320.0).abs() < 0.01, "cloaked: his own");
    sim.seconds(0.6);
    press_alt(&mut sim, p);
    sim.seconds(2.1);
    // Again while disguised: 0.5 s.
    spy::disguise(sim.app.world_mut(), p, "scout", Some(2)).unwrap();
    sim.seconds(0.55);
    assert_eq!(spy_state(&sim, p).disguise.map(|d| d.class), Some("scout".into()));
    assert!((speed(&sim, p) - 320.0).abs() < 0.01, "a Scout's never faster");
    // Firing ends it.
    no_spread(&mut sim, gun);
    run(&mut sim, p, 3, |s, k| s.intent(p).fire = k == 0);
    let s = spy_state(&sim, p);
    assert!(!s.disguised(), "gone on firing");
    assert!(s.wear_off > now(&sim), "wearing off");
}

#[test]
fn the_enforcers_shot_while_disguised_deals_x_1_2() {
    let mut sim = sim();
    let (p, gun) = holding(&mut sim, greybox::SPAWNS[0], "tf2:enforcer");
    sim.app.world_mut().entity_mut(p).insert(Team(1));
    no_spread(&mut sim, gun);
    let (t, _) = holding(&mut sim, greybox::SPAWNS[0] - Vec3::Z * 300.0 * U, MINIGUN);
    sim.app.world_mut().entity_mut(t).insert(Team(2));
    hold_still(&mut sim, t);
    let spot = sim.position(t);
    let shot = |sim: &mut Sim| {
        sim.app.world_mut().get_mut::<Transform>(t).unwrap().translation = spot;
        hold_still(sim, t);
        testkit::aim_at(sim, p, spot);
        set_health(sim, t, 300.0);
        let before = health_points(sim, t);
        run(sim, p, 3, |s, k| s.intent(p).fire = k == 0);
        (before - health_points(sim, t)).round()
    };
    let plain = shot(&mut sim);
    assert!((50.0..=54.0).contains(&plain), "about 52 at 300: {plain}");
    sim.seconds(1.0);
    spy::disguise(sim.app.world_mut(), p, "soldier", None).unwrap();
    sim.seconds(2.1);
    // The bonus on a hit of its own (52 at 300): 62.
    assert_eq!(hit(&mut sim, Some(p), t, 52.0, DamageKind::Bullet), 62.0, "x 1.2 disguised");
    // A shot ends it: the next is plain.
    let first = shot(&mut sim);
    assert!(first > plain + 5.0, "the breaking shot has it: {first} of {plain}");
    sim.seconds(1.0);
    assert!(!spy_state(&sim, p).disguised());
    assert_eq!(hit(&mut sim, Some(p), t, 52.0, DamageKind::Bullet), 52.0, "the disguise gone");
}

// ---- What an enemy's client sees ----

#[test]
fn an_enemy_client_sees_the_disguise_and_not_the_cloaked_spy() {
    use chimera_net::{harness::NetSim, net::memory::LinkConditions};
    let conditions = LinkConditions { latency: std::time::Duration::from_millis(30), jitter: std::time::Duration::ZERO, loss: 0.0 };
    let mut sim = NetSim::new(WithItems, conditions, 5, 0, |app| {
        app.add_plugins(GreyboxMapPlugin).insert_resource(chimera_core::slots::Loadout { movement: tf2::game::movement::ID });
    });
    sim.add_client(|_| {});
    sim.add_client(|_| {});
    sim.set_frame(TICK_INTERVAL);
    sim.until_joined(900);
    let spy_server = sim.character_of(0).unwrap();
    let enemy_server = sim.character_of(1).unwrap();
    let gun = give(sim.server.app.world_mut(), spy_server, REVOLVER).unwrap();
    sim.server.app.world_mut().get_mut::<Inventory>(spy_server).unwrap().wanted = Some(gun);
    give(sim.server.app.world_mut(), spy_server, "tf2:invis_watch").unwrap();
    let m = give(sim.server.app.world_mut(), enemy_server, MINIGUN).unwrap();
    sim.server.app.world_mut().get_mut::<Inventory>(enemy_server).unwrap().wanted = Some(m);
    // TF2's identity, as a party's slots give a TF2 gun's holder.
    let tf2_id = || chimera_core::slots::GameIdentity(tf2::game::GAME.into());
    sim.server.app.world_mut().entity_mut(spy_server).insert((Team(1), tf2_id()));
    sim.server.app.world_mut().entity_mut(enemy_server).insert((Team(2), tf2_id()));
    sim.ticks(150);
    let active = sim.server.app.world().get::<Inventory>(spy_server).unwrap().active;
    let ids: Vec<_> = sim.server.app.world().get::<Inventory>(spy_server).unwrap().weapons.iter().map(|w| sim.server.app.world().get::<chimera_sim::weapon::Weapon>(*w).unwrap().id).collect();
    assert_eq!(active, Some(gun), "the revolver drawn: {ids:?}");
    let enemy_name = sim.server.app.world().get::<chimera_net::net::NetCharacter>(enemy_server).unwrap().name.clone();
    spy::disguise(sim.server.app.world_mut(), spy_server, "heavy", Some(2)).unwrap();
    sim.ticks(200);
    // On the enemy's client, the Spy's entity.
    let spy_owner = sim.client_id(0);
    let spy_there = {
        let w = sim.clients[1].app.world_mut();
        let mut q = w.query::<(Entity, &chimera_net::net::NetCharacter)>();
        q.iter(w).find(|(_, c)| c.owner == Some(spy_owner)).map(|(e, _)| e).expect("the Spy on the enemy's client")
    };
    let me = sim.local_player(1).unwrap();
    let w = sim.clients[1].app.world();
    let g = w.get::<Guise>(spy_there).expect("his guise sent");
    let pose = g.shown_to(w.get::<Team>(spy_there), w.get::<Team>(me)).expect("an enemy sees the pose");
    assert_eq!((pose.class.as_str(), pose.name.as_str()), ("heavy", enemy_name.as_str()), "a real enemy's name");
    assert_eq!(pose.health * 100.0, 300.0, "his health, at most the Heavy's");
    let body = chimera_world::map::BodyName::named(w.get::<chimera_world::map::BodyName>(spy_there)).map(|n| n.0.clone());
    assert_eq!(body.as_deref(), Some("models/player/heavy.mdl"), "the Heavy's body");
    assert_eq!(pose.weapon.as_deref(), Some(MINIGUN), "the minigun in hand (drawn: `player_anim::hold_weapons`)");
    // His own client sees him as he is.
    let own = sim.local_player(0).unwrap();
    assert_ne!(sim.clients[0].app.world().get::<chimera_world::map::BodyName>(own).map(|n| n.0.as_str()), Some("models/player/heavy.mdl"));
    // Cloaked: invisible to the enemy.
    {
        let mut i = sim.clients[0].app.world_mut().get_mut::<chimera_core::core::Intent>(own).unwrap();
        i.secondary = true;
    }
    sim.ticks(2);
    sim.clients[0].app.world_mut().get_mut::<chimera_core::core::Intent>(own).unwrap().secondary = false;
    sim.ticks(120);
    let w = sim.clients[1].app.world();
    assert!(w.get::<Guise>(spy_there).is_some_and(Guise::invisible), "not seen: {:?}", w.get::<Guise>(spy_there));
    assert_eq!(chimera_core::guise::drawn_opacity(w.get::<Guise>(spy_there), false), 0.0);
}

#[test]
fn the_watches_never_join_a_ladder_and_the_knives_do() {
    let set = GunNumbers::builtin().with_pack(&pack()).parse().unwrap();
    for id in ["tf2:invis_watch", "tf2:dead_ringer", "tf2:cloak_and_dagger"] {
        assert!(set.get(id).unwrap().class.is_none(), "{id}");
    }
    assert!(set.get("tf2:enforcer").unwrap().class.is_some());
    // The knives as the Knife (a kit melee).
    for id in ["tf2:your_eternal_reward", "tf2:big_earner", "tf2:spy_cicle"] {
        assert_eq!(set.get(id).unwrap().class, set.get(KNIFE).unwrap().class, "{id}");
    }
}
