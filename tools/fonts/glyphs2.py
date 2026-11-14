"""Glyph drawings for UGG Display (Bold, Regular) and UGG Mono.

Every glyph is drawn in a body box x 0..w, y 0..H (cap height) from
squircle boxes, counters, straight strokes and half-plane cuts (geom2).
Parameters (stems, bars, radii, widths) come from the font's P; the same
drawings serve all three fonts, with a few mono-specific forms (I, J, M, W,
1, zero) chosen by P.mono.

Design rules:
- Bowls are squircle boxes; the corner radius depends only on the bowl's
  height (rad), so full-height and half-height bowls share one curvature
  family. Counters get radii solved for an even stroke round the corner.
- Open terminals (C G S J 2 3 5 6 9 ? @ &) are cut at the logo's steep
  facet, 20 degrees off vertical, always so the outer edge runs longer.
- Diagonals (A V W 7 / \\ #) lean at the logo's 70-degree facet; the flag
  of 1 at its 25-degree shoulder facet.
"""
import math

import geom2 as gm
from geom2 import box, rect, poly, stroke, pstroke, union, diff, inter, halfplane, BIG

GLYPHS = {}  # name -> (codepoints, width key, sides, func)


def glyph(name, cps, wkey=None, sides="ss"):
    def deco(f):
        GLYPHS[name] = (cps, wkey or name, sides, f)
        return f
    return deco


class G:
    def __init__(self, P, w):
        self.P = P
        self.w = w
        self.H, self.S, self.T = P.H, P.S, P.T
        self.Sr, self.Tr = P.S * P.round_gain, P.T * P.round_gain
        self.Tm = P.T * P.mid_gain
        self.ov = P.ov
        self.mono = P.mono
        self.p = gm.empty()
        self.center_ink = False  # mono: centre the ink instead of the body
        self.logo = False        # ss02 wordmark forms (Black)

    def facet_end(self, x, y0, y1, side="right", reach=None):
        """Logo set: cut a horizontal arm's end (band y0..y1) with a line
        leaning like the symbol's steep facets (backslash, 70 degrees), the
        line crossing the arm's middle at x. Every cut in the wordmark is
        parallel to this one."""
        hp = halfplane(x, (y0 + y1) / 2, 90 + self.P.tilt, side, y0 - 1, y1 + 1)
        if reach is not None:
            hp = inter(hp, rect(x - reach, -BIG, x + reach, BIG))
        self.sub(hp)

    # ------------------------------------------------ ops
    def add(self, p):
        self.p = union(self.p, p)

    def sub(self, p):
        self.p = diff(self.p, p)

    def clip(self, p):
        self.p = inter(self.p, p)

    def rad(self, h):
        """Outer corner radius for a bowl of height h."""
        return min(self.P.R, self.P.rfrac * h)

    def ir(self, R, sx, ty, f=None):
        return gm.inner_radius(R, sx, ty, factor=self.P.cf if f is None else f)

    def ring(self, x0, y0, x1, y1, r=0, tl=None, tr=None, br=None, bl=None,
             sl=None, sr=None, tb=None, tt=None):
        S, T = self.Sr, self.Tr
        sl = S if sl is None else sl
        sr = S if sr is None else sr
        tb = T if tb is None else tb
        tt = T if tt is None else tt
        rr = {c: (r if v is None else v) for c, v in (("tl", tl), ("tr", tr), ("br", br), ("bl", bl))}
        outer = box(x0, y0, x1, y1, tl=rr["tl"], tr=rr["tr"], br=rr["br"], bl=rr["bl"])
        inner = box(x0 + sl, y0 + tb, x1 - sr, y1 - tt, k=gm.K_IN,
                    tl=self.ir(rr["tl"], sl, tt), tr=self.ir(rr["tr"], sr, tt),
                    br=self.ir(rr["br"], sr, tb), bl=self.ir(rr["bl"], sl, tb))
        return diff(outer, inner)

    def counter(self, x0, y0, x1, y1, **radii):
        return box(x0, y0, x1, y1, k=gm.K_IN, **radii)

    def term(self, x, y, side, arm, ylo=-BIG, yhi=BIG, tilt=None):
        """Cut an open terminal: remove everything on `side` of a line through
        (x, y) tilted `tilt` degrees off vertical so the arm's outer edge is
        the longer one. arm = 'upper' or 'lower' (which arm is being cut)."""
        t = self.P.tilt if tilt is None else tilt
        lean_right = (side == "right") == (arm == "upper")
        ang = 90 - t if lean_right else 90 + t
        self.sub(halfplane(x, y, ang, side, ylo, yhi))

    def vee(self, x0, x1, td, ang, flat_t=None, y0=0.0, y1=None, min_fb=0.45):
        """V between x0..x1, legs of perpendicular thickness td whose outer
        edges lean `ang` degrees. When the box is too narrow for that angle
        (the mono), the legs lean less so the vertex stays min_fb*hw wide
        on each side. A flat vertex bar keeps thin weights joined."""
        y1 = self.H if y1 is None else y1
        h = y1 - y0
        xc = (x0 + x1) / 2
        run = h / math.tan(math.radians(ang))
        hw = td / math.sin(math.radians(ang))
        fb = xc - x0 - run
        if fb < min_fb * hw:
            for _ in range(6):
                fb = min_fb * hw
                run = xc - x0 - fb
                a = math.atan2(h, run)
                hw = td / math.sin(a)
        L = poly([(x0, y1), (x0 + hw, y1), (xc - fb + hw, y0), (xc - fb, y0)])
        Rr = poly([(x1 - hw, y1), (x1, y1), (xc + fb, y0), (xc + fb - hw, y0)])
        sil = poly([(x0, y1), (x1, y1), (xc + fb, y0), (xc - fb, y0)])
        p = inter(union(L, Rr), sil)
        ft = self.T if flat_t is None else flat_t
        if fb > hw * 0.98 and ft > 0:
            p = union(p, inter(rect(x0, y0, x1, y0 + ft), sil))
        return p, sil, hw, fb


# ===================================================================== letters

