//! The Bevy side of TF2's HUD: the local player's state into `hud::Inputs`
//! each frame, the frame (`hud::Tf2Hud::frame`) into UI nodes under its
//! own root (`HudRoot(GAME)`, drawn by its own unit, H / 480), run only
//! while TF2's HUD is the gameplay's (`gameplay::hud_is`) and its look is
//! the active one (`map::hud::ActiveHud`, TF2's: `game::hud`); and TF2's
//! guns' kill icons for every app's kill feed (`register_kill_icons`: the
//! cells of `mod_textures.txt` cut from `hud/d_images`, from the install
//! or the asset store once they are here).
//!
//! Known shortcuts: the node pool and its sync repeat HL2:DM's HUD's own
//! (a shared one in `source_engine::hud` would serve both); the kill
//! feed's row look is the party's (its shared feed), TF2's icons in it.

use std::{collections::BTreeMap, sync::Arc};

use bevy::{asset::RenderAssetUsages, prelude::*};
use chimera_client::client::{
    hud::{GameDraws, HudRoot, ScopeUp},
    hud_text::{GlyphFont, GlyphParams, HudGlyphMaterial, HudText},
    weapon_icons::{WeaponIcon, WeaponIcons},
};
use chimera_core::core::{Dead, Health, LocalPlayer, Team};
use chimera_sim::weapon::{HeldGauge, Inventory, Magazine, Weapon};
use chimera_world::map::hud::{ActiveHud, GameHud, font_for_height};

use crate::hud::{Ammo, Draw, Inputs, Rgba, Scheme, Screen, Tf2Hud};
use tf2::game::{GAME, INFO, classes::{Classes, Tf2Class}};

/// What this HUD draws instead of the shell's: the crosshair, health and
/// ammo (the kill feed stays the app's).
pub const DRAWS: GameDraws = GameDraws { crosshair: true, status: true, hit_marker: false, killfeed: false };

/// The HUD's state and the frame it drew.
#[derive(Resource, Default)]
pub struct Tf2HudState {
    pub hud: Tf2Hud,
    pub inputs: Inputs,
    pub frame: Vec<Draw>,
}

/// The look as the HUD reads it.
pub struct Look<'a> {
    pub hud: &'a GameHud,
    pub screen: Screen,
}

impl Look<'_> {
    /// A scheme font's file and cell height (pixels) at this height.
    pub fn font(&self, name: &str) -> Option<(GlyphFont, f32)> {
        let e = font_for_height(self.hud.text_fonts.get(name)?, self.screen.h)?;
        let h = self.screen.h.round() as u32;
        let tall = if e.yres.is_some_and(|(lo, hi)| (lo..=hi).contains(&h)) { e.tall } else { (e.tall * self.screen.h / 480.0).trunc() };
        let file = self.hud.fonts.get(&format!("family:{}", e.family.to_lowercase()))?;
        Some((GlyphFont { data: file.data.clone(), additive: e.additive }, tall))
    }
}

impl Scheme for Look<'_> {
    fn colour(&self, name: &str) -> Option<Rgba> {
        self.hud.colors.get(name).copied().or_else(|| {
            self.hud.colors.iter().find(|(k, _)| k.eq_ignore_ascii_case(name)).map(|(_, v)| *v)
        })
    }

    fn tall(&self, font: &str) -> Option<f32> {
        self.font(font).map(|(_, t)| t)
    }

    fn has(&self, sprite: &str) -> bool {
        self.hud.sprites.contains_key(sprite)
    }
}

/// The active look when it is TF2's.
fn ours(active: Option<&ActiveHud>) -> Option<&ActiveHud> {
    active.filter(|a| a.0.game == GAME)
}

