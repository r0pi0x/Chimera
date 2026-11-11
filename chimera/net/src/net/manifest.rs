//! The match manifest and the mount doctor (docs/plans/active/
//! architecture-review.md M2, M3; L3).
//!
//! - **The manifest** (`MatchManifest`): what the match a server plays
//!   needs of a client: the map, the games whose installs it reads
//!   (`GameNeed`: the map's game required, the others the match names
//!   wanted: without them it loads but looks or sounds wrong), the
//!   gameplay's slots, the weapon sets, and the minigame (`MinigameOffer`:
//!   its id, the SHA-256 of its definition's text, the text). Built by
//!   `current` from the served map and the gameplay, then each part's hook
//!   (`NetHooks::manifest`: the minigame's, `minigame_net`). Sent in
//!   `Welcome`, in `ChangeLevel`, and alone as `ChangeMatch` when it
//!   changes on the same map (`announce`: a part calls it, e.g. when the
//!   minigame changes).
//! - **The mount doctor** (`MountReport`): what this machine has of each
//!   game (installed or not, its Steam build, notes such as a missing key),
//!   made by the app (`LocalMounts`; `examine` for the registered games'
//!   installs). A client with one tells the server at `Join`, in its
//!   userinfo under `MOUNTS_KEY` (`encode`: the installed games and their
//!   builds), so a build without it still joins and an older server ignores
//!   it. The server refuses a client whose report lacks a game the match
//!   requires, saying exactly which (`check`, `MISSING`); a client checks
//!   each manifest it gets against its own report before it loads
//!   anything, and runs the parts' checks (`NetHooks::accept`: the
//!   minigame's definition is one this build can play), leaving with the
//!   reason (`client::JoinFailure::Mounts`) when one fails. The manifest
//!   also offers the server's data documents and weapon registry
//!   (`server_docs`): a client whose own differ asks for the server's.
//! - A client without a report (`LocalMounts` absent: a faithful game's
//!   client) is taken as having what it needs: the map check still
//!   decides.
//!
//! Known shortcuts: the report is installs and Steam builds, not a hash of
//! the files the loaders read (M3); a game the server knows only by id is
//! named by its id (its short name when the server registered it); the
//! server re-checks a player's report only at join and when it changes
//! match (`minigame_net::change_match`'s playable set), never kicks one
//! whose install fails later.

use bevy::prelude::*;
use bevy_replicon::prelude::*;
use serde::{Deserialize, Serialize};

use super::{ChangeMatch, NetHooks, client::JoinFailure};
use crate::core::NetRole;
use chimera_core::game_registry::Games;

/// What a match needs of a client (`current`).
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct MatchManifest {
    /// The map's id (`greybox`, `cs_source:de_dust2`).
    pub map: String,
    /// The games whose installs the match reads, each once, the required
    /// first.
    pub games: Vec<GameNeed>,
    /// The gameplay's slots (`rules`, `hud`, `binds`, `bots`) by game id.
    pub slots: Vec<(String, String)>,
    /// The weapon sets (game ids) the match draws on (empty: any).
    pub weapons: Vec<String>,
    /// The minigame playing, if any.
    pub minigame: Option<MinigameOffer>,
    /// The server's data documents, by set (`server_docs`): a client
    /// whose own differ asks for them.
    pub docs: Vec<super::server_docs::DocsOffer>,
    /// The server's weapon registry (`server_docs`): a client's must
    /// match once it has the server's documents.
    pub registry: super::server_docs::RegistryOffer,
    /// Where a client without a game's install gets its files, and which
    /// (`assets`; `chimera:asset_url` on the server).
    pub assets: Option<super::assets::AssetOffer>,
}

/// A game a match reads from the client's install.
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq, Eq)]
pub struct GameNeed {
    pub game: String,
    /// Nothing loads without it (the map's game); else the match plays
    /// without it, looking or sounding wrong.
    pub required: bool,
}

/// The minigame a match plays: its id, the SHA-256 of its definition's
/// text and the text, so a client without the same definition plays the
/// server's.
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct MinigameOffer {
    pub id: String,
    pub hash: [u8; 32],
    pub text: String,
}

