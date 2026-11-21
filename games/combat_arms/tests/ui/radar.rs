//! specs/combat_arms/radar_minimap.md's test cases that read the panels'
//! state from the world (`t25_` is test case 25): the radar (`CaRadar`:
//! zoom keys, the profile, the rotate option, who is on it, the arrows,
//! the scanner items' contacts), the scene (`CaScene`) and the full map
//! (`CaFullMap`: the legend, the spawn markers, the death marks), headless
//! on `CaSim` with the HUD's state plugin and no window.
//!
//! The local player is at the file's origin facing north unless a case
//! moves it; positions in the cases are the spec's "rel" (file X, Z).

use bevy::prelude::*;
use chimera_core::core::{Dead, LocalPlayer, MapLoads, SpawnPoint, Team};
use chimera_sim::weapon::{Inventory, give};
use chimera_world::{greybox::GreyboxMapPlugin, harness::Sim};
use combat_arms::{
    game::{
        inventory::UNIT,
        radar::{
            CaRadarProfile, CaRadarRound, CaScanner, CaScans, CaSeen,
            look::{self},
            mission::MapImage,
        },
        rules::{
            objectives::{CaCarry, CaObjectives, Flag, FlagAt},
            objects::BombZone,
        },
    },
    harness::{CaSim, with_mode},
    ui::hud::{
        CaHudStatePlugin, HudScreen,
        full_map::CaFullMap,
        model::Screen,
        radar::{CaRadar, RadarKeys},
        scene::DeathMarks,
    },
};

#[path = "../it/mode_kit.rs"]
mod mode_kit;
use mode_kit::*;

/// A file point as an engine position.
fn at(x: f32, z: f32) -> Vec3 {
    Vec3::new(x * UNIT, 1.0, -z * UNIT)
}

fn showdown() -> MapImage {
    let t = "[Object0]\nLargeMap = \"UI_HUD_Map\\mapImage\\Map_showdown.dtx\"\nLargeMapWidth = 1024\nLargeMapHeight = 1024\nLargeMapPoint0_2D = (128, 128)\nLargeMapPoint0_3D = <-2048, 0, 2304>\nLargeMapPoint1_2D = (895, 895)\nLargeMapPoint1_3D = <1536, 0, -1280>\n";
    MapImage::of_document(&lithtech_formats::attributes::Document::parse(t.as_bytes()).unwrap(), 0)
}

/// A match of `mode` with the local player on Alpha at the origin; the
/// round live. Others join with `others`.
fn live(mode: &str) -> (Sim, Entity) {
    // `mode_kit::game` with the HUD's state plugin.
    let mut sim = Sim::with(CaSim, |app| {
        app.add_plugins((GreyboxMapPlugin, CaHudStatePlugin));
    });
    {
        let w = sim.app.world_mut();
        let old: Vec<Entity> = w.query_filtered::<Entity, With<SpawnPoint>>().iter(w).collect();
        for e in old {
            w.despawn(e);
        }
        for (k, team) in [JSF, CREED].into_iter().enumerate() {
            for i in 0..2 {
                w.spawn((
                    Name::new(format!("Spawn {}", k * 2 + i)),
                    SpawnPoint { team: Some(Team(team)) },
                    Transform::from_translation(point(team, i)),
                ));
            }
        }
        with_mode(w, mode);
    }
    let me = join(&mut sim, JSF);
    sim.app.world_mut().entity_mut(me).insert((LocalPlayer, Name::new("Me")));
    until_live(&mut sim);
    if m(&sim).warmup {
        // An empty team after the warm-up ends the match: the other team
        // keeps a player, Creed's, out of the way.
        let foe = join(&mut sim, CREED);
        sim.app.world_mut().entity_mut(foe).insert(Name::new("Foe"));
        past_warmup(&mut sim);
        put(&mut sim, foe, at(-4000.0, -4000.0));
    }
    unprotect_all(&mut sim);
    (sim, me)
}

fn radar(sim: &Sim) -> CaRadar {
    sim.app.world().resource::<CaRadar>().clone()
}
fn map(sim: &Sim) -> CaFullMap {
    sim.app.world().resource::<CaFullMap>().clone()
}

/// A player of `team` held at the file point (x, z).
fn spot(sim: &mut Sim, team: u8, x: f32, z: f32, name: &str) -> Entity {
    let e = join(sim, team);
    sim.app.world_mut().entity_mut(e).insert(Name::new(name.to_string()));
    put(sim, e, at(x, z));
    e
}

/// `n` rendered frames (one tick each), `who` held where the case puts
/// them.
fn frames(sim: &mut Sim, n: usize, who: &[(Entity, Vec3)]) {
    for _ in 0..n {
        for (e, p) in who {
            put(sim, *e, *p);
        }
        sim.ticks(1);
    }
}

/// Keep everyone where the cases put them for a few frames.
fn settle(sim: &mut Sim, who: &[(Entity, Vec3)]) {
    for _ in 0..3 {
        for (e, p) in who {
            put(sim, *e, *p);
        }
        sim.ticks(1);
    }
}

fn mark_of(r: &CaRadar, e: Entity) -> Option<&combat_arms::ui::hud::radar::Mark> {
    r.marks.iter().find(|m| m.entity == Some(e))
}

