//! specs/combat_arms/effects_sounds.md 11 "Kill call-outs" and Follow-up
//! (2026-11-14) 7 and 8: who speaks `EnemyDown` and `ManDown`, who hears
//! them, which file (`game::effects::kill_callout`), cases 106 and 111.
//! The records are a fixture of ours (`FIXTURE`), never the install's.

use bevy::prelude::*;
use chimera_core::core::{Damage, DamageKind, Died, Health, Hitgroup, LocalPlayer, Team};
use chimera_world::harness::Sim;
use combat_arms::game::{
    effects::{
        kill_callout::{self, CaTeamCallout, CaTeamCallouts, ENEMY_DOWN, FEMALE, MALE, MAN_DOWN, Q_MALE, VIP_M},
        sounds::CaSound,
    },
    inventory::UNIT,
    movement::{self, CaInfected},
    rules::escort::CaVip,
};

/// Our records: `EnemyDown` as the stock one (a file per voice, the
/// infected templates silent) and a `ManDown` with a male file (the
/// stock one has none) so its speaker can be heard.
const FIXTURE: &str = r#"
[AutoCastTeams3DSound0]
Name = "EnemyDown"
3DsoundRadius = 1000.0
Male_Snd0 = "test\enemydown_male.wav"
Female_Snd0 = "test\enemydown_female.wav"
VIP_M_Snd0 = "test\enemydown_female.wav"

[AutoCastTeams3DSound1]
Name = "ManDown"
3DsoundRadius = 1000.0
Male_Snd0 = "test\mandown_male.wav"
"#;

/// The stock `ManDown`: no files at all.
const STOCK_MAN_DOWN: &str = r#"
[AutoCastTeams3DSound0]
Name = "EnemyDown"
3DsoundRadius = 1000.0
Male_Snd0 = "test\enemydown_male.wav"

[AutoCastTeams3DSound1]
Name = "ManDown"
3DsoundRadius = 1000.0
"#;

fn sim(records: &str) -> Sim {
    let mut sim = crate::common::sim();
    sim.app.insert_resource(CaTeamCallouts::parse(records).unwrap());
    sim.app.init_resource::<Heard>().add_systems(Last, log_sounds);
    sim
}

#[derive(Resource, Default)]
struct Heard(Vec<String>);

fn log_sounds(mut sounds: MessageReader<CaSound>, mut log: ResMut<Heard>) {
    log.0.extend(
        sounds
            .read()
            .filter(|s| s.file.starts_with("test\\"))
            .map(|s| s.file.clone()),
    );
}

/// A living player of `team` at `x` u along X.
fn player(sim: &mut Sim, team: u8, x: f32) -> Entity {
    let p = sim.spawn_character(Vec3::new(x * UNIT, 1.0, 0.0), movement::ID);
    sim.app
        .world_mut()
        .entity_mut(p)
        .insert((Team(team), Health { current: 1.0, max: 1.0 }));
    p
}

fn died(victim: Entity, attacker: Option<Entity>, at: Vec3) -> Died {
    Died {
        entity: victim,
        attacker,
        damage: Damage {
            target: victim,
            attacker,
            amount: 1.0,
            point: at,
            dir: Vec3::ZERO,
            hitgroup: Hitgroup::Generic,
            kind: if attacker.is_some() {
                DamageKind::Bullet
            } else {
                DamageKind::Fall
            },
            weapon: None,
            force: Vec3::ZERO,
        },
    }
}

fn calls(sim: &mut Sim, d: &Died) -> Vec<(u8, CaTeamCallout)> {
    let world = sim.app.world_mut();
    world.get_mut::<Health>(d.entity).unwrap().current = 0.0;
    kill_callout::of_death(world, d)
}