/// The local player's state into the inputs, and the frame.
#[allow(clippy::type_complexity)]
pub fn collect(
    mut state: ResMut<Tf2HudState>,
    time: Res<Time>,
    windows: Query<&Window>,
    active: Option<Res<ActiveHud>>,
    player: Option<Single<(&Health, Option<&Inventory>, Option<&Team>, Has<Dead>, Option<&Tf2Class>), With<LocalPlayer>>>,
    mut classes: Local<Option<Classes>>,
    weapons: Query<(&Weapon, Option<&Magazine>, Option<&HeldGauge>)>,
    meters: Query<(
        Option<&tf2::game::items::meters::Tf2Meter>,
        Option<&tf2::game::items::consumables::Tf2Consumable>,
        Option<&tf2::game::items::shields::Tf2Shield>,
        Option<&tf2::game::items::spy::Tf2Cloak>,
        Option<&tf2::game::items::kit::Tf2Thruster>,
    )>,
    target: Res<Tf2Target>,
    scope: Option<Res<ScopeUp>>,
    shoulder: Option<Res<chimera_client::client::third_person::Shoulder>>,
) {
    let (Some(active), Some(w)) = (ours(active.as_deref()), windows.iter().next()) else {
        state.frame.clear();
        return;
    };
    let Some((health, inv, team, dead, class)) = player.map(|p| *p) else {
        state.frame.clear();
        return;
    };
    let gun = inv.and_then(|i| i.active).and_then(|e| weapons.get(e).ok());
    let ammo = gun.and_then(|(_, m, _)| m).map(|m| Ammo {
        clip: m.clip as i32,
        reserve: (m.reserve_max > 0).then_some(m.reserve as i32),
    });
    let blu = tf2::game::bodies::skin_of(team.copied()) == 1;
    let table = classes.get_or_insert_with(tf2::game::classes::classes);
    let portrait = class.and_then(|c| tf2::game::hud::portrait(&c.class(table).name, blu));
    let inputs = Inputs {
        alive: !dead && health.current > 0.0,
        health: (health.current * 100.0).round(),
        max: (health.max * 100.0).round(),
        blu,
        ammo,
        portrait,
        time: time.elapsed_secs_f64(),
        scope: scope.is_some_and(|s| s.0).then(|| gun.and_then(|(.., g)| g).map_or(0.0, |g| g.0)),
        crosshair_dim: shoulder.map_or(0.0, |s| s.dim),
        // The first item meter carried (batch2b_mechanics.md 14).
        meter: inv.and_then(|i| {
            i.weapons.iter().find_map(|w| match meters.get(*w).ok()? {
                (Some(m), ..) => {
                    let (l, f) = m.meter();
                    let label = if m.params.kind == tf2::game::items::meters::MeterKind::Heads { format!("{l}: {}", m.heads()) } else { l.to_string() };
                    Some((label, f))
                }
                (_, Some(c), ..) => {
                    let (l, f) = c.meter();
                    Some((l.to_string(), f))
                }
                (_, _, Some(s), ..) => {
                    let (l, f) = s.meter();
                    Some((l.to_string(), f))
                }
                // A watch's cloak (batch3_mechanics.md 3).
                (_, _, _, Some(c), _) => {
                    let (l, f) = c.meter();
                    Some((l.to_string(), f))
                }
                (.., Some(t)) => {
                    let (l, f) = t.meter();
                    Some((l.to_string(), f))
                }
                _ => None,
            })
        }),
        target: target.0.clone(),
    };
    let look = Look { hud: &active.0, screen: Screen::new(w.width(), w.height()) };
    let st = &mut *state;
    st.frame = st.hud.frame(&inputs, look.screen, &look);
    st.inputs = inputs;
}

/// The player under the local player's crosshair, as this machine shows
/// him (`target`): name and health points.
#[derive(Resource, Default, Clone, Debug, PartialEq)]
pub struct Tf2Target(pub Option<(String, f32)>);

/// How far the target ID reaches (units).
pub const TARGET_RANGE: f32 = 2000.0;

