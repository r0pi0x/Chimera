//! The scenario's teleporter pairs as the map's teleporters
//! (specs/halo_ce/scenario.md, "Netgame flags", "Teleporter pairs";
//! multiplayer_rules.md 6.8; movement.md, "Teleporters"), in the engine's
//! game-neutral form (`map::teleport::MapTeleporters`, the map's typed
//! extra), so every game's characters on a Halo map use them.
//!
//! Each teleporter source flag with a target (`Scenario::teleporter_pairs`:
//! the first target flag in block order with the source's team index)
//! is a pad: its source at the source flag, its destination at the
//! target's position, both through `space::point` (feet), the yaws through
//! `space::yaw` (the look turns by target facing minus source facing, the
//! spec's heading rule). The rule's numbers: a unit within 0.5 WU (3D) of a
//! source is taken (`TELEPORT_RADIUS`), the source it arrives beside stays
//! disarmed until it is more than 1 WU away (`TELEPORT_REARM`), the
//! velocity is kept in world axes (movement.md). The exit's test is the
//! engine's `Exit::Pill` with the spec's table (multiplayer_rules.md 6.8):
//! the arriving pill (the movement's radius, 0.2, and segment, 0.3
//! standing and 0.1 crouched) with its bottom sphere's centre on the
//! target flag blocked by a living character's pill; the deepest blocker's
//! count rises one per refused try and falls one per tick it blocks
//! nobody, and it dies in the tick of the count's 90th
//! (`TELEFRAG_TICKS`); the blocked message on the first refusal and every
//! 121st (`MESSAGE_EVERY`).
//! A source without a target (none on the disc) is no pad.
//!
//! The view is levelled on arrival (`MapTeleporters::level_view`: the
//! pitch becomes 0, 6.8 "View").
//!
//! The local player's effects (6.8 "Local player effects", `effects`): the
//! sound `sfx\ui\teleporter_activate` (unspatialized, the announcer's
//! queue's immediate line 27, `ACTIVATE_SOUND`, compiled with the audio
//! kit) and a screen flash of colour (0.35, 1.0, 0.35) at alpha 0.5 fading
//! over 1 s (`FLASH`), on the machine that shows that player, found by
//! the body's jump between frames (`MapTeleporters::arrived`): no network
//! message of their own. The pads' shimmer is the teleporter scenery's
//! own shader (`maps::generic`).
//!
//! The blocked message (`TeleportersPlugin`, `blocked_message`): each
//! refusal the engine reports (`teleport::TeleportRefused`, on the first
//! and every 121st) shows "Teleporter is blocked" (`BLOCKED_TEXT`, 6.8's
//! text, the executable's) to the refused player: as the shell's hint
//! (`net::Notice`) on the machine that shows it, sent to its client when
//! the player is a remote one.
//!
//! Known shortcuts: every living character blocks (no vehicles yet; the
//! dead never do); a crouch is the footing's (crouched or not: the
//! segment of a crouch in progress is the nearer end's); the blocked
//! message is drawn where the shell draws hints, not on Halo's message
//! lines (its placement and life are multiplayer_rules.md open question
//! 19; Halo's HUD draws no message lines yet).

use bevy::prelude::*;
use blam_formats::scenario::{NetgameFlag, Scenario, netgame};

use super::space;
use crate::map::{
    MapData,
    teleport::{Exit, MapTeleporters, TeleportRefused, Teleporter},
};

/// Refused tries a blocker's count must reach to kill it
/// (multiplayer_rules.md 6.8: the 90th).
pub const TELEFRAG_TICKS: f32 = 90.0;

/// The blocked message's period in refused tries (multiplayer_rules.md
/// 6.8: the first and every 121st).
pub const MESSAGE_EVERY: u32 = 121;

/// The netgame flags' teleporters in the engine's form (module doc).
pub fn teleporters(flags: &[NetgameFlag]) -> MapTeleporters {
    let pads = netgame::teleporter_pairs(flags)
        .into_iter()
        .filter_map(|pair| {
            let source = flags.get(pair.source)?;
            let target = flags.get(pair.target?)?;
            Some(Teleporter {
                source: space::point(source.position),
                source_yaw: space::yaw(source.facing),
                dest: space::point(target.position),
                dest_yaw: space::yaw(target.facing),
            })
        })
        .collect();
    MapTeleporters {
        pads,
        radius: netgame::TELEPORT_RADIUS * space::METERS_PER_UNIT,
        rearm: netgame::TELEPORT_REARM * space::METERS_PER_UNIT,
        keep_velocity: true,
        telefrag: None,
        exit: exit(),
        level_view: true,
    }
}

