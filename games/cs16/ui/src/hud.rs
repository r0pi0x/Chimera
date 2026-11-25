//! Counter-Strike 1.6's HUD while a CS 1.6 weapon is in hand
//! (specs/cs16/hud.md): health with the cross and its bar at the bottom
//! left, armour with the suit icon lit from the bottom by the armour left
//! (the helmet pair with a helmet), the ammunition at the bottom right
//! (clip, bar, reserve and the ammunition icon; a grenade's count alone),
//! the numbers in the number sprites, dim at rest and flaring on a change
//! (`Fades`), in the HUD colour (health in the warning colour at 25 or
//! less), and the damage direction arcs (`Pain`). Everything sits at its
//! sprites' own pixel size by the spec's formulas in the window's real
//! pixels (no scaling: parity). The sprites are the install's grey
//! additive atlases (`cs16::game::hud`), tinted and added onto the frame
//! (`HudGlyphMaterial`, additive). `load_pictures` also registers the
//! `d_` kill icons and the headshot mark for every app's kill feed.
//!
//! - `Frame`: what the HUD shows, from the local player's state;
//!   `draws`: the frame laid out as sprites and filled boxes in screen
//!   pixels (pure; tests read both without a window);
//! - the Bevy side: `spawn_root`, `collect` (the frame, the fades),
//!   `pain` (the arcs' strengths), `sync` (a pool of nodes from the
//!   draws) and `load_pictures`.
//!
//! Known shortcuts:
//! - The crosshair is the shell's (four lines, its gap from the weapon's
//!   spread, `cl_crosshaircolor` and friends, default green), not CS 1.6's
//!   own dynamic crosshair (hud.md leaves its numbers open, question 3).
//! - The kill feed is the app's, with CS 1.6's `d_` icons tinted 255 80 0
//!   and blended by their brightness (not added), `d_skull` for a CS 1.6
//!   weapon without an icon; its layout and names are the app's.
//! - Not drawn: the weapon menu (bucket list; the shell's weapon menu
//!   shows), money (unused in the party), the round timer, the flashbang
//!   fade's own (the shell's flash white is drawn), the damage type icons.
//! - The 640 pictures at every width (no 320 layout below 640 wide).
//! - Ours where the spec leaves it open: armour's lit rows truncated
//!   (24 x armour / 100); a hit from within 50 units needs the attacker's
//!   position (a network client, which has no attacker, lights by the
//!   direction alone); undirected damage (a fall) lights no arc; health
//!   and armour hidden while dead or spectating (the whole HUD then).

use std::collections::BTreeMap;

use bevy::{asset::RenderAssetUsages, prelude::*};
use chimera_client::client::{
    hud::{GAME_HUD_Z, GameDraws, HudRoot},
    hud_text::{GlyphParams, HudGlyphMaterial},
    weapon_icons::{WeaponIcon, WeaponIcons},
};
use chimera_core::core::{DamageTaken, Dead, Health, Intent, LocalPlayer, Spectating};
use chimera_sim::weapon::{Armor, Inventory, Magazine, Weapon, grenade::Throwable};
use chimera_world::map::hud::UiImage;

use cs16::game::{GAME, INFO, METERS_PER_UNIT};

/// What this HUD draws instead of the shell's: health, armour and ammo
/// (the crosshair, hit marker and kill feed stay the shell's and the
/// app's).
pub const DRAWS: GameDraws = GameDraws { crosshair: false, status: true, hit_marker: false, killfeed: false };

/// hud.md's constants.
pub mod consts {
    /// Every number and icon's tint.
    pub const HUD_COLOUR: [u8; 3] = [255, 160, 0];
    /// Health and its cross at `WARNING_HEALTH` or less.
    pub const WARNING_COLOUR: [u8; 3] = [250, 0, 0];
    pub const WARNING_HEALTH: u32 = 25;
    /// Health at or below this is drawn at full alpha.
    pub const BRIGHT_HEALTH: u32 = 15;
    /// A number at rest.
    pub const DIM: f32 = 100.0;
    /// A fade's start (health, armour) and how fast fades drain (a second).
    pub const FADE: f32 = 100.0;
    pub const DRAIN: f32 = 20.0;
    /// Alpha added to the dim alpha at a fade's start.
    pub const FLARE: f32 = 128.0;
    /// The ammunition's fade on each update.
    pub const AMMO_FLARE: f32 = 200.0;
    /// The `number_0` cell at 640.
    pub const FONT_HEIGHT: i32 = 25;
    pub const DIGIT_WIDTH: i32 = 20;
    /// The cross's and suit icons' size when their pictures aren't here.
    pub const ICON: i32 = 24;
    /// The kill feed's icon tint.
    pub const KILL_ICON_COLOUR: [u8; 3] = [255, 80, 0];
    /// Pain arcs: drawn above this strength, which drains this much a
    /// second; within this many units of the source, all four.
    pub const PAIN_SHOWN: f32 = 0.4;
    pub const PAIN_DRAIN: f32 = 2.0;
    pub const PAIN_NEAR: f32 = 50.0;
    pub const PAIN_SIDE: f32 = 0.3;
}

