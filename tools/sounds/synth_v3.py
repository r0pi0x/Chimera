#!/usr/bin/env python3
"""Ultimate Gun Game's "Signal" UI sound kit, v3: lower, layered, designed (kept for A/B; v4 is synth.py).

v2 read as "squeaky": single high tones. v3 moves the body down to roughly
80-800 Hz and builds every sound from layers:
  transient  a click, snap or crushed noise grain (short, quiet sparkle)
  body       detuned unison saws / pulses through a resonant low-pass with a
             fast filter envelope, low-ratio FM, or band-passed noise with a
             sweeping cutoff
  low        a kick, knock or driven sub (harmonics so laptops hear it)
  space      a short designed room: pre-delay, a tight two-tap delay and a
             small plate-like tail, rendered to mono
Pitch envelopes (quick drops), saturation and light bit-crush keep the
breakcore grit of v2 without the fizz. Every drum, swell and slice is ours:
oscillators and noise only, no samples, no recorded breaks.

  venv/bin/python synth_v3.py -> v3/wav/*.wav (48 kHz 16-bit mono), ogg/ (page
                                copies), manifest.json (the event table)

Key: D Lydian. Deterministic (fixed seeds, seeded dither). Licence: CC0 1.0.
"""
import json, os, shutil, subprocess
import numpy as np
from scipy import signal
from scipy.io import wavfile
from synth_v1 import (SR, T, env, blip, rng, svf, click, sos, hpf, lpf, room, Mix, lufs_m, peak_db, finish,
                      write_wav, hz, deg)
from synth_v2 import (crush, sat, wobble, ring, fade, repitch, stutter, osc, sub, kick, snare, hat, crash, stab,
                      svf_lp, tame, S16, S32, S64, BPM)
import synth_v2

HERE = os.path.dirname(os.path.abspath(__file__))
WAV, OGG = os.path.join(HERE, 'v3', 'wav'), os.path.join(HERE, 'v3', 'ogg')
OGG_Q = '1'   # page copies only


# ---------------------------------------------------------------- v3 layers
def unison(notes, d, shape='saw', voices=3, spread=0.007, hmax=6000.0, pw=0.5, pdrop=0.0, ptau=0.02):
    """Detuned unison of each note; pdrop: semitones the pitch starts above and falls from."""
    t = T(d)
    pe = 2 ** (pdrop * np.exp(-t / ptau) / 12)
    y = np.zeros(len(t))
    for n in notes:
        for v in range(voices):
            dt = spread * (v - (voices - 1) / 2) / max(1, (voices - 1) / 2)
            y += osc(hz(n) * (1 + dt) * pe, d, shape, pw=pw, hmax=hmax)
    return y / (len(notes) * voices) * 2.2


def filt(x, f0, f1, tau, q=1.2, f_end=None):
    """Resonant low-pass whose cutoff moves f0 -> f1 exponentially (time constant tau)."""
    t = np.arange(len(x)) / SR
    fc = f1 + (f0 - f1) * np.exp(-t / tau)
    return svf_lp(x, fc, q)


def bpnoise(d, f0, f1, seed, q=2.0, curve=1.0):
    t = T(d)
    fc = f0 * (f1 / f0) ** ((t / d) ** curve)
    y = svf(rng(seed).standard_normal(len(t)), fc, q)
    return y / (np.max(np.abs(y)) + 1e-12)


def knock(f0=240, f1=120, d=0.05, tau=0.012, drive=1.8):
    """The 'thock': a short low-mid tone with a fast pitch drop."""
    t = T(d)
    fr = f1 + (f0 - f1) * np.exp(-t / 0.006)
    return sat(np.sin(2 * np.pi * np.cumsum(fr) / SR), drive) * env(d, 0.0008, tau)


def snap(seed, d=0.006, lo=1200, hi=4500):
    y = rng(seed).standard_normal(int(d * SR))
    y = signal.sosfilt(sos('bandpass', [lo, hi]), y)
    return y / (np.max(np.abs(y)) + 1e-12) * env(d, 0.0003, d / 4)


def space(x, pre=0.008, size=0.18, wet=0.14, taps=((0.023, 0.22), (0.041, 0.14)), seed=301, bright=4500):
    """Designed space in mono: pre-delay, a tight two-tap delay, a small plate-like tail."""
    n = len(x)
    pad = int((pre + max(t for t, _ in taps) + size) * SR) + 10
    y = np.concatenate([x, np.zeros(pad)])
    dry = y.copy()
    for t, g in taps:
        k = int(t * SR); y[k:] += g * lpf(dry, bright)[:len(y) - k]
    tt = T(size)
    ir = rng(seed).standard_normal(len(tt)) * np.exp(-tt / (size / 6.9))
    ir = lpf(hpf(ir, 200), bright); ir /= np.sqrt(np.sum(ir ** 2))
    k = int(pre * SR)
    tail = signal.fftconvolve(dry, ir)[:len(y) - k]
    y[k:] += wet * tail
    return y


def flange(x, dmin=0.4, dmax=4.0, mod=None, g=0.7):
    """Phasing / flanging: x plus a copy delayed by a moving 0.4-4 ms (mod 0..1 per sample)."""
    n = np.arange(len(x))
    if mod is None:
        mod = 0.5 + 0.5 * np.sin(2 * np.pi * 0.7 * n / SR)
    d = (dmin + (dmax - dmin) * mod) * SR / 1000
    return (x + g * np.interp(n - d, n, x, left=0)) / (1 + g)


def swell(d, seed, lo=800, hi=7000, tau=None):
    """A reversed-cymbal style swell: a decaying noise wash, reversed (rises into its end)."""
    nz = rng(seed).standard_normal(int(d * SR))
    nz = signal.sosfilt(sos('bandpass', [lo, hi]), nz)
    nz = nz / (np.max(np.abs(nz)) + 1e-12) * env(d, 0.002, tau or d / 4)
    return nz[::-1]


def lowsnare(d=0.16, seed=2, tone=160):
    return lpf(snare(d, tone, seed, 0.05, 0.045, 0.8, 5000, 2.0), 6000)


# ---- the sci-fi layer
def grains(d, seed, density=120, notes=(62, 66, 69, 74, 76), glen=(0.006, 0.018), lp=5000, shape=None):
    """Granular shimmer: many tiny windowed FM grains on quantized pitches."""
    r = rng(seed); n = len(T(d))
    y = np.zeros(n + int(glen[1] * SR) + 10)
    for _ in range(int(density * d)):
        t0 = r.random() * d
        if shape is not None and r.random() > shape(t0 / d):
            continue
        g = glen[0] + r.random() * (glen[1] - glen[0])
        f = hz(float(r.choice(notes)))
        tt = T(g)
        gr = np.sin(2 * np.pi * f * tt + 0.8 * np.sin(2 * np.pi * f * 2 * tt)) * np.hanning(len(tt))
        i = int(t0 * SR); y[i:i + len(gr)] += gr * (0.5 + 0.5 * r.random())
    return lpf(y[:n], lp) / 3


