"""Build UGG Display (Black, Bold, Regular) and UGG Mono (Regular).

    python build.py                    # all four into apps/ultimate_gun_game/fonts/
    python build.py black mono         # some of them
    UGG_FONTS_OUT=<dir> python build.py   # elsewhere
    UGG_FONTS_WOFF2=1 python build.py     # also WOFF2 (needs brotli)

Requirements and licence: README.md beside this file (OFL-1.1; the fonts'
FONTLOG.txt in apps/ultimate_gun_game/fonts/).

Glyph drawings: glyphs2.py; curve geometry: geom2.py. Outlines are built
with cubic squircle corners, overlaps removed with skia-pathops, then
converted to TrueType quadratics with cu2qu (max error 0.5 unit).
"""
import io
import math
import os
import sys
from types import SimpleNamespace as NS

import pathops
from fontTools.feaLib.builder import addOpenTypeFeaturesFromString
from fontTools.fontBuilder import FontBuilder
from fontTools.pens.cu2quPen import Cu2QuPen
from fontTools.pens.ttGlyphPen import TTGlyphPen
from fontTools.pens.recordingPen import RecordingPen
from fontTools.ttLib import TTFont

import geom2 as gm
from glyphs2 import GLYPHS, G

HERE = os.path.dirname(os.path.abspath(__file__))
OUT = os.environ.get("UGG_FONTS_OUT") or os.path.join(HERE, "..", "..", "apps", "ultimate_gun_game", "fonts")
WOFF2 = os.environ.get("UGG_FONTS_WOFF2") == "1"
UPM = 1000
VERSION = 0.4

BASE = dict(
    H=700, ov=8, R=272, rfrac=0.45, cf=0.95, round_gain=1.03, mid_gain=0.93,
    tilt=20, facet=70, diag=0.95, mdiag=0.84, wdiag=0.86, wang=73,
    ym=0.505, ymB=0.53, ymS=0.52, ym2=0.47, ym3=0.53, ym8=0.535,
    abar=0.22, gbar=0.47, jstub=0.44, karm=0.16, kleg=0.50, mvert=0.20,
    pbowl=0.40, rbowl=0.43, yjoin=0.42, c_open=0.40, tend=0.10, s_open=0.40, qtail=58,
    rleg=64, wfb=0.55, amp_arm=0.36, b8=0.94, b8m=0.88, amp_leg=52, qtop=0.30, qdrop=0.12, qout=0.04, zslash=0.80, flag=25, four_bar=0.25, five_bowl=0.60, six_bowl=0.60,
    seven=66, three_mid=0.30, dot=1.08, colon_lo=0.10, dash_y=0.47,
    mono=False, kern_factor=0.5,
    six_k=1.0, amp_k=0.94, amp_yu=0.52, amp_xu1=0.60,
    a_slit=0.17, a_taper=0.20, a_round=0.80, a_sq_bar=0.33,
)

# body widths in cap heights (Bold); Regular scales by `wscale`
WIDTHS = dict(
    A=1.04, B=0.98, C=1.02, D=1.04, E=0.86, F=0.84, G=1.06, H=1.02, J=0.88,
    K=1.00, L=0.82, M=1.24, N=1.04, O=1.10, P=0.96, Q=1.10, R=1.00, S=0.98,
    T=0.96, U=1.02, V=1.04, X=1.04, Y=1.04, Z=0.92, digit=0.82,
    question=0.80, hyphen=0.44, endash=0.72, emdash=1.36, paren=0.30,
    bracket=0.28, plus=0.60, mult=0.48, aster=0.44, percent=1.12, hash=0.96,
    amp=1.08, at=1.22, arrow=0.96, arrowv=0.58, tri=0.40, bullet=0.30,
    degree=0.34, underscore=0.80, notdef=0.62,
)

ROUND_L = set("CGOQS") | {"zero.pnum", "six.pnum", "eight.pnum", "nine.pnum"}
ROUND_R = set("BDOQS") | {"zero.pnum", "eight.pnum", "nine.pnum", "six.pnum"}

