//! The Hunter and our own lines (`game::campaign`, specs/halo_ce/
//! characters.md sections 2 to 7): the Hunter's row merges into its tables
//! (its pill, camera and speed; 200 body, no shield, its skin a head, its
//! shield arm taking nothing); its cannon is its own gun (its holder is the
//! Hunter whatever the setting; a drawn ladder deals it only with the
//! setting on); the dialogue slots by name, every character's chosen
//! lines named; the becoming and kill rules, and a body saying its
//! becoming line once per becoming and a kill line after a kill
//! (`voice::chatter`); the first-person view made from a body keeps only
//! the kept bones' triangles and puts the eye at the view's origin
//! (`first_person`).

use std::sync::Arc;

use bevy::prelude::*;
use chimera_core::core::BodyRegion;
use halo_ce::game::{
    campaign::{
        self, BodyKind, BodyTables, CampaignData, first_person,
        voice::{self, BodySounds, CampaignSounds, Chatter},
    },
    movement::MovementData,
    weapons::{self, rung::HUNTER_RUNG},
};

#[test]
fn the_hunters_tables() {
    let data = CampaignData::builtin();
    let t = BodyTables::build(&data, &MovementData::builtin()).unwrap();
    let base = MovementData::builtin();
    let m = &t.movement[&BodyKind::Hunter];
    assert_eq!((m.height_stand, m.height_crouch, m.radius), (0.8, 0.8, 0.24));
    assert_eq!((m.camera_stand, m.camera_crouch), (0.6, 0.3));
    let f = data.speed_factor(data.body(BodyKind::Hunter).unwrap().speed_ratio);
    assert!((f - 1.0153333).abs() < 1e-5, "{f}");
    assert!((m.run_forward - base.run_forward * f).abs() < 1e-5);
    assert_eq!((m.jump_velocity, m.air_accel), (base.jump_velocity, base.air_accel), "the Spartan's mover");
    let v = &t.vitality[&BodyKind::Hunter];
    assert_eq!((v.model.maximum_body, v.model.maximum_shield), (200.0, 0.0));
    assert!(!v.shield_covers_body());
    let head = &v.model.materials[v.material_of(Some(BodyRegion::Head))];
    assert_eq!((head.name.as_str(), head.head, head.body), ("skin", true, 1.0));
    let arm = &v.model.materials[v.material_of(Some(halo_ce::game::body::hits::SHIELD_REGION))];
    assert_eq!((arm.name.as_str(), arm.body), ("shield", 0.0));
    assert_eq!(v.model.materials[v.material_of(Some(BodyRegion::Chest))].name, "body");
    assert_eq!(v.unit.hard_ping_threshold, 0.25);
    assert_eq!(data.body(BodyKind::Hunter).unwrap().weapons, [r"weapons\fuel rod gun\hunter fuel rod"]);
}

#[test]
fn the_cannon_is_the_hunters_own_rung() {
    use chimera_sim::minigame::{
        ladder::{Ladder, OnRung},
        power::PowerReaders,
    };
    let data = CampaignData::builtin();
    assert_eq!(data.own_gun(HUNTER_RUNG), Some(BodyKind::Hunter));
    assert_eq!(data.own_gun(weapons::PLASMA_RIFLE), None);
    assert_eq!(data.own_gun(weapons::PISTOL), None, "a gun not listed is the Spartan's");
    assert!(halo_ce::game::parts::PARTS.guns.iter().any(|g| g.id == HUNTER_RUNG), "Halo lends it");
    assert!(weapons::DATA.iter().any(|(n, _)| *n == "hunter_fuel_rod.toml"));
    // Its holder wears the Hunter, campaign bodies off.
    let mut world = World::new();
    world.insert_resource(data);
    world.insert_resource(Ladder { rungs: vec![weapons::PLASMA_RIFLE, HUNTER_RUNG], ..Default::default() });
    let e = world.spawn(OnRung { index: 1, ..Default::default() }).id();
    campaign::choose(&mut world, e);
    assert_eq!(world.get::<BodyKind>(e), Some(&BodyKind::Hunter));
    assert!(world.resource::<campaign::UnitsOnDemand>().wanted.contains(&BodyKind::Hunter), "its unit asked for");
    world.entity_mut(e).insert(OnRung { index: 0, ..Default::default() });
    campaign::choose(&mut world, e);
    assert_eq!(world.get::<BodyKind>(e), None, "the plasma rifle, setting off: the Spartan");
    // A drawn ladder deals it only with the setting on.
    campaign::own_guns_drawn(&mut world, false);
    let why = |w: &World| w.resource::<PowerReaders>().exclusions.reason(HUNTER_RUNG).map(str::to_string);
    assert_eq!(why(&world).as_deref(), Some(campaign::OWN_GUN_EXCLUDED));
    campaign::own_guns_drawn(&mut world, false);
    assert_eq!(world.resource::<PowerReaders>().exclusions.0.len(), 1, "said once");
    campaign::own_guns_drawn(&mut world, true);
    assert_eq!(why(&world), None);
}