@glyph("A", [0x41], sides="dd")
def _A(g):
    H, w = g.H, g.w
    p, sil, hw, fb = g.vee(0, w, g.S * g.P.diag, g.P.facet)
    p = gm.transform(p, 1, 0, 0, -1, 0, H)
    sil = gm.transform(sil, 1, 0, 0, -1, 0, H)
    style = getattr(g, "a_style", "plain") if g.logo else "plain"
    if style == "squircle":
        _A_squircle(g)
        return
    yb = g.P.abar * H
    bar = inter(rect(0, yb, w, yb + g.T * 0.96), sil)
    c = 1 / math.tan(math.radians(g.P.facet))
    run = H * c
    gap = g.S * g.P.a_slit             # horizontal width of a slit
    # the right leg as its own parallelogram (outer edge = silhouette's right
    # side, a backslash at 70 degrees, parallel to every logo cut)
    xr_out0, xr_in0 = w, w - hw         # at the baseline
    blade = poly([(xr_in0, 0), (xr_out0, 0), (xr_out0 - run, H), (xr_in0 - run, H)])

    def slit_left_of(x0_base):
        # region left of a backslash line through (x0_base, 0)
        return halfplane(x0_base, 0, 180 - g.P.facet, "left")

    if style == "plain":
        g.add(p)
        g.add(bar)
    elif style == "slit":
        # crossbar hangs from the left leg only; its right end is cut parallel
        # to the right leg, leaving a slit like the gaps between the
        # symbol's pieces. At small sizes the slit closes up into a plain A.
        g.add(p)
        g.add(inter(bar, slit_left_of(xr_in0 - gap)))
    elif style == "split":
        # two facets: a free-standing right blade at full cap height, and the
        # left leg + crossbar, parted from it by a slit along its inner edge
        left = inter(union(p, bar), slit_left_of(xr_in0 - gap))
        g.add(left)
        g.add(inter(blade, sil))


def _A_squircle(g):
    """Flat-topped trapezoid A: an upturned U with squircle shoulders and a
    squared counter, tapered so its sides lean; the crossbar sits high
    enough that it reads as an A, not a n."""
    H, S, w = g.H, g.S, g.w
    xc = w / 2
    k = g.P.a_taper                       # outer top is (1-k) of the base width
    R = g.rad(H) * g.P.a_round
    Sl = S * 1.0
    outer = box(0, 0, w, H, tl=R, tr=R)
    # inner walls: taper them so the legs keep a constant horizontal width
    X, C = xc, xc - Sl
    kc = k * X / C
    yb = g.P.a_sq_bar * H
    tb = g.T * 0.96
    ri = g.ir(R, Sl, g.T)
    ctr = box(Sl, yb + tb, w - Sl, H - g.T, k=gm.K_IN, tl=ri, tr=ri)
    low = rect(Sl, -50, w - Sl, yb)
    shape = diff(gm.taper(outer, xc, k, H), gm.taper(ctr, xc, kc, H))
    shape = diff(shape, gm.taper(low, xc, kc, H))
    g.add(inter(shape, rect(-10, 0, w + 10, H)))


@glyph("B", [0x42], sides="sr")
def _B(g):
    H, S, Sr, w = g.H, g.S, g.Sr, g.w
    T, Tm = g.T * g.P.b8, g.Tm * g.P.b8m
    ym = g.P.ymB * H
    d = 0.055 * w
    Ru, Rl = g.rad(H - ym + Tm / 2), g.rad(ym + Tm / 2)
    g.add(box(0, ym - Tm / 2, w - d, H, tr=Ru, br=Ru))
    g.add(box(0, 0, w, ym + Tm / 2, tr=Rl, br=Rl))
    g.sub(g.counter(S, ym + Tm / 2, w - d - Sr, H - T, tr=g.ir(Ru, Sr, T), br=g.ir(Ru, Sr, Tm)))
    g.sub(g.counter(S, T, w - Sr, ym - Tm / 2, tr=g.ir(Rl, Sr, Tm), br=g.ir(Rl, Sr, T)))


def _c_open(g):
    return g.P.c_open * g.rad(g.H + 2 * g.ov)


def tend(g, width):
    """Distance from the box edge to the middle of an open terminal's cut.
    Every open terminal is cut on a straight stretch of stroke (the corner on
    the terminal's side is square), 20 degrees off perpendicular, outer edge
    longer, so all terminals share one angle, one face length (stroke /
    cos 20) and one sharp-corner treatment."""
    return g.P.tend * width


@glyph("C", [0x43], sides="rc")
def _C(g):
    H, ov, w = g.H, g.ov, g.w
    R = g.rad(H + 2 * ov)
    g.add(g.ring(0, -ov, w, H + ov, r=R, tr=0, br=0))
    g.sub(rect(w - g.Sr - 1, -ov + g.Tr - 0.5, BIG, H + ov - g.Tr + 0.5))
    xc = w - tend(g, w)
    g.term(xc, H + ov - g.Tr / 2, "right", "upper", ylo=H / 2)
    g.term(xc, -ov + g.Tr / 2, "right", "lower", yhi=H / 2)


@glyph("D", [0x44], sides="sr")
def _D(g):
    H, w = g.H, g.w
    R = g.rad(H)
    g.add(g.ring(0, 0, w, H, tl=0, bl=0, tr=R, br=R, sl=g.S, tb=g.T, tt=g.T))


def _bars(g, w, mid, top=True, bottom=True, ym=None):
    H, S, T = g.H, g.S, g.T
    ym = g.P.ym * H if ym is None else ym
    g.add(rect(0, 0, S, H))
    if top:
        g.add(rect(0, H - T, w, H))
    if mid:
        g.add(rect(0, ym - g.Tm / 2, w * mid, ym + g.Tm / 2))
    if bottom:
        g.add(rect(0, 0, w, T))


@glyph("E", [0x45], sides="so")
def _E(g):
    Tm0 = g.Tm
    g.Tm = Tm0 * g.P.b8m
    _bars(g, g.w, 0.88)
    if g.logo:
        H, T, w = g.H, g.T, g.w
        ym = g.P.ym * H
        k = T * 0.18
        g.facet_end(w - k, H - T, H)
        g.facet_end(w * 0.88 - k, ym - g.Tm / 2, ym + g.Tm / 2)
        g.facet_end(w - k, 0, T)
    g.Tm = Tm0


@glyph("F", [0x46], sides="so")
def _F(g):
    _bars(g, g.w, 0.86, bottom=False, ym=g.P.ym * g.H - g.T * 0.12)


