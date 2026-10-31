//! A minigame's definition as data (docs/plans/active/architecture-review.md
//! D2): our own TOML files (`minigames/`), parsed at run time and checked
//! against what the app registered (`check`). The shape:
//!
//! ```toml
//! id = "originals:gun_game_dust2"
//! name = "Gun Game: {map}"   # {map}: the map played (hud::shown_name), else as written
//! players = { min = 2, max = 16 }
//! maps = ["cs_source:de_dust2", "combat_arms:warehouse"]   # a pool, the first played first
//! teams = "free_for_all"                                  # or "two", "one_team"
//! time_limit = "10m"                                      # "90s", "1.5s", "500ms", or seconds
//! rotate = true                                           # default: after the match, the pool's next map
//! rotation = "random"        # or "in_turn" (the default): the next map at random from the pool,
//! avoid_recent = 10          #   not one of the last this many played (at most the pool's size
//!                            #   less one: never the same map twice); the host's `MapHistory`
//! map_vote = true            # the players vote for the next map on the end screen
//!                            # (`minigame::vote`; default off); tuned: { candidates = 3,
//!                            # time = "12s", reveal = "3s" } over chimera/sim/data/map_vote.toml
//! cues = "cs_source"          # the cue pack its callouts sound (`chimera_core::cues`); "none";
//!                             # default: the pack of the game whose HUD is up, else the first
//! music = "combat_arms"       # the game whose music director plays its moments
//!                             # (`minigame::moments`); "none"; default: the first game whose
//!                             # parts lend one (`GameParts::music`)
//!
//! [slots]                     # the gameplay's slots by game id (`chimera_core::gameplay`)
//! hud = "combat_arms"         # the HUD drawn
//! binds = "combat_arms"       # the default binds and movement cvars
//! tick = "combat_arms"        # whose fixed tick the match runs at
//! bots = "combat_arms"        # whose bots play (a game that gates them by `gameplay::bots_are`)
//! movement = "combat_arms:movement"   # checked to be registered; the app's characters use it
//! character = "combat_arms:soldier"   # informational (the kit isn't a slot yet)
//! identity = "weapon"         # per player: the held weapon's game gives its movement,
//!                             # body and HUD (`chimera_sim::identity`); default: the slots above
//! weapons = ["cs_source", "combat_arms"]   # the weapon sets a definition may name
//!
//! [spawn]
//! respawn = { delay = "2s", protection = "2s" }   # protection: both windows; or
//! # protection = { damage = "4s", bullets = "3s", fire_ends = true }
//! #   no damage for `damage`; for `bullets` attacks pass through the player
//! #   (`core::SpawnProtected`: bullets and swings hit what is behind,
//! #   projectiles fly through, blasts leave it out), its body drawn
//! #   see-through; the player's first attack ends both (`fire_ends`, the
//! #   default); a hit on the player shows nothing meanwhile
//! points = "far_from_enemies"   # or "map" (in turn), "team" (the player's team's),
//!                               # "random_hidden" (anywhere walkable, out of enemies' sight)
//! min_distance = 20.32          # m from a living enemy for "random_hidden" (800 u)
//! kit = ["combat_arms:m9_knife"]   # given every life, before the rules' weapons; or by
//! # kit = { cs_source = ["cs_source:weapon_knife"], combat_arms = ["combat_arms:m9_knife"] }
//! #   game: the kit of the rules' weapon's game (`Kit::ByGame`), swapped with it
//!
//! [rules]
//! module = "chimera:ladder"   # a registered rules module; the other keys are its own
//! ladder = ["cs_source:weapon_glock", "combat_arms:m4a1"]
//!
//! [modifiers]                 # registered modifiers by name, each with its value
//! ```
//!
//! Unknown keys are errors (but under `[rules]`, which its module reads).
//! Durations are strings with a unit (`ms`, `s`, `m`) or plain seconds.

use serde::Deserialize;

use crate::mode::Teams;

