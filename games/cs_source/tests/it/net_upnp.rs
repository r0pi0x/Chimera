//! UPnP port mapping while hosting (`net_upnp`, `net::upnp`;
//! docs/plans/active/nat-traversal.md): a fake router stands in for the
//! real one, so no test asks the network's router for anything.

use std::{
    sync::{Arc, Mutex},
    time::Duration,
};

use bevy::prelude::*;
use crate::stack::{
    console::Console,
    games::cs_source::movement::{self as source, SourceMovementPlugin},
    greybox::GreyboxMapPlugin,
    harness::{CsSim, Sim},
    net::{
        self, NetSettings,
        upnp::{Mapped, PortMapper, PortMapping, Upnp, UpnpState},
    },
    slots::Loadout,
};

/// A router that records what it is asked, and refuses when told to.
#[derive(Default)]
struct FakeRouter {
    calls: Mutex<Vec<String>>,
    refuse: bool,
    public: Option<&'static str>,
}

impl PortMapper for FakeRouter {
    fn map(&self, port: u16, lease_secs: u32) -> Result<Mapped, String> {
        self.calls.lock().unwrap().push(format!("map {port} {lease_secs}"));
        if self.refuse {
            return Err("no UPnP router answered".into());
        }
        Ok(Mapped {
            router: "192.168.1.1".into(),
            public_ip: self.public.map(|p| p.parse().unwrap()),
        })
    }

    fn unmap(&self, port: u16) -> Result<(), String> {
        self.calls.lock().unwrap().push(format!("unmap {port}"));
        Ok(())
    }
}

fn server(router: Arc<FakeRouter>, upnp: bool) -> Sim {
    let mut sim = Sim::with(CsSim, |app| {
        app.add_plugins((net::NetPlugin, GreyboxMapPlugin, SourceMovementPlugin))
            .insert_resource(Loadout { movement: source::ID });
    });
    let w = sim.app.world_mut();
    w.insert_resource(PortMapping(router));
    w.resource_mut::<NetSettings>().hostport = 0;
    w.resource_mut::<NetSettings>().maxplayers = 4;
    if upnp {
        w.resource_mut::<Console>().submit("net_upnp 1");
    }
    sim.app.update();
    sim
}

/// Step until `done` (the router answers on another thread).
fn until(sim: &mut Sim, mut done: impl FnMut(&mut Sim) -> bool) -> bool {
    for _ in 0..500 {
        if done(sim) {
            return true;
        }
        sim.app.update();
        chimera_world::harness::let_io_run();
    }
    done(sim)
}

fn state(sim: &Sim) -> UpnpState {
    sim.app.world().resource::<Upnp>().state.clone()
}

#[test]
fn hosting_asks_the_router_and_hanging_up_takes_it_back() {
    let router = Arc::new(FakeRouter {
        public: Some("81.2.69.160"),
        ..default()
    });
    let mut sim = server(router.clone(), true);
    let addr = net::server::listen(sim.app.world_mut()).expect("listen");
    let port = addr.port();
    assert!(
        until(&mut sim, |s| matches!(state(s), UpnpState::Mapped(..))),
        "mapped: {:?}",
        state(&sim)
    );
    assert_eq!(
        router.calls.lock().unwrap().clone(),
        vec![format!("map {port} {}", net::upnp::LEASE_SECS)]
    );
    let status = net::status(sim.app.world_mut());
    assert!(
        status.contains(&format!(
            "upnp    : UDP {port} forwarded by router 192.168.1.1; friends connect to 81.2.69.160:{port}"
        )),
        "{status}"
    );
    net::disconnect(sim.app.world_mut(), "done");
    assert!(until(&mut sim, |_| router.calls.lock().unwrap().len() == 2));
    assert_eq!(router.calls.lock().unwrap()[1], format!("unmap {port}"));
    assert_eq!(state(&sim), UpnpState::Off);
}

#[test]
fn without_net_upnp_the_router_is_left_alone() {
    let router = Arc::new(FakeRouter::default());
    let mut sim = server(router.clone(), false);
    net::server::listen(sim.app.world_mut()).expect("listen");
    for _ in 0..20 {
        sim.app.update();
    }
    assert!(router.calls.lock().unwrap().is_empty());
    assert!(!net::status(sim.app.world_mut()).contains("upnp"));
}

#[test]
fn a_router_that_refuses_or_sits_behind_carrier_grade_nat_is_reported() {
    let router = Arc::new(FakeRouter {
        refuse: true,
        ..default()
    });
    let mut sim = server(router, true);
    let port = net::server::listen(sim.app.world_mut()).expect("listen").port();
    assert!(until(&mut sim, |s| matches!(state(s), UpnpState::Failed(..))));
    let status = net::status(sim.app.world_mut());
    assert!(
        status.contains(&format!("couldn't forward UDP {port}: no UPnP router answered")),
        "{status}"
    );

    let cgnat = Arc::new(FakeRouter {
        public: Some("100.72.3.4"),
        ..default()
    });
    let mut sim = server(cgnat, true);
    net::server::listen(sim.app.world_mut()).expect("listen");
    assert!(until(&mut sim, |s| matches!(state(s), UpnpState::Mapped(..))));
    let status = net::status(sim.app.world_mut());
    assert!(status.contains("carrier-grade NAT"), "{status}");
}
