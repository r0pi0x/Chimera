//! The local player's profile and party as the shell's bar shows them
//! (`chimera_ui::shell::Party`; docs/plans/active/ugg-ui.md "Signal
//! revision 8", stage 2):
//!
//! - **Avatar** (`cl_avatar`, saved, userinfo: the server shows it in the
//!   lobby's rows): one of the 32 looks (`chimera_ui::avatar::AvatarLook`'s
//!   byte, glyph x 8 + colour), drawn at random on the first run (the cvar
//!   at its unset -1 once the config is read) and saved.
//! - **Away**: the window unfocused is *not looking* (the crossed-out
//!   eye); no key, button or pointer move for `IDLE_SECS` (5 min) is
//!   *idle* (greyed).
//! - **The edge strip**: in the lobby, loaded is Ready (white), still
//!   downloading or loading a hollow strip filling with the share done; in
//!   a match, cobalt; else none.
//! - **The party** is you alone (the leader) until parties exist.
//!
//! `Party` is written only when it changes, so screens rebuild on it.

use bevy::{
    input::{keyboard::KeyboardInput, mouse::MouseMotion},
    prelude::*,
    window::{PrimaryWindow, WindowFocused},
};
use chimera_ui::{
    avatar::{AvatarLook, AvatarMarks, EdgeState, Face},
    shell::{Member, Party},
};

use crate::console::{ConsoleAppExt, Console};

/// `cl_avatar`: the look's byte, -1 unset.
#[derive(Resource, Clone, Copy, Debug, PartialEq, Eq)]
pub struct AvatarSetting(pub i32);

impl Default for AvatarSetting {
    fn default() -> Self {
        AvatarSetting(-1)
    }
}

impl AvatarSetting {
    pub fn look(self) -> Option<AvatarLook> {
        u8::try_from(self.0).ok().and_then(AvatarLook::from_byte)
    }
}

/// Seconds without input before you count as idle.
pub const IDLE_SECS: f64 = 300.0;

/// Away state (module doc).
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq)]
pub struct Presence {
    pub unfocused: bool,
    /// The last input's time (`Time<Real>` seconds).
    pub last_input: f64,
    pub idle: bool,
}

/// What the lobby or the match says about you (set by the screens that
/// know: the lobby view, the match), read into the edge strip.
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq)]
pub struct LocalStatus(pub EdgeState);

pub struct ProfilePlugin;

impl Plugin for ProfilePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<AvatarSetting>()
            .init_resource::<Presence>()
            .init_resource::<LocalStatus>()
            .init_resource::<Party>()
            .add_message::<WindowFocused>()
            .add_message::<KeyboardInput>()
            .add_message::<MouseMotion>();
        crate::console::resource_cvar::<AvatarSetting, i32>(
            app,
            "cl_avatar",
            "Your avatar: glyph x 8 + colour (0-31; glyphs Operator, Trooper, Vanguard, Player; colours Ice, Teal, Lime, Sand, Rose, Orchid, Violet, Steel). -1: drawn at random on the next start.",
            |a| &mut a.0,
        );
        {
            let mut console = app.world_mut().resource_mut::<Console>();
            console.archive("cl_avatar");
            console.userinfo("cl_avatar");
            console.set_range("cl_avatar", -1.0, 31.0);
        }
        let _ = app.console_command("chimera:presence", "presence [away|idle|back]: pretend the window lost focus, you went idle, or came back (screenshots, tests).", |w, a| {
            let mut p = w.resource_mut::<Presence>();
            match a.first().map(String::as_str) {
                Some("away") => p.unfocused = true,
                Some("idle") => p.idle = true,
                Some("back") => *p = Presence { last_input: p.last_input, ..default() },
                _ => {}
            }
            Ok(Some(format!("{:?}", *p)))
        });
        app.add_systems(Update, ((draw_avatar, track_presence, local_status, update_party).chain(), report_tile));
    }
}

