//! The local player's HUD (crosshair, health, ammo, killfeed,
//! sniper scope) and bodies for other characters until character models
//! exist.
//!
//! Covers: crosshair (gap from spread, CS:S's `cl_crosshaircolor` presets,
//! `cl_crosshairscale`, `cl_crosshairalpha` with `cl_crosshairusealpha`,
//! `cl_dynamiccrosshair`), sniper scope overlay, smoke grey and flash white
//! screen tints, health, ammo, killfeed, capsule bodies for other
//! characters. A game that draws its own crosshair, health and ammo, hit
//! marker (`hit_marker`: the opt-in generic one, `client::hit_marker`, stays out) or kill feed turns this HUD's off with `GameDraws`;
//! the shell's overlay layer (`client::overlay`: a minigame's panel and
//! callouts) is no part of it, so no game's HUD hides or replaces it.
//!
//! An app whose HUD changes (`gameplay::Gameplay::hud`: Lucker Party's
//! local player follows its weapon's game, `chimera_sim::identity`) keeps
//! only the active one up: a game's HUD roots (`HudRoot`) show only while
//! its HUD is the gameplay's; `GameDraws` is the active HUD's
//! (`HudDraws`); the look (`map::hud::ActiveHud`) is the look of the game
//! whose HUD draws, among the map's and its guests' (`HudLookOwner`,
//! `PutAway`), and none for a HUD without one: a game's HUD takes its parts
//! down when the look goes and builds them when it is back.
//! `Radars` (`hud_radar`) says whether the games' radars draw; an app with
//! one kill feed for all its HUDs (`kill_feed::SharedKillFeed`) has this
//! HUD's left out too.

use bevy::prelude::*;

use super::{
    FirstPersonCamera,
    ui_write::{edit, set_text},
};
use crate::{
    core::{Dead, Died, Health, Intent, LocalPlayer, Score, Team},
    weapon::{Hitscan, Inventory, Magazine, Weapon, Zoomed},
};

pub struct HudPlugin;

impl Plugin for HudPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<crate::map::MapHooks>();
        app.world_mut().resource_mut::<crate::map::MapHooks>().settle.push(warm_huds);
        app.init_resource::<Killfeed>()
            .init_resource::<GameDraws>()
            .init_resource::<CrosshairColor>()
            .init_resource::<ScopeOverlayImage>()
            .init_resource::<ScopeOverlayTexel>()
            .add_systems(Startup, spawn_hud)
            .add_systems(
                Update,
                (
                    character_bodies,
                    show_bodies,
                    (killfeed, draw_hud, draw_crosshair, draw_scope).chain(),
                    (scope_aspect, game_scope)
                        .chain()
                        .run_if(scope_inputs_changed),
                    screen_tints,
                ),
            )
;
        if !app.is_plugin_added::<HudSwitchPlugin>() {
            app.add_plugins(HudSwitchPlugin);
        }
        crosshair_cvars(app);
    }
}

/// Only the gameplay's HUD up, in an app with several (`HudRoot`,
/// `HudDraws`, `HudLookOwner`); `HudPlugin` adds it.
pub struct HudSwitchPlugin;

impl Plugin for HudSwitchPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<GameDraws>().init_resource::<PutAway>();
        radar_cvar(app);
        app.add_systems(
            Update,
            (
                show_active_huds,
                draws_follow_hud.run_if(resource_exists_and_changed::<crate::gameplay::Gameplay>),
                put_away_hud_look.run_if(resource_exists::<HudLookOwner>),
            )
                .in_set(HudSwitch),
        );
    }
}

/// Where the active HUD's roots and `GameDraws` follow the gameplay's
/// HUD (`HudSwitchPlugin`): what reads them runs after it, so the frame
/// the HUD changes draws one HUD's parts only.
#[derive(SystemSet, Clone, Debug, PartialEq, Eq, Hash)]
pub struct HudSwitch;

/// Which parts of this HUD a game's own HUD draws instead (none by
/// default: this HUD draws them all). A game's front end that has its own
/// crosshair, health and ammo panels, hit indicator or kill feed sets it
/// (Combat Arms' `ui::hud`); the scope overlay, tints and bodies stay.
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct GameDraws {
    pub crosshair: bool,
    /// Health, armour, ammo (the bottom texts).
    pub status: bool,
    pub hit_marker: bool,
    pub killfeed: bool,
}

/// A root of game `0`'s HUD nodes: shown only while that game's HUD is
/// the gameplay's (`show_active_huds`). A game marks the roots it spawns at
/// start; without a `Gameplay` they show.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub struct HudRoot(pub &'static str);

