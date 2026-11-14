#!/usr/bin/env python3
"""Ultimate Gun Game's "Signal" UI sound kit, v7: a futuristic latch, a glassier hover, a magnetic ready.

Changes from v6 (everything else is v6's, carried over; v6 is frozen in synth_v6.py):
  - hover: glassier. The same clicky front (the pulse's contact and v6's tiny FM tick), but the ring is a
    clean glass partial pair: two near-unison modes at 2.45 kHz (a slow shimmer, no grit) with one weak
    inharmonic overtone, instead of v6's three-mode glass at 2.1 kHz. v6's hover ships beside it
    (ui_hover_v6) for A/B;
  - select: a futuristic latch instead of v6's glassy click (heard as "flicking a plastic bottle": a
    700 Hz mode bank and an 880-960 Hz FM blip ringing ~55 ms in the 400-1200 Hz band, the hollow "tok").
    Now: a 16 ms magnetic pull-in (a rising tone and a sub swelling in), then a metal/composite latch with
    two contacts 4 ms apart (tight inharmonic modes at 1.75 kHz and 1.55 kHz, a damped composite body at
    330 Hz that dies in 4 ms), a tight low thump, and a clean glass accent at 3.1 kHz. Nothing rings in the
    tok band;
  - back: the same latch disengaging: the contacts first (lighter, lower), then a magnetic release (a
    tone falling 460 to 170 Hz under a closing filter), a smaller thump, no accent;
  - ready: v6's variant B "magnetic", refined, is now the default (lobby_ready): the same pull-in (35 ms,
    180 to 420 Hz) but smoothed and with a sub swell under it, released 3 ms before the contact instead of
    cut at full level; the latch is the select's family (metal contacts) over a shorter 820 Hz lock body;
    a tight thump and bloom; the rising digital sweep (0.9 to 1.5 kHz) cleaner, ending in a quiet glass
    shimmer. v6's A and C stay as lobby_ready_a and lobby_ready_c;
  - unready: no high layer. The magnetic latch releasing, soft and low: a muffled release contact, a tone
    falling 400 to 110 Hz while its low-pass closes 1.6 kHz to 180 Hz, a soft thump and a sinking bloom;
    everything under 2.5 kHz;
  - the menu family keeps v6's clean room; the plate and nearly-dry A/B renders are dropped.

  venv/bin/python synth.py   -> wav/*.wav, ogg/, manifest.json
Deterministic. Everything synthesized here. Licence: CC0 1.0.
"""
import os
import numpy as np
from synth_v1 import SR, T, env, Mix, lpf, hpf
from synth_v2 import svf_lp, TIER
from synth_v4 import thump
from synth_v5 import comp, mbsat
from synth_v6 import pulse_modal, LATCH, fmclick, bloom, clean_room, hover_dry as hover_dry_v6
import synth_v4, synth_v6

HERE = os.path.dirname(os.path.abspath(__file__))


# ================================================================ v7 building blocks
TIGHT = ((1.0, 2.76, 5.40), (0.009, 0.0045, 0.0022), (1.0, 0.3, 0.05))          # a metal contact: short, high
BODY = ((1.0, 1.59, 2.14), (0.004, 0.003, 0.002), (1.0, 0.5, 0.3))              # a composite body: no ring


def glass(f, d=0.09, tau=0.022, beat=1.006, over=0.12, width=0.0004, soft=0.0006):
    """A clean glass ring: two near-unison modes (a slow shimmer, f * (beat - 1) Hz) and one weak
    inharmonic overtone (x 2.76), rung by a raised-cosine pulse."""
    return pulse_modal(f, (1.0, beat, 2.76), (tau, 0.9 * tau, 0.25 * tau), (1.0, 0.7, over),
                       d=d, width=width, soft=soft)


