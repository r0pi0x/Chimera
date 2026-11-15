# UGG fonts: sources

The build scripts of the Ultimate Gun Game's faces (UGG Display Black,
Bold, Regular; UGG Mono Regular), the OFL-1.1 sources of the TTFs in
`apps/ultimate_gun_game/fonts/` (licence: `OFL.txt` there and here;
history: `FONTLOG.txt` there). Our own work, Copyright 2026 Lucker Party
contributors.

- `geom2.py`: curve geometry (squircle boxes, strokes, boolean ops).
- `glyphs2.py`: every glyph drawn from the font's parameters.
- `lower.py`: the lower case (a draft, v0.5), the accented letters
  (outlines composed from a base and a mark), the Latin-1 extras and the
  missing ASCII symbols.
- `build.py`: the four fonts' parameters, metrics, kerning and OpenType
  features (`pnum`, `tnum`, `zero`; Black's `ss02` wordmark set and its
  A alternates `ss03`-`ss06`; Mono's `ss01`, `ss03`), the TrueType
  conversion and a validation pass.

Build (Python 3.11 or later):

```
python -m venv .venv
.venv/bin/pip install fonttools skia-pathops          # brotli too for WOFF2
.venv/bin/python -I tools/fonts/build.py              # all four, validated
UGG_FONTS_LOWER=1 UGG_FONTS_OUT=<dir> .venv/bin/python -I tools/fonts/build.py
                                                      # the lower-case draft
```

The default output replaces the committed TTFs; commit them with a line in
`FONTLOG.txt` and the version (`VERSION` in `build.py`). The default
build is the shipped one: lower-case code points map to the capitals,
because the kit's text roles rely on that (`TypeSpec::upper` is off).
`UGG_FONTS_LOWER=1` maps them to the drawn lower case instead; it
replaces the shipped build once the roles that must stay in capitals
upper-case their own text.
