//! Network play (docs/plans/active/multiplayer.md): an authoritative
//! server (a listen server in the game, or the dedicated `mashup_server`)
//! and clients, over bevy_replicon with renet's netcode UDP transport.
//!
//! So far (slices 1 to 7): connect and disconnect (`connect`,
//! `disconnect`, `listen`, `status`; `ip`, `hostport`, `maxplayers`, `name`),
//! a join handshake (build version, protocol hash, then the map by name
//! and file hash), a character spawned on the server per client, and
//! characters replicated to every client (`NetCharacter`, `NetBody`,
//! `Team`, `Health`). A client sends a user command per server tick
//! (`UserCmds`), runs its clock ahead of the server's so each arrives in
//! time, predicts its own player and corrects it from the server's state
//! (`OwnState`): `predict`. Others are drawn in the past from a buffer of
//! snapshots keyed by server tick (`cl_interp`): `interp`. Moving brushes
//! replicate their motion state (`NetMover`) and a client steps them to
//! the tick it predicts, carrying its own player as the server does:
//! Source's logic's part (`logic::net`). Physics props are drawn from
//! snapshots like others: `props`.
//! Weapons: the client predicts its own with its movement (what it
//! carries goes in `OwnState`), the server hits with lag compensation,
//! others' shots, items, grenades and deaths come from the server:
//! `weapons`.
//!
//! Scores, money, chat and cvars: `game`, `chat`, `cvars`; a game's own
//! rules (CS:S's rounds, bomb and hostages) are its part (`parts`). Bots
//! are the server's like any character (`core::Bot`, `BotHooks`). Maps (`maps`): the server
//! offers its map (name, hash, size); a client without that file fetches
//! it over the connection or from `sv_downloadurl` into the content
//! cache, and a map change on the server takes every client along.
//! Breakables (`breakables`): broken windows' panes replicate and what
//! breaking throws (gibs, shards, falling panes) goes to every client.
//! Joining's stages, for the loading dialog: `client::JoinProgress`.
//!
//! `NetRole` (core) says what this process is; `authoritative` systems
//! (rules, bots, damage, logic) don't run on a client.
//!
//! Network play (docs/plans/active/multiplayer.md, slices 1-7): `NetPlugin`
//! (replicon + renet netcode over UDP, `AuthMethod::Custom`, a mutate message
//! with every update), the protocol in registration order (`Join`, `Refused`,
//! `Welcome` (with the map's offer: hash, size, `sv_downloadurl`,
//! `sv_allowdownload`, which the player's `cl_downloadfilter none` refuses; the
//! server's name and players; the match manifest, `manifest`), `ChangingLevel`,
//! `ChangeLevel` (with the manifest), `ChangeMatch` (the match changed on the
//! same map: its manifest), `Loaded` (a
//! client has the map: the server lets its player into the game), `MapRequest`,
//! `MapAck`, `MapChunk`, `MapDenied`, `UserCmds` of `NetCmd` (one per server
//! tick, made safe by `NetCmd::apply`, with the view tick its shots are traced
//! at; and the newest own state the client holds, `ack`), `OwnStateDelta` (an
//! `OwnState` as a `delta` against the acknowledged one), `OwnState` (with the
//! map index of the mover stood on, the map camera viewed through:
//! `core::MapView`, and the server's changes outside the player's commands:
//! `core::Rebased`, a restart); replicated `NetCharacter`, `Spectating`, `Unassigned`,
//! `ChoosingClass`, `PlayerClass`, `NetBody` (quantized: `quant`;
//! `body_flags::TELEPORTS` counts the server's teleports), `Team`, `Health`,
//! `slots::GameIdentity` (whose body a character wears), the map render looks `map::RenderLook` (players) and
//! `BrushTint` (movers), `NetProp`, `NetHold`, `NetHeld`, `NetItem`, `NetSmoke`, the body,
//! prop and item ones also into each entity's `interp::Snapshots` by
//! server tick, all written in tick order (`ordered`); weapons' messages
//! `FireBullets`, `WeaponFx`, `Detonation`, `Senses`, `Killed`, `HitConfirm`,
//! `Hurt`, `DropRequest`; game rules' `NetScore`, `BuyRequest`, `ServerMotd`
//! (`motd.rs`), `SayRequest`, `RadioRequest`, `Notice`, `ChatMessage`,
//! `RadioCall`, `ServerSound`, `CvarValues`, `NameRequest`, `NameChanged`,
//! `decals::Decals`, voice (`voice::VoiceData`, `VoiceRelay`), votes' `CallVote`,
//! `CastVote`, `VoteNews` (`vote`), `weapon_params::WeaponParams` (a weapon id's
//! server-decided parameters, once per id per client), `server_docs::DocsRequest`,
//! `DocsChunk`, `DocsDenied` (the server's data documents a client's
//! differ from: `server_docs`); then the parts' (`parts`, in name order: `minigame_net`'s
//! `NetMinigame`, `MinigameNews`, `InMinigame` and `Rung`; CS:S's
//! `NetRound`, `NetBomb`, `NetHostage`, `TeamRequest`, `ClassRequest`,
//! `ObjectiveNews`, `RoundOver`; Source's logic's `NetMover`, `MapHud`); `OwnState` also carries
//! money, armour, kit and the freeze's end tick (arming and defusing the bomb
//! are in its predicted state; `epoch`: the client clock epoch its `lead` is
//! from)), `canonical_position` (a predicted player's zeros as +0.0 at the end
//! of a tick), `NetSettings` (cvars `ip`, `hostport` 27015, `maxplayers`, `name`),
//! the server's `status` with `hostname` and query counts, `NET_VERSION` (also
//! in replicon's protocol hash), `NetEvent` for the client layer, `disconnect`,
//! `status`, the `connect`/`listen`/`status`/`ping`/`users` commands, the map
//! id and hash check (`map_matches`).
//!
//! Known shortcuts (docs/tech-debt.md): The network so far
//! (docs/plans/active/multiplayer.md, slices 1-7): a client steps movers ahead
//! with what it knows, so a mover starting, reversing after its wait, blocked
//! by someone else, or a train turning at a path corner mispredicts for a round
//! trip (corrected; the rest of the ride doesn't); parented brushes and
//! breakables aren't stepped ahead (drawn at their last snapshot); model doors
//! (`prop_door_rotating`) and prop skins/sequences aren't
//! replicated yet (props' pose and presence are, loose weapons and grenades in
//! flight too); others riding a mover are drawn in the past while the mover is
//! drawn at the predicted tick, so they float or sink by the mover's travel
//! over the delay (Source draws movers in the past instead); a predicted
//! player's position is written with +0.0 for -0.0 at the end of each tick in a
//! network game (`net::canonical_position`: the physics' transform sync does
//! that only sometimes, which a replay can't know); replicon's message ticks
//! are 32-bit (the server's tick wraps after 776 days); the server sends the
//! player's predicted state with each of its updates as a byte delta against
//! the newest one the client acknowledged (`net::delta`; ~25-90 bytes, the
//! whole ~400-650 when joining and after a map change), not quantized (the
//! client compares it bit for bit and replays from it); others' bodies
//! (`NetBody`) are quantized (`net::quant`: 1/32 unit, 1/8 unit/s, 16-bit
//! angles, ~20 bytes), props, items, movers and ragdolls are not; updates are
//! paced per client (`net::server::UpdatePacing`): `cl_updaterate` 20 and
//! `rate` 30000 (userinfo) within `sv_minupdaterate` 10, `sv_maxupdaterate` 66,
//! `sv_minrate` 5000 and `sv_maxrate` 0 (no limit) are the community's
//! documented CS:S defaults, unmeasured (spec session), the rate counts
//! replicon's message bytes (not renet's headers and acknowledgements) and a
//! client may owe or save up a quarter second of it, a client's mutations are
//! dropped on ticks it isn't due an update (reliable updates and messages
//! always go), its own state goes with its updates only; no transport
//! compression (deflate on each client's frame of messages saves ~9 % at 20
//! updates a second: not worth a dependency on both transports); the client's
//! fixed tick is nudged by adding or removing overstep, so `Time<Fixed>`'s
//! elapsed no longer matches the server's (`SimClock` is set from the server's
//! instead); `net_fakelag`/`net_fakeloss` apply only on a client's UDP
//! transport (not a server's, not `NetSim`'s link); weapons: lag compensation
//! doesn't rewind grenades' blasts, and a client's own traces test others' colliders a frame or two behind where
//! it draws them (cosmetic: the server's hits count), others' reload, draw and
//! knife sounds aren't sent (their fire sounds are), map logic's long-lived
//! sounds go to clients as they start, change and stop (`net::mapfx`), so a
//! client joining later doesn't hear loops already playing, a model-less
//! character is rewound as its box (the present trace uses its capsule),
//! `FireBullets` is unreliable (a lost one is a shot not drawn, as in Source);
//! rules (slice 5): a client's round clock and bomb timer run on the server's
//! clock as its updates arrive (about the one-way latency behind the server's,
//! ~50 ms at 100 ms ping); arming and defusing the bomb are predicted, but the
//! planted bomb itself, the HUD's words for them and a defuse someone else
//! starts come from the server (a round trip late), and a client's defuse
//! checks the bomb's use box against its own world (others' props drawn in the
//! past could differ: a correction); the freeze's start is heard a round trip
//! late (its end is known in advance and isn't mispredicted); objective events
//! and the rules' sounds go to every client, each client keeps what concerns it
//! (a counter-terrorist's client hears of the bomb being dropped, but shows
//! nothing); a player whose client is loading the map is out of the game
//! (`core::Connecting`), then comes in at once in deathmatch, in the freeze
//! time and when nobody of its team is alive (ours: CS:S restarts the game when
//! an empty team gets a player), else at the next round (CS:S's exact join
//! rules, e.g. a grace after the freeze, need the spec session); a team change
//! mid-round waits for the next round; `mp_limitteams` is checked in network
//! games only (single player kept as it was), with our words; chat and radio
//! flood limits are ours (a burst of 4 lines then one a second; 3 calls then
//! one every 1.5 s; the sender told in our words), CS:S's unmeasured; name
//! changes aren't rate-limited and a name another player has isn't made unique;
//! the dedicated server has no `RadioCommands` unless a map gives them, so any
//! short radio name is passed on; replicated cvars are polled every frame on
//! the server (~100 getters) rather than marked when set; bots (slice 6): names
//! from the install's botprofile.db ("Bot N" without it, e.g. on the greybox)
//! pick among the free profiles of `bot_difficulty` by the bot's seed (CS:S at
//! random), `bot_prefix` goes before the name with a space (unchecked against
//! CS:S), profiles' other attributes (skill, weapon preferences, voice) aren't
//! used, single player names its bots the same way; `bot_join_after_player`
//! keeps the quota from adding bots until a human is in the game but kicks none
//! when the last one leaves; quota bots join the smaller team, the terrorists
//! on a tie (CS:S picks at random); maps (slice 7): a `changelevel` to the same
//! map isn't announced to clients while the server loads it (they hear
//! `ChangeLevel` once it is in), only the map file is downloaded (content
//! outside its pakfile is not), the server reads the whole file into memory to
//! serve it and sends at most 64 KB a frame per client with 512 KB in flight on
//! the reliable ordered channel the chat also uses (a 150 MB community map
//! takes about 40 s on a good link; no congestion control beyond the window), a
//! client tells refusals apart by the server's words (`client::classify`;
//! `Refused` has no kind field, so older builds can still read it), late
//! joiners get the world's impact decals (not those on props or brush entities,
//! nor which variant each took; a client with `chimera_round_cleardecals 1` also
//! gets earlier rounds'); quitting through the remote protocol (`chimera/console
//! quit`) skips netcode's disconnect packet, so the server notices only when
//! the client times out; soak (multiplayer.md, "Soak";
//! games/cs_source/tests/it/heavy/net_soak.rs): replicon (0.44.3) holds a client's mutations
//! back until the update they follow arrives and by default applies only each
//! entity's newest one, though it acknowledged them all on arrival, so on lossy
//! links a value changed once on an entity whose body changes every tick was
//! lost for good (a dead player's last health, a score one kill short, every
//! player's 0/0 after a map change): the client asks replicon for the older
//! ones too and applies each component's values in tick order (`net::ordered`,
//! a receive marker with `need_history`; this replaced resending every changed
//! value for a second, ~12-20 KB/s a client), but replicon passes older
//! mutations only within 64 ticks of the entity's newest, so an update held up
//! over a second (three resends lost in a row at 300 ms) would still lose a
//! value changed once before it (not seen in the soaks); bandwidth is ~10-13
//! KB/s per client with twelve characters on de_dust2 at the default 20 updates
//! a second (peaks ~31-33 KB/s at joins and map changes; was 82-98 and 105-108
//! at 300 ms with 5 % loss), ~28-34 KB/s with an update every tick (47 at 300
//! ms/5 %: replicon sends a value with every update until it is acknowledged);
//! prediction errors come mostly from walking into other players and physics
//! props (drawn in the past; 0.4-0.8 % of states otherwise; contacts and hits
//! add ~2 %), and runs of up to ~80 in a row (1.2 s) happen where the client
//! has its player falling that the server stands on ground, not near another
//! player or a prop (not pinned down) and from damage's slowdown and blasts'
//! pushes (the server's), a respawn takes a round trip to predict right again;
//! a dedicated server still plays (silent) looping sounds for nobody, logging
//! each start at info level (1,111 lines in 17 minutes; it makes no player
//! ragdolls since 2026-11-12, `dedicated`); a client's main thread stalls 3-4 s putting a new map in (12 s
//! and a 5 s frame after it on this machine under load; during its loading
//! screen, packets wait in the socket meanwhile, and a stall over 30 s drops
//! the client) and `net_fakejitter` then reorders the whole burst, so renetcode
//! drops ~150 as replayed (only with the fake jitter); on this loaded box a
//! local client's ping reads ~60 ms and each client missed ~7 % of ticks'
//! commands over a 12-minute live run (most during map loads). Why it's OK for
//! now: Slice 8 and the open questions replace the rest; the rules are tested
//! in `games/cs_source/tests/ui/net_rules.rs`; commands, clock sync, prediction and
//! reconciliation are tested (`games/cs_source/tests/it/net_prediction.rs`), the join, map
//! check and replication too (`games/cs_source/tests/it/net.rs`).

