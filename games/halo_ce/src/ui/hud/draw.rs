//! Halo's HUD as a draw list (specs/halo_ce/hud.md, sections 2 to 11):
//! from the HUD's data (`data::HudData`), what the local player's unit
//! and weapon are now (`Unit`) and the HUD's own per-tick state
//! (`HudState`), the pictures to draw this frame in the game's order
//! (`Quad`s in canvas units: the weapon's crosshairs, its HUD chain from
//! the deepest parent, the grenade plate, the unit HUD's shield (meter
//! stages, then the plate), health (meter, then the plate) and the motion
//! tracker (disc, cone, sweep, blips)). Pure: the nodes (`nodes`) turn the
//! quads into UI material nodes.
//!
//! Known shortcuts (against the spec):
//! - One local player only (no split screen: the second unit HUD, the
//!   "split screen only" elements and the 0.5 crosshair scale are never
//!   used).
//! - Multitexture overlays (the sniper rifle's angle ticks) are not
//!   drawn. Of the scopes' screen effect (section 9, `scope_mask`) only
//!   the darkening by the mask's grey level is drawn (over the finished
//!   scene, under the HUD, as the pass's last step): the radial blur
//!   through the mask's alpha and the sniper's night vision (light
//!   enhancement and desaturation) are not (the spec allows the blur to
//!   degrade to none; both need the scene as a texture, a full-screen
//!   pass this HUD does not have).
//! - The sweep's compositing is ours (section 11 is inferred): the sweep
//!   picture is added in its light blue through the disc's mask, and each
//!   blip's brightness is scaled by the sweep's alpha where it lies.
//! - The autoaim's target (`game::weapons::aim`) does not reach the HUD:
//!   the aim crosshair never turns red and the distance and elevation
//!   numbers never draw.
//! - In the party layout the crosshairs also hide the frame behind them by
//!   their brightness (`Fill::Reticle`, ours: added on a bright map of
//!   another game, the plasma pistol's light blue reticle vanished).
//! - The flashlight meter, damage arrows, waypoints, teammate markers,
//!   messages and prompts are not drawn; the warning sounds do not play.

use blam_formats::{
    cache::TagRef,
    hud::{
        Anchor, ColorBlock, MeterElement, NumberElement, Placement, StaticElement,
        weapon::{CrosshairItem, FlashCutoffs, WeaponHud},
    },
};

use super::{
    data::{HudData, Picture},
    layout::{self, Canvas, Value},
    rules::{self, BlipKind, CrosshairStates, MeterDraw, Rgba, ShieldMark, Tracker, WeaponNow, crosshair},
};

/// How a quad fills its rectangle.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Fill {
    /// The picture times the colour, "alpha-multiply add" (section 3).
    Sprite(Rgba),
    /// The meter shader (section 4).
    Meter(MeterDraw),
    /// The tracker's sweep: the picture sampled over a window of `half`
    /// about its centre, in `colour`, through `mask`.
    Sweep { colour: [f32; 3], half: f32, mask: Option<Picture> },
    /// The scope's darkening (section 9): the frame multiplied by 1 - the
    /// texel's grey level (its blue: Y of an A8Y8 or AY8 mask).
    Darken,
    /// A crosshair in the party layout: a `Sprite` that also hides the
    /// frame behind it by its brightness (the party's maps are other
    /// games', often bright, where an added light blue disappears).
    Reticle(Rgba),
}

/// One picture drawn: where (canvas units: left, top, right, bottom) and
/// how.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Quad {
    pub picture: Picture,
    pub rect: [f32; 4],
    pub fill: Fill,
}

/// The weapon in the local player's hand.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Held {
    /// The weapon tag's path (`params::Gun::tag`).
    pub tag: String,
    pub now: WeaponNow,
}

/// The local player's unit as the HUD reads it.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Unit {
    pub alive: bool,
    /// The shield fraction (to 3 with an overshield) and the body's.
    pub shield: f32,
    pub body: f32,
    /// The shield's "depleted" flag (the health plate flashes).
    pub depleted: bool,
    /// None: empty hands (the default HUD).
    pub weapon: Option<Held>,
    pub grenades: i32,
    pub throwing: bool,
    /// The current grenade type (0 frag, 1 plasma: the grenade HUD drawn).
    pub grenade_kind: usize,
    /// The unit has no current grenade type (a new life before its first
    /// update, or no grenades at all): no grenade HUD.
    pub no_grenade_type: bool,
}