fn place(sim: &mut Sim, me: Entity, others: &[(Entity, f32, f32)]) {
    let mut who: Vec<(Entity, Vec3)> = vec![(me, at(0.0, 0.0))];
    who.extend(others.iter().map(|(e, x, z)| (*e, at(*x, *z))));
    settle(sim, &who);
}

// ---------------------------------------------------------------------------
// R4 to R6 in the world

#[test]
fn t01_an_ally_on_the_radar() {
    let (mut sim, me) = live("TDM");
    let ally = spot(&mut sim, JSF, 300.0, 400.0, "Ally");
    place(&mut sim, me, &[(ally, 300.0, 400.0)]);
    let r = radar(&sim);
    assert!(r.visible);
    let m = mark_of(&r, ally).expect("the ally is on the radar");
    // (An exact half pixel on y: rounds up; f32 dust at 0.018 m a unit
    // had it down.)
    assert_eq!(m.centre, (948, 49));
    assert_eq!(m.look.icon, look::icon("character_ally.dtx"));
    assert!(m.wedge.is_some(), "an ally has a facing wedge");
}

#[test]
fn t08_an_enemy_that_fires_is_not_shown() {
    let (mut sim, me) = live("TDM");
    let enemy = spot(&mut sim, CREED, 100.0, 100.0, "Enemy");
    place(&mut sim, me, &[(enemy, 100.0, 100.0)]);
    sim.intent(enemy).fire = true;
    sim.ticks(10);
    place(&mut sim, me, &[(enemy, 100.0, 100.0)]);
    assert!(mark_of(&radar(&sim), enemy).is_none());
    assert!(map(&sim).marks.iter().all(|m| m.entity != Some(enemy)));
}

#[test]
fn t09_an_enemy_flag_carrier_is_shown_with_the_badge() {
    let (mut sim, me) = live("TDM");
    let enemy = spot(&mut sim, CREED, 100.0, 100.0, "Enemy");
    sim.app.world_mut().entity_mut(enemy).insert(CaCarry { flag: Some(JSF), ..default() });
    place(&mut sim, me, &[(enemy, 100.0, 100.0)]);
    let r = radar(&sim);
    let m = mark_of(&r, enemy).expect("a flag carrier is revealed");
    assert_eq!(m.look.icon, look::icon("character_enemy.dtx"));
    assert_eq!(m.look.badge, Some(look::icon("character_flag_A.dtx")));
}

#[test]
fn t10_a_far_carrier_gets_an_edge_arrow() {
    let (mut sim, me) = live("TDM");
    let enemy = spot(&mut sim, CREED, 2000.0, 0.0, "Enemy");
    sim.app.world_mut().entity_mut(enemy).insert(CaCarry { flag: Some(JSF), ..default() });
    // The same height.
    let y = sim.app.world().get::<Transform>(me).unwrap().translation.y;
    let mut who = vec![(me, at(0.0, 0.0)), (enemy, at(2000.0, 0.0))];
    who[0].1.y = y;
    who[1].1.y = y;
    settle(&mut sim, &who);
    let r = radar(&sim);
    assert!(mark_of(&r, enemy).is_none(), "out of range: no icon");
    let a = r.arrows.iter().find(|a| a.entity == Some(enemy)).expect("an arrow");
    // The exact point is `it::radar::t10` (the arithmetic); in the world
    // the characters' heights drift by a hair, which the 3D
    // normalisation turns into a pixel at this boundary.
    assert!((979..=980).contains(&a.point.0) && a.point.1 == 98, "{:?}", a.point);
    // Corrections 2026-10-22 (2): the arrow shows the flag badge only, in
    // place of the character icon.
    assert_eq!(a.icon_image(), look::icon("character_flag_A.dtx"));
    assert_ne!(a.icon_image(), look::icon("character_enemy.dtx"));
}

// ---------------------------------------------------------------------------
// R3 zoom and the profile

#[test]
fn t25_holding_the_zoom_key() {
    let (mut sim, _) = live("TDM");
    sim.ticks(2);
    let z0 = radar(&sim).zoom;
    assert_eq!(z0, 1.0);
    sim.app.world_mut().resource_mut::<RadarKeys>().expand = true;
    for _ in 0..30 {
        sim.ticks(1);
    }
    let r = radar(&sim);
    assert!((r.zoom - 1.30).abs() < 1e-4, "{}", r.zoom);
    assert_eq!(r.knob, 88);
    frames(&mut sim, 100, &[]);
    let r = radar(&sim);
    assert_eq!((r.zoom, r.knob), (2.0, 18));
}

#[test]
fn t26_the_zoom_clamps_at_half() {
    let (mut sim, _) = live("TDM");
    sim.app.world_mut().resource_mut::<RadarKeys>().decrease = true;
    frames(&mut sim, 80, &[]);
    let r = radar(&sim);
    assert_eq!((r.zoom, r.knob), (0.5, 168));
}

#[test]
fn t27_the_zoom_is_saved_when_it_hides_and_restored_on_a_new_map() {
    let (mut sim, me) = live("TDM");
    sim.app.world_mut().resource_mut::<RadarKeys>().expand = true;
    frames(&mut sim, 40, &[]);
    sim.app.world_mut().resource_mut::<RadarKeys>().expand = false;
    sim.ticks(1);
    assert!((radar(&sim).zoom - 1.4).abs() < 1e-4);
    // Dead: the radar hides and the zoom goes to the profile.
    kill_now(&mut sim, me, me);
    sim.ticks(3);
    assert!(!radar(&sim).visible);
    let saved = sim.app.world().resource::<CaRadarProfile>().scale;
    assert!((saved - 1.4).abs() < 1e-4, "{saved}");
    // A map load: shown again at the saved zoom.
    sim.app.world_mut().insert_resource(MapLoads(5));
    let w = sim.app.world_mut();
    w.entity_mut(me).remove::<Dead>();
    w.get_mut::<chimera_core::core::Health>(me).unwrap().current = 1.0;
    sim.ticks(3);
    let r = radar(&sim);
    assert!(r.visible);
    assert!((r.zoom - 1.4).abs() < 1e-4, "{}", r.zoom);
}