pub mod anchors;
pub mod asset_startup;
pub mod assets;
pub mod breakables;
pub mod chat;
pub mod cheats;
pub mod client;
pub mod cvars;
pub mod decals;
pub mod delta;
pub mod formats;
pub mod game;
pub mod http;
pub mod interp;
pub mod mapfx;
pub mod manifest;
pub mod maps;
pub mod memory;
pub mod motd;
pub mod on_demand;
pub mod ordered;
pub mod own_items;
pub mod parts;
pub mod predict;
pub mod props;
pub mod quant;
pub mod query;
pub mod ragdolls;
pub mod reconnect;
pub mod server;
pub mod stats;
pub mod udp;
pub mod upnp;
pub mod voice;
pub mod vote;
pub mod server_docs;
pub mod weapon_params;
pub mod weapons;

use std::net::SocketAddr;

use bevy::{prelude::*, state::app::StatesPlugin};
use bevy_replicon::prelude::*;
use bevy_replicon_renet::{RenetChannelsExt, RenetClient, RenetServer, RepliconRenetPlugins, renet::ConnectionConfig};
use serde::{Deserialize, Serialize};

pub use self::parts::{BotHooks, Holds, JoinRules, NetAppExt, NetHooks, NetParts};
use self::ordered::OrderedAppExt;
use crate::{
    console::{ConsoleAppExt, resource_cvar},
    core::{Health, NetRole, Team},
};

/// Source's default port; the game and the dedicated server listen here
/// unless `hostport` says otherwise.
pub const DEFAULT_PORT: u16 = 27015;

/// The netcode protocol id: Lucker Party's, the same for every build, so a
/// client of another build still reaches the server and hears why it was
/// refused (`Join::version`) instead of timing out.
pub const PROTOCOL_ID: u64 = 0x4C55_434B_4552_5059;

/// This build's network version. A server refuses clients of another
/// version. Bump the suffix when the protocol changes in a way the
/// replicon protocol hash can't see (a field added to a message).
pub const NET_VERSION: &str = concat!(env!("CARGO_PKG_VERSION"), "/net49");

/// What a host's `changelevel` is in this app beyond a new map
/// (`host_changelevel`): set by an app part (`minigame_net`'s, a party
/// app's: the minigame's next match there, with its lobby).
#[derive(Resource, Clone, Copy)]
pub struct HostChangeLevel(pub fn(&mut World, &str) -> Option<Result<Option<String>, String>>);

/// A host's `changelevel` to `map` (the app's map id) as the app takes it
/// (`HostChangeLevel`); None: a plain level change. The apps'
/// `changelevel` commands ask this first.
pub fn host_changelevel(world: &mut World, map: &str) -> Option<Result<Option<String>, String>> {
    let hook = world.get_resource::<HostChangeLevel>()?.0;
    hook(world, map)
}

/// What a host's `changelevel` with no map is in this app
/// (`host_next_level`): set by an app part (`minigame_net`'s: the
/// minigame's next match, its map drawn as a rotation's).
#[derive(Resource, Clone, Copy)]
pub struct HostNextLevel(pub fn(&mut World) -> Option<Result<Option<String>, String>>);

/// A host's `changelevel` with no map as the app takes it
/// (`HostNextLevel`); None: the command needs a map. The apps'
/// `changelevel` commands ask this when given none.
pub fn host_next_level(world: &mut World) -> Option<Result<Option<String>, String>> {
    let hook = world.get_resource::<HostNextLevel>()?.0;
    hook(world)
}

/// The owner id of the listen server's own player (`NetCharacter::owner`).
/// Remote clients' ids are never 0.
pub const HOST_ID: u64 = 0;

/// The name of the greybox map in the handshake (and `map greybox`).
pub const GREYBOX: &str = "greybox";

/// A predicted player's position at the end of a tick (server and client)
/// and of each replayed one, its zeros written as +0.0. The physics'
/// transform sync after a live tick turns a -0.0 there into 0.0 or not,
/// depending on where the physics last had the body (avian's tolerance),
/// which a replay can't know; the two are the same position, but the
/// state is compared bit for bit and feeds the next tick (the movement's
/// own state keeps the sign). Network games only: single player keeps
/// its values as they were.
///
/// The physics' copy of the position (`Position`) is set to the same
/// value: the physics takes a moved transform only when it moved more
/// than its tolerance, and writes its own copy back after its step, so a
/// stale copy (e.g. a client's from before the server's state was put
/// over its transform) would undo a small correction every tick: a
/// player standing still 1 ULP away from the server's spot (below the
/// physics' 0.01 mm tolerance) mispredicted on every tick (found by the
/// soak, games/cs_source/tests/it/heavy/net_soak.rs). Steps that small within a tick are
/// kept by `core::keep_moved_positions`.
pub fn canonical_position(world: &mut World, e: Entity) {
    let Some(mut t) = world.get_mut::<Transform>(e) else { return };
    let canonical = t.translation + Vec3::ZERO;
    if canonical.to_array().map(f32::to_bits) != t.translation.to_array().map(f32::to_bits) {
        t.translation = canonical;
    }
    if let Some(mut p) = world.get_mut::<avian3d::prelude::Position>(e)
        && p.0.to_array().map(f32::to_bits) != canonical.to_array().map(f32::to_bits)
    {
        // Unmarked: a changed `Position` would win over the next tick's
        // moved transform in the physics' sync (avian's
        // `transform_to_position` keeps a position changed since its last
        // step), putting the player a tick behind.
        p.bypass_change_detection().0 = canonical;
    }
}

/// Network settings (console: `ip`, `hostport`, `maxplayers`, `name`).
#[derive(Resource, Clone, Debug)]
pub struct NetSettings {
    /// The address a server listens on (Source's `ip`): every interface
    /// by default; tool runs (`--screenshot`, `--views`, `--frames`) use
    /// loopback.
    pub ip: std::net::IpAddr,
    /// UDP port a server listens on (0: one the system picks).
    pub hostport: u16,
    /// Players a server takes, the host's own included on a listen
    /// server (Source's `maxplayers`; 1 is single player).
    pub maxplayers: u32,
    /// This player's name (Source's `name`). (The server's name,
    /// `hostname`, is `query::Hosting`'s.)
    pub name: String,
}

