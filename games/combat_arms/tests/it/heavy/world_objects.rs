//! specs/combat_arms/world_objects.md's cases on the user's install: the
//! object census of the 105 maps (case 1) and per map (2); FREEZER's
//! radio knob door and sliding beak and wing (3, 4); SNOWSTORM's boss
//! camera keyframer (12); HGOILRIG's rising sea from 90 s with its
//! volumes (8, also in play), its fan (9), its drum (10, 11), its keys'
//! blind data (14) and the decks it never had (26); BLOODMONEY's truck
//! doors in front of `UNITSPAWN_ALPHA`, which a hired unit now opens;
//! CITYOFWINGS's VIP cell (game_modes_2.md case 23); a network client
//! swimming in HGOILRIG's rising sea, with the sea and its water-sound
//! keyframers predicted (10, Client). Most run the object
//! world of the converted map (`objects::object_world`) at the server's
//! 20 Hz; the sea, the doors and the cell also in play. Pass with a note
//! without `[games.combat_arms] path` in chimera.local.toml (the keyed
//! maps without the V2 key).

use std::collections::BTreeMap;

use bevy::prelude::*;
use chimera_core::{
    core::{Health, Intent, Team},
    mount::{Mount, config::LocalConfig},
};
use chimera_world::{
    harness::Sim,
    map::{MapData, MapSimPlugin},
};
use combat_arms::{
    game::{
        GAME, INFO,
        objects::{
            self, CaObjectMessage, CaObjects,
            commands::{Ctx, Who},
            geometry::{from_engine, to_engine},
            keyframer::{self, segment_length},
            movers::{CharBox, State},
            runtime::{Fx, Kind, ObjectWorld},
        },
        movement::{self, movement_data},
        rules::mercs::{self, MercOrder},
        volumes::CaVolume,
        world,
    },
    harness::{CaSim, with_mode},
};
use lithtech_formats::world::{Value, World, surface_flags};

const DT: f32 = 0.05;

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

/// A map's world file, parsed (None without it or its key).
fn world_file(mount: &Mount, name: &str) -> Option<World> {
    let path = world::find(mount, name)?;
    World::parse(&mount.read(&path).ok()?).ok()
}

/// A map converted (None without it or its key).
fn map(mount: &Mount, name: &str) -> Option<MapData> {
    let m = world::find(mount, name).and_then(|_| world::load_world(mount, name).ok());
    if m.is_none() {
        eprintln!("skipping: {name} not readable");
    }
    m
}

/// The object world of a converted map, driven at 20 Hz.
struct T {
    w: ObjectWorld,
    t: f64,
}

impl T {
    fn of(map: &MapData) -> Self {
        let mut w = objects::object_world(&map.entities);
        w.fx.clear();
        Self { w, t: 0.0 }
    }
    fn id(&self, name: &str) -> u32 {
        *self.w.named_all(name).first().unwrap_or_else(|| panic!("no object {name}"))
    }
    fn frame(&mut self) {
        self.w.frame(self.t, DT);
        self.t += DT as f64;
    }
    fn frames(&mut self, n: usize) {
        for _ in 0..n {
            self.frame();
        }
    }
    fn send(&mut self, name: &str, message: &str, who: Who) {
        self.w.now = self.t;
        let id = self.id(name);
        self.w.deliver(id, message, Ctx { sender: who, target: None, ..Ctx::default() });
    }
    fn run(&mut self, text: &str) {
        self.w.now = self.t;
        self.w.run(text, Ctx::default());
    }
    fn pos(&self, name: &str) -> Vec3 {
        self.w.objects[self.id(name) as usize].pose.pos
    }
    fn state(&self, name: &str) -> State {
        self.w.objects[self.id(name) as usize].awm().unwrap().state
    }
    fn angles(&self, name: &str) -> Vec3 {
        self.w.objects[self.id(name) as usize].awm().unwrap().angles
    }
    fn on(&self, name: &str) -> bool {
        matches!(self.w.objects[self.id(name) as usize].kind, Kind::Switchable { on: true, .. })
    }
}

fn close3(a: Vec3, b: Vec3, tol: f32, what: &str) {
    assert!(a.distance(b) <= tol, "{what}: {a} vs {b}");
}

/// A player's box standing with its centre at `at` (file frame).
fn player_at(id: u64, at: Vec3) -> CharBox {
    CharBox {
        who: Who::Character(id),
        min: at - Vec3::new(16.0, 50.0, 16.0),
        max: at + Vec3::new(16.0, 50.0, 16.0),
        player: true,
        alive: true,
        using: false,
    }
}

/// Probe: print the objects of `CHIMERA_PROBE_MAP` matching
/// `CHIMERA_PROBE_FILTER` (`|`-separated) to target/scratch/probe.txt.
#[test]
fn probe_objects() {
    let (Ok(maps), Ok(filter)) = (std::env::var("CHIMERA_PROBE_MAP"), std::env::var("CHIMERA_PROBE_FILTER")) else {
        return;
    };
    let Some(mount) = mount() else { return };
    let text = |v: &Value| match v {
        Value::String(s) => format!("{s:?}"),
        Value::Vector(v) => format!("({}, {}, {})", v[0], v[1], v[2]),
        Value::Real(x) | Value::Long(x) | Value::Flags(x) => x.to_string(),
        Value::Bool(b) => b.to_string(),
        Value::Rotation(r) => format!("rot({}, {}, {})", r[0], r[1], r[2]),
        Value::Color(c) => format!("rgb{c:?}"),
        Value::Unknown { .. } => "?".into(),
    };
    let mut out = String::new();
    for m in maps.split(',') {
        let Some(w) = world_file(&mount, m) else {
            out += &format!("{m}: not readable\n");
            continue;
        };
        for (i, o) in w.objects.iter().enumerate() {
            let props: Vec<String> = o.properties.iter().map(|p| format!("{} = {}", p.name, text(&p.value))).collect();
            let hit = filter.split('|').any(|f| {
                let f = f.to_lowercase();
                o.class.to_lowercase().contains(&f) || props.iter().any(|p| p.to_lowercase().contains(&f))
            });
            if !hit {
                continue;
            }
            out += &format!("{m} [{i}] {}\n", o.class);
            for p in props {
                out += &format!("    {p}\n");
            }
            if let Some(md) = o.name().and_then(|n| w.models.iter().find(|x| x.name == n)) {
                out += &format!("    (model min {:?} max {:?} translation {:?})\n", md.min, md.max, md.translation);
            }
        }
    }
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/scratch");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("probe.txt"), out).unwrap();
}

