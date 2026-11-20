//! What flashes and blasts do to the local player's senses
//! (specs/cs_source/grenades.md 5.5, 6.3, 6.4):
//! - the flash overlay: white at the blindness's alpha plus a frozen copy
//!   of the frame taken at the flash (the after-image), both added on top
//!   of the view and fading together (CS:S's `effects/flashbang_white` and
//!   `effects/flashbang`, which reads the full-frame copy; no texture to
//!   load);
//! - hearing: `core::Deafened` for the local player sets
//!   `audio::hearing::Hearing` (muffled sounds, ringing);
//! - the view shake of nearby blasts (`weapon::grenade::Shake`), and the
//!   view model's again at 10 % on top (`map::ViewModelShake`).
//!
//! What flashes and blasts do to the local player's senses: the flash overlay
//! (white plus the frame captured at the flash by a screenshot read-back, the
//! after-image, added in an additive UI material and fading with
//! `core::Blinded`), `core::Deafened` for the local player into
//! `audio::hearing::Hearing`, and the view shake of `weapon::grenade::Shake`s
//! in `Detonated` (applied in `follow_eye`).

use bevy::{
    asset::{RenderAssetUsages, embedded_asset},
    camera::RenderTarget,
    prelude::*,
    reflect::TypePath,
    render::{
        render_resource::{
            AsBindGroup, BlendComponent, BlendFactor, BlendOperation, BlendState, RenderPipelineDescriptor,
        },
        view::screenshot::{Screenshot, ScreenshotCaptured},
    },
    shader::ShaderRef,
};

use super::FirstPersonCamera;
use crate::{
    core::{Blinded, Deafened, LocalPlayer},
    audio::hearing::Hearing,
    weapon::grenade::{Detonated, Shake},
};

pub struct SensesPlugin;

impl Plugin for SensesPlugin {
    fn build(&self, app: &mut App) {
        embedded_asset!(app, "flash.wesl");
        app.add_plugins(UiMaterialPlugin::<FlashMaterial>::default())
            .init_resource::<AfterImage>()
            .init_resource::<FlashFade>()
            .init_resource::<ViewShake>()
            .init_resource::<crate::map::ViewModelShake>()
            // The map logic's shakes (Source's logic writes them; a game
            // without that logic has none, and `shake` still reads).
            .add_message::<crate::map::logic_view::MapShake>()
            .add_systems(Startup, spawn_overlay)
            .add_systems(Update, (flash_overlay, local_hearing, shake.before(super::follow_eye)))
            .add_systems(Update, fade_to_black);
    }
}

// ---------------------------------------------------------------------------
// mp_fadetoblack

/// Seconds the view takes to go black after death with `mp_fadetoblack 1`
/// (ours: unmeasured).
const FADE_TO_BLACK: f32 = 1.0;

#[derive(Component)]
struct BlackOverlay;

/// How black the view is `since` seconds after death (`mp_fadetoblack`).
pub fn black_alpha(since: Option<f32>) -> f32 {
    since.map_or(0.0, |s| (s / FADE_TO_BLACK).clamp(0.0, 1.0))
}

/// `mp_fadetoblack 1`: a dead player's view fades to black (under the
/// HUD) and stays black until it lives again; spectators and the living
/// see as usual.
fn fade_to_black(
    rule: Option<Res<crate::core::FadeToBlack>>,
    me: Option<Single<(Has<crate::core::Dead>, Has<crate::core::Spectating>), With<LocalPlayer>>>,
    overlay: Query<(Entity, &mut BackgroundColor), With<BlackOverlay>>,
    time: Res<Time>,
    mut died_at: Local<Option<f32>>,
    mut commands: Commands,
) {
    let dead = rule.is_some_and(|r| r.0 != 0) && me.is_some_and(|m| m.0 && !m.1);
    let now = time.elapsed_secs();
    *died_at = match (dead, *died_at) {
        (true, None) => Some(now),
        (true, at) => at,
        (false, _) => None,
    };
    let alpha = black_alpha(died_at.map(|t| now - t));
    let mut overlay = overlay;
    match overlay.iter_mut().next() {
        Some((e, mut bg)) => {
            if alpha <= 0.0 {
                commands.entity(e).despawn();
            } else {
                bg.set_if_neq(BackgroundColor(Color::srgba(0.0, 0.0, 0.0, alpha)));
            }
        }
        None if alpha > 0.0 => {
            commands.spawn((
                BlackOverlay,
                Node {
                    position_type: PositionType::Absolute,
                    width: percent(100.0),
                    height: percent(100.0),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.0, 0.0, 0.0, alpha)),
                Pickable::IGNORE,
                GlobalZIndex(32),
            ));
        }
        None => {}
    }
}

