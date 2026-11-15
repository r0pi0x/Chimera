# UGG fonts: sources

The build scripts of the Ultimate Gun Game's faces (UGG Display Black,
Bold, Regular; UGG Mono Regular), the OFL-1.1 sources of the TTFs in
`chimera/ui/fonts/` (licence: `OFL.txt` there and here;
history: `FONTLOG.txt` there). Our own work, Copyright 2026 Lucker Party
contributors.

- `geom2.py`: curve geometry (squircle boxes, strokes, boolean ops).
- `glyphs2.py`: every glyph drawn from the font's parameters.
- `lower.py`: the lower case (v0.5), the accented letters
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
UGG_FONTS_LOWER=0 UGG_FONTS_OUT=<dir> .venv/bin/python -I tools/fonts/build.py
                                                      # the old caps-only build
```

The default output replaces the committed TTFs; commit them with a line in
`FONTLOG.txt` and the version (`VERSION` in `build.py`). The default
build is the shipped one (v0.5): a-z map to the drawn lower case, and
the kit's roles that must stay in capitals (titles, labels, buttons, the
small tracked Mono labels) upper-case their own text (`TypeSpec::upper`
in the theme). `UGG_FONTS_LOWER=0` maps every lower-case code point to
its capital instead (the v0.4 behaviour), for comparison only. The
faces are Chimera's: every app's UI text and Bevy's default font use
them (`chimera_ui::fonts::OWN`).
