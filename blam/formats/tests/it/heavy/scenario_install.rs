//! The scenario, sky, item collection and object header readers on the
//! user's Halo disc image, against section (b) of specs/halo_ce/scenario.md
//! and its "Measured facts" tables: `bloodgulch` and `hangemhigh` in detail,
//! then every multiplayer map's counts, references and placements. Each
//! test passes with a note when there is no image.

// The spec's numbers are written as it states them, rounded.
#![allow(clippy::approx_constant, clippy::excessive_precision)]

use std::collections::BTreeMap;

use blam_formats::cache::{MapFile, TagRef, class};
use blam_formats::scenario::{
    GameType, ItemCollection, NetgameFlagType, ObjectHeader, ObjectType, Orientation, Scenario, ScenarioType, Sky,
};

use super::maps::{MULTIPLAYER, load};

fn near(a: f32, b: f32, tol: f32, what: &str) {
    assert!((a - b).abs() <= tol, "{what}: {a} vs {b}");
}

fn near3(a: [f32; 3], b: [f32; 3], tol: f32, what: &str) {
    for k in 0..3 {
        near(a[k], b[k], tol, what);
    }
}

fn path(map: &MapFile, r: &TagRef) -> String {
    map.tag(r.id)
        .unwrap_or_else(|| panic!("unresolved reference {r:?}"))
        .path
        .clone()
}

/// The reference resolves to a tag of `class` (its own class field says so
/// too).
fn resolves(map: &MapFile, r: &TagRef, c: blam_formats::cache::ClassCode, what: &str) -> String {
    let t = map
        .tag(r.id)
        .unwrap_or_else(|| panic!("{what}: {r:?} resolves to no tag"));
    assert!(t.is_a(c), "{what}: {} is {}, not {c}", t.path, t.class);
    t.path.clone()
}

