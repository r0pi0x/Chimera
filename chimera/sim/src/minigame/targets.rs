//! Practice targets ("dummies") for a sandbox minigame (Ultimate Gun Game's
//! `sandbox`): a body from any game standing in front of the player, to
//! shoot at and measure against, and the measurement facts a "last kill"
//! card reads. Game-neutral: a target is a bot of the gameplay's bots'
//! game (`TargetBots`, the seam that game implements: Combat Arms'
//! `game::bots`) with a small rule on top (its home, its motion, whether it
//! may fire), put on a rung of the sandbox's ladder (`rung_command::
//! move_to`), which gives it that gun's game's body, health rules and gun
//! (`identity = "weapon"`), and held there like the player.
//!
//! **Requests** (messages, applied in `Update` by a system that queues the
//! world work, `commands.queue`; the console calls the same functions):
//! `SpawnTarget`, `SetTarget` (each field an option: None leaves it),
//! `RemoveTarget(entity)`, `ClearTargets`. A refusal is a `TargetRefused
//! { reason }` (and the console's error, or a warning in the log).
//!
//! - What it wears and holds, character first: `character` (a body id,
//!   `characters` lists them per game with display names) and `gun` (a
//!   gun id of the same game; None: the character's default, the first
//!   rung gun of its game that it may hold); or a `rung` (0-based) or `gun`
//!   alone, the body then the gun's game's default. Underneath it is always
//!   a rung of the sandbox's ladder, which holds every gun of every game,
//!   so a gun id is the rung that holds it. A game registers its bodies
//!   (`TargetCharacter`, `TargetsAppExt::target_character`: which guns
//!   each may hold, how it is worn); a game with none registered has one,
//!   `<game>:default`, the body its guns give. A body of one game with a
//!   gun of another is refused: the body follows the held gun's game
//!   (`identity`), and a cross-game pair needs that rule changed.
//! - Where: `distance_m` along the spawner's flat view direction from its
//!   position, on the floor found below that point (a ray down from 1.5 m
//!   above the spawner's height, 8 m long); refused when there is no
//!   floor (past the map's edge), the floor is too steep, or a standing
//!   player's box there meets the world (`spawns::stuck_at`: inside a
//!   wall). The spawn's origin and direction stay on `Target`, so a new
//!   `distance_m` re-places it along the same line.
//! - `motion`: `Still` (no movement keys; pushed off its spot, it walks
//!   back), `Strafes` (side to side across the spawner's line,
//!   `STRAFE_HALF` m each way, at its body's run speed; a leg blocked by a
//!   wall turns after `STRAFE_LEG` s), `Walks` (from one
//!   point to the next within `WALK_RADIUS` m of home, a fixed sequence).
//!   It never leaves its home area (`LEASH` m past it: it heads home).
//! - `shoots_back` (off by default): off, its trigger, secondary and game
//!   actions are kept released every tick (it still sees and turns); on,
//!   it fights as its game's bot does from where its motion keeps it.
//! - `protected` (off by default): `core::HealthHeld` (hits land and are
//!   measured, health never drops, it never dies), and its health and
//!   shield are put back to full at the start of each tick.
//! - `armored` (on by default): off, the body has no second bar at all
//!   (Halo's shield off, no Combat Arms gear, no CS:S kevlar): its
//!   `weapon::Armor` is held at 0 before each tick's hits, on spawn and
//!   after each respawn, and `TargetVitals` reads 0. Apart from
//!   `protected`.
//! - `respawn_secs` (3 s by default): `runtime::RespawnDelay`; while dead
//!   it carries `TargetRespawn { at }` (simulation time it returns, for a
//!   countdown ring at `home`); back, it is placed at home with the same
//!   settings, its rung and body again.
//!
//! **What the screens read**: `Target` (settings, home, origin and
//! direction, display name, game, character and its name, gun, rung, the
//! console's number), `TargetRespawn`, `TargetVitals` (health and shield
//! with their maxima, kept current each tick: `core::Health` and
//! `weapon::Armor`, the second bar of every game that has one: Halo's
//! shield, Combat Arms' gear, CS:S's kevlar), `characters`, and the facts:
//! - `HitMeasured`, one per attacker, victim and tick when a target is hit
//!   or a target hits anyone: the gun, `damage` (what health and shield
//!   lost, after every scale: the `DamageTaken`s plus the shield's drop),
//!   `damage_unscaled` (the `Damage`s as the weapons wrote them, before the
//!   victim's `DamageScale`, plus the shield's drop), the hit group of the
//!   best hit (`Hitgroup`, the head first) and the full region when the gun
//!   told it (`RegionHit`), the distance, the victim's health and shield
//!   before (the tick's start) and after, the hits (pellets) folded in, the
//!   time. Normalized like `Health` (1.0 = a standard player's 100).
//! - `KillMeasured`, when a target dies or kills: shots fired at the victim
//!   since its last death (`WeaponEventKind::ShotMade` rays passing within
//!   `AIM_RADIUS` m of its centre; never fewer than the hits), shots hit
//!   (ticks with a hit), headshots, the times from the first hit and the
//!   first shot to the kill, the distance, the total damage.
//!   Both are also logged (`HitMeasured ...`, `KillMeasured ...`).
//! - `BlastMeasured` ("Blasts"), one per blast while the sandbox plays,
//!   targets or not: built on what the blast's own code told
//!   (`weapon::grenade::BlastDealt`: the engine's blast, which CS:S's and
//!   HL2:DM's grenades, rockets and map explosions go through, Combat
//!   Arms' `blast_bodies`, Halo's detonation): the centre, the falloff's
//!   `inner` (full damage inside) and `outer` (none past), `max_damage`
//!   at the centre, `edge_damage` at `outer` (0 where the falloff reaches
//!   nothing there), and per victim (`BlastHit`) its point, distance from
//!   the centre, damage and whether it died that tick. A victim's damage
//!   is what its `HitMeasured` says for that attacker and tick (the
//!   blast's own number for a thing with no health). Linear falloffs from
//!   the centre have `inner` 0 (HL2:DM's, CS:S's grenade with
//!   `sv_legacy_grenade_damage 1`); Combat Arms' is full within a quarter
//!   of the radius; Halo's is full within its damaging effects' r0, none
//!   past their r1;
//!   a bell (CS:S's HE grenade and map explosions) has `inner` where it
//!   gives 99 % of the centre's and `outer` where it gives 1 %, no farther
//!   than its radius (`grenade::bell_reach`). Logged (`BlastMeasured ...`,
//!   `blast_line`).
//!
//! Every time here (`time`, `from_first_hit`, `from_first_shot`,
//! `TargetRespawn::at`) is simulation time (`SimClock`): `host_timescale
//! 0.5` runs the sandbox at half speed and the measurements stay in game
//! seconds.
//!
//! A target's second bar is its own game's or none: armour its bot was
//! given by another game (Combat Arms' gear) goes at each placing
//! (`strip_foreign_armour`), and the body's game gives its own (Halo a
//! full shield). For the tick it is placed in it can't be hit (one tick of
//! spawn protection: a respawn's identity switch holds the last game's
//! armour until the new game's step), and `TargetVitals` is kept after the
//! whole tick (`FixedPostUpdate`). Targets race nobody and count for no
//! one: the ladder's leaders and the match's scores leave them out
//! (`runtime`, `ladder`); the HUD's race chip and kill feed read
//! `is_target` (or `Has<Target>`) to leave them out.
//!
//! Placement needs no particular map: a ray down at the spot on any map's
//! collision; the motion moves by keys, with or without a nav mesh.
//!
//! Targets exist only while the sandbox plays (`rung_command::holds`): a
//! spawn is refused otherwise, and every target is removed when it stops
//! or the map is loaded again (`core::MapLoads`). Nothing here is
//! replicated (the sandbox is a local match).
//!
//! Console: `target add <rung n | gun id | character id> [distance m]
//! [still|strafe|walk] [shoots] [protect]`, `target set <n|all> <shoots |
//! noshoot | protect | noprotect | still | strafe | walk | gun <id> |
//! character <id> | respawn <s> | distance <m>>`, `target remove <n|all>`,
//! `target list`, `target characters`, `target look <n>` (turns the
//! player to face target n: staging a picture).
//!
//! Known shortcuts:
//! - The motion overrides the bot's movement keys after its brain
//!   (between `SimSet::Rules` and `SimSet::Commands`); the brain still
//!   plans its own route, which the keys ignore.
//! - A shot counts as aimed at a victim when its ray passes within
//!   `AIM_RADIUS` of the victim's centre: a lobbed grenade's or an arcing
//!   projectile's is counted by its launch line.
//! - Several hits on one victim by one attacker in one tick are one
//!   `HitMeasured` (a shotgun's pellets: `hits`); the shield's drop in a
//!   tick goes to the first attacker of that victim. A blast's victim hit
//!   by the same attacker in the same tick another way too (a rocket's
//!   direct hit, a second blast) has all of it in its `BlastHit`.
//! - A protected target's armour or shield is put back on the next tick:
//!   for one tick `TargetVitals` shows the drop.
//! - Bodies: only what a game registered (or its default); TF2's class and
//!   Halo's Spartan are registered by the party kit
//!   (apps/party_kit/src/target_bodies.rs).

