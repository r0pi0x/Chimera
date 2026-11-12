//! `NetSim`: a server and N clients in one process, each a headless
//! `Sim` (chimera_world's `harness`) on the same simulation base (a game's
//! set of plugins), joined by `net::memory`'s in-memory link and stepped
//! together. Game-neutral: what the server and clients run comes in as
//! the base and a setup; a game's own map loading for network tests
//! (`cs_source::harness::TestMaps`) sits above.

use std::{
    sync::{Arc, Mutex},
    time::Duration,
};

use bevy::{prelude::*, time::TimeUpdateStrategy};
use chimera_world::harness::Sim;

use crate::{
    core::{DEFAULT_TICK_HZ, LocalPlayer},
    net::{
        self, NetCharacter, NetPlugin, NetSettings,
        memory::{Link, LinkConditions},
    },
};

type Setup = Arc<dyn Fn(&mut App) + Send + Sync>;

/// A server and clients in one process, each a headless `Sim`, joined by
/// an in-memory link (`net::memory`) with seeded latency, jitter and loss.
/// Stepped together one tick at a time: the server, then each client.
pub struct NetSim {
    pub server: Sim,
    pub clients: Vec<Sim>,
    pub link: Arc<Mutex<Link>>,
    /// Each client's id, by index in `clients`, and the next one to give.
    ids: Vec<u64>,
    next_id: u64,
    /// Every app's base, the network and `setup`.
    build: Setup,
    /// Real time per `step` (a frame everywhere): a tick unless
    /// `set_frame` says otherwise.
    frame: Duration,
}

impl NetSim {
    /// A server running `base` (the simulation's plugins), the network and
    /// what `setup` adds (a map, game plugins) with `clients` clients
    /// connected (not yet joined: step until they are, e.g.
    /// `until_joined`). Client ids are 1, 2, ...
    pub fn new<B>(
        base: impl bevy::app::Plugins<B> + Clone + Send + Sync + 'static,
        conditions: LinkConditions,
        seed: u64,
        clients: usize,
        setup: impl Fn(&mut App) + Send + Sync + 'static,
    ) -> Self {
        let build: Setup = Arc::new(move |app: &mut App| {
            app.add_plugins(base.clone());
            app.add_plugins(NetPlugin);
            setup(app);
        });
        let link = Link::new(conditions, seed);
        let mut server = Sim::with((), |app| build(app));
        // Room for any test's players (`a_full_server_refuses` sets fewer).
        server.app.world_mut().resource_mut::<NetSettings>().maxplayers = 32;
        net::memory::serve(server.app.world_mut(), link.clone()).expect("serve");
        let mut sim = Self {
            server,
            clients: Vec::new(),
            link,
            ids: Vec::new(),
            next_id: 1,
            build,
            frame: Duration::from_secs_f64(1.0 / DEFAULT_TICK_HZ),
        };
        for _ in 0..clients {
            sim.add_client(|_| {});
        }
        sim
    }

    /// Connect another client, after `before` changes its world (e.g. its
    /// `net::NetVersion`). Returns its index in `clients`.
    pub fn add_client(&mut self, before: impl FnOnce(&mut World)) -> usize {
        let build = self.build.clone();
        let mut client = Sim::with((), |app| build(app));
        client.app.insert_resource(TimeUpdateStrategy::ManualDuration(self.frame));
        before(client.app.world_mut());
        // Ids never come back (a client removed keeps its id taken), as
        // real clients' random ones don't.
        let id = self.next_id;
        self.next_id += 1;
        net::memory::join(client.app.world_mut(), self.link.clone(), id).expect("join");
        self.clients.push(client);
        self.ids.push(id);
        self.clients.len() - 1
    }

    /// Client `i` leaves (`net::disconnect`, the server hears it) and is
    /// taken out of `clients`: later clients' indices move down by one,
    /// their ids stay.
    pub fn remove_client(&mut self, i: usize) {
        net::disconnect(self.clients[i].app.world_mut(), "Disconnect by user.");
        self.clients[i].app.update();
        self.clients.remove(i);
        self.ids.remove(i);
    }

    /// The server quits (`net::disconnect`: its clients hear it hang up)
    /// and stays down until `start_server`; its app goes on being stepped,
    /// out of the game.
    pub fn stop_server(&mut self) {
        net::disconnect(self.server.app.world_mut(), "Server shutting down.");
    }

