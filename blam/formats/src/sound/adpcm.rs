//! The Xbox ADPCM decoder (specs/halo_ce/sounds.md, "Xbox ADPCM"): IMA
//! ADPCM in independent blocks of 64 samples per channel. A mono block is
//! 36 bytes (a 4-byte header, then 32 data bytes), a stereo block 72 (two
//! headers, then eight groups of four left and four right data bytes). A
//! header holds the block's first sample (an `i16` predictor), the step
//! index (clamped to 88) and a reserved byte; the other 63 samples come
//! from the codes, low nibble first, and the last nibble is padding that
//! is never decoded. Only whole blocks decode: a trailing remainder is
//! ignored.

/// Bytes of a mono block.
pub const MONO_BLOCK_BYTES: usize = 36;
/// Bytes of a stereo block.
pub const STEREO_BLOCK_BYTES: usize = 72;
/// Samples (frames) a block decodes to, per channel.
pub const BLOCK_SAMPLES: usize = 64;
/// The step table's last index.
pub const MAX_STEP_INDEX: u8 = 88;

/// The IMA ADPCM step sizes, indices 0..=88.
pub const STEP_TABLE: [i32; 89] = [
    7, 8, 9, 10, 11, 12, 13, 14, 16, 17, 19, 21, 23, 25, 28, 31, 34, 37, 41, 45, 50, 55, 60, 66, 73, 80, 88, 97, 107,
    118, 130, 143, 157, 173, 190, 209, 230, 253, 279, 307, 337, 371, 408, 449, 494, 544, 598, 658, 724, 796, 876, 963,
    1060, 1166, 1282, 1411, 1552, 1707, 1878, 2066, 2272, 2499, 2749, 3024, 3327, 3660, 4026, 4428, 4871, 5358, 5894,
    6484, 7132, 7845, 8630, 9493, 10442, 11487, 12635, 13899, 15289, 16818, 18500, 20350, 22385, 24623, 27086, 29794,
    32767,
];

/// The step index adjustment per code (`c` and `c + 8` are equal).
pub const INDEX_ADJUST: [i32; 16] = [-1, -1, -1, -1, 2, 4, 6, 8, -1, -1, -1, -1, 2, 4, 6, 8];

/// The frames `bytes` bytes of ADPCM hold: whole blocks only.
pub fn frame_count(bytes: usize, channels: usize) -> usize {
    let block = if channels == 2 {
        STEREO_BLOCK_BYTES
    } else {
        MONO_BLOCK_BYTES
    };
    bytes / block * BLOCK_SAMPLES
}

/// One channel's 64 samples from its header and 32 data bytes.
fn decode_channel(predictor: i16, step_index: u8, data: &[u8; 32]) -> [i16; BLOCK_SAMPLES] {
    let mut out = [0i16; BLOCK_SAMPLES];
    out[0] = predictor;
    let mut pred = i32::from(predictor);
    let mut idx = i32::from(step_index.min(MAX_STEP_INDEX));
    // Nibbles 0 to 62 are codes; nibble 63 is padding.
    for n in 0..BLOCK_SAMPLES - 1 {
        let byte = data[n / 2];
        let code = usize::from(if n % 2 == 0 { byte & 0xF } else { byte >> 4 });
        let step = STEP_TABLE[idx as usize];
        let mut diff = step >> 3;
        if code & 4 != 0 {
            diff += step;
        }
        if code & 2 != 0 {
            diff += step >> 1;
        }
        if code & 1 != 0 {
            diff += step >> 2;
        }
        pred = if code & 8 != 0 { pred - diff } else { pred + diff };
        pred = pred.clamp(-32768, 32767);
        idx = (idx + INDEX_ADJUST[code]).clamp(0, i32::from(MAX_STEP_INDEX));
        out[n + 1] = pred as i16;
    }
    out
}

fn header(h: &[u8]) -> (i16, u8) {
    (i16::from_le_bytes([h[0], h[1]]), h[2])
}

/// The 64 samples of one mono block.
pub fn decode_mono_block(block: &[u8; MONO_BLOCK_BYTES]) -> [i16; BLOCK_SAMPLES] {
    let (pred, idx) = header(&block[..4]);
    decode_channel(pred, idx, block[4..].try_into().expect("32 bytes"))
}

/// The 64 frames of one stereo block, as `[left, right]`.
pub fn decode_stereo_block(block: &[u8; STEREO_BLOCK_BYTES]) -> [[i16; BLOCK_SAMPLES]; 2] {
    let mut data = [[0u8; 32]; 2];
    for g in 0..8 {
        let at = 8 + 8 * g;
        data[0][4 * g..4 * g + 4].copy_from_slice(&block[at..at + 4]);
        data[1][4 * g..4 * g + 4].copy_from_slice(&block[at + 4..at + 8]);
    }
    let (lp, li) = header(&block[0..4]);
    let (rp, ri) = header(&block[4..8]);
    [decode_channel(lp, li, &data[0]), decode_channel(rp, ri, &data[1])]
}

/// Decode mono blocks to samples (64 per block); a remainder shorter than
/// a block is ignored.
pub fn decode_mono(bytes: &[u8]) -> Vec<i16> {
    let mut out = Vec::with_capacity(frame_count(bytes.len(), 1));
    for b in bytes.as_chunks::<MONO_BLOCK_BYTES>().0 {
        out.extend_from_slice(&decode_mono_block(b));
    }
    out
}

/// Decode stereo blocks to interleaved samples (left, right, left, ...;
/// 128 per block); a remainder shorter than a block is ignored.
pub fn decode_stereo(bytes: &[u8]) -> Vec<i16> {
    let mut out = Vec::with_capacity(frame_count(bytes.len(), 2) * 2);
    for b in bytes.as_chunks::<STEREO_BLOCK_BYTES>().0 {
        let [l, r] = decode_stereo_block(b);
        for i in 0..BLOCK_SAMPLES {
            out.push(l[i]);
            out.push(r[i]);
        }
    }
    out
}

/// Decode `channels` (1 or 2) channels of blocks: samples interleaved.
pub fn decode(bytes: &[u8], channels: usize) -> Vec<i16> {
    if channels == 2 {
        decode_stereo(bytes)
    } else {
        decode_mono(bytes)
    }
}
