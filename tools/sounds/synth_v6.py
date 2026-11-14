#!/usr/bin/env python3
"""Ultimate Gun Game's "Signal" UI sound kit, v6: clean front, low body, procedural rank-up (kept for A/B; v7 is synth.py).

Changes from v5 (everything else is v5's, carried over):
  - a clean room: v5's plate convolved short cues with a decaying white-noise
    impulse response, and on a click that tail *is* filtered noise: the
    "rasp" behind select. The clean room is still one decorrelated IR per ear
    (mono-safe) but dense from the first sample (no early-reflection grain),
    frequency-dependent (highs die 3x faster than lows), low-passed near
    5 kHz, fed through a send filter (250 Hz - 4 kHz), shorter and drier.
    Hover and select are also rendered with v5's plate and nearly dry, for A/B;
  - clean excitation: menu resonators are rung by a raised-cosine pulse, not
    noise, so contacts are crisp without grit;
  - hover: a glassy digital touch, no thump, no noise;
  - select / back: two layers: a crisp glassy FM click in front (not high,
    ~0.7-1 kHz), a clean low body behind (thump + a smooth sine bloom), no
    broadband noise;
  - another player ready: a soft low-mid "someone's in" from the select family;
  - lobby ready: three polished variants (A precise, B magnetic, C heavy);
  - rank-up: procedural. rank_up(p, seed) renders the sound for progress
    p = rung / (ladder_len - 1) in [0, 1]; the kit ships a grid of p values for
    the page; an engine would render each match's ladder at match start.

  venv/bin/python synth_v6.py   -> v6/wav/*.wav, v6/ogg/, v6/manifest.json
Deterministic. Everything synthesized here. Licence: CC0 1.0.
"""
import os
import numpy as np
from scipy import signal
from synth_v1 import SR, T, env, blip, rng, svf, sos, hpf, lpf, Mix, hz
from synth_v2 import sat, osc, repitch, fade, svf_lp, S64, TIER
from synth_v3 import reese
from synth_v4 import thump, pan, Mix2, chorus2, widen
from synth_v5 import (modal, PLASTIC, METAL, air, smear, glint, formant, mbsat, comp, plate, soft_sub)
import synth_v4, synth_v5

HERE = os.path.dirname(os.path.abspath(__file__))


# ================================================================ v6 building blocks
def pulse_modal(f0, ratios, decays, gains, d=0.08, width=0.0006, soft=0.0):
    """Modal resonators rung by a raised-cosine pulse (clean, no noise grain)."""
    n = int(d * SR)
    x = np.zeros(n)
    w = max(4, int(width * SR))
    x[:w] = np.hanning(w)
    y = np.zeros(n)
    for r_, dc, g in zip(ratios, decays, gains):
        f = f0 * r_
        if f >= SR * 0.45:
            continue
        rr = np.exp(-1.0 / (dc * SR)); wv = 2 * np.pi * f / SR
        yy = signal.lfilter([1 - rr], [1, -2 * rr * np.cos(wv), rr * rr], x)
        y += g * yy / (np.max(np.abs(yy)) + 1e-12)
    if soft > 0:
        y *= 0.5 - 0.5 * np.cos(np.pi * np.clip(np.arange(n) / (soft * SR), 0, 1))
    return y / (np.max(np.abs(y)) + 1e-12)


GLASS = ((1.0, 2.76, 5.40), (0.012, 0.007, 0.004), (1.0, 0.35, 0.12))
LATCH = ((1.0, 2.76, 5.40, 8.93), (0.022, 0.013, 0.008, 0.005), (1.0, 0.5, 0.25, 0.12))


def fmclick(f0, f1, d=0.05, ratio=1.41, index=1.5, a=0.0015, tau=0.025, itau=0.006, lp=6000):
    """A glassy digital click: a short FM tone gliding f0 -> f1, its brightness falling fast."""
    t = T(d)
    f = f0 * (f1 / f0) ** np.clip(t / 0.02, 0, 1)
    ph = 2 * np.pi * np.cumsum(f) / SR
    y = np.sin(ph + index * np.exp(-t / itau) * np.sin(ratio * ph)) * env(d, a, tau)
    return lpf(y, lp)


def bloom(f=78.0, d=0.3, a=0.025, tau=0.12, harm=0.3, glide=1.0):
    """A clean low bloom: a sine and its octave swelling in smoothly (optionally gliding by `glide`)."""
    t = T(d)
    fr = f * glide ** (t / d)
    ph = 2 * np.pi * np.cumsum(fr) / SR
    return (np.sin(ph) + harm * np.sin(2 * ph)) * env(d, a, tau)


