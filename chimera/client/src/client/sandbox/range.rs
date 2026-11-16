//! What the range shows over the world while a sandbox plays (the design
//! rules, sections 20 and 21: Q5 A, Q6 A; `sb2-dummies`, `sb2-blast`):
//!
//! - **The look panel**: only on the target under the crosshair
//!   (`link::LookPanel`), 1.4 cq to the right of its head with a leader
//!   to it: its name and game, the last hit's damage big, its shield (or
//!   gear) and health as bars with their numbers, the damage in all and,
//!   while its shield comes back, RECHARGING.
//! - **The respawn ring**: on a fallen target's spot
//!   (`link::TargetRow::respawn`, `home`), the kit's countdown ring
//!   (`RingMaterial`) emptying and the seconds left in it.
//! - **Blast rings** (on by default, `SandboxOptions::blast_rings`): after
//!   an explosion (`link::BlastMeasured`) rings round its centre: full
//!   damage inside the inner one (none when the falloff starts at the
//!   centre), the edge's damage at the outer, their radii in metres, and
//!   each body's damage where it stood; they fade after `BLAST_SECS`.
//!
//! Places come from the player's eye (`FirstPersonCamera`): a point
//! behind it or off screen hides its piece. A staging without a camera
//! puts them at fixed places on the screen (`STAGED_AT`).
//!
//! Known shortcuts: the rings are circles on the screen at the radius
//! seen edge-on (not ellipses on the floor); the panel doesn't avoid the
//! screen's edges; the pieces are spawned again each frame while any
//! shows (their ring materials pooled).

use bevy::prelude::*;
use chimera_ui::{Role, RingMaterial, Tone, UiParityId, UiStyle, controls::Ui};

use super::{
    SandboxScreen, SandboxUi,
    link::{BenchView, BlastMeasured, LookPanel, LookedAt, SandboxOptions},
};
use crate::client::{FirstPersonCamera, signal::Look};

/// How long blast rings stay, s.
pub const BLAST_SECS: f64 = 4.0;

/// Where a staging puts things without a camera (share of the screen).
pub const STAGED_AT: Vec2 = Vec2::new(0.42, 0.42);

/// Blasts seen lately (the UI clock's time, the blast).
#[derive(Resource, Clone, Debug, Default)]
pub struct Blasts(pub Vec<(f64, BlastMeasured)>);

#[derive(Component)]
struct RangeRoot;

fn collect(
    ui: Res<SandboxUi>,
    clock: Option<Res<chimera_ui::UiClock>>,
    options: Res<SandboxOptions>,
    mut seen: MessageReader<BlastMeasured>,
    mut blasts: ResMut<Blasts>,
) {
    let now = clock.map_or(0.0, |c| c.now);
    // A staging's blast stays.
    if ui.staged {
        seen.clear();
        return;
    }
    for b in seen.read() {
        if options.blast_rings {
            blasts.0.push((now, b.clone()));
        }
    }
    if blasts.0.iter().any(|(t, _)| now - t > BLAST_SECS) {
        blasts.0.retain(|(t, _)| now - t <= BLAST_SECS);
    }
}

/// A point on the screen, logical px (None: behind or off it).
fn project(cam: Option<(&Camera, &GlobalTransform)>, at: Vec3, size: Vec2) -> Option<Vec2> {
    match cam {
        Some((c, g)) => c.world_to_viewport(g, at).ok().filter(|p| p.x >= 0.0 && p.y >= 0.0 && p.x <= size.x && p.y <= size.y),
        None => Some(STAGED_AT * size),
    }
}

