//! The server's data documents (docs/plans/active/multiplayer.md, "Server
//! data documents"; chimera_net's `net::server_docs`, Combat Arms'
//! `data::served`): a server whose weapons come from an install (an
//! attribute file written here: our M4A1 with another clip, two copies,
//! an attachment record of its own) and a client with none (no key) or
//! the same: the client without asks for the sets that differ, takes
//! them before it enters the game, and then has the server's registry and
//! predicts a copy's burst without error; the one with the same data gets
//! nothing; a set too big to send, or a registry that still differs,
//! ends the join with the reason.

use std::{
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};

use bevy::prelude::*;
use chimera_core::core::Intent;
use chimera_net::{
    harness::NetSim,
    net::{
        self,
        client::{JoinFailure, JoinProgress},
        memory::LinkConditions,
        predict::NetGraph,
        server_docs::{DATA, DocsHeard, DocsLimit, MAX_PACKED, registry_offer},
    },
};
use chimera_sim::{
    server_docs::ServerDocs,
    weapon::{self, Inventory, Magazine, Weapon, WeaponRegistry, data::WeaponDataAppExt},
};
use chimera_world::greybox::GreyboxMapPlugin;
use combat_arms::{
    game::{
        TICK_INTERVAL,
        attachments::CaModTable,
        data::{
            CaData,
            attributes::{MOD_FILE, WEAPONS_FILE},
            read_attributes,
            served::{ATTACHMENTS, MOVEMENT, WEAPONS},
        },
    },
    harness::CaSimWith,
};

const INSTALL: &str = r#"
[Ammo0] Name="556" Type=1 MaxAmount=120
[Ammo1] Name="9mm" Type=1 MaxAmount=120
[Weapon0] Name="M4A1" Guntype=4 AmmoName0="556" ShotsPerClip=40
[Weapon1] Name="Docs Rifle" Guntype=4 AmmoName0="556" ShotsPerClip=45 RapidFireCount=0 FireAniRate=1.0
[Weapon2] Name="Docs Carbine" Guntype=4 AmmoName0="9mm" ShotsPerClip=25
"#;

const MODS: &str = r#"
[Mod0] Name="Docs_Hider" Kinds=0 Type=3 Integrated=1
"#;

const COPY: &str = "combat_arms:docs_rifle";

/// The install's data, as the server reads it.
fn install() -> CaData {
    let (attrs, problems) = read_attributes(|name| match name {
        WEAPONS_FILE => Ok(INSTALL.as_bytes().to_vec()),
        MOD_FILE => Ok(MODS.as_bytes().to_vec()),
        _ => Err(std::io::Error::new(std::io::ErrorKind::NotFound, name.to_string())),
    });
    assert!(attrs.weapons.is_some() && attrs.mods.is_some(), "{problems:?}");
    CaData::with_attributes(attrs)
}

/// Each app built takes the next data (the server first, then each
/// client; the last for the rest).
#[derive(Clone)]
struct Pick(Arc<AtomicUsize>, Vec<CaData>);

impl Plugin for Pick {
    fn build(&self, app: &mut App) {
        let i = self.0.fetch_add(1, Ordering::SeqCst).min(self.1.len() - 1);
        CaSimWith(self.1[i].clone()).build(app);
    }
}

fn link() -> LinkConditions {
    LinkConditions {
        latency: Duration::from_millis(50),
        jitter: Duration::ZERO,
        loss: 0.0,
    }
}

/// A server on `server` data with one client on `client` data (`before`
/// changes its world), stepped until joined (or `max` frames).
fn net(server: CaData, client: CaData, before: impl FnOnce(&mut World), max: u64) -> NetSim {
    let pick = Pick(Arc::new(AtomicUsize::new(0)), vec![server, client]);
    let mut sim = NetSim::new(pick, link(), 13, 0, |app| {
        app.add_plugins(GreyboxMapPlugin);
    });
    sim.add_client(before);
    sim.set_frame(TICK_INTERVAL);
    sim.until(max, |s| s.local_player(0).is_some());
    sim
}