@glyph("G", [0x47], sides="rs")
def _G(g):
    H, ov, w, Tm = g.H, g.ov, g.w, g.Tm
    R = g.rad(H + 2 * ov)
    g.add(g.ring(0, -ov, w, H + ov, r=R, tr=0))
    yg = g.P.gbar * H + Tm / 2
    g.sub(rect(w - g.Sr - 1, yg, BIG, H + ov - g.Tr + 0.5))
    xc = w - tend(g, w)
    xg = 0.50 * w
    if g.logo:
        # v0.4: the crossbar's left end stays square. A backslash cut on a
        # left-facing end overhangs at the top, and on a bar hanging in the
        # bowl that overhang read as a hook.
        g.facet_end(xc, H + ov - g.Tr, H + ov)
        g.add(rect(xg, yg - Tm, w - g.Sr / 2, yg))
        return
    g.term(xc, H + ov - g.Tr / 2, "right", "upper", ylo=yg)
    g.add(rect(xg, yg - Tm, w - g.Sr / 2, yg))


@glyph("H", [0x48], sides="ss")
def _H(g):
    H, S, w = g.H, g.S, g.w
    g.add(rect(0, 0, S, H))
    g.add(rect(w - S, 0, w, H))
    ym = g.P.ym * H
    g.add(rect(0, ym - g.Tm / 2, w, ym + g.Tm / 2))


@glyph("I", [0x49], sides="ss")
def _I(g):
    H, S, w = g.H, g.S, g.w
    if g.mono:
        xc = w / 2
        g.add(rect(xc - S / 2, 0, xc + S / 2, H))
        a = 0.30 * w
        g.add(rect(xc - a, 0, xc + a, g.T))
        g.add(rect(xc - a, H - g.T, xc + a, H))
    else:
        g.add(rect(0, 0, S, H))


@glyph("J", [0x4A], sides="os")
def _J(g):
    H, ov, w = g.H, g.ov, g.w
    R = g.rad(H + ov)
    g.add(g.ring(0, -ov, w, H + 600, tl=0, tr=0, bl=R, br=R, sr=g.S))
    g.clip(rect(-10, -100, w + 10, H))
    yj = g.P.jstub * H
    t = math.tan(math.radians(g.P.tilt))
    s0 = g.Sr
    g.sub(poly([(-10, yj + (s0 / 2 + 10) * t), (s0 + 10, yj - (s0 / 2 + 10) * t),
                (s0 + 10, BIG), (-10, BIG)]))
    g.sub(rect(s0 + 10, yj - (s0 / 2) * t, w - g.S, BIG))
    if g.mono:
        g.add(rect(w * 0.30, H - g.T, w, H))


@glyph("K", [0x4B], sides="sd")
def _K(g):
    H, S, w = g.H, g.S, g.w
    td = S * g.P.diag * 0.97
    g.add(rect(0, 0, S, H))
    # arm: from low on the stem up to the top right
    ya = g.P.karm * H
    ha = td * 1.0
    x1 = w
    # centre line from (xs, ya) to (x1 - hw/2, H); hw depends on slope
    xs = S * 0.35
    for _ in range(4):
        dx, dy = (x1 - ha / 2) - xs, H - ya
        ha = td * math.hypot(dx, dy) / dy
    arm = stroke(xs, ya, x1 - ha / 2, H, td)
    g.add(inter(arm, rect(0, 0, w, H)))
    # leg: branches off the arm's centre line, lands at the bottom right
    yl = g.P.kleg * H
    t = (yl - ya) / (H - ya)
    xl = xs + t * ((x1 - ha / 2) - xs)
    tl = S * g.P.diag
    hl = tl
    for _ in range(4):
        dx, dy = (w - hl / 2) - xl, yl
        hl = tl * math.hypot(dx, dy) / dy
    g.add(stroke(xl, yl, w - hl / 2, 0, tl))
    g.clip(rect(0, 0, w, H))


@glyph("L", [0x4C], sides="so")
def _L(g):
    g.add(rect(0, 0, g.S, g.H))
    g.add(rect(0, 0, g.w, g.T))
    if g.logo:
        g.facet_end(g.w - g.T * 0.18, 0, g.T)


@glyph("M", [0x4D], sides="ss")
def _M(g):
    H, S, w = g.H, g.S, g.w
    td = S * g.P.mdiag
    g.add(rect(0, 0, S, H))
    g.add(rect(w - S, 0, w, H))
    yv = g.P.mvert * H
    xc = w / 2
    # each diagonal: top end spans from the stem's outer edge, bottom end
    # meets at the centre
    hw = td
    for _ in range(4):
        dx, dy = xc - hw / 2, H - yv
        hw = td * math.hypot(dx, dy) / dy
    L = stroke(hw / 2, H, xc, yv, td)
    g.add(L)
    g.add(gm.mirror_x(L, xc))
    g.clip(rect(0, 0, w, H))


@glyph("N", [0x4E], sides="ss")
def _N(g):
    H, S, w = g.H, g.S, g.w
    td = S * g.P.diag * (0.86 if g.mono else 1.0)
    g.add(rect(0, 0, S, H))
    g.add(rect(w - S, 0, w, H))
    hw = td
    for _ in range(4):
        hw = td * math.hypot(w - hw, H) / H
    g.add(stroke(hw / 2, H, w - hw / 2, 0, td))


@glyph("O", [0x4F], sides="rr")
def _O(g):
    g.add(g.ring(0, -g.ov, g.w, g.H + g.ov, r=g.rad(g.H + 2 * g.ov)))


@glyph("P", [0x50], sides="sr")
def _P(g):
    H, S, Sr, T, Tm, w = g.H, g.S, g.Sr, g.T, g.Tm, g.w
    yp = g.P.pbowl * H - Tm / 2
    Rp = g.rad(H - yp)
    g.add(rect(0, 0, S, H))
    g.add(box(0, yp, w, H, tr=Rp, br=Rp))
    g.sub(g.counter(S, yp + Tm, w - Sr, H - T, tr=g.ir(Rp, Sr, T), br=g.ir(Rp, Sr, Tm)))


@glyph("Q", [0x51], sides="rr")
def _Q(g):
    H, ov, w = g.H, g.ov, g.w
    g.add(g.ring(0, -ov, w, H + ov, r=g.rad(H + 2 * ov)))
    # tail: a short blade through the bottom-right corner at the shoulder facet
    a = math.radians(g.P.qtail)
    t = g.S * g.P.diag
    hw = t / math.sin(a)
    y0, y1 = g.P.qtop * H, -g.P.qdrop * H
    xb = w + g.P.qout * w          # outer end's right edge
    xt = xb - (y0 - y1) / math.tan(a)
    g.add(poly([(xt - hw, y0), (xt, y0), (xb, y1), (xb - hw, y1)]))