#[test]
fn bloodgulch() {
    let Some(map) = load("bloodgulch") else { return };
    let s = Scenario::read(&map).unwrap();
    assert_eq!(s.scenario_type, ScenarioType::Multiplayer);

    // The sky.
    assert_eq!(s.skies.len(), 1);
    let sky_ref = &s.skies[0].sky;
    assert_eq!(sky_ref.class, class::SKY);
    let sky_tag = map.tag(sky_ref.id).unwrap();
    assert_eq!(sky_tag.path, "levels\\test\\bloodgulch\\bloodgulch");
    let sky = Sky::read(&map, sky_tag).unwrap();
    assert_eq!(path(&map, &sky.model), "sky\\mp clear afternoon\\mp clear afternoon");
    let f = &sky.outdoor_fog;
    near3(
        [f.color.r, f.color.g, f.color.b],
        [1.0, 0.902, 0.769],
        1e-3,
        "fog colour",
    );
    near(f.max_density, 0.4, 1e-6, "fog density");
    assert_eq!((f.start_distance, f.opaque_distance), (3.0, 100.0));
    assert_eq!(sky.lights.len(), 1);
    let l = &sky.lights[0];
    near(l.direction_yaw, 0.1745, 1e-4, "sun yaw");
    near(l.direction_pitch, 0.6981, 1e-4, "sun pitch");
    near3(
        [l.color.r, l.color.g, l.color.b],
        [0.906, 0.894, 0.796],
        1e-3,
        "sun colour",
    );
    near(l.power, 1.8, 1e-6, "sun power");
    assert_eq!(path(&map, &l.lens_flare), "sky\\clear afternoon\\sun");
    near3(l.direction(), [0.7544, 0.1330, 0.6428], 1e-4, "sun direction");
    // The rest of the sky's table row.
    near3(
        [
            sky.indoor_ambient_color.r,
            sky.indoor_ambient_color.g,
            sky.indoor_ambient_color.b,
        ],
        [0.239, 0.294, 0.373],
        1e-3,
        "indoor ambient",
    );
    assert_eq!((sky.indoor_ambient_power, sky.outdoor_ambient_power), (20.0, 0.2));
    assert_eq!(
        (
            sky.indoor_fog.max_density,
            sky.indoor_fog.start_distance,
            sky.indoor_fog.opaque_distance
        ),
        (1.0, 1.0, 120.0)
    );
    assert!(sky.animation_graph.is_null() && sky.indoor_fog_screen.is_null());

    // Player starts.
    assert_eq!(s.player_starts.len(), 71);
    let slot0 = |p: &blam_formats::scenario::PlayerStart| p.game_types.0[0];
    let count = |ty: i16, team: i16| {
        s.player_starts
            .iter()
            .filter(|p| slot0(p) == ty && p.team_index == team)
            .count()
    };
    assert_eq!((count(1, 0), count(1, 1)), (17, 21));
    assert_eq!(s.player_starts.iter().filter(|p| slot0(p) == 5).count(), 17);
    assert_eq!((count(14, 0), count(14, 1)), (8, 8));
    assert_eq!(s.eligible_starts(GameType::Race).len(), 17);
    assert_eq!(s.eligible_starts(GameType::Slayer).len(), 16);
    assert_eq!(s.eligible_starts(GameType::CaptureTheFlag).len(), 17 + 21);
    let p = &s.player_starts[0];
    near3(p.position, [98.4934, -157.6390, 1.7047], 1e-4, "first start");
    near(p.facing, 1.328281, 1e-6, "first start facing");
    assert_eq!((p.team_index, p.bsp_index, p.game_types.0), (0, 0, [1, 0, 0, 0]));
    near3(p.forward(), [0.2401, 0.9707, 0.0], 1e-4, "first start forward");

    // Netgame flags.
    assert_eq!(s.netgame_flags.len(), 191);
    let kinds = |k: NetgameFlagType| s.netgame_flags.iter().filter(|f| f.flag_type == k).count();
    assert_eq!(kinds(NetgameFlagType::CaptureTheFlag), 2);
    assert_eq!(kinds(NetgameFlagType::TeleporterSource), 2);
    assert_eq!(kinds(NetgameFlagType::TeleporterTarget), 2);
    assert_eq!(kinds(NetgameFlagType::OddballBallSpawn), 9);
    assert_eq!(kinds(NetgameFlagType::RaceTrack), 5);
    assert_eq!(kinds(NetgameFlagType::RaceVehicle), 16);
    assert_eq!(kinds(NetgameFlagType::Hill), 155);
    let f = &s.netgame_flags[0];
    assert_eq!((f.flag_type, f.team_index), (NetgameFlagType::CaptureTheFlag, 1));
    near3(f.position, [40.2406, -79.1232, -0.1], 1e-4, "first flag");
    assert_eq!(f.facing, 0.0);
    // The capture flags and the teleporter example.
    let ctf: Vec<_> = s
        .netgame_flags
        .iter()
        .filter(|f| f.flag_type == NetgameFlagType::CaptureTheFlag)
        .collect();
    let red = ctf.iter().find(|f| f.team_index == 0).unwrap();
    near3(red.position, [95.69, -159.45, -0.10], 1e-2, "red flag");
    let pairs = s.teleporter_pairs();
    assert_eq!(pairs.len(), 2);
    let team0 = pairs.iter().find(|p| p.pair == 0).unwrap();
    let (src, tgt) = (&s.netgame_flags[team0.source], &s.netgame_flags[team0.target.unwrap()]);
    near3(src.position, [95.58, -155.45, 1.81], 1e-2, "source");
    near3(tgt.position, [82.83, -114.53, 0.70], 1e-2, "target");
    near(src.facing, 1.5872, 1e-4, "source facing");
    near(tgt.facing, 3.0917, 1e-4, "target facing");
    let hills: std::collections::BTreeSet<i16> = s
        .netgame_flags
        .iter()
        .filter(|f| f.flag_type == NetgameFlagType::Hill)
        .map(|f| f.team_index)
        .collect();
    assert_eq!(hills.len(), 10);

    // Netgame equipment.
    assert_eq!(s.netgame_equipment.len(), 35);
    let e = &s.netgame_equipment[0];
    assert_eq!(
        path(&map, &e.item_collection),
        "item collections\\single weapons\\shotgun"
    );
    near3(e.position, [38.4336, -79.1941, -0.2860], 1e-4, "first equipment");
    assert_eq!((e.game_types.0, e.spawn_time, e.flags), ([12, 0, 0, 0], 0, 0));
    near(e.facing, 1.5384, 1e-4, "equipment facing");
    // Periods: shield-invisibility 180, rocket launcher entry 90 (own time),
    // sniper rifle 120, super shield 60.
    let period = |i: usize| {
        let e = &s.netgame_equipment[i];
        let col = ItemCollection::read(&map, map.tag(e.item_collection.id).unwrap()).unwrap();
        e.period_seconds(col.spawn_time)
    };
    assert_eq!(
        [period(8), period(21), period(22), period(24), period(25), period(0)],
        [180, 90, 120, 60, 180, 30]
    );

    // Starting equipment: (CTF, flags 0, assault rifle, pistol) and
    // (all but CTF, flag 1, plasma pistol).
    assert_eq!(s.starting_equipment.len(), 2);
    let names = |i: usize| -> Vec<String> {
        s.starting_equipment[i]
            .item_collections
            .0
            .iter()
            .filter(|r| !r.is_null())
            .map(|r| path(&map, r))
            .collect()
    };
    assert_eq!(s.starting_equipment[0].game_types.0, [1, 0, 0, 0]);
    assert_eq!(s.starting_equipment[0].flags, 0);
    assert_eq!(
        names(0),
        [
            "item collections\\single weapons\\assault rifle",
            "item collections\\single weapons\\pistol"
        ]
    );
    assert_eq!(s.starting_equipment[1].game_types.0, [13, 0, 0, 0]);
    assert_eq!(s.starting_equipment[1].flags, 1);
    assert_eq!(names(1), ["item collections\\single weapons\\plasma pistol"]);

    // Vehicles.
    assert_eq!(s.vehicles.len(), 8);
    let mut by_name: BTreeMap<String, usize> = BTreeMap::new();
    for v in &s.vehicles {
        let entry = &s.vehicle_palette[v.header.palette_index as usize];
        *by_name.entry(path(&map, &entry.object)).or_default() += 1;
    }
    assert_eq!(
        by_name.into_iter().collect::<Vec<_>>(),
        [
            ("vehicles\\ghost\\ghost_mp".to_string(), 4),
            ("vehicles\\scorpion\\scorpion_mp".to_string(), 2),
            ("vehicles\\warthog\\warthog".to_string(), 2)
        ]
    );
    let v = &s.vehicles[0].header;
    near3(v.position, [29.137, -90.834, 0.2864], 1e-3, "first vehicle");
    near(v.rotation.yaw, -0.8483, 1e-4, "first vehicle yaw");
    assert_eq!((v.flags, v.name_index, v.variant), (0, -1, 0));

    // Scenery.
    assert_eq!(s.scenery.len(), 50);
    assert_eq!(s.scenery_palette.len(), 19);
    let first = &s.scenery[0].header;
    assert!(path(&map, &s.scenery_palette[first.palette_index as usize].object).contains("boulder"));
    near3(first.position, [37.9339, -88.6921, 0.0876], 1e-4, "first scenery");
    let r = first.rotation;
    near3(
        [r.yaw, r.pitch, r.roll],
        [2.3610, 0.1120, 0.1017],
        1e-4,
        "first scenery rotation",
    );
    assert_eq!(first.on_bsp_flags, 1);
    let used: std::collections::BTreeSet<i16> = s.scenery.iter().map(|p| p.header.palette_index).collect();
    assert_eq!(used.len(), 15);
    // The five (here four) buried placements: numbers 19 to 21 and 24.
    let buried: Vec<usize> = (0..s.scenery.len())
        .filter(|&i| !s.scenery[i].header.in_bsp(0))
        .collect();
    assert_eq!(buried, [19, 20, 21, 24]);
    let tilted = &s.scenery[24].header;
    assert!(path(&map, &s.scenery_palette[tilted.palette_index as usize].object).contains("teleporter_base"));

    // The rest.
    assert_eq!(
        (
            s.machines.len(),
            s.light_fixtures.len(),
            s.sound_scenery.len(),
            s.decals.len()
        ),
        (0, 0, 2, 8)
    );
    assert_eq!(Scenario::bsps(&map).unwrap().len(), 1);
    assert_eq!(s.structure_bsp_count.0, 1);
    assert_eq!(
        path(&map, &s.sound_scenery_palette[0].object),
        "sound\\sfx\\ui\\teleporter_loop"
    );

    // Positions inside the BSP's world bounds (the spec's numbers).
    let (lo, hi) = ([5.91, -190.22, -0.35], [131.90, -45.06, 26.23]);
    let inside = |p: [f32; 3], what: &str| {
        for k in 0..3 {
            assert!(
                p[k] >= lo[k] - 0.01 && p[k] <= hi[k] + 0.01,
                "{what} {p:?} outside the bounds"
            );
        }
    };
    s.player_starts.iter().for_each(|p| inside(p.position, "start"));
    s.netgame_equipment.iter().for_each(|e| inside(e.position, "equipment"));
    s.netgame_flags.iter().for_each(|f| inside(f.position, "flag"));
}

