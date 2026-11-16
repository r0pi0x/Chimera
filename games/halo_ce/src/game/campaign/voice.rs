//! What a campaign body sounds like (specs/halo_ce/characters.md section
//! 6, sounds.md "Unit dialogue"; effects.md "Material effects"): its own
//! footsteps, jump and landing sounds, its pain, death, scream and maimed
//! lines, and our own lines that say who it is.
//!
//! **The sounds** (`BodySounds`, made with the unit, `compile`): the
//! biped's own material effects (its footsteps tag, in the body unit:
//! `audio::steps` plays a campaign body's steps from them, kinds walk,
//! run, jump and jump land, as it plays the Spartan's from
//! `globals\cyborg`), and its dialogue's vocalizations 6 to 19 (pain body
//! minor to death flying) from the voice unit (`voice.map`), each slot's
//! sound or none. The dialogue is the biped's dialogue variant numbered 0
//! (else its first variant below 100), and the slots its row chooses for
//! our own lines (`super::Lines`; the voice set holds its whole dialogue,
//! so only the compiling reads further). Without the voice set the body is
//! silent. The decoded clips and entries join each map's `SoundBank`
//! (`join_banks`), on every machine that loaded the unit.
//!
//! **The lines** (`speak`, the host's, after the damage seam): for a body
//! of a campaign kind, each `HaloHurt` (a Halo hit that landed): a kill
//! says its death line (falling damage: death falling, flame: death
//! agonizing, a push of 2 WU/s or more: death flying, else death quiet:
//! a unit without an actor is practically never "severe"); a hit that
//! does not kill, while the major pain timer is 0, says pain falling for
//! falling damage, pain body major when the recent body damage is 0.6 or
//! more (once per life: the major timer never counts down), else, when
//! the minor timer is 0, fewer than 3 minor lines are counted and the
//! body is silent or a draw is below 0.4, pain body minor (body damage
//! taken) or pain shield (sounds.md "Pain line choice"). An empty slot
//! falls back by sounds.md's table. A line plays at the head (the body's
//! camera height) as a `GameSound`, which a network server sends to every
//! client; a 7-tick pause follows it. A line starts while another lasts
//! only when its priority is higher (pain 2, involuntary 7: the scream
//! and the maimed lines, death 10), and nothing but a death line once
//! dead. The timers step once per Halo tick.
//!
//! **The involuntary lines** (characters.md section 6, "What a
//! player-worn character says", items 2 and 3):
//! - *Scream fear*, a dangerous fall (`scream`): airborne and falling
//!   faster than a landing would start to hurt (the body's movement
//!   table's `fall_v_min`, the landing speed past which `globals\falling`
//!   is dealt: "falls fast enough to be hurt"), once per fall (on the
//!   ground again, the next fall may scream). An empty slot gives none
//!   (the Elite fills no scream fear: it falls silently; scream pain
//!   falls back to scream fear, not the other way).
//! - *Maimed limb*, or *maimed head* for a region flagged "causes
//!   head-maimed scream" (`maimed`), when a region is destroyed: said on
//!   a `RegionLost` message. Nothing writes one yet: Chimera's Halo
//!   vitality destroys no region, and none of the worn kinds has a
//!   destroyable region (characters.md section 3: the Elite's, the
//!   Grunt's and the Jackal's are not destroyable; the Jackal's shield
//!   region only disappears with the shield), so faithfully they never
//!   say it. The Flood's limbs will.
//!
//! **The slots' names** (`SLOT_NAMES`, `slot_of`): a dialogue tag is 256
//! slots (0x1010 bytes on every campaign character's); sounds.md names 6
//! to 19, characters.md section 6 lists the slots each character fills
//! by name but not by index. The index of each name is read from the
//! disc: the slots the characters' dialogues fill, matched in order with
//! the spec's lists (the line counts agree) and with the sound tags' own
//! names (`...\killedenemyplayer` in slot 55 of every dialogue that
//! fills it). Our data names slots by these names.
//!
//! **Lines that say who it is** (`chatter`, the host's, after the damage;
//! the owner's addition, 2026-11-16: the game's player-worn body says only
//! its pain, death and scream lines): a character's row names, in order,
//! the slots it says on becoming the character and after a kill
//! (`super::Lines`); the first its voice fills plays (`first_filled`; no
//! fallback). *Becoming*: a body that now wears a kind it did not wear at
//! the last tick (another kind, or none: the Spartan, another game's
//! character), or comes back to life as the same kind, says its becoming
//! line, the same kind again only `becoming_repeat_seconds` (data
//! `[chatter]`, 60) after its last (`becoming_due`); a body whose voice is
//! not in yet keeps the line waiting, dropped if it dies first. *Kill*: a
//! `Died` whose attacker is the body (not itself), with the chance
//! `kill_chance`, never within `kill_cooldown_seconds` of its last
//! (`kill_due`; the draw a hash of the tick and the body). Both play as
//! the damage lines do (`say`: at the eye, one line at a time per body)
//! at priority `CHATTER` (1: below pain, so a pain line cuts it and it
//! never cuts one), silent without the voice set, the Marine's drawn voice
//! for a Marine. Each line said is logged (debug: the kind, the slot's
//! name and the sound).
//!
//! Known shortcuts:
//! - The minor-pain draw is a hash of the tick and the body (no game
//!   state random generator).
//! - A line plays at a fixed height on the body, not its moving `head`
//!   marker.