/// A unit the tracker may show: an id, its kind, its horizontal
/// displacement from the viewer in world units (Halo's frame: x the
/// viewer's world forward axis at yaw 0, y left), speed in world units per
/// tick, and whether it fires.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Other {
    pub id: u64,
    pub kind: BlipKind,
    pub at: [f32; 2],
    pub speed: f32,
    pub firing: bool,
}

/// The HUD's own state of one local player, advanced once per Halo tick
/// (`HudState::tick`, "Per-tick order").
#[derive(Clone, Debug, Default, PartialEq)]
pub struct HudState {
    pub tick: Option<i64>,
    pub shield: ShieldMark,
    pub shield_flash: Option<i64>,
    pub health_flash: Option<i64>,
    pub weapon_flash: [Option<i64>; 8],
    pub grenade_flash: Option<i64>,
    pub crosshairs: CrosshairStates,
    pub tracker: Tracker,
}

fn latch(slot: &mut Option<i64>, on: bool, now: i64) {
    *slot = match (*slot, on) {
        (Some(s), true) => Some(s),
        (None, true) => Some(now),
        _ => None,
    };
}

/// The chain of the held weapon (or the empty hands').
pub fn chain_of<'a>(data: &'a HudData, unit: &Unit) -> &'a [WeaponHud] {
    match &unit.weapon {
        Some(h) => data.weapon(&h.tag).unwrap_or(&[]),
        None => &data.empty,
    }
}

/// The cutoffs each HUD of `chain` uses (its own, or its child's with
/// "use parent HUD flash parameters").
fn cutoffs(chain: &[WeaponHud]) -> Vec<FlashCutoffs> {
    let mut out: Vec<FlashCutoffs> = Vec::new();
    for (i, h) in chain.iter().enumerate() {
        let c = if i > 0 && h.cutoffs.flags & WeaponHud::USE_PARENT_FLASH != 0 { out[i - 1].clone() } else { h.cutoffs.clone() };
        out.push(c);
    }
    out
}

/// Ticks a latched warning of crosshair type `n` stays after its
/// condition ends: its items' flash period (section 8).
fn hold(chain: &[WeaponHud], n: usize) -> i64 {
    chain
        .iter()
        .flat_map(|h| &h.crosshairs)
        .filter(|c| c.kind as usize == n)
        .flat_map(|c| &c.items)
        .map(|i| (i.colors.flash_period * rules::TICKS) as i64)
        .max()
        .unwrap_or(0)
}

impl HudState {
    /// One Halo tick at `now`: the crosshair states, the shield's
    /// bookkeeping, the flash start ticks, and the tracker's history.
    pub fn tick(&mut self, data: &HudData, unit: &Unit, others: &[Other], yaw: f32, now: i64) {
        self.tick = Some(now);
        let chain = chain_of(data, unit);
        let w = unit.weapon.as_ref().map(|h| h.now).unwrap_or_default();
        let valid = chain.iter().fold(0, |v, h| v | h.valid_crosshairs);
        let root = chain.first().map(|h| h.cutoffs.clone()).unwrap_or(FlashCutoffs { flags: 0, total_ammo: 0, loaded_ammo: 0, heat: 0, age: 0 });
        if unit.weapon.is_none() {
            self.crosshairs.start = Default::default();
        }
        self.crosshairs.update(valid, &w, &root, unit.grenades > 0, unit.throwing, now, |n| hold(chain, n));
        self.shield.update(unit.shield, now);
        latch(&mut self.shield_flash, unit.alive && unit.shield < rules::SHIELD_FLASH_BELOW, now);
        latch(&mut self.health_flash, unit.alive && unit.depleted, now);
        let states = rules::weapon_states(&w, &root);
        for (slot, s) in self.weapon_flash.iter_mut().zip(states) {
            latch(slot, s.flashing && unit.weapon.is_some(), now);
        }
        let cutoff = data.grenades.get(unit.grenade_kind).or(data.grenades.first()).map_or(1, |g| i32::from(g.flash_cutoff));
        latch(&mut self.grenade_flash, unit.grenades <= cutoff, now);
        let range = data.globals.defaults.sensor_range.max(1.0);
        let sensitivity = data.globals.defaults.sensor_velocity_sensitivity;
        let seen: Vec<rules::Seen> = others
            .iter()
            .map(|o| rules::Seen {
                id: o.id,
                kind: o.kind,
                at: o.at,
                shows: rules::shows(o.speed, o.firing, (o.at[0] * o.at[0] + o.at[1] * o.at[1]).sqrt(), sensitivity, range),
            })
            .collect();
        self.tracker.frame(now.max(0) as u64, &seen, yaw, range);
    }
}

