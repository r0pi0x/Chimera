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
//!   `icons.prefer_game` is on, else the kit's fallback set by the gun's
//!   class (`chimera_ui::icons`: `choose`, `IconFor`), else the short name.
//!
//! Known shortcuts: no rolling numbers yet (M3's toast brings them).

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

/// `setup` ran.
#[derive(Resource)]
struct SignalSetUp;

/// The kit and the pieces' systems (icons' images, fades), once for the
/// screens that use them.
pub fn setup(app: &mut App) {
    if !app.is_plugin_added::<chimera_ui::ChimeraUiPlugin>() {
        app.add_plugins(chimera_ui::ChimeraUiPlugin);
    }
    if app.world().contains_resource::<SignalSetUp>() {
        return;
    }
    app.insert_resource(SignalSetUp).init_resource::<chimera_ui::icons::IconImages>().add_systems(
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

    /// Whether a gun prefers its game's own icon (`icons.prefer_game`).
    pub fn prefer_game_icons(&self) -> bool {
        self.theme.icons.prefer_game
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

/// A solid tag, `size` cq text (the kit's `widgets::tag`); its node.
pub fn tag(p: &mut ChildSpawnerCommands, look: &Look, kind: TagKind, text: &str, size: f32) -> Entity {
    let kind = match kind {
        TagKind::Host => chimera_ui::widgets::TagKind::Host,
        TagKind::You => chimera_ui::widgets::TagKind::You,
        TagKind::Accent => chimera_ui::widgets::TagKind::Accent,
    };
    let (node, label) = chimera_ui::widgets::tag(&look.theme, look.fonts.as_deref(), &look.units, kind, text, size);
    p.spawn((node, Pickable::IGNORE))
        .with_children(|t| {
            t.spawn((label, Pickable::IGNORE));
        })
        .id()
}

/// YOU and HOST in one chip split on the slant (the kit's
/// `widgets::spawn_you_host`); its node.
pub fn fused_tag(p: &mut ChildSpawnerCommands, look: &Look, mats: Option<&mut Assets<SlantMaterial>>, size: f32) -> Entity {
    let Some(mats) = mats else {
        return tag(p, look, TagKind::Host, "Host", size);
    };
    let parent = p.target_entity();
    let cmd = p.commands_mut();
    let chip = chimera_ui::widgets::spawn_you_host(cmd, mats, &look.theme, look.fonts.as_deref(), &look.units, size);
    cmd.entity(chip).insert((Pickable::IGNORE, Node { flex_shrink: 0.0, flex_direction: FlexDirection::Row, ..default() }));
    cmd.entity(parent).add_child(chip);
    chip
}

/// A player's marks after the name: the fused chip for you as host, HOST
/// alone for another's host (YOU shows as the row, not a tag); its node.
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
/// bot, empty for a person (names line up either way); its node.
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

/// `mark` `h` logical px tall in `color`, as a node in the flow; its node.
pub fn mark(p: &mut ChildSpawnerCommands, look: &Look, mark: Mark, h: f32, color: Color) -> Option<Entity> {
    icons::spawn(p, look, mark.key(), h, color)
}

/// The YOU row's look: the 7 % tint and the outline (60 %), one physical
/// pixel inside the row (the kit's `widgets::row_you`).
pub fn you_row(look: &Look) -> impl Bundle + use<> {
    chimera_ui::widgets::row_you(&look.theme, &look.units)
}

/// The fallback icon's name for a gun's data: melee by its slot (a
/// knife), else its class (`GunClass`: heavy to lmg, carbine to rifle,
/// heavy pistol to revolver, marksman to sniper, flamethrower to
/// launcher).
pub fn icon_class(data: &chimera_sim::weapon::data::WeaponData) -> Option<&'static str> {
    use chimera_ui::icons::IconFor;
    if data.melee.is_some() {
        return Some(IconFor::for_melee(true));
    }
    data.class.map(IconFor::gun_class)
}

/// Gun `id`'s icon `h` logical px tall: its game's own (a glyph, or an
/// image whose art is loaded) while `icons.prefer_game` is on, else the
/// kit's by class (`class`, `icon_class`; the kit's default without
/// one), so the cell is never empty. Its node.
pub fn gun_icon(p: &mut ChildSpawnerCommands, look: &Look, id: &str, class: Option<&str>, h: f32, color: Color) -> Option<Entity> {
    use chimera_ui::icons::{IconSource, choose};
    // A game's image that isn't loaded (its install's art missing) is none.
    let game = look.icons.as_deref().and_then(|i| i.icon(id)).filter(|icon| match icon {
        super::weapon_icons::WeaponIcon::Image { image, .. } => look.images.as_deref().is_some_and(|i| i.get(image).is_some()),
        super::weapon_icons::WeaponIcon::Glyph { .. } => true,
    });
    match (choose(look.prefer_game_icons(), game.is_some()), game) {
        (IconSource::Game, Some(icon)) => {
            Some(super::kill_feed::icon_slot(p, &icon, look.units.height / 480.0 * 4.0, h, 1.0, look.images.as_deref()))
        }
        _ => {
            // Never an empty cell: the class's, else the kit's default.
            let key = class.filter(|c| icons::has(c)).unwrap_or_else(|| chimera_ui::icons::IconFor::for_class(""));
            icons::spawn(p, look, key, h, color)
        }
    }
}

/// The kit's icons (`chimera_ui::icons`: the fallback gun set and the
/// marks) as the shell draws them: a node waits for its image
/// (`SignalIcon`), rasterized per physical height (`IconImages`) and
/// tinted by `ImageNode`'s colour.
pub mod icons {
    use std::sync::OnceLock;

    use chimera_ui::icons::{IconImages, Icons};

    use super::*;

    fn shipped() -> &'static Icons {
        static ICONS: OnceLock<Icons> = OnceLock::new();
        ICONS.get_or_init(Icons::shipped)
    }

    /// Whether the kit has icon `name`.
    pub fn has(name: &str) -> bool {
        shipped().get(name).is_some()
    }

    /// An icon node waiting for its image: `fill_icons` gives it one at
    /// `px` physical pixels tall.
    #[derive(Component, Clone, Debug)]
    pub struct SignalIcon {
        pub name: String,
        pub px: u32,
        pub color: Color,
    }

    /// Icon `name`, `h` logical px tall, tinted `color` (nothing when the
    /// kit has none by that name); its node.
    pub fn spawn(p: &mut ChildSpawnerCommands, look: &Look, name: &str, h: f32, color: Color) -> Option<Entity> {
        let icon = shipped().get(name)?;
        let sf = look.units.scale_factor.max(1e-3);
        let phys = (h * sf).round().max(1.0);
        let w = (phys * icon.aspect()).ceil() / sf;
        p.spawn((
            Node { width: px(w), height: px(phys / sf), flex_shrink: 0.0, ..default() },
            SignalIcon { name: name.to_string(), px: phys as u32, color },
            Pickable::IGNORE,
        ))
        .id()
        .into()
    }

    /// Gives each new icon node its image.
    pub fn fill_icons(
        mut cache: ResMut<IconImages>,
        mut images: Option<ResMut<Assets<Image>>>,
        new: Query<(Entity, &SignalIcon), Without<ImageNode>>,
        mut commands: Commands,
    ) {
        let Some(images) = images.as_deref_mut() else { return };
        for (e, icon) in &new {
            if let Some(handle) = cache.get(images, &icon.name, icon.px) {
                commands.entity(e).insert(ImageNode::new(handle).with_color(icon.color));
            }
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
