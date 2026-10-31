//! The death screen (specs/combat_arms/death_killcam.md 8) and the death
//! camera's input and camera on the shell: the install's `Hud_KillCam`
//! layout (root `KillCam`) bound by control name to the roles of
//! `game::death_cam::screen::DeathScreen`, as `bound` binds the other HUD
//! panels; places, sizes, fonts and art are the layout's, the values and
//! what shows are code's.
//!
//! - `KillerInfo`: `CamOn` (name, `hp`, `ap`) when the enemy view is on
//!   or the killer is a teammate, else `CamOff` (name); `Rank` isn't
//!   driven (no level data).
//! - `KillerWeapon`: `name` (the item name, `hud::weapon_name`) and `weapon_img` (hidden for another game's
//!   gun, which has no art in the install: the name alone); `DamageWnd`: `Damage` and
//!   `Damage_img` (the zone's texture from the layout's own strings:
//!   `DamagePointIcon<Zone>Tex`); `Target`: `name`, `KillCount` (the
//!   string `$DeadCamKillCount` with N) and `FreeCam_info` (Hunted);
//!   `Banner`: `Banner_icon` and `banner_description`; `Respawn` or the
//!   premium pass's `Respawn_EXPass`: `respawn_gauge` (whole percent,
//!   its bar from the strings `RespawnGauge_Perk` / `_Normal`), `timer`,
//!   `des_FastRespawn_2` (`$FastRespawnFreeDescription`, the count
//!   left) and `des_FastRespawn` (`$FastRespawnFreeCount`, left/maximum).
//! - `CaDeathInputPlugin`: Space, Ctrl and Shift (cycle, Hunted's free
//!   cameras), F7 (fast respawn), the mouse, the wheel and the movement
//!   keys into `CaDeathInput`, and the death camera's pose onto the
//!   shell's camera (`SpecView`), after the shell's own spectator. Pose
//!   and screen only while Combat Arms' HUD is the gameplay's (always in
//!   Combat Arms; in a party app while the dead player is a Combat Arms
//!   soldier: another game's identity keeps that game's death view, the
//!   shell's `client::spectate` for CS:S).
//!
//! Known shortcuts: without the install's layout no screen is drawn (ours
//! has no geometry to give: Open question 2), only the camera; the rank
//! icon isn't drawn; texts a layout holds as samples stay hidden
//! (`bound::hide_samples`).

use std::collections::HashMap;

use bevy::{
    input::mouse::{AccumulatedMouseMotion, AccumulatedMouseScroll, MouseScrollUnit},
    prelude::*,
    window::CursorOptions,
};

use super::bound::{graft_named, hide_samples, set_percent, set_text, set_texture, show};
use crate::{
    client::{
        FirstPersonCamera,
        canvas::{Scale, WidgetId},
        input::MouseSettings,
        spectate::{SpecView, SpectateSet, place_camera},
    },
    core::{LocalPlayer, MapView},
    game::death_cam::{
        CaDeathCam, CaDeathInput, DeathCamSet,
        machine::Input,
        screen::{DeathScreen, FastView},
    },
    ui::{
        CaUi,
        hud::{HudScreen, model::fill},
        layout::{Kind, Layout, Strings},
    },
};

/// The layout's root control.
pub const ROOT: &str = "KillCam";

pub use super::model::fill_all;

/// The kill-cam layout, with the controls by role.
#[derive(Clone, Debug, PartialEq)]
pub struct KillCamPanel {
    source: Layout,
    root: WidgetId,
    /// The weapons' kill icons by id (`install::kill_icons`).
    kill_icons: std::sync::Arc<HashMap<String, String>>,
}

impl KillCamPanel {
    /// `KillCam` of the install's kill-cam layout; None without it.
    pub fn new(layout: &Layout) -> Option<Self> {
        let (source, root) = graft_named(layout, ROOT)?;
        Some(Self {
            source,
            root,
            kill_icons: Default::default(),
        })
    }

    /// With the weapons' kill icons by id (`install::kill_icons`: the
    /// install's copies by their record's icon).
    pub fn with_kill_icons(mut self, icons: std::sync::Arc<HashMap<String, String>>) -> Self {
        self.kill_icons = icons;
        self
    }