def latch(f0=1750, f1=1550, gap=0.004, g2=0.7, body=330, gb=0.35, d=0.05, lp=5500):
    """A metal/composite latch: two tight metal contacts `gap` apart (the pawl's tip, then its seat), over a
    heavily damped composite body that dies in a few ms (weight without a hollow ring)."""
    a = pulse_modal(f0, *TIGHT, d=d, width=0.0004)
    b = pulse_modal(f1, *TIGHT, d=d, width=0.00045)
    k = int(gap * SR)
    y = a.copy(); y[k:] += g2 * b[:len(y) - k]
    y += gb * pulse_modal(body, *BODY, d=d, width=0.0008)
    return lpf(y, lp, 4)


def magpull(f0, f1, d, harm=0.25, release=0.003, sub=55.0, gsub=0.5):
    """A magnetic pull-in: a tone rising f0 -> f1 and a sub swelling in under it, both easing in
    (smoothstep^1.5) and released over the last `release` s, so the contact lands on silence."""
    t = T(d); q = t / d
    ph = 2 * np.pi * np.cumsum(f0 * (f1 / f0) ** q ** 1.5) / SR
    a = (q * q * (3 - 2 * q)) ** 1.5 * np.clip((d - t) / release, 0, 1)
    y = (np.sin(ph) + harm * np.sin(2 * ph)) * a
    return lpf(y, 2200) + gsub * np.sin(2 * np.pi * sub * t) * a


def magrelease(f0, f1, d, lp0, lp1, a=0.004, tau=0.07, harm=0.5):
    """A magnetic release: a tone falling f0 -> f1 with its harmonics under a low-pass closing lp0 -> lp1."""
    t = T(d); q = t / d
    ph = 2 * np.pi * np.cumsum(f0 * (f1 / f0) ** q ** 0.8) / SR
    y = np.sin(ph) + harm * np.sin(2 * ph) + 0.5 * harm * np.sin(3 * ph)
    y = svf_lp(y, lp0 * (lp1 / lp0) ** q, 0.75)
    return y / (np.max(np.abs(y)) + 1e-12) * env(d, a, tau)


# ================================================================ the event table
NEW = {}


def sound(id_, screen, tier, trigger, character, var=(0, 0), default=True, lufs=None, loop=False):
    def deco(fn):
        NEW[id_] = dict(fn=fn, screen=screen, tier=tier, target=lufs if lufs is not None else TIER[tier],
                        trigger=trigger, character=character, var_cents=var[0], var_db=var[1],
                        default_on=default, overlap='', loop=loop)
        return fn
    return deco


# ---- menu
def hover_dry():
    m = Mix(0.08)
    m.add(0, glass(2450, 0.08, 0.02, 1.006, 0.1, 0.00035, 0.0005), 0.55)
    m.add(0, fmclick(1400, 1520, 0.04, 3.5, 1.2, 0.0012, 0.009, 0.003, 7000), 0.3)    # v6's clicky tick
    return hpf(lpf(m.y, 7000), 500)


def select_dry():
    m = Mix(0.3)
    P = 0.016                                                                            # the contact lands here
    m.add(0, magpull(240, 560, P, 0.2, 0.002, 58, 0.6), 0.16)
    m.add(P, latch(1750, 1550, 0.004, 0.7, 330, 0.35), 0.55)
    m.add(P, thump(66, 0.16, 0.0025, 0.03, 1.4, 0.008), 0.7)
    m.add(P + 0.004, bloom(50, 0.12, 0.008, 0.035, 0.0), 0.2)
    m.add(P + 0.006, glass(3100, 0.1, 0.028, 1.004, 0.08), 0.12)                         # the glass accent
    return comp(mbsat(m.y, (1.3, 1.1, 1.0)), -16, 2.0)


def back_dry():
    m = Mix(0.3)
    m.add(0, latch(1500, 1650, 0.005, 0.5, 300, 0.3), 0.42)                              # contacts first
    m.add(0.006, magrelease(380, 140, 0.09, 900, 250, 0.004, 0.03, 0.25), 0.22)          # then the field lets go
    m.add(0, thump(62, 0.14, 0.003, 0.026, 1.3, 0.008), 0.55)
    m.add(0.004, bloom(52, 0.12, 0.01, 0.03, 0.0, 0.8), 0.18)
    return comp(mbsat(m.y, (1.25, 1.1, 1.0)), -16, 2.0)


