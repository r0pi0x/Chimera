//! Decals for players joining mid-game (docs/plans/active/multiplayer.md,
//! slice 7's leftover): the server keeps the impact decals put on the
//! world since its map loaded (`DecalLog`, the newest
//! `map::MAX_DECALS`) and sends them to a client when it comes into
//! the game (`Loaded`); the client places them as its own
//! (`map::PlaceDecal`). Decals on props and brush entities (which
//! move, break and come back) aren't kept, nor is which variant a decal
//! took (each client picks its own, as for every shot it draws).
//!
//! Players' sprays (`map::decal::Spray`, one entity each) replicate: late
//! joiners get them as they get every replicated entity, and the server's
//! despawning a replaced spray or the map's takes them off every client. A
//! client's `impulse 201` (`weapon::spray::SprayRequested`) goes to the
//! server as a `SprayRequest`, which sprays for its player
//! (`weapon::spray::spray_with`; its logo from the player's `cl_logofile`
//! userinfo). An imported logo's picture (at most 64 x 64, 16 KiB) rides
//! the request and then the replicated `Spray`, so every client draws it. Its other player impulses (`weapon::PlayerImpulse`:
//! `impulse 100`, a game's flashlight) go as `ImpulseRequest`.

use std::collections::VecDeque;

use bevy::prelude::*;
use bevy_replicon::prelude::*;
use serde::{Deserialize, Serialize};

use super::server::Player;
use crate::{
    core::NetRole,
    map::{DecalGroup, MAX_DECALS, MapBrushEntity, PlaceDecal, PropIndex, decal::Spray},
    weapon::spray::SprayRequested,
};

/// Client -> server: a player impulse of mine (`weapon::PlayerImpulse`:
/// `impulse 100`).
#[derive(Message, Serialize, Deserialize, Clone, Copy, Debug, Default, PartialEq)]
pub struct ImpulseRequest(pub u8);

/// Client -> server: spray my logo (`impulse 201`), with its picture when
/// it is one I imported (at most 64 x 64: the server refuses larger).
#[derive(Message, Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct SprayRequest(pub Option<crate::map::decal::SprayPicture>);

/// One world decal: its group (a named one or a surface material's), where,
/// the surface's normal, the direction of what made it, spun or upright.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct NetDecal {
    pub named: Option<String>,
    pub material: Option<char>,
    pub point: [f32; 3],
    pub normal: [f32; 3],
    pub dir: [f32; 3],
    pub spin: bool,
}

impl NetDecal {
    fn of(p: &PlaceDecal) -> Self {
        let (named, material) = match &p.group {
            DecalGroup::Named(n) => (Some(n.clone()), None),
            DecalGroup::Material(m) => (None, Some(*m)),
        };
        Self {
            named,
            material,
            point: p.point.to_array(),
            normal: p.normal.to_array(),
            dir: p.dir.to_array(),
            spin: p.spin,
        }
    }

    fn place(&self) -> Option<PlaceDecal> {
        let group = match (&self.named, self.material) {
            (Some(n), _) => DecalGroup::Named(n.chars().take(64).collect()),
            (None, Some(m)) => DecalGroup::Material(m),
            (None, None) => return None,
        };
        let finite = |v: [f32; 3]| v.iter().all(|x| x.is_finite());
        (finite(self.point) && finite(self.normal) && finite(self.dir)).then(|| PlaceDecal {
            target: None,
            group,
            point: Vec3::from_array(self.point),
            normal: Vec3::from_array(self.normal),
            dir: Vec3::from_array(self.dir),
            spin: self.spin,
            round: false,
        })
    }
}

/// Server -> a client coming into the game: the world's decals so far,
/// oldest first.
#[derive(Message, Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct Decals {
    pub list: Vec<NetDecal>,
}

/// On a server: the world's decals since the map loaded, oldest first.
#[derive(Resource, Default, Debug)]
pub struct DecalLog {
    pub decals: VecDeque<NetDecal>,
    /// The map loads counted when it was last cleared (`rules::MapLoads`).
    loads: u32,
}

