# Halo: Combat Evolved: disc image

Source basis: the user's own Xbox disc image of the game (a trimmed,
single-partition image) read with throwaway scripts, every file compared
against an extracted copy of the same disc; plus the publicly documented
Xbox DVD file system layout (called XDVDFS or XISO) for the parts one
image cannot show. No game behaviour reference was used.
Status: draft

## Summary

The disc is a read-only file system of 2048-byte sectors. One volume
descriptor at sector 32 names the root directory. A directory is a table
of variable-length entries linked as a binary search tree (case-folded
names); a file is a run of consecutive sectors. A reader needs only the
sector size, the volume descriptor, the tree walk and a base offset for
the partition. Everything here is little-endian.

## Units and conventions

- Sector: 2048 bytes. All "sector" numbers count from the start of the
  game partition, not from the start of the image file.
- Integers are little-endian: u16, u32, u64 as named.
- "Partition base": the byte offset in the image file where the game
  partition's sector 0 starts. Byte position of sector n is
  base + n * 2048.
- Names are 8-bit; on this disc every name is plain ASCII.
- Paths are written here with "/" between components.

## Constants

| Name (ours) | Value | Unit | Meaning |
|---|---|---|---|
| SECTOR_SIZE | 2048 | bytes | the only sector size |
| VOLUME_SECTOR | 32 | sector | where the volume descriptor lives |
| VOLUME_MAGIC | the 20 ASCII characters MICROSOFT*XBOX*MEDIA | text | at both ends of the descriptor |
| ENTRY_HEADER | 14 | bytes | fixed part of a directory entry |
| ENTRY_ALIGN | 4 | bytes | entries start on multiples of this |
| END_FILL | 0xFF | byte | padding value |
| NO_NODE | 0 | offset | a subtree link of 0 means "none" |
| ATTR_DIRECTORY | 0x10 | bit | entry is a directory |
| ATTR_ARCHIVE | 0x20 | bit | what ordinary files carry on this disc |
| MAX_NAME | 255 | bytes | most a one-byte length can hold |

## Volume descriptor

At partition base + 32 * 2048, one sector of 2048 bytes.

| Offset | Size | Type | Meaning |
|---|---|---|---|
| 0x000 | 20 | text | the magic text |
| 0x014 | 4 | u32 | sector of the root directory table |
| 0x018 | 4 | u32 | size in bytes of the root directory table |
| 0x01C | 8 | u64 | creation time, 100 ns ticks since 1601-01-01 UTC |
| 0x024 | 1992 | bytes | all zero |
| 0x7EC | 20 | text | the magic text again |

This disc: root sector 264, root size 88 bytes, time bytes e0 87 77 05
52 c1 01 (read as a number: 2001-10-15 08:21:50 UTC), the 1992 bytes all
zero. A reader must require both magics. The time is informational; a
reader may expose it or ignore it. Nothing else in the sector is used.

Sectors 0 to 31 of this image are not zero (they hold a platform
header the reader never needs); sectors 33 to 263 are zero. A reader does
not look at either range.

## Where the partition starts

A reader is given an image file and finds the base by probing: for each
candidate base, read 20 bytes at base + 0x10000 and keep the first
candidate whose bytes equal the magic text; then also require the closing
magic at base + 0x10000 + 0x7EC. Candidates, in this order:

| Base (hex) | Kind of image | Status |
|---|---|---|
| 0x00000000 | trimmed, single-partition image (what this disc is) | verified on this disc |
| 0x18300000 | full dump of an original-hardware-format disc | public knowledge, not verified here |
| 0x0FD90000 | full dump of the second-generation disc format | public knowledge, not verified here |
| 0x02090000 | full dump of the third-generation disc format | public knowledge, not verified here |
| 0x89D80000 | alternate third-generation layout | public knowledge, weakest, not verified |

Probing the unlisted bases on this image found no magic, as expected
(the image has the magic at 0x10000 only). A full dump places a
video-area partition before the game partition, which is why its base is
non-zero. If no candidate matches, refuse the image with a clear error
(not a disc, or an unsupported container). Only 2048-byte-sector
images are in scope; images with 2352-byte raw sectors, or compressed or
scrambled containers, are not supported and are not detected beyond the
probe failing.

## Directory tables