use std::collections::BTreeMap;

use bevy::prelude::*;
use blam_formats::{
    cache::{MapFile, class},
    object::Biped,
};

use super::{BodyDef, BodyKind};
use super::super::{
    audio::library::{AudioLibrary, AudioSet, Compiler},
    damage::HaloHurt,
    vitality::{DamageEffects, Vitality, data::Category},
};
use crate::map::{GameSound, PlaySound, sound::SoundBank};

/// The vocalization indices a worn body says (sounds.md's table).
pub const PAIN_BODY_MINOR: usize = 6;
pub const PAIN_BODY_MAJOR: usize = 7;
pub const PAIN_SHIELD: usize = 8;
pub const PAIN_FALLING: usize = 9;
pub const SCREAM_FEAR: usize = 10;
pub const SCREAM_PAIN: usize = 11;
pub const MAIMED_LIMB: usize = 12;
pub const MAIMED_HEAD: usize = 13;
pub const DEATH_QUIET: usize = 14;
pub const DEATH_FALLING: usize = 16;
pub const DEATH_AGONIZING: usize = 17;
pub const DEATH_FLYING: usize = 19;
/// The damage lines' slots: 0 to `SLOTS - 1` (each is read).
pub const SLOTS: usize = 20;

/// The slots of a dialogue tag (after its 0x10-byte header, 0x10 each:
/// every dialogue of the ten campaign characters' on the disc spans
/// 0x1010 bytes, 256 slots).
pub const DIALOGUE_SLOTS: usize = 256;

