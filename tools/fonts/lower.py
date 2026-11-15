"""Lower case, accents, Latin-1 letters and the missing ASCII symbols for
UGG Display and UGG Mono (v0.5 draft).

Every lower-case letter is drawn from the capitals' own parts, re-measured
for the x-height (`lc`): the same squircle bowls (radius by bowl height),
the same open terminals cut 20 degrees off perpendicular on a straight
stretch of stroke, the same 70-degree diagonals. Where a letter has a
capital twin in this construction (c o s v w x z, the arm and leg of k)
the capital's drawing runs again at x-height with the lower-case stems.

Metrics (fractions of the cap height H): x-height `xr` (0.74 Display,
0.76 Mono), ascender 1.06 H, descender -0.286 H (Display) / -0.288 H
(Mono). Lower-case stems are a little lighter than the capitals' (`sl_k`)
and the horizontals lighter again (`tl_k`, most in Black so its e and s
keep open eyes).

Composites (accented letters) are outlines, not TrueType components: the
base glyph plus a mark (`MARKS`) centred on the base's ink and stacked
above the x-height or the cap height (`compose`).

Known shortcuts:
- Marks are centred on the base's ink box, not on per-glyph anchors (the
  double-storey a and the j/i stems are the exceptions handled here).
- eth is drawn as a d with a barred ascender; a true round-backed eth
  waits for a later pass.
- The germandbls is a B-like construction with an open foot.
- The lower case is unhinted; the Mono's hinting (`FontHinting::Enabled`)
  only snaps outlines, there are no instructions.
"""
import math

import geom2 as gm
from geom2 import box, rect, poly, stroke, union, diff, inter, BIG
from glyphs2 import glyph, tend, _C, _K, _V, _W, _X, _Z, _s_shape, _dot, _math_y, \
    _exclam, _question

LOWER = [chr(c) for c in range(0x61, 0x7B)]

# body widths in x-heights (Bold; heavier weights add `lc_wadd`)
LC_WIDTHS = dict(
    a=0.96, b=1.02, c=0.94, d=1.02, e=1.00, f=0.62, g=1.02, h=1.00, k=0.96,
    m=None, n=1.00, o=1.04, p=1.02, q=1.02, r=0.66, s=0.94, t=0.66, u=1.00,
    v=1.00, w=None, x=1.00, y=1.00, z=0.88, j=0.44, l=0.34, i=None,
)


def lc(g):
    """Switch a glyph drawer to the lower-case metrics."""
    P = g.P
    g.H = P.xh
    g.S, g.T = P.Sl, P.Tl
    g.Sr, g.Tr = g.S * P.round_gain, g.T * P.round_gain
    g.Tm = g.T * P.mid_gain
    g.asc, g.desc = P.asc, P.desc
    return g


def lc_width(P, wkey):
    x, Sl = P.xh, P.Sl
    k = wkey[3:]
    if P.mono:
        return P.body
    add = max(0.0, Sl - P.Sl_ref) * 1.2
    if k == "i" or k == "lplain":
        return Sl
    if k == "l":
        return Sl + LC_WIDTHS["l"] * x
    if k == "j":
        return Sl + LC_WIDTHS["j"] * x
    if k == "m":
        n_ctr = LC_WIDTHS["n"] * x * P.wscale + add - 2 * Sl
        return 3 * Sl + 2 * 0.80 * n_ctr
    if k == "w":
        td = Sl * P.wdiag
        hw = td / math.sin(math.radians(P.wang))
        run = x / math.tan(math.radians(P.wang))
        fb = hw * P.wfb
        wv = 2 * (run + fb)
        return 2 * wv - 2 * (hw - fb)
    if k == "ae":
        return lc_width(P, "lc_a") + lc_width(P, "lc_e") - Sl
    if k == "germandbls":
        return 0.98 * x * P.wscale + add
    if k == "AE":
        return P.H * (1.04 + 0.86) * P.wscale * 0.80
    return LC_WIDTHS[k] * x * P.wscale + add


def _hook(g, x0, x1, ytop, R=None, Td=None):
    """Stem on the right (x1-S..x1) running down from ytop to the descender,
    turning left into a bar that ends in a 20-degree terminal at x0."""
    S, d = g.S, g.desc
    Td = g.T * 0.92 if Td is None else Td
    R = min(g.rad(ytop - d), 0.9 * (x1 - x0)) if R is None else R
    p = diff(box(x0, d, x1, ytop, br=R),
             g.counter(x0 - 50, d + Td, x1 - S, ytop + 50, br=g.ir(R, S, Td)))
    return p, Td


