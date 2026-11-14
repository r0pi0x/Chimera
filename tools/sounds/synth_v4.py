#!/usr/bin/env python3
"""Ultimate Gun Game's "Signal" UI sound kit, v4: hi-fi starship interface (kept for A/B; v5 is synth.py).

v3 read as "quick, tinny, plucky, spiky". v4 is mellow and premium:
  - soft attacks (3-10 ms raised-cosine, no hard transient spike), filters
    that bloom upward instead of snapping shut (the "pluck"), low resonance,
    rounder lows, slightly longer gentle tails, a lower peak-to-loudness ratio;
  - one button family: hover, select, back, denied (and slider, votes, chips)
    are the same warm body (a detuned saw chord through a soft low-pass bloom,
    a round sine thump, a breath of air) varied in level, length, brightness
    and pitch direction;
  - mechanical moments are a latch (servo slide, clunk, pneumatic hiss, a warm
    tone settling); countdowns are heartbeats and soft swells rising in tension
    to a resolve; rank-ups rise (swell into the hit, rising arpeggio, filter
    opening upward, no falling tail);
  - big moments in stereo (pass, go, reveal, air strike, the sci-fi loop):
    equal-power panning that follows the slab, decorrelated reverbs (one
    impulse response per ear) with long pre-delays, mid-side width; kick and
    bass always mono and centred; everything checked summed to mono.
Bit-crush and chopped grit stay in the knife sting and the loops only.
Everything is synthesized here (oscillators, noise, envelopes); no samples.

  venv/bin/python synth_v4.py -> v4/wav/*.wav (48 kHz 16-bit; mono, or stereo where
                                noted), ogg/ (page copies), manifest.json

Key: D Lydian. Deterministic (fixed seeds, seeded dither). Licence: CC0 1.0.
"""
import json, os, shutil, subprocess
import numpy as np
from scipy import signal
from scipy.io import wavfile
from synth_v1 import SR, T, env, blip, rng, svf, sos, hpf, lpf, Mix, lufs_m, peak_db, hz, deg
from synth_v2 import sat, osc, sub, repitch, fade, svf_lp, S16, S32, S64, BPM, TIER
from synth_v3 import unison, bpnoise, swell, grains, chirp, reese, kick, lowsnare
import synth_v3

HERE = os.path.dirname(os.path.abspath(__file__))
WAV, OGG = os.path.join(HERE, 'v4', 'wav'), os.path.join(HERE, 'v4', 'ogg')
MANIFEST, PREFIX = os.path.join(HERE, 'v4', 'manifest.json'), 'v4/wav/'
OGG_Q = '1'


# ================================================================ v4 building blocks
def warm(notes, d, a=0.006, tau=0.12, f0=250.0, f1=1800.0, btau=0.05, q=0.7, voices=3, spread=0.004,
         glide=0.0, gtau=0.08, hold=0.0, drive=1.15, hmax=4500.0):
    """A warm chord: detuned saws through a soft (unresonant) low-pass that blooms UP from f0 to f1;
    glide: semitones the pitch moves over the sound (+ up, - down)."""
    t = T(d)
    gm = 2 ** (glide * (1 - np.exp(-t / gtau)) / 12)
    y = np.zeros(len(t))
    for n in notes:
        for v in range(voices):
            dt = spread * (v - (voices - 1) / 2) / max(1, (voices - 1) / 2)
            y += osc(hz(n) * (1 + dt) * gm, d, 'saw', hmax=hmax)
    y /= len(notes) * voices
    fc = f0 + (f1 - f0) * (1 - np.exp(-t / btau))
    y = svf_lp(y * 2.0, fc, q)
    return sat(y, drive) * env(d, a, tau, hold)


def thump(f=73.4, d=0.25, a=0.004, tau=0.07, drop=1.25, dtau=0.02):
    """A round sine thump: soft attack, a gentle pitch settle (no click)."""
    t = T(d)
    fr = f * (1 + (drop - 1) * np.exp(-t / dtau))
    return np.sin(2 * np.pi * np.cumsum(fr) / SR) * env(d, a, tau)


def breath(d, lo, hi, seed, a=0.008, tau=0.05, hold=0.0):
    y = signal.sosfilt(sos('bandpass', [lo, hi]), rng(seed).standard_normal(len(T(d))))
    return y / (np.max(np.abs(y)) + 1e-12) * env(d, a, tau, hold)


def softglide(f0, f1, d, a=0.02, tau=0.1, harm=0.25, hold=0.0):
    """A smooth tone gliding f0 -> f1 (a servo's hum without the buzz)."""
    t = T(d)
    f = f0 * (f1 / f0) ** (t / d)
    ph = 2 * np.pi * np.cumsum(f) / SR
    return (np.sin(ph) + harm * np.sin(2 * ph) + 0.4 * harm * np.sin(3 * ph)) * env(d, a, tau, hold)


def small_room(x, wet=0.10, size=0.22, pre=0.012, seed=801):
    return synth_v3.space(x, pre, size, wet, taps=((0.019, 0.10), (0.033, 0.06)), seed=seed, bright=3500)


# ---- stereo
def pan(x, p):
    """Equal-power pan of a mono signal; p in -1..1 (scalar or per-sample)."""
    th = (np.broadcast_to(np.asarray(p, float), x.shape) + 1) * np.pi / 4
    return np.stack([x * np.cos(th), x * np.sin(th)])


def st(x):
    return x if x.ndim == 2 else pan(x, 0.0)


class Mix2:
    def __init__(self, d):
        self.y = np.zeros((2, int(round(d * SR))))

    def add(self, t0, x, g=1.0, p=0.0):
        x = pan(x, p) if x.ndim == 1 else x
        i = int(round(t0 * SR))
        if i + x.shape[1] > self.y.shape[1]:
            self.y = np.concatenate([self.y, np.zeros((2, i + x.shape[1] - self.y.shape[1]))], axis=1)
        self.y[:, i:i + x.shape[1]] += g * x
        return self


