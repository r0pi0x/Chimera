//! Recorded input played again through our movement and weapons, tick by
//! tick, and compared with what the recording says happened: movecmp at
//! scale with human play (`demosim`, docs/OBSERVABILITY.md "Re-simulating
//! a client demo").
//!
//! A [`Recording`] is one player's commands (buttons, move amounts, view
//! angles, weapon picks, command numbers) and the states the game
//! recorded for that player (feet, velocity, on ground, ducked, weapon,
//! clip, reserve, shots fired, accuracy penalty), each with a key that
//! orders it: from a client (POV) demo (`from_demo`: its `dem_usercmd`
//! frames, the recording player's own-client props keyed by `m_nTickBase`,
//! its `CTEFireBullets`), or from the CS:S probe's logs (`from_probe`: the
//! `mashup_run`/`mashup_wrun` input and output files movecmp uses, which
//! hold the real game's states per tick, its command numbers and seeds).
//!
//! [`resimulate`] places a character where the first recorded state has
//! the player (velocity, ground, duck, weapons, clip and reserve as
//! recorded; the weapon's accuracy penalty, shots fired, timers and
//! alternate state, its silencer on or burst mode, as far as recorded),
//! runs each later command for one tick (the command number
//! stamped, so spread and recoil take the command's seed), and compares
//! our state after each command with the state recorded for it: the one
//! whose key is the command's plus `offset` (a demo's tick base runs a
//! fixed amount ahead of its command numbers; [`align`] finds it by trying
//! the nearby ones on the first commands). The [`Report`] says where the
//! first difference beyond the tolerances is, and how large and how
//! frequent the differences are; shots are paired in order and compared
//! by tick, seed, accuracy and how far off the aim they went.
//!
//! The alternate state (`Observed::alt_mode`: the probe's `special`, a
//! demo's `m_bSilencerOn`/`m_bBurstMode`; our `AltModes` on weapons that
//! don't zoom) is compared per tick ("alt mode"). A toggle in the
//! recording is replayed as the command's attack2 (a probe bot's own
//! attack2 is in the buttons its movement ran with); one that shows
//! without attack2 in its command came from outside the commands (a
//! weapon drawn, a bot's AI) and is set on ours as recorded
//! (`Report::alt_mode_applied`), not counted as a divergence. Likewise a
//! weapon switch no command picked (a bot's AI, or the switch to the knife
//! when the probe's USP runs dry with no reserve) is made on ours as
//! recorded (`Report::weapon_applied`).
//!
//! The server's console variables the demo carries (`net_SetConVar`:
//! `sv_maxspeed`, `sv_accelerate`, ...) are applied to the movement first
//! (`Recording::cvars`), and the tick is the recording's.
//!
//! Known shortcuts: our spread generator is not CS:S's (weapons.md Q1), so
//! shots compare by seed and accuracy and by statistics of their offsets,
//! not pellet by pellet; `upmove` (swimming up, ladders by key) is not an
//! input of ours (`Intent` has none), only jump; the world is the map as
//! loaded (doors, movers and physics props where the map puts them, other
//! players not there to collide with); a weapon picked by a command
//! (`weaponselect`) is the one of that class carried; the server's
//! console variables are applied once, before the first command.

use std::{collections::HashMap, f32::consts::FRAC_PI_2, fmt};

use bevy::{ecs::message::Messages, prelude::*};
use source_formats::dem::usercmd::buttons;

use super::{
    demo::{CsDemo, FL_DUCKING, FL_ONGROUND, Happening, Thing, weapon_name},
    movement::{self, SourceMovement, SourceMovementConfig, to_engine, to_source},
    weapons::Inaccuracy,
};
use crate::{
    core::{Seed, SimClock, SimTick, Velocity},
    weapon::{AltModes, Inventory, Magazine, NetPart, Weapon, WeaponEvent, WeaponEventKind, WeaponState, Zoom, give},
};
use chimera_world::harness::Sim;

/// Our character's origin is this far above the feet (the standing box's
/// centre).
const ORIGIN_ABOVE_FEET: f32 = 36.0;
/// What a full move key sends in CS:S (`cl_forwardspeed`).
const KEY_SPEED: f32 = 400.0;

/// One command of the recording player.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Command {
    /// Orders commands; a state with key `key + offset` follows it.
    pub key: i64,
    /// When it was recorded (a demo's tick; the probe's: its place).
    pub tick: i32,
    /// The command number (seeds spread and recoil).
    pub number: u32,
    pub buttons: u32,
    pub forward: f32,
    pub side: f32,
    pub up: f32,
    /// View angles, Source's degrees.
    pub pitch: f32,
    pub yaw: f32,
    /// A weapon to switch to (`weapon_ak47`).
    pub weapon: Option<String>,
}

/// What the game recorded of the player at one point.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Observed {
    pub key: i64,
    /// When it was recorded (a demo's tick; the probe's: its row).
    pub tick: i32,
    /// Feet, Source units.
    pub origin: [f32; 3],
    pub velocity: Option<[f32; 3]>,
    pub on_ground: bool,
    pub ducked: bool,
    /// The weapon in hand (`weapon_ak47`).
    pub weapon: Option<String>,
    pub clip: Option<i32>,
    pub reserve: Option<i32>,
    pub shots_fired: Option<i32>,
    /// The accuracy penalty (the probe's `penalty`).
    pub inaccuracy: Option<f32>,
    /// The weapon's alternate state: its silencer on or burst mode (our
    /// `AltModes` mode 1; the probe's `special`, a demo's `m_bSilencerOn`
    /// or `m_bBurstMode`). Not compared on weapons that zoom instead.
    pub alt_mode: Option<u8>,
    /// Seconds from this state until the weapon may fire again, its
    /// secondary attack, and the player may attack at all (the probe's
    /// `next_primary`, `next_secondary`, `next_attack` less its tick
    /// base's time), for the start.
    pub waits: Option<[f32; 3]>,
    /// Other weapons carried (`weapon_knife`), for the start.
    pub carried: Vec<String>,
}

