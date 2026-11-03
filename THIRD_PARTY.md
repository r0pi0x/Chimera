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
