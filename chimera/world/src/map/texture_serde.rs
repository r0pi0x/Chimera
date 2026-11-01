//! A texture as the converted-map cache keeps it (`MapTexture`'s serde,
//! `cache`): its BC blocks (`MapTexture::bc`) without the RGBA8 image and
//! mip levels decoded from them, which a read decodes again on its thread
//! (texpresso, as the converters decode them: `decode_level`). That was
//! three quarters of a whole de_dust2 entry. Whether `rgba8` and `mips`
//! are the blocks decoded is checked when the entry is written, each on
//! its own: a texture whose texels aren't (Combat Arms' box-filtered mips,
//! one changed after decoding) keeps them, so a read always gives the
//! bytes converted.

use serde::{Deserialize, Deserializer, Serialize, Serializer};

use super::{
    MapBc, MapBcFormat, MapTexture,
    pod_serde::{pod, pod_vecs},
};

/// `rgba8` is level 0 of `bc` decoded: not stored.
const RGBA8_DECODED: u8 = 1;
/// `mips` are the other levels of `bc` decoded: not stored.
const MIPS_DECODED: u8 = 2;

/// Level `level` of `bc` as RGBA8, the texture `width` x `height`; None
/// when `bc` has no such level or its size is wrong.
pub fn decode_level(bc: &MapBc, width: u32, height: u32, level: usize) -> Option<Vec<u8>> {
    let data = bc.levels.get(level)?;
    let (w, h) = ((width >> level).max(1), (height >> level).max(1));
    if data.len() != MapBc::level_bytes(bc.format, w, h) {
        return None;
    }
    let format = match bc.format {
        MapBcFormat::Bc1 => texpresso::Format::Bc1,
        MapBcFormat::Bc2 => texpresso::Format::Bc2,
        MapBcFormat::Bc3 => texpresso::Format::Bc3,
    };
    let mut out = vec![0; w as usize * h as usize * 4];
    format.decompress(data, w as usize, h as usize, &mut out);
    Some(out)
}

/// Which of `t`'s texels are its blocks decoded (`RGBA8_DECODED`,
/// `MIPS_DECODED`).
fn decoded(t: &MapTexture) -> u8 {
    let Some(bc) = &t.bc else { return 0 };
    let level = |i: usize| decode_level(bc, t.width, t.height, i);
    let mut flags = 0;
    if !t.rgba8.is_empty() && level(0).as_deref() == Some(&t.rgba8[..]) {
        flags |= RGBA8_DECODED;
    }
    if !t.mips.is_empty()
        && t.mips.len() + 1 == bc.levels.len()
        && t.mips.iter().enumerate().all(|(i, m)| level(i + 1).as_deref() == Some(&m[..]))
    {
        flags |= MIPS_DECODED;
    }
    flags
}

/// A texture as stored: the fields in order, the decoded texels left out.
#[derive(Serialize)]
struct Stored<'a> {
    name: &'a str,
    srgb: bool,
    width: u32,
    height: u32,
    flags: u8,
    #[serde(with = "pod_vecs")]
    mips: &'a [Vec<u8>],
    #[serde(with = "pod")]
    rgba8: &'a [u8],
    bc: &'a Option<MapBc>,
}

/// `Stored` read back.
#[derive(Deserialize)]
struct Read {
    name: String,
    srgb: bool,
    width: u32,
    height: u32,
    flags: u8,
    #[serde(with = "pod_vecs")]
    mips: Vec<Vec<u8>>,
    #[serde(with = "pod")]
    rgba8: Vec<u8>,
    bc: Option<MapBc>,
}

impl Serialize for MapTexture {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        let flags = decoded(self);
        Stored {
            name: &self.name,
            srgb: self.srgb,
            width: self.width,
            height: self.height,
            flags,
            mips: if flags & MIPS_DECODED != 0 { &[] } else { &self.mips },
            rgba8: if flags & RGBA8_DECODED != 0 { &[] } else { &self.rgba8 },
            bc: &self.bc,
        }
        .serialize(s)
    }
}

