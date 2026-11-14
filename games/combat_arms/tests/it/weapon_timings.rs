//! Each install weapon fires, reloads and draws on its own view model's
//! animations (docs/backlog.md, "Combat Arms guns' own sounds, scopes and
//! models: audit", fix 3; docs/plans/active/ca-weapon-timings.md): copies
//! made from an install written here, their view models' lengths set in
//! the test (`weapons::timing::ViewTimes`), on the server and predicted;
//! ours keep their spec numbers; `FireAniRate` reaches every copy; a
//! client that can't read the models, or reads others, predicts on the
//! server's timing (`net::weapon_params`); our M92FS's own post-fire.

use std::time::Duration;

use bevy::prelude::*;
use chimera_core::core::{Health, Intent, SimClock};
use chimera_net::{
    harness::NetSim,
    net::{memory::LinkConditions, predict::NetGraph, weapon_params::ParamsHeard},
};
use chimera_sim::weapon::{self, Inventory, Magazine, ShellReload, Trigger, Weapon, data::WeaponDataAppExt};
use chimera_world::{greybox::GreyboxMapPlugin, harness::Sim};
use combat_arms::{
    game::{
        TICK_INTERVAL,
        attachments::CaAttachSlots,
        data::{attributes::WEAPONS_FILE, merge_weapon_docs, read_attributes},
        movement,
        weapons::timing::{AnimTimes, TIMING, ViewTimes},
    },
    harness::CaSim,
};
use toml::{Table, Value};

use super::attribute_data::meters_per_unit;

const INSTALL: &str = r#"
[Ammo0] Name="556" Type=1 MaxAmount=120
[Ammo1] Name="12ga" Type=1 MaxAmount=35
[Ammo2] Name="9mm" Type=1 MaxAmount=120
[Weapon0] Name="M4A1" Guntype=4 AmmoName0="556" ShotsPerClip=30 PVModelNormal="GUNS_M_PV_AR\PV_AR_M4A1.LTB"
[Weapon1] Name="Remington_870" Guntype=2 AmmoName0="12ga" ShotsPerClip=7 ReloadPerBullet=1 RapidFireCount=1
    PVModelNormal="GUNS_M_PV_SG\PV_SG_R870.LTB"
[Weapon2] Name="MP5A4" Guntype=3 AmmoName0="9mm" ShotsPerClip=30 PVModelNormal="GUNS_M_PV_SMG\PV_SMG_MP5.LTB"
[Weapon3] Name="Timing Rifle" Guntype=4 AmmoName0="556" ShotsPerClip=30 RapidFireCount=0 FireAniRate=0.25
    PVModelNormal="TEST\Timing\PV_Rifle.LTB"
[Weapon4] Name="Timing Shotgun" Guntype=2 AmmoName0="12ga" ShotsPerClip=7 ReloadPerBullet=1 RapidFireCount=1
    PVModelNormal="TEST\Timing\PV_Shotgun.LTB"
[Weapon5] Name="Timing SMG" Guntype=3 AmmoName0="9mm" ShotsPerClip=30 FireAniRate=0.5
    PVModelNormal="TEST\Timing\PV_SMG.LTB"
[Weapon6] Name="Beretta_M92FS" Guntype=1 AmmoName0="9mm" ShotsPerClip=15 RapidFireCount=1
    PVModelNormal="GUNS_M_PV_Pistol\PV_Pistol_M92FS.LTB"
[Weapon7] Name="M60" Guntype=5 AmmoName0="762" ShotsPerClip=100 PVModelNormal="GUNS_M_PV_MG\PV_MG_M60_SH.LTB"
[Weapon8] Name="M9" Guntype=0 AmmoName0="knife" PVModelNormal="GUNS_M_PV_MELEE\PV_Melee_M9_SH.ltb"
[Weapon9] Name="Timing Pistol" Guntype=1 AmmoName0="9mm" ShotsPerClip=15 RapidFireCount=1
    PVModelNormal="TEST\Timing\PV_Pistol.LTB"
[Weapon10] Name="Timing Minigun" Guntype=5 AmmoName0="762" ShotsPerClip=500 AccumPreFire=1 RapidFireCount=0
    PVModelNormal="TEST\Timing\PV_Minigun.LTB"
[Weapon11] Name="Timing Knife" Guntype=0 AmmoName0="knife" PVModelNormal="TEST\Timing\PV_Knife.LTB"
[Weapon12] Name="Timing Rifle Long" Guntype=4 AmmoName0="556" ShotsPerClip=30 RapidFireCount=0
    PVModelNormal="TEST\Timing\PV_Rifle_Long.LTB"
[Ammo3] Name="762" Type=1 MaxAmount=1000
[Ammo4] Name="knife" Type=1 MaxAmount=1000 InstDamage=45 AltInstDamage=75
"#;

const RIFLE: &str = "combat_arms:timing_rifle";
const SHOTGUN: &str = "combat_arms:timing_shotgun";
const SMG: &str = "combat_arms:timing_smg";
const PISTOL: &str = "combat_arms:timing_pistol";
const MINIGUN: &str = "combat_arms:timing_minigun";
const KNIFE: &str = "combat_arms:timing_knife";
const RIFLE_LONG: &str = "combat_arms:timing_rifle_long";