/// The slots by name (module doc, "The slots' names"): the vocalizations
/// the campaign characters' dialogues fill, by their index.
pub const SLOT_NAMES: &[(usize, &str)] = &[
    (0, "idle noncombat"),
    (1, "idle combat"),
    (2, "idle flee"),
    (6, "pain body minor"),
    (7, "pain body major"),
    (8, "pain shield"),
    (9, "pain falling"),
    (10, "scream fear"),
    (11, "scream pain"),
    (12, "maimed limb"),
    (13, "maimed head"),
    (14, "death quiet"),
    (15, "death violent"),
    (16, "death falling"),
    (17, "death agonizing"),
    (18, "death instant"),
    (19, "death flying"),
    (21, "damaged friend"),
    (22, "damaged friend player"),
    (23, "damaged enemy"),
    (24, "damaged enemy cm"),
    (29, "hurt friend"),
    (30, "hurt friend re"),
    (31, "hurt friend player"),
    (32, "hurt enemy"),
    (36, "hurt enemy needler"),
    (37, "hurt enemy plasma"),
    (39, "hurt enemy grenade"),
    (41, "hurt enemy melee"),
    (45, "hurt enemy mounted weapon"),
    (49, "killed friend"),
    (50, "killed friend cm"),
    (51, "killed friend player"),
    (52, "killed friend player cm"),
    (53, "killed enemy"),
    (54, "killed enemy cm"),
    (55, "killed enemy player"),
    (56, "killed enemy player cm"),
    (57, "killed enemy covenant"),
    (58, "killed enemy covenant cm"),
    (59, "killed enemy flood combat"),
    (60, "killed enemy flood combat cm"),
    (61, "killed enemy flood carrier"),
    (62, "killed enemy flood carrier cm"),
    (63, "killed enemy sentinel"),
    (64, "killed enemy sentinel cm"),
    (65, "killed enemy bullet"),
    (66, "killed enemy needler"),
    (68, "killed enemy sniper"),
    (69, "killed enemy grenade"),
    (73, "killed enemy shotgun"),
    (75, "killed enemy mounted weapon"),
    (76, "killing spree"),
    (84, "player kill sniper cm"),
    (85, "anyone kill grenade cm"),
    (88, "player kill flame cm"),
    (90, "player kill vehicle cm"),
    (92, "player killing spree cm"),
    (96, "friend died"),
    (97, "friend player died"),
    (98, "friend killed by friend"),
    (99, "friend killed by friendly player"),
    (100, "friend killed by enemy"),
    (101, "friend killed by enemy player"),
    (102, "friend killed by covenant"),
    (103, "friend killed by flood"),
    (104, "friend killed by sentinel"),
    (105, "friend betrayed"),
    (108, "new combat alone"),
    (109, "new enemy recent combat"),
    (110, "old enemy sighted"),
    (111, "unexpected enemy"),
    (112, "dead friend found"),
    (113, "alliance broken"),
    (114, "alliance reformed"),
    (115, "grenade throwing"),
    (116, "grenade sighted"),
    (117, "grenade startle"),
    (118, "grenade danger enemy"),
    (119, "grenade danger self"),
    (120, "grenade danger friend"),
    (123, "new combat group re"),
    (124, "new combat nearby re"),
    (125, "alert friend"),
    (126, "alert friend re"),
    (127, "alert lost contact"),
    (128, "alert lost contact re"),
    (129, "blocked"),
    (130, "blocked re"),
    (131, "search start"),
    (132, "search query"),
    (133, "search query re"),
    (134, "search report"),
    (135, "search abandon"),
    (137, "group uncover"),
    (138, "group uncover re"),
    (139, "advance"),
    (141, "retreat"),
    (142, "retreat re"),
    (143, "cover"),
    (148, "sighted friendly player"),
    (149, "shooting"),
    (150, "shooting vehicle"),
    (151, "shooting berserk"),
    (152, "shooting group"),
    (153, "shooting traitor"),
    (154, "taunt"),
    (156, "flee"),
    (157, "flee re"),
    (158, "flee leader died"),
    (161, "lost contact"),
    (162, "hiding finished"),
    (163, "vehicle entry"),
    (164, "vehicle exit"),
    (165, "vehicle woohoo"),
    (166, "vehicle scared"),
    (168, "partially sighted"),
    (169, "nothing there"),
    (177, "surprise"),
    (178, "berserk"),
    (179, "melee attack"),
    (180, "dive"),
    (181, "uncover exclamation"),
    (188, "celebration"),
    (189, "check body enemy"),
    (190, "check body friend"),
    (191, "shooting dead enemy"),
    (192, "shooting dead enemy player"),
    (197, "alone"),
    (198, "unscathed slightly wounded re"),
    (199, "seriously wounded"),
    (200, "seriously wounded re"),
    (201, "near massacre"),
    (202, "near massacre re"),
    (203, "rout"),
    (204, "rout re"),
];

/// A slot by its name (`SLOT_NAMES`; case and spaces as written there).
pub fn slot_of(name: &str) -> Option<usize> {
    SLOT_NAMES.iter().find(|(_, n)| n.eq_ignore_ascii_case(name.trim())).map(|(i, _)| *i)
}

/// A slot's name (`SLOT_NAMES`), or `slot <n>`.
pub fn slot_name(slot: usize) -> String {
    SLOT_NAMES.iter().find(|(i, _)| *i == slot).map_or_else(|| format!("slot {slot}"), |(_, n)| (*n).to_string())
}

/// A line's priority (sounds.md "Playing a line").
pub const PAIN: u8 = 2;
pub const INVOLUNTARY: u8 = 7;
pub const DEATH: u8 = 10;
/// Our own lines (module doc, "Lines that say who it is"): below pain.
pub const CHATTER: u8 = 1;