use std::collections::BTreeMap;

use avian3d::prelude::{SpatialQuery, SpatialQueryFilter};
use bevy::{ecs::system::SystemState, prelude::*};

use super::{
    ladder::{Ladder, Rung},
    rung_command, runtime,
};
use crate::{
    console::{ConsoleAppExt, command_player},
    core::{
        BodyRegion, CHARACTER_FILTER, Connecting, Damage, DamageTaken, Dead, Died, HealthHeld, Health, Hitgroup,
        Intent, MapLoads, RegionHit, SimClock, SimSet,
    },
    weapon::{Armor, Inventory, Weapon, WeaponEvent, WeaponEventKind, grenade::BlastDealt},
};

/// The practice props (`props`: bottles, tin cans, a target board), named
/// here as the screens name them.
pub use super::props::{ClearProps, Prop, PropHit, PropSpec, RemoveProp, ResetProp, ShotGroup, SpawnProp};

/// Half the strafe's span (m): it goes this far each way of home.
pub const STRAFE_HALF: f32 = 3.0;
/// The longest a strafe's leg lasts before it turns (s): a wall in its
/// way doesn't hold it.
pub const STRAFE_LEG: f64 = 3.0;
/// How far a walking target wanders from home (m).
pub const WALK_RADIUS: f32 = 4.0;
/// How far past its area a target may be pushed before it heads home (m).
pub const LEASH: f32 = 1.5;
/// A shot whose ray passes this close to a victim's centre was at it (m).
pub const AIM_RADIUS: f32 = 1.0;
/// The respawn delay by default (s).
pub const RESPAWN_SECS: f32 = 3.0;
/// The distances the screens offer (m).
pub const DISTANCES: [f32; 6] = [5.0, 10.0, 20.0, 40.0, 100.0, 200.0];

/// How a target moves.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TargetMotion {
    #[default]
    Still,
    Strafes,
    Walks,
}

impl TargetMotion {
    pub fn parse(s: &str) -> Option<Self> {
        match s.to_ascii_lowercase().as_str() {
            "still" => Some(Self::Still),
            "strafe" | "strafes" => Some(Self::Strafes),
            "walk" | "walks" => Some(Self::Walks),
            _ => None,
        }
    }

    pub fn word(self) -> &'static str {
        match self {
            Self::Still => "still",
            Self::Strafes => "strafe",
            Self::Walks => "walk",
        }
    }
}

/// Spawn a target (module doc, "Requests").
#[derive(Message, Clone, Debug, PartialEq)]
pub struct SpawnTarget {
    /// A body id (`characters`); None: the gun's game's default body.
    pub character: Option<String>,
    /// A gun id of the character's game; None: the character's default
    /// (or the rung's gun).
    pub gun: Option<String>,
    /// A rung of the ladder (0-based), instead of a gun.
    pub rung: Option<usize>,
    pub distance_m: f32,
    pub motion: TargetMotion,
    pub shoots_back: bool,
    pub protected: bool,
    /// Its body's second bar (shield, gear, kevlar); false: none at all.
    pub armored: bool,
    pub respawn_secs: f32,
    /// Whose position and view it is placed from (None: the local player).
    pub by: Option<Entity>,
}

impl Default for SpawnTarget {
    fn default() -> Self {
        Self {
            character: None,
            gun: None,
            rung: None,
            distance_m: 10.0,
            motion: TargetMotion::Still,
            shoots_back: false,
            protected: false,
            armored: true,
            respawn_secs: RESPAWN_SECS,
            by: None,
        }
    }
}

/// Change a target's settings (None: as it is). A new `character`, `gun`
/// or `rung` re-arms it; a new `distance_m` re-places it along its line.
#[derive(Message, Clone, Debug, PartialEq)]
pub struct SetTarget {
    pub target: Entity,
    pub character: Option<String>,
    pub gun: Option<String>,
    pub rung: Option<usize>,
    pub distance_m: Option<f32>,
    pub motion: Option<TargetMotion>,
    pub shoots_back: Option<bool>,
    pub protected: Option<bool>,
    pub armored: Option<bool>,
    pub respawn_secs: Option<f32>,
}

impl SetTarget {
    /// A change to `target` that leaves everything as it is (set the
    /// fields wanted).
    pub fn of(target: Entity) -> Self {
        Self {
            target,
            character: None,
            gun: None,
            rung: None,
            distance_m: None,
            motion: None,
            shoots_back: None,
            protected: None,
            armored: None,
            respawn_secs: None,
        }
    }
}

/// Remove one target.
#[derive(Message, Clone, Copy, Debug, PartialEq)]
pub struct RemoveTarget(pub Entity);

/// Remove every target.
#[derive(Message, Clone, Copy, Debug, Default, PartialEq)]
pub struct ClearTargets;

/// A request refused, and why (as the console says it).
#[derive(Message, Clone, Debug, PartialEq)]
pub struct TargetRefused {
    pub reason: String,
}

/// A practice target: on the bot that is its body, alive or dead.
#[derive(Component, Clone, Debug, PartialEq)]
pub struct Target {
    pub motion: TargetMotion,
    pub shoots_back: bool,
    pub protected: bool,
    /// Its second bar on (false: none, `TargetVitals` reads 0).
    pub armored: bool,
    pub respawn_secs: f32,
    /// Its feet's spot (m).
    pub home: Vec3,
    /// Where the spawner stood (its centre) and the flat unit direction it
    /// was placed along, and how far.
    pub origin: Vec3,
    pub dir: Vec3,
    pub distance_m: f32,
    /// The gun it holds and the rung that holds it (0-based).
    pub gun: String,
    pub rung: usize,
    /// Its body's id and display name, its game's id and display name.
    pub character: String,
    pub character_name: String,
    pub game: String,
    /// The game's full name ("Combat Arms") and short tag ("CA").
    pub game_name: String,
    pub game_tag: String,
    /// Its display name (`Target 2`) and the console's number for it.
    pub name: String,
    pub number: u32,
}

/// Whether `e` is a practice target (the HUD's race chip and kill feed
/// leave them out; a system reads `Has<Target>` or `Without<Target>`, or
/// below this crate `core::PracticeTarget`, the marker every target wears).
pub fn is_target(world: &World, e: Entity) -> bool {
    world.get::<Target>(e).is_some()
}

/// A dead target comes back at this simulation time (`SimClock` s).
#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub struct TargetRespawn {
    pub at: f64,
}

/// A target's health and second bar (shield, armour, gear) now, with their
/// maxima, normalized like `Health` (module doc).
#[derive(Component, Clone, Copy, Debug, Default, PartialEq)]
pub struct TargetVitals {
    pub health: f32,
    pub health_max: f32,
    pub shield: f32,
    /// The most the second bar held this life (0: the body has none).
    pub shield_max: f32,
}

/// A hit on a target, or by one (module doc).
#[derive(Message, Clone, Debug, PartialEq)]
pub struct HitMeasured {
    pub attacker: Option<Entity>,
    pub victim: Entity,
    pub gun: Option<&'static str>,
    pub damage: f32,
    pub damage_unscaled: f32,
    pub hitgroup: Hitgroup,
    pub region: Option<BodyRegion>,
    pub distance_m: f32,
    pub health_before: f32,
    pub health_after: f32,
    pub shield_before: f32,
    pub shield_after: f32,
    /// The hits folded into this one (pellets).
    pub hits: u32,
    pub time: f64,
}