/// The copies' view models, as the test writes them (s at rate 1).
fn models(app: &mut App) {
    let mut times = app.world_mut().get_resource_or_init::<ViewTimes>();
    times.set("test/timing/pv_rifle.ltb", AnimTimes {
        fire: Some(0.2),
        reload: Some(1.0),
        select: Some(0.5),
        // A 0 ms pre-fire, as the M4A1's: the update it takes stays.
        has_prefire: true,
        ..default()
    });
    // A 200 ms pre-fire (weapon_classes.md case 44).
    times.set("test/timing/pv_rifle_long.ltb", AnimTimes {
        fire: Some(0.2),
        reload: Some(1.0),
        select: Some(0.5),
        has_prefire: true,
        prefire: Some(0.2),
        ..default()
    });
    times.set("test/timing/pv_shotgun.ltb", AnimTimes {
        fire: Some(0.9),
        reload: Some(0.4),
        pre_reload: Some(0.25),
        post_reload: Some(0.6),
        select: Some(0.8),
        ..default()
    });
    // Semi-automatic, no pre-fire in its file: a 200 ms shot, a 300 ms
    // post-fire.
    times.set("test/timing/pv_pistol.ltb", AnimTimes {
        fire: Some(0.2),
        reload: Some(1.2),
        select: Some(0.5),
        postfire: Some(0.3),
        ..default()
    });
    // The spin of weapon_classes.md 3.3 at other lengths: a 1 s spin-up,
    // a 0.8 s spin-down, a 200 ms loop with keys at 0, 40, ..., 200 (a
    // round each, the one at the end the next loop's).
    times.set("test/timing/pv_minigun.ltb", AnimTimes {
        fire: Some(0.2),
        reload: Some(3.0),
        has_prefire: true,
        prefire: Some(1.0),
        postfire: Some(0.8),
        fire_keys: vec![0.0, 0.04, 0.08, 0.12, 0.16, 0.2],
        ..default()
    });
    // A slash of 500 ms hitting at 100 and 300 ms, a stab of 900 ms
    // hitting at 450 ms.
    times.set("test/timing/pv_knife.ltb", AnimTimes {
        fire: Some(0.5),
        alt_fire: Some(0.9),
        select: Some(0.3),
        fire_keys: vec![0.1, 0.3],
        alt_fire_keys: vec![0.45],
        postfire: Some(0.001),
        ..default()
    });
    // No select: the template's draw stays.
    times.set("test/timing/pv_smg.ltb", AnimTimes {
        fire: Some(0.12),
        reload: Some(1.5),
        ..default()
    });
}

fn merged() -> Vec<(String, Table)> {
    let (attrs, problems) = read_attributes(|name| match name {
        WEAPONS_FILE => Ok(INSTALL.as_bytes().to_vec()),
        _ => Ok(Vec::new()),
    });
    let weapons = attrs.weapons.as_ref().unwrap_or_else(|| panic!("{problems:?}"));
    let ours = [
        ("m4a1.toml", include_str!("../../data/weapons/m4a1.toml")),
        ("remington_870.toml", include_str!("../../data/weapons/remington_870.toml")),
        ("mp5a4.toml", include_str!("../../data/weapons/mp5a4.toml")),
        ("beretta_m92fs.toml", include_str!("../../data/weapons/beretta_m92fs.toml")),
        ("m60.toml", include_str!("../../data/weapons/m60.toml")),
        ("m9.toml", include_str!("../../data/weapons/m9.toml")),
    ];
    merge_weapon_docs(&ours, weapons, meters_per_unit())
        .unwrap()
        .into_iter()
        .map(|(n, t)| (n, t.parse().unwrap()))
        .collect()
}

fn doc<'a>(docs: &'a [(String, Table)], id: &str) -> &'a Table {
    &docs
        .iter()
        .find(|(_, t)| t.get("id").and_then(Value::as_str) == Some(id))
        .unwrap_or_else(|| panic!("{id}"))
        .1
}

fn hooks(t: &Table) -> &Table {
    t["hooks"].as_table().unwrap()
}

/// The copies' documents as text, to register.
fn copies() -> Vec<(String, String)> {
    let docs = merged();
    [RIFLE, SHOTGUN, SMG, PISTOL, MINIGUN, KNIFE, RIFLE_LONG]
        .iter()
        .map(|id| (id.to_string(), toml::to_string(doc(&docs, id)).unwrap()))
        .collect()
}

fn register(app: &mut App, copies: &[(String, String)]) {
    models(app);
    let refs: Vec<(&str, &str)> = copies.iter().map(|(n, t)| (n.as_str(), t.as_str())).collect();
    app.add_weapon_data(&refs).unwrap();
}