/// A region destroyed on a campaign body (module doc, the maimed lines):
/// `head` when the region causes the head-maimed scream.
#[derive(Message, Clone, Copy, Debug, PartialEq, Eq)]
pub struct RegionLost {
    pub target: Entity,
    pub head: bool,
}

/// The slot an empty one falls back to (sounds.md), or none.
pub fn fallback(slot: usize) -> Option<usize> {
    match slot {
        7..=9 => Some(6),
        11 => Some(10),
        13 => Some(12),
        14 | 17 | 19 => Some(15),
        16 | 18 => Some(14),
        _ => None,
    }
}

/// A body's sounds (module doc): a library (its sounds and footstep rows)
/// with its clips and entries, and its lines by vocalization.
#[derive(Clone, Debug, Default)]
pub struct BodySounds {
    pub set: AudioSet,
    /// Sound paths (keys of `set.library.sounds`) by vocalization index.
    pub lines: Vec<Option<String>>,
}

impl BodySounds {
    /// The sound for `slot`, its fallbacks followed.
    pub fn line(&self, slot: usize) -> Option<&str> {
        let mut s = Some(slot);
        while let Some(i) = s {
            if let Some(Some(p)) = self.lines.get(i) {
                return Some(p);
            }
            s = fallback(i);
        }
        None
    }

    /// Lines filled (for the log and tests).
    pub fn filled(&self) -> usize {
        self.lines.iter().flatten().count()
    }
}

/// `b`'s set appended to `a` (clip indices moved past `a`'s).
fn append(a: &mut AudioSet, b: AudioSet) {
    let offset = a.clips.len();
    a.clips.extend(b.clips);
    for (name, mut e) in b.entries {
        for w in &mut e.waves {
            *w += offset;
        }
        a.entries.entry(name).or_insert(e);
    }
    for (k, v) in b.library.sounds {
        a.library.sounds.entry(k).or_insert(v);
    }
    if a.library.footsteps.is_empty() {
        a.library.footsteps = b.library.footsteps;
    }
    a.warnings.extend(b.warnings);
}

/// Body `def`'s sounds from its unit `body` and its voice unit `voice`
/// (module doc).
pub fn compile(def: &BodyDef, body: &MapFile, voice: Option<&MapFile>) -> BodySounds {
    let biped = body.find(class::BIPED, &def.biped).and_then(|t| Biped::read(body, t).ok());
    let mut out = BodySounds { lines: vec![None; DIALOGUE_SLOTS], ..Default::default() };
    // Its footsteps: the biped's material effects, as the Spartan's.
    let mut c = Compiler::new(body);
    if let Some(foot) = biped
        .as_ref()
        .and_then(|b| body.tag(b.own.footsteps.id))
        .and_then(|t| blam_formats::effect::MaterialEffects::read(body, t).ok())
    {
        for kind in foot.kinds.iter().take(crate::game::audio::STEP_KINDS) {
            let row = kind.iter().map(|m| c.sound(&m.sound).unwrap_or_default()).collect();
            c.set.library.footsteps.push(row);
        }
    }
    out.set = c.set;
    let Some(voice) = voice else { return out };
    let lines = compile_lines(def, body, voice, None);
    out.lines = lines.lines;
    append(&mut out.set, lines.set);
    out
}

