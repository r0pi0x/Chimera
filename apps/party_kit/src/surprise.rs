//! The host's surprises (docs/playtest-hosting.md, "Surprises"): two
//! console commands a party server's host types to shake a match up,
//! built only from what every client of the party already understands
//! (replicated objects, the map logic's HUD messages and screen fades,
//! the rules' sounds, replicated cvars): no message, part or client code
//! of their own, so clients of an older build see them whole.
//!
//! - `surprise airstrike [count=30] [seconds=8]`: a warning for
//!   `WARNING` s (a red HUD message centre screen, a red flash and CS:S's
//!   radio call "Get out of there, it's gonna blow!", `ALARM`), then
//!   `count` shells of
//!   Combat Arms' air strikes (`LGB_SHELL`, `Napalm_Shell`: the designators'
//!   own `strike.shell`, dropped by `combat_arms::game::special::devices::
//!   drop_strike_shell`) land over `seconds` s, each on a floor point of
//!   the map open to the sky: half of them near a living player, the rest
//!   anywhere on the walkable area (the navigation mesh's areas, else
//!   about the spawn points; a point under a roof is tried again, and kept
//!   only when no open one turns up). A shell falls straight down from
//!   under the sky, drawn as the RPG-7's rocket (`BLAST_LOOK`: model,
//!   smoke tail, fly sound), and blows up with the shell's data: its blast
//!   (the LGB's 110 in 1200 u) or burning sphere (the napalm's 80 HP/s),
//!   its explosion drawn and heard as the rocket's. The
//!   deaths credit nobody, and with the shells' weapons in
//!   `chimera_sim::minigame::runtime::BlamelessWeapons` no rule hears of
//!   them: no rung up or down for anyone.
//! - `surprise gravity [seconds=60] [scale=0.2]`: moon gravity for
//!   `seconds` s: every movement's gravity times `scale` (each Source
//!   movement's `sv_gravity` cvar, CS:S's and HL2:DM's, which a client
//!   gets replicated and predicts its own player with; Combat Arms'
//!   `MovementData::gravity`, the server's alone), and the physics world's
//!   (`avian3d`'s `Gravity`: the props and ragdolls the server simulates),
//!   with a banner ("Moon gravity!"), a countdown in the last
//!   `COUNTDOWN` s and an ending message; then all of it back as it was.
//!   A map change ends it at once (the map sets its own gravity).
//! - `surprise stop`: ends both now (gravity back, shells not yet dropped
//!   never come).
//!
//! The commands run on the authority (a dedicated server's console, a
//! listen server's host); a network client is refused.
//!
//! Known shortcuts: the shells look and blow up as the RPG-7's rocket, not
//! as their own records (`LGB_Shell`'s missile, `Dirt_Burster`,
//! `Exp_Napalm_Shell`: no loaded weapon's, so a client can't be told to
//! draw them), and the napalm's
//! burning sphere has no look of its own (only its damage and the
//! victims' burn sounds); the alarm is a radio call, not a siren; a Combat Arms player's client predicts its movement
//! with its own gravity (Combat Arms' movement has no replicated
//! setting), so under moon gravity the server corrects it each snapshot
//! (an error of a few centimetres at a playtest's latency); clients draw
//! the ragdolls of the dead themselves with the map's gravity, so only a
//! listen server's own ragdolls float; Combat Arms' grenades and
//! projectiles keep their object gravity (a constant of their spec).

use avian3d::prelude::Gravity;
use bevy::{ecs::message::Messages, prelude::*};
use chimera_core::{
    console::{Console, ConsoleAppExt},
    core::{Dead, Health, Intent, NetRole, SimClock, SpawnPoint, authoritative},
};
use chimera_sim::minigame::runtime::BlamelessWeapons;
use chimera_world::map::{LoadedMapName, nav::NavMesh};
use chimera_world::map::{GameSound, PlaySound};
use combat_arms::game::{
    movement::MovementData,
    special::devices::{drop_strike_shell, strike_floor, strike_start},
};
use source_logic::logic::{HudEvent, HudMessage, HudShow, ScreenFade};