use consts::*;

/// The ammunition row's content.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Ammo {
    /// No ammunition type (the knife): no row.
    #[default]
    None,
    /// A magazine's clip and reserve.
    Magazine { clip: u32, reserve: u32 },
    /// What is carried (the grenades).
    Count(u32),
}

/// What the HUD shows.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Frame {
    /// Health (0-100, rounded up); none: no local player, dead or
    /// spectating (nothing but the pain arcs drawn).
    pub health: Option<u32>,
    /// Armour (0-100) and whether a helmet is worn.
    pub armour: (u32, bool),
    /// The weapon in hand's id.
    pub weapon_id: String,
    pub ammo: Ammo,
}

/// The numbers' fades (fade units) and what they last showed.
#[derive(Resource, Clone, Debug, Default, PartialEq)]
pub struct Fades {
    pub health: f32,
    pub armour: f32,
    pub ammo: f32,
    last: Option<(Option<u32>, (u32, bool), String, Ammo)>,
}

impl Fades {
    /// Drain the fades by `dt` seconds, then restart those whose value
    /// changed in `f` (the first frame seen restarts none).
    pub fn step(&mut self, f: &Frame, dt: f32) {
        for v in [&mut self.health, &mut self.armour, &mut self.ammo] {
            *v = (*v - DRAIN * dt).max(0.0);
        }
        let now = (f.health, f.armour, f.weapon_id.clone(), f.ammo);
        if let Some((health, armour, weapon, ammo)) = &self.last {
            if *health != f.health {
                self.health = FADE;
            }
            if armour.0 != f.armour.0 {
                self.armour = FADE;
            }
            if *weapon != f.weapon_id || *ammo != f.ammo {
                self.ammo = AMMO_FLARE;
            }
        }
        self.last = Some(now);
    }
}

/// Health's and armour's alpha at fade `fade` (hud.md, "Health").
pub fn number_alpha(fade: f32) -> u8 {
    if fade > 0.0 { (DIM + fade / FADE * FLARE).min(255.0) as u8 } else { DIM as u8 }
}

/// Health's alpha: `number_alpha`, full at 15 or less.
pub fn health_alpha(health: u32, fade: f32) -> u8 {
    if health <= BRIGHT_HEALTH { 255 } else { number_alpha(fade) }
}

/// The ammunition's alpha: the larger of the dim alpha and the fade.
pub fn ammo_alpha(fade: f32) -> u8 {
    fade.max(DIM).min(255.0) as u8
}

/// Health's colour.
pub fn health_colour(health: u32) -> [u8; 3] {
    if health <= WARNING_HEALTH { WARNING_COLOUR } else { HUD_COLOUR }
}

/// One thing drawn, in screen pixels (origin top left).
#[derive(Clone, Debug, PartialEq)]
pub enum Draw {
    /// Picture `pic` (`HudPictures`' key) at (`x`, `y`), `src` its part
    /// drawn (`[x, y, w, h]` in its texels; the whole when none), tinted
    /// and added.
    Sprite { pic: String, x: i32, y: i32, src: Option<[u32; 4]>, colour: [u8; 3], alpha: u8 },
    /// A filled box `[x, y, w, h]`, blended.
    Fill { rect: [i32; 4], colour: [u8; 3], alpha: u8 },
}

/// The key of weapon `id`'s ammunition icon in `HudPictures`.
pub fn ammo_key(id: &str) -> String {
    format!("ammo:{id}")
}

/// The key of pain arc `k` (0 front, 1 right, 2 behind, 3 left).
pub fn pain_key(k: usize) -> String {
    format!("pain_{k}")
}

