//! TF2's player voices (specs/tf2/voices.md): pain when hurt, a death
//! cry, and kill lines, by class, from data/voices.toml. All the
//! server's: each plays as a `map::GameSound` from the player on the
//! voice channel (`CHAN_VOICE`), which a network server sends to every
//! client, so a newer voice sound of the same player (his death cry, his
//! next line) replaces it there (spec 1.6, 3.3).
//!
//! - **Pain** (`pain`, spec 2): after a hit that took health and left the
//!   victim alive (so the killing hit has none), unless the victim is
//!   inside his pain gap or `IsDominating`: a fall plays the class's
//!   `<Class>.Death` (no gap), drowning `TFPlayer.Drown`, burning nothing,
//!   anything else one of the class's `<Class>.PainSharpNN` (drawn without
//!   repeats per class, spec 1.4), and the next pain waits for that line's
//!   scene length (`ClassVoice::pain`, spec 2.4, 2.5). Any game's gun
//!   makes a TF2 character cry; the bullet's impact stays the gun's game's.
//! - **Death** (`death`, spec 3.1): by the killing damage: a fall
//!   `Player.FallGib` (no channel: it replaces nothing), a blast
//!   `<Class>.ExplosionDeath`, a melee hit `<Class>.MeleeDeath`, else
//!   `<Class>.Death`.
//! - **Kill lines** (`kill_lines`, spec 4; Chimera's `chimera_sim::
//!   kill_voice`): the killer's class is the one it had on the kill's tick
//!   (`KillVoiceAsk::character`: a rung up's new class or game changes
//!   nothing); its rules (`ClassVoice::kill`, in the order of the spec's
//!   scores: the first that passes wins) by the kill's facts: the recent
//!   kills (this one included, 30 s), the killing gun's slot (melee: slot
//!   2 or a melee hit), a headshot, the victim's class, a domination or
//!   revenge (`dominations::KillMark`), a chance against one draw r in
//!   [0, 100], and the killer's contexts (`<Class>KillSpeech`, ...: set for
//!   their seconds when a line is chosen; spec 1.5). A line chosen is
//!   told to `kill_voice` (`KillVoiced`) and plays after its group's
//!   predelay from the killer, dead or alive, whatever character he is by
//!   then. Only the first kill of a frame by a killer may speak (spec 4.2).
//! - **Voice sets** (`voice_sets`): a TF2 character's class is its
//!   `kill_voice::CharacterVoice`.
//!
//! Known shortcuts (docs/tech-debt.md): TF2 voices (specs/tf2/voices.md):
//! everyone near hears the victim's `PainSharp` line, the attacker too
//! (the spec gives the attacker a `PainSevere` line of his own: our game
//! sounds go to everyone); no crit pain or crit death (our damage carries
//! no crit flag); no speaking window (a kill line is not held back by a
//! pain line just started), no on-fire lines, no stun, Spy disguise,
//! Dead Ringer, silent-killer or cosmetic rules; kill rules are first-
//! match in the spec's score order (no random tie between equal scores),
//! only the families data/voices.toml lists (no named-weapon, sentry,
//! assist or per-victim domination rules). Why it's OK for now: the
//! common lines play by the spec's numbers; the rest wants parts TF2 here
//! doesn't have yet.

use std::collections::BTreeMap;

use bevy::{platform::collections::HashMap, prelude::*};
use chimera_sim::kill_voice::{CharacterVoice, KillVoiceAsk, KillVoiceSet, KillVoiced};
use serde::Deserialize;

use super::{GAME, classes::{Classes, Tf2Class}};
use crate::{
    core::{DamageKind, DamageTaken, Died, Health, Hitgroup, SimClock},
    map::{GameSound, PlaySound},
    slots::GameIdentity,
};

/// The voices, as written.
pub const DATA: &str = include_str!("../../data/voices.toml");

/// Source's voice channel.
pub const CHAN_VOICE: u8 = 2;
/// Kills counted by `recentkills` are newer than this (s; spec 4.1).
pub const RECENT_KILLS: f64 = 30.0;
/// The drowning and fall-splat entries (spec 2.2, 3.2).
pub const DROWN: &str = "TFPlayer.Drown";
pub const FALL_GIB: &str = "Player.FallGib";
/// The context that silences pain lines (spec 2.6).
pub const DOMINATING: &str = "IsDominating";

