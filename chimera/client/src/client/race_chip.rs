//! The race chip and the Tab board (docs/plans/active/ugg-ui.md, M2): the
//! Signal look's top-centre plate for a ladder minigame (the gun game),
//! drawn only when the app's theme turns the Signal HUD on
//! (`signal::signal_on`, `flags.hud.signal`); it then stands in for the
//! minigame panel (`minigame_panel`) and the Tab scoreboard
//! (`minigame_scoreboard`) while a ladder plays. Other minigames and apps
//! keep those.
//!
//! - **The chip** (21 x 3.05 cq, 2.2 cq from the top): the leader (LEAD,
//!   name, rung), then YOU, your rung and your gun's icon, and the ladder's
//!   meter (one skewed tick a rung: yours lit, your current one in the
//!   accent, the leader's tick marked with a dot). **Leading**, it changes
//!   shape: a crown, YOU and your rung in the accent, the whole meter in
//!   the accent, then "+N over NAME" (the gap to second place, whose tick
//!   gets a hollow mark).
//! - **The board** (`+showscores`, Tab, held): the chip's plate opens
//!   into it (64 cq wide, as tall as its rows) in `motion.dur.board.open`
//!   (0.12 s, `expo_out`) and folds back in `close` (0.09 s,
//!   `cubic_in_out`), from wherever it is, so Tab can be spammed. Its head
//!   is the minigame's tag, the map and its game, "first to N" and the
//!   players; then a row a player, best first (`minigame_scoreboard::
//!   sorted`): position, the bot slot and name with HOST (or the fused
//!   YOU+HOST chip) and the matches won this server run (a trophy and the
//!   count, the session's leader in the accent, only when there are any: `MatchesWon`, M6's session wins
//!   hook in), the rung big, the gun's icon (`signal::gun_icon`) and its
//!   short name with its game, kills and deaths as two columns with their
//!   marks right-aligned over them, the ping (a 4-bar meter and the
//!   number, by the theme's `[network]` thresholds: `ping_level`; LAG at
//!   the worst; LOCAL for the listen host, a faint dash for a bot), and the ladder strip. The leader's row
//!   has the accent bar; yours is tinted and outlined.
//!
//! Content is rebuilt only when what it shows changes (`RaceView`), the
//! open and close move only the plate's size (`Timeline`). The nodes carry
//! the look book's names (`chimera_ui::UiParityId`: `race.plate`, `chip.*`
//! or `chip.leading.*`, `board.head.*`, `board.th.*`, `board.row.<n>.*`
//! from 1) for the UI parity check (`ui_parity`).
//!
//! Your rung on the chip rolls and your meter tick pops with the rank-up
//! toast's roll (`rank_toast`).
//!
//! Known shortcuts: the board lists
//! at most `MAX_ROWS` players; no match clock on the head; no packet loss on the board (not measured).

use bevy::prelude::*;
use chimera_sim::{
    minigame::{
        hud::MinigameHud,
        ladder::{Ladder, Rung},
        runtime::InMinigame,
        wins::MatchesWon,
    },
    weapon::data::WeaponCatalog,
};
use chimera_ui::{Prop, Role, SlantMaterial, TickBarMaterial, TickLayout, Timeline, Tone, Track, UiClock, UiParityId, parity};

use super::{
    console::HeldActions,
    minigame_scoreboard::{GunNames, SCOREBOARD_Z, gun_short_name},
    signal::{self, Look, Mark, TagKind},
};
use crate::{
    core::{LocalPlayer, Score},
    net::{HOST_ID, NetCharacter, NetScore, NetSettings, score_flags},
};

/// The chip and its board (module doc).
pub struct RaceChipPlugin;

impl Plugin for RaceChipPlugin {
    fn build(&self, app: &mut App) {
        super::overlay::layer(app);
        super::signal::setup(app);
        app.init_resource::<HeldActions>()
            .init_resource::<GunNames>()
            .init_resource::<RaceView>()
            .add_systems(Startup, spawn)
            .add_systems(Update, (gather, (build_chip, build_board), open_close).chain().in_set(RaceChipSystems));
        if !app.is_plugin_added::<super::rank_toast::RankToastPlugin>() {
            app.add_plugins(super::rank_toast::RankToastPlugin);
        }
    }
}

/// The chip's and board's systems (the rank-up toast runs after them).
#[derive(SystemSet, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct RaceChipSystems;

/// The staged race's ladder: (gun, short name, game, the kit's icon).
pub const STAGED_GUNS: [(&str, &str, &str, &str); 14] = [
    ("cs_source:weapon_glock", "GLOCK-18", "CS:S", "pistol"),
    ("combat_arms:m92fs", "M92FS", "CA", "pistol"),
    ("hl2dm:weapon_357", ".357 MAGNUM", "HL2:DM", "revolver"),
    ("cs_source:weapon_mp5navy", "MP5", "CS:S", "smg"),
    ("combat_arms:mp7", "MP7", "CA", "smg"),
    ("hl2dm:weapon_shotgun", "SHOTGUN", "HL2:DM", "shotgun"),
    ("cs_source:weapon_m4a1", "M4A1", "CS:S", "rifle"),
    ("combat_arms:ak_47", "AK-47", "CA", "rifle"),
    ("hl2dm:weapon_ar2", "PULSE RIFLE", "HL2:DM", "energy"),
    ("cs_source:weapon_m249", "M249", "CS:S", "lmg"),
    ("cs_source:weapon_awp", "AWP", "CS:S", "sniper"),
    ("hl2dm:weapon_crossbow", "CROSSBOW", "HL2:DM", "crossbow"),
    ("hl2dm:weapon_rpg", "RPG", "HL2:DM", "launcher"),
    ("cs_source:weapon_knife", "KNIFE", "CS:S", "knife"),
];