/// The digits of `n` in three places from `x` (hud.md, "Numbers"): no
/// leading zeros, zero as one `0` in the last place, longer numbers
/// growing to the right.
pub fn digits(n: u32, x: i32) -> Vec<(u32, i32)> {
    let s = n.to_string();
    let first = x + (3 - s.len() as i32).max(0) * DIGIT_WIDTH;
    s.bytes().enumerate().map(|(k, b)| ((b - b'0') as u32, first + k as i32 * DIGIT_WIDTH)).collect()
}

fn number(out: &mut Vec<Draw>, n: u32, x: i32, y: i32, colour: [u8; 3], alpha: u8) -> i32 {
    for (d, dx) in digits(n, x) {
        out.push(Draw::Sprite { pic: format!("number_{d}"), x: dx, y, src: None, colour, alpha });
    }
    x + (n.to_string().len() as i32).max(3) * DIGIT_WIDTH
}

/// The frame laid out on a `w` x `h` screen (module doc; hud.md's
/// formulas), `size`: a picture's size if it is here; `pain`: the arcs'
/// strengths.
pub fn draws(f: &Frame, fades: &Fades, pain: &[f32; 4], w: i32, h: i32, size: impl Fn(&str) -> Option<UVec2>) -> Vec<Draw> {
    let mut out = Vec::new();
    let health_now = f.health.unwrap_or(100);
    // The pain arcs (hud.md, "Damage direction").
    for (k, s) in pain.iter().enumerate() {
        if *s <= PAIN_SHOWN {
            continue;
        }
        let key = pain_key(k);
        let Some(sz) = size(&key) else { continue };
        let (fw, fh) = (sz.x as i32, sz.y as i32);
        let (x, y) = match k {
            0 => (w / 2 - fw / 2, h / 2 - 3 * fh),
            1 => (w / 2 + 2 * fw, h / 2 - fh / 2),
            2 => (w / 2 - fw / 2, h / 2 + 2 * fh),
            _ => (w / 2 - 3 * fw, h / 2 - fh / 2),
        };
        let alpha = (s.max(0.5) * 255.0).min(255.0) as u8;
        out.push(Draw::Sprite { pic: key, x, y, src: None, colour: health_colour(health_now), alpha });
    }
    let Some(health) = f.health else { return out };
    let y0 = h - FONT_HEIGHT - FONT_HEIGHT / 2;
    let row = y0 + FONT_HEIGHT / 5;
    // Health.
    let cross = size("cross").map_or(ICON, |s| s.x as i32);
    let colour = health_colour(health);
    let alpha = health_alpha(health, fades.health);
    out.push(Draw::Sprite { pic: "cross".into(), x: cross / 2, y: y0, src: None, colour, alpha });
    let end = number(&mut out, health, cross + DIGIT_WIDTH / 2, row, colour, alpha);
    out.push(Draw::Fill { rect: [end + DIGIT_WIDTH / 2, row, DIGIT_WIDTH / 10, FONT_HEIGHT], colour: HUD_COLOUR, alpha });
    // Armour.
    let (armour, helmet) = f.armour;
    let (empty, full) = if helmet { ("suithelmet_empty", "suithelmet_full") } else { ("suit_empty", "suit_full") };
    let icon = size(empty).map_or(UVec2::splat(ICON as u32), |s| s);
    let (ix, iy) = (w / 5, y0 - icon.y as i32 / 6);
    let alpha = number_alpha(fades.armour);
    out.push(Draw::Sprite { pic: empty.into(), x: ix, y: iy, src: None, colour: HUD_COLOUR, alpha });
    let lit = icon.y * armour.min(100) / 100;
    if lit > 0 {
        let top = icon.y - lit;
        out.push(Draw::Sprite { pic: full.into(), x: ix, y: iy + top as i32, src: Some([0, top, icon.x, lit]), colour: HUD_COLOUR, alpha });
    }
    number(&mut out, armour, ix + icon.x as i32, row, HUD_COLOUR, alpha);
    // Ammunition.
    let key = ammo_key(&f.weapon_id);
    let icon = size(&key).map_or(UVec2::splat(ICON as u32), |s| s);
    let alpha = ammo_alpha(fades.ammo);
    let icon_x = match f.ammo {
        Ammo::None => None,
        Ammo::Magazine { clip, reserve } => {
            let x = w - 8 * DIGIT_WIDTH - icon.x as i32;
            let bar = number(&mut out, clip, x, row, HUD_COLOUR, alpha) + DIGIT_WIDTH / 2;
            out.push(Draw::Fill { rect: [bar, row, DIGIT_WIDTH / 10, FONT_HEIGHT], colour: HUD_COLOUR, alpha });
            Some(number(&mut out, reserve, bar + 2 + DIGIT_WIDTH / 2, row, HUD_COLOUR, alpha))
        }
        Ammo::Count(n) => Some(number(&mut out, n, w - 4 * DIGIT_WIDTH - icon.x as i32, row, HUD_COLOUR, alpha)),
    };
    if let Some(x) = icon_x {
        out.push(Draw::Sprite { pic: key, x, y: row - icon.y as i32 / 8, src: None, colour: HUD_COLOUR, alpha });
    }
    out
}