A directory is a table of its entries stored contiguously from its start
sector for its size in bytes (the size is in the entry that names it, or
in the volume descriptor for the root). Entries are not sorted in the
table; they form a binary search tree through offsets.

Entry layout, all fields from the entry's own first byte:

| Offset | Size | Type | Meaning |
|---|---|---|---|
| 0 | 2 | u16 | left subtree: offset of the left child entry, in units of 4 bytes from the table start; 0 means none |
| 2 | 2 | u16 | right subtree: same unit; 0 means none |
| 4 | 4 | u32 | start sector of the file data, or of the subdirectory's table |
| 8 | 4 | u32 | size in bytes of the file, or of the subdirectory's table |
| 12 | 1 | byte | attributes |
| 13 | 1 | byte | name length n in bytes (not counting any terminator; none is stored) |
| 14 | n | bytes | the name |

The entry occupies 14 + n bytes, followed by 0xFF bytes up to the next
multiple of 4. The next entry in the table starts at that boundary.
Measured: the fill after a name was always 0xFF.

- The first entry of the table (offset 0) is the root of that
  directory's tree. Links are offsets, so the child of the root can sit
  anywhere later in the table.
- The tree is ordered by the comparison below. Smaller names are
  reached through the left link, larger through the right.
- A table's bytes that belong to no entry are 0xFF: the tail of a
  sector after the last entry that fits, and the rest of the table.
  No entry straddles a sector boundary (verified over all 58 tables: no
  entry crosses a 2048-byte boundary). When the next entry would not fit
  in what is left of a sector, the writer fills to the sector end and
  starts it at the next sector's first byte.
- A table's size is usually a whole number of sectors. The root table
  here is 88 bytes, not a multiple of 2048; the rest of its sector is
  0xFF. A reader must therefore use the size field as the limit and not
  round it; it may read the whole sector, but entries beyond the size
  are not part of the directory.
- A reader's end test for a missing link is the link value 0 (a child
  can never sit at offset 0, since that is the root). Do not rely on a
  0xFFFF terminator in the links; an entry whose first 4 bytes and start
  sector field are all 0xFF (the fill) means "no entry here" and a reader
  must treat reaching it as a corrupt table.

### Attribute byte

| Bit | Value | Meaning |
|---|---|---|
| 0 | 0x01 | read-only |
| 1 | 0x02 | hidden |
| 2 | 0x04 | system |
| 4 | 0x10 | directory |
| 5 | 0x20 | archive |
| 7 | 0x80 | normal (public knowledge; no entry here uses it) |

On this disc only two values occur: 0x10 on the 57 directories and 0x20
on the 1166 files. A reader decides "directory or file" by bit 0x10
alone and ignores the rest.

### Name ordering and lookup

The tree is ordered by comparing names byte by byte after folding
ASCII lower case letters (a to z, 0x61 to 0x7A) to upper case (0x41 to
0x5A); no other byte changes. The first differing folded byte decides;
if one folded name is a prefix of the other, the shorter is smaller.

The upper-case direction matters. The characters between Z and a in
ASCII (square brackets, backslash, caret, underscore and the back quote)
compare greater than every letter under this rule, but smaller under a
lower-casing rule. This disc has 606 entries with underscores, and a
lower-casing comparison fails to find every name in 6 of the 58 tables,
while the upper-casing comparison finds every name in all 58. So a
reader that binary-searches must fold to upper case.

Lookup of a name in a table: start at offset 0, compare the folded
wanted name with the folded entry name; equal means found; smaller means
follow the left link; larger the right; a link of 0 means not found.
Verified: with this rule a search by every entry's name and by its
case-swapped name finds it, in every directory of the disc. No two names
in one directory fold to the same value (checked), so the first match
is the only match.

A reader that does not want to depend on the ordering may instead walk
the whole tree and compare folded names; both give the same result on a
well-formed disc. Walking the whole tree is also how to list a directory
(visit left, then the node, then the right: this gives folded-name
order).

### Subdirectories, empty directories, zero-length files

- A directory entry (attribute bit 0x10) gives the sector and byte size
  of its own table. Its table has the same layout, and may hold more
  directory entries, to any depth. The deepest path on this disc has 6
  components.