/// A kill of a target, or by one (module doc).
#[derive(Message, Clone, Debug, PartialEq)]
pub struct KillMeasured {
    pub attacker: Option<Entity>,
    pub victim: Entity,
    pub gun: Option<&'static str>,
    pub shots_fired: u32,
    pub shots_hit: u32,
    pub headshots: u32,
    /// Seconds from the first hit, and from the first shot, to the kill.
    pub from_first_hit: f32,
    pub from_first_shot: f32,
    pub distance_m: f32,
    pub total_damage: f32,
    pub time: f64,
}

/// One blast while the sandbox plays (module doc, "Blasts").
#[derive(Message, Clone, Debug, PartialEq)]
pub struct BlastMeasured {
    pub attacker: Option<Entity>,
    pub gun: Option<&'static str>,
    /// The centre, world m.
    pub at: Vec3,
    /// Full damage inside, m.
    pub inner: f32,
    /// None past, m.
    pub outer: f32,
    /// At the centre, normalized like `Health` (1.0 = a standard
    /// player's 100).
    pub max_damage: f32,
    /// At `outer`, normalized (`weapon::grenade::BlastDealt::edge_damage`:
    /// 0 where the falloff reaches nothing at the radius).
    pub edge_damage: f32,
    pub hits: Vec<BlastHit>,
    /// `SimClock` s (simulation time).
    pub time: f64,
}

/// One victim of a `BlastMeasured`.
#[derive(Clone, Debug, PartialEq)]
pub struct BlastHit {
    pub victim: Entity,
    pub at: Vec3,
    pub distance_m: f32,
    /// As `HitMeasured::damage` (normalized).
    pub damage: f32,
    pub killed: bool,
}

/// The seam a game's bots give targets (`gameplay::GameHooks`, by the
/// gameplay's `bots` slot): add one bot to the match, remove one.
#[derive(Clone, Copy)]
pub struct TargetBots {
    pub add: fn(&mut World) -> Option<Entity>,
    pub remove: fn(&mut World, Entity),
}

/// A body a target may wear (a game's registration).
#[derive(Clone, Copy)]
pub struct TargetCharacter {
    /// Its id (`tf2:heavy`) and display name.
    pub id: &'static str,
    pub name: &'static str,
    pub game: &'static str,
    /// Whether it may hold gun `gun` (its id; the character's id given).
    pub holds: fn(&World, &str, &str) -> bool,
    /// Make target `e` this body (the character's id given), after each
    /// arming (a spawn, a rung set).
    pub wear: fn(&mut World, Entity, &str),
    /// What a fresh body has: health and second bar at their full
    /// (normalized like `TargetVitals`; the character's id given).
    pub vitals: fn(&World, &str) -> (f32, f32),
}

/// The registered bodies.
#[derive(Resource, Clone, Default)]
pub struct TargetCharacters(pub Vec<TargetCharacter>);

/// Registering a game's target bodies.
pub trait TargetsAppExt {
    fn target_character(&mut self, c: TargetCharacter) -> &mut Self;
    /// A game's full display name ("Combat Arms").
    fn target_game_name(&mut self, game: &'static str, name: &'static str) -> &mut Self;
}

impl TargetsAppExt for App {
    fn target_character(&mut self, c: TargetCharacter) -> &mut Self {
        self.world_mut().get_resource_or_init::<TargetCharacters>().0.push(c);
        self
    }

    fn target_game_name(&mut self, game: &'static str, name: &'static str) -> &mut Self {
        self.world_mut().get_resource_or_init::<TargetGameNames>().0.push((game, name));
        self
    }
}

/// A body as the screens list it.
#[derive(Clone, Debug, PartialEq)]
pub struct CharacterInfo {
    pub id: String,
    pub name: String,
    pub game: String,
    /// The game's full name ("Combat Arms") and its short tag ("CA").
    pub game_name: String,
    pub game_tag: String,
    /// What a fresh body of it has (normalized like `TargetVitals`).
    pub health_max: f32,
    pub shield_max: f32,
    /// The guns it may hold among the sandbox ladder's rungs, in ladder
    /// order.
    pub guns: Vec<String>,
}

/// The bodies a target may wear, per game of the playing ladder (in its
/// order): each game's registered ones, else its `<game>:default`.
pub fn characters(world: &World) -> Vec<CharacterInfo> {
    let mut out = Vec::new();
    for game in ladder_games(world) {
        let own: Vec<&TargetCharacter> = registered(world).filter(|c| c.game == game).collect();
        if own.is_empty() {
            out.push(default_info(world, &game));
        }
        for c in own {
            let (health_max, shield_max) = (c.vitals)(world, c.id);
            out.push(CharacterInfo {
                id: c.id.into(),
                name: c.name.into(),
                game: game.clone(),
                game_name: game_name(world, &game),
                game_tag: game_tag(world, &game),
                health_max,
                shield_max,
                guns: guns_of(world, &game, Some(c)),
            });
        }
    }
    out
}

/// One body's listing (`characters`), by id.
pub fn character_info(world: &World, id: &str) -> Option<CharacterInfo> {
    characters(world).into_iter().find(|c| c.id.eq_ignore_ascii_case(id))
}

fn registered(world: &World) -> impl Iterator<Item = &TargetCharacter> {
    world.get_resource::<TargetCharacters>().into_iter().flat_map(|c| c.0.iter())
}

fn game_of(id: &str) -> &str {
    id.split_once(':').map_or(id, |(g, _)| g)
}

/// A game's full name: registered (`TargetsAppExt::target_game_name`),
/// else its Steam name, else its short tag.
fn game_name(world: &World, game: &str) -> String {
    if let Some(n) = world.get_resource::<TargetGameNames>().and_then(|n| n.0.iter().find(|(g, _)| *g == game)) {
        return n.1.to_string();
    }
    let info = world.get_resource::<chimera_core::game_registry::Games>().and_then(|g| g.get(game));
    info.and_then(|g| g.steam.as_ref().map(|s| s.name.to_string())).unwrap_or_else(|| game_tag(world, game))
}

/// A game's short tag (`GameInfo::short_name`: "CA").
fn game_tag(world: &World, game: &str) -> String {
    world
        .get_resource::<chimera_core::game_registry::Games>()
        .and_then(|g| g.get(game))
        .map_or_else(|| game.to_string(), |g| g.short_name.to_string())
}

/// The rung guns of `game` a body (`c`; None: the game's default) may hold.
fn guns_of(world: &World, game: &str, c: Option<&TargetCharacter>) -> Vec<String> {
    let Some(l) = world.get_resource::<Ladder>() else { return Vec::new() };
    let mut out: Vec<String> = Vec::new();
    for g in &l.rungs[..usable(l)] {
        if game_of(g) == game && may_hold(world, c, g) && !out.iter().any(|x| x == g) {
            out.push(g.to_string());
        }
    }
    out
}

fn default_info(world: &World, game: &str) -> CharacterInfo {
    let tag = game_tag(world, game);
    CharacterInfo {
        id: format!("{game}:default"),
        name: format!("{tag} default"),
        game: game.into(),
        game_name: game_name(world, game),
        game_tag: tag,
        // Not registered: a standard player's health and no second bar
        // (assumed; a game registers its bodies' real numbers).
        health_max: 1.0,
        shield_max: 0.0,
        guns: guns_of(world, game, None),
    }
}

/// Games' full names for the screens (`TargetsAppExt::target_game_name`).
#[derive(Resource, Clone, Default)]
pub struct TargetGameNames(pub Vec<(&'static str, &'static str)>);

/// The rungs a target may take: all but the sandbox's sentinel.
fn usable(ladder: &Ladder) -> usize {
    ladder.rungs.len().saturating_sub(1)
}

fn ladder_games(world: &World) -> Vec<String> {
    let mut games: Vec<String> = Vec::new();
    if let Some(l) = world.get_resource::<Ladder>() {
        for id in &l.rungs[..usable(l)] {
            let g = game_of(id);
            if !games.iter().any(|x| x == g) {
                games.push(g.to_string());
            }
        }
    }
    games
}