/// A shot the game recorded for the player.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct RecordedShot {
    /// The key of the command that fired it, or (`by_state`) of the
    /// state it was recorded with (a demo's: the command's key plus the
    /// offset).
    pub key: i64,
    pub by_state: bool,
    /// The shot's seed (`& 255`), when recorded.
    pub seed: Option<u32>,
    pub inaccuracy: Option<f32>,
    pub spread: Option<f32>,
    /// The eye and view angles it left from, and where its bullet hit
    /// (Source units, degrees), when recorded.
    pub eye: Option<[f32; 3]>,
    pub angles: Option<[f32; 2]>,
    pub impact: Option<[f32; 3]>,
}

/// One player's recorded input and states.
#[derive(Clone, Debug, Default)]
pub struct Recording {
    /// Where it came from (for the report).
    pub name: String,
    pub map: String,
    pub tick_interval: f32,
    /// The server's console variables, in the order sent.
    pub cvars: Vec<(String, String)>,
    pub commands: Vec<Command>,
    pub states: Vec<Observed>,
    pub shots: Vec<RecordedShot>,
    /// The offset from a command's key to its state's when the source
    /// says (the probe's logs: 1); None: `align` finds it.
    pub offset: Option<i64>,
}

impl Recording {
    /// The recording player's commands and states in a client demo
    /// (`player`: its entity index; None: the demo's recorder). States are
    /// the player's own-client values at each new tick base.
    pub fn from_demo(demo: &CsDemo, player: Option<u32>) -> Result<Self, String> {
        let me = player
            .or(demo.recorder)
            .ok_or("not a client demo (SourceTV: no recording player); name one")?;
        let class_weapon = |class: u16| weapon_name(demo.class(class));
        let mut rec = Recording {
            name: demo.timeline.header.client_name.clone(),
            map: demo.map().to_string(),
            tick_interval: demo.timeline.tick_interval,
            cvars: demo.cvars.clone(),
            ..default()
        };
        let mut seen = std::collections::HashSet::new();
        let mut last_base = None;
        for f in &demo.timeline.frames {
            let local = match f.entity(me) {
                Some(Thing::Player(p)) => p.local.as_ref().map(|l| (p, l)),
                _ => None,
            };
            // A shot shows with the state after its command: keyed by that
            // state (its tick base) until the offset is known.
            let base = local.map(|(_, l)| l.tick_base).or(last_base);
            for e in &f.events {
                match e {
                    Happening::Command { cmd, weapon } => {
                        if !seen.insert(cmd.command_number) {
                            continue;
                        }
                        rec.commands.push(Command {
                            key: cmd.command_number as i64,
                            tick: f.tick,
                            number: cmd.command_number,
                            buttons: cmd.buttons,
                            forward: cmd.forward_move,
                            side: cmd.side_move,
                            up: cmd.up_move,
                            pitch: cmd.view_angles[0],
                            yaw: cmd.view_angles[1],
                            weapon: weapon.and_then(class_weapon),
                        });
                    }
                    Happening::Shot {
                        player,
                        origin,
                        pitch,
                        yaw,
                        seed,
                        inaccuracy,
                        spread,
                        ..
                    } if *player == me => rec.shots.push(RecordedShot {
                        key: base.unwrap_or(0),
                        by_state: true,
                        seed: Some(*seed),
                        inaccuracy: Some(*inaccuracy),
                        spread: Some(*spread),
                        eye: Some(*origin),
                        angles: Some([*pitch, *yaw]),
                        impact: None,
                    }),
                    _ => {}
                }
            }
            let Some((p, l)) = local else { continue };
            if last_base == Some(l.tick_base) {
                continue;
            }
            last_base = Some(l.tick_base);
            rec.states.push(Observed {
                key: l.tick_base,
                tick: f.tick,
                origin: l.origin,
                velocity: Some(l.velocity),
                on_ground: p.flags & FL_ONGROUND != 0,
                ducked: l.ducked || p.flags & FL_DUCKING != 0,
                weapon: p.weapon.and_then(class_weapon),
                clip: l.clip,
                reserve: l.reserve,
                shots_fired: Some(l.shots_fired),
                inaccuracy: None,
                alt_mode: Some(p.weapon_mode),
                waits: None,
                carried: l.weapons.iter().filter_map(|c| class_weapon(*c)).collect(),
            });
        }
        if rec.commands.is_empty() {
            return Err("the demo has no user commands (SourceTV, or a server's demo)".into());
        }
        if rec.states.is_empty() {
            return Err(format!(
                "no own-client values for player {me} (not the recording player?)"
            ));
        }
        Ok(rec)
    }

