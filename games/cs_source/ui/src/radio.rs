//! The team radio: the three radio menus (Z, X, C, or `radio1`..`radio3`)
//! picked with the number keys, a console command per call (`coverme`,
//! `enemyspot`, ... from `map::radio::RadioCommands`), and what
//! `core::Radio` calls sound like to the local player: the call's sound
//! entry and a "Name (RADIO): text" chat line, for their own calls and
//! living teammates', and the radio icon over a teammate's head while
//! they speak (the game's `sprites/radio`, else a text marker), hidden
//! behind walls as the game's depth-tested material is. With
//! `party_voice_lines_3d 1` (`map::sound::PartyVoices`: the party apps'
//! default) a living non-teammate's call within a talking voice's reach
//! (`sound::TALKING`, 76 m by `send_range`) plays in 3D at the caller
//! (its source, on the voice channel) at that level (`SoundBank::at_level`), with no chat
//! line or icon.
//!
//! The team radio: menus (Z/X/C or `radio1`-`radio3`, from
//! `map::radio::RadioCommands`) picked with number keys, a console command per
//! call (`coverme`, `enemyspot`, ...: registered when a map's radio loads)
//! writing `core::Radio`; calls the local player hears (their own, living
//! teammates') play their sound entry and print "Name (RADIO): text" (with the
//! sender's nav place) to the chat (with `party_voice_lines_3d 1` others'
//! calls within 76 m play in 3D at them, no line or icon), and a teammate's call shows the radio icon
//! over their head for 1.5 s (`RadioIcons`: the HUD's `radio` sprite,
//! `sprites/radio`, else a text marker; hidden behind walls and bodies, as its
//! material is depth-tested); `ignorerad 1` drops teammates' calls (no sound,
//! line or icon; your own still play); `RadioHearPlugin` is the hearing part
//! without a window (tests). The buy, team and radio menus close each other.
//!
//! Known shortcuts (docs/tech-debt.md): Radio (no spec; from the game's data
//! files and in-game behaviour): the pairing of console names, menu keys, sound
//! entries and text keys is ours (following the menus' order);
//! "Affirmative/Roger" picks one of its two at random; dead players can't call
//! and calls are heard by the caller and living or dead teammates; no
//! per-player rate limit; `ignorerad 1` drops teammates' calls (sound, line and
//! icon) but not your own (a guess); the radio icon over a teammate stays 1.5 s
//! (a guess), sized as 16 units across, hidden by a ray from the camera to it
//! (world, props and other bodies; its material is depth-tested), not a depth
//! test; the location form (`Game_radio_location`) is used whenever the caller
//! stands in a named nav place, with the raw place name. Chat colours: team
//! colours from the scheme; the normal (white) and location (light green)
//! colours are guesses. Chat lines stay 12 s (`hud_saytext_time`'s default), at
//! most 8; hints stay 4 s (a guess); both use Bevy's font at the scheme's sizes
//! (Verdana is a system font, not in the install). Bots' radio habits (enemy
//! spotted on first sight, enemy down after a kill, need backup at 40% health)
//! and their cooldowns (12 s per bot and call, 4 s team-wide for the same call)
//! are ours, and so are the calls bots start on their own (no bot chatter spec;
//! `bot::radio::speak`): the attackers' group leader says "Go go go" (a third
//! of rounds, from the team's plan seed and the round) or "Stick together team"
//! 1 s after the round opens (after the freeze; within 10 s, with a teammate
//! within 30 m), "Follow me" once the group has gathered before the site and
//! goes in (with a teammate within 30 m not already following it; not with a
//! bomb down), "Cover me" while planting or defusing with a teammate within 30
//! m, "Sector clear" 5 s after the last enemy seen or heard following a fight,
//! "In position" from the first bot of a team holding a spot, "Regroup team"
//! from the lowest-numbered survivor once a team of 3 or more bots is down to a
//! third (two or more alive, not fighting, no bomb down; never "Team fall
//! back", which sends bots back to spawn); round calls, regroup and in position
//! once a round per team, follow me and cover me once a round per bot; a team's
//! bots start one call per 3 s at most (answers and "Fire in the hole" not
//! counted; what happened (enemy spotted or down, need backup) waits up to 3 s
//! for its turn, a lasting state's call comes when it fits), most urgent first
//! (enemy down, spotted, need backup, cover me, regroup, round calls, follow
//! me, sector clear, in position), bot number breaking ties. Bots carry out
//! each other's commands through `obey` like a person's; answers are never
//! commands, so nothing answers an answer. The install's radio script
//! (`game_sounds_radio.txt`) has bomb sounds only as the round's announcements
//! (`Event.BombPlanted`, `Event.BombDefused`), no team call about the bomb, so
//! bots don't call it. Bots answering commands (no bot behaviour spec): one or
//! two of the nearest teammates answer "Roger that"/"Affirmative" 0.5-1.2 s
//! later (from the bot's seed and the time) and 0.8 s apart, "Negative" from a
//! busy one (fighting, planting/defusing, carrying the bomb, sent) nearer than
//! the one that follows; follow me: the nearest free bot within 30 m follows 40
//! s (cover me 20 s), walking when over 3.5 m away; stick together: every free
//! bot within 30 m follows; hold position / get in position: free bots within
//! 30 m stay within 4 m of the caller's spot for 30 s; regroup: every free bot
//! walks to the caller's spot (until within 4 m, at most 30 s); fall back:
//! every free bot walks to the team's spawn point nearest its spawns' middle
//! for up to 15 s (get out of there: those within 30 m); go / take the point /
//! storm the front: orders dropped, what they chased forgotten, no waiting for
//! the group for 20 s, and storming defenders hunt; report in: the two nearest
//! say "Reporting in"; need backup / taking fire from a person: one bot
//! `tactics` sent answers; orders come before chasing what was seen or heard,
//! after a fight and the objective; bots ignore each other's answers. Team join
//! lines (`Cstrike_game_join_*`) show for every character getting or changing
//! its team once the map's strings are loaded, bots included, in the normal
//! colour. C is the radio responses menu now, not a second crouch key. Text
//! chat: the typing line ("Say :", the engine's `chat_say` strings) sits under
//! the chat box in our look; team chat uses the place form whenever the sender
//! stands in a named nav place; the living don't read the dead (CS:S's default,
//! unmeasured); bots never chat. Why it's OK for now: Works and reads like the
//! game; a spec session can measure what CS:S does.

