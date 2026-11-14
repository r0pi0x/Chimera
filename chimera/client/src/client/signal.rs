//! The shell's Signal pieces (docs/plans/active/ugg-ui.md, M2): what the
//! race chip and board (`race_chip`), the kill feed and the fallback HUD
//! build their Signal look from, on the `chimera_ui` kit. `Look` (a system
//! param) holds the app's `Theme`, its faces (`KitFonts`), the units
//! (`UiUnits`) and the games' icons (`WeaponIcons`); `Look::on` is the one
//! switch (`flags.hud.signal`, only the Ultimate Gun Game's theme sets it),
//! so Lucker Party and the plain apps keep today's screens.
//!
//! - Text by role and size in cq (`Look::text`), never under the theme's
//!   smallest text (`UiUnits::text_px`).
//! - The player marks (look book V): `tag` (a solid fill: HOST cobalt, YOU
//!   white, LEAD accent), `fused_tag` (YOU and HOST in one chip split on
//!   the 12° slant), `bot_slot` (the antenna robot in a slot reserved left
//!   of every name, empty for a person), the YOU row (`you_row`: a 7 %
//!   tint inside an outline; hover never outlines).
//! - Gun icons (`gun_icon`): the gun's own game's kill icon
//!   (`WeaponIcons`, CS:S and HL2:DM glyphs, Combat Arms' images) while
//!   `flags.icons.prefer_game` is on, else the kit's fallback set by the
//!   gun's class (`chimera_ui::icons`), else the short name.
//!
//! Known shortcuts: no rolling numbers yet (M3's toast brings them); the
//! look book's icons are rasterized here (`icons`) until the kit's
//! `chimera_ui::icons` lands.

use bevy::{ecs::system::SystemParam, prelude::*};
use chimera_ui::{KitFonts, Role, SlantMaterial, Theme, Tone, UiUnits};

use super::weapon_icons::WeaponIcons;

/// What a Signal screen reads (module doc).
#[derive(SystemParam)]
pub struct Look<'w> {
    pub theme: Res<'w, Theme>,
    pub fonts: Option<Res<'w, KitFonts>>,
    pub units: Res<'w, UiUnits>,
    pub icons: Option<Res<'w, WeaponIcons>>,
    pub images: Option<Res<'w, Assets<Image>>>,
}

/// The kit and the pieces' systems (icons' images, fades), once for the
/// screens that use them.
pub fn setup(app: &mut App) {
    if !app.is_plugin_added::<chimera_ui::ChimeraUiPlugin>() {
        app.add_plugins(chimera_ui::ChimeraUiPlugin);
    }
    if app.world().contains_resource::<icons::IconCache>() {
        return;
    }
    app.init_resource::<icons::IconCache>().add_systems(
        PostUpdate,
        (icons::fill_icons, fade)
            .chain()
            .after(chimera_ui::UiKitSystems)
            .before(bevy::ui::UiSystems::Prepare),
    );
    super::text_warm::TextWarmAppExt::warm_texts(app, warm);
}

/// Every role and size the Signal HUD uses (the race chip and board, the
/// kill feed, the fallback HUD), laid out ahead with the guns' names
/// (`text_warm`): the first Tab or kill makes no glyph atlas.
pub const SIZES: &[(Role, f32)] = &[
    (Role::Mono, 0.42),
    (Role::Mono, 0.46),
    (Role::Mono, 0.5),
    (Role::Mono, 0.52),
    (Role::Mono, 0.54),
    (Role::Mono, 0.56),
    (Role::Mono, 0.6),
    (Role::Mono, 0.62),
    (Role::DisplayBold, 0.6),
    (Role::DisplayBold, 0.62),
    (Role::DisplayBold, 0.66),
    (Role::DisplayBold, 0.72),
    (Role::DisplayBold, 0.86),
    (Role::DisplayBold, 1.02),
    (Role::DisplayBold, 1.2),
    (Role::DisplayBold, 1.3),
    (Role::DisplayBold, 1.35),
];

fn warm(look: Look, names: Option<Res<super::minigame_scoreboard::GunNames>>) -> Vec<super::text_warm::WarmText> {
    if !look.on() {
        return Vec::new();
    }
    let extra: String = names.map(|n| n.short.values().flat_map(|v| v.to_uppercase().chars().collect::<Vec<_>>()).collect()).unwrap_or_default();
    SIZES.iter().map(|(role, size)| super::text_warm::WarmText { font: look.font(*role, *size), extra: extra.clone() }).collect()
}

