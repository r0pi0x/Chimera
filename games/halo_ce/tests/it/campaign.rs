//! The campaign's bodies (`game::campaign`, specs/halo_ce/characters.md):
//! data/campaign.toml parses and merges into the Elite's movement and
//! vitality tables (its pill, camera, speeds; 100 body and shield, its
//! 12 s delay, its arms' material) while the Spartan's stay as they are;
//! the guns' weights; the draw is the same for the same roll, gives the
//! Spartan when the Elite's unit is not loaded and splits the plasma
//! rifle's rolls by its weights; `movement_of` and `vitality_of` pick the
//! kind's table; a kind names its model on the character and takes the
//! name off again (`name_bodies`). The kinds as sandbox targets
//! (`targets`); a kind worn before its unit loaded is worn within two
//! frames of the load; the pain, death, scream and maimed lines' rules
//! (`voice`); a campaign body is never the table's black.

use bevy::prelude::*;
use chimera_core::core::BodyRegion;
use halo_ce::game::{
    campaign::{self, BodyKind, BodyTables, CampaignData, NamedByKind},
    movement::MovementData,
    vitality::Spartan,
};

fn tables() -> (CampaignData, BodyTables) {
    let data = CampaignData::builtin();
    let t = BodyTables::build(&data, &MovementData::builtin()).unwrap();
    (data, t)
}

#[test]
fn the_elite_tables_merge_over_the_spartans() {
    let (_, t) = tables();
    let base = MovementData::builtin();
    let m = &t.movement[&BodyKind::Elite];
    assert_eq!((m.height_stand, m.height_crouch, m.radius), (0.8, 0.5, 0.2));
    assert_eq!((m.camera_stand, m.camera_crouch), (0.6, 0.3));
    // The owner's rule: halfway to its own speed (115 %), 107.5 % of the
    // Spartan's, crouched too; everything else the Spartan's.
    let near = |a: f32, b: f32| (a - b).abs() < 1e-5;
    assert!(near(m.run_forward, 2.25 * 1.075) && near(m.run_backward, 2.0 * 1.075), "{} {}", m.run_forward, m.run_backward);
    assert!(near(m.sneak_forward, base.sneak_forward * 1.075));
    assert_eq!((m.soft_land_min, m.hard_land_min, m.max_slope_cos), (base.soft_land_min, base.hard_land_min, base.max_slope_cos));
    assert_eq!((m.gravity, m.jump_velocity, m.run_accel, m.air_accel), (base.gravity, base.jump_velocity, base.run_accel, base.air_accel));
    assert_eq!(t.animation[&BodyKind::Elite].front, 2.595);
    let v = &t.vitality[&BodyKind::Elite];
    assert_eq!((v.model.maximum_body, v.model.maximum_shield), (100.0, 100.0));
    assert_eq!(v.model.shield_material_type, 30);
    assert_eq!(v.shield_delay(), 360);
    assert_eq!(v.material_of(Some(BodyRegion::Head)), 0);
    assert_eq!(v.model.materials[v.material_of(Some(BodyRegion::LeftForearm))].body, 0.5);
    assert_eq!(v.model.materials[v.material_of(Some(BodyRegion::RightShin))].body, 0.8);
    assert_eq!(v.model.materials[v.material_of(Some(BodyRegion::Chest))].name, "body");
    let spartan = Spartan::load();
    assert_eq!(spartan.model.maximum_body, 75.0);
    assert_eq!(spartan.material_of(Some(BodyRegion::LeftForearm)), spartan.material_of(Some(BodyRegion::Chest)));
}

/// A tuning pass is data: a row's movement keys and the blend.
#[test]
fn movement_numbers_are_data() {
    let text = "speed_blend = 1.0
[bodies.elite]
biped = 'b'
donor_map = 'd'
speed_ratio = 0.5
                [bodies.elite.movement]
run_accel = 0.1
jump_velocity = 0.05
air_accel = 0.0
";
    let data = CampaignData::parse(text).unwrap();
    let m = data.movement(BodyKind::Elite, &MovementData::builtin()).unwrap();
    assert_eq!((m.run_accel, m.jump_velocity, m.air_accel), (0.1, 0.05, 0.0));
    assert!((m.run_forward - 1.125).abs() < 1e-6, "{}", m.run_forward);
}