def clean_room(x, wet=0.09, size=0.24, pre=0.006, seeds=(6001, 6002), send=(250, 4000), lp=5200):
    """A smooth, dense, decorrelated room: per ear, a noise IR split in three bands that decay at
    different rates (lows 1x, mids 0.6x, highs 0.3x of `size`), dense from the first sample, low-passed;
    fed by a band-limited send so clicks don't excite hiss. Returns stereo."""
    xs = x if x.ndim == 2 else np.stack([x, x]) / np.sqrt(2)
    mid = (xs[0] + xs[1]) / np.sqrt(2)
    snd = lpf(hpf(mid, send[0]), send[1])
    t = T(size)
    k = int(pre * SR)
    n = xs.shape[1] + k + len(t)
    y = np.zeros((2, n)); y[:, :xs.shape[1]] = xs
    for ch, sd in enumerate(seeds):
        nz = rng(sd).standard_normal(len(t))
        lo = lpf(nz, 600, 2); md = signal.sosfilt(sos('bandpass', [600, 2500]), nz); hi = signal.sosfilt(sos('bandpass', [2500, lp]), nz)
        tau = size / 6.9
        ir = lo * np.exp(-t / tau) + 0.8 * md * np.exp(-t / (0.6 * tau)) + 0.4 * hi * np.exp(-t / (0.3 * tau))
        ir *= 0.5 - 0.5 * np.cos(np.pi * np.clip(t / 0.004, 0, 1))
        ir = lpf(ir, lp, 4)
        ir /= np.sqrt(np.sum(ir ** 2))
        tail = signal.fftconvolve(snd, ir)
        y[ch, k:k + len(tail)] += wet * tail[:n - k]
    return y


SPACES = {'': lambda x: clean_room(x, 0.09, 0.24), '_plate': lambda x: plate(x, 0.18, 0.35),
          '_dry': lambda x: clean_room(x, 0.025, 0.15)}
SPACE_NAMES = {'': 'clean room (v6)', '_plate': 'v5\'s plate', '_dry': 'nearly dry'}


# ================================================================ the event table
S = {}


def sound(id_, screen, tier, trigger, character, var=(0, 0), default=True, lufs=None, loop=False, **extra):
    def deco(fn):
        S[id_] = dict(fn=fn, screen=screen, tier=tier, target=lufs if lufs is not None else TIER[tier],
                      trigger=trigger, character=character, var_cents=var[0], var_db=var[1],
                      default_on=default, overlap='', loop=loop, **extra)
        return fn
    return deco


# ---- menu
def hover_dry():
    m = Mix(0.06)
    m.add(0, pulse_modal(2100, *GLASS, d=0.05, width=0.0004, soft=0.0008), 0.5)
    m.add(0, fmclick(1400, 1520, 0.04, 3.5, 1.2, 0.0012, 0.009, 0.003, 7000), 0.45)
    return hpf(lpf(m.y, 7500), 400)


def select_dry():
    m = Mix(0.4)
    # front: a crisp glassy click and a short upward digital FM blip (~0.7-1 kHz)
    m.add(0, pulse_modal(700, *GLASS, d=0.07, width=0.0006, soft=0.0012), 0.45)
    m.add(0, fmclick(880, 960, 0.06, 1.41, 1.6, 0.0015, 0.03, 0.006, 6000), 0.5)
    # behind: a clean low body: a thump and a smooth sine bloom, a little sub
    m.add(0, thump(78, 0.3, 0.005, 0.07, 1.25, 0.015), 0.6)
    m.add(0.008, bloom(78, 0.32, 0.025, 0.12, 0.3), 0.45)
    m.add(0.005, bloom(45, 0.25, 0.02, 0.08, 0.0), 0.15)
    return comp(mbsat(m.y, (1.3, 1.15, 1.0)), -16, 2.0)


def back_dry():
    m = Mix(0.35)
    m.add(0, pulse_modal(560, *GLASS, d=0.06, width=0.0007, soft=0.0015), 0.35)
    m.add(0, fmclick(900, 760, 0.06, 1.41, 1.1, 0.002, 0.026, 0.006, 5000), 0.45)
    m.add(0, thump(70, 0.22, 0.005, 0.05, 1.15, 0.015), 0.45)
    m.add(0.006, bloom(76, 0.22, 0.02, 0.07, 0.25, 0.8), 0.4)          # the body withdraws (falls ~4 semitones)
    return comp(mbsat(m.y, (1.25, 1.1, 1.0)), -16, 2.0)