/// The most rows the board lists.
pub const MAX_ROWS: usize = 16;

/// One player as the chip and board show them.
#[derive(Clone, Debug, PartialEq)]
pub struct RaceRow {
    pub name: String,
    /// The rung, 0 the first gun.
    pub rung: usize,
    pub kills: i32,
    pub deaths: u32,
    /// The rung's gun.
    pub gun: Option<&'static str>,
    /// Its short name and its game's short name.
    pub gun_name: String,
    pub game: String,
    /// The fallback icon's name (`signal::icon_class`).
    pub class: Option<&'static str>,
    pub bot: bool,
    pub host: bool,
    pub local: bool,
    /// Matches won this server run.
    pub wins: u32,
    /// Ping in ms (0: the host's, or not measured yet).
    pub ping: u16,
}

/// A row's ping meter: how many of its 4 bars are lit.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub struct PingMeter(pub u8);

/// A ping's look on the board: the kit's `PingLevel` by the theme's
/// `[network]` thresholds (4 bars quiet green under 60 ms, 3 `fg` under
/// 120, 2 `warn` under 200, else 1 `threat` and a LAG flag). No packet
/// loss is measured yet (the kit's `loss` mark waits for it).
pub fn ping_level(theme: &chimera_ui::Theme, ping: u16) -> chimera_ui::widgets::PingLevel {
    chimera_ui::widgets::PingLevel::from_ms(theme, ping as u32)
}

/// A row's matches won this server run (trophy and count; only when any).
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub struct SessionWins(pub u32);

/// What the chip and board show now (`gather`); `active` while a ladder
/// plays under the Signal look.
#[derive(Resource, Clone, Debug, Default, PartialEq)]
pub struct RaceView {
    pub active: bool,
    /// Best first.
    pub rows: Vec<RaceRow>,
    /// Rungs on the ladder.
    pub rungs: usize,
    /// The map's name and its game's short name.
    pub map: String,
    pub map_game: String,
    /// Window height and scale factor the content was laid out for.
    pub size: (u32, u32),
}

impl RaceView {
    pub fn you(&self) -> Option<&RaceRow> {
        self.rows.iter().find(|r| r.local)
    }

    pub fn leader(&self) -> Option<&RaceRow> {
        self.rows.first()
    }

    /// You lead (first, best first): the chip's leader variant.
    pub fn leading(&self) -> bool {
        self.rows.first().is_some_and(|r| r.local)
    }
}


/// A staged race (`ui_scene chip|lead|board`, made-up players): shown
/// instead of the game's, the board open with `open`.
#[derive(Resource, Clone, Debug, Default)]
pub struct RaceStaged {
    pub view: RaceView,
    pub open: bool,
}

/// The made-up race the scenes stage: eight players, you (and the host)
/// seventh-rung in fourth, or leading (`lead`).
pub fn staged_race(lead: bool) -> RaceView {
    let gun = |rung: usize| -> (Option<&'static str>, &'static str, &'static str, Option<&'static str>) {
        let (id, name, game, class) = STAGED_GUNS[rung.min(13)];
        (Some(id), name, game, Some(class))
    };
    let row = |name: &str, rung: usize, kills: i32, deaths: u32, bot: bool, local: bool, host: bool, wins: u32, ping: u16| {
        let (id, gun_name, game, class) = gun(rung);
        RaceRow {
            name: name.into(),
            rung,
            kills,
            deaths,
            gun: id,
            gun_name: gun_name.into(),
            game: game.into(),
            class,
            bot,
            host,
            local,
            wins,
            ping,
        }
    };
    let rows = if lead {
        vec![
            row("kestrel", 10, 25, 8, false, true, true, 2, 0),
            row("lowgrav_larry", 8, 19, 11, false, false, false, 1, 48),
            row("nadia.exe", 7, 18, 12, false, false, false, 3, 87),
            row("Sgt. Biscuit", 7, 17, 14, true, false, false, 0, 0),
            row("vex", 6, 14, 10, false, false, false, 1, 143),
            row("mothman", 4, 10, 15, false, false, false, 0, 61),
            row("Rook", 3, 8, 16, true, false, false, 0, 0),
            row("pixelpirate", 1, 3, 19, false, false, false, 0, 231),
        ]
    } else {
        vec![
            row("nadia.exe", 10, 24, 9, false, false, false, 3, 87),
            row("lowgrav_larry", 8, 19, 11, false, false, false, 1, 48),
            row("Sgt. Biscuit", 7, 17, 14, true, false, false, 0, 0),
            row("kestrel", 6, 15, 12, false, true, true, 2, 0),
            row("vex", 6, 14, 10, false, false, false, 1, 143),
            row("mothman", 4, 10, 15, false, false, false, 0, 61),
            row("Rook", 3, 8, 16, true, false, false, 0, 0),
            row("pixelpirate", 1, 3, 19, false, false, false, 0, 231),
        ]
    };
    RaceView { active: true, rows, rungs: 14, map: "de_dust2".into(), map_game: "CS:S".into(), size: (0, 0) }
}

/// Stages scene `name` (`chip`, `lead`, `board`, `board_lead`), or clears
/// it (`None`).
pub fn stage(world: &mut World, name: Option<&str>) {
    match name {
        Some(n) => {
            let lead = n.contains("lead");
            world.insert_resource(RaceStaged { view: staged_race(lead), open: n.starts_with("board") });
        }
        None => {
            world.remove_resource::<RaceStaged>();
        }
    }
}

/// The plate's root (full width, top).
#[derive(Component)]
pub struct RaceChipRoot;

/// The plate that opens from the chip to the board.
#[derive(Component)]
pub struct RacePlate;

/// The chip's content.
#[derive(Component)]
pub struct ChipContent;

/// The board's content.
#[derive(Component)]
pub struct BoardContent;