/// The legs keep up: a move animation advances the ground speed over its
/// own speed in frames a tick.
#[test]
fn move_animations_play_at_the_ground_speed() {
    use halo_ce::game::body::anims::{Inputs, State};
    let mut i = Inputs { own_speeds: Some([2.0, 1.0, 1.0, 0.5, 0.5, 0.5]), speed: 3.0, ..Default::default() };
    assert_eq!(i.rate(State::MoveFront), 1.5);
    assert_eq!(i.rate(State::MoveBack), 3.0);
    assert_eq!(i.rate(State::Idle), 1.0);
    i.crouching = true;
    assert_eq!(i.rate(State::MoveFront), 3.0);
    assert_eq!(Inputs { speed: 3.0, ..Default::default() }.rate(State::MoveFront), 1.0, "the Spartan: a frame a tick");
}

#[test]
fn the_seam_picks_the_kinds_table() {
    let (_, t) = tables();
    let base = MovementData::builtin();
    assert_eq!(campaign::movement_of(&base, Some(&t), None).height_stand, 0.7);
    assert_eq!(campaign::movement_of(&base, Some(&t), Some(&BodyKind::Spartan)).height_stand, 0.7);
    assert_eq!(campaign::movement_of(&base, Some(&t), Some(&BodyKind::Elite)).height_stand, 0.8);
    assert_eq!(campaign::movement_of(&base, None, Some(&BodyKind::Elite)).height_stand, 0.7);
    let s = Spartan::load();
    assert_eq!(campaign::vitality_of(&s, Some(&t), Some(&BodyKind::Elite)).model.maximum_shield, 100.0);
}

#[test]
fn guns_name_who_carries_them() {
    let data = CampaignData::builtin();
    use BodyKind::{Elite, Grunt, Jackal, Marine, Spartan};
    assert_eq!(data.weights("halo_ce:plasma_rifle"), vec![(Elite, 1.0), (Marine, 1.0), (Spartan, 1.0)]);
    assert_eq!(data.weights("halo_ce:needler"), vec![(Elite, 1.0), (Grunt, 1.0), (Marine, 1.0), (Spartan, 1.0)]);
    assert_eq!(data.weights("halo_ce:plasma_pistol"), vec![(Grunt, 1.0), (Jackal, 1.0), (Spartan, 1.0)]);
    for gun in ["halo_ce:assault_rifle", "halo_ce:shotgun", "halo_ce:sniper_rifle"] {
        assert_eq!(data.weights(gun), vec![(Marine, 1.0), (Spartan, 1.0)], "{gun}");
    }
    assert_eq!(data.weights("halo_ce:pistol"), vec![(Spartan, 1.0)], "the Marine animates it, the campaign never gives it");
    assert!(CampaignData::parse("[guns.\"halo_ce:needler\"]\nflood = 1.0\n").is_err(), "an unknown body");
}

#[test]
fn the_draw_follows_the_weights_and_the_units_loaded() {
    let w = CampaignData::builtin().weights("halo_ce:plasma_rifle");
    let all = |_: BodyKind| true;
    let r = campaign::roll(&[7, 0, 42, 3, 0]);
    assert_eq!(r, campaign::roll(&[7, 0, 42, 3, 0]));
    assert_ne!(r, campaign::roll(&[7, 0, 42, 3, 1]));
    assert_eq!(campaign::draw(&w, all, r), campaign::draw(&w, all, r));
    let elites = (0..1000u64).filter(|i| campaign::draw(&w, all, campaign::roll(&[*i])) == BodyKind::Elite).count();
    assert!((250..420).contains(&elites), "{elites} of 1000: a third (Elite, Marine, Spartan)");
    let spartan_only = |k: BodyKind| k == BodyKind::Spartan;
    assert!((0..100u64).all(|i| campaign::draw(&w, spartan_only, campaign::roll(&[i])) == BodyKind::Spartan));
    assert_eq!(campaign::draw(&[], all, r), BodyKind::Spartan);
}