/// The merge writes the timing table on the copies of guns only, never
/// on ours, and `FireAniRate` on every weapon, a template without an
/// inventory table (the MP5A4) included.
#[test]
fn copies_get_a_timing_table_and_their_fire_rate() {
    let docs = merged();
    for id in [RIFLE, SHOTGUN, SMG, PISTOL, MINIGUN, KNIFE] {
        assert!(hooks(doc(&docs, id)).contains_key(TIMING), "{id}");
    }
    // `AccumPreFire`: a spin table, its lengths left to the model.
    assert!(hooks(doc(&docs, MINIGUN)).contains_key("combat_arms:spin"));
    assert!(!hooks(doc(&docs, "combat_arms:m60")).contains_key("combat_arms:spin"));
    for id in ["combat_arms:m4a1", "combat_arms:remington_870", "combat_arms:mp5a4"] {
        assert!(!hooks(doc(&docs, id)).contains_key(TIMING), "ours: {id}");
    }
    let rate = |id: &str| {
        hooks(doc(&docs, id))
            .get("combat_arms:inventory")
            .and_then(|t| t.get("fire_rate"))
            .and_then(Value::as_float)
    };
    assert_eq!(rate(RIFLE), Some(0.25));
    assert_eq!(rate(SMG), Some(0.5), "made where the MP5A4 has no inventory table");
    assert_eq!(rate("combat_arms:mp5a4"), Some(0.0));
}

fn sim() -> (Sim, Entity) {
    let copies = copies();
    let mut sim = Sim::with(CaSim, move |app| {
        app.add_plugins(GreyboxMapPlugin);
        register(app, &copies);
    });
    sim.set_tick_interval(TICK_INTERVAL);
    let p = sim.spawn_character(Vec3::new(30.0, 1.0, 30.0), movement::ID);
    sim.seconds(0.5);
    sim.app.world_mut().entity_mut(p).insert(Health {
        current: 1.0,
        max: 1.0,
    });
    (sim, p)
}

fn now(sim: &Sim) -> f64 {
    sim.app.world().resource::<SimClock>().now
}

/// The times the clip went down while fire was held for `seconds`.
fn shots(sim: &mut Sim, p: Entity, w: Entity, seconds: f64) -> Vec<f64> {
    let clip = |sim: &Sim| sim.app.world().get::<Magazine>(w).unwrap().clip;
    let mut out = Vec::new();
    sim.intent(p).fire = true;
    let end = now(sim) + seconds;
    let mut last = clip(sim);
    while now(sim) < end {
        sim.ticks(1);
        let c = clip(sim);
        if c < last {
            out.push(now(sim));
        }
        last = c;
    }
    sim.intent(p).fire = false;
    out
}

fn mean_gap(t: &[f64]) -> f64 {
    (t[t.len() - 1] - t[0]) / (t.len() - 1) as f64
}

/// A rifle copy fires at its own `fire_0` over 1 + `FireAniRate`
/// (0.2 / 1.25 = 160 ms), reloads in its `reload` and draws in its
/// `select`; with an attachments table these are its stock numbers.
#[test]
fn a_copy_fires_reloads_and_draws_on_its_own_animations() {
    let (mut sim, p) = sim();
    let w = weapon::give(sim.app.world_mut(), p, RIFLE).unwrap();
    {
        let world = sim.app.world();
        assert!((world.get::<Trigger>(w).unwrap().cycle - 0.16).abs() < 1e-6);
        assert_eq!(world.get::<Magazine>(w).unwrap().reload_time, 1.0);
        assert_eq!(world.get::<Weapon>(w).unwrap().draw_time, 0.5);
        let stock = world.get::<CaAttachSlots>(w).unwrap().stock;
        assert_eq!((stock.cycle, stock.reload, stock.draw), (0.2, 1.0, 0.5));
    }
    sim.seconds(1.0);
    assert_eq!(sim.app.world().get::<Inventory>(p).unwrap().active, Some(w));
    let t = shots(&mut sim, p, w, 2.0);
    assert!(t.len() >= 10, "{t:?}");
    let gap = mean_gap(&t);
    assert!((gap - 0.16).abs() < 0.01, "160 ms a round, not the M4A1's 105: {gap}");
    // The reload: its own 1 s.
    sim.seconds(0.5);
    sim.intent(p).reload = true;
    sim.ticks(1);
    sim.intent(p).reload = false;
    let start = now(&sim);
    while sim.app.world().get::<Magazine>(w).unwrap().clip < 30 && now(&sim) < start + 3.0 {
        sim.ticks(1);
    }
    let took = now(&sim) - start;
    assert!((took - 1.0).abs() < 0.05, "reloaded in {took} s");
    // Ours: the spec's numbers.
    let ours = weapon::give(sim.app.world_mut(), p, "combat_arms:m4a1").unwrap();
    let world = sim.app.world();
    assert!((world.get::<Trigger>(ours).unwrap().cycle - 0.105).abs() < 1e-6);
    assert_eq!(world.get::<Magazine>(ours).unwrap().reload_time, 2.3);
    assert_eq!(world.get::<Weapon>(ours).unwrap().draw_time, 1.2);
}

/// A shell-by-shell copy: `PreReload`, `reload` per shell, `PostReload`
/// (weapon_classes.md 5.5), and its pump is its `fire_0`.
#[test]
fn a_shell_copy_takes_its_shell_timings() {
    let (mut sim, p) = sim();
    let w = weapon::give(sim.app.world_mut(), p, SHOTGUN).unwrap();
    let world = sim.app.world();
    let s = world.get::<ShellReload>(w).unwrap();
    assert_eq!((s.start, s.insert, s.finish), (0.25, 0.4, 0.6));
    assert!((world.get::<Trigger>(w).unwrap().cycle - 0.9).abs() < 1e-6);
    assert_eq!(world.get::<Weapon>(w).unwrap().draw_time, 0.8);
}

