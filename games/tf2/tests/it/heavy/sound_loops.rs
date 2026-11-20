//! specs/source/sound_looping.md test cases 1-6 on the install's own
//! WAVs (read through the mount, decoded with `map::sound::decode_clip`):
//! the four Minigun loops loop from sample 0 by their `cue ` chunk, its
//! wind-up and wind-down don't loop, a `smpl`-only loop from 0, a `smpl`
//! loop and a `cue ` loop that start later. Passes with a note without
//! the install. Playing them (the wrap, the stop) is chimera_present's
//! `sound_looping` test.

use chimera_world::map::sound::decode_clip;

use super::maps::install_mount;

/// (file, frames, loop start).
const CASES: [(&str, usize, Option<usize>); 9] = [
    ("sound/weapons/minigun_shoot.wav", 176_441, Some(0)),
    ("sound/weapons/minigun_shoot_crit.wav", 176_441, Some(0)),
    ("sound/weapons/minigun_spin.wav", 26_626, Some(0)),
    ("sound/weapons/minigun_empty.wav", 26_560, Some(0)),
    ("sound/weapons/minigun_wind_up.wav", 43_884, None),
    ("sound/weapons/minigun_wind_down.wav", 146_851, None),
    ("sound/weapons/phlog_loop.wav", 160_064, Some(0)),
    ("sound/vehicles/v8/v8_turbo_on_loop1.wav", 337_855, Some(15_361)),
    ("sound/weapons/rocket_pack_boosters_loop.wav", 315_112, Some(132_566)),
];

#[test]
fn the_loops_by_their_own_chunks_1_to_6() {
    let Some(mount) = install_mount() else { return };
    let mut checked = 0;
    for (file, frames, start) in CASES {
        let Ok(bytes) = mount.read(file) else {
            eprintln!("note: {file} not in the install");
            continue;
        };
        let clip = decode_clip(&bytes).unwrap_or_else(|e| panic!("{file}: {e}"));
        let n = clip.samples.len() / clip.channels.max(1) as usize;
        assert_eq!(n, frames, "{file}: frames");
        assert_eq!(clip.loop_start, start, "{file}: loop start");
        checked += 1;
    }
    assert!(checked >= 6, "the Minigun's six at least: {checked}");
}