impl Default for NetSettings {
    fn default() -> Self {
        Self {
            ip: std::net::Ipv4Addr::UNSPECIFIED.into(),
            hostport: DEFAULT_PORT,
            maxplayers: 1,
            name: "Player".into(),
        }
    }
}

/// The version this process says it is in `Join` (`NET_VERSION`; tests
/// set another to be refused).
#[derive(Resource, Clone, Debug)]
pub struct NetVersion(pub String);

impl Default for NetVersion {
    fn default() -> Self {
        Self(NET_VERSION.into())
    }
}

/// What happened on the network, for the client layer (menus, map loads)
/// and logs.
#[derive(Message, Clone, Debug, PartialEq)]
pub enum NetEvent {
    /// Listening on this address (`listen`, `map` with `maxplayers` > 1, or
    /// the dedicated server).
    Listening(SocketAddr),
    /// Connecting to this server.
    Connecting(SocketAddr),
    /// The server is on this map and this client hasn't loaded it: load it
    /// (the client layer; a test's `harness::serve_maps`), from `file`
    /// when given (a downloaded copy in the content cache), else by name;
    /// then the handshake checks its hash.
    LoadMap { map: String, file: Option<std::path::PathBuf> },
    /// The server is changing level (to this map): the loading dialog
    /// shows it; the new map comes as `LoadMap` once the server has it.
    ChangingLevel(String),
    /// Joined: the map matches, the server spawns our character.
    Joined { map: String },
    /// Left the game (or stopped serving): why.
    Disconnected(String),
    /// Reconnecting to a server that went away (`reconnect`) stopped: it
    /// didn't come back in time (why, for the failure box).
    GaveUpReconnecting(String),
}

// --- Protocol: messages and replicated components. Registered in the same
// order on every side (`NetPlugin::build`).

/// Client -> server, first thing after connecting: who we are.
#[derive(Event, Serialize, Deserialize, Clone, Debug)]
pub struct Join {
    /// `NET_VERSION` of the client's build.
    pub version: String,
    /// Replicon's hash of everything registered (types and order).
    pub protocol: ProtocolHash,
    pub name: String,
    /// The player's userinfo cvars (`core::UserInfo`), by name.
    pub userinfo: Vec<(String, String)>,
}

/// Server -> client: refused to join (version, full); the server
/// disconnects it right after.
#[derive(Event, Serialize, Deserialize, Clone, Debug)]
pub struct Refused {
    pub reason: String,
}

/// Server -> client: joined. The map (`map` id: `greybox` or
/// `cs_source:<name>`), its file's hash and size, which the client
/// checks (and fetches, `maps`) before it plays, where to download it
/// from, the tick the server runs at, and what the loading dialog shows
/// of the server.
#[derive(Event, Serialize, Deserialize, Clone, Debug, Default)]
pub struct Welcome {
    pub map: String,
    pub map_hash: Option<[u8; 32]>,
    /// The map file's size, bytes (0: none, the greybox).
    pub map_size: u64,
    /// `sv_downloadurl`: an HTTP folder laid out like the game's
    /// (`<url>/maps/<name>.bsp.bz2` or `.bsp`); empty: none.
    pub download_url: String,
    /// `sv_allowdownload`: the server sends the map over the connection.
    pub allow_download: bool,
    /// Length of a server tick, ns (exact: the client's fixed tick must
    /// be the same `Duration`).
    pub tick_nanos: u64,
    /// The client's id (`NetCharacter::owner` of its character).
    pub you: u64,
    /// `hostname`, players in the game (bots included) and `maxplayers`.
    pub server_name: String,
    pub players: u32,
    pub max_players: u32,
    /// What the match needs (`manifest`): the client checks it before it
    /// loads anything.
    pub manifest: manifest::MatchManifest,
}

/// Server -> clients: a map change started (`changelevel`, `map`); the
/// new map follows as `ChangeLevel` once the server has loaded it.
#[derive(Event, Serialize, Deserialize, Clone, Debug, Default)]
pub struct ChangingLevel {
    pub map: String,
}

/// Server -> clients: the server is on another map now: load it (fetch
/// it first if needed, as on joining) and play on; the connection and
/// the characters stay.
#[derive(Event, Serialize, Deserialize, Clone, Debug, Default)]
pub struct ChangeLevel {
    pub map: String,
    pub map_hash: Option<[u8; 32]>,
    pub map_size: u64,
    pub download_url: String,
    pub allow_download: bool,
    pub tick_nanos: u64,
    /// The match on the new map (`manifest`).
    pub manifest: manifest::MatchManifest,
}

/// Server -> clients: the match changed on the same map (another
/// minigame: `minigame_net::change_match`): its manifest, which each client
/// checks (`manifest::accept`) and plays on with, connected; the parts
/// follow from what they replicate.
#[derive(Event, Serialize, Deserialize, Clone, Debug, Default)]
pub struct ChangeMatch {
    pub manifest: manifest::MatchManifest,
}

/// Client -> server: I have the map loaded (`map`, the id the server
/// offered): put me in the game (`rules::enter_game`). Until then the
/// server keeps the player out of it (`core::Connecting`).
#[derive(Message, Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct Loaded {
    pub map: String,
}

/// Client -> server: send me your map's file (`sv_allowdownload`).
#[derive(Message, Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct MapRequest {
    pub map: String,
}

/// Server -> client: a piece of the map file it asked for, at `offset`
/// of `total` bytes. Reliable and in order.
#[derive(Message, Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct MapChunk {
    pub offset: u64,
    pub total: u64,
    pub data: Vec<u8>,
}

/// Client -> server: bytes of the map received so far (the server keeps
/// a window of `maps::WINDOW` bytes in flight beyond it).
#[derive(Message, Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct MapAck {
    pub received: u64,
}

/// Server -> client: it won't send the map, and why.
#[derive(Message, Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct MapDenied {
    pub reason: String,
}

/// One user command (Source's usercmd): the intent for one server tick,
/// with the look angles as the exact `f32`s the client simulated with.
/// Its command number is the tick plus the character's `Seed` offset
/// (`core::number_commands`) on both sides.
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct NetCmd {
    /// The server tick this command is for.
    pub tick: u64,
    pub move_axis: [f32; 2],
    pub yaw: f32,
    pub pitch: f32,
    /// `buttons::*` bits.
    pub buttons: u16,
    /// `Intent::actions`: the named actions held (`core::ActionRegistry`).
    pub actions: u32,
    pub select: Option<u8>,
    /// The server tick (fractional) the client drew others at when it made
    /// this command (`interp::InterpClock::render_tick`; 0: none, or
    /// `cl_lagcompensation 0`): the server traces its shots against others
    /// where they were then (`weapon::lagcomp`).
    pub view_tick: f64,
    /// The count of the server's last look turn the client has taken
    /// (`core::ViewSet`, `OwnState::look`); the server keeps its own look
    /// over this command's until it matches.
    pub look_ack: u8,
}

/// `NetCmd::buttons` bits (Chimera's own buttons; a game's named actions
/// are `NetCmd::actions`).
pub mod buttons {
    pub const JUMP: u16 = 1;
    pub const CROUCH: u16 = 1 << 1;
    pub const SPRINT: u16 = 1 << 2;
    pub const WALK: u16 = 1 << 3;
    pub const FIRE: u16 = 1 << 4;
    pub const SECONDARY: u16 = 1 << 5;
    pub const RELOAD: u16 = 1 << 6;
    pub const LAST_WEAPON: u16 = 1 << 7;
    pub const USE: u16 = 1 << 8;
    /// `Intent::opposed`.
    pub const OPPOSED_X: u16 = 1 << 9;
    pub const OPPOSED_Y: u16 = 1 << 10;
}

impl NetCmd {
    pub fn of(tick: u64, i: &crate::core::Intent) -> Self {
        use buttons::*;
        let mut b = 0;
        for (on, bit) in [
            (i.jump, JUMP),
            (i.crouch, CROUCH),
            (i.sprint, SPRINT),
            (i.walk, WALK),
            (i.fire, FIRE),
            (i.secondary, SECONDARY),
            (i.reload, RELOAD),
            (i.last_weapon, LAST_WEAPON),
            (i.use_key, USE),
            (i.opposed.x, OPPOSED_X),
            (i.opposed.y, OPPOSED_Y),
        ] {
            if on {
                b |= bit;
            }
        }
        Self {
            tick,
            move_axis: i.move_axis.to_array(),
            yaw: i.yaw,
            pitch: i.pitch,
            buttons: b,
            select: i.select,
            actions: i.actions,
            view_tick: 0.0,
            look_ack: 0,
        }
    }

    /// The view tick, made safe: finite, not ahead of `tick` (the
    /// command's own), None when not given.
    pub fn view(&self, tick: u64) -> Option<f64> {
        (self.view_tick.is_finite() && self.view_tick > 0.0).then(|| self.view_tick.min(tick as f64))
    }

    /// Write into an intent, made safe first (never trust a client): the
    /// move axis at most 1 long, finite angles, yaw in [0, 2 pi), pitch
    /// within straight up and down, a weapon slot key 0-9 or a pick
    /// (`Intent::pick`). The command
    /// number stays (stamped each tick). A client puts its own intent
    /// through this too before it simulates (`normalize`), so both sides
    /// simulate the same values.
    pub fn apply(&self, i: &mut crate::core::Intent) {
        use buttons::*;
        let finite = |v: f32| if v.is_finite() { v } else { 0.0 };
        let axis = Vec2::new(finite(self.move_axis[0]), finite(self.move_axis[1]));
        i.move_axis = axis.clamp_length_max(1.0);
        i.yaw = finite(self.yaw).rem_euclid(std::f32::consts::TAU);
        let half_pi = std::f32::consts::FRAC_PI_2;
        i.pitch = finite(self.pitch).clamp(-half_pi, half_pi);
        let on = |bit: u16| self.buttons & bit != 0;
        i.jump = on(JUMP);
        i.crouch = on(CROUCH);
        i.sprint = on(SPRINT);
        i.walk = on(WALK);
        i.fire = on(FIRE);
        i.secondary = on(SECONDARY);
        i.reload = on(RELOAD);
        i.last_weapon = on(LAST_WEAPON);
        i.use_key = on(USE);
        i.opposed = BVec2::new(on(OPPOSED_X), on(OPPOSED_Y));
        i.actions = self.actions;
        i.select = self.select.filter(|s| *s < crate::core::PICK_SLOTS * (crate::core::PICK_DEPTH + 1));
    }