SIDES = dict(c=-0.30, s=1.0, r=0.84, o=0.50, t=0.26, d=0.14, p=0.80, n=0.86, z=0.0)

FONTS = {
    "bold": dict(
        family="UGG Display", style="Bold", ps="UGGDisplay-Bold", file="ugg-display-bold",
        weight=700, S=148, T=128, sb=60, space=230, wscale=1.0,
    ),
    "black": dict(
        family="UGG Display", style="Black", ps="UGGDisplay-Black", file="ugg-display-black",
        weight=900, S=200, T=166, sb=54, space=230, wscale=1.0, mid_gain=0.88, b8=0.90,
        b8m=0.80, cf=0.93, six_bowl=0.64, six_k=0.90, amp_k=0.80, amp_yu=0.49, amp_xu1=0.66,
    ),
    "regular": dict(
        family="UGG Display", style="Regular", ps="UGGDisplay-Regular", file="ugg-display-regular",
        weight=400, S=88, T=80, sb=70, space=230, wscale=0.97, ov=7, cf=0.96,
    ),
    "mono": dict(
        family="UGG Mono", style="Regular", ps="UGGMono-Regular", file="ugg-mono-regular",
        weight=400, S=86, T=78, sb=60, space=0, H=660, R=236, ov=7, mono=True,
        advance=620, body=496, zero_w=0.80, tend=0.05, cf=0.96, mdiag=0.70, diag=0.92, zslash=0.66,
    ),
}

MONO_W = dict(  # fractions of the mono body width
    question=0.84, hyphen=0.62, endash=1.0, emdash=1.25, paren=0.40, bracket=0.36,
    plus=0.80, mult=0.66, aster=0.66, percent=1.0, hash=1.0, amp=1.0, at=1.04,
    arrow=1.0, arrowv=0.80, tri=0.56, bullet=0.38, degree=0.46, underscore=1.25,
    notdef=0.8,
)


def make_params(key):
    d = dict(BASE)
    d.update(FONTS[key])
    P = NS(**d)
    P.key = key
    return P


def width_for(P, wkey):
    H, S = P.H, P.S
    if P.mono:
        if wkey in MONO_W:
            return P.body * MONO_W[wkey]
        if wkey in ("dot",):
            return P.dot * S
        if wkey == "quote1":
            return S * 0.92
        if wkey == "quote2":
            return S * 0.92 * 2 + 0.12 * H
        if wkey == "slash":
            return None
        if wkey == "barw":
            return S * 0.80
        if wkey == "space":
            return 0
        return P.body
    sc = P.wscale
    if wkey == "W":
        td = S * P.wdiag
        hw = td / math.sin(math.radians(P.wang))
        run = H / math.tan(math.radians(P.wang))
        fb = hw * P.wfb
        wv = 2 * (run + fb)
        return 2 * wv - 2 * (hw - fb)
    if wkey == "I":
        return S
    if wkey == "dot":
        return P.dot * S
    if wkey == "quote1":
        return S * 0.92
    if wkey == "quote2":
        return S * 0.92 * 2 + 0.12 * H
    if wkey == "slash":
        return None
    if wkey == "barw":
        return S * 0.80
    if wkey == "space":
        return 0
    f = WIDTHS[wkey]
    if wkey in ("A", "V", "M", "N", "O", "Q", "C", "G", "D", "H", "U", "X", "Y", "K", "R",
                "B", "P", "S", "E", "F", "L", "T", "Z", "J", "digit"):
        return f * H * sc
    return f * H * (1.0 if P.key in ("bold", "black") else 0.98)


def draw(P, name, w, **extra):
    cps, wkey, sides, f = GLYPHS[name]
    g = G(P, w if w is not None else 0)
    for k, v in extra.items():
        setattr(g, k, v)
    f(g)
    return gm.simplify(g.p)


