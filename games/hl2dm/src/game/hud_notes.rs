//! What the server tells one player's HUD about its pickups (hud.md 9,
//! weapons.md 2): the item message naming a kit's or battery's class
//! (`pickups::take`), and "ammo denied", for an ammo box
//! (`pickups::take`) or a carried weapon's type (`weapon::drop::AmmoFull`)
//! at its limit. `HudPickup` here, for its player; to a network client
//! `PickupNoteMsg`, which is `HudPickup` again there for the local player.
//! The HUD reads it (`ui::systems::collect`).

use bevy::prelude::*;
use bevy_replicon::prelude::*;
use chimera_net::net::parts::NetAppExt;
use chimera_net::net::server::Player;
use chimera_sim::weapon::drop::AmmoFull;
use serde::{Deserialize, Serialize};

use super::weapons::inventory::Hl2Item;
use crate::core::{LocalPlayer, NetRole};

/// One thing a HUD's history shows from the server.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum PickupNote {
    /// An item's class (`item_battery`, `item_healthkit`, `item_healthvial`).
    Item(String),
    /// A pickup refused: this ammo type is full.
    AmmoDenied(String),
}

/// `note` for `player`'s HUD.
#[derive(Message, Clone, Debug, PartialEq, Eq)]
pub struct HudPickup {
    pub player: Entity,
    pub note: PickupNote,
}

/// A `PickupNote`, on the wire to its player's client.
#[derive(Message, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PickupNoteMsg(pub PickupNote);

/// A carried weapon's type is at its limit: the notice names the type.
fn denied_weapons(
    mut full: MessageReader<AmmoFull>,
    items: Query<&Hl2Item>,
    mut out: MessageWriter<HudPickup>,
) {
    for f in full.read() {
        if let Some(ammo) = items.get(f.weapon).ok().and_then(|i| i.ammo) {
            out.write(HudPickup { player: f.who, note: PickupNote::AmmoDenied(ammo.to_string()) });
        }
    }
}

/// The server: this tick's notes, each to its player's client.
fn forward(
    mut notes: MessageReader<HudPickup>,
    clients: Query<(Entity, &Player)>,
    mut out: MessageWriter<ToClients<PickupNoteMsg>>,
) {
    for n in notes.read() {
        if let Some((client, _)) = clients.iter().find(|(_, p)| p.character == n.player) {
            out.write(ToClients {
                targets: SendTargets::Single(ClientId::Client(client)),
                message: PickupNoteMsg(n.note.clone()),
            });
        }
    }
}

/// A client: the server's notes are the local player's.
fn receive(
    mut notes: MessageReader<PickupNoteMsg>,
    me: Query<Entity, With<LocalPlayer>>,
    mut out: MessageWriter<HudPickup>,
) {
    let Some(player) = me.iter().next() else {
        notes.clear();
        return;
    };
    for n in notes.read() {
        out.write(HudPickup { player, note: n.0.clone() });
    }
}

fn register(app: &mut App) {
    app.add_server_message::<PickupNoteMsg>(Channel::Ordered)
        .add_systems(PostUpdate, forward.before(ServerSystems::Send).run_if(resource_equals(NetRole::Server)))
        .add_systems(PreUpdate, receive.after(ClientSystems::Receive).run_if(resource_equals(NetRole::Client)));
}

pub(super) fn plugin(app: &mut App) {
    app.add_message::<HudPickup>()
        .add_message::<AmmoFull>()
        .add_systems(FixedUpdate, denied_weapons.after(crate::core::SimSet::Weapons).run_if(crate::core::authoritative))
        .net_part("hl2dm_hud_notes", register);
}
