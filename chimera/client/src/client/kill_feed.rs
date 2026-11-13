//! One kill feed for an app with several games' HUDs (the party apps:
//! Ultimate Gun Game, Lucker Party), kept apart from every HUD and drawn
//! by the shell in one place.
//!
//! - **The state** (`KillFeed`, `KillFeedStatePlugin`): every death as
//!   `core::Died` tells it (a host's and a single player's own, a network
//!   client's from the server's `net::Killed`), as an entry: the killer
//!   and the victim (named as the HUD names them, `shown_name`, with
//!   their team and whether either is the local player), the weapon id
//!   (the killing hit's, else what the killer held when the hit was
//!   dealt: `weapon::DamageWeapons`), how it was dealt,
//!   and the flags: headshot, suicide (no killer, or the victim itself),
//!   team kill (same non-zero `core::Team`). Entries go after `lifetime`
//!   seconds, the oldest first past `max`, and all of them on a map
//!   change (`core::MapLoads`). No game's HUD keeps it, so an entry made
//!   just before the local player's HUD changes (the gun game's rung swap:
//!   `chimera_sim::identity`) is still there after.
//! - **The widget** (`KillFeedPlugin`, part of `ClientPlugins`): drawn
//!   only while the app asks for it (`SharedKillFeed`), at the screen's
//!   top right, one row per entry, newest at the bottom: killer, the gun's
//!   own game's icon (`weapon_icons::WeaponIcons`, else its display name
//!   in plain grey, `minigame_scoreboard::gun_name`: `fallback_word`), a headshot mark (the weapon's
//!   game's, `WeaponIcons::mark`, else "HS"), victim; names in team
//!   colours (a teammate's death green), the local player's in gold and
//!   its rows outlined; a row
//!   fades over its last second. Sized by the window's height. Every row
//!   is one fixed height; an icon sits in a line-tall box that centres it
//!   (an image's art, at the size its whole texture would have 90 % of the
//!   line tall: Combat Arms' are painted in the texture's top rows,
//!   `weapon_icons::ink_rows`; a glyph cut to fit, `icon_slot`), so rows are
//!   equal with any icon or none.
//! - **`SharedKillFeed`**: the app's flag that this feed is the one; the
//!   games' own feeds (CS:S's death notices, Combat Arms' kill feed,
//!   HL2:DM's death notices) and the shell's fallback HUD's leave theirs
//!   out while it is in. Plain CS:S, Combat Arms and HL2:DM never insert
//!   it, so theirs are unchanged.
//! - **A game's own feed** goes through one gate, `OwnKillFeed` (shown or
//!   not; the shell's entries only while shown), and registers how many
//!   lines it shows (`GameKillFeedAppExt::register_game_kill_feed`,
//!   `GameKillFeeds`): the party apps' tests require every composed game
//!   with a HUD to have registered, and count every feed after a kill, so
//!   a game whose feed forgets the flag shows up as a second feed there.
//!
//! Known shortcuts: no penetration (wallbang) flag: the killing hit
//! doesn't say whether it went through something; names are taken at the
//! kill (a later rename doesn't change a row); the rows sit where a
//! game's corner radar would (the party apps hide the radars:
//! `hud::Radars`).

use bevy::{prelude::*, window::PrimaryWindow};

use super::{
    fonts::UiFonts,
    minigame_scoreboard::{GunNames, gun_name},
    weapon_icons::{WeaponIcon, WeaponIcons, glyph_font, ink_rows, spawn_weapon_icon},
};
use crate::{
    core::{DamageKind, Died, Hitgroup, LocalPlayer, MapLoads, Team},
    net::{NetCharacter, NetSettings},
    weapon::{DamageWeapons, Inventory, Weapon},
};

/// The feed's state, without its drawing (headless: tests and a server).
pub struct KillFeedStatePlugin;

impl Plugin for KillFeedStatePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<KillFeed>().add_message::<Died>().add_systems(Update, record);
    }
}

