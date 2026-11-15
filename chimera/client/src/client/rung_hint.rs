//! The rung hint (the user's decision, 2026-11-15): a rung that declares
//! one (`chimera_sim::minigame::hud::RungHint`, opt-in in its weapon's
//! data: Halo's grenade rung only so far) tells its player which key
//! attacks: the key bound now to the hint's action (the console's binds
//! under the player's game's own layer, `Console::effective_binds`, read
//! each frame, so a rebind shows at once; "unbound" when none) and the
//! verb. Shown while the minigame's HUD says so (about 5 s on arriving on
//! the rung, about 3 s on each dry fire: decided on each machine from the
//! local player's own predicted events, nothing crosses the network).
//!
//! On the shell's overlay layer, the same whichever game's HUD is up:
//! bottom centre, its bottom `BOTTOM_CQ` cq above the window's bottom
//! (above every HUD's bottom row: the party's status band, 8.4 cq, under
//! the chat's lines, and Halo's shield and health block, which keeps
//! under the band's top; `rect` says where, tests check it). Under the
//! Signal look (`signal::signal_on`) it is the UI kit's key cap
//! (`chimera_ui::widgets::kbd`) beside the verb as a short label, both
//! Mono 0.56 cq as the kit's key lockups (Fg on Fg2), on the look's plate
//! tone, faded in and out by the look's feed timings (`motion.dur.feed`
//! `enter`, `leave`); without it, the plain text "Press G to throw" in the
//! overlay's face at the callouts' second-line size. `RungHintView` is
//! the state, headless too. `ui_scene runghint` stages it.
//!
//! Known shortcuts: a first cut of its look (docs/backlog.md).

use bevy::prelude::*;
use chimera_sim::minigame::hud::MinigameHud;
use chimera_ui::{KitFonts, Role, Theme, Tone, UiUnits};

use crate::core::LocalPlayer;

/// Its bottom above the window's bottom, cq.
pub const BOTTOM_CQ: f32 = 11.0;
/// The kit's lockup size (Mono, cq).
pub const SIZE_CQ: f32 = 0.56;

/// What the hint shows now.
#[derive(Resource, Clone, Debug, Default, PartialEq)]
pub struct RungHintView {
    /// The key bound to the action ("G"; "unbound").
    pub key: String,
    pub verb: String,
    /// Shown now.
    pub shown: bool,
    /// Its opacity (the fade).
    pub alpha: f32,
    /// A staged hint (`ui_scene runghint`): shown with this key and verb.
    pub staged: Option<(String, String)>,
}

impl RungHintView {
    /// The plain text ("Press G to throw").
    pub fn text(&self) -> String {
        format!("Press {} to {}", self.key, self.verb)
    }
}

/// Where the hint's box lies in a `width` x `height` logical-px window for
/// a box `w` x `h` px: (left, top, right, bottom), px from the top left.
pub fn rect(width: f32, height: f32, w: f32, h: f32) -> [f32; 4] {
    let units = UiUnits::new(chimera_ui::units::layout_height(width, height), 1.0);
    let bottom = height - units.cq(BOTTOM_CQ);
    [(width - w) / 2.0, bottom - h, (width + w) / 2.0, bottom]
}

/// The hint's root and its content.
#[derive(Component)]
struct HintRoot;

/// The hint.
pub struct RungHintPlugin;

impl Plugin for RungHintPlugin {
    fn build(&self, app: &mut App) {
        super::overlay::layer(app);
        app.init_resource::<RungHintView>().add_systems(Startup, spawn).add_systems(Update, (gather, draw).chain());
    }
}

