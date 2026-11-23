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
//! - **The party**: you first, then the hub party's others in join
//!   order (`chimera_net::net::party::PartyClient`; offline ones away, on a
//!   server in-match); you alone without one. The "+" is live with a
//!   hub account (`Party::invite`). Your name and avatar go to the party's
//!   hub (`PartyMe`).
//!
//! A member's preview card on hover (`chimera_ui::shell::Tip::card`) shows
//! the display name and `@username` (yours from `AccountNames`, the
//! others' as the hub has them from their tickets: `others_names`) and
//! the role (Host, Party leader) and, for a member on the lobby's server with a
//! round trip (not you), the ping, live (`update_pings`: `MemberPings`, apart
//! from `Party`, which a ping would rebuild every second).
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
            .init_resource::<AccountNames>()
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
        app.add_systems(Update, ((draw_avatar, track_presence, local_status, update_party, update_pings).chain(), report_tile));
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

/// The signed-in account's username and display name (the app writes it
/// from the hub account), for your own preview card in the party strip.
#[derive(Resource, Clone, Debug, Default, PartialEq, Eq)]
pub struct AccountNames(pub Option<(String, String)>);

#[allow(clippy::too_many_arguments)]
fn update_party(
    setting: Res<AvatarSetting>,
    presence: Res<Presence>,
    status: Res<LocalStatus>,
    net: Option<Res<crate::net::NetSettings>>,
    hub_party: Option<Res<chimera_net::net::party::PartyClient>>,
    mut party_me: ResMut<chimera_net::net::party::PartyMe>,
    mut party: ResMut<Party>,
    lobby: Option<Res<super::prematch_lobby::LobbyView>>,
    account: Option<Res<AccountNames>>,
) {
    let name = net.map_or_else(|| "Player".to_string(), |n| if n.name.is_empty() { "Player".into() } else { n.name.clone() });
    let look = setting.look().unwrap_or_else(|| AvatarLook::for_name(&name));
    // Who we are to the party's hub.
    let who = chimera_net::net::party::Me { name: name.clone(), avatar: setting.look().map_or(255, |l| l.byte()) };
    if party_me.0 != who {
        party_me.0 = who;
    }
    let view = hub_party.as_deref().and_then(|p| p.view.as_ref());
    let (shown_name, display) = match account.as_deref().and_then(|a| a.0.as_ref()) {
        Some((user, display)) => (user.clone(), Some(display.clone()).filter(|d| d != user)),
        None => (name, None),
    };
    let me = Member {
        name: shown_name,
        face: Face::Look(look),
        marks: AvatarMarks { you: true, away: presence.unfocused, idle: presence.idle },
        state: status.0,
        lead: view.is_none_or(|v| v.leading()),
        display,
        role: lobby.as_deref().filter(|l| l.shown).and_then(|l| l.rows.iter().find(|r| r.local)).filter(|r| r.host).map(|_| "Host".to_string()),
        // A ping changes every second: it would rebuild every screen.
        ping: None,
    };
    let mut members = vec![me];
    // The party's others, in join order (the hub's), offline marked away.
    for m in view.map(|v| v.members.iter().filter(|m| m.id != v.you).collect::<Vec<_>>()).unwrap_or_default() {
        let look = AvatarLook::from_byte(m.avatar).unwrap_or_else(|| AvatarLook::for_name(&m.name));
        // Its account's names (its ticket's), as yours: `@username` and
        // the display name; an older hub's, the name its client gave.
        let (name, display) = others_names(m);
        members.push(Member {
            name,
            face: Face::Look(look),
            marks: AvatarMarks { you: false, away: !m.online, idle: false },
            state: if m.at.is_some() { EdgeState::InMatch } else { EdgeState::Idle },
            lead: m.leader,
            display,
            role: m.leader.then(|| "Party leader".to_string()),
            ping: None,
        });
    }
    let want = Party { members, max: party.max, invite: hub_party.is_some_and(|p| p.available()) };
    if *party != want {
        *party = want;
    }
}

/// The pings of the party's members that are on the lobby's server (their
/// row by the name their client gave; remote rows only: the local one has
/// none), keyed as the card's member is (`others_names`).
fn update_pings(
    hub_party: Option<Res<chimera_net::net::party::PartyClient>>,
    lobby: Option<Res<super::prematch_lobby::LobbyView>>,
    mut pings: ResMut<chimera_ui::shell::MemberPings>,
) {
    let mut want = bevy::platform::collections::HashMap::new();
    if let (Some(v), Some(l)) = (hub_party.as_deref().and_then(|p| p.view.as_ref()), lobby.as_deref().filter(|l| l.shown)) {
        for m in v.members.iter().filter(|m| m.id != v.you && m.online) {
            if let Some(ms) = l.rows.iter().find(|r| !r.local && !r.bot && r.name == m.name).and_then(|r| r.ping_ms) {
                want.insert(others_names(m).0, ms);
            }
        }
    }
    if pings.0 != want {
        pings.0 = want;
    }
}

/// A party member's card names: its username and, when it differs, its
/// display name (the hub's from its ticket); from an older hub, the name
/// its client gave and none.
pub fn others_names(m: &chimera_net::net::party::MemberView) -> (String, Option<String>) {
    if m.username.is_empty() {
        return (m.name.clone(), None);
    }
    (m.username.clone(), Some(m.display_name.clone()).filter(|d| !d.is_empty() && *d != m.username))
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

#[cfg(test)]
mod tests {
    use chimera_net::net::party::MemberView;

    use super::others_names;

    #[test]
    fn a_party_members_card_shows_its_accounts_names() {
        let m = |name: &str, user: &str, display: &str| MemberView { name: name.into(), username: user.into(), display_name: display.into(), ..Default::default() };
        assert_eq!(others_names(&m("Cleo", "cleo", "Cleo Nova")), ("cleo".into(), Some("Cleo Nova".into())));
        assert_eq!(others_names(&m("Cleo", "cleo", "cleo")), ("cleo".into(), None), "the same: once");
        assert_eq!(others_names(&m("Cleo", "", "")), ("Cleo".into(), None), "an older hub's: the client's name");
    }
}