@glyph("R", [0x52], sides="sd")
def _R(g):
    H, S, Sr, T, Tm, w = g.H, g.S, g.Sr, g.T, g.Tm, g.w
    yp = g.P.rbowl * H - Tm / 2
    Rp = g.rad(H - yp)
    g.add(rect(0, 0, S, H))
    g.add(box(0, yp, w, H, tr=Rp, br=Rp))
    g.sub(g.counter(S, yp + Tm, w - Sr, H - T, tr=g.ir(Rp, Sr, T), br=g.ir(Rp, Sr, Tm)))
    # leg: straight, at the logo's steep facet, from under the bowl
    ang = math.radians(g.P.rleg)
    tl = S * g.P.diag
    hl = tl / math.sin(ang)
    ytop = yp + Tm * 0.5
    xb = w - hl / 2
    xt = xb - ytop / math.tan(ang)
    g.add(stroke(xt, ytop, xb, 0, tl))


@glyph("S", [0x53], sides="rr")
def _S(g):
    _s_shape(g, g.w, g.P.ymS * g.H)


def _s_shape(g, w, ym, d=None):
    H, ov, Tm, Tr = g.H, g.ov, g.Tm, g.Tr
    d = 0.022 * w if d is None else d
    Ru = g.rad(H + ov - ym + Tm / 2)
    Rl = g.rad(ym + Tm / 2 + ov)
    U = g.ring(d, ym - Tm / 2, w - d, H + ov, r=Ru, tr=0, br=0, tb=Tm)
    L = g.ring(0, -ov, w, ym + Tm / 2, r=Rl, bl=0, tl=0, tt=Tm)
    xt_u = w - d - tend(g, w)
    xt_l = tend(g, w)
    ang = 90 - g.P.tilt
    keepU = union(rect(-BIG, -BIG, w / 2 + 2, BIG),
                  diff(rect(-BIG, ym + Tm / 2 + 0.5, BIG, BIG),
                       halfplane(xt_u, H + ov - Tr / 2, ang, "right")))
    keepL = union(rect(w / 2 - 2, -BIG, BIG, BIG),
                  diff(rect(-BIG, -BIG, BIG, ym - Tm / 2 - 0.5),
                       halfplane(xt_l, -ov + Tr / 2, ang, "left")))
    U = diff(U, rect(w - d - g.Sr - 1, ym + Tm / 2 - 0.5, BIG, H + ov - Tr + 0.5))
    L = diff(L, rect(-BIG, -ov + Tr - 0.5, g.Sr + 1, ym - Tm / 2 + 0.5))
    g.add(inter(U, keepU))
    g.add(inter(L, keepL))


@glyph("T", [0x54], sides="tt")
def _T(g):
    H, S, w = g.H, g.S, g.w
    g.add(rect(0, H - g.T, w, H))
    g.add(rect(w / 2 - S / 2, 0, w / 2 + S / 2, H))
    if g.logo:
        k = g.T * 0.18
        g.facet_end(k, H - g.T, H, "left")
        g.facet_end(w - k, H - g.T, H)


@glyph("U", [0x55], sides="ss")
def _U(g):
    H, ov, w = g.H, g.ov, g.w
    R = g.rad(H + ov)
    g.add(g.ring(0, -ov, w, H + 600, tl=0, tr=0, bl=R, br=R, sl=g.S, sr=g.S))
    g.clip(rect(-10, -100, w + 10, H))


@glyph("V", [0x56], sides="dd")
def _V(g):
    p, sil, hw, fb = g.vee(0, g.w, g.S * g.P.diag, g.P.facet)
    g.add(p)


@glyph("W", [0x57], sides="dd")
def _W(g):
    H, S, w = g.H, g.S, g.w
    if g.mono:
        # squared W: a U with a centre stem, squircle bottom corners
        R = g.rad(H + g.ov) * 0.8
        g.add(g.ring(0, -g.ov, w, H + 600, tl=0, tr=0, bl=R, br=R, sl=S, sr=S))
        g.clip(rect(-10, -100, w + 10, H))
        sc = S * 0.74
        g.add(rect(w / 2 - sc / 2, 0, w / 2 + sc / 2, H * 0.80))
        return
    td = S * g.P.wdiag
    ang = g.P.wang
    hw = td / math.sin(math.radians(ang))
    run = H / math.tan(math.radians(ang))
    fb = hw * g.P.wfb
    wv = 2 * (run + fb)
    # place the two Vs so the middle apex is as wide as a vertex
    off = w - wv
    for x0 in (0, off):
        p, sil, _, _ = g.vee(x0, x0 + wv, td, ang, flat_t=0)
        g.add(p)


@glyph("X", [0x58], sides="dd")
def _X(g):
    H, w = g.H, g.w
    td = g.S * g.P.diag * 0.97
    hw = td
    for _ in range(4):
        hw = td * math.hypot(w - hw, H) / H
    g.add(stroke(hw / 2, 0, w - hw / 2, H, td))
    g.add(stroke(w - hw / 2, 0, hw / 2, H, td))


@glyph("Y", [0x59], sides="dd")
def _Y(g):
    H, S, w = g.H, g.S, g.w
    td = S * g.P.diag
    yj = g.P.yjoin * H
    xc = w / 2
    hw = td
    for _ in range(4):
        hw = td * math.hypot(xc - S / 2 - 0.0, H - yj) / (H - yj)
    # left arm: top end spans 0..hw, bottom end's left edge on the stem's left edge
    L = poly([(0, H), (hw, H), (xc - S / 2 + hw, yj), (xc - S / 2, yj)])
    sil = poly([(0, H), (w, H), (xc + S / 2, yj), (xc - S / 2, yj)])
    g.add(inter(union(L, gm.mirror_x(L, xc)), sil))
    g.add(rect(xc - S / 2, 0, xc + S / 2, yj + 1))
    g.clip(rect(0, 0, w, H))


@glyph("Z", [0x5A], sides="oo")
def _Z(g):
    H, T, w = g.H, g.T, g.w
    td = g.S * g.P.diag
    g.add(rect(0, H - T, w, H))
    g.add(rect(0, 0, w, T))
    hw = td
    for _ in range(4):
        hw = td * math.hypot(w - hw, H - 2 * T) / (H - 2 * T)
    g.add(stroke(w - hw / 2, H - T, hw / 2, T, td))


