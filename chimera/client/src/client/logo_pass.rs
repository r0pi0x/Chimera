//! The logo pass between screens (docs/plans/active/ugg-ui.md, 3 and M1),
//! for an app whose theme has one (`[layout.pass]`: the Ultimate Gun
//! Game's Signal); without one every move happens at once, as before.
//!
//! One transition for every screen change: four bands slanted at the
//! theme's angle sweep across (the kit's `PassMaterial`: an accent
//! hairline, the ink slab with the kit's `mark` icon, an accent band, a light
//! band) over `motion.dur.pass.total` (1.2 s); the screen swaps at
//! `swap` (0.6 s) while the slab covers the whole window (`covers`: at
//! 16:9, 21:9 and 4:3 alike), and the trailing edge reveals the next.
//! With reduced motion (`ui_reduce_motion 1`) it is a 150 ms fade through
//! ink, the swap at its middle.
//!
//! - `Passes::ask(key)` (a system parameter): a move through the pass.
//!   At the swap the pass triggers `PassCovered` with the keys asked, and
//!   each screen's observer makes its move. A pass already leading
//!   gathers the ask into its swap (a key asked twice is taken once: a
//!   system may ask every frame until it is done); one asked while the
//!   slab still covers after the swap is covered at once. `ask` returns
//!   false without the theme: the caller moves at once.
//! - The front end's moves go through it: the lobby coming up and going
//!   (`prematch_lobby`: it shows at a pass's swap and stays up until the
//!   next one's), the match end coming up and the next match after it
//!   (`match_end`: a pass that starts a swap's length before the change,
//!   so the slab covers it and the loading panel coming up), the party
//!   apps' menu, loading panel and game (`party_kit::main_menu`'s
//!   `ShownScreen`), and `ui_pass [p]` (console,
//!   a pass with nothing to swap; with `p`, held still at that progress
//!   until the next `ui_pass`: screenshots).
//! - The swap runs in `PreUpdate`, so a screen that comes up at it shows
//!   that frame.
//! - Its own root (`LogoPassRoot`) over everything (`PASS_Z`), moved
//!   with the lobby to whichever UI camera draws (the party menu's).
//!   `LogoPass::runs` and `swaps` count the passes (tests).
//!
//! Known shortcuts: a move asked once the slab has uncovered part of the
//! window starts a new pass from the left edge.

use bevy::{ecs::system::SystemParam, prelude::*, window::PrimaryWindow};
use chimera_core::console::{ConsoleAppExt, resource_cvar};
use chimera_ui::{IconImages, PassMaterial, Theme, UiClock, UiUnits};

/// Over every other screen and the console.
pub const PASS_Z: i32 = 1000;

/// The reduced-motion fade, seconds.
pub const FADE_SECS: f32 = 0.15;

struct Run {
    start: f64,
    swapped: bool,
    keys: Vec<&'static str>,
    fade: bool,
    /// Held at this many seconds in (`ui_pass <p>`, screenshots).
    held: Option<f32>,
}

/// The pass under way, if any.
#[derive(Resource, Default)]
pub struct LogoPass {
    run: Option<Run>,
    /// Passes begun (tests).
    pub runs: u32,
    /// Swaps done (tests).
    pub swaps: u32,
}

impl LogoPass {
    /// Seconds into the pass under way.
    pub fn elapsed(&self, now: f64) -> Option<f32> {
        self.run.as_ref().map(|r| r.held.unwrap_or((now - r.start) as f32))
    }

    /// A pass leads (its swap still to come).
    pub fn leading(&self) -> bool {
        self.run.as_ref().is_some_and(|r| !r.swapped)
    }

    /// Whether a leading pass will swap for `key`.
    pub fn pending(&self, key: &str) -> bool {
        self.run.as_ref().is_some_and(|r| !r.swapped && r.keys.contains(&key))
    }

    pub fn active(&self) -> bool {
        self.run.is_some()
    }

    /// The pass under way is the reduced-motion fade.
    pub fn fading(&self) -> bool {
        self.run.as_ref().is_some_and(|r| r.fade)
    }
}