def place(P, name, path, wkey, sides, w):
    """Return (positioned path, advance)."""
    b = path.bounds if len(list(path.segments)) else None
    if name == "space":
        return path, (P.advance if P.mono else P.space)
    if P.mono:
        A = P.advance
        if b is None:
            return path, A
        if sides[0] in "pz" or wkey in MONO_W or w is None or wkey in ("dot", "quote1", "quote2", "barw"):
            if wkey == "underscore" or wkey == "emdash":
                return gm.translate(path, -b[0] + (A - (b[2] - b[0])) / 2), A
            cx = (b[0] + b[2]) / 2
            return gm.translate(path, A / 2 - cx), A
        return gm.translate(path, (A - w) / 2), A
    sb = P.sb
    lf, rf = SIDES[sides[0]], SIDES[sides[1]]
    if b is None:
        return path, round(w)
    if sides[0] in "pz":
        x0, x1 = b[0], b[2]
    else:
        x0, x1 = 0, w
    lsb, rsb = sb * lf, sb * rf
    adv = round(lsb + (x1 - x0) + rsb)
    return gm.translate(path, lsb - x0), adv


ORDER_LETTERS = [chr(c) for c in range(0x41, 0x5B)]
DIGITS = ["zero", "one", "two", "three", "four", "five", "six", "seven", "eight", "nine"]
PUNCT = ["period", "comma", "colon", "semicolon", "exclam", "question", "quotesingle",
         "quotedbl", "hyphen", "endash", "emdash", "slash", "backslash", "parenleft",
         "parenright", "bracketleft", "bracketright", "plus", "equal", "percent",
         "numbersign", "ampersand", "at", "asterisk", "multiply", "arrowright", "arrowleft",
         "arrowup", "arrowdown", "triangleright", "bullet", "underscore", "bar", "degree"]
ORDER = [".notdef", "space"] + ORDER_LETTERS + DIGITS + PUNCT

# logo-set A styles: (feature, style); ss02's own A is A_DEFAULT
A_STYLES = [("ss03", "plain"), ("ss04", "slit"), ("ss05", "squircle"), ("ss06", "split")]
A_DEFAULT = "plain"


# tabular figures whose ink sits off-centre (open left sides) move left a little
OPTICAL_SHIFT = {"three": -0.035, "seven": -0.03}


def build_glyphs(P):
    gl = {}
    for n in ORDER:
        cps, wkey, sides, f = GLYPHS[n]
        w = width_for(P, wkey)
        path = draw(P, n, w)
        pp, adv = place(P, n, path, wkey, sides, w)
        if n in OPTICAL_SHIFT:
            pp = gm.translate(pp, OPTICAL_SHIFT[n] * w)
        gl[n] = NS(name=n, cps=cps, path=pp, adv=adv)
    if not P.mono:
        # proportional figures: same drawings, sidebearings fitted to the ink
        for n in DIGITS:
            p = gl[n].path
            b = p.bounds
            sb = P.sb * (0.70 if n == "one" else 0.84)
            pp = gm.translate(p, sb - b[0])
            gl[n + ".pnum"] = NS(name=n + ".pnum", cps=[], path=pp, adv=round(2 * sb + b[2] - b[0]))
        # slashed zero (feature `zero`)
        w = width_for(P, "digit")
        z = draw(P, "zero", w, slashed=True)
        pp, adv = place(P, "zero", z, "digit", "nn", w)
        gl["zero.slash"] = NS(name="zero.slash", cps=[], path=pp, adv=adv)
    else:
        # dotless/plain zero as an alternate for the mono (feature `ss01`)
        w = width_for(P, "digit")
        z = draw(P, "zero", w, noslash=True)
        pp, adv = place(P, "zero", z, "digit", "nn", w)
        gl["zero.plain"] = NS(name="zero.plain", cps=[], path=pp, adv=adv)
        # a 1 without a base (feature `ss03`)
        o = draw(P, "one", w, nobase=True)
        pp, adv = place(P, "one", o, "digit", "nn", w)
        gl["one.nobase"] = NS(name="one.nobase", cps=[], path=pp, adv=adv)
    if P.key == "black":
        # ss02: the wordmark set; every letter tighter, E G L T arm ends cut
        # parallel to the symbol's 70-degree facets. The A comes in four
        # styles (A_STYLES); ss02 uses A_DEFAULT, ss03-ss06 pick one each.
        P2 = NS(**vars(P))
        P2.sb = P.sb * 0.80
        for n in ORDER_LETTERS:
            cps, wkey, sides, f = GLYPHS[n]
            w = width_for(P, wkey)
            extra = dict(a_style=A_DEFAULT) if n == "A" else {}
            if n == "A" and A_DEFAULT == "squircle":
                sides = "ss"
            path = draw(P, n, w, logo=True, **extra)
            pp, adv = place(P2, n, path, wkey, sides, w)
            gl[n + ".ss02"] = NS(name=n + ".ss02", cps=[], path=pp, adv=adv)
        for feat, style in A_STYLES:
            w = width_for(P, "A")
            path = draw(P, "A", w, logo=True, a_style=style)
            pp, adv = place(P2, "A", path, "A", "ss" if style == "squircle" else GLYPHS["A"][2], w)
            gl["A." + feat] = NS(name="A." + feat, cps=[], path=pp, adv=adv)
    return gl