/// 106. Alpha A kills Bravo V; Bravo C 999 u from V speaks `ManDown`, D
/// at 1000 u does not; with both at 1000 u nobody; a fall: `EnemyDown`
/// to team 0 (nobody); a team kill: no `EnemyDown`.
#[test]
fn c106_who_speaks() {
    let mut sim = sim(FIXTURE);
    let a = player(&mut sim, 1, -500.0);
    let v = player(&mut sim, 2, 0.0);
    let c = player(&mut sim, 2, 999.0);
    let d = player(&mut sim, 2, -1000.0);
    sim.ticks(1);
    let out = calls(&mut sim, &died(v, Some(a), Vec3::ZERO));
    assert_eq!(out.len(), 2, "{out:?}");
    assert_eq!(
        (out[0].0, out[0].1.record, out[0].1.speaker, out[0].1.template),
        (1, ENEMY_DOWN, Some(a), MALE)
    );
    assert_eq!(
        (out[1].0, out[1].1.record, out[1].1.speaker),
        (2, MAN_DOWN, Some(c)),
        "C, not D"
    );
    // C at 1000 u too: nobody strictly closer.
    sim.app.world_mut().get_mut::<Transform>(c).unwrap().translation.x = 1000.0 * UNIT;
    let out = calls(&mut sim, &died(v, Some(a), Vec3::ZERO));
    assert_eq!(out.iter().filter(|(_, m)| m.record == MAN_DOWN).count(), 0, "{out:?}");
    let _ = d;
    // A fall: team 0, the male template, from where the damage was.
    let out = calls(&mut sim, &died(v, None, Vec3::new(1.0, 2.0, 3.0)));
    assert_eq!(out[0].0, 0);
    assert_eq!(
        (out[0].1.speaker, out[0].1.template, out[0].1.at),
        (None, MALE, [1.0, 2.0, 3.0])
    );
    // A team kill, a suicide: no EnemyDown.
    let mate = player(&mut sim, 2, 50.0);
    sim.ticks(1);
    let out = calls(&mut sim, &died(v, Some(mate), Vec3::ZERO));
    assert!(out.iter().all(|(_, m)| m.record != ENEMY_DOWN), "team kill: {out:?}");
    let out = calls(&mut sim, &died(v, Some(v), Vec3::ZERO));
    assert!(out.iter().all(|(_, m)| m.record != ENEMY_DOWN), "suicide: {out:?}");
}

/// The files heard by the local player `me` when `victim` dies by
/// `killer`.
fn heard(sim: &mut Sim, me: Entity, victim: Entity, killer: Option<Entity>) -> Vec<String> {
    for e in sim
        .app
        .world_mut()
        .query_filtered::<Entity, With<LocalPlayer>>()
        .iter(sim.app.world())
        .collect::<Vec<_>>()
    {
        sim.app.world_mut().entity_mut(e).remove::<LocalPlayer>();
    }
    sim.app.world_mut().entity_mut(me).insert(LocalPlayer);
    sim.app.world_mut().get_mut::<Health>(victim).unwrap().current = 0.0;
    sim.app.world_mut().resource_mut::<Heard>().0.clear();
    sim.app.world_mut().write_message(died(victim, killer, Vec3::ZERO));
    sim.ticks(2);
    sim.app.update();
    std::mem::take(&mut sim.app.world_mut().resource_mut::<Heard>().0)
}

/// 106 heard: the killer's team hears `EnemyDown`, the victim's team the
/// teammate's `ManDown`; a fall is heard by nobody; in free-for-all
/// (every player a team of their own) only the killer.
#[test]
fn c106_who_hears() {
    let mut sim = sim(FIXTURE);
    let a = player(&mut sim, 1, -500.0);
    let a2 = player(&mut sim, 1, -600.0);
    let v = player(&mut sim, 2, 0.0);
    let c = player(&mut sim, 2, 500.0);
    sim.ticks(1);
    assert_eq!(
        heard(&mut sim, a2, v, Some(a)),
        vec!["test\\enemydown_male.wav"],
        "A's teammate"
    );
    sim.app.world_mut().get_mut::<Health>(v).unwrap().current = 1.0;
    assert_eq!(
        heard(&mut sim, c, v, Some(a)),
        vec!["test\\mandown_male.wav"],
        "V's teammate hears C"
    );
    sim.app.world_mut().get_mut::<Health>(v).unwrap().current = 1.0;
    assert_eq!(
        heard(&mut sim, a2, v, None),
        vec!["test\\mandown_male.wav"; 0],
        "a fall: no EnemyDown for Alpha"
    );
    sim.app.world_mut().get_mut::<Health>(v).unwrap().current = 1.0;
    let fall = heard(&mut sim, c, v, None);
    assert!(
        !fall.iter().any(|f| f.contains("enemydown")),
        "nobody hears a world kill's EnemyDown: {fall:?}"
    );
    // Free-for-all: teams 3, 4, 5.
    let mut sim = sim_ffa();
    let (a, v, b) = (
        player(&mut sim, 3, 0.0),
        player(&mut sim, 4, 100.0),
        player(&mut sim, 5, 200.0),
    );
    sim.ticks(1);
    assert_eq!(
        heard(&mut sim, a, v, Some(a)),
        vec!["test\\enemydown_male.wav"],
        "the killer"
    );
    sim.app.world_mut().get_mut::<Health>(v).unwrap().current = 1.0;
    assert!(heard(&mut sim, b, v, Some(a)).is_empty(), "another player");
}