def hall(x, size=1.8, pre=0.05, wet=0.25, seeds=(901, 902), bright=6000, lo=180):
    """Decorrelated stereo reverb: one noise impulse response per ear (mono or stereo in)."""
    xs = st(x)
    mid = (xs[0] + xs[1]) / np.sqrt(2)
    t = T(size)
    out = []
    k = int(pre * SR)
    for ch, sd in enumerate(seeds):
        ir = rng(sd).standard_normal(len(t)) * np.exp(-t / (size / 6.9)) * np.clip(t / 0.02, 0, 1)
        ir = lpf(hpf(ir, lo), bright)
        ir /= np.sqrt(np.sum(ir ** 2))
        out.append(signal.fftconvolve(mid, ir))
    n = xs.shape[1] + k + len(t)
    y = np.zeros((2, n)); y[:, :xs.shape[1]] = xs
    for ch in range(2):
        y[ch, k:k + len(out[ch])] += wet * out[ch][:n - k]
    return y


def widen(x2, w):
    m, s = (x2[0] + x2[1]) / 2, (x2[0] - x2[1]) / 2
    s = s * w
    return np.stack([m + s, m - s])


def chorus2(x, cents=5, seed=0):
    """A mono line as two slightly detuned, decorrelated ears (for pads and alarms)."""
    r = 2 ** (cents / 1200)
    a = np.interp(np.arange(len(x)) * r, np.arange(len(x)), x, right=0)
    b = np.interp(np.arange(len(x)) / r, np.arange(len(x)), x, right=0)
    return np.stack([0.7 * x + 0.5 * a, 0.7 * x + 0.5 * b])


# ================================================================ the event table
S = {}


def sound(id_, screen, tier, trigger, character, var=(0, 0), default=True, lufs=None, loop=False):
    def deco(fn):
        S[id_] = dict(fn=fn, screen=screen, tier=tier, target=lufs if lufs is not None else TIER[tier],
                      trigger=trigger, character=character, var_cents=var[0], var_db=var[1],
                      default_on=default, overlap='', loop=loop)
        return fn
    return deco


# ---- the button family: one warm body, four states
SELECT_CHORD = (50, 57, 62, 66)          # D3 A3 D4 F#4


def button(notes=SELECT_CHORD, d=0.32, a=0.007, tau=0.13, bright=1900, btau=0.05, glide=0.0, thump_g=0.55,
           thump_f=73.4, air_g=0.12, hold=0.015, room=0.10):
    m = Mix(d + 0.05)
    m.add(0, warm(notes, d, a, tau, 260, bright, btau, glide=glide, hold=hold), 0.8)
    m.add(0, thump(thump_f * 2 ** (min(glide, 0) / 24), d * 0.8, 0.005, tau * 0.55), thump_g)
    m.add(0, breath(0.12, 350, 2200, 811, 0.01, 0.035), air_g)
    return lpf(small_room(m.y, room), 5000)


@sound('ui_hover', 'menu', 'whisper', 'Pointer or focus lands on a menu entry, card or button',
       'The button family\'s quiet state: the select\'s warm body as a D3+A3 dyad, a 6 ms soft attack, a dim '
       'bloom (to 750 Hz), a soft thump, 90 ms', var=(25, 1.2))
def _():
    return button((50, 57), 0.1, 0.006, 0.04, 750, 0.03, thump_g=0.35, air_g=0.08, hold=0.0, room=0.06)


@sound('ui_select', 'menu', 'tap', 'An entry or button is pressed (Play, a tab, a map card)',
       'The family\'s full state, a warm confirm: a round thump (D2) under a D3 A3 D4 F#4 chord that swells in '
       'with an 8 ms attack while its soft low-pass blooms upward to 1.9 kHz; a breath of air, a small room; no '
       'pluck', var=(15, 0.8))
def _():
    return button(SELECT_CHORD, 0.34, 0.008, 0.14, 1900, 0.05)


@sound('ui_back', 'menu', 'tap', 'Back, close or cancel (Esc)',
       'The same body, gently descending and darker: the chord eases down two semitones, its bloom stops at '
       '1.1 kHz', var=(15, 0.8), lufs=-33)
def _():
    return button(SELECT_CHORD, 0.3, 0.008, 0.12, 1100, 0.04, glide=-2.0)


@sound('ui_denied', 'menu', 'tap', 'A locked or unavailable entry is pressed',
       'The same body muted: dull (bloom to 450 Hz), short, a soft double-tap with the second a semitone lower',
       var=(0, 0.5))
def _():
    m = Mix(0.3)
    m.add(0, button(SELECT_CHORD[:3], 0.13, 0.006, 0.045, 450, 0.03, thump_g=0.5, air_g=0.04, hold=0, room=0.05), 1.0)
    m.add(0.09, button(tuple(n - 1 for n in SELECT_CHORD[:3]), 0.13, 0.006, 0.04, 420, 0.03, thump_g=0.45,
                        air_g=0.03, hold=0, room=0.05), 0.75)
    return m.y


@sound('ui_slider', 'menu', 'whisper', 'A slider step; the engine sets the speed from the value, 0.5 to 2.0 '
       '(D3 to D5), no random pitch', 'A soft detent: a round D4 tone (sine with a little 2nd harmonic) with a '
       '3 ms attack and a smooth 60 ms fall, a tiny thump; no resonance', var=(0, 0.6))
def _():
    m = Mix(0.09)
    t = T(0.08)
    ph = 2 * np.pi * hz(62) * t
    m.add(0, (np.sin(ph) + 0.18 * np.sin(2 * ph)) * env(0.08, 0.003, 0.022), 0.8)
    m.add(0, thump(110, 0.05, 0.003, 0.012, 1.1), 0.25)
    return lpf(m.y, 3000)


@sound('menu_backdrop', 'menu', 'whisper', 'The menu backdrop swaps (every 16 s), optional; stereo',
       'A wide breath of air drifting left to right under a hall', default=False, lufs=-41)
def _():
    d = 1.1; t = T(d)
    y = svf_lp(rng(41).standard_normal(len(t)), 300 + 1100 * np.sin(np.pi * t / d) ** 2, 0.8) * np.sin(np.pi * t / d) ** 1.5
    return hall(pan(y, np.linspace(-0.6, 0.6, len(t))), 1.4, 0.05, 0.3, (903, 904))


