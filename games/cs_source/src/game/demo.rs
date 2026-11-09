//! CS:S demos played back in our world (`playdemo`; the console command
//! and the camera are the front end's, `ui::demo`): a `.dem` file read
//! once into frames (`source_formats::dem::playback`, through `CsMapper`:
//! CS:S's classes and props as `Thing`s, its temp entities, sounds, game
//! events and user messages as `Happening`s), then shown frame by frame.
//! Nothing simulates while it plays (`core::Playback`: the rules, bots,
//! damage and map logic are off, as on a network client): every player is
//! a puppet that the recording moves.
//!
//! - **Players** (`CCSPlayer`): a character per player on a team
//!   (`DemoPuppet`, `NetDrawn`: our renderer, animation, effects and HUD
//!   show it), placed and turned as recorded (eased between snapshots),
//!   its team, class (model), health, armour, money, kills and deaths, on
//!   ground and ducked from `m_fFlags`, alive from `m_lifeState`, holding
//!   its active weapon (`m_hActiveWeapon`'s class, our weapon of that
//!   name: `weapon::remote::show_held`) with its silencer or burst on.
//! - **Shots** (`CTEFireBullets`): traced again here as a network client
//!   traces others' (`weapon::remote::RemoteShot`: tracers, impacts,
//!   decals, the fire sound, the body's and the watched view model's
//!   fire animation) from the recorded eye, angles, seed and spread.
//!   Reloads (`CTEPlayerAnimEvent`) as `RemoteAction`s for the bodies.
//! - **Grenades** in flight (`CBaseCSGrenadeProjectile`: their model
//!   names the weapon) drawn with their weapon's model; a grenade gone
//!   (its entity deleted) went off: `Detonated` and its sound. Smoke
//!   clouds (`ParticleSmokeGrenade`) as `SmokeCloud`s whose age and fade
//!   follow `m_flSpawnTime`, `m_FadeStartTime` and `m_FadeEndTime`.
//! - **The bomb** (`CPlantedC4`): our planted bomb where it lies, its
//!   clock (`m_flC4Blow`, `m_flTimerLength`) and beeps (CS:S's client
//!   beeps; the demo carries none).
//! - **The round** (`CCSGameRulesProxy`, `CCSTeam`, the round events): the
//!   HUD's round clock, freeze time, scores and the round's end.
//! - **Sounds** (`svc_Sounds`, by the `soundprecache` name), deaths
//!   (`player_death`: the kill feed, ragdolls), chat and radio
//!   (`SayText2`, `RadioText`: `DemoText`; `SendAudio`; bots' `RawAudio`).
//!
//! - **A client demo's own player** (`CsDemo::recorder`, from the server
//!   info's player slot): its own-client values (`PlayerSnap::local`:
//!   full-precision origin, velocity, ducked, tick base, clip, reserve,
//!   weapons carried) and its commands (`Happening::Command`), with the
//!   server's console variables (`CsDemo::cvars`), for re-simulating
//!   (`resim`); playback shows it like any player.
//!
//! The clock: `DemoPlayer::tick` runs with virtual time, so
//! `demo_timescale` is `Time<Virtual>`'s speed and `demo_pause` pauses it
//! (everything stops, as in CS:S). `demo_gototick` jumps anywhere: every
//! frame holds the whole state, so seeking rebuilds it from that frame
//! (no events replayed). `DemoClock` is the demo's own time and its jumps,
//! for what ages with it (the HUD's death notices: cleared on a jump).
//!
//! Known shortcuts (docs/tech-debt.md): CS:S players carry no animation in
//! demos (client-side animation: no `m_nSequence`, cycle or pose
//! parameters), so their bodies are animated by our player animation from
//! the recorded velocity (between snapshots), look and flags, as for any
//! character; anim events 4-7 are read as reload start, shell, reload end
//! (inferred from the sample demo's `ReloadEffect` messages), others
//! (jumps come from the movement) are left out; zoom is not shown (no
//! scope through a sniper's eyes); demo sounds play at a fixed 75 dB level
//! with the recorded volume and pitch (raw waves decoded from the install
//! at start, the map's sound bank copied once to add them), never looped
//! or stopped; physics props, doors and other map entities stay where the
//! map put them; flashbangs don't blind the watched player; a dead
//! player's body after a seek stands where it was (no ragdoll without the
//! death seen). Why it's OK for now: the players, their weapons, shots,
//! grenades, the bomb and the round play as recorded, which is what a
//! demo compared with CS:S's needs first.

use std::{collections::HashMap, sync::Arc};

use avian3d::prelude::ColliderDisabled;
use bevy::{app::RunFixedMainLoopSystems, prelude::*};
use source_formats::dem::{
    DemoState, Record,
    sendtable::PropValue,
    usercmd::UserCmd,
    entities::Entity as DemEntity,
    events::{EventValue, GameEvent, SND_STOP, UserMessage},
    playback::{self, ClassProps, Mapper, Props, Timeline},
};

use crate::{
    character::character_bundle,
    console::ConsoleAppExt,
    core::{
        Damage, Dead, Died, Health, Hitgroup, Intent, LocalPlayer, MovementState, Playback, PlayerClass, Score,
        Spectating, Team, Velocity,
    },
    map::{
        PlaySound,
        entities::{entity_rotation, rotation_to_engine},
        interp::{EyeView, NetDrawn, RenderedView, SNAP_SPEED},
        loose::{ShownItem, SitOnOrigin},
        sound::{Interval, MapSoundEntry, SoundBank, SoundLevel},
    },
    objectives::bomb::{BombRules, BombState, PlantedBomb, beep_interval},
    rules::rounds::{ATTACKERS, DEFENDERS, Phase, RoundEndReason, RoundEnded, RoundState, RoundSummary},
    weapon::{
        Armor, Hitscan, Inventory, SpreadShape, Weapon, WeaponEventKind, WeaponRegistry,
        economy::Money,
        grenade::{Detonated, GrenadeEffect, GrenadeKind, SmokeCloud, Throwable},
        remote::{RemoteAction, RemoteShot, show_held},
    },
};

const UNIT: f32 = 0.0254;
/// Our characters' origin sits this far above the feet (the standing
/// box's centre; Source's origin is the feet).
const ORIGIN_ABOVE_FEET: f32 = 36.0;
/// Eye heights above the feet, standing and ducked (CS:S's view offsets).
const EYE_STANDING: f32 = 64.0;
const EYE_DUCKED: f32 = 46.0;
/// `m_fFlags` bits.
pub const FL_ONGROUND: i64 = 1;
pub const FL_DUCKING: i64 = 2;
/// Players' scores and deaths kept per index (`CCSPlayerResource`).
const MAX_PLAYERS: usize = 65;

// ---------------------------------------------------------------------------
// What a demo holds