#[test]
fn t45_rotating_the_radar_mid_map_waits_for_the_next_map() {
    let (mut sim, _) = live("TDM");
    sim.ticks(2);
    assert!(!radar(&sim).rotate);
    sim.app.world_mut().resource_mut::<CaRadarProfile>().rotate = 1;
    sim.ticks(5);
    assert!(!radar(&sim).rotate, "no change until the next map");
    sim.app.world_mut().insert_resource(MapLoads(9));
    sim.ticks(2);
    assert!(radar(&sim).rotate);
}

// ---------------------------------------------------------------------------
// S1 to S3: the scanner items

/// Give `me` the item `id` and hold it with fire down.
fn hold_scanner(sim: &mut Sim, me: Entity, id: &'static str) {
    let w = sim.app.world_mut();
    give(w, me, id);
    let weapon = w
        .get::<Inventory>(me)
        .unwrap()
        .weapons
        .iter()
        .copied()
        .find(|e| w.get::<chimera_sim::weapon::Weapon>(*e).is_some_and(|x| x.id == id))
        .expect("the item");
    w.get_mut::<Inventory>(me).unwrap().wanted = Some(weapon);
    // Looking north, level.
    {
        let mut i = sim.intent(me);
        i.yaw = 0.0;
        i.pitch = 0.0;
    }
    sim.ticks(80);
    assert_eq!(in_hand(sim, me), Some(id));
}

#[test]
fn t28_a_heartbeat_contact_is_on_the_radar_only() {
    let (mut sim, me) = live("TDM");
    let enemy = spot(&mut sim, CREED, 0.0, 900.0, "Enemy");
    hold_scanner(&mut sim, me, "combat_arms:heartbeat_sensor");
    sim.intent(me).fire = true;
    for _ in 0..3 {
        put(&mut sim, me, at(0.0, 0.0));
        put(&mut sim, enemy, at(0.0, 900.0));
        sim.ticks(1);
    }
    assert!(sim.app.world().get::<CaScanner>(me).unwrap().on);
    // 900 u is past the radar's 750 at zoom 1: zoomed out it is on it.
    assert!(mark_of(&radar(&sim), enemy).is_none(), "out of the default window");
    sim.app.world_mut().resource_mut::<RadarKeys>().decrease = true;
    frames(&mut sim, 60, &[(me, at(0.0, 0.0)), (enemy, at(0.0, 900.0))]);
    sim.app.world_mut().resource_mut::<RadarKeys>().decrease = false;
    assert!(mark_of(&radar(&sim), enemy).is_some(), "on its own radar");
    // Not on the full map ...
    sim.app.world_mut().resource_mut::<RadarKeys>().map = true;
    sim.app.world_mut().resource_mut::<CaRadarRound>().image = showdown();
    sim.ticks(2);
    let f = map(&sim);
    assert!(f.visible);
    assert!(f.marks.iter().all(|m| m.entity != Some(enemy)), "{:?}", f.marks.iter().find(|m| m.entity == Some(enemy)));
    // ... and on no teammate's radar (option 0 shares nothing).
    assert_eq!(sim.app.world().get::<CaSeen>(enemy).copied().unwrap_or_default(), CaSeen::default());
}

#[test]
fn t29_30_out_of_range_or_behind_is_not_a_contact() {
    let (mut sim, me) = live("TDM");
    let far = spot(&mut sim, CREED, 0.0, 1100.0, "Far");
    let behind = spot(&mut sim, CREED, 0.0, -500.0, "Behind");
    hold_scanner(&mut sim, me, "combat_arms:heartbeat_sensor");
    sim.intent(me).fire = true;
    for _ in 0..3 {
        put(&mut sim, me, at(0.0, 0.0));
        put(&mut sim, far, at(0.0, 1100.0));
        put(&mut sim, behind, at(0.0, -500.0));
        sim.ticks(1);
    }
    let r = radar(&sim);
    assert!(mark_of(&r, far).is_none(), "1100 u ahead");
    assert!(mark_of(&r, behind).is_none(), "500 u behind");
}

