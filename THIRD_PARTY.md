# Third-party code

Code may enter this repository only from an approved permissive source
listed here (user decision, 2026-11-03). Everything else stays as before:
the Source SDK 2013, leaked or decompiled code (for example Combat Arms'),
and any non-permissive or unlisted code never enter the repo and are never
translated line by line; their behaviour reaches us only as specs written
in a separate session ([specs/README.md](specs/README.md)). Game assets and
keys never enter the repo.

Rules for using a listed source:

- A file that copies or adapts its code starts with a header comment naming
  the source, its licence and the original path, for example
  `// Adapted from hl2-rs (MIT, see THIRD_PARTY.md): crates/<crate>/src/<file>.rs`
  (or `Copied from` for an unchanged copy).
- Keep the copyright line and licence text below.
- Adding a source needs the user's OK. Then add an entry with every field
  below and, if the licence text is long, a `licenses/<name>.txt` file.
- `cargo test -p repo_checks` (`third_party_headers_name_listed_sources`,
  `third_party_entries_are_complete`) enforces this.

Entry format: a `## <name>` heading, then the fields `URL`, `Licence`,
`Copyright`, `Commit`, and either `Licence file` (a path under `licenses/`)
or the licence text in a fenced block.

## hl2-rs

- URL: https://github.com/kvalls/hl2-rs
- Licence: MIT
- Copyright: Copyright (c) 2026 HL2-RS contributors
- Commit: 304abfd367dfd2f36ede5d3b86dd94a250d9aa8f
- Approved: 2026-11-03 (user)

```text
MIT License

Copyright (c) 2026 HL2-RS contributors

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
```

## ICE

The designer's reference implementation of the ICE block cipher (ANSI C),
ported to Rust in `games/tf2/src/game/ice.rs` (TF2's encrypted `.ctx`
scripts). Taken: `ice.c`'s S-box constants (moduli, XOR offsets), P-box,
key rotation schedule, GF(2^8) arithmetic, S-box construction, round
function, key schedule (Thin-ICE, ICE, ICE-n) and block encrypt/decrypt.
The source files carry no licence or copyright line; the project's home
page states the public-domain dedication quoted below.

- URL: https://darkside.com.au/ice/index.html (source: https://darkside.com.au/ice/ice.c, https://darkside.com.au/ice/ice.h)
- Licence: public domain (dedication on the home page, quoted below)
- Copyright: none claimed ("There are no patents or copyrights"); written by Matthew Kwan, July 1996 ("Written by Matthew Kwan - July 1996", ice.c's header)
- Commit: none (not version-controlled); `ice.c` as fetched 2026-11-15, sha256 ecd05c5f778895aec9ff3433a5117382492417a938b988c7c053333664ceacf1 (the page dates its last source change 28 November 1998)
- Approved: 2026-11-15 (user)

```text
The ICE algorithm is public domain, and source code can be downloaded
from this page. There are no patents or copyrights, so its
use is unrestricted. Naturally, if you do decide to use it, the
author would love to hear about it.
```

## qbsp

A Rust reader for Quake-family BSP files, including GoldSrc's BSP 30.
Approved for the GoldSrc loader family (Counter-Strike 1.6, Half-Life
DM, DoD 1.3, ...; docs/plans/active/next-games.md, rank 2). **Not yet a
dependency**: nothing in the workspace uses it, and no code of it is
copied. When the GoldSrc plan adopts it, it enters as a crates.io
dependency (or adapted files with the header above), and this entry gets
the commit or version actually taken. The crate is dual-licensed; we
take it under MIT (its text below); Apache-2.0 is the alternative.

- URL: https://github.com/Noxmore/qbsp
- Licence: MIT OR Apache-2.0 (we use MIT)
- Copyright: Copyright (c) 2026 Noxmore
- Commit: 7c0b23e5152c87620765cd7499f5a377973dfb10 (main, 2026-10-09; crate version 0.17.0), reviewed, not yet taken
- Approved: 2026-11-23 (user)

```text
MIT License

Copyright (c) 2026 Noxmore

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
```