/// The theme switch for the shell's Signal HUD (`flags.hud.signal`).
pub fn signal_on(theme: Option<&Theme>) -> bool {
    theme.is_some_and(|t| t.flag("hud", "signal"))
}

impl Look<'_> {
    /// The Signal HUD is the app's look (module doc).
    pub fn on(&self) -> bool {
        self.theme.flag("hud", "signal")
    }

    /// `v` cq in logical px, snapped.
    pub fn px(&self, v: f32) -> f32 {
        self.units.snap(self.units.cq(v))
    }

    /// `v` cq as a `Val`.
    pub fn val(&self, v: f32) -> Val {
        self.units.val(v)
    }

    /// One physical pixel.
    pub fn hair(&self) -> f32 {
        self.units.hairline()
    }

    pub fn tone(&self, t: Tone) -> Color {
        self.theme.tone(t)
    }

    /// A text of `role` at `size` cq in `color`, on one line.
    pub fn text(&self, role: Role, size: f32, color: Color, text: &str) -> impl Bundle + use<> {
        let styled = chimera_ui::fonts::text(&self.theme, self.fonts.as_deref(), role, self.units.text_px(size));
        (
            Text::new(styled.case(text)),
            styled.bundle(),
            TextColor(color),
            TextLayout::new(Justify::Left, LineBreak::NoWrap),
        )
    }

    /// The `TextFont` of `role` at `size` cq (to measure or warm).
    pub fn font(&self, role: Role, size: f32) -> TextFont {
        chimera_ui::fonts::text(&self.theme, self.fonts.as_deref(), role, self.units.text_px(size)).font
    }

    /// `motion.dur.<piece>.<key>`, else `or` (the look book's number, for
    /// a theme that turns the look on without it).
    pub fn dur(&self, piece: &str, key: &str, or: f32) -> f32 {
        self.theme.dur_opt(piece, key).unwrap_or(or)
    }

    /// `layout.<screen>.<key>` in cq, else `or`.
    pub fn layout(&self, screen: &str, key: &str, or: f32) -> f32 {
        self.theme.layout_opt(screen, key).unwrap_or(or)
    }

    /// Whether a gun prefers its game's own icon (`flags.icons.prefer_game`).
    pub fn prefer_game_icons(&self) -> bool {
        self.theme.flag("icons", "prefer_game")
    }
}

/// A tag's kind (look book V: solid fills).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TagKind {
    /// HOST: solid cobalt.
    Host,
    /// YOU: solid white.
    You,
    /// LEAD and the leader chip's YOU: the accent.
    Accent,
}

/// A solid tag, `size` cq text (mono, tracked, upper case); its box.
pub fn tag(p: &mut ChildSpawnerCommands, look: &Look, kind: TagKind, text: &str, size: f32) -> Entity {
    let (bg, fg) = match kind {
        TagKind::Host | TagKind::Accent => (look.tone(Tone::Acc), look.tone(Tone::OnAcc)),
        TagKind::You => (look.tone(Tone::Fg), look.tone(Tone::Ink)),
    };
    let k = size / 0.62;
    p.spawn((
        Node {
            padding: UiRect::new(look.val(0.4 * k), look.val(0.4 * k), look.val(0.26 * k), look.val(0.2 * k)),
            flex_shrink: 0.0,
            align_items: AlignItems::Center,
            ..default()
        },
        BackgroundColor(bg),
        Pickable::IGNORE,
    ))
    .with_children(|t| {
        t.spawn((look.text(Role::Mono, size, fg, &text.to_uppercase()), Pickable::IGNORE));
    })
    .id()
}

