//! specs/combat_arms/world_objects.md Follow-up (2026-10-31b) and
//! effects_sounds.md Follow-up (2026-10-31b) on the user's install: the
//! event objects' census (1), the trigger resource table and the stock
//! markers' names (1.6, cases 35 and 37), the destructibles' debris types
//! (4, case 50), the debris table (effects_sounds.md case 84) and the
//! surface-0 polygons (case 96); the stock `VipExitTimer` run on OILRIG's
//! object world (case 28). Pass with a note without `[games.combat_arms]
//! path` in chimera.local.toml (the keyed archives without their keys).

use std::collections::{BTreeMap, BTreeSet};

use chimera_core::mount::{Mount, config::LocalConfig};
use combat_arms::game::{
    GAME, INFO,
    objects::{self, events::Notice, markers::TriggerResources, runtime::Fx},
    world,
};
use lithtech_formats::{
    attributes::Document,
    world::{Value, World, surface_flags},
};

fn mount() -> Option<Mount> {
    let Some(install) = LocalConfig::load().ok().and_then(|c| c.game_path(GAME)) else {
        eprintln!("skipping: no Combat Arms install configured");
        return None;
    };
    if !chimera_core::mount::served::usable("combat_arms", &install, "Game") {
        eprintln!("skipping: no Combat Arms install at {}", install.display());
        return None;
    }
    Some((INFO.open)(&install).unwrap())
}

fn world_file(mount: &Mount, name: &str) -> Option<World> {
    let path = world::find(mount, name)?;
    World::parse(&mount.read(&path).ok()?).ok()
}

/// Every readable world file (None when fewer than all 105 are).
fn all_worlds(mount: &Mount) -> Option<Vec<(String, World)>> {
    let names = world::map_names(mount);
    let worlds: Vec<(String, World)> =
        names.iter().filter_map(|n| world_file(mount, n).map(|w| (n.to_string(), w))).collect();
    if worlds.len() < 105 {
        eprintln!("skipping: {} of {} maps readable (the V2 key)", worlds.len(), names.len());
        return None;
    }
    Some(worlds)
}

fn string(o: &lithtech_formats::world::Object, k: &str) -> Option<String> {
    match o.get(k) {
        Some(Value::String(s)) => Some(s.clone()),
        _ => None,
    }
}

/// 1's census: the event objects' classes in all 105 maps (objects, maps)
/// and their names.
#[test]
fn event_object_census() {
    let Some(mount) = mount() else { return };
    let Some(worlds) = all_worlds(&mount) else { return };
    let mut counts: BTreeMap<&str, (usize, BTreeSet<String>)> = BTreeMap::new();
    let mut names: BTreeMap<&str, BTreeMap<String, usize>> = BTreeMap::new();
    let classes = [
        "TimerTrigger",
        "LightActivateTrigger",
        "CaptionTrigger",
        "MessageTrigger",
        "MissionTrigger",
        "TargetTrigger",
        "Camera",
        "CameraPoint",
    ];
    for (map, w) in &worlds {
        for o in &w.objects {
            let Some(c) = classes.iter().find(|c| **c == o.class) else { continue };
            let e = counts.entry(c).or_default();
            e.0 += 1;
            e.1.insert(map.clone());
            *names.entry(c).or_default().entry(o.name().unwrap_or("").to_string()).or_default() += 1;
        }
    }
    let n = |c: &str| counts.get(c).map_or((0, 0), |(k, m)| (*k, m.len()));
    assert_eq!(n("TimerTrigger"), (17, 10));
    assert_eq!(n("LightActivateTrigger").0, 79);
    assert_eq!(n("LightActivateTrigger").1, 1, "all in one map");
    assert_eq!(n("CaptionTrigger").0, 18);
    assert_eq!(n("MessageTrigger").0, 8);
    assert_eq!(n("MissionTrigger").0, 7);
    assert_eq!(n("TargetTrigger"), (208, 38));
    assert_eq!(n("Camera").0, 104);
    assert_eq!(n("CameraPoint").0, 389);
    assert_eq!(names["CaptionTrigger"].keys().collect::<Vec<_>>(), ["CapTRG01"]);
    assert_eq!(names["MessageTrigger"].keys().collect::<Vec<_>>(), ["MsgTRG01"]);
    assert_eq!(names["MissionTrigger"].keys().collect::<Vec<_>>(), ["MishTRG01"]);
    assert_eq!((names["Camera"]["Camera0"], names["Camera"]["Camera00"]), (99, 5));
}