// ---------------------------------------------------------------------------
// The flash overlay and after-image

/// The flash overlay's material: white and the frozen frame, added.
#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
pub struct FlashMaterial {
    /// White amount, after-image amount (see flash.wesl).
    #[uniform(0)]
    params: Vec4,
    #[texture(1)]
    #[sampler(2)]
    frozen: Handle<Image>,
}

impl UiMaterial for FlashMaterial {
    fn fragment_shader() -> ShaderRef {
        "embedded://chimera_client/client/flash.wesl".into()
    }

    /// Additive, as the game's flash materials.
    fn specialize(descriptor: &mut RenderPipelineDescriptor, _key: UiMaterialKey<Self>) {
        let add = BlendComponent {
            src_factor: BlendFactor::One,
            dst_factor: BlendFactor::One,
            operation: BlendOperation::Add,
        };
        let keep = BlendComponent {
            src_factor: BlendFactor::Zero,
            dst_factor: BlendFactor::One,
            operation: BlendOperation::Add,
        };
        if let Some(fragment) = descriptor.fragment.as_mut() {
            for target in fragment.targets.iter_mut().flatten() {
                target.blend = Some(BlendState {
                    color: add,
                    alpha: keep,
                });
            }
        }
    }
}

#[derive(Component)]
struct FlashOverlay;

/// The frame captured at the latest flash, once read back, and the
/// blindness it belongs to (its end).
#[derive(Resource, Default)]
pub struct AfterImage {
    pub image: Option<Handle<Image>>,
    /// The end of the blindness the last capture was for.
    flash_end: f64,
    /// Captures asked for so far (a late read-back of an older one is
    /// dropped).
    asked: u32,
}

/// The overlay's (white, after-image) amounts at `now`: both the
/// blindness's alpha (spec 6.3: both added at the current flash alpha),
/// the after-image only once a frame was captured.
pub fn overlay_amounts(blind: Option<&Blinded>, now: f64, captured: bool) -> (f32, f32) {
    overlay_amounts_with(FlashFade::default(), blind, now, captured)
}

/// How a flash's white fades: the whiteness of a `Blinded` at `now`. The
/// default is the blindness's own linear fade (Counter-Strike: Source's);
/// a game with another curve (Combat Arms' cosine) inserts its own.
#[derive(Resource, Clone, Copy)]
pub struct FlashFade(pub fn(&Blinded, f64) -> f32);

impl Default for FlashFade {
    fn default() -> Self {
        Self(Blinded::alpha_at_now)
    }
}

/// `overlay_amounts` with a game's fade curve.
pub fn overlay_amounts_with(fade: FlashFade, blind: Option<&Blinded>, now: f64, captured: bool) -> (f32, f32) {
    let a = blind.map_or(0.0, |b| (fade.0)(b, now));
    (a, if captured { a } else { 0.0 })
}

fn spawn_overlay(mut commands: Commands, mut materials: ResMut<Assets<FlashMaterial>>) {
    commands.spawn((
        FlashOverlay,
        Node {
            position_type: PositionType::Absolute,
            width: percent(100.0),
            height: percent(100.0),
            ..default()
        },
        MaterialNode(materials.add(FlashMaterial {
            params: Vec4::ZERO,
            frozen: Handle::default(),
        })),
        Pickable::IGNORE,
        GlobalZIndex(FLASH_OVERLAY_Z),
    ));
}

/// The flash overlay's layer: over a HUD drawn below it (Combat Arms'
/// white covers its HUD), under chat (41 and up) and menus.
pub const FLASH_OVERLAY_Z: i32 = 31;