/// The arcs' strengths (front, right, behind, left) after a hit from
/// direction `from` (toward the source, any length; flat in the view's
/// frame by `yaw`), `near`: the source within 50 units.
pub fn hit_arcs(mut strengths: [f32; 4], from: Vec3, yaw: f32, near: bool) -> [f32; 4] {
    if near {
        return [1.0; 4];
    }
    let from = from.with_y(0.0).normalize_or_zero();
    let forward = Vec3::new(-yaw.sin(), 0.0, -yaw.cos());
    let right = Vec3::new(yaw.cos(), 0.0, -yaw.sin());
    let (f, r) = (from.dot(forward), from.dot(right));
    if f > PAIN_SIDE {
        strengths[0] = strengths[0].max(f);
    } else if f < -PAIN_SIDE {
        strengths[2] = strengths[2].max(-f);
    }
    if r > PAIN_SIDE {
        strengths[1] = strengths[1].max(r);
    } else if r < -PAIN_SIDE {
        strengths[3] = strengths[3].max(-r);
    }
    strengths
}

/// The HUD's last frame and the screen it is laid out on (physical
/// pixels and the window's scale factor).
#[derive(Resource, Clone, Debug, PartialEq)]
pub struct Cs16HudState {
    pub frame: Frame,
    pub screen: (i32, i32, f32),
}

impl Default for Cs16HudState {
    fn default() -> Self {
        Self { frame: Frame::default(), screen: (640, 480, 1.0) }
    }
}

/// The pain arcs' strengths (front, right, behind, left).
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq)]
pub struct Pain(pub [f32; 4]);

/// The install's pictures as images by key (`ammo_key`, `pain_key`, the
/// shared entries' names), with their sizes in texels.
#[derive(Resource, Default)]
pub struct HudPictures(pub BTreeMap<String, (Handle<Image>, UVec2)>);

/// When `load_pictures` looks again (`Local`).
#[derive(Default)]
pub struct Tries {
    done: bool,
    next: f32,
    wait: f32,
}

const MOST_WAIT: f32 = 30.0;

fn image(images: &mut Assets<Image>, p: UiImage) -> (Handle<Image>, UVec2) {
    let size = UVec2::new(p.width, p.height);
    let image = Image::new(
        bevy::render::render_resource::Extent3d { width: p.width, height: p.height, depth_or_array_layers: 1 },
        bevy::render::render_resource::TextureDimension::D2,
        p.rgba8,
        bevy::render::render_resource::TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::default(),
    );
    (images.add(image), size)
}

/// A grey additive picture as one blended in `tint`: each texel the tint
/// with its brightness as alpha (for the kill feed, which blends).
pub fn tinted(p: &UiImage, tint: [u8; 3]) -> UiImage {
    let rgba8 = p
        .rgba8
        .chunks_exact(4)
        .flat_map(|px| {
            let peak = px[0].max(px[1]).max(px[2]);
            [tint[0], tint[1], tint[2], peak]
        })
        .collect();
    UiImage { width: p.width, height: p.height, rgba8 }
}

