//! specs/combat_arms/radar_minimap.md's test cases (`t01_` is test case 1)
//! that need no window: the arithmetic (`game::radar::maths`: pixels, range,
//! rotation, wedges, the guide, edge arrows, the image window, the full
//! map's transform, zoom, the corners from the mission data), who is shown
//! (`radar::look`), the legend, the scanner items and what the server sends
//! at a round start. The panels' own state (zoom keys, the profile, the
//! scene read from the world, the full map) is tests/ui/radar.rs; the
//! install's images and corners are `heavy::radar`.
//!
//! The Junk Flea, Showdown, Junk Flea 2 and WarHead map data are the spec's
//! install checks (46 to 51) written as mission-file text.

use bevy::prelude::*;
use combat_arms::game::radar::{
    legend,
    look::{self, Subject, Viewer},
    maths::{self, Corners, Frame, FullMap},
    mission::MapImage,
};
use combat_arms::game::rules::objectives::CaCarry;
use lithtech_formats::attributes::Document;

fn near(a: f32, b: f32, tol: f32) -> bool {
    (a - b).abs() <= tol
}

fn fixed(z: f64) -> Frame {
    Frame::new((0.0, 0.0), 0.0, z, false)
}

/// The mission-file text of one objective's map fields.
fn mission(image: &str, p0: ((i32, i32), (f64, f64)), p1: ((i32, i32), (f64, f64))) -> String {
    format!(
        "[Object0]\nLargeMap = \"UI_HUD_Map\\mapImage\\{image}\"\nLargeMapWidth = 1024\nLargeMapHeight = 1024\nLargeMapPoint0_2D = ({}, {})\nLargeMapPoint0_3D = <{}, 0, {}>\nLargeMapPoint1_2D = ({}, {})\nLargeMapPoint1_3D = <{}, 0, {}>\n",
        (p0.0).0,
        (p0.0).1,
        (p0.1).0,
        (p0.1).1,
        (p1.0).0,
        (p1.0).1,
        (p1.1).0,
        (p1.1).1,
    )
}

pub fn junk_flea() -> MapImage {
    let t = mission(
        "Map_JunkFlea.dtx",
        ((128, 128), (-1408.0, 1408.0)),
        ((895, 895), (2688.0, -2688.0)),
    );
    MapImage::of_document(&Document::parse(t.as_bytes()).unwrap(), 0)
}

pub fn showdown() -> MapImage {
    let t = mission(
        "Map_showdown.dtx",
        ((128, 128), (-2048.0, 2304.0)),
        ((895, 895), (1536.0, -1280.0)),
    );
    MapImage::of_document(&Document::parse(t.as_bytes()).unwrap(), 0)
}

pub fn junk_flea2() -> MapImage {
    let t = mission(
        "Map_JunkFlea2.dtx",
        ((128, 128), (-2560.0, 2560.0)),
        ((895, 895), (2560.0, -2560.0)),
    );
    MapImage::of_document(&Document::parse(t.as_bytes()).unwrap(), 0)
}

pub fn warhead() -> MapImage {
    let t = mission(
        "Map_WarHead.dtx",
        ((128, 128), (-2048.0, 3072.0)),
        ((895, 895), (4096.0, -3072.0)),
    );
    MapImage::of_document(&Document::parse(t.as_bytes()).unwrap(), 0)
}

// ---------------------------------------------------------------------------
// R4: world to radar pixel

#[test]
fn t01_ally_in_range() {
    assert_eq!(fixed(1.0).pixel((300.0, 400.0)), Some((948, 48)));
}

#[test]
fn t02_zoom_two() {
    assert_eq!(fixed(2.0).pixel((300.0, -200.0)), Some((985, 147)));
}

#[test]
fn t03_zoom_half() {
    assert_eq!(fixed(0.5).pixel((-1010.0, 1210.0)), Some((848, 22)));
}

