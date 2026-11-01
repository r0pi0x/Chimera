//! The video settings the options dialogs set (the game's dialog, its
//! rows: `games::cs_source::ui::options`): `mat_vsync`, `chimera_fullscreen`
//! and `chimera_resolution` (the window), `mat_antialias` (MSAA), texture
//! detail and filtering (`mat_picmip`, `mat_trilinear`, `mat_forceaniso`)
//! and the player's field of view (`fov_desired`).
//!
//! `fov_desired` (`PlayerFov`: the world camera's unzoomed field of view, 75
//! to 90, CS:S's 90); the video cvars (`mat_vsync`, `chimera_fullscreen`,
//! `chimera_resolution`, `mat_antialias`; archived) set the window's present
//! mode, mode and size when they change (not in automated runs, and only the
//! present mode given `--window`;
//! `window_mode`: full screen in the primary monitor's mode of that size,
//! else of its own size, else borderless; the size only for a window) and
//! every camera's MSAA; the options' video tab shows the window as it is
//! (`resolutions` lists its size with the monitor's modes, `mode_value`); the texture settings (`TextureSettings`:
//! `mat_picmip`, `mat_trilinear`, `mat_forceaniso`) each repeating, mipmapped
//! texture takes as it is added (`apply_texture_settings`, before it goes to
//! the GPU; so from the next map load).

use bevy::{
    prelude::*,
    window::{MonitorSelection, PresentMode, PrimaryMonitor, PrimaryWindow, VideoMode, VideoModeSelection, WindowMode},
};

use crate::console::{Console, ConsoleAppExt, resource_cvar};

pub struct VideoPlugin;

