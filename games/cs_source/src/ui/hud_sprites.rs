//! The game HUD's sprite pieces: the ammo-type icon in the ammo panel, and
//! the damage indicator (the HUD layout's `HudDamageIndicator`: a red bar
//! at the screen's left or right side, both for a hit from in front or
//! behind) that flashes on the side a hit came from.
//!
//! The game HUD's sprite icons (`map::hud::HudSprite`, rectangles of the game's
//! HUD sprite sheets): the ammo-type icon for types without a glyph (buckshot;
//! added to the screen) and the damage direction indicators (the HUD layout's
//! `HudDamageIndicator` side bars, from `core::DamageTaken`: what reached the
//! health, a network client's from the server as `net::Hurt`; a fall lights
//! both).
//!
//! Known shortcuts (docs/tech-debt.md): Damage indicator
//! (`client::hud_sprites`): the bars' place and size are hudlayout.res's
//! `HudDamageIndicator`, but which bar lights is ours (left or right by the
//! attacker's side, both from in front or behind or a fall), solid red at 0.8
//! alpha fading over 1 s regardless of the damage; `mp_fadetoblack` fades over
//! 1 s under the HUD (ours). Why it's OK for now: Capture C8.

use bevy::prelude::*;

use super::game_hud::scaled;
use crate::client::hud_text::{GlyphParams, HudGlyphMaterial};

use crate::{
    core::{DamageTaken, Dead, Intent, LocalPlayer},
    map::hud::{ActiveHud, GameHud},
    weapon::{Inventory, Weapon},
};

pub struct HudSpritesPlugin;

impl Plugin for HudSpritesPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (
                build.run_if(resource_exists_and_changed::<ActiveHud>),
                (pain, update).chain().run_if(resource_exists::<ActiveHud>),
            )
                .distributive_run_if(crate::gameplay::hud_is(crate::game::GAME)),
        )
        // The look gone (or put away for another game's HUD): its parts go.
        .add_systems(Update, teardown.run_if(resource_removed::<ActiveHud>));
    }
}

/// Seconds a damage indicator takes to fade (an assumption; the game's
/// timing isn't in its data files: capture C8).
const PAIN_SECONDS: f32 = 1.0;

#[derive(Component)]
struct SpritePart;

#[derive(Component)]
struct AmmoIcon;

/// A damage indicator bar (`LEFT` or `RIGHT`) and its seconds left.
#[derive(Component)]
struct Pain(usize, f32);

const LEFT: usize = 0;
const RIGHT: usize = 1;

/// The ammo icon for a weapon: its ammo type's (weapon data,
/// `hud_icon`).
pub fn ammo_icon(weapon: &str) -> Option<&'static str> {
    crate::game::weapons::data(weapon)?.ammo.as_ref()?.hud_icon.as_deref()
}

// The sprite sheets' additive pixels as alpha: the shell's (its HUD uses it
// too), at its old path.
pub use crate::client::hud::additive_as_alpha;

fn build(
    hud: Option<Res<ActiveHud>>,
    old: Query<Entity, With<SpritePart>>,
    mut images: ResMut<Assets<Image>>,
    mut commands: Commands,
) {
    // Gone mid-frame (the map unloaded: back to the menu).
    let Some(hud) = hud else { return };
    for e in &old {
        commands.entity(e).despawn();
    }
    let absolute = || Node {
        position_type: PositionType::Absolute,
        ..default()
    };
    commands.spawn((SpritePart, AmmoIcon, absolute(), Visibility::Hidden, GlobalZIndex(40)));
    // The damage indicator's bars, when the layout has it.
    if let Some(bars) = hud.0.panels.get("HudDamageIndicator").map(DamageBars::of) {
        let shape = images.add(trapezoid(bars));
        for side in [LEFT, RIGHT] {
            let node = ImageNode {
                image: shape.clone(),
                color: Color::NONE,
                flip_x: side == RIGHT,
                ..default()
            };
            commands.spawn((SpritePart, Pain(side, 0.0), node, absolute(), GlobalZIndex(41)));
        }
    }
}