    /// A probe run's logs (tools/css_probe: `mashup_run`/`mashup_wrun`'s
    /// `<name>.in` and `<name>.out`): input `t` leads to output row `t +
    /// 1` (row 0 is where the run started). Weapon runs' `W` rows give the
    /// weapon, clip, reserve, shots, accuracy penalty and the command's
    /// number; their `E` rows the shots (`weapon_fire`) and where they hit
    /// (`bullet_impact`). The probe's player is a bot, whose random seeds
    /// don't come from its command numbers, so shots' seeds aren't compared.
    pub fn from_probe(name: &str, map: &str, input: &str, output: &str) -> Result<Self, String> {
        let mut rec = Recording {
            name: name.to_string(),
            map: map.to_string(),
            tick_interval: crate::game::TICK_INTERVAL as f32,
            offset: Some(1),
            ..default()
        };
        let num = |v: &str| v.parse::<f32>().map_err(|_| format!("{name}: not a number: {v}"));
        for (t, line) in input.lines().filter(|l| !l.starts_with("start")).enumerate() {
            let f: Vec<&str> = line.split_whitespace().collect();
            if f.len() < 5 {
                continue;
            }
            rec.commands.push(Command {
                key: t as i64,
                tick: t as i32,
                number: t as u32,
                buttons: num(f[0])? as u32,
                forward: num(f[1])?,
                side: num(f[2])?,
                pitch: num(f[3])?,
                yaw: num(f[4])?,
                // A sixth column picks weapon slot n - 1 (none here: the
                // probe names no weapon).
                ..default()
            });
        }
        let mut weapons: HashMap<i64, Vec<&str>> = HashMap::new();
        // The buttons each row's movement ran with (the bot's own duck
        // jumps among them), when logged.
        let mut ran_with: HashMap<i64, u32> = HashMap::new();
        let mut fired: Vec<i64> = Vec::new();
        let mut impacts: HashMap<i64, Vec<[f32; 3]>> = HashMap::new();
        for line in output.lines() {
            let f: Vec<&str> = line.split_whitespace().collect();
            match f.first().copied() {
                Some("W") if f.len() > 2 => {
                    weapons.insert(f[1].parse().unwrap_or(-1), f);
                }
                Some("E") if f.len() > 3 => {
                    let t: i64 = f[1].parse().unwrap_or(-1);
                    match f[2] {
                        "weapon_fire" => fired.push(t),
                        "bullet_impact" if f.len() >= 7 => {
                            impacts.entry(t).or_default().push([num(f[4])?, num(f[5])?, num(f[6])?]);
                        }
                        _ => {}
                    }
                }
                Some(first) if first.parse::<i64>().is_ok() && f.len() >= 9 => {
                    let v: Vec<f32> = f.iter().filter_map(|x| x.parse().ok()).collect();
                    if let Some(b) = v.get(11) {
                        ran_with.insert(v[0] as i64, *b as u32);
                    }
                    rec.states.push(Observed {
                        key: v[0] as i64,
                        tick: v[0] as i32,
                        origin: [v[1], v[2], v[3]],
                        velocity: Some([v[4], v[5], v[6]]),
                        on_ground: v[7] != 0.0,
                        ducked: v[8] != 0.0,
                        ..default()
                    });
                }
                _ => {}
            }
        }
        // `W <t> <weapon> clip N reserve N ... shots N ... penalty X ...
        // seed S cmd N`: the state after input t - 1, whose command
        // number is `cmd`.
        fn field<'a>(w: &[&'a str], key: &str) -> Option<&'a str> {
            w.iter().position(|x| *x == key).and_then(|i| w.get(i + 1)).copied()
        }
        for s in &mut rec.states {
            let Some(w) = weapons.get(&s.key) else { continue };
            s.weapon = Some(w[2].to_string());
            // No ammo type (a knife: clip 255, reserve -1): neither.
            s.reserve = field(w, "reserve").and_then(|v| v.parse().ok()).filter(|r| *r >= 0);
            s.clip = field(w, "clip")
                .and_then(|v| v.parse().ok())
                .filter(|_| s.reserve.is_some());
            s.shots_fired = field(w, "shots").and_then(|v| v.parse().ok());
            s.inaccuracy = field(w, "penalty").and_then(|v| v.parse().ok());
            s.alt_mode = field(w, "special").and_then(|v| v.parse().ok());
            let time = |key: &str| field(w, key).and_then(|v| v.parse::<f64>().ok());
            if let Some(base) = time("tickbase") {
                let now = base * rec.tick_interval as f64;
                let wait = |key: &str| time(key).map_or(0.0, |t| (t - now) as f32);
                s.waits = Some([wait("next_primary"), wait("next_secondary"), wait("next_attack")]);
            }
        }
        // CS:S bots duck-jump on their own, inside the game rather than
        // through their buttons (movecmp, OBSERVABILITY.md): jump and duck
        // as the movement ran them, and duck held while the bot is ducked
        // in the air. Its own attack2 (the AI putting the silencer on) is
        // taken as pressed too.
        let after: HashMap<i64, (bool, bool)> = rec.states.iter().map(|s| (s.key, (s.ducked, s.on_ground))).collect();
        for c in &mut rec.commands {
            if let Some(b) = ran_with.get(&(c.key + 1)) {
                c.buttons = (c.buttons & !(buttons::JUMP | buttons::DUCK))
                    | (b & (buttons::JUMP | buttons::DUCK | buttons::ATTACK2));
            }
            if ran_with.contains_key(&(c.key + 1)) && after.get(&(c.key + 1)).is_some_and(|(d, g)| *d && !*g) {
                c.buttons |= buttons::DUCK;
            }
            if let Some(n) = weapons
                .get(&(c.key + 1))
                .and_then(|w| field(w, "cmd"))
                .and_then(|v| v.parse().ok())
            {
                c.number = n;
            }
        }
        for t in fired {
            let w = weapons.get(&t);
            let get3 = |key: &str| -> Option<[f32; 3]> {
                let w = w?;
                let i = w.iter().position(|x| *x == key)?;
                Some([
                    w.get(i + 1)?.parse().ok()?,
                    w.get(i + 2)?.parse().ok()?,
                    w.get(i + 3)?.parse().ok()?,
                ])
            };
            rec.shots.push(RecordedShot {
                key: t - 1,
                by_state: false,
                // A bot's seed is not its command number's (the probe's
                // `seed` and `cmd` don't agree by spec 4.2): not compared.
                seed: None,
                inaccuracy: None,
                spread: None,
                eye: get3("eye"),
                angles: w.and_then(|w| {
                    let i = w.iter().position(|x| *x == "ang")?;
                    Some([w.get(i + 1)?.parse().ok()?, w.get(i + 2)?.parse().ok()?])
                }),
                impact: impacts.get(&t).and_then(|v| v.first().copied()),
            });
        }
        if rec.commands.is_empty() || rec.states.is_empty() {
            return Err(format!("{name}: no inputs or no output rows"));
        }
        // Movement runs: the probe puts its bot on its knife.
        if rec.states[0].weapon.is_none() {
            rec.states[0].weapon = Some("weapon_knife".into());
        }
        Ok(rec)
    }

    /// The commands' move amounts' largest size (`cl_forwardspeed`, 400
    /// at least): our move input is a fraction of it.
    fn key_speed(&self) -> f32 {
        self.commands
            .iter()
            .map(|c| c.forward.abs().max(c.side.abs()))
            .fold(KEY_SPEED, f32::max)
    }
}

