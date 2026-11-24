//! A rung's gear on the Signal HUD (docs/plans/active/rung-passives.md,
//! P2; the user's pick of the mockup, 2026-11-24: option A): what a gun
//! game rung's passives and utility items (`ladder::RungExtras::passives`,
//! `minigame::passives`) look like, the same for everyone on the rung.
//!
//! - **The chip tail** (`GearTailPlugin`): under the race chip, centred
//!   on it (`TAIL_TOP` cq down), one slanted tile a item on the kit's plate
//!   tone with the 12° slant (`chimera_ui::SlantMaterial`, a `Fg2` bar at
//!   its left): the item's icon (`gear_icon`), its name and, for an item
//!   with a key, the kit's key cap (`chimera_ui::widgets::kbd`) with the
//!   key bound now to its action (`rung_hint::bound_key`, read each frame:
//!   a rebind shows at once) and the verb ("2 LAUNCH"). Shown on arriving
//!   on a rung with gear (`ladder::OnRung`, the local player's, on every
//!   machine) once the rank-up toast under the chip is gone; after
//!   `FOLD_AFTER` s the names, keys and verbs fold away (`FOLD_TIME`,
//!   `expo_out`) and only the icons stay, until the rung changes. None
//!   while the Tab board is open, without gear or off the Signal look.
//!   `GearTailView` is the state (headless too); `ui_scene passives`
//!   stages two items (`passives folded`: after the fold).
//! - **Icons** (`gear_icon`): the item's own game's icon (`WeaponIcons`:
//!   TF2's from its kill icons, an unlock's from the item pack's
//!   `item_iconname`) while `icons.prefer_game` is on and its art is
//!   loaded, else the kit's glyph its table names (`Passive::glyph`:
//!   boots, shield, jetpack, chute, pack, clock, jar, can, food, banner;
//!   `pack` without one).
//! - **The item's card** (`item_card`): the kit's tooltip card
//!   (`chimera_ui::shell::Tip::item`): icon, name, "game • class •
//!   worn/carried/utility", the table's one line (`Passive::summary`), the
//!   key cap and verb, and the foot "Rung NN extra • everyone on this
//!   rung" with the Chimera mark. The Tab board's Gear column
//!   (`race_chip`) carries it on each icon, shown on hover once the cursor
//!   is free (the board's right click).
//! - The kill feed (`kill_feed::signal_feed`) draws a passive's kill with
//!   the item's icon in the gun's place in the accent text tone, and the
//!   spectator plate (`spectator_plate`) lists the watched player's gear.
//!
//! Known shortcuts: no focus by keys on the board's icons (hover only); the
//! rank-up toast's "+ item" line (option C's) isn't built.

use bevy::prelude::*;
use chimera_sim::{
    minigame::{
        hud::MinigameHud,
        ladder::{Ladder, OnRung},
        passives::{PassiveKind, PassiveTables},
    },
    weapon::data::WeaponCatalog,
};
use chimera_ui::{Ease, Role, SlantMaterial, Tone, UiClock, UiParityId};

use super::{
    minigame_scoreboard::{GunNames, gun_name},
    signal::{self, Look},
};
use crate::core::LocalPlayer;

/// The tail's top, cq from the window's top (the chip's 2.2 + 3.05, and
/// a gap).
pub const TAIL_TOP: f32 = 5.45;
/// A tile's height, cq.
pub const TILE_H: f32 = 1.7;
/// The icon's height in a tile, cq.
pub const TILE_ICON_H: f32 = 1.05;
/// The name's size (Display), the key and verb's (Mono), cq.
pub const NAME_SIZE: f32 = 0.86;
pub const KEY_SIZE: f32 = 0.56;
/// The names fold away this long after the tail shows (s) ...
pub const FOLD_AFTER: f64 = 5.0;
/// ... over this long (s, `expo_out`).
pub const FOLD_TIME: f32 = 0.24;
/// The name, key and verb's widest, cq (folding shrinks it to none).
pub const NAME_MAX: f32 = 26.0;

/// One item as the HUD shows it.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct GearItem {
    /// Its weapon id.
    pub id: String,
    pub name: String,
    /// The kit's fallback icon.
    pub glyph: String,
    pub kind: PassiveKind,
    /// The action whose key it shows.
    pub action: Option<String>,
    pub verb: Option<String>,
    pub summary: Option<String>,
    /// The classes its table gives it to.
    pub classes: Vec<String>,
}