#[test]
fn t04_outside_the_square_is_not_drawn_and_has_no_arrow() {
    let f = fixed(1.0);
    assert!(!f.in_range((751.0, 0.0)));
    assert_eq!(f.pixel((751.0, 0.0)), None);
    // A teammate has no arrow: the radar's marks for it are none (the
    // arrow is a carrier's or an objective's: `look::ally`).
    let s = Subject {
        ally: true,
        alive: true,
        ..default()
    };
    assert!(!look::radar_player(&Viewer { team: 1, intel: false }, &s).unwrap().arrow);
}

#[test]
fn t05_the_range_is_a_square() {
    // 990 u away, drawn.
    assert!(fixed(1.0).pixel((700.0, 700.0)).is_some());
    // The edge counts.
    assert!(fixed(1.0).in_range((750.0, -750.0)));
}

#[test]
fn t06_zoom_two_shortens_the_range() {
    assert_eq!(fixed(2.0).pixel((400.0, 0.0)), None);
    assert!(fixed(2.0).pixel((375.0, 0.0)).is_some(), "the limit is 375");
}

#[test]
fn t07_rotating_radar() {
    let f = Frame::new((0.0, 0.0), std::f64::consts::FRAC_PI_2, 1.0, true);
    assert_eq!(f.pixel((300.0, 100.0)), Some((898, 60)));
    assert_eq!(fixed(1.0).pixel((300.0, 100.0)), Some((948, 85)));
}

// ---------------------------------------------------------------------------
// R6: who is shown

fn me() -> Viewer {
    Viewer { team: 1, intel: false }
}

#[test]
fn t08_an_enemy_is_hidden_whatever_it_fires() {
    // Firing, silenced or not, isn't an input: an enemy with nothing that
    // reveals it is not drawn (the world-level case: tests/ui/radar.rs).
    let s = Subject {
        alive: true,
        ..default()
    };
    assert_eq!(look::radar_player(&me(), &s), None);
}

#[test]
fn t09_an_enemy_flag_carrier_has_the_badge() {
    // The enemy is on Bravo and carries Alpha's flag.
    let s = Subject {
        alive: true,
        carry: CaCarry {
            flag: Some(1),
            ..default()
        },
        ..default()
    };
    let l = look::radar_player(&me(), &s).unwrap();
    assert_eq!(l.icon, look::icon("character_enemy.dtx"));
    assert_eq!(l.badge, Some(look::icon("character_flag_A.dtx")));
    assert!(l.arrow);
}

// ---------------------------------------------------------------------------
// R8: edge arrows

fn arrow_for(rel: (f64, f64), dy: f64) -> maths::Arrow {
    maths::arrow(rel.0, dy, rel.1)
}

#[test]
fn t10_a_carrier_to_the_east() {
    let a = arrow_for((2000.0, 0.0), 0.0);
    assert_eq!(a.point, (980, 98));
    // The box's top-left, and the arrow image unturned (+u right).
    assert_eq!((a.point.0 - 32, a.point.1 - 16), (948, 82));
    let q = maths::arrow_corners(&a, 1.0, 1.0);
    assert!(near(q[0].x, 948.0, 1e-3) && near(q[0].y, 82.0, 1e-3), "{q:?}");
    assert!(near(q[1].x, 1012.0, 1e-3) && near(q[1].y, 82.0, 1e-3), "{q:?}");
}

#[test]
fn t11_a_flag_base_to_the_north_east() {
    assert_eq!(arrow_for((1000.0, 2000.0), 0.0).point, (946, 28));
}

#[test]
fn t12_height_pulls_the_arrow_inwards() {
    assert_eq!(arrow_for((2000.0, 0.0), 2000.0).point, (959, 98));
}

// ---------------------------------------------------------------------------
// R7: wedges and the guide