#[test]
fn a_kind_names_its_model() {
    let mut app = App::new();
    app.add_systems(Update, campaign::name_bodies);
    let e = app.world_mut().spawn(BodyKind::Elite).id();
    app.update();
    assert_eq!(app.world().get::<chimera_world::map::BodyName>(e).map(|n| n.0.as_str()), Some("halo_ce:elite"));
    app.world_mut().entity_mut(e).insert(BodyKind::Spartan);
    app.update();
    assert!(app.world().get::<chimera_world::map::BodyName>(e).is_none());
    assert!(app.world().get::<NamedByKind>(e).is_none());
}

/// sounds.md "Pain line choice" and "Death line choice" for a worn body
/// (`campaign::voice`): the death lines by cause, one major pain per life,
/// at most 3 minor lines counted, the fallbacks.
#[test]
fn a_worn_body_says_pain_and_death_lines_by_the_rules() {
    use halo_ce::game::{campaign::voice::{self, Voice}, vitality::data::Category};
    let mut v = Voice::default();
    assert_eq!(voice::choose(&mut v, true, Category::Falling, 0.0, 0.0, true, 0.0), Some(voice::DEATH_FALLING));
    assert_eq!(voice::choose(&mut v, true, Category::Bullet, 2.5, 0.0, true, 0.0), Some(voice::DEATH_FLYING));
    assert_eq!(voice::choose(&mut v, true, Category::Bullet, 0.0, 0.0, true, 0.0), Some(voice::DEATH_QUIET));
    let mut v = Voice::default();
    assert_eq!(voice::choose(&mut v, false, Category::Bullet, 0.0, 0.1, true, 0.9), Some(voice::PAIN_BODY_MINOR));
    assert_eq!(voice::choose(&mut v, false, Category::Bullet, 0.0, 0.1, true, 0.0), None, "the minor timer");
    for _ in 0..15 {
        voice::step(&mut v);
    }
    assert_eq!(voice::choose(&mut v, false, Category::Bullet, 0.0, 0.1, false, 0.0), Some(voice::PAIN_SHIELD));
    assert_eq!(voice::choose(&mut v, false, Category::Bullet, 0.0, 0.7, true, 0.0), Some(voice::PAIN_BODY_MAJOR));
    assert_eq!(voice::choose(&mut v, false, Category::Bullet, 0.0, 0.7, true, 0.0), None, "one major line a life");
    let s = voice::BodySounds {
        lines: (0..voice::SLOTS).map(|i| (i == 6 || i == 15).then(|| format!("s{i}"))).collect(),
        ..Default::default()
    };
    assert_eq!(s.line(voice::PAIN_SHIELD), Some("s6"));
    assert_eq!(s.line(voice::DEATH_FALLING), Some("s15"), "death falling, then quiet, then violent");
    assert_eq!(s.line(voice::SCREAM_FEAR), None);
}

/// The Grunt's and the Jackal's rows: speeds by the blend (92 %, 80.5 %),
/// pills and cameras (the Grunt's below its head), vitality (the Grunt no
/// shield and no armour to other games; the Jackal's shield only on its
/// disc's material, which never hurts the body).
#[test]
fn the_grunt_and_the_jackal_tables() {
    let (_, t) = tables();
    let base = MovementData::builtin();
    let near = |a: f32, b: f32| (a - b).abs() < 1e-5;
    let g = &t.movement[&BodyKind::Grunt];
    assert!(near(g.run_forward, base.run_forward * 0.92), "{}", g.run_forward);
    assert_eq!((g.height_stand, g.camera_stand, g.camera_crouch), (0.55, 0.28, 0.28));
    let j = &t.movement[&BodyKind::Jackal];
    assert!(near(j.run_forward, base.run_forward * 0.805), "{}", j.run_forward);
    assert_eq!((j.radius, j.height_crouch), (0.18, 0.45));
    let gv = &t.vitality[&BodyKind::Grunt];
    assert_eq!((gv.model.maximum_body, gv.model.maximum_shield), (40.0, 0.0));
    assert!(!gv.shield_covers_body());
    assert_eq!(gv.model.materials[gv.material_of(Some(BodyRegion::Head))].name, "head");
    assert_eq!(gv.model.materials[gv.material_of(Some(BodyRegion::Chest))].body, 0.8);
    let jv = &t.vitality[&BodyKind::Jackal];
    assert_eq!((jv.model.maximum_body, jv.model.maximum_shield, jv.shield_delay()), (60.0, 100.0, 450));
    assert!(!jv.shield_covers_body(), "its shield is the arm's");
    let disc = &jv.model.materials[jv.material_of(Some(halo_ce::game::body::hits::SHIELD_REGION))];
    assert_eq!((disc.name.as_str(), disc.shield, disc.body), ("shield", 1.0, 0.0));
    assert_eq!(jv.model.materials[jv.material_of(Some(BodyRegion::Chest))].leak, 1.0);
    assert!(Spartan::load().shield_covers_body());
}

