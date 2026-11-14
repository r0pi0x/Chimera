"""Curve geometry for the UGG v2 fonts.

Shapes are skia-pathops paths made of lines and cubic Beziers. The core
primitive is `box`: a rectangle whose corners are superellipse-like
("squircle") corners, each a single cubic whose handles reach a fraction
`k` of the corner radius toward the sharp corner (k = 0.5523 is a circle;
larger k keeps the curvature low where the curve leaves the straight side
and tightens the apex, which is the squircle look). Counters use their own
radius, solved so the diagonal stroke at the corner has a chosen thickness
(`inner_radius`). Glyphs are built by boolean ops, then overlaps are removed
with pathops and the cubics converted to TrueType quadratics (cu2qu)."""
import math
from functools import lru_cache

import pathops
from fontTools.pens.recordingPen import RecordingPen
from fontTools.pens.transformPen import TransformPen

K_OUT = 0.72   # outer squircle corner handle
K_IN = 0.64    # counter corner handle
BIG = 5000


def empty():
    return pathops.Path()


def union(a, b):
    return pathops.op(a, b, pathops.PathOp.UNION, keep_starting_points=False)


def diff(a, b):
    return pathops.op(a, b, pathops.PathOp.DIFFERENCE, keep_starting_points=False)


def inter(a, b):
    return pathops.op(a, b, pathops.PathOp.INTERSECTION, keep_starting_points=False)


def simplify(p):
    return pathops.simplify(p, fix_winding=True, keep_starting_points=False)


def poly(pts):
    p = pathops.Path()
    pen = p.getPen()
    pen.moveTo(tuple(pts[0]))
    for q in pts[1:]:
        pen.lineTo(tuple(q))
    pen.closePath()
    return p


def rect(x0, y0, x1, y1):
    return poly([(x0, y0), (x1, y0), (x1, y1), (x0, y1)])


def _r2(r):
    if isinstance(r, (tuple, list)):
        return float(r[0]), float(r[1])
    return float(r), float(r)


def box(x0, y0, x1, y1, r=0, k=K_OUT, tl=None, tr=None, br=None, bl=None, ks=None):
    """Rectangle with squircle corners. `r` is the default radius; tl/tr/br/bl
    override per corner; a radius may be (rx, ry). `ks` overrides k per corner
    as a dict. Radii are clamped so opposite corners never overlap."""
    w, h = x1 - x0, y1 - y0
    rs = {c: _r2(r if v is None else v) for c, v in
          (("tl", tl), ("tr", tr), ("br", br), ("bl", bl))}
    # clamp
    for a, b, axis, span in (("tl", "tr", 0, w), ("bl", "br", 0, w),
                             ("tl", "bl", 1, h), ("tr", "br", 1, h)):
        s = rs[a][axis] + rs[b][axis]
        if s > span * 0.999 and s > 0:
            f = span * 0.999 / s
            for c in (a, b):
                v = list(rs[c])
                v[axis] *= f
                rs[c] = tuple(v)
    ks = ks or {}
    p = pathops.Path()
    pen = p.getPen()

    def corner(cx, cy, p0, p1, kk):
        # p0 -> p1 around corner (cx, cy)
        c1 = (p0[0] + kk * (cx - p0[0]), p0[1] + kk * (cy - p0[1]))
        c2 = (p1[0] + kk * (cx - p1[0]), p1[1] + kk * (cy - p1[1]))
        pen.lineTo(p0) if (pen_state[0] != p0) else None
        pen.curveTo(c1, c2, p1)
        pen_state[0] = p1

    pen_state = [None]
    bl_, br_, tr_, tl_ = rs["bl"], rs["br"], rs["tr"], rs["tl"]
    start = (x0 + bl_[0], y0)
    pen.moveTo(start)
    pen_state[0] = start

    def go(pt):
        if pen_state[0] != pt:
            pen.lineTo(pt)
            pen_state[0] = pt

    # bottom edge -> br
    if br_[0] > 0 and br_[1] > 0:
        go((x1 - br_[0], y0))
        corner(x1, y0, (x1 - br_[0], y0), (x1, y0 + br_[1]), ks.get("br", k))
    else:
        go((x1, y0))
    if tr_[0] > 0 and tr_[1] > 0:
        go((x1, y1 - tr_[1]))
        corner(x1, y1, (x1, y1 - tr_[1]), (x1 - tr_[0], y1), ks.get("tr", k))
    else:
        go((x1, y1))
    if tl_[0] > 0 and tl_[1] > 0:
        go((x0 + tl_[0], y1))
        corner(x0, y1, (x0 + tl_[0], y1), (x0, y1 - tl_[1]), ks.get("tl", k))
    else:
        go((x0, y1))
    if bl_[0] > 0 and bl_[1] > 0:
        go((x0, y0 + bl_[1]))
        corner(x0, y0, (x0, y0 + bl_[1]), (x0 + bl_[0], y0), ks.get("bl", k))
    else:
        go((x0, y0))
    pen.closePath()
    return p