/// The player under the crosshair (`Tf2Target`): a living one within
/// `TARGET_RANGE`, not hidden; a disguised enemy Spy by his disguise's
/// name and health (`items::spy::shown_pose`).
#[allow(clippy::type_complexity)]
pub fn target(
    mut out: ResMut<Tf2Target>,
    me: Option<Single<(Entity, &Transform, &chimera_core::core::Intent, &chimera_core::core::MovementState, Option<&Team>), With<LocalPlayer>>>,
    who: Query<(
        Option<&chimera_net::net::NetCharacter>,
        Option<&Name>,
        Option<&Health>,
        Option<&Team>,
        Option<&chimera_core::guise::Guise>,
    )>,
    spatial: avian3d::prelude::SpatialQuery,
    parents: Query<&ChildOf>,
    shoulder: Option<Res<chimera_client::client::third_person::Shoulder>>,
) {
    use chimera_client::client::aim;
    let found = me.and_then(|me| {
        let (e, at, intent, state, team) = *me;
        let ray = aim::crosshair_ray(shoulder.as_deref(), aim::view_ray(at, intent, state, None));
        let (t, _) = aim::character_under(&spatial, ray, TARGET_RANGE * 0.0254, e, |x| who.contains(x) && x != e, &parents)?;
        let (net, name, health, theirs, guise) = who.get(t).ok()?;
        if health.is_none_or(|h| h.current <= 0.0) || guise.is_some_and(|g| g.hidden > 127) {
            return None;
        }
        match tf2::game::items::spy::shown_pose(guise, theirs, t, Some((e, team))) {
            Some(p) => Some((p.name.clone(), p.health * 100.0)),
            None => {
                let n = net.map(|n| n.name.clone()).or_else(|| name.map(|n| n.to_string())).unwrap_or_default();
                Some((n, health.map_or(0.0, |h| h.current * 100.0)))
            }
        }
    });
    if out.0 != found {
        out.0 = found;
    }
}

/// The HUD's root node.
#[derive(Component)]
pub struct Root;

/// A pooled node of the frame.
#[derive(Component)]
pub struct Pooled;

const KIND_BOX: u8 = 0;
const KIND_TEXT: u8 = 1;
const KIND_SPRITE: u8 = 2;

/// The pictures drawn additively (their black adds nothing): the charge
/// meter's.
const ADDITIVE: &[&str] = &["sniperscope_numbers2"];

pub fn spawn_root(mut commands: Commands) {
    commands.spawn((
        Root,
        HudRoot(GAME),
        Node { position_type: PositionType::Absolute, left: px(0.0), top: px(0.0), width: percent(100.0), height: percent(100.0), ..default() },
        Visibility::Inherited,
        GlobalZIndex(chimera_client::client::hud::GAME_HUD_Z),
        Pickable::IGNORE,
    ));
}