def chirp(steps, dur=0.016, ratio=1.0, index=1.2):
    """A pitch-quantized 'computer' chirp: short FM steps on scale notes, kept low."""
    return np.concatenate([blip(hz(n), dur, tau=dur, a=0.001, ratio=ratio, index=index, itau=dur / 2) for n in steps])


def servo(f0, f1, d, seed=0, wobble_hz=38, q=4.0):
    """A servo / motor whir: a pulse gliding f0 -> f1, its filter tracking, with motor tremolo."""
    t = T(d)
    f = f0 * (f1 / f0) ** (t / d)
    y = osc(f, d, 'pulse', pw=0.3, hmax=3000) * (0.75 + 0.25 * np.sin(2 * np.pi * wobble_hz * t))
    y = svf(y, f * 3.0, q)
    return y / (np.max(np.abs(y)) + 1e-12)


def reese(note, d, detune=0.009, cut=600, drive=2.2):
    """Reese / neuro bass: two detuned saws beating, low-passed and driven."""
    y = osc(hz(note) * (1 - detune), d, 'saw', hmax=2500) + osc(hz(note) * (1 + detune), d, 'saw', hmax=2500)
    y = y + 0.6 * np.sin(2 * np.pi * hz(note) * T(d))
    return sat(lpf(y / 2, cut, 4), drive)


def bitflip(x, seed, amount=0.15, block=24):
    """Digital glitch: random short blocks sign-flipped, zeroed or held."""
    r = rng(seed); y = x.copy()
    for i in range(0, len(y) - block, block):
        c = r.random()
        if c < amount / 3: y[i:i + block] *= -1
        elif c < 2 * amount / 3: y[i:i + block] = 0
        elif c < amount: y[i:i + block] = y[i]
    return y


def air(x, wet=0.12, pre=0.045, size=0.7, seed=611):
    """Clean, airy space with a long pre-delay."""
    return space(x, pre, size, wet, taps=((0.09, 0.08),), seed=seed, bright=7000)


# ---------------------------------------------------------------- the event table
S = {}
TIER = synth_v2.TIER


def sound(id_, screen, tier, trigger, character, var=(0, 0), default=True, lufs=None, loop=False):
    def deco(fn):
        S[id_] = dict(fn=fn, screen=screen, tier=tier, target=lufs if lufs is not None else TIER[tier],
                      trigger=trigger, character=character, var_cents=var[0], var_db=var[1],
                      default_on=default, overlap='', loop=loop)
        return fn
    return deco


# ---- menu: dark, tactile switches
@sound('ui_hover', 'menu', 'whisper', 'Pointer or focus lands on a menu entry, card or button',
       'A soft "thock": a 210 to 130 Hz knock under a band-passed noise grain (700 to 400 Hz), a faint snap on '
       'top; 30 ms, low-passed at 4.5 kHz', var=(40, 1.5))
def _():
    m = Mix(0.035)
    m.add(0, knock(210, 130, 0.03, 0.007), 0.8)
    m.add(0, bpnoise(0.02, 700, 400, 11, 2.5) * env(0.02, 0.0005, 0.005), 0.5)
    m.add(0, snap(12, 0.003, 1500, 4000), 0.12)
    return lpf(crush(m.y, 11), 4500)


@sound('ui_select', 'menu', 'tap', 'An entry or button is pressed (Play, a tab, a map card)',
       'A deep switch: knock (160 to 90 Hz), a D3+A3 unison saw stab whose pitch drops two semitones as a '
       'resonant low-pass snaps shut (2.6 kHz to 280 Hz), a noise snap, a small room', var=(25, 1.0))
def _():
    m = Mix(0.14)
    m.add(0, knock(160, 90, 0.08, 0.025, 2.2), 0.8)
    body = filt(unison([50, 57], 0.12, pdrop=2, ptau=0.02), 2600, 280, 0.025, 1.6) * env(0.12, 0.001, 0.035)
    m.add(0.001, crush(sat(body, 1.8), 10), 0.7)
    m.add(0, snap(21, 0.005, 1500, 5000), 0.25)
    m.add(0.008, chirp([62, 57], 0.014, 1.0, 1.0), 0.10)                  # a low two-step data chirp
    return lpf(space(m.y, 0.006, 0.12, 0.10), 6000)


@sound('ui_back', 'menu', 'tap', 'Back, close or cancel (Esc)',
       'The select inverted: an A2+E3 stab whose pitch falls four semitones while its filter closes faster, '
       'a softer knock', var=(25, 1.0), lufs=-33)
def _():
    m = Mix(0.14)
    m.add(0, knock(140, 70, 0.07, 0.02, 2.0), 0.7)
    t = T(0.12)
    body = unison([45, 52], 0.12, pdrop=0) * 1.0
    f = hz(45) * 2 ** (-4 * np.clip(t / 0.08, 0, 1) / 12)
    body = osc(f, 0.12, 'saw') * 0.5 + osc(f * 1.4983, 0.12, 'saw') * 0.4
    m.add(0.001, filt(body, 1800, 200, 0.018, 1.5) * env(0.12, 0.001, 0.03), 0.8)
    m.add(0, snap(23, 0.004, 1200, 4000), 0.18)
    return lpf(space(m.y, 0.006, 0.1, 0.08), 5500)


@sound('ui_denied', 'menu', 'tap', 'A locked or unavailable entry is pressed',
       'Two dull gated buzzes: a D2+D#2 pulse cluster through a resonant 600 Hz low-pass, lightly crushed',
       var=(0, 0.5))
def _():
    m = Mix(0.18)
    for t0 in (0, 0.08):
        b = unison([38, 39], 0.065, 'pulse', voices=2, pw=0.3, hmax=3000)
        m.add(t0, crush(sat(filt(b, 1200, 500, 0.02, 2.0), 2.2), 8) * env(0.065, 0.002, 0.03, 0.015), 1.0)
    m.add(0, knock(110, 70, 0.05, 0.015), 0.4)
    return lpf(m.y, 2500)


@sound('ui_slider', 'menu', 'whisper', 'A slider step; the engine sets the speed from the value, 0.5 to 2.0 '
       '(D3 to D5), no random pitch', 'A pitched click: a knock and a D4 FM body (ratio 1, a short bell), 40 ms; '
       'its pitch is the value', var=(0, 0.8))
def _():
    m = Mix(0.045)
    m.add(0, blip(hz(62), 0.04, tau=0.012, ratio=1.0, index=1.2, itau=0.006), 0.8)
    m.add(0, knock(300, 200, 0.025, 0.006), 0.3)
    m.add(0, snap(31, 0.003, 1500, 4000), 0.1)
    return lpf(m.y, 5000)