/// One class's voice (data/voices.toml).
#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ClassVoice {
    /// Its entries' prefix (`Scout`).
    pub speaker: String,
    /// The scene length (s) of each `<speaker>.PainSharpNN`, NN from 01.
    pub pain: Vec<f64>,
    /// Kill rules, best score first.
    #[serde(default)]
    pub kill: Vec<KillRule>,
}

/// What a kill rule needs (spec 4.3, 4.4).
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum When {
    /// Any kill.
    Any,
    /// At least 2 recent kills (this one included).
    Many,
    /// At least 4.
    VeryMany,
    /// A melee kill.
    Melee,
    /// A headshot with a sniper rifle.
    RifleHeadshot,
    /// A kill with a sniper rifle.
    Rifle,
    /// The victim is a Spy.
    SpyVictim,
    /// The kill started a domination.
    Dominated,
    /// The kill was a revenge.
    Revenge,
}

/// One kill rule.
#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct KillRule {
    pub when: When,
    /// The killing gun's slot it needs (0 primary, 1 secondary); none: any.
    #[serde(default)]
    pub slot: Option<u8>,
    /// Passes when the draw r (0..=100) is above this; none: always.
    #[serde(default)]
    pub chance: Option<u8>,
    /// Contexts that must be unset.
    #[serde(default)]
    pub unless: Vec<String>,
    /// Contexts set when it speaks: (name, seconds).
    #[serde(default)]
    pub sets: Vec<(String, f64)>,
    /// Its response groups: one drawn, then a line of it.
    pub groups: Vec<Group>,
}

/// A response group.
#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Group {
    pub lines: Vec<String>,
    /// Seconds before it plays.
    #[serde(default)]
    pub predelay: f64,
}

/// Every class's voice, by class name (`classes.toml`'s keys).
#[derive(Resource, Clone, Debug, Deserialize, PartialEq)]
pub struct Tf2Voices {
    pub classes: BTreeMap<String, ClassVoice>,
}

impl Tf2Voices {
    pub fn parse(text: &str) -> Result<Self, String> {
        toml::from_str(text).map_err(|e| format!("voices.toml: {e}"))
    }

    /// The built-in table.
    pub fn built_in() -> Self {
        Self::parse(DATA).unwrap_or_else(|e| panic!("{e}"))
    }

    /// Every sound entry it can play (the maps' sound tables keep them):
    /// `SHARED` and each class's (`ClassVoice::entries`).
    pub fn entries(&self) -> Vec<String> {
        let mut out: Vec<String> = SHARED.iter().map(|e| e.to_string()).collect();
        out.extend(self.classes.values().flat_map(ClassVoice::entries));
        out.sort();
        out.dedup();
        out
    }
}

/// The entries any class can play (drowning, the fall splat).
pub const SHARED: [&str; 2] = [DROWN, FALL_GIB];

impl ClassVoice {
    /// The sound entries of this class's own voice: its pain lines, death
    /// cries and kill lines (the class's `players/<class>` set holds their
    /// waves: `sets::class_files`), sorted.
    pub fn entries(&self) -> Vec<String> {
        let mut out: Vec<String> = (1..=self.pain.len()).map(|n| pain_entry(&self.speaker, n)).collect();
        for kind in ["Death", "CritDeath", "MeleeDeath", "ExplosionDeath"] {
            out.push(format!("{}.{kind}", self.speaker));
        }
        out.extend(self.kill.iter().flat_map(|r| r.groups.iter().flat_map(|g| g.lines.iter().cloned())));
        out.sort();
        out.dedup();
        out
    }
}

impl Default for Tf2Voices {
    fn default() -> Self {
        Self::built_in()
    }
}

/// `<speaker>.PainSharpNN`.
pub fn pain_entry(speaker: &str, n: usize) -> String {
    format!("{speaker}.PainSharp{n:02}")
}