/// The first run's draw (module doc): once the cvar is still unset a few
/// frames in (the config has been read by then).
fn draw_avatar(mut setting: ResMut<AvatarSetting>, time: Res<Time<Real>>, mut frames: Local<u32>) {
    *frames += 1;
    if *frames < 3 || setting.0 >= 0 {
        return;
    }
    let seed = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos() as u64)
        ^ time.elapsed().as_nanos() as u64;
    setting.0 = AvatarLook::random(seed).byte() as i32;
}

fn track_presence(
    time: Res<Time<Real>>,
    mut presence: ResMut<Presence>,
    mut focus: MessageReader<WindowFocused>,
    primary: Query<Entity, With<PrimaryWindow>>,
    keys: MessageReader<KeyboardInput>,
    motion: MessageReader<MouseMotion>,
    mouse: Option<Res<ButtonInput<MouseButton>>>,
) {
    let now = time.elapsed_secs_f64();
    let mut p = *presence;
    for f in focus.read() {
        if primary.single().is_ok_and(|w| w == f.window) {
            p.unfocused = !f.focused;
        }
    }
    if !keys.is_empty() || !motion.is_empty() || mouse.is_some_and(|m| m.get_just_pressed().next().is_some()) {
        p.last_input = now;
        p.idle = false;
    } else if now - p.last_input >= IDLE_SECS {
        p.idle = true;
    }
    if p != *presence {
        *presence = p;
    }
}

/// The edge strip's state from where you are: in the lobby loaded
/// (Ready) or still downloading or loading (filling); in a match (a
/// server's or a client's, the lobby gone); else nothing.
fn local_status(
    lobby: Option<Res<super::prematch_lobby::LobbyView>>,
    role: Option<Res<chimera_core::core::NetRole>>,
    mut status: ResMut<LocalStatus>,
) {
    use chimera_net::prematch::status as st;
    let want = match lobby.as_deref().filter(|l| l.shown).and_then(|l| l.rows.iter().find(|r| r.local)) {
        Some(r) if r.status == st::READY || r.status == st::NOT_READY => EdgeState::Ready,
        Some(r) => EdgeState::Loading(r.fraction.unwrap_or(0.0)),
        None if matches!(role.as_deref(), Some(chimera_core::core::NetRole::Client | chimera_core::core::NetRole::Server)) => EdgeState::InMatch,
        None => EdgeState::Idle,
    };
    if status.0 != want {
        status.0 = want;
    }
}

fn update_party(
    setting: Res<AvatarSetting>,
    presence: Res<Presence>,
    status: Res<LocalStatus>,
    net: Option<Res<crate::net::NetSettings>>,
    mut party: ResMut<Party>,
) {
    let name = net.map_or_else(|| "Player".to_string(), |n| if n.name.is_empty() { "Player".into() } else { n.name.clone() });
    let look = setting.look().unwrap_or_else(|| AvatarLook::for_name(&name));
    let me = Member {
        name,
        face: Face::Look(look),
        marks: AvatarMarks { you: true, away: presence.unfocused, idle: presence.idle },
        state: status.0,
        lead: true,
    };
    let want = Party { members: vec![me], max: party.max };
    if *party != want {
        *party = want;
    }
}

/// The shell bar's Report a bug tile, on any screen: the report dialog
/// (`feedback`), as F10.
#[allow(deprecated)]
fn report_tile(
    tiles: Query<(&bevy::ui::Interaction, &chimera_ui::shell::ShellTile), (Changed<bevy::ui::Interaction>, Without<chimera_ui::shell::Disabled>)>,
    mut commands: Commands,
) {
    for (i, t) in &tiles {
        if *i == bevy::ui::Interaction::Pressed && *t == chimera_ui::shell::ShellTile::Report {
            commands.queue(|w: &mut World| {
                if w.contains_resource::<super::feedback::Feedback>() {
                    super::feedback::request(w, true);
                }
            });
        }
    }
}
