//! Attribute values in Combat Arms' layouts (specs/combat_arms/ui_system.md
//! L2): integer pairs and colours with `.` as a separator, decimal quads,
//! `ON` booleans, and enumerations matched by prefix.

use bevy::math::IVec2;

/// An RGBA colour, 0-255 each (not clamped: the data stays in range).
pub type Rgba = [u8; 4];

/// A piece read as an integer: leading spaces and a sign allowed, digits
/// up to the first other character (none: 0).
fn int(piece: &str) -> i32 {
    let s = piece.trim_start();
    let (neg, digits) = match s.as_bytes().first() {
        Some(b'-') => (true, &s[1..]),
        Some(b'+') => (false, &s[1..]),
        _ => (false, s),
    };
    let n = digits
        .bytes()
        .take_while(u8::is_ascii_digit)
        .fold(0i64, |n, d| (n * 10 + (d - b'0') as i64).min(i32::MAX as i64));
    (if neg { -n } else { n }) as i32
}

/// Every `.` a `,`, then the pieces as integers.
fn ints(s: &str) -> Vec<i32> {
    s.replace('.', ",").split(',').map(int).collect()
}

/// A pair (Size, Position, ...): pieces two at a time, the last complete
/// pair wins; one piece or none gives (1, 1), a dangling odd piece is
/// ignored.
pub fn pair(s: &str) -> IVec2 {
    let v = ints(s);
    if v.len() < 2 {
        return IVec2::ONE;
    }
    let last = (v.len() / 2 - 1) * 2;
    IVec2::new(v[last], v[last + 1])
}

/// A colour: three integers at a time (the last triple wins), opaque, or
/// (0, 0, 0, 0) when the count isn't a multiple of 3.
pub fn colour(s: &str) -> Rgba {
    let v = ints(s);
    if v.is_empty() || !v.len().is_multiple_of(3) {
        return [0; 4];
    }
    let t = &v[v.len() - 3..];
    [t[0] as u8, t[1] as u8, t[2] as u8, 255]
}

/// A colour with its alpha attribute's value.
pub fn colour_alpha(s: &str, alpha: Option<&str>, default: u8) -> Rgba {
    let mut c = colour(s);
    if c != [0; 4] {
        c[3] = alpha.map_or(default as i32, int) as u8;
    }
    c
}

/// A decimal quad (CustomUV, SimpleStretchValue; `.` is the decimal
/// point): fewer than four values give (0, 0, 0, 0).
pub fn quad(s: &str) -> [f32; 4] {
    let v: Vec<f32> = s.split(',').map(|p| p.trim().parse().unwrap_or(0.0)).collect();
    if v.len() < 4 {
        return [0.0; 4];
    }
    [v[0], v[1], v[2], v[3]]
}

/// `ON` in any case.
pub fn boolean(s: &str) -> bool {
    s.eq_ignore_ascii_case("on")
}

/// An integer attribute.
pub fn integer(s: &str) -> i32 {
    int(s)
}

/// The first entry of `table` that begins with `s` (any case; an empty
/// value matches the first entry), else None (the caller's default).
pub fn enumeration<T: Copy>(s: &str, table: &[(&str, T)]) -> Option<T> {
    table
        .iter()
        .find(|(name, _)| name.len() >= s.len() && name[..s.len()].eq_ignore_ascii_case(s))
        .map(|(_, v)| *v)
}