/// The state and the widget (`SharedKillFeed` turns the widget on).
pub struct KillFeedPlugin;

impl Plugin for KillFeedPlugin {
    fn build(&self, app: &mut App) {
        if !app.is_plugin_added::<KillFeedStatePlugin>() {
            app.add_plugins(KillFeedStatePlugin);
        }
        super::overlay::layer(app);
        app.add_systems(Startup, spawn_root).add_systems(Update, draw.after(record));
        super::text_warm::TextWarmAppExt::warm_texts(app, warm_fonts);
    }
}

/// The feed's names' font at the window's size, and the games' icon fonts
/// (`WeaponIcons::glyph_fonts`; their kill marks') at the sizes the feed
/// draws their glyphs (`icon_slot`), laid out ahead (`text_warm`).
fn warm_fonts(
    window: super::text_warm::PrimaryWindows,
    fonts: Option<Res<UiFonts>>,
    icons: Option<Res<WeaponIcons>>,
) -> Vec<super::text_warm::WarmText> {
    let height = super::text_warm::window_height(&window).max(240.0);
    let mut out = vec![name_font(fonts.as_deref(), 15.0 * height / 720.0).into()];
    let Some(icons) = icons else { return out };
    // As `draw` computes them (the same bits: the atlas is keyed by the size).
    let (icon_scale, line) = (height / 480.0, 22.0 * (height / 720.0));
    let glyph = |font: &Handle<Font>, em: f32, chars: String, fill: f32| {
        let icon = WeaponIcon::Glyph { font: font.clone(), ch: ' ', em };
        super::text_warm::WarmText { font: glyph_font(font, em, fitted_scale(&icon, icon_scale, line * fill)), extra: chars }
    };
    out.extend(icons.glyph_fonts().map(|g| glyph(&g.font, g.em, g.chars.clone(), WEAPON_FILL)));
    out.extend(icons.marks().filter_map(|m| match m {
        WeaponIcon::Glyph { font, ch, em } => Some(glyph(font, *em, ch.to_string(), MARK_FILL)),
        WeaponIcon::Image { .. } => None,
    }));
    out
}

/// How much of a row's line a gun's icon and a kill mark fill (`icon_slot`).
const WEAPON_FILL: f32 = 0.9;
const MARK_FILL: f32 = 0.8;

/// The feed's names' font at `size` pixels.
fn name_font(fonts: Option<&UiFonts>, size: f32) -> TextFont {
    fonts.map(|f| f.em("Verdana", true, size)).unwrap_or(TextFont {
        font_size: FontSize::Px(size),
        ..default()
    })
}

/// The app draws one kill feed (this module's) for every game's HUD; the
/// games' own feeds stay out while it is in.
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SharedKillFeed;

/// The one gate a game's HUD passes before it shows its own kill feed
/// (death notices, a feed panel): `shown` is false while the app draws
/// the shared feed (`SharedKillFeed`), and `entries` (this module's state,
/// for a HUD that draws from it) is then empty. A game's own feed reads
/// this instead of checking the flag itself, and registers how many lines
/// it shows (`GameKillFeedAppExt::register_game_kill_feed`).
#[derive(bevy::ecs::system::SystemParam)]
pub struct OwnKillFeed<'w> {
    shared: Option<Res<'w, SharedKillFeed>>,
    feed: Option<Res<'w, KillFeed>>,
}

impl OwnKillFeed<'_> {
    /// Whether the game's HUD shows its own feed (no shared feed).
    pub fn shown(&self) -> bool {
        self.shared.is_none()
    }

    /// The shell's entries, oldest first, while the game's own feed shows
    /// (none while the shared feed is the one).
    pub fn entries(&self) -> impl Iterator<Item = &KillEntry> {
        self.feed.as_deref().filter(|_| self.shown()).into_iter().flat_map(|f| f.entries.iter())
    }
}

/// How many lines a game's own kill feed shows now (its HUD's state).
pub type FeedLines = fn(&World) -> usize;