/// Others' voice lines in 3D (`party_voice_lines_3d`, docs/DESIGN.md):
/// in a free-for-all a player who asked hears the killer's `EnemyDown`
/// within 2R (the record's radius 1000 u: 2000 u) of the killer, not
/// beyond; one who didn't ask, nothing (the game's own audience); a world
/// kill's (no speaker) nobody; the victim's team's `ManDown` stays theirs.
#[test]
fn party_voice_lines_carry_enemy_down_to_those_who_asked() {
    let mut sim = sim_ffa();
    let a = player(&mut sim, 3, 0.0);
    let v = player(&mut sim, 4, 100.0);
    let near = player(&mut sim, 5, 1900.0);
    let far = player(&mut sim, 6, 2100.0);
    let plain = player(&mut sim, 7, 150.0);
    // The reader is this machine's player: its setting is the console's
    // (its userinfo).
    let ask = |sim: &mut Sim, on: bool| {
        let line = format!("party_voice_lines_3d {}", u8::from(on));
        sim.app
            .world_mut()
            .resource_mut::<chimera_core::console::Console>()
            .submit(&line);
    };
    sim.ticks(1);
    ask(&mut sim, true);
    assert_eq!(
        heard(&mut sim, near, v, Some(a)),
        vec!["test\\enemydown_male.wav"],
        "asked, within 2R"
    );
    sim.app.world_mut().get_mut::<Health>(v).unwrap().current = 1.0;
    assert!(heard(&mut sim, far, v, Some(a)).is_empty(), "asked, beyond 2R");
    sim.app.world_mut().get_mut::<Health>(v).unwrap().current = 1.0;
    assert_eq!(
        heard(&mut sim, a, v, Some(a)),
        vec!["test\\enemydown_male.wav"],
        "the killer once, as before"
    );
    ask(&mut sim, false);
    sim.app.world_mut().get_mut::<Health>(v).unwrap().current = 1.0;
    assert!(heard(&mut sim, plain, v, Some(a)).is_empty(), "didn't ask");
    ask(&mut sim, true);
    sim.app.world_mut().get_mut::<Health>(v).unwrap().current = 1.0;
    assert!(heard(&mut sim, near, v, None).is_empty(), "a world kill: no speaker");
    // ManDown: a teammate of the victim speaks it to the victim's team
    // only, whoever asked.
    let mate = player(&mut sim, 4, 300.0);
    sim.ticks(1);
    let calls: Vec<u8> = {
        let w = sim.app.world_mut();
        w.get_mut::<Health>(v).unwrap().current = 0.0;
        kill_callout::of_death(w, &died(v, Some(a), Vec3::ZERO))
            .iter()
            .map(|(_, c)| c.record)
            .collect()
    };
    assert!(calls.contains(&MAN_DOWN), "{calls:?}");
    let call = CaTeamCallout {
        speaker: Some(mate),
        record: MAN_DOWN,
        template: MALE,
        at: (Vec3::X * 300.0 * UNIT).to_array(),
    };
    assert!(
        !kill_callout::party_hears(sim.app.world(), near, &call, 1000.0),
        "ManDown stays the team's"
    );
    let enemy_down = CaTeamCallout {
        record: ENEMY_DOWN,
        speaker: Some(a),
        at: [0.0; 3],
        ..call
    };
    assert!(kill_callout::party_hears(sim.app.world(), near, &enemy_down, 1000.0));
    assert!(
        !kill_callout::party_hears(sim.app.world(), a, &enemy_down, 1000.0),
        "not the speaker"
    );
}

fn sim_ffa() -> Sim {
    sim(FIXTURE)
}

/// 111. The files by voice: male, female and the male VIP's (the female
/// file), the infected templates silent; the stock `ManDown` silent for
/// every template.
#[test]
fn c111_callout_files() {
    let r = CaTeamCallouts::parse(FIXTURE).unwrap();
    assert_eq!(
        r.voice(ENEMY_DOWN, MALE).unwrap().0,
        ["test\\enemydown_male.wav".to_string()]
    );
    assert_eq!(
        r.voice(ENEMY_DOWN, FEMALE).unwrap().0,
        ["test\\enemydown_female.wav".to_string()]
    );
    assert_eq!(
        r.voice(ENEMY_DOWN, VIP_M).unwrap().0,
        ["test\\enemydown_female.wav".to_string()]
    );
    assert_eq!(r.voice(ENEMY_DOWN, Q_MALE), None, "infected");
    assert_eq!(r.0[0].radius, 1000.0);
    let stock = CaTeamCallouts::parse(STOCK_MAN_DOWN).unwrap();
    for t in 0..7 {
        assert_eq!(stock.voice(MAN_DOWN, t), None, "template {t}");
    }
    // The templates by character.
    let mut sim = sim(FIXTURE);
    let plain = player(&mut sim, 1, 0.0);
    let vip = player(&mut sim, 1, 10.0);
    let infected = player(&mut sim, 1, 20.0);
    sim.app.world_mut().entity_mut(vip).insert(CaVip);
    sim.app.world_mut().entity_mut(infected).insert(CaInfected);
    let w = sim.app.world();
    assert_eq!(kill_callout::template_of(w, plain), MALE);
    assert_eq!(kill_callout::template_of(w, vip), VIP_M);
    assert_eq!(kill_callout::template_of(w, infected), Q_MALE);
    // An infected killer says nothing.
    let v = player(&mut sim, 2, 30.0);
    sim.ticks(1);
    assert!(heard(&mut sim, infected, v, Some(infected)).is_empty());
}