for suf in SPACES:
    alt = suf != ''
    sound('ui_hover' + suf, 'menu', 'whisper',
          'Pointer or focus lands on a menu entry, card or button' + (f' (A/B: {SPACE_NAMES[suf]})' if alt else ''),
          'A clean, crisp, glassy digital touch: a raised-cosine pulse ringing three glass-like modes at 2.1 kHz '
          '(12 ms) and a tiny FM tick gliding up (1.4 to 1.5 kHz); no thump, no noise; ' + SPACE_NAMES[suf],
          var=(40, 1.2), default=not alt)((lambda sf: lambda: SPACES[sf](hover_dry()))(suf))
    sound('ui_select' + suf, 'menu', 'tap',
          'An entry or button is pressed (Play, a tab, a map card)' + (f' (A/B: {SPACE_NAMES[suf]})' if alt else ''),
          'Two clean layers. Front: a crisp glassy click (pulse-rung modes at 700 Hz) and a short FM blip '
          'stepping up 880 to 960 Hz. Behind: a clean low body (a 78 Hz thump, a smooth sine bloom with its '
          'octave, a little sub). No broadband noise; gentle multiband density; ' + SPACE_NAMES[suf],
          var=(20, 0.8), default=not alt)((lambda sf: lambda: SPACES[sf](select_dry()))(suf))


@sound('ui_back', 'menu', 'tap', 'Back, close or cancel (Esc)',
       'The select\'s two layers withdrawing: a softer glassy click at 560 Hz and an FM blip falling 900 to '
       '760 Hz; a smaller thump and a bloom that sinks; clean room', var=(20, 0.8), lufs=-33)
def _():
    return SPACES[''](back_dry())


@sound('lobby_peer', 'lobby', 'soft', 'Another player readies up (or joins)',
       'A soft low-mid "someone\'s in", from the select family: a gentle glassy click at 520 Hz, an FM blip '
       'lifting a little (600 to 660 Hz), a quiet low bloom; clean room', var=(25, 1.0), lufs=-38)
def _():
    m = Mix(0.3)
    m.add(0, pulse_modal(520, *GLASS, d=0.06, width=0.0008, soft=0.002), 0.3)
    m.add(0, fmclick(600, 660, 0.07, 1.41, 1.0, 0.003, 0.03, 0.008, 4500), 0.5)
    m.add(0.006, bloom(110, 0.2, 0.015, 0.07, 0.2), 0.3)
    return clean_room(m.y, 0.09, 0.22)


# ---- ready: three polished variants
def ready_a():
    m = Mix(0.4)
    m.add(0, pulse_modal(950, *LATCH, d=0.1, width=0.0005, soft=0.0008), 0.5)        # the latch
    m.add(0.018, pulse_modal(620, *LATCH, d=0.08, width=0.0006, soft=0.001), 0.3)     # and its catch
    nz = lpf(signal.sosfilt(sos('bandpass', [1500, 5000]), rng(6101).standard_normal(int(0.03 * SR))), 5000)
    m.add(0.004, nz / np.max(np.abs(nz)) * env(0.03, 0.002, 0.006), 0.08)              # a soft pneumatic tick
    m.add(0, thump(65, 0.25, 0.004, 0.055, 1.3, 0.012), 0.6)
    m.add(0.01, bloom(55, 0.3, 0.02, 0.1, 0.2), 0.3)
    m.add(0.045, fmclick(1150, 1150, 0.06, 1.41, 1.0, 0.004, 0.022, 0.008, 5500), 0.12)  # rising digital confirm
    m.add(0.085, fmclick(1530, 1530, 0.07, 1.41, 0.9, 0.004, 0.028, 0.008, 5500), 0.12)
    return clean_room(comp(mbsat(m.y, (1.4, 1.3, 1.05)), -16, 2.2), 0.12, 0.34)


def ready_b():
    m = Mix(0.45)
    t = T(0.035)
    pull = np.sin(2 * np.pi * np.cumsum(180 * (420 / 180) ** (t / 0.035)) / SR) * (t / 0.035) ** 2
    m.add(0, lpf(pull, 2000), 0.25)                                                     # a magnetic pull
    m.add(0.035, pulse_modal(820, *LATCH, d=0.1, width=0.0005, soft=0.0008), 0.5)
    m.add(0.035, thump(62, 0.25, 0.004, 0.055, 1.3, 0.012), 0.6)
    m.add(0.045, bloom(55, 0.3, 0.02, 0.1, 0.2), 0.3)
    m.add(0.05, fmclick(900, 1500, 0.1, 1.41, 1.0, 0.006, 0.04, 0.012, 5500), 0.14)    # a rising digital sweep
    return clean_room(comp(mbsat(m.y, (1.4, 1.3, 1.05)), -16, 2.2), 0.12, 0.34)