/// The captured frame as a texture: its bytes are what the view wrote
/// through an sRGB view, so read them as sRGB.
fn frozen_frame(mut image: Image) -> Image {
    let format = image.texture_descriptor.format;
    image.texture_descriptor.format = format.add_srgb_suffix();
    image.asset_usage = RenderAssetUsages::RENDER_WORLD;
    image
}

/// On a new flash, capture the frame (the view as it is before the white
/// shows: the overlay is left as it was for that frame); each frame, set
/// the overlay from the blindness.
#[allow(clippy::too_many_arguments)]
fn flash_overlay(
    local: Query<Ref<Blinded>, With<LocalPlayer>>,
    camera: Query<Option<&RenderTarget>, With<FirstPersonCamera>>,
    overlay: Query<&MaterialNode<FlashMaterial>, With<FlashOverlay>>,
    mut materials: ResMut<Assets<FlashMaterial>>,
    mut after: ResMut<AfterImage>,
    fade: Res<FlashFade>,
    time: Res<Time>,
    mut commands: Commands,
) {
    let now = time.elapsed_secs_f64();
    let blind = local.iter().next();
    if let Some(b) = &blind
        && b.is_changed()
        && b.end > after.flash_end + 1e-6
    {
        after.flash_end = b.end;
        after.asked += 1;
        let ask = after.asked;
        let target = match camera.iter().next().flatten() {
            Some(RenderTarget::Image(t)) => Screenshot::image(t.handle.clone()),
            _ => Screenshot::primary_window(),
        };
        commands.spawn(target).observe(
            move |shot: On<ScreenshotCaptured>, mut images: ResMut<Assets<Image>>, mut after: ResMut<AfterImage>| {
                if after.asked == ask {
                    after.image = Some(images.add(frozen_frame(shot.image.clone())));
                }
            },
        );
        return;
    }
    let (white, image) = overlay_amounts_with(*fade, blind.as_deref(), now, after.image.is_some());
    if white <= 0.0 && after.image.is_some() {
        after.image = None;
    }
    let Some(node) = overlay.iter().next() else { return };
    let params = Vec4::new(white, image, 0.0, 0.0);
    let frozen = after.image.clone().unwrap_or_default();
    if materials
        .get(&node.0)
        .is_some_and(|m| m.params != params || m.frozen != frozen)
        && let Some(mut m) = materials.get_mut(&node.0)
    {
        m.params = params;
        m.frozen = frozen;
    }
}

// ---------------------------------------------------------------------------
// Hearing

/// The local player's hearing effects go to the listener.
fn local_hearing(
    mut deafened: MessageReader<Deafened>,
    local: Option<Single<Entity, With<LocalPlayer>>>,
    mut hearing: ResMut<Hearing>,
    time: Res<Time>,
) {
    let me = local.map(|l| *l);
    let now = time.elapsed_secs_f64();
    for d in deafened.read() {
        if Some(d.target) == me {
            hearing.apply(d.effect, now);
        }
    }
}

// ---------------------------------------------------------------------------
// View shake

/// The view shakes running (centre, shake, start) and this frame's camera
/// offset (m) and roll (radians).
#[derive(Resource, Default)]
pub struct ViewShake {
    /// Centre, shake, start, from the map (env_shake).
    shakes: Vec<(Vec3, Shake, f64, bool)>,
    pub offset: Vec3,
    pub roll: f32,
}

/// Roll per metre of shake amplitude, radians (ours: 2 units roll the
/// view about half a degree).
const SHAKE_ROLL: f32 = 0.18;

/// The shake's displacement at `t` seconds for amplitude `a`: three
/// out-of-step sines at the shake's frequency (a smooth stand-in for
/// Source's random offset per period).
pub fn shake_offset(a: f32, frequency: f32, t: f32) -> (Vec3, f32) {
    let w = std::f32::consts::TAU * frequency * t;
    (
        Vec3::new((w).sin(), (w * 1.31 + 1.7).sin(), (w * 0.87 + 4.1).sin()) * a,
        (w * 0.53 + 2.3).sin() * a * SHAKE_ROLL,
    )
}