/// Cases 1 and 2: the classes the object world runs, counted in all 105
/// maps (objects / maps), and in four maps.
#[test]
fn t01_t02_census() {
    let Some(mount) = mount() else { return };
    let names = world::map_names(&mount);
    let mut counts: BTreeMap<String, (usize, usize)> = BTreeMap::new();
    let mut per: BTreeMap<String, BTreeMap<String, usize>> = BTreeMap::new();
    let mut read = 0;
    for n in &names {
        let Some(w) = world_file(&mount, n) else { continue };
        read += 1;
        let mut here: BTreeMap<String, usize> = BTreeMap::new();
        for o in &w.objects {
            *here.entry(o.class.clone()).or_default() += 1;
        }
        for (c, k) in &here {
            let e = counts.entry(c.clone()).or_default();
            e.0 += k;
            e.1 += 1;
        }
        per.insert(n.to_string(), here);
    }
    if read < 105 {
        eprintln!("skipping the census: {read} of {} maps readable (the V2 key)", names.len());
    } else {
        let want = [
            ("KeyFramer", 31, 10),
            ("RotatingDoor", 392, 45),
            ("SlidingDoor", 345, 39),
            ("RotatingWorldModel", 28, 7),
            ("SlidingWorldModel", 30, 2),
            ("SpinningWorldModel", 35, 16),
            ("RotatingSwitch", 7, 4),
            ("Breakable", 1087, 73),
            ("WorldModel", 3256, 100),
            ("WorldModelDebris", 1, 1),
            ("Trigger", 2919, 104),
            ("CommandTrigger", 508, 28),
            ("TouchTrigger", 49, 2),
            ("TargetTrigger", 208, 38),
            ("TimerTrigger", 17, 10),
            ("EventCounter", 48, 19),
            ("StartupCommand", 97, 68),
            ("Explosion", 340, 45),
            ("PropType", 79, 6),
            ("Spawner", 1701, 25),
            ("SoundFX", 1637, 88),
            ("Camera", 104, 100),
        ];
        for (c, objects, maps) in want {
            assert_eq!(counts.get(c).copied().unwrap_or_default(), (objects, maps), "{c}");
        }
        for c in ["Door", "Switch", "SlidingSwitch", "ActiveWorldModel", "Prop"] {
            assert!(!counts.contains_key(c), "{c}");
        }
    }
    let count = |m: &str, c: &str| per.get(m).and_then(|h| h.get(c)).copied().unwrap_or(0);
    if per.contains_key("warehouse") {
        let w = "warehouse";
        assert_eq!(
            [count(w, "Breakable"), count(w, "WorldModel"), count(w, "Trigger"), count(w, "Explosion"), count(w, "Camera")],
            [12, 41, 11, 6, 1]
        );
        assert_eq!(count(w, "RotatingDoor") + count(w, "SlidingDoor") + count(w, "KeyFramer"), 0);
    }
    if per.contains_key("freezer") {
        let f = "freezer";
        assert_eq!(
            [
                count(f, "RotatingDoor"),
                count(f, "SlidingDoor"),
                count(f, "WorldModel"),
                count(f, "Trigger"),
                count(f, "StartupCommand"),
                count(f, "SoundFX")
            ],
            [1, 2, 6, 18, 1, 10]
        );
    }
    if per.contains_key("snowstorm") {
        let s = "snowstorm";
        assert_eq!(
            [
                count(s, "Breakable"),
                count(s, "SlidingDoor"),
                count(s, "RotatingDoor"),
                count(s, "RotatingWorldModel"),
                count(s, "KeyFramer"),
                count(s, "WorldModel"),
                count(s, "EventCounter"),
                count(s, "Trigger"),
                count(s, "Spawner"),
                count(s, "PropType")
            ],
            [45, 15, 1, 2, 1, 48, 3, 21, 45, 2]
        );
    }
    if let Some(w) = per.contains_key("hgoilrig").then(|| world_file(&mount, "hgoilrig")).flatten() {
        let h = "hgoilrig";
        assert_eq!(
            [
                count(h, "KeyFramer"),
                count(h, "SpinningWorldModel"),
                count(h, "SlidingDoor"),
                count(h, "RotatingDoor"),
                count(h, "Breakable"),
                count(h, "WorldModel"),
                count(h, "Trigger"),
                count(h, "CommandTrigger")
            ],
            [8, 6, 5, 1, 11, 14, 25, 2]
        );
        let keys: Vec<usize> = w.blind_data.iter().filter(|c| c.id == keyframer::KEY_CHUNK_ID).map(|c| c.data.len()).collect();
        assert_eq!(keys, [124; 8]);
    }
}

/// Case 3: FREEZER's radio knob (`CMD_A_Radio`, a rotating door about its
/// `Pos`) opens away from its activator, closes when used again.
#[test]
fn t03_freezer_radio_knob() {
    let Some(mount) = mount() else { return };
    let Some(data) = map(&mount, "freezer") else { return };
    let base = T::of(&data);
    let id = base.id("CMD_A_Radio");
    let o = &base.w.objects[id as usize];
    close3(o.pose.pos, Vec3::new(508.5, 66.5, 367.8677), 1e-3, "placed at the BSP centre");
    assert_eq!(o.pose.rot, Quat::IDENTITY);
    let a = o.awm().unwrap();
    close3(a.pivot, Vec3::new(490.0, 80.0, 369.367), 1e-3, "pivot = Pos");
    assert_eq!(a.duration(State::PowerOff), 2.0, "closing time 0 = opening time");
    for (from, yaw1, centre1, yaw2, centre2) in [
        (
            Vec3::new(520.0, 66.5, 400.0),
            0.5f32,
            Some(Vec3::new(508.486, 66.5, 367.706)),
            1.0f32,
            Vec3::new(508.471, 66.5, 367.545),
        ),
        (Vec3::new(520.0, 66.5, 340.0), -0.5, None, -1.0, Vec3::new(508.523, 66.5, 368.191)),
    ] {
        let mut t = T::of(&data);
        t.frame();
        t.w.chars = vec![player_at(1, from)];
        let before = t.w.cmds.int("RadioA").unwrap_or(0);
        t.send("CMD_A_Radio", "ACTIVATE", Who::Character(1));
        assert_eq!(t.state("CMD_A_Radio"), State::PowerOn);
        t.frames(20);
        assert!((t.angles("CMD_A_Radio").y.to_degrees() - yaw1).abs() < 1e-3, "{}", t.angles("CMD_A_Radio").y.to_degrees());
        if let Some(c) = centre1 {
            close3(t.pos("CMD_A_Radio"), c, 2e-3, "after 1 s");
        }
        t.frames(20);
        assert!((t.angles("CMD_A_Radio").y.to_degrees() - yaw2).abs() < 1e-3);
        close3(t.pos("CMD_A_Radio"), centre2, 2e-3, "after 2 s");
        assert_eq!(t.state("CMD_A_Radio"), State::On);
        assert_eq!(t.w.cmds.int("RadioA"), Some(before + 1), "OpenCommand once");
        // Used again: closes over 2 s.
        t.send("CMD_A_Radio", "ACTIVATE", Who::Character(1));
        assert_eq!(t.state("CMD_A_Radio"), State::PowerOff);
        t.frames(40);
        assert_eq!(t.state("CMD_A_Radio"), State::Off);
        assert!(t.angles("CMD_A_Radio").y.abs() < 1e-5);
    }
}

/// Case 4: FREEZER's chicken beak slides down 8 u a frame to its open
/// pose on the 4th moving frame, its opening sound attached; the wing
/// slides along its tilted axis.
#[test]
fn t04_freezer_beak_and_wing() {
    let Some(mount) = mount() else { return };
    let Some(data) = map(&mount, "freezer") else { return };
    let mut t = T::of(&data);
    t.frame();
    t.w.fx.clear();
    let start = t.pos("Chk_mouth");
    close3(start, Vec3::new(126.708, 271.6895, 2042.4663), 1e-3, "beak's centre");
    t.send("Chk_mouth", "TRIGGER", Who::Nobody);
    let sound = t.w.fx.iter().find_map(|f| match f {
        Fx::Sound { file, attach, radius, inner, .. } => Some((file.clone(), *attach, *radius, *inner)),
        _ => None,
    });
    let (file, attach, radius, inner) = sound.expect("the opening sound on the trigger frame");
    assert!(file.eq_ignore_ascii_case("SND\\Freezer\\Crazy_Chicken.wav"), "{file}");
    assert_eq!((attach, radius, inner), (Some(t.id("Chk_mouth")), 1000.0, 250.0));
    // The beak's sound (14): a one-shot that follows it.
    let s = t.w.objects[t.id("Chk_mouth") as usize].sound.clone().expect("the beak's sound");
    assert_eq!((s.looping, s.attach), (false, Some(t.id("Chk_mouth"))));
    for n in 1..=4 {
        t.frame();
        close3(t.pos("Chk_mouth"), start - Vec3::Y * 8.0 * n as f32, 1e-3, &format!("frame {n}"));
    }
    assert_eq!(t.state("Chk_mouth"), State::On);
    let wing = t.pos("Chk_wing");
    t.send("Chk_wing", "TRIGGER", Who::Nobody);
    t.frames(40);
    close3(t.pos("Chk_wing"), wing + Vec3::new(0.0, 11.314, 11.314), 1e-2, "the wing open");
}