/// Our player's state after a command, in the recording's terms.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Ours {
    pub origin: [f32; 3],
    pub velocity: [f32; 3],
    pub on_ground: bool,
    pub ducked: bool,
    pub weapon: Option<String>,
    pub clip: Option<i32>,
    pub reserve: Option<i32>,
    pub shots_fired: Option<i32>,
    pub inaccuracy: Option<f32>,
    /// The active weapon's `AltModes` mode, when it has modes that don't
    /// zoom (silencer, burst).
    pub alt_mode: Option<u8>,
}

/// A shot we fired.
#[derive(Clone, Debug, PartialEq)]
pub struct OurShot {
    pub key: i64,
    pub seed: u32,
    pub inaccuracy: Option<f32>,
    pub spread: Option<f32>,
    /// Degrees off the aim (pitch and yaw as fired) the bullet went.
    pub off_aim: f32,
}

/// What differs first, and where.
#[derive(Clone, Debug, PartialEq)]
pub struct Divergence {
    /// The command's place (from 0) and key.
    pub index: usize,
    pub key: i64,
    /// What differs ("position 0.31 u", "on ground", "clip").
    pub what: String,
    pub theirs: Observed,
    pub ours: Ours,
}

/// Limits past which a tick counts as different.
#[derive(Clone, Copy, Debug)]
pub struct Tolerance {
    /// Units.
    pub position: f32,
    /// Units per second.
    pub velocity: f32,
    pub inaccuracy: f32,
}

impl Default for Tolerance {
    fn default() -> Self {
        Self {
            position: 0.1,
            velocity: 0.5,
            inaccuracy: 0.0005,
        }
    }
}

/// How shots compared.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ShotStats {
    pub recorded: usize,
    pub ours: usize,
    /// Paired in order: how many fired on the same command.
    pub same_tick: usize,
    /// Seeds recorded and compared, and how many were equal.
    pub seeds_compared: usize,
    pub seeds_equal: usize,
    /// Largest difference in accuracy (inaccuracy) at a paired shot.
    pub max_inaccuracy_diff: f32,
    /// Mean degrees off the aim, recorded (from impacts) and ours.
    pub theirs_off_aim: Option<f32>,
    pub ours_off_aim: Option<f32>,
    pub first_mismatch: Option<String>,
}

/// One compared tick, for CSVs.
#[derive(Clone, Debug)]
pub struct Row {
    pub key: i64,
    pub theirs: Observed,
    pub ours: Ours,
}

/// What a re-simulation found.
#[derive(Clone, Debug, Default)]
pub struct Report {
    pub name: String,
    /// From a command's key to its state's.
    pub offset: i64,
    /// Commands run, and how many had a recorded state to compare.
    pub commands: usize,
    pub compared: usize,
    pub first: Option<Divergence>,
    pub max_position: f32,
    pub mean_position: f32,
    pub max_velocity: f32,
    /// Largest accuracy penalty difference (where recorded).
    pub max_inaccuracy: f32,
    /// Ticks where these differed.
    pub ground: usize,
    pub ducked: usize,
    pub weapon: usize,
    pub clip: usize,
    pub reserve: usize,
    pub shots_fired: usize,
    pub inaccuracy: usize,
    /// Ticks where the weapon's alternate state (silencer, burst) differed
    /// after a command with attack2 (a divergence: our toggle isn't the
    /// game's).
    pub alt_mode: usize,
    /// Times the recording's alternate state was set on ours because it
    /// changed without attack2 (from outside the commands: a weapon
    /// switched to, a bot's AI).
    pub alt_mode_applied: usize,
    /// Times ours switched to the recorded weapon because it changed
    /// without a command picking it (a bot's AI, the game's own switch
    /// when a weapon runs dry).
    pub weapon_applied: usize,
    pub shots: ShotStats,
    /// The console variables applied (name, value).
    pub cvars: Vec<(String, String)>,
    pub rows: Vec<Row>,
}

impl Report {
    /// Per-tick CSV: key, then the recorded and our values.
    pub fn csv(&self) -> String {
        let mut out = String::from(
            "key,css_x,css_y,css_z,css_vx,css_vy,css_vz,css_ground,css_ducked,css_clip,\
             ours_x,ours_y,ours_z,ours_vx,ours_vy,ours_vz,ours_ground,ours_ducked,ours_clip,pos_diff,\
             css_inaccuracy,ours_inaccuracy,css_alt_mode,ours_alt_mode\n",
        );
        for r in &self.rows {
            let v = r.theirs.velocity.unwrap_or_default();
            let o = &r.ours;
            out += &format!(
                "{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{}\n",
                r.key,
                r.theirs.origin[0],
                r.theirs.origin[1],
                r.theirs.origin[2],
                v[0],
                v[1],
                v[2],
                r.theirs.on_ground as u8,
                r.theirs.ducked as u8,
                r.theirs.clip.map_or(String::new(), |c| c.to_string()),
                o.origin[0],
                o.origin[1],
                o.origin[2],
                o.velocity[0],
                o.velocity[1],
                o.velocity[2],
                o.on_ground as u8,
                o.ducked as u8,
                o.clip.map_or(String::new(), |c| c.to_string()),
                dist(r.theirs.origin, o.origin),
                r.theirs.inaccuracy.map_or(String::new(), |v| v.to_string()),
                o.inaccuracy.map_or(String::new(), |v| v.to_string()),
                r.theirs.alt_mode.map_or(String::new(), |v| v.to_string()),
                o.alt_mode.map_or(String::new(), |v| v.to_string()),
            );
        }
        out
    }
}