- Public knowledge, not seen on this disc: an empty directory is written
  with start sector 0 and size 0 and has no table; a zero-length file
  likewise has size 0 and an unimportant start sector. A reader must
  treat a directory of size 0 as having no entries without reading
  anything, and a file of size 0 as an empty byte string without a
  sector range check. This disc has neither (no empty directories, no
  zero-length files).

## File data

- A file's bytes are one contiguous run starting at the first byte of
  its start sector and lasting size bytes. Files start on sector
  boundaries; there is no other alignment. The last sector of a
  file is padded; on this disc the padding in a file's last sector was
  0xFF in every file checked (all 1166), but a reader never reads it.
- The size field is a u32: a file is at most 4 GiB - 1 byte. The
  largest file here is 200,614,260 bytes.
- Directory tables are interleaved with file data; no two files overlap
  (checked) and the files' sector order is not the tree order or
  the path order. The first sectors after the volume descriptor region
  hold the root table (264) and then the first file (265).
- Image length here is 1,820,736 sectors (3,728,867,328 bytes,
  an exact multiple of 2048); the highest file ends at sector
  1,820,724; the 12 sectors after it are zero.

## Path rules for a reader

- Separator: "/" or "\" in the caller's path; split into components.
  Ignore a leading separator, empty components and "." components.
  Refuse ".." (there is no parent link on disc).
- A stored name never contains a separator. A reader must refuse an
  entry whose name has "/" or "\" in it, whose length is 0, or whose
  bytes include 0x00; it may skip such an entry with a warning instead of
  failing the whole disc.