/// Case 12: SNOWSTORM's boss camera keyframer: velocity mode, `Sine`, a
/// straight segment.
#[test]
fn t12_snowstorm_boss_camera() {
    let Some(mount) = mount() else { return };
    let Some(data) = map(&mount, "snowstorm") else { return };
    let mut t = T::of(&data);
    let kf = t.id("Cut_Boss_Camera");
    let Kind::KeyFramer(k) = &t.w.objects[kf as usize].kind else { panic!("a keyframer") };
    assert_eq!(k.keys.len(), 2);
    assert!(k.ignore_offsets && k.total_time == 2.0);
    close3(k.keys[0].pos, Vec3::new(40.0, -2909.0, -903.0), 1e-3, "key 0");
    close3(k.keys[1].pos, Vec3::new(40.0, -3012.0, -1923.0), 1e-3, "key 1");
    assert!(k.keys[0].next.is_none() && k.keys[1].prev.is_some() && k.keys[1].next.is_some());
    assert!((k.length() - 1025.187).abs() < 1e-2);
    t.frame();
    t.send("Cut_Boss_Camera", "ON", Who::Nobody);
    t.frames(21);
    close3(t.pos("Camera0"), Vec3::new(40.0, -2962.496, -1432.765), 0.05, "after update 20");
    t.frames(20);
    close3(t.pos("Camera0"), k_pos(&t, kf, 1), 1e-3, "update 40 on key 1");
    t.frame();
    let Kind::KeyFramer(k) = &t.w.objects[kf as usize].kind else { unreachable!() };
    assert!(!k.active, "the list ended");
    close3(t.pos("Camera0"), Vec3::new(40.0, -3012.0, -1923.0), 1e-3, "left on key 1");
}

fn k_pos(t: &T, kf: u32, i: usize) -> Vec3 {
    let Kind::KeyFramer(k) = &t.w.objects[kf as usize].kind else { unreachable!() };
    k.keys[i].pos
}

/// Case 14: HGOILRIG `KeyFramerA`'s keys from blind data.
#[test]
fn t14_hgoilrig_keys() {
    let Some(mount) = mount() else { return };
    let Some(data) = map(&mount, "hgoilrig") else { return };
    let t = T::of(&data);
    let lengths: Vec<(String, f32, f32)> = ["A", "B", "C", "D", "E", "F", "G", "H"]
        .iter()
        .map(|s| {
            let id = t.id(&format!("KeyFramer{s}"));
            let Kind::KeyFramer(k) = &t.w.objects[id as usize].kind else { panic!("KeyFramer{s}") };
            (s.to_string(), k.keys[0].pos.distance(k.keys[1].pos), k.length())
        })
        .collect();
    let id = t.id("KeyFramerA");
    let Kind::KeyFramer(k) = &t.w.objects[id as usize].kind else { unreachable!() };
    assert_eq!(k.keys.len(), 2);
    close3(k.keys[0].pos, Vec3::new(-5216.0, -14.0, 2596.0), 1e-3, "key 0");
    close3(k.keys[0].prev.unwrap(), Vec3::new(-612.0, 0.0, 624.0), 1e-3, "prev 0");
    close3(k.keys[0].next.unwrap(), Vec3::new(508.0, -14.0, -352.0), 1e-3, "next 0");
    close3(k.keys[1].pos, Vec3::new(-2336.0, 946.0, 740.0), 1e-3, "key 1");
    assert!((k.keys[1].stamp - 90.0).abs() < 1e-4);
    assert!((k.keys[0].angles.y + std::f32::consts::FRAC_PI_2).abs() < 1e-4);
    assert!((k.keys[1].angles.y + 3.0 * std::f32::consts::FRAC_PI_4).abs() < 1e-4);
    assert!((segment_length(&k.keys[0], &k.keys[1]) - 3566.53).abs() < 0.5, "arc {}", k.length());
    assert!((k.length() / k.total_time - 39.63).abs() < 0.01);
    let want = [3558.19, 2066.17, 5344.00, 7093.98, 6386.63, 5655.04, 6053.42, 4658.17];
    for ((name, straight, _), w) in lengths.iter().zip(want) {
        assert!((straight - w).abs() < 0.05, "{name}: {straight}");
    }
}

/// Case 8: HGOILRIG's sea rises 500 u from 90 s into round 4 (its start
/// trigger's `delayid Wup 90`), carrying its `Water` and `PoisonGas00`
/// volumes; the end trigger puts it back at once.
#[test]
fn t08_hgoilrig_rising_sea() {
    let Some(mount) = mount() else { return };
    let Some(data) = map(&mount, "hgoilrig") else { return };
    let mut t = T::of(&data);
    t.frame();
    let sea = t.pos("SlidingWater");
    close3(sea, Vec3::new(560.708, 1.0, -1951.5889), 1e-3, "the sea's centre");
    let water = t.pos("Water");
    let gas = t.pos("PoisonGas00");
    t.send("Level04StartTrigger", "TRIGGER", Who::Nobody);
    let start = t.t;
    // Frames until the sea moves.
    let mut first = None;
    let mut half = None;
    let mut up = None;
    for _ in 0..(230.0 / DT) as usize {
        let before = t.pos("SlidingWater").y;
        t.frame();
        let now = t.pos("SlidingWater").y;
        let at = t.t - DT as f64 - start;
        if first.is_none() && t.state("SlidingWater") == State::PowerOn {
            first = Some(at);
        }
        let tau = t.w.objects[t.id("SlidingWater") as usize].awm().unwrap().tau;
        if half.is_none() && (tau - 60.0).abs() < 0.026 {
            half = Some((now - before) / DT);
        }
        if up.is_none() && t.state("SlidingWater") == State::On {
            up = Some(at);
        }
    }
    let first = first.expect("rising");
    assert!((first - 90.0).abs() < 0.06, "from 90 s: {first}");
    let half = half.expect("half time");
    assert!((half - 4.632).abs() < 0.02, "4.632 u/s at half time: {half}");
    let up = up.expect("up");
    assert!(up > 205.0 && up < 210.06, "500 u up after about 120 s (a little early): {}", up - 90.0);
    close3(t.pos("SlidingWater"), sea + Vec3::Y * 500.0, 1e-2, "500 u up");
    close3(t.pos("Water") - t.pos("SlidingWater"), water - sea, 1e-2, "the water keeps its offset");
    close3(t.pos("PoisonGas00") - t.pos("SlidingWater"), gas - sea, 1e-2, "the gas keeps its offset");
    t.w.fx.clear();
    t.send("Level04EndTrigger", "TRIGGER", Who::Nobody);
    close3(t.pos("SlidingWater"), sea, 1e-3, "init: back at once");
    assert_eq!(t.state("SlidingWater"), State::Off);
    assert!(t.w.cmds.pending.iter().flatten().all(|p| p.id.as_deref() != Some("Wup")));
}