/// YOU and HOST in one chip split on the slant: white YOU, cobalt HOST
/// (look book V's fused chip); its box.
pub fn fused_tag(p: &mut ChildSpawnerCommands, look: &Look, mats: Option<&mut Assets<SlantMaterial>>, size: f32) -> Entity {
    let Some(mats) = mats else {
        return tag(p, look, TagKind::Host, "Host", size);
    };
    let k = size / 0.62;
    let em = look.units.text_px(size);
    let (pad_t, pad_b) = (look.val(0.26 * k), look.val(0.2 * k));
    let you = mats.add(SlantMaterial::new(&look.theme, look.tone(Tone::Fg)).edges(false, true));
    let host = mats.add(SlantMaterial::new(&look.theme, look.tone(Tone::Acc)).edges(true, false));
    p.spawn((Node { flex_shrink: 0.0, ..default() }, Pickable::IGNORE)).with_children(|f| {
        f.spawn((
            Node {
                padding: UiRect::new(px(0.55 * em), px(0.8 * em), pad_t, pad_b),
                align_items: AlignItems::Center,
                ..default()
            },
            MaterialNode(you),
            Pickable::IGNORE,
        ))
        .with_children(|t| {
            t.spawn((look.text(Role::Mono, size, look.tone(Tone::Ink), "YOU"), Pickable::IGNORE));
        });
        f.spawn((
            Node {
                padding: UiRect::new(px(0.8 * em), px(0.55 * em), pad_t, pad_b),
                margin: UiRect::left(px(-0.27 * em)),
                align_items: AlignItems::Center,
                ..default()
            },
            MaterialNode(host),
            Pickable::IGNORE,
        ))
        .with_children(|t| {
            t.spawn((look.text(Role::Mono, size, look.tone(Tone::OnAcc), "HOST"), Pickable::IGNORE));
        });
    })
    .id()
}

/// A player's marks after the name: the fused chip for you as host, HOST
/// alone for another's host (YOU shows as the row, not a tag).
pub fn host_tag(
    p: &mut ChildSpawnerCommands,
    look: &Look,
    mats: Option<&mut Assets<SlantMaterial>>,
    host: bool,
    you: bool,
    size: f32,
) -> Option<Entity> {
    match (host, you) {
        (true, true) => Some(fused_tag(p, look, mats, size)),
        (true, false) => Some(tag(p, look, TagKind::Host, "Host", size)),
        _ => None,
    }
}

/// The slot left of every name, `size` cq square: the robot mark for a
/// bot, empty for a person (names line up either way); the slot.
pub fn bot_slot(p: &mut ChildSpawnerCommands, look: &Look, bot: bool, size: f32, color: Color) -> Entity {
    let s = look.px(size);
    p.spawn((Node { width: px(s), height: px(s), flex_shrink: 0.0, ..default() }, Pickable::IGNORE))
        .with_children(|b| {
            if bot {
                mark(b, look, Mark::Bot, s, color);
            }
        })
        .id()
}

/// The kit's marks the shell draws.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mark {
    Bot,
    Crown,
    Trophy,
    Kill,
    Death,
}

impl Mark {
    /// The icon's name in the kit's set.
    pub fn key(self) -> &'static str {
        match self {
            Mark::Bot => "bot",
            Mark::Crown => "crown",
            Mark::Trophy => "trophy",
            Mark::Kill => "kill",
            Mark::Death => "death",
        }
    }
}

/// `mark` `h` logical px tall in `color`, as a node in the flow.
pub fn mark(p: &mut ChildSpawnerCommands, look: &Look, mark: Mark, h: f32, color: Color) -> Option<Entity> {
    icons::spawn(p, look, mark.key(), h, color)
}

/// A light tint's alpha over a dark ground: the look book's display-space
/// `a` as Bevy's linear blend (`a^2.2`), else 7 % white reads as 30 %.
pub fn tint(a: f32) -> f32 {
    a.clamp(0.0, 1.0).powf(2.2)
}

/// The YOU row's look: the 7 % tint and the outline (60 %), one physical
/// pixel inside the row.
pub fn you_row(look: &Look) -> (BackgroundColor, BorderColor) {
    (BackgroundColor(look.tone(Tone::Fg).with_alpha(tint(0.07))), BorderColor::all(look.tone(Tone::Fg).with_alpha(tint(0.6))))
}

/// The fallback icon's name for a gun's data: melee by its slot (a
/// knife), else its class (`GunClass`: heavy to lmg, carbine to rifle,
/// heavy pistol to revolver, marksman to sniper, flamethrower to
/// launcher).
pub fn icon_class(data: &chimera_sim::weapon::data::WeaponData) -> Option<&'static str> {
    use chimera_core::game_parts::GunClass as C;
    if data.melee.is_some() {
        return Some("knife");
    }
    Some(match data.class? {
        C::Pistol => "pistol",
        C::Smg => "smg",
        C::Shotgun => "shotgun",
        C::Carbine | C::Rifle => "rifle",
        C::Heavy => "lmg",
        C::Marksman | C::Sniper => "sniper",
        C::HeavyPistol => "revolver",
        C::Launcher | C::Flamethrower => "launcher",
        C::Grenade => "grenade",
    })
}

