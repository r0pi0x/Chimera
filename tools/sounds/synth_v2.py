#!/usr/bin/env python3
"""Ultimate Gun Game's "Signal" UI sound kit, v2: breakcore grit, arcade-FPS punch (kept for A/B; v3 is synth.py).

Everything is synthesized here: our own drum hits (kick, snare, hat from
oscillators and noise; no recorded breaks of any kind), bit-crushing and
sample-rate reduction, saturation, tape wobble, gated noise, ring
modulation, and breakcore's signature edit: chopped, retriggered
micro-slices (buffer repeats and accelerating snare rolls). Deterministic:
fixed seeds, seeded dither.

  venv/bin/python synth_v2.py -> v2/wav/*.wav (48 kHz, 16-bit mono), ogg/*.ogg
                                (page copies), manifest.json (event table),
                                incl. menu_loop (a 16-bar 174 BPM soundtrack sketch)

Key: D Lydian (as v1), so the ladder still climbs one scale step per rung.
Sub-bass is always driven so its 2nd and 3rd harmonics carry it on laptop
speakers. Repeated sounds are low-passed (6.5-8 kHz) after crushing so the
aliasing grit stays warm, never fizzy. The v1 primitives and loudness code
are reused from synth_v1.py.

Licence: our own work, CC0 1.0.
"""
import json, os, shutil, subprocess
import numpy as np
from scipy import signal
from scipy.io import wavfile
from synth_v1 import (SR, T, env, blip, rng, svf, slant, click, sos, hpf, lpf, room, Mix, lufs_m, peak_db,
                      finish, write_wav, hz, deg)

HERE = os.path.dirname(os.path.abspath(__file__))
WAV, OGG = os.path.join(HERE, 'v2', 'wav'), os.path.join(HERE, 'v2', 'ogg')
MANIFEST, PREFIX = os.path.join(HERE, 'v2', 'manifest.json'), 'v2/wav/'
OGG_Q = '2'          # the page's copies only; the engine ships the WAVs
BPM = 174
S16 = 60 / BPM / 4   # a 16th note, 86.2 ms
S32, S64 = S16 / 2, S16 / 4