@sound('menu_backdrop', 'menu', 'whisper', 'The menu backdrop swaps (every 16 s), optional',
       'A dark flanged wash: low-passed noise opening 250 Hz to 1.5 kHz and closing, the flanger sweeping',
       default=False, lufs=-41)
def _():
    d = 1.0; t = T(d)
    nz = rng(41).standard_normal(len(t))
    fc = 250 + 1250 * np.sin(np.pi * t / d) ** 2
    y = svf_lp(nz, fc, 1.2) * np.sin(np.pi * t / d) ** 1.5
    return flange(y, 0.5, 5.0, 0.5 + 0.5 * np.cos(2 * np.pi * t / d), 0.8)


# ---- the logo pass: a cinematic whoosh
def hold(p):
    q = 2 * np.clip(p, 0, 1) - 1
    return 0.5 + 0.5 * np.sign(q) * np.abs(q) ** 3


@sound('pass_sweep', 'pass', 'event', 'The logo pass (1.2 s; covered 450-940 ms, swap at 600 ms)',
       'Cinematic: two noise layers (a band-pass riding the slab\'s speed and a low-pass whose cutoff sweeps up '
       'into the cover, dives, and opens again on the exit) through a flanger that follows the slab; a reversed '
       'swell and a pitch-swept sub riser (30 to 70 Hz) into a deep sub-drop impact at 440 ms; a low rumble '
       'while covered with a low shimmer as the mark wipes in; a crushed buffer-repeat glitch on the 600 ms swap; '
       'a filtered tail out after the stripes')
def _():
    d = 1.55; t = T(d); p = t / 1.2
    v = (2 * np.clip(p, 0, 1) - 1) ** 2                      # slab speed
    v = np.where(p > 1, np.exp(-(t - 1.2) / 0.06), v)
    r = rng(401)
    # noise A: band-pass on the speed
    a = svf(r.standard_normal(len(t)), 180 + 3800 * v ** 1.2, 1.3)
    a = a / np.max(np.abs(a)) * (0.08 + 0.92 * v ** 0.8)
    # noise B: low-pass sweep up to the cover, dive, open on the exit, close in the tail
    fc = np.interp(t, [0, 0.42, 0.47, 0.9, 1.18, 1.3, 1.55], [250, 5200, 300, 380, 6500, 2200, 200])
    b = svf_lp(r.standard_normal(len(t)), fc, 1.4)
    b = b / np.max(np.abs(b)) * np.interp(t, [0, 0.4, 0.46, 0.92, 1.15, 1.25, 1.55], [0.1, 0.8, 0.12, 0.12, 0.8, 0.35, 0])
    mod = np.interp(t, [0, 0.44, 0.94, 1.2, 1.55], [1, 0, 0.2, 1, 0.6])     # the flanger follows the slab
    wh = flange(a + 0.8 * b, 0.4, 4.5, mod, 0.75) * np.clip(t / 0.03, 0, 1)
    m = Mix(d)
    m.add(0, sat(wh * 1.2, 1.5), 0.7)
    sw = swell(0.36, 402, 600, 6000, 0.08)
    m.add(0.44 - 0.36, sw, 0.35)                                            # reversed swell into the cover
    tr = T(0.36)                                                             # sub riser 30 -> 70 Hz
    fr = 30 * (70 / 30) ** (tr / 0.36)
    ris = sat(np.sin(2 * np.pi * np.cumsum(fr) / SR), 2.2) * (tr / 0.36) ** 2
    m.add(0.08, ris, 0.45)
    m.add(0.44, kick(0.6, 120, 32, 0.04, 0.22, 3.0, 0.4, 403), 0.95)        # the impact
    m.add(0.44, sub(hz(26), 0.75, 0.3, 2.6, glide=(80, 0.06)), 0.85)
    m.add(0.44, lpf(rng(404).standard_normal(int(0.4 * SR)), 280) * env(0.4, 0.002, 0.12), 0.6)
    m.add(0.46, lpf(rng(405).standard_normal(int(0.48 * SR)), 160) * env(0.48, 0.05, 0.25), 0.25)  # covered rumble
    sh = filt(unison([62, 69], 0.3, voices=3, spread=0.01), 300, 2200, 0.15, 1.3) * env(0.3, 0.06, 0.08)
    m.add(0.40, sh, 0.12)                                                    # the mark wipes in
    m.add(0.38, grains(0.56, 409, 160, (62, 66, 69, 74, 76, 81), (0.005, 0.016), 4500,
                       shape=lambda u: np.sin(np.pi * u) ** 0.7), 0.22)          # holographic shimmer while covered
    m.add(0.02, servo(55, 230, 0.42, 410) * np.linspace(0.2, 1, int(0.42 * SR)), 0.16)   # servo spins up with the slab
    m.add(0.92, servo(230, 50, 0.34, 411) * np.linspace(1, 0.1, int(0.34 * SR)), 0.14)   # and winds down on the exit
    m.add(0.44, reese(26, 0.8, 0.011, 700) * env(0.8, 0.004, 0.22), 0.4)   # reese / neuro weight under the drop
    pre = m.y[int(0.585 * SR):int(0.6 * SR)].copy() + 0.6 * crush(knock(300, 180, 0.015, 0.006), 6, 3)
    st = np.concatenate([fade(crush(pre, 7 - k, 2 + k), 0.4) * 0.9 ** k for k in range(4)])
    m.add(0.6, bitflip(st, 412, 0.35, 16), 0.9)                              # the swap glitch, bit-flipped
    m.add(0.6, snap(406, 0.004, 1200, 4000), 0.3)
    for k, tt in enumerate([1.165, 1.205]):                                  # the stripes
        m.add(tt, lowsnare(0.08, 407 + k, 180), 0.12 + 0.05 * k)
    return air(space(m.y, 0.012, 0.3, 0.1, seed=408), 0.1, 0.05, 0.8, 413)


# ---- lobby
@sound('lobby_ready', 'lobby', 'tap', 'You press F3 and become ready',
       'Lock-in: a knock and short sub, a D3 A3 D4 unison-saw chord whose resonant low-pass sweeps up '
       '(300 Hz to 3 kHz in 120 ms), a second latch click at 110 ms, a small room', var=(0, 0.8), lufs=-30)
