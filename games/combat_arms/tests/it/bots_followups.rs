//! specs/combat_arms/bots.md's follow-up cases (45-57: the answers to its
//! open questions 3, 6, 11, 14, 16-24), where our bots play them
//! (`case_46_` is test case 46): the rules as functions, and the scenarios
//! on `CaSim`.

use bevy::prelude::*;
use chimera_core::core::{Bot, DamageKind};
use chimera_world::map::MapEntity;
use combat_arms::game::{
    bots::{
        BotData,
        chat::{CaChat, Event},
        map_ai::MapAi,
        math::{self, Sense},
    },
    rules::spawns,
};

use crate::mode_kit::*;

/// 46. The bot's fire interval by weapon class (its fire animation's
/// length): rifle 100 ms, automatic shotgun 466, sniper 1000, knife 500.
#[test]
fn case_46_fire_interval() {
    assert_eq!(math::fire_interval_ms(0), 100);
    assert_eq!(math::fire_interval_ms(6), 466);
    assert_eq!(math::fire_interval_ms(5), 1000);
    assert_eq!(math::fire_interval_ms(14), 500);
    assert_eq!(math::fire_interval_ms(27), 1000, "the double barrel");
    assert_eq!(math::fire_interval_ms(12), 1200, "rocket launchers");
    assert_eq!(math::fire_interval_ms(28), 133, "the chain gun");
}

/// 47. The templates' speeds (data): Normal run 270 and jump 300 (the
/// jump-over's too), Veteran run 300 and jump 315.
#[test]
fn case_47_speeds() {
    let d = BotData::default();
    let t = &d.template;
    use combat_arms::game::bots::data::{Level, Template};
    // The data's "Normal" templates are shown as Easy.
    let n = Template::new(Level::Easy, 0);
    let x = Template::new(Level::Veteran, 0);
    assert_eq!((t.run_speed.at(n), t.jump_speed.at(n)), (270.0, 300.0), "Normal");
    assert_eq!((t.run_speed.at(x), t.jump_speed.at(x)), (300.0, 315.0), "Veteran");
}

fn volume(name: &str, class: &str, min: [f32; 3], max: [f32; 3], only_down: bool) -> MapEntity {
    let c = [
        (min[0] + max[0]) / 2.0,
        (min[1] + max[1]) / 2.0,
        (min[2] + max[2]) / 2.0,
    ];
    let h = [
        (max[0] - min[0]) / 2.0,
        (max[1] - min[1]) / 2.0,
        (max[2] - min[2]) / 2.0,
    ];
    // The entity's vectors are the world file's (x z y in the entity).
    MapEntity {
        keyvalues: vec![
            ("classname".into(), class.into()),
            ("targetname".into(), name.into()),
            ("origin".into(), format!("{} {} {}", c[0], c[2], c[1])),
            ("Dims".into(), format!("{} {} {}", h[0], h[2], h[1])),
            ("OnlyJumpDown".into(), if only_down { "1" } else { "0" }.into()),
        ],
        ..default()
    }
}

/// 48. A jump-down volume links one way: from its upper neighbour down to
/// the lower one, never upward.
#[test]
fn case_48_jump_down_volume() {
    let ents = [
        volume("upper", "AIVolume", [0.0, 100.0, 0.0], [200.0, 120.0, 200.0], false),
        volume("jump", "AIVolumeJumpUp", [180.0, 0.0, 0.0], [260.0, 110.0, 200.0], true),
        volume("lower", "AIVolume", [240.0, -20.0, 0.0], [600.0, 10.0, 200.0], false),
    ];
    let ai = MapAi::read(&ents, 1.0);
    assert!(ai.volumes[1].only_jump_down);
    let mesh = ai.nav_mesh(1.0, |_, _| None).unwrap();
    let links = |i: usize| mesh.areas[i].links.iter().map(|l| l.0).collect::<Vec<_>>();
    assert!(links(0).contains(&1), "upper into the drop");
    assert!(links(1).contains(&2), "the drop down to the lower");
    assert!(!links(2).contains(&1), "never up into it");
    assert!(!links(1).contains(&0), "nor out of it upward");
}

