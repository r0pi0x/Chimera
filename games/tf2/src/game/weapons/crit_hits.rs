//! A crit's and a mini-crit's feedback (specs/tf2/batch2b_mechanics.md 18,
//! weapons_stock.md 8.3): each crit or mini-crit hit (`CritHit`, written
//! by the conditions: `items::effects::crits` and a blast's,
//! `blast::BlastWorld::detonate`) plays its hit sound at the victim
//! (`TFPlayer.CritHit`, `TFPlayer.CritHitMini`; the server's, as every
//! game sound), and its attacker sees its floating text over the victim
//! (`CritText`: `crit_text` or `minicrit_text` of `particles/crit.pcf`,
//! whose numbers are the file's: one sprite 24 above the point, locked to
//! it, rising, 2 s, fading out over its second half, green for a crit and
//! yellow for a mini-crit, easing to red). The text goes to the attacker's
//! own client (a host's own player: here) and is drawn by every machine
//! that draws (`draw`).
//!
//! Known shortcuts: the point is the victim's eyes (where the game puts
//! it, head or centre, was not measured); only the attacker sees the
//! text (the spec leaves the audience open; the usual TF2 behaviour).

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use crate::core::{LocalPlayer, MovementState};
use crate::map::{GameSound, PlaySound, psys::StartParticleSystem};

/// The hit sounds.
pub const CRIT_HIT: &str = super::melee::CRIT_HIT;
pub const CRIT_HIT_MINI: &str = "tf2:TFPlayer.CritHitMini";
/// The texts' systems and their file.
pub const CRIT_TEXT: &str = "crit_text";
pub const MINICRIT_TEXT: &str = "minicrit_text";
pub const FILE: &str = "particles/crit.pcf";

/// A crit (or mini-crit) hit on the server: who on whom, where it hit,
/// and whether its sound plays here (a melee crit swing plays its own).
#[derive(Message, Clone, Copy, Debug)]
pub struct CritHit {
    pub attacker: Entity,
    pub victim: Entity,
    pub at: Vec3,
    pub mini: bool,
    pub sound: bool,
}

/// The attacker's text over the victim (to his client).
#[derive(Message, Clone, Copy, Debug, Serialize, Deserialize)]
pub struct CritText {
    pub at: Vec3,
    pub mini: bool,
}

impl CritText {
    pub fn system(&self) -> &'static str {
        if self.mini { MINICRIT_TEXT } else { CRIT_TEXT }
    }
}

/// The server: each crit hit's sound and its attacker's text.
#[allow(clippy::type_complexity)]
fn feedback(
    mut hits: MessageReader<CritHit>,
    victims: Query<(&Transform, Option<&MovementState>)>,
    local: Query<(), With<LocalPlayer>>,
    mut sounds: MessageWriter<GameSound>,
    mut texts: MessageWriter<CritText>,
    mut out: MessageWriter<TextFor>,
) {
    for h in hits.read() {
        if h.sound {
            let name = if h.mini { CRIT_HIT_MINI } else { CRIT_HIT };
            sounds.write(GameSound(PlaySound { source: Some(h.victim), ..PlaySound::at(name, h.at) }));
        }
        let at = victims.get(h.victim).map_or(h.at, |(t, s)| t.translation + s.map_or(Vec3::ZERO, |s| s.eye_offset));
        let text = CritText { at, mini: h.mini };
        if local.contains(h.attacker) {
            texts.write(text);
        }
        out.write(TextFor { attacker: h.attacker, text });
    }
}

/// A text for an attacker's client (the server's, `forward` sends it).
#[derive(Message, Clone, Copy, Debug)]
pub struct TextFor {
    pub attacker: Entity,
    pub text: CritText,
}

/// The server: each text to its attacker's client.
fn forward(
    mut texts: MessageReader<TextFor>,
    players: Query<(Entity, &chimera_net::net::server::Player)>,
    mut out: MessageWriter<bevy_replicon::prelude::ToClients<CritText>>,
) {
    use bevy_replicon::prelude::*;
    for t in texts.read() {
        for (client, _) in players.iter().filter(|(_, p)| p.character == t.attacker) {
            out.write(ToClients { targets: SendTargets::Single(ClientId::Client(client)), message: t.text });
        }
    }
}

/// Every machine that draws: the texts.
fn draw(mut texts: MessageReader<CritText>, mut start: MessageWriter<StartParticleSystem>) {
    for t in texts.read() {
        start.write(StartParticleSystem::new(t.system(), t.at, Vec3::Z));
    }
}

fn register(app: &mut App) {
    use crate::core::NetRole;
    use bevy_replicon::prelude::*;
    app.add_server_message::<CritText>(Channel::Unreliable)
        .add_systems(PostUpdate, forward.before(ServerSystems::Send).run_if(resource_equals(NetRole::Server)));
}

/// The feedback (from `weapons::Tf2GunsPlugin`).
pub fn add(app: &mut App) {
    use chimera_net::net::parts::NetAppExt;
    app.add_message::<CritHit>()
        .add_message::<CritText>()
        .add_message::<TextFor>()
        .add_message::<GameSound>()
        .add_message::<StartParticleSystem>()
        .add_systems(FixedUpdate, feedback.after(crate::core::apply_damage).run_if(crate::core::authoritative))
        .add_systems(
            Update,
            draw.run_if(not(resource_exists::<chimera_net::dedicated::Serving>)).before(crate::map::psys::StartedSystemsRun),
        )
        .net_part("tf2_crit_text", register);
}
