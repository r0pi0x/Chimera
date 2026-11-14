#!/usr/bin/env python3
"""Ultimate Gun Game's "Signal" UI sound kit, v5: designed sounds, not notes (kept for A/B; v6 is synth.py).

v4's menu cues read as "a synth chord on a piano". v5 drops musical chords
as the body of UI sounds. Bodies are now:
  - modal synthesis: a short, soft excitation (a windowed noise impulse whose
    window sets the hardness) ringing a small bank of damped resonators at
    inharmonic ratios: a plastic switch, a metal latch, a magnetic snap;
  - air: filtered noise with shaped envelopes, short reverse swells, spectral
    smears (noise grains scattered over a few tens of ms);
  - sub and thud: round, soft-attack low ends for weight;
  - sweeteners: faint inharmonic FM glints or formant sweeps, filtered and
    quiet, never a recognisable chord;
then dense and expensive: multiband saturation (low / mid / high driven
separately), gentle compression, and a short decorrelated stereo plate so
even small cues have space (mono-safe: one impulse response per ear, never
inverted). Rewards ride a real riser into a crunchy impact; the air strike is a
synthesized air-raid siren (a rotor's port-chopped tone spinning up and
coasting down) over a building drone, with risers each second and a huge dark
impact.

Kept from v4 (the user liked them): both loops, the countdown ticks, the logo
pass (4 dB quieter), the reveal, Tab and backdrop.

  venv/bin/python synth_v5.py -> v5/wav/*.wav (48 kHz 16-bit, mono or stereo), ogg/, manifest.json
Deterministic. Everything synthesized here. Licence: CC0 1.0.
"""
import os
import numpy as np
from scipy import signal
from synth_v1 import SR, T, env, blip, rng, svf, sos, hpf, lpf, Mix, hz, deg
from synth_v2 import sat, osc, sub, repitch, fade, svf_lp, S16, S32, S64, TIER
from synth_v3 import swell, grains, reese
from synth_v4 import thump, breath, pan, st, Mix2, hall, widen, chorus2
import synth_v3, synth_v4

HERE = os.path.dirname(os.path.abspath(__file__))


# ================================================================ v5 building blocks
def excite(d=0.002, hardness=0.5, seed=0):
    """A contact impulse: noise under a window whose shape sets the hardness (0 soft .. 1 hard)."""
    n = max(8, int(d * SR))
    beta = 2 + 14 * hardness
    w = np.kaiser(n, beta)
    return rng(seed).standard_normal(n) * w


def modal(f0, ratios, decays, gains, d=0.12, hardness=0.4, exd=0.002, seed=0, detune=0.0, soft=0.0):
    """Modal synthesis: an impulse ringing damped two-pole resonators (f0 * ratio, decay seconds)."""
    x = np.zeros(int(d * SR))
    e = excite(exd, hardness, seed)
    x[:len(e)] = e
    y = np.zeros_like(x)
    for k, (r_, dc, g) in enumerate(zip(ratios, decays, gains)):
        f = f0 * r_ * (1 + detune * (k % 2 * 2 - 1))
        if f >= SR * 0.45:
            continue
        rr = np.exp(-1.0 / (dc * SR))
        w = 2 * np.pi * f / SR
        yy = signal.lfilter([1 - rr], [1, -2 * rr * np.cos(w), rr * rr], x)
        y += g * yy / (np.max(np.abs(yy)) + 1e-12)
    if soft > 0:                                   # round the transient: a raised-cosine fade-in
        y *= 0.5 - 0.5 * np.cos(np.pi * np.clip(np.arange(len(y)) / (soft * SR), 0, 1))
    return y / (np.max(np.abs(y)) + 1e-12)


PLASTIC = ((1.0, 2.32, 4.25, 6.63), (0.018, 0.011, 0.007, 0.005), (1.0, 0.55, 0.3, 0.18))
METAL = ((1.0, 2.76, 5.40, 8.93), (0.035, 0.022, 0.014, 0.009), (1.0, 0.6, 0.35, 0.2))
DULL = ((1.0, 1.59, 2.14, 2.65), (0.010, 0.007, 0.005, 0.004), (1.0, 0.5, 0.3, 0.2))


def air(d, lo, hi, seed, a=0.006, tau=0.03, hold=0.0, sweep=None):
    """Filtered noise; sweep=(f0, f1) moves a soft low-pass across the sound."""
    y = signal.sosfilt(sos('bandpass', [lo, hi]), rng(seed).standard_normal(len(T(d))))
    if sweep:
        t = T(d)
        y = svf_lp(y, sweep[0] * (sweep[1] / sweep[0]) ** (t / d), 0.7)
    return y / (np.max(np.abs(y)) + 1e-12) * env(d, a, tau, hold)