/// The designators whose shells fall (special_weapons.md 6.10), and how
/// many of every `LGB_WEIGHT + NAPALM_WEIGHT` shells are each's.
pub const LGB: &str = "combat_arms:lgb";
pub const NAPALM: &str = "combat_arms:airstrike_napalm";
pub const LGB_WEIGHT: u32 = 2;
pub const NAPALM_WEIGHT: u32 = 1;

/// The warning before the first shell, s.
pub const WARNING: f64 = 3.0;
/// The default shell count and how long they fall, s.
pub const SHELLS: usize = 30;
pub const STRIKE_SECONDS: f64 = 8.0;
/// Shells near a living player, of every two.
pub const NEAR_PLAYER: f32 = 0.5;
/// How far from that player one lands at most, m.
pub const NEAR_RADIUS: f32 = 12.0;
/// Tries for a floor point open to the sky.
pub const TRIES: usize = 12;

/// The alarm: CS:S's radio call "Get out of there, it's gonna blow!",
/// played to everyone unspatialized. A sound the server sends plays only
/// when the client's map has its entry ready (`map::SoundBank`); this one
/// every party map has (CS:S's radio entries go into every map, CS:S's or
/// as the guest's: `cs_source::game::sound::load`), its waves decoded with
/// the map, so it never waits on a file. No game's siren is among the
/// entries every map holds.
pub const ALARM: &str = "Radio.GetOutOfThere";

/// The weapon a shell is drawn in flight and blows up as: our designators'
/// shells have no model (their `LGB_Shell` record's isn't any weapon's
/// projectile, and a client draws only the projectile models its weapon
/// set names, loaded with the map), and their explosion records
/// (`Dirt_Burster`, `Exp_Napalm_Shell`) are no loaded weapon's (a client
/// draws a detonation only by a weapon it knows: `chimera_net`'s
/// `Detonation`). So a shell is the RPG-7's rocket, nose down with its
/// smoke tail and fly sound, and blows up as it does (its `Rocket`
/// record: the blast group and sound).
pub const BLAST_LOOK: &str = combat_arms::game::special::devices::STRIKE_LOOK;

/// The moon's defaults: how long, s, and the gravity's scale.
pub const GRAVITY_SECONDS: f64 = 60.0;
pub const GRAVITY_SCALE: f32 = 0.2;
/// How long the opening banner holds, s.
const BANNER: f64 = 3.0;
/// The last seconds of moon gravity are counted down (not over the
/// opening banner).
pub const COUNTDOWN: u32 = 5;

/// HUD message channels (map logic's game_text has six, 0-5; a map's
/// own usually take the first).
const STRIKE_CHANNEL: usize = 4;
const GRAVITY_CHANNEL: usize = 5;

/// The console commands and the surprises' steps.
pub struct SurprisePlugin;

impl Plugin for SurprisePlugin {
    fn build(&self, app: &mut App) {
        // Nothing in the world until a surprise is asked for (`arm`): with
        // a resource more at the start, the party's bots played otherwise
        // in apps/party_kit/tests/it/archetypes.rs (its tick measured 1.37x
        // with more archetypes, against 1.19x).
        app.console_command(
                "chimera:surprise",
                "surprise airstrike [count] [seconds] | surprise gravity [seconds] [scale] | surprise stop: \
                 the host's surprises (a warning, then Combat Arms air strike shells over the map; moon gravity).",
                command,
            );
    }

    /// The steps, at the end of each tick (after its simulation), run only
    /// while a surprise is under way.
    fn finish(&self, app: &mut App) {
        app.add_systems(
            FixedLast,
            (
                air_strike_step.run_if(|s: Option<Res<AirStrike>>| s.is_some_and(|s| !s.drops.is_empty())),
                gravity_step.run_if(|m: Option<Res<MoonGravity>>| m.is_some_and(|m| m.until.is_some())),
            )
                .run_if(authoritative),
        );
    }
}