/// What a request's character, gun and rung come to.
struct Arming {
    rung: usize,
    gun: &'static str,
    info: CharacterInfo,
    wear: Option<TargetCharacter>,
}

fn may_hold(world: &World, c: Option<&TargetCharacter>, gun: &str) -> bool {
    c.is_none_or(|c| (c.holds)(world, c.id, gun))
}

fn resolve(world: &World, character: Option<&str>, gun: Option<&str>, rung: Option<usize>) -> Result<Arming, String> {
    let ladder = world.get_resource::<Ladder>().ok_or("no ladder plays")?;
    let n = usable(ladder);
    let rung_of = |gun: &str| ladder.rungs[..n].iter().position(|r| r.eq_ignore_ascii_case(gun));
    let mut at = match (rung, gun) {
        (Some(r), _) if r >= n => return Err(format!("rung {} is past the ladder's {n} (`rung list`)", r + 1)),
        (Some(r), _) => Some(r),
        (None, Some(g)) => Some(rung_of(g).ok_or_else(|| format!("no rung holds {g:?} (`rung list`)"))?),
        (None, None) => None,
    };
    let reg = |id: &str| registered(world).find(|c| c.id.eq_ignore_ascii_case(id)).copied();
    let (info, wear) = match character {
        Some(id) => {
            let info = character_info(world, id).ok_or_else(|| format!("no body {id:?} (`target characters`)"))?;
            (info, reg(id))
        }
        None => {
            let r = at.ok_or("name a body, a gun or a rung")?;
            let game = game_of(ladder.rungs[r]);
            match registered(world).find(|c| c.game == game && (c.holds)(world, c.id, ladder.rungs[r])) {
                Some(c) => (character_info(world, c.id).unwrap_or_else(|| default_info(world, game)), Some(*c)),
                None => (default_info(world, game), None),
            }
        }
    };
    if at.is_none() {
        at = ladder.rungs[..n]
            .iter()
            .position(|g| game_of(g) == info.game && may_hold(world, wear.as_ref(), g));
    }
    let r = at.ok_or_else(|| format!("no rung holds a gun {} may hold", info.name))?;
    let gun = ladder.rungs[r];
    if game_of(gun) != info.game {
        return Err(format!(
            "{} is {}'s body and {gun} is {}'s gun: a body follows the game of the gun it holds, so a body of one game \
             with another's gun isn't possible yet",
            info.name,
            info.game_name,
            game_name(world, game_of(gun))
        ));
    }
    if !may_hold(world, wear.as_ref(), gun) {
        return Err(format!("{} can't hold {gun}", info.name));
    }
    Ok(Arming { rung: r, gun, info, wear })
}

/// Where a target stands `distance` m from `origin` along `dir` (flat):
/// its feet, or why there is no spot.
pub(super) fn find_spot(world: &mut World, origin: Vec3, dir: Vec3, distance: f32) -> Result<Vec3, String> {
    if !(distance.is_finite() && distance > 0.5) {
        return Err(format!("a distance of {distance} m is too short"));
    }
    let characters: Vec<Entity> = world.query_filtered::<Entity, With<Intent>>().iter(world).collect();
    let mut state: SystemState<SpatialQuery> = SystemState::new(world);
    let q = state.get(world).map_err(|e| e.to_string())?;
    let filter = SpatialQueryFilter::from_mask(CHARACTER_FILTER).with_excluded_entities(characters);
    let at = origin + dir * distance;
    let from = Vec3::new(at.x, origin.y + 1.5, at.z);
    let Some(hit) = q.cast_ray(from, Dir3::NEG_Y, 8.0, true, &filter) else {
        return Err(format!("no floor {distance} m ahead (past the map's edge or over a drop)"));
    };
    if hit.distance < 0.05 {
        return Err(format!("the spot {distance} m ahead is inside a wall"));
    }
    if hit.normal.y < 0.7 {
        return Err(format!("the floor {distance} m ahead is too steep to stand on"));
    }
    let feet = Vec3::new(at.x, from.y - hit.distance, at.z);
    if super::spawns::stuck_at(&q, &filter, feet) {
        return Err(format!("the spot {distance} m ahead is inside a wall"));
    }
    Ok(feet)
}

/// The motion's state and the map the target was made on.
#[derive(Component, Clone, Copy, Debug, Default)]
struct TargetDrive {
    sign: f32,
    waypoint: u32,
    until: f64,
    map_loads: u32,
}

/// A target that is still to be put at home (a new one, or back from the
/// dead: `TargetRespawn`).
#[derive(Component, Clone, Copy, Debug)]
struct TargetPending;

/// The console's next number.
#[derive(Resource, Default)]
struct TargetCount(u32);

/// Spawn a target now; the bot it is.
pub fn spawn(world: &mut World, req: &SpawnTarget) -> Result<Entity, String> {
    if !rung_command::holds(world) {
        return Err("targets are the sandbox's: start it with `sandbox`".into());
    }
    let arming = resolve(world, req.character.as_deref(), req.gun.as_deref(), req.rung)?;
    let by = match req.by {
        Some(e) => e,
        None => command_player(world).map_err(|_| "no player to place it in front of".to_string())?,
    };
    let origin = world.get::<Transform>(by).ok_or("the player has no place")?.translation;
    let yaw = world.get::<Intent>(by).map_or(0.0, |i| i.yaw);
    let dir = Vec3::new(-yaw.sin(), 0.0, -yaw.cos());
    let home = find_spot(world, origin, dir, req.distance_m)?;
    let hooks = chimera_core::gameplay::active_hook::<TargetBots>(world, |g| g.bots)
        .ok_or("no game's bots play here to be targets")?;
    let e = (hooks.add)(world).ok_or("no bot could be added")?;
    let number = {
        let mut c = world.get_resource_or_init::<TargetCount>();
        c.0 += 1;
        c.0
    };
    let name = format!("Target {number}");
    let map_loads = world.get_resource::<MapLoads>().map_or(0, |m| m.0);
    let target = Target {
        motion: req.motion,
        shoots_back: req.shoots_back,
        protected: req.protected,
        armored: req.armored,
        respawn_secs: req.respawn_secs.max(0.0),
        home,
        origin,
        dir,
        distance_m: req.distance_m,
        gun: arming.gun.to_string(),
        rung: arming.rung,
        character: arming.info.id.clone(),
        character_name: arming.info.name.clone(),
        game: arming.info.game.clone(),
        game_name: arming.info.game_name.clone(),
        game_tag: arming.info.game_tag.clone(),
        name: name.clone(),
        number,
    };
    world.entity_mut(e).insert((
        Name::new(format!("{name} ({})", arming.info.name)),
        runtime::RespawnDelay(target.respawn_secs),
        target,
        TargetDrive { sign: 1.0, map_loads, ..default() },
        TargetVitals::default(),
        TargetPending,
        crate::core::PracticeTarget,
    ));
    if req.protected {
        world.entity_mut(e).insert(HealthHeld);
    }
    Ok(e)
}

/// Put target `e` at home, armed and dressed: its first placing and
/// every respawn. False while it can't be yet (not in the match).
fn place(world: &mut World, e: Entity) -> bool {
    let Some(t) = world.get::<Target>(e).cloned() else { return false };
    if world.get::<Rung>(e).is_none() || world.get::<Dead>(e).is_some() || world.get::<Connecting>(e).is_some() {
        return false;
    }
    let Some(ladder) = world.get_resource::<Ladder>().cloned() else { return false };
    let d = t.origin - t.home;
    let yaw = (-d.x).atan2(-d.z);
    let at = Transform::from_translation(t.home + Vec3::Y * crate::map::SPAWN_LIFT)
        .with_rotation(Quat::from_euler(EulerRot::YXZ, yaw, 0.0, 0.0));
    crate::spawning::revive(world, e, Some(at));
    // Hittable from the next tick: this one's identity switch may still
    // hold another game's armour (`Armor`) until its own game's step.
    runtime::unprotect(world, e);
    let now = world.resource::<SimClock>().now;
    let tick = world.resource::<SimClock>().delta.as_secs_f64().max(1e-3);
    world.entity_mut(e).insert((
        crate::core::SpawnProtected { damage_until: now + tick, bullets_until: now + tick },
        runtime::Protected { fire_ends: false },
        crate::core::God,
    ));
    strip_foreign_armour(world, e, &t.game);
    rung_command::move_to(world, e, t.rung, &ladder);
    wear(world, e, &t.character);
    if let Ok(mut ent) = world.get_entity_mut(e) {
        ent.remove::<(TargetPending, TargetRespawn)>();
        if let Some(mut d) = ent.get_mut::<TargetDrive>() {
            d.until = now;
        }
        if let Some(mut v) = ent.get_mut::<TargetVitals>() {
            v.shield_max = 0.0;
        }
    }
    true
}