/// Case 8 in play: after the start trigger and 100 s, the `Water`
/// volume's top has risen (the bridge moves its planes).
#[test]
fn t08_hgoilrig_sea_volume_in_play() {
    let Some(mount) = mount() else { return };
    let Some(data) = world::find(&mount, "hgoilrig").and_then(|_| world::load_from(&mount, "hgoilrig").ok()) else {
        eprintln!("skipping: hgoilrig not readable");
        return;
    };
    let s = movement_data().meters_per_unit;
    let mut sim = Sim::new(CaSim, MapSimPlugin::new(data));
    sim.ticks(3);
    let top = |sim: &mut Sim| {
        let w = sim.app.world_mut();
        let mut q = w.query::<(&Name, &CaVolume)>();
        q.iter(w).find(|(n, _)| n.as_str().starts_with("Volume Water ")).map(|(_, v)| v.max.y / s).unwrap()
    };
    let before = top(&mut sim);
    assert!((before - -58.0).abs() < 0.5, "the water's top at load: {before}");
    sim.app.world_mut().write_message(CaObjectMessage {
        target: "Level04StartTrigger".into(),
        message: "trigger".into(),
    });
    sim.seconds(150.0);
    let after = top(&mut sim);
    assert!(after > before + 100.0, "risen: {before} → {after}");
}

/// Case 9: HGOILRIG's fan spins about X at a turn a second from its first
/// update, about its own centre.
#[test]
fn t09_hgoilrig_fan() {
    let Some(mount) = mount() else { return };
    let Some(data) = map(&mount, "hgoilrig") else { return };
    let mut t = T::of(&data);
    let centre = t.pos("RotatingWorldModel0");
    close3(centre, Vec3::new(1091.9429, 638.4099, -809.6885), 1e-3, "centre");
    for _ in 0..30 {
        t.frame();
    }
    let a = t.angles("RotatingWorldModel0");
    let elapsed = (t.t - DT as f64) as f32;
    let mut want = std::f32::consts::TAU * elapsed;
    while want > std::f32::consts::TAU {
        want -= std::f32::consts::TAU;
    }
    assert!((a.x - want).abs() < 1e-3 || (a.x - want).abs() > std::f32::consts::TAU - 1e-3, "pitch {} vs {want}", a.x);
    close3(t.pos("RotatingWorldModel0"), centre, 1e-3, "the centre stays");
}

/// Cases 10 and 11: HGOILRIG's drum: 8 + 8 damage kills it (at ≤ 1 HP),
/// its death command shows its effect, sets off `DExp02` and asks the
/// controller for supply items; it hides, not removed; a third bullet
/// passes. The round reset brings it back visible and solid but dead.
#[test]
fn t10_t11_hgoilrig_drum() {
    let Some(mount) = mount() else { return };
    let Some(data) = map(&mount, "hgoilrig") else { return };
    let mut t = T::of(&data);
    t.frame();
    t.w.fx.clear();
    // The round's setup: the controller's ObjectStartCommand.
    t.run("msg memorize trigger;");
    let id = t.id("BRK70Drum02");
    t.w.damage(id, 8.0, Who::Character(1), true);
    assert_eq!(t.w.objects[id as usize].hp.as_ref().unwrap().hp, 2.0);
    t.w.damage(id, 8.0, Who::Character(1), true);
    let o = &t.w.objects[id as usize];
    assert!(o.hp.as_ref().unwrap().dead);
    assert!(!o.visible && !o.solid && !o.rayhit && !o.removed);
    assert!(t.on("FX70Drum02"), "its effect on");
    let blast = t.w.fx.iter().find_map(|f| match f {
        Fx::Explode { at, radius, damage, .. } => Some((*at, *radius, *damage)),
        _ => None,
    });
    let (at, radius, damage) = blast.expect("DExp02 went off");
    close3(at, Vec3::new(7046.0, 368.0, -2409.235), 1e-2, "DExp02");
    assert_eq!((radius, damage), (200.0, 5.0));
    assert!(t.w.fx.iter().any(|f| matches!(f, Fx::ObjectMessage { name, message, .. }
        if name == "Level19GameControl" && message.eq_ignore_ascii_case("Randommapitem Spawner141_Box"))));
    t.w.damage(id, 8.0, Who::Character(1), true);
    // Case 11: the level start trigger's Visible_1, Solid_1, Remember.
    t.send("Level04StartTrigger", "TRIGGER", Who::Nobody);
    let o = &t.w.objects[id as usize];
    assert!(o.visible && o.solid, "back");
    assert!(o.hp.as_ref().unwrap().dead, "but dead: can't break again");
    // 9 damage on a fresh drum leaves 1 HP: dead.
    let mut t = T::of(&data);
    t.w.damage(id, 9.0, Who::Character(1), true);
    assert!(t.w.objects[id as usize].hp.as_ref().unwrap().dead);
}

/// Case 26: HGOILRIG has no decks: nothing solid below its 16 unlocked
/// `HungerGame` points but the sea; its locked points are all at one
/// place but one.
#[test]
fn t26_hgoilrig_has_no_decks() {
    let Some(mount) = mount() else { return };
    let Some(w) = world_file(&mount, "hgoilrig") else {
        eprintln!("skipping: hgoilrig not readable");
        return;
    };
    let physics = w.models.iter().find(|m| m.is_physics()).unwrap();
    let below = |p: Vec3| {
        (0..physics.polygons.len()).find(|&i| {
            // Solid faces, not the sky portals (the world's outer box).
            let flags = physics.polygon_surface(i).flags;
            if flags & surface_flags::SOLID == 0 || flags & surface_flags::SKY_PORTAL != 0 {
                return false;
            }
            let v: Vec<Vec3> = physics.polygon_vertices(i).map(Vec3::from).collect();
            // The vertical line through p crosses the polygon below p.
            let n = Vec3::from(physics.polygon_plane(i).normal);
            if n.y.abs() < 1e-4 {
                return false;
            }
            let d = physics.polygon_plane(i).dist;
            let y = (d - n.x * p.x - n.z * p.z) / n.y;
            if y > p.y || y < -10240.0 {
                return false;
            }
            let q = Vec3::new(p.x, y, p.z);
            (0..v.len()).all(|k| (v[(k + 1) % v.len()] - v[k]).cross(q - v[k]).dot(n) >= -1e-3)
        })
    };
    let points: Vec<(String, Vec3, bool)> = w
        .objects
        .iter()
        .filter(|o| o.class == "GameStartPoint" && o.string("GameType") == Some("HungerGame"))
        .map(|o| (o.name().unwrap_or("").to_string(), Vec3::from(o.pos().unwrap()), o.bool("Locked") == Some(true)))
        .collect();
    let unlocked: Vec<_> = points.iter().filter(|p| !p.2).collect();
    assert_eq!(unlocked.len(), 16);
    for (n, p, _) in &unlocked {
        if let Some(i) = below(*p) {
            let s = physics.polygon_surface(i);
            let v: Vec<Vec3> = physics.polygon_vertices(i).map(Vec3::from).collect();
            panic!(
                "{n} at {p} has a floor: polygon {i} flags {:#x} texture {:?} type {} corners {v:?}",
                s.flags,
                physics.textures.get(s.texture as usize),
                s.surface_type
            );
        }
    }
    let locked: Vec<_> = points.iter().filter(|p| p.2).collect();
    assert_eq!(locked.len(), 16);
    for (n, p, _) in &locked {
        if n == "GameStartPoint20" {
            close3(*p, Vec3::new(4709.924, 1558.125, 3076.678), 1e-2, n);
        } else {
            close3(*p, Vec3::new(4709.0, 1558.0, 3076.0), 1.0, n);
        }
    }
}