impl fmt::Display for Report {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(
            f,
            "{}: {} commands, {} compared (state key = command key {:+})",
            self.name, self.commands, self.compared, self.offset
        )?;
        if !self.cvars.is_empty() {
            let list: Vec<String> = self.cvars.iter().map(|(k, v)| format!("{k} {v}")).collect();
            writeln!(f, "  server settings applied: {}", list.join(", "))?;
        }
        writeln!(
            f,
            "  position: max {:.3} u, mean {:.3} u; velocity: max {:.2} u/s; accuracy penalty: max diff {:.5}",
            self.max_position, self.mean_position, self.max_velocity, self.max_inaccuracy
        )?;
        writeln!(
            f,
            "  ticks differing: on ground {}, ducked {}, weapon {}, clip {}, reserve {}, shots fired {}, accuracy {}, alt mode {}",
            self.ground,
            self.ducked,
            self.weapon,
            self.clip,
            self.reserve,
            self.shots_fired,
            self.inaccuracy,
            self.alt_mode
        )?;
        if self.weapon_applied > 0 {
            writeln!(
                f,
                "  weapon switched to as recorded {} times (changed without a pick)",
                self.weapon_applied
            )?;
        }
        if self.alt_mode_applied > 0 {
            writeln!(
                f,
                "  alt mode (silencer, burst) set from the recording {} times (changed without attack2)",
                self.alt_mode_applied
            )?;
        }
        let s = &self.shots;
        writeln!(
            f,
            "  shots: {} recorded, {} ours, {} on the same command; seeds {}/{} equal; accuracy max diff {:.5}{}{}",
            s.recorded,
            s.ours,
            s.same_tick,
            s.seeds_equal,
            s.seeds_compared,
            s.max_inaccuracy_diff,
            s.theirs_off_aim
                .map_or(String::new(), |d| format!("; off aim: recorded {d:.3} deg")),
            s.ours_off_aim.map_or(String::new(), |d| format!(", ours {d:.3} deg")),
        )?;
        if let Some(m) = &s.first_mismatch {
            writeln!(f, "  first shot mismatch: {m}")?;
        }
        match &self.first {
            None => writeln!(f, "  first divergence: none"),
            Some(d) => {
                let t = &d.theirs;
                let o = &d.ours;
                writeln!(
                    f,
                    "  first divergence: command {} (key {}): {}\n    css  {:.2?} v {:.1?} g{} d{} {:?} clip {:?}\n    ours {:.2?} v {:.1?} g{} d{} {:?} clip {:?}",
                    d.index,
                    d.key,
                    d.what,
                    t.origin,
                    t.velocity.unwrap_or_default(),
                    t.on_ground as u8,
                    t.ducked as u8,
                    t.weapon,
                    t.clip,
                    o.origin,
                    o.velocity,
                    o.on_ground as u8,
                    o.ducked as u8,
                    o.weapon,
                    o.clip,
                )
            }
        }
    }
}

fn dist(a: [f32; 3], b: [f32; 3]) -> f32 {
    Vec3::from_array(a).distance(Vec3::from_array(b))
}

/// Options for a run.
#[derive(Clone, Copy, Debug, Default)]
pub struct Options {
    /// From a command's key to its state's (None: the recording's, else
    /// `align`'s).
    pub offset: Option<i64>,
    /// Run at most this many commands.
    pub limit: Option<usize>,
    pub tolerance: Tolerance,
}

/// Re-simulate `rec` in a fresh world from `make` (CS:S's simulation with
/// a map, `movement::SourceMovementPlugin` and `weapons::CsWeaponsPlugin`)
/// and compare.
pub fn resimulate(make: &dyn Fn() -> Sim, rec: &Recording, opts: Options) -> Result<Report, String> {
    let offset = match opts.offset.or(rec.offset) {
        Some(o) => o,
        None => align(make, rec)?,
    };
    run(&mut make(), rec, offset, opts)
}

/// The offset from command keys to state keys that fits the first
/// commands best: candidates around the first state's key less the key of
/// the last command recorded by then, each tried on up to 64 commands.
pub fn align(make: &dyn Fn() -> Sim, rec: &Recording) -> Result<i64, String> {
    let first = rec.states.first().ok_or("no recorded states")?;
    let before = rec
        .commands
        .iter()
        .rev()
        .find(|c| c.tick <= first.tick)
        .or(rec.commands.first())
        .ok_or("no commands")?;
    let guess = first.key - before.key;
    let mut best: Option<(f32, i64)> = None;
    for o in guess - 4..=guess + 8 {
        let opts = Options {
            offset: Some(o),
            limit: Some(64),
            ..default()
        };
        let Ok(r) = run(&mut make(), rec, o, opts) else {
            continue;
        };
        if r.compared < 8 {
            continue;
        }
        if best.is_none_or(|(e, _)| r.mean_position < e) {
            best = Some((r.mean_position, o));
        }
    }
    best.map(|(_, o)| o)
        .ok_or_else(|| "no offset lines the commands up with the states".to_string())
}

