//! Halo's HUD on the client shell (the Bevy side of `ui::hud`): the HUD's
//! data read from the donor map on a thread the first time Halo's HUD is
//! up (`HudLoad`), its pages as textures (`HudImages`), what the local
//! player's unit and weapon are this frame (`collect`), the per-tick state
//! (`HudState`, advanced by the Halo ticks the frame crossed), and the
//! draw list as UI material nodes under Halo's own root (`HudRoot`), sized
//! from the design canvas (`layout::Canvas`), reused frame to frame.

use std::sync::{Arc, Mutex};

use bevy::{
    asset::RenderAssetUsages,
    image::ImageSampler,
    prelude::*,
    render::render_resource::{Extent3d, TextureDimension, TextureFormat},
    window::PrimaryWindow,
};
use blam_formats::cache::MapFile;
use chimera_client::client::{FirstPersonCamera, hud::HudRoot};
use chimera_sim::weapon::Inventory;

use super::{
    data::HudData,
    draw::{self, Fill, HudState, Held, Other, Quad, Unit},
    layout::Canvas,
    material::{HudMaterial, HudParams},
    rules::{BlipKind, WeaponNow},
};
use crate::{
    core::{DamageTaken, Dead, Health, Intent, LocalPlayer, Team, Velocity},
    game::{
        GAME, INFO, METERS_PER_UNIT,
        body::load::DONOR_MAP,
        maps::map_path,
        vitality::Vitality,
        weapons::{HaloGun, HaloHands, HaloWeapon, tick::MagazineMode},
    },
};

/// The HUD's data: not asked for yet, being read, read, or why not.
#[derive(Resource, Default)]
pub enum HudLoad {
    #[default]
    Idle,
    Loading(Arc<Mutex<Option<Result<HudData, String>>>>),
    Ready(Arc<HudData>),
    Failed(String),
}

impl HudLoad {
    pub fn data(&self) -> Option<&Arc<HudData>> {
        match self {
            HudLoad::Ready(d) => Some(d),
            _ => None,
        }
    }
}

/// The pages' textures: (page tag id, image index) to its image.
#[derive(Resource, Default)]
pub struct HudImages(pub std::collections::BTreeMap<(u32, usize), Handle<Image>>);

/// The local player's HUD: the state, this frame's unit and the quads
/// drawn.
#[derive(Resource, Default)]
pub struct HaloHud {
    pub state: HudState,
    pub unit: Option<Unit>,
    pub quads: Vec<Quad>,
    /// Where the quads' blocks are (`draw::Blocks`, canvas units).
    pub blocks: draw::Blocks,
    /// The canvas they are drawn on.
    pub canvas: Option<Canvas>,
    /// The scope's blur this frame (`draw::scope_blur`; `scope_post`).
    pub blur: Option<draw::ScopeBlur>,
}

/// Halo's HUD root.
#[derive(Component)]
pub struct Root;

/// A picture node of the root, by its place in the draw list.
#[derive(Component)]
pub struct Picture(pub usize);

pub fn spawn_root(mut commands: Commands) {
    commands.spawn((
        Root,
        HudRoot(GAME),
        Node { position_type: PositionType::Absolute, left: px(0.0), top: px(0.0), width: percent(100.0), height: percent(100.0), ..default() },
        Visibility::Inherited,
        Pickable::IGNORE,
    ));
}

/// The HUD read from the install's donor map (`body::load::DONOR_MAP`).
pub fn read_donor() -> Result<HudData, String> {
    let install = INFO.install().ok_or_else(|| format!("no `{GAME}` install"))?;
    let mount = (INFO.open)(&install).map_err(|e| format!("{}: {e}", install.display()))?;
    let path = map_path(DONOR_MAP);
    let bytes = mount.read(&path).map_err(|e| format!("{path}: {e}"))?;
    let map = MapFile::load(&bytes).map_err(|e| format!("{path}: {e}"))?;
    HudData::load(&map).map_err(|e| format!("{path}: {e}"))
}