def _arch(g, x0, x1, top, R, sl=None, sr=None, inner_tl=None):
    """An n's arch: square outer top-left on the stem, squircle top-right."""
    sl = g.S if sl is None else sl
    sr = g.S if sr is None else sr
    T = g.T
    itl = R * 0.32 if inner_tl is None else inner_tl
    return diff(box(x0, 0, x1, top, tr=R),
                g.counter(x0 + sl, -50, x1 - sr, top - T, tl=itl, tr=g.ir(R, sr, T)))


def _n_path(g, w):
    return _arch(g, 0, w, g.H, g.rad(g.H))


def _bowl(g, w, ylo=None, yhi=None):
    """A bowl whose right side is a straight stem (d, q, single-storey a)."""
    ov = g.ov * 0.5
    ylo = -ov if ylo is None else ylo
    yhi = g.H + ov if yhi is None else yhi
    R = g.rad(yhi - ylo)
    return g.ring(0, ylo, w, yhi, tl=R, bl=R, tr=0, br=0, sr=g.S, sl=g.Sr)


def _dot_box(g, xc, top=None):
    """The i/j dot: a stem-wide squircle box under the ascender line."""
    S, T, x = g.S, g.T, g.H
    top = g.asc if top is None else top
    gap = max(0.55 * T, 0.12 * x)
    dh = min(S, top - x - gap)
    return box(xc - S / 2, top - dh, xc + S / 2, top, r=min(S, dh) * 0.26)


def _mono_flag(g, xs, y_top, reach):
    """Mono: a slab to the left of a stem's top (i, j, l)."""
    return rect(xs - reach, y_top - g.T, xs + 1, y_top)


# ===================================================================== a-z

@glyph("a", [0x61], "lc_a", "rs")
def _a(g):
    lc(g)
    if getattr(g, "single", False):
        _a_single(g)
        return
    x, S, T, w, ov = g.H, g.S, g.T, g.w, g.ov
    Tm = g.Tm * 0.85                 # the bowl's roof: lighter, so both counters stay open
    R = g.rad(x)
    yb = g.P.a_bowl * x
    # top bar and right stem
    top = diff(box(0, -ov * 0.5, w, x + ov, tr=R),
               g.counter(-BIG, -BIG, w - S, x + ov - g.Tr, tr=g.ir(R, S, g.Tr)))
    g.add(top)
    g.term(tend(g, w) + 0.06 * w, x + ov - g.Tr / 2, "left", "upper", ylo=yb + 1)
    # lower bowl: its right side is the stem
    Rb = g.rad(yb + ov)
    bowl = diff(box(0, -ov, w - S + 1, yb, tl=Rb, bl=Rb),
                g.counter(g.Sr, -ov + g.Tr, w + 10, yb - Tm,
                          tl=g.ir(Rb, g.Sr, Tm), bl=g.ir(Rb, g.Sr, g.Tr)))
    g.add(bowl)
    g.add(rect(w - S, -ov, w, yb))


def _a_single(g):
    w = g.w
    g.add(_bowl(g, w))
    g.add(rect(w - g.S, -g.ov * 0.5, w, g.H + g.ov * 0.5))


@glyph("b", [0x62], "lc_b", "sr")
def _b(g):
    lc(g)
    w = g.w
    p = union(_bowl(g, w), rect(w - g.S, -g.ov * 0.5, w, g.asc))
    g.add(gm.mirror_x(p, w / 2))


@glyph("c", [0x63], "lc_c", "rc")
def _c(g):
    lc(g)
    _C(g)


@glyph("d", [0x64], "lc_d", "rs")
def _d(g):
    lc(g)
    w = g.w
    g.add(_bowl(g, w))
    g.add(rect(w - g.S, -g.ov * 0.5, w, g.asc))


@glyph("e", [0x65], "lc_e", "ro")
def _e(g):
    lc(g)
    x, ov, w, Tm = g.H, g.ov, g.w, g.Tm
    sqleft = getattr(g, "sq_left", False)     # the e of the ae ligature
    R = g.rad(x + 2 * ov)
    if sqleft:
        g.add(g.ring(0, -ov, w, x + ov, r=R, br=0, tl=0, bl=0, sl=g.S))
    else:
        g.add(g.ring(0, -ov, w, x + ov, r=R, br=0))
    ye = g.P.e_bar * x
    g.add(rect(1, ye - Tm / 2, w - 1, ye + Tm / 2))
    g.sub(rect(w - g.Sr - 1, -ov + g.Tr - 0.5, BIG, ye - Tm / 2 + 0.5))
    g.term(w - tend(g, w), -ov + g.Tr / 2, "right", "lower", yhi=ye - Tm / 2)