/// The lines alone of voice unit `voice` (no footsteps): the dialogue
/// named `name` among the biped's variants (`unit::dialogue_name`: one of
/// a kind's several voices), else the variant numbered 0 (else its first
/// below 100).
pub fn compile_lines(def: &BodyDef, body: &MapFile, voice: &MapFile, name: Option<&str>) -> BodySounds {
    let mut out = BodySounds { lines: vec![None; DIALOGUE_SLOTS], ..Default::default() };
    let Some(b) = body.find(class::BIPED, &def.biped).and_then(|t| Biped::read(body, t).ok()) else { return out };
    let v = &b.unit.dialogue_variants;
    let named = |d: &&blam_formats::object::DialogueVariant| {
        name.is_none_or(|n| voice.tag(d.dialogue.id).is_some_and(|t| super::unit::dialogue_name(&t.path) == n))
    };
    let chosen = match name {
        Some(_) => v.iter().find(named),
        None => v.iter().find(|d| d.variant_number == 0).or_else(|| v.iter().find(|d| d.variant_number < 100)),
    };
    let Some(dialogue) = chosen.and_then(|d| voice.tag(d.dialogue.id)) else { return out };
    let Ok(view) = voice.tag_view(dialogue, 0x10 + DIALOGUE_SLOTS * 0x10) else { return out };
    let mut c = Compiler::new(voice);
    // The damage lines, and our own chosen ones (`super::Lines`).
    let chosen = def.lines.iter().flat_map(|l| l.becoming.iter().chain(&l.kill)).filter_map(|n| slot_of(n));
    let mut slots: Vec<usize> = (PAIN_BODY_MINOR..SLOTS).chain(chosen).filter(|s| *s < DIALOGUE_SLOTS).collect();
    slots.sort_unstable();
    slots.dedup();
    for slot in slots {
        let r = view.tag_ref(0x10 + slot * 0x10).ok().filter(|r| voice.tag(r.id).is_some());
        out.lines[slot] = r.and_then(|r| c.sound(&r));
    }
    out.set = c.set;
    out
}

/// Each loaded kind's sounds (`unit::load_on_demand` adds them).
#[derive(Resource, Clone, Default)]
pub struct CampaignSounds(pub BTreeMap<BodyKind, std::sync::Arc<BodySounds>>);

/// The lines of each loaded voice of a kind with several
/// (`BodyDef::voices`), by kind and voice (`super::BodyVoice`).
#[derive(Resource, Clone, Default)]
pub struct CampaignVoices(pub BTreeMap<(BodyKind, u8), std::sync::Arc<BodySounds>>);

impl CampaignSounds {
    /// Kind `kind`'s library, when it has footsteps of its own.
    pub fn steps(&self, kind: Option<&BodyKind>) -> Option<&AudioLibrary> {
        let s = self.0.get(kind?)?;
        (!s.set.library.footsteps.is_empty()).then_some(&s.set.library)
    }
}

/// The name that marks a kind's sounds (or one of its voices) as in a
/// bank.
fn marker(kind: BodyKind, voice: Option<u8>) -> String {
    match voice {
        Some(v) => format!("halo_ce:campaign sounds#{}#{v}", kind.id()),
        None => format!("halo_ce:campaign sounds#{}", kind.id()),
    }
}

/// Every machine: each loaded kind's (and voice's) clips and entries in
/// the map's `SoundBank` (a new map's bank has none; added once per bank).
pub fn join_banks(sounds: Option<Res<CampaignSounds>>, voices: Option<Res<CampaignVoices>>, bank: Option<ResMut<SoundBank>>) {
    let Some(mut bank) = bank else { return };
    let all = sounds
        .iter()
        .flat_map(|s| s.0.iter().map(|(k, s)| (marker(*k, None), s)))
        .chain(voices.iter().flat_map(|v| v.0.iter().map(|((k, v), s)| (marker(*k, Some(*v)), s))));
    let missing: Vec<(String, &std::sync::Arc<BodySounds>)> =
        all.filter(|(name, s)| !s.set.entries.is_empty() && bank.0.entry(name).is_none()).collect();
    if missing.is_empty() {
        return;
    }
    let m = std::sync::Arc::make_mut(&mut bank.0);
    for (name, s) in missing {
        let offset = m.clips.len();
        m.clips.extend(s.set.clips.iter().cloned());
        for (name, e) in &s.set.entries {
            let mut e = e.clone();
            for w in &mut e.waves {
                *w += offset;
            }
            m.entries.entry(name.to_lowercase()).or_insert(e);
        }
        // The marker: an entry with nothing to play.
        if let Some(e) = s.set.entries.values().next() {
            m.entries.insert(name, crate::map::MapSoundEntry { waves: Vec::new(), ..e.clone() });
        }
    }
}

