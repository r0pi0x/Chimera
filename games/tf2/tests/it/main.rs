//! `tf2`'s integration tests, as one test binary (each file a module):
//! what the game tells the shell (`info`), the skeleton on the headless
//! harness (`skeleton`: `harness::Tf2Sim` and the greybox), the spawn
//! rule on made-up entities (`spawns`), TF2's movement and classes
//! (`movement`: movement.md's test cases), ICE against its spec (`ice`),
//! the script reader on fixtures encrypted with throwaway keys
//! (`scripts`), the guns (`weapons`: M2b's four; `weapons_more`: the
//! next eight; `melee`: the nine stock melee weapons and the backstab;
//! `held_guns`: a gun coming into and leaving the hand, its loops and view model;
//! `minigun`: minigun.md's cases, its states, one sound at a time, running dry,
//! the barrel and what other clients hear),
//! a gun's impacts taking TF2's own looks on another game's map
//! (`impacts`), a first-person muzzle flash at the muzzle re-projected
//! into the world view (`flashes`), the guns' other effects (`effects`:
//! the Minigun's constant systems, tracers, explosions and scorches, the
//! backblast), the Flame Thrower's points and the stream on them
//! (`flames`: the same points on the server and every client, damage
//! only where they are, walls), the class sets a match offers (`player_sets`), the ragdolls' lifetime
//! (`ragdolls`: ragdolls.md's cases on a made-up skeleton), the class a
//! rung of several classes draws with the ladder (`rung_classes`), the
//! players' pain, death and kill lines (`voices`), the kill icons from
//! the asset store once their files are here (`kill_icons`), and, in `heavy`, the user's install (its mount, ctf_2fort
//! and cp_badlands converted with their spawns, the converted-map cache's
//! key, a player walking on ctf_2fort, TF2's bodies that come late (`late_bodies`: a class picked mid-match, a body
//! built again once its lightwarp arrives), the class models' animation (`player_anim`: player_animation.md tests 20-36), the guns' looks on another game's map
//! (`looks`: particle systems, impacts, the `players` set's files), M2's weapon sets packed into a
//! scratch store and read back, the scripts decrypted with the user's key
//! when it is set),
//! found through Steam, `[games.tf2] path` or `$CHIMERA_TF2_ROOT`, each
//! test passing with a note without one.
//! `cargo test -p tf2 --features dev --test it`; where tests go:
//! docs/OBSERVABILITY.md, section 1.

mod ice;
mod impacts;
mod info;
mod kill_icons;
mod movement;
mod player_sets;
mod ragdolls;
mod rung_classes;
mod scripts;
mod skeleton;
mod spawns;
mod voices;
mod weapon_scripts;
mod effects;
mod flashes;
mod weapons;
mod melee;
mod weapons_more;
mod held_guns;
mod minigun;
mod flames;
mod blood;

// Slow ones (the user's install): the fast tier skips `heavy::`.
mod heavy {
    mod bodies;
    mod guns;
    mod late_bodies;
    mod looks;
    mod map_cache;
    mod maps;
    mod mount;
    mod pack;
    mod player_anim;
    #[cfg(feature = "ui")]
    mod particles;
    mod scripts;
    mod sound_loops;
    mod walk;
}
