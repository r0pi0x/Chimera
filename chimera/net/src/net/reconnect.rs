//! Joining again on its own when the server goes away: a client whose
//! server quit, crashed or stopped answering (not one that left by its own
//! `disconnect`, was kicked or refused, or failed to fetch the map) waits
//! on the menu in `Reconnecting` ("Connection lost - reconnecting...
//! (attempt N)", the server's reason under it, Cancel or Escape to stop),
//! asks the same server every `RETRY_EVERY` s whether it is back (a UDP
//! server's info query, `query::ServerQueries`; on the in-memory link
//! `memory::Link::serving`) and, once it answers, joins it as `connect`
//! does (the asset offer, the map, the pre-match lobby as for any join).
//! After `GIVE_UP_AFTER` s it stops: the failure box says why
//! (`NetEvent::GaveUpReconnecting`). `cl_autoreconnect 0` turns it off
//! (saved).
//!
//! Who decides: the client's connection watch (`client::watch_connection`,
//! `watch_join`) marks a loss (`Lost`) before it disconnects; `net::disconnect`
//! hands the mark and any reconnect under way to `after`. The server sees a
//! new connection (a new id; a stale twin of the same name and address is
//! dropped as for any rejoin: `server::drop_stale_twins`).
//!
//! Known shortcuts: a server killed without hanging up is noticed only
//! when the connection times out (`client::TIMEOUT_SECONDS`, 30 s), as
//! before; the reason shown for a clean shutdown is netcode's hang-up in
//! CS:S's words ("Server shutting down"), since the wire carries no reason.

use std::{
    net::SocketAddr,
    sync::{Arc, Mutex},
};

use bevy::prelude::*;

use super::{
    NetEvent,
    client::{JoinFailure, JoinProgress},
    query::{QueryState, ServerQueries},
};
use crate::{
    console::{Console, resource_cvar},
    core::NetRole,
};

/// Seconds between asking the server whether it is back.
pub const RETRY_EVERY: f64 = 3.0;

/// Seconds after the loss when reconnecting stops.
pub const GIVE_UP_AFTER: f64 = 120.0;

pub(super) fn plugin(app: &mut App) {
    app.init_resource::<ReconnectSettings>();
    resource_cvar::<ReconnectSettings, u32>(
        app,
        "cl_autoreconnect",
        "1: when the server goes away (quits, crashes, times out), join it again on its own once it is back \
         (every 3 s for 2 minutes); 0: back to the menu.",
        |s| &mut s.enabled,
    );
    app.world_mut().resource_mut::<Console>().archive("cl_autoreconnect");
    app.add_systems(Update, redial.run_if(resource_exists::<Reconnecting>));
}

/// `cl_autoreconnect`.
#[derive(Resource, Clone, Debug)]
pub struct ReconnectSettings {
    pub enabled: u32,
}

impl Default for ReconnectSettings {
    fn default() -> Self {
        Self { enabled: 1 }
    }
}

/// How to reach the server again.
#[derive(Clone)]
pub enum Redial {
    Udp(SocketAddr),
    /// The in-memory link (tests).
    Memory(Arc<Mutex<super::memory::Link>>),
}

impl std::fmt::Debug for Redial {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Redial::Udp(a) => write!(f, "Udp({a})"),
            Redial::Memory(_) => write!(f, "Memory"),
        }
    }
}

/// The server this client last connected to (kept after a disconnect).
#[derive(Resource, Clone, Debug)]
pub struct LastServer(pub Redial);

/// The connection was lost (not left, refused or failed): set by the
/// connection watch just before it disconnects.
#[derive(Resource)]
pub(super) struct Lost;

/// Reconnecting: waiting for the server to come back (the client is
/// standalone, on the menu), or joining it again (a client again).
#[derive(Resource)]
pub struct Reconnecting {
    /// Why the connection was lost (the server's or netcode's words).
    pub reason: String,
    /// Times the server was asked whether it is back.
    pub attempt: u32,
    /// The server's address (None on the in-memory link).
    pub address: Option<SocketAddr>,
    /// When the connection was lost (real s).
    pub since: f64,
    next: f64,
    to: Redial,
    probe: Option<ServerQueries>,
}

impl Reconnecting {
    /// The reconnect box's text.
    pub fn status(&self) -> String {
        format!(
            "Connection lost - reconnecting... (attempt {})\n{}",
            self.attempt.max(1),
            self.reason.trim_end_matches('.')
        )
    }
}

/// Stop reconnecting (Cancel, Escape, the player starting something else).
/// True if a reconnect was under way.
pub fn cancel(world: &mut World) -> bool {
    let was = world.remove_resource::<Reconnecting>().is_some();
    if was {
        info!("reconnect cancelled");
    }
    was
}

