//! Asking the host's home router to forward the game port (UPnP IGD), so
//! friends can join over the internet without the host setting up port
//! forwarding by hand (docs/plans/active/nat-traversal.md, option b).
//!
//! `net_upnp 1` (0 by default: Source has no such thing, and it opens a
//! port on the router): while this process serves over UDP (`listen`, the
//! dedicated server), it asks the router for a mapping of the same UDP
//! port to this machine, renews the lease, and removes it when serving
//! stops or the game quits. `status` shows the result. Routers without
//! UPnP (or with it switched off) and carrier-grade NAT can't be helped
//! this way: the status line says so, and the host forwards by hand or
//! uses a VPN (Tailscale, ZeroTier).
//!
//! The router is reached through `PortMapping` (UPnP IGD with `igd-next`
//! by default); tests put in a fake one and never touch the network.
//!
//! Known shortcuts (docs/tech-debt.md): UPnP port mapping (`net::upnp`,
//! `net_upnp`, off by default) speaks UPnP IGD only: not NAT-PMP/PCP (Apple
//! and some other routers), not IPv6 pinholes, and only the router nearest
//! the host (double NAT behind an ISP box isn't opened; `status` warns when
//! the router's outside address is private or CGNAT's). Not tried against a
//! real router here (tests use a fake one). Why it's OK for now: A prototype
//! for testers; docs/plans/active/nat-traversal.md.

use std::{
    net::{IpAddr, SocketAddr, UdpSocket},
    sync::{Arc, Mutex, mpsc},
    time::Duration,
};

use bevy::prelude::*;

use crate::console::resource_cvar;

/// The lease asked for, s (renewed at half of it; a mapping left behind
/// by a crash goes by itself within this).
pub const LEASE_SECS: u32 = 3600;

/// What the router said to a mapping.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Mapped {
    /// The router's LAN address.
    pub router: String,
    /// The router's outside address, if it said.
    pub public_ip: Option<IpAddr>,
}

/// Something that forwards a UDP port on the router: UPnP IGD in the
/// game (`Igd`), a fake in tests. Calls block (seconds at most) and run
/// off the main thread.
pub trait PortMapper: Send + Sync + 'static {
    fn map(&self, port: u16, lease_secs: u32) -> Result<Mapped, String>;
    fn unmap(&self, port: u16) -> Result<(), String>;
}

/// The port mapper in use.
#[derive(Resource, Clone)]
pub struct PortMapping(pub Arc<dyn PortMapper>);

impl Default for PortMapping {
    fn default() -> Self {
        Self(Arc::new(Igd::default()))
    }
}

/// Where the mapping stands.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum UpnpState {
    /// Not asked (`net_upnp 0`, or not serving over UDP).
    #[default]
    Off,
    Asking(u16),
    Mapped(u16, Mapped),
    Failed(u16, String),
}

impl UpnpState {
    fn port(&self) -> Option<u16> {
        match self {
            UpnpState::Off => None,
            UpnpState::Asking(p) | UpnpState::Mapped(p, _) | UpnpState::Failed(p, _) => Some(*p),
        }
    }

    /// The `status` line.
    pub fn line(&self) -> Option<String> {
        Some(match self {
            UpnpState::Off => return None,
            UpnpState::Asking(p) => format!("asking the router to forward UDP {p}..."),
            UpnpState::Mapped(p, m) => {
                let public = match m.public_ip {
                    Some(ip) if behind_another_nat(ip) => format!(
                        "; its outside address {ip} is private too (carrier-grade NAT): friends can't reach it, use a VPN"
                    ),
                    Some(ip) => format!("; friends connect to {ip}:{p}"),
                    None => String::new(),
                };
                format!("UDP {p} forwarded by router {}{public}", m.router)
            }
            UpnpState::Failed(p, why) => format!("couldn't forward UDP {p}: {why}"),
        })
    }
}

/// `net_upnp` and the mapping's state.
#[derive(Resource, Default)]
pub struct Upnp {
    /// `net_upnp`: 1 asks, 0 doesn't.
    pub enabled: u8,
    pub state: UpnpState,
    /// A request in flight: its port, whether it renews a mapping, and
    /// where the answer comes.
    pending: Mutex<Option<(u16, bool, mpsc::Receiver<Result<Mapped, String>>)>>,
    /// When the lease is renewed (real time, s).
    renew_at: f64,
}

/// An outside address that is itself private (RFC 1918, or 100.64/10,
/// carrier-grade NAT's): the router is behind another NAT.
pub fn behind_another_nat(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => {
            let o = v4.octets();
            v4.is_private() || v4.is_loopback() || v4.is_link_local() || (o[0] == 100 && (64..128).contains(&o[1]))
        }
        IpAddr::V6(_) => false,
    }
}

pub(super) fn plugin(app: &mut App) {
    app.init_resource::<Upnp>().init_resource::<PortMapping>();
    resource_cvar::<Upnp, u8>(
        app,
        "chimera:upnp",
        "1: while hosting, ask the router (UPnP) to forward the game's UDP port to this machine",
        |u| &mut u.enabled,
    );
    app.add_systems(Update, keep_mapping).add_systems(Last, unmap_on_exit);
}

fn ask(mapper: &PortMapping, port: u16) -> mpsc::Receiver<Result<Mapped, String>> {
    let (tx, rx) = mpsc::channel();
    let m = mapper.0.clone();
    std::thread::spawn(move || {
        let _ = tx.send(m.map(port, LEASE_SECS));
    });
    rx
}

fn forget(mapper: &PortMapping, port: u16) {
    let m = mapper.0.clone();
    std::thread::spawn(move || {
        if let Err(e) = m.unmap(port) {
            warn!("UPnP: removing the mapping of UDP {port}: {e}");
        }
    });
}