# ------------------------------------------------------------------ kerning

def profiles(polys, ys):
    L, R = [], []
    for y in ys:
        xs = []
        for c in polys:
            n = len(c)
            for i in range(n):
                x0, y0 = c[i - 1]
                x1, y1 = c[i]
                if (y0 > y) != (y1 > y):
                    xs.append(x0 + (y - y0) * (x1 - x0) / (y1 - y0))
        if xs:
            L.append(min(xs))
            R.append(max(xs))
        else:
            L.append(None)
            R.append(None)
    return L, R


def auto_kern(P, gl, lefts, rights, ref_pair=("H", "H")):
    H = P.H
    ys = [y + 0.5 for y in range(-120, H + 120, 10)]
    names = set(lefts) | set(rights) | set(ref_pair)
    polys = {n: gm.flatten(gl[n].path, 8) for n in names}
    prof = {n: profiles(polys[n], ys) for n in names}
    band = [(i, y) for i, y in enumerate(ys) if 0 <= y <= H]
    a0, b0 = ref_pair
    ref = gl[a0].adv - max(x for x in prof[a0][1] if x is not None) + min(x for x in prof[b0][0] if x is not None)
    cap = ref + 0.20 * H
    pairs = {}
    for a in lefts:
        Ra = prof[a][1]
        for b in rights:
            Lb = prof[b][0]
            gaps = [gl[a].adv - ra + lb for ra, lb in zip(Ra, Lb) if ra is not None and lb is not None]
            if not gaps:
                continue
            mn = min(gaps)
            tot, cnt = 0.0, 0
            for i, y in band:
                ra, lb = Ra[i], Lb[i]
                cnt += 1
                if ra is None and lb is None:
                    tot += cap
                elif ra is None or lb is None:
                    tot += cap
                else:
                    tot += min(gl[a].adv - ra + lb, cap)
            eff = tot / cnt
            k = (ref - eff) * P.kern_factor
            k = max(k, 0.55 * ref - mn)       # never closer than 55% of HH
            k = int(round(k / 5.0) * 5)
            if abs(k) >= 10:
                pairs[(a, b)] = k
    return pairs, ref


# ------------------------------------------------------------------ output

def tt_glyph(path):
    pen = TTGlyphPen(None)
    cpen = Cu2QuPen(pen, max_err=0.5, reverse_direction=True)
    path.draw(RoundingPen(cpen))
    return pen.glyph()


class RoundingPen:
    """Pass-through that keeps float coordinates (TTGlyphPen rounds)."""
    def __init__(self, out):
        self.out = out

    def moveTo(self, p):
        self.out.moveTo(p)

    def lineTo(self, p):
        self.out.lineTo(p)

    def curveTo(self, *ps):
        self.out.curveTo(*ps)

    def qCurveTo(self, *ps):
        self.out.qCurveTo(*ps)

    def closePath(self):
        self.out.closePath()

    def endPath(self):
        self.out.endPath()