/// Our state now.
fn ours(sim: &Sim, p: Entity) -> Ours {
    let w = sim.app.world();
    let me = w.get::<SourceMovement>(p);
    let active = w.get::<Inventory>(p).and_then(|i| i.active);
    let magazine = active.and_then(|a| w.get::<Magazine>(a));
    Ours {
        origin: (to_source(sim.position(p)) - Vec3::Z * ORIGIN_ABOVE_FEET).to_array(),
        velocity: to_source(sim.velocity(p)).to_array(),
        on_ground: me.is_some_and(|m| m.on_ground),
        ducked: me.is_some_and(|m| m.ducked),
        weapon: active
            .and_then(|a| w.get::<Weapon>(a))
            .map(|w| w.id.rsplit(':').next().unwrap_or(w.id).to_string()),
        clip: magazine.map(|m| m.clip as i32),
        reserve: magazine.map(|m| m.reserve as i32),
        shots_fired: active.and_then(|a| w.get::<WeaponState>(a)).map(|s| s.burst as i32),
        inaccuracy: active.and_then(|a| w.get::<Inaccuracy>(a)).map(|i| i.value),
        alt_mode: active.and_then(|a| toggle(w, a)).map(|m| m.current),
    }
}

/// The weapon's modes when they toggle a state (silencer, burst) rather
/// than zoom.
fn toggle(w: &World, weapon: Entity) -> Option<&AltModes> {
    if w.get::<Zoom>(weapon).is_some() {
        return None;
    }
    w.get::<AltModes>(weapon)
}

/// Switch to the carried weapon `name` (given if not carried); false if
/// there is no such weapon.
fn switch_to(world: &mut World, p: Entity, name: &str) -> bool {
    let carried = world.get::<Inventory>(p).and_then(|i| {
        i.weapons
            .iter()
            .copied()
            .find(|e| world.get::<Weapon>(*e).is_some_and(|w| w.id.ends_with(name)))
    });
    let Some(target) = carried.or_else(|| give(world, p, name)) else {
        return false;
    };
    let Some(mut i) = world.get_mut::<Inventory>(p) else {
        return false;
    };
    i.wanted = Some(target);
    true
}

/// Set the active weapon's alternate mode (silencer, burst) to `mode`.
fn set_alt_mode(world: &mut World, p: Entity, mode: u8) -> bool {
    let Some(a) = world.get::<Inventory>(p).and_then(|i| i.active) else {
        return false;
    };
    if toggle(world, a).is_none() {
        return false;
    }
    let Some(mut m) = world.get_mut::<AltModes>(a) else {
        return false;
    };
    m.current = mode.min(m.count.saturating_sub(1));
    true
}

/// Our view angles from Source's degrees.
fn look(pitch: f32, yaw: f32) -> (f32, f32) {
    (yaw.to_radians() - FRAC_PI_2, -pitch.to_radians())
}