    /// The intent as the server will simulate it (`of`, then `apply`),
    /// written back in place; returns the command for `tick`.
    pub fn normalize(tick: u64, i: &mut crate::core::Intent) -> Self {
        Self::of(tick, i).apply(i);
        Self::of(tick, i)
    }
}

/// Client -> server, every frame it ran ticks: its new commands and the
/// `CMD_BACKUP` before them again (Source's `cl_cmdbackup`), so a lost
/// packet costs nothing. Unreliable.
#[derive(Message, Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct UserCmds {
    /// Oldest first.
    pub cmds: Vec<NetCmd>,
    /// The client's clock epoch: how often its command clock has jumped or
    /// started over (`predict::CommandClock::epoch`). The server reports the lead of
    /// the newest epoch only, so commands sent before a jump never steer
    /// the clock after it.
    pub epoch: u32,
    /// The newest own state (`OwnState::tick`) this client holds (0:
    /// none): the server sends the next ones as deltas against it
    /// (`OwnStateDelta`).
    pub ack: u64,
}

/// Commands each `UserCmds` repeats besides the new ones.
pub const CMD_BACKUP: usize = 3;

/// Server -> the client a character belongs to, at its update rate
/// (`server::UpdatePacing`): its `OwnState` as a delta (`delta`) against
/// the one with tick `base` (0: against nothing), which the client said
/// it holds (`UserCmds::ack`), as Source's snapshots are deltas against
/// the last acknowledged one. The client rebuilds the exact bytes
/// (`predict::rebuild_own_states`), so its comparison stays bit for bit.
/// Unreliable; only the newest matters.
#[derive(Message, Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct OwnStateDelta {
    pub tick: u64,
    pub base: u64,
    pub delta: Vec<u8>,
}

/// The server's state of a client's own player after a tick (its
/// predicted components, `core::PredictedComponents::encode`), which the
/// client compares with what it predicted for that tick, and how the
/// client's commands are arriving (the clock sync: `lead`). Sent as an
/// `OwnStateDelta`; a client message (`Messages<OwnState>`) once rebuilt.
#[derive(Message, Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct OwnState {
    /// The server tick this state is after.
    pub tick: u64,
    /// The server's simulation time at that tick (`SimClock::now`) and its
    /// tick length, ns: the client runs each command at the same time.
    pub time_nanos: u64,
    pub tick_nanos: u64,
    /// The predicted components, encoded.
    pub state: Vec<u8>,
    /// The character's movement implementation (`slots::MovementSlot`).
    pub movement: String,
    /// The character's `Seed` (its command number offset).
    pub seed: u64,
    /// The rules hold it (dead): its commands do nothing but look.
    pub held: bool,
    /// In the game and alive: not dead (`core::Dead`, which a connecting
    /// player is too) and above 0 health. When it changes the server
    /// spawned, killed or let in the player outside its commands (moved
    /// it to a spawn point, gave or dropped its weapons): the client
    /// takes that state as a restart, not a prediction error.
    pub alive: bool,
    /// The smallest lead of the commands heard since the last `OwnState`:
    /// the newest command's tick less the server's last tick when it
    /// arrived (1: just in time; in a frame that catches up several ticks,
    /// the last but one it runs). None: none heard.
    pub lead: Option<i32>,
    /// The newest command tick heard.
    pub newest: u64,
    /// The clock epoch `lead` is from (`UserCmds::epoch`): the client
    /// counts only leads of its present epoch.
    pub epoch: u32,
    /// The map entity index (`map::MapBrushEntity`) of the mover the
    /// player stands on (`MovementState::ground`, an entity, isn't in
    /// `state`: ids differ), so a correction keeps it riding.
    pub ground: Option<u32>,
    /// Commands waiting for later ticks.
    pub buffered: u16,
    /// Freeze time holds the player until this tick (it may look and pick
    /// a weapon); `held` is the dead's hold (only the look).
    pub frozen_until: Option<u64>,
    /// What the HUD shows of the player that isn't predicted, as the
    /// games' own-state parts (`parts::OwnParts`: CS:S's money, armour,
    /// defusal kit and night vision), in one blob. (Arming and defusing the
    /// bomb are predicted: `objectives::bomb::Arming`, `Defusing` in
    /// `state`.)
    pub parts: Vec<u8>,
    /// Ticks run without this client's command (its last one repeated),
    /// and commands that came after their tick, since it joined.
    pub missed: u32,
    pub late: u32,
    /// The map camera the player views through (`core::MapView`: origin,
    /// rotation), if any.
    pub view: Option<([f32; 3], [f32; 4])>,
    /// The server's last turn of the player's look (`core::ViewSet`: count,
    /// yaw, pitch): a client that hasn't taken that count turns to it and
    /// echoes the count (`NetCmd::look_ack`).
    pub look: Option<(u8, f32, f32)>,
    /// The times the server changed the player's predicted state outside
    /// its commands (`core::Rebased`: a rules' weapon swap). When it
    /// changes, the client takes the state as a restart, as for `alive`.
    pub rebased: u32,
}

/// A character the server replicates: whose it is and its name. On a
/// client, the character's local components are added when it arrives
/// (`client::character_arrived`).
#[derive(Component, Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct NetCharacter {
    /// The player's client id (`HOST_ID` for a listen server's host), or
    /// None for a bot.
    pub owner: Option<u64>,
    pub name: String,
}

/// What clients draw a character from, written by the server every tick
/// from the simulation (`server::write_bodies`): position, velocity, look,
/// eye height and movement flags. A client keeps them by server tick
/// (`interp::Snapshots`) and draws others between two of them. Sent
/// quantized (`quant`: 1/32 unit, 16-bit angles); the server keeps the
/// quantized values (`NetBody::quantized`).
#[derive(Component, Clone, Debug, Default, PartialEq)]
pub struct NetBody {
    pub origin: [f32; 3],
    pub velocity: [f32; 3],
    pub yaw: f32,
    pub pitch: f32,
    pub eye: [f32; 3],
    /// `body_flags::*` bits.
    pub flags: u8,
}

/// `NetBody::flags` bits.
pub mod body_flags {
    pub const ON_GROUND: u8 = 1;
    pub const CROUCHING: u8 = 1 << 1;
    pub const ON_LADDER: u8 = 1 << 2;
    pub const DEAD: u8 = 1 << 3;
    /// Two bits counting the server's teleports of the body (a move no
    /// body makes in a tick: `map::interp::SNAP_SPEED`), as Source marks
    /// a teleport so clients don't interpolate across it: a client draws
    /// the newer snapshot at once when the count changed between two,
    /// however far apart in time they are (`cl_updaterate`, a loss).
    pub const TELEPORTS: u8 = 3 << 4;
    pub const TELEPORT_ONE: u8 = 1 << 4;
}

/// A physics prop's pose (engine space) and whether it is there, written
/// by the server on the prop's node after each tick (`props::write_props`);
/// a client draws its own copy (the node with the same `map::PropIndex`)
/// from these at the render time (`props`). Sent quantized (`quant`: 1/32
/// unit, 16-bit quaternion parts; the server keeps the quantized values,
/// `NetProp::quantized`), so a prop shaking below that grid sends nothing.
#[derive(Component, Clone, Debug, Default, PartialEq)]
pub struct NetProp {
    /// `map::PropIndex`.
    pub index: u32,
    pub origin: [f32; 3],
    pub rotation: [f32; 4],
    /// `prop_flags::*` bits.
    pub flags: u16,
}

/// A map-placed ragdoll's parts (`map::placed_ragdoll`): each body's
/// position and rotation (x, y, z, then the quaternion's x, y, z, w;
/// engine space), at most `ragdolls::MAX_PARTS`, by its node's
/// `map::PropIndex` (`ragdolls`).
#[derive(Component, Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct NetRagdoll {
    pub index: u32,
    pub parts: Vec<[f32; 7]>,
}

/// A placed weapon's anchor (`map::entities::MapAnchor`): its pose, by
/// the anchor's map entity index (`anchors`).
#[derive(Component, Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct NetAnchor {
    pub index: u32,
    pub origin: [f32; 3],
    pub rotation: [f32; 4],
}

/// `NetProp::flags` bits.
pub mod prop_flags {
    pub const VISIBLE: u16 = 1;
    pub const SOLID: u16 = 1 << 1;
}

/// A physics prop a player holds (`map::held::Held`, lift C6): by its
/// node's `map::PropIndex`, the holder's client id (`NetCharacter::owner`)
/// and where it is held relative to the holder's eye and view (position in
/// the view's frame, rotation after the view's, engine space). The
/// holder's client draws it there from its own predicted view; everyone
/// else draws the prop's `NetProp` (`props`). Written by the server after
/// each tick, removed when let go.
#[derive(Component, Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct NetHold {
    pub index: u32,
    pub holder: u64,
    pub offset: [f32; 3],
    pub rotation: [f32; 4],
}

/// What a character holds, for others to draw (`weapons`): the active
/// weapon's registry index (`weapon::WeaponRegistry::index`), its
/// `AltModes` mode (a silencer on) and whether a grenade's pin is out.
/// Written by the server after each tick.
#[derive(Component, Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct NetHeld {
    pub weapon: Option<u16>,
    pub mode: u8,
    pub primed: bool,
}

