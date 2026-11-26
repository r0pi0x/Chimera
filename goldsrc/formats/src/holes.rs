//! The colour under an alpha-tested image's holes (specs/goldsrc/wad3.md,
//! "Converting to RGBA": a hole's colour "should not bleed: use black or
//! the neighbours' colour for filtering"). A `{` texture's, a masked
//! studio texture's and an alpha-test sprite's index 255 is a hole, and
//! its palette colour is a key colour (pure blue, (0, 0, 252) or (0, 0,
//! 255), in every stock `{` texture). Filtering mixes a hole's colour into
//! the texels beside it and the mip levels average it in, so a thin leaf
//! or blade showed blue (the key colour kept) or black (holes made black):
//! de_aztec's grass sprites drew blue-purple and its vines black.
//! `fill` gives each hole the mean colour of its nearest opaque texels,
//! growing outwards ring by ring; the alpha stays 0. An image with no
//! opaque texel keeps black holes.

/// Fill the colour of `rgba`'s holes (alpha 0; `width` x `height`, RGBA8
/// row by row) from their nearest opaque texels (module doc).
pub fn fill(rgba: &mut [u8], width: usize, height: usize) {
    let n = width * height;
    if n == 0 || rgba.len() < n * 4 {
        return;
    }
    // Whether each texel's colour is settled (opaque, or filled by an
    // earlier ring).
    let mut done: Vec<bool> = (0..n).map(|i| rgba[i * 4 + 3] != 0).collect();
    if !done.contains(&true) {
        for i in 0..n {
            rgba[i * 4..i * 4 + 3].fill(0);
        }
        return;
    }
    let neighbours = |i: usize| {
        let (x, y) = ((i % width) as isize, (i / width) as isize);
        (-1isize..=1)
            .flat_map(move |dy| (-1isize..=1).map(move |dx| (x + dx, y + dy)))
            .filter_map(move |(nx, ny)| {
                (nx >= 0 && ny >= 0 && (nx as usize) < width && (ny as usize) < height && (nx, ny) != (x, y))
                    .then(|| ny as usize * width + nx as usize)
            })
    };
    // The first ring: holes beside an opaque texel.
    let mut ring: Vec<usize> = (0..n).filter(|&i| !done[i] && neighbours(i).any(|j| done[j])).collect();
    while !ring.is_empty() {
        let colours: Vec<[u8; 3]> = ring
            .iter()
            .map(|&i| {
                let (mut sum, mut count) = ([0u32; 3], 0u32);
                for j in neighbours(i).filter(|&j| done[j]) {
                    for c in 0..3 {
                        sum[c] += rgba[j * 4 + c] as u32;
                    }
                    count += 1;
                }
                sum.map(|s| ((s + count / 2) / count.max(1)) as u8)
            })
            .collect();
        for (&i, c) in ring.iter().zip(&colours) {
            rgba[i * 4..i * 4 + 3].copy_from_slice(c);
            done[i] = true;
        }
        let mut next: Vec<usize> = ring.iter().flat_map(|&i| neighbours(i)).filter(|&j| !done[j]).collect();
        next.sort_unstable();
        next.dedup();
        ring = next;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A green texel among blue-keyed holes: the holes take its green (the
    /// nearest ring first, then the next from it), alpha stays 0, the
    /// opaque texel is untouched.
    #[test]
    fn holes_take_their_nearest_opaque_colour() {
        let (w, h) = (4, 3);
        let mut px: Vec<u8> = (0..w * h).flat_map(|_| [0, 0, 252, 0]).collect();
        px[4..8].copy_from_slice(&[40, 90, 20, 255]);
        fill(&mut px, w, h);
        for (i, p) in px.chunks(4).enumerate() {
            assert_eq!(&p[..3], &[40, 90, 20], "texel {i}");
            assert_eq!(p[3], if i == 1 { 255 } else { 0 }, "texel {i}");
        }
    }

    /// A hole between two colours takes their mean; an image of holes
    /// only is black.
    #[test]
    fn a_hole_between_colours_takes_their_mean_and_all_holes_are_black() {
        let mut px = vec![200, 0, 0, 255, 0, 0, 255, 0, 0, 100, 0, 255];
        fill(&mut px, 3, 1);
        assert_eq!(&px[4..8], &[100, 50, 0, 0]);
        let mut empty = vec![0, 0, 255, 0, 0, 0, 255, 0];
        fill(&mut empty, 2, 1);
        assert_eq!(empty, vec![0; 8]);
    }
}