pub(super) fn plugin(app: &mut App) {
    app.add_server_message::<Decals>(Channel::Ordered)
        .make_message_independent::<Decals>()
        .init_resource::<DecalLog>()
        .add_message::<PlaceDecal>()
        .replicate::<Spray>()
        .add_client_message::<SprayRequest>(Channel::Ordered)
        .add_client_message::<ImpulseRequest>(Channel::Ordered)
        .add_systems(PostUpdate, record.run_if(resource_equals(NetRole::Server)))
        .add_systems(
            PreUpdate,
            (receive_sprays, receive_impulses)
                .chain()
                .after(ServerSystems::Receive)
                .run_if(resource_equals(NetRole::Server)),
        )
        .add_systems(
            PostUpdate,
            replicate_sprays
                .before(ServerSystems::Send)
                .run_if(resource_equals(NetRole::Server)),
        )
        .add_systems(
            PostUpdate,
            (send_sprays, send_impulses)
                .chain()
                .before(ClientSystems::Send)
                .run_if(resource_equals(NetRole::Client)),
        )
        .add_systems(
            PreUpdate,
            receive
                .after(ClientSystems::Receive)
                .run_if(resource_equals(NetRole::Client)),
        );
}

/// Keep the world's new decals (a new map starts the log over).
fn record(
    mut asks: MessageReader<PlaceDecal>,
    loads: Option<Res<crate::core::MapLoads>>,
    movable: Query<(), Or<(With<PropIndex>, With<MapBrushEntity>)>>,
    parents: Query<&ChildOf>,
    mut log: ResMut<DecalLog>,
) {
    let loads = loads.map_or(0, |l| l.0);
    if log.loads != loads {
        log.loads = loads;
        log.decals.clear();
    }
    for ask in asks.read() {
        let on_movable = ask.target.is_some_and(|t| {
            std::iter::once(t)
                .chain(parents.get(t).ok().map(|c| c.parent()))
                .any(|e| movable.contains(e))
        });
        // The round's own (the bomb's scorch) go at its restart.
        if on_movable || ask.round {
            continue;
        }
        log.decals.push_back(NetDecal::of(ask));
        while log.decals.len() > MAX_DECALS {
            log.decals.pop_front();
        }
    }
}

/// Send `client` the world's decals so far (it just came into the game).
pub(super) fn send(world: &mut World, client: Entity) {
    let list: Vec<NetDecal> = world.resource::<DecalLog>().decals.iter().cloned().collect();
    if list.is_empty() || world.get::<Player>(client).is_none() {
        return;
    }
    world.write_message(ToClients {
        targets: SendTargets::Single(ClientId::Client(client)),
        message: Decals { list },
    });
}

/// New sprays go to every client.
fn replicate_sprays(new: Query<Entity, (Added<Spray>, Without<Replicated>)>, mut commands: Commands) {
    for e in &new {
        commands.entity(e).insert(Replicated);
    }
}

/// Our `impulse 201`, to the server.
fn send_sprays(mut asked: MessageReader<SprayRequested>, mut out: MessageWriter<SprayRequest>) {
    for a in asked.read() {
        out.write(SprayRequest(a.0.clone()));
    }
}

/// Our player impulses (`impulse 100`), to the server.
fn send_impulses(mut asked: MessageReader<crate::weapon::PlayerImpulse>, mut out: MessageWriter<ImpulseRequest>) {
    for i in asked.read() {
        out.write(ImpulseRequest(i.impulse));
    }
}

/// A client's impulse, for its player.
fn receive_impulses(
    mut requests: MessageReader<FromClient<ImpulseRequest>>,
    players: Query<&Player>,
    mut out: MessageWriter<crate::weapon::PlayerImpulse>,
) {
    for r in requests.read() {
        let Some(client) = r.client_id.entity() else { continue };
        let Ok(p) = players.get(client) else { continue };
        out.write(crate::weapon::PlayerImpulse { player: p.character, impulse: r.message.0 });
    }
}

/// A client's spray, for its player (refusals say nothing, as in the
/// game).
fn receive_sprays(
    mut requests: MessageReader<FromClient<SprayRequest>>,
    players: Query<&Player>,
    mut commands: Commands,
) {
    for r in requests.read() {
        let Some(client) = r.client_id.entity() else { continue };
        let Ok(p) = players.get(client) else { continue };
        let character = p.character;
        let picture = r.message.0.clone();
        commands.queue(move |w: &mut World| {
            if w.get_entity(character).is_ok() {
                let _ = crate::weapon::spray::spray_with(w, character, picture);
            }
        });
    }
}

/// The server's decals, placed here.
fn receive(mut got: MessageReader<Decals>, mut place: MessageWriter<PlaceDecal>) {
    for d in got.read() {
        place.write_batch(d.list.iter().take(MAX_DECALS).filter_map(NetDecal::place));
    }
}
