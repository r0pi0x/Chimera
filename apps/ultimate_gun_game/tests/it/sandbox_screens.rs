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

/// A key pressed for one frame of the sandbox's `ButtonInput` (no input
/// plugin here: pressed, a frame, released and cleared).
fn key(sim: &mut Sim, k: KeyCode) {
    sim.app.world_mut().resource_mut::<ButtonInput<KeyCode>>().press(k);
    sim.app.update();
    let mut input = sim.app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
    input.release(k);
    input.clear();
    sim.seconds(0.3);
}

/// The pinned bar and K end to end on the real mechanics: two guns
/// pinned, 1 and 2 switch to them (`PutOnGun`), a kill with the second
/// is the last-kill card, and K after a clear puts the targets back
/// (`RepeatLayout`).
#[test]
fn pinned_guns_switch_on_1_and_2_and_k_repeats_the_set_up() {
    use chimera_core::console::Console;
    use chimera_sim::minigame::ladder::{Ladder, Rung};
    let (mut sim, me) = play();
    let ladder = sim.app.world().resource::<Ladder>().clone();
    // Another of the game's guns on the ladder (its own rung).
    let second: &'static str = ladder.rungs.iter().copied().find(|r| *r != GUN && r.starts_with("combat_arms:") && !r.contains("grenade")).expect("a second gun");
    // Two targets.
    for d in [10.0, 15.0] {
        ask(&mut sim, BenchRequest::Spawn(TargetSpec { character: "combat_arms:default".into(), distance_m: d, ..default() }));
    }
    assert_eq!(sim.app.world().resource::<BenchView>().targets.len(), 2);
    // Pin two guns (the console's `sandbox_pin`, as Ctrl+1-6 or a drop).
    for line in [format!("sandbox_pin 1 {GUN}"), format!("sandbox_pin 2 {second}")] {
        sim.app.world_mut().resource_mut::<Console>().submit(line);
    }
    sim.seconds(0.3);
    let pins = sim.app.world().resource::<SandboxUi>().pins.clone();
    assert_eq!((pins.slot_of(GUN), pins.slot_of(second)), (Some(0), Some(1)), "{pins:?}");
    // 2, then 1, then 2: the player on each gun's rung, the chip with it.
    for (k, gun) in [(KeyCode::Digit2, second), (KeyCode::Digit1, GUN), (KeyCode::Digit2, second)] {
        key(&mut sim, k);
        let rung = sim.app.world().get::<Rung>(me).map(|r| r.0).expect("a rung");
        assert_eq!(ladder.rungs.get(rung).copied(), Some(gun), "{k:?} puts {gun} in hand");
        assert_eq!(sim.app.world().resource::<SandboxUi>().current.as_deref(), Some(gun), "the chip follows {k:?}");
    }
    // A kill with it: the last-kill card names it.
    let e = Entity::from_bits(sim.app.world().resource::<BenchView>().targets[0].id);
    for _ in 0..10 {
        let point = sim.position(e);
        sim.app.world_mut().write_message(Damage {
            target: e,
            attacker: Some(me),
            amount: 0.4,
            point,
            dir: Vec3::NEG_Z,
            hitgroup: Hitgroup::Head,
            kind: DamageKind::Bullet,
            weapon: Some(second),
            force: Vec3::ZERO,
        });
        sim.seconds(0.1);
    }
    sim.seconds(0.5);
    let log = sim.app.world().resource::<KillLog>().clone();
    let card = log.0.last().unwrap_or_else(|| panic!("a card: {log:?}"));
    assert_eq!(card.gun, second, "the card's gun");
    assert!(!card.hits.is_empty() && card.shots >= 1, "{card:?}");
    // Cleared, then K: both back.
    ask(&mut sim, BenchRequest::Clear);
    assert!(sim.app.world().resource::<BenchView>().targets.is_empty());
    key(&mut sim, KeyCode::KeyK);
    sim.seconds(1.5);
    let back = sim.app.world().resource::<BenchView>().targets.clone();
    assert_eq!(back.len(), 2, "K put both back: {back:?}");
    let mut d: Vec<f32> = back.iter().map(|t| t.spec.distance_m).collect();
    d.sort_by(f32::total_cmp);
    assert!((d[0] - 10.0).abs() < 1.0 && (d[1] - 15.0).abs() < 1.0, "at their distances: {d:?}");
}

/// Change gun lists every gun the sandbox can switch to: each rung but
/// the sentinel, once, the games' melee weapons among them (the ladder
/// carries them: `sandbox::melee_weapons`), grouped as Melee.
#[test]
fn change_gun_lists_every_rung_with_melee_as_its_own_class() {
    use chimera_client::client::sandbox::model::{Files, GunView, section_of};
    use chimera_sim::minigame::ladder::Ladder;
    let (sim, _) = play();
    let ladder = sim.app.world().resource::<Ladder>().clone();
    let usable = &ladder.rungs[..ladder.rungs.len() - 1];
    let mut unique: Vec<&str> = Vec::new();
    for r in usable {
        if !unique.contains(r) {
            unique.push(r);
        }
    }
    let cat = sim.app.world().resource::<SandboxCatalogue>().clone();
    let listed: Vec<&str> = cat.guns.iter().map(|g| g.id.as_str()).collect();
    assert_eq!(listed, unique, "every rung but the sentinel, once, in the ladder's order");
    let shown: usize = GunView::default().shown(&cat.guns, &|_| Files::Here).iter().map(|s| s.1.len()).sum();
    assert_eq!(shown, cat.guns.len(), "the grid shows all of them");
    // The melee weapons of the ladder's games are rungs, listed as Melee.
    let games: Vec<&str> = unique.iter().filter_map(|r| r.split_once(':').map(|(g, _)| g)).collect();
    let melee: Vec<&str> = ultimate_gun_game::sandbox::melee_weapons(sim.app.world())
        .into_iter()
        .filter(|m| games.iter().any(|g| m.starts_with(&format!("{g}:"))))
        .collect();
    assert!(!melee.is_empty(), "some melee weapon in the ladder's games");
    for m in &melee {
        let g = cat.get(m).unwrap_or_else(|| panic!("{m} listed"));
        assert_eq!(section_of(g), "Melee", "{m}: {:?}", g.tags);
    }
}