/// The kinds as sandbox targets (`campaign::targets`): offered only for
/// their guns and once their units loaded; a fresh Elite's shield full, a
/// Grunt's bar empty, a Jackal's full (its arm's shield is its second bar);
/// wearing one forces the kind, the Spartan takes it off.
#[test]
fn the_kinds_are_sandbox_targets_once_loaded() {
    use halo_ce::game::campaign::{ForcedBody, UnitsOnDemand, targets};
    let (data, t) = tables();
    let mut world = World::new();
    world.insert_resource(data);
    world.insert_resource(t);
    world.init_resource::<UnitsOnDemand>();
    let ids: Vec<&str> = targets::characters().iter().map(|c| c.id).collect();
    assert_eq!(ids, ["halo_ce:elite", "halo_ce:grunt", "halo_ce:jackal", "halo_ce:marine", "halo_ce:hunter"]);
    assert!(!targets::holds(&world, "halo_ce:elite", "halo_ce:plasma_rifle"), "not before its unit loaded");
    world.resource_mut::<UnitsOnDemand>().mark_ready(BodyKind::Elite);
    assert!(targets::holds(&world, "halo_ce:elite", "halo_ce:plasma_rifle"));
    assert!(targets::holds(&world, "halo_ce:elite", "halo_ce:needler"));
    assert!(!targets::holds(&world, "halo_ce:elite", "halo_ce:pistol"), "not a gun the campaign gives it");
    assert!(!targets::holds(&world, "halo_ce:jackal", "halo_ce:plasma_pistol"), "the Jackal's unit is not loaded");
    assert_eq!(targets::vitals(&world, "halo_ce:elite"), (1.0, 1.0));
    assert_eq!(targets::vitals(&world, "halo_ce:grunt"), (1.0, 0.0));
    assert_eq!(targets::vitals(&world, "halo_ce:jackal"), (1.0, 1.0));
    let e = world.spawn_empty().id();
    targets::wear(&mut world, e, "halo_ce:grunt");
    assert_eq!((world.get::<BodyKind>(e), world.get::<ForcedBody>(e)), (Some(&BodyKind::Grunt), Some(&ForcedBody(BodyKind::Grunt))));
    assert!(world.resource::<UnitsOnDemand>().wanted.contains(&BodyKind::Grunt), "its unit asked for");
    targets::wear_spartan(&mut world, e, "halo_ce:spartan");
    assert!(world.get::<BodyKind>(e).is_none() && world.get::<ForcedBody>(e).is_none());
}