/// Ready the surprises' state in `world` (once): the strike's and the
/// moon's resources, and the shells' designators in `BlamelessWeapons`
/// (their deaths with no killer are the world's: no rung down; a
/// designator's whose thrower left too).
pub fn arm(world: &mut World) {
    world.get_resource_or_init::<AirStrike>();
    world.get_resource_or_init::<MoonGravity>();
    let mut blameless = world.get_resource_or_init::<BlamelessWeapons>();
    for w in [LGB, NAPALM] {
        if !blameless.0.contains(&w) {
            blameless.0.push(w);
        }
    }
}

/// `surprise <what> [args]`.
fn command(world: &mut World, args: &[String]) -> Result<Option<String>, String> {
    if world.get_resource::<NetRole>() == Some(&NetRole::Client) {
        return Err("surprises are the server's: type it on the server's console".into());
    }
    arm(world);
    let num = |i: usize| -> Result<Option<f64>, String> {
        args.get(i).map(|s| s.parse::<f64>().map_err(|_| format!("not a number: {s}"))).transpose()
    };
    match args.first().map(String::as_str) {
        Some("airstrike") => {
            let count = num(1)?.map_or(SHELLS, |n| n.max(1.0) as usize).min(200);
            let seconds = num(2)?.unwrap_or(STRIKE_SECONDS).max(0.0);
            start_air_strike(world, count, seconds).map(Some)
        }
        Some("gravity") => {
            let seconds = num(1)?.unwrap_or(GRAVITY_SECONDS).max(1.0);
            let scale = num(2)?.map_or(GRAVITY_SCALE, |s| s as f32).clamp(0.01, 10.0);
            start_gravity(world, seconds, scale).map(Some)
        }
        Some("stop") => {
            world.resource_mut::<AirStrike>().drops.clear();
            end_gravity(world, false);
            Ok(Some("surprises stopped".into()))
        }
        _ => Err("usage: surprise airstrike [count] [seconds] | surprise gravity [seconds] [scale] | surprise stop".into()),
    }
}

// ---------------------------------------------------------------------------
// The air strike

/// The strike under way: when each shell drops (server time, s) and of
/// which designator.
#[derive(Resource, Clone, Debug, Default)]
pub struct AirStrike {
    pub drops: Vec<(f64, &'static str)>,
    /// Shells dropped since the command.
    pub dropped: usize,
    /// Where they fell (metres), for the log and the tests.
    pub fallen: Vec<Vec3>,
    pub rng: u64,
}

/// A small random generator (xorshift*), seeded from the clock.
fn next(rng: &mut u64) -> f32 {
    let mut x = *rng | 1;
    x ^= x >> 12;
    x ^= x << 25;
    x ^= x >> 27;
    *rng = x;
    (x.wrapping_mul(0x2545_F491_4F6C_DD1D) >> 40) as f32 / (1u64 << 24) as f32
}

fn start_air_strike(world: &mut World, count: usize, seconds: f64) -> Result<String, String> {
    let catalog = world
        .get_resource::<chimera_sim::weapon::data::WeaponCatalog>()
        .ok_or("no weapons loaded")?
        .clone();
    let lgb: &'static str = catalog.get(LGB).map(|d| d.id.as_str()).ok_or("no LGB designator (Combat Arms' guns)")?;
    let napalm: &'static str =
        catalog.get(NAPALM).map(|d| d.id.as_str()).ok_or("no napalm designator (Combat Arms' guns)")?;
    let clock = *world.resource::<SimClock>();
    let mut s = world.resource_mut::<AirStrike>();
    s.rng = s.rng.wrapping_add(clock.tick.wrapping_mul(0x9E37_79B9_7F4A_7C15)) | 1;
    let begin = clock.now + WARNING;
    let mut drops = Vec::with_capacity(count);
    for i in 0..count {
        let even = if count > 1 { seconds * i as f64 / (count - 1) as f64 } else { 0.0 };
        let jitter = (next(&mut s.rng) as f64 - 0.5) * seconds / count as f64;
        let at = begin + (even + jitter).clamp(0.0, seconds);
        let pick = (next(&mut s.rng) * (LGB_WEIGHT + NAPALM_WEIGHT) as f32) as u32;
        drops.push((at, if pick < LGB_WEIGHT { lgb } else { napalm }));
    }
    drops.sort_by(|a, b| a.0.total_cmp(&b.0));
    s.drops = drops;
    s.dropped = 0;
    s.fallen.clear();
    warn_air_strike(world);
    let text = format!("surprise: air strike, {count} shells over {seconds:.0} s after a {WARNING:.0} s warning");
    info!("{text}");
    Ok(text)
}