def _():
    d = 0.42
    m = Mix(0.5)
    m.add(0, knock(150, 75, 0.1, 0.035, 2.4), 0.8)
    m.add(0, sub(hz(38), 0.2, 0.06, 2.2), 0.4)
    ch = unison([50, 57, 62], d, voices=3)
    t = T(d)
    fc = 300 * (3000 / 300) ** np.clip(t / 0.12, 0, 1) * np.exp(-np.maximum(t - 0.14, 0) / 0.25)
    m.add(0.005, sat(svf_lp(ch, np.maximum(fc, 250), 1.8), 1.6) * env(d, 0.004, 0.11, 0.06), 0.6)
    m.add(0, snap(61, 0.004, 1500, 4500), 0.2)
    m.add(0.11, knock(260, 160, 0.03, 0.008), 0.35)
    m.add(0.11, snap(62, 0.003, 2000, 5000), 0.15)
    m.add(0, servo(70, 200, 0.13, 64) * env(0.13, 0.02, 0.06), 0.14)      # a servo seats the latch
    m.add(0.12, grains(0.2, 65, 90, (62, 69, 74), (0.006, 0.014), 4000), 0.08)
    return lpf(space(m.y, 0.008, 0.16, 0.12), 6500)


@sound('lobby_unready', 'lobby', 'tap', 'You press F3 again and are no longer ready',
       'Release: the same chord with its low-pass closing (2.5 kHz to 220 Hz) and its pitch sagging a semitone; '
       'no latch, softer knock', var=(0, 0.8), lufs=-31)
def _():
    d = 0.4; t = T(d)
    m = Mix(0.5)
    m.add(0, knock(130, 70, 0.08, 0.025, 2.0), 0.6)
    sag = 2 ** (-1 * np.clip(t / 0.25, 0, 1) / 12)
    ch = sum(osc(hz(n) * (1 + dt) * sag, d, 'saw') for n in (50, 57, 62) for dt in (-0.006, 0, 0.006)) / 9 * 2.2
    m.add(0.003, sat(filt(ch, 2500, 220, 0.07, 1.6), 1.5) * env(d, 0.003, 0.12), 0.65)
    m.add(0.02, servo(200, 60, 0.2, 66) * env(0.2, 0.01, 0.08), 0.12)      # the servo lets go
    return lpf(space(m.y, 0.008, 0.14, 0.1), 5000)


@sound('lobby_peer', 'lobby', 'soft', 'Another player readies up (or joins)',
       'A soft low tick: a 180 Hz knock and a 600 Hz noise grain', var=(30, 1.5), lufs=-38)
def _():
    m = Mix(0.06)
    m.add(0, knock(190, 140, 0.05, 0.012), 0.8)
    m.add(0, bpnoise(0.02, 650, 500, 63, 3) * env(0.02, 0.0005, 0.005), 0.4)
    return lpf(space(m.y, 0.005, 0.08, 0.08), 4000)


def pulse_tick(seed, peak, notes=(38, 50), extra=None, d=0.22):
    """A deep tonal pulse: a D2+D3 unison saw through a low-pass that opens to `peak` and closes, a noise snap."""
    m = Mix(d + 0.05)
    body = unison(list(notes) + (extra or []), d, voices=2, spread=0.005)
    t = T(d)
    fc = 160 + (peak - 160) * np.exp(-t / 0.035) * np.clip(t / 0.004, 0, 1)
    m.add(0, sat(svf_lp(body, fc, 1.6), 1.8) * env(d, 0.002, 0.06), 0.8)
    m.add(0, knock(120, 60, 0.09, 0.03, 2.4), 0.6)
    m.add(0, snap(seed, 0.006, 1200, 4000), 0.25)
    return m.y


@sound('count_tick', 'lobby', 'tap', 'Each second of a countdown (the lobby start; the map vote\'s last 10 s)',
       'A deep tonal pulse: D2+D3 unison saws through a low-pass that blooms to 700 Hz and closes, a knock, a '
       'noise snap', var=(8, 0.7), lufs=-34)
def _():
    return lpf(space(pulse_tick(71, 700), 0.006, 0.1, 0.08), 5000)


@sound('count_tick_3', 'lobby', 'cue', 'The countdown shows 3', 'The pulse brighter (1 kHz) with A2 added',
       var=(5, 0.5), lufs=-31)
def _():
    m = Mix(0.3); m.add(0, pulse_tick(72, 1000, extra=[45])); m.add(0.01, chirp([62], 0.02), 0.08)
    return lpf(space(m.y, 0.006, 0.12, 0.09), 5500)


@sound('count_tick_2', 'lobby', 'cue', 'The countdown shows 2', 'Brighter again (1.5 kHz), the octave D4 added',
       var=(5, 0.5), lufs=-30)
def _():
    m = Mix(0.3); m.add(0, pulse_tick(73, 1500, extra=[45, 62])); m.add(0.01, chirp([62, 66], 0.018), 0.09)
    return lpf(space(m.y, 0.006, 0.12, 0.1), 6000)


@sound('count_tick_1', 'lobby', 'cue', 'The countdown shows 1',
       'The brightest pulse (2.2 kHz) under the only breakcore roll: eight low-tuned snare hits accelerating '
       '1/32 to 1/64 and crushing as they go, into the go', var=(5, 0.5), lufs=-29)