fn run(sim: &mut Sim, rec: &Recording, offset: i64, opts: Options) -> Result<Report, String> {
    let start = rec.states.first().ok_or("no recorded states")?;
    let todo: Vec<&Command> = rec
        .commands
        .iter()
        .filter(|c| c.key + offset > start.key)
        .take(opts.limit.unwrap_or(usize::MAX))
        .collect();
    let states: HashMap<i64, &Observed> = rec.states.iter().map(|s| (s.key, s)).collect();

    // The server's settings, the recording's tick and move keys.
    sim.set_tick_interval(rec.tick_interval as f64);
    let mut config = SourceMovementConfig::default();
    let mut applied = Vec::new();
    for (k, v) in &rec.cvars {
        if config.set_cvar(k, v).is_ok() {
            applied.retain(|(n, _): &(String, String)| n != k);
            applied.push((k.clone(), v.clone()));
        }
    }
    config.key_speed = rec.key_speed();
    sim.app.insert_resource(config);

    // The player with its weapons, drawn, then placed as recorded.
    let p = sim.spawn_character(
        to_engine(Vec3::from_array(start.origin) + Vec3::Z * ORIGIN_ABOVE_FEET),
        movement::ID,
    );
    sim.ticks(1);
    // What the recording says it carries, not the game's spawn loadout.
    if start.weapon.is_some() {
        let world = sim.app.world_mut();
        let old = world.get::<Inventory>(p).map(|i| i.weapons.clone()).unwrap_or_default();
        for w in old {
            if let Ok(e) = world.get_entity_mut(w) {
                e.despawn();
            }
        }
        world.entity_mut(p).insert(Inventory::default());
    }
    let mut held = None;
    for w in start.carried.iter().chain(start.weapon.iter()) {
        let world = sim.app.world_mut();
        let has = world.get::<Inventory>(p).is_some_and(|i| {
            i.weapons
                .iter()
                .any(|e| world.get::<Weapon>(*e).is_some_and(|x| x.id.ends_with(w.as_str())))
        });
        if has {
            continue;
        }
        if let Some(e) = give(world, p, w)
            && Some(w) == start.weapon.as_ref()
        {
            held = Some(e);
        }
    }
    if let Some(e) = held {
        sim.app.world_mut().get_mut::<Inventory>(p).expect("given").wanted = Some(e);
    }
    let draw = held
        .and_then(|e| sim.app.world().get::<Weapon>(e))
        .map_or(0.0, |w| w.draw_time);
    sim.seconds(draw as f64 + 0.05);
    {
        let world = sim.app.world_mut();
        let mut tf = world.get_mut::<Transform>(p).expect("a character");
        tf.translation = to_engine(Vec3::from_array(start.origin) + Vec3::Z * ORIGIN_ABOVE_FEET);
        world.get_mut::<Velocity>(p).expect("a character").0 =
            to_engine(Vec3::from_array(start.velocity.unwrap_or_default()));
        let mut me = world
            .get_mut::<SourceMovement>(p)
            .ok_or("no Source movement (add its plugin)")?;
        me.on_ground = start.on_ground;
        me.ducked = start.ducked;
        // The weapon's timers as recorded (ready unless the recording
        // says to wait), counted from the first command's time.
        let next = world.resource::<SimClock>().now + rec.tick_interval as f64;
        let [primary, secondary, attack] = start
            .waits
            .unwrap_or_default()
            .map(|w| if w > 0.0 { next + w as f64 } else { 0.0 });
        if let Some(m) = start.alt_mode {
            set_alt_mode(world, p, m);
        }
        if let Some(a) = world.get::<Inventory>(p).and_then(|i| i.active) {
            if let Some(mut m) = world.get_mut::<Magazine>(a) {
                if let Some(c) = start.clip {
                    m.clip = c.max(0) as u32;
                }
                if let Some(r) = start.reserve {
                    m.reserve = r.max(0) as u32;
                }
            }
            // The penalty as recorded, and where it recovers from: on the
            // ground or not, falling as recorded (not what the placement
            // ticks left).
            if let Some(mut i) = world.get_mut::<Inaccuracy>(a) {
                let (value, _, _) = i.net();
                let fall = start.velocity.map_or(0.0, |v| v[2]);
                i.set_net((start.inaccuracy.unwrap_or(value), start.on_ground, fall));
            }
            if let Some(mut s) = world.get_mut::<WeaponState>(a) {
                s.next_primary = primary;
                s.next_secondary = secondary;
                if let Some(n) = start.shots_fired {
                    s.burst = n.max(0) as u32;
                }
            }
        }
        if let Some(mut i) = world.get_mut::<Inventory>(p) {
            i.next_attack = attack;
        }
    }

    let mut report = Report {
        name: rec.name.clone(),
        offset,
        cvars: applied,
        ..default()
    };
    let key_speed = rec.key_speed();
    let mut cursor = sim.app.world().resource::<Messages<WeaponEvent>>().get_cursor();
    let mut our_shots: Vec<OurShot> = Vec::new();
    let mut sum_position = 0.0;
    for (index, c) in todo.iter().enumerate() {
        // The command: its number (spread and recoil seeds), angles,
        // buttons and move amounts; a weapon pick switches at once.
        let next_tick = sim.app.world().resource::<SimTick>().0 + 1;
        let (yaw, pitch) = look(c.pitch, c.yaw);
        {
            let world = sim.app.world_mut();
            world
                .entity_mut(p)
                .insert(Seed(c.number.wrapping_sub(next_tick as u32) as u64));
            if let Some(name) = &c.weapon {
                let target = world.get::<Inventory>(p).and_then(|i| {
                    i.weapons
                        .iter()
                        .copied()
                        .find(|e| world.get::<Weapon>(*e).is_some_and(|w| w.id.ends_with(name.as_str())))
                });
                if let (Some(t), Some(mut i)) = (target, world.get_mut::<Inventory>(p))
                    && i.active != Some(t)
                {
                    i.wanted = Some(t);
                }
            }
        }
        {
            let mut i = sim.intent(p);
            i.move_axis = Vec2::new(c.side / key_speed, c.forward / key_speed);
            i.yaw = yaw;
            i.pitch = pitch;
            i.jump = c.buttons & buttons::JUMP != 0;
            i.crouch = c.buttons & buttons::DUCK != 0;
            i.walk = c.buttons & buttons::SPEED != 0;
            i.fire = c.buttons & buttons::ATTACK != 0;
            i.secondary = c.buttons & buttons::ATTACK2 != 0;
            i.reload = c.buttons & buttons::RELOAD != 0;
            i.use_key = c.buttons & buttons::USE != 0;
        }
        sim.ticks(1);
        // Our shots this command.
        let mut fired: Option<(u32, Option<f32>, Option<f32>, Quat)> = None;
        let events: Vec<WeaponEvent> = cursor
            .read(sim.app.world().resource::<Messages<WeaponEvent>>())
            .filter(|e| e.owner == p)
            .cloned()
            .collect();
        for e in events {
            match e.kind {
                WeaponEventKind::Fired {
                    seed,
                    yaw,
                    pitch,
                    spread,
                    ..
                } => {
                    let (inacc, spr) = match spread {
                        crate::weapon::SpreadShape::Disc { inaccuracy, spread } => (Some(inaccuracy), Some(spread)),
                        crate::weapon::SpreadShape::Template(s) | crate::weapon::SpreadShape::Cone(s) => (Some(s), None),
                    };
                    fired = Some((seed, inacc, spr, crate::weapon::aim_of(yaw, pitch)));
                }
                WeaponEventKind::Shot { from, to, .. } => {
                    if let Some((seed, inaccuracy, spread, aim)) = fired.take() {
                        let dir = (to - from).normalize_or_zero();
                        let off = (aim * Vec3::NEG_Z).angle_between(dir).to_degrees();
                        our_shots.push(OurShot {
                            key: c.key,
                            seed,
                            inaccuracy,
                            spread,
                            off_aim: off,
                        });
                    }
                }
                _ => {}
            }
        }
        report.commands += 1;
        let Some(theirs) = states.get(&(c.key + offset)) else {
            continue;
        };
        let mut ours = ours(sim, p);
        report.compared += 1;
        let dp = dist(theirs.origin, ours.origin);
        sum_position += dp;
        report.max_position = report.max_position.max(dp);
        let dv = theirs.velocity.map_or(0.0, |v| dist(v, ours.velocity));
        report.max_velocity = report.max_velocity.max(dv);
        let tol = opts.tolerance;
        let mut what = Vec::new();
        if dp > tol.position {
            what.push(format!("position {dp:.3} u"));
        }
        if dv > tol.velocity {
            what.push(format!("velocity {dv:.2} u/s"));
        }
        let mut count = |differs: bool, n: &mut usize, name: &str| {
            if differs {
                *n += 1;
                what.push(name.to_string());
            }
        };
        count(theirs.on_ground != ours.on_ground, &mut report.ground, "on ground");
        count(theirs.ducked != ours.ducked, &mut report.ducked, "ducked");
        let weapon_differs = theirs.weapon.is_some() && theirs.weapon != ours.weapon;
        // A switch no command picked (a bot's AI, the game's own when a
        // weapon runs dry): switched to as recorded, from the next command
        // on; its weapon's values aren't compared this once.
        let switched = weapon_differs
            && c.weapon.is_none()
            && theirs
                .weapon
                .as_deref()
                .is_some_and(|name| switch_to(sim.app.world_mut(), p, name));
        if switched {
            report.weapon_applied += 1;
        }
        count(weapon_differs && !switched, &mut report.weapon, "weapon");
        let differs = |a: Option<i32>, b: Option<i32>| !switched && a.is_some() && a != b;
        count(differs(theirs.clip, ours.clip), &mut report.clip, "clip");
        count(differs(theirs.reserve, ours.reserve), &mut report.reserve, "reserve");
        count(
            differs(theirs.shots_fired, ours.shots_fired),
            &mut report.shots_fired,
            "shots fired",
        );
        let di = theirs
            .inaccuracy
            .zip(ours.inaccuracy)
            .filter(|_| !switched)
            .map(|(a, b)| (a - b).abs());
        report.max_inaccuracy = report.max_inaccuracy.max(di.unwrap_or(0.0));
        count(
            di.is_some_and(|d| d > tol.inaccuracy),
            &mut report.inaccuracy,
            "accuracy",
        );
        // The silencer or burst (of the same weapon): a toggle by this
        // command's attack2 must match; a change without one came from
        // outside the commands (a weapon drawn, a bot's AI) and is taken as
        // recorded.
        if let (Some(a), Some(b)) = (theirs.alt_mode, ours.alt_mode)
            && a != b
            && !weapon_differs
        {
            if c.buttons & buttons::ATTACK2 != 0 {
                count(true, &mut report.alt_mode, "alt mode");
            } else if set_alt_mode(sim.app.world_mut(), p, a) {
                report.alt_mode_applied += 1;
                ours.alt_mode = Some(a);
            }
        }
        if report.first.is_none() && !what.is_empty() {
            report.first = Some(Divergence {
                index,
                key: c.key,
                what: what.join(", "),
                theirs: (*theirs).clone(),
                ours: ours.clone(),
            });
        }
        report.rows.push(Row {
            key: c.key,
            theirs: (*theirs).clone(),
            ours,
        });
    }
    report.mean_position = if report.compared > 0 {
        sum_position / report.compared as f32
    } else {
        0.0
    };
    // Shots: a demo's are keyed by the state they came with.
    let last = todo.last().map_or(i64::MIN, |c| c.key);
    let first = todo.first().map_or(i64::MAX, |c| c.key);
    let recorded: Vec<RecordedShot> = rec
        .shots
        .iter()
        .map(|s| {
            let mut s = s.clone();
            if s.by_state {
                s.key -= offset;
                s.by_state = false;
            }
            s
        })
        .filter(|s| s.key >= first && s.key <= last)
        .collect();
    report.shots = compare_shots(&recorded, &our_shots, opts.tolerance);
    Ok(report)
}