impl<'de> Deserialize<'de> for MapTexture {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let r = Read::deserialize(d)?;
        let bad = |what: &str| serde::de::Error::custom(format!("{}: {what} don't decode", r.name));
        if r.flags & !(RGBA8_DECODED | MIPS_DECODED) != 0 {
            return Err(serde::de::Error::custom(format!("{}: unknown flags {}", r.name, r.flags)));
        }
        let level = |i: usize| r.bc.as_ref().and_then(|bc| decode_level(bc, r.width, r.height, i));
        let rgba8 = if r.flags & RGBA8_DECODED != 0 {
            level(0).ok_or_else(|| bad("its blocks"))?
        } else {
            r.rgba8
        };
        let mips = if r.flags & MIPS_DECODED != 0 {
            let n = r.bc.as_ref().map_or(0, |bc| bc.levels.len());
            (1..n).map(level).collect::<Option<Vec<_>>>().ok_or_else(|| bad("its mip blocks"))?
        } else {
            r.mips
        };
        Ok(MapTexture {
            name: r.name,
            srgb: r.srgb,
            mips,
            width: r.width,
            height: r.height,
            rgba8,
            bc: r.bc,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// An 8 x 8 BC1 texture with its three levels, its texels decoded.
    fn texture() -> MapTexture {
        let block = |c: u16| {
            let mut b = c.to_le_bytes().to_vec();
            b.extend(0u16.to_le_bytes());
            b.extend([0b0001_1011; 4]);
            b
        };
        let bc = MapBc {
            format: MapBcFormat::Bc1,
            levels: vec![[0xf800, 0x07e0, 0x001f, 0xffff].iter().flat_map(|&c| block(c)).collect(), block(0x07e0), block(0x001f), block(0xf800)],
        };
        let mut t = MapTexture {
            name: "t".into(),
            srgb: true,
            mips: Vec::new(),
            width: 8,
            height: 8,
            rgba8: Vec::new(),
            bc: Some(bc),
        };
        t.rgba8 = decode_level(t.bc.as_ref().unwrap(), 8, 8, 0).unwrap();
        t.mips = (1..4).map(|i| decode_level(t.bc.as_ref().unwrap(), 8, 8, i).unwrap()).collect();
        t
    }

    fn round_trip(t: &MapTexture) -> (MapTexture, usize) {
        let bytes = postcard::to_allocvec(t).unwrap();
        (postcard::from_bytes(&bytes).unwrap(), bytes.len())
    }

    fn same(a: &MapTexture, b: &MapTexture) {
        assert_eq!((&a.name, a.srgb, a.width, a.height), (&b.name, b.srgb, b.width, b.height));
        assert_eq!((&a.rgba8, &a.mips, &a.bc), (&b.rgba8, &b.mips, &b.bc));
    }

    #[test]
    fn decoded_texels_are_left_out_and_decoded_again() {
        let t = texture();
        assert_eq!(decoded(&t), RGBA8_DECODED | MIPS_DECODED);
        let (back, size) = round_trip(&t);
        same(&t, &back);
        assert!(size < t.rgba8.len(), "the texels aren't stored: {size} bytes");
    }

    #[test]
    fn texels_that_arent_the_blocks_decoded_are_kept() {
        // Box-filtered mips (Combat Arms), a texel changed after decoding.
        let mut t = texture();
        t.mips = t.box_mips();
        t.mips[0][0] ^= 1;
        assert_eq!(decoded(&t), RGBA8_DECODED);
        same(&t, &round_trip(&t).0);
        t.rgba8[5] ^= 1;
        assert_eq!(decoded(&t), 0);
        same(&t, &round_trip(&t).0);
        // No blocks (lean placeholders, other formats).
        let plain = MapTexture { bc: None, ..texture() };
        assert_eq!(decoded(&plain), 0);
        same(&plain, &round_trip(&plain).0);
    }
}