/// One entity at a frame, as playback uses it. Positions and angles are
/// Source's (units, degrees).
#[derive(Clone, Debug)]
pub enum Thing {
    Player(PlayerSnap),
    Grenade(GrenadeSnap),
    Smoke(SmokeSnap),
    Bomb(BombSnap),
    Rules(RulesSnap),
    Team(TeamSnap),
    /// Kills and deaths by player index (`CCSPlayerResource`).
    Scores(Arc<[(i32, i32)]>),
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct PlayerSnap {
    /// Feet.
    pub origin: [f32; 3],
    /// Eye angles.
    pub pitch: f32,
    pub yaw: f32,
    /// Source's team number: 1 spectators, 2 terrorists, 3 counter-terrorists.
    pub team: u8,
    /// `m_iClass`: 1-4 the terrorists' models, 5-8 the counter-terrorists'.
    pub class: u8,
    pub health: i32,
    pub armor: i32,
    pub helmet: bool,
    /// `m_fFlags`.
    pub flags: i64,
    pub alive: bool,
    /// The active weapon's server class (`CsDemo::classes`).
    pub weapon: Option<u16>,
    /// Its silencer on or burst mode (our alternate mode 1).
    pub weapon_mode: u8,
    pub money: i32,
    pub night_vision: bool,
    pub defuser: bool,
    /// What only the player's own client gets (a client demo's recording
    /// player; SourceTV has none).
    pub local: Option<LocalSnap>,
}

/// A player's own-client values (`localdata`, `cslocaldata`, its weapons'
/// `LocalWeaponData`): the recording player's in a client (POV) demo.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct LocalSnap {
    /// Feet, at full precision.
    pub origin: [f32; 3],
    pub velocity: [f32; 3],
    /// `m_Local.m_bDucked`, `m_bDucking`.
    pub ducked: bool,
    pub ducking: bool,
    /// Commands run so far (`m_nTickBase`).
    pub tick_base: i64,
    /// `m_iShotsFired`.
    pub shots_fired: i32,
    /// The active weapon's clip and its ammo type's reserve
    /// (`m_iClip1`, `m_iAmmo[m_iPrimaryAmmoType]`).
    pub clip: Option<i32>,
    pub reserve: Option<i32>,
    /// The weapons carried, by server class (`m_hMyWeapons`).
    pub weapons: Vec<u16>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct GrenadeSnap {
    pub origin: [f32; 3],
    pub angles: [f32; 3],
    /// `modelprecache` index (`CsDemo::models`).
    pub model: Option<u16>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct SmokeSnap {
    pub origin: [f32; 3],
    /// Server times (s).
    pub spawn_time: f32,
    pub fade_start: f32,
    pub fade_end: f32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct BombSnap {
    pub origin: [f32; 3],
    pub angles: [f32; 3],
    pub ticking: bool,
    /// When it blows (server time, s) and its timer (s).
    pub blow: f32,
    pub timer: f32,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct RulesSnap {
    pub freeze: bool,
    /// Round length (s) and when it goes (went) live (server time, s).
    pub round_time: i32,
    pub round_start: f32,
    /// Between `round_end` and the next `round_start`: the winner (Source's
    /// team number; None: a draw).
    pub over: Option<Option<u8>>,
    /// Rounds started since the recording began.
    pub rounds: u32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TeamSnap {
    pub team: u8,
    pub score: i32,
}

/// Something that happened, at its frame. Entities by index.
#[derive(Clone, Debug, PartialEq)]
pub enum Happening {
    /// A round fired (`CTEFireBullets`): the shooter's eye and angles.
    Shot {
        player: u32,
        origin: [f32; 3],
        pitch: f32,
        yaw: f32,
        mode: u8,
        seed: u32,
        inaccuracy: f32,
        spread: f32,
    },
    /// `CTEPlayerAnimEvent`.
    Anim {
        player: u32,
        event: i64,
    },
    /// A sound started (`svc_Sounds`), by its file (no prefix characters,
    /// lower case, under `sound/`).
    Sound {
        entity: u32,
        file: String,
        origin: [f32; 3],
        volume: f32,
        pitch: u32,
    },
    Death {
        victim: u32,
        attacker: Option<u32>,
        weapon: String,
        headshot: bool,
    },
    RoundStart,
    RoundEnd {
        winner: Option<u8>,
        message: String,
    },
    /// A grenade gone from the world: it went off where it was.
    Detonate {
        origin: [f32; 3],
        model: Option<u16>,
    },
    /// A line for the chat area (chat, radio).
    Text(String),
    /// A command of the recording player (client demos), with the server
    /// class of the weapon it picks (`weaponselect`), if any.
    Command {
        cmd: UserCmd,
        weapon: Option<u16>,
    },
    /// A sound entry everyone hears (`SendAudio`: the radio, the announcer).
    Audio(String),
    /// A bot's chatter (`RawAudio`): its wave under `sound/`.
    Voice {
        entity: u32,
        file: String,
    },
}

/// A CS:S demo read for playback.
#[derive(Clone, Debug)]
pub struct CsDemo {
    pub timeline: Timeline<Thing, Happening>,
    /// Server classes by id.
    pub classes: Vec<String>,
    /// `modelprecache` by index.
    pub models: Vec<String>,
    /// Players' names by entity index (the last ones seen).
    pub names: HashMap<u32, String>,
    /// The waves its sounds and bots' chatter play (`Happening::Sound`'s
    /// and `Voice`'s files).
    pub sounds: Vec<String>,
    /// The recording player's entity index (a client demo: its
    /// `svc_ServerInfo` player slot + 1; None for SourceTV).
    pub recorder: Option<u32>,
    /// The server's console variables as it sent them (`net_SetConVar`,
    /// in order: a later value overrides).
    pub cvars: Vec<(String, String)>,
}

impl CsDemo {
    /// The map the demo was recorded on.
    pub fn map(&self) -> &str {
        &self.timeline.map
    }

    pub fn class(&self, id: u16) -> &str {
        self.classes.get(id as usize).map_or("", String::as_str)
    }

    pub fn model(&self, index: u16) -> &str {
        self.models.get(index as usize).map_or("", String::as_str)
    }
}

/// Read a demo's bytes.
pub fn read(data: &[u8]) -> Result<CsDemo, String> {
    let mut mapper = CsMapper::default();
    let timeline = Timeline::read(data, &mut mapper)?;
    let mut sounds: Vec<String> = timeline
        .frames
        .iter()
        .flat_map(|f| &f.events)
        .filter_map(|e| match e {
            Happening::Sound { file, .. } | Happening::Voice { file, .. } => Some(file.clone()),
            _ => None,
        })
        .collect();
    sounds.sort();
    sounds.dedup();
    Ok(CsDemo {
        timeline,
        classes: mapper.classes,
        models: mapper.models,
        names: mapper.names,
        sounds,
        recorder: mapper.recorder,
        cvars: mapper.cvars,
    })
}

/// Read a demo file.
pub fn load(path: &std::path::Path) -> Result<CsDemo, String> {
    let data = std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
    read(&data).map_err(|e| format!("{}: {e}", path.display()))
}

// ---------------------------------------------------------------------------
// CS:S's classes and props

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum Kind {
    #[default]
    Other,
    Player,
    Grenade,
    Smoke,
    Bomb,
    Rules,
    Team,
    Resource,
    FireBullets,
    AnimEvent,
}

/// Prop paths per kind, in the order the mapper reads them.
const PLAYER_PROPS: &[&str] = &[
    "csnonlocaldata.m_vecOrigin",
    "m_angEyeAngles[0]",
    "m_angEyeAngles[1]",
    "m_iTeamNum",
    "m_iClass",
    "m_iHealth",
    "m_ArmorValue",
    "m_bHasHelmet",
    "m_fFlags",
    "m_lifeState",
    "m_hActiveWeapon",
    "m_iAccount",
    "m_bNightVisionOn",
    "m_bHasDefuser",
    // 14.. the player's own client's (`LocalSnap`).
    "cslocaldata.m_vecOrigin",
    "cslocaldata.m_vecOrigin[2]",
    "m_vecVelocity[0]",
    "m_vecVelocity[1]",
    "m_vecVelocity[2]",
    "m_bDucked",
    "m_bDucking",
    "m_nTickBase",
    "m_iShotsFired",
];
/// Where `PLAYER_PROPS`' own-client props start, and the arrays after
/// them: `m_iAmmo` then `m_hMyWeapons`.
const LOCAL_AT: usize = 14;
const AMMO_AT: usize = 23;
const AMMO_TYPES: usize = 32;
const MY_WEAPONS: usize = 48;
const ENTITY_PROPS: &[&str] = &["m_vecOrigin", "m_angRotation", "m_nModelIndex"];
const SMOKE_PROPS: &[&str] = &["m_vecOrigin", "m_flSpawnTime", "m_FadeStartTime", "m_FadeEndTime"];
const BOMB_PROPS: &[&str] = &[
    "m_vecOrigin",
    "m_angRotation",
    "m_bBombTicking",
    "m_flC4Blow",
    "m_flTimerLength",
];
const RULES_PROPS: &[&str] = &[
    "cs_gamerules_data.m_bFreezePeriod",
    "cs_gamerules_data.m_iRoundTime",
    "cs_gamerules_data.m_fRoundStartTime",
];
const TEAM_PROPS: &[&str] = &["m_iTeamNum", "m_iScore"];
const BULLETS_PROPS: &[&str] = &[
    "m_vecOrigin",
    "m_vecAngles[0]",
    "m_vecAngles[1]",
    "m_iMode",
    "m_iSeed",
    "m_iPlayer",
    "m_fInaccuracy",
    "m_fSpread",
];
const ANIM_PROPS: &[&str] = &["m_hPlayer", "m_iEvent"];
const WEAPON_PROPS: &[&str] = &["m_bSilencerOn", "m_bBurstMode", "m_iClip1", "m_iPrimaryAmmoType"];

/// CS:S's classes and props into `Thing`s and `Happening`s.
#[derive(Default)]
pub struct CsMapper {
    kinds: Vec<Kind>,
    props: Vec<ClassProps>,
    /// Weapons' silencer and burst props by class (looked up on use).
    weapon_props: HashMap<u16, ClassProps>,
    classes: Vec<String>,
    models: Vec<String>,
    names: HashMap<u32, String>,
    /// Grenades in the last frame (to tell where a deleted one was).
    grenades: HashMap<u32, GrenadeSnap>,
    over: Option<Option<u8>>,
    rounds: u32,
    recorder: Option<u32>,
    cvars: Vec<(String, String)>,
}

impl CsMapper {
    fn kind(&self, class: u16) -> Kind {
        self.kinds.get(class as usize).copied().unwrap_or_default()
    }

    fn at(&self, class: u16, i: usize) -> Option<usize> {
        self.props.get(class as usize)?.at(i)
    }

    fn player(&mut self, state: &DemoState, e: &DemEntity) -> PlayerSnap {
        let c = e.class;
        let props: Vec<Option<usize>> = (0..AMMO_AT + AMMO_TYPES + MY_WEAPONS).map(|i| self.at(c, i)).collect();
        let p = |i: usize| props[i];
        let weapon = e.handle(p(10)).and_then(|i| state.entity(i)).map(|w| (w.class, w));
        let (weapon_mode, clip, ammo_type) = match weapon {
            Some((class, w)) => {
                let props = self.weapon_props.entry(class).or_insert_with(|| {
                    ClassProps::new(state.flat.get(class as usize).map_or(&[], Vec::as_slice), WEAPON_PROPS)
                });
                let on = |i| w.int(props.at(i)).unwrap_or(0) != 0;
                (
                    u8::from(on(0) || on(1)),
                    w.int(props.at(2)).map(|v| v as i32),
                    w.int(props.at(3)),
                )
            }
            None => (0, None, None),
        };
        // A client demo's own player: its origin only in `cslocaldata`
        // (a vector, or its x and y with z apart).
        let local = e.value(p(LOCAL_AT)).and_then(|v| {
            let origin = match v {
                PropValue::Vector(v) => *v,
                PropValue::VectorXY([x, y]) => [*x, *y, e.float(p(LOCAL_AT + 1)).unwrap_or(0.0)],
                _ => return None,
            };
            let f = |i| e.float(p(i)).unwrap_or(0.0);
            Some(LocalSnap {
                origin,
                velocity: [f(LOCAL_AT + 2), f(LOCAL_AT + 3), f(LOCAL_AT + 4)],
                ducked: e.int(p(LOCAL_AT + 5)).unwrap_or(0) != 0,
                ducking: e.int(p(LOCAL_AT + 6)).unwrap_or(0) != 0,
                tick_base: e.int(p(LOCAL_AT + 7)).unwrap_or(0),
                shots_fired: e.int(p(LOCAL_AT + 8)).unwrap_or(0) as i32,
                clip,
                reserve: ammo_type
                    .filter(|t| (0..AMMO_TYPES as i64).contains(t))
                    .and_then(|t| e.int(p(AMMO_AT + t as usize)))
                    .map(|v| v as i32),
                weapons: (0..MY_WEAPONS)
                    .filter_map(|i| e.handle(p(AMMO_AT + AMMO_TYPES + i)))
                    .filter_map(|i| state.entity(i).map(|w| w.class))
                    .collect(),
            })
        });
        PlayerSnap {
            origin: e.vector(p(0)).or(local.as_ref().map(|l| l.origin)).unwrap_or_default(),
            pitch: e.float(p(1)).unwrap_or(0.0),
            yaw: e.float(p(2)).unwrap_or(0.0),
            team: e.int(p(3)).unwrap_or(0) as u8,
            class: e.int(p(4)).unwrap_or(0) as u8,
            health: e.int(p(5)).unwrap_or(0) as i32,
            armor: e.int(p(6)).unwrap_or(0) as i32,
            helmet: e.int(p(7)).unwrap_or(0) != 0,
            flags: e.int(p(8)).unwrap_or(0),
            alive: e.int(p(9)).unwrap_or(0) == 0 && e.int(p(5)).unwrap_or(0) > 0,
            weapon: weapon.map(|(class, _)| class),
            weapon_mode,
            money: e.int(p(11)).unwrap_or(0) as i32,
            night_vision: e.int(p(12)).unwrap_or(0) != 0,
            defuser: e.int(p(13)).unwrap_or(0) != 0,
            local,
        }
    }

    /// An entity index by a game event's `userid`.
    fn by_user(state: &DemoState, user: Option<&EventValue>) -> Option<u32> {
        let id = match user? {
            EventValue::Short(v) => *v as i32,
            EventValue::Long(v) => *v,
            EventValue::Byte(v) => *v as i32,
            _ => return None,
        };
        state.players().into_iter().find(|p| p.2 == id).map(|p| p.0)
    }

    fn game_event(&mut self, state: &DemoState, e: &GameEvent, out: &mut Vec<Happening>) {
        let text = |k: &str| match e.get(k) {
            Some(EventValue::String(s)) => s.clone(),
            _ => String::new(),
        };
        match e.name.as_str() {
            "player_death" => {
                let Some(victim) = Self::by_user(state, e.get("userid")) else {
                    return;
                };
                out.push(Happening::Death {
                    victim,
                    attacker: Self::by_user(state, e.get("attacker")).filter(|a| *a != victim),
                    weapon: text("weapon"),
                    headshot: matches!(e.get("headshot"), Some(EventValue::Bool(true))),
                });
            }
            "round_start" => {
                self.over = None;
                self.rounds += 1;
                out.push(Happening::RoundStart);
            }
            "round_end" => {
                let winner = match e.get("winner") {
                    Some(EventValue::Byte(w)) => Some(*w),
                    Some(EventValue::Short(w)) => Some(*w as u8),
                    Some(EventValue::Long(w)) => Some(*w as u8),
                    _ => None,
                }
                .filter(|w| *w == 2 || *w == 3);
                self.over = Some(winner);
                out.push(Happening::RoundEnd {
                    winner,
                    message: text("message"),
                });
            }
            _ => {}
        }
    }

    fn user_message(&self, m: &UserMessage, out: &mut Vec<Happening>) {
        let field = |k: &str| m.fields.iter().find(|(n, _)| n == k).map(|(_, v)| unquote(v));
        match m.name.as_str() {
            "SayText2" | "RadioText" => {
                let msg = field("msg").unwrap_or_default();
                let p = |i: usize| field(&format!("p{i}")).map(|s| words(&s)).unwrap_or_default();
                let line = match msg.as_str() {
                    "#Game_radio_location" => format!("{} @ {} (RADIO): {}", p(1), p(2), p(3)),
                    "#Game_radio" => format!("{} (RADIO): {}", p(1), p(2)),
                    m if m.starts_with("#Cstrike_Chat") => {
                        let team = match m {
                            "#Cstrike_Chat_CT" | "#Cstrike_Chat_CT_Loc" => "(Counter-Terrorist) ",
                            "#Cstrike_Chat_T" | "#Cstrike_Chat_T_Loc" => "(Terrorist) ",
                            _ if m.ends_with("Dead") => "*DEAD* ",
                            _ => "",
                        };
                        format!("{team}{} : {}", p(1), p(2))
                    }
                    other => words(other),
                };
                out.push(Happening::Text(line));
            }
            "SayText" => {
                if let Some(t) = field("text") {
                    out.push(Happening::Text(t));
                }
            }
            "SendAudio" => {
                if let Some(t) = field("text") {
                    out.push(Happening::Audio(t));
                }
            }
            "RawAudio" => {
                if let (Some(entity), Some(file)) = (field("entity").and_then(|e| e.parse().ok()), field("file")) {
                    out.push(Happening::Voice {
                        entity,
                        file: file.replace('\\', "/").to_lowercase(),
                    });
                }
            }
            _ => {}
        }
    }
}

/// A decoded user message field's text (strings come quoted).
fn unquote(v: &str) -> String {
    v.strip_prefix('"')
        .and_then(|s| s.strip_suffix('"'))
        .map_or_else(|| v.to_string(), |s| s.replace("\\\"", "\"").replace("\\\\", "\\"))
}

/// A localization token as words (`#Cstrike_TitlesTXT_Fire_in_the_hole`:
/// "Fire in the hole"); plain text stays.
fn words(token: &str) -> String {
    match token.strip_prefix('#') {
        Some(t) => t
            .trim_start_matches("Cstrike_TitlesTXT_")
            .trim_start_matches("Cstrike_")
            .replace('_', " "),
        None => token.to_string(),
    }
}

/// A sound's file under `sound/`: no prefix characters, forward slashes,
/// lower case.
pub fn sound_file(name: &str) -> String {
    super::sound::wave_file(name)
        .trim_start_matches("sound/")
        .to_lowercase()
}

impl Mapper for CsMapper {
    type Entity = Thing;
    type Event = Happening;

    fn data_tables(&mut self, state: &DemoState) {
        self.classes = state.classes.iter().map(|c| c.name.clone()).collect();
        self.kinds = self
            .classes
            .iter()
            .map(|n| match n.as_str() {
                "CCSPlayer" => Kind::Player,
                "ParticleSmokeGrenade" => Kind::Smoke,
                "CPlantedC4" => Kind::Bomb,
                "CCSGameRulesProxy" => Kind::Rules,
                "CCSTeam" => Kind::Team,
                "CCSPlayerResource" => Kind::Resource,
                "CTEFireBullets" => Kind::FireBullets,
                "CTEPlayerAnimEvent" => Kind::AnimEvent,
                n if n.contains("Grenade") && n.ends_with("Projectile") => Kind::Grenade,
                _ => Kind::Other,
            })
            .collect();
        let resource_paths: Vec<String> = ["m_iScore", "m_iDeaths"]
            .iter()
            .flat_map(|p| (0..MAX_PLAYERS).map(move |i| format!("{p}.{i:03}")))
            .collect();
        let resource_paths: Vec<&str> = resource_paths.iter().map(String::as_str).collect();
        let player_arrays: Vec<String> = (0..AMMO_TYPES)
            .map(|i| format!("m_iAmmo.{i:03}"))
            .chain((0..MY_WEAPONS).map(|i| format!("m_hMyWeapons.{i:03}")))
            .collect();
        let player_paths: Vec<&str> = PLAYER_PROPS
            .iter()
            .copied()
            .chain(player_arrays.iter().map(String::as_str))
            .collect();
        self.props = self
            .kinds
            .iter()
            .zip(&state.flat)
            .map(|(k, flat)| {
                let paths = match k {
                    Kind::Player => player_paths.as_slice(),
                    Kind::Grenade => ENTITY_PROPS,
                    Kind::Smoke => SMOKE_PROPS,
                    Kind::Bomb => BOMB_PROPS,
                    Kind::Rules => RULES_PROPS,
                    Kind::Team => TEAM_PROPS,
                    Kind::Resource => resource_paths.as_slice(),
                    Kind::FireBullets => BULLETS_PROPS,
                    Kind::AnimEvent => ANIM_PROPS,
                    Kind::Other => return ClassProps::default(),
                };
                ClassProps::new(flat, paths)
            })
            .collect();
        self.weapon_props.clear();
    }

    fn entity(&mut self, state: &DemoState, index: u32, e: &DemEntity) -> Option<Thing> {
        let c = e.class;
        let p = |i| self.at(c, i);
        Some(match self.kind(c) {
            Kind::Player => Thing::Player(self.player(state, e)),
            Kind::Grenade => {
                let g = GrenadeSnap {
                    origin: e.vector(p(0))?,
                    angles: e.vector(p(1)).unwrap_or_default(),
                    model: e.int(p(2)).map(|m| m as u16),
                };
                self.grenades.insert(index, g.clone());
                Thing::Grenade(g)
            }
            Kind::Smoke => Thing::Smoke(SmokeSnap {
                origin: e.vector(p(0))?,
                spawn_time: e.float(p(1)).unwrap_or(0.0),
                fade_start: e.float(p(2)).unwrap_or(0.0),
                fade_end: e.float(p(3)).unwrap_or(0.0),
            }),
            Kind::Bomb => Thing::Bomb(BombSnap {
                origin: e.vector(p(0))?,
                angles: e.vector(p(1)).unwrap_or_default(),
                ticking: e.int(p(2)).unwrap_or(0) != 0,
                blow: e.float(p(3)).unwrap_or(0.0),
                timer: e.float(p(4)).unwrap_or(0.0),
            }),
            Kind::Rules => Thing::Rules(RulesSnap {
                freeze: e.int(p(0)).unwrap_or(0) != 0,
                round_time: e.int(p(1)).unwrap_or(0) as i32,
                round_start: e.float(p(2)).unwrap_or(0.0),
                over: self.over,
                rounds: self.rounds,
            }),
            Kind::Team => Thing::Team(TeamSnap {
                team: e.int(p(0))? as u8,
                score: e.int(p(1)).unwrap_or(0) as i32,
            }),
            Kind::Resource => Thing::Scores(
                (0..MAX_PLAYERS)
                    .map(|i| {
                        (
                            e.int(p(i)).unwrap_or(0) as i32,
                            e.int(p(MAX_PLAYERS + i)).unwrap_or(0) as i32,
                        )
                    })
                    .collect(),
            ),
            _ => return None,
        })
    }

    fn record(&mut self, state: &DemoState, record: &Record, out: &mut Vec<Happening>) {
        match record {
            Record::StringTable { table, .. } => {
                let Some(t) = state.string_tables.get(*table) else {
                    return;
                };
                match t.name.as_str() {
                    "modelprecache" => self.models = t.entries.iter().map(|e| e.string.clone()).collect(),
                    "userinfo" => {
                        for (index, name, ..) in state.players() {
                            self.names.insert(index, name);
                        }
                    }
                    _ => {}
                }
            }
            Record::Entities(changes) => {
                for i in playback::deleted(changes) {
                    if let Some(g) = self.grenades.remove(&i) {
                        out.push(Happening::Detonate {
                            origin: g.origin,
                            model: g.model,
                        });
                    }
                }
                // An index taken by something else: not a grenade now.
                for c in changes {
                    if state
                        .entity(c.index)
                        .is_some_and(|e| self.kind(e.class) != Kind::Grenade)
                    {
                        self.grenades.remove(&c.index);
                    }
                }
            }
            Record::TempEntities(list) => {
                let mut last_player = None;
                for te in list {
                    let c = te.class;
                    let p = |i| self.at(c, i);
                    let props = te.props.as_slice();
                    match self.kind(c) {
                        Kind::FireBullets => out.push(Happening::Shot {
                            player: props.int(p(5)).unwrap_or(0) as u32 + 1,
                            origin: props.vector(p(0)).unwrap_or_default(),
                            pitch: props.float(p(1)).unwrap_or(0.0),
                            yaw: props.float(p(2)).unwrap_or(0.0),
                            mode: props.int(p(3)).unwrap_or(0) as u8,
                            seed: props.int(p(4)).unwrap_or(0) as u32,
                            inaccuracy: props.float(p(6)).unwrap_or(0.0),
                            spread: props.float(p(7)).unwrap_or(0.0),
                        }),
                        Kind::AnimEvent => {
                            // An event without its player is the one
                            // before's (sent as a delta from it).
                            let player = props.handle(p(0)).or(last_player);
                            last_player = player;
                            if let Some(player) = player {
                                out.push(Happening::Anim {
                                    player,
                                    event: props.int(p(1)).unwrap_or(0),
                                });
                            }
                        }
                        _ => {}
                    }
                }
            }
            Record::Sounds(sounds) => {
                for s in sounds {
                    if s.flags & SND_STOP != 0 {
                        continue;
                    }
                    let Some(name) = state.table_string("soundprecache", s.sound_index as usize) else {
                        continue;
                    };
                    out.push(Happening::Sound {
                        entity: s.entity,
                        file: sound_file(name),
                        origin: s.origin,
                        volume: s.volume,
                        pitch: s.pitch,
                    });
                }
            }
            Record::GameEvent(e) => self.game_event(state, e, out),
            Record::UserMessage(m) => self.user_message(m, out),
            Record::UserCmd { cmd, .. } => out.push(Happening::Command {
                cmd: *cmd,
                weapon: (cmd.weapon_select != 0)
                    .then(|| state.entity(cmd.weapon_select).map(|w| w.class))
                    .flatten(),
            }),
            Record::ServerInfo(info) => {
                self.recorder = (!info.hltv).then_some(info.player_slot as u32 + 1);
            }
            Record::SetConVar(vars) => self.cvars.extend(vars.iter().cloned()),
            _ => {}
        }
    }
}

// ---------------------------------------------------------------------------
// Source to ours

/// A Source position (units, Z up) in engine space.
pub fn engine(v: [f32; 3]) -> Vec3 {
    super::movement::to_engine(Vec3::from_array(v))
}

/// A player's feet to its character's origin (engine space).
pub fn character_origin(feet: [f32; 3]) -> Vec3 {
    engine(feet) + Vec3::Y * ORIGIN_ABOVE_FEET * UNIT
}

/// Source eye angles (degrees: pitch down +, yaw from +X to the left) as
/// our look: yaw (radians, 0 down -Z, the same turn) and pitch (up +).
pub fn look(pitch: f32, yaw: f32) -> (f32, f32) {
    let pitch = (pitch + 180.0).rem_euclid(360.0) - 180.0;
    (
        (yaw - 90.0).to_radians().rem_euclid(std::f32::consts::TAU),
        -pitch.to_radians(),
    )
}

/// The eye above our character's origin for `flags` (ducked or not).
pub fn eye_offset(flags: i64) -> Vec3 {
    let eye = if flags & FL_DUCKING != 0 {
        EYE_DUCKED
    } else {
        EYE_STANDING
    };
    Vec3::Y * (eye - ORIGIN_ABOVE_FEET) * UNIT
}

/// Our team for Source's team number.
pub fn team(source: u8) -> Option<Team> {
    match source {
        2 => Some(ATTACKERS),
        3 => Some(DEFENDERS),
        _ => None,
    }
}

/// Our weapon id's name part for a weapon's server class (`CAK47`,
/// `CWeaponM4A1`, `CKnife`: `weapon_ak47`, ...).
pub fn weapon_name(class: &str) -> Option<String> {
    let base = class.strip_prefix("CWeapon").or_else(|| class.strip_prefix('C'))?;
    (!base.is_empty()).then(|| format!("weapon_{}", base.to_lowercase()))
}

/// The grenade a thrown grenade's model is (`w_eq_flashbang_thrown.mdl`).
pub fn grenade_name(model: &str) -> Option<&'static str> {
    let m = model.to_lowercase();
    if m.contains("flashbang") {
        Some("weapon_flashbang")
    } else if m.contains("smoke") {
        Some("weapon_smokegrenade")
    } else if m.contains("frag") || m.contains("hegrenade") {
        Some("weapon_hegrenade")
    } else {
        None
    }
}

fn lerp_angle(a: f32, b: f32, f: f32) -> f32 {
    let d = (b - a + 180.0).rem_euclid(360.0) - 180.0;
    a + d * f
}

// ---------------------------------------------------------------------------
// Playing

/// A character the demo moves: the player with this entity index.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub struct DemoPuppet(pub u32);

/// Something else the demo shows (a grenade, a smoke cloud, the bomb):
/// its entity index.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub struct DemoThing(pub u32);

/// A line of chat or radio from the demo, for the chat area (the front
/// end shows it).
#[derive(Message, Clone, Debug, PartialEq)]
pub struct DemoText(pub String);

/// The demo's own clock, for what ages with it (the HUD's death notices):
/// seconds of demo shown (standing still while paused) and how many jumps
/// (`seek`, the start counts) it has made. Present while a demo plays.
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq)]
pub struct DemoClock {
    pub seconds: f64,
    pub seeks: u32,
}

/// The demo playing (`start`), until `stop`.
#[derive(Resource)]
pub struct DemoPlayer {
    pub demo: Arc<CsDemo>,
    /// The demo tick shown now (fractional between frames).
    pub tick: f64,
    /// The last frame whose events were played.
    played: Option<usize>,
    /// Jump to `tick` without playing what lies between.
    seek: bool,
    pub puppets: HashMap<u32, Entity>,
    pub things: HashMap<u32, Entity>,
    /// Our weapon id per server class (looked up once).
    weapons: HashMap<u16, Option<&'static str>>,
    /// What each puppet was last given to hold.
    held: HashMap<u32, (Option<&'static str>, u8)>,
    /// Grenades' rules, from weapons built once.
    templates: HashMap<&'static str, Entity>,
    /// The planted bomb's next beep (our clock).
    next_beep: f64,
    /// The last tick has been shown.
    pub ended: bool,
}

impl DemoPlayer {
    pub fn timeline(&self) -> &Timeline<Thing, Happening> {
        &self.demo.timeline
    }
}

pub struct DemoPlugin;

impl Plugin for DemoPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<DemoText>()
            .add_message::<Died>()
            .add_message::<RoundEnded>()
            .add_message::<Detonated>()
            .add_systems(
                RunFixedMainLoop,
                play.in_set(RunFixedMainLoopSystems::AfterFixedMainLoop)
                    .run_if(resource_exists::<DemoPlayer>),
            )
            .console_command(
                "demo_timescale",
                "demo_timescale <scale>: play the demo this many times faster (0.5 half speed).",
                |w, a| {
                    let s: f32 = a
                        .first()
                        .and_then(|v| v.parse().ok())
                        .filter(|s: &f32| *s > 0.0 && *s <= 100.0)
                        .ok_or("demo_timescale <scale>: a number above 0")?;
                    w.resource_mut::<Time<Virtual>>().set_relative_speed(s);
                    Ok(None)
                },
            )
            .console_command("demo_pause", "Pause the demo playing.", |w, _| {
                playing(w)?;
                w.resource_mut::<Time<Virtual>>().pause();
                Ok(Some("demo paused".into()))
            })
            .console_command("demo_resume", "Resume the paused demo.", |w, _| {
                playing(w)?;
                w.resource_mut::<Time<Virtual>>().unpause();
                Ok(None)
            })
            .console_command("demo_togglepause", "Pause or resume the demo.", |w, _| {
                playing(w)?;
                let mut t = w.resource_mut::<Time<Virtual>>();
                if t.is_paused() {
                    t.unpause();
                } else {
                    t.pause();
                }
                Ok(None)
            })
            .console_command(
                "demo_gototick",
                "demo_gototick <tick> [relative]: jump to a demo tick (forward or back); \
                 with a second argument 1, that many ticks from now.",
                |w, a| {
                    let n: f64 = a.first().and_then(|v| v.parse().ok()).ok_or("demo_gototick <tick>")?;
                    let relative = a.get(1).is_some_and(|r| r != "0");
                    let now = playing(w)?;
                    seek(w, if relative { now + n } else { n });
                    Ok(None)
                },
            )
            .console_command("stopdemo", "Stop the demo playing.", |w, _| {
                playing(w)?;
                stop(w);
                Ok(Some("demo stopped".into()))
            })
            .console_command("demo_info", "The demo playing: file's map, tick, length.", |w, _| {
                let p = w.get_resource::<DemoPlayer>().ok_or("no demo playing")?;
                let t = p.timeline();
                let (first, last) = t.ticks();
                Ok(Some(format!(
                    "{} on {}: tick {:.0} of {first}..{last} ({:.1} of {:.1} s), {} frames, {:.0} ticks/s{}",
                    t.header.server_name,
                    t.map,
                    p.tick,
                    t.seconds(p.tick - first as f64),
                    t.seconds((last - first) as f64),
                    t.frames.len(),
                    1.0 / t.tick_interval,
                    if p.ended { ", ended" } else { "" },
                )))
            });
    }
}

/// The tick playing now, or an error without a demo.
fn playing(w: &World) -> Result<f64, String> {
    w.get_resource::<DemoPlayer>()
        .map(|p| p.tick)
        .ok_or_else(|| "no demo playing".to_string())
}

/// Play `demo` on the map loaded now (its own; the caller loads it):
/// nothing simulates (`core::Playback`), everyone but the local player
/// goes, the local player watches (`core::Spectating`), and the demo's
/// players come in as puppets from its first frame.
pub fn start(world: &mut World, demo: Arc<CsDemo>) {
    stop(world);
    world.insert_resource(Playback);
    // Everyone else goes (bots, a previous game's players).
    let others: Vec<Entity> = world
        .query_filtered::<Entity, (With<Intent>, Without<LocalPlayer>)>()
        .iter(world)
        .collect();
    for e in others {
        despawn_character(world, e);
    }
    let me: Option<Entity> = world.query_filtered::<Entity, With<LocalPlayer>>().iter(world).next();
    if let Some(me) = me {
        let weapons = world
            .get::<Inventory>(me)
            .map(|i| i.weapons.clone())
            .unwrap_or_default();
        for w in weapons {
            if let Ok(e) = world.get_entity_mut(w) {
                e.despawn();
            }
        }
        let since = world.resource::<Time>().elapsed_secs_f64();
        let mut e = world.entity_mut(me);
        e.insert((
            Spectating,
            Team(0),
            Dead { since },
            ColliderDisabled,
            Inventory::default(),
        ));
        if let Some(mut h) = e.get_mut::<Health>() {
            h.current = 0.0;
        }
        if let Some(mut v) = e.get_mut::<Velocity>() {
            v.0 = Vec3::ZERO;
        }
    }
    world.insert_resource(RoundState::default());
    world.insert_resource(BombState::default());
    add_sounds(world, &demo.sounds);
    let first = demo.timeline.ticks().0 as f64;
    info!(
        "playing demo of {} ({} frames, {:.1} s)",
        demo.map(),
        demo.timeline.frames.len(),
        demo.timeline
            .seconds((demo.timeline.ticks().1 - demo.timeline.ticks().0) as f64)
    );
    world.insert_resource(DemoPlayer {
        demo,
        tick: first,
        played: None,
        seek: true,
        puppets: HashMap::new(),
        things: HashMap::new(),
        weapons: HashMap::new(),
        held: HashMap::new(),
        templates: HashMap::new(),
        next_beep: 0.0,
        ended: false,
    });
}

/// Stop the demo playing: its puppets and things go, the simulation is
/// back (the local player stays a spectator until it joins a team).
pub fn stop(world: &mut World) {
    let Some(p) = world.remove_resource::<DemoPlayer>() else {
        world.remove_resource::<Playback>();
        return;
    };
    for e in p.puppets.values().copied().collect::<Vec<_>>() {
        despawn_character(world, e);
    }
    for e in p.things.values().chain(p.templates.values()) {
        if let Ok(e) = world.get_entity_mut(*e) {
            e.despawn();
        }
    }
    world.remove_resource::<Playback>();
    world.remove_resource::<DemoClock>();
    world.insert_resource(BombState::default());
    let mut t = world.resource_mut::<Time<Virtual>>();
    t.unpause();
    t.set_relative_speed(1.0);
}

/// Show demo tick `tick` next (clamped to the demo), without playing what
/// lies between.
pub fn seek(world: &mut World, tick: f64) {
    if let Some(mut p) = world.get_resource_mut::<DemoPlayer>() {
        let (first, last) = p.demo.timeline.ticks();
        p.tick = tick.clamp(first as f64, last as f64);
        p.seek = true;
        p.ended = false;
    }
}

fn despawn_character(world: &mut World, e: Entity) {
    let weapons = world.get::<Inventory>(e).map(|i| i.weapons.clone()).unwrap_or_default();
    for w in weapons {
        if let Ok(w) = world.get_entity_mut(w) {
            w.despawn();
        }
    }
    if let Ok(e) = world.get_entity_mut(e) {
        e.despawn();
    }
}

/// The demo's waves as sound entries by file (`Happening::Sound`), read
/// from the install and added to the map's bank (a copy).
fn add_sounds(world: &mut World, files: &[String]) {
    let Some(bank) = world.get_resource::<SoundBank>() else {
        return;
    };
    let missing: Vec<&String> = files.iter().filter(|f| bank.0.entry(f).is_none()).collect();
    if missing.is_empty() {
        return;
    }
    let Some(mount) = super::INFO
        .install()
        .and_then(|p| super::mount::open(&p).ok())
    else {
        return;
    };
    let mut sounds = (*bank.0).clone();
    let mut added = 0;
    for f in missing {
        let Some(clip) = mount
            .read(&format!("sound/{f}"))
            .ok()
            .and_then(|b| super::wav::decode_any(&b).ok())
        else {
            continue;
        };
        sounds.clips.push(clip);
        sounds.entries.insert(
            f.clone(),
            MapSoundEntry {
                waves: vec![sounds.clips.len() - 1],
                volume: Interval::fixed(1.0),
                pitch: Interval::fixed(100.0),
                level: SoundLevel::Db(75.0),
                channel: 6,
                dry: false,
            },
        );
        added += 1;
    }
    if added > 0 {
        world.insert_resource(SoundBank(Arc::new(sounds)));
    }
}

/// Our weapon id for a server class (cached).
fn weapon_of(world: &World, p: &mut DemoPlayer, class: u16) -> Option<&'static str> {
    *p.weapons.entry(class).or_insert_with(|| {
        let name = weapon_name(p.demo.class(class))?;
        world.resource::<WeaponRegistry>().find(&name).map(|d| d.id)
    })
}