/// Pair recorded and our shots in order and compare them.
fn compare_shots(theirs: &[RecordedShot], ours: &[OurShot], tol: Tolerance) -> ShotStats {
    let mut s = ShotStats {
        recorded: theirs.len(),
        ours: ours.len(),
        ..default()
    };
    let mut off_theirs = Vec::new();
    for t in theirs {
        if let (Some(eye), Some([pitch, yaw]), Some(hit)) = (t.eye, t.angles, t.impact) {
            let (y, p) = look(pitch, yaw);
            let aim = crate::weapon::aim_of(y, p) * Vec3::NEG_Z;
            let dir = to_engine(Vec3::from_array(hit) - Vec3::from_array(eye)).normalize_or_zero();
            off_theirs.push(aim.angle_between(dir).to_degrees());
        }
    }
    let mean = |v: &[f32]| (!v.is_empty()).then(|| v.iter().sum::<f32>() / v.len() as f32);
    s.theirs_off_aim = mean(&off_theirs);
    s.ours_off_aim = mean(&ours.iter().map(|o| o.off_aim).collect::<Vec<_>>());
    for (i, (t, o)) in theirs.iter().zip(ours).enumerate() {
        let mut bad = Vec::new();
        if t.key == o.key {
            s.same_tick += 1;
        } else {
            bad.push(format!("command {} vs ours {}", t.key, o.key));
        }
        if let Some(seed) = t.seed {
            s.seeds_compared += 1;
            if seed == o.seed {
                s.seeds_equal += 1;
            } else {
                bad.push(format!("seed {seed} vs ours {}", o.seed));
            }
        }
        if let (Some(a), Some(b)) = (t.inaccuracy, o.inaccuracy) {
            let d = (a - b).abs();
            s.max_inaccuracy_diff = s.max_inaccuracy_diff.max(d);
            if d > tol.inaccuracy {
                bad.push(format!("inaccuracy {a:.5} vs ours {b:.5}"));
            }
        }
        if s.first_mismatch.is_none() && !bad.is_empty() {
            s.first_mismatch = Some(format!("shot {i}: {}", bad.join(", ")));
        }
    }
    if s.first_mismatch.is_none() && theirs.len() != ours.len() {
        s.first_mismatch = Some(format!("{} shots recorded, {} ours", theirs.len(), ours.len()));
    }
    s
}
