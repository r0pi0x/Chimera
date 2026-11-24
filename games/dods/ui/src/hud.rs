//! DoD:S's HUD while a DoD:S weapon is in hand (specs/dods/hud.md): the
//! player status strip at the bottom left, laid out on the layout files'
//! 640 x 480 screen scaled to the window's height (`LAYOUT`): the main
//! background, the class silhouette that turns red as health is lost
//! with the health bar beside it, the held weapon's picture, the magazine
//! that empties shot by shot with the spare magazines' count, and the
//! sliced stamina bar under them, in the scheme's colours (`colours`). The
//! pictures are the install's (`dods::game::hud::pictures`, loaded by
//! `load_pictures`, which also registers DoD:S's kill icons for every
//! app's kill feed); until they are here (or without the install) the
//! weapon's name and its rounds are written instead. The crosshair is the
//! shell's (its gap from the spread); the kill feed is the app's.
//!
//! - `Frame`: what the HUD shows, from the local player's state (`frame`;
//!   tests read it without a window);
//! - the Bevy side: `spawn_root`, `collect` (the frame), `sync` (the
//!   nodes) and `load_pictures`.
//!
//! Known shortcuts:
//! - The panels' cut corners (`CornerToCut`) are square; the stamina icon
//!   (a glyph of the client's `ClientTitleFontPlayerIcons`, not in the
//!   server's files) isn't drawn; the "x N" font and size are ours.
//! - Ours where the spec leaves it open (hud.md, open questions): the
//!   silhouette's red part from the top down, drawn over the figure; the
//!   health bar in the scheme's three health colours; the magazine box
//!   by the cell's shape (taller than wide: the large one); the MG heat,
//!   fire select and team icon panels aren't drawn; no fades.

use std::collections::BTreeMap;

use bevy::{asset::RenderAssetUsages, prelude::*};
use chimera_client::client::{
    hud::{GameDraws, HudRoot},
    weapon_icons::{WeaponIcon, WeaponIcons},
};
use chimera_core::core::{Health, LocalPlayer};
use chimera_sim::weapon::{Inventory, Magazine, Weapon};
use chimera_world::map::hud::UiImage;

use dods::game::{
    GAME, INFO,
    classes::{Classes, RungClass},
    hud::ClipFill,
    stamina::DodsStamina,
};

/// What this HUD draws instead of the shell's: health and ammo (the
/// crosshair, hit marker and kill feed stay the shell's).
pub const DRAWS: GameDraws = GameDraws { crosshair: false, status: true, hit_marker: false, killfeed: false };

/// The layout (hud.md, "The player status strip"), in the virtual
/// 640 x 480 screen's units, absolute: `[x, y, w, h]`.
pub mod layout {
    /// The strip's top (`HudPlayerStatusPanel`'s y).
    pub const STRIP_Y: f32 = 290.0;
    pub const BACKGROUND: [f32; 4] = [16.0, STRIP_Y + 128.0, 240.0, 32.0];
    pub const HEALTH: [f32; 2] = [42.0, STRIP_Y + 91.0];
    /// Inside the health panel.
    pub const SILHOUETTE: [f32; 4] = [0.0, 5.0, 36.0, 72.0];
    pub const HEALTH_BAR: [f32; 4] = [36.0, 20.0, 8.0, 54.0];
    pub const LEFT_LINE: [f32; 4] = [88.0, STRIP_Y + 131.0, 1.0, 26.0];
    pub const RIGHT_LINE: [f32; 4] = [160.0, STRIP_Y + 131.0, 1.0, 26.0];
    pub const WEAPON: [f32; 4] = [90.0, STRIP_Y + 130.0, 67.0, 27.0];
    pub const AMMO: [f32; 2] = [168.0, STRIP_Y + 115.0];
    /// Inside the ammo panel.
    pub const LARGE_ICON: [f32; 4] = [0.0, 0.0, 32.0, 51.0];
    pub const SMALL_ICON: [f32; 4] = [0.0, 16.0, 32.0, 25.0];
    pub const EXTRA_CLIP: [f32; 4] = [39.0, 17.0, 22.0, 24.0];
    pub const CLIP_COUNT: [f32; 2] = [62.0, 23.0];
    pub const STAMINA: [f32; 2] = [10.0, STRIP_Y + 161.0];
    /// Inside the stamina panel.
    pub const STAMINA_BACKGROUND: [f32; 4] = [6.0, 7.0, 240.0, 14.0];
    pub const STAMINA_BAR: [f32; 4] = [31.0, 9.0, 210.0, 10.0];
    pub const SLICE: f32 = 2.0;
    pub const SLICE_GAP: f32 = 2.0;
    /// Stamina below this share draws in the low colours.
    pub const STAMINA_WARNING: f32 = 0.35;
    /// Health shares where the bar turns yellow, then red.
    pub const FIRST_WARNING: f32 = 0.50;
    pub const SECOND_WARNING: f32 = 0.25;
    /// The spare count's text height (ours).
    pub const COUNT_TALL: f32 = 12.0;