/// A minigame's definition.
#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MinigameDef {
    /// `namespace:name` (`originals:gun_game_dust2`).
    pub id: String,
    /// The name players see.
    pub name: String,
    #[serde(default)]
    pub description: String,
    pub players: Players,
    /// The map pool by id (`game:map`), the first played first.
    pub maps: Vec<String>,
    pub teams: TeamsDef,
    /// The match's length (seconds).
    #[serde(deserialize_with = "seconds")]
    pub time_limit: f32,
    /// After the match: the next map of the pool and a new match (a
    /// server; the default), else this map again.
    #[serde(default = "yes")]
    pub rotate: bool,
    /// How `rotate` picks the next map.
    #[serde(default)]
    pub rotation: Rotation,
    /// `Rotation::Random`: the last this many maps played are skipped
    /// (as many as the pool allows).
    #[serde(default = "ten")]
    pub avoid_recent: usize,
    /// A vote for the next map on the match-end screen (`super::vote`):
    /// `true`, `false` (the default) or the settings tuned.
    #[serde(default)]
    pub map_vote: Option<MapVoteParam>,
    /// The cue pack its callouts sound (`chimera_core::cues::CuePacks::
    /// choose`): a pack's id, `none`, or None (`default`) the first
    /// registered.
    #[serde(default)]
    pub cues: Option<String>,
    /// The game whose music director plays the match's moments
    /// (`super::moments`): a game's id, `none`, or None the first whose
    /// parts lend one (`chimera_core::game_parts::GameParts::music`).
    #[serde(default)]
    pub music: Option<String>,
    #[serde(default)]
    pub slots: Slots,
    #[serde(default)]
    pub spawn: SpawnDef,
    pub rules: RulesDef,
    /// Modifiers by name, each with its value (`ModifierAppExt`).
    #[serde(default)]
    pub modifiers: toml::Table,
}

fn yes() -> bool {
    true
}

fn two_seconds() -> f32 {
    2.0
}

fn ten() -> usize {
    10
}

/// `map_vote`: on or off, or tuned (`super::vote::MapVoteSettings`'s
/// keys over the built-in).
#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(untagged)]
pub enum MapVoteParam {
    On(bool),
    Tuned(toml::Table),
}

/// How a rotating minigame picks its next map.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Rotation {
    /// The pool's next map after this one.
    #[default]
    InTurn,
    /// A random map of the pool, not one played lately
    /// (`MinigameDef::avoid_recent`).
    Random,
}

/// How many players a match takes: it waits for `min`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Players {
    pub min: u32,
    pub max: u32,
}

/// How a minigame makes teams.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TeamsDef {
    FreeForAll,
    Two,
    OneTeam,
}

impl TeamsDef {
    /// The mode runtime's teams.
    pub fn teams(self) -> Teams {
        match self {
            Self::FreeForAll => Teams::FreeForAll,
            Self::Two => Teams::Two,
            Self::OneTeam => Teams::AllAlpha,
        }
    }
}

/// The gameplay's slots (game ids), the movement and character asked for
/// and the weapon sets allowed.
#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Slots {
    pub hud: Option<String>,
    pub binds: Option<String>,
    pub tick: Option<String>,
    pub bots: Option<String>,
    pub movement: Option<String>,
    pub character: Option<String>,
    /// What each player's own game slots follow (None: the slots above,
    /// the same for everyone).
    #[serde(default)]
    pub identity: Option<IdentityDef>,
    /// Game ids whose weapons the definition may name (empty: any).
    #[serde(default)]
    pub weapons: Vec<String>,
}

/// What a player's own game identity follows (`slots::FollowsWeapon`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IdentityDef {
    /// The game of the weapon it holds: its movement, its body and look,
    /// its HUD (`chimera_sim::identity`).
    Weapon,
}

/// Where and how players come back.
#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SpawnDef {
    #[serde(default)]
    pub respawn: Respawn,
    #[serde(default)]
    pub points: Points,
    /// `random_hidden`'s least distance (m) from a living enemy: 800 u
    /// by default.
    #[serde(default = "default_min_distance")]
    pub min_distance: f32,
    /// Which way `random_hidden` spawns face (`Facing`).
    #[serde(default)]
    pub facing: Facing,
    /// Weapons given every life before the rules' own.
    #[serde(default)]
    pub kit: Kit,
}

/// The weapons a life starts with besides the rules' own: the same for
/// everyone (a list), or by game (a table of game id to list): the kit of
/// the game of the weapon the rules give (`for_game`; the gun game's
/// knife is the gun's game's), changed with it (`runtime::refit_kit`).
#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(untagged)]
pub enum Kit {
    List(Vec<String>),
    ByGame(std::collections::BTreeMap<String, Vec<String>>),
}