/// Read the donor map's HUD on a thread (once), and make its textures
/// when it is read.
pub fn load(mut load: ResMut<HudLoad>, mut images: ResMut<HudImages>, assets: Option<ResMut<Assets<Image>>>) {
    match &*load {
        HudLoad::Idle => {
            let slot = Arc::new(Mutex::new(None));
            let out = slot.clone();
            std::thread::spawn(move || {
                let r = read_donor();
                *out.lock().unwrap_or_else(|e| e.into_inner()) = Some(r);
            });
            *load = HudLoad::Loading(slot);
        }
        HudLoad::Loading(slot) => {
            let Some(r) = slot.lock().unwrap_or_else(|e| e.into_inner()).take() else { return };
            *load = match r {
                Ok(data) => {
                    if let Some(mut assets) = assets {
                        for (id, page) in &data.pages {
                            for (i, (w, h, rgba)) in page.images.iter().enumerate() {
                                let mut image = Image::new(
                                    Extent3d { width: *w, height: *h, depth_or_array_layers: 1 },
                                    TextureDimension::D2,
                                    rgba.clone(),
                                    TextureFormat::Rgba8Unorm,
                                    RenderAssetUsages::RENDER_WORLD,
                                );
                                image.sampler = if page.meter { ImageSampler::nearest() } else { ImageSampler::linear() };
                                images.0.insert((*id, i), assets.add(image));
                            }
                        }
                    }
                    HudLoad::Ready(Arc::new(data))
                }
                Err(e) => {
                    warn!("Halo's HUD: {e}");
                    HudLoad::Failed(e)
                }
            };
        }
        _ => {}
    }
}

/// A position in Halo's horizontal world frame (x forward at yaw 0, y
/// left: the engine's -z and -x), in world units.
fn halo_xy(p: Vec3) -> [f32; 2] {
    [-p.z / METERS_PER_UNIT, -p.x / METERS_PER_UNIT]
}

/// What the HUD reads of a Halo gun.
pub fn weapon_now(gun: &HaloGun, weapon: &HaloWeapon, firing: bool) -> WeaponNow {
    let mag = weapon.0.magazine.as_ref();
    WeaponNow {
        loaded: i32::from(gun.loaded),
        reserve: i32::from(gun.reserve),
        loaded_max: mag.map_or(0, |m| i32::from(m.rounds_loaded_maximum)),
        reloading: matches!(gun.magazine, MagazineMode::Reloading | MagazineMode::Chambering),
        ready: gun.magazine == MagazineMode::Idle,
        magazine: mag.is_some(),
        heat: gun.heat,
        age: gun.age,
        overheated: gun.overheated,
        zoom: gun.zoom,
        firing,
    }
}

type Me<'a> = (Entity, &'a GlobalTransform, Option<&'a Vitality>, Option<&'a Health>, Has<Dead>, Option<&'a Inventory>, Option<&'a Intent>, Option<&'a Team>);
type Them<'a> = (Entity, &'a GlobalTransform, Option<&'a Velocity>, Option<&'a Intent>, Option<&'a Team>, Has<Dead>);