/// Each game's `GameDraws` by its id, for an app with several HUDs:
/// `GameDraws` is the gameplay's HUD's (one not listed: this HUD draws
/// everything). Empty: `GameDraws` is left as the game set it.
#[derive(Resource, Clone, Debug, Default)]
pub struct HudDraws(pub Vec<(&'static str, GameDraws)>);

/// Whether the games' radars draw (`hud_radar`, archived; 1 unless the
/// app starts it at 0: the party apps, where one game's radar over
/// another's map matches neither and many maps have no overview). CS:S's
/// radar and Combat Arms' corner map read it.
#[derive(Resource, Clone, Copy, Debug, PartialEq, Eq)]
pub struct Radars {
    pub shown: u8,
}

impl Default for Radars {
    fn default() -> Self {
        Self { shown: 1 }
    }
}

impl Radars {
    pub fn shown(&self) -> bool {
        self.shown != 0
    }
}

/// `hud_radar` (`Radars`).
fn radar_cvar(app: &mut App) {
    app.init_resource::<Radars>();
    if app.world().get_resource::<crate::console::Console>().is_some_and(|c| c.cvar("hud_radar").is_some()) {
        return;
    }
    crate::console::resource_cvar::<Radars, u8>(app, "chimera:radar", "1: the games' radars draw; 0: hidden.", |r| &mut r.shown);
    app.world_mut().resource_mut::<crate::console::Console>().archive("hud_radar");
}

/// Starts `hud_radar` (`Radars`) at `shown` in this app, as its default
/// (a config that says otherwise wins when it is read).
pub fn default_radars(app: &mut App, shown: bool) {
    radar_cvar(app);
    let v = shown as u8;
    app.insert_resource(Radars { shown: v });
    app.world_mut().resource_mut::<crate::console::Console>().set_default("hud_radar", &v.to_string());
}

/// An app with several HUDs and looks (`map::hud::ActiveHud`, `GameHud::
/// game`): `ActiveHud` is only ever the look of the game whose HUD is the
/// gameplay's, from the map's own and its guests' (`map::hud::GuestLooks`),
/// or none while that HUD has no look (Combat Arms' draws from its own
/// data). Each swap leaves a frame without one, so every game's HUD takes
/// its parts down as when a map has no look. The value is the game a look
/// that names none belongs to (and what shows with no gameplay). Without
/// the resource the look stays.
#[derive(Resource, Clone, Copy, Debug)]
pub struct HudLookOwner(pub &'static str);

/// The looks not up (`HudLookOwner`), and the map they are of
/// (`map::LoadedMapName`).
#[derive(Resource, Default)]
pub struct PutAway(Vec<(crate::map::hud::ActiveHud, Option<String>)>);

impl PutAway {
    /// `game`'s look while another game's HUD draws (its game still
    /// registers its icons from it: `weapon_icons`).
    pub fn look_of(&self, game: &str) -> Option<&crate::map::hud::ActiveHud> {
        self.0.iter().map(|(l, _)| l).find(|l| l.0.game == game)
    }
}

/// Frames each HUD is shown behind the loading screen (`warm_huds`): its
/// texts laid out, their glyph atlases made.
const WARM_FRAMES: u8 = 4;

/// The HUDs `warm_huds` shows in turn, and where it is.
#[derive(Resource)]
struct WarmingHuds {
    huds: Vec<&'static str>,
    at: usize,
    frames: u8,
}

/// Behind the loading screen (`map::MapHooks::settle`), every HUD of the
/// app's games shown for a few frames (`gameplay::HudWarm`): a HUD's first
/// frame lays out each of its texts and makes Bevy's glyph atlases for each
/// font and size (a 1 MB image each): about 116 M instructions and 30 ms for
/// Combat Arms', on the rung that first gave a Combat Arms gun
/// (docs/performance.md, "Nothing blocks the main thread"). Kept for the
/// session: each map warms again only what changed (the screen's size).
pub fn warm_huds(world: &mut World) -> bool {
    use crate::gameplay::{Gameplay, HudWarm};
    let warming = world.remove_resource::<WarmingHuds>().filter(|w| w.at < w.huds.len());
    let mut warming = match warming {
        Some(w) => w,
        None => {
            if world.remove_resource::<HudWarm>().is_some() {
                return true;
            }
            let Some(registry) = world.get_resource::<chimera_core::game_parts::PartsRegistry>() else {
                return true;
            };
            let mut huds: Vec<&'static str> = Vec::new();
            for parts in registry.all() {
                let hud = parts.hud.unwrap_or(registry.fallback_hud);
                if !huds.contains(&hud) {
                    huds.push(hud);
                }
            }
            // The HUD showing last, so it is the one on when play starts.
            if let Some(now) = world.get_resource::<Gameplay>().map(|g| g.hud) {
                huds.retain(|h| *h != now);
                huds.push(now);
            }
            if huds.len() < 2 {
                return true;
            }
            WarmingHuds { huds, at: 0, frames: 0 }
        }
    };
    let hud = warming.huds[warming.at];
    if warming.frames == 0 {
        world.insert_resource(HudWarm(hud));
        if let Some(mut g) = world.get_resource_mut::<Gameplay>()
            && g.hud != hud
        {
            g.hud = hud;
        }
    }
    warming.frames += 1;
    if warming.frames >= WARM_FRAMES {
        warming.at += 1;
        warming.frames = 0;
    }
    if warming.at >= warming.huds.len() {
        // Done: the next call hands the HUD back (`HudWarm` removed) and
        // the next map starts over.
        return false;
    }
    world.insert_resource(warming);
    false
}

/// Roots of a HUD that isn't the gameplay's hidden; the active one's shown
/// as it becomes active (or appears), then left to its game.
fn show_active_huds(gameplay: Option<Res<crate::gameplay::Gameplay>>, mut roots: Query<(Ref<HudRoot>, &mut Visibility)>) {
    let Some(gameplay) = gameplay else { return };
    for (root, mut vis) in &mut roots {
        if gameplay.hud != root.0 {
            if *vis != Visibility::Hidden {
                *vis = Visibility::Hidden;
            }
        } else if (gameplay.is_changed() || root.is_added()) && *vis == Visibility::Hidden {
            *vis = Visibility::Inherited;
        }
    }
}

/// `GameDraws` from the gameplay's HUD (`HudDraws`).
fn draws_follow_hud(
    gameplay: Res<crate::gameplay::Gameplay>,
    registry: Option<Res<HudDraws>>,
    mut draws: ResMut<GameDraws>,
) {
    let Some(registry) = registry.filter(|r| !r.0.is_empty()) else { return };
    let want = registry.0.iter().find(|(g, _)| *g == gameplay.hud).map(|(_, d)| *d).unwrap_or_default();
    if *draws != want {
        *draws = want;
    }
}

/// The look of the game whose HUD draws, the others put away
/// (`HudLookOwner`).
fn put_away_hud_look(world: &mut World) {
    use crate::map::hud::{ActiveHud, GuestLooks};
    let Some(owner) = world.get_resource::<HudLookOwner>().map(|o| o.0) else { return };
    let want = world.get_resource::<crate::gameplay::Gameplay>().map_or(owner, |g| g.hud);
    let map = world.get_resource::<crate::map::LoadedMapName>().map(|m| m.0.clone());
    let guests = world.remove_resource::<GuestLooks>();
    let whose = |look: &ActiveHud| if look.0.game.is_empty() { owner.to_string() } else { look.0.game.clone() };
    let up = world.get_resource::<ActiveHud>().map(whose);
    let taken = up.as_ref().is_some_and(|g| g != want).then(|| world.remove_resource::<ActiveHud>()).flatten();
    let mut away = world.resource_mut::<PutAway>();
    // A map's guests come with its load: the old map's looks are over.
    if guests.is_some() {
        away.0.clear();
    }
    away.0.retain(|(_, of)| *of == map);
    for look in guests.into_iter().flat_map(|g| g.0).chain(taken) {
        let game = whose(&look);
        away.0.retain(|(l, _)| whose(l) != game);
        away.0.push((look, map.clone()));
    }
    // After a frame without one when one just went.
    if up.is_none() {
        let back = away.0.iter().position(|(l, _)| whose(l) == want);
        if let Some(i) = back {
            let (look, _) = away.0.remove(i);
            world.insert_resource(look);
        }
    }
}

/// The crosshair's look, CS:S's cvars (all archived, CS:S's defaults):
/// `cl_crosshaircolor` (presets 0-4, 5 custom: `cl_crosshaircolor_r`,
/// `_g`, `_b`), `cl_crosshairsize` (line length; 5 is ours unscaled),
/// `cl_crosshairthickness` (line width; 0.5 is ours), `cl_crosshairdot`
/// (a centre dot), `cl_crosshairscale` (0: our size; else the screen
/// height it is drawn for, lower is bigger: 1200 small, 768 medium, 600
/// large), `cl_crosshairusealpha` with `cl_crosshairalpha` (0-255) for
/// translucency, and `cl_dynamiccrosshair` (the gap follows the weapon's
/// spread; 0: a fixed gap). How size and thickness map to pixels is ours
/// (docs/plans/active/ui-parity.md: a reference capture to match).
#[derive(Resource, Clone, Copy, Debug, PartialEq)]
pub struct CrosshairColor {
    pub color: u8,
    pub scale: f32,
    pub alpha: u8,
    pub use_alpha: u8,
    pub dynamic: u8,
    pub size: f32,
    pub thickness: f32,
    pub dot: u8,
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl Default for CrosshairColor {
    fn default() -> Self {
        Self {
            color: 0,
            scale: 0.0,
            alpha: 200,
            use_alpha: 0,
            dynamic: 1,
            size: 5.0,
            thickness: 0.5,
            dot: 0,
            r: 50,
            g: 250,
            b: 50,
        }
    }
}

/// `cl_crosshairsize` and `cl_crosshairthickness` drawing our lines
/// (`LINE_LENGTH`, `LINE_WIDTH`) at their own size.
const BASE_SIZE: f32 = 5.0;
const BASE_THICKNESS: f32 = 0.5;

/// The crosshair's cvars.
pub const CROSSHAIR_CVARS: [&str; 11] = [
    "cl_crosshaircolor",
    "cl_crosshairscale",
    "cl_crosshairalpha",
    "cl_crosshairusealpha",
    "cl_dynamiccrosshair",
    "cl_crosshairsize",
    "cl_crosshairthickness",
    "cl_crosshairdot",
    "cl_crosshaircolor_r",
    "cl_crosshaircolor_g",
    "cl_crosshaircolor_b",
];

pub fn crosshair_cvars(app: &mut App) {
    use crate::console::resource_cvar;
    resource_cvar::<CrosshairColor, u8>(
        app,
        "cl_crosshaircolor",
        "Crosshair colour: 0 green, 1 red, 2 blue, 3 yellow, 4 cyan, 5 custom (cl_crosshaircolor_r/g/b).",
        |c| &mut c.color,
    );
    resource_cvar::<CrosshairColor, f32>(
        app,
        "cl_crosshairscale",
        "Crosshair size: 0 ours; else the screen height it is drawn for (lower is bigger: 1200, 768, 600).",
        |c| &mut c.scale,
    );
    resource_cvar::<CrosshairColor, u8>(
        app,
        "cl_crosshairalpha",
        "Crosshair opacity (0-255) with cl_crosshairusealpha 1.",
        |c| &mut c.alpha,
    );
    resource_cvar::<CrosshairColor, u8>(
        app,
        "cl_crosshairusealpha",
        "1: draw the crosshair cl_crosshairalpha translucent.",
        |c| &mut c.use_alpha,
    );
    resource_cvar::<CrosshairColor, u8>(
        app,
        "cl_dynamiccrosshair",
        "1: the crosshair's gap follows the weapon's spread; 0: a fixed gap.",
        |c| &mut c.dynamic,
    );
    resource_cvar::<CrosshairColor, f32>(app, "cl_crosshairsize", "Crosshair line length (5: the usual).", |c| {
        &mut c.size
    });
    resource_cvar::<CrosshairColor, f32>(
        app,
        "cl_crosshairthickness",
        "Crosshair line width (0.5: the usual).",
        |c| &mut c.thickness,
    );
    resource_cvar::<CrosshairColor, u8>(app, "cl_crosshairdot", "1: a dot at the crosshair's centre.", |c| {
        &mut c.dot
    });
    resource_cvar::<CrosshairColor, u8>(app, "cl_crosshaircolor_r", "Custom crosshair red (0-255).", |c| &mut c.r);
    resource_cvar::<CrosshairColor, u8>(app, "cl_crosshaircolor_g", "Custom crosshair green (0-255).", |c| {
        &mut c.g
    });
    resource_cvar::<CrosshairColor, u8>(app, "cl_crosshaircolor_b", "Custom crosshair blue (0-255).", |c| &mut c.b);
    let mut console = app.world_mut().resource_mut::<crate::console::Console>();
    for name in CROSSHAIR_CVARS {
        console.archive(name);
    }
}

impl CrosshairColor {
    pub fn color(self) -> Color {
        let a = if self.use_alpha != 0 {
            self.alpha as f32 / 255.0
        } else {
            CROSSHAIR_COLOR.alpha()
        };
        match self.color {
            1 => Color::srgba(1.0, 0.3, 0.3, a),
            2 => Color::srgba(0.3, 0.3, 1.0, a),
            3 => Color::srgba(1.0, 1.0, 0.3, a),
            4 => Color::srgba(0.3, 1.0, 1.0, a),
            5 => Color::srgb_u8(self.r, self.g, self.b).with_alpha(a),
            _ => CROSSHAIR_COLOR.with_alpha(a),
        }
    }

    /// How much bigger than ours the lines are drawn in a window `height`
    /// pixels tall (`cl_crosshairscale`).
    pub fn size(self, height: f32) -> f32 {
        if self.scale > 0.0 {
            (height / self.scale).clamp(0.25, 4.0)
        } else {
            1.0
        }
    }

    /// The four lines and the dot (offset of each one's centre from the
    /// crosshair's centre, x right and y down, and its size; the dot's
    /// offset is zero, its size zero when off) for a spread gap in
    /// pixels, in a window `height` pixels tall.
    pub fn lines(self, spread_gap: f32, height: f32) -> [(Vec2, Vec2); 5] {
        let k = self.size(height);
        let gap = MIN_GAP * k + if self.dynamic != 0 { spread_gap } else { 0.0 };
        let length = (LINE_LENGTH * k * self.size.max(0.0) / BASE_SIZE).round();
        let width = (LINE_WIDTH * k * self.thickness.max(0.0) / BASE_THICKNESS).round().max(1.0);
        let out = gap + length / 2.0;
        let dot = if self.dot != 0 { Vec2::splat(width) } else { Vec2::ZERO };
        [
            (Vec2::new(out, 0.0), Vec2::new(length, width)),
            (Vec2::new(-out, 0.0), Vec2::new(length, width)),
            (Vec2::new(0.0, out), Vec2::new(width, length)),
            (Vec2::new(0.0, -out), Vec2::new(width, length)),
            (Vec2::ZERO, dot),
        ]
    }
}

#[derive(Component)]
struct HealthText;
#[derive(Component)]
struct AmmoText;
#[derive(Component)]
struct KillfeedText;
#[derive(Component)]
struct CenterText;
/// The sniper scope overlay (shown while the local player is scoped).
#[derive(Component)]
struct ScopeOverlay;
/// A black bar beside the scope's middle (hidden when the game's picture
/// is sized by `ScopeOverlayTexel`).
#[derive(Component)]
struct ScopeBar;
/// The scope's square middle (the ring, the lens and cross hairs).
#[derive(Component)]
struct ScopeSquare;
/// The stand-in scope picture, kept on the square to go back to when the
/// map's HUD look (CS:S's scope textures) is put away.
#[derive(Component)]
struct GenericScope(Handle<Image>);
/// A game's own scope picture for the weapon looked through now (Combat
/// Arms' `ZoomSpriteName`), set by the game while it is zoomed and loaded,
/// cleared otherwise. Drawn centred at its own aspect with black borders,
/// ahead of the map's HUD look (CS:S's scope textures) and the generic
/// stand-in.
#[derive(Resource, Default, Clone, Debug)]
pub struct ScopeOverlayImage(pub Option<Handle<Image>>);
/// How big a game's own scope picture is drawn
/// (`ScopeOverlayImage`): one texel's side as a fraction of the screen's
/// height, texels square at every aspect, the picture centred and cropped
/// by the screen, nothing filling what it leaves (Combat Arms:
/// specs/combat_arms/weapon_classes.md 7.10). None: the picture fills the
/// view's height at its own aspect between black bars.
#[derive(Resource, Default, Clone, Copy, Debug, PartialEq)]
pub struct ScopeOverlayTexel(pub Option<f32>);

/// The scope picture's rectangle in screen pixels (left, top, width,
/// height) for a `texture` of texels, a `texel` fraction of the screen
/// height and the `screen`'s size.
pub fn scope_card(texture: Vec2, texel: f32, screen: Vec2) -> [f32; 4] {
    let (w, h) = (texture.x * texel * screen.y, texture.y * texel * screen.y);
    [screen.x / 2.0 - w / 2.0, screen.y / 2.0 - h / 2.0, w, h]
}

/// One of the four crosshair lines: its direction from the centre (zero:
/// the centre dot).
#[derive(Component)]
struct CrosshairLine(Vec2);

/// The HUD's own crosshair (CS:S's five dots, the base HUD's
/// `crosshair_default` sprite), drawn when no weapon draws one.
#[derive(Component)]
struct DefaultCrosshair;

/// Which crosshair is drawn.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CrosshairKind {
    /// None: dead, behind a scope, or no player.
    Hidden,
    /// The held weapon's (lines, the gap following its spread).
    Weapon,
    /// No weapon held (e.g. stripped by the map): the HUD's default, CS:S's
    /// five dots.
    Default,
}

/// The crosshair for a player's inventory (None: no player).
pub fn crosshair_kind(inventory: Option<&Inventory>, dead: bool, scoped: bool) -> CrosshairKind {
    match inventory {
        None => CrosshairKind::Hidden,
        Some(_) if dead || scoped => CrosshairKind::Hidden,
        Some(i) if i.active.is_none() => CrosshairKind::Default,
        Some(_) => CrosshairKind::Weapon,
    }
}

/// The default crosshair's dots (stand-in when the install has no sprite):
/// offsets from the centre, pixels.
const DEFAULT_DOT_SPREAD: f32 = 8.0;

#[derive(Resource, Default)]
struct Killfeed(Vec<(String, f32)>);

const HUD_COLOR: Color = Color::srgba(1.0, 0.86, 0.45, 0.9);
const CROSSHAIR_COLOR: Color = Color::srgba(0.3, 1.0, 0.3, 0.9);
const LINE_LENGTH: f32 = 7.0;
const LINE_WIDTH: f32 = 2.0;
const MIN_GAP: f32 = 3.0;
const FEED_SECONDS: f32 = 6.0;

/// Our stand-in for CS:S's scope: black outside a circle filling the
/// screen's height, thin black cross hairs through it, black bars beside it.
fn scope_image() -> Image {
    use bevy::{
        asset::RenderAssetUsages,
        render::render_resource::{Extent3d, TextureDimension, TextureFormat},
    };
    const N: u32 = 512;
    let mut data = Vec::with_capacity((N * N * 4) as usize);
    let c = (N as f32 - 1.0) / 2.0;
    for y in 0..N {
        for x in 0..N {
            let (dx, dy) = (x as f32 - c, y as f32 - c);
            let r = (dx * dx + dy * dy).sqrt() / c;
            // Soft edge over about a pixel; the cross hairs one pixel wide.
            let outside = ((r - 0.99) * c).clamp(0.0, 1.0);
            let line = if dx.abs() < 0.75 || dy.abs() < 0.75 { 1.0 } else { 0.0 };
            let alpha = outside.max(line);
            data.extend_from_slice(&[0, 0, 0, (alpha * 255.0) as u8]);
        }
    }
    Image::new(
        Extent3d {
            width: N,
            height: N,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        data,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::RENDER_WORLD,
    )
}

fn spawn_scope(commands: &mut Commands, images: &mut Assets<Image>) {
    let image = images.add(scope_image());
    let bar = || {
        (
            Node {
                flex_grow: 1.0,
                height: percent(100.0),
                ..default()
            },
            BackgroundColor(Color::BLACK),
            ScopeBar,
        )
    };
    commands.spawn((
        ScopeOverlay,
        Node {
            position_type: PositionType::Absolute,
            width: percent(100.0),
            height: percent(100.0),
            flex_direction: FlexDirection::Row,
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            ..default()
        },
        GlobalZIndex(39),
        Visibility::Hidden,
        children![
            bar(),
            (
                ScopeSquare,
                GenericScope(image.clone()),
                Node {
                    height: percent(100.0),
                    aspect_ratio: Some(1.0),
                    flex_shrink: 0.0,
                    ..default()
                },
                ImageNode::new(image),
            ),
            bar(),
        ],
    ));
}

/// Full-screen tints: grey inside smoke (specs/cs_source/grenades.md
/// 7.4), under the flash overlay (`senses`) and the HUD.
#[derive(Component, Clone, Copy, PartialEq)]
enum ScreenTint {
    Smoke,
}

fn spawn_tints(commands: &mut Commands) {
    for (tint, z) in [(ScreenTint::Smoke, 30)] {
        commands.spawn((
            tint,
            Node {
                position_type: PositionType::Absolute,
                width: percent(100.0),
                height: percent(100.0),
                ..default()
            },
            BackgroundColor(Color::NONE),
            Pickable::IGNORE,
            GlobalZIndex(z),
        ));
    }
}

/// The smoke clouds' grey (colour 0.3, alpha from the camera's place in
/// each cloud).
fn screen_tints(
    mut tints: Query<(&ScreenTint, &mut BackgroundColor)>,
    camera: Query<&GlobalTransform, With<FirstPersonCamera>>,
    clouds: Query<&crate::weapon::grenade::SmokeCloud>,
    time: Res<Time>,
) {
    let now = time.elapsed_secs_f64();
    let smoke = camera.iter().next().map_or(0.0, |c| {
        crate::weapon::grenade::smoke_fog_at(c.translation(), now, clouds.iter())
    });
    for (tint, mut color) in &mut tints {
        let want = match tint {
            ScreenTint::Smoke => Color::srgba(0.3, 0.3, 0.3, smoke),
        };
        if color.0 != want {
            color.0 = want;
        }
    }
}

fn spawn_hud(mut commands: Commands, mut images: ResMut<Assets<Image>>) {
    spawn_tints(&mut commands);
    spawn_scope(&mut commands, &mut images);
    let font = |size: f32| TextFont {
        font_size: FontSize::Px(size),
        ..default()
    };
    commands.spawn((
        HealthText,
        Text::default(),
        font(28.0),
        TextColor(HUD_COLOR),
        Node {
            position_type: PositionType::Absolute,
            bottom: px(14.0),
            left: px(20.0),
            ..default()
        },
        GlobalZIndex(40),
    ));
    commands.spawn((
        AmmoText,
        Text::default(),
        font(28.0),
        TextColor(HUD_COLOR),
        TextLayout::justify(Justify::Right),
        Node {
            position_type: PositionType::Absolute,
            bottom: px(14.0),
            right: px(20.0),
            ..default()
        },
        GlobalZIndex(40),
    ));
    commands.spawn((
        KillfeedText,
        Text::default(),
        font(15.0),
        TextColor(Color::srgb(0.95, 0.95, 0.95)),
        TextLayout::justify(Justify::Right),
        Node {
            position_type: PositionType::Absolute,
            top: px(90.0),
            right: px(10.0),
            ..default()
        },
        GlobalZIndex(40),
    ));
    commands.spawn((
        CenterText,
        Text::default(),
        font(22.0),
        TextColor(Color::srgb(1.0, 0.4, 0.35)),
        TextLayout::justify(Justify::Center),
        Node {
            position_type: PositionType::Absolute,
            top: percent(60.0),
            width: percent(100.0),
            ..default()
        },
        GlobalZIndex(40),
    ));
    commands.spawn((
        DefaultCrosshair,
        ImageNode::default(),
        Node {
            position_type: PositionType::Absolute,
            ..default()
        },
        Visibility::Hidden,
        GlobalZIndex(40),
    ));
    for dir in [Vec2::X, Vec2::NEG_X, Vec2::Y, Vec2::NEG_Y, Vec2::ZERO] {
        commands.spawn((
            CrosshairLine(dir),
            Node {
                position_type: PositionType::Absolute,
                ..default()
            },
            BackgroundColor(CROSSHAIR_COLOR),
            GlobalZIndex(40),
        ));
    }
}

type Local<'a> = (
    &'a Health,
    Option<&'a crate::weapon::Armor>,
    Option<&'a Inventory>,
    Option<&'a Score>,
    Option<&'a Dead>,
);

fn draw_hud(
    player: Option<Single<Local, With<LocalPlayer>>>,
    weapons: Query<(&Weapon, Option<&Magazine>)>,
    mut health_text: Single<&mut Text, (With<HealthText>, Without<AmmoText>, Without<CenterText>)>,
    mut ammo_text: Single<&mut Text, (With<AmmoText>, Without<HealthText>, Without<CenterText>)>,
    mut center: Single<&mut Text, (With<CenterText>, Without<HealthText>, Without<AmmoText>)>,
    game_hud: Option<Res<crate::map::hud::ActiveHud>>,
    spectator: Option<Res<super::spectate::Spectator>>,
    own: Res<GameDraws>,
) {
    let Some(p) = player else { return };
    // The game's own HUD draws health, armour and ammo when there is one;
    // spectating, the spectator panel says who you watch instead.
    let spectating = spectator.is_some_and(|s| s.active());
    let plain = game_hud.is_none() && !spectating && !own.status;
    let (health, armor, inv, score, dead) = *p;
    let score = score.copied().unwrap_or_default();
    let armor = match armor.filter(|a| a.amount > 0.0) {
        Some(a) => format!(
            "    {} {:.0}",
            if a.helmet { "[H]" } else { "[ ]" },
            (a.amount * 100.0).round()
        ),
        None => String::new(),
    };
    // (With the game's HUD, kills and deaths belong on a scoreboard.)
    // Written only on a change (`ui_write`): an unchanged text marked
    // changed is laid out again.
    let health = if !plain {
        String::new()
    } else {
        format!(
            "+ {:.0}{armor}    K {}  D {}",
            (health.current * 100.0).ceil(),
            score.kills,
            score.deaths
        )
    };
    set_text(&mut health_text, &health);
    let ammo = if !plain {
        String::new()
    } else {
        match inv.and_then(|i| i.active).and_then(|w| weapons.get(w).ok()) {
            Some((w, Some(m))) => format!("{}\n{} | {}", short(w.id), m.clip, m.reserve),
            Some((w, None)) => short(w.id).to_string(),
            None => String::new(),
        }
    };
    set_text(&mut ammo_text, &ammo);
    set_text(
        &mut center,
        if dead.is_some() && !spectating {
            "You died. Respawning..."
        } else {
            ""
        },
    );
}

/// `cs_source:weapon_ak47` -> `ak47`.
fn short(id: &str) -> &str {
    let id = id.rsplit(':').next().unwrap_or(id);
    id.strip_prefix("weapon_").unwrap_or(id)
}

/// The size of the space the crosshair's lines are placed in: the UI's
/// (the window the UI camera draws to, in its logical pixels), else the
/// 3D camera's. Not the 3D camera's first: while a dialog is open in a
/// game it draws into a picture (`game_menu::world_behind_dialogs`) whose
/// pixels are physical ones, so on a scaled display (Windows at 125 %)
/// the centre came out right of and below the window's.
pub fn crosshair_space(ui: Option<&bevy::ui::ComputedUiRenderTargetInfo>, camera: &Camera) -> Option<Vec2> {
    ui.map(|i| i.logical_size())
        .filter(|s| s.x > 0.0 && s.y > 0.0)
        .or_else(|| camera.logical_viewport_size())
}

/// Crosshair gap from the held weapon's spread at the camera's field of
/// view.
#[allow(clippy::type_complexity)]
fn draw_crosshair(
    player: Option<Single<(&Inventory, Option<&Dead>, Option<&Zoomed>), With<LocalPlayer>>>,
    (spectating, watched): (Option<Res<super::spectate::SpecView>>, Query<(&Inventory, Option<&Zoomed>)>),
    scans: Query<&Hitscan>,
    camera: Query<(&Camera, &Projection), With<FirstPersonCamera>>,
    mut lines: Query<(
        &CrosshairLine,
        &mut Node,
        &mut Visibility,
        &mut BackgroundColor,
        Option<&bevy::ui::ComputedUiRenderTargetInfo>,
    )>,
    color: Res<CrosshairColor>,
    (hud, mut default_crosshair, mut images, mut sprite): (
        Option<Res<crate::map::hud::ActiveHud>>,
        Query<(&mut ImageNode, &mut Node, &mut Visibility), (With<DefaultCrosshair>, Without<CrosshairLine>)>,
        ResMut<Assets<Image>>,
        bevy::ecs::system::Local<Option<(Handle<Image>, Handle<Image>, Rect)>>,
    ),
    own: Res<GameDraws>,
) {
    let Some((cam, proj)) = camera.iter().next() else {
        return;
    };
    let Some(size) = crosshair_space(lines.iter().find_map(|l| l.4), cam) else {
        return;
    };
    // Watching someone in first person: their weapon's crosshair (CS:S
    // draws the watched player's, in your own crosshair settings).
    let in_eye = spectating.and_then(|s| s.in_eye).and_then(|t| watched.get(t).ok());
    let (inv, dead, zoomed) = match (in_eye, player.map(|p| *p)) {
        (Some((i, z)), _) => (Some(i), None, z),
        (None, Some((i, d, z))) => (Some(i), d, z),
        (None, None) => (None, None, None),
    };
    // A scope draws its own cross hairs.
    let kind = if own.crosshair {
        CrosshairKind::Hidden
    } else {
        crosshair_kind(inv, dead.is_some(), zoomed.is_some_and(|z| z.scope))
    };
    // No weapon: the HUD's default crosshair sprite, drawn additively
    // (converted once to alpha), at its own pixel size.
    let hud_sprite = hud.as_ref().and_then(|h| {
        let s = h.0.sprites.get("crosshair_default")?;
        let handle = h.1.get(&s.texture)?;
        let [x, y, w, h] = s.rect;
        Some((handle, Rect::new(x, y, x + w, y + h)))
    });
    if let Some((handle, rect)) = hud_sprite
        && sprite.as_ref().is_none_or(|(source, ..)| source != handle)
    {
        let converted = images
            .get(handle)
            .and_then(additive_as_alpha)
            .map(|i| images.add(i))
            .unwrap_or_else(|| handle.clone());
        *sprite = Some((handle.clone(), converted, rect));
    }
    let sprite_shown = kind == CrosshairKind::Default && hud_sprite.is_some();
    for (mut image, mut node, mut vis) in &mut default_crosshair {
        vis.set_if_neq(if sprite_shown { Visibility::Inherited } else { Visibility::Hidden });
        if let (true, Some((_, handle, rect))) = (sprite_shown, sprite.as_ref()) {
            if image.image != *handle || image.rect != Some(*rect) {
                image.image = handle.clone();
                image.rect = Some(*rect);
            }
            if image.color != color.color() {
                image.color = color.color();
            }
            let (w, h) = (rect.width(), rect.height());
            edit(&mut node, |node| {
                node.left = px((size.x / 2.0 - w / 2.0).round());
                node.top = px((size.y / 2.0 - h / 2.0).round());
                node.width = px(w);
                node.height = px(h);
            });
        }
    }
    // Without the sprite, five dots drawn with the lines' nodes.
    let dots = kind == CrosshairKind::Default && !sprite_shown;
    let visible = kind == CrosshairKind::Weapon || dots;
    let spread = inv
        .and_then(|i| i.active)
        .and_then(|w| scans.get(w).ok())
        .map_or(0.0, |s| s.spread.width());
    let fov = match proj {
        Projection::Perspective(p) => p.fov,
        _ => 1.0,
    };
    let spread_gap = spread / (fov / 2.0).tan() * size.y / 2.0;
    let shapes = color.lines(spread_gap, size.y);
    let centre = size / 2.0;
    for (line, mut node, mut vis, mut bg, _) in &mut lines {
        bg.set_if_neq(BackgroundColor(color.color()));
        vis.set_if_neq(if visible {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        });
        let d = line.0;
        // The line on that side (UI y grows downward).
        let side = Vec2::new(d.x, -d.y);
        if dots {
            let c = (centre + side * DEFAULT_DOT_SPREAD).round();
            edit(&mut node, |node| {
                node.left = px(c.x - 1.0);
                node.top = px(c.y - 1.0);
                node.width = px(2.0);
                node.height = px(2.0);
            });
            continue;
        }
        let Some(&(offset, Vec2 { x: w, y: h })) = shapes.iter().find(|(o, _)| o.normalize_or_zero() == side) else {
            continue;
        };
        let c = centre + offset;
        edit(&mut node, |node| {
            node.left = px(c.x - w / 2.0);
            node.top = px(c.y - h / 2.0);
            node.width = px(w);
            node.height = px(h);
        });
    }
}

/// The scope overlay's look follows the HUD look present: CS:S's own
/// scope textures (`scope_arc`, `scope_lens` HUD sprites) while the map's
/// HUD look is CS:S's, drawn as the ring from four mirrored quarters, the
/// lens tint under it and thin black cross hairs; our generic stand-in
/// when there is none (the look put away while another game's HUD is the
/// gameplay's, as with a Combat Arms gun: `HudLookOwner`) or it has no
/// scope textures.
fn game_scope(
    hud: Option<Res<crate::map::hud::ActiveHud>>,
    own: Option<Res<ScopeOverlayImage>>,
    square: Single<(Entity, &GenericScope), With<ScopeSquare>>,
    mut commands: Commands,
) {
    if let Some(image) = own.and_then(|o| o.0.clone()) {
        commands
            .entity(square.0)
            .despawn_related::<Children>()
            .insert(ImageNode::new(image));
        return;
    }
    let sprite = |name: &str| {
        let hud = hud.as_ref()?;
        let s = hud.0.sprites.get(name)?;
        Some(hud.1.get(&s.texture)?.clone())
    };
    let (square, generic) = *square;
    let Some(arc) = sprite("scope_arc") else {
        commands
            .entity(square)
            .despawn_related::<Children>()
            .insert(ImageNode::new(generic.0.clone()));
        return;
    };
    let mut e = commands.entity(square);
    e.remove::<ImageNode>().despawn_related::<Children>();
    let abs = |left: f32, top: f32, w: f32, h: f32| Node {
        position_type: PositionType::Absolute,
        left: percent(left),
        top: percent(top),
        width: percent(w),
        height: percent(h),
        ..default()
    };
    e.with_children(|c| {
        if let Some(lens) = sprite("scope_lens") {
            c.spawn((abs(0.0, 0.0, 100.0, 100.0), ImageNode::new(lens)));
        }
        // Stored as the bottom-right quarter.
        for (left, top, flip_x, flip_y) in [(50.0, 50.0, false, false), (0.0, 50.0, true, false), (50.0, 0.0, false, true), (0.0, 0.0, true, true)] {
            c.spawn((
                abs(left, top, 50.0, 50.0),
                ImageNode {
                    image: arc.clone(),
                    color: Color::BLACK,
                    flip_x,
                    flip_y,
                    ..default()
                },
            ));
        }
        c.spawn((abs(0.0, 50.0, 100.0, 0.0), Outline::new(px(0.5), px(0.0), Color::BLACK)));
        c.spawn((abs(50.0, 0.0, 0.0, 100.0), Outline::new(px(0.5), px(0.0), Color::BLACK)));
    });
}

/// The HUD look or the game's scope picture changed or went away.
fn scope_inputs_changed(
    hud: Option<Res<crate::map::hud::ActiveHud>>,
    own: Option<Res<ScopeOverlayImage>>,
    texel: Option<Res<ScopeOverlayTexel>>,
    mut had: bevy::ecs::system::Local<bool>,
) -> bool {
    let removed = *had && hud.is_none();
    *had = hud.is_some();
    removed
        || hud.is_some_and(|h| h.is_changed())
        || own.is_some_and(|o| o.is_changed())
        || texel.is_some_and(|t| t.is_changed())
}

/// The scope's middle: with a game's picture and a texel size, the
/// picture's own size (square texels, scaled by the screen's height,
/// centred, the bars gone: it is cropped by the screen and the world
/// shows around and through it); with a picture alone, its aspect at the
/// view's height between black bars; the others' square.
fn scope_aspect(
    own: Option<Res<ScopeOverlayImage>>,
    texel: Option<Res<ScopeOverlayTexel>>,
    images: Res<Assets<Image>>,
    mut square: Single<&mut Node, With<ScopeSquare>>,
    mut bars: Query<&mut Node, (With<ScopeBar>, Without<ScopeSquare>)>,
) {
    let size = own
        .and_then(|o| o.0.as_ref().and_then(|h| images.get(h)))
        .map(|i| i.size_f32())
        .filter(|s| s.x > 0.0 && s.y > 0.0);
    let ratio = size.map_or(1.0, |s| s.x / s.y);
    let card = size.zip(texel.and_then(|t| t.0));
    // The card sits by absolute offsets from the middle: centred whatever
    // its size, even past the screen's edges.
    let layout = match card {
        Some((s, t)) => {
            let (w, h) = (100.0 * s.x * t, 100.0 * s.y * t);
            (
                PositionType::Absolute,
                [vh(w), vh(h)],
                None,
                [percent(50.0), percent(50.0)],
                UiRect::new(vh(-w / 2.0), Val::ZERO, vh(-h / 2.0), Val::ZERO),
                Display::None,
            )
        }
        None => (
            PositionType::Relative,
            [Val::Auto, percent(100.0)],
            Some(ratio),
            [Val::Auto, Val::Auto],
            UiRect::ZERO,
            Display::Flex,
        ),
    };
    let (position, [width, height], aspect, [left, top], margin, display) = layout;
    let node = &mut **square;
    if (node.position_type, node.width, node.height, node.aspect_ratio, node.left, node.top, node.margin)
        != (position, width, height, aspect, left, top, margin)
    {
        node.position_type = position;
        node.width = width;
        node.height = height;
        node.aspect_ratio = aspect;
        node.left = left;
        node.top = top;
        node.margin = margin;
    }
    for mut bar in &mut bars {
        if bar.display != display {
            bar.display = display;
        }
    }
}

/// Shown while the local player looks through a scope, or while watching
/// in first person someone who does.
fn draw_scope(
    player: Option<Single<Option<&Zoomed>, With<LocalPlayer>>>,
    (spectating, watched): (Option<Res<super::spectate::SpecView>>, Query<&Zoomed>),
    mut overlay: Query<&mut Visibility, With<ScopeOverlay>>,
) {
    let scoped = match spectating.and_then(|s| s.in_eye) {
        Some(t) => watched.get(t).is_ok_and(|z| z.scope),
        None => player.is_some_and(|z| z.is_some_and(|z| z.scope)),
    };
    let want = if scoped {
        Visibility::Inherited
    } else {
        Visibility::Hidden
    };
    for mut v in &mut overlay {
        if *v != want {
            *v = want;
        }
    }
}

fn killfeed(
    mut died: MessageReader<Died>,
    names: Query<(Option<&Name>, Has<LocalPlayer>)>,
    mut feed: ResMut<Killfeed>,
    mut text: Single<&mut Text, With<KillfeedText>>,
    time: Res<Time>,
    game_hud: Option<Res<crate::map::hud::ActiveHud>>,
    own: Res<GameDraws>,
    shared: Option<Res<super::kill_feed::SharedKillFeed>>,
) {
    // The game's own HUD shows its death notices instead, or the app's
    // one feed (`kill_feed`).
    if game_hud.is_some() || own.killfeed || shared.is_some() {
        died.clear();
        set_text(&mut text, "");
        return;
    }
    let name = |e: Entity| match names.get(e) {
        Ok((_, true)) => "You".to_string(),
        Ok((Some(n), _)) => n.to_string(),
        _ => format!("{e}"),
    };
    for d in died.read() {
        let line = match d.attacker.filter(|a| *a != d.entity) {
            Some(a) => format!("{} killed {}", name(a), name(d.entity)),
            None => format!("{} died", name(d.entity)),
        };
        feed.0.push((line, FEED_SECONDS));
    }
    let dt = time.delta_secs();
    feed.0.retain_mut(|(_, left)| {
        *left -= dt;
        *left > 0.0
    });
    if feed.0.len() > 6 {
        let extra = feed.0.len() - 6;
        feed.0.drain(..extra);
    }
    set_text(&mut text, &feed.0.iter().map(|(l, _)| l.as_str()).collect::<Vec<_>>().join("\n"));
}

/// A character drawn as a capsule (`character_bodies`).
#[derive(Component)]
#[component(storage = "SparseSet")]
pub struct CapsuleBody;

/// A capsule for every character that isn't the local player, on maps
/// without character models (or none of its identity's game,
/// `slots::GameIdentity`); on maps with them (map::attach_bodies) the
/// capsules go. Characters outlive map changes (a network game's above
/// all: a client gets them before the server's map is in), so this
/// follows the map, not the character's arrival: a client that joined
/// de_dust2 from the greybox kept a capsule through every model (seen
/// in the network soak's screenshots). Never in an app with
/// `game_parts::NoStandIns`.
#[allow(clippy::type_complexity)]
pub fn character_bodies(
    characters: Query<
        (
            Entity,
            &Team,
            Has<CapsuleBody>,
            (
                Option<&crate::map::BodyName>,
                Option<&crate::core::PlayerClass>,
                Option<&crate::slots::GameIdentity>,
            ),
        ),
        (With<Intent>, Without<LocalPlayer>),
    >,
    models: Option<Res<crate::map::CharacterModels>>,
    no_stand_ins: Option<Res<chimera_core::game_parts::NoStandIns>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut commands: Commands,
) {
    for (e, team, capsule, (name, class, identity)) in &characters {
        // A model for it: the map's, among its identity's game's when it
        // has one (another game's character on this map is a capsule).
        // An app without stand-ins (`NoStandIns`) never draws one: not on
        // a join's first frames, before the map's models are in, nor at a
        // map change.
        let modelled = no_stand_ins.is_some()
            || models
                .as_ref()
                .is_some_and(|m| identity.is_none() || m.for_identity(identity, Some(*team), name, class).is_some());
        match (modelled, capsule) {
            (true, true) => {
                commands
                    .entity(e)
                    .remove::<(CapsuleBody, Mesh3d, MeshMaterial3d<StandardMaterial>)>();
            }
            (false, false) => {
                let color = match team.0 {
                    0 => Color::srgb(0.35, 0.5, 0.9),
                    _ => Color::srgb(0.85, 0.4, 0.3),
                };
                let r = crate::character::CAPSULE_RADIUS;
                commands.entity(e).insert((
                    CapsuleBody,
                    Mesh3d(meshes.add(Capsule3d::new(r, crate::character::CAPSULE_HEIGHT - 2.0 * r))),
                    MeshMaterial3d(materials.add(StandardMaterial {
                        base_color: color,
                        perceptual_roughness: 0.8,
                        ..default()
                    })),
                ));
            }
            _ => {}
        }
    }
}

/// The dead aren't drawn, nor the one a spectator looks out of.
fn show_bodies(
    mut bodies: Query<(Entity, &mut Visibility, Has<Dead>), (With<Intent>, Without<LocalPlayer>)>,
    spectating: Option<Res<super::spectate::SpecView>>,
) {
    let in_eye = spectating.and_then(|s| s.in_eye);
    for (e, mut vis, dead) in &mut bodies {
        let want = if dead || in_eye == Some(e) {
            Visibility::Hidden
        } else {
            Visibility::Inherited
        };
        if *vis != want {
            *vis = want;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::console::Console;

    fn crosshair_after(line: &str) -> CrosshairColor {
        let mut app = App::new();
        app.add_plugins(crate::console::ConsolePlugin).init_resource::<CrosshairColor>();
        crosshair_cvars(&mut app);
        app.world_mut().resource_mut::<Console>().submit(line);
        app.update();
        *app.world().resource::<CrosshairColor>()
    }

    /// Capsules follow the map, not the characters' arrival: a character
    /// that came in on the greybox (no models) loses its capsule on a
    /// map with models, and gets one again back on the greybox.
    #[test]
    fn no_weapon_shows_the_default_crosshair() {
        let mut inv = Inventory::default();
        assert_eq!(crosshair_kind(Some(&inv), false, false), CrosshairKind::Default);
        assert_eq!(crosshair_kind(Some(&inv), true, false), CrosshairKind::Hidden);
        assert_eq!(crosshair_kind(None, false, false), CrosshairKind::Hidden);
        inv.active = Some(Entity::from_raw_u32(7).unwrap());
        assert_eq!(crosshair_kind(Some(&inv), false, false), CrosshairKind::Weapon);
        assert_eq!(crosshair_kind(Some(&inv), false, true), CrosshairKind::Hidden);
    }

    /// The scope overlay follows the HUD look present: a CS:S AWP's gets
    /// CS:S's scope textures (a look with `scope_arc`), a Combat Arms
    /// sniper's (the look put away for Combat Arms' HUD) the generic one,
    /// and CS:S's again when the look is back.
    #[test]
    fn scope_overlay_follows_the_hud_look() {
        use crate::map::hud::{ActiveHud, GameHud, HudSprite};
        use std::{collections::HashMap, sync::Arc};
        let mut app = App::new();
        app.init_resource::<Assets<Image>>();
        app.init_resource::<ScopeOverlayImage>();
        app.add_systems(Update, game_scope.run_if(scope_inputs_changed));
        app.world_mut().resource_scope(|w: &mut World, mut images: Mut<Assets<Image>>| {
            let mut commands = w.commands();
            spawn_scope(&mut commands, &mut images);
        });
        app.world_mut().flush();
        let look = || {
            let mut hud = GameHud::default();
            hud.sprites.insert("scope_arc".into(), HudSprite { texture: 0, rect: [0.0; 4] });
            ActiveHud(Arc::new(hud), HashMap::from([(0, Handle::default())]))
        };
        let state = |app: &mut App| {
            let w = app.world_mut();
            let (e, generic) = w.query::<(Entity, &GenericScope)>().single(w).map(|(e, g)| (e, g.0.clone())).unwrap();
            let kids = w.get::<Children>(e).map_or(0, |c| c.len());
            let on_generic = w.get::<ImageNode>(e).is_some_and(|i| i.image == generic);
            (kids, on_generic)
        };
        app.update();
        assert_eq!(state(&mut app), (0, true), "no look: generic");
        app.insert_resource(look());
        app.update();
        let (kids, on_generic) = state(&mut app);
        assert!(kids >= 4 && !on_generic, "CS:S's scope with its look");
        app.world_mut().remove_resource::<ActiveHud>();
        app.update();
        assert_eq!(state(&mut app), (0, true), "look put away: generic, not CS:S's");
        app.insert_resource(look());
        app.update();
        assert!(state(&mut app).0 >= 4, "CS:S's again");
        // A game's own picture comes first, then gives way again.
        let own = app.world_mut().resource_mut::<Assets<Image>>().add(Image::default());
        app.insert_resource(ScopeOverlayImage(Some(own.clone())));
        app.update();
        let w = app.world_mut();
        let (e, _) = w.query::<(Entity, &GenericScope)>().single(w).unwrap();
        assert!(w.get::<ImageNode>(e).is_some_and(|i| i.image == own), "the game's picture over CS:S's look");
        assert!(w.get::<Children>(e).is_none_or(|c| c.is_empty()));
        app.insert_resource(ScopeOverlayImage(None));
        app.update();
        assert!(state(&mut app).0 >= 4, "CS:S's again once the picture goes");
    }

    /// A game's picture with a texel size is a centred card in `vh` units
    /// with the bars gone; without the size, its aspect between the bars.
    #[test]
    fn scope_card_sizes_by_the_screens_height() {
        let mut app = App::new();
        app.init_resource::<Assets<Image>>();
        app.init_resource::<ScopeOverlayImage>().init_resource::<ScopeOverlayTexel>();
        app.add_systems(Update, scope_aspect);
        app.world_mut().resource_scope(|w: &mut World, mut images: Mut<Assets<Image>>| {
            let mut commands = w.commands();
            spawn_scope(&mut commands, &mut images);
        });
        app.world_mut().flush();
        use bevy::{asset::RenderAssetUsages, render::render_resource::{Extent3d, TextureDimension, TextureFormat}};
        let own = app.world_mut().resource_mut::<Assets<Image>>().add(Image::new_fill(
            Extent3d { width: 1024, height: 512, depth_or_array_layers: 1 },
            TextureDimension::D2,
            &[0, 0, 0, 0],
            TextureFormat::Rgba8UnormSrgb,
            RenderAssetUsages::RENDER_WORLD,
        ));
        app.insert_resource(ScopeOverlayImage(Some(own)));
        let state = |app: &mut App| {
            let w = app.world_mut();
            let sq = w.query_filtered::<&Node, With<ScopeSquare>>().single(w).unwrap().clone();
            let bars: Vec<Display> = w.query_filtered::<&Node, With<ScopeBar>>().iter(w).map(|n| n.display).collect();
            (sq, bars)
        };
        app.update();
        let (sq, bars) = state(&mut app);
        assert_eq!((sq.aspect_ratio, bars), (Some(2.0), vec![Display::Flex; 2]), "its aspect between bars");
        app.insert_resource(ScopeOverlayTexel(Some(0.002)));
        app.update();
        let (sq, bars) = state(&mut app);
        assert_eq!((sq.width, sq.height, sq.aspect_ratio), (vh(204.8), vh(102.4), None));
        assert_eq!(sq.position_type, PositionType::Absolute);
        assert_eq!((sq.left, sq.margin.left, sq.top, sq.margin.top), (percent(50.0), vh(-102.4), percent(50.0), vh(-51.2)));
        assert_eq!(bars, vec![Display::None; 2], "no black fill");
        app.insert_resource(ScopeOverlayTexel(None));
        app.update();
        assert_eq!(state(&mut app).0.aspect_ratio, Some(2.0));
    }

    #[test]
    fn capsule_bodies_follow_the_map() {
        use bevy::ecs::system::RunSystemOnce;
        let mut world = World::new();
        world.init_resource::<Assets<Mesh>>();
        world.init_resource::<Assets<StandardMaterial>>();
        let other = world.spawn((Intent::default(), Team(1))).id();
        let me = world.spawn((Intent::default(), Team(2), LocalPlayer)).id();
        let run = |world: &mut World| world.run_system_once(character_bodies).unwrap();
        run(&mut world);
        assert!(world.get::<CapsuleBody>(other).is_some() && world.get::<Mesh3d>(other).is_some());
        assert!(world.get::<CapsuleBody>(me).is_none(), "not the local player");
        world.insert_resource(crate::map::CharacterModels(Default::default()));
        run(&mut world);
        assert!(world.get::<CapsuleBody>(other).is_none() && world.get::<Mesh3d>(other).is_none());
        world.remove_resource::<crate::map::CharacterModels>();
        run(&mut world);
        assert!(world.get::<CapsuleBody>(other).is_some());
    }

    #[test]
    fn the_default_crosshair_is_ours_unchanged() {
        let c = CrosshairColor::default();
        let l = c.lines(0.0, 720.0);
        assert_eq!(l[0], (Vec2::new(MIN_GAP + LINE_LENGTH / 2.0, 0.0), Vec2::new(LINE_LENGTH, LINE_WIDTH)));
        assert_eq!(l[4].1, Vec2::ZERO, "no dot");
    }

    #[test]
    fn size_thickness_and_dot_change_the_drawn_crosshair() {
        let base = CrosshairColor::default().lines(0.0, 720.0);
        let big = crosshair_after("cl_crosshairsize 10; cl_crosshairthickness 1; cl_crosshairdot 1").lines(0.0, 720.0);
        assert_eq!(big[0].1, Vec2::new(LINE_LENGTH * 2.0, LINE_WIDTH * 2.0), "twice as long and wide");
        assert!(big[0].0.x > base[0].0.x, "the line's centre moves out with its length");
        assert_eq!(big[2].1, Vec2::new(LINE_WIDTH * 2.0, LINE_LENGTH * 2.0));
        assert_eq!(big[4], (Vec2::ZERO, Vec2::splat(LINE_WIDTH * 2.0)), "a centre dot as wide as the lines");
        let none = crosshair_after("cl_crosshairsize 0").lines(0.0, 720.0);
        assert_eq!(none[0].1.x, 0.0, "size 0: no lines");
        let thin = crosshair_after("cl_crosshairthickness 0").lines(0.0, 720.0);
        assert_eq!(thin[0].1.y, 1.0, "at least a pixel wide");
    }

    #[test]
    fn custom_colour_and_translucency() {
        let c = crosshair_after("cl_crosshaircolor 5; cl_crosshaircolor_r 255; cl_crosshaircolor_g 0; cl_crosshaircolor_b 128");
        let rgba = c.color().to_srgba();
        assert_eq!((rgba.red, rgba.green), (1.0, 0.0));
        assert!((rgba.blue - 128.0 / 255.0).abs() < 1e-4);
        let preset = crosshair_after("cl_crosshaircolor 1").color().to_srgba();
        assert_eq!(preset.red, 1.0, "red preset");
        let faint = crosshair_after("cl_crosshairusealpha 1; cl_crosshairalpha 51").color();
        assert!((faint.alpha() - 0.2).abs() < 1e-4);
    }
}

/// A HUD sprite sheet as CS:S draws it, additively: black is
/// nothing. Drawn alpha blended here, so each texel's brightness becomes
/// its coverage (colour divided by it); otherwise the sprite's black
/// surround shows as a dark box. `None` if the image isn't 8-bit RGBA.
pub fn additive_as_alpha(image: &Image) -> Option<Image> {
    use bevy::render::render_resource::TextureFormat;
    if !matches!(
        image.texture_descriptor.format,
        TextureFormat::Rgba8Unorm | TextureFormat::Rgba8UnormSrgb
    ) {
        return None;
    }
    let mut out = image.clone();
    let data = out.data.as_mut()?;
    for px in data.chunks_exact_mut(4) {
        let peak = px[0].max(px[1]).max(px[2]);
        if peak == 0 {
            px.copy_from_slice(&[0, 0, 0, 0]);
            continue;
        }
        for c in &mut px[..3] {
            *c = ((*c as u32 * 255 + peak as u32 / 2) / peak as u32) as u8;
        }
        px[3] = ((px[3] as u32 * peak as u32 + 127) / 255) as u8;
    }
    Some(out)
}

#[cfg(test)]
mod warm_tests {
    use std::time::Duration;

    use bevy::prelude::*;
    use chimera_core::{
        game_parts::{GameParts, PartsRegistry},
        gameplay::{Gameplay, HudWarm},
    };

    const fn parts(game: &'static str, hud: Option<&'static str>) -> GameParts {
        GameParts {
            game,
            guns: &[],
            knife: None,
            movement: None,
            characters: None,
            hud,
            maps: &[],
            announcer: None,
            match_won_sound: None,
            match_lost_sound: None,
            music: false,
            cues: None,
        }
    }
    static A: GameParts = parts("a", Some("a"));
    static B: GameParts = parts("b", Some("b"));
    static C: GameParts = parts("c", None);

    /// Behind the loading screen every game's HUD is the gameplay's for a few
    /// frames, the one playing last, and then it is handed back.
    #[test]
    fn each_hud_is_shown_then_the_playing_one_stays() {
        let mut world = World::new();
        let mut registry = PartsRegistry::default();
        for p in [&A, &B, &C] {
            registry.register(p);
        }
        world.insert_resource(registry);
        world.insert_resource(Gameplay {
            rules: "b",
            bots: "b",
            hud: "b",
            binds: "b",
            tick: Duration::from_millis(15),
        });
        let mut shown = Vec::new();
        let mut calls = 0;
        while !super::warm_huds(&mut world) {
            let hud = world.resource::<Gameplay>().hud;
            assert_eq!(world.get_resource::<HudWarm>().map(|w| w.0), Some(hud));
            if shown.last() != Some(&hud) {
                shown.push(hud);
            }
            calls += 1;
            assert!(calls < 100);
        }
        assert_eq!(shown, ["a", chimera_core::game_parts::MINIMAL_HUD, "b"]);
        assert!(world.get_resource::<HudWarm>().is_none(), "handed back");
        assert_eq!(world.resource::<Gameplay>().hud, "b");
    }
}