impl Default for Kit {
    fn default() -> Self {
        Self::List(Vec::new())
    }
}

impl Kit {
    /// Every weapon id the kit names, for any game.
    pub fn all(&self) -> Vec<&str> {
        match self {
            Self::List(l) => l.iter().map(String::as_str).collect(),
            Self::ByGame(m) => m.values().flatten().map(String::as_str).collect(),
        }
    }

    /// The kit given with a rules weapon of game `game` (None: the rules
    /// give none): the list, or that game's (none named: nothing).
    pub fn for_game(&self, game: Option<&str>) -> Vec<&str> {
        match self {
            Self::List(l) => l.iter().map(String::as_str).collect(),
            Self::ByGame(m) => game
                .and_then(|g| m.iter().find(|(k, _)| k.eq_ignore_ascii_case(g)))
                .map(|(_, l)| l.iter().map(String::as_str).collect())
                .unwrap_or_default(),
        }
    }
}

/// Which way a `random_hidden` spawn faces: toward open space. The server
/// casts `rays` horizontal rays from eye height around the spot against
/// the static world, caps each at `range` m, smooths the distances over
/// `smooth` neighbouring rays, and faces the most open direction (among
/// those within `tie` of the best, the one toward the middle of the
/// walkable area), plus a random +-`jitter` degrees.
#[derive(Clone, Copy, Debug, PartialEq, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct Facing {
    /// Rays around the spot; 0 turns the feature off (the old eight
    /// fixed facings are then used).
    pub rays: u32,
    /// Longest ray (m).
    pub range: f32,
    /// Taps of the smoothing window over neighbouring rays (1: none).
    pub smooth: u32,
    /// The random turn either way (degrees).
    pub jitter: f32,
    /// Pitch (degrees; negative looks down).
    pub pitch: f32,
    /// Directions within this fraction of the best distance tie; the tie
    /// goes toward the walkable area's centroid (0: no tiebreak).
    pub tie: f32,
}

impl Default for Facing {
    fn default() -> Self {
        Self {
            rays: 32,
            range: 30.0,
            smooth: 5,
            jitter: 10.0,
            pitch: -2.0,
            tie: 0.1,
        }
    }
}

fn default_min_distance() -> f32 {
    20.32
}

impl Default for SpawnDef {
    fn default() -> Self {
        Self {
            respawn: Respawn::default(),
            points: Points::default(),
            min_distance: default_min_distance(),
            facing: Facing::default(),
            kit: Kit::default(),
        }
    }
}

/// The dead respawn after `delay` s, protected (`Protection`).
#[derive(Clone, Copy, Debug, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Respawn {
    /// Default 2 s (`Respawn::default`'s).
    #[serde(deserialize_with = "seconds", default = "two_seconds")]
    pub delay: f32,
    #[serde(default)]
    pub protection: Protection,
}

impl Default for Respawn {
    fn default() -> Self {
        Self {
            delay: 2.0,
            protection: Protection::both(1.5),
        }
    }
}

/// Spawn protection (`core::SpawnProtected`; Combat Arms' S6 in shape):
/// no damage for `damage` s; attacks pass through the player for
/// `bullets` s; the player's first attack ends both when `fire_ends`. A
/// plain duration is both windows, ended by firing.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Protection {
    pub damage: f32,
    pub bullets: f32,
    pub fire_ends: bool,
}

impl Protection {
    /// Both windows `seconds` long, ended by firing.
    pub fn both(seconds: f32) -> Self {
        Self {
            damage: seconds,
            bullets: seconds,
            fire_ends: true,
        }
    }

    /// Whether it protects at all.
    pub fn any(&self) -> bool {
        self.damage > 0.0 || self.bullets > 0.0
    }
}

impl Default for Protection {
    /// None.
    fn default() -> Self {
        Self::both(0.0)
    }
}