#[allow(clippy::too_many_arguments)]
fn draw(
    look: Look,
    ui_state: Res<SandboxUi>,
    panel: Res<LookPanel>,
    bench: Res<BenchView>,
    blasts: Res<Blasts>,
    clock: Option<Res<chimera_ui::UiClock>>,
    sim: Option<Res<crate::core::SimClock>>,
    camera: Query<(&Camera, &GlobalTransform), With<FirstPersonCamera>>,
    root: Query<Entity, With<RangeRoot>>,
    mut rings: ResMut<Assets<RingMaterial>>,
    mut pool: Local<Vec<Handle<RingMaterial>>>,
    mut c: Commands,
) {
    for r in &root {
        c.entity(r).despawn();
    }
    let on = look.on() && ui_state.on && ui_state.screen != SandboxScreen::ChangeGun;
    let staged = ui_state.staged;
    let any = panel.0.is_some() || !blasts.0.is_empty() || bench.targets.iter().any(|t| t.respawn.is_some());
    if !on || !any {
        return;
    }
    let size = Vec2::new(look.units.height * 16.0 / 9.0, look.units.height);
    // The rings' materials are reused frame to frame (a material made
    // and dropped each frame is never drawn).
    let mut used = 0usize;
    let cam = camera.iter().next().filter(|_| !staged);
    let ui = Ui::new(&look.theme, look.fonts.as_deref(), *look.units);
    let root = c
        .spawn((
            RangeRoot,
            Node { position_type: PositionType::Absolute, left: Val::ZERO, top: Val::ZERO, width: Val::Percent(100.0), height: Val::Percent(100.0), ..default() },
            GlobalZIndex(crate::client::minigame_scoreboard::SCOREBOARD_Z),
            Pickable::IGNORE,
        ))
        .id();
    if let Some(p) = &panel.0
        && let Some(at) = project(cam, p.top, size)
    {
        look_panel(&mut c, &ui, &look, root, p, at);
    }
    // Respawn rings.
    let now = sim.map_or(0.0, |s| s.now);
    for (i, t) in bench.targets.iter().enumerate() {
        let Some((at, total)) = t.respawn else { continue };
        let spot = match (cam, t.home) {
            (Some(_), Some(h)) => project(cam, h + Vec3::Y * 0.9, size),
            (None, _) => Some(STAGED_AT * size + Vec2::new(260.0 + 140.0 * i as f32, 90.0)),
            _ => None,
        };
        let Some(spot) = spot else { continue };
        let left = (at - now).max(0.0);
        let share = if staged { 0.62 } else { (left / total.max(0.01) as f64) as f32 };
        let d = look.px(1.5);
        let mut m = RingMaterial::new(look.theme.tone_a(Tone::Fg5, 1.0), look.theme.tone(Tone::Fg), (2.0 * look.units.scale_factor).round());
        m.set_share(share);
        let ring = c
            .spawn((
                Node { position_type: PositionType::Absolute, left: Val::Px(spot.x - d / 2.0), top: Val::Px(spot.y - d / 2.0), width: Val::Px(d), height: Val::Px(d), justify_content: JustifyContent::Center, align_items: AlignItems::Center, ..default() },
                MaterialNode(pooled(&mut rings, &mut pool, &mut used, m)),
                UiParityId::new(format!("range.respawn.{}", i + 1)),
                ChildOf(root),
            ))
            .id();
        let secs = if staged { 2 } else { left.ceil() as u32 };
        ui.text(&mut c, ring, Role::Mono, 0.6, Tone::Fg, &secs.to_string());
    }
    // Blast rings.
    let t_now = clock.map_or(0.0, |c| c.now);
    for (k, (t0, b)) in blasts.0.iter().enumerate() {
        let a = if staged { 1.0 } else { (1.0 - ((t_now - t0) / BLAST_SECS) as f32).clamp(0.0, 1.0) };
        let centre = if cam.is_none() { Some(STAGED_AT * size + Vec2::new(0.0, 120.0 + 60.0 * k as f32)) } else { project(cam, b.at, size) };
        let Some(centre) = centre else { continue };
        // A radius on the screen: the edge seen across the view.
        let radius = |r: f32| -> f32 {
            match cam {
                Some((_, g)) => project(cam, b.at + g.right() * r, size).map_or(0.0, |e| (e - centre).length()),
                None => r * 40.0,
            }
        };
        let circles = [
            (b.outer, Tone::Fg2, format!("{:.1} m \u{00b7} {:.0} at the edge", b.outer, b.edge_damage)),
            (b.inner, Tone::Fg, format!("{:.1} m \u{00b7} {:.0} full", b.inner, b.max_damage)),
        ];
        for (r, tone, words) in circles.into_iter().filter(|(r, ..)| *r > 0.05) {
            let px = radius(r).max(4.0);
            let m = RingMaterial::new(Color::srgba(1.0, 0.55, 0.24, 0.16 * a), Color::srgba(1.0, 0.67, 0.35, 0.9 * a), (2.0 * look.units.scale_factor).round());
            c.spawn((
                Node { position_type: PositionType::Absolute, left: Val::Px(centre.x - px), top: Val::Px(centre.y - px), width: Val::Px(px * 2.0), height: Val::Px(px * 2.0), ..default() },
                MaterialNode(pooled(&mut rings, &mut pool, &mut used, m)),
                UiParityId::new("range.blast"),
                ChildOf(root),
            ));
            let l = c
                .spawn((
                    UiStyle { gap: Some(0.4), align: Some(AlignItems::Baseline), pad: Some([0.3, 0.5, 0.26, 0.5].into()), ..UiStyle::row() }.bg_a(Tone::Ink, 0.78 * a),
                    Node { position_type: PositionType::Absolute, left: Val::Px(centre.x + px + 6.0), top: Val::Px(centre.y - 10.0), ..default() },
                    ChildOf(root),
                ))
                .id();
            ui.text_a(&mut c, l, Role::Mono, 0.4, tone, a, &words);
        }
        for (p, dmg) in &b.hits {
            let at = if cam.is_none() { Some(centre + Vec2::new(90.0, -30.0)) } else { project(cam, *p + Vec3::Y * 1.8, size) };
            if let Some(at) = at {
                let t = c
                    .spawn((Node { position_type: PositionType::Absolute, left: Val::Px(at.x), top: Val::Px(at.y), ..default() }, UiTransform { translation: Val2::percent(-50.0, 0.0), ..default() }, ChildOf(root)))
                    .id();
                ui.text_a(&mut c, t, Role::Mono, 0.9, Tone::Fg, a, &format!("{dmg:.0}"));
            }
        }
    }
}