#[test]
fn t31_a_detector_shares_its_list_with_the_team() {
    let (mut sim, me) = live("TDM");
    let mate = spot(&mut sim, JSF, 1200.0, 1200.0, "Mate");
    let enemy = spot(&mut sim, CREED, 0.0, 900.0, "Enemy");
    hold_scanner(&mut sim, me, "combat_arms:heartbeat_detector");
    sim.intent(me).fire = true;
    let keep = |sim: &mut Sim| {
        put(sim, me, at(0.0, 0.0));
        put(sim, mate, at(1200.0, 1200.0));
        put(sim, enemy, at(0.0, 900.0));
    };
    for _ in 0..8 {
        keep(&mut sim);
        sim.ticks(1);
    }
    let seen = sim.app.world().get::<CaSeen>(enemy).copied().unwrap_or_default();
    assert!(seen.contact_of(JSF), "within about 100 ms: {seen:?}");
    // On the teammate's radar: it is the local player now.
    {
        let w = sim.app.world_mut();
        w.entity_mut(me).remove::<LocalPlayer>();
        w.entity_mut(mate).insert(LocalPlayer);
    }
    // The teammate's radar range is 750 u: bring it near the enemy.
    for _ in 0..3 {
        put(&mut sim, me, at(0.0, 0.0));
        put(&mut sim, mate, at(300.0, 900.0));
        put(&mut sim, enemy, at(0.0, 900.0));
        sim.ticks(1);
    }
    assert!(mark_of(&radar(&sim), enemy).is_some(), "on the teammate's radar");
    // Gone when the sender releases fire.
    sim.intent(me).fire = false;
    for _ in 0..3 {
        put(&mut sim, me, at(0.0, 0.0));
        put(&mut sim, mate, at(300.0, 900.0));
        put(&mut sim, enemy, at(0.0, 900.0));
        sim.ticks(1);
    }
    assert!(mark_of(&radar(&sim), enemy).is_none());
}

#[test]
fn t32_a_satellite_scan_shows_every_enemy_for_seven_seconds() {
    let (mut sim, me) = live("TDM");
    let enemy = spot(&mut sim, CREED, 300.0, -200.0, "Enemy");
    hold_scanner(&mut sim, me, "combat_arms:satellite_scanner");
    let keep = |sim: &mut Sim| {
        put(sim, me, at(0.0, 0.0));
        put(sim, enemy, at(300.0, -200.0));
    };
    sim.intent(me).fire = true;
    keep(&mut sim);
    sim.ticks(1);
    sim.intent(me).fire = false;
    let t0 = now(&sim);
    assert!(sim.app.world().resource::<CaScans>().satellite[0].is_some());
    // Mid-scan.
    while now(&sim) - t0 < 6.9 {
        keep(&mut sim);
        sim.ticks(1);
    }
    sim.ticks(1);
    assert!(mark_of(&radar(&sim), enemy).is_some(), "still scanned at 6.9 s");
    sim.app.world_mut().resource_mut::<RadarKeys>().map = true;
    sim.app.world_mut().resource_mut::<CaRadarRound>().image = showdown();
    keep(&mut sim);
    sim.ticks(1);
    assert!(map(&sim).marks.iter().any(|m| m.entity == Some(enemy)), "and on the full map");
    while now(&sim) - t0 < 7.1 {
        keep(&mut sim);
        sim.ticks(1);
    }
    sim.ticks(2);
    assert!(mark_of(&radar(&sim), enemy).is_none(), "over at 7 s");
}

#[test]
fn t33_the_battery_runs_out_in_ten_seconds() {
    let (mut sim, me) = live("TDM");
    hold_scanner(&mut sim, me, "combat_arms:heartbeat_sensor");
    sim.intent(me).fire = true;
    let t0 = now(&sim);
    while now(&sim) - t0 < 10.2 {
        put(&mut sim, me, at(0.0, 0.0));
        sim.ticks(1);
    }
    let s = *sim.app.world().get::<CaScanner>(me).unwrap();
    assert!(s.battery <= 0.01 && !s.on, "{s:?}");
}

// ---------------------------------------------------------------------------
// F4: deaths

#[test]
fn t34_a_killed_ally_vanishes_from_the_radar_and_leaves_a_mark() {
    let (mut sim, me) = live("TDM");
    let ally = spot(&mut sim, JSF, 300.0, 400.0, "Ally");
    let enemy = spot(&mut sim, CREED, -3000.0, 0.0, "Enemy");
    sim.app.world_mut().resource_mut::<RadarKeys>().map = true;
    sim.app.world_mut().resource_mut::<CaRadarRound>().image = showdown();
    place(&mut sim, me, &[(ally, 300.0, 400.0), (enemy, -3000.0, 0.0)]);
    assert!(mark_of(&radar(&sim), ally).is_some());
    kill(&mut sim, ally, enemy);
    for _ in 0..3 {
        put(&mut sim, me, at(0.0, 0.0));
        put(&mut sim, ally, at(300.0, 400.0));
        sim.ticks(1);
    }
    assert!(mark_of(&radar(&sim), ally).is_none(), "the dead vanish from the radar");
    let f = map(&sim);
    let a = f.marks.iter().find(|m| m.entity == Some(ally)).expect("on the full map");
    assert_eq!(a.look.icon, look::icon("area_death_ally.DTX"));
    assert_eq!(f.deaths.len(), 1, "a death mark");
    let d = f.deaths[0];
    assert!((d.size().x - 54.0).abs() < 1e-3 && (d.size().y - 54.0).abs() < 1e-3);
    // Centred on the victim's icon.
    let c = (d.min + d.max) / 2.0;
    assert!((c.x - a.centre.0 as f32).abs() < 1.0 && (c.y - a.centre.1 as f32).abs() < 1.0);
}

#[test]
fn t35_the_twenty_first_kill_reuses_the_oldest_mark() {
    let mut marks = DeathMarks::default();
    for i in 0..21 {
        marks.push(combat_arms::ui::hud::scene::DeathMark((i as f64, 0.0)));
    }
    assert_eq!(marks.marks.len(), 20);
    assert_eq!(marks.marks[0].0.0, 20.0, "the first mark moved to the 21st position");
    assert_eq!(marks.marks[1].0.0, 1.0);
}