# ---- the logo pass: soft, wide, panned with the slab
def hold(p):
    q = 2 * np.clip(p, 0, 1) - 1
    return 0.5 + 0.5 * np.sign(q) * np.abs(q) ** 3


@sound('pass_sweep', 'pass', 'event', 'The logo pass (1.2 s; covered 450-940 ms, swap at 600 ms); stereo',
       'A soft, spread whoosh that pans across with the slab (equal-power, from its on-screen position): a '
       'gentle band-pass on the slab\'s speed and a low-pass that blooms into the cover and again on the exit, '
       'a servo hum gliding with it; a sub riser into a round sub drop at 440 ms; the mark appears as a '
       'reversed-reverb rise into a warm wide glow (no transient); a breath of shimmer at the swap; soft air '
       'flicks for the stripes on the right; a hall tail')
def _():
    d = 1.6; t = T(d); p = t / 1.2
    x = -175 + 287 * hold(p)
    c = x + 70                                                  # slab centre, in screen widths x 100
    pn = np.clip((c - 50) / 85, -1, 1)
    v = (2 * np.clip(p, 0, 1) - 1) ** 2
    v = np.where(p > 1, np.exp(-(t - 1.2) / 0.08), v)
    r = rng(1401)
    a = svf(r.standard_normal(len(t)), 160 + 2300 * v ** 1.2, 0.9)
    a = a / np.max(np.abs(a)) * (0.05 + 0.95 * v ** 1.1)
    fc = np.interp(t, [0, 0.42, 0.48, 0.9, 1.18, 1.32, 1.6], [220, 2600, 320, 360, 3000, 1200, 200])
    b = svf_lp(r.standard_normal(len(t)), fc, 0.8)
    b = b / np.max(np.abs(b)) * np.interp(t, [0, 0.4, 0.47, 0.92, 1.15, 1.3, 1.6], [0.05, 0.7, 0.1, 0.1, 0.7, 0.25, 0])
    wh = (a + 0.8 * b) * np.clip(t / 0.12, 0, 1) ** 2                 # a slow, soft onset
    hum = softglide(70, 180, 0.44, 0.15, 0.1, 0.3, 0.2)
    m = Mix2(d)
    m.add(0, pan(wh, pn), 0.75)
    m.add(0.0, pan(hum, pn[:len(hum)]), 0.10)
    m.add(0.92, pan(softglide(180, 60, 0.36, 0.05, 0.12, 0.3, 0.1), pn[int(0.92 * SR):int(0.92 * SR) + len(T(0.36))]), 0.08)
    tr = T(0.36)
    ris = np.sin(2 * np.pi * np.cumsum(30 * (70 / 30) ** (tr / 0.36)) / SR) * (tr / 0.36) ** 2
    m.add(0.08, sat(ris, 1.6), 0.4)
    m.add(0.44, thump(55, 0.7, 0.005, 0.22, 1.6, 0.05), 0.9)           # the round drop
    m.add(0.44, sub(hz(26), 0.8, 0.3, 1.8, a=0.006, glide=(60, 0.06)), 0.55)
    m.add(0.44, lpf(rng(1404).standard_normal(int(0.5 * SR)), 220) * env(0.5, 0.01, 0.15), 0.4)
    # the mark: a reversed reverb of the glow, rising into the glow itself
    glow_notes = (50, 57, 64, 66)
    hit = warm(glow_notes, 0.25, 0.005, 0.08, 400, 2400, 0.02)
    rv = hall(hit, 1.0, 0.0, 1.0, (1411, 1412), 5000)[:, :int(0.42 * SR)]
    rv = rv[:, ::-1] * np.linspace(0, 1, rv.shape[1]) ** 1.5
    m.add(0.50 - rv.shape[1] / SR, rv, 0.35)
    gl = warm(glow_notes, 0.9, 0.04, 0.35, 300, 1500, 0.12, hold=0.08)
    m.add(0.50, widen(chorus2(gl, 6), 1.3), 0.42)
    fl = breath(0.12, 1500, 4500, 1415, 0.03, 0.03) * (0.6 + 0.4 * np.sin(2 * np.pi * 36 * T(0.12)))
    m.add(0.58, fl, 0.08, 0.1)                                          # the swap: a breath of shimmer
    for k, tt in enumerate([1.165, 1.205]):
        m.add(tt, breath(0.06, 900, 3500, 1416 + k, 0.008, 0.02), 0.12, 0.6 + 0.2 * k)
    return hall(m.y, 1.6, 0.06, 0.22, (1421, 1422), 5000)


# ---- lobby
def latch(t0, m, deep=1.0, seed=0):
    m.add(t0, thump(68 * deep, 0.25, 0.003, 0.07, 1.4, 0.015), 0.85)
    m.add(t0, lpf(breath(0.05, 150, 900, 1500 + seed, 0.002, 0.015), 900), 0.45)
    m.add(t0, softglide(150 * deep, 130 * deep, 0.06, 0.003, 0.02, 0.4), 0.3)
    m.add(t0 + 0.012, breath(0.3, 1200, 5000, 1510 + seed, 0.012, 0.11), 0.16)     # the "shhh"


@sound('lobby_ready', 'lobby', 'tap', 'You press F3 and become ready',
       'Locking in: a smooth servo slide (a hum gliding 110 to 240 Hz over a soft noise sweep), a round "clunk" '
       '(a low thump with a muffled knock), a pneumatic "shhh", then a warm D3 A3 D4 F#4 tone settling in with a '
       'gentle bloom', var=(0, 0.6), lufs=-30)
def _():
    m = Mix(0.9)
    m.add(0, softglide(110, 240, 0.17, 0.03, 0.08, 0.3, 0.1), 0.28)
    m.add(0, bpnoise(0.17, 400, 1500, 1501, 1.0) * env(0.17, 0.05, 0.06, 0.06), 0.12)
    latch(0.17, m, 1.0, 1)
    m.add(0.18, warm(SELECT_CHORD, 0.6, 0.025, 0.3, 300, 1500, 0.1, hold=0.04), 0.5)
    return lpf(small_room(m.y, 0.12, 0.3), 5000)