def smear(x, spread=0.03, seed=0, n=6):
    """Spectral smear: the sound copied as a few scattered, softened grains."""
    r = rng(seed)
    out = np.zeros(len(x) + int(spread * SR))
    for k in range(n):
        o = int(r.random() * spread * SR)
        out[o:o + len(x)] += x * (0.6 ** k) * np.hanning(len(x)) ** 0.25
    return out / n * 2


def glint(fc, d, ratio=1.414, index=1.2, a=0.012, tau=0.05, rise=1.0, lp=5000):
    """A sweetener: an inharmonic FM glint (optionally rising by `rise`), filtered."""
    t = T(d)
    f = fc * rise ** (t / d)
    ph = 2 * np.pi * np.cumsum(f) / SR
    y = np.sin(ph + index * np.exp(-t / (tau * 2)) * np.sin(ratio * ph))
    return lpf(y * env(d, a, tau), lp)


def formant(d, f0, f1, seed, q=5.0, a=0.02, tau=0.1):
    t = T(d)
    y = svf(rng(seed).standard_normal(len(t)), f0 * (f1 / f0) ** (t / d), q)
    return y / (np.max(np.abs(y)) + 1e-12) * env(d, a, tau)


def mbsat(x, drives=(1.6, 2.0, 1.3), mix=(1.0, 0.9, 0.7), xo=(220, 2200)):
    """Multiband saturation: low, mid and high driven separately, summed."""
    lo = signal.sosfiltfilt(sos('lowpass', xo[0], 4), x)
    hi = signal.sosfiltfilt(sos('highpass', xo[1], 4), x)
    md = x - lo - hi
    pk = np.max(np.abs(x)) + 1e-12
    return sum(m * sat(b / pk, dr) * pk for b, dr, m in zip((lo, md, hi), drives, mix))


def comp(x, thr_db=-18, ratio=3.0, tau=0.008):
    """Gentle compression on a smoothed RMS envelope (dense, not pumping)."""
    pk = np.max(np.abs(x)) + 1e-12
    y = x / pk
    a = np.exp(-1 / (tau * SR))
    e = np.sqrt(signal.lfilter([1 - a], [1, -a], y * y) + 1e-12)
    ed = 20 * np.log10(e)
    gr = np.where(ed > thr_db, (thr_db - ed) * (1 - 1 / ratio), 0.0)
    return y * 10 ** (gr / 20) * pk


def plate(x, wet=0.16, size=0.32, pre=0.008, seeds=(3001, 3002), bright=5000):
    """A short decorrelated stereo plate: small cues get space; mono-safe."""
    return synth_v4.hall(x, size, pre, wet, seeds, bright, 250)


def soft_sub(f0, f1, d, a=0.005, tau=0.12, drive=1.8):
    t = T(d)
    fr = f1 + (f0 - f1) * np.exp(-t / 0.04)
    return sat(np.sin(2 * np.pi * np.cumsum(fr) / SR), drive) * env(d, a, tau)


# ================================================================ the event table
S = {}


def sound(id_, screen, tier, trigger, character, var=(0, 0), default=True, lufs=None, loop=False):
    def deco(fn):
        S[id_] = dict(fn=fn, screen=screen, tier=tier, target=lufs if lufs is not None else TIER[tier],
                      trigger=trigger, character=character, var_cents=var[0], var_db=var[1],
                      default_on=default, overlap='', loop=loop)
        return fn
    return deco


def keep(id_, note='', lufs=None):
    """Carry a v4 sound over unchanged (optionally at another level)."""
    s = dict(synth_v4.S[id_])
    if lufs is not None:
        s['target'] = lufs
    s['character'] = 'v4\'s, kept' + (note and ' ' + note) + ': ' + s['character']
    S[id_] = s


# ---- menu: one designed family (engage, air, plate)
def engage(m, t0, f0=520, mat=PLASTIC, g=1.0, hard=0.35, seed=0, thud=0.5, thud_f=80):
    m.add(t0, lpf(modal(f0, *mat, d=0.1, hardness=hard, seed=seed, soft=0.003), 5500), 0.55 * g)
    m.add(t0, thump(thud_f, 0.12, 0.004, 0.035, 1.2, 0.012), thud * g)