fn wedge_at_13(z: f64, xr: f64, yr: f64) -> [Vec2; 4] {
    let f = fixed(z);
    let c = f.pixel((300.0, 400.0)).unwrap();
    maths::wedge(c, 0.0, f.rot(), z, xr, yr)
}

#[test]
fn t13_the_wedge_of_an_ally_facing_north() {
    let q = wedge_at_13(1.0, 1.0, 1.0);
    for (got, want) in q
        .iter()
        .zip([(932.0, 80.0), (932.0, 16.0), (964.0, 16.0), (964.0, 80.0)])
    {
        assert!(near(got.x, want.0, 0.01) && near(got.y, want.1, 0.01), "{q:?}");
    }
}

#[test]
fn t14_the_wedge_at_1920_by_1080() {
    let q = wedge_at_13(1.0, 1920.0 / 1024.0, 1080.0 / 768.0);
    assert!(near(q[0].x, 1747.5, 0.01) && near(q[0].y, 112.5, 0.01), "{q:?}");
    assert!(near(q[1].x, 1747.5, 0.01) && near(q[1].y, 22.5, 0.01), "{q:?}");
}

#[test]
fn t15_the_wedge_grows_with_the_zoom() {
    let f = fixed(2.0);
    let c = f.pixel((100.0, 200.0)).unwrap();
    let q = maths::wedge(c, 0.0, 0.0, 2.0, 1.0, 1.0);
    let (cx, cy) = (c.0 as f32, c.1 as f32);
    // +-32 across and +-64 along, from the zoom-2 position.
    for (got, want) in q
        .iter()
        .zip([(-32.0, 64.0), (-32.0, -64.0), (32.0, -64.0), (32.0, 64.0)])
    {
        assert!(
            near(got.x, cx + want.0, 0.01) && near(got.y, cy + want.1, 0.01),
            "{q:?}"
        );
    }
}

#[test]
fn t16_the_guide_upright() {
    let q = maths::guide(&fixed(1.0), 1.0, 1.0);
    for (got, want) in q
        .iter()
        .zip([(653.0, -161.0), (1165.0, -161.0), (1165.0, 351.0), (653.0, 351.0)])
    {
        assert!(near(got.x, want.0, 0.01) && near(got.y, want.1, 0.01), "{q:?}");
    }
}

#[test]
fn t17_the_guide_turns_with_the_view() {
    let f = Frame::new((0.0, 0.0), std::f64::consts::FRAC_PI_2, 1.0, false);
    let q = maths::guide(&f, 1.0, 1.0);
    assert!(near(q[0].x, 1165.0, 0.01) && near(q[0].y, -161.0, 0.01), "{q:?}");
    assert!(near(q[1].x, 1165.0, 0.01) && near(q[1].y, 351.0, 0.01), "{q:?}");
    // Rotating mode: not turned.
    let f = Frame { rotate: true, ..f };
    assert!(near(maths::guide(&f, 1.0, 1.0)[0].x, 653.0, 0.01));
}

// ---------------------------------------------------------------------------
// R5: the image window

#[test]
fn t18_the_window_on_junk_flea() {
    let c = junk_flea().corners.unwrap();
    let w = maths::window(&fixed(1.0), (0.0, 0.0), &c);
    for (got, want) in [(w.u0, 0.108176), (w.v0, 0.108176), (w.uw, 0.548601), (w.vh, 0.548601)] {
        assert!((got - want).abs() < 2e-6, "{w:?}");
    }
}

#[test]
fn t19_the_window_moves_and_shrinks() {
    let c = junk_flea().corners.unwrap();
    let f = Frame::new((500.0, -300.0), 0.0, 2.0, false);
    let w = maths::window(&f, (0.0, 0.0), &c);
    for (got, want) in [(w.u0, 0.336760), (w.v0, 0.300187), (w.uw, 0.274301), (w.vh, 0.274301)] {
        assert!((got - want).abs() < 2e-6, "{w:?}");
    }
}