/// Waiting for the server to come back (on the menu, not mid-join).
pub fn waiting(world: &World) -> bool {
    world.contains_resource::<Reconnecting>() && world.get_resource::<NetRole>() == Some(&NetRole::Standalone)
}

fn now(world: &World) -> f64 {
    world.get_resource::<Time<Real>>().map_or(0.0, |t| t.elapsed_secs_f64())
}

/// A client's game ended (`net::disconnect`, after `client::ended`): a
/// lost connection starts reconnecting (or goes on with the one under way,
/// `prior`); anything else ends it.
pub(super) fn after(world: &mut World, reason: &str, lost: bool, prior: Option<Reconnecting>) {
    let enabled = world.get_resource::<ReconnectSettings>().is_some_and(|s| s.enabled != 0);
    if !lost || !enabled {
        return;
    }
    let Some(LastServer(to)) = world.get_resource::<LastServer>().cloned() else {
        return;
    };
    let t = now(world);
    let r = match prior {
        // A join of the server that answered failed too: ask again later.
        Some(mut p) => {
            p.next = t + RETRY_EVERY;
            p
        }
        None => {
            warn!("connection lost ({reason}): reconnecting every {RETRY_EVERY} s");
            Reconnecting {
                reason: reason.to_string(),
                attempt: 0,
                address: match &to {
                    Redial::Udp(a) => Some(*a),
                    Redial::Memory(_) => None,
                },
                since: t,
                next: t,
                to,
                probe: None,
            }
        }
    };
    // The reconnect box shows it, not the failure box.
    world.resource_mut::<JoinProgress>().failure = None;
    world.insert_resource(r);
}

/// Ask the server whether it is back, every `RETRY_EVERY` s; join it once
/// it answers (`rejoin`); give up after `GIVE_UP_AFTER` s.
fn redial(
    role: Res<NetRole>,
    settings: Res<ReconnectSettings>,
    time: Res<Time<Real>>,
    mut r: ResMut<Reconnecting>,
    mut commands: Commands,
) {
    if *role != NetRole::Standalone {
        // Joining it again (or the player hosts a game).
        return;
    }
    if settings.enabled == 0 {
        commands.queue(|w: &mut World| {
            cancel(w);
        });
        return;
    }
    let t = time.elapsed_secs_f64();
    if t - r.since > GIVE_UP_AFTER {
        commands.queue(give_up);
        return;
    }
    let due = t >= r.next;
    if due {
        r.attempt += 1;
        r.next = t + RETRY_EVERY;
    }
    let r = &mut *r;
    let back = match &r.to {
        Redial::Udp(addr) => {
            let probe = r.probe.get_or_insert_with(ServerQueries::default);
            probe.poll();
            let answered = probe.result(*addr).is_some_and(
                |q| matches!(&q.state, QueryState::Answered { info, .. } if info.is_ours()),
            );
            if due && !answered {
                probe.forget(*addr);
                probe.query(*addr);
            } else if answered {
                // Ask afresh next time, should this join fail too.
                probe.forget(*addr);
            }
            answered
        }
        Redial::Memory(link) => due && link.lock().unwrap().serving,
    };
    if back {
        info!("the server is back (attempt {}): joining it again", r.attempt);
        commands.queue(rejoin(r.to.clone()));
    }
}

/// Join the server again, as `connect` does.
fn rejoin(to: Redial) -> impl FnOnce(&mut World) {
    move |world: &mut World| {
        if world.get_resource::<NetRole>() != Some(&NetRole::Standalone) {
            return;
        }
        let joined = match to {
            Redial::Udp(addr) => super::client::connect(world, addr),
            Redial::Memory(link) => {
                let id = link.lock().unwrap().fresh_id();
                super::memory::join(world, link, id)
            }
        };
        if let Err(e) = joined {
            warn!("reconnecting: {e}");
        }
    }
}

/// Reconnecting took too long: the failure box says why.
fn give_up(world: &mut World) {
    let Some(r) = world.remove_resource::<Reconnecting>() else { return };
    let text = format!(
        "{}\nThe server didn't come back ({} attempts in {} s).",
        r.reason,
        r.attempt,
        GIVE_UP_AFTER as u32
    );
    warn!("gave up reconnecting: {text}");
    {
        let mut p = world.resource_mut::<JoinProgress>();
        p.failure = Some((JoinFailure::Dropped, text.clone()));
    }
    world.write_message(NetEvent::GaveUpReconnecting(text));
}
