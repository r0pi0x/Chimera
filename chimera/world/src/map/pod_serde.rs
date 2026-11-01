//! Big arrays of plain numbers (texels, vertices, lightmap samples) as one
//! byte string each when a map is serialized (`cache`), instead of serde's
//! element by element: `#[serde(with = "pod")]` on a `Vec<T>` of a
//! `bytemuck::Pod` type, `pod_vecs` on a `Vec<Vec<T>>`, `pod_opt3` on an
//! `Option<[Vec<T>; 3]>`, `pod_arrays` on a `[Vec<T>; N]`, `pod_arc` on an
//! `Arc<Vec<T>>`, `pod_opt_arc` on an `Option<Arc<Vec<T>>>`. Reading
//! copies each string out (no alignment needed). The bytes are the
//! machine's own order: the cache is local to the machine.

use std::{marker::PhantomData, sync::Arc};

use bytemuck::Pod;
use serde::{Deserialize, Deserializer, Serialize, Serializer, de::Visitor};

/// A slice written as its bytes.
struct Blob<'a, T>(&'a [T]);

impl<T: Pod> Serialize for Blob<'_, T> {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_bytes(bytemuck::cast_slice(self.0))
    }
}

/// A vector read back from its bytes.
struct Owned<T>(Vec<T>);

struct BlobVisitor<T>(PhantomData<T>);

impl<'de, T: Pod> Visitor<'de> for BlobVisitor<T> {
    type Value = Owned<T>;

    fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "bytes of {}", std::any::type_name::<T>())
    }

    fn visit_bytes<E: serde::de::Error>(self, b: &[u8]) -> Result<Self::Value, E> {
        if !b.len().is_multiple_of(size_of::<T>().max(1)) {
            return Err(E::invalid_length(b.len(), &self));
        }
        // One copy when the bytes happen to be aligned for `T` (bytes
        // always are), else element by element.
        Ok(Owned(match bytemuck::try_cast_slice::<u8, T>(b) {
            Ok(s) => s.to_vec(),
            Err(_) => bytemuck::pod_collect_to_vec(b),
        }))
    }

    fn visit_byte_buf<E: serde::de::Error>(self, b: Vec<u8>) -> Result<Self::Value, E> {
        self.visit_bytes(&b)
    }

    fn visit_seq<A: serde::de::SeqAccess<'de>>(self, mut seq: A) -> Result<Self::Value, A::Error> {
        let mut b = Vec::new();
        while let Some(x) = seq.next_element::<u8>()? {
            b.push(x);
        }
        self.visit_bytes(&b)
    }
}

impl<'de, T: Pod> Deserialize<'de> for Owned<T> {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        d.deserialize_bytes(BlobVisitor(PhantomData))
    }
}

/// `Vec<T>`.
pub mod pod {
    use super::*;

    pub fn serialize<T: Pod, S: Serializer>(v: &[T], s: S) -> Result<S::Ok, S::Error> {
        Blob(v).serialize(s)
    }

    pub fn deserialize<'de, T: Pod, D: Deserializer<'de>>(d: D) -> Result<Vec<T>, D::Error> {
        Owned::deserialize(d).map(|o| o.0)
    }
}

/// `Vec<Vec<T>>`.
pub mod pod_vecs {
    use super::*;

    pub fn serialize<T: Pod, S: Serializer>(v: &[Vec<T>], s: S) -> Result<S::Ok, S::Error> {
        s.collect_seq(v.iter().map(|x| Blob(x)))
    }

    pub fn deserialize<'de, T: Pod, D: Deserializer<'de>>(d: D) -> Result<Vec<Vec<T>>, D::Error> {
        Ok(Vec::<Owned<T>>::deserialize(d)?.into_iter().map(|o| o.0).collect())
    }
}

/// `[Vec<T>; N]`.
pub mod pod_arrays {
    use super::*;

    pub fn serialize<T: Pod, S: Serializer, const N: usize>(v: &[Vec<T>; N], s: S) -> Result<S::Ok, S::Error> {
        s.collect_seq(v.iter().map(|x| Blob(x)))
    }

    pub fn deserialize<'de, T: Pod, D: Deserializer<'de>, const N: usize>(d: D) -> Result<[Vec<T>; N], D::Error> {
        let v: Vec<Vec<T>> = Vec::<Owned<T>>::deserialize(d)?.into_iter().map(|o| o.0).collect();
        let n = v.len();
        v.try_into()
            .map_err(|_| serde::de::Error::invalid_length(n, &"the array's length"))
    }
}

/// `Option<[Vec<T>; 3]>`.
pub mod pod_opt3 {
    use super::*;

    pub fn serialize<T: Pod, S: Serializer>(v: &Option<[Vec<T>; 3]>, s: S) -> Result<S::Ok, S::Error> {
        v.as_ref().map(|[a, b, c]| [Blob(a), Blob(b), Blob(c)]).serialize(s)
    }

    pub fn deserialize<'de, T: Pod, D: Deserializer<'de>>(d: D) -> Result<Option<[Vec<T>; 3]>, D::Error> {
        Ok(Option::<[Owned<T>; 3]>::deserialize(d)?.map(|[a, b, c]| [a.0, b.0, c.0]))
    }
}