    /// A new server process on the same link (as a dedicated server
    /// started again on the same address), built as the first was.
    pub fn start_server(&mut self) {
        let build = self.build.clone();
        let mut server = Sim::with((), |app| build(app));
        server.app.world_mut().resource_mut::<NetSettings>().maxplayers = 32;
        server
            .app
            .insert_resource(TimeUpdateStrategy::ManualDuration(self.frame));
        net::memory::serve(server.app.world_mut(), self.link.clone()).expect("serve");
        self.server = server;
    }

    /// Client `i`'s link (both ways) from now on, in place of the
    /// conditions the sim was made with.
    pub fn set_conditions(&mut self, i: usize, conditions: LinkConditions) {
        let id = self.client_id(i);
        self.link.lock().unwrap().per_client.insert(id, conditions);
    }

    /// Client `i`'s id (`NetCharacter::owner` of its character): 1, 2, ...
    /// in the order they were added.
    pub fn client_id(&self, i: usize) -> u64 {
        self.ids[i]
    }

    /// Every app's frame from now on (s): e.g. 1/240 for frames between
    /// ticks, as a game draws them. `step` then advances by that.
    pub fn set_frame(&mut self, secs: f64) {
        self.frame = Duration::from_secs_f64(secs);
        for sim in std::iter::once(&mut self.server).chain(&mut self.clients) {
            sim.app.insert_resource(TimeUpdateStrategy::ManualDuration(self.frame));
        }
    }

    /// One frame everywhere (a tick unless `set_frame` changed it): the
    /// link's clock, the server, every client.
    pub fn step(&mut self) {
        let dt = self.frame;
        self.link.lock().unwrap().advance(dt);
        self.server.app.update();
        for c in &mut self.clients {
            c.app.update();
        }
    }

    pub fn ticks(&mut self, n: u64) {
        for _ in 0..n {
            self.step();
        }
    }

    /// Step until `done` holds (true) or `max` ticks pass (false).
    pub fn until(&mut self, max: u64, mut done: impl FnMut(&mut Self) -> bool) -> bool {
        for _ in 0..max {
            if done(self) {
                return true;
            }
            self.step();
        }
        done(self)
    }

    /// Step until every client has joined (its map checked) and sees its
    /// own character; panics after `max` ticks.
    pub fn until_joined(&mut self, max: u64) {
        let ok = self.until(max, |s| {
            (0..s.clients.len()).all(|i| {
                s.clients[i].app.world().contains_resource::<net::client::Joined>() && s.local_player(i).is_some()
            })
        });
        assert!(ok, "clients didn't join within {max} ticks");
    }

    /// Client `i`'s own character (its `LocalPlayer`), in its world.
    pub fn local_player(&mut self, i: usize) -> Option<Entity> {
        let world = self.clients[i].app.world_mut();
        world
            .query_filtered::<Entity, With<LocalPlayer>>()
            .iter(world)
            .next()
    }

    /// Step until the server has turned every client's character (its
    /// spawn facing, `core::ViewSet`: the server spawns it a few ticks
    /// after the client sees it) and the client has taken that look (the
    /// last command the server ran acks it, `NetCmd::look_ack`). Until
    /// then the turn overwrites the client's own look when it arrives and
    /// the server keeps its look over the commands', so a test that
    /// scripts a client's look right after joining waits for this first.
    /// Panics after `max` ticks.
    pub fn until_looks_taken(&mut self, max: u64) {
        let ok = self.until(max, |s| {
            (0..s.clients.len()).all(|i| {
                let Some(e) = s.character_of(i) else { return false };
                let world = s.server.app.world();
                let Some(turn) = world.get::<crate::core::ViewSet>(e) else { return false };
                world
                    .get::<net::server::CommandBuffer>(e)
                    .and_then(|b| b.last.as_ref())
                    .is_some_and(|c| c.look_ack == turn.count)
            })
        });
        assert!(ok, "clients didn't take the server's look within {max} ticks");
    }

    /// The server's character for client `i`.
    pub fn character_of(&mut self, i: usize) -> Option<Entity> {
        let id = self.client_id(i);
        Self::owned_by(&mut self.server, Some(id))
    }

    /// The character owned by `owner` in a sim's world (`None`: a bot).
    pub fn owned_by(sim: &mut Sim, owner: Option<u64>) -> Option<Entity> {
        let world = sim.app.world_mut();
        world
            .query::<(Entity, &NetCharacter)>()
            .iter(world)
            .find(|(_, c)| c.owner == owner)
            .map(|(e, _)| e)
    }
}