#[test]
fn hangemhigh() {
    let Some(map) = load("hangemhigh") else { return };
    let s = Scenario::read(&map).unwrap();
    assert_eq!(s.player_starts.len(), 67);
    let slot0 = |p: &blam_formats::scenario::PlayerStart| p.game_types.0[0];
    let count = |ty: i16, team: i16| {
        s.player_starts
            .iter()
            .filter(|p| slot0(p) == ty && p.team_index == team)
            .count()
    };
    assert_eq!((count(1, 0), count(1, 1)), (19, 20));
    assert_eq!((count(13, 0), count(13, 1)), (10, 18));
    let p = &s.player_starts[0];
    near3(p.position, [27.9078, -15.5764, -2.7938], 1e-4, "first start");
    near(p.facing, 1.590419, 1e-6, "first start facing");
    assert_eq!((p.team_index, p.game_types.0), (0, [13, 0, 0, 0]));

    assert_eq!(s.netgame_flags.len(), 92);
    let kinds = |k: NetgameFlagType| s.netgame_flags.iter().filter(|f| f.flag_type == k).count();
    assert_eq!(kinds(NetgameFlagType::CaptureTheFlag), 2);
    assert_eq!(kinds(NetgameFlagType::OddballBallSpawn), 5);
    assert_eq!(kinds(NetgameFlagType::RaceTrack), 11);
    assert_eq!(kinds(NetgameFlagType::Hill), 74);
    assert_eq!(
        kinds(NetgameFlagType::TeleporterSource) + kinds(NetgameFlagType::TeleporterTarget),
        0
    );
    let hills: std::collections::BTreeSet<i16> = s
        .netgame_flags
        .iter()
        .filter(|f| f.flag_type == NetgameFlagType::Hill)
        .map(|f| f.team_index)
        .collect();
    assert_eq!(hills.len(), 14);
    let f = &s.netgame_flags[0];
    assert_eq!((f.flag_type, f.team_index), (NetgameFlagType::RaceTrack, 0));
    near3(f.position, [20.2444, 12.5342, -7.9494], 1e-4, "first flag");
    assert_eq!(f.facing, 0.0);

    assert_eq!(s.netgame_equipment.len(), 45);
    let e = &s.netgame_equipment[0];
    assert_eq!(
        path(&map, &e.item_collection),
        "item collections\\single weapons\\assault rifle"
    );
    near3(e.position, [27.9907, -2.5534, -7.9494], 1e-4, "first equipment");
    assert_eq!((e.game_types.0, e.spawn_time, e.flags), ([12, 0, 0, 0], 0, 0));
    near(e.facing, 1.5536, 1e-4, "equipment facing");
    assert!(s.netgame_equipment[8].levitate(), "the invisibility powerup levitates");

    // Starting equipment, as the spec lists the three entries.
    assert_eq!(s.starting_equipment.len(), 3);
    let se: Vec<_> = s
        .starting_equipment
        .iter()
        .map(|e| {
            let n: Vec<String> = e
                .item_collections
                .0
                .iter()
                .filter(|r| !r.is_null())
                .map(|r| path(&map, r).rsplit('\\').next().unwrap().to_string())
                .collect();
            (e.game_types.0[0], e.flags, n)
        })
        .collect();
    assert_eq!(
        se,
        [
            (14, 1, vec!["pistol".to_string()]),
            (1, 2, vec!["plasma rifle".to_string()]),
            (5, 1, vec!["plasma pistol".to_string()])
        ]
    );

    assert_eq!(s.scenery.len(), 8);
    assert_eq!(s.scenery_palette.len(), 2);
    assert!(s.vehicles.is_empty());
    assert_eq!(s.decals.len(), 2);
    let first = &s.scenery[0].header;
    assert_eq!(
        path(&map, &s.scenery_palette[first.palette_index as usize].object),
        "scenery\\floor_arrow\\floor_arrow"
    );
    near3(first.position, [31.9421, -12.9793, -3.8979], 1e-4, "first scenery");
    near(first.rotation.yaw, -1.5708, 1e-4, "first scenery yaw");
    assert_eq!(first.on_bsp_flags, 1);
    let sky = Sky::read(&map, map.tag(s.skies[0].sky.id).unwrap()).unwrap();
    assert!(!sky.model.is_null());
    assert_eq!(path(&map, &s.skies[0].sky), "levels\\test\\hangemhigh\\hangemhigh");
    assert_eq!(Scenario::bsps(&map).unwrap().len(), 1);

    let (lo, hi) = ([5.90, -22.82, -9.25], [35.88, 15.82, 8.18]);
    let inside = |p: [f32; 3], what: &str| {
        for k in 0..3 {
            assert!(
                p[k] >= lo[k] - 0.01 && p[k] <= hi[k] + 0.01,
                "{what} {p:?} outside the bounds"
            );
        }
    };
    s.player_starts.iter().for_each(|p| inside(p.position, "start"));
    s.netgame_equipment.iter().for_each(|e| inside(e.position, "equipment"));
}