/// 1.6, cases 35 and 37: the install's trigger resource table has 308
/// records; 104 of the 107 stock marker names have one, the three that
/// don't being `DynamiteTargeticon00`, `WeaponSpawnMarker00` and
/// `WeaponSpawnMarker01`. VIP Escort's exit markers carry the pictures the
/// spec names.
#[test]
fn trigger_resource_table_and_marker_names() {
    let Some(mount) = mount() else { return };
    let Ok(bytes) = mount.read(objects::markers::FILE) else {
        eprintln!("skipping: no {} (the V1 key)", objects::markers::FILE);
        return;
    };
    let table = TriggerResources::parse(&Document::parse(&bytes).unwrap());
    assert_eq!(table.0.len(), 308);
    let z = table.find("Indicator_EZone_unable").expect("the exit's marker");
    assert_eq!(z.guide.replace('/', "\\").to_lowercase(), "ui_hud_common\\guide\\guide_area_exit_notopen.dtx");
    assert_eq!(z.map.replace('/', "\\").to_lowercase(), "ui_hud_map\\icon\\area_exit_notopen.dtx");
    assert!(z.show_dist);
    let Some(worlds) = all_worlds(&mount) else { return };
    let markers: BTreeSet<String> = worlds
        .iter()
        .flat_map(|(_, w)| w.objects_of("TargetTrigger").filter_map(|(_, o)| o.name().map(str::to_string)).collect::<Vec<_>>())
        .collect();
    assert_eq!(markers.len(), 107);
    let missing: Vec<&String> = markers.iter().filter(|n| table.find(n).is_none()).collect();
    assert_eq!(missing, ["DynamiteTargeticon00", "WeaponSpawnMarker00", "WeaponSpawnMarker01"]);
}

/// 4 and case 50: the destructibles naming a debris type other than
/// `None`: 390 (`WorldModel` `Window` 86, `Window_Large` 61,
/// `Window_smoll` 52, `Stone` 52; `Breakable` `56x56WoodBox` 120, `Window`
/// 13, `Window_Large` 6); `56x56WoodBox` only on breakables with
/// `SurfaceOverride` `Wood`, `HitPoints` 300 and `RemoveOnDeath` false;
/// 47 `Window` world models never destroyed.
#[test]
fn debris_type_census() {
    let Some(mount) = mount() else { return };
    let Some(worlds) = all_worlds(&mount) else { return };
    let mut by: BTreeMap<(String, String), usize> = BTreeMap::new();
    let mut never = 0;
    for (_, w) in &worlds {
        for o in &w.objects {
            let Some(d) = string(o, "DebrisType") else { continue };
            if d.eq_ignore_ascii_case("None") || d.is_empty() {
                continue;
            }
            *by.entry((o.class.clone(), d.clone())).or_default() += 1;
            if o.class == "WorldModel" && d == "Window" && o.bool("NeverDestroy") == Some(true) {
                never += 1;
            }
            if d == "56x56WoodBox" {
                assert_eq!(o.class, "Breakable");
                assert_eq!(string(o, "SurfaceOverride").as_deref(), Some("Wood"));
                assert_eq!(o.real("HitPoints"), Some(300.0));
                assert_eq!(o.bool("RemoveOnDeath"), Some(false));
            }
        }
    }
    let n = |c: &str, d: &str| by.get(&(c.to_string(), d.to_string())).copied().unwrap_or(0);
    assert_eq!(
        [n("WorldModel", "Window"), n("WorldModel", "Window_Large"), n("WorldModel", "Window_smoll"), n("WorldModel", "Stone")],
        [86, 61, 52, 52]
    );
    assert_eq!([n("Breakable", "56x56WoodBox"), n("Breakable", "Window"), n("Breakable", "Window_Large")], [120, 13, 6]);
    assert_eq!(by.values().sum::<usize>(), 390, "{by:?}");
    assert_eq!(never, 47);
}

