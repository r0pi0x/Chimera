//! TF2's player voices (specs/tf2/voices.md; `game::voices`) on the
//! harness: the data covers every class; pain from any game's gun with
//! its class's gap (spec 2.3-2.5, test cases on the gap); no pain on the
//! killing hit, the death cry by the killing damage (spec 3.1); a kill
//! line of the class the killer was on the kill's tick, after its predelay,
//! though a rung up made the killer another game's character on the
//! kill's own tick (`chimera_sim::kill_voice`).

use bevy::prelude::*;
use chimera_core::{
    core::{Damage, DamageKind, Died, Health, Hitgroup},
    slots::GameIdentity,
};
use chimera_sim::kill_voice::{CharacterVoice, KillVoiceOutcome};
use chimera_world::{greybox::GreyboxMapPlugin, harness::Sim, map::GameSound};
use tf2::{
    game::{
        classes::{Classes, Tf2Class},
        movement,
        voices::{Tf2Voices, pain_entry},
    },
    harness::Tf2Sim,
};

#[derive(Resource, Default)]
struct Heard(Vec<(String, Option<Entity>)>, Vec<KillVoiceOutcome>);

fn log(mut sounds: MessageReader<GameSound>, mut outcomes: MessageReader<KillVoiceOutcome>, mut heard: ResMut<Heard>) {
    heard.0.extend(sounds.read().map(|s| (s.0.entry.clone(), s.0.source)));
    heard.1.extend(outcomes.read().copied());
}

fn sim() -> Sim {
    let mut sim = Sim::new(Tf2Sim, GreyboxMapPlugin);
    sim.set_tick_interval(tf2::game::TICK_INTERVAL);
    sim.app.init_resource::<Heard>().add_systems(Last, log);
    sim
}

/// A TF2 player of class `class`.
fn player(sim: &mut Sim, x: f32, class: &str) -> Entity {
    let p = sim.spawn_character(Vec3::new(x, 1.0, 0.0), movement::ID);
    sim.ticks(2);
    let index = Classes::default().get(class).unwrap().index;
    sim.app.world_mut().entity_mut(p).insert((Tf2Class(index), Health { current: 125.0, max: 125.0 }));
    sim.ticks(2);
    p
}

fn hit(sim: &mut Sim, target: Entity, attacker: Option<Entity>, amount: f32, kind: DamageKind, weapon: Option<&'static str>, hitgroup: Hitgroup) {
    // Health to spare (the class's own may not be on yet).
    if let Some(mut h) = sim.app.world_mut().get_mut::<Health>(target) {
        h.current = h.current.max(100.0);
    }
    sim.app.world_mut().write_message(Damage {
        target,
        attacker,
        amount,
        point: Vec3::ZERO,
        dir: Vec3::X,
        hitgroup,
        kind,
        weapon,
        force: Vec3::ZERO,
    });
}

fn heard(sim: &mut Sim) -> Vec<(String, Option<Entity>)> {
    std::mem::take(&mut sim.app.world_mut().resource_mut::<Heard>().0)
}

#[test]
fn the_data_covers_every_class() {
    let voices = Tf2Voices::built_in();
    for c in &Classes::default().classes {
        let v = voices.classes.get(&c.name).unwrap_or_else(|| panic!("{}: no voice", c.name));
        assert!(!v.pain.is_empty() && !v.kill.is_empty(), "{}", c.name);
    }
    let entries = voices.entries();
    assert!(entries.contains(&"Scout.PainSharp08".to_string()) && entries.contains(&"Spy.ExplosionDeath".to_string()));
    assert!(entries.contains(&"Sniper.SpecialCompleted25".to_string()), "a kill line");
}

