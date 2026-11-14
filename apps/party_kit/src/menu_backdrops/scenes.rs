//! The three backdrops as painters (`paint::Paint`), from the look book
//! V's numbers (its frames are the spec; built again here): each draws
//! the whole menu frame `w` x `h` physical px at `t` s into its loop, `d`
//! physical px a look-book pixel.

use std::collections::HashMap;

use bevy::{
    color::{Alpha, LinearRgba},
    math::Vec2,
};
use chimera_client::client::weapon_icons::WeaponIcon;
use chimera_ui::{Icons, Role, icons};

use super::paint::{Align, Face, Half, ImageItem, Paint, lerp};

/// tan 12°: the one slant.
pub const TAN: f32 = 0.2126;

fn c01(x: f32) -> f32 {
    x.clamp(0.0, 1.0)
}
fn e_out(p: f32) -> f32 {
    chimera_ui::Ease::ExpoOut.sample(p)
}
fn e_in(p: f32) -> f32 {
    chimera_ui::Ease::ExpoIn.sample(p)
}
fn eio(p: f32) -> f32 {
    chimera_ui::Ease::CubicInOut.sample(p)
}
fn modf(a: f32, n: f32) -> f32 {
    a.rem_euclid(n)
}
fn v(x: f32, y: f32) -> Vec2 {
    Vec2::new(x, y)
}

/// The colours, with alphas as the look book writes them (display
/// space, blended in linear light: `light_alpha`).
#[derive(Clone, Copy, Debug)]
pub struct Pal {
    pub fg: LinearRgba,
    pub acc: LinearRgba,
}

impl Pal {
    pub fn fg(&self, a: f32) -> LinearRgba {
        self.fg.with_alpha(chimera_ui::theme::light_alpha(a))
    }
    pub fn acc(&self, a: f32) -> LinearRgba {
        self.acc.with_alpha(chimera_ui::theme::light_alpha(a))
    }
}

/// An icon as triangles, its width in units (16 tall), and its rings.
pub struct Shape {
    pub tris: Vec<[Vec2; 3]>,
    pub w: f32,
    pub rings: Vec<Vec<Vec2>>,
}

/// The kit's icons as shapes (once).
pub struct Shapes {
    pub by_key: HashMap<String, Shape>,
    /// The mark's six facets.
    pub facets: Vec<Shape>,
}

impl Shapes {
    pub fn new(icons_: &Icons) -> Shapes {
        let mut by_key = HashMap::new();
        for (k, icon) in &icons_.0 {
            by_key.insert(k.clone(), shape(icon));
        }
        let facets = icons_
            .get("mark")
            .map(|m| {
                m.shapes
                    .iter()
                    .map(|s| {
                        shape(&icons::Icon {
                            name: m.name.clone(),
                            w: m.w,
                            h: m.h,
                            shapes: vec![s.clone()],
                        })
                    })
                    .collect()
            })
            .unwrap_or_default();
        Shapes { by_key, facets }
    }

    fn get(&self, k: &str) -> Option<&Shape> {
        self.by_key.get(k)
    }
}

fn shape(icon: &icons::Icon) -> Shape {
    Shape {
        tris: icons::tessellate(icon),
        w: icon.w,
        rings: icon
            .shapes
            .iter()
            .flatten()
            .map(|r| r.iter().map(|p| Vec2::from(*p)).collect())
            .collect(),
    }
}

/// A ladder rung: the gun's name, its game's short name, its class icon,
/// the game's own kill icon when at hand and preferred.
#[derive(Clone, Debug)]
pub struct Rung {
    pub name: &'static str,
    pub game: &'static str,
    pub class: &'static str,
    pub id: &'static str,
    pub game_icon: Option<WeaponIcon>,
}

/// The look book's ladder (our weapon ids): a cross-game climb.
pub const LADDER: [(&str, &str, &str, &str); 14] = [
    ("Glock-18", "CS:S", "pistol", "cs_source:weapon_glock"),
    ("Desert Eagle", "CS:S", "pistol", "cs_source:weapon_deagle"),
    (".357 Magnum", "HL2:DM", "revolver", "hl2dm:weapon_357"),
    ("M3 Super 90", "CS:S", "shotgun", "cs_source:weapon_m3"),
    ("SPAS-12", "HL2:DM", "shotgun", "hl2dm:weapon_shotgun"),
    ("M249", "CS:S", "lmg", "cs_source:weapon_m249"),
    ("MP5A4", "Combat Arms", "smg", "combat_arms:mp5a4"),
    ("P90", "CS:S", "smg", "cs_source:weapon_p90"),
    ("M4A1", "Combat Arms", "rifle", "combat_arms:m4a1"),
    ("AK-47", "CS:S", "rifle", "cs_source:weapon_ak47"),
    ("AR2", "HL2:DM", "energy", "hl2dm:weapon_ar2"),
    ("AWP", "CS:S", "sniper", "cs_source:weapon_awp"),
    ("RPG", "HL2:DM", "launcher", "hl2dm:weapon_rpg"),
    ("Knife", "CS:S", "knife", "cs_source:weapon_knife"),
];

fn class_name(k: &str) -> &'static str {
    match k {
        "pistol" => "Pistol",
        "revolver" => "Revolver",
        "smg" => "SMG",
        "rifle" => "Rifle",
        "lmg" => "LMG",
        "shotgun" => "Shotgun",
        "sniper" => "Sniper",
        "launcher" => "Launcher",
        "energy" => "Energy",
        "crossbow" => "Crossbow",
        "grenade" => "Grenade",
        "knife" => "Knife",
        _ => "Blunt",
    }
}

fn mono(px: f32) -> (Face, f32) {
    (Face::Role(Role::Mono), px)
}

// ---------------------------------------------------------------------------
// A. Ladder

