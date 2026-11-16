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
    play_on(None)
}

/// `play` on a code-built map (`range`), the player just behind its
/// firing line.
fn play_on(map: Option<&str>) -> (Sim, Entity) {
    let mut sim = Sim::new(
        combat_arms::harness::CaSim,
        (chimera_world::greybox::GreyboxMapPlugin, party_kit::PartyPartsPlugin, ultimate_gun_game::UltimateGunGamePlugin::default(), SandboxUiPlugin),
    );
    let mut at = Vec3::new(0.0, 1.0, 0.0);
    if let Some(map) = map {
        let w = sim.app.world_mut();
        chimera_world::greybox::unload(w);
        chimera_world::greybox::select(w, map);
        chimera_world::greybox::respawn(w);
        sim.ticks(2);
        if let Some(l) = sim.app.world().get_resource::<chimera_world::range::RangeLayout>() {
            at = l.point(-0.5, 0.0) + Vec3::Y;
        }
    }
    sim.app.insert_resource(Theme::from_toml("name = \"test\"\n[flags.hud]\nsignal = true\n").expect("theme"));
    sim.app.insert_resource(UiClock::manual());
    ultimate_gun_game::sandbox::generate(sim.app.world_mut(), "combat_arms").expect("the definition");
    minigame::start(sim.app.world_mut(), SANDBOX).expect("the sandbox starts");
    let me = sim.spawn_character(at, movement::ID);
    sim.app.world_mut().entity_mut(me).insert(LocalPlayer);
    sim.seconds(1.5);
    (sim, me)
}

