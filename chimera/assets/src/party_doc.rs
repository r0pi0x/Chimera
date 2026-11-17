//! Parties on the hub, both sides (docs/plans/active/parties.md): the
//! JSON the hub service's party routes (`chimera_services::hub::party`)
//! take and answer, the blocking calls a client makes (on a thread, as
//! `hub_doc`'s), and the client's `party.toml` (its member token).
//!
//! - Every call is `POST /hub/party/<what>` with an account's ticket
//!   (`Authorization: Bearer`) and a JSON body; a member's calls carry its
//!   member token (`chimera_party_` and 32 hex digits) in the body, never
//!   in the URL.
//! - `create` and `join` (by the invite code) answer the token once and
//!   the party (`Joined`); `state` (the heartbeat, every `POLL_SECS`),
//!   `kick`, `promote` and `lead` answer the party (`PartyView`);
//!   `leave` answers `{}`.
//! - A token the hub doesn't know is 404 (`Failure::status`): the client
//!   forgets it (`gone`).
//! - `PartyFile` (`<data_local>/chimera/party.toml`, `CHIMERA_PARTY`
//!   names another): the hub's origin and the token, so reopening the
//!   game keeps the player in the party.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

pub use crate::hub_doc::Failure;
use crate::store::write_atomic;

/// A party's largest size (the shell's rack).
pub const MAX_MEMBERS: usize = 8;
/// How often a member asks the party's state (the heartbeat), s.
pub const POLL_SECS: u64 = 10;
/// Seen this recently: online, s.
pub const ONLINE_SECS: u64 = 45;
/// `PartyFile`'s other path.
pub const ENV: &str = "CHIMERA_PARTY";
/// A member token's prefix.
pub const TOKEN_PREFIX: &str = "chimera_party_";

/// Who a member is, as each call tells the hub (the player's `name` and
/// `cl_avatar`).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Me {
    pub name: String,
    /// `cl_avatar`'s byte (255: none).
    #[serde(default)]
    pub avatar: u8,
}

/// `POST /hub/party/create`.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Create {
    #[serde(flatten)]
    pub me: Me,
}

/// `POST /hub/party/join`.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Join {
    pub code: String,
    #[serde(flatten)]
    pub me: Me,
}

/// `POST /hub/party/state`: the heartbeat, with where the member is.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct State {
    pub token: String,
    #[serde(flatten)]
    pub me: Me,
    /// The game server it is connected to (`ip:port`), none on the menu.
    #[serde(default)]
    pub at: Option<String>,
}

/// `POST /hub/party/leave`.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Token {
    pub token: String,
}

/// `POST /hub/party/kick` and `promote`: the leader's, on `member`.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct OnMember {
    pub token: String,
    pub member: String,
}

/// A game server the party goes to.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Target {
    /// `ip:port`; for a hosted game (`hosted`), the hub puts the address
    /// it sees the leader at in front of the port.
    pub addr: String,
    #[serde(default)]
    pub name: String,
}

/// `POST /hub/party/lead`: the leader takes the party to `target`, or
/// out of a server (none): `together` ("Leave with my party") moves
/// everyone back to the menu, else ("Leave alone") nobody moves.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Lead {
    pub token: String,
    #[serde(default)]
    pub target: Option<Target>,
    #[serde(default)]
    pub together: bool,
    /// The leader hosts the game (`target.addr` is `:port` or any
    /// address: the hub puts the leader's own).
    #[serde(default)]
    pub hosted: bool,
}

/// What a move asks of the members.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MoveKind {
    /// No move yet.
    #[default]
    None,
    /// Join the leader at the target.
    Join,
    /// Leave the server the party was on, back to the menu.
    Leave,
}

/// A member as answered.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct MemberView {
    /// The party's own id for it (8 hex digits).
    pub id: String,
    pub name: String,
    pub avatar: u8,
    pub online: bool,
    pub leader: bool,
    /// The game server it is on, if any.
    #[serde(default)]
    pub at: Option<String>,
}

/// The party as answered.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PartyView {
    /// The public id (12 hex digits): what a game server sees (`cl_party`).
    pub id: String,
    /// The invite code, `XXX-XXX`.
    pub code: String,
    /// The members, in join order.
    pub members: Vec<MemberView>,
    /// Which member is the asker.
    pub you: String,
    /// Where the leader took the party.
    #[serde(default)]
    pub target: Option<Target>,
    /// Counts the leader's moves; `kind` the last one's.
    pub moves: u32,
    pub kind: MoveKind,
    /// The server the last `Leave` move left (the members there go).
    #[serde(default)]
    pub left: Option<String>,
}