#[test]
fn t20_the_window_adds_the_world_offset() {
    let c = showdown().corners.unwrap();
    let w = maths::window(&fixed(1.0), (-480.0, 1216.0), &c);
    for (got, want) in [(w.u0, 0.139211), (w.v0, 0.038896), (w.uw, 0.626973), (w.vh, 0.626973)] {
        assert!((got - want).abs() < 2e-6, "{w:?}");
    }
}

// ---------------------------------------------------------------------------
// F3: world to full-map pixel

#[test]
fn t21_full_map_on_junk_flea() {
    let m = FullMap::new(&junk_flea().corners.unwrap(), (0.0, 0.0));
    assert_eq!(m.pixel((1000.0, -500.0)), (696, 376));
}

#[test]
fn t22_full_map_on_showdown() {
    let m = FullMap::new(&showdown().corners.unwrap(), (-480.0, 1216.0));
    assert_eq!(m.pixel((0.0, 0.0)), (620, 293));
    assert_eq!(m.pixel((1000.0, -500.0)), (763, 364));
}

#[test]
fn t23_full_map_on_junk_flea_2() {
    let m = FullMap::new(&junk_flea2().corners.unwrap(), (0.0, 0.0));
    assert_eq!(m.pixel((1000.0, -500.0)), (751, 443));
}

#[test]
fn t24_full_map_on_warhead() {
    let m = FullMap::new(&warhead().corners.unwrap(), (1570.5, 640.0));
    assert_eq!(m.pixel((0.0, 0.0)), (697, 340));
}

// ---------------------------------------------------------------------------
// R3: zoom

#[test]
fn t25_holding_expand() {
    let mut z = maths::ZOOM_DEFAULT;
    for _ in 0..30 {
        z = maths::zoom_step(z, true, false);
    }
    assert!((z - 1.30).abs() < 1e-4, "{z}");
    assert_eq!(maths::knob_y(z), 88);
    for _ in 0..100 {
        z = maths::zoom_step(z, true, false);
    }
    assert_eq!(z, 2.0);
    assert_eq!(maths::knob_y(z), 18);
}

#[test]
fn t26_holding_decrease_clamps() {
    let mut z = maths::ZOOM_DEFAULT;
    for _ in 0..80 {
        z = maths::zoom_step(z, false, true);
    }
    assert_eq!(z, 0.5);
    assert_eq!(maths::knob_y(z), 168);
    assert_eq!(maths::knob_y(1.0), 118);
}

// ---------------------------------------------------------------------------
// R1: corners

#[test]
fn t42_a_mission_without_map_fields_has_no_image() {
    let doc = Document::parse(b"[Object0]\nTimeLimit = 300\n").unwrap();
    let m = MapImage::of_document(&doc, 0);
    assert_eq!(m.key(), None);
    assert_eq!(m.corners, None, "NaN corners are no corners");
    let c = maths::corners((0, 0), (0.0, 0.0), (0, 0), (0.0, 0.0), (0, 0));
    assert!(!c.is_finite());
}

#[test]
fn t48_to_51_the_corners_of_the_four_maps() {
    let check = |m: MapImage, lt: (f32, f32), rb: (f32, f32)| {
        let Corners { lt: l, rb: r } = m.corners.unwrap();
        assert!(near(l.0, lt.0, 0.01) && near(l.1, lt.1, 0.01), "{l:?} != {lt:?}");
        assert!(near(r.0, rb.0, 0.01) && near(r.1, rb.1, 0.01), "{r:?} != {rb:?}");
    };
    check(showdown(), (-2646.112, 2902.112), (2138.785, -1882.785));
    check(junk_flea2(), (-3414.446, 3414.446), (3421.121, -3421.121));
    check(warhead(), (-3073.335, 4097.335), (5129.346, -4105.346));
    check(junk_flea(), (-2091.557, 2091.557), (3376.897, -3376.897));
}