def ready_c():
    m = Mix(0.5)
    m.add(0, pulse_modal(640, *LATCH, d=0.12, width=0.0007, soft=0.001), 0.5)
    m.add(0.02, pulse_modal(480, *LATCH, d=0.12, width=0.0008, soft=0.001), 0.4)
    m.add(0, thump(58, 0.3, 0.004, 0.06, 1.3, 0.012), 0.55)
    m.add(0.02, thump(52, 0.3, 0.004, 0.07, 1.3, 0.012), 0.5)
    sm = lpf(rng(6301).standard_normal(int(0.06 * SR)), 2500, 4)
    m.add(0.025, sm / np.max(np.abs(sm)) * env(0.06, 0.01, 0.015), 0.07)                # smooth air, no rasp
    m.add(0.06, fmclick(1300, 1700, 0.09, 1.41, 0.9, 0.006, 0.035, 0.01, 5500), 0.12)
    return clean_room(comp(mbsat(m.y, (1.5, 1.3, 1.05)), -15, 2.2), 0.14, 0.45)


for rid, fn, desc in [
        ('lobby_ready', ready_a, 'Variant A, precise: a tight pulse-rung metal latch at 950 Hz and its catch 18 ms later, '
                                 'a soft filtered pneumatic tick, a 65 Hz thump and low bloom, then a two-step digital '
                                 'confirmation rising (1.15 to 1.53 kHz, glassy FM, quiet); a smooth clean room'),
        ('lobby_ready_b', ready_b, 'Variant B, magnetic: a 35 ms magnetic pull (a tone sweeping up) into the latch, the '
                                   'thump and bloom, a smooth rising digital sweep (0.9 to 1.5 kHz); clean room'),
        ('lobby_ready_c', ready_c, 'Variant C, heavy: a double latch (640 and 480 Hz) with a double thump, a breath of '
                                   'smooth low-passed air, a rising glint; a longer clean room')]:
    sound(rid, 'lobby', 'tap', 'You press F3 and become ready' + ('' if rid == 'lobby_ready' else ' (A/B variant)'),
          desc, var=(0, 0.6), lufs=-30, default=rid == 'lobby_ready')(fn)