/// This frame's unit and the tracker's units; then the Halo ticks the
/// frame crossed, and the draw list.
#[allow(clippy::too_many_arguments)]
pub fn collect(
    load: Res<HudLoad>,
    time: Res<Time>,
    me: Query<Me, With<LocalPlayer>>,
    guns: Query<(&HaloGun, &HaloWeapon)>,
    hands: Query<&HaloHands, With<LocalPlayer>>,
    others: Query<Them, With<Health>>,
    camera: Query<&GlobalTransform, With<FirstPersonCamera>>,
    window: Query<&Window, With<PrimaryWindow>>,
    mut hud: ResMut<HaloHud>,
    layout: Option<Res<super::HaloHudLayout>>,
    mut taken: MessageReader<DamageTaken>,
    (mut victims, effects): (MessageReader<crate::game::fx::victim::HaloVictim>, Option<Res<crate::game::vitality::DamageEffects>>),
) {
    let layout = layout.map_or_else(Default::default, |l| l.0);
    // A hit whose damage effect has "indicators always point down" sets
    // only the bottom arrow (hud.md 12): its `HaloVictim` comes in the
    // same frame as its `DamageTaken`.
    let downs: Vec<bool> = victims
        .read()
        .filter(|v| v.directed)
        .map(|v| effects.as_deref().is_some_and(|e| points_down(v, e)))
        .collect();
    let Some(data) = load.data() else {
        taken.clear();
        return;
    };
    let now = (time.elapsed_secs_f64() * 30.0).floor() as i64;
    let Some((me_e, at, vitality, health, dead, inventory, intent, team)) = me.iter().next() else {
        hud.unit = None;
        hud.quads.clear();
        hud.blur = None;
        hud.state.arrows = [None; 4];
        taken.clear();
        return;
    };
    let firing = intent.is_some_and(|i| i.fire);
    let weapon = inventory.and_then(|i| i.active).and_then(|e| guns.get(e).ok()).map(|(gun, w)| Held { tag: w.0.tag.clone(), now: weapon_now(gun, w, firing) });
    // The camera's vertical field of view for the scope's blur (hud.md 9).
    let v = inventory.and_then(|i| i.active).and_then(|e| guns.get(e).ok()).map_or(0.0, |(gun, w)| {
        let m = if gun.zoom >= 0 { w.0.magnification(gun.zoom) } else { 1.0 };
        draw::scope_fov(w.0.camera_field_of_view, m)
    });
    let alive = !dead && vitality.is_none_or(|v| !v.dead) && health.is_none_or(|h| h.current > 0.0);
    let unit = Unit {
        alive,
        shield: vitality.map_or(0.0, |v| v.shield),
        body: vitality.map_or_else(|| health.map_or(0.0, |h| h.current), |v| v.body),
        depleted: vitality.is_some_and(|v| v.depleted),
        weapon,
        // The selected type's count (`grenades::Throw::selected`).
        grenades: hands.get(me_e).map_or(0, |h| h.throw.selected().map_or(0, |k| i32::from(h.throw.counts[k]))),
        throwing: hands.get(me_e).is_ok_and(|h| h.grenade_held),
        grenade_kind: hands.get(me_e).map_or(0, |h| h.throw.selected().unwrap_or(0)),
        no_grenade_type: hands.get(me_e).is_ok_and(|h| h.throw.selected().is_none()),
    };
    let eye = camera.iter().next().unwrap_or(at);
    let f = eye.forward();
    let fwd = halo_xy(*f);
    let yaw = fwd[1].atan2(fwd[0]);
    let origin = halo_xy(at.translation());
    let tick_wu = METERS_PER_UNIT * 30.0;
    let others: Vec<Other> = others
        .iter()
        .filter(|(.., dead)| !dead)
        .map(|(e, t, v, i, their_team, _)| {
            let p = halo_xy(t.translation());
            let kind = if e == me_e {
                BlipKind::Own
            } else if team.is_some_and(|a| a.0 != 0 && their_team.is_some_and(|b| b == a)) {
                BlipKind::Friend
            } else {
                BlipKind::Enemy
            };
            Other {
                id: e.to_bits(),
                kind,
                at: [p[0] - origin[0], p[1] - origin[1]],
                speed: v.map_or(0.0, |v| v.0.length() / tick_wu),
                firing: i.is_some_and(|i| i.fire),
            }
        })
        .collect();
    let hud = &mut *hud;
    // The damage arrows: each hit that reached the player from a source
    // (hud.md 12), by where it came from in the camera's frame.
    let down = downs.iter().any(|d| *d);
    let mut arrows = 0;
    for t in taken.read() {
        let d = &t.damage;
        if d.target != me_e || !t.directed || d.amount <= 0.0 {
            continue;
        }
        let to = -d.dir;
        let sides = super::rules::arrow_sides([to.dot(*eye.right()), to.dot(*eye.forward()), to.dot(*eye.up())], down);
        hud.state.hit(sides, now);
        arrows += 1;
    }
    // A down-pointing hit whose `DamageTaken` came in another frame.
    if down && arrows == 0 {
        hud.state.hit(super::rules::arrow_sides([0.0; 3], true), now);
    }
    if !alive {
        hud.state.arrows = [None; 4];
    }
    let from = hud.state.tick.map_or(now, |t| (t + 1).max(now - 30));
    // The party's layout draws no tracker: its units are not followed.
    let others = if layout == super::layout::HudLayout::Party { Vec::new() } else { others };
    for tick in from..=now {
        hud.state.tick(data, &unit, &others, yaw, tick);
    }
    let canvas = window.iter().next().map_or(Canvas::ORIGINAL, |w| Canvas::of(w.width(), w.height())).with_layout(layout);
    // The sweep at the frame's own time (`draw::draw_blocks_at`).
    (hud.quads, hud.blocks) = draw::draw_blocks_at(data, &unit, &hud.state, &canvas, time.elapsed_secs_f64() * 30.0);
    hud.canvas = Some(canvas);
    hud.blur = draw::scope_blur(data, &unit, v);
    hud.unit = Some(unit);
}

/// A hit's damage effect has "indicators always point down" (hud.md 12).
pub fn points_down(v: &crate::game::fx::victim::HaloVictim, effects: &crate::game::vitality::DamageEffects) -> bool {
    use crate::game::vitality::data::flags::DAMAGE_INDICATORS_ALWAYS_POINT_DOWN;
    effects.at(v.effect).is_some_and(|(_, e)| e.has(DAMAGE_INDICATORS_ALWAYS_POINT_DOWN))
}