/// Map the port this process serves on while `net_upnp` is 1; renew it;
/// remove it when serving stops (or `net_upnp` goes to 0).
fn keep_mapping(world: &mut World) {
    let now = world.resource::<Time<Real>>().elapsed_secs_f64();
    let want = world
        .get_resource::<super::udp::UdpServer>()
        .and_then(|s| s.local_addr())
        .map(|a| a.port())
        .filter(|_| world.resource::<Upnp>().enabled != 0);
    let mapper = world.resource::<PortMapping>().clone();
    let mut u = world.resource_mut::<Upnp>();
    // An answer.
    let answer = {
        let mut pending = u.pending.lock().unwrap();
        match pending.as_ref().map(|(p, renew, rx)| (*p, *renew, rx.try_recv())) {
            Some((port, renew, Ok(r))) => {
                *pending = None;
                Some((port, renew, r))
            }
            Some((_, _, Err(mpsc::TryRecvError::Disconnected))) => {
                *pending = None;
                None
            }
            _ => None,
        }
    };
    let current = u.state.port();
    if let Some((port, renew, r)) = answer.clone()
        && current == Some(port)
    {
        match r {
            Ok(m) => {
                if !renew {
                    let s = UpnpState::Mapped(port, m.clone());
                    info!("UPnP: {}", s.line().unwrap_or_default());
                }
                u.state = UpnpState::Mapped(port, m);
                u.renew_at = now + f64::from(LEASE_SECS) / 2.0;
            }
            Err(e) => {
                warn!("UPnP: couldn't forward UDP {port}: {e}");
                u.state = UpnpState::Failed(port, e);
            }
        }
    } else if let Some((port, _, Ok(_))) = answer {
        // Mapped after serving moved on: take it back.
        forget(&mapper, port);
    }
    if want != u.state.port() {
        if let UpnpState::Mapped(p, _) = u.state {
            forget(&mapper, p);
        }
        u.state = match want {
            Some(p) => {
                *u.pending.lock().unwrap() = Some((p, false, ask(&mapper, p)));
                UpnpState::Asking(p)
            }
            None => UpnpState::Off,
        };
    } else if let UpnpState::Mapped(p, _) = u.state
        && now >= u.renew_at
        && u.pending.lock().unwrap().is_none()
    {
        u.renew_at = now + f64::from(LEASE_SECS) / 2.0;
        *u.pending.lock().unwrap() = Some((p, true, ask(&mapper, p)));
    }
}

/// The game quitting while mapped: remove the mapping (waiting a moment
/// for the router; the lease ends it otherwise).
fn unmap_on_exit(exit: MessageReader<AppExit>, upnp: Res<Upnp>, mapper: Res<PortMapping>) {
    if exit.is_empty() {
        return;
    }
    if let UpnpState::Mapped(p, _) = upnp.state {
        let m = mapper.0.clone();
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            let _ = tx.send(m.unmap(p));
        });
        let _ = rx.recv_timeout(Duration::from_secs(3));
    }
}

/// UPnP IGD (`igd-next`): finds the router by SSDP on the LAN, maps the
/// port to this machine's address toward it.
#[derive(Default)]
pub struct Igd {
    gateway: Mutex<Option<igd_next::Gateway>>,
}

impl Igd {
    fn gateway(&self) -> Result<igd_next::Gateway, String> {
        let mut g = self.gateway.lock().unwrap();
        if let Some(gw) = g.as_ref() {
            return Ok(gw.clone());
        }
        let mut options = igd_next::SearchOptions::default();
        options.timeout = Some(Duration::from_secs(3));
        options.single_search_timeout = Some(Duration::from_secs(3));
        let gw = igd_next::search_gateway(options).map_err(|e| format!("no UPnP router answered ({e})"))?;
        *g = Some(gw.clone());
        Ok(gw)
    }
}

/// This machine's address toward `to` (no packet is sent).
fn local_ip_toward(to: SocketAddr) -> Result<IpAddr, String> {
    let s = UdpSocket::bind(("0.0.0.0", 0)).map_err(|e| e.to_string())?;
    s.connect(to).map_err(|e| e.to_string())?;
    Ok(s.local_addr().map_err(|e| e.to_string())?.ip())
}

impl PortMapper for Igd {
    fn map(&self, port: u16, lease_secs: u32) -> Result<Mapped, String> {
        let gw = self.gateway()?;
        let local = local_ip_toward(gw.addr)?;
        gw.add_port(
            igd_next::PortMappingProtocol::UDP,
            port,
            SocketAddr::new(local, port),
            lease_secs,
            "Lucker Party server",
        )
        .map_err(|e| format!("the router refused ({e})"))?;
        Ok(Mapped {
            router: gw.addr.ip().to_string(),
            public_ip: gw.get_external_ip().ok(),
        })
    }

    fn unmap(&self, port: u16) -> Result<(), String> {
        let gw = self.gateway()?;
        gw.remove_port(igd_next::PortMappingProtocol::UDP, port)
            .map_err(|e| e.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn carrier_grade_and_private_outside_addresses_are_spotted() {
        for ip in ["100.64.0.1", "100.127.255.254", "10.0.0.1", "192.168.1.1", "172.16.0.1"] {
            assert!(behind_another_nat(ip.parse().unwrap()), "{ip}");
        }
        for ip in ["100.63.0.1", "100.128.0.1", "8.8.8.8", "81.2.69.1"] {
            assert!(!behind_another_nat(ip.parse().unwrap()), "{ip}");
        }
    }
}
