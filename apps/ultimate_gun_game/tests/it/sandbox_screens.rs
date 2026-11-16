//! The sandbox's screens on the real targets (`chimera_client::client::
//! sandbox::link`'s bridge to `chimera_sim::minigame::targets`): the bench
//! asks for a target and lists it, moves it, removes it; a refused spawn's
//! reason shows; a kill by the local player becomes the last-kill card;
//! the screens know a sandbox plays and its guns. On the party's
//! simulation, headless.

use bevy::prelude::*;
use chimera_client::client::sandbox::{
    SandboxCatalogue, SandboxUi, SandboxUiPlugin,
    link::{BenchRequest, BenchView, KillLog, PropSpec, TargetSpec},
};
use chimera_core::core::{Damage, DamageKind, Hitgroup, LocalPlayer};
use chimera_sim::minigame::{self, targets::Target};
use chimera_ui::{Theme, UiClock};
use chimera_world::harness::Sim;
use combat_arms::game::movement;
use ultimate_gun_game::sandbox::SANDBOX;


const GUN: &str = "combat_arms:m4a1";

fn play() -> (Sim, Entity) {
    let mut sim = Sim::new(
        combat_arms::harness::CaSim,
        (chimera_world::greybox::GreyboxMapPlugin, party_kit::PartyPartsPlugin, ultimate_gun_game::UltimateGunGamePlugin::default(), SandboxUiPlugin),
    );
    sim.app.insert_resource(Theme::from_toml("name = \"test\"\n[flags.hud]\nsignal = true\n").expect("theme"));
    sim.app.insert_resource(UiClock::manual());
    ultimate_gun_game::sandbox::generate(sim.app.world_mut(), "combat_arms").expect("the definition");
    minigame::start(sim.app.world_mut(), SANDBOX).expect("the sandbox starts");
    let me = sim.spawn_character(Vec3::new(0.0, 1.0, 0.0), movement::ID);
    sim.app.world_mut().entity_mut(me).insert(LocalPlayer);
    sim.seconds(1.5);
    (sim, me)
}

fn ask(sim: &mut Sim, r: BenchRequest) {
    sim.app.world_mut().write_message(r);
    sim.seconds(1.0);
}

#[test]
fn the_bench_spawns_lists_moves_and_removes_a_real_target() {
    let (mut sim, me) = play();
    let ui = sim.app.world().resource::<SandboxUi>().clone();
    assert!(ui.on, "a sandbox plays");
    assert!(sim.app.world().resource::<SandboxCatalogue>().guns.len() > 10, "its guns listed");
    let spec = TargetSpec { character: "combat_arms:default".into(), gun: Some(GUN.into()), ..default() };
    ask(&mut sim, BenchRequest::Spawn(spec.clone()));
    let view = sim.app.world().resource::<BenchView>().clone();
    assert_eq!(view.targets.len(), 1, "listed: {view:?}");
    let row = view.targets[0].clone();
    assert!(!row.name.is_empty() && !row.game.is_empty() && row.home.is_some(), "{row:?}");
    assert_eq!(row.spec.distance_m, 10.0);
    // Moved to 20 m.
    ask(&mut sim, BenchRequest::Set(row.id, TargetSpec { distance_m: 20.0, ..row.spec.clone() }));
    let e = Entity::from_bits(row.id);
    assert_eq!(sim.app.world().get::<Target>(e).map(|t| t.distance_m), Some(20.0));
    // A spawn past the floor: refused, its reason shown.
    ask(&mut sim, BenchRequest::Spawn(TargetSpec { distance_m: 5000.0, ..spec.clone() }));
    let view = sim.app.world().resource::<BenchView>().clone();
    assert!(view.refused.as_deref().is_some_and(|r| r.contains("no floor")), "{:?}", view.refused);
    assert_eq!(view.targets.len(), 1);
    // A kill: the last-kill card.
    for _ in 0..10 {
        let point = sim.position(e);
        sim.app.world_mut().write_message(Damage {
            target: e,
            attacker: Some(me),
            amount: 0.4,
            point,
            dir: Vec3::NEG_Z,
            hitgroup: Hitgroup::Chest,
            kind: DamageKind::Bullet,
            weapon: Some(GUN),
            force: Vec3::ZERO,
        });
        sim.seconds(0.1);
    }
    sim.seconds(0.5);
    let log = sim.app.world().resource::<KillLog>().clone();
    let k = log.0.last().unwrap_or_else(|| panic!("a card: {log:?}"));
    assert_eq!(k.gun, GUN);
    assert!(!k.hits.is_empty(), "its hits: {k:?}");
    assert!((k.distance_m - 20.0).abs() < 1.0, "{k:?}");
    // Removed.
    ask(&mut sim, BenchRequest::Remove(row.id));
    assert!(sim.app.world().resource::<BenchView>().targets.is_empty());
    // A prop: listed with its pieces standing, then cleared away.
    ask(&mut sim, BenchRequest::AddProp(PropSpec { kind: "bottles".into(), distance_m: 10.0 }));
    let props = sim.app.world().resource::<BenchView>().props.clone();
    assert_eq!(props.len(), 1, "{props:?}");
    assert_eq!(props[0].standing, (6, 6));
    ask(&mut sim, BenchRequest::Clear);
    assert!(sim.app.world().resource::<BenchView>().props.is_empty());
}

#[test]
fn a_switch_a_condition_and_k_go_to_the_mechanics() {
    use chimera_client::client::sandbox::{SandboxAction, bench::BenchAction, link::SandboxOption};
    use chimera_sim::minigame::{conditions::SandboxConditions, ladder::{Ladder, Rung}};
    let (mut sim, me) = play();
    // A switch: `PutOnGun`, the player on the gun's rung.
    sim.app.world_mut().write_message(SandboxAction::Switch(GUN.into()));
    sim.seconds(0.5);
    let rung = sim.app.world().get::<Rung>(me).map(|r| r.0).expect("a rung");
    let ladder = sim.app.world().resource::<Ladder>().clone();
    assert_eq!(ladder.rungs.get(rung).copied(), Some(GUN), "on the gun's rung");
    assert_eq!(sim.app.world().resource::<SandboxUi>().current.as_deref(), Some(GUN), "the chip follows");
    // A condition: endless ammo off is the mechanics' (`SetCondition`),
    // and the bench shows it as they hold it. (Half speed is the shell's
    // `host_timescale`, not in this headless app.)
    sim.app.world_mut().write_message(BenchAction::Option(SandboxOption::EndlessAmmo, false));
    sim.seconds(0.5);
    assert!(!sim.app.world().resource::<SandboxConditions>().endless_ammo);
    assert!(!sim.app.world().resource::<chimera_client::client::sandbox::link::SandboxOptions>().endless_ammo, "shown as held");
    // K: the last set-up again.
    ask(&mut sim, BenchRequest::Spawn(TargetSpec { character: "combat_arms:default".into(), gun: Some(GUN.into()), ..default() }));
    ask(&mut sim, BenchRequest::Clear);
    assert!(sim.app.world().resource::<BenchView>().targets.is_empty());
    sim.app.world_mut().write_message(SandboxAction::Repeat);
    sim.seconds(1.5);
    assert_eq!(sim.app.world().resource::<BenchView>().targets.len(), 1, "K put it back");
}
