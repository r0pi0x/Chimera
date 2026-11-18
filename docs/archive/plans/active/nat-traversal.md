# Plan: playing with friends behind NAT

Status (2026-10-13): research done; UPnP port mapping prototyped
(`net_upnp`, off by default, `net::upnp`); the rest is a recommendation
for the user. Answers multiplayer.md open question 3 (backlog,
"Decisions 2026-10-13": "investigate whether Steam networking
(relays/NAT traversal) is possible at all for our own client").

Today a host forwards UDP 27015 on its router, or everyone joins one
Tailscale/ZeroTier network (dist/README.txt, docs/OBSERVABILITY.md
"Network play"). Both need a technical host, and port forwarding can't
work behind carrier-grade NAT (CGNAT: the router's outside address is
itself private, common on mobile and some fibre/cable ISPs).

## 1. Steam networking: verdict

**Not usable by us today.** Steam's networking (ISteamNetworkingSockets
with the Steam Datagram Relay: relays through Valve's backbone, P2P with
Steam doing the signalling and NAT traversal) is reached through the
Steamworks API, and the Steamworks API only runs for an app the player
owns, under that app's id:

- "A running Steam client is required to provide implementations of the
  various Steamworks interfaces", and the API "will not initialize if it
  does not know the App ID of your game"; for testing, "ensure that you
  own a license for the App ID on the currently active Steam account"
  ([Steamworks API overview](https://partner.steamgames.com/doc/sdk/api)).
  So every player needs Steam running, logged in, owning the app id.
- **CS:S's app id (240) is out**: running under it makes Steam treat our
  program as Counter-Strike: Source (the friends list shows them playing
  CS:S, its relay certificates and tickets are CS:S's). That is
  presenting our program as another company's product, which our rules
  forbid, and the Steamworks agreements give the API to a partner for
  their own apps only.
- **Spacewar (480)** is Valve's sample app for the SDK examples
  (`steam_appid.txt` is a development override that "you should not ship
  with your builds", same page). Developers test with it before they
  have an app id; shipping a game on it is impersonating Valve's sample,
  and it is known as a pirates' disguise. Test-only, not for us.
- **An app id of our own**: Steam Direct, $100 a new app, recoupable
  after $1,000 of revenue, after the Steamworks partner paperwork
  (identity, tax, bank) ([Steam Direct fee](https://partner.steamgames.com/doc/gettingstarted/appfee)).
  Then the app is a Steam product (it can stay unreleased or be a free
  playtest), and our players would get it through Steam. SDR is offered
  to games on Steam; relays for non-Steam players come with "we cannot
  promise that this service will always be available to non-Steam
  players" ([Steam Datagram Relay](https://partner.steamgames.com/doc/features/multiplayer/steamdatagramrelay)).
  Risks beyond cost: a game built on another game's assets (loaded from
  the player's CS:S) going through Valve's review, Steam on every
  player's PC, the Steamworks SDK redistributable (`steam_api64.dll`/
  `libsteam_api.so`, loaded dynamically by the `steamworks` crate:
  "the libraries need to exist somewhere the operating system can find
  them", [steamworks-rs](https://github.com/Noxime/steamworks-rs)) shipped
  under that partner agreement, never committed here.
- **Code side, once there is an app id**: `renet_steam` 3.0.0 (renet's
  Steam transport over `steamworks` 0.12; MIT/Apache) plugs into the
  renet we already use ([docs.rs](https://docs.rs/renet_steam/latest/renet_steam/));
  a transport change plus a "join through Steam friends" flow. Medium
  work, small risk; the blocker is the app id, not the code.
- **The open-source GameNetworkingSockets** (Valve, BSD-3-Clause, C++,
  [GitHub](https://github.com/ValveSoftware/GameNetworkingSockets)) works
  without Steam: reliable and unreliable messages, fragmentation and
  retransmission, AES-GCM-256 encryption with Curve25519 key exchange,
  and P2P "NAT traversal through google WebRTC's ICE implementation" for
  which you "plug in your own signaling service". It does **not** give
  the relay network ("not able to offer access to SDR on other platforms
  to all partners"). For us it would replace renet (which already gives
  reliable UDP; encryption is netcode's secure mode) to get ICE, and we
  would still have to host the signalling service and a TURN relay for
  the pairs ICE can't connect. No gain over options (c)/(d) below for
  more work (a C++ dependency on three platforms).

## 2. Options, ranked

Effort: S ≈ a session, M ≈ 2-3, L ≈ 4+. "Reach": which hosts it helps.

| | Option | Effort | Running cost | Reach | Player experience |
|---|---|---|---|---|---|
| a | Docs: forward the port by hand, or Tailscale/ZeroTier (done) | done | none | all, but CGNAT needs the VPN | technical: router pages, or everyone installs a VPN client and joins the host's network (Tailscale free: 6 users; ZeroTier free: 10 devices) |
| b | **UPnP port mapping on the host's router** (prototyped: `net_upnp 1`) | S (done) | none | routers with UPnP on (most home routers ship with it; some ISPs' turn it off); never CGNAT | nothing to do when it works; `status` says the address to give friends, or why it failed |
| c | UDP hole punching with a rendezvous server we host | M-L | a tiny always-on service (any cheap VPS) | most pairs; not when both ends are behind "hard" (endpoint-dependent, symmetric) NATs, or UDP is blocked ([Tailscale: how NAT traversal works](https://tailscale.com/blog/how-nat-traversal-works): "over 90%" direct, the rest needs a relay) | join by a code or a list, no router setup; needs our service up |
| d | A relay we host (or simply the dedicated server on a rented machine) | relay: M; rented server: none (docs) | a VPS, a few euros a month; a relay adds a hop of latency | everyone | relay: as (c) but always works; rented server: everyone connects to a public address, the best experience for a regular group |
| e | Steam networking with an app id of our own | L plus the Steam process | $100 once, Steam's terms | everyone with Steam | invites through the friends list, relays, no addresses; needs Steam running and the app in each library |

Notes.

- (b) is what many PC games do (Source never did). Risks: it opens a
  port on the router while hosting (only ours, only UDP, with a one-hour
  lease renewed while serving and removed when serving stops), and some
  routers answer success and forward nothing. Double NAT (an ISP router
  in front of the home router) maps only the inner one: `status` warns
  when the router's outside address is private or CGNAT's 100.64/10.
  NAT-PMP/PCP (Apple, some others) is not covered; `natpmp` crates exist
  if a tester's router needs it.
- (c) needs changes in our UDP transport (`net::udp` is ours already):
  both ends register with the rendezvous service, which tells each the
  other's public address; both send first so their NATs open; then
  netcode's handshake runs as now. A connect-by-code flow in the menu.
- (d), the rented dedicated server, needs no code: `cs_source_server` runs
  headless on Linux (the capacity test: a 32-player de_dust2 server uses
  a few ms of CPU a tick, multiplayer.md "Capacity"); the box needs the
  maps (CS:S's dedicated server content, Steam app 232330 through
  SteamCMD with an anonymous login, has them; check our loader with that
  layout first: to do).

## 3. Recommendation

1. **Now:** keep (a); ship (b) as an opt-in (`net_upnp 1`, off by
   default: parity with Source, and it changes a router's settings); add
   it to Create Server as a checkbox if testers like it. Try it: host
   with `net_upnp 1` in the console before `map`/Create Server (or
   `cs_source_server ... +net_upnp 1`), then `status`: the `upnp` line gives
   the address to give friends, or why the router said no (the log too).
2. **For a regular group:** a rented Linux VPS running `cs_source_server`
   (d): no NAT for anyone, nothing to install for players. Document once
   the server's map layout is checked.
3. **Later, with a Lucker Party lobby:** (c) with (d)'s relay as the
   fallback, as one small service of ours (it also gives the lobby a
   home). (e) only if Lucker Party becomes a Steam app anyway (store
   review of a game built on CS:S content is the real question, not the
   code).

## 4. The prototype (b)

`chimera/net/src/net/upnp.rs`: `net_upnp` (0/1); while this process serves over UDP
it asks `PortMapping` (UPnP IGD through `igd-next` 0.18: SSDP search with
3 s timeouts, `AddPortMapping` of the same UDP port to this machine's
address toward the router, lease `LEASE_SECS` = 1 h renewed at half) on a
thread; `status` shows `upnp : UDP 27015 forwarded by router
192.168.1.1; friends connect to <public>:27015`, the reason it failed, or
the CGNAT warning; the mapping is removed when serving stops and when the
game quits. Tests (`games/cs_source/tests/it/net_upnp.rs`) use a fake router: no test
touches the real network.