impl PartyView {
    pub fn me(&self) -> Option<&MemberView> {
        self.members.iter().find(|m| m.id == self.you)
    }

    pub fn leader(&self) -> Option<&MemberView> {
        self.members.iter().find(|m| m.leader)
    }

    /// Whether the asker leads.
    pub fn leading(&self) -> bool {
        self.me().is_some_and(|m| m.leader)
    }
}

/// `create`'s and `join`'s answer.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Joined {
    pub token: String,
    pub party: PartyView,
}

/// Whether `f` means the token is no longer a member's (forget it).
pub fn gone(f: &Failure) -> bool {
    f.status == Some(404)
}

// --- The calls.

fn post<A: Serialize, T: for<'de> Deserialize<'de>>(origin: &str, what: &str, fingerprint: Option<&str>, ticket: &str, body: &A) -> Result<T, Failure> {
    let url = crate::hub_doc::Discovery::url(origin, &format!("/hub/party/{what}"));
    let bytes = serde_json::to_vec(body).map_err(|e| Failure { status: None, message: e.to_string() })?;
    let answer = crate::hub_doc::post_json(&url, fingerprint, ticket, &bytes)?;
    serde_json::from_slice(&answer).map_err(|e| Failure { status: None, message: format!("the hub's answer: {e}") })
}

pub fn create(origin: &str, fingerprint: Option<&str>, ticket: &str, body: &Create) -> Result<Joined, Failure> {
    post(origin, "create", fingerprint, ticket, body)
}

pub fn join(origin: &str, fingerprint: Option<&str>, ticket: &str, body: &Join) -> Result<Joined, Failure> {
    post(origin, "join", fingerprint, ticket, body)
}

pub fn state(origin: &str, fingerprint: Option<&str>, ticket: &str, body: &State) -> Result<PartyView, Failure> {
    post(origin, "state", fingerprint, ticket, body)
}

pub fn leave(origin: &str, fingerprint: Option<&str>, ticket: &str, token: &str) -> Result<(), Failure> {
    post::<_, serde_json::Value>(origin, "leave", fingerprint, ticket, &Token { token: token.into() }).map(drop)
}

pub fn kick(origin: &str, fingerprint: Option<&str>, ticket: &str, body: &OnMember) -> Result<PartyView, Failure> {
    post(origin, "kick", fingerprint, ticket, body)
}

pub fn promote(origin: &str, fingerprint: Option<&str>, ticket: &str, body: &OnMember) -> Result<PartyView, Failure> {
    post(origin, "promote", fingerprint, ticket, body)
}

pub fn lead(origin: &str, fingerprint: Option<&str>, ticket: &str, body: &Lead) -> Result<PartyView, Failure> {
    post(origin, "lead", fingerprint, ticket, body)
}

// --- The client's file.

/// `party.toml`: the party this player is in, on which hub.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PartyFile {
    /// The hub's origin.
    pub hub: String,
    pub token: String,
}

impl PartyFile {
    /// `CHIMERA_PARTY`, else `<data_local>/chimera/party.toml`.
    pub fn path() -> Option<PathBuf> {
        if let Some(p) = std::env::var_os(ENV).filter(|v| !v.is_empty()) {
            return Some(PathBuf::from(p));
        }
        dirs::data_local_dir().map(|d| d.join("chimera").join("party.toml"))
    }

    pub fn load_from(path: &Path) -> Option<Self> {
        toml::from_str(&std::fs::read_to_string(path).ok()?).ok()
    }

    pub fn save_to(&self, path: &Path) -> Result<(), String> {
        let text = toml::to_string(self).map_err(|e| e.to_string())?;
        write_atomic(path, text.as_bytes()).map_err(|e| format!("{}: {e}", path.display()))
    }

    pub fn remove(path: &Path) {
        let _ = std::fs::remove_file(path);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_file_round_trips() {
        let dir = std::env::temp_dir().join(format!("party_doc_{}", std::process::id()));
        let path = dir.join("party.toml");
        let f = PartyFile { hub: "https://hub.invalid:1".into(), token: format!("{TOKEN_PREFIX}00") };
        f.save_to(&path).unwrap();
        assert_eq!(PartyFile::load_from(&path), Some(f));
        PartyFile::remove(&path);
        assert_eq!(PartyFile::load_from(&path), None);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn a_view_names_its_leader() {
        let v = PartyView {
            members: vec![
                MemberView { id: "a".into(), leader: true, ..Default::default() },
                MemberView { id: "b".into(), ..Default::default() },
            ],
            you: "b".into(),
            ..Default::default()
        };
        assert_eq!(v.leader().map(|m| m.id.as_str()), Some("a"));
        assert!(!v.leading());
    }
}