/// The warning: the banner, a red flash and the alarm at every living
/// player.
fn warn_air_strike(world: &mut World) {
    hud(world, banner("AIR STRIKE INCOMING!\nTAKE COVER!", STRIKE_CHANNEL, [255, 48, 32], WARNING as f32 + 2.0));
    flash(world, [255, 0, 0], 90, 0.8);
    if world.contains_resource::<Messages<GameSound>>() {
        world.write_message(GameSound(PlaySound::ui(ALARM)));
    }
}

/// The shells due drop (queued: each finds its point in the world).
fn air_strike_step(clock: Res<SimClock>, mut strike: ResMut<AirStrike>, mut commands: Commands) {
    let due = strike.drops.iter().take_while(|(t, _)| *t <= clock.now).count();
    for (_, designator) in strike.drops.drain(..due) {
        commands.queue(move |world: &mut World| {
            if let Some(ground) = target(world, designator) {
                drop_strike_shell(world, designator, ground, Some(BLAST_LOOK));
                let mut s = world.resource_mut::<AirStrike>();
                s.dropped += 1;
                s.fallen.push(ground);
            }
        });
    }
}

/// A floor point for the next shell: near a living player (half of them)
/// or anywhere walkable, open to the sky if one turns up in `TRIES`.
fn target(world: &mut World, designator: &str) -> Option<Vec3> {
    let players = living(world);
    let mut covered = None;
    for _ in 0..TRIES {
        let (near, r1, r2, r3) = {
            let mut s = world.resource_mut::<AirStrike>();
            (next(&mut s.rng), next(&mut s.rng), next(&mut s.rng), next(&mut s.rng))
        };
        let candidate = if near < NEAR_PLAYER && !players.is_empty() {
            let (_, at) = players[((r1 * players.len() as f32) as usize).min(players.len() - 1)];
            let (angle, dist) = (r2 * std::f32::consts::TAU, NEAR_RADIUS * r3.sqrt());
            let above = at + Vec3::new(angle.cos() * dist, 1.5, angle.sin() * dist);
            strike_floor(world, above, 40.0)
        } else {
            walkable(world, r1, r2, r3)
        };
        let Some(ground) = candidate else { continue };
        match strike_start(world, designator, ground) {
            Some((_, true)) => return Some(ground),
            Some((_, false)) => covered = covered.or(Some(ground)),
            None => return None,
        }
    }
    covered
}