#[test]
fn t36_a_fall_leaves_no_death_mark_but_the_death_icon() {
    let (mut sim, me) = live("TDM");
    let ally = spot(&mut sim, JSF, 300.0, 400.0, "Ally");
    sim.app.world_mut().resource_mut::<RadarKeys>().map = true;
    sim.app.world_mut().resource_mut::<CaRadarRound>().image = showdown();
    place(&mut sim, me, &[(ally, 300.0, 400.0)]);
    hit(&mut sim, ally, None, 500.0, chimera_core::core::DamageKind::Fall, None);
    for _ in 0..3 {
        put(&mut sim, me, at(0.0, 0.0));
        put(&mut sim, ally, at(300.0, 400.0));
        sim.ticks(1);
    }
    let f = map(&sim);
    assert!(f.deaths.is_empty(), "no death mark for a fall");
    assert!(f.marks.iter().any(|m| m.entity == Some(ally) && m.look.icon == look::icon("area_death_ally.DTX")));
}

// ---------------------------------------------------------------------------
// R6: objectives

fn site(number: i32, family: &str, x: f32, z: f32) -> BombZone {
    bomb_zone(family, number, CREED, at(x, z))
}

#[test]
fn t37_search_and_destroy_sites() {
    let (mut sim, me) = live("TMM");
    {
        let mut o = objects(&mut sim);
        o.bomb_zones = vec![site(0, "Doobic", 200.0, 0.0), site(1, "Doobic", 3000.0, 0.0)];
    }
    place(&mut sim, me, &[]);
    let r = radar(&sim);
    assert!(r.marks.iter().any(|k| k.look.icon == look::icon("area_bomb_A.dtx")), "site A in range");
    let b = r.arrows.iter().find(|a| a.look.icon == look::icon("area_bomb_B.dtx"));
    assert!(b.is_some(), "site B out of range: an edge arrow");
}

#[test]
fn t38_bombing_run_site() {
    let (mut sim, me) = live("SBT");
    {
        let mut o = objects(&mut sim);
        o.bomb_zones = vec![site(0, "Sabotage", 200.0, 0.0)];
    }
    place(&mut sim, me, &[]);
    assert!(radar(&sim).marks.iter().any(|k| k.look.icon == look::icon("area_bomb.dtx")));
}

// ---------------------------------------------------------------------------
// F2, F5: the full map's layout

#[test]
fn t39_the_legend_and_the_spawn_markers() {
    let (mut sim, me) = live("TDM");
    sim.app.world_mut().resource_mut::<RadarKeys>().map = true;
    sim.app.world_mut().resource_mut::<CaRadarRound>().image = showdown();
    sim.app.world_mut().resource_mut::<CaRadarRound>().spawns = [Some([100.0, 100.0]), Some([-100.0, -100.0])];
    place(&mut sim, me, &[]);
    let f = map(&sim);
    assert!(f.visible);
    assert_eq!(f.legend.len(), 6, "six rows");
    for (i, row) in f.legend.iter().enumerate() {
        assert_eq!(row.at, (101.0, 109.0 + 37.0 * i as f32));
    }
    let icons: Vec<String> = f.marks.iter().map(|m| m.look.icon.clone()).collect();
    assert!(icons.contains(&look::icon("area_respawn_A.dtx")), "{icons:?}");
    assert!(icons.contains(&look::icon("area_respawn_B.dtx")), "{icons:?}");
}

#[test]
fn t40_vip_escort_markers() {
    let (mut sim, me) = live("ESC");
    sim.app.world_mut().resource_mut::<RadarKeys>().map = true;
    sim.app.world_mut().resource_mut::<CaRadarRound>().image = showdown();
    sim.app.world_mut().resource_mut::<CaRadarRound>().spawns = [Some([100.0, 100.0]), Some([-100.0, -100.0])];
    sim.app.world_mut().resource_mut::<CaObjectives>().escort = 2;
    place(&mut sim, me, &[]);
    let f = map(&sim);
    let icons: Vec<String> = f.marks.iter().map(|m| m.look.icon.clone()).collect();
    assert!(icons.contains(&look::icon("area_respawn_DF.dtx")), "{icons:?}");
    assert!(icons.contains(&look::icon("area_respawn_OF.dtx")), "{icons:?}");
}

#[test]
fn t42_without_map_fields_there_is_no_image_but_icons_still_work() {
    let (mut sim, me) = live("TDM");
    let ally = spot(&mut sim, JSF, 100.0, 100.0, "Ally");
    sim.app.world_mut().resource_mut::<RadarKeys>().map = true;
    *sim.app.world_mut().resource_mut::<CaRadarRound>() = CaRadarRound::default();
    place(&mut sim, me, &[(ally, 100.0, 100.0)]);
    let r = radar(&sim);
    assert!(r.map.is_none(), "no picture under the radar");
    assert!(mark_of(&r, ally).is_some(), "icons still work");
    assert!(map(&sim).picture.is_none(), "and none on the full map");
}

// ---------------------------------------------------------------------------
// R6, F4: Spy Hunt and Capture the Flag

