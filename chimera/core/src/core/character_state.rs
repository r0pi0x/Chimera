use bevy::prelude::*;
use serde::{Deserialize, Serialize, de::DeserializeOwned};

use super::*;

/// `chimera_noblock` (ours, a server setting sent to clients, off by
/// default as CS:S has nothing like it): 0 players block each other (CS:S),
/// 1 teammates pass through each other, 2 nobody blocks anybody.
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct NoBlock(pub u8);

impl NoBlock {
    /// Whether two characters on these teams block each other (None: not a
    /// player on a team, e.g. a hostage: always blocks).
    pub fn blocks(self, a: Option<u8>, b: Option<u8>) -> bool {
        match (self.0, a, b) {
            (_, None, _) | (_, _, None) | (0, ..) => true,
            (1, a, b) => a != b,
            _ => false,
        }
    }
}

/// Takes no damage (the `god` command).
#[derive(Component, Reflect, Default, Clone, Copy, Debug)]
#[reflect(Component)]
#[component(storage = "SparseSet")]
pub struct God;

/// Spawn protection's two windows, `SimClock` seconds (a minigame's,
/// `chimera_sim::minigame::runtime`; replicated to network clients on
/// their own clock by `chimera_net::minigame_net`), in shape Combat Arms'
/// (specs/combat_arms/game_modes.md S6) with the party's own look (user,
/// 2026-10-29): the body draws see-through on every machine
/// (`chimera_present`'s `render::tint::fade_protected`). Until
/// `bullets_until` the player isn't there for attacks (`passes_attacks`):
/// bullets, swings and thrown projectiles pass through and hit what is
/// behind, blasts leave it out; until `damage_until` no damage lands (the
/// rules hold `God` meanwhile). While either runs, a hit on the player
/// shows nothing on any machine (`hides_hits`): every delivery tells the
/// trace with no `hit` (no blood, puff, decal, impact sound, hit
/// indicator or name-tag flash: `chimera_sim::weapon::impact::
/// hide_protected`), so the server and every client agree.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq)]
#[component(storage = "SparseSet")]
pub struct SpawnProtected {
    pub damage_until: f64,
    pub bullets_until: f64,
}

impl SpawnProtected {
    /// Whether a hit on the player at `now` shows nothing (either window).
    pub fn hides_hits(&self, now: f64) -> bool {
        now + 1e-9 < self.damage_until.max(self.bullets_until)
    }

    /// Whether attacks pass through the player at `now` (bullets,
    /// swings, projectiles; blasts leave it out): as if it weren't there.
    pub fn passes_attacks(&self, now: f64) -> bool {
        now + 1e-9 < self.bullets_until
    }

    /// Whether both windows are over at `now`.
    pub fn over(&self, now: f64) -> bool {
        !self.hides_hits(now)
    }
}

/// Flies through walls: the noclip movement's marker (`chimera_sim`'s
/// `movement::noclip`; Source's noclip move type). Not `God`: CS:S still
/// traces a noclipping player's hitboxes (bullets hit it and go on through
/// as through any body) and hands its damage on, but it takes none, from
/// anything (bullets, blasts, trigger_hurt, the world), nor loses armour;
/// falls count again once it walks (specs/cs_source/movement.md "Noclip
/// and spawning"). `DamageRules` refuses it.
#[derive(Component, Reflect, Default, Clone, Copy, Debug)]
#[reflect(Component)]
pub struct Noclip;

/// A character says a radio command to its team (Counter-Strike's radio):
/// `command` is the game's console name for it (`coverme`, `enemyspot`,
/// ...; `fireinhole` for a thrown grenade). The client resolves it through
/// the map's `map::radio::RadioCommands` and plays it if it hears it.
#[derive(Message, Clone, Debug, PartialEq)]
pub struct Radio {
    pub sender: Entity,
    pub command: String,
}

/// Damage that reached a character's health (after the rules: not
/// refused, not under `God`): for the victim's HUD (the damage
/// indicators). A network client gets its own player's from the server
/// (`net::Hurt`), with `damage.attacker` None and `directed` saying
/// whether someone else dealt it.
#[derive(Message, Clone, Debug)]
pub struct DamageTaken {
    pub damage: Damage,
    /// Dealt by someone other than the victim (a shot, a grenade), so it
    /// has a side to come from.
    pub directed: bool,
}

