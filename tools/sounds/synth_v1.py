#!/usr/bin/env python3
"""Ultimate Gun Game's "Signal" UI sound kit, v1 "clean" (kept for A/B; v2 is synth.py).

Every sound is built here from oscillators, filtered noise and envelopes
(no samples, nothing traced from any game). Deterministic: fixed seeds and
seeded TPDF dither, so a rerun gives the same bytes.

  venv/bin/python synth_v1.py       -> v1/wav/*.wav (48 kHz, 16-bit, mono),
                                       ogg/*.ogg (if ffmpeg has libvorbis),
                                       manifest.json (the event table)

Palette: one key (D Lydian: D E F# G# A B C#), four primitives:
  blip   - a sine with a little FM, fast decay (tones, all in key)
  slant  - a short noise burst through a band-pass that glides
           (the 12-degree slant as a sound: tilted, quick)
  click  - a 3-4 ms high-passed noise transient (crispness)
  thump  - a sine dropping in pitch, a touch of drive (the sub)
Loudness: each sound is normalised to its tier's target, measured as the
maximum momentary loudness (ITU-R BS.1770 K-weighting, 400 ms window,
silence-padded so short sounds are judged as heard); sample peak (4x
oversampled) kept under -1 dBFS.

Licence: our own work, CC0 1.0.
"""
import json, os, shutil, subprocess
import numpy as np
from scipy import signal
from scipy.io import wavfile

SR = 48000
OGG_Q = '2'   # the page's copy only
HERE = os.path.dirname(os.path.abspath(__file__))
WAV = os.path.join(HERE, 'v1', 'wav')
OGG = os.path.join(HERE, 'v1', 'ogg')

# ---------------------------------------------------------------- the key
LYD = [0, 2, 4, 6, 7, 9, 11]          # D Lydian steps