// ---------------------------------------------------------------------------
// F2: the legend

#[test]
fn t41_a_mode_without_a_list_gets_the_first() {
    let lists = legend::builtin();
    let l = legend::pick(&lists, 77).unwrap();
    assert_eq!(l.game_type, lists[0].game_type);
    assert_eq!(l.items.len(), 6, "Elimination's six rows");
    assert_eq!(l.items[0].text, "ALLY VIEW");
    assert_eq!(l.items[5].text, "BRAVO Respawn Area");
}

// ---------------------------------------------------------------------------
// F5: the spawn markers' icons

#[test]
fn t40_vip_escort_markers() {
    // Escort team Bravo: its marker is the defence one.
    assert_eq!(look::respawn(2, Some(2)), look::icon("area_respawn_DF.dtx"));
    assert_eq!(look::respawn(1, Some(2)), look::icon("area_respawn_OF.dtx"));
    assert_eq!(look::respawn(1, None), look::icon("area_respawn_A.dtx"));
    assert_eq!(look::respawn(2, None), look::icon("area_respawn_B.dtx"));
}

// ---------------------------------------------------------------------------
// R6: objectives' icons

#[test]
fn t37_38_bomb_site_icons() {
    assert_eq!(look::bomb_site(0, false), look::icon("area_bomb_A.dtx"));
    assert_eq!(look::bomb_site(1, false), look::icon("area_bomb_B.dtx"));
    assert_eq!(look::bomb_site(0, true), look::icon("area_bomb.dtx"));
}

#[test]
fn t43_spy_hunt_intel_carriers_see_each_other_as_spies() {
    let carrier = Subject {
        alive: true,
        carry: CaCarry { intel: 1, ..default() },
        ..default()
    };
    let holder = Viewer { team: 1, intel: true };
    let l = look::radar_player(&holder, &carrier).unwrap();
    assert_eq!(l.icon, look::icon("character_spy_pre.DTX"));
    assert_eq!(l.view, Some(look::icon("character_spy_view.dtx")));
    assert_eq!(
        look::map_player(&holder, &carrier).unwrap().icon,
        look::icon("character_spy_pre.DTX")
    );
    // Once the viewer drops its intel: hidden.
    assert_eq!(look::radar_player(&me(), &carrier), None);
    assert_eq!(look::map_player(&me(), &carrier), None);
}

#[test]
fn t44_flag_icons() {
    assert_eq!(look::flag_base(1, true), look::icon("area_flag_A.dtx"));
    assert_eq!(look::flag_base(1, false), look::icon("area_flagPoint_A.dtx"));
    assert_eq!(look::loose_flag(1), look::icon("character_flag_A.dtx"));
    assert_eq!(look::flag_base(2, true), look::icon("area_flag_B.dtx"));
}

// ---------------------------------------------------------------------------
// N: what the server sends at a round start

use super::mode_kit::{CREED, JSF, game, join, start, until_live};
use combat_arms::game::radar::{CaRadarProfile, CaRadarRound, RadarImageSource};

fn showdown_source(_world: &str, _mode: &str) -> Option<MapImage> {
    Some(showdown())
}

fn none_source(_world: &str, _mode: &str) -> Option<MapImage> {
    None
}