#[test]
fn the_slots_have_names_and_every_line_is_named() {
    assert_eq!(voice::slot_of("taunt"), Some(154));
    assert_eq!(voice::slot_of("Killed Enemy Player"), Some(55));
    assert_eq!(voice::slot_of("celebration"), Some(188));
    assert_eq!(voice::slot_name(179), "melee attack");
    assert_eq!(voice::slot_name(3), "slot 3");
    assert!(voice::SLOT_NAMES.windows(2).all(|w| w[0].0 < w[1].0 && w[1].0 < voice::DIALOGUE_SLOTS));
    let data = CampaignData::builtin();
    for kind in [BodyKind::Elite, BodyKind::Grunt, BodyKind::Jackal, BodyKind::Marine, BodyKind::Hunter] {
        let l = data.body(kind).and_then(|d| d.lines.as_ref()).unwrap_or_else(|| panic!("{kind:?} has lines"));
        assert!(!l.becoming.is_empty() && !l.kill.is_empty(), "{kind:?}");
    }
    assert!(CampaignData::parse("[bodies.elite]\nbiped = 'x'\ndonor_map = 'd20'\nlines = { becoming = ['no such line'] }\n").is_err());
    let c = data.chatter;
    assert_eq!((c.becoming_repeat_seconds, c.kill_chance, c.kill_cooldown_seconds), (60.0, 0.5, 10.0));
}

#[test]
fn becoming_and_kill_rules() {
    use voice::{becoming_due, kill_due};
    let (e, g) = (BodyKind::Elite, BodyKind::Grunt);
    assert!(becoming_due(None, e, None, 0.0, 60.0), "the first becoming");
    assert!(becoming_due(Some(g), e, Some(10.0), 11.0, 60.0), "another kind: always");
    assert!(becoming_due(None, e, Some(10.0), 11.0, 60.0), "after another game's character: always");
    assert!(!becoming_due(Some(e), e, Some(10.0), 69.0, 60.0), "the same again within a minute");
    assert!(becoming_due(Some(e), e, Some(10.0), 70.0, 60.0), "a minute on");
    assert!(kill_due(None, 0.0, 0.5, 10.0, 0.2));
    assert!(!kill_due(None, 0.0, 0.5, 10.0, 0.7), "the chance");
    assert!(!kill_due(Some(5.0), 9.0, 0.5, 10.0, 0.0), "the cooldown");
    assert!(kill_due(Some(5.0), 15.0, 0.5, 10.0, 0.0));
}

/// An Elite whose voice fills taunt and killed enemy player.
fn elite_voice() -> BodySounds {
    let mut s = BodySounds { lines: vec![None; voice::DIALOGUE_SLOTS], ..Default::default() };
    for (slot, path) in [(154, "taunt"), (55, "killed enemy player")] {
        s.lines[slot] = Some(path.to_string());
        s.set.library.sounds.insert(path.to_string(), halo_ce::game::audio::library::SoundInfo { entry: path.into(), longest_ms: 1000, ..Default::default() });
    }
    s
}

fn chatter_app() -> App {
    let mut app = App::new();
    let mut data = CampaignData::builtin();
    data.chatter.kill_chance = 1.0;
    app.insert_resource(data)
        .insert_resource(CampaignSounds([(BodyKind::Elite, Arc::new(elite_voice()))].into_iter().collect()))
        .init_resource::<Chatter>()
        .init_resource::<chimera_core::core::SimClock>()
        .add_message::<chimera_core::core::Died>()
        .add_message::<chimera_world::map::GameSound>()
        .add_systems(Update, voice::chatter);
    app
}

fn said(app: &mut App) -> Vec<String> {
    let mut m = app.world_mut().resource_mut::<Messages<chimera_world::map::GameSound>>();
    m.drain().map(|s| s.0.entry).collect()
}

fn died(victim: Entity, killer: Entity) -> chimera_core::core::Died {
    use chimera_core::core::{Damage, DamageKind, Hitgroup};
    let damage = Damage {
        target: victim,
        attacker: Some(killer),
        amount: 100.0,
        point: Vec3::ZERO,
        dir: Vec3::NEG_Z,
        hitgroup: Hitgroup::Generic,
        kind: DamageKind::Generic,
        weapon: None,
        force: Vec3::ZERO,
    };
    chimera_core::core::Died { entity: victim, attacker: Some(killer), damage }
}