def build_font(key):
    P = make_params(key)
    gl = build_glyphs(P)
    order = list(ORDER)
    extra = [n for n in gl if n not in order]
    order += extra

    pairs = {}
    if not P.mono:
        punct_k = ["period", "comma", "colon", "semicolon", "exclam", "question", "quotesingle",
                   "quotedbl", "hyphen", "endash", "slash", "parenleft", "parenright",
                   "bracketleft", "bracketright", "ampersand", "asterisk", "backslash", "underscore"]
        L = ORDER_LETTERS + [d + ".pnum" for d in DIGITS]
        lefts = L + ["period", "comma", "quotesingle", "quotedbl", "hyphen", "endash", "parenleft",
                     "bracketleft", "slash", "ampersand", "asterisk"]
        rights = L + ["period", "comma", "colon", "semicolon", "quotesingle", "quotedbl", "hyphen",
                      "endash", "parenright", "bracketright", "slash", "exclam", "question",
                      "ampersand", "asterisk"]
        raw, ref = auto_kern(P, gl, lefts, rights)
        pn = set(d + ".pnum" for d in DIGITS)
        # only shapes with an open or diagonal side get kerned; rounds and
        # stems are spaced by their sidebearings alone
        open_r = set("AFKLPRTVWXYZ") | {"seven.pnum", "one.pnum", "four.pnum"} | set(punct_k)
        open_l = set("AJTVWXYZ") | {"one.pnum", "four.pnum", "seven.pnum"} | set(punct_k)
        for (a, b), k in raw.items():
            if a not in open_r and b not in open_l:
                continue
            # a round meeting an open side reads close already: kern it gently
            if b in ROUND_L or a in ROUND_R:
                k = max(k, -int(0.5 * P.sb) // 5 * 5)
                if abs(k) < 10:
                    continue
            # letters: all pairs; figures: only proportional figures, and only
            # against punctuation or letters (tabular figures never move)
            if a in pn and b in pn:
                continue
            if a in ("parenleft", "bracketleft") and b in ("parenright", "bracketright"):
                continue
            if a in punct_k and b in punct_k:
                continue
            pairs[(a, b)] = k
        if P.key == "black":
            # wordmark set: kerned among itself, a little tighter
            ss = [n + ".ss02" for n in ORDER_LETTERS] + ["A." + f for f, _ in A_STYLES]
            P3 = NS(**vars(P))
            P3.kern_factor = 0.7
            raw2, _ = auto_kern(P3, gl, ss, ss)
            for (a, b), k in raw2.items():
                ba, bb = a.split(".")[0], b.split(".")[0]
                if ba not in open_r and bb not in open_l:
                    continue
                if bb in ROUND_L or ba in ROUND_R:
                    k = max(k, -int(0.5 * P.sb) // 5 * 5)
                    if abs(k) < 10:
                        continue
                pairs[(a, b)] = k

    fb = FontBuilder(UPM, isTTF=True)
    fb.setupGlyphOrder(order)
    cmap = {}
    for n in ORDER:
        for cp in gl[n].cps:
            cmap[cp] = n
    for c in ORDER_LETTERS:
        cmap[ord(c.lower())] = c
    fb.setupCharacterMap(cmap)

    glyf, metrics = {}, {}
    ymin_all, ymax_all = 0, P.H
    for n in order:
        g = tt_glyph(gl[n].path) if len(list(gl[n].path.segments)) else TTGlyphPen(None).glyph()
        glyf[n] = g
    fb.setupGlyf(glyf)
    gf = fb.font["glyf"]
    for n in order:
        g = gf[n]
        g.recalcBounds(gf)
        lsb = g.xMin if g.numberOfContours else 0
        if g.numberOfContours:
            ymin_all, ymax_all = min(ymin_all, g.yMin), max(ymax_all, g.yMax)
        metrics[n] = (gl[n].adv, lsb)
    fb.setupHorizontalMetrics(metrics)
    asc, desc = (850, -150) if not P.mono else (820, -160)
    fb.setupHorizontalHeader(ascent=asc, descent=desc, lineGap=0)
    lic = ("This Font Software is licensed under the SIL Open Font License, Version 1.1. "
           "This license is available with a FAQ at: https://openfontlicense.org")
    vs = f"Version {VERSION:.3f}"
    desc_txt = ("Display face for Ultimate Gun Game: wide, square-shouldered capitals whose "
                "bowls are squircles, open terminals cut 20 degrees off vertical and diagonals "
                "at 70 degrees, both taken from the game's logo facets. Caps only: lowercase maps "
                "to capitals. Tabular figures by default; pnum gives proportional figures, zero a "
                "slashed zero.")
    if P.key == "black":
        desc_txt += (" ss02 is the wordmark set (tighter, arm ends cut at 70 degrees); "
                     "ss03-ss06 choose its A: plain, slit crossbar, squircle, split.")
    if P.mono:
        desc_txt = ("Data monospace for Ultimate Gun Game, companion to UGG Display: squircle bowls, "
                    "every glyph on one advance, slashed zero by default (ss01 gives a plain zero). "
                    "Caps only: lowercase maps to capitals.")
    fb.setupNameTable({
        "copyright": "Copyright 2026 Lucker Party contributors",
        "familyName": P.family if P.style in ("Regular", "Bold") else f"{P.family} {P.style}",
        "styleName": P.style if P.style in ("Regular", "Bold") else "Regular",
        "typographicFamily": P.family,
        "typographicSubfamily": P.style,
        "uniqueFontIdentifier": f"{VERSION:.3f};NONE;{P.ps}",
        "fullName": f"{P.family} {P.style}",
        "version": vs,
        "psName": P.ps,
        "manufacturer": "Lucker Party contributors",
        "designer": "Lucker Party contributors",
        "description": desc_txt,
        "licenseDescription": lic,
        "licenseInfoURL": "https://openfontlicense.org",
    })
    bold = P.style == "Bold"
    fb.setupOS2(
        sTypoAscender=asc, sTypoDescender=desc, sTypoLineGap=0,
        usWinAscent=max(asc, ymax_all + 10), usWinDescent=max(-desc, -ymin_all + 10),
        sCapHeight=P.H, sxHeight=P.H, usWeightClass=P.weight, fsType=0,
        achVendID="NONE", fsSelection=(1 << 7) | ((1 << 5) if bold else (1 << 6)),
        ulCodePageRange1=1, version=4,
        xAvgCharWidth=P.advance if P.mono else 600,
    )
    fb.setupPost(italicAngle=0, isFixedPitch=1 if P.mono else 0,
                 underlinePosition=-110, underlineThickness=60)
    fb.setupHead(unitsPerEm=UPM, fontRevision=VERSION, macStyle=1 if bold else 0)
    fb.font["head"].created = fb.font["head"].modified = 3846528000
    pan = fb.font["OS/2"].panose
    pan.bFamilyType, pan.bSerifStyle, pan.bWeight = 2, 11, (8 if bold else 5)
    pan.bProportion = 9 if P.mono else 3
    fb.font["OS/2"].recalcUnicodeRanges(fb.font)
    fb.font["OS/2"].recalcAvgCharWidth(fb.font)

    fea = ["languagesystem DFLT dflt;", "languagesystem latn dflt;"]
    if not P.mono:
        subs = " ".join(f"sub {d} by {d}.pnum;" for d in DIGITS)
        back = " ".join(f"sub {d}.pnum by {d};" for d in DIGITS)
        fea += [f"feature pnum {{ {subs} }} pnum;",
                f"feature tnum {{ {back} }} tnum;",
                "feature zero { sub zero by zero.slash; } zero;"]
        if P.key == "black":
            src = " ".join(ORDER_LETTERS)
            dst = " ".join(n + ".ss02" for n in ORDER_LETTERS)
            fea += ["feature ss02 {", f"  sub [{src}] by [{dst}];", "} ss02;"]
            for feat, style in A_STYLES:
                fea.append(f"feature {feat} {{ sub [A A.ss02] by A.{feat}; }} {feat};")
        fea.append("feature kern {")
        for (a, b), k in sorted(pairs.items()):
            fea.append(f"  pos {a} {b} {k};")
        fea.append("} kern;")
    else:
        fea += ["feature ss01 { sub zero by zero.plain; } ss01;",
                "feature ss03 { sub one by one.nobase; } ss03;"]
    addOpenTypeFeaturesFromString(fb.font, "\n".join(fea))

    out_ttf = os.path.join(OUT, P.file + ".ttf")
    fb.save(out_ttf)
    if WOFF2:
        f2 = TTFont(out_ttf)
        f2.flavor = "woff2"
        f2.save(os.path.join(OUT, P.file + ".woff2"))
    return P, out_ttf, pairs, len(order), gl


# ------------------------------------------------------------------ validation

def glyph_contours(font, name):
    gs = font.getGlyphSet()
    rec = RecordingPen()
    gs[name].draw(rec)
    p = pathops.Path()
    rec.replay(p.getPen())
    return gm.flatten(p, 10), p, rec


def validate(ttf):
    """Round-trip (XML and WOFF2), every glyph: closed contours, TrueType
    winding (outer clockwise, holes counter-clockwise), no overlaps or
    self-intersections."""
    f = TTFont(ttf)
    buf = io.StringIO()
    f.saveXML(buf)
    f3 = TTFont()
    buf.seek(0)
    f3.importXML(buf)
    out = io.BytesIO()
    f3.save(out)
    out.seek(0)
    f4 = TTFont(out)
    problems = []
    if f4.getGlyphOrder() != f.getGlyphOrder():
        problems.append("XML round trip changed the glyph order")
    wf = TTFont(ttf.replace(".ttf", ".woff2")) if WOFF2 else None
    if wf and wf.getGlyphOrder() != f.getGlyphOrder():
        problems.append("WOFF2 glyph order differs")
    for n in f.getGlyphOrder():
        a = f["glyf"][n]
        if wf and a.numberOfContours and a.getCoordinates(f["glyf"])[0] != wf["glyf"][n].getCoordinates(wf["glyf"])[0]:
            problems.append(f"{n}: WOFF2 outline differs")
        c4 = f4["glyf"][n]
        if a.numberOfContours and a.getCoordinates(f["glyf"])[0] != c4.getCoordinates(f4["glyf"])[0]:
            problems.append(f"{n}: XML round-trip outline differs")
    for n in f.getGlyphOrder():
        if f["glyf"][n].numberOfContours == 0:
            continue
        cs, p, rec = glyph_contours(f, n)
        ops = [op for op, _ in rec.value]
        if ops.count("moveTo") != ops.count("closePath"):
            problems.append(f"{n}: open contour")
        for i, c in enumerate(cs):
            if len(c) < 3:
                problems.append(f"{n}: degenerate contour")
                continue
            # parity depth of a point just inside this contour's ink side
            m = _probe(c)
            depth = sum(1 for j, o in enumerate(cs) if j != i and _inside(m, o))
            hole = depth % 2 == 1
            cw = gm.area(c) < 0
            if cw == hole:
                problems.append(f"{n}: contour {i} wrong winding")
        s = pathops.simplify(p, fix_winding=True)
        a1 = sum(-gm.area(c) for c in cs)
        a2 = abs(s.area)
        if abs(a1 - a2) > max(4, 0.002 * a2):
            problems.append(f"{n}: overlap/self-intersection (area {a1:.0f} vs {a2:.0f})")
    return problems


def _probe(c):
    a, b = c[0], c[1]
    return ((a[0] + b[0]) / 2, (a[1] + b[1]) / 2)


def _inside(pt, poly):
    x, y = pt
    ins = False
    n = len(poly)
    for i in range(n):
        x0, y0 = poly[i - 1]
        x1, y1 = poly[i]
        if (y0 > y) != (y1 > y):
            xi = x0 + (y - y0) * (x1 - x0) / (y1 - y0)
            if xi > x:
                ins = not ins
    return ins


if __name__ == "__main__":
    keys = sys.argv[1:] or list(FONTS)
    for k in keys:
        P, ttf, pairs, count, gl = build_font(k)
        probs = validate(ttf)
        print(f"{P.family} {P.style}: {count} glyphs, {len(pairs)} kern pairs, "
              f"{os.path.getsize(ttf)} B ttf; "
              f"problems: {probs or 'none'}")