#[test]
fn t39_the_round_start_sends_the_image_and_the_team_means() {
    let mut sim = game("TDM", 2);
    sim.app.world_mut().insert_resource(RadarImageSource(showdown_source));
    // The map's start points of the mode's family: the means are of each
    // team's own (file units 0.018 m).
    let m = |x: f32, z: f32| Vec3::new(x * 0.018, 1.0, -z * 0.018);
    {
        let mut o = super::mode_kit::objects(&mut sim);
        o.starts = vec![
            start("Team Death Match", Some(JSF), m(10.0, 100.0)),
            start("Team Death Match", Some(JSF), m(30.0, 300.0)),
            start("Team Death Match", Some(CREED), m(-50.0, -20.0)),
        ];
    }
    join(&mut sim, JSF);
    join(&mut sim, CREED);
    until_live(&mut sim);
    let r = sim.app.world().resource::<CaRadarRound>().clone();
    assert_eq!(r.image, showdown(), "the mission's image and corners");
    let a = r.spawns[0].expect("Alpha's marker");
    assert!((a[0] - 20.0).abs() < 0.01 && (a[1] - 200.0).abs() < 0.01, "{a:?}");
    let b = r.spawns[1].expect("Bravo's marker");
    assert!((b[0] - -50.0).abs() < 0.01 && (b[1] - -20.0).abs() < 0.01, "{b:?}");
}

#[test]
fn a_team_without_start_points_has_no_marker() {
    let mut sim = game("TDM", 0);
    sim.app.world_mut().insert_resource(RadarImageSource(none_source));
    join(&mut sim, JSF);
    join(&mut sim, CREED);
    sim.ticks(80);
    let r = sim.app.world().resource::<CaRadarRound>().clone();
    assert_eq!(r.spawns, [None, None], "NaN on the server: not visibly placed");
    assert_eq!(r.image.key(), None, "and no mission data: no image");
}

#[test]
fn one_man_army_and_arms_race_send_no_markers() {
    for mode in ["FFA", "VWRM"] {
        let mut sim = game(mode, 2);
        sim.app.world_mut().insert_resource(RadarImageSource(showdown_source));
        join(&mut sim, JSF);
        join(&mut sim, CREED);
        sim.ticks(80);
        let r = sim.app.world().resource::<CaRadarRound>().clone();
        assert_eq!(r.spawns, [None, None], "{mode}");
        assert_eq!(r.image, showdown(), "{mode}: the image still comes");
    }
}

#[test]
fn the_round_data_and_scans_reach_the_clients() {
    use chimera_net::{harness::NetSim, net::memory::LinkConditions};
    use combat_arms::game::{TICK_INTERVAL, radar::CaSeen, rules::Phase};
    let link = LinkConditions {
        latency: std::time::Duration::from_millis(30),
        jitter: std::time::Duration::ZERO,
        loss: 0.0,
    };
    let mut sim = NetSim::new(combat_arms::harness::CaSim, link, 11, 2, |app| {
        super::net_modes::setup(app);
        app.insert_resource(RadarImageSource(showdown_source));
    });
    sim.set_frame(TICK_INTERVAL);
    sim.until_joined(600);
    let live = sim.until(400, |s| {
        super::net_modes::server_match(s).phase == Phase::Live && super::net_modes::view(s, 0).phase == Phase::Live
    });
    assert!(live, "live on both");
    // The round's image and markers on every client: the server's.
    let server = sim.server.app.world().resource::<CaRadarRound>().clone();
    assert_eq!(server.image, showdown());
    assert!(server.spawns.iter().all(|s| s.is_some()), "{:?}", server.spawns);
    let ok = sim.until(120, |s| {
        (0..2).all(|i| *s.clients[i].app.world().resource::<CaRadarRound>() == server)
    });
    assert!(ok, "the clients have the server's round data");
    // A scan marks a player for the other team on every machine (S2).
    sim.server
        .app
        .world_mut()
        .resource_mut::<combat_arms::game::radar::CaScans>()
        .system = true;
    let marked = |s: &mut NetSim, i: usize| {
        let w = s.clients[i].app.world_mut();
        w.query::<&CaSeen>().iter(w).filter(|c| c.scan == 0b11).count()
    };
    let ok = sim.until(120, |s| marked(s, 0) == 2 && marked(s, 1) == 2);
    assert!(
        ok,
        "the system scan marks both players on both clients: {} {}",
        marked(&mut sim, 0),
        marked(&mut sim, 1)
    );
}

