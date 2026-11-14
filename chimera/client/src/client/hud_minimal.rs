//! The minimal HUD (`chimera:minimal`, the HUD slot `MINIMAL_HUD`): what a
//! game that lends no HUD shows when the app falls back to the shell's own
//! (`PartsRegistry::fallback_hud`). Plain text: health and armour (bottom
//! left), the held weapon's id and its clip and reserve (bottom right), a
//! cross crosshair. A playing minigame's panel is the shell's overlay's
//! (`client::minigame_panel`), as over every game's HUD.
//!
//! Its roots are `HudRoot(MINIMAL_HUD)`, so `HudSwitchPlugin` shows them
//! only while the gameplay's HUD is the minimal one and hides them when a
//! game's own HUD is up; while it is up it takes over the shell's status
//! texts and crosshair (`HudDraws`). An app adds `MinimalHudPlugin`.
//!
//! Under the Signal look (`signal::signal_on`) the texts give way to the
//! look book's HUD (`signal_hud`): health bottom left on a plate ("HP",
//! the number, ten slanted segments), ammo bottom right (the clip's
//! number, "/ reserve", a bar a round of the clip, lit while loaded). It
//! stays this HUD's (`HudRoot(MINIMAL_HUD)`), so it shows only for a game
//! or gun that lends none, never over a game's HUD.
//!
//! Known shortcuts: the weapon is called by its id (no display-name
//! registry in the shell); no kill feed or scoreboard (the opt-in `cl_hitmarker` works over it).

use bevy::prelude::*;
use chimera_core::game_parts::MINIMAL_HUD;

use super::hud::{GameDraws, HudDraws, HudRoot, HudSwitchPlugin};
use crate::{
    core::{Health, LocalPlayer},
    weapon::{Armor, Inventory, Magazine, Weapon},
};

/// The minimal HUD's nodes and what it takes over from the shell's.
pub struct MinimalHudPlugin;

impl Plugin for MinimalHudPlugin {
    fn build(&self, app: &mut App) {
        if !app.is_plugin_added::<HudSwitchPlugin>() {
            app.add_plugins(HudSwitchPlugin);
        }
        app.init_resource::<HudDraws>();
        app.world_mut().resource_mut::<HudDraws>().0.push((
            MINIMAL_HUD,
            GameDraws {
                crosshair: true,
                status: true,
                ..default()
            },
        ));
        super::signal::setup(app);
        app.add_systems(Startup, spawn).add_systems(Update, (draw, signal_hud::draw).chain());
    }
}

#[derive(Component)]
pub struct MinimalStatus;
#[derive(Component)]
pub struct MinimalAmmo;
/// The mode switch's centre notice (`weapon::notice::CenterNotice`).
#[derive(Component)]
pub struct MinimalNotice;

const COLOR: Color = Color::srgb(0.95, 0.95, 0.95);

fn spawn(mut commands: Commands) {
    let text = |size: f32| (Text::default(), TextFont { font_size: FontSize::Px(size), ..default() }, TextColor(COLOR));
    commands
        .spawn((
            HudRoot(MINIMAL_HUD),
            Visibility::Hidden,
            GlobalZIndex(40),
            Node {
                position_type: PositionType::Absolute,
                width: percent(100.0),
                height: percent(100.0),
                ..default()
            },
        ))
        .with_children(|root| {
            root.spawn((
                MinimalStatus,
                text(24.0),
                Node { position_type: PositionType::Absolute, bottom: px(14.0), left: px(20.0), ..default() },
            ));
            root.spawn((
                MinimalAmmo,
                text(24.0),
                TextLayout::justify(Justify::Right),
                Node { position_type: PositionType::Absolute, bottom: px(14.0), right: px(20.0), ..default() },
            ));
            // The weapon mode switch's notice, under the crosshair.
            root.spawn((
                MinimalNotice,
                text(20.0),
                TextLayout::justify(Justify::Center),
                Node {
                    position_type: PositionType::Absolute,
                    left: px(0.0),
                    width: percent(100.0),
                    top: percent(60.0),
                    ..default()
                },
            ));
            // The crosshair: two bars across the screen's middle.
            for (w, h) in [(14.0, 2.0), (2.0, 14.0)] {
                root.spawn((
                    BackgroundColor(COLOR),
                    Node {
                        position_type: PositionType::Absolute,
                        left: percent(50.0),
                        top: percent(50.0),
                        width: px(w),
                        height: px(h),
                        margin: UiRect { left: px(-w / 2.0), top: px(-h / 2.0), ..default() },
                        ..default()
                    },
                ));
            }
        });
}

