//! What the range shows over the world while a sandbox plays (the design
//! rules, sections 20 and 21: Q5 A, Q6 A; `sb2-dummies`, `sb2-blast`):
//!
//! - **The look panel**: only on the target under the crosshair
//!   (`link::LookPanel`), beside the region its body takes on the screen
//!   (its hull's box projected, read from this frame's places: the eye as
//!   placed this frame, the body as drawn), at least 1.4 cq to the right
//!   of its head, with a leader to the head: it never covers the body and
//!   moves with it at the frame rate (playtest 2026-11-17: it trailed the
//!   view by a frame, from last frame's transforms). Its name and game,
//!   the last hit's damage big, its shield (or
//!   gear) and health as bars with their numbers, the damage in all and,
//!   while its shield comes back, RECHARGING.
//! - **The respawn ring**: on a fallen target's spot
//!   (`link::TargetRow::respawn`, `home`), the kit's countdown ring
//!   (`RingMaterial`) emptying and the seconds left in it.
//! - **Blast spheres** (on by default, `SandboxOptions::blast_rings`):
//!   after an explosion (`link::BlastMeasured`) translucent spheres in the
//!   world round its centre (`BlastSphere`, meshes: steady from any view,
//!   playtest 2026-11-17: the screen circles flickered): full damage
//!   inside the inner one (none when the falloff starts at the centre),
//!   the edge's damage at the outer, with labels of their radii in metres
//!   beside them on the screen, and each body's damage where it stood;
//!   they fade after `BLAST_SECS`.
//!
//! Places come from the player's eye (`FirstPersonCamera`): a point
//! behind it or off screen hides its piece. A staging without a camera
//! puts them at fixed places on the screen (`STAGED_AT`). None of them
//! under `cl_drawhud 0` (`hud::DRAWHUD_COVER`: captures against the real
//! games).
//!
//! The pieces are spawned when what they say changes (`Shape`: the
//! panel's numbers, a ring's seconds, a blast's words, a piece coming on
//! or off screen) and otherwise rebound in place each frame: their places,
//! a blast's fade, a ring's share (its pooled material). `RangeBuilds`
//! counts the spawns. The staged range (a panel, a ring, a blast) held
//! still costs about 2.7 M instructions over 120 frames, from 107 M when
//! every piece was spawned again each frame (chimera_client's
//! `sandbox::the_range_readouts_rebind_in_place_instead_of_spawning_each_frame`).
//!
//! Known shortcuts: the panel doesn't avoid the screen's edges; the
//! body's region is its hull's box, not the drawn mesh's bounds.

use bevy::prelude::*;
use chimera_ui::{Role, RingMaterial, Tone, UiParityId, UiStyle, UiText, controls::Ui};

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

/// The head's top above the hull's (m): heads stand out of the hulls.
pub const HEAD_ABOVE_HULL: f32 = 0.15;

/// Room kept round the body's box (m): limbs and guns out of the hull.
pub const BODY_MARGIN: f32 = 0.15;

/// The screen's rectangle (logical px) round a world box `lo`..`hi`
/// (None: a corner is behind the eye, or no camera).
pub fn screen_region(cam: Option<(&Camera, &GlobalTransform)>, lo: Vec3, hi: Vec3) -> Option<Rect> {
    let (c, g) = cam?;
    let mut r: Option<Rect> = None;
    for i in 0..8 {
        let p = Vec3::new(if i & 1 == 0 { lo.x } else { hi.x }, if i & 2 == 0 { lo.y } else { hi.y }, if i & 4 == 0 { lo.z } else { hi.z });
        let s = c.world_to_viewport(g, p).ok()?;
        r = Some(r.map_or(Rect::from_corners(s, s), |r| r.union_point(s)));
    }
    r
}

/// A point on the screen, logical px (None: behind or off it).
fn project(cam: Option<(&Camera, &GlobalTransform)>, at: Vec3, size: Vec2) -> Option<Vec2> {
    match cam {
        Some((c, g)) => c.world_to_viewport(g, at).ok().filter(|p| p.x >= 0.0 && p.y >= 0.0 && p.x <= size.x && p.y <= size.y),
        None => Some(STAGED_AT * size),
    }
}

/// What the pieces say, not where they are: a change spawns them again,
/// a move (the eye, a fade, a ring emptying) rebinds them in place.
#[derive(Clone, Debug, Default, PartialEq)]
struct Shape {
    /// The look panel's target (its places zeroed), when on screen.
    panel: Option<LookedAt>,
    /// Each respawn ring on screen: the target's index, the seconds shown.
    rings: Vec<(usize, u32)>,
    /// Each blast on screen: its circles' words, and each hit's words
    /// (None: off screen).
    blasts: Vec<(Vec<(String, Tone)>, Vec<Option<String>>)>,
}