/// A target's second bar is its own game's or none: armour another game
/// gave its bot (Combat Arms' gear on a TF2 body) goes, unless the bot's
/// own game is the target's or the armour is pieces of the target's game
/// (`weapon::PiecesOf`: Halo's shield). The body's game then gives what it
/// gives a character becoming its own (Halo a full shield).
fn strip_foreign_armour(world: &mut World, e: Entity, game: &str) {
    let bots = world.get_resource::<chimera_core::gameplay::Gameplay>().map(|g| g.bots);
    let Ok(mut ent) = world.get_entity_mut(e) else { return };
    let own = ent.get::<crate::weapon::PiecesOf>().is_some_and(|p| p.0 == game);
    if ent.contains::<Armor>() && bots != Some(game) && !own {
        ent.remove::<(Armor, crate::weapon::ArmorPieces, crate::weapon::ArmorRatio, crate::weapon::PiecesOf)>();
    }
}

fn wear(world: &mut World, e: Entity, character: &str) {
    let found = registered(world).find(|c| c.id.eq_ignore_ascii_case(character)).copied();
    if let Some(c) = found {
        (c.wear)(world, e, c.id);
    }
}

/// Change target `e` (`SetTarget`): what changed, said.
pub fn set(world: &mut World, req: &SetTarget) -> Result<String, String> {
    let e = req.target;
    let mut t = world.get::<Target>(e).cloned().ok_or("no such target")?;
    let rearm = req.character.is_some() || req.gun.is_some() || req.rung.is_some();
    let home = match req.distance_m {
        Some(d) => Some(find_spot(world, t.origin, t.dir, d)?),
        None => None,
    };
    if rearm {
        // A new gun alone keeps the body when it may hold it.
        let keep = req.character.is_none() && req.rung.is_none();
        let character = req.character.clone().or_else(|| keep.then(|| t.character.clone()));
        let arming = match resolve(world, character.as_deref(), req.gun.as_deref(), req.rung) {
            Err(_) if keep => resolve(world, None, req.gun.as_deref(), None)?,
            r => r?,
        };
        t.rung = arming.rung;
        t.gun = arming.gun.to_string();
        t.character = arming.info.id;
        t.character_name = arming.info.name;
        t.game = arming.info.game;
        t.game_name = arming.info.game_name;
        t.game_tag = arming.info.game_tag;
        let _ = arming.wear;
    }
    if let (Some(h), Some(d)) = (home, req.distance_m) {
        t.home = h;
        t.distance_m = d;
    }
    t.motion = req.motion.unwrap_or(t.motion);
    t.shoots_back = req.shoots_back.unwrap_or(t.shoots_back);
    t.protected = req.protected.unwrap_or(t.protected);
    let armour_off = req.armored == Some(false) && t.armored;
    t.armored = req.armored.unwrap_or(t.armored);
    t.respawn_secs = req.respawn_secs.map_or(t.respawn_secs, |s| s.max(0.0));
    let said = describe(&t);
    let alive = world.get::<Dead>(e).is_none();
    {
        let mut ent = world.entity_mut(e);
        ent.insert((runtime::RespawnDelay(t.respawn_secs), Name::new(format!("{} ({})", t.name, t.character_name))));
        if t.protected {
            ent.insert(HealthHeld);
        } else {
            ent.remove::<HealthHeld>();
        }
        ent.insert(t);
        if armour_off && let Some(mut v) = ent.get_mut::<TargetVitals>() {
            v.shield = 0.0;
            v.shield_max = 0.0;
        }
    }
    if alive && (rearm || home.is_some()) {
        world.entity_mut(e).insert(TargetPending);
    }
    Ok(said)
}

/// Remove target `e`.
pub fn remove(world: &mut World, e: Entity) {
    if world.get::<Target>(e).is_none() {
        return;
    }
    match chimera_core::gameplay::active_hook::<TargetBots>(world, |g| g.bots) {
        Some(h) => (h.remove)(world, e),
        None => {
            if let Ok(ent) = world.get_entity_mut(e) {
                ent.despawn();
            }
        }
    }
}

/// Remove every target; how many.
pub fn clear(world: &mut World) -> usize {
    let all = all_targets(world);
    for (e, _) in &all {
        remove(world, *e);
    }
    all.len()
}

/// Every target, by number.
pub fn all_targets(world: &mut World) -> Vec<(Entity, Target)> {
    let mut all: Vec<(Entity, Target)> = world.query::<(Entity, &Target)>().iter(world).map(|(e, t)| (e, t.clone())).collect();
    all.sort_by_key(|(_, t)| t.number);
    all
}

fn describe(t: &Target) -> String {
    let flags = [
        (t.shoots_back, "shoots back"),
        (t.protected, "protected"),
        (!t.armored, "no armour"),
    ]
    .iter()
    .filter(|(on, _)| *on)
    .map(|(_, s)| *s)
    .collect::<Vec<_>>()
    .join(", ");
    format!(
        "{}: {} ({}) with {} (rung {}), {:.0} m, {}{}{}, respawn {:.1} s",
        t.name,
        t.character_name,
        t.game_tag,
        t.gun,
        t.rung + 1,
        t.distance_m,
        t.motion.word(),
        if flags.is_empty() { "" } else { ", " },
        flags,
        t.respawn_secs
    )
}

/// `target list`'s text.
fn list(world: &mut World) -> String {
    let now = world.get_resource::<SimClock>().map_or(0.0, |c| c.now);
    let all = all_targets(world);
    if all.is_empty() {
        return "no targets (`target add <rung | gun | body> [distance] [still|strafe|walk]`)".into();
    }
    let mut out = format!("{} targets:", all.len());
    for (e, t) in all {
        let v = world.get::<TargetVitals>(e).copied().unwrap_or_default();
        let state = match world.get::<TargetRespawn>(e) {
            Some(r) => format!("dead, back in {:.1} s", (r.at - now).max(0.0)),
            None => format!("health {:.0}/{:.0} shield {:.0}/{:.0}", v.health * 100.0, v.health_max * 100.0, v.shield * 100.0, v.shield_max * 100.0),
        };
        out.push_str(&format!("\n{:>3}  {}; {state}", t.number, describe(&t)));
    }
    out
}

/// The targets a `<n|all>` names.
fn pick(world: &mut World, which: &str) -> Result<Vec<Entity>, String> {
    let all = all_targets(world);
    if which.eq_ignore_ascii_case("all") {
        return Ok(all.into_iter().map(|(e, _)| e).collect());
    }
    let n: u32 = which.parse().map_err(|_| format!("not a target number: {which} (`target list`)"))?;
    all.into_iter().find(|(_, t)| t.number == n).map(|(e, _)| vec![e]).ok_or_else(|| format!("no target {n} (`target list`)"))
}

const USAGE: &str = "target add <rung n | gun id | body id> [distance m] [still|strafe|walk] [shoots] [protect] [noarmor] | \
    target set <n|all> <shoots|noshoot|protect|noprotect|armor|noarmor|still|strafe|walk|gun <id>|character <id>|respawn <s>|distance <m>> | \
    target remove <n|all> | target list | target characters | target look <n>";