# ===================================================================== figures

@glyph("zero", [0x30], "digit", "nn")
def _zero(g):
    H, ov, w = g.H, g.ov, g.w
    if g.mono and not getattr(g, "plain_o_width", False):
        # narrower than O and rounder: a condensed squircle oval
        w0 = w * g.P.zero_w
        x0 = (w - w0) / 2
        R = g.rad(H + 2 * ov)
        g.add(g.ring(x0, -ov, x0 + w0, H + ov, r=(w0 * 0.47, R * 1.12)))
        if not getattr(g, "noslash", False):
            _zero_slash(g, x0, x0 + w0)
        return
    g.add(g.ring(0, -ov, w, H + ov, r=g.rad(H + 2 * ov)))
    if g.mono or getattr(g, "slashed", False):
        _zero_slash(g)


def _zero_slash(g, x0=0, x1=None):
    H, ov = g.H, g.ov
    x1 = g.w if x1 is None else x1
    w = x1 - x0
    ix0, iy0, ix1, iy1 = x0 + g.Sr, -ov + g.Tr, x1 - g.Sr, H + ov - g.Tr
    t = g.T * g.P.zslash
    ang = math.radians(g.P.facet)
    cx, cy = x0 + w / 2, H / 2
    L = (iy1 - iy0) * 0.5 / math.sin(ang) + 40
    dx, dy = L * math.cos(ang), L * math.sin(ang)
    s = pstroke(cx - dx, cy - dy, cx + dx, cy + dy, t)
    g.add(inter(s, rect(ix0 - 5, iy0 - 5, ix1 + 5, iy1 + 5)))


@glyph("one", [0x31], "digit", "nn")
def _one(g):
    H, S, T, w = g.H, g.S, g.T, g.w
    a = math.radians(g.P.flag)
    fl = 0.30 * w
    base = g.mono and not getattr(g, "nobase", False)
    xs = w / 2 - S / 2 + fl * (0.0 if base else 0.42)
    g.add(rect(xs, 0, xs + S, H))
    tv = T * 0.98 / math.cos(a)
    g.add(poly([(xs + 1, H), (xs - fl, H - fl * math.tan(a)),
                (xs - fl, H - fl * math.tan(a) - tv), (xs + 1, H - tv)]))
    if base:
        b = 0.36 * w
        g.add(rect(xs + S / 2 - b, 0, xs + S / 2 + b, T))


@glyph("two", [0x32], "digit", "nn")
def _two(g):
    H, ov, Tm, Tr, w = g.H, g.ov, g.Tm, g.Tr, g.w
    ym = g.P.ym2 * H
    Ru = g.rad(H + ov - ym + Tm / 2)
    Rl = g.rad(ym + Tm / 2)
    U = g.ring(0, ym - Tm / 2, w, H + ov, r=Ru, tl=0, bl=0, tb=Tm)
    L = g.ring(0, 0, w, ym + Tm / 2, tl=Rl, tr=0, br=0, bl=0, sl=g.S, tb=g.T, tt=Tm)
    xt = tend(g, w)
    keepU = union(rect(w / 2 - 2, -BIG, BIG, BIG),
                  diff(rect(-BIG, ym + Tm / 2 + 0.5, BIG, BIG),
                       halfplane(xt, H + ov - Tr / 2, 90 + g.P.tilt, "left")))
    keepL = union(rect(-BIG, -BIG, w / 2 + 2, BIG), rect(-BIG, -BIG, BIG, g.T + 0.5))
    U = diff(U, rect(-BIG, ym + Tm / 2 - 0.5, g.Sr + 1, H + ov - Tr + 0.5))
    g.add(inter(U, keepU))
    g.add(inter(L, keepL))


@glyph("three", [0x33], "digit", "nn")
def _three(g):
    H, ov, Tm, Tr, Sr, w = g.H, g.ov, g.Tm, g.Tr, g.Sr, g.w
    ym = g.P.ym3 * H
    d = 0.035 * w
    Ru = g.rad(H + ov - ym + Tm / 2)
    Rl = g.rad(ym + Tm / 2 + ov)
    g.add(box(d, ym - Tm / 2, w - d, H + ov, r=Ru, tl=0))
    g.add(box(0, -ov, w, ym + Tm / 2, r=Rl, bl=0))
    g.sub(g.counter(-200, ym + Tm / 2, w - d - Sr, H + ov - Tr,
                    tr=g.ir(Ru, Sr, Tr), br=g.ir(Ru, Sr, Tm)))
    g.sub(g.counter(-200, -ov + Tr, w - Sr, ym - Tm / 2,
                    tr=g.ir(Rl, Sr, Tm), br=g.ir(Rl, Sr, Tr)))
    xm = g.P.three_mid * w
    g.sub(rect(-BIG, ym - Tm, xm, ym + Tm))
    g.term(d + tend(g, w), H + ov - Tr / 2, "left", "upper", ylo=ym + Tm / 2 + 1)
    g.term(tend(g, w), -ov + Tr / 2, "left", "lower", yhi=ym - Tm / 2 - 1)


@glyph("four", [0x34], "digit", "nn")
def _four(g):
    H, S, T, w = g.H, g.S, g.T, g.w
    x4 = w - S - 0.11 * w
    y4 = g.P.four_bar * H
    R4 = g.rad(H - y4) * 0.70
    if g.mono:
        # data face: a closed 4 with a facet diagonal reads better at 12 px
        g.add(rect(0, y4, w, y4 + T))
        g.add(rect(x4, 0, x4 + S, H))
        td = S * g.P.diag
        ang = math.atan2(H - y4, x4 + S * 0.5)
        hw = td / math.sin(ang)
        g.add(inter(poly([(0, y4), (hw, y4), (x4 + S, H), (x4 + S - hw, H)]), rect(0, 0, w, H)))
        return
    U = diff(box(0, y4, x4 + S, H + 400, bl=R4),
             g.counter(S, y4 + T, x4, H + 500, bl=g.ir(R4, S, T)))
    g.add(inter(U, rect(-10, -10, w + 10, H)))
    g.add(rect(x4, y4, w, y4 + T))
    g.add(rect(x4, 0, x4 + S, H))