fn pass(app: &mut App, seconds: f64) {
    app.world_mut().resource_mut::<chimera_core::core::SimClock>().now += seconds;
    // The line ends (its timers are `speak`'s; here by hand).
    let mut q = app.world_mut().query::<&mut voice::Voice>();
    for mut v in q.iter_mut(app.world_mut()) {
        v.speaking = 0;
        v.priority = 0;
    }
}

#[test]
fn a_body_says_who_it_has_become_and_boasts_after_a_kill() {
    let mut app = chatter_app();
    let e = app.world_mut().spawn((BodyKind::Elite, Transform::default(), voice::Voice::default())).id();
    let victim = app.world_mut().spawn_empty().id();
    app.update();
    assert_eq!(said(&mut app), ["taunt"], "becoming an Elite");
    pass(&mut app, 1.0);
    app.update();
    assert!(said(&mut app).is_empty(), "once per becoming");
    // A kill: its kill line (the first of its list its voice fills).
    app.world_mut().write_message(died(victim, e));
    app.update();
    assert_eq!(said(&mut app), ["killed enemy player"]);
    pass(&mut app, 1.0);
    app.world_mut().write_message(died(victim, e));
    app.update();
    assert!(said(&mut app).is_empty(), "within the cooldown");
    // Another game's character, then the Elite again: a becoming.
    app.world_mut().entity_mut(e).remove::<BodyKind>();
    app.update();
    app.world_mut().entity_mut(e).insert(BodyKind::Elite);
    pass(&mut app, 1.0);
    app.update();
    assert_eq!(said(&mut app), ["taunt"]);
    // Without its voice: silent, and the becoming waits for it.
    let g = app.world_mut().spawn((BodyKind::Grunt, Transform::default(), voice::Voice::default())).id();
    app.update();
    assert!(said(&mut app).is_empty());
    assert!(app.world().resource::<Chatter>().0[&g].pending, "waits for its voice");
}

#[test]
fn a_body_view_keeps_the_kept_bones_triangles() {
    use chimera_world::map::{MapBone, MapCharacterModel, MapMesh};
    let bone = |name: &str, parent: Option<usize>| MapBone { name: name.into(), parent, position: Vec3::ZERO, rotation: Quat::IDENTITY };
    let bones = vec![bone("pelvis", None), bone("r upperarm", Some(0)), bone("r hand", Some(1))];
    assert_eq!(first_person::kept(&bones, &["R UpperArm".into()]), [false, true, true]);
    let mesh = MapMesh {
        // In the character's frame: vertex 5 at the eye (1.8 m up).
        positions: vec![[0.0; 3], [0.0; 3], [0.0; 3], [0.0; 3], [0.0; 3], [0.0, 1.8, 0.0]],
        joints: vec![[0, 0, 0, 0], [0, 0, 0, 0], [0, 0, 0, 0], [1, 0, 0, 0], [2, 0, 0, 0], [1, 2, 0, 0]],
        joint_weights: vec![[1.0, 0.0, 0.0, 0.0]; 5].into_iter().chain([[0.5, 0.5, 0.0, 0.0]]).collect(),
        indices: vec![0, 1, 2, 3, 4, 5, 0, 3, 4],
        ..Default::default()
    };
    let model = MapCharacterModel {
        game: Some(halo_ce::game::GAME),
        team: None,
        name: Some("halo_ce:hunter".into()),
        class: None,
        model: chimera_world::map::MapModel { meshes: vec![mesh.clone(), MapMesh { indices: vec![0, 1, 2], ..mesh }], ..Default::default() },
        hitboxes: Vec::new(),
        bones,
        root: halo_ce::game::body::skeleton::root(),
        animations: None,
        boxes: Vec::new(),
        ragdoll: None,
    };
    let view = first_person::BodyView { guns: vec![HUNTER_RUNG.into()], keep: vec!["r upperarm".into()], label: "fr".into(), ..Default::default() };
    let v = first_person::view_model(&model, &view, HUNTER_RUNG, 1.8);
    assert_eq!(v.key, HUNTER_RUNG);
    assert_eq!(v.model.meshes.len(), 1, "a mesh with nothing kept goes");
    assert_eq!(v.model.meshes[0].indices, [3, 4, 5], "only the arm's triangle");
    assert!(Vec3::from(v.model.meshes[0].positions[5]).length() < 1e-4, "its meshes in the eye's space");
    // The eye (1.8 m above the feet) is the view's origin; up is up.
    let eye_in_skeleton = model.root.to_matrix().inverse().transform_point3(Vec3::Y * 1.8);
    assert!(v.root.transform_point(eye_in_skeleton).length() < 1e-4);
    let moved = first_person::root(model.root, 1.8, &first_person::BodyView { offset: [0.1, -0.2, 0.3], ..view });
    assert!((moved.transform_point(eye_in_skeleton) - Vec3::new(0.1, -0.2, 0.3)).length() < 1e-4);
}