/// A random point of the walkable area: on a navigation area (by its
/// size), else near a spawn point.
fn walkable(world: &mut World, r1: f32, r2: f32, r3: f32) -> Option<Vec3> {
    if let Some(nav) = world.get_resource::<NavMesh>().filter(|n| !n.areas.is_empty()) {
        let size = |a: &chimera_world::map::nav::NavArea| ((a.max.x - a.min.x) * (a.max.y - a.min.y)).max(0.0);
        let total: f32 = nav.areas.iter().map(size).sum();
        let mut pick = r1 * total;
        for a in &nav.areas {
            pick -= size(a);
            if pick <= 0.0 {
                let x = a.min.x + (a.max.x - a.min.x) * r2;
                let z = a.min.y + (a.max.y - a.min.y) * r3;
                let y = a.height_at(x, z);
                return strike_floor(world, Vec3::new(x, y + 1.0, z), 4.0).or(Some(Vec3::new(x, y, z)));
            }
        }
    }
    let spawns: Vec<Vec3> = world
        .query_filtered::<&GlobalTransform, With<SpawnPoint>>()
        .iter(world)
        .map(|t| t.translation())
        .collect();
    if spawns.is_empty() {
        return None;
    }
    let at = spawns[((r1 * spawns.len() as f32) as usize).min(spawns.len() - 1)];
    let (angle, dist) = (r2 * std::f32::consts::TAU, NEAR_RADIUS * r3.sqrt());
    strike_floor(world, at + Vec3::new(angle.cos() * dist, 1.5, angle.sin() * dist), 40.0)
}

/// Every living player (not a bystander NPC): its entity and position.
fn living(world: &mut World) -> Vec<(Entity, Vec3)> {
    world
        .query_filtered::<(Entity, &Transform, &Health), (With<Intent>, Without<Dead>, Without<chimera_core::core::Npc>)>()
        .iter(world)
        .filter(|(_, _, h)| h.current > 0.0)
        .map(|(e, t, _)| (e, t.translation))
        .collect()
}

// ---------------------------------------------------------------------------
// Moon gravity

/// Moon gravity under way: until when (server time, s), its scale, what
/// it changed (to put back) and the map it began on.
#[derive(Resource, Clone, Debug, Default)]
pub struct MoonGravity {
    pub until: Option<f64>,
    /// When it began (server time, s).
    pub since: f64,
    pub scale: f32,
    /// Each Source movement's `sv_gravity` cvar (full name) and its value
    /// before.
    pub cvars: Vec<(String, String)>,
    /// Combat Arms' movement gravity before (u/s²).
    pub movement: Option<f32>,
    /// The physics world's before.
    pub physics: Option<Vec3>,
    pub map: Option<String>,
    /// The last whole second counted down.
    pub counted: Option<u32>,
}

/// Every cvar named `sv_gravity` in any namespace (CS:S's, HL2:DM's).
fn gravity_cvars(world: &World) -> Vec<String> {
    world
        .resource::<Console>()
        .cvars()
        .filter(|c| chimera_core::console::short_name(&c.name) == "sv_gravity")
        .map(|c| c.name.clone())
        .collect()
}

fn get_cvar(world: &mut World, name: &str) -> Option<String> {
    let get = world.resource::<Console>().cvar(name)?.get.clone();
    get(world)
}

fn set_cvar(world: &mut World, name: &str, value: &str) {
    let Some(set) = world.resource::<Console>().cvar(name).map(|c| c.set.clone()) else { return };
    if let Err(e) = set(world, value) {
        warn!("surprise: {name} {value}: {e}");
    }
}

fn start_gravity(world: &mut World, seconds: f64, scale: f32) -> Result<String, String> {
    // Again while on: from the gravity before it, not the moon's.
    end_gravity(world, false);
    let mut saved = Vec::new();
    for name in gravity_cvars(world) {
        let Some(old) = get_cvar(world, &name) else { continue };
        let Ok(v) = old.parse::<f32>() else { continue };
        set_cvar(world, &name, &format!("{}", v * scale));
        saved.push((name, old));
    }
    let movement = world.get_resource_mut::<MovementData>().map(|mut m| {
        let old = m.gravity;
        m.gravity = old * scale;
        old
    });
    let physics = world.get_resource_mut::<Gravity>().map(|mut g| {
        let old = g.0;
        g.0 = old * scale;
        old
    });
    let now = world.resource::<SimClock>().now;
    let map = world.get_resource::<LoadedMapName>().map(|m| m.0.clone());
    *world.resource_mut::<MoonGravity>() = MoonGravity {
        until: Some(now + seconds),
        since: now,
        scale,
        cvars: saved,
        movement,
        physics,
        map,
        counted: None,
    };
    let text = if seconds >= 1.5 {
        format!("Moon gravity!\nfor {seconds:.0} seconds")
    } else {
        "Moon gravity!".to_string()
    };
    hud(world, banner(&text, GRAVITY_CHANNEL, [140, 200, 255], BANNER as f32));
    flash(world, [120, 160, 255], 60, 0.6);
    let text = format!("surprise: moon gravity x{scale} for {seconds:.0} s");
    info!("{text}");
    Ok(text)
}