/// A body's dialogue timers (module doc), the host's.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq)]
pub struct Voice {
    /// Ticks until another pain line may start (major: never counts down).
    pub major: u32,
    pub minor: u32,
    pub minor_count: u32,
    pub decay: u32,
    /// Ticks until the line playing (and its pause) ends.
    pub speaking: u32,
    /// The priority of the line playing.
    pub priority: u8,
    /// This fall's scream said (cleared on the ground).
    pub screamed: bool,
    /// The body it was alive as (a respawn resets the timers).
    pub dead: bool,
}

/// A dangerous fall's scream (module doc): `airborne`, falling at `down`
/// WU/t where a landing past `harmful` WU/t hurts.
pub fn scream(v: &mut Voice, airborne: bool, down: f32, harmful: f32) -> Option<usize> {
    if !airborne {
        v.screamed = false;
        return None;
    }
    if v.screamed || v.dead || down <= harmful {
        return None;
    }
    v.screamed = true;
    Some(SCREAM_FEAR)
}

/// A destroyed region's line (module doc).
pub fn maimed(head: bool) -> usize {
    if head { MAIMED_HEAD } else { MAIMED_LIMB }
}

/// Whether a line of `priority` may start now: silent, or above the line
/// playing; once dead, only a death line.
pub fn may_start(v: &Voice, priority: u8) -> bool {
    (!v.dead || priority >= DEATH) && (v.speaking == 0 || priority > v.priority)
}

/// What a hit makes a body say (module doc), or none: the slot.
pub fn choose(v: &mut Voice, died: bool, category: Category, push: f32, recent_body: f32, body_hit: bool, draw: f32) -> Option<usize> {
    if died {
        return Some(match category {
            Category::Falling => DEATH_FALLING,
            Category::Flame => DEATH_AGONIZING,
            _ if push >= 2.0 => DEATH_FLYING,
            _ => DEATH_QUIET,
        });
    }
    if v.major > 0 {
        return None;
    }
    if category == Category::Falling {
        v.major = u32::MAX;
        return Some(PAIN_FALLING);
    }
    if recent_body >= 0.6 {
        v.major = u32::MAX;
        return Some(PAIN_BODY_MAJOR);
    }
    let silent = v.speaking == 0;
    if v.minor == 0 && v.minor_count < 3 && (silent || draw < 0.4) {
        v.minor = 30;
        v.minor_count += 1;
        v.decay = 22;
        return Some(if body_hit { PAIN_BODY_MINOR } else { PAIN_SHIELD });
    }
    None
}

/// One Halo tick of the timers.
pub fn step(v: &mut Voice) {
    v.minor = v.minor.saturating_sub(2);
    v.speaking = v.speaking.saturating_sub(1);
    if v.speaking == 0 {
        v.priority = 0;
    }
    if v.minor_count > 0 {
        v.decay = v.decay.saturating_sub(1);
        if v.decay == 0 {
            v.minor_count -= 1;
            v.decay = 22;
        }
    }
}

