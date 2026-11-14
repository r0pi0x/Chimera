//! The Signal menu's backdrops (docs/plans/active/ugg-ui.md, M5), in the
//! right two-thirds of the menu (`signal_menu`), over the ambient ground,
//! fading out toward the menu column:
//!
//! - **A Ladder**: fourteen gun plates between two rails leaning 12°,
//!   seen straight on, climbed one plate a beat (1.6 s: a 4 % hoist and an
//!   eased settle, the look book's keyframes, `scenes::ladder_state`); the
//!   brackets glide and lock with a gentle zoom; the knife plate at the top
//!   in cobalt with a finish chequer on the slant, a crown and FINAL •
//!   KNIFE; a 14-tick meter at the right. Each gun's game's own kill icon
//!   when at hand (`WeaponIcons`, `icons.prefer_game`: a glyph of its
//!   icon font or its image), else the kit's class icon. The climb is its
//!   dwell (`LADDER_PERIOD`, 24.5 s).
//! - **B Assembly**: a gun drafted on its 16-unit grid, traced by a 12°
//!   scan, hatched, filled, dimensioned, filed on a shelf (3.4 s a gun).
//! - **C Ident**: the mark signs on facet by facet on one 12° scan with a
//!   cobalt wake, a bloom and outline flash on the lock, a breathing light
//!   and a sheen through the hold, signs off. Slot C is the in-engine
//!   Showcase when assets are at hand: milestone M5b, not built yet; the
//!   Ident is its no-asset fallback and what shows until then.
//!
//! Drawing: each backdrop is painted on the CPU (`paint::Paint`: convex
//! fills, exact gradients, strokes, the kit's icons as triangles, all
//! clipped by half-planes) into one vertex-coloured `Mesh2d` with its
//! texts as `Text2d` (images as `Sprite`s), on a render layer of its own,
//! by a 2D camera drawing into an image the size of the backdrop's area
//! (the right 64 % of the window, physical px, MSAA); the menu shows it
//! through `BackdropsMaterial` (`menu_backdrops.wesl`), which fades it out
//! toward the menu column (the look book's `destination-out` ramp: 0 at
//! the left edge, 8 % at 27 % of the width, whole from 46 %) and runs the
//! swap: the outgoing backdrop keeps drawing into a second image while the
//! incoming one wipes over it on the slant with a 3 px accent edge
//! (`motion.dur.menu.backdrop_swap`, `cubic_in_out`). The cameras run only
//! while the menu's first page shows (the second only during a swap);
//! a frozen backdrop (reduced motion, a staged scene) is painted once.
//!
//! Known shortcuts: the lock's glow is a wider faint stroke (no blur);
//! a label's wipe-in is a fade; the Ladder's guns are the look book's
//! cross-game climb, not the server's coming ladder.

pub mod paint;
pub mod scenes;

use bevy::{
    asset::{RenderAssetUsages, embedded_asset},
    camera::{RenderTarget, visibility::RenderLayers},
    color::LinearRgba,
    core_pipeline::tonemapping::Tonemapping,
    mesh::{Indices, PrimitiveTopology},
    prelude::*,
    reflect::TypePath,
    render::render_resource::{AsBindGroup, Extent3d, TextureFormat},
    shader::ShaderRef,
    sprite::Anchor,
    window::PrimaryWindow,
};
use chimera_client::client::{logo_pass::ReduceMotion, weapon_icons::WeaponIcons};
use chimera_ui::{IconImages, KitFonts, Theme, Tone, UiClock};

use crate::main_menu::{Screen, ShownScreen};
use crate::signal_menu::{SignalMenu, themed};
use paint::{Align, Face, Paint};
pub use scenes::LADDER_PERIOD;
use scenes::{Pal, Rung, Shapes};

/// The backdrops' area: the right part of the window from here (0-1).
pub const AREA_LEFT: f32 = 0.36;

/// The render layers the two canvases draw on.
const LAYERS: [usize; 2] = [21, 22];

/// Each backdrop's still frame (reduced motion; a staged scene's default).
pub fn still(which: usize) -> f32 {
    match which {
        0 => scenes::LADDER_STILL,
        1 => scenes::ASSEMBLY_STILL,
        _ => scenes::IDENT_STILL,
    }
}

