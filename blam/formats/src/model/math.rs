//! The model reader's small maths (specs/halo_ce/models.md): the rigid
//! `Transform` a node or marker is, the game's quaternion convention (a
//! stored quaternion is the usual rotation of its conjugate), the packed
//! vectors and 16-bit coordinates of the Xbox vertex, and the strip to
//! triangle rule. Plain functions of their arguments, no game data.

/// A rigid transform (scale 1) as the game stores matrices: the images of
/// the x, y and z axes (forward, left, up: the matrix's columns) and the
/// position. Maps a point p to `p.x forward + p.y left + p.z up +
/// position`, a vector the same without the position.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Transform {
    pub forward: [f32; 3],
    pub left: [f32; 3],
    pub up: [f32; 3],
    pub position: [f32; 3],
}

impl Transform {
    pub const IDENTITY: Transform = Transform {
        forward: [1.0, 0.0, 0.0],
        left: [0.0, 1.0, 0.0],
        up: [0.0, 0.0, 1.0],
        position: [0.0; 3],
    };

    /// The transform with the rotation of a stored quaternion `[i, j, k,
    /// w]` (the game's convention: see `rotation_axes`) and `position`.
    pub fn from_quaternion(q: [f32; 4], position: [f32; 3]) -> Transform {
        let [forward, left, up] = rotation_axes(q);
        Transform {
            forward,
            left,
            up,
            position,
        }
    }

    /// The image of a vector (no translation).
    pub fn apply_vector(&self, v: [f32; 3]) -> [f32; 3] {
        let (f, l, u) = (self.forward, self.left, self.up);
        [
            v[0] * f[0] + v[1] * l[0] + v[2] * u[0],
            v[0] * f[1] + v[1] * l[1] + v[2] * u[1],
            v[0] * f[2] + v[1] * l[2] + v[2] * u[2],
        ]
    }

    /// The image of a point.
    pub fn apply_point(&self, p: [f32; 3]) -> [f32; 3] {
        let v = self.apply_vector(p);
        [
            v[0] + self.position[0],
            v[1] + self.position[1],
            v[2] + self.position[2],
        ]
    }

    /// `self` applied after `inner`: `self.then(inner)` maps p to
    /// `self(inner(p))`. A child's world transform is `parent.then(local)`.
    pub fn then(&self, inner: &Transform) -> Transform {
        Transform {
            forward: self.apply_vector(inner.forward),
            left: self.apply_vector(inner.left),
            up: self.apply_vector(inner.up),
            position: self.apply_point(inner.position),
        }
    }

    /// The inverse: the transposed rotation and `-R^T position`.
    pub fn inverse(&self) -> Transform {
        let (f, l, u) = (self.forward, self.left, self.up);
        let rt = Transform {
            forward: [f[0], l[0], u[0]],
            left: [f[1], l[1], u[1]],
            up: [f[2], l[2], u[2]],
            position: [0.0; 3],
        };
        let p = rt.apply_vector(self.position);
        Transform {
            position: [-p[0], -p[1], -p[2]],
            ..rt
        }
    }

    /// The largest absolute difference between any two components.
    pub fn max_difference(&self, other: &Transform) -> f32 {
        let a = [self.forward, self.left, self.up, self.position];
        let b = [other.forward, other.left, other.up, other.position];
        a.iter()
            .flatten()
            .zip(b.iter().flatten())
            .map(|(x, y)| (x - y).abs())
            .fold(0.0, f32::max)
    }
}

/// The rotation a stored quaternion `[i, j, k, w]` means in the game's
/// convention, as its forward, left and up axes (the matrix's columns).
/// With `s = 2 / |q|^2` (0 for a zero quaternion) this is the transpose
/// of the usual matrix: the usual rotation of the conjugate. A stored
/// `(0, 0, sin 45, cos 45)` turns +X to -Y.
pub fn rotation_axes(q: [f32; 4]) -> [[f32; 3]; 3] {
    let [i, j, k, w] = q;
    let n = i * i + j * j + k * k + w * w;
    let s = if n == 0.0 { 0.0 } else { 2.0 / n };
    [
        [1.0 - s * (j * j + k * k), s * (i * j - w * k), s * (i * k + w * j)],
        [s * (i * j + w * k), 1.0 - s * (i * i + k * k), s * (j * k - w * i)],
        [s * (i * k - w * j), s * (j * k + w * i), 1.0 - s * (i * i + j * j)],
    ]
}

/// A packed vector (the BSP's and the model vertex's): x in bits 0..10, y
/// in 11..21 (11 bits each), z in 22..31 (10 bits), two's complement;
/// components `(2x + 1) / 2047`, `(2y + 1) / 2047`, `(2z + 1) / 1023`.
pub fn unpack_vector(w: u32) -> [f32; 3] {
    let x = ((w << 21) as i32) >> 21;
    let y = ((w << 10) as i32) >> 21;
    let z = (w as i32) >> 22;
    [
        (2 * x + 1) as f32 / 2047.0,
        (2 * y + 1) as f32 / 2047.0,
        (2 * z + 1) as f32 / 1023.0,
    ]
}

/// A 16-bit coordinate: `(2s + 1) / 65535`.
pub fn unpack_coordinate(s: i16) -> f32 {
    (2 * i32::from(s) + 1) as f32 / 65535.0
}

/// The triangles of a strip, in the game's winding (clockwise seen from
/// the front, the side the vertex normals point to): triangle `t` is
/// `(k[t], k[t+1], k[t+2])` for even t and `(k[t], k[t+2], k[t+1])` for
/// odd t. A triangle with two equal indices (a join) is dropped but still
/// counts for the parity.
pub fn strip_triangles(strip: &[u16]) -> Vec<[u16; 3]> {
    let mut out = Vec::with_capacity(strip.len().saturating_sub(2));
    for (t, w) in strip.windows(3).enumerate() {
        let (a, b, c) = (w[0], w[1], w[2]);
        if a == b || b == c || a == c {
            continue;
        }
        out.push(if t % 2 == 0 { [a, b, c] } else { [a, c, b] });
    }
    out
}