impl MatchManifest {
    /// Game `game` is needed (`required` wins over wanted); an empty id or
    /// one already there as required changes nothing.
    pub fn need(&mut self, game: &str, required: bool) {
        if game.is_empty() {
            return;
        }
        match self.games.iter_mut().find(|g| g.game.eq_ignore_ascii_case(game)) {
            Some(g) => g.required |= required,
            None => self.games.push(GameNeed {
                game: game.to_string(),
                required,
            }),
        }
        self.games.sort_by_key(|g| !g.required);
    }
}

/// What this machine has of a game (the mount doctor's line for it).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct GameMount {
    pub game: String,
    pub installed: bool,
    /// The install's Steam build, when known.
    pub build: Option<String>,
    /// What the install lacks that only spoils the game (a key), in words.
    pub notes: Vec<String>,
}

/// The mount doctor's report: this machine's games (`examine`, or an
/// app's own).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MountReport {
    pub games: Vec<GameMount>,
}

impl MountReport {
    pub fn get(&self, game: &str) -> Option<&GameMount> {
        self.games.iter().find(|g| g.game.eq_ignore_ascii_case(game))
    }

    /// Whether `game` is installed here.
    pub fn has(&self, game: &str) -> bool {
        self.get(game).is_some_and(|g| g.installed)
    }
}

/// On a client: this machine's report, which it sends at `Join` and checks
/// manifests against. Absent: no report (taken as having everything).
#[derive(Resource, Clone, Debug, Default, PartialEq)]
pub struct LocalMounts(pub MountReport);

/// On a client: the server's manifest, the newest heard.
#[derive(Resource, Clone, Debug, Default, PartialEq)]
pub struct ServerManifest(pub MatchManifest);

/// On a server: the manifest clients last heard (`Welcome` aside), so
/// `announce` sends only a change.
#[derive(Resource, Clone, Debug, Default, PartialEq)]
pub struct SentManifest(pub MatchManifest);

/// The userinfo key a client's report goes under (`encode`).
pub const MOUNTS_KEY: &str = "_mounts";

/// How a refusal for a missing game starts (`client::classify` tells it
/// apart by it).
pub const MISSING: &str = "This match needs ";

/// How a part's refusal of a minigame this build can't play starts
/// (`minigame_net`'s `accept` hook).
pub const CANT_PLAY: &str = "The server's minigame ";

/// A report as a userinfo value (at most 256 bytes): the installed games,
/// `id` or `id@build`, comma-separated (notes stay on the client).
pub fn encode(report: &MountReport) -> String {
    let mut out = String::new();
    for g in report.games.iter().filter(|g| g.installed) {
        let entry = match &g.build {
            Some(b) if !b.is_empty() => format!("{}@{b}", g.game),
            _ => g.game.clone(),
        };
        if out.len() + entry.len() + 1 > 256 {
            break;
        }
        if !out.is_empty() {
            out.push(',');
        }
        out.push_str(&entry);
    }
    out
}

/// A userinfo value back into a report (only installed games are in it).
pub fn decode(value: &str) -> MountReport {
    MountReport {
        games: value
            .split(',')
            .map(str::trim)
            .filter(|e| !e.is_empty())
            .map(|e| {
                let (game, build) = match e.split_once('@') {
                    Some((g, b)) => (g, Some(b.to_string())),
                    None => (e, None),
                };
                GameMount {
                    game: game.to_string(),
                    installed: true,
                    build,
                    notes: Vec::new(),
                }
            })
            .collect(),
    }
}

/// A client's report from its userinfo (None: it sent none).
pub fn from_userinfo<'a>(mut userinfo: impl Iterator<Item = (&'a str, &'a str)>) -> Option<MountReport> {
    userinfo
        .find(|(k, _)| k.eq_ignore_ascii_case(MOUNTS_KEY))
        .map(|(_, v)| decode(v))
}

/// A game's name for a message: its short name and id when registered.
pub fn game_name(games: Option<&Games>, id: &str) -> String {
    match games.and_then(|g| g.get(id)) {
        Some(g) if !g.short_name.eq_ignore_ascii_case(id) => format!("{} ({id})", g.short_name),
        _ => id.to_string(),
    }
}