def _cubic(p0, p1, p2, p3, t):
    u = 1 - t
    return (u ** 3 * p0[0] + 3 * u * u * t * p1[0] + 3 * u * t * t * p2[0] + t ** 3 * p3[0],
            u ** 3 * p0[1] + 3 * u * u * t * p1[1] + 3 * u * t * t * p2[1] + t ** 3 * p3[1])


def _corner_pts(r, k, ox, oy, n):
    # corner at (ox, oy), quadrant interior toward +x +y
    rx, ry = r
    p0 = (ox + rx, oy)
    p3 = (ox, oy + ry)
    p1 = (p0[0] + k * (ox - p0[0]), oy)
    p2 = (ox, p3[1] + k * (oy - p3[1]))
    return [_cubic(p0, p1, p2, p3, i / (n - 1)) for i in range(n)]


@lru_cache(maxsize=4096)
def inner_radius(R, sx, ty, factor=0.96, k_out=K_OUT, k_in=K_IN, rmax=1e9):
    """Counter corner radius (circular) for an outer corner of radius R
    (a number or (rx, ry)) when the strokes meeting there are sx (vertical
    stroke) and ty (horizontal) thick: the thickness across the corner's
    apex becomes factor * mean(sx, ty)."""
    Rr = _r2(R)
    if Rr[0] <= 0:
        return 0.0
    outer = _corner_pts(Rr, k_out, 0, 0, 240)
    target = factor * (sx + ty) / 2

    def thick(ri):
        if ri <= 0.5:
            p = (sx, ty)
        else:
            p = _corner_pts((ri, ri), k_in, sx, ty, 3)[1]
        return min(math.dist(p, q) for q in outer)

    lo, hi = 0.0, min(rmax, max(Rr) * 1.2)
    if thick(hi) < target:
        return hi
    if thick(lo) >= target:
        return 0.0
    for _ in range(40):
        mid = (lo + hi) / 2
        if thick(mid) < target:
            lo = mid
        else:
            hi = mid
    return (lo + hi) / 2


def halfplane(px, py, ang, side, ylo=-BIG, yhi=BIG):
    """Region on one side ('left'/'right') of the line through (px, py) at
    `ang` degrees from horizontal, limited to the band ylo..yhi."""
    c = 1 / math.tan(math.radians(ang)) if abs(ang - 90) > 1e-9 else 0.0

    def x(y):
        return px + (y - py) * c

    if side == "right":
        return poly([(x(ylo), ylo), (BIG, ylo), (BIG, yhi), (x(yhi), yhi)])
    return poly([(-BIG, ylo), (x(ylo), ylo), (x(yhi), yhi), (-BIG, yhi)])


def stroke(x0, y0, x1, y1, t):
    """Straight stroke of perpendicular thickness t whose centre line runs
    (x0,y0)->(x1,y1), with horizontal end faces."""
    dx, dy = x1 - x0, y1 - y0
    h = t * math.hypot(dx, dy) / abs(dy) / 2
    return poly([(x0 - h, y0), (x0 + h, y0), (x1 + h, y1), (x1 - h, y1)])