@sound('ui_hover', 'menu', 'whisper', 'Pointer or focus lands on a menu entry, card or button',
       'Glassier than v6: the same clicky contact and tiny FM tick (1.4 to 1.5 kHz), ringing a clean glass '
       'partial pair at 2.45 kHz (two near-unison modes, a slow shimmer, 20 ms) with one weak overtone; no '
       'thump, no noise; clean room', var=(40, 1.2))
def _():
    return clean_room(hover_dry(), 0.09, 0.24)


@sound('ui_hover_v6', 'menu', 'whisper', 'Hover, A/B: v6\'s hover, for comparison with the glassier v7 one',
       'v6\'s: a raised-cosine pulse ringing three glass-like modes at 2.1 kHz (12 ms) and a tiny FM tick '
       'gliding up (1.4 to 1.5 kHz); clean room', var=(40, 1.2), default=False)
def _():
    return clean_room(hover_dry_v6(), 0.09, 0.24)


@sound('ui_select', 'menu', 'tap', 'An entry or button is pressed (Play, a tab, a map card)',
       'A futuristic latch: a 16 ms magnetic pull-in (a tone rising 240 to 560 Hz, a sub swelling in), then '
       'a metal/composite latch engaging (two tight metal contacts 4 ms apart at 1.75 and 1.55 kHz over a '
       'composite body damped in 4 ms), a tight 66 Hz thump and a clean glass accent at 3.1 kHz. Nothing '
       'rings in the 400-1200 Hz "tok" band; clean room', var=(20, 0.8))
def _():
    return clean_room(select_dry(), 0.09, 0.24)


@sound('ui_back', 'menu', 'tap', 'Back, close or cancel (Esc)',
       'The latch disengaging: the two contacts first (lighter, the second higher), then the magnetic field '
       'letting go (a tone falling 380 to 140 Hz under a closing filter), a smaller thump and a sinking '
       'bloom; no accent; clean room', var=(20, 0.8), lufs=-33)
def _():
    return clean_room(back_dry(), 0.09, 0.24)


# ---- lobby
def ready_b7():
    m = Mix(0.45)
    P = 0.035
    m.add(0, magpull(180, 420, P, 0.25, 0.003, 55, 0.7), 0.3)                            # the magnetic pull
    m.add(P, latch(1600, 1420, 0.005, 0.75, 300, 0.3), 0.45)                             # the select's latch
    m.add(P, lpf(pulse_modal(820, (1.0, 2.76, 5.40, 8.93), (0.012, 0.007, 0.004, 0.003), (1.0, 0.5, 0.25, 0.12),
                             d=0.06, width=0.0005, soft=0.0008), 5000), 0.3)            # a shorter lock body
    m.add(P, thump(62, 0.22, 0.003, 0.045, 1.35, 0.01), 0.65)
    m.add(P + 0.008, bloom(55, 0.26, 0.015, 0.08, 0.2), 0.28)
    m.add(P + 0.015, fmclick(900, 1500, 0.1, 1.41, 0.8, 0.006, 0.04, 0.012, 5000), 0.13)  # the rising sweep
    m.add(P + 0.05, glass(1500, 0.16, 0.05, 1.008, 0.06, 0.0008, 0.004), 0.06)           # a glass shimmer on top
    return clean_room(comp(mbsat(m.y, (1.4, 1.25, 1.0)), -16, 2.2), 0.12, 0.34)


@sound('lobby_ready', 'lobby', 'tap', 'You press F3 and become ready',
       'v6\'s variant B "magnetic", refined: a 35 ms magnetic pull (a tone rising 180 to 420 Hz with a sub '
       'swelling under it, eased and released just before the contact), the select\'s metal latch over a '
       'short 820 Hz lock body, a tight 62 Hz thump and low bloom, a clean rising digital sweep (0.9 to '
       '1.5 kHz) settling into a quiet glass shimmer; clean room', var=(0, 0.6), lufs=-30)
