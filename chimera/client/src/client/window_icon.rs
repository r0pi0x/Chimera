//! The window's icon (the taskbar's, the title bar's): an app lists square
//! PNGs (`GameAppHooks::window_icons`), one is drawn at random at each
//! start and set on the primary window through winit once the window
//! exists. A file that doesn't decode is skipped (the system's icon stays).
//! The Windows executable's own icon (Explorer, a pinned taskbar) is a
//! resource its app's build.rs embeds, not this.

use bevy::prelude::*;
use winit::window::Icon;

/// A PNG as a winit icon: None unless it decodes to a square image.
pub fn decode(png: &[u8]) -> Option<Icon> {
    let img = image::load_from_memory_with_format(png, image::ImageFormat::Png).ok()?.into_rgba8();
    let (w, h) = img.dimensions();
    if w != h {
        return None;
    }
    Icon::from_rgba(img.into_raw(), w, h).ok()
}

/// Sets one of `pngs`, drawn at random, on the primary window once it
/// exists (nothing for an empty list).
pub fn set_at_start(app: &mut App, pngs: &'static [&'static [u8]]) {
    if pngs.is_empty() {
        return;
    }
    let pick = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.subsec_nanos() as usize)
        % pngs.len();
    app.add_systems(Update, move |mut done: Local<bool>| {
        if *done {
            return;
        }
        bevy::winit::WINIT_WINDOWS.with_borrow(|windows| {
            for window in windows.windows.values() {
                *done = true;
                if let Some(icon) = decode(pngs[pick]) {
                    window.set_window_icon(Some(icon));
                }
            }
        });
    });
}