/// Where on a body a gun's hit landed, in full (`BodyRegion`; its
/// `Damage` carries only the coarse `Hitgroup`, so a groin hit reads as
/// the stomach there): written by a gun's hit beside its `Damage`, on the
/// server, for what tells regions apart after the fact (the announcer's
/// headshot and groin-shot callouts, `chimera_sim::announce`).
#[derive(Message, Clone, Copy, Debug, PartialEq)]
pub struct RegionHit {
    pub target: Entity,
    pub region: BodyRegion,
}

/// The characters whose body is a woman's (Combat Arms' female
/// skeleton), as the game that knows keeps them: what picks a gendered
/// line (the announcer's groin-shot voice). A resource, not a marker, so
/// a body's sex never moves its character to another archetype.
#[derive(Resource, Default, Clone, Debug, PartialEq, Eq)]
pub struct FemaleBodies(pub std::collections::BTreeSet<Entity>);

/// Something's health reached zero.
#[derive(Message, Clone, Debug)]
pub struct Died {
    pub entity: Entity,
    pub attacker: Option<Entity>,
    /// The hit that killed it (direction and point push its ragdoll).
    pub damage: Damage,
}

/// A thing that is no character was destroyed by an attack (a game's
/// placed device: Combat Arms' auto turret): what it was (its weapon id:
/// its name and icon), whose it was, who broke it and with what. Only
/// the kill feed reads it (a line, no score, no rung: it is no `Died`);
/// a network server sends it to its clients (`net::ObjectDestroyed`).
#[derive(Message, Clone, Copy, Debug, PartialEq)]
pub struct Destroyed {
    pub what: &'static str,
    pub owner: Option<Entity>,
    pub attacker: Option<Entity>,
    pub weapon: Option<&'static str>,
}

/// Physics layer of ragdoll bodies. They collide only with the default
/// layer (the world, props), not with characters or each other, and
/// gameplay queries (movement, bullets, sight) leave them out with
/// `SOLID_LAYERS` (specs/cs_source/ragdolls.md 4: ragdolls are debris).
pub const RAGDOLL_LAYER: avian3d::prelude::LayerMask = avian3d::prelude::LayerMask(1 << 1);
/// Every layer but ragdolls, physics shadows (and the loose items' copy
/// of the terrain): the mask for gameplay spatial queries.
pub const SOLID_LAYERS: avian3d::prelude::LayerMask = avian3d::prelude::LayerMask(
    !(1 << 1) & !(1 << 3) & !SHADOW_LAYER.0 & !PHYSICS_CLIP_LAYER.0 & !PLAYER_CLIP_LAYER.0,
);
/// Physics layer of map surfaces only characters' movement meets
/// (`MapCollisionSet` with `characters` alone: Combat Arms' collision
/// blockers; Source's player clips, `MapPlayerClips`). Spatial queries
/// with `SOLID_LAYERS` or `NOT_SHADOW` (shots, sight, cover) pass through;
/// `CHARACTER_FILTER` meets them.
pub const PLAYER_CLIP_LAYER: avian3d::prelude::LayerMask = avian3d::prelude::LayerMask(1 << 6);
/// Physics layer of map surfaces only traces meet (`MapCollisionSet` with
/// `traces` alone: Combat Arms' non-solid detail and ray-hit-only world
/// models): in `SOLID_LAYERS`, not in `CHARACTER_FILTER`.
pub const TRACE_ONLY_LAYER: avian3d::prelude::LayerMask = avian3d::prelude::LayerMask(1 << 7);
/// Physics layer of brushes solid to physics objects only (Source's
/// func_clip_vphysics, `map::physics_clip`): props, physics brushes and
/// loose items (`ITEM_LAYER`) collide with them; characters, ragdolls and
/// every spatial query (`SOLID_LAYERS`, `NOT_SHADOW`) pass through.
pub const PHYSICS_CLIP_LAYER: avian3d::prelude::LayerMask = avian3d::prelude::LayerMask(1 << 5);
/// Physics layer of players' physics shadows (specs/cs_source/physics_props.md
/// 4.1, `map::prop_physics::PhysicsShadow`): invisible bodies that only
/// touch the physics props players push. No spatial query should see
/// them: use `SOLID_LAYERS`, or `NOT_SHADOW` for queries of everything.
pub const SHADOW_LAYER: avian3d::prelude::LayerMask = avian3d::prelude::LayerMask(1 << 4);
/// Every layer but physics shadows (and physics clips, which only
/// physics objects meet, and player clips, which only characters' movement
/// meets: invisible walls stop no gib, shell, glow or name trace).
pub const NOT_SHADOW: avian3d::prelude::LayerMask =
    avian3d::prelude::LayerMask(!SHADOW_LAYER.0 & !PHYSICS_CLIP_LAYER.0 & !PLAYER_CLIP_LAYER.0);