@sound('lobby_unready', 'lobby', 'tap', 'You press F3 again and are no longer ready',
       'The latch releasing: a soft unlatch and its hiss first, then the servo slides back down (240 to 100 Hz) '
       'while the warm tone fades under a closing filter', var=(0, 0.6), lufs=-31)
def _():
    m = Mix(0.8)
    m.add(0, thump(80, 0.15, 0.003, 0.04, 1.2), 0.5)
    m.add(0, lpf(breath(0.04, 150, 900, 1521, 0.002, 0.012), 900), 0.3)
    m.add(0.01, breath(0.22, 1200, 4500, 1522, 0.01, 0.08), 0.13)
    m.add(0.03, softglide(240, 100, 0.26, 0.02, 0.1, 0.3, 0.1), 0.26)
    t = T(0.45)
    w = warm(SELECT_CHORD, 0.45, 0.02, 0.16, 1400, 1400, 1.0)
    m.add(0.03, svf_lp(w, 300 + 1100 * np.exp(-t / 0.08), 0.7), 0.5)
    return lpf(small_room(m.y, 0.1, 0.25), 4500)


@sound('lobby_peer', 'lobby', 'soft', 'Another player readies up (or joins)',
       'The hover\'s body, lower and softer: an A2+E3 dyad, dim, 100 ms', var=(20, 1.2), lufs=-38)
def _():
    return button((45, 52), 0.11, 0.006, 0.045, 650, 0.03, thump_g=0.4, thump_f=55, air_g=0.06, hold=0, room=0.06)


def beat(m, t0, g=1.0, dub=False):
    """A heartbeat: a soft sub 'lub' (D2 with its octave), optionally the quieter 'dub'."""
    m.add(t0, thump(73.4, 0.3, 0.008, 0.075, 1.15, 0.03) + 0.25 * thump(146.8, 0.3, 0.008, 0.04, 1.1), g)
    if dub:
        m.add(t0 + 0.17, thump(73.4, 0.25, 0.008, 0.06, 1.1, 0.03), 0.6 * g)


def swell_tone(note, d=0.4, a=0.03, tau=0.12, index=0.5):
    return blip(hz(note), d, tau=tau, a=a, ratio=1.0, index=index, itau=tau * 2)


@sound('count_tick', 'lobby', 'tap', 'Each second of a countdown (the lobby start; the map vote\'s last 10 s)',
       'A heartbeat: a soft sub pulse on D2 under a quiet A3 FM swell (30 ms attack) and a low breath; nothing '
       'plucked', var=(5, 0.5), lufs=-34)
def _():
    m = Mix(0.45)
    beat(m, 0)
    m.add(0.005, swell_tone(57, 0.35, 0.03, 0.1, 0.4), 0.22)
    m.add(0, lpf(breath(0.25, 200, 900, 1601, 0.03, 0.07), 900), 0.1)
    return lpf(m.y, 3000)


def tension(note, seed, bright):
    m = Mix(0.6)
    beat(m, 0, 1.0, dub=True)
    m.add(0.005, swell_tone(note, 0.45, 0.03, 0.14, 0.6), 0.3)
    m.add(0.005, swell_tone(note - 12, 0.4, 0.03, 0.12, 0.3), 0.18)
    m.add(0, breath(0.3, 250, bright, seed, 0.04, 0.08), 0.12)
    return lpf(m.y, max(3000, bright + 500))


@sound('count_tick_3', 'lobby', 'cue', 'The countdown shows 3', 'The heartbeat doubles (lub-dub); the swell '
       'rises to A3 with its octave below, a brighter breath', var=(3, 0.4), lufs=-31)
def _():
    return tension(57, 1611, 1400)


@sound('count_tick_2', 'lobby', 'cue', 'The countdown shows 2', 'Lub-dub; the swell steps up to B3, brighter',
       var=(3, 0.4), lufs=-30)
def _():
    return tension(59, 1612, 1900)


@sound('count_tick_1', 'lobby', 'cue', 'The countdown shows 1',
       'Lub-dub; the swell on C#4, the leading tone that wants to resolve, and a reversed-reverb rise with a '
       'sub swell building through the second into the go', var=(3, 0.4), lufs=-29)
def _():
    m = Mix(1.0)
    m.add(0, tension(61, 1613, 2400), 1.0)
    rs = swell(0.6, 1614, 300, 3000, 0.15)
    t = T(0.6)
    m.add(0.38, svf_lp(rs, 300 + 2700 * (t / 0.6) ** 2, 0.7), 0.3)
    m.add(0.38, np.sin(2 * np.pi * hz(26) * t) * (t / 0.6) ** 2, 0.35)
    return lpf(m.y, 4000)


@sound('count_go', 'lobby', 'event', 'The countdown reaches 0 and the match starts; stereo',
       'The resolve: a wide warm D major chord (D2 A2 D3 F#3 A3 D4) swelling in over 20 ms and blooming open to '
       '3.5 kHz, a round sub drop, a soft noise bloom, the upper voices widened, a hall')
def _():
    m = Mix2(2.0)
    m.add(0, thump(60, 0.8, 0.006, 0.25, 1.5, 0.04), 0.9)
    m.add(0, sub(hz(26), 1.0, 0.35, 1.6, a=0.008), 0.45)
    m.add(0, warm((38, 45, 50), 1.4, 0.02, 0.5, 250, 1800, 0.12, hold=0.05), 0.5)
    hi = warm((54, 57, 62), 1.4, 0.025, 0.55, 300, 3500, 0.12, hold=0.05)
    m.add(0, widen(chorus2(hi, 7), 1.4), 0.45)
    m.add(0, breath(0.6, 300, 3000, 1621, 0.03, 0.15), 0.12)
    return hall(m.y, 1.8, 0.05, 0.22, (1622, 1623), 6000)


# ---- HUD
@sound('hud_chip', 'hud', 'soft', 'A HUD chip slides in (threat, objective, status)',
       'The hover\'s body on an F#3+C#4 dyad with a short rising breath', var=(20, 1.0), lufs=-35)