/// A grenade's effect (its weapon's rule), from a weapon built once.
fn grenade_effect(world: &mut World, p: &mut DemoPlayer, id: &'static str) -> Option<GrenadeEffect> {
    let e = match p.templates.get(id) {
        Some(e) => *e,
        None => {
            let build = world.resource::<WeaponRegistry>().find(id)?.build.clone();
            let mut e = world.spawn(Name::new(format!("Demo template {id}")));
            build(&mut e);
            let e = e.id();
            p.templates.insert(id, e);
            e
        }
    };
    world.get::<Throwable>(e).map(|t| t.effect.clone())
}

fn play(world: &mut World) {
    let Some(mut p) = world.remove_resource::<DemoPlayer>() else {
        return;
    };
    let demo = p.demo.clone();
    let t = &demo.timeline;
    let interval = t.tick_interval.max(1e-4) as f64;
    let (first, last) = t.ticks();
    if !p.seek && !p.ended {
        p.tick += world.resource::<Time>().delta_secs_f64() / interval;
    }
    if p.tick >= last as f64 {
        p.tick = last as f64;
        if !p.ended {
            p.ended = true;
            info!("demo ended at tick {last}");
        }
    }
    p.tick = p.tick.max(first as f64);
    let Some(frame) = t.frame_at(p.tick) else {
        world.insert_resource(p);
        return;
    };
    let seeking = p.seek;
    let mut clock = world.get_resource::<DemoClock>().copied().unwrap_or_default();
    clock.seconds = (p.tick - first as f64) * interval;
    clock.seeks += u32::from(seeking);
    if world.get_resource::<DemoClock>() != Some(&clock) {
        world.insert_resource(clock);
    }
    if seeking {
        // A jump: rebuild what lasts from the frame (smoke clouds started
        // at their age, the bomb on its clock); nothing in between plays.
        for e in p.things.drain().map(|(_, e)| e).collect::<Vec<_>>() {
            if let Ok(e) = world.get_entity_mut(e) {
                e.despawn();
            }
        }
        p.played = Some(frame);
    }
    apply_players(world, &mut p, seeking);
    let from = p.played.map_or(0, |i| i + 1);
    for i in from..=frame {
        for h in &t.frames[i].events {
            happen(world, &mut p, h);
        }
    }
    p.played = Some(frame);
    apply_things(world, &mut p);
    apply_round(world, &p);
    p.seek = false;
    world.insert_resource(p);
}