    /// The panel showing `screen` at `scale`; `textures` are the layout
    /// attributes' textures by key in lower case (the zone icons
    /// `damagepointicon<zone>tex`, the gauge bars `respawngauge_perk` and
    /// `_normal`).
    pub fn build(&self, screen: &DeathScreen, strings: &Strings, textures: &HashMap<String, String>, scale: Scale) -> Layout {
        let mut l = self.source.clone();
        l.scale = scale;
        let root = self.root;
        l.tree.get_mut(root).show = true;
        let find = |l: &Layout, from: Option<WidgetId>, name: &str| from.and_then(|f| l.tree.find_from(f, name));
        let root_ = Some(root);
        // The killer.
        let info = find(&l, root_, "KillerInfo");
        show(&mut l, info, screen.killer.is_some());
        if let Some(k) = &screen.killer {
            let (on, off) = (find(&l, info, "CamOn"), find(&l, info, "CamOff"));
            show(&mut l, on, k.cam_on);
            show(&mut l, off, !k.cam_on);
            let shown = if k.cam_on { on } else { off };
            let id = find(&l, shown, "name");
            set_text(&mut l, id, Some(&k.name), None);
            let id = find(&l, shown, "Rank");
            show(&mut l, id, false);
            if k.cam_on {
                let id = find(&l, on, "hp");
                set_text(&mut l, id, Some(&k.hp), None);
                let id = find(&l, on, "ap");
                set_text(&mut l, id, Some(&k.ap), None);
            }
        }
        // The killing weapon.
        let weapon = find(&l, root_, "KillerWeapon");
        show(&mut l, weapon, screen.weapon.is_some());
        if let Some(w) = &screen.weapon {
            let id = find(&l, weapon, "name");
            set_text(&mut l, id, Some(&w.name), None);
            let id = find(&l, weapon, "weapon_img");
            // Another game's gun has no art here: its name alone.
            match crate::ui::install::is_own_gun(&w.id) {
                true => set_texture(&mut l, id, &crate::ui::install::kill_art(&self.kill_icons, &w.id)),
                false => show(&mut l, id, false),
            }
        }
        // The last hit.
        let hit = find(&l, root_, "DamageWnd");
        show(&mut l, hit, screen.damage.is_some());
        if let Some(d) = &screen.damage {
            let id = find(&l, hit, "Damage");
            set_text(&mut l, id, Some(&d.text), None);
            let id = find(&l, hit, "Damage_img");
            match textures.get(&d.zone.string().to_lowercase()) {
                Some(t) => set_texture(&mut l, id, t),
                None => show(&mut l, id, false),
            }
        }
        // The watched player.
        let target = find(&l, root_, "Target");
        show(&mut l, target, screen.target.is_some());
        if let Some(t) = &screen.target {
            let id = find(&l, target, "name");
            set_text(&mut l, id, Some(&t.name), None);
            let id = find(&l, target, "KillCount");
            let text = t.kills_you.map(|n| fill(&strings.resolve("$DeadCamKillCount"), &n.to_string())).unwrap_or_default();
            set_text(&mut l, id, Some(&text), None);
            let id = find(&l, target, "FreeCam_info");
            show(&mut l, id, t.free_cam_info);
        }
        // The tip.
        let banner = find(&l, root_, "Banner");
        show(&mut l, banner, screen.banner.is_some());
        if let Some(b) = &screen.banner {
            let id = find(&l, banner, "Banner_icon");
            set_texture(&mut l, id, &crate::ui::install::texture_key(&b.icon));
            let id = find(&l, banner, "banner_description");
            set_text(&mut l, id, Some(&strings.resolve(&b.text)), None);
        }
        // The respawn countdown.
        let (plain, express) = (find(&l, root_, "Respawn"), find(&l, root_, "Respawn_EXPass"));
        let premium = screen.respawn.as_ref().is_some_and(|r| r.fast == Some(FastView::Premium));
        show(&mut l, plain, screen.respawn.is_some() && !premium);
        show(&mut l, express, premium);
        if let Some(r) = &screen.respawn {
            let panel = if premium { express } else { plain };
            let id = find(&l, panel, "respawn_gauge");
            set_percent(&mut l, id, r.gauge);
            let bar = if r.perk { "respawngauge_perk" } else { "respawngauge_normal" };
            if let (Some(id), Some(art)) = (id, textures.get(bar))
                && let Kind::Gauge(g) = &mut l.widget_mut(id).kind
            {
                g.bar = art.clone();
            }
            let id = find(&l, panel, "timer");
            set_text(&mut l, id, Some(&r.timer), None);
            let (a, b) = (find(&l, panel, "des_FastRespawn_2"), find(&l, panel, "des_FastRespawn"));
            show(&mut l, a, matches!(r.fast, Some(FastView::Free { .. })));
            show(&mut l, b, matches!(r.fast, Some(FastView::Free { .. })));
            // The layout's third description isn't a role of the spec's.
            let c = find(&l, panel, "des_FastRespawn_3");
            show(&mut l, c, false);
            if let Some(FastView::Free { left, max }) = r.fast {
                let (left, max) = (left.to_string(), max.to_string());
                set_text(&mut l, a, Some(&fill_all(&strings.resolve("$FastRespawnFreeDescription"), &[&left])), None);
                set_text(&mut l, b, Some(&fill_all(&strings.resolve("$FastRespawnFreeCount"), &[&left, &max])), None);
            }
        }
        hide_samples(&mut l, root);
        l
    }
}


