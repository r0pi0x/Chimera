//! The loading screen (specs/combat_arms/ui_system.md H: the loading
//! screen's layout, shown from a map load's start to its end, stretched
//! by x_r, y_r): while the shell joins a server or follows its map change
//! (`client::front_end`'s events, observed: `Joining`, until
//! `EnteredGame`, `MapLoadFailed` or `LeftGame`; the party apps keep it
//! to Combat Arms' maps, `party_kit::main_menu`), the install's loading layout
//! (`ui::install::LOADING_FILE`) bound by name: `MapImage` the map's
//! picture (`UI_SCR_Loading\MapLoading\MapImage\<map>.dtx`, read when
//! the screen opens), `mapName` and its shadow the map's name (the
//! joining label: the address until the map is known), `LoadingGauge`
//! the join's progress. `CaLoading` is the state, headless; without the
//! install's layout nothing is drawn (the shell's console says what
//! loads).
//!
//! The gauge runs on the real clock by the join's part
//! (`LoadingGauge`, ui_system.md Corrections 7); `mapmode` shows the
//! mode's name and the Quarantine backdrop its mode when the mode is
//! known (`CaLoading::mode`); the cursor is hidden while it shows.
//!
//! Known shortcuts (the most literal reading): the join's parts are our
//! reading of its stages (`part`: connecting and the server's info the
//! early part, a level change, verifying and downloading the level's
//! setting up, loading the map entering the world); nothing tells the
//! client the room's mode while joining yet (so the mode line and the
//! Quarantine backdrop stay hidden) or whether the room is official;
//! there is no room data, so the picture is the map's by name
//! (`loading_picture` has the spec's fallback for when there is); the
//! operation's description and the two tip banners (the map's and the
//! mode's guide entries) have no data (samples in the layout: hidden),
//! and tips whose string ids the table lacks are hidden; a local `map`
//! loads in one frame and shows no loading screen.

use bevy::prelude::*;

use super::{
    CaUi,
    draw::{self, CaArt, CaFaces},
    hud::{HudScreen, bound, layer, model::Screen},
    install::texture_key,
    layout::{Layout, Strings},
};
use crate::client::{canvas::Scale, fonts::UiFonts, front_end as fe};

/// The loading screen's state: what is loading (the joining label), how
/// far (0 to 1), and the room's mode when known (its key).
#[derive(Resource, Clone, Debug, Default, PartialEq)]
pub struct CaLoading {
    pub label: Option<String>,
    pub progress: f32,
    pub mode: Option<String>,
}

/// The loading gauge (ui_system.md H, Corrections 7), percent on the real
/// millisecond clock: until the level starts being set up it rises 0.2
/// a ms, at most 15, the time T summed; when entering the world starts it
/// jumps to 55 and then rises 15 / T a ms, at most 100.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct LoadingGauge {
    pub percent: f64,
    /// The early part's time, ms.
    pub early: f64,
    /// Rising after the jump: its speed (percent a ms).
    pub speed: Option<f64>,
}

/// Where a join is, for the gauge.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LoadingPart {
    /// Before the level starts being set up.
    Early,
    /// The level being set up (the gauge holds).
    Level,
    /// Entering the world.
    Entering,
}

impl LoadingGauge {
    /// `dt` ms more in `part`.
    pub fn advance(&mut self, dt: f64, part: LoadingPart) {
        match (part, self.speed) {
            (LoadingPart::Early, None) => {
                self.early += dt;
                self.percent = (self.percent + 0.2 * dt).min(15.0);
            }
            (LoadingPart::Level, None) => {}
            (LoadingPart::Entering, None) => {
                self.percent = 55.0;
                self.speed = Some(if self.early > 0.0 { 15.0 / self.early } else { f64::INFINITY });
            }
            (_, Some(v)) => self.percent = (self.percent + v * dt).min(100.0),
        }
    }
}