/// The death entry for the killing damage's kind (spec 3.1; no crits).
pub fn death_entry(speaker: &str, kind: DamageKind) -> String {
    match kind {
        DamageKind::Fall => FALL_GIB.to_string(),
        DamageKind::Blast | DamageKind::BlastSurface => format!("{speaker}.ExplosionDeath"),
        DamageKind::Melee => format!("{speaker}.MeleeDeath"),
        _ => format!("{speaker}.Death"),
    }
}

/// A kill's facts for the rules.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct KillFacts {
    /// Recent kills, this one included.
    pub recent: usize,
    pub slot: Option<u8>,
    pub melee: bool,
    pub rifle: bool,
    pub headshot: bool,
    pub spy_victim: bool,
    pub dominated: bool,
    pub revenge: bool,
    /// The draw, 0..=100.
    pub r: u8,
}

impl KillRule {
    /// Whether it passes for `facts` with the speaker's live contexts
    /// `set` (names).
    pub fn passes(&self, f: &KillFacts, set: &dyn Fn(&str) -> bool) -> bool {
        let need = match self.when {
            When::Any => true,
            When::Many => f.recent >= 2,
            When::VeryMany => f.recent >= 4,
            When::Melee => f.melee,
            When::RifleHeadshot => f.rifle && f.headshot,
            When::Rifle => f.rifle,
            When::SpyVictim => f.spy_victim,
            When::Dominated => f.dominated,
            When::Revenge => f.revenge,
        };
        need && self.slot.is_none_or(|s| f.slot == Some(s))
            && self.chance.is_none_or(|c| f.r > c)
            && !self.unless.iter().any(|c| set(c))
    }
}

/// A player's voice state on the server (spec 1.5, 2.5, 4.1).
#[derive(Clone, Debug, Default)]
pub struct VoiceState {
    /// No pain line before this (SimClock s).
    pub next_pain: f64,
    /// Contexts and when each expires.
    pub contexts: BTreeMap<String, f64>,
    /// Its kills' times.
    pub kills: Vec<f64>,
    /// The last frame it had a kill request.
    pub asked: u32,
}

/// The server's voice state: players', the lines drawn per pool (no
/// repeats until a pool's every line was used, spec 1.4), lines waiting
/// out their predelay.
#[derive(Resource, Default)]
pub struct VoiceBook {
    pub players: HashMap<Entity, VoiceState>,
    pub used: HashMap<String, Vec<usize>>,
    /// (due, speaker, entry).
    pub pending: Vec<(f64, Entity, String)>,
    pub draws: u32,
    pub frame: u32,
}

impl VoiceBook {
    /// The next draw in [0, 1).
    fn next(&mut self) -> f32 {
        self.draws = self.draws.wrapping_add(1);
        chimera_sim::weapon::random::shared_random(self.draws, "tf2:voices", 0).next()
    }

    /// r, uniform in 0..=100 (spec, "Units and conventions").
    pub fn draw_r(&mut self) -> u8 {
        ((self.next() * 101.0) as u8).min(100)
    }

    /// One of `n` lines of pool `pool`, none twice until all were used.
    pub fn pick(&mut self, pool: &str, n: usize) -> usize {
        let roll = self.next();
        let used = self.used.entry(pool.to_string()).or_default();
        if used.len() >= n {
            used.clear();
        }
        let free: Vec<usize> = (0..n).filter(|i| !used.contains(i)).collect();
        let i = free[((roll * free.len() as f32) as usize).min(free.len() - 1)];
        used.push(i);
        i
    }
}

pub struct Tf2VoicesPlugin;

impl Plugin for Tf2VoicesPlugin {
    fn build(&self, app: &mut App) {
        let authoritative = crate::core::authoritative;
        app.init_resource::<Tf2Voices>()
            .init_resource::<VoiceBook>()
            .add_message::<DamageTaken>()
            .add_message::<Died>()
            .add_message::<GameSound>()
            .add_message::<chimera_sim::dominations::KillMark>()
            .add_systems(
                FixedUpdate,
                // The server's sounds and its own book: either order
                // against the tick's other systems is right.
                (voice_sets, (pain, death, speak).chain().after(crate::core::apply_damage))
                    .ambiguous_with_all()
                    .run_if(authoritative),
            )
            .configure_sets(PostUpdate, KillVoiceSet::Answer.after(chimera_sim::dominations::DominationSet))
            .add_systems(PostUpdate, kill_lines.in_set(KillVoiceSet::Answer).run_if(authoritative));
    }
}