impl Plugin for VideoPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<VideoSettings>()
            .init_resource::<PlayerFov>()
            .add_systems(Update, apply_video)
            .add_systems(PostUpdate, apply_msaa);
        resource_cvar::<VideoSettings, u8>(
            app,
            "mat_vsync",
            "1: wait for vertical sync (no tearing, frames capped at the refresh rate).",
            |v| &mut v.vsync,
        );
        resource_cvar::<VideoSettings, u8>(
            app,
            "chimera:fullscreen",
            "Display mode: 0 windowed, 1 full screen, 2 borderless full screen.",
            |v| &mut v.fullscreen,
        );
        resource_cvar::<VideoSettings, String>(
            app,
            "chimera:resolution",
            "Window size in pixels, e.g. 1920x1080 (empty: as started).",
            |v| &mut v.resolution,
        );
        resource_cvar::<VideoSettings, u8>(
            app,
            "mat_antialias",
            "Multisample antialiasing: 0 off, 2 or 4 samples per pixel (4 by default). Off saves GPU time \
             at high resolutions (each sample is drawn and resolved for every camera).",
            |v| &mut v.antialias,
        );
        app.console_cvar(
            "fov_desired",
            "The player's field of view, 75 to 90 degrees (horizontal at 4:3); zoomed scopes keep theirs.",
            "90",
            |w| w.get_resource::<PlayerFov>().map(|f| f.0.to_string()),
            |w, v| {
                let v: f32 = v.trim().parse().map_err(|_| format!("bad value \"{v}\""))?;
                w.get_resource_mut::<PlayerFov>().ok_or("not available")?.0 = v.clamp(FOV_RANGE.0, FOV_RANGE.1);
                Ok(())
            },
        );
        app.init_resource::<crate::render::rope_sway::RopeWind>();
        resource_cvar::<crate::render::rope_sway::RopeWind, f32>(
            app,
            "rope_wind_dist",
            "Ropes sway in the wind while the camera is within this many units of them (0: never).",
            |w| &mut w.dist,
        );
        app.init_resource::<crate::render::lod::ModelSettings>();
        resource_cvar::<crate::render::lod::ModelSettings, u8>(
            app,
            "r_rootlod",
            "Model detail: the finest level of detail models draw, 0 high, 1 medium, 2 low (models with fewer \
             levels draw their coarsest). Applies at once.",
            |s| &mut s.root_lod,
        );
        resource_cvar::<crate::render::lod::ModelSettings, i32>(
            app,
            "r_lod",
            "-1: models draw the level of detail their size on screen asks for; n: always level n.",
            |s| &mut s.lod,
        );
        app.init_resource::<crate::render::shadows::ShadowSettings>();
        resource_cvar::<crate::render::shadows::ShadowSettings, u8>(
            app,
            "r_shadows",
            "0: no dynamic shadows (props' and characters').",
            |s| &mut s.shadows,
        );
        resource_cvar::<crate::render::shadows::ShadowSettings, u8>(
            app,
            "r_shadowrendertotexture",
            "1: props cast their silhouette (render-to-texture shadows); 0: blob shadows, a round patch straight \
             down (shadow detail Low). Applies at once.",
            |s| &mut s.render_to_texture,
        );
        resource_cvar::<crate::render::shadows::ShadowSettings, u8>(
            app,
            "r_flashlightdepthtexture",
            "1: flashlights cast depth shadows (shadow detail High; CS:S has no flashlights: as Medium).",
            |s| &mut s.flashlight_depth,
        );
        app.init_resource::<TextureSettings>().add_systems(
            PostUpdate,
            apply_texture_settings
                .after(bevy::asset::AssetEventSystems)
                .run_if(resource_exists::<Assets<Image>>),
        );
        resource_cvar::<TextureSettings, i32>(
            app,
            "mat_picmip",
            "Texture detail: 2 low, 1 medium, 0 high, -1 very high (as high: no larger textures). Each level skips \
             a texture's largest mip. Applies from the next map load.",
            |t| &mut t.picmip,
        );
        resource_cvar::<TextureSettings, u8>(
            app,
            "mat_trilinear",
            "1: blend between mip levels (trilinear filtering). Applies from the next map load.",
            |t| &mut t.trilinear,
        );
        resource_cvar::<TextureSettings, u16>(
            app,
            "mat_forceaniso",
            "Anisotropic filtering: 1 off, 2, 4, 8 or 16 samples. Applies from the next map load.",
            |t| &mut t.aniso,
        );
        let mut console = app.world_mut().resource_mut::<Console>();
        for name in [
            "mat_vsync",
            "chimera_fullscreen",
            "chimera_resolution",
            "mat_antialias",
            "fov_desired",
            "mat_picmip",
            "mat_trilinear",
            "mat_forceaniso",
            "r_rootlod",
            "r_shadows",
            "r_shadowrendertotexture",
            "r_flashlightdepthtexture",
        ] {
            console.archive(name);
        }
    }
}

/// Video > Advanced's texture detail and filtering (`mat_picmip`,
/// `mat_trilinear`, `mat_forceaniso`; CS:S's defaults: high, bilinear).
/// Each map texture takes them as it loads (`apply_texture_settings`):
/// the textures live only on the GPU once drawn, so a change shows from
/// the next map load (CS:S's applies at once).
#[derive(Resource, Clone, Copy, Debug, PartialEq)]
pub struct TextureSettings {
    pub picmip: i32,
    pub trilinear: u8,
    pub aniso: u16,
}

impl Default for TextureSettings {
    fn default() -> Self {
        Self {
            picmip: 0,
            trilinear: 0,
            aniso: 1,
        }
    }
}

impl TextureSettings {
    /// Set a map texture's sampler: repeating, mipmapped and filtered
    /// (the world's, models', water's; not the HUD's or UI's pictures,
    /// which clamp, or pixel-art ones, which don't filter). Returns
    /// whether it is one.
    pub fn apply(&self, image: &mut Image) -> bool {
        use bevy::image::{ImageAddressMode, ImageFilterMode, ImageSampler};
        let levels = image.texture_descriptor.mip_level_count;
        let ImageSampler::Descriptor(d) = &mut image.sampler else {
            return false;
        };
        if levels < 2
            || d.address_mode_u != ImageAddressMode::Repeat
            || d.mag_filter != ImageFilterMode::Linear
            || d.min_filter != ImageFilterMode::Linear
        {
            return false;
        }
        let aniso = self.aniso.clamp(1, 16);
        // The GPU filters anisotropically only with every filter linear.
        d.mipmap_filter = if self.trilinear != 0 || aniso > 1 {
            ImageFilterMode::Linear
        } else {
            ImageFilterMode::Nearest
        };
        d.anisotropy_clamp = aniso;
        // Low and medium skip the largest one or two levels (never the
        // last).
        d.lod_min_clamp = self.picmip.clamp(0, levels as i32 - 1) as f32;
        true
    }
}

