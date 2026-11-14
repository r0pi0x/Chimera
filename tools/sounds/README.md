# Ultimate Gun Game's UI sounds (the generator)

`synth.py` (v7, the current kit) synthesizes Ultimate Gun Game's "Signal" UI
sound kit from code: oscillators, noise, modal resonators, filters and
envelopes. No samples. It builds on the earlier versions' building blocks,
which stay beside it (`synth_v1.py` to `synth_v6.py`, each frozen as it
shipped). Everything here and everything it writes is ours, CC0 1.0
(`apps/ultimate_gun_game/assets/sounds/ui/LICENSE.txt`).

## Rebuild

Python 3 with numpy and scipy (ffmpeg optional: it writes OGG page copies).

```
cd tools/sounds
python3 -m venv venv
venv/bin/pip install numpy scipy
venv/bin/python synth.py        # -> wav/*.wav, ogg/, manifest.json (here)
```

It is deterministic (fixed seeds, seeded dither). `manifest.json` is the
event table as designed: each sound's trigger, loudness tier, per-play pitch
and gain spread, channels, on/off default, measured loudness and peak.

The game plays the files its table names
(`apps/ultimate_gun_game/data/ui_sounds.toml`): copy those from `wav/` into
`apps/ultimate_gun_game/assets/sounds/ui/` (mono where a sound is mono, as
written). The A/B variants (`ui_hover_v6`, `lobby_ready_a`, `lobby_ready_c`),
the backdrop swap (`menu_backdrop`, off) and the breakcore loop (`menu_loop`)
are not shipped. `apps/ultimate_gun_game/src/signal.rs` (`UI_SOUNDS`) embeds
them; the repository's checks allow WAV files only there, beside the
licence (`own_sounds_carry_their_licence`).

## The rank-up

`rank_up(p, seed)` renders the rank-up for progress p = rung / (ladder
length - 1). The game ships its 17 renders on the p grid (`rank_p00` to
`rank_p16`, p in steps of 1/16) and plays the nearest; the last rung plays
`rank_14_knife`, and `rank_chain` layers over a chained rank-up. Porting
`rank_up` to Rust (each match's ladder rendered on a task at its start, no
per-rung files) is the follow-up (docs/plans/active/ugg-ui.md, "UI sound").