/// The ladder's beat: 1.6 s, landing at .95 s.
pub const BEAT: f32 = 1.6;
pub const LAND: f32 = 0.95;
const N: usize = 14;
/// The climb's loop (13 beats to the knife, a 2.2 s hold, a .55 s fade).
pub const LADDER_PERIOD: f32 = (N as f32 - 1.0) * BEAT + LAND + 2.2 + 0.55;
pub const LADDER_STILL: f32 = (N as f32 - 1.0) * BEAT + LAND + 1.2;

/// Keyframes `[time, plates]`, sine in-out into each.
fn kf(k: &[(f32, f32)], tau: f32) -> f32 {
    if tau <= k[0].0 {
        return k[0].1;
    }
    for w in k.windows(2) {
        if tau <= w[1].0 {
            let p = (tau - w[0].0) / (w[1].0 - w[0].0);
            return lerp(w[0].1, w[1].1, 0.5 - 0.5 * (std::f32::consts::PI * p).cos());
        }
    }
    k[k.len() - 1].1
}

const KB: [(f32, f32); 3] = [(0.0, 0.0), (0.55, 1.04), (LAND, 1.0)];
const KC: [(f32, f32); 3] = [(0.0, 0.0), (0.65, 1.02), (LAND, 1.0)];
const KK: [(f32, f32); 3] = [(0.0, 0.0), (0.58, 1.05), (LAND, 1.0)];

/// Where the climb is at `tt` (`b` the brackets' plate, `c` the
/// camera's, `k` the beat).
#[derive(Clone, Copy, Debug)]
pub struct LadderState {
    pub b: f32,
    pub c: f32,
    pub k: usize,
    pub tau: f32,
    pub land: f32,
    pub fin: bool,
    pub arr: f32,
}

pub fn ladder_state(tt: f32) -> LadderState {
    let k = ((tt / BEAT).floor() as usize).min(N - 1);
    let tau = tt - k as f32 * BEAT;
    if k == 0 {
        return LadderState {
            b: 0.0,
            c: 0.0,
            k: 0,
            tau: 9.0,
            land: 9.0,
            fin: false,
            arr: -1.0,
        };
    }
    let fin = k == N - 1;
    let km1 = (k - 1) as f32;
    LadderState {
        b: km1 + kf(if fin { &KK } else { &KB }, tau),
        c: km1 + kf(&KC, tau),
        k,
        tau,
        land: tau - LAND,
        fin,
        arr: if fin { tau - LAND } else { -1.0 },
    }
}

struct Geo {
    s: f32,
    ph: f32,
    pw: f32,
    cx: f32,
    ay: f32,
}

struct Plate {
    y: f32,
    top: f32,
    cx: f32,
    q: [Vec2; 4],
}

impl Geo {
    fn rail(&self, side: f32, y: f32) -> f32 {
        self.cx + side * self.pw / 2.0 + (self.ay - y) * TAN
    }
    fn plate(&self, i: f32, c: f32) -> Plate {
        let y = self.ay - (i - c) * self.s;
        let (top, bot) = (y - self.ph / 2.0, y + self.ph / 2.0);
        Plate {
            y,
            top,
            cx: self.cx + (self.ay - y) * TAN,
            q: [
                v(self.rail(-1.0, top), top),
                v(self.rail(1.0, top), top),
                v(self.rail(1.0, bot), bot),
                v(self.rail(-1.0, bot), bot),
            ],
        }
    }
}

/// An icon's box on a plate: centre, half sizes, units to px.
struct IconBox {
    cx: f32,
    cy: f32,
    hw: f32,
    hh: f32,
    us: f32,
}

const CROWN: &str = "crown";