impl<'de> Deserialize<'de> for Protection {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Windows {
            #[serde(deserialize_with = "seconds", default)]
            damage: f32,
            #[serde(deserialize_with = "seconds", default)]
            bullets: f32,
            #[serde(default = "yes")]
            fire_ends: bool,
        }
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum Value {
            Windows(Windows),
            Both(#[serde(deserialize_with = "seconds")] f32),
        }
        Ok(match Value::deserialize(d)? {
            Value::Both(s) => Self::both(s),
            Value::Windows(w) => Self {
                damage: w.damage,
                bullets: w.bullets,
                fire_ends: w.fire_ends,
            },
        })
    }
}

/// Which spawn point a player comes back at.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Points {
    /// The map's points in turn, skipping occupied ones.
    Map,
    /// The point farthest from the nearest living other player.
    #[default]
    FarFromEnemies,
    /// The player's team's points (any where there are none).
    Team,
    /// A spot anywhere on the map's walkable floor (`spawns`), at random
    /// (seeded), out of every living enemy's sight and at least
    /// `min_distance` from each; relaxed when none is.
    RandomHidden,
}

/// The rules module and its own keys.
#[derive(Clone, Debug, PartialEq, Deserialize)]
pub struct RulesDef {
    pub module: String,
    /// The module's parameters (every other key under `[rules]`).
    #[serde(flatten)]
    pub params: toml::Table,
}

impl MinigameDef {
    /// Parse a definition (`name` for errors; checks its own consistency,
    /// not what the app registered: `check`).
    pub fn parse(name: &str, text: &str) -> Result<Self, String> {
        let def: Self = toml::from_str(text).map_err(|e| format!("{name}: {e}"))?;
        def.check_self().map_err(|e| format!("{name}: {e}"))?;
        Ok(def)
    }

    /// What the definition alone must satisfy.
    pub fn check_self(&self) -> Result<(), String> {
        let namespaced = |s: &str| s.split_once(':').is_some_and(|(a, b)| !a.is_empty() && !b.is_empty());
        if !namespaced(&self.id) {
            return Err(format!("id `{}` isn't `namespace:name`", self.id));
        }
        if self.players.min == 0 || self.players.min > self.players.max {
            return Err(format!("players: min {} and max {} (1 <= min <= max)", self.players.min, self.players.max));
        }
        if self.maps.is_empty() {
            return Err("maps: the pool is empty".into());
        }
        if let Some(m) = self.maps.iter().find(|m| !namespaced(m)) {
            return Err(format!("maps: `{m}` isn't `game:map`"));
        }
        if self.time_limit <= 0.0 {
            return Err("time_limit must be more than 0".into());
        }
        if self.spawn.respawn.delay < 0.0 || self.spawn.respawn.protection.damage < 0.0 || self.spawn.respawn.protection.bullets < 0.0 {
            return Err("spawn.respawn: negative times".into());
        }
        super::vote::MapVoteSettings::of(self).map_err(|e| format!("map_vote: {e}"))?;
        Ok(())
    }

    /// Whether weapon `id` is from a set the definition allows.
    pub fn allows_weapon(&self, id: &str) -> bool {
        let game = id.split_once(':').map_or("", |(g, _)| g);
        self.slots.weapons.is_empty() || self.slots.weapons.iter().any(|w| w == game)
    }
}

/// A duration: `"10m"`, `"90s"`, `"1.5s"`, `"500ms"`, or a number of
/// seconds.
pub fn parse_seconds(s: &str) -> Result<f32, String> {
    let s = s.trim();
    let (number, scale) = if let Some(n) = s.strip_suffix("ms") {
        (n, 0.001)
    } else if let Some(n) = s.strip_suffix('s') {
        (n, 1.0)
    } else if let Some(n) = s.strip_suffix('m') {
        (n, 60.0)
    } else {
        (s, 1.0)
    };
    let n: f32 = number.trim().parse().map_err(|_| format!("`{s}` isn't a duration (10m, 90s, 500ms)"))?;
    if !n.is_finite() || n < 0.0 {
        return Err(format!("`{s}`: not a duration"));
    }
    Ok(n * scale)
}