/// The damage indicator's bars as the HUD layout gives them
/// (`HudDamageIndicator`, 480-line units): each bar's outer edge
/// `xpos` in from the screen's side, `wide` across, `tall1` high at the
/// outer edge from `ypos` down, `tall2` at the inner edge (centred on the
/// outer one), in `color` (its alpha is the hit's).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DamageBars {
    pub xpos: f32,
    pub ypos: f32,
    pub wide: f32,
    pub tall1: f32,
    pub tall2: f32,
    pub color: [u8; 3],
}

impl DamageBars {
    fn of(p: &crate::map::hud::HudPanel) -> Self {
        let num = |k: &str, d: f32| p.keys.get(k).and_then(|v| v.trim().parse().ok()).unwrap_or(d);
        let color = p
            .keys
            .get("dmgcolorleft")
            .map(|v| v.split_whitespace().filter_map(|x| x.parse::<u8>().ok()).collect::<Vec<_>>())
            .filter(|c| c.len() >= 3)
            .map_or([255, 0, 0], |c| [c[0], c[1], c[2]]);
        Self {
            xpos: num("dmg_xpos", 30.0),
            ypos: num("dmg_ypos", 100.0),
            wide: num("dmg_wide", 36.0),
            tall1: num("dmg_tall1", 240.0),
            tall2: num("dmg_tall2", 200.0),
            color,
        }
    }

    /// The left bar's box in a window `w` x `h` (the right one mirrors it).
    pub fn rect(&self, side: usize, w: f32, h: f32) -> Rect {
        let s = h / 480.0;
        let left = if side == LEFT {
            self.xpos * s
        } else {
            w - (self.xpos + self.wide) * s
        };
        Rect::new(left, self.ypos * s, left + self.wide * s, (self.ypos + self.tall1) * s)
    }
}