/// A. The ladder straight on (module doc of `menu_backdrops`).
pub fn ladder(p: &mut Paint, sh: &Shapes, pal: Pal, rungs: &[Rung], w: f32, h: f32, t: f32, d: f32, still: bool) {
    let tt = if still { LADDER_STILL } else { modf(t, LADDER_PERIOD) };
    let g = Geo {
        s: h * 0.17,
        ph: h * 0.17 * 0.6,
        pw: w * 0.2,
        cx: w * 0.585,
        ay: h * 0.6,
    };
    let st = ladder_state(tt);
    let (cc, b) = (st.c, st.b);
    let hair = d.round().max(1.0);
    let a = if still {
        1.0
    } else {
        c01(tt / 0.45) * (1.0 - eio((tt - (LADDER_PERIOD - 0.55)) / 0.55))
    };
    if a <= 0.0 {
        return;
    }
    p.alpha = a;
    let icon_box = |i: usize, c: f32| -> IconBox {
        let pl = g.plate(i as f32, c);
        let r = &rungs[i];
        let iw0 = sh.get(r.class).map_or(24.0, |s| s.w);
        let ih = g.ph * 0.5;
        let us = (ih / 16.0).min(g.pw * 0.6 / iw0);
        let mut iw = iw0 * us;
        if r.game_icon.is_some() {
            iw = (g.pw * 0.6).min(16.0 * us * 2.6);
        }
        IconBox {
            cx: pl.cx,
            cy: pl.y,
            hw: iw / 2.0,
            hh: 8.0 * us,
            us,
        }
    };
    // Rails, fading at both ends.
    let r_top = (-h * 0.05).max(g.plate((N - 1) as f32, cc).top - g.ph * 0.9);
    let rail_stops = [
        (0.0, pal.fg(0.0)),
        (0.18, pal.fg(0.26)),
        (0.85, pal.fg(0.26)),
        (1.0, pal.fg(0.0)),
    ];
    for sd in [-1.0, 1.0] {
        p.line_grad(
            v(g.rail(sd, r_top), r_top),
            v(g.rail(sd, h * 1.05), h * 1.05),
            (1.2 * d).max(1.0),
            v(0.0, 1.0 / h),
            0.0,
            &rail_stops,
        );
    }
    let mut fin_plate: Option<Plate> = None;
    let lo = (cc - 4.0).floor().max(0.0) as usize;
    let hi = ((cc + 4.0).ceil() as usize).min(N - 1);
    for i in lo..=hi {
        let pl = g.plate(i as f32, cc);
        if pl.top + g.ph < -h * 0.05 || pl.top > h * 1.05 {
            continue;
        }
        let vf = c01((pl.y + g.ph * 0.2) / (h * 0.2)) * c01((h * 1.06 - pl.y) / (h * 0.22));
        let hl = c01(1.0 - (i as f32 - b).abs() * 1.4);
        let last = i == N - 1;
        let done = (i as f32) < b - 0.5;
        if vf <= 0.0 {
            continue;
        }
        p.alpha = a * vf;
        p.fill(
            &pl.q,
            if last {
                pal.acc(0.12 + 0.1 * hl)
            } else {
                pal.fg(if done { 0.05 } else { 0.03 + 0.05 * hl })
            },
        );
        p.stroke(&pl.q, hair, if last { pal.acc(0.85) } else { pal.fg(0.16 + 0.4 * hl) });
        if last {
            // The finish chequer at both ends, cells on the slant.
            let cs = g.ph / 4.0;
            for sd in [-1.0f32, 1.0] {
                for r in 0..4 {
                    for col in 0..2 {
                        if (r + col) % 2 == 1 {
                            continue;
                        }
                        let y0 = pl.top + r as f32 * cs;
                        let y1 = y0 + cs;
                        let o0 = if sd < 0.0 {
                            col as f32 * cs
                        } else {
                            -(col as f32 + 1.0) * cs
                        };
                        let o1 = o0 + cs;
                        p.fill(
                            &[
                                v(g.rail(sd, y0) + o0, y0),
                                v(g.rail(sd, y0) + o1, y0),
                                v(g.rail(sd, y1) + o1, y1),
                                v(g.rail(sd, y1) + o0, y1),
                            ],
                            pal.acc(0.7),
                        );
                    }
                }
            }
            // The crown over the plate's centre, on the slant.
            if let Some(cr) = sh.get(CROWN) {
                let chh = g.ph * 0.42;
                let csx = chh / 14.0;
                let cby = pl.top - g.ph * 0.2;
                let cbx = pl.cx + (pl.y - cby) * TAN;
                let (ox, oy) = (cbx - 11.0 * csx, cby - 14.0 * csx);
                p.tris(
                    &cr.tris,
                    |q| v(ox + csx * (q.x - TAN * q.y + TAN * 14.0), oy + csx * q.y),
                    pal.acc(0.95),
                );
            }
        }
        // The gun.
        let r = &rungs[i];
        let ib = icon_box(i, cc);
        let ia = if last {
            0.55 + 0.45 * hl
        } else {
            (if done { 0.32 } else { 0.38 }) + 0.62 * hl
        };
        game_or_class_icon(p, sh, pal, r, &ib, ia);
        // The rung's number left of the rail.
        let (face, px) = mono((h * 0.019).round());
        p.text(
            format!("{:02}", i + 1),
            face,
            px,
            if last { pal.acc(0.95) } else { pal.fg(0.32 + 0.6 * hl) },
            v(g.rail(-1.0, pl.y) - 14.0 * d, pl.y + h * 0.0065),
            Align::Right,
            0.0,
        );
        if last && hl < 0.5 {
            let (face, px) = mono((h * 0.017).round());
            p.text(
                "FINAL \u{2022} KNIFE",
                face,
                px,
                pal.acc(0.95),
                v(g.rail(1.0, pl.y) + 16.0 * d, pl.y + h * 0.006),
                Align::Left,
                0.0,
            );
        }
        if last {
            fin_plate = Some(pl);
        }
    }
    p.alpha = a;
    // A glint across the knife plate on arrival.
    if let Some(fp) = &fin_plate
        && st.arr >= 0.0
    {
        let gq = (st.arr - 0.18) / 0.75;
        if gq > 0.0 && gq < 1.0 {
            let gx = lerp(g.rail(-1.0, fp.y) - g.ph, g.rail(1.0, fp.y) + g.ph, eio(gq));
            let k = (std::f32::consts::PI * gq).sin();
            // Along x on the slant: t = x + tan (y - y_plate).
            let c = -TAN * fp.y;
            p.fill_grad(
                &fp.q,
                v(1.0, TAN),
                c,
                &[
                    (gx - g.ph * 0.9, pal.fg(0.0)),
                    (gx, pal.fg(0.35 * k)),
                    (gx + g.ph * 0.9, pal.fg(0.0)),
                ],
            );
        }
    }
    // The brackets glide between plates and lock with a gentle zoom.
    let i0 = (b.floor() as usize).min(N - 2);
    let fr = b - i0 as f32;
    let (b0, b1) = (icon_box(i0, cc), icon_box(i0 + 1, cc));
    let (bx, by) = (lerp(b0.cx, b1.cx, fr), lerp(b0.cy, b1.cy, fr));
    let pad = g.ph * 0.2;
    let (bw, bh) = (lerp(b0.hw, b1.hw, fr) + pad, lerp(b0.hh, b1.hh, fr) + pad);
    let zs = (if st.fin { 0.08 } else { 0.06 }) * (std::f32::consts::PI * c01(st.tau / LAND)).sin();
    let cl = g.ph * 0.2;
    let mut brackets = |p: &mut Paint, s: f32, al: f32, lw: f32| {
        for q in [(-1.0f32, -1.0f32), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)] {
            let (px, py) = (bx + q.0 * bw * s, by + q.1 * bh * s);
            p.line_ext(v(px - q.0 * cl, py), v(px, py), lw, lw * 0.5, pal.acc(al));
            p.line_ext(v(px, py), v(px, py - q.1 * cl), lw, lw * 0.5, pal.acc(al));
        }
    };
    brackets(p, 1.0 + zs, 1.0, (1.6 * d).max(1.5));
    if st.arr >= 0.0 {
        let aq = c01(st.arr / 0.7);
        if aq < 1.0 {
            brackets(p, 1.0 + 0.55 * e_out(aq), 1.0 - aq, (1.2 * d).max(1.0));
        }
        let aq2 = c01((st.arr - 0.25) / 0.8);
        if aq2 > 0.0 && aq2 < 1.0 {
            brackets(p, 1.0 + 0.55 * e_out(aq2), 0.6 * (1.0 - aq2), d.max(1.0));
        }
    }
    // The current plate's label.
    let kk = (b.round() as usize).min(N - 1);
    let pk = g.plate(kk as f32, cc);
    let la = c01(1.0 - (b - kk as f32).abs() * 2.4);
    if la > 0.0 {
        p.alpha = a * la;
        let (lx, ly) = (g.rail(1.0, pk.top) + 18.0 * d, pk.y - g.ph * 0.28);
        let last_k = kk == N - 1;
        let r = &rungs[kk];
        p.text(
            format!("{:02}", kk + 1),
            Face::Role(Role::DisplayBold),
            (h * 0.064).round(),
            if last_k { pal.acc(1.0) } else { pal.fg(1.0) },
            v(lx, ly + h * 0.012),
            Align::Left,
            0.0,
        );
        let (face, px) = mono((h * 0.019).round());
        p.text(
            if last_k {
                "KNIFE".to_string()
            } else {
                r.name.to_uppercase()
            },
            face.clone(),
            px,
            pal.fg(0.85),
            v(lx, ly + h * 0.046),
            Align::Left,
            0.0,
        );
        let line = if last_k {
            "FINAL \u{2022} KNIFE WINS".to_string()
        } else {
            format!("{} \u{2022} {}", r.game, class_name(r.class)).to_uppercase()
        };
        p.text(
            line,
            face,
            px,
            if last_k { pal.acc(0.95) } else { pal.fg(0.45) },
            v(lx, ly + h * 0.074),
            Align::Left,
            0.0,
        );
        p.alpha = a;
    }
    // The whole climb: 14 ticks on the slant, the knife's crowned.
    let (mx, my0, my1, tw) = (w * 0.905, h * 0.76, h * 0.24, h * 0.03);
    let th = (2.0 * d).max((h * 0.011).round());
    for r in 0..N {
        let yy = lerp(my0, my1, r as f32 / (N - 1) as f32);
        let xx = mx + (h * 0.5 - yy) * TAN * 0.35;
        let lr = if st.k == 0 || st.land >= 0.0 {
            st.k as i32
        } else {
            st.k as i32 - 1
        };
        let (on, cur) = (r as i32 <= lr, r as i32 == lr);
        let q = [
            v(xx + th * TAN, yy - th / 2.0),
            v(xx + tw + th * TAN, yy - th / 2.0),
            v(xx + tw, yy + th / 2.0),
            v(xx, yy + th / 2.0),
        ];
        if r == N - 1 {
            p.fill(&q, pal.acc(if on { 1.0 } else { 0.25 }));
            p.stroke(&q, hair, pal.acc(0.9));
            if let Some(cr) = sh.get(CROWN) {
                let s2 = h * 0.012 / 14.0;
                let (ox, oy) = (xx + tw / 2.0 - 11.0 * s2, yy - th / 2.0 - h * 0.018);
                p.tris(&cr.tris, |c| v(ox + c.x * s2, oy + c.y * s2), pal.acc(0.95));
            }
        } else {
            p.fill(
                &q,
                if cur {
                    pal.acc(1.0)
                } else if on {
                    pal.fg(0.8)
                } else {
                    pal.fg(0.14)
                },
            );
        }
    }
    let (face, px) = mono((h * 0.015).round());
    p.text(
        format!("RUNG {:02} / 14", (b.round() as usize).min(N - 1) + 1),
        face,
        px,
        pal.fg(0.45),
        v(mx, my0 + h * 0.045),
        Align::Left,
        0.0,
    );
    p.alpha = 1.0;
}