/// Something small the server simulates and clients draw from snapshots
/// (`weapons`): a loose weapon or a grenade in flight, by its held-model
/// key (`map::loose::ShownItem`) and pose, and the client id of the player
/// who threw or fired it (`NetCharacter::owner`; None: a bot's, the
/// world's): that player's client draws its own predicted copy instead
/// (`own_items`). Sent quantized as `NetProp` (`quant`,
/// `NetItem::quantized`).
#[derive(Component, Clone, Debug, Default, PartialEq)]
pub struct NetItem {
    pub model: String,
    pub origin: [f32; 3],
    pub rotation: [f32; 4],
    pub owner: Option<u64>,
}

/// A smoke cloud (`weapon::grenade::SmokeCloud`): where, and the grenade
/// whose rule it follows (registry index). A client starts its own copy
/// when it hears of it.
#[derive(Component, Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct NetSmoke {
    pub centre: [f32; 3],
    pub weapon: u16,
    /// Seconds it had been there when the server last wrote it.
    pub age: f32,
}

/// Server -> every client but the shooter's: one round someone fired
/// (`weapon::WeaponEventKind::Fired`), for the client to trace again and
/// draw (Source's "fire bullets" message): who, which weapon (registry
/// index), from where, the angles, the spread seed and the spread, and
/// what the server's own trace found (`flags`). Unreliable, like Source's temporary entities.
#[derive(Message, Serialize, Deserialize, Clone, Debug, PartialEq, bevy::ecs::entity::MapEntities)]
pub struct FireBullets {
    #[entities]
    pub shooter: Entity,
    pub weapon: u16,
    pub origin: [f32; 3],
    pub yaw: f32,
    pub pitch: f32,
    pub seed: u32,
    /// `weapon::SpreadShape` (`spread_shape`): a disc (inaccuracy,
    /// spread), else the template's or the cone's scale in `inaccuracy`.
    pub shape: u8,
    pub inaccuracy: f32,
    pub spread: f32,
    pub mode: u8,
    /// `weapon::remote::shot_flags`: for a weapon with
    /// `Hitscan::tracer_every`, whether the server's count gave the round
    /// a tracer and whether it hit anything.
    pub flags: u8,
}

/// Server -> every client but the doer's: something else a weapon did,
/// for the body's animation (`weapon_fx::*`).
#[derive(Message, Serialize, Deserialize, Clone, Debug, PartialEq, bevy::ecs::entity::MapEntities)]
pub struct WeaponFx {
    #[entities]
    pub owner: Entity,
    pub kind: u8,
}

/// `FireBullets::shape` (the template 0 and the disc 1 as the bool it
/// was).
pub mod spread_shape {
    pub const TEMPLATE: u8 = 0;
    pub const DISC: u8 = 1;
    pub const CONE: u8 = 2;
}

/// `WeaponFx::kind`.
pub mod weapon_fx {
    pub const RELOAD: u8 = 1;
    pub const SHELL: u8 = 2;
    pub const RELOADED: u8 = 3;
    pub const SWING: u8 = 4;
    pub const SWING2: u8 = 5;
    pub const PIN: u8 = 6;
    pub const THROWN: u8 = 7;
    /// Arming the bomb began / stopped short (the body's `*_Shoot_C4`).
    pub const ARMING: u8 = 8;
    pub const ARMING_STOPPED: u8 = 9;
}

/// Server -> clients: a grenade went off (`weapon::grenade::Detonated`):
/// what kind, which grenade (registry index), where and the ground under
/// it, for the explosion's look.
#[derive(Message, Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Detonation {
    pub kind: u8,
    pub weapon: u16,
    pub at: [f32; 3],
    pub ground: Option<([f32; 3], [f32; 3])>,
    /// `core::BlastFlags`.
    pub flags: u16,
}

/// Server -> clients: map logic's long-lived sounds (`map::SoundControl`
/// under the logic's keys: ambient_generic, a mover's loop, fire), which
/// clients, running no map logic, would not hear otherwise (`mapfx`).
/// In order: started, changed, stopped.
#[derive(Message, Serialize, Deserialize, Clone, Debug, PartialEq, bevy::ecs::entity::MapEntities)]
pub struct LogicSound {
    pub key: u64,
    pub what: LogicSoundKind,
    /// A started sound follows this replicated entity (a player)...
    #[entities]
    pub follow: Option<Entity>,
}

/// What a `LogicSound` does.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub enum LogicSoundKind {
    Start {
        entry: String,
        at: Option<[f32; 3]>,
        /// ... or this map part: (true: a brush entity, false: a prop;
        /// map entity index), the client's own copy of it.
        follow_part: Option<(bool, u32)>,
        volume: Option<f32>,
        pitch: Option<f32>,
        level: Option<crate::map::SoundLevel>,
        clip: Option<u32>,
        looping: bool,
        dry: bool,
    },
    Change {
        volume: Option<f32>,
        pitch: Option<f32>,
    },
    Stop,
    /// Every long-lived sound but the soundscape's (a round restart).
    StopAll,
}

/// Server -> clients: a map effect the logic set off (`mapfx`): a
/// strike-generator env_beam's strike (map entity index, engine-space
/// ends, life), a point_tesla's spark, an env_spark burst.
#[derive(Message, Serialize, Deserialize, Clone, Debug, PartialEq)]
pub enum MapFx {
    Strike {
        entity: u32,
        start: [f32; 3],
        end: [f32; 3],
        life: f32,
    },
    Tesla {
        entity: u32,
    },
    Spark {
        at: [f32; 3],
        dir: [f32; 3],
        magnitude: f32,
    },
    /// A screen shake (`map::MapShake`: env_shake, a canister's impact).
    Shake {
        at: [f32; 3],
        amplitude: f32,
        frequency: f32,
        duration: f32,
        radius: f32,
    },
}

/// Server -> the client a character belongs to: its player was blinded
/// (alpha, seconds held, seconds fading: `core::Blinded` from now on) or
/// its hearing hit (`core::Deafened`).
#[derive(Message, Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Senses {
    pub blind: Option<(f32, f32, f32)>,
    pub hearing: Option<crate::core::HearingEffect>,
}

/// Server -> clients: a character died (`core::Died`): who, by whom, and
/// the killing hit (its weapon's registry index: the gun that killed,
/// thrown or held, `weapon::DamageWeapons`; else, for a kill no gun dealt,
/// the class the rules named it by, `class`: HL2:DM's `prop_physics`;
/// hitgroup and kind as `net::weapons` numbers them, where
/// and which way), for the kill feed and the client's ragdoll; with the
/// announcer's facts.
#[derive(Message, Serialize, Deserialize, Clone, Debug, PartialEq, bevy::ecs::entity::MapEntities)]
pub struct Killed {
    #[entities]
    pub victim: Entity,
    #[entities]
    pub attacker: Option<Entity>,
    pub weapon: Option<u16>,
    /// The killing hit's weapon id when it is no registered gun (a class
    /// of no game: a prop's, a map's): the death notices name it.
    pub class: Option<String>,
    pub hitgroup: u8,
    pub kind: u8,
    pub point: [f32; 3],
    pub dir: [f32; 3],
    pub force: [f32; 3],
    /// An enemy kill's facts for the announcer (multi-kill count, first
    /// kill, head or groin shot, revenge: `announcer::KillFacts`), when the
    /// server announces (specs/combat_arms/kill_streaks.md A1).
    pub facts: Option<chimera_core::announcer::KillFacts>,
}

/// Server -> clients: a thing that is no character was destroyed
/// (`core::Destroyed`: a game's placed device, Combat Arms' auto turret),
/// for the kill feed: what it was (its weapon's registry index), whose,
/// by whom and with what.
#[derive(Message, Serialize, Deserialize, Clone, Debug, PartialEq, bevy::ecs::entity::MapEntities)]
pub struct ObjectDestroyed {
    pub what: u16,
    #[entities]
    pub owner: Option<Entity>,
    #[entities]
    pub attacker: Option<Entity>,
    pub weapon: Option<u16>,
}

/// Server -> the shooter's client: its shot hit (the server's
/// confirmation: the hit marker), how hard and where on the body.
#[derive(Message, Serialize, Deserialize, Clone, Debug, PartialEq, bevy::ecs::entity::MapEntities)]
pub struct HitConfirm {
    #[entities]
    pub target: Entity,
    pub amount: f32,
    pub hitgroup: u8,
}

/// Server -> the client a character belongs to: damage reached its
/// player's health (`core::DamageTaken`): how much, which way it
/// travelled, how it was dealt (`kind` as `net::weapons` numbers them)
/// and whether someone else dealt it, for the damage indicators.
#[derive(Message, Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Hurt {
    pub amount: f32,
    pub dir: [f32; 3],
    pub kind: u8,
    pub directed: bool,
}

/// Client -> server: drop the weapon I hold (Source's `drop`).
#[derive(Message, Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct DropRequest;

// --- Game rules (slice 5): `game`, `chat`, `cvars`.

/// A character's line on the scoreboard (`game::write_scores`): kills,
/// deaths, ping (ms, from the server's measure of its client's round
/// trip; 0 for the host and bots) and `score_flags::*`.
#[derive(Component, Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct NetScore {
    pub kills: i32,
    pub deaths: u32,
    pub ping: u16,
    pub flags: u8,
}

/// `NetScore::flags`.
pub mod score_flags {
    pub const BOT: u8 = 1;
    /// Carries the bomb (shown to teammates).
    pub const BOMB: u8 = 1 << 1;
    /// Has a defusal kit.
    pub const KIT: u8 = 1 << 2;
}

/// Client -> server: buy this (`buy <what>`, the buy menu, `buyammo1`).
#[derive(Message, Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct BuyRequest {
    pub what: String,
}

/// The server's message of the day (CS:S's `motd.txt`), shown as a
/// player joins (`games::cs_source::ui::join_menus`): `html` the file as written (often
/// HTML), `text` the plain-text one (`motd_text.txt`) for players with
/// `cl_disablehtmlmotd 1`. Server -> a client as it is let in; a game of
/// one's or a listen server's own come from its files (`motd::current`).
#[derive(Resource, Message, Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct ServerMotd {
    /// The server's name (`hostname`), the panel's title.
    pub server: String,
    pub html: String,
    pub text: String,
}

/// Client -> server: say this to everyone or the team (`say`,
/// `say_team`).
#[derive(Message, Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct SayRequest {
    pub text: String,
    pub team_only: bool,
}