/// A TF2 character's class name is its voice set.
fn voice_sets(
    classes: Option<Res<Classes>>,
    characters: Query<(Entity, &Tf2Class, Option<&GameIdentity>, Option<&CharacterVoice>)>,
    mut commands: Commands,
) {
    let fallback;
    let classes = match classes.as_deref() {
        Some(c) => c,
        None => {
            fallback = Classes::default();
            &fallback
        }
    };
    for (e, class, identity, voice) in &characters {
        if !GameIdentity::allows(identity, GAME) {
            continue;
        }
        let name = &class.class(classes).name;
        if voice.is_none_or(|v| &v.0 != name) {
            commands.entity(e).insert(CharacterVoice(name.clone()));
        }
    }
}

/// The class voice of a TF2 character (None: not one now).
fn class_voice<'a>(
    voices: &'a Tf2Voices,
    identity: Option<&GameIdentity>,
    voice: Option<&CharacterVoice>,
) -> Option<&'a ClassVoice> {
    if !GameIdentity::allows(identity, GAME) {
        return None;
    }
    voices.classes.get(&voice?.0)
}

fn voice_sound(entry: impl Into<String>, at: Vec3, speaker: Entity) -> GameSound {
    GameSound(PlaySound {
        source: Some(speaker),
        channel: Some(CHAN_VOICE),
        ..PlaySound::at(entry, at)
    })
}

/// Pain (spec 2).
#[allow(clippy::type_complexity)]
fn pain(
    mut taken: MessageReader<DamageTaken>,
    victims: Query<(&Health, &Transform, Option<&GameIdentity>, Option<&CharacterVoice>), With<Tf2Class>>,
    voices: Res<Tf2Voices>,
    clock: Res<SimClock>,
    mut book: ResMut<VoiceBook>,
    mut out: MessageWriter<GameSound>,
) {
    let now = clock.now;
    for t in taken.read() {
        let d = &t.damage;
        let Ok((health, at, identity, voice)) = victims.get(d.target) else { continue };
        let Some(class) = class_voice(&voices, identity, voice) else { continue };
        if health.current <= 0.0 || d.amount <= 0.0 {
            continue;
        }
        let state = book.players.entry(d.target).or_default();
        if now < state.next_pain || state.contexts.get(DOMINATING).is_some_and(|&until| now < until) {
            continue;
        }
        let entry = match d.kind {
            DamageKind::Fall => format!("{}.Death", class.speaker),
            DamageKind::Drown => DROWN.to_string(),
            DamageKind::Burn => continue,
            _ if class.pain.is_empty() => continue,
            _ => {
                let pool = format!("{}.pain", class.speaker);
                let n = book.pick(&pool, class.pain.len());
                book.players.entry(d.target).or_default().next_pain = now + class.pain[n];
                pain_entry(&class.speaker, n + 1)
            }
        };
        out.write(voice_sound(entry, at.translation, d.target));
    }
}

/// Death cries (spec 3).
fn death(
    mut died: MessageReader<Died>,
    victims: Query<(&Transform, Option<&GameIdentity>, Option<&CharacterVoice>), With<Tf2Class>>,
    voices: Res<Tf2Voices>,
    mut out: MessageWriter<GameSound>,
) {
    for d in died.read() {
        let Ok((at, identity, voice)) = victims.get(d.entity) else { continue };
        let Some(class) = class_voice(&voices, identity, voice) else { continue };
        let entry = death_entry(&class.speaker, d.damage.kind);
        out.write(if d.damage.kind == DamageKind::Fall {
            GameSound(PlaySound::at(entry, at.translation))
        } else {
            voice_sound(entry, at.translation, d.entity)
        });
    }
}