#[test]
fn t43_spies_see_each_other() {
    let (mut sim, me) = live("TDM");
    let enemy = spot(&mut sim, CREED, 200.0, 0.0, "Spy");
    sim.app.world_mut().entity_mut(enemy).insert(CaCarry { intel: 1, ..default() });
    sim.app.world_mut().entity_mut(me).insert(CaCarry { intel: 1, ..default() });
    sim.app.world_mut().resource_mut::<RadarKeys>().map = true;
    sim.app.world_mut().resource_mut::<CaRadarRound>().image = showdown();
    place(&mut sim, me, &[(enemy, 200.0, 0.0)]);
    let r = radar(&sim);
    let m = mark_of(&r, enemy).expect("shown to an intel holder");
    assert_eq!(m.look.icon, look::icon("character_spy_pre.DTX"));
    assert!(map(&sim).marks.iter().any(|k| k.entity == Some(enemy) && k.look.icon == look::icon("character_spy_pre.DTX")));
    sim.app.world_mut().entity_mut(me).remove::<CaCarry>();
    place(&mut sim, me, &[(enemy, 200.0, 0.0)]);
    let r = radar(&sim);
    assert!(mark_of(&r, enemy).is_none(), "hidden once the local player drops its intel");
}

#[test]
fn t44_capture_the_flag_icons() {
    let (mut sim, me) = live("CTF");
    let (flag_a, zone_a) = flag_and_zone(JSF, at(200.0, 0.0));
    {
        let mut o = objects(&mut sim);
        o.flags = vec![flag_a];
        o.flag_zones = vec![zone_a];
    }
    sim.app.world_mut().resource_mut::<CaObjectives>().flags = vec![Flag {
        team: JSF,
        home: at(200.0, 0.0),
        at: FlagAt::Home,
    }];
    place(&mut sim, me, &[]);
    let icons = |r: &CaRadar| r.marks.iter().map(|m| m.look.icon.clone()).collect::<Vec<_>>();
    assert!(icons(&radar(&sim)).contains(&look::icon("area_flag_A.dtx")), "{:?}", icons(&radar(&sim)));
    // Dropped: the base shows the flag away and the loose flag its place.
    sim.app.world_mut().resource_mut::<CaObjectives>().flags = vec![Flag {
        team: JSF,
        home: at(200.0, 0.0),
        at: FlagAt::Dropped {
            at: at(-300.0, 100.0),
            returns: 1e9,
        },
    }];
    place(&mut sim, me, &[]);
    let got = icons(&radar(&sim));
    assert!(got.contains(&look::icon("area_flagPoint_A.dtx")), "{got:?}");
    assert!(got.contains(&look::icon("character_flag_A.dtx")), "{got:?}");
}

#[test]
fn the_screen_is_stretched_not_squashed() {
    // x_r, y_r: the window's rect at 1920 x 1080 (R4's last line).
    let (mut sim, _) = live("TDM");
    sim.app.world_mut().insert_resource(HudScreen(Screen::new(1920.0, 1080.0)));
    sim.ticks(2);
    let w = radar(&sim).window.unwrap();
    assert!((w.min.x - 818.0 * 1.875).abs() < 1e-3 && (w.min.y - 5.0 * 1.40625).abs() < 1e-3);
}

// ---------------------------------------------------------------------------
// Non-4:3 screens: the turned quads (wedge, guide, edge arrows) turn at the
// view's speed and keep their shape; the spec's corners are sheared there.

/// The edge u's angle of an upright quad.
fn angle_of(q: [Vec2; 4]) -> f32 {
    let u = q[1] - q[0];
    u.y.atan2(u.x)
}

fn wrap(a: f32) -> f32 {
    let t = std::f32::consts::TAU;
    (a + std::f32::consts::PI).rem_euclid(t) - std::f32::consts::PI
}

#[test]
fn a_wedge_turns_uniformly_on_a_16_9_screen() {
    use combat_arms::{game::radar::maths, ui::hud::radar::upright};
    let s = Screen::new(1920.0, 1080.0);
    let step = std::f64::consts::TAU / 36.0;
    let mut last: Option<f32> = None;
    for i in 0..37 {
        let yaw = step * i as f64;
        let q = maths::wedge((948, 48), yaw, 0.0, 1.0, s.xr() as f64, s.yr() as f64);
        let u = upright(q, s);
        // A rectangle of the icon's 2:1 sides, whatever the yaw.
        let (a, b) = (u[1] - u[0], u[3] - u[0]);
        assert!(a.dot(b).abs() < 0.05, "square corners at {yaw}: {u:?}");
        assert!((a.length() / b.length() - 2.0).abs() < 1e-3, "sides at {yaw}: {u:?}");
        // Its angle is the view's: equal steps for equal yaw steps.
        let ang = angle_of(u);
        if let Some(l) = last {
            assert!((wrap(ang - l) - step as f32).abs() < 1e-3, "step at {yaw}: {}", wrap(ang - l));
        }
        last = Some(ang);
        // The same centre as the spec's quad.
        assert!(((q[0] + q[2]) / 2.0 - (u[0] + u[2]) / 2.0).length() < 1e-3);
    }
}