    /// The stamina bar's slices (53).
    pub fn slices() -> usize {
        ((STAMINA_BAR[2] + SLICE_GAP) / (SLICE + SLICE_GAP)).floor() as usize
    }

    /// The slices lit at stamina share `share` (0-1).
    pub fn lit(share: f32) -> usize {
        ((slices() as f32 * share.clamp(0.0, 1.0)) + 1e-4).floor() as usize
    }
}

/// The scheme's colours (hud.md, "Colours").
pub mod colours {
    use bevy::color::Color;

    const fn c(r: u8, g: u8, b: u8, a: u8) -> Color {
        Color::srgba(r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0, a as f32 / 255.0)
    }
    pub const BACKGROUND: Color = c(123, 110, 59, 184);
    pub const BORDER: Color = c(255, 255, 255, 102);
    pub const BAR_ACTIVE: Color = c(240, 207, 78, 255);
    pub const BAR_INACTIVE: Color = c(140, 120, 73, 255);
    pub const BAR_ACTIVE_LOW: Color = c(240, 30, 30, 255);
    pub const BAR_INACTIVE_LOW: Color = c(240, 30, 30, 99);
    pub const HEALTH_GREEN: Color = c(85, 112, 66, 255);
    pub const HEALTH_YELLOW: Color = c(240, 207, 78, 255);
    pub const HEALTH_RED: Color = c(180, 20, 20, 255);
    pub const HEALTH_BG: Color = c(170, 150, 73, 255);
    pub const COUNT: Color = c(240, 207, 78, 102);
    pub const COUNT_X: Color = c(255, 255, 255, 102);
    /// The written fallback's text (ours).
    pub const TEXT: Color = c(240, 207, 78, 255);
}

/// The health bar's colour at health share `share`.
pub fn health_colour(share: f32) -> Color {
    if share > layout::FIRST_WARNING {
        colours::HEALTH_GREEN
    } else if share > layout::SECOND_WARNING {
        colours::HEALTH_YELLOW
    } else {
        colours::HEALTH_RED
    }
}

/// What the HUD shows.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Frame {
    /// Health, 0-100 (rounded up), none without a local player.
    pub health: Option<u32>,
    /// Stamina, 0-1 (none: not a DoD:S player).
    pub stamina: Option<f32>,
    /// The weapon in hand's id.
    pub weapon_id: String,
    /// The weapon in hand's name.
    pub weapon: String,
    /// Its clip and reserve, when it has a magazine.
    pub ammo: Option<(u32, u32)>,
    /// The holder's class (`dods:us_rifleman`), from the weapon and the
    /// rung's drawn class.
    pub class: Option<String>,
}

/// The frame from the local player's health, stamina and weapon (its id
/// and, with a magazine, its clip and reserve; `name`: its display name,
/// else its id) and class.
pub fn frame(
    health: Option<&Health>,
    stamina: Option<&DodsStamina>,
    weapon: Option<(&str, Option<(u32, u32)>)>,
    class: Option<&str>,
    name: impl Fn(&str) -> Option<String>,
) -> Frame {
    Frame {
        health: health.map(|h| (h.current * 100.0).ceil().max(0.0) as u32),
        stamina: stamina.map(|s| (s.value / dods::game::stamina::MAX).clamp(0.0, 1.0)),
        weapon_id: weapon.map(|(id, _)| id.to_string()).unwrap_or_default(),
        weapon: weapon.map(|(id, _)| name(id).unwrap_or_else(|| id.to_string())).unwrap_or_default(),
        ammo: weapon.and_then(|(_, m)| m),
        class: class.map(str::to_string),
    }
}