/// Every player on a team as a puppet where the demo has it now.
fn apply_players(world: &mut World, p: &mut DemoPlayer, seeking: bool) {
    let demo = p.demo.clone();
    let t = &demo.timeline;
    let Some(frame) = t.frame_at(p.tick) else { return };
    let now = world.resource::<Time>().elapsed_secs_f64();
    let fixed_now = world.resource::<Time<Fixed>>().elapsed_secs_f64();
    let scores = t.frames[frame].entities.iter().find_map(|(_, e)| match e {
        Thing::Scores(s) => Some(s.clone()),
        _ => None,
    });
    let mut seen = Vec::new();
    for (index, thing) in &t.frames[frame].entities {
        let Thing::Player(_) = thing else { continue };
        let Some(b) = t.between(*index, p.tick) else { continue };
        let (Thing::Player(a), Thing::Player(z)) = (b.from, b.to) else {
            continue;
        };
        let Some(team) = team(a.team) else { continue };
        seen.push(*index);
        let dt = (b.ticks.1 - b.ticks.0) as f32 * t.tick_interval;
        let (pa, pz) = (engine(a.origin), engine(z.origin));
        let teleport = dt <= 0.0 || pa.distance(pz) > SNAP_SPEED * dt;
        let f = if teleport { 0.0 } else { b.fraction };
        let origin = character_origin(a.origin).lerp(character_origin(z.origin), f);
        let velocity = if teleport { Vec3::ZERO } else { (pz - pa) / dt };
        let (yaw, pitch) = look(lerp_angle(a.pitch, z.pitch, f), lerp_angle(a.yaw, z.yaw, f));
        let e = match p.puppets.get(index).copied().filter(|e| world.get_entity(*e).is_ok()) {
            Some(e) => e,
            None => {
                let name = demo
                    .names
                    .get(index)
                    .cloned()
                    .unwrap_or_else(|| format!("Player {index}"));
                let e = world
                    .spawn(character_bundle(Transform::from_translation(origin), team))
                    .insert((
                        Name::new(name),
                        NetDrawn,
                        DemoPuppet(*index),
                        RenderedView::default(),
                        PlayerClass(a.class),
                        Money(a.money.max(0) as u32),
                        Armor::default(),
                        Score::default(),
                    ))
                    .id();
                p.puppets.insert(*index, e);
                p.held.remove(index);
                e
            }
        };
        let alive = a.alive;
        let was_dead = world.get::<Dead>(e).is_some();
        let mut ent = world.entity_mut(e);
        if ent.get::<Team>() != Some(&team) {
            ent.insert(team);
        }
        if ent.get::<PlayerClass>().map(|c| c.0) != Some(a.class) {
            ent.insert(PlayerClass(a.class));
        }
        let health = (a.health.max(0) as f32) * 0.01;
        if let Some(mut h) = ent.get_mut::<Health>()
            && h.current != health
        {
            h.current = health;
        }
        let armor = Armor {
            amount: a.armor.max(0) as f32 * 0.01,
            helmet: a.helmet,
        };
        if ent.get::<Armor>() != Some(&armor) {
            ent.insert(armor);
        }
        let money = Money(a.money.max(0) as u32);
        if ent.get::<Money>() != Some(&money) {
            ent.insert(money);
        }
        if let Some((kills, deaths)) = scores.as_ref().and_then(|s| s.get(*index as usize).copied())
            && let Some(mut s) = ent.get_mut::<Score>()
            && (s.kills != kills || s.deaths != deaths.max(0) as u32)
        {
            s.kills = kills;
            s.deaths = deaths.max(0) as u32;
        }
        match (alive, was_dead) {
            (false, false) => {
                ent.insert(Dead { since: fixed_now });
            }
            (true, true) => {
                ent.remove::<Dead>();
            }
            _ => {}
        }
        // The dead lie where they fell (their recorded origin follows their
        // spectator camera); a jump places them.
        if alive || seeking || !was_dead {
            let crouch = a.flags & FL_DUCKING != 0;
            let eye = eye_offset(a.flags);
            if let Some(mut tf) = ent.get_mut::<Transform>()
                && tf.translation != origin
            {
                tf.translation = origin;
            }
            if let Some(mut v) = ent.get_mut::<Velocity>() {
                v.0 = velocity;
            }
            if let Some(mut s) = ent.get_mut::<MovementState>() {
                let ground = a.flags & FL_ONGROUND != 0;
                if s.on_ground != ground || s.crouching != crouch || s.eye_offset != eye {
                    s.on_ground = ground;
                    s.crouching = crouch;
                    s.eye_offset = eye;
                }
            }
            if let Some(mut i) = ent.get_mut::<Intent>()
                && (i.yaw != yaw || i.pitch != pitch || i.crouch != crouch)
            {
                i.yaw = yaw;
                i.pitch = pitch;
                i.crouch = crouch;
            }
            if let Some(mut view) = ent.get_mut::<RenderedView>() {
                view.now = EyeView {
                    eye_offset: eye,
                    view_roll: 0.0,
                    yaw,
                    pitch,
                    punch: Vec3::ZERO,
                };
            }
        }
        // What it holds.
        let weapon = if alive {
            a.weapon.and_then(|c| weapon_of(world, p, c))
        } else {
            None
        };
        let held = (weapon, a.weapon_mode);
        if p.held.get(index) != Some(&held) {
            p.held.insert(*index, held);
            show_held(world, e, weapon, a.weapon_mode, false);
        }
        let _ = now;
    }
    let gone: Vec<u32> = p.puppets.keys().copied().filter(|i| !seen.contains(i)).collect();
    for i in gone {
        if let Some(e) = p.puppets.remove(&i) {
            despawn_character(world, e);
        }
        p.held.remove(&i);
    }
}