/// The `used`-th pooled ring material, set to `m`.
fn pooled(rings: &mut Assets<RingMaterial>, pool: &mut Vec<Handle<RingMaterial>>, used: &mut usize, m: RingMaterial) -> Handle<RingMaterial> {
    if *used == pool.len() {
        pool.push(rings.add(m));
    } else if let Some(mut r) = rings.get_mut(&pool[*used]) {
        if *r != m {
            *r = m;
        }
    }
    *used += 1;
    pool[*used - 1].clone()
}

fn bar(c: &mut Commands, ui: &Ui, parent: Entity, label: &str, v: (f32, f32), tone: Tone) {
    let r = ui.boxed(c, parent, UiStyle { gap: Some(0.35), align: Some(AlignItems::Center), ..UiStyle::row() });
    let l = ui.boxed(c, r, UiStyle { w: Some(2.1), shrink: Some(0.0), ..UiStyle::row() });
    ui.text(c, l, Role::Mono, 0.38, Tone::Fg3, label);
    let track = ui.boxed(c, r, UiStyle { grow: Some(1.0), h: Some(0.38), ..UiStyle::row() }.bg(Tone::Fg5));
    let f = (v.0 / v.1.max(1e-3)).clamp(0.0, 1.0);
    c.spawn((Node { width: Val::Percent(f * 100.0), height: Val::Percent(100.0), ..default() }, BackgroundColor(ui.color(tone, 1.0)), ChildOf(track)));
    let n = ui.boxed(c, r, UiStyle { w: Some(1.1), justify: Some(JustifyContent::FlexEnd), shrink: Some(0.0), ..UiStyle::row() });
    ui.text(c, n, Role::Mono, 0.44, Tone::Fg, &format!("{:.0}", v.0));
}