/// The spare magazines shown for `reserve` rounds of a `clip_size` clip
/// (rounded up; the rounds themselves with no clip size).
pub fn spare_magazines(reserve: u32, clip_size: u32) -> u32 {
    if clip_size == 0 { reserve } else { reserve.div_ceil(clip_size) }
}

/// The HUD's last frame.
#[derive(Resource, Default)]
pub struct DodsHudState {
    pub frame: Frame,
}

/// The weapons' display names (their data's `hud.name`).
#[derive(Resource)]
pub struct Names(pub Vec<(String, String)>);

impl Default for Names {
    fn default() -> Self {
        Names(dods::game::weapons::dods_weapons().weapons.into_iter().filter_map(|w| Some((w.id, w.hud.name?))).collect())
    }
}

/// A picture as an image and its size in texels.
pub type Picture = (Handle<Image>, Vec2);

/// A weapon's pictures as images (`dods::game::hud::WeaponPictures`).
#[derive(Clone, Debug, Default)]
pub struct WeaponImages {
    pub weapon: Option<Picture>,
    pub glow: Option<Picture>,
    pub ammo: Option<Picture>,
    pub ammo_empty: Option<Picture>,
    pub clip: Option<ClipFill>,
    pub clip_size: u32,
}

/// The install's pictures as images, once loaded (`load_pictures`).
#[derive(Resource, Default)]
pub struct HudPictures {
    pub weapons: BTreeMap<String, WeaponImages>,
    /// Each class's silhouette and outline.
    pub classes: BTreeMap<String, (Option<Picture>, Option<Picture>)>,
}

/// When `load_pictures` looks again (`Local`): loaded, or the next try
/// and the wait after it.
#[derive(Default)]
pub struct Tries {
    done: bool,
    next: f32,
    wait: f32,
}

/// The longest wait between two tries (s).
const MOST_WAIT: f32 = 30.0;

fn image(images: &mut Assets<Image>, p: UiImage) -> Picture {
    let size = Vec2::new(p.width as f32, p.height as f32);
    let image = Image::new(
        bevy::render::render_resource::Extent3d { width: p.width, height: p.height, depth_or_array_layers: 1 },
        bevy::render::render_resource::TextureDimension::D2,
        p.rgba8,
        bevy::render::render_resource::TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::default(),
    );
    (images.add(image), size)
}

/// DoD:S's pictures from its mount (the install, or the asset store's
/// `players` set), tried at startup and again further apart (2 s doubling
/// up to 30 s) until their files are here, never fetching: the HUD's
/// (`HudPictures`) and the kill icons, registered for every app's kill
/// feed (`WeaponIcons`; until then the feed writes the gun's name).
pub fn load_pictures(
    mut icons: ResMut<WeaponIcons>,
    mut pictures: ResMut<HudPictures>,
    images: Option<ResMut<Assets<Image>>>,
    time: Option<Res<Time<Real>>>,
    mut tries: Local<Tries>,
) {
    let now = time.map_or(0.0, |t| t.elapsed_secs());
    if tries.done || now < tries.next {
        return;
    }
    let Some(mut images) = images else { return };
    tries.wait = (tries.wait * 2.0).clamp(2.0, MOST_WAIT);
    tries.next = now + tries.wait;
    let Some(mount) = INFO.mount() else { return };
    let Some(read) = dods::game::hud::pictures(&mount) else { return };
    tries.done = true;
    let kill: BTreeMap<String, Picture> = read.kill_icons.into_iter().map(|(id, p)| (id, image(&mut images, p))).collect();
    info!("DoD:S: {} kill icons, {} weapons' and {} classes' HUD pictures", kill.len(), read.weapons.len(), read.classes.len());
    if !kill.is_empty() {
        icons.register(GAME, move |id| kill.get(id).map(|(image, size)| WeaponIcon::Image { image: image.clone(), size: *size }));
    }
    let mut to = |p: Option<UiImage>| p.map(|p| image(&mut images, p));
    pictures.weapons = read
        .weapons
        .into_iter()
        .map(|(id, w)| {
            let images = WeaponImages {
                weapon: to(w.weapon),
                glow: to(w.glow),
                ammo: to(w.ammo),
                ammo_empty: to(w.ammo_empty),
                clip: w.clip,
                clip_size: w.clip_size,
            };
            (id, images)
        })
        .collect();
    pictures.classes = read.classes.into_iter().map(|(id, c)| (id, (to(c.image), to(c.outline)))).collect();
}