/// `Arc<Vec<T>>`.
pub mod pod_arc {
    use super::*;

    pub fn serialize<T: Pod, S: Serializer>(v: &Arc<Vec<T>>, s: S) -> Result<S::Ok, S::Error> {
        Blob(v.as_slice()).serialize(s)
    }

    pub fn deserialize<'de, T: Pod, D: Deserializer<'de>>(d: D) -> Result<Arc<Vec<T>>, D::Error> {
        Owned::deserialize(d).map(|o| Arc::new(o.0))
    }
}

/// `Option<Arc<Vec<T>>>`.
pub mod pod_opt_arc {
    use super::*;

    pub fn serialize<T: Pod, S: Serializer>(v: &Option<Arc<Vec<T>>>, s: S) -> Result<S::Ok, S::Error> {
        v.as_ref().map(|v| Blob(v.as_slice())).serialize(s)
    }

    pub fn deserialize<'de, T: Pod, D: Deserializer<'de>>(d: D) -> Result<Option<Arc<Vec<T>>>, D::Error> {
        Ok(Option::<Owned<T>>::deserialize(d)?.map(|o| Arc::new(o.0)))
    }
}

/// `Vec<T>` of big values: each element postcard-encoded on its own and
/// written as a byte string, so they encode and decode on threads (a
/// map's textures, which decode their BC blocks: `texture_serde`).
/// Encoding goes a batch of elements at a time (two per thread), so a
/// streamed entry never holds every element's bytes; decoding takes the
/// biggest first. Reading needs a deserializer that lends its bytes
/// (postcard from a slice).
pub mod par_vec {
    use serde::de::DeserializeOwned;

    use super::*;
    use crate::map::cache::parallel;

    pub fn serialize<T: Serialize + Sync, S: Serializer>(v: &[T], s: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeSeq;
        // A batch at a time, so a map's encoding (`cache`, streamed to its
        // file) never holds all its textures' bytes at once.
        let batch = std::thread::available_parallelism().map_or(4, |t| t.get()) * 2;
        let mut seq = s.serialize_seq(Some(v.len()))?;
        for part in v.chunks(batch) {
            let encoded = parallel(part.len(), |i| postcard::to_allocvec(&part[i]));
            for b in encoded {
                seq.serialize_element(&Blob(&b.map_err(serde::ser::Error::custom)?))?;
            }
        }
        seq.end()
    }

    pub fn deserialize<'de, T: DeserializeOwned + Send, D: Deserializer<'de>>(d: D) -> Result<Vec<T>, D::Error> {
        let encoded: Vec<&'de [u8]> = Vec::deserialize(d)?;
        // The biggest first (a texture's blocks decode in time with their
        // size), so the last thread doesn't finish alone on a big one.
        let mut order: Vec<usize> = (0..encoded.len()).collect();
        order.sort_by_key(|&i| std::cmp::Reverse(encoded[i].len()));
        let decoded = parallel(order.len(), |i| postcard::from_bytes::<T>(encoded[order[i]]));
        let mut out: Vec<Option<T>> = std::iter::repeat_with(|| None).take(encoded.len()).collect();
        for (i, d) in order.into_iter().zip(decoded) {
            out[i] = Some(d.map_err(serde::de::Error::custom)?);
        }
        Ok(out.into_iter().map(|x| x.expect("every element decoded")).collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Serialize, Deserialize, PartialEq, Debug)]
    struct All {
        #[serde(with = "pod")]
        a: Vec<[f32; 3]>,
        #[serde(with = "pod_vecs")]
        b: Vec<Vec<u8>>,
        #[serde(with = "pod_arrays")]
        c: [Vec<u16>; 2],
        #[serde(with = "pod_opt3")]
        d: Option<[Vec<u32>; 3]>,
        #[serde(with = "pod_arc")]
        e: Arc<Vec<u8>>,
        #[serde(with = "pod_opt_arc")]
        f: Option<Arc<Vec<[f32; 3]>>>,
        #[serde(with = "par_vec")]
        g: Vec<(String, Vec<u8>)>,
    }

    #[test]
    fn every_shape_reads_back() {
        let x = All {
            a: vec![[1.0, 2.0, 3.0], [4.5, -0.0, f32::MAX]],
            b: vec![vec![], vec![1, 2, 3]],
            c: [vec![7], vec![8, 9]],
            d: Some([vec![1], vec![], vec![u32::MAX]]),
            e: Arc::new(vec![5; 9]),
            f: None,
            g: (0..40).map(|i| (format!("t{i}"), vec![i as u8; i])).collect(),
        };
        let bytes = postcard::to_allocvec(&x).unwrap();
        assert_eq!(postcard::from_bytes::<All>(&bytes).unwrap(), x);
        // A byte string that isn't a whole number of elements is refused.
        let bad = postcard::to_allocvec(&(vec![0u8; 5],)).unwrap();
        assert!(postcard::from_bytes::<(Owned<[f32; 3]>,)>(&bad).is_err());
    }
}