/// A copy of a template without an inventory table takes its
/// `FireAniRate` (0.12 / 1.5 = 80 ms) and keeps the template's draw where
/// its model has no `select`.
#[test]
fn a_copy_without_an_inventory_template_keeps_its_fire_rate() {
    let (mut sim, p) = sim();
    let w = weapon::give(sim.app.world_mut(), p, SMG).unwrap();
    let world = sim.app.world();
    assert!((world.get::<Trigger>(w).unwrap().cycle - 0.08).abs() < 1e-6);
    assert_eq!(world.get::<Magazine>(w).unwrap().reload_time, 1.5);
    assert_eq!(world.get::<Weapon>(w).unwrap().draw_time, 1.0, "the MP5A4's draw");
}

/// A network game: the server reads the copies' models (the test's
/// `ViewTimes`); one client whose world `client` prepares (its own reads)
/// joins.
fn net(client: impl FnOnce(&mut World)) -> NetSim {
    let copies = copies();
    let link = LinkConditions {
        latency: Duration::from_millis(50),
        jitter: Duration::ZERO,
        loss: 0.0,
    };
    let mut sim = NetSim::new(CaSim, link, 11, 0, move |app| {
        app.add_plugins(GreyboxMapPlugin);
        register(app, &copies);
    });
    sim.add_client(client);
    sim.set_frame(TICK_INTERVAL);
    sim.until_joined(600);
    sim.ticks(240);
    sim
}

/// The client's weapon of `id` (the newest one).
fn client_weapon(sim: &mut NetSim, id: &str) -> Entity {
    let me = sim.local_player(0).unwrap();
    let w = sim.clients[0].app.world();
    *w.get::<Inventory>(me)
        .unwrap()
        .weapons
        .iter()
        .rev()
        .find(|&&e| w.get::<Weapon>(e).unwrap().id == id)
        .unwrap()
}

/// The server gives the rifle copy, the client fires it for 1.6 s: the
/// server fires at the copy's 160 ms cycle and the client predicts every
/// round without error.
fn burst(sim: &mut NetSim) -> Entity {
    let theirs = sim.character_of(0).unwrap();
    let sw = weapon::give(sim.server.app.world_mut(), theirs, RIFLE).unwrap();
    sim.ticks(150);
    let cw = client_weapon(sim, RIFLE);
    let me = sim.local_player(0).unwrap();
    assert_eq!(sim.clients[0].app.world().get::<Inventory>(me).unwrap().active, Some(cw));
    assert!((sim.clients[0].app.world().get::<Trigger>(cw).unwrap().cycle - 0.16).abs() < 1e-6);
    *sim.clients[0].app.world_mut().resource_mut::<NetGraph>() = NetGraph::default();
    let intent = |sim: &mut NetSim, fire: bool| {
        let me = sim.local_player(0).unwrap();
        sim.clients[0].app.world_mut().get_mut::<Intent>(me).unwrap().fire = fire;
    };
    let clip = |w: &World, e: Entity| w.get::<Magazine>(e).unwrap().clip;
    let before = clip(sim.server.app.world(), sw);
    intent(sim, true);
    // 1.6 s held: 1.6 / 0.16 + 1 = 11 rounds at the copy's cycle (16 at
    // the M4A1's), give or take one for the frame the press lands on.
    let ticks = (1.6 / TICK_INTERVAL).round() as u64;
    sim.ticks(ticks);
    intent(sim, false);
    sim.ticks(60);
    let fired = before - clip(sim.server.app.world(), sw);
    assert!((10..=12).contains(&fired), "the server fired {fired} rounds");
    let g = sim.clients[0].app.world().resource::<NetGraph>().clone();
    println!("timings: {} states checked, {} errors ({:?})", g.checked, g.errors, g.last_differing);
    assert!(g.checked > 0);
    assert_eq!(g.errors, 0, "mispredicted ({:?})", g.last_differing);
    let cw = client_weapon(sim, RIFLE);
    assert_eq!(clip(sim.clients[0].app.world(), cw), clip(sim.server.app.world(), sw));
    sw
}

/// The server and the client read the same models: the copy's timing is
/// the server's and predicted.
#[test]
fn a_copys_timing_is_the_servers_and_predicted() {
    let mut sim = net(|_| {});
    burst(&mut sim);
}