/// The host: each campaign body's lines this tick (module doc).
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub fn speak(
    mut hurts: MessageReader<HaloHurt>,
    mut lost: MessageReader<RegionLost>,
    effects: Res<DamageEffects>,
    sounds: Option<Res<CampaignSounds>>,
    voices: Option<Res<CampaignVoices>>,
    tables: Option<Res<super::BodyTables>>,
    clock: Res<crate::core::SimClock>,
    mut bodies: Query<(
        Entity,
        &BodyKind,
        Option<&super::BodyVoice>,
        &Transform,
        Option<&Vitality>,
        Option<&mut Voice>,
        Option<&crate::core::Velocity>,
        Option<&crate::core::MovementState>,
    )>,
    mut lines: Option<ResMut<Messages<GameSound>>>,
    mut commands: Commands,
) {
    let ticks = crate::game::ticks::halo_ticks(&clock);
    let hurts: Vec<HaloHurt> = hurts.read().copied().collect();
    let lost: Vec<RegionLost> = lost.read().copied().collect();
    let per_tick = crate::game::METERS_PER_UNIT * crate::game::movement::data::TICKS_PER_SECOND;
    for (e, kind, chosen, t, vitality, voice, velocity, state) in &mut bodies {
        let Some(mut voice) = voice else {
            commands.entity(e).insert(Voice::default());
            continue;
        };
        for _ in 0..ticks {
            step(&mut voice);
        }
        let alive = vitality.is_none_or(|v| !v.dead);
        if alive && voice.dead {
            *voice = Voice::default();
        }
        // A kind with several voices speaks its drawn one's lines.
        let s = match chosen {
            Some(v) => voices.as_deref().and_then(|s| s.0.get(&(*kind, v.0))),
            None => sounds.as_deref().and_then(|s| s.0.get(kind)),
        };
        let Some(s) = s else { continue };
        let table = tables.as_deref().and_then(|t| t.movement.get(kind));
        // This tick's lines, in the order they came: (slot, priority).
        let mut said: Vec<(usize, u8)> = Vec::new();
        let mut died_now = false;
        for h in hurts.iter().filter(|h| h.target == e && h.outcome.landed) {
            if voice.dead || died_now {
                break;
            }
            let Some(fx) = effects.get(h.effect) else { continue };
            let recent = vitality.map_or(0.0, |v| v.recent_body_damage);
            let draw = (crate::game::campaign::roll(&[clock.tick, e.to_bits()]) >> 40) as f32 / (1u64 << 24) as f32;
            let died = h.outcome.died;
            if let Some(slot) = choose(&mut voice, died, fx.category, fx.accel, recent, h.outcome.body_amount > 0.0, draw) {
                said.push((slot, if died { DEATH } else { PAIN }));
            }
            died_now |= died;
        }
        for l in lost.iter().filter(|l| l.target == e) {
            said.push((maimed(l.head), INVOLUNTARY));
        }
        if let (Some(m), Some(state), Some(vel)) = (table, state, velocity)
            && let Some(slot) = scream(&mut voice, !state.on_ground, -vel.0.y / per_tick, m.fall_v_min)
        {
            said.push((slot, INVOLUNTARY));
        }
        for (slot, priority) in said {
            say(&mut voice, s, slot, priority, head(t, table), lines.as_deref_mut());
        }
        voice.dead |= died_now;
    }
}

/// Where a body's line plays: at its eye (the body's origin is its pill's
/// centre; `table`: its movement table).
pub fn head(t: &Transform, table: Option<&crate::game::movement::MovementData>) -> Vec3 {
    let above = table.map_or(0.2, |m| m.eye(0.0) - m.centre_above_origin(0.0));
    t.translation + Vec3::Y * above * crate::game::METERS_PER_UNIT
}

/// Line `slot` (its fallbacks followed) of `s` at `at`, if a line of
/// `priority` may start (`may_start`): the body speaks for its longest
/// permutation and a 7-tick pause. The sound's path, if it played.
pub fn say<'a>(voice: &mut Voice, s: &'a BodySounds, slot: usize, priority: u8, at: Vec3, out: Option<&mut Messages<GameSound>>) -> Option<&'a str> {
    if !may_start(voice, priority) {
        return None;
    }
    let path = s.line(slot)?;
    let info = s.set.library.sounds.get(path)?;
    voice.speaking = (info.longest_ms.max(0) as u32 * 30 / 1000) + 7;
    voice.priority = priority;
    if let Some(m) = out {
        m.write(GameSound(PlaySound::at(info.entry.clone(), at)));
    }
    Some(path)
}

/// The first of `slots` (names, `slot_of`) that `s` fills itself (no
/// fallback: our own lines never borrow a pain or death line).
pub fn first_filled(s: &BodySounds, slots: &[String]) -> Option<usize> {
    slots.iter().filter_map(|n| slot_of(n)).find(|i| s.lines.get(*i).is_some_and(Option::is_some))
}

/// Our own lines' state, the host's (module doc, "Lines that say who it
/// is"): per character, the kind it wore and whether it was alive at the
/// last tick, a becoming waiting for its line (its voice may still be
/// loading), and when its last becoming and kill lines were said
/// (seconds of the simulation clock).
#[derive(Resource, Clone, Debug, Default)]
pub struct Chatter(pub BTreeMap<Entity, ChatterState>);

/// One character's (`Chatter`).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ChatterState {
    pub worn: Option<BodyKind>,
    pub alive: bool,
    pub pending: bool,
    pub becoming_at: Option<f64>,
    pub kill_at: Option<f64>,
}