/// Each game's own kill feed by its game id, as its HUD registers it
/// (`GameKillFeedAppExt`): what a check reads to see that only one feed
/// shows (the party apps' tests count every registered feed after a kill).
#[derive(Resource, Clone, Debug, Default)]
pub struct GameKillFeeds(pub Vec<(&'static str, FeedLines)>);

impl GameKillFeeds {
    /// The lines each registered game's feed shows now.
    pub fn lines(world: &World) -> Vec<(&'static str, usize)> {
        world.get_resource::<Self>().map_or_else(Vec::new, |f| f.0.iter().map(|(g, n)| (*g, n(world))).collect())
    }
}

/// A game's HUD declares its own kill feed (once per game).
pub trait GameKillFeedAppExt {
    fn register_game_kill_feed(&mut self, game: &'static str, lines: FeedLines) -> &mut Self;
}

impl GameKillFeedAppExt for App {
    fn register_game_kill_feed(&mut self, game: &'static str, lines: FeedLines) -> &mut Self {
        let mut feeds = self.world_mut().get_resource_or_init::<GameKillFeeds>();
        if !feeds.0.iter().any(|(g, _)| *g == game) {
            feeds.0.push((game, lines));
        }
        self
    }
}

/// One side of a kill.
#[derive(Clone, Debug, PartialEq)]
pub struct FeedName {
    pub entity: Entity,
    pub name: String,
    pub team: Option<u8>,
    /// The local player (at the kill).
    pub local: bool,
}

/// One kill.
#[derive(Clone, Debug, PartialEq)]
pub struct KillEntry {
    /// Counts up from 0 over the app's life.
    pub id: u64,
    /// None: the world (a fall, a map's hurt) or the victim itself.
    pub killer: Option<FeedName>,
    pub victim: FeedName,
    /// The weapon id (`game:name`), if known.
    pub weapon: Option<&'static str>,
    pub kind: DamageKind,
    pub headshot: bool,
    pub suicide: bool,
    pub team_kill: bool,
    /// When (`Time` seconds).
    pub at: f32,
}

impl KillEntry {
    /// Whether the local player is in it.
    pub fn involves_local(&self) -> bool {
        self.victim.local || self.killer.as_ref().is_some_and(|k| k.local)
    }
}

/// The kill feed: its entries, oldest first.
#[derive(Resource, Clone, Debug)]
pub struct KillFeed {
    pub entries: Vec<KillEntry>,
    /// Seconds an entry stays.
    pub lifetime: f32,
    /// The most entries kept.
    pub max: usize,
    next: u64,
    map: Option<u32>,
}

impl Default for KillFeed {
    fn default() -> Self {
        Self {
            entries: Vec::new(),
            lifetime: LIFETIME,
            max: MAX_LINES,
            next: 0,
            map: None,
        }
    }
}

/// Seconds an entry stays, and the last of them it fades over.
pub const LIFETIME: f32 = 6.0;
pub const FADE: f32 = 1.0;
pub const MAX_LINES: usize = 5;

#[allow(clippy::type_complexity)]
fn record(
    mut died: MessageReader<Died>,
    who: Query<(Option<&Name>, Option<&Team>, Has<LocalPlayer>, Option<&NetCharacter>, Option<&Inventory>)>,
    weapons: Query<&Weapon>,
    held: Option<Res<DamageWeapons>>,
    settings: Option<Res<NetSettings>>,
    loads: Option<Res<MapLoads>>,
    time: Res<Time>,
    mut feed: ResMut<KillFeed>,
) {
    let now = time.elapsed_secs();
    // A new map: the last one's kills go.
    let map = loads.map(|l| l.0);
    if feed.map != map {
        feed.map = map;
        if !feed.entries.is_empty() {
            feed.entries.clear();
        }
    }
    let side = |e: Entity| match who.get(e) {
        Ok((name, team, local, net, _)) => FeedName {
            entity: e,
            name: super::shown_name(e, local, net, name, settings.as_deref()),
            team: team.map(|t| t.0),
            local,
        },
        Err(_) => FeedName {
            entity: e,
            name: e.to_string(),
            team: None,
            local: false,
        },
    };
    for d in died.read() {
        let killer = d.attacker.filter(|a| *a != d.entity);
        // A thrown grenade names itself; otherwise what the killer held
        // when its hit was dealt (`DamageWeapons`; a network client's
        // `Died` has it from the server), else holds now.
        let weapon = held.as_deref().map_or(d.damage.weapon, |h| h.of(&d.damage));
        let weapon = weapon.filter(|_| killer.is_some()).or_else(|| {
            let k = killer?;
            let w = who.get(k).ok()?.4?.active?;
            Some(weapons.get(w).ok()?.id)
        });
        let victim = side(d.entity);
        let killer = killer.map(side);
        let team_kill = killer
            .as_ref()
            .is_some_and(|k| k.team.is_some_and(|t| t != 0 && Some(t) == victim.team));
        let id = feed.next;
        feed.next += 1;
        feed.entries.push(KillEntry {
            id,
            suicide: killer.is_none(),
            killer,
            victim,
            weapon,
            kind: d.damage.kind,
            headshot: d.damage.hitgroup == Hitgroup::Head,
            team_kill,
            at: now,
        });
    }
    let (lifetime, max) = (feed.lifetime, feed.max);
    if feed.entries.iter().any(|e| now - e.at >= lifetime) {
        feed.entries.retain(|e| now - e.at < lifetime);
    }
    if feed.entries.len() > max {
        let extra = feed.entries.len() - max;
        feed.entries.drain(..extra);
    }
}

/// The widget's root (top right).
#[derive(Component)]
pub struct KillFeedRoot;

/// A drawn row: its entry's id.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub struct KillFeedRow(pub u64);

fn spawn_root(layer: Res<super::overlay::OverlayLayer>, mut commands: Commands) {
    commands.spawn((
        KillFeedRoot,
        ChildOf(layer.0),
        Visibility::Hidden,
        GlobalZIndex(45),
        Node {
            position_type: PositionType::Absolute,
            right: px(0.0),
            top: px(0.0),
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::FlexEnd,
            ..default()
        },
    ));
}

const TEXT: Color = Color::srgb(0.92, 0.92, 0.92);
const GOLD: Color = Color::srgb(1.0, 0.82, 0.25);
const RED: Color = Color::srgb(0.95, 0.45, 0.35);
const BLUE: Color = Color::srgb(0.55, 0.75, 1.0);
/// A teammate killed by its own side.
const MATE: Color = Color::srgb(0.6, 0.9, 0.5);

/// A name's colour: the local player's gold, else by team (1: Terrorists,
/// Alpha; 2: Counter-Terrorists, Bravo), else plain.
fn name_colour(n: &FeedName) -> Color {
    match (n.local, n.team) {
        (true, _) => GOLD,
        (_, Some(1)) => RED,
        (_, Some(2)) => BLUE,
        _ => TEXT,
    }
}

fn with_alpha(c: Color, a: f32) -> Color {
    let alpha = c.alpha();
    c.with_alpha(alpha * a)
}

/// How opaque entry `e` is at `now` (1, then down to 0 over `FADE`),
/// in tenths so the rows are rebuilt only as it steps.
fn opacity(e: &KillEntry, now: f32, lifetime: f32) -> u8 {
    let left = (lifetime - (now - e.at)).max(0.0);
    ((left / FADE).min(1.0) * 10.0).ceil() as u8
}

#[allow(clippy::type_complexity, clippy::too_many_arguments)]
fn draw(
    shared: Option<Res<SharedKillFeed>>,
    feed: Res<KillFeed>,
    time: Res<Time>,
    window: Query<&Window, With<PrimaryWindow>>,
    (icons, names, catalog, fonts, images): (
        Option<Res<WeaponIcons>>,
        Option<Res<GunNames>>,
        Option<Res<crate::weapon::data::WeaponCatalog>>,
        Option<Res<UiFonts>>,
        Option<Res<Assets<Image>>>,
    ),
    mut root: Single<(Entity, &mut Visibility, &mut Node), With<KillFeedRoot>>,
    mut last: Local<Option<(Vec<(u64, u8)>, u32, bool)>>,
    mut commands: Commands,
) {
    let (root_e, vis, node) = &mut *root;
    if shared.is_none() {
        vis.set_if_neq(Visibility::Hidden);
        return;
    }
    vis.set_if_neq(Visibility::Inherited);
    let now = time.elapsed_secs();
    let height = window.iter().next().map_or(720.0, |w| w.height()).max(240.0);
    let shown: Vec<(u64, u8)> = feed.entries.iter().map(|e| (e.id, opacity(e, now, feed.lifetime))).collect();
    let icons_changed = icons.as_ref().is_some_and(|i| i.is_changed());
    let key = (shown, height as u32, icons_changed);
    if last.as_ref() == Some(&key) {
        return;
    }
    *last = Some(key);
    commands.entity(*root_e).despawn_related::<Children>();
    // Sized for a 720-line screen.
    let s = height / 720.0;
    node.right = px(14.0 * s);
    node.top = px(12.0 * s);
    node.row_gap = px(3.0 * s);
    let font = |size: f32| name_font(fonts.as_deref(), size);
    let size = 15.0 * s;
    let line = 22.0 * s;
    let icon_scale = height / 480.0;
    commands.entity(*root_e).with_children(|rows| {
        for e in &feed.entries {
            let a = opacity(e, now, feed.lifetime) as f32 / 10.0;
            if a <= 0.0 {
                continue;
            }
            let local = e.involves_local();
            rows.spawn((
                KillFeedRow(e.id),
                Node {
                    flex_direction: FlexDirection::Row,
                    align_items: AlignItems::Center,
                    column_gap: px(6.0 * s),
                    padding: UiRect::axes(px(7.0 * s), px(0.0)),
                    height: px(line),
                    border: UiRect::all(px(if local { (1.5 * s).max(1.0) } else { 0.0 })),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.5 * a)),
                BorderColor::all(with_alpha(Color::srgba(1.0, 0.82, 0.25, 0.85), a)),
            ))
            .with_children(|r| {
                let text = |r: &mut ChildSpawnerCommands, t: String, c: Color| {
                    r.spawn((Text::new(t), font(size), TextColor(with_alpha(c, a))));
                };
                if let Some(k) = &e.killer {
                    text(r, k.name.clone(), name_colour(k));
                }
                let game = e.weapon.and_then(|w| w.split_once(':')).map(|(g, _)| g);
                match e.weapon.and_then(|w| icons.as_ref()?.kill_icon(w, e.kind)) {
                    Some(icon) => icon_slot(r, &icon, icon_scale, line, WEAPON_FILL, images.as_deref()),
                    None => {
                        let what = fallback_word(e, |w| gun_name(w, names.as_deref(), catalog.as_deref()));
                        text(r, what, Color::srgb(0.78, 0.78, 0.8));
                    }
                }
                if e.headshot {
                    match game.and_then(|g| icons.as_ref()?.mark(g, HEADSHOT)) {
                        Some(mark) => icon_slot(r, &mark, icon_scale, line, MARK_FILL, images.as_deref()),
                        None => text(r, "HS".into(), Color::srgb(1.0, 0.35, 0.3)),
                    }
                }
                // A teammate killed: its name in the team-kill colour.
                let c = if e.team_kill && !e.victim.local { MATE } else { name_colour(&e.victim) };
                text(r, e.victim.name.clone(), c);
            });
        }
    });
}

/// Spawns `icon` in a box as tall as the line, centring it vertically
/// whatever its art: an image's whole texture would be `fill` of the line
/// tall (width by its shape), and only the rows with art are drawn, so the
/// art is what is centred; a glyph's font is cut down when it would be taller
/// than that. The row's height is fixed, so a line is as tall with an icon,
/// without one or with a tall one (widths vary).
fn icon_slot(r: &mut ChildSpawnerCommands, icon: &WeaponIcon, scale: f32, line: f32, fill: f32, images: Option<&Assets<Image>>) {
    let scale = fitted_scale(icon, scale, line * fill);
    // An icon font's ink sits a little above its line's middle: nudged down.
    let nudge = match icon {
        WeaponIcon::Glyph { em, .. } => 0.16 * em * scale,
        WeaponIcon::Image { .. } => 0.0,
    };
    r.spawn(Node {
        height: px(line),
        padding: UiRect::top(px(nudge)),
        align_items: AlignItems::Center,
        justify_content: JustifyContent::Center,
        flex_shrink: 0.0,
        ..default()
    })
    .with_children(|b| {
        if let WeaponIcon::Image { image, size } = icon
            && let Some((top, bottom)) = images.and_then(|i| i.get(image)).and_then(ink_rows)
            && size.y > 0.0
        {
            let tall = line * fill;
            b.spawn((
                Node {
                    width: px(tall * size.x / size.y),
                    height: px(tall * (bottom - top) / size.y),
                    ..default()
                },
                ImageNode {
                    rect: Some(Rect::new(0.0, top, size.x, bottom)),
                    ..ImageNode::new(image.clone())
                },
            ));
        } else {
            spawn_weapon_icon(b, icon, scale, line * fill, None);
        }
    });
}

/// `scale`, lowered so a glyph icon's size fits `room` pixels (an image
/// is sized by the height it is given).
pub fn fitted_scale(icon: &WeaponIcon, scale: f32, room: f32) -> f32 {
    match icon {
        WeaponIcon::Glyph { em, .. } if *em * scale > room => room / em,
        _ => scale,
    }
}

/// The words in a row's icon place when the gun has no icon: the gun's
/// display name (`name`), else how the victim died; plain, no brackets.
pub fn fallback_word(e: &KillEntry, name: impl Fn(&str) -> String) -> String {
    match (e.weapon, e.kind) {
        (Some(w), _) => name(w),
        (None, DamageKind::Fall) => "fell".into(),
        (None, DamageKind::Blast | DamageKind::BlastSurface) => "blast".into(),
        (None, DamageKind::Burn) => "burned".into(),
        (None, DamageKind::Drown) => "drowned".into(),
        (None, DamageKind::Poison) => "poisoned".into(),
        (None, DamageKind::Radiation) => "irradiated".into(),
        (None, _) if e.suicide => "died".into(),
        (None, _) => "killed".into(),
    }
}

/// The headshot mark's name in `WeaponIcons::mark`.
pub const HEADSHOT: &str = "headshot";

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(weapon: Option<&'static str>, kind: DamageKind, suicide: bool) -> KillEntry {
        let side = FeedName {
            entity: Entity::PLACEHOLDER,
            name: "A".into(),
            team: None,
            local: false,
        };
        KillEntry {
            id: 0,
            killer: (!suicide).then(|| side.clone()),
            victim: side,
            weapon,
            kind,
            headshot: false,
            suicide,
            team_kill: false,
            at: 0.0,
        }
    }

    /// A gun without an icon: its clean name in the icon's place (no
    /// brackets, seen as "[Ironwood M1866 lv.4]" in a capture); a death
    /// without a gun: how.
    #[test]
    fn the_text_fallback_is_a_plain_name() {
        let name = |w: &str| if w == "combat_arms:ironwood_m1866_lv.4" { "Ironwood M1866 lv.4".to_string() } else { w.to_string() };
        let gun = entry(Some("combat_arms:ironwood_m1866_lv.4"), DamageKind::Bullet, false);
        assert_eq!(fallback_word(&gun, name), "Ironwood M1866 lv.4");
        assert_eq!(fallback_word(&entry(None, DamageKind::Fall, true), name), "fell");
        assert_eq!(fallback_word(&entry(None, DamageKind::Bullet, true), name), "died");
        assert_eq!(fallback_word(&entry(None, DamageKind::Bullet, false), name), "killed");
    }
}