def _():
    return ready_b7()


sound('lobby_ready_a', 'lobby', 'tap', 'You press F3 and become ready (A/B: v6\'s variant A)',
      'v6\'s variant A, precise: a tight pulse-rung metal latch at 950 Hz and its catch 18 ms later, a soft '
      'filtered pneumatic tick, a 65 Hz thump and low bloom, then a two-step digital confirmation rising (1.15 '
      'to 1.53 kHz, glassy FM, quiet); a smooth clean room', var=(0, 0.6), lufs=-30, default=False)(synth_v6.ready_a)
sound('lobby_ready_c', 'lobby', 'tap', 'You press F3 and become ready (A/B: v6\'s variant C)',
      'v6\'s variant C, heavy: a double latch (640 and 480 Hz) with a double thump, a breath of smooth '
      'low-passed air, a rising glint; a longer clean room', var=(0, 0.6), lufs=-30, default=False)(synth_v6.ready_c)


@sound('lobby_unready', 'lobby', 'tap', 'You press F3 again and are no longer ready',
       'The magnetic latch releasing, soft and low: a muffled release contact, a tone falling 400 to 110 Hz '
       'while its low-pass closes 1.6 kHz to 180 Hz, a soft 58 Hz thump and a sinking bloom; nothing above '
       '2.5 kHz; clean room', var=(0, 0.6), lufs=-32)
def _():
    m = Mix(0.35)
    m.add(0, lpf(latch(1200, 1050, 0.006, 0.6, 280, 0.45), 1800, 4), 0.3)               # a muffled release
    m.add(0.008, magrelease(400, 110, 0.26, 1600, 180, 0.012, 0.09, 0.6), 0.5)          # the field closing down
    m.add(0, thump(58, 0.18, 0.004, 0.04, 1.15, 0.012), 0.45)
    m.add(0.01, bloom(70, 0.28, 0.02, 0.09, 0.15, 0.7), 0.3)
    y = lpf(comp(mbsat(m.y, (1.3, 1.1, 1.0)), -16, 2.0), 2500, 4)
    return clean_room(y, 0.1, 0.3)


# ---- carry everything else over from v6, in v6's order; v7's sounds and variants in place
REPLACED = {'ui_hover', 'ui_select', 'ui_back', 'lobby_ready', 'lobby_ready_b', 'lobby_ready_c', 'lobby_unready'}
AFTER = {'ui_hover': ['ui_hover_v6'], 'lobby_ready': ['lobby_ready_a', 'lobby_ready_c']}
S = {}
for k, v in synth_v6.S.items():
    if k.endswith(('_plate', '_dry')) or (k in REPLACED and k not in NEW):
        continue
    if k in NEW:
        S[k] = NEW[k]
    else:
        s = dict(v)
        if not s['character'].startswith('v'):
            s['character'] = 'v6\'s, kept: ' + s['character']
        S[k] = s
    for a in AFTER.get(k, []):
        S[a] = NEW[a]
assert set(NEW) <= set(S), set(NEW) - set(S)


if __name__ == '__main__':
    synth_v4.S.clear(); synth_v4.S.update(S)
    synth_v4.WAV, synth_v4.OGG = os.path.join(HERE, 'wav'), os.path.join(HERE, 'ogg')
    synth_v4.MANIFEST, synth_v4.PREFIX = os.path.join(HERE, 'manifest.json'), 'wav/'
    for d in ('wav', 'ogg'):                       # v6's A/B renders that v7 no longer ships
        for f in os.listdir(os.path.join(HERE, d)) if os.path.isdir(os.path.join(HERE, d)) else []:
            if f.rsplit('.', 1)[0] not in S:
                os.remove(os.path.join(HERE, d, f))
    synth_v4.main()