/// 50. Footsteps from the leg animations' keys: running at 50 and 350 ms
/// (every 300 ms), the sprint at 63 and 312, the jumps' landing at 600;
/// walking and crouch-walking silent.
#[test]
fn case_50_footstep_keys() {
    assert_eq!(math::footstep_keys("LRF"), &[50, 350]);
    assert_eq!(math::footstep_keys("LRB"), &[50, 350]);
    assert_eq!(math::footstep_keys("LRRF"), &[63, 312]);
    assert_eq!(math::footstep_keys("LSJ"), &[600]);
    assert_eq!(math::footstep_keys("LJJ"), &[600]);
    assert!(math::footstep_keys("LWF").is_empty() && math::footstep_keys("LCF").is_empty());
    assert!(math::passed_key(40, 60, 600, 50));
    assert!(!math::passed_key(50, 60, 600, 50), "once");
    assert!(math::passed_key(590, 20, 600, 600), "the wrap");
    // The run's two keys a cycle: every 300 ms at 15 ms ticks.
    let mut t = 0;
    let mut steps = Vec::new();
    for tick in 1..=80 {
        let next = (t + 15) % 600;
        if math::footstep_keys("LRF")
            .iter()
            .any(|k| math::passed_key(t, next, 600, *k))
        {
            steps.push(tick * 15);
        }
        t = next;
    }
    assert!(steps.windows(2).all(|w| w[1] - w[0] == 300), "{steps:?}");
}

/// 51. False footsteps: one step (0.5) then nothing decays below 0.5: one
/// false stimulation; the third fires the sense.
#[test]
fn case_51_false_footsteps() {
    let mut s = Sense::default();
    let mut fired = 0;
    for _ in 0..3 {
        assert!(!s.add(0.5));
        for _ in 0..10 {
            if s.decay(0.1) {
                fired += 1;
            }
        }
    }
    assert_eq!(fired, 1, "the third false one fires");
    assert_eq!(s.false_count, 0, "the counter restarted");
    // Two steps close together (1.0) fire at once.
    let mut s = Sense::default();
    assert!(!s.add(0.5));
    assert!(s.add(0.5));
}

/// 53. The grenade dodge's five directions: away, 45° and 90° to one side,
/// 90° and 45° to the other (never toward the grenade).
#[test]
fn case_53_dodge_directions() {
    assert_eq!(math::DODGE_STEPS, [0, 1, 2, -2, -1]);
    for k in math::DODGE_STEPS {
        let away = Quat::from_rotation_y(k as f32 * std::f32::consts::FRAC_PI_4) * Vec3::X;
        assert!(away.x > -1e-4, "no westward component: {k}");
    }
}

/// 55. A bot's protection is 3 s of no damage, whatever the mode's, and it
/// respawns 7 s after its death.
#[test]
fn case_55_bot_protection_and_respawn() {
    let mut sim = game("TDM", 4);
    settings(&mut sim).respawn_wait_client = 2.0;
    let human = join(&mut sim, JSF);
    let bot = join(&mut sim, CREED);
    sim.app.world_mut().entity_mut(bot).insert(Bot);
    until_live(&mut sim);
    let t0 = now(&sim);
    let wait_to = |sim: &mut chimera_world::harness::Sim, t: f64| {
        while now(sim) < t {
            sim.ticks(1);
        }
    };
    wait_to(&mut sim, t0 + 2.85);
    hit(&mut sim, bot, Some(human), 0.2, DamageKind::Blast, None);
    sim.ticks(1);
    assert_eq!(health(&sim, bot), 1.0, "protected at 2.9 s");
    wait_to(&mut sim, t0 + 3.1);
    hit(&mut sim, bot, Some(human), 0.2, DamageKind::Blast, None);
    sim.ticks(1);
    assert!(health(&sim, bot) < 1.0, "hurt at 3.1 s");
    hit(&mut sim, bot, Some(human), 2.0, DamageKind::Blast, None);
    sim.ticks(1);
    let died = now(&sim);
    assert!(dead(&sim, bot));
    wait_to(&mut sim, died + 6.8);
    assert!(dead(&sim, bot), "not before 7 s");
    wait_to(&mut sim, died + 7.3);
    assert!(!dead(&sim, bot), "back at 7 s");
    assert_eq!(spawns::BOT_RESPAWN, 7.0);
}

/// 56. Bot chat: the cooldown restarts before the roll; a millisecond
/// clock ending in 42 (≤ 60) shows a kill line, 75 shows none and the bot
/// stays silent for the cooldown.
#[test]
fn case_56_chat() {
    let mut c = CaChat {
        set: 0,
        until: 0.0,
        dead: false,
    };
    let line = c.event(Event::Kill, 30.0, 30_042, 20.0, 3);
    assert_eq!(line.as_deref(), Some("$Message0_Kill_3"));
    assert_eq!(c.until, 50.0);
    let mut c = CaChat {
        set: 0,
        until: 0.0,
        dead: false,
    };
    assert_eq!(c.event(Event::Kill, 30.0, 30_075, 20.0, 3), None);
    assert_eq!(c.event(Event::Kill, 40.0, 40_010, 20.0, 3), None, "silent until 50 s");
    assert!(c.event(Event::Kill, 50.0, 50_010, 20.0, 3).is_some());
    assert_eq!(Event::Respawn.percent(), 25);
}