/// A kind worn before its unit loaded: the Spartan's body meanwhile, and
/// its own within two frames of the load's completion (the model added as
/// `unit::load_on_demand` adds it: `map::add_character_model`).
#[test]
fn a_kind_worn_before_its_unit_wears_it_soon_after_the_load() {
    use std::sync::Arc;
    use chimera_world::map::{BodyModel, CharacterModels, MapCharacterModel, add_character_model, append_late_characters, attach_hitboxes};
    let model = |name: Option<&str>| MapCharacterModel {
        game: Some(halo_ce::game::GAME),
        team: None,
        name: name.map(str::to_string),
        class: None,
        model: Default::default(),
        hitboxes: Vec::new(),
        bones: Vec::new(),
        root: Transform::IDENTITY,
        animations: None,
        boxes: Vec::new(),
        ragdoll: None,
    };
    let mut app = App::new();
    app.add_systems(PreUpdate, append_late_characters);
    app.add_systems(Update, (campaign::name_bodies, attach_hitboxes).chain());
    app.insert_resource(CharacterModels(Arc::new(vec![model(None)])));
    let e = app
        .world_mut()
        .spawn((chimera_core::core::Intent::default(), chimera_core::slots::GameIdentity(halo_ce::game::GAME.into()), BodyKind::Elite))
        .id();
    for _ in 0..5 {
        app.update();
    }
    assert_eq!(app.world().get::<BodyModel>(e), Some(&BodyModel(0)), "the Spartan's until its unit is in");
    add_character_model(app.world_mut(), model(Some("halo_ce:elite")), Arc::new(Vec::new()));
    let frames = (1..=10).find(|_| {
        app.update();
        app.world().get::<BodyModel>(e) == Some(&BodyModel(1))
    });
    assert!(frames.is_some_and(|n| n <= 2), "worn {frames:?} frames after the load");
}

/// The involuntary lines (characters.md section 6, items 2 and 3;
/// `campaign::voice`): a dangerous fall screams once per fall, only past
/// the speed a landing starts to hurt; a lost region is maimed limb or
/// head; a line outranks one playing only by priority, and once dead only
/// a death line starts.
#[test]
fn a_worn_body_screams_on_a_dangerous_fall_and_when_maimed() {
    use halo_ce::game::campaign::voice::{self, Voice};
    let mut v = Voice::default();
    assert_eq!(voice::scream(&mut v, true, 0.5, 0.6), None, "not yet fast enough to hurt");
    assert_eq!(voice::scream(&mut v, true, 0.7, 0.6), Some(voice::SCREAM_FEAR));
    assert_eq!(voice::scream(&mut v, true, 0.9, 0.6), None, "once a fall");
    assert_eq!(voice::scream(&mut v, false, 0.0, 0.6), None);
    assert_eq!(voice::scream(&mut v, true, 0.7, 0.6), Some(voice::SCREAM_FEAR), "the next fall");
    assert_eq!((voice::maimed(false), voice::maimed(true)), (voice::MAIMED_LIMB, voice::MAIMED_HEAD));
    let s = voice::BodySounds {
        lines: (0..voice::SLOTS).map(|i| (i == 10 || i == 12).then(|| format!("s{i}"))).collect(),
        ..Default::default()
    };
    assert_eq!(s.line(voice::SCREAM_PAIN), Some("s10"));
    assert_eq!(s.line(voice::MAIMED_HEAD), Some("s12"));
    let speaking = |priority| Voice { speaking: 20, priority, ..Default::default() };
    assert!(voice::may_start(&speaking(voice::PAIN), voice::INVOLUNTARY), "a scream over a pain line");
    assert!(!voice::may_start(&speaking(voice::INVOLUNTARY), voice::PAIN));
    assert!(!voice::may_start(&speaking(voice::INVOLUNTARY), voice::INVOLUNTARY));
    assert!(voice::may_start(&speaking(voice::INVOLUNTARY), voice::DEATH));
    let dead = Voice { dead: true, ..Default::default() };
    assert!(!voice::may_start(&dead, voice::INVOLUNTARY) && voice::may_start(&dead, voice::DEATH));
    let mut fallen = Voice { dead: true, ..Default::default() };
    assert_eq!(voice::scream(&mut fallen, true, 2.0, 0.6), None, "the dead don't scream");
}