/// The gauge's part for a join stage (ours: connecting and the server's
/// info are the early part, a level change, verifying and downloading
/// the level's setting up, loading the map entering the world).
pub fn part(stage: crate::net::client::JoinStage) -> LoadingPart {
    use crate::net::client::JoinStage as S;
    match stage {
        S::Idle | S::Connecting | S::ServerInfo => LoadingPart::Early,
        S::ChangingLevel | S::Verifying | S::Downloading => LoadingPart::Level,
        S::LoadingMap | S::Joined => LoadingPart::Entering,
    }
}

/// The loading picture (Corrections 7): the room data's for the map and
/// mode (`picture`), else the same map's team deathmatch one, when that
/// texture exists (`exists`).
pub fn loading_picture(
    map: &str,
    mode: &str,
    picture: impl Fn(&str, &str) -> Option<String>,
    exists: impl Fn(&str) -> bool,
) -> Option<String> {
    [mode, "TDM"]
        .into_iter()
        .filter_map(|m| picture(map, m))
        .find(|t| exists(t))
}

/// The mode line (`mapmode`, Corrections 7): the mode's full name, with
/// the string table's "official" word and a space in front in official
/// rooms.
pub fn mode_line(name: &str, official: Option<&str>) -> String {
    match official {
        Some(word) => format!("{word} {name}"),
        None => name.to_string(),
    }
}

/// The map picture's texture for `label` (a map id or name; `None` for
/// an address).
pub fn map_image(label: &str) -> Option<String> {
    let name = label.rsplit(':').next().unwrap_or(label).trim().to_lowercase();
    if name.is_empty() || name.contains('.') || name.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    Some(texture_key(&format!("UI_SCR_Loading\\MapLoading\\MapImage\\{name}")))
}

/// The loading layout bound to `state` at `scale` (None: the layout lacks
/// its controls).
pub fn build(src: &Layout, state: &CaLoading, image: Option<&str>, strings: &Strings, scale: Scale) -> Option<Layout> {
    let label = state.label.as_deref()?;
    let mut l = Layout::new();
    l.scale = scale;
    for top in src.tree.get(0).children.clone() {
        l.tree.graft(0, &src.tree, top);
    }
    let find = |l: &Layout, name: &str| l.tree.find(name);
    find(&l, "mapName")?;
    let id = find(&l, "MapImage");
    match image {
        Some(t) => bound::set_texture(&mut l, id, t),
        None => bound::show(&mut l, id, false),
    }
    let name = label.rsplit(':').next().unwrap_or(label).to_uppercase();
    for n in ["mapName", "mapName_shadow"] {
        let id = find(&l, n);
        bound::set_text(&mut l, id, Some(&name), None);
    }
    // The mode's line and Quarantine's backdrop, when the mode is known.
    let mode = state.mode.as_deref().and_then(crate::game::rules::mode_by_key);
    let id = find(&l, "mapmode");
    match mode {
        Some(m) => bound::set_text(&mut l, id, Some(&mode_line(m.shown_as, None)), None),
        None => bound::show(&mut l, id, false),
    }
    let id = find(&l, "Quarantine");
    bound::show(&mut l, id, state.mode.as_deref() == Some("QRT"));
    let id = find(&l, "LoadingGauge");
    bound::set_fraction(&mut l, id, state.progress);
    bound::hide_samples(&mut l, 0);
    // A tip whose string id the table lacks would show the raw id (R2):
    // hidden.
    for id in l.tree.order() {
        let missing = l
            .widget(id)
            .text()
            .is_some_and(|t| t.set.is_none() && t.element.starts_with('$') && strings.resolve(&t.element) == t.element);
        if missing {
            l.tree.get_mut(id).show = false;
        }
    }
    Some(l)
}

/// The loading screen and its hooks.
pub struct CaLoadingPlugin;