use bevy::{prelude::*, window::CursorOptions};

use crate::client::chat::ChatLine;
use crate::{
    console::{Command, Console, ConsoleAppExt},
    core::{Health, LocalPlayer, Radio, Team},
    map::{
        PlaySound,
        nav::NavMesh,
        radio::{RadioCommands, hears},
        sound::{PartyVoices, SoundBank, TALKING, send_range},
    },
};

/// The voice channel (Source's CHAN_VOICE) a caller's call plays on when
/// heard in 3D under `party_voice_lines_3d`: a new call replaces their last.
pub const RADIO_CHANNEL: u8 = 2;

pub struct RadioPlugin;

impl Plugin for RadioPlugin {
    fn build(&self, app: &mut App) {
        use crate::client::binds::BindsAppExt;
        for i in 1..=3 {
            app.polled_bind(&format!("radio{i}"));
        }
        app.add_plugins(RadioHearPlugin)
            .init_resource::<RadioMenu>()
            .add_systems(Update, ((keys, draw).chain(), draw_icons.after(hear)));
        crate::client::front_end::sync_panel::<RadioMenu, _>(app, "radio", true, |m| m.0.is_some(), draw);
        for n in 1..=3usize {
            let help = ["Radio commands", "Group radio commands", "Radio responses"][n - 1];
            app.console_command(
                &format!("radio{n}"),
                &format!("Open the {help} menu ({}).", ["Z", "X", "C"][n - 1]),
                move |w, _| {
                    open(w, Some(n - 1));
                    Ok(None)
                },
            );
        }
    }
}

/// What the local player hears of the radio, without a window (tests):
/// the calls' sounds, chat lines and icons, the radio's console commands,
/// and `ignorerad`.
pub struct RadioHearPlugin;

impl Plugin for RadioHearPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<RadioIcons>()
            .init_resource::<RadioSettings>()
            .add_message::<Radio>()
            .add_message::<ChatLine>()
            .add_message::<PlaySound>()
            .add_systems(
                Update,
                (
                    register_commands.run_if(resource_exists_and_changed::<RadioCommands>),
                    hear,
                ),
            );
        crate::console::resource_cvar::<RadioSettings, u8>(
            app,
            "ignorerad",
            "1: don't show or play teammates' radio calls (your own still play).",
            |s| &mut s.ignore,
        );
    }
}