/// One map's row of the spec's tables.
struct Row {
    name: &'static str,
    starts: usize,
    ctf: (usize, usize),
    /// Race starts (game type 5).
    race: usize,
    /// Wildcard starts: (slot value, team, count).
    wildcard: &'static [(i16, i16, usize)],
    /// CTF, teleporter sources, targets, oddball, race track, race vehicle,
    /// hill flags, total.
    flags: [usize; 8],
    equipment: usize,
    vehicles: usize,
    scenery: (usize, usize, usize),
    machines: usize,
    light_fixtures: usize,
    sound_scenery: usize,
    decals: usize,
    sky: Option<&'static str>,
    /// "Entries by collection".
    collections: &'static str,
}

const ROWS: [Row; 13] = [
    Row {
        name: "beavercreek",
        starts: 70,
        ctf: (26, 22),
        race: 0,
        wildcard: &[(12, 0, 4), (12, 1, 8), (13, 0, 6), (13, 1, 4)],
        flags: [2, 2, 2, 8, 11, 0, 72, 97],
        equipment: 35,
        vehicles: 0,
        scenery: (30, 7, 9),
        machines: 0,
        light_fixtures: 0,
        sound_scenery: 8,
        decals: 8,
        sky: Some("levels\\test\\beavercreek\\beavercreek sky"),
        collections: "assault rifle 2, frag grenades 8, needler 3, plasma grenades 8, plasma rifle 2, powerup health pack 4, powerup invisibility 3, powerup super shield 2, rocket launcher 1, sniper rifle 2",
    },
    Row {
        name: "bloodgulch",
        starts: 71,
        ctf: (17, 21),
        race: 17,
        wildcard: &[(14, 0, 8), (14, 1, 8)],
        flags: [2, 2, 2, 9, 5, 16, 155, 191],
        equipment: 35,
        vehicles: 8,
        scenery: (50, 15, 19),
        machines: 0,
        light_fixtures: 0,
        sound_scenery: 2,
        decals: 8,
        sky: Some("levels\\test\\bloodgulch\\bloodgulch"),
        collections: "assault rifle 2, frag grenades 8, pistol 4, plasma grenades 8, plasma rifle 2, powerup health pack 2, powerup super shield 1, rocket launcher 1, shield-invisibility 1, shotgun 4, sniper rifle 2",
    },
    Row {
        name: "boardingaction",
        starts: 73,
        ctf: (27 + 2, 25 + 3),
        race: 0,
        wildcard: &[(13, 0, 16)],
        flags: [2, 8, 8, 10, 13, 0, 36, 77],
        equipment: 52,
        vehicles: 0,
        scenery: (29, 6, 7),
        machines: 0,
        light_fixtures: 0,
        sound_scenery: 0,
        decals: 55,
        sky: Some("levels\\test\\boardingaction\\boardingaction space"),
        collections: "assault rifle 2, frag grenades 8, needler 4, plasma grenades 16, plasma rifle 3, powerup health pack 7, powerup invisibility 2, powerup super shield 2, rocket launcher 4, shotgun 2, sniper rifle 2",
    },
    Row {
        name: "carousel",
        starts: 63,
        ctf: (18, 18),
        race: 0,
        wildcard: &[(13, 0, 27)],
        flags: [2, 4, 4, 5, 10, 0, 54, 79],
        equipment: 37,
        vehicles: 0,
        scenery: (40, 5, 5),
        machines: 0,
        light_fixtures: 13,
        sound_scenery: 0,
        decals: 0,
        sky: Some("levels\\test\\carousel\\carousel"),
        collections: "assault rifle 1, flame thrower 1, frag grenades 8, needler 2, pistol 4, plasma grenades 8, powerup health pack 1, powerup invisibility 3, powerup super shield 3, rocket launcher 1, shotgun 2, sniper rifle 3",
    },
    Row {
        name: "chillout",
        starts: 58,
        ctf: (16, 17),
        race: 0,
        wildcard: &[(13, 0, 25)],
        flags: [2, 3, 3, 8, 12, 0, 21, 49],
        equipment: 33,
        vehicles: 0,
        scenery: (8, 3, 4),
        machines: 0,
        light_fixtures: 0,
        sound_scenery: 0,
        decals: 0,
        sky: Some("levels\\test\\chillout\\chillout sky"),
        collections: "assault rifle 1, frag grenades 8, needler 2, pistol 2, plasma grenades 8, plasma rifle 2, powerup health pack 4, powerup invisibility 2, powerup super shield 1, rocket launcher 1, shotgun 1, sniper rifle 1",
    },
    Row {
        name: "damnation",
        starts: 67,
        ctf: (19, 18),
        race: 0,
        wildcard: &[(13, 1, 30)],
        flags: [2, 2, 2, 8, 4, 0, 61, 79],
        equipment: 46,
        vehicles: 0,
        scenery: (12, 5, 6),
        machines: 0,
        light_fixtures: 28,
        sound_scenery: 3,
        decals: 0,
        sky: Some("sky\\mp_dusk\\mp_dusk"),
        collections: "assault rifle 4, frag grenades 16, pistol 1, plasma grenades 8, plasma pistol 1, plasma rifle 3, powerup health pack 4, powerup invisibility 1, powerup super shield 1, rocket launcher 1, shotgun 4, sniper rifle 2",
    },
    Row {
        name: "hangemhigh",
        starts: 67,
        ctf: (19, 20),
        race: 0,
        wildcard: &[(13, 0, 10), (13, 1, 18)],
        flags: [2, 0, 0, 5, 11, 0, 74, 92],
        equipment: 45,
        vehicles: 0,
        scenery: (8, 2, 2),
        machines: 0,
        light_fixtures: 0,
        sound_scenery: 0,
        decals: 2,
        sky: Some("levels\\test\\hangemhigh\\hangemhigh"),
        collections: "assault rifle 2, frag grenades 16, needler 3, pistol 2, plasma grenades 8, plasma rifle 2, powerup health pack 2, powerup invisibility 1, rocket launcher 1, shield-invisibility 1, shotgun 4, sniper rifle 3",
    },
    Row {
        name: "longest",
        starts: 71,
        ctf: (20, 23),
        race: 0,
        wildcard: &[(13, 0, 28)],
        flags: [2, 0, 0, 8, 5, 0, 36, 51],
        equipment: 40,
        vehicles: 0,
        scenery: (4, 2, 2),
        machines: 0,
        light_fixtures: 0,
        sound_scenery: 0,
        decals: 2,
        sky: Some("sky\\sky_sidewinder\\sky_sidewinder"),
        collections: "assault rifle 5, frag grenades 16, pistol 2, plasma grenades 8, plasma rifle 1, powerup health pack 4, powerup invisibility 1, powerup super shield 1, shotgun 2",
    },
    Row {
        name: "prisoner",
        starts: 58,
        ctf: (16, 18),
        race: 0,
        wildcard: &[(13, 0, 21), (13, 1, 1), (13, 2, 1), (13, 3, 1)],
        flags: [2, 0, 0, 2, 9, 0, 34, 47],
        equipment: 51,
        vehicles: 0,
        scenery: (2, 1, 1),
        machines: 0,
        light_fixtures: 0,
        sound_scenery: 0,
        decals: 0,
        sky: Some("levels\\test\\prisoner\\prisoner sky"),
        collections: "assault rifle 2, frag grenades 12, plasma grenades 16, plasma rifle 3, powerup health pack 5, powerup invisibility 1, powerup super shield 3, rocket launcher 1, shotgun 4, sniper rifle 4",
    },
    Row {
        name: "putput",
        starts: 59,
        ctf: (16, 16),
        race: 0,
        wildcard: &[(13, 0, 15), (13, 1, 12)],
        flags: [2, 30, 30, 5, 9, 0, 58, 134],
        equipment: 40,
        vehicles: 0,
        scenery: (32, 2, 2),
        machines: 0,
        light_fixtures: 0,
        sound_scenery: 0,
        decals: 7,
        sky: None,
        collections: "assault rifle 1, frag grenades 8, group all powerups 1, pistol 2, plasma grenades 8, plasma pistol 4, plasma rifle 1, powerup health pack 13, rocket launcher 1, shotgun 1",
    },
    Row {
        name: "ratrace",
        starts: 48,
        ctf: (0, 0),
        race: 0,
        wildcard: &[(12, 0, 14), (12, 1, 17), (13, 0, 5), (13, 1, 8), (13, 3, 4)],
        flags: [2, 4, 4, 7, 4, 0, 22, 43],
        equipment: 30,
        vehicles: 0,
        scenery: (8, 3, 3),
        machines: 0,
        light_fixtures: 5,
        sound_scenery: 0,
        decals: 0,
        sky: None,
        collections: "assault rifle 2, frag grenades 8, pistol 4, plasma grenades 8, plasma rifle 1, powerup health pack 2, powerup invisibility 1, powerup super shield 1, shotgun 3",
    },
    Row {
        name: "sidewinder",
        starts: 76,
        ctf: (21, 23),
        race: 16,
        wildcard: &[(14, 0, 16)],
        flags: [2, 4, 4, 8, 5, 16, 127, 166],
        equipment: 47,
        vehicles: 8,
        scenery: (35, 10, 12),
        machines: 0,
        light_fixtures: 0,
        sound_scenery: 0,
        decals: 24,
        sky: Some("sky\\sky_sidewinder\\sky_sidewinder"),
        collections: "assault rifle 2, frag grenades 16, needler 4, pistol 2, plasma grenades 8, plasma rifle 2, powerup health pack 2, powerup invisibility 1, powerup super shield 2, rocket launcher 2, shotgun 4, sniper rifle 2",
    },
    Row {
        name: "wizard",
        starts: 48,
        ctf: (16, 16),
        race: 0,
        wildcard: &[(13, 0, 1), (13, 1, 1), (13, 2, 1), (13, 3, 13)],
        flags: [2, 4, 4, 5, 4, 0, 40, 59],
        equipment: 44,
        vehicles: 0,
        scenery: (6, 2, 2),
        machines: 4,
        light_fixtures: 0,
        sound_scenery: 0,
        decals: 0,
        sky: Some("levels\\test\\wizard\\wizard sky"),
        collections: "assault rifle 3, frag grenades 8, needler 2, pistol 6, plasma grenades 8, plasma rifle 3, powerup health pack 4, powerup invisibility 2, powerup super shield 2, rocket launcher 2, shield-invisibility 2, shotgun 2",
    },
];