/// A gun id, exact or the one gun of the ladder whose id holds the text.
fn gun_named(world: &World, text: &str) -> Result<String, String> {
    let ladder = world.get_resource::<Ladder>().ok_or("no ladder plays")?;
    let rungs = &ladder.rungs[..usable(ladder)];
    if let Some(g) = rungs.iter().find(|r| r.eq_ignore_ascii_case(text)) {
        return Ok(g.to_string());
    }
    let t = text.to_lowercase();
    let mut found: Vec<&str> = rungs.iter().copied().filter(|r| r.to_lowercase().contains(&t)).collect();
    found.dedup();
    match found.as_slice() {
        [g] => Ok(g.to_string()),
        [] => Err(format!("no gun, body or rung matches {text:?} (`rung list`, `target characters`)")),
        many => Err(format!("{text:?} fits several guns: {} (name one fully)", many.join(", "))),
    }
}

fn command(world: &mut World, a: &[String]) -> Result<Option<String>, String> {
    let sub = a.first().map(|s| s.to_ascii_lowercase()).ok_or(USAGE)?;
    match sub.as_str() {
        "add" => {
            let what = a.get(1).ok_or(USAGE)?;
            let mut req = SpawnTarget::default();
            if let Ok(n) = what.parse::<usize>() {
                req.rung = Some(n.checked_sub(1).ok_or("rungs are numbered from 1 (`rung list`)")?);
            } else if character_info(world, what).is_some() {
                req.character = Some(what.clone());
            } else {
                req.gun = Some(gun_named(world, what)?);
            }
            for w in &a[2..] {
                if let Ok(d) = w.trim_end_matches('m').parse::<f32>() {
                    req.distance_m = d;
                } else if let Some(m) = TargetMotion::parse(w) {
                    req.motion = m;
                } else if w.eq_ignore_ascii_case("shoots") {
                    req.shoots_back = true;
                } else if w.eq_ignore_ascii_case("protect") {
                    req.protected = true;
                } else if w.eq_ignore_ascii_case("noarmor") {
                    req.armored = false;
                } else if w.eq_ignore_ascii_case("armor") {
                    req.armored = true;
                } else {
                    return Err(format!("what is {w:?}? {USAGE}"));
                }
            }
            let e = spawn(world, &req)?;
            Ok(Some(format!("added {}", world.get::<Target>(e).map(describe).unwrap_or_default())))
        }
        "set" => {
            let which = a.get(1).ok_or(USAGE)?;
            let what = a.get(2).map(|s| s.to_ascii_lowercase()).ok_or(USAGE)?;
            let arg = a.get(3);
            let mut said = Vec::new();
            for e in pick(world, which)? {
                let mut req = SetTarget::of(e);
                match what.as_str() {
                    "shoots" => req.shoots_back = Some(true),
                    "noshoot" => req.shoots_back = Some(false),
                    "protect" => req.protected = Some(true),
                    "noprotect" => req.protected = Some(false),
                    "armor" => req.armored = Some(true),
                    "noarmor" => req.armored = Some(false),
                    "gun" => req.gun = Some(gun_named(world, arg.ok_or("gun <id>")?)?),
                    "character" | "body" => req.character = Some(arg.ok_or("character <id>")?.clone()),
                    "respawn" => req.respawn_secs = Some(arg.and_then(|s| s.parse().ok()).ok_or("respawn <seconds>")?),
                    "distance" => req.distance_m = Some(arg.and_then(|s| s.trim_end_matches('m').parse().ok()).ok_or("distance <m>")?),
                    w => req.motion = Some(TargetMotion::parse(w).ok_or_else(|| format!("what is {w:?}? {USAGE}"))?),
                }
                said.push(set(world, &req)?);
            }
            Ok(Some(said.join("\n")))
        }
        "remove" | "kick" => {
            let which = a.get(1).map_or("all", String::as_str);
            let all = pick(world, which)?;
            for e in &all {
                remove(world, *e);
            }
            Ok(Some(format!("removed {} targets", all.len())))
        }
        "look" => {
            // Turn the player to face target n's chest (staging a scene).
            let which = a.get(1).ok_or("target look <n>")?;
            let e = *pick(world, which)?.first().ok_or("no target")?;
            let me = command_player(world)?;
            let eye = world.get::<Transform>(me).ok_or("the player has no place")?.translation + Vec3::Y * 0.6;
            let at = world.get::<Transform>(e).ok_or("the target has no place")?.translation + Vec3::Y * 0.3;
            let d = at - eye;
            let (yaw, pitch) = ((-d.x).atan2(-d.z), d.y.atan2(d.xz().length()));
            crate::core::set_view(world, me, yaw, pitch);
            Ok(Some(format!("looking at target {which}, {:.1} m away", d.length())))
        }
        "list" => Ok(Some(list(world))),
        "characters" | "bodies" => {
            let all = characters(world);
            Ok(Some(all.iter().map(|c| format!("{}  {}  ({})", c.id, c.name, c.game_name)).collect::<Vec<_>>().join("\n")))
        }
        _ => Err(USAGE.into()),
    }
}

/// The requests (messages), applied as world work.
fn apply_requests(
    mut spawns: MessageReader<SpawnTarget>,
    mut sets: MessageReader<SetTarget>,
    mut removes: MessageReader<RemoveTarget>,
    mut clears: MessageReader<ClearTargets>,
    mut commands: Commands,
) {
    fn refuse(world: &mut World, reason: String) {
        warn!("target: {reason}");
        world.write_message(TargetRefused { reason });
    }
    for r in spawns.read().cloned() {
        commands.queue(move |world: &mut World| {
            if let Err(e) = spawn(world, &r) {
                refuse(world, e);
            }
        });
    }
    for r in sets.read().cloned() {
        commands.queue(move |world: &mut World| {
            if let Err(e) = set(world, &r) {
                refuse(world, e);
            }
        });
    }
    for r in removes.read().copied() {
        commands.queue(move |world: &mut World| remove(world, r.0));
    }
    if clears.read().count() > 0 {
        commands.queue(|world: &mut World| {
            clear(world);
        });
    }
}

/// Targets to be put at home (new, re-armed, back from the dead), and
/// the dead marked with their return; all gone out of the sandbox.
#[allow(clippy::type_complexity)]
fn settle(
    pending: Query<Entity, (With<Target>, Without<Dead>, Or<(With<TargetPending>, With<TargetRespawn>)>)>,
    dead: Query<(Entity, &Target, &Dead), Without<TargetRespawn>>,
    drives: Query<&TargetDrive>,
    rungs: Query<Entity, (With<Target>, Changed<super::ladder::OnRung>)>,
    loads: Option<Res<MapLoads>>,
    mut commands: Commands,
) {
    let loads = loads.map_or(0, |m| m.0);
    if drives.iter().any(|d| d.map_loads != loads) {
        commands.queue(|world: &mut World| {
            clear(world);
        });
        return;
    }
    if !drives.is_empty() {
        commands.queue(|world: &mut World| {
            if !rung_command::holds(world) {
                clear(world);
            }
        });
    }
    for (e, t, d) in &dead {
        commands.entity(e).try_insert(TargetRespawn { at: d.since + t.respawn_secs as f64 });
    }
    for e in &pending {
        commands.queue(move |world: &mut World| {
            place(world, e);
        });
    }
    // A rung set again (the sandbox's hold putting it back): dressed again.
    for e in &rungs {
        commands.queue(move |world: &mut World| {
            if let Some(c) = world.get::<Target>(e).map(|t| t.character.clone()) {
                wear(world, e, &c);
            }
        });
    }
}

fn axis_toward(dir: Vec3, yaw: f32) -> Vec2 {
    let d = Vec3::new(dir.x, 0.0, dir.z).normalize_or_zero();
    let forward = Vec3::new(-yaw.sin(), 0.0, -yaw.cos());
    let right = Vec3::new(yaw.cos(), 0.0, -yaw.sin());
    let a = Vec2::new(d.dot(right), d.dot(forward));
    if a.length() > 1e-3 { a / a.x.abs().max(a.y.abs()) } else { Vec2::ZERO }
}

/// The walk's `k`th point around home (a fixed sequence).
fn waypoint(home: Vec3, number: u32, k: u32) -> Vec3 {
    let a = (k as f32) * 2.399_963 + number as f32;
    let r = WALK_RADIUS * (0.4 + 0.6 * ((k as f32) * 0.618_034).fract());
    home + Vec3::new(a.cos() * r, 0.0, a.sin() * r)
}

