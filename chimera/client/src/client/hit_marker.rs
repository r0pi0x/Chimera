//! An opt-in hit marker for every gun, drawn by the shell over any game's
//! HUD (docs/DESIGN.md, "One HUD layer over several games' HUDs"). Plain
//! CS:S has no hit feedback and Combat Arms has its own hit indicator, so
//! this is off (`cl_hitmarker 0`, archived) until a player turns it on.
//!
//! With `cl_hitmarker 1`, a small cross at the crosshair's centre shows on
//! the shooter's own client for `HIT_SECONDS` when one of its rounds
//! damaged another character: a white "x" for a hit, a yellow "x" for a
//! headshot, a red "X" for a kill (`HitKind`, the strongest of the
//! moment). It reads the shooter's `WeaponEvent::Hit` (the host's own, a
//! network client's from the server's `net::HitConfirm`) and `core::Died`
//! with the local player as the attacker; the victim sees nothing. A hit
//! that dealt no damage (spawn protection, a zero-damage round) or whose
//! target is not a character (one with `Health` and an `Intent`, which
//! players, bots and hostages have; a prop, door or breakable is not, even
//! if something gives it `Health`) shows nothing. While the active HUD
//! draws its own hit indicator (`hud::GameDraws::hit_marker`, Combat
//! Arms' for its guns) the generic one stays out. `HitMarkerView` is the
//! state, headless too (tests read it). Part of `ClientPlugins`.
//!
//! Known shortcuts: no sound (the cvar is a switch only); a plain text
//! glyph, no animation.

use bevy::prelude::*;

use super::hud::GameDraws;
use crate::{
    core::{Died, Health, Hitgroup, Intent, LocalPlayer},
    weapon::{WeaponEvent, WeaponEventKind},
};

/// Seconds the marker stays up.
pub const HIT_SECONDS: f32 = 0.25;

/// `cl_hitmarker`: 1 draws the marker, 0 (the default) does not.
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct HitMarkerSetting {
    pub on: u8,
}

/// What kind of hit the marker shows, weakest first.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub enum HitKind {
    #[default]
    Hit,
    Headshot,
    Kill,
}

/// The marker now: seconds left (0: hidden) and its kind.
#[derive(Resource, Clone, Copy, Debug, Default)]
pub struct HitMarkerView {
    pub left: f32,
    pub kind: HitKind,
}

#[derive(Component)]
struct HitMarkerText;

/// The setting, the state and the node.
pub struct HitMarkerPlugin;

impl Plugin for HitMarkerPlugin {
    fn build(&self, app: &mut App) {
        super::overlay::layer(app);
        app.init_resource::<HitMarkerSetting>()
            .init_resource::<HitMarkerView>()
            .add_message::<Died>()
            .add_systems(Startup, spawn)
            .add_systems(Update, update);
        if app.world().get_resource::<crate::console::Console>().is_some() {
            crate::console::resource_cvar::<HitMarkerSetting, u8>(
                app,
                "chimera:hitmarker",
                "1: a cross shows when your shot damages another player (yellow headshot, red kill); 0: off.",
                |s| &mut s.on,
            );
            app.world_mut().resource_mut::<crate::console::Console>().archive("cl_hitmarker");
        }
    }
}

fn spawn(layer: Res<super::overlay::OverlayLayer>, mut commands: Commands) {
    commands.spawn((
        HitMarkerText,
        ChildOf(layer.0),
        Text::new("x"),
        TextFont {
            font_size: FontSize::Px(26.0),
            ..default()
        },
        TextColor(Color::WHITE),
        TextLayout::justify(Justify::Center),
        Node {
            position_type: PositionType::Absolute,
            top: percent(50.0),
            width: percent(100.0),
            margin: UiRect::top(px(-17.0)),
            ..default()
        },
        Pickable::IGNORE,
        GlobalZIndex(42),
        Visibility::Hidden,
    ));
}

#[allow(clippy::type_complexity)]
fn update(
    mut hits: MessageReader<WeaponEvent>,
    mut died: MessageReader<Died>,
    local: Option<Single<Entity, With<LocalPlayer>>>,
    characters: Query<(), (With<Health>, With<Intent>)>,
    setting: Res<HitMarkerSetting>,
    draws: Option<Res<GameDraws>>,
    time: Res<Time<Real>>,
    mut view: ResMut<HitMarkerView>,
    text: Option<Single<(&mut Visibility, &mut TextColor, &mut Text), With<HitMarkerText>>>,
) {
    let me = local.map(|l| *l);
    let wanted = setting.on != 0 && !draws.is_some_and(|d| d.hit_marker);
    let raise = |view: &mut HitMarkerView, kind: HitKind| {
        if view.left <= 0.0 {
            view.kind = kind;
        } else {
            view.kind = view.kind.max(kind);
        }
        view.left = HIT_SECONDS;
    };
    for e in hits.read().filter(|e| e.shown()) {
        let WeaponEventKind::Hit {
            target,
            amount,
            hitgroup,
        } = e.kind
        else {
            continue;
        };
        if wanted && Some(e.owner) == me && Some(target) != me && amount > 0.0 && characters.contains(target) {
            raise(&mut view, if hitgroup == Hitgroup::Head { HitKind::Headshot } else { HitKind::Hit });
        }
    }
    for d in died.read() {
        if wanted && me.is_some() && d.attacker == me && Some(d.entity) != me {
            raise(&mut view, HitKind::Kill);
        }
    }
    if !wanted {
        view.left = 0.0;
    }
    view.left = (view.left - time.delta_secs()).max(0.0);
    if let Some(t) = text {
        let (mut vis, mut color, mut glyph) = t.into_inner();
        *vis = if view.left > 0.0 { Visibility::Inherited } else { Visibility::Hidden };
        let (c, g) = match view.kind {
            HitKind::Hit => (Color::WHITE, "x"),
            HitKind::Headshot => (Color::srgb(1.0, 0.85, 0.2), "x"),
            HitKind::Kill => (Color::srgb(1.0, 0.2, 0.2), "X"),
        };
        color.0 = c.with_alpha((view.left / HIT_SECONDS).min(1.0));
        if glyph.0 != g {
            glyph.0 = g.into();
        }
    }
}