@glyph("f", [0x66], "lc_f", "tt")
def _f(g):
    lc(g)
    x, S, T, w, asc = g.H, g.S, g.T, g.w, g.asc
    xs = (0.30 if g.mono else 0.20) * w
    Rf = min(g.rad(asc - x), 0.80 * (w - xs))
    g.add(diff(box(xs, 0, w, asc, tl=Rf),
               g.counter(xs + S, -50, w + 50, asc - T, tl=g.ir(Rf, S, T))))
    g.term(w - tend(g, w), asc - T / 2, "right", "upper", ylo=asc - T - 2)
    g.add(rect(0, x - T, (0.92 if g.mono else 0.84) * w, x))
    if g.mono:
        g.add(rect(xs - 0.16 * w, 0, xs + S + 0.30 * w, T))


@glyph("g", [0x67], "lc_g", "rs")
def _g(g):
    lc(g)
    w = g.w
    g.add(_bowl(g, w))
    hook, Td = _hook(g, 0, w, g.H + g.ov * 0.5)
    g.add(hook)
    g.term(tend(g, w) + 0.04 * w, g.desc + Td / 2, "left", "lower", yhi=g.desc + Td + 1)


@glyph("h", [0x68], "lc_h", "ss")
def _h(g):
    lc(g)
    g.add(_n_path(g, g.w))
    g.add(rect(0, 0, g.S, g.asc))


@glyph("i", [0x69], "lc_i", "ss")
def _i(g):
    lc(g)
    _dotless_i(g)
    xc = g.w / 2 if g.mono else g.S / 2
    if g.mono:
        xc = g.w / 2 + 0.04 * g.w
    g.add(_dot_box(g, xc))


def _dotless_i(g):
    S, x = g.S, g.H
    if g.mono:
        xs = g.w / 2 - S / 2 + 0.04 * g.w
        g.add(rect(xs, 0, xs + S, x))
        g.add(_mono_flag(g, xs, x, 0.26 * g.w))
        g.add(rect(xs + S / 2 - 0.36 * g.w, 0, xs + S / 2 + 0.36 * g.w, g.T))
    else:
        g.add(rect(0, 0, S, x))


@glyph("dotlessi", [0x131], "lc_i", "ss")
def _dotlessi(g):
    lc(g)
    _dotless_i(g)


@glyph("j", [0x6A], "lc_j", "hs")
def _j(g):
    lc(g)
    w, S = g.w, g.S
    if g.mono:
        x1 = w / 2 + S / 2 + 0.12 * w
        hook, Td = _hook(g, 0.06 * w, x1, g.H)
        g.add(hook)
        g.add(_mono_flag(g, x1 - S, g.H, 0.26 * w))
        g.term(0.06 * w + tend(g, w), g.desc + Td / 2, "left", "lower", yhi=g.desc + Td + 1)
        g.add(_dot_box(g, x1 - S / 2))
        return
    hook, Td = _hook(g, 0, w, g.H)
    g.add(hook)
    g.term(tend(g, w), g.desc + Td / 2, "left", "lower", yhi=g.desc + Td + 1)
    g.add(_dot_box(g, w - S / 2))


@glyph("dotlessj", [0x237], "lc_j", "hs")
def _dotlessj(g):
    lc(g)
    w = g.w
    hook, Td = _hook(g, 0, w, g.H)
    g.add(hook)
    g.term(tend(g, w), g.desc + Td / 2, "left", "lower", yhi=g.desc + Td + 1)


@glyph("k", [0x6B], "lc_k", "sd")
def _k(g):
    lc(g)
    _K(g)
    g.add(rect(0, 0, g.S, g.asc))


@glyph("l", [0x6C], "lc_l", "so")
def _l(g):
    lc(g)
    if getattr(g, "plain", False):
        g.add(rect(0, 0, g.S, g.asc))
        return
    S, T, w, ov, asc = g.S, g.T, g.w, g.ov, g.asc
    xs = 0.22 * w if g.mono else 0
    x1 = w - (0.04 * w if g.mono else 0)
    R = min(g.rad(g.H), 0.9 * (x1 - xs))
    g.add(diff(box(xs, -ov, x1, asc, bl=R),
               g.counter(xs + S, -ov + T, x1 + 50, asc + 50, bl=g.ir(R, S, T))))
    g.term(x1 - tend(g, x1 - xs), -ov + T / 2, "right", "lower", yhi=-ov + T + 1)
    if g.mono:
        g.add(_mono_flag(g, xs, asc, 0.18 * w))


@glyph("m", [0x6D], "lc_m", "ss")
def _m(g):
    lc(g)
    x, w = g.H, g.w
    Sm = g.S * (0.84 if g.mono else 0.97)
    xm = (w - Sm) / 2
    cw = xm - Sm
    R = min(g.rad(x), 0.62 * (cw + 2 * Sm)) * 0.85
    g.add(_arch(g, 0, xm + Sm, x, R, sl=Sm, sr=Sm, inner_tl=R * 0.28))
    g.add(_arch(g, xm, w, x, R, sl=Sm, sr=Sm, inner_tl=R * 0.28))