/// Each texture added this frame takes `TextureSettings` before it goes
/// to the GPU (the render world takes it at the end of the frame).
fn apply_texture_settings(
    settings: Res<TextureSettings>,
    mut events: MessageReader<AssetEvent<Image>>,
    mut images: ResMut<Assets<Image>>,
) {
    for event in events.read() {
        if let AssetEvent::Added { id } = event
            && let Some(image) = images.get_mut_untracked(*id)
        {
            settings.apply(image);
        }
    }
}

/// `fov_desired`: the player's unzoomed field of view, degrees
/// horizontal at 4:3 (CS:S's 90; the video Advanced dialog's slider takes
/// 75 to 90). The world camera uses it unzoomed (`client::zoom_camera`);
/// the view model zooms with it (`map::view_model::view_model_fov`); a
/// scope's zoom is its own.
#[derive(Resource, Clone, Copy, Debug, PartialEq)]
pub struct PlayerFov(pub f32);

impl Default for PlayerFov {
    fn default() -> Self {
        Self(90.0)
    }
}

/// `fov_desired`'s bounds (CS:S's slider's, `OptionsSubVideoAdvancedDlg.res`).
pub const FOV_RANGE: (f32, f32) = (75.0, 90.0);

/// The window's mode, size and vsync, as the options set them.
#[derive(Resource, Clone, Debug, PartialEq)]
pub struct VideoSettings {
    pub vsync: u8,
    pub fullscreen: u8,
    pub resolution: String,
    /// MSAA samples (`mat_antialias`).
    pub antialias: u8,
}

impl Default for VideoSettings {
    fn default() -> Self {
        Self {
            vsync: 1,
            fullscreen: 0,
            resolution: String::new(),
            antialias: 4,
        }
    }
}

impl VideoSettings {
    /// `mat_antialias` as Bevy's MSAA setting: 0 or 1 off, 2, anything
    /// higher 4 (8 isn't supported everywhere).
    pub fn msaa(&self) -> Msaa {
        match self.antialias {
            0 | 1 => Msaa::Off,
            2 | 3 => Msaa::Sample2,
            _ => Msaa::Sample4,
        }
    }
}

/// Every camera (the main view, sky, view model, water reflection and the
/// HUD's: the ones drawing into one target should agree, or each switch
/// costs a full-screen copy) takes `mat_antialias`.
fn apply_msaa(
    settings: Res<VideoSettings>,
    cameras: Query<(Entity, Option<&Msaa>), With<Camera>>,
    mut commands: Commands,
) {
    let want = settings.msaa();
    for (e, msaa) in &cameras {
        if msaa != Some(&want) {
            commands.entity(e).insert(want);
        }
    }
}

/// `1920x1080` -> (1920, 1080).
pub fn parse_resolution(s: &str) -> Option<UVec2> {
    let (w, h) = s.trim().split_once(['x', 'X'])?;
    let size = UVec2::new(w.trim().parse().ok()?, h.trim().parse().ok()?);
    (size.x >= 320 && size.y >= 240).then_some(size)
}

/// The window sizes to offer: the primary monitor's modes, largest
/// first, else common ones; and the window's own size (`current`) when
/// it isn't one of them, as CS:S lists the display's modes plus the one
/// it runs at.
pub fn resolutions(monitors: &[(UVec2, Vec<UVec2>)], current: Option<UVec2>) -> Vec<String> {
    let mut sizes: Vec<UVec2> = monitors.iter().flat_map(|(_, modes)| modes.iter().copied()).collect();
    if sizes.is_empty() {
        sizes = [
            (800, 600),
            (1024, 768),
            (1280, 720),
            (1280, 1024),
            (1366, 768),
            (1600, 900),
            (1920, 1080),
            (2560, 1440),
        ]
        .map(|(w, h)| UVec2::new(w, h))
        .to_vec();
        if let Some((size, _)) = monitors.first() {
            sizes.retain(|s| s.x <= size.x && s.y <= size.y);
            sizes.push(*size);
        }
    }
    sizes.retain(|s| s.x >= 640 && s.y >= 480);
    sizes.extend(current.filter(|c| c.x > 0 && c.y > 0));
    sizes.sort_by_key(|s| (std::cmp::Reverse(s.x), std::cmp::Reverse(s.y)));
    sizes.dedup();
    sizes.iter().map(|s| format!("{}x{}", s.x, s.y)).collect()
}