/// Item `id` as the HUD shows it: its table's row (`PassiveTables`), its
/// name from the catalogue.
pub fn gear_item(id: &str, tables: Option<&PassiveTables>, names: Option<&GunNames>, catalog: Option<&WeaponCatalog>) -> GearItem {
    let row = tables.and_then(|t| t.get(id));
    GearItem {
        id: id.to_string(),
        name: gun_name(id, names, catalog),
        glyph: row.and_then(|r| r.glyph.clone()).filter(|g| signal::icons::has(g)).unwrap_or_else(|| "pack".into()),
        kind: row.map(|r| r.kind).unwrap_or_default(),
        action: row.and_then(|r| r.key.clone()),
        verb: row.and_then(|r| r.verb.clone()),
        summary: row.and_then(|r| r.summary.clone()),
        classes: row.map(|r| r.classes.iter().filter(|c| *c != "*").cloned().collect()).unwrap_or_default(),
    }
}

/// Item `id`'s icon `h` logical px tall: its game's own while preferred
/// and loaded, else the kit's `glyph` (module doc). Its node.
pub fn gear_icon(p: &mut ChildSpawnerCommands, look: &Look, id: &str, glyph: &str, h: f32, color: Color) -> Option<Entity> {
    signal::gun_icon(p, look, id, Some(glyph), h, color)
}

/// The key bound now to `action` ("2", "RMB", "SPACE"; "unbound"): a
/// `+action` bind, else a plain one (`slot2`).
pub fn action_key(console: Option<&chimera_core::console::Console>, game: Option<&str>, action: &str) -> String {
    let plus = super::rung_hint::bound_key(console, game, action);
    if plus != "unbound" {
        return plus;
    }
    let Some(console) = console else { return plus };
    let binds = console.effective_binds(game);
    super::binds::keys_for(&binds, action).first().map_or(plus, |k| super::rung_hint::key_name(super::binds::display(k)))
}

/// The kit's card for `item` on rung `rung` (0: the first), its key
/// `key` (bound now).
pub fn item_card(item: &GearItem, game: &str, rung: usize, key: Option<&str>) -> chimera_ui::shell::ItemCard {
    let mut sub = vec![game.to_string()];
    sub.extend(item.classes.iter().map(|c| title(c)));
    sub.push(item.kind.word().to_string());
    chimera_ui::shell::ItemCard {
        title: item.name.clone(),
        sub: sub.join(" \u{2022} "),
        text: item.summary.clone(),
        key: key.zip(item.verb.as_deref()).map(|(k, v)| (k.to_string(), v.to_string())),
        foot: format!("Rung {:02} extra \u{2022} everyone on this rung", rung + 1),
        icon: Some(item.glyph.clone()),
    }
}

/// `soldier` -> `Soldier`.
fn title(s: &str) -> String {
    let mut c = s.chars();
    c.next().map(|f| f.to_uppercase().chain(c).collect()).unwrap_or_default()
}

/// A tile of the tail as shown.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct TailTile {
    pub item: GearItem,
    /// The key bound now (none: an item without one).
    pub key: Option<String>,
}

/// What the tail shows now (module doc).
#[derive(Resource, Clone, Debug, Default, PartialEq)]
pub struct GearTailView {
    pub tiles: Vec<TailTile>,
    /// Shown now (the Signal look, gear on the rung, no toast, no board).
    pub shown: bool,
    /// When the tiles first showed for this rung (UI clock, s).
    pub since: Option<f64>,
    /// How far the names have folded, 0 (open) to 1 (icons only).
    pub fold: f32,
    /// The rung the tiles are of.
    pub rung: Option<usize>,
    /// A staged tail (`ui_scene passives`): its tiles, folded or not.
    pub staged: Option<(Vec<TailTile>, bool)>,
}

impl GearTailView {
    /// The names, keys and verbs are folded away.
    pub fn folded(&self) -> bool {
        self.fold >= 1.0
    }
}

/// The tail's root (top centre, under the chip).
#[derive(Component)]
pub struct GearTailRoot;

/// A tile.
#[derive(Component, Clone, Debug)]
pub struct GearTile(pub String);

/// A tile's foldable part: the name, key and verb.
#[derive(Component)]
pub struct GearTileText;

/// The chip tail.
pub struct GearTailPlugin;

impl Plugin for GearTailPlugin {
    fn build(&self, app: &mut App) {
        super::overlay::layer(app);
        super::signal::setup(app);
        app.init_resource::<GearTailView>()
            .init_resource::<GunNames>()
            .add_systems(Startup, spawn)
            .add_systems(Update, (gather, draw, fold).chain().after(super::race_chip::RaceChipSystems));
    }
}