# ---- rank-up: procedural
def rank_up(p, seed=4100):
    """The rank-up for progress p = rung / (ladder_len - 1) in [0, 1] (the knife rung uses its own sting).

    Deterministic given (p, seed): every noise source is seeded from `seed`. Parameter curves:
      riser (0 .. 0.30 s):  white noise through a low-pass whose cutoff climbs 300 Hz -> end(p) on the
                            curve (t/R)^1.4, end(p) = 2500 + 5000 p Hz; level (t/R)^1.8; 8 ms release.
                            An FM tone from D2 (73.4 Hz) up 1.5 + 1.0 p octaves (curve (t/R)^1.3), index
                            0.5 -> 2.5, ratio 1.41, low-passed 3 kHz. A reese on D1, cutoff 300 + 900 p Hz.
      impact (at 0.30 s):   saturated sub 95 -> 38 Hz; a hard metal modal transient (240 Hz); an air snap;
                            a glitch burst (the riser's last 40 ms, 3 repeats, scattered), gain 0.3 + 0.15 p;
                            a crunch (noise + D2 saw) multiband-saturated with drives (2.5 + p, 3 + 1.5 p,
                            1.5 + p), gain 0.25 + 0.1 p.
      tone:                 inharmonic FM shimmer at 500 + 400 p Hz (ratio 2.76) and 707 + 560 p Hz (ratio
                            1.41), low-passed 3.5 + 2.5 p kHz; a formant sweep 500 -> 1800 + 1500 p Hz.
      finish:               compression, low-pass 6.5 + 1.5 p kHz, v5's plate (0.2 wet, 0.45 s).
    """
    p = float(np.clip(p, 0, 1))
    R = 0.3
    m = Mix2(1.5)
    t = T(R); q = t / R
    end = 2500 + 5000 * p
    nz = rng(seed).standard_normal(len(t))
    rs = svf_lp(nz, 300 * (end / 300) ** q ** 1.4, 0.9) * q ** 1.8 * np.clip((R - t) / 0.008, 0, 1)
    m.add(0, chorus2(rs, 12), 0.45)
    f0, f1 = hz(38), hz(38) * 2 ** (1.5 + 1.0 * p)
    ph = 2 * np.pi * np.cumsum(f0 * (f1 / f0) ** q ** 1.3) / SR
    m.add(0, lpf(np.sin(ph + (0.5 + 2.0 * q) * np.sin(1.41 * ph)) * q ** 1.5, 3000), 0.3)
    m.add(0, reese(26, R, 0.012, 300 + 900 * p) * q ** 2, 0.2)
    m.add(R, soft_sub(95, 38, 0.6, 0.003, 0.18, 2.6), 0.85)
    m.add(R, lpf(modal(240, *METAL, d=0.15, hardness=0.7, seed=seed + 10), 6000), 0.35)
    m.add(R, air(0.05, 1500, 8000, seed + 20, 0.001, 0.012), 0.35)
    burst = rs[-int(0.04 * SR):]
    gb = np.concatenate([fade(repitch(burst, 1 + 0.08 * k), 0.4) * 0.8 ** k for k in range(3)])
    m.add(R + 0.01, chorus2(smear(gb, 0.06, seed + 30, 5), 15), 0.3 + 0.15 * p)
    crunch = air(0.2, 300, 6000, seed + 40, 0.002, 0.05) + 0.6 * osc(hz(38), 0.2, 'saw', hmax=3000) * env(0.2, 0.002, 0.05)
    m.add(R, mbsat(crunch, (2.5 + p, 3.0 + 1.5 * p, 1.5 + p)), 0.25 + 0.1 * p)
    sh = glint(500 + 400 * p, 0.8, 2.76, 3.0, 0.01, 0.2, 1.06, 3500 + 2500 * p) \
        + 0.5 * glint(707 + 560 * p, 0.8, 1.41, 2.0, 0.015, 0.18, 1.06, 3500 + 2500 * p)
    m.add(R, widen(chorus2(sh, 10), 1.3), 0.3)
    m.add(R, formant(0.4, 500, 1800 + 1500 * p, seed + 50, 6, 0.02, 0.12), 0.12)
    y = np.stack([comp(ch, -14, 2.5) for ch in m.y])
    return plate(lpf(y, 6500 + 1500 * p), 0.2, 0.45)


P_STEPS = 16
for _k in range(P_STEPS + 1):
    _p = _k / P_STEPS
    sound(f'rank_p{_k:02d}', 'rank', 'event', f'rank_up(p = {_p:.3f}): any ladder\'s rung where p = rung / (len - 1)',
          'Procedural: a 300 ms riser (filtered noise to an end point that rises with p, 2.5 to 7.5 kHz; an FM '
          'tone sweeping up 1.5 to 2.5 octaves; a reese) into a saturated sub, a metal transient, a glitch burst, '
          'a multiband crunch and an inharmonic FM shimmer; brightness and crunch follow p. The page holds 17 '
          'renders (p in steps of 1/16) and plays the nearest; an engine renders the exact p.',
          lufs=-27, p=_p, seed=4100 + _k)((lambda p, k: lambda: rank_up(p, 4100 + k))(_p, _k))


# ---- carry everything else over from v5; keep v5's order, with v6's variants and the p grid in place
for k, v in synth_v5.S.items():
    if k in S or (k.startswith('rank_') and k[5:7].isdigit() and k != 'rank_14_knife'):
        continue
    s = dict(v)
    if not s['character'].startswith('v'):
        s['character'] = 'v5\'s, kept: ' + s['character']
    S[k] = s
_order = []
for k in synth_v5.S:
    if k == 'rank_02':
        _order += [f'rank_p{i:02d}' for i in range(P_STEPS + 1)]
    if k in S and k not in _order:
        _order.append(k)
        _order += [k + suf for suf in ('_plate', '_dry') if k + suf in S]
        if k == 'lobby_ready':
            _order += ['lobby_ready_b', 'lobby_ready_c']
S = {k: S[k] for k in _order + [k for k in S if k not in _order]}


if __name__ == '__main__':
    synth_v4.S.clear(); synth_v4.S.update(S)
    synth_v4.WAV, synth_v4.OGG = os.path.join(HERE, 'v6', 'wav'), os.path.join(HERE, 'v6', 'ogg')
    synth_v4.MANIFEST, synth_v4.PREFIX = os.path.join(HERE, 'v6', 'manifest.json'), 'v6/wav/'
    synth_v4.main()