#[test]
fn the_bench_puts_a_target_on_the_ranges_platform_and_says_why_not_off_it() {
    use chimera_client::client::sandbox::link::{TargetMotion, TargetPlace};
    // On the range: spawned on the platform, back on a line, onto it again.
    let (mut sim, _) = play_on(Some("range"));
    let top = sim.app.world().resource::<chimera_world::range::RangeLayout>().platform_top;
    let spec = TargetSpec { character: "combat_arms:default".into(), gun: Some(GUN.into()), ..default() };
    ask(&mut sim, BenchRequest::Spawn(TargetSpec { place: TargetPlace::Platform, ..spec.clone() }));
    let view = sim.app.world().resource::<BenchView>().clone();
    assert_eq!(view.refused, None);
    let row = view.targets[0].clone();
    assert_eq!(row.spec.place, TargetPlace::Platform);
    assert!((row.home.unwrap().xz() - top.xz()).length() < 1.3, "on the platform: {row:?}");
    ask(&mut sim, BenchRequest::Set(row.id, TargetSpec { place: TargetPlace::Line, distance_m: 20.0, ..row.spec.clone() }));
    let row = sim.app.world().resource::<BenchView>().targets[0].clone();
    assert_eq!((row.spec.place, row.spec.distance_m), (TargetPlace::Line, 20.0));
    ask(&mut sim, BenchRequest::Set(row.id, TargetSpec { place: TargetPlace::Platform, ..row.spec.clone() }));
    let row = sim.app.world().resource::<BenchView>().targets[0].clone();
    assert_eq!(row.spec.place, TargetPlace::Platform);
    assert_eq!(sim.app.world().get::<Target>(Entity::from_bits(row.id)).map(|t| t.place), Some(TargetPlace::Platform));
    // A strafe there is the mechanics' refusal, shown.
    ask(&mut sim, BenchRequest::Set(row.id, TargetSpec { motion: TargetMotion::Strafes, ..row.spec.clone() }));
    let view = sim.app.world().resource::<BenchView>().clone();
    assert!(view.refused.is_some(), "refused: {view:?}");
    assert_eq!(view.targets[0].spec.motion, TargetMotion::Still);
    // Off the range: refused, in the bench's words.
    let (mut sim, _) = play();
    ask(&mut sim, BenchRequest::Spawn(spec.clone()));
    let row = sim.app.world().resource::<BenchView>().targets[0].clone();
    ask(&mut sim, BenchRequest::Set(row.id, TargetSpec { place: TargetPlace::Platform, ..row.spec.clone() }));
    let view = sim.app.world().resource::<BenchView>().clone();
    assert_eq!(view.refused.as_deref(), Some("The platform is on the range only."));
    assert_eq!(view.targets[0].spec.place, TargetPlace::Line);
    ask(&mut sim, BenchRequest::Spawn(TargetSpec { place: TargetPlace::Platform, ..spec }));
    let view = sim.app.world().resource::<BenchView>().clone();
    assert_eq!(view.refused.as_deref(), Some("The platform is on the range only."));
    assert_eq!(view.targets.len(), 1);
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
    let (sim, me) = play();
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
    // The grid shows all of them, a gun's variants as one tile.
    let tiles = GunView::default().tiles(&cat.guns, &|_| Files::Here);
    let mut in_tiles: Vec<usize> = tiles.iter().flat_map(|s| s.1.iter()).flat_map(|t| t.variants.clone()).collect();
    in_tiles.sort();
    assert_eq!(in_tiles, (0..cat.guns.len()).collect::<Vec<_>>(), "every gun on one tile");
    for t in tiles.iter().flat_map(|s| s.1.iter()) {
        let family = &cat.guns[t.variants[0]].family;
        assert!(t.variants.iter().all(|v| &cat.guns[*v].family == family), "a tile is one family");
        assert_eq!(&cat.guns[t.face].id, family, "its face the base gun");
    }
    // Every variant stays a rung: the console's `rung <id>` takes it.
    let mut sim = sim;
    for g in cat.guns.iter().filter(|g| g.id != g.family).take(3).chain(cat.guns.iter().take(1)) {
        sim.app.world_mut().resource_mut::<chimera_core::console::Console>().submit(format!("rung {}", g.id));
        sim.seconds(0.3);
        assert_eq!(sim.app.world().get::<chimera_sim::minigame::ladder::Rung>(me).map(|r| r.0), Some(g.rung), "rung {}", g.id);
    }
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

/// The mechanics' own waiting pick (`rung_command::GunPending`: a console
/// `rung` whose gun's files aren't in) shows in the chip's Next strip, and
/// its end (`GunPickEnded`) clears it: replaced by a newer pick, applied
/// when the files come, dropped when the ladder changes under it.
#[test]
fn a_pick_the_mechanics_hold_shows_as_next_until_it_ends() {
    use std::collections::BTreeSet;

    use chimera_client::client::sandbox::{hud, link::SandboxOptions, units::UnitProgress};
    use chimera_core::console::Console;
    use chimera_sim::minigame::{gun_files::GunFiles, ladder::Ladder, rung_command::GunPending};
    use chimera_ui::UiParityId;

    const SNIPER: &str = "tf2:tf_weapon_sniperrifle";
    const ROCKETS: &str = "tf2:tf_weapon_rocketlauncher";
    #[derive(Resource, Default)]
    struct Missing(BTreeSet<String>);
    fn fake(w: &mut World, id: &str) -> bool {
        !w.resource::<Missing>().0.contains(id)
    }
    let (mut sim, me) = play();
    let run = |sim: &mut Sim, line: &str| {
        sim.app.world_mut().resource_mut::<Console>().submit(line.to_string());
        sim.app.update();
        sim.app.update();
    };
    let missing = |sim: &mut Sim, guns: &[&str]| {
        sim.app.world_mut().insert_resource(Missing(guns.iter().map(|g| g.to_string()).collect()));
    };
    let ui = |sim: &Sim| sim.app.world().resource::<SandboxUi>().clone();
    let shown = |sim: &Sim| {
        let w = sim.app.world();
        hud::shown(&ui(sim), w.resource::<SandboxCatalogue>(), w.resource::<UnitProgress>(), &KillLog::default(), &SandboxOptions::default(), 0.0)
    };
    let drawn = |sim: &mut Sim, id: &str| {
        let w = sim.app.world_mut();
        w.query::<&UiParityId>().iter(w).any(|p| p.0 == id)
    };
    run(&mut sim, &format!("rung {GUN}"));
    sim.seconds(0.5);
    assert_eq!(ui(&sim).current.as_deref(), Some(GUN));
    missing(&mut sim, &[SNIPER, ROCKETS]);
    sim.app.world_mut().insert_resource(GunFiles(vec![fake]));
    // Picked at the console: it waits, shown as Next with its name.
    run(&mut sim, &format!("rung {SNIPER}"));
    assert_eq!(sim.app.world().get::<GunPending>(me).map(|p| p.gun), Some(SNIPER));
    let u = ui(&sim);
    assert_eq!((u.pending.as_deref(), u.pick_held, u.current.as_deref()), (Some(SNIPER), true, Some(GUN)));
    let s = shown(&sim);
    let (face, _) = s.next.clone().expect("the Next strip");
    assert_eq!(face.id, SNIPER);
    assert!(!face.name.is_empty() && face.name != SNIPER, "its name: {face:?}");
    assert!(s.next_held);
    assert!(drawn(&mut sim, "sb.next"), "the strip drawn");
    // A newer pick replaces it.
    run(&mut sim, &format!("rung {ROCKETS}"));
    assert_eq!(ui(&sim).pending.as_deref(), Some(ROCKETS));
    // Its files come: applied, in hand, the strip gone.
    missing(&mut sim, &[]);
    sim.app.update();
    sim.app.update();
    let u = ui(&sim);
    assert_eq!((u.pending.as_deref(), u.pick_held, u.current.as_deref()), (None, false, Some(ROCKETS)));
    assert!(shown(&sim).next.is_none());
    assert!(!drawn(&mut sim, "sb.next"));
    // A pick waiting when the ladder changes under it: dropped, said so.
    missing(&mut sim, &[SNIPER]);
    run(&mut sim, &format!("rung {SNIPER}"));
    assert_eq!(ui(&sim).pending.as_deref(), Some(SNIPER));
    let at = sim.app.world().get::<GunPending>(me).map(|p| p.rung).unwrap();
    sim.app.world_mut().resource_mut::<Ladder>().rungs[at] = GUN;
    sim.app.update();
    sim.app.update();
    let u = ui(&sim);
    assert_eq!((u.pending.as_deref(), u.pick_held), (None, false));
    assert!(u.said.as_deref().is_some_and(|s| s.contains("dropped")), "{:?}", u.said);
    assert!(drawn(&mut sim, "sb.said"), "the word on the chip's foot");
}

/// Nobody races in a sandbox: switching rung there raises no lead event
/// (so no "You took the lead" banner, cue or announcer call), up or down
/// the ladder.
#[test]
fn switching_rung_in_the_sandbox_takes_no_lead() {
    use chimera_core::console::Console;
    use chimera_sim::minigame::{MinigameEvent, ladder::{Leaders, Rung}};

    #[derive(Resource, Default)]
    struct Events(Vec<&'static str>);
    fn keep(mut seen: ResMut<Events>, mut r: MessageReader<MinigameEvent>) {
        seen.0.extend(r.read().map(|e| e.name));
    }
    let (mut sim, me) = play();
    sim.app.init_resource::<Events>().add_systems(Last, keep).add_systems(FixedPostUpdate, keep);
    for rung in ["5", "2", "9", "3"] {
        sim.app.world_mut().resource_mut::<Console>().submit(format!("rung {rung}"));
        sim.seconds(1.0);
    }
    assert_eq!(sim.app.world().get::<Rung>(me).map(|r| r.0), Some(2), "on the rung asked");
    let seen = sim.app.world().resource::<Events>().0.clone();
    assert!(!seen.iter().any(|n| n.starts_with("lead_")), "no lead in a sandbox: {seen:?}");
    assert!(sim.app.world().get_resource::<Leaders>().is_none_or(|l| l.0.is_empty()));
}

#[test]
fn the_catalogue_names_guns_by_their_games_strings_never_by_a_token() {
    use chimera_client::client::minigame_scoreboard::GunNames;
    // The sandbox comes up before any game's strings are read (`+sandbox`
    // on the command line): CS:S's guns, whose data name a string token
    // (`Cstrike_WPNHUD_*`), show their ids' names, not the token.
    let (mut sim, _) = play();
    let cat = sim.app.world().resource::<SandboxCatalogue>().clone();
    let cs: Vec<_> = cat.guns.iter().filter(|g| g.game == "cs_source").collect();
    assert!(cs.len() > 10, "CS:S's guns are in the sandbox: {}", cs.len());
    for g in &cat.guns {
        assert!(!chimera_sim::weapon::names::is_string_token(&g.name), "{} is named by a token: {}", g.id, g.name);
        assert!(!g.name.to_lowercase().contains("wpnhud"), "{}: {}", g.id, g.name);
    }
    assert_eq!(cat.get("cs_source:weapon_p90").map(|g| g.name.as_str()), Some("P90"));
    // The strings arrive (the app reads them from the install or the
    // asset store after the ladder): the catalogue is made again with them.
    let mut names = sim.app.world_mut().get_resource_or_init::<GunNames>();
    names.full.insert("cs_source:weapon_p90".into(), "ES C90".into());
    sim.ticks(2);
    let cat = sim.app.world().resource::<SandboxCatalogue>();
    assert_eq!(cat.get("cs_source:weapon_p90").map(|g| g.name.as_str()), Some("ES C90"), "CS:S's own name");
    assert_eq!(cat.guns.iter().filter(|g| g.game == "cs_source").count(), cs.len());
}