/// CS 1.6's pictures from its mount (the install, or the asset store's
/// `players` set), tried at startup and again further apart (2 s doubling
/// up to 30 s) until their files are here, never fetching: the HUD's
/// (`HudPictures`) and the kill icons and headshot mark, registered for
/// every app's kill feed (`WeaponIcons`).
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
    let Some(read) = cs16::game::hud::pictures(&mount) else { return };
    tries.done = true;
    let mut kill = |p: &UiImage| {
        let (image, size) = image(&mut images, tinted(p, KILL_ICON_COLOUR));
        WeaponIcon::Image { image, size: size.as_vec2() }
    };
    let kills: BTreeMap<String, WeaponIcon> = read.kill_icons.iter().map(|(id, p)| (id.clone(), kill(p))).collect();
    let skull = read.shared.get("d_skull").map(&mut kill);
    if let Some(head) = read.shared.get("d_headshot") {
        icons.register_mark(GAME, "headshot", kill(head));
    }
    info!("CS 1.6: {} kill icons, {} weapons' and {} shared HUD pictures", kills.len(), read.weapons.len(), read.shared.len());
    if !kills.is_empty() || skull.is_some() {
        icons.register(GAME, move |id| kills.get(id).cloned().or_else(|| skull.clone()));
    }
    let mut out = BTreeMap::new();
    for (name, p) in read.shared {
        out.insert(name, image(&mut images, p));
    }
    for (id, w) in read.weapons {
        if let Some(p) = w.ammo {
            out.insert(ammo_key(&id), image(&mut images, p));
        }
    }
    for (k, p) in read.pain.into_iter().take(4).enumerate() {
        out.insert(pain_key(k), image(&mut images, p));
    }
    pictures.0 = out;
}

/// A sprite node of the pool (`sync`), by index.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub struct SpriteSlot(pub usize);

/// A filled box of the pool (`sync`), by index.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub struct FillSlot(pub usize);

/// The root, hidden until CS 1.6's HUD is the gameplay's.
#[derive(Component)]
pub struct Cs16Root;

/// The root (its nodes are spawned by `sync` as the draws need them).
pub fn spawn_root(mut commands: Commands) {
    commands.spawn((
        Cs16Root,
        HudRoot(GAME),
        Visibility::Hidden,
        GlobalZIndex(GAME_HUD_Z),
        Node { position_type: PositionType::Absolute, width: percent(100.0), height: percent(100.0), ..default() },
    ));
}

/// The local player's state into the frame, the fades stepped, the
/// window's size read.
#[allow(clippy::type_complexity)]
pub fn collect(
    player: Option<Single<(&Health, Option<&Armor>, Option<&Inventory>, Has<Dead>, Has<Spectating>), With<LocalPlayer>>>,
    weapons: Query<(&Weapon, Option<&Magazine>, Option<&Throwable>)>,
    windows: Query<&Window>,
    time: Option<Res<Time>>,
    mut fades: ResMut<Fades>,
    mut state: ResMut<Cs16HudState>,
) {
    let f = match player {
        Some(p) => {
            let (health, armour, inv, dead, spectating) = *p;
            let held = inv.and_then(|i| i.active).and_then(|w| weapons.get(w).ok());
            let ammo = match held {
                Some((_, Some(m), _)) => Ammo::Magazine { clip: m.clip, reserve: m.reserve },
                Some((_, None, Some(t))) => Ammo::Count(t.count),
                _ => Ammo::None,
            };
            Frame {
                health: (!dead && !spectating).then(|| (health.current * 100.0).ceil().max(0.0) as u32),
                armour: armour.map_or((0, false), |a| ((a.amount * 100.0).round().max(0.0) as u32, a.helmet)),
                weapon_id: held.map(|(w, ..)| w.id.to_string()).unwrap_or_default(),
                ammo,
            }
        }
        None => Frame::default(),
    };
    fades.step(&f, time.map_or(0.0, |t| t.delta_secs()));
    let screen = windows
        .iter()
        .next()
        .map_or(state.screen, |w| (w.physical_width() as i32, w.physical_height() as i32, w.scale_factor()));
    if state.frame != f || state.screen != screen {
        state.frame = f;
        state.screen = screen;
    }
}

/// The arcs' strengths drained, and raised by the hits on the local
/// player (`core::DamageTaken`; a network client's from the server).
pub fn pain(
    mut taken: MessageReader<DamageTaken>,
    me: Option<Single<(Entity, &Intent, &GlobalTransform), With<LocalPlayer>>>,
    places: Query<&GlobalTransform>,
    time: Option<Res<Time>>,
    mut pain: ResMut<Pain>,
) {
    let dt = time.map_or(0.0, |t| t.delta_secs());
    let mut s = pain.0.map(|v| (v - PAIN_DRAIN * dt).max(0.0));
    if let Some(me) = me {
        let (me, intent, at) = *me;
        for t in taken.read() {
            let d = &t.damage;
            if d.target != me || !t.directed {
                continue;
            }
            let source = d.attacker.and_then(|a| places.get(a).ok()).map(|g| g.translation());
            let near = source.is_some_and(|p| p.distance(at.translation()) <= PAIN_NEAR * METERS_PER_UNIT);
            let from = source.map_or(-d.dir, |p| p - at.translation());
            s = hit_arcs(s, from, intent.yaw, near);
        }
    } else {
        taken.clear();
    }
    if pain.0 != s {
        pain.0 = s;
    }
}