/// A placement's scale (0 reads as 1).
fn scale_of(p: &Placement) -> [f32; 2] {
    p.scale.map(|s| if s == 0.0 { 1.0 } else { s })
}

/// Draws into a list (`draw`).
struct Painter<'a> {
    data: &'a HudData,
    frame: [i32; 4],
    now: i64,
    out: Vec<Quad>,
}

impl Painter<'_> {
    fn push(&mut self, picture: Picture, r: [i32; 4], fill: Fill) {
        self.out.push(Quad { picture, rect: r.map(|v| v as f32), fill });
    }

    /// A static element by `anchor` with its colour.
    fn static_el(&mut self, el: &StaticElement, anchor: Anchor, colour: Rgba) {
        let Some(p) = self.data.picture(&el.bitmap, el.sequence, 0) else { return };
        let r = layout::rect(anchor, el.placement.offset, p.size, scale_of(&el.placement), self.frame);
        self.push(p, r, Fill::Sprite(colour));
    }

    fn meter(&mut self, el: &MeterElement, anchor: Anchor, draw: MeterDraw) {
        let Some(p) = self.data.picture(&el.bitmap, el.sequence, 0) else { return };
        let r = layout::rect(anchor, el.placement.offset, p.size, scale_of(&el.placement), self.frame);
        self.push(p, r, Fill::Meter(draw));
    }

    #[allow(clippy::too_many_arguments)]
    fn number(&mut self, el: &NumberElement, anchor: Anchor, value: Value, colour: Rgba) {
        let n = &self.data.number;
        let origin = layout::anchor_point(anchor, el.placement.offset, self.frame);
        let glyphs = layout::number_glyphs(
            value,
            el.digits,
            el.fractional_digits,
            el.flags & NumberElement::LEADING_ZEROS != 0,
            el.flags & NumberElement::METRES != 0,
            anchor,
            origin,
            i32::from(n.screen_width),
            i32::from(n.decimal_point_width),
        );
        for (glyph, x) in glyphs {
            let Some(p) = self.data.picture(&n.bitmap, 0, i64::from(glyph)) else { continue };
            let r = layout::rect_at(anchor, [x, origin[1]], p.size, scale_of(&el.placement));
            self.push(p, r, Fill::Sprite(colour));
        }
    }

    /// An animated item's frame: ((now - start) div rate div 30) mod
    /// count, 0 unless animated.
    fn frame(&self, start: Option<i64>, rate: i16, animated: bool) -> i64 {
        match start {
            Some(s) if animated && rate > 0 => (self.now - s) / i64::from(rate) / 30,
            _ => 0,
        }
    }
}

/// Whether an element of `map_type` draws in one local window (bit 0:
/// always, bit 1: one window).
fn one_window(map_type: i16) -> bool {
    matches!(map_type, 0 | 1)
}

/// Whether the zoom's mask is drawn (hud.md 9, the fourth errata: it dims,
/// nothing is blacked out).
pub const SCOPE_MASK: bool = true;