@sound('ui_hover', 'menu', 'whisper', 'Pointer or focus lands on a menu entry, card or button',
       'A barely-there magnetic touch: a soft air puff (900 Hz to 4 kHz, 6 ms attack) with a faint plastic tap '
       'under it (soft impulse), in a short stereo plate', var=(40, 1.2))
def _():
    m = Mix(0.12)
    m.add(0, air(0.06, 900, 4000, 3101, 0.006, 0.018), 0.6)
    m.add(0.004, lpf(modal(1500, *PLASTIC, d=0.05, hardness=0.15, seed=3102, soft=0.004), 4500), 0.18)
    return plate(m.y, 0.14, 0.25)


@sound('ui_select', 'menu', 'tap', 'An entry or button is pressed (Play, a tab, a map card)',
       'An expensive confirm, no chord: a 15 ms reverse air swell into a soft magnetic/mechanical engage (a '
       'plastic-and-metal modal latch with a round 80 Hz thud and a whisper of sub), then an airy swell blooming '
       'open and a faint inharmonic FM glint as the sweetener; multiband-saturated, gently compressed, a short '
       'wide plate', var=(25, 0.8))
def _():
    m = Mix(0.5)
    m.add(0, air(0.015, 600, 3000, 3201, 0.014, 0.004)[::-1], 0.35)
    engage(m, 0.015, 520, PLASTIC, 1.0, 0.35, 3202, 0.55)
    m.add(0.015, lpf(modal(1300, *METAL, d=0.08, hardness=0.3, seed=3203), 5000), 0.12)
    m.add(0.015, soft_sub(70, 45, 0.18, 0.006, 0.06), 0.18)
    m.add(0.03, air(0.25, 800, 6000, 3204, 0.04, 0.07, sweep=(700, 5000)), 0.3)
    m.add(0.035, glint(2200, 0.18, 1.37, 0.8, 0.015, 0.05, 1.08), 0.07)
    y = comp(mbsat(m.y, (1.5, 1.8, 1.2)), -16, 2.5)
    return plate(lpf(y, 6500), 0.18, 0.35)


@sound('ui_back', 'menu', 'tap', 'Back, close or cancel (Esc)',
       'The same family withdrawing: an air whoosh-out whose filter closes (3.5 kHz to 500 Hz) and a softer, lower '
       'disengage; no tone, no sub', var=(25, 0.8), lufs=-33)
def _():
    m = Mix(0.45)
    m.add(0, air(0.2, 400, 5000, 3301, 0.012, 0.06, sweep=(3500, 500)), 0.45)
    engage(m, 0.03, 400, PLASTIC, 0.8, 0.25, 3302, 0.4, 70)
    y = comp(mbsat(m.y, (1.4, 1.6, 1.1)), -16, 2.5)
    return plate(lpf(y, 6000), 0.16, 0.3)


@sound('ui_denied', 'menu', 'tap', 'A locked or unavailable entry is pressed',
       'v4\'s direction in the new family: a muted, dull double-tap (heavily damped modes, low-passed at '
       '1.2 kHz, a soft thud), the second lower and quieter; no tone', var=(0, 0.5))
def _():
    m = Mix(0.3)
    for k, (t0, f0, g) in enumerate([(0, 330, 1.0), (0.09, 290, 0.7)]):
        m.add(t0, lpf(modal(f0, *DULL, d=0.06, hardness=0.2, seed=3401 + k, soft=0.005), 1200), 0.8 * g)
        m.add(t0, thump(70, 0.1, 0.004, 0.03, 1.1), 0.6 * g)
        m.add(t0, air(0.04, 200, 900, 3403 + k, 0.006, 0.012), 0.25 * g)
    return plate(m.y, 0.1, 0.22)


@sound('ui_slider', 'menu', 'whisper', 'A slider step; the engine sets the speed from the value, 0.5 to 2.0, no '
       'random pitch', 'A smooth tactile detent: a soft noise micro-click (2 ms, kaiser-windowed) ringing one '
       'low-Q resonator at 1.2 kHz (8 ms), so the value moves the detent\'s colour (600 Hz to 2.4 kHz), not a '
       'pitch', var=(0, 0.6))
def _():
    m = Mix(0.05)
    m.add(0, lpf(modal(1200, (1.0, 1.8), (0.010, 0.005), (1.0, 0.3), d=0.05, hardness=0.2, seed=3501, soft=0.003), 4500), 0.7)
    m.add(0, air(0.025, 1500, 5000, 3502, 0.003, 0.006), 0.2)
    return m.y


keep('menu_backdrop')


# ---- logo pass
keep('pass_sweep', '4 dB quieter', -30)