/// A length in the layout's units as a share of the window's height.
fn u(v: f32) -> Val {
    Val::Vh(v * 100.0 / 480.0)
}

/// An absolute node at `[x, y, w, h]` of its parent.
fn place([x, y, w, h]: [f32; 4]) -> Node {
    Node { position_type: PositionType::Absolute, left: u(x), top: u(y), width: u(w), height: u(h), ..default() }
}

/// `[x, y, w, h]` of a `size` picture fitted into `bx` keeping its aspect,
/// centred.
pub fn fit(bx: [f32; 4], size: Vec2) -> [f32; 4] {
    if size.x <= 0.0 || size.y <= 0.0 {
        return bx;
    }
    let s = (bx[2] / size.x).min(bx[3] / size.y);
    let (w, h) = (size.x * s, size.y * s);
    [bx[0] + (bx[2] - w) / 2.0, bx[1] + (bx[3] - h) / 2.0, w, h]
}

/// The HUD's parts, by what `sync` sets on them.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub enum Part {
    Silhouette,
    SilhouetteOutline,
    SilhouetteRed,
    HealthFill,
    WeaponPicture,
    WeaponGlow,
    /// The written weapon name and rounds (no pictures).
    WeaponText,
    MagazineEmpty,
    MagazineFull,
    ExtraClip,
    CountX,
    Count,
    Slice(usize),
}

/// The root and its panels, hidden until DoD:S's HUD is the gameplay's.
pub fn spawn_root(mut commands: Commands) {
    let text = |tall: f32, colour: Color| {
        (Text::default(), TextFont { font_size: FontSize::Vh(tall * 100.0 / 480.0), ..default() }, TextColor(colour))
    };
    let picture = |part: Part, bx: [f32; 4]| (part, ImageNode::default(), place(bx), Visibility::Hidden);
    commands
        .spawn((
            HudRoot(GAME),
            Visibility::Hidden,
            GlobalZIndex(40),
            Node { position_type: PositionType::Absolute, width: percent(100.0), height: percent(100.0), ..default() },
        ))
        .with_children(|root| {
            root.spawn((BackgroundColor(colours::BACKGROUND), place(layout::BACKGROUND)));
            root.spawn((BackgroundColor(colours::BORDER), place(layout::LEFT_LINE)));
            root.spawn((BackgroundColor(colours::BORDER), place(layout::RIGHT_LINE)));
            let [hx, hy] = layout::HEALTH;
            root.spawn(place([hx, hy, 46.0, 78.0])).with_children(|p| {
                p.spawn(picture(Part::SilhouetteOutline, layout::SILHOUETTE));
                p.spawn(picture(Part::Silhouette, layout::SILHOUETTE));
                p.spawn(picture(Part::SilhouetteRed, layout::SILHOUETTE));
                p.spawn((BackgroundColor(colours::HEALTH_BG), place(layout::HEALTH_BAR))).with_children(|b| {
                    b.spawn((
                        Part::HealthFill,
                        BackgroundColor(colours::HEALTH_GREEN),
                        Node { position_type: PositionType::Absolute, left: px(0.0), bottom: px(0.0), width: percent(100.0), height: percent(100.0), ..default() },
                    ));
                });
            });
            root.spawn(picture(Part::WeaponGlow, layout::WEAPON));
            root.spawn(picture(Part::WeaponPicture, layout::WEAPON));
            root.spawn((
                Part::WeaponText,
                text(10.0, colours::TEXT),
                Node { position_type: PositionType::Absolute, left: u(layout::WEAPON[0]), top: u(layout::WEAPON[1]), ..default() },
            ));
            let [ax, ay] = layout::AMMO;
            root.spawn(place([ax, ay, 84.0, 52.0])).with_children(|p| {
                p.spawn(picture(Part::MagazineEmpty, layout::LARGE_ICON));
                p.spawn(picture(Part::MagazineFull, layout::LARGE_ICON));
                p.spawn(picture(Part::ExtraClip, layout::EXTRA_CLIP));
                let [cx, cy] = layout::CLIP_COUNT;
                p.spawn(Node {
                    position_type: PositionType::Absolute,
                    left: u(cx),
                    top: u(cy),
                    column_gap: u(2.0),
                    align_items: AlignItems::Baseline,
                    ..default()
                })
                .with_children(|row| {
                    row.spawn((Part::CountX, text(layout::COUNT_TALL * 0.75, colours::COUNT_X)));
                    row.spawn((Part::Count, text(layout::COUNT_TALL, colours::COUNT)));
                });
            });
            let [sx, sy] = layout::STAMINA;
            root.spawn(place([sx, sy, 266.0, 32.0])).with_children(|p| {
                p.spawn((BackgroundColor(colours::BACKGROUND), place(layout::STAMINA_BACKGROUND)));
                let [bx, by, _, bh] = layout::STAMINA_BAR;
                for k in 0..layout::slices() {
                    let x = bx + k as f32 * (layout::SLICE + layout::SLICE_GAP);
                    p.spawn((Part::Slice(k), BackgroundColor(colours::BAR_INACTIVE), place([x, by, layout::SLICE, bh])));
                }
            });
        });
}