/// One thing that happened, played now.
fn happen(world: &mut World, p: &mut DemoPlayer, h: &Happening) {
    let puppet = |i: u32| p.puppets.get(&i).copied();
    match h {
        Happening::Shot {
            player,
            origin,
            pitch,
            yaw,
            mode,
            seed,
            inaccuracy,
            spread,
        } => {
            let Some(shooter) = puppet(*player) else { return };
            let Some(active) = world.get::<Inventory>(shooter).and_then(|i| i.active) else {
                return;
            };
            let Some(weapon) = world.get::<Weapon>(active).map(|w| w.id) else {
                return;
            };
            let disc = world
                .get::<Hitscan>(active)
                .is_some_and(|h| matches!(h.spread, SpreadShape::Disc { .. }));
            let (yaw, pitch) = look(*pitch, *yaw);
            world.write_message(RemoteShot {
                shooter,
                weapon,
                origin: engine(*origin),
                yaw,
                pitch,
                seed: *seed,
                spread: if disc {
                    SpreadShape::Disc {
                        inaccuracy: *inaccuracy,
                        spread: *spread,
                    }
                } else {
                    SpreadShape::Template(*inaccuracy)
                },
                mode: *mode,
                flags: 0,
            });
        }
        Happening::Anim { player, event } => {
            let Some(shooter) = puppet(*player) else { return };
            let kind = match event {
                4 | 5 => WeaponEventKind::ReloadStarted,
                6 => WeaponEventKind::ShellInserting,
                7 => WeaponEventKind::Reloaded,
                _ => return,
            };
            world.write_message(RemoteAction { shooter, kind });
        }
        Happening::Sound {
            entity,
            file,
            origin,
            volume,
            pitch,
        } => {
            let source = puppet(*entity);
            let at = if *origin == [0.0; 3] {
                source.and_then(|s| world.get::<Transform>(s)).map(|t| t.translation)
            } else {
                Some(engine(*origin))
            };
            world.write_message(PlaySound {
                entry: file.clone(),
                at,
                volume: Some(*volume),
                pitch: Some(*pitch as f32),
                source,
                channel: None,
            });
        }
        Happening::Voice { entity, file } => {
            let source = puppet(*entity);
            let at = source.and_then(|s| world.get::<Transform>(s)).map(|t| t.translation);
            world.write_message(PlaySound {
                entry: file.clone(),
                at,
                volume: None,
                pitch: None,
                source,
                channel: None,
            });
        }
        Happening::Audio(entry) => {
            world.write_message(PlaySound::ui(entry.clone()));
        }
        Happening::Text(line) => {
            world.write_message(DemoText(line.clone()));
        }
        Happening::Death {
            victim,
            attacker,
            weapon,
            headshot,
        } => {
            let Some(v) = puppet(*victim) else { return };
            let a = attacker.and_then(puppet);
            let weapon = world
                .resource::<WeaponRegistry>()
                .find(&format!("weapon_{weapon}"))
                .map(|d| d.id);
            let at = world.get::<Transform>(v).map_or(Vec3::ZERO, |t| t.translation);
            let from = a.and_then(|a| world.get::<Transform>(a)).map_or(at, |t| t.translation);
            let dir = (at - from).normalize_or_zero();
            if world.get::<Dead>(v).is_none() {
                let since = world.resource::<Time<Fixed>>().elapsed_secs_f64();
                world.entity_mut(v).insert(Dead { since });
            }
            world.write_message(Died {
                entity: v,
                attacker: a,
                damage: Damage {
                    target: v,
                    attacker: a,
                    amount: 0.0,
                    point: at,
                    dir,
                    hitgroup: if *headshot { Hitgroup::Head } else { Hitgroup::default() },
                    weapon,
                    kind: Default::default(),
                    force: dir * 30.0,
                },
            });
        }
        Happening::RoundEnd { winner, message } => {
            let winner = winner.and_then(team);
            let reason = match message.as_str() {
                "#Target_Bombed" => RoundEndReason::TargetBombed,
                "#Bomb_Defused" => RoundEndReason::BombDefused,
                "#Target_Saved" => RoundEndReason::TargetSaved,
                "#All_Hostages_Rescued" => RoundEndReason::HostagesRescued,
                "#Hostages_Not_Rescued" => RoundEndReason::HostagesNotRescued,
                "#Round_Draw" => RoundEndReason::Draw,
                _ => RoundEndReason::Eliminated,
            };
            world.write_message(RoundEnded {
                winner,
                reason,
                summary: RoundSummary::default(),
            });
        }
        Happening::RoundStart | Happening::Command { .. } => {}
        Happening::Detonate { origin, model } => {
            let Some(name) = model.and_then(|m| grenade_name(p.demo.model(m))) else {
                return;
            };
            let Some(id) = world.resource::<WeaponRegistry>().find(name).map(|d| d.id) else {
                return;
            };
            let at = engine(*origin);
            let (kind, sound, shake) = match grenade_effect(world, p, id) {
                Some(GrenadeEffect::Blast(b)) => (GrenadeKind::Blast, b.sound, b.shake),
                Some(GrenadeEffect::Flash(f)) => (GrenadeKind::Flash, f.sound, None),
                // A smoke grenade lies on in its cloud (the cloud is its own
                // entity).
                _ => return,
            };
            world.write_message(Detonated {
                kind,
                weapon: id,
                thrower: None,
                at,
                ground: None,
                projectile: Entity::PLACEHOLDER,
                shake,
                flags: Default::default(),
            });
            if let Some(s) = sound {
                world.write_message(PlaySound::at(s, at));
            }
        }
    }
}