/// Where the pieces go this frame (logical px), and how faded.
#[derive(Default)]
struct Places {
    anchor: Option<Vec2>,
    /// The panel's distance right of the anchor (px): past the body.
    lead: f32,
    /// Each ring's spot and its material.
    rings: Vec<(Vec2, RingMaterial)>,
    /// Each blast: its centre, its alpha, each circle's radius (px) and
    /// material, each hit's spot.
    blasts: Vec<(Vec2, f32, Vec<(f32, RingMaterial)>, Vec<Option<Vec2>>)>,
}

/// The pieces spawned (`Shape`'s), to rebind in place.
struct Built {
    shape: Shape,
    root: Entity,
    /// The panel's anchor, its leader and the panel.
    anchor: Option<(Entity, Entity, Entity)>,
    rings: Vec<Entity>,
    /// Each blast: (label box, label text) per sphere, (spot, text) per
    /// hit on screen.
    blasts: Vec<(Vec<(Entity, Entity)>, Vec<Option<(Entity, Entity)>>)>,
}

/// How many times the range's pieces were spawned (tests: a move
/// rebinds, it never spawns them again).
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RangeBuilds(pub u64);

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn draw(
    look: Look,
    ui_state: Res<SandboxUi>,
    panel: Res<LookPanel>,
    bench: Res<BenchView>,
    blasts: Res<Blasts>,
    clock: Option<Res<chimera_ui::UiClock>>,
    sim: Option<Res<crate::core::SimClock>>,
    (camera, places_now, hulls): (
        Query<(Entity, &Camera), With<FirstPersonCamera>>,
        TransformHelper,
        Query<(&avian3d::prelude::ColliderAabb, &avian3d::prelude::Position)>,
    ),
    mut nodes: Query<&mut Node>,
    mut styles: Query<&mut UiStyle>,
    mut texts: Query<&mut UiText>,
    mut rings: ResMut<Assets<RingMaterial>>,
    mut pool: Local<Vec<Handle<RingMaterial>>>,
    mut built: Local<Option<Built>>,
    (mut builds, cover): (ResMut<RangeBuilds>, Option<Res<crate::client::hud::HudCover>>),
    mut c: Commands,
) {
    // `cl_drawhud 0` (captures against another game) hides these too.
    let drawhud_off = cover.is_some_and(|c| c.is(crate::client::hud::DRAWHUD_COVER));
    let on = look.on() && ui_state.on && ui_state.screen != SandboxScreen::ChangeGun && !drawhud_off;
    let staged = ui_state.staged;
    let any = panel.0.is_some() || !blasts.0.is_empty() || bench.targets.iter().any(|t| t.respawn.is_some());
    if !on || !any {
        if let Some(b) = built.take() {
            c.entity(b.root).try_despawn();
        }
        return;
    }
    let size = Vec2::new(look.units.height * 16.0 / 9.0, look.units.height);
    // The eye as placed this frame (`GlobalTransform`s are last frame's
    // until `PostUpdate`: the panel trailed the view by a frame).
    let eye = camera
        .iter()
        .next()
        .filter(|_| !staged)
        .and_then(|(e, c)| places_now.compute_global_transform(e).ok().map(|g| (c, g)));
    let cam = eye.as_ref().map(|(c, g)| (*c, g));
    // What shows and where.
    let mut shape = Shape::default();
    let mut places = Places::default();
    let lead_min = look.px(1.4);
    if let Some(p) = &panel.0 {
        // The body where it is drawn this frame: its hull's box (as the
        // simulation has it, around its place now).
        let body = p.entity.and_then(|e| {
            let now = places_now.compute_global_transform(e).ok()?.translation();
            let (aabb, pos) = hulls.get(e).ok()?;
            Some((now + (aabb.min - pos.0), now + (aabb.max - pos.0)))
        });
        let (head, region) = match body {
            Some((lo, hi)) => {
                let top = Vec3::new((lo.x + hi.x) / 2.0, hi.y + HEAD_ABOVE_HULL, (lo.z + hi.z) / 2.0);
                (top, screen_region(cam, lo - Vec3::splat(BODY_MARGIN), hi + Vec3::new(BODY_MARGIN, HEAD_ABOVE_HULL, BODY_MARGIN)))
            }
            None => (p.top, None),
        };
        if let Some(at) = project(cam, head, size) {
            shape.panel = Some(LookedAt { top: Vec3::ZERO, feet: Vec3::ZERO, ..p.clone() });
            places.anchor = Some(at);
            // Beside the body: past its right edge on the screen.
            places.lead = region.map_or(lead_min, |r| (r.max.x - at.x + look.px(0.5)).max(lead_min));
        }
    }
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
        let mut m = RingMaterial::new(look.theme.tone_a(Tone::Fg5, 1.0), look.theme.tone(Tone::Fg), (2.0 * look.units.scale_factor).round());
        m.set_share(share);
        shape.rings.push((i, if staged { 2 } else { left.ceil() as u32 }));
        places.rings.push((spot, m));
    }
    let t_now = clock.map_or(0.0, |c| c.now);
    // The spheres are in the world (`blast_spheres`); here their labels.
    for (k, (t0, b)) in blasts.0.iter().enumerate() {
        let a = if staged { 1.0 } else { (1.0 - ((t_now - t0) / BLAST_SECS) as f32).clamp(0.0, 1.0) };
        let centre = if cam.is_none() { Some(STAGED_AT * size + Vec2::new(0.0, 120.0 + 60.0 * k as f32)) } else { project(cam, b.at, size) };
        let Some(centre) = centre else { continue };
        // A radius on the screen: the edge seen across the view.
        let radius = |r: f32| -> f32 {
            match cam {
                Some((c, g)) => c.world_to_viewport(g, b.at + g.right() * r).map_or(0.0, |e| (e - centre).length()),
                None => r * 40.0,
            }
        };
        let circles = [
            (b.outer, (format!("{:.1} m \u{00b7} {:.0} at the edge", b.outer, b.edge_damage), Tone::Fg2)),
            (b.inner, (format!("{:.1} m \u{00b7} {:.0} full", b.inner, b.max_damage), Tone::Fg)),
        ];
        let mut words = Vec::new();
        let mut circ = Vec::new();
        for (r, w) in circles.into_iter().filter(|(r, ..)| *r > 0.05) {
            let m = RingMaterial::new(Color::srgba(1.0, 0.55, 0.24, 0.16 * a), Color::srgba(1.0, 0.67, 0.35, 0.9 * a), (2.0 * look.units.scale_factor).round());
            words.push(w);
            circ.push((radius(r).max(4.0), m));
        }
        let mut hit_words = Vec::new();
        let mut hit_at = Vec::new();
        for (p, dmg) in &b.hits {
            let at = if cam.is_none() { Some(centre + Vec2::new(90.0, -30.0)) } else { project(cam, *p + Vec3::Y * 1.8, size) };
            hit_words.push(at.map(|_| format!("{dmg:.0}")));
            hit_at.push(at);
        }
        shape.blasts.push((words, hit_words));
        places.blasts.push((centre, a, circ, hit_at));
    }
    // The rings' materials are reused frame to frame (a material made
    // and dropped each frame is never drawn), in the same order either way.
    let mut used = 0usize;
    let mut handles: Vec<Handle<RingMaterial>> = Vec::new();
    for (_, m) in &places.rings {
        handles.push(pooled(&mut rings, &mut pool, &mut used, m.clone()));
    }

    let ring_d = look.px(1.5);
    // The same words: rebind in place.
    if let Some(b) = built.as_ref()
        && b.shape == shape
        && nodes.contains(b.root)
    {
        // The panel past the body: its leader's length and its place.
        if let (Some((_, leader, panel_e)), Some(_)) = (b.anchor, places.anchor) {
            if let Ok(mut n) = nodes.get_mut(leader)
                && n.width != Val::Px(places.lead)
            {
                n.width = Val::Px(places.lead);
            }
            if let Ok(mut n) = nodes.get_mut(panel_e)
                && n.left != Val::Px(places.lead)
            {
                n.left = Val::Px(places.lead);
            }
        }
        let mut put = |e: Entity, left: f32, top: f32, wh: Option<f32>| {
            if let Ok(mut n) = nodes.get_mut(e) {
                let (l, t) = (Val::Px(left), Val::Px(top));
                if n.left != l || n.top != t {
                    n.left = l;
                    n.top = t;
                }
                if let Some(wh) = wh
                    && n.width != Val::Px(wh)
                {
                    n.width = Val::Px(wh);
                    n.height = Val::Px(wh);
                }
            }
        };
        if let (Some((e, _, _)), Some(at)) = (b.anchor, places.anchor) {
            put(e, at.x, at.y, None);
        }
        for (e, (spot, _)) in b.rings.iter().zip(&places.rings) {
            put(*e, spot.x - ring_d / 2.0, spot.y - ring_d / 2.0, None);
        }
        for ((circles, hits), (centre, a, circ, hit_at)) in b.blasts.iter().zip(&places.blasts) {
            for ((label, text), (px, _)) in circles.iter().zip(circ) {
                put(*label, centre.x + px + 6.0, centre.y - 10.0, None);
                fade(&mut styles, &mut texts, Some(*label), *text, *a);
            }
            for (h, at) in hits.iter().zip(hit_at) {
                if let (Some((spot, text)), Some(at)) = (h, at) {
                    put(*spot, at.x, at.y, None);
                    fade(&mut styles, &mut texts, None, *text, *a);
                }
            }
        }
        return;
    }
    // Spawned again.
    if let Some(b) = built.take() {
        c.entity(b.root).try_despawn();
    }
    builds.0 += 1;
    let ui = Ui::new(&look.theme, look.fonts.as_deref(), *look.units);
    let root = c
        .spawn((
            RangeRoot,
            Node { position_type: PositionType::Absolute, left: Val::ZERO, top: Val::ZERO, width: Val::Percent(100.0), height: Val::Percent(100.0), ..default() },
            GlobalZIndex(crate::client::minigame_scoreboard::SCOREBOARD_Z),
            Pickable::IGNORE,
        ))
        .id();
    let mut handles = handles.into_iter();
    let anchor = match (&shape.panel, places.anchor) {
        (Some(p), Some(at)) => Some(look_panel(&mut c, &ui, &look, root, p, at, places.lead)),
        _ => None,
    };
    let mut ring_es = Vec::new();
    for ((i, secs), (spot, _)) in shape.rings.iter().zip(&places.rings) {
        let ring = c
            .spawn((
                Node { position_type: PositionType::Absolute, left: Val::Px(spot.x - ring_d / 2.0), top: Val::Px(spot.y - ring_d / 2.0), width: Val::Px(ring_d), height: Val::Px(ring_d), justify_content: JustifyContent::Center, align_items: AlignItems::Center, ..default() },
                MaterialNode(handles.next().unwrap_or_default()),
                UiParityId::new(format!("range.respawn.{}", i + 1)),
                ChildOf(root),
            ))
            .id();
        ui.text(&mut c, ring, Role::Mono, 0.6, Tone::Fg, &secs.to_string());
        ring_es.push(ring);
    }
    let mut blast_es = Vec::new();
    for ((words, hit_words), (centre, a, circ, hit_at)) in shape.blasts.iter().zip(&places.blasts) {
        let mut circles = Vec::new();
        for ((w, tone), (px, _)) in words.iter().zip(circ) {
            let l = c
                .spawn((
                    UiStyle { gap: Some(0.4), align: Some(AlignItems::Baseline), pad: Some([0.3, 0.5, 0.26, 0.5].into()), ..UiStyle::row() }.bg_a(Tone::Ink, 0.78 * a),
                    Node { position_type: PositionType::Absolute, left: Val::Px(centre.x + px + 6.0), top: Val::Px(centre.y - 10.0), ..default() },
                    UiParityId::new("range.blast"),
                    ChildOf(root),
                ))
                .id();
            let t = ui.text_a(&mut c, l, Role::Mono, 0.4, *tone, *a, w);
            circles.push((l, t));
        }
        let mut hits = Vec::new();
        for (w, at) in hit_words.iter().zip(hit_at) {
            hits.push(match (w, at) {
                (Some(w), Some(at)) => {
                    let spot = c
                        .spawn((Node { position_type: PositionType::Absolute, left: Val::Px(at.x), top: Val::Px(at.y), ..default() }, UiTransform { translation: Val2::percent(-50.0, 0.0), ..default() }, ChildOf(root)))
                        .id();
                    let t = ui.text_a(&mut c, spot, Role::Mono, 0.9, Tone::Fg, *a, w);
                    Some((spot, t))
                }
                _ => None,
            });
        }
        blast_es.push((circles, hits));
    }
    *built = Some(Built { shape, root, anchor, rings: ring_es, blasts: blast_es });
}