/// A rung's gun: the game's own kill icon when at hand (a glyph or an
/// image), else the class's from the kit.
fn game_or_class_icon(p: &mut Paint, sh: &Shapes, pal: Pal, r: &Rung, ib: &IconBox, ia: f32) {
    match &r.game_icon {
        Some(WeaponIcon::Glyph { font, ch, .. }) => {
            // The glyph about as tall as the box (icon fonts fill their em).
            let px = (ib.hh * 2.0 * 1.25).round();
            p.text(
                ch.to_string(),
                Face::Font(font.clone()),
                px,
                pal.fg(ia),
                v(ib.cx, ib.cy + px * 0.3),
                Align::Centre,
                0.0,
            );
        }
        Some(WeaponIcon::Image { image, size }) => {
            let k = ((ib.hw * 2.0) / size.x.max(1.0)).min((ib.hh * 2.0) / size.y.max(1.0));
            let s = *size * k;
            let mut c = pal.fg(ia);
            c.alpha *= p.alpha;
            p.images.push(ImageItem {
                image: image.clone(),
                min: v(ib.cx - s.x / 2.0, ib.cy - s.y / 2.0),
                size: s,
                color: c,
            });
        }
        None => {
            if let Some(s) = sh.get(r.class) {
                let (ox, oy, us) = (ib.cx - ib.hw, ib.cy - ib.hh, ib.us);
                p.tris(&s.tris, |q| v(ox + q.x * us, oy + q.y * us), pal.fg(ia));
            }
        }
    }
}

// ---------------------------------------------------------------------------
// B. Assembly

const SEQ: [(&str, &str); 8] = [
    ("revolver", "HL2:DM"),
    ("smg", "Combat Arms"),
    ("shotgun", "HL2:DM"),
    ("rifle", "Combat Arms"),
    ("lmg", "CS:S"),
    ("sniper", "CS:S"),
    ("crossbow", "HL2:DM"),
    ("knife", "CS:S"),
];
const SL: f32 = 3.4;
pub const ASSEMBLY_STILL: f32 = 12.6;

fn gab(game: &str) -> &'static str {
    match game {
        "CS:S" => "CS:S",
        "HL2:DM" => "HL2:DM",
        _ => "CA",
    }
}