#[allow(clippy::type_complexity)]
fn draw(
    look: super::signal::Look,
    player: Option<Single<(&Health, Option<&Armor>, Option<&Inventory>), With<LocalPlayer>>>,
    weapons: Query<(&Weapon, Option<&Magazine>)>,
    mut status: Single<
        &mut Text,
        (With<MinimalStatus>, Without<MinimalAmmo>, Without<MinimalNotice>),
    >,
    mut ammo: Single<
        &mut Text,
        (With<MinimalAmmo>, Without<MinimalStatus>, Without<MinimalNotice>),
    >,
    mut notice: Single<
        &mut Text,
        (With<MinimalNotice>, Without<MinimalStatus>, Without<MinimalAmmo>),
    >,
    center: Option<Res<crate::weapon::notice::CenterNotice>>,
    time: Res<Time>,
) {
    if look.on() {
        // The Signal HUD draws health and ammo (`signal_hud`).
        if !status.0.is_empty() {
            status.0.clear();
        }
        if !ammo.0.is_empty() {
            ammo.0.clear();
        }
    }
    let wanted = center.and_then(|c| c.text(time.elapsed_secs()).map(str::to_string)).unwrap_or_default();
    if notice.0 != wanted {
        notice.0 = wanted;
    }
    let (mut s, mut a) = (String::new(), String::new());
    if let Some(p) = player {
        let (health, armor, inv) = *p;
        s = format!("+ {:.0}", (health.current * 100.0).ceil());
        if let Some(armor) = armor.filter(|a| a.amount > 0.0) {
            s += &format!("   armour {:.0}", (armor.amount * 100.0).round());
        }
        a = match inv.and_then(|i| i.active).and_then(|w| weapons.get(w).ok()) {
            Some((w, Some(m))) => format!("{}\n{} | {}", w.id, m.clip, m.reserve),
            Some((w, None)) => w.id.to_string(),
            None => String::new(),
        };
    }
    if !look.on() {
        status.0 = s;
        ammo.0 = a;
    }
}

/// The fallback HUD in the Signal look (module doc).
pub mod signal_hud {
    use chimera_ui::{Role, TickBarMaterial, TickLayout, Tone};

    use super::*;
    use crate::client::signal::Look;

    /// The health plate (bottom left) and the ammo plate (bottom right).
    #[derive(Component)]
    pub struct SignalHp;
    #[derive(Component)]
    pub struct SignalAmmo;

    /// What the plates show: health 0-100, clip, clip size, reserve, and
    /// the window they were laid out for.
    #[derive(Clone, Debug, Default, PartialEq)]
    pub struct Shown {
        pub hp: Option<u32>,
        pub ammo: Option<(u32, u32, u32)>,
        pub size: (u32, u32),
    }

    /// Ten health segments.
    pub const SEGMENTS: u32 = 10;
    /// The most round bars drawn (a bigger clip draws this many, scaled).
    pub const MAX_BARS: u32 = 40;