/// Client -> server: a radio call (`coverme`, ...).
#[derive(Message, Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct RadioRequest {
    pub command: String,
}

/// Server -> a client: why its request was refused (a buy, a team), as
/// the game's hint.
#[derive(Message, Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct Notice {
    pub text: String,
}

/// Server -> each player who reads it (`map::radio::sees_say`): a chat
/// line, for the client to put in the game's format. A listen server's
/// host gets its copy as this message too.
#[derive(Message, Serialize, Deserialize, Clone, Debug, Default, PartialEq, bevy::ecs::entity::MapEntities)]
pub struct ChatMessage {
    #[entities]
    pub sender: Option<Entity>,
    pub name: String,
    /// The sender's team number, if any; whether alive; where (the nav
    /// mesh's place name).
    pub team: Option<u8>,
    pub alive: bool,
    pub team_only: bool,
    pub place: Option<String>,
    pub text: String,
    /// The server's own line (a map's `say` on a dedicated server):
    /// shown as "Console: text".
    pub console: bool,
}

/// Server -> each player who hears it (`map::radio::hears`): a radio
/// call, played as `core::Radio` here.
#[derive(Message, Serialize, Deserialize, Clone, Debug, PartialEq, bevy::ecs::entity::MapEntities)]
pub struct RadioCall {
    #[entities]
    pub sender: Entity,
    pub command: String,
}

/// Server -> clients: a sound of the game's rules (`map::GameSound`: the
/// bomb, hostages), played where the server played it.
#[derive(Message, Serialize, Deserialize, Clone, Debug, Default, PartialEq, bevy::ecs::entity::MapEntities)]
pub struct ServerSound {
    pub entry: String,
    pub at: Option<[f32; 3]>,
    pub volume: Option<f32>,
    pub pitch: Option<f32>,
    #[entities]
    pub source: Option<Entity>,
    pub channel: Option<u8>,
}

/// Server -> clients: replicated cvars' values (`console::CvarScope::
/// Replicated`): all of them when a client joins, then those that
/// changed. A client sets them and refuses local changes while connected.
#[derive(Message, Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct CvarValues {
    pub values: Vec<(String, String)>,
}

/// Client -> server: my name is now this (the `name` cvar changed while
/// connected; Source's setinfo).
#[derive(Message, Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct NameRequest {
    pub name: String,
}

/// Client -> server: my userinfo cvars are now these (one changed while
/// connected; Source's setinfo). The server keeps them as the player's
/// `core::UserInfo`.
#[derive(Message, Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct UserInfoRequest {
    pub values: Vec<(String, String)>,
}

/// Server -> one client: set these userinfo cvars of yours (by their
/// userinfo key): the server changed its copy (HL2:DM telling a client
/// its `cl_playermodel`; Source's server setting a client's cvar).
#[derive(Message, Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct UserInfoSet {
    pub values: Vec<(String, String)>,
}

/// Server -> every player: someone changed their name (the game's
/// "* %s1 changed name to %s2").
#[derive(Message, Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct NameChanged {
    pub old: String,
    pub new: String,
}

/// Server -> every player: someone joined the server ("Player %s1 has
/// joined the game") or left it, and why ("Player %s1 left the game
/// (%s2)": "Disconnect by user.", "timed out", a kick's reason), the
/// game's words (`map::radio::SayFormats`).
#[derive(Message, Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct PlayerCame {
    pub name: String,
    /// None: joined; else left, for this reason.
    pub left: Option<String>,
}

/// The network's systems, protocol and console commands. With
/// `RepliconPlugins` and renet's; nothing runs until `listen` or
/// `connect` (single player stays `NetRole::Standalone`).
pub struct NetPlugin;

impl Plugin for NetPlugin {
    fn build(&self, app: &mut App) {
        if !app.is_plugin_added::<StatesPlugin>() {
            app.add_plugins(StatesPlugin);
        }
        app.add_plugins((
            RepliconPlugins
                .set(RepliconSharedPlugin {
                    // Our own handshake (`Join`): a version message before the
                    // protocol hash.
                    auth_method: AuthMethod::Custom,
                })
                .set(ServerPlugin {
                    // A mutate message with every update, empty or not: a
                    // client learns that what didn't change held still at
                    // that tick (`interp`), and when ticks arrive (its
                    // clock). Each client's are paced: `server::UpdatePacing`.
                    track_mutate_messages: true,
                    ..ServerPlugin::new(FixedPostUpdate)
                }),
            RepliconRenetPlugins,
        ));
        // Replicated values in tick order, held-back ones too (`ordered`).
        ordered::plugin(app);
        app
            // The protocol, in the same order everywhere. `Join` and
            // `Refused` come first so another build can still read them.
            .add_client_event::<Join>(Channel::Ordered)
            .add_server_event::<Refused>(Channel::Ordered)
            .make_event_independent::<Refused>()
            .add_server_event::<Welcome>(Channel::Ordered)
            .make_event_independent::<Welcome>()
            // Maps (slice 7).
            .add_server_event::<ChangingLevel>(Channel::Ordered)
            .make_event_independent::<ChangingLevel>()
            .add_server_event::<ChangeLevel>(Channel::Ordered)
            .make_event_independent::<ChangeLevel>()
            // The match changed on the same map (net31).
            .add_server_event::<ChangeMatch>(Channel::Ordered)
            .make_event_independent::<ChangeMatch>()
            .add_client_message::<Loaded>(Channel::Ordered)
            .add_client_message::<MapRequest>(Channel::Ordered)
            .add_client_message::<MapAck>(Channel::Unreliable)
            .add_server_message::<MapChunk>(Channel::Ordered)
            .make_message_independent::<MapChunk>()
            .add_server_message::<MapDenied>(Channel::Ordered)
            .make_message_independent::<MapDenied>()
            .add_client_message::<UserCmds>(Channel::Unreliable)
            .add_server_message::<OwnStateDelta>(Channel::Unreliable)
            // Holds no entities: no need to wait for replication.
            .make_message_independent::<OwnStateDelta>()
            .replicate::<NetCharacter>()
            .ordered::<NetCharacter>()
            .replicate::<NetBody>()
            .replicate::<Team>()
            .ordered::<Team>()
            .replicate::<crate::core::Spectating>()
            .ordered::<crate::core::Spectating>()
            .replicate::<crate::core::Unassigned>()
            .ordered::<crate::core::Unassigned>()
            .replicate::<crate::core::ChoosingClass>()
            .ordered::<crate::core::ChoosingClass>()
            .replicate::<crate::core::PlayerClass>()
            .ordered::<crate::core::PlayerClass>()
            .replicate::<Health>()
            .ordered::<Health>()
            // Whose game a character is (its body, its player's HUD) where
            // that follows its weapon (`slots::GameIdentity`).
            .replicate::<crate::slots::GameIdentity>()
            .ordered::<crate::slots::GameIdentity>()
            // Map render looks: players' (invisibility, team colours) and
            // movers' (colour, alpha, texture frame; the movers themselves
            // are Source's logic's part, `NetParts`), only when they change.
            .replicate::<crate::map::RenderLook>()
            .ordered::<crate::map::RenderLook>()
            .replicate::<crate::map::BrushTint>()
            .ordered::<crate::map::BrushTint>()
            // Every value a client receives also goes into its snapshot
            // buffer, by the server tick it is from.
            .set_receive_fns::<NetBody>(interp::write_snapshot::<NetBody>, interp::remove_snapshots::<NetBody>)
            .ordered_with::<NetBody>(interp::write_snapshot::<NetBody>, interp::remove_snapshots::<NetBody>)
            .replicate::<NetProp>()
            .set_receive_fns::<NetProp>(interp::write_snapshot::<NetProp>, interp::remove_snapshots::<NetProp>)
            .ordered_with::<NetProp>(interp::write_snapshot::<NetProp>, interp::remove_snapshots::<NetProp>)
            .replicate::<NetAnchor>()
            .set_receive_fns::<NetAnchor>(interp::write_snapshot::<NetAnchor>, interp::remove_snapshots::<NetAnchor>)
            .ordered_with::<NetAnchor>(interp::write_snapshot::<NetAnchor>, interp::remove_snapshots::<NetAnchor>)
            .replicate::<NetRagdoll>()
            .set_receive_fns::<NetRagdoll>(interp::write_snapshot::<NetRagdoll>, interp::remove_snapshots::<NetRagdoll>)
            .ordered_with::<NetRagdoll>(interp::write_snapshot::<NetRagdoll>, interp::remove_snapshots::<NetRagdoll>)
            // Weapons (slice 4).
            .replicate::<NetHeld>()
            .ordered::<NetHeld>()
            .replicate::<NetHold>()
            .ordered::<NetHold>()
            .replicate::<NetItem>()
            .set_receive_fns::<NetItem>(interp::write_snapshot::<NetItem>, interp::remove_snapshots::<NetItem>)
            .ordered_with::<NetItem>(interp::write_snapshot::<NetItem>, interp::remove_snapshots::<NetItem>)
            .replicate::<NetSmoke>()
            .ordered::<NetSmoke>()
            .add_mapped_server_message::<FireBullets>(Channel::Unreliable)
            .add_mapped_server_message::<WeaponFx>(Channel::Unreliable)
            .add_server_message::<Detonation>(Channel::Unreliable)
            .make_message_independent::<Detonation>()
            .add_server_message::<Senses>(Channel::Ordered)
            .make_message_independent::<Senses>()
            .add_client_message::<DropRequest>(Channel::Ordered)
            .add_mapped_server_message::<Killed>(Channel::Ordered)
            .add_mapped_server_message::<ObjectDestroyed>(Channel::Ordered)
            .add_mapped_server_message::<HitConfirm>(Channel::Unordered)
            .add_server_message::<Hurt>(Channel::Unordered)
            .make_message_independent::<Hurt>()
            // Game rules (slice 5): what every game has; a game's own (CS:S's
            // round, bomb and hostages) are its part (`NetParts`).
            .replicate::<NetScore>()
            .ordered::<NetScore>()
            .add_client_message::<BuyRequest>(Channel::Ordered)
            .add_server_message::<ServerMotd>(Channel::Ordered)
            .make_message_independent::<ServerMotd>()
            .add_client_message::<SayRequest>(Channel::Ordered)
            .add_client_message::<RadioRequest>(Channel::Ordered)
            .add_server_message::<Notice>(Channel::Ordered)
            .make_message_independent::<Notice>()
            .add_mapped_server_message::<ChatMessage>(Channel::Ordered)
            .add_mapped_server_message::<RadioCall>(Channel::Ordered)
            .add_mapped_server_message::<ServerSound>(Channel::Unreliable)
            .add_server_message::<CvarValues>(Channel::Ordered)
            .make_message_independent::<CvarValues>()
            .add_client_message::<NameRequest>(Channel::Ordered)
            .add_client_message::<UserInfoRequest>(Channel::Ordered)
            .add_server_message::<UserInfoSet>(Channel::Ordered)
            .add_server_message::<NameChanged>(Channel::Ordered)
            .make_message_independent::<NameChanged>()
            .add_server_message::<MapFx>(Channel::Unreliable)
            .make_message_independent::<MapFx>()
            .add_mapped_server_message::<LogicSound>(Channel::Ordered)
            // Cheats from clients, joins and leaves (net13).
            .add_client_message::<cheats::ClientCommand>(Channel::Ordered)
            .add_server_message::<cheats::ConsoleText>(Channel::Ordered)
            .make_message_independent::<cheats::ConsoleText>()
            .add_server_message::<PlayerCame>(Channel::Ordered)
            .make_message_independent::<PlayerCame>()
            // Voice chat (net25): unreliable, as Source's; nothing at rest.
            .add_client_message::<voice::VoiceData>(Channel::Unreliable)
            .add_mapped_server_message::<voice::VoiceRelay>(Channel::Unreliable)
            // Votes (net26).
            .add_client_message::<vote::CallVote>(Channel::Ordered)
            .add_client_message::<vote::CastVote>(Channel::Ordered)
            .add_server_message::<vote::VoteNews>(Channel::Ordered)
            .make_message_independent::<vote::VoteNews>()
            // The net graph's server numbers (`stats`).
            .add_server_message::<stats::ServerStats>(Channel::Unreliable)
            .make_message_independent::<stats::ServerStats>()
            // Weapon parameters the server decides (net32).
            .add_server_message::<weapon_params::WeaponParams>(Channel::Ordered)
            .make_message_independent::<weapon_params::WeaponParams>()
            // The server's data documents (net33).
            .add_client_message::<server_docs::DocsRequest>(Channel::Ordered)
            .add_server_message::<server_docs::DocsChunk>(Channel::Ordered)
            .add_server_message::<server_docs::DocsDenied>(Channel::Ordered)
            .init_resource::<NetSettings>()
            .init_resource::<NetVersion>()
            .init_resource::<NetHooks>()
            .init_resource::<parts::OwnParts>()
            .add_message::<NetEvent>()
            .add_systems(Update, log_events);
        app.world_mut().resource_mut::<ProtocolHasher>().add_custom(NET_VERSION);
        server::plugin(app);
        client::plugin(app);
        motd::plugin(app);
        predict::plugin(app);
        interp::plugin(app);
        props::plugin(app);
        anchors::plugin(app);
        ragdolls::plugin(app);
        weapons::plugin(app);
        own_items::plugin(app);
        game::plugin(app);
        chat::plugin(app);
        cheats::plugin(app);
        cvars::plugin(app);
        maps::plugin(app);
        manifest::plugin(app);
        assets::plugin(app);
        on_demand::plugin(app);
        mapfx::plugin(app);
        breakables::plugin(app);
        decals::plugin(app);
        udp::plugin(app);
        upnp::plugin(app);
        voice::plugin(app);
        query::plugin(app);
        memory::plugin(app);
        reconnect::plugin(app);
        vote::plugin(app);
        stats::plugin(app);
        weapon_params::plugin(app);
        server_docs::plugin(app);
        commands(app);
    }