/// The local player's radio settings.
#[derive(Resource, Default, Debug, Clone)]
pub struct RadioSettings {
    /// `ignorerad`: teammates' calls aren't shown or played.
    pub ignore: u8,
}

/// Which radio menu is open (0-based).
#[derive(Resource, Default, Debug, Clone, Copy, PartialEq)]
pub struct RadioMenu(pub Option<usize>);

/// Open (or with the same menu open, close) a radio menu, closing the
/// other menus.
fn open(w: &mut World, menu: Option<usize>) {
    let mut m = w.resource_mut::<RadioMenu>();
    m.0 = if m.0 == menu { None } else { menu };
    if m.0.is_some() {
        close_others(w);
    }
}

/// Close the buy and team menus (a radio menu opened).
fn close_others(w: &mut World) {
    if let Some(mut b) = w.get_resource_mut::<super::buy_menu::BuyMenu>() {
        *b = default();
    }
    if let Some(mut t) = w.get_resource_mut::<super::team_menu::TeamMenu>() {
        t.0 = false;
    }
}

/// A console command per menu call, as the map's radio names them.
fn register_commands(radio: Res<RadioCommands>, mut console: ResMut<Console>) {
    for c in radio.commands.iter().filter(|c| c.menu.is_some()) {
        let name = c.command.clone();
        let label = c.variants.first().map(|v| v.1.clone()).unwrap_or_default();
        console.add_command(Command {
            name: name.clone(),
            help: format!("Radio: \"{label}\" to your team."),
            run: std::sync::Arc::new(move |w, _| {
                // A network client asks the server (who hears it, and we
                // hear our own call when it comes back).
                if w.get_resource::<crate::core::NetRole>() == Some(&crate::core::NetRole::Client) {
                    w.write_message(crate::net::RadioRequest { command: name.clone() });
                    return Ok(None);
                }
                let player = w
                    .query_filtered::<Entity, With<LocalPlayer>>()
                    .iter(w)
                    .next()
                    .ok_or("no local player")?;
                w.write_message(Radio {
                    sender: player,
                    command: name.clone(),
                });
                Ok(None)
            }),
            complete: None,
        });
    }
}

const DIGITS: [KeyCode; 10] = [
    KeyCode::Digit0,
    KeyCode::Digit1,
    KeyCode::Digit2,
    KeyCode::Digit3,
    KeyCode::Digit4,
    KeyCode::Digit5,
    KeyCode::Digit6,
    KeyCode::Digit7,
    KeyCode::Digit8,
    KeyCode::Digit9,
];

fn keys(
    (keys, mouse): (Res<ButtonInput<KeyCode>>, Res<ButtonInput<MouseButton>>),
    cursor: Single<&CursorOptions>,
    menu: Res<RadioMenu>,
    radio: Option<Res<RadioCommands>>,
    mut console: ResMut<Console>,
    mut captured: Option<ResMut<crate::client::binds::CapturedKeys>>,
    mut commands: Commands,
) {
    if !crate::client::input::cursor_grabbed(&cursor) {
        return;
    }
    // The radio keys (`radio1` to `radio3`: Z, X, C).
    for i in 0..3 {
        if crate::client::binds::just_pressed(&console.binds, &keys, &mouse, &format!("radio{}", i + 1)) {
            commands.queue(move |w: &mut World| open(w, Some(i)));
            return;
        }
    }
    let Some(open_menu) = menu.0 else { return };
    let Some(n) = DIGITS.iter().position(|k| keys.just_pressed(*k)) else {
        return;
    };
    // The number is the menu's: no weapon slot takes it, nor once the menu
    // has closed with the key still down.
    if let Some(c) = captured.as_deref_mut() {
        c.capture(DIGITS[n]);
    }
    if n == 0 {
        commands.queue(|w: &mut World| w.resource_mut::<RadioMenu>().0 = None);
        return;
    }
    if let Some(c) = radio.as_ref().and_then(|r| r.pick(open_menu, n)) {
        console.submit(c.command.clone());
        commands.queue(|w: &mut World| w.resource_mut::<RadioMenu>().0 = None);
    }
}