/// Each tick, after the bots' brains: the motion's keys, the trigger held
/// off when it may not fire.
fn drive(clock: Res<SimClock>, mut q: Query<(&Target, &mut TargetDrive, &Transform, &mut Intent), (Without<Dead>, Without<TargetPending>)>) {
    let now = clock.now;
    for (t, mut d, tf, mut intent) in &mut q {
        let feet = tf.translation - Vec3::Y * crate::map::SPAWN_LIFT;
        let off = (feet - t.home).xz().length();
        let side = Vec3::new(t.dir.z, 0.0, -t.dir.x);
        let goal = match t.motion {
            TargetMotion::Still => (off > 1.0).then_some(t.home),
            TargetMotion::Strafes if off > STRAFE_HALF + LEASH => Some(t.home),
            TargetMotion::Strafes => {
                let to = t.home + side * STRAFE_HALF * d.sign;
                // Turned at the end, or after `STRAFE_LEG` s (a wall in the way).
                if (feet - to).xz().length() < 0.3 || (feet - t.home).dot(side) * d.sign > STRAFE_HALF || now > d.until + STRAFE_LEG {
                    d.sign = -d.sign;
                    d.until = now;
                }
                Some(t.home + side * STRAFE_HALF * d.sign)
            }
            TargetMotion::Walks if off > WALK_RADIUS + LEASH => Some(t.home),
            TargetMotion::Walks => {
                let to = waypoint(t.home, t.number, d.waypoint);
                if (feet - to).xz().length() < 0.5 || now > d.until + 5.0 {
                    d.waypoint += 1;
                    d.until = now;
                }
                Some(waypoint(t.home, t.number, d.waypoint))
            }
        };
        let axis = goal.map_or(Vec2::ZERO, |g| axis_toward(g - feet, intent.yaw));
        if intent.move_axis != axis {
            intent.move_axis = axis;
        }
        if intent.jump || intent.crouch || intent.sprint {
            intent.jump = false;
            intent.crouch = false;
            intent.sprint = false;
        }
        if !t.shoots_back && (intent.fire || intent.secondary || intent.actions != 0) {
            intent.fire = false;
            intent.secondary = false;
            intent.actions = 0;
        }
    }
}

/// Health and second bar at the tick's start, before its hits.
#[derive(Resource, Default)]
struct Before(BTreeMap<Entity, (f32, f32)>, BTreeMap<Entity, &'static str>);

/// One attacker's run at one victim since the victim's last death.
#[derive(Clone, Copy, Debug, Default)]
struct Engagement {
    first_shot: Option<f64>,
    first_hit: Option<f64>,
    shots: u32,
    hits: u32,
    headshots: u32,
    damage: f32,
    last_shot_tick: Option<u64>,
}

#[derive(Resource, Default)]
struct Engagements(BTreeMap<(Entity, Entity), Engagement>);

/// Before the tick's weapons: protected targets back to full, and every
/// character's health and second bar noted.
fn before_hits(
    mut before: ResMut<Before>,
    mut q: Query<(Entity, &mut Health, Option<&mut Armor>, Option<&Target>, Option<&TargetVitals>), With<Intent>>,
    (held, guns): (Query<(Entity, &Inventory)>, Query<&Weapon>),
) {
    before.0.clear();
    // The guns in hand before the hits (a ladder's kill swaps the killer's).
    before.1.clear();
    for (e, inv) in &held {
        if let Some(w) = inv.active.and_then(|w| guns.get(w).ok()) {
            before.1.insert(e, w.id);
        }
    }
    for (e, mut h, armor, t, v) in &mut q {
        let mut armor = armor;
        // No second bar: whatever a game put back (Halo's recharge) goes
        // before the hits (Halo's step takes it in after them).
        if t.is_some_and(|t| !t.armored)
            && let Some(a) = armor.as_deref_mut()
            && a.amount != 0.0
        {
            a.amount = 0.0;
        }
        if t.is_some_and(|t| t.protected) && h.current > 0.0 {
            if h.current != h.max {
                h.current = h.max;
            }
            if let (Some(a), Some(v)) = (armor.as_deref_mut(), v)
                && a.amount < v.shield_max
            {
                a.amount = v.shield_max;
            }
        }
        before.0.insert(e, (h.current, armor.map_or(0.0, |a| a.amount)));
    }
}

/// What a tick's hits on one victim by one attacker add up to.
#[derive(Default)]
struct Tally {
    unscaled: f32,
    taken: f32,
    hitgroup: Option<Hitgroup>,
    weapon: Option<&'static str>,
    hits: u32,
}

/// After the tick's damage: the hits and kills measured, the dead
/// targets' returns set, the vitals kept.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn measure(
    (mut damage, mut taken, mut regions, mut died, mut weapon_events, mut blasts): (
        MessageReader<Damage>,
        MessageReader<DamageTaken>,
        MessageReader<RegionHit>,
        MessageReader<Died>,
        MessageReader<WeaponEvent>,
        MessageReader<BlastDealt>,
    ),
    (mut hits_out, mut kills_out, mut blasts_out): (
        MessageWriter<HitMeasured>,
        MessageWriter<KillMeasured>,
        MessageWriter<BlastMeasured>,
    ),
    sandbox: (Option<Res<rung_command::HoldRungs>>, Option<Res<super::ActiveMinigame>>),
    clock: Res<SimClock>,
    before: Res<Before>,
    mut engaged: ResMut<Engagements>,
    targets: Query<(), With<Target>>,
    chars: Query<(Entity, &Transform, &Health, Option<&Armor>, Has<Dead>), With<Intent>>,
    held: Query<&Inventory>,
    guns: Query<&Weapon>,
) {
    let now = clock.now;
    let pos = |e: Entity| chars.get(e).ok().map(|c| c.1.translation);
    let held_gun = |e: Entity| {
        before.1.get(&e).copied().or_else(|| held.get(e).ok().and_then(|i| i.active).and_then(|w| guns.get(w).ok()).map(|w| w.id))
    };
    let watched = |a: Option<Entity>, v: Entity| targets.contains(v) || a.is_some_and(|a| targets.contains(a));
    // Shots at victims.
    for ev in weapon_events.read() {
        let WeaponEventKind::ShotMade { origin, dir, .. } = ev.kind else { continue };
        if ev.replay {
            continue;
        }
        let a = ev.owner;
        for (v, tf, ..) in &chars {
            // Alive at the tick's start (a killing shot counts).
            let alive = before.0.get(&v).is_some_and(|(h, _)| *h > 0.0);
            if v == a || !alive || !watched(Some(a), v) {
                continue;
            }
            let to = tf.translation - origin;
            let along = to.dot(dir);
            if along <= 0.0 || (to - dir * along).length() > AIM_RADIUS {
                continue;
            }
            let g = engaged.0.entry((v, a)).or_default();
            if g.last_shot_tick != Some(clock.tick) {
                g.shots += 1;
                g.last_shot_tick = Some(clock.tick);
            }
            g.first_shot.get_or_insert(now);
        }
    }
    // The tick's blasts, measured while the sandbox plays.
    let (hold, active) = sandbox;
    let blasts: Vec<BlastDealt> = if rung_command::sandbox_plays(hold, active) {
        blasts.read().cloned().collect()
    } else {
        blasts.clear();
        Vec::new()
    };
    let in_blast = |a: Option<Entity>, v: Entity| blasts.iter().any(|b| b.attacker == a && b.victims.iter().any(|x| x.victim == v));
    // The tick's hits, per victim and attacker.
    let mut tally: BTreeMap<(Entity, Option<Entity>), Tally> = BTreeMap::new();
    for d in damage.read() {
        let blasted = in_blast(d.attacker, d.target);
        if !blasted && (!watched(d.attacker, d.target) || d.attacker == Some(d.target)) {
            continue;
        }
        let t = tally.entry((d.target, d.attacker)).or_default();
        t.unscaled += d.amount;
        t.hits += 1;
        t.weapon = t.weapon.or(d.weapon).or_else(|| d.attacker.and_then(held_gun));
        if t.hitgroup.is_none_or(|g| g != Hitgroup::Head) {
            t.hitgroup = Some(d.hitgroup);
        }
    }
    for d in taken.read() {
        if let Some(t) = tally.get_mut(&(d.damage.target, d.damage.attacker)) {
            t.taken += d.damage.amount;
        }
    }
    let mut region: BTreeMap<Entity, BodyRegion> = BTreeMap::new();
    for r in regions.read() {
        region.insert(r.target, r.region);
    }
    let mut shield_given: Vec<Entity> = Vec::new();
    // What each pair's hits came to (`HitMeasured::damage`), for blasts.
    let mut measured: BTreeMap<(Entity, Option<Entity>), f32> = BTreeMap::new();
    for ((v, a), t) in tally {
        let Ok((_, tf, h, armor, _)) = chars.get(v) else { continue };
        let (hb, sb) = before.0.get(&v).copied().unwrap_or((h.current, armor.map_or(0.0, |a| a.amount)));
        let sa = armor.map_or(0.0, |a| a.amount);
        let shield_lost = if shield_given.contains(&v) { 0.0 } else { (sb - sa).max(0.0) };
        shield_given.push(v);
        measured.insert((v, a), t.taken + shield_lost);
        if !watched(a, v) || a == Some(v) {
            continue;
        }
        let distance_m = a.and_then(pos).map_or(0.0, |p| p.distance(tf.translation));
        let hit = HitMeasured {
            attacker: a,
            victim: v,
            gun: t.weapon,
            damage: t.taken + shield_lost,
            damage_unscaled: t.unscaled + shield_lost,
            hitgroup: t.hitgroup.unwrap_or_default(),
            region: region.get(&v).copied(),
            distance_m,
            health_before: hb,
            health_after: h.current,
            shield_before: sb,
            shield_after: sa,
            hits: t.hits,
            time: now,
        };
        info!(
            "HitMeasured attacker={:?} victim={v:?} gun={} damage={:.1} unscaled={:.1} group={:?} region={:?} distance={:.1}m \
             health {:.1}->{:.1} shield {:.1}->{:.1} hits={} t={now:.3}",
            a,
            hit.gun.unwrap_or("?"),
            hit.damage * 100.0,
            hit.damage_unscaled * 100.0,
            hit.hitgroup,
            hit.region,
            distance_m,
            hb * 100.0,
            h.current * 100.0,
            sb * 100.0,
            sa * 100.0,
            t.hits
        );
        if let Some(a) = a {
            let g = engaged.0.entry((v, a)).or_default();
            g.first_hit.get_or_insert(now);
            g.first_shot.get_or_insert(now);
            g.hits += 1;
            g.headshots += u32::from(hit.hitgroup == Hitgroup::Head);
            g.damage += hit.damage;
        }
        hits_out.write(hit);
    }
    let deaths: Vec<Died> = died.read().cloned().collect();
    // Blasts: each victim's damage as its `HitMeasured` (the blast's own
    // where nothing measured it: a thing with no health).
    for b in blasts {
        let hits: Vec<BlastHit> = b
            .victims
            .iter()
            .map(|x| BlastHit {
                victim: x.victim,
                at: x.at,
                distance_m: x.distance_m,
                damage: measured.get(&(x.victim, b.attacker)).copied().unwrap_or(x.damage),
                killed: deaths.iter().any(|d| d.entity == x.victim),
            })
            .collect();
        let m = BlastMeasured {
            attacker: b.attacker,
            gun: b.weapon,
            at: b.at,
            inner: b.inner,
            outer: b.outer,
            max_damage: b.max_damage,
            edge_damage: b.edge_damage,
            hits,
            time: now,
        };
        info!("{}", blast_line(&m));
        blasts_out.write(m);
    }
    // Kills.
    for d in &deaths {
        let v = d.entity;
        if !watched(d.attacker, v) {
            continue;
        }
        let g = d.attacker.and_then(|a| engaged.0.get(&(v, a)).copied()).unwrap_or_default();
        let distance_m = d.attacker.and_then(pos).zip(pos(v)).map_or(0.0, |(a, b)| a.distance(b));
        let kill = KillMeasured {
            attacker: d.attacker,
            victim: v,
            gun: d.damage.weapon.or_else(|| d.attacker.and_then(held_gun)),
            shots_fired: g.shots.max(g.hits),
            shots_hit: g.hits,
            headshots: g.headshots,
            from_first_hit: g.first_hit.map_or(0.0, |t| (now - t) as f32),
            from_first_shot: g.first_shot.map_or(0.0, |t| (now - t) as f32),
            distance_m,
            total_damage: g.damage,
            time: now,
        };
        info!(
            "KillMeasured attacker={:?} victim={v:?} gun={} shots={} hits={} headshots={} first_hit->kill={:.2}s \
             first_shot->kill={:.2}s distance={:.1}m damage={:.1} t={now:.3}",
            kill.attacker,
            kill.gun.unwrap_or("?"),
            kill.shots_fired,
            kill.shots_hit,
            kill.headshots,
            kill.from_first_hit,
            kill.from_first_shot,
            distance_m,
            kill.total_damage * 100.0
        );
        kills_out.write(kill);
        engaged.0.retain(|(victim, _), _| *victim != v);
    }
    // Gone characters' runs.
    engaged.0.retain(|(v, a), _| chars.contains(*v) && chars.contains(*a));
}