#[test]
fn the_radar_options_are_saved_with_the_profile() {
    let sim = game("TDM", 1);
    let console = sim.app.world().resource::<chimera_core::console::Console>();
    for name in ["radarscale", "rotateradar"] {
        // The options' profile keeps them (tests/it/options.rs), not the
        // console's config.
        let c = console.cvar(name).unwrap_or_else(|| panic!("{name}"));
        assert!(!c.archive, "{name} is the profile's, not the config's");
    }
    assert_eq!(
        *sim.app.world().resource::<CaRadarProfile>(),
        CaRadarProfile { scale: 1.0, rotate: 0 }
    );
}

// --- Corrections 2026-10-22

/// Correction 1: in Arms Race a scan never reveals an enemy on the radar,
/// the last stage does, the full map is unaffected.
#[test]
fn corr1_arms_race_scans_do_not_reach_the_radar() {
    let scanned = |last| Subject {
        alive: true,
        arms_race: true,
        shared_scan: true,
        contact: true,
        team_contact: true,
        last_stage: last,
        ..default()
    };
    assert_eq!(look::radar_player(&me(), &scanned(false)), None);
    assert!(look::radar_player(&me(), &scanned(true)).is_some());
    assert!(
        look::map_player(&me(), &scanned(false)).is_some(),
        "the full map still shows a shared scan"
    );
    // Elimination: unchanged.
    let elim = Subject {
        arms_race: false,
        ..scanned(false)
    };
    assert!(look::radar_player(&me(), &elim).is_some());
}

/// Correction 4: a dead VIP ally shows the VIP icon, no wedge; a dead
/// plain ally the death icon; a dead enemy only while a rule reveals it.
#[test]
fn corr4_dead_players_on_the_full_map() {
    let vip = Subject {
        ally: true,
        alive: false,
        vip: true,
        ..default()
    };
    let l = look::map_player(&me(), &vip).unwrap();
    assert_eq!((l.icon, l.view), (look::icon("character_VIP.dtx"), None));
    let ally = Subject {
        ally: true,
        alive: false,
        ..default()
    };
    assert_eq!(
        look::map_player(&me(), &ally).unwrap().icon,
        look::icon("area_death_ally.DTX")
    );
    let hidden = Subject {
        alive: false,
        ..default()
    };
    assert_eq!(look::map_player(&me(), &hidden), None);
    let scanned = Subject {
        alive: false,
        shared_scan: true,
        ..default()
    };
    let l = look::map_player(&me(), &scanned).unwrap();
    assert_eq!((l.icon, l.view), (look::icon("character_enemy.dtx"), None));
    assert_eq!(
        look::radar_player(&me(), &scanned),
        None,
        "the radar hides every dead player"
    );
}

/// Correction 5 (the install has no misspelt key): an Alpha carrier, who
/// carries Bravo's flag, has an empty badge on the radar and Bravo's carrier
/// keeps hers; the full map shows both.
#[test]
fn corr5_alpha_carriers_have_no_badge_on_the_radar() {
    let alpha = Subject {
        alive: true,
        carry: CaCarry {
            flag: Some(2),
            ..default()
        },
        ..default()
    };
    let l = look::radar_player(&me(), &alpha).unwrap();
    assert_eq!(l.badge.as_deref(), Some(""));
    assert!(l.arrow);
    assert_eq!(
        look::map_player(&me(), &alpha).unwrap().badge,
        Some(look::icon("character_flag_B.dtx"))
    );
    let bravo = Subject {
        alive: true,
        carry: CaCarry {
            flag: Some(1),
            ..default()
        },
        ..default()
    };
    assert_eq!(
        look::radar_player(&me(), &bravo).unwrap().badge,
        Some(look::icon("character_flag_A.dtx"))
    );
    let ally = Subject { ally: true, ..alpha };
    assert_eq!(look::radar_player(&me(), &ally).unwrap().badge.as_deref(), Some(""));
}