/// Physics layer of loose items (dropped weapons): they collide with the
/// world, props and each other but never with characters (Source's
/// weapon collision group), so a weapon thrown from inside its dropper
/// isn't shoved out of them. Shots and other gameplay queries still see
/// them.
pub const ITEM_LAYER: avian3d::prelude::LayerMask = avian3d::prelude::LayerMask(1 << 2);
/// What a character's body collides with: solid things but loose items
/// and traces-only surfaces, and players-only ones.
pub const CHARACTER_FILTER: avian3d::prelude::LayerMask =
    avian3d::prelude::LayerMask((SOLID_LAYERS.0 | PLAYER_CLIP_LAYER.0) & !ITEM_LAYER.0 & !TRACE_ONLY_LAYER.0);
/// The map's displacement surfaces as loose items collide with them (a
/// copy with its internal edges fixed, `map::spawn_map`); nothing else
/// touches or queries it.
pub const ITEM_GROUND_LAYER: avian3d::prelude::LayerMask = avian3d::prelude::LayerMask(1 << 3);

/// A speed cap the character's equipment imposes (e.g. the held weapon),
/// meters per second; None: none (the movement's own). Movement
/// implementations that model it read it. On every character from the
/// spawn (`CharacterPartsExt::on_every_character`): coming and going with
/// the weapon, it was a new archetype for every combination of the rest.
#[derive(Component, Reflect, Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
#[reflect(Component)]
#[component(storage = "SparseSet")]
pub struct MaxSpeed(pub Option<f32>);

/// A place a character can spawn, provided by the Map slot.
#[derive(Component, Reflect, Default, Clone, Copy, Debug)]
#[reflect(Component)]
pub struct SpawnPoint {
    pub team: Option<Team>,
}

/// The character controlled by this machine's keyboard and mouse.
#[derive(Component, Reflect, Default)]
#[reflect(Component)]
pub struct LocalPlayer;

/// A player's own settings the simulation reads (Source's userinfo: the
/// client cvars marked `console::Console::userinfo`, e.g.
/// `cl_autowepswitch`). The local player's come from its console; a
/// network server's remote players' from their clients (`net`, as `name`
/// is sent). Characters without one (bots) use each setting's default
/// behaviour, as its reader says.
#[derive(Component, Reflect, Default, Clone, Debug, PartialEq)]
#[reflect(Component)]
pub struct UserInfo(pub std::collections::BTreeMap<String, String>);

impl UserInfo {
    /// A setting as a number (`Some(n != 0)`), None when unset or not a
    /// number.
    pub fn flag(&self, key: &str) -> Option<bool> {
        let v = self.0.get(key)?;
        v.trim().parse::<f32>().ok().map(|n| n != 0.0)
    }

    /// The player's party as the hub names it (`cl_party`: its public id;
    /// docs/plans/active/parties.md), None alone.
    pub fn party(&self) -> Option<&str> {
        self.0.get(PARTY_KEY).map(|s| s.trim()).filter(|s| !s.is_empty())
    }
}

/// The userinfo key of a player's party (`UserInfo::party`).
pub const PARTY_KEY: &str = "cl_party";

/// The side (1 or 2) most of `party`'s other players are on, among
/// `others` (their party and team), if any of them is on one: where a
/// joining member goes, so a party plays on one team.
pub fn party_side<'a>(party: Option<&str>, others: impl IntoIterator<Item = (Option<&'a str>, u8)>) -> Option<u8> {
    let party = party?;
    let mut count = [0usize; 2];
    for (p, team) in others {
        if p == Some(party) && (1..=2).contains(&team) {
            count[team as usize - 1] += 1;
        }
    }
    match count {
        [0, 0] => None,
        [a, b] if a >= b => Some(1),
        _ => Some(2),
    }
}

/// A character a person plays from another machine (a network server's
/// client): not this machine's `LocalPlayer`, and not a computer player.
#[derive(Component, Reflect, Default, Clone, Copy, Debug)]
#[reflect(Component)]
pub struct RemotePlayer;

/// A computer player (a bot): the game's bot module puts its brain
/// beside it (CS:S's `bot::Bot` requires it). The network counts and
/// kicks bots by it (`net`'s `BotHooks`) without naming a game.
#[derive(Component, Reflect, Default, Clone, Copy, Debug)]
#[reflect(Component)]
pub struct Bot;

/// A character that isn't a player (CS:S's hostages): on no team's count,
/// not among a server's players, and replicated by its game rather than
/// as a player.
#[derive(Component, Reflect, Default, Clone, Copy, Debug)]
#[reflect(Component)]
pub struct Npc;