/// Gun `id`'s icon `h` logical px tall: its game's own while
/// `flags.icons.prefer_game` is on (else after the kit's), the kit's by
/// class (`class`, `icon_class`): its node; `None` when there is none
/// (the caller shows the name).
pub fn gun_icon(p: &mut ChildSpawnerCommands, look: &Look, id: &str, class: Option<&str>, h: f32, color: Color) -> Option<Entity> {
    let game = || look.icons.as_deref().and_then(|i| i.icon(id));
    let own = |p: &mut ChildSpawnerCommands| {
        game().map(|icon| super::kill_feed::icon_slot(p, &icon, look.units.height / 480.0 * 4.0, h, 1.0, look.images.as_deref()))
    };
    if look.prefer_game_icons()
        && let Some(e) = own(p)
    {
        return Some(e);
    }
    if let Some(c) = class.filter(|c| icons::has(c)) {
        return icons::spawn(p, look, c, h, color);
    }
    own(p)
}

/// The kit's look book icons as the shell draws them: the fallback gun
/// set (13 classes, 16 units tall, muzzle right) and the marks (bot,
/// crown, trophy, kill, death), our own design (lookbook icons v2),
/// rasterized per physical height into a coverage image (even-odd, 4x4
/// samples a pixel) and cached (`IconCache`), tinted by `ImageNode`'s
/// colour.
///
/// Known shortcuts: lives here until `chimera_ui::icons` (M0) lands, then
/// moves there.
pub mod icons {
    use std::collections::HashMap;

    use bevy::{
        asset::RenderAssetUsages,
        render::render_resource::{Extent3d, TextureDimension, TextureFormat},
    };

    use super::*;