/// A client whose reads of the copies' models fail (no key for their
/// archive, no file) predicts on the server's timing, sent once per
/// weapon id (`net::weapon_params`), kept for the weapons it builds again.
#[test]
fn a_client_without_the_models_predicts_on_the_servers_timing() {
    let mut sim = net(|w| {
        let mut times = w.resource_mut::<ViewTimes>();
        for path in ["test/timing/pv_rifle.ltb", "test/timing/pv_shotgun.ltb", "test/timing/pv_smg.ltb"] {
            times.0.insert(path.into(), None);
        }
    });
    burst(&mut sim);
    // The shell copy's shell timings and pump, and its draw.
    let theirs = sim.character_of(0).unwrap();
    let ss = weapon::give(sim.server.app.world_mut(), theirs, SHOTGUN).unwrap();
    // A second rifle: the client builds its weapons again.
    weapon::give(sim.server.app.world_mut(), theirs, RIFLE).unwrap();
    sim.ticks(60);
    let cs = client_weapon(&mut sim, SHOTGUN);
    let me = sim.local_player(0).unwrap();
    let (server, client) = (sim.server.app.world(), sim.clients[0].app.world());
    let shell = |w: &World, e: Entity| {
        let s = w.get::<ShellReload>(e).unwrap();
        (s.start, s.insert, s.finish)
    };
    assert_eq!(shell(client, cs), (0.25, 0.4, 0.6));
    assert_eq!(shell(client, cs), shell(server, ss));
    assert_eq!(client.get::<Trigger>(cs).unwrap().cycle, server.get::<Trigger>(ss).unwrap().cycle);
    assert_eq!(client.get::<Weapon>(cs).unwrap().draw_time, 0.8);
    let rifles: Vec<Entity> = client
        .get::<Inventory>(me)
        .unwrap()
        .weapons
        .iter()
        .copied()
        .filter(|&e| client.get::<Weapon>(e).unwrap().id == RIFLE)
        .collect();
    assert_eq!(rifles.len(), 2);
    for r in rifles {
        assert!((client.get::<Trigger>(r).unwrap().cycle - 0.16).abs() < 1e-6);
        assert_eq!(client.get::<Magazine>(r).unwrap().reload_time, 1.0);
        assert_eq!(client.get::<CaAttachSlots>(r).unwrap().stock.cycle, 0.2);
    }
    // Once per weapon id.
    let heard = &client.resource::<ParamsHeard>().0;
    assert_eq!(heard.get(RIFLE), Some(&1), "{heard:?}");
    assert_eq!(heard.get(SHOTGUN), Some(&1), "{heard:?}");
    // Our own documents carry no timing: nothing sent for them.
    assert!(heard.keys().all(|id| [RIFLE, SHOTGUN].contains(id)), "{heard:?}");
}

/// A client that reads other lengths for the rifle's model (another
/// install version) still takes the server's.
#[test]
fn a_client_with_its_own_models_takes_the_servers_timing() {
    let mut sim = net(|w| {
        w.resource_mut::<ViewTimes>().set("test/timing/pv_rifle.ltb", AnimTimes {
            fire: Some(0.3),
            reload: Some(2.0),
            select: Some(0.9),
            ..default()
        });
    });
    burst(&mut sim);
    let cw = client_weapon(&mut sim, RIFLE);
    let client = sim.clients[0].app.world();
    assert_eq!(client.get::<Magazine>(cw).unwrap().reload_time, 1.0);
    assert_eq!(client.get::<Weapon>(cw).unwrap().draw_time, 0.5);
}

// ---------------------------------------------------------------------------
// The gear's animation rates (docs/plans/active/ca-weapon-timings.md,
// part 2): the vest's `ReloadAniRate` and `SelectAniRate` in the weapon's
// own reload and draw, server-authoritative and predicted; the gear
// records go to a client whose own differ (`data::served::GEAR`).

/// The server's gear data: every vest with `ReloadAniRate` 0.25 and
/// `SelectAniRate` 0.5 (rates 1.25 and 1.5).
fn fast_vests() -> combat_arms::game::gear::records::GearData {
    let mut g = combat_arms::game::gear::records::GearData::built_in();
    for v in g.vests.values_mut() {
        v.reload_ani_rate = 0.25;
        v.select_ani_rate = 0.5;
    }
    g
}

/// The first app built (the server) wears `fast_vests`; the clients
/// keep ours.
#[derive(Clone)]
struct ServerGear(std::sync::Arc<std::sync::atomic::AtomicUsize>);

impl Plugin for ServerGear {
    fn build(&self, app: &mut App) {
        if self.0.fetch_add(1, std::sync::atomic::Ordering::SeqCst) == 0 {
            app.insert_resource(combat_arms::game::gear::CaGearData(std::sync::Arc::new(fast_vests())));
        }
        CaSim.build(app);
    }
}