def _():
    m = Mix(0.15)
    m.add(0, breath(0.05, 400, 1500, 1701, 0.02, 0.015), 0.3)
    m.add(0.01, button((54, 61), 0.11, 0.006, 0.045, 900, 0.03, thump_g=0.25, air_g=0.04, hold=0, room=0.06), 1.0)
    return m.y


@sound('feed_self', 'hud', 'soft', 'Your own kill enters the kill feed (off where an announcer pack calls kills)',
       'A small select: the family body on A2+E3, short', var=(20, 1.0), default=False, lufs=-36)
def _():
    return button((45, 52, 57), 0.16, 0.007, 0.06, 1200, 0.04, thump_g=0.4, thump_f=55, hold=0, room=0.06)


@sound('tab_open', 'hud', 'whisper', 'The Tab board opens (0.12 s), optional', 'A soft breath opening upward',
       var=(20, 1.0), default=False)
def _():
    d = 0.12; t = T(d)
    return svf_lp(rng(1711).standard_normal(len(t)), 300 * 5 ** (t / d), 0.7) * env(d, 0.04, 0.03)


@sound('tab_close', 'hud', 'whisper', 'The Tab board closes (0.09 s), optional', 'The same, closing',
       var=(20, 1.0), default=False)
def _():
    d = 0.1; t = T(d)
    return svf_lp(rng(1712).standard_normal(len(t)), 1500 / 5 ** (t / d), 0.7) * env(d, 0.01, 0.025)


# ---- rank-up: it rises
def rank(n):
    u = (n - 2) / 11
    i = n - 2
    top = deg(round(i * 7 / 11))                 # D4 .. D5: the top of the arpeggio climbs with the rung
    m = Mix(1.1)
    t0 = 0.14                                    # the hit; the swell builds into it
    sw = swell(0.14, 1800 + n, 300, 2500 + 2500 * u, 0.04)
    tt = T(0.14)
    m.add(t0 - 0.14, svf_lp(sw, 300 + (2200 + 2000 * u) * (tt / 0.14) ** 2, 0.7), 0.3)
    m.add(0.0, softglide(hz(45), hz(50), 0.14, 0.04, 0.2, 0.2) * (tt / 0.14), 0.2)   # a pitch rise into it
    m.add(t0, thump(65, 0.5, 0.004, 0.13, 1.5, 0.03), 0.8)
    m.add(t0, sub(hz(26), 0.5, 0.16, 1.6, a=0.006), 0.35)
    peak = 1400 + 3000 * u
    for k, nt in enumerate((50, 57, top)):       # D3, A3, then the rung's note: always upward
        d = 0.75 - 0.05 * k
        m.add(t0 + 0.045 * k, warm((nt,), d, 0.006, 0.28, 400 + 300 * k, peak, 0.08 + 0.03 * k, hold=0.03), 0.42)
    m.add(t0 + 0.11, warm((top + 7,), 0.5, 0.03, 0.2, 600, peak, 0.1), 0.12 + 0.1 * u)   # a fifth above, rising in
    g = grains(0.5, 1820 + n, 30 + 120 * u, (top, top + 4, top + 7, top + 12), (0.008, 0.02), 3500 + 1500 * u,
               shape=lambda v: v ** 0.5)          # shimmer that thickens toward the end: upward, not falling
    m.add(t0 + 0.08, g * env(0.5, 0.15, 0.2), 0.08 + 0.1 * u)
    m.add(t0, breath(0.3, 500, 3000, 1830 + n, 0.01, 0.08), 0.1)
    return lpf(small_room(m.y, 0.14, 0.35), 5500)


for _n in range(2, 14):
    sound(f'rank_{_n:02d}', 'rank', 'event', f'You reach rung {_n} (a kill moves you up the ladder)',
          'A rising level-up: a reversed swell and a low pitch rise build into a round thump and sub (at 140 ms), '
          'then a D3, A3 and the rung\'s note step upward (D4 at rung 2 to D5 at 13), each blooming open; a fifth '
          'rises in above and a granular shimmer thickens toward the end, so the tail lifts instead of falling. '
          'Higher rungs: brighter (1.4 to 4.4 kHz) and more shimmer.', lufs=-27)((lambda n: lambda: rank(n))(_n))


@sound('rank_14_knife', 'rank', 'hero', 'You reach the knife rung (14, the last)',
       'v3\'s sting, unchanged (the grit lives here): an accelerating snare roll, a big distorted Lydian chord '
       'chopped into a buffer-repeat cascade, then the chord rings out over kick, sub drop and a ring-modulated '
       'blade')
def _():
    return synth_v3.S['rank_14_knife']['fn']()


@sound('rank_chain', 'rank', 'soft', 'A second rank-up lands while the toast still shows (under 1.15 s)',
       'Layered on the next rung: two quick soft warm tones stepping up (A4, D5)', lufs=-34)
def _():
    m = Mix(0.3)
    m.add(0, warm((69,), 0.15, 0.006, 0.05, 600, 2000, 0.03), 0.6)
    m.add(0.05, warm((74,), 0.2, 0.006, 0.07, 700, 2400, 0.03), 0.7)
    return lpf(m.y, 4500)


# ---- air strike: a ship's alarm
def rumble(d, seed, a=0.2, tau=0.5, hold=0.0):
    """A deep ship rumble in stereo: decorrelated low noise and a slow D1."""
    t = T(d)
    l = lpf(rng(seed).standard_normal(len(t)), 110, 4)
    r = lpf(rng(seed + 1).standard_normal(len(t)), 110, 4)
    l /= np.max(np.abs(l)); r /= np.max(np.abs(r))
    s = np.sin(2 * np.pi * hz(26) * t) * (0.8 + 0.2 * np.sin(2 * np.pi * 0.7 * t))
    e = env(d, a, tau, hold)
    return np.stack([(0.6 * l + 0.7 * s) * e, (0.6 * r + 0.7 * s) * e])