@glyph("five", [0x35], "digit", "nn")
def _five(g):
    H, ov, S, T, Tm, Tr, Sr, w = g.H, g.ov, g.S, g.T, g.Tm, g.Tr, g.Sr, g.w
    yb = g.P.five_bowl * H
    Rb = g.rad(yb + ov)
    g.add(box(0, -ov, w, yb, tl=0, tr=Rb, br=Rb, bl=0))
    g.sub(g.counter(-200, -ov + Tr, w - Sr, yb - Tm, tr=g.ir(Rb, Sr, Tm), br=g.ir(Rb, Sr, Tr)))
    g.add(rect(0, yb - Tm, S, H))
    g.add(rect(0, H - T, w * 0.97, H))
    g.term(tend(g, w), -ov + Tr / 2, "left", "lower", yhi=yb - Tm - 1)


def _six_path(g):
    kk = g.P.six_k                      # Black opens the bowl a little
    H, ov, S, T, Tm, Tr, Sr, w = g.H, g.ov, g.S * kk, g.T, g.Tm * g.P.b8m * kk, g.Tr * kk, g.Sr * kk, g.w
    yb = g.P.six_bowl * H
    Rb = g.rad(yb + ov)
    Rt = g.rad(H + 2 * ov) * 0.92
    up = diff(box(0, yb * 0.45, w, H + ov, tl=Rt, tr=0),
              g.counter(S, yb * 0.45 - 50, w + 100, H + ov - Tr, tl=g.ir(Rt, S, Tr)))
    bowl = diff(box(0, -ov, w, yb, r=Rb),
                g.counter(S, -ov + Tr, w - Sr, yb - Tm,
                          tl=g.ir(Rb, S, Tm), tr=g.ir(Rb, Sr, Tm),
                          br=g.ir(Rb, Sr, Tr), bl=g.ir(Rb, S, Tr)))
    p = union(up, bowl)
    xt = w - tend(g, w)
    ang = 90 - g.P.tilt
    p = diff(p, halfplane(xt, H + ov - Tr / 2, ang, "right", yb + 1, BIG))
    return p


@glyph("six", [0x36], "digit", "nn")
def _six(g):
    g.add(_six_path(g))


@glyph("seven", [0x37], "digit", "nn")
def _seven(g):
    H, S, T, w = g.H, g.S, g.T, g.w
    g.add(rect(0, H - T, w, H))
    ang = math.radians(g.P.seven)
    td = S * g.P.diag
    hw = td / math.sin(ang)
    # right edge passes through (w, H - T)
    c = 1 / math.tan(ang)
    xr_top = w + T * c
    xr_bot = w - (H - T) * c
    g.add(poly([(xr_top - hw, H), (xr_top, H), (xr_bot, 0), (xr_bot - hw, 0)]))
    g.clip(rect(-10, -10, w, H))


@glyph("eight", [0x38], "digit", "nn")
def _eight(g):
    H, ov, Sr, w = g.H, g.ov, g.Sr, g.w
    Tr, Tm = g.Tr * g.P.b8, g.Tm * g.P.b8m
    ym = g.P.ym8 * H
    d = 0.045 * w
    Ru = g.rad(H + ov - ym + Tm / 2)
    Rl = g.rad(ym + Tm / 2 + ov)
    g.add(box(d, ym - Tm / 2, w - d, H + ov, r=Ru))
    g.add(box(0, -ov, w, ym + Tm / 2, r=Rl))
    iu = dict(tl=g.ir(Ru, Sr, Tr), tr=g.ir(Ru, Sr, Tr), br=g.ir(Ru, Sr, Tm), bl=g.ir(Ru, Sr, Tm))
    il = dict(tl=g.ir(Rl, Sr, Tm), tr=g.ir(Rl, Sr, Tm), br=g.ir(Rl, Sr, Tr), bl=g.ir(Rl, Sr, Tr))
    g.sub(g.counter(d + Sr * 0.97, ym + Tm / 2, w - d - Sr * 0.97, H + ov - Tr, **iu))
    g.sub(g.counter(Sr, -ov + Tr, w - Sr, ym - Tm / 2, **il))


@glyph("nine", [0x39], "digit", "nn")
def _nine(g):
    g.add(gm.rotate180(_six_path(g), g.w / 2, g.H / 2))


# ===================================================================== punctuation

def _dot(g, x, y, s=None):
    s = g.P.dot * g.S if s is None else s
    return box(x, y, x + s, y + s, r=s * 0.26)


@glyph("period", [0x2E], "dot", "pp")
def _period(g):
    g.add(_dot(g, 0, 0))


def _comma_path(g, x, y):
    s = g.P.dot * g.S
    a = math.radians(g.P.facet)
    drop = 0.20 * g.H
    tw = s * 0.62
    c = 1 / math.tan(a)
    d = box(x, y, x + s, y + s, tl=s * 0.26, tr=s * 0.26, bl=s * 0.26, br=0)
    tail = poly([(x + s - tw, y + s * 0.4), (x + s, y + s * 0.4),
                 (x + s - (s * 0.4 + drop) * c, y - drop),
                 (x + s - tw - (s * 0.4 + drop) * c, y - drop)])
    return union(d, tail)


@glyph("comma", [0x2C], "dot", "pp")
def _comma(g):
    g.add(_comma_path(g, 0, 0))


def _colon_ys(g):
    s = g.P.dot * g.S
    lo = g.P.colon_lo * g.H
    return lo, g.H - lo - s


@glyph("colon", [0x3A], "dot", "pp")
def _colon(g):
    lo, hi = _colon_ys(g)
    g.add(_dot(g, 0, lo))
    g.add(_dot(g, 0, hi))


@glyph("semicolon", [0x3B], "dot", "pp")
def _semicolon(g):
    lo, hi = _colon_ys(g)
    g.add(_comma_path(g, 0, lo))
    g.add(_dot(g, 0, hi))


@glyph("exclam", [0x21], "dot", "pp")
def _exclam(g):
    s = g.P.dot * g.S
    g.add(_dot(g, 0, 0))
    g.add(rect(0, s + 0.13 * g.H, s, g.H))


@glyph("question", [0x3F], "question", "op")
def _question(g):
    H, ov, Tm, Tr, w, S = g.H, g.ov, g.Tm, g.Tr, g.w, g.S
    s = g.P.dot * S
    yq = 0.40 * H
    Rq = g.rad(H + ov - yq)
    U = g.ring(0, yq, w, H + ov, r=Rq, tl=0, bl=0, tb=Tm)
    xs = w / 2 - S / 2 + 0.04 * w
    xt = tend(g, w)
    keepU = union(rect(xs, -BIG, BIG, BIG),
                  diff(rect(-BIG, yq + Tm + 0.5, BIG, BIG),
                       halfplane(xt, H + ov - Tr / 2, 90 + g.P.tilt, "left")))
    U = diff(U, rect(-BIG, yq + Tm - 0.5, g.Sr + 1, H + ov - Tr + 0.5))
    g.add(inter(U, keepU))
    g.add(rect(xs, s + 0.13 * H, xs + S, yq + Tm))
    g.add(_dot(g, xs + S / 2 - s / 2, 0))