pub(super) fn seconds<'de, D: serde::Deserializer<'de>>(d: D) -> Result<f32, D::Error> {
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum Value {
        Number(f64),
        Text(String),
    }
    match Value::deserialize(d)? {
        Value::Number(n) if n >= 0.0 => Ok(n as f32),
        Value::Number(n) => Err(serde::de::Error::custom(format!("{n}: a negative duration"))),
        Value::Text(t) => parse_seconds(&t).map_err(serde::de::Error::custom),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DEF: &str = r#"
id = "test:ladder"
name = "Ladder"
players = { min = 2, max = 8 }
maps = ["greybox:greybox"]
teams = "free_for_all"
time_limit = "2m"

[spawn]
respawn = { delay = "500ms", protection = 1 }
kit = ["kit:shotgun"]

[rules]
module = "chimera:ladder"
ladder = ["kit:pistol", "kit:rifle"]
"#;

    #[test]
    fn a_definition_reads() {
        let def = MinigameDef::parse("t", DEF).unwrap();
        assert_eq!(def.time_limit, 120.0);
        assert_eq!(def.spawn.respawn, Respawn { delay: 0.5, protection: Protection::both(1.0) });
        let windows = MinigameDef::parse(
            "t",
            &DEF.replace("protection = 1", "protection = { damage = \"4s\", bullets = \"3s\", fire_ends = false }"),
        )
        .unwrap();
        assert_eq!(windows.spawn.respawn.protection, Protection { damage: 4.0, bullets: 3.0, fire_ends: false });
        assert!(MinigameDef::parse("t", &DEF.replace("protection = 1", "protection = { damage = \"4s\", what = 1 }")).is_err());
        assert_eq!(def.spawn.points, Points::FarFromEnemies);
        assert_eq!(def.spawn.min_distance, 20.32);
        let hidden = DEF.replace("[spawn]", "[spawn]\npoints = \"random_hidden\"\nmin_distance = 15");
        let def = MinigameDef::parse("t", &hidden).unwrap();
        assert_eq!((def.spawn.points, def.spawn.min_distance), (Points::RandomHidden, 15.0));
        let def = MinigameDef::parse("t", DEF).unwrap();
        assert_eq!(def.teams.teams(), Teams::FreeForAll);
        assert_eq!(def.rules.module, "chimera:ladder");
        assert!(def.rules.params.contains_key("ladder"));
        assert!(def.allows_weapon("anything:at_all"), "no weapon sets named: any");
        assert_eq!(def.slots.identity, None, "slots the same for everyone by default");
        let follows = MinigameDef::parse("t", &DEF.replace("[spawn]", "[slots]\nidentity = \"weapon\"\n[spawn]")).unwrap();
        assert_eq!(follows.slots.identity, Some(IdentityDef::Weapon));
        assert_eq!(def.spawn.kit.for_game(Some("kit")), vec!["kit:shotgun"], "a list: everyone's");
        let by_game = DEF.replace(
            "kit = [\"kit:shotgun\"]",
            "kit = { a = [\"a:knife\"], b = [\"b:knife\", \"b:nade\"] }",
        );
        let kit = MinigameDef::parse("t", &by_game).unwrap().spawn.kit;
        assert_eq!(kit.for_game(Some("b")), vec!["b:knife", "b:nade"]);
        assert_eq!(kit.for_game(Some("A")), vec!["a:knife"]);
        assert!(kit.for_game(Some("c")).is_empty() && kit.for_game(None).is_empty());
        assert_eq!(kit.all(), vec!["a:knife", "b:knife", "b:nade"]);
    }

    #[test]
    fn mistakes_are_errors() {
        let bad = |from: &str, to: &str| MinigameDef::parse("t", &DEF.replace(from, to)).unwrap_err();
        assert!(bad("time_limit = \"2m\"", "time_limit = \"2 minutes\"").contains("duration"));
        assert!(bad("min = 2", "min = 9").contains("min"));
        assert!(bad("id = \"test:ladder\"", "id = \"ladder\"").contains("namespace"));
        assert!(bad("teams = \"free_for_all\"", "teams = \"pairs\"").contains("unknown variant"));
        assert!(bad("[spawn]", "colour = 1\n[spawn]").contains("unknown field"));
        assert!(bad("maps = [\"greybox:greybox\"]", "maps = []").contains("empty"));
    }

    #[test]
    fn durations() {
        assert_eq!(parse_seconds("10m"), Ok(600.0));
        assert_eq!(parse_seconds("1.5s"), Ok(1.5));
        assert_eq!(parse_seconds("250ms"), Ok(0.25));
        assert_eq!(parse_seconds("3"), Ok(3.0));
        assert!(parse_seconds("-1s").is_err());
    }
}
