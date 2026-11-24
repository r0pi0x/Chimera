# GoldSrc specs

File formats and engine behaviour shared by every game on Valve's first
engine (Counter-Strike 1.6, Half-Life, Half-Life Deathmatch, Day of
Defeat 1.3, Team Fortress Classic, Opposing Force), written in our own
words for the `goldsrc` loader family (crate `goldsrc_formats`) and the
games on it (`games/cs16` first). The plan is
[docs/plans/active/goldsrc.md](../../docs/plans/active/goldsrc.md); a
game's own behaviour (movement, guns, damage) lives in its own folder
(`specs/cs16/`). The rules for writing specs are in
[specs/README.md](../README.md).

## This family's rules for spec writers

- Sources: public format documentation and community write-ups, the
  installed files (read in place with throwaway scripts kept outside the
  repository), measurement on the dedicated server, and, for spec
  sessions only, Valve's Half-Life SDK. Never ReGameDLL_CS, ReHLDS,
  leaked or decompiled code.
- No code blocks, no structure, function or constant names from the SDK
  or any engine source. Field names in these specs are ours.
- **The install is the ground truth.** Every layout is checked against
  the files of `~/games/cs16_ds` (HLDS, app 90, `mod cstrike`, 2026-11-24:
  `cstrike/` and `valve/`); each spec says what was checked against
  which file and gives worked examples with the real numbers.
- One conventions source: [bsp.md](bsp.md) "Units and conventions"
  (little-endian, the inch, Z up). The others refer to it.

## Specs

| Spec | Covers | Status |
|---|---|---|
| [bsp.md](bsp.md) | BSP 30: header and lumps, every record, textures in the map, lightmaps, visibility, entities, contents, the clip hulls 0-3 (sizes, how planes and clip nodes describe solid space, hull traces), brush entities, texture name prefixes | draft |
| [wad3.md](wad3.md) | WAD3: header, directory, mip textures, `qpic` images, fonts, decals, transparency | draft |
| [mdl.md](mdl.md) | MDL v10: header, bones, controllers, hitboxes and hitgroups, sequences, groups and blends, the animation compression, events, attachments, body parts, meshes and triangle commands, textures and flags, external texture and sequence files | draft |
| [spr.md](spr.md) | SPR v2: header, orientation types, texture formats, palette, frames and groups; the HUD sprite lists | draft |
| [mount.md](mount.md) | the content folder chain (`liblist.gam`, `cstrike` then `valve`), loose files and case, where each kind of file lives, `materials.txt` | draft |