# ---- lobby
@sound('lobby_ready', 'lobby', 'tap', 'You press F3 and become ready',
       'A short premium lock-in (about 300 ms): a precise metal modal latch, a compressed-air tick (8 ms), a '
       'low 65 Hz thud, then a quick bright digital confirmation (an inharmonic FM glint rising, filtered at '
       '5 kHz) and a short wide plate; multiband-saturated', var=(0, 0.6), lufs=-30)
def _():
    m = Mix(0.35)
    m.add(0, lpf(modal(760, *METAL, d=0.12, hardness=0.55, seed=3601, soft=0.0012), 6000), 0.6)
    m.add(0, lpf(modal(380, *PLASTIC, d=0.08, hardness=0.4, seed=3602), 4000), 0.35)
    m.add(0.008, air(0.03, 2000, 7000, 3603, 0.0015, 0.006), 0.35)
    m.add(0, thump(65, 0.18, 0.004, 0.05, 1.3, 0.012), 0.6)
    m.add(0.03, glint(1800, 0.09, 1.41, 1.0, 0.008, 0.03, 1.35), 0.14)
    m.add(0.04, glint(2700, 0.07, 1.41, 0.6, 0.008, 0.02, 1.2), 0.07)
    y = comp(mbsat(m.y, (1.6, 1.8, 1.25)), -16, 2.5)
    return plate(lpf(y, 7000), 0.18, 0.3)


@sound('lobby_unready', 'lobby', 'tap', 'You press F3 again and are no longer ready',
       'The lock releasing: a softer, lower unlatch, a longer air release (hiss, 70 ms), the glint falling '
       'away under a closing filter; short plate', var=(0, 0.6), lufs=-31)
def _():
    m = Mix(0.35)
    m.add(0, lpf(modal(560, *METAL, d=0.1, hardness=0.3, seed=3701), 4500), 0.45)
    m.add(0, thump(60, 0.15, 0.004, 0.04, 1.1), 0.4)
    m.add(0.01, air(0.09, 1200, 5000, 3702, 0.006, 0.03, sweep=(5000, 1200)), 0.3)
    m.add(0.02, glint(2200, 0.12, 1.41, 0.6, 0.01, 0.04, 0.7, 3500), 0.1)
    y = comp(mbsat(m.y, (1.4, 1.6, 1.1)), -16, 2.5)
    return plate(lpf(y, 6000), 0.16, 0.3)


@sound('lobby_peer', 'lobby', 'soft', 'Another player readies up (or joins)',
       'A light, positive tertiary cue: a tiny upward glint (an FM sparkle rising a fifth, 2.3 to 3.5 kHz, '
       'filtered) over a puff of bright air, quiet', var=(30, 1.2), lufs=-38)
def _():
    m = Mix(0.15)
    m.add(0, air(0.05, 2000, 7000, 3801, 0.008, 0.015), 0.4)
    m.add(0.005, glint(2300, 0.09, 1.41, 0.5, 0.008, 0.03, 1.5, 6500), 0.5)
    return plate(m.y, 0.14, 0.25)


for _id in ('count_tick', 'count_tick_3', 'count_tick_2', 'count_tick_1'):
    keep(_id)


@sound('count_go', 'lobby', 'event', 'The countdown reaches 0 and the match starts; stereo',
       'A launch, not a chord: a saturated sub punch and a low impact; a wide pressure release (a noise burst '
       'whose high-pass sweeps up as it opens out, decorrelated left and right); a granular burst of sparks '
       'thrown wide; an inharmonic metallic FM bloom swelling under it; multiband-saturated; a long dark hall')
def _():
    m = Mix2(2.6)
    m.add(0, soft_sub(85, 34, 0.9, 0.004, 0.3, 2.4), 0.9)
    m.add(0, lpf(modal(140, *METAL, d=0.3, hardness=0.5, seed=3901), 2500), 0.35)
    for ch, sd in ((-0.7, 3902), (0.7, 3903)):
        t = T(0.9)
        nz = air(0.9, 150, 9000, sd, 0.005, 0.25)
        nz = signal.sosfilt(sos('lowpass', 7000), nz)
        hp = svf(nz, 400 * (3000 / 400) ** (t / 0.9), 0.8)
        m.add(0, pan(hp, ch), 0.35)
    gl = grains(0.6, 3904, 140, (78, 81, 85, 88, 92), (0.004, 0.012), 6000, shape=lambda u: (1 - u) ** 2)
    gr = grains(0.6, 3905, 140, (78, 81, 85, 88, 92), (0.004, 0.012), 6000, shape=lambda u: (1 - u) ** 2)
    m.add(0.01, np.stack([gl, gr]), 0.25)
    bloom = glint(330, 1.4, 2.76, 2.5, 0.08, 0.45, 1.02, 3000) + 0.6 * glint(495, 1.4, 1.41, 1.5, 0.1, 0.4, 1.0, 3000)
    m.add(0.02, widen(chorus2(bloom, 8), 1.4), 0.3)
    y = m.y
    y = np.stack([comp(mbsat(ch, (2.0, 1.7, 1.2)), -14, 2.5) for ch in y])
    return synth_v4.hall(y, 2.4, 0.06, 0.24, (3906, 3907), 4500)