/// What a client with `report` lacks of `manifest`: Err with the refusal
/// (every required game missing, named), else the wanted games it lacks
/// (warnings).
pub fn check(manifest: &MatchManifest, report: &MountReport, games: Option<&Games>) -> Result<Vec<String>, String> {
    let missing = |required: bool| -> Vec<String> {
        manifest
            .games
            .iter()
            .filter(|n| n.required == required && !report.has(&n.game))
            .map(|n| game_name(games, &n.game))
            .collect()
    };
    let required = missing(true);
    if !required.is_empty() {
        // The map's own game says which map needs it.
        let map_game = manifest.map.split_once(':').map(|(g, _)| g);
        let of_map = manifest
            .games
            .iter()
            .any(|n| n.required && Some(n.game.as_str()) == map_game && !report.has(&n.game));
        let what = if of_map {
            format!(" (its map {})", manifest.map)
        } else {
            String::new()
        };
        return Err(format!(
            "{MISSING}{}{what}, which isn't installed on your machine: install it, or name its folder in \
             chimera.local.toml (`[games.<id>] path`).",
            required.join(" and ")
        ));
    }
    Ok(missing(false)
        .into_iter()
        .map(|g| format!("{g} isn't installed here: the match plays, but what it draws of it won't load"))
        .collect())
}

/// The mount doctor for the registered games: each one's install (the
/// config, the saved settings, Steam: `GameInfo::install`) and its Steam
/// build.
pub fn examine(games: &Games) -> MountReport {
    MountReport {
        games: games
            .all()
            .map(|g| {
                let install = g.install();
                let build = install
                    .as_ref()
                    .zip(g.steam.as_ref())
                    .and_then(|(p, s)| chimera_core::mount::install::build_id_near(p, s));
                GameMount {
                    game: g.id.to_string(),
                    installed: install.is_some(),
                    build,
                    notes: Vec::new(),
                }
            })
            .collect(),
    }
}

/// The match this server plays, as a manifest: the served map (its game
/// required), the gameplay's slots, then each part's additions
/// (`NetHooks::manifest`).
pub fn current(world: &mut World) -> MatchManifest {
    let map = super::maps::served(world).map;
    let mut m = MatchManifest {
        map: map.clone(),
        ..default()
    };
    if map != super::GREYBOX
        && let Some((game, _)) = map.split_once(':')
    {
        m.need(game, true);
    }
    if let Some(g) = world.get_resource::<chimera_core::gameplay::Gameplay>() {
        m.slots = [("rules", g.rules), ("hud", g.hud), ("binds", g.binds), ("bots", g.bots)]
            .into_iter()
            .map(|(s, id)| (s.to_string(), id.to_string()))
            .collect();
    }
    let hooks = world.get_resource::<NetHooks>().map(|h| h.manifest.clone()).unwrap_or_default();
    for hook in hooks {
        hook(world, &mut m);
    }
    super::server_docs::offer(world, &mut m);
    super::assets::offer(world, &mut m);
    m
}

/// The server: tell clients the match changed (`ChangeMatch`) if its
/// manifest isn't the one they last heard. A map change is `ChangeLevel`'s
/// (which carries the manifest): nothing is sent while the served map isn't
/// the one clients last heard of.
pub fn announce(world: &mut World) {
    if world.get_resource::<NetRole>() != Some(&NetRole::Server) {
        return;
    }
    let now = current(world);
    let last = world.get_resource::<SentManifest>().map(|s| s.0.clone());
    match last {
        Some(last) if last == now => return,
        Some(last) if last.map != now.map => return,
        _ => {}
    }
    info!(
        "the match changed: {} on {}",
        now.minigame.as_ref().map_or("no minigame", |m| m.id.as_str()),
        now.map
    );
    world.insert_resource(SentManifest(now.clone()));
    world.commands().server_trigger(ToClients {
        targets: SendTargets::All,
        message: ChangeMatch { manifest: now },
    });
}

