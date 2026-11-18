//! Big arrays of plain numbers in the parsed types (a model's vertices,
//! an animation's keys) written as one byte string each when they
//! serialize, instead of serde's element by element: `#[serde(with =
//! "plain")]` on a `Vec<T>` of a `Plain` type, `plain_vecs` on a
//! `Vec<Vec<T>>`. Little-endian, so the bytes are the same on every
//! machine. What serializes them: a game's kept parts on disk (Combat
//! Arms' rigs, chimera_world's `map::kept`).

use std::marker::PhantomData;

use serde::{Deserialize, Deserializer, Serialize, Serializer, de::Visitor};

/// A fixed-size value of plain numbers.
pub trait Plain: Copy {
    const SIZE: usize;
    fn put(&self, out: &mut Vec<u8>);
    fn get(bytes: &[u8]) -> Self;
}

macro_rules! plain_number {
    ($t:ty) => {
        impl Plain for $t {
            const SIZE: usize = std::mem::size_of::<$t>();
            fn put(&self, out: &mut Vec<u8>) {
                out.extend_from_slice(&self.to_le_bytes());
            }
            fn get(bytes: &[u8]) -> Self {
                <$t>::from_le_bytes(bytes.try_into().expect("sized"))
            }
        }
    };
}

plain_number!(f32);
plain_number!(u16);
plain_number!(u8);

impl<T: Plain, const N: usize> Plain for [T; N] {
    const SIZE: usize = T::SIZE * N;
    fn put(&self, out: &mut Vec<u8>) {
        for x in self {
            x.put(out);
        }
    }
    fn get(bytes: &[u8]) -> Self {
        std::array::from_fn(|i| T::get(&bytes[i * T::SIZE..(i + 1) * T::SIZE]))
    }
}

impl<A: Plain, B: Plain> Plain for (A, B) {
    const SIZE: usize = A::SIZE + B::SIZE;
    fn put(&self, out: &mut Vec<u8>) {
        self.0.put(out);
        self.1.put(out);
    }
    fn get(bytes: &[u8]) -> Self {
        (A::get(&bytes[..A::SIZE]), B::get(&bytes[A::SIZE..]))
    }
}

fn to_bytes<T: Plain>(v: &[T]) -> Vec<u8> {
    let mut out = Vec::with_capacity(v.len() * T::SIZE);
    for x in v {
        x.put(&mut out);
    }
    out
}

fn from_bytes<T: Plain, E: serde::de::Error>(b: &[u8]) -> Result<Vec<T>, E> {
    if T::SIZE == 0 || b.len() % T::SIZE != 0 {
        return Err(E::custom("plain array of a wrong length"));
    }
    Ok(b.chunks_exact(T::SIZE).map(T::get).collect())
}

struct Bytes<T>(PhantomData<T>);

impl<'de, T: Plain> Visitor<'de> for Bytes<T> {
    type Value = Vec<T>;
    fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        f.write_str("a byte string")
    }
    fn visit_bytes<E: serde::de::Error>(self, b: &[u8]) -> Result<Vec<T>, E> {
        from_bytes(b)
    }
    fn visit_byte_buf<E: serde::de::Error>(self, b: Vec<u8>) -> Result<Vec<T>, E> {
        from_bytes(&b)
    }
}

/// `Vec<T>`.
pub mod plain {
    use super::*;

    pub fn serialize<T: Plain, S: Serializer>(v: &[T], s: S) -> Result<S::Ok, S::Error> {
        s.serialize_bytes(&to_bytes(v))
    }

    pub fn deserialize<'de, T: Plain, D: Deserializer<'de>>(d: D) -> Result<Vec<T>, D::Error> {
        d.deserialize_byte_buf(Bytes(PhantomData))
    }
}

/// `Vec<Vec<T>>`.
pub mod plain_vecs {
    use super::*;

    #[derive(Serialize)]
    struct Out<'a, T: Plain>(#[serde(with = "plain")] &'a [T]);

    #[derive(Deserialize)]
    struct In<T: Plain>(#[serde(with = "plain")] Vec<T>);

    pub fn serialize<T: Plain, S: Serializer>(v: &[Vec<T>], s: S) -> Result<S::Ok, S::Error> {
        s.collect_seq(v.iter().map(|x| Out(x)))
    }

    pub fn deserialize<'de, T: Plain, D: Deserializer<'de>>(d: D) -> Result<Vec<Vec<T>>, D::Error> {
        Ok(Vec::<In<T>>::deserialize(d)?.into_iter().map(|x| x.0).collect())
    }
}