@glyph("n", [0x6E], "lc_n", "ss")
def _n(g):
    lc(g)
    g.add(_n_path(g, g.w))


@glyph("o", [0x6F], "lc_o", "rr")
def _o(g):
    lc(g)
    g.add(g.ring(0, -g.ov, g.w, g.H + g.ov, r=g.rad(g.H + 2 * g.ov)))


@glyph("p", [0x70], "lc_p", "sr")
def _p(g):
    lc(g)
    w = g.w
    p = union(_bowl(g, w), rect(w - g.S, g.desc, w, g.H + g.ov * 0.5))
    g.add(gm.mirror_x(p, w / 2))


@glyph("q", [0x71], "lc_q", "rs")
def _q(g):
    lc(g)
    w = g.w
    g.add(_bowl(g, w))
    g.add(rect(w - g.S, g.desc, w, g.H + g.ov * 0.5))


@glyph("r", [0x72], "lc_r", "st")
def _r(g):
    lc(g)
    x, S, T, w = g.H, g.S, g.T, g.w
    xs = 0.20 * w if g.mono else 0
    R = g.rad(x) * 0.32
    g.add(diff(box(xs, 0, w, x),
               g.counter(xs + S, -50, w + 50, x - T, tl=R)))
    g.term(w - tend(g, w - xs), x - T / 2, "right", "upper", ylo=x - T - 2)
    if g.mono:
        g.add(rect(0.04 * w, 0, xs + S + 0.30 * w, T))


@glyph("s", [0x73], "lc_s", "rr")
def _s(g):
    lc(g)
    _s_shape(g, g.w, g.P.ymS * g.H)


@glyph("t", [0x74], "lc_t", "to")
def _t(g):
    lc(g)
    x, S, T, w, ov, asc = g.H, g.S, g.T, g.w, g.ov, g.asc
    xs = (0.28 if g.mono else 0.20) * w
    th = x + 0.42 * (asc - x)
    Rt = min(g.rad(x), 0.85 * (w - xs))
    g.add(diff(box(xs, -ov, w, th, bl=Rt),
               g.counter(xs + S, -ov + T, w + 50, th + 50, bl=g.ir(Rt, S, T))))
    g.term(w - tend(g, w - xs), -ov + T / 2, "right", "lower", yhi=-ov + T + 1)
    g.add(rect(0, x - T, (0.94 if g.mono else 0.88) * w, x))


@glyph("u", [0x75], "lc_u", "ss")
def _u(g):
    lc(g)
    g.add(gm.rotate180(_n_path(g, g.w), g.w / 2, g.H / 2))


@glyph("v", [0x76], "lc_v", "dd")
def _v(g):
    lc(g)
    _V(g)


@glyph("w", [0x77], "lc_w", "dd")
def _w(g):
    lc(g)
    if g.mono:
        # the capital's squared Mono W reads as a Cyrillic letter in lower
        # case: two narrow Vs instead, their legs leaning as far as fits
        w, td = g.w, g.S * g.P.wdiag
        for x0, x1 in ((0, 0.56 * w), (0.44 * w, w)):
            p, sil, _, _ = g.vee(x0, x1, td, g.P.wang, flat_t=0, min_fb=0.30)
            g.add(p)
        return
    _W(g)


@glyph("x", [0x78], "lc_x", "dd")
def _x(g):
    lc(g)
    _X(g)


@glyph("y", [0x79], "lc_y", "dd")
def _y(g):
    lc(g)
    x, w = g.H, g.w
    td = g.S * g.P.diag
    p, sil, hw, fb = g.vee(0, w, td, g.P.facet, y1=x)
    xc = w / 2
    # left leg only above the baseline; the right leg runs on to the descender
    g.add(inter(p, rect(-10, 0, xc + fb + 1, x)))
    d = g.desc
    x1t, x1b = w, xc + fb                 # right leg's outer edge, top and baseline
    dx = (x1t - x1b) / x
    xd = x1b + d * dx
    g.add(poly([(w - hw, x), (w, x), (xd, d), (xd - hw, d)]))


@glyph("z", [0x7A], "lc_z", "oo")
def _z(g):
    lc(g)
    _Z(g)


# ===================================================================== Latin-1 letters

@glyph("oslash", [0xF8], "lc_o", "rr")
def _oslash(g):
    lc(g)
    _o(g)
    g.add(_slash_through(g, 0, g.w, -g.ov, g.H + g.ov))


@glyph("Oslash", [0xD8], "O", "rr")
def _Oslash(g):
    g.add(g.ring(0, -g.ov, g.w, g.H + g.ov, r=g.rad(g.H + 2 * g.ov)))
    g.add(_slash_through(g, 0, g.w, -g.ov, g.H + g.ov))