    /// (name, width, height in units, path: absolute M/L/H/V/Z).
    pub const ICONS: &[(&str, f32, f32, &str)] = &[
        ("pistol", 24.0, 16.0, "M4 2 3 2 2 7 4.6 7 3 15 8 15 9.2 9 10 9 10 12 16 12 17 11 17 9 20 9 20 7 22 7 23 6 23 2 22 2 22 1 20 1 20 2 7 2 7 1 4 1ZM15 9 15 11 11 11 11 9Z"),
        ("revolver", 26.0, 16.0, "M6 3 6 5 4 15 8 15 9.167 9.167 10 10 10 13 15 13 15 10 16 9 16 6 23 6 23 5 26 5 26 1 24 1 24 3 16 3 16 4 15 3 10 3 9 4 9 3 8 3 7 1 4 1ZM11 6 14 6 14 7 11 7ZM11 10 14 10 14 12 11 12Z"),
        ("smg", 36.0, 16.0, "M26 3 23 3 23 5 2 5 1 10 3 10 3.6 7 11 7 11 10 13 10 12 15 15 15 16 10 20 10 20 15 23 15 23 10 26 10 28 8 34 8 34 9 36 9 36 5 34 5 34 6 28 6 28 5 26 5Z"),
        ("rifle", 48.0, 16.0, "M17 3 13 3 13 5 12 5 12 6 3 6 2 11 12 9 12 10 16 10 15 15 18 15 19 10 23 10 24 12 26 15 30 15 28 12 27 10 30 10 30 9 41 9 42 8 46 8 46 9 48 9 48 5 46 5 46 6 44 6 44 2 42 2 42 5 17 5Z"),
        ("lmg", 52.0, 16.0, "M18 5 12 5 12 6 3 6 2 11 12 9 12 10 15 10 14 15 17 15 18 10 21 10 22 15 30 15 29 10 30 10 30 9 40 9 40 10 39 15 41 15 42 10 43 15 45 15 44 10 44 9 42 9 42 8 48 8 48 9 52 9 52 5 48 5 48 6 30 6 30 5 28 5 28 2 18 2ZM26 4 26 5 20 5 20 4Z"),
        ("shotgun", 48.0, 16.0, "M12 5 2 7 1 12 11 10 15 10 14 15 17 15 18 10 28.4 10 28 12 31 12 31 10 32 10 32 12 34 12 34 10 35 10 35 12 38 12 38.4 10 44 10 44 8 38.8 8 39 7 48 7 48 5ZM29 7 28.8 8 22 8 22 7Z"),
        ("sniper", 54.0, 16.0, "M16 4 20 4 20 5 5 5 3 7 2 12 12 10 17 10 16 15 19 15 20 10 23 10 23 12 27 12 27 10 30 10 30 8 50 8 50 9 54 9 54 5 50 5 50 6 30 6 30 5 27 5 27 4 32 4 33 3 33 0 27 0 27 1 20 1 20 0 16 0ZM22 5 22 4 25 4 25 5Z"),
        ("launcher", 48.0, 16.0, "M21 2 21 5 9 5 6 2 2 2 0 12 7 12 9 10 16 10 15 15 18 15 19 10 26 10 25 15 28 15 29 10 36 10 36 5 25 5 25 2ZM42 12 48 9 48 6 42 3 37 3 37 12Z"),
        ("energy", 48.0, 16.0, "M31 6 29 4 14 4 4 6 3 11 16.8 11 16 15 19 15 19.8 11 29 11 31 9 31 8 33.8 8 33 12 35 12 35.8 8 37.8 8 37 12 39 12 39.8 8 41.8 8 41 12 43 12 43.8 8 46 8 48 7 46 6 44.2 6 45 2 43 2 42.2 6 40.2 6 41 2 39 2 38.2 6 36.2 6 37 2 35 2 34.2 6ZM26 6 26 8 18 8 18 6Z"),
        ("crossbow", 44.0, 16.0, "M26 4 20 4 20 6 3 6 2 11 13 9 16 9 15 14 18 14 19 9 22 9 32 15 35 15 39 8 37.857 6 40 6 40 7 44 5 40 3 40 4 36.714 4 35 1 32 1ZM35.429 9 32.276 14.517 24 9ZM27.6 4 32.241 1.422 33.714 4Z"),
        ("grenade", 16.0, 16.0, "M0 4 4 4 4 5 3 5 1 7 1 13 3 15 10 15 12 13 12 7 10 5 9 5 9 4 11 4 13 6 13 12 15 12 15 5 12 2 4 2 4 0 0 0ZM1 1 3 1 3 3 1 3ZM7 6 7 10 11 10 11 11 7 11 7 14 6 14 6 11 2 11 2 10 6 10 6 6Z"),
        ("knife", 34.0, 16.0, "M2 5 1 10 10.6 10 10 13 12 13 13 8 13 10 32 10 34 8 27 5 13.6 5 14 3 12 3 11 8 11 5ZM25 7 25 8 15 8 15 7Z"),
        ("blunt", 32.0, 16.0, "M21 13 29 13 30 12 32 2 31 1 23 1 22 2 21 7 4 7 4 6 1 6 0 7 0 11 4 11 4 10 20.4 10 20 12ZM29 4 29 5 25 5 25 4Z"),
        ("bot", 16.0, 16.0, "M3 4 7 4 7 2 6 2 6 0 10 0 10 2 9 2 9 4 13 4 14 5 14 8 16 8 16 12 14 12 14 14 13 15 3 15 2 14 2 12 0 12 0 8 2 8 2 5ZM4 7 7 7 7 10 4 10ZM9 7 12 7 12 10 9 10ZM5 12 11 12 11 13 5 13Z"),
        ("crown", 22.0, 14.0, "M0 14L2 2L7 8L11 0L15 8L20 2L22 14Z"),
        ("trophy", 16.0, 16.0, "M3 1H13V3H16V6L13 9H12L10 11V13H12V16H4V13H6V11L4 9H3L0 6V3H3ZM1.5 4.5V5.4L3 6.9V4.5ZM14.5 4.5H13V6.9L14.5 5.4Z"),
        ("kill", 16.0, 16.0, "M7 0H9V4H7ZM7 12H9V16H7ZM0 7H4V9H0ZM12 7H16V9H12ZM3 3H13V13H3ZM5 5V11H11V5ZM7 7H9V9H7Z"),
        ("death", 16.0, 16.0, "M3 0H13L16 3V10L13 12V16H3V12L0 10V3ZM3 5V8H7V5ZM9 5V8H13V5ZM7 10L8 8.5L9 10ZM6 13V16H7V13ZM9 13V16H10V13Z"),
    ];

    pub fn get(name: &str) -> Option<(f32, f32, &'static str)> {
        ICONS.iter().find(|(n, ..)| *n == name).map(|(_, w, h, d)| (*w, *h, *d))
    }