/// Paints backdrop `which` at `t` into `p` (a frame `w` x `h` physical px).
#[allow(clippy::too_many_arguments)]
pub fn paint(
    p: &mut Paint,
    shapes: &Shapes,
    pal: Pal,
    rungs: &[Rung],
    which: usize,
    w: f32,
    h: f32,
    t: f32,
    still: bool,
) {
    // A look-book pixel: its frames are 1280 wide at 720 lines.
    let d = (h / 720.0).max(1.0);
    match which {
        0 => scenes::ladder(p, shapes, pal, rungs, w, h, t, d, still),
        1 => scenes::assembly(p, shapes, pal, w, h, t, d),
        _ => scenes::ident(p, shapes, pal, w, h, t, d),
    }
}

/// The ladder's rungs, each with its game's icon when at hand and
/// preferred (`icons::choose`).
pub fn rungs(icons: Option<&WeaponIcons>, prefer_game: bool) -> Vec<Rung> {
    scenes::LADDER
        .iter()
        .map(|&(name, game, class, id)| {
            let own = icons.and_then(|i| i.icon(id));
            let game_icon = match chimera_ui::icons::choose(prefer_game, own.is_some()) {
                chimera_ui::icons::IconSource::Game => own,
                chimera_ui::icons::IconSource::Fallback => None,
            };
            Rung {
                name,
                game,
                class,
                id,
                game_icon,
            }
        })
        .collect()
}

/// The composite: the two canvases, the swap's wipe and the fade toward
/// the menu (module doc).
#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
pub struct BackdropsMaterial {
    #[uniform(0)]
    pub acc: LinearRgba,
    /// x: the wipe's x at the foot (screen physical px), y: the wipe is
    /// on (0/1), z: the edge's width (px), w: tan of the slant.
    #[uniform(1)]
    pub wipe: Vec4,
    /// x: the area's left (screen px), y, z: the screen's width and
    /// height (px), w: opacity.
    #[uniform(2)]
    pub frame: Vec4,
    /// The current backdrop's sheen (`Paint::sheen`; w 0: none).
    #[uniform(7)]
    pub sheen: Vec4,
    /// The type colour (the sheen's).
    #[uniform(8)]
    pub fg: LinearRgba,
    #[texture(3)]
    #[sampler(4)]
    pub a: Handle<Image>,
    #[texture(5)]
    #[sampler(6)]
    pub b: Handle<Image>,
}

impl UiMaterial for BackdropsMaterial {
    fn fragment_shader() -> ShaderRef {
        "embedded://party_kit/menu_backdrops.wesl".into()
    }
}

/// The node showing the backdrops (under the page's frame).
#[derive(Component)]
pub struct BackdropLayer;

/// One canvas: its camera, image, mesh and the texts and images drawn
/// beside the mesh.
struct Slot {
    camera: Entity,
    image: Handle<Image>,
    mesh: Handle<Mesh>,
    texts: Vec<Entity>,
    sprites: Vec<Entity>,
    /// What it last painted (backdrop, time in ms, size): a frozen
    /// backdrop is painted once.
    painted: Option<(usize, i64, UVec2)>,
}

/// The canvases and what they share.
#[derive(Resource, Default)]
pub struct Canvases {
    slots: Vec<Slot>,
    size: UVec2,
    shapes: Option<Shapes>,
    paint: Option<Paint>,
    rungs: Vec<Rung>,
    /// Triangles painted last frame (tests, the frame's cost).
    pub triangles: usize,
    /// The current backdrop's sheen for the composite.
    sheen: Vec4,
}

pub fn add(app: &mut App) {
    app.init_resource::<Canvases>().add_systems(
        PostUpdate,
        draw.before(chimera_ui::UiKitSystems)
            .run_if(|t: Option<Res<Theme>>| themed(t.as_deref())),
    );
    if app.get_sub_app(bevy::render::RenderApp).is_some() {
        embedded_asset!(app, "menu_backdrops.wesl");
        app.add_plugins(UiMaterialPlugin::<BackdropsMaterial>::default());
    } else if app.is_plugin_added::<AssetPlugin>() {
        app.init_asset::<BackdropsMaterial>();
    }
}

/// The backdrops' node under the page (`signal_menu` builds it): the area
/// right of `AREA_LEFT`.
pub fn spawn_layer(c: &mut Commands, root: Entity) {
    c.spawn((
        BackdropLayer,
        Name::new("Menu backdrops"),
        Node {
            position_type: PositionType::Absolute,
            left: percent(AREA_LEFT * 100.0),
            right: px(0.0),
            top: px(0.0),
            bottom: px(0.0),
            ..default()
        },
        ChildOf(root),
    ));
}