#[derive(Component)]
struct MenuText;

/// The open menu, as CS:S's HUD menu shows it: the title, the numbered
/// calls, "0. Exit".
fn menu_lines(radio: &RadioCommands, menu: usize) -> Option<(String, Vec<String>)> {
    let m = radio.menus.get(menu)?;
    let mut items: Vec<String> = if m.items.is_empty() {
        // No menu text: the calls' own words.
        radio
            .commands
            .iter()
            .filter_map(|c| Some((c.menu.filter(|x| x.0 == menu)?.1, c.variants.first()?.1.clone())))
            .map(|(n, l)| format!("{n}. \"{l}\""))
            .collect()
    } else {
        m.items.iter().map(|(n, l)| format!("{n}. \"{l}\"")).collect()
    };
    items.push(String::new());
    items.push("0. Exit".into());
    let title = if m.title.is_empty() {
        ["Radio Commands", "Group Radio Commands", "Radio Responses/Reports"][menu.min(2)].to_string()
    } else {
        m.title.clone()
    };
    Some((title, items))
}

fn draw(
    menu: Res<RadioMenu>,
    radio: Option<Res<RadioCommands>>,
    hud: Option<Res<crate::map::hud::ActiveHud>>,
    shown: Query<Entity, With<MenuText>>,
    windows: Query<&Window>,
    fonts: Res<crate::client::fonts::UiFonts>,
    mut built: Local<Option<(Option<usize>, f32)>>,
    mut commands: Commands,
) {
    let h = windows.iter().next().map_or(480.0, |w| w.height());
    let key = (menu.0.filter(|_| radio.is_some()), h);
    if *built == Some(key) {
        return;
    }
    *built = Some(key);
    for e in &shown {
        commands.entity(e).despawn();
    }
    let (Some(open), Some(radio)) = (menu.0, radio) else {
        return;
    };
    let Some((title, items)) = menu_lines(&radio, open) else {
        return;
    };
    let scale = h / 480.0;
    let named = |n: &str, d: Color| hud.as_ref().and_then(|h| h.0.color(n)).unwrap_or(d);
    let title_color = named("MenuColor", Color::srgb_u8(233, 208, 173));
    let item_color = named("ItemColor", Color::srgba_u8(255, 167, 42, 200));
    let bg = named("MenuBoxBg", Color::srgba_u8(0, 0, 0, 100));
    // HudMenu's `TextFont` and `ItemFont`: the client scheme's Default.
    let font = fonts.client("Default", h, 12.0);
    let root = commands
        .spawn((
            MenuText,
            Text::default(),
            font.clone(),
            BackgroundColor(bg),
            Node {
                position_type: PositionType::Absolute,
                left: px(8.0 * scale),
                top: Val::Percent(40.0),
                padding: UiRect::all(px(6.0 * scale)),
                border_radius: BorderRadius::all(px(4.0 * scale)),
                ..default()
            },
            GlobalZIndex(50),
        ))
        .id();
    commands.spawn((
        TextSpan::new(format!("{title}\n\n")),
        font.clone(),
        TextColor(title_color),
        ChildOf(root),
    ));
    commands.spawn((
        TextSpan::new(items.join("\n")),
        font,
        TextColor(item_color),
        ChildOf(root),
    ));
}

/// Seconds the radio icon stays over a caller's head (ours: CS:S's
/// timing isn't in the specs; docs/tech-debt.md).
pub const RADIO_ICON_SECONDS: f32 = 1.5;

/// Teammates speaking on the radio: seconds their icon has left, and its
/// node once drawn.
#[derive(Resource, Default, Debug)]
pub struct RadioIcons(pub Vec<(Entity, f32, Option<Entity>)>);

impl RadioIcons {
    /// `who` spoke: show (or keep showing) their icon.
    pub fn speak(&mut self, who: Entity) {
        match self.0.iter_mut().find(|(e, ..)| *e == who) {
            Some(i) => i.1 = RADIO_ICON_SECONDS,
            None => self.0.push((who, RADIO_ICON_SECONDS, None)),
        }
    }
}

#[derive(Component)]
struct RadioIcon;