fn shake(
    mut detonated: MessageReader<Detonated>,
    mut map_shakes: MessageReader<crate::map::logic_view::MapShake>,
    camera: Query<&GlobalTransform, With<FirstPersonCamera>>,
    mut view: ResMut<ViewShake>,
    mut vm: ResMut<crate::map::ViewModelShake>,
    time: Res<Time>,
) {
    let now = time.elapsed_secs_f64();
    for d in detonated.read() {
        if let Some(s) = d.shake {
            view.shakes.push((d.at, s, now, false));
        }
    }
    // env_shake (amplitude 0: StopShake ends the map's shakes).
    for m in map_shakes.read() {
        if m.amplitude <= 0.0 {
            view.shakes.retain(|(.., from_map)| !from_map);
            continue;
        }
        view.shakes.push((
            m.at,
            Shake {
                amplitude: m.amplitude,
                frequency: m.frequency,
                duration: m.duration,
                radius: m.radius,
            },
            now,
            true,
        ));
    }
    view.shakes.retain(|(_, s, start, _)| now - start < s.duration as f64);
    let eye = camera.iter().next().map(|c| c.translation());
    let (mut offset, mut roll) = (Vec3::ZERO, 0.0);
    if let Some(eye) = eye {
        for (at, s, start, _) in &view.shakes {
            let t = (now - start) as f32;
            let (o, r) = shake_offset(s.amplitude_at(at.distance(eye), t), s.frequency, t);
            offset += o;
            roll += r;
        }
    }
    view.offset = offset;
    view.roll = roll;
    // The view model shakes again at 10 % in the view's frame (the
    // character's frame never turns: its offset is the world's).
    let turn = camera.iter().next().map_or(Quat::IDENTITY, |c| c.rotation());
    let again = crate::map::ViewModelShake {
        offset: turn.inverse() * offset * VIEW_MODEL_SHAKE,
        roll: roll * VIEW_MODEL_SHAKE,
    };
    vm.set_if_neq(again);
}

/// The view model's shake on top of the view's (Source's 10 %:
/// specs/cs_source/view_models.md 1, step 4).
pub const VIEW_MODEL_SHAKE: f32 = 0.1;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fade_to_black_over_a_second_after_death() {
        assert_eq!(black_alpha(None), 0.0, "alive");
        assert_eq!(black_alpha(Some(0.0)), 0.0);
        assert!((black_alpha(Some(0.5)) - 0.5).abs() < 1e-6);
        assert_eq!(black_alpha(Some(3.0)), 1.0, "then black");
    }

    #[test]
    fn overlay_holds_then_fades_with_its_after_image() {
        let b = Blinded {
            alpha: 1.0,
            fade_start: 12.0,
            end: 15.0,
        };
        assert_eq!(overlay_amounts(Some(&b), 10.0, false), (1.0, 0.0));
        assert_eq!(overlay_amounts(Some(&b), 11.0, true), (1.0, 1.0));
        let (w, i) = overlay_amounts(Some(&b), 13.5, true);
        assert!((w - 0.5).abs() < 1e-6 && w == i);
        assert_eq!(overlay_amounts(Some(&b), 15.0, true), (0.0, 0.0));
        assert_eq!(overlay_amounts(None, 13.0, true), (0.0, 0.0));
    }

    #[test]
    fn shake_falls_off_and_dies_out() {
        let s = Shake {
            amplitude: 0.05,
            frequency: 30.0,
            duration: 1.0,
            radius: 10.0,
        };
        assert_eq!(s.amplitude_at(0.0, 0.0), 0.05);
        assert!((s.amplitude_at(5.0, 0.5) - 0.0125).abs() < 1e-6);
        assert_eq!(s.amplitude_at(10.0, 0.0), 0.0);
        assert_eq!(s.amplitude_at(0.0, 1.0), 0.0);
        let (o, r) = shake_offset(0.05, 30.0, 0.0123);
        assert!(o.abs().max_element() <= 0.05 && r.abs() <= 0.05 * SHAKE_ROLL);
        assert_eq!(shake_offset(0.0, 30.0, 0.3), (Vec3::ZERO, 0.0));
    }
}