/// effects_sounds.md 96: surface type 0 on 312 of WAREHOUSE's 32,688
/// solid physics polygons; across the install 100,922 of 4,057,293 (in 98
/// maps) and 72,275 of 594,095 solid world-model polygons.
#[test]
fn unknown_surface_census() {
    let Some(mount) = mount() else { return };
    let count = |w: &World, physics: bool| {
        let (mut zero, mut all) = (0usize, 0usize);
        for m in w.models.iter().filter(|m| m.is_physics() == physics) {
            for i in 0..m.polygons.len() {
                let s = m.polygon_surface(i);
                if s.flags & surface_flags::SOLID == 0 {
                    continue;
                }
                all += 1;
                zero += (s.surface_type == 0) as usize;
            }
        }
        (zero, all)
    };
    if let Some(w) = world_file(&mount, "warehouse") {
        assert_eq!(count(&w, true), (312, 32_688), "WAREHOUSE");
    }
    let Some(worlds) = all_worlds(&mount) else { return };
    let (mut z, mut a, mut maps, mut wz, mut wa) = (0, 0, 0, 0, 0);
    for (_, w) in &worlds {
        let (zero, all) = count(w, true);
        z += zero;
        a += all;
        maps += (zero > 0) as usize;
        let (zero, all) = count(w, false);
        wz += zero;
        wa += all;
    }
    assert_eq!((z, a, maps), (100_922, 4_057_293, 98), "physics");
    assert_eq!((wz, wa), (72_275, 594_095), "world models");
}

/// Case 28 on the stock data: each map's `VipExitTimer`, started at 0,
/// stops itself at its `TimeAmount` + 1 s (the mission file sets 60 s:
/// game_modes_2.md case 54's 61 s).
#[test]
fn stock_vip_exit_timers() {
    let Some(mount) = mount() else { return };
    let mut seen = 0;
    for name in world::map_names(&mount) {
        let Some(w) = world_file(&mount, &name) else { continue };
        if !w.objects.iter().any(|o| o.class == "TimerTrigger" && o.name() == Some("VipExitTimer")) {
            continue;
        }
        let Ok(map) = world::load_world(&mount, &name) else { continue };
        let mut ow = objects::object_world(&map.entities);
        let id = ow.named_all("VipExitTimer")[0];
        let (time, auto) = match &ow.objects[id as usize].kind {
            combat_arms::game::objects::runtime::Kind::Timer(t) => (t.time, t.auto_stop),
            k => panic!("{name}: {k:?}"),
        };
        eprintln!("{name}: VipExitTimer T {time} auto-stop {auto}");
        if !auto {
            continue;
        }
        seen += 1;
        ow.fx.clear();
        ow.deliver(id, "START", Default::default());
        let mut t = 0.0f64;
        let mut stopped = None;
        while t < time as f64 + 5.0 && stopped.is_none() {
            ow.frame(t, 0.05);
            if ow.fx.iter().any(|f| matches!(f, Fx::Notice { notice: Notice::Timer { on: false, .. }, .. })) {
                stopped = Some(t);
            }
            ow.fx.clear();
            t += 0.05;
        }
        let s = stopped.expect("it stopped itself");
        assert!((s - (time as f64 + 1.0)).abs() < 1e-3, "{name}: stopped at {s}");
    }
    eprintln!("{seen} exit timers");
}