/// A blast's sphere in the world (`blast_spheres`): which blast (its
/// time on the UI clock) and its share of the full alpha.
#[derive(Component)]
pub struct BlastSphere {
    pub at: f64,
    pub alpha: f32,
}

/// The blasts' spheres: spawned with each blast, faded with it (their
/// materials' alpha), gone with it; out of the sandbox, or with Change gun
/// open, none. Unlit, both faces (seen from inside too), not casting
/// shadows.
#[allow(clippy::type_complexity)]
fn blast_spheres(
    ui_state: Res<SandboxUi>,
    blasts: Res<Blasts>,
    clock: Option<Res<chimera_ui::UiClock>>,
    spheres: Query<(Entity, &BlastSphere, &MeshMaterial3d<StandardMaterial>)>,
    mut meshes: Option<ResMut<Assets<Mesh>>>,
    mut materials: Option<ResMut<Assets<StandardMaterial>>>,
    mut c: Commands,
) {
    let (Some(meshes), Some(materials)) = (meshes.as_deref_mut(), materials.as_deref_mut()) else { return };
    let on = ui_state.on && !ui_state.staged && ui_state.screen != SandboxScreen::ChangeGun;
    let now = clock.map_or(0.0, |c| c.now);
    for (e, s, m) in &spheres {
        let Some((t0, _)) = blasts.0.iter().find(|(t, _)| *t == s.at).filter(|_| on) else {
            c.entity(e).try_despawn();
            continue;
        };
        let a = (1.0 - ((now - t0) / BLAST_SECS) as f32).clamp(0.0, 1.0) * s.alpha;
        if let Some(mut mat) = materials.get_mut(&m.0)
            && (mat.base_color.alpha() - a).abs() > 1e-3
        {
            mat.base_color.set_alpha(a);
        }
    }
    if !on {
        return;
    }
    for (t0, b) in &blasts.0 {
        if spheres.iter().any(|(_, s, _)| s.at == *t0) {
            continue;
        }
        for (r, alpha) in [(b.outer, 0.10), (b.inner, 0.18)] {
            if r <= 0.05 {
                continue;
            }
            let material = materials.add(StandardMaterial {
                base_color: Color::srgba(1.0, 0.55, 0.24, alpha),
                alpha_mode: AlphaMode::Blend,
                unlit: true,
                double_sided: true,
                cull_mode: None,
                ..default()
            });
            c.spawn((
                BlastSphere { at: *t0, alpha },
                Mesh3d(meshes.add(Sphere::new(r).mesh().uv(48, 24))),
                MeshMaterial3d(material),
                Transform::from_translation(b.at),
                bevy::light::NotShadowCaster,
                bevy::light::NotShadowReceiver,
            ));
        }
    }
}