# ---------------------------------------------------------------- grit
def crush(x, bits=8, down=1):
    """Sample-rate reduction (sample and hold) and bit reduction."""
    if down > 1:
        x = x[(np.arange(len(x)) // down) * down]
    q = 2 ** (bits - 1)
    return np.round(x * q) / q


def sat(x, drive=3.0, asym=0.0):
    """tanh waveshaper; asym adds even harmonics (tape / transistor colour)."""
    return (np.tanh(drive * (x + asym)) - np.tanh(drive * asym)) / np.tanh(drive)


def wobble(x, depth_ms=1.2, rate=4.0, phase=0.0):
    """Tape wow: a delay line swept by a sine."""
    n = np.arange(len(x))
    d = depth_ms * SR / 1000 * (1 + np.sin(2 * np.pi * rate * n / SR + phase)) / 2
    return np.interp(n - d, n, x, left=0)


def ring(x, f):
    return x * np.sin(2 * np.pi * f * np.arange(len(x)) / SR)


def fade(x, ms=0.8):
    f = min(len(x) // 2, int(ms * SR / 1000))
    if f > 0:
        x = x.copy(); x[:f] *= np.linspace(0, 1, f); x[-f:] *= np.linspace(1, 0, f)
    return x


def repitch(x, r):
    """Resample by ratio r (r > 1 higher and shorter)."""
    n = int(len(x) / r)
    return np.interp(np.arange(n) * r, np.arange(len(x)), x)


def stutter(x, t0, sl, reps, decay=0.9, rise=1.0, crush_bits=None):
    """Buffer repeat: the slice x[t0 : t0+sl] retriggered reps times (each
    pitched by rise**k, gain decay**k), returned as one array."""
    i = int(t0 * SR); n = int(sl * SR)
    piece = x[i:i + n]
    out = []
    for k in range(reps):
        p = fade(repitch(piece, rise ** k)[:n], 0.6)
        p = np.pad(p, (0, n - len(p)))
        if crush_bits:
            p = crush(p, max(3, crush_bits - k), 1 + k // 2)
        out.append(p * decay ** k)
    return np.concatenate(out)


def osc(f, d, shape='saw', pw=0.5, hmax=7000.0):
    """Band-limited saw / square / pulse by additive synthesis (f scalar or array)."""
    t = T(d)
    f = np.broadcast_to(np.asarray(f, float), t.shape)
    ph = 2 * np.pi * np.cumsum(f) / SR
    nmax = max(1, int(hmax / float(np.max(f))))
    y = np.zeros_like(t)
    for k in range(1, nmax + 1):
        if shape == 'saw':
            y += np.sin(k * ph) / k
        elif shape == 'square':
            if k % 2: y += np.sin(k * ph) / k
        else:   # pulse of width pw
            y += np.sin(np.pi * k * pw) / k * np.cos(k * ph)
    return y / (np.max(np.abs(y)) + 1e-12)


def sub(f, d, tau=0.2, drive=2.2, a=0.003, glide=None):
    """Sub-bass with its own harmonics (drive): heard on small speakers too."""
    t = T(d)
    fr = np.full_like(t, f) if not glide else glide[0] + (f - glide[0]) * np.exp(-t / glide[1])
    ph = 2 * np.pi * np.cumsum(fr) / SR
    y = sat(np.sin(ph) + 0.25 * np.sin(2 * ph), drive, 0.08)
    return y * env(d, a, tau)


# ---------------------------------------------------------------- drums (ours)
def kick(d=0.4, f0=180, f1=46, ptau=0.03, tau=0.16, drive=2.5, snap=0.5, seed=1):
    t = T(d)
    fr = f1 + (f0 - f1) * np.exp(-t / ptau)
    ph = 2 * np.pi * np.cumsum(fr) / SR
    y = sat(np.sin(ph), drive, 0.05) * env(d, 0.001, tau)
    c = click(seed, 0.004, 2500)
    y[:len(c)] += snap * c
    return y


def snare(d=0.2, tone=185, seed=2, tau=0.07, ntau=0.055, body=0.6, bright=7000, drive=2.0):
    t = T(d)
    b = (np.sin(2 * np.pi * tone * t) + 0.5 * np.sin(2 * np.pi * tone * 1.72 * t)) * env(d, 0.001, tau * 0.5)
    nz = rng(seed).standard_normal(len(t))
    nz = signal.sosfilt(sos('bandpass', [900, bright]), nz) * env(d, 0.001, ntau)
    nz /= np.max(np.abs(nz)) + 1e-12
    return sat(body * b + nz, drive)


def hat(d=0.05, seed=3, tau=0.012, lp=9000):
    """Metallic: six inharmonic squares through a high band, short."""
    t = T(d)
    y = sum(np.sign(np.sin(2 * np.pi * f * t + k)) for k, f in enumerate([317, 421, 563, 677, 811, 977]))
    y = y + 0.6 * rng(seed).standard_normal(len(t))
    y = signal.sosfilt(sos('bandpass', [5500, lp]), y)
    y /= np.max(np.abs(y)) + 1e-12
    return y * env(d, 0.0005, tau)


def roll(m, t0, t1, n, hit, g0=0.4, g1=1.0, curve=1.6, pitch1=1.0):
    """n hits from t0 to t1, accelerating (curve > 1), crescendo, pitch rising."""
    for k in range(n):
        u = k / max(1, n - 1)
        tt = t0 + (t1 - t0) * (1 - (1 - u) ** curve)
        m.add(tt, repitch(hit, 1 + (pitch1 - 1) * u), g0 + (g1 - g0) * u)
    return m


def crash(d=1.0, seed=4, tau=0.35, lp=9000):
    nz = rng(seed).standard_normal(int(d * SR))
    y = signal.sosfilt(sos('bandpass', [3000, lp]), nz) + 0.5 * ring(signal.sosfilt(sos('bandpass', [2000, 6000]), nz), 590)
    return y / np.max(np.abs(y)) * env(d, 0.002, tau)


def stab(notes, d, tau=0.18, drive=3.0, cut0=8000, cut1=1500, ctau=0.08, shape='saw', bits=None, down=1, detune=0.004):
    """A driven chord stab with a closing low-pass sweep (blockwise)."""
    y = sum(osc(hz(n) * (1 + detune * (k % 3 - 1)), d, shape) for k, n in enumerate(notes)) / len(notes)
    y = sat(y * 1.4, drive, 0.1) * env(d, 0.002, tau)
    if bits:
        y = crush(y, bits, down)
    # sweep: 32-sample blocks through a one-pole-ish low-pass with a falling cutoff
    t = T(d)
    fc = cut1 + (cut0 - cut1) * np.exp(-t / ctau)
    return svf_lp(y, fc, 0.9)


def svf_lp(x, fc, q=0.8):
    """TPT state-variable low-pass with a per-sample cutoff."""
    g = np.tan(np.pi * np.clip(np.broadcast_to(np.asarray(fc, float), x.shape), 20, SR * 0.45) / SR)
    k = 1.0 / q
    out = np.empty_like(x); ic1 = ic2 = 0.0
    for n in range(len(x)):
        gn = g[n]
        a1 = 1.0 / (1.0 + gn * (gn + k)); a2 = gn * a1; a3 = gn * a2
        v3 = x[n] - ic2; v1 = a1 * ic1 + a2 * v3; v2 = ic2 + a2 * ic1 + a3 * v3
        ic1 = 2 * v1 - ic1; ic2 = 2 * v2 - ic2
        out[n] = v2
    return out


def tame(x, f=7000, o=4):
    """Keep crushed grit warm: low-pass the fizz on repeated sounds."""
    return lpf(x, f, o)


# ---------------------------------------------------------------- the event table
S = {}
TIER = {'whisper': -42, 'soft': -37, 'tap': -32, 'cue': -29, 'event': -26, 'hero': -22, 'music': -20}


def sound(id_, screen, tier, trigger, character, var=(0, 0), default=True, lufs=None, loop=False):
    def deco(fn):
        S[id_] = dict(fn=fn, screen=screen, tier=tier, target=lufs if lufs is not None else TIER[tier],
                      trigger=trigger, character=character, var_cents=var[0], var_db=var[1],
                      default_on=default, overlap='', loop=loop)
        return fn
    return deco


# ---- menu
@sound('ui_hover', 'menu', 'whisper', 'Pointer or focus lands on a menu entry, card or button',
       'Crunchy tick: a 20 ms D7 square blip crushed to 5 bits at 16 kHz, a grain of gated noise, low-passed '
       'at 6.5 kHz so it never fizzes', var=(70, 1.5))
def _():
    m = Mix(0.025)
    m.add(0, crush(osc(hz(98), 0.02, 'square') * env(0.02, 0.0005, 0.005), 5, 3), 0.8)
    m.add(0, crush(rng(11).standard_normal(int(0.004 * SR)) * env(0.004, 0.0003, 0.001), 4, 2), 0.25)
    return tame(m.y, 6500)


@sound('ui_select', 'menu', 'tap', 'An entry or button is pressed (Play, a tab, a map card)',
       'Arcade blip-up (A5 to D6 in 12 ms, 25 % pulse, 6-bit) on a tiny kick thump and a snare snap',
       var=(30, 1.0))
def _():
    m = Mix(0.1)
    m.add(0, kick(0.07, 240, 90, 0.01, 0.025, 2, 0.6, 21), 0.55)
    p = osc(hz(81) + (hz(86) - hz(81)) * np.clip(T(0.08) / 0.012, 0, 1), 0.08, 'pulse', 0.25)
    m.add(0.003, crush(p * env(0.08, 0.001, 0.02), 6, 2), 0.5)
    m.add(0, snare(0.03, 220, 22, 0.01, 0.006, 0.2), 0.25)
    return tame(m.y, 8000)


@sound('ui_back', 'menu', 'tap', 'Back, close or cancel (Esc)',
       'The select reversed: a crushed pulse blip that drops D6 to A5, a softer thump', var=(30, 1.0), lufs=-33)
def _():
    m = Mix(0.1)
    m.add(0, kick(0.06, 180, 70, 0.01, 0.02, 2, 0.3, 23), 0.4)
    p = osc(hz(86) + (hz(81) - hz(86)) * np.clip(T(0.09) / 0.03, 0, 1), 0.09, 'pulse', 0.25)
    m.add(0.002, crush(p * env(0.09, 0.001, 0.025), 6, 2), 0.55)
    return tame(m.y, 7000)


@sound('ui_denied', 'menu', 'tap', 'A locked or unavailable entry is pressed',
       'Two gated buzzes (a D3 square ring-modulated at 37 Hz, 5-bit), the classic "nope", dark', var=(0, 0.5))
def _():
    m = Mix(0.17)
    for t0 in (0, 0.075):
        b = ring(osc(hz(50), 0.06, 'square', hmax=3000), 37) + 0.5 * osc(hz(50), 0.06, 'square', hmax=3000)
        m.add(t0, crush(sat(b, 3) * env(0.06, 0.002, 0.03, 0.02), 5, 2), 1.0)
    return lpf(m.y, 3000)


@sound('ui_slider', 'menu', 'whisper', 'A slider moves one step (the engine sets speed 0.8 to 1.25 by value)',
       'A 12 ms crushed tick', var=(40, 1.0))
def _():
    return tame(crush(osc(hz(93), 0.012, 'square') * env(0.012, 0.0005, 0.003), 4, 3), 6500)


@sound('menu_backdrop', 'menu', 'whisper', 'The menu backdrop swaps (every 16 s), optional',
       'A tape-wobbled noise swell that ends in a 3-slice stutter; quiet', default=False, lufs=-41)
def _():
    sw = slant(0.7, 400, 3000, 41, q=1.4, a=0.45, tau=0.2, curve=0.8)
    sw = wobble(sw, 2.0, 3.0)
    m = Mix(1.0)
    m.add(0, sw[:int(0.62 * SR)], 1.0)
    m.add(0.62, stutter(sw, 0.62 - S32, S32, 3, 0.7, 1.0, 7), 0.9)
    return lpf(m.y, 6000)


# ---- logo pass
def hold(p):
    q = 2 * np.clip(p, 0, 1) - 1
    return 0.5 + 0.5 * np.sign(q) * np.abs(q) ** 3


@sound('pass_sweep', 'pass', 'event', 'The logo pass (1.2 s; covered 450-940 ms, swap at 600 ms)',
       'A chopped whoosh: the noise band rides the slab\'s speed but is cut into 1/32-note slices at 174 BPM, '
       'some retriggered; a snare roll builds into the cover, a kick and a driven sub drop (110 to 36 Hz) land '
       'on it, a crushed 3x glitch on the swap, then the whoosh chops out with hats on the stripes')
def _():
    d = 1.36
    t = T(d); p = t / 1.2
    v = (2 * np.clip(p, 0, 1) - 1) ** 2
    v = np.where(p > 1, np.exp(-(t - 1.2) / 0.04), v)
    nz = svf(rng(51).standard_normal(len(t)), 260 + 5200 * v ** 0.9, 1.5)
    nz = sat(nz / np.max(np.abs(nz)) * 1.5, 2) * (0.1 + 0.9 * v ** 0.7) * np.clip(t / 0.04, 0, 1) ** 2
    # chop: 1/32 slices; in the fast parts some slices mute or repeat the one before
    r = rng(57); n = int(S32 * SR); w = nz.copy()
    for k in range(int(len(t) / n)):
        a = k * n; tm = a / SR
        if 0.42 < tm < 0.92:
            continue
        c = r.random()
        if c < 0.22:
            w[a:a + n] *= 0.15
        elif c < 0.45 and k > 0:
            w[a:a + n] = fade(nz[a - n:a], 0.6)
        else:
            w[a:a + n] = fade(w[a:a + n], 0.6)
    m = Mix(d)
    m.add(0, lpf(w, 9000), 0.8)
    roll(m, 0.16, 0.41, 7, crush(snare(0.08, 200, 52, 0.03, 0.03, 0.4), 8, 2), 0.12, 0.45, 1.8, 1.25)
    m.add(0.44, kick(0.4, 150, 40, 0.03, 0.14, 3, 0.5, 53), 0.9)
    m.add(0.44, sub(55, 0.5, 0.16, 2.5, glide=(110, 0.05)), 0.8)
    m.add(0.44, crush(crash(0.3, 54, 0.06, 7000), 6, 3), 0.25)
    g = crush(osc(hz(74), 0.012, 'square') * env(0.012, 0.0005, 0.006), 5, 3)
    m.add(0.60, np.tile(np.pad(g, (0, int(0.004 * SR))), 3), 0.35)          # the swap: a crushed glitch
    m.add(0.84, crush(osc(np.linspace(hz(93), hz(81), int(0.06 * SR)), 0.06, 'pulse', 0.25)
                      * env(0.06, 0.001, 0.02), 6, 2), 0.15)
    for k, tt in enumerate([1.0, 1.043, 1.086, 1.129, 1.165, 1.205]):
        m.add(tt, hat(0.04, 55 + k, 0.01), 0.18 + 0.05 * k)
    return m.y


# ---- lobby
@sound('lobby_ready', 'lobby', 'tap', 'You press F3 and become ready',
       'Arcade "power-up": A4 then D5, 25 % pulse, driven and 8-bit, a sub kick under the second note, a hat',
       var=(0, 0.8), lufs=-30)
def _():
    m = Mix(0.32)
    for t0, n, d in [(0, 69, 0.07), (0.07, 74, 0.22)]:
        y = sat(osc(hz(n), d, 'pulse', 0.25), 2.5) * env(d, 0.001, 0.045 if d < 0.1 else 0.08)
        m.add(t0, crush(y, 8, 2), 0.6)
    m.add(0.07, kick(0.15, 160, 60, 0.015, 0.05, 2.5, 0.2, 61), 0.5)
    m.add(0.07, hat(0.04, 62), 0.15)
    return tame(m.y, 8000)


@sound('lobby_unready', 'lobby', 'tap', 'You press F3 again and are no longer ready',
       'Power-down: D5 then A4 that tape-stops (sags three semitones), darker filter, no kick',
       var=(0, 0.8), lufs=-31)
def _():
    m = Mix(0.32)
    m.add(0, crush(sat(osc(hz(74), 0.07, 'pulse', 0.25), 2) * env(0.07, 0.001, 0.04), 8, 2), 0.55)
    d = 0.24; t = T(d)
    f = hz(69) * 2 ** (-3 * np.clip((t - 0.06) / 0.16, 0, 1) ** 2 / 12)
    m.add(0.07, crush(sat(osc(f, d, 'pulse', 0.3), 2) * env(d, 0.001, 0.09), 7, 2), 0.6)
    return lpf(m.y, 4500)


@sound('lobby_peer', 'lobby', 'soft', 'Another player readies up (or joins)', 'A soft crushed D6 pip',
       var=(40, 1.5), lufs=-38)
def _():
    return tame(crush(osc(hz(86), 0.08, 'pulse', 0.3) * env(0.08, 0.002, 0.02), 6, 2), 6500)


def tickhit(n=86, seed=71):
    m = Mix(0.05)
    m.add(0, hat(0.035, seed, 0.008, 8000), 0.5)
    m.add(0, crush(osc(hz(n), 0.03, 'square') * env(0.03, 0.0005, 0.006), 5, 2), 0.6)
    return m.y


@sound('count_tick', 'lobby', 'tap', 'Each second of a countdown (the lobby start; the map vote\'s last 10 s)',
       'A closed hat with a crushed D6 tick', var=(10, 0.7), lufs=-34)
def _():
    return tame(tickhit(), 7500)


@sound('count_tick_3', 'lobby', 'cue', 'The countdown shows 3', 'A snare with a flam (two hits, a 1/32 apart)',
       var=(5, 0.5), lufs=-31)
def _():
    sn = snare(0.18, 190, 72, 0.05, 0.05)
    m = Mix(0.3)
    m.add(0, sn, 0.45); m.add(S32, sn, 0.9)
    m.add(S32, tickhit(88, 72), 0.4)
    return tame(m.y, 8000)


@sound('count_tick_2', 'lobby', 'cue', 'The countdown shows 2', 'A 1/32 snare roll of four, pitching up, into a hit',
       var=(5, 0.5), lufs=-30)
def _():
    sn = snare(0.16, 195, 73, 0.045, 0.045)
    m = Mix(0.36)
    roll(m, 0, 3 * S32, 4, sn, 0.35, 0.95, 1.0, 1.08)
    m.add(3 * S32, tickhit(90, 73), 0.45)
    m.add(3 * S32, kick(0.15, 170, 60, 0.015, 0.05, 2.5, 0.2, 74), 0.4)
    return tame(m.y, 8000)


@sound('count_tick_1', 'lobby', 'cue', 'The countdown shows 1',
       'Eight-hit snare roll accelerating from 1/32 to 1/64, pitching up and crushing as it goes, a sub hit under '
       'the start; ends 0.42 s in, before the go', var=(5, 0.5), lufs=-29)
def _():
    sn = snare(0.12, 200, 75, 0.04, 0.04)
    m = Mix(0.55)
    for k in range(8):
        tt = 0.32 * (1 - (1 - k / 7) ** 1.5)
        m.add(tt, crush(repitch(sn, 1 + 0.06 * k), 9 - k // 2, 1 + k // 3), 0.35 + 0.08 * k)
    m.add(0, sub(hz(38), 0.3, 0.1, 2.5), 0.5)
    m.add(0.32, tickhit(93, 76), 0.5)
    return tame(m.y, 8000)


@sound('count_go', 'lobby', 'event', 'The countdown reaches 0 and the match starts',
       'Drop: kick and a driven D sub, a distorted D power chord (D3 A3 D4 F#4) that stutters three 1/64 slices '
       'before it rings, a crushed crash')
def _():
    st = stab([50, 57, 62, 66], 1.0, tau=0.3, drive=4, cut0=7000, cut1=1400, ctau=0.15)
    m = Mix(1.2)
    m.add(0, stutter(st, 0, S64, 3, 0.85, 1.0), 0.8)
    m.add(3 * S64, st, 0.8)
    m.add(3 * S64, kick(0.5, 170, 42, 0.03, 0.18, 3, 0.5, 81), 1.0)
    m.add(3 * S64, sub(hz(26), 0.8, 0.25, 2.5, glide=(hz(38), 0.04)), 0.7)
    m.add(3 * S64, crush(crash(0.9, 82, 0.3, 8000), 7, 2), 0.3)
    m.add(3 * S64, snare(0.2, 185, 83), 0.4)
    return wobble(m.y, 0.6, 5)


# ---- HUD
@sound('hud_chip', 'hud', 'soft', 'A HUD chip slides in (threat, objective, status)',
       'A crushed F#6 pip over two gated noise grains', var=(30, 1.0), lufs=-35)
def _():
    m = Mix(0.08)
    for k in range(2):
        m.add(k * S64, crush(hpf(rng(91 + k).standard_normal(int(0.012 * SR)), 2000) * env(0.012, 0.001, 0.004), 5, 2), 0.4)
    m.add(0.02, crush(osc(hz(90), 0.05, 'pulse', 0.25) * env(0.05, 0.001, 0.014), 6, 2), 0.6)
    return tame(m.y, 7000)


@sound('feed_self', 'hud', 'soft', 'Your own kill enters the kill feed (off where an announcer pack calls kills)',
       'A crunchy snap: snare grain and a crushed A6 pip', var=(30, 1.0), default=False, lufs=-36)
def _():
    m = Mix(0.07)
    m.add(0, snare(0.03, 230, 92, 0.008, 0.008, 0.2), 0.4)
    m.add(0.003, crush(osc(hz(93), 0.05, 'square') * env(0.05, 0.001, 0.012), 5, 2), 0.6)
    return tame(m.y, 7000)


@sound('tab_open', 'hud', 'whisper', 'The Tab board opens (0.12 s), optional', 'A crushed air slant up',
       var=(40, 1.0), default=False)
def _():
    return tame(crush(slant(0.07, 1200, 4500, 93, q=1.8, a=0.02, tau=0.018), 6, 2), 6000)


@sound('tab_close', 'hud', 'whisper', 'The Tab board closes (0.09 s), optional', 'The same, down and shorter',
       var=(40, 1.0), default=False)
def _():
    return tame(crush(slant(0.05, 4000, 1200, 94, q=1.8, a=0.004, tau=0.012), 6, 2), 6000)


# ---- rank-up: an arcade level-up stab per rung
def rank(n):
    i = n - 2
    m = Mix(0.75)
    m.add(0, kick(0.2, 210, 60, 0.012, 0.07, 2.5, 0.4, 100 + n), 0.55)
    m.add(0, snare(0.06, 210, 110 + n, 0.015, 0.015, 0.3), 0.25)
    for k, nm in enumerate([deg(i), deg(i + 2), deg(i + 4)]):            # 3-note blip-up
        m.add(0.004 + 0.021 * k, crush(osc(hz(nm + 12), 0.024, 'pulse', 0.25) * env(0.024, 0.0005, 0.012), 7, 2), 0.35)
    notes = [deg(i), deg(i + 4), deg(i + 7)]
    st = stab(notes, 0.55, tau=0.13, drive=3.5, cut0=8000, cut1=1800, ctau=0.07, shape='saw', bits=8, down=2)
    t0 = 0.066
    m.add(t0, stutter(st, 0, S64, 2, 0.8), 0.75)                        # chopped twice, then it rings
    m.add(t0 + 2 * S64, st, 0.75)
    m.add(t0 + 2 * S64, sub(hz(deg(i) - 24), 0.35, 0.1, 2.5), 0.5)
    if n >= 10:
        m.add(t0 + 2 * S64, crush(osc(hz(deg(i + 14)), 0.25, 'square') * env(0.25, 0.002, 0.06), 6, 2), 0.18)
    m.add(0.38, hat(0.03, 120 + n, 0.008), 0.2)                         # the number lands
    return tame(m.y, 9000)


for _n in range(2, 14):
    sound(f'rank_{_n:02d}', 'rank', 'event', f'You reach rung {_n} (a kill moves you up the ladder)',
          'Level-up stab: kick and snare snap, a 3-note crushed blip-up, then a driven saw chord (1-5-8 of the '
          'rung\'s scale degree) chopped twice at 1/64 before it rings, its filter closing, a driven sub under it; '
          'the whole stab climbs a scale step per rung' + (', a square sparkle two octaves up' if _n >= 10 else ''),
          lufs=-27)((lambda n: lambda: rank(n))(_n))


@sound('rank_14_knife', 'rank', 'hero', 'You reach the knife rung (14, the last)',
       'Breakcore drop: a ten-hit snare roll that accelerates and pitches up, then a big distorted Lydian chord '
       '(D2 A2 D3 A3 E4 G#4) on a kick, sub drop and crash, chopped into a buffer-repeat cascade (1/16, 1/32, '
       '1/64, crushing deeper each time) before the chord rings out with tape wobble and a ring-modulated "blade"')
def _():
    m = Mix(2.0)
    sn = snare(0.1, 200, 140, 0.035, 0.035)
    for k in range(10):
        tt = 0.3 * (1 - (1 - k / 9) ** 1.7)
        m.add(tt, crush(repitch(sn, 1 + 0.05 * k), 9 - k // 3, 1 + k // 4), 0.2 + 0.06 * k)
    t0 = 0.33
    st = stab([38, 45, 50, 57, 64, 68], 1.3, tau=0.4, drive=5, cut0=9000, cut1=1600, ctau=0.25, bits=8, down=2)
    casc = np.concatenate([stutter(st, 0, S16, 2, 0.95), stutter(st, 0, S32, 2, 0.9, 1.0, 8),
                           stutter(st, 0, S64, 4, 0.9, 1.06, 7)])
    m.add(t0, casc, 0.8)
    t1 = t0 + len(casc) / SR
    for tt in (t0, t1):
        m.add(tt, kick(0.5, 170, 38, 0.03, 0.2, 3.5, 0.5, 141), 0.9)
    m.add(t1, wobble(st, 1.5, 5.5), 0.85)
    m.add(t1, sub(hz(26), 1.2, 0.4, 2.8, glide=(hz(38), 0.05)), 0.75)
    m.add(t1, crush(crash(1.2, 142, 0.45, 9000), 7, 2), 0.3)
    m.add(t1, ring(blip(2794.0, 0.5, tau=0.1, ratio=3.73, index=2.0), 311) * 0.8, 0.25)
    return lpf(room(m.y, 0.5, 0.15, 143), 11000)


@sound('rank_chain', 'rank', 'soft', 'A second rank-up lands while the toast still shows (under 1.15 s)',
       'Layered on the next rung: a crushed E7 glint retriggered in three 1/64 slices', lufs=-34)
def _():
    g = crush(osc(hz(100), 0.06, 'square') * env(0.06, 0.001, 0.02), 6, 2)
    m = Mix(0.15)
    m.add(0, stutter(np.pad(g, (0, 2000)), 0, S64, 3, 0.85, 1.12), 0.7)
    return tame(m.y, 8000)


# ---- air strike
@sound('air_alert', 'air', 'event', 'The air-strike banner opens (0.25 s into the strike; stands in for the first pulse)',
       'A siren-ish stab falling a tritone (G#4 to D4), ring-modulated at 30 Hz and crushed, over a gated noise '
       'riser and a snare hit', lufs=-26)
def _():
    m = Mix(0.6)
    nz = slant(0.25, 600, 3500, 151, q=2, a=0.08, tau=0.06)
    g = (np.floor(T(0.25) / S64) % 2 == 0).astype(float)
    m.add(0, nz * g, 0.3)
    for t0, n, d in [(0.02, 68, 0.12), (0.13, 62, 0.4)]:
        b = osc(hz(n), d, 'saw', hmax=5000)
        b = 0.6 * b + 0.6 * ring(b, 30)
        m.add(t0, crush(sat(b, 3) * env(d, 0.003, d / 3.5), 7, 2), 0.7)
    m.add(0.13, snare(0.2, 180, 152), 0.35)
    m.add(0.13, sub(hz(38), 0.3, 0.1), 0.4)
    return lpf(m.y, 7000)


@sound('air_pulse', 'air', 'cue', 'Each countdown second of the strike, with the screen edge\'s pulse',
       'A driven sub pulse on D1 with its harmonics plus a growling G#2 square, shaped like the edge glow (80 ms '
       'rise, 0.32 s fall), slight tape wobble', var=(0, 0.5), lufs=-28)
def _():
    d = 0.85
    y = sub(hz(26), d, 0.2, 3.0, a=0.08) + 0.35 * sat(osc(hz(44), d, 'square', hmax=1500), 2) * env(d, 0.08, 0.2)
    return lpf(wobble(y, 0.8, 6), 1600)


@sound('air_impact', 'air', 'hero', 'The strike lands (UI layer only; the world\'s explosions are the game\'s)',
       'A heavy kick and sub drop, a 4-bit crushed noise burst that stutters out in three repeats', lufs=-24)
def _():
    m = Mix(0.9)
    m.add(0, kick(0.7, 130, 30, 0.04, 0.25, 4, 0.6, 161), 1.0)
    m.add(0, sub(hz(26), 0.7, 0.22, 3, glide=(hz(38), 0.05)), 0.6)
    bz = crush(signal.sosfilt(sos('bandpass', [300, 3500]), rng(162).standard_normal(int(0.12 * SR))), 4, 6)
    bz = bz / np.max(np.abs(bz)) * env(0.12, 0.002, 0.05)
    m.add(0, bz, 0.4)
    m.add(0.12, stutter(np.pad(bz, (0, 4000)), 0, S32, 3, 0.6, 0.9), 0.35)
    return lpf(m.y, 7000)


# ---- match end
@sound('end_reveal', 'end', 'hero', 'The winner is revealed on the match-end screen',
       'Arcade stinger: a seven-note crushed square run up, a snare-roll pickup, then kick, crash and a big '
       'driven Dmaj9(#11) stab chopped twice at 1/32 before it rings with tape wobble', lufs=-24)
def _():
    m = Mix(1.8)
    for k, i in enumerate([0, 2, 4, 7, 9, 11, 14]):
        m.add(0.032 * k, crush(osc(hz(deg(i) + 12), 0.04, 'square') * env(0.04, 0.001, 0.02), 7, 2), 0.3)
    roll(m, 0.12, 0.24, 4, snare(0.08, 200, 171, 0.03, 0.03), 0.15, 0.4, 1.0, 1.1)
    t0 = 0.26
    st = stab([50, 57, 64, 66, 73, 80], 1.4, tau=0.4, drive=4, cut0=9000, cut1=1800, ctau=0.2)
    m.add(t0, stutter(st, 0, S32, 2, 0.9), 0.8)
    m.add(t0 + 2 * S32, wobble(st, 1.0, 5), 0.8)
    for tt in (t0, t0 + 2 * S32):
        m.add(tt, kick(0.45, 170, 42, 0.03, 0.16, 3, 0.5, 172), 0.8)
    m.add(t0 + 2 * S32, sub(hz(38), 1.0, 0.3, 2.5), 0.6)
    m.add(t0 + 2 * S32, crush(crash(1.2, 173, 0.45, 9000), 7, 2), 0.3)
    m.add(t0 + 2 * S32, snare(0.2, 185, 174), 0.5)
    return lpf(room(m.y, 0.45, 0.15, 175), 11000)


def vote(nm, k):
    m = Mix(0.13)
    m.add(0, kick(0.06, 220, 90, 0.01, 0.02, 2, 0.6, 180 + k), 0.45)
    m.add(0.002, crush(osc(hz(nm), 0.1, 'pulse', 0.25) * env(0.1, 0.001, 0.028), 6, 2), 0.6)
    m.add(0, hat(0.03, 185 + k, 0.007), 0.15)
    return tame(m.y, 8000)


for _k, _nm in enumerate([74, 76, 78, 81]):
    sound(f'vote_cast_{_k + 1}', 'end', 'tap', f'You vote for map card {_k + 1} (or change your vote to it)',
          'The select\'s thump and hat with a crushed pulse on the card\'s own note (D5, E5, F#5, A5)',
          var=(0, 1.0), lufs=-31)((lambda nm, k: lambda: vote(nm, k))(_nm, _k))


@sound('vote_other', 'end', 'soft', 'Someone else votes (a pip lands on a card)', 'A soft crushed A5 pip',
       var=(40, 1.5), lufs=-39)
def _():
    return tame(crush(osc(hz(81), 0.06, 'pulse', 0.3) * env(0.06, 0.002, 0.016), 6, 2), 6500)


@sound('vote_lock', 'end', 'event', 'The vote countdown hits 0: the vote locks (the pass then starts the next match)',
       'Gritty "ka-chunk": a snare and a ring-modulated metal clank, then a kick, a crushed D2 square stab and '
       'a driven sub, the second hit retriggered once at 1/64')
def _():
    m = Mix(0.5)
    m.add(0, snare(0.1, 230, 191, 0.025, 0.025), 0.5)
    m.add(0, crush(ring(osc(hz(74), 0.06, 'square', hmax=6000), 517) * env(0.06, 0.0005, 0.018), 6, 2), 0.5)
    h = Mix(0.4)
    h.add(0, kick(0.35, 190, 45, 0.02, 0.1, 3, 0.6, 192), 1.0)
    h.add(0, crush(sat(osc(hz(38), 0.3, 'square', hmax=3000), 3) * env(0.3, 0.002, 0.07), 6, 2), 0.5)
    h.add(0, sub(hz(38), 0.35, 0.1, 2.5), 0.5)
    m.add(0.075, stutter(h.y, 0, S64, 1, 1.0), 0.6)
    m.add(0.075 + S64, h.y, 0.9)
    return lpf(m.y, 8000)


# ---- the menu loop sketch
@sound('menu_loop', 'music', 'music', 'The main menu (a sketch of the soundtrack\'s direction); loops seamlessly',
       '16 bars at 174 BPM (22.1 s). Our own one-bar break (kick, snare, ghost notes, hats) chopped into 16th '
       'slices and re-sequenced per bar: retriggers, 1/32 and 1/64 snare rolls, reversed and repitched slices, '
       'a fill every 4 bars. A driven sub line and a tape-wobbled detuned pad on D, E, Bm, A (two bars each), a '
       'crushed square motif every 4 bars. Sidechained to the kick.', lufs=-20, loop=True)
def _():
    bar = 16 * S16
    L = 16 * bar
    N = int(round(L * SR))
    # our break: one bar, 16 steps
    src = Mix(bar + 0.5)
    K = kick(0.35, 175, 46, 0.03, 0.14, 2.8, 0.5, 201)
    SN = snare(0.22, 185, 202, 0.06, 0.06)
    GH = snare(0.1, 200, 203, 0.03, 0.03, 0.4)
    for s in (0, 6, 10):
        src.add(s * S16, K, 1.0)
    for s in (4, 12):
        src.add(s * S16, SN, 0.8)
    for s, g in ((7, 0.22), (9, 0.18), (14, 0.25), (15, 0.3)):
        src.add(s * S16, GH, g)
    for s in range(16):
        src.add(s * S16, hat(0.04 if s % 2 else 0.07, 210 + s, 0.012 if s % 2 else 0.03), 0.12 if s % 2 else 0.2)
    brk = src.y
    n16 = int(round(S16 * SR))
    slices = [brk[k * n16:(k + 1) * n16 + 200] for k in range(16)]
    r = rng(220)
    out = Mix(L + 1.0)
    kick_times = []
    for b in range(16):
        seq = list(range(16))
        edits = []
        fill = b % 4 == 3
        for _ in range(int(r.integers(2, 5))):
            pos = int(r.integers(0, 12 if fill else 16))
            kind = r.choice(['rep', 'r32', 'rev', 'pitch'])
            edits.append((pos, kind))
        if fill:
            for pos in range(12, 16):
                edits.append((pos, 'r64' if pos >= 14 else 'r32'))
        ed = dict(edits)
        for pos in range(16):
            t0 = b * bar + pos * S16
            k = seq[pos]
            e = ed.get(pos)
            if e == 'rep':
                k = seq[max(0, pos - 1)] if pos else 0
            if e in ('r32', 'r64'):
                part = S32 if e == 'r32' else S64
                src_k = 4 if fill else k                                     # rolls favour the snare slice
                sl = slices[src_k][:int(part * SR)]
                for j in range(int(round(S16 / part))):
                    pr = 1 + (0.06 * j if fill and pos >= 14 else 0)
                    out.add(t0 + j * part, fade(repitch(sl, pr)[:int(part * SR)], 0.5), 0.9)
                continue
            sl = slices[k]
            if e == 'rev':
                sl = sl[::-1]
            if e == 'pitch':
                sl = repitch(sl, float(r.choice([0.75, 1.5])))
            out.add(t0, fade(sl[:n16 + 200], 0.5), 1.0)
            if k in (0, 6, 10) and e not in ('rev',):
                kick_times.append(t0)
    drums = out.y
    # sidechain envelope from the kicks
    t = np.arange(len(drums)) / SR
    duck = np.ones_like(t)
    for kt in kick_times:
        i = int(kt * SR); j = min(len(t), i + int(0.25 * SR))
        duck[i:j] = np.minimum(duck[i:j], 1 - 0.65 * np.exp(-(t[i:j] - kt) / 0.08))
    # harmony: D, E, Bm, A (two bars each), twice
    prog = [(38, [62, 66, 69, 76]), (40, [64, 68, 71, 78]), (35, [59, 62, 66, 73]), (33, [57, 61, 64, 71])] * 2
    bass = Mix(L + 1.0); pad = Mix(L + 1.0); lead = Mix(L + 1.0)
    for c, (root, chord) in enumerate(prog):
        t0 = c * 2 * bar
        for bb in range(2):
            for s, ln in ((0, 3), (3, 3), (6, 2), (10, 3), (14, 2)):
                nt = root + (12 if (s == 14 and bb == 1) else 0)
                d = ln * S16
                bass.add(t0 + bb * bar + s * S16, sub(hz(nt), d + 0.02, d * 0.9, 2.4, a=0.004), 0.8)
        d = 2 * bar + 0.15
        voices = sum(osc(hz(n) * (1 + dt), d, 'saw', hmax=4000) for n in chord for dt in (-0.004, 0.004))
        pv = lpf(voices / 8, 1600) * env(d, 0.12, 9.0)
        pv[-int(0.15 * SR):] *= np.linspace(1, 0, int(0.15 * SR))
        pad.add(t0, pv, 1.0)
        if c % 2 == 1:                                                # crushed square motif every 4 bars
            for k, i in enumerate([4, 2, 4, 7, 6]):
                tt = t0 + bar + (8 + 2 * k) * S16 * 0.5 + 8 * S16
                ln = S16 * 0.9
                lead.add(tt, crush(osc(hz(deg(i) + 12), ln, 'square', hmax=6000) * env(ln, 0.002, 0.05), 6, 3), 0.5)
    def wrap(x):
        y = x[:N].copy(); tail = x[N:]
        y[:len(tail)] += tail[:N]
        return y
    drums, bassw, padw, leadw, duckw = wrap(drums), wrap(bass.y), wrap(pad.y), wrap(lead.y), duck[:N]
    # tape wobble with a whole number of cycles over the loop
    padw = wobble(np.concatenate([padw[-2000:], padw]), 2.0, 22 / L)[2000:]
    mix = sat(drums * 0.9, 1.6) + bassw * 0.55 * duckw + padw * 0.28 * (0.5 + 0.5 * duckw) + lpf(leadw, 6000) * 0.3
    mix = lpf(sat(mix, 1.3), 12000)
    return mix


# ---------------------------------------------------------------- write
def hf_share(x):
    f, p = signal.welch(x, SR, nperseg=1024)
    return float(np.sum(p[f > 6000]) / (np.sum(p) + 1e-20))


def main():
    os.makedirs(WAV, exist_ok=True)
    enc = shutil.which('ffmpeg')
    has_vorbis = enc and 'libvorbis' in subprocess.run([enc, '-hide_banner', '-encoders'], capture_output=True,
                                                         text=True).stdout
    if has_vorbis:
        os.makedirs(OGG, exist_ok=True)
    manifest = []
    for k, (id_, s) in enumerate(S.items()):
        x = np.asarray(s['fn'](), float)
        if s['loop']:                     # keep the loop's exact length (no tail trim) and its seam
            g = 10 ** ((s['target'] - lufs_m(x)) / 20); x = x * g
            pk = peak_db(x); limited = pk > -1
            if limited: x *= 10 ** ((-1 - pk) / 20)
        else:
            x, limited = finish(x, s['target'])
        path = os.path.join(WAV, id_ + '.wav')
        write_wav(path, x, 19000 + k)
        _, back = wavfile.read(path)
        xb = back.astype(float) / 32768
        e = {k2: v for k2, v in s.items() if k2 != 'fn'}
        e.update(id=id_, file=f'{PREFIX}{id_}.wav', ms=round(1000 * len(xb) / SR), lufs_m=round(lufs_m(xb), 1),
                 peak_dbfs=round(peak_db(xb), 1), limited=bool(limited), bytes=os.path.getsize(path),
                 hf6k=round(100 * hf_share(xb), 1))
        if s['loop']:
            e['seam_step'] = round(float(abs(xb[0] - xb[-1])), 4)
            e['typical_step'] = round(float(np.percentile(np.abs(np.diff(xb)), 99)), 4)
        if has_vorbis:
            op = os.path.join(OGG, id_ + '.ogg')
            subprocess.run([enc, '-v', 'error', '-y', '-i', path, '-c:a', 'libvorbis', '-q:a', OGG_Q,
                            '-map_metadata', '-1', '-fflags', '+bitexact', '-flags:a', '+bitexact', op], check=True)
            e['ogg_bytes'] = os.path.getsize(op)
        manifest.append(e)
        print(f"{id_:16s} {e['ms']:6d} ms {e['lufs_m']:6.1f} LUFS-M (t {s['target']}) pk {e['peak_dbfs']:6.1f} "
              f">6k {e['hf6k']:5.1f}% {e['bytes'] // 1024:5d} KiB{'  LIMITED' if limited else ''}"
              + (f"  seam {e['seam_step']} vs p99 {e['typical_step']}" if s['loop'] else ''))
    json.dump(dict(sample_rate=SR, key='D Lydian', bpm=BPM, tiers=TIER, sounds=manifest),
              open(MANIFEST, 'w'), indent=1)
    tot = sum(e['bytes'] for e in manifest)
    print(f"{len(manifest)} sounds, {tot / 1024:.0f} KiB WAV"
          + (f", {sum(e['ogg_bytes'] for e in manifest) / 1024:.0f} KiB OGG" if has_vorbis else ''))


if __name__ == '__main__':
    main()