/// The zoom's screen effect's darkening (section 9, step 5): the first
/// screen effect of the weapon's HUD chain, when zoomed or its "only when
/// zoomed" bit is clear: its full-screen mask one texel per 480-line pixel
/// (a canvas unit, scaled with the HUD) centred on the view, the frame
/// multiplied by 1 - the mask's grey level Y (0 in the window, 1 on the
/// frame line, about 0.5 outside), and beyond the texture its edge texel
/// (the address clamps: the disc's masks' border is Y 128 or 132).
fn scope_mask(p: &mut Painter, chain: &[WeaponHud], zoomed: bool, canvas: &Canvas) {
    let Some(effect) = chain.iter().flat_map(|h| &h.screen_effects).next() else { return };
    if !zoomed && effect.mask_flags & 1 != 0 {
        return;
    }
    let Some(mask) = p.data.whole_image(&effect.mask_full, 0) else { return };
    let (w, h) = (mask.size[0] as i32, mask.size[1] as i32);
    let (cx, cy) = (canvas.width / 2, layout::CANVAS_HEIGHT / 2);
    let r = [cx - w / 2, cy - h / 2, cx - w / 2 + w, cy - h / 2 + h];
    p.push(mask, r, Fill::Darken);
    // Beyond the mask: its corner texel (clamped addressing; the disc's
    // masks' whole border is one grey).
    let rim = Picture { uv: [0.25 / mask.size[0], 0.25 / mask.size[1], 0.75 / mask.size[0], 0.75 / mask.size[1]], size: [1.0, 1.0], ..mask };
    let (cw, ch) = (canvas.width, layout::CANVAS_HEIGHT);
    for b in [[0, 0, cw, r[1]], [0, r[3], cw, ch], [0, r[1], r[0], r[3]], [r[2], r[1], cw, r[3]]] {
        if b[2] > b[0] && b[3] > b[1] {
            p.push(rim, b, Fill::Darken);
        }
    }
}

/// Where this frame's blocks are (canvas units: their pictures' bounding
/// boxes; None: not drawn): the weapon's (its HUD chain's elements, top
/// left), the grenade plate's and the unit's shield and health block
/// (without the tracker).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Blocks {
    pub weapon: Option<[f32; 4]>,
    pub grenades: Option<[f32; 4]>,
    pub unit: Option<[f32; 4]>,
}

/// This frame's quads.
pub fn draw(data: &HudData, unit: &Unit, state: &HudState, canvas: &Canvas, now: i64) -> Vec<Quad> {
    draw_blocks(data, unit, state, canvas, now).0
}