/// A vest's reload and select bonuses shorten the M4A1's own reload
/// (2.3 s / 1.25 = 1.84 s) and draw (1.2 s / 1.5 = 0.8 s) on the server,
/// and the client, whose gear records were ours, takes the server's
/// (`combat_arms:gear`) before it enters and predicts the reload without
/// error.
#[test]
fn a_vests_rates_shorten_the_reload_and_draw_on_both_sides() {
    use chimera_net::net::server_docs::DocsHeard;
    use combat_arms::game::{data::served::GEAR, gear::CaGearData};
    let link = LinkConditions {
        latency: Duration::from_millis(50),
        jitter: Duration::ZERO,
        loss: 0.0,
    };
    let pick = ServerGear(std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0)));
    let mut sim = NetSim::new(pick, link, 17, 0, |app| {
        app.add_plugins(GreyboxMapPlugin);
    });
    sim.add_client(|w| assert_ne!(*w.resource::<CaGearData>().0, fast_vests(), "the client starts on ours"));
    sim.set_frame(TICK_INTERVAL);
    sim.until_joined(900);
    sim.ticks(240);
    {
        let client = sim.clients[0].app.world();
        let heard = &client.resource::<DocsHeard>().0;
        assert!(heard.iter().any(|h| h.0 == GEAR), "the gear set came: {heard:?}");
        assert_eq!(*client.resource::<CaGearData>().0, fast_vests());
    }
    let theirs = sim.character_of(0).unwrap();
    let sw = weapon::give(sim.server.app.world_mut(), theirs, "combat_arms:m4a1").unwrap();
    sim.ticks(150);
    let cw = client_weapon(&mut sim, "combat_arms:m4a1");
    for (side, w, e) in [("server", sim.server.app.world(), sw), ("client", sim.clients[0].app.world(), cw)] {
        let reload = w.get::<Magazine>(e).unwrap().reload_time;
        let draw = w.get::<Weapon>(e).unwrap().draw_time;
        assert!((reload - 1.84).abs() < 1e-5, "{side}: reload {reload}");
        assert!((draw - 0.8).abs() < 1e-5, "{side}: draw {draw}");
    }
    // Fire a few rounds, then reload: timed on the server, predicted.
    let intent = |sim: &mut NetSim, f: &dyn Fn(&mut Intent)| {
        let me = sim.local_player(0).unwrap();
        f(&mut sim.clients[0].app.world_mut().get_mut::<Intent>(me).unwrap());
    };
    intent(&mut sim, &|i| i.fire = true);
    sim.ticks(30);
    intent(&mut sim, &|i| i.fire = false);
    sim.ticks(30);
    *sim.clients[0].app.world_mut().resource_mut::<NetGraph>() = NetGraph::default();
    let clip = |w: &World, e: Entity| w.get::<Magazine>(e).unwrap().clip;
    assert!(clip(sim.server.app.world(), sw) < 30);
    intent(&mut sim, &|i| i.reload = true);
    sim.ticks(2);
    intent(&mut sim, &|i| i.reload = false);
    let (mut started, mut filled) = (None, None);
    for _ in 0..((3.0 / TICK_INTERVAL) as u64) {
        sim.ticks(1);
        let w = sim.server.app.world();
        let now = w.resource::<SimClock>().now;
        let st = w.get::<chimera_sim::weapon::WeaponState>(sw).unwrap();
        if started.is_none() && st.reload_end.is_some() {
            started = Some(now);
        }
        if started.is_some() && filled.is_none() && clip(w, sw) == 30 {
            filled = Some(now);
        }
    }
    let took = filled.unwrap() - started.unwrap();
    assert!((took - 1.84).abs() < 0.05, "the server reloaded in {took} s, not 2.3");
    let g = sim.clients[0].app.world().resource::<NetGraph>().clone();
    println!("vest reload: {took:.3} s, {} states checked, {} errors ({:?})", g.checked, g.errors, g.last_differing);
    assert!(g.checked > 0);
    assert_eq!(g.errors, 0, "mispredicted ({:?})", g.last_differing);
    let cw = client_weapon(&mut sim, "combat_arms:m4a1");
    assert_eq!(clip(sim.clients[0].app.world(), cw), 30);
}

// ---------------------------------------------------------------------------
// The rest of the fire sequence from the model (the plan's part 3).

/// Ticks until weapon `w`'s clip goes down (at most `max`); the time it did.
fn next_shot(sim: &mut Sim, w: Entity, max: u64) -> Option<f64> {
    let clip = |sim: &Sim| sim.app.world().get::<Magazine>(w).unwrap().clip;
    let before = clip(sim);
    for _ in 0..max {
        sim.ticks(1);
        if clip(sim) < before {
            return Some(now(sim));
        }
    }
    None
}

/// A semi-automatic copy takes its model's `postfire` (300 ms) and, with
/// no `prefire` in its file, the shorter chain (latency 2): a press during
/// the post-fire fires on that update (weapon_classes.md 4: it cuts the
/// post-fire short); a reload asked after a shot waits for the post-fire
/// (view_models.md 7, "Reload after firing"); a press after the post-fire
/// pays the chain again.
#[test]
fn a_semi_automatic_copy_takes_its_postfire() {
    let (mut sim, p) = sim();
    let w = weapon::give(sim.app.world_mut(), p, PISTOL).unwrap();
    {
        let tr = sim.app.world().get::<Trigger>(w).unwrap();
        assert!(!tr.automatic);
        assert!((tr.cycle - 0.2).abs() < 1e-6 && (tr.postfire - 0.3).abs() < 1e-6, "{tr:?}");
        assert_eq!(tr.latency, 2, "no prefire in its file");
    }
    sim.seconds(1.0);
    let tick = TICK_INTERVAL as f64;
    // From idle: the chain's 2 updates.
    sim.intent(p).fire = true;
    let pressed = now(&sim);
    let t0 = next_shot(&mut sim, w, 30).expect("a shot");
    sim.intent(p).fire = false;
    // The update that sees the press, then the chain's 2.
    assert!((t0 - pressed - 3.0 * tick).abs() < tick * 0.5 + 1e-6, "first round {} s after the press", t0 - pressed);
    // Pressed 250 ms on (the shot's 200 over, the post-fire playing): at once.
    while now(&sim) < t0 + 0.25 {
        sim.ticks(1);
    }
    sim.intent(p).fire = true;
    let pressed = now(&sim);
    let t1 = next_shot(&mut sim, w, 30).expect("a shot");
    sim.intent(p).fire = false;
    assert!(t1 - pressed <= tick + 1e-6, "cut the post-fire short: {} s after the press", t1 - pressed);
    // A reload asked now starts when the shot (200 ms) and the post-fire
    // (300 ms) are over.
    sim.intent(p).reload = true;
    let mut started = None;
    for _ in 0..90 {
        sim.ticks(1);
        if started.is_none() && sim.app.world().get::<chimera_sim::weapon::WeaponState>(w).unwrap().reload_end.is_some() {
            started = Some(now(&sim));
        }
    }
    sim.intent(p).reload = false;
    let started = started.expect("reloaded");
    assert!(started - t1 >= 0.5 - tick, "the reload waited for the post-fire: {} s", started - t1);
    assert!(started - t1 <= 0.5 + 2.0 * tick, "and no longer: {} s", started - t1);
    // After the post-fire: the chain again.
    sim.seconds(2.0);
    sim.intent(p).fire = true;
    let pressed = now(&sim);
    let t2 = next_shot(&mut sim, w, 30).expect("a shot");
    sim.intent(p).fire = false;
    assert!(t2 - pressed > 2.5 * tick, "the chain again: {} s", t2 - pressed);
}