# ---- HUD
@sound('hud_chip', 'hud', 'soft', 'A HUD chip slides in (threat, objective, status)',
       'The hover\'s air touch with a short upward air sweep and a faint glint', var=(25, 1.0), lufs=-35)
def _():
    m = Mix(0.16)
    m.add(0, air(0.07, 600, 5000, 4001, 0.01, 0.02, sweep=(800, 4500)), 0.5)
    m.add(0.01, lpf(modal(1100, *PLASTIC, d=0.05, hardness=0.2, seed=4002), 4500), 0.2)
    m.add(0.015, glint(2000, 0.06, 1.41, 0.5, 0.008, 0.02, 1.2), 0.12)
    return plate(m.y, 0.14, 0.25)


@sound('feed_self', 'hud', 'soft', 'Your own kill enters the kill feed (off where an announcer pack calls kills)',
       'A small engage: the select\'s latch and thud, shorter, no swell', var=(25, 1.0), default=False, lufs=-36)
def _():
    m = Mix(0.15)
    engage(m, 0, 600, PLASTIC, 0.8, 0.35, 4011, 0.4)
    return plate(m.y, 0.12, 0.25)


keep('tab_open')
keep('tab_close')


# ---- rank-up: a riser into a crunchy impact
def rank(n):
    u = (n - 2) / 11
    R = 0.3                                                # the riser; the impact lands at 0.3 s
    m = Mix2(1.5)
    t = T(R)
    p = (t / R)
    end = 2500 + 5000 * u                                  # the riser's end point climbs with the rung
    nz = rng(4100 + n).standard_normal(len(t))
    rs = svf_lp(nz, 300 * (end / 300) ** p ** 1.4, 0.9) * p ** 1.8 * np.clip((R - t) / 0.008, 0, 1)
    m.add(0, chorus2(rs, 12), 0.45)
    f0, f1 = hz(38), hz(38) * 2 ** (1.5 + 1.0 * u)         # an FM tone sweeping up 1.5 to 2.5 octaves
    fr = f0 * (f1 / f0) ** p ** 1.3
    ph = 2 * np.pi * np.cumsum(fr) / SR
    fm = np.sin(ph + (0.5 + 2.0 * p) * np.sin(1.41 * ph)) * p ** 1.5
    m.add(0, lpf(fm, 3000), 0.3)
    rz = reese(26, R, 0.012, 300 + 900 * u) * p ** 2
    m.add(0, rz, 0.2)
    # the impact
    m.add(R, soft_sub(95, 38, 0.6, 0.003, 0.18, 2.6), 0.85)
    m.add(R, lpf(modal(240, *METAL, d=0.15, hardness=0.7, seed=4110 + n), 6000), 0.35)
    m.add(R, air(0.05, 1500, 8000, 4120 + n, 0.001, 0.012), 0.35)
    burst = rs[-int(0.04 * SR):]
    gb = np.concatenate([fade(repitch(burst, 1 + 0.08 * k), 0.4) * 0.8 ** k for k in range(3)])
    m.add(R + 0.01, chorus2(smear(gb, 0.06, 4130 + n, 5), 15), 0.3 + 0.15 * u)        # glitch / granular burst
    crunch = air(0.2, 300, 6000, 4140 + n, 0.002, 0.05) + 0.6 * osc(hz(38), 0.2, 'saw', hmax=3000) * env(0.2, 0.002, 0.05)
    m.add(R, mbsat(crunch, (2.5 + u, 3.0 + 1.5 * u, 1.5 + u)), 0.25 + 0.1 * u)       # crunchy texture
    # tonal: an inharmonic metallic FM shimmer and a formant sweep up, not a triad
    sh = glint(500 + 400 * u, 0.8, 2.76, 3.0, 0.01, 0.2, 1.06, 3500 + 2500 * u) \
        + 0.5 * glint(707 + 560 * u, 0.8, 1.41, 2.0, 0.015, 0.18, 1.06, 3500 + 2500 * u)
    m.add(R, widen(chorus2(sh, 10), 1.3), 0.3)
    m.add(R, formant(0.4, 500, 1800 + 1500 * u, 4150 + n, 6, 0.02, 0.12), 0.12)
    y = np.stack([comp(ch, -14, 2.5) for ch in m.y])
    return plate(lpf(y, 6500 + 1500 * u), 0.2, 0.45)