/// The local player's state into the frame.
#[allow(clippy::type_complexity)]
pub fn collect(
    player: Option<Single<(&Health, Option<&DodsStamina>, Option<&Inventory>, Option<&RungClass>), With<LocalPlayer>>>,
    weapons: Query<(&Weapon, Option<&Magazine>)>,
    names: Res<Names>,
    classes: Option<Res<Classes>>,
    mut state: ResMut<DodsHudState>,
) {
    let f = match player {
        Some(p) => {
            let (health, stamina, inv, drawn) = *p;
            let held = inv
                .and_then(|i| i.active)
                .and_then(|w| weapons.get(w).ok())
                .map(|(w, m)| (w.id.as_ref(), m.map(|m| (m.clip, m.reserve))));
            let fallback;
            let table = match &classes {
                Some(c) => &**c,
                None => {
                    fallback = dods::game::classes::classes();
                    &fallback
                }
            };
            let drawn = drawn.map(|d| d.0.as_str()).filter(|d| !d.is_empty());
            let class = held.and_then(|(id, _)| table.held(id, drawn)).map(|c| c.id.as_str());
            frame(Some(health), stamina, held, class, |id| names.0.iter().find(|(i, _)| i == id).map(|(_, n)| n.clone()))
        }
        None => Frame::default(),
    };
    if state.frame != f {
        state.frame = f;
    }
}