/// Our M92FS document plays its 150 ms `postfire` after a shot
/// (weapon_classes.md 4, view_models.md 7; ca-weapon-timings.md open
/// question 7): a reload asked after a shot starts when the shot's 180 ms
/// and the post-fire's 150 are over, and a press during the post-fire
/// still fires on the update that sees it (the M92FS fires every 180 ms
/// plus frame rounding, Quirk 6).
#[test]
fn our_m92fs_holds_a_reload_for_its_postfire() {
    let (mut sim, p) = sim();
    let w = weapon::give(sim.app.world_mut(), p, "combat_arms:beretta_m92fs").unwrap();
    {
        let tr = sim.app.world().get::<Trigger>(w).unwrap();
        assert!(!tr.automatic);
        assert!((tr.cycle - 0.18).abs() < 1e-6 && (tr.postfire - 0.15).abs() < 1e-6, "{tr:?}");
    }
    sim.seconds(1.0);
    let tick = TICK_INTERVAL as f64;
    sim.intent(p).fire = true;
    let t0 = next_shot(&mut sim, w, 30).expect("a shot");
    sim.intent(p).fire = false;
    // Pressed 250 ms on (the shot's 180 over, the post-fire playing): at once.
    while now(&sim) < t0 + 0.25 {
        sim.ticks(1);
    }
    sim.intent(p).fire = true;
    let pressed = now(&sim);
    let t1 = next_shot(&mut sim, w, 30).expect("a shot");
    sim.intent(p).fire = false;
    assert!(t1 - pressed <= tick + 1e-6, "cut the post-fire short: {} s after the press", t1 - pressed);
    // A reload asked now waits for the shot (180 ms) and the post-fire (150).
    sim.intent(p).reload = true;
    let mut started = None;
    for _ in 0..90 {
        sim.ticks(1);
        if started.is_none() && sim.app.world().get::<chimera_sim::weapon::WeaponState>(w).unwrap().reload_end.is_some() {
            started = Some(now(&sim));
        }
    }
    sim.intent(p).reload = false;
    let started = started.expect("reloaded");
    assert!(started - t1 >= 0.33 - tick, "the reload waited for the post-fire: {} s", started - t1);
    assert!(started - t1 <= 0.33 + 2.0 * tick, "and no longer: {} s", started - t1);
}

/// A minigun copy (`AccumPreFire`) spins on its own model: a 1 s spin-up,
/// a 0.8 s spin-down, its 200 ms loop's five keys a round each (40 ms),
/// latency 1, no trigger post-fire; held fire's first round comes when the
/// spin-up ends.
#[test]
fn a_minigun_copy_spins_on_its_own_model() {
    use combat_arms::game::weapons::classes::CaSpin;
    let (mut sim, p) = sim();
    let w = weapon::give(sim.app.world_mut(), p, MINIGUN).unwrap();
    {
        let world = sim.app.world();
        let s = world.get::<CaSpin>(w).unwrap().params;
        assert_eq!((s.prefire, s.postfire, s.loop_len, s.reversible), (1.0, 0.8, 0.2, true));
        let tr = world.get::<Trigger>(w).unwrap();
        assert!((tr.cycle - 0.04).abs() < 1e-6, "{tr:?}");
        assert_eq!((tr.latency, tr.postfire), (1, 0.0));
    }
    sim.seconds(2.0);
    let t = shots(&mut sim, p, w, 2.0);
    let first = t[0] - (now(&sim) - 2.0);
    assert!((first - 1.0).abs() < 0.06, "first round {first} s after the press");
    let gap = mean_gap(&t);
    assert!((gap - 0.04).abs() < 0.006, "{gap} s a round");
}

