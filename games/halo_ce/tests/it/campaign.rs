//! The campaign's bodies (`game::campaign`, specs/halo_ce/characters.md):
//! data/campaign.toml parses and merges into the Elite's movement and
//! vitality tables (its pill, camera, speeds; 100 body and shield, its
//! 12 s delay, its arms' material) while the Spartan's stay as they are;
//! the guns' weights; the draw is the same for the same roll, gives the
//! Spartan when the Elite's unit is not loaded and splits the plasma
//! rifle's rolls by its weights; `movement_of` and `vitality_of` pick the
//! kind's table; a kind names its model on the character and takes the
//! name off again (`name_bodies`).

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
    assert_eq!(data.weights("halo_ce:plasma_rifle"), vec![(BodyKind::Elite, 1.0), (BodyKind::Spartan, 1.0)]);
    assert_eq!(data.weights("halo_ce:needler"), vec![(BodyKind::Elite, 1.0), (BodyKind::Spartan, 1.0)]);
    assert_eq!(data.weights("halo_ce:assault_rifle"), vec![(BodyKind::Spartan, 1.0)]);
    assert!(CampaignData::parse("[guns.\"halo_ce:needler\"]\ngrunt = 1.0\n").is_err(), "an unknown body");
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
    assert!((400..600).contains(&elites), "{elites} of 1000");
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