/// BLOODMONEY: the truck doors in front of `UNITSPAWN_ALPHA` (bots open
/// them: `AIActivate`): a hired unit following its hirer outside walks
/// into them and they open; they close again 2 s after.
#[test]
fn bloodmoney_unit_opens_the_truck_doors() {
    let Some(mount) = mount() else { return };
    let Some(data) = world::find(&mount, "bloodmoney").and_then(|_| world::load_from(&mount, "bloodmoney").ok()) else {
        eprintln!("skipping: bloodmoney not readable");
        return;
    };
    let s = movement_data().meters_per_unit;
    let mut sim = Sim::new(CaSim, MapSimPlugin::new(data));
    sim.ticks(2);
    with_mode(sim.app.world_mut(), "SQM");
    let a = sim.spawn_character(Vec3::new(0.0, 1.0, 0.0), movement::ID);
    sim.app.world_mut().entity_mut(a).insert(Team(1));
    // An opponent far away: an empty team ends the match after the
    // warm-up (game_modes.md correction 1).
    let b = sim.spawn_character(Vec3::new(0.0, 1.0, 60.0), movement::ID);
    sim.app.world_mut().entity_mut(b).insert(Team(2));
    // Past the warm-up to the first real round.
    for _ in 0..(40.0 / 0.015) as usize {
        let m = sim.app.world().resource::<combat_arms::game::rules::CaMatch>();
        if m.phase == combat_arms::game::rules::Phase::Live && !m.warmup {
            break;
        }
        sim.ticks(1);
    }
    // The hirer outside the truck, the doors between.
    let outside = to_engine(Vec3::new(1946.0, 60.0, -250.0), s);
    sim.app.world_mut().entity_mut(a).insert(Transform::from_translation(outside));
    if let Some(mut b) = sim.app.world_mut().get_mut::<chimera_core::core::Velocity>(a) {
        b.0 = Vec3::ZERO;
    }
    sim.app.world_mut().get_mut::<combat_arms::game::rules::hired_guns::CaBank>(a).unwrap().0 = 5000;
    let merc = mercs::hire(sim.app.world_mut(), a, 0).expect("hired");
    mercs::set_order(sim.app.world_mut(), a, MercOrder::FollowMe);
    let state = |sim: &Sim, name: &str| {
        let o = sim.app.world().resource::<CaObjects>();
        o.world.objects[o.named(name).unwrap() as usize].awm().unwrap().state
    };
    assert_eq!(state(&sim, "Door_L_BattXL01"), State::Off);
    let mut opened = false;
    for _ in 0..(15.0 / 0.015) as usize {
        sim.ticks(1);
        let (l, r) = (state(&sim, "Door_L_BattXL01"), state(&sim, "Door_R_BattXL01"));
        if l != State::Off || r != State::Off {
            opened = true;
        }
        if from_engine(sim.position(merc), s).z > -660.0 {
            break;
        }
    }
    let at = from_engine(sim.position(merc), s);
    eprintln!("bloodmoney: the unit at {at:.0} (doors opened: {opened})");
    assert!(opened, "the unit opened the doors");
    assert!(at.z > -680.0, "the unit came out past the doors (z -687): {at:.0}");
}

/// game_modes_2.md case 23 on CITYOFWINGS: the VIP's cell. Holding use
/// on `AT_VIP` for its 13 s opens `VIP_Door` and locks both use
/// triggers; the round's end trigger closes it and unlocks them; from
/// outside `AT_VIP_Door` takes 1 s. A closed door stops a player.
#[test]
fn cityofwings_vip_cell() {
    let Some(mount) = mount() else { return };
    let Some(data) = world::find(&mount, "cityofwings").and_then(|_| world::load_from(&mount, "cityofwings").ok()) else {
        eprintln!("skipping: cityofwings not readable");
        return;
    };
    let s = movement_data().meters_per_unit;
    let mut sim = Sim::new(CaSim, MapSimPlugin::new(data));
    sim.ticks(3);
    let objects = |sim: &Sim| -> (State, bool, bool) {
        let o = sim.app.world().resource::<CaObjects>();
        let door = o.world.objects[o.named("VIP_Door").unwrap() as usize].awm().unwrap().state;
        let locked = |n: &str| match &o.world.objects[o.named(n).unwrap() as usize].kind {
            Kind::Use(u) => u.locked,
            _ => panic!("{n}"),
        };
        (door, locked("AT_VIP"), locked("AT_VIP_Door"))
    };
    assert_eq!(objects(&sim), (State::Off, false, false));
    // The VIP inside, at its trigger, holding use.
    let at = |name: &str, sim: &Sim| {
        let o = sim.app.world().resource::<CaObjects>();
        o.world.objects[o.named(name).unwrap() as usize].pose.pos
    };
    let inside = at("AT_VIP", &sim);
    let vip = sim.spawn_character(to_engine(inside, s), movement::ID);
    sim.seconds(0.5);
    sim.intent(vip).use_key = true;
    sim.seconds(12.5);
    assert_eq!(objects(&sim).0, State::Off, "not before 13 s");
    sim.seconds(1.0);
    assert_eq!(objects(&sim), (State::PowerOn, true, true), "open, both locked");
    sim.seconds(1.5);
    assert_eq!(objects(&sim).0, State::On);
    sim.intent(vip).use_key = false;
    // The round ends: its end trigger.
    sim.app.world_mut().write_message(CaObjectMessage {
        target: "Level12EndTrigger".into(),
        message: "trigger".into(),
    });
    sim.seconds(1.5);
    assert_eq!(objects(&sim), (State::Off, false, false), "closed, unlocked");
    // From outside: 1 s.
    let outside = at("AT_VIP_Door", &sim);
    let escort = sim.spawn_character(to_engine(outside, s), movement::ID);
    sim.seconds(0.5);
    sim.intent(escort).use_key = true;
    sim.seconds(1.2);
    assert_eq!(objects(&sim).0, State::PowerOn, "1 s from outside");
    let _ = (Health::default(), Intent::default());
}

/// JUNK_FLEA2's delivery (`Initiaion_TRG01`, fired by the mode): the kill
/// boxes start off; 7 s after the trigger the box's collision turns
/// solid, `Box_Kill01` kills for half a second and `Sub_Box_Kill01` stays
/// on under the solid box. A player at the playtest's spot beside it
/// lives through it all.
#[test]
fn junk_flea2_delivery_sequence() {
    let Some(mount) = mount() else { return };
    let Some(data) = map(&mount, "junk_flea2") else { return };
    let mut t = T::of(&data);
    let hidden = |t: &T, n: &str| !t.w.objects[t.id(n) as usize].visible;
    t.frame();
    assert!(hidden(&t, "Box_Kill01") && hidden(&t, "Sub_Box_Kill01"), "off from the start");
    assert!(!t.w.objects[t.id("Collision_Box01") as usize].solid, "the box's collision off");
    t.send("Initiaion_TRG01", "TRIGGER", Who::Nobody);
    let start = t.t;
    let mut on_for = 0.0;
    let mut solid_at = None;
    for _ in 0..(30.0 / DT) as usize {
        t.frame();
        let now = t.t - DT as f64 - start;
        if !hidden(&t, "Box_Kill01") {
            on_for += DT as f64;
            // (The frame the trigger came in counts toward its delays.)
            assert!(now >= 6.94 && now < 7.55, "Box_Kill01 on at {now}");
        }
        if solid_at.is_none() && t.w.objects[t.id("Collision_Box01") as usize].solid {
            solid_at = Some(now);
        }
    }
    assert!((on_for - 0.5).abs() < 0.06, "on for half a second: {on_for}");
    assert!(solid_at.is_some_and(|s| (s - 7.0).abs() < 0.11), "{solid_at:?}");
    assert!(!hidden(&t, "Sub_Box_Kill01"), "under the box from 7 s");
    assert!(t.w.objects[t.id("Collision_MDL01") as usize].solid, "the model's collision from 20 s");
}