/// A knife copy swings on its own model: the slash `fire_0` (500 ms) with
/// its `FIRE_KEY`s (100, 300 ms), the stab `alt_fire_0` (900 ms) with its
/// `ALT_FIRE_KEY` (450 ms), the draw its `select`.
#[test]
fn a_knife_copys_swings_come_from_its_model() {
    use combat_arms::game::inventory::Knife;
    let (mut sim, p) = sim();
    let w = weapon::give(sim.app.world_mut(), p, KNIFE).unwrap();
    let world = sim.app.world();
    let k = world.get::<Knife>(w).unwrap();
    assert_eq!((k.attacks[0].length, k.attacks[0].hits.clone()), (0.5, vec![0.1, 0.3]));
    assert_eq!((k.attacks[1].length, k.attacks[1].hits.clone()), (0.9, vec![0.45]));
    assert_eq!(k.attacks[0].postfire, 0.001);
    assert_eq!(world.get::<Weapon>(w).unwrap().draw_time, 0.3);
    // Ours: the spec's M9.
    let ours = weapon::give(sim.app.world_mut(), p, "combat_arms:m9_knife").unwrap();
    let k = sim.app.world().get::<Knife>(ours).unwrap();
    assert_eq!((k.attacks[0].length, k.attacks[0].hits.clone()), (0.65, vec![0.155, 0.402]));
}

/// A client that can't read the models takes the server's spin, knife
/// swings, post-fire and latency (`combat_arms:timing`).
#[test]
fn a_client_without_the_models_takes_the_servers_sequence() {
    use combat_arms::game::{inventory::Knife, weapons::classes::CaSpin};
    let mut sim = net(|w| {
        let mut times = w.resource_mut::<ViewTimes>();
        for path in ["test/timing/pv_pistol.ltb", "test/timing/pv_minigun.ltb", "test/timing/pv_knife.ltb"] {
            times.0.insert(path.into(), None);
        }
    });
    let theirs = sim.character_of(0).unwrap();
    let mut server = Vec::new();
    for id in [PISTOL, MINIGUN, KNIFE] {
        server.push(weapon::give(sim.server.app.world_mut(), theirs, id).unwrap());
    }
    sim.ticks(150);
    let (cp, cm, ck) = (client_weapon(&mut sim, PISTOL), client_weapon(&mut sim, MINIGUN), client_weapon(&mut sim, KNIFE));
    let (s, c) = (sim.server.app.world(), sim.clients[0].app.world());
    let tr = |w: &World, e: Entity| {
        let t = w.get::<Trigger>(e).unwrap();
        (t.cycle, t.postfire, t.latency)
    };
    assert_eq!(tr(c, cp), tr(s, server[0]));
    assert_eq!(tr(c, cp).2, 2);
    assert_eq!(tr(c, cm), tr(s, server[1]));
    assert_eq!(c.get::<CaSpin>(cm).unwrap().params, s.get::<CaSpin>(server[1]).unwrap().params);
    assert_eq!(c.get::<CaSpin>(cm).unwrap().params.prefire, 1.0);
    assert_eq!(c.get::<Knife>(ck).unwrap().attacks, s.get::<Knife>(server[2]).unwrap().attacks);
    assert_eq!(c.get::<Knife>(ck).unwrap().attacks[1].hits, vec![0.45]);
}

/// weapon_classes.md 43 and 44 (Corrections 2026-10-30, 1), at 60 updates
/// a second: a pistol plays no pre-fire (the M92FS fires on update 2, as
/// a primary shotgun; the M4A1's 0 ms pre-fire on update 3); a 200 ms
/// pre-fire delays the first round by its whole length: `prefire` from
/// update 1, seen done on update 14 (233.3 ms), the round on update 15
/// (250 ms).
#[test]
fn t43_t44_pre_fire_by_class_and_length() {
    use combat_arms::game::weapons::timing::prefire_latency;
    assert_eq!(prefire_latency(0.0, 1.0 / 60.0), 3);
    assert_eq!(prefire_latency(0.2, 1.0 / 60.0), 15);
    let copies = copies();
    let mut sim = Sim::with(CaSim, move |app| {
        app.add_plugins(GreyboxMapPlugin);
        register(app, &copies);
    });
    let tick = 1.0 / 60.0;
    sim.set_tick_interval(tick);
    let p = sim.spawn_character(Vec3::new(30.0, 1.0, 30.0), movement::ID);
    sim.seconds(0.5);
    sim.app.world_mut().entity_mut(p).insert(Health { current: 1.0, max: 1.0 });
    for (id, updates) in [
        ("combat_arms:beretta_m92fs", 2),
        ("combat_arms:remington_870", 2),
        ("combat_arms:m4a1", 3),
        (RIFLE_LONG, 15),
    ] {
        let w = weapon::give(sim.app.world_mut(), p, id).unwrap();
        assert_eq!(sim.app.world().get::<Trigger>(w).unwrap().latency, updates, "{id}");
        sim.seconds(2.0);
        sim.intent(p).fire = true;
        let pressed = now(&sim);
        let t = next_shot(&mut sim, w, 40).expect("a shot");
        sim.intent(p).fire = false;
        // The update that sees the press is update 0 (the tick after `pressed`).
        let at = ((t - pressed) / tick).round() as u32 - 1;
        assert_eq!(at, updates, "{id}: the round on update {at}");
        sim.seconds(1.0);
    }
}