    /// Whether the set has icon `name`.
    pub fn has(name: &str) -> bool {
        get(name).is_some()
    }

    /// A path's rings (absolute M, L, H, V, Z and implicit L pairs).
    pub fn rings(d: &str) -> Vec<Vec<Vec2>> {
        let mut out: Vec<Vec<Vec2>> = Vec::new();
        let mut cur = Vec2::ZERO;
        let mut cmd = 'M';
        let mut nums: Vec<f32> = Vec::new();
        let flush = |cmd: char, nums: &mut Vec<f32>, out: &mut Vec<Vec<Vec2>>, cur: &mut Vec2| {
            let mut i = 0;
            while i < nums.len() {
                match cmd {
                    'M' | 'L' if i + 1 < nums.len() => {
                        *cur = Vec2::new(nums[i], nums[i + 1]);
                        if cmd == 'M' && i == 0 {
                            out.push(vec![*cur]);
                        } else if let Some(r) = out.last_mut() {
                            r.push(*cur);
                        }
                        i += 2;
                    }
                    'H' => {
                        cur.x = nums[i];
                        if let Some(r) = out.last_mut() {
                            r.push(*cur);
                        }
                        i += 1;
                    }
                    'V' => {
                        cur.y = nums[i];
                        if let Some(r) = out.last_mut() {
                            r.push(*cur);
                        }
                        i += 1;
                    }
                    _ => i += 1,
                }
            }
            nums.clear();
        };
        let mut num = String::new();
        for c in d.chars().chain(std::iter::once(' ')) {
            if c.is_ascii_digit() || c == '.' || (c == '-' && num.is_empty()) {
                num.push(c);
                continue;
            }
            if !num.is_empty() {
                nums.push(num.parse().unwrap_or(0.0));
                num.clear();
            }
            if c.is_ascii_alphabetic() {
                flush(cmd, &mut nums, &mut out, &mut cur);
                cmd = c.to_ascii_uppercase();
            }
        }
        flush(cmd, &mut nums, &mut out, &mut cur);
        out.retain(|r| r.len() >= 3);
        out
    }

    /// Whether `p` is inside `rings` by the even-odd rule.
    fn inside(rings: &[Vec<Vec2>], p: Vec2) -> bool {
        let mut odd = false;
        for r in rings {
            let mut j = r.len() - 1;
            for i in 0..r.len() {
                let (a, b) = (r[i], r[j]);
                if (a.y > p.y) != (b.y > p.y) && p.x < (b.x - a.x) * (p.y - a.y) / (b.y - a.y) + a.x {
                    odd = !odd;
                }
                j = i;
            }
        }
        odd
    }

    /// Icon `name`'s coverage at `h` pixels tall: (width, alpha bytes).
    pub fn coverage(name: &str, h: u32) -> Option<(u32, Vec<u8>)> {
        let (w_u, h_u, d) = get(name)?;
        let h = h.max(1);
        let s = h as f32 / h_u;
        let w = (w_u * s).ceil().max(1.0) as u32;
        let rings = rings(d);
        let mut out = vec![0u8; (w * h) as usize];
        const N: u32 = 4;
        for y in 0..h {
            for x in 0..w {
                let mut hits = 0;
                for sy in 0..N {
                    for sx in 0..N {
                        let p = Vec2::new(
                            (x as f32 + (sx as f32 + 0.5) / N as f32) / s,
                            (y as f32 + (sy as f32 + 0.5) / N as f32) / s,
                        );
                        if inside(&rings, p) {
                            hits += 1;
                        }
                    }
                }
                out[(y * w + x) as usize] = (hits * 255 / (N * N)) as u8;
            }
        }
        Some((w, out))
    }