/// effects_sounds.md 84: the install's `ATTRIBUTES\DEBRIS.TXT` gives the
/// six records of the spec's table, ours (data/debris.toml) the same.
#[test]
fn install_debris_table() {
    use combat_arms::game::effects::debris::{CaDebrisTable, FILE};
    let Some(mount) = mount() else { return };
    let Ok(bytes) = mount.read(FILE) else {
        eprintln!("skipping: no {FILE} (the V1 key)");
        return;
    };
    let install = CaDebrisTable::parse(&Document::parse(&bytes).unwrap());
    let ours = &CaDebrisTable::built_in();
    assert_eq!(install.0.len(), 6);
    let path = |s: &String| s.replace('/', "\\").to_lowercase();
    for (a, b) in install.0.iter().zip(&ours.0) {
        assert_eq!(a.name, b.name);
        assert_eq!((a.surface, a.number, a.bounce, a.rotate != 0), (b.surface, b.number, b.bounce, b.rotate != 0), "{}", a.name);
        for (x, y) in [(a.scale, b.scale), (a.life, b.life)] {
            assert!((x[0] - y[0]).abs() < 1e-4 && (x[1] - y[1]).abs() < 1e-4, "{}", a.name);
        }
        for (x, y) in [
            (a.vel_min, b.vel_min),
            (a.vel_max, b.vel_max),
            (a.world_vel_min, b.world_vel_min),
            (a.world_vel_max, b.world_vel_max),
            (a.offset_min, b.offset_min),
            (a.offset_max, b.offset_max),
        ] {
            assert!(x.iter().zip(y).all(|(p, q)| (p - q).abs() < 1e-3), "{}: {x:?} vs {y:?}", a.name);
        }
        let (x, y) = ([a.fade, a.gravity, a.alpha], [b.fade, b.gravity, b.alpha]);
        assert!(x.iter().zip(y).all(|(p, q)| (p - q).abs() < 1e-4), "{}: {x:?} vs {y:?}", a.name);
        for (x, y) in [
            (&a.models, &b.models),
            (&a.skins, &b.skins),
            (&a.bounce_sounds, &b.bounce_sounds),
            (&a.explode_sounds, &b.explode_sounds),
        ] {
            assert_eq!(x.iter().map(path).collect::<Vec<_>>(), y.iter().map(path).collect::<Vec<_>>(), "{}", a.name);
        }
        assert!(a.world_fx.is_empty() && a.impact_fx.is_empty());
    }
    assert_eq!(install.0[4].rotate, 360, "56x56WoodBox's Rotate stored as 360");
}

/// 4 on a stock map: the first map with a `56x56WoodBox` breakable; shot
/// to death on its object world, it sends its debris effect (that record,
/// along the shot) and a client throws 6 pieces of it at high detail.
#[test]
fn stock_wood_box_debris() {
    use combat_arms::game::effects::debris::{CaDebrisTable, DebrisLevel, launch};
    let Some(mount) = mount() else { return };
    let table = CaDebrisTable::install();
    for name in world::map_names(&mount) {
        let Some(w) = world_file(&mount, &name) else { continue };
        let Some(boxed) = w
            .objects
            .iter()
            .find(|o| o.class == "Breakable" && string(o, "DebrisType").as_deref() == Some("56x56WoodBox"))
            .and_then(|o| o.name().map(str::to_string))
        else {
            continue;
        };
        let Ok(map) = world::load_world(&mount, &name) else { continue };
        let mut ow = objects::object_world(&map.entities);
        ow.frame(0.0, 0.05);
        ow.fx.clear();
        let id = ow.named_all(&boxed)[0];
        ow.hit_dir = bevy::math::Vec3::X;
        ow.damage(id, 1000.0, Default::default(), true);
        let sent = ow.fx.iter().find_map(|f| match f {
            Fx::Debris { at, debris, dir } => Some((*at, debris.clone(), *dir)),
            _ => None,
        });
        let (at, debris, dir) = sent.expect("its debris");
        assert_eq!((debris.as_str(), dir), ("56x56WoodBox", bevy::math::Vec3::X), "{name} {boxed}");
        assert_eq!(at, ow.objects[id as usize].pose.pos);
        let r = table.get(table.id(&debris).unwrap()).unwrap();
        assert_eq!(launch(r, DebrisLevel::High, at, dir, &mut || 0.5).len(), 6);
        eprintln!("{name}: {boxed} at {at}");
        return;
    }
    panic!("no 56x56WoodBox breakable on a readable map");
}