/// The pass's swap: the slab covers the window; the screens asked for
/// (`keys`) make their moves.
#[derive(Event, Clone, Debug)]
pub struct PassCovered {
    pub keys: Vec<&'static str>,
}

impl PassCovered {
    pub fn has(&self, key: &str) -> bool {
        self.keys.contains(&key)
    }
}

/// `ui_reduce_motion`: the pass as a short fade.
#[derive(Resource, Clone, Copy, Debug, Default)]
pub struct ReduceMotion {
    pub on: u8,
}

/// The pass's root (module doc).
#[derive(Component)]
pub struct LogoPassRoot;

#[derive(Component)]
struct PassSweep;

#[derive(Component)]
struct PassFade;

/// Whether the theme draws a pass.
pub fn themed(theme: Option<&Theme>) -> bool {
    theme.is_some_and(|t| t.has_layout("pass"))
}

/// The pass's length and swap, seconds.
fn times(theme: &Theme, fade: bool) -> (f32, f32) {
    if fade { (FADE_SECS, FADE_SECS / 2.0) } else { (theme.dur("pass", "total").max(0.01), theme.dur("pass", "swap")) }
}

/// Asking for moves through the pass (module doc).
#[derive(SystemParam)]
pub struct Passes<'w, 's> {
    pass: Option<ResMut<'w, LogoPass>>,
    theme: Option<Res<'w, Theme>>,
    clock: Option<Res<'w, UiClock>>,
    reduce: Option<Res<'w, ReduceMotion>>,
    window: Query<'w, 's, &'static Window, With<PrimaryWindow>>,
    commands: Commands<'w, 's>,
}

impl Passes<'_, '_> {
    /// Moves go through the pass (the theme has one).
    pub fn on(&self) -> bool {
        self.pass.is_some() && themed(self.theme.as_deref())
    }

    /// Whether a leading pass will swap for `key`.
    pub fn pending(&self, key: &str) -> bool {
        self.pass.as_ref().is_some_and(|p| p.pending(key))
    }

    /// The pass's swap time, seconds (0 without the theme).
    pub fn swap_secs(&self) -> f32 {
        match &self.theme {
            Some(t) if self.on() => times(t, self.reduce.as_ref().is_some_and(|r| r.on != 0)).1,
            _ => 0.0,
        }
    }

    /// A move for `key` through the pass: true when it waits for a
    /// `PassCovered` with the key, false when the caller moves at once.
    pub fn ask(&mut self, key: &'static str) -> bool {
        if !self.on() {
            return false;
        }
        let (Some(theme), Some(lp)) = (self.theme.as_deref(), self.pass.as_deref_mut()) else { return false };
        let now = self.clock.as_ref().map_or(0.0, |c| c.now);
        let fade = self.reduce.as_ref().is_some_and(|r| r.on != 0);
        let aspect = self.window.iter().next().map_or(16.0 / 9.0, |w| w.width() / w.height().max(1.0));
        ask(lp, theme, &mut self.commands, key, now, fade, aspect);
        true
    }
}

fn ask(lp: &mut LogoPass, theme: &Theme, commands: &mut Commands, key: &'static str, now: f64, fade: bool, aspect: f32) {
    if let Some(r) = lp.run.as_mut() {
        if !r.swapped {
            if !r.keys.contains(&key) {
                r.keys.push(key);
            }
            return;
        }
        // Swapped: still covered, the move happens under the slab now.
        let (total, _) = times(theme, r.fade);
        let t = r.held.unwrap_or((now - r.start) as f32);
        if r.held.is_none() && !r.fade && covers(theme, t / total, aspect) {
            commands.trigger(PassCovered { keys: vec![key] });
            return;
        }
    }
    lp.runs += 1;
    lp.run = Some(Run { start: now, swapped: false, keys: vec![key], fade, held: None });
}