/// Grenades in flight, smoke clouds and the planted bomb as the frame has
/// them.
fn apply_things(world: &mut World, p: &mut DemoPlayer) {
    let demo = p.demo.clone();
    let t = &demo.timeline;
    let Some(frame) = t.frame_at(p.tick) else { return };
    let now = world.resource::<Time>().elapsed_secs_f64();
    let server_now = t.server_time(p.tick);
    // Server time `s` on our clock.
    let local = |s: f32| now + (s as f64 - server_now);
    let mut seen = Vec::new();
    for (index, thing) in &t.frames[frame].entities {
        let existing = p.things.get(index).copied().filter(|e| world.get_entity(*e).is_ok());
        match thing {
            Thing::Grenade(_) => {
                let Some(b) = t.between(*index, p.tick) else { continue };
                let (Thing::Grenade(a), Thing::Grenade(z)) = (b.from, b.to) else {
                    continue;
                };
                let Some(id) = a
                    .model
                    .and_then(|m| grenade_name(demo.model(m)))
                    .and_then(|n| world.resource::<WeaponRegistry>().find(n).map(|d| d.id))
                else {
                    continue;
                };
                seen.push(*index);
                let at = engine(a.origin).lerp(engine(z.origin), b.fraction);
                let turn = rotation_to_engine(entity_rotation(Vec3::from_array(a.angles)));
                let pose = Transform::from_translation(at).with_rotation(turn);
                match existing {
                    Some(e) => {
                        if let Some(mut tf) = world.get_mut::<Transform>(e) {
                            *tf = pose;
                        }
                    }
                    None => {
                        let e = world
                            .spawn((
                                Name::new(format!("Demo grenade {id}")),
                                ShownItem(id.to_string()),
                                pose,
                                NetDrawn,
                                DemoThing(*index),
                            ))
                            .id();
                        p.things.insert(*index, e);
                    }
                }
            }
            Thing::Smoke(s) => {
                seen.push(*index);
                if existing.is_some() {
                    continue;
                }
                let Some(id) = world
                    .resource::<WeaponRegistry>()
                    .find("weapon_smokegrenade")
                    .map(|d| d.id)
                else {
                    continue;
                };
                let Some(GrenadeEffect::Smoke(mut smoke)) = grenade_effect(world, p, id) else {
                    continue;
                };
                // Its fade as recorded (times after the cloud began).
                if s.fade_end > s.fade_start && s.fade_start >= s.spawn_time {
                    smoke.fade_start = s.fade_start - s.spawn_time;
                    smoke.fade_end = s.fade_end - s.spawn_time;
                }
                let started = local(s.spawn_time);
                let e = world
                    .spawn((
                        Name::new("Demo smoke cloud"),
                        Transform::from_translation(engine(s.origin)),
                        SmokeCloud {
                            centre: engine(s.origin),
                            started,
                            smoke,
                            grenade: None,
                            weapon: Some(id),
                        },
                        DemoThing(*index),
                    ))
                    .id();
                p.things.insert(*index, e);
            }
            Thing::Bomb(bomb) => {
                seen.push(*index);
                let planted = PlantedBomb {
                    planter: None,
                    site: None,
                    planted_at: local(bomb.blow - bomb.timer),
                    explode_at: local(bomb.blow),
                    timer: bomb.timer,
                    next_beep: p.next_beep,
                    // The server's: it thinks.
                    next_think: f64::INFINITY,
                    defuse: None,
                    defused: !bomb.ticking,
                };
                let e = match existing {
                    Some(e) => {
                        world.entity_mut(e).insert(planted);
                        e
                    }
                    None => {
                        let rules = world.get_resource::<BombRules>().cloned().unwrap_or_default();
                        let (yaw, _) = look(0.0, bomb.angles[1]);
                        let mut b = world.spawn((
                            Name::new("Demo planted bomb"),
                            planted,
                            Transform::from_translation(engine(bomb.origin)).with_rotation(Quat::from_rotation_y(yaw)),
                            NetDrawn,
                            DemoThing(*index),
                        ));
                        if let Some(model) = &rules.planted_model {
                            b.insert((ShownItem(model.clone()), SitOnOrigin(rules.planted_turn)));
                        }
                        let e = b.id();
                        p.things.insert(*index, e);
                        p.next_beep = now;
                        e
                    }
                };
                // CS:S's bomb beeps on each client; the demo has none.
                if bomb.ticking && now >= p.next_beep {
                    let remaining = (bomb.blow as f64 - server_now) as f32;
                    p.next_beep = now + beep_interval(remaining, bomb.timer) as f64;
                    let at = world.get::<Transform>(e).map_or(Vec3::ZERO, |t| t.translation);
                    if let Some(s) = world.get_resource::<BombRules>().and_then(|r| r.sounds.beep.clone()) {
                        world.write_message(PlaySound {
                            pitch: None,
                            entry: s,
                            at: Some(at),
                            volume: None,
                            source: Some(e),
                            channel: None,
                        });
                    }
                }
                let state = BombState {
                    planted: bomb.ticking.then_some(e),
                    planter: None,
                    outcome: None,
                };
                if world.get_resource::<BombState>() != Some(&state) {
                    world.insert_resource(state);
                }
            }
            _ => {}
        }
    }
    let gone: Vec<u32> = p.things.keys().copied().filter(|i| !seen.contains(i)).collect();
    for i in gone {
        if let Some(e) = p.things.remove(&i)
            && let Ok(e) = world.get_entity_mut(e)
        {
            let bomb = e.contains::<PlantedBomb>();
            e.despawn();
            if bomb {
                world.insert_resource(BombState::default());
            }
        }
    }
}