/// The frame and the pictures into the nodes (only when either changed).
#[allow(clippy::type_complexity)]
pub fn sync(
    state: Res<DodsHudState>,
    pictures: Res<HudPictures>,
    mut images: Query<(&Part, &mut ImageNode, &mut Node, &mut Visibility), Without<Text>>,
    mut fills: Query<(&Part, &mut Node, &mut BackgroundColor), (Without<ImageNode>, Without<Text>)>,
    mut texts: Query<(&Part, &mut Text, &mut Visibility), Without<ImageNode>>,
) {
    if !state.is_changed() && !pictures.is_changed() {
        return;
    }
    let f = &state.frame;
    let health = f.health.unwrap_or(0) as f32 / 100.0;
    let weapon = pictures.weapons.get(&f.weapon_id);
    let class = f.class.as_ref().and_then(|c| pictures.classes.get(c));
    let (clip, reserve) = f.ammo.unwrap_or((0, 0));
    // The magazine's box by the full picture's shape.
    let magazine_box = weapon.and_then(|w| w.ammo.as_ref()).map(|(_, s)| {
        let bx = if s.y > s.x { layout::LARGE_ICON } else { layout::SMALL_ICON };
        fit(bx, *s)
    });
    for (part, mut img, mut node, mut vis) in &mut images {
        let mut show = |p: Option<&Picture>, bx: [f32; 4], rect: Option<Rect>, colour: Color| -> Visibility {
            let Some((handle, _)) = p else { return Visibility::Hidden };
            if img.image != *handle {
                img.image = handle.clone();
            }
            img.rect = rect;
            img.color = colour;
            let [x, y, w, h] = bx;
            node.left = u(x);
            node.top = u(y);
            node.width = u(w);
            node.height = u(h);
            Visibility::Inherited
        };
        let want = match part {
            Part::Silhouette => show(class.and_then(|c| c.0.as_ref()), layout::SILHOUETTE, None, Color::WHITE),
            Part::SilhouetteOutline => show(class.and_then(|c| c.1.as_ref()), layout::SILHOUETTE, None, Color::WHITE),
            Part::SilhouetteRed => {
                // The lost share, from the top down (ours: hud.md, open
                // question 1).
                let lost = 1.0 - health.clamp(0.0, 1.0);
                let p = class.and_then(|c| c.0.as_ref());
                let [x, y, w, h] = layout::SILHOUETTE;
                match p {
                    Some((_, size)) if lost > 0.0 => {
                        show(p, [x, y, w, h * lost], Some(Rect::new(0.0, 0.0, size.x, size.y * lost)), colours::HEALTH_RED)
                    }
                    _ => Visibility::Hidden,
                }
            }
            Part::WeaponPicture => {
                let p = weapon.and_then(|w| w.weapon.as_ref());
                show(p, fit(layout::WEAPON, p.map_or(Vec2::ONE, |p| p.1)), None, Color::WHITE)
            }
            Part::WeaponGlow => {
                let p = weapon.and_then(|w| w.glow.as_ref());
                show(p, fit(layout::WEAPON, p.map_or(Vec2::ONE, |p| p.1)), None, Color::WHITE)
            }
            Part::MagazineEmpty => match (magazine_box, weapon.and_then(|w| w.ammo_empty.as_ref())) {
                (Some(bx), p) if f.ammo.is_some() => show(p, bx, None, Color::WHITE),
                _ => Visibility::Hidden,
            },
            Part::MagazineFull => match (magazine_box, weapon) {
                (Some(bx), Some(w)) if f.ammo.is_some() => {
                    let p = w.ammo.as_ref();
                    let shown = w.clip.map_or(if clip > 0 { 1.0 } else { 0.0 }, |c| c.shown(clip));
                    let size = p.map_or(Vec2::ONE, |p| p.1);
                    if shown <= 0.0 {
                        Visibility::Hidden
                    } else {
                        let [x, y, bw, bh] = bx;
                        let h = bh * shown;
                        show(p, [x, y + bh - h, bw, h], Some(Rect::new(0.0, size.y * (1.0 - shown), size.x, size.y)), Color::WHITE)
                    }
                }
                _ => Visibility::Hidden,
            },
            Part::ExtraClip => {
                let p = weapon.and_then(|w| w.ammo.as_ref()).filter(|_| f.ammo.is_some());
                show(p, fit(layout::EXTRA_CLIP, p.map_or(Vec2::ONE, |p| p.1)), None, Color::WHITE)
            }
            _ => continue,
        };
        vis.set_if_neq(want);
    }
    for (part, mut node, mut colour) in &mut fills {
        match part {
            Part::HealthFill => {
                node.height = percent(health.clamp(0.0, 1.0) * 100.0);
                colour.set_if_neq(BackgroundColor(health_colour(health)));
            }
            Part::Slice(k) => {
                let share = f.stamina.unwrap_or(0.0);
                let lit = *k < layout::lit(share);
                let low = share < layout::STAMINA_WARNING;
                let want = match (lit, low) {
                    (true, false) => colours::BAR_ACTIVE,
                    (false, false) => colours::BAR_INACTIVE,
                    (true, true) => colours::BAR_ACTIVE_LOW,
                    (false, true) => colours::BAR_INACTIVE_LOW,
                };
                colour.set_if_neq(BackgroundColor(want));
            }
            _ => {}
        }
    }
    let pictured = weapon.is_some_and(|w| w.weapon.is_some());
    for (part, mut text, mut vis) in &mut texts {
        let want = match part {
            Part::WeaponText if !pictured => match f.ammo {
                Some((c, r)) => format!("{}  {c} | {r}", f.weapon),
                None => f.weapon.clone(),
            },
            Part::CountX if pictured && f.ammo.is_some() => "x".to_string(),
            Part::Count if pictured => match f.ammo {
                Some(_) => spare_magazines(reserve, weapon.map_or(0, |w| w.clip_size)).to_string(),
                None => String::new(),
            },
            _ => String::new(),
        };
        vis.set_if_neq(if want.is_empty() { Visibility::Hidden } else { Visibility::Inherited });
        if text.0 != want {
            text.0 = want;
        }
    }
}