def _slash_through(g, x0, x1, y0, y1):
    ext = 0.06 * (y1 - y0)
    t = g.T * g.P.zslash
    ang = math.radians(g.P.facet)
    hw = t / math.sin(ang)
    ya, yb = y0 - ext, y1 + ext
    run = (yb - ya) / math.tan(ang)
    xa = (x0 + x1) / 2 - run / 2 - hw / 2
    return poly([(xa, ya), (xa + hw, ya), (xa + hw + run, yb), (xa + run, yb)])


@glyph("ae", [0xE6], "lc_ae", "ro")
def _ae(g):
    lc(g)
    P = g.P
    wa = lc_width(P, "lc_a")
    we = lc_width(P, "lc_e")
    if g.mono:
        wa = we = (g.w + g.S) / 2
    g0 = type(g)(P, wa)
    _a(g0)
    g1 = type(g)(P, we)
    g1.sq_left = True
    _e(g1)
    g.add(g0.p)
    g.add(gm.translate(g1.p, wa - g.S))


@glyph("AE", [0xC6], "lc_AE", "so")
def _AE(g):
    """The A's leg leans to the E's stem (shallower than the 70-degree
    facet, so the A keeps a counter and a visible crossbar)."""
    from glyphs2 import _E
    H, S, T, w = g.H, g.S, g.T, g.w
    xe = 0.40 * w
    a = math.atan2(H, xe)
    hw = S * g.P.diag / math.sin(a)
    g.add(poly([(0, 0), (hw, 0), (xe + hw, H), (xe, H)]))
    yb = g.P.abar * H
    g.add(rect(hw + yb * xe / H - 2, yb, xe + 1, yb + T * 0.96))
    ge = type(g)(g.P, w - xe)
    _E(ge)
    g.add(gm.translate(ge.p, xe))
    g.clip(rect(-10, 0, w + 10, H))


@glyph("germandbls", [0xDF], "lc_germandbls", "sr")
def _germandbls(g):
    lc(g)
    x, S, T, w, asc = g.H, g.S, g.T, g.w, g.asc
    Tm = g.Tm
    ym = 0.58 * asc
    du = 0.10 * w
    Rt = g.rad(asc - ym) * 0.95
    Ru = g.rad(asc - ym)
    Rl = g.rad(ym + Tm / 2)
    up = diff(box(0, ym - Tm / 2, w - du, asc, tl=Rt, tr=Ru, br=Ru),
              g.counter(S, ym + Tm / 2, w - du - g.Sr, asc - T,
                        tl=g.ir(Rt, S, T), tr=g.ir(Ru, g.Sr, T), br=g.ir(Ru, g.Sr, Tm)))
    lo = diff(box(0, -g.ov, w, ym + Tm / 2, tr=Rl, br=Rl),
              g.counter(S, -g.ov + T, w - g.Sr, ym - Tm / 2,
                        tr=g.ir(Rl, g.Sr, Tm), br=g.ir(Rl, g.Sr, T)))
    lo = diff(lo, rect(S - 1, -BIG, S + 0.22 * w, -g.ov + T + 1))
    g.add(lo)
    g.term(S + 0.22 * w + tend(g, w) * 0.6, -g.ov + T / 2, "left", "lower", yhi=-g.ov + T + 1)
    g.add(up)
    g.add(rect(0, 0, S, asc - Rt))


@glyph("eth", [0xF0], "lc_d", "rs")
def _eth(g):
    lc(g)
    w = g.w
    _d(g)
    yb = g.H + 0.50 * (g.asc - g.H) - g.T * 0.42
    g.add(rect(w - g.S - 0.30 * w, yb, w + 0.06 * w, yb + g.T * 0.84))


@glyph("Eth", [0xD0], "D", "pr")
def _Eth(g):
    from glyphs2 import _D
    _D(g)
    ym = g.P.ym * g.H
    g.add(rect(-0.16 * g.w, ym - g.Tm / 2, g.S + 0.18 * g.w, ym + g.Tm / 2))


@glyph("thorn", [0xFE], "lc_p", "sr")
def _thorn(g):
    lc(g)
    w = g.w
    p = union(_bowl(g, w), rect(w - g.S, g.desc, w, g.asc))
    g.add(gm.mirror_x(p, w / 2))


@glyph("Thorn", [0xDE], "P", "sr")
def _Thorn(g):
    H, S, T, Tm, w = g.H, g.S, g.T, g.Tm, g.w
    y0, y1 = 0.17 * H, 0.83 * H
    Rp = g.rad(y1 - y0)
    g.add(rect(0, 0, S, H))
    g.add(box(0, y0, w, y1, tr=Rp, br=Rp))
    g.sub(g.counter(S, y0 + Tm, w - g.Sr, y1 - T, tr=g.ir(Rp, g.Sr, T), br=g.ir(Rp, g.Sr, Tm)))