/// SHORTFUSE's sliding doors (`Door_OUT_*01`, `Door_IN_*01`) are drawn as
/// doors: the `Filter` volumes beside them store `Visible` as the real 0,
/// which was read as "no property" and drew each volume's box with its
/// empty texture name as one flat slab (and its faces over the doors).
/// Nothing in the converted map is drawn flat, each door's node carries
/// its door-texture mesh, and the OUT leaves slide apart 72 u on a
/// trigger.
#[test]
fn shortfuse_doors_are_drawn_and_open() {
    let Some(mount) = mount() else { return };
    let Some(data) = map(&mount, "shortfuse") else { return };
    let flat: Vec<_> = data.meshes.iter().filter(|m| m.material.is_empty()).collect();
    assert!(flat.is_empty(), "{} flat meshes drawn (volumes' boxes)", flat.len());
    let mut t = T::of(&data);
    for n in ["Door_OUT_L01", "Door_OUT_R01", "Door_IN_L01", "Door_IN_R01"] {
        let id = t.id(n) as usize;
        let meshes: Vec<_> = data.meshes.iter().filter(|m| m.entity == Some(id)).collect();
        assert!(
            !meshes.is_empty() && meshes.iter().all(|m| m.material.to_lowercase().contains("autodoor") && m.texture.is_some()),
            "{n}: drawn with the door texture"
        );
    }
    let (l, r) = (t.pos("Door_OUT_L01"), t.pos("Door_OUT_R01"));
    t.frame();
    t.send("Door_OUT_L01", "TRIGGER", Who::Nobody);
    t.send("Door_OUT_R01", "TRIGGER", Who::Nobody);
    t.frames(30);
    assert_eq!(t.state("Door_OUT_L01"), State::On);
    let (l2, r2) = (t.pos("Door_OUT_L01"), t.pos("Door_OUT_R01"));
    close3(l2 - l, Vec3::new(0.0, 0.0, -72.0), 0.05, "left leaf");
    close3(r2 - r, Vec3::new(0.0, 0.0, 72.0), 0.05, "right leaf");
}

/// SHORTFUSE's dumpster lids (`RotatingDoor00`, `02`, ...) are not
/// activatable themselves (world_objects.md 7: only an object with
/// `PlayerActivate` takes the use key); each is opened through the
/// small panel beside it (`RotatingDoor01`, `03`, ...: `PlayerActivate`,
/// `OpenCommand` "msg RotatingDoor00 on"). The use ray from the eye hits
/// the panel's front faces and opens its lid; aimed at the lid it does
/// nothing, and a second press closes it. The panel is a thin frustum
/// without caps (its bevels and sides are what a ray meets).
#[test]
fn shortfuse_lid_opens_through_its_panel() {
    let Some(mount) = mount() else { return };
    let Some(data) = map(&mount, "shortfuse") else { return };
    let s = movement_data().meters_per_unit;
    let mut sim = Sim::new(CaSim, MapSimPlugin::new(data));
    sim.ticks(3);
    with_mode(sim.app.world_mut(), "QRT");
    sim.seconds(2.0);
    let (panel, lid) = {
        let o = sim.app.world().resource::<CaObjects>();
        let a = |n: &str| o.world.objects[o.named(n).unwrap() as usize].awm().unwrap().activatable();
        assert!(a("RotatingDoor01") && !a("RotatingDoor00"), "the panel takes the use key, the lid doesn't");
        (o.named("RotatingDoor01").unwrap(), o.named("RotatingDoor00").unwrap())
    };
    let state = |sim: &Sim, id: u32| {
        let o = sim.app.world().resource::<CaObjects>();
        o.world.objects[id as usize].awm().unwrap().state
    };
    // Stand east of the panel, the use ray at the middle of its bevel
    // (file frame (458.04, -4056.98, 587.0)).
    let press = |sim: &mut Sim, at: Vec3| {
        let me = sim.spawn_character(to_engine(Vec3::new(494.0, -4037.5, 587.0), s), movement::ID);
        sim.seconds(0.3);
        let w = sim.app.world();
        let eye = w.get::<Transform>(me).unwrap().translation + w.get::<chimera_core::core::MovementState>(me).unwrap().eye_offset;
        let d = (to_engine(at, s) - eye).normalize();
        {
            let mut i = sim.intent(me);
            i.yaw = (-d.x).atan2(-d.z);
            i.pitch = d.y.asin();
        }
        sim.seconds(0.2);
        sim.intent(me).use_key = true;
        sim.seconds(0.3);
        sim.intent(me).use_key = false;
        sim.seconds(0.3);
        sim.app.world_mut().despawn(me);
    };
    // The middle of the lid: nothing.
    let lid_at = sim.app.world().resource::<CaObjects>().world.objects[lid as usize].pose.pos;
    press(&mut sim, lid_at);
    assert_eq!((state(&sim, panel), state(&sim, lid)), (State::Off, State::Off), "the lid itself isn't used");
    press(&mut sim, Vec3::new(458.04, -4056.98, 587.0));
    assert_eq!(state(&sim, panel), State::On, "the panel took the use key");
    assert_ne!(state(&sim, lid), State::Off, "its lid opens");
    sim.seconds(1.5);
    assert_eq!(state(&sim, lid), State::On);
    press(&mut sim, Vec3::new(458.04, -4056.98, 587.0));
    sim.seconds(1.5);
    assert_eq!((state(&sim, panel), state(&sim, lid)), (State::Off, State::Off), "a second press closes it");
}


/// 14 on the stock maps: every active world model of FREEZER, SNOWSTORM,
/// HGOILRIG and BLOODMONEY that names an opening (power-on) sound starts
/// it when triggered from rest, as the object's own sound, attached when
/// its `SoundPos` is (0, 0, 0), radius `SoundRadius` and inner a quarter
/// of it; none of them loops (no stock map sets `LoopSounds`), so no
/// transition stops one.
#[test]
fn stock_transition_sounds() {
    let Some(mount) = mount() else { return };
    let mut seen = 0;
    for name in ["freezer", "snowstorm", "hgoilrig", "bloodmoney"] {
        let Some(data) = map(&mount, name) else { continue };
        let mut t = T::of(&data);
        t.frame();
        let ids: Vec<u32> = (0..t.w.objects.len() as u32)
            .filter(|&i| t.w.objects[i as usize].awm().is_some_and(|a| a.state == State::Off && !a.locked && !a.sounds[0].is_empty()))
            .collect();
        for id in ids {
            let (file, pos, radius) = {
                let a = t.w.objects[id as usize].awm().unwrap();
                (a.sounds[0].clone(), a.sound_pos, a.sound_radius)
            };
            t.w.now = t.t;
            t.w.deliver(id, "TRIGGER", Ctx::default());
            let o = &t.w.objects[id as usize];
            let s = o.sound.clone().unwrap_or_else(|| panic!("{name} {}: no sound", o.name));
            assert_eq!(s.file, file, "{name} {}", o.name);
            assert!(!s.looping, "{name} {}", o.name);
            assert_eq!(s.attach, (pos == Vec3::ZERO).then_some(id), "{name} {}", o.name);
            assert_eq!((s.radius, s.inner), (radius, radius * 0.25), "{name} {}", o.name);
            seen += 1;
        }
    }
    if seen > 0 {
        assert!(seen >= 20, "{seen} sounding world models");
    }
}