/// A canvas text's parts.
type TextParts = (
    &'static mut Text2d,
    &'static mut TextFont,
    &'static mut TextColor,
    &'static mut Anchor,
    &'static mut Transform,
    &'static mut Visibility,
    &'static mut bevy::text::LetterSpacing,
);

/// Render-side assets, when there is a renderer.
type RenderAssets<'w> = (
    Option<ResMut<'w, Assets<Image>>>,
    Option<ResMut<'w, Assets<Mesh>>>,
    Option<ResMut<'w, Assets<ColorMaterial>>>,
    Option<ResMut<'w, Assets<BackdropsMaterial>>>,
);

/// Paints the shown backdrops and keeps their canvases (module doc).
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn draw(
    theme: Res<Theme>,
    fonts: Option<Res<KitFonts>>,
    clock: Res<UiClock>,
    shown: Res<ShownScreen>,
    state: Res<SignalMenu>,
    reduce: Option<Res<ReduceMotion>>,
    weapon_icons: Option<Res<WeaponIcons>>,
    mut icon_images: ResMut<IconImages>,
    windows: Query<&Window, With<PrimaryWindow>>,
    layer: Query<(Entity, Option<&MaterialNode<BackdropsMaterial>>), With<BackdropLayer>>,
    mut canvases: ResMut<Canvases>,
    (mut images, mut meshes, mut colors, mut composites): RenderAssets,
    mut cameras: Query<&mut Camera>,
    mut texts: Query<TextParts, Without<Sprite>>,
    mut sprites: Query<(&mut Sprite, &mut Transform, &mut Visibility), Without<Text2d>>,
    mut commands: Commands,
) {
    let cv = &mut *canvases;
    let up = shown.screen == Screen::MainMenu && !layer.is_empty();
    let still = reduce.is_some_and(|r| r.on != 0);
    // Which backdrop each canvas shows, and when in it.
    let wants: [Option<(usize, f32)>; 2] = if up {
        let prev = state.previous.map(|p| {
            (
                p,
                if state.freeze.is_some() {
                    still_or(state.freeze, p)
                } else {
                    (clock.now - state.previous_since) as f32
                },
            )
        });
        let cur_t = if still {
            self::still(state.backdrop)
        } else {
            state.local(clock.now)
        };
        [Some((state.backdrop, cur_t)), prev.filter(|_| !still)]
    } else {
        [None, None]
    };
    for (i, slot) in cv.slots.iter().enumerate() {
        if let Ok(mut cam) = cameras.get_mut(slot.camera) {
            let on = wants[i].is_some();
            if cam.is_active != on {
                cam.is_active = on;
            }
        }
    }
    if !up {
        return;
    }
    let (pw, ph) = windows.iter().next().map_or((1920, 1080), |w| {
        (w.physical_width().max(2), w.physical_height().max(2))
    });
    let x0 = (pw as f32 * AREA_LEFT).round() as u32;
    let size = UVec2::new(pw - x0, ph);
    if cv.shapes.is_none() {
        cv.shapes = Some(Shapes::new(icon_images.icons()));
    }
    if cv.rungs.is_empty() || weapon_icons.as_ref().is_some_and(|w| w.is_changed()) {
        cv.rungs = rungs(weapon_icons.as_deref(), theme.icons.prefer_game);
    }
    let pal = Pal {
        fg: theme.tone(Tone::Fg).into(),
        acc: theme.tone(Tone::Acc).into(),
    };
    let renders = images.is_some() && meshes.is_some() && colors.is_some() && composites.is_some();
    // The canvases, once there is a renderer; resized with the window.
    if renders && cv.slots.is_empty() {
        let (images, meshes, colors) = (
            images.as_deref_mut().unwrap(),
            meshes.as_deref_mut().unwrap(),
            colors.as_deref_mut().unwrap(),
        );
        for (i, l) in LAYERS.iter().enumerate() {
            let image = images.add(Image::new_target_texture(
                size.x,
                size.y,
                TextureFormat::Rgba8UnormSrgb,
                None,
            ));
            let mesh = meshes.add(Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default()));
            let camera = commands
                .spawn((
                    Name::new("Menu backdrop canvas"),
                    Camera2d,
                    Camera {
                        order: -40 + i as isize,
                        clear_color: ClearColorConfig::Custom(Color::NONE),
                        is_active: false,
                        ..default()
                    },
                    RenderTarget::Image(image.clone().into()),
                    Tonemapping::None,
                    Msaa::Sample4,
                    RenderLayers::layer(*l),
                ))
                .id();
            commands.spawn((
                Mesh2d(mesh.clone()),
                MeshMaterial2d(colors.add(ColorMaterial {
                    // Blended: the vertices' alphas count.
                    alpha_mode: bevy::sprite_render::AlphaMode2d::Blend,
                    ..ColorMaterial::from_color(Color::WHITE)
                })),
                Transform::default(),
                RenderLayers::layer(*l),
            ));
            cv.slots.push(Slot {
                camera,
                image,
                mesh,
                texts: Vec::new(),
                sprites: Vec::new(),
                painted: None,
            });
        }
        cv.size = size;
    }
    if renders && cv.size != size {
        let images = images.as_deref_mut().unwrap();
        for s in &cv.slots {
            if let Some(mut img) = images.get_mut(&s.image) {
                img.resize(Extent3d {
                    width: size.x,
                    height: size.y,
                    depth_or_array_layers: 1,
                });
            }
        }
        cv.size = size;
    }
    // The composite node's material.
    if let (true, Some((node, mat))) = (renders && !cv.slots.is_empty(), layer.iter().next()) {
        let composites = composites.as_deref_mut().unwrap();
        let swap = theme.dur_opt(crate::signal_menu::L, "backdrop_swap").unwrap_or(0.52);
        let p = ((clock.now - state.swap_at) as f32 / swap).clamp(0.0, 1.0);
        let e = chimera_ui::Ease::CubicInOut.sample(p);
        let (w, h) = (pw as f32, ph as f32);
        let tan = theme.tan();
        let ex = paint::lerp(-h * tan, w * 1.02, e);
        let d = (h / 720.0).max(1.0);
        let m = BackdropsMaterial {
            acc: theme.tone(Tone::Acc).into(),
            wipe: Vec4::new(ex, if wants[1].is_some() { 1.0 } else { 0.0 }, 3.0 * d, tan),
            frame: Vec4::new(x0 as f32, w, h, 1.0),
            sheen: cv.sheen,
            fg: theme.tone(Tone::Fg).into(),
            a: cv.slots[0].image.clone(),
            b: cv.slots[1].image.clone(),
        };
        match mat.and_then(|m| composites.get(&m.0).map(|x| (m.0.clone(), x.wipe, x.frame, x.sheen))) {
            Some((handle, wipe, frame, sheen)) => {
                if (wipe != m.wipe || frame != m.frame || sheen != m.sheen)
                    && let Some(mut x) = composites.get_mut(&handle)
                {
                    x.wipe = m.wipe;
                    x.frame = m.frame;
                    x.sheen = m.sheen;
                }
            }
            None => {
                commands.entity(node).insert(MaterialNode(composites.add(m)));
            }
        }
    }
    // Paint each canvas.
    let shapes = cv.shapes.take().unwrap();
    let mut p = cv.paint.take().unwrap_or_else(Paint::new);
    let mut tris = 0;
    let frozen = still || state.freeze.is_some();
    for (i, want) in wants.iter().enumerate() {
        let Some((which, t)) = *want else { continue };
        let key = (which, (t * 1000.0).round() as i64, size);
        if frozen && cv.slots.get(i).is_some_and(|s| s.painted == Some(key)) {
            continue;
        }
        p.clear();
        self::paint(&mut p, &shapes, pal, &cv.rungs, which, pw as f32, ph as f32, t, still);
        tris += p.triangles();
        if i == 0 {
            cv.sheen = p.sheen.map_or(Vec4::ZERO, Vec4::from_array);
        }
        let Some(slot) = cv.slots.get_mut(i) else { continue };
        slot.painted = Some(key);
        // The mesh: canvas px (y down) to the camera's world (y up,
        // centred), the area's left at its left edge.
        if let Some(mut mesh) = meshes.as_deref_mut().and_then(|m| m.get_mut(&slot.mesh)) {
            let (ox, oy) = (x0 as f32 + size.x as f32 / 2.0, size.y as f32 / 2.0);
            let pos: Vec<[f32; 3]> = p.pos.iter().map(|q| [q[0] - ox, oy - q[1], 0.0]).collect();
            let n = pos.len();
            mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, pos);
            mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, p.col.clone());
            mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, vec![[0.0f32, 0.0]; n]);
            mesh.insert_indices(Indices::U32(p.idx.clone()));
        }
        sync_texts(
            &mut commands,
            slot,
            &p,
            &theme,
            fonts.as_deref(),
            x0,
            size,
            LAYERS[i],
            &mut texts,
        );
        sync_sprites(&mut commands, slot, &p, x0, size, LAYERS[i], &mut sprites);
    }
    cv.triangles = tris;
    cv.shapes = Some(shapes);
    cv.paint = Some(p);
}

