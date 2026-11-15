# TF2: the ICE block cipher (encrypted script files)

Source basis: the designer's published paper on the cipher (Fast Software
Encryption workshop, 1997), the designer's public web pages describing the
algorithm, its variants and its certification triplets, and the slides of
the paper's presentation; for the game's use of it, public community
documentation of the engine's encrypted script files. No implementation
of the cipher and no game or engine code was read.
Status: draft. **A model written from this description does not yet
reproduce the paper's certification triplets** (Open question 1). Every
other check the paper makes possible passes; see Test cases.

## Summary

ICE is a 64-bit Feistel block cipher in the style of DES. Its round
function expands a 32-bit half into four 10-bit values, swaps bits between
them under the control of 20 key bits ("keyed permutation", like the salt
of Unix crypt), XORs in 40 more key bits, looks the result up in four
10-to-8-bit S-boxes built from exponentiation in GF(2^8), and permutes the
32 output bits. The key schedule uses 60 key bits per round, each key bit
taken one at a time from four rotating 16-bit registers and inverted after
every use. The engine's encrypted script files (`.ctx`) are KeyValues text
encrypted block by block with an 8-byte key the user supplies locally.

## Units and conventions

- Bits are numbered from the right starting at 0, as in the paper: in a
  32-bit half, bit 0 is the least significant and bit 31 the most
  significant. In an n-bit value, bit n−1 is the leftmost.
- A 64-bit block is held as two 32-bit halves, L (left) and R (right).
- Byte order (assumed big-endian throughout; see Open question 2): a block
  is 8 bytes b0..b7; L is b0 b1 b2 b3 with b0 most significant, R is
  b4 b5 b6 b7 with b4 most significant. Output is written back the same way.
  Hex strings in the test vectors are those bytes in order.
- A 64-bit key chunk is 8 bytes k0..k7 read as one big-endian 64-bit number
  with key bit 63 the top bit of k0 and key bit 0 the bottom bit of k7.
- "XOR" is bitwise exclusive or. "¬b" is the complement of a bit.
- There is no notion of time: this is a pure function of key and data.

## Constants

### Variants

| Variant (ours) | Level | Key size | Rounds | Notes |
|---|---|---|---|---|
| Thin-ICE | 0 | 64 bits (8 bytes) | 8 | the first 8 rounds of the level-1 schedule |
| ICE | 1 | 64 bits (8 bytes) | 16 | the standard cipher |
| ICE-n | n ≥ 1 | 64·n bits (8·n bytes) | 16·n | schedule nested from n level-1 schedules |

Block size is 64 bits (8 bytes) at every level. Each round uses 60 subkey
bits, kept as three 20-bit values SK1, SK2, SK3.

### Expansion E (32-bit half P → four 10-bit values)

Each value is listed from its bit 9 (leftmost) down to its bit 0.

| Value | Bits 9 … 0, as bits of P |
|---|---|
| E1 | P1 P0 P31 P30 P29 P28 P27 P26 P25 P24 |
| E2 | P25 P24 P23 P22 P21 P20 P19 P18 P17 P16 |
| E3 | P17 P16 P15 P14 P13 P12 P11 P10 P9 P8 |
| E4 | P9 P8 P7 P6 P5 P4 P3 P2 P1 P0 |

Bits 0, 1, 8, 9, 16, 17, 24, 25 of P each feed two neighbouring S-boxes.
It is convenient to name two 20-bit values: A = E1 in bits 19..10 and E2
in bits 9..0; B = E3 in bits 19..10 and E4 in bits 9..0.

### S-box XOR offsets O (hexadecimal), by S-box and row

| S-box | Row 0 | Row 1 | Row 2 | Row 3 |
|---|---|---|---|---|
| S1 | 83 | 85 | 9b | cd |
| S2 | cc | a7 | ad | 41 |
| S3 | 4b | 2e | d4 | 33 |
| S4 | ea | cd | 2e | 04 |

### S-box moduli M (decimal; degree-8 irreducible polynomials over GF(2)), by S-box and row

| S-box | Row 0 | Row 1 | Row 2 | Row 3 |
|---|---|---|---|---|
| S1 | 333 | 313 | 505 | 369 |
| S2 | 379 | 375 | 319 | 391 |
| S3 | 361 | 445 | 451 | 397 |
| S4 | 397 | 425 | 395 | 505 |

Read each number in binary as a polynomial: bit k is the coefficient of
x^k. All sixteen have bit 8 set (256 to 511) and are irreducible; for
example 333 = 1 0100 1101 in binary = x^8 + x^6 + x^3 + x^2 + 1. (There
are exactly 30 irreducible polynomials of degree 8; the paper's design
search ran over those 30.)

