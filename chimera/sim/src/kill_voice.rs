//! Kill voices for any game (docs/DESIGN.md, "Kill voices"): on a kill,
//! the killer's character speaks a kill line of its own game or class,
//! if its game gives it one, and a killer whose character says nothing
//! hears the minigame's rung-up sound instead (`KillVoiceOutcome`, which
//! the client shell plays: `chimera_client::client::callouts`).
//!
//! - **Whose line**: the character the killer was at the start of the
//!   kill's frame (`at_kill`: what it is now, unless it changed in the
//!   frame's ticks, then what it was before: `CharacterAtTick`, noted by observers when
//!   either part is added or replaced, no per-tick work): its `GameIdentity` and
//!   its `CharacterVoice` (a
//!   game's name for the voice set it speaks with: a TF2 class, Combat
//!   Arms' sound template). A rung up that swaps the killer's character on
//!   the kill's tick (after the deaths), or right after, changes neither:
//!   the line chosen is the old character's, and once started it is a
//!   sound of its own, not re-chosen.
//! - **The server** (`ask`, PostUpdate, once a frame after its ticks):
//!   each death by
//!   another character's hand becomes a `KillVoiceAsk` with that
//!   character. Each game reads the asks for its characters
//!   (`KillVoiceAsk::is_for`) in `KillVoiceSet::Answer` (PostUpdate),
//!   plays its line by its own rules (its own sound path, so the network
//!   carries it as that game's sounds do) and writes `KillVoiced` for a
//!   kill it spoke for (a line chosen, even one that starts later).
//!   `settle` (`KillVoiceSet::Settle`, before the announcer and the
//!   network's kills) writes one `KillVoiceOutcome` per ask; a network
//!   server sends it with the kill (`net::Killed::marks`), and a client
//!   writes it again.
//! - **No game** answers for a character without a kill line (CS:S's,
//!   HL2:DM's and Halo's players say nothing on a kill): its outcome is
//!   unvoiced.

use bevy::prelude::*;

use crate::{
    core::{Damage, Died, Health},
    slots::GameIdentity,
};

/// A game's name for the voice set a character speaks with (a TF2
/// class's name, Combat Arms' sound template): kept current by the game
/// that owns the character, noted with each damage it deals.
#[derive(Component, Clone, Debug, PartialEq, Eq)]
pub struct CharacterVoice(pub String);

/// A killer's character when its damage was dealt: its game (None: no
/// identity, the app's game) and voice set.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct KillerCharacter {
    pub game: Option<String>,
    pub voice: Option<String>,
}

/// A character as it was before it changed on tick `tick` (`SimTick`;
/// the first change of that tick kept): what it speaks a kill line of
/// that tick as (`at_kill`).
#[derive(Component, Clone, Debug, Default, PartialEq, Eq)]
pub struct CharacterAtTick {
    pub tick: u64,
    pub character: KillerCharacter,
}

/// The character a kill made after tick `since` is spoken as: the one
/// before a change after that tick, else the one now.
pub fn at_kill(
    at_tick: Option<&CharacterAtTick>,
    identity: Option<&GameIdentity>,
    voice: Option<&CharacterVoice>,
    since: u64,
) -> KillerCharacter {
    match at_tick {
        Some(c) if c.tick > since => c.character.clone(),
        _ => now_of(identity, voice),
    }
}

/// The server asks the killer's game for a kill line (module doc).
#[derive(Message, Clone, Debug)]
pub struct KillVoiceAsk {
    pub killer: Entity,
    pub victim: Entity,
    /// The killer's character at the kill.
    pub character: KillerCharacter,
    /// The killing damage.
    pub damage: Damage,
}

impl KillVoiceAsk {
    /// Whether game `game` speaks for this kill's character: its game, or
    /// a character of no identity (a plain app's).
    pub fn is_for(&self, game: &str) -> bool {
        self.character.game.as_deref().is_none_or(|g| g.eq_ignore_ascii_case(game))
    }
}

/// A game spoke for a kill (a line chosen for its killer).
#[derive(Message, Clone, Copy, Debug, PartialEq, Eq)]
pub struct KillVoiced {
    pub killer: Entity,
    pub victim: Entity,
}

/// What a kill's killer said: `voiced` a line of its character's, else
/// nothing (its player hears the rung-up sound). On the server as
/// settled, on a network client as sent with the kill.
#[derive(Message, Clone, Copy, Debug, PartialEq, Eq)]
pub struct KillVoiceOutcome {
    pub killer: Entity,
    pub victim: Entity,
    pub voiced: bool,
}

/// Where games answer the asks, then the outcomes settle (PostUpdate,
/// before the announcer's counting and the network's kills).
#[derive(SystemSet, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum KillVoiceSet {
    Ask,
    Answer,
    Settle,
}

pub struct KillVoicePlugin;