/// DoD:S's HUD: its state, root and nodes, drawing while its HUD is the
/// gameplay's; its pictures and kill icons loaded once here.
pub struct DodsHudPlugin;

impl Plugin for DodsHudPlugin {
    fn build(&self, app: &mut App) {
        app.world_mut().get_resource_or_init::<chimera_client::client::hud::HudDraws>().0.push((GAME, DRAWS));
        // Its kill feed is the app's: it shows none of its own.
        chimera_client::client::kill_feed::GameKillFeedAppExt::register_game_kill_feed(app, GAME, |_| 0);
        app.init_resource::<DodsHudState>()
            .init_resource::<Names>()
            .init_resource::<HudPictures>()
            .init_resource::<WeaponIcons>()
            .add_systems(Startup, spawn_root)
            .add_systems(Update, ((collect, sync).chain().run_if(chimera_core::gameplay::hud_is(GAME)), load_pictures));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_frame_reads_health_stamina_rounds_and_class() {
        let f = frame(
            Some(&Health { current: 0.734, max: 1.0 }),
            Some(&DodsStamina { value: 40.0, sprinting: false }),
            Some(("dods:weapon_garand", Some((8, 80)))),
            Some("dods:us_rifleman"),
            |_| Some("M1 Garand".into()),
        );
        assert_eq!(
            f,
            Frame {
                health: Some(74),
                stamina: Some(0.4),
                weapon_id: "dods:weapon_garand".into(),
                weapon: "M1 Garand".into(),
                ammo: Some((8, 80)),
                class: Some("dods:us_rifleman".into()),
            }
        );
        assert_eq!(frame(None, None, None, None, |_| None), Frame::default());
    }

    /// hud.md test cases 3 to 6.
    #[test]
    fn the_spec_cases_hold() {
        assert_eq!(spare_magazines(210, 30), 7);
        assert_eq!(spare_magazines(1, 30), 1);
        assert_eq!(spare_magazines(0, 30), 0);
        assert_eq!(health_colour(1.0), colours::HEALTH_GREEN);
        assert_eq!(health_colour(0.5), colours::HEALTH_YELLOW);
        assert_eq!(health_colour(0.2), colours::HEALTH_RED);
        assert_eq!(layout::slices(), 53);
        assert_eq!(layout::lit(1.0), 53);
        assert_eq!(layout::lit(0.5), 26);
        assert_eq!(layout::lit(0.3), 15);
        let [x, y, w, h] = layout::BACKGROUND;
        assert_eq!((x, x + w, y, y + h), (16.0, 256.0, 418.0, 450.0));
        let [sx, sy] = layout::STAMINA;
        let [bx, by, bw, bh] = layout::STAMINA_BAR;
        assert_eq!((sx + bx, sx + bx + bw, sy + by, sy + by + bh), (41.0, 251.0, 460.0, 470.0));
    }

    #[test]
    fn a_picture_fits_its_box_keeping_its_shape() {
        assert!((fit([0.0, 0.0, 67.0, 27.0], Vec2::new(350.0, 122.0))[3] - 67.0 * 122.0 / 350.0).abs() < 1e-4);
        let f = fit([0.0, 0.0, 32.0, 51.0], Vec2::new(36.0, 184.0));
        assert_eq!(f[3], 51.0);
        assert!((f[0] - (32.0 - 36.0 * 51.0 / 184.0) / 2.0).abs() < 1e-4);
    }
}