### P-box (32 S-box output bits → 32-bit F output)

"Si.j" means bit j of S-box i's 8-bit output (bit 7 its most significant).

| Output bit | 31 | 30 | 29 | 28 | 27 | 26 | 25 | 24 | 23 | 22 | 21 | 20 | 19 | 18 | 17 | 16 |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| Source | S1.7 | S4.7 | S3.7 | S2.7 | S2.6 | S3.6 | S1.6 | S4.6 | S3.5 | S2.5 | S4.5 | S1.5 | S4.4 | S1.4 | S2.4 | S3.4 |

| Output bit | 15 | 14 | 13 | 12 | 11 | 10 | 9 | 8 | 7 | 6 | 5 | 4 | 3 | 2 | 1 | 0 |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| Source | S2.3 | S3.3 | S4.3 | S1.3 | S1.2 | S4.2 | S2.2 | S3.2 | S4.1 | S1.1 | S3.1 | S2.1 | S3.0 | S2.0 | S1.0 | S4.0 |

Properties the table has (useful as a transcription check): every S-box
bit appears exactly once; the eight bits of each S-box land in eight
different 4-bit groups of the output; output bits 16 apart never come from
the same S-box nor from S1/S3 or S2/S4.

### Key rotations KR (level-1 schedule, by round)

| Round | 1 | 2 | 3 | 4 | 5 | 6 | 7 | 8 | 9 | 10 | 11 | 12 | 13 | 14 | 15 | 16 |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| KR | 0 | 1 | 2 | 3 | 2 | 1 | 3 | 0 | 1 | 3 | 2 | 0 | 3 | 1 | 0 | 2 |

## Behavior

### Block cipher structure

Inputs: an 8-byte block and the per-round subkeys (SK1, SK2, SK3) for
rounds 1..N, N = 8 (level 0) or 16·n (level n).

Load L and R from the block (Units). For each round r = 1..N in turn:

```math
L \leftarrow L \oplus F(R, \mathrm{SK}_r)
```

then exchange L and R, except after the last round, where the exchange is
left out. The output block is L followed by R, stored as loaded.

Decryption is the same procedure with the subkeys used in reverse order
(round r uses the subkeys of round N+1−r). Nothing else changes.

### The round function F

Input: a 32-bit value P and one round's SK1, SK2, SK3 (20 bits each).
Output: a 32-bit value.

1. **Expansion.** Form E1..E4, and from them A and B, as in the expansion
   table.
2. **Keyed permutation.** For every bit position k = 0..19 where SK3 has a
   1, exchange bit k of A with bit k of B. So SK3 bit 19 exchanges bit 9 of
   E1 with bit 9 of E3; SK3 bit 10 exchanges bit 0 of E1 with bit 0 of E3;
   SK3 bit 9 exchanges bit 9 of E2 with bit 9 of E4; SK3 bit 0 exchanges
   bit 0 of E2 with bit 0 of E4. As a formula, with D = SK3 AND (A XOR B):

   ```math
   A' = A \oplus D, \qquad B' = B \oplus D
   ```

3. **Key mixing.** A'' = A' XOR SK1; B'' = B' XOR SK2. (SK1 feeds S1 and
   S2, SK2 feeds S3 and S4.) The paper is explicit that the keyed
   permutation comes before the XOR.
4. **S-boxes.** S1 takes bits 19..10 of A'', S2 bits 9..0 of A'', S3 bits
   19..10 of B'', S4 bits 9..0 of B''. Each gives 8 bits.
5. **P-box.** Place the 32 S-box output bits into the result as the P-box
   table says.

### S-box contents

Each S-box i (1..4) maps a 10-bit input X (bits X9..X0) to 8 bits:

- Row R = 2·X9 + X0 (the outermost two bits, X9 the high one), 0..3.
- Column C = the 8 bits X8..X1 as a number (X8 the high bit), 0..255.
- Output:

```math
S_i(X) = \left(C \oplus O_{i,R}\right)^{7} \bmod M_{i,R}
```

computed in GF(2^8) defined by the modulus M(i,R). Concretely: treat bytes
as polynomials over GF(2) of degree < 8 (bit k = coefficient of x^k).
Multiplication is carry-less polynomial multiplication followed by the
remainder on division by M(i,R); addition is XOR. Raise to the 7th power by
multiplying the value by itself (any order of multiplications gives the
same result; 0 maps to 0, 1 maps to 1). Because gcd(7, 255) = 1, every row
is a permutation of 0..255.