#[test]
fn the_guide_turns_uniformly_on_a_16_9_screen() {
    use combat_arms::{
        game::radar::maths::{self, Frame},
        ui::hud::radar::upright,
    };
    let s = Screen::new(1920.0, 1080.0);
    let step = std::f64::consts::TAU / 36.0;
    let mut last: Option<f32> = None;
    for i in 0..37 {
        let f = Frame::new((0.0, 0.0), step * i as f64, 1.0, false);
        let g = upright(maths::guide(&f, s.xr() as f64, s.yr() as f64), s);
        let (a, b) = (g[1] - g[0], g[3] - g[0]);
        assert!(a.dot(b).abs() < 0.5 && (a.length() - b.length()).abs() < 1e-2, "{g:?}");
        let ang = angle_of(g);
        if let Some(l) = last {
            assert!((wrap(ang - l) - step as f32).abs() < 1e-3);
        }
        last = Some(ang);
    }
}

/// In the world at 16:9: the wedge drawn upright, the window inside the
/// frame box.
#[test]
fn an_ally_wedge_at_16_9() {
    let (mut sim, me) = live("TDM");
    sim.app.world_mut().insert_resource(HudScreen(Screen::new(1920.0, 1080.0)));
    let ally = spot(&mut sim, JSF, 300.0, 400.0, "Ally");
    place(&mut sim, me, &[(ally, 300.0, 400.0)]);
    let r = radar(&sim);
    let w = mark_of(&r, ally).and_then(|m| m.wedge).expect("wedge");
    let u = combat_arms::ui::hud::radar::upright(w, r.screen);
    let (a, b) = (u[1] - u[0], u[3] - u[0]);
    assert!(a.dot(b).abs() < 0.05 && (a.length() / b.length() - 2.0).abs() < 1e-3, "{u:?}");
    let win = r.window.expect("window");
    let (fx, fy, fw, fh) = combat_arms::ui::hud::radar::FRAME_AT;
    let frame = r.screen.rect(Vec2::new(fx, fy), Vec2::new(fw, fh));
    assert!(win.min.x >= frame.min.x && win.min.y >= frame.min.y && win.max.x <= frame.max.x && win.max.y <= frame.max.y);
}

/// The frame and the map over the window at any resolution (the backlog's
/// "two boxes"; Corrections 2026-10-28, 1, cases 53-55): the map quad is
/// the layout's (725, -88) 372 x 372, twice the window (818, 5) 186 x 186
/// about its centre (911, 98); the frame is `bg.dtx` unstretched at one
/// texel per ref px from (782, -30), so its outline (texel columns 35 and
/// 221, rows 34 and 220) lands on pixels 817 and 1003 across, 4 and 190
/// down at 1024 x 768: one pixel outside the window's left and top, on
/// its last column and row.
#[test]
fn the_frame_and_the_map_sit_over_the_window_at_any_resolution() {
    for (w, h) in [(1024.0, 768.0), (1920.0, 1080.0), (2560.0, 1440.0)] {
        let (mut sim, me) = live("TDM");
        sim.app.world_mut().insert_resource(HudScreen(Screen::new(w, h)));
        sim.app.world_mut().resource_mut::<CaRadarRound>().image = showdown();
        place(&mut sim, me, &[]);
        let r = radar(&sim);
        let (xr, yr) = (w / 1024.0, h / 768.0);
        let near = |a: f32, b: f32| assert!((a - b).abs() < 0.5, "{w}x{h}: {a} vs {b}");
        let win = r.window.expect("window");
        near(win.min.x, 818.0 * xr);
        near(win.min.y, 5.0 * yr);
        near(win.size().x, 186.0 * xr);
        near(win.size().y, 186.0 * yr);
        let q = r.map.as_ref().expect("map quad").corners;
        near(q[0].x, 725.0 * xr);
        near(q[0].y, -88.0 * yr);
        near(q[2].x, (725.0 + 372.0) * xr);
        near(q[2].y, (-88.0 + 372.0) * yr);
        let (fx, fy, fw, fh) = combat_arms::ui::hud::radar::FRAME_AT;
        assert_eq!((fx, fy, fw, fh), (782.0, -30.0, 256.0, 256.0), "the image at its own size");
        let frame = r.screen.rect(Vec2::new(fx, fy), Vec2::new(fw, fh));
        // One texel per ref px: the image is scaled by the screen's stretch only.
        near(frame.size().x, 256.0 * xr);
        near(frame.size().y, 256.0 * yr);
        let ((c0, c1), (r0, r1)) = combat_arms::ui::hud::radar::FRAME_OUTLINE;
        // Case 55: the outline's left/top are one ref px outside the window,
        // its right/bottom on the window's last column and row.
        near(frame.min.x + c0 * xr, win.min.x - xr);
        near(frame.min.y + r0 * yr, win.min.y - yr);
        near(frame.min.x + c1 * xr, win.max.x - xr);
        near(frame.min.y + r1 * yr, win.max.y - yr);
        if (w, h) == (1024.0, 768.0) {
            // Case 53: pixels 817 and 1003, 4 and 190.
            near(frame.min.x + c0, 817.0);
            near(frame.min.x + c1, 1003.0);
            near(frame.min.y + r0, 4.0);
            near(frame.min.y + r1, 190.0);
        }
    }
}

/// Case 54: at 1920 x 1080 the control (782, -30) 227 x 226 is the quad
/// x 1466.25, y -42.1875, 425.625 x 317.8125 (the visible part of the
/// image: the rest of the 256 x 256 is transparent).
#[test]
fn the_frame_control_at_1920_x_1080() {
    let s = Screen::new(1920.0, 1080.0);
    let r = s.rect(Vec2::new(782.0, -30.0), Vec2::new(227.0, 226.0));
    let near = |a: f32, b: f32| assert!((a - b).abs() < 1e-2, "{a} vs {b}");
    near(r.min.x, 1466.25);
    near(r.min.y, -42.1875);
    near(r.size().x, 425.625);
    near(r.size().y, 317.8125);
}