@glyph("quotesingle", [0x27], "quote1", "pp")
def _quotesingle(g):
    g.add(rect(0, g.H * 0.66, g.S * 0.92, g.H))


@glyph("quotedbl", [0x22], "quote2", "pp")
def _quotedbl(g):
    t = g.S * 0.92
    g.add(rect(0, g.H * 0.66, t, g.H))
    g.add(rect(g.w - t, g.H * 0.66, g.w, g.H))


def _dash(g, w):
    y = g.P.dash_y * g.H
    g.add(rect(0, y - g.T * 0.98 / 2, w, y + g.T * 0.98 / 2))


@glyph("hyphen", [0x2D], "hyphen", "pp")
def _hyphen(g):
    _dash(g, g.w)


@glyph("endash", [0x2013], "endash", "pp")
def _endash(g):
    _dash(g, g.w)


@glyph("emdash", [0x2014], "emdash", "pp")
def _emdash(g):
    _dash(g, g.w)


@glyph("underscore", [0x5F], "underscore", "zz")
def _underscore(g):
    g.add(rect(0, -0.17 * g.H, g.w, -0.17 * g.H + g.T * 0.92))


@glyph("bar", [0x7C], "barw", "pp")
def _bar(g):
    g.add(rect(0, -0.17 * g.H, g.w, g.H * 1.13))


def _slash_path(g, w, ext=0.10):
    H = g.H
    td = g.S * g.P.diag * 0.92
    ang = math.radians(g.P.facet)
    hw = td / math.sin(ang)
    y0, y1 = -ext * H, H + ext * H
    run = (y1 - y0) / math.tan(ang)
    return poly([(0, y0), (hw, y0), (hw + run, y1), (run, y1)])


@glyph("slash", [0x2F], "slash", "pp")
def _slash(g):
    g.add(_slash_path(g, g.w))


@glyph("backslash", [0x5C], "slash", "pp")
def _backslash(g):
    p = _slash_path(g, g.w)
    b = p.bounds
    g.add(gm.mirror_x(p, (b[0] + b[2]) / 2))


def _paren_path(g, w):
    H = g.H
    y0, y1 = -0.13 * H, H * 1.13
    R = g.rad(y1 - y0) * 0.78
    t = g.S * 0.92
    p = diff(box(0, y0, w * 4, y1, tl=R, bl=R),
             g.counter(t, y0 + g.T * 0.92, w * 5, y1 - g.T * 0.92,
                       tl=g.ir(R, t, g.T * 0.92), bl=g.ir(R, t, g.T * 0.92)))
    return inter(p, rect(-10, -BIG, w, BIG))


@glyph("parenleft", [0x28], "paren", "pp")
def _parenleft(g):
    g.add(_paren_path(g, g.w))


@glyph("parenright", [0x29], "paren", "pp")
def _parenright(g):
    g.add(gm.mirror_x(_paren_path(g, g.w), g.w / 2))


def _bracket_path(g, w):
    H = g.H
    y0, y1 = -0.13 * H, H * 1.13
    t = g.S * 0.92
    p = union(rect(0, y0, t, y1), rect(0, y0, w, y0 + g.T * 0.92))
    return union(p, rect(0, y1 - g.T * 0.92, w, y1))


@glyph("bracketleft", [0x5B], "bracket", "pp")
def _bracketleft(g):
    g.add(_bracket_path(g, g.w))


@glyph("bracketright", [0x5D], "bracket", "pp")
def _bracketright(g):
    g.add(gm.mirror_x(_bracket_path(g, g.w), g.w / 2))


def _math_y(g):
    return g.P.dash_y * g.H


@glyph("plus", [0x2B], "plus", "pp")
def _plus(g):
    w, t = g.w, g.T * 0.98
    yc = _math_y(g)
    g.add(rect(0, yc - t / 2, w, yc + t / 2))
    g.add(rect(w / 2 - t / 2, yc - w / 2, w / 2 + t / 2, yc + w / 2))


@glyph("equal", [0x3D], "plus", "pp")
def _equal(g):
    w, t = g.w, g.T * 0.92
    yc = _math_y(g)
    gap = 0.11 * g.H
    g.add(rect(0, yc + gap / 2, w, yc + gap / 2 + t))
    g.add(rect(0, yc - gap / 2 - t, w, yc - gap / 2))


@glyph("multiply", [0xD7], "mult", "pp")
def _multiply(g):
    w, t = g.w, g.T * 0.98
    yc = _math_y(g)
    h = w / 2
    a = pstroke(0, yc - h, w, yc + h, t)
    b = pstroke(0, yc + h, w, yc - h, t)
    g.add(inter(union(a, b), rect(0, yc - h, w, yc + h)))


@glyph("asterisk", [0x2A], "aster", "pp")
def _asterisk(g):
    w = g.w
    t = g.T * 0.86
    r = w / 2
    yc = g.H - r * 1.0
    arm = rect(w / 2 - t / 2, yc - r, w / 2 + t / 2, yc + r)
    for deg in (0, 60, 120):
        g.add(gm.rotate(arm, deg, w / 2, yc))


@glyph("percent", [0x25], "percent", "dd")
def _percent(g):
    H, w = g.H, g.w
    t = g.S * 0.74
    tb = g.T * 0.74
    rw = 0.36 * w
    rh = 0.40 * H
    R = g.rad(rh) * 0.9
    ring = g.ring(0, H - rh, rw, H, r=R, sl=t, sr=t, tb=tb, tt=tb)
    g.add(ring)
    g.add(gm.translate(ring, w - rw, -(H - rh)))
    # slash at the facet
    td = g.S * g.P.diag * 0.86
    ang = math.radians(g.P.facet)
    hw = td / math.sin(ang)
    run = H / math.tan(ang)
    x0 = w / 2 - run / 2 - hw / 2
    g.add(poly([(x0, 0), (x0 + hw, 0), (x0 + hw + run, H), (x0 + run, H)]))