for _n in range(2, 14):
    sound(f'rank_{_n:02d}', 'rank', 'event', f'You reach rung {_n} (a kill moves you up the ladder); stereo',
          'A modern riser into a crunchy impact: 300 ms of rising filtered white noise (its cutoff climbing to an '
          'end point that rises with the rung, 2.5 to 7.5 kHz), a pitch-swept FM tone (up 1.5 to 2.5 octaves) and a '
          'reese swelling; then a saturated sub, a hard metal transient, a glitch/granular burst cut from the '
          'riser\'s top, a multiband-saturated crunch, an inharmonic metallic FM shimmer and a formant sweep '
          'upward; a wide plate. Higher rungs: a higher riser end, more brightness, crunch and burst.',
          lufs=-27)((lambda n: lambda: rank(n))(_n))


@sound('rank_14_knife', 'rank', 'hero', 'You reach the knife rung (14, the last); stereo',
       'The sting the user likes (v3/v4), fitted to the family: its roll, chord, buffer-repeat cascade and blade '
       'unchanged, now with an inharmonic metallic shimmer and a granular burst on the drop, multiband density '
       'and a wide decorrelated plate')
def _():
    k = synth_v3.S['rank_14_knife']['fn']()
    m = Mix2(len(k) / SR + 0.5)
    m.add(0, k, 1.0)
    sh = glint(450, 1.2, 2.76, 3.0, 0.01, 0.3, 1.04, 5000) + 0.5 * glint(640, 1.2, 1.41, 2.0, 0.01, 0.3, 1.04, 5000)
    m.add(0.33, widen(chorus2(sh, 10), 1.3), 0.12)
    gl = grains(0.5, 4201, 140, (78, 81, 85, 88), (0.004, 0.012), 6000, shape=lambda u: 1 - u)
    gr = grains(0.5, 4202, 140, (78, 81, 85, 88), (0.004, 0.012), 6000, shape=lambda u: 1 - u)
    m.add(0.33, np.stack([gl, gr]), 0.12)
    y = np.stack([mbsat(ch, (1.3, 1.4, 1.1), (1, 1, 0.9)) for ch in m.y])
    return plate(y, 0.16, 0.6, 0.012)


@sound('rank_chain', 'rank', 'soft', 'A second rank-up lands while the toast still shows (under 1.15 s)',
       'A quick upward glint: a tiny riser snap and an FM sparkle rising a fifth', lufs=-34)
def _():
    m = Mix(0.2)
    t = T(0.06)
    m.add(0, svf_lp(rng(4301).standard_normal(len(t)), 800 * 6 ** (t / 0.06), 0.8) * (t / 0.06) ** 1.5, 0.4)
    m.add(0.06, glint(2000, 0.1, 1.41, 0.7, 0.006, 0.03, 1.5, 6000), 0.5)
    return plate(m.y, 0.14, 0.25)


# ---- air strike: an air-raid siren, a drone, risers, a huge impact
def siren(d, curve, ports=(1.0, 1.2), seed=0, hmax=3500.0):
    """A rotor siren: air chopped by ports, rich in harmonics; `curve` (0..1 per sample) is the rotor's speed.
    Two rotors a minor third apart (5:6), slightly detuned, plus the air's own noise."""
    t = T(d)
    base = 120 + 320 * curve                                # Hz at the lower rotor
    y = np.zeros(len(t))
    for k, pr in enumerate(ports):
        f = base * pr * (1 + 0.003 * k)
        ph = 2 * np.pi * np.cumsum(f) / SR
        nh = int(hmax / (float(np.max(f)) + 1))
        w = sum(np.sin(h * ph) / h ** 1.3 for h in range(1, nh + 1))   # chopped air: a soft saw-like spectrum
        y += w
    nzz = signal.sosfilt(sos('bandpass', [300, 3000]), rng(seed).standard_normal(len(t)))
    y = y / len(ports) + 0.15 * nzz / np.max(np.abs(nzz)) * curve
    y = lpf(hpf(y, 160), 2600)                              # distance: no lows, no air on top
    return y / (np.max(np.abs(y)) + 1e-12)