/// The icons over speaking teammates' heads, placed on screen each frame
/// (sized as a sprite 16 units across). Hidden behind walls and bodies:
/// the game's `sprites/radio` material is drawn depth-tested
/// (`$ignorez 0`), unlike the planted bomb's marker.
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
fn draw_icons(
    time: Res<Time>,
    mut icons: ResMut<RadioIcons>,
    hud: Option<Res<crate::map::hud::ActiveHud>>,
    camera: Query<(&Camera, &GlobalTransform), With<crate::client::FirstPersonCamera>>,
    who: Query<(&GlobalTransform, Option<&crate::core::MovementState>, Option<&Health>)>,
    mut nodes: Query<(&mut Node, &mut Visibility), With<RadioIcon>>,
    (spatial, local, spec): (
        avian3d::prelude::SpatialQuery,
        Query<Entity, With<crate::core::LocalPlayer>>,
        Option<Res<crate::client::spectate::SpecView>>,
    ),
    mut commands: Commands,
) {
    let cam = camera.iter().next();
    // Whose eyes the camera is in (not in the way of its own view).
    let viewers: Vec<Entity> = local.iter().chain(spec.and_then(|s| s.in_eye)).collect();
    let dt = time.delta_secs();
    icons.0.retain_mut(|(e, left, node)| {
        *left -= dt;
        let speaker = who.get(*e).ok().filter(|(.., h)| h.is_none_or(|h| h.current > 0.0));
        let (Some((gt, state, _)), true) = (speaker, *left > 0.0) else {
            if let Some(n) = node {
                commands.entity(*n).despawn();
            }
            return false;
        };
        let n = *node.get_or_insert_with(|| {
            let sprite = hud
                .as_ref()
                .and_then(|h| Some((h.0.sprites.get("radio")?, h.1.clone())))
                .and_then(|(s, textures)| {
                    let [x, y, w, h] = s.rect;
                    Some(ImageNode {
                        image: textures.get(&s.texture)?.clone(),
                        rect: Some(Rect::new(x, y, x + w, y + h)),
                        ..default()
                    })
                });
            let node = Node {
                position_type: PositionType::Absolute,
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            };
            let mut c = commands.spawn((RadioIcon, node, Visibility::Hidden, GlobalZIndex(30)));
            match sprite {
                Some(image) => {
                    c.insert(image);
                }
                None => {
                    c.insert((
                        Text::new("((o))"),
                        TextFont {
                            font_size: FontSize::Px(14.0),
                            ..default()
                        },
                        TextColor(Color::srgb_u8(255, 176, 0)),
                    ));
                }
            }
            c.id()
        });
        // Above the head: the hull's top plus 10 units.
        let top = state.map_or(0.9, |s| s.hull_max.y) + 10.0 * 0.0254;
        let at = gt.translation() + Vec3::Y * top;
        if let (Some((c, ct)), Ok((mut node, mut vis))) = (cam, nodes.get_mut(n)) {
            let right = ct.right() * (8.0 * 0.0254);
            let hidden = || {
                let from = ct.translation();
                let to = at - from;
                let Ok(dir) = Dir3::new(to) else { return false };
                let skip = viewers.iter().copied().chain([*e]);
                let filter = avian3d::prelude::SpatialQueryFilter::from_excluded_entities(skip)
                    .with_mask(crate::core::SOLID_LAYERS);
                spatial.cast_ray(from, dir, to.length(), true, &filter).is_some()
            };
            match (c.world_to_viewport(ct, at), c.world_to_viewport(ct, at + right)) {
                (Ok(_), Ok(_)) if hidden() => *vis = Visibility::Hidden,
                (Ok(p), Ok(q)) => {
                    let size = ((q - p).length() * 2.0).clamp(8.0, 64.0);
                    node.left = px(p.x - size / 2.0);
                    node.top = px(p.y - size);
                    node.width = px(size);
                    node.height = px(size);
                    *vis = Visibility::Visible;
                }
                _ => *vis = Visibility::Hidden,
            }
        }
        true
    });
}