/// This frame's quads and where its blocks are.
pub fn draw_blocks(data: &HudData, unit: &Unit, state: &HudState, canvas: &Canvas, now: i64) -> (Vec<Quad>, Blocks) {
    let mut blocks = Blocks::default();
    let bounds = |q: &[Quad]| layout::rect_bounds(q.iter().map(|q| q.rect));
    let mut p = Painter { data, frame: canvas.frame(), now, out: Vec::new() };
    let chain = chain_of(data, unit);
    let w = unit.weapon.as_ref().map(|h| h.now).unwrap_or_default();
    let disabled_all = !unit.alive;
    // 1. The scope's mask (section 9), under the HUD.
    if SCOPE_MASK && unit.alive && unit.weapon.is_some() {
        scope_mask(&mut p, chain, w.zoom >= 0, canvas);
    }
    // 2. Crosshairs: the weapon's own HUD first, then each parent.
    let crosshairs_from = p.out.len();
    if unit.alive {
        let xs = &state.crosshairs;
        for hud in chain {
            for c in &hud.crosshairs {
                let n = c.kind as usize;
                if c.kind < 0 || n >= crosshair::COUNT || c.runtime_flags & 1 != 0 || !one_window(c.map_type) || !xs.renders(n) {
                    continue;
                }
                for item in &c.items {
                    crosshair_item(&mut p, c.kind, &c.bitmap, item, xs);
                }
            }
        }
    }
    if canvas.layout == layout::HudLayout::Party {
        for q in &mut p.out[crosshairs_from..] {
            if let Fill::Sprite(c) = q.fill {
                q.fill = Fill::Reticle(c);
            }
        }
    }
    // 3. The weapon HUD, deepest parent first.
    let weapon_from = p.out.len();
    if !chain.is_empty() && unit.weapon.is_some() {
        let cut = cutoffs(chain);
        for (hud, c) in chain.iter().zip(&cut).rev() {
            let states = rules::weapon_states(&w, c);
            let anchor = Anchor::of(hud.anchor);
            let st = |s: i16| usize::try_from(s).ok().filter(|s| *s < 8);
            for el in hud.statics.iter().filter(|e| e.runtime_flags & 1 == 0 && one_window(e.map_type)) {
                let Some(s) = st(el.state) else { continue };
                let start = state.weapon_flash[s].or(Some(now));
                let colour = rules::color(&el.element.colors, states[s].flashing.then_some(start).flatten(), states[s].disabled || disabled_all, now);
                p.static_el(&el.element, anchor, colour);
            }
            for el in hud.meters.iter().filter(|e| e.runtime_flags & 1 == 0 && one_window(e.map_type)) {
                let Some(s) = st(el.state) else { continue };
                let byte = i32::from(states[s].value as u8);
                let reference = if states[s].flashing { state.weapon_flash[s].unwrap_or(now) as f32 } else { -1.0 };
                let d = rules::meter_draw(&el.element, byte, byte, states[s].flashing, states[s].disabled || disabled_all, reference, None);
                p.meter(&el.element, anchor, d);
            }
            for el in hud.numbers.iter().filter(|e| e.runtime_flags & 1 == 0 && one_window(e.map_type)) {
                let Some(s) = st(el.state) else { continue };
                let value = match states[s].real {
                    Some(r) if el.element.fractional_digits > 0 || r.is_nan() => {
                        if r.is_nan() {
                            continue;
                        }
                        Value::Real(r)
                    }
                    _ if el.weapon_flags & 1 != 0 => Value::Int(states[s].value / i64::from(w.loaded_max.max(1))),
                    _ => Value::Int(states[s].value),
                };
                let start = state.weapon_flash[s].or(Some(now));
                let colour = rules::color(&el.element.colors, states[s].flashing.then_some(start).flatten(), states[s].disabled || disabled_all, now);
                p.number(&el.element, anchor, value, colour);
            }
            for el in hud.overlays.iter().filter(|e| e.runtime_flags & 1 == 0 && one_window(e.map_type)) {
                let Some(s) = st(el.state) else { continue };
                for item in el.items.iter().filter(|i| i.flags & 2 == 0 && i.kind & states[s].mask != 0) {
                    let flashes = item.flags & 1 != 0 && states[s].flashing;
                    let start = state.weapon_flash[s].unwrap_or(now);
                    let colour = if flashes { rules::flash(&item.colors, start, now) } else { rules::rgba(item.colors.color) };
                    let frame = p.frame(Some(start), item.frame_rate, flashes);
                    let Some(pic) = data.picture(&el.bitmap, item.sequence, frame) else { continue };
                    let r = layout::rect(anchor, item.placement.offset, pic.size, scale_of(&item.placement), p.frame);
                    p.push(pic, r, Fill::Sprite(colour));
                }
            }
        }
    }
    blocks.weapon = bounds(&p.out[weapon_from..]);
    // 4. The grenade HUD of the current type.
    let grenades_from = p.out.len();
    if let Some(g) = data.grenades.get(unit.grenade_kind).or(data.grenades.first()).filter(|_| unit.alive && !unit.no_grenade_type) {
        let anchor = Anchor::of(g.anchor);
        let count = unit.grenades;
        let flashing = count <= i32::from(g.flash_cutoff);
        let disabled = count == 0;
        let start = flashing.then(|| state.grenade_flash.unwrap_or(now));
        for el in [&g.background, &g.count_background] {
            let colour = rules::color(&el.colors, start, disabled, now);
            p.static_el(el, anchor, colour);
        }
        if g.count.digits > 0 {
            let colour = rules::color(&g.count.colors, start, disabled, now);
            p.number(&g.count, anchor, Value::Int(i64::from(count)), colour);
        }
        let mask = 8 | if flashing { 1 } else { 0 } | if disabled { 2 } else { 0 } | if !flashing && !disabled { 4 } else { 0 };
        for item in g.overlays.iter().filter(|i| i.kind & mask != 0 && i.flags & 2 == 0) {
            let flashes = item.flags & 1 != 0 && flashing;
            let colour = if flashes { rules::flash(&item.colors, start.unwrap_or(now), now) } else { rules::rgba(item.colors.color) };
            let Some(pic) = data.picture(&g.overlay_bitmap, item.sequence, 0) else { continue };
            let r = layout::rect(anchor, item.placement.offset, pic.size, scale_of(&item.placement), p.frame);
            p.push(pic, r, Fill::Sprite(colour));
        }
    }
    blocks.grenades = bounds(&p.out[grenades_from..]);
    // The party's layout: a grenade plate reaching into the party's
    // top-centre column (a narrow window) drops under the weapon's block.
    if canvas.layout == layout::HudLayout::Party
        && let Some(g) = blocks.grenades
        && g[2] > canvas.party_column()[0]
    {
        let dy = blocks.weapon.map_or(0.0, |w| (w[3] + 4.0 - g[1]).max(0.0));
        for q in &mut p.out[grenades_from..] {
            q.rect[1] += dy;
            q.rect[3] += dy;
        }
        blocks.grenades = bounds(&p.out[grenades_from..]);
    }
    // 7. The unit HUD.
    let block = p.out.len();
    let u = &data.unit;
    let anchor = Anchor::of(u.anchor);
    p.static_el(&u.background, anchor, rules::color(&u.background.colors, None, disabled_all, now));
    // The shield: the meter's stages from the lowest up, then the plate.
    let shield_flashing = unit.alive && unit.shield < rules::SHIELD_FLASH_BELOW;
    for stage in rules::shield_stages(unit.shield.max(0.0), &state.shield, u.shield.meter.value_scale) {
        let d = rules::meter_draw(&u.shield.meter, stage.min_value, stage.max_value, shield_flashing, disabled_all, stage.reference, stage.colour);
        p.meter(&u.shield.meter, anchor, d);
    }
    let start = shield_flashing.then(|| state.shield_flash.unwrap_or(now));
    p.static_el(&u.shield.background, anchor, rules::color(&u.shield.background.colors, start, disabled_all, now));
    // Health: the meter in its colour by vitality, then the plate
    // (flashing while the shield is depleted).
    let (value, colour) = rules::health(&u.health, unit.body);
    let health_flashing = unit.alive && unit.depleted;
    let d = rules::meter_draw(&u.health.meter, value, value, health_flashing, disabled_all, -1.0, Some(colour));
    p.meter(&u.health.meter, anchor, d);
    let start = health_flashing.then(|| state.health_flash.unwrap_or(now));
    p.static_el(&u.health.background, anchor, rules::color(&u.health.background.colors, start, disabled_all, now));
    // The motion tracker, anchored bottom left.
    let sensor = Anchor::BottomLeft;
    let tracker_from = p.out.len();
    p.static_el(&u.sensor.background, sensor, rules::color(&u.sensor.background.colors, None, disabled_all, now));
    p.static_el(&u.sensor.foreground, sensor, rules::color(&u.sensor.foreground.colors, None, disabled_all, now));
    if canvas.layout == layout::HudLayout::Party {
        // The party's layout: the block in the tracker's place, no tracker.
        let disc = layout::rect_bounds(p.out[tracker_from..].iter().map(|q| q.rect));
        p.out.truncate(tracker_from);
        if let (Some(tracker), Some(own)) = (disc, layout::rect_bounds(p.out[block..].iter().map(|q| q.rect))) {
            let [dx, dy] = layout::party_offset(own, tracker, p.frame, canvas.party_floor());
            for q in &mut p.out[block..] {
                q.rect = [q.rect[0] + dx, q.rect[1] + dy, q.rect[2] + dx, q.rect[3] + dy];
            }
        }
        blocks.unit = bounds(&p.out[block..]);
        return (p.out, blocks);
    }
    blocks.unit = bounds(&p.out[block..tracker_from]);
    if unit.alive {
        let centre = layout::anchor_point(sensor, u.blip_placement.offset, p.frame);
        tracker(&mut p, state, centre, now);
    }
    (p.out, blocks)
}