@glyph("numbersign", [0x23], "hash", "dd")
def _numbersign(g):
    H, w = g.H, g.w
    t = g.T * 0.92
    ang = math.radians(78)
    td = g.S * 0.86
    hw = td / math.sin(ang)
    run = H / math.tan(ang)
    for xa in (0.18 * w, 0.56 * w):
        g.add(poly([(xa, 0), (xa + hw, 0), (xa + hw + run, H), (xa + run, H)]))
    g.add(rect(0, 0.30 * H - t / 2, w, 0.30 * H + t / 2))
    g.add(rect(0, 0.68 * H - t / 2, w, 0.68 * H + t / 2))
    g.clip(rect(0, 0, w, H))


@glyph("ampersand", [0x26], "amp", "rd")
def _ampersand(g):
    H, ov, S, T, w = g.H, g.ov, g.S, g.T, g.w
    Sa, Ta = S * g.P.amp_k, T * g.P.amp_k
    # upper loop: a small squircle ring, set left of centre
    yu = g.P.amp_yu * H
    xu0, xu1 = 0.08 * w, g.P.amp_xu1 * w
    Ru = g.rad(H + ov - yu)
    g.add(g.ring(xu0, yu, xu1, H + ov, r=Ru, sl=Sa, sr=Sa, tb=Ta, tt=Ta))
    # lower bowl: wider, its upper right opened; the right side stops at an
    # arm height with a facet terminal
    yl = 0.60 * H
    xl1 = 0.80 * w
    Rl = g.rad(yl + ov)
    low = g.ring(0, -ov, xl1, yl, r=Rl, sl=Sa, sr=Sa, tb=Ta, tt=Ta)
    ya = g.P.amp_arm * H
    low = diff(low, rect(xu1 - Sa * 0.5, ya, BIG, BIG))
    g.add(low)
    # leg: from inside the loop's lower right, down to the foot at the right
    td = S * g.P.diag * 0.92
    ang = math.radians(g.P.amp_leg)
    hw = td / math.sin(ang)
    y0 = yu + Ta
    run = y0 / math.tan(ang)
    g.add(poly([(w - hw - run, y0), (w - run, y0), (w, 0), (w - hw, 0)]))
    g.clip(rect(-10, -50, w, H + 50))


@glyph("at", [0x40], "at", "rr")
def _at(g):
    H, ov, S, T, w = g.H, g.ov, g.S, g.T, g.w
    t = S * 0.80
    tb = T * 0.80
    y0, y1 = -0.16 * H, H
    R = g.rad(y1 - y0)
    outer = g.ring(0, y0, w, y1, r=R, sl=t, sr=t, tb=tb, tt=tb)
    ya = 0.17 * H                       # floor of the inner a
    outer = diff(outer, rect(-BIG, y0 - 10, BIG, ya).__class__() if False else rect(w * 0.5, y0 - 10, BIG, ya))
    g.add(outer)
    # inner a: a squircle ring whose right side is the stem
    ax0, ax1 = 0.26 * w, w - t - 0.12 * w
    ay0, ay1 = ya, 0.66 * H
    Ra = g.rad(ay1 - ay0) * 0.9
    g.add(g.ring(ax0, ay0, ax1, ay1, r=Ra, tr=0, br=0, sl=t, sr=t, tb=tb, tt=tb))
    # the stem's foot runs right into the outer ring's side
    g.add(rect(ax1 - t, ay0, w - t * 0.5, ay0 + tb))
    # outer stroke's tail ends under the a with a facet terminal
    g.term(w * 0.62, y0 + tb / 2, "right", "lower", yhi=ya - 1)


def _chevron(g, x1, yc, L, t, shaft_x0):
    h = t * math.sqrt(2)
    head = poly([(x1, yc), (x1 - L, yc + L), (x1 - L - h, yc + L), (x1 - h, yc),
                 (x1 - L - h, yc - L), (x1 - L, yc - L)])
    shaft = rect(shaft_x0, yc - t / 2, x1 - h * 0.6, yc + t / 2)
    return union(head, shaft)


def _arrow_right(g):
    w = g.w
    t = g.T * 0.96
    yc = _math_y(g)
    L = 0.25 * g.H
    return _chevron(g, w, yc, L, t, 0)


@glyph("arrowright", [0x2192], "arrow", "pp")
def _arrowright(g):
    g.add(_arrow_right(g))


@glyph("arrowleft", [0x2190], "arrow", "pp")
def _arrowleft(g):
    g.add(gm.mirror_x(_arrow_right(g), g.w / 2))


def _arrow_up(g):
    # an upward arrow whose shaft spans the cap height (baseline to cap)
    H, w = g.H, g.w
    t = g.T * 0.96
    L = 0.25 * H
    xc = w / 2
    h = t * math.sqrt(2)
    head = poly([(xc, H), (xc + L, H - L), (xc + L, H - L - h), (xc, H - h),
                 (xc - L, H - L - h), (xc - L, H - L)])
    shaft = rect(xc - t / 2, 0, xc + t / 2, H - h * 0.6)
    return union(head, shaft)


@glyph("arrowup", [0x2191], "arrowv", "pp")
def _arrowup(g):
    g.add(_arrow_up(g))


@glyph("arrowdown", [0x2193], "arrowv", "pp")
def _arrowdown(g):
    g.add(gm.transform(_arrow_up(g), 1, 0, 0, -1, 0, g.H))


@glyph("triangleright", [0x25B8], "tri", "pp")
def _triangleright(g):
    yc = _math_y(g)
    s = 0.25 * g.H
    g.add(poly([(0, yc - s), (g.w, yc), (0, yc + s)]))


@glyph("bullet", [0x2022], "bullet", "pp")
def _bullet(g):
    s = g.w
    yc = _math_y(g)
    g.add(box(0, yc - s / 2, s, yc + s / 2, r=s * 0.30))


@glyph("degree", [0xB0], "degree", "pp")
def _degree(g):
    s = g.w
    t = g.T * 0.80
    g.add(g.ring(0, g.H - s, s, g.H, r=s * 0.36, sl=t, sr=t, tb=t, tt=t))


@glyph("space", [0x20, 0xA0], "space", "pp")
def _space(g):
    pass


@glyph(".notdef", [], "notdef", "ss")
def _notdef(g):
    t = g.T * 0.6
    g.add(diff(rect(0, 0, g.w, g.H), rect(t, t, g.w - t, g.H - t)))