/// Play and print the radio calls the local player hears.
#[allow(clippy::type_complexity)]
fn hear(
    mut calls: MessageReader<Radio>,
    radio: Option<Res<RadioCommands>>,
    local: Query<(Entity, Option<&Team>), With<LocalPlayer>>,
    who: Query<(
        Option<&Name>,
        Option<&Team>,
        Option<&Health>,
        Has<LocalPlayer>,
        Option<&GlobalTransform>,
        Option<&crate::net::NetCharacter>,
    )>,
    nav: Option<Res<NavMesh>>,
    settings: Res<RadioSettings>,
    net: Option<Res<crate::net::NetSettings>>,
    (voices, mut bank): (Option<Res<PartyVoices>>, Option<ResMut<SoundBank>>),
    mut play: MessageWriter<PlaySound>,
    mut chat: MessageWriter<ChatLine>,
    mut icons: ResMut<RadioIcons>,
    mut rng: Local<u64>,
) {
    let Some(radio) = radio else {
        calls.clear();
        return;
    };
    let Some((me, my_team)) = local.iter().next() else {
        calls.clear();
        return;
    };
    for call in calls.read() {
        let Some(c) = radio.get(&call.command) else { continue };
        let Ok((name, team, health, is_local, at, character)) = who.get(call.sender) else {
            continue;
        };
        let alive = health.is_none_or(|h| h.current > 0.0);
        let stock = hears(me, my_team.map(|t| t.0), call.sender, team.map(|t| t.0), alive);
        // Another's call the game gives only to their team: heard in 3D
        // at them under `party_voice_lines_3d`, within a talking voice's
        // reach; no chat line or icon.
        let mine = who.get(me).ok().and_then(|w| w.4).map(|t| t.translation());
        let speaker = at.map(|t| t.translation());
        let party = !stock
            && !is_local
            && alive
            && PartyVoices::is_on(voices.as_deref())
            && mine.zip(speaker).is_some_and(|(m, s)| send_range(TALKING).is_none_or(|r| m.distance(s) <= r));
        if !(stock || party) || c.variants.is_empty() || (settings.ignore != 0 && !is_local) {
            continue;
        }
        // xorshift: pick a variant.
        *rng = rng.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut x = *rng;
        x ^= x >> 30;
        x = x.wrapping_mul(0xBF58_476D_1CE4_E5B9);
        x ^= x >> 27;
        let (sound, text) = &c.variants[(x % c.variants.len() as u64) as usize];
        if party {
            let Some(entry) = bank.as_deref_mut().and_then(|b| b.at_level(sound, TALKING, "talking")) else {
                continue;
            };
            play.write(PlaySound {
                entry,
                at: speaker,
                volume: None,
                pitch: None,
                source: Some(call.sender),
                channel: Some(RADIO_CHANNEL),
            });
            continue;
        }
        play.write(PlaySound::ui(sound.clone()));
        if !is_local {
            icons.speak(call.sender);
        }
        let sender = crate::client::shown_name(call.sender, is_local, character, name, net.as_deref());
        let place = nav.as_ref().zip(at).and_then(|(nav, at)| {
            let area = nav.area_at(at.translation())?;
            nav.places.get(nav.areas[area].place?).cloned()
        });
        chat.write(ChatLine(radio.line(&sender, text, place.as_deref()), team.copied()));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::map::radio::{RadioCommand, RadioMenu as Menu};

    #[test]
    fn speaking_again_keeps_one_icon_and_restarts_it() {
        let mut icons = RadioIcons::default();
        let a = Entity::from_raw_u32(5).unwrap();
        icons.speak(a);
        icons.0[0].1 = 0.2;
        icons.speak(a);
        assert_eq!(icons.0.len(), 1);
        assert_eq!(icons.0[0].1, RADIO_ICON_SECONDS);
    }

    #[test]
    fn menu_text_from_the_game_or_the_calls() {
        let mut r = RadioCommands {
            commands: vec![RadioCommand {
                command: "coverme".into(),
                menu: Some((0, 1)),
                variants: vec![("Radio.CoverMe".into(), "Cover Me!".into())],
            }],
            menus: vec![Menu::default()],
            ..default()
        };
        let (title, items) = menu_lines(&r, 0).unwrap();
        assert_eq!(title, "Radio Commands");
        assert_eq!(items, ["1. \"Cover Me!\"", "", "0. Exit"]);
        r.menus[0] = Menu {
            title: "Radio Commands".into(),
            items: vec![(1, "Cover Me".into())],
        };
        assert_eq!(menu_lines(&r, 0).unwrap().1[0], "1. \"Cover Me\"");
        assert!(menu_lines(&r, 1).is_none());
    }
}