/// The sound a teleported local player hears (multiplayer_rules.md 9,
/// line 27).
pub const ACTIVATE_SOUND: &str = r"sound\sfx\ui\teleporter_activate";

/// The teleported local player's flash (6.8): colour (0.35, 1.0, 0.35) as
/// bytes, alpha 0.5, seconds.
pub const FLASH: ([u8; 3], u8, f32) = ([89, 255, 89], 128, 1.0);

/// How far (m) a body's jump may differ from a pad's and still be its
/// (`MapTeleporters::arrived`: what it walked between two frames).
pub const ARRIVAL_SLACK: f32 = 1.0;

/// The teleported local player's sound and flash (module doc), on a Halo
/// map with teleporters.
#[allow(clippy::too_many_arguments)]
pub fn effects(
    pads: Option<Res<MapTeleporters>>,
    game: Option<Res<crate::core::MapGame>>,
    me: Query<(Entity, &Transform), With<crate::core::LocalPlayer>>,
    mut last: Local<Option<(Entity, Vec3)>>,
    time: Res<Time>,
    mut sounds: MessageWriter<crate::map::PlaySound>,
    fades: Option<ResMut<crate::map::logic_view::ScreenFades>>,
) {
    let Ok((e, t)) = me.single() else {
        *last = None;
        return;
    };
    let now = t.translation;
    let before = last.replace((e, now)).filter(|(was, _)| *was == e).map(|(_, p)| p);
    let (Some(pads), Some(before)) = (pads, before) else { return };
    if game.is_none_or(|g| g.0 != crate::game::GAME) {
        return;
    }
    let half = Vec3::Y * chimera_core::character::CAPSULE_HEIGHT / 2.0;
    if pads.arrived(before, now, half, ARRIVAL_SLACK).is_none() {
        return;
    }
    sounds.write(crate::map::PlaySound::ui(crate::game::fx::name_of(ACTIVATE_SOUND)));
    if let Some(mut fades) = fades {
        let (color, alpha, seconds) = FLASH;
        fades.show(
            crate::map::logic_view::ScreenFade {
                duration: seconds,
                hold: 0.0,
                color,
                alpha,
                fade_in: true,
                modulate: false,
                stay_out: false,
                purge: false,
            },
            time.elapsed_secs_f64(),
        );
    }
}

/// What a player whose exit is blocked is shown (multiplayer_rules.md
/// 6.8, "Blocked message").
pub const BLOCKED_TEXT: &str = "Teleporter is blocked";

/// The blocked message (module doc): the engine's refusals as hints.
pub struct TeleportersPlugin;

impl Plugin for TeleportersPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<TeleportRefused>().add_systems(Update, blocked_message);
    }
}

/// Each refused player is shown `BLOCKED_TEXT`: a local one by its
/// machine's hint (`Notice`, when it draws them), a remote one's client
/// sent it (`net::game::notify`).
pub fn blocked_message(
    mut refused: MessageReader<TeleportRefused>,
    local: Query<(), With<crate::core::LocalPlayer>>,
    players: Query<(Entity, &chimera_net::net::server::Player)>,
    notices: Option<ResMut<Messages<chimera_net::net::Notice>>>,
    mut commands: Commands,
) {
    let mut notices = notices;
    for r in refused.read() {
        if local.contains(r.character) {
            if let Some(n) = notices.as_mut() {
                n.write(chimera_net::net::Notice { text: BLOCKED_TEXT.into() });
            }
        } else if let Some((client, _)) = players.iter().find(|(_, p)| p.character == r.character) {
            commands.queue(move |w: &mut World| chimera_net::net::game::notify(w, client, BLOCKED_TEXT.into()));
        }
    }
}

/// Halo's exit test (module doc), in metres.
pub fn exit() -> Exit {
    let d = crate::game::movement::MovementData::builtin();
    let m = space::METERS_PER_UNIT;
    Exit::Pill {
        radius: d.radius * m,
        standing: d.segment(0.0) * m,
        crouched: d.segment(1.0) * m,
        tick: crate::game::TICK_INTERVAL as f32,
        telefrag_ticks: TELEFRAG_TICKS,
        message_every: MESSAGE_EVERY,
    }
}

/// `map`'s teleporters from the scenario (none: no extra).
pub fn add(scenario: &Scenario, map: &mut MapData) {
    let t = teleporters(&scenario.netgame_flags);
    if !t.pads.is_empty() {
        map.extras.insert(t);
    }
}