```math
a \otimes b = \Big(\sum_{j:\,b_j=1} a\,x^{j}\Big) \bmod M, \qquad y^{7} = y\otimes y\otimes y\otimes y\otimes y\otimes y\otimes y
```

Implementations may precompute each S-box as a 1024-entry table and may
fold the P-box into it (each entry then being the 32-bit word with that
S-box's 8 bits already in their output positions, so F is the OR of four
lookups); the result is identical.

### Level-1 key schedule (64-bit key → 16 rounds)

Inputs: one 64-bit key chunk (bits K63..K0). Output: SK1, SK2, SK3 for
rounds 1..16.

1. Split the key into four 16-bit registers: KB0 = K63..K48,
   KB1 = K47..K32, KB2 = K31..K16, KB3 = K15..K0 (the first-named bit is
   each register's bit 15).
2. For each round n = 1..16, start SK1, SK2, SK3 at zero and make 60 draws
   numbered d = 0..59. Draw d:
   - uses register KB[(d mod 4 + KR(n)) mod 4] (so each group of four
     consecutive draws takes one bit from each register, starting at
     register KR(n));
   - takes that register's bit 0 as the key bit b;
   - shifts that register right by one and puts ¬b into its bit 15 (a
     16-bit rotate right that inverts the bit going round);
   - shifts the receiving subkey register left by one and puts b into its
     bit 0 (so a subkey's first-drawn bit ends up as its bit 19 and its
     last-drawn bit as its bit 0).
3. Which subkey receives draw d: the paper's wording ("for SK = SK1, SK2,
   SK3 in turn, 5 times each", with four draws inside) admits two
   readings:
   - **(a) blocked**: draws 0–19 go to SK1, 20–39 to SK2, 40–59 to SK3;
   - **(b) cycled**: draws are taken in groups of four; groups go to SK1,
     SK2, SK3, SK1, SK2, SK3, … (group g of 15 to subkey g mod 3), five
     groups per subkey.

   Both readings give every key bit 15 uses in 16 rounds, each in a
   different one of the 15 4-bit groups of the 60 round bits, as the paper
   requires. Only reading (b) meets the paper's Thin-ICE criterion that in
   the first 8 rounds each key bit lands in each of the three 20-bit
   subkeys 2 or 3 times (reading (a) gives counts from 0 to 5). Take (b)
   unless the certification triplets say otherwise (Open question 1).

Every subkey bit is one key bit, possibly inverted: a key bit is used as is
the first time and complemented on each later use, alternating. Registers
carry their state from one round to the next; they are never reloaded.

The paper's text says the inverted bit goes into "bit 19" of the 16-bit
register; that is read here as a misprint for bit 15 (a 16-bit register
has no bit 19). Open question 1 lists this among the things checked.

### Thin-ICE (level 0)

Build the level-1 schedule from the 8-byte key and keep rounds 1..8 only.
Encrypt and decrypt with those 8 rounds.

### ICE-n (level n ≥ 2)

The key is n chunks of 8 bytes, chunk 1 first. Start from an empty list of
rounds. For chunk c = 1..n in turn: build the 16-round level-1 schedule of
chunk c, cut the list built so far at its half-way point, and insert the
16 new rounds there. The first chunk ends up outermost and the last chunk
contiguous in the middle. For ICE-3 (48 rounds) the paper's slides give:

| Rounds | Subkeys taken from |
|---|---|
| 1–8 | chunk 1 (rounds 1–8 of its schedule) |
| 9–16 | chunk 2 (rounds 1–8) |
| 17–32 | chunk 3 (rounds 1–16) |
| 33–40 | chunk 2 (rounds 9–16) |
| 41–48 | chunk 1 (rounds 9–16) |

ICE-2 is therefore: chunk 1 rounds 1–8, chunk 2 rounds 1–16, chunk 1
rounds 9–16. ICE-1 is the plain 16-round schedule. ("Chunk 1 = the first 8
key bytes" is assumed; the slides number key bits without saying which
end is bit 0. The ICE-2 triplet settles it once Open question 1 is
resolved.)

### TF2's encrypted script files (`.ctx`)

- The engine can read certain script files (weapon scripts, under
  `scripts/`) from a `.ctx` file instead of the plain `.txt`; public
  documentation says that once encrypted files exist for a category, the
  engine reads only those for it.
- The key is 8 bytes: an 8-character ASCII string, byte i = code of
  character i. It is not written in this spec or anywhere in the
  repository; the user supplies it locally (local config), as with other
  keys.
- Variant: the 8-byte key fits level 0 (Thin-ICE) and level 1 alike. The
  game is understood to use **level 0 (Thin-ICE, 8 rounds)**; confirm with
  a real file (Open question 3).
- Mode: plain ECB. No header, no IV, no padding, no length field, no
  checksum. Bytes 8k..8k+7 of the file decrypt independently as one block,
  for k = 0, 1, … while a full block remains.
- A trailing remainder of 1–7 bytes (file length not a multiple of 8) is
  not encrypted: it is copied through unchanged on both encryption and
  decryption.
- The plaintext is Valve KeyValues text (the same text a `.txt` script
  would hold): quoted keys and values, braces, `//` comments; usually
  ASCII/UTF-8 with no byte-order mark.

## Per-block order

1. (Once per key) Build the round subkeys for the level: level-1 schedule
   per chunk, nested for ICE-n, truncated to 8 rounds for Thin-ICE. For
   decryption, reverse their order.
2. Load L, R from the 8 bytes, big-endian.
3. For each round: expand R, apply the keyed permutation with SK3, XOR SK1
   and SK2, S-box lookups, P-box, XOR into L, exchange halves (not after the
   last round).
4. Store L, R big-endian.

For a `.ctx` file: repeat steps 2–4 for each whole 8-byte block in file
order, copy the 0–7 trailing bytes as they are.

## Edge cases

- Empty file, or a file of 1–7 bytes: output equals input.
- The key schedule depends only on the key; build it once per key.
- Level 0 and level 1 take the same 8-byte key but give different
  ciphertexts; the level must be chosen, not inferred from the key length.
- Key shorter than 8·n bytes for level n: not defined by the cipher;
  reject. (The engine's keys are always exactly 8 characters.)
- Decrypting with the wrong key or level produces bytes that are not text;
  a decoder can detect this (e.g. NUL or most bytes outside printable
  ASCII and whitespace) and report a key error rather than parse garbage.
- An all-zero key does not give all-zero subkeys (the inversions see to
  it).

## Quirks

- The trailing partial block is left in the clear. A file's last up to 7
  bytes (usually a closing brace and line endings) are readable without the
  key. Keep: the files are made that way.
- Thin-ICE is known to be weak (published differential attacks), so the
  encryption is obfuscation, not protection. Irrelevant to us; we only
  decrypt the user's own files.

## Test cases

Hex strings are bytes in order.

| # | Setup | Input | After | Expected |
|---|---|---|---|---|
| 1 | ICE (level 1), key `deadbeef01234567` | encrypt `fedcba9876543210` | 16 rounds | `7d6ef1ef30d47a96` (paper's certification triplet) |
| 2 | Thin-ICE (level 0), key `deadbeef01234567` | encrypt `fedcba9876543210` | 8 rounds | `de240d83a00a9cc0` (paper's triplet) |
| 3 | ICE-2, key `00112233445566778899aabbccddeeff` | encrypt `fedcba9876543210` | 32 rounds | `f94840d86972f21c` (paper's triplet) |
| 4 | each of 1–3 | decrypt the expected ciphertext | — | the plaintext `fedcba9876543210` |
| 5 | any level, any key, 1000 random blocks | decrypt(encrypt(x)) | — | x for every block |
| 6 | GF arithmetic, modulus 333 | 2^7 ; 3^7 ; 0^7 ; 1^7 | — | 0x80 ; 0xff ; 0x00 ; 0x01 |
| 7 | each of the 16 moduli | the map y → y^7 on 0..255 | — | a permutation of 0..255; for every nonzero input difference no output difference occurs more than 6 times out of 256 |
| 8 | each S-box, input differences confined to bits X7..X2 | count, over all 1024 inputs, how often each (input difference, output difference) pair occurs | — | largest count 18; pairs reaching 18: S1 4, S2 5, S3 6, S4 7 (22 in all, the paper's "22 one-round characteristics with probability 18/1024") |
| 9 | S-boxes + P-box, independent uniform S-box inputs | input difference `008c008c` (S2 and S4 each get 10-bit difference 0x08c) | one F | output difference `4042a085` with S2 and S4 each succeeding 18 of 1024 times (324/2^20, as the paper states); same for `00dc00dc` → `5920a681` |
| 10 | F with SK1 = SK2 = SK3 = 0 | all 2^32 inputs x | — | F(x) is never 0 and never equals x (paper's selection criteria; holds for this spec's row/offset reading and for none of 15 alternative readings tried) |
| 11 | S-box contents (derived from this spec, not from the reference) | S1, S2, S3, S4 at inputs 0x000, 0x001, 0x200, 0x201, 0x3ff | — | S1: 17 1f 82 95 7a; S2: 11 43 ab 2b e3; S3: cf 2b 81 1d df; S4: bc 24 e6 99 92 (hex) |
| 12 | F (derived from this spec), SK1 = SK2 = SK3 = 0 | x = 0 ; 1 ; 0x80000000 ; 0xffffffff ; 0x12345678 | — | 642e6d6e ; a6365d6e ; 643e656e ; 7e5d51fc ; 45a8bb84 (hex) |
| 13 | F (derived), x = 0x12345678 | SK1 = 0xfffff, others 0 ; SK3 = 0xfffff, others 0 | — | d7eaa196 ; 5573b477 |
| 14 | keyed permutation | any x with equal 16-bit halves (x = h·2^16 + h), any SK3 | — | F(x) does not depend on SK3 (A = B for such x) |
| 15 | level-1 schedule, symbolic | track which key bit (and whether inverted) lands in each of the 960 subkey bits | 16 rounds | every key bit used exactly 15 times, once in each of the 15 4-bit groups of the 60 round bits; uses alternate plain, inverted, plain, … |
| 16 | `.ctx` framing, any key | 19-byte input | encrypt | bytes 0–15 are two ECB blocks; bytes 16–18 unchanged |
| 17 | `.ctx` framing | 5-byte input | encrypt or decrypt | output equals input |
| 18 | real `.ctx` from the user's install, user's key, level 0 | decrypt | — | printable text that parses as KeyValues; re-encrypting gives the original file byte for byte |

Cases 11–13 pin down this spec's reading, not the reference; if case 1
fails they may change with the fix. Cases 1–3 are the arbiter.

## Open questions

1. **The certification triplets are not reproduced.** A throwaway model
   written from this description (in a scratch area, not kept) gives
   `bdbb718633e2ebfd` for case 1 with reading (a) and `dfe0139a21555fb0`
   with reading (b), not `7d6ef1ef30d47a96` (case 2: `f320ec3a74ce3bab`
   and `689214b8f6b2eebc`, not `de240d83a00a9cc0`). What is confirmed: the
   S-box polynomial arithmetic and the S-box/P-box structure (cases 7–9
   reproduce the paper's own analysis numbers exactly), and the row and
   offset reading (case 10 holds for it alone among 16 readings). Also
   checked without success, in every combination: both subkey-fill
   readings and a bit-by-bit cycled fill; every assignment of the three
   generated 20-bit values to the roles SK1/SK2/SK3; bits drawn from the
   top instead of the bottom of the registers; inverted bit put back at
   bit 15 vs 16–20 or 31; inversion on or off; subkey bits accumulated
   from the other end or inverted; register index (d + KR) vs (d − KR) and
   cumulative rotations; all 24 orders of the four key registers, each with
   the bytes inside a register swapped or not; little-endian key, block or
   halves; halves exchanged at load or store; subkeys in reverse order;
   the subkey XOR before the keyed permutation; the offset XOR after the
   exponentiation; bit-reversed S-box column or output; the offset and
   modulus tables transposed; the P-box read inverted. None hits. So
   something in the key schedule (most likely) or the data loading differs
   from the paper's description in a way not listed here. The paper and
   the designer's page both say the source code is the better reference
   for the key schedule, which we may not read. Ways forward, in order:
   (a) an implementation session codes this spec with every listed
   alternative as a switch and also tries case 18 (a real file and the
   user's key) as an oracle: readable KeyValues text proves a combination
   right independently of the triplets; (b) a separate spec session reads
   further published prose on ICE (e.g. the 1998 FSE differential
   cryptanalysis paper on ICE, which restates the cipher) and amends this
   spec.
2. Byte order of block and key: big-endian is assumed (the paper's
   triplets are written as hex strings of the bytes; the paper never states
   the packing). Settled once case 1 passes.
3. Level used by the game's `.ctx` files: understood to be level 0
   (Thin-ICE) with an 8-byte key; not confirmed from public documentation
   in this session (the engine wiki was unreachable). Check with case 18:
   level 0 vs level 1 decryption of a real file, whichever gives text.
4. Whether decrypted `.ctx` text can carry trailing padding (NUL or spaces)
   before the clear trailing bytes, and whether some files have a UTF-8
   byte-order mark. Check on real files; the KeyValues reader should
   tolerate both.
5. The paper's other selection figures do not match this reading exactly
   (sum of popcount(F(x) XOR x) over the 65,536 inputs with equal halves:
   1,049,600 here vs 1,048,576 claimed; popcount sum of F over the 32
   single-bit inputs: 523 here vs 506 claimed; several symmetric
   characteristic probabilities over 2^40 differ). The paper itself says
   its analysis software was flawed, so these are not used as tests.