/// The staged scenes' two TF2 items (`ui_scene passives`): the Mantreads
/// (worn) and the Thermal Thruster (carried, its slot's key).
pub fn staged_tiles() -> Vec<TailTile> {
    vec![
        TailTile {
            item: GearItem {
                id: "tf2:mantreads".into(),
                name: "Mantreads".into(),
                glyph: "boots".into(),
                summary: Some("Stomp an enemy by landing on his head; less push from blasts and airblast".into()),
                classes: vec!["soldier".into()],
                ..default()
            },
            key: None,
        },
        TailTile {
            item: GearItem {
                id: "tf2:thermal_thruster".into(),
                name: "Thermal Thruster".into(),
                glyph: "jetpack".into(),
                kind: PassiveKind::Carried,
                action: Some("slot2".into()),
                verb: Some("Launch".into()),
                summary: Some("Switch to it and fire to launch through the air; stomps on landing".into()),
                classes: vec!["pyro".into()],
            },
            key: Some("2".into()),
        },
    ]
}

/// A staged board row's gear on rung `rung` (the staged race's): the two
/// items on the sixth and seventh rungs, the Razorback on the eleventh,
/// none elsewhere.
pub fn staged_gear(rung: usize) -> Vec<TailTile> {
    match rung {
        6 | 7 => staged_tiles(),
        10 => vec![TailTile {
            item: GearItem { id: "tf2:razorback".into(), name: "Razorback".into(), glyph: "pack".into(), summary: Some("Blocks one backstab".into()), classes: vec!["sniper".into()], ..default() },
            key: None,
        }],
        _ => Vec::new(),
    }
}

/// Stages the tail (`ui_scene passives`): two TF2 items, `folded` or not;
/// none clears it.
pub fn stage(world: &mut World, on: Option<bool>) {
    if let Some(mut v) = world.get_resource_mut::<GearTailView>() {
        v.staged = on.map(|folded| (staged_tiles(), folded));
    }
}

fn spawn(layer: Res<super::overlay::OverlayLayer>, mut commands: Commands) {
    commands.spawn((
        GearTailRoot,
        UiParityId::new("gear.tail"),
        ChildOf(layer.0),
        Visibility::Hidden,
        GlobalZIndex(super::minigame_scoreboard::SCOREBOARD_Z),
        Pickable::IGNORE,
        Node {
            position_type: PositionType::Absolute,
            left: px(0.0),
            width: percent(100.0),
            justify_content: JustifyContent::Center,
            ..default()
        },
    ));
}

#[allow(clippy::too_many_arguments)]
fn gather(
    look: Look,
    clock: Option<Res<UiClock>>,
    hud: Option<Res<MinigameHud>>,
    ladder: Option<Res<Ladder>>,
    race: Option<Res<super::race_chip::RaceView>>,
    open: Option<Res<super::race_chip::BoardOpen>>,
    toast: Option<Res<super::rank_toast::RankToast>>,
    (tables, names, catalog, console): (
        Option<Res<PassiveTables>>,
        Option<Res<GunNames>>,
        Option<Res<WeaponCatalog>>,
        Option<Res<chimera_core::console::Console>>,
    ),
    me: Query<(&OnRung, Option<&crate::slots::GameIdentity>), With<LocalPlayer>>,
    mut view: ResMut<GearTailView>,
) {
    let now = clock.as_deref().map_or(0.0, |c| c.now);
    let mut next = GearTailView { staged: view.staged.clone(), ..default() };
    if let Some((tiles, folded)) = &view.staged {
        next.tiles = tiles.clone();
        next.shown = true;
        next.rung = Some(6);
        next.since = Some(if *folded { f64::MIN } else { view.since.unwrap_or(now) });
    } else {
        let racing = look.on() && race.as_deref().is_some_and(|r| r.active) && hud.as_deref().is_some_and(|h| h.playing());
        let mine = me.iter().next();
        let ids: Vec<String> = mine.and_then(|(o, _)| o.extras.as_ref()).map(|x| x.passives.clone()).unwrap_or_default();
        let game = mine.and_then(|(_, g)| g).map(|g| g.0.as_str());
        let rung = mine.map(|(o, _)| o.index);
        next.tiles = ids
            .iter()
            .map(|id| {
                let item = gear_item(id, tables.as_deref(), names.as_deref(), catalog.as_deref());
                let key = item.action.as_deref().map(|a| action_key(console.as_deref(), game, a));
                TailTile { item, key }
            })
            .collect();
        // The rank-up toast under the chip says the rung first.
        let times = super::rank_toast::ToastTimes::of(&look);
        let toasting = toast.as_deref().is_some_and(|t| t.pose(now, &times).is_some_and(|p| p.visible()));
        let board = open.as_deref().is_some_and(|o| o.0);
        next.shown = racing && ladder.is_some() && !next.tiles.is_empty() && !toasting && !board;
        next.rung = rung;
        // The fold counts from the first showing on this rung.
        let same = view.rung == rung && view.tiles.iter().map(|t| &t.item.id).eq(next.tiles.iter().map(|t| &t.item.id));
        next.since = match (same, view.since) {
            (true, Some(s)) => Some(s),
            _ if next.shown => Some(now),
            _ => None,
        };
    }
    let p = next.since.map_or(0.0, |s| ((now - s - FOLD_AFTER) as f32 / FOLD_TIME).clamp(0.0, 1.0));
    next.fold = if next.since == Some(f64::MIN) { 1.0 } else { Ease::ExpoOut.sample(p).min(if p >= 1.0 { 1.0 } else { 0.999 }) };
    view.set_if_neq(next);
}