# ===================================================================== marks

def mark_metrics(g, cap):
    """(bottom, height, stroke) for marks over capitals or lower case."""
    P = g.P
    if cap:
        H = P.H
        bot = H + 0.05 * H
        top = (850 if not P.mono else 820) - 8
        return bot, top - bot, min(P.Sl * 0.86, (top - bot) * 0.95)
    x, T = P.xh, P.Tl
    gap = max(0.55 * T, 0.12 * x)
    bot = x + gap
    top = P.asc + 0.03 * P.H
    return bot, top - bot, P.Sl * 0.86


MARK_ANG = 52      # acute and grave, degrees from horizontal


def mk_acute(g, cap, flip=False):
    bot, hm, t = mark_metrics(g, cap)
    hm *= 0.92
    run = hm / math.tan(math.radians(MARK_ANG))
    p = stroke(-run / 2, 0, run / 2, hm, t)
    if flip:
        p = gm.mirror_x(p, 0)
    return gm.translate(p, 0, bot)


def mk_grave(g, cap):
    return mk_acute(g, cap, flip=True)


def mk_circumflex(g, cap, inverted=False):
    bot, hm, t = mark_metrics(g, cap)
    hm *= 0.80
    t *= 0.90
    wc = (0.62 if not cap else 0.50) * g.P.xh
    hw = t / math.sin(math.atan2(hm, wc / 2))
    L = stroke(-wc / 2 + hw / 2, 0, 0, hm, t)
    p = inter(union(L, gm.mirror_x(L, 0)), rect(-BIG, 0, BIG, hm))
    if inverted:
        p = gm.transform(p, 1, 0, 0, -1, 0, hm)
    return gm.translate(p, 0, bot)


def mk_dieresis(g, cap):
    bot, hm, t = mark_metrics(g, cap)
    S = g.P.Sl * (0.92 if cap else 1.0)
    dh = min(S, hm * (0.92 if cap else 1.0))
    if not cap:
        # the i's dot: same size and height
        T, x = g.P.Tl, g.P.xh
        gap = max(0.55 * T, 0.12 * x)
        dh = min(S, g.P.asc - x - gap)
    gap2 = max(0.85 * S, 0.16 * g.P.xh)
    r = min(S, dh) * 0.26
    a = box(-gap2 / 2 - S, 0, -gap2 / 2, dh, r=r)
    b = box(gap2 / 2, 0, gap2 / 2 + S, dh, r=r)
    return gm.translate(union(a, b), 0, bot)


def _zigzag(wt, ht, t):
    """An angular tilde: three strokes with flat ends and flat turns."""
    xs = [-wt / 2, -wt / 6, wt / 6, wt / 2]
    hws = []
    seg = []
    for i in range(3):
        y0, y1 = (0, ht) if i % 2 == 0 else (ht, 0)
        dx = xs[i + 1] - xs[i]
        hw = t * math.hypot(dx, ht) / ht / 2
        hws.append(hw)
        x0 = xs[i] + (hw if i == 0 else 0)
        x1 = xs[i + 1] - (hw if i == 2 else 0)
        seg.append(stroke(x0, y0, x1, y1, t))
    p = seg[0]
    for s in seg[1:]:
        p = union(p, s)
    return inter(p, rect(-BIG, 0, BIG, ht))


def mk_tilde(g, cap):
    bot, hm, t = mark_metrics(g, cap)
    ht = hm * (0.66 if not cap else 0.86)
    wt = (0.78 if not cap else 0.70) * g.P.xh
    return gm.translate(_zigzag(wt, ht, min(t * 0.80, ht * 0.50)), 0, bot + (hm - ht) * (0.25 if not cap else 0))


def mk_ring(g, cap):
    bot, hm, t = mark_metrics(g, cap)
    s = min(hm * (1.05 if cap else 0.95), 0.40 * g.P.xh)
    sw = max(s * 1.15, 0.36 * g.P.xh)
    tt = max(min(g.P.Tl * 0.70, s * 0.30), 30)
    st = max(min(g.P.Sl * 0.74, sw * 0.30), 34)
    p = g.ring(-sw / 2, 0, sw / 2, s, r=min(s, sw) * 0.40, sl=st, sr=st, tb=tt, tt=tt)
    return gm.translate(p, 0, bot - (0.02 * g.P.H if cap else 0))