    #[allow(clippy::type_complexity)]
    pub fn draw(
        look: Look,
        player: Option<Single<(&Health, Option<&Inventory>), With<LocalPlayer>>>,
        weapons: Query<&Magazine>,
        roots: Query<(Entity, &HudRoot)>,
        old: Query<Entity, Or<(With<SignalHp>, With<SignalAmmo>)>>,
        mut ticks: Option<ResMut<Assets<TickBarMaterial>>>,
        mut last: Local<Shown>,
        mut commands: Commands,
    ) {
        let mut shown = Shown { size: (look.units.height as u32, (look.units.scale_factor * 100.0) as u32), ..default() };
        if look.on()
            && let Some(p) = player
        {
            let (health, inv) = *p;
            shown.hp = Some(((health.current * 100.0).ceil().max(0.0)) as u32);
            shown.ammo = inv.and_then(|i| i.active).and_then(|w| weapons.get(w).ok()).map(|m| (m.clip, m.size, m.reserve));
        }
        if *last == shown && (shown.hp.is_none() || !old.is_empty()) {
            return;
        }
        *last = shown.clone();
        for e in &old {
            commands.entity(e).despawn();
        }
        let Some(hud) = roots.iter().find(|(_, r)| r.0 == MINIMAL_HUD).map(|(e, _)| e) else { return };
        let Some(hp) = shown.hp else { return };
        let sf = look.units.scale_factor.max(1e-3);
        let plate = look.tone(Tone::Plate);
        let plate_node = |left: bool| Node {
            position_type: PositionType::Absolute,
            left: if left { look.val(2.2) } else { Val::Auto },
            right: if left { Val::Auto } else { look.val(2.2) },
            bottom: look.val(2.2),
            padding: UiRect::axes(look.val(0.9), look.val(0.6)),
            column_gap: look.val(0.8),
            align_items: AlignItems::Center,
            ..default()
        };
        let mut bar = |n: u32, lit: u32, pitch_cq: f32, tick_cq: f32, h_cq: f32, skew: bool| {
            let pitch = look.units.phys(look.units.cq(pitch_cq)).round().max(2.0) as u32;
            let tick = look.units.phys(look.units.cq(tick_cq)).round().clamp(1.0, pitch as f32 - 1.0) as u32;
            let h = look.units.phys(look.units.cq(h_cq)).round().max(3.0) as u32;
            let lay = TickLayout { pitch, tick, gap: pitch - tick, w: n * pitch - (pitch - tick), h };
            let extra = if skew { (h as f32 * look.theme.tan()).ceil() as u32 } else { 0 };
            let mut mat = TickBarMaterial::new(&look.theme, n, lay).unlit(look.tone(Tone::Fg4));
            if skew {
                mat = mat.skewed(&look.theme);
            }
            mat.set(lit, n);
            let node = Node { width: px((lay.w + extra) as f32 / sf), height: px(h as f32 / sf), flex_shrink: 0.0, ..default() };
            (node, ticks.as_deref_mut().map(|t| MaterialNode(t.add(mat))))
        };
        let glow = |v: &str| (look.text(Role::DisplayBold, 1.35, look.tone(Tone::Fg), v), TextShadow { offset: Vec2::ZERO, color: look.tone(Tone::Glow) });
        let lit = ((hp as f32 / 100.0) * SEGMENTS as f32).ceil().min(SEGMENTS as f32) as u32;
        let (seg_node, seg_mat) = bar(SEGMENTS, lit, 0.62, 0.46, 0.8, true);
        commands
            .spawn((SignalHp, ChildOf(hud), plate_node(true), BackgroundColor(plate), Pickable::IGNORE))
            .with_children(|p| {
                p.spawn((look.text(Role::Mono, 0.5, look.tone(Tone::Fg3), "HP"), Pickable::IGNORE));
                p.spawn((glow(&hp.to_string()), Pickable::IGNORE));
                let mut s = p.spawn((seg_node, Pickable::IGNORE));
                if let Some(m) = seg_mat {
                    s.insert(m);
                }
            });
        if let Some((clip, size, reserve)) = shown.ammo {
            let n = size.clamp(1, MAX_BARS);
            let lit = if size == 0 { 0 } else { ((clip as f32 / size as f32) * n as f32).ceil() as u32 };
            let (bars_node, bars_mat) = bar(n, lit.min(n), 0.28, 0.16, 0.8, false);
            commands
                .spawn((SignalAmmo, ChildOf(hud), plate_node(false), BackgroundColor(plate), Pickable::IGNORE))
                .with_children(|p| {
                    p.spawn((
                        Node { min_width: look.val(1.9), justify_content: JustifyContent::FlexEnd, ..default() },
                        Pickable::IGNORE,
                    ))
                    .with_children(|v| {
                        v.spawn((glow(&clip.to_string()), Pickable::IGNORE));
                    });
                    p.spawn((look.text(Role::Mono, 0.56, look.tone(Tone::Fg3), &format!("/ {reserve}")), Pickable::IGNORE));
                    let mut b = p.spawn((bars_node, Pickable::IGNORE));
                    if let Some(m) = bars_mat {
                        b.insert(m);
                    }
                });
        }
    }
}