/// A `BlastMeasured` as the log says it (damage ×100, as `HitMeasured`'s).
pub fn blast_line(m: &BlastMeasured) -> String {
    let hits: Vec<String> = m
        .hits
        .iter()
        .map(|h| format!("{:?} {:.1}m {:.1}{}", h.victim, h.distance_m, h.damage * 100.0, if h.killed { " killed" } else { "" }))
        .collect();
    format!(
        "BlastMeasured attacker={:?} gun={} at=({:.2},{:.2},{:.2}) inner={:.2}m outer={:.2}m max={:.1} edge={:.1} hits=[{}] t={:.3}",
        m.attacker,
        m.gun.unwrap_or("?"),
        m.at.x,
        m.at.y,
        m.at.z,
        m.inner,
        m.outer,
        m.max_damage * 100.0,
        m.edge_damage * 100.0,
        hits.join(", "),
        m.time
    )
}

/// The vitals after the whole tick (a game's own step, Halo's shield's,
/// runs after the damage).
fn keep_vitals(mut vitals: Query<(&mut TargetVitals, &Target, &Health, Option<&Armor>)>) {
    for (mut vt, t, h, armor) in &mut vitals {
        let shield = if t.armored { armor.map_or(0.0, |a| a.amount) } else { 0.0 };
        let next = TargetVitals { health: h.current, health_max: h.max, shield, shield_max: vt.shield_max.max(shield) };
        if *vt != next {
            *vt = next;
        }
    }
}

pub(super) fn plugin(app: &mut App) {
    app.add_message::<SpawnTarget>()
        .add_message::<SetTarget>()
        .add_message::<RemoveTarget>()
        .add_message::<ClearTargets>()
        .add_message::<TargetRefused>()
        .add_message::<HitMeasured>()
        .add_message::<KillMeasured>()
        .add_message::<BlastMeasured>()
        .add_message::<BlastDealt>()
        .add_message::<Damage>()
        .add_message::<DamageTaken>()
        .add_message::<RegionHit>()
        .add_message::<Died>()
        .add_message::<WeaponEvent>()
        .init_resource::<Before>()
        .init_resource::<Engagements>()
        .init_resource::<TargetCharacters>()
        .add_systems(Update, apply_requests)
        .add_systems(
            FixedUpdate,
            (
                settle.after(SimSet::Rules).before(SimSet::Commands),
                drive.after(settle).after(SimSet::Rules).before(SimSet::Commands),
            )
                .run_if(crate::core::authoritative)
                .run_if(any_with_component::<Target>),
        )
        // Blasts are measured with no target too, while the sandbox plays.
        .add_systems(
            FixedUpdate,
            (
                before_hits.after(SimSet::Movement).before(SimSet::Weapons),
                measure.after(crate::core::apply_damage),
            )
                .run_if(crate::core::authoritative)
                .run_if(any_with_component::<Target>.or_else(rung_command::sandbox_plays)),
        )
        .add_systems(FixedPostUpdate, keep_vitals.run_if(crate::core::authoritative).run_if(any_with_component::<Target>))
        .console_command("chimera:target", &format!("Practice targets in the sandbox: {USAGE}"), command);
}