def alarm(note, d, a=0.04, tau=0.2, hold=0.0, f0=500, f1=2200, ftau=0.08, vib=5.0, seed=0):
    """A sleek alarm tone: a sine with low-index FM and gentle vibrato over a filtered saw an octave down,
    the filter moving, chorused to two ears."""
    t = T(d)
    f = hz(note) * (1 + 0.004 * np.sin(2 * np.pi * vib * t))
    ph = 2 * np.pi * np.cumsum(f) / SR
    tone = np.sin(ph + 0.6 * np.sin(ph))
    pad = osc(f / 2, d, 'saw', hmax=4000)
    fc = f0 + (f1 - f0) * (1 - np.exp(-t / ftau)) * np.exp(-np.maximum(t - 3 * ftau, 0) / (4 * ftau))
    body = svf_lp(pad, fc, 0.8)
    y = (0.7 * tone + 0.5 * body) * env(d, a, tau, hold)
    return chorus2(y, 6)


@sound('air_alert', 'air', 'event', 'The air-strike banner opens (0.25 s; stands in for the first pulse); stereo',
       'A ship\'s alarm: two sleek tones a tritone apart (G#4 then D4), each a vibrato sine with soft FM over a '
       'filtered saw whose filter moves, chorused wide, over a deep stereo rumble swelling in; a hall')
def _():
    m = Mix2(1.3)
    m.add(0, rumble(1.0, 1901, 0.15, 0.4, 0.2), 0.5)
    m.add(0.0, alarm(68, 0.24, 0.03, 0.1, 0.08, 600, 2400, 0.06), 0.45)
    m.add(0.2, alarm(62, 0.55, 0.03, 0.2, 0.12, 500, 2000, 0.08), 0.5)
    return lpf(hall(m.y, 1.4, 0.04, 0.2, (1902, 1903), 5000), 7000)


@sound('air_pulse', 'air', 'cue', 'Each countdown second of the strike, with the screen edge\'s pulse; stereo',
       'The alarm\'s D4 tone shaped like the edge glow (80 ms rise, 0.32 s fall), its filter opening with the '
       'glow, over a rumble pulse and a soft sub heartbeat', var=(0, 0.4), lufs=-28)
def _():
    m = Mix2(1.0)
    m.add(0, rumble(0.85, 1911, 0.08, 0.3), 0.55)
    m.add(0, alarm(62, 0.85, 0.08, 0.2, 0.0, 400, 2000, 0.1, 4.0), 0.5)
    m.add(0, thump(60, 0.4, 0.01, 0.1, 1.2), 0.5)
    return hall(m.y, 0.9, 0.03, 0.15, (1912, 1913), 4500)


@sound('air_impact', 'air', 'hero', 'The strike lands (UI layer only; the world\'s explosions are the game\'s); stereo',
       'A deep, round impact: a soft-attack sub drop, a wide decorrelated low noise burst, the rumble rolling '
       'on, a big hall; no crush', lufs=-24)
def _():
    m = Mix2(1.6)
    m.add(0, thump(55, 0.9, 0.004, 0.28, 1.8, 0.05), 1.0)
    m.add(0, sub(hz(26), 1.0, 0.35, 2.0, a=0.005, glide=(hz(38), 0.05)), 0.55)
    bl = lpf(rng(1921).standard_normal(int(0.5 * SR)), 1500) * env(0.5, 0.004, 0.08)
    br = lpf(rng(1922).standard_normal(int(0.5 * SR)), 1500) * env(0.5, 0.004, 0.08)
    m.add(0, np.stack([bl, br]) / np.max(np.abs(bl)), 0.35)
    m.add(0.05, rumble(1.3, 1923, 0.05, 0.5), 0.5)
    return hall(m.y, 2.0, 0.03, 0.25, (1924, 1925), 3500)


# ---- match end
@sound('end_reveal', 'end', 'hero', 'The winner is revealed on the match-end screen; stereo',
       'A reversed swell and a soft rising arpeggio (D3 A3 D4 E4 F#4 A4, each blooming) into a round sub hit '
       'and a wide warm Dmaj9 chord (D2 A2 E3 F#3 C#4) opening to 3 kHz, shimmer drifting wide, a long hall', lufs=-24)
def _():
    m = Mix2(2.6)
    for k, nt in enumerate((50, 57, 62, 64, 66, 69)):
        m.add(0.04 * k, warm((nt,), 0.5, 0.006, 0.15, 500, 2200, 0.04), 0.2, -0.5 + 0.2 * k)
    sw = swell(0.3, 2001, 300, 4000, 0.08)
    m.add(0.0, chorus2(sw, 10), 0.18)
    t0 = 0.3
    m.add(t0, thump(58, 0.9, 0.005, 0.28, 1.6, 0.04), 0.85)
    m.add(t0, sub(hz(26), 1.2, 0.4, 1.6, a=0.008), 0.45)
    m.add(t0, warm((38, 45), 1.8, 0.02, 0.7, 250, 1500, 0.15, hold=0.05), 0.45)
    hi = warm((52, 54, 61), 1.8, 0.025, 0.75, 300, 3000, 0.15, hold=0.05)
    m.add(t0, widen(chorus2(hi, 8), 1.5), 0.42)
    gl = grains(1.4, 2002, 60, (62, 66, 69, 73, 74, 76), (0.01, 0.03), 4000, shape=lambda u: 1 - u)
    gr = grains(1.4, 2003, 60, (62, 66, 69, 73, 74, 76), (0.01, 0.03), 4000, shape=lambda u: 1 - u)
    m.add(t0 + 0.05, np.stack([gl, gr]), 0.12)
    return hall(m.y, 2.2, 0.06, 0.25, (2004, 2005), 5500)


def vote(nm, k):
    return button((nm, nm + 7, nm + 12), 0.26, 0.007, 0.1, 1700, 0.045, thump_g=0.45, hold=0.01, room=0.08)


for _k, _nm in enumerate([50, 52, 54, 57]):
    sound(f'vote_cast_{_k + 1}', 'end', 'tap', f'You vote for map card {_k + 1} (or change your vote to it)',
          'The select\'s body on the card\'s own root (D3, E3, F#3, A3), slightly shorter', var=(0, 0.8),
          lufs=-31)((lambda nm, k: lambda: vote(nm, k))(_nm, _k))