fn still_or(freeze: Option<f32>, which: usize) -> f32 {
    freeze.unwrap_or_else(|| still(which))
}

/// The canvas's texts as `Text2d`s, reusing the slot's.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn sync_texts(
    commands: &mut Commands,
    slot: &mut Slot,
    p: &Paint,
    theme: &Theme,
    fonts: Option<&KitFonts>,
    x0: u32,
    size: UVec2,
    layer: usize,
    texts: &mut Query<TextParts, Without<Sprite>>,
) {
    let (ox, oy) = (x0 as f32 + size.x as f32 / 2.0, size.y as f32 / 2.0);
    for (k, item) in p.texts.iter().enumerate() {
        let font = match &item.face {
            Face::Role(role) => {
                let mut f = chimera_ui::fonts::text(theme, fonts, *role, item.px).font;
                f.font_size = bevy::text::FontSize::Px(item.px);
                f
            }
            Face::Font(h) => TextFont {
                font: bevy::text::FontSource::Handle(h.clone()),
                font_size: bevy::text::FontSize::Px(item.px),
                ..default()
            },
        };
        // The baseline's point to the line box's foot (a fifth of the size
        // below it, the line one size tall).
        let at = Vec3::new(item.at.x - ox, oy - (item.at.y + item.px * 0.2), 1.0);
        let anchor = match item.align {
            Align::Left => Anchor::BOTTOM_LEFT,
            Align::Centre => Anchor::BOTTOM_CENTER,
            Align::Right => Anchor::BOTTOM_RIGHT,
        };
        let color = TextColor(Color::LinearRgba(item.color));
        if k >= slot.texts.len() {
            let e = commands
                .spawn((
                    Text2d(item.text.clone()),
                    font,
                    color,
                    bevy::text::LetterSpacing::Px(item.track * item.px),
                    chimera_ui::fonts::LINE_HEIGHT,
                    anchor,
                    Transform::from_translation(at),
                    RenderLayers::layer(layer),
                ))
                .id();
            slot.texts.push(e);
            continue;
        }
        let Ok((mut t, mut f, mut c, mut a, mut tf, mut vis, mut ls)) = texts.get_mut(slot.texts[k]) else {
            continue;
        };
        if t.0 != item.text {
            t.0 = item.text.clone();
        }
        if *f != font {
            *f = font;
        }
        c.set_if_neq(color);
        a.set_if_neq(anchor);
        if tf.translation != at {
            tf.translation = at;
        }
        vis.set_if_neq(Visibility::Inherited);
        ls.set_if_neq(bevy::text::LetterSpacing::Px(item.track * item.px));
    }
    for &e in slot.texts.iter().skip(p.texts.len()) {
        if let Ok((.., mut vis, _)) = texts.get_mut(e) {
            vis.set_if_neq(Visibility::Hidden);
        }
    }
}