/// The primary monitor's size and modes (for `resolutions`).
pub fn monitor_modes(monitors: &Query<&bevy::window::Monitor, With<PrimaryMonitor>>) -> Vec<(UVec2, Vec<UVec2>)> {
    monitors
        .iter()
        .map(|m| {
            (
                UVec2::new(m.physical_width, m.physical_height),
                m.video_modes.iter().map(|v| v.physical_size).collect(),
            )
        })
        .collect()
}

/// `chimera_fullscreen`'s value for a window mode: 0 windowed, 1 full
/// screen, 2 borderless full screen.
pub fn mode_value(mode: &WindowMode) -> u8 {
    match mode {
        WindowMode::Windowed => 0,
        WindowMode::Fullscreen(..) => 1,
        WindowMode::BorderlessFullscreen(_) => 2,
    }
}

/// The window mode `chimera_fullscreen` asks for: 0 windowed, 2 borderless
/// on the window's monitor, 1 full screen in the primary monitor's mode
/// of `size` (else of the monitor's own size; the highest refresh rate);
/// borderless when the monitor has neither (Bevy would warn and leave
/// the window as it was). `monitor`: its size and modes, when known.
pub fn window_mode(fullscreen: u8, size: Option<UVec2>, monitor: Option<(UVec2, &[VideoMode])>) -> WindowMode {
    match fullscreen {
        1 => {
            let Some((native, modes)) = monitor else {
                return WindowMode::Fullscreen(MonitorSelection::Primary, VideoModeSelection::Current);
            };
            let best = |want: UVec2| {
                modes
                    .iter()
                    .filter(|m| m.physical_size == want)
                    .max_by_key(|m| (m.refresh_rate_millihertz, m.bit_depth))
                    .copied()
            };
            match size.and_then(best).or_else(|| best(native)) {
                Some(m) => WindowMode::Fullscreen(MonitorSelection::Primary, VideoModeSelection::Specific(m)),
                None => WindowMode::BorderlessFullscreen(MonitorSelection::Current),
            }
        }
        2 => WindowMode::BorderlessFullscreen(MonitorSelection::Current),
        _ => WindowMode::Windowed,
    }
}