fn ids(w: &World) -> Vec<&'static str> {
    w.resource::<WeaponRegistry>().0.iter().map(|d| d.id).collect()
}

/// The client without the install's data takes the server's weapon
/// documents and attachment records (its movement is the same: not
/// asked for), before it enters the game; then its registry is the
/// server's, index for index, and it predicts the copy's burst on the
/// copy's clip without error. Leaving puts its own data back.
#[test]
fn a_client_without_the_data_takes_the_servers() {
    let server_data = install();
    assert!(server_data.weapon_set().get(COPY).is_some());
    let own = Arc::new(std::sync::Mutex::new(Vec::new()));
    let keep = own.clone();
    let mut sim = net(
        server_data,
        CaData::builtin(),
        move |w| *keep.lock().unwrap() = ids(w),
        900,
    );
    assert!(!own.lock().unwrap().contains(&COPY));
    sim.ticks(240);
    let (server, client) = (sim.server.app.world(), sim.clients[0].app.world());
    let heard = client
        .get_resource::<DocsHeard>()
        .expect("the server's data came")
        .0
        .clone();
    for (name, packed, unpacked, count) in &heard {
        println!("{name}: {count} documents, {unpacked} bytes, {packed} packed");
        assert!(*packed as usize <= MAX_PACKED && packed < unpacked, "{name}");
    }
    let names: Vec<&str> = heard.iter().map(|h| h.0.as_str()).collect();
    assert_eq!(names, [ATTACHMENTS, WEAPONS], "only the sets that differ");
    assert!(!names.contains(&MOVEMENT));
    assert_eq!(
        registry_offer(client.resource::<WeaponRegistry>()),
        registry_offer(server.resource::<WeaponRegistry>())
    );
    assert_eq!(ids(client), ids(server));
    assert_eq!(client.resource::<CaModTable>(), server.resource::<CaModTable>());
    assert!(client.resource::<CaModTable>().find("Docs_Hider").is_some());
    assert!(client.resource::<ServerDocs>().get(WEAPONS).unwrap().replaced());

    // The copy: the server's clip, predicted without error.
    let theirs = sim.character_of(0).unwrap();
    let sw = weapon::give(sim.server.app.world_mut(), theirs, COPY).unwrap();
    sim.ticks(150);
    let me = sim.local_player(0).unwrap();
    let cw = {
        let w = sim.clients[0].app.world();
        let active = w.get::<Inventory>(me).unwrap().active.unwrap();
        assert_eq!(w.get::<Weapon>(active).unwrap().id, COPY);
        active
    };
    let clip = |w: &World, e: Entity| w.get::<Magazine>(e).unwrap().clip;
    assert_eq!(sim.clients[0].app.world().get::<Magazine>(cw).unwrap().size, 45);
    *sim.clients[0].app.world_mut().resource_mut::<NetGraph>() = NetGraph::default();
    let before = clip(sim.server.app.world(), sw);
    let fire = |sim: &mut NetSim, on: bool| {
        let me = sim.local_player(0).unwrap();
        sim.clients[0].app.world_mut().get_mut::<Intent>(me).unwrap().fire = on;
    };
    fire(&mut sim, true);
    sim.ticks((1.0 / TICK_INTERVAL).round() as u64);
    fire(&mut sim, false);
    sim.ticks(60);
    let fired = before - clip(sim.server.app.world(), sw);
    assert!(fired >= 5, "the server fired {fired} rounds");
    let g = sim.clients[0].app.world().resource::<NetGraph>().clone();
    println!(
        "burst: {fired} rounds, {} states checked, {} errors ({:?})",
        g.checked, g.errors, g.last_differing
    );
    assert!(g.checked > 0);
    assert_eq!(g.errors, 0, "mispredicted ({:?})", g.last_differing);
    assert_eq!(clip(sim.clients[0].app.world(), cw), clip(sim.server.app.world(), sw));

    // Leaving: this machine's own data again.
    net::disconnect(sim.clients[0].app.world_mut(), "test");
    let client = sim.clients[0].app.world();
    assert_eq!(ids(client), *own.lock().unwrap());
    assert!(client.resource::<CaModTable>().find("Docs_Hider").is_none());
    assert!(!client.resource::<ServerDocs>().get(WEAPONS).unwrap().replaced());
}