    /// Rasterized icons by (name, physical height).
    #[derive(Resource, Default)]
    pub struct IconCache(HashMap<(&'static str, u32), Handle<Image>>);

    /// An icon node waiting for its image: `fill_icons` rasterizes it at
    /// the node's physical height.
    #[derive(Component, Clone, Debug)]
    pub struct SignalIcon {
        pub name: &'static str,
        pub px: u32,
        pub color: Color,
    }

    /// Icon `name`, `h` logical px tall, tinted `color` (nothing when the
    /// set has none by that name).
    pub fn spawn(p: &mut ChildSpawnerCommands, look: &Look, name: &str, h: f32, color: Color) -> Option<Entity> {
        let Some((w_u, h_u, _)) = get(name) else { return None };
        let Some(&(key, ..)) = ICONS.iter().find(|(n, ..)| *n == name) else { return None };
        let phys = (h * look.units.scale_factor).round().max(1.0);
        let h = phys / look.units.scale_factor.max(1e-3);
        let w = (w_u * phys / h_u).ceil() / look.units.scale_factor.max(1e-3);
        Some(
            p.spawn((
                Node { width: px(w), height: px(h), flex_shrink: 0.0, ..default() },
                SignalIcon { name: key, px: phys as u32, color },
                Pickable::IGNORE,
            ))
            .id(),
        )
    }

    /// Gives each new icon node its image (from the cache or rasterized).
    pub fn fill_icons(
        mut cache: ResMut<IconCache>,
        mut images: Option<ResMut<Assets<Image>>>,
        new: Query<(Entity, &SignalIcon), Without<ImageNode>>,
        mut commands: Commands,
    ) {
        let Some(images) = images.as_deref_mut() else { return };
        for (e, icon) in &new {
            let handle = cache
                .0
                .entry((icon.name, icon.px))
                .or_insert_with(|| {
                    let (w, alpha) = coverage(icon.name, icon.px).unwrap_or((1, vec![0]));
                    let data: Vec<u8> = alpha.iter().flat_map(|a| [255, 255, 255, *a]).collect();
                    images.add(Image::new(
                        Extent3d { width: w, height: icon.px.max(1), depth_or_array_layers: 1 },
                        TextureDimension::D2,
                        data,
                        TextureFormat::Rgba8UnormSrgb,
                        RenderAssetUsages::RENDER_WORLD | RenderAssetUsages::MAIN_WORLD,
                    ))
                })
                .clone();
            commands.entity(e).insert(ImageNode::new(handle).with_color(icon.color));
        }
    }
}

/// A painted node's own alphas, kept the first time `fade` dims it.
#[derive(Component, Clone, Copy, Debug)]
pub struct BaseAlpha {
    bg: f32,
    text: f32,
    border: f32,
    image: f32,
}

/// Fades a node and everything under it by its timeline's opacity
/// (`Prop::Opacity`): Bevy UI has no group opacity, so each colour's
/// alpha is its own times the opacity. Only when the timeline moved.
#[allow(clippy::type_complexity)]
pub fn fade(
    faders: Query<(Entity, &chimera_ui::Timeline), Changed<chimera_ui::Timeline>>,
    children: Query<&Children>,
    mut paint: Query<(
        Option<&mut BackgroundColor>,
        Option<&mut TextColor>,
        Option<&mut BorderColor>,
        Option<&mut ImageNode>,
        Option<&BaseAlpha>,
    )>,
    mut commands: Commands,
) {
    for (root, tl) in &faders {
        let Some(o) = tl.value(chimera_ui::Prop::Opacity) else { continue };
        let o = o.clamp(0.0, 1.0);
        for e in std::iter::once(root).chain(children.iter_descendants(root)) {
            let Ok((bg, text, border, image, base)) = paint.get_mut(e) else { continue };
            let base = match base {
                Some(b) => *b,
                None => {
                    let b = BaseAlpha {
                        bg: bg.as_ref().map_or(1.0, |c| c.0.alpha()),
                        text: text.as_ref().map_or(1.0, |c| c.0.alpha()),
                        border: border.as_ref().map_or(1.0, |c| c.top.alpha()),
                        image: image.as_ref().map_or(1.0, |c| c.color.alpha()),
                    };
                    commands.entity(e).try_insert(b);
                    b
                }
            };
            if let Some(mut c) = bg {
                let a = base.bg * o;
                if c.0.alpha() != a {
                    c.0.set_alpha(a);
                }
            }
            if let Some(mut c) = text {
                let a = base.text * o;
                if c.0.alpha() != a {
                    c.0.set_alpha(a);
                }
            }
            if let Some(mut c) = border {
                let a = base.border * o;
                if c.top.alpha() != a {
                    c.top.set_alpha(a);
                    c.right.set_alpha(a);
                    c.bottom.set_alpha(a);
                    c.left.set_alpha(a);
                }
            }
            if let Some(mut c) = image {
                let a = base.image * o;
                if c.color.alpha() != a {
                    c.color.set_alpha(a);
                }
            }
        }
    }
}