fn colour(c: [u8; 3], alpha: u8) -> Color {
    Color::srgba_u8(c[0], c[1], c[2], alpha)
}

/// The draws into the pool's nodes (each a `HudGlyphMaterial`, additive,
/// or a filled box), at the window's pixels (physical / scale factor).
#[allow(clippy::type_complexity)]
pub fn sync(
    state: Res<Cs16HudState>,
    fades: Res<Fades>,
    pain: Res<Pain>,
    pictures: Res<HudPictures>,
    root: Option<Single<Entity, With<Cs16Root>>>,
    mut materials: ResMut<Assets<HudGlyphMaterial>>,
    mut sprites: Query<(&SpriteSlot, &MaterialNode<HudGlyphMaterial>, &mut Node, &mut Visibility), Without<FillSlot>>,
    mut fills: Query<(&FillSlot, &mut Node, &mut BackgroundColor, &mut Visibility), Without<SpriteSlot>>,
    mut commands: Commands,
) {
    let Some(root) = root else { return };
    let (w, h, sf) = state.screen;
    let sf = if sf > 0.0 { sf } else { 1.0 };
    let all = draws(&state.frame, &fades, &pain.0, w, h, |k| pictures.0.get(k).map(|p| p.1));
    let mut want_sprites = Vec::new();
    let mut want_fills = Vec::new();
    for d in all {
        match d {
            Draw::Sprite { pic, x, y, src, colour: c, alpha } => {
                if let Some((image, size)) = pictures.0.get(&pic) {
                    want_sprites.push((image.clone(), *size, x, y, src, colour(c, alpha)));
                }
            }
            Draw::Fill { rect, colour: c, alpha } => want_fills.push((rect, colour(c, alpha))),
        }
    }
    let px = |v: i32| Val::Px(v as f32 / sf);
    let mut have = 0;
    for (slot, material, mut node, mut vis) in &mut sprites {
        have = have.max(slot.0 + 1);
        let Some((image, size, x, y, src, c)) = want_sprites.get(slot.0) else {
            vis.set_if_neq(Visibility::Hidden);
            continue;
        };
        let [sx, sy, sw, sh] = src.unwrap_or([0, 0, size.x, size.y]);
        let uv = Vec4::new(sx as f32 / size.x as f32, sy as f32 / size.y as f32, sw as f32 / size.x as f32, sh as f32 / size.y as f32);
        let params = GlyphParams { color: c.to_linear(), uv };
        if let Some(m) = materials.get(&material.0)
            && (m.params != params || m.coverage != *image)
            && let Some(mut m) = materials.get_mut(&material.0)
        {
            m.params = params;
            m.coverage = image.clone();
        }
        let (l, t, ww, hh) = (px(*x), px(*y), px(sw as i32), px(sh as i32));
        if node.left != l || node.top != t || node.width != ww || node.height != hh {
            node.left = l;
            node.top = t;
            node.width = ww;
            node.height = hh;
        }
        vis.set_if_neq(Visibility::Inherited);
    }
    for k in have..want_sprites.len() {
        let (image, ..) = &want_sprites[k];
        let m = materials.add(HudGlyphMaterial { params: GlyphParams::whole(Color::NONE), coverage: image.clone(), additive: true });
        let node = Node { position_type: PositionType::Absolute, ..default() };
        commands.entity(*root).with_child((SpriteSlot(k), MaterialNode(m), node, Visibility::Hidden));
    }
    let mut have = 0;
    for (slot, mut node, mut bg, mut vis) in &mut fills {
        have = have.max(slot.0 + 1);
        let Some(([x, y, ww, hh], c)) = want_fills.get(slot.0) else {
            vis.set_if_neq(Visibility::Hidden);
            continue;
        };
        let (l, t, wv, hv) = (px(*x), px(*y), px(*ww), px(*hh));
        if node.left != l || node.top != t || node.width != wv || node.height != hv {
            node.left = l;
            node.top = t;
            node.width = wv;
            node.height = hv;
        }
        bg.set_if_neq(BackgroundColor(*c));
        vis.set_if_neq(Visibility::Inherited);
    }
    for k in have..want_fills.len() {
        let node = Node { position_type: PositionType::Absolute, ..default() };
        commands.entity(*root).with_child((FillSlot(k), node, BackgroundColor(Color::NONE), Visibility::Hidden));
    }
}