/// B. A gun drafted on its grid, traced, hatched, filled, dimensioned and
/// filed on a shelf.
pub fn assembly(p: &mut Paint, sh: &Shapes, pal: Pal, w: f32, h: f32, t: f32, d: f32) {
    let period = SEQ.len() as f32 * SL;
    let tt = modf(t, period);
    let j = ((tt / SL).floor() as usize).min(SEQ.len() - 1);
    let tau = tt - j as f32 * SL;
    let Some(gs) = sh.get(SEQ[j].0) else { return };
    let gw = gs.w;
    let sq = |x: f32, y: f32| x - (8.0 - y) * TAN;
    // The scan's range and the guides' heights.
    let (mut q0, mut q1) = (f32::MAX, f32::MIN);
    let mut ys: Vec<f32> = Vec::new();
    let mut edges: Vec<(Vec2, Vec2)> = Vec::new();
    for r in &gs.rings {
        for i in 0..r.len() {
            let (a, b) = (r[i], r[(i + 1) % r.len()]);
            edges.push((a, b));
            if a.y == b.y && (b.x - a.x).abs() >= 4.0 && !ys.contains(&a.y) {
                ys.push(a.y);
            }
            for q in [a, b] {
                q0 = q0.min(sq(q.x, q.y));
                q1 = q1.max(sq(q.x, q.y));
            }
        }
    }
    let (q0, q1) = (q0 - 0.6, q1 + 0.6);
    let u = (w * 0.36 / 54.0).min(h * 0.40 / 16.0);
    let (cx, cy) = (w * 0.665, h * 0.40);
    let (ox, oy) = (cx - gw * u / 2.0, cy - 8.0 * u);
    let x_ = |q: f32| ox + q * u;
    let y_ = |q: f32| oy + q * u;
    let hair = d.round().max(1.0);
    let hp = if hair as i32 % 2 == 1 { 0.5 } else { 0.0 };
    // Exit: a 12° wipe erasing left to right.
    let ex = c01((tau - 2.95) / 0.4);
    let fe = lerp(q0, q1, eio(ex));
    let mut pushed = 0;
    if ex > 0.0 {
        // Keep x >= X(fe) + 14u tan - tan (y - Y(-6)).
        let (x0, y0) = (x_(fe) + 14.0 * u * TAN, y_(-6.0));
        p.push(Half {
            n: v(1.0, TAN),
            c: -(x0 + TAN * y0),
        });
        pushed += 1;
    }
    // 1. The grid: a dot a unit, then the guides.
    let ga = e_out(tau / 0.45);
    let ds = (d * 1.2).round().max(1.0);
    let dot = pal.fg(0.16 * ga);
    for gy in 0..=16 {
        for gx in 0..=gw as i32 {
            p.rect(
                x_(gx as f32).round() - (ds as i32 >> 1) as f32,
                y_(gy as f32).round() - (ds as i32 >> 1) as f32,
                ds,
                ds,
                dot,
            );
        }
    }
    let guide = pal.fg(0.075 * ga);
    let gl = lerp(0.0, w * 0.16, e_out(tau / 0.7));
    for &yv in &ys {
        let yy = y_(yv).round() + hp;
        p.line(v(x_(0.0) - gl, yy), v(x_(gw) + gl, yy), hair, guide);
    }
    for uu in [0.0, gw] {
        let xx = x_(uu).round() + hp;
        p.line(v(xx, y_(0.0) - gl * 0.5), v(xx, y_(16.0) + gl * 0.5), hair, guide);
    }
    // 3. Hatch on the slant behind a second, slower scan; 4. the fill.
    let hpz = c01((tau - 1.0) / 1.0);
    let fh = lerp(q0, q1, eio(hpz));
    let fa = 0.86 * e_out((tau - 1.85) / 0.45);
    if hpz > 0.0 && fa < 0.86 {
        // Left of the hatch front: x_icon <= fh + tan (8 - y_icon).
        p.push(Half {
            n: v(-1.0 / u, -TAN / u),
            c: fh + 8.0 * TAN + ox / u + TAN * oy / u,
        });
        let st = (3.0 * d).max(u * 0.75);
        let col = pal.fg(0.34 * (1.0 - fa));
        // Line k runs from (k + 16u tan, Y(0)) to (k, Y(16)): its points
        // have x - (Y(16) - y) tan = k. Each triangle takes the lines whose
        // k crosses it.
        let k0 = x_(-4.0);
        let kof = |q: Vec2| q.x - (y_(16.0) - q.y) * TAN;
        for tri in &gs.tris {
            let q = tri.map(|c| v(ox + c.x * u, oy + c.y * u));
            let (lo, hi) = q
                .iter()
                .fold((f32::MAX, f32::MIN), |(lo, hi), c| (lo.min(kof(*c)), hi.max(kof(*c))));
            let first = ((lo - k0 - hair) / st).ceil().max(0.0) as i32;
            let last = ((hi - k0 + hair) / st).floor() as i32;
            if last < first {
                continue;
            }
            let n = p.push_convex(&q);
            for i in first..=last {
                let k = k0 + i as f32 * st;
                p.line(
                    v((k + 16.0 * u * TAN).round(), y_(0.0)),
                    v(k.round(), y_(16.0)),
                    hair,
                    col,
                );
            }
            p.pop(n);
        }
        p.pop(1);
    }
    if fa > 0.0 {
        p.tris(&gs.tris, |c| v(ox + c.x * u, oy + c.y * u), pal.fg(fa));
    }
    // 2. The scan traces the outline.
    let sp = c01((tau - 0.3) / 1.15);
    let f = lerp(q0, q1, eio(sp));
    let ow = (1.5 * d).round().max(1.0);
    for &(a, b) in &edges {
        let (s0, s1) = (sq(a.x, a.y), sq(b.x, b.y));
        if s0.min(s1) > f {
            continue;
        }
        let (mut p0, mut p1) = (a, b);
        if s0.max(s1) > f {
            let tc = (f - s0) / (s1 - s0);
            if s1 > s0 {
                p1 = a.lerp(b, tc);
            } else {
                p0 = a.lerp(b, tc);
            }
        }
        let m = |c: Vec2| v((ox + c.x * u).round() + hp, (oy + c.y * u).round() + hp);
        p.line_ext(m(p0), m(p1), ow, ow * 0.5, pal.fg(0.95));
    }
    // The pen: an accent hairline on the slant at the scan's front.
    if sp > 0.0 && sp < 1.0 {
        let pa = (std::f32::consts::PI * sp).sin();
        p.line(
            v(x_(f) + 11.0 * u * TAN, y_(-3.0)),
            v(x_(f) - 11.0 * u * TAN, y_(19.0)),
            (1.5 * d).max(1.5),
            pal.acc(pa),
        );
    }
    // 5. Dimensions.
    let da = e_out((tau - 1.55) / 0.4);
    if da > 0.0 {
        let col = pal.fg(0.42 * da);
        let yd = (y_(16.0) + u * 2.2).round() + hp;
        let xd = (x_(0.0) - u * 2.2).round() + hp;
        let tk = u * 0.6;
        let xr = x_(0.0) + (x_(gw) - x_(0.0)) * da;
        let (fs, mid) = ((h * 0.016).round(), (x_(0.0) + x_(gw)) / 2.0);
        let label = format!("{} U", gw as i32);
        let gap = label.len() as f32 * fs * 0.62 / 2.0 + 5.0 * d;
        // The run, broken where its label sits.
        if xr > mid - gap {
            p.line(v(x_(0.0), yd), v(mid - gap, yd), hair, col);
            if xr > mid + gap {
                p.line(v(mid + gap, yd), v(xr, yd), hair, col);
            }
        } else {
            p.line(v(x_(0.0), yd), v(xr, yd), hair, col);
        }
        p.line(v(x_(0.0), yd - tk), v(x_(0.0), yd + tk), hair, col);
        p.line(v(xr, yd - tk), v(xr, yd + tk), hair, col);
        let yb = y_(16.0) - (y_(16.0) - y_(0.0)) * da;
        p.line(v(xd, y_(16.0)), v(xd, yb), hair, col);
        p.line(v(xd - tk, y_(16.0)), v(xd + tk, y_(16.0)), hair, col);
        p.line(v(xd - tk, yb), v(xd + tk, yb), hair, col);
        p.text(
            label,
            Face::Role(Role::Mono),
            fs,
            pal.fg(0.6 * da),
            v(mid, yd + h * 0.0055),
            Align::Centre,
            0.0,
        );
        p.text(
            "16 U",
            Face::Role(Role::Mono),
            fs,
            pal.fg(0.6 * da),
            v(xd - tk - 4.0 * d, (y_(0.0) + y_(16.0)) / 2.0 + h * 0.0055),
            Align::Right,
            0.0,
        );
    }
    p.pop(pushed);
    // The label: class and game.
    let la = e_out((tau - 1.9) / 0.35) * (1.0 - c01((tau - 2.95) / 0.25));
    let (lx, ly) = (x_(0.0), y_(16.0) + u * 2.2 + h * 0.085);
    if la > 0.0 {
        p.alpha = la;
        p.text(
            class_name(SEQ[j].0).to_uppercase(),
            Face::Role(Role::DisplayBold),
            (h * 0.042).round(),
            pal.fg(1.0),
            v(lx, ly),
            Align::Left,
            0.0,
        );
        let rungs: Vec<usize> = LADDER
            .iter()
            .enumerate()
            .filter(|(_, l)| l.2 == SEQ[j].0)
            .map(|(i, _)| i + 1)
            .collect();
        let rg = match rungs.as_slice() {
            [] => String::new(),
            [one] => format!(" \u{2022} LADDER RUNG {one:02}"),
            [first, .., last] => format!(" \u{2022} LADDER RUNGS {first:02}\u{2013}{last:02}"),
        };
        p.text(
            format!("{}{rg}", SEQ[j].1.to_uppercase()),
            Face::Role(Role::Mono),
            (h * 0.017).round(),
            pal.fg(0.55),
            v(lx, ly + h * 0.032),
            Align::Left,
            0.0,
        );
        p.rect(lx - 12.0 * d, ly - h * 0.034, 3.0 * d, h * 0.072, pal.acc(1.0));
        p.alpha = 1.0;
    }
    // The header readout.
    let phase = if tau < 0.3 {
        "DRAFT"
    } else if tau < 1.45 {
        "TRACE"
    } else if tau < 1.9 {
        "HATCH"
    } else if tau < 2.95 {
        "FILL \u{2022} DIMENSION"
    } else {
        "FILE"
    };
    p.text(
        format!("BUILD {:02} / {:02} \u{2022} {phase}", j + 1, SEQ.len()),
        Face::Role(Role::Mono),
        (h * 0.016).round(),
        pal.fg(0.42),
        v(x_(0.0), y_(0.0) - u * 2.4),
        Align::Left,
        0.0,
    );
    // 6. The shelf: every gun built this cycle, filed under its game.
    let n = SEQ.len();
    let (sx0, sx1, sy, shh) = (w * 0.49, w * 0.93, h * 0.70, h * 0.03);
    let sw = (sx1 - sx0) / n as f32;
    for k2 in 0..n {
        let bx = sx0 + k2 as f32 * sw;
        let done = k2 < j || (k2 == j && tau > 2.95);
        let Some(ic) = sh.get(SEQ[k2].0) else { continue };
        let us = (shh / 16.0).min((sw - 14.0 * d) / ic.w);
        let iw = ic.w * us;
        let cur = k2 == j;
        let fin = if cur { e_out((tau - 2.95) / 0.3) } else { 1.0 };
        p.rect(
            bx.round(),
            (sy + shh + h * 0.012).round(),
            (sw - 8.0 * d).round(),
            hair,
            pal.fg(0.09),
        );
        let (ix, iy) = (bx + (sw - 8.0 * d) / 2.0 - iw / 2.0, sy + shh - 16.0 * us);
        if done {
            p.tris(
                &ic.tris,
                |c| v(ix + c.x * us, iy + c.y * us),
                pal.fg((if cur { 0.95 } else { 0.5 }) * fin),
            );
        } else {
            p.dashed_rect(
                ix.round() + hp,
                iy.round() + hp,
                iw.round(),
                (16.0 * us).round(),
                hair,
                2.0 * d,
                3.0 * d,
                pal.fg(0.14),
            );
        }
        p.text(
            gab(SEQ[k2].1),
            Face::Role(Role::Mono),
            (h * 0.0135).round(),
            pal.fg(if cur { 0.75 } else { 0.32 }),
            v(bx.round(), (sy + shh + h * 0.036).round()),
            Align::Left,
            0.0,
        );
        if cur {
            p.rect(
                bx.round(),
                (sy + shh + h * 0.012).round() - hair,
                ((sw - 8.0 * d) * c01(tau / 2.95)).round(),
                hair * 2.0,
                pal.acc(1.0),
            );
        }
    }
}