/// The tracker's sweep and blips about `centre`.
fn tracker(p: &mut Painter, state: &HudState, centre: [i32; 2], now: i64) {
    let data = p.data;
    let half = rules::RADAR_HALF_FIELD;
    let window = rules::sweep(now);
    let (cx, cy) = (centre[0] as f32, centre[1] as f32);
    let field = [cx - half, cy - half, cx + half, cy + half];
    let sweeper = data.sweeper.and_then(|s| data.picture_of(s, 0, 0).or_else(|| data.pages.get(&s).map(|_| Picture { page: s, image: 0, uv: [0.0, 0.0, 1.0, 1.0], size: [64.0, 64.0] })));
    if let Some(pic) = sweeper {
        let mask = data.sweeper_mask.map(|m| Picture { page: m, image: 0, uv: [0.0, 0.0, 1.0, 1.0], size: [64.0, 64.0] });
        p.out.push(Quad { picture: pic, rect: field, fill: Fill::Sweep { colour: [0.4588, 0.7294, 1.0], half: window, mask } });
    }
    let Some(blip_page) = data.blip else { return };
    let Some(blip) = data.picture_of(blip_page, 0, 0).or_else(|| data.pages.get(&blip_page).map(|_| Picture { page: blip_page, image: 0, uv: [0.0, 0.0, 1.0, 1.0], size: [32.0, 32.0] })) else {
        return;
    };
    let range = data.globals.defaults.sensor_range.max(1.0);
    let scale = data.globals.defaults.sensor_scale;
    for (h, entry) in state.tracker.entries.iter().enumerate().rev() {
        let (brightness, size) = rules::blip_fade(h);
        for b in entry {
            let Some([ox, oy]) = rules::blip_offset(b.dx, b.dy, b.yaw, range, scale, half) else { continue };
            // The sweep's alpha where the blip lies: dim (its centre's)
            // except where the ring passes.
            let lit = sweeper.map_or(1.0, |s| data.alpha_at(s.page, s.image, 0.5 + ox / half * window, 0.5 + oy / half * window));
            let e = size * 0.0625 * half;
            let c = b.kind.colour().map(|v| v * brightness * lit);
            p.out.push(Quad { picture: blip, rect: [cx + ox - e, cy + oy - e, cx + ox + e, cy + oy + e], fill: Fill::Sprite([c[0], c[1], c[2], 0.0]) });
        }
    }
}