/// Kills and deaths (the scoreboard's).
#[derive(Component, Default, Clone, Copy, Debug, Reflect)]
#[reflect(Component)]
pub struct Score {
    pub kills: i32,
    pub deaths: u32,
}

/// Dead since this time (fixed-tick seconds).
#[derive(Component, Clone, Copy, Debug)]
#[component(storage = "SparseSet")]
pub struct Dead {
    pub since: f64,
}

/// Maps loaded so far (the rules' `new_game`): a map change, or the same
/// map loaded again, counts one. A network server notices a reload of
/// the same map by it (`net::maps`).
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MapLoads(pub u32);

/// Bits a carried item marks its carrier's scoreboard line with
/// (`net::score_flags`: CS:S's bomb sets `2`), ORed over everything the
/// character carries.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ScoreMark(pub u8);

/// This build's version text (the app stamps it: `mashup::core::VERSION`),
/// for what reports it without being rebuilt by every commit (`net`'s
/// `version` command).
#[derive(Resource, Clone, Copy, Debug, PartialEq, Eq)]
pub struct BuildVersion(pub &'static str);

/// The fixed tick rate until a game's map sets its own (the greybox, a
/// game without a tick: `game_registry::GameInfo::tick_interval`).
pub const DEFAULT_TICK_HZ: f64 = 64.0;

/// How a player comes into a game it starts or joins: CS:S's way, the
/// join menus (`Menus`: the server's message of the day, then the team
/// menu, then the class menu; until it picks it is `core::Unassigned`),
/// or straight onto a team, as tests and scripted runs want: `Auto` the
/// smaller team (counter-terrorists on a tie), `Team` that one (`Team(0)`
/// the spectators). The local player's (single player, a listen server's
/// host) and, for a network client, the one it asks the server for
/// (`net`). The game's interactive runs use `Menus`
/// (`client::Args::join_choice`); the default is `Auto`.
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum JoinChoice {
    Menus,
    #[default]
    Auto,
    Team(Team),
}

impl JoinChoice {
    /// As a userinfo value (`net`'s join handshake): `menus`, `auto`, or
    /// the `jointeam` number.
    pub fn key(self) -> String {
        match self {
            Self::Menus => "menus".into(),
            Self::Auto => "auto".into(),
            Self::Team(t) => (t.0 + 1).to_string(),
        }
    }

    pub fn from_key(key: &str) -> Option<Self> {
        match key.trim() {
            "menus" => Some(Self::Menus),
            "auto" | "0" => Some(Self::Auto),
            "1" => Some(Self::Team(Team(0))),
            "2" => Some(Self::Team(Team(1))),
            "3" => Some(Self::Team(Team(2))),
            _ => None,
        }
    }
}

/// The maps played in turn (a game's `mapcycle.txt`, by short name, as
/// `changelevel` takes them) and the one loaded now. Games and the client
/// fill it.
#[derive(Resource, Clone, Debug, Default, PartialEq)]
pub struct MapCycle {
    pub maps: Vec<String>,
    pub current: String,
}

impl MapCycle {
    /// The map after the current one (wrapping; the first when the current
    /// one isn't in the cycle); None without a cycle.
    pub fn next(&self) -> Option<String> {
        let i = self.maps.iter().position(|m| m.eq_ignore_ascii_case(&self.current));
        let next = i.map_or(0, |i| (i + 1) % self.maps.len());
        self.maps.get(next).cloned()
    }
}

/// The dead's camera rules, the server's (replicated to clients; set by
/// the rules' cvars, CS:S's defaults measured on its dedicated server
/// 2026-10-15): `mp_forcecamera` (`force_camera`: 1 the dead on a team
/// watch only their team), `spec_freeze_time` (`freeze_hold`, seconds the
/// freeze cam's picture holds) and `spec_freeze_traveltime`
/// (`freeze_travel`, seconds its camera takes to reach the killer; at
/// least `FREEZE_TRAVEL_MIN`). Read by `client::spectate`.
#[derive(Resource, Clone, Copy, Debug, PartialEq)]
pub struct SpectatorRules {
    pub force_camera: u8,
    pub freeze_hold: f32,
    pub freeze_travel: f32,
}

/// `spec_freeze_traveltime` and `spec_freeze_time` defaults, seconds.
pub const FREEZE_TRAVEL: f32 = 0.7;
pub const FREEZE_HOLD: f32 = 5.0;
/// `spec_freeze_traveltime`'s minimum, seconds.
pub const FREEZE_TRAVEL_MIN: f32 = 0.01;

impl Default for SpectatorRules {
    fn default() -> Self {
        Self {
            force_camera: 1,
            freeze_hold: FREEZE_HOLD,
            freeze_travel: FREEZE_TRAVEL,
        }
    }
}

/// When the dead local player comes back, as the rules know it: set by
/// rules that respawn the dead a fixed time after their death and say so
/// (the minigame runtime: `chimera_sim::minigame::hud::update`, from its
/// definition's `respawn.delay`). `at` is `SimClock` seconds (a network
/// client's own clock, as its `Dead::since`); `delay` the whole wait from
/// the death. None while alive, or when nothing respawns the player on a
/// timer (plain CS:S's and Combat Arms' rules leave it unset: their death
/// views keep their own timings and screens). Read by the shell's death
/// view (`client::spectate`, which fits CS:S's death and freeze cams into
/// the wait) and its countdown (`client::respawn_countdown`).
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq)]
pub struct RespawnDue(pub Option<RespawnAt>);

