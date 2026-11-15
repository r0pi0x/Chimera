//! The tag data's primitive value types (specs/halo_ce/cache_file.md,
//! "Primitive types") as plain Rust values, and the declarative layer for
//! class readers: `Field` (a type that reads itself at an offset of a
//! `View`), `TagStruct` (a struct of fixed size read from a view) and the
//! `tag_struct!` macro, which turns a spec's field table into a struct
//! with a `TagStruct` impl and compile-time checks that every field fits.
//!
//! Points and vectors are arrays in stored order (`[x, y, z]`, `[i, j,
//! k]`), quaternions `[i, j, k, w]` (vector part first, as stored);
//! everything with a less obvious order (colours, planes, angles, bounds,
//! rectangles) is a struct with named fields.

use std::io;

use super::view::{DataRef, TagRef, View};
use super::{ClassCode, TagId};

/// A 2D integer rectangle (`rectangle2d`): stored top, left, bottom, right.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Rect2d {
    pub top: i16,
    pub left: i16,
    pub bottom: i16,
    pub right: i16,
}

/// Yaw and pitch in radians (`real_euler_angles2d`).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Euler2d {
    pub yaw: f32,
    pub pitch: f32,
}

/// Yaw, pitch and roll in radians (`real_euler_angles3d`).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Euler3d {
    pub yaw: f32,
    pub pitch: f32,
    pub roll: f32,
}

/// A 2D plane (`real_plane2d`): points p on it satisfy `normal . p = d`.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Plane2d {
    pub normal: [f32; 2],
    pub d: f32,
}

/// A 3D plane (`real_plane3d`): points p on it satisfy `normal . p = d`.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Plane3d {
    pub normal: [f32; 3],
    pub d: f32,
}

/// `real_rgb_color`: stored red, green, blue; 0..1.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct RealRgb {
    pub r: f32,
    pub g: f32,
    pub b: f32,
}

/// `real_argb_color`: stored alpha first, then red, green, blue.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct RealArgb {
    pub a: f32,
    pub r: f32,
    pub g: f32,
    pub b: f32,
}

/// `real_hsv_color`: stored hue, saturation, value; 0..1 each.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct RealHsv {
    pub h: f32,
    pub s: f32,
    pub v: f32,
}

/// `real_ahsv_color`: stored alpha, hue, saturation, value.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct RealAhsv {
    pub a: f32,
    pub h: f32,
    pub s: f32,
    pub v: f32,
}

/// `rgb_color`: a u32 0x??RRGGBB (bytes in the file B, G, R, ignored).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Rgb8 {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

/// `argb_color`: a u32 0xAARRGGBB (bytes in the file B, G, R, A).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Argb8 {
    pub a: u8,
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

/// A range (`short_bounds`, `real_bounds`, `angle_bounds`,
/// `fraction_bounds`): stored lower, upper.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Bounds<T> {
    pub lower: T,
    pub upper: T,
}

/// `real_rectangle2d`: stored x0, x1, y0, y1 (lower then upper per axis).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct RealRect2d {
    pub x: Bounds<f32>,
    pub y: Bounds<f32>,
}

/// `real_rectangle3d`: stored x0, x1, y0, y1, z0, z1.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct RealRect3d {
    pub x: Bounds<f32>,
    pub y: Bounds<f32>,
    pub z: Bounds<f32>,
}