/// A pass held still at progress `p` (0-1) with nothing to swap, until
/// the next `ui_pass` (screenshots); none without the theme. A pass
/// leading swaps at once.
pub fn hold(world: &mut World, p: f32) {
    let Some(theme) = world.get_resource::<Theme>().filter(|t| themed(Some(t))) else { return };
    let fade = world.get_resource::<ReduceMotion>().is_some_and(|r| r.on != 0);
    let (total, _) = times(theme, fade);
    let now = world.get_resource::<UiClock>().map_or(0.0, |c| c.now);
    let Some(mut lp) = world.get_resource_mut::<LogoPass>() else { return };
    let due = lp.run.take().filter(|r| !r.swapped).map(|r| r.keys);
    lp.run = Some(Run { start: now, swapped: true, keys: Vec::new(), fade, held: Some(p.clamp(0.0, 1.0) * total) });
    if let Some(keys) = due {
        world.trigger(PassCovered { keys });
    }
}

/// A pass with nothing to swap (`ui_pass`); a held one lets go first.
pub fn run_empty(world: &mut World) {
    if !themed(world.get_resource::<Theme>()) {
        return;
    }
    let now = world.get_resource::<UiClock>().map_or(0.0, |c| c.now);
    let fade = world.get_resource::<ReduceMotion>().is_some_and(|r| r.on != 0);
    let Some(mut lp) = world.get_resource_mut::<LogoPass>() else { return };
    if lp.run.as_ref().is_some_and(|r| r.held.is_some()) {
        lp.run = None;
    }
    if lp.leading() {
        return;
    }
    lp.runs += 1;
    lp.run = Some(Run { start: now, swapped: false, keys: Vec::new(), fade, held: None });
}

fn hold_ease(p: f32) -> f32 {
    let q = 2.0 * p.clamp(0.0, 1.0) - 1.0;
    0.5 + 0.5 * q.signum() * q.abs().powi(3)
}

/// Whether the ink slab covers a window `aspect` (width / height) wide
/// at progress `p`, as `pass.wesl` draws it: every point's slanted
/// coordinate inside the slab.
pub fn covers(theme: &Theme, p: f32, aspect: f32) -> bool {
    const FRAME_H: f32 = 56.25;
    let l = |k: &str| theme.layout("pass", k);
    let wcq = aspect * FRAME_H;
    let s = (wcq / 100.0).max(1.0);
    let off = (wcq - 100.0 * s) * 0.5;
    let x0 = l("from") + l("travel") * hold_ease(p);
    let tan = theme.tan();
    // The slanted coordinate (measured at mid-height): its least (top
    // left) and most (bottom right).
    let lo = (0.0 - off - FRAME_H * 0.5 * tan) / s;
    let hi = (wcq - off + FRAME_H * 0.5 * tan) / s;
    lo >= x0 && hi <= x0 + l("band_ink_w")
}

/// Adds the pass, its cvar and `ui_pass`.
pub struct LogoPassPlugin;

impl Plugin for LogoPassPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<LogoPass>().init_resource::<ReduceMotion>();
        resource_cvar::<ReduceMotion, u8>(
            app,
            "ui_reduce_motion",
            "1: screen changes fade in 150 ms instead of the logo pass.",
            |r| &mut r.on,
        );
        app.console_command(
            "ui_pass",
            "ui_pass [p]: run the logo pass between screens once (nothing swaps); with p (0-1), hold it still there until the next ui_pass.",
            |w, a| {
                match a.first() {
                    Some(p) => hold(w, p.parse::<f32>().map_err(|_| format!("ui_pass: {p}: not a number"))?),
                    None => run_empty(w),
                }
                Ok(None)
            },
        );
        // The swap before the frame's screens follow their state, so a
        // screen that comes up at it shows that frame.
        app.add_systems(PreUpdate, step).add_systems(Update, draw);
    }
}