@sound('vote_other', 'end', 'soft', 'Someone else votes (a pip lands on a card)',
       'The hover\'s body on A3+E4, softer', var=(20, 1.2), lufs=-39)
def _():
    return button((57, 64), 0.1, 0.006, 0.04, 700, 0.03, thump_g=0.25, air_g=0.05, hold=0, room=0.06)


@sound('vote_lock', 'end', 'event', 'The vote countdown hits 0: the vote locks (the pass then starts the next match)',
       'A deeper latch: a short servo slide, a heavy round clunk, the hiss, and a low D2+A2 tone settling')
def _():
    m = Mix(0.9)
    m.add(0, softglide(90, 170, 0.08, 0.02, 0.05, 0.3), 0.25)
    latch(0.08, m, 0.8, 7)
    m.add(0.085, thump(48, 0.5, 0.004, 0.14, 1.5, 0.03), 0.5)
    m.add(0.09, warm((38, 45, 50), 0.6, 0.02, 0.25, 250, 1100, 0.08, hold=0.03), 0.5)
    return lpf(small_room(m.y, 0.12, 0.3), 4500)


# ---- loops
@sound('menu_loop', 'music', 'music', 'The main menu (the breakcore sketch); loops seamlessly; mono',
       'v3\'s breakcore loop, unchanged: 16 bars at 174 BPM of our own chopped break, a driven sub line, a '
       'breathing pad, a reversed swell into every 4th downbeat', lufs=-20, loop=True)
def _():
    return synth_v3.S['menu_loop']['fn']()


@sound('menu_loop_scifi', 'music', 'music', 'The main menu (the sci-fi variation); loops seamlessly; stereo',
       'v3\'s sci-fi loop opened up in stereo: kick, snare, reese bass and sub mono in the centre; the granular '
       'pad as two independent streams (one per ear) widened mid-side; data chirps placed across the field; the '
       'servo sweeps auto-panned; a big decorrelated hall (3 s, 70 ms pre-delay) on the pad, chirps and snare, '
       'wrapped so it rings across the loop point; sidechain pump from the kick. Summed to mono it keeps its '
       'balance (pad and hall decorrelated, never phase-inverted).', lufs=-21, loop=True)