fn parse_collections(s: &str) -> BTreeMap<String, usize> {
    s.split(", ")
        .map(|e| {
            let (name, n) = e.rsplit_once(' ').unwrap();
            (name.to_string(), n.parse().unwrap())
        })
        .collect()
}

#[test]
fn every_multiplayer_map_matches_the_measured_facts() {
    assert_eq!(ROWS.map(|r| r.name), MULTIPLAYER);
    let mut distinct_collections = std::collections::BTreeSet::new();
    let mut buried_scenery = 0;
    for row in &ROWS {
        let Some(map) = load(row.name) else { return };
        let s = Scenario::read(&map).unwrap_or_else(|e| panic!("{}: {e}", row.name));
        let n = row.name;
        assert_eq!(s.scenario_type, ScenarioType::Multiplayer, "{n}");

        // Player starts: totals, CTF by team, race, wildcards; the rest of
        // the checks of the spec.
        assert_eq!(s.player_starts.len(), row.starts, "{n}: starts");
        let slot0 = |p: &blam_formats::scenario::PlayerStart| p.game_types.0[0];
        let ctf = |t: i16| {
            s.player_starts
                .iter()
                .filter(|p| slot0(p) == 1 && p.team_index == t)
                .count()
        };
        assert_eq!(
            (ctf(0), ctf(1)),
            (
                if n == "boardingaction" { 27 } else { row.ctf.0 },
                if n == "boardingaction" { 25 } else { row.ctf.1 }
            ),
            "{n}: ctf"
        );
        let ctf_all = s.player_starts.iter().filter(|p| slot0(p) == 1).count();
        assert_eq!(ctf_all, row.ctf.0 + row.ctf.1, "{n}: all ctf starts");
        assert_eq!(
            s.player_starts.iter().filter(|p| slot0(p) == 5).count(),
            row.race,
            "{n}: race"
        );
        let mut wild: BTreeMap<(i16, i16), usize> = BTreeMap::new();
        for p in &s.player_starts {
            if (12..=14).contains(&slot0(p)) {
                *wild.entry((slot0(p), p.team_index)).or_default() += 1;
            }
        }
        let want: BTreeMap<_, _> = row.wildcard.iter().map(|&(t, team, k)| ((t, team), k)).collect();
        assert_eq!(wild, want, "{n}: wildcard starts");
        let others = s
            .player_starts
            .iter()
            .filter(|p| ![1, 5, 12, 13, 14].contains(&slot0(p)))
            .count();
        assert_eq!(others, 0, "{n}: starts of other game types");
        for p in &s.player_starts {
            assert_eq!(&p.game_types.0[1..], [0, 0, 0], "{n}: only the first slot is used");
            assert_eq!(p.bsp_index, 0, "{n}");
            assert!((0..=3).contains(&p.team_index), "{n}");
            assert!(
                p.facing.abs() <= std::f32::consts::PI + 1e-6,
                "{n}: heading {}",
                p.facing
            );
        }

        // Netgame flags.
        let k = |t: NetgameFlagType| s.netgame_flags.iter().filter(|f| f.flag_type == t).count();
        assert_eq!(
            [
                k(NetgameFlagType::CaptureTheFlag),
                k(NetgameFlagType::TeleporterSource),
                k(NetgameFlagType::TeleporterTarget),
                k(NetgameFlagType::OddballBallSpawn),
                k(NetgameFlagType::RaceTrack),
                k(NetgameFlagType::RaceVehicle),
                k(NetgameFlagType::Hill),
                s.netgame_flags.len(),
            ],
            row.flags,
            "{n}: netgame flags"
        );
        assert_eq!(
            k(NetgameFlagType::CaptureTheFlagVehicle) + k(NetgameFlagType::Bank),
            0,
            "{n}"
        );
        for f in &s.netgame_flags {
            assert!(f.reference.is_null(), "{n}: flag references are null");
        }
        // Every capture flag team once; every teleporter source has one target.
        let mut teams: Vec<i16> = s
            .netgame_flags
            .iter()
            .filter(|f| f.flag_type == NetgameFlagType::CaptureTheFlag)
            .map(|f| f.team_index)
            .collect();
        teams.sort();
        assert_eq!(teams, [0, 1], "{n}");
        for pair in s.teleporter_pairs() {
            let targets = s
                .netgame_flags
                .iter()
                .filter(|f| f.flag_type == NetgameFlagType::TeleporterTarget && f.team_index == pair.pair)
                .count();
            assert_eq!(targets, 1, "{n}: pair {}", pair.pair);
            assert!(pair.target.is_some());
        }

        // Netgame equipment and its collections.
        assert_eq!(s.netgame_equipment.len(), row.equipment, "{n}: equipment");
        let mut by_collection: BTreeMap<String, usize> = BTreeMap::new();
        for e in &s.netgame_equipment {
            let p = resolves(&map, &e.item_collection, class::ITEM_COLLECTION, n);
            assert!(p.starts_with("item collections\\"), "{n}: {p}");
            distinct_collections.insert(p.clone());
            *by_collection
                .entry(p.rsplit('\\').next().unwrap().to_string())
                .or_default() += 1;
            assert_eq!(e.team_index, 0, "{n}");
            assert!((0..=180).contains(&e.spawn_time), "{n}");
        }
        assert_eq!(
            by_collection,
            parse_collections(row.collections),
            "{n}: entries by collection"
        );
        for sc in &s.starting_equipment {
            for r in sc.item_collections.0.iter().filter(|r| !r.is_null()) {
                resolves(&map, r, class::ITEM_COLLECTION, n);
            }
        }

        // Object counts and palettes.
        assert_eq!(s.vehicles.len(), row.vehicles, "{n}");
        assert_eq!(s.scenery.len(), row.scenery.0, "{n}: scenery");
        assert_eq!(s.scenery_palette.len(), row.scenery.2, "{n}: scenery palette");
        let used: std::collections::BTreeSet<i16> = s.scenery.iter().map(|p| p.header.palette_index).collect();
        assert_eq!(used.len(), row.scenery.1, "{n}: scenery tags used");
        assert_eq!(s.machines.len(), row.machines, "{n}");
        assert_eq!(s.light_fixtures.len(), row.light_fixtures, "{n}");
        assert_eq!(s.sound_scenery.len(), row.sound_scenery, "{n}");
        assert_eq!(s.decals.len(), row.decals, "{n}");
        assert!(
            s.bipeds.is_empty() && s.weapons.is_empty() && s.equipment.is_empty() && s.controls.is_empty(),
            "{n}"
        );
        assert!(
            s.device_groups.is_empty() && s.trigger_volumes.is_empty() && s.starting_profiles.is_empty(),
            "{n}"
        );
        assert_eq!(
            (s.scripts.0, s.ai_encounters.0, s.detail_object_collection_palette.0),
            (0, 0, 0),
            "{n}"
        );
        assert_eq!(Scenario::bsps(&map).unwrap().len(), 1, "{n}");
        assert_eq!(s.structure_bsp_count.0, 1, "{n}");

        macro_rules! placements {
            ($placements:expr, $palette:expr, $class:expr, $what:expr) => {
                for p in &$placements {
                    let h = &p.header;
                    assert!(
                        h.palette_entry($palette.len()).is_some(),
                        "{n}: {} placement palette index {} of {}",
                        $what,
                        h.palette_index,
                        $palette.len()
                    );
                    assert_eq!(h.name_index, -1, "{n}");
                    assert_eq!((h.flags, h.variant), (0, 0), "{n}");
                    assert!(h.position.iter().all(|v| v.is_finite()), "{n}");
                }
                for e in &$palette {
                    assert_eq!(e.object.class, $class, "{n}: {} palette entry class", $what);
                    resolves(&map, &e.object, $class, &format!("{n}: {} palette", $what));
                }
            };
        }
        placements!(s.scenery, s.scenery_palette, class::SCENERY, "scenery");
        placements!(s.vehicles, s.vehicle_palette, class::VEHICLE, "vehicle");
        placements!(s.machines, s.machine_palette, class::DEVICE_MACHINE, "machine");
        placements!(
            s.light_fixtures,
            s.light_fixture_palette,
            class::DEVICE_LIGHT_FIXTURE,
            "light fixture"
        );
        placements!(
            s.sound_scenery,
            s.sound_scenery_palette,
            class::SOUND_SCENERY,
            "sound scenery"
        );
        for e in s.biped_palette.iter().chain(&s.weapon_palette) {
            resolves(&map, &e.object, e.object.class, n);
        }
        for p in &s.light_fixtures {
            assert!(p.header.in_bsp(0), "{n}: light fixtures sit in BSP 0");
        }
        buried_scenery += s.scenery.iter().filter(|p| !p.header.in_bsp(0)).count();

        // The palette's scenery tags read as object headers.
        for e in &s.scenery_palette {
            let t = map.tag(e.object.id).unwrap();
            let h = ObjectHeader::read(&map, t).unwrap();
            assert_eq!(h.object_type, ObjectType::Scenery, "{n}: {}", t.path);
            assert!(h.bounding_radius.is_finite() && h.bounding_radius >= 0.0, "{n}");
            assert_eq!(h.origin_offset, [0.0; 3], "{n}: {}", t.path);
            near(h.render_bounding_radius, h.bounding_radius, 1e-5, &t.path);
            if !h.model.is_null() {
                resolves(&map, &h.model, class::MODEL, &t.path);
            }
            if !h.collision_model.is_null() {
                resolves(&map, &h.collision_model, class::MODEL_COLLISION_GEOMETRY, &t.path);
            }
            if !h.animation_graph.is_null() {
                resolves(&map, &h.animation_graph, class::MODEL_ANIMATIONS, &t.path);
            }
        }

        // The sky.
        match row.sky {
            None => assert!(s.skies.is_empty(), "{n}"),
            Some(sky_path) => {
                assert_eq!(s.skies.len(), 1, "{n}");
                assert_eq!(s.skies[0].sky.class, class::SKY, "{n}");
                assert_eq!(resolves(&map, &s.skies[0].sky, class::SKY, n), sky_path, "{n}");
                let sky = Sky::read(&map, map.tag(s.skies[0].sky.id).unwrap()).unwrap();
                if !sky.model.is_null() {
                    resolves(&map, &sky.model, class::MODEL, n);
                } else {
                    assert_eq!(n, "chillout");
                }
                assert!(sky.animation_graph.is_null() && sky.indoor_fog_screen.is_null(), "{n}");
                assert_eq!((sky.shader_functions.0, sky.animations.len()), (0, 0), "{n}");
                assert_eq!(sky.lights.len(), if n == "boardingaction" { 2 } else { 1 }, "{n}");
                for l in &sky.lights {
                    if !l.lens_flare.is_null() {
                        resolves(&map, &l.lens_flare, class::LENS_FLARE, n);
                    }
                    assert_eq!(l.marker_name, "", "{n}");
                    assert!(l.direction().iter().all(|v| v.abs() <= 1.0 + 1e-6));
                }
            }
        }
        // Every item collection reads, with items that resolve to a weapon
        // or equipment tag.
        for t in map.of_class(class::ITEM_COLLECTION) {
            let col = ItemCollection::read(&map, t).unwrap();
            assert!(!col.permutations.is_empty(), "{n}: {}", t.path);
            for p in &col.permutations {
                let item = map
                    .tag(p.item.id)
                    .unwrap_or_else(|| panic!("{n}: {} item unresolved", t.path));
                assert!(item.is_a(class::ITEM), "{n}: {} holds {}", t.path, item.class);
                assert!(p.weight > 0.0);
            }
        }
    }
    assert_eq!(distinct_collections.len(), 16, "{distinct_collections:?}");
    assert_eq!(
        buried_scenery, 5,
        "the five scenery placements with the on-BSP bit clear"
    );
}