def mk_cedilla(g, cap):
    S, T = g.P.Sl, g.P.Tl
    t = S * 0.70
    tb = T * 0.70
    hc = 0.27 * g.P.xh
    cw = 0.26 * g.P.xh + t
    R = min(hc, cw) * 0.55
    p = diff(box(-cw + t / 2, -hc, t / 2, 2, br=R),
             g.counter(-cw - 50, -hc + tb, -t / 2, 60, br=g.ir(R, t, tb)))
    return p


MARKS = dict(grave=mk_grave, acute=mk_acute, circumflex=mk_circumflex, tilde=mk_tilde,
             dieresis=mk_dieresis, ring=mk_ring, cedilla=mk_cedilla)

# composite name -> (codepoint, base, mark)
_ACC = [("grave", 0), ("acute", 1), ("circumflex", 2), ("tilde", 3), ("dieresis", 4), ("ring", 5)]
COMPOSITES = {}


def _add(name, cp, base, mark):
    COMPOSITES[name] = (cp, base, mark)


for base, cps in [("A", (0xC0, 0xC1, 0xC2, 0xC3, 0xC4, 0xC5)), ("a", (0xE0, 0xE1, 0xE2, 0xE3, 0xE4, 0xE5))]:
    for (m, _), cp in zip(_ACC, cps):
        _add(base + m if m != "ring" else base + "ring", cp, base, m)
for base, cp0 in [("E", 0xC8), ("e", 0xE8), ("I", 0xCC), ("i", 0xEC), ("U", 0xD9), ("u", 0xF9)]:
    b = "dotlessi" if base == "i" else base
    for (m, cp) in zip(["grave", "acute", "circumflex", "dieresis"], range(cp0, cp0 + 4)):
        _add(base + m, cp, b, m)
for base, cps in [("O", (0xD2, 0xD3, 0xD4, 0xD5, 0xD6)), ("o", (0xF2, 0xF3, 0xF4, 0xF5, 0xF6))]:
    for m, cp in zip(["grave", "acute", "circumflex", "tilde", "dieresis"], cps):
        _add(base + m, cp, base, m)
_add("Ntilde", 0xD1, "N", "tilde")
_add("ntilde", 0xF1, "n", "tilde")
_add("Ccedilla", 0xC7, "C", "cedilla")
_add("ccedilla", 0xE7, "c", "cedilla")
_add("Yacute", 0xDD, "Y", "acute")
_add("yacute", 0xFD, "y", "acute")
_add("ydieresis", 0xFF, "y", "dieresis")
_add("Ydieresis", 0x178, "Y", "dieresis")

LOWER_LATIN = ["dotlessi", "oslash", "ae", "eth", "thorn"]
UPPER_LATIN = ["Oslash", "AE", "Eth", "Thorn"]


# ===================================================================== symbols

@glyph("dollar", [0x24], "digit", "nn")
def _dollar(g):
    H, w = g.H, g.w
    _s_shape(g, w, g.P.ymS * H)
    t = g.S * 0.62
    xc = w / 2
    g.add(rect(xc - t / 2, -0.12 * H, xc + t / 2, -g.ov + 1))
    g.add(rect(xc - t / 2, H + g.ov - 1, xc + t / 2, H + 0.12 * H))
    # the bar shows through the spine too, so it reads at 9 px
    g.add(rect(xc - t / 2, g.P.ymS * H - g.Tm / 2, xc + t / 2, g.P.ymS * H + g.Tm / 2))


def _angle(g):
    t = g.T * 0.96
    L = 0.27 * g.H
    h = t * math.sqrt(2)
    yc = _math_y(g)
    x1 = L + h
    return poly([(x1, yc), (x1 - L, yc + L), (x1 - L - h, yc + L), (x1 - h, yc),
                 (x1 - L - h, yc - L), (x1 - L, yc - L)])


@glyph("greater", [0x3E], "angle", "pp")
def _greater(g):
    g.add(_angle(g))


@glyph("less", [0x3C], "angle", "pp")
def _less(g):
    p = _angle(g)
    b = p.bounds
    g.add(gm.mirror_x(p, (b[0] + b[2]) / 2))


@glyph("asciicircum", [0x5E], "angle", "pp")
def _asciicircum(g):
    H = g.H
    t = g.T * 0.92
    hm = 0.36 * H
    wc = 0.56 * H
    hw = t / math.sin(math.atan2(hm, wc / 2))
    L = stroke(-wc / 2 + hw / 2, H - hm, 0, H, t)
    g.add(inter(union(L, gm.mirror_x(L, 0)), rect(-BIG, H - hm, BIG, H)))


@glyph("grave", [0x60, 0x2CB], "dot", "pp")
def _grave(g):
    g.add(gm.translate(mk_acute(g, False, flip=True), 0, g.P.H - g.P.xh - (mark_metrics(g, False)[0] - g.P.xh) - 0.02 * g.P.H))