impl Plugin for KillVoicePlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<Died>()
            .add_message::<KillVoiceAsk>()
            .add_message::<KillVoiced>()
            .add_message::<KillVoiceOutcome>()
            .configure_sets(
                PostUpdate,
                (KillVoiceSet::Ask, KillVoiceSet::Answer, KillVoiceSet::Settle)
                    .chain()
                    .before(chimera_core::announcer::AnnounceSet),
            )
            .add_observer(identity_replaced)
            .add_observer(voice_replaced)
            .add_observer(identity_added)
            .add_observer(voice_added)
            // Once a frame, after its ticks (as the dominations count): a
            // system in the fixed tick reorders the tick's unordered pairs.
            .add_systems(PostUpdate, ask.in_set(KillVoiceSet::Ask).run_if(crate::core::authoritative))
            .add_systems(PostUpdate, settle.in_set(KillVoiceSet::Settle).run_if(crate::core::authoritative));
    }
}

/// The server: each death by another character's hand, asked of the
/// killer's game with the killer's character at the kill.
pub fn ask(
    mut died: MessageReader<Died>,
    characters: Query<(Option<&CharacterAtTick>, Option<&GameIdentity>, Option<&CharacterVoice>), With<Health>>,
    tick: Option<Res<crate::core::SimTick>>,
    mut seen: Local<Option<u64>>,
    mut out: MessageWriter<KillVoiceAsk>,
) {
    // This frame's ticks are those after the last one seen.
    let tick = tick.map_or(0, |t| t.0);
    let since = seen.replace(tick).unwrap_or(tick.saturating_sub(1));
    for d in died.read() {
        let Some(killer) = d.attacker.filter(|a| *a != d.entity) else { continue };
        let Ok((at_tick, identity, voice)) = characters.get(killer) else { continue };
        let character = at_kill(at_tick, identity, voice, since);
        out.write(KillVoiceAsk {
            killer,
            victim: d.entity,
            character,
            damage: d.damage.clone(),
        });
    }
}

/// The server: one outcome per ask, voiced when a game spoke for it.
pub fn settle(
    mut asks: MessageReader<KillVoiceAsk>,
    mut voiced: MessageReader<KillVoiced>,
    mut out: MessageWriter<KillVoiceOutcome>,
) {
    let spoken: Vec<KillVoiced> = voiced.read().copied().collect();
    for a in asks.read() {
        out.write(KillVoiceOutcome {
            killer: a.killer,
            victim: a.victim,
            voiced: spoken.iter().any(|v| v.killer == a.killer && v.victim == a.victim),
        });
    }
}

/// The character `identity` and `voice` make now.
pub fn now_of(identity: Option<&GameIdentity>, voice: Option<&CharacterVoice>) -> KillerCharacter {
    KillerCharacter {
        game: identity.map(|i| i.0.clone()),
        voice: voice.map(|v| v.0.clone()),
    }
}

/// Note `e`'s character as it is (before a change) unless already noted
/// this tick.
fn note_before(
    commands: &mut Commands,
    e: Entity,
    at_tick: Option<&CharacterAtTick>,
    character: KillerCharacter,
    tick: u64,
) {
    if at_tick.is_some_and(|c| c.tick == tick) {
        return;
    }
    commands.entity(e).try_insert(CharacterAtTick { tick, character });
}

/// A character's game is about to change (a rung up's swap).
fn identity_replaced(
    on: On<Discard<GameIdentity>>,
    characters: Query<(&GameIdentity, Option<&CharacterVoice>, Option<&CharacterAtTick>)>,
    tick: Option<Res<crate::core::SimTick>>,
    mut commands: Commands,
) {
    let e = on.event_target();
    let Ok((identity, voice, at_tick)) = characters.get(e) else { return };
    let tick = tick.map_or(0, |t| t.0);
    note_before(&mut commands, e, at_tick, now_of(Some(identity), voice), tick);
}

/// A character's voice set is about to change (a TF2 class, a look).
fn voice_replaced(
    on: On<Discard<CharacterVoice>>,
    characters: Query<(&CharacterVoice, Option<&GameIdentity>, Option<&CharacterAtTick>)>,
    tick: Option<Res<crate::core::SimTick>>,
    mut commands: Commands,
) {
    let e = on.event_target();
    let Ok((voice, identity, at_tick)) = characters.get(e) else { return };
    let tick = tick.map_or(0, |t| t.0);
    note_before(&mut commands, e, at_tick, now_of(identity, Some(voice)), tick);
}

/// A character gets a game for the first time (it was the app's).
fn identity_added(
    on: On<Add<GameIdentity>>,
    characters: Query<(Option<&CharacterVoice>, Option<&CharacterAtTick>)>,
    tick: Option<Res<crate::core::SimTick>>,
    mut commands: Commands,
) {
    let e = on.event_target();
    let Ok((voice, at_tick)) = characters.get(e) else { return };
    let tick = tick.map_or(0, |t| t.0);
    note_before(&mut commands, e, at_tick, now_of(None, voice), tick);
}

/// A character gets a voice set for the first time.
fn voice_added(
    on: On<Add<CharacterVoice>>,
    characters: Query<(Option<&GameIdentity>, Option<&CharacterAtTick>)>,
    tick: Option<Res<crate::core::SimTick>>,
    mut commands: Commands,
) {
    let e = on.event_target();
    let Ok((identity, at_tick)) = characters.get(e) else { return };
    let tick = tick.map_or(0, |t| t.0);
    note_before(&mut commands, e, at_tick, now_of(identity, None), tick);
}