fn look_panel(c: &mut Commands, ui: &Ui, look: &Look, root: Entity, p: &LookedAt, at: Vec2) {
    let lead = look.px(1.4);
    let anchor = c.spawn((Node { position_type: PositionType::Absolute, left: Val::Px(at.x), top: Val::Px(at.y), ..default() }, ChildOf(root))).id();
    // The leader: a line from the head to the panel, a dot at the head.
    c.spawn((Node { position_type: PositionType::Absolute, left: Val::ZERO, top: look.val(1.0), width: Val::Px(lead), height: Val::Px(look.hair()), ..default() }, BackgroundColor(look.theme.tone_a(Tone::Fg, 0.6)), ChildOf(anchor)));
    c.spawn((Node { position_type: PositionType::Absolute, left: look.val(-0.15), top: look.val(0.85), width: look.val(0.3), height: look.val(0.3), ..default() }, BackgroundColor(look.tone(Tone::Fg)), ChildOf(anchor)));
    let panel = c
        .spawn((
            UiStyle { w: Some(10.4), gap: Some(0.35), pad: Some([0.6, 0.7, 0.6, 0.7].into()), ..UiStyle::column() }.bg_a(Tone::Ink, 0.86).outline(Tone::Fg5, 1.0),
            Node { position_type: PositionType::Absolute, left: Val::Px(lead), top: Val::ZERO, ..default() },
            UiParityId::new("range.look"),
            ChildOf(anchor),
        ))
        .id();
    let h = ui.boxed(c, panel, UiStyle { gap: Some(0.5), align: Some(AlignItems::Baseline), ..UiStyle::row() });
    ui.text(c, h, Role::DisplayRegular, 0.68, Tone::Fg, &p.name);
    ui.text(c, h, Role::Mono, 0.4, Tone::Fg3, &p.game_short);
    let d = ui.boxed(c, panel, UiStyle { gap: Some(0.4), align: Some(AlignItems::Baseline), ..UiStyle::row() });
    ui.text(c, d, Role::Mono, 1.5, if p.last.is_some() { Tone::Fg } else { Tone::Fg4 }, &p.last.map_or("\u{2014}".into(), |v| format!("{v:.0}")));
    ui.text(c, d, Role::Mono, 0.44, Tone::Fg2, "Last hit");
    if let Some(s) = p.shield {
        bar(c, ui, panel, "Shield", s, Tone::Fg2);
    }
    bar(c, ui, panel, "Health", p.health, Tone::Fg);
    let f = ui.boxed(c, panel, UiStyle { justify: Some(JustifyContent::SpaceBetween), pad: Some([0.3, 0.0, 0.0, 0.0].into()), ..UiStyle::row() });
    c.spawn((Node { position_type: PositionType::Absolute, left: Val::ZERO, right: Val::ZERO, top: Val::ZERO, height: Val::Px(look.hair()), ..default() }, BackgroundColor(look.tone(Tone::Fg5)), ChildOf(f)));
    ui.text(c, f, Role::Mono, 0.38, Tone::Fg2, &format!("In all {:.0}", p.total));
    if p.recharging {
        ui.text(c, f, Role::Mono, 0.38, Tone::AccText, "Recharging");
    }
}

/// The range's systems.
pub fn add(app: &mut App) {
    app.init_resource::<Blasts>().add_systems(
        Update,
        (collect, draw)
            .chain()
            .after(super::SandboxSystems)
            .run_if(resource_exists::<chimera_ui::Theme>)
            .run_if(resource_exists::<chimera_ui::UiUnits>)
            .run_if(resource_exists::<Assets<RingMaterial>>),
    );
}