/// One crosshair item (section 8, "Draw").
fn crosshair_item(p: &mut Painter, kind: i16, page: &TagRef, item: &CrosshairItem, xs: &CrosshairStates) {
    if item.flags & CrosshairItem::INVALID != 0
        || (item.flags & CrosshairItem::ONLY_ZOOMED != 0 && xs.zoom == 0)
        || (item.flags & CrosshairItem::NOT_ZOOMED != 0 && xs.zoom != 0)
    {
        return;
    }
    let flashes = item.flags & CrosshairItem::FLASHES != 0;
    let now = p.now;
    let solid = |b: &ColorBlock| rules::flash(b, 0, now);
    let (frame, colour) = match kind as usize {
        crosshair::AIM => {
            if flashes {
                (0, if xs.aim > 0 { solid(&item.colors) } else { rules::rgba(item.colors.color) })
            } else {
                (xs.aim, rules::rgba(item.colors.color))
            }
        }
        crosshair::ZOOM => {
            let frame = if item.flags & CrosshairItem::ONE_ZOOM_LEVEL != 0 {
                if xs.zoom == 0 {
                    return;
                }
                0
            } else {
                xs.zoom - i64::from(item.flags & CrosshairItem::ONLY_ZOOMED != 0)
            };
            (frame, if flashes && xs.aim > 0 { solid(&item.colors) } else { rules::rgba(item.colors.color) })
        }
        n => {
            let start = xs.start[n];
            let frame = p.frame(start, item.frame_rate, true);
            (frame, if flashes { rules::flash(&item.colors, start.unwrap_or(now), now) } else { rules::rgba(item.colors.color) })
        }
    };
    let pic = if item.flags & CrosshairItem::NOT_A_SPRITE != 0 { p.data.whole_image(page, item.sequence) } else { p.data.picture(page, item.sequence, frame) };
    let Some(pic) = pic else { return };
    let r = layout::rect(Anchor::Center, item.placement.offset, pic.size, scale_of(&item.placement), p.frame);
    p.push(pic, r, Fill::Sprite(colour));
}