/// The cannon's pace (`params::Trigger::charge_paces`, our rule in its
/// data): a held trigger fires a rod at the press and one each charge
/// after (37 ticks); tapping, as a bot does, fires no faster. The tag's
/// own rule (paces off): each tap fires (its 10 rounds a second).
#[test]
fn the_cannons_charge_paces_a_held_trigger_and_tapping() {
    use halo_ce::game::weapons::tick::{GunEvent, HaloGun, Inputs};
    let shots = |g: &weapons::params::Gun, press: &dyn Fn(u32) -> bool| -> Vec<u32> {
        let mut w = HaloGun::new(g);
        w.in_hand = true;
        w.released_since_shot = true;
        (0..300u32)
            .filter(|t| {
                let i = Inputs { fire: press(*t), held: true, aim: Vec3::NEG_Z, ..Default::default() };
                w.tick(g, &i).iter().any(|e| matches!(e, GunEvent::Fired { .. }))
            })
            .collect()
    };
    let gaps = |s: &[u32]| s.windows(2).map(|w| w[1] - w[0]).collect::<Vec<u32>>();
    let set = chimera_sim::weapon::data::WeaponSet::parse(weapons::DATA, &[]).unwrap();
    let g: weapons::params::Gun = set.get(HUNTER_RUNG).unwrap().hooks.get(weapons::WEAPON_HOOK).cloned().unwrap().try_into().unwrap();
    assert!(g.trigger.charge_paces);
    let charge = g.trigger.charging_ticks() as u32;
    assert_eq!(charge, 37, "1.25 s, truncated");
    let held = shots(&g, &|_| true);
    assert_eq!(held[0], 0, "the press fires at once");
    assert!(held.len() >= 7 && gaps(&held).iter().all(|d| *d == charge), "held: {held:?}");
    let tapped = shots(&g, &|t| t % 4 == 0);
    assert!(tapped.len() >= 6 && gaps(&tapped).iter().all(|d| *d >= charge && *d <= charge + 4), "tapped: {tapped:?}");
    let mut tag = g.clone();
    tag.trigger.charge_paces = false;
    let tapped = shots(&tag, &|t| t % 4 == 0);
    assert!(tapped.len() > 60, "the tag's own rule: each tap fires ({})", tapped.len());
}

/// `FxSet::merge` (the characters' own weapons joining the donor's
/// library): the other set's effects, particles and its gun's and
/// projectile's indices moved past this set's; what this set has stays.
#[test]
fn an_effects_set_merges_with_its_indices_moved() {
    use halo_ce::game::fx::load::{FxEffect, FxEntry, FxEvent, FxGun, FxParticle, FxProjectile, FxSet};
    let effect = |path: &str, particle: Option<usize>| FxEffect {
        path: path.into(),
        events: vec![FxEvent { entries: vec![FxEntry { particle, ..Default::default() }], ..Default::default() }],
        ..Default::default()
    };
    let mut base = FxSet::default();
    base.library.effects = vec![effect("a", Some(0)), effect("b", None)];
    base.library.by_path = [("a".to_string(), 0), ("b".to_string(), 1)].into();
    base.library.particles = vec![FxParticle::default()];
    base.library.guns.insert("gun".into(), FxGun { firing: vec![Some(1)], ..Default::default() });
    let mut other = FxSet::default();
    other.library.effects = vec![effect("c", Some(0)), effect("b", None)];
    other.library.by_path = [("c".to_string(), 0), ("b".to_string(), 1)].into();
    other.library.particles = vec![FxParticle::default()];
    other.library.guns.insert("gun".into(), FxGun { firing: vec![Some(0)], ..Default::default() });
    other.library.guns.insert("cannon".into(), FxGun { firing: vec![Some(0)], impacts: vec![None, Some(1)], ..Default::default() });
    other.library.projectiles.insert("rod".into(), FxProjectile { detonation: Some(0), attached: vec![(1, "x".into())], ..Default::default() });
    base.merge(&other);
    let lib = &base.library;
    assert_eq!(lib.effects.len(), 4);
    assert_eq!(lib.effects[2].events[0].entries[0].particle, Some(1));
    assert_eq!((lib.by_path["c"], lib.by_path["b"]), (2, 1), "a path already there stays");
    assert_eq!(lib.guns["gun"].firing, [Some(1)], "a gun already there stays");
    assert_eq!(lib.guns["cannon"].firing, [Some(2)]);
    assert_eq!(lib.guns["cannon"].impacts, [None, Some(3)]);
    assert_eq!(lib.projectiles["rod"].detonation, Some(2));
    assert_eq!(lib.projectiles["rod"].attached[0].0, 3);
}