#[test]
fn item_collections_of_bloodgulch() {
    let Some(map) = load("bloodgulch") else { return };
    let s = Scenario::read(&map).unwrap();
    let mut seen: BTreeMap<String, (usize, i16, f32)> = BTreeMap::new();
    for e in &s.netgame_equipment {
        let t = map.tag(e.item_collection.id).unwrap();
        let col = ItemCollection::read(&map, t).unwrap();
        seen.insert(
            t.path.rsplit('\\').next().unwrap().to_string(),
            (
                col.permutations.len(),
                col.spawn_time,
                col.permutations.iter().map(|p| p.weight).sum(),
            ),
        );
    }
    for (name, spawn) in [
        ("plasma grenades", 180),
        ("rocket launcher", 120),
        ("powerup super shield", 60),
        ("shield-invisibility", 180),
        ("shotgun", 0),
        ("frag grenades", 0),
    ] {
        assert_eq!(seen[name].1, spawn, "{name} spawn time");
    }
    assert_eq!(seen["shield-invisibility"].0, 2);
    assert_eq!(seen["shield-invisibility"].2, 100.0);
    assert_eq!(seen["shotgun"], (1, 0, 100.0));
    // Drawing from the two-way powerup: r < 50 is the first permutation.
    let t = map
        .find(
            class::ITEM_COLLECTION,
            "item collections\\powerups\\shield-invisibility",
        )
        .unwrap();
    let col = ItemCollection::read(&map, t).unwrap();
    assert_eq!(col.total_weight(), 100);
    assert!(std::ptr::eq(col.pick(49).unwrap(), &col.permutations[0]));
    assert!(std::ptr::eq(col.pick(50).unwrap(), &col.permutations[1]));
    let items: Vec<String> = col.permutations.iter().map(|p| path(&map, &p.item)).collect();
    assert!(
        items.iter().any(|p| p.contains("camouflage")) && items.iter().any(|p| p.contains("over shield")),
        "{items:?}"
    );
}

