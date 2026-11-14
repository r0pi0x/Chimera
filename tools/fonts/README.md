# UGG fonts: sources

The build scripts of the Ultimate Gun Game's faces (UGG Display Black,
Bold, Regular; UGG Mono Regular), the OFL-1.1 sources of the TTFs in
`apps/ultimate_gun_game/fonts/` (licence: `OFL.txt` there and here;
history: `FONTLOG.txt` there). Our own work, Copyright 2026 Lucker Party
contributors.

- `geom2.py`: curve geometry (squircle boxes, strokes, boolean ops).
- `glyphs2.py`: every glyph drawn from the font's parameters.
- `build.py`: the four fonts' parameters, metrics, kerning and OpenType
  features (`pnum`, `tnum`, `zero`; Black's `ss02` wordmark set and its
  A alternates `ss03`-`ss06`; Mono's `ss01`, `ss03`), the TrueType
  conversion and a validation pass.

Build (Python 3.11 or later):

```
python -m venv .venv
.venv/bin/pip install fonttools skia-pathops          # brotli too for WOFF2
.venv/bin/python tools/fonts/build.py                 # all four, validated
```

The output replaces the committed TTFs; commit them with a line in
`FONTLOG.txt` and the version (`VERSION` in `build.py`).