@glyph("acute", [0xB4], "dot", "pp")
def _acute(g):
    g.add(gm.translate(mk_acute(g, False), 0, g.P.H - g.P.xh - (mark_metrics(g, False)[0] - g.P.xh) - 0.02 * g.P.H))


@glyph("dieresis", [0xA8], "dot", "pp")
def _dieresis(g):
    g.add(mk_dieresis(g, True))


@glyph("cedilla", [0xB8], "dot", "pp")
def _cedilla(g):
    g.add(mk_cedilla(g, False))


def _brace_path(g, w):
    H = g.H
    y0, y1 = -0.13 * H, H * 1.13
    t = g.S * 0.92
    tb = g.T * 0.92
    nub = 0.36 * w
    w = max(w, nub + t + 0.85 * t + 0.04 * H)   # the arms stay visible past the stem
    ym = (y0 + y1) / 2
    R = min(0.6 * (w - nub), 0.18 * (y1 - y0))
    xs = nub
    outer = union(box(xs, ym, w, y1, tl=R), box(xs, y0, w, ym + 1, bl=R))
    inner = union(g.counter(xs + t, ym + tb / 2, w + 50, y1 - tb, tl=g.ir(R, t, tb)),
                  g.counter(xs + t, y0 + tb, w + 50, ym - tb / 2, bl=g.ir(R, t, tb)))
    p = diff(outer, inner)
    p = union(p, rect(0, ym - tb / 2, xs + t, ym + tb / 2))
    return p


@glyph("braceleft", [0x7B], "brace", "pp")
def _braceleft(g):
    g.add(_brace_path(g, g.w))


@glyph("braceright", [0x7D], "brace", "pp")
def _braceright(g):
    g.add(gm.mirror_x(_brace_path(g, g.w), g.w / 2))


@glyph("asciitilde", [0x7E], "plus", "pp")
def _asciitilde(g):
    w = g.w
    ht = 0.24 * g.H
    p = _zigzag(w, ht, min(g.T * 0.86, ht * 0.52))   # Black: keep the turns open
    g.add(gm.translate(p, w / 2, _math_y(g) - ht / 2))


@glyph("periodcentered", [0xB7], "dot", "pp")
def _periodcentered(g):
    s = g.P.dot * g.S * 0.80
    g.add(_dot(g, 0, _math_y(g) - s / 2, s))


@glyph("divide", [0xF7], "plus", "pp")
def _divide(g):
    w, t = g.w, g.T * 0.98
    yc = _math_y(g)
    s = g.P.dot * g.S * 0.86
    g.add(rect(0, yc - t / 2, w, yc + t / 2))
    g.add(_dot(g, w / 2 - s / 2, yc + t / 2 + 0.07 * g.H, s))
    g.add(_dot(g, w / 2 - s / 2, yc - t / 2 - 0.07 * g.H - s, s))


@glyph("exclamdown", [0xA1], "dot", "pp")
def _exclamdown(g):
    _exclam(g)
    b = g.p.bounds
    g.p = gm.rotate180(g.p, (b[0] + b[2]) / 2, g.P.xh / 2 + 0.0)
    g.p = gm.translate(g.p, 0, -(g.p.bounds[1] - g.P.desc * 0.95))


@glyph("questiondown", [0xBF], "question", "op")
def _questiondown(g):
    _question(g)
    g.p = gm.rotate180(g.p, g.w / 2, g.H / 2)
    g.p = gm.translate(g.p, 0, -(g.p.bounds[1] - g.P.desc * 0.95))


def _guil(g, n):
    t = g.T * 0.86
    L = 0.17 * g.H
    h = t * math.sqrt(2)
    yc = _math_y(g)
    one = poly([(L + h, yc), (h, yc + L), (0, yc + L), (L, yc), (0, yc - L), (h, yc - L)])
    p = one
    for i in range(1, n):
        p = union(p, gm.translate(one, i * (L + h) * 0.82))
    return p


@glyph("guillemotright", [0xBB], "dot", "pp")
def _guillemotright(g):
    g.add(_guil(g, 2))


@glyph("guillemotleft", [0xAB], "dot", "pp")
def _guillemotleft(g):
    p = _guil(g, 2)
    b = p.bounds
    g.add(gm.mirror_x(p, (b[0] + b[2]) / 2))


SYMBOLS = ["dollar", "less", "greater", "asciicircum", "grave", "braceleft", "braceright",
           "asciitilde", "periodcentered", "divide", "exclamdown", "questiondown",
           "guillemotleft", "guillemotright", "acute", "dieresis", "cedilla"]

SYMBOL_WIDTHS = dict(angle=0.0, brace=0.30)   # angle: ink-fitted ('pp')
MONO_SYMBOL_W = dict(angle=0.9, brace=0.40)