#[test]
fn wizard_alert_lights_orient_into_their_corners() {
    let Some(map) = load("wizard") else { return };
    let s = Scenario::read(&map).unwrap();
    assert_eq!(s.machines.len(), 4);
    assert_eq!(s.machine_palette.len(), 2);
    let mut found_corner = false;
    for m in &s.machines {
        let h = &m.header;
        assert_eq!(h.palette_index, 0);
        assert_eq!(
            path(&map, &s.machine_palette[0].object),
            "levels\\test\\wizard\\devices\\wiz_alert_light\\wiz_alert_light"
        );
        assert_eq!((m.power_group, m.position_group, m.device_flags), (-1, -1, 0));
        let o = Orientation::from_rotation(&h.rotation);
        // Each light's up axis points into its own corner of the room.
        assert!(o.up[0] * h.position[0] > 0.0 && o.up[1] * h.position[1] > 0.0, "{h:?}");
        assert!(o.up[2].abs() < 0.2);
        if (h.position[0] + 9.66).abs() < 0.05 && (h.position[1] - 9.77).abs() < 0.05 {
            near3(o.up, [-0.69, 0.72, -0.04], 0.02, "up axis at the corner (-9.66, 9.77)");
            found_corner = true;
        }
    }
    assert!(found_corner);
}