/// A campaign body (a colour of its own after the table's) wears its own
/// colour where the table gives black (a silhouette otherwise); the
/// Spartan keeps the table's.
#[test]
fn a_campaign_body_is_never_black() {
    use std::sync::Arc;
    use chimera_world::map::{BodyModel, BodySkin, CharacterModels, MapCharacterModel};
    use halo_ce::game::body::{self, COLOURS};
    let model = |skins: usize| {
        let mut m = MapCharacterModel {
            game: Some(halo_ce::game::GAME),
            team: None,
            name: None,
            class: None,
            model: Default::default(),
            hitboxes: Vec::new(),
            bones: Vec::new(),
            root: Transform::IDENTITY,
            animations: None,
            boxes: Vec::new(),
            ragdoll: None,
        };
        m.model.skins = vec![Vec::new(); skins];
        m
    };
    let mut app = App::new();
    app.add_systems(Update, body::wear_colours);
    app.insert_resource(CharacterModels(Arc::new(vec![model(COLOURS.len()), model(COLOURS.len() + 1)])));
    // Team 4: a free-for-all player's second profile colour, the table's black.
    let black = chimera_core::core::Team(4);
    assert_eq!(body::skin_of(Some(black)), body::BLACK);
    let spartan = app.world_mut().spawn((BodyModel(0), black)).id();
    let elite = app.world_mut().spawn((BodyModel(1), black)).id();
    let red = app.world_mut().spawn((BodyModel(1), chimera_core::core::Team(1))).id();
    app.update();
    let skin = |e| app.world().get::<BodySkin>(e).map(|s| s.0);
    assert_eq!(skin(spartan), Some(body::BLACK as i32));
    assert_eq!(skin(elite), Some(COLOURS.len() as i32));
    assert_eq!(skin(red), Some(body::RED as i32));
}


/// The Marine's row (characters.md sections 3 to 6): 96.7 % of the
/// Spartan's speed with the blend, its pill and camera, 50 body and a
/// 50-point failing shield whose materials leak 20 %, its four voices.
#[test]
fn the_marines_tables() {
    let (data, t) = tables();
    let base = MovementData::builtin();
    let m = &t.movement[&BodyKind::Marine];
    assert!((m.run_forward - base.run_forward * (1.0 + 0.5 * (0.9333333 - 1.0))).abs() < 1e-5, "{}", m.run_forward);
    assert_eq!((m.height_stand, m.height_crouch, m.radius, m.camera_stand, m.camera_crouch), (0.64, 0.5, 0.18, 0.62, 0.44));
    let v = &t.vitality[&BodyKind::Marine];
    assert_eq!((v.model.maximum_body, v.model.maximum_shield, v.shield_delay()), (50.0, 50.0, 240));
    assert_eq!((v.model.shield_failure_threshold, v.model.failing_shield_leak), (1.0, 0.2));
    assert!(v.shield_covers_body(), "its shield leaks 20 %, it does not leak all");
    assert!(v.model.materials[v.material_of(Some(BodyRegion::Head))].head);
    assert_eq!(data.body(BodyKind::Marine).unwrap().voices, ["bisenti", "fitzgerald", "aussie", "mendoza"]);
}

/// A body of a kind with several voices gets one, the same for the same
/// player and match, and a kind with one voice none (`draw_voices`).
#[test]
fn a_marine_is_given_one_of_its_voices() {
    use halo_ce::game::campaign::BodyVoice;
    let draw = |seed: u64| {
        let mut app = App::new();
        app.insert_resource(CampaignData::builtin());
        app.add_systems(Update, campaign::draw_voices);
        let m = app.world_mut().spawn((BodyKind::Marine, chimera_core::core::Seed(seed))).id();
        let e = app.world_mut().spawn((BodyKind::Elite, chimera_core::core::Seed(seed))).id();
        app.update();
        assert!(app.world().get::<BodyVoice>(e).is_none(), "one voice: none to draw");
        app.world().get::<BodyVoice>(m).copied().expect("a voice")
    };
    assert_eq!(draw(7), draw(7));
    let drawn: std::collections::BTreeSet<u8> = (0..64).map(|s| draw(s).0).collect();
    assert_eq!(drawn.len(), 4, "every voice comes up: {drawn:?}");
}

#[test]
fn campaign_bodies_are_on_by_default() {
    assert!(halo_ce::game::campaign::CampaignBodies::default().0, "the host setting defaults to 1");
}