/// CS 1.6's HUD: its state, root and nodes, drawing while its HUD is the
/// gameplay's; its pictures and kill icons loaded once here.
pub struct Cs16HudPlugin;

impl Plugin for Cs16HudPlugin {
    fn build(&self, app: &mut App) {
        app.world_mut().get_resource_or_init::<chimera_client::client::hud::HudDraws>().0.push((GAME, DRAWS));
        // Its kill feed is the app's: it shows none of its own.
        chimera_client::client::kill_feed::GameKillFeedAppExt::register_game_kill_feed(app, GAME, |_| 0);
        app.init_resource::<Cs16HudState>()
            .init_resource::<Fades>()
            .init_resource::<Pain>()
            .init_resource::<HudPictures>()
            .init_resource::<WeaponIcons>()
            .add_message::<DamageTaken>()
            .add_systems(Startup, spawn_root)
            .add_systems(Update, ((collect, pain, sync).chain().run_if(chimera_core::gameplay::hud_is(GAME)), load_pictures));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sizes(k: &str) -> Option<UVec2> {
        match k {
            "cross" | "suit_empty" | "suit_full" | "suithelmet_empty" | "suithelmet_full" => Some(UVec2::splat(24)),
            k if k.starts_with("ammo:") => Some(UVec2::splat(24)),
            "pain_0" | "pain_2" => Some(UVec2::new(128, 48)),
            "pain_1" | "pain_3" => Some(UVec2::new(48, 128)),
            _ => None,
        }
    }

    fn sprites(d: &[Draw], prefix: &str) -> Vec<(String, i32, i32)> {
        d.iter()
            .filter_map(|d| match d {
                Draw::Sprite { pic, x, y, .. } if pic.starts_with(prefix) => Some((pic.clone(), *x, *y)),
                _ => None,
            })
            .collect()
    }

    fn at(health: u32) -> Frame {
        Frame { health: Some(health), ..default() }
    }

    /// hud.md's test cases 1 and 2.
    #[test]
    fn health_sits_by_the_formulas() {
        let d = draws(&at(100), &Fades::default(), &[0.0; 4], 640, 480, sizes);
        assert_eq!(sprites(&d, "cross"), vec![("cross".into(), 12, 443)]);
        let nums: Vec<_> = sprites(&d, "number_").into_iter().take(3).collect();
        assert_eq!(nums, vec![("number_1".into(), 34, 448), ("number_0".into(), 54, 448), ("number_0".into(), 74, 448)]);
        assert!(d.iter().any(|d| matches!(d, Draw::Fill { rect: [104, 448, 2, 25], .. })));
        let d = draws(&at(7), &Fades::default(), &[0.0; 4], 640, 480, sizes);
        let seven = d.iter().find(|d| matches!(d, Draw::Sprite { pic, .. } if pic == "number_7")).unwrap();
        assert!(matches!(seven, Draw::Sprite { x: 74, colour: WARNING_COLOUR, alpha: 255, .. }));
    }

    /// hud.md's test cases 3 and 4, and the ammunition's (9).
    #[test]
    fn fades_follow_the_spec() {
        let mut f = Fades::default();
        f.step(&at(100), 0.0);
        f.step(&at(60), 0.0);
        f.step(&at(60), 2.5);
        assert_eq!(health_alpha(60, f.health), 164);
        f.step(&at(60), 3.5);
        assert_eq!(health_alpha(60, f.health), 100);
        let mut f = Fades::default();
        let ak = |clip| Frame { health: Some(100), weapon_id: "cs16:weapon_ak47".into(), ammo: Ammo::Magazine { clip, reserve: 90 }, ..default() };
        f.step(&ak(30), 0.0);
        f.step(&ak(29), 0.0);
        assert_eq!(ammo_alpha(f.ammo), 200);
        f.step(&ak(29), 2.0);
        assert_eq!(ammo_alpha(f.ammo), 160);
    }

    /// hud.md's test cases 5 and 6.
    #[test]
    fn armour_lights_its_lower_part() {
        let f = Frame { health: Some(100), armour: (50, false), ..default() };
        let d = draws(&f, &Fades::default(), &[0.0; 4], 640, 480, sizes);
        assert_eq!(sprites(&d, "suit_empty"), vec![("suit_empty".into(), 128, 439)]);
        let lit = d.iter().find_map(|d| match d {
            Draw::Sprite { pic, y, src, .. } if pic == "suit_full" => Some((*y, *src)),
            _ => None,
        });
        assert_eq!(lit, Some((451, Some([0, 12, 24, 12]))));
        let nums = sprites(&d, "number_");
        assert_eq!(nums[3..], [("number_5".to_string(), 172, 448), ("number_0".to_string(), 192, 448)]);
        let d = draws(&f, &Fades::default(), &[0.0; 4], 1920, 1080, sizes);
        assert_eq!(sprites(&d, "suit_empty")[0].1, 384);
    }

    /// hud.md's test cases 7 and 8.
    #[test]
    fn ammunition_sits_by_the_formulas() {
        let f = Frame { health: Some(100), weapon_id: "cs16:weapon_ak47".into(), ammo: Ammo::Magazine { clip: 30, reserve: 90 }, ..default() };
        let d = draws(&f, &Fades::default(), &[0.0; 4], 640, 480, sizes);
        let xs: Vec<i32> = sprites(&d, "number_").iter().skip(4).map(|s| s.1).collect();
        assert_eq!(xs, vec![476, 496, 558, 578]);
        assert!(d.iter().any(|d| matches!(d, Draw::Fill { rect: [526, 448, 2, 25], .. })));
        assert_eq!(sprites(&d, "ammo:"), vec![("ammo:cs16:weapon_ak47".into(), 598, 445)]);
        let f = Frame { health: Some(100), weapon_id: "cs16:weapon_hegrenade".into(), ammo: Ammo::Count(1), ..default() };
        let d = draws(&f, &Fades::default(), &[0.0; 4], 640, 480, sizes);
        assert_eq!(sprites(&d, "number_").last().unwrap().1, 576);
        assert_eq!(sprites(&d, "ammo:")[0].1, 596);
        let knife = Frame { health: Some(100), weapon_id: "cs16:weapon_knife".into(), ..default() };
        assert!(sprites(&draws(&knife, &Fades::default(), &[0.0; 4], 640, 480, sizes), "ammo:").is_empty());
    }

    #[test]
    fn digits_leave_leading_places_empty() {
        assert_eq!(digits(0, 0), vec![(0, 40)]);
        assert_eq!(digits(42, 0), vec![(4, 20), (2, 40)]);
        assert_eq!(digits(1234, 0).len(), 4);
    }

    /// hud.md's test cases 13 and 14, and the arcs' places at 640 x 480.
    #[test]
    fn pain_arcs_light_by_direction() {
        // Looking down -Z (yaw 0); the source straight ahead.
        let s = hit_arcs([0.0; 4], Vec3::new(0.0, 0.0, -1.0), 0.0, false);
        assert_eq!(s, [1.0, 0.0, 0.0, 0.0]);
        // From the right (+X).
        assert_eq!(hit_arcs([0.0; 4], Vec3::X, 0.0, false), [0.0, 1.0, 0.0, 0.0]);
        assert_eq!(hit_arcs([0.0; 4], Vec3::X, 0.0, true), [1.0; 4]);
        // Drawn while above 0.4: 0.3 s at 2 a second.
        assert!(1.0 - PAIN_DRAIN * 0.29 > PAIN_SHOWN && 1.0 - PAIN_DRAIN * 0.31 < PAIN_SHOWN);
        let d = draws(&Frame::default(), &Fades::default(), &[1.0; 4], 640, 480, sizes);
        let places: Vec<_> = sprites(&d, "pain_").into_iter().map(|s| (s.1, s.2)).collect();
        assert_eq!(places, vec![(256, 96), (416, 176), (256, 336), (176, 176)]);
    }

    #[test]
    fn a_kill_icon_is_tinted_by_its_brightness() {
        let p = UiImage { width: 2, height: 1, rgba8: vec![0, 0, 0, 255, 200, 100, 50, 255] };
        assert_eq!(tinted(&p, KILL_ICON_COLOUR).rgba8, vec![255, 80, 0, 0, 255, 80, 0, 200]);
    }
}