/// The frame into UI nodes: a pool of nodes per kind, reused frame to
/// frame, the extras hidden.
#[allow(clippy::too_many_arguments)]
pub fn sync(
    state: Res<Tf2HudState>,
    roots: Query<Entity, With<Root>>,
    mut pool: Local<[Vec<Entity>; 3]>,
    mut nodes: Query<(&mut Node, &mut Visibility, &mut ZIndex)>,
    mut boxes: Query<&mut BackgroundColor>,
    mut sprites: Query<&mut MaterialNode<HudGlyphMaterial>>,
    (active, windows): (Option<Res<ActiveHud>>, Query<&Window>),
    mut materials: ResMut<Assets<HudGlyphMaterial>>,
    images: Res<Assets<Image>>,
    mut commands: Commands,
) {
    let Some(root) = roots.iter().next() else { return };
    let mut used = [0usize; 3];
    if let (Some(active), Some(w)) = (ours(active.as_deref()), windows.iter().next()) {
        let look = Look { hud: &active.0, screen: Screen::new(w.width(), w.height()) };
        for (z, d) in state.frame.iter().enumerate() {
            let kind = match d {
                Draw::Box { .. } => KIND_BOX,
                Draw::Text { .. } => KIND_TEXT,
                Draw::Sprite { .. } => KIND_SPRITE,
            };
            let slot = used[kind as usize];
            used[kind as usize] += 1;
            let entity = match pool[kind as usize].get(slot) {
                Some(e) => *e,
                None => {
                    let mut e = commands.spawn((
                        Pooled,
                        Node { position_type: PositionType::Absolute, ..default() },
                        Visibility::Hidden,
                        ZIndex(0),
                        ChildOf(root),
                    ));
                    if kind == KIND_BOX {
                        e.insert(BackgroundColor(Color::NONE));
                    }
                    let id = e.id();
                    pool[kind as usize].push(id);
                    if kind == KIND_TEXT {
                        // The text itself, a child of the screen-sized node.
                        commands.spawn((
                            Node { position_type: PositionType::Absolute, ..default() },
                            HudText {
                                font: GlyphFont { data: Arc::new(Vec::new()), additive: false },
                                tall: 1.0,
                                text: String::new(),
                                color: Color::NONE,
                                at: Vec2::ZERO,
                                align: crate::hud::Align::Left,
                            },
                            ChildOf(id),
                        ));
                    }
                    id
                }
            };
            let Ok((mut node, mut vis, mut zi)) = nodes.get_mut(entity) else { continue };
            zi.set_if_neq(ZIndex(z as i32));
            vis.set_if_neq(Visibility::Inherited);
            let place = |node: &mut Node, r: Rect| {
                node.left = px(r.min.x);
                node.top = px(r.min.y);
                node.width = px(r.width());
                node.height = px(r.height());
            };
            let rgba = |c: Rgba| Color::srgba_u8(c[0], c[1], c[2], c[3]);
            match d {
                Draw::Box { rect, color, radius } => {
                    node.border_radius = BorderRadius::all(px(*radius));
                    place(&mut node, *rect);
                    if let Ok(mut bg) = boxes.get_mut(entity) {
                        bg.set_if_neq(BackgroundColor(rgba(*color)));
                    }
                }
                Draw::Text { font, tall, text, color, at, align } => {
                    place(&mut node, Rect::new(0.0, 0.0, look.screen.w, look.screen.h));
                    let Some((glyph, _)) = look.font(font) else { continue };
                    let want = HudText { font: glyph, tall: *tall, text: text.clone(), color: rgba(*color), at: *at, align: *align };
                    commands.queue(move |w: &mut World| {
                        let child = w.get::<Children>(entity).and_then(|c| c.first().copied());
                        if let Some(child) = child
                            && let Some(mut t) = w.get_mut::<HudText>(child)
                            && *t != want
                        {
                            *t = want;
                        }
                    });
                }
                Draw::Sprite { sprite, rect, uv, color } => {
                    place(&mut node, *rect);
                    let Some(sp) = look.hud.sprites.get(*sprite) else { continue };
                    let Some(image) = active.1.get(&sp.texture) else { continue };
                    let size = images.get(image).map_or(Vec2::ONE, |i| i.size().as_vec2());
                    let [x, y, w, h] = sp.rect;
                    let (u0, v0) = ((x + uv.min.x * w) / size.x, (y + uv.min.y * h) / size.y);
                    let (du, dv) = (uv.width() * w / size.x, uv.height() * h / size.y);
                    let want = HudGlyphMaterial {
                        params: GlyphParams { color: rgba(*color).to_linear(), uv: Vec4::new(u0, v0, du, dv) },
                        coverage: image.clone(),
                        additive: ADDITIVE.contains(sprite),
                    };
                    match sprites.get_mut(entity) {
                        Ok(m) => {
                            if materials.get(&m.0).is_none_or(|cur| cur.params != want.params || cur.coverage != want.coverage || cur.additive != want.additive) {
                                let _ = materials.insert(&m.0, want);
                            }
                        }
                        Err(_) => {
                            commands.entity(entity).insert(MaterialNode(materials.add(want)));
                        }
                    }
                }
            }
        }
    }
    for (kind, list) in pool.iter().enumerate() {
        for e in list.iter().skip(used[kind]) {
            if let Ok((_, mut v, _)) = nodes.get_mut(*e) {
                v.set_if_neq(Visibility::Hidden);
            }
        }
    }
}