/// The canvas's images as `Sprite`s, reusing the slot's.
#[allow(clippy::type_complexity)]
fn sync_sprites(
    commands: &mut Commands,
    slot: &mut Slot,
    p: &Paint,
    x0: u32,
    size: UVec2,
    layer: usize,
    sprites: &mut Query<(&mut Sprite, &mut Transform, &mut Visibility), Without<Text2d>>,
) {
    let (ox, oy) = (x0 as f32 + size.x as f32 / 2.0, size.y as f32 / 2.0);
    for (k, item) in p.images.iter().enumerate() {
        let centre = item.min + item.size / 2.0;
        let at = Vec3::new(centre.x - ox, oy - centre.y, 0.5);
        let sprite = Sprite {
            image: item.image.clone(),
            color: Color::LinearRgba(item.color),
            custom_size: Some(item.size),
            ..default()
        };
        if k >= slot.sprites.len() {
            let e = commands
                .spawn((sprite, Transform::from_translation(at), RenderLayers::layer(layer)))
                .id();
            slot.sprites.push(e);
            continue;
        }
        let Ok((mut s, mut tf, mut vis)) = sprites.get_mut(slot.sprites[k]) else {
            continue;
        };
        if s.image != sprite.image || s.color != sprite.color || s.custom_size != sprite.custom_size {
            *s = sprite;
        }
        if tf.translation != at {
            tf.translation = at;
        }
        vis.set_if_neq(Visibility::Inherited);
    }
    for &e in slot.sprites.iter().skip(p.images.len()) {
        if let Ok((.., mut vis)) = sprites.get_mut(e) {
            vis.set_if_neq(Visibility::Hidden);
        }
    }
}