/// Case 57: Warehouse (Showdown's data: top-left z 2966, 4784.897 tall,
/// world offset z 1216), zoom 1: the window's top edge samples
/// v = (2902.112 - 2966) / 4784.897 = -0.01335, clamped to the image's
/// first row, not wrapped or dropped.
#[test]
fn case_57_the_window_may_reach_above_the_image() {
    use combat_arms::game::radar::maths::{Corners, Frame, window};
    let c = Corners { lt: (-2000.0, 2902.112), rb: (2784.897, 2902.112 - 4784.897) };
    // The window's top edge is 2966 in the formula's terms (z + W_r + O_z).
    let f = Frame { x: 0.0, z: 250.0, y: 0.0, yaw: 0.0, zoom: 1.0, rotate: false };
    let w = window(&f, (0.0, 1216.0), &c);
    assert!((w.v0 - (-0.01335)).abs() < 1e-4, "{}", w.v0);
    assert!(w.v0 < 0.0, "outside [0, 1]: the shader clamps it");
}

// ---------------------------------------------------------------------------
// Corrections 2026-10-22

/// Correction 3: held, the full map stays through the round's end.
#[test]
fn corr3_the_full_map_stays_after_the_round_ends() {
    use combat_arms::game::rules::{CaMatchView, Phase};
    let (mut sim, me) = live("TDM");
    sim.app.world_mut().resource_mut::<CaRadarRound>().image = showdown();
    sim.app.world_mut().resource_mut::<RadarKeys>().map = true;
    place(&mut sim, me, &[]);
    assert!(map(&sim).visible);
    sim.app.world_mut().resource_mut::<CaMatchView>().phase = Phase::MatchOver;
    place(&mut sim, me, &[]);
    assert!(map(&sim).visible, "held across the end");
}

/// Correction 4: a dead enemy a scan still reveals stays on the full map
/// with its living icon and no wedge, at its death spot; it is not on the
/// radar.
#[test]
fn corr4_a_revealed_dead_enemy_stays_where_it_died() {
    let (mut sim, me) = live("TDM");
    let enemy = spot(&mut sim, CREED, 300.0, -200.0, "Enemy");
    let killer = spot(&mut sim, JSF, -3000.0, 0.0, "Killer");
    hold_scanner(&mut sim, me, "combat_arms:satellite_scanner");
    sim.app.world_mut().resource_mut::<RadarKeys>().map = true;
    sim.app.world_mut().resource_mut::<CaRadarRound>().image = showdown();
    sim.intent(me).fire = true;
    place(&mut sim, me, &[(enemy, 300.0, -200.0), (killer, -3000.0, 0.0)]);
    sim.intent(me).fire = false;
    kill(&mut sim, enemy, killer);
    let there = [(me, at(0.0, 0.0)), (enemy, at(300.0, -200.0)), (killer, at(-3000.0, 0.0))];
    settle(&mut sim, &there);
    // The body is pushed away afterwards: the mark does not follow.
    let moved = [(me, at(0.0, 0.0)), (enemy, at(500.0, -200.0)), (killer, at(-3000.0, 0.0))];
    settle(&mut sim, &moved);
    assert!(mark_of(&radar(&sim), enemy).is_none(), "the dead are not on the radar");
    let f = map(&sim);
    let m = f.marks.iter().find(|m| m.entity == Some(enemy)).expect("revealed while dead");
    assert_eq!(m.look.icon, look::icon("character_enemy.dtx"));
    assert!(m.wedge.is_none());
    let scene = sim.app.world().resource::<combat_arms::ui::hud::scene::CaScene>();
    let t = scene.things.iter().find(|t| t.entity == Some(enemy)).unwrap();
    assert!((t.at.0 - 300.0).abs() < 2.0, "{:?}", t.at);
}

/// Correction 4: a dead ally stays at its death spot.
#[test]
fn corr4_a_dead_ally_does_not_follow_the_body() {
    let (mut sim, me) = live("TDM");
    let ally = spot(&mut sim, JSF, 300.0, 400.0, "Ally");
    let enemy = spot(&mut sim, CREED, -3000.0, 0.0, "Enemy");
    sim.app.world_mut().resource_mut::<RadarKeys>().map = true;
    sim.app.world_mut().resource_mut::<CaRadarRound>().image = showdown();
    place(&mut sim, me, &[(ally, 300.0, 400.0), (enemy, -3000.0, 0.0)]);
    kill(&mut sim, ally, enemy);
    settle(&mut sim, &[(me, at(0.0, 0.0)), (ally, at(300.0, 400.0)), (enemy, at(-3000.0, 0.0))]);
    settle(&mut sim, &[(me, at(0.0, 0.0)), (ally, at(500.0, 400.0)), (enemy, at(-3000.0, 0.0))]);
    let scene = sim.app.world().resource::<combat_arms::ui::hud::scene::CaScene>();
    let t = scene.things.iter().find(|t| t.entity == Some(ally)).unwrap();
    assert!((t.at.0 - 300.0).abs() < 2.0, "{:?}", t.at);
    assert_eq!(t.map.as_ref().unwrap().icon, look::icon("area_death_ally.DTX"));
}