/// Rebuilds the tiles when what they say changes (not the fold).
#[allow(clippy::type_complexity)]
fn draw(
    look: Look,
    view: Res<GearTailView>,
    mut slants: Option<ResMut<Assets<SlantMaterial>>>,
    mut root: Query<(Entity, &mut Node, &mut Visibility), With<GearTailRoot>>,
    mut commands: Commands,
    mut built: Local<Option<(Vec<TailTile>, u32, u32)>>,
) {
    let Ok((root, mut node, mut vis)) = root.single_mut() else { return };
    vis.set_if_neq(if view.shown { Visibility::Inherited } else { Visibility::Hidden });
    if !view.shown {
        return;
    }
    let top = look.val(TAIL_TOP);
    if node.top != top {
        node.top = top;
    }
    let key = (view.tiles.clone(), look.units.height as u32, (look.units.scale_factor * 100.0) as u32);
    if built.as_ref() == Some(&key) {
        return;
    }
    *built = Some(key);
    commands.entity(root).despawn_related::<Children>();
    let bar = (look.units.phys(look.units.cq(0.14))).round().max(1.0);
    commands.entity(root).with_children(|r| {
        r.spawn((Node { column_gap: look.val(0.3), align_items: AlignItems::Stretch, ..default() }, Pickable::IGNORE))
            .with_children(|row| {
                for (i, t) in view.tiles.iter().enumerate() {
                    let mut tile = row.spawn((
                        GearTile(t.item.id.clone()),
                        UiParityId(format!("gear.tile.{}", i + 1)),
                        Pickable::IGNORE,
                        Node {
                            height: look.val(TILE_H),
                            align_items: AlignItems::Center,
                            column_gap: look.val(0.4),
                            // Room for the slant on both sides and the bar.
                            padding: UiRect::axes(look.val(0.75), px(0.0)),
                            flex_shrink: 0.0,
                            ..default()
                        },
                    ));
                    match slants.as_deref_mut() {
                        Some(mats) => {
                            let mut m = SlantMaterial::new(&look.theme, look.tone(Tone::Plate));
                            m.accent = look.tone(Tone::Fg2).into();
                            m.shape.w = bar;
                            tile.insert(MaterialNode(mats.add(m)));
                        }
                        None => {
                            tile.insert(BackgroundColor(look.tone(Tone::Plate)));
                        }
                    }
                    tile.with_children(|c| {
                        gear_icon(c, &look, &t.item.id, &t.item.glyph, look.px(TILE_ICON_H), look.tone(Tone::Fg));
                        c.spawn((
                            GearTileText,
                            Pickable::IGNORE,
                            Node { column_gap: look.val(0.4), align_items: AlignItems::Center, overflow: Overflow::clip(), max_width: look.val(NAME_MAX), ..default() },
                        ))
                        .with_children(|c| {
                            c.spawn((look.tracked(Role::DisplayBold, NAME_SIZE, NAME_SIZE * 0.07, look.tone(Tone::Fg), &t.item.name.to_uppercase()), Pickable::IGNORE));
                            if let (Some(k), Some(verb)) = (&t.key, &t.item.verb) {
                                let parent = c.target_entity();
                                let cmd = c.commands_mut();
                                let cap = chimera_ui::widgets::spawn_pair(cmd, chimera_ui::widgets::kbd(&look.theme, look.fonts.as_deref(), &look.units, k, KEY_SIZE));
                                cmd.entity(cap).insert((ChildOf(parent), Pickable::IGNORE));
                                c.spawn((look.tracked(Role::Mono, KEY_SIZE, KEY_SIZE * 0.08, look.tone(Tone::Fg2), &verb.to_uppercase()), Pickable::IGNORE));
                            }
                        });
                    });
                }
            });
    });
}

/// The fold: the text part's widest shrinks to none and fades.
fn fold(look: Look, view: Res<GearTailView>, mut texts: Query<(&mut Node, Entity), With<GearTileText>>, mut commands: Commands) {
    let f = view.fold;
    let max = look.val(NAME_MAX * (1.0 - f));
    for (mut node, e) in &mut texts {
        if node.max_width != max {
            node.max_width = max;
        }
        let gone = f >= 1.0;
        let display = if gone { Display::None } else { Display::Flex };
        if node.display != display {
            node.display = display;
        }
        commands.entity(e).insert(signal::UiOpacity(1.0 - f));
    }
}