/// The round clock, freeze time and scores the HUD shows.
fn apply_round(world: &mut World, p: &DemoPlayer) {
    let t = &p.demo.timeline;
    let Some(frame) = t.frame_at(p.tick) else { return };
    let now = world.resource::<Time>().elapsed_secs_f64();
    let server_now = t.server_time(p.tick);
    let local = |s: f32| now + (s as f64 - server_now);
    let mut wins = [0u32; 2];
    let mut rules = None;
    for (_, thing) in &t.frames[frame].entities {
        match thing {
            Thing::Team(s) => match team(s.team) {
                Some(ATTACKERS) => wins[0] = s.score.max(0) as u32,
                Some(DEFENDERS) => wins[1] = s.score.max(0) as u32,
                _ => {}
            },
            Thing::Rules(r) => rules = Some(r.clone()),
            _ => {}
        }
    }
    let Some(r) = rules else { return };
    let phase = match r.over {
        // The next round starts after `mp_round_restart_delay` (5 s).
        Some(winner) => Phase::Over {
            until: now + 5.0,
            winner: winner.and_then(team),
        },
        None if r.freeze => Phase::Freeze {
            until: local(r.round_start),
        },
        None => Phase::Live {
            since: local(r.round_start),
            ends: local(r.round_start + r.round_time as f32),
        },
    };
    let mut state = world.get_resource_or_init::<RoundState>();
    // Times move with our clock every frame: only the kind of phase and its
    // ends matter to the HUD.
    state.phase = phase;
    if state.wins != wins {
        state.wins = wins;
    }
    if state.number != r.rounds {
        state.number = r.rounds;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn source_angles_and_places_are_ours() {
        // Yaw 90 (Source +Y, our -Z) is our 0; pitch down (+) is ours down (-).
        let (yaw, pitch) = look(10.0, 90.0);
        assert!(yaw.abs() < 1e-6 && (pitch + 10f32.to_radians()).abs() < 1e-6);
        // Pitch past 180 is up.
        let (_, pitch) = look(356.0, 0.0);
        assert!((pitch - 4f32.to_radians()).abs() < 1e-5);
        // Facing Source +X: our look turns -Z to +X.
        let (yaw, _) = look(0.0, 0.0);
        let dir = Quat::from_rotation_y(yaw) * Vec3::NEG_Z;
        assert!(dir.distance(Vec3::X) < 1e-5, "{dir}");
        let o = character_origin([100.0, 200.0, 10.0]);
        assert!(o.distance(Vec3::new(100.0, 46.0, -200.0) * UNIT) < 1e-5, "{o}");
        assert_eq!(team(2), Some(ATTACKERS));
        assert_eq!(team(3), Some(DEFENDERS));
        assert_eq!(team(1), None);
        assert!((lerp_angle(350.0, 10.0, 0.5) - 360.0).abs() < 1e-4);
    }

    #[test]
    fn weapon_classes_and_grenade_models_name_our_weapons() {
        assert_eq!(weapon_name("CAK47").as_deref(), Some("weapon_ak47"));
        assert_eq!(weapon_name("CWeaponM4A1").as_deref(), Some("weapon_m4a1"));
        assert_eq!(weapon_name("CKnife").as_deref(), Some("weapon_knife"));
        assert_eq!(weapon_name("CC4").as_deref(), Some("weapon_c4"));
        assert_eq!(weapon_name("CWeaponMP5Navy").as_deref(), Some("weapon_mp5navy"));
        assert_eq!(
            grenade_name("models/Weapons/w_eq_flashbang_thrown.mdl"),
            Some("weapon_flashbang")
        );
        assert_eq!(
            grenade_name("models/weapons/w_eq_fraggrenade_thrown.mdl"),
            Some("weapon_hegrenade")
        );
        assert_eq!(
            grenade_name("models/weapons/w_eq_smokegrenade_thrown.mdl"),
            Some("weapon_smokegrenade")
        );
        assert_eq!(
            sound_file(")Weapons\\knife/knife_deploy1.wav"),
            "weapons/knife/knife_deploy1.wav"
        );
        assert_eq!(words("#Cstrike_TitlesTXT_Fire_in_the_hole"), "Fire in the hole");
    }

    /// The sample bot round (never committed; `$CHIMERA_DEMOS` or the
    /// workspace's `target/demos/`): its players, shots, deaths, grenade
    /// and bomb as `Thing`s and `Happening`s.
    #[test]
    fn the_sample_round_reads_as_css_things() {
        let dir = chimera_core::env::var_os("CHIMERA_DEMOS")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/demos"));
        let Ok(data) = std::fs::read(dir.join("mashup_round_bots.dem")) else {
            eprintln!("skipping: no mashup_round_bots.dem in {}", dir.display());
            return;
        };
        let demo = read(&data).unwrap();
        let t = &demo.timeline;
        let events = || t.frames.iter().flat_map(|f| &f.events);
        assert_eq!(events().filter(|e| matches!(e, Happening::Shot { .. })).count(), 182);
        assert_eq!(events().filter(|e| matches!(e, Happening::Death { .. })).count(), 9);
        // Mark (entity 7) killed Grant (6) with the AK at tick 1058.
        let f = &t.frames[t.frame_at(1058.0).unwrap()];
        assert!(f.events.contains(&Happening::Death {
            victim: 6,
            attacker: Some(7),
            weapon: "ak47".into(),
            headshot: true,
        }));
        // His shots before it, from his eye.
        let shooter = events().find_map(|e| match e {
            Happening::Shot { player, .. } if *player == 7 => Some(*player),
            _ => None,
        });
        assert_eq!(shooter, Some(7));
        assert_eq!(demo.names.get(&7).map(String::as_str), Some("Mark"));
        // Six bots on teams with weapons.
        let players: Vec<&PlayerSnap> = f
            .entities
            .iter()
            .filter_map(|(_, e)| match e {
                Thing::Player(p) if p.team >= 2 => Some(p),
                _ => None,
            })
            .collect();
        assert_eq!(players.len(), 6);
        assert!(
            players
                .iter()
                .any(|p| p.weapon.is_some_and(|w| demo.class(w) == "CAK47"))
        );
        // The flashbang thrown at 662 flies, then goes off.
        let grenade = t.frames[t.frame_at(700.0).unwrap()]
            .entities
            .iter()
            .find_map(|(_, e)| match e {
                Thing::Grenade(g) => Some(g.clone()),
                _ => None,
            })
            .expect("a grenade in flight");
        assert_eq!(
            grenade_name(demo.model(grenade.model.unwrap())),
            Some("weapon_flashbang")
        );
        assert!(events().any(|e| matches!(e, Happening::Detonate { .. })));
        // The bomb planted at 8678 with its clock.
        let bomb = t.frames[t.frame_at(8700.0).unwrap()]
            .entities
            .iter()
            .find_map(|(_, e)| match e {
                Thing::Bomb(b) => Some(b.clone()),
                _ => None,
            })
            .expect("the planted bomb");
        assert!(bomb.ticking && bomb.timer > 0.0 && bomb.blow > 0.0);
        assert!((bomb.origin[0] + 1480.56).abs() < 0.1, "{:?}", bomb.origin);
        // Rounds: over at 2510, the next live from 2850.
        let rules = |tick: f64| {
            t.frames[t.frame_at(tick).unwrap()]
                .entities
                .iter()
                .find_map(|(_, e)| match e {
                    Thing::Rules(r) => Some(r.clone()),
                    _ => None,
                })
                .unwrap()
        };
        assert_eq!(rules(2600.0).over, Some(Some(3)));
        assert_eq!(rules(2900.0).over, None);
        assert!(events().any(|e| matches!(e, Happening::Audio(a) if a == "Radio.FireInTheHole")));
        assert!(
            events().any(|e| matches!(e, Happening::Text(l) if l.contains("Fire in the hole"))),
            "radio text"
        );
        assert!(
            demo.sounds.iter().any(|s| s == "weapons/c4/c4_plant.wav"),
            "{:?}",
            &demo.sounds[..5]
        );
    }
}