/// A value stored at a fixed offset in a struct: how many bytes it takes
/// and how to read it from a view. `tag_struct!` reads each field through
/// this. The spec's primitive names map to these types:
///
/// | spec | Rust |
/// |---|---|
/// | i8 u8 i16 u16 i32 u32, enum16, flags8/16/32, index8/16/32, block flags | `i8` `u8` `i16` `u16` `i32` `u32` (enum16 = `i16`, flags = unsigned, index = signed; check indices with `View::index16` & co.) |
/// | f32, fraction, angle | `f32` |
/// | tag string | `String` (32 bytes, NUL-terminated) |
/// | class code, tag id | `ClassCode` (a NONE slot reads as `ClassCode(0xFFFFFFFF)`), `TagId` |
/// | point2d | `[i16; 2]` |
/// | real_point2d / real_vector2d | `[f32; 2]` |
/// | real_point3d / real_vector3d | `[f32; 3]` |
/// | real_quaternion | `[f32; 4]` (i, j, k, w) |
/// | the others | the struct of the same name above |
/// | tag block | `Vec<T>` with `T: TagStruct` (the element), read whole |
/// | tag reference, data reference | `TagRef`, `DataRef` |
pub trait Field: Sized {
    /// Bytes the field takes in its struct.
    const SIZE: usize;
    /// Read the field at `offset` of `v`.
    fn read(v: &View<'_>, offset: usize) -> io::Result<Self>;
}

/// A fixed-size struct of tag data (a tag's main struct, a block's
/// element). Written by hand or by `tag_struct!`.
pub trait TagStruct: Sized {
    /// The struct's size in bytes (from the class spec).
    const SIZE: usize;
    /// Read from a view of exactly `SIZE` bytes.
    fn read(v: &View<'_>) -> io::Result<Self>;
}

macro_rules! field_via {
    ($($t:ty => $size:literal, $method:ident;)*) => {$(
        impl Field for $t {
            const SIZE: usize = $size;
            fn read(v: &View<'_>, offset: usize) -> io::Result<Self> {
                v.$method(offset)
            }
        }
    )*};
}

field_via! {
    u8 => 1, u8;
    i8 => 1, i8;
    u16 => 2, u16;
    i16 => 2, i16;
    u32 => 4, u32;
    i32 => 4, i32;
    f32 => 4, f32;
    ClassCode => 4, class;
    TagId => 4, tag_id;
    [i16; 2] => 4, point2d;
    [f32; 2] => 8, point2;
    [f32; 3] => 12, point3;
    [f32; 4] => 16, quaternion;
    Rect2d => 8, rect2d;
    Euler2d => 8, euler2d;
    Euler3d => 12, euler3d;
    Plane2d => 12, plane2d;
    Plane3d => 16, plane3d;
    RealRgb => 12, real_rgb;
    RealArgb => 16, real_argb;
    RealHsv => 12, real_hsv;
    RealAhsv => 16, real_ahsv;
    Rgb8 => 4, rgb8;
    Argb8 => 4, argb8;
    Bounds<i16> => 4, short_bounds;
    Bounds<f32> => 8, real_bounds;
    RealRect2d => 16, real_rect2d;
    RealRect3d => 24, real_rect3d;
    TagRef => 16, tag_ref;
    DataRef => 20, data_ref;
}

impl Field for String {
    const SIZE: usize = 32;
    fn read(v: &View<'_>, offset: usize) -> io::Result<Self> {
        v.tag_string(offset).map(str::to_owned)
    }
}

/// A tag block field read whole: every element as `T`.
impl<T: TagStruct> Field for Vec<T> {
    const SIZE: usize = 12;
    fn read(v: &View<'_>, offset: usize) -> io::Result<Self> {
        v.block(offset, T::SIZE)?.read_all()
    }
}

/// Declares a struct from a class spec's field table and implements
/// `TagStruct` for it. Each field is `offset name: Type` with a `Field`
/// type; offsets are the spec's, the size after the name is the struct's
/// size. Fields left out (padding, runtime words, fields not needed yet)
/// are skipped. A field that would end past the size, or a block whose
/// element type is wrong, fails to compile.
///
/// ```ignore
/// blam_formats::tag_struct! {
///     /// A player spawn (scenario.md).
///     pub struct Spawn: 0x34 {
///         0x00 position: [f32; 3],
///         0x0C facing: f32,
///     }
/// }
/// let spawns: Vec<Spawn> = scenario.block(0x354, Spawn::SIZE)?.read_all()?;
/// ```
#[macro_export]
macro_rules! tag_struct {
    (
        $(#[$meta:meta])*
        $vis:vis struct $name:ident : $size:literal {
            $( $(#[$fmeta:meta])* $off:literal $field:ident : $ty:ty ),* $(,)?
        }
    ) => {
        $(#[$meta])*
        #[derive(Clone, Debug, PartialEq)]
        $vis struct $name {
            $( $(#[$fmeta])* pub $field: $ty, )*
        }

        impl $crate::cache::TagStruct for $name {
            const SIZE: usize = $size;
            fn read(v: &$crate::cache::View<'_>) -> ::std::io::Result<Self> {
                Ok(Self {
                    $( $field: <$ty as $crate::cache::Field>::read(v, $off)?, )*
                })
            }
        }

        const _: () = {
            $(
                assert!(
                    $off + <$ty as $crate::cache::Field>::SIZE <= $size,
                    concat!(stringify!($name), ".", stringify!($field), " ends past the struct's size"),
                );
            )*
        };
    };
}