pub use tf2::game::hud::{crop, icon_cells};

/// When `register_kill_icons` looks again (`Local`): registered, or the
/// next try and the wait after it.
#[derive(Default)]
pub struct KillIconTries {
    done: bool,
    next: f32,
    wait: f32,
}

/// The longest wait between two tries (s).
const MOST_WAIT: f32 = 30.0;

/// TF2's guns' kill icons (each gun's `hud.icon`, the kill-icon name: the
/// Rocket Launcher's the rocket's), cut from TF2's sheet
/// (`game::hud::kill_icon_pictures`) through TF2's mount (the install, or
/// the asset store: `players`, which every TF2 match offers, carries
/// `scripts/mod_textures.txt` and the sheets), registered for every
/// app's kill feed once their files are here: tried at startup and again,
/// further apart each time (2 s doubling up to 30 s), until they are (a
/// tester reading TF2 from the asset service gets them with the first
/// match that offers TF2); never fetching. Until then the feed writes the
/// gun's name.
pub fn register_kill_icons(
    mut icons: ResMut<WeaponIcons>,
    images: Option<ResMut<Assets<Image>>>,
    time: Option<Res<Time<Real>>>,
    mut tries: Local<KillIconTries>,
) {
    let now = time.map_or(0.0, |t| t.elapsed_secs());
    if tries.done || now < tries.next {
        return;
    }
    let Some(mut images) = images else { return };
    tries.wait = (tries.wait * 2.0).clamp(2.0, MOST_WAIT);
    tries.next = now + tries.wait;
    let Some(mount) = INFO.mount() else { return };
    let Some(pictures) = tf2::game::hud::kill_icon_pictures(&mount) else { return };
    tries.done = true;
    if pictures.is_empty() {
        warn!("TF2: no kill icons (scripts/mod_textures.txt or its sheets don't read)");
        return;
    }
    let by_gun: BTreeMap<String, (Handle<Image>, Vec2)> = pictures
        .into_iter()
        .map(|(gun, p)| {
            let size = Vec2::new(p.width as f32, p.height as f32);
            let image = Image::new(
                bevy::render::render_resource::Extent3d { width: p.width, height: p.height, depth_or_array_layers: 1 },
                bevy::render::render_resource::TextureDimension::D2,
                p.rgba8,
                bevy::render::render_resource::TextureFormat::Rgba8UnormSrgb,
                RenderAssetUsages::default(),
            );
            (gun, (images.add(image), size))
        })
        .collect();
    info!("TF2: {} kill icons", by_gun.len());
    icons.register(GAME, move |id| by_gun.get(id).map(|(image, size)| WeaponIcon::Image { image: image.clone(), size: *size }));
}

#[cfg(test)]
mod tests {
    use chimera_world::map::hud::UiImage;

    use super::*;

    /// hud.md 3.6's cells by name, in the file's shape (ours).
    #[test]
    fn the_kill_icon_cells_read() {
        let file = r#""sprites/640_hud"
        {
            TextureData
            {
                "scattergun" { "dfile" "hud/d_images" "dnegfile" "hud/dneg_images" "x" "96" "y" "192" "width" "96" "height" "32" }
                "minigun" { "file" "hud/d_images" "x" "0" "y" "384" "width" "96" "height" "32" }
                "ammo_12g" { "font" "TFTypeDeath" "character" "a" }
            }
        }"#;
        let c = icon_cells(file);
        assert_eq!(c.get("scattergun"), Some(&("hud/d_images".to_string(), [96, 192, 96, 32])));
        assert_eq!(c.get("minigun").map(|x| x.1), Some([0, 384, 96, 32]));
        assert!(!c.contains_key("ammo_12g"));
    }

    #[test]
    fn a_cell_is_cut_row_by_row() {
        let image = UiImage { width: 4, height: 2, rgba8: (0..32).collect() };
        assert_eq!(crop(&image, 1, 1, 2, 1), Some((20..28).collect()));
        assert_eq!(crop(&image, 3, 0, 2, 1), None);
    }
}