/// 14: HGOILRIG's eight `SND_Water*` sounds (looping, radius 3000, inner
/// 250, placed in 3D, `PlayAttached`) ride their keyframers from the sea
/// edge toward the rig once `CMD_SNDWaterON` fires; in play each has an
/// anchor at its object, which the sound follows.
#[test]
fn hgoilrig_water_sounds_follow_their_keyframers() {
    use combat_arms::game::{
        music::ambient::SoundFx,
        objects::sound::{CaObjectSounds, ObjectSoundAnchor},
    };
    let Some(mount) = mount() else { return };
    let Some(data) = map(&mount, "hgoilrig") else { return };
    let mut t = T::of(&data);
    t.frame();
    for c in ["A", "B", "C", "D", "E", "F", "G", "H"] {
        let o = &t.w.objects[t.id(&format!("SND_Water{c}")) as usize];
        let f = SoundFx::from_props(0, &o.name, &o.props, o.pose.pos, true, 0);
        assert!(f.looping && f.attached && !f.ambient, "SND_Water{c}");
        assert_eq!((f.outer, f.inner), (3000.0, 250.0), "SND_Water{c}");
    }
    let start = t.pos("SND_WaterA");
    let kf = t.id("KeyFramerA");
    close3(start, k_pos(&t, kf, 0), 1.0, "at the sea edge");
    t.send("CMD_SNDWaterON", "TRIGGER", Who::Nobody);
    t.frames((45.0 / DT) as usize);
    let half = t.pos("SND_WaterA");
    assert!(half.y > -14.0 + 100.0 && half.y < 946.0, "rising toward the rig: {half}");
    t.frames((50.0 / DT) as usize);
    close3(t.pos("SND_WaterA"), k_pos(&t, kf, 1), 1.0, "at the rig after 90 s");
    // In play: the anchor follows.
    let mut sim = Sim::new(CaSim, MapSimPlugin::new(data));
    sim.ticks(10);
    sim.app.world_mut().write_message(CaObjectMessage {
        target: "CMD_SNDWaterON".into(),
        message: "trigger".into(),
    });
    sim.seconds(20.0);
    let w = sim.app.world_mut();
    let id = w.resource::<CaObjects>().named("SND_WaterA").unwrap();
    let e = *w.resource::<CaObjectSounds>().anchors.get(&id).expect("an anchor");
    assert_eq!(w.get::<ObjectSoundAnchor>(e), Some(&ObjectSoundAnchor(id)));
    let o = w.resource::<CaObjects>();
    let pos = o.world.objects[id as usize].pose.pos;
    assert!(pos.distance(start) > 100.0, "moved: {pos}");
    let at = w.get::<Transform>(e).unwrap().translation;
    assert!(at.distance(to_engine(pos, o.scale)) < 0.05, "the anchor at the sound");
}


/// world_dat.md "Light groups" and its case 12 on the stock maps: each
/// group a `LightGroup` object switches is a light style of the converted
/// map that starts as the object says (SNOWSTORM's two off; GRAVITYB's
/// `Light_Grv_01` on, `Light_Grv_02` off; RURALESTATE's fridge light
/// off) and lights texels; the object carries its style. RURALESTATE's
/// fridge door switches its light on as it opens and off once closed
/// (its `OpeningCommand` and `ClosedCommand`).
#[test]
fn stock_light_groups() {
    use combat_arms::game::objects::{lights, runtime::keys};
    let Some(mount) = mount() else { return };
    for (name, want) in [
        ("snowstorm", &[("LightGroup_H01", false), ("LightGroup_H02", false)][..]),
        ("gravityb", &[("Light_Grv_01", true), ("Light_Grv_02", false)][..]),
        ("ruralestate", &[("LightFridge01", false)][..]),
    ] {
        let Some(data) = map(&mount, name) else { continue };
        let l = data.lightmap.as_ref().expect("a lightmap");
        assert_eq!(l.styles.len(), want.len(), "{name}");
        for (group, on) in want {
            let e = data
                .entities
                .iter()
                .find(|e| e.classname() == "LightGroup" && e.get("targetname") == Some(*group))
                .unwrap_or_else(|| panic!("{name}: {group}"));
            let style: u8 = e.get(keys::LIGHT_STYLE).expect("its style").parse().unwrap();
            let s = l.styles.iter().find(|s| s.style == style).expect("the style");
            assert_eq!(s.on, *on, "{name} {group}");
            assert!(!s.texels.is_empty(), "{name} {group} lights something");
        }
        if name != "ruralestate" {
            continue;
        }
        let mut t = T::of(&data);
        t.frame();
        let style = |t: &T| lights::wanted(&t.w);
        let fridge: u8 = l.styles[0].style;
        assert_eq!(style(&t), [(fridge, false)]);
        t.send("RotatingDoor0", "ACTIVATE", Who::Character(1));
        assert_eq!(style(&t), [(fridge, true)], "on as the door opens");
        t.frames(30);
        t.send("RotatingDoor0", "ACTIVATE", Who::Character(1));
        t.frames(30);
        assert_eq!(t.state("RotatingDoor0"), State::Off);
        assert_eq!(style(&t), [(fridge, false)], "off once it is closed");
    }
}