// ---------------------------------------------------------------------------
// C. Ident

const VC: f32 = 658.0;
const UC: f32 = 624.0;
const BASE: [f32; 6] = [0.88, 0.72, 0.85, 0.9, 0.74, 0.8];
const T0: f32 = 0.4;
const T1: f32 = 2.2;
const LK: f32 = 0.9;
const OFF0: f32 = 13.6;
const OFF1: f32 = 15.3;
const SHEEN: [f32; 2] = [4.0, 10.0];
const SH: f32 = 2.8;
pub const IDENT_STILL: f32 = 7.0;
pub const IDENT_PERIOD: f32 = 16.0;

/// C. The Ident: the mark signs on facet by facet on one 12° scan.
pub fn ident(p: &mut Paint, sh: &Shapes, pal: Pal, w: f32, h: f32, t: f32, d: f32) {
    let facets = &sh.facets;
    if facets.is_empty() {
        return;
    }
    // Each facet's scan span and centre, in the look book's units (the
    // mark's y + 210).
    let spans: Vec<(f32, f32, Vec2)> = facets
        .iter()
        .map(|f| {
            let (mut lo, mut hi, mut sum, mut m) = (f32::MAX, f32::MIN, Vec2::ZERO, 0.0);
            for r in &f.rings {
                for q in r {
                    let y = q.y + 210.0;
                    let s = q.x - (VC - y) * TAN;
                    lo = lo.min(s);
                    hi = hi.max(s);
                    sum += v(q.x, y);
                    m += 1.0;
                }
            }
            (lo, hi, sum / m)
        })
        .collect();
    let sq0 = spans.iter().map(|s| s.0).fold(f32::MAX, f32::min);
    let sq1 = spans.iter().map(|s| s.1).fold(f32::MIN, f32::max);
    let tt = modf(t, IDENT_PERIOD);
    let k = h * 0.5 / 896.0;
    let (ox, oy) = (w * 0.665 - UC * k, h * 0.45 - 448.0 * k);
    let x_ = |u: f32| ox + u * k;
    let y_ = |q: f32| oy + (q - 210.0) * k;
    let (yt, yb, yc) = (y_(210.0) - h * 0.035, y_(1106.0) + h * 0.035, y_(VC));
    let hair = d.round().max(1.0);
    let hp = if hair as i32 % 2 == 1 { 0.5 } else { 0.0 };
    let pad = 60.0;
    let f_on = lerp(sq0 - pad, sq1 + pad, eio((tt - T0) / (T1 - T0)));
    let f_off = lerp(sq0 - pad, sq1 + pad, eio((tt - OFF0) / (OFF1 - OFF0)));
    let (on, off, gone) = (tt >= T0, tt >= OFF0, tt >= OFF1);
    let hold = c01((tt - T1) / 1.2) * (1.0 - c01((tt - OFF0) / 0.6));
    let (bx0, bx1, by0, by1) = (x_(-48.0), x_(1296.0), y_(210.0) - h * 0.02, y_(1106.0) + h * 0.02);
    // Left of the scan's front at `fq`: x <= X(fq) + (yc - y) tan.
    let front = |fq: f32| Half {
        n: v(-1.0, -TAN),
        c: x_(fq) + yc * TAN,
    };
    // 1. The construction grid, drawn in by the scan, gone after the lock.
    let ga = 0.075 * (1.0 - eio((tt - T1 - 0.1) / 1.5));
    if on && ga > 0.0 {
        p.push(front(f_on));
        let n = p.push_convex(&[v(bx0, by0), v(bx1, by0), v(bx1, by1), v(bx0, by1)]);
        let col = pal.fg(ga);
        let mut uu = -48.0;
        while uu <= 1296.0 + 400.0 {
            let (xa, xb) = (x_(uu) + (yc - by0) * TAN, x_(uu) + (yc - by1) * TAN);
            p.line(v(xa.round() + hp, by0), v(xb.round() + hp, by1), hair, col);
            uu += 112.0;
        }
        let mut vv = 210.0;
        while vv <= 1106.0 {
            let yy = y_(vv).round() + hp;
            p.line(v(bx0, yy), v(bx1, yy), hair, col);
            vv += 112.0;
        }
        p.pop(n + 1);
    }
    // 2. The bloom behind the mark: rises on the lock, settles low.
    let lp = (tt - T1) / LK;
    let bl = if on && !gone {
        (0.13 * (-lp.max(0.0) * 2.2).exp() * c01(lp * 6.0) + 0.05 * hold) * (1.0 - c01((tt - OFF0) / 1.4))
    } else {
        0.0
    };
    if bl > 0.002 {
        let stops: Vec<(f32, LinearRgba)> = (0..=8)
            .map(|q| {
                let r = q as f32 / 8.0;
                (r, pal.acc(bl * (-r * r * 3.2).exp() * (1.0 - r)))
            })
            .collect();
        p.radial(v(x_(UC), y_(VC)), h * 0.42, 1.35, &stops, 48);
    }
    // 3. The facets sign on along the scan and settle inward, breathe
    // through the hold, ease outward on the sign-off.
    if on && !gone {
        let pushed = if !off {
            p.push(front(f_on));
            1
        } else {
            0
        };
        for (i, f) in facets.iter().enumerate() {
            let (lo, hi, cu) = spans[i];
            let span = (hi - lo).max(1.0);
            let lit = if off { 1.0 } else { c01((f_on - lo) / span) };
            let e = e_out(lit);
            let eo = if off { eio(c01((f_off - lo) / span)) } else { 0.0 };
            let dx = (cu.x - UC) * (0.06 * (1.0 - e) + 0.05 * eo);
            let dy = (cu.y - VC) * (0.06 * (1.0 - e) + 0.05 * eo);
            let br = 0.05 * hold * (std::f32::consts::TAU * (tt / 7.0) - cu.x / 420.0).sin();
            let a = ((BASE[i.min(5)] + br) * e * (1.0 - eo)).max(0.0);
            if a <= 0.003 {
                continue;
            }
            // Mark units (y as the icon's) to the canvas.
            let (mx, my) = (ox + dx * k, oy + dy * k);
            let m = |q: Vec2| v(mx + q.x * k, my + q.y * k);
            p.tris(&f.tris, m, pal.fg(a));
        }
        // 4. The sheen: a soft band with a bright core across the facets
        // only, laid by the composite over what is painted (`Paint::sheen`:
        // per pixel, cheaper than cutting every facet into its bands).
        if !off {
            for s0 in SHEEN {
                let pr = (tt - s0) / SH;
                if pr <= 0.0 || pr >= 1.0 {
                    continue;
                }
                let cx = lerp(-260.0, 1500.0, eio(pr));
                let kk = (std::f32::consts::PI * pr).sin();
                // t = x_m + tan (y_lb - VC) in the mark's units (no drift:
                // the facets' settle is over by then), as 0-1 across the
                // band's 520 units.
                let c = -ox / k + TAN * (-oy / k + 210.0 - VC);
                let s = 520.0;
                p.sheen = Some([1.0 / (k * s), TAN / (k * s), (c - (cx - 260.0)) / s, kk]);
            }
        }
        // 5. The lock: an outline flash with a soft glow.
        if (0.0..1.0).contains(&lp) && !off {
            let fade = (1.0 - lp) * (1.0 - lp);
            for f in facets {
                for r in &f.rings {
                    let pts: Vec<Vec2> = r.iter().map(|q| v(ox + q.x * k, oy + q.y * k)).collect();
                    p.stroke(&pts, 6.0 * d, pal.acc(0.18 * (1.0 - lp)));
                    p.stroke(&pts, (1.5 * d).max(1.2), pal.acc(0.95 * fade));
                }
            }
        }
        p.pop(pushed);
    }
    // The scan: one accent hairline with a soft cobalt wake behind it.
    let scan = if tt >= T0 && tt < T1 + 0.05 {
        Some((f_on, (tt - T0) / (T1 - T0)))
    } else if off && !gone {
        Some((f_off, (tt - OFF0) / (OFF1 - OFF0)))
    } else {
        None
    };
    if let Some((fq, sp)) = scan {
        let al = (std::f32::consts::PI * c01(sp)).sin();
        let fx = x_(fq);
        let wk = h * 0.16;
        let (top, bot) = (v(fx + (yc - yt) * TAN, yt), v(fx + (yc - yb) * TAN, yb));
        p.fill_grad(
            &[top, bot, bot - v(wk, 0.0), top - v(wk, 0.0)],
            v(1.0, 0.0),
            0.0,
            &[(fx - wk, pal.acc(0.0)), (fx, pal.acc(0.16 * al))],
        );
        p.line(top, bot, (1.6 * d).max(1.5), pal.acc(al));
    }
    // Safe-frame brackets and the readout.
    let bl2 = h * 0.03;
    let col = pal.fg(0.4);
    for (x, y, sx, sy) in [
        (bx0, by0, 1.0, 1.0),
        (bx1, by0, -1.0, 1.0),
        (bx0, by1, 1.0, -1.0),
        (bx1, by1, -1.0, -1.0),
    ] {
        let (x, y) = (x.round() + hp, y.round() + hp);
        p.line_ext(v(x + sx * bl2, y), v(x, y), hair, hair * 0.5, col);
        p.line_ext(v(x, y), v(x, y + sy * bl2), hair, hair * 0.5, col);
    }
    let stx = if !on || gone {
        "STANDBY"
    } else if tt < T1 {
        "ACQUIRING"
    } else if off {
        "SIGNING OFF"
    } else {
        "LOCKED"
    };
    let locked = stx == "LOCKED";
    let fs = (h * 0.017).round();
    let ry = by1 + h * 0.045;
    let sqr = (h * 0.013).round();
    if locked {
        p.rect(bx0.round(), (ry - sqr).round(), sqr, sqr, pal.acc(1.0));
    } else {
        let (x, y) = (bx0.round() + hp, (ry - sqr).round() + hp);
        p.stroke(
            &[
                v(x, y),
                v(x + sqr - hair, y),
                v(x + sqr - hair, y + sqr - hair),
                v(x, y + sqr - hair),
            ],
            hair,
            pal.fg(0.5),
        );
    }
    p.text(
        format!("IDENT \u{2022} {stx}"),
        Face::Role(Role::Mono),
        fs,
        pal.fg(if locked { 0.7 } else { 0.45 }),
        v(bx0 + sqr + h * 0.012, ry),
        Align::Left,
        0.0,
    );
    p.text(
        "CH 01 \u{2022} ULTIMATE GUN GAME",
        Face::Role(Role::Mono),
        fs,
        pal.fg(0.4),
        v(bx1, ry),
        Align::Right,
        0.0,
    );
}

/// The look book's in/out curves, for tests.
pub fn eases(p: f32) -> (f32, f32) {
    (e_in(p), e_out(p))
}