/// The kill voice is the character the killer was on the kill's tick
/// (`chimera_sim::kill_voice::at_kill`), not the one a rung up
/// made it since: a woman of ours who is TF2's by now still speaks her
/// `EnemyDown`; a TF2 character who is ours by now speaks none of ours.
#[test]
fn enemy_down_is_the_character_it_was_when_it_struck() {
    use chimera_core::slots::GameIdentity;
    use chimera_sim::kill_voice::{CharacterAtTick, KillerCharacter};
    let mut sim = sim(FIXTURE);
    let a = player(&mut sim, 1, -500.0);
    let v = player(&mut sim, 2, 0.0);
    sim.ticks(1);
    let struck = |sim: &mut Sim, game: &str, voice: u8, now: &str| {
        let w = sim.app.world_mut();
        let tick = w.resource::<chimera_core::core::SimTick>().0;
        w.entity_mut(a).insert((
            CharacterAtTick {
                tick,
                character: KillerCharacter {
                    game: Some(game.into()),
                    voice: Some(voice.to_string()),
                },
            },
            GameIdentity(now.into()),
        ));
    };
    struck(&mut sim, "combat_arms", FEMALE, "tf2");
    let out = calls(&mut sim, &died(v, Some(a), Vec3::ZERO));
    let enemy_down: Vec<_> = out.iter().filter(|(_, m)| m.record == ENEMY_DOWN).collect();
    assert_eq!(enemy_down.len(), 1, "{out:?}");
    assert_eq!(enemy_down[0].1.template, FEMALE, "her template then");
    struck(&mut sim, "tf2", MALE, "combat_arms");
    let out = calls(&mut sim, &died(v, Some(a), Vec3::ZERO));
    assert!(out.iter().all(|(_, m)| m.record != ENEMY_DOWN), "TF2's kill: {out:?}");
}

/// The whole tick: a kill whose killer a rung up swaps to TF2 on the same
/// tick (as the ladder does, after the deaths) still has our `EnemyDown`
/// heard, in the killer's template then, and `kill_voice` hears it was
/// voiced (no rung-up sound for it).
#[test]
fn kill_voice_survives_the_killers_rung_change() {
    use chimera_core::slots::GameIdentity;
    use chimera_sim::kill_voice::KillVoiceOutcome;
    #[derive(Resource, Default)]
    struct Outcomes(Vec<KillVoiceOutcome>);
    let mut sim = sim(FIXTURE);
    sim.app
        .init_resource::<Outcomes>()
        .add_systems(
            Last,
            |mut r: MessageReader<KillVoiceOutcome>, mut o: ResMut<Outcomes>| o.0.extend(r.read().copied()),
        )
        // The rung up: right after the tick's deaths, the killer is TF2's.
        .add_systems(
            FixedUpdate,
            (|mut died: MessageReader<Died>, mut commands: Commands| {
                for d in died.read() {
                    if let Some(a) = d.attacker {
                        commands.entity(a).insert(GameIdentity("tf2".into()));
                    }
                }
            })
            .after(chimera_core::core::apply_damage),
        );
    let a = player(&mut sim, 1, -500.0);
    let v = player(&mut sim, 2, 0.0);
    sim.app
        .world_mut()
        .entity_mut(a)
        .insert((LocalPlayer, GameIdentity("combat_arms".into())));
    sim.ticks(2);
    sim.app.world_mut().resource_mut::<Heard>().0.clear();
    sim.app.world_mut().write_message(Damage {
        target: v,
        attacker: Some(a),
        amount: 5.0,
        point: Vec3::ZERO,
        dir: Vec3::X,
        hitgroup: Hitgroup::Chest,
        kind: DamageKind::Bullet,
        weapon: None,
        force: Vec3::ZERO,
    });
    sim.ticks(3);
    sim.app.update();
    let w = sim.app.world();
    assert_eq!(
        w.get::<GameIdentity>(a).map(|i| i.0.as_str()),
        Some("tf2"),
        "swapped on the kill"
    );
    assert_eq!(
        w.resource::<Heard>().0,
        vec!["test\\enemydown_male.wav"],
        "our line, as he was"
    );
    let outcomes = &w.resource::<Outcomes>().0;
    assert_eq!(
        outcomes,
        &[KillVoiceOutcome {
            killer: a,
            victim: v,
            voiced: true
        }]
    );
}