/// world_objects.md 10 (Client) on SHORTFUSE: a client standing where
/// `Door_OUT_L01` closes. Its prediction steps the leaf as the server's
/// frame does: the client's leaf, at the tick the client predicts, is
/// where the server's is at that tick (after the round trip that tells
/// it of the start), the leaf stops at the player on both (6: never
/// pushes) and the player standing there is never mispredicted.
#[test]
fn shortfuse_door_closing_on_a_client_is_predicted() {
    use crate::object_carry::{assert_agrees, graph, link, put, ride, send};
    use chimera_net::{harness::NetSim, net::predict::NetGraph};
    let Some(mount) = mount() else { return };
    let Some(data) = map(&mount, "shortfuse") else { return };
    let s = movement_data().meters_per_unit;
    let (closed, half) = {
        let t = T::of(&data);
        let o = &t.w.objects[t.id("Door_OUT_L01") as usize];
        (o.pose.pos, o.pose.half)
    };
    let leaf = |app: &App| {
        let o = app.world().resource::<CaObjects>();
        let n = *o.nodes.get(&o.named("Door_OUT_L01").unwrap()).unwrap();
        from_engine(app.world().get::<Transform>(n).unwrap().translation, s)
    };
    for (latency, seed) in [(50, 81), (150, 82)] {
        let d = data.clone();
        // Two clients, on two teams.
        let mut sim = NetSim::new(CaSim, link(latency, 0), seed, 2, move |app| {
            app.add_plugins(MapSimPlugin::new(d.clone()));
        });
        sim.set_frame(combat_arms::game::TICK_INTERVAL);
        sim.until_joined(1200);
        // (Kept short: a player left in this doorway for long is killed by
        // an endless-fall volume's lasting damage.)
        sim.ticks(60);
        send(&mut sim, "Door_OUT_L01", "trigger");
        send(&mut sim, "Door_OUT_R01", "trigger");
        sim.ticks(120);
        let open = leaf(&sim.server.app);
        assert!(open.distance(closed) > 60.0, "the leaf opened: {open:.1} (closed {closed:.1})");
        // Our player on the floor near the far end of where the leaf
        // closes (it travels some 50 u to it).
        put(&mut sim, Vec3::new(closed.x, closed.y - half.y + 60.0, closed.z + 30.0), s);
        sim.ticks(80);
        *sim.clients[0].app.world_mut().resource_mut::<NetGraph>() = NetGraph::default();
        let theirs = sim.character_of(0).unwrap();
        let before = sim.server.app.world().get::<Transform>(theirs).unwrap().translation;
        send(&mut sim, "Door_OUT_L01", "trigger");
        let window = ((2 * latency) as f64 / 1000.0 / combat_arms::game::TICK_INTERVAL) as u32 + 8;
        let r = ride(&mut sim, "Door_OUT_L01", 600, window);
        let (server_leaf, client_leaf) = (leaf(&sim.server.app), leaf(&sim.clients[0].app));
        let after = sim.server.app.world().get::<Transform>(theirs).unwrap().translation;
        let g = graph(&sim);
        eprintln!(
            "shortfuse {latency} ms: leaf stopped at {server_leaf:.1} (client {client_leaf:.1}; open {open:.1}, closed \
             {closed:.1}); the client's leaf off the server's by {:.4} m at the same tick after the start; player \
             moved {:.4} m; errors at {:?}, worst gap {:.4} m, {} of {} states wrong",
            r.node_gap_after,
            after.distance(before),
            r.errors,
            r.worst_gap,
            g.errors,
            g.checked
        );
        let what = format!("shortfuse {latency} ms");
        assert!(server_leaf.distance(closed) > 10.0, "{what}: the leaf stopped at the player");
        assert!(server_leaf.distance(open) > 10.0, "{what}: the leaf moved toward the player");
        assert!(after.distance(before) < 1e-3, "{what}: the player wasn't pushed");
        let hp = sim.server.app.world().get::<Health>(theirs).map_or(0.0, |h| h.current);
        assert!(hp > 0.0, "{what}: the player is alive (a dead one doesn't block)");
        assert!(client_leaf.distance(server_leaf) < 0.01, "{what}: the client's leaf is the server's");
        assert!(r.node_gap_after < 1e-4, "{what}: the client's leaf at its tick is {} m off", r.node_gap_after);
        assert_eq!(g.errors, 0, "{what}: standing in a closing door isn't mispredicted");
        assert_agrees(&mut sim, &what);
    }
}

/// world_objects.md 10 (Client) and test case 8 on HGOILRIG: a client
/// swimming in the `Water` volume while the sea rises (`SlidingWater`
/// on: its `OpeningCommand` also starts the eight water-sound keyframers).
/// The client steps the sea, which carries its volumes, and the
/// keyframers as the server does: the swimmer rises with the water and
/// isn't corrected after the start's round trip, and the client's sea and
/// water sounds are the server's at the same tick.
#[test]
fn hgoilrig_rising_sea_with_a_client_swimming_is_predicted() {
    use crate::object_carry::{assert_agrees, graph, link, put, send};
    use chimera_core::core::{SimClock, SimTick};
    use chimera_net::{harness::NetSim, net::predict::NetGraph};
    let Some(mount) = mount() else { return };
    let Some(data) = map(&mount, "hgoilrig") else { return };
    let s = movement_data().meters_per_unit;
    let sea = T::of(&data).pos("SlidingWater");
    let latency = 100;
    let mut sim = NetSim::new(CaSim, link(latency, 0), 83, 1, move |app| {
        app.add_plugins(MapSimPlugin::new(data.clone()));
    });
    sim.set_frame(combat_arms::game::TICK_INTERVAL);
    sim.until_joined(1200);
    sim.ticks(60);
    // In the water (its top at y -58 at load), over the sea's centre.
    put(&mut sim, Vec3::new(sea.x, -120.0, sea.z), s);
    sim.ticks(400);
    *sim.clients[0].app.world_mut().resource_mut::<NetGraph>() = NetGraph::default();
    let theirs = sim.character_of(0).unwrap();
    let feet = |sim: &NetSim| from_engine(sim.server.app.world().get::<Transform>(theirs).unwrap().translation, s).y;
    let y0 = feet(&sim);
    // The objects' poses by tick: the sea and `SND_WaterA`.
    let poses = |app: &App| {
        let o = app.world().resource::<CaObjects>();
        let p = |n: &str| o.world.objects[o.named(n).unwrap() as usize].pose.pos;
        (p("SlidingWater"), p("SND_WaterA"))
    };
    let window = ((2 * latency) as f64 / 1000.0 / combat_arms::game::TICK_INTERVAL) as u32 + 8;
    send(&mut sim, "SlidingWater", "on");
    let mut server: BTreeMap<u64, (Vec3, Vec3)> = BTreeMap::new();
    let mut client: Vec<(u64, (Vec3, Vec3))> = Vec::new();
    let mut errors_after = 0;
    let mut last = 0;
    // 125 s: the sea's 120 s rise (its sounds' 90 s ride within it).
    let ticks = (125.0 / combat_arms::game::TICK_INTERVAL) as u32;
    for k in 0..ticks {
        sim.step();
        let e = graph(&sim).errors;
        if e > last && k > window {
            errors_after += e - last;
        }
        last = e;
        server.insert(sim.server.app.world().resource::<SimTick>().0, poses(&sim.server.app));
        if k > window {
            let c = &sim.clients[0].app;
            client.push((c.world().resource::<SimClock>().tick, poses(c)));
        }
    }
    let (mut sea_gap, mut sound_gap, mut compared) = (0.0f32, 0.0f32, 0);
    for (t, (a, b)) in &client {
        if let Some((sa, sb)) = server.get(t) {
            sea_gap = sea_gap.max(a.distance(*sa));
            sound_gap = sound_gap.max(b.distance(*sb));
            compared += 1;
        }
    }
    let (risen, y1) = (poses(&sim.server.app).0.y - sea.y, feet(&sim));
    let g = graph(&sim);
    eprintln!(
        "hgoilrig {latency} ms: the sea rose {risen:.1} u, the swimmer {y0:.1} -> {y1:.1} u; {errors_after} errors after \
         the start's round trip, {} of {} states wrong, worst correction {:.4} m; the client's sea off the server's \
         by {sea_gap:.4} u and SND_WaterA by {sound_gap:.4} u at the same tick ({compared} ticks)",
        g.errors, g.checked, g.worst_error
    );
    assert!((risen - 500.0).abs() < 1.0, "the sea rose 500 u: {risen}");
    assert!(y1 > y0 + 300.0 && y1 < 442.0, "the swimmer rose with the water, in it: {y0} -> {y1}");
    assert!(compared > 1000, "compared {compared} ticks");
    assert!(sea_gap < 1e-3, "the client's sea is the server's: {sea_gap} u off");
    assert!(sound_gap < 1e-3, "the client's water sound is the server's: {sound_gap} u off");
    assert_eq!(errors_after, 0, "the swimmer isn't corrected after the start");
    assert_agrees(&mut sim, "hgoilrig");
}