/// The left bar's shape: white where the trapezoid is (the outer edge on
/// the left, full height; the inner edge `tall2` of it), clear elsewhere.
fn trapezoid(bars: DamageBars) -> Image {
    use bevy::{
        asset::RenderAssetUsages,
        render::render_resource::{Extent3d, TextureDimension, TextureFormat},
    };
    const PER_UNIT: f32 = 4.0;
    let (w, h) = ((bars.wide * PER_UNIT).max(1.0) as u32, (bars.tall1 * PER_UNIT).max(1.0) as u32);
    let mut data = vec![0u8; (w * h * 4) as usize];
    for y in 0..h {
        for x in 0..w {
            let across = (x as f32 + 0.5) / w as f32;
            let half = (bars.tall1 + (bars.tall2 - bars.tall1) * across) / bars.tall1 / 2.0;
            let from_mid = ((y as f32 + 0.5) / h as f32 - 0.5).abs();
            if from_mid <= half {
                let i = ((y * w + x) * 4) as usize;
                data[i..i + 4].copy_from_slice(&[255, 255, 255, 255]);
            }
        }
    }
    Image::new(
        Extent3d {
            width: w,
            height: h,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        data,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::all(),
    )
}

fn teardown(parts: Query<Entity, With<SpritePart>>, mut commands: Commands) {
    for e in &parts {
        commands.entity(e).despawn();
    }
}

/// Light the indicator on the side each hit on the local player came from:
/// damage that reached its health (`core::DamageTaken`; not under god
/// mode or refused by the rules), a network client's from the server
/// (`net::Hurt`).
fn pain(
    mut taken: MessageReader<DamageTaken>,
    me: Option<Single<(Entity, &Intent), With<LocalPlayer>>>,
    mut parts: Query<&mut Pain>,
    time: Res<Time>,
) {
    let dt = time.delta_secs();
    for mut p in &mut parts {
        p.1 = (p.1 - dt).max(0.0);
    }
    let Some(me) = me else {
        taken.clear();
        return;
    };
    let (me, intent) = *me;
    for t in taken.read() {
        let d = &t.damage;
        if d.target != me {
            continue;
        }
        // A fall has no direction: both sides light (a guess; see
        // docs/tech-debt.md).
        let lit = if d.kind == crate::core::DamageKind::Fall {
            vec![LEFT, RIGHT]
        } else if !t.directed {
            continue;
        } else {
            sides(d.dir, intent.yaw)
        };
        for mut p in &mut parts {
            if lit.contains(&p.0) {
                p.1 = PAIN_SECONDS;
            }
        }
    }
}

/// Which bars a hit lights: the left one for a hit from the left, the
/// right one from the right, both from in front or behind (ours: capture
/// C8). From the shot's direction (`dir`, attacker toward you) and your
/// look yaw.
fn sides(dir: Vec3, yaw: f32) -> Vec<usize> {
    let from = -dir.with_y(0.0).normalize_or_zero();
    let right = Vec3::new(yaw.cos(), 0.0, -yaw.sin());
    let r = from.dot(right);
    if r < -0.3 {
        vec![LEFT]
    } else if r > 0.3 {
        vec![RIGHT]
    } else {
        vec![LEFT, RIGHT]
    }
}

#[allow(clippy::type_complexity)]
fn update(
    hud: Option<Res<ActiveHud>>,
    windows: Query<&Window>,
    player: Option<Single<(Option<&Inventory>, Has<Dead>), With<LocalPlayer>>>,
    weapons: Query<&Weapon>,
    mut icon: Query<
        (
            &mut Node,
            &mut Visibility,
            Option<&MaterialNode<HudGlyphMaterial>>,
            Entity,
        ),
        (With<AmmoIcon>, Without<Pain>),
    >,
    mut pains: Query<(&Pain, &mut Node, &mut ImageNode), Without<AmmoIcon>>,
    images: Res<Assets<Image>>,
    mut materials: ResMut<Assets<HudGlyphMaterial>>,
    mut commands: Commands,
) {
    // Gone mid-frame (the map unloaded: back to the menu).
    let Some(hud) = hud else { return };
    let Some(window) = windows.iter().next() else { return };
    let (w, h) = (window.width(), window.height());
    let scale = h / 480.0;
    let game: &GameHud = &hud.0;
    let (inventory, dead) = player.map(|p| *p).unwrap_or((None, true));
    // Ammo icon: the sprite at its size in 480-line pixels, its corner at
    // the panel's icon position, added to the screen in the panel colour
    // (black is nothing), as CS:S draws it.
    if let (Ok((mut node, mut vis, material, e)), Some(panel)) = (icon.single_mut(), game.panels.get("HudAmmo")) {
        let sprite = inventory
            .and_then(|i| i.active)
            .and_then(|w| weapons.get(w).ok())
            .and_then(|w| ammo_icon(w.id))
            // Ammo types with a glyph are drawn by `game_hud`.
            .filter(|n| super::game_hud::ammo_glyph(n).is_none() || !game.fonts.contains_key("CSTypeDeath"))
            .and_then(|n| game.sprites.get(n));
        let image = sprite
            .filter(|_| !dead)
            .and_then(|s| Some((s, hud.1.get(&s.texture)?.clone())));
        match image {
            Some((s, image)) => {
                let fg = game.color("Panel.FgColor").unwrap_or(Color::srgba_u8(255, 176, 0, 120));
                // The sprite's rectangle of its sheet.
                let size = images.get(&image).map_or(Vec2::ONE, |i| i.size().as_vec2());
                let [x, y, sw, sh] = s.rect;
                let want = HudGlyphMaterial {
                    params: GlyphParams {
                        color: fg.to_linear(),
                        uv: Vec4::new(x / size.x, y / size.y, sw / size.x, sh / size.y),
                    },
                    coverage: image,
                    additive: true,
                };
                let same = material
                    .and_then(|m| materials.get(&m.0))
                    .is_some_and(|m| m.coverage == want.coverage && m.params == want.params);
                if !same {
                    commands.entity(e).insert(MaterialNode(materials.add(want)));
                }
                let r = super::game_hud::panel_rect(panel, w, h);
                let at = r.min + Vec2::new(scaled(panel.icon.x, scale), scaled(panel.icon.y, scale));
                // Written only on a change (`ui_write`).
                crate::client::ui_write::edit(&mut node, |node| {
                    node.left = px(at.x);
                    node.top = px(at.y);
                    node.width = px(scaled(s.rect[2], scale));
                    node.height = px(scaled(s.rect[3], scale));
                });
                vis.set_if_neq(Visibility::Inherited);
            }
            None => {
                vis.set_if_neq(Visibility::Hidden);
            }
        }
    }
    // The damage indicator's bars at the screen's sides.
    let Some(bars) = game.panels.get("HudDamageIndicator").map(DamageBars::of) else {
        return;
    };
    for (p, mut node, mut img) in &mut pains {
        let r = bars.rect(p.0, w, h);
        crate::client::ui_write::edit(&mut node, |node| {
            node.left = px(r.min.x);
            node.top = px(r.min.y);
            node.width = px(r.width());
            node.height = px(r.height());
        });
        // Fading out over `PAIN_SECONDS` from its full strength (ours).
        let alpha = if dead { 0.0 } else { (p.1 / PAIN_SECONDS).clamp(0.0, 1.0) };
        let [r, g, b] = bars.color;
        let color = Color::srgba_u8(r, g, b, 255).with_alpha(alpha * 0.8);
        if img.color != color {
            img.color = color;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hits_light_the_side_they_came_from() {
        // Facing -Z (yaw 0); a shot travelling +Z came from in front:
        // both bars, as from behind.
        assert_eq!(sides(Vec3::Z, 0.0), vec![LEFT, RIGHT]);
        assert_eq!(sides(Vec3::NEG_Z, 0.0), vec![LEFT, RIGHT]);
        // From the left (-X side): the shot travels +X.
        assert_eq!(sides(Vec3::X, 0.0), vec![LEFT]);
        assert_eq!(sides(Vec3::NEG_X, 0.0), vec![RIGHT]);
        // Turned 90 degrees left (facing -X), a shot from -X is in front.
        assert_eq!(sides(Vec3::X, std::f32::consts::FRAC_PI_2), vec![LEFT, RIGHT]);
        // Diagonal: front and right.
        assert_eq!(sides(Vec3::new(-1.0, 0.0, 1.0), 0.0), vec![RIGHT]);
    }

    #[test]
    fn damage_bars_sit_where_the_layout_puts_them() {
        // CS:S's hudlayout.res values.
        let bars = DamageBars {
            xpos: 30.0,
            ypos: 100.0,
            wide: 36.0,
            tall1: 240.0,
            tall2: 200.0,
            color: [255, 0, 0],
        };
        // 1280x960: 2 pixels a unit.
        assert_eq!(bars.rect(LEFT, 1280.0, 960.0), Rect::new(60.0, 200.0, 132.0, 680.0));
        assert_eq!(bars.rect(RIGHT, 1280.0, 960.0), Rect::new(1148.0, 200.0, 1220.0, 680.0));
        // The shape: full height at the outer edge, tall2 at the inner.
        let img = trapezoid(bars);
        let (w, h) = (img.width(), img.height());
        let a = |x: u32, y: u32| img.data.as_ref().unwrap()[((y * w + x) * 4 + 3) as usize];
        assert_eq!((a(0, 2), a(0, h / 2)), (255, 255), "the outer edge, top to bottom");
        let inner_gap = ((240.0 - 200.0) / 2.0 / 240.0 * h as f32) as u32;
        assert_eq!(a(w - 1, inner_gap - 2), 0, "above the inner edge");
        assert_eq!(a(w - 1, inner_gap + 2), 255, "the inner edge");
    }

    #[test]
    fn additive_sprites_turn_black_into_transparency() {
        use bevy::{
            asset::RenderAssetUsages,
            render::render_resource::{Extent3d, TextureDimension, TextureFormat},
        };
        let img = Image::new(
            Extent3d { width: 3, height: 1, depth_or_array_layers: 1 },
            TextureDimension::D2,
            vec![0, 0, 0, 255, 128, 0, 0, 255, 255, 255, 255, 255],
            TextureFormat::Rgba8UnormSrgb,
            RenderAssetUsages::all(),
        );
        let out = additive_as_alpha(&img).unwrap();
        let d = out.data.unwrap();
        assert_eq!(&d[0..4], &[0, 0, 0, 0], "black is clear");
        assert_eq!(&d[4..8], &[255, 0, 0, 128], "half-bright red is full red at half coverage");
        assert_eq!(&d[8..12], &[255, 255, 255, 255], "white stays");
    }

    #[test]
    fn ammo_icons_by_weapon() {
        crate::game::weapons::cs_weapons();
        assert_eq!(ammo_icon("cs_source:weapon_ak47"), Some("ammo_762"));
        assert_eq!(ammo_icon("cs_source:weapon_usp"), Some("ammo_45"));
        assert_eq!(ammo_icon("cs_source:weapon_knife"), None);
    }
}