/// Triggers the swap at its time, ends the pass.
fn step(clock: Option<Res<UiClock>>, theme: Option<Res<Theme>>, mut lp: ResMut<LogoPass>, mut commands: Commands) {
    let now = clock.map_or(0.0, |c| c.now);
    let Some(theme) = theme else { return };
    let Some(r) = lp.run.as_mut().filter(|r| r.held.is_none()) else { return };
    let (total, swap) = times(&theme, r.fade);
    let t = (now - r.start) as f32;
    if !r.swapped && t >= swap {
        r.swapped = true;
        let keys = std::mem::take(&mut r.keys);
        lp.swaps += 1;
        commands.trigger(PassCovered { keys });
    }
    if lp.run.as_ref().is_some_and(|r| r.swapped && (now - r.start) as f32 >= total) {
        lp.run = None;
    }
}

#[derive(Default)]
struct Drawn {
    material: Option<Handle<PassMaterial>>,
    mark_h: u32,
}

#[allow(clippy::too_many_arguments)]
fn draw(
    lp: Res<LogoPass>,
    clock: Res<UiClock>,
    theme: Option<Res<Theme>>,
    units: Res<UiUnits>,
    mut icons: Option<ResMut<IconImages>>,
    mut root: Query<&mut Visibility, With<LogoPassRoot>>,
    mut sweep: Query<&mut Visibility, (With<PassSweep>, Without<LogoPassRoot>)>,
    mut fade: Query<(&mut Visibility, &mut BackgroundColor), (With<PassFade>, Without<LogoPassRoot>, Without<PassSweep>)>,
    mut materials: Option<ResMut<Assets<PassMaterial>>>,
    mut images: Option<ResMut<Assets<Image>>>,
    mut drawn: Local<Drawn>,
    mut commands: Commands,
) {
    let Some(theme) = theme.filter(|t| themed(Some(t))) else { return };
    let (Some(materials), Some(images)) = (materials.as_deref_mut(), images.as_deref_mut()) else { return };
    let Ok(mut vis) = root.single_mut() else {
        if lp.active() {
            // The root, made the first time a pass runs.
            let mat = materials.add(PassMaterial::new(&theme, None));
            drawn.material = Some(mat.clone());
            let full = || Node {
                position_type: PositionType::Absolute,
                left: px(0.0),
                top: px(0.0),
                width: percent(100.0),
                height: percent(100.0),
                ..default()
            };
            commands
                .spawn((LogoPassRoot, Visibility::Hidden, GlobalZIndex(PASS_Z), full()))
                .with_children(|r| {
                    r.spawn((PassSweep, MaterialNode(mat), full(), Visibility::Inherited));
                    r.spawn((PassFade, BackgroundColor(Color::NONE), full(), Visibility::Hidden));
                });
        }
        return;
    };
    let Some(t) = lp.elapsed(clock.now) else {
        vis.set_if_neq(Visibility::Hidden);
        return;
    };
    vis.set_if_neq(Visibility::Inherited);
    let fading = lp.fading();
    let (total, swap) = times(&theme, fading);
    if let Ok(mut s) = sweep.single_mut() {
        s.set_if_neq(if fading { Visibility::Hidden } else { Visibility::Inherited });
    }
    if let Ok((mut v, mut bg)) = fade.single_mut() {
        v.set_if_neq(if fading { Visibility::Inherited } else { Visibility::Hidden });
        if fading {
            let a = if t < swap { t / swap.max(1e-3) } else { 1.0 - (t - swap) / (total - swap).max(1e-3) };
            let c = theme.tone(chimera_ui::Tone::Ink).with_alpha(a.clamp(0.0, 1.0));
            if bg.0 != c {
                bg.0 = c;
            }
        }
    }
    if fading {
        return;
    }
    let Some(handle) = drawn.material.clone() else { return };
    // The mark at the size it shows, made again when the window's height changes.
    let mark_h = units.phys(units.cq(theme.layout("pass", "mark_h"))).round().max(1.0) as u32;
    let p = (t / total).clamp(0.0, 1.0);
    if let Some(mut m) = materials.get_mut(&handle) {
        if (drawn.mark_h != mark_h || m.mark.is_none())
            && let Some(img) = icons.as_deref_mut().and_then(|i| i.get(images, "mark", mark_h))
        {
            *m = PassMaterial::new(&theme, Some(img));
            drawn.mark_h = mark_h;
        }
        m.set_progress(p);
    }
}