/// A client with the server's data asks for nothing and gets nothing.
#[test]
fn a_client_with_the_same_data_gets_nothing() {
    let data = install();
    let mut sim = net(data.clone(), data, |_| {}, 900);
    sim.ticks(60);
    let client = sim.clients[0].app.world();
    assert!(client.get_resource::<DocsHeard>().is_none());
    assert!(!client.resource::<ServerDocs>().get(WEAPONS).unwrap().replaced());
    assert_eq!(ids(client), ids(sim.server.app.world()));
}

/// A set that packs bigger than the server sends: the join ends with the
/// reason, nothing played on other weapons.
#[test]
fn a_set_too_big_to_send_refuses_the_join() {
    let mut sim = net(install(), CaData::builtin(), |_| {}, 0);
    sim.server.app.insert_resource(DocsLimit(1024));
    sim.until(900, |s| {
        s.clients[0].app.world().resource::<JoinProgress>().failure.is_some()
    });
    let p = sim.clients[0].app.world().resource::<JoinProgress>().clone();
    let (kind, reason) = p.failure.expect("refused");
    println!("{reason}");
    assert_eq!(kind, JoinFailure::Mounts);
    assert!(reason.starts_with(DATA) && reason.contains("too big"), "{reason}");
    assert!(sim.local_player(0).is_none());
}

/// A client whose registry still differs from the server's (another
/// build: a weapon of its own) leaves with the reason.
#[test]
fn a_registry_that_differs_ends_the_join() {
    let mut sim = net(CaData::builtin(), CaData::builtin(), |_| {}, 0);
    // The client gets a weapon of its own (as another build would have).
    sim.clients[0]
        .app
        .add_weapon_data(&[("extra.toml", "id = \"test:extra\"\nslot = 2\ndraw = 1.0\n")])
        .unwrap();
    sim.until(900, |s| {
        s.clients[0].app.world().resource::<JoinProgress>().failure.is_some()
    });
    let (kind, reason) = sim.clients[0]
        .app
        .world()
        .resource::<JoinProgress>()
        .failure
        .clone()
        .expect("refused");
    println!("{reason}");
    assert_eq!(kind, JoinFailure::Mounts);
    assert!(reason.starts_with(DATA) && reason.contains("weapon list"), "{reason}");
}

/// Every format Combat Arms' data is read in has its fields traced
/// (`chimera_sim::data_format`: the join compares them, `net::formats`),
/// the view model's `hidden_pieces` (the 2026-11-12 rehearsal's change)
/// among them, and the list a client sends stays small.
#[test]
fn combat_arms_data_formats_are_traced() {
    use chimera_net::net::formats::{encode, fingerprints};
    use chimera_sim::data_format::formats;
    let sim = chimera_world::harness::Sim::new(combat_arms::harness::CaSim, GreyboxMapPlugin);
    let world = sim.app.world();
    let all = formats(world);
    for name in [combat_arms::game::view_model::VIEW, ATTACHMENTS, MOVEMENT, combat_arms::game::data::served::GEAR] {
        assert!(all.contains_key(name), "{name}: {:?}", all.keys().collect::<Vec<_>>());
    }
    assert!(all[combat_arms::game::view_model::VIEW].contains("hidden_pieces"));
    for (name, f) in all.iter().filter(|(n, _)| n.starts_with("combat_arms:")) {
        assert!(f.contains("struct ") || f.contains(". map"), "{name}: {f}");
    }
    let sent = encode(&fingerprints(world));
    println!("{} formats, {} bytes: {sent}", all.len(), sent.len());
    assert!(sent.len() < 4096);
}