def drone(d, seed, build=1.0):
    t = T(d)
    p = (t / d)
    r = svf_lp(reese(26, d, 0.007, 900, 2.0), 180 + 400 * p * build, 0.8)   # opens as it builds
    l = lpf(rng(seed).standard_normal(len(t)), 100, 4); rr = lpf(rng(seed + 1).standard_normal(len(t)), 100, 4)
    l /= np.max(np.abs(l)); rr /= np.max(np.abs(rr))
    s = np.sin(2 * np.pi * hz(26) * t)
    g = (0.3 + 0.7 * p ** 1.5 * build)
    return np.stack([(0.5 * r + 0.5 * l + 0.6 * s) * g, (0.5 * r + 0.5 * rr + 0.6 * s) * g])


@sound('air_alert', 'air', 'event', 'The air-strike banner opens (0.25 s); its siren runs to the impact; stereo',
       'An air-raid siren, synthesized: two rotor tones a minor third apart, their chopped-air harmonics and '
       'wind noise, spinning up over 1.3 s, holding, then coasting down into the impact (3.9 s); far away '
       '(high-passed, low-passed, a 4 s hall with a long pre-delay), drifting across the field; under it a deep '
       'reese drone and sub rumble building in intensity', lufs=-26)
def _():
    d = 3.95
    t = T(d)
    up = 1 - np.exp(-t / 0.45)
    down = np.clip((t - 2.4) / 1.5, 0, 1) ** 1.5
    curve = np.clip(up * (1 - 0.55 * down), 0, 1)
    sr_ = siren(d, curve, seed=4401) * env(d, 0.25, 9.0) * (1 - 0.6 * down) * np.clip((d - t) / 0.35, 0, 1)
    m = Mix2(d + 0.2)
    m.add(0, pan(sr_, 0.4 * np.sin(2 * np.pi * t / 7.0)), 0.55)
    m.add(0, drone(d, 4402) * np.clip((d - t) / 0.3, 0, 1), 0.45)
    y = synth_v4.hall(m.y, 4.0, 0.09, 0.45, (4403, 4404), 3000)
    return y


@sound('air_pulse', 'air', 'cue', 'Each countdown second of the strike (1.2, 2.2, 3.2 s); stereo',
       'A tension riser per second: a deep sub pulse on the beat, then a reversed swell and a rising filtered '
       'noise with a pitch-swept FM underneath that build through the second into the next one (the last into '
       'the impact)', var=(0, 0.4), lufs=-28)
def _():
    m = Mix2(1.0)
    m.add(0, soft_sub(60, 36, 0.5, 0.006, 0.16, 2.2), 0.7)
    R = 0.9; t = T(R); p = t / R
    sw = swell(R, 4501, 200, 3000, 0.2)
    cut = np.clip((R - t) / 0.02, 0, 1)                    # a 20 ms release, so the riser never clicks off
    m.add(0.05, chorus2(svf_lp(sw, 200 + 2200 * p ** 2, 0.8) * cut, 10), 0.45)
    fr = hz(26) * 2 ** (2 * p ** 1.5)
    ph = 2 * np.pi * np.cumsum(fr) / SR
    m.add(0.05, lpf(np.sin(ph + 1.2 * p * np.sin(1.41 * ph)) * p ** 2 * cut, 1500), 0.3)
    y = np.stack([comp(mbsat(ch, (1.8, 1.5, 1.1)), -14, 2.5) for ch in m.y])
    return synth_v4.hall(y, 1.2, 0.04, 0.15, (4502, 4503), 3000)


@sound('air_impact', 'air', 'hero', 'The strike lands (UI layer only; the world\'s explosions are the game\'s); stereo',
       'A huge low impact with a long dark tail: a saturated sub drop (90 to 24 Hz), a wide decorrelated '
       'low-passed blast, low granular debris, the drone collapsing into a rumble, a 4.5 s dark hall',
       lufs=-24)