- Lookup is case-insensitive using the ASCII fold above. Non-ASCII bytes
  (0x80 and up) are compared as they are (no folding). Listing returns
  names as stored (the disc's own casing), decoded as ASCII, or as
  Latin-1 if any byte is above 0x7F.
- Every component but the last must name a directory; the last may name
  either. Asking for a file's bytes of a directory is an error, as is
  listing a file.
- Safety checks, each a refusal with an error, never a panic or an
  endless loop:
  - A link offset times 4 plus 14 must be inside the table size; the
    whole entry (14 + name length) must be inside the table size.
  - A tree walk must visit each offset at most once; a repeated offset
    is a loop. Cap the walk at table size / 4 nodes as well.
  - A start sector times 2048, plus the size (files) or the table size
    (directories), must not pass the image's length minus the base.
    Check in 64-bit arithmetic.
  - Directory nesting deeper than, say, 64 levels is refused (a
    directory table that leads back to an ancestor).
  - A sector or size that overflows 64-bit arithmetic is refused.
- A read of a file's bytes returns exactly size bytes; a short read from
  the image is an error.
- The listing can be built lazily per directory (the tables are small)
  or all at once at open time. The measured walk of the whole disc reads
  58 small tables.

## Measured facts of this disc

| Fact | Value |
|---|---|
| Partition base | 0 |
| Root table | sector 264, 88 bytes |
| Files | 1,166 |
| Directories (not counting the root) | 57 |
| Total file bytes | 3,726,832,117 (3,554.2 MiB as the user's folder shows) |
| Top-level entries | default.xbe (file), bink (directory), maps (directory), XDemos (directory) |
| Files at top level | 1 (default.xbe); bink has 5, maps has 24, XDemos has 1,136 |
| default.xbe | 2,535,424 bytes, start sector 265 |
| maps/bloodgulch.map | 22,571,008 bytes, start sector 835,909 |
| Largest file | bink/credits.bik, 200,614,260 bytes |
| Deepest path | XDemos/MunchTrial/Effects/health/Maps/clouds_dry_128_nobands.bmp (6 components) |
| Longest name | 34 bytes |
| Names with bytes above 0x7F | none |
| Tables of more than one sector | 2: XDemos/MunchTrial/Effects/Water (4096 bytes) and XDemos/MunchTrial/Oddio (20480 bytes) |
| Table sizes | 55 of 2048, 1 of 4096, 1 of 20480, root of 88 |
| Attribute values seen | 0x10 and 0x20 only |
| Empty directories, zero-length files | none |

The root tree is a small real example: the first entry (offset 0) is
default.xbe; its left link (7, byte 28) is bink and its right link (12,
byte 48) is maps; maps has a right link (17, byte 68) to XDemos. The root
table ends at byte 88.

The maps folder holds the 24 maps: a10, a30, a50, b30, b40, beavercreek,
bloodgulch, boardingaction, c10, c20, c40, carousel, chillout, d20, d40,
damnation, hangemhigh, longest, prisoner, putput, ratrace, sidewinder, ui,
wizard (each with the extension .map).

## Verification done

A lister written from this spec's rules walked the whole image: 1,166
files, 57 directories, no loops; every file's size and its SHA-1 hash
equal those of the extracted copy of the same disc (0 mismatches); every
entry satisfies the tree ordering under the upper-case fold; every table
byte outside an entry is 0xFF; no entry straddles a sector boundary; all
file ranges are inside the image and do not overlap.

## Per-tick order

Not applicable: the disc has no per-tick behaviour.

## Edge cases

- A name search on a name that is longer than 255 bytes cannot match;
  return not found.
- Two names that differ only by case cannot both exist in one directory
  (the tree would be ambiguous); a reader may assume that.
- A directory entry with size 0: no entries.
- A table whose size is not a multiple of 4 at its end: read entries by
  the offsets only; ignore trailing bytes.
- File start sector 0 with size 0: empty file (see above).

## Quirks

- Table entries are in tree order of insertion, not sorted; do not
  assume the first entry is the alphabetically first.
- The root table size is not sector-rounded (88), the others are.
- The tree comparison folds to upper case, not lower case; see above.
- File tail padding is 0xFF here but nothing may rely on it.

## Test cases

### (a) A synthetic image a unit test can build in memory

Image length: 39 sectors = 79,872 bytes, every byte zero unless said.
Partition base 0. Fill every unlisted byte of a directory table sector
with 0xFF.

Volume descriptor, sector 32 (byte 65,536):

| Offset in sector | Bytes |
|---|---|
| 0x000 | the 20 magic characters |
| 0x014 | u32 33 (root sector) |
| 0x018 | u32 68 (root size) |
| 0x01C | u64 0 |
| 0x024 .. 0x7EB | zero |
| 0x7EC | the 20 magic characters |

Root directory table, sector 33 (68 bytes used, the sector's other
1,980 bytes 0xFF). Three entries:

| Entry | Table offset | Left | Right | Start sector | Size | Attr | Name length | Name | Fill after name |
|---|---|---|---|---|---|---|---|---|---|
| 1 | byte 0 | 6 | 11 | 35 | 5 | 0x20 | 10 | readme.txt | none (24 bytes, already aligned) |
| 2 | byte 24 | 0 | 0 | 34 | 2048 | 0x10 | 4 | Maps | 2 bytes of 0xFF (18 + 2 = 20 bytes) |
| 3 | byte 44 | 0 | 0 | 36 | 3000 | 0x20 | 8 | Zeta.bin | none (22 + 2 = 24 bytes) |

Check of the unit: entry 2 is at byte 24 = offset 6 and entry 3 at
byte 44 = offset 11. Entry 3's fill is 2 bytes (22 rounds to 24); entry
1 is 14 + 10 = 24, already aligned; entry 2 is 14 + 4 = 18, rounded to
20. Table end 44 + 24 = 68. The tree: root entry 1 (folded README.TXT),
left child Maps (folded MAPS, smaller), right child Zeta.bin (folded
ZETA.BIN, larger).

Subdirectory table "Maps", sector 34 (2048 bytes, only the first entry
used, the rest 0xFF): one entry at byte 0: left 0, right 0, start sector
38, size 4, attribute 0x20, name length 7, name "A_B.map", then 3 bytes of
0xFF fill (14 + 7 = 21, rounded to 24).

File data: sector 35 starts with the 5 bytes of the ASCII text HELLO,
the rest of the sector zero. Sectors 36 and 37 hold the 3,000 bytes of
Zeta.bin where byte i equals i mod 251 (the rest of sector 37 zero).
Sector 38 starts with the 4 bytes 01 02 03 04, the rest zero.

Expected results:

- Listing of the root, in tree order: Maps (directory), readme.txt
  (file, 5), Zeta.bin (file, 3000). Count: 2 files and 1 directory at
  the top.
- Listing of "Maps": A_B.map (4).
- Recursive file list: Maps/A_B.map (4), readme.txt (5), Zeta.bin
  (3000); total bytes 3009.
- Read "readme.txt" gives 48 45 4C 4C 4F; read "README.TXT" and
  "/readme.txt" give the same; read "maps/a_b.MAP" gives 01 02 03 04;
  read "Maps\A_B.map" the same; read "zeta.BIN" gives 3000 bytes where
  byte 2999 equals 238 (2999 = 11 * 251 + 238, hex EE).
- Errors: "nothing.txt" is not found; "Maps" read as a file is an error;
  "readme.txt/x" (a file used as a directory) is an error; "Maps/.." is
  refused.
- Corruption cases to build by editing the image: (1) set entry 1's
  left link to 0: Maps becomes unreachable and the listing shows 2
  entries; this is merely a different tree and must not fail. (2) set
  Maps's left link to 11 and Zeta.bin's left link to 6 (a cycle): listing
  or lookup must return an error, not hang. (3) set a link to 40
  (byte 160, past the 68-byte size): error.
  (4) set Zeta.bin's start sector to 38: error because the range
  passes the image end (38 * 2048 + 3000 > image length). (5) Corrupt
  the closing magic: refuse the image. (6) Put the whole image at base
  0x18300000 in a longer buffer, with zeros before it: the probe finds
  the base and the same listing results.
- Ordering check with the underscore: in a directory holding the two
  names "A_" and "AZ" the upper-case fold orders AZ before A_
  ("Z" is 0x5A, "_" is 0x5F); a lower-case fold would order them the
  other way. A synthetic table holding those two, with AZ as the left
  child of A_, must have both found by lookup.

### (b) Disc-backed expectations (slow tests, skipped when the disc is
absent)

| Check | Expected |
|---|---|
| number of files | 1,166 |
| number of directories (not the root) | 57 |
| sum of file sizes | 3,726,832,117 bytes |
| image base found by the probe | 0 |
| top-level names | bink, default.xbe, maps, XDemos |
| size of default.xbe | 2,535,424 |
| first 4 bytes of default.xbe | 58 42 45 48 (the text XBEH) |
| size of maps/bloodgulch.map | 22,571,008 |
| first 16 bytes of maps/bloodgulch.map | 64 61 65 68 05 00 00 00 00 68 a4 02 31 06 00 00 |
| size of XDemos/MunchTrial/Layout/AI/Mechanics/doautosave.txt | 53 |
| its first 16 bytes | 73 63 72 69 70 74 20 64 6f 61 75 74 6f 73 61 76 (the text "script doautosav") |
| case-insensitive lookup | MAPS/BLOODGULCH.MAP finds the same file |
| largest file | bink/credits.bik, 200,614,260 bytes |
| number of maps in maps/ | 24 |

The first 4 bytes of bloodgulch are the header-magic marker of the map
file (see cache_file.md; the bytes read as text are "daeh").

## Confidence

| Section | Confidence |
|---|---|
| Sector size, volume descriptor, magic, root fields | checked against the disc |
| Partition base 0 for a trimmed image | checked against the disc |
| Other partition bases | read from public knowledge only; not verified; the 0x89D80000 and 0x02090000 values are the least sure |
| Entry layout, attribute byte, fill bytes, no straddling | checked against the disc (only attribute values 0x10 and 0x20 seen; other bits from public knowledge) |
| Name ordering rule (upper-case fold) | checked against the disc, with the lower-case alternative shown to fail |
| Empty directory and zero-length file conventions | public knowledge; not present on this disc |
| File data layout and limits | checked against the disc (4 GiB limit is from the u32 field) |
| Path and safety rules | our own design |
| Measured facts | checked against the disc |

## Open questions

- The exact partition bases for full-disc dumps are from public
  knowledge; the user has no such dump. A reader should log which
  candidate it picked. A second image would settle them.
- The conventions for empty directories and zero-length files have no
  example on this disc.
- Whether the real console's comparison treats non-ASCII bytes (0x80 and
  up) with any fold is unknown; no name here uses them.
- The 32 sectors before the volume descriptor (non-zero here) are not
  understood and not needed; whether any game logic wants them is
  assumed no.
- Dates in a directory entry: this format has none per file (checked:
  the entry has no time field); the volume's creation time is the only
  timestamp.