// ---------------------------------------------------------------------------
// The shell: input, camera, drawing

#[derive(Component)]
struct DeathRoot;

/// The death camera's input and camera on the shell, and the screen.
pub struct CaDeathScreenPlugin;

impl Plugin for CaDeathScreenPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<DeathUi>()
            .add_systems(Startup, |mut commands: Commands| {
                commands.spawn((DeathRoot, super::layer(super::HUD_Z)));
            })
            .add_systems(Update, read_input.before(DeathCamSet))
            .add_systems(
                Update,
                place_death_camera
                    .in_set(SpectateSet)
                    .after(place_camera)
                    .after(DeathCamSet)
                    .run_if(crate::gameplay::hud_is(crate::game::GAME)),
            )
            .add_systems(Update, (load_panel, draw_screen).chain().after(DeathCamSet));
    }
}

/// The bound panel, when the install has the layout.
#[derive(Resource, Default)]
pub struct DeathUi {
    pub panel: Option<KillCamPanel>,
    /// The layout attributes' textures the screen draws (`TEXTURES`).
    pub textures: HashMap<String, String>,
    loaded: bool,
}

/// The shell's input into `CaDeathInput`: Space, Ctrl, Shift and F7 as
/// physical keys (not rebindable commands), the mouse, the wheel and the
/// movement keys of a free camera.
#[allow(clippy::too_many_arguments)]
pub fn read_input(
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    motion: Res<AccumulatedMouseMotion>,
    scroll: Res<AccumulatedMouseScroll>,
    settings: Option<Res<MouseSettings>>,
    cursor: Option<Single<&CursorOptions>>,
    console: Option<Res<crate::console::Console>>,
    time: Res<Time>,
    mut input: ResMut<CaDeathInput>,
) {
    let grabbed = cursor.is_none_or(|c| crate::client::input::cursor_grabbed(&c));
    let any = |ks: &[KeyCode]| ks.iter().any(|k| keys.just_pressed(*k));
    let empty = std::collections::BTreeMap::new();
    let binds = console.as_ref().map_or(&empty, |c| &c.binds);
    let bound = |c: &str| crate::client::binds::pressed(binds, &keys, &mouse, c);
    let look = match (&settings, grabbed) {
        (Some(s), true) => s.look_delta(motion.delta),
        _ => Vec2::ZERO,
    };
    let wheel = match scroll.unit {
        MouseScrollUnit::Line => scroll.delta.y,
        MouseScrollUnit::Pixel => scroll.delta.y / 40.0,
    };
    input.0 = Input {
        look,
        wheel: if grabbed { wheel } else { 0.0 },
        space: any(&[KeyCode::Space]),
        ctrl: any(&[KeyCode::ControlLeft, KeyCode::ControlRight]),
        shift: any(&[KeyCode::ShiftLeft, KeyCode::ShiftRight]),
        fast: any(&[KeyCode::F7]),
        axis: Vec2::new(
            bound("+moveright") as i8 as f32 - bound("+moveleft") as i8 as f32,
            bound("+forward") as i8 as f32 - bound("+back") as i8 as f32,
        ),
        run: bound("+sprint") || bound("+speed"),
        dt: time.delta_secs(),
    };
}

