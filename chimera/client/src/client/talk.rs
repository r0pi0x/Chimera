//! Who is talking, for the lists (docs/plans/active/ugg-ui.md, M10): a
//! small level-meter mark (`voice`, the kit's icon) beside a player's name
//! on the Tab board (`race_chip`) and the lobby's lanes (`lobby_panel`)
//! while that player's voice is coming in (or is being sent, for you).
//!
//! The source is the voice transport's own state, `net::voice::
//! VoiceActivity` (the same Combat Arms' talk icon and CS:S's voice rows
//! read): `Talking` is who it says is talking now, by user id (`NetCharacter::
//! owner`; a lobby's rows carry the same id) and whether you are. A system
//! that is not the transport sets it by writing `Talking` itself (a test,
//! a scene), when `VoiceActivity` is not in the world: `gather` leaves it
//! alone then. A screen spawns a `TalkMark` hidden, wherever it wants the
//! mark; `show` reveals it while its player talks and hides it again, no
//! rebuild of the list (a row never moves for a talker). A muted player's
//! voice is not heard (`VoiceMutes`), so it is never "talking" here.
//!
//! Bots never talk. `TalkPlugin` is added by the screens that carry marks.

use std::collections::BTreeSet;

use bevy::prelude::*;

use crate::net::{
    NetCharacter,
    voice::VoiceActivity,
};

/// Who is talking now (module doc).
#[derive(Resource, Clone, Debug, Default, PartialEq, Eq)]
pub struct Talking {
    /// User ids heard within `TALK_SECONDS`.
    pub users: BTreeSet<u64>,
    /// You are sending voice.
    pub local: bool,
}

impl Talking {
    /// Whether the player with this user id (`local`: it is you) talks.
    pub fn is(&self, owner: Option<u64>, local: bool) -> bool {
        (local && self.local) || owner.is_some_and(|id| self.users.contains(&id))
    }
}

/// A talk mark on a list: shown while its player talks.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub struct TalkMark {
    /// The player's user id (None: a bot, or not known).
    pub owner: Option<u64>,
    /// The mark is yours.
    pub local: bool,
}

/// The talk indicator's systems.
pub struct TalkPlugin;

impl Plugin for TalkPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Talking>().add_systems(Update, (gather, show).chain());
    }
}

/// `Talking` from the transport's activity, when there is one.
fn gather(activity: Option<Res<VoiceActivity>>, time: Option<Res<Time<Real>>>, characters: Query<&NetCharacter>, mut talking: ResMut<Talking>) {
    let Some(activity) = activity else { return };
    let now = time.map_or(0.0, |t| t.elapsed_secs_f64());
    let users: BTreeSet<u64> = activity.talking(now).into_iter().filter_map(|e| characters.get(e).ok()?.owner).collect();
    if talking.users != users || talking.local != activity.local {
        *talking = Talking { users, local: activity.local };
    }
}

/// The marks show while their players talk.
fn show(talking: Res<Talking>, mut marks: Query<(&TalkMark, &mut Visibility)>) {
    for (m, mut vis) in &mut marks {
        vis.set_if_neq(if talking.is(m.owner, m.local) { Visibility::Inherited } else { Visibility::Hidden });
    }
}