/// The server sent `manifest` (in `Welcome` or `ChangeLevel`).
pub(super) fn sent(world: &mut World, manifest: &MatchManifest) {
    world.insert_resource(SentManifest(manifest.clone()));
}

/// A client: can it play `manifest`? Its report (if it has one) against
/// what the match needs, then the parts' checks (`NetHooks::accept`). On
/// success the manifest is the client's `ServerManifest` and the wanted
/// games it lacks are logged.
pub fn accept(world: &mut World, manifest: &MatchManifest) -> Result<(), String> {
    if let Some(local) = world.get_resource::<LocalMounts>() {
        let warnings = check(manifest, &local.0, world.get_resource::<Games>())?;
        for w in warnings {
            warn!("{w}");
        }
    }
    let hooks = world.get_resource::<NetHooks>().map(|h| h.accept.clone()).unwrap_or_default();
    for hook in hooks {
        hook(world, manifest)?;
    }
    super::server_docs::check(world, manifest)?;
    world.insert_resource(ServerManifest(manifest.clone()));
    Ok(())
}

/// `accept`, leaving the server with the reason when it fails; whether it
/// passed.
pub(super) fn accept_or_leave(world: &mut World, manifest: &MatchManifest) -> bool {
    match accept(world, manifest) {
        Ok(()) => true,
        Err(reason) => {
            warn!("can't play the server's match: {reason}");
            super::client::fail(world, JoinFailure::Mounts, &reason);
            false
        }
    }
}

pub(super) fn plugin(app: &mut App) {
    app.add_observer(change_match);
}

/// The server changed match on the same map: check it, then play on (the
/// parts follow: the minigame from its replicated state).
fn change_match(c: On<ChangeMatch>, mut commands: Commands) {
    let manifest = c.manifest.clone();
    commands.queue(move |world: &mut World| {
        if world.get_resource::<NetRole>() != Some(&NetRole::Client) {
            return;
        }
        info!(
            "the server changed match: {}",
            manifest.minigame.as_ref().map_or("no minigame", |m| m.id.as_str())
        );
        accept_or_leave(world, &manifest);
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn report(installed: &[&str]) -> MountReport {
        MountReport {
            games: ["a_game", "b_game"]
                .iter()
                .map(|g| GameMount {
                    game: g.to_string(),
                    installed: installed.contains(g),
                    build: (*g == "a_game").then(|| "123".to_string()),
                    notes: Vec::new(),
                })
                .collect(),
        }
    }

    #[test]
    fn a_report_travels_as_userinfo() {
        let r = report(&["a_game", "b_game"]);
        assert_eq!(encode(&r), "a_game@123,b_game");
        let back = decode(&encode(&r));
        assert!(back.has("a_game") && back.has("B_GAME") && !back.has("c_game"));
        assert_eq!(back.get("a_game").unwrap().build.as_deref(), Some("123"));
        assert_eq!(encode(&report(&[])), "", "nothing installed: an empty value, not no report");
        let info = [("name", "x"), ("_MOUNTS", "")];
        assert_eq!(from_userinfo(info.into_iter()), Some(MountReport::default()));
        assert_eq!(from_userinfo([("name", "x")].into_iter()), None);
    }

    #[test]
    fn the_doctor_names_exactly_what_is_missing() {
        let mut m = MatchManifest {
            map: "a_game:m".into(),
            ..default()
        };
        m.need("b_game", false);
        m.need("a_game", true);
        m.need("a_game", false);
        assert_eq!(m.games[0], GameNeed { game: "a_game".into(), required: true }, "required first, kept required");
        assert_eq!(m.games.len(), 2);
        let both = check(&m, &report(&["a_game", "b_game"]), None);
        assert_eq!(both, Ok(vec![]));
        let wanted = check(&m, &report(&["a_game"]), None).unwrap();
        assert_eq!(wanted.len(), 1);
        assert!(wanted[0].starts_with("b_game isn't installed"), "{wanted:?}");
        let refused = check(&m, &report(&["b_game"]), None).unwrap_err();
        assert!(refused.starts_with("This match needs a_game (its map a_game:m), which isn't installed"), "{refused}");
    }
}