/// A game's dead view instead of CS:S's death and freeze cams and
/// watching others (HL2:DM's, specs/hl2dm/rules.md "The dead view"):
/// while the local player is dead and not on the spectator team, the
/// camera sits `distance` units behind the point `height` units above its
/// ragdoll's root (its own eye without one), along its view, which the
/// mouse still turns, pulled in by a box of half-size `wall` against the
/// world. Distances in Source units. Read by `client::spectate`.
#[derive(Resource, Clone, Copy, Debug, PartialEq)]
pub struct OrbitDeadView {
    pub distance: f32,
    pub height: f32,
    pub wall: f32,
}

/// A timed respawn: at `at` (`SimClock` s), `delay` s after the death.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RespawnAt {
    pub at: f64,
    pub delay: f64,
}

/// `mp_fadetoblack` (CS:S's default 0): the dead see black
/// (`client::senses`; set by the rules' `mp_fadetoblack`).
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FadeToBlack(pub u8);


/// A remote player whose client is still loading the map (joining, or
/// after a map change): not in the game yet. It is dead, not solid and
/// at no health, so nothing hits it; the rules don't spawn it or count it
/// on its team until its client has the map (`rules::enter_game`), as
/// CS:S keeps connecting players out of the game.
#[derive(Component, Reflect, Default, Clone, Copy, Debug)]
#[reflect(Component)]
pub struct Connecting;

/// On CS:S's spectator team (`spectate`, `jointeam 1`): no team of its
/// own (`Team(0)`), dead, not solid, no health and no weapons, so it has
/// no body; the rules neither spawn it nor count it for a round's end
/// (`rules::join_spectators`). Its client watches others or flies free
/// (`client::spectate`). Replicated, so every client lists it.
#[derive(Component, Reflect, Default, Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[reflect(Component)]
pub struct Spectating;

/// In the game but on no team yet (CS:S's unassigned players): it has not
/// picked one in the team menu. Also `Spectating` (no body, watching,
/// counted on no team); `rules::join_team` or `rules::join_spectators`
/// ends it. Its client shows the join menus (the MOTD, then the team
/// menu: `games::cs_source::ui::join_menus`). Replicated.
#[derive(Component, Reflect, Default, Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[reflect(Component)]
pub struct Unassigned;

/// On a team, picked from the team menu after joining the game, but no
/// class (player model) chosen yet: still no body, neither spawned nor
/// counted for a round's end until `joinclass` (`rules::join_class`), as
/// CS:S waits for the class menu. Replicated.
#[derive(Component, Reflect, Default, Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[reflect(Component)]
pub struct ChoosingClass;

/// A player's class (CS:S's `joinclass` number: its player model): 1-4
/// the terrorists' (Phoenix Connexion, Elite Crew, Arctic Avengers,
/// Guerilla Warfare), 5-8 the counter-terrorists' (SEAL Team 6, GSG-9,
/// SAS, GIGN). Without one a character wears its team's first model.
/// Replicated; a game's loader draws it (`map::BodyName`).
#[derive(Component, Reflect, Default, Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[reflect(Component)]
#[component(storage = "SparseSet")]
pub struct PlayerClass(pub u8);

impl PlayerClass {
    /// Classes per team.
    pub const PER_TEAM: u8 = 4;

    /// The team (ours: 1 terrorists, 2 counter-terrorists) a class belongs to.
    pub fn team(self) -> Option<Team> {
        match self.0 {
            1..=4 => Some(Team(1)),
            5..=8 => Some(Team(2)),
            _ => None,
        }
    }
}