def _():
    m = Mix(0.6)
    m.add(0, pulse_tick(74, 2200, extra=[45, 62], d=0.3), 0.9)
    m.add(0.01, chirp([62, 66, 69], 0.016), 0.09)
    sn = lowsnare(0.12, 75, 170)
    for k in range(8):
        tt = 0.5 * (1 - (1 - k / 7) ** 1.5)
        m.add(tt, crush(repitch(sn, 1 + 0.035 * k), 10 - k // 2, 1 + k // 3), 0.18 + 0.07 * k)
    return lpf(m.y, 6500)


@sound('count_go', 'lobby', 'event', 'The countdown reaches 0 and the match starts',
       'Drop: kick and a driven D1 sub, a wide D chord (D2 A2 D3 F#3 A3) whose low-pass opens 300 Hz to 5 kHz, '
       'stuttered three 1/64 slices before it rings, a low crash, a room')
def _():
    d = 1.1; t = T(d)
    ch = unison([38, 45, 50, 54, 57], d, voices=3, spread=0.008)
    fc = 300 * (5000 / 300) ** np.clip(t / 0.08, 0, 1) * np.exp(-np.maximum(t - 0.1, 0) / 0.5)
    st = sat(svf_lp(ch, np.maximum(fc, 250), 1.4), 2.4) * env(d, 0.002, 0.32)
    m = Mix(1.4)
    m.add(0, stutter(st, 0, S64, 3, 0.85), 0.7)
    t0 = 3 * S64
    m.add(t0, st, 0.7)
    m.add(t0, kick(0.5, 150, 40, 0.03, 0.18, 3, 0.4, 81), 1.0)
    m.add(t0, sub(hz(26), 0.9, 0.3, 2.6, glide=(hz(38), 0.04)), 0.7)
    m.add(t0, lpf(crush(crash(0.9, 82, 0.3, 7000), 8, 2), 5000), 0.2)
    m.add(t0, lowsnare(0.2, 83, 160), 0.35)
    m.add(t0, reese(26, 0.9, 0.01, 600) * env(0.9, 0.004, 0.3), 0.35)
    m.add(t0, grains(0.6, 84, 90, (62, 66, 69, 74), (0.006, 0.016), 4500, shape=lambda u: 1 - u), 0.1)
    return lpf(space(m.y, 0.012, 0.35, 0.14), 8000)


# ---- HUD
@sound('hud_chip', 'hud', 'soft', 'A HUD chip slides in (threat, objective, status)',
       'A low pop: a knock and a noise band sweeping 400 Hz to 1.2 kHz', var=(25, 1.0), lufs=-35)
def _():
    m = Mix(0.08)
    m.add(0, bpnoise(0.05, 400, 1200, 91, 2.5) * env(0.05, 0.004, 0.012), 0.6)
    m.add(0.01, knock(230, 150, 0.04, 0.01), 0.6)
    m.add(0.012, chirp([66, 69], 0.012), 0.08)
    return lpf(space(m.y, 0.005, 0.08, 0.08), 5000)


@sound('feed_self', 'hud', 'soft', 'Your own kill enters the kill feed (off where an announcer pack calls kills)',
       'A short low stab: knock and an A2+E3 pulse through a closing filter', var=(25, 1.0), default=False,
       lufs=-36)
def _():
    m = Mix(0.1)
    m.add(0, knock(170, 100, 0.06, 0.018), 0.6)
    m.add(0, filt(unison([45, 52], 0.08, 'pulse', 2, pw=0.3), 1800, 250, 0.015, 1.5) * env(0.08, 0.001, 0.02), 0.6)
    m.add(0, snap(92, 0.003), 0.15)
    return lpf(m.y, 5000)


@sound('tab_open', 'hud', 'whisper', 'The Tab board opens (0.12 s), optional',
       'A low-passed noise swish opening 300 Hz to 1.8 kHz, flanged', var=(30, 1.0), default=False)
def _():
    d = 0.09; t = T(d)
    y = svf_lp(rng(93).standard_normal(len(t)), 300 * 6 ** (t / d), 1.3) * env(d, 0.03, 0.02)
    return flange(y, 0.5, 3, t / d)


@sound('tab_close', 'hud', 'whisper', 'The Tab board closes (0.09 s), optional', 'The same, closing',
       var=(30, 1.0), default=False)
def _():
    d = 0.07; t = T(d)
    y = svf_lp(rng(94).standard_normal(len(t)), 1800 / 6 ** (t / d), 1.3) * env(d, 0.004, 0.016)
    return flange(y, 0.5, 3, 1 - t / d)


# ---- rank-up: a weighty impact; the rung shows as brightness and a rising harmonic layer
def rank(n):
    u = (n - 2) / 11                       # 0 at rung 2 .. 1 at rung 13
    m = Mix(0.9)
    t0 = 0.1                               # the impact (the number rolls at 0.08-0.38 s)
    sw = swell(0.1, 500 + n, 500, 3000 + 3000 * u, 0.03)
    m.add(t0 - 0.1, sw, 0.3)                                     # reversed swell into it
    m.add(t0, kick(0.35, 140, 45, 0.025, 0.11, 2.8, 0.35, 510 + n), 0.9)
    m.add(t0, sub(hz(26), 0.4, 0.13, 2.4, glide=(hz(38), 0.03)), 0.55)
    d = 0.55; t = T(d)
    ch = unison([50, 57, 64], d, voices=3 + (n >= 8), spread=0.008)   # Dsus2 add9 (D3 A3 E4), the same each rung
    peak = 900 + 3600 * u
    fc = 220 + (peak - 220) * np.exp(-t / (0.05 + 0.05 * u)) * np.clip(t / 0.003, 0, 1)
    m.add(t0, crush(sat(svf_lp(ch, fc, 1.5 + u), 2.0 + u), 11) * env(d, 0.002, 0.16), 0.7)
    # the rising harmonic layer: a quiet low-ratio FM tone a scale step higher per rung (D4 .. A5), mid-register
    m.add(t0 + 0.02, chirp([deg(n - 4), deg(n - 3)], 0.016, 1.0, 1.0), 0.08)   # quantized data steps
    m.add(t0 + 0.052, blip(hz(deg(n - 2)), 0.3, tau=0.07, a=0.004, ratio=1.0, index=0.7 + 0.6 * u), 0.10 + 0.06 * u)
    m.add(t0, reese(38, 0.4, 0.008, 500 + 400 * u) * env(0.4, 0.003, 0.12), 0.22)
    m.add(t0 + 0.04, grains(0.35, 540 + n, 40 + 160 * u, (62, 66, 69, 74, 76), (0.005, 0.014), 3500 + 2000 * u,
                             shape=lambda v: 1 - v), 0.06 + 0.1 * u)
    m.add(t0, snap(520 + n, 0.005, 1500, 5000), 0.25)
    m.add(0.38, knock(250, 160, 0.03, 0.008), 0.25)               # the number lands
    return lpf(space(m.y, 0.01, 0.2, 0.12, seed=530), 7000)


for _n in range(2, 14):
    sound(f'rank_{_n:02d}', 'rank', 'event', f'You reach rung {_n} (a kill moves you up the ladder)',
          'Level-up impact: a reversed swell into a kick and D1 sub drop (at 100 ms), a D3 A3 E4 unison-saw stab '
          'whose resonant filter opens further each rung (900 Hz at rung 2 to 4.5 kHz at 13, more drive, a fourth '
          'voice from rung 8), and a quiet FM layer that climbs a scale step per rung (D4 to A5); the chord\'s '
          'pitch stays put', lufs=-27)((lambda n: lambda: rank(n))(_n))


@sound('rank_14_knife', 'rank', 'hero', 'You reach the knife rung (14, the last)',
       'v2\'s sting, a little lower: an accelerating ten-hit snare roll (tuned down), a big distorted Lydian chord '
       '(D2 A2 D3 A3 E4 G#4) on a kick, sub drop and crash, chopped into a buffer-repeat cascade (1/16, 1/32, '
       '1/64, crushing deeper), then the chord rings out with tape wobble and a lower ring-modulated "blade"')
def _():
    m = Mix(2.0)
    sn = snare(0.1, 170, 140, 0.035, 0.035, 0.6, 6000)
    for k in range(10):
        tt = 0.3 * (1 - (1 - k / 9) ** 1.7)
        m.add(tt, crush(repitch(sn, 1 + 0.04 * k), 9 - k // 3, 1 + k // 4), 0.2 + 0.06 * k)
    t0 = 0.33
    st = stab([38, 45, 50, 57, 64, 68], 1.3, tau=0.4, drive=5, cut0=7000, cut1=1300, ctau=0.25, bits=8, down=2)
    casc = np.concatenate([stutter(st, 0, S16, 2, 0.95), stutter(st, 0, S32, 2, 0.9, 1.0, 8),
                           stutter(st, 0, S64, 4, 0.9, 1.05, 7)])
    m.add(t0, casc, 0.8)
    t1 = t0 + len(casc) / SR
    for tt in (t0, t1):
        m.add(tt, kick(0.5, 160, 36, 0.03, 0.2, 3.5, 0.4, 141), 0.95)
    m.add(t1, wobble(st, 1.5, 5.5), 0.85)
    m.add(t1, sub(hz(26), 1.2, 0.4, 2.8, glide=(hz(38), 0.05)), 0.8)
    m.add(t1, lpf(crush(crash(1.2, 142, 0.45, 8000), 7, 2), 6000), 0.25)
    m.add(t1, ring(blip(1397.0, 0.5, tau=0.1, ratio=3.73, index=2.0), 311) * 0.8, 0.2)
    return lpf(room(m.y, 0.5, 0.15, 143), 9000)


@sound('rank_chain', 'rank', 'soft', 'A second rank-up lands while the toast still shows (under 1.15 s)',
       'Layered on the next rung: a low double-tap (two knocks a 1/64 apart) with a crushed grain', lufs=-34)
def _():
    m = Mix(0.1)
    for k in range(2):
        m.add(k * S64, knock(220 + 40 * k, 140, 0.04, 0.01, 2.0), 0.7)
        m.add(k * S64, crush(snap(150 + k, 0.004, 1200, 4000), 6, 2), 0.2)
    return lpf(m.y, 5000)


# ---- air strike
@sound('air_alert', 'air', 'event', 'The air-strike banner opens (0.25 s; stands in for the first pulse)',
       'A low siren stab falling a tritone (G#3 to D3) in unison saws, ring-modulated at 30 Hz under a closing '
       'filter, a gated noise riser, a snare, a sub', lufs=-26)
def _():
    m = Mix(0.65)
    g = (np.floor(T(0.25) / S64) % 2 == 0).astype(float)
    m.add(0, bpnoise(0.25, 400, 2200, 151, 2) * env(0.25, 0.08, 0.06) * g, 0.3)
    for t0, n, d in [(0.02, 56, 0.12), (0.13, 50, 0.42)]:
        b = unison([n, n - 12], d, voices=3)
        b = 0.6 * b + 0.5 * ring(b, 30)
        m.add(t0, crush(sat(filt(b, 3000, 500, 0.08, 1.6), 2.5), 9) * env(d, 0.003, d / 3.5), 0.75)
    m.add(0.13, lowsnare(0.2, 152, 160), 0.35)
    m.add(0.13, sub(hz(38), 0.3, 0.1), 0.45)
    return lpf(space(m.y, 0.01, 0.2, 0.1), 6000)


@sound('air_pulse', 'air', 'cue', 'Each countdown second of the strike, with the screen edge\'s pulse',
       'v2\'s driven D1 sub pulse and growling G#2 square, now through a low-pass that opens with the glow '
       '(80 ms rise, 0.32 s fall)', var=(0, 0.5), lufs=-28)
def _():
    d = 0.85; t = T(d)
    y = sub(hz(26), d, 0.2, 3.0, a=0.08) + 0.4 * sat(osc(hz(44), d, 'square', hmax=2000), 2) * env(d, 0.08, 0.2)
    fc = 300 + 1300 * env(d, 0.08, 0.2)
    return wobble(svf_lp(y, fc, 1.2), 0.8, 6)


@sound('air_impact', 'air', 'hero', 'The strike lands (UI layer only; the world\'s explosions are the game\'s)',
       'A heavy kick and sub drop, a crushed low noise burst that stutters out in three repeats, a room',
       lufs=-24)
def _():
    m = Mix(0.9)
    m.add(0, kick(0.7, 120, 28, 0.04, 0.25, 4, 0.5, 161), 1.0)
    m.add(0, sub(hz(26), 0.7, 0.22, 3, glide=(hz(38), 0.05)), 0.65)
    bz = crush(signal.sosfilt(sos('bandpass', [200, 2000]), rng(162).standard_normal(int(0.12 * SR))), 5, 6)
    bz = bz / np.max(np.abs(bz)) * env(0.12, 0.002, 0.05)
    m.add(0, bz, 0.4)
    m.add(0.12, stutter(np.pad(bz, (0, 4000)), 0, S32, 3, 0.6, 0.9), 0.3)
    return lpf(space(m.y, 0.015, 0.3, 0.1), 5000)


# ---- match end
@sound('end_reveal', 'end', 'hero', 'The winner is revealed on the match-end screen',
       'Stinger: a reversed swell and a filter-swept rising unison arpeggio (D2 to D4) into kick, sub, a low '
       'crash and a big driven Dmaj9(#11) stab (D2 A2 E3 F#3 C#4 G#4) chopped twice at 1/32, opening and '
       'ringing with tape wobble in a room', lufs=-24)
def _():
    m = Mix(2.0)
    for k, i in enumerate([0, 4, 7, 9, 11, 14]):
        nt = deg(i) - 24
        b = filt(unison([nt], 0.09, voices=3), 400 + 300 * k, 300, 0.04, 1.6) * env(0.09, 0.002, 0.04)
        m.add(0.036 * k, b, 0.45)
    m.add(0.26 - 0.3, np.zeros(1), 0)
    sw = swell(0.26, 171, 500, 6000, 0.06)
    m.add(0, sw, 0.3)
    t0 = 0.26
    d = 1.4; t = T(d)
    ch = unison([38, 45, 52, 54, 61, 68], d, voices=3, spread=0.008)
    fc = 400 * (6000 / 400) ** np.clip(t / 0.1, 0, 1) * np.exp(-np.maximum(t - 0.12, 0) / 0.6)
    st = sat(svf_lp(ch, np.maximum(fc, 300), 1.3), 2.6) * env(d, 0.002, 0.42)
    m.add(t0, stutter(st, 0, S32, 2, 0.9), 0.75)
    m.add(t0 + 2 * S32, wobble(st, 1.0, 5), 0.75)
    for tt in (t0, t0 + 2 * S32):
        m.add(tt, kick(0.45, 150, 40, 0.03, 0.16, 3, 0.4, 172), 0.85)
    m.add(t0 + 2 * S32, sub(hz(38), 1.0, 0.32, 2.5), 0.6)
    m.add(t0 + 2 * S32, lpf(crush(crash(1.2, 173, 0.45, 8000), 8, 2), 6000), 0.22)
    m.add(t0 + 2 * S32, lowsnare(0.2, 174, 160), 0.45)
    m.add(t0 + 2 * S32, reese(26, 1.1, 0.01, 650) * env(1.1, 0.004, 0.35), 0.35)
    m.add(t0 + 2 * S32, grains(1.0, 176, 120, (62, 66, 69, 74, 76, 78), (0.006, 0.02), 5000, shape=lambda u: 1 - u), 0.14)
    m.add(0.0, servo(60, 240, 0.26, 177) * np.linspace(0.1, 1, int(0.26 * SR)), 0.12)
    return lpf(space(m.y, 0.015, 0.45, 0.15, seed=175), 9000)


def vote(nm, k):
    m = Mix(0.16)
    m.add(0, knock(170, 95, 0.07, 0.022, 2.2), 0.7)
    m.add(0.001, blip(hz(nm), 0.12, tau=0.035, a=0.002, ratio=1.0, index=1.4, itau=0.012, glide=(hz(nm), 1)), 0.55)
    m.add(0.001, filt(unison([nm - 12], 0.1, 'pulse', 2, pw=0.3), 1800, 250, 0.02, 1.4) * env(0.1, 0.001, 0.03), 0.4)
    m.add(0, snap(185 + k, 0.004), 0.2)
    return lpf(space(m.y, 0.006, 0.1, 0.08), 5500)


for _k, _nm in enumerate([62, 64, 66, 69]):
    sound(f'vote_cast_{_k + 1}', 'end', 'tap', f'You vote for map card {_k + 1} (or change your vote to it)',
          'The select\'s knock with a low FM body and a pulse an octave under it, on the card\'s note '
          '(D4, E4, F#4, A4)', var=(0, 1.0), lufs=-31)((lambda nm, k: lambda: vote(nm, k))(_nm, _k))


@sound('vote_other', 'end', 'soft', 'Someone else votes (a pip lands on a card)',
       'A soft low tick on A3 (knock and a short FM body)', var=(30, 1.5), lufs=-39)
def _():
    m = Mix(0.07)
    m.add(0, knock(200, 140, 0.04, 0.01), 0.6)
    m.add(0, blip(hz(57), 0.05, tau=0.015, ratio=1.0, index=0.8), 0.4)
    return lpf(m.y, 4000)


@sound('vote_lock', 'end', 'event', 'The vote countdown hits 0: the vote locks (the pass then starts the next match)',
       'Gritty "ka-chunk": a low snare and a ring-modulated metal clank (D3), then a kick, a crushed D2 '
       'square stab and a driven sub, retriggered once at 1/64, a room')
def _():
    m = Mix(0.55)
    m.add(0, lowsnare(0.1, 191, 190), 0.5)
    m.add(0, crush(ring(osc(hz(50), 0.07, 'square', hmax=4000), 317) * env(0.07, 0.0005, 0.02), 7, 2), 0.45)
    h = Mix(0.4)
    h.add(0, kick(0.35, 170, 42, 0.02, 0.1, 3, 0.4, 192), 1.0)
    h.add(0, crush(sat(filt(osc(hz(38), 0.3, 'square', hmax=3000), 2000, 300, 0.03, 1.5), 3) * env(0.3, 0.002, 0.07), 8, 2), 0.5)
    h.add(0, sub(hz(38), 0.35, 0.1, 2.5), 0.5)
    m.add(0.075, bitflip(stutter(h.y, 0, S64, 1, 1.0), 193, 0.4, 12), 0.6)
    m.add(0.075 + S64, h.y, 0.9)
    return lpf(space(m.y, 0.01, 0.2, 0.1), 6500)


# ---- the menu loop sketch (v2's, darker and moving)
@sound('menu_loop', 'music', 'music', 'The main menu (a sketch of the soundtrack\'s direction); loops seamlessly',
       'v2\'s loop, kept and developed: 16 bars at 174 BPM (22.1 s) of our own break, chopped and re-sequenced per '
       'bar (retriggers, 1/32 and 1/64 rolls, reversed and repitched slices, a fill every 4 bars), the snare '
       'darker; a driven sub line; the detuned pad now through a low-pass that breathes over 8 bars; the motif an '
       'octave lower through a filter; a reversed swell into every 4th downbeat. Sidechained to the kick.',
       lufs=-20, loop=True)
def _():
    bar = 16 * S16
    L = 16 * bar
    N = int(round(L * SR))
    src = Mix(bar + 0.5)
    K = kick(0.35, 165, 44, 0.03, 0.15, 2.8, 0.4, 201)
    SN = lowsnare(0.22, 202, 175)
    GH = lowsnare(0.1, 203, 190)
    for s in (0, 6, 10):
        src.add(s * S16, K, 1.0)
    for s in (4, 12):
        src.add(s * S16, SN, 0.85)
    for s, g in ((7, 0.22), (9, 0.18), (14, 0.25), (15, 0.3)):
        src.add(s * S16, GH, g)
    for s in range(16):
        src.add(s * S16, lpf(hat(0.04 if s % 2 else 0.07, 210 + s, 0.012 if s % 2 else 0.03), 8000),
                0.1 if s % 2 else 0.16)
    brk = src.y
    n16 = int(round(S16 * SR))
    slices = [brk[k * n16:(k + 1) * n16 + 200] for k in range(16)]
    r = rng(220)
    out = Mix(L + 1.0)
    kick_times = []
    for b in range(16):
        fill = b % 4 == 3
        ed = {}
        for _ in range(int(r.integers(2, 5))):
            ed[int(r.integers(0, 12 if fill else 16))] = str(r.choice(['rep', 'r32', 'rev', 'pitch']))
        if fill:
            for pos in range(12, 16):
                ed[pos] = 'r64' if pos >= 14 else 'r32'
        for pos in range(16):
            t0 = b * bar + pos * S16
            k = pos
            e = ed.get(pos)
            if e == 'rep':
                k = max(0, pos - 1)
            if e in ('r32', 'r64'):
                part = S32 if e == 'r32' else S64
                sl = slices[4 if fill else k][:int(part * SR)]
                for j in range(int(round(S16 / part))):
                    pr = 1 + (0.05 * j if fill and pos >= 14 else 0)
                    out.add(t0 + j * part, fade(repitch(sl, pr)[:int(part * SR)], 0.5), 0.85)
                continue
            sl = slices[k]
            if e == 'rev':
                sl = sl[::-1]
            if e == 'pitch':
                sl = repitch(sl, float(r.choice([0.75, 1.5])))
            out.add(t0, fade(sl[:n16 + 200], 0.5), 1.0)
            if k in (0, 6, 10) and e != 'rev':
                kick_times.append(t0)
        if b % 4 == 3:                                        # a reversed swell into the next 4-bar downbeat
            sw = swell(2 * S16 * 4, 230 + b, 500, 5000, 0.12)
            out.add((b + 1) * bar - len(sw) / SR, sw, 0.12)
    drums = out.y
    t = np.arange(len(drums)) / SR
    duck = np.ones_like(t)
    for kt in kick_times:
        i = int(kt * SR); j = min(len(t), i + int(0.25 * SR))
        duck[i:j] = np.minimum(duck[i:j], 1 - 0.65 * np.clip((t[i:j] - kt) / 0.004, 0, 1) * np.exp(-(t[i:j] - kt) / 0.08))
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
        voices = sum(osc(hz(n - 12) * (1 + dt), d, 'saw', hmax=4000) for n in chord for dt in (-0.004, 0.004))
        pv = voices / 8 * env(d, 0.12, 9.0)
        pv[-int(0.15 * SR):] *= np.linspace(1, 0, int(0.15 * SR))
        pad.add(t0, pv, 1.0)
        if c % 2 == 1:
            for k, i in enumerate([4, 2, 4, 7, 6]):
                tt = t0 + bar + (8 + 2 * k) * S16 * 0.5 + 8 * S16
                ln = S16 * 0.95
                b = filt(unison([deg(i)], ln, 'pulse', 2, pw=0.3), 2500, 500, 0.04, 1.6) * env(ln, 0.002, 0.06)
                lead.add(tt, crush(b, 9, 2), 0.6)

    def wrap(x):
        y = x[:N].copy(); tail = x[N:]
        y[:len(tail)] += tail[:N]
        return y
    drums, bassw, padw, leadw, duckw = wrap(drums), wrap(bass.y), wrap(pad.y), wrap(lead.y), duck[:N]
    tt = np.arange(N) / SR
    fcp = 700 + 1300 * (0.5 - 0.5 * np.cos(2 * np.pi * tt / (L / 2)))   # breathes twice per loop
    ext = np.concatenate([padw[-SR:], padw])                              # warm the filter on the loop's end
    padw = svf_lp(ext, np.concatenate([fcp[-SR:], fcp]), 1.1)[SR:]
    padw = wobble(np.concatenate([padw[-2000:], padw]), 2.0, 22 / L)[2000:]
    mix = sat(drums * 0.9, 1.6) + bassw * 0.55 * duckw + padw * 0.34 * (0.5 + 0.5 * duckw) + leadw * 0.3
    return lpf(sat(mix, 1.3), 11000)


@sound('menu_loop_scifi', 'music', 'music', 'An alternative menu loop: the darker sci-fi variation; loops seamlessly',
       '16 bars at 174 BPM played half-time (22.1 s): a sparse kick and half-time snare that pump everything '
       'through a sidechain, a reese bass on D, E, B, A whose low-pass breathes, a granular shimmer pad on the '
       'chord tones in an airy pre-delayed space, servo sweeps every 4 bars, quiet data chirps, and the breakcore '
       'DNA in a chopped snare roll ending every 4th bar.', lufs=-21, loop=True)
def _():
    bar = 16 * S16
    L = 16 * bar
    N = int(round(L * SR))
    drums = Mix(L + 1.0); kicks = []
    K = kick(0.45, 150, 40, 0.035, 0.2, 3.0, 0.35, 701)
    SN = lowsnare(0.3, 702, 165)
    r = rng(703)
    for b in range(16):
        t0 = b * bar
        for s in (0, 10) if b % 2 == 0 else (0, 7, 10):
            drums.add(t0 + s * S16, K, 1.0); kicks.append(t0 + s * S16)
        drums.add(t0 + 8 * S16, SN, 0.8)
        for s in range(0, 16, 2):
            drums.add(t0 + s * S16, lpf(hat(0.05, 710 + s, 0.015), 7000), 0.08 if s % 4 else 0.12)
        if b % 4 == 3:                                     # the breakcore DNA: a chopped roll into the 4th downbeat
            sl = SN[:int(S64 * SR)]
            for j in range(8):
                drums.add(t0 + 12 * S16 + 4 * S16 * (1 - (1 - j / 8) ** 1.6),
                          fade(crush(repitch(sl, 1 + 0.04 * j), 10 - j // 2, 1 + j // 3), 0.4), 0.3 + 0.06 * j)
    t = np.arange(int((L + 1.0) * SR)) / SR
    duck = np.ones_like(t)
    for kt in kicks:
        i = int(kt * SR); j = min(len(t), i + int(0.35 * SR))
        duck[i:j] = np.minimum(duck[i:j], 1 - 0.75 * np.clip((t[i:j] - kt) / 0.004, 0, 1) * np.exp(-(t[i:j] - kt) / 0.12))
    prog = [(26, [62, 66, 69, 76]), (28, [64, 68, 71, 78]), (35 - 12, [59, 62, 66, 73]), (33 - 12, [57, 61, 64, 71])] * 2
    bass = Mix(L + 1.0); pad = Mix(L + 1.0); fx = Mix(L + 1.0)
    for c, (root, chord) in enumerate(prog):
        t0 = c * 2 * bar; d = 2 * bar + 0.1
        rb = reese(root + 12, d, 0.008, 900, 2.0)
        tt = T(d)
        rb = svf_lp(rb, 250 + 700 * (0.5 - 0.5 * np.cos(2 * np.pi * tt / bar)), 1.3)   # breathes once a bar
        rb *= np.clip(tt / 0.02, 0, 1) * np.clip((d - tt) / 0.08, 0, 1)
        bass.add(t0, rb, 0.9)
        g = grains(d, 720 + c, 70, tuple(n - 12 for n in chord) + tuple(chord), (0.02, 0.06), 4200)
        pad.add(t0, g * np.clip(tt / 0.3, 0, 1) * np.clip((d - tt) / 0.1, 0, 1), 1.0)
        if c % 2 == 0:
            fx.add(t0 + bar * 1.5, servo(60, 220, bar * 0.5, 730 + c) * np.hanning(int(bar * 0.5 * SR)), 0.12)
        for k in range(3):
            st = [chord[int(r.integers(0, 4))] - 12 for _ in range(3)]
            fx.add(t0 + float(r.integers(1, 30)) * S16, chirp(st, 0.02, 1.0, 1.1), 0.06)

    def wrap(x):
        y = x[:N].copy(); tail = x[N:]
        y[:len(tail)] += tail[:N]
        return y
    dw, bw, pw, fw = wrap(drums.y), wrap(bass.y), wrap(pad.y), wrap(fx.y)
    dk = wrap(duck - 1) + 1
    pw = wrap(air(pw, 0.35, 0.06, 1.2, 740)[:len(pad.y)])
    mix = sat(dw * 0.85, 1.5) + bw * 0.5 * dk + pw * 0.5 * (0.35 + 0.65 * dk) + fw * (0.5 + 0.5 * dk)
    return lpf(sat(mix, 1.25), 10000)


# ---------------------------------------------------------------- write
main = None
if __name__ == '__main__':
    synth_v2.S.clear(); synth_v2.S.update(S)
    synth_v2.WAV, synth_v2.OGG, synth_v2.OGG_Q, synth_v2.HERE = WAV, OGG, OGG_Q, HERE
    synth_v2.MANIFEST, synth_v2.PREFIX = os.path.join(HERE, 'v3', 'manifest.json'), 'v3/wav/'
    synth_v2.main()