/// Kill lines (spec 4): the rules of the killer's class at the kill.
#[allow(clippy::too_many_arguments)]
fn kill_lines(
    mut asks: MessageReader<KillVoiceAsk>,
    mut marks: MessageReader<chimera_sim::dominations::KillMark>,
    characters: Query<(Option<&GameIdentity>, Option<&CharacterVoice>)>,
    living: Query<&Health>,
    voices: Res<Tf2Voices>,
    catalog: Option<Res<chimera_sim::weapon::data::WeaponCatalog>>,
    weapons: Option<Res<chimera_sim::weapon::DamageWeapons>>,
    clock: Res<SimClock>,
    mut book: ResMut<VoiceBook>,
    mut out: MessageWriter<KillVoiced>,
) {
    let marks: Vec<_> = marks.read().copied().collect();
    book.frame = book.frame.wrapping_add(1);
    let (now, frame) = (clock.now, book.frame);
    for a in asks.read() {
        if !a.is_for(GAME) {
            continue;
        }
        let Some(class) = a.character.voice.as_deref().and_then(|v| voices.classes.get(v)) else { continue };
        let state = book.players.entry(a.killer).or_default();
        state.kills.retain(|t| now - t < RECENT_KILLS);
        state.kills.push(now);
        let recent = state.kills.len();
        // Gates (spec 4.2): alive, one request a frame.
        if living.get(a.killer).is_ok_and(|h| h.current <= 0.0) || state.asked == frame {
            continue;
        }
        state.asked = frame;
        let id = weapons.as_deref().map_or(a.damage.weapon, |w| w.of(&a.damage));
        let slot = id.and_then(|id| catalog.as_deref()?.get(id)).map(|w| w.slot);
        let mark = marks.iter().find(|m| m.killer == a.killer && m.victim == a.victim);
        let victim_class = characters
            .get(a.victim)
            .ok()
            .filter(|(i, _)| GameIdentity::allows(*i, GAME))
            .and_then(|(_, v)| v.map(|v| v.0.clone()));
        let r = book.draw_r();
        let facts = KillFacts {
            recent,
            slot,
            melee: a.damage.kind == DamageKind::Melee || slot == Some(2),
            rifle: id.is_some_and(|id| id.contains("sniperrifle")),
            headshot: a.damage.hitgroup == Hitgroup::Head,
            spy_victim: victim_class.as_deref() == Some("spy"),
            dominated: mark.is_some_and(|m| m.dominated),
            revenge: mark.is_some_and(|m| m.revenge),
            r,
        };
        let contexts = book.players.get(&a.killer).map(|s| s.contexts.clone()).unwrap_or_default();
        let set = |c: &str| contexts.get(c).is_some_and(|&until| now < until);
        let Some((index, rule)) = class.kill.iter().enumerate().find(|(_, rule)| rule.passes(&facts, &set)) else {
            continue;
        };
        if rule.groups.is_empty() {
            continue;
        }
        let pool = format!("{}.kill{index}", class.speaker);
        let gi = book.pick(&format!("{pool}.group"), rule.groups.len());
        let g = &rule.groups[gi];
        if g.lines.is_empty() {
            continue;
        }
        let line = g.lines[book.pick(&format!("{pool}.{gi}"), g.lines.len())].clone();
        let state = book.players.entry(a.killer).or_default();
        for (name, seconds) in &rule.sets {
            state.contexts.insert(name.clone(), now + seconds);
        }
        book.pending.push((now + g.predelay, a.killer, line));
        out.write(KillVoiced {
            killer: a.killer,
            victim: a.victim,
        });
    }
}

/// Lines whose predelay is over, from their speaker wherever he is (dead
/// or another character by now); a speaker gone takes his along. Players
/// gone leave the book.
fn speak(
    speakers: Query<&Transform>,
    clock: Res<SimClock>,
    mut book: ResMut<VoiceBook>,
    mut out: MessageWriter<GameSound>,
) {
    let now = clock.now;
    let book = &mut *book;
    book.pending.retain(|(due, speaker, entry)| {
        let Ok(at) = speakers.get(*speaker) else { return false };
        if now + 1e-9 < *due {
            return true;
        }
        out.write(voice_sound(entry.clone(), at.translation, *speaker));
        false
    });
    book.players.retain(|e, _| speakers.contains(*e));
}