def hstroke(x0, y0, x1, y1, t):
    """Stroke with vertical end faces (for shallow strokes)."""
    dx, dy = x1 - x0, y1 - y0
    h = t * math.hypot(dx, dy) / abs(dx) / 2
    return poly([(x0, y0 - h), (x1, y1 - h), (x1, y1 + h), (x0, y0 + h)])


def pstroke(x0, y0, x1, y1, t):
    """Stroke with end faces perpendicular to its direction."""
    dx, dy = x1 - x0, y1 - y0
    L = math.hypot(dx, dy)
    nx, ny = -dy / L * t / 2, dx / L * t / 2
    return poly([(x0 + nx, y0 + ny), (x0 - nx, y0 - ny), (x1 - nx, y1 - ny), (x1 + nx, y1 + ny)])


def transform(p, xx=1, xy=0, yx=0, yy=1, dx=0, dy=0):
    out = pathops.Path()
    p.draw(TransformPen(out.getPen(), (xx, xy, yx, yy, dx, dy)))
    return out


def translate(p, dx, dy=0):
    return transform(p, dx=dx, dy=dy)


def rotate180(p, cx, cy):
    return transform(p, -1, 0, 0, -1, 2 * cx, 2 * cy)


def rotate(p, deg, cx, cy):
    a = math.radians(deg)
    c, s = math.cos(a), math.sin(a)
    # x' = c x - s y + dx ; y' = s x + c y + dy, keeping (cx, cy) fixed
    return transform(p, c, s, -s, c, cx - c * cx + s * cy, cy - s * cx - c * cy)


def mirror_x(p, cx):
    return transform(p, -1, 0, 0, 1, 2 * cx, 0)


def flatten(p, n=12):
    """Contours as point lists (cubics/quads sampled)."""
    rec = RecordingPen()
    p.draw(rec)
    out, cur = [], None
    for op, args in rec.value:
        if op == "moveTo":
            cur = [args[0]]
        elif op == "lineTo":
            cur.append(args[0])
        elif op == "curveTo":
            p0 = cur[-1]
            c1, c2, p3 = args
            for i in range(1, n + 1):
                cur.append(_cubic(p0, c1, c2, p3, i / n))
        elif op == "qCurveTo":
            # TrueType-style implied points
            pts = list(args)
            p0 = cur[-1]
            offs, end = pts[:-1], pts[-1]
            segs = []
            for i, o in enumerate(offs):
                e = end if i == len(offs) - 1 else ((o[0] + offs[i + 1][0]) / 2, (o[1] + offs[i + 1][1]) / 2)
                segs.append((o, e))
            for o, e in segs:
                for j in range(1, n + 1):
                    t = j / n
                    u = 1 - t
                    cur.append((u * u * p0[0] + 2 * u * t * o[0] + t * t * e[0],
                                u * u * p0[1] + 2 * u * t * o[1] + t * t * e[1]))
                p0 = e
        elif op in ("closePath", "endPath"):
            if cur:
                out.append(cur)
            cur = None
    return out


def area(pts):
    s = 0.0
    for i in range(len(pts)):
        x0, y0 = pts[i - 1]
        x1, y1 = pts[i]
        s += x0 * y1 - x1 * y0
    return s / 2


def bounds(p):
    if not p.contours or len(list(p.segments)) == 0:
        return None
    return p.bounds


def warp(p, fn):
    """Apply a point map (x, y) -> (x', y') to every on- and off-curve point.
    Not exact for curves under a non-affine map; fine for the gentle tapers
    used here (the curves are short corners)."""
    rec = RecordingPen()
    p.draw(rec)
    out = pathops.Path()
    pen = out.getPen()
    for op, args in rec.value:
        getattr(pen, op)(*[fn(*a) for a in args])
    return out


def taper(p, xc, k, H):
    """Linear taper about x = xc: at height y, x moves toward xc by the
    factor (1 - k * y / H). Vertical lines become straight slanted lines."""
    return warp(p, lambda x, y: (xc + (x - xc) * (1 - k * y / H), y))