/// Whether a body that now wears `kind` (`was`: what it wore before, None:
/// no campaign kind) says its becoming line, its last said at `last`
/// (seconds) and now `now`: a new kind always; the same kind again (a new
/// life) once `repeat` seconds have passed.
pub fn becoming_due(was: Option<BodyKind>, kind: BodyKind, last: Option<f64>, now: f64, repeat: f32) -> bool {
    was != Some(kind) || last.is_none_or(|t| now - t >= f64::from(repeat))
}

/// Whether a kill makes the killer's body say a kill line: `draw` (0..1)
/// below `chance`, its last kill line `cooldown` seconds or more ago.
pub fn kill_due(last: Option<f64>, now: f64, chance: f32, cooldown: f32, draw: f32) -> bool {
    draw < chance && last.is_none_or(|t| now - t >= f64::from(cooldown))
}

/// The host: each campaign body's becoming and kill lines this tick
/// (module doc, "Lines that say who it is").
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub fn chatter(
    mut died: MessageReader<crate::core::Died>,
    data: Res<super::CampaignData>,
    sounds: Option<Res<CampaignSounds>>,
    voices: Option<Res<CampaignVoices>>,
    tables: Option<Res<super::BodyTables>>,
    clock: Res<crate::core::SimClock>,
    mut state: ResMut<Chatter>,
    mut bodies: Query<(Entity, &BodyKind, Option<&super::BodyVoice>, &Transform, Option<&Vitality>, Option<&mut Voice>)>,
    mut lines: Option<ResMut<Messages<GameSound>>>,
) {
    let now = clock.now;
    let rules = data.chatter;
    let kills: Vec<Entity> = died.read().filter_map(|d| d.attacker.filter(|k| *k != d.entity)).collect();
    let mut seen = Vec::new();
    for (e, kind, chosen, t, vitality, voice) in &mut bodies {
        let kind = *kind;
        if kind == BodyKind::Spartan {
            continue;
        }
        seen.push(e);
        let alive = vitality.is_none_or(|v| !v.dead);
        let st = state.0.entry(e).or_default();
        // A becoming: another kind (or none) before, or a new life.
        if alive && (st.worn != Some(kind) || !st.alive) && becoming_due(st.worn, kind, st.becoming_at, now, rules.becoming_repeat_seconds) {
            st.pending = true;
        }
        if !alive {
            st.pending = false;
        }
        st.worn = Some(kind);
        st.alive = alive;
        let Some(lists) = data.body(kind).and_then(|d| d.lines.as_ref()) else {
            st.pending = false;
            continue;
        };
        let s = match chosen {
            Some(v) => voices.as_deref().and_then(|s| s.0.get(&(kind, v.0))),
            None => sounds.as_deref().and_then(|s| s.0.get(&kind)),
        };
        // Its voice not in yet: the becoming waits (silent without it).
        let (Some(s), Some(mut voice)) = (s, voice) else { continue };
        let table = tables.as_deref().and_then(|t| t.movement.get(&kind));
        let mut what = None;
        if std::mem::take(&mut st.pending)
            && let Some(slot) = first_filled(s, &lists.becoming)
            && let Some(path) = say(&mut voice, s, slot, CHATTER, head(t, table), lines.as_deref_mut())
        {
            st.becoming_at = Some(now);
            what = Some(("becoming", slot, path));
        }
        let draw = (crate::game::campaign::roll(&[clock.tick, e.to_bits(), 0x6b69_6c6c]) >> 40) as f32 / (1u64 << 24) as f32;
        if alive
            && what.is_none()
            && kills.contains(&e)
            && kill_due(st.kill_at, now, rules.kill_chance, rules.kill_cooldown_seconds, draw)
            && let Some(slot) = first_filled(s, &lists.kill)
            && let Some(path) = say(&mut voice, s, slot, CHATTER, head(t, table), lines.as_deref_mut())
        {
            st.kill_at = Some(now);
            what = Some(("kill", slot, path));
        }
        if let Some((why, slot, path)) = what {
            debug!("halo_ce: the {} body {e} says its {why} line: {} (slot {slot}) `{path}`", kind.id(), slot_name(slot));
        }
    }
    // A character that wears no campaign kind (or is gone) starts afresh.
    state.0.retain(|e, _| seen.contains(e));
}