/// A Soldier hit by another game's gun cries `Soldier.PainSharpNN` from
/// himself; another hit inside the line's scene length is silent, one
/// after it cries again; the killing hit has no pain, only the death cry
/// for its damage (a bullet: `Soldier.Death`).
#[test]
fn pain_from_another_games_gun_and_the_death_cry() {
    let mut sim = sim();
    let v = player(&mut sim, 0.0, "soldier");
    heard(&mut sim);
    hit(&mut sim, v, None, 10.0, DamageKind::Bullet, Some("combat_arms:ak47"), Hitgroup::Chest);
    sim.ticks(2);
    let first = heard(&mut sim);
    let pains: Vec<_> = first.iter().filter(|(e, _)| e.starts_with("Soldier.PainSharp")).collect();
    assert_eq!(pains.len(), 1, "{first:?}");
    assert_eq!(pains[0].1, Some(v), "from the victim");
    let n: usize = pains[0].0["Soldier.PainSharp".len()..].parse().unwrap();
    let gap = Tf2Voices::built_in().classes["soldier"].pain[n - 1];
    hit(&mut sim, v, None, 10.0, DamageKind::Bullet, Some("cs_source:weapon_ak47"), Hitgroup::Chest);
    sim.ticks(1);
    assert!(heard(&mut sim).iter().all(|(e, _)| !e.contains("Pain")), "inside the {gap} s gap");
    sim.seconds(gap + 0.05);
    hit(&mut sim, v, None, 10.0, DamageKind::Bullet, Some("cs_source:weapon_ak47"), Hitgroup::Chest);
    sim.ticks(2);
    assert!(heard(&mut sim).iter().any(|(e, _)| e.starts_with("Soldier.PainSharp")), "after it");
    sim.seconds(5.0);
    heard(&mut sim);
    hit(&mut sim, v, None, 500.0, DamageKind::Bullet, Some("combat_arms:ak47"), Hitgroup::Chest);
    sim.ticks(2);
    let death = heard(&mut sim);
    assert!(death.iter().all(|(e, _)| !e.contains("PainSharp")), "no pain on the killing hit: {death:?}");
    assert!(death.contains(&("Soldier.Death".into(), Some(v))), "{death:?}");
    assert_eq!(pain_entry("Soldier", 3), "Soldier.PainSharp03");
}

/// A Sniper's rifle headshot always speaks (spec A.6, `SniperHeadShotKill`)
/// after its group's predelay, from the killer. The rung up on the kill's
/// tick makes the killer Combat Arms' Scout-holding character: the line
/// is still the Sniper's, plays in full, and the kill is voiced.
#[test]
fn kill_line_survives_the_killers_rung_change() {
    let mut sim = sim();
    sim.app.add_systems(
        FixedUpdate,
        (|mut died: MessageReader<Died>, mut commands: Commands| {
            for d in died.read() {
                if let Some(a) = d.attacker {
                    commands.entity(a).insert((GameIdentity("combat_arms".into()), Tf2Class(1)));
                }
            }
        })
        .after(chimera_core::core::apply_damage),
    );
    let k = player(&mut sim, 0.0, "sniper");
    let v = player(&mut sim, 5.0, "scout");
    assert_eq!(sim.app.world().get::<CharacterVoice>(k).map(|c| c.0.as_str()), Some("sniper"));
    heard(&mut sim);
    hit(&mut sim, v, Some(k), 500.0, DamageKind::Bullet, Some("tf2:tf_weapon_sniperrifle"), Hitgroup::Head);
    sim.ticks(2);
    let w = sim.app.world();
    assert_eq!(w.get::<GameIdentity>(k).map(|i| i.0.as_str()), Some("combat_arms"), "swapped on the kill");
    assert_eq!(w.resource::<Heard>().1, vec![KillVoiceOutcome { killer: k, victim: v, voiced: true }]);
    assert!(heard(&mut sim).iter().all(|(_, s)| *s != Some(k)), "not before the predelay");
    sim.seconds(2.0);
    let lines: Vec<_> = heard(&mut sim).into_iter().filter(|(_, s)| *s == Some(k)).collect();
    assert_eq!(lines.len(), 1, "{lines:?}");
    assert!(lines[0].0.starts_with("Sniper."), "the Sniper's line: {lines:?}");
}

/// A kill with no line (a Scout's first kill with his scattergun: no rule
/// passes) is unvoiced: its player hears the rung-up sound.
#[test]
fn a_kill_without_a_line_is_unvoiced() {
    let mut sim = sim();
    let k = player(&mut sim, 0.0, "scout");
    let v = player(&mut sim, 5.0, "soldier");
    hit(&mut sim, v, Some(k), 500.0, DamageKind::Bullet, Some("tf2:tf_weapon_scattergun"), Hitgroup::Chest);
    sim.ticks(2);
    assert_eq!(
        sim.app.world().resource::<Heard>().1,
        vec![KillVoiceOutcome { killer: k, victim: v, voiced: false }]
    );
}