/// A board row: its player.
#[derive(Component, Clone, Debug, PartialEq)]
pub struct BoardRow(pub RaceRow);

/// The chip's leader-variant marker (crown and gap).
#[derive(Component)]
pub struct LeaderChip;

/// The board is open (Tab held): the plate opens toward it.
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct BoardOpen(pub bool);

const CHIP_W: f32 = 21.0;
/// The leader variant is 2 cq wider (the look book's plate while you lead).
const CHIP_W_LEAD: f32 = 23.0;

/// The closed plate's width for `view`.
fn chip_w(view: &RaceView) -> f32 {
    if view.leading() { CHIP_W_LEAD } else { CHIP_W }
}
const CHIP_H: f32 = 3.05;
const BOARD_W: f32 = 64.0;
const ROW_H: f32 = 2.55;
/// The board's height besides its rows: padding, head, column titles.
const BOARD_BASE: f32 = 5.5;
/// The board's columns: position, player, rung, icon, gun (rest), K, D,
/// ping, ladder; the gap between them; the row's side padding.
const COLS: [f32; 9] = [1.6, 11.4, 2.6, 3.8, 0.0, 1.7, 1.7, 3.6, 11.4];
const COL_GAP: f32 = 0.8;
/// The look book's tracking in cq (its `letter-spacing` em times the size
/// it was set at): a row's numbers and game (.08 em of .54), the column
/// titles (.12 em of .46), the head's race (.12 em of .52) and map's game
/// (.1 em of .56).
const TRACK_ROW: f32 = 0.54 * 0.08;
const TRACK_TITLES: f32 = 0.46 * 0.12;
const TRACK_HEAD: f32 = 0.52 * 0.12;
const TRACK_MAP_GAME: f32 = 0.56 * 0.1;
/// A ping's number (or a bot's dash) is at least 2.2 characters wide.
const PING_VALUE_W: f32 = 0.5 * 0.6 * 2.2;
const ROW_PAD: f32 = 0.6;

/// The board's height in cq for `rows` rows.
pub fn board_h(rows: usize) -> f32 {
    BOARD_BASE + rows.min(MAX_ROWS) as f32 * ROW_H
}