/// A blast's label box and text to alpha `a` (only when it changed).
fn fade(styles: &mut Query<&mut UiStyle>, texts: &mut Query<&mut UiText>, label: Option<Entity>, text: Entity, a: f32) {
    if let Some(l) = label
        && let Ok(mut s) = styles.get_mut(l)
        && s.bg.is_some_and(|(_, was)| (was - 0.78 * a).abs() > 1e-3)
    {
        s.bg = Some((Tone::Ink, 0.78 * a));
    }
    if let Ok(mut t) = texts.get_mut(text)
        && (t.alpha - a).abs() > 1e-3
    {
        t.alpha = a;
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

fn look_panel(c: &mut Commands, ui: &Ui, look: &Look, root: Entity, p: &LookedAt, at: Vec2, lead: f32) -> (Entity, Entity, Entity) {
    let anchor = c.spawn((Node { position_type: PositionType::Absolute, left: Val::Px(at.x), top: Val::Px(at.y), ..default() }, ChildOf(root))).id();
    // The leader: a line from the head to the panel, a dot at the head.
    let leader = c
        .spawn((Node { position_type: PositionType::Absolute, left: Val::ZERO, top: look.val(1.0), width: Val::Px(lead), height: Val::Px(look.hair()), ..default() }, BackgroundColor(look.theme.tone_a(Tone::Fg, 0.6)), ChildOf(anchor)))
        .id();
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
    // No hit yet: two dashes (the Mono face has no em dash).
    let d = ui.boxed(c, panel, UiStyle { gap: Some(0.4), align: Some(AlignItems::Baseline), ..UiStyle::row() });
    ui.text(c, d, Role::Mono, 1.5, if p.last.is_some() { Tone::Fg } else { Tone::Fg4 }, &p.last.map_or("--".into(), |v| format!("{v:.0}")));
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
    (anchor, leader, panel)
}

/// The range's systems.
pub fn add(app: &mut App) {
    app.init_resource::<Blasts>().init_resource::<RangeBuilds>().add_systems(
        Update,
        (collect, draw, blast_spheres)
            .chain()
            .after(super::SandboxSystems)
            // The eye placed this frame (`draw` reads it).
            .after(crate::map::attached::EyePlaced)
            .run_if(resource_exists::<chimera_ui::Theme>)
            .run_if(resource_exists::<chimera_ui::UiUnits>)
            .run_if(resource_exists::<Assets<RingMaterial>>),
    );
}