    /// The parts games and Source's logic registered (`NetAppExt::net_part`),
    /// in name order, after every plugin's `build` (so whichever order the
    /// app adds them in) and before replicon's own `finish`, which seals
    /// the protocol (this plugin adds replicon's, so it finishes first).
    fn finish(&self, app: &mut App) {
        parts::apply(app);
    }
}

/// Network events in the log (and so the game's console).
fn log_events(mut events: MessageReader<NetEvent>) {
    for e in events.read() {
        match e {
            NetEvent::Listening(a) => info!("listening on UDP {a}"),
            NetEvent::Connecting(a) => info!("connecting to {a}..."),
            NetEvent::LoadMap { map, file } => match file {
                Some(f) => info!("the server is on {map}: loading it from {}", f.display()),
                None => info!("the server is on {map}: loading it"),
            },
            NetEvent::ChangingLevel(m) => info!("the server is changing level to {m}"),
            NetEvent::Joined { map } => info!("joined the game on {map}"),
            NetEvent::Disconnected(r) => info!("disconnected: {r}"),
            NetEvent::GaveUpReconnecting(r) => info!("gave up reconnecting: {r}"),
        }
    }
}

/// Renet's connection settings for replicon's channels.
pub(crate) fn connection_config(world: &World) -> ConnectionConfig {
    let channels = world.resource::<RepliconChannels>();
    ConnectionConfig {
        server_channels_config: channels.server_configs(),
        client_channels_config: channels.client_configs(),
        ..default()
    }
}

/// The loaded map's id as the handshake names it: `greybox` or the
/// `game:name` the map was loaded as (`map::LoadedMapName`).
pub fn current_map(world: &World) -> String {
    let name = world
        .get_resource::<crate::map::LoadedMapName>()
        .map(|n| n.0.clone())
        .unwrap_or_default();
    normalize_map(&name)
}

/// `greybox` for the greybox's names (none, `greybox`, `chimera:greybox`, the old `mashup:greybox`);
/// other ids as they are.
pub fn normalize_map(name: &str) -> String {
    if ["", GREYBOX, "chimera:greybox", "mashup:greybox"].iter().any(|g| name.eq_ignore_ascii_case(g)) {
        GREYBOX.into()
    } else {
        name.into()
    }
}

/// Whether the loaded map is `map` and its file hash is `hash`: Some(true)
/// or Some(false) once a map by that name is loaded, None while it isn't
/// (still loading, or another one).
pub fn map_matches(world: &World, map: &str, hash: Option<[u8; 32]>) -> Option<bool> {
    if map == GREYBOX {
        return (current_map(world) == GREYBOX).then_some(true);
    }
    let file = world.get_resource::<crate::map::MapFile>()?;
    let want_name = map.rsplit(':').next().unwrap_or(map);
    // The loaded file's name, with or without its game (`cs_source:x`).
    let have = file.name.rsplit(':').next().unwrap_or(&file.name);
    if !have.eq_ignore_ascii_case(want_name) || current_map(world) != map {
        return None;
    }
    Some(file.hash == hash)
}

/// Stop serving or leave the server, back to single player
/// (`NetRole::Standalone`): remote players' characters and everything
/// the server sent go. Reports `NetEvent::Disconnected(reason)`.
pub fn disconnect(world: &mut World, reason: &str) {
    let role = world.get_resource::<NetRole>().copied().unwrap_or_default();
    // A lost connection (`reconnect::Lost`) goes on reconnecting; leaving
    // (also while waiting on the menu), a refusal or a failure stops it.
    let lost = world.remove_resource::<reconnect::Lost>().is_some();
    let prior = world.remove_resource::<reconnect::Reconnecting>();
    match role {
        NetRole::Standalone => return,
        NetRole::Server => {
            server::stop(world);
            maps::stop_serving(world);
        }
        NetRole::Client => {
            client::stop(world);
            client::ended(world, reason);
            reconnect::after(world, reason, lost, prior);
        }
    }
    world.insert_resource(NetRole::Standalone);
    world.insert_resource(LastDisconnect(reason.into()));
    world.write_message(NetEvent::Disconnected(reason.into()));
}

/// Why the last network game ended (`disconnect`).
#[derive(Resource, Clone, Debug, PartialEq)]
pub struct LastDisconnect(pub String);

/// Whether a server or client transport is up.
pub fn active(world: &World) -> bool {
    world.contains_resource::<RenetServer>() || world.contains_resource::<RenetClient>()
}

/// Parse `ip[:port]` (or `host[:port]`), port `DEFAULT_PORT` when left out.
pub fn parse_address(text: &str) -> Result<SocketAddr, String> {
    use std::net::ToSocketAddrs;
    let text = text.trim();
    if text.is_empty() {
        return Err("connect <ip[:port]>".into());
    }
    // A bare IPv6 address, or one in brackets with a port.
    if let Ok(ip) = text.parse::<std::net::IpAddr>() {
        return Ok(SocketAddr::new(ip, DEFAULT_PORT));
    }
    if let Ok(a) = text.parse::<SocketAddr>() {
        return Ok(a);
    }
    let with_port = if text.rsplit_once(':').is_some_and(|(_, p)| p.parse::<u16>().is_ok()) {
        text.to_string()
    } else {
        format!("{text}:{DEFAULT_PORT}")
    };
    with_port
        .to_socket_addrs()
        .map_err(|e| format!("{text}: {e}"))?
        .find(|a| a.is_ipv4())
        .or_else(|| with_port.to_socket_addrs().ok()?.next())
        .ok_or_else(|| format!("{text}: no address"))
}

