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
    play_with(map, ())
}

/// `play_on` with more plugins (added before the app is built).
fn play_with<M>(map: Option<&str>, extra: impl bevy::app::Plugins<M>) -> (Sim, Entity) {
    let mut sim = Sim::new(
        combat_arms::harness::CaSim,
        (chimera_world::greybox::GreyboxMapPlugin, party_kit::PartyPartsPlugin, ultimate_gun_game::UltimateGunGamePlugin::default(), SandboxUiPlugin, extra),
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

/// B opens Change gun once per press and closes it on the next, with
/// each game's character, through the real input path (key events, the
/// key held with the system's repeats; playtest 2026-11-18: it opened and
/// closed at once, the view reading the opening press as its close). No
/// game's bind on B or T acts while the sandbox plays (CS:S's `buymenu`,
/// Combat Arms' radio and push to talk: the sandbox owns the keys,
/// `binds::OwnedKeys`), and T still opens the bench.
#[test]
fn b_toggles_change_gun_once_per_press_with_every_games_character_and_no_bind_acts() {
    use bevy::input::{
        ButtonState,
        keyboard::{Key, KeyboardInput},
    };
    use chimera_client::client::{binds, sandbox::SandboxScreen};
    use chimera_core::{console::Console, slots::GameIdentity};
    let movements = ["cs_source:movement", "combat_arms:movement", "hl2dm:movement", "tf2:movement", "halo_ce:movement"];
    for movement in movements {
        let (mut sim, me) = play();
        // The platform's key path (InputPlugin's keyboard system): key
        // events into the pressed keys, as a real client has.
        sim.app
            .init_resource::<ButtonInput<bevy::input::keyboard::Key>>()
            .init_resource::<ButtonInput<MouseButton>>()
            .add_message::<bevy::input::keyboard::KeyboardFocusLost>()
            .add_systems(PreUpdate, bevy::input::keyboard::keyboard_input_system.in_set(bevy::input::InputSystems));
        let game = movement.split(':').next().unwrap();
        sim.app.world_mut().commands().queue(chimera_core::slots::set_movement(me, movement));
        sim.app.world_mut().entity_mut(me).insert(GameIdentity(game.into()));
        // Each game's binds on B and T (its layer's, else CS:S's).
        let lines: Vec<(String, String)> = {
            let c = sim.app.world().resource::<Console>();
            let binds = c.effective_binds(Some(game));
            ["b", "t"].iter().map(|k| (k.to_string(), binds.get(*k).cloned().unwrap_or_else(|| if *k == "b" { "buymenu".into() } else { "+voicechat".into() }))).collect()
        };
        for (k, line) in &lines {
            sim.app.world_mut().resource_mut::<Console>().binds.insert(k.clone(), line.clone());
        }
        let commands: Vec<String> = lines.iter().map(|(_, l)| l.split(';').next().unwrap().split_whitespace().next().unwrap_or("").to_string()).collect();
        let key = |sim: &mut Sim, code: KeyCode, ch: &str, state: ButtonState, repeat: bool| {
            sim.app.world_mut().write_message(KeyboardInput {
                key_code: code,
                logical_key: Key::Character(ch.into()),
                state,
                text: (state == ButtonState::Pressed).then(|| ch.into()),
                repeat,
                window: Entity::PLACEHOLDER,
            });
        };
        let screen = |sim: &Sim| sim.app.world().resource::<SandboxUi>().screen;
        // No game's bind sees the key, this frame or while held.
        let bind_acts = |sim: &Sim| {
            let w = sim.app.world();
            let (c, k, m) = (w.resource::<Console>(), w.resource::<ButtonInput<KeyCode>>(), w.resource::<ButtonInput<MouseButton>>());
            let captured = w.get_resource::<binds::CapturedKeys>();
            commands.iter().find(|cmd| binds::just_pressed(&c.binds, k, m, cmd) || binds::pressed_free(&c.binds, k, m, captured, cmd)).cloned()
        };
        sim.app.update();
        sim.app.update();
        assert!(sim.app.world().resource::<SandboxUi>().on, "{movement}: the sandbox plays");
        for (round, open) in [(0, SandboxScreen::ChangeGun), (1, SandboxScreen::None), (2, SandboxScreen::ChangeGun)] {
            key(&mut sim, KeyCode::KeyB, "b", ButtonState::Pressed, false);
            sim.app.update();
            assert_eq!(bind_acts(&sim), None, "{movement}, press {round}: a bind on B acted");
            for f in 0..40 {
                if f % 3 == 0 {
                    key(&mut sim, KeyCode::KeyB, "b", ButtonState::Pressed, true);
                }
                sim.app.update();
                assert_eq!(screen(&sim), open, "{movement}, press {round}, held frame {f}");
                assert_eq!(bind_acts(&sim), None, "{movement}, press {round}, held frame {f}: a bind on B acted");
            }
            key(&mut sim, KeyCode::KeyB, "b", ButtonState::Released, false);
            for _ in 0..3 {
                sim.app.update();
            }
            assert_eq!(screen(&sim), open, "{movement}, press {round}: released");
        }
        // T: the bench, the game's T bind (push to talk) silent.
        key(&mut sim, KeyCode::KeyB, "b", ButtonState::Pressed, false);
        sim.app.update();
        key(&mut sim, KeyCode::KeyB, "b", ButtonState::Released, false);
        sim.app.update();
        assert_eq!(screen(&sim), SandboxScreen::None, "{movement}: closed");
        key(&mut sim, KeyCode::KeyT, "t", ButtonState::Pressed, false);
        sim.app.update();
        assert_eq!(bind_acts(&sim), None, "{movement}: a bind on T acted");
        sim.app.update();
        assert_eq!(screen(&sim), SandboxScreen::Bench, "{movement}: T opens the bench");
    }
}

/// Each command's `fire` as the simulation read it.
#[derive(Resource, Default)]
struct Fired(Vec<bool>);

fn record_fire(me: Single<&chimera_core::core::Intent, With<LocalPlayer>>, mut f: ResMut<Fired>) {
    f.0.push(me.fire);
}

/// The sandbox's screens own the mouse as the Escape menu's sync makes
/// them (`front_end::AppMenu` open while a screen is up).
fn app_menu_follows_the_sandbox(ui: Res<SandboxUi>, mut m: ResMut<chimera_client::client::front_end::AppMenu>) {
    let open = ui.screen_open();
    if m.open != open {
        m.open = open;
    }
}

/// A double-click on a gun in Change gun (press, release, press: the
/// list's `Activate` on the second press) switches to it and closes the
/// view, the button still down: no command fires, neither with the gun
/// in hand before nor after, and the picked gun is in hand. (Bug: the
/// cursor was grabbed with the button held and the old gun fired once.)
#[test]
fn a_double_click_in_change_gun_switches_without_a_shot() {
    use bevy::{
        input::mouse::{AccumulatedMouseMotion, AccumulatedMouseScroll},
        window::{CursorGrabMode, CursorOptions},
    };
    use chimera_client::client::{
        front_end::AppMenu,
        input::LocalInputPlugin,
        sandbox::{SandboxAction, SandboxScreen, change_gun::LIST},
        view::FreeCam,
    };
    use chimera_core::console::Console;
    use chimera_sim::minigame::ladder::{Ladder, Rung};
    use chimera_ui::vlist::{VCursor, VListEvent};
    /// The client's own mouse input over the sandbox.
    struct ClickInput;
    impl Plugin for ClickInput {
        fn build(&self, app: &mut App) {
            app.init_resource::<ButtonInput<MouseButton>>()
                .init_resource::<AccumulatedMouseMotion>()
                .init_resource::<AccumulatedMouseScroll>()
                .init_resource::<FreeCam>()
                .init_resource::<AppMenu>()
                .init_resource::<Fired>()
                .add_message::<bevy::window::WindowFocused>()
                .add_plugins(LocalInputPlugin)
                .add_systems(Update, app_menu_follows_the_sandbox)
                .add_systems(FixedUpdate, record_fire);
        }
    }
    let (mut sim, me) = play_with(None, ClickInput);
    sim.app.world_mut().spawn(CursorOptions { grab_mode: CursorGrabMode::Locked, visible: false, ..default() });
    sim.app.world_mut().resource_mut::<Console>().submit("bind mouse1 +attack");
    let ladder = sim.app.world().resource::<Ladder>().clone();
    let second: &'static str = ladder.rungs.iter().copied().find(|r| *r != GUN && r.starts_with("combat_arms:") && !r.contains("grenade")).expect("a second gun");
    sim.app.world_mut().write_message(SandboxAction::Switch(GUN.into()));
    sim.seconds(0.5);
    let rung_gun = |sim: &Sim| sim.app.world().get::<Rung>(me).and_then(|r| ladder.rungs.get(r.0).copied());
    assert_eq!(rung_gun(&sim), Some(GUN));
    // One frame; the presses' just-pressed go as Bevy's input clears them.
    let frame = |sim: &mut Sim| {
        sim.app.update();
        sim.app.world_mut().resource_mut::<ButtonInput<MouseButton>>().clear();
    };
    let grabbed = |sim: &mut Sim| sim.app.world_mut().query::<&CursorOptions>().single(sim.app.world()).unwrap().grab_mode != CursorGrabMode::None;
    // Change gun open: the cursor free.
    sim.app.world_mut().resource_mut::<SandboxUi>().screen = SandboxScreen::ChangeGun;
    for _ in 0..3 {
        frame(&mut sim);
    }
    assert!(!grabbed(&mut sim), "Change gun freed the cursor");
    sim.app.world_mut().resource_mut::<Fired>().0.clear();
    // Press (selects the gun), release, press (switches: `Activate`).
    sim.app.world_mut().resource_mut::<SandboxUi>().view.selected = Some(second.into());
    mouse_of(&mut sim).press(MouseButton::Left);
    frame(&mut sim);
    mouse_of(&mut sim).release(MouseButton::Left);
    frame(&mut sim);
    mouse_of(&mut sim).press(MouseButton::Left);
    sim.app.world_mut().write_message(VListEvent::Activate { key: LIST.into(), at: VCursor::Item(0, 0) });
    frame(&mut sim);
    assert_eq!(sim.app.world().resource::<SandboxUi>().screen, SandboxScreen::None, "the double-click closed the view");
    // The button held on into the game for half a second.
    let tick = sim.tick();
    while sim.tick() < tick + (0.5 * sim.tick_hz()) as u64 {
        frame(&mut sim);
    }
    assert!(grabbed(&mut sim), "back in the game");
    assert_eq!(rung_gun(&sim), Some(second), "the picked gun in hand");
    let fired = sim.app.world().resource::<Fired>().0.iter().filter(|f| **f).count();
    assert_eq!(fired, 0, "commands fired by the clicks meant for the picker");
    // Let go, a new press fires (the picked gun).
    mouse_of(&mut sim).release(MouseButton::Left);
    frame(&mut sim);
    mouse_of(&mut sim).press(MouseButton::Left);
    let tick = sim.tick();
    while sim.tick() < tick + 2 {
        frame(&mut sim);
    }
    assert!(sim.app.world().resource::<Fired>().0.iter().any(|f| *f), "a press in the game fires");
}

fn mouse_of(sim: &mut Sim) -> Mut<'_, ButtonInput<MouseButton>> {
    sim.app.world_mut().resource_mut::<ButtonInput<MouseButton>>()
}