/// The quads as nodes under the root: one node per quad in draw order
/// (later on top), the extra ones hidden.
#[allow(clippy::too_many_arguments)]
pub fn sync(
    mut commands: Commands,
    hud: Res<HaloHud>,
    images: Res<HudImages>,
    root: Query<Entity, With<Root>>,
    window: Query<&Window, With<PrimaryWindow>>,
    materials: Option<ResMut<Assets<HudMaterial>>>,
    mut nodes: Query<(&mut Node, &mut Visibility, &MaterialNode<HudMaterial>), With<Picture>>,
    mut pool: Local<Vec<(Entity, HudMaterial)>>,
    filter: Option<Res<super::material::HudFilterSetting>>,
) {
    let filter = filter.map_or(0, |f| f.0);
    let (Some(mut materials), Ok(root)) = (materials, root.single()) else { return };
    let canvas = window.iter().next().map_or(Canvas::ORIGINAL, |w| Canvas::of(w.width(), w.height()));
    for (i, q) in hud.quads.iter().enumerate() {
        let image = images.0.get(&(q.picture.page, q.picture.image)).cloned().unwrap_or_default();
        let mask = match &q.fill {
            Fill::Sweep { mask: Some(m), .. } => images.0.get(&(m.page, m.image)).cloned().unwrap_or_default(),
            _ => Handle::default(),
        };
        let mut params = HudParams::of(&q.fill, q.picture.uv);
        if matches!(q.fill, Fill::Sprite(_) | Fill::Reticle(_) | Fill::Arrow { .. }) {
            params.style.x = f32::from(super::material::HudFilter::from_code(filter).code());
        }
        let want = HudMaterial { params, image, mask };
        let [left, top, width, height] = canvas.to_screen(q.rect);
        let node = Node { position_type: PositionType::Absolute, left: px(left), top: px(top), width: px(width), height: px(height), ..default() };
        if let Some((e, last)) = pool.get_mut(i) {
            let Ok((mut n, mut v, m)) = nodes.get_mut(*e) else { continue };
            if *n != node {
                *n = node;
            }
            v.set_if_neq(Visibility::Inherited);
            if last.params != want.params || last.image != want.image || last.mask != want.mask {
                if let Some(mut asset) = materials.get_mut(&m.0) {
                    *asset = want.clone();
                }
                *last = want;
            }
        } else {
            let handle = materials.add(want.clone());
            let e = commands.spawn((Picture(i), node, MaterialNode(handle), Pickable::IGNORE, ChildOf(root))).id();
            pool.push((e, want));
        }
    }
    for (e, _) in pool.iter().skip(hud.quads.len()) {
        if let Ok((_, mut v, _)) = nodes.get_mut(*e) {
            v.set_if_neq(Visibility::Hidden);
        }
    }
}

/// The scope's blur passes Halo asks of the engine (hud.md 9): two
/// passes of the radial blur through the mask's alpha, the mask one texel
/// per 480-line pixel centred on the view and r in 480-line pixels
/// (`radial_of`).
pub const SCOPE_PASSES: u32 = 2;

/// The engine's radial blur for a scope's blur and its mask's image.
pub fn radial_of(blur: &draw::ScopeBlur, mask: Handle<Image>) -> chimera_present::render::screen_post::RadialBlur {
    let lines = super::layout::CANVAS_HEIGHT as f32;
    chimera_present::render::screen_post::RadialBlur {
        mask,
        mask_texels_per_height: lines,
        radius: blur.radius / lines,
        centre: Vec2::splat(0.5),
        passes: SCOPE_PASSES,
    }
}

/// No scope blur while Halo's HUD is not the gameplay's (`collect` does
/// not run then).
pub fn clear_blur(mut hud: ResMut<HaloHud>) {
    if hud.blur.is_some() {
        hud.blur = None;
    }
}

/// The scope's blur to the engine's full-screen passes (Halo owns
/// `ScreenPost::radial`), or none.
pub fn scope_post(hud: Res<HaloHud>, images: Res<HudImages>, post: Option<ResMut<chimera_present::render::screen_post::ScreenPost>>) {
    let Some(mut post) = post else { return };
    let want = hud.blur.as_ref().and_then(|b| images.0.get(&(b.mask.page, b.mask.image)).map(|m| radial_of(b, m.clone())));
    if post.radial != want {
        post.radial = want;
    }
}