fn commands(app: &mut App) {
    resource_cvar::<NetSettings, std::net::IpAddr>(
        app,
        "ip",
        "Address a server listens on (0.0.0.0: every interface; 127.0.0.1: this machine only).",
        |s| &mut s.ip,
    );
    resource_cvar::<NetSettings, u16>(
        app,
        "hostport",
        "UDP port a server listens on (Source's default 27015; 0: one the system picks).",
        |s| &mut s.hostport,
    );
    resource_cvar::<NetSettings, u32>(
        app,
        "maxplayers",
        "Players a server takes, the host included (1: single player). Above 1, `map` hosts a listen server.",
        |s| &mut s.maxplayers,
    );
    resource_cvar::<NetSettings, String>(app, "name", "Your player name (saved; renames you on the server too).", |s| &mut s.name);
    // Saved in config.cfg (Source archives `name`).
    app.world_mut().resource_mut::<crate::console::Console>().archive("name");
    app.console_command(
        "connect",
        "connect <ip[:port]>: join a server (port 27015 unless given).",
        |w, a| {
            let addr = parse_address(&a.join(" "))?;
            reconnect::cancel(w);
            client::connect(w, addr)?;
            Ok(Some(format!("Connecting to {addr}...")))
        },
    )
    .console_command(
        "listen",
        "Host the loaded map as a listen server on hostport (maxplayers raised to 8 if 1).",
        |w, _| {
            if w.resource::<NetSettings>().maxplayers <= 1 {
                w.resource_mut::<NetSettings>().maxplayers = 8;
            }
            let addr = server::listen(w)?;
            Ok(Some(format!("Listening on UDP {addr} ({})", current_map(w))))
        },
    )
    .console_command("status", "Server, map and players (with ping).", |w, _| {
        Ok(Some(status(w)))
    })
    .console_command("version", "This build's version and network protocol.", |w, _| {
        let build = w.get_resource::<crate::core::BuildVersion>().map_or("unknown", |b| b.0);
        Ok(Some(version_text(build)))
    })
    .console_command("ping", "Each player's ping, as the scoreboard has it.", |w, _| {
        Ok(Some(ping_text(w)))
    })
    .console_command("users", "The players in the game: slot, user id, name.", |w, _| {
        Ok(Some(users_text(w)))
    });
}

/// The players (not bots) in a network game, by client id: id, name and
/// ping (the scoreboard's, `NetScore`); alone, our own name.
fn people(world: &mut World) -> Vec<(u64, String, u16)> {
    let mut people: Vec<(u64, String, u16)> = world
        .query::<(&NetCharacter, Option<&NetScore>)>()
        .iter(world)
        .filter_map(|(c, s)| Some((c.owner?, c.name.clone(), s.map_or(0, |s| s.ping))))
        .collect();
    if people.is_empty() {
        let name = world.get_resource::<NetSettings>().map_or_else(|| "Player".into(), |s| s.name.clone());
        people.push((HOST_ID, name, 0));
    }
    people.sort_by_key(|p| p.0);
    people
}

/// `ping`: Source's "Client ping times:" list.
pub fn ping_text(world: &mut World) -> String {
    let mut out = vec!["Client ping times:".to_string()];
    for (_, name, ping) in people(world) {
        out.push(format!("{ping:>4} ms : {name}"));
    }
    out.join("\n")
}

/// `users`: Source's `<slot:userid:"name">` list (our user id is the
/// client id; the host's is 0).
pub fn users_text(world: &mut World) -> String {
    let people = people(world);
    let mut out = vec!["<slot:userid:\"name\">".to_string()];
    for (slot, (id, name, _)) in people.iter().enumerate() {
        out.push(format!("{slot}:{id}:\"{name}\""));
    }
    out.push(format!("{} users", people.len()));
    out.join("\n")
}

/// `version`: the build (`core::BuildVersion`, the app's stamp) and its
/// network protocol.
pub fn version_text(build: &str) -> String {
    format!("Lucker Party {build}\nProtocol version {NET_VERSION}")
}

/// `status`: Source's layout, roughly.
pub fn status(world: &mut World) -> String {
    let role = world.get_resource::<NetRole>().copied().unwrap_or_default();
    let map = current_map(world);
    let mut out = vec![format!("version : {NET_VERSION}")];
    match role {
        NetRole::Standalone => {
            out.push(format!("map     : {map}"));
            out.push("not connected (single player)".into());
        }
        NetRole::Client => {
            let addr = world.get_resource::<client::ServerAddress>().map(|a| a.0.to_string());
            out.push(format!("server  : {}", addr.unwrap_or_else(|| "(local)".into())));
            out.push(format!("map     : {map}"));
            if let Some(c) = world.get_resource::<RenetClient>() {
                out.push(format!(
                    "state   : {}",
                    if c.is_connected() {
                        "connected"
                    } else if c.is_connecting() {
                        "connecting"
                    } else {
                        "disconnected"
                    }
                ));
                out.push(format!(
                    "ping    : {:.0} ms, loss {:.1}%",
                    c.rtt() * 1000.0,
                    c.packet_loss() * 100.0
                ));
            }
            if let Some(g) = world.get_resource::<predict::NetGraph>() {
                out.push(format!("predict : {}", g.prediction_line()));
                out.push(format!("interp  : {}", g.interp_line()));
            }
        }
        NetRole::Server => {
            let settings = world.resource::<NetSettings>().clone();
            out.insert(0, format!("hostname: {}", world.resource::<query::Hosting>().hostname));
            let addr = world.get_resource::<udp::UdpServer>().and_then(|t| t.local_addr());
            out.push(match addr {
                Some(a) => format!("udp/ip  : {a}"),
                None => format!("udp/ip  : {}:{}", settings.ip, settings.hostport),
            });
            if let Some(line) = world.resource::<upnp::Upnp>().state.line() {
                out.push(format!("upnp    : {line}"));
            }
            if let Some(t) = world.get_resource::<udp::UdpServer>() {
                let q = &t.queries;
                out.push(format!(
                    "queries : {} answered, {} challenged, {} dropped",
                    q.answered, q.challenged, q.dropped
                ));
            }
            out.push(format!("map     : {map}"));
            let players = server::players(world);
            out.push(format!("players : {} ({} max)", players.len(), settings.maxplayers));
            out.push("# id               name                 ping  loss   cmds missed".into());
            for p in players {
                out.push(format!(
                    "# {:<16} {:<20} {:>4}  {:>4.1}%  {:>4} {:>6}",
                    p.id,
                    p.name,
                    p.ping_ms.map_or("-".into(), |p| format!("{p:.0}")),
                    p.loss * 100.0,
                    p.buffered,
                    p.missed
                ));
            }
            let step = world.resource::<Time<Fixed>>().timestep().as_secs_f64();
            let lag = world.resource::<crate::weapon::lagcomp::LagCompStats>();
            out.push(format!(
                "lagcomp : {} shots rewound, {} held at sv_maxunlag, {} not across a teleport{}",
                lag.rewinds,
                lag.clamped,
                lag.teleports,
                lag.last.as_ref().map_or(String::new(), |r| format!(
                    "; last {:.0} ms back, {} moved",
                    r.seconds_back(step) * 1000.0,
                    r.moved
                ))
            ));
        }
    }
    out.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ping_and_users_list_the_people_not_bots() {
        let mut w = World::new();
        let alone = users_text(&mut w);
        assert_eq!(alone, "<slot:userid:\"name\">\n0:0:\"Player\"\n1 users");
        w.spawn((
            NetCharacter {
                owner: Some(7),
                name: "b".into(),
            },
            NetScore {
                ping: 42,
                ..Default::default()
            },
        ));
        w.spawn(NetCharacter {
            owner: Some(HOST_ID),
            name: "a".into(),
        });
        w.spawn(NetCharacter {
            owner: None,
            name: "Bot".into(),
        });
        assert_eq!(users_text(&mut w), "<slot:userid:\"name\">\n0:0:\"a\"\n1:7:\"b\"\n2 users");
        assert_eq!(ping_text(&mut w), "Client ping times:\n   0 ms : a\n  42 ms : b");
    }

    #[test]
    fn addresses_default_to_the_source_port() {
        assert_eq!(parse_address("127.0.0.1").unwrap(), "127.0.0.1:27015".parse().unwrap());
        assert_eq!(
            parse_address("127.0.0.1:27016").unwrap(),
            "127.0.0.1:27016".parse().unwrap()
        );
        assert_eq!(parse_address("localhost:27020").unwrap().port(), 27020);
        assert_eq!(parse_address("::1").unwrap(), "[::1]:27015".parse().unwrap());
        assert!(parse_address("").is_err());
    }

    #[test]
    fn intents_from_clients_are_made_safe() {
        let mut i = crate::core::Intent::default();
        NetCmd {
            tick: 7,
            move_axis: [3.0, f32::NAN],
            yaw: f32::INFINITY,
            pitch: 7.0,
            buttons: buttons::JUMP | buttons::FIRE,
            select: Some(200),
            actions: 0b101,
            view_tick: f64::NAN,
            look_ack: 0,
        }
        .apply(&mut i);
        assert_eq!(i.move_axis, Vec2::new(1.0, 0.0));
        assert_eq!(i.yaw, 0.0);
        assert_eq!(i.pitch, std::f32::consts::FRAC_PI_2);
        assert!(i.jump && i.fire && !i.crouch);
        assert_eq!(i.actions, 0b101);
        assert_eq!(i.select, None);
        // A view tick that isn't one, or one ahead of its command: none,
        // or the command's own.
        let c = |v: f64| NetCmd {
            tick: 7,
            view_tick: v,
            ..default()
        };
        assert_eq!(c(f64::NAN).view(7), None);
        assert_eq!(c(f64::INFINITY).view(7), None);
        assert_eq!(c(0.0).view(7), None);
        assert_eq!(c(5.5).view(7), Some(5.5));
        assert_eq!(c(90.0).view(7), Some(7.0));
        // A round trip keeps what a real intent says.
        let src = crate::core::Intent {
            move_axis: Vec2::new(0.6, -0.8),
            yaw: 1.25,
            pitch: -0.3,
            crouch: true,
            use_key: true,
            select: Some(2),
            actions: 0b10,
            ..default()
        };
        let mut back = crate::core::Intent::default();
        NetCmd::of(1, &src).apply(&mut back);
        assert_eq!(NetCmd::of(1, &back), NetCmd::of(1, &src));
        assert_eq!(back.actions, 0b10);
    }

    #[test]
    fn greybox_names() {
        assert_eq!(normalize_map(""), GREYBOX);
        assert_eq!(normalize_map("mashup:greybox"), GREYBOX);
        assert_eq!(normalize_map("chimera:greybox"), GREYBOX);
        assert_eq!(normalize_map("cs_source:de_dust2"), "cs_source:de_dust2");
    }
}