def _():
    m = Mix2(4.5)
    m.add(0, soft_sub(90, 24, 2.0, 0.003, 0.6, 2.8), 1.0)
    m.add(0, lpf(modal(70, *METAL, d=0.5, hardness=0.8, seed=4601), 900), 0.4)
    for ch, sd in ((-0.8, 4602), (0.8, 4603)):
        bl = lpf(rng(sd).standard_normal(int(1.5 * SR)), 1200, 4) * env(1.5, 0.003, 0.35)
        m.add(0, pan(bl / np.max(np.abs(bl)), ch), 0.5)
    dl = lpf(grains(1.5, 4604, 60, (38, 40, 43, 45), (0.02, 0.06), 600, shape=lambda u: (1 - u) ** 2), 600)
    dr = lpf(grains(1.5, 4605, 60, (38, 40, 43, 45), (0.02, 0.06), 600, shape=lambda u: (1 - u) ** 2), 600)
    m.add(0.05, np.stack([dl, dr]), 0.3)
    dn = drone(3.5, 4606, 0.4)
    dn = dn * np.exp(-T(3.5) / 1.2)
    m.add(0.02, dn, 0.4)
    y = np.stack([comp(mbsat(ch, (2.2, 1.5, 1.1), (1, 0.9, 0.6)), -12, 2.5) for ch in m.y])
    return synth_v4.hall(y, 4.5, 0.05, 0.4, (4607, 4608), 1500)


# ---- match end
keep('end_reveal')


def vote(f0, k):
    m = Mix(0.4)
    m.add(0, air(0.012, 600, 3000, 4700 + k, 0.01, 0.003)[::-1], 0.3)
    engage(m, 0.012, f0, PLASTIC, 1.0, 0.35, 4710 + k, 0.5)
    m.add(0.025, air(0.18, 800, 5000, 4720 + k, 0.03, 0.05, sweep=(700, 4000)), 0.25)
    y = comp(mbsat(m.y, (1.5, 1.7, 1.2)), -16, 2.5)
    return plate(lpf(y, 6500), 0.16, 0.3)


for _k, _f in enumerate([460, 520, 585, 690]):
    sound(f'vote_cast_{_k + 1}', 'end', 'tap', f'You vote for map card {_k + 1} (or change your vote to it)',
          'The select\'s engage and airy swell, a little shorter; each card\'s latch sits a step higher in '
          'its colour (460 to 690 Hz), not a note', var=(0, 0.8), lufs=-31)((lambda f, k: lambda: vote(f, k))(_f, _k))


@sound('vote_other', 'end', 'soft', 'Someone else votes (a pip lands on a card)',
       'The peer\'s glint, smaller and lower', var=(30, 1.2), lufs=-39)
def _():
    m = Mix(0.12)
    m.add(0, air(0.04, 1500, 6000, 4801, 0.006, 0.012), 0.4)
    m.add(0.004, glint(1700, 0.07, 1.41, 0.4, 0.006, 0.025, 1.3, 5500), 0.45)
    return plate(m.y, 0.12, 0.22)


@sound('vote_lock', 'end', 'event', 'The vote countdown hits 0: the vote locks (the pass then starts the next match)',
       'A heavy lock: a big metal latch and a plastic one together, a compressed-air burst, a saturated low '
       'thud and sub, a short buffer-repeat of the latch, a falling-into-place glint; multiband density, plate')
def _():
    m = Mix(0.7)
    lt = lpf(modal(420, *METAL, d=0.15, hardness=0.6, seed=4901), 6000)
    m.add(0, lt, 0.6)
    m.add(0.045, fade(lt[:int(0.03 * SR)], 0.5) * 0.5, 0.6)
    m.add(0, lpf(modal(240, *PLASTIC, d=0.1, hardness=0.5, seed=4902), 4000), 0.4)
    m.add(0.006, air(0.06, 1500, 7000, 4903, 0.002, 0.015), 0.35)
    m.add(0, soft_sub(85, 38, 0.4, 0.003, 0.12, 2.4), 0.8)
    m.add(0.03, glint(1500, 0.15, 1.41, 0.8, 0.01, 0.05, 0.85, 4500), 0.12)
    y = comp(mbsat(m.y, (2.0, 1.8, 1.2)), -14, 2.5)
    return plate(lpf(y, 6500), 0.2, 0.45)


keep('menu_loop')
keep('menu_loop_scifi')

# keep v4's screen order for the page
ORDER = list(synth_v4.S.keys())
S = {k: S[k] for k in ORDER if k in S} | {k: v for k, v in S.items() if k not in ORDER}


if __name__ == '__main__':
    synth_v4.S.clear(); synth_v4.S.update(S)
    synth_v4.WAV, synth_v4.OGG = os.path.join(HERE, 'v5', 'wav'), os.path.join(HERE, 'v5', 'ogg')
    synth_v4.MANIFEST, synth_v4.PREFIX = os.path.join(HERE, 'v5', 'manifest.json'), 'v5/wav/'
    synth_v4.main()