fn spawn(layer: Res<super::overlay::OverlayLayer>, mut commands: Commands) {
    commands.spawn((
        HintRoot,
        ChildOf(layer.0),
        Visibility::Hidden,
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

/// The key bound to `action` for the local player now ("unbound").
pub fn bound_key(console: Option<&chimera_core::console::Console>, game: Option<&str>, action: &str) -> String {
    let Some(console) = console else { return "unbound".into() };
    let binds = console.effective_binds(game);
    super::binds::keys_for(&binds, &format!("+{action}")).first().map_or_else(|| "unbound".into(), |k| super::binds::display(k))
}

fn gather(
    hud: Option<Res<MinigameHud>>,
    console: Option<Res<chimera_core::console::Console>>,
    identity: Query<&crate::slots::GameIdentity, With<LocalPlayer>>,
    time: Res<Time>,
    theme: Option<Res<Theme>>,
    mut view: ResMut<RungHintView>,
) {
    let now = time.elapsed_secs_f64();
    let game = identity.iter().next().map(|i| i.0.as_str());
    let hint = hud.as_ref().and_then(|h| h.rung_hint.as_ref());
    let (action, verb, shown) = match (&view.staged, hint) {
        (Some((key, verb)), _) => (None, verb.clone(), Some(key.clone())),
        (None, Some(h)) if h.shown(now) => (Some(h.action.clone()), h.verb.clone(), None),
        _ => (None, view.verb.clone(), None),
    };
    let on = shown.is_some() || action.is_some();
    let key = match (shown, action) {
        (Some(k), _) => k,
        (None, Some(a)) => bound_key(console.as_deref(), game, &a),
        _ => view.key.clone(),
    };
    // In and out by the look's feed timings.
    let t = theme.as_deref();
    let (enter, leave) = (t.and_then(|t| t.dur_opt("feed", "enter")).unwrap_or(0.22), t.and_then(|t| t.dur_opt("feed", "leave")).unwrap_or(0.25));
    let dt = time.delta_secs();
    let alpha = if on { (view.alpha + dt / enter.max(1e-3)).min(1.0) } else { (view.alpha - dt / leave.max(1e-3)).max(0.0) };
    let next = RungHintView { key, verb, shown: on, alpha, staged: view.staged.clone() };
    view.set_if_neq(next);
}

#[allow(clippy::too_many_arguments)]
fn draw(
    view: Res<RungHintView>,
    theme: Option<Res<Theme>>,
    kit: Option<Res<KitFonts>>,
    units: Option<Res<UiUnits>>,
    fonts: Option<Res<super::fonts::UiFonts>>,
    window: Query<&Window, With<bevy::window::PrimaryWindow>>,
    mut root: Query<(Entity, &mut Node, &mut Visibility), With<HintRoot>>,
    mut commands: Commands,
    mut built: Local<Option<(String, String, bool, u32)>>,
) {
    let Ok((root, mut node, mut vis)) = root.single_mut() else { return };
    let visible = view.alpha > 0.0;
    vis.set_if_neq(if visible { Visibility::Inherited } else { Visibility::Hidden });
    if !visible {
        return;
    }
    let height = window.iter().next().map_or(720.0, |w| w.height());
    let units = units.as_deref().copied().unwrap_or_default();
    let bottom = Val::Px(units.snap(units.cq(BOTTOM_CQ)));
    if node.bottom != bottom {
        node.bottom = bottom;
    }
    let signal = super::signal::signal_on(theme.as_deref());
    let key = (view.key.clone(), view.verb.clone(), signal, height as u32);
    if built.as_ref() != Some(&key) {
        *built = Some(key);
        commands.entity(root).despawn_related::<Children>();
        match (signal, theme.as_deref()) {
            (true, Some(theme)) => {
                let plate = commands
                    .spawn((
                        ChildOf(root),
                        Pickable::IGNORE,
                        Node {
                            column_gap: units.val(0.45),
                            align_items: AlignItems::Center,
                            padding: UiRect::axes(units.val(0.7), units.val(0.4)),
                            ..default()
                        },
                        BackgroundColor(theme.tone(Tone::Plate)),
                    ))
                    .id();
                let cap = chimera_ui::widgets::spawn_pair(&mut commands, chimera_ui::widgets::kbd(theme, kit.as_deref(), &units, &view.key, SIZE_CQ));
                commands.entity(cap).insert((ChildOf(plate), Pickable::IGNORE));
                let s = chimera_ui::fonts::text(theme, kit.as_deref(), Role::Mono, units.text_px(SIZE_CQ));
                commands.spawn((ChildOf(plate), Pickable::IGNORE, Text::new(view.verb.to_uppercase()), s.bundle(), TextColor(theme.tone(Tone::Fg2))));
            }
            _ => {
                let size = super::callouts::LINE_SIZES[1] * super::overlay::scale(height);
                commands.spawn((
                    ChildOf(root),
                    Pickable::IGNORE,
                    Node { padding: UiRect::axes(px(size * 0.6), px(size * 0.3)), ..default() },
                    BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.45)),
                    children![(Text::new(view.text()), super::overlay::font(fonts.as_deref(), size, false), TextColor(Color::WHITE))],
                ));
            }
        }
    }
    commands.entity(root).insert(super::signal::UiOpacity(view.alpha));
}