fn spawn(layer: Res<super::overlay::OverlayLayer>, mut commands: Commands) {
    commands.init_resource::<BoardOpen>();
    commands
        .spawn((
            RaceChipRoot,
            ChildOf(layer.0),
            Visibility::Hidden,
            GlobalZIndex(SCOREBOARD_Z),
            Pickable::IGNORE,
            Node {
                position_type: PositionType::Absolute,
                left: px(0.0),
                top: px(0.0),
                width: percent(100.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::FlexStart,
                ..default()
            },
        ))
        .with_children(|r| {
            r.spawn((
                RacePlate,
                UiParityId::new("race.plate"),
                Pickable::IGNORE,
                Node {
                    overflow: Overflow::clip(),
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::FlexStart,
                    flex_shrink: 0.0,
                    ..default()
                },
                UiTransform::default(),
                Timeline::new([Track::new(Prop::Width, CHIP_W, CHIP_W), Track::new(Prop::Height, CHIP_H, CHIP_H)]),
            ));
        });
}

#[allow(clippy::type_complexity)]
fn gather(
    look: Look,
    hud: Option<Res<MinigameHud>>,
    ladder: Option<Res<Ladder>>,
    settings: Option<Res<NetSettings>>,
    (gun_names, catalog, map_name, games): (
        Res<GunNames>,
        Option<Res<WeaponCatalog>>,
        Option<Res<crate::map::LoadedMapName>>,
        Option<Res<chimera_core::game_registry::Games>>,
    ),
    players: Query<
        (
            Entity,
            Option<&Rung>,
            Option<&Score>,
            Option<&MatchesWon>,
            Option<&NetScore>,
            Option<&NetCharacter>,
            Option<&Name>,
            Has<LocalPlayer>,
            Has<crate::core::Bot>,
        ),
        With<InMinigame>,
    >,
    mut view: ResMut<RaceView>,
    staged: Option<Res<RaceStaged>>,
) {
    if let Some(st) = staged {
        let mut next = st.view.clone();
        next.active = true;
        next.size = (look.units.height as u32, (look.units.scale_factor * 100.0) as u32);
        if *view != next {
            *view = next;
        }
        return;
    }
    let active = look.on() && ladder.is_some() && hud.as_deref().is_some_and(|h| h.playing() && !h.results_shown);
    let mut next = RaceView { active, size: (look.units.height as u32, (look.units.scale_factor * 100.0) as u32), ..default() };
    if let (true, Some(ladder)) = (active, ladder.as_deref()) {
        let game_of = |id: &str| -> String {
            let g = id.split_once(':').map_or("", |(g, _)| g);
            games.as_deref().and_then(|r| r.get(g)).map_or_else(|| g.to_uppercase(), |i| i.short_name.to_string())
        };
        let mut rows: Vec<RaceRow> = players
            .iter()
            .map(|(e, rung, score, won, net, ch, name, local, is_bot)| {
                let rung = rung.map_or(0, |r| r.0);
                let s = score.copied().unwrap_or_default();
                let gun = ladder.shown_weapon(rung, catalog.as_deref());
                RaceRow {
                    name: super::shown_name(e, local, ch, name, settings.as_deref()),
                    rung,
                    kills: s.kills,
                    deaths: s.deaths,
                    gun,
                    gun_name: gun.map(|id| gun_short_name(id, Some(&gun_names), catalog.as_deref())).unwrap_or_default(),
                    game: gun.map(game_of).unwrap_or_default(),
                    class: gun.and_then(|id| catalog.as_deref()?.get(id)).and_then(signal::icon_class),
                    bot: is_bot || net.is_some_and(|n| n.flags & score_flags::BOT != 0),
                    host: ch.is_some_and(|c| c.owner == Some(HOST_ID)),
                    local,
                    wins: won.map_or(0, |w| w.0),
                    ping: net.map_or(0, |n| n.ping),
                }
            })
            .collect();
        rows.sort_by(|a, b| {
            b.rung
                .cmp(&a.rung)
                .then(b.kills.cmp(&a.kills))
                .then(a.deaths.cmp(&b.deaths))
                .then(a.name.cmp(&b.name))
        });
        next.rows = rows;
        next.rungs = ladder.rungs.len();
        if let Some(m) = map_name.as_deref() {
            let (g, n) = m.0.split_once(':').unwrap_or(("", m.0.as_str()));
            next.map = n.to_string();
            next.map_game = games.as_deref().and_then(|r| r.get(g)).map_or(String::new(), |i| i.short_name.to_string());
        }
    }
    if *view != next {
        *view = next;
    }
}

/// The chip's content, rebuilt when the view changes.
fn build_chip(
    look: Look,
    view: Res<RaceView>,
    mut ticks: Option<ResMut<Assets<TickBarMaterial>>>,
    mut slants: Option<ResMut<Assets<SlantMaterial>>>,
    plate: Single<Entity, With<RacePlate>>,
    old: Query<Entity, With<ChipContent>>,
    mut commands: Commands,
) {
    if !view.is_changed() && !old.is_empty() {
        return;
    }
    for e in &old {
        commands.entity(e).despawn();
    }
    if !view.active {
        commands.spawn((ChipContent, ChildOf(*plate), Node { position_type: PositionType::Absolute, ..default() }));
        return;
    }
    let you = view.you();
    let leader = view.leader();
    let leading = view.leading();
    let second = view.rows.get(1);
    let gap = look.px(0.6);
    // The look book's names: `chip`, or `chip.leading` for the leader variant.
    let cid = if leading { "chip.leading" } else { "chip" };
    let mut chip = commands.spawn((
        ChipContent,
        UiParityId::new(cid),
        ChildOf(*plate),
        Pickable::IGNORE,
        Node {
            position_type: PositionType::Absolute,
            left: percent(50.0),
            margin: UiRect::left(px(-look.px(chip_w(&view)) / 2.0)),
            width: look.val(chip_w(&view)),
            flex_shrink: 0.0,
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::Center,
            padding: UiRect::top(look.val(0.56)),
            row_gap: look.val(0.42),
            ..default()
        },
    ));
    if leading {
        chip.insert(LeaderChip);
    }
    chip.with_children(|c| {
        // The first line: the leader (or you, leading) and you (or the gap).
        c.spawn((Node { align_items: AlignItems::Center, column_gap: px(gap), ..default() }, UiParityId(format!("{cid}.line")), Pickable::IGNORE))
            .with_children(|r1| {
                // A rung: at least 1.5 cq wide (the look book's `.v`), so
                // the line keeps its width as the digits change.
                // Your rung rolls with the rank-up toast (`rank_toast`).
                let num = |r1: &mut ChildSpawnerCommands, n: usize, size: f32, tone: Tone, id: &str| {
                    r1.spawn((Node { min_width: look.val(1.5), flex_shrink: 0.0, ..default() }, Pickable::IGNORE)).with_children(|v| {
                        if id == "you.rung" {
                            let e = super::rank_toast::roll(v, &look, n, size, tone, true);
                            v.commands().entity(e).insert(UiParityId(format!("{cid}.{id}")));
                            return;
                        }
                        v.spawn((
                            look.tracked(Role::DisplayBold, size, size * 0.02, look.tone(tone), &format!("{:02}", n + 1)),
                            UiParityId(format!("{cid}.{id}")),
                            Pickable::IGNORE,
                        ));
                    });
                };
                // A group of the line's pieces, `gap` cq apart.
                let group = |gap: f32| (Node { align_items: AlignItems::Center, column_gap: look.val(gap), ..default() }, Pickable::IGNORE);
                let sep = |r1: &mut ChildSpawnerCommands| {
                    r1.spawn((
                        Node { width: px(look.hair()), height: look.val(1.2), margin: UiRect::horizontal(look.val(0.25)), ..default() },
                        BackgroundColor(look.tone(Tone::Fg4)),
                        UiParityId(format!("{cid}.sep")),
                        Pickable::IGNORE,
                    ));
                };
                let gun = |r1: &mut ChildSpawnerCommands, row: &RaceRow| {
                    if let Some(id) = row.gun {
                        r1.spawn((Node { height: look.val(0.9), width: look.val(2.8), align_items: AlignItems::Center, ..default() }, Pickable::IGNORE))
                            .with_children(|g| {
                                let e = signal::gun_icon(g, &look, id, row.class, look.px(0.9), look.tone(Tone::Fg));
                                parity::name(g, e, format!("{cid}.you.icon"));
                            });
                    }
                };
                match (leading, you, leader) {
                    (true, Some(me), _) => {
                        // You (crown, tag, rung, gun) .5 cq apart; the gap
                        // (+N over NAME) .4 cq apart.
                        r1.spawn(group(0.5)).with_children(|g| {
                            let e = signal::mark(g, &look, Mark::Crown, look.px(0.66), look.tone(Tone::AccText));
                            parity::name(g, e, format!("{cid}.crown"));
                            let e = signal::tag(g, &look, TagKind::Accent, "You", 0.62);
                            parity::name(g, e, format!("{cid}.you.tag"));
                            num(g, me.rung, 1.02, Tone::AccText, "you.rung");
                            gun(g, me);
                        });
                        if let Some(s) = second {
                            sep(r1);
                            r1.spawn(group(0.4)).with_children(|g| {
                                g.spawn((
                                    look.tracked(Role::DisplayBold, 0.86, 0.86 * 0.02, look.tone(Tone::Fg), &format!("+{}", me.rung.saturating_sub(s.rung))),
                                    UiParityId(format!("{cid}.gap.n")),
                                    Pickable::IGNORE,
                                ));
                                g.spawn((look.tracked(Role::Mono, 0.5, 0.56 * 0.1, look.tone(Tone::Fg3), "OVER"), UiParityId(format!("{cid}.gap.over")), Pickable::IGNORE));
                                g.spawn((
                                    look.tracked(Role::DisplayBold, 0.62, 0.62 * 0.03, look.tone(Tone::Fg2), &s.name.to_uppercase()),
                                    UiParityId(format!("{cid}.gap.name")),
                                    Pickable::IGNORE,
                                ));
                            });
                        }
                    }
                    (_, me, Some(l)) => {
                        // The leader (tag, name, rung) .5 cq apart.
                        r1.spawn(group(0.5)).with_children(|g| {
                            let e = signal::tag(g, &look, TagKind::Accent, "Lead", 0.62);
                            parity::name(g, e, format!("{cid}.lead.tag"));
                            g.spawn((
                                look.tracked(Role::DisplayBold, 0.72, 0.72 * 0.03, look.tone(Tone::Fg), &l.name.to_uppercase()),
                                UiParityId(format!("{cid}.lead.name")),
                                Pickable::IGNORE,
                            ));
                            num(g, l.rung, 1.02, Tone::AccText, "lead.rung");
                        });
                        if let Some(me) = me {
                            sep(r1);
                            let e = signal::tag(r1, &look, TagKind::You, "You", 0.62);
                            parity::name(r1, e, format!("{cid}.you.tag"));
                            num(r1, me.rung, 1.02, Tone::Fg, "you.rung");
                            gun(r1, me);
                        }
                    }
                    _ => {}
                }
            });
        // The meter: a skewed tick a rung.
        let n = view.rungs.max(1) as u32;
        let sf = look.units.scale_factor.max(1e-3);
        let pitch_cq = (17.0 / n as f32).min(0.84);
        let pitch = (look.units.phys(look.units.cq(pitch_cq))).round().max(3.0) as u32;
        let tick = ((pitch as f32) * 0.66 / 0.84).round().max(2.0) as u32;
        let h = look.units.phys(look.units.cq(0.32)).round().max(2.0) as u32;
        let lay = TickLayout { pitch, tick, gap: pitch - tick, w: n * pitch - (pitch - tick), h };
        let skew_extra = (h as f32 * look.theme.tan()).ceil() as u32;
        let me = you.map_or(0, |r| r.rung as u32 + 1);
        let mut mat = TickBarMaterial::new(&look.theme, n, lay).skewed(&look.theme).unlit(look.tone(Tone::Fg4));
        if leading {
            mat.set(me, 0);
        } else {
            mat.set(me, me.saturating_sub(1));
        }
        let meter_w = (lay.w + skew_extra) as f32 / sf;
        c.spawn((Node { width: px(meter_w), height: px(h as f32 / sf), ..default() }, UiParityId(format!("{cid}.meter")), Pickable::IGNORE)).with_children(|m| {
            if let Some(ticks) = ticks.as_deref_mut() {
                m.spawn((
                    Node { width: percent(100.0), height: percent(100.0), ..default() },
                    MaterialNode(ticks.add(mat)),
                    Pickable::IGNORE,
                ));
            }
            // Your tick, popping as your rung rolls (`rank_toast`).
            if me > 0 {
                let x = ((me - 1) * pitch) as f32 / sf;
                super::rank_toast::tick_pop(m, &look, slants.as_deref_mut(), x, (tick + skew_extra) as f32 / sf, h as f32 / sf);
            }
            // The leader's tick (chasing: a dot) or second place's
            // (leading: hollow).
            let marked = if leading { second } else { leader.filter(|l| !l.local) };
            if let Some(r) = marked {
                let d = look.px(0.24);
                let x = (r.rung as u32 * pitch) as f32 / sf + (tick as f32 / sf - d) / 2.0 + skew_extra as f32 / sf;
                let mut dot = m.spawn((
                    Node {
                        position_type: PositionType::Absolute,
                        left: px(x.round()),
                        bottom: px(h as f32 / sf + look.px(0.14)),
                        width: px(d),
                        height: px(d),
                        ..default()
                    },
                    Pickable::IGNORE,
                ));
                if leading {
                    dot.insert((
                        Node {
                            position_type: PositionType::Absolute,
                            left: px(x.round()),
                            bottom: px(h as f32 / sf + look.px(0.14)),
                            width: px(d),
                            height: px(d),
                            border: UiRect::all(px(look.hair())),
                            ..default()
                        },
                        BorderColor::all(look.tone(Tone::Fg2)),
                    ));
                } else {
                    dot.insert(BackgroundColor(look.tone(Tone::AccText)));
                }
            }
        });
    });
}

/// The board's content, rebuilt when the view changes.
fn build_board(
    look: Look,
    view: Res<RaceView>,
    mut ticks: Option<ResMut<Assets<TickBarMaterial>>>,
    mut slants: Option<ResMut<Assets<SlantMaterial>>>,
    plate: Single<Entity, With<RacePlate>>,
    old: Query<Entity, With<BoardContent>>,
    mut commands: Commands,
) {
    if !view.is_changed() && !old.is_empty() {
        return;
    }
    for e in &old {
        commands.entity(e).despawn();
    }
    let mut board = commands.spawn((
        BoardContent,
        UiParityId::new("board"),
        ChildOf(*plate),
        Visibility::Hidden,
        Pickable::IGNORE,
        Node {
            position_type: PositionType::Absolute,
            left: percent(50.0),
            margin: UiRect::left(px(-look.px(BOARD_W) / 2.0)),
            width: look.val(BOARD_W),
            flex_shrink: 0.0,
            flex_direction: FlexDirection::Column,
            padding: UiRect::new(look.val(1.5), look.val(1.5), look.val(1.2), px(0.0)),
            ..default()
        },
    ));
    if !view.active {
        return;
    }
    let col = |i: usize| look.val(COLS[i]);
    let hair = look.hair();
    board.with_children(|b| {
        // The head: the minigame's tag, the map and its game, the race.
        b.spawn((
            // On one baseline, as the look book's: the tag's bottom is the
            // head's, the map's .09 cq above it, the small texts' .2 cq
            // (Bevy's text has no baseline to align by).
            Node { align_items: AlignItems::FlexEnd, column_gap: look.val(1.6), margin: UiRect::bottom(look.val(1.0)), ..default() },
            UiParityId::new("board.head"),
            Pickable::IGNORE,
        ))
        .with_children(|t| {
            let e = signal::tag(t, &look, TagKind::Accent, "Gun game", 0.62);
            parity::name(t, e, "board.head.tag");
            t.spawn((
                Node { margin: UiRect::bottom(look.val(0.09)), ..default() },
                look.tracked(Role::DisplayBold, 1.3, 0.0, look.tone(Tone::Fg), &view.map.to_uppercase()),
                UiParityId::new("board.head.map"),
                Pickable::IGNORE,
            ));
            if !view.map_game.is_empty() {
                t.spawn((
                    Node { margin: UiRect::new(look.val(-0.9), px(0.0), px(0.0), look.val(0.2)), ..default() },
                    look.tracked(Role::Mono, 0.56, TRACK_MAP_GAME, look.tone(Tone::Fg2), &format!("• {}", view.map_game)),
                    UiParityId::new("board.head.game"),
                    Pickable::IGNORE,
                ));
            }
            t.spawn((Node { flex_grow: 1.0, ..default() }, Pickable::IGNORE));
            t.spawn((
                Node { margin: UiRect::bottom(look.val(0.24)), ..default() },
                look.tracked(
                    Role::Mono,
                    0.52,
                    TRACK_HEAD,
                    look.tone(Tone::Fg3),
                    &format!("FIRST TO {} • {} PLAYERS", view.rungs, view.rows.len()),
                ),
                UiParityId::new("board.head.info"),
                Pickable::IGNORE,
            ));
        });
        let row_node = |h: Option<f32>| Node {
            align_items: AlignItems::Center,
            column_gap: look.val(COL_GAP),
            padding: UiRect::horizontal(look.val(ROW_PAD)),
            height: h.map_or(Val::Auto, |h| look.val(h)),
            flex_shrink: 0.0,
            ..default()
        };
        let cell = |w: usize| Node {
            width: if COLS[w] > 0.0 { col(w) } else { Val::Auto },
            flex_grow: if COLS[w] > 0.0 { 0.0 } else { 1.0 },
            flex_shrink: 0.0,
            min_width: px(0.0),
            overflow: Overflow::clip(),
            align_items: AlignItems::Center,
            ..default()
        };
        let right = |w: usize| Node { justify_content: JustifyContent::FlexEnd, ..cell(w) };
        // The column titles; the kill and death marks right-aligned over
        // their numbers.
        // The titles' rule is their bottom border, .5 cq under them.
        b.spawn((
            Node {
                padding: UiRect::new(look.val(ROW_PAD), look.val(ROW_PAD), px(0.0), look.val(0.5)),
                border: UiRect::bottom(px(hair)),
                ..row_node(None)
            },
            BorderColor::all(look.tone(Tone::Fg4)),
            UiParityId::new("board.th"),
            Pickable::IGNORE,
        ))
        .with_children(|h| {
            let th = |h: &mut ChildSpawnerCommands, i: usize, text: &str, r: bool, id: &str| {
                h.spawn((if r { right(i) } else { cell(i) }, Pickable::IGNORE)).with_children(|c| {
                    let mut t = c.spawn((look.tracked(Role::Mono, 0.46, TRACK_TITLES, look.tone(Tone::Fg3), text), Pickable::IGNORE));
                    if !id.is_empty() {
                        t.insert(UiParityId(format!("board.th.{id}")));
                    }
                });
            };
            th(h, 0, "#", false, "pos");
            th(h, 1, "PLAYER", false, "player");
            th(h, 2, "RUNG", true, "rung");
            th(h, 3, "", false, "");
            th(h, 4, "GUN • GAME", false, "gun");
            for (i, m, id) in [(5, Mark::Kill, "board.th.k"), (6, Mark::Death, "board.th.d")] {
                h.spawn((right(i), Pickable::IGNORE)).with_children(|c| {
                    let e = signal::mark(c, &look, m, look.px(0.6), look.tone(Tone::Fg3));
                    parity::name(c, e, id);
                });
            }
            th(h, 7, "PING", true, "ping");
            h.spawn((Node { padding: UiRect::left(look.val(0.8)), align_self: AlignSelf::Stretch, ..cell(8) }, UiParityId::new("board.th.ladder"), Pickable::IGNORE)).with_children(|c| {
                c.spawn((look.tracked(Role::Mono, 0.46, TRACK_TITLES, look.tone(Tone::Fg3), "LADDER"), Pickable::IGNORE));
            });
        });
        let n = view.rungs.max(1) as u32;
        let top_wins = view.rows.iter().map(|r| r.wins).max().unwrap_or(0);
        for (i, row) in view.rows.iter().take(MAX_ROWS).enumerate() {
            let lead = i == 0;
            // The look book counts rows from 1.
            let p = format!("board.row.{}", i + 1);
            let mut r = b.spawn((
                BoardRow(row.clone()),
                Node { border: UiRect::bottom(px(hair)), ..row_node(Some(ROW_H)) },
                BorderColor::all(look.tone(Tone::Fg5)),
                UiParityId(p.clone()),
                Pickable::IGNORE,
            ));
            if row.local {
                r.insert(signal::you_row(&look));
            }
            r.with_children(|r| {
                // The leader's accent bar.
                if lead {
                    r.spawn((
                        Node { position_type: PositionType::Absolute, left: px(0.0), top: px(0.0), bottom: px(0.0), width: look.val(0.22), ..default() },
                        BackgroundColor(look.tone(Tone::Acc)),
                        Pickable::IGNORE,
                    ));
                }
                r.spawn((cell(0), Pickable::IGNORE)).with_children(|c| {
                    c.spawn((look.tracked(Role::Mono, 0.54, TRACK_ROW, look.tone(Tone::Fg3), &format!("{:02}", i + 1)), UiParityId(format!("{p}.pos")), Pickable::IGNORE));
                });
                r.spawn((Node { column_gap: look.val(0.45), ..cell(1) }, Pickable::IGNORE)).with_children(|c| {
                    let e = signal::bot_slot(c, &look, row.bot, 0.66, look.tone(Tone::Fg2));
                    parity::name(c, e, format!("{p}.bot"));
                    c.spawn((
                        Node { flex_shrink: 1.0, min_width: px(0.0), overflow: Overflow::clip(), ..default() },
                        look.tracked(Role::DisplayBold, 0.66, 0.66 * 0.03, look.tone(Tone::Fg), &row.name.to_uppercase()),
                        UiParityId(format!("{p}.name")),
                        Pickable::IGNORE,
                    ));
                    let e = signal::host_tag(c, &look, slants.as_deref_mut(), row.host, row.local, 0.42);
                    parity::name(c, e, format!("{p}.host"));
                    if row.wins > 0 {
                        // The session's leader in the accent.
                        let tone = if row.wins == top_wins { Tone::AccText } else { Tone::Fg2 };
                        c.spawn((Node { align_items: AlignItems::Center, column_gap: look.val(0.18), flex_shrink: 0.0, ..default() }, SessionWins(row.wins), Pickable::IGNORE))
                            .with_children(|w| {
                                signal::mark(w, &look, Mark::Trophy, look.px(0.6), look.tone(tone));
                                w.spawn((look.tracked(Role::Mono, 0.5, 0.5 * 0.04, look.tone(tone), &row.wins.to_string()), UiParityId(format!("{p}.wins")), Pickable::IGNORE));
                            });
                    }
                });
                r.spawn((right(2), Pickable::IGNORE)).with_children(|c| {
                    let tone = if lead { Tone::AccText } else { Tone::Fg };
                    c.spawn((look.tracked(Role::DisplayBold, 1.2, TRACK_ROW, look.tone(tone), &format!("{:02}", row.rung + 1)), UiParityId(format!("{p}.rung")), Pickable::IGNORE));
                });
                r.spawn((right(3), Pickable::IGNORE)).with_children(|c| {
                    if let Some(id) = row.gun {
                        let e = signal::gun_icon(c, &look, id, row.class, look.px(0.84), look.tone(Tone::Fg));
                        parity::name(c, e, format!("{p}.icon"));
                    }
                });
                r.spawn((Node { column_gap: look.val(0.35), ..cell(4) }, Pickable::IGNORE)).with_children(|c| {
                    c.spawn((look.tracked(Role::DisplayBold, 0.6, 0.6 * 0.03, look.tone(Tone::Fg), &row.gun_name.to_uppercase()), UiParityId(format!("{p}.gun")), Pickable::IGNORE));
                    c.spawn((look.tracked(Role::Mono, 0.54, TRACK_ROW, look.tone(Tone::Fg3), &row.game.to_uppercase()), UiParityId(format!("{p}.game")), Pickable::IGNORE));
                });
                r.spawn((right(5), Pickable::IGNORE)).with_children(|c| {
                    c.spawn((look.tracked(Role::Mono, 0.6, TRACK_ROW, look.tone(Tone::Fg), &row.kills.to_string()), UiParityId(format!("{p}.k")), Pickable::IGNORE));
                });
                r.spawn((right(6), Pickable::IGNORE)).with_children(|c| {
                    c.spawn((look.tracked(Role::Mono, 0.6, TRACK_ROW, look.tone(Tone::Fg3), &row.deaths.to_string()), UiParityId(format!("{p}.d")), Pickable::IGNORE));
                });
                // Ping: a 4-bar meter and the number, coloured by how bad.
                // (The look book's `.png`: .5 cq type, the meter .92 em
                // tall, .3 em apart, right-aligned in its cell.)
                r.spawn((right(7), Pickable::IGNORE)).with_children(|c| {
                c.spawn((
                    Node { align_items: AlignItems::Center, column_gap: look.val(0.15), flex_shrink: 0.0, ..default() },
                    UiParityId(format!("{p}.ping")),
                    Pickable::IGNORE,
                ))
                .with_children(|c| {
                    // A bot: the faint dash (the robot marks it already);
                    // the listen host: LOCAL.
                    if row.bot {
                        c.spawn((Node { min_width: look.val(PING_VALUE_W), ..default() }, look.text(Role::Mono, 0.5, look.tone(Tone::Fg4), "—"), Pickable::IGNORE)).insert(TextLayout::new(Justify::Right, LineBreak::NoWrap));
                        return;
                    }
                    if row.host && row.ping == 0 {
                        c.spawn((look.text(Role::Mono, 0.46, look.tone(Tone::Fg3), "LOCAL"), Pickable::IGNORE));
                        return;
                    }
                    let level = ping_level(&look.theme, row.ping);
                    if let Some(slants) = slants.as_deref_mut() {
                        let parent = c.target_entity();
                        let cmd = c.commands_mut();
                        let m = chimera_ui::widgets::spawn_ping_meter(cmd, slants, &look.theme, &look.units, 0.46, level);
                        cmd.entity(m).insert((PingMeter(level.lit as u8), Pickable::IGNORE));
                        cmd.entity(parent).add_child(m);
                    } else {
                        c.spawn((PingMeter(level.lit as u8), Node::default(), Pickable::IGNORE));
                    }
                    // A good line's number stays quiet.
                    let number = if level.lit == 4 { look.tone(Tone::Fg3) } else { level.color(&look.theme) };
                    c.spawn((Node { min_width: look.val(PING_VALUE_W), ..default() }, look.tracked(Role::Mono, 0.5, 0.5 * 0.02, number, &row.ping.to_string()), Pickable::IGNORE)).insert(TextLayout::new(Justify::Right, LineBreak::NoWrap));
                    if level.lit == 1 {
                        c.spawn((look.text(Role::Mono, 0.42, look.tone(Tone::Threat), "LAG"), Pickable::IGNORE));
                    }
                });
                });
                // The ladder strip, set apart by a hairline.
                r.spawn((
                    Node { padding: UiRect::left(look.val(0.8)), border: UiRect::left(px(hair)), align_self: AlignSelf::Stretch, ..cell(8) },
                    BorderColor::all(look.tone(Tone::Fg5)),
                    UiParityId(format!("{p}.ladder")),
                    Pickable::IGNORE,
                ))
                .with_children(|c| {
                    let sf = look.units.scale_factor.max(1e-3);
                    let room = look.units.phys(look.units.cq(COLS[8] - 1.0));
                    // The look book's .58 cq ticks .16 cq apart, narrower
                    // for a long ladder.
                    let pitch = (room / n as f32).min(look.units.phys(look.units.cq(0.74))).floor().max(3.0) as u32;
                    let tick = ((pitch as f32) * 0.58 / 0.74).round().max(2.0) as u32;
                    let h = look.units.phys(look.units.cq(0.42)).round().max(2.0) as u32;
                    let lay = TickLayout { pitch, tick, gap: pitch - tick, w: n * pitch - (pitch - tick), h };
                    let extra = (h as f32 * look.theme.tan()).ceil() as u32;
                    let mut mat = TickBarMaterial::new(&look.theme, n, lay).skewed(&look.theme).unlit(look.tone(Tone::Fg4));
                    let lit = row.rung as u32 + 1;
                    mat.set(lit, lit - 1);
                    let mut strip = c.spawn((
                        Node { width: px((lay.w + extra) as f32 / sf), height: px(h as f32 / sf), ..default() },
                        UiParityId(format!("{p}.ticks")),
                        Pickable::IGNORE,
                    ));
                    if let Some(ticks) = ticks.as_deref_mut() {
                        strip.insert(MaterialNode(ticks.add(mat)));
                    }
                });
            });
        }
    });
}

/// Tab opens the plate toward the board and folds it back, from wherever
/// it is; the chip's and the board's content show by how far it is open.
#[allow(clippy::type_complexity)]
fn open_close(
    look: Look,
    view: Res<RaceView>,
    held: Res<HeldActions>,
    clock: Res<UiClock>,
    mut open: ResMut<BoardOpen>,
    staged: Option<Res<RaceStaged>>,
    mut root: Single<(&mut Visibility, &mut Node), (With<RaceChipRoot>, Without<ChipContent>, Without<BoardContent>, Without<RacePlate>)>,
    mut plate: Single<(&mut Timeline, &mut Node, &mut BackgroundColor), With<RacePlate>>,
    mut chip: Query<&mut Visibility, (With<ChipContent>, Without<BoardContent>, Without<RaceChipRoot>)>,
    mut board: Query<&mut Visibility, (With<BoardContent>, Without<ChipContent>, Without<RaceChipRoot>)>,
) {
    let (vis, root_node) = &mut *root;
    vis.set_if_neq(if view.active { Visibility::Inherited } else { Visibility::Hidden });
    // The chip sits 2.2 cq down; the board grows from it.
    let top = look.val(look.layout("chip", "top", 2.2));
    if root_node.top != top {
        root_node.top = top;
    }
    let want = view.active && (held.showscores || staged.is_some_and(|s| s.open));
    let (tl, node, bg) = &mut *plate;
    let target_h = board_h(view.rows.len());
    let plate_bg = look.tone(Tone::Plate);
    if bg.0 != plate_bg {
        bg.0 = plate_bg;
    }
    let closed_w = chip_w(&view);
    if want != open.0
        || (want && tl.tracks.iter().any(|t| t.prop == Prop::Height && t.to != target_h))
        || (!want && tl.tracks.iter().any(|t| t.prop == Prop::Width && t.to != closed_w))
    {
        open.0 = want;
        let (w, h, dur, ease) = if want {
            (BOARD_W, target_h, look.dur("board", "open", 0.12), look.theme.ease("expo_out"))
        } else {
            (chip_w(&view), CHIP_H, look.dur("board", "close", 0.09), look.theme.ease("cubic_in_out"))
        };
        tl.redirect(clock.now, Prop::Width, w, dur, ease);
        tl.redirect(clock.now, Prop::Height, h, dur, ease);
    }
    // Before the kit's first step the node has no size yet.
    if node.width == Val::Auto {
        node.width = look.val(CHIP_W);
        node.height = look.val(CHIP_H);
    }
    let w = tl.value(Prop::Width).unwrap_or(CHIP_W);
    let p = ((w - closed_w) / (BOARD_W - closed_w)).clamp(0.0, 1.0);
    let chip_on = p < 0.3;
    let board_on = p > 0.35;
    for mut v in &mut chip {
        v.set_if_neq(if chip_on { Visibility::Inherited } else { Visibility::Hidden });
    }
    for mut v in &mut board {
        v.set_if_neq(if board_on { Visibility::Inherited } else { Visibility::Hidden });
    }
}