def deg(i, base=62):
    """Scale degree i of D Lydian from D4 (MIDI 62)."""
    return base + 12 * (i // 7) + LYD[i % 7]


def hz(m):
    return 440.0 * 2 ** ((m - 69) / 12)


# ---------------------------------------------------------------- primitives
def T(d):
    return np.arange(int(round(d * SR))) / SR


def env(d, a=0.001, tau=0.05, hold=0.0):
    t = T(d)
    att = np.clip(t / max(a, 1e-5), 0, 1)
    att = 0.5 - 0.5 * np.cos(np.pi * att)
    return att * np.exp(-np.maximum(t - a - hold, 0) / tau)


def blip(f, d, tau=0.05, a=0.001, ratio=2.0, index=1.0, itau=None, hold=0.0, glide=None):
    """Sine with phase modulation; f in Hz; glide=(f_end, time constant)."""
    t = T(d)
    if glide:
        fe, gt = glide
        fr = fe + (f - fe) * np.exp(-t / gt)
    else:
        fr = np.full_like(t, f)
    ph = 2 * np.pi * np.cumsum(fr) / SR
    ie = index * np.exp(-t / (itau or tau * 0.6))
    return np.sin(ph + ie * np.sin(ratio * ph)) * env(d, a, tau, hold)


def rng(seed):
    return np.random.default_rng(seed)


def svf(x, fc, q):
    """TPT state-variable filter with a per-sample cutoff; returns band-pass."""
    fc = np.broadcast_to(np.asarray(fc, float), x.shape)
    g = np.tan(np.pi * np.clip(fc, 20, SR * 0.45) / SR)
    k = 1.0 / q
    out = np.empty_like(x)
    ic1 = ic2 = 0.0
    for n in range(len(x)):
        gn = g[n]
        a1 = 1.0 / (1.0 + gn * (gn + k)); a2 = gn * a1; a3 = gn * a2
        v3 = x[n] - ic2
        v1 = a1 * ic1 + a2 * v3
        v2 = ic2 + a2 * ic1 + a3 * v3
        ic1 = 2 * v1 - ic1; ic2 = 2 * v2 - ic2
        out[n] = v1
    return out


def slant(d, f0, f1, seed, q=2.0, a=0.002, tau=None, curve=1.0):
    """Noise burst whose band glides f0 -> f1 (exponentially)."""
    t = T(d)
    p = (t / d) ** curve
    fc = f0 * (f1 / f0) ** p
    y = svf(rng(seed).standard_normal(len(t)), fc, q)
    y /= np.max(np.abs(y)) + 1e-12
    return y * env(d, a, tau or d / 4)


def click(seed, d=0.004, hp=2500):
    y = rng(seed).standard_normal(int(d * SR)) * env(d, 0.0002, d / 5)
    return hpf(y, hp)


def thump(f0=95, f1=42, d=0.35, tau=0.11, drive=1.6):
    t = T(d)
    fr = f1 + (f0 - f1) * np.exp(-t / 0.035)
    ph = 2 * np.pi * np.cumsum(fr) / SR
    y = np.tanh(drive * np.sin(ph)) / np.tanh(drive)   # odd harmonics: audible on small speakers
    return y * env(d, 0.002, tau)


def sos(kind, f, order=2):
    return signal.butter(order, f, kind, fs=SR, output='sos')


def hpf(x, f, o=2): return signal.sosfilt(sos('highpass', f, o), x)
def lpf(x, f, o=2): return signal.sosfilt(sos('lowpass', f, o), x)


def room(x, size=0.3, wet=0.14, seed=7, bright=6000):
    """A small synthetic room: decaying noise as the impulse response."""
    t = T(size)
    ir = rng(seed).standard_normal(len(t)) * np.exp(-t / (size / 6.9))
    ir = lpf(ir, bright)
    ir /= np.sqrt(np.sum(ir ** 2))
    tail = signal.fftconvolve(x, ir)
    y = np.zeros(len(tail)); y[:len(x)] = x
    return y + wet * tail


class Mix:
    def __init__(self, d):
        self.y = np.zeros(int(round(d * SR)))

    def add(self, t0, x, g=1.0):
        i = int(round(t0 * SR))
        if i + len(x) > len(self.y):
            self.y = np.concatenate([self.y, np.zeros(i + len(x) - len(self.y))])
        self.y[i:i + len(x)] += g * x
        return self


# ---------------------------------------------------------------- loudness
KW1 = ([1.53512485958697, -2.69169618940638, 1.19839281085285], [1.0, -1.69065929318241, 0.73248077421585])
KW2 = ([1.0, -2.0, 1.0], [1.0, -1.99004745483398, 0.99007225036621])


def lufs_m(x):
    """Maximum momentary loudness (BS.1770 K-weighting, 400 ms windows)."""
    w = int(0.4 * SR)
    xp = np.concatenate([np.zeros(w), x, np.zeros(w)])
    y = signal.lfilter(*KW2, signal.lfilter(*KW1, xp))
    cs = np.concatenate([[0], np.cumsum(y * y)])
    ms = (cs[w:] - cs[:-w]) / w
    return -0.691 + 10 * np.log10(np.max(ms) + 1e-20)


def peak_db(x):
    return 20 * np.log10(np.max(np.abs(signal.resample_poly(x, 4, 1))) + 1e-20)


def finish(x, target, ceiling=-1.0):
    # trim the tail below -60 dB of the peak, then a 4 ms fade
    a = np.abs(x); thr = a.max() * 10 ** (-60 / 20)
    last = np.nonzero(a > thr)[0][-1] + int(0.004 * SR)
    x = x[:min(len(x), last)].copy()
    f = min(len(x), int(0.004 * SR))
    x[-f:] *= np.linspace(1, 0, f)
    g = 10 ** ((target - lufs_m(x)) / 20)
    x = x * g
    pk = peak_db(x)
    limited = pk > ceiling
    if limited:
        x *= 10 ** ((ceiling - pk) / 20)
    return x, limited


# ---------------------------------------------------------------- the sounds
S = {}           # id -> (array, meta)
TIER = {'whisper': -42, 'soft': -37, 'tap': -32, 'cue': -29, 'event': -26, 'hero': -22}


def sound(id_, screen, tier, trigger, character, var=(0, 0), default=True, overlap='', lufs=None):
    def deco(fn):
        S[id_] = dict(fn=fn, screen=screen, tier=tier, target=lufs if lufs is not None else TIER[tier],
                      trigger=trigger, character=character, var_cents=var[0], var_db=var[1],
                      default_on=default, overlap=overlap)
        return fn
    return deco


# ---- menu
@sound('ui_hover', 'menu', 'whisper', 'Pointer or focus lands on a menu entry, card or button',
       'Air tick: a 26 ms noise slant (4.5 to 7 kHz) under a faint D7 glint', var=(60, 1.5))
def _():
    m = Mix(0.03)
    m.add(0, slant(0.026, 4500, 7000, 11, q=3, tau=0.006), 1.0)
    m.add(0.001, blip(hz(98), 0.024, tau=0.005, ratio=1, index=0), 0.25)
    return m.y


@sound('ui_select', 'menu', 'tap', 'An entry or button is pressed (Play, a tab, a map card)',
       'Crisp: a click, a D6 FM blip (3:1, bright then mellow) and a short upward slant', var=(30, 1.0))
def _():
    m = Mix(0.09)
    m.add(0, click(21, 0.004, 3000), 0.9)
    m.add(0.002, blip(hz(86), 0.08, tau=0.022, ratio=3, index=1.6, itau=0.008), 0.8)
    m.add(0.0, slant(0.04, 2500, 6000, 22, q=2.5, tau=0.01), 0.35)
    return m.y


@sound('ui_back', 'menu', 'tap', 'Back, close or cancel (Esc)',
       'The select inverted: an A5 blip that slides down a step, a downward slant', var=(30, 1.0), lufs=-33)
def _():
    m = Mix(0.1)
    m.add(0, click(23, 0.003, 2500), 0.6)
    m.add(0.002, blip(hz(81), 0.09, tau=0.025, ratio=2, index=1.0, glide=(hz(79), 0.03)), 0.8)
    m.add(0.0, slant(0.05, 5000, 1800, 24, q=2.5, tau=0.012), 0.3)
    return m.y


@sound('ui_denied', 'menu', 'tap', 'A locked or unavailable entry is pressed (host-only option, full server)',
       'Two dull low pulses a semitone apart (C#4 then D4, filtered square), no click', var=(0, 0.5))
def _():
    m = Mix(0.17)
    for i, (n, t0) in enumerate([(61, 0), (62, 0.075)]):
        t = T(0.07)
        sq = np.tanh(4 * np.sin(2 * np.pi * hz(n) * t))
        m.add(t0, lpf(sq, 900) * env(0.07, 0.003, 0.02), 1.0)
    return m.y


@sound('ui_slider', 'menu', 'whisper', 'A slider moves one step (volume, sensitivity); also list scroll steps',
       'A 10 ms filtered tick, its pitch follows the value (the engine sets speed 0.8 to 1.25)', var=(40, 1.0))
def _():
    m = Mix(0.014)
    m.add(0, click(31, 0.003, 4000), 0.6)
    m.add(0, blip(hz(93), 0.012, tau=0.003, ratio=1, index=0), 0.5)
    return m.y


@sound('menu_backdrop', 'menu', 'whisper', 'The menu backdrop swaps (every 16 s)',
       'A slow, very quiet breath of air sliding up on the slant, 0.9 s; optional', default=False, lufs=-41)
def _():
    m = Mix(0.95)
    m.add(0, slant(0.9, 500, 3200, 41, q=1.4, a=0.35, tau=0.18, curve=0.8), 1.0)
    return lpf(m.y, 5000)


# ---- logo pass
def hold(p):
    q = 2 * np.clip(p, 0, 1) - 1
    return 0.5 + 0.5 * np.sign(q) * np.abs(q) ** 3


@sound('pass_sweep', 'pass', 'event', 'The logo pass (1.2 s; covered 450-940 ms, screen swap at 600 ms)',
       'A noise band riding the slab\'s speed (fast in, settles, fast out), a sub thump as the screen is covered, '
       'a glint as the mark wipes in, a low cut at the swap, two flicks as the trailing stripes leave')
def _():
    d = 1.32
    t = T(d)
    p = t / 1.2
    v = (2 * np.clip(p, 0, 1) - 1) ** 2                 # slab speed, 1 at the edges, 0 mid
    v = np.where(p > 1, np.exp(-(t - 1.2) / 0.04), v)
    fc = 280 + 5200 * v ** 0.9
    nz = svf(rng(51).standard_normal(len(t)), fc, 1.6)
    nz /= np.max(np.abs(nz))
    ampl = (0.12 + 0.88 * v ** 0.7) * np.clip(t / 0.05, 0, 1) ** 2
    m = Mix(d)
    m.add(0, nz * ampl, 0.9)
    m.add(0.44, thump(100, 40, 0.42, 0.12), 0.95)          # covered
    m.add(0.44, lpf(slant(0.12, 300, 120, 52, q=1.2, tau=0.04), 600), 0.4)
    gl = blip(hz(86), 0.22, tau=0.07, a=0.03, ratio=2, index=0.8, glide=(hz(93), 0.06))
    m.add(0.40, gl, 0.16)                                 # mark wipes in (400-590 ms)
    m.add(0.60, click(53, 0.003, 1500), 0.35)             # swap
    m.add(0.60, blip(hz(50), 0.08, tau=0.03, ratio=1, index=0.5), 0.35)
    m.add(0.84, blip(hz(93), 0.12, tau=0.04, a=0.01, ratio=2, index=0.6, glide=(hz(86), 0.05)), 0.10)
    m.add(1.165, slant(0.03, 3000, 8000, 54, q=3, tau=0.008), 0.25)  # trailing cobalt / white stripes
    m.add(1.205, slant(0.03, 3500, 9000, 55, q=3, tau=0.008), 0.2)
    return m.y


# ---- lobby
@sound('lobby_ready', 'lobby', 'tap', 'You press F3 and become ready', 'Rising two-note: A4 then D5, round FM (2:1)',
       var=(0, 0.8), lufs=-30)
def _():
    m = Mix(0.3)
    m.add(0, click(61, 0.003, 3000), 0.4)
    m.add(0.0, blip(hz(69), 0.12, tau=0.04, ratio=2, index=1.1), 0.7)
    m.add(0.075, blip(hz(74), 0.24, tau=0.07, ratio=2, index=1.3), 0.9)
    return m.y


@sound('lobby_unready', 'lobby', 'tap', 'You press F3 again and are no longer ready',
       'Falling two-note: D5 then A4, softer FM, the last note sags a little', var=(0, 0.8), lufs=-31)
def _():
    m = Mix(0.3)
    m.add(0, click(62, 0.003, 2500), 0.3)
    m.add(0.0, blip(hz(74), 0.1, tau=0.035, ratio=2, index=0.7), 0.7)
    m.add(0.075, blip(hz(69), 0.22, tau=0.06, ratio=2, index=0.5, glide=(hz(68.6), 0.1)), 0.9)
    return m.y


@sound('lobby_peer', 'lobby', 'soft', 'Another player readies up (or joins)', 'A single soft D6 pip, no click',
       var=(40, 1.5), lufs=-38)
def _():
    return blip(hz(86), 0.09, tau=0.025, a=0.002, ratio=2, index=0.4)


def tick(n, seed, body=0.0, d=0.03, tau=0.006, g=1.0):
    m = Mix(d)
    m.add(0, click(seed, 0.003, 3000), 0.5)
    m.add(0, blip(hz(n), d, tau=tau, ratio=3, index=0.6, itau=0.003), g)
    if body:
        m.add(0, blip(hz(n - 12), d, tau=tau * 2.2, ratio=2, index=0.8), body)
    return m.y


@sound('count_tick', 'lobby', 'tap', 'Each second of a countdown (the lobby start, the map vote: last 10 s)',
       'A dry 22 ms tick on D6 with a click on top', var=(10, 0.7), lufs=-34)
def _():
    return tick(86, 71, d=0.03)


@sound('count_tick_3', 'lobby', 'cue', 'The countdown shows 3', 'Tick on E6 with an octave body, longer',
       var=(5, 0.5), lufs=-31)
def _():
    return tick(88, 72, body=0.5, d=0.07, tau=0.012)


@sound('count_tick_2', 'lobby', 'cue', 'The countdown shows 2', 'Tick on F#6 with an octave body',
       var=(5, 0.5), lufs=-30)
def _():
    return tick(90, 73, body=0.6, d=0.075, tau=0.013)


@sound('count_tick_1', 'lobby', 'cue', 'The countdown shows 1', 'Tick on A6 with an octave body and a short slant',
       var=(5, 0.5), lufs=-29)
def _():
    m = Mix(0.09)
    m.add(0, tick(93, 74, body=0.7, d=0.085, tau=0.015))
    m.add(0, slant(0.05, 3000, 7000, 75, q=3, tau=0.012), 0.2)
    return m.y


@sound('count_go', 'lobby', 'event', 'The countdown reaches 0 and the match starts',
       'Bloom: an upward slant into a D major chord (D4 F#4 A4 D5) on a sub thump, D6 ping on top, small room')
def _():
    m = Mix(1.0)
    m.add(0, slant(0.08, 900, 6000, 81, q=2, a=0.04, tau=0.02, curve=1.5), 0.4)
    for i, n in enumerate([62, 66, 69, 74]):
        m.add(0.07 + 0.006 * i, blip(hz(n), 0.8, tau=0.2, a=0.004, ratio=2, index=1.8, itau=0.06), 0.42)
    m.add(0.07, thump(90, 45, 0.4, 0.1), 0.6)
    m.add(0.12, blip(hz(86), 0.5, tau=0.12, ratio=3, index=1.0), 0.22)
    return room(m.y, 0.35, 0.16, 82)


# ---- HUD
@sound('hud_chip', 'hud', 'soft', 'A HUD chip slides in (threat, objective, status)',
       'A 40 ms slant up with an F#6 pip', var=(30, 1.0), lufs=-35)
def _():
    m = Mix(0.07)
    m.add(0, slant(0.04, 1800, 5000, 91, q=2.5, tau=0.01), 0.6)
    m.add(0.008, blip(hz(90), 0.06, tau=0.016, ratio=2, index=0.5), 0.6)
    return m.y


@sound('feed_self', 'hud', 'soft', 'Your own kill enters the kill feed',
       'A tiny slant tick with an A6 glint; off where an announcer pack calls kills', var=(30, 1.0), default=False,
       lufs=-36)
def _():
    m = Mix(0.06)
    m.add(0, click(92, 0.003, 4000), 0.4)
    m.add(0.002, blip(hz(93), 0.05, tau=0.012, ratio=3, index=0.5), 0.6)
    return m.y


@sound('tab_open', 'hud', 'whisper', 'The Tab board opens (0.12 s)', 'A barely-there 60 ms air slant up; optional',
       var=(40, 1.0), default=False)
def _():
    return slant(0.07, 1200, 4500, 93, q=1.8, a=0.02, tau=0.018)


@sound('tab_close', 'hud', 'whisper', 'The Tab board closes (0.09 s)', 'The same, downward and shorter; optional',
       var=(40, 1.0), default=False)
def _():
    return slant(0.05, 4000, 1200, 94, q=1.8, a=0.004, tau=0.012)


# ---- rank-up (rungs 2..13 climb; 14 is the knife)
def rank(n):
    i = n - 2                                  # rung 2 -> D4 triad ... rung 13 -> A5 triad
    m = Mix(0.8)
    m.add(0, slant(0.05, 2000, 7000, 100 + n, q=2.5, tau=0.012), 0.25)      # the toast's clip wipe
    notes = [deg(i), deg(i + 2), deg(i + 4), deg(i + 7)]
    for k, nm in enumerate(notes):
        last = k == len(notes) - 1
        m.add(0.012 + 0.042 * k, blip(hz(nm), 0.6 if last else 0.18, tau=0.14 if last else 0.05, a=0.002,
                                       ratio=2, index=1.4 if last else 1.0, itau=0.03), 0.55 if last else 0.42)
    if n >= 10:                                 # the upper rungs: a fifth above the top shimmers in
        m.add(0.16, blip(hz(deg(i + 11)), 0.4, tau=0.1, a=0.01, ratio=2, index=0.5), 0.18)
    m.add(0.38, click(120 + n, 0.003, 3500), 0.18)                         # the number lands
    return room(m.y, 0.25, 0.12, 130)


for _n in range(2, 14):
    sound(f'rank_{_n:02d}', 'rank', 'event', f'You reach rung {_n} (a kill moves you up the ladder)',
          'Rising arpeggio in D Lydian (1-3-5-8 of the rung\'s degree), a soft wipe, a tick as the number lands; '
          'pitch climbs one scale step per rung' + (', a fifth shimmers above from rung 10' if _n >= 10 else ''),
          lufs=-27)((lambda n: lambda: rank(n))(_n))


@sound('rank_14_knife', 'rank', 'hero', 'You reach the knife rung (14, the last)',
       'Sting: a fast seven-note run up to C#6, then a wide Lydian chord (D3 bass, D5 A5 E6 G#6) on a sub, a '
       'metallic glint (inharmonic FM) and a bigger room')
def _():
    m = Mix(1.6)
    for k in range(7):
        m.add(0.026 * k, blip(hz(deg(7 + k)), 0.14, tau=0.04, ratio=2, index=1.0), 0.32)
    t0 = 0.2
    m.add(t0, thump(110, 38, 0.5, 0.16), 0.7)
    m.add(t0, blip(hz(50), 1.2, tau=0.35, a=0.005, ratio=1, index=0.3), 0.35)
    for k, nm in enumerate([74, 81, 88, 92]):
        m.add(t0 + 0.008 * k, blip(hz(nm), 1.3, tau=0.32, a=0.004, ratio=2, index=1.6, itau=0.08), 0.26)
    m.add(t0, blip(2794.0, 0.6, tau=0.12, a=0.001, ratio=3.73, index=2.5, itau=0.05), 0.14)   # blade glint
    m.add(t0 - 0.02, slant(0.5, 3000, 11000, 140, q=2, a=0.02, tau=0.12), 0.18)
    return room(m.y, 0.6, 0.2, 141)


@sound('rank_chain', 'rank', 'soft', 'A second rank-up lands while the toast still shows (chained, under 1.15 s)',
       'Layered on the next rung: a fast high double glint (E7, A7)', lufs=-34)
def _():
    m = Mix(0.14)
    m.add(0, blip(hz(100), 0.08, tau=0.02, ratio=2, index=0.6), 0.6)
    m.add(0.035, blip(hz(105), 0.1, tau=0.025, ratio=2, index=0.6), 0.6)
    return m.y


# ---- air strike
def threat_tone(d, notes=(50, 56), tau=0.3, a=0.08, cutoff=1100):
    t = T(d)
    y = sum(np.tanh(2.5 * np.sin(2 * np.pi * hz(n) * t * (1 + 0.0015 * k))) for k, n in enumerate(notes))
    return lpf(y / len(notes), cutoff, 2) * env(d, a, tau)


@sound('air_alert', 'air', 'event', 'The air-strike banner opens (0.25 s into the strike)',
       'Two stabs falling a tritone (G#4 to D4, driven and filtered), a slant opening on the banner', lufs=-26)
def _():
    m = Mix(0.6)
    m.add(0, slant(0.25, 600, 3500, 151, q=2, a=0.08, tau=0.06), 0.35)
    m.add(0.02, threat_tone(0.14, (68,), tau=0.04, a=0.004, cutoff=2400), 0.8)
    m.add(0.13, threat_tone(0.4, (62,), tau=0.1, a=0.004, cutoff=1800), 0.9)
    return m.y


@sound('air_pulse', 'air', 'cue', 'Each countdown second of the strike, with the screen edge\'s pulse',
       'A dark pulse (D3 and G#3 together), shaped like the edge glow: 80 ms rise, 0.32 s fall', var=(0, 0.5),
       lufs=-28)
def _():
    return threat_tone(0.85, (50, 56), tau=0.32 / 1.6, a=0.08, cutoff=1000)


@sound('air_impact', 'air', 'hero', 'The strike lands (UI layer only; the world\'s explosions are the game\'s)',
       'A sub drop and a short burst of static that matches the screen shake; no explosion of its own', lufs=-24)
def _():
    m = Mix(0.7)
    m.add(0, thump(80, 30, 0.6, 0.18, drive=2.2), 1.0)
    st = rng(161).standard_normal(int(0.45 * SR))
    st = np.round(st * 3) / 3                                     # coarse: a broadcast glitch
    st = signal.sosfilt(sos('bandpass', [400, 3000]), st) * env(0.45, 0.002, 0.08)
    m.add(0, st, 0.35)
    return m.y


# ---- match end
@sound('end_reveal', 'end', 'hero', 'The winner is revealed on the match-end screen',
       'A rising slant into a stamp: sub, a bright Dmaj9(#11) chord (D4 A4 E5 F#5 C#6), a G#6 spark, a room',
       lufs=-24)
def _():
    m = Mix(1.4)
    m.add(0, slant(0.22, 400, 5000, 171, q=1.8, a=0.15, tau=0.03, curve=1.6), 0.45)
    t0 = 0.2
    m.add(t0, click(172, 0.004, 2000), 0.5)
    m.add(t0, thump(95, 40, 0.5, 0.14), 0.7)
    for k, nm in enumerate([62, 69, 76, 78, 85]):
        m.add(t0 + 0.005 * k, blip(hz(nm), 1.1, tau=0.28, a=0.003, ratio=2, index=1.5, itau=0.07), 0.24)
    m.add(t0 + 0.12, blip(hz(92), 0.4, tau=0.09, ratio=3, index=0.8), 0.12)
    return room(m.y, 0.5, 0.18, 173)


for _k, _nm in enumerate([74, 76, 78, 81]):
    sound(f'vote_cast_{_k + 1}', 'end', 'tap', f'You vote for map card {_k + 1} (or change your vote to it)',
          f'The select click with its pip on the card\'s own note ({["D5", "E5", "F#5", "A5"][_k]}): '
          'four cards, four steps', var=(0, 1.0), lufs=-31)(
        (lambda nm, k: lambda: Mix(0.12).add(0, click(180 + k, 0.004, 3000), 0.8)
         .add(0.002, blip(hz(nm), 0.11, tau=0.03, ratio=3, index=1.4, itau=0.01), 0.8).y)(_nm, _k))


@sound('vote_other', 'end', 'soft', 'Someone else votes (a pip lands on a card)', 'A soft A5 pip', var=(40, 1.5),
       lufs=-39)
def _():
    return blip(hz(81), 0.07, tau=0.018, a=0.002, ratio=2, index=0.3)


@sound('vote_lock', 'end', 'event', 'The vote countdown hits 0: the vote locks (the pass then starts the next match)',
       'Two hits, "ka-chunk": a click and a D5 blip, then a low D3 FM hit on a thump', lufs=-26)
def _():
    m = Mix(0.45)
    m.add(0, click(191, 0.004, 2500), 0.7)
    m.add(0, blip(hz(74), 0.06, tau=0.015, ratio=3, index=1.2), 0.5)
    m.add(0.07, click(192, 0.005, 1200), 0.6)
    m.add(0.07, blip(hz(50), 0.3, tau=0.07, ratio=2, index=2.0, itau=0.03), 0.8)
    m.add(0.07, thump(90, 45, 0.3, 0.08), 0.6)
    return m.y


# ---------------------------------------------------------------- write
def write_wav(path, x, seed):
    d = rng(seed)
    q = x * 32767 + (d.random(len(x)) - d.random(len(x)))         # TPDF dither, 1 LSB
    wavfile.write(path, SR, np.clip(np.round(q), -32768, 32767).astype(np.int16))


def main():
    os.makedirs(WAV, exist_ok=True)
    enc = shutil.which('ffmpeg')
    has_vorbis = enc and 'libvorbis' in subprocess.run([enc, '-hide_banner', '-encoders'], capture_output=True,
                                                         text=True).stdout
    if has_vorbis:
        os.makedirs(OGG, exist_ok=True)
    manifest = []
    for k, (id_, s) in enumerate(S.items()):
        x, limited = finish(np.asarray(s['fn'](), float), s['target'])
        path = os.path.join(WAV, id_ + '.wav')
        write_wav(path, x, 9000 + k)
        _, back = wavfile.read(path)
        xb = back.astype(float) / 32768
        e = {k2: v for k2, v in s.items() if k2 != 'fn'}
        e.update(id=id_, file=f'v1/wav/{id_}.wav', ms=round(1000 * len(xb) / SR),
                 lufs_m=round(lufs_m(xb), 1), peak_dbfs=round(peak_db(xb), 1), limited=bool(limited),
                 bytes=os.path.getsize(path))
        if has_vorbis:
            op = os.path.join(OGG, id_ + '.ogg')
            subprocess.run([enc, '-v', 'error', '-y', '-i', path, '-c:a', 'libvorbis', '-q:a', OGG_Q,
                            '-map_metadata', '-1', '-fflags', '+bitexact', '-flags:a', '+bitexact', op], check=True)
            e['ogg_bytes'] = os.path.getsize(op)
        manifest.append(e)
        print(f"{id_:16s} {e['ms']:5d} ms  {e['lufs_m']:6.1f} LUFS-M (target {s['target']})  "
              f"peak {e['peak_dbfs']:6.1f} dBFS  {e['bytes'] // 1024:4d} KiB{'  LIMITED' if limited else ''}")
    json.dump(dict(sample_rate=SR, key='D Lydian', tiers=TIER, sounds=manifest),
              open(os.path.join(HERE, 'v1', 'manifest.json'), 'w'), indent=1)
    tot = sum(e['bytes'] for e in manifest)
    print(f"{len(manifest)} sounds, {tot / 1024:.0f} KiB WAV"
          + (f", {sum(e['ogg_bytes'] for e in manifest) / 1024:.0f} KiB OGG" if has_vorbis else ''))


if __name__ == '__main__':
    main()