def _():
    bar = 16 * S16
    L = 16 * bar
    N = int(round(L * SR))
    Lx = L + 4.0
    drums = Mix(Lx); snares = Mix(Lx); kicks = []
    K = kick(0.45, 150, 40, 0.035, 0.2, 3.0, 0.35, 701)
    SN = lowsnare(0.3, 702, 165)
    r = rng(703)
    from synth_v2 import hat, crush
    for b in range(16):
        t0 = b * bar
        for s in (0, 10) if b % 2 == 0 else (0, 7, 10):
            drums.add(t0 + s * S16, K, 1.0); kicks.append(t0 + s * S16)
        snares.add(t0 + 8 * S16, SN, 0.8)
        for s in range(0, 16, 2):
            drums.add(t0 + s * S16, lpf(hat(0.05, 710 + s, 0.015), 7000), 0.07 if s % 4 else 0.1)
        if b % 4 == 3:
            sl = SN[:int(S64 * SR)]
            for j in range(8):
                snares.add(t0 + 12 * S16 + 4 * S16 * (1 - (1 - j / 8) ** 1.6),
                           fade(crush(repitch(sl, 1 + 0.04 * j), 10 - j // 2, 1 + j // 3), 0.4), 0.3 + 0.06 * j)
    t = np.arange(len(drums.y)) / SR
    duck = np.ones_like(t)
    for kt in kicks:
        i = int(kt * SR); j = min(len(t), i + int(0.35 * SR))
        duck[i:j] = np.minimum(duck[i:j], 1 - 0.75 * np.clip((t[i:j] - kt) / 0.004, 0, 1) * np.exp(-(t[i:j] - kt) / 0.12))
    prog = [(26, [62, 66, 69, 76]), (28, [64, 68, 71, 78]), (23, [59, 62, 66, 73]), (21, [57, 61, 64, 71])] * 2
    bass = Mix(Lx); pad = Mix2(Lx); fx = Mix2(Lx)
    for c, (root, chord) in enumerate(prog):
        t0 = c * 2 * bar; d = 2 * bar + 0.1
        rb = reese(root + 12, d, 0.008, 900, 2.0)
        tt = T(d)
        rb = svf_lp(rb, 250 + 700 * (0.5 - 0.5 * np.cos(2 * np.pi * tt / bar)), 1.3)
        rb *= np.clip(tt / 0.02, 0, 1) * np.clip((d - tt) / 0.08, 0, 1)
        bass.add(t0, rb, 0.9)
        notes = tuple(n - 12 for n in chord) + tuple(chord)
        gl = grains(d, 720 + c, 70, notes, (0.02, 0.06), 4200)
        gr = grains(d, 760 + c, 70, notes, (0.02, 0.06), 4200)
        e = np.clip(tt / 0.3, 0, 1) * np.clip((d - tt) / 0.1, 0, 1)
        pad.add(t0, widen(np.stack([gl * e, gr * e]), 1.6), 1.0)
        if c % 2 == 0:
            from synth_v3 import servo
            ln = bar * 0.5
            sv = servo(60, 220, ln, 730 + c) * np.hanning(len(T(ln)))
            fx.add(t0 + bar * 1.5, pan(sv, np.sin(np.linspace(-np.pi / 2, np.pi / 2, len(sv))) * (1 if c % 4 == 0 else -1) * 0.8), 0.12)
        for k in range(3):
            stp = [chord[int(r.integers(0, 4))] - 12 for _ in range(3)]
            fx.add(t0 + float(r.integers(1, 30)) * S16, chirp(stp, 0.02, 1.0, 1.1), 0.06, float(r.uniform(-0.8, 0.8)))

    def wrap(x):
        x = np.atleast_2d(x)
        y = x[:, :N].copy(); k = N
        while k < x.shape[1]:
            seg = x[:, k:k + N]; y[:, :seg.shape[1]] += seg; k += N
        return y
    dk = wrap(duck - 1)[0] + 1
    verb_in = pad.y + fx.y + st(snares.y) * 0.35
    big = hall(verb_in, 3.0, 0.07, 1.0, (741, 742), 5500)
    wet = big - np.pad(verb_in, ((0, 0), (0, big.shape[1] - verb_in.shape[1])))
    dw, sw, bw = wrap(drums.y)[0], wrap(snares.y)[0], wrap(bass.y)[0]
    pw, fw, ww = wrap(pad.y), wrap(fx.y), wrap(wet)
    centre = sat(dw * 0.85 + sw * 0.85, 1.5) + bw * 0.5 * dk
    sides = pw * 0.75 * (0.35 + 0.65 * dk) + fw * (0.5 + 0.5 * dk) + ww * 0.45 * (0.5 + 0.5 * dk)
    mix = st(centre) + sides                                        # centre: equal-power, mono-identical
    return lpf(sat(mix, 1.25), 10000)


# ================================================================ writing (mono or stereo)
KW1 = ([1.53512485958697, -2.69169618940638, 1.19839281085285], [1.0, -1.69065929318241, 0.73248077421585])
KW2 = ([1.0, -2.0, 1.0], [1.0, -1.99004745483398, 0.99007225036621])


def loud(x):
    """BS.1770 maximum momentary loudness, mono or stereo (channel powers summed)."""
    x = np.atleast_2d(x)
    w = int(0.4 * SR)
    tot = 0
    for ch in x:
        xp = np.concatenate([np.zeros(w), ch, np.zeros(w)])
        y = signal.lfilter(*KW2, signal.lfilter(*KW1, xp))
        cs = np.concatenate([[0], np.cumsum(y * y)])
        tot = tot + (cs[w:] - cs[:-w]) / w
    return -0.691 + 10 * np.log10(np.max(tot) + 1e-20)


def peak(x):
    return max(peak_db(ch) for ch in np.atleast_2d(x))


def finish2(x, target, loop=False, ceiling=-1.0):
    x = np.atleast_2d(np.asarray(x, float))
    if not loop:
        a = np.max(np.abs(x), axis=0); thr = a.max() * 10 ** (-60 / 20)
        last = np.nonzero(a > thr)[0][-1] + int(0.004 * SR)
        x = x[:, :min(x.shape[1], last)].copy()
        f = min(x.shape[1], int(0.006 * SR)); x[:, -f:] *= np.linspace(1, 0, f)
    x = x * 10 ** ((target - loud(x)) / 20)
    pk = peak(x)
    if pk > ceiling:
        x *= 10 ** ((ceiling - pk) / 20)
    return x, pk > ceiling


def hf_share(m):
    f, p = signal.welch(m, SR, nperseg=min(1024, len(m)))
    return float(np.sum(p[f > 6000]) / (np.sum(p) + 1e-20))


def main():
    os.makedirs(WAV, exist_ok=True); os.makedirs(OGG, exist_ok=True)
    enc = shutil.which('ffmpeg')
    man = []
    for k, (id_, s) in enumerate(S.items()):
        x, limited = finish2(s['fn'](), s['target'], s['loop'])
        path = os.path.join(WAV, id_ + '.wav')
        dd = rng(29000 + k)
        q = x * 32767 + (dd.random(x.shape) - dd.random(x.shape))
        q = np.clip(np.round(q), -32768, 32767).astype(np.int16)
        wavfile.write(path, SR, q[0] if q.shape[0] == 1 else q.T)
        _, back = wavfile.read(path)
        xb = np.atleast_2d((back.T if back.ndim == 2 else back).astype(float) / 32768)
        stereo = xb.shape[0] == 2
        mid = xb.mean(axis=0)
        e = {k2: v for k2, v in s.items() if k2 != 'fn'}
        e.update(id=id_, file=f'{PREFIX}{id_}.wav', channels=xb.shape[0], ms=round(1000 * xb.shape[1] / SR),
                 lufs_m=round(loud(xb), 1), peak_dbfs=round(peak(xb), 1), limited=bool(limited),
                 bytes=os.path.getsize(path), hf6k=round(100 * hf_share(mid), 1))
        e['plr'] = round(e['peak_dbfs'] - e['lufs_m'], 1)
        if stereo:
            e['corr'] = round(float(np.corrcoef(xb[0], xb[1])[0, 1]), 2)
            e['mono_fold_db'] = round(loud(mid) - (e['lufs_m'] - 3.01), 1)   # 0 = fully correlated, -3 = decorrelated
        if s['loop']:
            e['seam_step'] = round(float(np.max(np.abs(xb[:, 0] - xb[:, -1]))), 4)
        if enc:
            op = os.path.join(OGG, id_ + '.ogg')
            subprocess.run([enc, '-v', 'error', '-y', '-i', path, '-c:a', 'libvorbis', '-q:a', OGG_Q,
                            '-map_metadata', '-1', '-fflags', '+bitexact', '-flags:a', '+bitexact', op], check=True)
            e['ogg_bytes'] = os.path.getsize(op)
        man.append(e)
        print(f"{id_:16s} {'ST' if stereo else 'mo'} {e['ms']:6d} ms {e['lufs_m']:6.1f} LUFS-M (t {s['target']}) "
              f"pk {e['peak_dbfs']:6.1f} PLR {e['plr']:5.1f} >6k {e['hf6k']:4.1f}%"
              + (f" corr {e['corr']:5.2f} fold {e['mono_fold_db']:5.1f}" if stereo else '')
              + (f" seam {e['seam_step']}" if s['loop'] else '') + ('  LIMITED' if limited else ''))
    json.dump(dict(sample_rate=SR, key='D Lydian', bpm=BPM, tiers=TIER, sounds=man),
              open(MANIFEST, 'w'), indent=1)
    print(f"{len(man)} sounds, {sum(e['bytes'] for e in man) / 1024:.0f} KiB WAV, "
          f"{sum(e.get('ogg_bytes', 0) for e in man) / 1024:.0f} KiB OGG")


if __name__ == '__main__':
    main()