/// The death camera's pose on the shell's camera (after the shell's own
/// spectator placed its).
pub fn place_death_camera(death: Res<CaDeathCam>, mut view: ResMut<SpecView>) {
    match death.pose {
        Some(pose) if death.active() => {
            view.set_if_neq(SpecView {
                pose: Some(pose),
                in_eye: None,
            });
        }
        _ => {}
    }
}

/// The install's kill-cam layout, once.
fn load_panel(ui: Res<CaUi>, mut state: ResMut<DeathUi>) {
    if state.loaded {
        return;
    }
    state.loaded = true;
    state.panel = ui
        .install
        .as_ref()
        .and_then(|i| Some(KillCamPanel::new(i.kill_cam.as_ref()?)?.with_kill_icons(i.kill_icons.clone())));
    if let Some(inst) = ui.install.as_ref() {
        state.textures = crate::ui::install::DEATH_LAYOUT_ART
            .iter()
            .filter_map(|k| Some((k.to_string(), inst.hud_texture(k)?)))
            .collect();
    }
}

/// The screen while the player is dead (hidden during a map camera).
#[allow(clippy::too_many_arguments)]
/// The killing gun by its display name, as the HUD names it
/// (`hud::weapon_name`: the item table's, another game's from its
/// strings, else its data's), not its id.
pub fn name_weapon(screen: &mut DeathScreen, items: &crate::ui::hud::ItemNames, catalog: &crate::weapon::data::WeaponCatalog) {
    if let Some(w) = screen.weapon.as_mut() {
        w.name = crate::ui::hud::weapon_name(items, catalog, &w.id);
    }
}

fn draw_screen(
    death: Res<CaDeathCam>,
    state: Res<DeathUi>,
    hud: Res<crate::ui::hud::CaHud>,
    ui: Res<CaUi>,
    params: super::PenParams,
    art: Option<Res<crate::ui::draw::CaArt>>,
    map_view: Query<(), (With<LocalPlayer>, With<MapView>)>,
    gameplay: Option<Res<crate::gameplay::Gameplay>>,
    root: Single<Entity, With<DeathRoot>>,
    mut last: Local<Option<(DeathScreen, crate::ui::hud::Screen, bool)>>,
    mut commands: Commands,
    names: (Res<crate::ui::hud::ItemNames>, Option<Res<crate::weapon::data::WeaponCatalog>>),
) {
    let Some(panel) = &state.panel else { return };
    let loaded = art.as_ref().is_some_and(|a| !a.0.is_empty());
    let ours = gameplay.is_none_or(|g| g.hud == crate::game::GAME);
    let up = death.screen_up && map_view.is_empty() && ours;
    let mut shown = if up { death.screen.clone() } else { DeathScreen::default() };
    if let Some(catalog) = names.1.as_deref() {
        name_weapon(&mut shown, &names.0, catalog);
    }
    if !super::rebuild(&mut last, (shown.clone(), hud.screen, loaded), *root, &mut commands) || !up {
        return;
    }
    let layout = panel.build(&shown, &ui.strings, &state.textures, super::scale(hud.screen));
    let draws = layout.draw(&ui.ctx());
    commands.entity(*root).with_children(|c| {
        crate::ui::draw::spawn_draws(c, &draws, &ui.stand_ins, art.as_deref(), params.1.as_deref(), params.2.as_deref());
    });
}

#[allow(dead_code)]
type _Unused = (FirstPersonCamera, HudScreen);