/// Set the window as the video cvars say, when they change. Automated runs
/// (screenshots, captures) leave the window alone; given a window size
/// (`--window`), only vsync applies.
fn apply_video(
    settings: Res<VideoSettings>,
    args: Option<Res<crate::client::ClientArgs>>,
    mut window: Query<&mut Window, With<PrimaryWindow>>,
    monitors: Query<&bevy::window::Monitor, With<PrimaryMonitor>>,
) {
    if !settings.is_changed() {
        return;
    }
    if args.as_ref().is_some_and(|a| a.0.screenshot.is_some() || a.0.views.is_some()) {
        return;
    }
    let Ok(mut window) = window.single_mut() else { return };
    let present = if settings.vsync != 0 {
        PresentMode::AutoVsync
    } else {
        PresentMode::AutoNoVsync
    };
    if window.present_mode != present {
        window.present_mode = present;
    }
    // A window size given (`--window`) keeps its size and mode, but
    // `mat_vsync 0` still uncaps it (profiling runs time frames that way).
    if args.is_some_and(|a| a.0.window.is_some()) {
        return;
    }
    let size = parse_resolution(&settings.resolution);
    let monitor = monitors.iter().next();
    let mode = window_mode(
        settings.fullscreen,
        size,
        monitor.map(|m| (UVec2::new(m.physical_width, m.physical_height), m.video_modes.as_slice())),
    );
    if window.mode != mode {
        window.mode = mode;
    }
    if matches!(mode, WindowMode::Windowed)
        && let Some(size) = size
        && window.resolution.physical_size() != size
    {
        window.resolution.set_physical_resolution(size.x, size.y);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Each display mode gives exactly that mode: full screen in the
    /// monitor's mode of the size asked for (its fastest refresh), else of
    /// the monitor's own size, else borderless rather than nothing; and a
    /// window's mode reads back as the same value.
    #[test]
    fn display_modes_give_exactly_that_mode() {
        let mode = |w, h, hz| VideoMode {
            physical_size: UVec2::new(w, h),
            bit_depth: 32,
            refresh_rate_millihertz: hz,
        };
        let modes = [mode(1920, 1080, 60_000), mode(1920, 1080, 144_000), mode(1280, 720, 60_000)];
        let native = UVec2::new(1920, 1080);
        let monitor = Some((native, &modes[..]));
        assert_eq!(window_mode(0, Some(native), monitor), WindowMode::Windowed);
        assert_eq!(
            window_mode(2, None, monitor),
            WindowMode::BorderlessFullscreen(MonitorSelection::Current)
        );
        let full = |m| WindowMode::Fullscreen(MonitorSelection::Primary, VideoModeSelection::Specific(m));
        assert_eq!(window_mode(1, Some(UVec2::new(1280, 720)), monitor), full(modes[2]));
        assert_eq!(window_mode(1, None, monitor), full(modes[1]), "as started: the monitor's size, fastest");
        assert_eq!(window_mode(1, Some(UVec2::new(1000, 700)), monitor), full(modes[1]));
        assert_eq!(
            window_mode(1, None, Some((native, &[]))),
            WindowMode::BorderlessFullscreen(MonitorSelection::Current)
        );
        for v in 0..=2 {
            assert_eq!(mode_value(&window_mode(v, None, monitor)), v);
        }
    }

    /// Texture detail and filtering set a map texture's sampler; pictures
    /// that clamp (HUD, UI) or have no mips are left alone.
    #[test]
    fn texture_settings_set_map_samplers() {
        use bevy::image::{ImageAddressMode, ImageFilterMode, ImageSampler, ImageSamplerDescriptor};
        let texture = |address, levels| {
            let mut image = Image::default();
            image.texture_descriptor.mip_level_count = levels;
            image.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor {
                address_mode_u: address,
                address_mode_v: address,
                mipmap_filter: ImageFilterMode::Linear,
                ..ImageSamplerDescriptor::linear()
            });
            image
        };
        let sampler = |image: &Image| match &image.sampler {
            ImageSampler::Descriptor(d) => (d.mipmap_filter, d.anisotropy_clamp, d.lod_min_clamp),
            _ => panic!(),
        };
        // CS:S's defaults: bilinear (nearest mip), no anisotropy, full size.
        let mut map = texture(ImageAddressMode::Repeat, 8);
        assert!(TextureSettings::default().apply(&mut map));
        assert_eq!(sampler(&map), (ImageFilterMode::Nearest, 1, 0.0));
        // Low detail, anisotropic 8x (which needs the mip filter linear).
        let low = TextureSettings {
            picmip: 2,
            trilinear: 0,
            aniso: 8,
        };
        assert!(low.apply(&mut map));
        assert_eq!(sampler(&map), (ImageFilterMode::Linear, 8, 2.0));
        // Never past the last mip.
        let mut small = texture(ImageAddressMode::Repeat, 2);
        TextureSettings { picmip: 2, ..default() }.apply(&mut small);
        assert_eq!(sampler(&small).2, 1.0);
        // A HUD picture (clamped) and one without mips: untouched.
        let mut hud = texture(ImageAddressMode::ClampToEdge, 8);
        assert!(!low.apply(&mut hud));
        assert_eq!(sampler(&hud), (ImageFilterMode::Linear, 1, 0.0));
        assert!(!low.apply(&mut texture(ImageAddressMode::Repeat, 1)));
    }
}