/// Put back what moon gravity changed; `say` it ended.
fn end_gravity(world: &mut World, say: bool) {
    let m = std::mem::take(&mut *world.resource_mut::<MoonGravity>());
    if m.until.is_none() {
        return;
    }
    for (name, old) in &m.cvars {
        set_cvar(world, name, old);
    }
    if let (Some(old), Some(mut d)) = (m.movement, world.get_resource_mut::<MovementData>()) {
        d.gravity = old;
    }
    // A map change set the map's own physics gravity already.
    let same_map = world.get_resource::<LoadedMapName>().map(|n| n.0.clone()) == m.map;
    if let (Some(old), true) = (m.physics, same_map)
        && let Some(mut g) = world.get_resource_mut::<Gravity>()
    {
        g.0 = old;
    }
    if say {
        hud(world, banner("Gravity is back!", GRAVITY_CHANNEL, [140, 200, 255], 2.5));
    }
    info!("surprise: gravity back");
}

/// Moon gravity's countdown and end (and its end at a map change).
fn gravity_step(clock: Res<SimClock>, map: Option<Res<LoadedMapName>>, mut moon: ResMut<MoonGravity>, mut commands: Commands) {
    let Some(until) = moon.until else { return };
    if map.map(|m| m.0.clone()) != moon.map {
        commands.queue(|world: &mut World| end_gravity(world, false));
        return;
    }
    let now = clock.now;
    if now >= until {
        // Once: the queued end clears `until`.
        moon.until = Some(f64::INFINITY);
        commands.queue(|world: &mut World| end_gravity(world, true));
        return;
    }
    let left = (until - now).ceil() as u32;
    // Not over the opening banner (a short moon's).
    let banner_up = now < moon.since + BANNER;
    if left <= COUNTDOWN && !banner_up && moon.counted != Some(left) {
        moon.counted = Some(left);
        commands.queue(move |world: &mut World| {
            hud(world, banner(&format!("Gravity returns in {left}"), GRAVITY_CHANNEL, [140, 200, 255], 1.2));
        });
    }
}

// ---------------------------------------------------------------------------
// What every client shows

/// A HUD message centre screen, a third of the way down, held `hold` s.
fn banner(text: &str, channel: usize, color: [u8; 3], hold: f32) -> HudMessage {
    HudMessage {
        text: text.into(),
        x: -1.0,
        y: 0.3,
        channel,
        effect: 0,
        color,
        color2: color,
        fade_in: 0.1,
        fade_out: 0.5,
        hold,
        fx_time: 0.0,
    }
}

/// Show `m` on every player's HUD (map logic's game_text, which every
/// client of the party draws).
fn hud(world: &mut World, m: HudMessage) {
    if world.contains_resource::<Messages<HudEvent>>() {
        world.write_message(HudEvent {
            to: None,
            what: HudShow::Text(m),
        });
    }
}

/// A screen flash of `color` at `alpha` fading out over `seconds`
/// (map logic's env_fade).
fn flash(world: &mut World, color: [u8; 3], alpha: u8, seconds: f32) {
    if world.contains_resource::<Messages<HudEvent>>() {
        world.write_message(HudEvent {
            to: None,
            what: HudShow::Fade(ScreenFade {
                duration: ScreenFade::quantize(seconds),
                hold: 0.0,
                color,
                alpha,
                fade_in: true,
                modulate: false,
                stay_out: false,
                purge: false,
            }),
        });
    }
}