impl Plugin for CaLoadingPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<CaLoading>()
            .add_systems(Startup, |mut commands: Commands| {
                commands.spawn((LoadingRoot, layer(60)));
            })
            .add_systems(Update, (track, draw, cursor).chain());
        app.add_observer(|j: On<fe::Joining>, mut l: Option<ResMut<CaLoading>>| {
            if let Some(l) = l.as_deref_mut() {
                l.label = Some(j.label.clone());
            }
        })
        .add_observer(|_: On<fe::EnteredGame>, l: Option<ResMut<CaLoading>>| done(l))
        .add_observer(|_: On<fe::MapLoadFailed>, l: Option<ResMut<CaLoading>>| done(l))
        .add_observer(|_: On<fe::LeftGame>, l: Option<ResMut<CaLoading>>| done(l));
    }
}

/// The loading screen is done (the game entered, the load failed, the
/// game left).
fn done(l: Option<ResMut<CaLoading>>) {
    if let Some(mut l) = l {
        *l = CaLoading::default();
    }
}

#[derive(Component)]
struct LoadingRoot;

/// The join's progress into `CaLoading`: the gauge on the real clock by
/// the join's part.
fn track(
    join: Option<Res<crate::net::client::JoinProgress>>,
    real: Res<Time<Real>>,
    mut gauge: Local<LoadingGauge>,
    mut state: ResMut<CaLoading>,
) {
    if state.label.is_none() {
        *gauge = LoadingGauge::default();
        return;
    }
    let stage = join.map_or(crate::net::client::JoinStage::Idle, |j| j.stage);
    gauge.advance(real.delta().as_secs_f64() * 1000.0, part(stage));
    let p = (gauge.percent / 100.0) as f32;
    if state.progress != p {
        state.progress = p;
    }
}

/// The cursor is hidden while the loading screen shows (Corrections 7).
fn cursor(state: Res<CaLoading>, cursor: Option<Single<&mut bevy::window::CursorOptions>>, mut hid: Local<bool>) {
    let Some(mut cursor) = cursor else { return };
    let showing = state.label.is_some();
    if showing && cursor.visible {
        cursor.visible = false;
        *hid = true;
    } else if !showing && *hid {
        *hid = false;
        if !crate::client::input::cursor_grabbed(&cursor) {
            cursor.visible = true;
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn draw(
    state: Res<CaLoading>,
    ui: Res<CaUi>,
    screen: Option<Res<HudScreen>>,
    fonts: Option<Res<UiFonts>>,
    faces: Option<Res<CaFaces>>,
    mut art: Option<ResMut<CaArt>>,
    mut images: Option<ResMut<Assets<Image>>>,
    root: Single<Entity, With<LoadingRoot>>,
    mut last: Local<Option<(CaLoading, Screen)>>,
    mut commands: Commands,
) {
    let screen = screen.map_or(Screen::new(1024.0, 768.0), |s| s.0);
    if last.as_ref() == Some(&(state.clone(), screen)) {
        return;
    }
    *last = Some((state.clone(), screen));
    commands.entity(*root).despawn_related::<Children>();
    let (Some(src), Some(label)) = (ui.install.as_ref().and_then(|i| i.loading.as_ref()), state.label.as_deref()) else {
        return;
    };
    // The map's picture, read when first needed (they are large).
    let mut image = map_image(label);
    if let (Some(key), Some(art), Some(images)) = (image.clone(), art.as_mut(), images.as_mut())
        && art.get(&key).is_none()
    {
        match crate::game::INFO.mount().and_then(|m| super::install::decode(&m, &key)) {
            Some(a) => {
                let handle = images.add(draw::rgba_image(&a));
                art.0.insert(key, (handle, a.size));
            }
            None => image = None,
        }
    }
    let Some(layout) = build(src, &state, image.as_deref(), &ui.strings, crate::ui::hud::scale(screen)) else { return };
    let draws = layout.draw(&ui.ctx());
    commands.entity(*root).with_children(|c| {
        draw::spawn_draws(c, &draws, &ui.stand_ins, art.as_deref(), fonts.as_deref(), faces.as_deref());
    });
}
